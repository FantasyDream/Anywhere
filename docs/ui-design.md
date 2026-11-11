# GPUI UI 设计与原生材质

当前 GPUI 产品视觉契约。页面行为见 [Launcher 交互](launcher-interaction.md)，
源码入口是 `apps/gpui/src/`。最后整理：2026-11-11。

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
- Running Tasks 输出区跟随当前可见流；用户向上滚动后暂停自动跟随，直到选择
  Latest。
- Clipboard History 使用 Launcher 宽度与单列结果；Clipboard Shelf 使用独立
  480×520 浮动窗口，不内嵌完整正文。
- 插件 Detail 使用普通 GPUI 文本与滚动区域。未知 NativeTool 明确显示错误，
  不尝试解释任意 UI DSL。

不因一次页面需求建立通用表单 schema、页面配置系统或一层包装组件。出现重复
布局且语义相同，再提取。颜色和尺寸不散落为一次性字面量。

## 焦点与输入

- Launcher 初始焦点在查询编辑器。方向键移动选择；Enter 执行；Esc 清空、
  返回或隐藏。
- `prefer_character_input` 为真时，根视图直接让字符、Enter、Esc、方向键和
  Space 先交给输入法。
- 查询中的空间是文字；只有列表已选中且不在输入法组合态时，Space 才切换
  系统预览。
- Quick Look 关闭后，焦点回到发起预览的当前页面编辑器。
- 设置分类切换、未保存草稿和工具返回都恢复对应页面的原编辑器，而非首页。
- WebView 获得焦点后，页面内部键盘由 WebKit 管理；返回和宿主命令使用显式
  原生路径，不从网页 keydown 猜测。

## 材质与可读性

主窗口和 Clipboard Shelf 使用透明窗口背景。AppKit 材质层只在系统能力可用时
启用；Reduce Transparency 或能力缺失时使用不透明主题回退。

浅色材质上的正文与选中文字不得低于 4.5:1，语义图标不得低于 3:1。边界、焦点
和错误状态同时使用形状或文字，不只降低透明度。实际桌面材质与文字栅格化必须
实机检查；代表色对比测试不能证明所有系统外观。

## Web 默认样式

`resources/web-controls.css` 和 `resources/web-bootstrap.js` 由 GPUI Web 文档
自动注入。普通 HTML 按钮、输入框、下拉框、标签、checkbox / radio、链接、
代码块和分隔线因此继承主题；插件不导入另一份基础 CSS。

公共规则使用低优先级选择器，允许插件局部覆盖布局。Web 透明背景由文档 surface
决定，CSS 控件层不创建第二个模糊层。Web 与 GPUI 共享 fonts、colors、spacing
和 control metrics，允许系统 WebKit 与 GPUI 文字栅格存在细微差异。

## 窗口

| 窗口 | 行为 |
| --- | --- |
| Launcher | 648 px 主窗口，不可缩放；关闭执行产品清理后隐藏 |
| Clipboard Shelf | 480×520 `WindowKind::Floating`，重复打开时激活已有窗口 |
| Quick Look | 系统 QLPreviewPanel，同一进程最多一个所有者 |
| Running Tasks | 按需窗口，保留任务会话和滚动位置 |
| Web / Plugin | 当前 Launcher 内的工具 surface，返回保留父查询与选择 |

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
