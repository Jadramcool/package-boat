# Changelog

本项目遵循 [Semantic Versioning](https://semver.org/)。

## [Unreleased]

### Added

- 浏览器分块上传、连接中断后的缺块续传、失败重试和逐块 SHA-256 完整性校验。
- 受认证保护的上传会话 API，支持进度查询、幂等块重试、完成提交和取消清理。
- 上传队列实时速度与预计剩余时间，弱网停滞时会清除过期估算。
- 基于 Node.js 内置测试运行器的前端传输指标单元测试，并接入 CI。
- 桌面端一键清空共享清单，清空操作只移除记录，不删除磁盘文件。

### Changed

- 中断恢复基准改为验证实际的分块续传路径和恢复阶段传输量。
- 接收目录不再在服务启动时被批量扫描为共享文件；升级时会移除旧版自动导入记录，但保留磁盘文件。
- 按 Tauri 2 官方规范对齐桌面端工程：启用 CSP（`csp`/`devCsp`），`devUrl` 改用 `127.0.0.1`，显式声明窗口 `label`；前端迁移到顶层 `apps/desktop/`；`lib.rs` 拆分为 commands/state/events/error/tray/window 模块并以 `thiserror` 统一错误类型；接入 `tauri-specta` 从 Rust 生成 `bindings.ts`（命令、事件与数据类型全量类型安全），替换手写 `types.ts`；CI 新增版本一致性校验与 `tauri build --no-bundle` 构建验证。
- 桌面端配置去重与安全收紧：`tauri.conf.json` 省略 `version` 字段改为自动继承 Cargo 包版本（`check-versions` 与 Release 工作流同步移除该校验点）；移除前端多余的 `@tauri-apps/cli` 依赖；CSP 移除未使用的 `asset:`/`customprotocol:` 来源；CI 新增 `cargo-deny` 依赖审计（许可证、安全公告与来源校验）。
- 升级传递依赖 `h2` 至 0.4.19，消除 RUSTSEC-2026-0258（无界空 DATA 帧队列）安全公告。
- 前端接入 ESLint 10（`eslint-plugin-vue` essential + TypeScript recommended），存量代码零违规，CI 新增 lint 门禁。

### Removed

- 移除独立命令行服务端（`laneshare` CLI），仅保留 Windows 桌面端。

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
