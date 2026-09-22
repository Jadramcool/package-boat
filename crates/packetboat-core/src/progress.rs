//! 传输进度跟踪：任务栏总进度 + 桌面端「传输中」实时列表。
//!
//! 每个上传/下载任务有独立 ID；并发任务互不覆盖。速度用约 1 秒滑动窗口估算。

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::time::{Duration, Instant};

/// 传输方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TransferKind {
    /// 远程设备上传到本机。
    Upload,
    /// 远程设备从本机下载。
    Download,
}

/// 传输进度快照（桌面端序列化为 JSON，任务栏进度条）。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct TransferProgress {
    /// 是否有传输正在进行。
    pub active: bool,
    /// 已完成字节数（所有活跃任务之和）。
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub done: u64,
    /// 总字节数（任一任务未知时为 `None`）。
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub total: Option<u64>,
}

impl TransferProgress {
    pub fn ratio(&self) -> Option<f64> {
        match self.total {
            Some(total) if total > 0 => Some((self.done as f64 / total as f64).clamp(0.0, 1.0)),
            _ => None,
        }
    }
}

/// 桌面端「传输中」一条任务。
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct ActiveTransfer {
    pub id: String,
    pub name: String,
    pub kind: TransferKind,
    /// 来源设备 IP（未知时为空字符串）。
    pub peer_ip: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub done: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub total: Option<u64>,
    /// 估算速度（字节/秒）。
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub speed_bps: u64,
    /// 进度百分比 0–100；总大小未知时为 `None`。
    pub progress: Option<u8>,
    /// 是否处于暂停（流挂起，未取消）。
    pub paused: bool,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

struct TaskState {
    name: String,
    kind: TransferKind,
    peer_ip: String,
    done: u64,
    total: Option<u64>,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    window_bytes: u64,
    window_start: Instant,
    speed_bps: u64,
    /// 并发流计数（浏览器 Range 多连接下载时同一文件只显示一条）。
    active_streams: u32,
}

/// 用户取消后的冷却时间：窗口内同 ID 下载会被拒绝，避免浏览器重试把任务“复活”。
const CANCEL_COOLDOWN: Duration = Duration::from_secs(20);

/// 线程安全的多任务传输注册表。
#[derive(Default)]
pub struct TransferRegistry {
    tasks: Mutex<HashMap<String, TaskState>>,
    /// 用户主动取消的任务 id → 取消时间（冷却窗口）。
    cancelled: Mutex<HashMap<String, Instant>>,
    /// 暂停中的任务 id → 等待中的 Waker（继续时唤醒）。
    paused: Mutex<HashMap<String, Option<Waker>>>,
    /// 本传输会话是否已计过下载次数（同一 id 并发流/Drop 只计一次）。
    counted: Mutex<HashSet<String>>,
    /// 任务栏进度用的序号，保证 snapshot 在无任务时也可读。
    seq: AtomicU64,
}

/// 简单单槽跟踪器别名：旧调用点（测试 / 任务栏）仍可用。
pub type ProgressTracker = TransferRegistry;

const SPEED_WINDOW: Duration = Duration::from_millis(900);
/// 无更新超过该时长的未完成任务在 list 时剔除（客户端异常断开兜底）。
const _STALE_AFTER: Duration = Duration::from_secs(60);

impl TransferRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(TransferRegistry::default())
    }

    /// 兼容旧 API：匿名单槽传输（上传 multipart / 测试）。
    pub fn begin(&self, total: u64) {
        let _ = self.begin_task(
            "__default__",
            "传输中",
            TransferKind::Upload,
            "",
            if total > 0 { Some(total) } else { None },
        );
    }

    /// 登记一个传输任务。同一 id 再次调用会复用条目并增加并发流计数
    /// （浏览器 Range 多连接下载同一文件时只展示一条）。
    /// 返回 `true` 表示新建了任务（首次开始传输）。
    pub fn begin_task(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
        kind: TransferKind,
        peer_ip: impl Into<String>,
        total: Option<u64>,
    ) -> bool {
        let id = id.into();
        let name = name.into();
        let peer_ip = peer_ip.into();
        let now = Utc::now();
        let mut tasks = self.tasks.lock().expect("transfer registry poisoned");
        let total = total.filter(|value| *value > 0);
        if let Some(task) = tasks.get_mut(&id) {
            task.active_streams = task.active_streams.saturating_add(1);
            task.updated_at = now;
            // 总大小取已知较大值（全量 size 优先于单次 Range 的 content-length）
            if total.unwrap_or(0) > task.total.unwrap_or(0) {
                task.total = total;
            }
            if !peer_ip.is_empty() && task.peer_ip.is_empty() {
                task.peer_ip = peer_ip;
            }
            return false;
        }
        // 冷却期内不重建任务：停止后浏览器 Range 重试不应让条目复活
        if self.recently_cancelled(&id) {
            return false;
        }
        // 新的传输会话：允许再次计一次下载次数
        self.counted
            .lock()
            .expect("transfer registry poisoned")
            .remove(&id);
        tasks.insert(
            id.clone(),
            TaskState {
                name,
                kind,
                peer_ip,
                done: 0,
                total,
                started_at: now,
                updated_at: now,
                window_bytes: 0,
                window_start: Instant::now(),
                speed_bps: 0,
                active_streams: 1,
            },
        );
        self.seq.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// 兼容旧 API：向默认槽累加字节。
    pub fn add(&self, n: u64) {
        self.add_to("__default__", n);
    }

    /// 向指定任务累加已传字节，并更新速度估算。
    pub fn add_to(&self, id: &str, n: u64) {
        if n == 0 {
            return;
        }
        let mut tasks = self.tasks.lock().expect("transfer registry poisoned");
        // 默认槽不存在时自动创建，避免旧路径丢进度
        if !tasks.contains_key(id) {
            let now = Utc::now();
            tasks.insert(
                id.to_string(),
                TaskState {
                    name: "传输中".to_string(),
                    kind: TransferKind::Upload,
                    peer_ip: String::new(),
                    done: 0,
                    total: None,
                    started_at: now,
                    updated_at: now,
                    window_bytes: 0,
                    window_start: Instant::now(),
                    speed_bps: 0,
                    active_streams: 1,
                },
            );
        }
        if let Some(task) = tasks.get_mut(id) {
            task.done = task.done.saturating_add(n);
            task.updated_at = Utc::now();
            task.window_bytes = task.window_bytes.saturating_add(n);
            let elapsed = task.window_start.elapsed();
            if elapsed >= SPEED_WINDOW {
                let micros = elapsed.as_micros().max(1) as u64;
                task.speed_bps = task.window_bytes.saturating_mul(1_000_000) / micros;
                task.window_bytes = 0;
                task.window_start = Instant::now();
            }
        }
    }

    /// 兼容旧 API：结束默认槽。
    pub fn finish(&self) {
        self.finish_task("__default__");
    }

    /// 结束并移除指定任务（不考虑并发流）。
    pub fn finish_task(&self, id: &str) {
        self.tasks
            .lock()
            .expect("transfer registry poisoned")
            .remove(id);
    }

    /// 单条连接/流结束：递减并发计数。
    /// 最后一条流结束且任务被移除时，返回 `Some(should_count_download)`：
    /// 仅当已传字节达到目标且本会话尚未计过次数时为 `true`。
    pub fn finish_stream(&self, id: &str) -> Option<bool> {
        let mut tasks = self.tasks.lock().expect("transfer registry poisoned");
        let task = tasks.get_mut(id)?;
        task.active_streams = task.active_streams.saturating_sub(1);
        if task.active_streams > 0 {
            return None;
        }
        let done = task.done;
        let total = task.total;
        tasks.remove(id);
        drop(tasks);
        if self.recently_cancelled(id) {
            return None;
        }
        // 完整下载：已知总大小时要求 done >= total；ZIP 等未知总长时以流自然结束且有数据为准
        let complete = match total {
            Some(total) if total > 0 => done >= total,
            _ => done > 0,
        };
        if !complete {
            return Some(false);
        }
        // 同一传输会话只计一次（并发 Range / poll+Drop 重复调用）
        let first = self
            .counted
            .lock()
            .expect("transfer registry poisoned")
            .insert(id.to_string());
        Some(first)
    }

    /// 主动取消传输：标记取消并立刻从「传输中」列表移除。
    /// 下载/上传流在下一次 poll 时检测到取消并结束。
    pub fn cancel(&self, id: &str) -> bool {
        let existed = {
            let mut tasks = self.tasks.lock().expect("transfer registry poisoned");
            tasks.remove(id).is_some()
        };
        self.paused
            .lock()
            .expect("transfer registry poisoned")
            .remove(id);
        self.cancelled
            .lock()
            .expect("transfer registry poisoned")
            .insert(id.to_string(), Instant::now());
        existed
    }

    /// 流是否已被用户取消（冷却窗口内仍算取消）。
    pub fn is_cancelled(&self, id: &str) -> bool {
        self.recently_cancelled(id)
    }

    /// 取消冷却是否仍生效。
    pub fn recently_cancelled(&self, id: &str) -> bool {
        let mut cancelled = self.cancelled.lock().expect("transfer registry poisoned");
        match cancelled.get(id) {
            Some(at) if at.elapsed() < CANCEL_COOLDOWN => true,
            Some(_) => {
                cancelled.remove(id);
                false
            }
            None => false,
        }
    }

    /// 暂停传输：流在下一次 poll 时挂起（TCP 连接可保持，客户端表现为速度为 0）。
    pub fn pause(&self, id: &str) -> bool {
        let exists = self
            .tasks
            .lock()
            .expect("transfer registry poisoned")
            .contains_key(id);
        if !exists {
            return false;
        }
        self.paused
            .lock()
            .expect("transfer registry poisoned")
            .entry(id.to_string())
            .or_insert(None);
        true
    }

    /// 继续传输：清除暂停标记并唤醒挂起的流。
    pub fn resume(&self, id: &str) -> bool {
        let removed = self
            .paused
            .lock()
            .expect("transfer registry poisoned")
            .remove(id);
        match removed {
            Some(slot) => {
                if let Some(waker) = slot {
                    waker.wake();
                }
                true
            }
            None => false,
        }
    }

    /// 任务是否暂停中。
    pub fn is_paused(&self, id: &str) -> bool {
        self.paused
            .lock()
            .expect("transfer registry poisoned")
            .contains_key(id)
    }

    /// 暂停时登记 Waker，便于 resume 唤醒。
    pub fn store_pause_waker(&self, id: &str, waker: Waker) {
        let mut paused = self.paused.lock().expect("transfer registry poisoned");
        if let Some(slot) = paused.get_mut(id) {
            *slot = Some(waker);
        }
    }

    /// 清除取消标记（同 id 新传输开始时调用）。
    pub fn clear_cancel(&self, id: &str) {
        self.cancelled
            .lock()
            .expect("transfer registry poisoned")
            .remove(id);
        let waker = self
            .paused
            .lock()
            .expect("transfer registry poisoned")
            .remove(id);
        if let Some(Some(waker)) = waker {
            waker.wake();
        }
    }

    /// 当前活跃传输列表（按最近更新降序）。
    pub fn list(&self) -> Vec<ActiveTransfer> {
        let now = Utc::now();
        let stale = chrono::Duration::seconds(60);
        let mut tasks = self.tasks.lock().expect("transfer registry poisoned");
        tasks.retain(|_, task| now - task.updated_at <= stale);
        let paused_ids: HashSet<String> = self
            .paused
            .lock()
            .expect("transfer registry poisoned")
            .keys()
            .cloned()
            .collect();
        let mut items: Vec<ActiveTransfer> = tasks
            .iter()
            .map(|(id, task)| {
                let progress = task
                    .total
                    .filter(|total| *total > 0)
                    .map(|total| ((task.done.saturating_mul(100)) / total).min(100) as u8);
                let paused = paused_ids.contains(id);
                ActiveTransfer {
                    id: id.clone(),
                    name: task.name.clone(),
                    kind: task.kind,
                    peer_ip: task.peer_ip.clone(),
                    done: task.done,
                    total: task.total,
                    speed_bps: if paused { 0 } else { task.speed_bps },
                    progress,
                    paused,
                    started_at: task.started_at,
                    updated_at: task.updated_at,
                }
            })
            .collect();
        items.sort_by_key(|item| std::cmp::Reverse(item.updated_at));
        items
    }

    /// 任务栏进度：汇总所有活跃任务。
    pub fn snapshot(&self) -> TransferProgress {
        let tasks = self.tasks.lock().expect("transfer registry poisoned");
        if tasks.is_empty() {
            return TransferProgress {
                active: false,
                done: 0,
                total: None,
            };
        }
        let mut done = 0u64;
        let mut total = Some(0u64);
        for task in tasks.values() {
            done = done.saturating_add(task.done);
            match (total, task.total) {
                (Some(sum), Some(value)) => total = Some(sum.saturating_add(value)),
                _ => total = None,
            }
        }
        TransferProgress {
            active: true,
            done,
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_lifecycle() {
        let tracker = TransferRegistry::new();
        assert!(!tracker.snapshot().active);
        assert_eq!(tracker.snapshot().total, None);

        tracker.begin(100);
        let snapshot = tracker.snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.total, Some(100));
        assert_eq!(snapshot.done, 0);

        tracker.add(40);
        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.done, 40);
        assert_eq!(snapshot.ratio(), Some(0.4));

        tracker.finish();
        assert!(!tracker.snapshot().active);
    }

    #[test]
    fn unknown_total_has_no_ratio() {
        let tracker = TransferRegistry::new();
        tracker.begin(0);
        let snapshot = tracker.snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.total, None);
        assert_eq!(snapshot.ratio(), None);
    }

    #[test]
    fn concurrent_tasks_do_not_overwrite() {
        let registry = TransferRegistry::new();
        registry.begin_task(
            "dl-1",
            "movie.mp4",
            TransferKind::Download,
            "10.0.0.2",
            Some(1000),
        );
        registry.begin_task(
            "up-1",
            "photo.jpg",
            TransferKind::Upload,
            "10.0.0.3",
            Some(500),
        );
        registry.add_to("dl-1", 200);
        registry.add_to("up-1", 100);

        let list = registry.list();
        assert_eq!(list.len(), 2);
        let dl = list.iter().find(|item| item.id == "dl-1").unwrap();
        assert_eq!(dl.kind, TransferKind::Download);
        assert_eq!(dl.done, 200);
        assert_eq!(dl.total, Some(1000));
        assert_eq!(dl.name, "movie.mp4");
        assert_eq!(dl.peer_ip, "10.0.0.2");

        let snap = registry.snapshot();
        assert_eq!(snap.done, 300);
        assert_eq!(snap.total, Some(1500));

        registry.finish_task("dl-1");
        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "up-1");
    }

    #[test]
    fn progress_percent_is_computed() {
        let registry = TransferRegistry::new();
        registry.begin_task("a", "a.bin", TransferKind::Download, "", Some(200));
        registry.add_to("a", 50);
        let item = &registry.list()[0];
        assert_eq!(item.progress, Some(25));
    }

    #[test]
    fn cancel_marks_and_removes_transfer() {
        let registry = TransferRegistry::new();
        registry.begin_task(
            "dl:1",
            "a.bin",
            TransferKind::Download,
            "1.1.1.1",
            Some(100),
        );
        registry.add_to("dl:1", 10);
        assert_eq!(registry.list().len(), 1);
        assert!(registry.cancel("dl:1"));
        assert!(registry.list().is_empty());
        assert!(registry.is_cancelled("dl:1"));
        // 冷却期内 begin_task 不应复活任务
        assert!(!registry.begin_task(
            "dl:1",
            "a.bin",
            TransferKind::Download,
            "1.1.1.1",
            Some(100)
        ));
        assert!(registry.list().is_empty());
        // 清除取消标记后可重新开始
        registry.clear_cancel("dl:1");
        assert!(registry.begin_task(
            "dl:1",
            "a.bin",
            TransferKind::Download,
            "1.1.1.1",
            Some(100)
        ));
        assert!(!registry.is_cancelled("dl:1"));
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn concurrent_ranges_share_one_download_entry() {
        let registry = TransferRegistry::new();
        // 浏览器 4 个 Range 连接下载同一文件
        registry.begin_task(
            "dl:file1",
            "movie.mp4",
            TransferKind::Download,
            "10.0.0.2",
            Some(1000),
        );
        registry.begin_task(
            "dl:file1",
            "movie.mp4",
            TransferKind::Download,
            "10.0.0.2",
            Some(250),
        );
        registry.begin_task(
            "dl:file1",
            "movie.mp4",
            TransferKind::Download,
            "10.0.0.2",
            Some(250),
        );
        registry.begin_task(
            "dl:file1",
            "movie.mp4",
            TransferKind::Download,
            "10.0.0.2",
            Some(250),
        );
        registry.add_to("dl:file1", 250);
        registry.add_to("dl:file1", 250);
        registry.add_to("dl:file1", 250);
        registry.add_to("dl:file1", 250);

        let list = registry.list();
        assert_eq!(list.len(), 1, "同一文件的并发 Range 应合并为一条");
        assert_eq!(list[0].done, 1000);
        assert_eq!(list[0].total, Some(1000), "总大小取完整文件");
        assert_eq!(list[0].progress, Some(100));

        // 部分流结束不移除任务
        assert_eq!(registry.finish_stream("dl:file1"), None);
        assert_eq!(registry.finish_stream("dl:file1"), None);
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.finish_stream("dl:file1"), None);
        assert_eq!(
            registry.finish_stream("dl:file1"),
            Some(true),
            "全部分片完成后应算完整下载"
        );
        assert!(registry.list().is_empty());
    }

    #[test]
    fn incomplete_stream_is_not_complete() {
        let registry = TransferRegistry::new();
        registry.begin_task(
            "dl:x",
            "big.bin",
            TransferKind::Download,
            "1.1.1.1",
            Some(1000),
        );
        registry.add_to("dl:x", 200);
        assert_eq!(registry.finish_stream("dl:x"), Some(false));
    }

    #[test]
    fn complete_download_counts_only_once() {
        let registry = TransferRegistry::new();
        registry.begin_task(
            "dl:z",
            "a.bin",
            TransferKind::Download,
            "1.1.1.1",
            Some(100),
        );
        registry.add_to("dl:z", 100);
        assert_eq!(registry.finish_stream("dl:z"), Some(true), "首次完整应计数");
        // Drop 或重复 finish 不应再计
        assert_eq!(registry.finish_stream("dl:z"), None);
        // 新的传输会话可以再计一次
        registry.begin_task(
            "dl:z",
            "a.bin",
            TransferKind::Download,
            "1.1.1.1",
            Some(100),
        );
        registry.add_to("dl:z", 100);
        assert_eq!(registry.finish_stream("dl:z"), Some(true));
    }
}
