//! HTTP 服务器：axum 路由、认证、上传下载、SSE、静态资源与中间件。
//! 路由与响应语义逐一对齐 Go 版 `internal/server`。

use crate::assets;
use crate::auth::{AuthManager, ConnectedDevice, SESSION_COOKIE_NAME};
use crate::catalog::{Catalog, Item, SourceType};
use crate::download_slots::{DownloadSlot, DownloadSlots};
use crate::files::{save_upload, SaveUploadError};
use crate::hub::Hub;
use crate::progress::{ActiveTransfer, TransferKind, TransferRegistry};
use crate::uploads::{UploadError, UploadManager, MAX_CHUNK_SIZE};
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
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
    /// 外部注入的传输注册表（任务栏总进度 + 桌面「传输中」列表）；缺省时内部创建。
    pub progress: Option<Arc<TransferRegistry>>,
    /// 主机侧传输历史（可选；宿主注入后任务结束时写入，并暴露 /api/history）。
    pub history: Option<Arc<crate::history::TransferHistory>>,
    /// 是否需要六位配对码才能访问；关闭时所有受保护端点对局域网匿名放行。
    pub require_pairing: bool,
    /// 下载专用端口（可选，宿主绑定后通过 `with_download_port` 注入）。
    pub download_port: Option<u16>,
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
    progress: Arc<TransferRegistry>,
    /// 主机侧传输历史（可为空：未注入时不写、不提供 API 数据源）。
    history: Option<Arc<crate::history::TransferHistory>>,
    /// 分块上传会话及临时文件状态。
    uploads: Arc<UploadManager>,
    /// 是否需要配对码访问（快照自设置；更改后由宿主重启服务器生效）。
    require_pairing: bool,
    /// 下载专用端口（保留字段；方案 C 下为 None，下载与页面同端口）。
    download_port: Option<u16>,
    /// 每 IP 并发下载槽位（方案 C：限制 Range 并行，保住页面连接）。
    download_slots: Arc<DownloadSlots>,
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
            progress: match config.progress {
                Some(progress) => progress,
                None => TransferRegistry::new(),
            },
            history: config.history.clone(),
            uploads,
            require_pairing: config.require_pairing,
            download_port: config.download_port,
            download_slots: DownloadSlots::new(),
        })
    }

    /// 绑定下载专用端口后写入（影响 `/api/info` 与前端下载 URL）。
    pub fn with_download_port(mut self, port: u16) -> Self {
        self.download_port = Some(port);
        self
    }

    /// 下载专用端口。
    pub fn download_port(&self) -> Option<u16> {
        self.download_port
    }

    /// 传输注册表（任务栏进度与「传输中」列表）。
    pub fn progress(&self) -> Arc<TransferRegistry> {
        Arc::clone(&self.progress)
    }

    /// 当前活跃传输任务。
    pub fn active_transfers(&self) -> Vec<ActiveTransfer> {
        self.progress.list()
    }

    /// 主动取消传输任务（下载流中止 / 从列表移除）。
    pub fn cancel_transfer(&self, id: &str) -> bool {
        self.progress.cancel(id)
    }

    /// 共享目录表（文件系统监听等外围组件使用）。
    pub fn catalog_handle(&self) -> Arc<Catalog> {
        Arc::clone(&self.catalog)
    }

    /// SSE 事件中枢（文件系统监听广播使用）。
    pub fn hub_handle(&self) -> Arc<Hub> {
        Arc::clone(&self.hub)
    }

    /// 当前配对码。
    pub fn access_code(&self) -> String {
        self.auth.code()
    }

    /// 手动刷新配对码（不重启服务）。已配对设备会话仍然有效。
    pub fn refresh_access_code(&self) -> String {
        self.auth.rotate_code()
    }

    /// 认证管理器句柄（宿主热刷新配对码时使用）。
    pub fn auth_handle(&self) -> Arc<AuthManager> {
        Arc::clone(&self.auth)
    }

    /// 当前已连接/已配对设备。
    pub fn connected_devices(&self) -> Vec<ConnectedDevice> {
        self.auth.devices()
    }

    /// 下载专用路由：只暴露下载与 info，与主站共用认证/目录表，端口分离。
    /// 浏览器对不同端口使用独立连接池，并行 Range 不会挤掉页面 SSE/API。
    pub fn download_router(&self) -> Router {
        let server = Arc::new(self.clone());
        Router::new()
            .route("/api/info", get(handle_info))
            .route("/api/files/{id}/download", get(handle_download))
            .with_state(server.clone())
            .layer(middleware::from_fn_with_state(server.clone(), require_auth))
            .layer(middleware::from_fn(security_headers))
            .layer(middleware::from_fn_with_state(server, request_logger))
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
            .route("/api/files/{id}/text", get(handle_note_content))
            .route("/api/files/{id}", delete(handle_delete))
            .route("/api/devices", get(handle_devices))
            .route("/api/transfers", get(handle_active_transfers))
            .route(
                "/api/history",
                get(handle_history).delete(handle_clear_history),
            )
            .route("/api/notes", axum::routing::post(handle_create_note))
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
    /// 可选设备标签；缺省时服务端按 User-Agent 生成。
    #[serde(default)]
    device_label: Option<String>,
}

