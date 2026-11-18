# 当前工作树审查实施记录（2026-11-17）

本轮按用户选择实施 P1→P5。起点为 `codex/experiment-gpui-kit` /
`b428899c9ce70c92e12de24950405222ea3b6c28`。开始时有 58 个已修改文件和
16 个未跟踪文件；这些内容已在仓库外逐文件备份并保存哈希、Git 状态与补丁。
备份保存在仓库外的临时目录，不随提交或发布物上传。

已有工作不记为本轮新增成果；以下描述的是相对备份起点新增的修复和调整。
本轮随后完成了独立 App 打包、签名检查、预览窗口冒烟和隔离数据目录下的真实
Core 启动，没有替换默认 App、改个人数据或停止现场已有实例。混合工作树仍留作
可审查改动，未将原有编辑合并提交，也未推送。自动化结果与物理桌验分开记录。

## 阶段落地

| 阶段 | 已实施范围 | 验证状态 |
| --- | --- | --- |
| P1 | 解除工具/菜单/对象行数上限；设置、刷新、授权引导；窗口回收、书签和剪贴板读取；原生材质、辅助窗口和 Web 宿主 | 已落地，自动化通过；完成启动与首屏冒烟，完整物理矩阵待验 |
| P2 | AX 批量读取；进程内 OSA、NSWorkspace 和多范围 Spotlight；Core 快照/匹配、文件记录/配额、历史缓存、SQLite 与 IPC | 已落地，自动化通过；调整见下文 |
| P3 | 五类工具列表及任务日志虚拟化；80 ms 调度；提示、源路径、下拉选择、Shelf 移动和行菜单 | 已落地，自动化通过；完成启动与首屏冒烟，完整物理矩阵待验 |
| P4 | 死通道及旧游标移除；视图、设置、原生层及 Core 发布/对象管线拆分；强类型指令和工程脚本去重 | 已落地，自动化通过；拆分边界见下文 |
| P5 | 计划与构建记录归档；当前指南和链接修复；全仓门禁 | 已完成，自动化通过 |

## 功能与资源安全

- 主 Launcher 的空查询/普通查询仍使用紧凑行数。FileSearch 依据当前代码的
  `state.file_search` 判断，子菜单和对象动作可访问全部可见项；末行无下一批时
  继续普通选择行为。投影示例现在断言真正投影 10,000 行。
- Application History 的 Refresh 回调使用包含 fallback Copy 在内的真实索引。
  设置动作搜索加入本地化标题；动作选择器纳入模态状态和焦点范围；登录项只在
  开关发生变更时执行系统操作。重复 ARB 键现在使生成器失败。
- 主窗口隐藏时立即撤销副作用；保留工作区在 15 秒后自动清空 Web 文档和页面
  快照，恢复后的旧计时任务不得清理当前界面。Tasks / Shelf 关闭观察器释放壳层
  保存的实体句柄。普通隐藏保留有界图标 LRU。
- 安全作用域 URL 使用逐个 RAII 守卫；部分恢复失败、替换和析构都会结束已开启
  的租约。剪贴板格式只读取一次 NSData；成功 PNG 不再重复拉取 TIFF，可选大格式
  超限不丢弃已成功的表示。写操作消费随操作返回的快照。
- 切回 Opaque 时恢复原生 `opaque` 与窗口背景。安装单个窗口材质不再覆盖进程级
  外观状态；系统和设置变更同时刷新主窗口、Shelf 和 Tasks。辅助窗口按自身用途
  更新语言与搜索占位。
- HTML 在已有 head 内、已有资源之前注入策略；补齐无 head、doctype 片段、注释、
  大小写和含引号的标签边界。原 HTTP CSP 继续生效。Web 回复使用字符串帧；共享
  preview fraction、浅色 elevated/action 色与 select 箭头保持同步生成。

## 热路径与边界

- AX 扫描批量取得 Role、Title、Identifier、Enabled、Children 和快捷键；递归复用
  已读节点，以集合统计父级身份唯一性。显式授权引导可以再次触发，后台线程不再
  同步等待主队列。
