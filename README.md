# Bexo Studio

Bexo Studio 是一个以 `Rust + Tauri v2` 为核心的桌面型 vibe coding 工具箱。

它的首个目标不是做聊天客户端，而是做一个真正可恢复、可编排、可扩展的开发工作台：

- 管理多个项目与工作区
- 一键恢复开发环境状态
- 打开终端、IDE、Codex、附加开发命令
- 管理多个 `CODEX_HOME` / 账号 / 配置上下文
- 支持最小化到托盘、后台常驻和快速恢复

## Current Status

当前版本：`0.1.15`。

当前仓库已从框架搭建阶段进入 Windows-first 的功能完善与生产可靠性阶段，当前重点是：

- 稳定 WORKBENCH、Session / History、Codex Auth 与 Prompts 主流程
- 完善 Windows 自启动、托盘、全局热键和输入注入可靠性
- 保持亮色/暗色主题、紧凑桌面布局与 production WebView 表现一致
- 继续以自动测试、真实桌面运行和安装包回归作为交付门禁

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
  - Windows 开机启动由 Rust 单一注册表适配路径负责，支持幂等校验、写后读回与失败回滚
  - 生产日志目录不依赖进程工作目录，消除登录启动场景下可能在 setup 前静默退出的高可信失败路径
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
    - `Workbench`（根路由 `/`）
    - `Session / History`
    - `Codex Auth`
    - `Prompts`
    - `Settings`
  - `Workspaces / Snapshots / Profiles / Logs` 已退出主导航并冻结为占位页
  - `Workbench` 已接入真实工作区列表、资源浏览器和终端命令组，不再使用演示面板
  - `Settings` 使用 `General / Hotkeys` 两个紧凑设置分区
  - `General` 已接入真实设置：
    - 启动项管理
      - `随系统启动`（Switch）
      - `静默启动`（Switch，仅在 `--autostart` 场景生效）
    - `Windows Terminal` 路径
    - 通过目录选择器手动选择并即时保存
    - 终端命令 Shell 可选 `PowerShell 7` / `cmd.exe`，默认优先 `PowerShell 7`
    - 终端模板管理
    - 通过弹窗管理模板的保存、删除、更新与拖拽排序
  - `Hotkeys` 已接入截图热键录制、注册健康状态和 5 个 Prompt 快速粘贴槽位
- Home Workspace Picker & Safe Remove：
  - 首页左侧工作区改为真实数据源，不再展示演示列表
  - 顶部提供工作区操作菜单：
    - `新建工作区`
    - `全选工作区`
    - `运行工作区`
    - `导入项目列表`
    - `导出项目列表`
  - 通过原生目录选择器注册工作区文件夹
  - 支持工作区多选勾选
  - 工作区描述优先展示目录路径
  - 列表显示最近运行时间
  - 列表支持安全移除，并明确“不删除磁盘上的文件夹”
  - 工作区列表支持版本化 `.bexo-workspaces.json` 迁移：
    - 导出全部工作区、项目、启动任务和置顶状态，不包含凭据、日志、快照或临时选择状态
    - 导入先预检本机目录、重复路径和现有工作区，再由用户选择新建、跳过或显式更新
    - 缺失目录可逐项重新定位；目标机缺少 Codex Profile 或自定义编辑器时会解除对应绑定并明确警告
    - 缺失目录必须重新定位；应用前校验文件 SHA-256，写库使用单个 SQLite 事务
  - 工作区项支持：
    - 只读查看该工作区路径下的 Codex 历史会话
    - 复制绝对路径
    - 在该目录打开终端
    - 使用默认编辑器打开工作区
      - 默认值初始为 `VS Code`
      - 可切换为 `JetBrains IDE`
      - 默认编辑器选择会持久化保存
    - 为工作区添加、编辑和清空项目备注
    - 卡片备注单行预览，工作区搜索同时匹配名称、路径和备注
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
  - 命令组只管理 `terminal_command`
  - 没有工作区时显示空状态
  - 模板配置不再前端写死，统一存入本地偏好
  - 首页模板下拉按设置页保存的模板顺序展示
- Session / History：
  - 左侧主导航提供全局 Session/History 入口
  - 页面只读展示本机 Codex sessions
  - 支持按已注册工作区路径筛选
  - 首屏加载 10 条，滚动到底部自动追加 10 条；搜索和筛选由后端分页处理
  - 点击 session 后读取最近对话，向上滚动加载更早消息
