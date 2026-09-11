//! 文件处理：文件名净化、Content-Disposition（RFC 5987）、上传落盘（临时文件 + 原子重命名）。

use crate::catalog::{Catalog, Item};
use rand::RngCore;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter as AsyncBufWriter};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

/// 大文件传输使用较大的用户态缓冲，减少异步运行时与系统调用开销。
pub const TRANSFER_BUFFER_SIZE: usize = 256 * 1024;

/// 启动时回收超过此时长的残留上传/压缩临时文件。
pub const STALE_TRANSFER_FILE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// 临时文件清理结果。失败项会被保留，避免因清理问题阻止服务启动。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: usize,
    pub failed: usize,
}

impl CleanupReport {
    fn merge(&mut self, other: CleanupReport) {
        self.removed += other.removed;
        self.failed += other.failed;
    }
}

/// 清理接收目录中的残留上传文件，以及系统临时目录中的残留文件夹 ZIP。
/// 仅删除严格匹配 PacketBoat 命名规则且超过 24 小时的普通文件。
pub fn cleanup_stale_transfer_files(storage_dir: &Path) -> CleanupReport {
    let mut report = cleanup_stale_in_directory(storage_dir, STALE_TRANSFER_FILE_AGE);
    let system_temp = std::env::temp_dir();
    if system_temp != storage_dir {
        report.merge(cleanup_stale_in_directory(
            &system_temp,
            STALE_TRANSFER_FILE_AGE,
        ));
    }
    report
}

/// 检查接收目录所在文件系统能否容纳预期请求体。
/// multipart 请求体略大于实际文件，因此该检查会保守预留协议开销。
pub fn has_upload_capacity(storage_dir: &Path, expected_bytes: u64) -> std::io::Result<bool> {
    fs2::available_space(storage_dir).map(|available| available >= expected_bytes)
}

fn cleanup_stale_in_directory(directory: &Path, stale_after: Duration) -> CleanupReport {
    let mut report = CleanupReport::default();
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => {
            report.failed += 1;
            return report;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                report.failed += 1;
                continue;
            }
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !is_packetboat_transfer_temp_name(name) {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) => continue,
            Err(_) => {
                report.failed += 1;
                continue;
            }
        };
        let stale = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= stale_after);
        if !stale {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => report.removed += 1,
            Err(_) => report.failed += 1,
        }
    }
    report
}

fn is_packetboat_transfer_temp_name(name: &str) -> bool {
    if let Some(body) = name
        .strip_prefix(".packetboat-zip-")
        .and_then(|value| value.strip_suffix(".zip"))
    {
        return valid_pid_and_suffix(body, false);
    }
    if let Some(body) = name
        .strip_prefix(".packetboat-")
        .and_then(|value| value.strip_suffix(".part"))
    {
        return valid_pid_and_suffix(body, true);
    }
    false
}

fn valid_pid_and_suffix(body: &str, fixed_suffix: bool) -> bool {
    let Some((pid, suffix)) = body.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && !suffix.is_empty()
        && (!fixed_suffix || suffix.len() == 16)
        && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 单文件超过大小上限的错误。
#[derive(Debug, Clone, Copy)]
pub struct FileTooLarge;

impl std::fmt::Display for FileTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file exceeds the configured size limit")
    }
}

impl std::error::Error for FileTooLarge {}

