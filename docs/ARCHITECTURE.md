# Architecture

PacketBoat 将传输能力集中在 `packetboat-core`，桌面端只负责生命周期与交互。

```text
Vue browser UI ── HTTP/SSE ──> packetboat-core
                                  ↑
                                  └────── packetboat-desktop (Tauri commands)
                                                   ↑
                                             Vue desktop UI
```

两个 Vue 界面位于同一个 pnpm/Vite 包。根组件根据 Tauri 运行时选择桌面入口，否则加载浏览器入口；浏览器侧的配对、文件架、投递队列等组件集中在 `apps/desktop/src/web`。生产构建完成后，脚本会把 Vite 产物及 gzip 变体同步到 `packetboat-core/assets/dist`，由 `rust-embed` 编入 CLI 和桌面服务。

## 核心模块

- `server.rs`：Axum 路由、认证中间件、上传、Range 下载、SSE 和安全响应头。
- `files.rs`：文件名净化、临时文件落盘、唯一命名和文件夹 ZIP。
- `uploads.rs`：分块上传会话、块摘要校验、缺块查询、取消和原子提交。
- `catalog.rs`：共享条目持久化。`linked` 只引用用户明确选择的原文件，`received` 只指向本次服务明确接收的上传文件；`inbox` 为可选开启的接收目录顶层扫描项。`list()` 带短 TTL 缓存；文件系统事件通过 `apply_path_event` 增量刷新命中条目。
- `auth.rs`：随机配对码、IP 尝试限制和内存会话。
- `host.rs`：服务启动、停止、端口回退和状态管理。
- `hub.rs`：非阻塞 SSE 广播；慢消费者不会阻塞文件操作。
- `watch.rs`：非递归监听条目父目录，事件去抖后增量刷新目录表并广播。

## 传输路径

浏览器先创建上传会话，再按服务端协商的块大小写入接收目录中的随机 `.part` 文件。每块都携带 SHA-256，服务端验证、定点写入并同步后才记录为已接收；客户端重新连接时查询会话，只补传缺失块。全部块完成后，服务在短临界区内选择唯一文件名并原子重命名，随后更新 catalog。传统 multipart 端点继续保留供兼容客户端使用。空间不足返回 HTTP 507，其他失败也会清理临时文件。

服务启动时默认不会扫描或共享接收目录中的既有普通文件，只回收接收目录和系统临时目录中严格匹配 PacketBoat 命名规则且超过 24 小时的残留 `.part` 文件；普通文件、符号链接和仍然较新的传输文件不会被删除。开启「共享目录内全部文件」后，接收目录顶层的文件/文件夹会以 `inbox` 来源出现在列表中，便于整夹共享。

单文件下载使用 256 KiB 流缓冲，并在 Reader 层严格限制 Range 长度。文件夹下载在内存管道中边遍历边用 Deflate 压缩并流式发送（data descriptor），不再先把完整临时 ZIP 落到磁盘；客户端断开时打包任务随管道关闭而中止。

## 持久化

设置和 catalog 使用“同目录临时文件 → 同步 → 原子替换”写入。catalog 只在用户明确共享文件或上传完成时登记条目；接收目录中的既有文件仅在开启 inbox 扫描时出现在列表，且不会写入 catalog.json。

## 安全边界

- 浏览器 API 除设备信息和配对接口外均要求有效会话。
- 配对失败按来源 IP 限速；会话仅保存在内存并通过 HttpOnly、SameSite Cookie 传递。
- 共享目标拒绝符号链接；下载只解析 catalog 条目或开启后的 inbox 顶层扫描项。
- 默认无 TLS，因此部署边界是可信局域网。
