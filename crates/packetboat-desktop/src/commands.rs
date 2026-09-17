//! Tauri 命令：状态查询、共享管理、接收目录、服务开关与进度。
//! 命令命名与 JSON 结构与 Go 版 Wails `DesktopApp` 对齐，前端用 `invoke` 调用。

use crate::error::AppError;
use crate::state::{
    build_state, emit_error, emit_notice, emit_state_changed, Desktop, StatePayload,
};
use packetboat_core::catalog::Item;
use packetboat_core::progress::TransferProgress;
use packetboat_core::settings::Settings;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
#[specta::specta]
pub fn get_state(state: State<'_, Desktop>) -> StatePayload {
    build_state(&state)
}

/// 原位共享一批文件（来自拖拽或选择），返回本次新增的条目。
#[tauri::command]
#[specta::specta]
pub async fn add_linked_files(
    app: AppHandle,
    state: State<'_, Desktop>,
    paths: Vec<String>,
) -> Result<Vec<Item>, AppError> {
    match state.catalog.add_linked(&paths) {
        Ok(items) => {
            emit_state_changed(&app);
            Ok(items)
        }
        Err(err) => {
            emit_error(&app, &err);
            Err(err.into())
        }
    }
}

/// 弹出文件选择对话框并原位共享所选文件。
#[tauri::command]
#[specta::specta]
pub async fn choose_linked_files(
    app: AppHandle,
    state: State<'_, Desktop>,
) -> Result<Vec<Item>, AppError> {
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
#[specta::specta]
pub fn unshare(app: AppHandle, state: State<'_, Desktop>, id: String) -> Result<(), AppError> {
    match state.catalog.remove(&id) {
        Ok(_) => {
            emit_state_changed(&app);
            Ok(())
        }
        Err(err) => {
            emit_error(&app, &err);
            Err(err.into())
        }
    }
}

/// 清空共享清单，但不删除原位共享文件或接收目录中的实际文件。
#[tauri::command]
#[specta::specta]
pub fn clear_shared_files(app: AppHandle, state: State<'_, Desktop>) -> Result<u32, AppError> {
    match state.catalog.clear() {
        Ok(count) => {
            emit_state_changed(&app);
            Ok(count as u32)
        }
        Err(err) => {
            emit_error(&app, &err);
            Err(err.into())
        }
    }
}

/// 选择远程上传文件的接收目录；更新设置后重启局域网服务。
#[tauri::command]
#[specta::specta]
pub async fn choose_receive_directory(
    app: AppHandle,
    state: State<'_, Desktop>,
) -> Result<String, AppError> {
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
        .map_err(|err| AppError::new(err.to_string()))?
        .to_string_lossy()
        .into_owned();

    let updated = state
        .settings
        .set_receive_dir(&directory)
        .inspect_err(|err| emit_error(&app, err))?;

    if let Err(err) = state.host.restart().await {
        emit_error(&app, &err);
        return Err(err.into());
    }
    emit_state_changed(&app);
    Ok(updated.receive_dir)
}

/// 开关局域网服务器。
#[tauri::command]
#[specta::specta]
pub async fn toggle_server(app: AppHandle, state: State<'_, Desktop>) -> Result<(), AppError> {
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
            Err(err.into())
        }
    }
}

/// 更新「是否需要配对码访问」开关；服务运行中时重启以生效。
#[tauri::command]
#[specta::specta]
pub async fn set_require_pairing(
    app: AppHandle,
    state: State<'_, Desktop>,
    enabled: bool,
) -> Result<Settings, AppError> {
    let updated = state
        .settings
        .set_require_pairing(enabled)
        .inspect_err(|err| emit_error(&app, err))?;

    if state.host.state().running {
        if let Err(err) = state.host.restart().await {
            emit_error(&app, &err);
            return Err(err.into());
        }
    }
    emit_state_changed(&app);
    Ok(updated)
}

/// 在资源管理器中显示条目所在目录。
#[tauri::command]
#[specta::specta]
pub fn reveal_item(state: State<'_, Desktop>, id: String) -> Result<(), AppError> {
    let (item, _) = state
        .catalog
        .resolve(&id)
        .map_err(|_| AppError::new("文件不可用"))?;
    tauri_plugin_opener::reveal_item_in_dir(PathBuf::from(item.local_path))
        .map_err(|err| AppError::new(err.to_string()))
}

/// 当前传输进度（任务栏进度条轮询）。
#[tauri::command]
#[specta::specta]
pub fn get_transfer_progress(state: State<'_, Desktop>) -> TransferProgress {
    state.host.progress().snapshot()
}

/// 从任务栏拖入文件：直接把路径加入原位共享（非命令，供窗口事件调用）。
pub fn add_linked_from_paths(app: &AppHandle, paths: Vec<PathBuf>) {
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
            emit_state_changed(app);
            let message = if items.is_empty() {
                "所选文件已经在共享清单中。".to_string()
            } else {
                format!("已添加 {} 个共享文件", items.len())
            };
            emit_notice(app, message);
        }
        Err(err) => emit_error(app, err),
    }
}
