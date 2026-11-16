# V0.14 GPUI 技术方案与验收

状态：开发预览，功能接入完成，自动化与物理桌面证据分开记录。最后核对：
2026-11-16。VoiceOver 明确不在当前验收范围。

本页定义当前交付范围和证据边界；实现边界见
[GPUI 产品架构与迁移契约](gpui-migration.md)。

## 交付范围

| 区域 | 当前行为 | 自动化状态 |
| --- | --- | --- |
| 产品壳 | macOS 15+ 菜单栏后台应用、无边框浮动 Launcher、搜索图标菜单、全局热键 | Covered / Physical Partial |
| Launcher | Universal / Current App、Scope、菜单、查询、选择、对象动作、结果 | Covered / Physical Partial |
| 文件搜索 | Spotlight + 完整遍历、过滤、连续结果、Quick Look 路由 | Covered / Physical Partial |
| Clipboard | History、策略、独立 Shelf 窗口、复用与关闭清理 | Covered / Physical Partial |
| Settings | 热键、Context、目录、剪贴板、应用历史、Feature、语言、登录项、标题栏草稿路由 | Covered / Physical Partial |
| Application history | 18 个来源、配置与筛选、页面对象动作 | Covered |
| Tools | NativeTool 路由、Plugin Detail、未知类型错误 | Covered |
| Web plugins | 安装审查、启停、固定 origin、IPC、ack、主题与 teardown | Covered / Native Unverified |
| Packaging | `Anywhere.app`、固定 Bundle ID、签名、ZIP 和 SHA-256 | Scripted / Local Verified |

Covered 表示有代码路径和自动化回归，不表示真实窗口、键盘、系统权限或视觉
验收已经完成。

## 稳定边界

- Rust Core 拥有 Context、Action、匹配、排序、授权、执行资格和资源预算。
- GPUI 拥有页面、输入、选择、焦点、窗口和展示状态。
- AppKit、Quick Look、WebKit、状态栏、热键与登录项只作为薄系统适配器。
- Scope 过滤 Action，Context 决定适用性；Current App 聚合可用领域并包含菜单，
  菜单不单独占一个 Scope。
- 不可用菜单隐藏；缺少辅助功能权限只移除菜单，不阻断普通搜索与动作。
- 版本和身份检查在异步发布与副作用开始前复验；稳定 ID 不单独授权旧目标。
- 文件、目录、工作区文件与远程 URI 保持不同类型；不猜测工作区目录或应用。

## 自动化证据

2026-11-13 在 macOS 27.2.1（build 26B131）/ arm64、Rust 1.100.0-beta.7 上，
于 `codex/experiment-gpui-kit` 工作区完成以下验证：

| 门禁 | 结果 |
| --- | --- |
| `scripts/check-gpui.sh` | 通过；GPUI 109 项测试通过 |
| `cargo test --workspace --locked -- --test-threads=1` | 791 passed、6 ignored、0 failed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过，无 warning |
| `scripts/check.sh` | 通过；格式、生成物、573 条双语消息、文档链接、原生探测、Rust、Web SDK 与示例、架构边界全部通过 |
| `scripts/check-web-sdk.sh` | 50 项通过，包括真实 Rust broker、二进制帧、取消、隐藏 / 恢复和 1 MiB 文本传输 |
| `scripts/check-web-examples.sh` | JSON 示例 30 项通过；三个插件包、TypeScript 后台 SDK、QuickJS 大文本和工具模板构建通过 |
| `git diff --check` | 通过 |

本轮定向回归还覆盖了设置页的长中文路径输入宽度、搜索引擎本地化，以及应用历史
按来源身份和启用状态筛选。设置主体沿用独立纵向滚动区；共享开关、快捷键、Context、
搜索、剪贴板和 Feature 行按卡片分组。GPUI 组件的字体、输入框、主次按钮、列表、
选框和滚动条颜色已投影到同一套 design tokens，避免系统字体族和默认蓝紫主题
形成第二套控件外观。以上是布局结构回归，逐页像素对照仍需在解锁后的真实桌面完成。

工作区测试结果为 Core 354 通过 / 1 ignored、Core architecture 6、execution sessions
6、feature catalog 5、MVP 195 通过 / 4 ignored、search fields 3、
GPUI 103、Platform 72 通过 / 1 ignored、Plugin 34、Protocol 7 和 Protocol
reexports 1。忽略项均标注了需要外部工具或大型 fixture 的显式原因。

### 2026-11-13 异步启动与职责拆分

`LauncherView` 新增显式 `Preview / Starting / Ready / Failed` 生命周期。产品
主窗口和状态栏先以安全默认值启动，后台 `anywhere-startup` 线程打开
`ApplicationRuntime`、SQLite、插件和目录授权；Ready 后在主线程一次应用缓存
偏好、设置和热键。Starting / Failed 的菜单与快捷键只显示状态，不预留 Session
或执行动作。

导航 / activation / retention、按键 / IME 路由、Settings 草稿与保存控制器已从
根视图拆到独立模块；Launcher 根文件从约 7,200 行降到约 6,100 行。异步启动新增
“Starting 不创建 Session”回归，同时保留既有同模式恢复、设置保存、输入法和窗口
几何测试。`LiveLauncher` 最后释放时使用 background runtime shutdown，避免系统
退出等待 worker。

Release r5 隔离包在唯一 Bundle ID / 数据目录下确认：

- LaunchServices 类型仍为 `UIElement`；初始状态栏使用安全默认值出现，约 105 ms
  后替换为隔离目录中的 `⌃⌥⇧U` / `⌃⌥⇧I` 并完成热键注册；从材质准备到热键 Ready
  的日志跨度约 134 ms。
- 日志顺序为 Starting 壳和 Glass host → background runtime Ready → 状态菜单
  重标 → 全局热键注册，主线程未执行同步存储打开。
- 一次真实系统热键在该进程产生 60 ms `key=true app_active=true` 探针和
  `GPUIView` first responder。随后桌面进入锁屏，r5 的持续交互、截图与逐页视觉
  矩阵未完成；该单项不能替代 r4 的完整序列，也不能据此把 r5 全部桌面项标为通过。

最终包为
`build/Anywhere-GPUI-Refactor-Acceptance-20261113.app`，Build 87、arm64、
44 MB。严格签名、plist 与动态库检查通过；包内无 Flutter / Dart、旧桥接 framework
或非系统 dylib。独立 Bundle ID
`dev.anywhere.launcher.gpui.refactor20261113final2` 的真实启动仍为
`UIElement`，锁屏状态下 on-screen Launcher 窗口数为 0。日志确认安全默认状态栏
先出现，随后 Glass host、缓存偏好和 `⌃⌥⇧U` / `⌃⌥⇧I` 热键完成 Ready；进程环境
只包含预期的 `ANYWHERE_DATA_DIR`。锁屏阻止了最终包的真实热键、输入和截图序列，
因此最终包的 UI 物理结论只包含隐藏启动与 Ready 路径。

### 2026-11-13 原 Flutter 界面差异修复

以提交 `81c523c` 的 Flutter 源码作为固定视觉与交互 oracle，本轮逐项复核并修复：

- Launcher 搜索头恢复 72 px 几何、无额外输入内边距、loading spinner / Esc
  切换、原 Scope 与文件过滤 chip 间距；对象操作只对原始 primary target 且
  Core 返回匹配对象动作的行显示。
- Settings 的 Done 恢复 outlined 样式；动作目录默认不常驻。完整动作目录只在
  点按“添加动作快捷键”后进入模态选择，随后单独录制和弦、选择 global/local
  scope 并保存；Context shortcut 保留独立的动作规则添加控件。
- Clipboard History 的 Delete 恢复直接删除；Clipboard Shelf 恢复原五项动作，
  包括“Copy and return to app”、Preview、Delete 和 Refresh，并记录最近一个
  Regular 前台应用作为显式返回目标。
- Application History 删除原 Flutter 不存在的来源切换条，页面标题和搜索模式
  使用当前来源名称；空态使用历史图标。
- Clipboard History、Application History、Tasks、Web 包列表和 Settings 的
  固定高度行恢复 `min-width: 0`、明确行高、截断与不可收缩操作区；Tasks 页头
  保留独立 28 px titlebar inset，Settings 标题与 Save / Done 保持分离。

此前 +75 采用 HUD；当前 +79 已改用 Glass 的 contentView 承载路径。
最新 `scripts/check-gpui.sh` 103 项测试及 `scripts/check.sh` 通过，包含 Settings
根焦点回归。桌面现已解锁，窗口截图和原生 key / active 探针可以读取；当前
窗口截图仍不足以证明桌面透色，逐页物理矩阵继续保持 Partial，不能沿用旧包的
视觉结果作为新材质验收。

2026-11-13 根背景排查确认 `gpui-component` Root 仍绘制默认纯色背景；仅改
透明页面不足以露出材质。真实 GPUI 彩条 A/B 已确认清空 Root token 后玻璃
可以显示，产品修复包为 +80。桌面采样和文字对比度仍单独验收，详见
[排查记录](archive/gpui-glass-investigation-2026-11-13.md)。

2026-11-13 UI 细节修复：模式选择器 resize 改为延迟更新当前窗口；窗口轮廓
不再挤占布局空间；中英文选择器显式行高；搜索头、候选、剪贴板与任务行禁止
Flex 压缩，右侧操作固定宽度；文件搜索的重试按钮限制在状态栏高度内。
新增紧凑窗口实际尺寸、双语行内边界、长标题与右侧操作不相交、Settings
不被迟到 resize 覆盖的回归。`scripts/check-gpui.sh` 103 项测试和全仓
`scripts/check.sh` 均通过。

### 2026-11-13 UI 对齐缺陷修复（Build 95）

v92 真实桌面验收发现四项 UI 缺陷，Build 95 已完成修复并重复原序列：

- Current App 的 Scope 查询只使用 Core 已按 query/Scope 过滤的 `candidates`；
  空查询和菜单浏览仍使用完整 Scope offers。Finder 当前应用输入 `open` 后窗口
  为 648×229（顶部 y=312）并只显示 2 条匹配动作，输入 `zzzz` 显示空态，不再
  保留初始 4 条结果。
- Settings 样式切换完成后恢复 `GPUIView` first responder；返回 Launcher 的
  window style 在异步完成后再次恢复搜索编辑器。Done 和 Esc 返回后直接键入
  字母均进入查询框，修复了 v92 的 `focused=0`。同步修改 window style 的旧方案
  已撤销，避免 GPUI `RefCell already borrowed` 重入。
