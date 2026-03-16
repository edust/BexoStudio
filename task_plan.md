# task_plan

## 2026-03-15 Desktop Duplication 常驻最近帧原型
- 目标：用 `Desktop Duplication API` 替换当前启动期常驻 `WGC live capture`，实现“无黄边 + 接近实时”的单显示器最近帧缓存原型。
- 范围：
  - `src-tauri/src/services/desktop_duplication_capture.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/services/mod.rs`
  - `src-tauri/src/app/mod.rs`
  - 保持现有前端会话/预览协议不变，优先替换 Rust 侧 live capture 后端
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证启动无黄边、热键截图命中 live cache、Desktop Duplication 失败时仍由 one-shot 链路兜底
- 状态：进行中（2026-03-15）。

## 2026-03-14 截图加载黑屏移除
- 目标：修复截图热键后先出现黑屏再出底图的问题，尽量贴近“立即看到预览图”体验。
- 范围：
  - `src/pages/screenshot-overlay-page.tsx`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/tauri.conf.json`
  - 加载期不渲染黑色遮罩与工具条
  - overlay 窗口改透明，避免底图未就绪时黑底闪现
  - preview 生成优先走 uniform scale native 路径，减少逻辑缩放开销
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run`
  - `npm run web:build`
- 状态：已完成代码落地与编译验证（2026-03-14）。

## 2026-03-14 截图 overlay 几何锁定与无感对齐
- 目标：修复截图 overlay 可被移动、关闭时画面明显跳动的问题，保证截图态视觉 1:1 贴合屏幕。
- 范围：
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/app/window.rs`
  - 新增 overlay 几何探测日志（目标逻辑坐标 vs 当前逻辑/物理坐标）
  - 在 overlay `Moved/Resized/ScaleFactorChanged` 事件上自动回正窗口位置/尺寸
  - Windows 下追加 native 样式锁，移除可拖动/可调整边框样式
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run`
- 状态：已完成代码落地与编译验证（2026-03-14）。

## 2026-03-14 截图 preview 性能优化
- 目标：降低截图会话 `preview_image_ready` 的 `prepare_ms / encode_ms`，减少截图后等待时间。
- 范围：
  - `src-tauri/src/services/screenshot_service.rs`
  - 引入 preview 专用快速 PNG 编码路径（Fast + NoFilter）
  - 会话监视器原图缓存改为 `Arc<RgbaImage>`，避免 preview/crop 阶段的大块内存 clone
  - 记录更细的 preview 性能日志字段（编码路径、字节大小）
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run`
- 状态：已完成代码落地与编译验证（2026-03-14）。

## 2026-03-14 截图 preview DPI 坐标归一化修复
- 目标：修复高 DPI 场景下截图 overlay preview 可能异常放大的问题，统一截图会话 display 坐标语义。
- 范围：
  - `ScreenshotService` 新增 monitor 坐标标准化（raw display -> logical display）
  - 当 `screenshots/display_info.scale_factor` 不可靠时，优先使用 Tauri monitor `scale_factor`
  - 为 monitor/session 增加 DPI 判定诊断日志（raw/measured/reported/normalized）
  - 保持 overlay 窗口走 `Logical` 定位和尺寸 API，但输入改为明确 logical px
  - 增加坐标标准化单元测试
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run`
- 状态：已完成代码落地与编译验证（2026-03-14）。

## 2026-03-14 截图启动延迟与 DPI 修复
- 目标：解决截图热键触发后进入截图态过慢，以及高 DPI / 多屏缩放下截图显示与导出不准确的问题。
- 范围：
  - 为截图启动链路补结构化耗时日志
  - 将截图会话改为 overlay 先显示、图像异步准备
  - 重构截图数据模型，分离显示图与原始图，补 per-monitor DPI 元数据
  - 参考 `ParrotTranslator` 收敛交互体验
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证单屏/高 DPI/双屏混合缩放/连续触发截图热键
- 状态：已完成代码落地与编译验证，待本机手工回归（2026-03-14）。

## 2026-03-14 全局热键可靠性彻底修复
- 目标：解决 `Bexo Studio` 热键在 Windows 上“全局任意位置触发不灵敏 / 偶发失效”的系统性问题，而不是只修单个截图热键。
- 范围：
  - 重新梳理 Rust 侧全局快捷键与 Windows hook 双通道注册链路
  - 排查启动时机、生命周期、托盘常驻、窗口恢复、重复注册、回滚和消息线程问题
  - 明确哪些热键属于全局、哪些属于页面内，并修复容易造成“误以为热键失效”的路径
  - 补充更强的触发/注册日志与必要的自愈逻辑
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证应用前台/后台/托盘常驻/切到其他程序时的热键触发
- 状态：进行中（2026-03-14）。

## 2026-03-14 截图工具热键可配置化
- 目标：将 overlay 内固定 `1~5` 工具切换热键接入 Hotkeys 配置，并把截图全局默认热键切换为 `Ctrl+Shift+1`。
- 范围：
  - 前端设置页新增截图工具热键配置项（选区/线条/矩形/圆形/箭头）
  - overlay 改为读取偏好并按配置匹配工具切换热键
  - 偏好服务新增截图工具热键校验/规范化，修复 `1~5` 被误判格式无效
  - 老默认 `Ctrl+Shift+4` 迁移到 `Ctrl+Shift+1`
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证设置页保存、overlay 工具切换、恢复默认行为
- 状态：已完成代码落地与编译验证（2026-03-14）。

## 2026-03-14 Bexo Windows Hook 热键移植
- 目标：移植 `voiceType` 的 Windows hook 热键架构到 Bexo，先支持 `RAlt` 作为截图热键。
- 范围：
  - Rust 新增 hook 热键管理层
  - `HotkeyService` 改为 basic/global + advanced/hook 双通道
  - 热键校验与设置页录制器扩展到 side-specific modifier
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证 `RAlt` 与 `Ctrl+Shift+4`
- 状态：已完成代码接入（2026-03-14，Rust/前端编译验证通过；`RAlt` 与回归热键手工验证待本机确认）

## 2026-03-14 截图热键可靠性修复
- 目标：修复 `Ctrl+Alt+A` 作为默认截图热键时在 Windows 上触发不稳定的问题，并避免老用户继续保留历史默认值。
- 范围：
  - 调整截图热键默认值
  - 初始化时迁移历史默认 `Ctrl+Alt+A`
  - 设置页补充 `Ctrl+Alt` 风险提示
  - 热键服务补充触发与截图启动日志
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `cargo test --manifest-path src-tauri/Cargo.toml preferences_service`
  - `npm run web:build`
  - 手工验证默认值、迁移、提示和截图触发

## 2026-03-14 启动时 screenshot overlay 异常前置
- 目标：修复程序启动时 `screenshot_overlay` 被错误显示并抢占前台，避免用户在未触发截图热键时看到黑屏占位页。
- 范围：
  - 调整 `tauri-plugin-window-state` 对 `screenshot_overlay` 的处理策略
  - 为 overlay 页面补充“无会话即自隐藏”的防御性兜底
  - 同步根级 planning 文件与交付记录
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证冷启动不再弹出 overlay，截图热键仍能正常拉起会话
- 状态：已完成（2026-03-14，编译验证通过；冷启动与截图热键手工验证待本机确认）

## 2026-03-11 资源浏览器在这里打开终端
- 目标：在资源浏览器右键菜单增加“在这里打开终端”，并在当前目录打开 wt。
- 方案：新增 Rust 命令 `open_workspace_terminal_at_path`，前端菜单调用该命令。
- 验证：前端构建 + Rust check + 手工路径场景验证。

## 2026-03-11 热键截图与标注（方案）
- 新增方案蓝图：scripts/work/2026-03-11-hotkey-screenshot-annotate/task_plan.md
- 目标：对标微信截图交互，分阶段交付 MVP -> 标注 -> 打磨。

## 2026-03-11 Phase A 热键设置 + 截图热键生效
- 目标：先交付 `Settings > Hotkeys` 中的截图热键配置，并让全局热键即时生效。
- 范围：
  - 偏好模型新增 `hotkey`（含 `screenshotCapture` 和语音热键预留字段）
  - Rust 新增热键服务，启动注册与更新重载
  - 前端设置页新增 Hotkeys 子页与录制交互
  - 前端监听 `hotkey://trigger` 事件做当前阶段反馈
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证默认热键、修改后热键生效、错误场景提示

## 2026-03-12 Phase B 热键触发截图会话（overlay 选区 + 复制/保存）
- 目标：从全局截图热键直达截图 overlay，会话内完成选区、复制、保存、取消。
- 范围：
  - Rust 新增截图会话服务与命令
  - 新增 overlay 窗口打开流程与会话关闭清理
  - 前端新增 screenshot overlay 路由与页面交互
  - capability 放开 overlay 窗口权限
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证热键触发、复制、保存、取消
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase C 标注工具栏 + 撤销重做
- 目标：在截图 overlay 内新增微信式标注能力，并确保复制/保存输出包含标注结果。
- 范围：
  - 前端新增标注工具（线/框/圆/箭头/文字/画笔/填色）
  - 新增撤销/重做
  - 复制/保存导出带标注位图
