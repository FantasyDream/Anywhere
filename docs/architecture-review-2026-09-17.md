# 2026-09-17 架构优化实施记录

本轮目标：修复当前并发边界、理清代码职责，为后续功能提供可维护的基础。用户提供的 V0.2.x 交互规格和 V0.3—V1.0 Roadmap 是未来方向，本轮不实施这些功能。当前架构说明见 [architecture.md](architecture.md)。

## 已实施

### 激活生命周期

原实现先异步捕获环境，再分配 SessionId；慢激活可能在新激活之后覆盖会话。现改为同步 `begin_activation` 预留 SessionId，随后 `activate_session(id)`，每个异步边界校验版本。Flutter 在首屏返回前也能取消已预留的会话，旧回调不能取消或覆盖新会话。

实现：[activate.rs](../crates/anywhere-core/src/application/activate.rs)、[LauncherController](../apps/flutter/lib/launcher/launcher_controller.dart)。FRB 公开 API 和生成绑定已同步更新。

### 存储与快照提交

SQLite scoped/global history 在 blocking worker 中读取；失败保留诊断并用空历史继续。搜索/排序移出 session 锁，锁内只提取不可变输入及最终复验/提交。索引和历史使用 Arc 快照，竞争提交重新计算，避免丢失 late context。`latest_state` 只返回有效的已提交版本。

实现：[launcher_service.rs](../crates/anywhere-core/src/application/launcher_service.rs)、[session_runtime.rs](../crates/anywhere-core/src/application/session_runtime.rs)。

### Rust 模块边界

- session model/runtime 移到 Application 并隐藏内部可变状态；内建装配集中到 composition。
- 内建动作的 Handler 与声明归各自 feature，通用代码不再通过命令 ID 或内建执行枚举分派具体行为。现有声明式插件仍共用注册表和执行校验。
- Candidate 不依赖 IndexedApplication，执行目标采用当前两种明确的类型；不预建通用 Invocation 协议。
- Provider、CandidateSource、UsageRepository 保留已有契约；ActionSearch 装配实际存在的匹配和兜底来源。
- 应用发现、路径与本地化读取归 Platform；Core 保留别名、去重和匹配。
- 架构脚本增加 `core/` 不导入 features/application/persistence 的生产模块检查。

实现：[composition.rs](../crates/anywhere-core/src/application/composition.rs)、[ActionHandler](../crates/anywhere-core/src/core/action/handler.rs)、[Candidate](../crates/anywhere-core/src/core/search/candidate.rs)、[Platform](../crates/anywhere-platform/src/lib.rs)。

### Flutter 职责拆分

原页面同时承担 UI、会话、订阅、缓存和原生窗口消息。现拆为 LauncherController、LauncherViewState、WindowHost、ApplicationIconCache 和 CandidateRow。Controller 使用明确阶段管理异步状态；图标 epoch 防止已取消回调填充新缓存。现有输入、Scope 和 Esc 行为保留。

实现：[launcher/](../apps/flutter/lib/launcher/)、[infrastructure/](../apps/flutter/lib/infrastructure/)。旧入口只保留兼容导出。

### 构建与 Impeller

`.flutter-version` 统一为官方 Flutter **3.47.4**；与官方发行清单一致的 framework 为 `9584c6713b324636289d067944a46fd6b49df14b`，engine 为 `06a2e2a110089dff50fe635cffd2a61e1b24fbcd`，Dart 为 3.13.3。本地使用 PATH SDK，不再优先使用旧 `.tools/flutter` 快照。构建和检查脚本校验版本并打印 revision，CI 从同一文件取版本。

Windows workflow 增加 macOS/Windows 的 Rust 与 Flutter 校验，成功后才打包。macOS plist 与 Windows runner 显式启用 Impeller。

## MVP 取舍与未来接入点

保留五个 crate、一个搜索/排序/动作注册链和顺序 Context Graph。当前没有拆 feature crate、引入数据库线程池、消息总线、Provider scheduler 或 Flutter 状态框架。

- **V0.2.x 交互**：未来集中修改 Controller 的入口、Scope/Query/Esc 转移及 Rust Scope 策略；当前 All、单入口、输入后回 All 与 Esc 关闭仍保持。
- **V0.3 Current App Actions**：菜单发现放 Platform，动作来源在 feature 注册；届时用真实菜单句柄和失效模型决定动态 Handler 的数据结构。现在不抓菜单，不承诺稳定动作 ABI。
- **V0.4/V0.5 Context 扩展**：复用 Provider + Bundle。实际需要 Selected Files、Selected Text、Browser URL 或开发工具时，再增加类型与平台能力。
- **V0.6 Extension 化**：届时定义版本协商、加载/卸载、权限和稳定 API；现有 Handler 是内部接口。
- **V0.7 UI**：届时先设计 Host-rendered UI 协议，需要 WebView 时再隔离运行环境。
- **V1.0+**：Clipboard Manager、File Search、Browser History、OCR、翻译、远程与 AI、Marketplace 均未预建。

应用索引仍每进程一次，刷新/TTL/监听留待需求；自定义 macOS Host 继续使用，原生插件增多后再评估标准 Xcode runner。

## 验证

新增 Facade 回归覆盖激活乱序、首屏前取消、慢候选来源不占用 session 锁、慢存储不阻塞 async 取消、历史错误诊断，以及注册新 Handler 后经过公共 Facade 执行。新增 Flutter 回归覆盖取消中的激活、dispose 和图标旧响应；原有 18 项 widget 场景继续保留。

最终验证结果：

- `scripts/check.sh` 通过：119 项 Rust 测试（含 38 项 Facade 场景）、Rust fmt / Clippy、Cargo 与模块架构检查；Flutter analyze 无问题，21 项 Flutter 测试通过。
- 修改后的 FRB API 使用 2.11.1 生成器重新生成，bridge crate 编译通过。工作流 YAML 和 shell 脚本语法检查通过。
- 官方 Flutter 3.47.4 下 `scripts/build-macos.sh` 成功生成 arm64 Release 包，`codesign --verify --deep --strict` 通过，产物 `FLTEnableImpeller=true`。
- 重启到该包后日志确认 Impeller/MetalSDF。真实窗口截图与 AX 状态确认：`chrome` 搜索及图标、逐字删除、`true` JSON 动作及格式化预览、方向键选中、空结果高度收缩；Esc 后返回 `noWindowsAvailable`。
- 输入时日志有系统 IMK mach-port 提示，上述英文输入场景未见故障；中文 IME 组合输入、反复热键唤醒和长时间压力回归尚未验证。
- 本次未推送，Windows 远端 CI 与 Windows 实机尚未运行，不能以 macOS 检查代替。短程 Impeller 验证不能证明历史引擎崩溃根因已修复。
