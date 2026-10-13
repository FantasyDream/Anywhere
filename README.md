# Context-Aware Launcher

Context-Aware Launcher：先理解当前对象，再推荐下一步动作。Host 是 Flutter，业务和动作执行在 Rust；SQLite 只保存命令使用次数与时间。

```text
Understand → Enrich → Match → Rank → Execute
```

在 Finder 中按 **⌘⌥Space**，首页显示少量推荐动作；Current App 可以搜索当前环境全部动作，Folder / Git 等 Scope 用于缩小范围。输入 `git sta` 可执行 Git 动作，输入 URL、JSON 或路径会得到相应操作；通过 Universal 入口或清除 Scope 后，输入 `chrome` 可搜索应用。菜单层级通过 **Browse Menus…** 探索。

macOS 应用可在本机编译。

`dev-0.7` 的文件搜索是普通 **File Search** Action：在 Global Search 输入“文件搜索”或 `files` 查找，也可在 Settings → Shortcuts → Action shortcuts 中设置用户自定义快捷键，并按需开启 Global。空白首页不常驻工具入口；搜索图标菜单可在 Global Search / Current App 间切换。 文件页提供文档、PDF、图片、音频、视频、压缩包、代码等类型过滤（查询时按后缀分类，先过滤再加载）；类型栏与 Scope 共用标签样式，通过 Ctrl Tab / Ctrl Shift Tab 切换；裸 Tab / Shift Tab 不再遍历焦点。结果为按需追加的连续列表，没有分页按钮或页码。预览在 ⌘K / Ctrl K 的二级 Actions 中选择 Preview 打开，Esc / 返回先关闭预览并保留查询。

文件搜索自动结合 Spotlight 快速候选与 Rust 完整文件名遍历，不需要安装 `rg` 或选择引擎。遍历同时搜索文件和文件夹，包含未索引、隐藏（按设置）和被 `.gitignore` 忽略的对象；仅用户明确的排除项缩小范围。Settings → Search 配置范围（空列表为 Home）和隐藏开关，新安装默认包含隐藏文件，保留旧配置。所有命中写入查询临时文件，隐藏超过 15 秒或换查询后清理，不维护长期文件索引。

文件候选按类型使用统一透明线性图标，预览区共用；PDF、文档、表格、图片、音视频、压缩包、源码与配置等有对应形状和淡色。图标按文件名提示分类，不读取正文，也不生成逐文件缩略图。当前预览仍为文本 / 元数据，其他产品调研与后续富预览方案见 [文件预览与图标](docs/file-preview-and-icons-v0.7.md)。

文件页扩展主 Launcher 的搜索栏、结果行和底栏，输入一个字符即可检索，支持字面标点、Unicode 和路径导航。内部按 64 项批次追加，方向键和滚动在同一连续列表中导航；底栏显示已找到数量及搜索中/完成/部分结果。类型栏与预览仅在文件页出现。单击选择，Enter / Open 或双击打开，Actions / ⌘K 提供已有文件操作。Esc 先清空文件查询，再返回原页面；返回按钮直接回父页。普通 Launcher 查询、现有 Scope 与内容输入模式不触发文件检索。Search 设置支持开关、目录与排除项，失败可 Retry。完整规则见 [统一交互模型](docs/launcher-ui-v0.7.md)，验收边界见 [V0.7 MVP](docs/mvp-v0.7.md)。

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

首次启动自动显示一次，之后留在菜单栏。**⌘⌥Space** 打开 Current App Launcher，有当前应用身份且存在可见动作时默认选择其 Scope；加 **Shift** 打开 Universal Search，不选择 Scope。两个入口共用窗口，切换入口保留查询；重复同一入口关闭。失去焦点隐藏，15 秒内重新唤醒恢复原页面、查询、Scope、文件类型和预览；超过 15 秒清理会话，下一次唤醒重新捕获上下文。详见 [状态保留规则](docs/launcher-retention-v0.7.md)。

Current App 唤醒时，如果存在选中文本，或复制时间可确认且距今不超过 **15 秒** 的剪贴板文本，会先进入定制的 Global 内容输入页面并自动填入，只解析该输入的 Context。Esc 先返回 Current App 并恢复环境 Context，再按 Esc 返回空白 Global；不会因自填内容改变 Scope 的定义。

