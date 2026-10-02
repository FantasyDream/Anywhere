# Context-Aware Launcher

Context-Aware Launcher：先理解当前对象，再推荐下一步动作。Host 是 Flutter，业务和动作执行在 Rust；SQLite 只保存命令使用次数与时间。

```text
Understand → Enrich → Match → Rank → Execute
```

在 Finder 中按 **⌘⌥Space**，首页显示少量推荐动作；Current App 可以搜索当前环境全部动作，Folder / Git 等 Scope 用于缩小范围。输入 `git sta` 可执行 Git 动作，输入 URL、JSON 或路径会得到相应操作；通过 Universal 入口或清除 Scope 后，输入 `chrome` 可搜索应用。菜单层级通过 **Browse Menus…** 探索。

macOS 应用可在本机编译。

## 构建与运行

需要 macOS 12 或以上、PATH 上的 Rust stable、与 `.flutter-version` 相同的 Flutter stable，以及 Apple Command Line Tools。项目使用程序化的 Flutter macOS Host，构建脚本直接调用 Flutter assemble、swiftc 和 codesign；不依赖 CocoaPods 或 Xcode GUI 工程。生成物为本机架构的本地签名应用，不是跨架构发布包。

```bash
# 在仓库根目录；使用系统已安装的 cargo / flutter。
source scripts/env.sh
scripts/setup-local-signing.sh # 仅首次：在登录钥匙串创建本机签名身份
scripts/build-macos.sh
scripts/run-macos.sh
# 也可以在 Finder 双击 build/Anywhere.app
```

`scripts/env.sh` 只设置本机 clang/swift 模块缓存，不再改写 `CARGO_HOME`、`PUB_CACHE` 或把项目内 `.tools` SDK 放到 PATH 前面。

辅助功能授权与代码签名身份关联。构建脚本现在使用登录钥匙串中长期复用的 `Anywhere Local Code Signing` 身份，不再每次用随代码变化的临时签名；从旧签名切换过来时需重新授权一次。同一台 Mac 上重建或替换应用后的授权保留情况仍待实机验收。请勿删除该钥匙串身份。已有 Apple 签名身份时可设置 `ANYWHERE_CODESIGN_IDENTITY`；发布给其他 Mac 的应用应使用 Developer ID 签名，本机自签名身份只供本机开发使用。

macOS Host 显式启用 **Impeller**。

首次启动自动显示一次，之后留在菜单栏。**⌘⌥Space** 打开 Current App Launcher，有当前应用身份且存在可见动作时默认选择其 Scope；加 **Shift** 打开 Universal Search，不选择 Scope。两个入口共用窗口，切换入口保留查询；重复同一入口关闭。失去焦点隐藏，重新唤醒清空输入并重新捕获上下文。

界面由搜索框、Scope Bar 和结果列表组成。Current App Scope 聚合当前环境全部可用 Action，包括菜单、Folder、Git、剪贴板及输入识别动作；其他 Scope 按 Action Group 筛选。只显示有可见动作的入口，没有 All 或 Menu Scope。Context 决定动作是否适用，Scope 决定查询范围。当前应用菜单归 `app.<bundle_id>` 分组，顶部显示实际应用名；目录、Git 等分组可作用于多个 Context。

**↑ / ↓** 选择，**Enter** 执行动作或进入子菜单；**Ctrl+Tab / Ctrl+Shift+Tab** 循环可见 Scope，None 不参与。切换 Scope 保留查询，输入不会自动退出 Scope。**Esc** 依次清查询、返回菜单、清 Scope、关闭。空输入时 **→ / ←** 可进入/返回菜单，输入非空时左右键保留光标功能。

空查询只显示少量推荐动作，不显示 Applications；应用首页最多 7 个推荐动作和 **Browse Menus…**。输入后扁平搜索整个 Scope，Browse Menus 按菜单层级探索；子菜单里输入同样搜索整个应用，清空后回到原浏览位置。菜单行采用叶子标题与菜单路径副标题。无匹配时 Enter 保持 Scope；Current App 搜索全部可用 Action；Esc 清 Scope 或使用 Universal 入口后还可搜索 Applications。结果列表最多露出 7 行，更多内容可滚动，窗口高度上限 520。

Current App 固定首位槽位，只有可见动作时才显示；其他分组按真正执行产生的 frecency、base_priority 和稳定 ID 排序。Session 内已有槽位冻结，迟到分组只追加。浏览与切 Scope 不增加使用次数。不可用菜单继续 Hidden，权限失败只影响依赖该权限的动作。

Finder 目录捕获需要 macOS 的 Automation 授权。拒绝授权时保留输入功能并显示诊断；「最近使用」「共享」等虚拟页面没有目录上下文，不作为错误显示。目录上下文在每次唤醒时捕获一次，绑定当时的前台应用，并在该 Session 中保持不变；如果 40ms 内没读完，首屏先用已有 Context 显示，结果到达后再追加。

V0.4 的 Finder 选中项、选中文本和浏览器 URL 也以唤醒时的前台应用实例为目标异步读取。应用身份在显示 Launcher 前冻结；具体值以系统读取时为准，浏览器在等待期间跳转时可能得到跳转后的 URL。

Settings → Context Sources 可独立关闭四种来源，并查看上次唤醒的采集状态。无选区、不支持、超时、内容过长、选中项过多和权限拒绝分别处理；首页只提示需要处理的问题，拒绝授权时可打开系统设置，处理后重新唤醒即可重采集。关闭窗口后只保留状态元数据供设置页查看，不保留旧 Context 正文。

剪贴板只读取一次文本快照，内容上限 16 KiB；已知系统策略拒绝时单独提示权限，无法可靠确定原因时报告不可用，不猜测权限。来源失败不阻塞其他动作。Copy URL 与 Copy Path 在剪贴板已包含相同目标时隐藏；不同 URL 的打开动作保留来源标注，相同目标合并。

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

`cargo test -p anywhere-core` 不需要 Flutter、Dart 或可见 GUI。完整检查脚本校验共享 Flutter 版本，并运行 Rust fmt / tests / Clippy、架构检查和 Flutter analyze / tests。

Rust 测试使用真实临时文件和 SQLite；OS 动作由 TestPlatform 记录，测试不会打开浏览器、覆盖剪贴板或运行 Git。V0.5 的进程测试会在临时目录启动受控子进程，并对无外部依赖的 Rust 项目执行 Cargo Check。目前有 17 个内建命令。

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

[架构](docs/architecture.md) · [新版路线图](docs/roadmap.md) · [对象与动作导航设计](docs/action-navigation-design.md) · [V0.3.x MVP](docs/mvp-v0.3x.md) · [Context 模型](docs/context-model.md) · [插件协议](docs/plugin-protocol.md) · [Current App 菜单动作](docs/current-app-actions.md)

`dev-0.4` 已完成工程收尾并提交；实机验收状态见 [0.4 验收记录](docs/v0.4-context-acceptance.md)。当前 `dev-0.5` 开发版本为 `0.5.0-dev.1`（App `0.5.0+4`），已接入独立进程会话及 Rust 项目的 Cargo Check/Test。任务输出、退出码和停止入口位于菜单栏的 Running Tasks 窗口，关闭 Launcher 或任务窗口不会停止任务；退出 Anywhere 会停止任务。本阶段实现、限制与后续切片见 [0.5 MVP](docs/mvp-v0.5.md)。

开发项目识别、文件搜索和插件 UI 属于后续阶段；可执行插件运行时、AI、账户与云同步尚未纳入当前版本。
