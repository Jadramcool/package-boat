//! 真实场景与压力测试：通过 HTTP 走完整链路（配对、上传、下载、文件夹 ZIP、
//! inbox 本地文件、并发与批量），不模拟 UI，验证服务端在实际负载下的行为。

use packetboat_core::catalog::Catalog;
use packetboat_core::server::{Config, Server};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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
    // 给 accept 循环一点时间就绪
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    (url, code, handle)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("packetboat-scenario-{tag}-{}", std::process::id()));
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

async fn pair(client: &reqwest::Client, url: &str, code: &str) -> reqwest::StatusCode {
    client
        .post(format!("{url}/api/session"))
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .expect("pair")
        .status()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn payload(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| seed.wrapping_add((i % 251) as u8))
        .collect()
}

async fn multipart_upload(
    client: &reqwest::Client,
    url: &str,
    name: &str,
    data: Vec<u8>,
) -> (reqwest::StatusCode, serde_json::Value) {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(data).file_name(name.to_string()),
    );
    let response = client
        .post(format!("{url}/api/files"))
        .multipart(form)
        .send()
        .await
        .expect("upload");
    let status = response.status();
    let json: serde_json::Value = response.json().await.unwrap_or_default();
    (status, json)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scenario_end_to_end_and_stress() {
    let storage = temp_dir("e2e");
    let catalog = Arc::new(Catalog::open(storage.join("catalog.json")).unwrap());
    let max_upload = 64 * 1024 * 1024;
    let (url, code, handle) = spawn(Config {
        device_name: "scenario-pc".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: max_upload,
        version: "0.1.0".into(),
        catalog: Some(catalog.clone()),
        progress: None,
        require_pairing: true,
    })
    .await;

    let client = cookie_client();
    let mut report: Vec<String> = Vec::new();

    // —— 场景 1：鉴权 ——
    let unauth = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(unauth, reqwest::StatusCode::UNAUTHORIZED);
    let bad = pair(&client, &url, "000000").await;
    assert_eq!(bad, reqwest::StatusCode::UNAUTHORIZED);
    let ok = pair(&client, &url, &code).await;
    assert_eq!(ok, reqwest::StatusCode::NO_CONTENT);
    report.push("S1 鉴权: 未登录401 / 错码401 / 正确配对204 — PASS".into());

    // —— 场景 2：小文件上传下载删除 ——
    let small = payload(4 * 1024, 11);
    let expect_hash = sha256_hex(&small);
    let (status, json) = multipart_upload(&client, &url, "note.bin", small.clone()).await;
    assert_eq!(status, reqwest::StatusCode::CREATED);
    let id = json["files"][0]["id"].as_str().unwrap().to_string();
    let body = client
        .get(format!("{url}/api/files/{id}/download"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(sha256_hex(&body), expect_hash);
    report.push(format!(
        "S2 小文件闭环: 上传/下载 SHA256 一致 ({} bytes) — PASS",
        body.len()
    ));

    // —— 场景 3：Range 下载 ——
    let mid = client
        .get(format!("{url}/api/files/{id}/download"))
        .header("Range", "bytes=100-199")
        .send()
        .await
        .unwrap();
    assert_eq!(mid.status(), reqwest::StatusCode::PARTIAL_CONTENT);
    let part = mid.bytes().await.unwrap();
    assert_eq!(part.len(), 100);
    assert_eq!(&part[..], &small[100..200]);
    report.push("S3 Range 下载: 100-199 字节切片正确 — PASS".into());

    // —— 场景 4：并发上传压力（16 个 256KiB）——
    let start = Instant::now();
    let mut tasks = Vec::new();
    for index in 0..16u32 {
        let client = client.clone();
        let url = url.clone();
        let data = payload(256 * 1024, index as u8 + 1);
        let name = format!("batch-{index:02}.bin");
        tasks.push(tokio::spawn(async move {
            multipart_upload(&client, &url, &name, data).await
        }));
    }
    let mut ok_count = 0;
    for task in tasks {
        let (status, json) = task.await.unwrap();
        assert_eq!(status, reqwest::StatusCode::CREATED);
        assert!(json["files"][0]["name"].is_string());
        ok_count += 1;
    }
    let elapsed = start.elapsed();
    assert_eq!(ok_count, 16);
    let listing: serde_json::Value = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let total = listing["files"].as_array().map(|a| a.len()).unwrap_or(0);
    assert!(total >= 16, "list should contain batch files, got {total}");
    report.push(format!(
        "S4 并发上传压力: 16×256KiB 全部 201，列表 ≥{total}，耗时 {:.0} ms — PASS",
        elapsed.as_millis()
    ));

    // —— 场景 5：大文件 8MiB + 并行 Range 完整性 ——
    let large = payload(8 * 1024 * 1024, 77);
    let large_hash = sha256_hex(&large);
    let start = Instant::now();
    let (status, json) = multipart_upload(&client, &url, "large.bin", large.clone()).await;
    assert_eq!(status, reqwest::StatusCode::CREATED);
    let large_id = json["files"][0]["id"].as_str().unwrap().to_string();
    let upload_ms = start.elapsed().as_millis();

    let chunk = 2 * 1024 * 1024;
    let mut parts = Vec::new();
    for i in 0..4u64 {
        let client = client.clone();
        let url = url.clone();
        let id = large_id.clone();
        let start_b = i * chunk;
        let end_b = start_b + chunk - 1;
        parts.push(tokio::spawn(async move {
            let resp = client
                .get(format!("{url}/api/files/{id}/download"))
                .header("Range", format!("bytes={start_b}-{end_b}"))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), reqwest::StatusCode::PARTIAL_CONTENT);
            resp.bytes().await.unwrap()
        }));
    }
    let mut joined = Vec::with_capacity(large.len());
    for part in parts {
        joined.extend_from_slice(&part.await.unwrap());
    }
    assert_eq!(sha256_hex(&joined), large_hash);
    report.push(format!(
        "S5 大文件+并行Range: 8MiB 上传 {upload_ms} ms，4 段拼接 SHA256 一致 — PASS"
    ));

    // —— 场景 6：共享文件夹 → 流式 ZIP ——
    let share_dir = temp_dir("shared-folder");
    std::fs::create_dir_all(share_dir.join("sub")).unwrap();
    std::fs::write(share_dir.join("a.txt"), b"alpha").unwrap();
    std::fs::write(share_dir.join("sub/b.txt"), b"beta-data").unwrap();
    catalog
        .add_linked(&[share_dir.to_string_lossy().into_owned()])
        .unwrap();
    let listing: serde_json::Value = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let files = listing["files"].as_array().unwrap().clone();
    let folder = files
        .iter()
        .find(|f| f["is_dir"] == serde_json::json!(true))
        .expect("shared folder in list");
    let folder_id = folder["id"].as_str().unwrap().to_string();
    let zip_resp = client
        .get(format!("{url}/api/files/{folder_id}/download"))
        .send()
        .await
        .unwrap();
    assert_eq!(zip_resp.status(), reqwest::StatusCode::OK);
    assert_eq!(
        zip_resp.headers().get("content-type").unwrap(),
        "application/zip"
    );
    let zip_bytes = zip_resp.bytes().await.unwrap();
    assert!(zip_bytes.len() > 100, "zip should not be empty");
    // 本地用 zip crate 解析（与 e2e 相同）
    let cursor = std::io::Cursor::new(&zip_bytes[..]);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut names = Vec::new();
    for i in 0..archive.len() {
        names.push(archive.by_index(i).unwrap().name().to_string());
    }
    assert!(
        names
            .iter()
            .any(|n| n.ends_with("a.txt") || n.ends_with("sub/b.txt") || n.contains("a.txt")),
        "zip entries missing content, got {names:?}"
    );
    report.push(format!(
        "S6 文件夹流式ZIP: {} 条目，包体 {} bytes — PASS",
        names.len(),
        zip_bytes.len()
    ));

    // —— 场景 7：inbox 本地文件 ——
    let inbox_file = storage.join("local-on-disk.bin");
    std::fs::write(&inbox_file, b"not-uploaded").unwrap();
    catalog.set_inbox_dir(Some(storage.clone()));
    let listing: serde_json::Value = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let files = listing["files"].as_array().unwrap();
    let inbox_item = files
        .iter()
        .find(|f| f["name"] == "local-on-disk.bin")
        .expect("inbox file listed");
    assert_eq!(inbox_item["source_type"], serde_json::json!("inbox"));
    let inbox_id = inbox_item["id"].as_str().unwrap().to_string();
    let body = client
        .get(format!("{url}/api/files/{inbox_id}/download"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(&body[..], b"not-uploaded");
    // 关闭 inbox 后不再出现
    catalog.set_inbox_dir(None);
    let listing: serde_json::Value = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let still = listing["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["name"] == "local-on-disk.bin");
    assert!(!still);
    report.push("S7 inbox 本地文件: 列表/下载/关闭后隐藏 — PASS".into());

    // —— 场景 8：分块上传续传 ——
    #[derive(serde::Deserialize)]
    struct CreateSession {
        id: String,
        chunk_size: u64,
        total_chunks: u32,
        #[allow(dead_code)]
        received_chunks: Vec<u32>,
    }
    let chunk_payload = payload(600 * 1024, 42);
    let created = client
        .post(format!("{url}/api/uploads"))
        .json(&serde_json::json!({
            "file_name": "resume.bin",
            "file_size": chunk_payload.len(),
            "chunk_size": 256 * 1024,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), reqwest::StatusCode::CREATED);
    let session: CreateSession = created.json().await.unwrap();
    assert!(session.total_chunks >= 2);
    let chunk_size = session.chunk_size as usize;
    // 只上传第 0 块
    let c0 = &chunk_payload[..chunk_size];
    let h0 = sha256_hex(c0);
    let put = client
        .put(format!("{url}/api/uploads/{}/chunks/0", session.id))
        .header("X-Chunk-SHA256", &h0)
        .header("Content-Type", "application/octet-stream")
        .body(c0.to_vec())
        .send()
        .await
        .unwrap();
    assert!(
        put.status().is_success(),
        "chunk0 upload failed: {}",
        put.status()
    );
    // 查询状态，只收到 chunk 0
    let status_resp: serde_json::Value = client
        .get(format!("{url}/api/uploads/{}", session.id))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let received = status_resp["received_chunks"].as_array().unwrap();
    assert_eq!(received.len(), 1);
    // 补全剩余块
    let mut offset = chunk_size;
    let mut index = 1u32;
    while offset < chunk_payload.len() {
        let end = (offset + chunk_size).min(chunk_payload.len());
        let chunk = &chunk_payload[offset..end];
        let digest = sha256_hex(chunk);
        let put = client
            .put(format!("{url}/api/uploads/{}/chunks/{index}", session.id))
            .header("X-Chunk-SHA256", &digest)
            .header("Content-Type", "application/octet-stream")
            .body(chunk.to_vec())
            .send()
            .await
            .unwrap();
        assert!(put.status().is_success(), "chunk {index} failed");
        offset = end;
        index += 1;
    }
    let complete = client
        .post(format!("{url}/api/uploads/{}/complete", session.id))
        .send()
        .await
        .unwrap();
    assert_eq!(complete.status(), reqwest::StatusCode::CREATED);
    let completed: serde_json::Value = complete.json().await.unwrap();
    let rid = completed["files"][0]["id"].as_str().unwrap().to_string();
    let got = client
        .get(format!("{url}/api/files/{rid}/download"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(sha256_hex(&got), sha256_hex(&chunk_payload));
    report.push(format!(
        "S8 分块续传: {} 分块，中断后补齐并提交，内容哈希一致 — PASS",
        session.total_chunks
    ));

    // —— 场景 9：批量文件列表压力（200 小文件）——
    let batch_dir = temp_dir("batch200");
    catalog.set_inbox_dir(None);
    // 用 multipart 上传 30 个（更快），再加 inbox 200 个目录文件测列表
    for i in 0..30 {
        let (status, _) = multipart_upload(
            &client,
            &url,
            &format!("stress-{i:03}.txt"),
            payload(1024, i as u8),
        )
        .await;
        assert_eq!(status, reqwest::StatusCode::CREATED);
    }
    for i in 0..200 {
        std::fs::write(
            batch_dir.join(format!("dir-file-{i:03}.dat")),
            payload(64, i as u8),
        )
        .unwrap();
    }
    catalog.set_inbox_dir(Some(batch_dir.clone()));
    let start = Instant::now();
    let listing: serde_json::Value = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let list_ms = start.elapsed().as_millis();
    let count = listing["files"].as_array().unwrap().len();
    assert!(count >= 200, "expected >=200 items, got {count}");
    // 第二次列表应命中 TTL 缓存，不应明显变慢（宽松断言：仍在 2s 内）
    let start = Instant::now();
    let _: serde_json::Value = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cached_ms = start.elapsed().as_millis();
    assert!(cached_ms < 2_000);
    report.push(format!(
        "S9 列表压力: {count} 条，首次 {list_ms} ms，缓存后 {cached_ms} ms — PASS"
    ));

    // —— 场景 10：并行下载压力（8 路同文件）——
    let mut downloads = Vec::new();
    for _ in 0..8 {
        let client = client.clone();
        let url = url.clone();
        let id = large_id.clone();
        downloads.push(tokio::spawn(async move {
            let resp = client
                .get(format!("{url}/api/files/{id}/download"))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), reqwest::StatusCode::OK);
            resp.bytes().await.unwrap()
        }));
    }
    let start = Instant::now();
    for task in downloads {
        let bytes = task.await.unwrap();
        assert_eq!(bytes.len(), large.len());
    }
    report.push(format!(
        "S10 并行下载压力: 8×8MiB 全部完整，耗时 {:.0} ms — PASS",
        start.elapsed().as_millis()
    ));

    // —— 场景 11：错误路径 ——
    let missing = client
        .get(format!("{url}/api/files/deadbeef/download"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(missing, reqwest::StatusCode::NOT_FOUND);
    let bad_chunk = client
        .put(format!("{url}/api/uploads/nonexist/chunks/0"))
        .header("X-Chunk-SHA256", sha256_hex(b"x").as_str())
        .body(vec![b'x'])
        .send()
        .await
        .unwrap()
        .status();
    assert!(bad_chunk.is_client_error() || bad_chunk == reqwest::StatusCode::NOT_FOUND);
    report.push(format!(
        "S11 错误路径: 缺失文件404，无效会话 {bad_chunk} — PASS"
    ));

    handle.abort();
    let _ = std::fs::remove_dir_all(&storage);
    let _ = std::fs::remove_dir_all(&share_dir);
    let _ = std::fs::remove_dir_all(&batch_dir);

    println!("\n========== PacketBoat 场景测试报告 ==========");
    for line in &report {
        println!("{line}");
    }
    println!("==============================================\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scenario_pairing_disabled_anonymous() {
    let storage = temp_dir("nopair");
    let (url, _code, handle) = spawn(Config {
        device_name: "open-pc".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024 * 1024,
        version: "0.1.0".into(),
        catalog: None,
        progress: None,
        require_pairing: false,
    })
    .await;
    let client = reqwest::Client::new();
    let status = client
        .get(format!("{url}/api/files"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::OK);
    let info: serde_json::Value = client
        .get(format!("{url}/api/info"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info["requires_auth"], serde_json::json!(false));
    println!("S12 免配对模式: 匿名列表 OK，requires_auth=false — PASS");
    handle.abort();
    let _ = std::fs::remove_dir_all(&storage);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scenario_disk_precheck_rejects_oversized() {
    let storage = temp_dir("limit");
    let (url, code, handle) = spawn(Config {
        device_name: "limited".into(),
        storage_dir: storage.clone(),
        max_upload_bytes: 1024, // 1KiB 上限
        version: "0.1.0".into(),
        catalog: None,
        progress: None,
        require_pairing: true,
    })
    .await;
    let client = cookie_client();
    assert_eq!(
        pair(&client, &url, &code).await,
        reqwest::StatusCode::NO_CONTENT
    );
    let (status, _) = multipart_upload(&client, &url, "too-big.bin", payload(4096, 1)).await;
    assert_eq!(status, reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    println!("S13 超限上传: 返回 413 — PASS");
    handle.abort();
    let _ = std::fs::remove_dir_all(&storage);
}
