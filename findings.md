# findings

## 2026-07-11 UI 缩放与布局回归

- 用户截图显示异常跨 WORKBENCH、资源浏览器、终端命令组和 Prompts，初始分类为全局/共享层回归，尚未确认根因。
- 上一轮快速粘贴静默变更按目标仅移除成功 Toast 和成功系统通知；本轮将用实际 Git diff 与构建产物核实是否存在意外波及，不凭变更意图排除关联。
- 原图显示 Tailwind 自定义壳层仍正常，而 Ant Design Input/Button 盒模型与 clear icon 定位跨页面同时丢失；优先验证生产 CSP 是否拦截 CSS-in-JS style 注入。
- 当前 CSP 已允许 inline style，源码也无主窗口 zoom/scale；根因尚未成立，下一证据点是运行时 `data-css-hash` 与受影响组件的 computed style/最终 JSX。
- 当前源码在 Chrome/Vite 与独立 Tauri/WebView2 dev 中均能完整注入 Ant CSS-in-JS；问题现已限定为生产构建/安装版差异或安装版运行时状态。
- production Tauri 初始 `/history` 样本出现 0px border，但不是与 dev 相同元素，不能据此直接定根因；官方 layer 兼容配置缺失是高可信候选，等待 HOME/Prompts 同元素 production 对照。
- HOME/Prompts production 同元素已确认 0px/0px/0px/透明/auto，且截图精确复现；根因为 Ant CSS-in-JS 与 Tailwind v4 的 production cascade layer 顺序缺失。
- 继续审计发现 Ant 元素 hash 与动态 style 规则 hash 匹配且规则未分层；需最终区分“级联覆盖”与“CSP 使动态 style sheet 未激活”，尚不实施修复。
- 最终区分完成：production Ant style 的 `sheet` 全为 null 且 nonce 为空，确定是 Tauri production nonce CSP 拦截 Ant runtime styles；不是 cascade layer 覆盖。
- 现有静态 overlay style 已获 Tauri production nonce 且生效；Ant ConfigProvider 支持原生 csp nonce。Ant Icons 与 Sonner runtime style 也被同一问题影响，修复/验证范围需覆盖三者。

## 2026-07-10 常用 Prompts 模块

- 现有剪贴板 helper 会 trim 后复制，无法满足 Prompt 正文原样复制；本轮将改为仅 trim 判空、复制原始内容。
- Prompt 模块复用现有 SQLite 确定事务、CommandResponse、批量排序和 Tauri 命令权限一致性门禁。
- 页面采用 Codex Auth 同类的“页面自持有列表 + 编辑器”结构，但从起点拆分 feature components，避免新巨石页面。
- 不新增依赖；长列表用项目内固定行高窗口化，排序期间临时渲染完整列表保证拖拽命中。
- 最终结果：完整集合事务排序、原样正文复制、脏状态保护、窗口化列表、双端输入校验和 main-window-only 权限均闭环；二次 Review 无未关闭高/中严重度发现。

## 2026-07-10 Prompt 全局热键快速粘贴

- 已确认项目存在 `tauri-plugin-global-shortcut`、Windows 高级热键 hook、热键健康状态/回滚、`arboard`、系统通知和 Prompt SQLite 稳定 ID，无需引入平行基础设施。
- 用户锁定默认组合为 `Ctrl+Alt+Shift+1` 至 `Ctrl+Alt+Shift+5`；虽然 `Ctrl+Alt` 对部分 AltGr 布局存在风险，但必须尊重该产品决策并保留可配置和冲突反馈。
- 绑定必须存 Prompt ID，不能按列表实时位置解析，否则拖拽排序会静默改变热键语义。
- 全局触发时修饰键可能仍处于按下态；粘贴动作必须在 Released 阶段或确认修饰键释放后执行，避免发送成 `Ctrl+Shift+V`。
- 第三方应用是否接受粘贴无法可靠确认；按最新交互要求，成功结果完全静默，仅失败时发送通知与错误 Toast。
- `ui-ux-pro-max` 安装目录和仓库均缺失 `search.py`，本轮改为复用既有 Settings 视觉系统并执行技能内置检查表。
- 最终实现采用 `PromptQuickPasteService -> PromptPasteAdapter -> SystemPromptPasteAdapter`，Windows 剪贴板与输入注入不泄漏到前端或业务编排层。
- 运行 Review 已关闭 SendInput 部分发送残留按键、修饰键超时后的剪贴板部分副作用、非 Windows 失败副作用和前端通知初始化竞态。
- `global-hotkey 0.7.0` Windows 源码确认 Released 事件真实存在；Prompt action 在 Released 处理，adapter 再等待全部修饰键释放。
- Settings 亮/暗 1440×1000 截图通过；动态 Prompt 徽标因 headless web 无 Tauri SQLite 数据，保留为桌面人工验收项。
- 实机回归发现 Prompt 简介同时使用 `line-clamp-2` 与 `block`；生产 CSS 中后声明的 `.block { display:block }` 覆盖了两行截断所需的 `display:-webkit-box`，且外框没有裁剪边界，因此长正文穿透相邻条目。
- 修复后简介使用有效两行截断、34px 最大高度和长串换行，内容区与 78px 外框均裁剪；82px 虚拟行高保持不变。亮/暗主题的中文、无空格长串、多行内容视觉回归均无重叠。
- STEP 6 对照确认 WORKBENCH 顺滑排序来自 stable-ID Motion Reorder、layoutScroll、shared spring 和 drag-end persistence；Prompts 原自研 elementFromPoint 指针排序缺少 layout 动画。
- Prompts 已对齐 WORKBENCH 卡片状态面与 Motion 主路径；80 条以内完整 Motion，超过阈值保留 windowing + pointer fallback 并加入 layout 过渡。排序增加 optimistic query update 和失败回滚。
- STEP 7 实机回归确认上一轮漏掉 WORKBENCH 的显式暗色 hover 分支；Prompt 已改为共享 themeMode 条件，暗色 hover 精确使用 `#2a2d2e / #3c3c3c`，不再出现亮色白块。

## 2026-05-06 Terminal Shell Preference
- 当前 Home 终端命令组实际启动链路为 `wt.exe new-tab ... cmd.exe /D /K <commandLine>`，因此用户按 `Ctrl+C` 后回到的是 cmd 会话，而不是 PowerShell 7。
- `cmd.exe /K` 的启动命令不会按用户预期进入 PowerShell/PSReadLine 历史；这解释了停止 Codex 后上翻没有熟悉历史的体验。
- 2026-04-21 将可见终端命令切到 cmd 是为兼容旧问题：Windows PowerShell 5 对 `cd "path" && command` 支持不好。现在用户明确安装并希望使用 PowerShell 7，因此应改成可配置 shell，而不是继续硬编码 cmd。
- restore 的 `terminal_command` 也有独立后台执行路径，当前使用 `cmd.exe /D /C`。只改 Home 可见 tab 会造成两条命令语义不一致。