- 验证：
  - `npm run web:build`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - 手工验证工具行为与导出结果
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D 多屏无缝 + DPI/性能/快捷键打磨
- 目标：实现多屏截图会话、修正 DPI 映射精度并打磨交互快捷键与渲染性能。
- 范围：
  - Rust 会话改为虚拟桌面多屏拼接
  - 选区映射改双轴比例
  - 前端工具快捷键、线宽快捷键、Esc 分级处理
  - 画笔采样阈值优化
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工多屏/高 DPI/快捷键回归
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.2 文本工具原位输入
- 目标：将截图文字工具从 `window.prompt` 升级为原位输入框，支持更自然的输入、提交与取消体验。
- 范围：
  - overlay 内新增文本编辑态与原位输入框
  - 支持 IME 组合输入，避免中文输入时误提交
  - `Enter` 提交、`Esc` 取消、失焦自动提交
  - 与现有撤销/重做、复制/保存导出链路兼容
- 验证：
  - `npm run web:build`
  - 手工验证文本点击输入、中文输入法、Enter/Esc、失焦提交、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.3 文本工具对象编辑打磨
- 目标：让文字标注具备对象级编辑能力，支持拖动重定位、双击编辑已有文字，以及字号/颜色即时预览。
- 范围：
  - 文字标注支持命中、选中和拖动预览
  - 双击已有文字进入原位编辑
  - 选中文字后颜色/字号调整即时反映到画面
  - 保持撤销/重做、复制/保存导出一致
- 验证：
  - `npm run web:build`
  - 手工验证文字拖动、双击编辑、颜色字号即时预览、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.4 文本样式与键盘微调
- 目标：继续完善文字标注，新增描边/背景/高亮样式，以及方向键微调和 Delete 删除。
- 范围：
  - 文字模型新增样式字段
  - 编辑态与选中态支持描边/背景/高亮即时预览
  - 方向键微调位置，`Shift+方向键` 快速移动
  - `Delete` 删除当前选中文字
  - 复制/保存导出与样式表现保持一致
- 验证：
  - `npm run web:build`
  - 手工验证文字样式、方向键微调、Delete 删除、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.5 文字旋转/透明度/层级/多选
- 目标：把文字标注升级为可批量操作对象，支持旋转、透明度、层级前后移和多对象选择。
- 范围：
  - 文字对象新增 `rotation / opacity`
  - `Ctrl/Cmd+点击` 多选文字，`Ctrl/Cmd+A` 全选当前截图中的文字对象
  - 多选对象支持分组拖动、方向键整体微调、Delete 批量删除
  - 工具栏新增旋转/透明度输入与 `前移 / 后移` 层级按钮
  - SVG 预览、命中测试、选中框与 Canvas 导出统一支持旋转/透明度
- 验证：
  - `npm run web:build`
  - 手工验证多选、分组拖动、旋转、透明度、层级调整、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.6 文字复制/粘贴/重复/对齐吸附
- 目标：继续补齐微信式文字标注效率能力，支持对象复制、粘贴、重复以及拖拽时对齐吸附。
- 范围：
  - 当前截图会话内新增文字对象内部剪贴板
  - `Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D` 对文字对象生效
  - 工具栏补充复制/粘贴/重复入口
  - 文字分组拖动时支持对齐吸附与辅助线
  - 保持撤销/重做、导出和多选链路一致
- 验证：
  - `npm run web:build`
  - 手工验证复制、粘贴、重复、拖拽吸附、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.7 马赛克/模糊
- 目标：补齐截图标注中的隐私遮挡能力，支持马赛克与模糊区域。
- 范围：
  - 新增 `马赛克`、`模糊` 两个标注工具
  - 预览层支持效果实时显示
  - 导出链路支持效果写入最终 PNG
  - 保持撤销/重做、快捷键和选区边界约束一致
- 验证：
  - `npm run web:build`
  - 手工验证马赛克、模糊、导出一致性、撤销重做
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.8 编号标注
- 目标：补齐微信式截图中的编号标注能力，支持快速连续落点和导出。
- 范围：
  - 新增 `编号` 工具
  - 点击选区内位置即可落一个递增编号
  - 编号颜色跟随当前颜色，尺寸复用当前字号设定
  - SVG 预览与 PNG 导出保持一致
- 验证：
  - `npm run web:build`
  - 手工验证编号递增、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.9 马赛克/模糊对象二次编辑
- 目标：让马赛克/模糊区域具备对象级命中、选中、删除和强度二次编辑能力。
- 范围：
  - effect annotation 支持命中测试与单对象选中
  - 选中后提供可视化边框反馈
  - `Delete / Backspace` 删除当前选中的 effect 对象
  - 工具栏马赛克/模糊强度输入可对选中对象做二次编辑；未选中时仍作为新建默认值
  - 保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证命中选中、Delete 删除、强度调整、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.10 马赛克/模糊对象拖动与缩放
- 目标：让 effect 对象支持拖动重定位和边缘拖拽缩放，进一步接近微信截图体验。
- 范围：
  - effect annotation 新增拖动预览态与缩放预览态
  - 选中 effect 后可拖动整个区域重定位
  - 选中 effect 后可通过边缘/角点句柄缩放区域
  - 松手后一次性提交历史，保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证拖动、四边/四角缩放、边界约束、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.11 编号对象命中/删除/拖动
- 目标：让编号对象具备对象级命中、Delete 删除和拖动重定位能力。
- 范围：
  - number annotation 支持命中测试与单对象选中
  - 选中编号后提供可视化边框反馈
  - `Delete / Backspace` 删除当前选中的编号对象
  - 拖动编号对象时做预览，mouseup 后一次性提交历史
  - 保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证命中选中、Delete 删除、拖动、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.12 编号对象颜色/字号二次编辑
- 目标：让选中的编号对象支持颜色和字号二次编辑。
- 范围：
  - 选中编号后，颜色按钮可直接修改当前编号颜色
  - 选中编号后，字号输入可直接修改当前编号大小
  - 工具栏颜色/字号在选中编号时同步显示当前对象值
  - 保持撤销/重做、边界约束、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证改色、改字号、边界回退、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.13 编号对象复制/重复
- 目标：让编号对象支持对象级复制、粘贴和重复，继续补齐截图 overlay 的高频编辑能力。
- 范围：
  - 当前截图会话内新增编号对象内部剪贴板
  - `Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D` 对选中编号生效
  - 工具栏补充编号对象的复制/粘贴/重复入口
  - 新对象沿用偏移 + 边界回退策略，并保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证编号复制、粘贴、重复、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.14 编号对象层级前后移
- 目标：让编号对象支持对象级层级前后移，补齐和文字对象一致的基础排序能力。
- 范围：
  - 选中编号后支持 `前移 / 后移`
  - `Ctrl/Cmd+[`、`Ctrl/Cmd+]` 在选中编号时生效
  - 排序继续基于全量 annotation 顺序，保持预览、命中和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证编号跨对象前移/后移、快捷键、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.15 effect 对象复制/重复
- 目标：让马赛克/模糊 effect 对象支持对象级复制、粘贴和重复，继续补齐非文本对象编辑能力。
- 范围：
  - 当前截图会话内新增 effect 对象内部剪贴板
  - `Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D` 对选中 effect 生效
  - 工具栏补充 effect 对象的复制/粘贴/重复入口
  - 新对象沿用偏移 + 选区边界回退策略，并保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证 effect 复制、粘贴、重复、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.16 effect 对象层级前后移
- 目标：让马赛克/模糊 effect 对象支持对象级层级前后移，补齐和文字/编号对象一致的基础排序能力。
- 范围：
  - 选中 effect 后支持 `前移 / 后移`
  - `Ctrl/Cmd+[`、`Ctrl/Cmd+]` 在选中 effect 时生效
  - 排序继续基于全量 annotation 顺序，保持预览、命中和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证 effect 跨对象前移/后移、快捷键、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.17 effect 对象方向键微调
- 目标：让马赛克/模糊 effect 对象支持方向键微调，补齐对象级键盘编辑能力。
- 范围：
  - 选中 effect 后支持方向键微调位置
  - `Shift+方向键` 执行更大步长移动
  - 位移继续受截图选区边界约束，并保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证 effect 方向键微调、Shift 快速移动、边界约束、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.18 编号对象方向键微调
- 目标：让编号对象支持方向键微调，补齐对象级键盘编辑能力。
- 范围：
  - 选中编号后支持方向键微调位置
  - `Shift+方向键` 执行更大步长移动
  - 位移继续受截图选区边界约束，并保持撤销/重做、预览和导出一致
