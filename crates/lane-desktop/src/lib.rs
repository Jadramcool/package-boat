//! LANE 桌面端（Tauri 2）：把 lane-core 的服务器/目录表/设置桥接给前端。
//! 命令命名与 JSON 结构与 Go 版 Wails `DesktopApp` 对齐，前端用 `invoke` 调用。

use lane_core::catalog::{Catalog, Item};
use lane_core::host::{HostManager, HostState};
use lane_core::progress::TransferProgress;
use lane_core::settings::{Settings, Store as SettingsStore};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tracing::{info, warn};

const VERSION: &str = "0.2.0";

/// 桌面端共享状态。
struct Desktop {
    catalog: Arc<Catalog>,
    settings: Arc<SettingsStore>,
    host: Arc<HostManager>,
    version: String,
}

/// 返回给前端的完整状态快照（与 Go 版 `DesktopState` 字段一致）。
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct StatePayload {
    host: HostState,
    settings: Settings,
    items: Vec<Item>,
    version: String,
}

fn build_state(state: &State<'_, Desktop>) -> StatePayload {
    StatePayload {
        host: state.host.state(),
        settings: state.settings.get(),
        items: state.catalog.list(),
        version: state.version.clone(),
    }
}

fn emit_state_changed(app: &AppHandle) {
    let _ = app.emit("desktop:state-changed", ());
}

fn emit_error(app: &AppHandle, message: &str) {
    let _ = app.emit("desktop:error", message.to_string());
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_state(state: State<'_, Desktop>) -> StatePayload {
    build_state(&state)
}

/// 原位共享一批文件（来自拖拽或选择），返回本次新增的条目。
#[tauri::command]
async fn add_linked_files(
    app: AppHandle,
    state: State<'_, Desktop>,
    paths: Vec<String>,
) -> Result<Vec<Item>, String> {
    match state.catalog.add_linked(&paths) {
        Ok(items) => {
            emit_state_changed(&app);
            Ok(items)
        }
        Err(err) => {
            emit_error(&app, &err);
            Err(err)
        }
    }
}

/// 弹出文件选择对话框并原位共享所选文件。
#[tauri::command]
async fn choose_linked_files(
    app: AppHandle,
    state: State<'_, Desktop>,
) -> Result<Vec<Item>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("选择要共享的文件（不会复制或移动）")
        .blocking_pick_files();
    let Some(files) = picked else {
        return Ok(Vec::new());
    };
    let paths: Vec<String> = files
        .into_iter()
        .filter_map(|file| file.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    add_linked_files(app, state, paths).await
}

/// 取消共享（只移除目录表记录，不删除原文件）。
#[tauri::command]
fn unshare(app: AppHandle, state: State<'_, Desktop>, id: String) -> Result<(), String> {
    match state.catalog.remove(&id) {
        Ok(_) => {
            emit_state_changed(&app);
            Ok(())
        }
        Err(err) => {
            emit_error(&app, &err);
            Err(err)
        }
    }
}

/// 选择远程上传文件的接收目录；更新设置后重启局域网服务。
#[tauri::command]
async fn choose_receive_directory(
    app: AppHandle,
    state: State<'_, Desktop>,
) -> Result<String, String> {
    let current = state.settings.get().receive_dir;
    let picked = app
        .dialog()
        .file()
        .set_title("选择远程上传文件的接收目录")
        .set_directory(PathBuf::from(&current))
        .blocking_pick_folder();
    let Some(folder) = picked else {
        return Ok(current);
    };
    let directory = folder
        .into_path()
        .map_err(|err| err.to_string())?
        .to_string_lossy()
        .into_owned();

    let updated = state
        .settings
        .set_receive_dir(&directory)
        .inspect_err(|err| {
            emit_error(&app, err);
        })?;

    if let Err(err) = state.host.restart().await {
        emit_error(&app, &err);
        return Err(err);
    }
    emit_state_changed(&app);
    Ok(updated.receive_dir)
}

/// 开关局域网服务器。
#[tauri::command]
async fn toggle_server(app: AppHandle, state: State<'_, Desktop>) -> Result<(), String> {
    let result = if state.host.state().running {
        state.host.stop().await;
        Ok(())
    } else {
        state.host.start().await
    };
    match result {
        Ok(()) => {
            emit_state_changed(&app);
            Ok(())
        }
        Err(err) => {
            emit_error(&app, &err);
            Err(err)
        }
    }
}

/// 在资源管理器中显示条目所在目录。
#[tauri::command]
fn reveal_item(state: State<'_, Desktop>, id: String) -> Result<(), String> {
    let (item, _) = state
        .catalog
        .resolve(&id)
        .map_err(|_| "文件不可用".to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(PathBuf::from(item.local_path))
        .map_err(|err| err.to_string())
}

/// 当前传输进度（任务栏进度条轮询）。
#[tauri::command]
fn get_transfer_progress(state: State<'_, Desktop>) -> TransferProgress {
    state.host.progress().snapshot()
}

/// 从任务栏拖入文件：直接把路径加入原位共享。
fn add_linked_from_paths(app: &AppHandle, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let state = app.state::<Desktop>();
    let strings: Vec<String> = paths
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    match state.catalog.add_linked(&strings) {
        Ok(items) => {
            let _ = app.emit("desktop:state-changed", ());
            let message = if items.is_empty() {
                "所选文件已经在共享清单中。".to_string()
            } else {
                format!("已添加 {} 个共享文件", items.len())
            };
            let _ = app.emit("desktop:notice", message);
        }
        Err(err) => {
            let _ = app.emit("desktop:error", err);
        }
    }
}

// ---------------------------------------------------------------------------
// 应用入口
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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

    let catalog = match Catalog::open(paths.catalog.clone()) {
        Ok(catalog) => Arc::new(catalog),
        Err(message) => {
            eprintln!("读取共享目录表失败：{message}");
            std::process::exit(1);
        }
    };
    let device_name = default_device_name();
    let settings = match SettingsStore::open(
        paths.settings.clone(),
        Settings::defaults(device_name, paths.receive_dir.clone()),
    ) {
        Ok(store) => Arc::new(store),
        Err(message) => {
            eprintln!("读取设置失败：{message}");
            std::process::exit(1);
        }
    };
    let host = Arc::new(HostManager::new(catalog.clone(), settings.clone(), VERSION));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(Desktop {
            catalog,
            settings,
            host,
            version: VERSION.to_string(),
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            add_linked_files,
            choose_linked_files,
            unshare,
            choose_receive_directory,
            toggle_server,
            reveal_item,
            get_transfer_progress
        ])
        .setup(|app| {
            // 系统托盘：显示主窗口 / 退出
            let show_item = MenuItem::with_id(app, "show", "打开 LANE", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let icon = app.default_window_icon().cloned().ok_or("缺少应用图标")?;
            TrayIconBuilder::with_id("lane-tray")
                .icon(icon)
                .tooltip("LANE · 局域网投递站")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "quit" => quit_app(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键单击托盘图标 → 显示/聚焦主窗口
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

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
                add_linked_from_paths(window.app_handle(), paths.clone());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running LANE desktop");
}

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "LANE 文件站".to_string())
}

/// 显示并聚焦主窗口（若窗口未创建则先创建）。
fn show_main_window(app: &AppHandle) {
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
            .title("LANE · 局域网投递站")
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

/// 退出：先优雅停止局域网服务，再退出进程。
fn quit_app(app: &AppHandle) {
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
