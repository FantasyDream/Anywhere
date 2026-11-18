> 历史归档：以下记录保留原日期与当时的实施状态。当前契约以 [架构](../architecture.md)、[UI 设计](../ui-design.md)与[验收](../mvp-gpui.md)为准。

# GPUI 界面对齐与后台唤醒修复方案

状态：2026-11-13 已修复 v92 物理验收发现的 Current App 查询刷新、Settings
返回焦点、脏草稿确认框和文字对比度缺陷，并完成 Build 95 原序列复验。2026-11-14
又修复 Global 非空查询切回 Current App、App History 来源加载和其余 Settings
分类归属，并在 Build 98 完成真实桌面复验；随后修复 Application History
Settings 自锁、History / Clipboard / Tasks 页面浮层越界和浮层透底。Build 106
完成真实桌面复验，并新增两个生命周期修复：从隐藏 Launcher 经状态栏关闭
Settings 后恢复或重建查询流，以及状态栏 Running Tasks 窗口的置前、激活和
LaunchServices 兜底。Build 107 又拆分三类窗口策略：只有 Launcher 与
Clipboard Shelf 保持 floating / all-Spaces，Settings 与 Running Tasks 使用
normal level / 默认 Space / 原生交通灯；Shelf 不再套用 Launcher 的无边框样式，
并补齐空列表 Refresh 操作入口。随后修复新窗口创建 / 样式配置重入、系统关闭
后的空 handle 错误，以及 Launcher 根页 Esc 首次只切换入口的问题。Build 107
自动化和三类窗口的物理层级、失焦、关闭、最小化 / 恢复路径均已通过；
U00–U12 的其余物理与辅助功能矩阵仍未全部完成。Build 109 进一步恢复 Flutter
根页 Esc 链、Shelf 右下角定位 / 最小化恢复、迟到复制返回资格、Clipboard /
Application History 空态操作、History 固定 Refresh 和 Web Tool 宿主 Actions；
自动化门禁通过，但最终物理复验因验收机重新锁屏保持 Pending。
解锁后的 Build 113–121 序列又修复了 Shelf / Tasks 复用 panic、Settings 关闭
语义、Shelf 返回目标、Web Packages 普通窗口与导入审核布局，以及 Web 插件
把 `ENTRY_CONTEXT` 的占位 Session 0 冻结进 invocation 导致 `state.subscribe`
返回 `INACTIVE` 的缺陷。Build 121 将 Settings 根焦点挂到有 ID / role 的
Application Group，并完成 Launcher / Settings / WebView 物理复验；完整门禁为
GPUI 126 项、workspace 808 passed / 6 ignored / 0 failed。Build 123 又修复
WKWebView 在宿主本地监听器之前消费聚焦页面 Esc 的问题：Web 捕获阶段通过
当前 document 的受控 IPC 返回 Launcher，输入法组字仍优先；物理复验通过，
门禁为 GPUI 128 项、workspace 810 passed / 6 ignored / 0 failed。Build 126–129
继续修复共享滚动句柄被 `overflow_y_scrollbar()` 覆盖、长列表选中项 reveal、
Tasks 输出跟随 / Latest、任务启动后未自动打开 Tasks，以及复用窗口时未选中新
`execution_session_id` 的问题；Build 129 完成物理复验，GPUI 为 133 passed。
workspace 为 815 passed / 6 ignored / 0 failed，`scripts/check.sh` 通过。
Build 130–131 继续按 Flutter oracle 修复工具页边界与 History 生命周期：
Clipboard History / Shelf、Application History 与 Tasks 的 ↑↓ 在首尾改为
保持选中而非回绕；Application History 后台刷新保留选中 `object_key`，从 Action
重新打开时清空旧查询和来源行；其键盘 Esc 与页面 Esc 控件统一为先清空查询、
再返回。Tasks 多词筛选改为每个词独立命中任务或目录，筛选隐藏所选任务时保持
可见相邻项，新启动任务不被旧筛选遮住。当前自动化门禁为 GPUI 140 passed、
workspace 822 passed / 6 ignored / 0 failed；v131 已完成真实桌面复验并保持
Launcher / Shelf / Settings / Tasks 三类窗口契约。
Build 132–136 又修复了 Launcher 激活错误恢复最小化 Shelf 的生命周期缺陷：
App 激活只在 `activateWithOptions` 未请求成功时使用 LaunchServices 兜底，
异步激活完成后只恢复目标窗口的 key 所有权，不再重复激活整个应用。状态栏从
其他前台应用打开 Tasks 时会延迟确认 key；空 Tasks 的 `⌘K / ⌘L / ⌘. / ⌘R /
⌘F / ⌘⌥↑↓ / Esc` 也改为不依赖搜索框焦点。Build 136 完成物理复验，GPUI 为
143 passed，workspace 为 825 passed / 6 ignored / 0 failed；VoiceOver 已从本轮
验收范围排除。
Build 137–139 继续做源码审计：移除状态栏复用 Tasks 时的重复激活，并让 Tasks
裸 Esc 尊重 CharacterInput / 输入法组合。Build 139 自动化门禁通过，解锁后又
完成真实桌面复验：已有 Tasks 复用只发生一次激活并在 120 / 320 ms 取得 key；
拼音组合输入时 Esc 只取消候选；Tasks 最小化后 Launcher 热键不恢复它，状态栏
可显式 deminiaturize 并恢复键盘所有权。
Build 140–143 又修复了独立窗口构造时的隐藏输入焦点：Launcher 主搜索框不再由
共享 `build` 无条件聚焦，Shelf 只聚焦自己的搜索框，Tasks 从空页开始不安装
编辑器输入 handler；`⌘F` 仅聚焦已渲染的任务筛选框。Build 143 完成真实桌面
复验：空 Tasks 在 ABC 下直接输入或 `⌘F` 后输入均无候选窗与隐藏焦点；非空
Tasks 的已提交筛选在拼音 Esc 后保留，Launcher / Shelf / Settings / Tasks
三类窗口契约也全部保持。门禁为 GPUI 146 passed、workspace 828 passed /
6 ignored / 0 failed，最终候选为
`build/acceptance-gpui-ui/v143-20261115-091707/Anywhere-GPUI-UI-Alignment-v143.app`。
Build 144 继续按 Flutter `ExecutionSessionsView`、ToolPageShell 和 Clipboard
行交互对齐焦点：Tasks Esc / Enter / 行点击离开筛选焦点，Tasks 与 Shelf
`⌘F` 全选，Clipboard 行点击恢复搜索焦点，ToolPageShell 页面消费 Tab /
Shift+Tab。GPUI 152 passed、workspace 834 passed / 6 ignored / 0 failed。
构建后验收机锁屏，v144 新增物理矩阵保持 Pending，不能由自动化或 Build 143
截图代替。
Build 146 继续修复源码审计发现的窗口激活与 Tasks 操作差异：
`activateWithOptions` 返回 `true` 但应用仍非 active 时进入 LaunchServices
兜底；新建 Running Tasks 在样式配置后显式激活，不再只在复用已有窗口时激活。
Tasks 有详情时 `⌘K` 改为打开当前任务操作菜单（Copy command，结束后追加
Remove result），无详情时才打开 Tool options；详情标题栏改为单个更多操作
按钮。Tool options 的第二项恢复为 Flutter 的 Launcher（不再重复页脚 Keyboard
help）；任务 argv 按 Flutter 的 POSIX 引号规则显示和复制。GPUI 157 passed、
workspace 839 passed / 6 ignored / 0 failed。v146 构建与启动后验收机仍锁屏，
激活与 `⌘K` 物理矩阵继续保持 Pending。
Build 147–159 又修复了物理复验发现的 Tasks 裸键路由和 Clipboard Shelf 行点击
回焦：Tasks 的 Esc / ↑ / ↓ / Enter 改由原生 KeyDown monitor 在 AppKit 文本
输入上下文之前转发并排除 Shift / Command / Control / Option，Shelf 的搜索
焦点恢复延迟到鼠标事件结束之后。v154 补了 Shift 透传，v155 将其提取为可测试
函数并新增修饰键回归。v156 又按 Flutter `ClipboardShelfPage._key` 修复操作
菜单的命令透传与过期目标 Refresh。v156 物理复验发现跨窗口恢复 Launcher 会在
Tasks 更新内重入 AppKit move，产生 `RefCell already borrowed`；v159 在离开
实体更新后显示主窗口并恢复焦点。当前门禁为 GPUI 164、workspace 846 passed /
6 ignored / 0 failed；v159 已通过工具菜单、任务菜单、筛选 / Shift、Clipboard
Actions 和 Shelf 行点击的物理序列，且最终产物严格签名与身份已核对。
Build 160 又补了 Flutter 输入行为差异：按住 Enter / Return 的 auto-repeat
不得重复执行、复制 Shelf 条目或恢复 Tasks Tool options；Application History
不绑定 `⌘K` / `⌘R`，当前对象 Actions 只从行内 `…` 或右键打开。Clipboard
History / Shelf 命令与本地 Action 快捷键也不接受 auto-repeat。
Build 161–163 继续对照 Flutter `NativePreviewController` 和键盘焦点契约：`⌘Y` 在文件、
Clipboard History 和 Shelf 中统一为开关预览且不接受 repeat；预览中的 `⌘L`
关闭面板并返回查询，Left / Right / Home / End / Backspace / Delete 或 `⌘A`
先退出浏览态再继续文本编辑。文本输入路径产生的 repeat Space 会回滚该次空格，
不再重复开关预览；Space 只在查询编辑器拥有焦点时切换预览，Back 等按钮聚焦时
保留 Space / Enter 激活。Tasks 的 `⌘K` 也只处理首次 key-down，不会在按住时
把刚打开的任务菜单或 Tool options 再次关闭。Launcher 选中可浏览菜单且输入
为空时，`⌘K` 在没有对象 Actions 的情况下进入菜单，与右箭头共用入口。
Settings 快捷键录制中的 repeat 和 Esc 同样只处理首次 key-down。自动化门禁为
GPUI 181 passed、workspace 863 passed / 6 ignored / 0 failed。v160 的
00–19 物理证据属于较早构建，v164 新增物理序列因锁屏
继续保持 Pending。
最终候选为
`build/acceptance-gpui-ui/v164-20261116-092559/Anywhere-GPUI-UI-Alignment-v164.app`
（`0.14.0+164`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv164`），严格签名、
arm64、CDHash `ca0ad875...`、可执行文件 SHA-256 `ddf016c5...` 已核对；进程
PID `2112` 以隔离数据启动，日志无 ERROR / WARN。

Build 165 根据 v164 真实截图继续修复两处视觉缺陷：空态与输入后的
`contentScrim` 改为与 Flutter `surface/window` 一致的约 63% 中性灰
（`#a0303438`），不再使用 40% 纯黑遮罩；候选项变化引发的窗口高度更新改为
一次原子 `NSWindow.setFrame:display:`，同时完成尺寸和顶部锚点调整，并合并
快速连续 resize 请求。旧路径先用 `setContentSize` 改高度、再在下一主队列
回调移动原点，会在过渡帧产生高度与顶部位置不一致的撕裂观感。Build 165 的
自动门禁为 GPUI 182 passed、workspace 864 passed / 6 ignored / 0 failed；
解锁后的真实桌面记录空态 / 查询态页头玻璃仅约 2–4 个 RGB 级差，60 fps 录屏
抽帧未出现中间错位帧，72 / 136.5 / 472.5 px 高度均保持顶部 y=1128 原子切换。
最终候选为
`build/acceptance-gpui-ui/v165-20261116-100723/Anywhere-GPUI-UI-Alignment-v165.app`
（`0.14.0+165`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv165`，PID
`16941`），严格签名、arm64、Rust 日志零 ERROR / WARN；真实拼音输入出现过
一条 macOS `IMKCFRunLoopWakeUpReliable` stderr 提示，但输入与候选提交正常。
该轮只关闭这两项视觉缺陷及 Launcher Esc / 失焦复验；v164 的其他待物理项、
浅色外观和辅助功能矩阵仍按原边界保留。

Build 167 继续完成 [同轮 UI 审查](reviews/gpui-ui-review-2026-11-16.md)
的 15 项修复。最终门禁为 GPUI 200 passed、workspace 882 passed / 6 ignored /
0 failed。v166 真机截图暴露约 63% `contentScrim` 在 72 / 473 pt 高度下仍有约
21 个 RGB 级差，因此 v167 提高到约 93% 中性灰（`#ed303438`，Increase
Contrast 约 95%），最终同一页头采样差约 3–6 个通道级。最终候选为
`build/acceptance-gpui-ui/v167-20261116-153939/Anywhere-GPUI-UI-Alignment-v167.app`
（`0.14.0+167`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv167`，PID
`58275`，父 PID `1`），arm64、CDHash `76f55f34...`、可执行 SHA-256
`1d8eedb2...`。Launcher / Shelf 的 layer 3 与失焦契约、Settings / Tasks 的
layer 0 普通窗口契约已在最终包复验；Reduce Motion 系统切换、800×600 小屏和
长 Unsupported Tool 正文仍需专门物理矩阵。

原审查日期：2026-11-11；前轮实施复核日期：2026-11-12。

审查对象是 `codex/experiment-gpui-kit` 的 `c00dce4d680fe2e34ee2e179501a6f84c1845798`
及当时已有的 19 个未提交修改文件。旧界面参考固定为
`81c523cdb95031ac00cfcdc25360eefb48f85b21`，不使用随提交变化的 `HEAD^` 作为长期基线。
最初的审查阶段只读取源码、依赖实现和历史设计；后续实施复核已补充当前构建、
真实桌面输入和完整工程门禁，证据边界见“实施记录”和
[GPUI 验收](../mvp-gpui.md)。

补充范围：原生 Liquid Glass、文字呈现、插件默认控件和全部页面布局均属于本轮
对齐目标。材质与字体基准在逐页调整前完成，不作为最后可选的美化项。

当前契约继续以 [Launcher 交互](../launcher-interaction.md)、[UI 设计](../ui-design.md)
和 [GPUI 架构](gpui-migration.md) 为准；实际完成程度记录在
[GPUI 验收](../mvp-gpui.md)。本方案执行完后，将稳定规则并回这些专题，审查记录归档。

实现阅读顺序：[问题证据](#1-审查结论与证据) → [视觉目标](#2-界面目标与基线)
→ [行为与数据契约](#6-实现前必须固定的行为与数据契约) →
[小任务清单](#7-可直接执行的小任务清单) → [验证流程](#4-验证流程)。
第 3 节是任务说明，第 7 节是执行切片，不代表两套待实现功能。

## 2026-11-13 后续修复计划（当前执行入口）

本节保留 U00–U12 的完整实施范围。2026-11-13 已完成其中的四项具体缺陷：
Current App 非空查询改用 Core 过滤结果；Settings 进入时恢复原生 responder，
退出时在异步 window style 完成后恢复搜索焦点；脏草稿确认框改为不透明表层；
Launcher 增加由 token 驱动的可读性 scrim。2026-11-14 的 Build 96–98 进一步
修复显式 Current App 导航的迟到 Scope、Settings 来源加载、分类归属和页面文案。
Build 99–106 又修复 Application History 会话自锁、空对象动作面板、页面级
浮层越界 / 透明、Settings 退出后的查询流断连和状态栏 Tasks 窗口不可见；
Build 126–129 又修复所有共享长列表的 ScrollHandle、选中项 reveal、File Search
触底分页、Tasks 跟随 / Latest，以及进程任务自动打开 Tasks、复用窗口选中新
任务的行为差异。这些修复不改变 U00–U12 的待验收范围。
其余 U 项不得因这些具体缺陷通过而整体标为完成，后面的 T / W 清单仍保留前轮
设计与证据。当前物理复验记录见
[GPUI 验收](v0.14-gpui-development-records.md#2026-11-14-ui-对齐后续缺陷build-9698)。

### A. 本轮审查结果与范围

对照固定 Flutter 提交 `81c523c`，并读取本机 `Anywhere-flutter-3-50-memory`
工作树的原生材质实现。两者用途分别是交互契约和用户当前参考外观；实现前记录
本机 Flutter 工作树的 HEAD / dirty 状态及二进制身份，不能假定二者完全相同。

| 项目 | 当前证据 | 处理 |
| --- | --- | --- |
| Current App 默认 Scope | Flutter `_maybeSelectScope()` 与唤醒路径有默认 / 迟到选择；GPUI `reset_for_activation()` 清空选择，`apply_shared()` 直接复制 snapshot，没有对应的入口默认选择策略 | U02，代码差异已确认；用户已反馈未选中 |
| Tab 经过不可见状态 | GPUI `cycle_scope()` 最后一个 Scope 后进入 None，反向首项也进入 None；Flutter 直接按可见 scopes 取模；GPUI Scope 栏没有 None 按钮 | U03，确定的行为差异 |
| Tab 事件归属 | GPUI 有 Tab 分支，但在普通冒泡阶段处理，并使用 `(control || !modified)`；可见栏、对象动作层和混合修饰键需要统一审查 | U04，风险已定位，不能描述为完全未实现 Tab |
| 四角白边 | GPUI Titled + FullSizeContentView；Flutter Borderless；原生单变量实验切点 sRGB 205 → 163 | U01，证实窗口样式增加边缘亮度；整圈、真实 GPUI 尚需 A/B |
| 字体和白底可读性 | 已使用系统字体，GPUI 自绘字形不会自动成为 AppKit vibrant 文本 | U06，视觉 / 明暗外观复核，不先加背景补偿 |
| Current / Universal 遮挡 | 已有展开高度和行高修复，历史记录通过过部分中英文状态 | U05，复验当前构建；失败才修，不能当作从未修复 |
| 工具页、Settings、插件 | 前轮已有大量结构和交互修复；缺少统一当前构建对照矩阵 | U08–U10，逐项补证据，不预设全部存在 bug |

本轮不更改动作排名、Context 适用性、菜单授权和插件协议，不恢复 Flutter
运行时，不增加新的 Scope 类型或单独 Menu Scope，不引入第二套 UI 框架。

### B. 必须先冻结的交互结果

| 场景 | 必须达到的结果 |
| --- | --- |
| Current App 新会话、当前应用 Scope 已存在 | 默认选中 `is_current_app` 的稳定 ID；选中视觉、实际结果过滤、输入提示一致 |
| Current App Scope 迟到 | 仅本次入口仍待默认选择且用户未操作时补选一次，不抢走用户已选领域、查询或结果浏览 |
| 没有 Current App Scope / 没有可用动作 | 不伪造空 Scope；保持无过滤和明确空态，不擅自选其他领域首项 |
| Global 空首页 | `selected_scope=None` 合法；保持原版空首页与隐藏分类栏，不能为“有默认选中”强选 Current App |
| Global 输入后出现分类栏 | 无过滤仍可合法存在；首次 Tab 到首个可见 Scope，Shift+Tab 到末项 |
| 已选 Scope 时 Tab / Shift+Tab | 只在可见 Scope 中正反循环，不经过无按钮的 None；一项时保持，零项时不变 |
| 当前应用独一项、分类栏省略 | 保持输入区应用标签；Tab 不制造隐藏切换或从 Current App 跳到 Global |
| 鼠标点击 Scope | 与键盘使用同一选择入口；保留查询，搜索编辑器恢复焦点，选择项自动滚入可视区 |
| 新 Scope 追加 / 现有 Scope 消失 | 追加不改用户选择和已有顺序；当前选择消失时按 Flutter 回到 None 并退出失效菜单层，不自动抢选别的项 |
| 对象 Actions / 菜单 / 输出 / 绑定选择 | 不因 Tab 改变背后的 Scope；按各层原版键盘契约处理 |
| 隐藏恢复 / 新唤醒 | 15 秒恢复保留原 Scope；真正新会话重新应用入口默认；旧快照不能改新选择 |
| Web / Settings / IME | Web 自己处理 Tab；Settings 正常遍历控件；输入法组词优先，不触发后台导航 |

### C. 实现任务

#### U00 · 固定可重现基准与证据表

- 读取前述 Flutter 提交中的 `launcher_controller.dart`、`launcher_screen.dart`、
  `launcher_view_state.dart`、`launcher_navigation.dart` 和对应 widgets；原生部分
  读取 `Main.swift`、`WindowMaterial.swift`、`AppearanceHost.swift`。
- 记录 GPUI 源码 HEAD + 工作区 diff 身份、OS / SDK、外观、缩放、可用权限、
  App 路径 / Bundle ID / PID、数据目录与实际快捷键。使用独立数据和快捷键。
- 建立固定 fixture：当前应用 + Git + Folder、Scope 迟到 / 删除、无权限、
  Global 查询、长中文 / 英文、长路径、零 / 单 / 多项、两页对象 Actions。
- 每个 fixture 写清入口、按键 / 点击步骤、预期选中 ID 与结果 ID、截图、实际
  结果和 Pass / Fail / Pending；核心业务结果使用可控 backend，不依赖本机排名。
- 验收：每条报告能复现；旧截图和日志只作为基线，不充作新构建结果。

#### U01 · 原生窗口和玻璃背景

- 入口：`platform/mod.rs::configure_window()`、`platform/material.rs`、
  `platform/material/macos.rs`、`app.rs`。
- 先做真实 GPUI A/B，只将 Launcher 样式切为 Borderless + NonactivatingPanel；
  保持同一材质、尺寸、圆角、Root 透明度、系统外观与背景。记录四角和直边，
  不以单个像素或独立 AppKit 实验代替生产窗口验收。
- 保留 GPUIPanel / GPUIWindow 已提供的 key/main 能力和输入 responder 链；
  切换后显式恢复 GPUI editor。不要为了无边框再全局 swizzle NSWindow。
- Settings / Tasks 保留应有标题栏和交通灯；返回 Launcher 时恢复无边框；
  Shelf 按独立浮窗契约处理，不将所有窗口强行改成一种 styleMask。
- 所有产品窗口统一使用 tintColor 为 nil 的原生 Regular Glass，以系统通知
  作为视觉对照。GPUI 主题根画布、产品页、搜索头和未选中结果行透明，
  不使用 `contentScrim` / `pageScrim` 或补色结果卡片；普通交互高亮和局部
  控件表面保留，不额外绘制整窗描边、渐变或阴影。无边框
  不等于消除系统应有的细微高光。
- 外层 host / 双重圆角是否调整，须在窗口样式 A/B 后另做单变量验证；不能同时
  改三处而失去归因。主窗口白底角落不得出现额外亮色楔形或矩形底层露出。
- Reduce Transparency / Increase Contrast 和系统外观继续走原生适配；禁止读取
  未公开偏好键或自行模拟透明度滑块。平台中立计划只传系统状态和几何，macOS
  映射 AppKit，未来 Windows 映射 DWM / Mica / Acrylic；本轮不宣称完成 Windows。
- 验收：真热键、菜单栏、key=true、立即输入、中文 IME、失焦隐藏、进出 Settings、
  动态高度、多显示器；亮 / 暗 / 彩色背景各截图。任何关键焦点回归先修后推进。

#### U02 · Current App 默认选中与 Scope 状态一致性

- 入口：`launcher/view/navigation.rs`、`view/live_state.rs`、`view/mod.rs` 的
  snapshot 应用路径、`launcher/model.rs`、`launcher/snapshot_gate.rs`。
- 建立最小显式选择意图：入口默认待定、用户明确选择、恢复父工作区。绑定本次
  session / activation generation；不要将 pending 默认选择绑到会频繁变化的
  单个流订阅 generation，也不要只在 render 时伪造 selected 外观。
- Current App 新会话在权威 scopes 中查找 is_current_app；未到达时保留待定。
  按 Flutter 对照处理用户输入、手动 Scope、结果移动、菜单浏览取消自动选择；
  消费成功或会话关闭后清理待定。Global 新会话明确无默认应用过滤。
- 用户和默认选择都走同一个选择用例，更新实际查询所带 selected_scope。Core
  继续提供领域 / 候选权限，UI 只表达用户导航意图；不得在 Core 排名层补 UI 默认。
- 选择、过滤结果、提示和提交给 Core 的 scope 必须一致。若先显示本地选中态，
  待匹配快照到达前不能执行上一 Scope 的陈旧行；采用现有 revision / request gate
  拒绝迟到回包，不能让 apply_shared 无条件抹掉更新的本地选择意图。
- 测试：首次就绪、迟到、用户已输入 / 已切换后迟到、Scope 删除、同名不同 ID、
  旧 Session 回包、15 秒恢复、新会话重置；断言真实 scope ID 和结果集合。

#### U03 · 只循环可见 Scope，并统一点击路径

- 入口：`LauncherModel::cycle_scope()` / `cycle_scope_previous()`、
  `render_launcher()`、`scope_button()`、`reveal_selected_scope()`。
- 提取前后方向共用的可见项导航逻辑；N>0 时按索引取模。不再把 None 加入
  可见项循环；None 只作为合法的无过滤状态或显式回到 Global 的导航结果。
- 不要修改业务 Scope 排序；以 Core 提供的稳定 ID / 顺序为准。若以后显示过滤
  Scope，键盘与渲染必须读取同一可见项集合。
- 更新现有 `scope_cycle_returns_to_universal` 这类固化差异的测试，补齐零项、
  单项、多项、None 起步、反向、末首回环、长栏自动滚动与保留查询的回归。
- 验收：A→B→C→A；A 反向到 C；每一步均有唯一可见选中项，点击后继续 Tab
  从点击项出发，不能出现“按一次却全部未选中”。

#### U04 · 键盘路由、焦点与事件消费

- 入口：`view/input.rs`、`view/mod.rs` 的 capture / bubbling、输入组件绑定。
- 为路由建立明确的 Handled / PassThrough 结果；以录制、IME、原生预览、局部
  层、搜索导航、编辑器为顺序检查拥有者。避免 capture 和 bubbling 各执行一次。
- Tab 在编辑器默认焦点遍历之前消费，但仅对允许分类导航的当前搜索层生效；
  搜索页没有可切换分类时依原版消费而不移动到按钮。Settings / Web 维持各自规则。
- 显式允许 Tab、Shift+Tab、Ctrl+Tab、Ctrl+Shift+Tab；显式排除带 Command / Alt
  的组合，不能用 control=true 放行所有混合修饰键。测试真实 key event，不只调用
  handle_key 或 cycle_scope。对象层和模式选择器不得修改背后 Scope。
- Scope 切换保留文本、selection / 光标和输入焦点；输入法候选确认、组合态的
  Enter / Esc / Space / Tab 先交编辑器。方向键附加修饰键继续用于文本编辑。
- 验收：连续 10 次正反 Tab 一键一步；点击 chip 后仍可直接输入；中文组词不
  丢字、不执行；按钮聚焦时 Enter 不误执行候选，Web Tab 不触发宿主导航。

#### U05 · Current / Universal 选择器与导航层

- 入口：`view/navigation.rs`、`view/mod.rs` 的模式选择器和尺寸策略、输入同步。
- 复核空 72 px Launcher 展开、已有结果展开、中英文长标签、关闭与再次展开；
  使用现有两条 44 px 记录的布局，禁止绝对定位菜单超出真实 NSWindow 而被裁切。
- 保证图标 / 标题 / 说明 / check 各有自己的布局区域，固定控件不收缩，文本
  min_w=0；窗口 resize 完成后展示有效命中区，旧 resize 不覆盖新页面。
- 验证鼠标命中、Up / Down / Enter / Esc 和返回搜索焦点；模式与 Scope 分开：
  Tab 切 Scope 不切 Current / Universal，选择模式后再应用 U02 的入口策略。
- 模式切换时文本保留 / 清理遵循 Flutter 对应转换，不统一清空所有导航状态。

#### U06 · 字体、文字对比、圆角和基础控件

- 入口：`ui/theme.rs`、共享 components、`resources/design-tokens.json`；修改源
  token 后运行生成器，禁止手改 Rust / Web 投影。
- 对照系统字体解析、中文 fallback、字号、字重、行高、baseline、placeholder、
  disabled 和 selected。优先修复尺寸 / 错误颜色 / 二次 alpha，不加假粗描边或字影。
- 真实窗口比对亮 / 暗系统外观；必要时从 AppKit 公共语义颜色映射文本 token，
  在主线程按 effectiveAppearance 解析，并在外观变化时刷新所有工具窗口和 Web
  主题。保持 GPUI 文字能力边界，不宣称已经获得 AppKit 文本 vibrancy。
- 白底文字对比不足须另列原因及对照结果；不得重新给玻璃加 tint、整窗填充或
  深色外框。关键正文按 4.5:1、语义图标按 3:1 目标检查实际合成背景，截图上
  不满足时保持 Fail / Pending，不能用 token 的静态数值代替实测。
- 检查 Input 的额外 padding、Button 默认背景、Keycap、Tooltip、focus ring；
  控件选择态与键盘焦点分开，鼠标切分类不残留旧 General 焦点框。

#### U07 · Launcher、File Search 与对象 Actions

- 入口：`candidate_row.rs`、`page_shell.rs`、共享组件和 File Search page。
- 复验 72 px 页头、36 px 分类栏、56 px 候选行、32 px 图标、长标题和副标题
  截断；Actions / 快捷键区不收缩、不覆盖文字。顶部锚点跨 0 / 3 / 7 / 更多行稳定。
- 普通动作默认首项；支持预览的文件 / Clipboard 默认无对象选中，首次方向键
  选择第一项；不得将“Scope 默认选中”误实现为所有对象列表自动选中。
- 对象 Actions 仅在 Core 提供匹配目标时显示；普通应用不凭选中态显示 `… ⌘K`。
  返回恢复父查询、scope、选中稳定 ID、滚动和过滤；旧目标回包不能覆盖新对象。
- 文件 Tab / Shift+Tab 切类型并清理旧浏览资格；保留查询。验证分页末尾继续加载、
  失败重试、空状态、错误条和 Quick Look，不把普通输入空格当预览热键。

#### U08 · Clipboard / Shelf / Application History / Tasks

- 先按 fixture 复核，失败项单独最小修复，不重写已经对齐的页面。
- Application History 的当前对象操作只使用行内 `…` 和右键，不把 Launcher
  的 `⌘K` 对象动作绑定带入此页；也不增加 Flutter 没有的 `⌘R` 刷新绑定。
- Enter / Return 的 auto-repeat 不得重复执行或恢复页面；Tasks 的原生键路由
  必须携带 `NSEvent.isARepeat`，GPUI key-down 路径使用 `is_held`。Clipboard
  命令和本地 Action 快捷键同样只响应首次 key-down。
- 预览的 Space / `⌘Y` 只按首次 key-down 切换；Clipboard 文本输入路径产生的
  repeat Space 必须回滚。预览中的 `⌘L`、编辑导航和 `⌘A` 先关闭预览；Settings
  的快捷键录制及 Esc 不接受 repeat。
- 查询编辑器拥有焦点时 Space 才切换预览；页面按钮聚焦时 Space / Enter 激活
  按钮。Tasks 的 `⌘K` 菜单同样不接受 repeat，避免按住时打开后立即关闭。
- Clipboard History 保持普通工具页，查询 / 复制 / 预览 / 删除按原语义处理；
  检查删除是否被错误挂起到只在 Shelf 渲染的确认流程，复制反馈必须绑定原项。
- Shelf 保持独立置顶窗、compact 行和搜索 / copy / preview / delete / refresh；
  核对“Copy and return to app”、快捷编号及删除确认，明确原应用回跳失败状态。
- App History 保持单一选定来源，核对额外 source chip 是否符合基线；打开、双击、
  复制、对象操作及来源标题不挤占正文。不把 Launcher Usage 混成历史。
- History / Clipboard / Tasks 的页面级操作浮层统一从 `page_popup` 渲染，并
  以窗口坐标锚定、自动避让窗口边缘；不得在滚动内容内用 `absolute` 自行定位。
  对象动作失败必须撤销空面板，不能保留 loading 壳。
- Tasks 复验 252 px 侧栏、任务过滤 / 选择、详情流、Output / Diagnostics、
  Copy / Stop / Remove / Follow / Latest、空态、长路径和标题栏避让；后台状态
  更新不抢选择、不把其他任务输出放入当前详情，滚动输出不截为开头一段。

#### U09 · Settings 与保存边界

- 复核七分类、164 px 侧栏、顶部 Done / Save、dirty 提示、长说明和窄空间换行；
  分类滚动 / 键盘焦点与当前分类一致，Done 保留原版 outlined 样式。
- Action shortcuts 默认显示已绑定项，添加时才打开目录；全局 / 本地、上下文
  route、录制、冲突、移除、失效说明均可操作，不留下仅外观存在的控件。
- 检查目录授权、来源状态与错误、语言、开机启动、隐私入口、剪贴板策略的
  保存 / 失败反馈；外部授权面板不被失焦逻辑当作 Launcher 关闭。
- 标题栏关闭、Done、Esc、热键、模式切换统一走草稿关闭守卫；保存期间不重复
  提交，失败不清草稿、不提前返回；退出 Settings 恢复 U01 的 Launcher 样式和输入。

#### U10 · Web 工具与插件默认 UI

- 使用无业务 CSS 的默认控件 fixture 检查字体、按钮、表单、radio / switch、
  focus、tooltip、overflow、中文和错误页；透明 Web 根不得盖住原生玻璃。
- 宿主页头、WebView 内容与边缘只有一套布局；检查 resize 后 WebView frame、
  输入焦点、文字选择 / Copy / Paste、Tab 顺序和页面滚动。宿主不吞 Web 内部 Tab。
- 页面收到 effective appearance / accessibility 更新；旧 document 的 IPC、
  state 和 close 回包不能更新新页面。验证 hide/resume/final close 与 helper 回收，
  不为视觉修复扩大插件权限或执行边界。

#### U11 · 当前构建完整验收

先运行受影响 model / keyboard / snapshot / layout 测试，再运行项目门禁：

```sh
cargo fmt --all -- --check
scripts/check-gpui.sh
scripts/check.sh
git diff --check
scripts/build-macos.sh
```

必须测试实际元素事件和可控异步顺序，不能只改 model 测试使其配合新实现。
原生 style/key 能力用原生测试检查；材质、IME、真实焦点与热键仍需桌面观察。

| 编号 | 物理验收步骤 | 关键结果 |
| --- | --- | --- |
| P01 | 启动、菜单栏和两个真实热键呼出 | 后台启动、无 Dock / 初始抢焦点；输入立即可用 |
| P02 | Current App 即时 / 迟到多个 Scope | 默认当前应用，唯一高亮与结果一致 |
| P03 | 用户切领域后追加迟到 Scope | 选择不被默认策略覆盖 |
| P04 | Tab / Shift+Tab / Ctrl 别名 | 可见项循环，无 None 停顿，无焦点漂移 |
| P05 | 单 Scope 隐栏 / Global 空首页 | 没有隐形切换，也没有伪默认高亮 |
| P06 | 中文组词、粘贴、多行 Content、修饰方向键 | 不误导航 / 执行，光标和组合态正确 |
| P07 | 选择器、对象层、菜单、输出与 Esc 返回 | 只改变当前层，不修改背后 Scope |
| P08 | 白 / 暗 / 彩色背景，100% / Retina | 四角无异常白楔，文字可读，无重复描边 |
| P09 | 明暗外观及辅助功能设置变化 | 所有存活窗口 / Web 同步；不可测试则 Pending |
| P10 | 72 px 空态到多行、520 上限再返回 | 顶部固定、命中区和圆角正确、无旧 resize 覆盖 |
| P11 | Settings 反复进出，保存成功 / 失败 / 取消 | 草稿守卫正常，无边框和搜索焦点恢复 |
| P12 | 点击其他应用、Quick Look、授权面板 | 正常失焦隐藏，受保护原生交互不被误隐藏 |
| P13 | 15 秒内恢复与超过 15 秒新唤醒 | 保留 / 新默认严格按会话规则，旧回包无效 |
| P14 | 文件 / 剪贴板 / 历史 / Tasks / Web 长文本 | 标题、状态、正文与操作无重叠，页面语义一致 |
| P15 | 多屏、缩放变化、键盘焦点导航 | 窗口可达，选中 / 焦点 / 可访问名称一致 |

每行附 fixture、实际输入序列、当前构建身份、观察和截图 / 日志路径。当前没有
权限或系统环境时保留 Pending，不自行修改用户系统设置以补齐记录。

#### U12 · 模块收口与交付

- 仅将本轮形成的 Scope 选择策略、键盘路由和窗口样式策略收至现有职责模块；
  不借本轮重构 Core 排名或重新拆整仓。渲染函数读取展示状态，不发默认选择请求。
- 更新 `launcher-interaction.md` 的默认 Scope / 正反循环 / 事件归属和当前
  事件式失焦实际实现；`ui-design.md` 维护材质与布局；MVP 维护当前验收状态。
- 删除确实无调用的旧 Scope / overlay 分支；先查调用，保留序列化与插件契约。
- 交付源码 diff、自动门禁、签名包、P01–P15 结果与已知差异；只有实现和物理证据
  均具备的项目标记完成，不以“已打包 / 注册热键 / 安装材质”宣称 UI 全部一致。

### D. 开发切片与推进顺序

1. U00 → U01：独立提交窗口样式修复及原生焦点证据；若无边框焦点失败先处理。
2. U02 → U03 → U04：默认 Scope、可见项循环、事件路由分三个可审查切片；完成
   这一组后先验收 P02–P07，不将所有页面美化捆在同一改动中。
3. U05 → U06 → U07：选择器、文字基线和主列表；以同一基础组件向工具页推广。
4. U08 → U09 → U10：逐页执行差异矩阵；每页只修复复现项并留下对应回归。
5. U11 → U12：当前源码完整门禁、包装与真实桌验，再回收文档及确实过期的分支。

每个切片先读调用路径和现有测试，重现 → 最小修复 → 定向检查 → 对应物理步骤；
不会通过先隐藏 Scope、禁用 Tab、改排名或叠加深色背景来掩盖缺陷。

## 实施记录（2026-11-12）

本轮已把方案中的原生窗口、会话、顺序、设置、插件样式和高频页面代码落到
`codex/experiment-gpui-kit` 工作树。当前证据分三级：

- Code Done：源码路径和定向回归存在。GPUI 当前 90 项测试全部通过。
- Physical Smoke：已在隔离 Bundle ID、数据目录和真实系统热键下观察。
- Pending / Partial：仍需重复桌面矩阵、系统辅助功能或逐状态截图确认。

已完成的 P0 修复：

- Launcher 使用原生 `NSPanel` / nonactivating-panel 启动形态。呼出期间保持
  Accessory activation policy，通过不带 `ActivateAllWindows` 的
  `NSRunningApplication.activateWithOptions` 激活，并在主循环后确认 key；
  只有激活请求未生效时才使用 LaunchServices 兜底，避免重复激活整个应用或
  恢复无关最小化窗口。`LSUIElement` 启动仍无窗口、无 Dock、不抢前台。
- 样式只在确实变化时重设；重设后重新定位 `GPUIView` first responder。GPUI
  editor 在主循环后再次聚焦，日志同时记录 `isKeyWindow`、应用 active、
  first responder 和 editor focus。
- `LiveLauncher::suspend_current_now` 在从 GPUI 主线程跨入 Core 前显式进入
  Tokio runtime，修复隐藏时 `tokio::spawn` 的 reactor panic。
- 窗口高度变化由已提交后的主队列重新锚定，首次呼出把顶部固定在当前鼠标屏
  可用高度的 80%；后续 resize 复用该锚点。原 Flutter 的合法查询结果上限是
  7 行；真实桌面复测了 72 → 137 → 473 → 72，520 只作为布局 clamp 的纯几何
  回归，不能通过伪造第 8 行宣称物理通过。

已完成的产品和页面实现：

- 新 Session、suspend / resume、SnapshotGate、ActionResult 展示代次和过期回调
  资格已接入；发现到期重采，保留工作区恢复。
- Settings 保留七个稳定分类、164 点侧栏、草稿关闭和单次保存；Context 来源状态、
  目录授权、剪贴板策略、应用历史、Feature、语言和登录项均有 GPUI 路径。
- Settings 的共享开关和策略行按原卡片层级分组；应用历史增加来源搜索与仅启用
  来源筛选，路径和适配目录使用上下布局，搜索引擎显示本地化名称而不是内部值。
- Action shortcut 已支持全局/本地切换、录制、移除、冲突校验、历史格式迁移和
  多 Action context-route 草稿；全局 binding 与两个 Launcher 热键一起事务式
  注册和回滚。
- Launcher / File Search / Clipboard / History / Tasks / Web / Plugin Detail 使用
  任务尺寸表各自的窗口几何；列表选中、真实复制反馈、Task detail 身份和图标
  ticket / 128 项 LRU 均已实现。
- Application History 已接回 Core 对象动作；行内 Actions 和右键绑定当前条目
  与请求票据，旧选择、旧 revision、旧来源和旧 120 ms 查询结果不能改写新页面。
  该页面不绑定 `⌘K` / `⌘R`；历史 Session 在退出、超期和重新打开时撤销，
  短期隐藏则取消当前读取、保留 Session 供 15 秒内恢复刷新。设置页可直接打开
  `application-history-sources` 自定义适配目录。
- GPUI 根画布 alpha 为 0，macOS 26+ 通过 `NSGlassEffectView.contentView`
  承载原内容；旧系统使用 HUD，Reduce Transparency 使用不透明 fallback。
  以 [当前材质契约](../ui-design.md#材质与可读性) 为准；桌面透色仍待物理验收。
- Web 宿主在挂载和辅助功能变化时设置透明 canvas；零业务样式控件 fixture、
  Web Text / JSON 响应布局和共享控件状态使用生成后的 SDK CSS。
- 候选行恢复截断副标题、快捷键和对象操作的 tooltip；渲染代码已按
  `view/pages/`、`candidate_row.rs`、`page_shell.rs` 与 `settings.rs` 拆分，
  主协调文件由约 9,700 行降到 6,489 行。
- Settings 的两个启动器快捷键恢复为原版单按钮录制控件，AX 尺寸实测
  98×26，显示 `⌃⌥⇧O` / `⌃⌥⇧P`，使用原版透明底色和 divider 描边；内部
  稳定字符串只在保存层使用。该控件保留 Button 的 Tab / Space / Enter
  键盘语义，记录结束时恢复同一控件焦点。

当前桌验证据：

- 隔离进程类型为 `UIElement`；正常启动时 CoreGraphics 可见窗口数为 0，前台
  应用保持不变。
- 全程 Accessory 的 live Core 包在真实 `⌃⌥⇧U` / `⌃⌥⇧I` 事件下显示窗口。
  采样批记录 31 次 `show_started`，31 次均以 `visible=true key=true` 完成，
  20 次隐藏；interrupt 期间出现的一次 60 ms `key=false` 在 280 ms 重试后
  记录 `key_activation_recovered`，没有失败告警、reactor panic 或 AppKit
  重入错误。
- 状态栏两个搜索项、Running Tasks、Settings 和 Quit 均已物理点击；搜索与
  Settings 窗口直接成为 key，first responder 为 `GPUIView`，活动 editor 日志
  为 `focused=true`。Build 106 的真实 Running Tasks 入口先创建 920×600
  窗口，再在菜单关闭后置前并进入键盘链；Tool options 可选，空态标题栏、
  页脚和内容均未越界。
- 真实中文输入法在 Launcher 显示候选窗，Space 提交“你好”后 editor 为
  `safari你好`；这是原生 AppKit / GPUI 输入链，不是模拟 widget 输入。
- 设置页无草稿时真实 Esc 可返回 Launcher。实现使用 AppKit 本地事件监听，在
  文本框输入上下文之前处理无 marked text 的 Esc；存在输入法组字时事件回到
  输入法。清洁关闭已物理验证，脏草稿完整物理矩阵仍待补。
- 实测 `72 → 137 → 473 → 72` 时窗口原点保持 `(956,312)`；顶部没有跳动。
  473 是原 Flutter 基线的 7 行查询高度；8 行不是合法模型状态。520 点上限仍由
  合成布局 clamp 回归覆盖，当前未标为物理通过。
- Finder 成为前台约 220 ms 后主 Launcher 隐藏；独立 Clipboard Shelf 在
  Finder 前台保持 480×520 可见，点击后 Esc 可关闭。
- Build 123 在 WebView 聚焦的真实 Text Pad 中输入后按 Esc，日志收到当前
  document 的 `web_view_escape_requested`，920×600 Web Tool 回到 648×137
  Launcher 且保留查询；同包复验 Settings / Tasks 为 normal layer、Shelf
  失焦保持并需显式关闭。
- macOS 27.2.1 上截取到 648×72、floating layer 的原生半透明材质窗口。材质在
  彩色桌面上可见，但 Reduce Transparency、Increase Contrast 和逐状态视觉差异
  仍需按 N13 / N16 重测。
- 2026-11-12 最终候选 v16 的隔离包以 `open -n` 启动后进程类型为
  `UIElement`，启动窗口数 0；系统级 `⌃⌥⇧O` 事件完成 Current App 呼出，
  日志记录约 54 ms 内应用 active、窗口 key、`GPUIView` first responder 和
  editor focus。事件由 `GlobalHotKeyEvent::set_event_handler` 推入异步队列，
  不再轮询热键全局接收器。
- 从状态栏进入 Settings 的真实序列暴露了 GPUI 更新期间同步应用 AppKit
  window style 导致的 `RefCell already borrowed`。实现改为在更新返回后异步
  应用 Settings / Launcher 原生样式。v16 物理完成了真实状态栏点击、Settings
  激活、Tab/Shift+Tab 到两个 98×26 录制器、Space 开始录制、`⌃⌥⇧K` 输入、
  Esc 脏草稿提示和“放弃修改并关闭”；配置文件保持原值，且没有 borrow、
  configuration-failed、key-activation-failed 或原生重入日志。

仍未完成：

- 脏 Settings 的保存成功 / 保存失败物理分支，以及设置内输入法组字期间 Esc
  的独立桌面用例。
- 520 clamp 的合法产品状态和所有页面截图矩阵；不再把原 Flutter 不存在的
  第 8 行当作验收前提。
- Reduce Transparency / Increase Contrast / Reduce Motion、macOS 15。
- WebView helper 最终回收、Web suspend / resume 原稿恢复的完整物理序列。
- 操作、查询、Web 和设置控制器仍集中在协调文件中；页面渲染已经独立，但后续
  行为变动仍应优先落到对应页面模型或 host adapter。

## 1. 审查结论与证据

当前迁移已经建立 GPUI 页面与后台应用身份，但还没有完成 Spotlight 式生命周期，
也没有达到原 UI 的视觉与交互一致性。优先修复激活、隐藏和异步资格，再调整界面。

以下 P1 表示本轮必须先修复；P2 表示后续逐页对齐项。“代码确认”不等于物理复现。

### R1 · P1 · 首次真正唤出可能就在重复激活已使用的 Session

代码确认，调用链如下：

1. `host/runtime/mod.rs::LiveLauncher::open_at` 只调用一次 `begin_activation()`，将
   `session_id` 固定在实例中。
2. `launcher/view/mod.rs` 构造主页面时，即使窗口隐藏，也调用
   `live.activate(Environment)`。
3. 菜单栏和热键都进入 `activate_from_hotkey`，再次调用 `live.activate(input_mode)`。
4. `application/activate.rs::activate_session_in_mode` 拒绝已经
   `activation_started` 的会话，也要求当前 revision 为 0；Host 将拒绝映射成
   `Launcher session was superseded`。

因此正常启动激活完成后，再唤出不会建立正确的新会话；模式切换和 Current App
环境重采也不能依赖这个调用。不能仅修窗口显示后就认定唤醒完成。

修复：服务启动与 Launcher 会话激活分开；新激活预留新 Session，恢复使用现有
`resume_session`；同时更新 `SnapshotGate` 与请求 revision。参见 T02。

### R2 · P1 · 隐藏没有接上 Core 的挂起和 15 秒恢复契约

代码确认：`prepare_hide` 清理导航、Quick Look 和 Web surface，但 GPUI Host 没有
调用 Core 的 `suspend_session`、`resume_session` 或 `deactivate`。热键还会无条件
将页面改成 Launcher、清除 Scope 和对象动作。

影响：隐藏不等于 Core 撤销未开始执行的资格；工具页和查询状态也没有完整的
15 秒恢复链。Core 已有这些接口及顺序测试，无需另造一套业务生命周期。

修复：隐藏先撤销展示代次和执行资格，保留允许恢复的展示状态；过期或空首页走
新激活。已开始的后台任务按原任务契约继续，不因隐藏被误杀。参见 T02、T03。

### R3 · P1 · 旧动作结果可以影响新的窗口

代码确认：`activate_selection → live.execute → poll` 是真实可达路径。
`poll` 的 State 分支检查 `stream_generation`，但 ActionResult 分支直接
`handle_action_result`、隐藏窗口并打开目标页，没有同等资格检查。

复现场景：开始一个较慢动作 → 修改查询或重新唤出 → 旧动作返回
`close_window=true` 或工具目的地。旧结果可以关闭或改写新界面。

修复：在所有 UI 发布和原生副作用前校验 Session、展示代次和请求身份；让验证后的
结果一次性产生可执行的 UI effect。不得在校验闭包外继续无条件 `hide_main_window`。
`poll_follow` 的重复结果处理也应统一，不能保留潜在的旁路。参见 T03。

### R4 · P1 · 顶部锚点公式错误，多显示器选择也不一致

代码确认：`platform/mod.rs::position_launcher_window` 使用：

```text
y = visible.maxY - window.height * 0.20 - window.height
```

因此窗口顶部是 `visible.maxY - window.height * 0.20`，高度从 72 变成 520 时，
顶部会移动 89.6 个逻辑点。异步重新定位避免了部分重入，却没有修正这个公式。
屏幕优先取 `window.screen()`，也可能停在上次显示器。

旧实现首次显示按鼠标所在屏幕，顶部留白是可用屏幕高度的 20%；随后 resize
保持原来的窗口顶部。修复应区分“唤出定位”与“原地变高”，并处理可用区域边界。
参见 T05。

### R5 · P1 · 空首页和工具页面尺寸偏离原设计

代码确认：`desired_window_size` 对空 rows 总加 64 点 body 和分隔线，缺少原
`hasPanel` 条件。没有 Scope、预览或通知时也会算出 136.5，而非只有输入栏的 72。
现有测试的 178.5 还包含 preview 提示，不能把它当作正常首页基准。

Clipboard、Application History、Web 等被统一成 648×560；Tasks 是 900×560，
并使用不可缩放的主窗口。详见第 2 节。修复不能只改高度断言，应同时纠正 body
可见条件和实际布局。参见 T05、T07、T10—T12。

### R6 · P1 · 唤出焦点尚无当前版本完整证据

当前调用先聚焦 GPUI 输入，再执行原生 show。此前会话日志曾出现
`visible=true key=false`，但本次未复现，单次即时读数也不能证明窗口永久不能聚焦。

必须纠正一个推断：锁定依赖 `gpui-fast-macos 0.1.14` 的窗口子类已经将
`canBecomeKeyWindow`、`canBecomeMainWindow` 返回 YES。不能仅因 Borderless
就机械改成普通带标题窗口。

另一个代码风险是失焦每 120 ms 轮询，两次命中后隐藏。需要纠正此前审查结论：
虽然 `dismiss_on_focus_loss` 仅排除 Settings 和 Shelf，platform 外层的
`focus_loss_should_dismiss` 已在 Quick Look 可见时暂停隐藏，不能认定没有预览豁免。
现有豁免不区分内部焦点转移与真正切到外部应用，可能让 Launcher 随预览一直留在
屏幕上；菜单、授权面板也缺少显式的焦点所有权。具体系统行为仍需桌验。

修复：统一显示顺序、确认 key 后恢复当前编辑器，使用窗口通知和可取消宽限表达
失焦；不要通过不断抢回焦点掩盖问题。参见 T04。

### R7 · P1 · 保存快捷键或语言后，菜单栏标签仍是旧值

代码确认：`app.rs` 只在启动时创建 `StatusMenuLabels`。Preferences 事件会更新
语言、偏好和热键注册，产品壳却没有对应的菜单更新事件。

修复：成功应用设置后通知 ProductShell，主线程更新菜单。注册失败时区分已保存
配置和实际生效的快捷键，不能在菜单上宣称失败的快捷键已可用。参见 T06。

### R8 · P1 · Settings 的结构和关闭流程都未对齐

代码确认：原 7 分类变成 8 分类，Language 被单独移出 General；侧栏从 164 变成
184，标题、关闭入口、卡片及常驻 Save / Done 布局改变。快捷键主要用文本输入，
Context 来源状态等区域简化。它不是仅调字号就能恢复的页面。

另外，未保存关闭提示中的 Save 调用普通 `save_settings`，Preferences 回包只把
`settings_close_pending` 清零，没有继续完成关闭；需要区分 Save 与 SaveAndClose。
参见 T09。

### R9 · P2 · 样式、交互和可访问性只完成了部分迁移

- `render_page_shell` 被 7 个页面复用，抹平了页面专属的标题、查询和操作区域。
- Candidate Row 主要用 `on_click` 选择；旧版按下即选择，并限定对象区双击，
  当前行没有完整迁移这些语义。
- 一些 aria 属性已经存在，不能说完全没有 Accessibility；但应用代码未找到
  对应旧 AppTooltip 的实现，图标与截断文本仍需逐项补齐提示和状态语义。
- `Load more`、`Pause 15 min`、设置分类等直接写双语分支，没有走本地化源文件。
- 根视图统一 glass surface 和 0.7 边框，旧 panel 使用 0.5。Settings 应有独立
  window 语义；当前 surface 与 window token 数值相同，不能把换函数名当作视觉修复。
- 审查时旧 macOS 材质支持 Glass / HUD，但应用尚未接入 Reduce Transparency
  检测与不透明 fallback；当前实现已改为透明 GPUI 主题和页面画布、无补色蒙版、
  原生 Regular Glass / HUD 材质和明确的 Opaque 回退，仍需两种系统状态下的最终
  桌面矩阵。
- `view.rs` 达 7,493 行，渲染、异步事件、焦点和全部工具混在一起，增加逐页对齐成本。

参见 T07—T13。

### R10 · P1 · 插件透明 WebView 被默认不透明画布覆盖

代码确认：`launcher/view/mod.rs` 创建 WebView 时设置 `with_transparent(true)`，
但 `web_tool.rs::web_theme_css` 将 html / body / 常见应用根节点的背景强制设为
`var(--aw-web-canvas, var(--aw-fallback-surface))`。当前产品没有设置
`--aw-web-canvas` 的路径，默认命中不透明 fallback。

旧版 `web_document_theme.dart` 与 `web_view_appearance.dart` 会在确认原生透明
支持后更新这个变量。迁移保留了 CSS，缺少对应的画布状态交接，所以只打开
WebView transparent 选项不能恢复插件的玻璃背景。参见第 2.6 节和 T12。

### R11 · P1 · 键盘规则缺少方向、焦点和层级条件

代码确认：`handle_key` 只接收 command / modified，丢失 Shift 的独立信息；
Tab 分支总调用 `cycle_scope()` 或 `next_filter()`，无法实现 Shift+Tab 反向。
分支也没有明确检查编辑器焦点、对象 Actions / 输出层和实际可见分类。

已修复：Launcher 根 Esc 现在实现现行契约的 Content → Current App →
空白 Global → 隐藏，普通查询先清空；空输入菜单的左右键也没有对应分支。根 capture 先处理
预览与 ⌘K / ⌘Y，后面的 bubble 才检查 IME，必须统一审查事件所有者，避免
控件 Enter、文本编辑和中文组词落入宿主命令。

另有一处基准冲突：当前文档要求文件页首次方向键选择首项，现有测试却断言
首次 Up 选择末项。本轮按现行产品契约执行并在回归中注明旧断言为何修改；不能
默默保留冲突，或只把测试改成符合现状。详见第 6.2 节、T07、T08。

### R12 · P1 · 剪贴板查询与操作回包会破坏当前列表

代码确认：`request_clipboard_search` 不推进独立查询代次，多个搜索共享
`stream_generation`；Clipboard 回包也不带 query / request ID，较慢的旧查询
可以覆盖较新的结果。`copy_clipboard`、`delete_clipboard` 成功后总返回空查询
snapshot，输入框仍有文字时结果却变成全部记录。

`copy_selected_clipboard` 在真正复制完成前就显示 Copied。复制失败或用户已换
条目时，反馈会失真。应将操作结果与当前查询刷新分开，并绑定条目及 operation ID。
Application History / 来源切换也应使用相同的页面请求身份原则，但不能仅凭共享
事件处理器断言其 Core 内部同样存在乱序缺陷。详见第 6.3 节、T03、T10。

### R13 · P1 · 长列表键盘导航缺少可见性和分页交接

代码确认：结果列表虽可滚动，当前持有并主动滚动的句柄仅覆盖 Scope 和文件类型；
文件 / 剪贴板选择方法只修改 index，没有将选中行滚入视口。文件末尾 Down 会
循环选择，`load_more_files` 只接在鼠标按钮上，没有实现契约中的末尾按需追加。

列表还会逐帧 clone 并构建全部已有行；它是需要测量和优化的成本，不能直接称为
已复现性能故障。修复先保证选中 ID、可见区域与分页语义，再对长列表使用锁定
GPUI 版本提供的普通虚拟列表。详见第 6.5 节、T07、T08、T10—T12。

### R14 · P2 · 图标请求换代后可能永久停留在 pending

代码确认：`request_application_icons` 先写入 `application_icons_requested`，
`poll_application_icon` 遇到旧 stream generation 时直接丢弃事件；移除 pending
标记只在被接受的 `apply_auxiliary_event` 内执行。输入变化后迟到的图标响应被
拒绝，原请求标记仍保留，相同候选再次出现时就不再发请求。

当前图标缓存也没有显式容量或并发上限。应让请求有独立 ticket，完成、失败、
取消都结算其自身 pending；旧完成不能移除新请求的标记。缓存使用应用身份 / 图标
版本，不以跨 Session 变化的候选 ID 作为唯一内容身份。详见第 6.6 节、T13。

### R15 · P1 · Tasks 详情可能与选中任务不一致，输出只显示开头

代码确认：点击任务 A、B 可以发出两个同代请求；`TasksPageModel::apply_detail`
仅检查返回任务是否存在于整个 rows，而不检查是否仍为当前选中任务。A 晚于 B
返回时会覆盖详情。选中项消失后，`apply_rows` 回退首项也可能留下旧详情。

`render_tasks` 把 stdout 接在 stderr 前面并 `take(400)`，所以长任务只展示
拼接后的前 400 行，Latest 无法靠滚动修复这个数据截断。当前 UI 也没有持续刷新
运行任务详情的完整链路。修复需要匹配选中任务和请求版本、使用已有有界输出契约，
清楚区分流，不能虚构 stdout / stderr 的真实交错时序。详见 T12 和第 6.5 节。

### R16 · P1 · Settings 可以被热键绕过关闭守卫，保存也未完整串行化

代码确认：热键直接进入 `activate_from_hotkey` 并把 screen 改为 Launcher，
没有先通过设置草稿决策；状态栏 Quit 也直接退出。原版 toggle 已对热键执行
`closeSettings` 守卫。Done / Esc 做提示不足以覆盖全部离开入口。

普通 Save 按钮虽然在 saving 时禁用，⌘S 和关闭提示的 Save 仍可重复调用
`save_settings`。Host 并发 blocking save 共用 `settings.json.tmp`，可能相互
覆盖或 rename 失败；Core 设置在文件持久化前已应用，失败路径还可能造成生效值
与磁盘 / 草稿不同。HotKeyController 回滚失败也没有向上完整报告。

应建立单一设置保存操作与待完成离开意图，明确可回滚配置及登录项等外部效果的
部分失败。详见第 6.4 节、T06、T09。

### R17 · P1 · Web 的隐藏保留被当作最终销毁

代码确认：`prepare_hide → close_web_surface` 会最终关闭 Rust document，清空
handle 并释放 WebView；FrontendRuntime 的 `close_web_document` 会清除当前
binding。这不只是视觉隐藏，插件未提交的编辑内容也随 WebView 销毁。

Core / SDK 已有保留文档恢复机制：suspend 撤权，resume 为保留原生视图签发新
epoch / runtime，origin 保持，SDK 重连不重复 onLaunch。Host 需要接上这条链，
而不是只记录一个旧 handle 再创建新网页。详见第 6.7 节、T02、T12。

## 2. 界面目标与基线

### 2.1 视觉原则

保持原设计的布局、密度、主要操作和层级；使用 GPUI 普通组件与共享 tokens。
不引入第二套页面框架，不把 Flutter 带回产品或构建链。原设计 tokens 指定 dark，
本轮不新增产品浅色主题；在系统浅色、深色环境下均验证深色面板的可读性。

数值是逻辑点，截图另记录 backing scale，不能直接把 Retina 像素当布局数值。
原源码决定基准，截图用于验证实际绘制；当前明确的产品契约优先于更早历史行为。

### 2.2 页面窗口表

| 页面 | 当前审查值 | 本轮目标 | 依据 |
| --- | --- | --- | --- |
| Launcher | 宽 648，初建 72，但空 body 会撑高 | 648，72–520，按状态展开 | 原 LauncherLayout / 当前契约 |
| Content 输入 | 普通页头增加 24 | 96 点输入区，再按 panel 扩展 | 原 LauncherLayout |
| File Search | 648×600 | 648×600，保留 | 原 LauncherLayout |
| Settings | 648×508 | 保留尺寸；侧栏 164、顶部 inset 42 | 原 settings_screen |
| Clipboard History | 648×560 | 648×600 | 原 clipboard_history_session |
| Application History | 648×560 | 760×520 | 原 application_history/session |
| Web Tool | 648×560 | 920×600 | 原 web_tool_session |
| 普通 Tool / Plugin Detail | 多数 648×560 | 默认 648×480，逐页核对特殊声明 | 原 ToolPageSession |
| Clipboard Shelf | 480×520 | 保持 480×520，独立置顶窗口 | 当前 AGENTS / UI 契约 |
| Running Tasks | 主窗口 900×560，不可缩放 | 独立 GPUI 窗口 920×600，最小 760×420 | 原 TasksWindowController / 当前生命周期契约 |

Shelf 的更早原生定位代码还存在 420×560 的历史值；本轮遵循已接受的 480×520，
不把旧默认值覆盖回去。窗口超出小屏可用区域时允许收缩并滚动，不能越出屏幕。

### 2.3 高频组件几何表

| 组件 | 目标 | 交互要求 |
| --- | --- | --- |
| 外层面板 | 圆角 18；所有窗口用未着色 Regular Glass；页面 / 搜索头 / 普通结果行透明，无背景蒙版和窗口描边 | 圆角裁切、原生高光与阴影不重复叠加；保留系统随高度和背景改变浓度的行为 |
| SearchHeader | 高 72；水平 padding 20；左控件 36；gap 12 | 无表单边框；模式、返回、Esc 提示与原版一致 |
| 搜索 / 模式文字 | 21 / 10；模式 weight 500 | 保持文本基线，长应用名截断有提示 |
| ScopeBar | 高 36；水平 padding 12；chip 高 26、圆角 10 | 单 Current App 隐藏栏；多项可滚动且选中项可见 |
| CandidateRow | 外高 56；上下各 2；内层 52；水平 padding 10 | 按下选择；对象区双击；操作按钮阻止冒泡 |
| 候选图文 | 图标 32；间距 12；标题 15；副标题 11.5 | 选中态不再叠 hover；disabled 不执行 |
| ResultHeading | 24 | 普通查询不增加结果数量标题 |
| Notice / EmptyState | Notice 44；有 panel 才使用空结果 body 64 | 空首页不显示“没有结果”占位 |
| Keycap / Tooltip | 使用现有对应 tokens | 中英文名称与快捷键语义完整 |

结构数值中尚未存在于 tokens 的重复值，应补充源 JSON 并生成 Rust 投影。
不是每个页面常量都需要抽成新组件；只抽取重复且语义一致的结构。

### 2.4 原生 Liquid Glass 的实现目标

结论（2026-11-17）：采用透明 GPUI 主题和页面画布、无补色蒙版与单一 AppKit
Regular 材质。macOS 26+ 使用
`NSGlassEffectView.contentView` 承载原内容，旧系统使用 HUD。原先强制 HUD、
将 glass 作为 sibling 的方案已被纠正；窗口级截图的黑色也不足以证明系统
不支持原生玻璃。当前实现和证据边界见 [材质契约](../ui-design.md#材质与可读性)。

Apple 将 NSGlassEffectView 标为 macOS 26+；集成必须按系统能力选择分支，继续
支持产品的 macOS 15 最低版本。[API 可用性](https://developer.apple.com/documentation/appkit/nsglasseffectview)

| 条件 | 原生材质 | 内容表面 |
| --- | --- | --- |
| macOS 26+，普通显示设置 | 所有窗口使用 NSGlassEffectView regular、nil tint；contentView 承载内容 | 主题 / 页面 / 搜索头 / 普通结果行透明，不叠加背景蒙版 |
| macOS 15–25 | NSVisualEffectView HUD，behindWindow / active | 同一透明画布，交互高亮与局部控件表面保留 |
| Reduce Transparency 或材质创建失败 | 明确的不透明 fallback | 原 fallbackSurface，仍保持文字、边框与布局 |

Reduce Transparency 是产品的明确回退策略；系统原生玻璃本身也会响应辅助功能
设置，不应把本方案的不透明策略误写成系统唯一行为。Increase Contrast 加强文字
与控件边界，不提高背景或结果行补色 alpha；Reduce Motion 关闭
额外布局动画，不覆盖用户系统设置。

实施原则：

1. 新增薄 `platform/material.rs`，保存每个窗口的材质 host、模式与生命周期。
   通过 objc2 调用 AppKit；当前依赖已有相关绑定，实施时核对 SDK 和运行时
   availability，不升级最低系统或直接调用旧系统不存在的类。
2. macOS 26+ 层次为 window → NSGlassEffectView → contentView → GPUIView；
   旧系统为 window → NSVisualEffectView → 原内容。保留 Metal layer、raw handle、
   delegate、输入协议和 WebView 子树。
3. 先恢复原内容并解除旧 glass.contentView，再切换材质；不把 glass 放在 sibling
   背景层，不把此前尝试的黑色截图当成最终结论。
4. GPUI window token alpha 为 0，停用内建 Blurred，保证每个窗口只有一个背景
   材质。Web 同步移除 canvas 底色和 backdrop-filter。
5. 窗口阴影、材质圆角、透明容器裁切和 WebView 原生裁切分工明确。禁止再以
   “降噪”为名给根画布添加深色遮罩；需要对比度的列表、字段和按钮只绘制局部
   surface。
6. 包装必须先做真实 GPUI / WebView 最小集成验证：原始句柄不变、resize 正确、
   命中坐标正确、IME 候选位置正确、辅助功能角色可达。若公开宿主接口不足，
   通过受版本控制的小型依赖 patch / 上游接口解决，禁止手改 Cargo registry 缓存。
7. 不给每行候选、按钮、输入框再加独立玻璃。Launcher 作为瞬时浮层使用一块
   glass；Settings 卡片、Tasks 输出和插件编辑器使用稳定的内容表面。
   这是保持原产品形态的选择，不把“所有内容都玻璃化”扩成全新设计。

布局原型必须同时包含一段 GPUI 文字与一个 Web 输入框。原生材质能承载自定义
内容，不代表 Metal 绘制的文字自动具备原生 NSLabel 的全部动态文字处理；文字
颜色及可读性仍由 GPUI / Web tokens 和实际合成效果共同验证。
相关集成方式见 [Apple AppKit 示例](https://developer.apple.com/videos/play/wwdc2025/310/)。

### 2.5 字体与文字效果

已核对：现有生成器 `rust_font_family()` 已正确将 Web 的系统字体栈映射为
GPUI `.SystemUIFont`，等宽体映射为 `Menlo`。这不是一个需要换字体的缺陷。
原 Flutter 用 `.AppleSystemUIFont`；平台 fallback 负责中文和 Emoji。

旧 `AppStyle.textShadows` 是空列表；本轮不增加文字发光、描边或投影。清晰度应
来自字体度量、字重、颜色和背景层级。Web 保留 `-webkit-font-smoothing: auto`，
不以强制细灰度抗锯齿作为统一外观的方法。

| 语义角色 | 字号 / 字重 | 行高与应用位置 |
| --- | --- | --- |
| 搜索 / 页面大标题 | 21 / 400 | 原 body 行高 1.25；72 点页头内对齐 |
| Content 输入 | 17 / 400 | 原 body 行高 1.25；两行区和光标独立验收 |
| 模式小标题 | 10 / 500 | 原 caption 行高 1.3 |
| 候选标题 | 15 / 600 | 对齐 macOS 通知卡的半粗系统字重；不能由默认控件额外撑高 |
| 候选副标题 | 11.5 / 400 | 单行截断，与标题间距 2 |
| 正文 / 设置值 | 13 / 400 | 1.25，长说明按内容换行 |
| 分节标题 | 15 / 600 | 与原卡片标题一致，不随全局默认 h1 放大 |
| 说明文字 | 11 / 400 | 1.3，secondary / muted 按语义使用 |
| 控件标签 | 11 / 400，主要动作 500 | 控件高 24；保持按钮内垂直居中 |
| Keycap | 10.5 / 500 | 1.1，独立圆角和内距 |
| 代码 / 任务输出 | Menlo 11 / 400 | 1.55；保留空白、长行滚动 |

实现要求：补齐必要的文本角色映射，在 GPUI 和 CSS 中显式应用字体、字重、行高、
颜色和间距；不用一条 root font 就认为所有 Input / Button 内部文本已继承。
检查 placeholder、selected、disabled、链接、error、光标、选择底色及中英文混排。

使用固定样本文本检查 `Anywhere 中文 Aa 0123 → ⌘K`、中英文长文件名、Emoji、
中英文标点和代码。分别记录实际字体解析、文字边界、基线与截断点；缺字必须修复，
不因 GPUI / WebKit 栅格算法不同要求逐像素完全一致，也不接受明显变细、变粗或
上下错位。1× / 2× 与实际使用的缩放比例都要检查。

### 2.6 插件默认样式与透明画布

继续使用已存在的链路：`resources/design-tokens.json` → GPUI Rust tokens / SDK
tokens.css；`resources/web-controls.css` → SDK controls.css；GPUI Web host 自动
注入。修改源文件并运行生成器，不分别维护两套颜色，也不要求插件作者引入新 UI 库。

必须补齐的宿主画布流程：

1. Web 文档初始安全底色为 fallback；原生 WebView 挂载并确认当前 surface 可透明
   后，宿主对当前 document 设置 `--aw-web-canvas: transparent`，显示下方同一
   原生材质。不能依赖编辑器 focus 回调才执行。
2. 设置透明的原生 WebView 和 CSS 根背景是两件事，必须同时成立。html、body、
   实际应用根节点保持一致；React / Vue 挂载替换后也不重新铺黑底。
3. Reduce Transparency 时宿主底层已经是不透明 surface；Web 能透明时可继续
   透出这个底层。原生 WebView 不支持透明或合成失败时才给网页使用 fallback，
   避免将半透明 surface 在 html / body / app 各画一次。
4. 显示、重载、替换、恢复文档以及系统设置变化都更新画布。更新携带 document
   handle / generation，迟到完成不能改写新页面，继续保持现有 IPC 权限边界。
5. `!important` 仅用于确由宿主拥有的根画布规则；内部控件继续使用 `:where()`
   低优先级。插件可以定义工作区布局和局部内容表面，不能被全局背景重置全部抹平。
6. 检查加载首帧、overscroll、透明原生子视图、圆角和错误页，避免白闪、黑块或
   背景色变化。WKWebView 的 underPageBackgroundColor 只涉及页面背后的颜色，
   不是单独设置它就能完成整条透明链路。
   [WebKit 属性说明](https://developer.apple.com/documentation/webkit/wkwebview/underpagebackgroundcolor)

默认控件覆盖表：

| 类型 | 默认样式与行为 |
| --- | --- |
| Button / icon button | normal、primary、hover、pressed、focus-visible、disabled，统一高度 / 圆角 / 字重 |
| Input / textarea / select | 默认 field 表面；搜索才使用无边框变体；placeholder、readonly、invalid、焦点和选择色 |
| Checkbox / radio / switch | 可见选中与键盘焦点；原生语义和 label 关联；不靠颜色单独区分 |
| Label / heading / help / error | 对应第 2.5 节角色；长提示换行；错误在字段附近持久显示 |
| Card / divider / toolbar / tabs | 共享 tokens；工具栏可换行，选中 tab 与候选选中层级一致 |
| Menu / tooltip / dialog | 稳定可读背景、层级、焦点返回；菜单不被 WebView 原生裁切吃掉 |
| Code / editor / output / kbd | 系统等宽体与相同 Keycap；选区、滚动条、长行及粘贴正常 |

默认 CSS 不能凭样式赋予行为：现有语义 HTML 优先，自定义 switch / dialog 需要
补键盘与 ARIA。不要给所有 input type 都套文本框几何，checkbox / range 等单独
验证；未知插件私有控件的 shadow DOM 不承诺自动被全局 CSS 覆盖。

建立一个“零业务样式”的 HTML fixture 展示上述控件，确认插件不写 CSS 也可用；
再验收 Web Text 和 JSON Workspace。JSON 的 `.json-workspace { min-width: 620px }`
与共享 560 点窄布局规则需要共同处理，不能因示例覆盖导致窄屏 / 放大文字时丢操作。
宿主的标题 / 返回 / 权限状态只出现一次，插件内容不重复画应用标题栏。

### 2.7 全页面布局审核表

窗口尺寸不是布局验收的全部。每页必须交付区域结构、滚动所有者、操作位置和
加载 / 空 / 错误 / 禁用 / 权限 / 长文本状态，不仅截取一个正常满列表画面。

| 页面 | 区域与层级 | 重点检查 |
| --- | --- | --- |
| Launcher / Content | 页头 → 条件 Scope → 条件 heading / notice → 结果 | 空首页只有输入；长应用名、两行内容、菜单与对象动作不挤压 |
| File Search | 搜索头 → 类型过滤 → 查询状态 → 列表 | 过滤横滚、连续结果稳定；列表滚动而页头不滚 |
| Clipboard History | 标题 / 查询 → 记录或暂停状态 → 条目列表 | 不内嵌完整预览；复制、预览、更多定位稳定 |
| Clipboard Shelf | 紧凑独立页头 / 查询 → 列表 → 必要操作 | 480 宽可达，复制反馈不让行宽和按钮跳动 |
| Application History | 来源与查询 → 状态 / 刷新 → 对象列表 | 760 宽有完整路径层级；来源配置不混在结果中 |
| Settings 七页 | 164 侧栏 → 页标题 / Done → 分节卡片 | 右侧独立滚动；共享标签 / 控件对齐；草稿提示不遮住操作 |
| Tasks | 任务导航 → 状态 / 操作 → 输出 | 独立窗口；输出自滚；停止、复制、Latest 始终可达 |
| Web Tool | 单一宿主页头 → 插件工具栏与内容 | 透明画布；编辑区填剩余高度；无双重 padding / 标题 / blur |
| Web Packages / Plugin Detail | 标题 → 包 / 权限或详情 → 操作 | 安装、启停、错误、说明完整；长权限列表可滚动 |
| 菜单 / Tooltip / Dialog / Notice | 跟随所属页的浮层或状态区 | 不裁切、不越屏；遮挡时焦点仍正确；提示不是第二套窗口壳 |

每页记录 `header / toolbar / body / status / overlay` 的实际边界，应用共享
padding、gap、圆角和控件度量；页面仅保留其语义特例。内容最小尺寸不足时先明确
换行 / 单列 / 滚动策略，不靠缩小字体或隐藏主要动作。插件内容拥有自己的滚动，
宿主不能再给整个 WebView 套一层竞争滚动条。

## 3. 实施任务

执行顺序：T01 → T02 → T03 → T04 → T05 → T06 → T07 → T08 → T09 →
T10 → T11 → T12 → T13 → T14。每个任务完成后先跑定向检查，再推进下一项。
一个任务可以分多个小提交，但不要把整个 view.rs 机械搬家与行为修复混成一次修改。

### T01 · 固定视觉基准和可复现数据

输入：上述旧提交及已有可运行 Flutter 实例。只将其用于参考，不修改或停止用户
日常实例，不重新接回 Flutter 构建。

步骤：

1. 用 `git show 81c523cdb95031ac00cfcdc25360eefb48f85b21:apps/flutter/...`
   阅读 launcher_layout、launcher_components、candidate_row、settings_screen、
   settings_components、tool_page_shell、app_style、WindowMaterial 和窗口协调器。
2. 建立合成 fixture：固定应用、文件、菜单、disabled 项、长标题、中文和英文。
   不用真实剪贴板正文或私人历史作为可提交的截图数据。
3. 为每个状态记录：页面、模式、Scope、查询、选中行、loading、notice、输出、
   窗口逻辑尺寸、scale、系统外观、Reduce Transparency。
4. 截取原版与 GPUI 对照。原实例不能方便重放的状态先以源码几何为基准，标记
   “截图待采集”，不得用生成 mockup 冒充原版截图。
5. 图片和日志放在忽略的 `build/acceptance-gpui-ui/`，文档只记录矩阵与结果，
   不建立指向忽略文件的仓库 Markdown 链接。

状态清单：空首页；单 Current App；多 Scope；有 / 无查询的 3、5、7 行；
loading / 空结果 / 权限 notice；菜单子级；对象动作；Content 两行；输出；
File Search 各类型；History / Shelf；Settings 七页；Tasks；Web 和 Plugin Detail。

完成条件：每个后续页面任务都有基准来源和状态 ID。截图缺失项如实保留，不阻塞
已经明确的会话、定位和尺寸修复。

### T02 · 修复激活、隐藏与恢复

修改入口：`host/runtime/mod.rs`、`app.rs`、`launcher/view/mod.rs`、
`launcher/snapshot_gate.rs`。
小型生命周期模型可放 `launcher/window/lifecycle.rs`；AppKit 实现仍留 platform。

步骤：

1. 移除隐藏页面构造时的会话激活；后台服务可以初始化，Context 按真实激活采集。
2. 给激活请求明确分类：新 Current App、新 Universal、恢复、打开设置。
   两个系统入口统一调用产品壳，禁止各自重复拼接状态清理和 show。
3. 新激活调用 `begin_activation`，在异步结果返回前将新 SessionId 绑定到
   Host、展示模型和 SnapshotGate；重置 request revision，旧请求持有旧身份。
4. 不仅把 `session_id` 换成一个可变数值：所有查询、执行、图标、文件操作和订阅
   必须在提交时捕获明确 SessionId，防止旧任务读取到新会话。
5. 隐藏路径统一调用 `suspend_session`；确认撤权步骤执行后清理并隐藏。不要仅
   fire-and-forget 一个未开始轮询的 Future，然后认为已经完成撤权。
6. 有查询或工具页且未过期时用 `resume_session`；普通唤醒先恢复保留工作区，不因
   触发的是另一种 Launcher 热键就强行丢弃它。恢复失败、空首页、配置失效或明确
   全局 Action 入口才创建新会话。恢复时保留页面、查询、Scope、选中对象及滚动位置。
7. 恢复后的候选资格仍由 Core 重算；候选 ID 不存在时按原页面规则回退选择。
   Web 保留原生视图和编辑内容，通过 Rust 保留绑定换取新 handle / epoch，不能
   继续用旧授权，也不能恢复时无条件销毁重建网页。详见第 6.7 节。
8. 在切换应用激活前冻结来源应用身份。若当前异步 capture 会晚于 show，增加最小
   的 capture-ready 交接，让身份采集完成后再激活 Anywhere；慢来源读取继续异步。
   不等待全部 Context 结果才显示，也不让 Current App 采集到 Anywhere 自身。

模型分开表达：主窗口 `Hidden / Showing / Visible / Hiding`，当前页及
SessionId / presentation generation / hidden_at；Shelf、Tasks 是独立窗口句柄，
Quick Look 是带 owner 的临时 surface，不能把它们做成互斥的主窗口枚举状态。

必要测试：启动不调用 activate；首次和连续唤出；两种模式切换；14 秒恢复与超过
15 秒重建；空首页重采；恢复工具页；隐藏后尚未开始的执行失效；旧 Session 回包拒绝。
时间边界读现有 retention policy，使用可控时钟，避免测试真实等待 15 秒。

### T03 · 收紧异步 UI effect 资格

依赖 T02。修改 `poll`、`poll_follow`、ActionResult 路由及延迟 resize / focus 回调。

步骤：

1. 为一次展示保留独立 presentation generation，不用 query revision 代替窗口身份。
2. 统一 ActionResult 接收流程；先校验再产生 Hide / OpenDestination / Notice 等
   effect。对原生操作和第二次 GPUI 更新也要在紧邻执行处复验。
3. Hide、替换页面、替换 Web 文档时撤销对应回调资格；拒绝迟到错误覆盖新页面。
4. Settings 保存等应用级操作使用自己的 operation ID，不因搜索流变化丢失成功
   回包；也不能把一条全局 generation 判断粗暴套给所有不相关操作。
5. 已完成动作的真实结果和 Usage 记录仍由 Core 正常处理，只拒绝过期的 UI effect。

同时按第 6.3 节拆分 Clipboard / History 查询、Tasks detail 和 Settings save 的
请求身份。图标请求的完成结算不依赖查询结果是否仍可显示；异步错误也按拥有者路由。

必要测试用可控 gate：阻塞旧动作回包 → 建立新展示 → 释放回包；分别断言新窗口
未隐藏、目的地未替换、输入未清空。再覆盖隐藏后迟到 snapshot / preview / focus。

### T04 · 完成原生焦点链与失焦通知

依赖 T02、T03。修改 `app.rs`、`platform/mod.rs` 及当前页输入焦点路由。

步骤：

1. 记录激活 source、generation、App active、窗口 class / ID、canBecomeKey、
   isKey、NSApp.keyWindow、firstResponder。日志不记录输入或剪贴板正文。
2. 在 GPUI update 可变借用退出后执行 AppKit 配置和 show；避免 resize / move
   同步回调重入。禁止通过原生调用覆盖 GPUI 自己的窗口 delegate。
3. 显示顺序：冻结来源 → 配置样式和尺寸 → 定位 → 激活 / order front / make key
   → 下一主循环恢复当前页编辑器。若已有有效 IME composition，不重复重置输入。
4. 在 didBecomeKey 或受代次保护的回调中确认成功；约 250 ms 后仍失败则记录
   明确失败状态。不得用无限激活重试抢回用户已经切走的焦点。
5. 用 GPUI 现有窗口激活观察接口或 AppKit 通知替换 120 ms 持续轮询；失焦后
   使用一个可取消的约 200 ms 宽限任务。成为 key、隐藏或换代即取消旧任务。
6. 区分外部失焦与自己拥有的菜单、Quick Look、文件授权面板、WebView 首响应者
   转移。Quick Look 的豁免只覆盖内部转移，切换到 Finder 仍应关闭预览并隐藏。
7. Settings 不自动隐藏；Shelf / Tasks 焦点与主窗口分别管理，主窗口隐藏不能
   关闭它们。离开 Shelf 不等于关闭 Shelf，不设置“任意辅助窗口存在就永不隐藏”。
8. observer 和 timer 随拥有者释放；确认改热键、开关设置、重复唤醒不会累计订阅。

验收：真实菜单栏和热键各连续唤出 20 次；每次 key 成功并可立即输入；切到 Finder
后约 200–300 ms 隐藏；中文组词、候选窗、Quick Look 往返不被错误清理。
自动化只验证状态和取消顺序，物理项目必须另行记录。

### T05 · 恢复尺寸策略与固定顶部

依赖 T02。修改 `desired_window_size`、resize 路由和原生 placement；提取纯几何函数。

步骤：

1. 明确 hasPanel / hasControls / hasNotice / hasResultHeading；空 rows 本身
   不代表应出现空结果 panel。输出高度按原文本行数和上限计算。
2. preview 模式提示是开发辅助信息，不能进入产品空首页的黄金尺寸断言。
3. 由一个 LayoutSpec 同时驱动窗口高度和渲染可见区，避免两套条件失配。
4. 用第 2 节页面表替换通用 560，Tasks 的独立窗口在 T12 接上。
5. 每次唤出选择鼠标所在 NSScreen，取 visibleFrame；首次定位采用：
   `top = visible.maxY - visible.height * 0.20`，`y = max(visible.minY, top - h)`。
   x 居中并 clamp 到可用区域，宽高先约束到可用范围。
6. 可见期间 resize 保持这次展示的顶部和屏幕，不能鼠标一移动就把窗口搬到另一屏。
   屏幕移除或可用区域变更时重新 clamp。边界装不下时优先保证完整可达并滚动。
7. 合并同一帧的重复 resize；延迟定位携带窗口 generation，旧任务不能移动新页面。
   Settings 采用自身居中策略，不通过通用 show 再次覆盖成 Launcher 顶部定位。

必要测试：空首页、Content、loading / notice、输出、3/5/7 行、单 / 多 Scope；
负坐标副屏、Dock 在左右、不同 scale、小屏、拔屏；同一顶部的 72 → 520 → 72。
从原状态计算期望值，不照抄现有 215 / 402.5 等带 fixture 条件的断言。

### T06 · 状态栏与实际生效设置同步

依赖 T02。修改 `LauncherEvent`、`ProductShell`、`StatusItemHost`、热键注册结果。

步骤：保存完成事件携带 locale 与生效快捷键 → 产品壳在主线程更新已有菜单项 →
保持 status item、target、action 所有权不重复安装。快捷键注册回滚或失败时显示
明确错误，菜单保持实际生效值；保存后切换中英文无需重启。

必要测试：改两个快捷键、改语言、注册失败回滚、连续保存，标签和实际注册一致。

### T07 · 原生材质、字体基线与 Launcher 组件

依赖 T01、T05。修改 tokens 源、`ui/theme.rs`，从 view 中提取已存在的组件。

先完成 T07a：按第 2.4 节实现 `platform/material.rs` 的最小原生玻璃集成，
包含 GPUI 和 WebView 透明内容、隐藏 / 显示 / resize、HUD / opaque fallback。
这一阶段先验证焦点、IME、坐标与材质真实效果，不能拖到页面全部调整完才尝试。

再完成 T07b：按第 2.5 节建立文本样本与角色映射，核对普通文字、Input、Button、
代码与 Web 文字；保持当前正确的字体 family 投影。材质和字体基线成立后执行
下列 T07c 页面组件步骤。

T07c 先完成第 6.2 节的键盘路由及第 6.5 节的父工作区保存，再调整组件视觉；
工具栏 Button 的 Enter 不能执行背后的候选，Shift+Tab、Esc 链和菜单左右键有
独立回归。选中行可见性与点击 / 键盘 / 增量结果三种滚动意图区分处理。

步骤：

1. 先迁移 SearchHeader、ScopeBar、CandidateRow、Keycap、Notice、EmptyState；
   每次迁移删除原重复实现，保持事件回调连回现有模型。
2. 按第 2 节对齐几何、文字基线、图标、selected / hover / disabled / focus。
   保留现有已对齐数值，不重造普通 Button、Switch、Input。
3. 搜索输入仅做需要的无边框变体；普通设置字段保留表单边界。
4. 候选按下选择，只有对象区域处理双击；预览、主操作、更多按钮各自阻止冒泡。
   disabled 禁止触发动作；是否允许选择按原页面语义处理。
5. 补 tooltip、截断文本完整内容和辅助功能名称 / 选中 / disabled 状态。
   普通搜索不加数量标题；Current App 的菜单仍处于同一 Scope。
6. 用 T01 的状态截图对照，先修结构、密度和层级，再处理栅格细差。

完成条件：Launcher 所有基准状态齐全；普通页头、Scope、行内容主边界偏差目标
不超过 1 个逻辑点，无裁切或额外栏位。抗锯齿及背景模糊不做整图零差异硬断言。

### T08 · File Search 对齐

依赖 T07。保留 648×600；专用搜索头、类型过滤、查询进度 / 数量区、连续列表。
恢复过滤 chip 横向滚动、选中项可见、没有隐式首选行、浏览与输入态区分。

对象操作保留行内位置，移除通用 shell 造成的多余返回或固定宽度输入；不恢复
固定预览栏，Space / ⌘Y 继续使用 Quick Look。类型、过滤、追加排序仍由已有
Core 管理，UI 不重新排序。

末尾 Down 和滚动接近末尾使用同一分页请求入口，按 session / revision / cursor
去重；有下一页时不先回绕首项。查询变化撤销旧分页资格，失败保留当前页并允许
重试。鼠标加载不改变选择或滚动位置；键盘追加后按原导航意图选择并滚入视口。

验收：空查询、有结果、无结果、增量扫描、错误 / 部分结果、长路径；Down / Up、
Enter、⌘K、中文输入中的 Space、Esc；切过滤后无旧结果混入。

### T09 · Settings 按七分类重做

依赖 T06、T07。修改 settings/model 和页面渲染，保留 host/preferences 业务路径。

步骤：

1. 恢复七分类，Language 回到 General；侧栏 164、顶部 inset 42，右侧独立
   page toolbar、Done、卡片、分节和可滚动内容。分类使用稳定 ID，不拿翻译作 ID。
2. General：语言、登录项及既有选项；Shortcuts：两种全局热键和 Action shortcut
   catalog，使用录制、清除、取消和冲突反馈，避免要求用户手写底层快捷键字符串。
3. Context：来源开关、状态、隐私设置入口、排除应用；Search：目录授权、引擎、
   隐藏项和排除项；Clipboard：启用、暂停、保留期及既有策略。
4. App History：来源状态、默认检测、自定义路径和动作开关；Features：保留
   Feature catalog 的能力与状态，不能退化成只有名称和总开关。
5. 原共享卡片内 toggle row 不各自套 field 背景；输入框才使用 field 样式。
6. 明确 `SaveOnly / SaveAndClose / Discard / Cancel`。异步保存成功才完成待关闭
   意图；失败保留草稿、错误和窗口。Done、Esc、系统关闭共用此决策。
7. 切换分类恢复其滚动和焦点；录制快捷键时暂停宿主相关命令路由，结束后恢复。

按第 6.4 节统一热键、菜单切模式、Quit 和系统关闭的草稿守卫；保存串行化覆盖
⌘S、提示内 Save 及按钮，不能只禁用一处按钮。重入关闭合并成一个待完成意图。

必要测试：七分类可达、两种语言、草稿跨分类保持、无修改关闭、保存后关闭、保存
失败不关闭、放弃、取消、权限失败、快捷键冲突。每分类保留一组物理截图。

### T10 · Clipboard History 与 Shelf

依赖 T07、T08。History 恢复 648×600 的标题 / 查询 / 状态 / 列表布局；Shelf
保持独立 480×520，提供自己的搜索、复制反馈、预览、删除和刷新。

复用条目呈现，不复用窗口寿命。主窗口隐藏不销毁 Shelf；重复打开聚焦已有 Shelf。
用文本、图片、文件列表、超长内容 fixture 检查行密度和 Quick Look。复制后焦点
返回和窗口是否保留遵循各自入口契约，不全局硬编码一种行为。

必要测试：独立选择和查询；主窗口与 Shelf 并存；Esc / 系统关闭清理；删除后选择；
迟到预览不重新弹出。物理检查窗口置顶、复制反馈及预览关闭后的编辑器焦点。

修复 R12：查询有独立 request ID，回包携带查询身份；复制 / 删除只报告对应操作
的结果，刷新重新使用当前查询。Copied 只在复制成功后出现，绑定条目 ID；失败
保留查询和选择，显示可恢复错误。History 与 Shelf 的请求代次不能互相作废。

### T11 · Application History 和普通工具页

依赖 T07。历史页恢复 760×520；来源、查询、状态、刷新与对象动作按原页面布局。
来源配置留在 Settings。长标题和路径截断，说明文字保持稳定行高。

Plugin Detail / 未知工具 / Web Packages 分别核对旧 ToolPageSession，默认
648×480；为页面组成专属 header / body / actions，逐步取消不合适的
`render_page_shell` 调用，不给通用 shell 再增加一串页面判断。

必要测试：无来源、不可用来源、切来源、查询无结果、刷新、对象动作、错误可恢复；
不混淆应用历史、Usage、剪贴板历史。

### T12 · Tasks 与 Web Tool

依赖 T03、T07。两者分别实现，不放到一份超大提交。

Tasks：在 ProductShell 中增加独立 GPUI 窗口句柄，默认 920×600、最小 760×420，
允许缩放。重复打开恢复窗口与选中任务；主窗口隐藏不影响它。任务列表、输出、
停止、复制、Latest 和滚动跟随按原布局；关闭窗口不等于停止后台任务。

详情必须同时匹配所选 task ID 与 detail request ID；移除选中任务就清理旧详情。
使用既有 Core 有界输出和状态版本，优先接已有订阅；无订阅时只在任务窗口可见且
有运行任务时做单一可取消刷新。显示最新可用输出，不能 `take(400)` 永久保留开头；
若只有 stdout / stderr 两个独立缓冲，就按流展示或明确分区，不伪造交错顺序。
未读新输出和 Latest 只改变滚动跟随；不应触发后台任务重跑。

Web：主窗口工具面恢复 920×600，WebView 在内容区正确裁切；使用原页头和返回
层级。保持 Rust document handle、origin、IPC、theme 和 teardown 所有权，
WebKit 内部焦点变化不误触发隐藏。替换文档的迟到回复、关闭回调不能影响新文档。

Web 分两个小任务完成：先按第 2.6 节补宿主透明画布状态交接，修复 R10；再完成
默认控件状态、零样式 fixture、Web Text / JSON Workspace 的布局与窄尺寸回归。
复用现有 token / CSS 生成器与 SDK，不扩展一套声明式 UI 或新的插件主题 API。

恢复路径先按第 6.7 节拆开 suspend / resume / final close，修复 R17；原生 WebView
保留时，IPC 与资源协议闭包仍须换到新的 Rust 绑定，不能沿用初始化时捕获的旧 handle。

必要测试：Tasks 重开 / 双开去重 / resize / 停止 / 滚动；Web 打开、返回、隐藏、
恢复、替换文档。物理检查中文输入、网页焦点和 WebKit helper 最终释放。

### T13 · 跨页面视觉验收、本地化与模块收口

T07 已建立原生材质和字体基线；此阶段验证各页面与实际辅助功能设置：

1. 在纯色桌面和高对比背景分别对照原版，检查玻璃边缘、surface alpha、字体
   字重和行高，不用一张固定背景截图或 RGBA 相同替代整个视觉矩阵。
2. 运行时切换 Reduce Transparency、Increase Contrast、Reduce Motion，验证
   当前所有窗口和 Web 文档同步变化，不重启、不抢焦点，也不恢复旧文档。
3. 按第 2.7 节逐页检查正常、空、加载、错误、长文本和缩放；修复共性问题时
   回归所有引用该 token / 组件的页面与默认插件控件。
4. 核验 Launcher / Settings / Shelf / Tasks / Web 的圆角、阴影、边框及原生
   子视图裁切。macOS 15 与可用的新系统分别记录证据；未测系统标为待验。

同时完成 R14 图标 ticket、并发 / 缓存预算和第 6.6 节资源采样，先修 pending
生命周期，再测是否需要更多缓存策略；不要用一次 RSS 下降宣称完成内存优化。

本地化步骤：逐页将硬编码用户文案迁移到 `resources/localization/`；运行生成器，
使用 typed l10n API。错误详情允许保留底层诊断，但外层提示应本地化。长英文和
中文 tooltip 都不能被固定宽度裁掉。

模块拆分随 T07—T12 完成，不另起纯搬家阶段：

```text
apps/gpui/src/
  app.rs                        # 窗口所有权与入口编排
  host/runtime/                 # Core facade 与按领域拆分的请求方法
  platform/                     # AppKit、材质、通知、Quick Look
  launcher/
    window/                     # 可测试的激活/尺寸/焦点策略
    components/                 # 已验证可复用的页头、候选、提示
    pages/                      # Launcher、Files、Clipboard、History、Tasks、Web
    pages/settings/             # 设置框架及七个分类
    view/mod.rs                 # 实体与事件协调，逐步减少页面细节
