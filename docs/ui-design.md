# GPUI UI 设计与原生材质

当前 GPUI 产品视觉契约。页面行为见 [Launcher 交互](launcher-interaction.md)，
源码入口是 `apps/gpui/src/`。最后整理：2026-11-23。

原版 Liquid Glass、文字度量、插件透明画布及逐页布局的目标与实施状态见
[界面对齐方案](archive/gpui-ui-alignment-plan.md#2-界面目标与基线)。当前代码已经接入
原生材质 host；物理效果仍按验收矩阵记录。

## 信息层级

按页面任务决定可见性。主要任务常驻；低频动作使用带文字的菜单或行内入口；
错误、权限和未保存草稿不能被折叠或紧凑布局掩盖。

| 页面 | 默认突出 | 按需打开 |
| --- | --- | --- |
| Launcher | 输入、Scope、候选、执行 | `⌘K` 对象动作、模式选择、菜单浏览 |
| 文件搜索 | 查询、类型过滤、稳定列表 | Quick Look、对象动作、错误详情 |
| Clipboard History | 查询、内容列表、复制 | Quick Look、`⌘K` 操作 |
| Clipboard Shelf | 搜索、快速复制、行反馈 | 预览、删除、刷新 |
| 应用历史 | 来源查询、选择和打开 | 对象动作、刷新；来源配置在 Settings |
| Settings | 分类、开关、目录、保存和错误 | 来源路径、隐藏应用、能力说明 |
| Running Tasks | 状态、输出、停止和复制 | 对象动作与滚动帮助 |
| Web 工具 | 工具内容、权限状态和返回 | 更新、撤销和诊断 |

单一 Current App 使用搜索框上方的应用标签说明范围，不显示只有一个选项的
Scope 标签栏。首页保留 Rust 顺序并限制可见条数；菜单浏览入口必须在首屏可达。
文件搜索、剪贴板历史、应用历史与 Web 工具共用页头的返回、内容区、Esc 和随主题变化的操作浮层样式。
应用历史和剪贴板历史的页头操作在有无结果时均可刷新，行内操作只作用于选中项。
JSON 工作区用带文字的“JSON 操作”按钮区分文档操作与宿主页头的工具选项。
Web 工具页头显示工具名称。应用历史的对象菜单保留对象标题，长列表只在操作行区域
滚动，方向键会把选中行滚入视野。菜单项左对齐，菜单占用的高度随可用窗口调整。

## Tokens 与组件

[设计参数](../resources/design-tokens.json) 是颜色、字号、间距、圆角、页头高度
和控件尺寸的唯一来源。Rust 投影由
`python3 scripts/generate-design-tokens.py` 生成到
`apps/gpui/src/ui/design_tokens.rs`。

- 页面使用 GPUI 普通组件和 `ui/theme.rs`，不另建平行 UI framework。
- Launcher 输入区、Scope 行和结果行共享相同焦点边框、行高和文字层级。
- 结果行选中态以中性透明高光覆盖为主（Dark Glass 使用半透明白色提亮，Light Glass 保持高对比度柔和灰蓝），只搭配分隔线强度的细边界，并在左侧以绝对定位 3×20 px 强调色指示条标示焦点，保持标题与搜索框文字左对齐；Increase Contrast 时恢复焦点轮廓色。未选中行 hover 使用更弱的 `hover`，不能与选中态相同；符号/工具图标统一包裹在 `30×30 px`（圆角 7 px）的半透明 Squircle 图标底板中，真实应用图标居中置于 32 px 槽位并使用 6 px 圆角。
- Launcher、文件搜索、剪贴板历史、Shelf 与应用历史的列表标题支持查询关键词高亮（`StyledText::with_highlights`，按 UTF-8 边界安全切分）；文件路径副标题按中段省略（保留首段与末两级路径），悬停 tooltip 保留完整绝对路径。
- 剪贴板历史与应用历史底部显示 32 px 上下文操作页脚栏（`footer_bar`），集中呈现主动作、次要动作及快捷键键帽（如 `↵`、`␣`、`⌘K`、`⌘R`）；主 Launcher 搜索页保持紧凑高度，不显示额外页脚栏。
- 剪贴板历史提供类型筛选栏（全部 / 文本 / 图片 / 文件）、选中项右侧实时预览检查器（含十六进制 / RGB 颜色色块预览、字节数、相对时间与快捷操作）以及非阻塞底部居中 HUD Toast（1.6 秒自动消失，不引发窗口尺寸重排）。
- 文件搜索扫描中与 Running Tasks 活跃任务状态（Running / Stopping）使用动态 `Spinner` 替代静态图标；Running Tasks 选中行保留语义状态色并在侧栏与详情头展示已运行/总耗时，Output / Diagnostics 标签具备 hover 与 active 按压态。文件搜索在滚动接近底部时自动续页，不再常驻冗余的静态“加载更多”按钮，仅在翻页在途时展示内联加载指示。
- Settings 所有模态对话框（剪贴板确认、未保存关闭提示、Action 快捷键选择器）使用随明暗主题自适应的 100% 不透明 `actionMenuSurface` 与主题前景色，避免浅色模式下黑字叠黑底；侧栏七个分类使用差异化语义图标色；外观设置提供 System / Light / Dark 三张微缩界面预览卡片。主题、语言、开机自启、采集和文件搜索选项即时保存。搜索目录、排除目录名和隐藏应用展示可逐项移除的标签，批量文本编辑按需展开。路径与快捷键草稿在设置栏保存或放弃，侧栏标记未保存分类。
- Launcher 候选标题与搜索输入文字左对齐，图标框与搜索图标左对齐；结果区上下留白对称。
  副标题为空、与标题重复或与来源标签重复时不显示，Core 也不为空 Context 详情拼接
  悬空的 `·`。
- 普通搜索不显示结果条数。有对象动作时行内显示 `… ⌘K`；无对象动作时保留
  Enter 提示。
- 文件页在类型过滤下方突出结果数。扫描中、完成、部分结果分别配搜索、对勾或警示图标，扫描期间已有的问题数同步显示，重试与状态文字并排。指定类型搜索完成且无匹配、无错误时，可点“搜索所有文件类型”保留关键词并返回输入框。再次点击当前类型保留浏览位置与预览。
- Launcher、应用历史、剪贴板历史与 Shelf 使用同一套行内主操作按钮，文字和细边界标出打开或复制。Shelf 的「复制」支持直接点击，应用历史中可悬停查看完整标题和路径。
- 共享结果行向辅助技术提供标题和副标题。不可用候选保留原因和禁用指针，点击后维持当前选择、菜单层级及提示。
- Settings 使用紧凑侧栏和统一行背景；标题、状态、描述和控件保持稳定对齐。
- Clipboard capture 属于 Clipboard 页，并以 “When opening Launcher” 分区说明
  15 秒自动填入；隐藏应用属于 Search 页。Context 只列 Finder selection、Selected
  text 和 Browser current page，Features 使用顶部能力说明与 “Built In” 分组。
- Settings 的未保存关闭确认框使用 100% 不透明的 `menuSurface`，外层使用
  `modalScrim` 遮挡底页；底层设置文字不得透过对话框。工具栏 dirty 状态按自然
  宽度显示，不能用固定 120 px 硬截断。确认框打开后，遮罩负责阻断底层指针命中；
  Tab / Shift+Tab 只在确认按钮间循环，普通字符和组合键不能到达底页，Esc 只取消
  当前模态，关闭后恢复打开前的焦点。
- 模式选择器的两条记录各 44 px，标题和说明显式设置行高；搜索头和选择器
  均不可被 Flex 压缩。紧凑窗口打开选择器必须实际 resize 后再显示完整内容。
- Scope 内容不足一屏时整组水平居中；内容溢出时从可访问的左侧起点排列，溢出
  宽度进入横向滚动范围，选中项必须能滚入视口。不能通过给溢出滚动内容直接
  `justify_center` 把首项放到负坐标。
- Launcher Notice 使用共享的 44–96 px 有界高度；内容超过上限时在 Notice 内部
  纵向滚动。搜索头、Notice 和结果区按文档流依次布局，不得互相覆盖；文案变化
  后的窗口高度仍由同一可见状态收敛入口更新。
- 动态 resize 从当前事件回调延后执行，避免重借当前窗口而失败；执行时读取
  最新页面，防止过期的 Launcher resize 覆盖刚打开的 Settings。
- 输入、清空查询和结果数量变化引起的窗口伸缩直接切换，不播放原生 resize 或
  图层隐式动画。macOS 在同一无动画事务内更新窗口位置、尺寸与材质布局，并在
  提交前重绘可见内容，避免缩放旧的 Metal 图像造成撕裂；顶部锚点保持不变。
- 窗口外框、高光边和阴影由系统材质提供。GPUI 使用统一的内容圆角，页面高度不能因
  原生边框被挤压或裁切。
- 固定高度工具行中的标题和副标题为单行并显式截断；按钮和快捷键作为不可收缩
  控件留在右侧，说明文字占剩余宽度，不能侵入操作区。
- 工具页右侧菜单使用不透明表面、264 px 宽和 260 px 最大高度。原生列表页通过透明区域拦截菜单外点击，关闭时恢复编辑器焦点。主 Launcher 与文件搜索保留候选列表内的对象动作下钻。
- Settings、Tasks、Clipboard Shelf 和独立工具窗口的窗口按钮通过统一平台抽象按系统习惯呈现：macOS 保留左上角原生交通灯，Windows 在右上角绘制最小化 / 最大化（还原）/ 关闭按钮。
  工具页头位于 28 px 窗口控制区下方。多操作区和脏草稿操作区在窄宽度下换行，
  不覆盖标题或相邻操作。
- Running Tasks 输出区跟随当前可见流；用户向上滚动后暂停自动跟随，直到选择
  Latest。
- Clipboard History 使用 Launcher 宽度与单列结果；Clipboard Shelf 使用独立
  480×520 浮动窗口，最小 360×360，定位在鼠标所在屏幕右下角，不内嵌完整正文。
  Shelf 页头从 28 px 窗口控制区后开始，状态图标、搜索框与可见 Esc 不得相交。
  可见 Esc 和键盘 Esc 依次处理预览、局部操作 /
  删除确认、查询，最后才是关闭窗口。
- 未知 NativeTool 明确显示错误，
  不尝试解释任意 UI DSL；输入正文在固定页头下使用有界滚动区域，长路径和无空格
  标识必须换行且最后一行可到达。
- 会删除剪贴板数据的 Clear 和缩短保留期限必须先显示不可恢复范围；取消不改写
  运行时，提交期间阻止重复触发。暂停提供恢复 / 5 / 15 / 60 分钟，期限提供容量
  上限 / 1 / 7 / 30 / 90 天，并显示当前真实状态。
- Reduce Motion 从 macOS 辅助功能设置读取并在运行中同步到 GPUI；该状态只关闭
  额外动效，不覆盖用户的系统选择。所有页面和辅助窗口在原生 resize 时按当前
  屏幕可用区域约束宽高，内容在窄宽度下滚动或换行。

不因一次页面需求建立通用表单 schema、页面配置系统或一层包装组件。出现重复
布局且语义相同，再提取。颜色和尺寸不散落为一次性字面量。

## 样式责任边界

页面以组合方式使用统一呈现层，不再重复维护同一语义的字号、行高与选中态：

| 层 | 责任 | 入口 |
| --- | --- | --- |
| 参数 | 颜色、尺寸、字体的唯一源与生成投影 | `resources/design-tokens.json` |
| 语义样式 | `body_text`、`caption_text`、`label_text`、`result_surface` | `apps/gpui/src/ui/styles.rs` |
| 公共组件 | Keycap、Notice、EmptyState、SectionHeading、StatusBadge | `apps/gpui/src/launcher/components/mod.rs` |
| 页面容器 | 统一搜索头、返回、提示与内容区域 | `apps/gpui/src/launcher/view/page_shell.rs` |
| 页面 | 数据、事件、业务状态和不同内容的布局组合 | `apps/gpui/src/launcher/view/pages/` |
| 原生背景 | 系统玻璃、辅助功能降级、窗口轮廓 | `apps/gpui/src/platform/material.rs`、`platform/material/macos.rs` |

本轮合并 43 处重复文字 / 选中态定义及 6 处结果行圆角 / hover 逻辑。
样式在 render 时读取当前主题，不能缓存旧主题颜色。统一样式不增加窗口背景、
蒙层或玻璃描边。Tasks 元数据、Settings 表单及插件内容仍有各自布局；这些不是
同一种页面，不强行用 schema 解释它们。后续相同语义的控件继续进入公共组件，
不要在页面复制一份再调参数。

## 焦点与输入

- Launcher 初始焦点在查询编辑器。方向键移动选择；Enter 执行；根页 Esc 按 Content → Current App → Global → 隐藏返回，局部菜单、对象操作和预览先退出所属层。
- Launcher 是菜单栏后台应用：启动时不显示窗口、不抢焦点，也不进入 Dock。
  点击菜单栏搜索图标或全局热键后，无边框浮动窗口才获得焦点。
- 呼出期间保持 Accessory policy，通过程序化激活让浮动窗口成为 key；失去 key
  后隐藏。这样不会为了获得焦点临时暴露 Dock 项。样式变化后重新恢复 GPUI
  first responder，再在主循环下一拍聚焦当前编辑器。
- Launcher、其文件搜索和内容页失焦后隐藏。Settings 与 Running Tasks 使用普通
  窗口层级和默认 Space 行为，失焦后保持打开但可以被其他应用覆盖。Clipboard
  Shelf 只保持置顶，失焦不隐藏，必须使用明确的关闭入口。Quick Look 显示期间
  不把系统预览面板误判为外部失焦。
- 输入法组字优先；内容页编辑器聚焦且无修饰键时，Launcher 在字符输入前接管
  `↑` / `↓`、`Enter` / `Return`，而 `Shift+Enter` 和其他修饰组合保留编辑语义。
  其余 `prefer_character_input` 字符、Esc、方向键和 Space 先交给输入法。
- 查询中的空间是文字；只有列表已选中且不在输入法组合态时，Space 才切换
  系统预览。
- Quick Look 关闭后，焦点回到发起预览的当前页面编辑器。
- 设置分类切换、未保存草稿和工具返回都恢复对应页面的原编辑器，而非首页。
- WebView 获得焦点后，WebKit 先将按键送到当前控件。网页浮层消费 Esc 后留在文档，
  未消费的 Esc 通过当前文档句柄发给宿主，沿相同的返回路径处理。输入法组词继续由
  WebKit 处理。

## 材质与可读性

Launcher 使用 AppKit 无边框 nonactivating `NSPanel`。macOS 26+ 将原 GPUI
内容容器设为 `NSGlassEffectView.contentView`，再将该 glass view 设为窗口的
`contentView`。不得把 glass 当作 GPUI 的 sibling 背景：AppKit 只保证
`contentView` 的合成顺序。[Apple 的内容容器契约](https://developer.apple.com/documentation/appkit/nsglasseffectview/contentview)

macOS 15–25 使用 `NSVisualEffectView(HUDWindow, behindWindow, active)` 承载
原 GPUI 内容。上层 `platform/material.rs` 只产生跨平台材质计划：系统明暗、
Reduce Transparency、Increase Contrast 与 18 px 圆角；下层 AppKit adapter 再映射
到 Glass 或 HUD。macOS 26+ 的 Launcher、Settings、Tasks 和 Clipboard Shelf
统一使用 `WindowGlassStyle::Regular`，映射为未着色的
`NSGlassEffectViewStyle::Regular`。以 macOS 系统通知为视觉对照，玻璃浓度、
折射、高光和轮廓由 AppKit 合成。
Windows 在透明效果可用时使用 DWM 与 Windows Composition，按窗口类型设置
原生背景。macOS 和 Windows 共用 GPUI 内容布局、
控件配色和字体。两端在绘制前注册随应用分发的 Noto Sans SC 与 Fira Mono；字体覆盖范围
以外的字符由系统补齐。Noto Sans SC 使用从 2.004 的可变 TTF 固定得到的四个字重（400、
500、600、700），供 Core Text 和 DirectWrite 选择相同的字形。字体按 SIL OFL 随包发布。
Launcher 默认是无任务栏图标的工具浮窗。打开设置时使用可出现在任务栏的窗口。
Launcher 的背景通过 `HostBackdropBrush` 和圆角几何裁切，关闭系统透明效果或无法
创建背景时使用不透明主题色。高对比度和动画状态由 Windows 适配层读取。
[Noto Sans SC 2.004](https://github.com/notofonts/noto-cjk/releases/tag/Sans2.004) · [Fira Mono](https://github.com/google/fonts/tree/4c89430f86c811a3522161890da7b63894e1117c/ofl/firamono)
[Windows 指南](windows.md) · [Microsoft DWM 圆角说明](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners)

Reduce Transparency 时拆除材质并使用不透明 fallback，变化由
`NSWorkspace.notificationCenter()` 的
`NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification` 立即刷新。
Increase Contrast 调整文字与控件边界的语义对比，不增加根背景、页头或结果行的
补色蒙版，也不将浅色材质误切为深色字色方案。窗口高光和边缘仍交给系统，
不额外覆盖描边或阴影。Settings、Tasks 与
Clipboard Shelf 共享这条安装路径。模式切换先取回原内容，再解除旧 glass 的
contentView 引用，避免嵌套和旧材质持有内容。

GPUI Kit 的组件 Root 与页面是两层：必须在 `configure_component_theme()` 中
同时清空 `theme.colors.background` 与 `theme.tokens.background`。启动时先准备
原生材质和实际 Light / Dark 外观，再加载对应 GPUI Kit 主题并应用颜色。Root
使用后者的 Background 绘制；只清空 LauncherView 或设置 CAMetalLayer 为非
opaque，仍会被 Root 的纯色 quad 遮挡。实测 A/B 与复现命令见
[根背景排查记录](archive/gpui-glass-investigation-2026-11-13.md)。

GPU 主题根背景与所有产品页背景保持透明；不再按页面绘制 `contentScrim` /
`pageScrim`。这两个共享 token 保留为透明值，兼容已有 Web 样式变量。搜索头和
未选中结果行也透明，选中 / hover 只使用普通低 alpha 交互高亮，不再绘制固定
深色的结果卡片。输入框、按钮、确认框等局部控件表面保留各自语义；仅确认框
出现时的模态交互遮挡用于阻止点击穿透，不属于玻璃背景补色。

`Regular` Liquid Glass 可以随宿主面积、背后内容和系统设置改变浓度，这是本次
保留的系统行为，不再切换为 Clear 或通过蒙版抵消。`tintColor` 为 `nil`，原生
appearance 由已保存的外观设置决定；GPUI 不额外叠加整窗描边、渐变、阴影或第二层模糊。
原生 18 px 圆角与几何 mask 仅负责裁切，窗口 near-clear 背景仅用于保留系统阴影。

局部玻璃表面通过克制的双主题窗口托底（`palettes.glassDark.window` / `palettes.glassLight.window`，开启 Increase Contrast 时切换到 `contrast.*` 生成调色板）将极端纯白或纯黑底层的玻璃合成亮度约束在中灰对比度死区之外，同时保留原生玻璃折射与边缘高光。`ui/theme.rs` 直接选择生成的 `Palette` / `FixedColors`。Web 文档同步调色板和 `contrast.fixed` 中的 `--aw-*` 变量，操作菜单随当前明暗外观，采用不透明的菜单表面和独立选中背景。浅色在窗口玻璃之上
使用更高覆盖率的浅灰托底，JSON 编辑区单独使用高覆盖率的表面，以减轻桌面内容透出。
原生材质当前的 Light / Dark 外观：普通浅色 Glass / HUD 使用深炭墨色系
Primary / Secondary / Muted 与对应状态色，深色 Glass / HUD 使用柔和亮色角色及微暗向选中态（避免白雾高亮在白底上降低对比度）。
Reduce Transparency 使用和当前 Light / Dark 外观对应的实色背景及字色；
Increase Contrast 沿用该外观的高对比度角色。`menuSurface`、确认框和主按钮等固定深色局部表面使用独立的
`onDark*` 字色与前景观；Field、Card、Hover、Selection、Keycap 等半透明局部
表面随玻璃明暗选择对应表面并搭配同向文字。Accent、Warning、Danger、Success
及图标也有表面变体，不能只切换三个灰阶角色。搜索头、候选标题和模式选项沿用
产品统一字体 Noto Sans SC，主要标题使用 Semibold，说明文字保持 Normal，与 macOS
通知卡的层级一致。

设置 → 通用 → 外观提供跟随系统、浅色和深色。已有设置默认跟随系统。保存后为
AppKit 设置应用级外观并刷新主窗口、Settings、Tasks、Shelf 和当前 Web 文档，
设置草稿按现有保存与放弃流程处理。启动时在读取已保存偏好后应用该选择。跟随
系统时通过窗口外观通知同步，辅助功能通知继续刷新原生材质与语义配色。WebView
通过当前 Rust 文档句柄更新 `--aw-*` 语义变量和原生控件配色，保留文档输入、
选择与滚动位置。系统明暗不等于字下
背景的实际亮度；当前方案改善可读性，但单一外观信号不能保证所有透明混合背景
达标，仍失败的局部区域应调整对应语义角色或局部表面，不恢复整窗 scrim。

自动回归覆盖透明 token、表面 / 材质不一致时的字色选择、Web 变量切换和旧文档
句柄隔离。macOS 26+ Glass、15–25 HUD、浅 / 深系统外观、辅助功能
及实际桌面明暗交叉矩阵仍须物理验收；代表色计算和单元测试不能充当实机通过
结论。完整实施与验收边界见
[Regular 玻璃调整与验收](archive/reviews/gpui-regular-glass-2026-11-17.md)。

系统通知和 Spotlight 参考截图是视觉基准，不是公开源码契约。使用公开
[NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview)
容器，不能因此声称复制了系统通知或 Spotlight 的私有实现。GPUI 文字仍由 Metal 绘制，
不会仅因放进材质就自动变成 AppKit 的语义文本 / vibrancy；因此字色由产品主题
按上述表面规则提供，而不修改原生 tint 或增加第二套模糊表面。

此前强制 HUD 和 sibling 结构的尝试不能作为 Liquid Glass 成功证据。
新路径已确认安装 Glass、非 opaque 内容层和 key focus。Root 修复后，同窗
彩色底图上的真实 GPUI 玻璃已通过 A/B。当前 token 与语义配色已有自动
回归，但实际材质对比度、Reduce Transparency、浅色系统主题和 VoiceOver 仍按
验收矩阵保持 Pending，不能仅凭安装日志推断全部辅助功能完成。

浅色材质上的正文与选中文字不得低于 4.5:1，语义图标不得低于 3:1。边界、焦点
和错误状态同时使用形状或文字，不只降低透明度。实际桌面材质与文字栅格化必须
实机检查；代表色对比测试不能证明所有系统外观。当前物理验收范围和状态见 [GPUI 验收](mvp-gpui.md#物理桌验矩阵)。

## Web 默认样式

`resources/web-controls.css` 和 `resources/web-bootstrap.js` 由 GPUI Web 文档
自动注入。普通 HTML 按钮、输入框、下拉框、标签、checkbox / radio、链接、
代码块和分隔线因此继承主题；插件不导入另一份基础 CSS。

公共规则使用低优先级选择器，允许插件局部覆盖布局。WebView 挂载后由宿主把
`--aw-web-canvas` 切到 transparent，使 html、body 和常见应用根节点露出同一个
原生材质；Reduce Transparency 时露出的是宿主不透明 fallback。宿主同时把
Primary / Secondary / Muted、状态色、Field、Card、交互表面与焦点边界等
`--aw-*` 变量从首帧起使用当前玻璃表面配色，并随主题更新。dialog / tooltip 保留
固定深色的 `--aw-on-dark-*` 字色。更新只作用于当前 Rust 文档句柄，不重载页面。CSS 控件层
不创建第二个模糊层。Web 与 GPUI 共享 fonts、colors、spacing 和 control metrics，
允许系统 WebKit 与 GPUI 文字栅格存在细微差异。

macOS `lb-wry 0.53.3` 依赖已显式启用 `transparent` feature，因此
`.with_transparent(true)` 会关闭 WKWebView 自身背景绘制，再由
`--aw-web-canvas` 与原生材质交接。普通 Web 控件的 `color-scheme` 继承根节点；
带按钮和选择器的操作菜单可使用 `.aw-action-popover`，继承宿主的浅色或深色
菜单底色。固定深色的 dialog / tooltip 局部使用 `dark` 和 `--aw-on-dark-*`。自动依赖与 CSS 检查不能替代
桌面、主题和 Reduce Transparency 的真实 WebView 验收；诊断与回归范围见
[Web 插件排查](archive/reviews/gpui-web-plugin-background-review-2026-11-16.md)。

## 窗口

| 窗口 | 行为 |
| --- | --- |
| Launcher | 648 px 宽，初始 72 px 高并随内容增长到 520 px；无边框、不可缩放、置顶并加入所有 Space；失焦隐藏，根页 Esc 按 Content → Current App → Global → 隐藏返回。文件搜索、Clipboard History、Application History 和 Web / Plugin 是本窗口内的页面 |
| Settings | 复用主原生窗口，648×508、普通窗口层级和默认 Space 行为、透明标题栏、保留关闭 / 最小化按钮；未保存草稿先显示保存 / 放弃提示，失焦不隐藏但可被其他应用覆盖；再次打开时从最小化恢复 |
| Clipboard Shelf | 480×520、最小 360×360、可缩放；首次定位到鼠标所在屏幕右下角，可拖动并在本次进程内保留位置；置顶并加入所有 Space，保留关闭 / 最小化 / 缩放按钮；失焦不隐藏，只有明确关闭或清空查询后的 Esc 才结束窗口；再次打开时从最小化恢复 |
| Quick Look | 系统 QLPreviewPanel，同一进程最多一个所有者 |
| Running Tasks | 按需 920×600 普通窗口，最小 760×420，默认 Space 行为；保留任务会话和滚动位置，可被其他应用覆盖；再次打开时从最小化恢复 |
| Web / Plugin | 当前 Launcher 内的工具 surface，页头省略号提供 Reload / Back，返回保留父查询与选择 |

主窗口启动时隐藏且不激活。产品以 `LSUIElement` / Accessory 身份运行，菜单栏
搜索图标提供 Current App、Universal Search、Settings、Running Tasks 和 Quit；两项搜索入口
同时显示当前配置的快捷键。关闭 Launcher 只隐藏，关闭 Settings 先走草稿决策。

Launcher 隐藏后，有查询或工具页面的状态保留 15 秒；空白首页再次唤醒重新采集
环境。Shelf 有自己的窗口寿命，主窗口隐藏不强制关闭 Shelf，但应用退出释放全部。

## 状态与提示

成功、信息、警告和错误使用不同语义颜色与图标，正文保持可读主色。错误显示
具体动作和可重试入口，不以单个 toast 替代持久状态。来源权限问题只禁用对应
来源，其他动作保持可用。

Web 工具栏和操作区允许换行；窄宽或矮高时内容滚动，不丢操作。输入焦点使用
明确边框。加载只显示当前查询状态，不把空结果误标为失败。

## 验收

自动化覆盖状态模型、路由和清理顺序。物理验收另行检查：

- 中文输入法组合、候选窗、Enter、Esc、方向键与 Space；
- 主窗口、状态栏、Shelf 和 Quick Look 的焦点往返；
- Reduce Transparency、浅 / 深主题、显示缩放和窗口边界；
- Quick Look、WebKit helper、登录项和 Accessibility 的系统行为；
- VoiceOver 页面顺序、名称、状态与操作。

当前证据与未验证项见 [GPUI 技术方案与验收](mvp-gpui.md#物理桌验矩阵)。
