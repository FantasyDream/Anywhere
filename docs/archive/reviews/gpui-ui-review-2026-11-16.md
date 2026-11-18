# GPUI UI 审查与解决方案（2026-11-16）

审查基线：`codex/experiment-gpui-kit`，HEAD
`75dee3ebb18a0e4e8381785e8dc3eb73da7ec01a`，包含上轮尚未提交的原生 resize
无动画事务修复。以下正文保留当时的审查与方案；同日晚些时候已在工作区完成
15 项修复、200 项 GPUI 回归和 v166 / v167 真机复核，逐项状态见文末“修复
复核”。临时诊断代码和观测标记已从源码移除，上轮两处修改予以保留。

本轮记录 15 项：11 项界面 / 交互问题、2 项实现边界缺口、1 项开发预览问题、
1 项测试缺口。结论分别标注为 GPUI 诊断复现、源码确认或原生几何推导。
没有真实桌面的逐帧视频，因此不能宣称已找出所有合成撕裂，或已完成原生视觉验收。

当前产品契约仍以 [UI 设计](../../ui-design.md)、
[Launcher 交互](../../launcher-interaction.md)和[使用指南](../../usage.md)为准；
此文是对应基线的审查记录。

## 1. 同类过渡问题的检查结果

应用页面没有显式添加整页缩放、滑动或交叉淡入动画。存在的组件动效包括 Spinner、
Tooltip 和输入框颜色变化；共享 Web 控件使用 120 ms 背景 / 边框颜色过渡。
这些动效的存在不能证明其会产生用户反馈的撕裂。

上轮修复只在原生窗口尺寸变化时执行。**尺寸不变的结果替换、错误提示覆盖、
弹层切换以及 GPUI 渲染缓存更新不会因此自动得到修复。** 本轮实际复现的多处
越界和尺寸不同步，本身也会产生文字叠在旧内容上、空白突变或结果被裁切的现象。

| 场景 | 当前更新路径 | 本轮判断与后续处理 |
| --- | --- | --- |
| Launcher 输入 / 清空 / 结果数量变化 | `desired_window_size` → 延后 resize → AppKit | 已有无动画事务；仍需修复 UI03 的漏触发和 UI02 的越界 |
| Scope、菜单、对象 Actions | 前端先改变页面，再按不同事件请求 resize | Scope 点击缺少即时尺寸同步；统一检查可见区域的变化 |
| 文件搜索、Clipboard、Application History | 固定页面尺寸，替换列表 / 状态 / Actions | 没有整页布局动画；同尺寸撕裂需要独立排查渲染与合成，不能只检查 resize |
| Settings、Web / Plugin 页面进入 / 返回 | 共用主原生窗口，修改尺寸和窗口样式 | 尺寸经过共享入口；style mask、材质重装与 WebView 布局仍需原生桌验 |
| Tasks、Clipboard Shelf 手动缩放 | 各自原生窗口的 live resize | 不经过主窗口自动 resize 补丁；需分别检查材质和 Metal 内容同步 |
| Tooltip、输入框、Web 控件反馈 | 组件 / CSS 局部动效 | 没有同源撕裂证据；系统 Reduce Motion 接入存在 UI12 缺口 |

保留“窗口和结果布局直接切换”的方案。后续如增加淡入，只对尺寸已经稳定的局部
内容操作透明度；不对搜索头、文字、整张旧纹理或原生玻璃做缩放。

## 2. 需要优先修复的问题

### UI01 · P1 · 设置关闭确认框没有隔离底层交互

- 位置：[settings.rs](../../../apps/gpui/src/launcher/view/settings.rs) 的
  `render_settings`，以及 Launcher 的 Settings Tab 路由。
- 诊断复现：打开脏草稿关闭确认后，Tab 仍能聚焦底层 Search 分类，Space 能切换
  分类，确认框继续显示；直接点击遮罩下的 `open-at-login`，开关也从 true 变为 false。
- 影响：提示“保存 / 放弃”期间，底层设置仍可改变；部分底层设置会立即调用运行时。
- 方案：优先使用 GPUI Kit 的 Dialog 生命周期，隔离遮罩命中与焦点；保存当前焦点，
  打开时聚焦“继续编辑”，限定 Tab / Shift+Tab 的循环范围，关闭后恢复焦点。
  对绑定选择等自绘模态层同样检查。沿用无布局动效策略。