/// 净化上传文件名（与 Go 版 `sanitizeFilename` 行为一致）：
/// - `\` 归一化为 `/`，只取最后一个路径段；
/// - 控制字符与 `<>:"/\|?*` 替换为 `_`；
/// - 首尾空格与点号去除；
/// - 为空 / `.` / `..` 视为无效；
/// - 超过 180 个字符时截断主干（扩展名最多保留 30 字符）。
pub fn sanitize_filename(raw: &str) -> String {
    let normalized = raw.replace('\\', "/");
    let name = normalized.rsplit('/').next().unwrap_or("");
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c| c == ' ' || c == '.').to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return String::new();
    }

    let runes: Vec<char> = cleaned.chars().collect();
    if runes.len() <= 180 {
        return cleaned;
    }

    // 截断：主干最多 180 - 扩展名长度，扩展名最多 30 字符
    let dot = cleaned.rfind('.');
    let (stem, extension) = match dot {
        Some(pos) => (&cleaned[..pos], &cleaned[pos..]),
        None => (&cleaned[..], ""),
    };
    let ext_runes: Vec<char> = extension.chars().take(30).collect();
    let max_stem = 180usize.saturating_sub(ext_runes.len()).max(1);
    let stem_runes: Vec<char> = stem.chars().take(max_stem).collect();
    let mut result: String = stem_runes.into_iter().collect();
    result = result.trim_matches(|c| c == ' ' || c == '.').to_string();
    result.push_str(&ext_runes.into_iter().collect::<String>());
    result
}

/// 生成 `Content-Disposition` 响应头值。非 ASCII 文件名按 RFC 5987 编码为 `filename*`，
/// 行为与 Go 版 `mime.FormatMediaType("attachment", ...)` 一致。
pub fn content_disposition(name: &str) -> String {
    if name.is_ascii() {
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        return format!("attachment; filename=\"{escaped}\"");
    }
    let encoded = rfc5987_encode(name);
    format!("attachment; filename*=UTF-8''{encoded}")
}

/// RFC 5987 百分号编码（UTF-8 字节；保留 `A-Za-z0-9` 与 `!#$&+-.^_`|~`）。
fn rfc5987_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'&'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~' => out.push(*byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 在接收目录内生成不冲突的目标路径：`name`，已存在则 `stem (1).ext`、`stem (2).ext`…
pub fn unique_destination(storage_dir: &Path, name: &str) -> PathBuf {
    let candidate = storage_dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let extension = Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let stem = name
        .strip_suffix(&extension)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name);
    for index in 1.. {
        let candidate = storage_dir.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// 解码 RFC 2047 encoded-word 文件名（`.NET HttpClient` 等客户端上传中文名时会发
/// `=?utf-8?B?...?=` 形式，Go 原版 net/http 会自动解码；这里对齐该行为）。
/// 仅当整个字符串是单个 encoded-word 时解码，否则原样返回。
pub fn decode_rfc2047_filename(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(token) = trimmed
        .strip_prefix("=?")
        .and_then(|s| s.strip_suffix("?="))
    else {
        return raw.to_string();
    };
    let mut parts = token.splitn(3, '?');
    let _charset = parts.next();
    let encoding = parts.next();
    let payload = parts.next();
    let (Some(encoding), Some(payload)) = (encoding, payload) else {
        return raw.to_string();
    };
    match encoding.to_ascii_uppercase().as_str() {
        "B" => {
            use base64::Engine;
            match base64::engine::general_purpose::STANDARD.decode(payload) {
                Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| raw.to_string()),
                Err(_) => raw.to_string(),
            }
        }
        "Q" => decode_q_encoding(payload).unwrap_or_else(|| raw.to_string()),
        _ => raw.to_string(),
    }
}

/// RFC 2047 Q 编码解码：`_` → 空格，`=XX` → 十六进制字节。
fn decode_q_encoding(payload: &str) -> Option<String> {
    let mut bytes: Vec<u8> = Vec::with_capacity(payload.len());
    let chars: Vec<char> = payload.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '_' => bytes.push(b' '),
            '=' => {
                if index + 2 >= chars.len() {
                    return None;
                }
                let hex: String = chars[index + 1..index + 3].iter().collect();
                bytes.push(u8::from_str_radix(&hex, 16).ok()?);
                index += 2;
            }
            c if c.is_ascii() => bytes.push(c as u8),
            _ => return None,
        }
        index += 1;
    }
    String::from_utf8(bytes).ok()
}

