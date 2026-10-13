# V0.7 代码与架构 Review

日期：2026-10-12。基线：`dev-0.7` 当前完整工作区，版本 `0.7.0+19`，包括未提交与未跟踪的实现。没有以 HEAD 或单次 diff 代替当前代码。

> 本文前半保留 build 19 的审查证据与当时行号；当前修复状态以文末 build 20 记录为准。

## 结论

现有五个 crate 与 Rust / Flutter / Native Host 的基本分工可以继续使用。Context → Action → Candidate → Ranking → Execution 主流程、动作目录版本、菜单执行凭证和后台任务生命周期已有清晰契约与回归测试。没有发现需要整体推倒重写的依据。

需要先处理权限配置失效、会话关闭后的数据保留、图标加载并发上限；随后优化大量文件结果的加载与快照构造。面向 V0.8，主要结构问题是领域能力进入通用搜索契约，以及 Launcher 导航仍靠多个可空字段组合表达。

以下区分已复现缺陷、代码直接显示的性能风险、后续扩展建议。测试通过不等于这些额外时序已经被覆盖。

## 1. [P1 / 已复现] 旧授权回调可以覆盖更新后的文件搜索设置

位置：

- [FileSearchAccess.swift](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/macos/Runner/FileSearchAccess.swift:65)：正在授权时先返回空目录，连 `enabled == false` 的撤销分支也不会执行。
- [同文件旧回调](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/macos/Runner/FileSearchAccess.swift:82)：捕获旧 scope，完成时没有检查设置版本，继续写 bookmark、恢复 lease 和发布 ready。
- [Dart 授权适配](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/infrastructure/file_search_access.dart:42)：使用调用开始时的 settings 发布 Core 配置，没有版本复验。
- [SettingsController](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/settings/settings_controller.dart:300)：授权与普通 save 没有共用队列；还直接调用静态服务，绕过构造器注入的 `_applyFileSearch`。

复现顺序：开始旧目录授权 → 新请求关闭搜索或更换范围 → 新请求完成 → 旧授权完成。Swift 受控测试确认新 disable 返回空目录后，旧回调仍恢复旧根且 `ready == true`；Dart 受控测试确认最终 Core write 为 `enabled=true` 与旧根，覆盖已完成的 disable。

影响：保存的设置、原生授权状态和实际 Core 搜索范围可能不一致。这里复现的是公开异步服务的调用顺序，没有声称已通过真实 NSOpenPanel 手动操作复现该竞态。

最小修复：为配置准备增加递增 generation；关闭/更改范围立即使旧请求失效。Native 回调和 Dart 最终 write 都校验 generation；save 与 authorize 通过同一个可注入协调器。不能仅靠禁用一个按钮保证正确性，也不能让撤销排队等待用户关闭旧授权窗口。

补测：授权未完成时关闭、缩小范围、两次请求逆序返回、dispose 后回调；断言旧请求无 bookmark/lease/ready/Core 配置副作用。

## 2. [P2 / 已复现] 关闭 Launcher 并未释放完整的会话结果

位置：[deactivate](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/launcher_service.rs:554)、[invalidate_execution](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/session_model.rs:108)。

关闭会话会撤销执行资格、清空部分容器和临时搜索工作区；但 `session.state.candidates`、`state.result_actions`、输入、已提交 Context 和 activation captures 仍留在应用长期持有的 Session 中。被 clear 的 Vec / HashMap 还保留容量。下一次 `begin_activation` 才整体替换 Session。

直接调用生产 `invalidate_execution` 的受控证据：预置 8,192 个结果与 24,576 个次级动作展示项后，调用结束仍保留全部这些展示项；search_candidates 的 len 为 0，capacity 仍为 8,192。这证明对象保留，没有测量或推断同等字节数的 RSS，也不是无限累计泄漏。

影响：大量浏览后的内存开销和采集内容在窗口关闭后仍驻留，直到下一次激活或进程退出。与当前“退出搜索释放会话结果”的文档描述不完全一致。

最小修复：区分“执行资格失效”和“会话结束”。结束时替换会话临时数据及其容器，保留递增 SessionId 所需的身份信息；后台 ExecutionSessionManager、应用目录与必要缓存保留各自生命周期。

