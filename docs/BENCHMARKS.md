# Transfer benchmarks

LANE 的传输基准是一个独立的端到端程序：它启动真实 Axum 服务，通过 HTTP 完成配对、multipart 上传和 Range 下载，并在临时接收目录中执行真实文件写入与同步。

## 场景

| 场景 | 验证内容 |
|---|---|
| `large_upload` | 单个大文件流式上传与落盘吞吐 |
| `weak_network_upload` | 客户端按目标带宽、初始延迟和确定性抖动发送请求体 |
| `concurrent_uploads` | 多个独立 multipart 请求并发上传的聚合吞吐 |
| `parallel_range_download` | 把大文件切成多个并行 Range，校验响应长度和内容 |
| `resumable_upload_recovery` | 上传部分块后重新查询会话，确认恢复阶段只发送缺失块并正确提交 |

弱网模型发生在客户端请求体生产端，不需要管理员权限或系统级流量整形。它能稳定重现低带宽、首包延迟和不均匀分块，但不模拟真实网络中的重传、乱序或操作系统 TCP 队列拥塞。

## 运行

快速验证：

```powershell
cargo bench -p lane-core --bench transfer -- --profile smoke
```

开发机基准：

```powershell
cargo bench -p lane-core --bench transfer -- --profile standard
```

长时间、大磁盘压力测试：

```powershell
cargo bench -p lane-core --bench transfer -- --profile stress
```

| Profile | 大文件 | 弱网上传 | 并发上传 | 中断重试 |
|---|---:|---:|---:|---:|
| `smoke` | 8 MiB | 2 MiB，20 Mbit/s，20 ms 延迟 + 0–10 ms 抖动 | 2 × 4 MiB | 4 MiB |
| `standard` | 128 MiB | 8 MiB，20 Mbit/s，80 ms 延迟 + 0–40 ms 抖动 | 4 × 32 MiB | 16 MiB |
| `stress` | 1 GiB | 64 MiB，10 Mbit/s，150 ms 延迟 + 0–100 ms 抖动 | 8 × 128 MiB | 128 MiB |

报告默认写入：

- `target/benchmarks/transfer-report.md`
- `target/benchmarks/transfer-report.json`

可以通过 `--output-dir <目录>` 修改输出位置。

## 自定义弱网与负载

预设可以用环境变量覆盖：

```powershell
$env:LANE_BENCH_LARGE_MIB = "512"
$env:LANE_BENCH_WEAK_MIB = "32"
$env:LANE_BENCH_WEAK_MBPS = "5"
$env:LANE_BENCH_WEAK_LATENCY_MS = "120"
$env:LANE_BENCH_WEAK_JITTER_MS = "80"
$env:LANE_BENCH_CONCURRENT_FILES = "6"
$env:LANE_BENCH_CONCURRENT_MIB = "64"
$env:LANE_BENCH_RANGE_WORKERS = "6"
$env:LANE_BENCH_INTERRUPTED_MIB = "64"

cargo bench -p lane-core --bench transfer -- --profile standard
```

## 结果解读

- 吞吐按有效文件字节计算，不包含 multipart 边界和 HTTP 头。
- 上传结果包含文件写入、刷新、`fsync`、原子重命名和 catalog 更新耗时。断点恢复场景的吞吐只按恢复阶段实际补传的字节计算。
- Range 下载在 loopback 上运行，主要衡量服务端读取、HTTP 编码和客户端消费能力，并不等同于真实 Wi-Fi 速度。
- GitHub 托管 Runner 的硬件会变化。判断回归时应比较同一操作系统、相同 profile 的多次结果，而不是设置单次绝对阈值。
- `stress` 会产生 GiB 级临时数据，运行前应确认磁盘空间。

## 自动运行

`.github/workflows/benchmark.yml` 支持手动选择 profile，并每周运行一次 `standard`。Markdown 会写入 Job Summary，Markdown/JSON 原始报告作为 30 天构建产物保存。普通 CI 不运行长基准。
