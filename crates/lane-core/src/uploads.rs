//! 分块上传会话管理。
//!
//! 每个会话对应接收目录中的一个隐藏临时文件。客户端可查询已经确认的块，
//! 只补传缺失部分；所有块完成后再原子提交到目录表。

use crate::catalog::{Catalog, Item};
use crate::files::{
    commit_temporary_upload, has_upload_capacity, sanitize_filename, SaveUploadError,
};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};

pub const DEFAULT_CHUNK_SIZE: u64 = 8 * 1024 * 1024;
pub const MIN_CHUNK_SIZE: u64 = 256 * 1024;
pub const MAX_CHUNK_SIZE: u64 = 16 * 1024 * 1024;
const SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UploadStatus {
    pub id: String,
    pub file_name: String,
    pub file_size: u64,
    pub chunk_size: u64,
    pub total_chunks: u32,
    pub received_chunks: Vec<u32>,
    pub uploaded_bytes: u64,
    pub completed: bool,
}

pub struct ChunkWriteResult {
    pub status: UploadStatus,
    pub newly_received: bool,
}

struct UploadSession {
    state: Mutex<UploadState>,
}

struct UploadState {
    id: String,
    file_name: String,
    file_size: u64,
    chunk_size: u64,
    temp_path: PathBuf,
    /// 已落盘块及其客户端声明且经服务端验证的 SHA-256。
    received: BTreeMap<u32, String>,
    updated_at: Instant,
    active: bool,
    /// 已提交会话保留为短期 tombstone，使最终响应丢失后的 complete 可幂等重试。
    completed_item: Option<Item>,
}

pub struct UploadManager {
    storage_dir: PathBuf,
    max_upload_bytes: u64,
    sessions: RwLock<HashMap<String, Arc<UploadSession>>>,
}

impl UploadManager {
    pub fn new(storage_dir: PathBuf, max_upload_bytes: u64) -> Self {
        Self {
            storage_dir,
            max_upload_bytes,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create(
        &self,
        file_name: &str,
        file_size: u64,
        requested_chunk_size: Option<u64>,
    ) -> Result<UploadStatus, UploadError> {
        self.cleanup_expired().await;
        let file_name = sanitize_filename(file_name);
        if file_name.is_empty() {
            return Err(UploadError::InvalidName);
        }
        if file_size > self.max_upload_bytes {
            return Err(UploadError::TooLarge);
        }
        match has_upload_capacity(&self.storage_dir, file_size) {
            Ok(true) => {}
            Ok(false) => return Err(UploadError::InsufficientStorage),
            Err(error) => return Err(UploadError::Io(format!("查询可用空间: {error}"))),
        }

        let chunk_size = requested_chunk_size
            .unwrap_or(DEFAULT_CHUNK_SIZE)
            .clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        chunk_count(file_size, chunk_size)?;

        let (id, temp_path, file) = self.create_temp_file().await?;
        if let Err(error) = file.set_len(file_size).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(map_io("预分配临时文件", error));
        }
        drop(file);

        let state = UploadState {
            id: id.clone(),
            file_name,
            file_size,
            chunk_size,
            temp_path,
            received: BTreeMap::new(),
            updated_at: Instant::now(),
            active: true,
            completed_item: None,
        };
        let status = status_from(&state);
        self.sessions.write().await.insert(
            id,
            Arc::new(UploadSession {
                state: Mutex::new(state),
            }),
        );
        Ok(status)
    }

    pub async fn status(&self, id: &str) -> Result<UploadStatus, UploadError> {
        let session = self.session(id).await?;
        let mut state = session.state.lock().await;
        if state.completed_item.is_some() {
            return Ok(status_from(&state));
        }
        if !state.active {
            return Err(UploadError::NotFound);
        }
        if state.updated_at.elapsed() > SESSION_TTL {
            state.active = false;
            let path = state.temp_path.clone();
            drop(state);
            self.sessions.write().await.remove(id);
            let _ = tokio::fs::remove_file(path).await;
            return Err(UploadError::NotFound);
        }
        Ok(status_from(&state))
    }

    pub async fn write_chunk(
        &self,
        id: &str,
        index: u32,
        checksum: &str,
        bytes: &[u8],
    ) -> Result<ChunkWriteResult, UploadError> {
        let session = self.session(id).await?;
        let mut state = session.state.lock().await;
        if state.completed_item.is_some() {
            return Err(UploadError::AlreadyCompleted);
        }
        if !state.active {
            return Err(UploadError::NotFound);
        }
        let expected_len = expected_chunk_len(&state, index)?;
        if bytes.len() as u64 != expected_len {
            return Err(UploadError::InvalidChunk(format!(
                "块大小应为 {expected_len} 字节，实际为 {} 字节",
                bytes.len()
            )));
        }
        let checksum = normalize_checksum(checksum)?;
        let actual = hex_digest(bytes);
        if actual != checksum {
            return Err(UploadError::ChecksumMismatch);
        }
        if let Some(existing) = state.received.get(&index) {
            if existing == &checksum {
                state.updated_at = Instant::now();
                return Ok(ChunkWriteResult {
                    status: status_from(&state),
                    newly_received: false,
                });
            }
            return Err(UploadError::ChunkConflict);
        }

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&state.temp_path)
            .await
            .map_err(|error| map_io("打开分块临时文件", error))?;
        file.seek(std::io::SeekFrom::Start(index as u64 * state.chunk_size))
            .await
            .map_err(|error| map_io("定位分块", error))?;
        file.write_all(bytes)
            .await
            .map_err(|error| map_io("写入分块", error))?;
        file.sync_data()
            .await
            .map_err(|error| map_io("同步分块", error))?;

        state.received.insert(index, checksum);
        state.updated_at = Instant::now();
        Ok(ChunkWriteResult {
            status: status_from(&state),
            newly_received: true,
        })
    }

