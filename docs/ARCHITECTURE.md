# Architecture

LANE 将传输能力集中在 `lane-core`，CLI 和桌面端只负责生命周期与交互。

```text
Vue browser UI ── HTTP/SSE ──> lane-core <── lane-cli
                                  ↑
                                  └────── lane-desktop (Tauri commands)
                                                   ↑
                                             Vue desktop UI
```

两个 Vue 界面位于同一个 pnpm/Vite 包。根组件根据 Tauri 运行时选择桌面入口，否则加载浏览器入口；浏览器侧的配对、文件架、投递队列等组件集中在 `ui/src/web`。生产构建完成后，脚本会把 Vite 产物及 gzip 变体同步到 `lane-core/assets/dist`，由 `rust-embed` 编入 CLI 和桌面服务。

## 核心模块

- `server.rs`：Axum 路由、认证中间件、上传、Range 下载、SSE 和安全响应头。
- `files.rs`：文件名净化、临时文件落盘、唯一命名和文件夹 ZIP。
- `catalog.rs`：共享条目持久化。`linked` 只引用原文件，`received` 指向接收目录。
- `auth.rs`：随机配对码、IP 尝试限制和内存会话。
- `host.rs`：服务启动、停止、端口回退和状态管理。
- `hub.rs`：非阻塞 SSE 广播；慢消费者不会阻塞文件操作。

## 传输路径

上传内容由 multipart 流直接写入接收目录中的随机 `.part` 文件。服务会先根据请求体长度检查目标文件系统可用空间；成功写入并同步后，在短临界区内选择唯一文件名并原子重命名，随后更新 catalog。空间不足返回 HTTP 507，其他失败也会清理临时文件。

服务启动时会扫描接收目录和系统临时目录，只回收严格匹配 LANE 命名规则且超过 24 小时的残留 `.part`/ZIP 文件；普通文件、符号链接和仍然较新的传输文件不会被删除。

单文件下载使用 256 KiB 流缓冲，并在 Reader 层严格限制 Range 长度。文件夹下载当前先在线程池中构建临时 ZIP，再流式发送并在响应结束后删除。

## 持久化

设置和 catalog 使用“同目录临时文件 → 同步 → 原子替换”写入。启动时扫描接收目录，并批量合并后只写一次 catalog，避免大量文件下的重复全量写入。

## 安全边界

- 浏览器 API 除设备信息和配对接口外均要求有效会话。
- 配对失败按来源 IP 限速；会话仅保存在内存并通过 HttpOnly、SameSite Cookie 传递。
- 共享目标拒绝符号链接；下载只解析 catalog 中已登记的条目。
- 默认无 TLS，因此部署边界是可信局域网。
