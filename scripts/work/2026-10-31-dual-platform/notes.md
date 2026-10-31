# 审计证据
- 初始 git status 干净。
- docs/ 和 scripts/ 被 .gitignore 忽略，当前检出不存在；README 引用的架构文档与 release 脚本缺失。
- package.json/Cargo.toml 当前版本 0.1.19，README 的 0.1.15 已过期。
- 本机 arm64，Xcode Command Line Tools 路径 /Library/Developer/CommandLineTools；PATH 中未找到 node/npm/cargo/rustc，进一步寻找工具链。
- 终端适配器仅 WindowsTerminalAdapter；Codex 启动计划硬编码 cmd.exe /K；OSS credential 非 Windows 分支未实现。

- Node 使用 Codex bundled runtime；npm 10.9.2 与 Rust 1.99.1 安装于 /tmp/bexo-build-tools，无全局 PATH 修改；npm ci 保持锁文件。
- Apple API 依据本机 SDK Security/Keychain、CoreGraphics、ApplicationServices 头文件；Terminal 标签页依据系统 Terminal.sdef。macOS 截图复用已存在的 screenshots + WebView fallback，保持 Windows 原生渲染。
- 前端 62/62 测试及 production build 已通过。Rust 首次获取 crates.io 索引持续缓慢，已停止该 cargo（PID 43987），下一次关闭 HTTP multiplexing、设置 30 秒网络超时与 2 次重试。
- 官方依据：https://rust-lang.github.io/rustup/installation/；https://v2.tauri.app/start/prerequisites/。Tauri 锁定 2.10.3，不能用最新 2.12 文档替代版本事实。
