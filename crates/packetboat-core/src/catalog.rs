//! 共享目录表：`catalog.json`。`linked` 条目只保存源文件绝对路径（不复制、不移动），
//! `received` 条目只由本次服务明确接收的上传文件产生。所有变更先落盘再提交内存状态。
//!
//! **安全边界（当前版本）**：`remove` / `clear` / HTTP DELETE 一律只把条目移出共享清单，
//! 绝不删除磁盘上的原文件或接收目录文件。inbox 扫描条目通过 `hidden_inbox` 路径名单隐藏。
//!
//! `list()` 结果带短 TTL 缓存；文件系统监听通过 `apply_path_event` 只重读命中的条目，
//! 避免每次列表都对全部路径做磁盘 stat。

use crate::CATALOG_VERSION;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

/// 列表缓存有效期：窗口内重复列表不再触发全量 stat。
const LIST_CACHE_TTL: Duration = Duration::from_millis(1_500);

/// 监听父目录数量上限；超出后依赖 TTL 全量刷新兜底。
pub const MAX_WATCH_ROOTS: usize = 256;

/// 条目来源类型，JSON 序列化为小写。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    /// 原位共享：用户明确选择的本机路径，不复制。
    Linked,
    /// 远程接收：本次服务通过上传接口落盘的文件。
    Received,
    /// 接收目录扫描：目录内已有的顶层文件/文件夹（非上传产生）。
    Inbox,
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
    /// 用户从清单移除过的接收目录路径（规范化 key）。只隐藏，不删盘。
    #[serde(default)]
    hidden_inbox: Vec<String>,
}

/// 共享目录表存储。`path` 为空时仅内存操作（测试用）。
pub struct Catalog {
    path: PathBuf,
    inner: RwLock<Vec<Item>>,
    /// `list()` 的短 TTL 缓存；`None` 表示需要重建。
    list_cache: Mutex<Option<(Instant, Vec<Item>)>>,
    /// 接收目录扫描根；`Some` 时列表会合并该目录顶层文件/文件夹（不落盘 catalog）。
    inbox_dir: RwLock<Option<PathBuf>>,
    /// 已从清单移除、但仍在磁盘上的接收目录路径 key。
    hidden_inbox: RwLock<HashSet<String>>,
}

