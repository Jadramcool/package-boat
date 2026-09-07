//! 桌面端事件：经 tauri-specta 注册，前端可从生成的 bindings 导入类型安全的事件对象。

use serde::Serialize;

/// 共享目录表或服务状态发生变化，前端应刷新状态快照。
#[derive(Debug, Clone, Copy, Serialize, specta::Type, tauri_specta::Event)]
pub struct StateChanged;

/// 服务端错误提示（设置、目录表或服务生命周期失败时）。
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct ErrorNotice(pub String);

/// 操作成功提示（共享文件、清空清单等）。
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct SuccessNotice(pub String);
