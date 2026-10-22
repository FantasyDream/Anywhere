> 历史记录：保留当时版本与验收边界，当前行为以[文档导航](../README.md)为准。

# 内置功能模块迁移路线图

确认日期：2026-10-14。工作分支 `dev-0.9`；基线 `e4dbedc`。用户已确认取消第一方热加载，优先内置功能与原生页面，第三方 WebView 插件放到最后。

## 2026-10-16 范围调整

用户确认 JSON 功能整体延期，在 WebView 插件机制完成后以 WebView 插件实现。P08 原生 JSON 保留为已尝试但未验收的开发记录，不作为当前交付目标；P09 增加撤下该原生页面 / 动作 / 快捷键入口与 JSON 自动内容引导的清理。Build 5 对应旧实现；本轮源码已撤下原生 JSON 功能、专用桥接和仅服务它的执行框架，Build 6 关键闭环验收已完成。通用 ToolPageRegistry / Shell、结果 / Detail 导航和内置模块接口继续保留。后续顺序见 [产品路线图](../roadmap.md)。

## 执行方式

按用户 2026-10-14 补充授权自动逐阶段执行，不要求每阶段确认。每次聚焦一个阶段；阶段开始时核对上一阶段记录和当前工作树，完成局部改造与对应验证后记录结果，再进入下一阶段。阶段编号用于续接任务，不要求每阶段增加版本或打包。

- 每阶段一个主要改造点；不同时做目录搬迁、业务功能、页面和配置迁移。
- 先建立新入口并迁移调用，再删除旧入口；桥接代码明确标记在哪个阶段退役。
- ActionId、ProviderId、ContextType、Scope、快捷键、Usage 和 Storage namespace 保持稳定。
- Native 功能不提供加载 / 重载 / 卸载或原生热更新预留；数据刷新、业务取消和执行资格检查保留。
- 新增行为用真实业务路径验证；完整门禁作为跨阶段检查点，最终工具页完成后再打包与实机验收。
- 失败时恢复本阶段源码 / 配置修改；不通过重置工作树丢失此前阶段或用户工作，不覆盖损坏的用户数据。

## 阶段

| 阶段 | 交付范围 | 验收 / 停止条件 | 当前状态 |
|---|---|---|---|
| P01 静态目录入口 | 建立内置功能注册契约与单一静态目录，集中构造器，先兼容旧 FirstPartyExtension | 9 个模块身份 / 顺序不变；重复 ID 和构造器身份不匹配被拒绝；旧功能回归 | 已完成，415 Rust / 238 Flutter 通过 |
| P02 编译期自动注册 | 按模块入口生成目录，加入生成结果门禁；不做运行期扫描 | 新增测试模块不改 Launcher 或中央业务清单；生成确定、冲突失败；继续保持现有顺序 | 已完成，421 Rust / 238 Flutter 通过 |
| P03 类型化描述 | 原生 FeatureDescriptor 替换 FirstPartyManifest / TOML / 独立版本协商；应用版本统一 | 内置 metadata 不再经过外部协议解析；状态与能力信息一致，公共身份不变 | 已完成，423 Rust / 238 Flutter 通过 |
| P04 类型化业务服务 | 先迁移收藏目录的 repository / service，移除 Any / ExtensionState 状态恢复依赖；收窄服务注入 | 收藏查询、保存 / 移除、并发事务、损坏恢复不变；模块持有明确业务类型 | 已完成，427 Rust / 238 Flutter 通过 |
| P05a 功能配置 | 功能设置与旧外部批准分离，建立 FeaturePolicySnapshot / generation，迁移旧禁用键 | 配置 round-trip、旧文件迁移、失败回滚、未知 schema 不覆盖 | 已完成，436 Rust / 238 Flutter 通过 |
| P05b 策略与资格 | 识别 / Actions / Sources / 快捷键统一按功能策略筛选；停用取消待处理工作 | 开关即时生效；旧候选、迟到结果和隐藏恢复不能越过新策略；已启动任务独立 | 已完成，442 Rust / 238 Flutter 通过 |
| P05c 退役重载框架 | 内置贡献只在启动装配；移除 Native runtime 重建、第一方 Reload、混合 Manager；旧外部适配仅启动装载 | 第一方路径无热加载框架 / Any 状态箱 / NativeProcessRuntime；数据刷新与取消保护仍在 | 已完成，445 Rust / 238 Flutter 通过 |
| P06 轻量识别目录 | 自动填入复用当前启用的轻量识别器，移除对 JSON / URL 的直接接线 | 禁用功能后不会借该识别器进入内容页；选区优先、剪贴板 15 秒和迟到保护不变 | 已完成，449 Rust / 238 Flutter 通过 |
| P07 原生页面注册 | ToolPageShell + 静态 ViewRegistry，迁移内部 Detail，停止外部 UI DSL 扩展 | 新页面不改 Launcher 业务分支；Esc / 焦点 / IME / 15 秒恢复一致 | 已完成，449 Rust / 241 Flutter 通过 |
| P08 JSON 工具页（延期） | 聚合已有 Rust JSON 能力，加入原生编辑、格式化、压缩、复制、错误展示 | 输入 → 处理 → 错误 / 结果 → 返回真实闭环；错误保留输入，副作用正常记 Usage | 原生实现与单元检查完成；真实快捷键未通过，已移出 0.9，待 WebView 机制后重做 |
| P09 收尾 | 清理过渡适配、重复目录 / API、更新文档，全量门禁、性能与实际包验收 | 旧缺陷场景再验证；报告实际包 / GUI / RSS 的证据和未覆盖项 | 已完成：449 Rust / 251 Flutter，Native / Clippy / analyze / 架构门禁通过；Build 6 完成关键闭环、状态恢复与性能采样，完整发行矩阵另行验收 |