impl Catalog {
    /// 打开目录表；文件不存在时得到空目录表。
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let catalog = Catalog {
            path,
            inner: RwLock::new(Vec::new()),
            list_cache: Mutex::new(None),
            inbox_dir: RwLock::new(None),
            hidden_inbox: RwLock::new(HashSet::new()),
        };
        if !catalog.path.as_os_str().is_empty() {
            catalog.load()?;
        }
        Ok(catalog)
    }

    /// 启用/关闭「展示接收目录既有文件」。关闭时传 `None`。
    /// 不在此处理隐藏名单：恢复已移出项请用独立的 `rescan_inbox`。
    pub fn set_inbox_dir(&self, dir: Option<PathBuf>) {
        *self.inbox_dir.write().expect("inbox lock poisoned") = dir;
        self.invalidate_list_cache();
    }

    /// 重新扫描接收目录：清空该目录下的隐藏名单，把此前「移出清单」的本地文件加回来。
    /// 与开关解耦；未开启扫描时清空全部隐藏名单（开关再开时也能扫到）。
    /// 返回本次取消隐藏的路径条数。
    pub fn rescan_inbox(&self) -> Result<usize, String> {
        let removed = match self.inbox_dir() {
            Some(dir) => self.clear_hidden_under(&dir),
            None => self.clear_all_hidden_inbox(),
        };
        self.invalidate_list_cache();
        Ok(removed)
    }

    /// 清空落在 `receive_dir` 下的 inbox 隐藏路径，返回移除条数。
    fn clear_hidden_under(&self, receive_dir: &Path) -> usize {
        let dir_key = normalize_path_key(&receive_dir.to_string_lossy());
        let prefix = format!("{dir_key}\\");
        let removed = {
            let mut hidden = self
                .hidden_inbox
                .write()
                .expect("hidden inbox lock poisoned");
            let before = hidden.len();
            hidden.retain(|key| !(key == &dir_key || key.starts_with(&prefix)));
            before - hidden.len()
        };
        if removed == 0 {
            return 0;
        }
        self.persist_hidden_snapshot();
        removed
    }

    fn clear_all_hidden_inbox(&self) -> usize {
        let removed = {
            let mut hidden = self
                .hidden_inbox
                .write()
                .expect("hidden inbox lock poisoned");
            let before = hidden.len();
            hidden.clear();
            before
        };
        if removed == 0 {
            return 0;
        }
        self.persist_hidden_snapshot();
        removed
    }

    fn persist_hidden_snapshot(&self) {
        let items = self.inner.read().expect("catalog lock poisoned").clone();
        if let Err(err) = self.save_locked(&items, &self.hidden_snapshot()) {
            eprintln!("保存共享目录隐藏名单失败: {err}");
        }
    }

    fn is_hidden_inbox(&self, path: &str) -> bool {
        let key = normalize_path_key(path);
        self.hidden_inbox
            .read()
            .expect("hidden inbox lock poisoned")
            .contains(&key)
    }

    /// 记住「移出清单」的接收目录路径；保存失败时回滚内存名单。
    fn hide_inbox_path(&self, path: &str) -> Result<(), String> {
        let key = normalize_path_key(path);
        {
            let mut hidden = self
                .hidden_inbox
                .write()
                .expect("hidden inbox lock poisoned");
            if !hidden.insert(key.clone()) {
                return Ok(());
            }
        }
        let items = self.inner.read().expect("catalog lock poisoned").clone();
        if let Err(err) = self.save_locked(&items, &self.hidden_snapshot()) {
            self.hidden_inbox
                .write()
                .expect("hidden inbox lock poisoned")
                .remove(&key);
            return Err(err);
        }
        Ok(())
    }

    fn inbox_dir(&self) -> Option<PathBuf> {
        self.inbox_dir.read().expect("inbox lock poisoned").clone()
    }

    fn invalidate_list_cache(&self) {
        *self.list_cache.lock().expect("list cache poisoned") = None;
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
            if let Err(err) = self.save_locked(&guard, &self.hidden_snapshot()) {
                guard.truncate(start_len);
                return Err(err);
            }
            self.invalidate_list_cache();
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

        let path_key = absolute.to_string_lossy();
        self.hidden_inbox
            .write()
            .expect("hidden inbox lock poisoned")
            .remove(&normalize_path_key(&path_key));

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
        if let Err(err) = self.save_locked(&guard, &self.hidden_snapshot()) {
            guard.pop();
            return Err(err);
        }
        self.invalidate_list_cache();
        Ok(item)
    }

    /// 列出全部条目：优先返回短 TTL 缓存；过期时重建（全量 stat）。
    /// 磁盘元数据读取在锁外进行：慢速盘/大目录的 stat 不再阻塞并发的
    /// 列表查询、下载解析与上传登记。
    pub fn list(&self) -> Vec<Item> {
        {
            let cache = self.list_cache.lock().expect("list cache poisoned");
            if let Some((at, items)) = cache.as_ref() {
                if at.elapsed() < LIST_CACHE_TTL {
                    return items.clone();
                }
            }
        }
        self.rebuild_list_cache()
    }

    fn rebuild_list_cache(&self) -> Vec<Item> {
        let mut result = {
            let guard = self.inner.read().expect("catalog lock poisoned");
            guard.clone()
        };
        for item in result.iter_mut() {
            apply_disk_meta(item);
        }
        if let Some(dir) = self.inbox_dir() {
            let hidden = self
                .hidden_inbox
                .read()
                .expect("hidden inbox lock poisoned")
                .clone();
            append_inbox_items(&mut result, &dir, &hidden);
        }
        result.sort_by_key(|item| std::cmp::Reverse(item.added_at));
        *self.list_cache.lock().expect("list cache poisoned") =
            Some((Instant::now(), result.clone()));
        result
    }

    /// 文件系统事件：只重读路径命中的条目并更新列表缓存，返回是否有变化。
    /// 事件可能是文件本身或其父目录，按路径相等或父目录关系匹配。
    pub fn apply_path_event(&self, changed: &Path) -> bool {
        let changed_str = normalize_path_key(&changed.to_string_lossy());
        let matched = {
            let guard = self.inner.read().expect("catalog lock poisoned");
            guard
                .iter()
                .any(|item| path_event_matches_item(item, &changed_str))
        };
        if !matched {
            return false;
        }

        let mut cache = self.list_cache.lock().expect("list cache poisoned");
        let Some((_, items)) = cache.as_mut() else {
            // 无缓存时下次 list 会全量重建，这里只需让调用方知道应广播。
            return true;
        };
        let mut changed_any = false;
        for item in items.iter_mut() {
            if path_event_matches_item(item, &changed_str) {
                let before = (
                    item.available,
                    item.size,
                    item.modified_at,
                    item.name.clone(),
                );
                apply_disk_meta(item);
                let after = (
                    item.available,
                    item.size,
                    item.modified_at,
                    item.name.clone(),
                );
                if before != after {
                    changed_any = true;
                }
            }
        }
        if changed_any {
            *cache = Some((Instant::now(), items.clone()));
        }
        changed_any
    }

    /// 强制下次 `list` 全量重建（周期性兜底或监听失败时）。
    pub fn mark_list_stale(&self) {
        self.invalidate_list_cache();
    }

    /// 需要非递归监听的根目录：每个条目的父目录；文件夹条目额外监听自身。
    /// 数量受 `MAX_WATCH_ROOTS` 限制。
    pub fn watch_roots(&self) -> Vec<PathBuf> {
        let guard = self.inner.read().expect("catalog lock poisoned");
        let mut roots: Vec<PathBuf> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for item in guard.iter() {
            let path = Path::new(&item.local_path);
            if let Some(parent) = path.parent() {
                let key = normalize_path_key(&parent.to_string_lossy());
                if seen.insert(key) {
                    roots.push(parent.to_path_buf());
                }
            }
            if item.is_dir {
                let key = normalize_path_key(&item.local_path);
                if seen.insert(key) {
                    roots.push(path.to_path_buf());
                }
            }
            if roots.len() >= MAX_WATCH_ROOTS {
                break;
            }
        }
        if let Some(inbox) = self.inbox_dir() {
            let key = normalize_path_key(&inbox.to_string_lossy());
            if seen.insert(key) {
                roots.push(inbox);
            }
        }
        roots
    }

    /// 按 id 解析条目与文件元数据；目标不可用（缺失/符号链接/非普通文件或目录）时返回错误。
    pub fn resolve(&self, id: &str) -> Result<(Item, std::fs::Metadata), std::io::Error> {
        if let Some(dir) = self.inbox_dir() {
            if let Some((item, metadata)) = resolve_inbox_item(id, &dir) {
                if self.is_hidden_inbox(&item.local_path) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "not found",
                    ));
                }
                return Ok((item, metadata));
            }
        }
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
        if let Some(dir) = self.inbox_dir() {
            if let Some((item, _)) = resolve_inbox_item(id, &dir) {
                if self.is_hidden_inbox(&item.local_path) {
                    return None;
                }
                return Some(item);
            }
        }
        let guard = self.inner.read().expect("catalog lock poisoned");
        guard.iter().find(|item| item.id == id).cloned()
    }

    /// 移除条目：只移出共享清单，**绝不删除磁盘文件**。
    /// inbox 扫描条目写入 `hidden_inbox`；接收目录内的 catalog 条目同样隐藏，避免关闭扫描后再次冒出。
    /// 保存失败时回滚内存状态。
    pub fn remove(&self, id: &str) -> Result<Item, String> {
        if inbox_id_prefix_ok(id) {
            if let Some(dir) = self.inbox_dir() {
                let (item, _) =
                    resolve_inbox_item(id, &dir).ok_or_else(|| "条目不存在".to_string())?;
                self.hide_inbox_path(&item.local_path)?;
                self.invalidate_list_cache();
                return Ok(item);
            }
        }
        let mut guard = self.inner.write().expect("catalog lock poisoned");
        let index = guard.iter().position(|item| item.id == id);
        let index = match index {
            Some(index) => index,
            None => return Err("条目不存在".to_string()),
        };
        let removed = guard.remove(index);
        let mut restored_hidden = false;
        if self.should_hide_after_remove(&removed) {
            let key = normalize_path_key(&removed.local_path);
            if self
                .hidden_inbox
                .write()
                .expect("hidden inbox lock poisoned")
                .insert(key)
            {
                restored_hidden = true;
            }
        }
        if let Err(err) = self.save_locked(&guard, &self.hidden_snapshot()) {
            guard.insert(index, removed.clone());
            if restored_hidden {
                self.hidden_inbox
                    .write()
                    .expect("hidden inbox lock poisoned")
                    .remove(&normalize_path_key(&removed.local_path));
            }
            return Err(err);
        }
        self.invalidate_list_cache();
        Ok(removed)
    }

    /// 接收目录内的条目（含远程上传）移出清单后必须进隐藏名单，否则开启目录扫描时会再次出现。
    fn should_hide_after_remove(&self, item: &Item) -> bool {
        if item.source_type == SourceType::Inbox || item.source_type == SourceType::Received {
            return true;
        }
        if let Some(dir) = self.inbox_dir() {
            let dir_key = normalize_path_key(&dir.to_string_lossy());
            let item_key = normalize_path_key(&item.local_path);
            return item_key.starts_with(&format!("{dir_key}\\"));
        }
        false
    }

    /// 清空共享清单：只移除记录/隐藏扫描条目，不删除任何磁盘文件。
    pub fn clear(&self) -> Result<usize, String> {
        let mut guard = self.inner.write().expect("catalog lock poisoned");
        let mut hidden = self
            .hidden_inbox
            .write()
            .expect("hidden inbox lock poisoned");
        let mut hidden_before: Vec<String> = Vec::new();
        let mut cleared = 0usize;

        if !guard.is_empty() || self.inbox_dir().is_some() {
            for item in guard.iter() {
                if self.should_hide_after_remove(item) {
                    let key = normalize_path_key(&item.local_path);
                    if hidden.insert(key.clone()) {
                        hidden_before.push(key);
                    }
                }
            }
            if let Some(dir) = self.inbox_dir() {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if is_inbox_temp_name(&name) {
                            continue;
                        }
                        let Ok(file_type) = entry.file_type() else {
                            continue;
                        };
                        if file_type.is_symlink() || (!file_type.is_file() && !file_type.is_dir()) {
                            continue;
                        }
                        let key = normalize_path_key(&entry.path().to_string_lossy());
                        if hidden.insert(key.clone()) {
                            hidden_before.push(key);
                        }
                    }
                }
            }

            let previous = std::mem::take(&mut *guard);
            cleared = previous.len();
            if let Err(err) = self.save_locked(&[], &hidden) {
                *guard = previous;
                for key in hidden_before {
                    hidden.remove(&key);
                }
                return Err(err);
            }
        }
        drop(hidden);
        drop(guard);
        self.invalidate_list_cache();
        Ok(cleared)
    }

    fn load(&self) -> Result<(), String> {
        let content = match std::fs::read(&self.path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(format!("读取共享目录表: {err}")),
        };
        let stored: PersistedCatalog =
            serde_json::from_slice(&content).map_err(|err| format!("解析共享目录表: {err}"))?;
        let hidden: HashSet<String> = stored.hidden_inbox.into_iter().collect();
        let mut items = match stored.version {
            CATALOG_VERSION => stored.items,
            1 => {
                // v1 会在启动时扫描接收目录，无法区分自动导入文件与真实上传文件。
                // 为避免升级后继续意外暴露整个目录，只迁移原位共享条目；磁盘文件不删除。
                let mut items = stored.items;
                items.retain(|item| item.source_type != SourceType::Received);
                self.save_locked(&items, &hidden)?;
                items
            }
            version => return Err(format!("不支持的共享目录表版本: {version}")),
        };
        *self.inner.write().expect("catalog lock poisoned") = std::mem::take(&mut items);
        *self
            .hidden_inbox
            .write()
            .expect("hidden inbox lock poisoned") = hidden;
        self.invalidate_list_cache();
        Ok(())
    }

    /// 持久化目录表与 inbox 隐藏名单。调用方需自行持有/传入 hidden 快照，避免锁重入。
    fn save_locked(&self, items: &[Item], hidden: &HashSet<String>) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        let mut hidden_vec: Vec<String> = hidden.iter().cloned().collect();
        hidden_vec.sort();
        let persisted = PersistedCatalog {
            version: CATALOG_VERSION,
            items: items.to_vec(),
            hidden_inbox: hidden_vec,
        };
        crate::settings::atomic_write_json(&self.path, &persisted)
    }

    fn hidden_snapshot(&self) -> HashSet<String> {
        self.hidden_inbox
            .read()
            .expect("hidden inbox lock poisoned")
            .clone()
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

/// 用磁盘元数据刷新条目的可用性/名称/大小/修改时间；缺失或非法目标标记不可用。
fn apply_disk_meta(item: &mut Item) {
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
            return;
        }
    }
    item.available = false;
}

