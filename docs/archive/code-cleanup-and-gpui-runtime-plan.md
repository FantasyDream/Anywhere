> 历史归档：以下记录保留原日期与当时的实施状态。当前契约以 [架构](../architecture.md)、[UI 设计](../ui-design.md)与[验收](../mvp-gpui.md)为准。

# 全仓代码整理与 GPUI 通信优化实施方案

状态：实施中。审查与首批实现日期：2026-11-13。核心通信、生命周期和主要
页面状态迁移已经落地；全仓逐域审查、深层页面 Entity 拆分、完整性能 A/B 和
物理桌面矩阵仍需继续。本文不是“全仓重构已完成”的声明。

基线为 `codex/experiment-gpui-kit` 的 `c00dce4` 加当前未提交工作树。
本轮 `scripts/check-gpui.sh` 的 89 项测试、完整 `scripts/check.sh` 和
workspace 771 项通过 / 6 ignored 均为当前源码结果。实施前的审查基线仍与
最终结果分开记录；更完整的物理证据边界见 [GPUI 验收](../mvp-gpui.md)。
方案初版只新增文档；随后已按下方 T00–T11 / T20 的首批切片修改产品实现。

契约入口：[架构](../architecture.md)、[Launcher 交互](../launcher-interaction.md)、
[文件搜索](../file-search.md)、[原生预览](../native-preview.md)、
[UI 设计](../ui-design.md)、[代码规范](../code-style.md)、[性能](../performance.md)。

### 实施进度（2026-11-13）

已落下可独立复核的改动，方案整体仍未完成：

- T03：同模式菜单恢复不再推进 stream generation；已加入定序回归并通过。
- T04：状态 follow 改为 Core watch 上的共享快照，接收端关闭时立即结束；
  Launcher 与 Tasks 的旧等待均有显式取消信号。
- T06/T07：DTO、启动偏好应用和快照 / 页面 DTO 投影已从 runtime facade
  分到 `dto.rs` 与 `runtime_support.rs`；Launcher、Actions、Settings、
  Clipboard、Application History、Tasks、Web 请求方法继续拆到
  `host/runtime/{launcher,actions,settings,clipboard,app_history,tasks,web}.rs`。
  所有 GPUI 原生
  通道改为有界。激活、恢复、输入、分页与 follow 统一使用
  `LiveEvent::SharedState(Arc<LauncherState>)`，产品链不再保留 owned 快照双分支，
  也不在 host 转发链做解包与重装箱。
- T09/T10：启动设置 / preferences 使用缓存，Launcher 模型保留共享快照，并
  在候选、Context、Scope 与输入未变时复用现有 5 / 7 行投影。
  `LauncherView` 先以 Starting 状态创建 GPUI 外壳，独立启动线程才打开
  `ApplicationRuntime`、SQLite 与插件目录；Ready 后一次注入缓存设置、偏好和
  热键，Starting / Failed 状态不预留 Session 或执行动作。
- T12/T13：File Search、Clipboard、Application History、Tasks 和 Web 的页面
  工作态从根控制器拆出；Session、revision、snapshot gate 与 stream cancellation
  归入 `live_state`，WebView / document / ack 归入 `web_surface`。Settings 的共享
  卡片、字段布局以及草稿 / 保存 / 关闭控制器分别位于独立模块；导航、激活 /
  retention 和按键 / IME 路由也已移出根文件。根控制器仍负责页面选择与少量
  跨页焦点，页面 Entity 深度迁移仍是后续工作。
- T11：执行管理器发布 watch 更新；Tasks 删除 1.5 秒轮询，改为 40 ms 合并
  通知，并用 `output_revision` / summary 判断是否重新读取完整输出。
- T05：Host preferences 与目录书签使用 `take(limit + 1)` 有界读取，超限文件
  不会先整体分配；运行期启动失败改为窗口错误，不再在 GPUI 回调 panic。
- T17（部分）：执行管理器的 `list` / `detail` / `shutdown` 现在传播锁不可用，
  不再把管理器损坏伪装成空任务列表；记录锁仍采用有日志的 poison recovery，
  应用排除更新失败也会留下明确 warning。该检查点有 poison 回归，但 Core 其他
  领域的锁审计仍未全部完成。
- T20：Web IPC 入站队列有界，AppKit / 全局热键通道也使用有界非阻塞发送；
  字节脚本使用 serde 生成十进制数组，不再逐字节 `String` 拼接。
- T16（部分）：`LiveLauncher` 最后一个 owner 释放时使用
  `Runtime::shutdown_background`，避免 Core / helper 收尾阻塞 AppKit / GPUI
  主线程；Web、preview 的 generation 与显式关闭继续保留。
