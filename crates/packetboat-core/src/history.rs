//! 主机侧传输历史：完成/取消/中断的落盘元数据，按保留天数自动清理。
//!
//! 与 `TransferRegistry` 解耦：注册表只负责进行中任务；结束后由注册表回调写入本存储。
//! 桌面端与浏览器接收页共用同一份历史（`GET /api/history`）。

use crate::progress::TransferKind;
use crate::settings::atomic_write_json;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 历史条目硬上限。保留天数之外再限制条数，避免高频传输把 `history.json`
/// 和前端轮询响应无界放大。
pub const MAX_HISTORY_ENTRIES: usize = 500;

/// 历史结果状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    /// 完整完成。
    Complete,
    /// 用户主动取消。
    Cancelled,
    /// 未完整结束（中断 / 异常断开）。
    Incomplete,
}

/// 单条传输历史记录（新到旧展示时由 `list` 排序）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct HistoryEntry {
    /// 单次传输事件 ID。并发 Range 共用一个 ID，但同一文件的下一次下载必须生成新 ID。
    pub id: String,
    pub name: String,
    pub kind: TransferKind,
    /// 结束时实际已传/已收字节数。旧版本字段 `size` 通过 serde alias 兼容读取。
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub transferred_bytes: u64,
    /// 任务声明/探测到的总字节数；未知时为 `None`。
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub total_bytes: Option<u64>,
    pub status: HistoryStatus,
    /// 对端 IP（未知为空串）。
    pub peer_ip: String,
    /// 失败/中断原因（可选）。
    pub error: Option<String>,
    pub finished_at: DateTime<Utc>,
}

/// 落盘结构：仅条目数组。
#[derive(Debug, Default, Serialize)]
struct HistoryFile {
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

/// 读取旧版本文件时允许 `size` 字段和缺失字段。
#[derive(Debug, Default, Deserialize)]
struct HistoryFileRead {
    #[serde(default)]
    entries: Vec<PersistedHistoryEntry>,
}

#[derive(Debug, Deserialize)]
struct PersistedHistoryEntry {
    id: String,
    name: String,
    kind: TransferKind,
    #[serde(default, alias = "size")]
    transferred_bytes: u64,
    #[serde(default)]
    total_bytes: Option<u64>,
    status: HistoryStatus,
    #[serde(default)]
    peer_ip: String,
    #[serde(default)]
    error: Option<String>,
    finished_at: DateTime<Utc>,
}

impl From<PersistedHistoryEntry> for HistoryEntry {
    fn from(value: PersistedHistoryEntry) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            transferred_bytes: value.transferred_bytes,
            total_bytes: value.total_bytes,
            status: value.status,
            peer_ip: value.peer_ip,
            error: value.error,
            finished_at: value.finished_at,
        }
    }
}

/// 线程安全的传输历史存储。
pub struct TransferHistory {
    path: PathBuf,
    entries: Mutex<Vec<HistoryEntry>>,
    /// 保留天数；`0` 表示关闭历史，不写入且清空已有数据。
    retention_days: Mutex<u32>,
}

