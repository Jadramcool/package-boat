//! End-to-end transfer benchmark suite.
//!
//! Run with:
//! `cargo bench -p packetboat-core --bench transfer -- --profile smoke`

use axum::body::Bytes;
use futures_util::future::join_all;
use futures_util::StreamExt;
use packetboat_core::server::{Config as ServerConfig, Server};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, RANGE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MIB: u64 = 1024 * 1024;
const STREAM_CHUNK_SIZE: usize = 256 * 1024;
const BENCHMARK_VERSION: u32 = 2;

type BenchResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone, Serialize)]
struct BenchConfig {
    profile: String,
    large_mib: u64,
    weak_mib: u64,
    weak_mbps: f64,
    weak_latency_ms: u64,
    weak_jitter_ms: u64,
    concurrent_files: usize,
    concurrent_mib: u64,
    range_workers: usize,
    interrupted_mib: u64,
    #[serde(skip_serializing)]
    output_dir: PathBuf,
}

impl BenchConfig {
    fn from_args() -> BenchResult<Self> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.iter().any(|arg| arg == "-h" || arg == "--help") {
            print_help();
            std::process::exit(0);
        }

        let profile = argument_value(&args, "--profile").unwrap_or_else(|| "standard".into());
        let output_dir = argument_value(&args, "--output-dir")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/benchmarks"));
        let output_dir = if output_dir.is_absolute() {
            output_dir
        } else {
            workspace_root().join(output_dir)
        };
        reject_unknown_args(&args)?;

        let mut config = match profile.as_str() {
            "smoke" => Self {
                profile: profile.clone(),
                large_mib: 8,
                weak_mib: 2,
                weak_mbps: 20.0,
                weak_latency_ms: 20,
                weak_jitter_ms: 10,
                concurrent_files: 2,
                concurrent_mib: 4,
                range_workers: 2,
                interrupted_mib: 4,
                output_dir,
            },
            "standard" => Self {
                profile: profile.clone(),
                large_mib: 128,
                weak_mib: 8,
                weak_mbps: 20.0,
                weak_latency_ms: 80,
                weak_jitter_ms: 40,
                concurrent_files: 4,
                concurrent_mib: 32,
                range_workers: 4,
                interrupted_mib: 16,
                output_dir,
            },
            "stress" => Self {
                profile: profile.clone(),
                large_mib: 1024,
                weak_mib: 64,
                weak_mbps: 10.0,
                weak_latency_ms: 150,
                weak_jitter_ms: 100,
                concurrent_files: 8,
                concurrent_mib: 128,
                range_workers: 8,
                interrupted_mib: 128,
                output_dir,
            },
            other => return Err(failure(format!("unknown profile {other:?}"))),
        };

        override_u64("PACKETBOAT_BENCH_LARGE_MIB", &mut config.large_mib)?;
        override_u64("PACKETBOAT_BENCH_WEAK_MIB", &mut config.weak_mib)?;
        override_f64("PACKETBOAT_BENCH_WEAK_MBPS", &mut config.weak_mbps)?;
        override_u64(
            "PACKETBOAT_BENCH_WEAK_LATENCY_MS",
            &mut config.weak_latency_ms,
        )?;
        override_u64(
            "PACKETBOAT_BENCH_WEAK_JITTER_MS",
            &mut config.weak_jitter_ms,
        )?;
        override_usize(
            "PACKETBOAT_BENCH_CONCURRENT_FILES",
            &mut config.concurrent_files,
        )?;
        override_u64(
            "PACKETBOAT_BENCH_CONCURRENT_MIB",
            &mut config.concurrent_mib,
        )?;
        override_usize("PACKETBOAT_BENCH_RANGE_WORKERS", &mut config.range_workers)?;
        override_u64(
            "PACKETBOAT_BENCH_INTERRUPTED_MIB",
            &mut config.interrupted_mib,
        )?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> BenchResult<()> {
        if self.large_mib == 0
            || self.weak_mib == 0
            || self.concurrent_mib == 0
            || self.interrupted_mib == 0
            || self.concurrent_files == 0
            || self.range_workers == 0
            || !self.weak_mbps.is_finite()
            || self.weak_mbps <= 0.0
        {
            return Err(failure("benchmark values must be positive"));
        }
        Ok(())
    }

