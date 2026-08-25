//! HTTP 服务器：axum 路由、认证、上传下载、SSE、静态资源与中间件。
//! 路由与响应语义逐一对齐 Go 版 `internal/server`。

use crate::assets;
use crate::auth::{AuthManager, SESSION_COOKIE_NAME};
use crate::catalog::{Catalog, Item, SourceType};
use crate::files::{save_upload, SaveUploadError};
use crate::hub::Hub;
use crate::uploads::{UploadError, UploadManager, MAX_CHUNK_SIZE};
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{DefaultBodyLimit, Json, Path, Request, State};
use axum::http::header::{self, HeaderValue};
use axum::http::{Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Response, Sse};
use axum::routing::{delete, get, put};
use axum::Router;
use chrono::{DateTime, Utc};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tower_http::catch_panic::CatchPanicLayer;
use tracing::{info, warn};

/// 服务器配置。
pub struct Config {
    pub device_name: String,
    pub storage_dir: PathBuf,
    pub max_upload_bytes: i64,
    pub version: String,
    pub catalog: Option<Arc<Catalog>>,
    /// 外部注入的传输进度跟踪器（桌面端任务栏进度）；缺省时内部创建。
    pub progress: Option<Arc<crate::progress::ProgressTracker>>,
}

/// 服务器状态（全部字段为共享引用，可廉价克隆）。
#[derive(Clone)]
pub struct Server {
    device_name: String,
    storage_dir: PathBuf,
    max_upload_bytes: i64,
    version: String,
    catalog: Arc<Catalog>,
    auth: Arc<AuthManager>,
    hub: Arc<Hub>,
    /// 序列化上传文件的重命名与删除（与 Go 版 `fileMu` 一致）。
    file_mu: Arc<tokio::sync::Mutex<()>>,
    /// 传输进度（上传/下载）跟踪。
    progress: Arc<crate::progress::ProgressTracker>,
    /// 分块上传会话及临时文件状态。
    uploads: Arc<UploadManager>,
}

impl Server {
    pub fn new(config: Config) -> Result<Server, String> {
        let device_name = config.device_name.trim().to_string();
        if device_name.is_empty() {
            return Err("设备名称不能为空".to_string());
        }
        if config.max_upload_bytes <= 0 {
            return Err("上传大小限制必须大于 0".to_string());
        }
        let storage_dir = std::path::absolute(&config.storage_dir)
            .map_err(|err| format!("解析保存目录: {err}"))?;
        std::fs::create_dir_all(&storage_dir).map_err(|err| format!("创建保存目录: {err}"))?;

        let cleanup = crate::files::cleanup_stale_transfer_files(&storage_dir);
        if cleanup.removed > 0 {
            info!(removed = cleanup.removed, "已清理残留传输临时文件");
        }
        if cleanup.failed > 0 {
            warn!(failed = cleanup.failed, "部分残留传输临时文件清理失败");
        }

        let catalog = match config.catalog {
            Some(catalog) => catalog,
            None => Arc::new(
                Catalog::open(PathBuf::new()).map_err(|err| format!("初始化共享目录表: {err}"))?,
            ),
        };
        let max_upload_bytes = config.max_upload_bytes;
        let uploads = Arc::new(UploadManager::new(
            storage_dir.clone(),
            max_upload_bytes as u64,
        ));
        Ok(Server {
            device_name,
            storage_dir,
            max_upload_bytes,
            version: config.version,
            catalog,
            auth: Arc::new(AuthManager::new()),
            hub: Arc::new(Hub::new()),
            file_mu: Arc::new(tokio::sync::Mutex::new(())),
            progress: config.progress.unwrap_or_default(),
            uploads,
        })
    }

    /// 传输进度跟踪器（桌面端任务栏进度读取）。
    pub fn progress(&self) -> Arc<crate::progress::ProgressTracker> {
        Arc::clone(&self.progress)
    }

    /// 当前配对码。
    pub fn access_code(&self) -> String {
        self.auth.code().to_string()
    }

