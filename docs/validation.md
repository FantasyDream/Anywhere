# MVP 验收记录

日期：2026-09-14。源码、桥接绑定和本机架构 Release 应用已生成。**真实 GUI 的 A–F 验收尚未完成，不能据此宣称 MVP 全部通过。**

## 构建环境

- macOS 26.6.2 / Apple Silicon；应用最低 macOS 12。
- Rust 1.98.1，flutter_rust_bridge Rust/Dart/codegen 均为 2.11.1。
- Flutter 官方 stable 分支源码快照 `9584c6713b324636289d067944a46fd6b49df14b`，Engine `06a2e2a110089dff50fe635cffd2a61e1b24fbcd`，Dart 3.13.3。
- SDK 使用源码归档引导本地 Git 元数据，`flutter --version` 显示 `3.47.0-1.0.pre-1` / 本地 revision `84a0b73719`；不能把它标记为官方 3.47.2 tag。依赖锁文件随源码提供。
- 构建使用 Apple Command Line Tools、Flutter assemble、swiftc、codesign。没有安装或依赖完整 Xcode GUI 工程。

## 初版已执行的检查（第二轮修订前）

| 检查 | 结果 |
|---|---|
| `cargo fmt --all --check` | 通过 |
| `cargo test --workspace --locked` | 17 项通过：Core 8、Facade 集成 5、Platform 1、Plugin 2、Protocol 1 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 |
| `python3 scripts/check-architecture.py` | 90 个 Core 直接/间接依赖，无 Flutter / UI bridge 依赖 |
| `flutter analyze` | No issues found |
| `flutter test` | 4 项通过：捕获完成后显示、键盘操作、输入乱序响应、过期执行响应 |
| FRB 2.11.1 生成 | 完成，Rust/Dart 绑定可编译；启动入口包含 ping 往返检查 |
| `scripts/build-macos.sh` | arm64 Release 构建通过 |
| `plutil -lint`、`codesign --verify --deep --strict` | 通过 |
| Mach-O 动态链接检查 | 正确引用打包的 FlutterMacOS.framework / App.framework |

Core 集成测试通过同一个 Application Facade 覆盖 A–F。文件和目录来自真实临时目录，SQLite 测试使用真实数据库并验证重新打开。OS 动作使用 TestPlatform，未把模拟调用当成真实浏览器、Terminal 或剪贴板验收。

其他覆盖包括 URL/JSON 的 additive 识别、JSON primitive/raw 保留、空输入、非法 URL、带空格路径、稳定排序、显式输入优先、历史加分上限、Provider 环/深度/去重、Revision/Session/隐藏后旧结果丢弃、过期 Action、插件原子注册与非法文件隔离、模板编码与展开大小上限、平台失败、成功动作后的历史写入失败。

## 性能采样

运行 `cargo run --release --locked -p launcher-core --example measure`。每种输入预热 100 次、测量 1,000 次，固定内建注册表、空历史、关闭日志 subscriber；单位 ms。

| 输入 | Resolver p50 / p95 | Ranking p50 / p95 |
|---|---:|---:|
| `https://github.com` | 0.035 / 0.050 | 0.003 / 0.003 |
| `{"name":"foo","age":18}` | 0.024 / 0.032 | 0.003 / 0.003 |
| `Cargo.toml` | 0.012 / 0.017 | 0.002 / 0.002 |
| `.` | 0.010 / 0.013 | 0.002 / 0.002 |
| `hello world` | 0.018 / 0.024 | 0.001 / 0.001 |

这是热缓存 Core 采样，**不包括** Flutter/FFI、日志 I/O、Finder Automation 或真实窗口。不得用它证明 activation <100 ms。

应用日志位于 `~/Library/Application Support/ContextLauncher/launcher.log`，每次启动覆盖。Rust 记录 `activation_context_captured`、`activation_actions_ready`、`input_context_resolved`、`input_actions_ready`、`ranking_completed` 的 duration_us；Swift 记录 `activation_requested` 和 `launcher_visible.hotkey_to_visible_ms`。比较 native 请求与 Rust 捕获完成的时间戳可计算包含桥接的捕获耗时；可见耗时使用 native monotonic clock。首次 Automation 授权耗时需单独记录。

## 真实应用验收：进行中

