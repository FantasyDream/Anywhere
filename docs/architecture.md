# 架构与代码地图

当前源码为 V0.14 macOS 15+ GPUI 开发预览。Rust Core 决定业务语义，GPUI
呈现并处理输入，AppKit / Quick Look / WebKit 只提供系统适配。最后核对：
2026-11-23。

产品入口是 `apps/gpui/src/app.rs`。当前范围与证据见
[GPUI 交付范围与验收状态](mvp-gpui.md)，窗口、焦点和输入契约见
[Launcher 交互](launcher-interaction.md)与[界面设计](ui-design.md)。

## 分层

```mermaid
flowchart TD
    GPUI[GPUI pages / input / windows] --> Runtime[ApplicationRuntime]
    Runtime --> Application[Application use cases]
    Application --> Core[Core models and algorithms]
    Application --> Features[First-party features]
    Features --> Core
    Application --> Persistence[SQLite / preferences]
    Application --> Platform[Platform capabilities]
    GPUI --> Native[AppKit / Quick Look / WebKit]
    Web[WebView + TypeScript SDK] --> Broker[Rust capability broker]
    Background[On-demand QuickJS] --> Broker
    Broker --> Runtime
```

| 层 | 职责 | 约束 |
| --- | --- | --- |
| `anywhere-protocol` | Context、Action、Candidate、Launcher、执行和 Web 协议模型 | 传输模型；不保存 OS 对象 |
| `anywhere-core/core` | Context Graph、候选、Ranking、SessionFacts、ExecutionSession | 不依赖 application、Feature 或平台实现 |
| `anywhere-core/application` | 激活、Context 采集、查询、快照、取消、执行和装配 | 协调用例；不决定像素或窗口外观 |
| `frontend_runtime` | Application、设置、Usage、剪贴板与 Web document 的共同所有权 | 不拥有窗口、IME 或页面导航 |
| `features` | 应用、文件、文本、URL、Git、Developer、剪贴板与应用历史 | 静态注册；通过贡献接入统一目录 |
| `persistence` | Usage、设置、历史、剪贴板、模块存储与 Web 包 | 有界读写、事务与损坏恢复 |
| `anywhere-platform` | 系统目录、前台应用、Context、文件搜索、剪贴板、资源、菜单与 Git | 窄能力接口；OS 差异留在适配器 |
| `apps/gpui` | 页面、选择、焦点、设置、窗口、状态栏、热键与 Web 文档 | 消费 Core 排序与资格；不复制业务策略 |
| `packages/plugin-sdk` | Web UI、按需后台任务和权限受限 API | 不直接取得文件或进程权限 |

外观和能力由同一进程表达：`resources/design-tokens.json` 生成 Rust 主题投影与
Web CSS；`apps/gpui/src/platform/material/` 读取 macOS 明暗、Reduce
Transparency、Increase Contrast 和 Reduce Motion，安装
`NSGlassEffectView` / `NSVisualEffectView`。GPUI 只绘制透明内容层和语义色，
不伪造原生玻璃，也没有 Flutter / Dart / bridge 之间的 appearance 回传。

图标缓存、文件结果窗口和 Web document 分别限额；当前没有跨语言的
`memory-pressure` 协议或 `ANYWHERE_PROFILE_UI` 开关。性能诊断使用
`performance.md` 中的 Rust/GPUI 入口和显式环境变量，不以已删除的 Flutter
ImageCache / engine 指标推断当前资源行为。

## 代码组织

```text
crates/anywhere-core/src/
├── core/            Context / Action / Candidate / Search / Ranking / Session
├── application/     activate, query, snapshot, execution, settings, Web ownership
│   └── frontend_runtime/  GPUI-facing shared service composition
├── features/        第一方能力、编译期 catalog 与策略
└── persistence/     SQLite、配置、剪贴板、应用历史与 Web 包

apps/gpui/src/
├── app.rs           产品壳、主窗口、状态栏、热键及独立 Shelf / Running Tasks 窗口
├── launcher/        Snapshot gate、页面模型、独立工具窗壳（tool_windows.rs）、图标缓存与视图（controllers/、pages/、settings/）
├── settings/        设置展示模型
├── host/            数据路径、运行时分派、快照投影、原生借用协调、偏好、热键与目录授权
├── platform/        共享桌面接口、能力和键位策略，连接 AppKit 与 Win32 适配器
├── l10n/            共享本地化的 Rust 投影
└── ui/              设计 tokens 投影与 theme
```