    /// 构建 axum 路由（含中间件）。
    pub fn router(&self) -> Router {
        let server = Arc::new(self.clone());
        let upload_body_limit = self.max_upload_bytes.saturating_add(1024 * 1024) as usize;

        let public = Router::new()
            .route("/api/info", get(handle_info))
            .route(
                "/api/session",
                get(handle_session_status)
                    .post(handle_pair)
                    .delete(handle_sign_out),
            )
            .route_layer(DefaultBodyLimit::max(4096));

        let protected = Router::new()
            .route("/api/files", get(handle_file_list).post(handle_upload))
            .route("/api/files/{id}/download", get(handle_download))
            .route("/api/files/{id}", delete(handle_delete))
            .route(
                "/api/uploads",
                axum::routing::post(handle_create_chunk_upload),
            )
            .route(
                "/api/uploads/{id}",
                get(handle_chunk_upload_status).delete(handle_cancel_chunk_upload),
            )
            .route("/api/uploads/{id}/chunks/{index}", put(handle_upload_chunk))
            .route(
                "/api/uploads/{id}/complete",
                axum::routing::post(handle_complete_chunk_upload),
            )
            .route("/api/events", get(handle_events))
            .route_layer(DefaultBodyLimit::max(upload_body_limit))
            .layer(middleware::from_fn_with_state(server.clone(), require_auth));

        Router::new()
            .merge(public)
            .merge(protected)
            .fallback(static_fallback)
            .with_state(server.clone())
            .layer(middleware::from_fn(security_headers))
            .layer(CatchPanicLayer::new())
            .layer(middleware::from_fn_with_state(
                server.clone(),
                request_logger,
            ))
    }
}

// ---------------------------------------------------------------------------
// 请求/响应辅助
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairPayload {
    code: String,
}

#[derive(Serialize)]
struct InfoPayload<'a> {
    device_name: &'a str,
    requires_auth: bool,
    max_upload_bytes: i64,
    version: &'a str,
}

#[derive(Serialize)]
struct SessionPayload {
    authenticated: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUploadPayload {
    file_name: String,
    file_size: u64,
    #[serde(default)]
    chunk_size: Option<u64>,
}

/// 浏览器端文件记录（不含 local_path）。
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct FileRecord {
    id: String,
    name: String,
    size: i64,
    modified: DateTime<Utc>,
    source_type: SourceType,
    available: bool,
    /// 文件夹条目（下载时打包为 zip）。
    #[serde(default)]
    is_dir: bool,
}

fn record_from_item(item: &Item) -> FileRecord {
    FileRecord {
        id: item.id.clone(),
        name: item.name.clone(),
        size: item.size,
        modified: item.modified_at,
        source_type: item.source_type,
        available: item.available,
        is_dir: item.is_dir,
    }
}

fn json_response(status: StatusCode, payload: impl Serialize) -> Response {
    let body = serde_json::to_vec(&payload).unwrap_or_default();
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
}

fn json_error(status: StatusCode, message: &str) -> Response {
    assets::json_error(status, message)
}

fn no_content() -> Response {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .expect("static response")
}

fn session_token_from(request: &Request) -> Option<String> {
    let cookie = request.headers().get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE_NAME}=")) {
            return Some(value.to_string());
        }
    }
    None
}

fn set_session_cookie(token: &str, expires: DateTime<Utc>) -> HeaderValue {
    let max_age = (expires - Utc::now()).num_seconds().max(1);
    let http_date = httpdate::fmt_http_date(SystemTime::from(expires));
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}; Expires={http_date}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("lane_session=; Path=/; HttpOnly"))
}

fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
    ))
    .expect("valid cookie header")
}

// ---------------------------------------------------------------------------
// 公开端点
// ---------------------------------------------------------------------------

async fn handle_info(State(server): State<Arc<Server>>) -> Response {
    json_response(
        StatusCode::OK,
        InfoPayload {
            device_name: &server.device_name,
            requires_auth: true,
            max_upload_bytes: server.max_upload_bytes,
            version: &server.version,
        },
    )
}

