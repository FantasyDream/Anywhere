# Launcher 交互契约

现行导航与交互契约；文档整理：2026-11-23。使用教程见 [README](../README.md)，本页是导航、快捷键与状态恢复的统一依据。

## 入口与 Scope

- Current App 默认 `⌃⌥Space`；Global Search 默认 `⌥Space`。菜单栏搜索图标菜单也可切换两种入口，并仅显示本进程实际注册成功的快捷键；启动注册前与注册失败时不显示未生效的键位。
- Settings 的快捷键录制器允许 Option 单独作为修饰键，例如 `⌥Space`、`⌥K`；内部持久化仍使用 `alt`，状态菜单和设置界面显示 macOS 的 `⌥` 标签。
- 空白 Global 不常驻 File Search 等工具卡片；输入名称查找普通 Action，或设置 Action 快捷键。
- Current App 聚合当前环境所有可用 Action，包括应用菜单、Folder、Git、Developer 等；其他 Scope 硬过滤原领域 Group，Action 不改写 group_id。
- 当前应用组为 `app.<bundle_id>`，显示实际应用名。无可见动作不生成空 Scope；应用启动候选只属于 Global Search。
- Global Search 已有非空查询时切回 Current App，已有应用 Scope 必须立即选中；
  Environment 首次快照尚未提供 Scope 时保留一次显式导航补选，到达后立即用同一
  查询重搜。新结果返回前不得显示或执行 Global 的旧应用行；Environment Context
  已完成但仍无应用 Scope 时结束等待并显示空态，不伪造过滤范围。
- Scope 只改变搜索范围。输入、文件类型和内容来源不成为额外 Scope。只有单一 Current App 时，应用名显示在输入区，省略重复分类栏；范围和键盘逻辑不变。
- Current App 首位；Session 中既有槽位固定，迟到领域追加，消失领域空出槽位。切 Scope 与浏览不计 Usage；菜单不读取 / 写入长期 Usage。

全局热键注册失败不阻塞其他功能或状态菜单入口。设置页持续显示注册错误与恢复
方式，页面切换、设置快照刷新和普通操作完成不能清除此诊断；没有修改时提供
“重试快捷键”。保存或重试成功后清除诊断。更换键位失败时恢复原绑定，菜单显示
回滚后实际可用的键位；用户期望配置仍保留，供修改或重试。管理器在启动时创建
失败也允许后续重试，事件分发读取当前控制器，恢复后无需重启。

## 推荐、搜索、浏览与对象操作

空查询的动作排序统一为：本次来源应用内的执行次数降序 → 最近执行时间降序 → 上下文相关性和动作默认优先级 → 稳定业务 ID。同一动作在首页和 Git 等领域 Scope 中保持相对顺序。计数直接比较原始整数，不套用关键词评分的 20 次上限；没有历史时自然使用上下文 / 默认优先级兜底，不另设首页推荐白名单、强制置顶或首页加分。使用记录按来源应用、动作 ID、Context 类型区分，不按 Scope 或具体项目路径重复记录。菜单仍按现有安全契约隔离长期 Usage，不恢复不可靠的旧记录。菜单的冷启动默认值优先常见编辑操作（如复制、粘贴、剪切、全选、查找），设置项不会因为英文单词的偶然子串被提升。

首页仅截取上述排序的前几项：可浏览菜单时最多五项，为 Browse Menus… 留出首屏空间；其他首页最多七项。全部动作仍可通过查询、对应 Scope 或菜单浏览使用。GPUI 只保留 Rust 顺序并限制条数，不决定业务排名。输入关键词后继续按关键词匹配优先的既有评分，不让执行频次压过实际匹配。菜单浏览仍使用原菜单结构；清空查询返回原菜单位置。

支持系统预览的文件 / 剪贴板列表默认不选中对象；首次方向键选择第一项，查询或类型变化清除选择。普通动作与应用历史仍沿用默认选中首项。

Clipboard History / Shelf 的首次加载、检索和刷新共用查询代次，旧列表不能覆盖
新查询。查询结果与操作反馈还绑定发起时的页面和会话代次，离开或隐藏后不能修改
其他页面的提示或 loading；过期操作也不能结束新操作的 loading。写操作完成时，
如果查询已改变，则重新读取当前查询。Settings 的剪贴板读写只刷新配置、状态和
用量。隐藏恢复后重发当前剪贴板查询。Shelf 的 `⌘↑` / `⌘↓` 跳到首尾并滚入视口；鼠标、方向键和
快捷键改变所选条目时，共用预览更新与复制反馈清理。