```

只在确有共享需求时新增 presentation 类型，不要求把整页写成配置 schema。
每移出一页同步代码地图；移除旧 renderer，保证只有一个实现。

### T14 · 完整验收与文档交付

在固定源码、构建和数据 fixture 上运行第 4 节。将每项证据绑定到实际测试版本；
如果物理测试后又改了相关代码，需要重测受影响序列。旧的 727 / 46 等通过数
只属于原来的工作区，不能复制为新版本结果。

完成条件：R1—R17 逐项核对和修复；每页有对照状态与结果；自动门禁及物理核心
矩阵通过。环境不可用的项目明确保持 Pending / Partial，不宣称全面验收通过。

## 4. 验证流程

### 4.1 自动化与构建

每个实现任务先运行受影响的定向测试，再运行快速 GPUI 门禁。最终执行：

```sh
scripts/check-gpui.sh
scripts/check.sh
git diff --check
```

完整脚本已包含格式、生成物、工作区测试和 Clippy；先读脚本确认覆盖，不无理由
重复整套测试。必要的独立定位命令是：

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 scripts/generate-design-tokens.py --check
python3 scripts/generate-gpui-localizations.py --check
```

涉及源配置时先运行相同生成器的非 `--check` 模式，不手改 generated.rs。
构建包再按开发指南检查：