async fn handle_session_status(State(server): State<Arc<Server>>, request: Request) -> Response {
    let authenticated = server
        .auth
        .authenticated(session_token_from(&request).as_deref());
    json_response(StatusCode::OK, SessionPayload { authenticated })
}

async fn handle_pair(
    State(server): State<Arc<Server>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Result<axum::body::Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let body = match body {
        Ok(body) => body,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };
    let payload: PairPayload = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };

    let ip = addr.ip().to_string();
    match server.auth.pair(&ip, payload.code.trim()) {
        Ok((token, expires)) => {
            let mut response = no_content();
            response
                .headers_mut()
                .insert(header::SET_COOKIE, set_session_cookie(&token, expires));
            response
        }
        Err(message) => json_error(StatusCode::UNAUTHORIZED, &message),
    }
}

async fn handle_sign_out(State(server): State<Arc<Server>>, request: Request) -> Response {
    server
        .auth
        .sign_out(session_token_from(&request).as_deref());
    let mut response = no_content();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clear_session_cookie());
    response
}

// ---------------------------------------------------------------------------
// 受保护端点
// ---------------------------------------------------------------------------

async fn handle_file_list(State(server): State<Arc<Server>>) -> Response {
    let items = server.catalog.list();
    let mut records: Vec<FileRecord> = items.iter().map(record_from_item).collect();
    records.sort_by_key(|record| std::cmp::Reverse(record.modified));
    json_response(StatusCode::OK, serde_json::json!({ "files": records }))
}

async fn handle_create_chunk_upload(
    State(server): State<Arc<Server>>,
    Json(payload): Json<CreateUploadPayload>,
) -> Response {
    match server
        .uploads
        .create(&payload.file_name, payload.file_size, payload.chunk_size)
        .await
    {
        Ok(status) => {
            server.progress.begin(status.file_size);
            json_response(StatusCode::CREATED, status)
        }
        Err(error) => upload_error_response(error),
    }
}

async fn handle_chunk_upload_status(
    State(server): State<Arc<Server>>,
    Path(id): Path<String>,
) -> Response {
    match server.uploads.status(&id).await {
        Ok(status) => json_response(StatusCode::OK, status),
        Err(error) => upload_error_response(error),
    }
}

async fn handle_upload_chunk(
    State(server): State<Arc<Server>>,
    Path((id, index)): Path<(String, u32)>,
    request: Request,
) -> Response {
    let checksum = match request
        .headers()
        .get("x-chunk-sha256")
        .and_then(|value| value.to_str().ok())
    {
        Some(checksum) => checksum.to_string(),
        None => return json_error(StatusCode::BAD_REQUEST, "缺少 X-Chunk-SHA256 请求头"),
    };
    let bytes = match axum::body::to_bytes(request.into_body(), MAX_CHUNK_SIZE as usize).await {
        Ok(bytes) => bytes,
        Err(_) => return json_error(StatusCode::PAYLOAD_TOO_LARGE, "分块超过大小限制"),
    };
    match server
        .uploads
        .write_chunk(&id, index, &checksum, &bytes)
        .await
    {
        Ok(result) => {
            if result.newly_received {
                server.progress.add(bytes.len() as u64);
            }
            json_response(StatusCode::OK, result.status)
        }
        Err(error) => upload_error_response(error),
    }
}

