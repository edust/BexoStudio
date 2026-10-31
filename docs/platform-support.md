# 双平台实现与验收

目标：Windows x64（`x86_64-pc-windows-msvc`）、macOS Apple Silicon（`aarch64-apple-darwin`）。Linux 和 Intel Mac 不在本次交付范围。

## 实现边界

| 能力 | Windows x64 | macOS arm64 |
| --- | --- | --- |
| 工作区、项目、快照、恢复记录、Prompts、导入导出 | 共享 Rust/SQLite | 共享 Rust/SQLite |
| 系统终端 | Windows Terminal + PowerShell 7/cmd | Terminal + zsh 登录 Shell |
| 批量终端 | 每工作区一个窗口、命令独立标签页 | 每工作区一个窗口、命令独立标签页；Terminal 自动化、System Events 辅助功能授权 |
| Codex | cmd 执行 CLI、注入 CODEX_HOME | 直接执行 CLI、在目标终端显式注入 CODEX_HOME |
| 编辑器 | PATH/常见安装目录/用户配置 | PATH、Homebrew 常见目录、/Applications、~/Applications、用户指定 .app 或 CLI |
| Rust 直启任务的进程取消 | taskkill /T | 独立 POSIX 进程组；取消清理该组 |
| 系统文件打开 | Explorer | /usr/bin/open |
| 截图 | 现有 Windows 原生捕获、预览、交互 | CoreGraphics 截图与现有 WebView 选区/标注/导出；保留 Retina 原像素 |
| 快速粘贴 | Ctrl+V 输入注入 | Command+V 输入注入；辅助功能授权 |
| OSS Secret | Windows Credential Manager | macOS 登录钥匙串；不使用命令行传递 Secret |
| 托盘/通知/单实例/文件监听 | 已有 Tauri 插件 | 同一组插件的 macOS 实现 |
| 登录启动 | 现有注册表实现 | 已有 autostart 插件的 LaunchAgent 实现 |
| 全局热键 | 普通热键及左右侧修饰键 hook | 普通全局组合键；录制 Control/Option/Shift/Command，不生成 Windows 左右侧 hook 格式 |

Windows 数据与 IPC 不迁移、不批量改写。`windowsTerminalPath` 和 capability `windows_terminal` 保留兼容键名；macOS 使用系统 Terminal，不消费 Windows Terminal 路径。新建 macOS 项目记录 `platform=macos`、`terminal_type=macos_terminal`。Shell 类型增加 `zsh`，macOS 总是选择 zsh；Windows 保持 PowerShell/cmd，读取 zsh 偏好时按原默认 PowerShell 路径执行。

外部 Terminal 接管的交互 shell 不属于 Rust 启动器的进程组，需在对应终端窗口关闭；不能把启动器取消当作终端内服务已停止。

macOS 新输入的终端命令原样交给 zsh，避免变量、管道、引号和空参数在前端拆分后失真。跨平台导入不会自动把 PowerShell/cmd 命令改写成 POSIX 命令；用户需按目标平台编辑命令。

## 系统授权

- Terminal 启动：首次操作由系统请求自动化授权。
- 批量标签页、快速粘贴：在系统设置的“隐私与安全性 → 辅助功能”允许 Bexo Studio。
- 截图：首次截图请求屏幕录制授权；拒绝时返回明确错误，按系统提示授权并重启应用。
- 钥匙串：由系统控制解锁与访问；密钥不进入数据库、日志或临时文件。原生 Keychain Services 不提供取消 API，应用等待系统对话框的允许/取消结果，不在 5 秒后丢弃写入任务并误报失败。锁定状态下需在本机解锁或取消系统提示。
- 应用不绕过或代替用户授予系统权限。

macOS 截图使用不透明的全屏底图窗口，因此无需 Tauri 的 macos-private-api 或改动依赖锁文件。Windows WebView2 禁用 GPU 参数保持原样。

## 构建与运行

项目根目录提供打包入口：macOS arm64 执行 `./build_macos_arm64.sh`，Windows x64 双击或执行 `build_win64.bat`。脚本自动定位项目目录，缺少 Tauri CLI 时执行 `npm ci`，然后调用 `npm run release:build`；失败时返回非零退出码。Mac 脚本优先使用 PATH 中的工具，并兼容本机验收时准备的临时工具链；工具链不存在时需要自行安装。Windows 脚本结束时暂停以便查看结果。

共同前提：Node.js 24、npm、Rust stable。Windows 需要 MSVC Build Tools 和 WebView2；macOS 需要 Xcode Command Line Tools。

在仓库根目录：

```sh
npm ci
npm run desktop:dev
```

开发服务器使用 http://localhost:1420，Tauri 会自动启动；Ctrl+C 停止。浏览器单独运行 `npm run web:dev` 只能验证前端，不能验收系统功能。

```sh
npm run web:test
npm run web:build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm run release:build
```

发行脚本按宿主选择上述目标，以仓库版本构建，**不自动递增版本或改写锁文件**。产物在 `src-tauri/target/<target>/release/bundle/`：Windows `nsis/*.exe`，macOS `macos/*.app` 与 `dmg/*.dmg`。安装 `.app` 到 Applications 后再进行登录启动验收；开发路径不能作为稳定发行安装路径。

`.github/workflows/desktop.yml` 提供两平台测试和安装包构建，不自动发布。macOS 签名、公证和公开分发需要正式 Apple Developer 凭据；本次不调用发布服务。

macOS 本地包默认使用 `bundle.macOS.signingIdentity: "-"` 做应用包 ad-hoc 签名，使签名绑定 `studio.bexo.desktop` 和 Info.plist；无需证书或钥匙串密码。这不等于 Developer ID 签名或公证。`Entitlements.plist` 声明已有 Terminal 自动化功能所需的 Apple Events 能力，实际访问仍由系统提示授权。重新构建可能改变签名，若系统授权失效，需在系统设置重新添加当前应用并重启；普通 `desktop:dev` 二进制不能替代打包应用的权限验收。正式发行时按 Tauri 文档配置真实签名身份和公证凭据。

Windows MSVC 构建显式把与 Tauri 默认内容一致的 Common Controls v6 清单嵌入应用和测试程序，避免 `cargo test` 的库测试 harness 因缺少清单而在加载时退出 `STATUS_ENTRYPOINT_NOT_FOUND`。对应上游问题：[Tauri #13419](https://github.com/tauri-apps/tauri/issues/13419)。

## 验收状态

实际命令结果、产物及未执行项目以 `scripts/work/2026-10-31-dual-platform/deliverable.md` 为准。编译通过不等于跨应用粘贴、截图权限、多显示器、登录启动或 Windows 桌面验收通过。

## 依据

- Tauri 平台配置与 Info.plist 合并：https://v2.tauri.app/reference/config/
- GitHub macOS arm64 runner 标签：https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- 已安装版本源码：Cargo.lock 中 Tauri 2.10.3、screenshots 0.8.10、display-info 0.4.8。
- 本机 SDK：Security.framework Keychain 头文件、CoreGraphics CGEvent/CGWindow 头文件、ApplicationServices AXUIElement 头文件。
- Terminal AppleScript 字典：`/System/Applications/Utilities/Terminal.app/Contents/Resources/Terminal.sdef`。
