//! 窗口与进程生命周期辅助。

use crate::state::Desktop;
use tauri::{AppHandle, Manager};

/// 显示并聚焦主窗口（若窗口未创建则先创建）。
pub fn show_main_window(app: &AppHandle) {
    match app.get_webview_window("main") {
        Some(window) => {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        None => {
            if let Ok(builder) = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("PacketBoat · 局域网投递站")
            .inner_size(1180.0, 780.0)
            .min_inner_size(860.0, 620.0)
            .decorations(false)
            .center()
            .build()
            {
                let _ = builder.set_focus();
            }
        }
    }
}

/// 默认设备名：优先取主机名，缺失时使用固定文案。
pub fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "PacketBoat 文件站".to_string())
}

/// 退出：先优雅停止局域网服务，再退出进程。
pub fn quit_app(app: &AppHandle) {
    let app = app.clone();
    if let Some(desktop) = app.try_state::<Desktop>() {
        let host = desktop.host.clone();
        tauri::async_runtime::spawn(async move {
            host.stop().await;
            app.exit(0);
        });
    } else {
        app.exit(0);
    }
}
