# V0.5 可维护性 Review 与后续架构调整

> 历史记录：仅描述文中日期与构建，不作为当前使用或实现契约。当前入口见 [文档导航](../../README.md)。

审查日期：2026-10-05。基线：`dev-0.5@6c943c1`。对照当前 `docs/roadmap.md` 的 V0.6～V1.0，不按早期已被裁剪的路线图评判。

后续修复：同日已落实下列第 1～4、6 项，交付构建号 `0.5.0+11`；具体实现与验证见文末。以下问题与行号描述的是修复前基线。

## 结论

现有 Context → Action → Candidate → Ranking → Execution 主流程可以继续使用。框架契约、产品用例、第一方功能、平台实现和 Flutter 展示有基本边界，当前没有证据支持重写或拆出更多 crate。

发现一个经受控测试确认的执行失效问题；此外，快捷键身份契约、Host/Controller 职责和动态属性契约应在 V0.6 前整理。同步搜索、声明式 UI 和进程输出接口的拓展分别随 V0.7、V0.9、V0.10 实施。

下文区分：已复现的行为问题、代码直接显示的维护负担，以及尚未交付功能需要的接口演进。后两者不等于当前功能已经错误。

## 1. [P2 / 已复现] Registry 重载不能撤销已排队的普通动作

证据：

- `crates/anywhere-core/src/application/launcher_service.rs:151` 的 `replace_registry` 替换基础/会话 Registry，并触发菜单刷新；没有改变当前 Session/Revision。
- `crates/anywhere-core/src/application/execute.rs:67` 先捕获 Registry 与 Candidate，随后将它们交给 blocking worker。
- 同文件 `:104` 复验 Session/Revision，`:107` 的 Registry 目标复验只适用于具有 `execution_token` 的动作。声明式动作没有该 token，见 `core/action/descriptor.rs:25`。

受控复现：限制 blocking pool 为一个线程，先占住线程；调用 `execute` 并明确 poll 到 Pending；随后把 Registry 换成不含 `example.search` 的内建 Registry。确认当前快照已无此动作，再释放线程。旧请求返回成功，TestPlatform 收到一次打开 URL 的调用。

这是 Facade 生命周期缺口。当前设置页重载通常先关闭 Launcher 会话，不能据此宣称日常 UI 必然触发；但公共 API 已允许这种顺序，后续快捷键分派、插件禁用/重载会依赖同一边界。

最小调整：

- 给执行凭证加入动作目录版本；在 Registry 替换时推进版本，在 worker 开始副作用前复验。
- 目录版本与菜单发现 token 分开：前者管理动作定义/授权失效，后者管理扫描目标失效。
- 优先选择“重载使旧请求失效”，不要在同一个旧请求中默默换用同 ID 的新 Handler。
- 已开始的后台进程继续按 ExecutionSession 管理，不因 Registry 重载被隐式终止。

补测：排队期间移除动作、同 ID 换执行器、`execute_builtin` 的目标被移除；三者都应拒绝旧请求且不发生平台副作用。保留已有 Session 取消和菜单重扫的回归。

## 2. [P2 / V0.6 前置] 身份分层主要靠约定，绑定资格没有显式契约

证据：

- `anywhere-protocol/src/lib.rs:11` 的 ActionId 与 CandidateId 都是 String 别名；`application/launcher_service.rs:47` 的 execute 参数仍以 ActionId 标注 Candidate ID。
- `core/search/candidate.rs:84` 给菜单 Candidate 增加 discovery token，说明它不能作为持久快捷键配置的目标。
- `core/action/handler.rs:44` 只有 `records_usage` 和 `execution_token`；没有“可持久绑定”的独立策略。
- `features/app_actions/convert.rs:77` 将应用菜单自带快捷键放到字符串 `shortcut` 中，Flutter 将其作为显示标签。它还不是用户配置的快捷键。
- 路线图明确禁止未经验证的菜单身份被静默长期绑定。

风险：V0.6 若直接复用行 ID、`shortcut` 或 `records_usage` 来决定绑定，可能存下本次扫描身份，或把菜单展示快捷键混为 Launcher 快捷键。当前尚未实现 Action 自定义快捷键，因此这属于必须提前落实的契约，不是已发生的绑定故障。

