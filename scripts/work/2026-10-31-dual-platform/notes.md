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