```sh
scripts/build-macos.sh
codesign --verify --deep --strict build/Anywhere.app
plutil -lint build/Anywhere.app/Contents/Info.plist
otool -L build/Anywhere.app/Contents/MacOS/Anywhere
```

检查包资源与动态库中没有 Flutter / Dart / 旧桥接运行库；仅查看主程序名称不足以
证明清除完成。历史文档中的 Flutter 名称不属于应删除的运行时依赖。

### 4.2 真实桌面矩阵

使用独立 `ANYWHERE_DATA_DIR` 和不与日常实例冲突的快捷键。生命周期验收不用
`ANYWHERE_PREVIEW=1`，它会主动显示预览并跳过真实 Core，不能验证会话问题。

| ID | 操作序列 | 必须观察到 |
| --- | --- | --- |
| N01 | 正常启动 | UIElement / Accessory，无 Dock、无 Launcher，不抢焦点 |
| N02 | Finder 前台，分别按两种真实热键 | 正确模式、正确来源，key window、立即可输入，无 superseded 错误 |
| N03 | 点击菜单栏两个搜索项及 Settings | 三条入口正确，无靠手动点输入框补焦点 |
| N04 | 输入并进工具页，隐藏 14 秒后再唤出；再隐藏超过 15 秒 | 前者恢复允许保留的状态，后者重建；不恢复旧预览和执行资格 |
| N05 | 慢动作 → 隐藏 → 新唤出 → 旧动作回包 | 新窗口、输入、页面不受旧结果控制 |
| N06 | 72 → 3/5/7 行（含 473）→ 清空 | 顶部固定，底部展开收回，无闪跳或 AppKit 重入；520 仅验布局 clamp |
| N07 | 鼠标移至另一屏后唤出，期间改变结果数 | 首次落在鼠标屏；resize 不跟随鼠标跨屏；边界不越屏 |
| N08 | Launcher / Files 切 Finder；Quick Look 内部转移再切 Finder | 外部失焦隐藏；内部转移不误隐藏；预览不会永久豁免 |
| N09 | Settings 草稿关闭：取消、放弃、保存、失败 | 每条分支行为正确；保存成功继续关闭，失败保留草稿 |
| N10 | Shelf / Tasks 与 Launcher 同时打开 | 各自寿命独立；重复打开不创建重复窗口 |
| N11 | 中文连续组词、候选、Enter、Esc、方向键、Space | 输入法优先，候选位置正确，不误执行或打开预览 |
| N12 | 改语言和快捷键后立即查看菜单并唤出 | 菜单与实际生效配置一致，无须重启 |
| N13 | 改系统外观、Reduce Transparency、缩放与背景 | 深色面板可读；opaque fallback 生效，无黑角或双层模糊 |
| N15 | Web 替换、隐藏、返回；关闭全部辅助窗口 | 旧 Web 回调不越权，helper 与 observer 正常回收 |
| N16 | 原生 glass 内同时显示 GPUI 与 Web 输入，移动 / resize / 唤出 | 一个材质 host，透明链成立；坐标、IME、裁切与 AX 子树正确 |
| N17 | 原版 / GPUI / Web 使用固定中英文、Emoji、代码文本，切 scale | 字体角色、行高、基线、字重和截断一致；无缺字或额外阴影 |
| N18 | 零样式插件展示默认控件全部状态，随后打开两个示例插件 | 默认即统一；无不透明根背景、白闪、重复标题；键盘和菜单可用 |
| N19 | 逐页展示长文本、错误、权限和小可用区域 | 主要操作可达、滚动所有权正确、无裁切或通过缩小字体硬塞 |
| N20 | 按 Shift+Tab、修饰方向键、控件 Enter、菜单左右键和连续 Esc | 焦点与层级各自处理，前后循环正确，不触发背后候选 |
| N21 | 快速输入剪贴板查询并复制 / 删除匹配项 | 列表仍匹配当前输入；成功反馈在实际完成后且属于正确条目 |
| N22 | 文件 / 剪贴板长列表持续按 Down，并在文件末尾加载更多 | 选中项始终可见；不重复分页、不误回绕、不丢父查询 |
| N23 | 设置编辑后按热键、切模式、Quit；连续按 ⌘S | 同一草稿守卫；取消保留；保存不重复、失败不假关闭 |
| N24 | Tasks 快速切 A / B，运行输出超过 400 行，用户向上滚动 | 详情与所选任务一致；可看最新输出；手动滚动不被抢回底部 |
| N25 | Web 编辑未提交内容，隐藏 14 秒后唤出，再测超过 15 秒 | 未过期保留原稿且旧 IPC 失效；过期后释放并按正常新打开处理 |
| N26 | 新旧查询交替显示同一应用，连续唤出 / 预览 / Web 往返 | 图标可重试、无持续占槽；observer / timer / WebView 不逐轮累积 |