impl TransferHistory {
    /// 打开历史文件；文件不存在或损坏时从空列表开始。
    /// `path` 为空字符串时仅内存工作（测试）。
    pub fn open(path: PathBuf, retention_days: u32) -> Arc<Self> {
        let entries = if path.as_os_str().is_empty() {
            Vec::new()
        } else {
            match std::fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str::<HistoryFileRead>(&content)
                    .map(|file| file.entries.into_iter().map(HistoryEntry::from).collect())
                    .unwrap_or_default(),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(_) => Vec::new(),
            }
        };
        let history = Arc::new(TransferHistory {
            path,
            entries: Mutex::new(entries),
            retention_days: Mutex::new(retention_days),
        });
        // 启动时按当前保留天数修剪一次
        let _ = history.prune_and_persist();
        history
    }

    /// 更新保留天数；设为 `0` 时清空已有历史。
    pub fn set_retention_days(&self, days: u32) -> Result<(), String> {
        let days = days.min(365);
        let previous_days = self.retention_days();
        let previous_entries = self.entries.lock().expect("history poisoned").clone();
        let mut guard = self
            .retention_days
            .lock()
            .expect("history retention poisoned");
        *guard = days;
        drop(guard);
        let result = if days == 0 {
            self.clear().map(|_| ())
        } else {
            self.prune_and_persist()
        };
        if let Err(err) = result {
            *self
                .retention_days
                .lock()
                .expect("history retention poisoned") = previous_days;
            *self.entries.lock().expect("history poisoned") = previous_entries;
            return Err(err);
        }
        Ok(())
    }

    /// 当前保留天数。
    pub fn retention_days(&self) -> u32 {
        *self
            .retention_days
            .lock()
            .expect("history retention poisoned")
    }

    /// 记录一条结束传输；关闭（`retention_days == 0`）时忽略。
    /// 同 id 覆盖并移到最前（最新结果优先）。
    /// `finished_at` 由调用方设置（`entry_from_task` 用当前时间；测试可注入历史时间）。
    pub fn record(&self, entry: HistoryEntry) {
        if self.retention_days() == 0 {
            return;
        }
        {
            let mut entries = self.entries.lock().expect("history poisoned");
            entries.retain(|item| item.id != entry.id);
            entries.insert(0, entry);
            prune_entries(&mut entries, self.retention_days(), Utc::now());
        }
        // 落盘失败不阻塞传输路径；内存列表仍可用。
        let _ = self.persist();
    }

    /// 当前历史（新到旧），已按保留天数修剪。
    pub fn list(&self) -> Vec<HistoryEntry> {
        let _ = self.prune_and_persist();
        self.entries.lock().expect("history poisoned").clone()
    }

    /// 清空历史，返回删除条数。
    pub fn clear(&self) -> Result<u32, String> {
        let removed = {
            let mut entries = self.entries.lock().expect("history poisoned");
            let count = entries.len() as u32;
            entries.clear();
            count
        };
        self.persist()?;
        Ok(removed)
    }

    fn prune_and_persist(&self) -> Result<(), String> {
        let days = self.retention_days();
        let changed = {
            let mut entries = self.entries.lock().expect("history poisoned");
            prune_entries(&mut entries, days, Utc::now())
        };
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        let snapshot = {
            let entries = self.entries.lock().expect("history poisoned");
            HistoryFile {
                entries: entries.clone(),
            }
        };
        atomic_write_json(&self.path, &snapshot)
    }
}

/// 从注册表任务信息构造历史条目的辅助（供 progress 调用）。
pub fn entry_from_task(
    id: &str,
    name: &str,
    kind: TransferKind,
    peer_ip: &str,
    transferred_bytes: u64,
    total_bytes: Option<u64>,
    status: HistoryStatus,
) -> HistoryEntry {
    HistoryEntry {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        transferred_bytes,
        total_bytes,
        status,
        peer_ip: peer_ip.to_string(),
        error: None,
        finished_at: Utc::now(),
    }
}

/// 在锁内裁剪：按结束时间排序、按 ID 去重、应用天数与条数上限。
/// 返回条目是否发生变化。
fn prune_entries(entries: &mut Vec<HistoryEntry>, days: u32, now: DateTime<Utc>) -> bool {
    let before = entries.clone();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.finished_at));
    if days == 0 {
        entries.clear();
    } else {
        let cutoff = now - Duration::days(i64::from(days));
        let mut seen = std::collections::HashSet::new();
        entries.retain(|entry| entry.finished_at >= cutoff && seen.insert(entry.id.clone()));
        entries.truncate(MAX_HISTORY_ENTRIES);
    }
    *entries != before
}

/// 空路径便捷构造（单测）。
pub fn open_in_memory(retention_days: u32) -> Arc<TransferHistory> {
    TransferHistory::open(PathBuf::new(), retention_days)
}