首次 GUI 检查超时，未取得窗口/可访问性树证据。源码定位到隐藏 FlutterViewController 等待 viewWillAppear 才启动引擎，而显示流程反过来等待 Dart 的循环依赖。修复为显式启动引擎，ready/show 不等待隐藏窗口绘制帧；捕获和排序仍在显示前完成。此修复已构建，实际窗口行为仍待验证。

再次通过界面工具启动时，自动审批要求该时点的用户确认。用户已明确回复“允许”，随后启动成功，没有绕过审批。

实机进一步发现挂载 FlutterViewController 会把窗口改为其初始视图尺寸 0×0。原生日志复现 `window_width:0, window_height:0`，界面工具同时稳定返回 ScreenCaptureKit -3811。按 Flutter SDK 的 MainFlutterWindow 模板，在挂载后恢复原窗口 frame；重新构建启动后日志为 760×520，界面工具正常返回可访问性树和截图。这是实机回归验证，Flutter widget 测试本身不覆盖 AppKit 窗口尺寸。

已观察到真实窗口、输入框、IntelliJ IDEA 前台应用 Context、空状态和键盘提示；Esc 后界面工具返回 noWindowsAvailable，确认窗口隐藏。当前第一次修复后显示调用耗时 11.674 ms，属于非 Finder 场景的单次样本。系统热键仍需区分人工键盘和工具向指定进程发送的模拟键，不能把后者未触发回调当作真实键盘失败。

| 场景 | 操作与预期 | 真实 GUI 状态 |
|---|---|---|
| A | Finder 打开实际目录，⌘⌥Space，无输入出现 Terminal / Copy Path / Open Directory | 用户确认能唤醒；报告 A03 复制失败、A07 虚拟页面报错；第二轮待人工回归 |
| B | 输入 `https://github.com`，Browser / Copy URL + Text 动作，不混入 Finder 动作 | 用户反馈初版混入 Finder 动作；第二轮修复待人工回归 |
| C | 输入 `{"name":"foo","age":18}`，Format / Minify / Copy Formatted JSON | 待验收 |
| D | 输入真实文件路径，Open File / Reveal / Copy Path | 用户报告 D04 复制失败；第二轮待人工回归 |
| E | 输入真实目录，Open Directory / Terminal / Copy Path，无重复或环境动作 | 用户报告 E04 复制失败；第二轮待人工回归 |
| F | 输入 `hello world`，显示来自 Google TOML 插件的 Search Google | 待验收 |

用户选择接手后续手工验收，步骤见 [完整手工验收清单](manual-validation.md)。界面工具在模拟输入中出现粘贴无效、冒号缺失，AX setValue 与 Flutter 画面也不一致，未据此判定应用在真实键盘下失败。B–F、复制/打开执行与正常 Finder 激活性能仍需按清单确认。Finder 第一次授权样本为约 3,991 ms，不作为已授权正常激活的性能结果。

可选平台手工 harness：`cargo run -p launcher-platform --example platform` 默认只读取前台上下文；其余子命令见源码。该 harness 的静态编译与输入拒绝测试已通过；第二轮通过其中的 copy 分支做过编码回归，不能替代整条 GUI 复制粘贴流程验收。

## 第二轮修订与人工交接

用户已测到 X 组，报告失焦不隐藏、空列表占位/固定高度、显式输入仍带 Finder 动作，以及 A03/D04/E04 复制失败、A07 虚拟页面报错。本轮实现如下：

| 问题 | 修订 | 人工回归 |
|---|---|---|
| A03 / D04 / E04 中文路径复制为空 | 复现 GUI 的 C locale 下 pbcopy 返回 0 却写入空剪贴板；统一平台子进程使用 UTF-8 locale | 三项均需在新应用中实际复制、到编辑器连续粘贴 |
| A07 最近使用/共享报 -1700 | Finder alias 转换失败视为无目录，其他系统错误仍报告 | A07 拆为虚拟页面场景，原无窗口 Desktop 场景移到 A08 |
| B01 / E01 环境动作残留 | 非空输入仅匹配 UserInput；清空恢复本次环境快照 | 检查整个列表没有目录 A 的动作 |
| 失焦、旧回调与尺寸 | AppKit 立即隐藏，Dart/Core 失效旧 Session，generation 防止旧响应干扰新窗口 | K11、X08/X09/X11 |
| UI 与列表高度 | 紧凑深色搜索栏、圆角细边框；按行增高，7 行后滚动，上限 520 | K01、K12–K14 |

在用户要求「不要自己测」之前已执行：