P05 拆成三个阶段，避免同时修改配置、查询资格和装配生命周期。P06 依赖已验证的功能策略，P07 保留通用页面，原 P08 JSON 业务已延期。第三方 WebView / 热加载、PTY / Terminal 和 IDEA Recent Projects 不混入当前迁移。

## 阶段记录格式

每阶段完成后在 [V0.9 开发记录](v0.9-development-records.md) 写入：

1. 阶段、基线和实际修改范围。
2. 已完成内容，以及仍存在的过渡接口。
3. 定向 / 全量验证命令、结果和具体失败修复。
4. 数据或 UI 行为是否改变，未验证部分。
5. 下一阶段的准确入口和禁止顺带处理的范围。

V0.9 当前进度以 [MVP](mvp-v0.9.md) 为准。完整设计见 [内置功能模块方案](../research/builtin-feature-modules.md)。

## 当前注册方式（P02）

每个模块拥有 `crates/anywhere-core/src/features/<module>/registration.rs`，提供 Rust 常量 `REGISTRATIONS: &[BuiltinFeatureRegistration]`。一个业务实现可以注册多个功能，例如 Developer 的 5 个功能。业务目录在后续迁移中归并；当前不同时搬迁业务代码。

Cargo build.rs 只在编译时发现这些入口，生成 OUT_DIR 目录；Rust 对声明类型、非法 / 重复 ID 做编译期校验。P03 起构造后校验 FeatureDescriptor 的身份、名称和操作声明，避免注册到另一功能；Host 允许列表与执行检查独立保留。目录按显式 order 再按 ID 排序，迁移阶段的功能顺序保持；延期 JSON 后为 8 个可配置内置功能；`new` 默认 order 1000。运行时没有文件发现、代码加载或定时扫描。

新增 / 移除入口后运行：

```sh
cargo run -p anywhere-core --example generate_feature_catalog --locked
cargo run -p anywhere-core --example generate_feature_catalog --locked -- --check
```

提交生成快照 `src/features/catalog_generated.rs`，供评审与门禁核对；Cargo 编译使用 OUT_DIR 自动生成的目录。`scripts/check.sh`、macOS 打包与 Linux / Windows CI 检查快照，防止遗漏生成结果。新增功能仍需实现自己的业务代码及正常 Rust 模块声明，无需修改 Launcher 或中央构造清单。不使用额外生成依赖或独立 metadata DSL。

模块 metadata 使用 `features::descriptor::FeatureDescriptor` 静态常量，由 BuiltinFeature 的 `descriptor()` 返回；不再新增 `extensions/<id>/manifest.toml`。描述只有稳定 ID、显示名称和操作枚举，构建版本从 `APP_VERSION` 取得。配置 / Trait / runtime 的退役按 P04 / P05 继续，不需要为新描述重新迁移用户数据。

P04 起，Application 持有 FeatureServices 的类型化业务绑定。收藏目录的 repository 知道存储格式 / key，service 提供有界缓存与保存 / 移除 / 刷新 / 打开前校验，Actions / Source 只注入具体 service。过渡 runtime 接收 FeatureServices，不接收任意状态箱或原始 Storage；只在启用并装配时获取 / 刷新 service。数据库 Host 重连失败后新绑定不复用旧 service。配置与整个 runtime 重建流程继续按 P05a–P05c 迁移，不能把 P04 写成框架已经全部退役。