最小调整：

- 先纠正参数名/类型语义；内部在执行与持久化边界采用独立 ID 包装类型，桥接仍可传字符串。
- 显式定义绑定资格及拒绝原因；持久历史资格与快捷键资格分别表达。
- 应用原生快捷键、Launcher 内快捷键、全局快捷键分别建模；保存业务动作 ID 与 Context 匹配规则，执行时重新捕获 Context 并解析当前 Candidate。
- 菜单继续保持历史隔离和禁止持久绑定，直到独立身份契约可验证。

补测：序列化重载后的稳定绑定、缺少 Context、动作移除、冲突、菜单绑定被拒绝；不能仅验证快捷键字符串能显示。

## 3. [P2 / 维护负担] 原生 Host 与 LauncherController 承担过多职责

问题由职责交叉体现，而不是单纯文件行数：

- `Main.swift:152` 在 AppDelegate 中处理设置保存、热键注册失败恢复、登录项与磁盘持久化；`:282` 管理共享 Flutter 视图在 Launcher/Settings 间转移；`:413` 创建任务 engine、窗口和第二条 Channel；`:517` 处理全局热键；`:765` 协调退出。
- `launcher_controller.dart:558` 管设置窗口；`:579` 管关闭草稿回调；`:597` 直接协调后台任务退出；`:627` 同时保存原生设置并更新 Rust 偏好；`:661` 管插件重载。Launcher 的输入、Scope、菜单导航和异步快照也由同一个 Controller 管理。
- 热键标签和键码映射在 Dart `settings/host_settings.dart:124` 与 Swift `Main.swift:863` 中各实现一次。

影响：增加一类 Action 快捷键、一个插件窗口或一项设置，容易同时触碰窗口代次、关闭草稿、热键、退出和 Launcher 状态。V0.9 多视图、V0.10 Terminal 将进一步放大这一成本。

最小调整：

- Swift 提取 HotKeyService、SettingsStore、窗口协调对象；AppDelegate 保留系统事件转发和装配。
- Flutter 提取 SettingsController，以及应用级关闭/退出协调；LauncherController 继续管理 Launcher 会话、查询和选择。
- 共享 engine、窗口焦点规则、generation 校验保持现有行为，不因拆职责改动产品体验。
- 统一快捷键数据与显示转换的来源，并把原生注册冲突/恢复、关闭/最小化/退出的行为变成可测试契约。

验收：复用已有草稿 Save/Discard/Cancel、录制取消、最小化恢复和 Launcher 失焦用例；补 Host 热键注册失败恢复用例。不能仅靠文件拆分和编译通过验收。

## 4. [P2 / 维护负担] 动态项目属性是字符串协议，契约破损会静默消失

证据：

- `core/context/mod.rs:34` 的 Project 保存 `BTreeMap<String, String>`。
- `features/developer/provider.rs:142` 写 manager、JSON 编码的脚本列表与字符串 true；`:160` 写位置固定的三项 Cargo JSON 数组。
- `features/developer/dynamic.rs:41` 将脚本解码失败当作空列表；`:77` 对 Cargo target 解码失败直接跳过。
- `core/action/availability.rs:53` 只验证属性名非空；`:82` 只接受精确字符串 true，否则隐藏动作。

当前内建 Provider 正常生成对应格式，不能据此认定现有 Node/Cargo 动作有普遍故障。问题是生产者、动态工厂、Availability 消费者靠多个字符串约定同步；以后改字段、换格式或迁出模块时，错误可能表现为“没有动作”，而不是可追踪的契约失败。

最小调整：先在 developer feature 内引入共享的 Node/Cargo 元数据结构、键常量与 encode/decode 适配；合法空能力与格式错误分开，错误带 Provider/Factory 身份的诊断。保留开放 ContextTypeId，不把每个项目类型都增加为 Core 专用分支。外部传输格式在 V0.8 再版本化。

补测：坏 JSON、缺少字段、未知 manager、字段版本不兼容时有诊断且不运行命令；合法空脚本/无 Cargo binary 保持正常降级。继续验证元数据不包含脚本正文。