第一方能力在启动时绑定一次；设置或外部数据刷新只更新策略和业务数据，不重建
模块生命周期。第三方包使用独立安装批准和 SDK。第三方 Context 限定为
`plugin.<package-id>.*`，脚本输出不能转换为原生文件权限。

## 启动与依赖

`apps/gpui/src/main.rs` 检查宿主环境后启动 GPUI application。
`app.rs` 先创建 Starting 窗口和状态栏壳，随后由独立启动线程打开共享
runtime；Ready 后才注入缓存设置、偏好和热键。Starting / Failed 状态不能预留
Session 或执行动作。`app.rs` 最终创建：

- `LiveLauncher` 与 `ApplicationRuntime`；
- 主 GPUI 窗口；
- macOS `NSStatusBar` 状态项或 Windows 托盘图标；
- 全局热键控制器；
- 按需打开的独立 Clipboard Shelf 与 Running Tasks 窗口；
- 窗口关闭、清理和状态栏恢复路径。

`LiveLauncher` facade 位于 `apps/gpui/src/host/runtime/mod.rs`，在 blocking
runtime 上执行 Core 服务；同目录按 Launcher、Actions、Settings、Clipboard、
Application History、Tasks 和 Web 拆分请求方法。`host/runtime_support.rs`
负责启动偏好应用、目录恢复和领域快照到 GPUI DTO 的纯投影。同步设置或系统
操作不得阻塞渲染线程。产品路径由
`apps/gpui/src/host/mod.rs` 构造，默认数据目录是
`~/Library/Application Support/Anywhere`（macOS）或 `%LOCALAPPDATA%\Anywhere`（Windows）。

GPUI 内的 Session / revision / snapshot gate / stream cancellation 由
`launcher/view/live_state.rs` 持有；File Search、Clipboard、Application History、
Tasks 的页面工作态由 `launcher/model.rs`、`launcher/pages.rs` 与
`launcher/pages/tasks.rs` 持有。文件分页在 `launcher/view/controllers/file_search.rs`
复用 Launcher 快照门禁。Tasks 列表只由任务订阅发布。WebView / document / ack
状态由 `launcher/view/web_surface.rs` 持有；设置编辑态（`SettingsEditor`）、快捷键
录制态（`ShortcutEditor`）、预览态（`PreviewState`）、窗口滞留态（`WindowPresence`）、
滚动句柄（`ScrollHandles`）与图标加载器（`IconLoader`）集中在 `launcher/view/state.rs`。
固定命令由 `launcher/view/actions.rs` 通过 GPUI `actions!` 与 `key_context` 注册派发，
其余按键与 IME 优先级拦截位于 `view/input.rs`；根视图负责路由和跨页意图，
activation / retention 位于 `view/navigation.rs` 与 `view/controllers/navigation.rs`，
通用列表行组件位于 `launcher/components/list_row.rs`，Settings 草稿 / 保存 / 关闭控制器
位于 `view/settings/controller.rs`。原生通道采用有界 admission；激活、恢复、分页和后续 follow
统一把完整 Launcher 快照表达为 `Arc<LauncherState>`，单次回复只投递一个结果。

### 从 Flutter 桥接到进程内共享

Flutter 阶段由 Dart controller 接收生成的桥接 DTO，再投影出页面行。现在 GPUI 和
Application 可以直接借用同一批 Rust 数据。保留 Application 作为异步和授权边界有
实际用途：匹配、目录读取和动作解析可以离开渲染线程，发布和执行仍能检查 Session、
revision、目录授权与动作版本。窗口焦点、选择、语言和布局由 GPUI 单独持有。