- Finder / 浏览器使用最多 8 个串行 OSA 编译缓存，输入仅来自固定只读脚本。外部
  Apple Event 改为捕获的 PID，发送前和返回后验证 bundle / 进程启动身份；解释器
  和事件设置期限。合成原生测试覆盖 Unicode、NUL、输出上限、错误类别、超时、
  PID 替换和旧身份拒绝。首次系统编译/授权及真实应用仍列为桌验。
- 资源打开改走 NSWorkspace；选定 JetBrains 安装仍使用其原有显式参数交接。
  Spotlight 用一次 MDQuery 的 search scopes 数组提交所有根，而不依赖未文档化的
  重复 `mdfind -onlyin` 语义。返回路径仍经过授权根、隐藏/排除、真实路径和存在性
  核验；Combined 与 ripgrep 并行，均保留取消和期限。
- Usage 查询使用借用键；候选共享 ActionDescriptor 的 Arc；注册表无动态工厂时
  不克隆。空查询 offers、Scopes 与快捷键目录用单项缓存，完整键包含注册表、
  Context、Facts、Usage 和排名时钟；动态工厂保留请求版本，静态缓存重新绑定本次
  Context。Context 根索引替代重复父链查找，拼音状态使用固定栈数组。
- 文件类型先过滤，ASCII 名称走快路径。二进制匹配记录保存 score 与归一化名称，
  归并不再重算。目录队列在读端追上缓冲尾部时才 flush，保留完整记录边界。
  每查询临时存储预算为 512 MiB；失败/失效排序段删除并归还预算。前 128 个投影
  缓存仅服务展示，实际执行仍重新验证目标。
- 配置的临时写入和 fsync 在内存发布锁之外准备，锁内复验期望配置再提交。
  进程管理先预留并发名额，再于锁外 spawn；关闭或启动失败释放名额。
- 应用历史解析按来源、规范路径、大小、mtime 及 WAL 大小/mtime 缓存；会话、配置
  或来源变化失效。JSON 原文直接交给 QuickJS；应用目标查找复用 ApplicationIndex。
  菜单准备去掉第二次完整读取，执行守卫仍完整重读并校验对象。
- Usage 增加 source_app 索引、GROUP BY、缓存语句及 NORMAL 同步配置；旧 JSON 键
  在数据库打开事务中幂等迁移。剪贴板仅超限时淘汰，search_text 只存最多 128 KiB
  的 UTF-8 安全前缀，原始内容仍完整保存。Module Storage 使用缓存语句。
- wire 请求单次 JSON 解码，保留重复/未知字段和形状限制；后台 QuickJS 直接分发
  WebCall。GPUI 回复元数据只解析一份，请求与响应的调试输出有日志级别守卫。
  文档传输同时支持字符串与兼容二进制回复，只接受可见文档中仍待处理的请求；
  无效 JSON、超限、未知 ID、缺失结果或结果/错误同时出现的帧不消耗 pending 名额。

## GPUI 与代码组织

Clipboard、Shelf、Application History、File Search 和 Tasks 使用 uniform_list，
回调只构造可见范围；Tasks 输出按缓存的行范围虚拟化，超长行分块并允许横向滚动，
完整复制不变。剪贴板日期采用有界格式缓存，并使用记录当时的时区偏移，保留 DST。
搜索和预览切换各有 80 ms 防抖及 generation 检查。任务跟随输出只在状态翻转时通知。

ClipboardNotice 已显示；设置校验及目录授权失败提供中英文信息。源路径编辑器位于
当前源卡片内；语言和引擎通过显式菜单选择，保存后应用。Shelf 状态文字提供拖动区，
搜索框保持文本操作，窗口保留本次进程内的位置并在重开时裁剪到屏幕。动作菜单依据
当前列表滚动位置锚定选中行，并共享 menu_popover 表面。

Launcher 的内联测试迁入 `view/tests/`，功能方法迁入 `view/controllers/`；辅助窗口有
独立 Render 壳，只初始化自身查询编辑器和必要的共享控件。七类设置表单各有模块，
macOS window / status_item / observers / activation 分开；原生窗口通过 identifier 绑定。
私有落盘共用 atomic_write_private。poll 的首次回复及 follow 共用事件分发。

SessionCoordinator 封装 Session 锁及 execution epoch；Resolver 的发布组装移入
`session_runtime/publication.rs`，文件进度和完整快照共用 publish_state。文件/普通对象
与历史对象共用票据、Usage 查询、匹配和发布复验。声明式注册迁入 application；Core
生产代码不反向依赖 anywhere-plugin。WebPackageOperation 与搜索引擎使用枚举。