每条记录：源码版本 / diff、Bundle 路径与构建身份、系统版本、显示器 / scale、
fixture ID、操作序列、截图或日志、Actual、Pass / Fail / Pending。热键注册日志
不能替代 N02；单元测试不能替代 N11；token 测试不能替代 N13。

### 4.3 确定性回归用例清单

以下 A 编号是待新增或扩展的测试，不表示当前已经存在或通过。先检索现有测试，
优先扩充同一入口；只为能复现的行为和顺序建立断言，不测试组件搬家这种机械变化。

| ID | 安排的事件顺序 | 必须断言 | 对应问题 |
| --- | --- | --- | --- |
| A01 | 服务启动 → 第一次热键 → 再次隐藏唤出 | 构造不激活；新 Session / 恢复路线正确 | R1、R2 |
| A02 | suspend 完成前再收到 wake；同时释放旧 hide 回调 | 先等旧撤权完成再恢复；旧 hide 不关闭新展示 | R2、R3 |
| A03 | hide → 重复 blur / hide → 14,999 / 15,000 / 15,001 ms | 期限不因重复通知延长；边界按同一 retention policy | R2 |
| A04 | 旧动作 gate 阻塞 → 新展示 / 查询 → 旧 close / destination / error 回包 | 新窗口、页面和 notice 不受影响；Core 已完成结果不被伪造撤销 | R3 |
| A05 | 旧 focus / placement 任务 → 换窗口或 Settings → 释放旧任务 | 不移动、不激活新页面；无同步 AppKit 重入 | R4、R6 |
| A06 | blur → 内部 Quick Look 转移 → 再切外部应用 | 内部转移取消宽限；外部失焦仍隐藏并关闭预览 | R6 |
| A07 | 系统 / 菜单 / 热键分别离开脏 Settings，随后 Cancel / Discard / Save | 共用守卫；离开意图至多执行一次 | R8、R16 |
| A08 | 保存 A gate 阻塞 → 重复 ⌘S / Save → 编辑草稿 B → 完成 A | 无并发写同一文件；不把 B 标为已保存或直接关闭 | R16 |
| A09 | 热键注册第二步失败；模拟旧绑定回滚也失败 | 报告真实生效集合，菜单不展示未成功值 | R7、R16 |
| A10 | 同页 query A → B；先完成 B 再 A | 只发布 B，旧失败不覆盖新成功；loading 属于 B | R12 |
| A11 | Clipboard 查询非空 → Copy / Delete → 回包前修改查询 | 操作结果绑定原条目；刷新用当前查询，不返回空查询全列表 | R12 |
| A12 | Copy gate 返回错误 / 成功；当前选择在期间变化 | 不提前报成功；不把反馈放到新条目 | R12 |
| A13 | 无选中 → 首次 Up / Down → 超出视口；手动滚动后仅追加进度 | 首次依现行契约首项；键盘 reveal；进度不抢滚动 | R11、R13 |
| A14 | 末尾 Down 与鼠标滚动同时触发同 cursor；再切 query | 只发一次分页；旧页不追加到新查询 | R13 |
| A15 | 图标旧请求 gate → query 换代 → 同应用新请求 → 旧请求完成 | 旧 ticket 只结算自己，新 ticket 未被清除，候选可再次加载 | R14 |
| A16 | 图标成功 / None / error / 接收端关闭；请求超过预算 | 所有路径释放槽位；并发与缓存有界；失败不会永久阻塞 | R14 |
| A17 | Task A / B detail 乱序；移除当前任务 | 详情只匹配当前 ID / request；删除后无残留详情 | R15 |
| A18 | 运行输出跨多个版本；滚动离开底部 → Latest | 最新输出可达；旧版本不回退；手动滚动与自动跟随正确 | R15 |
| A19 | Web suspend → 旧 RPC 完成 → resume 新绑定 | 旧 RPC 无副作用，保留 DOM 草稿，onLaunch 仍只一次 | R17 |
| A20 | Web 旧透明配置 / state / close → 替换 document → 旧回包 | 不改变新背景、不关闭新文档，不重用旧 handle | R3、R10、R17 |
| A21 | 输入法组合 / 文本字段 / 录制器 / 按钮分别接收同一按键 | 仅所属路由消费一次；未处理事件交给默认编辑 | R11 |
| A22 | 进入工具 → 改工具查询 → Back / Esc → 父工作区 | 父 mode / query / Scope / selection / scroll 恢复 | R2、R11 |
| A23 | 快速打开 / 关闭 Settings、Shelf、Tasks、预览、Web 50 轮 | 无重复窗口和订阅；关闭后无对应页面持续刷新任务 | R6、R14、R17 |
| A24 | native material 创建失败 / 系统透明设置变化 / 文档替换 | fallback 可读，只有一个材质 host；变化不重建业务 Session | R9、R10 |