## 2026-05-06 自启动可靠性
- 本机 `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 中 `Bexo Studio` 当前值为 `"C:\Users\aka86\AppData\Local\Bexo Studio\bexo-studio.exe" "--autostart"`，目标文件存在，版本 `0.1.11`。
- `HKCU\...\Explorer\StartupApproved\Run` 中 `Bexo Studio` 的二进制值以 `0x02` 开头，表示 Windows 任务管理器启动项未禁用。
- 手动执行安装版 Run 命令后，`bexo-studio` 进程保持运行；Windows Application/System 事件日志没有 Bexo 相关崩溃。
- 安装版日志明确记录 `main window hidden on autostart due to startup.startSilently=true`，偏好文件也显示 `startup.launchAtLogin=true` 与 `startup.startSilently=true`。因此“没启动”很可能是静默启动隐藏到后台/托盘。
- 真实缺陷在路径漂移风险：之前从开发版运行 Bexo 时，Run 值一度被同步成 `"D:\Desktop\rust\BexoStudio\src-tauri\target\debug\bexo-studio.exe" "--autostart"`。开发版路径不适合作为登录自启动目标，重启后可能表现为没启动或像闪退。

## 2026-05-06
- 截图 `Esc` 取消链路在 2026-04-20 后改为 `tauri-plugin-global-shortcut` 注册裸 `"Escape"`，以解决 WebView 焦点不稳定时无法收到 `keydown` 的问题。
- 本轮确认直接根因：`apply_escape_cancel_hook()` 注册的是 `app.global_shortcut().on_shortcut("Escape", ...)`，但 `clear_escape_cancel_hook()` 只调用 `escape_hook_manager.clear_bindings()`，没有调用 `app.global_shortcut().unregister("Escape")`。
- `copy_selection / save_selection / cancel_session / cancel_active_session_from_escape` 以及 overlay close 都会走 `clear_active_session()`，因此应把 `Escape` 的 global shortcut 注销放到这个统一清理路径。
- 为避免清理时误动其他模块未来可能注册的 `Escape`，需要记录截图服务是否确实注册了 `Escape`，并只在该标记为 true 时注销。

## 2026-05-05
- Codex Auth 额度查询当前使用 Rust `reqwest::Client::new()` 发起请求，依赖配置为 `default-features = false` 且只启用 `json` / `rustls`，没有显式系统代理或手动代理配置。
- 因为用户截图显示所有账号重启后均 `Network error: error sending request for url (https://chatgpt.com/backend-api/wham/usage)`，且该 API 在国内网络常需代理，合理根因是当前 quota 请求没有走用户系统代理。
- 只启用系统代理能解决一部分机器，但代理工具没有写入 Windows 系统代理或需要本地 HTTP/SOCKS 端口时仍会失败；因此需要同时支持手动代理。
- `reqwest 0.13.2` 的代理能力可以在保持 `default-features = false` 和 `rustls` 的同时启用 `system-proxy` 与 `socks`。
- 额度 HTTP client 不应继续作为 `CodexAuthService` 的固定字段，否则 Settings 里修改代理模式后需要重启才生效；当前更合适的是按查询读取偏好并构造 client。
- 手动代理 URL 即使当前模式不是 `manual` 也应校验协议，因为 UI 允许先保存地址再切换手动模式。

## 2026-05-04
- Codex Auth 单账号额度查询已经集中在 Rust `CodexAuthService::query_quota`，结果持久化到 `codex_auth_profiles.last_quota_json` / `last_quota_checked_at`，因此批量刷新不应在前端直接散写 HTTP 请求。
- Settings 当前通过 `AppPreferences` / `PreferencesService` / `tauri-plugin-store` 持久化，新增自动刷新间隔应进入 `AppPreferences.codexAuth`，而不是 SQLite 或页面 local state。
- 直接用 `setInterval` 做自动刷新存在重叠风险：当一轮 quota 请求慢于间隔时，下一轮会并发进入；正确实现应为 `批量刷新完成 -> 倒计时 -> 下一轮`。
- 偏好加载完成导致 React effect 重启时也可能制造第二轮并发请求，因此需要前端 in-flight guard；Rust 批量 command 也加 mutex，保证即使 UI 重启或重复调用也不会批量重叠。
- 自动刷新当前设计为 Codex Auth 页面作用域，不做全局后台刷新，避免用户离开页面后继续调用外部额度接口。

## 2026-04-21
- “打开工作区终端后 PowerShell prompt 显示 `Microsoft.PowerShell.Core\FileSystem::\\?\D:\...`”的来源在 Rust `workspace_service.rs`：
  - `open_workspace_terminal_at_path()` 会先调用 `canonicalize_existing_directory()`
  - Windows 下 `std::fs::canonicalize()` 返回 verbatim path，例如 `\\?\D:\...`
  - 当前代码把 `workspace_root.display().to_string()` / `target_directory.display().to_string()` 直接传给 Windows Terminal `-d`
- 修复边界应保持内部 canonical path 不变，只在传给外部终端和返回前端前去掉 Windows verbatim 前缀。

## 2026-04-20
- 当前代码里工作区只有 `sortOrder`，没有任何“置顶状态”字段；如果只靠改排序来实现“置顶”，程序无法稳定区分“普通排在前面”和“被置顶”，因此也没法正确支持“取消置顶”。
- 更合理的落点是 `AppPreferences.workspace.pinnedWorkspaceIds`：
  - 它属于用户本地 UI 偏好，而不是工作区领域数据
  - 不需要改 SQLite schema
  - 和现有 `workspace.selectedWorkspaceIds` 在语义上一致
- 置顶这轮最终采用的是“偏好负责 membership，`sortOrder` 负责组内顺序”：
  - `pinnedWorkspaceIds` 只表达哪些工作区属于置顶组
  - 真正的前后顺序继续由 `sortOrder` 驱动
  - 点击 `置顶` 时，再同步把目标工作区移动到顶部并写回新的 `sortOrder`
- 这样做可以避免后续拖拽排序和置顶能力互相覆盖，也不用给 `pinnedWorkspaceIds` 再单独维护一套顺序语义。
- `src/components/shell/section-sidebar.tsx` 当前工作区排序实现有两个明显风险点：
  - `workspaceItems` 每次 query 更新都会走 `reconcileWorkspaceSidebarItems()` 回写 `orderedWorkspaceItems`，拖拽过程中如果数据回流，容易和 `Reorder.Group` 的预览状态打架
- `motion.dev` 官方文档对 React Layout 动画的要求是：滚动容器应加 `layoutScroll`，这样 Motion 才能在拖拽/重排时正确计入 scroll offset。
- 继续下钻后，真正最危险的实现细节是：`Reorder.Group values` / `Reorder.Item value` 传入的是整条 `WorkspaceSidebarItem` 对象，而这类对象会随着 query / recent target 更新不断换引用。
- 对 `motion/react Reorder` 来说，这会让“当前被拖的是谁、周围列表该怎么重排”在拖拽中途变得不稳定，直接对应你看到的“下面几项消失”和“排序错乱”。
- 因此这轮最小正确修复不是继续补动画参数，而是：
  - `value` 改为稳定主键 `workspace.id`
  - 滚动容器加 `layoutScroll`
  - 拖拽项补 `relative`，让 `z-index` 在拖拽中真正生效
- Home 页“新增终端命令”的直接风险点已经定位到前端 `src/lib/terminal-command.ts`：
  - `parseTerminalCommandLine` 把所有反斜杠都当成 escape 起点，导致 `C:\abcd` 会在保存前被解析成 `C:abcd`
  - `buildTerminalCommandLine` / Rust `build_terminal_command_line` 又会在展示或执行前自行补引号、补转义，进一步放大原始输入失真
- 这说明问题不在 SQLite 持久化层，而在“表单单行命令 <-> 结构化 `command + args`”双向转换本身。
- 参考 PowerShell 官方 quoting 规则后，本轮 Rust 侧采用了更稳的命令重组方式：
  - 单引号字符串在 PowerShell 中按字面量处理
  - 单引号内容里的单引号应写成两个单引号
  - 用 call operator `&` 执行 `& 'cmd' 'arg1' 'arg2'`，能避免把路径、空格参数和引号再次改写
- 这次“加了文字/图形后，切回选区工具，再单击选区外导致控件消失并冻结”的直接触发链，不在 Rust，而在前端 `handleStagePointerDown/Up`：
  - `tool === "select"` 时会直接进入“开始新选区”分支
  - `pointerdown` 立刻清空当前 `selection`
  - `pointerup` 若新框不足 `2px`，会执行 `resetAnnotations()`
- 这意味着“单击选区外”会被误判成一次失败的新选区创建，从而把现有对象全部清空；你看到的“控件瞬间消失”正是 `resetAnnotations()` 的直接结果。
- 最新日志里 `seq=38 visible=false -> seq=39 visible=true exclusion_rects=0 -> seq=40 visible=true exclusion_rects=1` 且卡死在 `seq=40`，说明对象被清空后，页面又重新进入了 native selection eligible 状态，并死在“恢复 WebView exclusion rect”这一步。
- 更稳的修复不是继续补 native runtime 节流，而是先切断前端误清空链路：
  - `select` 工具开始重画选区时，不在 `pointerdown` 立刻清空当前 `selection`
  - 只有拖出了有效新框，才提交新选区并 `resetAnnotations()`
  - 如果只是单击外部或极小位移，则保留原选区和现有对象
- 新一轮 toolbar 改造里，真正要收口的不是单个按钮样式，而是“多段纵向 JSX 结构”本身；只有把工具、样式、步进、对象操作、会话动作统一抽成 icon action 数据，才能稳定落成单行 rail。
- 单行 toolbar 一旦按钮数变多，固定 `left clamp 240` 会失效；这轮需要补 toolbar 实际宽度测量，再按测量值参与水平 clamp，否则选区靠边时工具栏会越界。
- 为了满足“toolbar 一行”的要求，`SelectionStatusBar` 不能继续塞在工具栏容器里，否则视觉上仍会变成多层堆叠；更稳的做法是把状态栏独立成上方浮层，toolbar 本身只保留 icon rail。
- 最新 `runtime-logs/log.log` 继续把截图崩溃窗口收窄到了“拖拽提交后”：
  - `native_interaction_drag_committed` 已成功完成，说明选区移动本身没有立刻炸掉。
  - 紧接着前端连续打出两条 `runtime_update_sent seq=12/13`，但没有任何 `runtime_update_completed`，之后进程退出。
  - 同一段日志里仍然能看到 `exclusion_rects_update_sent ... reason=selection_drag_start`，说明“拖拽开始时清空一次 exclusion rect”这条实验性路径仍在参与更新链路。
- 当前更危险的点不是普通拖拽绘制，而是“拖拽提交后的 runtime update 重入 / 并发窗口更新”。
- 更稳的修复方向应改为：
  - 前端彻底撤掉拖拽期 `update_native_interaction_exclusion_rects`
  - 拖拽期间冻结 Web toolbar 位置，不再隐藏后重建位置
  - 对 `update_native_interaction_runtime` 做 payload 去重和单飞串行
- 最新一轮本机日志继续证明，“一划完框立刻二次点击去拖”时，真正卡住的是“划框提交后的第一条 `runtime_update_sent seq=2`”：
  - `native_interaction_drag_committed ... drag_mode=creating` 已完成
  - 随后 `state_updated_applied drag_mode=none`
  - 接着发出 `runtime_update_sent seq=2 ... exclusion_rects=1`
  - 之后不再出现 `runtime_update_completed`
- 这说明即使 runtime update 已串行化，只要它在“drag 刚结束、用户马上二次按下”这个极窄窗口里执行，仍可能把 Native 交互拖死。
- 因此还需要再加一层“拖拽后短暂稳定窗口”：
  - selection drag 刚结束后约 220ms 内，前端不恢复 toolbar 洞位同步
  - toolbar 继续冻结在旧位置
  - 用户若立刻再次拖选区，只走纯 native selection interaction
- 当前截图颜色能力几乎全部集中在前端 `screenshot-overlay-page.tsx`：
  - `color` 本地状态
  - `applyColor(...)`
  - 工具栏颜色按钮
  - 选中对象后同步颜色草稿的若干 `setColor(...)`
- Rust / Native 侧虽然仍有 `annotation_color` 契约，但这轮不必改；前端只要固定传默认色，就能先把“颜色切换能力”从产品面去掉。
- 这轮最小正确做法不是“只隐藏按钮”，而是把前端颜色状态也收成固定默认色，并同步删掉“颜色/线宽”“颜色/字号”之类提示文案。
- 当前固定默认色已收敛为 `#00d08f`，这样能和现有选区边框、Native shape 默认草稿色保持一致，避免前后端默认色不一致。

## 2026-04-19
- `NativeInteractionWindow` 当前已经在像素缓冲里生成整屏遮罩：`build_base_mask_buffer()` 会把整张 buffer 填满统一的半透明遮罩底色。
- 选区外遮罩之所以当前几乎不可见，不是没画，而是 `sync_window_input_region()` 把窗口 `SetWindowRgn` 收缩到了选框输入区域附近，直接把整屏可见区域裁掉了。
- 同一个 `SetWindowRgn` 裁切还会切掉拖动中的边线/句柄，解释了“移动时边线消失一截再出现”的现象。
- 当前 exclusion rect 已同时具备：
  - `WM_NCHITTEST -> HTTRANSPARENT`
  - 像素缓冲透明清空
  因此完全可以把非交互区透明透传收敛到 hit-test，而不是继续复用 window region 做视觉裁切。
- 本轮修复策略是：
  - `SetWindowRgn` 改回全屏 region，只保留 exclusion rect 打洞
  - `WM_NCHITTEST` 新增“仅交互区命中”判定，让选框外区域透传到底层 overlay
  - 遮罩底色改为更偏灰的半透明蒙层，降低“纯黑洞口”的观感
- 新一轮回归暴露的 toolbar 残影，不是遮罩颜色问题，而是拖动期间前端跳过 `update_native_interaction_runtime` 后，旧 exclusion rect 没有单独刷新。
- 新日志已证明“拖动期间按最新几何单独推送 exclusion rect”会在 `drag_mode=creating` 下形成高频命令风暴，是错误修复方向。
- 更稳的做法是：
  - drag start 时清空一次 exclusion rect，立即去掉旧洞
  - drag 过程中直接隐藏 toolbar，不再追求每帧跟随
  - drag end 后靠正常 runtime update 恢复新位置

## 2026-04-19
- 当前 screenshot overlay 已支持单家族对象选择的方向键微调：`text / shape / pen / number / effect` 都有各自 `nudgeSelected*` helper。
- 跨家族混选已经具备 `mixed drag / mixed layer / mixed clipboard / mixed delete`，缺的只是统一的 `mixed nudge` 入口。
- 现有几何基础已经足够：
  - `resolveObjectSelectionGroupBounds(...)`
  - `clampGroupDeltaToSelection(...)`
  - `offsetObjectSelectionAnnotation(...)`
  - `restoreObjectSelections(...)`
- 当前键盘分发里，mixed selection 遇到方向键仍只弹 `“跨家族混选的方向键微调将在下一阶段接入”`，这说明问题是入口缺失，不是底层模型缺失。
- `nudgeSelectedMixedObjects` 直接复用现有 object group bounds/clamp/offset/restore 链路即可闭环，不需要再拆成五个家族分别移动；否则容易把组内相对位置和边界约束做散。

## 2026-03-18
- `log.log` 已证明本次“按截图热键立即无响应”不是截图会话启动失败：`received hotkey -> native_preview_window_shown -> native_interaction_session_prepared -> get_screenshot_session completed -> start_session_completed` 全部成功。
- 挂起发生在截图态建立之后；日志最后一个前端关键点是 `preview_image_decode_start`，最后一个 Rust 侧新增行为之一是 `escape_cancel_binding_applied source=global_shortcut shortcut=Escape`。
- 当前最值得优先审查的两条路径是：
  - `ScreenshotService` 运行期动态注册 `Escape` 全局热键
  - overlay 页 `raw_protocol` 预览解码后的渲染链路
- `screenshot-overlay-page` 在 `nativePreviewActive=true` 且没有 effect preview 时，原本仍会立即 `loadImage(previewImageSource)` 解码 WebView 预览图；这条 eager decode 与日志里最后一个前端关键点完全对齐，但该会话下 WebView 实际并不展示这张图。
- overlay 本地已经有 `keydown Escape -> handleCancel()`，而 NativeInteraction backend 也已有 `CancelRequested -> cancel_active_session_from_escape()`；因此 `ScreenshotService` 会话期再动态注册 `Escape` 全局热键是冗余路径，应移除以降低热路径风险。

## 2026-03-15
- 用户当前目标已从“继续抠 WGC/WebView 图片链路”切换为“无黄边 + 接近实时”，这要求 live capture 后端本身不再依赖 `Windows.Graphics.Capture` 的系统捕获边框机制。
- 在当前 `tauri dev` / 非 package identity 运行形态下，`GraphicsCaptureAccessKind::Borderless + graphicsCaptureWithoutBorder capability` 并不是可立即稳定落地的方案；即使发布态可做，也不能解决当前开发态黄边问题。
- 因此下一轮正确路线是 `Desktop Duplication API` 常驻最近帧原型，而不是继续围绕 `WGC borderless` 打补丁。
- `Desktop Duplication` 的实现关键约束来自官方 DXGI 文档：
  - `IDXGIOutput1::DuplicateOutput` 需要“来自目标输出适配器”的 D3D11 设备；
  - `AcquireNextFrame` 必须使用有限超时，不能永久阻塞 worker；
  - 成功获取一帧后必须及时 `ReleaseFrame`；
  - 更适合做“常驻最近帧缓存 + 热键时冻结”的路径。
- 当前工程下，`Desktop Duplication` 初始化失败时不应自动回退到启动期 `WGC live capture`，否则会重新引入 WGC 的系统黄边；更稳妥的兜底是仅保留现有热键后的 one-shot `WGC/GDI` 链路。

## 2026-03-14
- 在 Tauri 进程 DPI awareness 打开的场景下，`display_info.scale_factor` 可能固定为 `1`，即使系统缩放并非 100%；这会让“仅依赖 screenshots/display_info.scale_factor”的归一化策略失效。
- 针对上述机型，应优先使用 Tauri `available_monitors()` 的 monitor `scale_factor` 和 physical rect 来恢复 logical rect，再回落到 screenshots 侧启发式。
- 在部分 Windows 高 DPI 机型上，`display_info.width/height` 可能与截图返回的 `capture_width/height` 接近 1:1，而 `display_info.scale_factor` 仍大于 1；这说明 display 坐标语义可能偏向 physical。
- 当这类 raw display 坐标直接喂给 `Position::Logical/Size::Logical` 时，overlay 会被按逻辑像素再次放大，表现为“preview 巨大”。
- 更稳妥的做法是先做 monitor 级坐标标准化：依据 `capture/raw_display` 的 measured scale 与 reported scale 判定坐标空间，再统一折算为 logical display 坐标进入会话与窗口定位。
- 修复后需要保留结构化诊断日志（raw display、capture、reported sf、measured sf、normalized display、coordinate space），否则不同机型很难复盘 DPI 语义漂移。
- 预览性能瓶颈除 PNG 编码外，还包括“会话图像二次 clone”；此前 `CapturedMonitorFrame::to_image()` 每次会复制整块 RGBA 原图。
- 把会话原图缓存改成 `Arc<RgbaImage>` 并在 preview/crop 链路按引用访问，可显著减少 4K 场景下的内存复制开销。
- preview 编码改为 `PngEncoder(Fast + NoFilter)` 后，编码耗时可明显下降；为稳健性可保留失败回退到默认 PNG 编码路径。
- overlay 页面对窗口拖拽并无显式调用：仓库内未检索到 `data-tauri-drag-region / startDragging`，可排除前端主动拖拽窗口路径。
- 现有 screenshot overlay 链路缺少“运行期几何锁定”；窗口显示后若发生 `Moved/Resized/ScaleFactorChanged`，此前不会自动回正。
- 在高 DPI 与无边框窗口组合下，单次 `set_position/set_size` 后仍可能出现逻辑几何漂移；应记录“目标 vs 当前几何差值”并在漂移时回正。
- 你提供的新日志证明漂移依然存在且可人为拖动：`overlay_geometry_drift_detected` 的 `window_moved` 偏移可达 `(1412, 685)`，说明仅靠事件后回正不足以满足“不可移动”要求。
- `overlay_geometry_applied` 中 `outer_physical=3866x2173`（目标 4K 为 3840x2160）表明存在无边框窗口外框膨胀；几何探测应以 `inner_position/inner_size`（client 区域）为准，而不是 `outer_position`。
- Windows 场景需要额外做原生样式锁（去掉 `WS_CAPTION/WS_THICKFRAME/WS_SYSMENU` 等）才能从底层禁止拖动。
- 截图进入后的“黑屏等待”来自两点叠加：`sessionImageReady=false` 时页面仍渲染全屏 `bg-black/45` mask，且 overlay 窗口本身是非透明。
- 仅移除 loading 提示文案不够，必须同时处理“加载期渲染策略 + 窗口透明策略”，否则黑屏仍会出现。
- 对 uniform scale 场景，preview 生成可走 native 拼接路径，避免先缩到 logical 再编码，从而缩短底图就绪时间。

- 当前截图启动慢的主因不是热键注册，而是 `start_session()` 把抓屏、多屏拼接、PNG 编码、Base64 包装全部放在 overlay 显示之前；窗口真正出现前已经走完了一整条重链路。
- 当前 `capture_virtual_desktop()` 会把 `screenshots` 抓到的物理像素图缩回 `display_info.width/height`，同时会话 `scale_factor` 又被写死为 `1.0`；这意味着高 DPI 信息在后端已经丢失，前端再按统一比例导出只能做近似。
- `screenshots@0.8.10` 在 Windows 下实际上已经按 `display_info.scale_factor` 抓取物理像素大小，Bexo 当前额外的 `resize` 才是 DPI 丢失的关键步骤。
- `ParrotTranslator` 的可借鉴点不是它的单屏抓取路径，而是“显示图 + 原图”分离和最终导出时再做坐标回映射的架构。
- `screenshot://session-updated` 一旦改成同一 session 内的 `loading -> ready` 更新，前端不能再沿用“收到事件就整页 reset”的旧写法，否则 preview 一准备好就会把用户当前状态清空。
- 多屏 mixed DPI 场景下不存在一个天然统一的 native 像素网格；更稳妥的做法是后端显式区分 `native` 与 `logical_fallback`，在无法保证统一倍率时退回逻辑尺寸底图，而不是继续伪造全局 `captureWidth / displayWidth`。

- `voiceType` 的热键实现不是单纯依赖 `tauri-plugin-global-shortcut`；它在 Windows 上额外维护了一条 `WH_KEYBOARD_LL` 低层键盘钩子路径，用来承载 `RAlt`、`LWin+LAlt` 这类 side-specific / modifier-only 热键。
- `voiceType` 的默认 toggle 已经从历史 `Alt+B` 迁移到 `RAlt`，说明它自己也回避了 `Alt + 字母` 的不稳定路径。
- `voiceType` 明确限制高级热键中的 `Alt + 字母/功能键` 与 `Win + 字母/功能键`，理由分别是菜单抢占与开始菜单抢占；这套限制适合直接移植到 Bexo。
- Bexo 当前设置页录制器只支持通用 `Ctrl / Alt / Shift / Super`，不支持 `LAlt / RAlt` 这类 side-specific token，因此即使后端支持 hook，前端也必须同步扩展。
- Windows hook 热键线程如果在消息队列尚未创建时就接收 `PostThreadMessageW`，配置更新可能直接投递失败；启动线程时需要先显式创建消息队列，再接受配置消息。
- 若设置页仍坚持“热键必须包含非修饰键”，即使后端已支持 hook，`RAlt` 这类 modifier-only 热键也永远无法由用户录制出来；录制器必须单独放开“side-specific modifier-only”这一类组合。
- 截图工具热键属于 overlay 内部热键，不应复用 `tauri-plugin-global-shortcut` 的解析器；否则 `1~5` 这类单键会被误判为“热键格式无效”。
- 仅改 Settings 保存逻辑不够，overlay 如果仍用硬编码 `TOOL_HOTKEY_MAP`，用户配置不会生效；必须把读取偏好与事件匹配一起改造。
- 截图全局热键改为 `Ctrl+Shift+1` 后，与默认工具热键 `1~5` 在应用内不冲突：全局热键含修饰键，overlay 工具热键默认无修饰键。
- `settings-page` 中若直接在渲染阶段构造 `screenshotToolHotkeys` 对象，并在 `useEffect` 里无条件 `setScreenshotToolHotkeyDrafts`，会触发 React 无限更新；需要 `useMemo` 稳定引用并在 `setState` 前做值比较。

- 当前截图热键 `Ctrl+Alt+A` 已能成功注册到全局热键层，但日志里缺少对应的“收到热键”记录，说明问题更偏向 Windows 对该组合的实际投递稳定性，而不是注册失败。
- `tauri-plugin-global-shortcut` 在 Windows 下直接调用 `RegisterHotKey`，没有额外的补偿或去抖层；默认键位设计本身必须规避高风险组合。
- 只改前端默认值不足以修复老用户，因为历史默认 `Ctrl+Alt+A` 已可能持久化到 `settings/preferences.json`；必须在偏好初始化阶段补一次迁移。
- 对 `Ctrl+Alt+...` 更稳妥的策略是“提示风险但不强制拦截”，既能把问题暴露清楚，也不阻止少数用户继续自定义。

- 启动时看到“当前没有可用截图会话，请先按截图热键。”并不是普通对话框，而是预创建的 `screenshot_overlay` 独立窗口被错误显示到了前台。
- 本机 `C:\Users\aka86\AppData\Roaming\studio.bexo.desktop\.window-state.json` 当前只记录了 `main`，说明这次异常不是“overlay 上次状态被保存后恢复”，而是启动期窗口恢复逻辑本身把隐藏窗口显示了出来。
- `tauri-plugin-window-state@2.4.1` 的 `restore_state` 在“窗口没有已保存状态”的分支里仍会执行 `show() + set_focus()`；对预创建且 `visible: false` 的 `screenshot_overlay` 来说，这会把辅助窗口错误顶到最前面。
- `screenshot_overlay` 属于临时工具窗口，不应该参与通用窗口状态保存/恢复；更稳妥的做法是把它加入 window-state denylist，并在前端 overlay 页对“无会话”场景做自隐藏兜底。
- 你本次 `log.log` 已证明当前首帧慢点不在协议读文件，而在两段固定成本：`start_session_completed total_ms≈463~516ms` 与 `preview_image_ready total_ms≈522~567ms`；其中 4K BMP 编码约 `494~532ms`，已经是新的主瓶颈。
- 同一批日志里 `preview protocol served file` 紧跟在 `finish_preview_preparation_completed` 之后，说明自定义协议读取链路已通，不再是主故障点。
- overlay 页已经埋了 `preview_image_decode_* / preview_canvas_first_paint`，但当前只写 `console.info`，不会稳定进入 Tauri/Rust 持久化日志，因此 ready 之后的前端首帧阶段仍不可观测。
- 当前 overlay 在 `imageStatus=ready` 后会同时渲染整屏 `<img>` 与整屏 `<canvas>`，而 canvas 默认会再次把整张底图画一遍；即使没有 effect 预览，这次整屏重绘也会额外占用首屏预算。
- 对截图编辑 overlay 而言，首屏背景图只需要与逻辑显示尺寸一致；最终复制/保存仍走原始 capture 数据，因此高 DPI 单屏没有必要优先生成 3840x2160 native 预览文件。
- 你最新的 `log.log` 证明，“逻辑尺寸预览”这轮并没有把首帧做快，反而把瓶颈从 `encode_ms≈500ms` 转成了 `prepare_ms≈1200ms`；当前最慢的是 CPU resize，不是编码。
- `ParrotTranslator` 本地源码的关键点不是“缩得更好”，而是“显示路径根本不缩”：`QScreen::grabWindow(0)` 直接拿到 `QPixmap`，编辑窗口 `paintEvent` 直接 `drawPixmap(...)`，最终导出时才用 `scaleRect` 把逻辑选区映射回原始图。
- `ParrotTranslator` 的 `displayScreenshot` 与 `originalScreenshot` 是两套职责：显示快路径直接用 display pixmap，导出/复制才依赖原始图；这是当前 Bexo 应该对齐的结构。
- 在你这台 4K@200% 单屏机上，ParrotTranslator 的 `displayCapture.scaled(screenGeometry * devicePixelRatio)` 实际目标尺寸仍然是物理像素尺寸，本质上接近“保留原图 + 附带 DPR 元数据”，而不是先缩成逻辑图。

## 2026-03-11
- 仓库缺少 `docs/*` 文档与根级计划文件，已按当前任务补齐最小工作记录。
- 现有命令仅支持按 workspace 根路径打开终端，不支持资源路径定向打开。
- 新命令实现中，`targetPath` 若指向文件会自动回退到父目录，符合“在这里打开终端”预期。
- 为避免路径逃逸，后端对 `targetPath` 做了绝对路径、存在性、canonicalize 后工作区边界校验。
- 本机 `cargo test` 仍存在 `STATUS_ENTRYPOINT_NOT_FOUND`，当前可通过 `cargo check` + 前端构建完成编译级验证。
- VS Code 在部分环境会解析为 `code.cmd`，通过 `cmd.exe /C` 启动时会出现控制台闪窗；对 `cmd.exe` 启动加 `CREATE_NO_WINDOW` 可消除闪窗。
- `taskkill` 默认也可能触发短暂控制台窗口，现已同样加 `CREATE_NO_WINDOW` 与外部命令启动策略保持一致。

- 截图能力属于系统敏感操作，必须 Rust 侧实现热键/采集/剪贴板，前端只做交互与绘制。
- Phase A 热键采用 action-based 契约：统一事件 `hotkey://trigger` + `action` 字段，当前先落地 `screenshot_capture`，并预留语音输入动作键位。
- `docs/` 目录在当前仓库不存在，后续按根级 planning 文件与 `scripts/work/...` 继续维护变更说明。
- `tauri-plugin-global-shortcut` 已在本仓库依赖与 capability 中就绪，可直接承载 Phase A 的全局热键注册。
- 热键更新流程必须和偏好保存联动；否则会出现“设置保存成功但运行态未生效”的断裂。已在 `PreferencesService` 中串联并加入失败回滚。
- `screenshots@0.8.10` 在 Windows 下返回的是物理像素截图，而 `DisplayInfo.width/height` 是逻辑像素；Phase B 已通过 `scale_factor` 做选区坐标换算，避免高 DPI 下裁剪偏移。
- 为规避 Windows 上运行时创建 webview 的事件线程风险，Phase B 将 `screenshot_overlay` 窗口改为 `tauri.conf.json` 预创建（默认隐藏），截图时只做定位/显示。
- overlay 关闭事件已接入会话清理，避免残留旧 session 影响下一次热键截图。
- Phase C 采用前端矢量标注模型 + 导出位图策略：避免后端重复实现图形引擎，同时保留 Rust 侧对剪贴板/文件写入的控制。
- 为保证导出结果与屏幕像素一致，导出时使用 `captureWidth/displayWidth` 与 `captureHeight/displayHeight` 比例将标注坐标映射到物理像素后再合成 PNG。
- 后端复制/保存命令新增可选 `renderedImage` 入参，兼容“仅选区截图”（Phase B）与“带标注导出”（Phase C）两条链路。
- Phase D 将抓屏模型从“单主屏”升级为“虚拟桌面多屏拼接”，可覆盖负坐标显示器布局。
- DPI 映射不再依赖单一 `scale_factor`，改用双轴比例映射，避免异形比例或拼接场景下选区偏移。
- overlay 快捷键采用分层冲突处理（Esc 优先清理本地编辑态，最后才取消会话），避免误关截图流程。
- 文本工具从 `prompt` 升级为原位输入时，必须显式处理 IME 组合态；否则中文输入按 `Enter` 选词会被误判为“提交标注”。
- 原位文本输入与复制/保存按钮存在 blur -> click 的事件顺序问题；通过 `ref` 持有当前编辑态和标注数组，可避免失焦提交后导出遗漏最新文字。
- 若文字标注支持拖动和二次编辑，现有“仅追加”的撤销栈模型不够用；需要切换为 annotation 快照历史，才能覆盖移动/更新场景而不丢撤销语义。
- 文字编辑态若允许点击工具栏调色/改字号，`textarea` 的 blur 不能直接提交；需要识别 `relatedTarget` 是否仍在工具栏内，否则会把“即时预览”打断成一次提交。
- 文本样式一旦扩展到描边/背景/高亮，命中框、拖动边界、SVG 预览和 Canvas 导出必须共用同一套文本布局计算；否则选中框和最终导出会错位。
- 文本再扩展到旋转后，矩形包围盒已不足以支撑命中测试；需要对指针点做逆旋转，再用统一布局模型做命中与边界约束。
- 复制/粘贴不应直接复用系统剪贴板图片链路；截图 overlay 内的对象复制更适合维护独立的“文字对象剪贴板”，否则会和整张截图复制动作混淆。
- 对齐吸附不宜只做“坐标修正”而没有视觉反馈；拖拽时需要同步渲染辅助线，否则用户会感知为对象突然跳动。
- 马赛克/模糊属于“基于底图像素重采样”的效果，不能用普通 SVG 线框近似；预览和导出都必须走 Canvas 像素级处理才不会失真。
- 编号标注不能分别在 SVG 和 Canvas 里各自手写尺寸规则；需要抽出统一的徽标布局计算，才能保证多位数编号、导出结果和屏幕预览一致。
- effect 对象的“选中框”与“实际像素效果”应解耦：效果仍走 Canvas 预览，命中与可视化反馈只需要基于 effect 矩形做独立 overlay，避免把选中态塞回像素处理链路。
- effect 拖动和缩放必须采用“预览态 + mouseup 一次提交”的模式；如果在 pointermove 每帧写入历史栈，撤销会碎片化且明显拖慢大面积模糊预览。
- 编号对象虽然是圆形徽标，但命中和拖动边界应基于统一的徽标布局计算，而不是仅拿点击点做半径猜测；否则大字号或多位数编号会出现拖动选不中的问题。
- 编号字号二次编辑不能只改 `size` 字段而忽略选区约束；字号变大后必须重新走 `clampNumberAnnotationToSelection`，否则对象会被放大到选区外侧。
- 编号对象复制/重复不应挤进现有文字对象 clipboard；更稳妥的做法是沿用同样的“会话内对象剪贴板”模式，但保持 number/text 各自独立，避免 `Ctrl/Cmd+V` 时混淆选中语义和反馈文案。
- 编号对象做层级前后移时，不需要新建第二套 z-index 模型；继续复用 `annotations` 数组顺序做相邻交换，才能保证 SVG 预览、命中测试和 Canvas 导出顺序始终一致。
- effect 对象复制/重复不需要单独设计几何模型；直接复用 `resolveEffectAnnotationBounds + createEffectAnnotationWithBounds`，再叠加统一的 offset / 边界回退，就能和拖拽缩放后的对象状态保持一致。
- effect 对象做层级前后移时同样不该额外维护 effect 专属层级；继续接入统一的 `moveAnnotationLayer` 才能保证 effect 与文字/编号混排时的顺序解释一致。
- effect 方向键微调应延续文字对象的“单次按键即一次历史提交”语义，而不是合并成隐藏批处理；这样撤销行为才和现有编辑器模型一致，也更容易控制边界回退。
- 编号方向键微调最稳妥的实现仍是复用 `commitSelectedNumberMutation + clampNumberAnnotationToSelection`；这样字号变化后的命中半径和边界回退会自动沿用已有编号布局规则。
- 编号快捷提示的可视强化更适合放在 `NumberSelectionOverlay`，因为这是唯一始终跟随对象、且不会进入导出结果的装饰层；如果放进主工具栏，只会重复全局信息而不能建立对象就地关联。
- effect 快捷提示同样应放在 `EffectSelectionOverlay`，并复用现有局部标签区域扩展；这样能和 effect 的选中框/句柄形成同一视觉组，不需要新增任何可点击控件。
- 编号/effect 多选的第一阶段不该直接复制文字对象那套完整分组编辑器；更稳妥的是先把“主选中 id + 选中 id 列表”模型接进现有单对象路径，再复用现有 mutation helper 扩展到批量删除、层级和方向键微调。
- 对编号/effect 做基础多选时，拖拽/缩放不宜立即扩展成组变换；先让普通点击回落到单对象拖拽，`Ctrl/Cmd+点击` 只负责维护选择集合，能显著降低几何和历史栈复杂度。
- 在多选尚未支持分组复制/重复前，工具栏和快捷键都应显式限制为单对象复制/重复，避免“看起来选中了多个，实际只复制主对象”的隐式行为。
- 编号/effect 分组拖拽适合复用文字对象的“预览态 delta + mouseup 一次提交”模式；这样可以直接沿用现有撤销栈语义，并避免 pointermove 期间持续写历史。
- effect 进入多选后，缩放句柄命中必须继续限定在“仅单选 effect”场景；否则分组拖拽和句柄缩放会在命中入口上互相抢占。
- 编号/effect 分组复制最稳妥的做法是直接对齐文字对象 clipboard 结构，统一为 `items + groupBounds + pasteCount`；这样可以直接复用已有 `resolvePasteOffset`，不需要再造第 3 套粘贴偏移规则。
- 组内对象复制后不应逐个再做独立 clamp，否则靠边场景会破坏相对位置；只要原始 groupBounds 来自当前选区内对象，并用 `resolvePasteOffset` 先裁好整组偏移，就能整体保持队形。
- 框选多选和“重画截图选区”共享拖拽手势时，最稳妥的分流条件是“是否在现有截图选区内起拖且当前存在编号/effect 对象”；框内拖拽走对象框选，框外拖拽仍走截图选区重绘。
- 框选同时命中编号和 effect 时不能混选；按“当前已选家族优先，否则取框内最上层命中对象的家族”决策，比按数量或固定优先级更符合当前点击选择语义。
- 置顶/置底不该引入独立 z-index 字段；直接在 `annotations` 数组上做稳定重排即可，才能保证 SVG 预览、命中测试和 Canvas 导出对顺序的解释一致。
- 多选对象做置顶/置底时，必须保留组内相对顺序；否则一次操作后同组对象会互相穿插，用户会把它感知成顺序损坏。
- 文字对象的 `置顶/置底` 逻辑其实已经接入统一层级分发；当前缺口只在 `TextSelectionOverlay` 没有把这组快捷入口显式露出，属于入口层不对齐，不是能力层缺失。
- 文字对象的就地提示不应单独造一套浮层体系；直接沿用 `TextSelectionOverlay` 的非导出装饰层扩展，才能保持与当前选中框、旋转包围盒和拖动命中完全同源。
- 图形对象最稳妥的首版二次编辑粒度是“单对象编辑闭环”，先把命中、拖动、缩放和样式回写接进现有单对象路径；如果一上来就做多选，命中优先级和几何句柄冲突会显著放大复杂度。
- 线条/箭头与矩形/圆形的编辑模型不应完全拆开；统一抽象成 `move + handle` 变换模式，只在句柄几何求解上按图形类型分支，能最大程度复用当前 effect 的交互骨架。
- 画笔路径的首版二次编辑不宜直接做“逐点编辑”或缩放；先把整条路径看成一个可移动的对象，再补颜色/线宽回写，能在复杂度可控的前提下覆盖绝大多数返工场景。
- 画笔命中最稳妥的是沿折线逐段做 `point-to-segment` 距离测试，并把容差和 `strokeWidth` 联动；只看包围盒会导致长路径大面积误选。
- 画笔复制/重复没有必要再造第三套偏移策略；直接复用现有 `resolvePasteOffset`，才能保持文字、编号、effect、画笔的对象复制体验一致。
- 画笔当前仍是单对象选择，但 clipboard 结构可以先保持 `items + groupBounds + pasteCount` 的统一契约；这样后续扩展画笔多选时，不需要再改粘贴分发协议。
- 画笔多选的第一阶段不应该直接跳到分组拖拽；先把“主选中 id + 选中 id 列表”接进现有单对象链路，补齐批量删除、层级和方向键微调，复杂度更可控。
- 多选画笔阶段不能静默退化成“复制主对象”；在没有分组 clipboard 前，要么禁用工具栏按钮，要么在快捷键路径上明确提示“当前仅支持单个画笔复制/重复”。
- 画笔分组拖拽最稳妥的状态模型是保存 `originAnnotations + delta + groupBounds`，而不是在 `pointermove` 期间持续回写点数组；这样可以直接复用现有预览/撤销语义。
- 画笔分组拖拽应先只做整组平移，不做分组缩放；自由路径一旦引入组缩放，点集重采样和线宽语义都会马上复杂化。
- 画笔多选组复制不需要再发明专用 clipboard；既然 `PenClipboardState` 已经是 `items + groupBounds + pasteCount`，直接放开多选写入即可，后续粘贴逻辑天然成立。
- 组复制后的选中态必须直接切到新组，而不是只选中新组最后一条路径；否则用户在紧接着继续拖动或调层级时会感知成“整组复制不完整”。
- 画笔框选不能只靠 `resolvePenAnnotationBounds` 做包围盒相交；长折线路径会在空洞区域产生明显误选，至少要补“路径点在框内或线段穿过框边”这一层实际几何命中。
- 画笔接入对象框选后，`canStartObjectMarquee` 不能在 `pointerdown` 就清掉当前画笔选择；否则 `pointerup` 时会丢失当前家族，导致增量框选和同家族优先决策失效。
- 图形当前虽是单选模型，但 clipboard 仍应沿用 `items + groupBounds + pasteCount` 统一契约；这样后续放开图形多选时，粘贴偏移和分发协议无需再改。
- 图形粘贴最稳妥的是继续复用 `offsetShapeAnnotation` 整体平移 `start/end`，而不是按图形类型各自重建；这样线条、箭头、矩形、圆形都能沿用同一套偏移规则。
- 图形多选首版不宜直接扩成分组拖拽；先把 `selectedShapeId + selectedShapeIds` 接进现有单图形链路，补齐批量删除、层级和方向键微调，能把句柄命中和历史栈复杂度控制在可验证范围内。
- 图形多选下复制/重复不能静默退化成“只复制主对象”；键盘分发、工具栏入口和反馈文案都应显式限制为单图形复制/重复，避免用户对当前选择范围产生错误预期。
- 图形分组拖拽不需要为线条/箭头/矩形/圆形分别设计不同的组移动模型；直接复用 `offsetShapeAnnotation + resolveShapeGroupBounds + clampGroupDeltaToSelection` 就能覆盖全部图形家族，并保持边界约束语义一致。
- 图形组拖拽的第一阶段不应带上分组缩放；单图形仍保留句柄编辑，多图形只做整组平移，能避免句柄命中与组变换在同一入口上相互抢占。
- 图形 clipboard 既然已经沿用 `items + groupBounds + pasteCount` 统一结构，就不该继续人为限制多选写入；放开后可以直接复用已有 `resolvePasteOffset` 做整组复制/粘贴/重复。
- 图形组复制后的选中态必须切到新组，而不是只保留最后一个图形为单选；否则用户复制后紧接着继续拖动、调层级或改样式时，会感知成“组复制不完整”。
- 图形接入对象框选后，`objectSelectionMarquee` 启动时不能先清掉当前图形选择；否则“当前家族优先”会在 pointerdown 就被抹掉，增量框选和同家族优先都会失效。
- 图形框选命中不必强行复刻点击命中的精细度；线条/箭头用线段与矩形相交，矩形/圆形用外接 bounds 相交即可，足以满足对象框选语义，同时保持实现可控。
- 框选提示层如果直接另写一套命中判断，后续很容易和 `pointerup` 最终提交分叉；这轮已改为完全复用同一解析函数。
- 对象框选的可视强化不应该覆盖现有选择模型，而应在框选阶段只表现“将会发生什么”，这样追加框选和普通框选的心智最稳定。
- 文字框选若只用 `bounds`，旋转文本会出现明显“视觉覆盖但未选中”的错觉；最稳妥的是直接复用 `resolveTextAnnotationLayout` 的角点和边线。
- 文字一旦接入统一对象框选，当前家族优先也要把 `text` 放在同一决策链里，否则文字追加框选会退化成切家族重选。
- 组选中框最稳妥的做法不是发明新的组几何，而是直接复用各家族已有的 `resolve*GroupBounds`；这样拖动、复制和层级后，组框总能跟现有对象语义保持一致。
- 主对象提示收敛不等于强行统一文案内容；更合理的是统一渲染壳和排列，再保留各家族自己的操作语义提示。
- 状态栏真正需要统一的是数据模型，而不是文案模板本身；先抽出 title / subtitle / chips，后续混选才有空间做标签并集而不是再复制条件分支。
- 预览态、单选态、多选态如果共用同一渲染壳，后续跨家族混选只需要扩展状态组合逻辑，不需要再改工具栏结构。
- 跨家族混选第一阶段不应直接放开所有现有捷径；先统一删除、层级和全选，再对复制/重复/方向键显式拦截，才能避免“部分家族生效”的半成功状态。
- 混选场景下对象框选若继续依赖当前已选家族优先，会把跨家族 additive 选择重新拉回单家族心智；当已选家族数大于 1 时让 `preferredFamily` 回退为 `null` 更稳定。
- 跨家族 mixed selection 一旦开始整组拖拽，入口优先级必须高于单家族拖拽和句柄编辑；否则点击已选对象会先回退成单家族，mixed selection 会被意外打散。
- mixed group drag 不需要第 6 套专用位移算法；直接复用统一 `ObjectSelectionAnnotation` 偏移和统一 group bounds，就能让预览、提交、组选中框和边界约束保持同源。
- mixed clipboard 复制源不能按家族拼接；必须按 `annotations` 原始栈顺序提取当前选中对象，否则跨家族复制后组内相对层级会被打乱。
- mixed paste / duplicate 不该再单独维护第 6 套“混选恢复”逻辑；继续复用 `split buckets + restoreObjectSelections` 才能保证粘贴后各家族主选中对象、组选中框和后续 mixed drag 保持一致。
- 当前用户本机持久化的 `preferences.json` 里，`hotkey.screenshotCapture` 实际是 `LCtrl+LShift`；这类“仅修饰键”的截图热键会在按下第二个修饰键时误触发/不稳定，必须在启动时自修复，不能只靠设置页录制器约束新输入。
- `Bexo Studio` 当前真正的系统级全局热键只有后端 `HotkeyService` 注册的 `screenshotCapture / voiceInputToggle / voiceInputHold`；资源浏览器和截图 overlay 里的 `Ctrl+Shift+...` 只是页面内或 overlay 内快捷键，不能把它们和系统级热键混为一谈。
- `tauri-plugin-global-shortcut` 支持运行时 `unregister_all`/重新注册；当应用内热键状态和插件内部注册态发生漂移时，先清空再重建比按缓存逐个反注册更稳，适合用来修复“恢复默认提示已占用”这类自占用问题。
- 截图 overlay 的“黑屏等待”不只是图片编码耗时问题；只要全局 `body` 背景仍是主题色，窗口在底图未就绪时就会直接显示为整屏黑底/深色底。
- 仅把 overlay 根容器改透明不够，必须在 HTML 启动早期就对 `overlay=screenshot` 打标，并覆盖 `html/body/#root/.ant-app` 四层背景，否则仍会先闪一次主题背景。
- 若目标是“不要任何过渡界面”，仅做透明化仍不够；必须把 overlay 的 `show` 时机后移到 `preview_ready` 之后，预热阶段保持隐藏。
- 把窗口几何和原生样式锁前置到隐藏预热阶段，可以避免 show 瞬间出现“标题栏/边框态”的短暂闪现。
- 但“`show` 延迟到 `preview_ready`”会直接把 `preview_prepare+encode`（当前约 2s）暴露成可感知等待，不符合“按下即进工作态”的交互预期。
- 对该产品诉求，更合适的策略是“overlay 立即进入工作态 + 底图异步到位”，而不是“窗口延迟显示”。
- 顶部短暂蓝色标题栏是 Windows 非客户区（原生标题栏）在 overlay show 初期的瞬时闪现；其触发特征与 `overlay_geometry_drift_detected current_logical@7,0` 同步出现。
- 抑制该闪现需要窗口层处理：overlay title 置空 + show 后短轮询稳定 `style/geometry`，单靠前端样式无法解决。
- 当前“按下热键后 4~5 秒才看到冻结底图”的主因不是 `screen.capture()` 本身（该段通常几百毫秒内），而是后续链路叠加：
  - 全屏图（4K）预览拼图 + PNG 编码
  - 大体积 Base64 字符串跨 Tauri invoke 传输到前端
  - 前端 `loadImage(dataUrl)` 解码与首帧绘制
- `get_screenshot_session` 目前返回完整 `image_data_url`，这会把几十 MB 级字符串搬运成本放在“会话读取”链路里，容易产生明显体感延迟。
- 将预览主通道改为“临时文件路径 + `convertFileSrc`”后，IPC 只传小字符串路径，不再传输数十 MB 的 `dataUrl`，可直接消除此段大包序列化/反序列化成本。
- 预览编码选择“BMP 优先 + PNG 回退”能用更低 CPU 成本换取更快首帧，且保留像素无损，适合截图编辑底图场景。
- 单屏场景下，预览拼图不应总是走“新建黑底 + overlay”路径；直接复用单监视器图像可减少一次大块内存搬运。
- 在当前工程里，仅把预览改成 `convertFileSrc(tempPath)` 还不够；如果 `asset` 协议 feature/scope 未显式启用，这条链路会表现为“后端 ready 了，但前端底图完全不出现”。
- 对这个场景，自定义受控协议比继续追 `asset` 配置更稳：路径白名单、日志、MIME 和错误响应都能在应用内闭环控制。
- 若用户贴回的运行日志里仍然看不到 `preview_transport / get_screenshot_preview_rgba / preview_raw_fetch_*`，应先判定该日志来自旧构建或旧运行，而不是继续基于那份日志分析当前代码路径；否则会把排查重新带回已经废弃的 file/resize 分支。
- 最新 `log.log` 已证明 `raw_rgba_fast` 本身是通的，当前剩余最大单段耗时不是 Rust `get_preview_rgba`（仅约 `3~4ms`），而是前端 `preview_raw_fetch_done fetch_ms≈313~318ms`；这基本就是 `33MB RGBA` 经由 Tauri invoke 进入 JS 的搬运成本。
- 在当前 Tauri/WebView 架构下，若目标继续压到“接近实时”，下一步不该再优化 `putImageData` 或 Rust 会话查询，而应把单屏首屏预览改成“协议直供图片给 WebView 原生解码”，减少 JS 大包拷贝。
- 对截图 overlay 来说，“raw fast”真正该对齐的不是“前端拿到 raw RGBA”，而是“首屏显示阶段不让 JS 处理整张图”；让 WebView 直接从 custom protocol 解码图片，才更接近 ParrotTranslator 的原生显示思路。
- `DynamicImage::write_to(ImageOutputFormat::Bmp)` 在这条链路里不够快，首屏图若继续走 BMP，最好直接手写 BMP header + BGR row copy，避免通用编码器的额外开销。

## 2026-03-15
- 你最新的 `log.log` 已证明当前 1 秒延迟的主要成本是：`capture_ms≈330ms`、`overlay_ready_ms≈125ms`、以及 4K 首帧 `encode_ms≈370ms + 浏览器侧 onload 完成≈585~621ms`。
- 现有 `RawRgbaFast` 已不再经过 JS 大包搬运，但仍然把 `3840x2160` 首帧图送给协议与 WebView2；当前慢点已经从 JS IPC 转移为 4K 首帧图的编码/解码。
- 本地 `screenshots-0.8.10` crate Windows 后端确认仍是 GDI：`CreateCompatibleBitmap + StretchBlt + GetDIBits`。
- 因此下一轮最优先的收益点不是继续抠前端 `<img>` 细节，而是改成“逻辑尺寸快预览图 + 后台原图补齐”的 ParrotTranslator 式分离结构。

- 本轮实现从“快预览图 + 后台原图补齐”进一步收敛为“同次 GDI 双产物”：一次采集同时拿到原图和逻辑尺寸预览，避免预览/导出时刻不一致。
- 最新 `D:\Desktop\rust\BexoStudio\log.log` 已证明首帧图片链路已基本压缩完成：`get_preview_protocol_bmp_completed encode_ms=0 encode_path=cached`，前端 `preview_image_decode_done` 常态仅约 `52~102ms`，不再是主瓶颈。
- 该日志下真正主瓶颈已经稳定收敛为两段：`capture_ms≈399~422ms` 与 `overlay_ready_ms≈119~144ms`；热键到首帧的核心链路大约是 `518~567ms + 88~161ms`。
- 当前单屏快路径仍然是重型 GDI 双产物：同一次截图里执行 `CreateDCW/CreateCompatibleDC/CreateCompatibleBitmap + StretchBlt(raw) + StretchBlt(preview) + GetDIBits(raw) + GetDIBits(preview)`，这会把 4K 原图与 1080p 预览都在热键后同步取回，结构性成本仍高。
- 当前 overlay 也不是“纯热窗口切换”；每次截图仍会走 `hide -> set_decorations/set_resizable -> lock_native_style -> set_position/set_size -> show -> stabilize -> focus`，这解释了稳定的 `overlay_ready_ms≈120ms+`。
- 微调 BMP、WebView `<img>`、JS decode 的边际收益已经接近耗尽；若目标继续逼近微信/Qt 那种无感首帧，下一步必须优先动抓屏后端与 overlay 生命周期，而不是继续抠前端图片细节。
- Microsoft 官方资料给出的更合适方向是：
  - `Windows.Graphics.Capture` + `IGraphicsCaptureItemInterop::CreateForMonitor` 直接为 Win32 桌面应用创建 monitor capture item；
  - `Direct3D11CaptureFramePool::CreateFreeThreaded` 让 `FrameArrived` 在内部 worker thread 触发，避免 UI 线程/Dispatcher 依赖；
  - 或使用 `Desktop Duplication API` 保持一个持续的帧流会话，在热键时直接冻结最近一帧，而不是热键后才启动整套抓屏流程。
- 若产品目标是“几乎实时”，最值得做的不是一次性 one-shot 截图更快一点，而是“常驻捕获会话 + 最近帧缓存 + 热键时立即冻结”，这样才能绕开当前 `capture_ms≈400ms` 这段结构性等待。
- 2026-03-15 本轮已落地 `Windows.Graphics.Capture` 单显示器原型：
  - 使用 `RoInitialize(RO_INIT_MULTITHREADED)` 初始化 WinRT；
  - 通过 `IGraphicsCaptureItemInterop::CreateForMonitor` 为 `HMONITOR` 创建 `GraphicsCaptureItem`；
  - 使用 `Direct3D11CaptureFramePool::CreateFreeThreaded` + `FrameArrived` + `TryGetNextFrame` 取得首帧；
  - 通过 staging texture `Map/Unmap` 读回 top-down BGRA，再复用现有逻辑生成 raw RGBA 和预览 BMP。
- 该 WGC 实现已设计为“只在 Windows + 单显示器快路径尝试一次”，若任一阶段失败，直接记录 `wgc_capture_failed` 并回退现有 GDI 路线，不会中断主截图功能。
- overlay 预热已改为启动期执行一次：先将 overlay 移到 `(-32000,-32000)` 的 `1x1` 隐藏区域，锁定无边框样式后 `show -> sleep(24ms) -> hide`，将窗口实例保温以减少后续截图时的原生窗口建立即时成本。
- `ScreenshotState` 已增加 `overlay_prewarmed` 状态位，`start_session_completed` 现在会明确记录本次截图是否命中预热窗口。
- 2026-03-15 最新 `D:\Desktop\rust\BexoStudio\log.log` 已证实：
  - WGC 抓帧本身并不慢，`wgc_capture_completed total_ms≈181~256ms`；
  - 但同一次 `wgc_capture_attempted elapsed_ms≈801~873ms`，说明 WGC 之后的本地构建链路额外吃掉了约 `545~617ms`；
  - 这段额外成本来自当前代码仍在热路径上执行：
    - `rgba_image_from_top_down_bgra()` 的 4K 全图 BGRA->RGBA CPU 逐像素转换；
    - `build_preview_bmp_from_top_down_bgra_windows()` 内的 GDI `CreateDCW/CreateCompatibleBitmap/StretchDIBits/GetDIBits`；
    - 预览 BMP 包装与内存拷贝。
- 同一份日志也表明，overlay 预热已把 `overlay_ready_ms` 从此前约 `120~140ms` 压到 `64~81ms`，但仍会出现 `overlay_geometry_drift_detected`，说明窗口虽然预热了，截图时仍在做一次从 `1x1` 隐藏态到全屏态的原生几何修正。
- 现阶段热键到首帧的大头不再是 WebView decode：
  - `preview_image_decode_done from_loading_seen_ms≈77~142ms`
  - 但 `start_session_completed total_ms≈869~957ms`
  - 所以当前接近 1 秒的体感主要由“截图链路本身”决定，而不是前端显示决定。
- 对照微软官方资料，真正高收益的方向应改为：
  - 复用长期存在的 `GraphicsCaptureItem / D3D11 device / Direct3D11CaptureFramePool / GraphicsCaptureSession`，而不是每次热键都重建；
  - 避免在首帧热路径里做 GPU->CPU 读回与整图像素格式转换；
  - 让首帧直接走 GPU 纹理到 swap chain / composition surface 的显示路径；
  - 若需要“几乎无感”，进一步走持续捕获会话或 Desktop Duplication 最近帧缓存，而不是热键后再启动 snapshot。
- 2026-03-15 本轮实现的关键点不是继续抠 BMP/WebView，而是把 `WgcLiveCaptureHandle` 接入业务层：启动期建立常驻 WGC 会话，热键时优先消费最近一帧缓存，再回退 one-shot WGC/GDI。
- `CapturedMonitorFrame` 现已支持 `bgra_top_down + OnceLock<RgbaImage>` 双形态；这让热键路径可以只搬运 BGRA，不必立即做 4K `BGRA -> RGBA` 逐像素转换，真正需要裁剪/复制/保存时再 materialize。
- 只要 `start_session()` 仍然走一次性 `capture_single_monitor_fast_preview()`，日志里的 `capture_ms` 就还会包含抓屏本身；这轮已经加上 `capture_strategy=live_cache`、`frame_age_ms`、`live_capture_sequence`，后续是否真的脱离现抓要直接看这几个字段，而不是只看总时延。
- 为避免 overlay 污染最近帧缓存，live capture 回调现在会在活动截图会话存在时丢弃新帧，并以节流日志记录 `live_capture_frame_dropped reason=active_screenshot_session`。
- 当前 live cache 路线仍保留了“热键时用缓存 BGRA 生成逻辑尺寸 BMP 预览”这一步，目的是先把“热键后现抓”从主路径摘掉；如果你本机日志显示 `capture_strategy=live_cache` 后总时延仍明显大于目标，下一阶段就该直接上 native preview/composition，而不是再抠这层 BMP。
- 2026-03-15 最新 `log.log` 已证明 live cache 接入后，当前主要剩余成本分布大致为：`capture_ms≈100ms`、`overlay_ready_ms≈85~88ms`、`preview_first_paint from_loading_seen_ms≈80~160ms`；总体验已从约 1 秒压到零点几秒。
- 这 100ms 的 `capture_ms` 已经不再是抓屏 API，而是 `try_capture_from_live_cache()` 中的“缓存 BGRA -> 逻辑尺寸预览 BMP”热键态构建成本。
- 因此下一刀最划算的不是继续换抓屏 API，而是把预览 BMP 也前移到 live capture 回调里预生成；这样热键路径只读缓存，不再做任何预览编码工作。
- 当前日志里的 `overlay_geometry_drift_detected current_logical=1x1@7,0 -> 1920x1080@0,0` 说明 overlay 仍保留了“从 1x1 预热态切回全屏”的窗口几何修正成本；若本轮把热键 `capture_ms` 压下去后体感仍不够，下一阶段应优先继续砍 `overlay_ready_ms`，而不是回头优化图片协议。
- 2026-03-15 最新 `log.log` 已证实这轮预览缓存前移生效：`live_capture_snapshot_used ... preview_source=cached` 稳定出现，`capture_ms` 已从上一轮约 `100ms` 压到 `0~1ms`。
- 现阶段主瓶颈已经彻底收敛到窗口层：`overlay_ready_ms≈80~96ms`，同时每次热键都会出现 `overlay_geometry_drift_detected current_logical=1920x1080@7,0`，首轮还会额外经历 `1x1 -> 1920x1080` 的预热态切换。
- 这说明继续优化图片链路的收益已经小于窗口几何链路；下一步应优先绕开 Tauri 逻辑尺寸定位，直接用 Win32 物理坐标/尺寸设定 overlay。
- 单屏快路径下，`capture_width/height` 与 `display_width/height * scale_factor` 已经一致，可以安全把 overlay 几何直接下发到 `SetWindowPos(hwnd, ..., physical_x, physical_y, physical_width, physical_height)`；多屏/非统一缩放仍保留原 Tauri 逻辑坐标实现作为回退。
- 2026-03-15 最新冻结日志证明，上述 Win32 物理坐标路径在当前 Tauri/WebView 窗口模型里并不安全：窗口外框被设为 `3840x2160` 后，逻辑客户区却稳定落在 `1907x1074@-2,0`，导致 `Moved` 事件每次都会继续判定漂移，形成无穷回正循环。
- 这不是“性能优化副作用可以接受”的级别，而是明确的功能性回归。当前必须先回到稳定的逻辑坐标定位基线，再继续优化 `overlay_ready_ms`，否则后续所有性能数据都不可信。
- 2026-03-15 最新 `log.log` 表明当前热键到截图态已基本进入实时区间：`start_session_completed total_ms≈88~93ms`，`preview_first_paint from_loading_seen_ms≈79~83ms`，主流程已经不是性能问题。
- 第二次截图时“先闪上一次截图”不是后端把旧图片又发了一遍，而是前端复用同一个 overlay 窗口时，旧 `session` 的 `<img>` 在新会话 `loadSession()` 返回前还继续渲染。
- 仅在 `useEffect([session])` 中把 `previewSurfaceReady` 置 `false` 不够，因为这一步发生在新 `session` 写入之后；而旧图闪屏发生在 `session_updated_event` 到 `loadSession()` 完成之间这段窗口重用期。
- 对这个问题，最小且正确的修复点是：
  - 在收到新的 `sessionId` 事件时立即清空 `previewRenderableRef`、关闭 `previewSurfaceReady`、重置交互态；
  - 让底图 `<img>` 只在 `previewSurfaceReady` 为真时渲染，并以 `session.sessionId` 为 React `key`，避免旧 DOM 节点复用。
- 2026-03-15 你看到的“程序启动后整屏四边黄色线”不是应用自己画的 UI，而是 Windows Graphics Capture 的系统级捕获边框。
- 根因很直接：我们此前在 `setup()` 阶段就调用了 `initialize_live_capture()`，这会让应用一启动就开始常驻 monitor capture，于是 Windows 立即给整块显示器画出捕获边框。
- 当前代码虽然调用了 `GraphicsCaptureSession.SetIsBorderRequired(false)`，但这不足以在现有运行形态下真正关闭边框；微软官方要求还包括 `GraphicsCaptureAccess.RequestAccessAsync(GraphicsCaptureAccessKind.Borderless)` 与 `graphicsCaptureWithoutBorder` capability。
- 在当前 `tauri dev` 的 Win32 运行形态下，持续依赖这套 `Borderless` 能力并不稳妥；因此本轮采用的工程取舍是：先取消“启动即常驻 capture”，优先消除系统黄边，再保留单次 WGC 抓帧路径。
- 2026-03-15 默认截图热键变更不能只改一个常量；当前代码里至少有三层要同步：
  - Rust domain 默认值
  - Rust 偏好修复/迁移逻辑
  - 前端默认值和设置页默认文案
- 当前版本之前经历过两次默认值演进：`Ctrl+Shift+4 -> Ctrl+Shift+1 -> Ctrl+Shift+X`。如果迁移逻辑只处理一个旧默认值，就会导致一部分老用户停留在旧默认，而不是跟随新默认。
- 因此本次正确做法是同时保留：
  - `PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY = Ctrl+Shift+1`
  - `EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY = Ctrl+Shift+4`
  - 并在 `migrate_legacy_preferences()` 中统一迁移到 `DEFAULT_SCREENSHOT_CAPTURE_HOTKEY = Ctrl+Shift+X`
- 2026-03-15 最新 `desktop_duplication_live_cache` 日志已表明采集端瓶颈基本消失：`capture_ms≈1ms`，当前主要剩余成本集中在 `overlay_ready_ms≈100ms+` 与 WebView 首帧显示。
- 继续优化图片链路的边际收益已经很低；下一阶段最有效的方案是让 overlay 脱离 `1x1 -> 全屏 -> 回正` 的窗口切换路径，改为“全尺寸透明热态常驻”。
- 对当前 Tauri/Windows 组合来说，`set_focusable(false) + set_ignore_cursor_events(true)` 可以把 overlay 维持为可见但不抢交互的透明热态；激活时再切回可交互状态，比反复 `hide/show` 更符合剩余瓶颈位置。
- 若退出截图态后仍继续 `hide()` overlay，则热态设计无法生效；因此取消/复制/保存之后必须同步改成“恢复透明热态 + 清空前端 session 视图”，否则会再次出现旧图残留或重新走窗口显示路径。
- 2026-03-15 新日志已确认：这轮性能确实继续下降，最佳样本 `start_session_completed total_ms≈33ms`、常态约 `93~129ms`；但窗口抖动老问题再次出现，根因不是采集退化，而是 overlay 从透明热态切回可交互态后会自行偏移到 `-1,0`。
- 当前实现里 `geometry_reused=true` 的判断发生在激活前；而实际位移发生在 `set_focusable(true) / set_focus()` 之后，所以旧逻辑会把“激活前对齐、激活后偏移”的窗口错误地当成可复用几何。
- 解决这类抖动的正确位置不是继续抠 prewarm，而是“激活后再 probe 一次，并立即回正”；否则日志不会出现 `overlay_geometry_drift_detected`，但用户仍会看到窗口闪动位移。
## 2026-03-15 Overlay 抖动复盘补充
- 最新日志显示 `overlay_hot_state_activated ... corrected_after_focus=false`，但随后 `overlay_geometry_applied current_logical=1920x1080@-1,0`，说明对齐判定把 `-1px` 位置偏移当成了已对齐。
- 根因在 `OverlayGeometryProbe::is_aligned()`：之前对 `delta_x/delta_y` 使用了 `abs() <= 1` 容差，导致 post-focus 回正路径不会触发。
- 热态恢复链路每次强制 `set_overlay_window_geometry()` 也会放大开销；改为 probe 后仅在漂移时重设几何更合理。
## 2026-03-15 Overlay 无响应根因补充
- 最新 `log.log` 显示热键后立即进入 `overlay_geometry_drift_detected trigger=window_moved` 风暴，位置在 `7,0 -> -2,0` 间反复抖动，没有任何 `start_session_completed`/前端首帧日志，属于 Rust 侧窗口事件自激，不是前端卡死。
- 严格位置对齐判定本身是对的，但必须配套“忽略程序化几何事件”机制，否则 `set_overlay_window_geometry()` 触发的 `Moved/Resized` 会再次回调 `enforce_overlay_window_geometry()`。
## 2026-03-16 Overlay 无响应止血结论
- 最新日志显示卡死发生在 `window_moved` 事件风暴，且完全没有进入 `start_session_completed`，说明 UI 无响应发生在 Rust 侧 overlay 几何监听环路内。
- 事件抑制窗口方案不够稳，因为 Windows 仍可在抑制窗口外继续发出程序化 `Moved` 事件；对 overlay 这种固定全屏窗口，更稳妥的方案是撤掉自动监听，改为仅在显式激活流程里做几何控制。
- `tauri-plugin-log` 已支持 `TargetKind::Folder`，可直接把日志固定写入仓库根目录，避免继续依赖 OS log dir 或手工复制控制台输出。
## 2026-03-16 Overlay 生命周期判断补充
- 用户截图里的 `00:15` 控制台表明：`preview_first_paint from_loading_seen_ms=136.6`，所以图片链路不是 2 秒瓶颈；同时又出现 `overlay_geometry_drift_detected trigger=restore_hot_state` 和 `overlay_hot_state_restored total_ms=319`，说明真实问题是截图结束后的 overlay 热态恢复本身。
- 对 overlay 这种固定全屏窗口，可见热态/restore 链比隐藏稳态更脆弱，尤其在 Windows + Tauri 透明顶置窗口场景下会放大闪屏和几何漂移。
## 2026-03-16 无限重载根因补充
- `tauri dev` 会监听 `src-tauri` 目录变化；固定日志文件写进 `src-tauri\log.log` 后，相当于每条日志都在修改被监听文件，从而触发无限重编译。
- 这不是截图逻辑问题，是开发态日志目标目录选错了。
## 2026-03-16 Overlay 激活慢的直接根因
- 最新固定日志已证明采集链路不是瓶颈：`capture_ms=1`，`preview_first_paint from_loading_seen_ms≈130~150ms`；真正拖到 `1.1~1.3s` 的是 `overlay_ready_ms`。
- 根因在 `move_and_focus_overlay_window()`：几何复用条件写成了 `overlay_prewarmed && was_visible && geometry_aligned`。由于预热后的 overlay 本来就是隐藏的，`was_visible=false` 导致每次热键都被强制走 `set_overlay_window_geometry() + stabilize_overlay_window_after_show()`，把隐藏预热窗口错误地当成“未预热”处理。
- 同一份日志里的 `overlay_window_stabilization_fallback_exhausted` 和 `overlay_geometry_drift_detected trigger=post_focus_activation` 说明这条错误路径不仅慢，还会放大焦点切换后的几何漂移。
- 另外，`emit_session_updated()` 原先在 overlay 激活之后才发，导致隐藏窗口无法提前加载新 session，放大了可见阶段的白闪窗口。
## 2026-03-16 Live Cache 退回 WGC 的直接根因
- 最新固定日志还表明另一条退化路径：`capture_strategy=wgc_single_monitor` 且 `capture_ms=996ms`，同时 `live_capture_available=true`。这说明 `Desktop Duplication` 后台线程是活的，但热键瞬间没有拿到可用 snapshot，于是直接退回了一次性 WGC。
- 当前代码原先的行为是：`try_capture_from_live_cache()` 只要返回 `None`，就立即执行 `capture_single_monitor_fast_preview()`；这会把“启动后很快按热键”“snapshot 短暂过期”“后台帧刚好错过轮询窗口”这些场景全部放大成 1 秒级回退。
- 对当前 `LIVE_CAPTURE_MIN_INTERVAL_MS=80` 与 `LIVE_CAPTURE_MAX_FRAME_AGE_MS=250` 的设置来说，加一个很短的等待窗口比立即退回 WGC 更合理，因为 `Desktop Duplication` 很快就能产出下一帧。
## 2026-03-16 Overlay 激活真正的主瓶颈
- 你手动复制的控制台日志已经把热路径拆开了：`capture_ms=1`、`preview_first_paint from_loading_seen_ms≈146~199ms`，但 `overlay_activation_profile ... stabilize_ms≈1003~1265ms total_ms≈1123~1410ms`。
- 这说明上一阶段真正拖慢体验的不是采集，也不是图片解码，而是 `stabilize_overlay_window_after_show()`。
- 更关键的是，这段稳定化每次都以 `overlay_window_stabilization_fallback_exhausted` 结束，并且随后仍然要靠 `post_focus_activation` 再做一次 `@-2,0 -> @0,0` 的回正；也就是说它既慢，又没有真正解决问题。
- 对“hidden 但已 prewarmed”的窗口来说，继续跑这段稳定化没有收益，只会把窗口可见阶段变成 1 秒级阻塞和抖动来源。
## 2026-03-16 Overlay 白闪剩余根因补充
- 用户最新手动复制的 `D:\Desktop\rust\BexoStudio\log.log` 只有启动段，没有包含真实截图会话；当前必须以固定日志 `runtime-logs\log.log` 或完整控制台截段为准，不能只看被截断的 tail。
- 现有全局 CSS 虽然已经对 `html[data-overlay="screenshot"] body/#root/.ant-app` 设置透明，但这些规则依赖主 CSS 包加载完成；在 WebView 首次 show 的极短窗口里，仍可能先刷到系统默认白底。
- 对这个阶段，最小且正确的修复不是继续抠 React 组件，而是把透明样式前置到 `index.html <head>`，让 overlay 查询参数一命中就立即生效。
- `move_and_focus_overlay_window()` 此前把 `hidden_prewarmed` 直接等价为 `needs_geometry=true`，即便隐藏预热窗口已经对齐也会重放一次 geometry；这会放大可见前的微小位移。当前更合理的策略是只在 probe 未对齐时才重新设几何。
## 2026-03-16 Overlay 微抖动剩余根因补充
- 最新日志已证明当前剩余主成本稳定落在 `focus_ms + realign_ms`，而且 `overlay_geometry_drift_detected trigger=post_focus_activation current_logical=...@-2,0` 在同一台机器上反复一致。
- 这说明问题已经不是随机抖动，而是“窗口激活后有稳定的逻辑坐标偏移”。继续每次事后回正，只会把这 170~200ms 永久留在热路径里。
- 对这种稳定偏移，最有效的做法不是继续扩大容差，而是记录一次补偿值，在下次激活前直接预补偿；若设备环境变化，再根据新的 probe 自我修正。
- 2026-03-16 当前截图链路已经证明：继续优化 `capture_ms` 和图片协议都不是主方向。`Desktop Duplication` 让采集端接近 0 成本后，剩余上限主要受限于：
  - 全屏 Tauri/WebView overlay 的窗口激活/焦点模型
  - WebView 首帧底图显示模型
- 这意味着“无感实时”的正确方向不是继续抠单 WebView overlay，而是分层：
  - `Desktop Duplication` 继续作为最近帧来源
  - `NativePreviewWindow` 负责底图显示
  - `NativeInteractionWindow` 负责选区和高频交互
  - `NativeToolbarWindow` 负责截图运行时小工具栏
# 2026-04-30 Findings: Windows autostart registry quoting and silent launch

- Settings 页两个开关的前端链路存在，`handleToggleStartupSetting()` 会通过 `update_app_preferences` 保存 `startup.launchAtLogin / startup.startSilently`。
- Rust 侧也已接入 `tauri-plugin-autostart`，并配置了 `--autostart` 参数；app setup 中已有 `--autostart + startSilently` 的隐藏主窗口判断。
- 真实失效点在 Windows autostart 写入格式：本地依赖 `auto-launch 0.5.0` 在 Windows 下写 `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` 时直接拼接 `app_path + args`，没有给 exe 路径加引号。
- 对 `Bexo Studio.exe` 或安装目录包含空格的路径，这会导致 Windows 登录启动时无法按预期启动目标 exe，进而 `--autostart` 不会可靠进入进程，`静默启动` 也就看起来无效。
- 另一个可靠性缺口是 `tauri.conf.json` 中主窗口默认 `visible=true`；即使 setup 后续隐藏窗口，也可能在静默启动场景先闪出主窗口。
- 最终修复采用 Windows 专用注册表写入：Run 值格式为 `"完整\Bexo Studio.exe" "--autostart"`，并在应用启动时按保存偏好重新同步系统 autostart 状态；主窗口改为默认隐藏，由 setup 决定显示或保持隐藏。

---

  - `Tauri/WebView` 只保留非高频配置 UI
- 若直接一步把全部交互都改成 native，风险过大；更稳的路径是：
  - 先原生化底图
  - 再逐步下沉高频交互和工具栏
- 2026-03-16 Phase B 第一轮不应该直接把 runtime path 切过去；更稳的落地方式是先引入 `NativePreviewService` 骨架，把：
  - 生命周期状态
  - session 规格
  - Windows backend 类型入口
  - app setup 注入点
  先固定下来。
- 这样下一轮实现真正的 `NativePreviewWindow` 时，不需要再一边搭模块边界，一边改截图热路径，能显著降低回归风险。
- 2026-03-16 在不新增 DirectComposition feature 的前提下，Phase B 仍然可以先把底层图形上下文打通：
  - `ID3D11Device`
  - `ID3D11DeviceContext`
  - `IDXGIFactory2`
- 这意味着下一轮真正接 `NativePreviewWindow` 时，只需继续补：
  - window creation
  - swap chain
  - composition tree
  不需要再回头重做 bootstrap。
- 2026-03-16 `NativePreviewService` 如果直接把 `HWND` 和 `windows` crate COM 接口放进 `tauri::Manager::manage()` 的共享状态，会因为 `Send + Sync` 约束直接编译失败。
- Phase B 当前合理做法是把 backend 资源放到堆上，用 opaque handle 挂在状态层；这样服务边界和生命周期先固定下来，后续再根据 render thread 模型决定是否进一步正规化。
- 2026-03-16 `CreateSwapChainForComposition` + `CreateTargetForHwnd` + `SetContent` + `Commit` 这条链路已经能在当前项目中完成静态编译并通过 `cargo check`，说明 `NativePreviewWindow` 所需的最小 Windows 图形基础设施已经可用。
- 2026-03-16 Phase B 当前最合理的最小运行时接入，不是立即切掉 WebView 底图，而是先把 single-monitor BGRA frame 提交到 native swap chain，并把 show/hide/resize 路径跑通。这样能先验证 native runtime 生命周期，而不把底图、交互、工具栏三层同时重构。
- 2026-03-16 `Desktop Duplication` live cache 产出的 `bgra_top_down` 已经足够直接喂给 `IDXGISwapChain1` back buffer；当前 one-shot `wgc/gdi` fast preview 路径生成的是 `RgbaImage`，不适合作为本轮 native preview 的首批接入对象。
- 2026-03-16 `NativePreviewService` 需要有比“准备 session”更低层的 runtime API：必须显式区分 `prepare_session_frame` 与 `show/hide/clear`，否则 service 层仍然只是状态机，无法验证真正的 native runtime path。
- 2026-03-16 当 native preview 进入截图显示链路后，不能简单把 WebView 底图逻辑整段删掉。更稳妥的 Phase B 方案是：让 WebView 继续在后台 decode 同一张截图，只是不再把 `<img>` 作为底图渲染到屏幕上；这样效果预览和现有交互状态机还能继续工作。
- 2026-03-16 因此需要一个明确的运行时契约位：`ScreenshotSessionView.nativePreviewActive`。没有这个标志，前端无法区分“旧底图链路仍在显示”与“原生底图已经接管，只需保留交互层”。
- 2026-03-16 只要 `nativePreviewActive=true`，overlay 的交互就绪判断也不应再强制等待 `previewSurfaceReady`；否则原生底图虽然已经可见，WebView 仍会因为等待图片 decode 而人为延迟交互。
- 2026-03-16 当前 Phase B 的下一个真实问题不是“有没有原生底图”，而是“native preview 与 overlay 只是同时位于 topmost 带里，没有稳定的相对顺序”。仅靠两个窗口都 `TOPMOST`，在 show/focus 之后仍会被 Windows 重排。
- 2026-03-16 对这个问题，正确修复点不是继续调 overlay 几何，而是把 overlay HWND 作为 native preview 的显式锚点：首次显示时 `show_below_window(anchor_hwnd)`，overlay 激活完成后再 `sync_z_order_below_window(anchor_hwnd)`，这样“底图在下、交互层在上”的关系才是确定的。
- 2026-03-16 原生预览若已显示，而 `replace_active_session()` 等中途步骤失败，必须立即执行 `hide_and_clear_native_preview()`，否则会留下孤立的原生顶层窗口。这类失败清理必须在 Phase B 早期补齐。
- 2026-03-16 当 `Phase B` 已经把底图稳定交给原生层后，下一阶段最合理的推进方式不是继续让 WebView 承担高频交互，而是先建立 `NativeInteractionWindow` 的服务骨架和 Windows backend 入口。
- 2026-03-16 `NativeInteractionWindow` 第一轮骨架的目标不是马上替换现有选区逻辑，而是先固定：隐藏透明窗口、生命周期状态机、最小 show/hide/resize API，以及启动期注入点。这样后续原生化 hit test / drag / handle 时不会再一边改架构一边改交互热路径。
- 2026-03-16 最新手动控制台日志表明 `native_preview_window_shown`、`native_preview_z_order_synced`、`capture_strategy=desktop_duplication_live_cache` 都稳定出现，说明 Phase B 足够稳定，可以开始迁移高频交互而不是再回头修底图。
- 2026-03-16 对 Windows 交互层来说，`WS_EX_LAYERED + UpdateLayeredWindow` 是本轮最合适的 MVP 路径：它允许原生全屏半透明遮罩、透明开洞显示底图，并且窗口本身能直接处理鼠标消息和拖拽捕获。
- 2026-03-16 Phase C MVP 应只承接基础选区交互：新建选区、移动选区、8 向句柄 resize、鼠标捕获；复杂标注和工具栏继续留在 WebView，可以显著降低切换风险。
- 2026-03-16 对 `NativeInteractionWindow` 的第一轮运行时接线，最稳妥的模式不是“一刀切抢走全部输入”，而是只在 `tool === select && annotations.length === 0` 时启用 native base selection；一旦进入复杂标注阶段，仍回落到现有 WebView 交互。
- 2026-03-16 Microsoft 官方文档确认：使用 `UpdateLayeredWindow` 的 layered window 上，alpha 为 0 的区域会把鼠标消息透传到下面窗口。因此 toolbar / text editor 与 native interaction 并行的正确做法是 exclusion rect，而不是继续拆更多窗口。
- 2026-03-16 由于前端需要持续读取原生选区状态，`get_native_interaction_state` 这类轮询命令不能继续保留 `info` 级别日志，否则会快速刷爆控制台和固定日志；当前已降为 `debug`。
- 2026-03-16 用户最新日志已证明：截图启动和底图显示没问题，基础框选卡顿时 `capture_ms=0~25ms`，但 `native_interaction_session_prepared present_ms=349~367ms`，并且拖拽期间 `NativeInteractionWindow` 处于高频 `WM_MOUSEMOVE` 热路径。
- 2026-03-16 `native_interaction_backend_windows.rs` 在优化前每次 `present` 都重建整套 GDI 资源：`GetDC -> CreateCompatibleDC -> CreateDIBSection -> SelectObject -> UpdateLayeredWindow -> DeleteObject/DeleteDC/ReleaseDC`。这条链位于鼠标移动热循环内，是基础框选卡顿的首要工程问题。
- 2026-03-16 当前最稳妥的修复方式不是继续改截图采集或 WebView 轮询，而是让 `NativeInteractionWindow` 复用 GDI surface：screen DC、memory DC、DIBSection 只在窗口尺寸变化时重建，拖拽帧只做 buffer copy + `UpdateLayeredWindow`。
- 2026-03-16 全屏遮罩渲染同样有确定性 CPU 成本。将每帧整屏 `for pixel` 填充改为预生成 `base_mask_buffer` 后直接 `copy_from_slice`，可以在不改变绘制语义的前提下降低 4K 全屏遮罩的 CPU 写入开销。
- 2026-03-16 最新日志显示上述热路径优化已经生效：`native_interaction_drag_committed ... avg_present_ms=6~7 max_present_ms=11~15`，说明拖拽渲染本身已不再是主瓶颈。
- 2026-03-16 在渲染均值降到个位毫秒后，用户仍感到轻微卡顿，说明剩余延迟来自“状态同步和反馈链”，不是 `UpdateLayeredWindow`。当前最可疑的点是 WebView 每 `40ms` 轮询 `get_native_interaction_state()`。
- 2026-03-16 轮询模型还有一个明确副作用：截图结束后，前端 effect 清理与后端 `clear()` 之间会打架，导致 `update_native_interaction_runtime failed ... SESSION_NOT_PREPARED` 噪声日志。
- 2026-03-16 对高频交互来说，cursor 也必须跟 hit test 一起在 Native 侧决定；如果 hover 命中与 cursor 反馈分离，用户会直接感觉“拖拽手感黏滞”。这类问题继续留在 WebView 侧只会增加同步成本。
- 2026-03-16 下一类最值得下沉的复杂标注不是文字、马赛克或画笔，而是 `rect`。矩形和当前原生选区共享几何模型，能以最小代价验证“Native 创建草稿，WebView 只接收提交结果”的模式。

## 2026-03-16 Findings: Native interaction residual lag
- After persistent GDI surface reuse, selection drag present cost dropped to low single-digit milliseconds; remaining lag was no longer raster cost but state synchronization overhead.
- The remaining mixed-mode latency came from WebView polling `get_native_interaction_state()` every 40ms and from cursor/hover still being split between WebView and Native.
- Correct next step is event-driven state sync plus native hover/cursor ownership, followed by migrating the simplest high-frequency annotation family (`rect`) to Native first.

## 2026-03-16 Findings: State jitter and second shape migration
- After event sync landed, the remaining small interaction jitter came from a feedback loop: native selection updates changed React `selection`, which retriggered `update_native_interaction_runtime` even in selection mode.
- Breaking that loop requires treating Native as the sole owner of selection state while `interactionMode=selection`; the frontend should only consume events, not echo the same selection back.
- For the second native annotation family, `ellipse` is the safest next step because it reuses the same bounding-box interaction model as `rect` while still validating that the shape-commit protocol can support more than one annotation kind.

## 2026-03-16 Findings: Screenshot overlay UX should not expose full editing chrome before region selection
- 用户截图已直接证明当前问题不是 native 预览性能，而是 overlay 页面运行时 UX：顶部常驻说明条造成视觉遮挡，全量工具栏导致初始截图态像“完整编辑器”而不是“先选区后编辑”。
- 合理业务流应为：进入截图 -> 先框选区域 -> 再出现紧凑的区域编辑工具栏；高级控制仅在对应上下文出现。
- 因此此阶段应优先压缩 WebView 运行时 chrome，而不是继续向截图首帧链路追加性能优化。

## 2026-03-16 Findings: White flash and Esc failure were caused by two separate runtime ownership gaps
- 白闪并不全是首帧慢，而是部分会话退回 `wgc_single_monitor` 后 `native_preview_status=skipped`，底图重新落回 WebView `img` 路径。
- `Esc` 失效不是取消逻辑不存在，而是焦点不稳定：高频鼠标交互期间 NativeInteractionWindow 可能成为实际输入窗口，导致 WebView 的 `window.keydown` 不再可靠。
- 这类问题不应该继续靠“确保 WebView 拿焦点”来赌，正确做法是让 NativeInteractionWindow 自己发出取消请求事件。

## 2026-03-16 - White flash and Esc cancel findings
- `wgc_single_monitor` 回退时 `CapturedMonitorFrame.bgra_top_down=None`，导致 `build_native_preview_runtime_inputs()` 直接返回 `None`，native preview 被跳过，出现白闪回退到 WebView 底图。
- `Esc` 取消不能依赖 WebView `keydown`，也不能依赖 `SW_SHOWNOACTIVATE` 的 NativeInteraction 窗口过程。当前项目已存在稳定的 `WH_KEYBOARD_LL` manager，适合做截图会话级 `Esc` 取消闭环。

- 2026-03-17: 日志证明 screenshot 专用 cancel hook 已 apply(binding_count=1) 但从未产生 escape_cancel_hook_triggered；根因不在 overlay hide，而在回调根本没触发。改为 NativeInteractionWindow 直接 RegisterHotKey(Esc) 并通过 NativeInteractionBackendEvent::CancelRequested 驱动 ScreenshotService.cancel_active_session_from_escape。
- 2026-03-17 用户新日志已经直接证明：同一截图会话内多次出现 `native_interaction_drag_started ... drag_mode=creating hit_region=none`，因此“区域外不能重新划选”不是后端交互逻辑锁死，而是光标反馈误导了用户判断。
- `NativeInteractionWindow` 如果只在状态变化时 `SetCursor`，但不处理 `WM_SETCURSOR`，Windows 仍可能在下一次光标查询时恢复成默认/底层样式。这正是当前区域外出现“禁止”光标的高概率原因。
- 正确修复点是原生窗口过程：显式处理 `WM_SETCURSOR`，每次根据当前 `hovered_hit_region/drag_mode/interaction_mode` 强制回写 Native cursor。这样既不动现有 hit test 状态机，也能消除错误光标反馈。
- 2026-03-17 最新用户澄清了业务规则：基础截图并不是“允许随时重划新选区”，而是只允许首次框选；形成有效选区后，区域外应禁止新建，仅允许移动/缩放当前选区。这意味着之前把“区域外可重新 creating”当作正确行为的判断不符合产品要求。
- 同一份日志还明确给出了 `Esc` 失败根因：`update_native_interaction_runtime failed ... NATIVE_INTERACTION_ESCAPE_HOTKEY_REGISTER_FAILED`。这不是消息没收到，而是 `RegisterHotKey(Esc)` 在当前实现里根本注册失败。
- 因此正确修复不是继续调 WebView 焦点，也不是继续赌 `RegisterHotKey`，而是：
  1. 在 Native 选区状态机里锁住已有选区后的区域外 `creating`
  2. 用项目现成的 `WindowsHookHotkeyManager` 绑定 `Esc`，直接发 Native cancel request
- Phase C 当前不是全 Native。现状是：
  - Native：Desktop Duplication、NativePreviewWindow、NativeInteractionWindow（基础选区/句柄/部分 shape）
  - WebView：工具栏、复杂标注编辑流程、配置与非高频 UI
- 最新修复针对 Native 交互业务规则：已有选区后锁住区域外重建；Esc 取消改由低级键盘 hook 驱动，不再使用无效的 RegisterHotKey(Esc)。
- 2026-03-17 最新日志其实已经证明 Esc 取消链路在某些时刻是通的：日志中存在 `native_interaction_escape_cancel_requested`、`escape_cancel_completed`。因此问题不是“Esc 功能不存在”，而是它当前挂载在 NativeInteraction 局部路径上，实测仍然有时序/焦点依赖。
- 更稳的做法是把 Esc 取消再上移到 ScreenshotService 会话级：只要截图会话存在，就立即通过 `WindowsHookHotkeyManager` 绑定 Esc；会话清除时统一解绑。这能和 NativeInteractionWindow 的显示、焦点、show/hide 完全解耦。
- 当前截图运行时仍然是混合架构，不是全 Native：Native 负责采集、预览、基础选区/部分 shape，高频主链路已在 Native；WebView 仍负责工具栏、复杂标注编辑和非高频 UI。
- 2026-03-17 最新日志已经直接暴露出 `Esc` 不稳定的结构性原因：进程内同时存在三条 Windows 低级键盘 hook 线程，其中两条与截图取消相关。重复 hook 会导致同一按键在不同线程里被竞争消费。
- 日志里已经能看到 `ScreenshotService` 会话级 `Esc` 取消成功，因此 NativeInteraction 自己再保留一条取消 hook 没有价值，反而制造不稳定。
- 正确修复是单路径原则：截图态只保留 `ScreenshotService` 会话级 Esc hook，NativeInteraction 不再管理 Esc。

- 2026-03-17: 最新 log 显示 escape_cancel_hook_triggered 能成功，但触发时机不稳定；windows_hook_hotkey 单键 Esc 理论上应在 WM_KEYDOWN 触发，说明问题更可能在当前会话输入链路。决定改为 tauri-plugin-global-shortcut 的临时 Escape 注册为主路径，hook 仅作回退。

- 2026-03-17: 确认截图态 Esc 未响应的硬根因是 tauri-plugin-global-shortcut 在回调执行时持有 shortcuts 锁，而我们在回调里同步注销 Escape 会反向拿同一把锁，造成死锁。已改为在回调中异步派发取消，待回调退出后再执行会话清理和注销。

- 2026-03-17: 截图态 Esc 已稳定后，下一阶段的正确方向是把运行时工具栏从 WebView 拿到 Native，并优先下沉 arrow 这类高频 shape。

## 2026-03-17 Findings: Phase D should add arrow before forcing runtime toolbar cutover
- `arrow` 比 text/effect 更适合作为第三个 Native shape：它和现有 `rect/ellipse` 一样都是几何对象，只需要新增 draft 线段与提交事件，不需要改前端注释模型。
- 当前前端 `shape-annotation-committed` 已经是泛化协议，前端现有 `ShapeAnnotation.kind` 也已支持 `arrow`；因此最稳的做法是扩展 NativeInteraction 的 mode/kind 枚举，而不是重新设计一套 arrow 专用事件。
- `NativeToolbarWindow` 本轮不应该急着替换现有 WebView toolbar。先把隐藏 tool window、生命周期和 app 注入做成稳定骨架，再在下一轮接 selection anchor 和原生按钮，否则会把 Phase D 风险一次性放大。
- 2026-03-17: 修复截图态无法切换到 arrow/tool 按钮的问题。
  - 根因：`NativeInteractionWindow` 的 exclusion rect 只清除了视觉遮罩，没有在 Win32 命中测试里返回 `HTTRANSPARENT`。
  - 处理：在 `WM_NCHITTEST` 中对 toolbar/text-editor exclusion rect 命中后返回 `HTTRANSPARENT`，让下层 WebView 真正接收点击。
- 2026-03-17: 运行时 toolbar 点击不稳的根因，不只是 exclusion rect 的 `HTTRANSPARENT`，而是 `NativeInteractionWindow` 仍保持全屏输入区域。即使局部打洞，窗口命中和层级时序仍可能挡住 toolbar。更稳的输入模型是：初始框选时全屏输入；形成有效选区后，把 Native 输入区域收缩到“选区本体 + 句柄 padding”，把区域外输入交还给 overlay/WebView。
- 2026-03-17: toolbar 仍不可点的更深层根因，是 `NativeInteractionWindow` 的输入区域只在 `prepare_session/update_runtime` 时重算，但选区创建/移动/缩放是在 backend 内部完成的。视觉选区更新后，如果不立即重算 window region，实际挡鼠标的仍是旧 region（常见为全屏或旧位置），于是 toolbar 继续被挡住。
- 2026-03-17: `NativeToolbarWindow` 不再只是 skeleton。当前已经确认更稳的切换方式是：Rust 命令负责 runtime state 推送，Native toolbar 只发 `native_toolbar://action` 事件，WebView 只做业务动作分发和高级控制兜底。这样不会把截图复制/保存逻辑重写一遍。
- 2026-03-17: `NativeToolbarWindow` 的 Win32 后端采用真实 `BUTTON` 子窗口，而不是继续赌 layered/WebView exclusion 命中。原因很直接：工具切换、颜色、复制/保存/取消本质是标准命令按钮，没必要留在 WebView 才做得快。
- 2026-03-17: `line` 是 arrow 之后最合适的下沉对象。它和 `arrow` 共用两点几何模型，Native backend 只需要维护不带箭头的线段草稿即可，不需要引入新提交协议。
- 2026-03-17: NativeToolbarWindow 点击工具按钮后卡死的高概率根因，不是按钮命中本身，而是 `WM_COMMAND -> app_handle.emit -> WebView listener -> setTool/applyColor -> updateNativeToolbarRuntime` 形成了同步重入链。Win32 按钮消息处理栈内不应该同步再触发 toolbar 自身 runtime 回写。
- 更稳的修复方式是双保险：
  1. Rust 侧把 toolbar action 改为异步派发，保证 `WM_COMMAND` 先返回。
  2. Rust/前端两侧都对“值未变化”的工具/颜色更新做 no-op，避免无意义回写。
- 2026-03-17: 在 Native toolbar 已经接管工具/颜色/复制保存取消之后，下一批最应该下沉的不是新图形，而是 `撤销/重做/线宽` 这类运行时基础操作。原因：它们高频、边界清晰，且能立即减少截图态 WebView 主工具栏的职责。
- 这类运行时动作最适合使用离散动作而不是数值输入框：`undo / redo / decrease_stroke_width / increase_stroke_width`。这样 Native toolbar 不需要先引入复杂输入控件，也能完成最小闭环。
- 2026-03-17: 修复 Native toolbar 接管前 WebView 旧 toolbar 先闪一下的问题。
  - 原因：`showWebToolbarPrimaryRows` 之前依赖 `nativeToolbarState.visible`，需要等一次异步 runtime 回执，导致 WebView 主行先渲染一帧。
  - 处理：改为只要 `nativeToolbarRuntimeVisible` 成立，就提前把 WebView 主行隐藏，由 native toolbar 负责接管主运行时工具栏。

- 2026-03-17: 当前单对象 shape 编辑态的关键缺口不是创建，而是 active shape 协议缺失。已补 ctiveShape 运行时输入和 shape-annotation-updated 事件。

- 2026-03-17: 单对象 shape Native 编辑态的真实首个阻塞点，不一定在 Native backend 命中/拖拽，而可能在 WebView 的对象‘首击选中’入口。ect/ellipse 若只按 outline 命中，用户点击内部时会被误判为未命中，从而触发 object marquee 或重新框选，看起来像‘无法选中对象’。

- 2026-03-17: 当前截图态主工具栏消失的直接根因，不一定是 native toolbar 没准备，而是前端过早用 
ativeToolbarRuntimeVisible 接管了 WebView 主行。更稳的规则是：只有 
ativeToolbarActive（native 已确认 visible 且 session 有效）时，才隐藏 WebView 主工具行。

- 2026-03-17: shape 工具下必须把‘已有对象候选列表’下沉到 NativeInteraction runtime，不能只传 activeShape。否则 Native backend 在 line/rect/ellipse/arrow 模式只能无条件进入 *_creating，无法智能判断‘点中已有对象则编辑，点空白则新建’。
- 2026-03-17: 单对象 Native 编辑态除了 backend 命中，还必须把 native activeShape 反向同步到前端 selectedShape；否则对象即使在 Native 中已命中，WebView 仍会认为未选中，工具状态和编辑 UI 会漂移。
- 2026-03-17: native toolbar 多轮截图后回落到 WebView 的高概率根因仍在 Win32 show/hide 时序，已补 SWP_SHOWWINDOW 并保留 WebView 兜底；需要结合本轮回归确认是否已稳定。
- 2026-03-17 20:15 继续排查“画完对象切回选区后无响应”。根据手动控制台日志，卡死前最后一条为 `native_interaction_drag_started ... drag_mode=resizing ... mode=selection`，说明问题不在 shape 创建/移动，而在切回 `tool=select` 后的 selection resize 链路。
- 本轮前端收口：1) 从 shape 工具切回 `select` 时先清掉 shape 选中态，避免把上一轮 `activeShape` 带入 selection resize；2) `tool=select` 且 Native 正在 `creating/moving/resizing` 选区时，暂停向 Native 回写 `shapeCandidates`，降低 selection 每次变化导致的 runtime 更新风暴；3) 只有 Native 真正进入 `shape_moving/shape_start_moving/shape_end_moving/shape_resizing` 时，才把对象选中态同步回前端，避免在纯 selection 模式下误触发对象同步。
- 2026-03-17 20:26 继续修“切回选区后拖动选区句柄卡死”。基于最新日志，第二次卡死仍停在 `native_interaction_drag_started ... drag_mode=resizing ... mode=selection`，且没有 `drag_committed`。进一步收敛到 Native backend：拖拽过程中不应持续调用 `SetWindowRgn` 改输入区域。当前实现会在 selection create/move/resize 的每次 `WM_MOUSEMOVE` 中同步 window region，风险点高且在鼠标已 `SetCapture` 的前提下没有必要。
- 2026-03-17 21:42 再补一刀：NativeInteraction 拖拽进行中（`dragMode` 非空）时，前端不再调用 `updateNativeInteractionRuntime`。此前即便在初始选区创建阶段，前端也会随着 Native state event 持续回写 runtime，形成双向竞争。当前策略改为：拖拽期只接收 Native 状态事件，不向 Native 反向推送 runtime；拖拽结束后再恢复同步。
- 2026-03-17 21:55 最新 `log.log` 明确显示两类跨会话问题仍存在：
  1. 新 session 刚启动时，前端会发出旧 session 的 `updateNativeInteractionRuntime`，Rust 侧报 `NATIVE_INTERACTION_SESSION_MISMATCH`。
  2. 截图取消后，前端仍可能继续发 runtime 更新，Rust 侧报 `NATIVE_INTERACTION_SESSION_NOT_PREPARED`。