P05a 起内置配置独立为 features.json / FeaturePreferences，外部开关和批准留在 extensions.json / PluginPreferences。首次创建新文件时迁移旧禁用键并保留原文件，使用不覆盖已有目标的写入；新文件有效后只认新设置。每个设置操作只写一份文件，写前复验、发布前保存，损坏 / 冲突时保留旧策略与会话。FeaturePolicySnapshot 的 generation 只在内置配置或恢复资格变化时发布；外部审批和同策略 Reload 保持其值。统一业务资格接入在 P05b，重载框架退役在 P05c，当前设置 UI 外观未重做。

P05b 起，可配置贡献的 Provider / ActionBundle / 动态 Factory / runtime Actions / Sources 在注册时记录功能归属，基础文件与文本能力不属于可关闭模块。配置发布边界按快照筛选贡献；动态动作继承 Factory 归属，快捷键资格读取同一份有效注册表。Application 与 Manager 安装同一 Arc 策略，请求捕获其身份，提交和执行复验；不使用 ID 前缀或 UI Scope 猜测归属，也不在每次输入时重建整个目录。

策略变化使旧策略下的整个会话 / 请求失效，异步 Provider 等待与 Source 流退出；Graph settlement 也拒绝已完成但未折叠的迟到结果。Git / Developer 的排队 blocking worker 开始前检查请求资格，已经开始的 OS I/O 不强制中断，既有文件 worker admission / 清理和执行资源复验保持。配置保存失败保留旧快照、候选与隐藏恢复；已启动 ExecutionSession 独立。P05c 继续移除现存目录重建框架，P06 才改自动填入轻量识别目录。

P05c 起旧 FirstPartyExtension / ExtensionRuntime / ExtensionManager 和 Reload API 已退役。BuiltinFeature 的服务绑定只在启动执行，BuiltinCatalog 捕获所有编译内贡献；SettingsService 协调分离的 FeatureSettings / LegacyPluginSettings，仅发布策略选择。第二次启动装配同一 Application 被拒绝；装配失败可以重试。收藏目录数据刷新 / 文件修复通过同一个具体 service 更换 repository，不重建动作与来源。设置页称为 Features，Refresh 读取设置与数据。

旧声明式插件仅启动发现与注册。开关 / 撤销立即过滤已经捕获的贡献；首次批准或恢复启动时不可读的外部设置不会运行期装载，须重启激活。Refresh 只复验已有定义和批准，变更或移动安装会失效旧动作，新增文件不会被运行期发现。原授权绑定、数据文件、基础能力、工作取消 / admission、15 秒恢复和运行任务继续保留。

P06 起轻量 InputRecognizer 由 FeatureContribution 声明，与 Provider 同目录、同归属过滤。路径 / URL / JSON 复用原识别函数，按 Provider 优先级 / ID 稳定排序；自动填入不再使用硬编码 JSON / URL 列表。Application 在锁内捕获当前规则 / 策略身份 / catalog 版本，锁外执行识别并复验，配置变化拒绝迟到建议。超过采集 16 KiB 上限的输入不进入识别器，选区优先、剪贴板 15 秒及编辑 / 隐藏保护保持。

P07 起 Flutter 工具页使用静态 ToolPageRegistry、具体 ToolPageSession 和共享 ToolPageShell。Legacy Detail 兼容渲染与内部文本结果都进入共享页框架；Launcher 根据已注册 HostView 打开页面，不处理 Detail 字段。数据与编辑会话由 Launcher 15 秒生命周期持有，返回 / 过期 / 销毁释放，底层查询与选中项保持；页内焦点 / IME / Esc / bare Tab 规则和重复 Host show 的 editor 连接统一。

原 P08（现已延期）加入过可搜索 / 绑定快捷键的 JSON Workspace，使用统一 core.launcher 工具能力和 NativeTool 请求，不新增 Launcher 领域分支。格式化 / 压缩的原 ActionId 保持，打开同一原生编辑页并携带原输入 / 处理结果；复制格式化 JSON 的原行为保持。编辑、格式化 / 压缩、复制结果和错误行列闭环由 Rust 处理，失败保留输入。FeatureOperation 统一 session / policy / epoch、实际 worker admission 与 Usage；关闭页撤销排队工作并保持父查询。输入 256 KiB、结果 512 KiB，序列化写入有界缓冲而非先生成巨型字符串。

P09 将第一方领域代码与注册入口统一到 features/，BuiltinFeature / FeatureBindings 放在 module.rs，Host 许可适配放在 host_services.rs；移除 src/extensions 根目录。通用工具 EntrySource 从文件领域移到 features/tools.rs，新原生工具不依赖文件搜索实现。迁移只调整源码归属，不更改动作身份或持久配置命名。