- Core 5 项 Facade 集成通过，包含输入排除环境动作和清空恢复环境动作的断言；该断言在修复前失败。
- Flutter 7 项 widget 测试通过，覆盖动态高度/滚动和两类失焦异步回调；analyze 当时有 1 条大括号样式提示，随后已修正，但未再次运行 analyze。
- 真实剪贴板 harness：3 种文本 × 2 种 locale，共 6 项通过；保留可重复入口，保存并恢复原剪贴板类型。
- 第二轮 Release 应用已构建并通过构建脚本中的签名验证。
- 原生界面取得搜索栏 648×64 及文本 2 个动作、约 648×238 的截图/可访问性树；切换焦点后记录 `launcher_dismissed / focus_lost`。Finder「最近使用」「共享」经过平台读取均为 None。

以上范围不能替代用户的完整实机验收，特别是 GUI 复制粘贴、物理热键、中文输入法和跨应用竞态。**收到用户指示后已停止继续测试；剩余全部由用户执行。** 本轮未重新执行完整 workspace/clippy 检查，不把初版结果标为最新验证结果。

[完整 80 项人工测试清单](manual-validation.md) 保留原编号；K11–K14、A08、X11 为新增；X08/X09/X10 为修订。长列表的 8 动作样例在 `build/manual-validation/manual-window-size.toml`，未放入应用或用户插件目录。

可选编码回归入口（这里只记录命令，本轮不再运行）：

```bash
source scripts/env.sh
cargo build -p launcher-platform --example platform
xcrun swift -module-cache-path .tools/swift-cache \
  crates/launcher-platform/tests/clipboard_locale.swift "$PWD/target/debug/examples/platform"
```

## 毛玻璃与圆角修订

用户截图显示圆角内容外仍露出矩形底色。已将 NSWindow 的实色背景和 FlutterViewController 默认黑底改为透明，在 Flutter 后方添加原生 NSVisualEffectView；毛玻璃使用可拉伸圆角 mask，Flutter 背景、选中行和图标底色改为半透明。高度变化时同步刷新窗口阴影。

已重新构建本机 Release 应用；构建脚本中的签名步骤完成。遵循用户「不要自己测」的要求，本次未运行测试、未启动应用或操作 GUI。实际四角、毛玻璃透色及展开/收起效果由用户按更新后的 K01/K12/K14 验证。

## 0.1.1 输入闪退、文字对比度和 Windows 交付

读取用户机器已有的 5 份崩溃报告（2026-09-14 18:02、19:17、19:18 两次、19:30），均为 EXC_BAD_ACCESS，栈顶 `impeller::Canvas::SetupRenderPass`。按当前 SDK 的正式配置设 `FLTEnableImpeller=false` 切回 Skia/Metal，避开该崩溃路径；不能据此宣称已修复上游引擎。输入解析期间保留不可执行的已提交列表，提交新结果后才改变窗口高度，相同尺寸不重复调用原生 resize。半透明底色加深，正文、提示与副标题亮度提高。

0.1.1 (2) 的 macOS Release 已重新编译，构建脚本完成签名步骤；未自行启动或执行测试。用户重点回归 K01/K02/K10/K12，包括手打、中文组词、粘贴后退格以及浅色背景文字。

Windows 新增 Shell COM 目录捕获、文件/目录/URL 打开、文件定位、PowerShell 当前目录和 Unicode 剪贴板；保留平台中立 Core。用户授权上传 GitHub 后，私有仓库已创建，Actions 只构建打包，不执行测试。最终构建结果及产物见 [Windows 工作流](https://github.com/FantasyDream/Context-Aware-Launcher/actions/workflows/windows.yml)；Windows 人工验收尚未执行。

2026-09-15（北京时间），提交 `e82a5ec` 的 [Windows x64 构建](https://github.com/FantasyDream/Context-Aware-Launcher/actions/runs/34870418394) 成功，ZIP 已上传到 Artifacts。构建过程中修正 Flutter 已移除的参数、Windows SDK 宏名冲突，以及 VC++ 运行库安装路径。未运行测试或启动 Windows 应用；资源管理器、输入法、焦点与渲染效果仍需按 Windows W01–W18 人工确认。

## 当前边界

macOS 应用为本机 arm64、adhoc 本地签名包，没有发行证书、公证或 universal packaging。未验证其他 macOS 版本。Windows x64 按用户追加要求加入，尚未完成实机验证。完整验收前仍需解决真实界面运行中发现的问题。
