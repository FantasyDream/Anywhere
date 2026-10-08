# 编辑器与 Agent 项目目录探索

日期：2026-10-08。开发版本：0.6.0+22。

## 本轮结论

| 来源 | 直接观察或官方依据 | 当前交付 |
|---|---|---|
| IntelliJ IDEA | 本机项目窗口标题包含 `项目名 [~/目录] – 当前文件`；该目录存在且包含 `.idea` | 已接入保守的项目窗口读取；新版实机唤醒仍待确认 |
| VS Code | 本机 AX 窗口标题是 `Visual Studio Code`，没有路径；公开扩展 API 提供 `workspaceFolders` | 可靠方案是 Companion Extension，本轮未实现或安装 |
| Cursor IDE | 编辑器 workspace 与 Agent worktree 不一定相同 | 后续可验证 workspace 适配，不能将其宣称为当前 Agent 的目录 |
| Cursor Agents Window | 本机窗口标题为 `Cursor Agents`，AX 内容只有容器；截图可见项目名与聊天，未见完整路径 | 未自动读取目录；需要当前对话的 cwd/worktree 元数据 |
| ChatGPT / Codex | 工具拒绝访问 `com.openai.codex` 的 UI；官方文档区分 ChatGPT 项目与本地项目，并允许多个文件夹 | 本轮未验证当前可见聊天路径，不作自动接入 |

VS Code 的 `workspaceFolders` 是当前编辑器的 0–N 个工作区目录，`onDidChangeWindowState` 可报告窗口焦点变化；这些 API 由编辑器扩展调用，外部 Launcher 不能直接当作进程 API 使用。[VS Code API](https://code.visualstudio.com/api/references/vscode-api#workspace.workspaceFolders)、[窗口状态事件](https://code.visualstudio.com/api/references/vscode-api#window.onDidChangeWindowState)

Cursor 官方文档明确：Agents Window 中进入 worktree 会创建独立 checkout。因此项目主仓库与当前 Agent 执行目录必须分开；Cloud Agent 也不能被表示成可直接操作的本机目录。[Cursor Worktrees](https://cursor.com/docs/configuration/worktrees)

ChatGPT 的普通项目组织聊天与来源，本地项目可附加多个目录；新聊天默认使用 primary folder，已有聊天可能运行在 worktree。该文档描述产品内项目模型，没有建立供 Anywhere 查询“当前前台聊天目录”的接口。[OpenAI Projects and chats](https://learn.chatgpt.com/docs/projects?surface=app)

## 已实现：IDEA 的保守路径

1. 使用激活前捕获的应用 bundle ID、PID、进程启动身份，只适配 IDEA / Community / EAP 的显式白名单。
2. 在共享平台接口 `application_directory` 的 macOS Adapter 中读取该应用的聚焦窗口标题；不激活窗口、不读取编辑器正文、不扫描全部窗口、不取最近打开项目。
3. 每次读取在对应 AX 对象上设置消息超时，最多 0.1 秒，并按 0.5 秒总截止的剩余时间缩短；应用对象的超时不会自动覆盖窗口。超时设置失败或总截止已过时拒绝继续。读取前后校验应用实例，读取前后聚焦窗口必须相同。标题最大 8 KiB。
4. 跨平台 Rust 解析器只接受标题里的明确项目路径；支持绝对路径、`~/`、空格、Unicode 与方括号；目录必须真实存在，并包含 `.idea` 目录。
5. 目录进入现有 `DirectoryMemo → CurrentDirectoryProvider → Directory Context`，继续推导 Git 和 Developer Context。复用 Scope、Ranking、快捷键和执行，不新增 Scope。

标题格式变化、旧 `.ipr` 项目、未显示目录、非本机目录、项目目录不可读或缺少 `.idea` 时返回无目录，不从当前文件的祖先或项目名字猜测根路径。未授权、实例/窗口变化、超时和过大标题拒绝采集。原有 15 秒剪贴板和捕获内容页面规则保留。

该实现是已观察窗口格式的适配，未宣称 JetBrains 保证标题格式长期稳定。需要高覆盖时应使用 IDEA Companion Plugin 明确上报项目根目录；其公开 API 和跨版本契约仍需单独核对。

## 建议：统一 Editor / Agent Context Bridge

以下是后续设计建议，本轮没有实现 Bridge、编辑器扩展或 Agent 插件。

### 报告内容

| 字段 | 目的 |
|---|---|
| protocolVersion / adapterId / adapterInstance | 协议与报告实例识别 |
| applicationInstance / windowIdentity / focusGeneration | 绑定原前台窗口；适配器须证明关联，不能仅用应用名或“最后一个报告” |
| conversationId / executionLocation | 绑定当前 Agent 对话；区分 local / remote / cloud |
| workspaceRoots[] | 保留多个工作区根目录；本机目录使用明确的 file URI |
| workingDirectory / worktreeDirectory | 当前对话真实执行位置，与主项目根目录分开 |
| activeFileUri / observedAt / expiresAt | 辅助确定所在根目录及判断报告是否仍有效 |

只报告目录元数据，不需要发送聊天正文、代码或环境变量。跨平台 Core 处理版本、类型、过期与多目录；各 Adapter 负责原生窗口关联与路径定位。通信可用仅本机 IPC，配对时明确授予来源权限，有界消息与生命周期管理。

### 来源优先级与安全行为

- 已配对 Adapter 的当前窗口/对话元数据优先；IDEA 标题只能在没有可靠 Adapter 时作保守补充。
- Launcher 抢走焦点后使用本次激活绑定的源窗口快照；迟到报告不能改变本次来源，也不能污染另一个聊天。
- 单目录可直接产生 Directory Context；多目录保留独立对象，当前文件/明确 cwd 可以标记主对象。未知主目录时不自动选 `roots[0]`，执行时仍走现有目标选择逻辑。
- Agent 明确的本机 worktree 为执行目录，项目主仓库保留为独立信息。不能将远端 `/workspace/...` 当成本机路径。
- 关闭窗口、切换聊天、报告过期、实例改变或来源不明确时移除相关 Context；不扫描历史数据库或用截图中的名字反查同名目录。

### 建议顺序

1. IDEA 标题读取完成实机验证。
2. VS Code Companion Extension：上报 workspace 根目录、当前文件和焦点；验证多窗口、多根目录、Remote SSH/WSL、关闭和重启。
3. Cursor IDE：验证兼容的编辑器 API；与 Agents Window 独立验收。
4. Cursor / ChatGPT Agent：仅在确认可关联当前对话的元数据出口后接入；否则提供用户主动“发送当前目录给 Anywhere”的入口，不持续猜测。

## 验证边界

本轮验证了 Git 临时仓库读取、IDEA 项目路径解析、缺失身份拒绝、原生聚焦窗口变化/权限/超时/大小限制。真实应用观察只用于判断字段是否存在；新版 App 的实际 IDEA 唤醒、系统剪贴板和四应用的多窗口矩阵未人工验收。VS Code 扩展及 Agent Bridge 尚未实现。