| 路径 | 当前所有权 |
| --- | --- |
| Core 提交 → Session、watch、请求回复 | 排序候选、Context 候选与对象动作各自共享不可变数组，并通过 `published_state: Option<Arc<LauncherState>>` 在 watch 通道和查询回复间共享同一只读快照。文件搜索复用 `file_candidate_indices` 与增量 `displayed_file_paths`，不再在执行表重复保存整批文件 `Candidate`。Usage 查找通过 `UsageKey` + `Borrow<dyn UsageKeyRef>` 零拷贝查询，消除每次按键重建临时借用哈希表。执行表始终接收本次复验后的目标和守卫，即使展示完全一致也会更新。 |
| 快照订阅 → GPUI 视图 | `LiveLauncher::subscribe_snapshots` 直接暴露 `watch::Receiver<Option<Arc<LauncherState>>>`，`LauncherView` 在 GPUI executor 上直接监听 `changed().await`，取消额外的 Tokio 中继任务与 `mpsc` 通道；Blocking 线程池限制 `max_blocking_threads(16)` 与 15 秒回收。 |
| 原生图标 → Metal 纹理 | macOS `launcher_application_icon_bgra` / `launcher_file_icon_bgra` 通过 `CGBitmapContext` 直接光栅化预乘 BGRA8 像素，经 `oneshot` 返回并封装为 `Arc<RenderImage>`，消除 PNG 编解码往返；LRU（96 项，隐藏收缩至 32 项）在淘汰与 idle 清理时调用 `window.drop_image(...)` 立即回收 GPU 纹理。 |
| 剪贴板持久化与文件排序 | 剪贴板 SQLite 启用 WAL、2 MiB 页缓存与 FTS5 `trigram` 虚拟表（v4 迁移自动重建索引），中英文长词查询走索引交集，短词回退 `instr` 保证精确子串语义；删除/清理后执行 `wal_checkpoint(TRUNCATE)`。文件搜索外部排序在非测试构建提升至 4 MiB / 4096 条每 run，减少临时文件落盘与归并轮次。 |
| 快照 → 文件结果行 | 文件页保留共享候选和去重后的下标。方向键、分页恢复只读 ID 和可用性，虚拟列表（包括主 Launcher 列表在内的全部列表统一使用 `uniform_list`）按视口投影文字。普通搜索、菜单和混合结果沿用各自的展示规则。 |
| 选中对象 → 对象动作 | 文件和历史对象在入队前由 Core 记录选择顺序。较早的选择即使晚开始也不能覆盖已受理的新选择。文件动作在后台解析，GPUI 接收该请求复验过的快照，并在回包时核对页面、父对象与快照游标。 |
| 第三方 Web 工具 → Rust | 文档 handle、IPC、授权和资源预算仍由 broker 管理。这里跨越插件信任边界，页面无法直接借用原生执行对象。 |
| Launcher 页面 → 独立普通窗口 | 文件搜索、剪贴板历史、应用历史和 Web 工具从当前快照复制出独立 Session。主 Launcher 立即回到自己的搜索会话，窗口关闭只撤销自己的 Session。插件目录、来源授权或捕获策略变更会使所有旧工具窗口的执行资格立即失效，窗口保留并显示重新打开提示。 |

## 激活与 Context

1. UI 在展示任何窗口前预留 Session。
2. Rust 冻结当前前台应用身份，启动 Context、应用目录与历史读取。
3. InputMode 决定初始种子：Environment、Content、Search 或 FileSearch。
4. Provider 派生 Context；来源读取在 blocking worker 中完成。
5. 首份快照提交后显示或更新窗口；迟到 Context 以新的
   `SnapshotSeq` 继续发布。
6. 查询变化推进 `Revision`，旧结果在发布前按 Session / revision / snapshot
   游标复验。

当前应用读取按 Session 单飞。Launcher 或其他已取得焦点的应用不会成为新的
“来源应用”。缺失某项权限时只移除对应来源，其他动作继续运行。

已安装应用目录由 `application/application_index.rs` 独立刷新。最多进行一次扫描并保留
一个待执行请求。扫描中的连续激活合并为最新请求，已有目录仍可供当前查询使用。

## 快照、取消与执行

| 身份 / 版本 | 区分什么 |
| --- | --- |
| `SessionId` | 一次激活 |
| `Revision` | 同一会话的输入或查询模式变化 |
| `SnapshotSeq` | 同一 revision 的迟到补充 |
| `BusinessActionId` | Usage 与持久配置中的动作 |
| `ExecutionCandidateId` | 已提交候选及具体目标 |
| Catalog version / execution epoch | 定义、开关或权限变化后的执行资格 |
| Host / preview generation | 窗口、Quick Look 或 Web 文档替换 |

