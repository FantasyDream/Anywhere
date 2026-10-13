# Context 模型

当前实现：V0.8；最后核对：2026-10-13。Context 表示被操作的数据或环境，Scope 表示搜索范围，二者不互相替代。

## 数据与来源

Core 的 `ResolvedContext` 包含 id、type_id、typed ContextValue、source、provider_id、confidence、depth、source_context_id、capture、session_id 和 revision。

ContextValue 覆盖 Text、URL、保留 raw / parsed 的 JSON、File、FileSelection、Directory、Project、Repository、Application 与 Extension 文本。正文通过共享不可变引用传递；Project 使用明确的目录与安全属性，而非在 Flutter 中拼动态字典。

传输层使用字符串类型 ID、值与 properties，不暴露内部 enum 或 AX 对象。来源保留 UserInput、ApplicationContext、FinderSelection、SelectedText、BrowserCurrentPage、Clipboard 等语义，capture 元数据标注所属应用、捕获时间等信息。

| 类型 | 来源或识别 |
|---|---|
| `core.text` | 非空输入或成功采集的文本 |
| `core.url` | 明确合法的 HTTP(S) URL |
| `json.document` | 可解析的 JSON，保留原文；`core.json` 作为旧 matcher 兼容 |
| `core.file` / `core.directory` | 已存在的文件 / 目录，以及应用目录或文件管理器选中项 |
| `core.file_selection` | 多选集合；保留集合身份并有界派生子对象 |
| `core.application` | 显示 Launcher 前冻结的应用身份 |
| `git.repository` | 从目录有界向上检查仓库元数据 |
| `project.rust` / `project.maven` / `project.gradle` / `project.node` | 对应项目标记与必要的轻量元数据 |
| `java.spring_boot` | Maven / Gradle 项目声明的 Spring Boot 能力 |

## 种子与派生

```text
冻结的应用 / 各采集来源 → 种子 Context

core.text → URL / JSON / File / Directory（可叠加）
core.application → 原应用 Directory（按 Session 单飞）
core.directory → Git Repository / Developer Project
Project → 对应 Action 或动态动作目标
```

采集种子由 Application 的来源协调器管理；ContextProvider 从已有 Context 推导下游节点。识别阶段不运行 Git 报告或项目构建命令；执行放在 Action。

目录绑定原应用实例，不因输入变更重新读取前台应用。应用身份失效时丢弃目录，不猜测新应用。多选、浏览器、文本、剪贴板分别记录 Empty、Unsupported、PermissionDenied、ApplicationChanged、TimedOut 等安全状态，其他来源继续工作。

Environment、Content、Search、FileSearch、CapturedEnvironment 的种子选择见 [输入与默认唤醒](context-input-navigation.md)。当前 Environment 不混入剪贴板 / 选中文本正文；内容自填只解析该输入。

## 文件路径简写

路径识别和文件搜索共用 Rust 的 `anywhere-platform::paths`。开头的 `~`、`~/…`、`～`、`～/…` 展开到当前用户主目录，再验证真实文件 / 目录；输入文本保留原拼写，动作绑定规范化后的真实路径。先验证完整原值，再在不存在时尝试去除粘贴外围空白，避免破坏文件名尾部空格。

主目录未知时不把简写误当作当前工作目录；不执行 shell、变量或其他用户目录展开。路径内部的波浪号保持字面含义，重复分隔符不能让 join 替换主目录根。文件搜索仍复验实际授权范围，简写不扩大权限。

## 图、预算与去重

Provider 按注册顺序规则逐步处理；图的调用记录绑定 provider、输入节点、Session 和 Revision。单次图最多 128 节点、深度 3，输出必须符合 produces 和有限的 0..1 confidence。

首屏默认预算 40ms，已有 pending Provider 可获得额外 grace；预算不代表能中断同步 OS 调用。后台继续解析前后复验版本；被替代的调用不能提交旧结果。未真正启动的调用不能因预算截断而永久漏掉。

等价键保留类型、规范化值和来源；同来源等价节点去重，不把显式输入与环境对象抹成一个来源。候选可合并同目标的来源提示，保留真正执行对象。不可用复制动作仍按剪贴板事实隐藏。

## 身份与执行

内部节点身份 / 派生边只在对应解析版本内使用。业务 ActionId 用于命令历史与绑定，CandidateId 绑定具体执行对象；它们不取代 Session / Revision、动作目录版本或执行 epoch。

同 Revision 的迟到结果使用更高 SnapshotSeq，Host 拒绝乱序。执行复验已提交资格及资源，不能根据展示标题或对象在列表中的 index 猜测目标。动态菜单的长期身份限制见 [菜单动作](current-app-actions.md)。

详细装配和快照机制见 [架构](architecture.md)；当前工程验收与待验收矩阵见 [V0.8 MVP](mvp-v0.8.md)。
