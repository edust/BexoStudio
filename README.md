# Bexo Studio

Bexo Studio 是一个以 `Rust + Tauri v2` 为核心的桌面型 vibe coding 工具箱。

它的首个目标不是做聊天客户端，而是做一个真正可恢复、可编排、可扩展的开发工作台：

- 管理多个项目与工作区
- 一键恢复开发环境状态
- 打开终端、IDE、Codex、附加开发命令
- 管理多个 `CODEX_HOME` / 账号 / 配置上下文
- 支持最小化到托盘、后台常驻和快速恢复

## Current Status

当前仓库的开发优先级已切换为：

- 功能冻结
- 先重构桌面 UI 框架
- 收敛导航、页面密度和整体堆叠
- 暂不继续扩展业务能力

当前仓库已完成：

- 产品需求、技术架构、UI 系统、实施路线图与协作规范
- Phase 0 工程初始化：
  - `Tauri v2 + Rust`
  - `React + TypeScript + Vite + SWC`
  - `Tailwind CSS v4`
  - `Radix UI / shadcn-style` 基础组件
  - 三段式桌面 `AppShell`
  - 基础路由占位
  - tray / window-state / log 等插件骨架
- Phase 1 领域模型与持久化：
  - SQLite schema 与数据库初始化
  - Rust `commands / domain / persistence / services / error` 分层
  - `workspace / project / codex_profile / launch_task / snapshot / restore_run / restore_run_task` 领域模型
  - `list_workspaces / upsert_workspace / delete_workspace / upsert_project`
  - `list_codex_profiles / upsert_codex_profile`
  - Workspaces / Profiles 最小可用 CRUD 页面
  - `react-hook-form + zod` 表单校验与错误展示
- Phase 2 Snapshot / Restore Planner：
  - Snapshot 捕获、列表与 typed payload 持久化
  - Restore Preview 生成与项目级 action 规划
  - `restore_run / restore_run_task` dry-run 状态流
  - Snapshots / Logs 页面真实数据闭环
  - 结构化 restore log 文件写入骨架
- Phase 3 Windows Adapters：
  - `WindowsTerminal / VS Code / JetBrains / Codex` 适配器探测与启动
  - 真实 `restore_run / restore_run_task` 执行状态流
  - `CODEX_HOME` 注入与 `codex / codex resume --last` 终端启动编排
  - Snapshots 页能力探测与真实恢复入口
  - Logs 页真实执行结果与日志目录入口
- Phase 4/5 Tray & Diagnostics：
  - `tauri-plugin-store` 偏好存储与 `PreferencesService`
  - 用户可配置 `Windows Terminal / Codex CLI / VS Code / JetBrains` 路径
  - 工具探测优先级改为 `user_config -> PATH`
  - 托盘最近工作区快速恢复
  - `close to tray` 改为用户偏好
  - Home 页最近工作区真实入口
  - Snapshots / Logs 页显示 adapter source / executable path
  - Settings 页真实配置闭环
- Phase 6a Launch Tasks & Restore Event Flow：
  - `launch_task` 的 Rust `domain / persistence / service / command` 已接通
  - Workspaces 页可为项目配置最小可用 `terminal_command` 启动任务
  - Snapshot payload / Restore Preview 已携带 launch tasks
  - `restore_run_tasks` 已记录 project 级与 launch-task 级状态
  - `restore://run-event` 已接通，Snapshots / Logs 页可实时感知恢复事件
  - Logs 页通过事件驱动刷新，不依赖轮询
- Phase 6b Cancel Control & Task Type Expansion：
  - 新增 active run registry 与 child process registry
  - 新增 `cancel_restore_run`
  - 应用启动时会回收遗留的 interrupted restore runs
  - Launch Tasks 已扩展到 `open_path / ide / codex`
  - Snapshots / Logs 页已支持运行中取消与 cancelled 状态展示
- Phase 6c Action Cancel & Diagnostics：
  - child process registry 已细化到 `project_task_id + action_id`
  - 新增 `cancel_restore_action`
  - `restore://run-event` 已覆盖 `action_started / action_finished / action_cancel_requested / action_cancelled`
  - Snapshots / Logs 页已支持 action 级取消与 `cancelRequestedAt / diagnosticCode` 可视化
  - 已用用户配置的 `wt.exe` 完成一次真实 restore / action cancel 闭环验证