- 这说明当前真正需要先收口的是“session 切换期的 runtime update 闸门”，而不是继续盲改拖拽命中或 shape 创建逻辑。
- 2026-03-17 21:55 本轮修复策略：
  - 前端引入 `pendingSessionIdRef`，一旦收到 `session_updated_event`，旧 session 即不再允许向 NativeInteraction 回写 runtime。
  - 前端 runtime update effect 新增精确埋点：`runtime_update_sent/completed/failed/skipped`，日志带 `session_id / pending_session_id / active_session_id / tool / mode / candidates`。
  - Native state event 新增 `state_updated_applied/dropped` 埋点，直接暴露 session 过滤是否生效。
  - Rust `NativeInteractionService::update_runtime()` 对 `SESSION_NOT_PREPARED / SESSION_MISMATCH` 增加结构化日志，输出 `requested_session_id / active_session_id / visible / mode / lifecycle_state`，不再只靠模糊错误串排障。
- 2026-03-17 22:20 最新日志确认：第二次截图时 `native_toolbar_session_prepared` 存在，但没有 `native_toolbar_window_shown`。这说明问题不在 session 准备，而在“toolbar 可见化”链路。
- 当前 `NativeToolbarWindow` 的显示完全依赖前端 `updateNativeToolbarRuntime(visible=true)` 间接触发；`screenshot_service` 并不会主动 `show_prepared_session()`。这条设计本身依赖前端时序，二次截图时容易退回 WebView toolbar。
- 本轮针对 toolbar 增加和 NativeInteraction 对等的诊断能力：
  - 前端：`runtime_update_sent/completed/failed/skipped`
  - Rust command：`session_id / visible / active_tool / active_color`
  - Rust service：显式 `native_toolbar_window_shown/hidden trigger=runtime_update`
