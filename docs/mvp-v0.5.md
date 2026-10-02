# V0.5 MVP：Developer Actions 与可返回的进程会话

日期：2026-10-02。分支 `dev-0.5`，基线为 `dev-0.4` 的 `b622811`。Rust 开发版本 `0.5.0-dev.1`，macOS App `0.5.0+4`。本阶段先完成一条真实任务流程，再逐项扩展项目类型。

## 第一切片：已实现

```text
Finder / 用户输入的 Directory
  → RustProjectProvider（只检查 Cargo.toml 元数据）
  → project.rust
  → Cargo Check / Cargo Test
  → ExecutionSessionManager
  → Running Tasks 独立窗口
```

- Cargo Check/Test 进入现有 Current App 总动作列表与 Rust Scope；不新增第二套匹配或排序。Check 可以进入首页推荐，Test 通过搜索或 Rust Scope 使用。
- 从目录查找最近的 Cargo.toml，最多检查 32 层祖先；不执行 Cargo metadata，不读/运行构建脚本。Action 执行前再次检查目录和 manifest。
- 工具按绝对路径启动，参数单独传递，不启动 shell。支持继承 PATH、`~/.cargo/bin`、Homebrew 和系统工具目录；不自动读取 shell 初始化脚本。缺少 Cargo 会报告清晰错误，不创建虚假运行会话。
- 会话支持运行中、停止中、成功、失败、已停止，以及退出码、结束时间、分开的 stdout/stderr。不同流之间不承诺精确时间顺序。
- 一次最多 4 个运行任务；最多保留 32 个会话，满时淘汰最早已完成结果，不淘汰运行任务。每个输出流保留最后 128 KiB，超限后持续排空管道并显示提示。
- 启动后自动打开任务窗口，可查看输出、停止或移除完成结果。窗口显示失败时保留任务，提示从菜单栏 Running Tasks 返回，不重复启动。
- 关闭 Launcher、切换查询、再次唤醒或关闭任务窗口不会取消会话；正常退出 Anywhere 会结束所属进程组。会话仅在当前应用运行期间保留，不跨应用退出恢复。

## 自动化验收

- 用临时真实 Rust 项目通过 Application 门面执行 Cargo Check，验证 Context → Scope → Action → Session → 退出码流程；Launcher 关闭和再次激活后任务仍保留，使用历史只记录一次启动。
- 无 Cargo.toml 不出现 Rust Scope；manifest 被删除后执行失败，不启动进程。
- 受控进程覆盖大量 stdout/stderr、非零退出码、中文/emoji、字面参数、并发上限、保留数量、停止子进程、移除限制和关闭管理器。
- Flutter 覆盖旧任务结果不覆盖新选择、隐藏停止轮询、操作失败、任务窗口失败状态及移除、Launcher 打开任务窗口失败后仍可返回任务。
- 公共 DTO 已重新生成 bridge；`scripts/check.sh` 已通过：233 项 Rust 测试、52 项 Flutter 测试、Clippy、静态分析、架构与原生检查。
- `build/Anywhere-dev-0.5.app` 的 arm64 Release 构建与固定本机签名校验已通过，包内版本 `0.5.0`、构建号 `4`。这些结果不代表下方实机矩阵已通过。

## 实机验收：待验证

| 场景 | 通过条件 |
|---|---|
| Finder 中选择 Rust 项目目录 | Cargo Check 可发现并启动，工作目录正确 |
| 独立任务窗口首次打开、关闭后重开 | 第二 engine 正确连接同一 Rust Application，输出和任务身份保留 |
| Launcher 与任务窗口切换焦点 | Launcher 按原规则隐藏，任务窗口仍可使用 |
| 正常退出时有运行任务 | Anywhere 与所属构建进程全部退出，无残留子进程 |
| Cargo Test 在真实项目执行 | 成功、测试失败与用户停止均显示正确状态 |

构建通过不能替代这些窗口、焦点和退出场景的实机验收。原有 0.4 App 包保留，开发包单独输出到 `build/Anywhere-dev-0.5.app`。

## 后续切片

1. 完成任务窗口实机验收，处理发现的生命周期问题。
2. Maven：轻量识别 pom.xml，优先项目 wrapper，固定 Test/Package 动作。
3. Gradle：轻量识别 build.gradle / build.gradle.kts，优先 wrapper，固定 Build/Test。
4. Node：有界读取 package.json，只暴露明确声明的常用 scripts，并处理缺失脚本/工具。
5. Rust：根据实际需要增加 Run，确定多 binary / workspace 选择策略后再开放。
6. 评估现有 Git 长输出是否进入会话；复用已有 Git Actions。

每个项目类型必须有真实项目执行验收，Provider 不执行构建命令。PTY/交互式终端、任意自定义 shell、Spring Boot 多模块分析、后台守护进程不属于第一切片。V0.5 整体完成需上述声明的项目类型和任务生命周期实机验收通过。