- UI Refresh Light Theme Baseline：
  - 全局主题 token 已从深色切换为明亮基线
  - AppShell、Primary Rail、Section Sidebar 与主要页面卡片已去除黑底观感
  - 亮色方向保持桌面工具感与 Cherry Studio 式信息架构，不改成通用后台模板
- UI Refresh Compact Professional Workbench：
  - AppShell、Primary Rail、Section Sidebar 已收紧到更窄、更稳的桌面工作台节奏
  - Settings 页已改成更紧凑的专业配置流，减少大卡片展示感
  - 首页状态卡、工具诊断和最近工作区列表已降低首屏高度与留白
- UI Framework Reset: Ant Compact Shell：
  - 已接入 `Ant Design`
  - 当前壳层改为 `Ant Design + compact theme + Tailwind` 的混合方案
  - 一级导航已收敛为：
    - `Home`
    - `Settings`
  - `Workspaces / Snapshots / Profiles / Logs` 已退出主导航并冻结为占位页
  - `Home` 已改成只保留工作台画布和 panel 占位的紧凑框架页
  - `Settings` 已改成只保留 `General` 单项的紧凑设置页
  - `General` 当前已接入真实设置：
    - `Windows Terminal` 路径
    - 通过目录选择器手动选择并即时保存
    - 终端模板管理
    - 通过弹窗管理模板的保存、删除、更新与拖拽排序
  - 当前阶段重点是统一桌面框架，而不是继续做业务内容
- Home Workspace Picker & Safe Remove：
  - 首页左侧工作区改为真实数据源，不再展示演示列表
  - 顶部提供工作区操作菜单：
    - `新建工作区`
    - `全选工作区`
    - `运行工作区`
  - 通过原生目录选择器注册工作区文件夹
  - 支持工作区多选勾选
  - 工作区描述优先展示目录路径
  - 列表显示最近运行时间
  - 列表支持安全移除，并明确“不删除磁盘上的文件夹”
  - 工作区项支持：
    - 复制绝对路径
    - 在该目录打开终端
    - 使用默认编辑器打开工作区
      - 默认值初始为 `VS Code`
      - 可切换为 `JetBrains IDE`
      - 默认编辑器选择会持久化保存
- Home Content Workspace Detail：
  - 首页右侧 `Content View` 已移除示例块
  - 点击左侧工作区后，右侧显示资源浏览器、只读文件夹路径与终端命令组
  - 资源浏览器支持：
    - 左右折叠 / 展开
    - 宽度拖拽调整并持久化
    - 目录按需异步展开
    - 多选
    - Git 状态高亮：`modified / renamed / untracked / ignored`
    - `仅看变更` 快速筛选
    - `Ctrl+Shift+C` 复制选中资源绝对路径
    - `Ctrl+Shift+R` 在系统资源管理器中显示当前资源
    - 右键快捷菜单：显示到资源管理器、复制绝对路径
    - 原生文件监听启动前会先为当前工作区动态授权 `fs_scope`，失败时回退轮询刷新
    - 已接入原生拖出资源能力，已在宝塔上传窗口实测通过文件与文件夹拖入
  - 右侧已新增当前工作区默认项目的“终端命令组”
  - 支持：
    - 新增命令
    - 从 `Settings > General` 中读取终端模板并回填
    - 编辑
    - 删除
    - 拖放排序
    - 基础语法校验
    - `运行全部`
      - 一个新的 Windows Terminal 窗口
      - 按排序顺序逐个打开 tabs
      - tabs 之间固定间隔 `10s`
    - 单条命令独立窗口运行
    - 成功/失败同时触发系统级通知
  - 命令组只管理 `terminal_command`
  - 没有工作区时显示空状态
  - 模板配置不再前端写死，统一存入本地偏好
  - 首页模板下拉按设置页保存的模板顺序展示
- Dev Inspector Baseline：
  - 已接入 `code-inspector-plugin`
  - 在 `Vite serve` 开发模式下默认启用
  - 不注入生产构建

下一阶段将进入：

- Launch Task 模板化与批量编排 UX
- action 级取消继续向子进程树与更细粒度诊断下钻
- `cargo test` 宿主 `STATUS_ENTRYPOINT_NOT_FOUND` 环境问题排查

## Product Direction

### v1 核心
- 工作区注册与分组管理
  - 工作区由用户手动选择文件夹并注册