- 下一轮判断标准会非常明确：
  1. 若没有 `runtime_update_sent visible=true`，问题在前端 toolbar 可见性判定或 session gate。
  2. 若有 `runtime_update_sent visible=true` 但没有 `native_toolbar_window_shown trigger=runtime_update`，问题在 Rust service/backend。
  3. 若两者都有但界面仍是 WebView toolbar，问题在 Z-order 或前端 `nativeToolbarActive` 状态回写。
## 2026-03-17 Native Toolbar Runtime Failure Root Cause

- `native toolbar` 二次截图或切工具后回退到 WebView toolbar 的硬根因之一，不是前端没发 `visible=true`，而是前端日志里已经出现 `stroke_width=2.8568357680973286` 这类浮点值，而 Rust `update_native_toolbar_runtime` payload 把 `stroke_width` 定义成了 `u32`。
- 该错误发生在 Tauri 命令反序列化阶段，因此不会进入 `bexo::command::native_toolbar` 命令体，也不会产生 Rust 命令日志；前端只会拿到统一的 `命令执行失败`。
- 旧 WebView 主 toolbar 继续兜底会把 NativeToolbar 的失败掩盖掉，并导致截图运行时出现双 toolbar/回退错觉。截图运行时主 toolbar 需要彻底退出 WebView，只保留 NativeToolbarWindow。