界面由搜索框、Scope Bar 和结果列表组成。Current App Scope 聚合当前环境全部可用 Action，包括菜单、Folder、Git、剪贴板及输入识别动作；其他 Scope 按 Action Group 筛选。只显示有可见动作的入口，没有 All 或 Menu Scope。Context 决定动作是否适用，Scope 决定查询范围。当前应用菜单归 `app.<bundle_id>` 分组，顶部显示实际应用名；目录、Git 等分组可作用于多个 Context。

**↑ / ↓** 选择，**Enter** 执行动作或进入子菜单；**Ctrl+Tab / Ctrl+Shift+Tab** 循环可见 Scope，None 不参与。切换 Scope 保留查询，输入不会自动退出 Scope。**Esc** 依次清查询、返回菜单、清 Scope、关闭。空输入时 **→ / ←** 可进入/返回菜单，输入非空时左右键保留光标功能。

空查询只显示少量推荐动作，不显示 Applications；应用首页最多 7 个推荐动作和 **Browse Menus…**。输入后扁平搜索整个 Scope，Browse Menus 按菜单层级探索；子菜单里输入同样搜索整个应用，清空后回到原浏览位置。菜单行采用叶子标题与菜单路径副标题。无匹配时 Enter 保持 Scope；Current App 搜索全部可用 Action；Esc 清 Scope 或使用 Universal 入口后还可搜索 Applications。结果列表最多露出 7 行，更多内容可滚动，窗口高度上限 520。

Current App 固定首位槽位，只有可见动作时才显示；其他分组按真正执行产生的 frecency、base_priority 和稳定 ID 排序。Session 内已有槽位冻结，迟到分组只追加。浏览与切 Scope 不增加使用次数。不可用菜单继续 Hidden，权限失败只影响依赖该权限的动作。

Finder 目录捕获需要 macOS 的 Automation 授权。拒绝授权时保留输入功能并显示诊断；「最近使用」「共享」等虚拟页面没有目录上下文，不作为错误显示。目录上下文在每次唤醒时捕获一次，绑定当时的前台应用，并在该 Session 中保持不变；如果 40ms 内没读完，首屏先用已有 Context 显示，结果到达后再追加。

V0.4 的 Finder 选中项、选中文本和浏览器 URL 也以唤醒时的前台应用实例为目标异步读取。应用身份在显示 Launcher 前冻结；具体值以系统读取时为准，浏览器在等待期间跳转时可能得到跳转后的 URL。

Settings → Context Sources 可独立关闭四种来源，并查看上次唤醒的采集状态。无选区、不支持、超时、内容过长、选中项过多和权限拒绝分别处理；首页只提示需要处理的问题，拒绝授权时可打开系统设置，处理后重新唤醒即可重采集。隐藏后的 15 秒内保留原 Context 以恢复操作，超时后只保留状态元数据供设置页查看，不保留旧 Context 正文。

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

`cargo test -p anywhere-core` 不需要 Flutter、Dart 或可见 GUI。完整检查脚本校验共享 Flutter 版本和生成键码映射，并运行原生菜单/采集与 Host 服务回归、Rust fmt / tests / Clippy、架构检查和 Flutter analyze / tests。热键键码及显示标签只修改 `resources/hotkey-keymap.json`，随后执行 `python3 scripts/generate-keymap.py` 同步 Dart/Swift。

Rust 测试使用真实临时文件和 SQLite；Core 的 OS 动作由 TestPlatform 记录，不打开浏览器或覆盖系统剪贴板。平台 Git 回归会在临时仓库执行初始化、提交及只读查询，不修改当前项目仓库。V0.5 的进程测试会在临时目录启动受控子进程，并对无外部依赖的 Rust 项目执行 Cargo Check。基础动作由内建 ActionBundle 提供，Cargo 运行目标和 Node scripts 动作按 Context 动态生成。Maven、Gradle、Node 的真实工具验收单独运行，命令和环境要求见 [0.5 MVP](docs/mvp-v0.5.md)。

修改桥接公开 API 后重新生成：

```bash
cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked
cargo install cargo-expand --locked
scripts/generate-bridge.sh
```

已生成的 Dart/Rust 绑定随源码交付，普通构建无需安装生成器。

## 基础对象动作

| 对象 | 动作 |
|---|---|
| Directory | Open Terminal Here / Copy Path / Open Directory |
| Git repository | Git Status / Git Log / Git Diff / Git Staged Diff / Git Branches；Copy Branch Name / Copy Commit Hash / Copy Short Commit Hash / Copy Repository Path / Copy Remote URL |
| File | Open File / Reveal in File Manager / Copy Path |
| URL | Open in Default Browser / Copy URL |
| JSON | Format JSON / Minify JSON / Copy Formatted JSON |

