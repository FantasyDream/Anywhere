# 跨层正确性、资源与维护性审查（2026-10-30）

修复前审查基线：`codex/dependency-upgrades@c98a8e7` 加上当前尚未提交的 Build 36 元数据与 2026-10-29 六项修复。没有回退工作区，也没有把旧问题重新列为未修复。

本轮检查了 Launcher / 工具页 / 设置的交互与异步状态、应用历史 / 剪贴板查询、任务窗口与输出、Web UI 生命周期 / SDK / 包管理、Rust Context / Action / 快照 / 执行、项目识别、持久化、原生窗口 / 预览和现行文档。检查方式为入口与调用链审查，针对可疑时序增加受控探针；不是全仓每行均已验证的声明。

## 确认的问题

### R1 / P2：设置目录更新存在两个并发入口，旧响应可以覆盖较新的开关状态

位置：`apps/flutter/lib/settings/settings_controller.dart:79`、`:549`、`:565`、`:597`。

普通功能开关通过 `_runExtensionOperation` 检查 `_updatingFeatures`，但应用历史的 `onCatalogChanged` 直接调用 `_updateExtensions`。两个异步操作共同写同一个布尔忙碌标记、功能列表和快捷键目录；任意一条的 finally 都会把标记清为 false，即使另一条还没结束。功能列表提交只检查 disposed，没有操作代数或统一队列。

受控复现顺序：关闭功能 A 已在后端提交，但暂不交付其响应；应用历史设置更新并读回 A 的关闭状态，提前解除 busy；此时再次开启 A 被接纳并显示开启；最后交付第一次关闭的响应，UI 又显示关闭，后端实际仍为开启。探针结果：`busy_released_early=true / admitted_writes=2 / final enabled=false`，期望最后仍显示 true。

这是状态所有权问题，单独给回包加 mounted/disposed 检查不足以修复。建议统一功能目录操作与来源变更后的目录协调入口，明确排队及发布版本，忙碌寿命覆盖全部协调；不要只增加另一组独立重试或布尔标记。原生 / Rust 的执行资格仍复验，本探针没有证明旧权限能绕过授权。

### R2 / P2：后台 QuickJS 输出限额仍在创建整份 Rust String 后检查

位置：`crates/anywhere-core/src/application/web_plugins/background_runtime.rs:123–145`。

应用历史转换已经改为借用 VM UTF-8 缓冲区，但后台 Runtime 仍调用 `promise.finish::<String>()`，之后才检查 256 KiB 输出限额。Context resolver 与显式后台命令共用这条路径，因此自动识别也能触发限额之外的临时 Rust 分配。

直接复用生产 Runtime，输入 null、导出返回 `'x'.repeat(8 * 1024 * 1024)`。结果正确拒绝，但分配器记录到最大单次 Rust 分配 **8,388,610 字节**，输出上限只有 **262,144 字节**。没有打开 WebView、读取个人数据或执行宿主能力。

建议先完成 Promise 为 JS 字符串句柄，核对 UTF-8 字节数，再直接解析有界内容；让两个运行时在输出边界上遵守同一规则，保留各自执行及 VM 预算，不需要合并成通用脚本框架。这里测的是最大单次 Rust 分配，未计算 VM 的 C 堆、进程峰值或 RSS，也不是持续泄漏证据。

### R3 / P2：取消插件导入会丢掉尚未返回的首次安装列表

位置：`apps/flutter/lib/web_plugins/web_packages_section.dart:49`、`:56–57`、`:121–129`。

进入导入流程时就推进 `_generation`，使首次 `_refresh` 失效；用户取消文件选择或没有完成安装时，finally 只恢复按钮，没有补读列表。于是原本合法的首次列表响应永久被丢弃，本页继续显示空列表，必须离开页面后再进入才能恢复。

受控 Widget 复现：挂起首次 list；点击 Import；选择器返回 null；最后返回包含 Existing tool 的列表。页面中该条目数量为 **0**，期望 **1**。已有回归分别覆盖“安装完成拒绝旧列表”和“取消选择器恢复按钮”，但没有覆盖这两者组合的时序。

建议到实际发起安装变更时才撤销旧列表，或确保未变更的退出路径重新加载权威列表；继续保留旧读不能覆盖已完成安装的守卫。

### R4 / P3：应用历史的待刷新循环会绕过新输入的防抖，重复读取同一查询

位置：`apps/flutter/lib/features/application_history/session.dart:123–145`。

旧请求运行时，一次已经到期的防抖将 `_again` 置为 true。用户随后继续编辑并启动新的 120 ms 防抖；旧请求返回后，循环只检查 `_again && interactive`，不检查新防抖是否仍活跃，立即读取最新文本。新计时器稍后又读取同一文本。响应版本仍能阻止旧结果发布，但不能阻止这份重复工作。

探针挂起初始查询，输入 a 并推进 120 ms，再输入 ab；在新的防抖开始 20 ms 后放行初始查询。尚未到期就已有第二次读取；最终调用为 **[空查询, ab, ab]**，期望 **[空查询, ab]**。

建议待刷新循环在新防抖仍活跃时停止，由计时器提交最终文本。剪贴板会话已有相应判断，可保持同样语义，不必抽象一个通用查询框架。额外读取会重复来源 I/O、解析与 QuickJS 转换；没有据此声称实机延迟或 RSS 已量化。

### R5 / P3：现行插件架构说明与实际自动识别契约相互矛盾

位置：`docs/web-plugin-design.md:204`；对照 `docs/plugin-development.md:170`、`crates/anywhere-core/src/application/web_plugins/background_provider.rs:87–98`。

