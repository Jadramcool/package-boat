//! 桌面宿主管理：按设置启动/停止/重启局域网服务器，跟踪运行状态。
//! 对应 Go 版 `internal/host`。

use crate::catalog::Catalog;
use crate::network::{local_addresses, LocalAddress};
use crate::progress::ProgressTracker;
use crate::server::{Config, Server};
use crate::settings::Store as SettingsStore;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// 宿主运行状态（桌面 UI 直接序列化为 JSON）。
#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct HostState {
    pub running: bool,
    /// 可达性排序后的访问 URL 列表（等价于 `addresses` 的 `url` 投影，保留向后兼容）。
    pub urls: Vec<String>,
    /// 带适配器名、掩码、虚拟网卡标记与可达性分级的地址列表，供 UI 分组展示。
    pub addresses: Vec<LocalAddress>,
    pub access_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct HostHandle {
    token: CancellationToken,
    task: JoinHandle<()>,
}

struct HostInner {
    handle: Option<HostHandle>,
    state: HostState,
}

/// 绑定监听端口：目标端口被占用时自动顺延（最多尝试 50 个），端口 0 由系统分配。
/// CLI 与桌面宿主共用；调用方需自行决定是否回写设置。
pub async fn bind_with_fallback(host: &str, port: u16) -> Result<TcpListener, String> {
    if port == 0 {
        return TcpListener::bind((host, 0))
            .await
            .map_err(|err| format!("无法监听端口: {err}"));
    }
    let max_try = port.saturating_add(49);
    let mut last_error = String::new();
    for candidate in port..=max_try {
        match TcpListener::bind((host, candidate)).await {
            Ok(listener) => return Ok(listener),
            Err(err) => last_error = format!("{candidate}: {err}"),
        }
    }
    Err(format!(
        "无法监听端口（{port}-{max_try} 均被占用）：{last_error}"
    ))
}

/// 桌面宿主管理器。
pub struct HostManager {
    inner: Mutex<HostInner>,
    catalog: Arc<Catalog>,
    settings: Arc<SettingsStore>,
    version: String,
    progress: Arc<ProgressTracker>,
    /// 串行化 start/stop/restart，防止并发调用双起服务器（TOCTOU 竞态）。
    lifecycle: tokio::sync::Mutex<()>,
}

impl HostManager {
    pub fn new(catalog: Arc<Catalog>, settings: Arc<SettingsStore>, version: &str) -> Self {
        HostManager {
            inner: Mutex::new(HostInner {
                handle: None,
                state: HostState::default(),
            }),
            catalog,
            settings,
            version: version.to_string(),
            progress: ProgressTracker::new(),
            lifecycle: tokio::sync::Mutex::new(()),
        }
    }

    /// 传输进度跟踪器（桌面端任务栏进度读取；与服务器共享同一实例）。
    pub fn progress(&self) -> Arc<ProgressTracker> {
        Arc::clone(&self.progress)
    }

    /// 启动服务器（已在运行时则直接返回）。
    pub async fn start(&self) -> Result<(), String> {
        let _lifecycle = self.lifecycle.lock().await;
        {
            let inner = self.inner.lock().expect("host lock poisoned");
            if inner.state.running {
                return Ok(());
            }
        }

        let settings = self.settings.get();
        let app = Server::new(Config {
            device_name: settings.device_name.clone(),
            storage_dir: settings.receive_dir.clone().into(),
            max_upload_bytes: settings.max_upload_bytes,
            version: self.version.clone(),
            catalog: Some(self.catalog.clone()),
            progress: Some(Arc::clone(&self.progress)),
            require_pairing: settings.require_pairing,
        })
        .inspect_err(|err| {
            let mut inner = self.inner.lock().expect("host lock poisoned");
            inner.state.error = Some(err.to_string());
        })?;

        let listener = bind_with_fallback(&settings.host, settings.port)
            .await
            .inspect_err(|err| {
                let mut inner = self.inner.lock().expect("host lock poisoned");
                inner.state.error = Some(err.to_string());
            })?;
        let actual_port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or(settings.port);

        // 端口被占用而顺延时，回写设置，保证界面显示与实际一致
        if settings.port != 0 && actual_port != settings.port {
            if let Err(err) = self.settings.set_port(actual_port) {
                warn!("回写端口设置失败: {err}");
            }
        }

        let router = app.router();
        let token = CancellationToken::new();
        let shutdown_token = token.clone();
        let task = tokio::spawn(async move {
            let serve = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_token.cancelled_owned());
            if let Err(err) = serve.await {
                warn!("局域网服务异常退出: {err}");
            }
        });

