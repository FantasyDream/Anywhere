# 扩展与声明式插件

兼容协议：v1 / v2 / v3；最后整理：2026-10-23。本页区分内置 Rust 功能模块和外部 TOML 插件。两者都复用正常 Context、Action、Candidate、Ranking 和执行链路。

第一方已改为内置功能模块，按 [P01–P09](archive/feature-module-roadmap.md) 完成迁移。Terminal 延后；新的第三方 Web 插件已进入本地开发预览，详见 [Web 平台设计与进度](web-plugin-design.md)。新的本地 Web 包、SDK 与预览安装入口由上述独立契约管理；本页只记录 v1 / v2 / v3 已实现协议与兼容行为，停止扩展 UI DSL。内置 Manifest 已在 P03 移除，旧 Reload / runtime 已在 P05c 退役；新的 Web 动态装载不会恢复第一方热加载。

## 第一方模块

URL、Git、Maven、Gradle、Node、Rust、Spring Boot、收藏目录、剪贴板实现位于 `crates/anywhere-core/src/features/`。各模块使用静态 FeatureDescriptor 声明 ID、名称和系统操作，registration.rs 编译期注册；内置描述不经过 TOML、外部协议版本或包版本校验，不从用户目录加载任意 Rust 动态库。设置状态使用 App 的 Cargo workspace 版本，内置模块没有独立版本。

`BuiltinFeature` / `FeatureContribution` 贡献 Provider、ActionBundle、动态动作及 启动绑定 Sources。FeatureDescriptor 的操作声明仍需通过 Host 允许列表，每次 HostServices 调用仍检查授权；Storage 的 namespace 仍由 Host 按稳定功能 ID 绑定。P04 起使用 FeatureServices 的具体业务 service，ExtensionState / Any 状态恢复已移除；收藏目录仅 repository 接触绑定存储，动作 / 来源只持有 SavedDirectoriesService。内部 Trait 不是稳定外部 ABI，也不构成进程沙箱。

模块开关同时撤销识别器、动作与来源。策略发布推进目录 / 执行版本、失效保留会话并重新校验快捷键；已启动任务继续独立运行。架构详见 [装配与生命周期](architecture.md)。

## 外部插件安装

支持 `~/Library/Application Support/Anywhere/plugins/*.toml` 和 `<plugin>/manifest.toml`。安装或修改后重启 Anywhere，在 Settings → Features 查看启动时检查的定义。Refresh 只刷新设置与数据、复验已有定义，不扫描新插件。

本节 TOML 插件只描述匹配与受限操作，不提供 Flutter Widget，也不运行 Shell、Node、Python、WASM 或任意进程；v3 的受限数据视图由 Flutter Host 渲染。这些限制不代表 Web 插件尚未实现：Web 包在独立 WebView 中运行页面 JS，使用另一套 Manifest 与批准记录，见[开发指南](plugin-development.md)。当前没有 Marketplace。

## v2：显式能力批准

```toml
protocol_version = 2
id = "example_copy"
name = "Example Copy"
version = "1.0.0"
capabilities = ["clipboard.write"]

[[commands]]
id = "copy"
title = "Copy input"
accepts = ["core.text"]
action = { type = "copy", text = "{{context.value}}" }
```

包版本必须是数值 `major.minor.patch`；Capabilities 必须显式、唯一且在支持集合中。协议版本、命令、matcher、模板、能力不合法时拒绝整个 Manifest，不留下部分注册。

| action.type | 字段 | v2 必需能力 |
|---|---|---|
| `copy` | `text` | `clipboard.write` |
| `open_url` | `url` | `url.open` |
| `open_file` | `path` | `file.open` |

v2 不支持 `execute_builtin`，也不向外部声明式插件开放第一方的 `git.read`、`process.start`、`storage`。

Settings → Features 的 Review permissions → Allow for next launch 批准写入后需重启才注册并激活。审查弹窗初始聚焦 Cancel；Enter 取消，右方向键转到 Allow for next launch 后 Enter 批准。批准绑定内容、版本与安装路径，插件变化需要重新批准；Revoke permissions / Disable 撤销对应候选和快捷键，保留用户绑定配置。

执行持有不可变批准，HostServices 每次调用检查；产品能力令牌不代替 macOS 权限、授权目录、Session / Revision 或资源复验。

## v3：Host Detail / KeyValue（S1）

v1 / v2 的语义保持不变。UI 插件使用 `protocol_version = 3`、数值包版本及 `ui.render` 能力，仍通过 Settings → Features 审查、批准和启用。v3 也支持 v2 三种操作及其对应批准，不开放脚本、进程、Storage 或 `execute_builtin`。

新增 `action.type = "render_detail"`，`view.schema_version = 1`，描述 `title`、可选 `body` 与 `rows = [{label, value}, ...]`。标题和标签为字面文本，body / value 只支持 `{{context.value}}`。用户内容中的花括号、HTML 等作为普通文字，不重复插值、不执行或主动导航。