    fn max_payload_bytes(&self) -> u64 {
        [
            self.large_mib,
            self.weak_mib,
            self.concurrent_mib,
            self.interrupted_mib,
        ]
        .into_iter()
        .max()
        .unwrap_or(1)
            * MIB
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("packetboat-core must be inside the workspace crates directory")
        .to_path_buf()
}

#[derive(Debug, Serialize)]
struct Measurement {
    scenario: String,
    payload_bytes: u64,
    elapsed_seconds: f64,
    throughput_mib_per_second: f64,
    notes: String,
}

impl Measurement {
    fn new(
        scenario: impl Into<String>,
        payload_bytes: u64,
        elapsed: Duration,
        notes: impl Into<String>,
    ) -> Self {
        let elapsed_seconds = elapsed.as_secs_f64().max(f64::EPSILON);
        Self {
            scenario: scenario.into(),
            payload_bytes,
            elapsed_seconds,
            throughput_mib_per_second: payload_bytes as f64 / MIB as f64 / elapsed_seconds,
            notes: notes.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    generated_at: String,
    packetboat_version: String,
    os: String,
    arch: String,
    logical_cpus: usize,
    config: BenchConfig,
    measurements: Vec<Measurement>,
}

#[derive(Debug, serde::Deserialize)]
struct FileRecord {
    id: String,
    size: i64,
}

#[derive(Debug, serde::Deserialize)]
struct UploadResponse {
    files: Vec<FileRecord>,
}

#[derive(Debug, Deserialize)]
struct ChunkUploadSession {
    id: String,
    file_size: u64,
    chunk_size: u64,
    total_chunks: u32,
    received_chunks: Vec<u32>,
    uploaded_bytes: u64,
}

struct BenchServer {
    base_url: String,
    access_code: String,
    _storage: tempfile::TempDir,
    task: tokio::task::JoinHandle<()>,
}

impl BenchServer {
    async fn start(max_upload_bytes: u64) -> BenchResult<Self> {
        let storage = tempfile::tempdir()?;
        let storage_path = storage.path().to_path_buf();
        let server = Server::new(ServerConfig {
            device_name: "PacketBoat benchmark".into(),
            storage_dir: storage_path.clone(),
            max_upload_bytes: i64::try_from(max_upload_bytes)?,
            version: env!("CARGO_PKG_VERSION").into(),
            catalog: None,
            progress: None,
        })
        .map_err(failure)?;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let access_code = server.access_code();
        let router = server.router();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            {
                eprintln!("benchmark server failed: {error}");
            }
        });
        let base_url = format!("http://{address}");
        Ok(Self {
            base_url,
            access_code,
            _storage: storage,
            task,
        })
    }
}

impl Drop for BenchServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Copy)]
struct UploadSpec {
    payload_bytes: u64,
    transmitted_bytes: u64,
    network: Option<NetworkProfile>,
    pattern: u8,
    complete_multipart: bool,
}