- 未保存关闭确认框改用 100% 不透明的 `menuSurface` 和更强的 `modalScrim`；
  实测对话框主体为 RGB 43/46/53，底层设置内容不再透入。标题栏完整显示
  `Some changes were not saved`，Save changes 与 Done 不重叠。
- Launcher 根画布增加 40% 黑色内容对比 scrim，原生 Glass 仍为
  `tintColor=nil`。白底实际合成背景采样 RGB 73–82：primary 7.35–8.47:1、
  secondary 6.08–7.00:1、muted 5.61–6.47:1；彩色渐变采样背景 RGB 98–140：
  primary 7.30–9.14:1、secondary 6.03–7.56:1、muted 5.57–6.98:1；语义图标
  最低约 4.47:1。Increase Contrast 将 scrim alpha 提高到约 70%。Build 165
  后续把这一历史黑色遮罩改为与 Flutter 一致的约 63% 中性灰
  `contentScrim`（`#a0303438`）；Build 167 真机发现窗口高度变化仍会改变原生
  Glass 整体亮度，因此最终提高为约 93% 中性灰（`#ed303438`），Increase
  Contrast 为约 95%。

证据目录为
`build/acceptance-gpui-ui/v95-20261113-190205/`，Build `0.14.0+95`、Bundle ID
`dev.anywhere.launcher.gpui.uiplanv95`。关键截图是
`screenshots/current-query-open.png`、`screenshots/current-query-empty.png`、
`screenshots/p11-settings-dirty-close.png`、`screenshots/p08-white-crop.png` 和
`screenshots/p08-color-crop.png`。`scripts/check-gpui.sh` 103 项、全仓
`scripts/check.sh`、格式与生成物检查均通过。Settings 保存失败、浅色系统外观和
Reduce Transparency 仍未覆盖，继续保持 Pending。

### 2026-11-14 UI 对齐后续缺陷（Build 96–98）

Build 95 后继续按固定 Flutter 界面和真实桌面序列复核，又修复四类用户可见问题：

- Global Search 已有非空查询时切回 Current App，Global 的 Context 快照中没有
  Current App Scope；旧实现只记录迟到补选，而该补选又因查询非空永不执行。
  Build 98 将显式模式切换与入口默认补选分开：已有 Scope 时立即选中，Scope
  迟到时在 Environment 快照到达后补选并重新查询。等待新 Scope 结果期间隐藏旧
  Global 行，避免选中态与结果不一致或回车执行旧目标；Environment 已完成且确实
  没有应用 Scope 时结束等待并显示空态，不伪造 Scope。
- Settings 的 App History 页此前从不请求来源目录，只显示未检测到来源。现在
  打开 Settings 时若停在 App History，或点击该分类，会读取真实来源；该响应
  不会误启动隐藏的 Application History 页面查询。
- Settings 分类归属按 Flutter 对齐：Clipboard capture 放入 Clipboard 页并显示
  “When opening Launcher”和 15 秒自动填入说明；隐藏应用放入 Search 页；
  Browser current page 使用专属说明；Search 与 Features 使用各自页头文案和
  “Built In”分组。
- Launcher 的通用加载态改用 `Searching…`，不再把普通 Action / Scope 查询误标
  为 `Searching files…`。

最终验收包为
`build/acceptance-gpui-ui/v98-20261114-005135/Anywhere-GPUI-UI-Alignment-v98.app`，
Build `0.14.0+98`、Bundle ID
`dev.anywhere.launcher.gpui.uiplanv98`，使用独立 `data/` 与
`⌃⌥⇧J` / `⌃⌥⇧K`。Finder 前台输入 Global `open` 后切回 Current App，最终
AX 树和 `02-global-open-back-current.png` 只包含 `Open Directory` 与
`Open Terminal Here`，ChatGPT 被 Core 的 Current App 过滤移除；未选中任何
Global 应用行。`03-settings-app-history.png` 显示本机真实来源与启用状态，
`04-settings-context.png`、`05-settings-clipboard.png`、
`06-settings-search-top.png`、`07-settings-search-bottom.png` 和
`08-settings-features.png` 覆盖本次 Settings 对齐项。Done 与 Esc 返回后直接
键入 `d` / `q` 均追加到 Launcher 查询，分别为 `opend` 与 `opendq`。

最终窗口在 `open` 查询下为 `648×173`、`x=956 y=312`；CUA 窗口截图位于同一
目录。日志 `logs/validation-timeline.txt` 记录了 Global 结果、Current App
Scope 补选、重新查询以及最终仅含 Action 的快照顺序。`scripts/check-gpui.sh`
109 项、全仓 791 passed / 6 ignored / 0 failed、`scripts/check.sh`、格式、
生成物、文档链接与严格签名均通过。未执行 Settings 保存失败注入、来源开关
写盘、浅色系统外观、Reduce Transparency 或 Increase Contrast，
这些项目继续保持 Pending，不能由本轮的深色桌面截图替代。

### 2026-11-14 Application History 与工具浮层（Build 99–106）

Build 98 后继续按真实应用来源复核，发现 Cursor / IDEA / VS Code 的
Application History 都停在空列表，日志只有 `application_history_session_started`。
Build 100 的阻塞样本确认根因是 GPUI host 持有 `ApplicationRuntime.settings`
互斥锁时又调用 `app_history_sources_from` 读取同一锁：调用链为
`open_app_history_session → app_history_sources_from →
ApplicationRuntime::settings → pthread_mutex_lock_wait`。Build 101 先释放
settings guard 再读取来源，并新增 2 秒超时回归，旧实现会稳定超时而不是让
测试永久挂住。

Build 101 使用隔离数据实机确认 VS Code 来源加载 67 条历史，搜索 `Anywhere`
得到 4 条匹配；随后返回 Launcher 再进入 Cursor 与 IDEA 来源，均能显示已记录
项目。截图位于
`build/acceptance-gpui-ui/v101-20261114-070000/screenshots/10-application-history-vscode.png`
和同目录 `11-application-history-filter.png`，日志位于同目录
`logs/stderr.log`。该包为 `0.14.0+101`、Bundle ID
`dev.anywhere.launcher.gpui.uiplanv101`。

继续对 IDEA 条目按 `⌘K` 时，先得到空 Actions 和通用失败提示。AX 日志显示当时
验收 fixture 目录已被删除，Core 的 `history target unavailable` 拒绝是正确
的安全边界；恢复合成目标并刷新后，菜单返回 Copy Path、Git、Developer 等
19 个动作。这里仍暴露出两个 UI 问题：失败后空浮层没有关闭；浮层使用
`absolute + top/right` 放在滚动内容中，真实位置越出窗口。单元布局在旧实现上
测得 History 面板为 `x=-12 y=792`，而窗口为 `760×520`，与桌面截图的裁切一致。

Build 102 的修复：

- 新增 `page_popup`，使用 `deferred + anchored(TopRight)` 在窗口坐标绘制并
  `snap_to_window_with_margin(8)`；History、Clipboard History 和 Running Tasks
  的页面操作面板统一走该入口。
- 对象动作失败会撤销请求、清空动作并关闭空浮层；再次请求前清除旧失败提示。
  设置或历史服务不可用时也发送带请求身份的失败事件，不留下 loading 面板。
- 新增 History 浮层边界回归、Clipboard 浮层边界回归、Tasks 工具面板边界回归
  和会话锁超时回归；当前 `scripts/check-gpui.sh` 的 GPUI 单元测试为 112 项。

Build 102 包位于
`build/acceptance-gpui-ui/v102-20261114-072500/Anywhere-GPUI-UI-Alignment-v102.app`，
为 `0.14.0+102`、Bundle ID `dev.anywhere.launcher.gpui.uiplanv102`，使用
`⌃⌥⇧J` / `⌃⌥⇧K`。Build 102 启动后桌面锁屏，因此当时没有把浮层布局测试
当作物理完成。

Build 103 解锁后先修正验收包的独立 Bundle ID 和显示名，再用 `/private/tmp`
数据副本避开 Documents 自动化提示。History、Clipboard 和 Tasks 的浮层都
确认锚定在窗口内，但 `theme::card()` 的 `0x78` alpha 让底层行文字透入。
三个页面级浮层随后统一改为完全不透明的 `theme::menuSurface()`；对应的
History、Clipboard 和 Tasks 边界 / 状态断言保留在 GPUI 测试中。

继续从**隐藏 Launcher**经真实状态栏打开 Settings，修改草稿、放弃关闭后，
Build 103 暴露出 Launcher 只恢复 648×72 外观和焦点，查询流仍处于取消状态：
输入 `vscode` 后没有快照，窗口也不再展开。Build 104 在退出 Settings 时按
查询流状态选择保留、15 秒内恢复或重新激活 Session，并新增
`leaving_settings_keeps_resumes_or_restarts_the_launcher_stream` 回归。
Build 106 的同一真实序列最终得到 648×137 和 VS Code 历史动作，随后可进入
760×520 Application History。

Build 105 又从真实状态栏 `Running Tasks…` 发现新窗口虽已创建，却以普通层级
留在前台应用后面，用户实际看不到。Build 105 先增加菜单关闭后的延迟激活；
Build 106 进一步在 AppKit 声称已请求激活但应用仍非 active 时走
LaunchServices 兜底。最终构建中 920×600 Tasks 窗口直接可见，Tool options
面板锚定在工具栏下，`⌘F` 进入任务窗口事件链。

最终验收包：

