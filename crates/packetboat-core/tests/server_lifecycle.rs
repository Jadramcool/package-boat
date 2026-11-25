//! 端到端测试：移植 Go 版 `internal/server` 的行为测试。
//! 覆盖文件生命周期、静态资源与安全头、压缩缓存、原位共享下载与取消共享、唯一命名与大小限制。

use packetboat_core::catalog::Catalog;
use packetboat_core::files::MAX_NOTE_BYTES;
use packetboat_core::history::{open_in_memory, HistoryStatus};
use packetboat_core::progress::TransferRegistry;
use packetboat_core::server::{Config, Server};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

/// 启动测试服务器，返回（base_url, access_code, 服务器句柄）。
async fn spawn(config: Config) -> (String, String, tokio::task::JoinHandle<()>) {
    let server = Server::new(config).expect("server init");
    let code = server.access_code();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}");
    let router = server.router();
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("serve");
    });
    (url, code, handle)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("packetboat-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cookie_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap()
}

#[derive(serde::Deserialize)]
struct FileRecord {
    id: String,
}

#[derive(serde::Deserialize)]
struct FilesResponse {
    files: Vec<FileRecord>,
}

async fn pair(client: &reqwest::Client, url: &str, code: &str) -> reqwest::Response {
    client
        .post(format!("{url}/api/session"))
        .json(&serde_json::json!({
            "code": code,
            "device_label": "测试设备"
        }))
        .send()
        .await
        .expect("pair request")
}

