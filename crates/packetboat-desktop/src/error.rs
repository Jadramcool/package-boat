//! 桌面端命令的统一错误类型。
//!
//! packetboat-core 与 Tauri 插件的错误均以字符串形式返回，这里聚合为单一消息类型：
//! `thiserror` 提供 `Display`，`Serialize` 把错误序列化为字符串交给前端，
//! 与旧版 `Result<T, String>` 的线上行为保持一致。

use serde::Serialize;

/// 桌面端命令错误。
/// `Serialize` 输出纯字符串；`specta(transparent)` 让生成的 TS 类型同为 `string`。
#[derive(Debug, thiserror::Error, specta::Type)]
#[error("{message}")]
#[specta(transparent)]
pub struct AppError {
    message: String,
}

impl AppError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_plain_string() {
        let error = AppError::new("配对码不正确");
        assert_eq!(serde_json::to_string(&error).unwrap(), "\"配对码不正确\"");
    }

    #[test]
    fn from_string_keeps_message() {
        let error: AppError = "磁盘已满".to_string().into();
        assert_eq!(error.to_string(), "磁盘已满");
    }
}
