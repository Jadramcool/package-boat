# PacketBoat 局域网投递站

PacketBoat 是一个以 Rust 编写的局域网文件分享工具。发送端只需打开浏览器、输入六位配对码，即可向主机上传文件或下载主机分享的文件；数据不经过公网中转。

> 当前版本为 `0.1.0`，面向 Windows 桌面端。项目仍处于早期阶段，欢迎试用和贡献。

## 功能

局域网浏览器访问，发送端无需安装客户端；数据不经过公网中转。

### 传输

- 分块上传：会话协商块大小、逐块 SHA-256 校验、缺块续传与失败重试，完成后原子提交
- 实时速度与预计剩余时间，弱网停滞时自动清除过期估算
- 多文件并行上传（并发数当前固定为 2）
- 单文件 Range / 断点下载，256 KiB 流缓冲
- 文件夹下载边压缩边流式发送 ZIP，不落完整临时压缩包
- 可选「共享目录内全部文件」：把接收目录当作本地共享文件夹
- 单任务取消与上传会话清理
- 磁盘空间预检（不足返回 HTTP 507）、磁盘写满错误识别、24 小时残留临时文件回收
- Windows 任务栏传输进度

### 共享与目录表

- 原位共享文件与文件夹，只记录路径，不复制也不移动源文件
- 一键清空共享清单、单条取消共享，均只移除记录、不删除磁盘文件
- 接收目录不再被自动扫描为共享文件；旧版本数据升级时只保留原位共享记录，磁盘文件不受影响
- 文件缺失时保留条目并标记为不可用；区分「原位共享」与「远程接收」两类来源

### 认证与安全

- 六位配对码、按来源 IP 限速、内存会话 + HttpOnly/SameSite Cookie
- 除设备信息与配对接口外，所有 API 均要求有效会话
- 共享与下载拒绝符号链接，只解析目录表中已登记的条目

### 实时与连接

- SSE 实时刷新文件列表，慢消费者不会阻塞文件操作
- 多网卡地址列举、端口回退与自动分配

### 桌面端（Windows）

- 系统托盘常驻、关闭窗口最小化到托盘、无边框自定义标题栏、窗口置顶
- 拖拽文件到窗口即原位共享、接收目录设置、服务开关
- 访问地址二维码（可携带配对码自动配对）、一键复制地址

### 计划中（尚未支持）

- 可配置上传并发数、传输历史记录与暂停/继续、mDNS 设备发现、英文界面、安装包代码签名、HTTPS 与端到端加密。完整计划见 [ROADMAP.md](ROADMAP.md)。

## 仓库结构

```text
crates/
├── packetboat-core/       # HTTP 服务、认证、目录表与传输逻辑
└── packetboat-desktop/    # Tauri 2 Windows 桌面端
apps/
└── desktop/         # 统一 Vue 3 前端（桌面端 + 浏览器接收页）
```

桌面端和浏览器接收页源码均位于 `apps/desktop/src`。生产构建会同时生成两个按需加载的入口，并把可嵌入资源同步到 `packetboat-core/assets/dist`。

## 快速开始

需要稳定版 Rust 工具链、Node.js、pnpm 以及 [Tauri 2 的 Windows 开发依赖](https://v2.tauri.app/start/prerequisites/)。仓库通过 `packageManager` 固定 pnpm 版本。

```powershell
pnpm install --frozen-lockfile
pnpm run dev:desktop
```

窗口打开后局域网服务自动启动，界面上会显示访问地址和六位配对码。在同一局域网的设备上打开该地址即可使用。

## 开发

- `pnpm run dev:desktop`：桌面端开发模式，Vite 在 `http://127.0.0.1:5173` 提供热更新，改动 Rust 代码会自动重编译并重启应用。
- `pnpm run build:desktop`：构建 Windows 安装包。

单独开发浏览器接收页时，先启动桌面端（后端服务随之启动），再在另一个终端启动 Vite；浏览器打开 `http://127.0.0.1:5173` 即可获得热更新：

```powershell
pnpm run dev:desktop   # 终端 1：后端服务
pnpm run dev:web       # 终端 2：浏览器接收页热更新
```

如服务使用其他端口，可通过 `PACKETBOAT_DEV_SERVER_URL` 覆盖 Vite 的 `/api` 代理目标。

## 质量检查

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 快速端到端传输基准
cargo bench -p packetboat-core --bench transfer -- --profile smoke

pnpm install --frozen-lockfile
pnpm run test
pnpm run build
```

架构说明见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，分块协议见 [docs/UPLOAD_PROTOCOL.md](docs/UPLOAD_PROTOCOL.md)，基准方法见 [docs/BENCHMARKS.md](docs/BENCHMARKS.md)，版本变化见 [CHANGELOG.md](CHANGELOG.md)，计划中的能力见 [ROADMAP.md](ROADMAP.md)。

## 发布

推送形如 `v0.1.0` 的 Git 标签会触发 GitHub Release：自动构建 Windows NSIS 安装包，并附带 `SHA256SUMS.txt`。正式发布前应确保标签版本与 Cargo、Tauri 配置中的版本一致。

## 安全说明

PacketBoat 设计用于可信局域网，不提供公网暴露所需的 TLS、账户体系或端到端加密。请勿直接映射到公网；发现安全问题请按照 [SECURITY.md](SECURITY.md) 私下报告。

## 参与贡献

提交前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 和 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

## License

[MIT](LICENSE) © PacketBoat contributors