应用不内置 Web Search，也不自动安装搜索插件。需要时可修改 [搜索插件示例](plugins/examples/search.toml) 中的 URL 并安装到用户插件目录；示例中的 `example.com` 只是占位地址。插件命令与内建命令共用注册表和排序。

只识别明确的 HTTP(S) URL，不推断裸域名。路径必须真实存在，支持绝对路径；相对路径以进程工作目录为准，不解释 shell 语法或 `~`。JSON 支持对象、数组及合法 primitive，并保留原始文本。输入最大 1 MiB。

Git 动作只在执行时运行固定的只读命令；发现仓库只做文件系统检查，从不调用 Git。Git 未安装时依赖 Git 的动作显示 Disabled 并给出原因，Copy Repository Path 仍可使用。新增动作在 Git Scope 和搜索中显示，可绑定快捷键，默认首页保留原有推荐数量。分支和提交信息在执行时读取，边界与命令见 [Git 常用动作](docs/git-actions-v0.6.md)。剪贴板只在唤醒时读取一次文本快照；当它已经等于当前路径时 Copy Path / Copy Repository Path 隐藏。

## 数据与插件

IDEA 的项目窗口目录可通过已有 Directory Context 识别：窗口须显示明确项目路径，目录真实存在且包含 `.idea`，读取需要辅助功能权限。Git 与 Developer Actions 继续复用该目录。VS Code、Cursor Agent 和 ChatGPT 当前对话目录尚未接入；探索结论与统一 Bridge 设计见 [编辑器与 Agent Context](docs/editor-agent-context.md)。

- 历史：`~/Library/Application Support/Anywhere/usage.sqlite3`。
- 自定义插件：`~/Library/Application Support/Anywhere/plugins/*.toml`，或 `<plugin>/manifest.toml`。
- 示例：[plugins/examples/search.toml](plugins/examples/search.toml)。修改插件后重启应用。
- 日志：`~/Library/Application Support/Anywhere/launcher.log`，包含 Rust 和窗口耗时日志，每次启动覆盖。终端和 Finder 启动使用同一路径。

历史只存 command_id、context_type、source_app、次数、时间，不存输入或路径。成功执行后更新历史，下一次排序生效。频次和最近使用加分有上限，不会压过当前输入的来源优先级。

## 结构与边界

[架构](docs/architecture.md) · [新版路线图](docs/roadmap.md) · [对象与动作导航设计](docs/action-navigation-design.md) · [V0.3.x MVP](docs/mvp-v0.3x.md) · [Context 模型](docs/context-model.md) · [插件协议](docs/plugin-protocol.md) · [Current App 菜单动作](docs/current-app-actions.md)

`dev-0.4` 已完成工程收尾并提交；实机验收状态见 [0.4 验收记录](docs/v0.4-context-acceptance.md)。已收尾的 `dev-0.5` 版本为 `0.5.0-dev.3`（App `0.5.0+11`），已接入独立进程会话，以及 Rust、Maven、Gradle、Node 项目的常用动作。Maven/Gradle 优先使用项目 wrapper；Node 展示 package.json 明确声明的脚本，按 packageManager 或锁文件选择 npm/pnpm/yarn；Cargo Run 按 package/binary 展示明确目标，Spring Boot 提供 Maven/Gradle Run。任务输出、退出码和停止入口位于菜单栏的 Running Tasks 窗口，关闭 Launcher 或任务窗口不会停止任务；退出 Anywhere 会停止任务。设置页使用独立原生窗口，提供关闭和最小化；任务窗口还支持调整大小。⌘W 关闭当前独立窗口，⌘M 最小化；关闭设置会等待保存，并保护未应用的修改。本阶段实现、限制与后续切片见 [0.5 MVP](docs/mvp-v0.5.md)。

### 0.5 开发包

```bash
# 独立生成 0.5 开发包，保留 build/Anywhere.app 的已有版本。
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-dev-0.5.app" scripts/build-macos.sh
scripts/package-macos.sh build/Anywhere-dev-0.5.app
```