async fn handle_complete_chunk_upload(
    State(server): State<Arc<Server>>,
    Path(id): Path<String>,
) -> Response {
    match server
        .uploads
        .complete(&id, &server.catalog, &server.file_mu)
        .await
    {
        Ok(item) => {
            server.progress.finish();
            server
                .hub
                .publish(r#"{"type":"files_changed"}"#.to_string());
            json_response(
                StatusCode::CREATED,
                serde_json::json!({ "files": [record_from_item(&item)] }),
            )
        }
        Err(error) => upload_error_response(error),
    }
}

async fn handle_cancel_chunk_upload(
    State(server): State<Arc<Server>>,
    Path(id): Path<String>,
) -> Response {
    match server.uploads.cancel(&id).await {
        Ok(()) => {
            server.progress.finish();
            no_content()
        }
        Err(error) => upload_error_response(error),
    }
}

fn upload_error_response(error: UploadError) -> Response {
    let status = match &error {
        UploadError::InvalidName | UploadError::InvalidChecksum | UploadError::InvalidChunk(_) => {
            StatusCode::BAD_REQUEST
        }
        UploadError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        UploadError::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
        UploadError::NotFound => StatusCode::NOT_FOUND,
        UploadError::ChecksumMismatch => StatusCode::UNPROCESSABLE_ENTITY,
        UploadError::ChunkConflict | UploadError::Incomplete | UploadError::AlreadyCompleted => {
            StatusCode::CONFLICT
        }
        UploadError::Commit(SaveUploadError::TooLarge) => StatusCode::PAYLOAD_TOO_LARGE,
        UploadError::Commit(SaveUploadError::InsufficientStorage) => {
            StatusCode::INSUFFICIENT_STORAGE
        }
        UploadError::Commit(SaveUploadError::InvalidName) => StatusCode::BAD_REQUEST,
        UploadError::Io(_) | UploadError::Commit(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    if status.is_server_error() {
        warn!("分块上传失败: {error}");
    }
    json_error(status, &error.to_string())
}

async fn handle_upload(State(server): State<Arc<Server>>, request: Request) -> Response {
    // 手动构造 multipart（multer）：需要统计 body 字节以驱动任务栏进度
    let content_type = match request.headers().get(header::CONTENT_TYPE) {
        Some(value) => value.to_str().unwrap_or(""),
        None => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "请使用 multipart/form-data 上传文件",
            )
        }
    };
    let boundary = match multer::parse_boundary(content_type) {
        Ok(boundary) => boundary,
        Err(_) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "请使用 multipart/form-data 上传文件",
            )
        }
    };
    let total = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    if total > 0 {
        match crate::files::has_upload_capacity(&server.storage_dir, total) {
            Ok(false) => {
                return json_error(StatusCode::INSUFFICIENT_STORAGE, "接收目录可用空间不足")
            }
            Ok(true) => {}
            Err(err) => warn!("查询接收目录可用空间失败，继续尝试上传: {err}"),
        }
    }
    server.progress.begin(total);

    // 包装 body 为计数流（统计已读取字节），直接喂给 multer 解析
    let counting = CountingStream {
        inner: Box::pin(request.into_body().into_data_stream()),
        tracker: Arc::clone(&server.progress),
    };
    let mut multipart = multer::Multipart::new(counting, boundary);

    let mut uploaded: Vec<FileRecord> = Vec::new();
    loop {
        if uploaded.len() >= 50 {
            break;
        }
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => {
                server.progress.finish();
                return json_error(StatusCode::BAD_REQUEST, "读取上传内容失败");
            }
        };
        let Some(file_name) = field.file_name().map(|name| name.to_string()) else {
            continue; // 表单字段而非文件
        };

        // multer Field 为 Stream，转为 AsyncRead 后流式落盘
        use futures_util::TryStreamExt;
        let reader = tokio_util::io::StreamReader::new(field.map_err(std::io::Error::other));

        let result = save_upload(
            &server.catalog,
            &server.storage_dir,
            server.max_upload_bytes,
            reader,
            &file_name,
            &server.file_mu,
        )
        .await;
        match result {
            Ok(item) => uploaded.push(record_from_item(&item)),
            Err(SaveUploadError::TooLarge) => {
                server.progress.finish();
                return json_error(StatusCode::PAYLOAD_TOO_LARGE, "文件超过大小限制");
            }
            Err(SaveUploadError::InsufficientStorage) => {
                server.progress.finish();
                return json_error(StatusCode::INSUFFICIENT_STORAGE, "接收目录可用空间不足");
            }
            Err(SaveUploadError::InvalidName) => {
                server.progress.finish();
                return json_error(StatusCode::BAD_REQUEST, "文件名无效");
            }
            Err(err) => {
                warn!("保存上传文件失败: {err}");
                server.progress.finish();
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "保存文件失败，请检查磁盘空间和目录权限",
                );
            }
        }
    }
    server.progress.finish();

    if uploaded.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "请求中没有文件");
    }
    server
        .hub
        .publish(r#"{"type":"files_changed"}"#.to_string());
    json_response(
        StatusCode::CREATED,
        serde_json::json!({ "files": uploaded }),
    )
}

