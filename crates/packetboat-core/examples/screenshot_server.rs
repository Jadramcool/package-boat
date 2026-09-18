//! One-shot demo server for README screenshots. Not part of the product surface.
//! Seeds a temp receive dir + catalog, serves the real embedded frontend, writes
//! `target/screenshot-server.json` with {port, code, device_name}.

use packetboat_core::catalog::Catalog;
use packetboat_core::server::{Config, Server};
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

fn write_file(path: &PathBuf, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, bytes).expect("write file");
}

/// Sparse-ish placeholder with a realistic size for README screenshots.
fn write_sized(path: &PathBuf, size: u64, header: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    use std::io::{Seek, SeekFrom, Write};
    let mut f = std::fs::File::create(path).expect("create sized");
    f.write_all(header).ok();
    f.seek(SeekFrom::Start(size.saturating_sub(1))).ok();
    f.write_all(b"\0").ok();
}

#[tokio::main]
async fn main() {
    let root = std::env::temp_dir().join("packetboat-readme-shots");
    let receive = root.join("receive");
    let shared = root.join("shared");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&receive).unwrap();
    std::fs::create_dir_all(shared.join("室内平面图")).unwrap();

    // Shared originals (linked) — realistic sizes for screenshots
    write_sized(
        &shared.join("2026Q3-应收对账明细.xlsx"),
        2_411_725,
        b"PK\x03\x04",
    );
    write_sized(
        &shared.join("产品发布会素材.zip"),
        50_331_648,
        b"PK\x03\x04",
    );
    write_sized(
        &shared.join("接口对接说明-v3.docx"),
        1_887_437,
        b"PK\x03\x04",
    );
    write_sized(&shared.join("客户回访录音-0908.mp3"), 12_582_912, b"ID3");
    write_sized(
        &shared.join("室内平面图/一层平面.dwg"),
        6_710_886,
        b"AC1027",
    );
    write_file(
        &shared.join("合同扫描件-已移动.pdf"),
        b"%PDF-1.4 demo contract scan page",
    );

    // Received files under receive dir
    write_sized(
        &receive.join("安装包-PacketBoat-0.3.0.exe"),
        23_068_672,
        b"MZ",
    );
    write_sized(
        &receive.join("背景音乐清单.csv"),
        88_064,
        b"name,path\nintro,/music/intro.mp3\n",
    );
    write_sized(
        &receive.join("会议录屏_0912.mp4"),
        193_986_560,
        b"\0\0\0\x18ftypmp42",
    );

    let catalog_path = root.join("catalog.json");
    let catalog = Arc::new(Catalog::open(catalog_path).expect("catalog"));
    let linked = [
        shared.join("2026Q3-应收对账明细.xlsx"),
        shared.join("产品发布会素材.zip"),
        shared.join("接口对接说明-v3.docx"),
        shared.join("客户回访录音-0908.mp3"),
        shared.join("室内平面图"),
        shared.join("合同扫描件-已移动.pdf"),
    ];
    let paths: Vec<String> = linked
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    catalog.add_linked(&paths).expect("add linked");
    catalog
        .add_received(receive.join("安装包-PacketBoat-0.2.0.exe"))
        .ok();
    catalog.add_received(receive.join("背景音乐清单.csv")).ok();
    catalog.add_received(receive.join("会议录屏_0912.mp4")).ok();

    // After catalog registration, move one linked file away → unavailable badge
    let archived = shared.join("_archived");
    std::fs::create_dir_all(&archived).ok();
    let _ = std::fs::rename(
        shared.join("合同扫描件-已移动.pdf"),
        archived.join("合同扫描件-已移动.pdf"),
    );

    let server = Server::new(Config {
        device_name: "JAY-STUDIO".into(),
        storage_dir: receive.clone(),
        max_upload_bytes: 2 * 1024 * 1024 * 1024,
        version: env!("CARGO_PKG_VERSION").into(),
        catalog: Some(catalog.clone()),
        progress: None,
        require_pairing: true,
    })
    .expect("server");

    let listener = TcpListener::bind(("127.0.0.1", 18991))
        .await
        .expect("bind 18991");
    let port = listener.local_addr().unwrap().port();
    let code = server.access_code();
    let meta = serde_json::json!({
        "port": port,
        "code": code,
        "device_name": "JAY-STUDIO",
        "version": env!("CARGO_PKG_VERSION"),
        "receive_dir": receive.to_string_lossy(),
    });
    let meta_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshot-server.json");
    if let Some(parent) = meta_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let mut f = std::fs::File::create(&meta_path).expect("meta file");
    f.write_all(serde_json::to_string_pretty(&meta).unwrap().as_bytes())
        .unwrap();
    println!("SCREENSHOT_SERVER {}", meta);
    eprintln!("meta={}", meta_path.display());

    let router = server.router();
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("serve");
}
