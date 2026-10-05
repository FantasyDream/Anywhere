# Anywhere 产品路线图

状态：2026-10-06。版本号表示依赖顺序，不代表发布日期。`dev-0.4` 工程收尾已提交到 GitHub（`b622811`）；真实应用及权限矩阵仍须验收，不能据此标为正式发布。`dev-0.5` 已从该提交创建，已实现独立进程会话、Rust/Maven/Gradle/Node/Spring Boot 识别、Cargo Run 目标与 npm/pnpm/yarn 动态脚本，标准检查、四组真实工具及本机主要窗口生命周期验收通过；功能与工程收尾已完成，完整检查及开发包校验通过，见 [0.5 收尾记录](v0.5-closeout.md)。正式发布仍需安装升级、权限与读屏验收，完整功能说明见 [V0.5 MVP](mvp-v0.5.md)。V0.3.x 的遗留验收以 [MVP 验收清单](mvp-v0.3x.md)为准；V0.4 的当前实现与待验收项见 [验收记录](v0.4-context-acceptance.md)。

## 产品边界

2026-10-06：V0.5 收尾及可维护性修复已推送至 `dev-0.5@d03a1f2`，见 [最终功能核对](v0.5-final-acceptance.md)。`dev-0.6` 从该基线开发，Launcher 内动作快捷键切片已实现；全局意图分派与真实全局热键验收仍未完成，见 [V0.6 MVP](mvp-v0.6.md)。

Anywhere 的核心体验是：在唤醒时理解用户正在操作的对象，给出可执行的下一步动作；需要搜索时，应用、动作和其他结果进入统一候选列表。Rust 负责 Context、匹配、排序、执行与历史；Flutter 负责交互；原生平台层负责系统采集、窗口和进程能力。已有的 `ContextProvider`、`ActionBundle`、`CandidateSource`、Scope 与 Ranking 是继续扩展的基础，不为远期插件或 Agent 预建通用运行时。

近期以 macOS 为验收平台。Windows 能力须在 Host、CI 和实机验收恢复后单独排期，不占用这条主线的发布门槛。

## 交付顺序

| 阶段 | 必须交付 | 进入下一阶段的条件 |
|---|---|---|
| **V0.3.x App Actions 稳定版** | 菜单发现、执行、身份、历史、Scope 与权限降级形成闭环；修复真实应用验收发现的问题 | [MVP 验收清单](mvp-v0.3x.md)完成，不再继续增加 App-specific 功能 |
| **V0.4a Context 采集** | Finder 选中文件/目录、选中文本、单次剪贴板上下文；来源、归属、权限和失败状态明确 | 三种来源均在实机产生 Context，并复用现有 Action/Scope/Ranking；失败不影响已有能力 |
| **V0.4b 浏览器当前页** | 接入 Safari、Chrome、Edge 的当前 URL，明确授权与降级；其他浏览器按需适配 | 当前页与用户输入 URL 可区分来源；无权限或无页面时不猜测 URL |
| **V0.5a 进程会话基础** | 有界输出、状态、退出码、停止、返回会话；会话在 Launcher 隐藏后仍可访问 | 一个演示长任务可启动、查看、停止和重新进入，且不会阻塞 Launcher |
| **V0.5b Developer Context Pack** | 从目录轻量识别 Git、Maven、Gradle、Node、Rust、显式 Spring Boot；常用 Build/Test/Run、Node 全部支持的已声明脚本与 Cargo 运行目标 | 识别阶段不运行构建命令；每种已声明项目类型至少有一个实际项目通过端到端验收 |
| **V0.6 Action Shortcuts** | Launcher 内自定义快捷键；少量全局“意图快捷键”按新捕获的 Context 分派 | 冲突、缺失 Context、权限拒绝和动作失效都有确定行为；配置可迁移 |
| **V0.7 File Search** | macOS 使用 Spotlight 的异步结果进入统一 Candidate；可取消、限量、去重 | 连续输入不显示旧结果；文件结果不淹没当前 Context 的动作 |
| **V0.8 First-party 模块化与 Extension v2** | 已验证的能力逐步迁出集中装配；以真实模块验证版本化 Manifest、Capabilities 和宿主服务 | 至少两个第一方模块使用同一接口，旧声明式插件继续可用，有迁移测试 |
| **V0.9 Host-rendered Plugin UI** | List、Detail、Form、KeyValue、Markdown 的受限描述和宿主渲染 | 一个实际插件完成输入、校验、执行、错误与返回；无任意 Flutter Widget 注入 |
| **V0.10 PTY / Terminal** | 在进程会话之上增加 PTY、尺寸变化、输入流和终端视图 | 高输出量下有流控；隐藏与重新进入、停止和退出状态一致 |
| **V1.0** | Context-aware Launcher、Universal Search、Current App Actions、多来源 Context、Developer Actions、可扩展协议、可返回的执行会话 | 下方的 1.0 验收条件全部成立 |