/// 包装请求体流统计已读取字节数（上传进度）。
struct CountingStream {
    inner: std::pin::Pin<Box<dyn Stream<Item = Result<axum::body::Bytes, axum::Error>> + Send>>,
    tracker: Arc<crate::progress::ProgressTracker>,
}

impl futures_util::Stream for CountingStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(bytes))) => {
                self.tracker.add(bytes.len() as u64);
                std::task::Poll::Ready(Some(Ok(bytes)))
            }
            std::task::Poll::Ready(Some(Err(err))) => {
                std::task::Poll::Ready(Some(Err(std::io::Error::other(err))))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

async fn handle_download(
    State(server): State<Arc<Server>>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let (item, _metadata) = match server.catalog.resolve(&id) {
        Ok(resolved) => resolved,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "文件不存在"),
    };

    // 文件夹：实时打包为 zip 流式下载
    if item.is_dir {
        return zip_download(&item, request.method() == Method::HEAD).await;
    }

    // 提取所需请求头（避免跨 await 持有 &Request 导致 future 非 Send）
    let if_modified_since = request
        .headers()
        .get(header::IF_MODIFIED_SINCE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let range_header = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let head_only = request.method() == Method::HEAD;
    serve_file_download(server, &item, if_modified_since, range_header, head_only).await
}

/// 单文件下载：支持 Range 与 If-Modified-Since，并统计传输字节驱动任务栏进度。
async fn serve_file_download(
    server: Arc<Server>,
    item: &crate::catalog::Item,
    if_modified_since: Option<String>,
    range_header: Option<String>,
    head_only: bool,
) -> Response {
    let metadata = match std::fs::metadata(&item.local_path) {
        Ok(metadata) => metadata,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "文件不存在"),
    };
    let size = metadata.len();

    // If-Modified-Since → 304
    if let Some(value) = if_modified_since {
        if let Ok(since) = httpdate::parse_http_date(&value) {
            if let Ok(modified) = metadata.modified() {
                let last_modified = modified;
                let threshold = last_modified
                    .checked_sub(std::time::Duration::from_secs(1))
                    .unwrap_or(last_modified);
                if threshold <= since {
                    let mut response = Response::new(Body::empty());
                    *response.status_mut() = StatusCode::NOT_MODIFIED;
                    return response;
                }
            }
        }
    }

    // 空文件：直接返回 Content-Length: 0（避免 0 字节体声明为 1 导致客户端挂起）
    if size == 0 {
        let mut response = Response::new(Body::empty());
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            mime_guess::from_path(&item.local_path)
                .first_raw()
                .and_then(|mime| HeaderValue::from_str(mime).ok())
                .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream")),
        );
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from(0u64));
        response
            .headers_mut()
            .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        if let Ok(disposition) =
            HeaderValue::from_str(&crate::files::content_disposition(&item.name))
        {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, disposition);
        }
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
        return response;
    }

    // 解析 Range：语法非法忽略（全量 200）；语法合法但不可满足 → 416；多段 → 全量 200
    let range = match range_header.as_deref() {
        Some(value) => match http_range_header::parse_range_header(value) {
            Ok(parsed) => match parsed.validate(size) {
                Ok(ranges) if ranges.len() == 1 => Some((*ranges[0].start(), *ranges[0].end())),
                Ok(_) => None,
                Err(_) => {
                    let mut response = Response::new(Body::empty());
                    *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
                    response.headers_mut().insert(
                        header::CONTENT_RANGE,
                        HeaderValue::from_str(&format!("bytes */{size}"))
                            .expect("valid content-range"),
                    );
                    return response;
                }
            },
            Err(_) => None,
        },
        None => None,
    };

    let (start, end) = match range {
        Some((start, end)) => (start, end),
        None => (0, size.saturating_sub(1)),
    };
    let content_length = end.saturating_sub(start) + 1;

    server.progress.begin(content_length);
    let file = if head_only {
        None
    } else {
        match tokio::fs::File::open(&item.local_path).await {
            Ok(mut file) => {
                if start > 0 {
                    use tokio::io::AsyncSeekExt;
                    if let Err(err) = file.seek(std::io::SeekFrom::Start(start)).await {
                        server.progress.finish();
                        warn!("定位文件失败: {err}");
                        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "读取文件失败");
                    }
                }
                Some(file)
            }
            Err(_) => {
                server.progress.finish();
                return json_error(StatusCode::NOT_FOUND, "文件不存在");
            }
        }
    };

    let body = match file {
        Some(file) => {
            use tokio::io::AsyncReadExt;
            let stream = DownloadCountStream {
                inner: tokio_util::io::ReaderStream::with_capacity(
                    file.take(content_length),
                    crate::files::TRANSFER_BUFFER_SIZE,
                ),
                tracker: Arc::clone(&server.progress),
            };
            Body::from_stream(stream)
        }
        None => {
            server.progress.finish();
            Body::empty()
        }
    };

    let mut response = Response::new(body);
    if range.is_some() {
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        response.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
                .expect("valid content-range"),
        );
    }
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        mime_guess::from_path(&item.local_path)
            .first_raw()
            .and_then(|mime| HeaderValue::from_str(mime).ok())
            .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream")),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from(content_length));
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(disposition) = HeaderValue::from_str(&crate::files::content_disposition(&item.name)) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, disposition);
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    if let Ok(last_modified) = metadata.modified() {
        let date = httpdate::fmt_http_date(last_modified);
        if let Ok(value) = HeaderValue::from_str(&date) {
            response.headers_mut().insert(header::LAST_MODIFIED, value);
        }
    }
    response
}

