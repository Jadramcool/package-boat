//! 共享目录表：`catalog.json`。`linked` 条目只保存源文件绝对路径（不复制、不移动），
//! `received` 条目只由本次服务明确接收的上传文件产生。所有变更先落盘再提交内存状态。

use crate::CATALOG_VERSION;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// 条目来源类型，JSON 序列化为小写。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Linked,
    Received,
}

/// 目录表条目，字段与 Go 版 `catalog.Item` 一致（`is_dir` 为文件夹共享新增）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct Item {
    pub id: String,
    pub name: String,
    pub source_type: SourceType,
    pub local_path: String,
    /// 字节数；specta 导出为 number（TS 侧 2^53 内精度安全）。
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub size: i64,
    pub modified_at: DateTime<Utc>,
    pub added_at: DateTime<Utc>,
    pub available: bool,
    /// 是否为文件夹（共享文件夹时下载打包为 zip）。旧数据无此字段，默认 false。
    #[serde(default)]
    pub is_dir: bool,
}

#[derive(Serialize, Deserialize)]
struct PersistedCatalog {
    version: i32,
    items: Vec<Item>,
}

/// 共享目录表存储。`path` 为空时仅内存操作（测试用）。
pub struct Catalog {
    path: PathBuf,
    inner: RwLock<Vec<Item>>,
}