包启动冒烟进一步发现，Preview 壳的状态栏 Settings 回调虽然被触发，却在 app 分发层
因 `shell.live.is_none()` 被直接丢弃。`LauncherView::open_settings` 原本已支持
Preview/Ready，并对 Starting/Failed 显示状态；修复后该方法返回是否真正进入 Settings，
状态栏直接使用返回值，不再按 live runtime 拦截。新增
`preview_settings_open_without_a_live_runtime` 回归，并强化 Starting 状态返回 false
的既有断言。QA Preview 实机已显示 General、Shortcuts、Context、Search、Clipboard、
App History、Features、Done 与录制器；截图位于
`build/anywhere-settings-fix-window-20261117.png`。

快捷键注册进一步改为默认 `control+alt+Space` / `alt+Space`。读取设置时仅当两项
仍精确等于旧默认 `meta+alt+Space` / `meta+Space` 才迁移，任一自定义键对保持原值。
录制器和动作快捷键校验接受 Option 单独作为修饰键；GPUI 回归实际录制
`control+alt+Space`、`alt+Space` 与 `alt+KeyK`。QA Preview 中真实按
`Option+K` 得到 `⌥K`，恢复后显示 `⌃⌥Space` / `⌥Space`。新安装和旧默认设置
迁移后的真实注册日志均记录
`global_hotkeys_registered current_shortcut=control+alt+Space
universal_shortcut=alt+Space action_shortcut_count=1`。截图位于
`build/anywhere-hotkeys-fix-settings-20261117.png`。

LauncherEvent / subscribe / SessionSubscription / ContextDescriptor、旧 Scope ID
数组、不可达 Json Context、双向 JSON 文件游标、旧工具模块/窗口常量、无消费者原生
字符串、旧探针/fixture 和确认无调用的 ARB 键已清理。文件图标与过滤规则由同一
file-types 资源生成。构建脚本复用 SDK 安装/构建，starter 包名来自 npm pack 输出，
ZIP 启用压缩；架构检查先遮蔽字符串/注释，再处理测试代码。

## 核验后调整的审查建议

1. `InputMode` 在该快照中并非 FileSearch 判定字段，使用实际的 file_search 状态。
2. Module Storage 仍保留原 journal mode。它的共享连接本来就由一把 Mutex 串行化；
   直接开启 WAL 在损坏文件修复/重绑与文件边界回归中失败，没有并发读收益。已落地
   prepare_cached，没有以改变恢复契约换取配置上的 WAL。Usage 自有 WAL 不变。
3. `anyhow::Error::new(RuntimeError)` 不会丢失类型；新增带 context 后 downcast 的测试
   明确覆盖 Stale 和稳定 code，不把此项误报当作修复成果。
4. 原生 localization 仅保留有消费者的 InfoPlist automationUsage。ARB 删除数量以
   实际调用者为准。web-text/dist/sdk 在起点已经是忽略的构建产物，没有已跟踪文件
   可删除。
5. Project 的字符串属性仍是当前 Context 元数据契约；本轮没有把 Node/Cargo 类型
   引入 Core。内部包操作与引擎指令已强类型化，元数据契约没有借重构扩大权限。
6. 辅助窗口继续共用可复用控制器，冷页面模型保持默认空值；独立 Render 与编辑器
   初始化已拆开，没有宣称所有控制器字段都完成按类型拆分。

## 最终自动化证据

2026-11-17 在当前混合工作树运行完成：

| 检查 | 结果 |
| --- | --- |
| `scripts/check-gpui.sh` | exit 0；266 项 GPUI 测试通过；生成物、plist、fmt 和严格 Clippy 通过 |
| `scripts/check.sh` | exit 0；Rust 合计 951 passed / 6 ignored / 0 failed；严格 workspace Clippy 通过 |
| SDK / Web | 52 项 SDK 测试、31 项 JSON Workspace 测试通过；后台 QuickJS、1 MiB / Unicode、真实 Rust broker、独立模板构建及打包一致性通过 |
| 原生边界 | AX / Clipboard、进程身份、应用打开、OSA 和多范围 Spotlight 合成回归通过 |
| 文档与工程 | 112 份 Markdown、899 个本地链接；代码格式、版本、生成物及架构边界检查通过 |
| 变更与保护 | `git diff --check` 通过；起点备份 74 个文件的 SHA-256 全部与原始归档一致 |