文件、收藏目录等对象的 `⌘K` 二级 Actions 绑定当前对象。返回恢复查询、选择和列表位置；旧对象操作响应不能覆盖新目标。输出页面与菜单、对象动作、文件工具共用主窗口风格，不增加独立的通用功能导航栏。

对象操作请求同时绑定面板代次、父对象、页面、会话与 revision。回包仍须经过
共享快照 cursor 门禁，同 revision 的旧快照也不能回滚；旧错误不能关闭后开的
面板。Core 在发布前复验对象请求代次，较早请求不能先发布并使较新请求过期。

搜索页不显示通用页脚或普通结果条数。选中行右侧显示执行按钮与回车提示，对象支持更多操作时在同一行直接显示 `… ⌘K` 入口，替代主操作的重复回车图标；无更多操作时保留回车提示。主搜索列表支持 `⌘1`–`⌘9` 快速选中并执行第 1–9 项候选。原应用菜单的快捷键不展示，用户自定义的 Action 绑定保留。

文件搜索使用同一输入 / 列表，不保留固定预览区；文件数量、扫描进度和重试显示在类型筛选下方。它的类型标签与 Scope 样式及切换键一致，但语义是文件过滤。
滚动加载与末行 ↓ 共用同一个分页请求。加载期间按 ↓，仍选中原行时移到下一个结果。
完整规则见 [文件搜索](file-search.md)。

## 键盘

| 按键 | 行为 |
|---|---|
| `↑` / `↓` | 无修饰键时移动候选；文件列表末尾继续加载。带 Shift / Command / Control / Alt 时交给输入框编辑，并退出空格预览浏览状态 |
| `Enter` | 执行或进入当前项；按住产生的自动重复不再次执行 |
| `⌘1`–`⌘9` | Launcher 主列表快速选中并执行第 1–9 项可见候选；Clipboard Shelf 复制第 1–9 项 |
| `⌘K`（其他平台页面命令用 Ctrl） | 统一对象/页面 Actions 入口：Launcher（含菜单浏览）、File Search、Clipboard History / Shelf、Running Tasks 与 Application History（有选中项打开对象/回退操作，空态打开页头菜单） |
| `⌘R` | 刷新当前工具页（Clipboard History / Shelf、Running Tasks、Application History） |
| `⌘F` | 聚焦并全选当前工具页搜索框（Clipboard History / Shelf、Running Tasks、Application History） |
| `Tab` / `Shift+Tab` | 向后 / 向前循环可见 Scope；文件页循环类型（兼容 Ctrl+Tab / Ctrl+Shift+Tab） |
| `→` / `←` | 空输入进入 / 返回菜单，有输入保持光标移动 |
| `Esc` | 当前层级返回；根页面最后关闭 |

内容页的编辑器聚焦时，无修饰键的 `↑` / `↓` 和 `Enter` / `Return`
在字符输入与换行处理前交给候选导航和执行；`Shift+Enter` 保留换行，
其他修饰组合保留编辑器语义。

异步更新把选中候选移到另一行时，Launcher 将选中项滚入可视区。Web 工具导入确认页
只响应当前页面的控件，`Esc` 或页头返回先取消审查。设置重新置前时恢复打开的弹窗焦点，
设置页的 `⌘S` 优先处理保存，已有本地 Action 绑定仍可在其他页面使用。

Tab 仅在搜索框聚焦且分类可用时切换，保留查询与输入焦点；中文组词、导航选择器、对象 Actions、绑定选择或输出层不切换背后的 Scope。Tab 不遍历控件，Web 工具内部的 Tab 仍由页面处理。主窗口的 `⌘,` 打开 Settings，
关闭模式选择器并切换为设置窗口样式；快捷键录制与输入法组词优先处理，Shelf /
Tasks 独立窗口不响应此入口。用户配置的 Action 快捷键独立于原应用菜单快捷键；一个适用对象直接执行，多个对象先选择。快捷键目录和失效规则见 [架构](architecture.md)。

主窗口与工具页的键盘分发按优先级分层：