impl Catalog {
    /// 打开目录表；文件不存在时得到空目录表。
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let catalog = Catalog {
            path,
            inner: RwLock::new(Vec::new()),
        };
        if !catalog.path.as_os_str().is_empty() {
            catalog.load()?;
        }
        Ok(catalog)
    }

    /// 原位共享：验证并追加多个源文件路径。任一文件无效则整体失败，不做任何修改。
    pub fn add_linked(&self, paths: &[String]) -> Result<Vec<Item>, String> {
        if paths.is_empty() {
            return Err("没有选择文件".to_string());
        }

        let mut validated: Vec<Item> = Vec::with_capacity(paths.len());
        let mut seen: HashSet<String> = HashSet::with_capacity(paths.len());
        for raw in paths {
            let absolute =
                std::path::absolute(raw).map_err(|err| format!("解析文件路径 {raw:?}: {err}"))?;
            let metadata = std::fs::symlink_metadata(&absolute)
                .map_err(|err| format!("读取文件 {:?}: {err}", absolute))?;
            validate_shareable(&absolute, &metadata)?;

            let key = absolute.to_string_lossy().to_lowercase();
            if !seen.insert(key) {
                continue;
            }
            let is_dir = metadata.file_type().is_dir();
            validated.push(Item {
                id: String::new(),
                name: file_name_of(&absolute).to_string_lossy().into_owned(),
                source_type: SourceType::Linked,
                local_path: absolute.to_string_lossy().into_owned(),
                size: metadata.len() as i64,
                modified_at: DateTime::from(
                    metadata
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                ),
                added_at: Utc::now(),
                available: true,
                is_dir,
            });
        }

        let mut guard = self.inner.write().expect("catalog lock poisoned");
        let start_len = guard.len();
        let mut added: Vec<Item> = Vec::with_capacity(validated.len());
        for mut candidate in validated {
            if find_by_path(&guard, &candidate.local_path).is_some() {
                continue;
            }
            candidate.id = new_id();
            candidate.added_at = Utc::now();
            guard.push(candidate.clone());
            added.push(candidate);
        }
        if !added.is_empty() {
            if let Err(err) = self.save_locked(&guard) {
                guard.truncate(start_len);
                return Err(err);
            }
        }
        Ok(added)
    }

    /// 记录一个接收目录中的文件；路径已存在时直接返回原条目。
    pub fn add_received(&self, path: PathBuf) -> Result<Item, String> {
        let absolute = std::path::absolute(&path).map_err(|err| format!("解析接收路径: {err}"))?;
        let metadata =
            std::fs::symlink_metadata(&absolute).map_err(|err| format!("读取接收文件: {err}"))?;
        // 接收路径必须是普通文件（上传落盘/接收目录导入）
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err("接收路径不是普通文件".to_string());
        }

        let mut guard = self.inner.write().expect("catalog lock poisoned");
        if let Some(existing) = find_by_path(&guard, &absolute.to_string_lossy()) {
            return Ok(existing.clone());
        }
        let item = Item {
            id: new_id(),
            name: file_name_of(&absolute).to_string_lossy().into_owned(),
            source_type: SourceType::Received,
            local_path: absolute.to_string_lossy().into_owned(),
            size: metadata.len() as i64,
            modified_at: DateTime::from(
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            ),
            added_at: Utc::now(),
            available: true,
            is_dir: false,
        };
        guard.push(item.clone());
        if let Err(err) = self.save_locked(&guard) {
            guard.pop();
            return Err(err);
        }
        Ok(item)
    }

    /// 列出全部条目：刷新每个条目的可用性与大小/修改时间，按加入时间倒序（稳定排序）。
    /// 磁盘元数据读取在锁外进行：慢速盘/大目录的 stat 不再阻塞并发的
    /// 列表查询、下载解析与上传登记。
    pub fn list(&self) -> Vec<Item> {
        let mut result = {
            let guard = self.inner.read().expect("catalog lock poisoned");
            guard.clone()
        };
        for item in result.iter_mut() {
            if let Ok(metadata) = std::fs::symlink_metadata(&item.local_path) {
                let file_type = metadata.file_type();
                let valid = !file_type.is_symlink() && (file_type.is_file() || file_type.is_dir());
                if valid {
                    item.available = true;
                    item.name = file_name_of(Path::new(&item.local_path))
                        .to_string_lossy()
                        .into_owned();
                    item.size = metadata.len() as i64;
                    item.modified_at = DateTime::from(
                        metadata
                            .modified()
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                    );
                    continue;
                }
            }
            item.available = false;
        }
        result.sort_by_key(|item| std::cmp::Reverse(item.added_at));
        result
    }

    /// 按 id 解析条目与文件元数据；目标不可用（缺失/符号链接/非普通文件或目录）时返回错误。
    pub fn resolve(&self, id: &str) -> Result<(Item, std::fs::Metadata), std::io::Error> {
        let guard = self.inner.read().expect("catalog lock poisoned");
        for item in guard.iter() {
            if item.id != id {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&item.local_path)?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() || (!file_type.is_file() && !file_type.is_dir()) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "target unavailable",
                ));
            }
            return Ok((item.clone(), metadata));
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ))
    }

    /// 按 id 查询条目。
    pub fn get(&self, id: &str) -> Option<Item> {
        let guard = self.inner.read().expect("catalog lock poisoned");
        guard.iter().find(|item| item.id == id).cloned()
    }

    /// 移除条目（仅目录表，不触碰源文件）；保存失败时回滚。
    pub fn remove(&self, id: &str) -> Result<Item, String> {
        let mut guard = self.inner.write().expect("catalog lock poisoned");
        let index = guard.iter().position(|item| item.id == id);
        let index = match index {
            Some(index) => index,
            None => return Err("条目不存在".to_string()),
        };
        let removed = guard.remove(index);
        if let Err(err) = self.save_locked(&guard) {
            guard.insert(index, removed.clone());
            return Err(err);
        }
        Ok(removed)
    }

    /// 清空共享目录表，但不删除任何原位共享文件或接收文件。
    pub fn clear(&self) -> Result<usize, String> {
        let mut guard = self.inner.write().expect("catalog lock poisoned");
        if guard.is_empty() {
            return Ok(0);
        }

        let previous = std::mem::take(&mut *guard);
        if let Err(err) = self.save_locked(&[]) {
            *guard = previous;
            return Err(err);
        }
        Ok(previous.len())
    }

    fn load(&self) -> Result<(), String> {
        let content = match std::fs::read(&self.path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(format!("读取共享目录表: {err}")),
        };
        let stored: PersistedCatalog =
            serde_json::from_slice(&content).map_err(|err| format!("解析共享目录表: {err}"))?;
        let mut items = match stored.version {
            CATALOG_VERSION => stored.items,
            1 => {
                // v1 会在启动时扫描接收目录，无法区分自动导入文件与真实上传文件。
                // 为避免升级后继续意外暴露整个目录，只迁移原位共享条目；磁盘文件不删除。
                let mut items = stored.items;
                items.retain(|item| item.source_type != SourceType::Received);
                self.save_locked(&items)?;
                items
            }
            version => return Err(format!("不支持的共享目录表版本: {version}")),
        };
        *self.inner.write().expect("catalog lock poisoned") = std::mem::take(&mut items);
        Ok(())
    }

    fn save_locked(&self, items: &[Item]) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        let persisted = PersistedCatalog {
            version: CATALOG_VERSION,
            items: items.to_vec(),
        };
        crate::settings::atomic_write_json(&self.path, &persisted)
    }
}

