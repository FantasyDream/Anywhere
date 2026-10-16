# 第一方原生模块与第三方 Web 插件：现状和改造建议

核对日期：2026-10-14。代码基线 `dev-0.9@e4dbedc`。本文记录用户提出的新方向及改造建议，**不是已实现功能或实机验收**；本轮只梳理代码和官方接口资料，不修改运行代码。原 V0.9 Form / List / Markdown DSL 切片应暂停，待按本方案调整开发范围。

后续方向已调整：第一方彻底取消热加载 / 通用 runtime 模型，改为 [内置功能模块](builtin-feature-modules.md)；第三方 WebView 放到最后阶段。本页的第一方 Runtime / NativeProcessRuntime 和 V0.9 Web 实施顺序已被新方案替代，保留为调研过程。

## 1. 建议采用的边界

- 第一方：Rust 原生业务实现，复杂界面使用 Flutter，OS 能力留在平台适配器；通过稳定贡献接口与编译期自动注册接入。
- 第三方：本地 Web 包，HTML / CSS / JS 实现页面及工具逻辑，使用版本化 JavaScript SDK 调用受限 Host 能力。
- 两类共用 ExtensionCatalog / Lifecycle / Context / Action / Candidate / Ranking / Usage / Storage / 权限与资格校验；实现、信任来源与装载策略分别明确。
- 主搜索只读取已注册入口元数据；用户进入 Web 插件时再创建页面。第一版外部 Web 插件只消费 Context 和提供动作入口，不接入常驻后台 ContextProvider / CandidateSource。

“原生实现”不要求每个模块写 AppKit / Windows 控件。业务 Rust、展示 Flutter，保持当前跨平台分工。

## 2. 当前所有实现

### 2.1 九个第一方模块

| 模块 | 当前能力 | 接入方式 |
|---|---|---|
| JSON | JSON Context，格式化、压缩、复制格式化结果 | Provider + ActionBundle，输出使用现有文本结果界面 |
| URL | URL Context，打开浏览器、复制 URL | Provider + ActionBundle |
| Git | 仓库识别、Status / Diff / Staged Diff / Log / Branches，复制分支、提交、Remote URL、仓库路径 | Provider + ActionBundle，HostServices 读 Git / 写剪贴板 |
| Rust | Cargo 项目识别、Check / Test / Run 等，按项目展开目标 | Provider + Bundle + ContextActionFactory |
| Maven | 项目识别、Test / Package | Provider + Bundle，任务进入 Process Session |
| Gradle | 项目识别、Build / Test | Provider + Bundle，任务进入 Process Session |
| Node | 项目识别、按 package.json 展开 scripts | Provider + Bundle + ContextActionFactory |
| Spring Boot | Spring Boot 项目识别、运行等动作 | Developer 的参数化模块，任务进入 Process Session |
| 收藏目录 | 保存、查找、打开、移除收藏 | runtime Actions + CandidateSource + 有界共享状态 + 隔离 SQLite Storage |

清单来自 [features/module.rs](../../crates/anywhere-core/src/features/module.rs)。Developer 五个模块共享同一个 [实现](../../crates/anywhere-core/src/features/developer/mod.rs)，并非五套执行器。

`extensions/<id>/manifest.toml` 仅保存编译时嵌入的身份 / 版本 / 能力；实现仍在 `anywhere-core/src/extensions`，与 Host 一起编译。目录名叫 extensions 不等于可安装动态代码。

### 2.2 基础功能仍在 features

应用搜索、Current App 菜单、文件 / 目录 Context 与动作、文件搜索、Text 种子等仍在 `features`，没有全部迁成可开关第一方插件。它们是基础服务与系统适配入口，迁移时逐项判断，不为形式统一而全部可卸载。进程管理和通用设置也是 Host 服务。

### 2.3 第一方装配与 Reload

当前链路：