#[tokio::test]
async fn file_lifecycle() {
    let storage = temp_dir("lifecycle");
    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();

    // 未认证列表 → 401
    let status = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);

    // 错误配对码 → 401
    let status = pair(&client, &url, "999999").await.status();
    assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);

    // 正确配对码 → 204
    let status = pair(&client, &url, &code).await.status();
    assert_eq!(status, reqwest::StatusCode::NO_CONTENT);

    // 上传 → 201
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::text("hello over lan").file_name("hello.txt"),
    );
    let response = client
        .post(format!("{url}/api/files"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let uploaded: serde_json::Value = response.json().await.unwrap();
    let uploaded_file = &uploaded["files"][0];
    assert_eq!(uploaded_file["name"], "hello.txt");
    let id = uploaded_file["id"].as_str().unwrap().to_string();

    // 列表包含上传文件
    let listing: FilesResponse = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listing.files.len(), 1);
    assert_eq!(listing.files[0].id, id);

    // 下载内容一致
    let body = client
        .get(format!("{url}/api/files/{id}/download"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "hello over lan");

    // 移出清单 → 204；磁盘文件保留，但不再出现在共享下载路径
    let status = client
        .delete(format!("{url}/api/files/{id}"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::NO_CONTENT);
    assert!(
        storage.join("hello.txt").exists(),
        "删除共享条目不得删除接收目录中的磁盘文件"
    );
    let status = client
        .get(format!("{url}/api/files/{id}/download"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::NOT_FOUND);

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn note_delivery_writes_txt_and_lists_as_note() {
    let storage = temp_dir("note-delivery");
    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();

    // 未配对不能投递
    let status = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({ "title": "x", "content": "hello note" }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);

    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    // 空正文 → 400
    let status = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({ "title": "empty", "content": "   " }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);

    // 正常投递 → 201，is_note=true，磁盘有 .txt
    let response = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({
            "title": "会议纪要",
            "content": "今晚 8 点同步进度\n链接 https://example.local"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let payload: serde_json::Value = response.json().await.unwrap();
    let note = &payload["file"];
    assert_eq!(note["is_note"], true);
    assert_eq!(note["source_type"], "received");
    let name = note["name"].as_str().unwrap().to_string();
    assert!(name.ends_with(".txt"), "name={name}");
    let id = note["id"].as_str().unwrap().to_string();

    // 列表可见
    let listing: FilesResponse = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listing.files.len(), 1);
    assert_eq!(listing.files[0].id, id);

    // 下载正文一致
    let body = client
        .get(format!("{url}/api/files/{id}/download"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("今晚 8 点同步进度"));
    assert!(body.contains("https://example.local"));

    // 接收目录里应有该 .txt
    assert!(
        storage.join(&name).exists(),
        "文本投递应落盘到接收目录: {name}"
    );

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn note_text_preview_and_history_api_are_consistent() {
    let storage = temp_dir("note-history-api");
    let history = open_in_memory(7);
    let progress = TransferRegistry::new();
    progress.attach_history(Arc::clone(&history));
    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: None,
        progress: Some(progress),
        require_pairing: true,
        history: Some(Arc::clone(&history)),
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    let note: serde_json::Value = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({ "title": "可复制文本", "content": "第一行\n第二行" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = note["file"]["id"].as_str().unwrap().to_string();

    let preview: serde_json::Value = client
        .get(format!("{url}/api/files/{id}/text"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["content"], "第一行\n第二行");
    assert!(history.list().is_empty(), "文本预览不应产生下载历史");

    for _ in 0..2 {
        let body = client
            .get(format!("{url}/api/files/{id}/download"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "第一行\n第二行");
    }

    let payload: serde_json::Value = client
        .get(format!("{url}/api/history"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entries = payload["history"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "同一文件每次下载应保留独立历史");
    assert_ne!(entries[0]["id"], entries[1]["id"]);
    assert_eq!(entries[0]["status"], "complete");
    assert_eq!(entries[0]["kind"], "download");
    assert!(entries[0]["transferred_bytes"].as_u64().unwrap() > 0);
    assert_eq!(history.list()[0].status, HistoryStatus::Complete);

    let cleared: serde_json::Value = client
        .delete(format!("{url}/api/history"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["cleared"], 2);
    assert!(history.list().is_empty());

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn note_size_limit_counts_utf8_bytes_not_json_characters() {
    let storage = temp_dir("note-size-limit");
    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    let exact = "a".repeat(MAX_NOTE_BYTES);
    let status = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({ "title": "exact", "content": exact }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::CREATED);

    let too_large = "a".repeat(MAX_NOTE_BYTES + 1);
    let status = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({ "title": "too-large", "content": too_large }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::PAYLOAD_TOO_LARGE);

    // 每个字符都需要 JSON 转义时，请求体接近 2 倍；仍应接受合法的 64KB 正文。
    let escaped = "\"".repeat(MAX_NOTE_BYTES);
    let status = client
        .post(format!("{url}/api/notes"))
        .json(&serde_json::json!({ "title": "escaped", "content": escaped }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::CREATED);

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn existing_receive_files_are_not_shared_on_startup() {
    let storage = temp_dir("receive-isolation");
    std::fs::write(storage.join("unrelated.txt"), b"must stay private").unwrap();
    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    let listing: FilesResponse = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(listing.files.is_empty());
    assert!(storage.join("unrelated.txt").exists());

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn static_app_and_security_headers() {
    let storage = temp_dir("static");
    let (url, _, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;

    let response = reqwest::get(format!("{url}/missing-client-route"))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(
        body.contains(r#"<div id="app"></div>"#),
        "SPA fallback served"
    );
    let headers = reqwest::get(format!("{url}/")).await.unwrap();
    assert!(headers.headers().contains_key("content-security-policy"));

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn static_asset_compression_and_caching() {
    let storage = temp_dir("compress");
    let (url, _, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;

    let index = reqwest::get(format!("{url}/"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let script_path = index
        .split("src=\"./")
        .nth(1)
        .and_then(|value| value.split('"').next())
        .expect("built index contains a module script");

    // gzip 变体：Content-Encoding + 不可变缓存
    let response = reqwest::Client::new()
        .get(format!("{url}/{script_path}"))
        .header("Accept-Encoding", "br, gzip")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "gzip");
    assert_eq!(
        response.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    let compressed = response.bytes().await.unwrap();
    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut decoded = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut decoded).unwrap();
    assert!(
        decoded.starts_with(b"const") || decoded.len() > 100,
        "decompressed js"
    );

    // 禁用 gzip：返回原始内容（体积大于压缩变体）
    let response = reqwest::Client::new()
        .get(format!("{url}/{script_path}"))
        .header("Accept-Encoding", "gzip;q=0")
        .send()
        .await
        .unwrap();
    assert!(!response.headers().contains_key("content-encoding"));
    let raw = response.bytes().await.unwrap();
    assert!(
        raw.len() > compressed.len(),
        "raw asset is larger than gzip"
    );

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn linked_file_download_and_unshare_preserves_original() {
    let receive_dir = temp_dir("linked-receive");
    let original_dir = temp_dir("linked-original");
    let original = original_dir.join("original.txt");
    std::fs::write(&original, "kept in place").unwrap();

    let file_catalog =
        Arc::new(Catalog::open(temp_dir("linked-catalog").join("catalog.json")).unwrap());
    let linked = file_catalog
        .add_linked(&[original.to_string_lossy().into_owned()])
        .unwrap();
    assert_eq!(linked.len(), 1);

    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: receive_dir.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: Some(file_catalog.clone()),
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    // 原位文件可下载
    let body = client
        .get(format!("{url}/api/files/{}/download", linked[0].id))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "kept in place");

    // 取消共享不删除原文件
    let status = client
        .delete(format!("{url}/api/files/{}", linked[0].id))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::NO_CONTENT);
    assert_eq!(std::fs::read_to_string(&original).unwrap(), "kept in place");
    assert!(file_catalog.get(&linked[0].id).is_none());

    server.abort();
    let _ = std::fs::remove_dir_all(&receive_dir);
    let _ = std::fs::remove_dir_all(&original_dir);
}

#[tokio::test]
async fn download_count_and_connected_devices_are_tracked() {
    let receive_dir = temp_dir("devices-count");
    let file_catalog =
        Arc::new(Catalog::open(temp_dir("devices-count-catalog").join("catalog.json")).unwrap());
    let original = receive_dir.join("note.txt");
    std::fs::write(&original, "hello").unwrap();
    let linked = file_catalog
        .add_linked(&[original.to_string_lossy().into_owned()])
        .unwrap();

    let (url, code, server) = spawn(Config {
        device_name: "test-station".into(),
        storage_dir: receive_dir.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: Some(file_catalog.clone()),
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    let devices = client
        .get(format!("{url}/api/devices"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(devices["devices"][0]["auth_mode"], "paired");
    // 设备名由 UA 推导（测试客户端无 Android 机型时显示 HTTP 客户端标识）
    let label = devices["devices"][0]["device_label"].as_str().unwrap_or("");
    assert!(!label.is_empty());
    assert_eq!(devices["devices"][0]["online"], true);

    let before = file_catalog.list()[0].download_count;
    let body = client
        .get(format!("{url}/api/files/{}/download", linked[0].id))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "hello");

    let files = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(
        files["files"][0]["download_count"].as_u64().unwrap(),
        u64::from(before) + 1
    );

    server.abort();
    let _ = std::fs::remove_dir_all(&receive_dir);
}

#[tokio::test]
async fn empty_file_downloads_with_zero_length() {
    let storage = temp_dir("empty-file");
    let history = open_in_memory(7);
    let progress = TransferRegistry::new();
    progress.attach_history(Arc::clone(&history));
    let (url, code, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024,
        version: "test".into(),
        catalog: None,
        progress: Some(progress),
        require_pairing: true,
        history: Some(Arc::clone(&history)),
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    // 上传 0 字节文件
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(Vec::new()).file_name("empty.txt"),
    );
    let response = client
        .post(format!("{url}/api/files"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let uploaded: serde_json::Value = response.json().await.unwrap();
    let id = uploaded["files"][0]["id"].as_str().unwrap().to_string();

    // 下载：200 + Content-Length: 0 + 空 body
    let response = client
        .get(format!("{url}/api/files/{id}/download"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["content-length"], "0");
    assert_eq!(response.bytes().await.unwrap().len(), 0);
    let downloads: Vec<_> = history
        .list()
        .into_iter()
        .filter(|entry| entry.kind == packetboat_core::progress::TransferKind::Download)
        .collect();
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0].status, HistoryStatus::Complete);
    assert_eq!(downloads[0].transferred_bytes, 0);

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn conditional_and_range_requests() {
    let storage = temp_dir("cond-range");
    let history = open_in_memory(7);
    let progress = TransferRegistry::new();
    progress.attach_history(Arc::clone(&history));
    let (url, code, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024,
        version: "test".into(),
        catalog: None,
        progress: Some(progress),
        require_pairing: true,
        history: Some(Arc::clone(&history)),
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    // 上传一个 10 字节文件
    let content = b"0123456789".to_vec();
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content.clone()).file_name("data.bin"),
    );
    let response = client
        .post(format!("{url}/api/files"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let uploaded: serde_json::Value = response.json().await.unwrap();
    let id = uploaded["files"][0]["id"].as_str().unwrap().to_string();
    let dl = format!("{url}/api/files/{id}/download");

    // HEAD：200 + Content-Length 10 + 空 body
    let response = client.head(&dl).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["content-length"], "10");
    assert_eq!(response.bytes().await.unwrap().len(), 0);

    // 合法单段 Range → 206
    let response = client
        .get(&dl)
        .header("Range", "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-range"], "bytes 2-5/10");
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"2345");

    // 后缀 Range → 206
    let response = client
        .get(&dl)
        .header("Range", "bytes=-3")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"789");

    // 不可满足 Range → 416
    let response = client
        .get(&dl)
        .header("Range", "bytes=100-200")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        reqwest::StatusCode::RANGE_NOT_SATISFIABLE
    );

    // 多段 Range → 退化为全量 200
    let response = client
        .get(&dl)
        .header("Range", "bytes=0-1,5-6")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.bytes().await.unwrap().as_ref(), &content[..]);

    // If-Modified-Since（未来时间）→ 304
    let future = httpdate::fmt_http_date(
        std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
    );
    let response = client
        .get(&dl)
        .header("If-Modified-Since", future)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_MODIFIED);

    let downloads: Vec<_> = history
        .list()
        .into_iter()
        .filter(|entry| entry.kind == packetboat_core::progress::TransferKind::Download)
        .collect();
    assert_eq!(downloads.len(), 3, "三次成功 Range/全量请求都应有历史");
    assert!(downloads
        .iter()
        .all(|entry| entry.status == HistoryStatus::Complete));
    let bytes: Vec<u64> = downloads
        .iter()
        .map(|entry| entry.transferred_bytes)
        .collect();
    assert_eq!(bytes, vec![10, 3, 4]);

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn folder_share_downloads_as_zip() {
    let storage = temp_dir("folder-e2e");
    // 建一个含子目录与文件的文件夹
    let shared_dir = temp_dir("folder-src");
    let sub_dir = shared_dir.join("子目录");
    std::fs::create_dir_all(&sub_dir).unwrap();
    std::fs::write(shared_dir.join("readme.txt"), "folder readme").unwrap();
    std::fs::write(sub_dir.join("data.bin"), [0u8, 1, 2, 3, 4]).unwrap();
    let empty_dir = shared_dir.join("empty");
    std::fs::create_dir_all(&empty_dir).unwrap();

    let file_catalog =
        Arc::new(Catalog::open(temp_dir("folder-catalog").join("catalog.json")).unwrap());
    let linked = file_catalog
        .add_linked(&[shared_dir.to_string_lossy().into_owned()])
        .unwrap();
    assert_eq!(linked.len(), 1);
    assert!(linked[0].is_dir);

    let (url, code, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: Some(file_catalog.clone()),
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    // 下载 → 得到 zip
    let response = client
        .get(format!("{url}/api/files/{}/download", linked[0].id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/zip");
    assert!(response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .contains(".zip"));
    let body = response.bytes().await.unwrap();
    assert!(!body.is_empty());

    // 解析 zip 验证条目（顶层为文件夹名）
    let top = shared_dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let cursor = std::io::Cursor::new(body.to_vec());
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == &format!("{top}/readme.txt")),
        "names: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == &format!("{top}/子目录/data.bin")),
        "names: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == &format!("{top}/empty/")),
        "空目录应存在: {names:?}"
    );

    // 内容校验
    let mut readme = archive.by_name(&format!("{top}/readme.txt")).unwrap();
    let mut content = String::new();
    std::io::Read::read_to_string(&mut readme, &mut content).unwrap();
    assert_eq!(content, "folder readme");

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
    let _ = std::fs::remove_dir_all(&shared_dir);
}

#[tokio::test]
async fn save_upload_uses_unique_name_and_size_limit() {
    let storage = temp_dir("unique");
    let (url, code, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 5,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    async fn upload(client: &reqwest::Client, url: &str, content: &str) -> reqwest::Response {
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::text(content.to_string()).file_name("hello.txt"),
        );
        client
            .post(format!("{url}/api/files"))
            .multipart(form)
            .send()
            .await
            .unwrap()
    }

    let first = upload(&client, &url, "hello").await;
    assert_eq!(first.status(), reqwest::StatusCode::CREATED);
    let second = upload(&client, &url, "world").await;
    assert_eq!(second.status(), reqwest::StatusCode::CREATED);

    let first_json: serde_json::Value = first.json().await.unwrap();
    let second_json: serde_json::Value = second.json().await.unwrap();
    assert_eq!(first_json["files"][0]["name"], "hello.txt");
    assert_eq!(second_json["files"][0]["name"], "hello (1).txt");

    // 超过大小限制 → 413，且不留可见文件
    let too_large = upload(&client, &url, "too big").await;
    assert_eq!(too_large.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!storage.join("large.txt").exists());
    assert!(!storage.join(".packetboat-").exists());

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test]
async fn chunk_upload_resumes_validates_and_cleans_up() {
    use sha2::{Digest, Sha256};

    let storage = temp_dir("chunk-resume");
    let (url, code, server) = spawn(Config {
        device_name: "test".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "test".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
        history: None,
        download_port: None,
    })
    .await;
    let client = cookie_client();

    let create_url = format!("{url}/api/uploads");
    let unauthorized = client
        .post(&create_url)
        .json(&serde_json::json!({ "file_name": "resume.bin", "file_size": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        pair(&client, &url, &code).await.status(),
        reqwest::StatusCode::NO_CONTENT
    );

    let chunk_size = packetboat_core::uploads::MIN_CHUNK_SIZE as usize;
    let content: Vec<u8> = (0..chunk_size + 7)
        .map(|index| (index % 251) as u8)
        .collect();
    let created = client
        .post(&create_url)
        .json(&serde_json::json!({
            "file_name": "../resume.bin",
            "file_size": content.len(),
            "chunk_size": chunk_size,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), reqwest::StatusCode::CREATED);
    let session: serde_json::Value = created.json().await.unwrap();
    assert_eq!(session["file_name"], "resume.bin");
    assert_eq!(session["total_chunks"], 2);
    let upload_id = session["id"].as_str().unwrap();
    let first = &content[..chunk_size];
    let first_hash = format!("{:x}", Sha256::digest(first));
    let chunk_url = format!("{url}/api/uploads/{upload_id}/chunks/0");

    let bad_hash = client
        .put(&chunk_url)
        .header("X-Chunk-SHA256", "0".repeat(64))
        .body(first.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(bad_hash.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);

    let first_upload = client
        .put(&chunk_url)
        .header("X-Chunk-SHA256", &first_hash)
        .body(first.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(first_upload.status(), reqwest::StatusCode::OK);

    // 相同块与相同摘要可安全重试，不重复计入会话进度。
    let duplicate = client
        .put(&chunk_url)
        .header("X-Chunk-SHA256", &first_hash)
        .body(first.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), reqwest::StatusCode::OK);
    let duplicate_status: serde_json::Value = duplicate.json().await.unwrap();
    assert_eq!(duplicate_status["uploaded_bytes"], chunk_size);

    // 模拟连接中断：重新查询会话后只需补传第二块。
    let resumed: serde_json::Value = client
        .get(format!("{url}/api/uploads/{upload_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resumed["received_chunks"], serde_json::json!([0]));

    let incomplete = client
        .post(format!("{url}/api/uploads/{upload_id}/complete"))
        .send()
        .await
        .unwrap();
    assert_eq!(incomplete.status(), reqwest::StatusCode::CONFLICT);

    let second = &content[chunk_size..];
    let second_hash = format!("{:x}", Sha256::digest(second));
    let second_upload = client
        .put(format!("{url}/api/uploads/{upload_id}/chunks/1"))
        .header("X-Chunk-SHA256", second_hash)
        .body(second.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(second_upload.status(), reqwest::StatusCode::OK);

    let completed = client
        .post(format!("{url}/api/uploads/{upload_id}/complete"))
        .send()
        .await
        .unwrap();
    let completed_status = completed.status();
    let completed_body = completed.bytes().await.unwrap();
    assert_eq!(
        completed_status,
        reqwest::StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&completed_body)
    );
    let completed: serde_json::Value = serde_json::from_slice(&completed_body).unwrap();
    let file_id = completed["files"][0]["id"].as_str().unwrap();

    // 最终响应丢失时，客户端可重复 complete，必须返回同一文件而不是产生副本。
    let completed_again: serde_json::Value = client
        .post(format!("{url}/api/uploads/{upload_id}/complete"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(completed_again["files"][0]["id"], file_id);
    let completed_status: serde_json::Value = client
        .get(format!("{url}/api/uploads/{upload_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(completed_status["completed"], true);

    let downloaded = client
        .get(format!("{url}/api/files/{file_id}/download"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(downloaded.as_ref(), content.as_slice());

    let cancelled = client
        .post(&create_url)
        .json(&serde_json::json!({ "file_name": "cancel.bin", "file_size": 12 }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let cancelled_id = cancelled["id"].as_str().unwrap();
    let response = client
        .delete(format!("{url}/api/uploads/{cancelled_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    let missing = client
        .get(format!("{url}/api/uploads/{cancelled_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
    assert!(std::fs::read_dir(&storage).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".part")));

    server.abort();
    let _ = std::fs::remove_dir_all(&storage);
}