        let mut inner = self.inner.lock().expect("host lock poisoned");
        inner.handle = Some(HostHandle { token, task });
        let addresses = local_addresses(&settings.host, actual_port);
        inner.state = HostState {
            running: true,
            urls: addresses.iter().map(|a| a.url.clone()).collect(),
            addresses,
            access_code: app.access_code(),
            error: None,
        };
        Ok(())
    }

    /// 停止服务器并等待退出。
    pub async fn stop(&self) {
        let _lifecycle = self.lifecycle.lock().await;
        let handle = {
            let mut inner = self.inner.lock().expect("host lock poisoned");
            if !inner.state.running {
                return;
            }
            inner.state.running = false;
            inner.state.urls.clear();
            inner.state.addresses.clear();
            inner.state.access_code.clear();
            inner.handle.take()
        };
        if let Some(handle) = handle {
            handle.token.cancel();
            // 等待优雅退出；超时后强制中止，避免泄漏的服务器继续占用端口
            let task = handle.task;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !task.is_finished() && std::time::Instant::now() < deadline {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            if !task.is_finished() {
                warn!("停止服务器超时，强制中止");
                task.abort();
                let _ = task.await;
            }
        }
    }

    /// 停止后立即重启。
    pub async fn restart(&self) -> Result<(), String> {
        self.stop().await;
        self.start().await
    }

    /// 当前状态。
    pub fn state(&self) -> HostState {
        self.inner.lock().expect("host lock poisoned").state.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[tokio::test]
    async fn starts_and_stops_http_service() {
        use std::path::PathBuf;
        let temp = std::env::temp_dir().join(format!("packetboat-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let catalog = Arc::new(Catalog::open(PathBuf::new()).unwrap());
        let settings = Arc::new(
            SettingsStore::open(
                PathBuf::new(),
                Settings {
                    device_name: "test".into(),
                    host: "127.0.0.1".into(),
                    port: 0,
                    receive_dir: temp.to_string_lossy().into_owned(),
                    max_upload_bytes: 1024,
                    require_pairing: true,
                },
            )
            .unwrap(),
        );
        let manager = HostManager::new(catalog, settings, "test");
        manager.start().await.expect("start");
        let state = manager.state();
        assert!(state.running);
        assert_eq!(state.urls.len(), 1);
        assert_eq!(state.addresses.len(), 1);
        assert!(!state.access_code.is_empty());
        assert!(state.urls[0].contains("127.0.0.1"));
        // urls 必须是 addresses 的 url 投影，且顺序一致
        assert_eq!(
            state.urls,
            state
                .addresses
                .iter()
                .map(|a| a.url.clone())
                .collect::<Vec<_>>()
        );

        let response = reqwest_get(&state.urls[0]).await;
        assert_eq!(response, 200);

        manager.stop().await;
        assert!(!manager.state().running);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn port_conflict_falls_back() {
        use crate::settings::Settings;
        use std::path::PathBuf;
        let temp = std::env::temp_dir().join(format!("packetboat-host-fb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        // 先占住一个端口，让 start 顺延到下一个
        let occupied = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();

        let catalog = Arc::new(Catalog::open(PathBuf::new()).unwrap());
        let settings = Arc::new(
            SettingsStore::open(
                PathBuf::new(),
                Settings {
                    device_name: "test".into(),
                    host: "127.0.0.1".into(),
                    port: occupied_port,
                    receive_dir: temp.to_string_lossy().into_owned(),
                    max_upload_bytes: 1024,
                    require_pairing: true,
                },
            )
            .unwrap(),
        );
        let manager = HostManager::new(catalog, settings.clone(), "test");
        manager.start().await.expect("start should fall back");
        let state = manager.state();
        assert!(state.running);
        // 实际端口 ≠ 被占端口；URL 指向顺延后的端口
        let url = &state.urls[0];
        let actual_port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
        assert_ne!(actual_port, occupied_port);
        assert_eq!(settings.get().port, actual_port, "顺延后应回写 settings");

        manager.stop().await;
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn concurrent_start_only_starts_once() {
        use crate::settings::Settings;
        use std::path::PathBuf;
        let temp = std::env::temp_dir().join(format!("packetboat-host-cc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let catalog = Arc::new(Catalog::open(PathBuf::new()).unwrap());
        let settings = Arc::new(
            SettingsStore::open(
                PathBuf::new(),
                Settings {
                    device_name: "test".into(),
                    host: "127.0.0.1".into(),
                    port: 0,
                    receive_dir: temp.to_string_lossy().into_owned(),
                    max_upload_bytes: 1024,
                    require_pairing: true,
                },
            )
            .unwrap(),
        );
        let manager = Arc::new(HostManager::new(catalog, settings, "test"));

        // 并发调用 start：只应成功启动一个服务器
        let mut handles = Vec::new();
        for _ in 0..5 {
            let manager = Arc::clone(&manager);
            handles.push(tokio::spawn(async move { manager.start().await }));
        }
        let results = futures_util::future::join_all(handles).await;
        assert!(results.into_iter().all(|r| r.unwrap().is_ok()));
        let state = manager.state();
        assert!(state.running);
        assert_eq!(state.urls.len(), 1);

        manager.stop().await;
        assert!(!manager.state().running);
        let _ = std::fs::remove_dir_all(&temp);
    }

    async fn reqwest_get(url: &str) -> u16 {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let host_port = url.trim_start_matches("http://");
        let mut stream = tokio::net::TcpStream::connect(host_port)
            .await
            .expect("connect");
        let _ = stream
            .write_all(
                format!("GET /api/info HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await;
        let mut buffer = [0u8; 4096];
        let n = stream.read(&mut buffer).await.expect("read");
        let response = String::from_utf8_lossy(&buffer[..n]);
        let status_line = response.lines().next().unwrap_or("");
        status_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0)
    }
}