补测：大量结果关闭后 Session 临时字段及容量释放；迟到 worker 不重新填充；已有后台任务继续运行；重新唤醒 SessionId 单调增加。

## 3. [P2 / 已复现] 图标加载的真实并发不受跨会话上限约束

位置：[缓存清空](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/infrastructure/application_icon_cache.dart:40)、[旧响应分支](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/infrastructure/application_icon_cache.dart:93)、[dismiss 调用点](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/launcher_controller.dart:511)。

`clear()` 将 `_active` 设为 0，但已发出的 load 没有取消。新会话立即再发出 4 个；旧请求完成时因 epoch 不同直接返回，没有进行统一的物理请求计数。

Completer 控制的测试确认：4 个未完成请求 → clear → 新会话 4 个请求，真实峰值为 **8**。快速重复该过程可能继续叠加。现有 epoch 能阻止旧图片污染新会话，但不能限制仍在运行的工作量。

最小修复：将全生命周期 in-flight 计数与当前 epoch 的 pending 状态分开；实际请求结束都在 finally 释放名额，旧响应不发布缓存。若要兼顾新会话响应，需在原生加载端提供取消或独立、有界的 admission，不能把计数清零当成取消。

补测：反复 clear、旧请求逆序完成、失败与 dispose，断言物理峰值始终在预算内，新请求最终能开始且没有计数下溢。

## 4. [P2 / 算法与实测] 连续加载大量命中时存在重复全量工作

位置：

- [ScanStore.more](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/features/filesystem/scan.rs:584)：每追加最多 64 项，重新读取完整 matches 文件，并重新排名未展示项。
- [snapshot 输入复制](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/session_runtime.rs:442)：复制所有已加载候选。
- [snapshot 构造](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/snapshot.rs:125)：文件列表继续 clone。
- [次级动作构造](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/features/filesystem/search.rs:420)：对每个已加载文件重新进行动作搜索，而非仅对当前选中项构造。
- [执行表构造](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/session_runtime.rs:578)：每次重新复制候选与次级动作。

扫描结束后，取完 N 个命中的 matches 读取量约为 `N × ceil(N/64)`；整个连续加载过程趋于平方增长。快照构造及传输又反复处理增长中的已加载列表。遍历器本身的有界 RAM 不等于整条 UI / 执行链路内存有界。

本机 release 受控 fixture，使用生产 `more`，完整取回且去重校验通过：

| 命中数 | 批次数 | 全部 more 累计耗时 |
|---:|---:|---:|
| 2,048 | 32 | 20.9 ms |
| 8,192 | 128 | 309.5 ms |

命中增加 4 倍，累计耗时约增加 14.8 倍。此测量只含 Rust 记录读取/排名，不含遍历、候选动作构造、FRB 传输和 GUI；不能据此宣称现在的 GUI 必然卡顿，或外推固定的十万条耗时。

建议：保留连续列表体验和查询临时存储。扫描期间按新增记录维护有界排序段，完成后按需归并；次级动作针对当前选中对象延迟生成，执行目标共享不可变描述。之后引入结果增量更新和可回读的窗口缓存，保证早期结果仍可导航与执行。不需要恢复用户分页或新建长期文件索引。

补测：大量同名/短关键词命中，连续滚动与方向键、晚到高排名项、历史行执行；同时记录记录读取量、单批延迟、快照/传输量及 App RSS。现有诊断 benchmark 主要测旧 cursor `page`，不能替代当前连续 `more` 的性能验收。

## 5. [P2 / 代码风险，未实机复现] 原生授权准备在主线程做同步磁盘工作

位置：[MethodChannel 分派](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/macos/Runner/Main.swift:157)、[bookmark 恢复与 opendir](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/macos/Runner/FileSearchAccess.swift:36)、[bookmark 原子写入](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/macos/Runner/FileSearchAccess.swift:100)。

channel handler 直接调用 prepare；路径解析、bookmark 恢复、根目录 opendir、保存没有工作队列或超时边界。网络目录、离线卷、慢存储发生阻塞时会直接拖住 Native UI。当前只做根校验，正常本地目录成本可能很低；本轮没有做离线卷/网络卷实测。