/// 包装下载流统计已传输字节，流结束（或断开）时结束进度。
struct DownloadCountStream {
    inner: tokio_util::io::ReaderStream<tokio::io::Take<tokio::fs::File>>,
    tracker: Arc<crate::progress::ProgressTracker>,
}

impl futures_util::Stream for DownloadCountStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let poll = futures_util::StreamExt::poll_next_unpin(&mut self.inner, cx);
        if let std::task::Poll::Ready(Some(Ok(bytes))) = &poll {
            self.tracker.add(bytes.len() as u64);
        }
        if matches!(poll, std::task::Poll::Ready(None)) {
            self.tracker.finish();
        }
        poll
    }
}

impl Drop for DownloadCountStream {
    /// 客户端断开时流被 drop（不会走到 Ready(None)），此处兜底结束进度。
    fn drop(&mut self) {
        self.tracker.finish();
    }
}

/// 文件夹下载：后台线程打包为 zip，流式输出，响应体结束后自动删除临时文件。
async fn zip_download(item: &crate::catalog::Item, head_only: bool) -> Response {
    let zip_name = format!("{}.zip", item.name);

    // HEAD：不打包，仅返回头部（避免大目录白白压缩占用磁盘/CPU）
    let mut response = if head_only {
        Response::new(Body::empty())
    } else {
        let zip_path = std::env::temp_dir().join(format!(
            ".laneshare-zip-{}-{}.zip",
            std::process::id(),
            random_suffix()
        ));

        let source = item.local_path.clone();
        let destination = zip_path.clone();
        let build = tokio::task::spawn_blocking(move || {
            crate::files::build_folder_zip(std::path::Path::new(&source), &destination)
        })
        .await;
        match build {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let _ = std::fs::remove_file(&zip_path);
                warn!("打包文件夹失败: {err}");
                return json_error(StatusCode::INTERNAL_SERVER_ERROR, "打包文件夹失败，请重试");
            }
            Err(err) => {
                let _ = std::fs::remove_file(&zip_path);
                warn!("打包任务异常: {err}");
                return json_error(StatusCode::INTERNAL_SERVER_ERROR, "打包文件夹失败，请重试");
            }
        }

        let file = match tokio::fs::File::open(&zip_path).await {
            Ok(file) => file,
            Err(err) => {
                let _ = std::fs::remove_file(&zip_path);
                warn!("读取 zip 失败: {err}");
                return json_error(StatusCode::INTERNAL_SERVER_ERROR, "打包文件夹失败，请重试");
            }
        };
        let stream = ZipCleanupStream {
            inner: tokio_util::io::ReaderStream::with_capacity(
                file,
                crate::files::TRANSFER_BUFFER_SIZE,
            ),
            cleanup_path: zip_path,
        };
        Response::new(Body::from_stream(stream))
    };

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    if let Ok(disposition) = HeaderValue::from_str(&crate::files::content_disposition(&zip_name)) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, disposition);
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