- 验收：确认框打开后，鼠标不能改变底层开关，键盘不能到达底层分类或控件；
  Esc、Enter、Tab、保存中和 IME 组词分别验证。

### UI10 · P1 · 剪贴板批量删除和缩短期限没有确认

- 位置：`settings.rs` 的 `clipboard-clear-settings`、`clipboard-retention`；
  [clipboard.rs](../../../apps/gpui/src/host/runtime/clipboard.rs) 的运行时调用；
  [service.rs](../../../crates/anywhere-core/src/features/clipboard/service.rs) 的
  `set_retention_days` / `clear`。
- 源码确认：Clear 点击后直接执行清空；期限按钮直接写入 30 天。若原期限为 90 天，
  该操作立即清理超期内容。存储适配执行数据库 DELETE，没有产品内撤销流程。
  使用指南明确要求缩短期限先确认。
- 方案：Clear 和会删除数据的期限调整先展示明确的确认框，说明删除范围与不可恢复性；
  确认前只修改待确认值，取消不调用运行时；提交期间阻止重复触发。与 UI01 共用
  正确的模态交互能力。
- 验收：用独立临时库验证取消保留全部数据、确认只提交一次、缩短期限删除对应
  过期数据、延长期限不需要删除确认。不得用个人历史做删除测试。

### TEST01 · P1 验收缺口 · 部分尺寸测试没有改变实际布局 viewport

- 位置：[view/mod.rs](../../../apps/gpui/src/launcher/view/mod.rs) 中使用
  `resize_window_for_screen` / `window.resize` 的布局测试，例如模式选择器和长候选测试。
- 诊断复现：调用 `window.resize(360×360)` 后，bounds 为 360×360，viewport 仍为
  **1536×1080**；调用 `TestAppContext::simulate_window_resize` 后二者才一致。
  原因是锁定的 `gpui-fast 0.1.14` 测试后端中，普通 resize 只更新平台 bounds，
  模拟 resize 才触发布局尺寸回调。
- 影响：部分“紧凑窗口 / 窄宽度已通过”的断言没有证明真实视口中的布局正确。
  这不是已确认的生产窗口 resize 缺陷，也不影响所有状态 / 输入单元测试。
- 方案：为受影响的布局测试建立一个小的 viewport 同步 helper，或在创建测试窗口时
  指定真实初始尺寸；每次测量前断言 viewport 和目标尺寸一致，再检查可见区域、
  元素边界、相邻元素交集及滚动可达性。
- 验收：在 648 点 Launcher、480 / 360 点 Shelf、920 / 760 点 Tasks 上复验；
  不能只断言 `window.bounds()`。

## 3. 布局和交互缺陷

### UI02 · P2 · 多行提示覆盖搜索头与结果行

- 位置：Launcher `render_launcher` 的固定 44 点 notice 包装，及
  [components/mod.rs](../../../apps/gpui/src/launcher/components/mod.rs) 的可换行 Notice。
- 诊断复现：8 行错误提示的 Notice 高 147.5 点，从 y=20.5 开始；搜索头占
  y=0–72，结果行从 y=116.5 开始。三者实际重叠。
- 方案：取消固定 44 点与无限换行内容的组合。按实际测量高度为提示分配区域；
  较长详情使用有上限的滚动 / 展开区。重新计算窗口高度，保持搜索头与结果容器不可互相覆盖。
- 验收：单行、多行、长路径、中英文错误；提示上下边界不与搜索头或结果行相交，
  超过窗口上限的详情仍能完整访问。

### UI03 · P2 · UI 可见区域变化未统一触发窗口尺寸更新

- 位置：`activate_selection`、`finish_action_result`、`scope_button` 和
  `resize_current_window`；快照处理主要按 `rows_changed / scopes_changed` 请求 resize。
- 诊断复现：一条结果时窗口高 136.5 点；动作失败后期望 180.5 点，实际仍为
  136.5 点，结果行移动到 y=116.5–172.5，超出窗口。
- Scope 子场景：点击会先本地过滤行，但未立即请求 resize。缓存回包诊断从 5 行变
  1 行后，窗口仍高 396.5 点、期望 172.5 点。**当前 Core 普通解析会创建新的候选
  Arc，通常会在回包时补做 resize，因此此诊断不证明普通实时路径会永久保持错误高度。**
  本地切换与回包之间仍存在旧高度的中间状态。
