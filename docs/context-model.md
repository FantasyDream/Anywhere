# Context Model

Context 是一等对象，不是 Flutter 的动态字典。

Core 的 `ResolvedContext` 包含 id、type_id、typed ContextValue、source、provider_id、confidence、depth、source_context_id、session_id、revision。内部 ContextValue 对文本、URL、保留 raw/parsed 的 JSON、文件、目录、仓库和应用程序提供 typed helper；Extension(String) 允许新的类型标识。

Protocol 使用字符串 type_id 和 transport value，不序列化 Core 的 ContextValue enum。新增 `git.repository` 或 `github.issue` 不必扩充长期协议枚举。应用名称等附加展示信息放在显式 properties 字段。

| 稳定 type_id | Resolver / Provider |
|---|---|
| core.text | 任意非空输入，保留文本；Provider 链的第一跳 |
| core.url | 明确、合法的 HTTP(S) URL |
| json.document | serde_json 解析成功，保留 raw 原文和 parsed 值（V0.2 规范名） |
| core.file | 标准文件系统 API 验证真实存在的文件 |
| core.directory | 已存在的目录，或激活时的 Finder/Windows 资源管理器当前目录 |
| core.application | 激活前的前台应用 |
| git.repository | 从目录向上找到的最近仓库 root，只读文件系统元数据 |

## V0.2 统一 Provider 链

用户输入先生成 `core.text`，再由 Provider 链推导其他类型：

```text
core.text
└─ core.input_types → core.url / json.document / core.file / core.directory

core.application（激活前的前台应用，调用方在显示窗口前冻结）
└─ platform.current_directory → core.directory（原应用目录，按 Session 单飞）

core.directory
└─ core.git_repository → git.repository
```

目录链挂在 `core.application` 而不是 `core.text` 上：它的输入是那个冻结的前台应用身份，与用户后来输入了什么无关。

识别是可叠加 pipeline：URL 同时得到 Text + URL；JSON 同时得到 Text + JSON；路径同时保留 Text。没有互斥的“输入类型判断”。V0.1 的独立 Resolver 顺序已被 Provider 取代，识别规则本身不变。

`git.repository` 只做有界文件系统检查：向上查找 `.git` 目录或 worktree/submodule 的 `.git` 指向文件。**发现阶段不运行任何 Git 命令**；`git status`、`git log`、`git diff` 只在用户执行对应 Action 时运行。

## First Snapshot Budget

Provider 不知道超时。调用方决定等待多久：

```text
全部 Context 提前解析完成 → 立即提交
否则到达 first_snapshot_budget（默认 40ms）→ 用当前结果提交首屏
剩余结果完成 → 校验 Session/Revision → 追加 Context → 重新生成候选
```

预算到期不会取消 Provider；剩余工作在一个后台任务里继续。因为图是**单个顺序解析循环**，每个版本最多有一个 Provider 调用在飞。新 Revision 取代旧 Revision 时，旧结果在纳入图和提交状态之前被丢弃。

若预算在某个 Provider 调用被 poll 之前到期，调用方会撤销该 `(provider, context, session, revision)` 记录，使这次调用在后续的 late pass 中重试，而不会被永久跳过。

激活时的原前台应用身份在宿主显示窗口之前冻结；目录捕获按 Session 单飞并记忆，输入变化不会重新读取当前前台应用，也不会重复启动 Finder Automation。

## 等价与去重

Context Graph 的等价键是 `(type_id, canonical transport value, source)`。同一来源的同类同值只保留一个节点；显式输入与相同环境对象保留各自来源。不同版本重新分配内部节点 ID，source_context_id 指向该版本中的上游节点。

非空输入只匹配 user_input 来源的对象动作，但 V0.2 允许查询匹配到环境 Context 的动作：粘贴 URL 时仍然只出现该 URL 的动作，输入 `git sta` 时 Git 动作来自环境仓库 Context。

Provider 运行记录由 `(provider_id, source_context_id, session_id, revision)` 唯一确定，因此一个 Provider 可以在不同节点上各执行一次。最大深度为 3，最大节点数为 128；输出类型必须在 produces 中、confidence 必须是 0..1 的有限值。Provider 失败成为 diagnostic，其他候选继续可用。

## 状态版本

`LauncherState` 带 SessionId、Revision 和单调递增的 `snapshot_seq`，以及 `context_complete`。同一 Revision 的 late enrichment 会产生更高的 `snapshot_seq`，宿主据此拒绝乱序快照；跨 Session/Revision 的旧结果永不提交。候选 ID 是业务身份的稳定值（命令 + 规范化对象），不包含列表 index，因此 late rerank 不改变用户的选择；可执行性仍由 Session/Revision 校验。