#[derive(Serialize)]
struct InfoPayload<'a> {
    device_name: &'a str,
    requires_auth: bool,
    max_upload_bytes: i64,
    version: &'a str,
    /// 下载专用端口；页面据此拼下载 URL，与浏览端口分离连接池。
    download_port: Option<u16>,
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

/// 文本/剪贴板投递请求体。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateNotePayload {
    #[serde(default)]
    title: String,
    content: String,
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
    /// 文本/剪贴板投递条目。
    #[serde(default)]
    is_note: bool,
    /// 下载次数。
    #[serde(default)]
    download_count: u32,
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
        is_note: item.is_note,
        download_count: item.download_count,
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

fn user_agent_from_headers(headers: &axum::http::HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn device_label_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-device-label")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn remote_ip_from(request: &Request) -> String {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

async fn handle_pair(State(server): State<Arc<Server>>, request: Request) -> Response {
    let ip = remote_ip_from(&request);
    let user_agent = user_agent_from_headers(request.headers());
    let body = match axum::body::to_bytes(request.into_body(), 4096).await {
        Ok(body) => body,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };
    let payload: PairPayload = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };

    let device_label = payload.device_label.unwrap_or_default();
    match server
        .auth
        .pair(&ip, &user_agent, &device_label, payload.code.trim())
    {
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

fn set_session_cookie(token: &str, expires: DateTime<Utc>) -> HeaderValue {
    let max_age = (expires - Utc::now()).num_seconds().max(1);
    let http_date = httpdate::fmt_http_date(SystemTime::from(expires));
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}; Expires={http_date}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("packetboat_session=; Path=/; HttpOnly"))
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
            requires_auth: server.require_pairing,
            max_upload_bytes: server.max_upload_bytes,
            version: &server.version,
            download_port: server.download_port,
        },
    )
}

async fn handle_session_status(State(server): State<Arc<Server>>, request: Request) -> Response {
    let ip = remote_ip_from(&request);
    let user_agent = user_agent_from_headers(request.headers());
    let label = device_label_from_headers(request.headers());
    let token = session_token_from(&request);
    // 免配对模式下对手机端直接报「已通过」：跳过配对门进入接收页
    let authenticated = if !server.require_pairing {
        server.auth.touch_peer(&ip, &user_agent, label.as_deref());
        true
    } else {
        server.auth.authenticated(
            token.as_deref(),
            Some(&ip),
            Some(&user_agent),
            label.as_deref(),
        )
    };
    json_response(StatusCode::OK, SessionPayload { authenticated })
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

async fn handle_devices(State(server): State<Arc<Server>>) -> Response {
    let devices = server.connected_devices();
    json_response(StatusCode::OK, serde_json::json!({ "devices": devices }))
}

async fn handle_active_transfers(State(server): State<Arc<Server>>) -> Response {
    let transfers = server.active_transfers();
    json_response(
        StatusCode::OK,
        serde_json::json!({ "transfers": transfers }),
    )
}

/// 主机侧传输历史（新到旧）。
async fn handle_history(State(server): State<Arc<Server>>) -> Response {
    let entries = server
        .history
        .as_ref()
        .map(|history| history.list())
        .unwrap_or_default();
    json_response(StatusCode::OK, serde_json::json!({ "history": entries }))
}

/// 清空传输历史，返回删除条数。
async fn handle_clear_history(State(server): State<Arc<Server>>) -> Response {
    let Some(history) = server.history.as_ref() else {
        return json_response(StatusCode::OK, serde_json::json!({ "cleared": 0 }));
    };
    match history.clear() {
        Ok(cleared) => json_response(StatusCode::OK, serde_json::json!({ "cleared": cleared })),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &err),
    }
}