A02—A05、A08、A10、A14—A20 使用显式 gate / 测试 executor 控制顺序。
IME 的模型测试只验证事件分派，N11 / N16 / N20 才验证真实系统输入。
文件 fixture 使用临时合成目录，任务用可控假进程 / 已有假执行器，Web 用获准示例。

运行单个用例时使用锁定工具链，例如：

```sh
source scripts/env.sh
cargo test -p anywhere-gpui --locked <实际测试名称或模块过滤器>
scripts/check-gpui.sh
```

尖括号是实现后替换的参数，不将这条模板记录为已执行命令。若改到 Core 接口，
先跑其相关模块回归，再跑 GPUI；阶段结束运行一次完整 `scripts/check.sh` 即覆盖
已内含的 GPUI / workspace / Clippy，不为凑次数重复运行整套。

## 5. 给实现模型的执行约束

1. 先读当前任务涉及的调用链，再改代码；每次只处理一个 T 编号。任务内有明确
   实现时继续执行，不为可逆小改动反复请求确认。
2. 保留现有未提交修改；不 reset、checkout、stash、clean，不停止用户日常实例。
3. 不迁移匹配、排名、授权、Context 或执行规则到 GPUI；第一方功能走既有注册边界。
4. 不单独新增 Menu Scope，不让缺少 Accessibility 阻断普通功能，不将 Shelf
   塞回主窗口，不恢复固定预览 pane。