- `build/acceptance-gpui-ui/v106-20261114-091406/Anywhere-GPUI-UI-Alignment-v106.app`
- `0.14.0+106`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv106`。
- 证据目录同级的 `VALIDATION.md`、`metadata.env`、`screenshots/` 和 `logs/`。
- `10-vscode-results-after-settings.png` 证明 Settings 返回后的查询通道；
  `11-history-actions.png`、`12-clipboard-actions.png` 证明不透明浮层和右缘
  安全边距；`02-running-tasks-visible-active.png`、
  `04-running-tasks-options.png` 证明状态栏 Tasks 可见且选项浮层可达。
- `scripts/check-gpui.sh` 113 项、全仓 795 passed / 6 ignored / 0 failed、
  `scripts/check.sh`、workspace clippy、格式、生成物、文档链接和严格签名均通过。

Build 107 进一步固定窗口层级契约。Launcher 与 Clipboard Shelf 是仅有的
floating level / all-Spaces 窗口：Launcher 失焦隐藏；Shelf 失焦不
隐藏，只在明确关闭入口结束。Settings 与 Running Tasks 切换为 normal level、
默认 Space 行为和可见原生交通灯，不再随 Launcher 的无边框配置漂浮。Shelf
使用专用原生配置，不再复用 Launcher 的 borderless / nonactivating style；
空列表也恢复 Flutter 的省略号操作入口，打开后仅 Refresh 可用。窗口创建和
AppKit 样式 / 层级配置分到不同 update，避免新窗口 resize 回调重入 GPUI；
Shelf / Tasks 的系统关闭也改为延迟 `remove_window`，不再在关闭后记录
`window not found`。Build 107 当时把 Launcher 根页 Esc 暂时统一为一次隐藏；
该判断后来确认与 Flutter 不符，并在 Build 109 恢复完整返回链。

Build 107 的 `scripts/check-gpui.sh` 为 117 项通过。解锁后的物理验收确认：
Launcher 和 Shelf 的 CoreGraphics layer 为 3，Settings / Tasks 为 0；
TextEdit 获得焦点后 Launcher 隐藏，Shelf 保持 layer 3；Settings / Tasks
保持打开但排到 TextEdit 之后；Settings 返回后 Launcher 恢复 layer 3；Tasks
最小化后可由状态栏重新打开。Shelf 与 Tasks 的原生交通灯均可操作，关闭后无
运行错误。该轮物理证据不包含正确的根页 Esc 链。

Build 109 继续按 Flutter 源码补齐交互差异。Launcher 根页恢复
Content → Current App → 空白 Global → 隐藏链，普通查询先清空；Shelf 使用
鼠标所在屏幕右下角的 16 点内边距定位，重复打开会恢复最小化窗口；“复制并
返回”只有在完成时仍持有 Shelf key window 才切回原应用。Clipboard History 与
Application History 的空结果恢复页头操作菜单；History 不可打开条目提供
Copy / Refresh，零对象动作的可打开条目仍提供 Copy / Refresh，对象 Actions
末尾固定 Refresh。Web Tool 恢复宿主省略号
Actions，提供 Reload Tool 与 Back to Search，并在 teardown 时清除旧文档
状态。

最终候选：

- `build/acceptance-gpui-ui/v109-20261114-130007/Anywhere-GPUI-UI-Alignment-v109.app`
- `0.14.0+109`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv109`。
- `scripts/check-gpui.sh` 123 项；全仓 805 passed / 6 ignored / 0 failed；
  `scripts/check.sh`、workspace clippy、格式、生成物、文档链接和严格签名均通过。
- 最终进程 PID `28167` 已以隔离数据启动，LaunchServices 报告 `UIElement`，
  CoreGraphics 无可见窗口，TextEdit 前台未改变；日志无 ERROR、RefCell 重入、
  `window not found` 或 panic。

Build 109 的物理复验为 **Pending**：验收机随后再次锁屏，Computer Use 无法
控制 TextEdit / 验收窗口，自动解锁被系统暂停。因此本轮只记录启动冒烟、静态
签名和自动化结果，不把上述新 UI 行为写为物理 Pass。完整序列见
`build/acceptance-gpui-ui/v109-20261114-130007/VALIDATION.md`。Build 107 的
物理层级结果仍是历史证据，不能替代 Build 109 的同序列复验。

浅色系统外观、Reduce Transparency、Increase Contrast、真实输入法
完整组合矩阵、Settings 保存失败注入和 WebView helper 回收仍未在本轮完成，
继续保持 Pending / Partial。

Build 121 继续修复解锁后物理复验发现的生命周期与页面缺陷：

- Shelf / Tasks 最小化后复用同一原生窗口不再触发 GPUI 借用 panic；干净
  Settings 在主 Launcher 热键下真实退出，脏草稿则保留窗口并展示保存确认。
- Tasks 或 Clipboard Shelf 为 key window 时按 Universal 热键，Launcher 在
  58–61 ms 内成为 `layer=3`、`key=true` 并聚焦输入框。Shelf 选中记录后
  Command-Return 成功写入剪贴板并激活此前记录的 TextEdit，Shelf 保持置顶；
  日志无 `clipboard_shelf_return_destination_unavailable`。
- Web Packages 按普通 Settings 面处理，原生文件选择 sheet 出现时窗口和 sheet
  都保持可见。导入改为 `.awplugin` 文件而不是目录；审核态独占可滚动正文，
  Cancel 与 Approve 完整落在 648×480 窗口内，Approve 安装成功后返回包列表，
  Esc 返回 Settings。
- 修复 Web 插件从空白 `ENTRY_CONTEXT` 启动时把占位 `session_id=0` 写入
  invocation 的缺陷。Web invocation 现在显式绑定真实执行 Session，
  `state.subscribe` 不再返回 `INACTIVE`；Text Pad 能加载、接受 WebView 键盘输入、
  更新字符数，并可从宿主 Actions Reload 或返回原 Launcher 工作区。
- Settings 焦点从无 ID 的外层容器移到已有 `settings-surface` ID 的
  Application Group。真实 AX 树无缺少节点的警告，Tab 仍进入 General，分类、
  Done 和快捷键控件均继续暴露。

Build 121 候选为
`build/acceptance-gpui-ui/v121-20261114-180228/Anywhere-GPUI-UI-Alignment-v121.app`
（`0.14.0+121`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv121`）。构建通过
严格签名，LaunchServices 报告 `UIElement`。`scripts/check-gpui.sh` 为
126 passed；全仓 `808 passed / 6 ignored / 0 failed`；`scripts/check.sh`、
格式、生成物、文档、workspace clippy、Web SDK / examples 与架构检查均通过。
该构建的 Settings、Web Tool、IPC ack、实际 Web 输入、宿主 Actions 和返回路径
已完成物理验证，证据在对应 `VALIDATION.md`、`screenshots/` 与 `logs/`。

多 Space / fullscreen、完整 IME 组合矩阵、Settings 保存失败注入、Tasks 实际
输出 / Follow、Quick Look 媒体、旧 IPC 替换与 helper 回收、浅色 / 辅助功能
外观、登录项及 Accessibility 撤销 / 重新授权仍是 Pending。

Build 123 修复解锁后 v122 物理复验发现的 WebView Esc 缺陷：

- 根因是 WKWebView 首先消费聚焦页面的原生 `keyDown`，宿主的 AppKit 本地
  事件监听没有收到该 Esc，因此既未产生 `PlatformEvent`，也未退出 Web Tool。
- Web bootstrap 现在在捕获阶段监听无修饰、非重复、非输入法组字态的 Esc，
  通过固定 host frame 走现有 IPC。Rust 只接受与当前 document 句柄匹配的
  frame，再转到原来的页面返回逻辑；旧文档不能关闭替换后的页面。原生监听器
  保留为第二路径，输入法组字仍优先交回 WebKit。
- 物理复验中 Text Pad 的 AX 聚焦节点为 `AXTextArea`，真实键入后值为
  `text pad hello`；按 Esc 后日志记录 `web_view_escape_requested` 与
  `web_surface_closing screen=WebTool`，窗口从 920×600 回到 648×137，
  `text pad` 查询和 Launcher `layer=3` 保持不变。重复打开 / 输入 / Esc
  通过。
- 同包复验窗口契约：Launcher 失焦隐藏；Settings 与 Running Tasks 均为
  `layer=0` 普通窗口；Clipboard Shelf 为 `layer=3`，切到 TextEdit 后保持，
  点击交通灯关闭后才消失。Current App 空白根页仍是第一次 Esc 切到 Global、
  第二次 Esc 隐藏，没有改动这一正常行为。

最终候选为
`build/acceptance-gpui-ui/v123-20261114-183828/Anywhere-GPUI-UI-Alignment-v123.app`
（`0.14.0+123`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv123`）。构建通过
严格签名。`scripts/check-gpui.sh` 为 128 passed；全仓
`810 passed / 6 ignored / 0 failed`；`scripts/check.sh`、格式、生成物、
文档、workspace clippy、Web SDK / examples 与架构检查均通过。物理证据在
对应 `VALIDATION.md`、`screenshots/` 与 `logs/`。

2026-11-13 重构验收包
`build/Anywhere-GPUI-Refactor-Acceptance-20261113-r4.app` 再次通过 Release
构建、严格签名、plist、arm64 与系统动态库检查；包内未发现 Flutter / Dart
runtime、旧桥接 framework 或非系统 dylib。使用唯一 Bundle ID 和独立数据目录
真实启动后，LaunchServices 报告 `UIElement`，应用为 background-only、窗口数
0，进程环境只包含预期的 `ANYWHERE_DATA_DIR`（没有 `ANYWHERE_PREVIEW`），
热键注册和状态菜单构造成功。日志确认 `mode=Glass`、
`NSGlassEffectView`、内容层非 opaque、窗口非 opaque，材质 host 与 18 px
窗口圆角一致。

桌面解锁后的物理序列还确认：

- Current App 空态为 `648×72`、顶部 `y=312`；真实输入英文 `e` 后为
  `648×473`，顶部仍为 `y=312`，显示 7 条结果。清空 / 切换模式后保持紧凑
  高度；真实热键从事件到可见约 51–80 ms，60 / 280 ms 探针均为
  `visible=true key=true app_active=true`，first responder 为 `GPUIView`。
- 模式选择按钮展开为 `648×181`，Global Search 与 Current App 两行标题、说明
  和选中标记完整分离，没有遮挡；点击后能切回对应模式。
- 中文 Pinyin 输入 `nihao` 后由候选窗提交“你好”，窗口为 `648×137`，
  显示匹配项且顶部保持 `y=312`。截图同时保留系统组字 / 候选过程，证明真实
  IME 事件进入当前 GPUI editor。
- 同一模式热键可隐藏并在 15 秒内重新显示，输入 `e`、7 条结果与 `648×473`
  状态均恢复。切换到 Finder 后约 227 ms 完成失焦宽限并隐藏。
- Settings 为 `648×508`；General、Context、Search、Clipboard、App History、
  Features 六页实机检查未见标题、说明、状态和右侧操作重叠。Done 为 outlined
  button，Action shortcuts 默认折叠，只显示“Add action shortcut”和
  “Add context shortcut”。白色 / 深色背景截图都显示桌面内容透过 Glass，
  透明 GPUI 根画布没有再盖住原生材质。

上述截图和窗口 bounds 是 r4 物理证据；Reduce Transparency、
Increase Contrast、浅色系统主题、WebView helper 回收和完整 macOS 15 材质
仍为 Pending / Partial。

