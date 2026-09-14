# Context Model

Context 是一等对象，不是 Flutter 的动态字典。

Core 的 `ResolvedContext` 包含 id、type_id、typed ContextValue、source、provider_id、confidence、depth、source_context_id、session_id、revision。内部 ContextValue 对文本、URL、保留 raw/parsed 的 JSON、文件、目录和应用提供 typed helper；Extension(String) 允许新的类型标识。

Protocol 使用字符串 type_id 和 transport value，不序列化 Core 的 ContextValue enum。新增 `git.repository` 或 `github.issue` 不必扩充长期协议枚举。应用名称等附加展示信息放在显式 properties 字段。

| 稳定 type_id | Resolver / Provider |
|---|---|
| core.text | 任意非空输入，保留文本 |
| core.url | 明确、合法的 HTTP(S) URL |
| core.json | serde_json 解析成功，保留 raw 原文和 parsed 值 |
| core.file | 标准文件系统 API 验证真实存在的文件 |
| core.directory | 已存在的目录，或激活时的 Finder/Windows 资源管理器当前目录 |
| core.application | 激活前的前台应用 |

Resolver 是可叠加 pipeline：URL 同时得到 Text + URL；JSON 同时得到 Text + JSON；路径同时保留 Text。没有互斥的“输入类型判断”。

source 的稳定序列化值为 `user_input`、`foreground_application`、`application_context`。非空输入只匹配 user_input 来源，空输入匹配环境来源；来源同时保留在排名模型中；Provider 对用户输入的 enrichment 必须继承 user_input。

Context Graph 的等价键是 `(type_id, canonical transport value, source)`。同一来源的同类同值只保留一个节点；显式输入与相同环境对象保留各自来源。不同版本重新分配内部节点 ID，source_context_id 指向该版本中的上游节点。ActionId 还绑定 Session/Revision，不依赖节点 ID 的跨版本稳定性。

Ambient context 是激活快照；输入更新会重用该快照，不在 Launcher 已成为前台后重复询问 Finder。非空输入时快照不进入动作匹配，清空后恢复。失焦即结束 Session，每次重新唤醒捕获新的前台和目录。

Provider 运行记录由 `(provider_id, source_context_id, session_id, revision)` 唯一确定。最大深度为 3，最大节点数为 128；输出类型必须在 produces 中、confidence 必须是 0..1 的有限值。过期结果在纳入图之前丢弃，Provider 失败成为 diagnostic，其他命令继续可用。