架构页仍声明“第三方 JS 不加入自动输入识别器”，开发指南则明确第三方文本解析器参与默认输入识别。当前注册代码确实为接受 core.text 的后台 resolver 安装输入识别器，以 75 ms 预算调用 `resolve_job`。

需要区分 Web 命令的宿主格式匹配和获准后台 resolver 的自动识别：前者不执行 JS，后者会在显式命令执行前运行受限脚本并读取获准投影。不能把前者的“用户执行后交付 Invocation”描述直接用来说明全部后台解析器。建议按现行代码统一架构 / 开发指南，不再保留两套相反的能力边界。

## 设计与可维护性判断

- 设置目录问题应收敛到单一异步状态写入者；单一布尔标记无法表示来自不同入口的重叠操作。
- 限额检查要在所保护的 Rust 大分配之前，失效版本要在实际变更 / 发布边界；防抖要约束下一次真实读取，而不是只约束 UI 通知。
- 本轮没有证据支持重写 Registry、引入另一套插件框架、增加通用页面 schema 或把全部控制器合并。Core / Application / Features / Persistence / Platform / Bridge / Flutter / Host 的现有边界应保留。
- 原生焦点、预览、渲染与进程资源需要单独的实机证据；本轮不以源码审查提出切换 Impeller 或改写窗口生命周期。

## 验证与证据

- 当前工作区复跑上一轮相关 Flutter **83 项**及 SDK **29 项**均通过；本轮新增的 3 条 Flutter 缺陷期望均失败，分别对应 R1、R3、R4。
- `build/review-oct30-probe_test.dart` / `build/review-oct30-probe.log`：受控 Future / Widget 时序，仅使用合成后端与内容。
- `build/review_background_memory.rs` / `build/review-oct30-background-memory.log`：通过临时 Cargo example 直接调用生产后台 Runtime，记录 Rust 最大单次分配；临时 example 已移出源码树。
- 基线回归日志：`build/review-oct30-flutter-baseline.log`、`build/review-oct30-sdk-baseline.log`。
- R5 为当前代码与专题文档的交叉核对，不是依赖旧构建说明得出的推断。

本轮只新增审查记录及索引，未修改业务实现、提交 / 推送或重新打包。没有重新运行完整工程门禁；上一轮完整通过结果仍属于其原记录。本轮未进行原生 GUI / 物理输入法、整 App RSS / 峰值、远端 CI 或 Windows / Linux 实机验收。所有原始探针材料保存在被忽略的 build 目录，不随源码交付。


## 后续修复与验证（2026-10-30）

用户要求修复后，R1–R5 已在 `codex/dependency-upgrades` 工作区完成；上文的问题与未修复说明保留为初始审查记录。

| 问题 | 最终处理与回归 |
|---|---|
| R1 | 功能变更与来源变更后的目录协调共用一条队列，忙碌状态直接来自队尾 Future，不再让两个入口各自清布尔标记。队列进入和执行时撤销旧加载，最后一项结束才恢复控件；关闭等待队尾。受控成功 / 失败时序证明来源协调不能越过旧回包、队列未结束时拒绝额外开关，最后可正常开启并保留新状态。 |
| R2 | Promise 完成为 JS 字符串句柄，借用 VM UTF-8 缓冲区检查 256 KiB，再解析为 Rust Value；不创建完整 Rust String。覆盖精确边界、超出一字节、中文 UTF-8、8 MiB 超限、null 与异步导出。原 VM / JS / 宿主调用预算保持。 |
| R3 | 点击导入或取消批准不再使首次列表失效；真正提交安装才推进代数，仍拒绝旧列表覆盖完成的安装。安装响应失败后重新读取实际状态。回归覆盖取消选择、拒绝批准、安装成功拒绝旧读，以及安装失败读回后仍拒绝旧列表。 |
| R4 | 待刷新循环在新防抖仍活跃时停止，由对应计时器提交最新文本。旧读取成功或失败时，新防抖到期前均只有原请求，最终序列均为“空查询、ab”，没有提前读取和重复 ab。 |
| R5 | 架构 / 开发指南统一区分纯宿主 inputMatchers 与安装授权后的后台只读识别器，明确后者可以在显式命令之前运行，以及预填探测不能直接授权执行。 |

分配复测沿用相同生产后台 Runtime 和 8 MiB 超限字符串夹具，最大单次 Rust 分配由 **8,388,610 字节降为 176 字节**，仍正确拒绝；这不包含 VM 的 C 堆、总分配或整 App RSS。临时 Cargo example 已移出源码树。

新增的 6 个 Flutter 缺陷用例在生产修复前均失败，修复后通过；另补 1 个安装失败后的权威状态恢复回归。相关 Flutter 子集共 **77 项通过**；后台 Runtime 子集 **4 项通过**。

完整 `scripts/check.sh` exit 0：Core **329 / 1 忽略**、MVP **195 / 4 忽略**、Platform **70 / 1 忽略**、Flutter **483 / 1 跳过**、SDK **29**、JSON **30**；规范、Clippy、架构 / 生成一致性、原生自动回归与文档检查均通过。本轮未改变 Bridge API，未手改生成物；Bridge crate 随完整门禁编译，未单独重复真实动态库 FFI 测试。

原始本地证据：`build/cross-layer-fixes-before.log`、`build/cross-layer-fixes-flutter-targeted.log`、`build/cross-layer-fixes-rust-targeted.log`、`build/cross-layer-fixes-background-memory.log`、`build/cross-layer-fixes-full-check.log`。

仍未提交 / 推送或重新打包，main 不变；现有 Build 36 不包含本轮源码修复。没有新增 GUI / 物理输入法、整 App 内存、远端 CI 或跨平台运行验收。当前机制维护于架构、性能和插件专题指南。
