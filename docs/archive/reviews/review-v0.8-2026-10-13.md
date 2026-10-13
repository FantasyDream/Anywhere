# V0.8 综合代码审查

> 历史记录：仅描述文中日期与构建，不作为当前使用或实现契约。当前入口见 [文档导航](../../README.md)。

日期：2026-10-13。范围：`dev-0.8` 当前工作树，包含未提交的 S2/S3 实现，以及 Launcher、搜索、执行、平台与界面公共结构。不是仅对最后一次提交的审查。

## 结论

整体方向合理：Rust 管 Context、搜索、排序、执行与业务持久化；Flutter 管显示、导航、焦点和设置草稿；平台服务隔离系统能力。Git/Developer 的实现已迁入第一方模块，旧入口保留兼容导出。当前代码没有发现 Core 依赖 Flutter，或共享 UI 直接选择 macOS 适配器的情况。

仍有 **1 个 P1、4 个 P2 确定问题**，均有定向复现。现有自动化检查通过不能代表这些失败路径已经覆盖。建议先修复这些问题，再把 Source 生命周期和 Launcher 交互职责整理为 V0.9 的前置工作。

本次未修改生产代码。临时复现测试已移出正式测试目录，源码与日志保留在本地 `build/validation`。

## 修复记录 — Build 4

以下问题记录保留审查时的状态；**本轮代码修复已完成**，不再表示当前工作树仍有这五个缺陷。

| 项目 | 修复方式 | 回归证据 |
|---|---|---|
| R1 收藏覆盖 | Storage IMMEDIATE 事务内读取最新值并修改，提交成功后更新缓存；打开对象时验证跨实例变更 | 独立连接并发更新、两实例保存合并、另一实例移除后旧打开资格失效、额度/修改失败回滚 |
| R2 快捷键残留 | 所有配置操作的 finally 重读动作与资格；失败时清空不确定目录；现代 Reload 一次返回 Extension 目录 | 已提交但返回失败、legacy 重载后状态失败、重试恢复、dispose 后不发布 |
| R3 名称冲突 | 普通查询不解析操作前缀；文件和收藏统一 Object Actions，⌘K 打开、Esc 返回保留查询/选择/滚动 | remove/forget/unpin 名称、真实二级移除与旧目标校验、非 File 类型对象的键盘导航 |
| R4 排序绕过 | SearchContext 提供公共 Ranking Facts、Usage 和时钟；收藏使用标准评分函数 | SQLite execute_count 增加后候选评分提高，原有混合排序回归继续通过 |
| R5 无法恢复 | Reload 重开共享连接并刷新有界持久状态，runtime 每次重建 | 初始化损坏恢复、替换数据库恢复、已加载缓存刷新、新动作写入新数据库 |

其他改进：

- SourceCatalog 原子管理同步/异步 Sources；基础来源保留，旧请求取消，释放发布锁后 reset 旧来源。
- Manager 只缓存 ExtensionState 的有界持久数据，避免把进程、订阅或 UI runtime 永久缓存。
- Launcher 拆出 Backend、ActivationFlow、ShortcutCoordinator、ObjectActionCoordinator、HostViewNavigator；文件传输验证/合并由工作区协调器负责。Rust/Flutter 的业务与交互边界保持。
- 扩展偏好损坏/未知版本进入只读恢复；基础 Launcher 与设置可用，不覆盖原文件；修复后 Reload 恢复。4 MiB reader 在读取阶段生效。
- 设置仅维护一个 Extension 状态目录，旧 PluginStatus 接口放入兼容适配器；关闭、待授权、运行失败与不可用有明确标签。
- 架构护栏禁止 Core 导入 extensions，并补 grouped/aliased/qualified imports 的反例测试。
- 文件尾部更新共享 256 条不可变块，键索引按块复用，行包装按需创建，避免每次追加复制/包装所有已加载候选；保留连续滚动和稳定顺序。

验证：`scripts/check.sh` **386 Rust / 216 Flutter** 全部通过，6 Rust ignored；Native、fmt、Clippy、架构/生成规则、Flutter analyze 通过，架构规则另有 2 项 Python 测试。拆出快捷键协调器后再跑全部 Flutter 测试仍为 216 通过；最终检查和打包记录见 检查日志（本机 `build/validation/v0.8-fix-check.log`，不随仓库分发）、构建日志（本机 `build/validation/v0.8-fix-build.log`，不随仓库分发）。