/// 校验原位共享目标：拒绝符号链接，允许普通文件或文件夹。
fn validate_shareable(path: &Path, metadata: &std::fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "暂不支持符号链接：{}",
            file_name_of(path).to_string_lossy()
        ));
    }
    if !metadata.file_type().is_file() && !metadata.file_type().is_dir() {
        return Err(format!(
            "暂时只支持文件或文件夹：{}",
            file_name_of(path).to_string_lossy()
        ));
    }
    Ok(())
}

fn file_name_of(path: &Path) -> &std::ffi::OsStr {
    path.file_name().unwrap_or(path.as_os_str())
}

/// 大小写不敏感的路径比较（Windows 语义；对非 ASCII 字符做 Unicode 简单折叠）。
fn find_by_path<'a>(items: &'a [Item], path: &str) -> Option<&'a Item> {
    items
        .iter()
        .find(|item| path_eq_ignore_case(&item.local_path, path))
}

fn path_eq_ignore_case(a: &str, b: &str) -> bool {
    let a: String = a.chars().flat_map(char::to_lowercase).collect();
    let b: String = b.chars().flat_map(char::to_lowercase).collect();
    a == b
}

/// 16 字节随机数的十六进制编码（32 字符），与 Go 版 `randomID` 一致。
fn new_id() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("packetboat-catalog-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn linked_file_persisted_without_copying() {
        let root = temp_dir("persist");
        let original = root.join("original.txt");
        std::fs::write(&original, b"linked content").unwrap();
        let catalog_path = root.join("data").join("catalog.json");

        let store = Catalog::open(catalog_path.clone()).unwrap();
        let added = store
            .add_linked(&[original.to_string_lossy().into_owned()])
            .unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(
            added[0].local_path,
            original.to_string_lossy().replace('/', "\\")
        );
        assert_eq!(added[0].source_type, SourceType::Linked);

        let reloaded = Catalog::open(catalog_path).unwrap();
        let items = reloaded.list();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].local_path,
            original.to_string_lossy().replace('/', "\\")
        );
        assert!(original.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn removing_linked_item_never_deletes_original() {
        let root = temp_dir("keep");
        let original = root.join("keep.txt");
        std::fs::write(&original, b"keep me").unwrap();
        let store = Catalog::open(PathBuf::new()).unwrap();
        let added = store
            .add_linked(&[original.to_string_lossy().into_owned()])
            .unwrap();
        store.remove(&added[0].id).unwrap();
        assert_eq!(std::fs::read_to_string(&original).unwrap(), "keep me");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unavailable_linked_item_remains_in_catalog() {
        let root = temp_dir("unavailable");
        let original = root.join("temporary.txt");
        std::fs::write(&original, b"temporary").unwrap();
        let store = Catalog::open(PathBuf::new()).unwrap();
        let added = store
            .add_linked(&[original.to_string_lossy().into_owned()])
            .unwrap();
        std::fs::remove_file(&original).unwrap();
        let items = store.list();
        assert_eq!(items.len(), 1);
        assert!(!items[0].available);
        assert!(store.resolve(&added[0].id).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn folders_are_accepted_and_marked() {
        let store = Catalog::open(PathBuf::new()).unwrap();
        let dir = temp_dir("folder-share");
        let added = store
            .add_linked(&[dir.to_string_lossy().into_owned()])
            .unwrap();
        assert_eq!(added.len(), 1);
        assert!(added[0].is_dir);
        let items = store.list();
        assert_eq!(items.len(), 1);
        assert!(items[0].available);
        assert!(items[0].is_dir);
        // 解析也应成功（目录可用）
        assert!(store.resolve(&added[0].id).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicates_skipped_case_insensitively() {
        let root = temp_dir("dup");
        let original = root.join("Report.TXT");
        std::fs::write(&original, b"x").unwrap();
        let store = Catalog::open(PathBuf::new()).unwrap();
        let upper = original.to_string_lossy().to_uppercase();
        let first = store
            .add_linked(&[original.to_string_lossy().into_owned()])
            .unwrap();
        assert_eq!(first.len(), 1);
        let second = store.add_linked(&[upper]).unwrap();
        assert!(second.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn received_files_are_registered_only_when_explicitly_added() {
        let root = temp_dir("explicit-received");
        let received = root.join("received");
        std::fs::create_dir_all(&received).unwrap();
        let uploaded = received.join("uploaded.txt");
        std::fs::write(&uploaded, b"uploaded").unwrap();
        std::fs::write(received.join("unrelated.txt"), b"unrelated").unwrap();

        let catalog = Catalog::open(PathBuf::new()).unwrap();
        catalog.add_received(uploaded).unwrap();

        let items = catalog.list();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "uploaded.txt");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_received_items_are_removed_without_deleting_files() {
        let root = temp_dir("legacy-received");
        let existing = root.join("existing.txt");
        std::fs::write(&existing, b"keep on disk").unwrap();
        let catalog_path = root.join("catalog.json");
        let timestamp = Utc::now();
        let legacy = serde_json::json!({
            "version": 1,
            "items": [{
                "id": "legacy-received",
                "name": "existing.txt",
                "source_type": "received",
                "local_path": existing.to_string_lossy(),
                "size": 12,
                "modified_at": timestamp,
                "added_at": timestamp,
                "available": true,
                "is_dir": false
            }]
        });
        std::fs::write(&catalog_path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let catalog = Catalog::open(catalog_path.clone()).unwrap();
        assert!(catalog.list().is_empty());
        assert!(existing.exists());

        let migrated: serde_json::Value =
            serde_json::from_slice(&std::fs::read(catalog_path).unwrap()).unwrap();
        assert_eq!(migrated["version"], CATALOG_VERSION);
        assert!(migrated["items"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clearing_catalog_preserves_files() {
        let root = temp_dir("clear");
        let linked_file = root.join("linked.txt");
        let received_file = root.join("received.txt");
        std::fs::write(&linked_file, b"linked").unwrap();
        std::fs::write(&received_file, b"received").unwrap();

        let catalog = Catalog::open(root.join("catalog.json")).unwrap();
        catalog
            .add_linked(&[linked_file.to_string_lossy().into_owned()])
            .unwrap();
        catalog.add_received(received_file.clone()).unwrap();

        assert_eq!(catalog.clear().unwrap(), 2);
        assert!(catalog.list().is_empty());
        assert!(linked_file.exists());
        assert!(received_file.exists());

        let reloaded = Catalog::open(root.join("catalog.json")).unwrap();
        assert!(reloaded.list().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
