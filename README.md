# LANE 局域网投递站

LANE 是一个以 Rust 编写的局域网文件分享工具。发送端只需打开浏览器、输入六位配对码，即可向主机上传文件或下载主机分享的文件；数据不经过公网中转。

> 当前版本为 `0.2.0`，主要面向 Windows 桌面端，同时提供独立命令行服务端。项目仍处于早期阶段，欢迎试用和贡献。

## 功能

- 局域网浏览器访问，无需在发送端安装客户端
- 六位配对码、会话 Cookie 与按 IP 限速
- 多文件流式上传、单文件 Range/断点下载
- 原位分享文件和文件夹，不复制源文件
- 文件夹下载时自动打包 ZIP
- SSE 实时刷新文件列表
- Windows 托盘、二维码、接收目录设置和任务栏传输进度

## 仓库结构

```text
crates/
├── lane-core/       # HTTP 服务、认证、目录表与传输逻辑
├── lane-cli/        # laneshare 命令行程序
└── lane-desktop/    # Tauri 2 Windows 桌面端与 Vue 3 UI
```

浏览器接收页目前以构建产物形式嵌入 `lane-core/assets/dist`；将其源代码纳入同一仓库是近期路线图的一部分。

## 快速开始

需要稳定版 Rust 工具链。运行命令行版本：

```powershell
cargo run -p lane-cli -- --port 8080 --name "My PC"
```

终端会显示访问地址和配对码。在同一局域网的设备上打开该地址即可使用。可用参数：

```text
--host <地址>       默认 0.0.0.0
--port <端口>       默认 8080；0 表示自动分配
--dir <目录>        文件接收目录
--max-size <GB>     单文件大小上限，默认 10
--name <名称>       设备显示名称
```

## 桌面端开发

除 Rust 外，需要 Node.js、pnpm 以及 [Tauri 2 的 Windows 开发依赖](https://v2.tauri.app/start/prerequisites/)。仓库通过 `packageManager` 固定 pnpm 版本。

```powershell
cd crates/lane-desktop
pnpm --dir ui install --frozen-lockfile
& .\ui\node_modules\.bin\tauri.cmd dev
```

构建安装包：

```powershell
cd crates/lane-desktop
& .\ui\node_modules\.bin\tauri.cmd build
```

## 质量检查

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd crates/lane-desktop/ui
pnpm install --frozen-lockfile
pnpm run build
```

架构说明见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，计划中的能力见 [ROADMAP.md](ROADMAP.md)。

## 安全说明

LANE 设计用于可信局域网，不提供公网暴露所需的 TLS、账户体系或端到端加密。请勿直接映射到公网；发现安全问题请按照 [SECURITY.md](SECURITY.md) 私下报告。

## 参与贡献

提交前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 和 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

## License

[MIT](LICENSE) © LANE contributors