快照流程采用“锁内复制不可变事实 → 锁外匹配 / 排序 → 锁内复验并发布”。
GPUI 的顺序门禁在 `apps/gpui/src/launcher/snapshot_gate.rs`。

执行请求在排队前冻结候选、来源应用、Session / revision、执行 epoch、功能策略
与目标 token。worker 在副作用前再次复验。执行成功即使窗口已隐藏仍记录 Usage。
隐藏窗口立即撤销待执行资格；已经启动的子进程由 ExecutionSession 独立持有。

空动作列表由 Rust 比较 Usage、最近使用、相关性和稳定 ID。关键词、文件和历史
结果保持各自契约；GPUI 只过滤当前 Scope 并限制可见行。

## 文件与预览

`features/filesystem/scan.rs` 持有查询寿命，子模块分别负责查询、范围、记录、
遍历、写入预算（`storage.rs`）、进度和分页；`result_store.rs` 维护连续外部排序。已显示前缀必须稳定，
查询切换后旧结果不能追加。

系统索引提供早期结果，完整目录遍历补充其余命中。磁盘授权在读取前复验；
文件、目录、工作区文件和远程 URI 保持不同类型。Quick Look 通过
`apps/gpui/src/platform/preview.rs` 管理 generation；关闭回调只作用于当前预览，
完成后恢复当前页面输入焦点。详见[文件搜索](file-search.md)和
[系统预览](native-preview.md)。

## 剪贴板与应用历史

剪贴板采集默认关闭。启用后，文本、图片、文件列表与可读取格式存入 SQLite；
查询、去重、配额和原始数据读取由持久化层负责。Clipboard History 是主窗口工具，
Clipboard Shelf 是独立的置顶 GPUI 窗口，两者复用同一策略与结果模型。
`HistoryService` 将策略 / `epoch` / 缓存状态锁（`state`）与 SQLite 仓库锁（`repository`）
分离，严格遵守 `state -> repository` 加锁顺序，读写 SQLite 前释放 `state` 并以 `epoch`
校验并发失效，避免大图片写入或过期清理阻塞状态读取。

应用历史通过 `source.json` / `transform.js` 读取一个选定外部来源，运行有界纯
JS 转换并形成普通候选和对象动作。它与 Launcher Usage、剪贴板历史是三类独立
数据。来源执行前重新读取并核对目标；失效或歧义时拒绝打开。详见
[应用历史](application-history.md)和[来源指南](application-history-sources.md)。

## 设置与系统适配

设置草稿、已保存值和保存结果分开管理。每个操作在 blocking runtime 中执行，
成功后才更新已保存快照；失败保留可重试状态。

- 热键：`apps/gpui/src/host/hotkeys.rs`，持有真实注册状态和最近错误，注册失败恢复旧绑定；菜单提示按实际注册投影，设置页保留诊断并允许重试。
- 原生启动配置与外观更新：`apps/gpui/src/host/native_window.rs` 通过 AsyncApp 的可失败窗口更新探测借用边界，释放借用后再执行可能同步触发 resize 的 AppKit 操作，最后返回 GPUI 更新页面。嵌套事件循环中借用尚未释放时等待重试；窗口关闭或 owner 释放时取消。普通 GCD 异步入队不能作为 GPUI 借用已释放的保证。
- 目录：`apps/gpui/src/host/directory_access.rs`，使用安全范围书签。
- 登录项：`apps/gpui/src/host/login_item.rs` 对照系统状态应用保存值。系统操作失败可在通用设置独立重试，直接使用已保存的启动偏好。显式数据目录下跳过系统操作。
- 窗口、状态栏与输入适配经 `apps/gpui/src/platform/`。页面共用布局和命令表，按能力呈现可用操作。AppKit 与 Win32 适配器负责原生窗口、材质和系统通知。
- Quick Look：`apps/gpui/src/platform/preview.rs`。
- Web document：`apps/gpui/src/web_tool.rs` 与 `launcher/view/`。