建议：AppKit panel 与 UI 状态发布保持主线程，磁盘检查/保存放到串行 worker；配合第 1 项的 generation 丢弃过期结果。超时不能假装取消了不可中断 I/O，也不能释放 worker 名额后无限启动新任务。仅证明根可读并不证明所有子目录均有 TCC 权限；现有授权文档已说明该系统边界。

## 结构改进建议

这些不是已复现的功能故障，应随实际扩展逐步落实。

### A. Launcher 导航需要显式状态，而不是继续增加可空字段

[LauncherController](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/launcher_controller.dart:165) 同时保存 inputMode、navigationMode、scopeId、actionPath、bindingChoices、fileActionTarget/title/parent、父页面与输出状态。文件搜索进入/退出、Esc、快捷键和结果展示需要共同维护这些字段；view 缓存还人工枚举二十余项输入。文件约 1,500 行不是根因，状态组合与转换分散才是维护成本。

建议用 sealed route/state 表达根页面、文件工具、菜单浏览、对象 Actions、结果页；每种状态持有自己的 payload 和返回信息。Context、Scope 的现有产品语义不改变。提取导航转换与文件工作区协调，LauncherController 保留会话/请求/窗口协调。先用现有唤醒、预填、Esc、父页面恢复和快捷键回归锁定行为，再移动职责；不单纯按行数拆文件。

### B. 通用异步搜索接口已经携带文件领域协议

[AsyncSearchRequest / AsyncCandidateBatch](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/core/search/candidate_source.rs:44) 直接包含 FileSearchFilter 与 FileSearchProgress；[Application 协调](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/search_sources.rs:186) 顺序跑来源，并以 `source.id() == "files"` 分支处理。

当前产品仅文件来源使用该慢查询路径，因此不能判定当前多来源 UI 已经阻塞。但新 Browser History / Clipboard / 插件来源会要求继续增加领域字段；一个长期来源也会阻止后续来源启动。

V0.8 应将通用查询凭证、SourceId、批次语义、取消/超时与领域请求分开。文件工具负责自己的过滤和进度适配；来源分别持有结果，明确 replace/append 与去重契约；独立运行并在总 admission 预算下限流。通过两个真实来源验证，避免先造通用动态消息框架。

### C. 第一方功能注册尚未形成统一贡献边界

[features/mod.rs](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/features/mod.rs:25) 注册 Bundle / Provider，[composition](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/composition.rs:8) 另外注册动态工厂，[Application 构造器](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/launcher_service.rs:101) 另外装配文件来源，而 [Core group](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/core/action/group.rs:21) 仍维护 Git/Maven/Node 等产品目录。

当前集中装配容易理解，五个 crate 不需要再分裂；但一个完整功能的贡献跨越多个入口。建议 V0.8 用 Git / Node 等已验证功能提供统一 FeatureContribution（Provider、Bundle、Factory、Group、Source）。Core 管注册验证；业务模块持有自己的展示/能力声明。内部静态注册与外部 Extension v2 的版本、能力授权、宿主 API 是不同契约，不能把 Rust trait 直接当 ABI。

### D. 文件类型规则已有重复与差异

[Dart FileFilter](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/file_search_controller.dart:7) 和 [Rust FilterName](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/features/filesystem/scan.rs:73) 各维护一份扩展名/特殊文件名。Rust Code 包含 conf、cfg、hs、sc、bat、cmd、fish、lock，Dart 对应集合不全。

实际生产有 file_search progress 时信任 Core 过滤；Dart 集合只用于没有 progress 的 fallback，因此不是所有当前 Code 搜索都会遗漏这些文件。问题是 fallback 与权威来源结果不一致，并且增加类型要改两处。

建议保留 Rust 为过滤唯一来源；若确实需要旧来源 fallback，用一份资源生成两端规则，如现有 hotkey-keymap 的方式。图标分类可独立，因为它是展示策略，不应反向决定搜索资格。

### E. 原生 Host 与桥接层应继续保持边界清晰