+84–+87 的材质 tint 与 GPUI 深色外框方案已按用户要求撤销，不能作为当前视觉
实现的验收证据。当前实现仅使用原生 Regular Glass（`tintColor=nil`），继承
系统外观，GPUI 根画布 alpha=0，不绘制窗口级蒙层、描边或阴影。此前模式选择器
的几何修复保留；原生高光与白底文字仍需按未着色构建重新观察。

+88 已通过 `scripts/check-gpui.sh`（90 项）、`scripts/check.sh` 和
`scripts/build-macos.sh` 本地签名构建。隔离目录日志确认 `LiquidGlass`、
`NSGlassEffectView`、18 px 圆角，窗口与内容层均非 opaque。CUA 可以读取
白底浏览器，但绑定 UIElement 验收应用与状态栏入口持续超时，未能获得该版本
窗口截图；原生高光、文字清晰度与 Spotlight 截图一致性仍为 Pending，
不以编译或安装日志代替视觉验收。


+82 / +83 解锁实机通过 CUA 检查英文与中文模式选择器展开为 180.5 px，两条
记录标题、说明及选中标记完整可见，关闭选择器后恢复完整候选列表。+83
`⌘,` 能进入 Settings 并切换原生窗口样式；General / Shortcuts / Context /
Search 的英文布局，以及中文 General / Shortcuts、dirty 提示、保存和返回已
检查，没有观察到标题、说明与操作区重叠。该实例使用隔离目录和
`ANYWHERE_PREVIEW=1`，只能作为这些 UI 状态的物理证据，不能替代真实 Core
数据、所有工具页面或桌面背景透色矩阵的验收。

当前自签发布包验证：

```text
scripts/build-macos.sh
0.14.0+1 -> build/Anywhere.app

codesign --verify --deep --strict --verbose=2 build/Anywhere.app
valid on disk; satisfies its Designated Requirement

scripts/package-macos.sh build/Anywhere.app
build/Anywhere-macos-arm64.zip
build/Anywhere-macos-arm64.zip.sha256

shasum -a 256 -c build/Anywhere-macos-arm64.zip.sha256
OK

# 当前候选
8d0cfde8ab2f33f092ec2a0b0fc9a15f787e4d0704517ef14cafdc0003db3c59
Anywhere-macos-arm64.zip
size=16,084,670 bytes
```

包结构必须满足：

- `CFBundleIdentifier = dev.anywhere.launcher`
- `LSMinimumSystemVersion = 15.0`
- `CFBundleExecutable = Anywhere`
- 主程序存在且签名有效
- 不存在其他 UI 框架或跨语言桥接运行库

本次包已逐项核对：可执行文件为 arm64 Mach-O `Contents/MacOS/Anywhere`；
`otool -L` 只列出 macOS 系统框架与系统库；包内只有主程序、图标、许可、
双语原生文案，没有其他 UI framework 或跨语言桥接运行库。

### 2026-11-14 滚动与任务窗口修复（Build 126–129）

Build 126 发现两个共享滚动缺陷。第一，页面的 `ScrollHandle` 被
`overflow_y_scrollbar()` 内部句柄覆盖，导致 `maximum` 虽可读但选中项 reveal、
Tasks 跟随和 File Search 触底分页实际使用错误状态；各长列表改为
`.overflow_y_scroll()` + `.vertical_scrollbar(&self.handle)`。第二，File Search
和 Tasks 曾用 `offset - maximum` 判断距底部距离，现统一为
`offset + maximum <= 24px`。Tasks 新输出会等待下一帧布局后落到
`-max_offset.y`，用户向上滚动后才切换 Latest。

长列表 reveal 还需要跳过组件插入的滚动条覆盖层：在 GPUI Fast 的子项布局中，
该绝对定位子项位于索引 0，业务行从索引 1 开始。Build 127 的物理序列在
Clipboard History 连续按 20 次 Down，选中 fixture 已滚入窗口；Build 129
在同一构建契约下再次通过。`file_search_selection_reveals_rows_in_a_long_tracked_list`
和 `tasks_output_follow_tracks_the_shared_scroll_handle` 固定了两个行为。

Build 127 的物理任务复验又发现两个 Flutter 行为差异：

- 进程 Action 返回 `execution_session_id` 后，结果处理先调用 `prepare_hide()`
  递增流 generation，再检查 `OpenTasks`，事件被自己判旧而吞掉；Launcher 隐藏，
  Tasks 窗口却没有自动出现。Build 128 在 dismissal 前完成结果路由并发出任务
  事件，Cargo Test 启动后自动复用/创建 `layer=0` 的 920×600 Running Tasks，
  复制输出得到 82,495 bytes。
- 已有 Tasks 窗口时再次启动任务，只激活旧窗口，没有使用新的
  `execution_session_id`，详情仍停在上一任务。Build 129 将任务 ID 从
  `LauncherEvent::OpenTasks(String)` 传到窗口创建/复用，并在下一次 Tasks
  快照中优先恢复该 ID。物理序列启动 s03 后，再启动 s04：窗口复用、s04 被选中
  且详情显示 Running / Stop；Stop 后 `npm run s04`、shell 与 `sleep 30`
  三个进程全部消失，UI 显示 Stopped。

v129 还复验了窗口与滚动契约：

- Launcher `layer=3`，切到 TextEdit 后隐藏；根页第一次 Esc 保留 648×72 根页，
  第二次 Esc 隐藏，未改 Current App 空白根页的原有返回链。
- Settings 与 Running Tasks 均为 `layer=0`，切到 TextEdit 后保持，不随
  Launcher 一起消失。
- Clipboard Shelf 为 `layer=3`，切到 TextEdit 后保持；只有点击原生关闭按钮后
  才消失。
- Tasks 的 Cargo Test 输出增长时保持 Following；验收工具向上滚动后按钮变为
  Latest，点击后恢复 Following 并回到输出底部；复制输出得到 82,590 bytes。

最终候选为
`build/acceptance-gpui-ui/v129-20261114-220545/Anywhere-GPUI-UI-Alignment-v129.app`
（`0.14.0+129`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv129`）。严格签名、
plist、arm64 和 CDHash `6c0d8ab8...` 已核对。`scripts/check-gpui.sh` 为
133 passed；Build 129 的 workspace 为 815 passed / 6 ignored / 0 failed，
`scripts/check.sh`、格式、生成物与文档门禁全部通过，详见该目录
`VALIDATION.md`。窗口层级、任务自动打开/选择、Stop、Latest、长列表 reveal
已有物理证据；Reduce Transparency、浅色主题、登录项、多 Space
和完整 IME 矩阵继续保持 Partial / Unverified。

### 2026-11-14 工具页边界、Tasks 筛选与 History 生命周期（Build 130–131）

继续逐项对照 Flutter oracle 后，Build 130–131 修复了以下可确定差异：

- Clipboard History / Shelf、Application History 和 Tasks 的方向键选择在末项 /
  首项保持不动。Launcher 主候选仍按 Flutter 首尾回绕，两者不是同一导航层。
- Application History 刷新按 `object_key` 保留选中身份，并在新快照布局后 reveal；
  查询变化才重置到首项。任务列表仍按稳定任务 ID 保留选择，已选任务消失时停在
  原位置夹取后的相邻项，而不是跳到最后一项。
- 从 Action 每次打开 Application History 都开始新的页面会话，清空上一次查询和
  来源行；查询或来源错误清空旧行，避免把过期对象继续显示为可执行结果。
- Application History 的 Esc 和页面右上角 Esc 控件先清空查询，再退出页面；
  页面返回箭头仍直接返回。Tasks 在同一选中项上重复按方向键不再清空并重载详情。
- Tasks 筛选按空白分词，每个词独立匹配标题或目录，不再要求整个筛选串连续出现；
  筛选隐藏原选中任务时保留可见位置的相邻项。新启动任务被旧筛选遮住时清空
  筛选，复用 Tasks 窗口仍能选中并显示新任务。

新增回归：

- `app_history_refresh_preserves_identity_and_a_new_query_resets_selection`
- `application_history_escape_clears_the_query_before_leaving`
- `application_history_escape_control_uses_the_same_query_chain`
- `opening_application_history_starts_with_a_fresh_query_and_rows`
- `task_rows_keep_the_nearest_position_when_the_selected_task_disappears`
- `task_refresh_keeps_selection_on_a_visible_filtered_row`
- `task_navigation_at_a_boundary_keeps_the_current_detail`

`scripts/check-gpui.sh` 为 140 passed；workspace 为 822 passed / 6 ignored /
0 failed，`scripts/check.sh`、格式、生成物和文档门禁通过。候选为
`build/acceptance-gpui-ui/v131-20261114-234709/Anywhere-GPUI-UI-Alignment-v131.app`
（`0.14.0+131`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv131`），严格签名、
plist、arm64 和 CDHash `83d6b0b0...` 已核对。

v131 的真实桌面结果：

- Current App 热键在 87 ms 内显示 `648×72`、`layer=3` 的 Launcher，60 / 280 ms
  均为 `key=true app_active=true`；切到 TextEdit 后隐藏。空白 Current App 根页
  第一次 Esc 仍切到 Global，第二次 Esc 隐藏，原返回链未改。
- Settings 为 `648×508`、`layer=0`；Tasks 为 `920×600`、`layer=0`。两者在切到
  TextEdit 后保持，不随 Launcher 隐藏。
- Clipboard Shelf 为 `480×520`、`layer=3`，定位屏幕右下角；切到 TextEdit 后
  保持，点击原生关闭按钮后才消失。
- Cursor Application History 以空查询、首项选中打开。按 5 次 Down 后选中第 6 条，
  `⌘R` 重建会话后仍保持同一 `object_key`。输入 `iscm` 后，真实 Esc 和右上角
  Esc 控件都只清空查询并保留 `760×520` 页面；返回箭头直接回 Launcher，再次
  打开时旧查询和旧来源行没有复现。
- Clipboard History 连续 40 次 Down 停在最后一项，再按 Down 仍不回到首项；
  连续 Up 回到首项。
- Tasks 既有窗口在筛选 `s04` 时启动 s04，任务可见并保持该筛选；随后改为 `s03`
  筛选再启动 s05，窗口复用后筛选清空，s05、s04、s03 全部可见且 s05 选中。
  `npm s05` 多词筛选只保留 s05；无筛选时从 s05 连续 Down 到末项 s03，再按 Down
  仍停在 s03，详情不跳回其他任务。

