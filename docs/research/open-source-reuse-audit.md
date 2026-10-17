# 当前功能的开源库复用清单

核验日期：2026-10-17。基线：`codex/dev-0.10`、Build 9 的当前工作区。检查了 Cargo / Flutter 依赖、功能模块、平台适配和 Host；外部能力以本轮访问的项目官方仓库、API 文档为依据。下列接入收益、成本和优先级是结合本项目的工程判断，未安装候选库、未做替换原型或同机性能测试。

## 结论

适合复用的是文件枚举、文字匹配、格式解码、热键、系统 API 绑定等基础部件。Context 来源、Scope、执行资格、稳定列表、15 秒保留等产品规则继续由 Anywhere 持有。没有证据支持用一个现成库无适配替换整个 Launcher 或文件搜索链路。

- **可局部引入：** `encoding_rs` / `chardetng`、`which`；分别补文本编码处理和简化可执行文件查找。
- **值得做原型：** `nucleo-matcher`、`ignore` / `walkdir`、复用现有 SQLite 做查询临时记录、`global-hotkey`。先验证行为等价与资源收益，再决定替换。
- **保留现有选型：** `clipboard-rs`、`rusqlite`、`image`、`flutter_rust_bridge`、Serde / URL / TOML / XML / Unicode 等库。
- **下一阶段直接使用：** PTY 优先评估 `portable-pty`，Terminal 渲染优先评估 `xterm.js`；它们补下一阶段能力，不算当前已接入。

成本“低 / 中 / 高”表示接口与行为适配规模，不是工期承诺；接入库通常减少基础设施代码，不自动降低 RSS 或延迟。

## 一、文件搜索、应用搜索和预览

