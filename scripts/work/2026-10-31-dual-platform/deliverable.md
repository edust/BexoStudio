# 双平台实现交付与剩余验收

代码适配已落地；**完整平台验收尚未完成**，权限确认及 Windows 执行仍待补齐。原 Windows 业务规则保留，未新增业务拒绝条件；未改依赖清单或锁文件，未提交、推送、部署或调用真实 OSS 服务。

## 实际验证

| 项目 | 结果 |
| --- | --- |
| `npm run web:test` | 65 通过，0 失败 |
| `npm run web:build` | TypeScript 与 Vite production build 通过（Tauri beforeBuildCommand） |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml --all-targets` | 124 通过、0 失败、1 ignored；ignored 原生授权测试单列如下 |
| macOS debug `.app` / `.dmg` | 构建成功，Mach-O arm64；最终产物核对见下方 |
| AppleScript 语法 | osacompile 通过 |
| 真实桌面 | 窗口加载、设置 IPC、全局热键注册、工作区创建/重启读取/移除、资源文件显示、单条 zsh 工作目录与文件输出通过 |
| 批量 Terminal 标签 | **Unverified**：首条执行，后续系统调用 -1712 超时；已补充串行化/目标前台与错误码，需授权后复验 |
| 原生 Keychain 读写/删除 | **Unverified**：虚拟凭据测试等待 SecurityAgent，180 秒 watchdog 已终止进程；未确认写入/清理结果，不读取真实账户 |
| 截图选区/标注/导出、跨应用粘贴、托盘交互、登录启动 | **Unverified**：部分需要屏幕录制/辅助功能授权；未擅自授予或修改系统设置 |
| Windows x64 构建与桌面回归 | **Unverified**：本机为 arm64 Mac，CI 已添加但未推送/运行 |
| 正式签名、公证、发行构建与发布 | **Unverified / 未执行**；已生成的是调试构建 |

编译仍有 Windows 专属代码在 Mac 上未使用的 warning，以及 `block 0.1.6` 的未来兼容提示；未为消除警告升级依赖。现有 Windows .cmd/hook 测试只在 Windows 执行；共享测试夹具和独立签名向量的修正均未改生产业务逻辑。

Keychain Services 没有取消接口。Mac 原生凭据操作等待系统允许/取消结果，防止“返回超时后继续写入”；这不等同于可强制中断系统认证。Rust 进程组取消只覆盖直接创建的进程组，已交由外部 Terminal 承载的 shell 需在终端结束。

## 产物与部署

- 应用：`/Users/mac/Desktop/BexoStudio/src-tauri/target/debug/bundle/macos/Bexo Studio.app`
- 安装镜像：`/Users/mac/Desktop/BexoStudio/src-tauri/target/debug/bundle/dmg/Bexo Studio_0.1.19_aarch64.dmg`
- 已验证构建目的地为本机 `target/debug/bundle/`，没有安装到 Applications 或进行发布。
- 用户安装时可将 `.app` 拖入 `/Applications`，随后启动并按系统提示授权；安装、登录启动与实际登录回归 **Unverified**。
- 正式发行使用 `npm run release:build`，产物进入 `target/<target>/release/bundle/`；正式签名/公证凭据未配置，本次没有数据库迁移要求或新增配置键。
- 本机日志在 `/tmp/bexo-build-tools/{frontend-final,rust-tests-final,desktop-build-final,desktop-runtime-final,keychain-test}.log`。

## 临时进程与数据

测试应用与已识别测试 shell 均已关闭，用户原有 Terminal 会话保留；测试工作区已通过应用从列表移除，磁盘验证文件留在 `/tmp/bexo-build-tools/desktop-fixture`。最终进程/端口复查记录在本文末尾。系统启动的共享 SecurityAgent 不属于测试服务，不终止系统代理。

## 手动启动与测试

本机临时工具链可直接运行（不会改系统 PATH 配置文件）：

```sh
cd "/Users/mac/Desktop/BexoStudio"
export CARGO_HOME="/tmp/bexo-build-tools/cargo"
export RUSTUP_HOME="/tmp/bexo-build-tools/rustup"
export PATH="/tmp/bexo-build-tools/bin:/tmp/bexo-build-tools/cargo/bin:/Users/mac/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin:$PATH"
npm run desktop:dev
```

Tauri 自动先启动 Vite，地址 `http://localhost:1420`，随后启动桌面。用 `Ctrl+C` 停止。本轮测试进程不会为用户保留，人工验收需要自行启动。`/tmp` 工具链可能被系统清理，长期开发请按 README 安装 Node.js 24、npm、Rust 和 Xcode CLT，然后直接运行相同 npm 命令。