5. 每个任务交付：改动文件、解决的 R 编号、定向验证命令及结果、桌验状态、遗留项。
   未完成的 UI 状态或原生测试必须明确写出。
6. 不把测试改成迎合当前错误尺寸，不用 sleep 模拟异步顺序，不用模拟热键成功
   宣称真实全局热键通过，不把历史截图当作当前构建截图。
7. 先完成 T02—T06 的唤出与窗口闭环，再批量推进次级页面；这也是下一轮实施入口。

## 6. 实现前必须固定的行为与数据契约

本节给出 T02—T13 的具体语义；类型名称是建议，不要求先建立框架。
当前页面模型仍在 `launcher/pages.rs`，不是 `pages/tasks.rs` 等独立文件。
实施中先用 `rg` 找实际定义与调用者，再按页面拆分，避免按未来目录猜文件位置。

### 6.1 所有权与窗口状态转换

| 拥有者 | 持有的数据 | 不应持有的数据 |
| --- | --- | --- |
| ProductShell | 共享 Runtime、主 / Shelf / Tasks 句柄、status item、待执行入口意图 | 候选排名、执行资格、剪贴板正文副本 |
| 主窗口展示 | native window lease、phase、presentation generation、当前页面与焦点目标 | 全应用唯一的查询代次 |
| Launcher 会话绑定 | SessionId、request revision、SnapshotGate、保留期限版本 | 可被独立 Shelf / Tasks 任意覆盖的全局 session_id |
| 各页面模型 | 查询、page epoch、request ID、选中对象 ID、scroll anchor、loading / error | 其他页面请求的完成标记 |
| Settings controller | persisted、effective、draft、draft version、save operation、pending leave | 依赖 Launcher 查询流的保存身份 |
| 原生临时 surface | owner window lease、generation、可取消订阅 | 按标题字符串扫描任意窗口后直接操作的长期权限 |

