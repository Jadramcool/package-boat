//! LANE · 局域网投递站 — 无界面命令行服务器。
//! 参数与 Go 版 `cmd/laneshare` 一致：-host -port -dir -max-size -name。

use lane_core::catalog::Catalog;
use lane_core::server::{Config, Server};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let paths = match lane_core::appdata::default_paths() {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("无法确定用户数据目录：{message}");
            std::process::exit(1);
        }
    };
    if let Err(message) = lane_core::appdata::ensure(&paths) {
        eprintln!("初始化用户数据目录：{message}");
        std::process::exit(1);
    }

    let mut host = String::from("0.0.0.0");
    let mut port: u16 = 8080;
    let mut storage = paths.receive_dir.clone();
    let mut max_size_gb: i64 = 10;
    let mut name = default_device_name();

    parse_args(
        &mut host,
        &mut port,
        &mut storage,
        &mut max_size_gb,
        &mut name,
    );

    if port == 0 && !is_port_zero_allowed() {
        eprintln!("端口必须在 0 到 65535 之间");
        std::process::exit(1);
    }
    if max_size_gb <= 0 {
        eprintln!("单文件大小上限必须大于 0");
        std::process::exit(1);
    }

    let catalog = match Catalog::open(paths.catalog.clone()) {
        Ok(catalog) => Arc::new(catalog),
        Err(message) => {
            eprintln!("读取共享目录表失败：{message}");
            std::process::exit(1);
        }
    };

    let app = match Server::new(Config {
        device_name: name.clone(),
        storage_dir: storage.clone(),
        max_upload_bytes: max_size_gb * 1024 * 1024 * 1024,
        version: VERSION.to_string(),
        catalog: Some(catalog),
        progress: None,
    }) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("初始化失败：{message}");
            std::process::exit(1);
        }
    };

    // 一次绑定持有 listener（端口被占用时自动顺延，port=0 由系统分配），
    // 避免 drop 后 re-bind 的竞态窗口
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (listener, actual_port) = runtime.block_on(async {
        match lane_core::host::bind_with_fallback(&host, port).await {
            Ok(listener) => {
                let actual = listener
                    .local_addr()
                    .map(|addr| addr.port())
                    .unwrap_or(port);
                (listener, actual)
            }
            Err(err) => {
                eprintln!("无法监听端口：{err}");
                std::process::exit(1);
            }
        }
    });

    let absolute_storage = std::path::absolute(&storage).unwrap_or(storage.clone());
    let urls = lane_core::network::local_urls(&host, actual_port);
    print_startup(&app.access_code(), &absolute_storage, &urls);

    runtime.block_on(async {
        let router = app.router();
        let serve = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal());
        if let Err(err) = serve.await {
            eprintln!("服务异常退出：{err}");
            std::process::exit(1);
        }
    });
    info!("服务已停止");
}

/// 绑定端口 0 视为合法（自动分配）；端口必须在 0-65535。
fn is_port_zero_allowed() -> bool {
    true
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("ctrl-c handler");
    };
    #[cfg(not(windows))]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler")
            .recv()
            .await;
    };
    #[cfg(windows)]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    eprintln!("\n正在安全关闭…");
}

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "LANE 文件站".to_string())
}

fn parse_args(
    host: &mut String,
    port: &mut u16,
    storage: &mut std::path::PathBuf,
    max_size_gb: &mut i64,
    name: &mut String,
) {
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "-host" | "--host" => {
                if let Some(value) = args.next() {
                    *host = value;
                }
            }
            "-port" | "--port" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse::<u16>() {
                        *port = parsed;
                    }
                }
            }
            "-dir" | "--dir" => {
                if let Some(value) = args.next() {
                    *storage = value.into();
                }
            }
            "-max-size" | "--max-size" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse::<i64>() {
                        *max_size_gb = parsed;
                    }
                }
            }
            "-name" | "--name" => {
                if let Some(value) = args.next() {
                    *name = value;
                }
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
    }
}

fn print_usage() {
    println!(
        "LANE · 局域网投递站

用法: laneshare [选项]

选项:
  -host       监听地址（默认 0.0.0.0）
  -port       监听端口，0 表示自动分配（默认 8080）
  -dir        接收文件的保存目录（默认 %USERPROFILE%\\Downloads\\LANE）
  -max-size   单文件大小上限，单位 GB（默认 10）
  -name       设备显示名称（默认当前主机名）"
    );
}

fn print_startup(code: &str, storage: &std::path::Path, urls: &[String]) {
    println!();
    println!("  LANE · 局域网投递站");
    println!("  ─────────────────────────");
    for url in urls {
        println!("  访问地址  {url}");
    }
    println!("  配 对 码  {code}");
    println!("  保存目录  {}", storage.display());
    println!("  按 Ctrl+C 停止服务");
    println!();
}