/// 流式输出 zip 并在流结束（或客户端断开）时删除临时文件。
struct ZipCleanupStream {
    inner: tokio_util::io::ReaderStream<tokio::fs::File>,
    cleanup_path: PathBuf,
}

impl futures_util::Stream for ZipCleanupStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.get_mut().inner;
        futures_util::StreamExt::poll_next_unpin(inner, cx)
    }
}

impl Drop for ZipCleanupStream {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.cleanup_path);
    }
}

fn random_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

async fn handle_delete(State(server): State<Arc<Server>>, Path(id): Path<String>) -> Response {
    let item = match server.catalog.get(&id) {
        Some(item) => item,
        None => return json_error(StatusCode::NOT_FOUND, "文件不存在"),
    };

    if item.source_type == SourceType::Received {
        let _guard = server.file_mu.lock().await;
        if let Err(err) = std::fs::remove_file(&item.local_path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                return json_error(StatusCode::INTERNAL_SERVER_ERROR, "无法删除接收文件");
            }
        }
    }

    if let Err(err) = server.catalog.remove(&item.id) {
        warn!("无法更新共享目录: {err}");
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "无法更新共享目录");
    }
    server
        .hub
        .publish(r#"{"type":"files_changed"}"#.to_string());
    no_content()
}

async fn handle_events(
    State(server): State<Arc<Server>>,
) -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let (mut receiver, unsubscribe) = server.hub.subscribe();

    let stream = async_stream::stream! {
        // 客户端断开时（流被 drop）自动取消订阅
        let _guard = UnsubscribeGuard(Some(Box::new(unsubscribe)));
        yield Ok(axum::response::sse::Event::default()
            .retry(std::time::Duration::from_secs(3))
            .event("ready")
            .data("{}"));
        while let Some(event) = receiver.recv().await {
            yield Ok(axum::response::sse::Event::default()
                .event("update")
                .data(event));
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(20)),
    )
}

struct UnsubscribeGuard(Option<Box<dyn FnOnce() + Send>>);

impl Drop for UnsubscribeGuard {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            cancel();
        }
    }
}

// ---------------------------------------------------------------------------
// 中间件
// ---------------------------------------------------------------------------

async fn require_auth(State(server): State<Arc<Server>>, request: Request, next: Next) -> Response {
    if server
        .auth
        .authenticated(session_token_from(&request).as_deref())
    {
        next.run(request).await
    } else {
        json_error(StatusCode::UNAUTHORIZED, "需要配对后才能访问")
    }
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

async fn request_logger(
    State(server): State<Arc<Server>>,
    request: Request,
    next: Next,
) -> Response {
    let _ = &server; // 保持签名一致，日志不依赖状态
    let started = std::time::Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let is_api = path.starts_with("/api/") && path != "/api/events";
    let response = next.run(request).await;
    if is_api {
        info!("{method} {path} {:?}", started.elapsed());
    }
    response
}

async fn static_fallback(request: Request) -> Response {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return json_error(StatusCode::METHOD_NOT_ALLOWED, "请求方法不支持");
    }
    let accept_encoding = request
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok());
    let head_only = request.method() == Method::HEAD;
    assets::static_response(request.uri().path(), accept_encoding, head_only)
}

// ---------------------------------------------------------------------------
// 其他
// ---------------------------------------------------------------------------

/// 便于外部（CLI/桌面）使用的辅助：创建服务器并返回路由器。
pub fn build_router(server: &Server) -> Router {
    server.router()
}
