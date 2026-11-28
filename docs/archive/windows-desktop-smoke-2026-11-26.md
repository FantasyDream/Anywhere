# Windows 桌面抽测记录（2026-11-26）

整理自本机 `build/windows-qa-20261126/report.md` 和该目录的工程日志。受测
源码为干净的 `0f18732d5ee000718f40b5e25f3d75a2c0b0fe18`。环境为
Windows 11 专业版 10.0.26300 x64，单屏 3840×2160，WebView2 Runtime
158.0.4370.33；使用 `ANYWHERE_DATA_DIR` 隔离。当前执行入口见
[Windows 桌面验收指南](../test-acceptance-windows.md)。

## 构建与工程检查

- 使用 Rust `1.100.0-beta.7`、clang-cl / lld-link、cargo-xwin 提供的 MSVC
  CRT 和 SDK 构建 release。GPUI 采用运行时着色器编译配置。验证了 PE x64、
  ZIP 完整性和解压内容。
- EXE SHA-256 为 `c7ba047415e4810a6646d70e494b926b98093113fdd31efc97e01562c5ad6e9d`。
- ZIP SHA-256 为 `789764596ee661600613204021bd314574458a68d34b0df4eee18d48ac8e48fe`。
- Core、Platform、Plugin、Protocol 的编译检查完成并有警告。共享 crate
  测试日志显示 Core 库 336 通过、33 失败、1 忽略，MVP 集成测试 171 通过、
  8 失败、4 忽略，Platform 库 38 通过、1 失败。失败覆盖历史来源、Web 包和
  后台解析、文件搜索、开发动作、SQLite 替换与剪贴板文件路径。日志未包含
  CI 串行 GPUI 门禁的结果。

## 桌面抽测

| 场景 | 当轮观察 |
| --- | --- |
| 启动、唤醒、托盘 | 解压启动可见，Esc 隐藏，Ctrl+Alt+K 恢复，托盘菜单可打开设置及退出；退出后 PID 结束 |
| 应用搜索 | 本机索引完成；查询 PowerShell 并启动对应进程 |
| 文本剪贴板 | 初始记录关闭；开启后记录两条合成文本，筛选、重新复制后剪贴板值匹配 |
| 文件搜索与打开 | 隔离设置保存后，中文名查询得到预期文件，系统关联应用打开目标；结果路径显示带 `\\?\` 前缀 |

本轮未取得设置开关点击后立即保存的稳定证据。Everything 未运行。默认目录单实例、
实体按键与 IME 候选、焦点与 DPI / 多屏、外观、登录启动、Explorer 重启、
WebView2、富剪贴板和外部预览留待对应环境执行。原记录包含桌面工具截图、进程和
数据层核对，未记录实体键鼠输入来源。

详细日志和截图为本机生成物。后续报告按实际受测产物补充独立证据。

## 后续 Windows 修复复验

同一 Windows 11 主机，源码为 `0f18732` 加本轮工作区差异。使用 MSVC x64
目标配合 clang-cl 与链接器构建。下列结果归属修复版，原抽测仍归属最初的 ZIP。

| 检查 | 结果 |
| --- | --- |
| 串行 Rust 库测试 | Core 371 通过 / 1 忽略，GPUI 311 通过，Platform 56 通过 |
| MVP 集成 | 177 通过 / 4 忽略。Windows 夹具覆盖 `.cmd` 包装器、缺失绝对路径和 SQLite 活跃文件锁 |
| Windows Clippy | GPUI 和 Platform 全目标严格检查通过 |
| Windows Release | x64 构建成功，EXE SHA-256 为 `f8539b2c6d67f7f10c9a02268ecef338a60bbd54a1a10aec7e45a4685b1920c2` |
| 标准菜单联调 | 从前台 7-Zip 26.02 枚举 65 项。原生适配投递“帮助 → 关于 7-Zip”后，桌面代理在无障碍树中观察到对应对话框 |
| QuickLook 联调 | 使用 QuickLook for Windows 4.5.0 Portable，先后观察到两个夹具的真实预览窗口。通过 QuickLook 自身协议关闭后，两个资源均在约 50 ms 的采样间隔内被适配层释放 |
| macOS 脚本 | Windows 主机执行 `check-gpui.sh` 时缺少 `plutil`；`check.sh` 在代码规范阶段缺少 ESLint。macOS 原生结果留待对应环境 |

菜单与预览联调直接执行产品的平台适配。桌面工具未枚举出 Anywhere Launcher
工具浮窗，因此 Launcher 菜单选择、按键、跨窗口焦点、完整文件搜索界面和多屏等
项目仍按 [Windows 桌面验收指南](../test-acceptance-windows.md)复验。

## 代码审查后复验

同一主机使用代码审查后的工作区。MSVC x64 库回归 743 项通过、1 项忽略，MVP 集成
177 项通过、4 项忽略。GPUI 与 Platform 的严格 Clippy 和 Windows x64 Release 构建
均通过。此构建 EXE SHA-256 为
`360c83e0f732f3cba5e46c3d0928153751956e9d49dee46b2c3c3dc42c934a22`。

QuickLook 4.5.0 Portable 的联调直接调用原生适配，
观察到独立窗口在冲突请求后仍保留，之后两个测试文件都可依次打开预览。QuickLook
自身协议关闭窗口后，适配层在约 50 ms 的轮询间隔内释放各自的租约。这组观察覆盖
外部预览和协议处理，Launcher / Shelf 焦点和实体按键仍待桌面验收。

原生回归覆盖同进程两个窗口的 Context 隔离，以及模态 owner 和执行前动态禁用
的 Win32 菜单项。GPUI 回归以可控完成顺序覆盖剪贴板写入期间排队的设置保存。
真实浏览器和 Explorer 多标签页兼容性仍待复验。