async fn handle_create_chunk_upload(
    State(server): State<Arc<Server>>,
    request: Request,
) -> Response {
    let peer_ip = remote_ip_from(&request);
    let body = match axum::body::to_bytes(request.into_body(), 4096).await {
        Ok(body) => body,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };
    let payload: CreateUploadPayload = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };
    match server
        .uploads
        .create(&payload.file_name, payload.file_size, payload.chunk_size)
        .await
    {
        Ok(status) => {
            server.progress.begin_task(
                &status.id,
                &status.file_name,
                TransferKind::Upload,
                &peer_ip,
                Some(status.file_size),
            );
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
    let peer_ip = remote_ip_from(&request);
    let checksum = match request
        .headers()
        .get("x-chunk-sha256")
        .and_then(|value| value.to_str().ok())
    {
        Some(checksum) => checksum.to_string(),
        None => return json_error(StatusCode::BAD_REQUEST, "缺少 X-Chunk-SHA256 请求头"),
    };
    if !server.progress.contains_task(&id) {
        if let Ok(status) = server.uploads.status(&id).await {
            server.progress.ensure_task(
                &status.id,
                &status.file_name,
                TransferKind::Upload,
                &peer_ip,
                Some(status.file_size),
            );
        }
    }
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
                server.progress.add_to(&id, bytes.len() as u64);
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
            server
                .progress
                .finish_task_as(&id, crate::history::HistoryStatus::Complete);
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
            server
                .progress
                .finish_task_as(&id, crate::history::HistoryStatus::Cancelled);
            no_content()
        }
        // 会话已过期但仍留有进度任务时，取消请求仍应幂等收口。
        Err(UploadError::NotFound) => {
            server
                .progress
                .finish_task_as(&id, crate::history::HistoryStatus::Cancelled);
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

/// 单次 multipart 请求允许携带的文件数量上限（超出整请求报错，不做静默截断）。
const MAX_MULTIPART_FILES: usize = 50;

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
        if uploaded.len() >= MAX_MULTIPART_FILES {
            // 明确报错而非静默截断：否则客户端会误以为全部上传成功
            server.progress.finish();
            return json_error(
                StatusCode::BAD_REQUEST,
                "单次上传最多 50 个文件，请分批发送",
            );
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

/// 文本/剪贴板投递：短文本写入接收目录并加入共享清单。
async fn handle_create_note(State(server): State<Arc<Server>>, request: Request) -> Response {
    // JSON 字符串中的引号、反斜杠和控制字符最多可以膨胀到原始 UTF-8 的约 6 倍。
    let body_limit = crate::files::MAX_NOTE_BYTES
        .saturating_mul(6)
        .saturating_add(4096);
    let body = match axum::body::to_bytes(request.into_body(), body_limit).await {
        Ok(body) => body,
        Err(_) => return json_error(StatusCode::PAYLOAD_TOO_LARGE, "文本内容超过大小限制"),
    };
    let payload: CreateNotePayload = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "请求格式不正确"),
    };
    let content = payload.content.trim_end();
    if content.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "文本内容不能为空");
    }
    if content.len() > crate::files::MAX_NOTE_BYTES {
        return json_error(StatusCode::PAYLOAD_TOO_LARGE, "文本内容超过大小限制");
    }

    match crate::files::save_note(
        &server.catalog,
        &server.storage_dir,
        &payload.title,
        content,
        &server.file_mu,
    )
    .await
    {
        Ok(item) => {
            server
                .hub
                .publish(r#"{"type":"files_changed"}"#.to_string());
            json_response(
                StatusCode::CREATED,
                serde_json::json!({ "file": record_from_item(&item) }),
            )
        }
        Err(SaveUploadError::TooLarge) => {
            json_error(StatusCode::PAYLOAD_TOO_LARGE, "文本内容超过大小限制")
        }
        Err(SaveUploadError::InsufficientStorage) => {
            json_error(StatusCode::INSUFFICIENT_STORAGE, "接收目录可用空间不足")
        }
        Err(SaveUploadError::InvalidName) => json_error(StatusCode::BAD_REQUEST, "标题无效"),
        Err(SaveUploadError::Catalog(err)) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &err),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "保存文本失败，请检查磁盘空间和目录权限",
        ),
    }
}

