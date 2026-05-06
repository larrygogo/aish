# aish

AI 友好的跨平台 SSH 桌面客户端。专为远程使用 AI CLI 工具（Claude Code、Codex、Aider 等）优化。

**状态：** 早期开发中（M0：workspace 骨架）。

## 核心特性（Phase 1 目标）

- **tmux 可视化管理**：连上服务器后自动列出已有 session/window/pane，点击即可切换、关闭、新建，无需记快捷键
- **多模态附件支持**：拖拽本地文件到窗口 → 自动 SFTP 上传 → 路径回填光标，让远端 AI CLI 直接拿到文件
- **安全的远程 env 注入**：API key 集中存在本地 OS keyring，连接时按 profile 注入到远端 shell，远端文件系统不留凭证

## 技术栈

- 语言：Rust（stable 1.80+）
- GUI：[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- 终端：[alacritty_terminal](https://github.com/alacritty/alacritty/tree/master/alacritty_terminal)
- SSH：[russh](https://github.com/Eugeny/russh)
- tmux 集成：control mode（`tmux -CC`）

详见 [docs/superpowers/specs/2026-05-06-aish-ssh-design.md](docs/superpowers/specs/2026-05-06-aish-ssh-design.md)。

## 构建

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## 平台支持

macOS / Windows / Linux（Linux GPUI 稳定性是已知短板，主推 macOS / Windows）

## 许可证

MIT