### 范围取舍

- **V0.4 的剪贴板是一次唤醒的一次快照，不是历史管理器。** macOS 对程序化访问剪贴板可能询问用户；来源应能独立关闭、失败或无权限，且不拖住历史加载。[Apple NSPasteboard 访问行为](https://developer.apple.com/documentation/appkit/nspasteboard/accessbehavior-swift.enum/default)。
- **V0.4b 先覆盖 Safari、Chrome、Edge。** 当前页通过各浏览器的 AppleScript 字典读取，需分别获得 macOS Automation 授权；其他浏览器按需求和实机验收结果再适配。[Chromium AppleScript 当前标签页示例](https://chromium.googlesource.com/chromium/src/+/f74d7ed2fd9792ed79b26a6512e7ba3df5f90eef/chrome/browser/ui/cocoa/applescript/examples/window_operations.applescript)。
- **V0.5 的 Provider 只读轻量元数据。** Maven/Gradle/Node/Rust 命令只能由用户选中 Action 后启动。动态列举所有 Gradle task、分析 Spring Boot 多模块依赖和自动运行项目脚本不属于第一批。现有 Git Status/Diff/Log 复用，不重复造动作。
- **V0.6 先完成快捷键的身份与冲突语义。** 导入/导出要在配置格式稳定后再加；不为了未来社区配置提前冻结格式。
- **V0.7 先做文件搜索。** Spotlight 提供异步查询；Windows Everything SDK 依赖用户已运行 Everything，且不作为 macOS 版本的发布门槛。[Apple NSMetadataQuery](https://developer.apple.com/documentation/foundation/nsmetadataquery)、[Everything SDK](https://www.voidtools.com/support/everything/sdk/)。剪贴板历史、浏览器历史与书签分别评估隐私、保留策略和授权后再排期。
- **V0.8 的第一方模块化与第三方执行能力是两件事。** 第一方代码可以静态链接并复用内部 Rust Trait；第三方协议需要版本、能力授权、隔离和兼容策略。现有声明式插件 v1 继续工作，不把内部 Rust Trait 当成稳定 ABI。
- **V0.9 先解决真实表单与结果展示。** Table、大型布局系统、任意脚本和动态 Web UI 等待明确用例。
- **通用 React/Vue/Svelte Web 插件、Code Agent 管理、ACP、Clipboard Manager、Screenshot/OCR/Translate、Marketplace 不列入 1.0 门槛。** Terminal 可以使用专门的 WebView 桥接；通用 Web 插件不是前置依赖。Flutter 官方仍指出 macOS Platform View 的手势支持不完整，终端输入需要独立验收。[Flutter macOS Platform Views](https://docs.flutter.dev/platform-integration/macos/platform-views)。

## 架构演进时机

```text
V0.3.x  保持现有 Session / Registry / Candidate 流程，先稳定它
V0.4    新增 Activation Context 种子采集；Provider 继续负责 Context 推导
V0.4    按 [Scope / Menu 规范](scope-group-menu-design.md) 稳定分组硬过滤、菜单推荐/搜索/浏览、键盘返回与使用记录
V0.5a   新增独立 ExecutionSession ID、状态和有界输出；与 Launcher Session 分离
V0.6    目录重载失效、身份/绑定资格、Host/Settings 拆职责已就绪；接入配置、冲突与 Context 分派
V0.7    新增异步搜索协调层；保留现有同步 CandidateSource 给内存来源
V0.8    从真实第一方模块提炼版本化 Extension 协议和能力授权
V0.9    给 Action 结果增加受限 UI 描述，不把插件 Widget 放进 Core
V0.10   在 ExecutionSession 上增加 PTY；终端通道采用分块与背压
```

### 必须守住的契约

1. **Context 种子与推导分离。** 当前 `ContextProvider` 是由一个 Context 推导下游 Context。Finder 多选、选中文本、浏览器当前页和剪贴板属于激活时采集，应先冻结前台应用身份，再由来源适配器生成种子。多选保留集合身份，并有界展开子文件；不为每个文件重复展示整组动作。
2. **身份分层。** `ActionId` 表示业务动作；`CandidateId` 表示动作与对象的组合；`Launcher SessionId/Revision` 是本次交互的执行凭证；未来 `ExecutionSessionId` 代表可返回的进程/PTY。它们不能互相替代。无法可靠识别的动态菜单不承诺永久稳定 ID，也不允许被长期快捷键静默绑定。
3. **敏感来源可控。** Context 记录来源、所属应用、捕获时刻和访问状态；不把选中文本、剪贴板正文、浏览器 URL 写入使用历史或诊断日志。用户输入的对象与环境捕获对象保留来源差异。
4. **查询与执行各自复验。** 迟到的 Context 或搜索批次须通过 Session/Revision/来源 generation 校验。选中动作执行前仍复验当前凭证、资源和权限。成功完成的副作用之后仍记录历史，即使 Launcher 已隐藏。
5. **快照和长任务分别限额。** Context 数量、Provider 时间、搜索批量、进程并发、日志字节数和终端缓冲区分别设限；不能用同一个“超时”覆盖全部生命周期。xterm.js 官方指出高速输出需要流控。[xterm.js Flow Control](https://xtermjs.org/docs/guides/flowcontrol/)。
6. **窗口职责分离。** Launcher 保持短暂、快速、失焦隐藏；Running Sessions/Terminal 使用独立的持久窗口。V1.0 的“持久”先定义为应用运行期间可返回；跨应用退出继续运行需要额外守护进程，另作决策。

## V1.0 验收定义

- 在无权限、部分权限、无 Context、慢来源和旧结果迟到的情况下，Launcher 仍可搜索应用和执行不依赖该权限的动作。
- Current App 菜单、Finder 选中项、选中文本和至少一种额外环境来源经真实应用验收；新增 Context 经统一 Action、Scope、Ranking 展示。
- Git、Maven、Gradle、Node、Rust 均有真实项目验收；长任务可返回、查看结果和停止，运行输出有界。
- 至少两个第一方模块通过同一内部装配接口工作；一个外部声明式插件与一个宿主 UI 插件使用版本化公开协议，旧版兼容及能力拒绝路径有自动化测试。
- 核心自动化检查、发布包签名、安装升级、权限降级、性能基线和手工交互矩阵都有记录。完成构建不能替代实机验收。

## 1.0 之后的候选方向

Agent CLI 模板可复用执行会话；ACP 在有两个真实 Agent 适配需求时评估。通用 Web 插件需单独解决隔离、导航、能力授权和高吞吐通信。剪贴板历史、浏览器历史/书签、截图、OCR、翻译和 JSON Workspace 按使用场景逐项立项，不并入 Core 主流程。

### 菜单身份与 V0.6 绑定门槛（2026-10-05）

菜单当前采用稳定业务 ID 与扫描执行 token 两层身份；长期语义未经验证，菜单历史暂时隔离。V0.6 不得直接持久绑定这些菜单 ID：先取得可验证身份与迁移规则；其他具有明确业务身份的动作可按计划接入快捷键。详见 [菜单动作安全说明](current-app-actions.md)。
