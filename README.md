# Context-Aware Launcher

Context-Aware Launcher：先理解当前对象，再推荐下一步动作。Host 是 Flutter，业务和动作执行在 Rust；SQLite 只保存命令使用次数与时间。

```text
Understand → Enrich → Match → Rank → Execute
```

在 Finder 中按 **⌘⌥Space**，无需输入即可得到 Open Terminal Here / Copy Path / Open Directory。输入 URL、JSON、文件、目录或普通文本，会得到对应动作；有输入时只显示该输入的动作，清空后恢复 Finder 环境动作。

macOS 应用可在本机编译；Windows x64 的 Host、资源管理器适配和一键打包脚本已加入，[GitHub Actions](https://github.com/FantasyDream/Context-Aware-Launcher/actions/workflows/windows.yml) 自动编译并提供 ZIP。Windows 实机验收仍待人工执行。见 [Windows 构建与 18 项人工验证](docs/windows.md)。

## 构建与运行

需要 macOS 12 或以上、Rust stable、Flutter（Dart >= 3.6）和 Apple Command Line Tools。项目使用程序化的 Flutter macOS Host，构建脚本直接调用 Flutter assemble、swiftc 和 codesign；不依赖 CocoaPods 或 Xcode GUI 工程。生成物为本机架构的本地签名应用，不是跨架构发布包。

```bash
# 在仓库根目录；已安装全局 SDK 时也可以使用这些脚本。
source scripts/env.sh
scripts/build-macos.sh
scripts/run-macos.sh
# 也可以在 Finder 双击 build/Context Launcher.app
```

本次环境安装的 SDK 在 `.tools/`，已从版本控制排除，未修改全局 PATH / SDK 默认版本。`scripts/env.sh` 在本地 SDK 存在时使用它们。新的机器需要先安装自己的 Rust / Flutter SDK。

首次启动自动显示一次，之后留在菜单栏。**⌘⌥Space** 打开/隐藏；**↑ / ↓** 选择；**Enter** 执行；**Esc** 隐藏。失去焦点立即隐藏；重新唤醒会清空输入并重新捕获上下文。无动作时只显示搜索栏，动作列表随数量增减，最多显示 7 行，更多动作可滚动，窗口高度上限 520。菜单栏可以打开或退出应用。快捷键冲突会显示诊断；释放其他应用对该组合键的占用后重启。

Finder 目录捕获需要 macOS 的 Automation 授权。拒绝授权时保留输入功能并显示诊断；「最近使用」「共享」等虚拟页面没有目录上下文，不作为错误显示。目录上下文在每次唤醒时重新捕获，在该 Session 的输入过程中保持不变。

JSON 的 Format / Minify 在窗口中显示结果，↑ / ↓ 返回动作列表；Copy Formatted JSON 写剪贴板。文件和 URL 的打开、复制等操作全部由 Rust 调用 Platform Adapter。

## 检查

```bash
source scripts/env.sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check-architecture.py
scripts/check.sh
cargo run --release -p launcher-core --example measure
```

`cargo test -p launcher-core` 不需要 Flutter、Dart 或可见 GUI。完整检查脚本还运行 Flutter analyze / widget tests。另有真实剪贴板的编码回归入口，见验收记录；它会临时改写并恢复系统剪贴板。

Rust 测试使用真实临时文件和 SQLite；OS 动作由 TestPlatform 记录，测试不会打开浏览器或覆盖剪贴板。真实 macOS 验收步骤和实测结果见 [验收记录](docs/validation.md)。

自己验证应用请按 [完整手工验收清单](docs/manual-validation.md) 执行；包含 A–F、12 个内建命令、键盘、历史、插件与错误恢复。

修改桥接公开 API 后重新生成：

```bash
cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked
cargo install cargo-expand --locked
scripts/generate-bridge.sh
```

已生成的 Dart/Rust 绑定随源码交付，普通构建无需安装生成器。

## 12 个内建命令

| 对象 | 动作 |
|---|---|
| Directory | Open Terminal Here / Copy Path / Open Directory |
| File | Open File / Reveal in File Manager / Copy Path |
| URL | Open in Default Browser / Copy URL |
| JSON | Format JSON / Minify JSON / Copy Formatted JSON |
| Text | Web Search |

附带 `google.search` 声明式插件；输入 `hello world` 时 Search Google 排在内建 Web Search 前。插件命令与内建命令共用注册表和排序。

只识别明确的 HTTP(S) URL，不推断裸域名。路径必须真实存在，支持绝对路径；相对路径以进程工作目录为准，不解释 shell 语法或 `~`。JSON 支持对象、数组及合法 primitive，并保留原始文本。输入最大 1 MiB。

## 数据与插件

- 历史：`~/Library/Application Support/ContextLauncher/usage.sqlite3`。
- 自定义插件：`~/Library/Application Support/ContextLauncher/plugins/*.toml`，或 `<plugin>/manifest.toml`。
- 示例：[plugins/examples/google.toml](plugins/examples/google.toml)。修改插件后重启应用。
- 日志：`~/Library/Application Support/ContextLauncher/launcher.log`，包含 Rust 和窗口耗时日志，每次启动覆盖。终端和 Finder 启动使用同一路径。

历史只存 command_id、context_type、source_app、次数、时间，不存输入或路径。成功执行后更新历史，下一次排序生效。频次和最近使用加分有上限，不会压过当前输入的来源优先级。

## 结构与边界

[架构](docs/architecture.md) · [Context 模型](docs/context-model.md) · [插件协议](docs/plugin-protocol.md) · [决策记录](docs/decisions/)

本次范围为 macOS 与 Windows 平台适配、内建上下文、声明式插件与键盘交互。没有可执行插件运行时、AI、网络 Provider、剪贴板管理器、浏览器捕获、账户或云同步。
