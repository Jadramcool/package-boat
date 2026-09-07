//! 设置存储：`settings.json` 的原子读写，字段与 Go 版 `internal/settings` 一致。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// 与 Go 版 `settings.Settings` 字段完全一致。所有字段带默认值，
/// 缺失字段/越界端口不阻塞启动（与 Go 版 `applyFallbacks` 容错语义一致）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub struct Settings {
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub host: String,
    #[cfg_attr(feature = "specta", specta(type = u16))]
    #[serde(default = "default_port", deserialize_with = "deserialize_port")]
    pub port: u16,
    #[serde(default)]
    pub receive_dir: String,
    #[serde(default)]
    /// 单文件上传上限；specta 导出为 number（TS 侧 2^53 内精度安全）。
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_upload_bytes: i64,
}

/// 缺省端口（与 Go 版默认一致）。
fn default_port() -> u16 {
    8080
}

/// 端口容错：`>65535`、负数、字符串、浮点等一律回退默认端口 8080
/// （`0` 保持合法 = 自动分配）。与 Go 版 `applyFallbacks` 的容错语义对齐。
fn deserialize_port<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Number(number) => match number.as_u64() {
            Some(port) if port <= 65535 => port as u16,
            _ => 8080,
        },
        _ => 8080,
    })
}

impl Settings {
    pub fn defaults(device_name: String, receive_dir: PathBuf) -> Self {
        Settings {
            device_name,
            host: "0.0.0.0".to_string(),
            port: 8080,
            receive_dir: receive_dir.to_string_lossy().into_owned(),
            max_upload_bytes: 10 * 1024 * 1024 * 1024,
        }
    }
}

/// 线程安全的设置存储。所有写操作先落盘（原子替换），失败时回滚内存状态。
pub struct Store {
    path: PathBuf,
    inner: RwLock<Settings>,
}

impl Store {
    /// 打开设置文件；文件不存在时使用 `defaults`，解析成功后对缺失字段应用回退。
    pub fn open(path: PathBuf, defaults: Settings) -> Result<Self, String> {
        let mut settings = defaults.clone();
        if !path.as_os_str().is_empty() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let parsed: Settings =
                        serde_json::from_str(&content).map_err(|err| format!("解析设置: {err}"))?;
                    settings = parsed;
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(format!("读取设置: {err}")),
            }
            apply_fallbacks(&mut settings, &defaults);
        }
        Ok(Store {
            path,
            inner: RwLock::new(settings),
        })
    }

    pub fn get(&self) -> Settings {
        self.inner.read().expect("settings lock poisoned").clone()
    }

    /// 更新接收目录：先创建目录，再落盘；失败时恢复原值。
    pub fn set_receive_dir(&self, directory: &str) -> Result<Settings, String> {
        let trimmed = directory.trim();
        let absolute =
            std::path::absolute(trimmed).map_err(|err| format!("解析接收目录: {err}"))?;
        std::fs::create_dir_all(&absolute).map_err(|err| format!("创建接收目录: {err}"))?;

        let mut guard = self.inner.write().expect("settings lock poisoned");
        let previous = guard.clone();
        guard.receive_dir = absolute.to_string_lossy().into_owned();
        match self.save_locked(&guard) {
            Ok(()) => Ok(guard.clone()),
            Err(err) => {
                *guard = previous;
                Err(err)
            }
        }
    }

    /// 更新监听端口（端口被占用自动顺延后回写，保证 UI 显示与实际一致）。
    pub fn set_port(&self, port: u16) -> Result<Settings, String> {
        let mut guard = self.inner.write().expect("settings lock poisoned");
        let previous = guard.clone();
        guard.port = port;
        match self.save_locked(&guard) {
            Ok(()) => Ok(guard.clone()),
            Err(err) => {
                *guard = previous;
                Err(err)
            }
        }
    }

    fn save_locked(&self, settings: &Settings) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        atomic_write_json(&self.path, settings)
    }
}

fn apply_fallbacks(settings: &mut Settings, defaults: &Settings) {
    if settings.device_name.trim().is_empty() {
        settings.device_name = defaults.device_name.clone();
    }
    if settings.host.trim().is_empty() {
        settings.host = defaults.host.clone();
    }
    // 端口为 u16，范围必然合法；0 表示自动分配，与 Go 版一致，不重置。
    if settings.receive_dir.trim().is_empty() {
        settings.receive_dir = defaults.receive_dir.clone();
    }
    if settings.max_upload_bytes <= 0 {
        settings.max_upload_bytes = defaults.max_upload_bytes;
    }
}