1. 输入法组合优先（`prefer_character_input` 为真时直接交由输入法，不截获普通字符、Esc、方向键与 Space；内容页无修饰键的候选导航与 Enter / Return 例外）；
2. 预处理拦截层（`intercept_window_keystroke`）：重复按键过滤（`filter_repeated_key`）→ 快捷键录制（`handle_shortcut_recording_key`）→ 弹层路由（`handle_overlay_keys`）→ 用户自定义快捷键（`handle_user_shortcut_key`）→ 预览覆盖层（`handle_preview_key_overlay`）；
3. GPUI `actions!` 与 `key_context` 派发（`apps/gpui/src/launcher/view/actions.rs`）：全局上下文 `Anywhere`（`⌘,` 打开设置、`⌘Q` 退出）及页面上下文 `Launcher` / `FileSearch` / `Clipboard` / `ClipboardShelf` / `Tasks` / `Settings`（如 `⌘Y`、`⌘L`、`⌘K`、`⌘R`、`⌘F`、`⌘S`、`⌘↵`、`⌘⇧⌫`、`⌘.`、`⌘⌥↑↓`）；非输入页（如 Tasks 无筛选框时）通过 `surface_focus_handle` 保持对应页面 `key_context` 留在派发路径中；
4. 导航与输入编辑层：Shelf `⌘1…9` / `⌘↑↓` 导航快捷键、Application History 局部快捷键，以及各页 `↑` / `↓` / `←` / `→` / `Enter` / `Tab` / `Esc` 导航；其余普通字符交由聚焦的编辑器处理。

Launcher 候选在首尾回绕；工具列表的 `↑` / `↓` 在首尾保持当前项。
Clipboard History / Shelf、Application History 与 Tasks 都使用后一规则，页边界
不跳到另一端，也不因在同一项上重复按键而刷新详情。Application History 刷新
按 `object_key` 保留选中项；只有查询变化或从 Action 重新打开页面才回到首项并
清空旧来源行。Tasks 筛选按空白分词，每个词都必须命中任务标题或目录；
新启动任务不在旧筛选结果中时先清空筛选，保证任务详情可达。

Tasks 无匹配结果时没有可执行的所选任务，停止 / 删除 / 详情请求不能退回未显示的
第一个任务。新任务的选中意图保留到对应 ID 实际进入快照，后续手动选择 / 筛选可撤销它；普通输出更新保留用户
手动滚动的侧栏位置，仅选择变化或显式打开任务时把所选行滚入视口。停止或删除
后的列表沿任务订阅更新，运行中的输出刷新保留当前操作提示。

Enter / Return 的首次 key-down 正常执行，按住产生的 auto-repeat 只忽略执行动作，
不能重复复制 Shelf 条目、重复打开 Tasks Tool options 或重复触发窗口恢复。
Clipboard History / Shelf 的 `⌘` 命令、以及与当前 Launcher 匹配的本地 Action
快捷键，同样只接受首次 key-down；不能因为按住按键重复刷新、切换 Actions 或
重复执行 Action。Settings 的快捷键录制、Esc 和预览 Space 也按首次 key-down
处理；录制中按住 Esc 只取消本次录制，不继续关闭 Settings。Tasks 的 `⌘K`
同样只处理首次 key-down，按住不会把刚打开的任务菜单再次切换到关闭态。

## Esc 与返回

- 导航选择器、对象 Actions、绑定选择、输出等局部层先退出该层。
- Launcher 根页按产品返回链处理：内容页先回 Current App；普通查询先清空；
  Current App 空白首页切到空白 Global Search；Global 空白首页最后隐藏。
- Application History 有查询时 Esc 先清空查询并保留页面；查询为空才返回。
  页面右上角的可见 Esc 控件与真实 Esc 走同一条返回链，不能从有查询状态直接
  退出页面。
- 菜单浏览仍先返回上一级菜单；输入法候选和系统预览继续优先处理 Esc。Web Tool
  聚焦且没有输入法组字时，Esc 通过当前 document 的受控 IPC 返回原 Launcher
  工作区；旧文档不能关闭替换后的页面。
