# Flutter / Rust 职责边界与后续重构 Review

日期：2026-10-12。基线：`dev-0.7` 当前完整工作区、build 20 实现及原生失焦回归，包括尚未提交的文件。审查以现行代码为准，不复用上一轮“已修复”作为正确性证明。

## 后续修复状态

本次要求的修复与重构已实施于 build 21；逐项对应、测试和性能证据见 [Flutter / Rust 边界重构收尾](refactor-boundaries-build21.md)。下面保留 build 20 的原始审查和复现记录，不作为新版仍存在这些问题的结论。

## 原始审查结论

分层总体正确：Flutter 没有自己识别 URL / JSON / Git 项目、执行系统命令或写 Usage；Rust 不依赖 Flutter Widget、颜色、像素或键盘焦点。当前无需更换框架或重新拆 crate。

边界仍有遗漏。Flutter 会从 ActionId / commandId 推断菜单或对象类型，Scope 过滤也同时存在于两端。Rust 的异步来源接口已经通用化，但应用协调仍以 files 作为特殊领域分支；不能据此声称所有未来 Feature 都已可无修改主流程接入。

本轮确认两项可复现功能问题，另列结构和性能建议。**原始审查阶段只做受控验证；后续修复见上方记录。**

## 1. [P2 / 已复现] 授权后的旧设置回读可以覆盖新保存的 UI 状态

位置：[SettingsController](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/settings/settings_controller.dart:315)。

`authorizeFileSearch` 在授权方法返回后检查配置 generation，但随后直接 `_current = await readSettings()`，没有在这次 await 返回后复验；`readSettingsStatus` 返回后也直接更新 warning/status。关闭或新 save 可以发生在回读期间。Search 页面允许授权期间关闭搜索或保存范围，只有 Authorize 按钮自身禁用，并未封住这个顺序。

受控 MethodChannel 顺序：授权完成 → 旧 readSettings 开始并挂起 → 保存新目录 / enabled=false 完成 → 旧回读返回 enabled=true 和旧目录。断言确认 Host 保存仍是新配置，而 `SettingsController.current` 重新变为旧配置。

影响是 UI / 后续编辑的配置基线过期；这条 witness **没有证明 Rust Core 权限被旧回调重新启用**。已有 FileSearchAccess generation/串行写入封住的是 Core 配置写入，不能替代设置回读隔离。

最小修复：先把所有回读结果放到局部变量，每次 await 后复验 disposed / 配置版本，最后一次性提交 current、warning、恢复状态。让保存与授权协调属于 SettingsController 的单一事务端口；Widget 可以排队保存草稿，但业务时序不应仅依赖 Widget 约束。

补测：旧设置回读迟到、旧 status 回读迟到、dispose 后返回；新保存配置及其状态必须不变。

## 2. [P2 / 已复现] Flutter 猜测 ActionId 的命名空间会隐藏合法插件动作

位置：[isMenu](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/launcher_view_state.dart:137)，[功能 Scope 展示](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/launcher_view_state.dart:259)。

Flutter 将 `menuPath 非空 OR commandId startsWith(app.)` 当成菜单。插件校验只保留 core 命名空间；插件 `id=app` 可以合法注册 `app.search`，其 menu_path 为空且 owning_app 为 None。Rust 受控运行确认解析和注册均成功。Dart witness 使用同类 app.* 普通候选，确认它被 isMenu 判为 true：功能 Scope 空查询时真实 Action 不在 visibleCandidates 中，取而代之的是 Browse Menus…。

这是实际的边界侵入：Flutter 从业务 ID 推导动作类别，依赖了未声明的命名约定。

最小修复：Rust 为 Candidate 提供明确的菜单 / 导航角色，Bridge 原样转换，Flutter 只按角色构建菜单视图。不要仅删除前缀判断然后依赖非空菜单路径：顶层菜单叶子的父路径可能为空。也不要把 owning_app 单独当成菜单标志，Context 来源归属不等于动作类型。保留合法插件命名与稳定 ActionId。

补测：合法 app.* 插件、空父路径的真实应用菜单、相同标题的普通动作、Current App 与功能 Scope；既要可见也要走原执行凭证。

## 3. 需要逐步收口的职责