- 方案：让 notice、Scope、菜单、Actions、loading、输入模式等影响尺寸的状态变化
  进入同一尺寸更新入口；合并同一轮请求，执行时读取最新页面 / 窗口身份。
  最小修复先补齐明确漏掉的提示与本地 Scope 变化。
- 验收：提示出现 / 消失、失败回包、5→1→5 行、空结果、loading→empty、同候选集
  元数据更新、快速返回 Settings；最后的尺寸始终对应最后的 UI 状态。

### UI04 · P2 · Scope 整组没有水平居中

- 位置：`render_launcher` 中 `launcher-scope-scroll`。
- 诊断复现：648 点 viewport 下，两个 Scope 的整组中心 x=86.5，而窗口中心
  x=324。按钮内部和垂直方向已经居中，缺少的是整组水平布局规则。
- 方案：外层维持横向滚动 viewport，内层承载自然宽度按钮组；内容不足一屏时居中，
  超过一屏时从可滚动起点排列。保留选中项滚入视口。避免直接对溢出的滚动容器
  设置 `justify_center`，导致首项落在负坐标而无法访问。
- 验收：单个非 Current App Scope、两个 Scope、多个长名称；中英文、点击、
  Tab / Shift+Tab 后都验证居中与滚动可达性。

### UI05 · P2 · Clipboard Shelf 页头进入原生交通灯区域

- 位置：[clipboard_shelf.rs](../../../apps/gpui/src/launcher/view/pages/clipboard_shelf.rs)
  的首个 64 点 header；[app.rs](../../../apps/gpui/src/app.rs) 的 `shelf_window_options`；
  原生配置保留三颗窗口按钮。
- 几何证据：Shelf 状态图标占 x=20–46、y=19–45；原生交通灯从约 x=14、y=16
  开始布置。480 和 360 点宽度下都进入同一区域。Tasks 有 28 点 titlebar inset，
  Shelf 没有。原生遮挡的最终外观仍需桌面确认。
- 方案：保留现有原生按钮，为 Shelf 页头预留对应 titlebar 区域，并纳入最小高度、
  搜索框与结果区的尺寸计算。
- 验收：默认 480×520 与最小 360×360 下，原生关闭 / 最小化 / 缩放按钮与状态、
  输入框没有重叠，鼠标命中正确。

### UI06 · P2 · Shelf 可见 Esc 与键盘 Esc 行为不同

- 位置：`clipboard_shelf.rs` 的 `shelf-escape` 直接调用 `go_back`。
- 诊断复现：有查询时，键盘 Esc 清空查询并保留 Shelf；点击可见 Esc 直接移除窗口。
  局部 Actions / 删除确认也可能被直接跳过。
- 方案：可见 Esc 调用现有 Esc 路由，与键盘共用“预览 / 局部操作 → 查询 → 关闭”
  的处理顺序；原生红色关闭按钮继续承担明确关窗行为。
- 验收：查询、Actions、删除确认、Quick Look 和空白页各比较键盘与鼠标结果。

### UI07 · P2 · 未支持工具页的长输入被裁切且无法滚动

- 位置：[unsupported_tool.rs](../../../apps/gpui/src/launcher/view/pages/unsupported_tool.rs)。
- 诊断复现：648×480 viewport 中，60 行输入使内容容器高 1293 点、输入文字高
  1098 点；页面外层裁切，内部没有滚动入口。
- 方案：内容区使用 `flex_1 + min_h(0)` 和普通 GPUI 滚动区域；长标识符与输入
  明确设置换行规则。页头保持固定，必要时提供复制输入 / 错误详情入口。
- 验收：空输入、多行正文、长 URL / 无空格路径；最后一行可滚动访问。

### UI08 · P2 · 放弃语言草稿没有恢复当前界面语言

- 位置：`settings.rs` 的 `language_settings`，以及
  [controller.rs](../../../apps/gpui/src/launcher/view/settings/controller.rs) 的 `discard_settings`。
- 诊断复现：已保存中文 → 点击切英文 → Discard。草稿和已保存值均为 Zh，当前
  `locale` 仍为 En，返回 Launcher 后继续显示英文。
- 方案：统一语言预览和恢复入口；Discard 根据已保存语言同步 locale、模型、各输入
  placeholder 和可见原生文案；保存失败继续保留草稿，保存成功才更新生效值。
