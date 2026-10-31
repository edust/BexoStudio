# Windows x64 / macOS arm64 双平台适配

## 目标与合同
保持 Windows 已有业务行为，为 macOS arm64 补齐平台实现；不新增业务拒绝条件。SQLite 和已有 IPC 保持兼容；系统控制留在 Rust。命令复用现有超时/取消设施；只对幂等探测考虑重试。

## Phases
### Phase 1: 审计与工具链
**Status:** complete
读取现有实现，列出平台依赖、缺失文件及验证条件。
### Phase 2: 系统适配
**Status:** complete
终端、IDE、Codex、路径、进程取消、凭据、桌面集成、截图与粘贴。
### Phase 3: UI 与发行
**Status:** complete
按平台展示设置、快捷键与诊断，提供两平台构建配置和说明。
### Phase 4: 验证与交付
**Status:** in_progress
本机编译/测试/桌面验证；Windows 独立验证，未执行的项目明确标记 Unverified。

## 原行为 → 新行为
- Windows 现有实现 → 保留行为。
- macOS 缺少平台实现或返回不支持 → 调用 macOS 对应实现。
- 新增业务拒绝：无。

## 验证计划
前端测试和 production build；Rust fmt/check/test；本机桌面启动、工作区/终端/IDE、截图/粘贴/托盘；Windows 构建与运行单独记录。不得调用真实 OSS 或更改用户系统设置进行自动验收。

代码与自动化检查已完成；Phase 4 剩余系统权限确认、macOS 交互验收及 Windows runner 验收，不能标记全部通过。详见 deliverable.md。

授权续验：辅助功能和录屏已开启；日志启动修复的独立 4 项测试通过。完整构建受待处理的 macOS 系统通知阻塞，已清理进程；等待本机处理提示后继续。当前安装包尚未包含该修复。

## 2026-10-31 继续验收：构建入口与签名修复
- 本轮基线 1b098587e582a2737780e8097eb95b03cfb852e1；Mac Rust 125 passed / 1 ignored，日志修复已重新打包且正常 LaunchServices 启动通过。
- Windows CI 41205789777：编译完成，测试 exe 在加载阶段退出 0xc0000139。已安装 tauri-winres 使用 embed_resource::compile，只向 bin 链接资源；按 Tauri #13419 维护者方案将同一 Common Controls v6 清单显式链接到所有 Windows MSVC 目标，避免遗漏测试 harness。原应用清单内容保持一致，无新增业务拒绝。
- Mac Terminal 后续标签返回 1002，刷新辅助功能并重启仍复现；codesign 实测只有 linker-signed 签名、Info.plist 未绑定、Identifier 为构建文件名。补齐应用包 ad-hoc 签名与 Apple Events entitlement；不需要钥匙串凭据，不修改 TCC 数据库。重新打包并验证签名、权限和真实命令输出。
- 依据：https://github.com/tauri-apps/tauri/issues/13419 、https://v2.tauri.app/distribute/sign/macos/ 、https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.automation.apple-events 。
- Windows 修复需新的 runner 复验；提交/推送仍需用户明确授权。Keychain 集成测试按用户反馈不再运行。

## 本轮收尾状态
此前 TCC 阻塞构建的问题已解除，最终 app/dmg 包含日志修复及完整包签名。Windows 清单与路径测试修复已在 runner 验证，141 项测试和 NSIS 通过；Mac 125 项测试、文件监听、批量 Terminal 的新 TTY 路径已通过真实执行。Phase 4 仍保留未完成的截图/快捷粘贴真实键盘、Keychain、Windows GUI、登录启动/多显示器验收。测试配置和进程已清理；以 deliverable.md 顶部最新记录为准。

## App 列表重复安装回归（进行中）
已确认同一 CDHash 的安装包再次 launch-disabled，原首次启动方法仅恢复单次安装。保持现有打包默认与业务行为，增加可选本机签名身份读取；在明确授权后用现有固定证书签名、备份当前安装并连续两轮替换，逐轮检查 LaunchServices 标志和用户实际 App 列表。若仍复发，不能把固定签名作为解决方案。尚未授权私钥使用和安装覆盖；不修改全局信任或系统安全设置。

App 列表回归状态：完成。本机固定签名构建、严格签名校验、DMG 校验、两轮替换均完成；第二轮未启动应用，登记无 launch-disabled，用户实测图标可见。原双平台任务的其他未验收项不因此改变。