- T02：新增 `projection_measure`，10,000 候选 Release 基线与证据边界记录在
  [性能](../performance.md#2026-11-13-合成投影基线)。
- T24（部分）：r4 隔离包完成隐藏启动、UIElement / 无 Dock 启动、真实热键、
  key / active、英文查询、中文 IME、72 / 137 / 473 高度与固定顶部、15 秒内
  恢复、Finder 失焦隐藏、模式选择器、Settings 六类，以及白 / 深背景 Glass
  截图。r5 重新构建异步启动链后，LaunchServices 仍报告 `UIElement`，状态栏先
  用安全默认值出现，随后约 105 ms 后替换为磁盘偏好和真实热键（准备材质到热键
  Ready 的日志跨度约 134 ms）；真实热键触发
  这次启动的隔离进程并在 60 ms 探针读到 `key=true app_active=true`。机器随后
  锁屏，r5 的逐页视觉复核未继续。Reduce Transparency、Increase Contrast、
  浅色系统主题、VoiceOver、WebView 与 Quick Look 的完整物理矩阵仍未完成。

尚未完成的重点是 runtime facade 的批量查询准入、页面 Entity 深度迁移、
platform facade 继续拆分、完整 25 项覆盖账本、性能 A/B 的全链路指标和其余
物理桌面矩阵。本段不得作为“全仓重构完成”证据；已通过的物理项以
[GPUI 验收](../mvp-gpui.md)的具体序列为准。

## 1. 结论与范围

采用“先固定行为和测量，再修具体缺陷，再按职责重构，最后验证收益”的顺序。
代码整洁的目标是减少修改一个功能时需要同时理解的状态、模块和隐藏副作用；
不把减少行数、增加 trait 数量或消除所有 `clone` 当作完成标准。

GPUI 与 Core 已经是同一进程中的 Rust 调用，没有 Flutter FFI、HTTP 或 JSON
序列化这一层主链路开销。应保留现有 Application / Tokio，优化状态所有权、
通知、拷贝、请求取消和 UI 投影。WebView 插件仍是独立信任边界，继续使用 SDK
和能力代理，不能套用原生 UI 的直接调用方式。

本轮范围覆盖所有随仓库维护的源码、资源生成器、测试、脚本和交付配置。
第三方依赖源码不在清理范围；依赖升级、Rust edition / toolchain 升级、
新 UI 框架、插件进程隔离和新平台 App 不混入行为保持型重构。

## 2. 已核实的问题与待验证风险

以下定位针对当前工作树，函数名优先于可能变化的行号。

| 编号 | 发现及证据 | 分类 | 优先级与处置 |
| --- | --- | --- | --- |
| F01 | `LauncherView::activate_from_entry` 在已显示、同模式的菜单恢复路径先增加 `stream_generation`，随后仅聚焦并返回；`poll_follow` 继续使用旧代次 | 已加入同模式定序回归并修复；静态跟随仍被保留 | 已关闭；同模式聚焦不得取消当前订阅 |
| F02 | `LiveLauncher::follow_snapshots` 只在发送下一份状态失败后发现 receiver 已关闭；30 秒 timeout 后直接继续等待；GPUI 的等待任务大量 `detach` | GPUI 持有独立 cancel sender；follow 同时监听 receiver close；旧任务有合成回归 | 已关闭通信生命周期缺口；100 次真实唤醒资源趋势仍需物理采样 |
| F03 | `LauncherView::build` 同步调用 `settings_snapshot()`、`preferences()`；后者直接读文件，前者取得设置锁并读取剪贴板状态；`app::run` 同步打开 runtime / 存储 | GPUI 主窗口先进入 Starting；独立启动线程打开 runtime / SQLite / 插件目录；Ready 后一次注入缓存偏好、设置与热键，Failed 显示错误窗口 | 已关闭主线程阻塞与半初始化执行缺口；慢存储 fixture 的物理时延仍待采样 |
| F04 | Core 的 watch 已保存 `Arc<LauncherState>`，但 `latest_state` 深拷贝其内部非共享字段，再包装为 `Box`，经 host 无界通道转发 | 产品链统一为 `LiveEvent::SharedState(Arc<LauncherState>)`，无 owned / shared 双分支；通道有界 | 已关闭结构性复制；仍保留一个受限 follow 转发任务，需用全链路 A/B 决定是否再直接暴露 watch |
| F05 | `LauncherState.candidates` 已经是 `Arc<Vec<_>>`；文件追加和进度发布已有共享 / 快路径，GPUI `LauncherModel::apply` 仍每次重建行 | 既有优化需保留；元数据更新触发重复投影 | P2，区分数据、选择、进度和样式失效原因 |
| F06 | `host::runtime::spawn` / `spawn_blocking` 为单个结果使用 `unbounded_channel<LiveEvent>`；查询状态也经无界队列 | 单次结果使用容量 1 的有界 mpsc；follow、原生事件和 Web IPC 都使用明确容量，AppKit 回调不阻塞等待 | 已关闭无限排队；typed `oneshot` 迁移作为后续 API 精简，不是当前正确性缺口 |
| F07 | Tasks 每 1.5 秒刷新摘要并重新请求选中任务的完整输出；Core 已有 `output_revision`，GPUI `ExecutionRow` 未保留 | Tasks 已改为 execution watch + 40 ms 合并，保留 `output_revision`，摘要未变时不重读完整输出 | 已关闭；输出突发和窗口重开仍纳入物理 / 资源矩阵 |
| F08 | Launcher 根视图仍约 7,200 行；Settings 展示仍约 2,200 行；platform facade 仍集中 | host runtime 已拆成 facade、support 与七个领域请求模块；页面工作态、Session、Web surface、导航 / activation、输入路由和 Settings 控制器已拆出，根文件约 6,100 行 | 部分关闭；页面 Entity 与 platform facade 仍可继续收口，不能只按行数宣称完成 |
| F09 | `HostPreferences::load` 和目录书签 load 直接 `fs::read`；Core 配置模块已有有界读取，但 host 未复用同类约束 | 两者使用有界读取、私有临时文件、`sync_all` 与原子 rename；超限、损坏、并发保存和权限失败有回归 | 已关闭 |
| F10 | `web_tool::reply_script` 将每个 UTF-8 字节转十进制字符串，再构造 `Uint8Array` JS 源码；Web IPC 入站队列在 broker 前无界 | Web IPC 在最前面按容量拒绝溢出；字节参数由 serde 生成，不再逐字节拼接；Unicode / 大帧 / 乱序协议测试通过 | 已关闭确定缺口；编码路径的独立微基准仍未形成整 App 结论 |
| F11 | Core execution manager 部分锁中毒使用 `unwrap`，其他入口返回空列表 / None；部分清理用 `let _`，错误表达不统一 | `start` / `list` / `detail` / `stop` / `remove` / `shutdown` 对管理器锁统一返回明确不可用；record poison 会记录并恢复；新增 poison 回归 | 该执行管理器检查点已关闭；Core 其他锁继续按 T17 / T18 审查 |
| F12 | `app.rs` 有未调用的 `_window_bounds_contract`；平台有依赖类型探针；旧迁移注释仍称 bridge / first vertical slice | `_window_bounds_contract` 已无调用；LauncherView 的旧 “first vertical slice” 说明已替换；现役 platform bridge 注释保留其真实含义 | 已关闭无用途清理候选，不误删协议兼容代码 |

审查覆盖情况：GPUI 主壳、host runtime、Launcher 状态与订阅、原生材质、Core
发布和任务路径已做调用链阅读；其他目录完成结构、依赖和重点实现抽样。
Persistence、SDK、所有 feature / 平台适配器仍需按第 7 节完成逐模块审查，
不能把当前发现列表当成一次完整安全或 Bug 审计。

## 3. 必须保持的行为

1. 排序、可用性、Context 和执行资格由 Core 决定。Unavailable 菜单隐藏；
   缺少 Accessibility 不阻断其他动作。Current App 不新增独立 Menu Scope。
2. `SessionId / Revision / SnapshotSeq`、catalog version、execution epoch、
   请求 / 窗口 / 文档 generation 的校验继续发生在发布及副作用前。
3. 隐藏立即撤销执行资格，15 秒仅保留展示工作区。后台进程寿命属于应用，
   不因关闭 Tasks 窗口或取消 UI 请求而终止。
4. 文件结果保持稳定前缀、完整遍历、有界内存、原有路径与授权区分；
   不把优化通信改成丢分页、丢终态或引入全盘常驻索引。
5. Settings 的草稿、已保存值、热键回滚、登录项和目录授权保持原事务语义。
   老配置、动作 ID、JSON 字段和 SDK 错误码保持兼容。
6. Clipboard History、独立 Shelf、Application History 和 Usage 互相独立；
   测试使用合成数据，不采集或改写个人剪贴板历史。
7. Web 文档只能由 Rust handle 访问；权限、origin、main frame、预算、ack、
   transfer、取消和迟到回调隔离均保留。
8. GPUI Root / 页面底板保持透明，由原生 Glass / HUD 提供材质；
   Reduce Transparency 仍使用不透明 fallback。保留已修复的固定行高、菜单尺寸、
   顶部锚点和中文布局，重构不重新设计 UI。
9. AppKit / GPUI 状态只能在相应主线程访问，原生对象不能为迁移方便加
   未证明安全的 `unsafe impl Send/Sync`。

## 4. 原生通信的目标设计

### 4.1 当前路径及应保留的能力

```text
Core publication
  -> watch<Option<Arc<LauncherState>>>
  -> wait_for_snapshot -> latest_state 的 owned clone -> Box<LauncherState>
  -> LiveLauncher::follow_snapshots -> unbounded LiveEvent::State
  -> GPUI recv().await -> SnapshotGate -> LauncherModel::apply
```

主状态已经通过 `recv().await` / watch 驱动，不是每帧轮询。
120 ms 历史查询延迟是 debounce；220 ms 失焦宽限是交互策略；不能作为“轮询”
统一删除。Tasks 的 1.5 秒刷新和 Quick Look 的状态监视要分别审查。

`LauncherState.candidates` 已共享，收益主要来自外层快照及 Context / result
actions 等非共享字段的重复复制、转发任务和无效行投影，不承诺整份快照当前都
在深拷贝，更不宣称需要替换成共享内存或自定义二进制协议。

### 4.2 按语义选择通道

| 数据 | 目标传输 | 丢弃 / 合并规则 |
| --- | --- | --- |
| 最新完整 Launcher 状态、文件进度、任务摘要 | `watch<Option<Arc<T>>>` 或封装其上的 session-aware subscription | 允许合并中间完整状态；保留最新值和资格，终止用明确状态 / 关闭表示 |
| 单次查询回复、保存结果、安装审查、执行结果 | typed `oneshot<Result<T, HostError>>`，有准入预算 | 每请求一个结果；不把副作用结果覆盖成“最新值” |
| 必须逐条处理的通知、Web 入站命令 | 有界队列及明确 admission / Busy 策略 | 不静默丢执行、保存或 IPC；不要在 AppKit 回调中阻塞等待容量 |
| 纯本地选择、hover、焦点、滚动 | GPUI Entity / 本地模型直接更新 | 不发往 Core，除非会影响查询或资格 |
| 图片、静态目录和有界输出快照 | 不可变数据 / Arc 与有界缓存 | 缓存 key 包含身份和版本；不无限延长资源寿命 |

watch 只保留最新值，读取循环使用 `borrow_and_update` 并及时释放借用，
不能跨 await 持有 watch 的 `Ref`。这是 Tokio 的明确语义，适合完整可替换状态，
不适合逐项执行结果。[Tokio watch 文档](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)

一次性结果和有界队列可复用现有 Tokio 能力；不额外引入另一套 runtime。
[Tokio channels 指南](https://tokio.rs/tokio/tutorial/channels)给出了相应使用模式。

### 4.3 建议接口形状

以下是待实施的接口草图，不是已存在 API；具体名字可按现有风格调整。

```rust
// Application 层：不包含 GPUI Entity / Window / AppKit 对象。
type SharedLauncherState = Arc<LauncherState>;

// 每个有效 Session 只保留一个状态订阅；内部沿用现有 watch。
fn subscribe_launcher(session_id: SessionId) -> Result<LauncherSubscription>;

// Host 层：Application 异步工作始终由所拥有的 Tokio runtime 驱动。
fn submit_query(request: QueryRequest) -> PendingRequest<QueryAck>;
fn execute(request: ExecuteRequest) -> PendingRequest<ActionResult>;
fn save_preferences(request: SavePreferences) -> PendingRequest<SavedPreferences>;
```

具体规则：

- `LauncherSubscription` 包含 session 身份、最新状态读取、changed 与关闭语义；
  不直接把全局 watch 裸露给所有页面。提供 `Arc` 读取 API，owned 兼容接口
  暂留给现有测试 / 调用方，逐个迁移，查清调用后再删。
- GPUI 直接等待可跨 executor 等待的 watch / oneshot 接收端，不每个快照再起
  一个 Tokio 转发任务。Core 查询 future、计时和原生业务操作仍由 Tokio / worker
  驱动；不能把它们直接塞进 GPUI foreground future 后假定存在 Tokio reactor。
- `QueryRequest` 显式冻结 session、revision、mode、scope / filter；
  `ExecuteRequest` 冻结显示时的候选身份。异步执行时不重新读取 host 的
  可变“当前 session”来代替请求身份。
- 初步保留查询调用的现有返回行为，先让共享订阅成为唯一的状态提交入口。
  单次回复用于 ack / 错误，不能和订阅重复提交同一状态。是否进一步改为
  “仅 ack”的 Core API 在兼容和测量通过后决定。
- 实现旧订阅停止后再挂新订阅，并在挂接时读取最新值，覆盖“先发布、后订阅”
  的时序。SnapshotGate 始终保留；watch 不能替代执行资格和顺序检查。
- 共享状态只读；UI query、selection、draft、focus 保持独立。不能对共享快照
  用 `Arc::make_mut` 修改业务结果来迁就某个页面。

### 4.4 取消和任务所有权

将取消拆成三件事：停止 UI 等待、停止可取消读取、撤销或管理业务副作用。

1. View 持有 `Task` / request scope，导航或隐藏时主动 drop / cancel；
   不只提高数字后等待下一次结果才结束旧任务。
2. Scope 至少区分 Session、页面查询、选中对象、Settings 保存、Web document，
   不能一个全局 generation 顺手让不相关的保存或文档失效。
3. 后台订阅使用关闭信号和状态变化的 select，receiver 关闭能立即结束等待；
   每个 Session / 页面活动任务数量有上界并可记录。
4. `spawn_blocking` 已开始的工作不能靠 abort 强制终止；有界读取、Semaphore
   准入、循环内取消检查和副作用前复验仍必要。执行中进程通过显式 Stop 管理。
5. 应用级热键 / 状态项监听可持有应用级长任务；有限的原生宽限定时器可保留，
   说明它们的 owner、取消条件和上限，不进行“一律禁止 detach”的机械替换。

GPUI `Task` drop 会取消未完成任务，和 Tokio JoinHandle drop 的语义不同，
实施时必须分别管理。[GPUI Kit Task 指南](https://gpui-kit.com/docs/task/)
`spawn_blocking` 已开始后不可中止的限制见
[Tokio 官方 API](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)。

### 4.5 UI 投影与渲染

- `LauncherModel` 保存只读 `Arc<LauncherState>`；允许复制可见的 5 / 7 个小型
  row 展示字段，不为消除少量 String clone 搞复杂借用生命周期。
- 将 metadata 更新和 row rebuild 分开，返回类似 `ModelChange` 的变化描述：
  rows、selection、scope、progress、notice、desired size、editor value。
- 只有编辑值变化才写 InputState；尺寸变化才安排 resize；可见图标集合变化
  才发起图标请求。保留延迟 resize 和重算最新页面的修复。
- 行缓存必须考虑 locale、scope、菜单 / 对象动作、上下文标签、可用性和列表
  身份。`candidate_list_version` 在文件追加时可以不变，不能单独用它判定整表
  未变化；progress-only 的共享 Arc 相同也不能忽略其他展示依赖。
- 先通过 Arc 身份和明确变化标记消除重复投影；大列表虚拟化仅用于测出瓶颈的
  Tasks / History / Web 包列表。Launcher 原有行数上限不需要虚拟化框架。
- 不给每个标签创建 Entity，不使用全局大锁缓存所有页面，不为批处理固定增加
  一帧输入延迟。若状态突发压垮主线程，再测量每帧至多一次可替换状态提交。

### 4.6 方案取舍

| 方案 | 决策 | 理由 |
| --- | --- | --- |
| 进程内 typed API + Arc + watch / oneshot | 优先实施 | 使用已有依赖，减少复制和中间队列，生命周期可测试 |
| 去掉 Tokio，所有业务改 GPUI executor | 不作为本轮目标 | Core 已依赖 Tokio、取消和 worker；风险与收益不匹配 |
| 所有命令进入单一 Actor | 不默认采用 | 会把慢设置 / 磁盘读取排在查询和执行前；只串行必须串行的写入 |
| RPC / HTTP / WebSocket / 新二进制协议 | 原生主链路不采用 | 增加序列化、服务生命周期和错误面 |
| lock-free、自定义 ring buffer、全量 snapshot diff | 测量证明需要后再提案 | 当前尚无对应瓶颈证据；会增加排序、丢包和恢复复杂度 |
| WebView 绕过 Rust broker | 不采用 | 原生优化不能取消第三方权限和文档隔离 |

## 5. 代码组织目标

保持现有 crate 边界，不为每个内部职责新建 crate。
下面是目标职责，不要求一次创建所有文件或先搭抽象框架。

```text
apps/gpui/src/
  app.rs                      装配入口
  shell/                      主窗口 / Shelf / Tasks 窗口与应用级事件
  host/runtime/
    mod.rs                    runtime 所有权和对外 facade
    request.rs                typed reply、准入、取消、错误映射
    launcher.rs                Session、输入与共享订阅
    settings.rs                设置快照、保存和原生效果协同
    clipboard.rs / history.rs / tasks.rs / web.rs
  launcher/
    model.rs                  Launcher 展示状态与纯变换
    presentation.rs           图标、文案与格式映射
    snapshot_gate.rs          顺序校验
    view/mod.rs               视图装配、当前页面、跨页面焦点
    view/input.rs             按键 / IME 路由
    view/navigation.rs        当前页面、返回与保留工作区
    view/session.rs           激活、订阅、隐藏与恢复
    view/settings/            七类设置、录制 / 草稿 / 保存展示
    view/pages/               页面渲染及本页状态 / 请求 owner
    components/               有稳定语义的共享控件
  platform/
    mod.rs                    窄 API / cfg facade
    window.rs / activation.rs / status_item.rs / observers.rs
    material.rs / preview.rs  保留已有的独立原生边界
```

重构关键是将 `web_*`、`clipboard_*`、`tasks_*`、`settings_*` 等字段收敛为
各自状态对象 / Entity，建立进入和退出语义。单纯把同一个大 `impl` 搬进十个
文件，或让新模块继续通过 `&mut LauncherView` 任意改全部字段，不算完成。

允许页面最初仍由根 Entity 渲染；只有需要独立通知、滚动或生命周期的页面再
提取 Entity。根层持有返回导航和工作区，工具页不拥有 Core Session 或重写授权。

Core 中仅优先整理真实耦合点：

- `launcher_service`：facade、共享订阅、retention / 生命周期分开。
- `session_runtime`：采集不可变事实、计算、复验、发布步骤明确，保留提交原子性。
- `core/action/descriptor`：定义 / 已注册值 / registry 与构建搜索字段按职责分开，
  只在有扩展需求处保留 trait，不复制 registry 或引入另一份动作目录。
- `core/execution`：manager、record / 输出、进程 worker 分开，先统一锁顺序、
  失败语义与资源回收，再移动代码。
- `frontend_runtime`：保留共享运行时所有权；按 settings / web 等领域提取方法，
  不为了名字中有 frontend 就删除。

## 6. Rust 与代码整洁约定

1. 函数以一个可命名的业务步骤为职责，I/O、状态计算和 UI 绘制分离。提取函数
   必须减少调用者需要理解的细节，不把两行简单表达式扩成多层跳转。
2. 尽量让非法状态无法表达，例如 Web 的 Unmounted / Mounting / Ready / Retained
   和 Settings 的 Editing / Saving / ClosePending；只把确实互斥的字段合并成 enum，
   不把所有正交状态相乘成巨大状态机。
3. 内部请求 ID、窗口代次可用轻量 newtype 防止混传；不在同一提交批量替换
   Protocol 的所有 String / u32 alias，不改持久化和序列化契约。
4. 错误在边界保留 kind / code / source 和操作上下文，UI 再本地化。业务可恢复
   错误不靠解析英文字符串；内部链条使用 `?`，顶层装配可继续使用 anyhow。
5. 不机械删除 unwrap / expect：测试、确实不可失败的静态构造保留并说明；生产
   I/O、用户数据和锁中毒逐个处理。不能用 `unwrap_or_default` 把失败伪装为空结果。
6. `std::sync::Mutex` 用于短同步临界区，异步锁只用于必须跨 await 的串行事务。
   不持锁 await / join / 阻塞 I/O；记录现有锁顺序，避免“优化”成反向加锁。
7. Arc 表达共享不可变所有权，不是默认包裹所有值；小值用所有权移动 / 借用，
   大字符串或数组是否共享由测量决定。不要默认把每个 String 改为 Arc<str>。
8. 临时目录、书签访问、观察者、Web 文档和预览租约采用明确 owner / RAII；
   Drop 只做可安全完成的收尾，可能阻塞的关闭放到显式 shutdown。
9. unsafe 必须说明线程、对象寿命、指针来源和 API 前置条件；逐步开启有价值的
   局部 lint，如 `unsafe_op_in_unsafe_fn`。不一次启用 pedantic 再大面积 allow。
10. visibility 尽可能 `pub(crate)` / 私有；用 From / TryFrom 做自然转换。
    public API 的 errors / panics / safety 文档与实际行为一致。
11. 先用普通函数和标准库，确有多实现才加 trait；共享控件按语义抽取，不建立
    “通用万能页面”或新的 UI DSL。已有生成器是唯一资源真源。

API 命名、转换、可预测性和错误约定参考
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)，
格式与工具链继续遵循仓库现有规范。

## 7. 全仓覆盖清单

每个区域登记“已审查 / 发现 / 处置 / 测试 / 延后理由”。可以审查后不改代码，
不能为证明覆盖率而制造格式变更。依赖与生成物在所属区域一起检查。

| 区域 | 必查事项 | 主要验证 |
| --- | --- | --- |
| GPUI shell / host / pages | 状态 owner、输入与焦点、原生回调、请求取消、保存反馈、局部重绘 | GPUI 行为 / 定序测试、双语物理矩阵 |
| Core context / action / search / ranking | registry 唯一性、版本复验、Hidden、稳定排序、临时分配 | 既有 Core / MVP 用例、查询和目录并发探针 |
| Application / frontend runtime | 锁顺序、Session、发布、执行票据、blocking 边界、关闭 | 可控顺序回归、订阅和 pending 数量 |
| 所有 first-party features | 注册贡献、业务边界、有界读取、取消、能力与错误降级 | 对应 feature / MVP 测试，文件完整遍历 |
| Persistence | 事务、损坏恢复、原子替换、限额、权限、SQLite 连接所有权 | 故障注入、超限、回滚和重开兼容 |
| anywhere-platform / macOS native | 主线程、FFI 安全、OS 身份、路径、进程 / watcher 寿命 | 平台单测、菜单 probe、跨平台编译、物理矩阵 |
| anywhere-protocol | 传输与内部状态分工、字段默认值、schema 和兼容 | serde round-trip、旧 fixture、生成漂移 |
| anywhere-plugin | manifest、资源与 ZIP 路径、digest / grant、部分失败原子性 | 包解析、安装和损坏包回归 |
| plugin-sdk / Web resources | transport、pending / timer 清理、state 比较、主题 / 语言、transfer | SDK 与 Release 示例 conformance |
| plugins/examples | 仅演示层业务，无绕过 SDK 的原生访问，示例可构建 | 模板解包、打包、权限拒绝测试 |
| scripts / resources / tools | 生成真源、重复配置、路径与清理安全、错误退出、依赖锁文件 | Python、格式、生成器 --check、Shell / TS 检查 |
| Cargo / CI / packaging / docs | 未用依赖、平台 feature、许可证、包内残留、文档入口 | 架构门禁、portable workflow、签名与动态库扫描 |

保留“旧格式迁移”和“历史文档”各自边界。不能搜索到 Flutter / Dart / bridge 就
删除：文件类型表中的 Dart 是受支持文件类型，Web bridge 是现役协议，
旧配置解析与归档也有明确用途。

## 8. 分阶段实现任务

执行顺序：S0 → S1 → S2 → S3 → S4 → S5 → S6 → S7。
S6 的只读审查可提前做；依赖修改保持顺序。每个任务独立可 review，不把全仓
格式化、移动和行为修改塞进一个提交。下列新文件名为建议，创建时同步代码地图。

### S0：冻结基线与测量入口

**T00：保护当前工作与建立覆盖账本。**

- 记录 branch、HEAD、`git status --short`，保存当前源码差异摘要及 untracked
  文件清单；不要只打 HEAD 包而漏掉现有未提交 UI 修复。
- 不 reset / checkout / stash / clean，不替换日常运行包，不按应用名批量停止进程。
- 在方案下登记第 7 节覆盖状态；实施日志只记录结论与证据，稳定规则回专题。
- 验收：可区分原有修改与本轮增量，原有配置 / Bundle / 用户数据不变。

**T01：建立自动化与行为基线。**

- 运行 `scripts/check-gpui.sh`、`scripts/check.sh`，记录实际测试数、ignored、
  平台和失败，不沿用本方案的 81 作为未来固定期望。
- 提取当前已有场景清单：同模式菜单恢复、跨模式、输入 / Scope、隐藏恢复、
  Settings 保存 / 放弃、Shelf、Tasks、Web、Quick Look、目录授权。
- 现有失败先分类为基线问题，不能删测试或放宽断言掩盖。
- 验收：日志可定位当前源码与环境；关键正常行为有可重复操作步骤。

**T02：补通信及 UI 投影测量。**

- 在现有 Core examples 之外新增合成 snapshot / GPUI 投影测量入口，不读取
  真实剪贴板或个人路径。构造 0、100、1,000、10,000 候选，以及较多 Context /
  result actions；文件追加使用 64 条分页和仅进度更新两种序列。
- 分别计时 Core 构建、发布、host 接收、gate、投影、布局提交；用关联
  session / revision / snapshot 计数串联，不把它们加成“显示器实际呈现”。
- 记录活动订阅 / 请求、峰值排队项 / 字节、取消完成、投影次数、resize 次数。
  分配次数使用独立基准工具 / 专用 feature，不把全局统计分配器带入发行包。
- 使用 Release、固定 fixture / seed、预热、多轮运行，输出 JSON 的 n、p50、
  p95、max、错误 / 取消数和源码差异标识。登记硬件、OS、语言、缩放、负载。
- 验收：同命令可重跑；观测本身不输出正文、路径、插件输入或凭据。

### S1：先修明确缺陷和失败语义

**T03：修复同模式菜单恢复导致订阅失效。**

- 在 `activate_from_entry` 增加定序测试：挂接 follow，模拟已显示的同模式
  `activate_from_menu`，再交付更高 seq；验证仍被接受。
- 只有需要切换会话 / 请求时推进相应 generation；纯 focus 路径不推进。
- 同时覆盖同热键隐藏、不同模式发新查询、Settings Wake 决策，防止修复过宽。
- 验收：新测试在修复前失败、修复后通过，不重采同模式来源应用。

**T04：封闭订阅取消缺口。**

- 先在现有 host follow 中加入独立关闭 / 取消等待，GPUI 保存任务 handle；
  后续 T07 替换转发链时复用此回归。
- 测试 UI 先关闭且 Core 永不发布下一份状态，旧订阅仍结束；连续 100 次查询
  和隐藏恢复不增加悬挂订阅。计时使用调度控制 / 信号，不靠实时间 sleep 碰运气。
- 验收：只允许当前 Session 的有效订阅存活；不会取消已开始的业务任务。

**T05：有界配置与错误显示。**

- 为 HostPreferences / BookmarkFile 定义字节、条目和字符串长度上限，数值
  根据现有最大合法 fixture / 迁移数据确定，登记在共享策略或有明确 owner 的常量。
- 使用 `Read::take(limit + 1)` 并检查超限，不能先读完整文件再检查长度。
- 区分 NotFound 默认值、Invalid / TooLarge、PermissionDenied / I/O。
  损坏配置进入可读错误 / 恢复入口，不回写空配置覆盖原内容。
- 保留旧 shortcut 解析和所有合法字段；书签损坏只关闭相应能力。
- 验收：超大稀疏文件、截断 JSON、旧格式、并发原子保存与权限失败均有回归。

### S2：建立有所有权的 host 请求边界

**T06：提取 runtime 模块，先不改变通信语义。**

- 已将原 `host/runtime.rs` facade 的 Launcher、Actions、Settings、Clipboard、
  Application History、Tasks、Web 请求方法移到同目录子模块；facade 保留原
  签名，DTO 转换留在对应领域。后续新增请求继续按领域落位。
- 为 Settings / 查询 / 对象操作定义 typed outcome，逐步替换万能 `LiveEvent`。
  单次结果改 oneshot，回复关闭映射为 Cancelled / Unavailable，不静默显示成功。
- 引入少量 `HostError` 类型与统一上下文，复用已存在的 RuntimeErrorKind；
  不建立第三套字符串错误目录，不一次迁移整个 Core 到新错误 crate。
- 验收：调用方无需全仓同时重写，编译、序列化和既有失败场景保持一致。

**T07：共享快照直接订阅。**

- 新增 session-aware shared subscription，保留 Arc 到 `LauncherModel`。
  删除这条产品链上的 Box 解包、owned clone 和无界状态转发。
- 一个 Session 一个状态订阅，query revision 只提高 gate 的 minimum；换 Session
  才换订阅。保存 / 执行结果仍有独立可靠单次 reply。
- 首份结果和 watch 同时到达只提交一次；先发布后订阅、恢复后 replay、同 revision
  迟到 seq、None / superseded 均覆盖。不要删除仍有 Core 调用方的 owned API。
- 审查 `LauncherEvent::ActionsUpdated` 与 watch 的双发布消费者，只有确认没有
  产品 / 测试 / 对外契约用途后才移除或改为轻量通知，不能直接删除 broadcast。
- 验收：快照从 Core 到 UI 保持共享身份；旧 session / revision / seq 不能提交；
  慢消费者只保留最新完整状态，执行结果、文件前缀和最终进度不丢。

**T08：请求准入与协作取消。**

- 先列出用户输入、图标、历史读取、保存、Web 请求的最大并发与 owner。
  使用已有 Semaphore / scope，必要时增局部 token；不为每个操作重复造取消框架。
- 旧查询未开始可撤销，正在读取检查取消，已冻结执行仍按执行票据验证。
  新输入允许替换未开始的旧输入，但 Enter / Copy / Save 不合并、不自动重试。
- 保持 Settings 写串行、图标既有并发上限和 Web 预算，避免单一全局队列互相阻塞。
- 验收：输入风暴与慢磁盘下，排队 / 内存 / worker 数有界；查询可继续响应，
  被拒绝请求有明确 Busy / Cancelled；旧读取完成不会写入新页面。

### S3：减少主线程工作与无效重绘

**T09：异步初始化与设置快照缓存。**

- 把 runtime / 数据库 / 包读取从 GPUI `run` 的同步热路径移出；识别必须主线程
  创建的原生对象，主线程只处理该部分，不整体搬走 AppKit 装配。
- 返回一次 InitialState / 只读缓存，避免每个 Shelf / Tasks 构造再次读 settings.json
  和剪贴板状态。Settings 打开时异步 refresh，不覆盖用户已经编辑的草稿。
- 启动期显示 / 热键行为明确为 Starting / Ready / Failed；事件按语义合并或拒绝，
  不在半初始化状态执行动作。可恢复错误有入口和日志，避免 UI 回调 panic。
- 来源应用必须在激活 / 抢焦点前冻结；不能异步化后读到 Anywhere 自己。
- 验收：用阻塞存储 fixture 保持 UI 事件可处理；首次和独立窗口打开不做同步磁盘读；
  真实热键、key / focus、来源身份另做物理验收。

当前实现：主窗口使用显式 `Preview / Starting / Ready / Failed` 状态；非 preview
启动由具名 `anywhere-startup` 线程打开 `LiveLauncher`，主线程只消费一次 Ready
快照。Starting / Failed 入口只显示状态，不预留 Session；状态栏先使用安全默认
标签，Ready 后用缓存偏好更新标签并注册热键。r5 隔离包已确认该切换进入真实
进程。阻塞存储 fixture 的自动化尚未加入，慢启动 UI 时延仍需专门测量。

**T10：增量投影与 UI 更新。**

- `LauncherModel::apply` 拆分接纳状态和展示重建；为同列表进度更新建立快速路径。
- 让变化描述控制 notify、输入同步、图标刷新和 resize；选中 ID 保持原规则。
- 缓存 key 包含第 4.5 节全部展示依赖；文件 append / replace / availability 更新
  分别测试，不能因版本号相同跳过新增行。
- 验收：100 次相同候选进度更新不发生 100 次完整行构造 / resize；locale、scope、
  Context 标签、对象动作及 selected 状态改变仍正确刷新。测量主线程 p95。

**T11：任务版本化读取和通知。**

- 先将 `output_revision` 保留到 GPUI，未变化且选中任务不变时不读 stdout / stderr。
- 再由 execution manager 发布有界最新摘要版本，状态 / 开始 / 完成 / 删除和输出
  都能唤醒订阅。Core 的 stdout drain 独立线程模型先保留，不顺带重写进程 I/O。
- 输出突发可合并通知，最终状态必须发出。UI 按版本获取有界输出快照；只有测量
  证明必要才做增量字节协议，必须处理 rolling tail、UTF-8 和截断标记。
- 验收：完成 / Stop 无需等 1.5 秒轮询；未变输出零重复读取；大量输出仍有界，
  关闭 Tasks 不结束进程，重开可读取完整保留尾部。

### S4：拆分 UI 职责，保持现有外观

**T12：Session、导航、输入与窗口分离。**

- 从 LauncherView 提取 SessionUiState、NavigationState 和请求 scope，再移动
  activation / retention / poll / handle_key。先保留根 Entity 和外部入口。
- 输入路由顺序固定：IME / recorder → 当前页 → Launcher 导航；不让通用
  shortcut handler 越过 Web 输入或组合输入语义。
- 将 Shell 中主窗口、Shelf / Tasks 的装配和关闭处理按 owner 分开；根不再用
  一组 standalone bool 隐式决定所有业务路径，迁移成明确窗口角色。
- 验收：同模式恢复、跨模式、15 秒到期、Esc、Cmd+,、输入法优先、独立窗口
  和延迟 resize 原有回归全部通过。

当前实现：`view/navigation.rs` 持有 activation、retention、resume、Settings
入口与隐藏；`view/input.rs` 独占按键 / shortcut / recorder / 页面导航顺序；
Settings 的草稿、动作上下文编辑、保存与关闭意图位于
`view/settings/controller.rs`。主窗口、Shelf 与 Tasks 的独立窗口 owner 仍集中在
`app.rs` 的 ProductShell，下一阶段再按窗口角色拆分。

**T13：页面状态和页面视图逐个迁移。**

- 顺序：Clipboard → Application History → Tasks → Web surface → Settings。
  每次只迁移一个页面，包含状态、请求票据、输入、enter / leave 与 cleanup。
- 根只负责路由和跨页意图；页面通过窄事件请求打开预览、执行或返回，不能任意
  修改 sibling 页面、Core 策略和整个 LauncherView。
- Settings 按 General / Shortcuts / Context / Search / Clipboard / App History /
  Features 渲染模块拆开；草稿、保存和退出决策是一个独立控制器，不能七份复制。
- 验收：页面迁出后根不保留其专属字段；逐页测试和双语截图无功能 / 布局回退。

**T14：共享组件与列表一致性。**

- 只抽取已经稳定重复的 SearchHeader、Notice、Keycap、CandidateRow、设置行 / 卡片。
  保留页面自己的高度、操作顺序、菜单和搜索语义，不强迫套一个万能 shell。
- 清理重复颜色 / 字号到 design tokens，重复文案到 ARB；运行生成器更新投影。
- 审查 tooltip、截断、辅助功能名称、focus-visible、hover / selected / disabled。
  已有 Root 透明渲染测试、180.5 px 菜单、长中文边界测试继续存在。
- 验收：布局不挤压，控件边缘与材质一致；英文 / 中文、亮暗桌面、Reduce
  Transparency / Increase Contrast 通过指定物理项。

### S5：原生窗口与资源安全整理

**T15：拆分 AppKit 边界。**

- 将平台 facade、窗口配置 / 定位、激活、状态菜单、通知 owner 分开，保留已独立
  的 material / preview。用显式窗口身份查找，不依赖任意第一个同名窗口。
- 为 observer token、event monitor、状态项、延迟激活回调记录和落实资源 owner。
  不在持有 GPUI 可变借用时同步触发 AppKit resize / move 重入。
- 逐个检查 unsafe 前置条件、main-thread marker、retain / release 和闭包捕获；
  不为“减少 unsafe 行数”改成未证明安全的指针转换。
- 验收：反复打开 / 关闭辅助窗口无累积 observer；后台身份、无 Dock、菜单点击、
  key / focus、屏幕锚点、Quick Look 与材质矩阵单独复验。

**T16：统一显式关闭与失败观测。**

- 区分 harmless stale callback 与真实 close / release / save 失败；前者可 debug，
  后者带操作和身份记录。不能把每次过期 update 都输出成 error。
- 关键租约先显式异步关闭，Drop 为兜底；App 退出先撤销请求和文档、再处理
  worker / runtime。不能把 Tokio Runtime 最后一个 owner 的潜在等待藏进 UI 回调。
- 验收：Web / preview 延迟关闭不影响新对象；退出无新副作用，后台 worker
  不因未开始的订阅而被长期保留；失败可定位且日志不泄露正文。

当前实现：`LiveLauncher` 持有可显式取走的 Tokio runtime；最后 owner 释放时使用
`shutdown_background`，避免退出路径等待 Core / helper。缺省 Drop 仍作为兜底，
应用退出前的完整文档 / worker 撤销顺序继续按 T16 验收。

### S6：Core、存储、协议、SDK 与工具逐域整理

**T17：发布与执行的职责拆分。**

- 按第 5 节整理 Core 热点；抽出纯参数准备 / 结果构造，不改变锁内复验和提交
  顺序。检查 snapshot / registry / catalog / inputs_epoch 一致性。
- execution manager 的 record 锁失败明确传播或标记不可用，不返回“没有任务”
  伪装成功。将可能阻塞的 join 放到管理锁外，审查失败进程创建与尾部回收。
- 验收：Context / Usage / catalog / index 并发发布、队列执行前隐藏、菜单定位
  过期与已有后台进程寿命的定序回归全部保留。

**T18：存储和 Feature 服务审查。**

- 逐个完成第 7 节 feature 覆盖，重复策略集中到对应 owner，不移入 UI。
- 核对配置的有界读、原子替换、fsync、并发冲突、损坏恢复与 SQLite 事务。
  仅抽取权限 / 恢复语义相同的工具；不把 Core 私有 config helper 粗暴公开给
  host，不为统一读写引入一个跨领域通用 repository 框架。
- 数据库查询、应用目录缓存、文件排序和 Unicode 分配只优化已测热点；
  不改历史语义、限额、稳定顺序或把持久连接无条件改连接池。
- 验收：旧数据重开与迁移、保存中断、超限拒绝、文件取消 / 清理、历史来源失败
  和不同平台路径分类保持正确。

**T19：Protocol 与旧兼容代码。**

- 建立所有导出类型 / 字段的实际消费者表，分清原生状态、Web wire、配置迁移。
  删除无调用且不属于兼容契约的 API；外部契约先标记再分版本移除。
- 保持 serde 默认值、字段名、enum 编码、SDK schema 与 Feature catalog。
  协议生成器只改源，使用临时生成比对验证同步。
- 验收：旧 fixture round-trip、未知 / 缺省字段和 SDK conformance 通过；
  无因“减少 DTO”意外改变的公开 JSON。

**T20：Web 入站队列和回复编码。**

- 审查 WebView callback → host queue → broker 各层的字节 / 并发预算，在最早
  可安全拒绝的位置限制排队。同步 callback 使用非阻塞 admission；超限通过
  现有错误协议明确失败，不能仅靠 SDK 正常行为防止插件直接洪泛 IPC。
- 分别测量 1 KiB、64 KiB 与协议允许最大单帧的十进制源码、JSON 字符串
  UTF-8 encode 等可实现路径；大文本继续走既有 transfer / chunk / ack。
- 原生 binary API 可用性以锁定版本和实测为准；若没有收益或需改 SDK，保留
  现有 wire，不为“二进制”换格式。始终不使用未转义正文拼 JS。
- 验收：Unicode / 代理对 / 换行 / U+2028 / U+2029、超帧、无效输入、来源
  错误、旧 handle、乱序回复、取消和 chunk conformance 全部通过。

**T21：SDK、示例、脚本与依赖清理。**

- 检查 SDK client 中 transport、pending、state、transfer 的职责与 timer /
  abort listener 清理；先保行为再拆模块，保留 unsubscribe / dispose 幂等。
- 检查所有脚本路径、临时目录与清理、子进程错误退出、生成物真源和重复限额。
  不因清理旧产品删除原子路径校验、模板检查或发行许可证。
- 对每个依赖查 Rust 调用、宏、cfg、build.rs、原生链接、测试与许可证用途；
  自动 unused 工具仅作候选列表，不凭静态扫描删平台依赖。
- 验收：SDK / 模板 / 示例可打包、脚本单测通过、lockfile 无无意变更；
  `_window_bounds_contract` 等确认无用途后删除，native 类型探针先检查集成用途。

### S7：回归、性能决策与交付

**T22：完整自动化与兼容门禁。**

```sh
scripts/check-gpui.sh
scripts/check.sh
git diff --check
```

完整脚本已包含 fmt、workspace test、Clippy、生成物、SDK、示例和架构检查，
不在同一未变源码上反复重复整套。按 portable workflow 在 Linux / Windows
检查共享 crates；远端未运行时明确写 Pending，不能用本机 macOS 替代。

最终 `scripts/check.sh` 在 2026-11-13 以 exit 0 完成：GPUI 89、workspace 771
passed / 6 ignored、573 条双语消息、Web SDK 50、JSON 示例 30、插件模板 /
QuickJS / TypeScript 后台 SDK、文档 100 files / 672 links、架构边界和严格
Clippy 全部通过；`cargo fmt --all -- --check` 与 `git diff --check` 同轮通过。
Linux / Windows 远端矩阵未在本机执行，不能由 macOS 结果替代。

**T23：性能 A/B 和资源矩阵。**

- 同机器、同配置、同 fixture、同编译参数比较 T02 基线与最终实现，多轮交错
  运行；报告 n / p50 / p95 / max 和变异，不只选最快的一次。
- 结构验收：状态链无额外 host 无界队列；关闭旧订阅不需要新状态唤醒；
  请求有并发上限；未变任务输出不重读；仅进度更新不全量投影或 resize。
- 性能门槛在 T02 数据出来后锁定；默认审查规则为稳定复现的 p95 回退超过
  10% 必须解释或回退该优化，接近噪声的微秒指标同时看绝对差值和方差。
  本方案不预先承诺固定倍数提升或内存降到某个数字。
- 隐藏空闲、100 次唤醒、15 秒到期、文件查询替换、Tasks 输出、Web 打开关闭
  采集主进程 / 所属 helper footprint、活动任务与唤醒趋势。
- 若 Arc 延长保留导致内存恶化、事件合并增加交互延迟或快路径漏更新，恢复
  较简单实现；不因为“用了零拷贝”就保留回退。

**T24：发布包和物理 UI 验收。**

```sh
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-refactor-acceptance.app" scripts/build-macos.sh
codesign --verify --deep --strict build/Anywhere-refactor-acceptance.app
plutil -lint build/Anywhere-refactor-acceptance.app/Contents/Info.plist
otool -L build/Anywhere-refactor-acceptance.app/Contents/MacOS/Anywhere
```

- 使用唯一 Bundle ID、独立数据目录、冲突检查后的热键；保持日常应用运行。
  真实业务验收不设置 `ANYWHERE_PREVIEW=1`。
- 物理矩阵：隐藏启动、菜单与热键、重复同模式恢复、跨模式、中文 IME、长中英
  文本、模式菜单、行内操作、Settings 草稿、Shelf、Tasks、Quick Look、Web。
- 材质矩阵：macOS 26+ Glass、旧系统 HUD、不同桌面、Reduce Transparency、
  Increase Contrast；GUI 截图和 key / focus 不能由纯状态断言代替。
- 原 Flutter 外观参照固定提交 `81c523c`；本轮修复后的 GPUI 截图作为回归补充。
  未完成平台、VoiceOver 或系统权限项目明确保留 Pending。
- 扫描包内 Flutter / Dart runtime 残留、许可证与动态库；关键词命中按用途判断。
  更新 architecture / performance / MVP 和相关主题文档，归档阶段性审查记录。

## 9. 每个提交的实施模板

```text
任务 ID / 前置任务：
具体问题与触发序列：
当前调用链与应保持的契约：
本次修改的 owner / 文件：
新增或复用的关键回归：
失败 / 取消 / 回滚行为：
自动化命令和实际结果：
物理证据 / 未验证项：
性能数据或本次不涉及性能的理由：
```

推荐提交边界：一个明确 Bug、一个页面 / 模块迁移、一段通信链路或一种可测
优化。模块搬迁与行为修复分开，以便 review 和只回退本轮改动。

## 10. 完成定义与首批执行顺序

完成本方案需要同时满足：

- 第 7 节全部区域有审查结论；所有发现都有修复或明确保留理由，不能只整理 GPUI。
- F01 定序复现后修复，F02–F03 生命周期与主线程边界满足验收；风险项以测量 /
  复现结论关闭，不能把未复现的猜测写成“已修复 Bug”。
- 原生通信使用共享最新状态、typed 单次结果和有界准入，权限 / 顺序 / 执行
  票据保持原义；所有优化有 A/B 或明确的结构性验收证据。
- LauncherView 不再拥有所有工具页字段；Core、Feature、Persistence 和 Platform
  边界通过门禁。只有移动文件而不减少职责耦合不算完成。
- 自动化、兼容、打包和要求的物理矩阵分别记录；未完成项明确列出。

首批建议依次执行 T00–T05，再执行 T06–T10。它们先解决监听正确性、取消和
主线程 / 通信热路径，收益与风险最容易验证。之后 T11–T16 逐页收敛，再完成
T17–T24；不先进行一次全仓大规模重命名或框架替换。