/// 流式保存上传文件：先写入同目录临时文件（`.packetboat-*.part`），fsync 后
/// 在 `file_mu` 保护下原子重命名为唯一目标路径，最后登记到目录表。
/// 任何一步失败都会清理临时文件。
pub async fn save_upload(
    catalog: &Catalog,
    storage_dir: &Path,
    max_upload_bytes: i64,
    reader: impl tokio::io::AsyncRead + Unpin,
    original_name: &str,
    file_mu: &tokio::sync::Mutex<()>,
) -> Result<Item, SaveUploadError> {
    let name = sanitize_filename(&decode_rfc2047_filename(original_name));
    if name.is_empty() {
        return Err(SaveUploadError::InvalidName);
    }

    let temp_path = storage_dir.join(format!(
        ".packetboat-{}-{}.part",
        std::process::id(),
        random_hex(8)
    ));

    if let Err(err) = write_temp(&temp_path, reader, max_upload_bytes).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(err);
    }

    let destination = {
        let _guard = file_mu.lock().await;
        let destination = unique_destination(storage_dir, &name);
        if let Err(err) = tokio::fs::rename(&temp_path, &destination).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(map_storage_io(&format!("保存文件到 {destination:?}"), err));
        }
        destination
    };

    match catalog.add_received(destination.clone()) {
        Ok(item) => Ok(item),
        Err(err) => {
            // catalog 写入失败时尽量把文件退回原会话，使客户端可以重试或取消。
            // 只有回退重命名也失败时才删除未登记的可见文件。
            if tokio::fs::rename(&destination, temp_path).await.is_err() {
                let _ = tokio::fs::remove_file(&destination).await;
            }
            Err(SaveUploadError::Catalog(err))
        }
    }
}

/// 将已经完整写入并同步的临时文件原子提交到接收目录，并登记到目录表。
/// 分块上传与传统 multipart 上传共用同一套唯一命名及失败清理语义。
pub(crate) async fn commit_temporary_upload(
    catalog: &Catalog,
    storage_dir: &Path,
    temp_path: &Path,
    name: &str,
    file_mu: &tokio::sync::Mutex<()>,
) -> Result<Item, SaveUploadError> {
    let destination = {
        let _guard = file_mu.lock().await;
        let destination = unique_destination(storage_dir, name);
        if let Err(err) = tokio::fs::rename(temp_path, &destination).await {
            return Err(map_storage_io(&format!("保存文件到 {destination:?}"), err));
        }
        destination
    };

    match catalog.add_received(destination.clone()) {
        Ok(item) => Ok(item),
        Err(err) => {
            // catalog 写入失败时把文件退回临时路径，与 save_upload 保持一致的
            // 回滚语义：会话保持 active，客户端重试 complete 时临时文件仍存在，
            // 已上传的数据不会丢失。只有回退重命名也失败时才删除未登记文件。
            if tokio::fs::rename(&destination, temp_path).await.is_err() {
                let _ = tokio::fs::remove_file(&destination).await;
            }
            Err(SaveUploadError::Catalog(err))
        }
    }
}

/// 写入临时文件：限制读取长度（超出上限判定为超大文件），完成后 fsync。
async fn write_temp(
    path: &Path,
    reader: impl tokio::io::AsyncRead + Unpin,
    max_upload_bytes: i64,
) -> Result<u64, SaveUploadError> {
    let temporary = tokio::fs::File::create(path)
        .await
        .map_err(|err| map_storage_io("创建临时文件", err))?;
    let mut temporary = AsyncBufWriter::with_capacity(TRANSFER_BUFFER_SIZE, temporary);
    let mut limited = reader.take(max_upload_bytes.saturating_add(1).max(0) as u64);
    let written = tokio::io::copy(&mut limited, &mut temporary)
        .await
        .map_err(|err| map_storage_io("写入上传文件", err))?;
    if written > max_upload_bytes as u64 {
        return Err(SaveUploadError::TooLarge);
    }
    temporary
        .flush()
        .await
        .map_err(|err| map_storage_io("刷新上传文件", err))?;
    temporary
        .get_ref()
        .sync_all()
        .await
        .map_err(|err| map_storage_io("同步文件", err))?;
    Ok(written)
}