- 所有 Web 工具页面（含内置 JSON Workspace 与已导入的第三方 Web 插件）保留页头外开按钮与 `⌘⇧O` 快捷方式以在独立窗口打开副本，并共享透明标题栏与 Liquid Glass 窗口外观；文件搜索、Clipboard History 与 Application History 不提供独立弹窗入口（独立的 `Clipboard Shelf` 与 `Running Tasks` 窗口保持不变）。打开 Web 独立窗口后，主搜索栏可继续用全局快捷键唤起，两个窗口各自保留会话、文档与焦点，失焦及主窗口隐藏均不会关闭它。
- 文件页：先退出对象 Actions；有查询先清空，再按一次返回父页。返回按钮直接回父页。
- Settings 使用带透明标题栏的独立窗口形态。标题栏关闭、Esc 和页面关闭入口都先处理保存与未保存草稿，不套用搜索页面的清空规则。
- 打开 Settings 先关闭原预览与 Web 文档并撤销在途请求，避免不可见工具继续持有
  原生资源和执行资格；关闭 Settings 后仍按 Launcher 恢复规则处理。
- 偏好保存、录制剪贴板和功能开关共用设置操作代次，同时只提交一个设置操作；
  保存期间禁用即时开关，回调也复验。设置回包独立于 Launcher 查询代次，
  无关查询错误不能结束保存；失败按实际快照恢复开关，保留偏好草稿。
- 保存成功排队退出 Settings 后，执行退出前仍复验关闭意图、保存状态和草稿；
  “继续编辑”可取消排队退出，新增草稿不会被迟到退出丢弃。
- 从状态栏在隐藏 Launcher 上打开 Settings 后，关闭设置必须同时恢复查询
  通道：15 秒内按原 workspace 恢复，否则重新创建 Session；不能只恢复窗口、
  输入焦点和清空后的查询框。
- 状态栏 `Running Tasks…` 新建或复用任务窗口时，菜单关闭后必须把该窗口
  置前并恢复键盘所有权；Accessory 应用首次激活未生效时使用 LaunchServices
  兜底，不能让 920×600 窗口留在前台应用后面。LaunchServices 异步激活完成后
  必须再次让目标任务成为 key；不能通过重复激活整个 App 来补偿。
- AppKit `activateWithOptions` 返回 `true` 不等于应用已经 active。直接请求后
  仍非 active 时必须进入 LaunchServices 兜底；返回布尔值本身不能阻止兜底。
  `ActivateAllWindows` 继续禁用，激活完成只恢复目标窗口的 key 所有权。
- 状态栏复用已有 Tasks 时只能由菜单关闭后的单次激活负责；窗口复用路径不能再
  提前激活一次。Action 自动打开 Tasks 才由窗口内部负责激活。
- Launcher 激活前先让自身成为 key，并且不得请求 `ActivateAllWindows`。直接
  激活已使应用 active 时不再同步调用 `NSApplication.activate()` 或
  LaunchServices；否则约半秒后的第二次 App 激活会恢复无关的最小化
  Shelf / Tasks。
- Action 启动进程任务后，先新建或复用 Running Tasks 并以返回的
  `execution_session_id` 选中该任务、加载其详情，再隐藏 Launcher。
  旧筛选隐藏目标时清空筛选，目标已在列表和稍后到达两种情况都走同一入口。
  任务启动带 warning 时留在 Launcher 显示提示，不自动抢走任务窗口焦点。
- Tasks 对当前选择最多保留一次详情读取。等待期间发生的列表更新合并到一次追读。
  详情已经包含更新的输出或任务状态时直接复用。切换任务或退出页面时使旧读取失效。
- 独立窗口构造时不得先聚焦 Launcher 主搜索框：Clipboard Shelf 只聚焦自己的
  搜索框，Running Tasks 在筛选框未渲染时不安装编辑器输入焦点。空 Tasks 的
  普通键入不能创建隐藏输入法组字；`⌘F` 仅在筛选框实际可见时聚焦，任务列表
  变为空时清除已移除筛选框的焦点。
- Tasks 的 `⌘F` 聚焦并全选现有筛选；有筛选时 Esc 清空后把焦点交给任务列表，
  筛选框 Enter 与任务行点击同样离开搜索焦点。Clipboard Shelf 的 `⌘F`
  全选查询，Clipboard History / Shelf 选中条目后在鼠标事件结束后恢复搜索
  焦点，避免 mouse-up 再次抢走输入响应者。
- Tasks 窗口的裸 Esc / ↑ / ↓ / Enter 在 AppKit 文本输入路径之前由原生
  KeyDown monitor 转发给任务状态机。不能只依赖 GPUI 的 `window.blur` 后
  dispatcher；裸键在无编辑器输入 handler 时可能被 macOS 文本输入上下文消费。
  原生事件缺少 window 归属时只回退到本应用的 key window，并继续跳过输入法
  组字中的按键；`NSEvent.isARepeat` 必须随事件传入，按住 Enter 不能重复选择
  Tool options。