- 验证：
  - `npm run web:build`
  - 手工验证编号方向键微调、Shift 快速移动、边界约束、撤销重做、复制保存
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.19 编号选中态快捷提示可视强化
- 目标：增强编号对象选中态的可视提示，让层级快捷入口在对象附近直接可见。
- 范围：
  - 编号选中态 overlay 增加快捷操作提示
  - 提示覆盖 `Ctrl/Cmd+[ / ]` 前后移层级和方向键微调
  - 不改变现有交互逻辑、导出链路和命中模型
- 验证：
  - `npm run web:build`
  - 手工验证编号选中提示显示位置、快捷提示文案、拖动/导出不受影响
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.20 effect 选中态快捷提示可视强化
- 目标：增强 effect 对象选中态的可视提示，让方向键微调和层级快捷入口在对象附近直接可见。
- 范围：
  - effect 选中态 overlay 增加快捷操作提示
  - 提示覆盖方向键微调和 `Ctrl/Cmd+[ / ]` 前后移层级
  - 不改变现有交互逻辑、导出链路和命中模型
- 验证：
  - `npm run web:build`
  - 手工验证 effect 选中提示显示位置、快捷提示文案、拖动/导出不受影响
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.21 编号/effect 多选基础能力
- 目标：为编号和 effect 对象补齐基础多选，先覆盖批量删除、层级调整和方向键微调。
- 范围：
  - `Ctrl/Cmd+点击` 可对编号和 effect 做增量多选/取消多选
  - 多选编号和 effect 支持批量 `Delete/Backspace`、`Ctrl/Cmd+[ / ]` 和方向键微调
  - overlay 为多选编号/effect 渲染组选中装饰，主选中对象保留就地快捷提示
  - 复制/重复继续限定为单个编号或单个 effect，多选时给出明确提示
- 验证：
  - `npm run web:build`
  - 手工验证编号/effect 多选、批量删除、批量层级前后移、批量方向键微调、单对象复制/重复限制提示
- 状态：已完成（2026-03-12）

## 2026-03-12 Phase D.22 编号/effect 分组拖拽
- 目标：让多选后的编号和 effect 支持整体拖拽移动，补齐多选后的基础鼠标编辑能力。
- 范围：
  - 多选编号时，普通点击组内任一对象可整体拖拽移动
  - 多选 effect 时，普通点击组内任一对象可整体拖拽移动
  - 组拖拽继续受截图选区边界约束，保留相对位置，`pointerup` 一次性提交历史
  - 多选 effect 时暂不支持分组缩放，句柄缩放继续限定单对象
- 验证：
  - `npm run web:build`
  - 手工验证编号/effect 分组拖拽、边界约束、撤销重做以及单对象缩放行为不回退
- 状态：已完成（2026-03-12）

## 2026-03-13 Phase D.23 编号/effect 分组复制与重复
- 目标：让编号和 effect 在多选后支持整组复制、粘贴和重复，补齐对象级批处理编辑能力。
- 范围：
  - 编号 clipboard 从单对象升级为分组对象，支持多选复制/粘贴/重复
  - effect clipboard 从单对象升级为分组对象，支持多选复制/粘贴/重复
  - 粘贴/重复继续沿用统一的偏移与选区边界约束，保持组内相对位置不变
  - 工具栏、快捷键和状态文案与分组复制能力保持一致
- 验证：
  - `npm run web:build`
  - 手工验证编号/effect 多选复制、粘贴、重复、撤销重做和导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.24 编号/effect 框选多选
- 目标：为编号和 effect 增加框选多选，补齐鼠标批量选择入口。
- 范围：
  - 在截图选区内空白区域拖框，可框选编号或 effect
  - 框内同时命中编号与 effect 时，优先沿当前已选对象家族；没有当前家族时按最上层命中对象决定家族
  - `Ctrl/Cmd+拖框` 支持增量框选；普通拖框为替换选择
  - 不改变现有截图选区重绘逻辑；截图选区外拖拽仍用于重选截图区域
- 验证：
  - `npm run web:build`
  - 手工验证编号/effect 框选、增量框选、混合命中家族决策、截图选区重绘不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.25 effect/number 置顶与置底入口
- 目标：为 effect 和编号对象补齐“置顶/置底”层级入口，缩短复杂标注场景下的层级调整路径。
- 范围：
  - 为当前选中的编号/effect 增加“置顶 / 置底”工具栏入口
  - 增加对应快捷键入口，沿用现有层级快捷键体系
  - 保持多选对象组内相对顺序不变
  - 不引入新的 z-index 模型，继续基于 annotations 顺序实现
- 验证：
  - `npm run web:build`
  - 手工验证编号/effect 单选和多选的置顶/置底、与前移/后移协同、撤销重做和导出顺序一致
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.26 文字对象置顶/置底快捷入口对齐
- 目标：将文字对象的就地快捷提示补齐到与编号/effect 一致的层级能力表达，避免“功能已支持但对象层入口缺失”。
- 范围：
  - 为文字选中 overlay 增加就地快捷提示
  - 补齐 `Ctrl/Cmd+[ / ]` 与 `Ctrl/Cmd+Shift+[ / ]` 提示
  - 多选文字时同步显示整组拖动/复制语义
  - 不改动既有层级重排逻辑，只补对象级可见入口
- 验证：
  - `npm run web:build`
  - 手工验证单选/多选文字的就地提示、快捷键行为和导出不受影响
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.27 图形对象选中与二次编辑
- 目标：为线条 / 箭头 / 矩形 / 圆形补齐对象级编辑闭环，摆脱“只能新建、不能回头改”的一次性标注模式。
- 范围：
  - 新增图形对象命中、选中态与可视化选中框
  - 支持图形对象拖动与句柄缩放/端点编辑
  - 支持选中后颜色、线宽、Delete、方向键微调、层级调整
  - 复制/重复与多选暂不纳入本阶段，先完成单对象编辑闭环
- 验证：
  - `npm run web:build`
  - 手工验证四类图形命中、拖动、缩放、改颜色/线宽、删除、层级和导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.28 画笔对象选中与二次编辑
- 目标：为画笔路径补齐对象级编辑闭环，避免自由绘制内容只能删除重画。
- 范围：
  - 新增画笔对象命中、选中态与局部提示
  - 支持整条路径拖动重定位
  - 支持选中后颜色、线宽、Delete、方向键微调、层级调整
  - 暂不做画笔路径缩放和点级编辑，先完成稳定的单对象编辑闭环
- 验证：
  - `npm run web:build`
  - 手工验证画笔命中、拖动、改颜色/线宽、删除、层级和导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.29 画笔对象复制与重复
- 目标：为画笔对象补齐复制、粘贴和重复，避免自由绘制内容只能重画一遍。
- 范围：
  - 新增画笔 clipboard 状态并接入对象级分发
  - 支持 `Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D` 作用于当前选中画笔
  - 工具栏、状态文案和就地提示同步补齐画笔复制语义
  - 不引入画笔多选，当前仍保持单对象复制/重复
- 验证：
  - `npm run web:build`
  - 手工验证画笔复制、粘贴、重复、层级与导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.30 画笔对象多选基础能力
- 目标：让画笔对象具备基础批量编辑能力，避免多条自由绘制内容只能一条条处理。
- 范围：
  - 为画笔补充“主选中 id + 选中 id 列表”模型
  - 支持 `Ctrl/Cmd+点击` 画笔增量多选/取消多选
  - 支持多选画笔的批量删除、层级前后移/置顶置底、方向键微调
  - 本阶段不做画笔分组拖拽，也不放开多选组复制/重复
- 验证：
  - `npm run web:build`
  - 手工验证画笔多选、批量删除、层级、方向键微调与导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.31 画笔对象分组拖拽
- 目标：让多选画笔支持整组拖动，补齐和编号/effect 一致的批量位移体验。
- 范围：
  - 为画笔补充分组拖拽预览状态
  - 多选画笔时普通点击组内任一画笔进入整组拖拽
  - `pointermove` 只更新预览，`pointerup` 一次性提交历史
  - 本阶段不做画笔分组缩放，也不放开多选组复制/重复
- 验证：
  - `npm run web:build`
  - 手工验证画笔多选整组拖拽、边界约束、撤销重做与导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.32 画笔多选组复制与重复
- 目标：让多选画笔支持整组复制、粘贴和重复，补齐与编号/effect 对齐的批处理能力。
- 范围：
  - 放开画笔多选下的复制/重复限制
  - 复用现有画笔 clipboard 结构和统一粘贴偏移规则
  - 粘贴/重复后自动选中新组画笔，并保持组内相对位置
  - 同步状态文案、工具栏和就地提示到分组复制语义
- 验证：
  - `npm run web:build`
  - 手工验证画笔多选复制、粘贴、重复、撤销重做与导出一致性
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.33 画笔对象框选多选
- 目标：让画笔对象接入现有框选多选入口，减少大量自由路径场景下的逐个点选成本。
- 范围：
  - 将画笔并入 `objectSelectionMarquee` 的家族决策
  - 在截图选区内空白拖框时支持框选画笔
  - `Ctrl/Cmd+拖框` 支持画笔增量框选
  - 不引入跨家族混选，继续遵守单家族选择规则
