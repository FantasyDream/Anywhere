# 004 — Windows 适配与渲染兼容性

用户明确要求新增 Windows 版本，覆盖原计划仅 macOS 的范围限制。Core 保持 UI/OS-neutral；Windows 原生能力只放在 launcher-platform，窗口/热键/托盘放在 Flutter Windows Host。

- 复用 Flutter 官方 Windows runner；默认 x64，Ctrl+Alt+Space 唤醒，支持失焦隐藏及当前 generation 的动态高度。
- 使用 Windows Shell COM 读取唤醒前资源管理器的活动视图。实际目录提供 Directory；Home/此电脑等虚拟视图返回 None，不能沿用上次目录。捕获 HWND 与前台应用快照，读取时不再猜测最新前台窗口。
- Windows 文件/目录/URL 交给 Shell，Reveal 使用 Shell 选择 API，终端用 PowerShell；剪贴板写 CF_UNICODETEXT。使用现有 cc 编译 native C++，不将 COM 代码放入 Core。
- Bridge 选择当前平台；Dart 仅选择动态库路径和平台字体。DTO、插件协议、注册表和匹配逻辑不变。
- macOS 的 5 份崩溃报告均为 EXC_BAD_ACCESS，栈顶 impeller::Canvas::SetupRenderPass。当前 SDK 支持 FLTEnableImpeller=false，先切回 Skia/Metal，待用户回归；这是兼容性避让，并非已修复 Flutter 引擎。输入只在结果提交时改变列表高度，不在每个字符处理中先收起再展开。
- 半透明底色加深并提高次要文字亮度，使浅色背景下文字仍清楚，保留原生毛玻璃。

Windows 完整构建需要 Windows 主机及 Visual Studio C++ 工具链。当前只有 Mac，用户确认暂时没有 Windows 主机/CI 仓库，因此交付适配源码和构建脚本；在取得环境前不能标记 Windows 可执行包已生成。用户要求人工测试，本轮不运行自动/GUI 测试。

依据：[Flutter Windows 构建环境](https://docs.flutter.dev/platform-integration/windows/setup)、[Flutter 渲染器配置](https://docs.flutter.dev/perf/impeller)、[IShellBrowser::QueryActiveShellView](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellbrowser-queryactiveshellview)。