6 项 ignored 保留其原有环境/耗时前提，没有用忽略替代失败。全仓门禁曾捕获字符串
Web 回复缺少 pending / 形状校验的问题，修正后重新完整运行上述门禁；此处只记录
最后通过结果。

最终日志另存于起点备份目录的 `final-gpui-gate.log`、`final-full-gate.log` 与
`final-docs.log`。同目录 `task-changes.patch` / `task-changes.json` 记录相对任务起点
的变更，排除原有编辑本身；补丁在临时起点快照上验证应用并逐文件校验结果哈希。

## 本轮包与桌面冒烟证据

最终审查包为 `build/Anywhere-review-20261117-hotkeys-fix.app`，指向当前混合工作树的 HEAD
`b428899c9ce70c92e12de24950405222ea3b6c28`。包内版本为 `0.14.0+1`，主程序为
arm64，最低系统为 macOS 15，`LSUIElement=true`。`codesign --verify --deep
--strict` 和 Designated Requirement 均通过；签名主体为本地
`Anywhere Local Code Signing`，CDHash
`c25b385ddd50b3d94265d10898c1b964cab162ff`。`Info.plist` 通过 lint，Mach-O
只依赖系统框架，Bundle 内没有额外 framework、dylib 或 so。主程序 SHA-256 为
`f988d2e1737f497fa151ac6fcfc6f04878eaf8a791b2866ff1d96c007b204e56`。
`spctl --assess` 以 status 3 拒绝该本地签名包；这符合当前没有 Developer ID /
公证的边界，因此该产物只用于本机验收，不能视为可分发发行包。

使用独立 `ANYWHERE_DATA_DIR` 和 `ANYWHERE_PREVIEW=1` 从该包直接启动，日志确认
Accessory 包显示一个 `648×360.5 pt` 窗口，窗口与首行均获得 key/focus，Liquid
Glass 材质安装完成，状态项可用。Retina 截图
`build/anywhere-review-preview-window-20261117.png` 为 `1296×722`，首屏显示
Open in Finder、Git Status、File Search、Browse Menus 与 Clipboard History。
日志为 `build/anywhere-review-preview-live-20261117.log`。

为了在没有停止用户现有实例的前提下使用 Computer Use 的唯一 Bundle 绑定，另从
该包复制了 `build/Anywhere-Review-QA-20261117.app`，仅修改临时
Bundle ID / 显示名并重新签名；可执行逻辑来自同一构建。该 QA 包装中实际验证了：
搜索文本触发结果过滤，Return 进入执行桥提示，Esc 清空查询并恢复首屏，Clipboard
和 File Search 在无 live bridge 的预览态显示明确不可用提示，没有误执行真实动作。
截图存于 `build/anywhere-review-qa-window-20261117.png`，日志存于
`build/anywhere-review-qa-live-20261117.log`。

关闭预览后又用原审查包和新的隔离数据目录启动真实 Core。进程可执行路径指向审查
包，运行策略为 `Accessory`，启动后保持非激活隐藏窗口。隔离目录创建了
`clipboard-history.sqlite3`、`extension-storage.sqlite3`、`usage.sqlite3`、
`web-storage.sqlite3` 和合法 `features.json`；四个 SQLite 的
`PRAGMA integrity_check` 均为 `ok`，进程打开的数据句柄也全部位于该目录。
现有默认实例占用 `⌥⌘Space`，因此本实例按设计记录
`global_hotkeys_unavailable`、恢复旧绑定并把状态菜单改为无快捷键标签，没有 panic
或锁错误。日志为 `build/anywhere-review-live-20261117.log`。

另用 QA 包装和隔离 `settings.json` 把热键改为 `⌃⌥⇧J` / `⌃⌥⇧K`，真实 Core
启动日志记录两条全局快捷键注册成功，状态菜单同步显示这两个键位。证据日志为
`build/anywhere-review-qa-live-hotkeys-20261117.log`。该结果证明注册与标签路径，
没有替代物理按键回调、冲突后注销/重注册或 IME 组字验收。