native window lease 指向本次实际窗口及其拥有者代次。原生操作尽量接收明确句柄，
标题仅用于显示或调试；窗口关闭重开后，即使 window number 被系统复用，旧回调
也不能操纵新窗口。Shelf / Tasks 不共享主窗口的焦点、保留计时器或查询代次。

| 当前状态 / 事件 | 处理 | 完成后的状态 |
| --- | --- | --- |
| 启动 | 初始化服务 / 菜单 / 热键，不采集 Launcher Context、不主动 show | Hidden，无活动 Launcher 会话 |
| Hidden，普通 wake，无可恢复状态 | 预留 Session，冻结来源身份，开始采集和展示 | Showing → Visible |
| Hidden，普通 wake，有合法 retained workspace | 等待本次 suspend 完成，resume 后恢复原 workspace；不先改成热键对应空页 | Showing → Visible |
| Hidden，过期 / 配置已失效 / 明确全局 Action | 最终释放旧状态，开始新会话 | Showing → Visible |
| Showing，再次请求隐藏 | 使激活与 focus 回调失效，撤销已预留 Session 的资格 | Hiding → Hidden |
| Visible，收到同模式的全局 Launcher 热键 | 走统一隐藏，保留符合条件的工作区 | Hiding → Hidden |
| Visible，收到不同模式的 Launcher 热键 | 按既有模式切换规则更新输入图与 revision，不误走重复 activate | Visible |
| 菜单栏明确 Show Current App / Universal | 明确显示 / 切模式；菜单操作不因为同模式而意外切换成隐藏 | Showing 或 Visible |
| 任意状态收到 OpenSettings | 先处理原页和临时 surface，再显示设置；同一设置窗口去重 | Visible，Settings |
| Settings 收到 Launcher 热键 / 切模式 / Quit | 进入同一个草稿守卫，按第 6.4 节保存待完成意图 | 决策后再完成原意图 |
| Visible，真正外部失焦 | 宽限期间阻止新的页面执行；到期复验所有权与 active 状态后 suspend / hide | Hidden |
| Hiding，再次普通 wake | 合并至一个恢复意图；等待撤权完成后恢复，不让旧 hide 尾部覆盖新 show | Showing → Visible |
| Hidden，重复 blur / hide | 幂等，不重新延长 retention，不重复释放资源 | Hidden |
| retention 到期 | 同时校验 Session、hidden epoch、窗口仍隐藏；最终释放临时数据 | Hidden，无 retained workspace |

普通热键 toggle 与显式菜单 Show 的区别来自原版交互；不得将所有入口等同于
`activate_from_hotkey` 的无条件清空。Settings 的 Done / Esc / 系统关闭通过守卫后
隐藏设置形态并保留合法父工作区；由热键发起的离开则继续执行 wake，不能直接把
未提交设置切到 Launcher 页面。

建议把展示阶段与会话操作分开：主线程先提高 presentation generation 并暂停 UI
执行入口，再由 Runtime 完成 Core 撤权。必要时给 Core 现有 suspend 提取轻量同步
撤权入口或等待 worker 确认，主线程不能阻塞等待慢 I/O。一次隐藏只撤权一次。
恢复必须串在同一次隐藏的撤权完成之后，不能把“已发异步请求”当成完成。

### 6.2 键盘事件的拥有者、优先级和消费结果

使用完整 modifiers 和 FocusTarget，保留 Shift / Control / Alt / Platform，
不要提前压成一个 modified 布尔值。处理器明确返回 Handled / PassThrough；
只有 Handled 才 stop propagation / prevent default，同一按键只能处理一次。

事件处理顺序：当前原生面板 / WebView 自有输入 → 当前编辑器 IME / composition →
活动 modal / 快捷键录制器 → 局部导航层 → 页面命令 → 默认文本 / 控件行为。
这里的顺序是所属输入表面的分派顺序，不要求从 GPUI 截获原生面板的所有按键。

| 场景 | 按键 | 预期 |
| --- | --- | --- |
| 当前编辑器正在 composition | 文字、Space、方向、Enter、Esc、Tab | 交给输入法；宿主不执行、清空或切 Scope |
| 普通文本 / Web 输入 | ⌘A/C/V/X/Z、修饰方向键、Delete | 保留标准编辑；不删除剪贴板条目或移动后台列表 |
| 搜索编辑器聚焦，分类实际可用，无局部层 | Tab / Shift+Tab；兼容 Ctrl 组合 | 正 / 反向切可见 Scope 或文件类型，查询和焦点保留 |
| 对象 Actions、菜单选择器、绑定选择、输出或无分类 | Tab | 不改变背后 Scope；按所属层既有规则处理 |
| Launcher 空查询且可浏览菜单 | → / ← | 进入 / 返回菜单；非空查询交给光标 |
| 普通候选列表 | 无修饰 Up / Down | 选择并 reveal；不打断输入法 |
| 文件 / 剪贴板无选中 | 首次 Up / Down | 按当前专题契约选首项；随后使用原列表导航规则 |
| 浏览中选中可预览对象 | Space / ⌘Y | 在具备资格时切预览；查询编辑中的空格仍是文字 |
| 工具栏按钮实际聚焦 | Space / Enter | 激活按钮；不执行背后的选中候选 |
| Settings 字段 / 录制器 | Enter、Tab、字符 | 所属字段 / 录制器处理；⌘S 只提交一次保存 |
| Web 插件编辑器 / 对话框 | Tab、Enter、Esc | Tab / Enter 由页面拥有；无输入法组字的 Esc 由受控宿主桥接返回原 Launcher 工作区，页面仍可响应其他返回入口 |

Esc 按最内层处理：IME → 预览 / modal → Web Tool 受控返回 → 导航选择器 /
对象 Actions / 绑定选择 / 输出 → 菜单层 → 页面父工作区。Launcher 的根页返回链：

```text
Content 根页 --Esc--> Current App 根页
Current App 空白根页 --Esc--> 空白 Global Search
Global 空白根页 --Esc--> 隐藏
任意根页有查询 --Esc--> 清空查询并保留入口
菜单 / 对象 Actions / 预览 --Esc--> 返回所属层
File Search 对象动作 --Esc--> 文件列表
File Search 有查询 --Esc--> 清空文件查询
File Search 空查询 --Esc--> 父工作区
Web Tool 聚焦、无组字 --Esc--> 原 Launcher 工作区
```

Clipboard / History 查询先按旧页面的清空 / 返回语义处理；删除条目的快捷键仅在
明确结果浏览且搜索编辑器不消费删除键时启用。普通 Action 快捷键按既有目录和
执行资格生效；不顺带恢复原应用菜单快捷键，也不在 Settings 录制时执行动作。

### 6.3 异步请求身份与结果发布

每个请求捕获不可变身份。可用一个薄 `RequestTicket` 结构配合现有 typed event，
不为此新增全产品事件总线。请求类型只携带其需要的字段：

| 请求 / 结果 | 必须匹配的身份 | 过期时处理 |
| --- | --- | --- |
| Launcher snapshot | SessionId、revision、snapshot_seq、所属展示 | 按 SnapshotGate 拒绝 |
| ActionResult 的 UI effect | SessionId、展示 generation、发起请求 / 目标身份 | 不隐藏、不导航、不覆盖 notice；Core 已完成效果正常记账 |
| Clipboard / History 搜索 | window owner、page epoch、query request ID、查询 / 来源 | 丢弃旧内容和旧错误，仅完成对应请求的资源清理 |
| Copy / Delete | owner、operation ID、item ID；刷新另读当前 query ticket | 操作完成独立结算；不能把原条目反馈放到新选择上 |
| Task detail | tasks window epoch、task ID、detail request ID、输出版本 | 不覆盖别的任务，停止旧刷新 |
| Settings save | 独立 save operation ID、发起时 draft version | 接收本次保存结果；保留期间新编辑，不依赖 query generation |
| 图标 | 稳定 app identity / 图标版本、request ticket、cache epoch | 结算旧 ticket；是否发布缓存按 cache epoch 判断 |
| Web RPC / state / resource / canvas | document handle、origin、document epoch、拥有者 | 取消或拒绝，绝不切到当前新 handle 继续执行 |
| 延迟 focus / placement / observer | native window lease、presentation generation、期望页面 / phase | 直接失效，无原生副作用 |

接受结果的同一次模型更新中先改 loading / data / selection，再生成必要 UI effect；
不能在 update 外留下无条件 hide 或导航。第二次排队到主线程的操作紧邻执行时
仍复验，因为用户可能在两个回调间再次切页。

正确性不依赖 debounce。若添加合并连续输入，先推进查询代次和撤销旧资格，再延后
昂贵查询；不能让旧列表在新查询中继续可执行。相同查询的进度更新不重置选中 ID，
旧请求失败不能清空新请求的 loading 或覆盖已显示的新成功状态。

### 6.4 Settings 保存和离开意图

保存状态至少区分 Idle / Saving / Failed；保存请求包含本次 draft version。
离开意图为 `CloseSettings / Wake(mode) / ShowPage / Quit`，只保留一个明确意图；
关闭确认重入合并，不堆叠对话框。多次 wake 选择最后一个明确入口，取消决策时
丢弃待执行意图，绝不让它在后续一次无关保存完成后突然执行。

保存期间重复 ⌘S / 按钮不启动第二个文件写入。允许继续编辑时提高 draft version；
旧保存成功只更新它保存的 persisted snapshot，draft 比它新则仍显示未保存。
SaveAndClose 只有本次提交覆盖当前草稿且保存成功才继续离开，否则留在设置。

保存顺序与失败策略：

1. prepare：校验完整 draft、解析并规范化两个快捷键并比较解析后的组合，不能仅
   比较两个原始字符串；解析目录、生成待提交值，保存前收集旧 effective snapshot。
2. 由一个 controller 串行化 Core 偏好应用、热键切换与配置持久化。文件使用本次
   operation 专属临时文件后 rename，避免多个 writer 共用固定 `.tmp`。
3. 任一步失败，回到已知的旧受控配置并实际检查恢复结果；回滚也失败就保留每项
   实际 effective 状态和错误。不能只恢复结构体字段却宣称系统注册成功。
4. 登录项等外部效果可能部分失败，返回明确的 saved / effective / effect errors，
   不宣称跨文件、Core 和系统服务具备原子事务。相关项失败不自动执行 pending leave。
5. 已由用户授权的目录书签和设置草稿是不同所有权；取消草稿不偷偷启用新搜索范围，
   也不重新弹授权。Feature / Clipboard 既有即时生效项按原契约显示反馈，不把
   它们的异步失败误当成整个草稿已成功保存。

Done、Esc、标题栏关闭、Launcher 热键、菜单跳页、Quit 都先走这个 controller。
Quit 的清理不再激活旧 Launcher；已开始任务的退出策略沿用当前契约。明确区分
保存配置失败和系统效果失败，错误附近提供重试且保留可编辑草稿。

### 6.5 父工作区、稳定选择与滚动

进入工具或对象动作前保存父 workspace：mode、query / content、Scope ID、菜单
路径、selected ID、scroll anchor、input selection。Context 和执行权仍引用
Rust Session，不能把旧候选列表当作恢复后的授权来源。焦点存语义目标而不是
已经销毁控件的 FocusHandle。

工具自己的 query 不写进父 Launcher query。Back 恢复父 workspace；Esc 按第 6.2
节层级处理。父对象已消失时回到最近合法层并按原选择规则回退，不以旧 index
恰好命中新行的方式恢复。下列滚动意图分别处理：

| 意图 | 行为 |
| --- | --- |
| 键盘选择 | 布局后最小距离 reveal 所选 ID，不改为全局 scroll-to-top |
| 用户手动滚动 | 保存 anchor；进度 / 图标 / notice 更新不把视口拉回选中项 |
| 查询 / Scope / 类型变化 | 按原规则清选择或选首项；新的列表从相应初始位置显示 |
| 同查询追加结果 | 保持已有 ID、顺序和 anchor；键盘末尾导航可继续到新增行 |
| 删除条目 | 在同过滤列表中选邻接合法项或空；列表高度变化保持视口合理 |
| 页面返回 / 期限内恢复 | 布局完成后恢复 anchor 与选择；无对象则安全回退 |
| Tasks 新输出 | 在底部才自动跟随；用户滚动离开后提示新输出，Latest 恢复跟随 |

分页按 cursor 去重且同一查询最多一个在途追加。失败保留旧数据，重试同 cursor；
切查询使旧 cursor 失效。长列表复用 GPUI 普通虚拟列表；实现时检查当前版本的
API 和行高度模型，不先增加通用列表框架。小列表不必为形式统一全部虚拟化。

### 6.6 图标与后台资源预算

图标请求从 query generation 分离。pending 改成 key → ticket / cache epoch，
完成时只有 ticket 一致才清理自己的槽；旧 ticket 完成不能删除同 key 新请求。
成功、None、error、任务取消和接收端关闭都有结算路径，清空 cache 时提升 epoch。

本轮图标实现先采用至多 4 个在途请求、128 个成功缓存项的 LRU 预算；集中定义在
GPUI 策略模块，若已有共享策略则复用。它是初始资源上限，不是内存性能承诺。
失败缓存有界并可在新会话 / catalog 变化后重试。缓存键至少包含稳定应用身份及
图标内容版本；候选 ID 只用于把结果映射回当前可见行，不解析成系统路径权限。

资源归属表必须随实现核对：

