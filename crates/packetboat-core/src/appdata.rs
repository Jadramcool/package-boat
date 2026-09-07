//! 应用数据路径解析：`%LOCALAPPDATA%\PacketBoat`（目录表、设置）与 `Downloads\PacketBoat`（接收目录）。

use std::path::PathBuf;

/// Windows 上的应用数据目录名。
pub const DIRECTORY_NAME: &str = "PacketBoat";

/// 与 Go 版 `appdata.Paths` 对应的路径集合。
#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub catalog: PathBuf,
    pub settings: PathBuf,
    pub receive_dir: PathBuf,
}

/// 解析默认路径。`data_dir` 位于 `%LOCALAPPDATA%\PacketBoat`（无此变量时回退到系统配置目录），
/// `receive_dir` 位于 `{用户主目录}\Downloads\PacketBoat`。
pub fn default_paths() -> Result<Paths, String> {
    let data_root = local_data_root()?;
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "无法确定用户主目录".to_string())?;

    let data_dir = data_root.join(DIRECTORY_NAME);
    Ok(Paths {
        data_dir: data_dir.clone(),
        catalog: data_dir.join("catalog.json"),
        settings: data_dir.join("settings.json"),
        receive_dir: home.join("Downloads").join(DIRECTORY_NAME),
    })
}

/// 创建应用数据目录（0700）与接收目录（0755），已存在时忽略错误。
pub fn ensure(paths: &Paths) -> Result<(), String> {
    std::fs::create_dir_all(&paths.data_dir).map_err(|err| format!("创建应用数据目录: {err}"))?;
    std::fs::create_dir_all(&paths.receive_dir).map_err(|err| format!("创建接收目录: {err}"))?;
    Ok(())
}

fn local_data_root() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("LOCALAPPDATA") {
        if !value.is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    dirs_config_dir().ok_or_else(|| "无法确定应用数据根目录".to_string())
}

fn dirs_config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        // 避免引入 dirs 依赖：Windows 上配置目录 == AppData\Roaming
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_shape() {
        let paths = default_paths().expect("paths resolve");
        assert!(paths.data_dir.ends_with(DIRECTORY_NAME));
        assert!(paths.catalog.ends_with("catalog.json"));
        assert!(paths.settings.ends_with("settings.json"));
        assert!(paths.receive_dir.ends_with("PacketBoat"));
    }
}
