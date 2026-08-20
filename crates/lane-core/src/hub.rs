//! SSE 事件中枢：为每个订阅者分配带缓冲的通道，发布时非阻塞投递（满则丢弃），
//! 与 Go 版 `eventHub` 语义一致。订阅者以自增 id 区分，退订闭包不借用 Hub，
//! 可安全放入 'static 的 SSE 流中。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

const CHANNEL_CAPACITY: usize = 8;

#[derive(Default)]
struct Inner {
    clients: HashMap<u64, mpsc::Sender<String>>,
}

/// 事件中枢。
#[derive(Default)]
pub struct Hub {
    inner: Arc<Mutex<Inner>>,
    counter: AtomicU64,
}

impl Hub {
    pub fn new() -> Self {
        Hub::default()
    }

    /// 订阅事件，返回接收端与取消订阅函数（不借用 Hub，可跨任务使用）。
    pub fn subscribe(&self) -> (mpsc::Receiver<String>, impl FnOnce() + Send + 'static) {
        let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        {
            let mut inner = self.inner.lock().expect("hub lock poisoned");
            inner.clients.insert(id, sender.clone());
        }
        let inner = Arc::clone(&self.inner);
        let unsubscribe = move || {
            if let Ok(mut inner) = inner.lock() {
                inner.clients.remove(&id);
            }
        };
        (receiver, unsubscribe)
    }

    /// 向所有订阅者发布事件；慢消费者直接丢弃（不阻塞发布者）。
    pub fn publish(&self, event: String) {
        let inner = self.inner.lock().expect("hub lock poisoned");
        for sender in inner.clients.values() {
            let _ = sender.try_send(event.clone());
        }
    }
}