SettingsStore / HotKeyService / LauncherWindowCoordinator / TasksWindowController 的拆分有效；FileSearchAccess 的原生 bookmark 所有权也合理。Flutter 与 Rust 使用 capability 和 DTO，未来 Windows 适配不需要重写全部页面。但主线程 I/O、静态服务绕过 DI 说明新功能仍容易跨过已有协调边界。

FRB 生成文件大不是手写代码可读性缺陷；手写 DTO 与转换则应随模块按查询、设置、执行、快捷键分组，保留一个薄公开门面。跨语言枚举/版本需要兼容测试，不能仅靠三端字段名称相同。Windows 的实际权限、热键、文件索引、路径编码仍必须单独实现和验收，当前 macOS gate 不能证明其正确性。

## 各维度判断

| 维度 | 当前判断 | 优先动作 |
|---|---|---|
| 架构 | 基本分层成立；新增文件领域协议侵入通用搜索 | 保留 crate 边界；分离 Source 生命周期与文件工具协议 |
| 模块划分 | Settings / Host / Execution 已拆；Launcher 导航仍集中 | 显式 route/state；统一贡献注册 |
| 性能与内存 | 遍历、输出与图片有预算；真实 in-flight 和结果生命周期有缺口 | 修第 2、3 项；测量并优化第 4、5 项 |
| 代码复用 | 常用文件动作复用 Registry/Usage，有效 | 选中项延迟构造 Actions；消除类型规则双份维护 |
| 可读性 | 核心用例/契约注释较清楚；大量条件状态增加理解成本 | 每个状态/操作一个责任，区分授权与设置应用 |
| 可扩展性 | 支持 Provider / Bundle / Factory / Source，尚未统一装配 | V0.8 用真实第一方模块验证贡献与外部协议 |
| 正确性与测试 | 版本失效、菜单歧义、执行复验已有覆盖 | 加并发授权、跨会话并发与关闭资源释放用例 |
| 跨平台 | 共享 Core/UI 方向正确，OS adapter 仍是必要边界 | 新平台逐项 capability 实现；不把 macOS 通过当跨平台验收 |

## 建议实施顺序

1. 先修授权 generation 与统一协调，封住旧回调覆盖设置。
2. 修会话结束释放与图标物理并发，保留现有 UI/快捷键/Scope。
3. 优化选中对象 Actions、重复快照复制与连续读取；补实际 App 的性能采样，再决定是否需要排序段/窗口缓存。
4. 提取 Launcher route/state 与授权 worker；保持所有现有交互回归通过。
5. V0.8 再统一 FeatureContribution / Source 契约与 Extension v2；V0.9 引入真实插件视图，V0.10 扩展现有 ExecutionSession 为 PTY/字节流，而不是提前添加未使用框架。

## 本轮验证边界与证据

- 重新运行 `scripts/check.sh`：**340 项 Rust、186 项 Flutter** 通过；6 项 Rust 测试默认忽略。原生 Host/menu 检查、fmt、Clippy、Flutter analyze、生成键码校验、架构检查通过。Core 可达依赖 107 个，未引入 UI/Flutter bridge 依赖。
- 对手写 Core / Application / features / platform / plugin / persistence、桥接门面、Flutter 导航/设置/任务/缓存、Native Host 的关键职责和生命周期做静态审查；生成文件以构建和分析验收，没有逐行人工审查生成代码。Review 无法证明所有路径不存在问题。
- Swift 真实临时目录/bookmark + 注入授权回调确认第 1 项 Native 部分；Dart fake channel + 注入 Core write 确认最终配置覆盖。图标 Completer 确认第 3 项。
- 第 2 项使用生产 Session 模型方法的独立 witness；第 4 项使用生产 ScanStore.more 的 release fixture。证据测试断言问题存在并通过，**不等于修复回归通过**。
- 检查日志：[完整 gate](/Users/luckincoffee/Documents/Github/Anywhere/build/validation/review-2026-10-12-check.log)、[权限 Dart witness](/Users/luckincoffee/Documents/Github/Anywhere/build/validation/review-access-dart-race.log)、[图标 witness](/Users/luckincoffee/Documents/Github/Anywhere/build/validation/review-icon-race.log)、[Session witness](/Users/luckincoffee/Documents/Github/Anywhere/build/validation/review-session-probe.log)、[连续加载基准](/Users/luckincoffee/Documents/Github/Anywhere/build/validation/review-search-probe.log)。辅助源码保留在 build/validation；临时 Rust example 已移出产品源码。
- 未运行新的真实 GUI、TCC/FDA 撤销矩阵、离线/网络卷、Windows，未测整 App RSS 或输入帧率。本轮没有修复产品实现、重新打包、提交或推送。

