//! 内嵌前端静态资源服务：Vite 构建产物经 rust-embed 嵌入。
//! 复刻 Go 版行为：`.gz` 预压缩变体、哈希资源不可变缓存、SPA 回退到 `index.html`。

use axum::body::Body;
use axum::http::{header, HeaderValue, Response, StatusCode};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/dist/"]
struct Dist;

/// 静态资源响应（含 HEAD 支持：Go 的 FileServer 语义下 HEAD 返回相同响应头）。
pub fn static_response(
    path: &str,
    accept_encoding: Option<&str>,
    head_only: bool,
) -> Response<Body> {
    let requested = path_clean(path);
    let requested = requested.trim_start_matches('/');

    if requested != "." && !requested.is_empty() {
        if let Some(content) = Dist::get(requested) {
            return serve_asset(requested, content.data.as_ref(), accept_encoding, head_only);
        }
    }

    // SPA 回退：始终返回 index.html
    match Dist::get("index.html") {
        Some(content) => {
            let mut response = Response::new(Body::from(content.data.into_owned()));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            );
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            if head_only {
                *response.body_mut() = Body::empty();
            }
            response
        }
        None => json_error(StatusCode::INTERNAL_SERVER_ERROR, "前端资源不可用"),
    }
}

fn serve_asset(
    requested: &str,
    content: &[u8],
    accept_encoding: Option<&str>,
    head_only: bool,
) -> Response<Body> {
    let mut response = Response::new(Body::empty());

    // 哈希资源：长期不可变缓存
    if is_hashed_asset(requested) {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }

    let gzip_path = format!("{requested}.gz");
    let gzip_available = Dist::get(&gzip_path).is_some();
    let accepts_gzip = accept_encoding
        .map(|value| accepts_encoding(value, "gzip"))
        .unwrap_or(false);

    if gzip_available && accepts_gzip {
        if let Some(compressed) = Dist::get(&gzip_path) {
            response
                .headers_mut()
                .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
            response
                .headers_mut()
                .append(header::VARY, HeaderValue::from_static("Accept-Encoding"));
            set_content_type(&mut response, requested);
            *response.body_mut() = Body::from(compressed.data.into_owned());
            return finish_asset(response, head_only);
        }
    }

    if gzip_available {
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    set_content_type(&mut response, requested);
    *response.body_mut() = Body::from(content.to_vec());
    finish_asset(response, head_only)
}

fn finish_asset(mut response: Response<Body>, head_only: bool) -> Response<Body> {
    if head_only {
        *response.body_mut() = Body::empty();
    }
    response
}

fn set_content_type(response: &mut Response<Body>, requested: &str) {
    if let Some(mime) = mime_guess::from_path(requested).first() {
        let value = mime.as_ref().to_string();
        let value = if value.starts_with("text/") && !value.contains("charset") {
            format!("{value}; charset=utf-8")
        } else {
            value
        };
        if let Ok(header_value) = HeaderValue::from_str(&value) {
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, header_value);
        }
    }
}

/// 判断是否为带内容哈希的构建产物。
///
/// Vite 使用 URL-safe base64 风格哈希，其中可能包含 `-`（例如 `C-FYbMVH`），
/// 因此不能只取文件名最后一个 `-` 后的片段。
fn is_hashed_asset(requested: &str) -> bool {
    if !requested.starts_with("assets/") {
        return false;
    }
    let base = requested.rsplit('/').next().unwrap_or(requested);
    let dot = base.rfind('.');
    let stem = match dot {
        Some(pos) => &base[..pos],
        None => base,
    };
    let Some((_, hash)) = stem.split_once('-') else {
        return false;
    };
    hash.len() >= 8
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        && hash
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

/// 解析 Accept-Encoding 的 q 值并判断是否接受指定编码（与 Go 版一致）。
fn accepts_encoding(header_value: &str, wanted: &str) -> bool {
    let mut wildcard_accepted = false;
    for value in header_value.split(',') {
        let mut parts = value.split(';');
        let name = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        let mut quality: f64 = 1.0;
        for parameter in parts {
            let parameter = parameter.trim();
            if let Some(q) = parameter.strip_prefix("q=") {
                quality = q.trim().parse::<f64>().unwrap_or(0.0);
            }
        }
        if quality <= 0.0 {
            continue;
        }
        if name == wanted {
            return true;
        }
        if name == "*" {
            wildcard_accepted = true;
        }
    }
    wildcard_accepted
}

/// 词法路径清理（等价于 Go 的 `path.Clean`）。
fn path_clean(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            segment => stack.push(segment),
        }
    }
    let cleaned = stack.join("/");
    if cleaned.is_empty() {
        ".".to_string()
    } else {
        cleaned
    }
}

pub(crate) fn json_error(status: StatusCode, message: &str) -> Response<Body> {
    let body = serde_json::json!({ "error": message });
    let body = serde_json::to_vec(&body).unwrap_or_default();
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_asset_detection() {
        assert!(is_hashed_asset("assets/index-Cx7t2389.js"));
        assert!(is_hashed_asset("assets/index-C-FYbMVH.js"));
        assert!(is_hashed_asset("assets/app-12345678.js"));
        assert!(!is_hashed_asset("assets/plain.js"));
        assert!(!is_hashed_asset("assets/some-long-name.js"));
        assert!(!is_hashed_asset("index.html"));
        assert!(!is_hashed_asset("assets/short-1.js"));
    }

    #[test]
    fn accepts_encoding_parsing() {
        assert!(accepts_encoding("br, gzip", "gzip"));
        assert!(!accepts_encoding("gzip;q=0", "gzip"));
        assert!(accepts_encoding("*", "gzip"));
        assert!(!accepts_encoding("br;q=0.5", "gzip"));
        assert!(accepts_encoding("gzip;q=0.5, br;q=1", "gzip"));
    }

    #[test]
    fn path_clean_basics() {
        assert_eq!(path_clean("/assets/../index.html"), "index.html");
        assert_eq!(path_clean("/foo/bar"), "foo/bar");
        assert_eq!(path_clean("/"), ".");
        assert_eq!(path_clean(""), ".");
    }
}