    pub async fn complete(
        &self,
        id: &str,
        catalog: &Catalog,
        file_mu: &Mutex<()>,
    ) -> Result<Item, UploadError> {
        let session = self.session(id).await?;
        let mut state = session.state.lock().await;
        if let Some(item) = &state.completed_item {
            return Ok(item.clone());
        }
        if !state.active {
            return Err(UploadError::NotFound);
        }
        if state.received.len() != chunk_count(state.file_size, state.chunk_size)? as usize {
            return Err(UploadError::Incomplete);
        }
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&state.temp_path)
            .await
            .map_err(|error| map_io("打开待提交文件", error))?;
        let metadata = file
            .metadata()
            .await
            .map_err(|error| map_io("读取待提交文件", error))?;
        if metadata.len() != state.file_size {
            return Err(UploadError::Incomplete);
        }
        file.sync_all()
            .await
            .map_err(|error| map_io("同步待提交文件", error))?;
        drop(file);
        state.active = false;
        let result = commit_temporary_upload(
            catalog,
            &self.storage_dir,
            &state.temp_path,
            &state.file_name,
            file_mu,
        )
        .await
        .map_err(UploadError::Commit);
        match &result {
            Ok(item) => {
                state.completed_item = Some(item.clone());
                state.updated_at = Instant::now();
            }
            Err(_) => {
                state.active = true;
                state.updated_at = Instant::now();
            }
        }
        drop(state);
        result
    }

    pub async fn cancel(&self, id: &str) -> Result<(), UploadError> {
        let session = self.session(id).await?;
        let mut state = session.state.lock().await;
        if state.completed_item.is_some() {
            drop(state);
            self.sessions.write().await.remove(id);
            return Ok(());
        }
        if !state.active {
            return Err(UploadError::NotFound);
        }
        state.active = false;
        let path = state.temp_path.clone();
        drop(state);
        self.sessions.write().await.remove(id);
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(map_io("删除分块临时文件", error)),
        }
    }

    async fn session(&self, id: &str) -> Result<Arc<UploadSession>, UploadError> {
        self.sessions
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or(UploadError::NotFound)
    }

    async fn create_temp_file(&self) -> Result<(String, PathBuf, tokio::fs::File), UploadError> {
        for _ in 0..8 {
            let id = random_hex(8);
            let path = self
                .storage_dir
                .join(format!(".laneshare-{}-{id}.part", std::process::id()));
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await
            {
                Ok(file) => return Ok((id, path, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(map_io("创建分块临时文件", error)),
            }
        }
        Err(UploadError::Io("无法生成唯一上传会话".to_string()))
    }

    async fn cleanup_expired(&self) {
        let sessions: Vec<(String, Arc<UploadSession>)> = self
            .sessions
            .read()
            .await
            .iter()
            .map(|(id, session)| (id.clone(), Arc::clone(session)))
            .collect();
        for (id, session) in sessions {
            let mut state = session.state.lock().await;
            if state.updated_at.elapsed() <= SESSION_TTL {
                continue;
            }
            let should_remove_file = state.completed_item.is_none();
            state.active = false;
            let path = state.temp_path.clone();
            drop(state);
            self.sessions.write().await.remove(&id);
            if should_remove_file {
                let _ = tokio::fs::remove_file(path).await;
            }
        }
    }
}

fn chunk_count(file_size: u64, chunk_size: u64) -> Result<u32, UploadError> {
    let count = file_size.div_ceil(chunk_size);
    u32::try_from(count).map_err(|_| UploadError::TooLarge)
}

fn expected_chunk_len(state: &UploadState, index: u32) -> Result<u64, UploadError> {
    let total = chunk_count(state.file_size, state.chunk_size)?;
    if index >= total {
        return Err(UploadError::InvalidChunk("分块索引超出范围".to_string()));
    }
    let offset = index as u64 * state.chunk_size;
    Ok((state.file_size - offset).min(state.chunk_size))
}

fn status_from(state: &UploadState) -> UploadStatus {
    let uploaded_bytes = state
        .received
        .keys()
        .filter_map(|index| expected_chunk_len(state, *index).ok())
        .sum();
    UploadStatus {
        id: state.id.clone(),
        file_name: state.file_name.clone(),
        file_size: state.file_size,
        chunk_size: state.chunk_size,
        total_chunks: chunk_count(state.file_size, state.chunk_size).unwrap_or(0),
        received_chunks: state.received.keys().copied().collect(),
        uploaded_bytes,
        completed: state.completed_item.is_some(),
    }
}

fn normalize_checksum(checksum: &str) -> Result<String, UploadError> {
    let checksum = checksum.trim().to_ascii_lowercase();
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(UploadError::InvalidChecksum);
    }
    Ok(checksum)
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn random_hex(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn map_io(context: &str, error: std::io::Error) -> UploadError {
    if matches!(error.raw_os_error(), Some(28 | 39 | 112)) {
        UploadError::InsufficientStorage
    } else {
        UploadError::Io(format!("{context}: {error}"))
    }
}

#[derive(Debug)]
pub enum UploadError {
    InvalidName,
    TooLarge,
    InsufficientStorage,
    NotFound,
    InvalidChecksum,
    ChecksumMismatch,
    ChunkConflict,
    InvalidChunk(String),
    Incomplete,
    AlreadyCompleted,
    Io(String),
    Commit(SaveUploadError),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => write!(formatter, "文件名无效"),
            Self::TooLarge => write!(formatter, "文件超过大小限制"),
            Self::InsufficientStorage => write!(formatter, "接收目录可用空间不足"),
            Self::NotFound => write!(formatter, "上传会话不存在或已过期"),
            Self::InvalidChecksum => write!(formatter, "SHA-256 校验值格式无效"),
            Self::ChecksumMismatch => write!(formatter, "分块 SHA-256 校验失败"),
            Self::ChunkConflict => write!(formatter, "分块已存在且校验值不一致"),
            Self::InvalidChunk(message) => write!(formatter, "{message}"),
            Self::Incomplete => write!(formatter, "仍有分块尚未上传"),
            Self::AlreadyCompleted => write!(formatter, "上传会话已经完成"),
            Self::Io(message) => write!(formatter, "{message}"),
            Self::Commit(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_lengths_include_short_tail() {
        let state = UploadState {
            id: "id".into(),
            file_name: "file".into(),
            file_size: MIN_CHUNK_SIZE + 7,
            chunk_size: MIN_CHUNK_SIZE,
            temp_path: std::path::Path::new("part").to_path_buf(),
            received: BTreeMap::new(),
            updated_at: Instant::now(),
            active: true,
            completed_item: None,
        };
        assert_eq!(chunk_count(state.file_size, state.chunk_size).unwrap(), 2);
        assert_eq!(expected_chunk_len(&state, 0).unwrap(), MIN_CHUNK_SIZE);
        assert_eq!(expected_chunk_len(&state, 1).unwrap(), 7);
        assert!(expected_chunk_len(&state, 2).is_err());
    }
}