证据在 `build/acceptance-gpui-ui/v131-20261114-234709/` 的 `VALIDATION.md`、
`logs/` 和 `screenshots/`。Reduce Transparency、浅色主题、登录项、
多 Space、睡眠唤醒和完整 IME 矩阵继续保持 Partial / Unverified。

### 2026-11-15 Spotlight 激活隔离与 Tasks 全局快捷键（Build 132–136）

真实桌面复现了 Launch Services 的窗口联动缺陷：Shelf 原生最小化后只按
Universal 热键，Launcher 出现时 Shelf 也被恢复。逐层定位确认：

- `ActivateAllWindows` 会恢复无关窗口，但仅清空 options 仍不够。
- 应用激活已由 `activateWithOptions(empty)` 异步完成；紧接着重复调用
  `NSApplication.activate()` 和 LaunchServices open 会在约 0.5 秒后再次激活
  App，把最小化 Shelf 一并拉起。
- Launcher 必须在 App 激活前成为 key，激活时才会优先选择它；从状态栏菜单
  打开 Tasks 时 `activateWithOptions` 可返回 false，需要在 LaunchServices
  完成激活后补一次 120 / 320 ms key-window 恢复。

最终实现不再同步重复激活整个 App；只在需要 LaunchServices 兜底时延迟恢复
目标任务成为 key。这样 Launcher 仍能在 60 / 280 ms 内收键，Tasks 能从其他
前台应用的菜单栏正常置前，同时不会恢复无关最小化窗口。

同一轮对照 Flutter 根级 `CallbackShortcuts` 后，修复了空 Tasks 无焦点控件时
`⌘K`、`⌘L`、`⌘.`、`⌘R`、`⌘F`、`⌘⌥↑↓` 和 Esc 被拦截器记录后直接丢弃的
问题。这些 Tasks 快捷键现在不依赖搜索框或结果行是否持有焦点。

Build 136 的真实桌面结果：

- Shelf 为 `480×520`、`layer=3`；原生最小化后 0.2 / 1.2 / 4.2 秒均不可见。
  切到 TextEdit 后只按 Universal 热键，80 / 280 / 780 ms 均只有
  `648×137 layer=3` Launcher（保留查询），Shelf 没有恢复。再次显式执行
  Shelf 可正常 deminiaturize 并成为 key。
- Shelf 保持 `layer=3`，切到 TextEdit 后仍存在，仅点击原生关闭按钮后消失。
- Launcher 为 `layer=3`，切到 TextEdit 后隐藏；Current App 空白根页的
  “第一次 Esc 切 Global、第二次隐藏”契约未改。
- 从 TextEdit 前台经状态栏打开 Tasks 后，日志记录 `key=true app_active=true`；
  120 / 320 ms key 恢复均成功。空 Tasks 按 `⌘L` 后出现 Launcher 且输入框
  `focused=true`。
- Settings `648×508` 与 Tasks `920×600` 均为 `layer=0`，切到 TextEdit 后
  两个普通窗口都保持存在。

新增回归：

- `launcher_activation_does_not_restore_unrelated_minimized_windows`
- `requested_activation_does_not_immediately_reactivate_the_application`
- `tasks_global_shortcuts_do_not_require_a_focused_control`

`scripts/check-gpui.sh` 为 143 passed；workspace 为 825 passed / 6 ignored /
0 failed，`scripts/check.sh`、格式、生成物和文档门禁通过。最终候选为
`build/acceptance-gpui-ui/v136-20261115-073843/Anywhere-GPUI-UI-Alignment-v136.app`
（`0.14.0+136`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv136`），严格签名、
plist、arm64 和 CDHash `97ccd836...` 已核对。VoiceOver 已从本轮验收范围排除；
Reduce Transparency、浅色主题、登录项、多 Space、睡眠唤醒和完整 IME 矩阵
继续保持 Partial / Unverified。

### 2026-11-15 Tasks 复用激活与输入法事件审计（Build 137–139）

源码审计又确认两个竞态/路由问题：

- Tasks 窗口已存在时，状态栏入口先在 `open_tasks_window` 内立即激活一次，80 ms
  后菜单处理又激活一次。两次激活都可能在后台菜单上下文中返回 true 但保持
  `app_active=false`，不能把窗口置前。
- 新加入的 Tasks 根级 Esc 路由会先于 `CharacterInput` 检查；组合输入中的 Esc
  必须继续归输入法，只有非 CharacterInput 的裸 Esc 才由 Tasks 页面处理。

Build 139 将状态栏复用 Tasks 的路径改为只由菜单关闭后的单次激活负责；Action
复用窗口仍保留窗口内部激活。Tasks 裸 Esc 现在尊重 CharacterInput，命令组合键
仍不依赖搜索框焦点。

自动化门禁仍为 `scripts/check-gpui.sh` 143 passed、workspace 825 passed /
6 ignored / 0 failed，`scripts/check.sh` 和文档检查通过。候选为
`build/acceptance-gpui-ui/v139-20261115-080358/Anywhere-GPUI-UI-Alignment-v139.app`
（`0.14.0+139`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv139`），严格签名、
plist、arm64 和 CDHash `2f4fa987...` 已核对。

解锁后的真实桌面复验通过：

- TextEdit 前台时从状态栏创建并再次复用 Tasks。复用路径只有一次
  `auxiliary_window_activation_finished`，120 / 320 ms 均为
  `key=true app_active=true`，没有再出现重复激活竞争。
- Tasks 中真实启用拼音并键入 `nihao`，候选窗出现后按 Esc 只取消组合；
  Tasks 保持 `920×600 layer=0` 和焦点，没有清空筛选或退出页面。
- Tasks 原生最小化后，只按 Universal 热键时 80 / 280 / 780 ms 均只有
  `648×72 layer=3` Launcher，Tasks 保持最小化；再从真实状态栏菜单打开
  Tasks 可 deminiaturize 同一窗口，并在 120 / 320 ms 取得 key。

证据见该目录的 `VALIDATION.md`、`logs/` 和 `screenshots/`。

### 2026-11-15 Tasks 空页隐藏焦点与窗口契约（Build 140–143）

后续源码审计发现，`LauncherView::build` 在返回前无条件聚焦 Launcher 主
搜索框。Tasks 窗口随后切换为 Tasks 页面，但隐藏编辑器仍可能安装平台
text-input handler；空 Tasks 没有可见筛选框，键盘输入仍可能进入输入法。

最终实现不再让 `build` 隐式聚焦编辑器，而由窗口入口显式选择初始焦点：
Launcher 聚焦主搜索框，Clipboard Shelf 聚焦自己的搜索框，Running Tasks
不安装任何隐藏编辑器焦点。Tasks 的 `⌘F` 只在筛选框实际渲染时聚焦；
任务列表从非空变为空时会清除已移除筛选框的焦点。

新增回归：

- `tasks_search_shortcut_only_focuses_the_visible_filter`
- `removing_every_task_blurs_the_hidden_filter`
- `tasks_window_does_not_keep_a_hidden_launcher_editor_focused`

Build 143 自动化门禁为 GPUI 146 passed、workspace 828 passed / 6 ignored /
0 failed；`scripts/check.sh`、Clippy、格式和文档检查通过。真实桌面复验通过：

- 空 Tasks 为 `920×600 layer=0`，AX 树没有文本框；在真正的 ABC
  (`com.apple.keylayout.ABC`) 下直接键入及 `⌘F` 后键入均无候选窗或隐藏焦点。
- 启动 `npm Run · s03` 后 `⌘F` 聚焦可见筛选框；ABC 输入 `s03`，拼音
  preedit 出现后按 Esc 恢复并保留已提交的 `s03`，Tasks 不退出、不关闭。
- 删除最后一条任务后，已聚焦筛选框随页面移除并清除焦点；随后在 ABC 下
  直接输入不产生候选窗、文本框或隐藏 IME 状态。
- Launcher 为 `layer=3` 且失焦隐藏；Clipboard Shelf 为
  `480×520 layer=3`，失焦保持、原生关闭按钮生效；Settings 与 Tasks
  均保持 `layer=0`，失焦不隐藏。

最终候选为
`build/acceptance-gpui-ui/v143-20261115-091707/Anywhere-GPUI-UI-Alignment-v143.app`
（`0.14.0+143`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv143`），严格签名、
plist、arm64 和 CDHash `9c6f43e1...` 已核对。证据见该目录的
`VALIDATION.md`、`logs/` 和 `screenshots/`。VoiceOver 继续排除；
Reduce Transparency、浅色主题、登录项、多 Space、睡眠唤醒和完整 IME 矩阵
继续保持 Partial / Unverified。

### 2026-11-15 Flutter 焦点与 Tab 对齐（Build 144）

继续以 Flutter `ExecutionSessionsView`、`ToolPageShell` 和 Clipboard 页面为
oracle 审计，发现并修复以下行为差异：

- Tasks 有筛选时 Esc 原先清空筛选后仍聚焦搜索框；Flutter 会清空筛选并把
  焦点交给任务列表。筛选框 Enter、点击任务行也会离开搜索焦点。
- Tasks 与 Clipboard Shelf 的 `⌘F` 原先只聚焦；Flutter 同时全选已有查询。
- Clipboard History / Shelf 点击条目后应恢复搜索焦点，GPUI 原先不保证。
- Clipboard History / Shelf / Application History / Plugin Detail / 结果详情
  使用 ToolPageShell 的 Tab 语义：无 Control / Option / Command 时消费
  Tab 与 Shift+Tab，不进行宿主焦点遍历。

新增回归：

- `tasks_escape_clears_the_filter_and_returns_focus_to_the_list`
- `tasks_enter_from_the_filter_moves_focus_to_the_list`
- `clicking_a_task_row_moves_focus_out_of_the_filter`
- `clipboard_shelf_search_shortcut_selects_the_existing_query`
- `selecting_a_clipboard_row_restores_search_focus`
- `tool_pages_consume_unmodified_tab_navigation`

Build 144 自动化门禁为 GPUI 152 passed、workspace 834 passed / 6 ignored /
0 failed；`scripts/check.sh`、Clippy、格式与文档检查通过。候选为
`build/acceptance-gpui-ui/v144-20261115-120415/Anywhere-GPUI-UI-Alignment-v144.app`
（`0.14.0+144`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv144`），严格签名、
arm64 和 CDHash `42a59bef...` 已核对。

构建后验收机进入登录锁屏（`CGSSessionScreenIsLocked=1`），因此本轮新增
焦点 / Tab 序列的物理状态为 **Pending**；Build 143 的窗口层级、空 Tasks
隐藏焦点与拼音 Esc 物理结果保留在对应目录，不转写为 v144 证据。