- 2026-03-18：基于最新边界确认，`toolbar` 本身并不属于必须 Native 化的低延迟层；当前实现里 Native toolbar 只是“Win32 按钮 + 事件回调到 React 业务处理”，没有把复杂状态真正移出 WebView。
- 这意味着 Native toolbar 的收益远小于其复杂度：它额外引入了 session gate、show/hide 时序、跨窗口事件桥接、多轮截图可见性等故障面，却没有像 `NativePreviewWindow` / `NativeInteractionWindow` 那样直接解决底图首帧或高频命中问题。
- 正确边界应回到：Native 只承载 preview / selection / 高频 shape interaction / system input-focus-windowing；WebView 承载 toolbar、属性、对象操作和复杂配置 UI。
- 2026-03-18：实际回改后不需要保留 `NativeToolbarWindow` 作为“点击穿透保险”。现有 `NativeInteractionWindow` exclusion rect / input region 已足够支撑 WebView toolbar 与 text editor 点击。
- 2026-03-18：在物理删除 `src-tauri/src/commands/native_toolbar.rs`、`src-tauri/src/services/native_toolbar_service.rs`、`src-tauri/src/services/native_toolbar_backend_windows.rs` 后，`npm run web:build` 与 `cargo check` 仍通过，说明 toolbar Native 化没有留下隐藏主路径依赖。
- 2026-04-20：这轮“选区外左键立即冻结”又暴露出同一个前端结构问题：`handleStagePointerDown()` 里 `if (!selection || tool === "select")` 会吞掉后面的“点击选区外仅清理对象态”分支。
- 在该分支存在时，`tool=select` 且已有选区的外部点击，会在 `pointerdown` 当场进入“重画选区起点”状态，即使用户只是单击一下，也会让 `activeRect` 暂时切成 1x1 的新框起点。
- 正确修复不是简单禁止外部拖拽，而是把“外部单击”和“外部拖拽新框”拆开：先记录 pending origin，只有移动距离超过阈值时才真正设置 `dragStart/dragCurrent`。
- 这样既避免单击时进入错误的重画选区态，也保留 WebView 模式下从外部重新拖选区的能力；同时它和 Native selection 模式“已有选区时区域外不应立即切态”的语义是一致的。
- 2026-04-20：用户最新日志继续证明“只有选区、没有对象”时也会冻结，因此对象分支不是主因。最新卡点是：
  - `native_interaction_drag_committed ... drag_mode=creating`
  - 随后前端立即发出 `runtime_update_sent seq=2 ... exclusion_rects=1`
  - 这条 `seq=2` 没有完成日志，进程进入假死