- 验收：中→英→放弃、英→中→放弃、System 模式、保存失败与重新打开设置；
  Launcher、设置、菜单文案与已保存状态保持一致。

### UI09 · P2 · 剪贴板暂停 / 保留期限控件没有完成选择和状态闭环

- 位置：`settings.rs` 的 `clipboard_settings`。
- 源码确认：期限按钮无论当前值为何都写 30；暂停按钮始终显示 Paused，点击只暂停
  15 分钟，没有 5 / 60 分钟与提前恢复入口。Core 已支持这些值，包含 `pause(0)` 恢复。
  设置页还依赖可能未加载的 `clipboard.status`，期限与用量可落入默认显示。
- 方案：进入分类时读取当前状态，用普通选择菜单提供 5 / 15 / 60 分钟和恢复，以及
  容量限制 / 1 / 7 / 30 / 90 天；显示真实暂停状态与当前选项。破坏性变化遵守 UI10。
- 验收：从尚未进入历史页的冷启动直接打开设置；各选项提交、失败、刷新、重新打开
  都显示真实值；暂停后可以立即恢复。

### UI11 · P3 · 成功、帮助和错误反馈使用了错误的视觉语义

- 位置：[tasks.rs](../../../apps/gpui/src/launcher/view/pages/tasks.rs) 的全页 notice
  固定使用 Error；Launcher / Shelf 的通用 notice 固定使用 Warning。
- 源码确认：Tasks 的“已复制”与键盘帮助也会以错误样式显示；相似操作在不同页面
  显示不同颜色和图标。
- 方案：提示状态同时携带已有 `NoticeKind` 与文案，事件产生时确定类型；复用
  当前 Notice 组件，不通过匹配本地化字符串推断成功或错误。
- 验收：复制成功为 Success，帮助为 Info，执行失败为 Error，权限限制为 Warning；
  同一语义跨页面、跨语言一致。

## 4. 实现边界与开发验证问题

### UI12 · P2 · macOS Reduce Motion 未连接到 GPUI 动效开关

- 源码确认：材质环境读取 Reduce Transparency / Increase Contrast，没有读取
  Reduce Motion；应用没有调用 `App::set_reduce_motion`。锁定依赖的 Tooltip 使用
  GPUI 的 reduce-motion 状态，App 默认值为 false。Web CSS 已有对应媒体查询。
- 方案：从平台读取并监听系统 Reduce Motion，初始化及变化时同步 GPUI 状态；
  新增的局部动效统一遵循此状态。无需新增一个平行的应用动画设置。
- 验收：系统选项开 / 关、应用运行中切换、Tooltip 和 Web 控件；需真实桌面补验。
  此项不等同于已证明 Tooltip 是用户撕裂问题的来源。

### UI13 · P2 · 工具页 resize 缺少可用屏幕尺寸约束

- 位置：`desired_window_size` 与 `platform::macos::launcher_frame`。
- 源码确认：初始窗口创建会约束到可用屏幕；之后 Web / Tasks 等页面可请求
  920×600，原生 resize 只约束原点，没有缩小尺寸。可用区域小于目标尺寸时，
  仅调整坐标无法让整个窗口留在屏内。
- 方案：每次 resize 都根据目标屏幕约束大小；页面内容提供明确的窄宽度适配和
  滚动策略。工具宽度、原生最小尺寸和布局规则需要一起调整。
- 验收：800×600 等小可用区域、菜单栏 / Dock、多显示器与负坐标；检查全部边界
  位于可用区域内。此项是边界分析，本轮没有原生小屏桌验。

### DEV01 · P3 · 无 Core 预览的输入过滤不调整窗口高度

- 位置：输入 Change 监听器和 `request_live_update` 的无 runtime 返回路径。
- 诊断复现：预览从 5 条结果输入到零匹配后，窗口仍高 360.5 点，期望 136.5 点。
- 方案：本地 preview 模型发生布局变化后也调用现有尺寸更新入口。
- 验收：preview 输入、清空与模式切换；将此问题限定于开发预览，不当成已确认的
  正常 Core 查询路径缺陷。

## 5. 建议实施顺序

1. **修正验证基础**：TEST01。先建立真实 viewport 的断言，保存上述失败序列为
   回归测试，避免后续修复继续被过大的测试画布掩盖。
2. **恢复模态与删除流程**：UI01、UI10。先完成遮罩、焦点和确认，再接入批量删除
   与期限调整；全部用独立临时数据验证。