| 当前功能 / 实现 | 开源候选 | 能替代 / 补充什么 | 成本与建议 |
|---|---|---|---|
| 文件名搜索：Spotlight 快速路径 + Rust 完整遍历 | [ignore](https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html) | 替换 `features/filesystem/scan.rs` 的部分目录枚举、过滤和错误遍历 | **中，原型候选。** 不是完整搜索引擎；权限、临时排除、链接边界、Unicode 匹配、取消和稳定结果仍需适配 |
| 同上，优先简单串行实现 | [walkdir](https://docs.rs/walkdir/latest/walkdir/struct.WalkDir.html) | 标准递归迭代器，减少路径遍历样板 | **中，基线候选。** `max_open` 限制句柄数量，达到限制会把目录条目读入内存，不是硬内存额度；不保证优于当前磁盘队列 |
| 并行枚举备选 | [zlob](https://github.com/dmtrKovalenko/zlob) | 回调式遍历 / glob | **高，暂不优先。** 引入 Zig / FFI 构建及错误 / 队列适配；现有约束下收益需测量；既有调研见 [搜索库补充](file-search-libraries.md) |
| 并行枚举备选 | [jwalk](https://github.com/Byron/jwalk) | Rayon 并行遍历 | **不作为新接入首选。** 官方仓库显示 2026-08-05 已归档；旧调研中的候选建议应结合此状态重新判断 |
| 查询临时记录与外部排序 | [SQLite 临时文件与排序机制](https://www.sqlite.org/tempfiles.html)，复用现有 `rusqlite` | 可能替换 `result_store.rs` 的分块排序 / 多路归并和部分自定义记录格式；按查询创建并销毁 DB | **中，值得单独原型。** 不增加数据库依赖，不建立长期索引；仍需保留已展示前缀冻结、原始路径身份、取消与流式提交。缓存 / temp_store 不是整个进程的硬内存上限 |
| 应用 / Action 查询：分词、Exact / Prefix / WordPrefix / Substring | [nucleo-matcher](https://github.com/helix-editor/nucleo) | 可选模糊评分、匹配位置与 Unicode 字素处理 | **中，体验原型优先。** 只接低层 matcher，不引入整个 picker；保留精确 / 前缀层级、标识符限制、各词命中与 frecency。文件搜索是否允许非连续模糊匹配需另定产品规则 |
| 已安装应用发现 | `walkdir` / [notify](https://github.com/notify-rs/notify) | 前者可简化有界目录扫描，后者可触发目录缓存失效 | **低至中，按需。** `catalog.rs` 已限制深度和数量；识别 `.app`、读取名称 / Bundle ID、重复安装优先级仍是应用发现逻辑。notify 是增量刷新能力，不是完整应用目录 SDK |
| 元数据 / 有界文本预览，当前只接受 UTF-8 文本 | [encoding_rs](https://github.com/hsivonen/encoding_rs) + [chardetng](https://github.com/hsivonen/chardetng) | 在现有 64 KiB 读取边界内支持 BOM / UTF-16、GBK 等编码解码与推测 | **低至中，建议优先补。** BOM 和有效 UTF-8 优先；chardetng 不负责识别 UTF-16，应由 BOM 层处理；检测可能误判，二进制不能仅凭“能解码”就显示为文本 |
| 文件类型过滤 / 图标分类 | [infer](https://github.com/bojand/infer) | 根据少量文件头识别部分真实 MIME 类型，补无扩展名或扩展名错误的预览 | **低，作为预览补充。** 名称搜索的扩展名过滤继续使用共享配置；不对全部候选读取正文，也不期待 infer 返回 UI 图标 |
| 图片解码 / 缩略图 | 已用 [image](https://github.com/image-rs/image) | 当前已复用解码器 | **保留。** 16 MiB 原始额度、尺寸 / 分配限制、512 像素缩略图和串行准入是产品资源策略，不能随着换库移除 |
| 文件 / 应用图标 | AppStyle / AppIcons、共享类型配置、系统应用图标 | 当前已使用 Flutter 图标和系统图标能力 | **保留。** 通用图标包不能代替应用真实图标、类型语义和有界加载；不为图标缓存另建完整框架 |

### 搜索替换必须保留的约束

`ignore::WalkBuilder` 默认启用 hidden、ignore、gitignore、父级规则等过滤。[官方选项说明](https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html#method.standard_filters)

Anywhere 要搜索设置允许的隐藏文件和未索引文件，不能直接套用默认配置。原型应关闭库的隐式标准过滤，再显式应用用户设置、系统临时目录和授权边界；错误作为 Partial 汇总，不能用 `filter_map(Result::ok)` 静默丢弃。`walkdir` 也必须保留平台隐藏属性检查，点号名称并不覆盖所有系统隐藏语义。

当前磁盘队列、查询临时数据与可见前缀稳定是低内存 / 使用体验设计；换成内存工作队列或全量 `collect()` 可能适得其反。不能将文件系统遍历顺序等同于稳定的用户结果顺序。

## 二、剪贴板、Context 和应用菜单

| 当前功能 / 实现 | 开源候选或现有依赖 | 接入判断 |
|---|---|---|
| 文本、图片、文件、HTML / RTF、自定义格式读写及监听 | 已用 [clipboard-rs](https://github.com/ChurchTao/clipboard-rs) 0.3.5 | **保留。** 已覆盖本项目所需的低层格式接口；macOS 权限 / 代数 / 大小预检仍需薄适配 |
| 剪贴板历史、去重、配额、暂停、搜索、恢复 | 已用 SQLite；[Maccy](https://github.com/p0deje/Maccy) 可借鉴 | **保留 HistoryService / Repository。** Maccy 是完整 macOS 应用，MIT，并非可直接嵌入 Flutter / Rust 的历史库；整体接入会再引入 UI / 存储模型 |
| 剪贴板替代后端 | [arboard](https://github.com/1Password/arboard) | **不建议改换。** 官方主要描述文本 / 图片读写；本轮未证实它能等价替代当前文件、RTF、多表示与监听契约，没有证据支持切换更省事 |
| macOS 当前应用、选中文本、菜单访问的底层 FFI | [objc2-app-kit](https://docs.rs/objc2-app-kit/latest/objc2_app_kit/) / [objc2-application-services](https://docs.rs/objc2-application-services/latest/objc2_application_services/) | **中至高，渐进候选。** 当前已依赖 objc2，可减少部分手写类型 / API 绑定；仍是 macOS API，菜单扫描、预算、身份与歧义处理不由绑定库提供 |
| macOS AX 包装另一选择 | [AXSwift](https://github.com/tmandry/AXSwift) | **低优先级。** 只有基础包装且不管理状态；GitHub API 本轮返回最后推送为 2023-07-25。现有采集主要在 Rust / Objective-C，改用它会增加 Swift 业务适配，不优于小步维护现有边界 |
| Windows 当前应用 / 文本 / 菜单采集 | [uiautomation-rs](https://github.com/leexgone/uiautomation-rs) | **Windows 接入时推荐评估。** 减少 COM / UIA 样板；不意味着 Windows 与 AX 菜单暴露行为相同，不承诺覆盖每个应用 |
| Finder 当前目录 / 选中项 | 当前冻结应用身份 + Apple Events / 系统接口 | **保留平台适配。** 本轮没有找到可等价替换当前身份复验、多选、权限、窗口变化与错误语义的跨平台现成库 |
| Safari / Chrome / Edge 当前页 URL | 当前 AppleScript / Apple Events | **保留。** 若以后扩浏览器能力，可用 [Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging) 做 Companion；这是额外扩展与协议，不是更简单的即插即用库，也不建议为取当前 URL 强制开启调试端口 |
| IDEA 项目目录 | 当前只接受窗口明确路径与 `.idea` 的轻量适配 | **保留保守路径。** 真正扩大可靠覆盖应通过 [IntelliJ Project API](https://plugins.jetbrains.com/docs/intellij/project.html) 的 Companion，避免猜私有配置 |
| VS Code / Cursor / Agent 当前项目 | 当前未实现可靠来源 | [VS Code workspace API](https://code.visualstudio.com/api/references/vscode-api#workspace) 可作为未来 Companion 的输入；不把扩展开发描述为当前已有功能或通用 Agent 目录库 |
| Current App 菜单 ActionId、重扫、缓存与执行 | 现有 Registry / discovery ticket / 资格复验 | **保留。** AX / UIA 只能提供系统对象；无法替代“目标不唯一不执行”和历史隔离契约。`muda` 创建本应用菜单，不能读取别的应用菜单 |

## 三、Git、开发动作和运行任务

| 当前功能 / 实现 | 候选 | 成本与建议 |
|---|---|---|---|
| Git 仓库轻量识别、worktree、分支 / remote / hash；Status / Diff / Log 固定 argv | [gix](https://github.com/GitoxideLabs/gitoxide) / [git2](https://github.com/rust-lang/git2-rs) | **中至高，暂不整体替换。** gix 可用于结构化只读元数据；git2 是 libgit2 绑定。需要逐项核对 worktree、unborn / detached HEAD、insteadOf、配置与短哈希等行为；当前通过用户 Git 获取报告具有一致性。先测多次进程启动是否真是瓶颈 |
| Maven / Gradle / Node / Rust / Spring Boot 轻量识别 | 已用 serde_json、toml、roxmltree；可评估 [cargo_toml](https://docs.rs/cargo_toml/latest/cargo_toml/) | **保持轻量。** cargo_toml 可提供更明确的 Cargo 模型，不自动解决全部 workspace / target 推导；Node scripts 与 Maven XML 没有必要引入完整项目解析框架 |
| Rust 完整 workspace / target 元数据 | [cargo_metadata](https://github.com/oli-obk/cargo_metadata) | **适合显式 Action，不放默认 Provider。** 它结构化访问 `cargo metadata`，通常仍会运行 Cargo；耗时、工具依赖与元数据输出需要预算，不能用它破坏“Provider 不执行项目命令”的契约 |
| 工具 PATH、GUI 环境、SDKMAN / NVM / Volta | [which](https://github.com/harryfei/which-rs) | **低至中，可局部采用。** 替换标准可执行查找；保留自定义工具目录、NVM 有界别名解析、不执行 shell 配置、Windows 不盲目启动批处理等规则 |
| 普通后台任务与停止进程树 | std::process / 现有 OwnedProcess；已有 Tokio | **保留 SessionManager。** 进程库不包含四任务准入、32 条留存、128 KiB 输出、状态 / 退出码、旧资格撤销；Windows 当前明确拒绝无进程树后端的启动，不能换普通 Child 就声称支持 |
| PTY 交互终端 | [portable-pty](https://docs.rs/portable-pty/latest/portable_pty/) | **V0.11 优先候选。** 提供跨平台 PTY 接口、输入输出、resize 等；应用的 attach / detach、停止整个任务树、流控和退出策略仍需实现 |
| 终端渲染 | [xterm.js](https://github.com/xtermjs/xterm.js) | **V0.11 优先候选。** 与现有规划的专用 WebView 一致；终端渲染器不负责启动进程。不要再自写 ANSI 终端渲染 |

## 四、窗口、快捷键、设置与模块体系

| 当前功能 / 实现 | 候选 | 成本与建议 |
|---|---|---|---|
| 全局唤醒 / Action 热键的系统注册 | [global-hotkey](https://github.com/tauri-apps/global-hotkey) | **中，跨平台后端原型优先。** 可替换 Carbon 注册部分；macOS 要在主线程接已有事件循环，Windows 也有循环线程要求。冲突回滚、按住去重、Context 绑定、导入导出和写后发布继续由产品层管理 |
| Flutter 侧热键 / 快捷键录制 | [hotkey_manager](https://github.com/leanflutter/hotkey_manager) | **备选。** 可简化 Flutter 的录制 / 注册桥接，但不要同时引入 global-hotkey 和另一套全局注册器；业务资格仍归 Rust，查询内按键仍用 Flutter Actions / Shortcuts |
| macOS 专属快捷键组件 | [KeyboardShortcuts](https://github.com/sindresorhus/KeyboardShortcuts) | **不作为本项目主选。** 有 Recorder / 存储 / 冲突提示，但会增加 macOS 专属组件与自己的 UserDefaults 持久化，不符合当前统一 Flutter 配置界面的目标 |
| 普通窗口大小 / 位置 / 最小化 / 关闭 | [window_manager](https://github.com/leanflutter/window_manager) | **中至高，平台重构时试普通窗口。** 主分支已改用 nativeapi，旧 0.5 示例与新接口不同，必须按固定发布版本评估；不直接接管 Launcher 的焦点、唤醒与玻璃策略 |
| 独立 Running Tasks / Clipboard Shelf 窗口 | [desktop_multi_window](https://pub.dev/packages/desktop_multi_window) | **中至高，保留为对照。** 文档明确每窗口独立 Flutter engine，换库不会自动减少 engine 开销，也无证据自动修复当前辅助 engine 的 AX / VoiceOver 问题 |
| 本应用菜单栏 / 托盘 | [muda](https://github.com/tauri-apps/muda) + [tray-icon](https://github.com/tauri-apps/tray-icon) | **中，Windows / Linux Host 阶段评估。** 可复用跨平台系统菜单与托盘；目前 macOS NSMenu / NSStatusItem 代码直接明确，没有必要单独为此重做 Host |
| Settings 控件、布局、样式 | [macos_ui](https://github.com/macosui/macos_ui)，现有 Flutter / Cupertino | **保留统一 AppStyle。** 可借鉴 / 局部复用控件；整套接管会重新引入另一套主题、焦点和导航，不自动解决一致性或玻璃效果 |
| 玻璃、圆角、窗口焦点、默认返回目标、15 秒状态保留 | 当前 Native Host + Flutter 协调器 | **保留现有窄接口。** 通用窗口库没有证据覆盖本项目所有组合行为；玻璃最终仍依赖系统材质，不能以“换成跨平台包”理解为消除平台实现 |
| 设置保存、快捷键导入导出、模块开关 | 当前 typed settings、原子发布、schema、保存队列 | **保留。** 通用 preferences 包只负责键值读写，不能代替损坏保护、迁移、资格撤销或关闭时等待保存 |
| 第一方模块自动注册与服务绑定 | 当前 Cargo 编译期目录、FeatureDescriptor、typed services、ToolPageRegistry | **保留。** 不引入热加载 / 通用 DI 容器，也不恢复 DSL；当前接口已服务实际业务 |
| Context Graph / Scope / Usage / Ranking / Snapshot | 当前 core + application | **保留。** 算法库可以补底层，不能替代来源归属、Context 派生、稳定身份、可见性、frecency 与 Session / Revision 资格契约 |
| Flutter ↔ Rust 业务桥接 | 已用 [flutter_rust_bridge](https://pub.dev/packages/flutter_rust_bridge) 2.11.1 | **保留。** 使用成熟生成式桥接；不为了引入平台库而把业务协议改成 JSON 消息总线 |

## 五、已经复用、不需要再次替换的基础库

当前 `Cargo.toml` 和 `pubspec.yaml` 可直接确认：

| 领域 | 当前依赖 | 建议 |
|---|---|---|
| 剪贴板 | clipboard-rs | 保留多格式读写 / 监听，产品权限策略留在适配层 |
| 持久化 | rusqlite / SQLite | 保留 repository 边界，无需为当前简单查询引入另一套 ORM |
| 图片 | image | 保留受控解码，按需启用格式 |
| JSON / TOML / XML | serde / serde_json、toml、roxmltree | 保留解析器；不把有界读取、语义识别误认为重复造解析器 |
| URL 与 Unicode | url、unicode-normalization、unicode-casefold | 保留规范化，尤其不能因换 matcher 丢弃中文词 |
| 并发 / 诊断 / 临时文件 | Tokio、tracing、tempfile、anyhow | 保留，按现有 blocking 边界与取消策略使用 |
| 桥接 / 基础 UI | flutter_rust_bridge、Flutter、cupertino_icons | 保留现有技术栈，不为库复用更换 Flutter / Impeller |

## 六、后续功能单独准备

当前未交付的能力不能列为“替换已实现功能”：

- `portable-pty` + `xterm.js`：V0.11 的主要候选，分别承担进程终端与渲染。
- [flutter_inappwebview](https://pub.dev/packages/flutter_inappwebview) / [wry](https://github.com/tauri-apps/wry)：未来 WebView 接入候选，二选一做 Host 原型；不等于现成插件权限 / 生命周期 / 热加载系统，不调整已确定的延期顺序。
- [pdfrx](https://pub.dev/packages/pdfrx)：未来 PDF 富预览候选；增加 native PDF 引擎与分发 / 解码开销，当前只做元数据 / 文本，不在这轮顺带接入。
- [xcap](https://github.com/nashaofu/xcap)：未来截图候选，不能当作当前选中文本 / 项目目录采集的替代。

## 七、建议接入顺序与验收

1. **小范围收益：** 在现有预览接口内试 encoding_rs / chardetng；工具查找器内试 which，不动 Context / Scope。
2. **搜索体验：** 只给应用 / Action 的低层匹配做 nucleo-matcher 对照；保留原 Exact / Prefix 层级和稳定排序。没有用户行为收益则不接。
3. **搜索维护成本：** 单独比较现有 FIFO、ignore / walkdir，以及 SQLite 查询临时存储；按相同路径集合、权限、隐藏 / 排除 / 链接行为测首批、全量、峰值 RSS、取消延迟。性能或覆盖不达标就保留原实现。
4. **平台后端：** Windows 阶段比较 global-hotkey 与 Flutter hotkey_manager 的单一注册链路；普通工具窗试 window_manager，保留 Launcher 的窄 Host 协调。
5. **按既定路线推进 PTY：** 采用成熟 PTY / Terminal 库，只实现本软件独有的 Session / 流控 / 返回交互。

许可要点来自官方源码：nucleo-matcher 为 MPL-2.0；clipboard-rs / Maccy / which / portable-pty / xterm.js 为 MIT；global-hotkey、muda、tray-icon 提供 MIT / Apache-2.0 双许可；notify 主 crate 为 CC0，debouncer 为 MIT / Apache-2.0；encoding_rs 的 Cargo 声明包含 Apache-2.0 / MIT / BSD-3-Clause，具体组件见其 COPYRIGHT。只按实际接入的固定包、feature 和许可文件记录，不把 GitHub 单个自动识别标签当作全部许可信息。

本轮交付为代码盘点与来源核验清单；未变更业务代码、替换依赖或宣称候选库已通过集成 / 性能测试。