- 这说明之前加的 selection cooldown 方向是对的，但实现存在 effect 时序竞争：冷却窗口通过 `setNativeSelectionRuntimeBlockedUntil(...)` 异步生效，而 `runtime_update` effect 在同一轮 effect flush 里仍能抢先读到旧值并把请求发出去。
- 正确修复是把 cooldown 闸门前移成同步 ref：拖拽从 `creating/moving/resizing -> none` 的那一轮 effect 中，先更新 `nativeSelectionRuntimeBlockedUntilRef.current`，再让 runtime update effect 直接读 ref，而不是读还没刷新的 state。
- 2026-04-20：在 cooldown 修复生效后，`runtime-logs/log.log` 已能看到 `selection_cooldown -> runtime_update_sent seq=3 -> runtime_update_completed seq=3`，说明“选区提交后第一条 runtime update 卡死”已被消掉。
- 但用户继续按“只拉选区然后点外部”复现时，项目日志没有新增未完成的 runtime update，系统侧 `Application Hang 1002 / AppHangB1` 反而记录了标准 UI 挂起。这说明新的冻结点更像 Win32 消息线程等待共享锁，而不是前端 effect 再次抢跑。
- 当前 `native_interaction_window_proc` 的 `WM_NCHITTEST / WM_SETCURSOR` 与鼠标处理函数之前都直接 `shared.lock()`；如果后台线程正在执行 `update_runtime / sync_window_input_region / present_interaction_surface` 并持有同一把锁，消息线程就会同步阻塞，最终被 WER 记成 `AppHangB1`。
- 这类路径的正确保护方式不是“继续延长 cooldown”，而是把消息线程上的锁策略改为 fail-open：`try_lock` 拿不到锁时直接返回 `HTTRANSPARENT` 或丢弃本次鼠标事件，优先保证窗口消息泵可返回，避免整进程假死。
- 2026-04-20：继续复盘代码后确认，上面的 `try_lock` 只改了消息入口还不够。`handle_left_button_down / handle_mouse_move / handle_left_button_up` 在释放状态锁后，仍会调用阻塞版 `present_interaction_surface()`，这依旧发生在 Win32 消息线程上。
- 因此真正需要收口到“消息线程全链路 fail-open”：
  - 入口 hit-test / cursor / mouse handlers 用 `try_lock`
  - handlers 尾部的 overlay 重绘也要改成 `try_present_interaction_surface`
  - 拿不到锁时允许丢一帧视觉更新，但不能阻塞窗口消息泵
