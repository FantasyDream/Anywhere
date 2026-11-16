# GPUI UI 设计与原生材质

当前 GPUI 产品视觉契约。页面行为见 [Launcher 交互](launcher-interaction.md)，
源码入口是 `apps/gpui/src/`。最后整理：2026-11-14。

原版 Liquid Glass、文字度量、插件透明画布及逐页布局的目标与实施状态见
[界面对齐方案](gpui-ui-alignment-plan.md#2-界面目标与基线)。当前代码已经接入
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
返回只保留页头入口和 Esc，不增加固定页脚。

## Tokens 与组件

[设计参数](../resources/design-tokens.json) 是颜色、字号、间距、圆角、页头高度
和控件尺寸的唯一来源。Rust 投影由
`python3 scripts/generate-design-tokens.py` 生成到
`apps/gpui/src/ui/design_tokens.rs`。

- 页面使用 GPUI 普通组件和 `ui/theme.rs`，不另建平行 UI framework。
- Launcher 输入区、Scope 行和结果行共享相同焦点边框、行高和文字层级。
- 选中态使用中性覆盖和边界；图标不套单独底板。
- 普通搜索不显示结果条数。有对象动作时行内显示 `… ⌘K`；无对象动作时保留
  Enter 提示。
- 文件页的数量、进度和错误属于查询，位于类型过滤下方；打开和更多操作属于
  选中行。
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
- 窗口外框、高光边和阴影由原生材质与 AppKit 提供，GPUI 不再覆盖窗口级描边。
  页面高度不能因外框边框被挤压或裁切。
- 固定高度工具行中的标题和副标题为单行并显式截断；按钮和快捷键作为不可收缩
  控件留在右侧，说明文字占剩余宽度，不能侵入操作区。
- 页面级操作浮层通过 `page_popup` 的锚定 / deferred 层绘制，以窗口右缘为锚并
  保留安全边距；History、Clipboard 和 Tasks 使用 100% 不透明的
  `menuSurface`，底层行文字不得透入。不能把 `absolute + right/top` 面板塞进
  滚动内容容器。对象动作请求失败时必须关闭空面板，不能留下永久 loading 或
  不可点击的浮层。
- Tasks 保留原生 traffic lights，页头从 28 px titlebar inset 之后开始；
  多操作区和脏草稿操作区在窄宽度下换行，不覆盖标题或相邻操作。
- Running Tasks 输出区跟随当前可见流；用户向上滚动后暂停自动跟随，直到选择
  Latest。
- Clipboard History 使用 Launcher 宽度与单列结果；Clipboard Shelf 使用独立
  480×520 浮动窗口，最小 360×360，定位在鼠标所在屏幕右下角，不内嵌完整正文。
  Shelf 页头从 28 px titlebar inset 后开始，原生交通灯、状态图标、搜索框与可见
  Esc 不得相交。可见 Esc 和键盘 Esc 走同一路由：依次处理预览、局部操作 /
  删除确认、查询，最后才是关闭窗口。
- 插件 Detail 使用普通 GPUI 文本与滚动区域。未知 NativeTool 明确显示错误，
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
| 原生背景 | 系统玻璃、辅助功能降级、窗口轮廓 | `apps/gpui/src/platform/material/` |

本轮合并 43 处重复文字 / 选中态定义及 6 处结果行圆角 / hover 逻辑。
样式在 render 时读取当前主题，不能缓存旧主题颜色。统一样式不增加窗口背景、
蒙层或玻璃描边。Tasks 元数据、Settings 表单及插件内容仍有各自布局；这些不是
同一种页面，不强行用 schema 解释它们。后续相同语义的控件继续进入公共组件，
不要在页面复制一份再调参数。

## 焦点与输入

- Launcher 初始焦点在查询编辑器。方向键移动选择；Enter 执行；根页 Esc 直接
  隐藏，局部菜单、对象操作和预览仍先返回所属层。
- Launcher 是菜单栏后台应用：启动时不显示窗口、不抢焦点，也不进入 Dock。
  点击菜单栏搜索图标或全局热键后，无边框浮动窗口才获得焦点。
- 呼出期间保持 Accessory policy，通过程序化激活让浮动窗口成为 key；失去 key
  后隐藏。这样不会为了获得焦点临时暴露 Dock 项。样式变化后重新恢复 GPUI
  first responder，再在主循环下一拍聚焦当前编辑器。
- Launcher、其文件搜索和内容页失焦后隐藏。Settings 与 Running Tasks 使用普通
  窗口层级和默认 Space 行为，失焦后保持打开但可以被其他应用覆盖。Clipboard
  Shelf 只保持置顶，失焦不隐藏，必须使用明确的关闭入口。Quick Look 显示期间
  不把系统预览面板误判为外部失焦。
- `prefer_character_input` 为真时，根视图直接让字符、Enter、Esc、方向键和
  Space 先交给输入法。
- 查询中的空间是文字；只有列表已选中且不在输入法组合态时，Space 才切换
  系统预览。
- Quick Look 关闭后，焦点回到发起预览的当前页面编辑器。
- 设置分类切换、未保存草稿和工具返回都恢复对应页面的原编辑器，而非首页。
- WebView 获得焦点后，页面内部键盘由 WebKit 管理；返回和宿主命令使用显式
  原生路径，不从网页 keydown 猜测。

## 材质与可读性

Launcher 使用 AppKit 无边框 nonactivating `NSPanel`。macOS 26+ 将原 GPUI
内容容器设为 `NSGlassEffectView.contentView`，再将该 glass view 设为窗口的
`contentView`。不得把 glass 当作 GPUI 的 sibling 背景：AppKit 只保证
`contentView` 的合成顺序。[Apple 的内容容器契约](https://developer.apple.com/documentation/appkit/nsglasseffectview/contentview)

macOS 15–25 使用 `NSVisualEffectView(HUDWindow, behindWindow, active)` 承载
原 GPUI 内容。上层 `platform/material.rs` 只产生跨平台材质计划：系统明暗、
Reduce Transparency、Increase Contrast 和 18 px 圆角；下层
AppKit adapter 再映射到 Glass 或 HUD。以后 Windows 的 Mica/Acrylic adapter
复用同一计划，不修改任何页面。

Reduce Transparency 时拆除材质并使用不透明 fallback，变化由
`NSWorkspace.notificationCenter()` 的
`NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification` 立即刷新。
Increase Contrast 调整文字与控件的语义对比；基础可读性 scrim 来自
`contentScrim` token，Increase Contrast 时提高其 alpha。窗口高光和边缘仍交给
系统，不额外覆盖描边或阴影。Settings、Tasks 与 Clipboard Shelf 共享这条安装
路径。模式切换先取回原内容，再解除旧 glass 的 contentView 引用，避免嵌套和
旧材质持有内容。

GPUI Kit 的组件 Root 与页面是两层：必须在 `configure_component_theme()` 中
同时清空 `theme.colors.background` 与 `theme.tokens.background`，并在加载 Dark
主题后再应用。Root 使用后者的 Background 绘制；只清空 LauncherView 或设置
CAMetalLayer 为非 opaque，仍会被 Root 的纯色 quad 遮挡。实测 A/B 与复现命令见
[根背景排查记录](archive/gpui-glass-investigation-2026-11-13.md)。

GPUI 的主题根背景保持透明，但 `LauncherView` 在原生材质上绘制一层约 93%
中性灰 `contentScrim`（`#ed303438`）。提高覆盖率不是替代原生玻璃，而是稳定
`NSGlassEffectView` 在不同窗口高度下随采样面积变化的整体亮度；窗口仍有 7%
桌面透射和系统材质高光。这是白底/彩色桌面下的统一可读性层，不是
`NSGlassEffectView` tint：原生 `Regular` Glass 仍使用 `tintColor=nil`，其余
折射、高光和桌面颜色继续由 AppKit 合成；Increase Contrast 时 scrim alpha
提高到约 95%。使用中性表面而不是纯黑遮罩，避免窗口高度变化时因桌面内容占比
不同而产生明暗跳变。Metal 渲染面和输入视图由 GPUI 继续拥有。GPUI 除这一层
scrim 外不叠加描边、渐变或阴影，原生 18 px 圆角与 mask 负责裁切。窗口使用
near-clear 背景保留系统阴影，该颜色 alpha 为 0.0001，不作为可见填充。

Spotlight 参考截图是视觉基准，不是公开源码契约。使用公开
[NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview)
容器，不能因此声称复制了 Spotlight 的私有实现。GPUI 文字仍由 Metal 绘制，
不会仅因放进材质就自动变成 AppKit 的语义文本 / vibrancy；保持系统字体，
物理验收已证明仅调整字体和文字 token 不能覆盖白底与高亮彩色桌面的最坏情况，
因此使用上述单层内容 scrim，而不修改原生 tint 或增加第二套模糊表面。

此前强制 HUD 和 sibling 结构的尝试不能作为 Liquid Glass 成功证据。
新路径已确认安装 Glass、非 opaque 内容层和 key focus。Root 修复后，同窗
彩色底图上的真实 GPUI 玻璃已通过 A/B。Build 95 对白底和彩色渐变背景的实际
合成截图采样显示，正文、次要文字与 muted 文字均达到 4.5:1 以上，语义图标均
高于 3:1；Reduce Transparency、浅色系统主题和 VoiceOver 仍按验收矩阵保持
Pending，不能仅凭安装日志推断全部辅助功能完成。

浅色材质上的正文与选中文字不得低于 4.5:1，语义图标不得低于 3:1。边界、焦点
和错误状态同时使用形状或文字，不只降低透明度。实际桌面材质与文字栅格化必须
实机检查；代表色对比测试不能证明所有系统外观。当前实测样本记录在
[GPUI 验收](mvp-gpui.md#2026-11-13-ui-对齐缺陷修复build-95)。

## Web 默认样式

`resources/web-controls.css` 和 `resources/web-bootstrap.js` 由 GPUI Web 文档
自动注入。普通 HTML 按钮、输入框、下拉框、标签、checkbox / radio、链接、
代码块和分隔线因此继承主题；插件不导入另一份基础 CSS。

公共规则使用低优先级选择器，允许插件局部覆盖布局。WebView 挂载后由宿主把
`--aw-web-canvas` 切到 transparent，使 html、body 和常见应用根节点露出同一个
原生材质；Reduce Transparency 时露出的是宿主不透明 fallback。CSS 控件层不创建
第二个模糊层。Web 与 GPUI 共享 fonts、colors、spacing 和 control metrics，
允许系统 WebKit 与 GPUI 文字栅格存在细微差异。

## 窗口

| 窗口 | 行为 |
| --- | --- |
| Launcher | 648 px 宽，初始 72 px 高并随内容增长到 520 px；无边框、不可缩放、置顶并加入所有 Space；失焦隐藏，根页 Esc 按 Content → Current App → Global → 隐藏返回。文件搜索、Clipboard History、Application History 和 Web / Plugin 是本窗口内的页面 |
| Settings | 复用主原生窗口，648×508、普通窗口层级和默认 Space 行为、透明标题栏、保留关闭 / 最小化按钮；未保存草稿先显示保存 / 放弃提示，失焦不隐藏但可被其他应用覆盖；再次打开时从最小化恢复 |
| Clipboard Shelf | 480×520、最小 360×360、可缩放；每次打开定位到鼠标所在屏幕右下角；置顶并加入所有 Space，保留关闭 / 最小化 / 缩放按钮；失焦不隐藏，只有明确关闭或清空查询后的 Esc 才结束窗口；再次打开时从最小化恢复 |
| Quick Look | 系统 QLPreviewPanel，同一进程最多一个所有者 |
| Running Tasks | 按需 920×600 普通窗口，最小 760×420，默认 Space 行为；保留任务会话和滚动位置，可被其他应用覆盖；再次打开时从最小化恢复 |
| Web / Plugin | 当前 Launcher 内的工具 surface，页头省略号提供 Reload / Back，返回保留父查询与选择 |

主窗口启动时隐藏且不激活。产品以 `LSUIElement` / Accessory 身份运行，菜单栏
搜索图标提供 Current App、Universal Search、Settings 和 Quit；两项搜索入口
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