Build 4 App/ZIP 已生成，包内 37 个文件与 8 个符号链接逐项一致，四个主二进制均为 arm64，保留 Impeller，签名 requirement 与 build 3 一致。详见 包检查记录（本机 `build/validation/v0.8-fix-package-check.json`，不随仓库分发）。

本地 Debug 模拟增量更新的 1k/10k 测试覆盖了结构共享、键盘查找和旧快照不可变性，耗时会受并发测试和编译影响，不能作为 GUI 帧时间或 App RSS 的结论。没有替换正在运行的 App；真实 GUI 授权、物理键盘与整机内存/帧时间仍需发布验收。

## 一、确定问题

## 截图回归修复 — Build 5

目录采集把应用身份失效转为字符串 Provider 错误，原 UI 又直接拼接技术诊断，使与 Global Search 无关的警告长期残留。已改为保留类型、安全跳过失效 / 不支持的目录上下文，限制后台采集提示的页面归属，并避免渲染内部 Provider ID。

补充 Finder 前后身份检查的阶段和拒绝原因日志，保持冻结身份约束。完整检查 388 Rust / 218 Flutter 通过；真实 Finder 生产接口连续读取 5 次成功，错误进程身份拒绝。旧日志不足以证明 Finder 退出或校验误判，当次触发原因仍待新版阶段日志确认；见 [Build 5 验证边界](../v0.8-development-records.md#build-5-目录采集安全降级2026-10-13)。

### R1 / P1：不同实例的收藏目录写入会覆盖已提交的数据

位置：DirectoryStore.change（历史源码 `crates/anywhere-core/src/extensions/saved_directories.rs:90`）。

`writes` 只保护一个 DirectoryStore 实例。更新从该实例的内存快照克隆整个列表，再用 `storage.put` 替换完整 JSON。SQLite 的事务保证单次写入完整，却没有让整个“读取列表—修改—写回”在不同实例之间原子执行。

复现：两个 Application/ExtensionManager 先后打开同一数据目录；A 保存 alpha；B 保存 beta；第三个实例重新读取时只剩 beta。A 已经成功提交的 alpha 丢失。原生支持目录是共享的 `Application Support/Anywhere`，当前启动路径未发现单实例互斥；并行启动开发包或强制第二实例可进入此条件。单进程内复用 runtime 的修复并不能保护这个边界。

建议：Storage 提供事务内读改写或版本 CAS，遇到冲突重新读取、合并并更新内存。单实例约束可以减少发生机会，但不应把一次 `put` 当成列表级并发保护。

最小补测：两个独立连接与 Store 交错 Save/Forget，重新打开后验证两边提交；覆盖相同目录的幂等操作、不同目录的合并、额度失败不覆盖旧值。这里丢失的是收藏数据，不是磁盘目录。

### R2 / P2：重载已生效后，状态读取失败会漏掉快捷键同步

位置：refreshPlugins（历史源码 `apps/flutter/lib/settings/settings_controller.dart:414`）、_updateExtensions（历史源码 `apps/flutter/lib/settings/settings_controller.dart:449`）。

刷新先调用 `_reload()`，再读取 `_readExtensions()`。而 `_syncShortcutCatalog()` 的 finally 位于 `await operation()` 成功之后。重载已在 Rust 生效，但随后状态读取失败时，流程直接进入外层 catch，跳过快捷键目录同步。

复现：初始 test.copy 有绑定且可用；重载成功后 Core 将其移除；紧接着状态读取抛错。同步调用次数不增加，Flutter 的 `shortcutSupported('test.copy')` 仍为 true。对应的 Native 同步入口没有被调用，因此旧系统快捷键可能继续注册并占用按键。

这不意味着撤销后的动作可以执行：Core 的执行资格和 epoch 仍会拒绝旧请求。问题是宿主按键和设置界面可能保留旧资格。

建议：目录重校验放进覆盖整个配置操作的 finally；资格读取失败时沿用现有清空 Native catalog 的降级行为。统一重载返回值，减少“配置提交成功—额外读取失败”的中间状态。

最小补测：重载成功 + 状态读取失败；后端已提交 + 返回通道失败；校验资格读取失败时注销失效按键；重试成功后恢复正确目录。

### R3 / P2：正常目录名被识别为移除收藏指令

位置：SavedDirectorySource.candidates（历史源码 `crates/anywhere-core/src/extensions/saved_directories.rs:251`）。

查询首词是 `forget`、`remove` 或 `unpin` 时，Source 无条件进入移除模式，剥掉首词，并仅生成 Forget 候选。

复现：收藏名为 `remove draft` 的目录，按完整目录名搜索，得到的是移除收藏动作，没有打开候选；执行后该收藏被移除。目录文件本身不删除。

这同时是功能设计和交互一致性问题：普通对象搜索承担了操作指令语法，用户输入目录名不能稳定表达“寻找这个目录”。

建议：普通查询始终按目录名/路径匹配；移除放到对象的二级 Actions，复用既有交互。若保留文本指令，应使用明确、不与普通名称抢占的入口，并保留对象搜索结果。

最小补测：名称以三个保留词开头，带空格、大小写及完整路径时仍能打开；二级移除明确选定对象，旧候选在移除后失效。

### R4 / P2：收藏目录记录了 Usage，但排序没有使用它

位置：SavedDirectorySource 的匹配与评分（历史源码 `crates/anywhere-core/src/extensions/saved_directories.rs:272`）。

Source 构造默认 SessionFacts、空 Usage 查询和时间 0，再将候选 score 覆盖为文本匹配分加 50。SearchEngine 对各 Source 已经生成的分数排序，没有在后面统一补上 Usage。

复现：相同查询的候选分数为 4050；成功打开后 SQLite 中 `(OPEN, saved.directory)` 的 execute_count 已增加到 1；重新激活并查询，分数仍为 4050。

因此，收藏来源相对于 Apps/Actions 的排序没有复用既有 frecency。现有 Usage 的键是动作与 Context 类型，也不能区分两个目录；如果以后需要“常用目录优先”，还需明确对象级使用记录契约。

建议：让 Source 返回匹配信息，由统一 Ranking 处理；或在只读 SearchContext 中明确传入标准匹配/排序服务及 Facts、Usage、时间。避免每个插件重新建立一套评分公式。

最小补测：固定时钟和 Usage，比较执行前后相同候选的分数；验证与应用、动作混合结果的排序；明确同一查询中稳定排序与跨查询学习的边界。

### R5 / P2：替换损坏的存储文件后，Reload 仍使用旧连接

位置：ExtensionManager.reload（历史源码 `crates/anywhere-core/src/application/extension_manager.rs:85`）。

只有 `self.storage` 为 Err 时才重新打开数据库。数据库连接成功、但模块内部值损坏时，Host 仍为 Ok，模块初始化失败。运行中将旧数据库移走并在同一路径创建正常数据库，Reload 继续读取旧文件的连接，模块无法恢复。

复现：写入非法收藏 JSON；模块不可用；将数据库备份到另一位置并创建正常数据库；Reload 后仍不可用；重新创建 Application/Manager 后恢复。该行为不符合 MVP 文档所述“修复后 Reload 可重试”的完整恢复预期。原连接上的原位修复和替换文件是不同条件。

建议：定义明确的存储恢复操作，检测连接所对应的文件已被替换，并协调旧 runtime、连接、正在写入的任务与 epoch 后重新绑定。不能简单每次 Reload 清空缓存，否则会重新引入同进程重载前后的写入覆盖。

最小补测：连接打开失败后恢复、值损坏后原位修复、数据库替换后恢复；与并发 Save/Forget 交错时保持数据和资格一致。

## 二、八个维度的评价

| 维度 | 当前可保留的设计 | 建议调整 |
|---|---|---|
| 架构 | Core 不依赖 UI；DTO、平台接口、业务执行分层清晰；配置切换有 catalog/execution epoch | 排序与 Source 生命周期还未完全统一；扩展配置错误的影响范围过大 |
| 模块划分 | JSON/URL/Git/Developer 是实际模块；Developer 共享识别实现；模块失败可隔离贡献 | 同步 Source 和异步 Source 走两套装配/替换路径；FeatureContribution 与 ExtensionRuntime 职责需要收拢 |
| 功能设计 | Context → Action → Scope → Ranking 主链保留；模块开关和授权不改变动作身份 | 收藏目录的搜索指令抢占对象名；应使用标准对象动作入口 |
| UI / 交互 | AppStyle、CandidateRow 和筛选条复用；文件页复用 Launcher；固定右预览、保留查询；小窗口布局有测试 | 扩展状态需更明确地表达关闭、待授权、运行中、失败；重载失败后仍需同步按键资格 |
| 性能 | 查询内存快照、存储配额、扫描并发控制、预览字节上限、查询临时外排均有边界 | 偏好文件先完整读取再检查大小；大量已加载文件结果的内存增长需测量；避免长期缓存未来昂贵 runtime |
| 代码复用 | 共同 ActionRegistry、平台服务、执行 Session、样式组件可继续复用 | 新 Source 自建 Matcher 和评分会造成行为分叉；统一 Ranking 上下文 |
| 可读性 | 局部模块有明确接口与契约；跨语言 DTO 自动生成 | LauncherController 同时承载多类流程，未来插件 UI 会继续扩大；legacy Plugin 与 Extension 状态双接口增加阅读成本 |
| 可扩展性 | 第一方模块与声明式 v2 的信任边界明确，能力批准与 metadata 分离 | 在 0.9 前补齐通用 Source 安装/取消和 Host View 导航契约；Windows 仍需实现平台适配器 |

### 2.1 同步与异步 Source 尚未使用同一生命周期

ExtensionRuntime（历史源码 `crates/anywhere-core/src/extensions/mod.rs:25`） 只提供同步 Sources；FeatureContribution（历史源码 `crates/anywhere-core/src/features/contribution.rs:7`） 另有异步 SourceFactory。

builtin_sources（历史源码 `crates/anywhere-core/src/application/composition.rs:47`） 在 Application 构建时装配异步来源；replace_extension_runtime（历史源码 `crates/anywhere-core/src/application/launcher_service.rs:256`） 只替换 Registry、Provider 和同步 Sources。当前收藏目录是同步来源，所以这不是已经发生的异步模块关闭缺陷；但今后浏览器、历史或网络扩展无法直接复用现有热重载契约。

建议一份 ModuleContribution 描述 Actions、Providers、同步/异步 Sources；宿主统一验证所有权、安装新配置、取消旧任务，并限制并发和资源。文件搜索服务的专用能力仍可以保留，不要求所有领域硬塞进同一种数据结构。

### 2.2 LauncherController 的职责需要继续拆分

LauncherController（历史源码 `apps/flutter/lib/launcher/launcher_controller.dart:376`） 当前 1708 行。问题不只是长度：它同时处理唤醒/保留会话/预填、全局快捷键解析、文件工作区返回与加载、菜单导航、结果执行展示、设置返回和窗口同步，共享 revision、overlay、selection 等状态。

现有 LauncherSessionCoordinator 与 FileWorkspaceCoordinator 已有帮助；但文件导航、结果合并和返回路径仍主要在 Controller。新增 HostView/插件页面会继续扩张中央流程。

建议按状态归属拆成 ActivationFlow、ShortcutCoordinator、ToolNavigation/OutputPresentation 等可独立验证的用例；统一请求身份和取消规则。焦点、导航、设置草稿仍留在 Flutter，不为缩短 Controller 把交互状态移进 Rust。

### 2.3 扩展偏好错误应有可用的恢复入口

ExtensionPreferences.load（历史源码 `crates/anywhere-core/src/persistence/extensions.rs:28`） 对 JSON、版本和读取错误返回 Err；initialize（历史源码 `crates/anywhere-bridge-flutter/src/api/settings.rs:38`） 在安装 Application 前传播错误；main（历史源码 `apps/flutter/lib/main.dart:177`） 进入启动失败视图。

这是当前明确采用的失败策略，不作为未经确认的新回归。但一个扩展偏好文件错误会阻止主 Launcher 与正常设置页可用，与“模块失败隔离”的目标有距离。

建议失败时保留基础 Apps/搜索及修复设置入口，受影响扩展和不确定授权保持不可用；未知 schema 不覆盖、不默默重置。读取采用有界 reader，使 4 MiB 限制在大文件分配前生效。

### 2.4 架构护栏没有覆盖新的 extensions 层

check-architecture.py（历史源码 `scripts/check-architecture.py:31`） 检查 Core 禁止导入 features/application/persistence，却未纳入 extensions。当前没有发现 Core 直接导入 extensions；问题是未来发生此依赖时现有门禁可能仍通过。

建议补入新产品层，并用小型正反例验证规则。源码正则检查可作为轻量保护；长期若需要编译级依赖约束，再考虑拆 crate，不必为了文件夹整理立即增加大量 crate。

### 2.5 性能结论的边界

- 收藏上限 100 条；查询不读 SQLite，当前克隆小列表不是需要立即复杂化的瓶颈。
- 文件结果外排的 SORT_BYTES（历史源码 `crates/anywhere-core/src/features/filesystem/result_store.rs:14`） 是 1 MiB；预览（历史源码 `crates/anywhere-core/src/features/filesystem/preview.rs:6`） 读取上限 64 KiB；搜索 worker（历史源码 `crates/anywhere-core/src/features/filesystem/search.rs:107`） 有并发限制。这些是局部资源约束，不等于整个 App 内存恒定。
- 无分页连续列表会保留已加载候选，结果数量增加时内存也会增长。应记录 1k/10k 命中滚动时的 Flutter heap、Rust heap、RSS 和合并耗时，再决定是否采用分块列表/增量合并；保持现有连续滚动体验。
- ExtensionManager 缓存 runtime 能避免重载写入覆盖。目前只是小型状态；未来 WebView/进程/订阅需要明确 shutdown/cancel，不能一律永久缓存。
- 本轮没有 GUI 帧时间、RSS 或泄漏测量，不能把自动化通过表述为流畅度和内存已经验收。

### 2.6 UI 与跨平台结论的边界

本轮验证了授权弹窗不使用 Tab 时可通过方向键到达 Allow 并用 Enter 确认，因此不报告“弹窗只能鼠标操作”。现有设置组件布局与文件预览交互由测试覆盖，但没有启动当前开发包做视觉验收，不能确认真实背景下的美观、对比度与原生焦点表现。

native_platform（历史源码 `crates/anywhere-platform/src/lib.rs:426`） 当前只为 macOS 提供实现；其他系统返回无适配器错误。共享代码可复用，不代表 Windows 应用已能运行。这是已知平台实现范围，不是本次新增缺陷。

## 三、修复与后续顺序

1. 修复 R1 的数据一致性；确认多实例策略并补 DB 级并发测试。
2. 修复 R2 的配置/快捷键生命周期，并同时补 R5 的存储恢复契约。
3. 修复 R3 的对象搜索语义、R4 的统一排序。
4. 0.8 收尾补架构护栏、扩展失败恢复入口和真实 GUI 授权/按键验收。
5. 0.9 前统一 Source 生命周期与 Host View 导航；拆分 Launcher 用例。优先完善现有契约，避免提前引入动态 ABI、Web runtime 或复杂插件框架。

## 四、验证与证据

### 定向复现

临时 Rust 测试运行：4/4 通过，断言的是上述问题确实存在，不代表修复完成。Flutter 定向测试：2/2 通过，其中一个复现快捷键漏同步，一个确认授权弹窗方向键可用。

本地证据：

- Rust 复现源码（历史源码 `build/validation/v0.8-review-rust-probes.rs`）、日志（本机 `build/validation/v0.8-review-rust-probes.log`，不随仓库分发）
- Flutter 复现源码（历史源码 `build/validation/v0.8-review-flutter-probes.dart`）、日志（本机 `build/validation/v0.8-review-flutter-probes.log`，不随仓库分发）
- 完整检查日志（本机 `build/validation/v0.8-review-check.log`，不随仓库分发）

`build/validation` 是本地忽略目录，不随报告推送。复现源码需分别放回 `crates/anywhere-core/tests/v08_review_probe.rs` 与 `apps/flutter/test/v08_review_probe_test.dart`，才能按原测试目标重新运行。

### 完整工程门禁

本轮重新执行 `scripts/check.sh`，全部通过：**377 Rust / 208 Flutter**，6 Rust ignored；Native、fmt、Clippy、架构护栏、生成规则与 Flutter analyze 通过。它不包含真实 GUI 权限交互、整机内存采样或 Windows 发布矩阵。

清理临时测试后，比对审查开始时的工作树状态与变更文件 SHA-256：原有变更文件内容全部一致，仅新增此报告；`git diff --check` 通过。

本次只新增此审查报告，未修复问题、未提交或推送代码。