- 2026-04-20：最新一次复现后，项目日志依旧停在 `runtime_update_completed seq=3`，没有新的 `state_lock_contended / present_skipped`，说明“选区外点一下就死”这次很可能根本没落进我们加日志的消息分支，而是在 `HTTRANSPARENT` 透传后卡在 overlay/native preview 的下层窗口栈。
- 当前实现里，`nativeBaseSelectionEligible` 为真时，WebView pointer 本来也不会处理这类点击；也就是说，把“选区外点击”继续透传到底层窗口没有实际收益，只会引入焦点/Z-order 不确定性。
- 更稳的兜底是：在 `NativeInteractionMode::Selection` 且已存在选区时，NativeInteractionWindow 直接吞掉选区外点击，保持 `toolbar/text editor` exclusion rect 仍然透传，其他区域全部由 native no-op 处理。
- 2026-04-20：用户最新回归显示外部点击已不再冻结，但 `Esc` 不能退出截图态；最新日志里也没有 `escape_cancel_completed`，说明不是“取消失败”，而是按键事件根本没进当前取消链路。
- 当前 overlay 的 `Esc` 只绑定在 WebView `window.keydown`，而在 native 选区模式下，鼠标操作后焦点并不可靠落回 WebView，因此这条路径不稳。
- Rust 侧其实已经有可用的 `cancel_active_session_from_escape()` 和截图事件总线，缺的是稳定的 `Esc` 输入来源。
- 最小正确修法不是把 `Esc` 直接在 Rust 里粗暴变成“强制关会话”，而是：
  - 会话级 hook 在 Rust 捕获 `Esc`
  - 发送 `screenshot://escape-pressed`
  - 前端复用现有分级 `Esc` 逻辑，优先取消文本编辑/拖拽/草稿/工具，再最后取消整个截图会话