- Codex Auth：
  - 左侧主导航提供 Codex 授权管理入口
  - 只管理 Codex `auth.json` 与 `config.toml`
  - 支持导入当前 Codex 配置、保存多套授权、查询 OAuth 额度和切换当前授权
  - 支持按设置的秒数自动队列刷新所有授权额度，默认 `60s`
  - 支持为额度查询配置系统代理、手动 HTTP/SOCKS 代理或禁用代理
  - 不包含 ChatGPT 登录托管、refresh token 自动维护或其他 AI CLI 控制
- Prompts：
  - 左侧主导航提供常用 Prompts 一级入口
  - SQLite 本地保存标题、完整正文与用户排序
  - 支持新增、编辑、删除、搜索、拖拽/键盘排序
  - 支持将全部已保存 Prompt 导出为版本化 JSON，并通过预检、冲突选择和单事务合并导入
  - 导入按稳定 UUID 判断更新；相同标题不会被猜测覆盖，重复导入保持幂等，新记录按文件顺序追加
  - 导出不包含未保存草稿、快捷粘贴热键、应用偏好或凭据；完整正文属于敏感信息，保存前会明确提示
  - 列表和编辑区均可一键复制，真实写入剪贴板后显示应用内通知
  - 未保存修改在切换条目或离开页面前会明确确认
  - 支持 5 个可配置的全局快速粘贴槽位，默认 `Ctrl+Alt+Shift+1` 到 `Ctrl+Alt+Shift+5`
  - 快捷键在 Settings / Hotkeys 中绑定稳定 Prompt ID；拖拽排序不会改变绑定含义
  - Windows 下由 Rust 写入剪贴板并向当前前台窗口发送 `Ctrl+V`，后台/托盘状态仍可使用
  - 快速粘贴成功后完全静默；失败时保留可操作的系统通知和应用内错误提示
- Screenshot & Hotkeys：
  - 默认截图全局热键为 `Ctrl+Shift+X`，可在 Settings / Hotkeys 中录制和恢复默认
  - 热键冲突、注册失败和 degraded 状态会明确显示，并支持重新注册
  - 截图底图与高频选区交互由 Windows 原生层负责，WebView 负责工具栏、属性面板和复杂配置 UI
- Reliability & Security Hardening：
  - SQLite 写 timeout 会 interrupt 并等待事务确定提交或回滚，不遗留后台悬空写
  - Native Preview/Interaction 由 typed owner thread 独占，跨线程不再传递裸指针
  - Preferences 使用字段级 patch 合并，热键 degraded 状态可见且可重试
  - Codex Auth 列表不返回凭据，授权切换串行并覆盖文件与 DB 状态回滚
  - Restore 失败先清理进程树；Screenshot listener/watch 部分失败会完整释放资源
  - 工作区/终端任务排序使用 Rust 单事务批量命令，不再因前端循环写入产生部分提交
  - Tauri capability 按窗口最小授权，生产/开发 CSP 分离；截图预览协议仅允许 Overlay 窗口
  - Ant Design、Icons 与 Sonner runtime style 继承 Tauri production nonce，避免安装版在 CSP 下退化为裸控件
  - Windows 下所有静态与动态 WebView2 统一禁用 GPU、GPU 合成和软件栅格器，并保留 Wry 默认兼容参数，规避本机硬件加速路径的稳定性风险
  - Overlay/History 已拆为懒加载 chunk，关键热键、runtime state 与 listener lifecycle 有自动测试
- Dev Inspector Baseline：
  - 已接入 `code-inspector-plugin`
  - 在 `Vite serve` 开发模式下默认启用
  - 不注入生产构建

后续优先事项：

- 对 release 安装包执行 Windows 自启动、CSP、托盘和全局热键持续回归
- 继续完善 Launch Task 批量编排、恢复诊断与被冻结模块的回归顺序
- 加强高权限目标窗口、剪贴板占用和热键冲突等异常环境诊断
- `cargo test` 宿主 `STATUS_ENTRYPOINT_NOT_FOUND` 环境问题排查

## Product Direction

### v1 核心
- 工作区注册与分组管理
  - 工作区由用户手动选择文件夹并注册