#[derive(Clone, Copy)]
struct NetworkProfile {
    bytes_per_second: f64,
    latency: Duration,
    jitter: Duration,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("transfer benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> BenchResult<()> {
    let config = BenchConfig::from_args()?;
    let max_upload_bytes = config.max_payload_bytes().saturating_add(MIB);
    let server = BenchServer::start(max_upload_bytes).await?;
    let client = benchmark_client()?;
    pair(&client, &server.base_url, &server.access_code).await?;

    println!("PacketBoat transfer benchmark ({})", config.profile);
    println!("server: {}", server.base_url);

    let mut measurements = Vec::new();

    let large_bytes = config.large_mib * MIB;
    let started = Instant::now();
    let large = upload_file(
        &client,
        &server.base_url,
        "benchmark-large.bin",
        UploadSpec::complete(large_bytes, 0xA5, None),
    )
    .await?;
    validate_size(&large, large_bytes)?;
    let elapsed = started.elapsed();
    print_measurement("large upload", large_bytes, elapsed);
    measurements.push(Measurement::new(
        "large_upload",
        large_bytes,
        elapsed,
        "single streamed multipart upload over loopback",
    ));

    let weak_bytes = config.weak_mib * MIB;
    let weak_rate = config.weak_mbps * 1_000_000.0 / 8.0;
    let started = Instant::now();
    let weak = upload_file(
        &client,
        &server.base_url,
        "benchmark-weak.bin",
        UploadSpec::complete(
            weak_bytes,
            0x5A,
            Some(NetworkProfile {
                bytes_per_second: weak_rate,
                latency: Duration::from_millis(config.weak_latency_ms),
                jitter: Duration::from_millis(config.weak_jitter_ms),
            }),
        ),
    )
    .await?;
    validate_size(&weak, weak_bytes)?;
    let elapsed = started.elapsed();
    print_measurement("weak-network upload", weak_bytes, elapsed);
    measurements.push(Measurement::new(
        "weak_network_upload",
        weak_bytes,
        elapsed,
        format!(
            "{:.2} Mbit/s, {} ms initial latency, up to {} ms deterministic jitter",
            config.weak_mbps, config.weak_latency_ms, config.weak_jitter_ms
        ),
    ));

    let concurrent_each = config.concurrent_mib * MIB;
    let started = Instant::now();
    let uploads = (0..config.concurrent_files).map(|index| {
        let client = client.clone();
        let base_url = server.base_url.clone();
        async move {
            upload_file(
                &client,
                &base_url,
                &format!("benchmark-concurrent-{index}.bin"),
                UploadSpec::complete(concurrent_each, index as u8, None),
            )
            .await
        }
    });
    let results = join_all(uploads).await;
    for result in results {
        validate_size(&result?, concurrent_each)?;
    }
    let concurrent_total = concurrent_each * config.concurrent_files as u64;
    let elapsed = started.elapsed();
    print_measurement("concurrent uploads", concurrent_total, elapsed);
    measurements.push(Measurement::new(
        "concurrent_uploads",
        concurrent_total,
        elapsed,
        format!(
            "{} simultaneous files, {} MiB each",
            config.concurrent_files, config.concurrent_mib
        ),
    ));

    let started = Instant::now();
    let downloaded = parallel_range_download(
        &client,
        &server.base_url,
        &large.id,
        large_bytes,
        config.range_workers,
        0xA5,
    )
    .await?;
    if downloaded != large_bytes {
        return Err(failure(format!(
            "range download size mismatch: {downloaded} != {large_bytes}"
        )));
    }
    let elapsed = started.elapsed();
    print_measurement("parallel Range download", downloaded, elapsed);
    measurements.push(Measurement::new(
        "parallel_range_download",
        downloaded,
        elapsed,
        format!("{} concurrent byte ranges", config.range_workers),
    ));

    let interrupted_bytes = config.interrupted_mib * MIB;
    let requested_chunk_size = (interrupted_bytes / 4).clamp(
        packetboat_core::uploads::MIN_CHUNK_SIZE,
        packetboat_core::uploads::MAX_CHUNK_SIZE,
    );
    let session = create_chunk_upload(
        &client,
        &server.base_url,
        "benchmark-interrupted.bin",
        interrupted_bytes,
        requested_chunk_size,
    )
    .await?;
    let initial_chunks = (session.total_chunks / 3).max(1);
    for index in 0..initial_chunks {
        upload_pattern_chunk(&client, &server.base_url, &session, index, 0x3C).await?;
    }
    // 用新的状态查询模拟客户端断线后重新连接；恢复阶段只发送缺失块。
    let resumed = get_chunk_upload(&client, &server.base_url, &session.id).await?;
    if resumed.received_chunks.len() != initial_chunks as usize {
        return Err(failure("resumable session did not retain uploaded chunks"));
    }
    let reused_bytes = resumed.uploaded_bytes;
    let recovery_bytes = interrupted_bytes.saturating_sub(reused_bytes);
    let started = Instant::now();
    for index in initial_chunks..resumed.total_chunks {
        upload_pattern_chunk(&client, &server.base_url, &resumed, index, 0x3C).await?;
    }
    let retry = complete_chunk_upload(&client, &server.base_url, &resumed.id).await?;
    validate_size(&retry, interrupted_bytes)?;
    let elapsed = started.elapsed();
    print_measurement("resumable upload recovery", recovery_bytes, elapsed);
    measurements.push(Measurement::new(
        "resumable_upload_recovery",
        recovery_bytes,
        elapsed,
        format!(
            "{} MiB total, {:.2} MiB retained; only missing chunks transferred after reconnect",
            config.interrupted_mib,
            reused_bytes as f64 / MIB as f64,
        ),
    ));

    let report = BenchmarkReport {
        schema_version: BENCHMARK_VERSION,
        generated_at: chrono::Utc::now().to_rfc3339(),
        packetboat_version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        logical_cpus: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        config: config.clone(),
        measurements,
    };
    write_reports(&report, &config.output_dir)?;
    Ok(())
}

impl UploadSpec {
    fn complete(payload_bytes: u64, pattern: u8, network: Option<NetworkProfile>) -> Self {
        Self {
            payload_bytes,
            transmitted_bytes: payload_bytes,
            network,
            pattern,
            complete_multipart: true,
        }
    }
}

fn benchmark_client() -> BenchResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(60 * 60))
        .build()?)
}