/// 原子写入 JSON：临时文件 + fsync + 重命名（重命名失败时先删除旧文件再重试）。
pub(crate) fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("创建数据目录: {err}"))?;
    }
    let content = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let temporary =
        tempfile::NamedTempFile::new_in(dir).map_err(|err| format!("创建临时文件: {err}"))?;
    std::fs::write(temporary.path(), &content).map_err(|err| err.to_string())?;
    sync_file(&temporary)?;
    match temporary.persist(path) {
        Ok(_) => Ok(()),
        Err(persist_err) => {
            // Windows 上重命名可能因目标存在而失败：删除后重试一次。
            let _ = std::fs::remove_file(path);
            persist_err
                .file
                .persist(path)
                .map(|_| ())
                .map_err(|err| format!("写入 {path:?}: {}", err.error))
        }
    }
}

fn sync_file(file: &tempfile::NamedTempFile) -> Result<(), String> {
    file.as_file()
        .sync_all()
        .map_err(|err| format!("同步文件: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> Settings {
        Settings {
            device_name: "TEST-PC".into(),
            host: "0.0.0.0".into(),
            port: 8080,
            receive_dir: r"C:\Temp\PacketBoat".into(),
            max_upload_bytes: 10 * 1024 * 1024 * 1024,
        }
    }

    #[test]
    fn persists_and_reloads() {
        let dir = std::env::temp_dir().join(format!("packetboat-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");

        let store = Store::open(path.clone(), test_settings()).unwrap();
        assert_eq!(store.get().device_name, "TEST-PC");

        let receive = dir.join("incoming");
        store.set_receive_dir(receive.to_str().unwrap()).unwrap();
        assert!(receive.exists());

        let reloaded = Store::open(path, test_settings()).unwrap();
        assert_eq!(
            reloaded.get().receive_dir,
            receive.to_str().unwrap().replace('/', "\\")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_uses_defaults() {
        let store = Store::open(PathBuf::new(), test_settings()).unwrap();
        assert_eq!(store.get().port, 8080);
    }

    #[test]
    fn malformed_port_values_do_not_block_startup() {
        // 缺 port 键 → 8080；负数/字符串/超界 → 8080
        for content in [
            r#"{"device_name":"X","host":"0.0.0.0","receive_dir":"C:\\T","max_upload_bytes":1}"#,
            r#"{"device_name":"X","host":"0.0.0.0","port":-1,"receive_dir":"C:\\T","max_upload_bytes":1}"#,
            r#"{"device_name":"X","host":"0.0.0.0","port":"8080","receive_dir":"C:\\T","max_upload_bytes":1}"#,
            r#"{"device_name":"X","host":"0.0.0.0","port":70000,"receive_dir":"C:\\T","max_upload_bytes":1}"#,
            r#"{"device_name":"X","host":"0.0.0.0","port":null,"receive_dir":"C:\\T","max_upload_bytes":1}"#,
        ] {
            let dir =
                std::env::temp_dir().join(format!("packetboat-settings-bad-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("settings.json");
            std::fs::write(&path, content).unwrap();
            let store = Store::open(path.clone(), test_settings()).unwrap();
            assert_eq!(store.get().port, 8080, "content: {content}");
            let _ = std::fs::remove_dir_all(&dir);
        }
        // 0（自动分配）仍保留
        let store = Store::open(
            PathBuf::new(),
            Settings {
                device_name: "X".into(),
                host: "0.0.0.0".into(),
                port: 0,
                receive_dir: "C:\\T".into(),
                max_upload_bytes: 1,
            },
        )
        .unwrap();
        assert_eq!(store.get().port, 0);
    }

    #[test]
    fn fallbacks_fill_blank_fields() {
        let mut defaults = test_settings();
        defaults.port = 9090;
        let mut settings = Settings {
            device_name: String::new(),
            host: String::new(),
            port: 0, // 0 = 自动分配，保持不动
            receive_dir: String::new(),
            max_upload_bytes: 0,
        };
        apply_fallbacks(&mut settings, &defaults);
        assert_eq!(settings.port, 0);
        assert_eq!(settings.device_name, "TEST-PC");
        assert_eq!(settings.max_upload_bytes, defaults.max_upload_bytes);
    }
}
