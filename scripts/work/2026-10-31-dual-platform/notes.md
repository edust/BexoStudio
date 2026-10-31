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

- 首次 cargo check 发现两个既有跨平台编译错误：通用截图路径调用的 BGRA/RGBA 纯转换函数错误地被 cfg(windows) 屏蔽，已解除纯函数平台限制。
- 工作期间 HEAD 被外部操作从 c52e889 更新至 483ad0e（“记忆存储”），保存了本轮已有改动。本代理未执行 git commit；继续保留该提交，最终文件清单对照原基线 c52e889。

- 首轮 Rust 112 通过/14 失败：Windows .cmd 集成测试缺平台条件且实际启动了两个 macOS Terminal 窗口。已通过专属测试目录和 custom title 精确关闭这两个窗口，保留用户原 Terminal PID 17840。后续 .cmd/Windows-hook 测试仅 Windows 执行，补 macOS zsh 实际参数 roundtrip 与进程组取消测试。
- 原有测试夹具问题：/var 与 /private/var 未规范化；D:\ 路径在 macOS 非绝对；归档导入测试使用不存在 C:/project 并错误包含无项目工作区；history helper 接收的是入口已小写的 query；multipart helper 输入由生产调用者排序，测试未排序。这些仅修正测试，不修改业务规则。
- OSS 官方示例的 canonical hash 经 Python hashlib 验证相同，但其示例 signing key/signature 与声明的 yourAccessKeySecret 不一致。独立 Python HMAC 得 signing key 8a01ff4efcc65ca2cbc75375045c61ab5f3fa8b9a2d84f0add27ef16a25feb3c，signature d3694c2dfc5371ee6acd35e88c4871ac95a7ba01d3a2f476768fe61218590097。测试改为独立计算向量，生产 signer 未变；官方依据 https://www.alibabacloud.com/help/en/oss/developer-reference/recommend-to-use-signature-version-4 。
- 第二轮测试编译曾误写 normalize_session_page_query，已按现有函数 normalize_session_query 修正。第三轮 120 通过/1 失败（归档夹具空工作区），已限定该单元测试导入其目标归档工作区。

- 最终自动化：前端 65/65；Rust all-targets 124 passed / 0 failed / 1 ignored。ignored 为显式本机钥匙串集成测试，实际单独运行因系统认证等待被 180 秒 watchdog 终止，不宣称通过。
- 真实桌面：主窗口/设置/IPC、全局热键注册、测试工作区创建/重启恢复/移除、资源列表、单条 zsh 命令的目录和文件输出已验证。工作区登记已清理，磁盘测试文件留在 /tmp/bexo-build-tools/desktop-fixture。
- 批量 Terminal 首条命令运行，后续标签 Apple Event 返回 -1712；权限与系统对话框尚待用户确认后复验。增加自动化串行化、前台目标确认、有限超时和脱敏错误码，未绕过系统授权。
- 原生钥匙串不支持任务取消；macOS 改为等待系统允许/取消结果，避免原有 5 秒超时遗留仍在执行的写操作。Windows 超时行为不变。原生测试未确认写入和清理结果；测试只用随机 bexo-platform-test-* 虚拟项，未读取真实凭据。
- 补齐配置兼容：Mac 保留而不校验未使用的 Windows Terminal 路径；VS Code .app 优先使用包内 CLI，保留 -n 新窗口语义；通用 .app 使用 open，不误传 -n 为另起应用实例。
- macOS .app/.dmg 调试构建已成功；正式签名、公证、Windows 构建/运行及系统权限验收未执行。没有改动依赖清单/锁文件，没有提交或推送。

2026-10-31 续验：用户已授权 Bexo Studio 自动化/辅助功能/录屏并在本机解锁系统设置；已从设置 UI 确认辅助功能和录屏开启。LaunchServices 启动 PID 90067 卡在 tauri-plugin-log RotatingFile::open_file，系统日志文件已打开，仓库 runtime-logs 未打开。修复 Mac 应用包不添加仓库日志目标（仍保留系统日志），直接开发二进制及 Windows 行为不变，无新增拒绝。

## 授权后续验（2026-10-31）

- 用户已明确授权本次 Bexo Studio 的自动化、辅助功能、录屏权限，并在本机完成系统设置解锁；设置 UI 已确认 Bexo Studio 辅助功能和录屏为 on。自动化列表尚无 Bexo Studio，需正常启动后触发并确认。
- 按 LaunchServices 正常启动的 PID 90067 卡在日志插件打开仓库日志文件；TCC 日志明确显示 `kTCCServiceSystemPolicyDesktopFolder` 请求等待响应。该请求还阻塞后续 dsymutil/esbuild 的访问。
- 修改 `src-tauri/src/logging/mod.rs`：macOS .app 不再添加仓库 runtime-logs 目标，保留系统日志；未打包开发二进制和 Windows 行为不变。新增回归用例。
- 从该源码提取原函数及现有测试，在 /tmp 独立 rustc --test 执行：4 passed / 0 failed。完整 Cargo 测试和 Tauri 重建受上述系统请求阻塞，已终止，不能宣称新修复已完整编译或打包。此前 65 前端/124 Rust 结果仍属于上一版源码。
- 第二次 Keychain 虚拟测试项为 `bexo-platform-test-a8f7542a-c8a2-4740-a17e-412fc4c2ca24`，等待系统认证，180 秒 watchdog 终止；读写和清理均未确认成功。
- 计算机工具拒绝访问 `com.apple.UserNotificationCenter`。已请求用户在本机拒绝旧 Bexo 的桌面访问提示，并处理钥匙串认证；没有改写 TCC 数据库、没有授予完全磁盘访问，也没有绕过工具限制。
- 本轮测试/构建进程已终止，需用户处理系统提示后重建，再继续 Terminal 标签、截图/粘贴和原生 Keychain 验收。当前 app/dmg 仍是上一版，**不包含本轮日志启动修复**。