- 验证：
  - `npm run web:build`
  - 手工验证画笔框选、增量框选、家族决策和后续批量操作不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.34 图形对象复制与重复
- 目标：让线条、箭头、矩形、圆形补齐对象级复制、粘贴、重复，和其他对象家族对齐。
- 范围：
  - 为图形新增对象级 clipboard
  - 接入 `Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D`
  - 补工具栏 `复制图形 / 粘贴图形 / 重复图形`
  - 同步状态文案、就地提示和顶部帮助到图形复制语义
- 验证：
  - `npm run web:build`
  - 手工验证图形复制、粘贴、重复、层级/拖动/缩放不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.35 图形对象多选基础能力
- 目标：让图形对象补齐基础多选，降低多图形批量删改和调层级的操作成本。
- 范围：
  - 为图形新增 `主选中 + 选中列表` 选择模型
  - 接入 `Ctrl/Cmd+点击` 增量多选/取消多选
  - 支持多选图形的批量删除、层级调整、方向键微调
  - 同步状态文案、帮助提示和 overlay 到多选语义
  - 本阶段不做图形分组拖拽，也不放开图形分组复制/重复
- 验证：
  - `npm run web:build`
  - 手工验证图形多选、批量删除、层级、方向键微调与现有单图形编辑不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.36 图形对象分组拖拽
- 目标：让多选图形在保持单图形句柄编辑不回退的前提下，补齐整组平移能力。
- 范围：
  - 为图形新增分组拖拽状态与预览链路
  - 多选图形时普通点击组内对象进入整组拖拽
  - `pointermove` 只更新预览，`pointerup` 一次性提交历史
  - 继续受截图选区边界约束
  - 本阶段不做图形分组缩放，也不扩展图形框选
- 验证：
  - `npm run web:build`
  - 手工验证图形分组拖拽、边界约束、撤销重做与单图形句柄编辑不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.37 图形对象多选组复制与重复
- 目标：让多选图形补齐整组复制、粘贴、重复，和画笔/编号/effect 的对象组编辑体验对齐。
- 范围：
  - 放开图形 clipboard 的多选写入
  - 放开多选图形的 `Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D`
  - 粘贴/重复后自动整组选中新组图形
  - 同步状态文案、顶部帮助、overlay 提示和工具栏按钮到组复制语义
- 验证：
  - `npm run web:build`
  - 手工验证图形组复制、组重复、组粘贴、后续拖拽/层级/样式编辑不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.38 图形对象接入统一框选入口
- 目标：让图形对象接入现有对象框选手势，和画笔/编号/effect 保持同一套空白拖框多选入口。
- 范围：
  - 将图形并入 `objectSelectionMarquee` 启动条件和家族决策
  - 为图形补充框选命中逻辑
  - 在 `pointerup` 中新增图形家族的框选提交分支
  - 顶部帮助文案同步到图形框选语义
  - 继续保持单家族框选，不放开跨家族混选
- 验证：
  - `npm run web:build`
  - 手工验证图形框选、增量框选、家族优先和后续批量操作不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.39 跨对象家族统一框选可视强化
- 目标：让图形、画笔、编号、效果共用更明确的框选预览反馈，降低单家族规则下的误解成本。
- 范围：
  - 为对象框选新增实时家族/数量预览
  - 为当前命中的对象渲染统一的预高亮装饰
  - 在框选提示中展示追加模式与其他命中家族摘要
  - 同步顶部/工具栏状态文案到框选预览语义
  - 不改变单家族框选规则，不放开跨家族混选
- 验证：
  - `npm run web:build`
  - 手工验证图形/画笔/编号/effect 框选预览、追加框选提示和最终选中结果一致
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.40 文字对象接入统一对象框选入口
- 目标：让文字对象进入和图形/画笔/编号/effect 相同的空白拖框多选入口，补齐对象家族的一致性。
- 范围：
  - 将文字并入 `objectSelectionMarquee` 的启动条件、家族优先和提交分支
  - 为文字补充框选命中逻辑，优先复用现有文字布局几何
  - 将文字并入统一框选预览和提示文案
  - 保持单家族框选，不放开跨家族混选
- 验证：
  - `npm run web:build`
  - 手工验证文字框选、增量框选、旋转文字命中和后续批量编辑不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.41 跨对象家族统一组选中框与主对象提示收敛
- 目标：收敛文字/图形/画笔/编号/effect 的多选组选中框与主对象提示样式，降低当前各家族提示分散、重复的问题。
- 范围：
  - 为多选对象家族新增统一的组选中框与组级提示层
  - 将各家族主对象提示气泡收敛到同一渲染组件
  - 保留现有句柄/单对象轮廓，不改已有编辑语义
  - 不放开跨家族混选，仅优化可视与提示一致性
- 验证：
  - `npm run web:build`
  - 手工验证各家族多选组选中框、单对象主提示和后续拖拽/层级/复制能力不回退
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.42 统一批处理状态栏收敛
- 目标：将当前工具栏中的家族分支长文案收敛为统一的批处理状态栏模型，为后续跨家族混选做准备。
- 范围：
  - 抽出统一状态栏数据模型（标题、摘要、能力标签）
  - 覆盖框选预览、单对象选中、多对象选中和空闲态
  - 在不改变现有选择语义的前提下，统一文字/图形/画笔/编号/effect 的状态栏表达
  - 不放开跨家族混选，仅先收敛当前单家族状态表达
- 验证：
  - `npm run web:build`
  - 手工验证各家族单选/多选/框选预览下的状态栏信息与可执行动作一致
- 状态：已完成（2026-03-13）

## 2026-03-13 Phase D.43 跨家族混选基础版
- 目标：放开文字/图形/画笔/编号/effect 的基础跨家族混选，让用户能先建立混合选择并执行最核心的公共批处理动作。
- 范围：
  - 放开 `Ctrl/Cmd+点击` 的跨家族叠加选择
  - 为混选补统一组选中框与状态栏表达
  - 打通混选下的删除、层级调整、全选等公共动作
  - 避免混选状态下误触发只对单家族生效的旧快捷键路径
  - 本阶段不做跨家族组拖拽、跨家族复制/重复、跨家族方向键微调
- 验证：
  - `npm run web:build`
  - 手工验证跨家族点击混选、删除、层级、全选和状态栏/组选中框表现一致
- 状态：已完成（2026-03-13）


## 2026-03-13 Phase D.44 跨家族混选分组拖拽
- 目标：让跨家族混选对象进入统一整组拖拽，补上 mixed selection 的核心空间操作能力。
- 范围：
  - 为 mixed selection 新增统一 group drag 状态与预览链路
  - 点击 mixed 选中对象时进入整组平移，不回退到单家族选择
  - 打通 pointermove 预览、pointerup 提交、Esc 取消和边界约束
  - 同步更新混选组选中框、状态栏和帮助文案到“可整组拖动”语义
  - 本阶段不做跨家族缩放、复制/重复、方向键微调
- 验证：
  - `npm run web:build`
  - 手工验证跨家族混选整组拖拽、边界约束、撤销重做和单家族句柄编辑不回退
- 状态：已完成（2026-03-13）



## 2026-03-13 Phase D.45 跨家族混选复制重复粘贴
- 目标：让跨家族 mixed selection 进入统一 clipboard 闭环，补上复制、重复、粘贴能力。
- 范围：
  - 新增 mixed clipboard 状态，支持跨家族对象组复制
  - 打通 mixed selection 下的 `Ctrl/Cmd+C`、`Ctrl/Cmd+D` 和 `Ctrl/Cmd+V`
  - 粘贴/重复后恢复为新的 mixed selection，保持组内相对位置
  - 同步更新状态栏、提示文案和工具栏入口
  - 本阶段不做跨家族方向键微调与跨家族组缩放
- 验证：
  - `npm run web:build`
  - 手工验证跨家族复制、重复、粘贴、撤销重做和后续整组拖拽/层级不回退
- 状态：已完成（2026-03-13）

## 2026-03-14 截图 Overlay 启动黑屏移除
- 目标：去掉进入截图流程前 1s 左右黑屏过渡，改为“窗口可见但画面无感”的透明预热。
- 范围：
  - 在 `index.html` 启动早期根据 `overlay=screenshot` 打标记，避免等待 React 挂载后才切样式。
  - 在 `globals.css` 对截图 overlay 强制 `html/body/#root/.ant-app` 透明背景，防止全局主题背景透出。
  - 在 `screenshot-overlay-page.tsx` 移除无会话中心提示，避免启动态出现额外视觉层。
- 验证：
  - `npm run web:build`
  - 手工验证按截图热键后，不再先看到整屏黑底，再出现截图底图。
- 状态：已完成（2026-03-14）