```text
modules() 的手工列表
→ FirstPartyExtension.manifest / contribution / runtime_with_state
→ FeatureContribution：Provider、Bundle、动态 Action Factory
→ ExtensionRuntime：Actions、Sources、有界共享 State
→ ExtensionManager prepare
→ 原子替换 Registry / Providers / Sources
```

已有能力：运行期启用 / 禁用，重建模块 runtime，重新读取 Storage，推进 catalog / execution epoch，失效旧候选与保留会话；退休的异步 Sources 执行 reset。已启动任务由 ExecutionSessionManager 独立持有。

证据：[贡献模型](../../crates/anywhere-core/src/features/contribution.rs)、[装配](../../crates/anywhere-core/src/application/composition.rs)、[Manager](../../crates/anywhere-core/src/application/settings_service.rs)、[原子替换](../../crates/anywhere-core/src/application/launcher_service.rs)。

Reload 不加载新的 Rust 模块、替换 dylib 或 Dart AOT 页面，也不是文件监视器。它重新装配已经编译进 App 的代码。

### 2.4 外部 TOML v1 / v2 / v3

| 版本 | 当前行为 | 加载 / 更新 |
|---|---|---|
| v1 | open_url / open_file / copy；兼容 execute_builtin 委托 | 扫描 TOML 并重新注册；沿旧安装授权语义 |
| v2 | 复制、打开 URL / 文件；显式能力批准 | 批准绑定安装路径和完整 Manifest；修改后重批 |
| v3 | 增加 ui.render / render_detail，Host 渲染 Detail / KeyValue | 同一批准机制；UI 定义改变也重批 |

当前只支持 `*.toml` 与 `<plugin>/manifest.toml`，没有执行第三方 JS / Rust / Shell。动作逻辑被限定在 `DeclarativeAction` 枚举及 context.value 模板内。Source / Provider / 原生 Storage 尚未开放给第三方。

证据：[Protocol](../../crates/anywhere-protocol/src/lib.rs)、[解析和发现](../../crates/anywhere-plugin/src/lib.rs)、[执行器](../../crates/anywhere-core/src/core/action/execute.rs)。

v3 的链路是 Manifest → Rust schema / 插值 / grant → typed HostView → FRB DTO → Flutter PluginDetailPage。虽然有 [HostViewNavigator](../../apps/flutter/lib/launcher/host_view_navigator.dart)，[Controller](../../apps/flutter/lib/launcher/launcher_controller.dart) 和 [Screen](../../apps/flutter/lib/launcher/launcher_screen.dart) 仍直接认识 pluginDetail。

当前 [pubspec](../../apps/flutter/pubspec.yaml) 无 WebView 依赖，Runner 无 WKWebView 实现。新的 Web 插件需要新增装载器、页面容器、SDK、资源访问与消息桥。

## 3. 需要解决的耦合

1. `modules()` 手工列模块；新增模块还涉及 Rust mod / 构造器接线。
2. `approved(id)` 用具体模块 ID 分支批准能力，Host 策略与业务清单耦合。
3. metadata 在每个模块里 include_str + OnceLock；贡献、持久状态和 runtime 的装配分两路处理。
4. `features::recognized_contexts` 直接调用 URL / JSON 识别器；`input_policy` 和文件相关识别使用它。不能只改注册表而保留这条路径。应让自动填入识别也读取当前启用的轻量识别贡献，并补“禁用模块后不靠该模块触发自动内容页”的回归。
5. 模块依赖完整 anywhere-core 中的业务类型、features 组定义等；包级边界没有完全分离。ActionHandler 接收完整 Platform，HostServices 的收窄依赖受信任实现遵守约定，不是对原生代码的安全沙箱。
6. UI enum、DTO、Controller 和 Screen 共同扩展每一种 DSL 视图。新增一个外部 UI 类型就增加 Host 协议 / 渲染责任。
7. 全体模块重建缺少通用 RuntimeHandle 的显式启动 / 取消 / 销毁协议；目前 Source reset 与 State refresh 各自承担部分职责。未据此认定已有资源泄漏，但未来 WebView / 订阅不能只依赖 Drop 或插件自觉。