/// 读取文本投递正文供接收页预览/复制。该接口不触发下载历史或下载次数。
async fn handle_note_content(
    State(server): State<Arc<Server>>,
    Path(id): Path<String>,
) -> Response {
    let Some(item) = server.catalog.get(&id) else {
        return json_error(StatusCode::NOT_FOUND, "文件不存在");
    };
    if !item.is_note {
        return json_error(StatusCode::BAD_REQUEST, "该文件不是文本投递");
    }
    let metadata = match tokio::fs::symlink_metadata(&item.local_path).await {
        Ok(metadata) => metadata,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "文本文件不存在"),
    };
    if !metadata.file_type().is_file() || metadata.len() > crate::files::MAX_NOTE_BYTES as u64 {
        return json_error(StatusCode::PAYLOAD_TOO_LARGE, "文本内容超过大小限制");
    }
    let bytes = match tokio::fs::read(&item.local_path).await {
        Ok(bytes) => bytes,
        Err(_) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, "读取文本失败"),
    };
    if bytes.len() > crate::files::MAX_NOTE_BYTES {
        return json_error(StatusCode::PAYLOAD_TOO_LARGE, "文本内容超过大小限制");
    }
    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => return json_error(StatusCode::UNPROCESSABLE_ENTITY, "文本文件不是有效 UTF-8"),
    };
    json_response(
        StatusCode::OK,
        serde_json::json!({ "content": content, "file": record_from_item(&item) }),
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

    let head_only = request.method() == Method::HEAD;
    let peer_ip = remote_ip_from(&request);
    // 同一文件的并发 Range 下载共用一个任务 id，避免浏览器多连接把一次下载拆成多条
    let transfer_id = format!("dl:{id}");

    // 刚被桌面端停止：冷却期内拒绝，避免浏览器 Range 重试让任务“复活”
    if !head_only && server.progress.recently_cancelled(&transfer_id) {
        return json_error(StatusCode::CONFLICT, "该传输刚被停止，请稍后再下载");
    }

    // 每 IP 并发下载限流（HEAD 不占槽）：超出立即 429，浏览器稍后重试即可续传
    let slot = if head_only {
        None
    } else {
        match server.download_slots.try_acquire(&peer_ip) {
            Some(slot) => Some(slot),
            None => return json_error(StatusCode::TOO_MANY_REQUESTS, "并发下载过多，请稍后重试"),
        }
    };

    // 文件夹：实时打包为 zip 流式下载
    if item.is_dir {
        if head_only {
            return zip_download(None, &item, "", None, None, None).await;
        }
        let _is_new = server.progress.begin_task(
            &transfer_id,
            format!("{}.zip", item.name),
            TransferKind::Download,
            &peer_ip,
            None,
        );
        return zip_download(
            Some(Arc::clone(&server.progress)),
            &item,
            &transfer_id,
            slot,
            Some(Arc::clone(&server.catalog)),
            Some(Arc::clone(&server.hub)),
        )
        .await;
    }

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
    serve_file_download(
        server,
        &item,
        if_modified_since,
        range_header,
        head_only,
        peer_ip,
        transfer_id,
        slot,
    )
    .await
}

