# Contributing

感谢你愿意改进 PacketBoat。

## 开始之前

1. 对较大的功能或行为变化，先创建 issue 说明使用场景和方案。
2. 从主分支创建短生命周期分支，每个 PR 聚焦一个问题。
3. 不要提交 `target`、`node_modules`、UI 临时构建目录或本地代理配置。

## 本地检查

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

pnpm install --frozen-lockfile
pnpm run build
```

涉及传输协议时，请补充 `crates/packetboat-core/tests` 下的集成测试；涉及 UI 时，请至少完成 TypeScript 类型检查和生产构建。`pnpm run build` 会刷新 `packetboat-core/assets/dist`，请将对应的嵌入资源一并提交。

## 提交与 PR

- 提交信息使用简短的祈使句，说明“做了什么”。
- PR 描述应包含问题、实现、测试结果及兼容性影响。
- 不要把真实配对码、用户目录、IP、日志或私有文件放入 issue、截图和测试夹具。
- 贡献代码即表示你同意按仓库的 MIT License 发布该贡献。

发布版本时还应更新根目录 `CHANGELOG.md`，并确保 Cargo workspace、Tauri 配置和桌面 UI 的版本号一致。