## 2026-03-14 截图单阶段进入（取消可见过渡界面）
- 目标：按截图热键后不再显示“第一阶段过渡界面”，仅在可操作时直接进入截图工作界面。
- 范围：
  - `start_session` 改为立即显示工作态 overlay（遮罩与工具区先展示，底图异步到位）。
  - `finish_preview_preparation` 仅负责会话图像状态更新，不再控制窗口显示时机。
  - 保留 overlay 几何/样式锁，避免显示瞬间边框/位移抖动。
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证按热键后不再看到第一阶段界面，仅直接进入工作界面。
- 状态：已完成代码落地与编译验证（2026-03-14）

## 2026-03-14 截图首帧延迟链路诊断日志
- 目标：定位“按下热键到预览底图出现 4~5 秒”的真实耗时分布，补齐毫秒级全链路日志。
- 范围：
  - Rust：`hotkey -> start_session -> capture_virtual_desktop -> preview_prepare -> finish_preview_preparation -> get_screenshot_session`
  - 前端：`session_updated_event -> getScreenshotSession invoke -> base64 image decode -> canvas first paint`
  - 所有阶段日志统一输出毫秒单位，并携带 `session_id` 与关键 payload 大小（如 `image_data_url_bytes`）
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工触发截图，核对日志链路是否完整、是否可对齐到同一 `session_id`
- 状态：已完成代码落地与编译验证（2026-03-14）

## 2026-03-14 截图预览文件链路加速（去 dataUrl 大包传输）
- 目标：将截图预览从超大 base64 dataUrl IPC 传输改为临时文件路径，降低首帧延迟并消除半透明等待阶段。
- 范围：
  - 后端 `ScreenshotSessionView` 增加 `preview_image_path`。
  - 预览生成链路改为：合成图 -> 快速编码（BMP 优先）-> 写入 temp 文件 -> 会话下发路径。
  - 前端 overlay 优先使用 `convertFileSrc(previewImagePath)` 加载预览，`imageDataUrl` 仅保留兼容 fallback。
  - 会话替换/取消/完成时清理旧预览临时文件，避免堆积。
  - 单屏快路径优化：减少不必要的整图拼接拷贝。
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
- 状态：已完成代码落地与编译验证（2026-03-14）

## 2026-03-14 截图预览自定义协议修复
- 目标：修复“预览文件已生成但 overlay 不显示底图”的问题，绕开 `asset/convertFileSrc` 权限链。
- 范围：
  - 在 `src-tauri/src/app/mod.rs` 注册只读 `bexo-preview` 协议，限制只读取 temp 截图预览目录内文件。
  - 前端 overlay 不再用 `convertFileSrc`，改为直接构造 `bexo-preview` URL。
  - 补充协议成功/拒绝/缺失日志，便于下一轮诊断。
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
- 状态：已完成代码落地与编译验证（2026-03-14）

## 2026-03-14 截图首帧继续提速（逻辑尺寸预览 + 前端首帧日志）
- 目标：继续压缩“按热键到底图可见”的延迟，重点收敛 4K 预览编码与前端首帧不可观测问题。
- 范围：
  - `src-tauri/src/services/screenshot_service.rs`
  - `src/pages/screenshot-overlay-page.tsx`
  - overlay 预览优先输出逻辑显示尺寸，避免高 DPI 场景继续编码 3840x2160 级预览文件
  - 前端 decode / first-paint 日志写入 Tauri log，而不是只打 `console`
  - 初始态无 effect 预览时不再无条件整屏 canvas 重绘
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工触发截图，对比 `start_session_completed / preview_image_ready / preview_image_decode_done / preview_canvas_first_paint`

## 2026-03-14 截图首帧直显快路径（对齐 ParrotTranslator）
- 目标：让单屏截图进入编辑界面时不再经历 `resize -> encode -> file -> img decode`，而是像 ParrotTranslator 一样“抓完即显”。
- 范围：
  - `src-tauri/src/domain/screenshot.rs`
  - `src-tauri/src/commands/screenshot.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src/lib/command-client.ts`
  - `src/types/backend.ts`
  - `src/pages/screenshot-overlay-page.tsx`
  - 单屏会话新增 `raw_rgba_fast` 预览传输模式
  - `image_status` 在快路径下立即进入 `ready`
  - 新增 `get_screenshot_preview_rgba` 二进制命令，前端直接拉原始像素并画到 base canvas
  - 多屏/非快路径继续保留现有文件预览 fallback
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工触发截图，对比 `capture_ms / start_session_completed / get_screenshot_preview_rgba / preview_raw_first_paint`

## 2026-03-14 截图首帧继续压缩（协议直供首屏预览）
- 目标：把单屏 `raw_rgba_fast` 的剩余瓶颈从 “33MB RGBA 通过 Tauri invoke 进入 JS” 改为 “Rust custom protocol 直接把首屏图片喂给 WebView”，进一步逼近 1 秒内首帧。
- 日志结论：
  - 现状已进入 `RawRgbaFast`，`preview_prepare/resize` 已消失。
  - 当前瓶颈分布：
    - `capture_ms ≈ 316~327ms`
    - `overlay_ready_ms ≈ 119~121ms`
    - `get_screenshot_preview_rgba total_ms ≈ 4ms`
    - 但前端 `preview_raw_fetch_done fetch_ms ≈ 313~318ms`
    - `preview_first_paint draw_ms ≈ 22~30ms`
  - 说明瓶颈不在 Rust 取图，而在 `33MB` 原始像素通过 JS IPC 搬运和 `putImageData` 链路。
- 范围：
  - `src-tauri/src/app/mod.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src/pages/screenshot-overlay-page.tsx`
  - `src/lib/command-client.ts`
  - 协议层支持按 session 直接返回首屏预览图，不再要求前端先拿原始 RGBA 数组。
  - 前端单屏快路径改为协议 `<img>` 直显，保留 file fallback。
  - 增补协议路径的编码/响应/首帧耗时日志，继续按毫秒比较。
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工触发截图，对比 `capture_ms / overlay_ready_ms / preview protocol served raw session / preview_image_decode_done / preview_first_paint`