async fn pair(client: &reqwest::Client, base_url: &str, access_code: &str) -> BenchResult<()> {
    let response = client
        .post(format!("{base_url}/api/session"))
        .json(&serde_json::json!({ "code": access_code }))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(failure(format!("pairing failed: {}", response.status())));
    }
    Ok(())
}

async fn upload_file(
    client: &reqwest::Client,
    base_url: &str,
    filename: &str,
    spec: UploadSpec,
) -> BenchResult<FileRecord> {
    let response = send_upload(client, base_url, filename, spec).await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(failure(format!("upload failed with {status}: {body}")));
    }
    let payload: UploadResponse = response.json().await?;
    payload
        .files
        .into_iter()
        .next()
        .ok_or_else(|| failure("upload response did not contain a file"))
}

async fn send_upload(
    client: &reqwest::Client,
    base_url: &str,
    filename: &str,
    spec: UploadSpec,
) -> Result<reqwest::Response, reqwest::Error> {
    let boundary = "packetboat-benchmark-boundary";
    let prefix = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    );
    let suffix = format!("\r\n--{boundary}--\r\n");
    let declared_length = prefix.len() as u64 + spec.payload_bytes + suffix.len() as u64;
    let prefix = Bytes::from(prefix);
    let suffix = Bytes::from(suffix);
    let template = Bytes::from(vec![spec.pattern; STREAM_CHUNK_SIZE]);
    let stream = async_stream::stream! {
        yield Ok::<Bytes, std::io::Error>(prefix);
        let started = Instant::now();
        let mut sent = 0u64;
        let mut chunk_index = 0u64;
        while sent < spec.transmitted_bytes {
            if let Some(network) = spec.network {
                // Deterministic jitter keeps benchmark runs comparable while still exercising
                // uneven body delivery. It is scheduled around the target rate, not accumulated.
                let jitter_ratio = ((chunk_index.wrapping_mul(1_103_515_245).wrapping_add(12_345))
                    % 1_000) as f64
                    / 999.0;
                let jitter = network.jitter.mul_f64(jitter_ratio);
                let target = network.latency
                    + Duration::from_secs_f64(sent as f64 / network.bytes_per_second)
                    + jitter;
                if target > started.elapsed() {
                    tokio::time::sleep(target - started.elapsed()).await;
                }
            }
            let remaining = spec.transmitted_bytes - sent;
            let length = remaining.min(STREAM_CHUNK_SIZE as u64) as usize;
            yield Ok(template.slice(..length));
            sent += length as u64;
            chunk_index += 1;
        }
        if let Some(network) = spec.network {
            let target = network.latency
                + Duration::from_secs_f64(sent as f64 / network.bytes_per_second);
            if target > started.elapsed() {
                tokio::time::sleep(target - started.elapsed()).await;
            }
        }
        if spec.complete_multipart {
            yield Ok(suffix);
        }
    };

    client
        .post(format!("{base_url}/api/files"))
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(CONTENT_LENGTH, declared_length)
        .body(reqwest::Body::wrap_stream(stream))
        .send()
        .await
}