/// 确保父目录存在后打开（桌面宿主用）。
pub fn open_at(path: &Path, retention_days: u32) -> Arc<TransferHistory> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    TransferHistory::open(path.to_path_buf(), retention_days)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, days_ago: i64) -> HistoryEntry {
        HistoryEntry {
            id: id.to_string(),
            name: format!("{id}.bin"),
            kind: TransferKind::Upload,
            transferred_bytes: 10,
            total_bytes: Some(10),
            status: HistoryStatus::Complete,
            peer_ip: "10.0.0.2".into(),
            error: None,
            finished_at: Utc::now() - Duration::days(days_ago),
        }
    }

    #[test]
    fn record_upserts_by_id_and_lists_newest_first() {
        let history = open_in_memory(7);
        history.record(sample("a", 0));
        history.record(sample("b", 0));
        let mut updated = sample("a", 0);
        updated.status = HistoryStatus::Cancelled;
        history.record(updated);

        let list = history.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "a");
        assert_eq!(list[0].status, HistoryStatus::Cancelled);
        assert_eq!(list[1].id, "b");
    }

    #[test]
    fn retention_prunes_old_entries() {
        let history = open_in_memory(7);
        history.record(sample("old", 10));
        history.record(sample("fresh", 1));
        let list = history.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "fresh");
    }

    #[test]
    fn zero_retention_disables_recording() {
        let history = open_in_memory(0);
        history.record(sample("a", 0));
        assert!(history.list().is_empty());
    }

    #[test]
    fn clear_returns_removed_count() {
        let history = open_in_memory(7);
        history.record(sample("a", 0));
        history.record(sample("b", 0));
        assert_eq!(history.clear().unwrap(), 2);
        assert!(history.list().is_empty());
    }

    #[test]
    fn distinct_transfer_sessions_keep_distinct_entries() {
        let history = open_in_memory(7);
        history.record(sample("first-session", 0));
        history.record(sample("second-session", 0));
        assert_eq!(history.list().len(), 2);
    }

    #[test]
    fn entry_limit_is_enforced() {
        let history = open_in_memory(365);
        for index in 0..(MAX_HISTORY_ENTRIES + 10) {
            history.record(sample(&format!("session-{index}"), 0));
        }
        let list = history.list();
        assert_eq!(list.len(), MAX_HISTORY_ENTRIES);
        assert_eq!(list[0].id, format!("session-{}", MAX_HISTORY_ENTRIES + 9));
    }

    #[test]
    fn legacy_size_field_is_read_as_transferred_bytes() {
        let dir = std::env::temp_dir().join(format!(
            "packetboat-history-legacy-{}-{}",
            std::process::id(),
            "size"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.json");
        std::fs::write(
            &path,
            r#"{"entries":[{"id":"legacy","name":"old.bin","kind":"upload","size":42,"status":"complete","peer_ip":"","error":null,"finished_at":"2026-11-25T00:00:00Z"}]}"#,
        )
        .unwrap();
        let history = open_at(&path, 365);
        let entry = history.list().into_iter().next().unwrap();
        assert_eq!(entry.transferred_bytes, 42);
        assert_eq!(entry.total_bytes, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_retention_zero_clears() {
        let history = open_in_memory(7);
        history.record(sample("a", 0));
        history.set_retention_days(0).unwrap();
        assert!(history.list().is_empty());
        assert_eq!(history.retention_days(), 0);
    }

    #[test]
    fn persists_and_reloads() {
        let dir = std::env::temp_dir().join(format!(
            "packetboat-history-{}-{}",
            std::process::id(),
            "reload"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.json");

        {
            let history = open_at(&path, 7);
            history.record(sample("keep", 0));
            assert_eq!(history.list().len(), 1, "内存中应有 1 条");
            assert!(path.exists(), "历史文件应已落盘: {path:?}");
        }
        let reloaded = open_at(&path, 7);
        let list = reloaded.list();
        assert_eq!(list.len(), 1, "重载后应有 1 条，文件内容可能无效");
        assert_eq!(list[0].id, "keep");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