## 2026-03-15 截图首帧继续提速（快预览图 + 后台原图）
- 目标：继续压缩截图热键到编辑首帧的延迟，优先突破当前约 1 秒门槛。
- 范围：
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/domain/screenshot.rs`
  - `src-tauri/src/commands/screenshot.rs`
  - `src/pages/screenshot-overlay-page.tsx`
  - `src/types/backend.ts`
  - `src-tauri/Cargo.toml`（如需补 GDI feature）
- 方案：
  - 单屏快路径首帧改走逻辑尺寸快速预览图
  - 原始 4K monitor 数据后台补齐，用于最终裁剪/复制/保存
  - 前端避免因后台补齐再次重载首帧
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证热键到首帧、复制/保存结果
- 状态：进行中（2026-03-15）。

## 2026-03-15 截图首帧继续提速（WGC 原型 + Overlay 预热）
- 目标：
  - 在 Windows 单显示器场景下引入 `Windows.Graphics.Capture` 原型，验证 `capture_ms` 是否能明显低于当前 GDI 快路径。
  - 在应用启动时预热 overlay 窗口，减少热键触发时的窗口创建、样式锁定和稳定化成本。
- 范围：
  - `src-tauri/Cargo.toml`
  - `src-tauri/src/services/wgc_capture.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/app/mod.rs`
- 方案：
  - 新增 Windows 专用 `wgc_capture` 服务，使用 `CreateForMonitor + Direct3D11CaptureFramePool::CreateFreeThreaded` 捕获首帧。
  - `start_session()` 在单显示器快路径下优先尝试 WGC，失败立即回退现有 GDI 双产物路径。
  - `setup()` 启动期预热隐藏 overlay 窗口，并在 `start_session_completed` 中记录 `overlay_prewarmed`。
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证 `overlay_prewarm_ready / wgc_capture_completed / wgc_capture_failed / capture_strategy / overlay_ready_ms`
- 状态：已完成实现与静态验证，等待你本机 `npm run desktop:dev` 热键实测（2026-03-15）。

## 2026-03-15 截图热路径继续提速（常驻 WGC 最近帧缓存）
- 目标：
  - 把单显示器截图从“热键后现抓图”改成“后台常驻 WGC 最近帧缓存 + 热键直接冻结”。
  - 将原始 RGBA 转换延后到真正裁剪/复制/保存时才发生，避免热键路径继续浪费在 4K 全图 CPU 转换上。
- 范围：
  - `src-tauri/src/services/wgc_capture.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/app/mod.rs`
  - `scripts/work/2026-03-15-screenshot-live-capture-native-preview/*`
- 方案：
  - 新增常驻 `start_live_capture()`，长期持有 `GraphicsCaptureSession / FramePool / D3D11 device`。
  - 启动期初始化 live capture，回调里只缓存 top-down BGRA 最近帧，不在热键路径里再做现抓。
  - `start_session()` 优先尝试 `capture_strategy=live_cache`，命中时直接用缓存 BGRA 构造会话并生成逻辑尺寸首帧预览。
  - `CapturedMonitorFrame` 改为支持 `bgra_top_down + OnceLock<RgbaImage>` 懒转换，裁剪时才做 RGBA materialize。
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证 `live_capture_started / live_capture_frame_ready / live_capture_snapshot_used / start_session_completed capture_strategy=live_cache`
- 状态：已完成实现与静态验证，等待你本机热键实测（2026-03-15）。

## 2026-03-15 截图 overlay 几何回归止血
- 目标：
  - 修复最新 Win32 物理坐标 overlay 路径引发的 `window_moved` 事件风暴与截图冻结。
  - 保留此前 live capture、预览缓存、overlay 预热带来的时延优化。
- 范围：
  - `src-tauri/src/services/screenshot_service.rs`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
  - `scripts/work/2026-03-15-screenshot-live-capture-native-preview/deliverable.md`
- 方案：
  - 删除最新引入的 `SetWindowPos` 物理坐标定位分支。
  - `set_overlay_window_geometry()` 恢复为稳定的逻辑坐标定位 + probe 回正。
  - 用日志验证不再出现无限刷屏的 `overlay_geometry_drift_detected`。
- 验证：
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - 手工验证热键后不会卡死，且可正常进入截图编辑态
- 状态：已完成代码止血与静态验证，等待你本机回归（2026-03-15）。

## 2026-03-15 截图 overlay 旧预览闪屏修复
- 目标：
  - 修复第二次及后续打开截图编辑窗口时，先闪一下上一次截图画面的状态复用问题。
  - 保持当前已进入亚 100ms 级别的截图会话启动性能。
- 范围：
  - `src/pages/screenshot-overlay-page.tsx`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
  - `scripts/work/2026-03-15-screenshot-live-capture-native-preview/deliverable.md`
- 方案：
  - 在 `screenshot://session-updated` 收到新 `sessionId` 时，立即清空旧 `previewRenderable` 和 `previewSurfaceReady`。
  - 预览 `<img>` 改为只在 `previewSurfaceReady` 为真时渲染，并绑定 `key={session.sessionId}` 强制按会话重建。
- 验证：
  - `npm run web:build`
  - 手工验证连续触发截图热键时不再先看到上一次截图
- 状态：已完成实现与静态验证，等待你本机回归（2026-03-15）。

## 2026-03-15 默认截图热键改为 Ctrl+Shift+X
- 目标：
  - 将默认全局截图热键从 `Ctrl+Shift+1` 调整为 `Ctrl+Shift+X`。
  - 确保新用户默认值、设置页默认文案、以及老用户默认值迁移逻辑一致。
- 范围：
  - `src-tauri/src/domain/preferences.rs`
  - `src-tauri/src/domain/mod.rs`
  - `src-tauri/src/services/preferences_service.rs`
  - `src/lib/app-preferences.ts`
  - `src/pages/settings-page.tsx`
- 方案：
  - Rust 默认常量改为 `Ctrl+Shift+X`
  - 将 `Ctrl+Shift+1` 标记为上一代默认值，`Ctrl+Shift+4` 标记为更早默认值，迁移时统一修复到 `Ctrl+Shift+X`
  - 同步前端默认值和设置页“恢复默认/推荐默认”文案
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run web:build`
  - 手工验证新装默认值、旧默认自动迁移、用户自定义值不被覆盖
- 状态：已完成实现与静态验证，等待你本机回归（2026-03-15）。

## 2026-03-15 启动期 WGC 黄边止血
- 目标：
  - 修复程序启动后整块屏幕四边出现黄色捕获边框的问题。
  - 在当前 `tauri dev` / Win32 运行形态下，优先保证启动期不出现系统级捕获提示。
- 范围：
  - `src-tauri/src/app/mod.rs`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- 方案：
  - 取消启动期自动 `initialize_live_capture()`。
  - 保留 WGC 单次抓帧路径，但不再在程序启动时常驻 monitor capture。
- 验证：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - 手工验证程序启动后屏幕四边不再出现黄色捕获边框
- 状态：已完成代码止血与静态验证，等待你本机回归（2026-03-15）。

## 2026-03-15 overlay 常驻全尺寸热态
- 目标：
  - 将截图 overlay 从“1x1 预热 + 热键时放大/回正”切换为“全尺寸透明热态常驻”。
  - 进一步压缩 `overlay_ready_ms`，避免每次截图都执行 `hide/show/set_geometry/stabilize` 全链路。
- 范围：
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/commands/screenshot.rs`
  - `src/pages/screenshot-overlay-page.tsx`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
  - `scripts/work/2026-03-15-desktop-duplication-live-capture/task_plan.md`
  - `scripts/work/2026-03-15-desktop-duplication-live-capture/deliverable.md`
- 方案：
  - overlay 预热后保持全尺寸透明热态，默认 `set_focusable(false) + set_ignore_cursor_events(true)`。
  - 激活截图时优先复用当前窗口几何，仅在未对齐时才重新定位与稳定化。
  - 取消截图/复制/保存后不再 `hide()` overlay，而是恢复为透明热态并清空前端预览状态。
  - 捕获前仅在存在活动截图会话时隐藏 overlay，避免把透明热态窗口误当成需要关闭的活动窗口。
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - `npm run web:build`
  - 手工验证连续截图时 `overlay_ready_ms` 是否下降，且无旧图残留/无交互阻塞
- 状态：已完成代码与静态验证，等待你本机回归（2026-03-15）。
## 2026-03-15 位置对齐判定收紧
- 目标：修复 overlay 热态激活后 `-1,0` 漂移被误判为已对齐，导致抖动回归。
- 调整：`OverlayGeometryProbe::is_aligned()` 改为位置必须严格对齐，尺寸保留 1px 容差。
- 调整：`restore_overlay_window_hot_state()` 不再每次强制 `set_overlay_window_geometry()`，先探测，只有漂移时才修正。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-15 Overlay 事件风暴止血
- 根因：`window::handle_window_event()` 对 overlay 的程序化 `Moved/Resized` 也执行 `enforce_overlay_window_geometry()`，在严格位置对齐后放大成自激循环，导致窗口抖动并最终无响应。
- 修复：引入短时 `overlay_event_suppressed_until`，在预热、热态恢复、截图激活前对程序化几何更新开启 250ms 抑制窗口。
- 修复：窗口事件处理与 `enforce_overlay_window_geometry()` 双重检查抑制状态，阻断事件风暴。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 Overlay 事件监听回退止血
- 目标：先恢复截图窗口可用性，阻断 overlay `Moved/Resized` 自激循环导致的未响应。
- 调整：移除 `window::handle_window_event()` 中对 screenshot overlay 的自动几何回正监听，仅保留 `CloseRequested` 清理会话。
- 调整：日志插件新增固定目录目标，运行时将日志稳定写入仓库根目录 `log.log`，后续统一以此文件为排障入口。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 Overlay 回退到隐藏稳态
- 结论：当前体验问题不在 Desktop Duplication 抓帧，而在 overlay 常驻可见热态与截图结束后 `restore hot state` 这套生命周期设计。
- 调整：预热只创建/定位/锁样式，不再 `show()` overlay。
- 调整：截图结束后不再恢复透明热态，全都直接隐藏 overlay。
- 目标：移除可见透明 overlay 在 idle/close 阶段的窗口切换，先消掉闪屏、抖动和几何漂移。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 固定日志路径避开 src-tauri 监听
- 根因：固定日志路径被配置到当前工作目录，而 `tauri dev` 的 Rust 进程工作目录是 `src-tauri`，导致日志写入 `src-tauri\log.log`，每次写日志都会触发 watcher 重编译。
- 修复：固定日志目录改为：若当前目录是 `src-tauri`，则写到上级目录 `runtime-logs`；否则写到当前目录下 `runtime-logs`。
- 目标文件：`D:\Desktop\rust\BexoStudio\runtime-logs\log.log`
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 Overlay 激活热路径收敛
- 目标：修复“hidden 但已预热”的 overlay 仍被误判为需要重新 geometry/stabilize，导致 `overlay_ready_ms≈1.1~1.3s` 与首屏白闪。
- 调整：`move_and_focus_overlay_window()` 的几何复用条件改为“`overlay_prewarmed && geometry_aligned`”，不再额外要求 `was_visible=true`。
- 调整：仅在确实需要重新几何时才执行 `stabilize_overlay_window_after_show()`，避免对隐藏预热窗口重复跑窗口稳定化回路。
- 调整：`start_session()` 改为先 `prepare_overlay_window()` 并 `emit_session_updated()`，再执行窗口激活；让隐藏的 overlay 可以在弹出前开始加载最新 session，缩小白闪窗口。
- 调整：新增 `overlay_activation_profile` 日志，精确记录 `style/geometry/show/stabilize/focus/realign` 各阶段耗时。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 Live Cache 命中率兜底
- 目标：修复 `Desktop Duplication` 明明已启动，但热键瞬间仍可能退回 `wgc_single_monitor`，把 `capture_ms` 重新打回 `~1s`。
- 调整：`start_session()` 在首次 `try_capture_from_live_cache()` 失败后，不再立刻回退一次性 WGC，而是额外等待一个短窗口（180ms）轮询 DD 最新帧。
- 调整：新增 `live_capture_snapshot_wait_succeeded` / `live_capture_snapshot_wait_timed_out` 日志，用来区分“短等待后命中 live cache”与“确实只能退回 one-shot”两种路径。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 Hidden-Prewarmed Overlay 稳定化短路
- 目标：修复 `overlay_activation_profile` 中 `stabilize_ms≈1000~1265ms` 的主瓶颈；当前日志已证明这段稳定化对隐藏预热窗口每次都失败，但仍白白阻塞 1 秒以上。
- 调整：`move_and_focus_overlay_window()` 新增 `hidden_prewarmed` 分支；当 overlay 已预热但当前处于隐藏状态时，保留一次 geometry 设置，但跳过 `stabilize_overlay_window_after_show()`。
- 调整：`overlay_activation_profile` 新增 `hidden_prewarmed` 字段，用于确认热键路径是否命中了这条短路。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`。
## 2026-03-16 Overlay 首屏透明样式前置 + 隐藏预热激活轻量化
- 目标：继续压缩隐藏预热 overlay 激活时的可见抖动，并优先消除截图预览出现前的整屏白闪。
- 调整：`src-tauri/src/services/screenshot_service.rs` 将 `Color` 正确改为 `tauri::window::Color`，补齐透明背景 API 的实际编译与运行。
- 调整：overlay 窗口统一显式设置 `background_color(0,0,0,0)`；预热/复用/新建三条路径都覆盖，避免 WebView 使用系统默认白底。
- 调整：`move_and_focus_overlay_window()` 对隐藏但已预热的窗口不再无条件认为需要重新 geometry；只要 probe 已对齐，就直接复用几何，进一步减少 show 前窗口位移。
- 调整：`index.html` 在 head 阶段内联 screenshot overlay 透明背景样式，确保 CSS 主包加载前就已经不是白色初始背景。
- 验证：`cargo fmt --manifest-path src-tauri/Cargo.toml`，`cargo check --manifest-path src-tauri/Cargo.toml`，`npm run web:build`。
## 2026-03-16 Overlay 焦点漂移补偿记忆
- 目标：继续压缩剩余的 `overlay_ready_ms≈200~300ms`，直指日志里稳定出现的 `post_focus_activation @-2,0 -> @0,0` 残余回正。
- 调整：`ScreenshotState` 新增 `overlay_focus_drift_compensation`，按显示尺寸与缩放因子记录最近一次焦点激活后的逻辑位置补偿。
- 调整：`move_and_focus_overlay_window()` 激活前读取补偿值；若当前会话尺寸/缩放匹配，则预先以补偿后的逻辑坐标设几何，而不是等 `set_focus()` 之后再回正。
- 调整：当 `realign_overlay_window_if_needed()` 仍探测到小范围位置漂移（当前限制为尺寸对齐且 `|delta_x|/|delta_y| <= 4`）时，更新补偿缓存，后续激活直接复用。
- 调整：`overlay_activation_profile` 日志新增 `focus_compensation=(x,y)`，用于确认补偿是否命中。
- 验证：`cargo fmt --manifest-path "src-tauri/Cargo.toml"`，`cargo check --manifest-path "src-tauri/Cargo.toml"`，`npm run web:build`。

## 2026-03-16 Native Preview Layer 蓝图与分阶段改造方案
- 目标：
  - 停止继续深挖单个全屏 WebView screenshot overlay。
  - 形成 Windows-first 的 `native preview layer` 蓝图，明确底图显示、交互层和 Tauri 壳层的职责边界。
  - 给下一轮 `Phase B: Native Bottom Layer MVP` 提供稳定施工依据。
- 范围：
  - `scripts/work/2026-03-16-native-preview-layer-blueprint/*`
  - `docs/technical-architecture.md`
  - `docs/implementation-roadmap.md`
  - `task_plan.md`
  - `findings.md`
  - `progress.md`
- 方案：
  - 保留 `Desktop Duplication` 作为最近帧源。
  - 新增 `NativePreviewWindow`（Win32 + D3D11 + DXGI swap chain + DirectComposition）承担冻结底图显示。
  - 最终将选区、高频交互与小工具栏也下沉到 Native。
  - `Tauri/WebView` 最终只保留截图配置与低频诊断页面。
  - 分阶段推进：
    - Phase B 先替换底图
    - Phase C 再原生化选区/句柄/高频输入
    - Phase D 再原生化小工具栏
    - Phase E 让 WebView 退出截图运行时主链路
- 验证：
  - 产出蓝图文档、风险分析、回滚策略与手工验证步骤
  - 暂不写应用代码
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase B: Native Bottom Layer MVP 实施计划与代码骨架
- 目标：
  - 开始 `NativePreviewWindow` 路线的实际工程落地。
  - 先交付一个可编译、可注入、边界清晰的 `NativePreviewService` 骨架。
  - 明确应用启动期初始化、状态机、session 规格和 Windows backend 入口。
- 范围：
  - `src-tauri/src/services/native_preview_service.rs`
  - `src-tauri/src/services/mod.rs`
  - `src-tauri/src/app/mod.rs`
  - `scripts/work/2026-03-16-native-preview-layer-phase-b-mvp/*`
- 方案：
  - 新增 `NativePreviewService`
  - 新增 `NativePreviewLifecycleState / NativePreviewSessionSpec / NativePreviewSourceKind`
  - 启动期初始化 NativePreviewService，但不接管现有截图链路
  - Windows 下实际完成 `D3D11 device + DXGI factory` bootstrap，并由 NativePreviewService 持有 backend 资源
  - 若初始化失败，仅记录日志，不影响当前截图功能
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - 启动期日志确认 native preview service 初始化结果
- 状态：已完成代码骨架与静态验证（2026-03-16）。

## 2026-03-16 Phase B: NativePreviewWindow + SwapChain + DirectComposition 基础设施
- 目标：
  - 继续推进 `Phase B`，把 native preview 从“服务骨架”推进到“真实原生渲染基础设施”。
  - 在不切换现有截图运行路径的前提下，完成：
    - 隐藏 Win32 preview window
    - `D3D11 device/context`
    - `IDXGIFactory2`
    - composition `IDXGISwapChain1`
    - `IDCompositionDevice / Target / Visual`
    - 一次最小透明 present 验证
- 范围：
  - `src-tauri/Cargo.toml`
  - `src-tauri/src/services/native_preview_backend_windows.rs`
  - `src-tauri/src/services/native_preview_service.rs`
  - `scripts/work/2026-03-16-native-preview-layer-phase-b-mvp/*`
  - 根级 `task_plan.md / findings.md / progress.md`
- 方案：
  - 新增 `windows` crate 的 `Win32_Graphics_DirectComposition / Win32_UI_WindowsAndMessaging / Win32_System_LibraryLoader` feature
  - Windows backend 真实创建隐藏预览窗口和 DComp 组合树
  - 通过 opaque backend handle 规避 `tauri::Manager::manage()` 的 `Send + Sync` 限制，不把 COM/`HWND` 直接暴露到共享状态
  - 初始化失败仅记录日志，不影响当前截图链路
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
- 状态：已完成基础设施落地与静态验证（2026-03-16）。

## 2026-03-16 Phase B: Native Preview Runtime Path
- 目标：
  - 把 `Desktop Duplication` 最近帧真正提交到 native swap chain
  - 打通 native preview 的 `prepare/show/hide/resize`
  - 在 `ScreenshotService` 生命周期里做最小接入，但不回头修旧 WebView 底图链路
- 范围：
  - `src-tauri/src/services/native_preview_backend_windows.rs`
  - `src-tauri/src/services/native_preview_service.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `scripts/work/2026-03-16-native-preview-layer-phase-b-mvp/*`
- 方案：
  - Windows backend 支持按会话几何更新窗口与 swap chain，并把 BGRA 帧提交到 back buffer
  - `NativePreviewService` 暴露真实 runtime API，并记录结构化耗时日志
  - `ScreenshotService::start_session()` 对单屏 live cache 路径优先提交 native preview
  - copy/save/cancel 路径统一隐藏并清理 native preview
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - 手工截图一次，确认 native preview 运行时日志出现且无新回归
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase B: Native Preview Foreground Layer & Parallel Display
- 目标：
  - 让 native preview 真正进入截图显示链路的前台可见层级
  - 建立 native preview 与透明 overlay 的并行显示关系
  - 让 WebView 不再承担截图底图的屏幕渲染职责
- 范围：
  - `src-tauri/src/domain/screenshot.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/commands/screenshot.rs`
  - `src/pages/screenshot-overlay-page.tsx`
  - `src/types/backend.ts`
- 方案：
  - 新增 `ScreenshotSessionView.nativePreviewActive`
  - native preview 显示成功后，截图会话明确标记原生底图已接管
  - overlay 页面继续在后台 decode 图片对象，但不再把 WebView `<img>` 当底图渲染
  - 交互就绪判断对 `nativePreviewActive` 放宽，不再强制等待 WebView 底图可见
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - `npm run web:build`
  - 手工截图时确认日志出现 `native_preview_active=true`
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase B: Native Preview Z-Order 稳定化
- 目标：
  - 让 native preview 与 overlay 不再只是“都在 topmost 带里”，而是明确形成“native preview 在下，overlay 在上”的稳定层级关系
  - 收紧截图会话 show/hide 时序，避免相对层级受焦点切换影响
- 范围：
  - `src-tauri/src/services/native_preview_backend_windows.rs`
  - `src-tauri/src/services/native_preview_service.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `scripts/work/2026-03-16-native-preview-layer-phase-b-mvp/*`
- 方案：
  - Windows backend 暴露 `show_below_window / sync_z_order_below_window`
  - screenshot 热路径改为先准备隐藏 overlay，再用 overlay HWND 作为锚点显示 native preview，overlay 激活后再做一次相对层级矫正
  - 通过结构化日志记录 anchor HWND、首次 show 和 post-focus sync 的耗时
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - 手工截图时确认日志出现 `native_preview_window_shown anchor=overlay` 与 `native_preview_z_order_synced`
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase C: Native Interaction Window Skeleton
- 目标：
  - 启动 `NativeInteractionWindow` 第一轮骨架，为后续原生化选区、拖拽、句柄与命中测试提供服务边界和 Windows backend 入口
  - 当前不接管现有 WebView 高频交互链路
- 范围：
  - `src-tauri/src/services/native_interaction_service.rs`
  - `src-tauri/src/services/native_interaction_backend_windows.rs`
  - `src-tauri/src/services/mod.rs`
  - `src-tauri/src/app/mod.rs`
  - `scripts/work/2026-03-16-native-interaction-window-phase-c/*`
- 方案：
  - 新增 `NativeInteractionService`
  - 新增 `NativeInteractionLifecycleState / NativeInteractionSessionSpec`
  - Windows 下创建隐藏的透明 `NativeInteractionWindow` 骨架并提供最小 show/hide/resize API
  - 启动期初始化 service，失败只记录日志，不影响现有截图链路
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - 启动应用时确认日志出现 `native_interaction_service_initialized` 或明确失败原因
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase C: Native Basic Selection MVP
- 目标：
  - 把 `NativeInteractionWindow` 从骨架推进到“基础选区高频交互 MVP”
  - 承接透明遮罩、选区矩形、8 向句柄 hit test 和拖拽捕获
  - 开始把基础选区交互从 WebView 往 Native 下沉
- 范围：
  - `src-tauri/src/services/native_interaction_service.rs`
  - `src-tauri/src/services/native_interaction_backend_windows.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/commands/screenshot.rs`
  - `src-tauri/src/domain/screenshot.rs`
  - `src/types/backend.ts`
  - `scripts/work/2026-03-16-native-interaction-window-phase-c/*`
- 方案：
  - 使用 `WS_EX_LAYERED` 顶层窗口 + `UpdateLayeredWindow` 绘制全屏半透明遮罩
  - Native backend 维护基础选区状态、句柄命中结果和拖拽状态机
  - 使用 `WM_LBUTTONDOWN / WM_MOUSEMOVE / WM_LBUTTONUP` 与 `SetCapture / ReleaseCapture` 完成创建、移动和 resize
  - 先把基础选区交互接入截图 show/hide 生命周期，不迁移复杂标注
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - 手工截图并拖出选区，确认日志出现 `native_interaction_drag_started / native_interaction_selection_updated / native_interaction_drag_committed`
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase C: Native Basic Selection MVP 实施进展
- 已完成：
  - `NativeInteractionWindow` 透明 layered 绘制层
  - 原生基础选区矩形
  - 8 向句柄 hit test
  - 创建 / 移动 / resize 拖拽捕获
  - `NativeInteraction` 状态查询命令和前端类型
- 仍待完成：
  - 接入截图运行时 show/hide
  - 在基础框选模式下把 pointer 输入从 WebView 切到 NativeInteractionWindow

## 2026-03-16 Phase C: Native Base Selection Runtime Wiring
- 目标：
  - 把 `NativeInteractionWindow` 接入截图会话生命周期
  - 在 `select` 工具下让基础框选由 Native 承接
  - 保持工具栏继续在 WebView 中工作
- 范围：
  - `src-tauri/src/services/native_interaction_service.rs`
  - `src-tauri/src/services/native_interaction_backend_windows.rs`
  - `src-tauri/src/services/screenshot_service.rs`
  - `src-tauri/src/commands/native_interaction.rs`
  - `src-tauri/src/commands/screenshot.rs`
  - `src/lib/command-client.ts`
  - `src/types/backend.ts`
  - `src/pages/screenshot-overlay-page.tsx`
  - `scripts/work/2026-03-16-native-interaction-window-phase-c/*`
- 方案：
  - NativeInteraction 新增 runtime update API：
    - `visible`
    - `exclusion_rects`
  - layered window 上 toolbar / text editor 区域打成 alpha=0，点击透传回 WebView
  - `ScreenshotService.start_session()` 先 prepare native interaction
  - copy/save/cancel 后统一 hide + clear native interaction
  - WebView 在 `tool === select && annotations.length === 0` 时轮询 native selection 状态，并停止自身基础框选 pointer 处理
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - `npm run web:build`
  - 手工截图时确认：
    - native interaction 选区可用
    - toolbar 可点击
    - 基础框选不再由 WebView 处理
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase C: Native Interaction Render Hot Path Optimization
- 目标：
  - 解决基础框选拖拽卡顿
  - 保持现有 `Desktop Duplication + NativePreviewWindow` 成果不回退
- 范围：
  - `src-tauri/src/services/native_interaction_backend_windows.rs`
  - `src-tauri/src/services/native_interaction_service.rs`
  - `scripts/work/2026-03-16-native-interaction-window-phase-c/*`
- 方案：
  - 把 `UpdateLayeredWindow` 所需的 screen DC / memory DC / DIBSection 改为按窗口尺寸复用
  - 仅在尺寸变化时重建 GDI surface
  - 为 native interaction present 链路增加阶段耗时日志
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - 手工拖拽选区，确认：
    - `native_interaction_session_prepared present_ms` 明显下降
    - 拖拽过程主观卡顿显著缓解
- 状态：进行中（2026-03-16）。

## 2026-03-16 Phase C: Native Interaction Event Sync + Cursor + Rect Annotation
- 目标：
  - 将 `NativeInteractionState` 从轮询同步改为事件化同步
  - 将基础 hover / cursor 提示完整下沉到 NativeInteractionWindow
  - 交付矩形标注原生创建第一版
- 范围：
  - `src-tauri/src/domain/mod.rs`
  - `src-tauri/src/domain/native_interaction.rs`
  - `src-tauri/src/services/native_interaction_service.rs`
  - `src-tauri/src/services/native_interaction_backend_windows.rs`
  - `src-tauri/src/commands/native_interaction.rs`
  - `src/lib/command-client.ts`
  - `src/types/backend.ts`
  - `src/pages/screenshot-overlay-page.tsx`
  - `scripts/work/2026-03-16-native-interaction-window-phase-c/*`
- 方案：
  - 新增 `native_interaction://state-updated`
  - 新增 `native_interaction://rect-annotation-committed`
  - `select` 工具完全用事件驱动同步 native 选区
  - `rect` 工具由 native 创建草稿并在鼠标抬起后提交到现有 annotation store
- 验证：
  - `cargo fmt --manifest-path "src-tauri/Cargo.toml"`
  - `cargo check --manifest-path "src-tauri/Cargo.toml"`
  - `npm run web:build`
  - 手工验证基础框选与矩形标注均不再依赖轮询
- 状态：进行中（2026-03-16）。

## 2026-03-16 NativeInteraction Event Sync + Rect MVP
- Completed backend event path for native interaction: selection/hover changes now emit `native_interaction://state-updated`; rect tool draft/commit now emit `native_interaction://rect-annotation-committed`.
- Removed WebView 40ms polling loop for native interaction state in screenshot overlay; switched to one-shot runtime update response plus event listeners.
- Added native interaction runtime mode selection (`selection` / `rect_annotation`) and passed current selection, annotation color, and stroke width to Rust runtime update.
- Added first native rect annotation MVP: native interaction layer now draws rect draft and commits rect annotations back into existing `ShapeAnnotation` pipeline.
- Verified with `cargo check` and `npm run web:build`.

## 2026-03-16 NativeInteraction event tightening + ellipse MVP
- Tightened native interaction synchronization to reduce feedback-loop jitter:
  - WebView no longer sends selection back to backend while in `selection` mode.
  - Frontend native interaction state updates are now equality-checked before triggering React state updates.
- Generalized native annotation commit from rect-only to shape commit event (`shape_annotation_committed`) with `kind=rect|ellipse`.
- Added `ellipse_annotation` native runtime mode and first native ellipse draft/commit path.
- Verified with `cargo check` and `npm run web:build`.
