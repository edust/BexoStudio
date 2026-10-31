# Windows x64 / macOS arm64 双平台适配

## 目标与合同
保持 Windows 已有业务行为，为 macOS arm64 补齐平台实现；不新增业务拒绝条件。SQLite 和已有 IPC 保持兼容；系统控制留在 Rust。命令复用现有超时/取消设施；只对幂等探测考虑重试。

## Phases
### Phase 1: 审计与工具链
**Status:** in_progress
读取现有实现，列出平台依赖、缺失文件及验证条件。
### Phase 2: 系统适配
**Status:** pending
终端、IDE、Codex、路径、进程取消、凭据、桌面集成、截图与粘贴。
### Phase 3: UI 与发行
**Status:** pending
按平台展示设置、快捷键与诊断，提供两平台构建配置和说明。
### Phase 4: 验证与交付
**Status:** pending
本机编译/测试/桌面验证；Windows 独立验证，未执行的项目明确标记 Unverified。

## 原行为 → 新行为
- Windows 现有实现 → 保留行为。
- macOS 缺少平台实现或返回不支持 → 调用 macOS 对应实现。
- 新增业务拒绝：无。

## 验证计划
前端测试和 production build；Rust fmt/check/test；本机桌面启动、工作区/终端/IDE、截图/粘贴/托盘；Windows 构建与运行单独记录。不得调用真实 OSS 或更改用户系统设置进行自动验收。