fn normalize_path_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

fn path_event_matches_item(item: &Item, changed_key: &str) -> bool {
    let item_path = normalize_path_key(&item.local_path);
    if item_path == changed_key {
        return true;
    }
    if item_path.starts_with(&format!("{changed_key}\\")) {
        return true;
    }
    Path::new(&item.local_path)
        .parent()
        .map(|parent| normalize_path_key(&parent.to_string_lossy()) == changed_key)
        .unwrap_or(false)
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

/// 接收目录扫描条目 id：`inbox:` + 路径 UTF-8 的小写十六进制。
pub fn inbox_item_id(path: &Path) -> String {
    let mut id = String::from("inbox:");
    for byte in path.to_string_lossy().as_bytes() {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

fn inbox_id_prefix_ok(id: &str) -> bool {
    id.starts_with("inbox:") && id.len() > "inbox:".len()
}

fn path_from_inbox_id(id: &str) -> Option<PathBuf> {
    let hex = id.strip_prefix("inbox:")?;
    if hex.is_empty() || hex.len() % 2 != 0 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let chars: Vec<char> = hex.chars().collect();
    for pair in chars.chunks(2) {
        let hi = pair[0].to_digit(16)? as u8;
        let lo = pair[1].to_digit(16)? as u8;
        bytes.push((hi << 4) | lo);
    }
    Some(PathBuf::from(String::from_utf8(bytes).ok()?))
}

fn is_inbox_temp_name(name: &str) -> bool {
    name.starts_with('.') || name.starts_with(".packetboat-") || name.ends_with(".part")
}

fn append_inbox_items(result: &mut Vec<Item>, dir: &Path, hidden: &HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let known: HashSet<String> = result
        .iter()
        .map(|item| normalize_path_key(&item.local_path))
        .collect();
    let now = Utc::now();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_inbox_temp_name(&name) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if !file_type.is_file() && !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let key = normalize_path_key(&path.to_string_lossy());
        if known.contains(&key) || hidden.contains(&key) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        result.push(Item {
            id: inbox_item_id(&path),
            name: name.clone(),
            source_type: SourceType::Inbox,
            local_path: path.to_string_lossy().into_owned(),
            size: metadata.len() as i64,
            modified_at: DateTime::from(
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            ),
            added_at: now,
            available: true,
            is_dir: file_type.is_dir(),
        });
    }
}

fn resolve_inbox_item(id: &str, receive_dir: &Path) -> Option<(Item, std::fs::Metadata)> {
    if !inbox_id_prefix_ok(id) {
        return None;
    }
    let path = path_from_inbox_id(id)?;
    // 仅允许解析接收目录内的直接子项，避免 id 伪造指向任意路径。
    let receive_key = normalize_path_key(&receive_dir.to_string_lossy());
    let path_key = normalize_path_key(&path.to_string_lossy());
    if path_key == receive_key || !path_key.starts_with(&format!("{receive_key}\\")) {
        return None;
    }
    let rest = &path_key[receive_key.len() + 1..];
    if rest.contains('\\') {
        return None;
    }
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() || (!file_type.is_file() && !file_type.is_dir()) {
        return None;
    }
    Some((
        Item {
            id: id.to_string(),
            name: file_name_of(&path).to_string_lossy().into_owned(),
            source_type: SourceType::Inbox,
            local_path: path.to_string_lossy().into_owned(),
            size: metadata.len() as i64,
            modified_at: DateTime::from(
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            ),
            added_at: Utc::now(),
            available: true,
            is_dir: file_type.is_dir(),
        },
        metadata,
    ))
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
        let dir =
            std::env::temp_dir().join(format!("packetboat-catalog-{tag}-{}", std::process::id()));
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

    #[test]
    fn list_cache_avoids_immediate_restat_and_path_events_refresh_single_entry() {
        let root = temp_dir("cache-event");
        let file = root.join("note.txt");
        std::fs::write(&file, b"v1").unwrap();
        let catalog = Catalog::open(PathBuf::new()).unwrap();
        catalog
            .add_linked(&[file.to_string_lossy().into_owned()])
            .unwrap();

        let first = catalog.list();
        assert!(first[0].available);
        assert_eq!(first[0].size, 2);

        // 窗口内改文件：list 仍返回缓存旧大小
        std::fs::write(&file, b"version-two").unwrap();
        let cached = catalog.list();
        assert_eq!(cached[0].size, 2);

        // 文件系统事件只重读命中条目
        assert!(catalog.apply_path_event(&file));
        let updated = catalog.list();
        assert_eq!(updated[0].size, 11);
        assert!(updated[0].available);

        // 删除后事件标记不可用
        std::fs::remove_file(&file).unwrap();
        assert!(catalog.apply_path_event(&file));
        assert!(!catalog.list()[0].available);

        // 无关路径不触发
        assert!(!catalog.apply_path_event(&root.join("other.txt")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn watch_roots_are_parents_and_shared_folders() {
        let root = temp_dir("watch-roots");
        let folder = root.join("shared");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("a.txt");
        std::fs::write(&file, b"x").unwrap();

        let catalog = Catalog::open(PathBuf::new()).unwrap();
        catalog
            .add_linked(&[
                folder.to_string_lossy().into_owned(),
                file.to_string_lossy().into_owned(),
            ])
            .unwrap();

        let roots = catalog.watch_roots();
        assert!(roots.contains(&root));
        assert!(roots.contains(&folder));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn inbox_lists_receive_dir_files_and_skips_catalog_paths() {
        let root = temp_dir("inbox");
        let plain = root.join("manual.txt");
        let uploaded = root.join("uploaded.txt");
        std::fs::write(&plain, b"hello").unwrap();
        std::fs::write(&uploaded, b"up").unwrap();
        std::fs::write(root.join(".hidden"), b"x").unwrap();
        std::fs::write(root.join(".packetboat-1-a.part"), b"tmp").unwrap();

        let catalog = Catalog::open(PathBuf::new()).unwrap();
        catalog.add_received(uploaded.clone()).unwrap();
        catalog.set_inbox_dir(Some(root.clone()));

        let items = catalog.list();
        let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
        assert!(names.contains(&"manual.txt"));
        assert!(names.contains(&"uploaded.txt"));
        assert!(!names.contains(&".hidden"));
        assert!(!names.contains(&".packetboat-1-a.part"));

        let manual = items.iter().find(|item| item.name == "manual.txt").unwrap();
        assert!(manual.id.starts_with("inbox:"));
        assert_eq!(manual.source_type, SourceType::Inbox);
        let resolved = catalog.resolve(&manual.id).unwrap();
        assert_eq!(
            resolved.0.local_path,
            plain.to_string_lossy().replace('/', "\\")
        );

        catalog.set_inbox_dir(None);
        assert!(catalog
            .list()
            .iter()
            .all(|item| !item.id.starts_with("inbox:")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn inbox_remove_hides_item_but_keeps_disk_file() {
        let root = temp_dir("inbox-remove");
        let file = root.join("share-me.txt");
        std::fs::write(&file, b"data").unwrap();
        let catalog = Catalog::open(PathBuf::new()).unwrap();
        catalog.set_inbox_dir(Some(root.clone()));
        let item = catalog
            .list()
            .into_iter()
            .find(|item| item.name == "share-me.txt")
            .unwrap();
        catalog.remove(&item.id).unwrap();
        assert!(file.exists(), "移出清单不得删除磁盘文件");
        assert!(
            catalog
                .list()
                .iter()
                .all(|item| item.name != "share-me.txt"),
            "移出后不应再出现在清单"
        );
        assert!(catalog.get(&item.id).is_none());
        // 独立「重新扫描」：清空隐藏名单，本地文件回到清单
        let restored = catalog.rescan_inbox().unwrap();
        assert!(restored >= 1);
        assert!(
            catalog
                .list()
                .iter()
                .any(|item| item.name == "share-me.txt"),
            "重新扫描接收目录后应恢复展示"
        );
        catalog.clear().unwrap();
        assert!(file.exists(), "清空清单也不得删除磁盘文件");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn received_remove_keeps_disk_file_and_hides_rescan() {
        let root = temp_dir("received-remove");
        let file = root.join("upload.bin");
        std::fs::write(&file, b"payload").unwrap();
        let catalog = Catalog::open(PathBuf::new()).unwrap();
        let item = catalog.add_received(file.clone()).unwrap();
        catalog.set_inbox_dir(Some(root.clone()));
        catalog.remove(&item.id).unwrap();
        assert!(file.exists(), "接收文件移出清单后仍须保留");
        assert!(catalog
            .list()
            .iter()
            .all(|entry| entry.local_path != item.local_path));
        // 独立重新扫描后，接收目录内文件以「本地文件」身份回到清单
        catalog.rescan_inbox().unwrap();
        assert!(catalog
            .list()
            .iter()
            .any(|entry| entry.local_path == item.local_path));
        let _ = std::fs::remove_dir_all(&root);
    }
}