- 2026-04-20：实际新日志进一步收窄到“旧的 WindowsHookHotkeyManager 版 `Esc` 方案仍不可靠”：
  - `escape_hook_applied` 已出现
  - `windows hook config applied binding_count=1` 已出现
  - 但按下 `Esc` 后仍然没有任何 `escape_hook_triggered`
- 这说明问题不在“会话没注册 `Esc`”，而在“当前这条 hook 触发源对裸 `Escape` 在截图态下并不稳定”。
- 当前更稳的路线应回到之前验证过的 `tauri-plugin-global-shortcut` 临时 `Escape` 主路径，但必须满足一个约束：
  - 回调里只异步发事件
  - 不在回调内同步注销 `Escape`
  - 避免重现历史 `shortcuts` 锁重入死锁
- 2026-04-20：用户新增需求要“彻底移除”设置页中的截图工具热键块；经全量检索确认，这不是单纯删一段 UI。
- 该能力完整链路包括：
  - `src/pages/settings-page.tsx` 的编辑/保存/恢复默认与去重逻辑
  - `src/pages/screenshot-overlay-page.tsx` 的运行时偏好读取
  - `src/lib/app-preferences.ts` / `src/types/backend.ts` 的默认值与类型
  - `src-tauri/src/domain/preferences.rs` / `src-tauri/src/services/preferences_service.rs` 的持久化、修复和冲突校验
- 如果只隐藏设置页区块，会留下 `hotkey.screenshotTools` 的前后端字段和 overlay 运行时依赖，属于未彻底移除。
- 当前最合理的收口方式是：删除“可配置化能力”，overlay 恢复固定工具热键；旧偏好中的 `screenshotTools` 交给 serde 当未知字段忽略，不需要额外迁移。

## 2026-04-21 Findings: Terminal command quote loss and PowerShell chain semantics
- 用户复现说明里包含两条独立信号：编辑框原本输入 `cd "D:\...\backend" && go run ./cmd`，保存后展示变成 `cd D:\...\backend && go run ./cmd`，说明保存链路可能做了破坏性命令解析或格式化。
- Windows Terminal 中的错误来自 PowerShell `Set-Location`，并且报 `&&` 不能作为参数，这说明当前启动语义等价于把 cmd 风格链式命令直接交给 PowerShell 处理。
- 本轮需要同时修“命令文本保真”和“Windows shell 兼容启动”；只修任意一边都会留下复现路径。
- 最终修复确认：Home 新保存的终端命令不再拆分成 argv，而是保存原始单行命令；Rust 可见终端启动切到 `cmd.exe /D /K`，restore 后台执行切到 `cmd.exe /D /C`，同时兼容旧的 `command + args` 记录。

## 2026-04-30 Codex History View
- CC Switch 的 Codex history 解析核心是读 `~/.codex/sessions/**/*.jsonl`，以 `session_meta.cwd` 判定项目归属，并把 `response_item` 中的 `message/function_call/function_call_output/reasoning` 转为展示消息。
- 本机 `~\.codex\sessions` 存在数百个 JSONL，且有 GB 级大文件；Bexo Studio 不能全量读取 session 文件。
- Bexo 的正确边界是 Rust 侧做路径校验、session 根解析、工作区范围过滤和分页读取；前端只拿已校验的只读数据。
- Workbench 工作区卡片的快捷动作位于 `src/components/shell/section-sidebar.tsx` 的 `WorkspaceSidebarCard`，新历史按钮应插入复制路径按钮之前。
- 现有 `src/app/app.tsx` 已通过 query 参数区分 screenshot overlay；Codex history 独立窗口可复用 `?window=codex-history&workspaceId=<id>` 分支，不需要新增主路由。

## 2026-04-30 Global Codex History Page
- 主导航并不是从 `primaryNavigation` 全量 map 渲染：`PrimaryRail` 之前只手动取 `home/settings`，所以即使新增 navigation item 也不会自动显示，必须同步改 rail 渲染逻辑。
- `/history` 需要独立 `AppRouteKey`，否则 `routeKeyFromPathname()` 会回落到 `home`，导致中间栏仍显示 WORKBENCH。
- 全局 History 不能复用 `list_codex_history_sessions(workspaceId)`；需要一个不按 workspace 过滤的后端列表命令。
- `get_codex_history_messages` 可以安全改成 `workspaceId` 可选：工作区窗口继续传 workspaceId 并做归属校验；全局页面不传 workspaceId，仅校验 sourcePath 位于已知 Codex sessions roots 内。
- Codex JSONL 解析后可直接用 `itemType` 区分展示意图：`message` 是用户/assistant 普通对话，`function_call`、`function_call_output`、`reasoning`、`web_search_call` 属于中间过程。仅折叠仍会占据阅读空间，因此默认应隐藏这些技术项，用户勾选后再显示。
- 用户角色里也会出现 Codex 注入的长规则块，例如 `# AGENTS.md instructions`、`<INSTRUCTIONS>`、`<environment_context>` 等；这些不是普通提问，应归入技术项默认隐藏，避免把结果阅读区撑开。
## 2026-05-01 Codex History Font Settings

- Codex history typography should live in `AppPreferences` rather than local component state because Settings, global History, and independent History windows already share the same preference query/cache path.
- The backend preference model uses serde defaults, so adding `codexHistory` is backward-compatible with existing `settings/preferences.json` files that do not yet contain the field.
- Empty `messageFontFamily` is the cleanest persisted representation for "system default"; storing a long CSS font stack as user data would make the setting harder to inspect and reset.

## 2026-05-04 Codex Auth Manager

- CC Switch 的 Codex 控制核心可以拆成两部分：`auth.json/config.toml` 的本地配置切换，以及 ChatGPT OAuth 登录/refresh token 托管。本轮用户明确只要前者和额度查询，登录托管不应混入。
- CC Switch 的额度查询路径是读取 `auth.json` 中 `auth_mode=chatgpt` 的 OAuth token，然后请求 `https://chatgpt.com/backend-api/wham/usage`；API key 模式不等价，应该返回不支持额度查询而不是伪造余额。
- Bexo 的边界必须保持 Rust owned：授权文件读写、TOML/JSON 校验、quota HTTP 请求和 rollback 都在 Rust 侧完成，前端只提交表单和展示结果。
- Fresh Codex installs may not yet have `config.toml`; importing such configs should save empty TOML and later switch 时创建空 `config.toml`，否则用户无法导入只有 `auth.json` 的真实环境。
- `reqwest 0.13.x` 的 TLS feature 是 `rustls`，不是 `rustls-tls`；新增依赖时已用 `cargo info reqwest@0.13.2` 校验。
## 2026-07-10 Windows 开机启动静默退出高可信根因

- 本机偏好 `launchAtLogin=true`、Run 键命令和 `StartupApproved` 启用值均正确，安装 exe 也存在。
- `Microsoft-Windows-Shell-Core/Operational` 事件 9707/9708 明确记录 Explorer 在登录后执行了 `bexo-studio.exe --autostart`；因此故障不是 Windows 未触发 Run，而是进程启动后未存活。
- 旧日志初始化把 `current_dir()/runtime-logs` 注册为必需写入目标；`tauri-plugin-log 2.8.0` 初始化 Folder target 时会执行 `create_dir_all` 并传播失败。该动作发生在应用 setup、托盘和启动日志之前，与“无应用日志、无窗口、无持久进程”的机器差异形成完整解释链。
- 证据边界：没有直接捕获已退出 PID 的 CWD 或原始插件错误，因此将其定级为高可信根因而非已动态复现的唯一根因；修复同时强化 Run 注册、校验与回滚，以覆盖独立的启动项可靠性风险。
- 如果不同 Windows 登录环境提供了不同的启动工作目录或写权限，旧实现即可表现为“新系统可启动、当前系统静默退出”；无论本机已退出进程当时的实际 CWD 为何，生产日志目标都不应依赖工作目录。
- Windows 自启动此前还存在插件与自有注册表代码双写、无条件重写 Run、主动创建 `StartupApproved`、无写后校验/事务回滚等可靠性风险，已一并收敛。

## 2026-07-10 全项目审查修复输入

- 已确认 2 个高风险：Native Preview/Interaction 通过 `usize` 绕过线程亲和；DB timeout 后 detached blocking write 仍可能提交。
- 已确认 9 个中风险：Hotkey health、Screenshot listener/结构、watch fallback、Tauri 权限、Codex Auth 事务/DTO、Preferences lost update、Restore cleanup、过期 example。
- 本轮不直接沿用旧结论作为完成证明；每项修复后必须从最终源码和验证结果重新关闭。
- 详细证据与实施记录：`scripts/work/2026-07-10-full-review-remediation/`。

## 2026-07-10 全量修复后二次 review 结论

- 原 2 个高风险、9 个中风险均已从最终源码关闭。
- 二次 review 额外发现并修复：DB read late-success、owner thread 缺 Win32 message pump、Auth mutation race/非原子替换、生产 CSP 混入 dev source、custom protocol 绕过窗口 ACL、两类拖拽排序部分提交、taskkill 无 deadline、overlay shortcut/数值容差边界。
- Workspace/LaunchTask 排序已改为 Rust 单事务批量命令；任一 UUID 重复、缺失或跨 project 时整体回滚。
- Tauri build manifest、invoke handler、main permission set 均为 64 项且测试锁定；Preview protocol 只接受 `screenshot_overlay` WebView。
- 本机 Run 值、安装 exe 与 StartupApproved `0x02` 只读复核正常；没有修改注册表。自启动代码修复重点仍是生产日志不依赖登录 CWD、安装路径保护、幂等写后读回与失败回滚。
- 未发现新的高/中严重度未修复逻辑错误。剩余限制是 Rust 测试宿主 `0xc0000139`、Native/Auth/下次登录需人工动态回归；Overlay 主编排文件和 Ant vendor chunk 属非阻断技术债。

## 2026-07-11 Production Runtime Style CSP Regression

- 用户截图中 Primary Rail、Tailwind panel 边界仍正常，退化集中在 Ant Input/Button/Empty/Icon 与 Sonner，排除了全局 WebView zoom 和本轮“成功提示静默”逻辑。
- 普通浏览器和 Tauri dev 的动态 stylesheet 正常；production WebView 中匹配 hash 的 `<style>` 文本存在，但全部 `sheet === null` 且无 nonce，证明是 CSP 激活阶段阻断而非 CSS cascade 或组件 style 覆盖。
- Tauri production 会为静态 style 注入随机 nonce；在 CSP 含 nonce 时，runtime CSS-in-JS 必须继承同页 nonce。正确修复是传递 nonce，而不是关闭 CSP 或继续堆叠覆盖样式。
- 修复后 HOME/Prompts 的 Ant runtime style 分别 43/43、30/30 激活，全部 head style 33/33 激活，Ant Icons 与 Sonner 也恢复；亮暗主题和 Prompt 编辑/校验 Toast 实机复核无阻断样式。