## 2026-10-31 继续验收：构建入口与签名修复
- 本轮基线 1b098587e582a2737780e8097eb95b03cfb852e1；Mac Rust 125 passed / 1 ignored，日志修复已重新打包且正常 LaunchServices 启动通过。
- Windows CI 41205789777：编译完成，测试 exe 在加载阶段退出 0xc0000139。已安装 tauri-winres 使用 embed_resource::compile，只向 bin 链接资源；按 Tauri #13419 维护者方案将同一 Common Controls v6 清单显式链接到所有 Windows MSVC 目标，避免遗漏测试 harness。原应用清单内容保持一致，无新增业务拒绝。
- Mac Terminal 后续标签返回 1002，刷新辅助功能并重启仍复现；codesign 实测只有 linker-signed 签名、Info.plist 未绑定、Identifier 为构建文件名。补齐应用包 ad-hoc 签名与 Apple Events entitlement；不需要钥匙串凭据，不修改 TCC 数据库。重新打包并验证签名、权限和真实命令输出。
- 依据：https://github.com/tauri-apps/tauri/issues/13419 、https://v2.tauri.app/distribute/sign/macos/ 、https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.automation.apple-events 。
- Windows 修复需新的 runner 复验；提交/推送仍需用户明确授权。Keychain 集成测试按用户反馈不再运行。

### 继续验收进展
- 用户授权仅提交/推送 Windows 清单修复两个文件；通过 GitHub 连接器创建 b83a2a90f14e36d7e26b2d3fa10e6f8fdb05332b，分支 codex/windows-test-manifest。CI https://github.com/edust/BexoStudio/actions/runs/41207352422 正在验证。Git CLI 无推送凭据；本地已 fetch 并切换到跟踪该远端的分支，原本地同内容提交保存在 codex/windows-test-manifest-local，未改写历史。
- 新 Mac 包 codesign --verify --strict 通过，Identifier=studio.bexo.desktop、Info.plist 已绑定，Apple Events entitlement=true。移除旧辅助功能记录、重新添加当前包并重启后，1002 消失，批量第二标签仍因数量未增加报 -2700。
- Terminal.sdef 确认窗口 id 是唯一标识，frontmost 为可写属性。脚本改为按 id 定位窗口并显式设置/有界等待 frontmost；osacompile 语法检查通过，4 项 Mac 终端测试通过，真实标签页复验进行中。

### 最终 TTY 构建与 Windows CI
- 用户现场确认两个标签确在同一可视窗口，旧脚本窗口的 tabs 计数却未增加。不能把 Terminal 的脚本窗口对象等同于可视窗口；此前将 -2700 归为按键无效的判断不充分。
- 按本机 Terminal.sdef 的 tty 属性，菜单操作前收集已有 TTY，等待前台选中标签的新 TTY 后再执行命令。仍保留唯一窗口 ID、前台等待、串行化、15/20 秒超时和脱敏错误。菜单按 SDK 定义的 AXMenuItemCmdChar/AXMenuItemCmdModifiers 匹配 Command+T，不依赖菜单语言或 profile 名。
- 清空两份虚拟输出后，真实 UI“运行全部”成功：verified-first.txt 为 `/private/tmp/bexo-build-tools/desktop-fixture` 加换行，verified-second.txt 为 `中文 $literal` 加换行。两个文件均本轮生成，无批量失败日志。
- 最终 TTY 构建：Rust all-targets 125 passed / 0 failed / 1 ignored，TypeScript/Vite 与 app+dmg 构建成功，codesign 严格校验和 DMG 校验通过。DMG SHA256：445c3bf359e9165bd4bed47a171518b33bb8e625107b4ec21f77419ac4e911d7。
- Windows 路径测试经用户另行授权，追加提交 30ffbfff1f2c32870647b8ca4baaabf29915dde5；只改三个测试的目录比较，生产行为不变。CI 41208497258 的 Windows job 成功：141 passed / 0 failed，NSIS 生成并上传 artifact 13046260130。该 run 的 macOS job 被外部取消；此前 main 1b09858 的 macOS job 成功。
- Windows artifact ZIP SHA256：a57375925f97646109c58aea83f7dce9f77fcdaa584510aadfb4fded00ddbae0，大小 8054971 字节；内含 Bexo Studio_0.1.19_x64-setup.exe。
- 用户回复截图选区已出现，但本机应用日志没有收到热键，CUA 仍看到主窗口且可切换页面，尚不能确认为 Bexo 截图通过。已询问界面所属应用及是否通过远程桌面发送按键，等待回复。快捷粘贴槽位 5 使用临时 Prompt 试验，原五个槽位均未绑定且关闭；CUA 定向按键未触发全局回调，真实键盘验收仍待进行。
