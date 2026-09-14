# Declarative Plugin Protocol v1

插件只声明 Command、Context Type Matcher 和 Action。不依赖 Flutter，不提供 Widget，不运行 Node/Python/WASM 或 shell。

```toml
protocol_version = 1
id = "google"
name = "Google Search"

[[commands]]
id = "search"
title = "Search Google"
subtitle = "Google Search plugin"
accepts = ["core.text"]
base_priority = 20

[commands.action]
type = "open_url"
url = "https://www.google.com/search?q={{context.value}}"
```

注册后的 ID 是 `google.search`。插件和命令 id 使用小写字母开头的点分片段，可包含数字、`_`、`-`；`core` 命名空间保留。Matcher 需要命名空间形式，如 `core.text` 或未来的 `git.repository`，不是封闭的内建类型列表。

| action.type | required field | 行为 |
|---|---|---|
| open_url | url | 插值后校验绝对 HTTP(S)，交给 Platform |
| open_file | path | 插值后交给 Platform，执行时检查文件仍存在 |
| copy | text | 插值后写文本剪贴板 |
| execute_builtin | command_id | 调用匹配该 Context 的内建命令 |

唯一模板变量是 `{{context.value}}`，支持重复出现。未知变量、未闭合或多余的双括号都拒绝。普通复制/文件路径替换保持原值；URL 模板中的变量按 UTF-8 做 percent-encoding，避免空格、`&`、`#` 等改变 query/path 结构。整个 URL 模板恰好等于 `{{context.value}}` 时，原值作为 URL 校验，不整体编码。

插件在 `plugins/*.toml` 和 `plugins/<id>/manifest.toml` 中发现，按路径顺序加载。内置插件先加载，自定义插件后加载，重名拒绝。整个 Manifest 先完成校验与注册检查，再一次性加入注册表，失败不会留下部分命令。

校验 protocol_version、id、唯一命令 id、matcher、action type、required fields、模板、内建目标兼容性；拒绝未知字段。单文件最大 256 KiB、1..100 个命令、单模板最大 64 KiB、插值结果最大 4 MiB、base_priority 为 -50..50。非法文件记录带路径/原因的 diagnostic 并跳过。

MVP 启动时读取插件，修改后重启。没有 marketplace、热更新、可执行 runtime 或插件 UI。
