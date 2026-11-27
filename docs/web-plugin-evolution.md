# 第三方 Web 插件架构审查与 V0.15 建议方案

2026-11-27，审查基线为 `964dd1e1`。本文是实施前提案。现有包的开发步骤、权限和额度仍以[插件开发指南](plugin-development.md)及[运行时契约](web-plugin-design.md)为准。

准备开工时，按[Phase 1 实施手册](web-plugin-phase-1.md)的版本、权限和任务顺序执行。本页保留设计取舍和源码证据。

**建议保留 WebView、按需 QuickJS 和 Rust 能力代理，做局部重构。** 现有架构足以让作者独立交付文本工具。开发时的主要成本是反复导入才能知道包为什么装不上、命令为什么没出现。使用时，插件能识别工单号却只能复制链接，能看到用户选中的 CSV 文件名却读不到内容。结合 Phase 1 定稿，建议先交付作者工具、跨平台快捷键和安装审查，再接入后台反馈、窗口控制、外链和选中文件的读取与预览。各项能力可独立验收。

## 当前作者能做到哪一步

| 用户入口 | 现有能力 | 对作者的实际意义 |
| --- | --- | --- |
| 搜索命令、Action 快捷键、Current App 中的适用动作 | Manifest 静态命令进入现有 ActionRegistry，按 Context 匹配并沿用 Launcher 排序和用户绑定。来源应用与实时前台应用可分别按权限读取 | 标题打开可以是空输入。需要对象的命令可声明 `requireContext`，识别 JSON 也有静态匹配器。命令标题和关键词由包提供，宿主尚无第三方动态结果贡献点。[命令注册](../crates/anywhere-core/src/application/web_plugins/catalog.rs#L120) |
| 打开工具页或独立窗口 | WKWebView / WebView2 加载本地资源。SDK 提供启动、Context / Input 订阅、取消、语言和共享样式。宿主提供独立工具窗口及其文档身份 | HTML、React 等均可处理业务。作者无需管理窗口、主题或 IPC。搜索框内容由 `input.read/write` 控制，与启动时投影的对象有不同语义。[SDK 门面](../packages/plugin-sdk/src/api.ts#L15) · [窗口绑定](../crates/anywhere-core/src/application/web_plugins/invocation.rs#L84) |
| 自动识别或显式运行后台命令 | 自动 resolver 只读获准 Context / Input / 应用信息，只产生本包命名空间的数据。显式命令还可使用获准的剪贴板文本和包私有 KV。QuickJS 每次按需创建，有执行时间、内存和调用次数限制 | JSON Workspace 已用一个普通包组合解析、无界面复制和 Web 编辑，共用偏好。适合离线、短时任务。当前 QuickJS 在 App 进程内运行。[解析器权限](../crates/anywhere-core/src/application/web_plugins/background_provider.rs#L182) · [混合包](../plugins/examples/json-workspace/manifest.json) |
| 安装、更新、停用、回滚 | Rust 校验包字节、平台和声明权限。导入批准绑定本次审查和内容摘要，同 ID 更新沿用本地安装来源。可回到上一份获批内容 | 包 ID、命令 ID、安装来源、摘要与本次执行身份各有用途。用户设置快捷键后可以持续使用同一命令，每次执行仍须复验资格。本地来源不能证明发布者身份。[安装记录](../crates/anywhere-core/src/application/web_plugins/packages.rs#L115) · [命令身份](../crates/anywhere-core/src/core/action/web_command.rs#L12) |

## 发现与优先级

| 优先级 | 源码或实测所见 | 影响及处理建议 |
| --- | --- | --- |
| 本轮修复 | 批准更新时，仅当新摘要和当前摘要不同才设置 `previous`。已安装 A、更新到 B、再次导入相同的 B 后，记录中的上一版会变为空。[批准与回滚](../crates/anywhere-core/src/application/web_plugins/packages.rs#L168) | 开发者重复导入会失去回到 A 的入口。对已启用的同来源、同摘要包保留上一版与现有实例。已停用包的重新启用作为明确操作处理。以 A → B → B → 回滚 A 验收，并覆盖重启后的结果。 |
| 本轮修复 | 后台命令先模拟文档 ready 并忽略其回复，随后才运行导出函数。ready 会记录 Usage。返回的业务值在普通执行入口被丢弃，成功时关闭 Launcher。[后台执行](../crates/anywhere-core/src/application/web_plugins/background_action.rs#L143) · [记录 Usage](../crates/anywhere-core/src/application/web_plugins/rpc_dispatch.rs#L209) · [执行结果](../crates/anywhere-core/src/application/execute.rs#L24) | 脚本失败也可能增加排序计数。抽出有限任务的开始、完成、取消三个时点，执行成功只记一次。记录失败作为已完成动作的警告。Web 页面继续以有效 ready 表示打开成功。按下文的版本约定接入定稿所需的后台结果反馈。 |
| 本轮补齐 | SDK 已有仓库外模板及可注入的 transport。发布给作者的 CLI 只有 `init` 和后台模板选项，包内工具负责收集资源，完整语义校验在 Rust。后台运行时又将多种脚本错误合并为一个失败原因。[创建器](../packages/plugin-sdk/tools/create.mjs#L100) · [包检查器](../crates/anywhere-plugin/examples/inspect_web_package.rs#L5) · [后台错误出口](../crates/anywhere-core/src/application/web_plugins/background_runtime.rs#L130) | 修改业务后要反复构建、打包、导入才看到真实反馈。开发工具应按包、命令、字段或导出名提示如何修正，并支持用合成数据复验。默认诊断只含错误类别、位置和额度。 |
| 本轮补齐 | Web 与显式后台命令共享 SDK 方法，自动 resolver 通过独立的只读分派执行。Manifest 与 API 当前严格匹配版本 1，未知字段和权限会被拒绝，SDK 的 TypeScript 接口也不能区分 resolver 与命令。[协议](../crates/anywhere-protocol/src/web_plugin.rs#L4) · [版本检查](../crates/anywhere-plugin/src/web/manifest.rs#L20) · [后台 SDK](../packages/plugin-sdk/src/background.ts#L60) | 作者可以写出类型检查通过、只在 resolver 中被拒绝的调用。给新模板只读 resolver 门面和命令门面，由同一份能力说明驱动文档、诊断和测试。Rust 继续逐次授权，旧 SDK 门面继续兼容。 |
| 发布参考包前修复 | 本次重建 JSON Workspace 0.7.3 时示例门禁失败。逐项比较 ZIP 后，唯一不同的资源是 Manifest 的 110 个换行，内置包为 CRLF，本机重建为 LF。两者解析出的 Manifest 相同。[逐字节门禁](../scripts/check-web-examples.sh#L12) · [打包资源写入](../packages/plugin-sdk/tools/package.py#L81) | 作者拿同一份源码构建会得到不同包。先固定受版本控制的打包文本输入换行，双平台干净检出重建并比较，随后同步内置包。导入时仍审查实际字节及摘要。 |
| 下一阶段评估 | 一次插件目录发布会清空已有 invocation、取消后台任务、撤销子窗口资格，并可能推进查询 revision。这是现行全目录失效契约。[目录发布](../crates/anywhere-core/src/application/web_plugins/application.rs#L43) · [清理实例](../crates/anywhere-core/src/application/web_plugins/invocation.rs#L175) · [窗口契约](architecture.md#从-flutter-桥接到进程内共享) | 更新插件 A 时，正在使用 B 的窗口也可能要求重开。先用独立开发配置消除日常迭代对正式窗口的干扰。只有多包并行工作成为实际需求时，再按安装来源、摘要及授权代次缩小撤销范围，并保留查询目录的原子发布。 |

前次架构审查通过了 66 项 Core 插件测试、SDK 检查、仓库外两个模板，以及 JSON 混合包与真实 Rust 宿主的协议测试。示例整体验证停在上表的包字节比较。当时 GPUI 和完整工程检查均停在基线已有的 Rust 格式差异。

## 用候选插件决定能力范围

| 插件与一次完整操作 | 已能复用 | 做完这件事还缺什么 | 建议 |
| --- | --- | --- | --- |
| 时间戳、Base64、JSON 转换。选中文本，按快捷键复制结果 | 自有 Context、后台命令、文本剪贴板、Usage | 离线执行已够用，作者需要测无输入、超时和拒绝授权 | 作为零新增权限的基准 |
| 工单跳转、站内文档快捷入口。选中工单号，回车进入对应页面 | 只读 resolver 识别编号，显式 Action 获取对象 | 让默认浏览器打开包声明的 HTTPS 站点 | 本轮首选，验证从识别到执行的完整链路 |
| Markdown 阅读器、CSV 检查器。选中一个本地文件，查看渲染或列统计 | 文件名、类型和不透明句柄，Web UI，独立窗口 | 读取本次明确选中的单个小文本文件 | 本轮候选，仅在两端能证明文件身份时交付 |
| 片段库、离线词典。在 Launcher 输入后逐项选择并复制 | 小型 KV 或包内词库，Web 页内搜索，剪贴板写入 | 要在宿主原生列表中选条目，需要命令内的查询、稳定对象 ID、分页及对象动作 | 后续先做显式进入的列表，避免每次唤醒调度所有插件 |
| 翻译、工单查询。选中文字或编号，得到远端结果 | Context、Web 页、显式命令 | 受限 HTTP 请求、取消与超时、按站点授权。有账户时还需系统凭据库 | 后续单独设计。浏览器跳转可先满足只需打开站点的插件 |
| 图片 OCR。选中图片，复制识别文本 | 资源元数据和文本剪贴板 | 图像输入、解码像素预算、OCR 服务及异步任务反馈。剪贴板图片另需授权 | 后续按平台验证，文本读取接口保持单一用途 |
| 项目构建、浏览器标签切换、向笔记仓库追加内容 | 内置项目任务、来源应用和当前 URL 等已有产品入口 | 分别需要受控任务模板与输出、准确的浏览器会话身份、授权目录内的写入与冲突处理 | 保留各自领域的身份和执行所有者，将来逐项开放。浏览器事项沿用[独立规划](browser-feature.md) |

## 本轮拟交付的设计

### 让作者在仓库外完成一次迭代

1. **取得工具。** 随对应 Host 预览提供 SDK 安装包、校验和、支持平台及兼容表。从指定 GitHub Release 获取可移动的 vendor 模板，提供 Web、TypeScript Web、后台和混合包起点。用 JSON Workspace 验证混合包，模板标注已实际测试的平台。
2. **提前发现错误。** 采用定稿中的 Node CLI 命令 `anywhere-plugin validate` 和 `pack`。源码阶段提供字段位置、类型与权限检查，打包时再检查完整产物。在 CI 中将实际产物交给与应用导入相同的 Rust 包检查器，普通作者只需 Node。检查结果标识目标宿主版本、平台、命令、需求权限和错误位置。导出函数缺失应在隔离的合成运行测试中定位。
3. **缩短反馈。** 建议给模板增加一个可直接运行的 `npm run dev`，启动本地预览、监听源码变更并在开发入口装载 `@anywhere/sdk/mock`。作者可切换无对象、选区、拒绝、超限、隐藏恢复和中途撤销，查看方法、结果码及耗时。临时预览使用独立入口、内存 KV、合成剪贴板及文件。为“命令为何隐藏”显示适用类型、缺少的 grant 和当前运行角色，处理大段输入时只显示字节数。复用 SDK transport 和协议夹具，在同一输入下对照真实 Rust 结果。[现有 transport](../packages/plugin-sdk/src/client.ts#L17) · [仓库外模板验收](../scripts/check-plugin-starter.sh)
4. **回到真实宿主。** 以临时插件目录和数据目录导入构建产物，走正常审查、安装、启动和撤销。普通 Reload 继续加载获批摘要。SDK 的检查、预览和测试命令使用同一套夹具。作者可将其直接放入 CI。VS Code 也用专用开发宿主运行集成测试，可参考其对单元测试与真实宿主测试的分工。[VS Code 扩展测试](https://code.visualstudio.com/api/working-with-extensions/testing-extension)

### 外链与文件读取

下表给出工单和文件两个试点的建议授权策略。`url.open` 与 `resources.read` 分开声明和批准，均禁止自动解析器调用。能力可用性取决于宿主实现、当前运行角色及本次安装授权。能力查询只帮助作者决定如何展示操作，broker 在产生副作用或交付数据前仍复验资格。

| 契约 | 打开站点链接 · 建议必做 | 读取选中文本文件 · 有条件交付 |
| --- | --- | --- |
| 作者调用 | `url.open`，接收绝对 URL。定稿允许 HTTP 和 HTTPS，建议首批工单插件声明确切 HTTPS origin。两种授权范围的取舍见下文 | `resources.readText`，只接收此次调用获得的句柄。除 `context.read` 对应类型外，另需选中文件正文读取权限 |
| 用户动作 | 用户执行命令后，Host 显示包名及可核对的完整目标，用户确认后交给默认浏览器。取消留在原处。页面加载、恢复和选区探测均不能自动打开 | 首版限用户在 Finder / Explorer 中明确选中的一个普通文件，再显式执行读取命令。授予读权限时说明该包可见正文。输入框里的路径、目录、工作区推断、脚本产出的数据仅作显示 |
| 宿主落点 | Rust 解析并规范化 URL，按 scheme、host、有效端口匹配 origin，拒绝内嵌凭据。使用现有平台打开接口。桥只返回交接结果，页面网络策略保持 `connect-src 'none'`。[平台接口](../crates/anywhere-platform/src/traits.rs#L279) · [页面策略](../apps/gpui/src/web_tool.rs#L365) | 现有启动投影保存的是句柄到路径的映射，后续快照仅取投影值，独立窗口还会复制映射。先为**明确的启动选区**建立宿主管理的文件租约，绑定包、调用和文档代次。独立窗口重新签发句柄。打开后复验原生文件身份，无法确认原对象时要求重新选择。[启动投影](../crates/anywhere-core/src/application/web_plugins/projection.rs#L69) · [后续快照](../crates/anywhere-core/src/application/web_plugins/host_state.rs#L44) · [窗口复制](../crates/anywhere-core/src/application/web_plugins/invocation.rs#L109) |
| 额度、错误与取消 | 一次用户确认只交接一次，拒绝未声明目标，区分取消、过期、宿主打开失败。发生失焦后保留已经完成的交接结果，SDK 不重试打开 | 首版建议 UTF-8 且最多 1 MiB，按字节有界读取，沿用现有文本分块。拒绝目录、链接跳转、非法 UTF-8 及超限项。合法 UTF-8 也可能含 NUL，若首版限定文本文件，需明定 NUL 处理并测试边界。隐藏、更新、取消时丢弃迟到结果并释放资源。旧句柄不能读到同路径替换后的文件 |
| 降级 | 不支持时可显示目标，已有剪贴板写权限的包可提供复制链接 | 不支持、权限不足或无法证明身份时，保留粘贴文本入口。任一平台的身份契约未验证，该平台继续只提供元数据 |

### 只重构已有边界

- **协议与 SDK。** 在 `anywhere-protocol` 的封闭方法集上给业务能力补齐所需权限、允许的角色、支持版本和额度。沿用现有生成器输出 TS 类型与机器可读能力表。SDK 给页面、显式命令、只读 resolver 合适的入口，并保留旧入口。授权判定和类型化执行仍在 Rust，GPUI 展示这份说明。新增能力走具体方法。每次新增都应能回答由谁授权、何时取消、用户在哪里看见结果。[桥协议](../crates/anywhere-protocol/src/web_bridge.rs#L14) · [现有生成器](../crates/anywhere-protocol/examples/generate_web_sdk_contract.rs)
- **任务与页面。** 先把后台命令的完成与 Usage 从页面 ready 拆开，复用现有登记身份、预算、取消和 broker。WebView 的 origin、主框架和代次校验保留在页面入口。resolver 继续使用只读投影。文件读取、stat 和预览共用原生对象身份及租约。现阶段无需更换 JS 引擎、重写插件体系或拆出通用插件进程。
- **兼容与数据。** 现有 v1 Host 严格解析 Manifest，校验器还写定了每包最多八项权限。新增权限时明确总额与每项范围预算，并测试边界。采用新声明的包标明新契约和最低 Host，由作者工具在导入前提示。新 Host 继续加载 v1 包，仅用基础能力的新 SDK 仍可产出 v1 包。旧 Host 可能只返回已有的版本或解析错误。协议帧版本与 SDK 包版本单独记录。更新界面在现有权限清单旁展示新增、移除的权限及站点，批准仍绑定精确摘要。[权限上限](../crates/anywhere-plugin/src/web/manifest.rs#L105) · [安装审查界面](../apps/gpui/src/launcher/view/pages/web_packages.rs#L303)
- **后续原生列表。** 先试片段库的命令内列表。用户进入后才将查询交给对应插件，返回有上限的条目、包内对象 ID 与声明过的动作。Rust 负责查询代次、取消、发布和选择时复验，GPUI 负责键盘、焦点、对象菜单。首页动态搜索待两个实际插件证明需求和延迟预算后再评估。内置异步候选源已存在，其静态来源 ID 和目录寿命需经适配。[候选源契约](../crates/anywhere-core/src/core/search/candidate_source.rs#L168)

## Phase 1 定稿的实施补充

定稿中的 Mock、Node CLI、混合模板、命令多语言和图标、SDK 随 Release 分发、开发 Skill 和四种安装入口都纳入本阶段。解包开发模式、自定义宿主动作菜单和第三方动态搜索留待后续。以下约定解决这些功能相互影响时的具体行为。

### 开工前锁定的契约

| 问题 | 建议写进接口的约定与验收 |
| --- | --- |
| **新模板能在哪些 Host 导入** | v1 Manifest 拒绝未知字段。`serde(default)` 能让新 Host 读取旧包，旧 Host 仍会拒绝带 `localizedTitles`、新权限或 `$schema` 的新包。建议将 `$schema` 限于作者源文件，由打包器在生成归档 Manifest 时剔除，审查与摘要统一针对归档字节。其余新增声明使用明确的目标契约。Manifest schema 由现有生成器单源发布到 SDK，桥与 TS 由对应生成器投影。固定 `maxBytes`、`modifiedMs`、`closeWindow` 在线上的拼写，用同一份 JSON 走通 TS → Rust → TS。复核时用现有 Rust 检查器读取 Text Pad，基础包通过，分别单加根字段 `$schema`、命令字段 `localizedTitles` 均报未知字段。新模板需另验目标为 v1 的实际产物。[严格 Manifest](../crates/anywhere-protocol/src/web_plugin.rs#L9) · [Manifest 生成器](../crates/anywhere-protocol/examples/generate_web_plugin_schema.rs) · [桥与 SDK 生成器](../crates/anywhere-protocol/examples/generate_web_sdk_contract.rs) |
| **哪个角色可执行什么** | Web 页可申请返回、关闭当前窗口或预览，并可按权限读文件、打开链接。显式后台 Action 可按权限读文件、打开链接，通过返回值控制本次执行反馈。自动 resolver 仅识别获准输入。建议只向有界面角色暴露窗口与预览方法，运行时仍拒绝角色不符的调用。同一包同时拥有 UI 和 resolver 时逐项测试，安装 grant 不能替代角色检查。[只读分派](../crates/anywhere-core/src/application/web_plugins/background_provider.rs#L182) |
| **文件句柄和预览谁持有** | `stat`、`readText`、`preview.open` 共用来源、原生对象身份和当前代次检查。动态快照可以仅显示名称，其投影不能直接当可读句柄。恢复或独立开窗后，如需继续访问，由宿主重签并更新 SDK 可见的句柄。预览需要单独的 `preview.open` grant，展示不隐含正文读取权。现有内部 `PreviewResource` 含绝对路径，使用 Rust → GPUI 的宿主内部消息传递。JS 只收到成功或类型化错误。对必须按路径打开的外部查看器，使用可验证身份的交接或有额度的宿主只读快照。验证旧句柄、同路径对象替换、QuickLook 未安装及关闭后的租约释放。[内部预览对象](../crates/anywhere-core/src/features/preview.rs#L11) · [Windows 预览](../apps/gpui/src/platform/windows/preview.rs) |
| **窗口命令何时算完成** | 领域层产生一次性、带文档与请求身份的宿主动作。GPUI 在执行前复验当前窗口、document generation 和 request ID。主窗口 back 返回上页，close 清理后隐藏。独立窗口两者都只结束该工具窗。SDK 成功表示宿主已接受，本次销毁造成的断开不得触发重复执行。预览需等平台受理后再返回成功。测试把 A 的回复延迟到 B 已打开，B 的窗口、焦点和预览均不受影响。[现有回执判定](../apps/gpui/src/launcher/view/mod.rs#L2807) · [回复交付](../apps/gpui/src/launcher/view/controllers/web.rs#L331) |
| **外链授权覆盖哪些目的地** | 按协议、主机、端口授权准确的 HTTP(S) origin。通用链接工具可单独申请任意 HTTP(S) 目的地，审查卡标明授权范围。两种授权每次都由宿主显示完整目的地并确认。Web、显式后台、Mock 共享未声明站点、取消和交接后失焦的用例。浏览器打开与插件读取网络响应是独立能力。[现有平台打开契约](../crates/anywhere-platform/src/macos/shell.rs#L37) · [实施契约](web-plugin-phase-1.md#能力和运行角色) |
| **后台返回值和诊断如何兼容** | 旧命令返回的任意 JSON 曾被正常执行入口忽略。建议仅对新契约命令解释 `message` 和 `closeWindow`，旧包即使恰巧返回同名字段也保持既有行为。消息限定字节、纯文本显示。`closeWindow=false` 在原搜索展示反馈并保留选择，默认关闭时若要求展示消息，应在关闭前交给宿主提示层，或由插件选择保留原搜索页。Usage 在成功后只记一次。生产诊断给出包、入口和错误类别。开发配置可显示经脱敏、按 UTF-8 字节截断的堆栈，异常正文可能包含选区或文件内容。取消、资源耗尽和普通脚本异常分别用合成任务验证。[返回值处理](../crates/anywhere-core/src/application/web_plugins/background_action.rs#L93) · [异常出口](../crates/anywhere-core/src/application/web_plugins/background_runtime.rs#L124) |

### 容易漏掉的交付验收

- **作者工具走到最终产物。** `validate` 区分尚未 build 的源项目和已经生成的归档。Node 打包器与 Rust 导入器共用合法和错误包夹具，覆盖路径大小写与 Unicode 冲突、压缩比、字节预算和 LF/CRLF。旧 Python 包仍可导入，验证新旧打包器对相同资源计算的宿主内容摘要，以及固定工具链下 Node 包跨平台重建的一致性。Mock 用同一组结果码、额度及撤销用例对照真实 broker。正式页不安装 Mock，QuickJS 模板不依赖 DOM。[归档校验](../crates/anywhere-plugin/src/web/archive.rs#L95) · [路径规范](../crates/anywhere-plugin/src/web/path.rs)
- **从公开资产验收分发。** 现有 workflow 上传 Actions artifact，发布流程单独列出 Release 文件。两处都加 SDK 与校验和，标注 tag、SDK 版本和最低 Host。用发布 URL 获取包后在仓库外运行 `init`，核实 vendor 包包含 schema、Mock 导出、模板及 Skill。Skill 从单一源打包，对各 Agent 分别验证发现路径，并提供可直接阅读的项目指南。vendor 提供 SDK 本身，模板中的 esbuild、TypeScript 首次安装仍需要可用的软件源或缓存。[工作流](../.github/workflows/macos.yml) · [Release 资产](releasing.md) · [vendor 复制清单](../packages/plugin-sdk/tools/create.mjs#L35)
- **跨平台快捷键和命令元数据。** JSON Workspace 的键帽、帮助文字和处理器均有 macOS 硬编码，但已有 Ctrl+Tab 与输入法组词保护。共享键位规格保留这些行为，区分额外修饰键、AltGr 和按住重复，保留原生粘贴与宿主 Esc / 动作菜单。Mock 切平台后标签与监听同时更新。用 WKWebView、WebView2 分别验收格式化、复制、粘贴和中文组词。命令标题按当前语言回退到 `title`，未知图标回退到公共图标。验证切换语言后中文、英文、拼音搜索仍命中同一 Action ID，快捷键和 Usage 延续。[示例按键](../plugins/examples/json-workspace/src/main.jsx#L266) · [示例提示](../plugins/examples/json-workspace/src/i18n.mjs#L13) · [搜索字段](../crates/anywhere-core/src/core/action/descriptor.rs#L99)
- **四个安装入口共用一次审查。** 选择器、拖放、选中文件动作及系统打开都提交到同一 inspect / approve 流程。建议一次展示一个包，审查中收到另一个包时保留当前卡并明确提示。签发 token 后只批准该次捕获的字节。审查卡对比命令、后台解析器、权限新增与已有权限的范围变化。只有可空输入打开的 Web 命令才显示“立即打开”。后台专用包和依赖选区的命令提供返回列表入口。macOS 验证 Finder 冷启动及已启动时的文件事件。Windows 便携 ZIP 需说明用户如何关联或“打开方式”选择 EXE。已有实例的第二次启动目前仅恢复窗口，还需把待审查文件交给主实例。双平台用含空格及中文的本地文件分别验证，也覆盖取消、重复打开和已安装 B 再导入 B 后仍能回滚到 A。[单份待审查记录](../crates/anywhere-core/src/application/web_plugins/packages.rs#L115) · [Windows 单实例入口](../apps/gpui/src/platform/windows.rs#L60) · [便携包内容](../scripts/package-windows.py)

## 实施顺序与验收决定

先完成旧包兼容、回滚和作者工具，再逐项接入宿主能力。原生文件身份验证通过后才接入预览。安装入口和发行资产需分别验证真实宿主。具体依赖、步骤和完成条件见[任务清单](web-plugin-phase-1.md#如何接手)。

下一阶段可用片段库和词典试验命令内列表，再评估按包撤销、凭据与定域网络、长任务和公开分发。列表先测快速连续输入时的首批延迟、旧结果清理和目标正确性。

**升级回归**需用旧 SDK / 新 Host、新 SDK 的基础包 / 旧 Host、新能力包 / 旧 Host 三组实物。覆盖同 ID 更名、删命令、增加权限、回滚及卸载重装。原业务 ID 的快捷键和 Usage 可延续，缺失命令的旧绑定不能执行。同一安装可继承 KV，卸载重装使用新的存储命名空间。回滚恢复获批包，KV 格式回退由插件处理。对旧候选、旧句柄和迟到结果分别验证执行与发布时的资格。[命令绑定策略](../crates/anywhere-core/src/core/action/descriptor.rs#L201) · [安装与卸载](../crates/anywhere-core/src/application/web_plugins/packages.rs#L193)

**桌面验收**使用同一组外部包，在 WKWebView、WebView2 分别记录安装、撤销、焦点返回、中文 IME、隐藏恢复和独立窗口结果。Windows 已有内置 JSON 工作流的单机记录，第三方安装、更新和撤销矩阵仍待执行。上述新增能力与桌面验收均为 Pending。[macOS 验收](test-acceptance-macos.md) · [Windows 插件验收](test-acceptance-windows.md#webview2-与插件) · [Windows 证据范围](windows-progress.md)