- 恢复快照与一键恢复
- 终端编排与附加命令启动
- Codex Profile / `CODEX_HOME` 管理
- Codex `auth.json` / `config.toml` 授权管理与切换
- 常用 Prompts 管理、一键复制和最多 5 个全局快速粘贴槽位
- VS Code / IDEA 启动
- 托盘化运行、窗口恢复、日志与通知

### 长期方向
- 演进为更完整的开发工作台
- 保留 Cherry Studio 风格的信息架构扩展空间
- 逐步增加工具页、脚本页、文件页、插件页、运行诊断页

## Design References
- 信息架构参考 Cherry Studio
- 设置页质感参考 CC Switch
- 默认视觉基线为明亮主题，同时完整支持暗色主题
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
- [产品需求](docs/product-requirements.md)
- [技术架构](docs/technical-architecture.md)
- [UI 系统](docs/ui-system.md)
- [实施路线图](docs/implementation-roadmap.md)
- [仓库协作规范](AGENTS.md)

## Local Development

前置环境：Node.js/npm、Rust stable、Windows MSVC Build Tools 与 Microsoft Edge WebView2 Runtime。

安装前端依赖：

```powershell
npm install
```

启动完整 Tauri 桌面应用（会自动启动 Vite）：

```powershell
npm run desktop:dev
```

仅调试浏览器前端时：

```powershell
npm run web:dev
```

常用验证命令：

```powershell
npm run web:test
npm run web:build
npm run desktop:build:debug
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --release
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-run
```

发行打包命令：

```powershell
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

运行环境说明：

- `wt`、`code`、`codex` 或 JetBrains IDE 不在 PATH 时，可在 `Settings / General` 中配置对应路径
- 工具探测优先级为 `user_config -> PATH`
- Launch Tasks 当前 UI 已开放：
  - `terminal_command`
  - `open_path`
  - `ide`
  - `codex`
- Home 终端命令组默认使用 `PowerShell 7(pwsh)` 执行启动命令；可在 `Settings / General` 切换回 `cmd.exe`
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
- [task_plan.md](task_plan.md)
- [findings.md](findings.md)
- [progress.md](progress.md)
- [work blueprint](scripts/work/2026-03-09-bexostudio-blueprint/task_plan.md)

## OSS 文件管理

当前已接入 Windows-first 的阿里云 OSS 文件管理模块：

- 支持多个手动录入的 RAM AccessKey 账号；Secret 只保存于 Windows Credential Manager。
- 支持手动绑定多个 Bucket、Region、Endpoint 和对象前缀；第一版不创建、删除或修改 Bucket 配置。
- 支持对象分页浏览、前缀导航、新建文件夹、上传、下载、删除和同 Bucket 重命名；文件夹以 `/` 结尾的 0 字节对象表示。
- 上传入口支持选择器多选和 Windows 资源管理器多文件拖放；每个文件独立入队，批次采用有界并发，单项失败不会阻塞其他文件。
- 传输队列显示账号、Bucket、Region、Object Key、本地路径、数值进度、速度、预计剩余时间和错误码；上传完成后自动刷新当前对象目录。
- 大文件使用 Multipart Upload / Range 下载，保存 checkpoint，支持取消、恢复、重启后继续和清理失败提示；单 PUT 取消在远端结果未知时进入“待确认”，避免误报已取消。
- 文件对象支持右键快捷菜单：下载文件、同 Bucket 复制文件、复制 15 分钟有效的私有下载 URL、删除；删除仍需二次确认，复制目标 Key 会阻止覆盖已有对象。
- 预签名下载 URL 只在本次操作中生成并复制，不写入 SQLite、日志、传输事件或应用状态；URL 生成由 Rust V4 signer 完成，Secret 不离开 Credential Manager。
- 前端不直连 OSS、不持有 Secret；Rust 统一负责 V4 签名、超时、有限重试、路径校验和传输状态事件。

详细实现记录见 [OSS 文件管理交付清单](scripts/work/2026-07-12-oss-file-manager/deliverable.md)、[新建文件夹交付清单](scripts/work/2026-07-12-oss-create-folder/deliverable.md)、[传输体验交付清单](scripts/work/2026-07-12-oss-transfer-ux/deliverable.md) 和 [对象右键菜单交付清单](scripts/work/2026-07-12-oss-object-context-menu/deliverable.md)。