## 本轮逐文件清单

基线为开工时 `c52e889`；期间外部提交 `483ad0e` 保存了部分本轮修改，本代理未提交，因此不能只用当前 `git diff` 计算本轮文件。

| 文件（绝对路径） | 类型 | 用途 |
| --- | --- | --- |
| [.github/workflows/desktop.yml](/Users/mac/Desktop/BexoStudio/.github/workflows/desktop.yml) | 新增 | Windows x64 / macOS arm64 测试与安装包流水线（未执行） |
| [.gitignore](/Users/mac/Desktop/BexoStudio/.gitignore) | 修改 | 只放行本次发行脚本、专项记录和平台文档 |
| [README.md](/Users/mac/Desktop/BexoStudio/README.md) | 修改 | 更新平台、工具链与发行入口说明 |
| [docs/platform-support.md](/Users/mac/Desktop/BexoStudio/docs/platform-support.md) | 新增 | 平台矩阵、授权、构建和验收边界 |
| [findings.md](/Users/mac/Desktop/BexoStudio/findings.md) | 修改 | 记录实现事实、失败与验证边界 |
| [progress.md](/Users/mac/Desktop/BexoStudio/progress.md) | 修改 | 记录实际进度 |
| [scripts/release/build-release-installer.mjs](/Users/mac/Desktop/BexoStudio/scripts/release/build-release-installer.mjs) | 新增 | 按宿主构建指定架构，不改版本和锁文件 |
| [scripts/work/2026-10-31-dual-platform/deliverable.md](/Users/mac/Desktop/BexoStudio/scripts/work/2026-10-31-dual-platform/deliverable.md) | 新增 | 实际验证、产物、未完成项和逐文件清单 |
| [scripts/work/2026-10-31-dual-platform/notes.md](/Users/mac/Desktop/BexoStudio/scripts/work/2026-10-31-dual-platform/notes.md) | 新增 | 源码依据、失败定位与决策记录 |
| [scripts/work/2026-10-31-dual-platform/task_plan.md](/Users/mac/Desktop/BexoStudio/scripts/work/2026-10-31-dual-platform/task_plan.md) | 新增 | 专项范围、原行为→新行为与状态 |
| [src-tauri/Info.plist](/Users/mac/Desktop/BexoStudio/src-tauri/Info.plist) | 新增 | Apple Events 用途说明 |
| [src-tauri/gen/schemas/macOS-schema.json](/Users/mac/Desktop/BexoStudio/src-tauri/gen/schemas/macOS-schema.json) | 新增 | Tauri 构建生成的 macOS 配置 schema |
| [src-tauri/src/adapters/codex.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/codex.rs) | 修改 | Mac Codex 命令与 CODEX_HOME 传递 |
| [src-tauri/src/adapters/ide.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/ide.rs) | 修改 | CLI/.app 发现及正确的项目/新窗口参数 |
| [src-tauri/src/adapters/mod.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/mod.rs) | 修改 | 平台终端、Shell 与文件管理器分派 |
| [src-tauri/src/adapters/oss/alibaba.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/oss/alibaba.rs) | 修改 | 仅修正 multipart 单测的已排序调用前提 |
| [src-tauri/src/adapters/oss/credentials.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/oss/credentials.rs) | 修改 | 按平台选择凭据存储 |
| [src-tauri/src/adapters/oss/credentials_macos.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/oss/credentials_macos.rs) | 新增 | Keychain FFI 与显式原生集成测试 |
| [src-tauri/src/adapters/oss/v4_signer.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/oss/v4_signer.rs) | 修改 | 仅修正由独立 HMAC 核实的签名测试向量 |
| [src-tauri/src/adapters/process.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/process.rs) | 修改 | Unix 进程组取消、应用包解析、终端超时/串行化/脱敏错误 |
| [src-tauri/src/adapters/prompt_paste.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/prompt_paste.rs) | 修改 | 选择 Mac 粘贴实现 |
| [src-tauri/src/adapters/prompt_paste_macos.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/prompt_paste_macos.rs) | 新增 | 辅助功能检查及 Command+V 事件 |
| [src-tauri/src/adapters/terminal.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/terminal.rs) | 修改 | Windows 读取新增 zsh 偏好时保持原回退 |
| [src-tauri/src/adapters/terminal_macos.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/adapters/terminal_macos.rs) | 新增 | Terminal/zsh、目录和环境转义、窗口标签调度 |
| [src-tauri/src/domain/preferences.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/domain/preferences.rs) | 修改 | zsh 枚举及平台默认值 |
| [src-tauri/src/persistence/workspace_repo.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/persistence/workspace_repo.rs) | 修改 | 按平台写入记录；修正跨平台测试夹具 |
| [src-tauri/src/services/codex_history_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/codex_history_service.rs) | 修改 | 仅修正单测入口规范化前提 |
| [src-tauri/src/services/oss_credential_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/oss_credential_service.rs) | 修改 | 等待不可取消的 Mac 原生授权结果，防止超时后继续写入 |
| [src-tauri/src/services/oss_transfer_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/oss_transfer_service.rs) | 修改 | 仅把单测 Windows 绝对路径改为宿主临时路径 |
| [src-tauri/src/services/preferences_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/preferences_service.rs) | 修改 | Mac 配置兼容、应用包与 Unix CLI 校验、平台测试隔离 |
| [src-tauri/src/services/resource_browser_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/resource_browser_service.rs) | 修改 | 仅规范化单测临时路径的 /var 符号链接 |
| [src-tauri/src/services/restore_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/restore_service.rs) | 修改 | 恢复流程使用平台适配器；Windows shim 测试限定平台 |
| [src-tauri/src/services/screenshot_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/screenshot_service.rs) | 修改 | Mac 权限、WebView 窗口及通用像素转换编译修复 |
| [src-tauri/src/services/workspace_service.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/services/workspace_service.rs) | 修改 | 工作区终端与文件打开使用平台适配器 |
| [src-tauri/tauri.macos.conf.json](/Users/mac/Desktop/BexoStudio/src-tauri/tauri.macos.conf.json) | 新增 | Mac 窗口、最低系统版本及 app/dmg 配置 |
| [src/components/shell/section-sidebar.tsx](/Users/mac/Desktop/BexoStudio/src/components/shell/section-sidebar.tsx) | 修改 | 平台终端提示和 Finder 菜单 |
| [src/features/oss/oss-account-dialog.tsx](/Users/mac/Desktop/BexoStudio/src/features/oss/oss-account-dialog.tsx) | 修改 | 系统凭据存储文案 |
| [src/features/oss/oss-account-sidebar.tsx](/Users/mac/Desktop/BexoStudio/src/features/oss/oss-account-sidebar.tsx) | 修改 | 系统凭据存储文案 |
| [src/features/resource-browser/resource-browser-pane.tsx](/Users/mac/Desktop/BexoStudio/src/features/resource-browser/resource-browser-pane.tsx) | 修改 | Mac Finder 菜单提示 |
| [src/features/resource-browser/resource-browser-utils.ts](/Users/mac/Desktop/BexoStudio/src/features/resource-browser/resource-browser-utils.ts) | 修改 | POSIX 根目录、父目录和大小写比较 |
| [src/features/settings/use-hotkey-recorder.ts](/Users/mac/Desktop/BexoStudio/src/features/settings/use-hotkey-recorder.ts) | 修改 | Mac 修饰键录制，保留 Windows 左右键逻辑 |
| [src/lib/terminal-command.ts](/Users/mac/Desktop/BexoStudio/src/lib/terminal-command.ts) | 修改 | Mac 原样保留 POSIX 命令；Windows 解析不变 |
| [src/pages/settings-page.tsx](/Users/mac/Desktop/BexoStudio/src/pages/settings-page.tsx) | 修改 | 平台终端设置、文件选择与代理提示 |
| [src/types/backend.ts](/Users/mac/Desktop/BexoStudio/src/types/backend.ts) | 修改 | 前后端 shell 枚举同步 |
| [task_plan.md](/Users/mac/Desktop/BexoStudio/task_plan.md) | 修改 | 登记双平台任务状态 |
| [tests/frontend/platform-terminal.test.mjs](/Users/mac/Desktop/BexoStudio/tests/frontend/platform-terminal.test.mjs) | 新增 | POSIX 命令/路径与 Windows 兼容回归 |