## build 20 修复记录（2026-10-12）

| 审查项 | 当前实现与回归 |
|---|---|
| 1. 旧授权覆盖新设置 | Dart 所有 Core 配置写入串行且复验 generation；Native worker / panel 回调校验 epoch；Settings 使用注入的授权端口并隔离旧状态回读。关闭、改范围、逆序完成与已排队旧工作有回归。 |
| 2. 会话数据无限保留 | 按新规则隐藏保留 15 秒，Core + Flutter 各有期限；隐藏立即暂停执行，到期整体替换 Session 释放内容和容量。14 秒恢复、到期清理、旧期限无效、旧排队执行不复活、设置草稿保留均有回归。 |
| 3. 图标物理并发叠加 | 实际 in-flight 计数独立于 epoch；clear 不取消计数，旧请求完成也释放名额。重复 clear / dispose 受控测试峰值不超过 4。 |
| 4. 大量连续结果重复工作 | 新增查询内外部排序与有序连续读取，增量记录仅与未读尾部归并；次级动作选中时生成，描述/Context 用 Arc，共用执行复验。多级归并、晚到项、旧行执行和读取失败重试有回归。 |
| 5. 授权主线程磁盘 I/O | AppKit 留主线程，书签解析、读取、保存和根检查放串行 worker；5 秒 watchdog 隔离旧发布，不假称已中断阻塞 OS I/O。 |
| A. 导航组合状态 | sealed LauncherOverlay 与具名父页面状态替代互斥可空字段；保留原 Context / Scope / Esc 行为。 |
| B. 文件协议侵入通用来源 | 通用可类型复验 payload、Replace / Append 与按来源结果所有权；各来源独立运行，受控慢来源与另一来源并发、清空隔离测试通过。 |
| C. 第一方多入口注册 | FeatureContribution 汇总 Bundle / Provider / Factory / Source，Git / Node 契约及贡献唯一性测试通过；产品 Group 元数据移入 features。 |
| D. 类型规则两份维护 | 一份 file-types.json 生成 Rust / Dart，在 check.sh 校验；图标保持独立展示策略。 |
| E. Host 边界 | 权限服务通过可注入端口协调，Native 仅 OS 服务。Bridge 保持公开门面；外部插件协议和 Windows 适配仍按路线图单独验收。 |

生产 more 的同机 release 受控采样：2,048 项累计 **20.9 → 5.1 ms**；8,192 项 **309.5 → 19.9 ms**，完整集合比对通过。这里只是 Rust 记录读取 / 排序，不能外推 App RSS 或帧率。列表仍累计持有已加载候选，FRB 快照仍为累计列表；增量传输和窗口缓存未加入，应在真实大列表测量后决定。

最终验证：完整 `scripts/check.sh` 通过 **346 项 Rust、189 项 Flutter**，6 项 Rust ignored；补充设置草稿超时回归后 Flutter analyze 与全部 **190 项 Flutter** 再次通过。Native Host / 授权 / 材质 / 焦点、格式、Clippy、生成键码/类型规则和架构 gate 通过。实机 build 20 已验证文件查询输入、Home 连续结果和按选中项打开二级 Actions；15 秒精确时序由自动化验证，没有把桌面注入组合键当作物理键盘验收。

证据：`build/validation/repair-final-check.log`、`repair-flutter-final-tests.log`、`repair-settings-retention.log`、`repair-search-probe.log`、`repair-build20.log`。App / ZIP / SHA-256 已生成；未提交或推送。

仍需未来实际需求驱动的工作：外部 Extension 版本/能力/UI 协议、Windows 系统服务、PTY/流式 Terminal。完整隐私授予/撤销、网络卷与冷盘矩阵未在本轮执行，不将测试通过描述为所有环境零故障。