- Tasks 有当前详情时，`⌘K` 打开任务操作菜单：Copy command；任务不再运行
  时增加 Remove result。无详情时仍打开窗口 Tool options。菜单内用
  `↑` / `↓`、Enter、Esc，切换任务或隐藏页面会关闭旧目标菜单。
- Tasks Tool options 固定为 Refresh / Launcher；Launcher 与 `⌘L` 走同一
  返回入口，Keyboard help 只保留在页脚。任务详情与 Copy command 使用
  POSIX argv 引号规则，不用无转义空格拼接。
- Clipboard History / Shelf、Application History、Plugin Detail 与结果详情
  消费无 Control / Option / Command 的 Tab / Shift+Tab；Settings 的焦点遍历和
  WebView Tab 保持各自规则。输入法组合中的 Tab 仍归编辑器。

## 隐藏与 15 秒保留

产品以菜单栏后台应用启动，Launcher 初始隐藏且不抢焦点。点击其他应用使
Launcher、其文件搜索或内容页失焦后，由系统窗口 / 应用通知驱动，在 220 ms 宽限后确认失焦再隐藏。Settings 与
Running Tasks 是普通层级窗口：失焦后保持打开，但会像普通应用窗口一样被其他
应用覆盖，不使用所有 Space。Clipboard Shelf 保持置顶和所有 Space，失焦不隐藏；
点击原生关闭按钮、页面关闭入口或根页 Esc 才结束 Shelf 窗口。Quick Look 可见时
暂停 Launcher 的失焦判断，关闭系统预览后再恢复普通焦点规则。从菜单栏或
Action 再次打开已最小化的 Settings / Tasks / Shelf 时先恢复窗口，再置前并
恢复键盘所有权。Shelf 首次定位到鼠标所在屏幕右下角，允许拖动并在本次进程内保留位置。

“复制并返回”只在复制完成时 Shelf 仍为 key window 的情况下切回原应用；关闭、
窗口失效或操作编号过期的迟到结果仍可完成剪贴板写入，但不能抢走当前应用焦点。

隐藏立即撤销 Session 执行资格。预览和排队动作的旧请求在恢复后仍然失效。

| 保留的界面 | 15 秒内再次唤醒 |
|---|---|
| 有查询、文件页、菜单 / 二级层、输出或非 Current App 已选 Scope | 恢复原操作和 Context；未完成查询可继续或重发 |
| 空白 Global / Current App 首页 | 隐藏时即释放会话，唤醒后按本次入口采集前台应用 |
| 明确 Global Action 快捷键、来源 / 扩展设置变化 | 原会话失效，重新捕获所需 Context |

恢复取消旧期限，再次隐藏重新计时。空白首页隐藏时、有内容的会话保留满 15 秒时，
释放临时查询、Context、结果、图标缓存和工具页数据，并清空会话编辑器的内容与撤销记录。
GPUI 与 Rust 分别校验会话身份，防止旧期限清理后来唤起的会话。设置草稿由设置页单独持有。

已经启动的后台任务独立于 Launcher，隐藏与会话清理不停止任务；退出应用结束任务。空白首页新采集规则修复了“复用旧空白 Global、一直不进入 Finder / 其他当前应用”的问题。

## UI 与验收

窗口高度、原生预览窗口和键盘焦点由 Host 决定；Rust 不传像素布局，也不复制候选 index 作为业务身份。shared UI 与材质见 [UI 设计](ui-design.md)。

Launcher 停用 AppKit 的窗口进出动画。输入即时更新，常规查询等待当前快照时保持原有窗口尺寸，
同一次 GPUI 更新中的多个尺寸请求合并，按最新布局从窗口顶边调整。页面切换独立确定目标尺寸和焦点。