打包前检查 Flutter / 原生版本和固定签名；打包后核对归档中的版本、主程序和 Rust 动态库，生成 ZIP 及 SHA-256 校验文件。复制或运行 `build/Anywhere-dev-0.5.app`，ZIP 用于传输。当前工程验收状态与交付边界见 [0.5 收尾记录](docs/v0.5-closeout.md)。

开发项目识别和常用构建/测试/运行已纳入 0.5；自定义 Action 快捷键已纳入 0.6，文件搜索在 0.7 开发中；插件 UI 属于后续阶段。可执行插件运行时、AI、账户与云同步尚未纳入当前版本。

### 菜单身份安全降级（0.5.0+11）

菜单重扫后旧候选会失效；标题或标识变化时拒绝旧目标并刷新。未验证长期身份的菜单暂不读取或记录 frecency，旧数据库数据保留，其他动作历史照常使用。原因、回归与长期快捷键限制见 [菜单身份安全说明](docs/current-app-actions.md)。

## V0.6 开发中：Action Shortcuts

当前分支 `dev-0.6`，Rust `0.6.0-dev.13`、App `0.6.0+13`，从已推送的 0.5 收尾基线 `d03a1f2` 开始。Settings → Action Shortcuts 可以搜索动作、录制和保存 Launcher 内快捷键。唤醒 Anywhere 后按组合键，只有一个可用对象时执行，多个对象时先选择；不需要先搜索出该动作。缺少 Context 或动作失效时拒绝执行，菜单长期绑定仍禁止。已有设置兼容读取，配置冲突明确提示。macOS 的唤醒键、固定动作与 Context 意图统一使用原生按键录制，录制取消和弹窗生命周期有保护；真实物理全局触发和不同键盘布局仍待验收。

编辑时开启 Global shortcut 可从其他应用触发固定动作：重新捕获 Context，等待时 Esc 取消，超时不执行。旧配置保持仅在 Launcher 内生效。Add context shortcut 可让同一组键按源应用与 Context 类型分派不同动作：更具体的条件优先，同等匹配由用户选择，已匹配动作失效时不回退到另一命令。旧版本 1/2 配置保留原有生效范围。插件重载会暂停被撤销动作的快捷键，动作恢复后重新注册；配置保留。关闭设置时恢复失败会留在原页反馈并允许重试，冲突不阻止退出；离开项目后仍能编辑已保存脚本的键位和生效范围，动作身份保持。真实系统验收尚未完成，完整进度与契约见 [0.6 MVP](docs/mvp-v0.6.md)，功能核对见 [0.6 收尾记录](docs/v0.6-final-acceptance.md)。开发 App 为 `build/Anywhere-dev-0.6.app`；0.5 的 App 和 ZIP 单独保留。0.5 的最终功能与验收证据见 [最终核对](docs/v0.5-final-acceptance.md)。

### 默认唤醒与内容输入

默认唤醒优先填入选中文本，否则使用 15 秒内新复制的剪贴板文本；输入框显示来源，Global Search 只解析这份内容。按 Esc 清空内容并返回 Current App，再按 Esc 进入空白 Global Search。旧剪贴板或复制时间未知时直接进入 Current App。原有 Scope 分类与筛选保持不变，详见 [交互契约](docs/context-input-navigation.md)。

### 统一视觉与页面导航

所有窗口、表单和弹窗共用 UI 主题与组件。设置按 General、Shortcuts、Context、Search、Plugins 分类；输入框、主要操作、状态与选中项有明确视觉区分。设计与验收见 [UI 系统梳理](docs/ui-system-v0.6.md)。

跨平台共享层与 Host 接入边界见 [platform-portability.md](docs/platform-portability.md)。当前完整系统适配器仍为 macOS；新增 Linux/Windows 共享代码 CI 不代表已交付 Windows App。

### 文件搜索目录授权（V0.7 build 19）

macOS 首次启动或保存新的搜索范围时，会先准备目录访问；选择所需文件夹后保存只读授权书签。只搜索有效授权范围，取消时不默认放开 Home；受保护位置会跳过并在底部注明。Settings → Search 可重新授权保存范围，或打开完全磁盘访问设置（需用户手动授予）。见 [授权流程](docs/file-search-access-v0.7.md)。

当前职责边界重构及增量快照验证：[build 21 记录](docs/refactor-boundaries-build21.md)。

V0.7 工程收尾已提交，当前开发分支为 dev-0.8：[第一方模块与 Extension v2 MVP](docs/mvp-v0.8.md)。
