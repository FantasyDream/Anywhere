# 004 — Windows 适配与渲染兼容性

用户明确要求新增 Windows 版本，覆盖原计划仅 macOS 的范围限制。Core 保持 UI/OS-neutral；Windows 原生能力只放在 anywhere-platform，窗口/热键/托盘放在 Flutter Windows Host。

- 复用 Flutter 官方 Windows runner；默认 x64，Ctrl+Alt+Space 唤醒，支持失焦隐藏及当前 generation 的动态高度。
- 使用 Windows Shell COM 读取唤醒前资源管理器的活动视图。实际目录提供 Directory；Home/此电脑等虚拟视图返回 None，不能沿用上次目录。捕获 HWND 与前台应用快照，读取时不再猜测最新前台窗口。
- Windows 文件/目录/URL 交给 Shell，Reveal 使用 Shell 选择 API，终端用 PowerShell；剪贴板写 CF_UNICODETEXT。使用现有 cc 编译 native C++，不将 COM 代码放入 Core。
- Bridge 选择当前平台；Dart 仅选择动态库路径和平台字体。DTO、插件协议、注册表和匹配逻辑不变。
- 当时 macOS 的 5 份崩溃报告均为 EXC_BAD_ACCESS，栈顶 impeller::Canvas::SetupRenderPass，因此曾使用 FLTEnableImpeller=false 切回 Skia/Metal。这是历史兼容性避让，并非已修复 Flutter 引擎。2026-09-17 按用户要求重新显式启用 Impeller，当前配置与验证边界见下文。输入仍只在结果提交时改变列表高度，不在每个字符处理中先收起再展开。
- 半透明底色加深并提高次要文字亮度，使浅色背景下文字仍清楚，保留原生毛玻璃。

Windows 完整构建需要 Windows 主机及 Visual Studio C++ 工具链。当前只有 Mac，用户确认暂时没有 Windows 主机/CI 仓库，因此最初准备适配源码和构建脚本。2026-09-15 用户进一步授权上传 GitHub 并配置 Actions，现使用私有仓库的 Windows 2022 runner 构建。构建成功仍不等于实机验收通过。当时按用户要求只准备人工验证；后续架构优化重新运行自动检查与 macOS 短程 GUI 验证，结果独立记录。

依据：[Flutter Windows 构建环境](https://docs.flutter.dev/platform-integration/windows/setup)、[Flutter 渲染器配置](https://docs.flutter.dev/perf/impeller)、[IShellBrowser::QueryActiveShellView](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellbrowser-queryactiveshellview)。

## 2026-09-17：重新启用 Impeller

按用户要求，macOS `Runner/Info.plist` 设置 `FLTEnableImpeller=true`，Windows runner 设置 `flutter::ImpellerSwitch::Enabled`。这是 Flutter 渲染后端的切换，Rust/FRB 和现有窗口实现不变。官方文档说明 Flutter 3.47 起桌面默认启用 Impeller；这里使用显式配置，避免自定义 Host 与 SDK 默认值的差异。

首次切换时本机 SDK 实测报告 `3.47.0-1.0.pre-1`、channel stable，framework `84a0b73719`，engine revision `06a2e2a110`；Windows CI 配置为 `3.47.2`。两者并不完全一致。随后架构优化统一为官方 Flutter 3.47.4，使用 PATH 上的 SDK，并由 `.flutter-version` 驱动本地校验与 CI；framework `9584c6713b`、engine `06a2e2a110`、Dart 3.13.3。

macOS arm64 Release 构建和签名校验通过，产物 plist 中开关为 true。重启到新包后，引擎日志确认为 `Using the Impeller rendering backend (MetalSDF).`。通过可见窗口与截图验证了搜索、图标、空结果、49 项结果列表、窗口高度变化及 JSON `true` 的 Format JSON 结果预览，随后按 Esc 隐藏。此短程检查未复现旧崩溃；不能据此认定旧崩溃已修复。中文输入法组合输入、反复热键唤醒与长时间使用仍需实机回归。

Windows 本次仅修改 runner 配置，没有在 Windows 上编译或运行。配置使用的枚举在本地 SDK 源码中存在，仍需 Windows 构建及图形驱动环境验收。


## 2026-09-17：架构重构后的复验

在官方 Flutter 3.47.4 下重新生成桥接、完成 119 项 Rust / 21 项 Flutter 测试及 Release 构建。重启后的实际窗口验证了应用搜索、图标、逐字删除、JSON 预览、方向键、空结果与 Esc 隐藏，日志仍明确使用 Impeller/MetalSDF。系统 IMK mach-port 提示仍出现，中文组合输入未验收；没有把本次短程测试视为旧引擎问题的根因修复。Windows 只完成配置与 CI 工作流改动，远端及实机验证仍待执行。