这些适配器的自动化只证明输入输出与清理顺序。物理权限、IME、系统批准与 WebKit
helper 行为必须按[验收矩阵](mvp-gpui.md#物理桌验矩阵)单独记录。

## Web 插件

安装时 Rust 验证 Manifest、资源、摘要和权限。批准后 WebView 只访问固定 origin
与已验证资源；页面通过受限 JSON 字符串帧 IPC 进入统一 broker。Rust 拥有文档 handle、
请求资格、预算和取消，GPUI 只挂载视图和转发生命周期事件。

替换文档会推进 generation。旧页面的资源请求、ack、状态事件或关闭回调不能影响
新文档。Web 包损坏只移除可选 Web 能力，不阻断 Launcher 启动。按需 QuickJS 使用
同一能力代理，当前仍为进程内执行；进程隔离尚未交付。

## 扩展入口

- 新 Core Context：定义类型与值，注册 Provider，并通过贡献接入策略。
- 新 Action：定义 ActionSpec / Handler / Bundle，声明执行能力与绑定资格。
- 新候选来源：实现 CandidateSource / AsyncCandidateSource，复用 Ranking 与寿命。
- 新页面：使用 typed `HostView::NativeTool` 路由，在 GPUI 页面层映射已知类型；
  未知或歧义 ID 明确拒绝。
- 新系统能力：先增加窄 Platform 接口，再由各平台适配器实现。
- 新第三方能力：统一通过 `.awplugin` Web 包、TypeScript SDK、Manifest 与权限代理接入；旧声明式 TOML 插件已下线。

## 验证

```sh
scripts/check-gpui.sh
scripts/check.sh
git diff --check
```

`check-architecture.py` 验证 Cargo 依赖与 Core 反向导入。自动化通过不代替真实
键盘、权限、窗口、Quick Look、WebView、VoiceOver、资源趋势或跨平台桌面产物。

## 本轮组织与通信约定

GPUI 工具列表采用 `uniform_list`，仅投影可见行。设置的七类表单位于
`apps/gpui/src/launcher/view/settings/`；设置决策在 controller 中，原生窗口延迟
变更经 `host/native_window.rs` 避免重入 GPUI 的窗口借用。macOS adapter 按
window、status_item、observers、activation 拆分，以原生 identifier 查找窗口。
Clipboard Shelf 与 Running Tasks 独立于主窗口，关闭会释放各自实体。

Core Launcher 状态只由 watch 发送最新不可变快照；已移除无生产消费者的旧
LauncherEvent 广播。动作结果与有限请求仍用有界通道，不缓存过期执行资格。
历史 Usage、单来源应用历史和剪贴板历史继续各自存储。

SessionCoordinator 封装 Session 锁与 execution epoch；Resolver 的准备 / 复验和
`session_runtime/publication.rs` 的快照组装分开。完整提交与文件进度快路径都在
同一组 Session / policy 锁内通过 `publish_state` 投递 watch。普通对象和应用历史
对象共用请求票据、借用 Usage 查询、动作匹配及发布复验；历史对象保留自己的
来源、路径和执行守卫。

Core 与 `host/` 中的共享锁按语义通过 `crates/anywhere-core/src/core/sync.rs`
统一处理中毒策略，并由 `scripts/architecture_rules.py` 的 `check_lock_usage`
门禁禁止生产代码直接 `.lock().unwrap()` / `.expect()`：

| 类别 | 锁不可用时 | 适用 |
|---|---|---|
| 授权 / 发布 | 拒绝（返回错误或不发布），记录 `error` | 能力检查、候选发布、RPC 准入 |
| 撤销 / 清理 | 恢复守卫并继续执行，记录 `error` | Web 失效、文档关闭、会话放弃、来源关闭 |
| 纯缓存 | 恢复守卫并重置为默认值 | 目录 memo、offer / 投影 / 快照缓存 |
| 服务状态 | 转为错误返回，不 panic | 应用历史、文件搜索、扫描队列 |

空查询 offers、Scopes 和快捷键目录使用有界单项缓存，键包含动作注册表、Context、
Facts、Usage 与排名时钟。动态工厂包含请求版本，静态 offers 在使用时绑定回当前
Context；缓存不能授权旧执行。开发者 Project 的字符串属性仍是现有捕获元数据契约，
没有在本轮增加 Core 对 Node / Cargo 类型的反向依赖。

Finder / 浏览器固定只读脚本在串行 OSA 组件内缓存编译，外部 Apple Event 绑定
捕获的 PID；每次发送与返回均复验进程启动身份，并设置事件和解释器执行期限。
首次 OSA 编译与系统授权仍需独立桌验。资源打开统一通过 NSWorkspace，保留
选定 JetBrains 安装的显式参数交接。