热键修复后的真实注册证据为
`build/anywhere-hotkeys-fix-live-20261117.log` 与
`build/anywhere-hotkeys-migration-option-20261117.log`。后者的输入设置仍保存旧
默认键对并包含全局 `alt+KeyT`，启动后迁移到新默认键，且三个快捷键全部注册成功。

上述运行均未替换 `build/Anywhere.app` 或结束既有 PID。临时启动实例在取证后
退出；因此本轮可以称为本机包构建、签名、隔离启动与首屏冒烟通过，不能称为发布
签名、公证或完整物理验收通过。

## 后续完整物理验收

以下完整矩阵仍有项目未在本轮执行。请继续使用隔离的合成数据/测试配置检查，
结束后恢复系统选项：

1. 搜索超过 64 个合成文件并加载多批；测试超过 7 项的应用子菜单和对象动作，确认
   鼠标、↑/↓、最后一行、⌘K 与返回均可访问完整列表且选择稳定。
2. 为 Clipboard 准备接近 1,000 条测试记录，快速输入、连续上下选择并滚到最后一项；
   在不同滚动位置开菜单，确认菜单靠近触发行、不越窗口，文本无跳动。测试图片
   PNG/TIFF、仅大格式和有效文本回退，以及通知横幅。
3. 在 App History 对不可打开条目开动作菜单，点击 Refresh 确认不发生 Copy；查询时
   修改合成来源/WAL，下一次读取应刷新，旧菜单或已换目标不得执行。
4. 中文设置中按中文动作名搜索；打开添加快捷键弹层，反复 Tab / Shift+Tab，确认
   焦点只在弹层内，Esc 先关弹层。使用中文 IME 检查 Enter/Esc/Space 组合输入。
5. 编辑两个历史源的自定义路径，确认仅当前卡片的编辑器和保存目标改变。分别选择
   语言、搜索引擎后取消，确认未应用；保存后检查已打开的主窗、Shelf、Tasks 标签。
6. 登录项处于待批准时保存无关设置，确认不会再次打开系统设置；只有主动改变该
   开关才执行登录项操作。显式 Accessibility 引导可再次点击，其他动作继续可用。
7. 拖动 Shelf 顶部状态文字；搜索文字仍可选取和编辑。关闭/重开应保留本进程位置；
   在小屏、多显示器、负坐标和不同缩放下重开均在可见范围内。
8. 同时打开主窗、Shelf 和 Tasks，切浅/深外观、Reduce Transparency、Increase
   Contrast 与 Reduce Motion；检查全部窗口、Opaque 底色、菜单/控件对比度和动画。
9. 开 JSON Workspace 和完整 HTML Web 插件，验证主题、双面板、select 箭头、刷新/
   返回与输入冻结。对插件替换后的迟到 reply/state/close 回调继续检查文档隔离。
10. 隐藏含 Web/历史/文件工作区的主窗，15 秒内恢复应保留内容并重新授权副作用；
    超过 15 秒且不再唤醒时观察文档/helper 归属和资源释放。记录主进程及其 helper
    的连续 footprint，不能仅凭单次 RSS 判断是否泄漏。
11. 反复打开/关闭 Running Tasks，运行大输出和长行任务，检查滚动、跟随、完整复制、
    停止和错误提示；关闭后确认窗口实体不再被 ProductShell 保存。
12. 在 Finder、Safari、Chrome、Edge 中分别测试首次授权、拒绝、无窗口、切换窗口、
    读取中退出/重启；结果只能来自激活前捕获的实例。记录首次 OSA 编译/授权耗时，
    不用合成脚本测试推断真实应用行为。
13. 使用合成文件/URL 检查默认应用打开、指定安装打开、Finder Reveal、Terminal 和
    JetBrains 项目交接。独立检查 Quick Look、状态项、全局热键及 VoiceOver 顺序。

上述完整物理矩阵仍无全量通过结论。本轮只对包结构、签名、隔离启动、首屏
搜索/Return/Esc、热键注册与冲突降级、SQLite 初始化形成了证据；真实 IME、
系统权限、Quick Look、WebKit、材质组合、Shelf 拖动、15 秒工作区清理、
状态菜单点击、VoiceOver 和整 App 性能数据仍未验证。
