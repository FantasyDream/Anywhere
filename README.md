# Context-Aware Launcher

Context-Aware Launcher：先理解当前对象，再推荐下一步动作。Host 是 Flutter，业务和动作执行在 Rust；SQLite 只保存命令使用次数与时间。

```text
Understand → Enrich → Match → Rank → Execute
```

在 Finder 中按 **⌘⌥Space**，无需输入即可得到当前环境的动作：普通目录是 Open Terminal Here / Copy Path / Open Directory，Git 目录则以 Git Status 开头。输入 `chrome` 直接打开应用，输入 `git sta` 找到 Git 动作，粘贴 URL、JSON 或路径仍得到对应动作。

macOS 应用可在本机编译；Windows x64 的 Host、资源管理器适配和一键打包脚本已加入，[GitHub Actions](https://github.com/FantasyDream/Anywhere/actions/workflows/windows.yml) 自动编译并提供 ZIP。Windows 实机验收仍待人工执行。见 [Windows 构建与人工验证](docs/windows.md)。

## 构建与运行

需要 macOS 12 或以上、PATH 上的 Rust stable、与 `.flutter-version` 相同的 Flutter stable，以及 Apple Command Line Tools。项目使用程序化的 Flutter macOS Host，构建脚本直接调用 Flutter assemble、swiftc 和 codesign；不依赖 CocoaPods 或 Xcode GUI 工程。生成物为本机架构的本地签名应用，不是跨架构发布包。

```bash
# 在仓库根目录；使用系统已安装的 cargo / flutter。
source scripts/env.sh
scripts/build-macos.sh
scripts/run-macos.sh
# 也可以在 Finder 双击 build/Anywhere.app
```

`scripts/env.sh` 只设置本机 clang/swift 模块缓存，不再改写 `CARGO_HOME`、`PUB_CACHE` 或把项目内 `.tools` SDK 放到 PATH 前面。

macOS 与 Windows Host 均显式启用 **Impeller**。macOS 新包已通过引擎日志确认使用 Impeller/MetalSDF；SDK 版本和验证范围见 [渲染器决策记录](docs/decisions/004-windows-and-renderer.md#2026-09-17重新启用-impeller)。

首次启动自动显示一次，之后留在菜单栏。**⌘⌥Space** 打开/隐藏；**↑ / ↓** 选择；**Enter** 执行；**Esc** 隐藏；**⌥ ← / →** 在 Scope Bar 中切换分组。失去焦点立即隐藏；重新唤醒会清空输入并重新捕获上下文。无动作时只显示搜索栏，列表随数量增减，最多显示 7 行，更多结果可滚动，窗口高度上限 520。菜单栏可以打开或退出应用。快捷键冲突会显示诊断；释放其他应用对该组合键的占用后重启。

Scope Bar 固定在搜索栏下方：`[ All ] [ Finder ] [ Git ] [ Folder ]`。**All** 默认选中；第二项始终是当前应用，只包含它的动作；之后是当前可见动作所属的动态分组，先按当前匹配动作的使用次数排序，同分时按 git → folder → file → web → json 与稳定 ID 排序。切换 Scope 是硬过滤，候选列表由 Rust 预计算，UI 不判断 Context 类型。

普通目录里 Finder 与 Folder 的内容相同是正常的：Finder 的动作就是它那个目录的动作，两者没有差异。只有在存在其他应用专属动作或 Git 仓库时两个 Scope 才会不同。

Finder 目录捕获需要 macOS 的 Automation 授权。拒绝授权时保留输入功能并显示诊断；「最近使用」「共享」等虚拟页面没有目录上下文，不作为错误显示。目录上下文在每次唤醒时捕获一次，绑定当时的前台应用，并在该 Session 中保持不变；如果 40ms 内没读完，首屏先用已有 Context 显示，结果到达后再追加。

JSON 的 Format / Minify 在窗口中显示结果，↑ / ↓ 返回动作列表；Copy Formatted JSON 写剪贴板。文件和 URL 的打开、复制等操作全部由 Rust 调用 Platform Adapter。

## 检查

```bash
source scripts/env.sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 scripts/check-architecture.py
scripts/check.sh
cargo run --release --locked -p anywhere-core --example measure
```

`cargo test -p anywhere-core` 不需要 Flutter、Dart 或可见 GUI。完整检查脚本校验共享 Flutter 版本，并运行 Rust fmt / tests / Clippy、架构检查和 Flutter analyze / tests。另有真实剪贴板的编码回归入口，见验收记录；它会临时改写并恢复系统剪贴板。

Rust 测试使用真实临时文件和 SQLite；OS 动作由 TestPlatform 记录，测试不会打开浏览器、覆盖剪贴板或运行 Git。真实 macOS 验收步骤和实测结果见 [V0.2 验收记录](docs/validation.md)。

自己验证应用请按 [完整手工验收清单](docs/manual-validation.md) 执行；历史清单中的命令和平台范围应以当前实现为准，目前有 14 个内建命令。

修改桥接公开 API 后重新生成：

```bash
cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked
cargo install cargo-expand --locked
scripts/generate-bridge.sh
```

已生成的 Dart/Rust 绑定随源码交付，普通构建无需安装生成器。

## 14 个内建命令

| 对象 | 动作 |
|---|---|
| Directory | Open Terminal Here / Copy Path / Open Directory |
| Git repository | Git Status / Git Log / Git Diff |
| File | Open File / Reveal in File Manager / Copy Path |
| URL | Open in Default Browser / Copy URL |
| JSON | Format JSON / Minify JSON / Copy Formatted JSON |

应用不内置 Web Search，也不自动安装搜索插件。需要时可修改 [搜索插件示例](plugins/examples/search.toml) 中的 URL 并安装到用户插件目录；示例中的 `example.com` 只是占位地址。插件命令与内建命令共用注册表和排序。

只识别明确的 HTTP(S) URL，不推断裸域名。路径必须真实存在，支持绝对路径；相对路径以进程工作目录为准，不解释 shell 语法或 `~`。JSON 支持对象、数组及合法 primitive，并保留原始文本。输入最大 1 MiB。

Git 动作只在用户按 Enter 时运行，参数是固定的 `status` / `log --oneline -n 100` / `diff --no-color`；发现仓库只做文件系统检查，从不调用 Git。Git 未安装时这三个动作显示 Disabled 并给出原因。剪贴板只在唤醒时读取一次文本快照；当它已经等于当前路径时 Copy Path 隐藏。

## 数据与插件

- 历史：`~/Library/Application Support/Anywhere/usage.sqlite3`。
- 自定义插件：`~/Library/Application Support/Anywhere/plugins/*.toml`，或 `<plugin>/manifest.toml`。
- 示例：[plugins/examples/search.toml](plugins/examples/search.toml)。修改插件后重启应用。
- 日志：`~/Library/Application Support/Anywhere/launcher.log`，包含 Rust 和窗口耗时日志，每次启动覆盖。终端和 Finder 启动使用同一路径。

历史只存 command_id、context_type、source_app、次数、时间，不存输入或路径。成功执行后更新历史，下一次排序生效。频次和最近使用加分有上限，不会压过当前输入的来源优先级。

## 结构与边界

[架构](docs/architecture.md) · [Context 模型](docs/context-model.md) · [插件协议](docs/plugin-protocol.md) · [决策记录](docs/decisions/) · [V0.2 范围](docs/decisions/005-v0.2-minimal-mvp.md)

本轮架构优化的实现、验证和未来 Roadmap 取舍见 [2026-09-17 实施记录](docs/architecture-review-2026-09-17.md)。

本次范围为 macOS 与 Windows 平台适配、内建上下文、应用搜索、Scope Bar、声明式插件与键盘交互。Windows 已有开始菜单快捷方式索引、应用启动、剪贴板读取和 Git 执行的实现；应用图标目前使用回退图标。源码支持与 Windows 实机验收是两个不同状态，后者仍待完成。

没有可执行插件运行时、AI、网络 Provider、剪贴板管理器、浏览器捕获、账户或云同步。V0.2 明确不做 Maven/Gradle/Node/Rust/Spring Boot 识别、完整 App Actions 抓取、JSONPath、JSON Workspace、Finder 选中文件、浏览器 URL、选中文本、文件搜索、浏览器历史、插件 UI、WebView、并行 Context Scheduler、复杂 Provider 分类和 ML Ranking。