/// 递增下载次数并广播文件变更（供列表刷新）。
fn record_download_and_notify(server: &Server, id: &str) {
    record_download_and_notify_parts(&server.catalog, &server.hub, id);
}

fn record_download_and_notify_parts(catalog: &Catalog, hub: &Hub, id: &str) {
    if catalog.record_download(id).is_some() {
        hub.publish(r#"{"type":"files_changed"}"#.to_string());
    }
}

/// 单文件下载：支持 Range 与 If-Modified-Since，并登记传输任务。
#[allow(clippy::too_many_arguments)]
async fn serve_file_download(
    server: Arc<Server>,
    item: &crate::catalog::Item,
    if_modified_since: Option<String>,
    range_header: Option<String>,
    head_only: bool,
    peer_ip: String,
    transfer_id: String,
    slot: Option<DownloadSlot>,
) -> Response {
    let metadata = match std::fs::metadata(&item.local_path) {
        Ok(metadata) => metadata,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "文件不存在"),
    };
    let size = metadata.len();
    // 恢复 Range：下载走独立端口后，并行续传不再挤占页面连接池

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

    // 空文件：GET 视为完整下载
    if size == 0 {
        if !head_only {
            let _is_new = server.progress.begin_task(
                &transfer_id,
                &item.name,
                TransferKind::Download,
                &peer_ip,
                Some(0),
            );
            server
                .progress
                .finish_task_as(&transfer_id, crate::history::HistoryStatus::Complete);
            record_download_and_notify(&server, &item.id);
        }
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

    let file = if head_only {
        None
    } else {
        match tokio::fs::File::open(&item.local_path).await {
            Ok(mut file) => {
                if start > 0 {
                    use tokio::io::AsyncSeekExt;
                    if let Err(err) = file.seek(std::io::SeekFrom::Start(start)).await {
                        warn!("定位文件失败: {err}");
                        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "读取文件失败");
                    }
                }
                Some(file)
            }
            Err(_) => {
                return json_error(StatusCode::NOT_FOUND, "文件不存在");
            }
        }
    };

    if file.is_some() {
        // 下载次数在流完整结束时再计（见 DownloadCountStream::finish_transfer）
        let _is_new = server.progress.begin_task(
            &transfer_id,
            &item.name,
            TransferKind::Download,
            &peer_ip,
            Some(size),
        );
    }

    let body = match file {
        Some(file) => {
            use tokio::io::AsyncReadExt;
            let stream = DownloadCountStream {
                inner: tokio_util::io::ReaderStream::with_capacity(
                    file.take(content_length),
                    crate::files::TRANSFER_BUFFER_SIZE,
                ),
                transfers: Arc::clone(&server.progress),
                transfer_id: transfer_id.clone(),
                file_id: item.id.clone(),
                catalog: Arc::clone(&server.catalog),
                hub: Arc::clone(&server.hub),
                delivered: 0,
                expected_bytes: content_length,
                finished: false,
                _slot: slot,
            };
            Body::from_stream(stream)
        }
        None => Body::empty(),
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

/// 包装下载流：传输监控 + **完整下完后**才计下载次数。
struct DownloadCountStream {
    inner: tokio_util::io::ReaderStream<tokio::io::Take<tokio::fs::File>>,
    transfers: Arc<TransferRegistry>,
    transfer_id: String,
    file_id: String,
    catalog: Arc<Catalog>,
    hub: Arc<Hub>,
    /// 当前 HTTP 流已经写出的字节数。
    delivered: u64,
    /// 当前 200/Range 响应要求写出的字节数。
    expected_bytes: u64,
    /// poll 已处理结束后置位，避免 Drop 再次 finish。
    finished: bool,
    /// 持有下载并发槽位直到流结束。
    _slot: Option<DownloadSlot>,
}

impl DownloadCountStream {
    fn finish_transfer(&mut self, completed: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(should_count) = self.transfers.finish_stream(&self.transfer_id, completed) {
            if should_count {
                record_download_and_notify_parts(&self.catalog, &self.hub, &self.file_id);
            }
        }
    }
}

impl futures_util::Stream for DownloadCountStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.transfers.is_cancelled(&self.transfer_id) {
            self.finish_transfer(false);
            return std::task::Poll::Ready(None);
        }
        if self.transfers.is_paused(&self.transfer_id) {
            self.transfers
                .store_pause_waker(&self.transfer_id, cx.waker().clone());
            return std::task::Poll::Pending;
        }
        let poll = futures_util::StreamExt::poll_next_unpin(&mut self.inner, cx);
        if let std::task::Poll::Ready(Some(Ok(bytes))) = &poll {
            self.delivered = self.delivered.saturating_add(bytes.len() as u64);
            self.transfers.add_to(&self.transfer_id, bytes.len() as u64);
        }
        if matches!(poll, std::task::Poll::Ready(None)) {
            self.finish_transfer(true);
        }
        poll
    }
}