回归覆盖内容自填 / Esc、晚到 Scope、非空 Global 查询切回 Current App、
对象返回、连续列表、IME、重复 show 后输入连接、失焦 generation、Settings /
Quick Look 失焦豁免、标题栏关闭草稿路由、15 秒恢复 / 超时、空白首页立即释放，以及查询快照乱序时的尺寸等待。
原生窗口动画、键盘与内存采样按 [GPUI 桌面测量](performance.md#gpui-桌面测量) 验收。
历史现场和验收记录见 [归档](archive/README.md)。

系统预览使用选中行入口、结果浏览状态的空格及 macOS `⌘Y`。`⌘Y` 可开关预览，
按住不重复请求；面板打开时可按 `⌘L` 关闭并返回查询编辑器。输入查询时空格仍可
编辑，原生面板不算外部失焦。只有查询编辑器拥有焦点时 Space 才归预览；页面按钮
聚焦时 Space / Enter 激活按钮。完整规则见 [系统预览](native-preview.md)。

## GPUI 键盘迁移核对

迁移基准为 Flutter 历史提交 `81c523c` 的 Launcher、ToolPageShell、Clipboard History / Shelf、
Application History、ExecutionSessionsView、Settings。以下区分源码路由与桌验；
已有路由不能代替真实 IME / 焦点验收。

| 页面 / 状态 | 按键与行为 | GPUI 核对 |
| --- | --- | --- |
| Launcher | ↑↓ 选择、Enter 执行、空查询 ←→ 菜单层级 | 已有；修饰键保留编辑语义 |
| Launcher | Tab / Shift+Tab、Ctrl 变体循环可见 Scope | 已有回归，不能经过不存在的 None 标签 |
| Launcher | ⌘Enter 模式选择，Esc 关闭模式菜单 | 已有；带修饰方向键不再误选模式 |
| Launcher / 文件 | ⌘K 对象操作、Space / ⌘Y 开关预览；预览中 ⌘L 返回查询 | 已有；IME / 编辑状态优先，repeat 只处理首次 key-down |
| 文件 | Tab 类型过滤、Esc 清查询 / 返回、末尾继续加载 | 已有 |
| Clipboard History | ↑↓、Enter 复制、⌘K 操作、Space / ⌘Y 开关预览、Esc 清查询 / 返回 | 已有；普通 Backspace / Delete 留给编辑器并关闭预览，不删除记录 |
| Clipboard History / Shelf 操作菜单 | 打开时 ⌘K 关闭；⌘F / ⌘R 等命令先关菜单再继续；输入文字恢复搜索；目标过期时 Refresh 仍可用 | 历史 Build 159 桌验通过；过期 target 仅有回归；v0.14.0 未逐项复验 |
| Shelf | ⌘1…9 复制、⌘Enter 复制返回、⌘F 搜索、⌘R 刷新、⌘K 操作 | 已有 |
| Shelf | ⌘↑↓ 首尾、PageUp / PageDown 按可见行翻页 | 补齐翻页，页边界不回环 |
| Shelf 删除确认 | ⌘Shift+Backspace 打开；←→ 切按钮、Enter 确认、Esc 取消 | 已有；不添加原版没有的裸 Backspace 删除 |
| 应用历史 | ↑↓、Enter 打开或复制、PageUp / PageDown、Esc；行内 `…` / 右键打开 Actions | 补齐翻页、不能打开时的复制分支和空结果刷新菜单；不绑定 `⌘K` / `⌘R` |
| Tasks | ⌘F 搜索、⌘K 操作、⌘R 刷新、Esc 清查询 / 返回 | 已有；不依赖搜索框或结果行持有焦点，⌘K 不接受 repeat |
| Tasks | 无 Shift / Command / Control / Option 的 Esc、↑↓、Enter 由原生键路由进入页面；Shift 修饰键保留编辑语义 | 历史 Build 159 桌验通过；v0.14.0 未逐项复验 |
| Tasks | ⌘. 停止、⌘⌥↑↓ 切任务、⌘L 返回 Launcher | 补齐；空任务页同样可用，移除普通 s 停止任务的误触路径 |
| Settings | ⌘, 打开、⌘S 保存、Esc 逐层取消、Tab / Shift+Tab 控件焦点 | 已有；输入法组合期间不截获 Esc / Tab，快捷键录制和 Esc 不接受 repeat |
| Web / 详情 | 插件内部编辑、返回、复制及滚动；宿主省略号 Actions 提供 Reload / Back | WebKit 所有权保持；完整物理矩阵待核对 |
| 全局 | 两个启动器热键、自定义 Action / Context 快捷键 | 使用现有注册与冲突校验，不重新解释绑定 |

修改键导航与翻页边界已有回归；Tasks 筛选后的滚动定位使用可见列表序号，
而非完整任务数组序号。真实按键是否到达页面、中文候选窗、辅助技术与插件内部
快捷键仍需按当前发布构建分别验收。历史 Flutter 路由只作为迁移对照，不宣称
所有迁移操作已在 v0.14.0 逐项物理通过。