## 4. 第一方改造

### 4.1 统一贡献与生命周期

将现有两路贡献整理为同一个逻辑接口：

```text
NativeExtensionFactory
  descriptor：id、版本、入口、所需能力、信任来源
  create(ExtensionHost) → RuntimeHandle
    contributions：轻量识别器、Providers、Actions、Factories、Sources
    state：有界数据 / Storage
    cancel / dispose：运行资源清理
```

这是内部 Rust API，复用已有 Provider / Action / Source Trait，不重新设计整套 Context。ExtensionHost 注入授予后的系统服务、Storage、调度 / 取消与日志，模块无需导入 Application / persistence 实现或自行选择 namespace。

权限从通用 HostTrustPolicy 决定，依据装载器确认的来源。第三方 Manifest 写 first_party 或换 ID 不能取得内置模块待遇；可信编译目录中的请求也由 Host 校验。能力声明仍保留，不能把模块自己写的请求当作批准。

先保持当前 crate 组织，完成依赖反转；需要独立 crate 时再把装配移到 application 层、导出窄的 extension-api。实现 crate 依赖公共契约，Host 在装配层依赖实现，避免 core ↔ extension-api / 模块互相引用。初期不为每个很小的模块建单独 crate。

### 4.2 自动注册

建议第一步使用编译期生成目录：模块在自己的目录提供 descriptor / factory；生成器校验 ID、排序、冲突和入口，生成 Rust 模块 / factory 列表与 Flutter 页面注册表。新增模块不修改 Launcher Controller、Screen 或手工业务清单。构建必须检查生成结果是否滞后。

生成器发现的是已纳入源码 / 构建的模块，新增 crate 仍需要 Cargo dependency / feature 接线；自动注册不能让尚未链接的代码出现。

