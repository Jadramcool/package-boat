//! LANE · 局域网投递站 — Rust 核心库
//!
//! 从 Go 版本（wails + net/http）移植的局域网文件传输服务器核心。
//! 提供：配对码认证、共享目录表（catalog）、设置存储、SSE 事件中枢、
//! 上传/下载流式处理、内嵌前端静态资源服务与桌面宿主管理。

pub mod appdata;
pub mod assets;
pub mod auth;
pub mod catalog;
pub mod files;
pub mod host;
pub mod hub;
pub mod network;
pub mod progress;
pub mod server;
pub mod settings;
pub mod uploads;

pub use catalog::{Item, SourceType};
pub use settings::Settings;

/// 数据文件中的目录表版本号（与 Go 版一致）。
pub const CATALOG_VERSION: i32 = 1;