## 5. [P3 / V0.8 演进] Feature 装配还没有统一贡献接口

`features/mod.rs:33` 汇总 Bundle，`:53` 汇总 Provider；`application/composition.rs:17` 另外注册 DeveloperActions 工厂；Group 常量与内建展示构造仍在 `core/action/group.rs:20`；额外 CandidateSource 由 Application 单独注册。

当前集中装配是合理的规模选择。但一个完整 Feature 的注册仍散落在几个地方；新增带 Provider、静态/动态动作、候选来源的功能需要记住多个入口。ContextActionFactory 也没有可用于诊断的模块 ID。

建议 V0.8 用 Node 和 Rust 两个实际模块验证同一个内部贡献接口，统一收集 Provider、Bundle、Factory、Group 与候选来源。框架只维护注册和校验，模块持有其声明。先静态链接，第三方 Manifest v2、Capabilities 与宿主服务另有公开协议和迁移测试；不把 Rust Trait 当 ABI。

## 6. [P3 / 当前文档] 架构说明与最新菜单降级冲突

`docs/architecture.md:83` 笼统说 Candidate ID 不含会话版本，未解释菜单 discovery token 的例外；`:97` 仍说动态菜单推荐使用真实历史。当前代码给菜单加 token 并不消费/写入持久历史，最新 `docs/current-app-actions.md` 和 roadmap 已说明这一降级。

应同步总架构文档，并区分 business identity、当前执行 capability 与长期绑定资格。否则下一阶段开发者容易按旧说明恢复不安全的 frecency 或菜单绑定。

## 随路线图实施的接口变化

| 阶段 | 当前代码的实际边界 | 应在本阶段完成的调整 |
|---|---|---|
| V0.6 Shortcuts | 热键只有两项原生唤醒入口；Action shortcut 是展示字符串 | 先完成执行目录版本、身份与绑定策略、Host/Settings 职责拆分；再实现配置、冲突与 Context 分派 |
| V0.7 File Search | CandidateSource 同步返回 Vec，SearchContext 不含取消凭证；SearchEngine 顺序收集 | 新增异步来源协调与有界批次，携带 Session/Revision/来源 generation；保留内存来源的同步接口。文件候选进入统一 Ranking，不用 LaunchApplication 目标冒充文件打开 |
| V0.8 Extension v2 | Feature 注册分散；v1 Manifest 只支持 Copy/OpenUrl/OpenFile/ExecuteBuiltin | 两个第一方模块验证贡献接口；公开协议独立定义版本、授权和兼容，保留 v1 |
| V0.9 Host UI | ExecutionOutput 是 text、close_window、process 的字段组合；插件没有视图描述 | 根据真实 List/Form/Detail 用例引入明确的结果类型和 Host View 生命周期；统一返回、错误和快捷键归属，不塞进 Launcher 菜单路径 |
| V0.10 PTY | Manager 直接调用 OwnedProcess；stdout/stderr 是两个文本尾缓冲；Flutter 可见时每 500ms 拉取完整详情 | 复用 Session 身份与状态，增加实际 Process/PTY 后端、输入和 resize；终端输出用有序字节流、游标、限量与背压。保持当前普通任务文本预览 |
| V1.0 | 进程会话属于应用生命周期，正常退出停止进程 | 与现路线图定义一致。跨应用退出存活需要额外生命周期设计，不通过修改 shutdown 默认行为引入 |

这些是未来功能的必要演进点，当前轮询和非交互 Process 实现本身不构成缺陷；现在不需要实现通用 Web 插件或 Agent 框架。

## 建议实施顺序与验收

1. 修复 Registry 重载下的排队动作失效，补三种受控回归。
2. 在 V0.6 开始前落实 ID/绑定策略，整理 Settings/Host 职责和 developer 元数据契约，同步架构文档。
3. 实现 V0.6 产品功能。按上述表格在后续阶段引入异步搜索、模块贡献、Host UI、PTY 流式输出。

保留现有 Context Graph、Availability、Scope、Ranking、ExecutionSession 与五个 crate 的职责。评判调整是否成功，应看新增功能需要修改哪些既有模块、错误与失效能否被检测、是否复用原执行路径，以及原有生命周期回归是否通过。

