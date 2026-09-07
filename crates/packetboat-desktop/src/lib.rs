//! PacketBoat 桌面端（Tauri 2）：把 packetboat-core 的服务器/目录表/设置桥接给前端。
//!
//! 模块划分：
//! - [`commands`]：暴露给前端的 Tauri 命令
//! - [`events`]：类型安全的前端事件
//! - [`state`]：共享状态与前端状态快照
//! - [`error`]：统一错误类型（序列化为字符串）
//! - [`tray`]：系统托盘
//! - [`window`]：窗口与进程生命周期

mod commands;
mod error;
mod events;
mod state;
mod tray;
mod window;

use events::{ErrorNotice, StateChanged, SuccessNotice};
use state::Desktop;
use tauri::{Manager, WindowEvent};
use tauri_specta::{collect_commands, collect_events, Builder};
use tracing::{info, warn};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 调试构建时把命令/事件/类型导出为 TypeScript 绑定文件。
/// 路径基于 crate 根目录解析，无论从哪个工作目录启动都有效。
#[cfg(debug_assertions)]
fn export_bindings(builder: &Builder<tauri::Wry>) {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src/bindings.ts");
    builder
        .export(specta_typescript::Typescript::default(), &path)
        .expect("导出前端 bindings 失败");
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    /// 运行 `cargo test` 即可重新生成前端 bindings.ts。
    #[test]
    fn exports_typescript_bindings() {
        let builder = Builder::<tauri::Wry>::new()
            .commands(collect_commands![
                commands::get_state,
                commands::add_linked_files,
                commands::choose_linked_files,
                commands::unshare,
                commands::clear_shared_files,
                commands::choose_receive_directory,
                commands::toggle_server,
                commands::reveal_item,
                commands::get_transfer_progress,
            ])
            .events(collect_events![StateChanged, ErrorNotice, SuccessNotice]);
        export_bindings(&builder);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let paths = match packetboat_core::appdata::default_paths() {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("无法确定用户数据目录：{message}");
            std::process::exit(1);
        }
    };
    if let Err(message) = packetboat_core::appdata::ensure(&paths) {
        eprintln!("初始化用户数据目录：{message}");
        std::process::exit(1);
    }

    let catalog = match packetboat_core::catalog::Catalog::open(paths.catalog.clone()) {
        Ok(catalog) => std::sync::Arc::new(catalog),
        Err(message) => {
            eprintln!("读取共享目录表失败：{message}");
            std::process::exit(1);
        }
    };
    let device_name = window::default_device_name();
    let settings = match packetboat_core::settings::Store::open(
        paths.settings.clone(),
        packetboat_core::settings::Settings::defaults(device_name, paths.receive_dir.clone()),
    ) {
        Ok(store) => std::sync::Arc::new(store),
        Err(message) => {
            eprintln!("读取设置失败：{message}");
            std::process::exit(1);
        }
    };
    let host = std::sync::Arc::new(packetboat_core::host::HostManager::new(
        catalog.clone(),
        settings.clone(),
        VERSION,
    ));

    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::get_state,
            commands::add_linked_files,
            commands::choose_linked_files,
            commands::unshare,
            commands::clear_shared_files,
            commands::choose_receive_directory,
            commands::toggle_server,
            commands::reveal_item,
            commands::get_transfer_progress,
        ])
        .events(collect_events![StateChanged, ErrorNotice, SuccessNotice]);

    // 调试构建时把命令/事件/类型导出为 TypeScript，前端据此获得类型安全封装。
    #[cfg(debug_assertions)]
    export_bindings(&builder);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Desktop {
            catalog,
            settings,
            host,
            version: VERSION.to_string(),
        })
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            tray::create_tray(app)?;

            // 启动时自动开启局域网服务（与 Go 版 `startup` 一致）
            let host = app.state::<Desktop>().host.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = host.start().await {
                    warn!("启动局域网服务失败: {err}");
                } else {
                    info!("局域网服务已启动");
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // 关闭窗口 → 最小化到托盘（局域网服务继续运行）
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // 任务栏/窗口拖放文件 → 原位共享
            WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                commands::add_linked_from_paths(window.app_handle(), paths.clone());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running PacketBoat desktop");
}
