//! 传输进度跟踪：用于桌面端任务栏进度条。跟踪"当前正在进行的传输"
//! （后发传输覆盖前者），并发传输只反映最近一次。

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// 传输进度快照（桌面端序列化为 JSON）。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct TransferProgress {
    /// 是否有传输正在进行。
    pub active: bool,
    /// 已完成字节数。
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub done: u64,
    /// 总字节数（`None` 表示未知，如 multipart 上传无明确总大小）。
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

/// 线程安全的进度跟踪器。
#[derive(Default)]
pub struct ProgressTracker {
    active: AtomicBool,
    done: AtomicU64,
    total: AtomicU64, // 0 表示未知
}

impl ProgressTracker {
    pub fn new() -> Arc<Self> {
        Arc::new(ProgressTracker::default())
    }

    /// 标记一段传输开始（total 为 0 表示未知）。
    pub fn begin(&self, total: u64) {
        self.done.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.active.store(true, Ordering::Release);
    }

    /// 累加已完成字节。
    pub fn add(&self, n: u64) {
        let _ = self.done.fetch_add(n, Ordering::Relaxed);
    }

    /// 标记传输结束。
    pub fn finish(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn snapshot(&self) -> TransferProgress {
        let active = self.active.load(Ordering::Acquire);
        let total = self.total.load(Ordering::Relaxed);
        TransferProgress {
            active,
            done: self.done.load(Ordering::Relaxed),
            total: if total > 0 { Some(total) } else { None },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_lifecycle() {
        let tracker = ProgressTracker::new();
        assert!(!tracker.snapshot().active);
        assert_eq!(tracker.snapshot().total, None);

        tracker.begin(100);
        let snapshot = tracker.snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.total, Some(100));
        assert_eq!(snapshot.done, 0);

        tracker.add(40);
        assert_eq!(tracker.snapshot().done, 40);
        assert_eq!(tracker.snapshot().ratio(), Some(0.4));

        tracker.finish();
        assert!(!tracker.snapshot().active);
    }

    #[test]
    fn unknown_total_has_no_ratio() {
        let tracker = ProgressTracker::new();
        tracker.begin(0);
        assert_eq!(tracker.snapshot().total, None);
        assert_eq!(tracker.snapshot().ratio(), None);
    }
}