### 2026-11-15 窗口激活与 Tasks 操作对齐（Build 146）

源码审计确认两处差异：

- AppKit `activateWithOptions` 可能返回 `true`，应用实际仍未 active。
  原逻辑因此跳过 LaunchServices 兜底，状态栏打开 Running Tasks 后可能仍
  留在原前台应用后面。现在以 `application.isActive()` 的实际结果决定兜底；
  新建 Running Tasks 也会在配置后显式激活，而不只在复用已有窗口时激活。
- Flutter `ExecutionSessionsView` 的 `⌘K` 在存在当前任务详情时打开任务
  操作菜单；GPUI 之前错误地始终切换 Tool options。现在有详情时显示
  Copy command / Remove result（后者仅任务结束后出现），无详情时才打开
  Tool options；详情标题栏改为单个更多操作按钮。
- Tool options 的第二项恢复为 Flutter 的 Launcher；页脚保留 Keyboard help。
  任务命令改用与 Flutter 相同的 POSIX 参数引号规则，含空格或单引号的
  argv 不再显示 / 复制成不可执行的一行。

Build 146 自动化门禁为 GPUI 157 passed、workspace 839 passed / 6 ignored /
0 failed；`scripts/check.sh`、Clippy、格式与文档检查通过。候选为
`build/acceptance-gpui-ui/v146-20261115-141234/Anywhere-GPUI-UI-Alignment-v146.app`
（`0.14.0+146`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv146`），严格签名、
arm64 和 CDHash `3c1e60e7...` 已核对。

构建与启动发生在验收机锁屏期间（`CGSSessionScreenIsLocked=1`），因此窗口
激活和 `⌘K` 操作菜单的物理状态为 **Pending**。v144 的焦点 / Tab 序列也
仍未完成物理复验；所有 Pending 项均保留在 v146 验收记录中。

### 2026-11-15 Tasks 原生键路由与 Shelf 回焦（Build 147–159）

v147 先补齐了 Tool options 的 GPUI 状态机和键盘回归，但物理复验发现裸 Esc
仍不能关闭弹层。定位结果是：Tasks 没有编辑器输入 handler 时，AppKit 会在
GPUI interceptor 之前消费裸 Esc / 方向键；`⌘K` 因为走 modified key equivalent
仍能进入 GPUI，因此自动化直接调用 `handle_key` 无法覆盖该缺陷。

v152 增加原生 Tasks 键路由：本应用 key window 为 Running Tasks 时，KeyDown
monitor 将无 Shift / Command / Control / Option 的 Esc、↑、↓、Enter 转成
`PlatformEvent::TasksKey`，再由任务状态机处理；Shift+方向键 / Enter 保留文本
编辑语义，IME 组合中的按键继续透传。
该中间候选的物理序列确认了 Tool options Esc / ↓ / Enter 返回 Launcher、
再次 `⌘K` 关闭，以及有详情任务菜单的 Esc、Copy command、Remove result。
同一序列也确认 Tasks 筛选 `⌘F` 全选、Esc 清空并失焦、Enter 失焦、行点击
失焦后再用 ↓ 切换任务。

物理复验同时发现 Clipboard Shelf 行点击后搜索框没有真正回焦。原同步
`input.focus` 会被随后的原生 mouse-up 覆盖；v153 改为在当前输入周期结束后
恢复焦点，并校验页面和选中行未变化。

v153 清理了 v152 的诊断性键日志和临时页面键盘 handler，只保留最小修复。
v154 又补了 Shift 透传，避免 Shift+方向键 / Enter 被 Tasks 导航截获；v155
把该判断提取为可测试函数，并新增
`tasks_window_navigation_passes_through_shift_modified_keys`，产物行为与 v154
一致。

v156 继续对照 Flutter `ClipboardShelfPage._key`：操作菜单打开时，`⌘K` 应关闭
菜单，`⌘F` / `⌘R` 等未消费命令应先退出菜单再继续执行；目标条目在刷新后消失
时，Refresh 仍必须执行。GPUI 现已采用同一顺序，并为打字退出菜单、命令透传和
过期 Refresh target 增加回归。

v156 解锁后的物理复验又发现：Tool options 选择 Launcher 时界面虽出现，但
`RestoreLauncher` 在 Tasks 窗口更新期间同步进入主窗口，日志出现
`RefCell already borrowed`。仅把 TasksShell 回调或 `restore_from_tool` 的
defer 往后挪仍会在 GPUI async context / window update 内移动窗口；v159 最终
拆分为“先完成实体更新，离开 update 后调用 `show_main_window`，再恢复编辑器
焦点”。Tool options、`⌘L` 两条恢复入口都新增了跨窗口回归。

自动化门禁为 GPUI 164 passed、workspace 846 passed / 6 ignored / 0 failed；
`scripts/check-gpui.sh`、`scripts/check.sh`、Clippy、格式、文档和
`git diff --check` 通过。最终候选为
`build/acceptance-gpui-ui/v159-20261115-185309/Anywhere-GPUI-UI-Alignment-v159.app`
（`0.14.0+159`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv159`），严格签名、
arm64、CDHash `fe348063...`、可执行文件 SHA-256 `c95648ce...` 已核对。

v159 的真实桌面复验通过：Tasks Tool options 的 `⌘K` / Esc / ↓ / Enter、
`⌘L` 恢复 Launcher 均无 `RefCell` 或 ERROR；有详情任务菜单的 Esc 与 Copy
command 通过；Tasks `⌘F` 全选、Esc 清查询并继续 Down、Enter 后继续 Down、
行点击后 Down、Shift+↓ / Shift+Enter 保留文本选择语义且未进入原生 Tasks
导航；Clipboard Actions 的 `⌘K` 开关、`⌘F` / `⌘R` 透传通过；Shelf 单击
条目后输入 `a`，AX 搜索框值为 `a` 并实际刷新结果。证据见该目录
`VALIDATION.md`、`logs/app-stdout.log` 和 `screenshots/`。

这组物理结果不等于完整 IME、Reduce Transparency、浅色主题、登录项、多 Space
或睡眠唤醒矩阵通过；这些项目继续保持 Pending / Partial。

### 2026-11-16 按住 Enter、历史快捷键与预览 repeat（Build 160–162）

v159 后的 Flutter oracle 抽样又确认两个输入差异：

- Enter / Return 的首次 key-down 正常执行，但按住产生的 auto-repeat 不应再次
  执行。GPUI 的 `on_key_down` 路径已把真实 `is_held` 传入统一 key 状态机，
  Tasks 原生 KeyDown monitor 也携带 `NSEvent.isARepeat`；重复 Enter / Return
  在复制 Shelf 条目、执行 Tool option 或窗口恢复之前被忽略。新增
  `held_return_copies_a_shelf_item_once` 和
  `repeated_tasks_enter_does_not_repeat_the_tool_option` 回归。当前 gpui-kit
  的 `KeystrokeEvent` 没有 repeat 字段，所有普通 interceptor 输入路径仍须在
  真实桌面复验，不能由这两个回归推断全部入口已物理通过。
- Application History 与 Launcher 的对象动作入口不同：Flutter oracle 不把
  `⌘K` 绑定到历史页面，当前对象 Actions 只由行内 `…` 或右键打开。GPUI 已删除
  页面的 `⌘K` 分支和按钮旁的快捷键文案，并新增
  `application_history_does_not_bind_command_k_to_actions` 回归；行内 `…`、
  右键、对象动作票据和 Refresh 按钮保持不变。Flutter oracle 也没有页面级
  `⌘R`，GPUI 同步删除该额外绑定并增加
  `application_history_does_not_bind_command_r_to_refresh` 回归。
- Clipboard History / Shelf 的 `⌘` 命令和匹配当前 Launcher 的本地 Action
  快捷键同样只处理首次 key-down。按住 `⌘K` 不会再次切换 Actions，按住 `⌘R`
  不会连续刷新，重复 Action 快捷键也不会重复执行；新增
  `held_command_k_does_not_toggle_shelf_actions_twice`、
  `repeated_clipboard_commands_do_not_toggle_or_refresh_again` 和
  `repeated_local_action_shortcuts_are_ignored_but_global_ones_are_not`
  回归。
- Build 161–163 继续对照 Flutter `NativePreviewController` 和键盘焦点契约。文件、Clipboard
  History 和 Shelf 的 `⌘Y` 现在统一开关预览，按住只处理首次 key-down；
  预览中的 `⌘L` 关闭并返回查询，Left / Right / Home / End / Backspace /
  Delete 或 `⌘A` 先退出浏览态再继续文本编辑。文本输入产生的 repeat Space
  会回滚该次空格，不再形成第二次 preview 请求。Space 仅在查询编辑器拥有
  焦点时切换预览；页面 Back 等按钮聚焦时由 Space / Enter 激活按钮。Tasks
  的 `⌘K` 也只处理首次 key-down，按住不会打开后立即关闭动作菜单。Launcher
  选中可浏览菜单且输入为空时，`⌘K` 会在没有对象 Actions 时进入菜单，和右箭头
  共用同一菜单浏览入口。
  Settings 的快捷键录制及 Esc 同样不接受 repeat，按住 Esc 只取消本次录制。新增
  `command_y_toggles_file_preview_once_and_ignores_repeat`、
  `command_y_closes_a_clipboard_preview_and_command_l_restores_search`、
  `editing_navigation_closes_the_current_preview`、
  `repeated_space_edit_does_not_toggle_a_preview` 和
  `repeated_settings_keys_do_not_finish_recording_or_close_the_page` 回归；
  另新增 `space_and_enter_activate_a_focused_page_button` 和
  `file_search_space_preview_requires_the_editor_to_own_focus`，并加强 Tasks
  两个 `⌘K` 入口的 held repeat 断言；新增
  `command_k_browses_a_menu_when_no_object_actions_exist`。

当前自动门禁为 GPUI 200 passed、workspace 882 passed / 6 ignored / 0 failed；
`scripts/check-gpui.sh`、`scripts/check.sh`、Clippy、格式、文档和
`git diff --check` 通过。

v160 的 00–19 物理序列在预览 repeat 修复前完成：Shelf held Enter 只让
`changeCount 210 → 211`，held `⌘K` 保持 Actions 打开，held `⌘R` 只增加一次
snapshot start；Application History 的 `⌘K` / `⌘R` 无动作，`…` Actions 可用，
held Enter Copy Path 只让 `changeCount 211 → 212`；Tasks Tool options 选中
Launcher 后 held Enter 只恢复一次且无 `RefCell already borrowed`。这些结果
不能替代 v164 对预览快捷方式、按钮焦点、菜单浏览和输入事件 repeat 的复验。证据见 v160 目录的
`VALIDATION.md`、`logs/app-stdout-physical-unlocked.log` 和
`screenshots/00...19`。

