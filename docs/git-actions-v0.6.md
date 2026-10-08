# Git 常用动作（0.6.0+22）

日期：2026-10-08。在已有 Status / Log / Diff 基础上增加六个动作，保留 build 19 的 UI、Scope 和 Context 规则。

build 22 进一步加入 Copy Remote URL。IDEA 项目目录读取与其他编辑器/Agent 的探索见 [编辑器与 Agent Context](editor-agent-context.md)。

## 操作

在已识别 Git 仓库的 Context 下，进入 Git Scope 可看到全部动作；Current App Scope 中也可以搜索。支持现有用户快捷键和全局动作快捷键。

| 动作 | 稳定业务 ID | 行为 |
|---|---|---|
| Copy Branch Name | `core.git.copy_branch_name` | 复制当前分支名，不带 `refs/heads/` 前缀 |
| Copy Commit Hash | `core.git.copy_commit_hash` | 复制 HEAD 的完整提交哈希 |
| Copy Short Commit Hash | `core.git.copy_short_commit_hash` | 复制 HEAD 的短哈希，遵循 Git 的缩写配置与唯一性规则 |
| Copy Repository Path | `core.git.copy_repository_path` | 复制仓库根目录，而非当前选中的子目录 |
| Copy Remote URL | `core.git.copy_remote_url` | 复制 origin 的 fetch 地址；没有 origin 时使用唯一 remote |
| Git Branches | `core.git.branches` | 在现有预览页展示本地分支列表 |
| Git Staged Diff | `core.git.staged_diff` | 在现有预览页展示暂存区差异 |

复制成功后关闭 Launcher；报告保留窗口进入预览。新增动作不作为默认首页推荐，避免增加首页密度。可搜索中文别名，如“复制分支名”“复制短哈希”“分支列表”“暂存区差异”。

## 数据与执行

- Provider 只识别仓库元数据，查询与快捷键目录不运行 Git。分支和哈希在执行时读取，执行前沿现有 Session/Revision 凭证检查。
- Git 命令为固定参数列表，使用共享 Rust 平台执行器，无 shell、pager、外部 diff/textconv 或 fsmonitor；5 秒截止，输出有上限。新动作不进入长期后台任务系统。
- Branch Name 使用 `symbolic-ref --quiet HEAD`，只去掉 `refs/heads/` 前缀；同名 tag 不影响复制结果。
- 完整/短哈希分别使用 `rev-parse --verify HEAD^{commit}`、`rev-parse --verify --short HEAD^{commit}`。只接受合法哈希；分支与哈希值拒绝截断、非法 UTF-8、空值或多行输出。
- Branches 使用 `branch --list --no-color`；Staged Diff 使用 `diff --cached --no-color --no-ext-diff --no-textconv`。前者只列本地分支，后者与原 Git Diff 的未暂存区分开。
- Remote URL 先用 `git remote` 列名字，再用固定参数 `remote get-url -- <选定名字>` 读取默认 fetch URL。支持 HTTPS、SSH、SCP 形式及本地路径，遵循 `insteadOf` 重写，不使用 push-only 地址，不联系远端。两个命令共享 5 秒截止。多个 URL 使用 Git 默认返回的第一条 fetch URL。
- 命令清除继承的仓库定位环境变量，使用 Context 仓库根目录；linked worktree 读取自己的 HEAD。
- 动作复用 Registry、排序、Usage 和快捷键目录。成功后按原业务 ID 记 Usage，Git 查询失败不写剪贴板、不计 Usage。新功能不持久化分支名、哈希或仓库路径。

## 边界

| 场景 | 行为 |
|---|---|
| 尚无提交 | 可以复制当前命名分支；复制哈希返回明确错误；分支列表可能为空 |
| detached HEAD | Copy Branch Name 返回错误，不复制替代文本；完整/短哈希仍可用 |
| 没有暂存变更 | 显示 `No staged changes.` |
| Git 不可用 | Git 命令动作禁用；复制仓库路径仍可用 |
| 无 remote | Copy Remote URL 提示无远程地址，保持剪贴板 |
| 多 remote 且没有 origin | 提示不能确定 remote，保持剪贴板，不猜一个地址 |
| 剪贴板等于仓库根目录 | 隐藏 Copy Repository Path；分支/哈希不预读，因此不按旧剪贴板内容隐藏 |
| 非 UTF-8 仓库路径 | 复制路径报错，不做有损替换 |

仓库识别仍遵循现有 Context Provider 范围；本次不扩展裸仓库或远程 URL 推断，不加入切分支、提交、拉取、推送等写入操作。

## 验证

- 平台临时真实仓库：无提交、detached HEAD、Unicode 分支、同名 tag、配置化短哈希、暂存/未暂存差异及 linked worktree。
- Core 回归：Git/Current App 搜索、快捷键目录与目标解析、跨 Session/重扫身份、根目录目标、成功 Usage、失败不复制、执行时读取新值、缺 Git 时降级、重复仓库路径隐藏。
- build 20 完整 `scripts/check.sh`：286 项 Rust 测试通过，4 项既有外部工具测试 ignored；134 项 Flutter 测试通过。原生回归、fmt、Clippy、架构检查、Flutter analyze 通过。
- build 22 完整检查：291 项 Rust、134 项 Flutter 通过（4 项既有外部工具测试 ignored）。新增真实临时仓库的 remote 缺失/歧义、origin 优先、fetch/push 区分、SSH/HTTPS、insteadOf 与再次执行读取新值；新动作纳入 Core 的快捷键/搜索/身份/Usage/缺 Git 回归。
- 系统剪贴板与新版 App 的真实 GUI 使用未在本轮人工验收；开发包构建和签名记录见 [V0.6 收尾](v0.6-final-acceptance.md)。