## 修复前 Review 验证范围

- 当前架构静态检查通过：104 个 Core 可达依赖，无 UI/Flutter bridge 依赖；Core 生产模块无 application/features/persistence 导入。
- 4 项架构集成测试通过。
- 已有 2 项排队执行回归通过：Session 关闭与菜单 discovery 失效。它们没有覆盖普通动作的 Registry 重载失效。
- 新增临时受控证据测试确认第 1 项问题。证据测试断言当前不安全行为并通过，不能算修复回归通过；测试文件已移出仓库，保留在 `/tmp/anywhere-maintainability-witness.rs`，日志在 `/tmp/anywhere-maintainability-audit.log`。
- 本轮没有重跑全部检查、打包或真实 GUI 验收；没有修改产品实现，仅新增本报告。

## 修复结果（0.5.0+11）

1. **动作重载失效**：Resolver 新增动作目录版本。Registry 替换同时清除执行缓存、推进版本并重算快照；快照提交拒绝旧版本。执行在 blocking worker 开始时、进程规划完成后启动前复验版本。重载后行 ID 带目录版本，旧行不能命中新注册的同 ID 实现；查询重排不推进版本。已有进程不被隐式停止。
2. **身份与绑定契约**：执行缓存采用 `ExecutionCandidateId`，历史和持久绑定采用 `BusinessActionId`；公开 bridge 继续传字符串。Registry 的 `persistent_binding` 返回业务 ID 与 Context 类型，验证当前动作和 `execute_builtin` 目标；菜单通过独立 BindingPolicy 拒绝持久绑定，既有菜单历史隔离保留。Candidate shortcut 明确只是原应用快捷键的展示。V0.6 的快捷键配置、冲突分派及 Context 缺失行为仍须随产品功能实现。
3. **Host 与 Controller 拆职责**：SettingsController 管设置保存、来源状态、插件和草稿关闭入口；ApplicationLifecycle 管退出决策与任务停止。原生 SettingsStore、HotKeyService、LauncherWindowCoordinator、TasksWindowController 分管存储、热键、共享视图与任务窗口。AppDelegate 保留系统事件、generation 和装配。热键键码/标签由 `resources/hotkey-keymap.json` 同时生成 Dart/Swift，检查与构建拒绝过期生成物。
4. **Developer 元数据**：NodeMetadata / CargoMetadata 统一生产和消费，带版本、命名字段、数量和内容校验。缺字段、坏 JSON、不支持 manager / version、缺少 capability 都产生带 Provider/Factory 身份的诊断，错误不含原始正文；合法空脚本和无 binary 保持无错误。Cargo 目标读取失败显式提示，Check/Test 保留。Provider 仍只读元数据，执行时仍重新检查项目。
5. **文档**：总架构更新菜单 token、历史隔离、目录版本、绑定策略、元数据和 Host 职责。V0.8 贡献接口、V0.7 异步搜索、V0.9 插件 UI、V0.10 PTY 没有提前实现，继续按实际路线图演进。

验证：

- `scripts/check.sh` 通过：259 项 Rust 测试、66 项 Flutter 测试；fmt / Clippy / Flutter analyze、架构与原生检查通过。
- 三种确定性排队回归覆盖插件移除、同 ID 换实现、`execute_builtin` 目标撤销，检查旧请求与重算后旧行拒绝，平台调用为空。另有持久绑定序列化、动作移除/候选 ID 拒绝及菜单绑定拒绝回归。
- 元数据新增三组单元测试覆盖非法与合法空能力；复用既有动态动作识别、身份与执行复验用例。
- 原生 Host 服务回归使用假热键后端：冲突不注销旧热键、注册失败恢复、恢复失败告警、保留其他设置、退出清理；真实临时文件验证旧设置兼容和持久化失败。不会注册物理热键。
- Flutter 复用设置草稿 Save/Discard/Cancel、保存失败与等待、关闭/最小化恢复、录制取消回归；新增退出取消和并发退出单次决策/停止用例。
- 本轮没有重跑真实窗口、物理热键、权限、多机器安装以及四项默认忽略的真实工具测试。编译和自动化不能替代这些实机验收。