Build 165 又根据 v164 截图修复了两项 Launcher 视觉缺陷：

- `contentScrim` 从 40% 纯黑改为与 Flutter `surface/window` 一致的约 63%
  中性灰（`#a0303438`）。空态与输入态使用同一层遮罩，不再因窗口高度和桌面
  内容占比不同而出现明显玻璃明暗差。
- 候选行变化引起的高度更新不再分两步执行。macOS 侧在一次
  `NSWindow.setFrame:display:` 中同时设置尺寸和顶部锚点，并合并快速连续的
  resize 请求；尺寸未变化的候选刷新完全不调用原生 resize。

v165 在解锁的真实桌面完成针对性复验：空态与 `ChatGPT` 查询态同一页头区域的
背景采样差约 2–4 个 RGB 级；60 fps 录屏的 30 fps 抽帧显示 72、136.5 和
472.5 px 高度均为单帧原子切换，顶部始终为 y=1128，无中间错位帧。真实 Esc
首次清查询、第二次隐藏，应用失焦隐藏也通过。Light 外观、Reduce Transparency
和 Increase Contrast 未在本次视觉序列中重跑。

最终候选为
`build/acceptance-gpui-ui/v165-20261116-100723/Anywhere-GPUI-UI-Alignment-v165.app`
（`0.14.0+165`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv165`，PID
`16941`），arm64、CDHash `c8b6067d...`、可执行文件 SHA-256 `4fa8b6d3...`；
严格签名、隔离数据目录和两个隔离热键已核对；Rust 日志零 ERROR / WARN，
真实拼音输入触发过一条 macOS `IMKCFRunLoopWakeUpReliable` stderr 提示，但输入
与候选提交正常。详细证据见该目录的 `VALIDATION.md`。

### 2026-11-16 UI 审查 15 项修复与 Build 167

按 [GPUI UI 审查](archive/reviews/gpui-ui-review-2026-11-16.md) 修复并在
v166 / v167 隔离包上复核：

- UI01 / UI10：Settings 关闭确认和剪贴板删除确认使用遮罩命中隔离、模态焦点
  循环与关闭后焦点恢复；普通字符、组合键、指针和 IME 不能到达底页。Clipboard
  Clear 及缩短期限先显示不可恢复范围，取消不调用运行时，loading 或 stale
  confirmation 的重复提交被拒绝。
- TEST01 / UI03 / DEV01：布局测试统一使用真实 viewport；Launcher 的 loading、
  empty、1 / 5 行、提示出现 / 消失和无 Core preview 过滤都经同一后渲染尺寸
  收敛入口；新增结果数量与 preview 回归。
- UI02 / UI04：Notice 高度限制为 44–96 px 并在内部滚动。Scope 用可收缩的左右
  gutter 居中短组，溢出时 gutter 归零、按钮作为滚动容器直接子项，选中项能滚入
  视口；新增短组和长组回归。
- UI05 / UI06：Shelf 页头从 28 px titlebar inset 后开始；480×520 和 360×360
  真机截图中状态图标、搜索框、可见 Esc 均不进入交通灯区域。可见 Esc 与键盘
  Esc 共用返回链，真机验证查询、Actions 和删除确认均只关闭当前局部层。
- UI07 / UI08：Unsupported Tool 输入进入有界纵向滚动并允许长路径换行；放弃
  语言草稿会在中 / 英双向恢复已保存 locale、模型和 placeholder。v167 真机执行
  中→英→放弃，Launcher 与 Settings 均恢复中文。
- UI09 / UI11：Clipboard 设置从冷启动读取真实状态，展示容量上限 / 1 / 7 / 30 /
  90 天和恢复 / 5 / 15 / 60 分钟；真机完成暂停 5 分钟、显示“已暂停”和立即
  恢复。状态回包断线会退出 loading 并报错。Tasks 键盘帮助显示 Info，
  Launcher / Shelf / Tasks 的 Notice 分别保留 Info、Warning、Error、Success
  语义。
- UI12 / UI13：macOS Reduce Motion 在启动和辅助功能通知时同步给 GPUI；原生
  resize 与初始辅助窗口按当前屏幕可用区域约束宽高，Settings 在小 viewport 下
  使用实际 native 高度并保留内容滚动。自动化与 v167 日志确认状态透传和约束
  函数；系统 Reduce Motion 切换及 800×600 真实显示器矩阵仍未执行。

v166 真机复验同时发现，上一版约 63% 的 `contentScrim` 在 72 / 473 pt 两种窗口
高度下仍出现最大约 21 个 RGB 级差；原因是 `NSGlassEffectView` 会随采样面积改变
整体亮度。v167 将中性灰 veil 提高到约 93%（`#ed303438`），保留 7% 桌面透射和
原生高光。同一页头区域的空态 / 查询态采样从 `RGB 93/97/100` 对
`75/79/79` 改善为 `57/60/65` 对 `53/57/59`，差约 4 / 3 / 6 个通道级。

最终候选为
`build/acceptance-gpui-ui/v167-20261116-153939/Anywhere-GPUI-UI-Alignment-v167.app`
（`0.14.0+167`，Bundle ID `dev.anywhere.launcher.gpui.uiplanv167`，PID
`58275`，父 PID `1`），arm64、CDHash `76f55f34...`、可执行文件 SHA-256
`1d8eedb2...`；严格签名、隔离数据、热键注册和零 Rust ERROR / WARN 已核对。
Launcher 为 layer 3 且失焦隐藏；Shelf 为 layer 3 且失焦保持；Settings 与
Running Tasks 为 layer 0，失焦后保持普通窗口行为。VoiceOver 按约定不验证。

## 已完成的物理验收

### 2026-11-12 最终候选 v16 复核

重新构建 `build/Anywhere.app`（`0.14.0+1`）后，制作唯一 Bundle ID
`dev.anywhere.launcher.gpui.20261112v16` 的隔离验收包，并通过 `open -n` 指定
独立 `ANYWHERE_DATA_DIR=/tmp/anywhere-gpui-physical-20261112-v16` 启动：

- LaunchServices 报告进程类型 `UIElement`；System Events 报告应用
  `visible=false`、窗口数 0，启动后前台应用保持不变。CoreGraphics 的可见
  Launcher 窗口数为 0。
- AX 菜单树可读取 Current App、Universal Search、Running Tasks、Settings 和
  Quit；隔离数据目录的配置让菜单显示实际快捷键 `⌃⌥⇧O` / `⌃⌥⇧P`，
  Settings 与 Quit 分别显示 `⌘,` / `⌘Q`。
- 系统级 `⌃⌥⇧O` 事件在约 54 ms 内完成 Current App 呼出；日志记录
  `app_active=true`、`key=true`、first responder 为 `GPUIView`，当前 editor
  为 `focused=true`。热键通过事件回调进入异步队列，不再每 24 ms 轮询全局
  接收器。
- 真实键盘输入 `e` 后窗口为 `648×473`，清空后为 `648×72`，两次窗口顶部均为
  y=312。原 Flutter 的 `maxRows` 是 7，因此 473 是合法的 7 行物理边界；
  第 8 行不是合法模型状态，520 仅保留合成布局 clamp 回归。
- 物理点击状态栏图标并选择 Settings 后窗口为 `648×508`，应用为 active、
  窗口为 key。真实 Tab 依次聚焦七个 Button 分类、Done 和两个 98×26 快捷键
  录制器；Shift+Tab 反向移动。Space 开始录制，`⌃⌥⇧K` 被写回草稿，AX 名称
  同步为“录制快捷键: ⌃⌥⇧K”。
- 录制后真实 Esc 打开脏草稿提示；“继续编辑”“放弃修改并关闭”“保存修改”
  三个 Button 都可见且可操作。点击“放弃修改并关闭”返回 Launcher，隔离配置
  仍保持 `KeyO` / `KeyP`，证明放弃没有写入磁盘。
- v16 的 Settings 截图中，七个分类恢复各自的原版图标色系，卡片、侧栏、字体、
  divider、控件和 translucent native material 采用同一主题。打开、Tab 导航、
  录制、脏草稿提示与关闭全序列没有 `RefCell already borrowed`、
  configuration-failed、key-activation-failed 或 reactor panic。
- 该包仍使用系统级合成键盘事件验证全局热键，不能替代人手逐键矩阵；
  Reduce Transparency、浅色环境、Quick Look、WebView helper
  回收和 Settings 保存失败继续为 Partial / Unverified。

### 2026-11-12 材质、圆角和文字布局修复

2026-11-12 当时 GPUI 根画布完全透明（window token alpha 为 0），文字、图标和
局部控件绘制在单独的原生材质 host 之上。Build 95 已在白底/彩色桌面的物理
对比度失败后加入单层 `contentScrim`；本节以下 HUD 日志仅保留为旧构建记录。
隔离运行时在 macOS 27.2.1 上记录：

```text
native_window_material_installed mode=Hud glass_available=true
reduce_transparency=false corner_radius=18.0
wrapper_class=NSView host_class=NSVisualEffectView
wrapper_masks_to_bounds=true wrapper_corner_radius=18.0
window_opaque=false
```