fn map_storage_io(context: &str, err: std::io::Error) -> SaveUploadError {
    // Unix/macOS ENOSPC=28；Windows ERROR_HANDLE_DISK_FULL=39、ERROR_DISK_FULL=112。
    if matches!(err.raw_os_error(), Some(28 | 39 | 112)) {
        SaveUploadError::InsufficientStorage
    } else {
        SaveUploadError::Io(format!("{context}: {err}"))
    }
}

pub(crate) fn random_hex(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 递归打包文件夹为 zip 文件。zip 内以文件夹名作为顶层目录（解压得到同名文件夹），
/// 相对路径使用 `/` 分隔，符号链接跳过（防循环），空文件夹写入目录条目。
/// 此函数为同步阻塞操作，应在 `spawn_blocking` 中调用。
pub fn build_folder_zip(source_dir: &Path, zip_path: &Path) -> Result<(), String> {
    let folder_name = source_dir
        .file_name()
        .ok_or_else(|| "文件夹名称无效".to_string())?
        .to_string_lossy()
        .into_owned();
    let file = File::create(zip_path).map_err(|err| format!("创建 zip 文件: {err}"))?;
    let mut writer = zip::ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    write_dir_to_zip(&mut writer, source_dir, &folder_name, &options)
        .map_err(|err| err.to_string())?;
    writer
        .finish()
        .map_err(|err| format!("完成 zip 写入: {err}"))?;
    Ok(())
}

fn write_dir_to_zip(
    writer: &mut zip::ZipWriter<BufWriter<File>>,
    dir: &Path,
    prefix: &str,
    options: &SimpleFileOptions,
) -> std::io::Result<()> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue; // 跳过符号链接，避免循环与越界
        }
        if file_type.is_dir() {
            // 目录条目（以 / 结尾）+ 递归
            writer.add_directory(relative.clone(), *options)?;
            write_dir_to_zip(writer, &entry.path(), &relative, options)?;
        } else if file_type.is_file() {
            writer.start_file(relative, *options)?;
            let mut file = File::open(entry.path())?;
            std::io::copy(&mut file, writer)?;
        }
    }
    Ok(())
}

/// 上传保存过程中的错误分类。
#[derive(Debug)]
pub enum SaveUploadError {
    InvalidName,
    TooLarge,
    InsufficientStorage,
    Io(String),
    Catalog(String),
}

impl SaveUploadError {
    pub fn is_too_large(&self) -> bool {
        matches!(self, SaveUploadError::TooLarge)
    }
}