3. **收敛布局更新**：UI02、UI03、UI04、UI05、UI06。维持无布局过渡，修复提示高度、
   尺寸漏触发、Scope 居中、Shelf 页头与返回行为。
4. **补齐设置与内容页**：UI07、UI08、UI09、UI11、DEV01。完成滚动、语言恢复、
   设置选择和反馈语义；修改文案与尺寸时更新本地化 / tokens 来源并运行生成器。
5. **完成平台与视觉矩阵**：UI12、UI13。对同尺寸替换、快速伸缩、Settings 返回、
   WebView 和独立窗口缩放采集逐帧证据，分别记录逻辑、构建和物理视觉结果。

## 6. 尚未确认为缺陷的调查项

- **同尺寸列表仍撕裂**：选择结果数量相同的两个查询反复切换，与确实发生 resize 的
  序列分别录屏。若同尺寸也失败，在独立测试实例中分别 A/B
  `GPUI_VIEW_RETENTION=0` 和 `GPUI_SCROLL_LAYERS=0`，定位缓存路径；这些是锁定
  GPUI Fast 的诊断开关，不建议未经性能验证就作为发行配置。
- **排队 resize 的失效边界**：当前 generation 在新 resize 请求时更新。还需用可控
  队列验证“旧请求等待 → 新页面目标尺寸恰好等于当前尺寸 → 新请求被跳过”的次序；
  若能复现旧尺寸覆盖新页面，再将页面 / 窗口 epoch 纳入提交校验。
- **大列表与输出刷新成本**：文件、剪贴板、应用历史及 Tasks 在 render 中 clone / 构建
  内容。先测最大数据量、连续输入与输出刷新下的帧耗时，再决定使用 GPUI 原生虚拟
  列表或减小快照复制。现有代码成本不等于已经测出掉帧或撕裂。
- **其他自绘浮层的键盘边界**：本轮确认了 Settings 关闭提示；绑定选择器、对象菜单、
  Clipboard 删除层还应补齐同样的遮罩、焦点恢复和外部点击验收，逐个确认后修复。
- **真实原生合成**：玻璃、交通灯、手动窗口缩放、Quick Look、IME 和 WebView 的
  桌面证据仍待采集，不能用本轮 headless 布局测试替代。
- **门禁稳定性**：首次全仓检查在 4 个文件扫描子进程 fixture 上失败，表现为超时
  或未生成 PID 文件；相关测试串行重跑及随后默认全仓重跑通过。原因尚未定位，
  应继续区分 fixture 启动延迟、测试调度和实际扫描逻辑，不据此宣布生产搜索有缺陷。

## 7. 验证记录

- 13 项临时 GPUI 诊断完成，其中页面矩阵覆盖 11 种 Screen × 中英文，共 22 个
  默认页面状态；另有 Scope、Shelf 最小宽度、模态、提示、长输入、语言放弃和测试
  后端尺寸诊断。它们观测现有行为，不是声明缺陷已修复的回归测试。
- 所有用于布局结论的测量均同步实际 viewport；另有专门的对照诊断记录普通
  `window.resize` 的测试后端差异。数据全部为合成 fixture。
- 诊断源码、临时观测补丁、原始输出保存在本机
  `build/gpui-ui-review-20261116-a9qluve3/`。该目录不随 Git 交付。
- 临时诊断源码与观测标记已移除；现有日常应用进程与个人数据未修改。
- 恢复源码后，`scripts/check-gpui.sh` 通过，包含原有 182 项 GPUI 测试和严格 Clippy。
- `scripts/check.sh` 首次在上述 4 个文件扫描测试失败；相关测试串行复查为 10 passed /
  1 ignored，随后默认全仓重跑通过，包含 workspace 测试、Clippy、原生检查、生成物、
  Web SDK / 示例及架构检查。没有修改扫描代码或放宽门禁。
- 文档检查通过：101 个 Markdown、689 个本地链接；`git diff --check` 通过。
  门禁日志与串行复查输出一并保存在上述本机证据目录。

## 8. 2026-11-16 修复复核

15 项产品修改均已进入工作区。自动门禁为 `scripts/check-gpui.sh` exit 0、
GPUI **200 passed**；`scripts/check.sh` exit 0、workspace **882 passed / 6
ignored / 0 failed**。格式、Clippy、设计 / 本地化生成物、文档、Web SDK /
examples、原生探测和架构边界检查均通过；“Physical”只指最终隔离包上的真实
桌面操作。