上述 HUD 日志属于旧构建。2026-11-13 已修正材质选择与内容承载：macOS 26+
实际选择 Glass，并以 `NSGlassEffectView.contentView` 承载原 GPUI 内容；旧系统
使用 HUD。新隔离构建 +77 日志确认 `mode=Glass`、`content_layer_opaque=false`、
`window_opaque=false`，60 / 280 ms 探针均为 key / active，first responder 为 GPUIView。
窗口截图仍呈深色，桌面透色矩阵尚未通过，不能据日志宣布 Liquid Glass 已验收。
当前实现规则见 [材质契约](ui-design.md#材质与可读性)。

同时修正 Settings 初始焦点：焦点由页面根节点持有，鼠标切换分类时不再留下
General 的白色键盘焦点环；增加回归验证且保留 Tab 导航。Context 状态只显示
单条状态文字，完整详情提供 tooltip 与辅助功能描述，避免两个省略文本互相挤压。

Tasks 页头额外增加 28 px titlebar inset；Settings 侧栏、toolbar、剪贴板 /
应用历史 / Web 列表以及固定高度行均补充了 `min-width: 0`、单行截断和明确
行高。真实结果列表已从空态 `648×72` 扩展到七行 `648×473`，顶部保持 `y=312`；
单结果 `648×137`、中文 IME 和 Settings 长文案换行均已检查。

旧 sibling 方案已被 contentView 承载替代。减少透明度、Increase Contrast 和
浅色 / 深色主题的最终物理验证继续保持 Partial / Unverified。

### 当前无边框 Accessory 包

2026-11-11 在 macOS 27.2.1（build 26B131）/ arm64 上使用独立验收包
`Anywhere-GPUI-Acceptance-Esc-20261111-2.app` 和独立数据目录完成以下序列：

- 启动进程类型为 `UIElement` / Accessory；正常启动时主窗口隐藏，前台应用不变，
  状态栏搜索图标存在，内部菜单标签为
  `当前应用  ⌃⌥⇧U`、`全局搜索  ⌃⌥⇧I`、`设置…`、`退出 Anywhere`。
- 通过真实系统按键交替触发 Current App 与 Universal Search。采样批记录
  31 次窗口显示，31 次均为 `visible=true key=true`，20 次隐藏；没有
  `launcher_window_key_activation_failed`，也没有 reactor panic 或 AppKit
  重入错误。一次 60 ms 探测短暂读到 `key=false`，280 ms 重试后记录
  `key_activation_recovered`，窗口恢复为 `key=true app_active=true`。
- 物理点击状态栏菜单的两个搜索项和 Settings 均进入正确入口；每次显示后
  first responder 为 `GPUIView`，当前 editor 日志为 `focused=true`。
- 设置页按一次真实 Esc 从 648×508 返回 Launcher。实现通过 AppKit 本地事件
  监听在文本框输入上下文之前路由无修饰 Esc；有 marked text 时事件交回输入法。
  草稿关闭、取消确认和保存 / 放弃分支由自动化回归覆盖，脏草稿的物理完整矩阵
  仍待补。
- 真实中文输入法在 Launcher 输入 `nihao` 后显示候选窗；Space 提交为“你好”，
  最终编辑器可见 `safari你好`。这说明 AppKit / GPUI 原生输入法路径可达当前
  editor。
- 实测窗口高度 `72 → 137 → 473 → 72` 时，原生 origin 的 y 均为 312；顶部
  不随窗口增长或收缩移动。520 点边界目前只有纯几何回归。
- Finder 成为前台后，主 Launcher 在约 220 ms 宽限后隐藏。独立 Clipboard
  Shelf 在 Finder 前台时仍保持 `480×520` 可见，点击其窗口后 Esc 可关闭。
- Current App 的浏览器页面读取触发 macOS Automation 权限框；选择“不允许”
  后，拒绝没有终止 Launcher，浏览器依赖 Context 保持不可用。

这些结果确认了当前 Accessory 窗口、真实热键、真实 IME、顶部锚点和外部失焦
工作。它们不覆盖 Reduce Transparency、macOS 15 材质或 WebView
内部焦点。

### 较早共享交互覆盖

2026-11-11 在 macOS 27.2.1（build 26B131）/ arm64 上使用独立数据目录和
`dev.anywhere.launcher.acceptance` 验收包实际完成以下序列：

- Launcher 能查询并打开 Clipboard Shelf；设置页的通用、快捷键、搜索、
  剪贴板、功能分类和关闭路径可见且可操作。
- 使用独立数据目录和 `⌃⌥⇧J` / `⌃⌥⇧K` 启动新的 GPUI 实例后，进程为
  `UIElement`，屏幕窗口数为 0，前台应用保持为启动前应用；两个热键均完成注册，
  启动日志没有 AppKit move / resize 重入错误，菜单栏搜索图标可见。
- 文件搜索查询没有隐式首项；首次 `Down` / `Up`、后续连续选择、
  `Enter`、`⌘K`、`⌘Y`、`Space`、Esc 分层返回和查询空格均实际执行。
- Clipboard History 与独立 Shelf 的查询空格、首次选择、连续切换和关闭后
  保留状态均实际执行；Shelf 首帧直接显示 5 条记录。
- Shelf 失焦到 Finder 后仍接收其可见区域的点击，确认
  `NSFloatingWindowLevel` 生效；Esc 先关闭 Quick Look、再关闭 Shelf，
  关闭后重新执行命令可再次显示数据。
- 文本 Quick Look 通过 `Space` 打开，使用 `Space` 或 Esc 关闭；关闭后
  查询、选择以及返回输入框的状态保持。验收日志中
  `quick_look_show_finished success=true`、`preview_open=true`，且未出现
  `RefCell already borrowed`。
- 彩色桌面截图中，648×72 的 floating Launcher 显示原生半透明背景与共享 surface
  层，未见交通灯或额外 Dock 项。该观察不等于 Reduce Transparency、macOS 15
  材质和 WebView 合成已经完成验收。

## 物理桌验矩阵

以下项目必须使用有交互桌面的 macOS 15+ 本机并记录设备、系统、构建号和操作
结果。Rust 单测、编译、签名或截图本身不能将它们标记为通过。

| 项目 | 必须复现的序列 | 当前状态 |
| --- | --- | --- |
| 中文 IME | 组合输入、候选窗、字符输入、Enter、Esc、方向键；确认 GPUI 不抢占 | Partial；当前 live Core 包实际显示候选窗，Space 提交“你好”并写入 Launcher editor；Enter、方向键和设置页完整组字矩阵仍待验 |
| 菜单栏恢复 | 启动后核对无窗口、无 Dock；隐藏 Launcher，点击菜单栏搜索图标，分别选择 Current App / Universal Search、Running Tasks、Settings 和 Quit | Partial；当前包物理点击搜索、Tasks、Settings 与菜单快捷键均进入正确入口，Tasks 重复点击复用同一 920×600 窗口；Dock 泄漏与所有休眠 / 唤醒组合仍待重复 |
| 全局热键 | 两种默认模式唤醒；冲突、注销和重新注册后的行为 | Partial；当前 Accessory 包交替执行 Current App / Universal Search，事件回调路径实测 46–54 ms 到可见窗口，采样批 31 次显示均为 key window、20 次隐藏、无失败告警或 reactor panic；冲突、注销和重注册仍待验 |
| Launcher 焦点 | 呼出后无需点击即可输入；置顶；失焦隐藏；根页 Esc 按 Content → Current App → Global → 隐藏返回；内部预览 / Web 焦点不误隐藏 | Partial；Build 123 实测 WebView 聚焦 Esc 返回且保留查询，Tasks / Shelf 为 key window 时热键呼出立即 `key=true`，Launcher 失焦隐藏；Current App 空白根页两次 Esc 链保持不变；Quick Look、多 Space 与预览豁免仍待验 |
| Settings 关闭 | 无草稿 Esc、脏草稿取消 / 放弃 / 保存、系统关闭和失败；普通窗口层级 | Partial；Build 123 实测 `layer=0`、无草稿 Esc 返回 Launcher；Build 121 实测脏草稿保留确认框，Settings 焦点节点已有 AX ID / role，Tab 可达 General；保存失败、系统关闭与输入法组字期间 Esc 仍待验 |
| Clipboard Shelf | 独立置顶、右下角定位、最小化恢复、迟到复制返回、Esc、系统关闭和失焦 | Partial；Build 123 实测 `layer=3`，切到 TextEdit 后保持，点击交通灯关闭才消失；Build 113 实测最小化复用不 panic、复制并返回 TextEdit 且无 destination warning；多 Space 和长时间关闭 / 恢复仍待验 |
| Quick Look | 空格 / `⌘Y` 开关、连续切换、`⌘L` 返回、唤醒、关闭后焦点回到正确输入框 | Partial；v164 已补文件与 Clipboard 的 `⌘Y`、`⌘L`、编辑导航、按钮焦点和 repeat 回归；v160 已验证文本文件和剪贴板文本的 Space / Esc、选择保留；v164 物理复验、媒体、多文件、唤醒与完整 IME 焦点仍待验 |
| WebView | 包加载、固定 origin、IPC、ack、焦点、宿主 Reload / Back、Esc、替换与 teardown、helper 回收 | Partial；Build 123 实测 Text Pad 加载、真实 WebView 键入、AX 值更新，聚焦 Esc 经 host IPC 返回并保留查询；Build 121 实测 Actions Reload 与 Back 返回；旧 IPC 替换、长时间循环与 helper 回收仍待验 |
| Accessibility | 权限缺失时菜单隐藏，其他动作可用；授权后重新扫描 | Partial；Settings 焦点缺少 AX 节点的警告已修复，分类与录制器可在 AX 树中经 Tab 导航；撤销 / 重新授权仍未验证 |
| 登录项 | 注册、系统批准、关闭、重新打开与独立数据目录不访问系统项 | Unverified |
| 材质与布局 | Reduce Transparency、浅/深主题、缩放；无交通灯；72 / 3、5、7 行与顶部锚点；失焦隐藏 | Partial；当前包实测 `72 → 137 → 473 → 72` 顶部坐标固定为 y=312，473 是合法 7 行上限，Finder 失焦隐藏；520 只有布局 clamp 回归。Reduce Transparency、Increase Contrast、浅色环境、缩放和完整逐页截图仍待验 |

物理验收发现缺陷时，先建立最小可复现序列。IME 或焦点问题不得用模拟按键直接
改写生产语义；修复后再重复原序列并保留未覆盖边界。

## 已知边界

- Windows / Linux 继续只验证共享 Rust crates，当前没有 GPUI 桌面产物。
- Developer ID 签名、公证、干净机器升级和自动更新尚未完成。
- 第三方插件的 UI 在 WebView 中运行；需要原生 WebView 的完整物理矩阵。
- Web 后台 QuickJS 仍为进程内执行；插件进程隔离尚未交付。
- 浏览器历史、书签、Tab 管理、插件市场与终端的 PTY 支持不属于本轮范围。
- 当前没有整 App GPUI 性能承诺；历史采样不能作为当前版本结论。

## 退出条件

本轮可标记为开发预览完成需要同时满足：

1. 快速 GPUI 门禁和完整工作区测试通过。
2. 完整 `scripts/check.sh` 通过，文档、本地化、设计参数和架构检查无漂移。
3. 发布 App 构建、签名、plist 和动态库检查通过。
4. 工作树中没有其他 UI 实现、桥接 crate、构建入口或现行文档依赖。
5. 物理矩阵逐项记录；未完成项保持 Unverified，不用自动化结果替代。