async fn create_chunk_upload(
    client: &reqwest::Client,
    base_url: &str,
    filename: &str,
    file_size: u64,
    chunk_size: u64,
) -> BenchResult<ChunkUploadSession> {
    let response = client
        .post(format!("{base_url}/api/uploads"))
        .json(&serde_json::json!({
            "file_name": filename,
            "file_size": file_size,
            "chunk_size": chunk_size,
        }))
        .send()
        .await?;
    parse_success(response, "create chunk upload").await
}

async fn get_chunk_upload(
    client: &reqwest::Client,
    base_url: &str,
    id: &str,
) -> BenchResult<ChunkUploadSession> {
    let response = client
        .get(format!("{base_url}/api/uploads/{id}"))
        .send()
        .await?;
    parse_success(response, "query chunk upload").await
}

async fn upload_pattern_chunk(
    client: &reqwest::Client,
    base_url: &str,
    session: &ChunkUploadSession,
    index: u32,
    pattern: u8,
) -> BenchResult<()> {
    let offset = index as u64 * session.chunk_size;
    let length = session
        .chunk_size
        .min(session.file_size.saturating_sub(offset));
    let body = vec![pattern; usize::try_from(length)?];
    let checksum = format!("{:x}", Sha256::digest(&body));
    let response = client
        .put(format!(
            "{base_url}/api/uploads/{}/chunks/{index}",
            session.id
        ))
        .header("X-Chunk-SHA256", checksum)
        .body(body)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(failure(format!(
            "upload chunk failed with {status}: {body}"
        )));
    }
    Ok(())
}

async fn complete_chunk_upload(
    client: &reqwest::Client,
    base_url: &str,
    id: &str,
) -> BenchResult<FileRecord> {
    let response = client
        .post(format!("{base_url}/api/uploads/{id}/complete"))
        .send()
        .await?;
    let payload: UploadResponse = parse_success(response, "complete chunk upload").await?;
    payload
        .files
        .into_iter()
        .next()
        .ok_or_else(|| failure("chunk upload response did not contain a file"))
}

async fn parse_success<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    operation: &str,
) -> BenchResult<T> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(failure(format!("{operation} failed with {status}: {body}")));
    }
    Ok(response.json().await?)
}

async fn parallel_range_download(
    client: &reqwest::Client,
    base_url: &str,
    id: &str,
    total_bytes: u64,
    workers: usize,
    expected_pattern: u8,
) -> BenchResult<u64> {
    let range_size = total_bytes.div_ceil(workers as u64);
    let requests = (0..workers).filter_map(|index| {
        let start = index as u64 * range_size;
        if start >= total_bytes {
            return None;
        }
        let end = (start + range_size - 1).min(total_bytes - 1);
        let expected = end - start + 1;
        let client = client.clone();
        let url = format!("{base_url}/api/files/{id}/download");
        Some(async move {
            let response = client
                .get(url)
                .header(RANGE, format!("bytes={start}-{end}"))
                .send()
                .await?;
            if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(failure(format!(
                    "range {start}-{end} returned {}",
                    response.status()
                )));
            }
            let mut received = 0u64;
            let mut body = response.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                if let (Some(first), Some(last)) = (chunk.first(), chunk.last()) {
                    if *first != expected_pattern || *last != expected_pattern {
                        return Err(failure(format!("range {start}-{end} content mismatch")));
                    }
                }
                received += chunk.len() as u64;
            }
            if received != expected {
                return Err(failure(format!(
                    "range {start}-{end} size mismatch: {received} != {expected}"
                )));
            }
            Ok::<u64, Box<dyn Error + Send + Sync>>(received)
        })
    });

    let mut total = 0u64;
    for result in join_all(requests).await {
        total += result?;
    }
    Ok(total)
}

fn validate_size(file: &FileRecord, expected: u64) -> BenchResult<()> {
    if file.size != i64::try_from(expected)? {
        return Err(failure(format!(
            "uploaded size mismatch: {} != {expected}",
            file.size
        )));
    }
    Ok(())
}