| 项 | 实现与自动验证 | 真机结论 |
| --- | --- | --- |
| UI01 | 确认层使用 `.occlude()`；指针命中被遮罩阻断。五个模态焦点 handle 限定 Tab / Shift+Tab，普通字符、组合键和 IME 被截获；Esc 只取消模态，关闭后延迟恢复原焦点。`settings_close_prompt_blocks_pointer_and_traps_focus` 覆盖指针、Tab 循环、字符键、Space 和恢复焦点。 | Pass。v166 / v167 真机确认点击遮罩下开关不改值；Tab 三按钮回环、字符键无效、Space 关闭；守卫语言草稿、Esc、Done 和放弃均只走当前层。 |
| UI10 | Clear 和会删除数据的期限缩短先显示确认；取消不调用运行时，`loading` 与 stale confirmation 禁止重复提交。`clipboard_settings_expose_real_choices_and_confirm_destructive_changes`、retention 分类和 stale guard 均有回归。 | Partial / Pass for prompt：v167 隔离库有 2 条合成记录，Clear 与 0→1 天均出现不可恢复说明，取消后记录仍为 2。没有为验收执行真实批量 DELETE，重复提交由自动测试与 disabled/loading 路径覆盖。 |
| TEST01 | 新增 `sync_test_viewport` / `sync_test_viewport_to_desired` / `render_and_sync_test_viewport`，布局测试改用 `simulate_window_resize` 并断言 `window.viewport_size()`，不再把平台 bounds 当作布局尺寸。 | 自动 Pass；结论属于 headless viewport，不冒充原生 resize。 |
| UI02 | Launcher Notice 高度按文案估算并 clamp 到 44–96 px，内部可滚动；`launcher_notice_uses_bounded_height_and_never_overlaps_rows` 断言搜索头、Notice、首行顺序与最大高度。 | Pending for physical long failure：最终包未构造出长 Unsupported Tool / runtime error 生命周期；短提示布局由既有真机序列覆盖。 |
| UI03 | 渲染后的 `sync_launcher_window_after_render` 统一比较最新可见状态，尺寸差 >0.5 px 才触发；失败提示、loading→empty、1→5 行和 Scope 本地变化均补 resize。`notice_changes_converge...`、`launcher_resize_converges...` 使用真实 viewport。 | Pass at endpoint / Partial for frame capture：v167 日志按 72 / 136.5 / 472.5 pt 单次原子 resize；60 fps AVFoundation 录屏只产出静态重复帧，因此不声称逐帧无撕裂，只保留最终几何和原子调用证据。 |
| UI04 | 短组用两侧可收缩 gutter 居中；溢出时 gutter 归零，Scope 作为滚动容器直接子项，`scroll_to_item` 能揭示尾部选择。短组与长组可达性各有一条真实 viewport 回归。 | Partial：v167 Finder Current App 真机显示三个 Scope 居中；真实数据没有产生需要横向滚动的超长 Scope 组，尾部 reveal 由自动测试 Pass。 |
| UI05 | Shelf 顶层增加 28 px titlebar inset；状态图标、搜索框、可见 Esc 和结果内容都在该区域之后。Shelf 回归检查 360×360 header；任务页已有独立 titlebar 断言。 | Pass for layout / Partial for pointer hit：v167 480×520 与 AX 设为 360×360 的截图均无内容进入交通灯区域；原生 AX close button frame 为 16×16 且 AXPress 成功关闭。CUA 的窗口截图覆盖层遮住了真实圆形交通灯，因此不把 CUA 坐标点击写成三颗按钮的鼠标验收。 |
| UI06 | 可见 Esc 改为调用本地 `handle_key`，与键盘共用预览、Actions、删除确认、查询和关闭顺序。回归分别验证查询、Actions、删除层。 | Pass。v166 / v167 真机可见 Esc 清查询、关闭 Actions、取消删除确认，Shelf 窗口均保持；原生红色关闭按钮才结束窗口。 |
| UI07 | 内容区 `size_full + min_h(0) + overflow_y_scroll + scrollbar`，长输入使用 `whitespace_normal`；回归滚到 60 行正文末行并断言在 viewport 内。 | Pending：最终包没有可达的未知 NativeTool 入口，未注入协议事件；自动滚动 Pass。 |
| UI08 | Discard 从已保存 preferences 恢复 locale、模型、placeholder 并发出 `PreferencesApplied`；中英文双向共用同一路径。 | Pass。v166 与 v167 真机均为中文→英文预览→放弃，Settings 和 Launcher 回到中文。 |
| UI09 | 进入 Clipboard 分类读取 snapshot；UI 显示容量上限 / 1 / 7 / 30 / 90 天、恢复 / 5 / 15 / 60 分钟和真实 paused 状态。状态通道关闭会清 loading 并报错，保留期限菜单与暂停选项有回归。 | Pass。v166 / v167 冷进入显示真实 2 条记录；真机选择暂停 5 分钟后按钮变为“已暂停”，再次打开菜单并立即恢复。 |
| UI11 | 所有 Notice 调用携带 `NoticeKind`；Tasks copy 使用 Success，键盘帮助 Info，执行失败 Error，权限限制 Warning；无字符串推断。 | Partial：v167 Tasks “键盘操作”真机显示 Info 图标与中性提示。没有构造实际任务复制成功场景。 |
| UI12 | 材质环境读取 `accessibilityDisplayShouldReduceMotion()`，启动及 `AccessibilityDisplayChanged` 更新 `App::set_reduce_motion`；计划透传回归 Pass。 | Pending：未切换用户系统 Reduce Motion。该项需要明确授权并恢复系统原值，不能由自动测试冒充。 |
| UI13 | 初建窗口和每次 `launcher_frame` 共用 `constrained_window_extent`，四边保留 `SPACE_LG`；Settings render 去掉固定 720 px 覆盖，使用实际 native viewport + 内容滚动。约束函数和 648×480 Settings viewport 有回归。 | Partial：v167 Settings / Tasks 均确认 layer 0 普通窗口，Shelf 通过 AX 放到 360×360 后内容不越界；800×600 可用区域、Dock / menu / 多屏负坐标未在真实显示器矩阵执行。 |
| DEV01 | 无 live runtime 的 `request_live_update` 走现有 resize；真实输入回归从 5 条过滤到空结果后断言 bounds、desired size 和最终 viewport 一致。 | 自动 Pass；最终包使用正常 Core runtime，无法在同一进程复现“无 Core preview”，因此没有物理 Pass。 |