### 3.1 业务判断集中在 Rust，Flutter 消费明确语义

- [Rust Scope 策略](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/core/search/scope.rs:84) 和 [搜索过滤](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/core/search/engine.rs:114) 已决定 Current App 聚合全部适用 Actions、其他 Scope 过滤 Group。
- [Flutter includesGroup](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/launcher_view_state.dart:125) 再实现同一策略；菜单浏览还需要处理完整 contextCandidates，因此不能直接删除所有客户端筛选。
- 建议 Rust 返回明确的已按当前 Scope 选取的完整 Action offers，Flutter 只组织推荐行 / 菜单层级。普通候选列表按 Rust 输出消费；保留“过期输入不展示”的 UI 防护，但不再重新决定领域适用性。

这是重复规则风险，**没有证明当前内建 Scope 结果已错误**。排序、frecency 与 Availability 仍由 Rust 决定。

### 3.2 对象类型不能由默认执行命令决定

[文件主候选](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/features/filesystem/search.rs:390) 的 kind 为 File；[文件次级动作](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/features/filesystem/search.rs:467) 也重写为 File。[图标 UI](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/widgets/candidate_icon.dart:24) 用 commandId / iconKey 判断目录；生成 FileFilter fallback 则通过 `core.directory.open` / `core.file.open` 分类。

现行 UI 已通过 fileObject / primaryAction 等分支区分，未据此报告当前图标错误。但默认动作改变、增加其他文件打开方式时，需要修改多个 UI 推断点。

建议分开 Candidate 行角色与 target/object 类型，显式携带 file/directory 标识；ActionId 仅标识动作和持久快捷键。Flutter 可以独立选择图标形状/颜色，不负责对象真实性或搜索资格。

### 3.3 输入预填与隐藏保留要有明确策略所有者

[ActivationPrefill](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/activation_prefill.dart:9) 在 Flutter 决定选中文本优先、剪贴板年龄和 15 秒有效期。它没有重新采集或解析 Context，因此不能把它等同于 Flutter 自己识别 Context。

如果将此行为定义为统一产品策略，建议 Rust Application 提供推荐输入/来源/有效性，Flutter 决定用户是否已编辑、是否应用推荐以及如何展示。当前并非执行安全缺陷。

隐藏保留的 Core deadline 与 Flutter UI Timer 都有必要：前者保障会话/执行凭证过期，后者释放控件状态。但 [Dart 期限](/Users/luckincoffee/Documents/Github/Anywhere/apps/flutter/lib/launcher/launcher_controller.dart:189) 与 [Rust 期限](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/launcher_service.rs:169) 不应长期各写一份 15 秒。后续由统一协议策略提供期限，Rust 返回恢复/过期结果；Flutter Timer 仅负责展示清理。不要把后台进程生命周期合并到 Launcher 会话。

### 3.4 通用 Source 协调尚有文件领域耦合

[请求构建](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/search_sources.rs:190) 对所有异步来源发同一个 FileSearchRequest payload；来源选择、配置版本与提交有 files 分支。`Any` 是内部运行时类型容器，不能被称为外部插件协议。

[来源注册](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/launcher_service.rs:352) 直接 push，没有拒绝重复 SourceId；提交的 source_candidates 以该 ID 为所有权 key。新增来源应先校验标识唯一，声明自己接受的查询模式、构建自己的请求。文件冻结/进度/加载追加交给文件工作区协调器，通用协调只管凭证、批次和来源生命周期。

当前生产异步来源仅文件，因此没有报告已经发生的 Browser/Clipboard 插件故障。V0.8 应先用一个实际非文件来源验证该边界，并明确总 admission / 结果预算。

### 3.5 Controller 的负担仍然较重，拆职责而非按行数拆文件

LauncherController 当前 1,677 行，负责 Session 调用、窗口同步、保留恢复、预填、Scope、菜单导航、文件追加、对象 Actions、快捷键和执行输出。sealed overlay 已改善互斥状态，但大量异步操作仍维护多个请求版本与字段。

建议逐步提取：

1. Session 生命周期协调：激活、暂停、恢复、订阅、结束及请求凭证。
2. 工具工作区协调：文件输入、类型、连续加载、对象操作、预览和返回信息。
3. 纯导航转换：Esc、返回、Scope/菜单位置；键盘焦点、滚动、像素仍归 Widget/UI。

