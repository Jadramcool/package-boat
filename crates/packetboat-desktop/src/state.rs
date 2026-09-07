//! 桌面端共享状态与前端状态快照。

use crate::events::{ErrorNotice, StateChanged, SuccessNotice};
use packetboat_core::catalog::{Catalog, Item};
use packetboat_core::host::{HostManager, HostState};
use packetboat_core::settings::{Settings, Store as SettingsStore};
use serde::Serialize;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_specta::Event;

/// 桌面端共享状态。
pub struct Desktop {
    pub catalog: Arc<Catalog>,
    pub settings: Arc<SettingsStore>,
    pub host: Arc<HostManager>,
    pub version: String,
}

/// 返回给前端的完整状态快照（与 Go 版 `DesktopState` 字段一致）。
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub struct StatePayload {
    pub host: HostState,
    pub settings: Settings,
    pub items: Vec<Item>,
    pub version: String,
}

pub fn build_state(state: &tauri::State<'_, Desktop>) -> StatePayload {
    StatePayload {
        host: state.host.state(),
        settings: state.settings.get(),
        items: state.catalog.list(),
        version: state.version.clone(),
    }
}

pub fn emit_state_changed(app: &AppHandle) {
    let _ = StateChanged.emit(app);
}

pub fn emit_error(app: &AppHandle, message: impl std::fmt::Display) {
    let _ = ErrorNotice(message.to_string()).emit(app);
}

pub fn emit_notice(app: &AppHandle, message: impl std::fmt::Display) {
    let _ = SuccessNotice(message.to_string()).emit(app);
}