| 资源 | 创建 / 活跃条件 | 停止 / 释放条件 |
| --- | --- | --- |
| 窗口 focus observer | 窗口存活时安装一次 | 窗口最终关闭；可见状态仅切策略，不重复注册 |
| blur 宽限 / focus retry / placement | 单次展示操作 | 换代、隐藏、获得目标焦点或任务完成 |
| Launcher snapshot subscription | 当前会话交互或明确保留的请求 | 隐藏暂停发布，过期 / superseded / 关闭时取消 |
| Quick Look monitor / lease | 当前 owner 的预览实际存在 | 关闭、失焦、换 owner / generation |
| Clipboard 页面搜索 | 页面查询变化或明确刷新 | 新查询、页面离开、独立窗口关闭 |
| Tasks 输出刷新 | Tasks 可见并有正在查看的运行任务 | 隐藏 / 关闭、切任务、任务完成；不停止真实进程 |
| Web state / IPC / canvas | 当前有资格的绑定 | suspend 取消旧绑定工作；final close 释放视图与资源 |
| retention deadline | 一次合格隐藏 | 恢复、过期或新会话；重复 blur 不重置 |

不把全部后台活动统一停掉：用户已启用的剪贴板采集与真实运行任务有自己的生命周期。
优化空闲轮询时也应保留这些能力。全局热键当前 24 ms 检查可在正确性稳定后改为
库回调 / 唤醒式桥接，严禁把阻塞 receiver 放到 GPUI 主线程。

性能记录遵循 [性能指南](../performance.md)：分别测热键事件到 show 调用、key /
可输入就绪、首份可用 snapshot、渲染帧和归属 helper。用相同 release 构建、固定
fixture，在冷启动及至少 30 次热唤出记录 p50 / p95 / max；50 次开关 / 隐藏恢复
后检查资源计数和 footprint 趋势。结果未采集前不编造毫秒或内存目标，不能只用
`hotkey_to_visible_ms` 表示屏幕首帧。

### 6.7 Web suspend、resume 和 final close

| 操作 | Core / Runtime | 原生 WebView / SDK |
| --- | --- | --- |
| 期限内普通隐藏 | suspend 当前 Session，旧 document binding 立即失效；保留 Rust 恢复引用 | visibility false，取消旧 pending，保留 DOM 和用户文稿，原生视图不可交互 |
| 期限内恢复 | 先 resume Session，再以 Rust 持有的旧绑定换取新 runtime / document epoch；origin 保持 | 复用同一原生视图，更新桥接绑定后 visibility true；SDK 重连，onLaunch 不重复 |
| 返回退出工具 / 替换包或文档 / 超期 / Quit | final close，撤销并清除 Rust 文档 | 取消任务与资源回调，释放原生视图，清理引用 |
| 恢复失败 / 包已撤销或更新 | 不继续使用旧绑定，按对应失效原因关闭或正常重新打开 | 不谎称原稿已恢复；不把旧 DOM 自动接到另一个 origin |

当前 wry 创建时 IPC / custom protocol 闭包捕获初始 handle。保留原生视图后，
必须为它提供由 Host 更新的明确 active binding；不能只替换 GPUI 字段，闭包仍向
旧 handle 发请求。每个异步请求进入时冻结绑定，返回时按其旧身份复验；不要为让
旧 RPC 成功而在中途偷偷改用新 handle。

恢复时重新建立当前绑定的 state 跟随、SDK 握手和透明画布；旧 pending 在 suspend
时已取消，不能重放复制、执行等副作用。资源协议继续由 Rust 校验包与 digest，
旧 origin / replaced document 的请求不因同窗口而获得新权限。用未保存编辑内容、
onLaunch 计数、origin 和两代 runtime ID 共同验证，不能只检查窗口还在。

## 7. 可直接执行的小任务清单

下表保留验收粒度。本轮状态以上面的实施记录为准：已有自动化和定向源码证据的
项目为 Code Done；只有真实验桌观察的 Physical Smoke 才记录在验收页；缺少
对应系统或矩阵的项目继续为 Pending / Partial。页面任务仍按前述 T 顺序维护，
不因代码行数拆分而改变已有验收。
下表 A / N 编号分别链接第 4 节的自动化与物理用例。

| 小任务 | 所属任务与入口 | 本次必须交付的结果 | 定向验证 / 结束条件 |
| --- | --- | --- | --- |
| W01 | T01；旧提交及 docs | 状态 fixture 与截图记录格式；冻结旧版几何 | 基准来源和缺失截图状态明确 |
| W02 | T02；host/runtime、view build | 移除启动时 activate，Runtime 与每次 Session 分开 | A01；启动仍有菜单 / 热键 |
| W03 | T02；app、生命周期小模型 | 所有 wake 入口、toggle / show 差异、Session reserve 和 gate 切换 | A01、A04；第 6.1 节入口表 |
| W04 | T02；统一 hide / resume | 撤权完成交接、幂等隐藏、保留期限与恢复工作区 | A02、A03；无真实 15 秒 sleep |
| W05 | T03；poll 及目的地路由 | ActionResult、error、原生 UI effect 资格一致 | A04、A05；旧结果无原生副作用 |
| W06 | T03；typed events、pages 模型 | 页面 request ticket 与设置 operation ID 分开 | A10、A17；旧结果不改变新 loading |
| W07 | T04；platform、app | 显示后 key / editor 焦点交接，取消旧 retry | N02、N03、N11；模型测试不代替桌验 |
| W08 | T04；observer / preview owner | 通知式失焦与可取消宽限，内部 / 外部焦点判定 | A06、N08；无永久预览豁免 |
| W09 | T05；layout 与 placement | 纯几何、hasPanel 条件、鼠标屏和固定顶部 | A05、N06、N07；空首页 72 |
| W10 | T06；status item / hotkeys | 设置成功更新菜单；注册与回滚真实状态 | A09、N12 |
| W11 | T07a；platform/material | 一块原生 glass / HUD / opaque host，保留原 GPUI 视图 | A24、N16；重新验证 N02 / N11 |
| W12 | T07b；tokens / theme / 文本样本 | 文字角色与实际字体 / 行高 / 基线 | N17；生成器 check 通过 |
| W13 | T07c；键盘路由 | 完整 modifiers、焦点 owner、Handled / PassThrough | A13、A21、N20 |
| W14 | T07c；导航与 pages | Esc 链、菜单左右键、父工作区恢复 | A22、N20；专题契约一致 |
| W15 | T07c；候选 / 页头 / Scope | 原几何、按下选择、双击区、tooltip / AX | N14、Launcher 全状态截图 |
| W16 | T08；文件页与列表 | reveal、分页去重、末尾键盘追加和错误重试 | A13、A14、N22 |
| W17 | T09；settings controller | 全入口草稿守卫、pending leave、单次保存版本 | A07、A08、N23 |
| W18 | T09；host/preferences / hotkeys | 串行保存、独立临时文件、解析组合判重与失败恢复 | A08、A09；故障后 effective 可解释 |
| W19 | T09；设置框架 / General / Shortcuts | 七分类、164 侧栏；通用、语言和录制器 | N09、N17；两页截图与录制取消 |
| W20 | T09；Context / Search | 来源状态、排除项、目录授权、引擎与错误 | 权限失败只影响来源；两页截图 |
| W21 | T09；Clipboard / History / Features | 策略、来源检测 / 路径、能力目录完整 | 三页截图；即时项与草稿状态分清 |
| W22 | T10；clipboard events / pages | 查询 ticket、复制真实反馈、操作后当前查询刷新 | A10—A12、N21 |
| W23 | T10；History / Shelf UI | 648×600 / 独立 480×520，选择 reveal 与独立寿命 | A13、A23、N10、N22 |
| W24 | T11；应用历史与详情 | 760×520；来源查询身份、返回和页专属布局 | A10、A22；长路径 / 无来源截图 |
| W25 | T11；Web Packages / Plugin Detail | 默认尺寸、权限 / 详情 / 错误及安装操作布局 | 无多余通用壳；未知工具错误可读 |
| W26 | T12；ProductShell / Tasks | 独立去重窗口、resize 与任务 ID 选择 / 详情 | A17、N10、N24 |
| W27 | T12；Tasks 输出 | 当前流有界输出、刷新取消、Latest 和手动滚动 | A18、A23、N24；不静默截断前 400 行 |
| W28 | T02 / T12；Web 生命周期 | suspend / resume / final close、保留原稿和新桥接绑定 | A19、A20、N25；onLaunch 一次 |
| W29 | T12；Web native / theme CSS | 原生透明与 CSS canvas 交接、无首帧白闪 | A20、A24、N16、N18 |
| W30 | T12；web-controls 源及示例 | 零样式控件状态页、Web Text / JSON 响应布局 | SDK / 示例检查与 N18 |
| W31 | T13；图标请求与缓存 | ticket 结算、4 并发 / 128 LRU、可重试失败 | A15、A16、N26 |
| W32 | T13；生命周期资源 | observer / timer / subscriptions 归属与取消，采样 | A23、N26；重复序列不累积 |
| W33 | T13；l10n 源与各页面 | 硬编码文案归位，长中文 / 英文及 tooltip | 生成器检查，全部页面状态核对 |
| W34 | T14；scripts、构建、验收记录 | 完整工程检查、签名包和真实桌验矩阵 | 绑定当前源码，Pending 不冒充 Pass |

W28 的 Web 恢复设计在 W04 时就要确定，最终在 Web 专项落地；不得在 W04 先接
不可恢复的 final close，再把其造成的数据丢失当作 W28 之外的问题。W11 会改变
原生视图层次，必须重测已通过的唤出 / IME；W15—W30 每移出一页同步删除旧实现，
不等到最后再进行一次高风险的全文件搬家。

建议每个小任务的交付记录固定为：

```text
任务：Wxx / Txx；状态：TODO / InProgress / CodeDone / Verified
源码身份：HEAD + 本轮 diff；构建身份：如涉及物理验收填写
修改入口：实际文件和符号
修复：对应 R 编号及用户可观察结果
定向测试：真实命令、用例与结果
桌面证据：N 编号及 Pass / Fail / Pending
下一任务：依赖已满足的 W 编号；剩余问题写明
```

CodeDone 只表示代码与定向检查完成，Verified 还需要该行要求的物理或视觉证据。
无桌面条件时继续其他不依赖它的实现项，保留对应 Pending；不为等待一次截图
停掉已经明确的代码修复，也不把缺少物理证据的关键入口宣称完成验收。

## 人工验收执行单（由维护者安排时间）

2026-11-13 起，代码修改完成后不自动启动程序或执行物理桌验。自动门禁照常执行；
以下清单由维护者决定时间。没有记录的新构建一律为 Pending，不能继承旧截图结论。

### 准备与记录

1. 记录 Git revision、未提交差异、macOS 版本、显示缩放、语言、系统外观及辅助功能选项。
2. 需要验收时才执行 `scripts/build-macos.sh`。包内版本、可执行路径、签名与进程身份必须对应本次代码。
3. 使用独立数据目录 `ANYWHERE_DATA_DIR`，配置与日常实例不冲突的两组热键。
   剪贴板采集保持关闭，测试历史使用合成文本。记录 PID 和 Bundle ID，禁止按应用名批量停止。
4. 每项记录步骤、预期、实际、截图 / 日志、Pass / Fail / Pending；失败后保留状态和精确复现步骤。
5. 本轮输入修复包含原生 first responder 恢复、Content 编辑器恢复，以及按所属窗口隔离全局按键拦截器。
   自动焦点测试仅覆盖 GPUI 逻辑，AppKit 实际收键仍待本清单验收。

### 首要阻断项：唤醒后能输入且 Esc 可用

| 步骤 | 预期 |
| --- | --- |
| 冷启动，尚未唤醒 | 后台运行，无 Launcher、无 Dock 项、不抢焦点 |
| 真实热键唤出 Current App，直接键入 abc | 输入立即可见；默认 Scope 与可见结果一致 |
| Universal 热键唤出，直接键入 abc | 输入接收字符，不需要额外鼠标点击 |
| 查询中按 Esc | 清查询；再按 Esc 按当前模式层级退回或隐藏 |
| 同一入口隐藏再唤出，重复 10 次 | 每次均可输入 / Esc；15 秒内按契约保留状态 |
| 从 Settings 返回 Launcher 再输入 | 普通搜索输入获得焦点；无不可见 Settings 控件吞键 |
| Content 模式隐藏再唤出 | 可见多行编辑器获得焦点，普通隐藏输入框不抢焦点 |
| 打开 Tasks / Shelf 后再次唤出 Launcher | Launcher 可输入；其他窗口的录制 / Esc / Tab 不干扰它 |
| 中文输入法组词，按方向键、Enter、Esc | 先处理候选 / 组词，不执行候选动作或隐藏 Launcher |
| 原生焦点日志 | 可见窗口 key=true；first responder 为 GPUIView 或当前 WebKit / IME 合法持有者，不能停在 GPUIPanel |

### 键盘与层级

逐项执行 [Flutter 键盘迁移核对](../launcher-interaction.md#flutter-键盘迁移核对) 的所有行，
每种操作分别覆盖按钮、真实键盘和隐藏恢复后的键盘入口。额外边界如下：

- Current App Scope 迟到后默认选中一次；用户已经输入或手动切换时不能被迟到结果抢回。
- Tab / Shift+Tab / Ctrl+Tab / Ctrl+Shift+Tab 每次只移动一个可见分类，首尾回环，不停在无按钮的 None。
- 文件页每次 Tab 只切一个类型；Settings Tab 遍历控件；Web 内部 Tab 交给 WebKit。
- 带 Shift / Command / Option / Control 的方向键保持文本编辑语义，不能误选结果；Tasks 显式组合键除外。
- Enter 分别验证普通动作、子菜单、文件、Clipboard 复制、应用历史打开 / 复制、对象操作及删除确认；按住 Enter 只触发一次。
- Esc 分别验证 Quick Look、模式菜单、对象操作、菜单层级、非空查询、空查询、Settings 草稿与快捷键录制。
- Shelf 验证复制回跳目标失效的反馈；普通 Backspace / Delete 编辑查询，不能误删记录。
- Tasks 搜索中键入 s 不能停止任务；停止必须使用按钮 / 菜单或 ⌘.；⌘L 返回 Launcher 后可立即输入。
- 翻页键在首尾不越界；筛选 Tasks 后选择的可见行必须滚入视口；跨页滚动位置不串用。

### 全页面视觉与布局

Launcher、File Search、Clipboard History、Shelf、Application History、Tasks、Settings 七分类、
Web Packages、Plugin Detail、Web Tool 各执行一次：空态、加载态、有数据、长文本、错误态与返回恢复。
使用中文 / English、短文本 / 超长路径、浅色 / 深色外观分别检查：

- 标题、副标题、按钮、Scope、Current / Universal 模式选择器、快捷键提示无遮挡或越界。
- 页头与 traffic lights 分离，固定按钮不被挤压；长内容截断或滚动，必要说明不逐字竖排。
- 选中、hover、disabled、键盘焦点与鼠标选择状态一致；Settings 切分类后无旧焦点框残留。
- Launcher 从 72 px 到多行 / 最大高度时顶部锚点固定，无闪烁、借用 panic 或旧 resize 覆盖新页面。
- Tasks 窄内容、长输出、Diagnostics、Follow / Latest 与手动上滚符合预期；Web 插件使用宿主共享样式。

### 原生材质、系统与资源

1. 白色、深色、彩色和高频纹理背景下检查四角，无白楔、方形底边或双重描边；同屏对照 Spotlight。
2. 背景仅由原生材质生成，GPUI 根透明；文字在明亮背景下可读，不靠额外整窗蒙层补偿。
3. 用户自行切换系统明暗、Reduce Transparency、Increase Contrast 后检查所有已打开窗口即时响应。
4. 主窗口失焦后按契约隐藏；Settings、Shelf、Quick Look 和 WebView 内部焦点转换不被误判。
5. 验证菜单栏、真实全局热键、中文 IME、Quick Look 关闭回焦及登录项。
6. Web 工具反复替换、关闭和隐藏，旧 IPC / 回调不能影响新文档；关闭后 helper / 预览资源按所有权释放。
7. 退出临时实例后核对 PID 已退出、日常实例仍在。性能采样另记空闲 / 唤醒 / 搜索 / 关闭后的 CPU 与内存，不能把单测当作性能证据。

### 自动门禁与签收

开发阶段执行 `scripts/check-gpui.sh`、`scripts/check.sh` 与 `git diff --check`。
构建、签名通过只表示产物可构建；上述人工项目全部保留独立结果。维护者完成后在当前
MVP 验收页填写构建身份、通过项、失败项与剩余项，再决定是否替换日常实例。

### 后续代码审查修复：输入与确认状态隔离

以下修复已加入源码，实际窗口验收由维护者安排：

| 问题 | 修复 | 人工复核步骤与预期 |
| --- | --- | --- |
| Shelf 删除确认前组合键可改变选择，确认会针对新选择 | 确认面板优先处理按键，删除绑定 pending 条目 ID；目标消失时不删除其他条目 | 使用两条合成记录，对第一条发起删除后按 ⌘↓ / ⌘2，确认对象不能变化；刷新导致原对象消失后确认不能删除第二条 |
| 剪贴板面板残留状态拦截其他页面的按键 | 面板路由仅在 Clipboard / Shelf 页面生效 | 保留确认状态后切到 Settings / Launcher，Esc 只操作当前页面 |
| 本地 Action 快捷键读取未保存草稿 | 执行路由改为读取已生效 preferences | 修改绑定但不保存，旧绑定仍生效、新绑定不生效；保存后交换；放弃后保留旧绑定 |
| Context 快捷键输入无效键后退出录制 | 数字或保留编辑组合被拒绝后继续录制 | 录制中输入 ⌘1、⌘C，显示错误且仍可直接录入合法组合；Esc 能取消 |
| Content Esc 返回后焦点仍在隐藏的多行编辑器 | 模式变更后聚焦普通 Launcher 输入 | Content 中按 Esc 后立即输入 abc，文字必须出现在当前可见搜索框；随后 Esc 按正常层级返回 |

自动回归覆盖确认面板优先级、删除目标身份 / 目标丢失、无效录制状态及 Content Esc
后的逻辑焦点。它们不证明原生按键、系统输入法或真实删除链路已物理验收。