另一种可用实现是 linkme distributed slice：由链接器收集静态注册项，官方列出 macOS / Windows 支持。它依然是编译 / 链接期注册，最终目录应按 ID 排序并检查冲突，还要验证独立 crate 未被链接裁掉。[linkme 官方文档](https://docs.rs/linkme/latest/linkme/)

### 4.3 原生页面注册

第一方页面由模块提供 Flutter factory，Host 维护 ViewRegistry。Rust 请求一个归属已验证的 view ID 和有界初始数据，Launcher 渲染通用 ToolPageShell，再将内容交给注册页面。

应用业务数据契约由模块拥有；Host 不再解释外部 List / Form / Markdown UI 描述。原 PluginDetailPage 的布局 / 主题可以作为内部组件复用，v3 UI DSL 停止扩展。

### 4.4 热加载边界

| 操作 | 建议 |
|---|---|
| 编译期自动发现 / 注册 | 实现 |
| 运行期启停、重建已内置模块 | 保留并统一生命周期 |
| 新增或替换 Rust / Flutter 原生代码，不重启 App | 首期不承诺 |
| 运行期安装 / 更新 Web 包 | Web Runtime 实现 |

Rust 原生 ABI 没有稳定性保证，现有 Rust Trait / Arc / Any 接口不能直接作为可更新 dylib 的跨版本协议。[Rust Reference](https://doc.rust-lang.org/reference/items/external-blocks.html)

若未来确实需要第一方独立二进制热更新，优先评估 NativeProcessRuntime：按需启动 Rust sidecar，通过版本化 IPC 调用，替换进程而不卸载持有引用的 Rust 动态库。这里预留 runtime / 传输边界，首期不实现额外进程。

## 5. 第三方 Web 插件

### 5.1 包与入口

```text
plugin/
├── manifest.json    身份、版本、SDK 版本、页面入口、命令元数据、权限
└── dist/
    ├── index.html
    └── assets/
```

React / Vue / Svelte / 普通 JavaScript 都可以构建成资源包。Manifest 只保存安装、匹配与导航元数据；动作逻辑、表单、列表及界面写在 JavaScript 中。首期命令的标题 / keywords / accepts 在加载 Manifest 时进入正常 Registry / Ranking / 快捷键，不需要启动 WebView 来回答每次主窗口查询。

加载本地不可变包快照，校验路径不能逃逸包根、资源与总包大小有界。批准身份应覆盖实际运行的 HTML / JS / CSS 等包内容；现在的“安装路径 + Manifest”身份不足以批准 Web 代码。内容变化后失效旧 grant / runtime / 回调并重批，切换失败保留可诊断的旧已批准快照。

生产首期加载本地 dist。开发服务器可作为独立 dev 模式，以 localhost、临时批准、可见开发标记和导航限制接入；不把任意远程网页自动当成有本地权限的插件。

### 5.2 JavaScript SDK

建议首期提供 Promise API：

- 获取本次命令获准访问的 Context / 输入；不发送整个 SessionFacts、其他应用数据或剪贴板历史。
- 获准后复制文本、打开 HTTP(S) URL、打开获准文件。
- 小额 namespace Storage（如果纳入首期能力），权限与配额由 Rust Host 执行。
- 订阅主题 / 可见性，返回 Launcher；注册与取消 SDK 回调。

文件读取、项目任务、网络或后台能力按实际工具需求扩展。原生能力调用都到 Rust broker：校验真实 runtime 绑定、plugin / command、包 generation、当前 view / invocation 与 grant，按需复验 Launcher Session / Revision 和文件授权。页面提交的 pluginId 或 sessionId 不作为身份证明。

JSON request / response 使用请求 ID、超时、取消、消息大小和 in-flight 限额；二进制 / 高输出流预留独立带背压通道，Terminal 阶段再实现。禁止把输入拼进可执行 JS 或向页面暴露整个 Platform 通用代理。

WebView 本身不能替 Host 完成本地能力隔离。消息需验证 document / frame 来源和参数，顶层导航、iframe、弹窗与外链有明确策略；Microsoft 官方也要求验证 origin、验证消息、使用明确的接口。[WebView2 安全指南](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security)

JS fetch / 图片 / WebSocket 等不经过 Host RPC，不能只拒绝 SDK network 方法便宣称禁网。若声明网络域权限，必须同时落实 Web 内容的 CSP、资源 / 导航策略并做真实适配器验证。生产页面进入远程内容前撤销原有本地能力。

### 5.3 平台适配

Host 暴露同一个 WebPluginSurface 接口：创建、资源装载、消息、焦点、尺寸、可见性、销毁 / 失败通知。

- macOS：WKWebView，公开 WebKit 消息 / 回复与 scheme handler 能力。[Apple 消息接口](https://developer.apple.com/documentation/webkit/wkscriptmessagehandlerwithreply)、[资源入口](https://developer.apple.com/documentation/webkit/wkwebviewconfiguration/seturlschemehandler(_:forurlscheme:))
- Windows：WebView2，使用相同 JS SDK / Host 消息协议；实现仍需 Windows Host 和实机验证。

官方 webview_flutter 当前列出 Android / iOS / macOS，未列 Windows；不能把引入该依赖等同于完成跨平台 Web 插件。[官方包](https://pub.dev/packages/webview_flutter)

当前 App 使用脚本手工编译 Swift Host / 组装 Flutter frameworks，未使用标准生成插件注册流程；引入 Flutter WebView 包要同时接入原生插件链接 / 注册和打包。建议先完成 Host Surface 的最小验证，依据其能力选择系统适配器或现成包装包，不直接换整套打包或引入自带 Chromium 的框架。Flutter 提供标准平台通信方式。[Flutter 平台通道](https://docs.flutter.dev/platform-integration/platform-channels)

### 5.4 交互与资源

- 现有 Launcher 保留搜索入口、导航和窗口材质；ToolPageShell 包住 Web 内容，主题值通过 CSS variables / SDK 提供。
- 继承 Esc 返回与原查询恢复。页面输入 / IME / 内部弹层先处理，跨层级返回有明确优先级；保留键在原生 WebView 焦点下也必须验证，不假设 Flutter Focus 能拦截全部按键。裸 Tab 规则延续当前产品约束。
- 安装 / 主搜索阶段只保留 Manifest 和入口；页面按需创建，首期只持有当前插件视图。
- 隐藏时立即撤销交互调用资格，保留页面状态 15 秒；恢复发新 epoch，旧请求 / 回调作废。到期、卸载、权限撤销、包更新时清理 surface / JS handlers / 订阅 / 待处理 RPC。
- suspend 事件是协作通知，不能保证任意 JS 真正停止运行；真实释放依赖 Host dispose。15 秒以内后台耗时要测，不能靠页面 self-pause 承诺。
- 已启动原生任务由现有 ExecutionSessionManager 持有，页面关闭只取消订阅，不结束用户任务。
- 不为每个已安装插件常驻 WebView。网络来源 / Cookie / Storage 隔离采用已验证的方案，避免为每个入口创建独立浏览器环境。WebView2 使用浏览器 / renderer / helper 进程，环境和 user data folder 会影响进程组；性能基线要测主 App 加 helper，不只看 Flutter RSS。[官方进程模型](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)

## 6. 保留、迁移与停止扩展

保留：现有 Context / Provider / Action / Candidate / Ranking / Usage / 快捷键，能力批准与 Storage，原子 runtime 替换、Session / epoch 资格、15 秒恢复、Process Sessions、AppStyle 和基础导航。

调整：模块自动注册、Host 服务注入、轻量识别目录、RuntimeHandle、原生 ViewRegistry，ExtensionManager 通过 Native / Web / Legacy adapter 装配，不继续膨胀成巨大的类型分支。

停止扩展：第三方动作表达式、模板控制流、Form / List / Markdown UI DSL。Web Manifest 和 SDK 消息是必要的元数据 / 通信契约，界面和业务行为由 JS 实现。

迁移：v1 / v2 移入 LegacyDeclarativeAdapter，暂时维持既有插件 / ActionId / 配置，不继续新增能力。v3 尚属开发切片，应先核验是否已有用户安装，再选择迁移为 Web 示例或退役；不直接删除用户目录。内部 Detail 组件可复用，不能据此继续宣传外部 DSL 路线。

## 7. 建议重排 V0.9

1. 原生模块自动注册与完整贡献接口；保证新增测试模块不改 Launcher / 业务清单，禁用后识别 / 动作 / Source 一并移除。
2. ToolPageShell + NativeViewRegistry，迁入 Context Detail 作为第一方页面，验证键盘 / 返回 / 15 秒保留。
3. macOS WebPluginSurface 最小验证：本地资源、主题、焦点、resize、输入 / IME、Esc / 弹层、返回和销毁；打包真实 App 后再选定包装库。
4. Web 包加载 + JS SDK + 权限 broker，用一个真实工具完成 Context → 编辑 / 计算 → 复制或打开 → 错误 → 返回。JSON 工具可作为候选，复用已有 JSON 第一方能力时通过明确 Host 方法调用，不开放 arbitrary execute_builtin。
5. 更新 / 撤销 / 旧回调 / 超限 / 崩溃 / 安装路径与 helper RSS / 帧时间矩阵。Windows 完成适配器前只声明 API 预留，不声明已可用。

Terminal / PTY 仍是后续独立阶段；新 Web 容器可复用，流控 / 二进制通道和长期任务必须另行验证。

建议优先顺序：第一方接口与注册 → 通用页面导航 → Web 容器 → SDK 与能力 → 实际工具与验收。第一方原生代码热更新另作里程碑，不把它混在首期自动注册验收里。