Bridge 的 api.rs 当前 958 行，不是生成文件膨胀导致的手写问题。可按查询 / 设置 / 快捷键 / 任务 / DTO 转换组织内部模块，保留薄公开门面和生成接口稳定；不要求复制每个 DTO 成一套 UI domain model。

## 4. 性能上的下一步

上一轮已移除每次 more 全量读取 matches 和每行生成二级动作；本轮不重复报告它们未修复。

仍存在累计列表复制：[锁内拷贝](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/session_runtime.rs:447)、[snapshot 准备](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/snapshot.rs:94)、[全量执行表](/Users/luckincoffee/Documents/Github/Anywhere/crates/anywhere-core/src/application/session_runtime.rs:599)。扫描进度更新也重建累计候选；Bridge/Flutter 接收整份列表。磁盘遍历和排序批次有界不等于整条展示链路恒定内存。

建议先测 1k / 10k 已加载行下的 commit / bridge / Dart 分配与进度更新延迟，再把进度更新从候选更新中分离、追加批次增量传输。保留连续列表、稳定选择和早期行可执行，不恢复用户分页。没有新的 App RSS 或帧率测量，因此这里只报告代码成本，不能宣称当前用户界面已经卡顿。

## 5. 哪些分工合理，应保留

| 职责 | 应有位置 | 本轮判断 |
|---|---|---|
| Context 采集/识别、URL/JSON/路径/项目 enrichment | Rust Application / Feature + Platform 采集端口 | 已由 Rust 持有 |
| 候选匹配、排名、去重、适用性、Usage、执行凭证 | Rust | 已由 Rust 持有；Flutter 的 Disabled/过期展示防护不是授权替代 |
| 像素布局、玻璃配色、图标 glyph、菜单展开、Esc/焦点/滚动、草稿 | Flutter | 合理，无需搬进 Rust |
| 语义图标 key、目标类型、来源、业务结果状态 | Rust read model / Protocol | 合理；不能通过 ActionId 猜测缺失语义 |
| Accessibility、Spotlight、文件打开、进程与路径系统细节 | Platform；AppKit/书签/系统热键通过 Host adapter | 合理；Native 不执行匹配/排名 |
| 目录授权与业务设置应用协调 | Flutter infrastructure / Settings 事务端口 + Rust 权威验证 | 合理跨层协作；旧回读需要修 |
| 配置校验 | Rust/Host 权威 + Flutter 即时反馈 | 允许多端防御；字节/字符边界与共享限制应有同一契约及一致性测试 |
| 语义投影与 UI 标签 | Application 的 presentation/read model；Flutter 本地化与视觉 | core/context/navigation 混有英文标签，后续可移动投影而保留图身份算法，不等于 Rust 依赖 Flutter |

文件类型规则已从一份资源生成两端；Flutter FileTypeAppearance 只用于展示，不能因它也看后缀就判定侵入搜索业务。HostView 的文件工具导航请求是动作结果契约，不包含 Widget 或像素，当前合理。

## 6. 验证与实施顺序

本轮重新运行 `scripts/check.sh`：**346 Rust / 191 Flutter** 通过；6 Rust ignored。原生 Host/menu/权限/材质/焦点、fmt、Clippy、架构护栏、生成键码/类型规则和 Flutter analyze 通过。护栏检查依赖方向与少量模式，**不会证明业务规则不存在跨语言重复**。

额外 witness：Dart 两项受控断言确认缺陷存在；Rust 确认普通 app.* 插件合法注册。证据：

- `build/validation/boundary-review-check.log`
- `build/validation/boundary_review_probe_test.dart`
- `build/validation/boundary-review-witness.log`
- `build/validation/review_boundary_probe.rs`
- `build/validation/boundary-review-plugin.log`

建议顺序：先修两个 P2 → 明确 Candidate 菜单/对象语义、收口 Scope offers → 提取 Session/工具协调与统一策略 → 隔离文件 Source adapter、校验唯一注册 → 按测量结果做快照增量。无需提前实现整个外部插件框架，也不改变现有 Scope / Context / 连续列表 / 15 秒交互契约。
