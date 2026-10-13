# 扩展与声明式插件

当前实现：V0.8；最后核对：2026-10-13。本页区分第一方 Rust 模块和外部 TOML 插件。两者都复用正常 Context、Action、Candidate、Ranking 和执行链路。

## 第一方模块

JSON、URL、Git、Maven、Gradle、Node、Rust、Spring Boot、收藏目录的 Manifest 位于 `extensions/`，实现位于 `crates/anywhere-core/src/extensions/`。metadata 在编译时嵌入，不从用户目录加载任意 Rust 动态库。

`FirstPartyExtension` / `FeatureContribution` 贡献 Provider、ActionBundle、动态动作及 runtime Sources。Host 审批能力并提供 HostServices / 隔离 Storage；有界 ExtensionState 可在 runtime 重建间共享。内部 Trait 不是稳定外部 ABI，也不构成进程沙箱。

模块开关同时撤销识别器、动作与来源。重载推进目录 / 执行版本、失效保留会话并重新校验快捷键；已启动任务继续独立运行。架构详见 [装配与生命周期](architecture.md)。

## 外部插件安装

支持 `~/Library/Application Support/Anywhere/plugins/*.toml` 和 `<plugin>/manifest.toml`。安装或修改后在 Settings → Extensions 点击 Reload。

插件只描述匹配与受限操作，不提供 Flutter Widget，也不运行 Shell、Node、Python、WASM 或任意进程。没有 Marketplace 或通用插件 UI runtime。

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

Settings → Extensions 的 Review permissions → Allow & enable 批准后才注册。审查弹窗初始聚焦 Cancel；Enter 取消，右方向键转到 Allow & enable 后 Enter 批准。批准绑定内容、版本与安装路径，插件变化需要重新批准；Revoke permissions / Disable 撤销对应候选和快捷键，保留用户绑定配置。

执行持有不可变批准，HostServices 每次调用检查；产品能力令牌不代替 macOS 权限、授权目录、Session / Revision 或资源复验。

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

命令注册身份为 `<plugin.id>.<command.id>`。标识使用合法的小写片段，`core` 命名空间保留；matcher 使用命名空间类型，如 `core.text`、`git.repository`。`core.json` matcher 归一到 `json.document`，业务动作 `core.json.*` 保持兼容。

可选 `group_id` 决定领域 Scope，默认 `plugin.<plugin_id>`；`aliases` / `keywords` 参与确定性查询匹配。插件不能通过 Group 改写 Context 来源或执行对象。

唯一模板变量为 `{{context.value}}`，可重复使用。未知变量、非法括号、超长字段 / 模板 / 插值结果拒绝。复制和文件路径保留原值；URL 中变量按 UTF-8 percent-encoding，整个模板就是该变量时直接验证原值为合法 HTTP(S) URL。

未知字段、命令 ID 冲突和不兼容的内建目标都拒绝。完整语法与限制以 `anywhere-plugin` 的校验代码 / 回归和 [示例目录](../plugins/examples) 为准。

## 持久化与恢复

`extensions.json` 保存模块状态和 v2 批准，模块数据通过 `extension-storage.sqlite3` 按 namespace 与额度隔离。Storage 的事务更新防止多个连接覆盖已提交收藏。

配置损坏 / 未知 schema 不覆盖原文件，模块关闭且只读，基础 Launcher 和设置仍可用。修复配置或存储后 Reload 重开连接、刷新状态。任何配置操作返回失败时也同步实际快捷键目录，不确定资格不继续注册旧按键。

收藏目录最多 100 项 / 64 KiB，普通查询始终找目录；对象 Actions 的首项保持默认打开操作，移除通过明确选择 Forget Saved Directory；`forget` / `remove` / `unpin` 不被解析为隐式删除指令。

当前实现与发布门槛见 [V0.8 MVP](mvp-v0.8.md)。历史 v1 / 版本迁移验收保存在 [归档](archive/README.md)。