fn write_reports(report: &BenchmarkReport, output_dir: &Path) -> BenchResult<()> {
    std::fs::create_dir_all(output_dir)?;
    let json_path = output_dir.join("transfer-report.json");
    let markdown_path = output_dir.join("transfer-report.md");
    std::fs::write(&json_path, serde_json::to_vec_pretty(report)?)?;
    let markdown = render_markdown(report);
    std::fs::write(&markdown_path, &markdown)?;
    println!();
    println!("{markdown}");
    println!("JSON report: {}", json_path.display());
    println!("Markdown report: {}", markdown_path.display());
    Ok(())
}

fn render_markdown(report: &BenchmarkReport) -> String {
    let mut output = format!(
        "# PacketBoat transfer benchmark\n\n- Generated: `{}`\n- Version: `{}`\n- Profile: `{}`\n- Platform: `{}/{}`\n- Logical CPUs: `{}`\n\n| Scenario | Payload | Elapsed | Throughput | Notes |\n|---|---:|---:|---:|---|\n",
        report.generated_at,
        report.packetboat_version,
        report.config.profile,
        report.os,
        report.arch,
        report.logical_cpus,
    );
    for measurement in &report.measurements {
        output.push_str(&format!(
            "| {} | {:.2} MiB | {:.3} s | {:.2} MiB/s | {} |\n",
            measurement.scenario,
            measurement.payload_bytes as f64 / MIB as f64,
            measurement.elapsed_seconds,
            measurement.throughput_mib_per_second,
            measurement.notes.replace('|', "\\|")
        ));
    }
    output
}

fn print_measurement(name: &str, bytes: u64, elapsed: Duration) {
    let mib = bytes as f64 / MIB as f64;
    let speed = mib / elapsed.as_secs_f64().max(f64::EPSILON);
    println!(
        "{name}: {mib:.2} MiB in {:.3} s ({speed:.2} MiB/s)",
        elapsed.as_secs_f64()
    );
}

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn reject_unknown_args(args: &[String]) -> BenchResult<()> {
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            // `cargo bench` appends this harness flag even when `harness = false`.
            "--bench" => index += 1,
            "--profile" | "--output-dir" => {
                if index + 1 >= args.len() {
                    return Err(failure(format!("{} requires a value", args[index])));
                }
                index += 2;
            }
            other => return Err(failure(format!("unknown argument {other:?}"))),
        }
    }
    Ok(())
}

fn override_u64(name: &str, target: &mut u64) -> BenchResult<()> {
    if let Some(value) = std::env::var_os(name) {
        *target = value.to_string_lossy().parse()?;
    }
    Ok(())
}

fn override_usize(name: &str, target: &mut usize) -> BenchResult<()> {
    if let Some(value) = std::env::var_os(name) {
        *target = value.to_string_lossy().parse()?;
    }
    Ok(())
}

fn override_f64(name: &str, target: &mut f64) -> BenchResult<()> {
    if let Some(value) = std::env::var_os(name) {
        *target = value.to_string_lossy().parse()?;
    }
    Ok(())
}

fn failure(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}

fn print_help() {
    println!(
        "PacketBoat transfer benchmark\n\n\
         Usage: cargo bench -p packetboat-core --bench transfer -- [options]\n\n\
         Options:\n  \
           --profile <smoke|standard|stress>  Workload preset (default: standard)\n  \
           --output-dir <path>                Report directory (default: target/benchmarks)\n\n\
         Environment overrides:\n  \
           PACKETBOAT_BENCH_LARGE_MIB\n  \
           PACKETBOAT_BENCH_WEAK_MIB\n  \
           PACKETBOAT_BENCH_WEAK_MBPS\n  \
           PACKETBOAT_BENCH_WEAK_LATENCY_MS\n  \
           PACKETBOAT_BENCH_WEAK_JITTER_MS\n  \
           PACKETBOAT_BENCH_CONCURRENT_FILES\n  \
           PACKETBOAT_BENCH_CONCURRENT_MIB\n  \
           PACKETBOAT_BENCH_RANGE_WORKERS\n  \
           PACKETBOAT_BENCH_INTERRUPTED_MIB"
    );
}