### 追加发现：玻璃亮度仍然随窗口高度变化

v166 最终包在 72 pt Global 空态与 473 pt 查询态之间，同一页头背景最大相差约
21 个 RGB 级。自动测试无法发现 AppKit `NSGlassEffectView` 随采样面积改变
整体亮度。v167 将 `contentScrim` 从 `#a0303438` 提高为 `#ed303438`：

| 构建 | 空态采样 | 查询态采样 | 最大通道差 |
| --- | --- | --- | --- |
| v166 | `RGB 93/97/100` | `RGB 75/79/79` | 21 |
| v167 | `RGB 57/60/65` | `RGB 53/57/59` | 6 |

v167 仍保留 7% 桌面透射和原生玻璃高光，但不再出现肉眼明显的空态 / 查询态
明暗跳变。采样点均为截图内 `(800, 60)` 页头无文字区，先将显示器
`U2790B` 配置文件转换到 sRGB，再读取同一顶部锚点下的像素。最终证据目录为
`build/acceptance-gpui-ui/v167-20261116-153939/`：

- App `0.14.0+167`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv167`，PID
  `58275`、父 PID `1`，隔离 `ANYWHERE_DATA_DIR`。
- CDHash `76f55f34f51c4a75a7e50d60cd41d4745d341e44`；可执行 SHA-256
  `1d8eedb2f418ba0bf96e5b357e412d6aea2c8313685dad11a1794f78ebeb67fd`。
- Launcher layer 3，失焦后隐藏；Shelf layer 3，切到 Finder 后保持；Settings /
  Running Tasks layer 0，失焦后保持普通窗口。Shelf 只能由明确关闭动作结束。
- Rust 日志无 `ERROR` / `WARN`、panic 或 `RefCell already borrowed`。
- 未覆盖：VoiceOver（按约定排除）、Reduce Motion 系统开关、800×600 显示器、
  浅色 / Reduce Transparency / Increase Contrast 完整矩阵、可注入的长 Notice
  与 Unsupported Tool、真实批量删除提交。