- 恢复快照与一键恢复
- 终端编排与附加命令启动
- Codex Profile / `CODEX_HOME` 管理
- VS Code / IDEA 启动
- 托盘化运行、窗口恢复、日志与通知

### 长期方向
- 演进为更完整的开发工作台
- 保留 Cherry Studio 风格的信息架构扩展空间
- 逐步增加工具页、脚本页、文件页、插件页、运行诊断页

## Design References
- 信息架构参考 Cherry Studio
- 设置页质感参考 CC Switch
- 当前视觉基线为明亮主题，不再以深色优先
- 当前亮色主题进一步收敛为紧凑、专业、偏桌面工作台的视觉基线
- 当前允许引入 `Ant Design` 以快速重建紧凑桌面 UI 框架
- 但仍不照搬 Cherry Studio 的 Electron / Redux / styled-components 重栈

## Chosen Stack
- Desktop shell: `Tauri v2`
- Runtime/system layer: `Rust`
- Frontend: `React + TypeScript + Vite`
- UI: `Tailwind CSS v4 + Ant Design + Radix UI`
- Client state: `Zustand + TanStack Query`
- Persistence: `SQLite + structured local store`
- System Notification: `tauri-plugin-notification`

## Document Map
- [产品需求](D:\Desktop\rust\BexoStudio\docs\product-requirements.md)
- [技术架构](D:\Desktop\rust\BexoStudio\docs\technical-architecture.md)
- [UI 系统](D:\Desktop\rust\BexoStudio\docs\ui-system.md)
- [实施路线图](D:\Desktop\rust\BexoStudio\docs\implementation-roadmap.md)
- [仓库协作规范](D:\Desktop\rust\BexoStudio\AGENTS.md)

## Local Development

```bash
npm install
npm run web:dev
npm run desktop:dev
```

可用验证命令：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run web:build
npm run desktop:build:debug
```

发行打包命令：

```bash
npm run release:build
```

说明：

- `release:build` 会先把发行版本号按 `patch` 自动 `+1`
- 会同步更新：
  - `package.json`
  - `src-tauri/Cargo.toml`
  - `src-tauri/tauri.conf.json`
- 然后执行 `tauri build --bundles nsis`
- 最终产出 Windows 安装包 `.exe`

补充说明：

- 当前 `desktop:dev` 依赖 `1420` 端口；若出现 `Port 1420 is already in use`，先清理遗留的 `vite` 进程再重跑
- 当前环境下 `cargo test` 能成功编译出测试可执行文件，但测试宿主启动时出现 `STATUS_ENTRYPOINT_NOT_FOUND`
- 因此本轮 Rust 验证以 `cargo check`、`cargo test --no-run`、桌面构建和可执行文件启动为主

当前机器能力探测基线：

- `wt`: 未在 PATH 中探测到
- `code`: 已探测到
- `codex`: 已探测到
- `idea / idea64.exe`: 已探测到

说明：

- 从 Phase 4/5 开始，`wt` 不在 PATH 已不再是硬阻塞
- 可在 `Settings / General` 中手动选择 `Windows Terminal` 目录
- 当前机器已验证 `wt` 用户配置目录可生效：
  - `D:\Downloads\Compressed\Microsoft.WindowsTerminalPreview_1.21.1772.0_x64\terminal-1.21.1772.0`
- Launch Tasks 当前 UI 已开放：
  - `terminal_command`
  - `open_path`
  - `ide`
  - `codex`
- 恢复运行中支持从 Snapshots / Logs 触发 run 级与 action 级取消
- 最近一次实机验证已确认：
  - `builtin:terminal_context` 使用 `wt.exe`
  - `executableSource = user_config`
  - 被取消 action 会保留 `cancelRequestedAt` 与 `PROCESS_CANCELLED`

## Build Outputs

已验证可生成：

- `src-tauri/target/debug/bexo-studio.exe`
- `src-tauri/target/release/bundle/msi/Bexo Studio_<version>_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Bexo Studio_<version>_x64-setup.exe`

## Repository Planning Files
- [task_plan.md](D:\Desktop\rust\BexoStudio\task_plan.md)
- [findings.md](D:\Desktop\rust\BexoStudio\findings.md)
- [progress.md](D:\Desktop\rust\BexoStudio\progress.md)
- [work blueprint](D:\Desktop\rust\BexoStudio\scripts\work\2026-03-09-bexostudio-blueprint\task_plan.md)