## 最终核对

- 最终源码已重新构建 app/dmg；确认包含 VS Code 包内 CLI 路由，Mach-O arm64，版本 0.1.19，AppleEvents 用途声明存在。
- 最终 DMG 经 hdiutil verify 校验通过；SHA-256：`a6419ecf21b3cfded186a61bc860ba83caa8ba8cfefe99d8df6ad0eb4f58deb4`。
- 最终 AppleScript 语法编译与 git diff --check 通过。
- 进程检查无本轮 bexo-studio、测试二进制、osascript、cargo/rustc；相关测试 shell 已结束；1420/1421 无监听。未执行 commit/push，依赖与锁文件无差异。

## 授权后续验（2026-10-31）

- 用户已明确授权本次 Bexo Studio 的自动化、辅助功能、录屏权限，并在本机完成系统设置解锁；设置 UI 已确认 Bexo Studio 辅助功能和录屏为 on。自动化列表尚无 Bexo Studio，需正常启动后触发并确认。
- 按 LaunchServices 正常启动的 PID 90067 卡在日志插件打开仓库日志文件；TCC 日志明确显示 `kTCCServiceSystemPolicyDesktopFolder` 请求等待响应。该请求还阻塞后续 dsymutil/esbuild 的访问。
- 修改 `src-tauri/src/logging/mod.rs`：macOS .app 不再添加仓库 runtime-logs 目标，保留系统日志；未打包开发二进制和 Windows 行为不变。新增回归用例。
- 从该源码提取原函数及现有测试，在 /tmp 独立 rustc --test 执行：4 passed / 0 failed。完整 Cargo 测试和 Tauri 重建受上述系统请求阻塞，已终止，不能宣称新修复已完整编译或打包。此前 65 前端/124 Rust 结果仍属于上一版源码。
- 第二次 Keychain 虚拟测试项为 `bexo-platform-test-a8f7542a-c8a2-4740-a17e-412fc4c2ca24`，等待系统认证，180 秒 watchdog 终止；读写和清理均未确认成功。
- 计算机工具拒绝访问 `com.apple.UserNotificationCenter`。已请求用户在本机拒绝旧 Bexo 的桌面访问提示，并处理钥匙串认证；没有改写 TCC 数据库、没有授予完全磁盘访问，也没有绕过工具限制。
- 本轮测试/构建进程已终止，需用户处理系统提示后重建，再继续 Terminal 标签、截图/粘贴和原生 Keychain 验收。当前 app/dmg 仍是上一版，**不包含本轮日志启动修复**。

本轮新增修改文件：

| 文件 | 类型 | 用途 |
| --- | --- | --- |
| [src-tauri/src/logging/mod.rs](/Users/mac/Desktop/BexoStudio/src-tauri/src/logging/mod.rs) | 修改 | 避免 macOS 应用包在首次启动时访问受保护的桌面仓库日志，增加回归用例 |