impl std::fmt::Display for SaveUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveUploadError::InvalidName => write!(f, "文件名无效"),
            SaveUploadError::TooLarge => write!(f, "文件超过大小限制"),
            SaveUploadError::InsufficientStorage => write!(f, "接收目录可用空间不足"),
            SaveUploadError::Io(message) => write!(f, "{message}"),
            SaveUploadError::Catalog(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_cases() {
        let cases = [
            ("../report.pdf", "report.pdf"),
            (r"..\..\photo?.jpg", "photo_.jpg"),
            ("  notes.txt.  ", "notes.txt"),
            ("控制\u{0}字符.txt", "控制_字符.txt"),
            ("...", ""),
        ];
        for (input, want) in cases {
            assert_eq!(sanitize_filename(input), want, "input: {input:?}");
        }
    }

    #[test]
    fn sanitize_long_name_truncates() {
        let long = format!("{}_tail.txt", "a".repeat(200));
        let result = sanitize_filename(&long);
        assert_eq!(result.chars().count(), 180);
        assert!(result.ends_with(".txt"));
    }

    #[test]
    fn content_disposition_ascii_and_utf8() {
        assert_eq!(
            content_disposition("hello.txt"),
            "attachment; filename=\"hello.txt\""
        );
        let cd = content_disposition("测试.txt");
        assert!(cd.starts_with("attachment; filename*=UTF-8''"));
        assert!(cd.contains("%E6%B5%8B%E8%AF%95"));
    }

    #[test]
    fn decode_rfc2047_filenames() {
        // B 编码：=?utf-8?B?5pa55qGI?= = "方案"（UTF-8 E6 96 B9 E6 A1 88）
        let decoded = decode_rfc2047_filename("=?utf-8?B?5pa55qGI?=");
        assert_eq!(decoded, "方案");
        // 混合：前缀 + encoded-word + 后缀（仅整体匹配时解码 —— 此处返回原样）
        let raw = "报告_=?utf-8?B?5pa55qGI?=_.txt";
        assert_eq!(decode_rfc2047_filename(raw), raw.to_string());
        // Q 编码：=?utf-8?Q?=E6=96=B9=E6=A1=88?= = "方案"
        let decoded = decode_rfc2047_filename("=?utf-8?Q?=E6=96=B9=E6=A1=88?=");
        assert_eq!(decoded, "方案");
        // 非法 base64 → 原样
        let raw = "=?utf-8?B?!!!?=";
        assert_eq!(decode_rfc2047_filename(raw), raw.to_string());
        // 普通文件名 → 原样
        let raw = "hello.txt";
        assert_eq!(decode_rfc2047_filename(raw), raw.to_string());
        // 净化链路：RFC2047 文件名（整体即 encoded-word）经净化后得到真实名称
        let sanitized = sanitize_filename(&decode_rfc2047_filename("=?utf-8?B?5pa55qGI?="));
        assert_eq!(sanitized, "方案");
    }

    #[test]
    fn unique_destination_numbering() {
        let dir = std::env::temp_dir().join(format!("packetboat-unique-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hello.txt"), b"first").unwrap();
        assert_eq!(
            unique_destination(&dir, "hello.txt"),
            dir.join("hello (1).txt")
        );
        std::fs::write(dir.join("hello (1).txt"), b"second").unwrap();
        assert_eq!(
            unique_destination(&dir, "hello.txt"),
            dir.join("hello (2).txt")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn transfer_temp_names_are_matched_strictly() {
        assert!(is_packetboat_transfer_temp_name(
            ".packetboat-1234-0123456789abcdef.part"
        ));
        assert!(is_packetboat_transfer_temp_name(
            ".packetboat-zip-1234-18d7a0ff.zip"
        ));
        assert!(!is_packetboat_transfer_temp_name(
            ".packetboat-not-a-temp.part"
        ));
        assert!(!is_packetboat_transfer_temp_name(
            ".packetboat-1234-0123456789abcdef.txt"
        ));
        assert!(!is_packetboat_transfer_temp_name("notes.zip"));
    }

    #[test]
    fn stale_transfer_cleanup_preserves_unrelated_files() {
        let dir = std::env::temp_dir().join(format!("packetboat-cleanup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let upload = dir.join(".packetboat-1234-0123456789abcdef.part");
        let archive = dir.join(".packetboat-zip-1234-18d7a0ff.zip");
        let unrelated = dir.join(".packetboat-user-notes.part");
        std::fs::write(&upload, b"partial").unwrap();
        std::fs::write(&archive, b"zip").unwrap();
        std::fs::write(&unrelated, b"keep").unwrap();

        let report = cleanup_stale_in_directory(&dir, Duration::ZERO);
        assert_eq!(
            report,
            CleanupReport {
                removed: 2,
                failed: 0
            }
        );
        assert!(!upload.exists());
        assert!(!archive.exists());
        assert!(unrelated.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upload_capacity_accepts_empty_request() {
        assert!(has_upload_capacity(&std::env::temp_dir(), 0).unwrap());
    }

    #[test]
    fn disk_full_errors_have_a_distinct_category() {
        for code in [28, 39, 112] {
            let error = map_storage_io("write", std::io::Error::from_raw_os_error(code));
            assert!(matches!(error, SaveUploadError::InsufficientStorage));
        }
        let error = map_storage_io("write", std::io::Error::other("x"));
        assert!(matches!(error, SaveUploadError::Io(_)));
    }
}