- 最多 32 行；标题 / 标签 1..200 bytes；body 64 KiB、单行 value 16 KiB、总文本 128 KiB（模板与展开后分别检查）。超限在进入 Host 前返回正常动作错误；仍保留查询供用户修改。
- 未知 UI 版本、字段 / 类型、变量和未声明能力拒绝整个插件；即使直接构造 DTO，注册 / 执行也校验与检查能力。
- 复用现有 Session、Revision、Candidate、能力批准、Usage 与快捷键；批准绑定 UI 定义，修改视图也需要重新批准。撤销 / 设置变更使旧候选、待执行结果与保留会话失效。
- 纯 Flutter 视图沿用 AppStyle 和 Launcher 材质。`↑↓` / `PageUp` / `PageDown` 滚动，`Esc` / Back 返回原查询和候选选择；不引入裸 Tab 操作。隐藏后按既有 15 秒规则保留。
- Detail 当前只读，文本可选择复制（Host 的普通文本操作）；不提供插件回调、表单提交、Markdown 链接或任意 Widget。

完整示例：[Context Detail](../plugins/examples/context-detail.toml)。它不会自动安装。将文件放到插件目录、重启并批准 `ui.render`，再重启激活后，输入可识别内容或在当前 Context 的插件 Scope 中运行 Inspect Context。

## v1：兼容协议

`protocol_version = 1` 继续可读，不要求 v2 包版本或 capabilities，不新增 v2 的确认流程。v1 保持既有 open_url、open_file、copy 和 `execute_builtin` 委托契约；委托仅指向兼容的 `core.*` 动作，继承目标 Availability 与执行资格，不能绕过菜单持久绑定限制。

```toml
protocol_version = 1
id = "example_search"
name = "Example Search"

[[commands]]
id = "search"
title = "Example Search"
accepts = ["core.text"]
action = { type = "open_url", url = "https://example.com/search?q={{context.value}}" }
```

例子不是内置服务，域名需自行配置。v1 是保留的兼容路径，不能据此宣称 v2 允许扩大能力。

## 共用匹配与模板

TOML 命令注册身份为 `<plugin.id>.<command.id>`。标识使用合法的小写片段，`core` 命名空间保留；matcher 使用命名空间类型，如 `core.text`、`git.repository`。`core.json` matcher 归一到 `json.document`，旧业务动作 `core.json.*` 已退役并从内置目录移除，委托该目标将失败；旧 matcher / Usage 迁移与动作 ID 仍保留兼容依据。当前没有内置 JSON Provider 生成该 Context，新的 JSON Web 工具通过 `core.text` 与 `inputMatchers` 匹配，不复用旧动作 ID。

可选 `group_id` 决定领域 Scope，默认 `plugin.<plugin_id>`；`aliases` / `keywords` 参与确定性查询匹配。插件不能通过 Group 改写 Context 来源或执行对象。

唯一模板变量为 `{{context.value}}`，可重复使用。未知变量、非法括号、超长字段 / 模板 / 插值结果拒绝。复制和文件路径保留原值；URL 中变量按 UTF-8 percent-encoding，整个模板就是该变量时直接验证原值为合法 HTTP(S) URL。

未知字段、命令 ID 冲突和不兼容的内建目标都拒绝。完整语法与限制以 `anywhere-plugin` 的校验代码 / 回归和 [示例目录](../plugins/examples) 为准。

## 持久化与恢复

`features.json`（schema 1）保存内置功能禁用 ID，不保存批准或独立版本；读取上限 128 KiB / 1024 个合法 ID。首次启动按旧 `extensions.json` 的 `first-party:<id>` 禁用键迁移，新文件存在后不再重新导入旧值。迁移禁止覆盖另一实例已创建的新文件，旧文件中的键作为备份保留。

`extensions.json` 保留外部插件开关和 v2 / v3 的精确内容 / 安装位置批准，读取上限仍为 4 MiB；旧第一方禁用键不再决定已迁移的内置开关。模块数据继续使用 `extension-storage.sqlite3` 的原 namespace 与额度。Storage 事务更新防止多个连接覆盖已提交收藏。

两份配置分别只读恢复：内置配置损坏时其他外部插件仍可使用 / 设置；新内置配置有效时外部配置损坏也不会关闭内置功能。新文件不存在且旧配置不可读时，无法迁移旧开关，内置功能先只读关闭。未知 schema、非法内容、超限不覆盖；写入前发现对应文件被修改 / 损坏也拒绝旧设置操作。每次只原子替换对应的一个文件，失败保持旧策略 / 目录 / 会话。

修复配置或存储后 Refresh 重开数据连接、刷新状态。任何配置操作返回失败时也同步实际快捷键目录，不确定资格不继续注册旧按键。FeaturePolicySnapshot 的 generation 只随内置策略与恢复资格变化发布；统一资格筛选已接入识别、候选、Sources 和快捷键，旧 Reload 已退役。

收藏目录最多 100 项 / 64 KiB，普通查询始终找目录；对象 Actions 的首项保持默认打开操作，移除通过明确选择 Forget Saved Directory；`forget` / `remove` / `unpin` 不被解析为隐式删除指令。

旧协议迁移与当时发布门槛见 [V0.9 记录](archive/mvp-v0.9.md)，当前整体状态见 [V0.12 MVP](mvp-v0.12.md)。历史 v1 / 版本迁移验收保存在 [归档](archive/README.md)。
