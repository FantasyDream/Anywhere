# Git 与开发项目动作

现行专题说明；文档整理：2026-10-23。目录、仓库、项目 Context 复用现有 Current App / 领域 Scope、Ranking、快捷键与执行，不创建 IDE 专用 Launcher。

## 目录来源

Finder 当前目录与选中项、已存在的输入路径以及支持的 IDEA 项目窗口都可以产生 Directory。IDEA 仅在窗口标题明确包含真实目录且存在 `.idea` 时接受；绑定冻结的应用实例和聚焦窗口，前后检查，读取超时 / 标题长度有界。

缺少路径、虚拟位置、不可读目录、窗口变化或旧 `.ipr` 形式不猜测项目根目录。IDEA / Community / EAP 在明确白名单内；VS Code、Cursor Agent 与 ChatGPT / Codex 当前聊天目录尚未实现可靠采集。

未来 Companion Bridge 的 workspace / worktree / conversation 区分保存在 [历史调研](research/editor-agent-bridge.md)，不是当前能力。

## Git

Provider 有界向上检查 `.git` 目录或指向文件，支持已有仓库 / linked worktree 语义，识别不运行 Git。动作执行才运行固定、只读 Git 命令或复制数据。

| 动作 | 行为 |
|---|---|
| Git Status / Log / Diff | 状态、提交记录、未暂存差异 |
| Git Staged Diff | 暂存差异，空时明确提示 |
| Git Branches | 本地分支列表 |
| Copy Branch Name | 当前命名分支，去掉 refs/heads/ 前缀 |
| Copy Commit Hash / Short Commit Hash | HEAD 完整 / Git 配置化短哈希 |
| Copy Repository Path | 仓库根目录，不是选中子目录 |
| Copy Remote URL | origin fetch 地址；没有 origin 时仅接受唯一 remote |

复制成功关闭 Launcher，报告在既有输出页展示。支持中文别名和持久 Action 快捷键；新增复制 / 报告不扩大默认推荐密度。

Git 使用固定 argv、无 shell / pager / 外部 diff / textconv / fsmonitor，时间和输出有界；短查询不进入长期后台任务。执行清除仓库定位环境变量并使用 Context 根，值在执行时重新读取。

| 边界 | 行为 |
|---|---|
| 尚无提交 | 可复制命名分支；哈希报错，剪贴板不改 |
| detached HEAD | 分支名报错；哈希仍可用 |
| 无 remote | 保持剪贴板并提示 |
| 多 remote 且无 origin | 不猜测地址 |
| origin 多 URL | 使用 Git 默认第一条 fetch URL，遵循 insteadOf；不请求远端 |
| Git 不可用 | Git 命令动作禁用，复制仓库路径仍可用 |
| 剪贴板已有仓库根 | 隐藏 Copy Repository Path；分支 / 哈希不预读 |

没有切分支、提交、拉取、推送等写入动作。成功记业务 Usage，失败不记；分支 / 哈希不持久保存。

## 项目动作

| 类型 | 识别与动作 |
|---|---|
| Rust | Cargo 项目标记；Check / Test；Run 按明确 package / binary 目标生成 |
| Maven | pom.xml；Test / Package，优先 mvnw |
| Gradle | Gradle 标记；Build / Test，优先 gradlew |
| Node | package.json 声明 scripts；按 packageManager / 锁文件选择 npm、pnpm、yarn |
| Spring Boot | 项目明确声明插件能力时，提供对应 Maven / Gradle Run |

Provider 只做标记与有界元数据读取，动态 Action 工厂基于已识别 Context 生成明确目标。执行重新检查目录、项目标记、工具和候选资格，不能把未知 shell 文本当作命令。

开发任务进入 ExecutionSessionManager。菜单栏 Running Tasks 查看状态、stdout / stderr、退出码、停止；关闭窗口或 Launcher 不停止已启动任务，退出应用会停止任务。当前只有进程会话，没有 PTY、交互式终端或应用重启后任务恢复。

## 模块与验证

Git 与开发项目属于编译内功能模块，可在 Settings → Features 关闭；识别器、动作、快捷键资格一起撤销。模块静态声明所需能力，命令经 HostServices 检查该声明和 Host 允许列表；不走第三方插件的安装批准 / 热加载流程。操作系统调用留在 Platform。

回归包含临时 Git 仓库、worktree、remote 歧义 / 地址重写、失败不复制 / 不计 Usage、项目识别 / 动态目标、排队失效、进程取消与有界输出。真实 Maven / Gradle / Node / Cargo 工具的历史验收见 [V0.5 收尾](archive/v0.5-closeout.md)；不能据此标记新包的全部 GUI 与工具矩阵已验证。