impl Drop for DownloadCountStream {
    /// 客户端断开时流被 drop：释放槽位；未传完则不计下载次数。
    fn drop(&mut self) {
        // 浏览器在收满 Content-Length 后不一定再 poll 到 EOF，直接 drop 时按
        // 当前响应实际写出的字节数判断是否完整。
        self.finish_transfer(self.delivered >= self.expected_bytes);
    }
}

/// 文件夹下载：边读源目录边压缩，通过 duplex 管道流式发送，不落完整临时 zip。
async fn zip_download(
    transfers: Option<Arc<TransferRegistry>>,
    item: &crate::catalog::Item,
    transfer_id: &str,
    slot: Option<DownloadSlot>,
    catalog: Option<Arc<Catalog>>,
    hub: Option<Arc<Hub>>,
) -> Response {
    let zip_name = format!("{}.zip", item.name);
    let head_only = transfers.is_none();

    let mut response = if head_only {
        Response::new(Body::empty())
    } else {
        let source = std::path::PathBuf::from(&item.local_path);
        if !source.is_dir() {
            return json_error(StatusCode::NOT_FOUND, "文件夹不存在");
        }

        let folder_name = item.name.clone();
        let (reader, writer) = tokio::io::duplex(crate::files::TRANSFER_BUFFER_SIZE * 2);
        tokio::spawn(async move {
            if let Err(err) = crate::files::stream_folder_zip(source, folder_name, writer).await {
                warn!("流式打包文件夹失败: {err}");
            }
        });

        let stream = TransferCountStream {
            inner: tokio_util::io::ReaderStream::with_capacity(
                reader,
                crate::files::TRANSFER_BUFFER_SIZE,
            ),
            transfers,
            transfer_id: transfer_id.to_string(),
            file_id: item.id.clone(),
            catalog,
            hub,
            finished: false,
            _slot: slot,
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

/// ZIP 流式下载：传输监控 + 完整结束后计下载次数。
struct TransferCountStream {
    inner: tokio_util::io::ReaderStream<tokio::io::DuplexStream>,
    transfers: Option<Arc<TransferRegistry>>,
    transfer_id: String,
    file_id: String,
    catalog: Option<Arc<Catalog>>,
    hub: Option<Arc<Hub>>,
    finished: bool,
    _slot: Option<DownloadSlot>,
}

impl TransferCountStream {
    fn finish_transfer(&mut self, completed: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        let Some(transfers) = &self.transfers else {
            return;
        };
        if let Some(should_count) = transfers.finish_stream(&self.transfer_id, completed) {
            if should_count {
                if let (Some(catalog), Some(hub)) = (&self.catalog, &self.hub) {
                    record_download_and_notify_parts(catalog, hub, &self.file_id);
                }
            }
        }
    }
}

impl futures_util::Stream for TransferCountStream {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(transfers) = &self.transfers {
            if transfers.is_cancelled(&self.transfer_id) {
                self.finish_transfer(false);
                return std::task::Poll::Ready(None);
            }
            if transfers.is_paused(&self.transfer_id) {
                transfers.store_pause_waker(&self.transfer_id, cx.waker().clone());
                return std::task::Poll::Pending;
            }
        }
        let poll = futures_util::StreamExt::poll_next_unpin(&mut self.inner, cx);
        if let (std::task::Poll::Ready(Some(Ok(bytes))), Some(transfers)) = (&poll, &self.transfers)
        {
            transfers.add_to(&self.transfer_id, bytes.len() as u64);
        }
        if matches!(poll, std::task::Poll::Ready(None)) {
            self.finish_transfer(true);
        }
        poll
    }
}

impl Drop for TransferCountStream {
    fn drop(&mut self) {
        self.finish_transfer(false);
    }
}

async fn handle_delete(State(server): State<Arc<Server>>, Path(id): Path<String>) -> Response {
    let item = match server.catalog.get(&id) {
        Some(item) => item,
        None => return json_error(StatusCode::NOT_FOUND, "文件不存在"),
    };

    // 当前版本安全边界：任何来源（linked / received / inbox）都只移出共享清单，
    // 绝不删除磁盘文件。inbox 扫描条目由 catalog 隐藏名单处理。
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
    request: Request,
) -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    // 打开 SSE 时登记长连接；流被 drop（页面关闭）时自动注销设备。
    let token = session_token_from(&request);
    let ip = remote_ip_from(&request);
    let user_agent = user_agent_from_headers(request.headers());
    let label = device_label_from_headers(request.headers());
    let connection_key =
        server
            .auth
            .begin_connection(token.as_deref(), &ip, &user_agent, label.as_deref());
    let lease = server.auth.lease_connection(connection_key);
    let (mut receiver, unsubscribe) = server.hub.subscribe();

    let stream = async_stream::stream! {
        let _lease = lease;
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
    let ip = remote_ip_from(&request);
    let user_agent = user_agent_from_headers(request.headers());
    let label = device_label_from_headers(request.headers());
    if !server.require_pairing {
        server.auth.touch_peer(&ip, &user_agent, label.as_deref());
        return next.run(request).await;
    }
    if server.auth.authenticated(
        session_token_from(&request).as_deref(),
        Some(&ip),
        Some(&user_agent),
        label.as_deref(),
    ) {
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
        // frame-src/connect-src 放行同机 http(s)：下载专用端口与页面端口分离
        HeaderValue::from_static("default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self' http: https:; frame-src 'self' http: https:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    async fn server_with(require_pairing: bool) -> Arc<Server> {
        let storage = std::env::temp_dir().join(format!(
            "packetboat-server-auth-{}-{}",
            std::process::id(),
            require_pairing as u8,
        ));
        let _ = std::fs::remove_dir_all(&storage);
        let server = Server::new(Config {
            device_name: "TEST-PC".into(),
            storage_dir: storage,
            max_upload_bytes: 1024,
            version: "test".into(),
            catalog: None,
            progress: None,
            require_pairing,
            history: None,
            download_port: None,
        })
        .expect("server");
        Arc::new(server)
    }

    async fn get(router: Router, path: &str) -> (StatusCode, serde_json::Value) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())
            .expect("request");
        let response = router.oneshot(request).await.expect("infallible");
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap_or_default();
        let payload = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, payload)
    }

    #[tokio::test]
    async fn pairing_required_blocks_anonymous_by_default() {
        let server = server_with(true).await;
        let (status, _) = get(server.router(), "/api/files").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, payload) = get(server.router(), "/api/info").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["requires_auth"], serde_json::Value::Bool(true));

        let (status, payload) = get(server.router(), "/api/session").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["authenticated"], serde_json::Value::Bool(false));
    }

    #[tokio::test]
    async fn pairing_disabled_grants_anonymous_access() {
        let server = server_with(false).await;
        // 受保护端点匿名放行
        let (status, _) = get(server.router(), "/api/files").await;
        assert_eq!(status, StatusCode::OK);

        // info 告知手机端无需配对
        let (status, payload) = get(server.router(), "/api/info").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["requires_auth"], serde_json::Value::Bool(false));

        // 会话状态直接报「已通过」，跳过配对门
        let (status, payload) = get(server.router(), "/api/session").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["authenticated"], serde_json::Value::Bool(true));
    }
}
