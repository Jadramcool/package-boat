//! 下载并发限制：同一 IP 的并发下载流上限。
//!
//! 浏览器 HTTP/1.1 对同源大约只有 6 条连接；不限并发时，大文件多路 Range
//! 会占满连接池，导致页面 SSE/API 掉线。方案 C：**同端口** 下仍支持 Range
//! 续传，但每 IP 最多 N 路同时下载，超出立即 429，给页面留连接。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 每个来源 IP 允许同时进行的下载流数（含 Range 分片）。
pub const MAX_DOWNLOAD_STREAMS_PER_IP: u32 = 3;

#[derive(Default)]
pub struct DownloadSlots {
    active: Mutex<HashMap<String, u32>>,
}

/// 下载槽位租约：Drop（流结束/断开）时释放。
pub struct DownloadSlot {
    ip: String,
    slots: Arc<DownloadSlots>,
}

impl Drop for DownloadSlot {
    fn drop(&mut self) {
        let mut map = self.slots.active.lock().expect("download slots poisoned");
        if let Some(count) = map.get_mut(&self.ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(&self.ip);
            }
        }
    }
}

impl DownloadSlots {
    pub fn new() -> Arc<Self> {
        Arc::new(DownloadSlots::default())
    }

    /// 尝试占用一个下载槽；超出每 IP 上限时返回 `None`。
    pub fn try_acquire(self: &Arc<Self>, ip: &str) -> Option<DownloadSlot> {
        let mut map = self.active.lock().expect("download slots poisoned");
        let count = map.entry(ip.to_string()).or_insert(0);
        if *count >= MAX_DOWNLOAD_STREAMS_PER_IP {
            return None;
        }
        *count += 1;
        Some(DownloadSlot {
            ip: ip.to_string(),
            slots: Arc::clone(self),
        })
    }

    /// 当前 IP 已占用槽位（测试/诊断用）。
    pub fn active_for(&self, ip: &str) -> u32 {
        self.active
            .lock()
            .expect("download slots poisoned")
            .get(ip)
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_concurrent_slots_per_ip() {
        let slots = DownloadSlots::new();
        let mut held = Vec::new();
        for i in 0..MAX_DOWNLOAD_STREAMS_PER_IP {
            held.push(
                slots
                    .try_acquire("10.0.0.1")
                    .unwrap_or_else(|| panic!("slot {i}")),
            );
        }
        assert!(slots.try_acquire("10.0.0.1").is_none(), "超过上限应失败");
        // 另一 IP 不受影响
        let other = slots.try_acquire("10.0.0.2").expect("other ip");
        drop(held.pop().unwrap());
        assert_eq!(
            slots.active_for("10.0.0.1"),
            MAX_DOWNLOAD_STREAMS_PER_IP - 1
        );
        let extra = slots.try_acquire("10.0.0.1").expect("slot after drop");
        drop(held);
        drop(extra);
        drop(other);
        assert_eq!(slots.active_for("10.0.0.1"), 0);
    }
}
