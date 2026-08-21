# Changelog

本项目遵循 [Semantic Versioning](https://semver.org/)。

## [Unreleased]

### Added

- 浏览器分块上传、连接中断后的缺块续传、失败重试和逐块 SHA-256 完整性校验。
- 受认证保护的上传会话 API，支持进度查询、幂等块重试、完成提交和取消清理。

### Changed

- 中断恢复基准改为验证实际的分块续传路径和恢复阶段传输量。

## [0.3.0] - 2026-08-20

### Added

- 浏览器接收页完整 Vue/TypeScript 源码，以及桌面/浏览器统一构建入口。
- 根级 pnpm workspace 和嵌入资源同步脚本，CI 会校验浏览器构建产物未过期。
- 端到端传输基准，覆盖大文件、带宽/延迟/抖动弱网、并发上传、并行 Range 下载和中断恢复。
- 手动及每周 GitHub Benchmark 工作流，输出 Markdown 和 JSON 报告。
- 服务启动时回收超过 24 小时的残留上传和文件夹 ZIP 临时文件。
- 上传前检查接收目录可用空间，空间不足时返回 HTTP 507。
- 识别上传过程中发生的磁盘写满错误，并返回明确错误信息。
- GitHub 标签发布自动构建 Windows NSIS、CLI ZIP 和 SHA-256 校验清单。

### Changed

- Windows 产品名称统一为 `LANE`。
- 桌面端运行时版本改为读取 Cargo 包版本，减少版本漂移。

## [0.2.0] - 2026-08-18

### Added

- Rust workspace、CLI、Tauri 桌面端和局域网浏览器文件传输。
- 配对认证、Range 下载、文件夹 ZIP、SSE 刷新和传输进度。
- 开源文档、CI 和社区模板。
