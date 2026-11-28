# 跨平台架构与接入契约

macOS 15+ 已发布 GPUI 开发构建。Windows 11 x64 源码提供共用 GPUI 的开发预览，
范围和实机状态见 [Windows 功能进度](windows-progress.md)。Linux 参与共享 crate 的编译和回归。

## 共享与系统边界

| 层 | 共享内容 | 系统边界 |
| --- | --- | --- |
| GPUI product | 页面、导航、输入、Scope、选择、设置与工具状态 | macOS 对接 AppKit / Quick Look / WebKit，Windows 对接 Win32、通知区域和 WebView2 |
| Product host | 数据路径、运行时、偏好、热键、目录授权和登录项 | `apps/gpui/src/host/`、`apps/gpui/src/platform/` |
| Rust Core | Context Graph、Action、候选、搜索、Ranking、Execution | 通过窄能力接口使用 OS |
| Platform services | 应用、Context、剪贴板、文件搜索、资源、菜单与 Git | 不把应用 ID、Finder 或 shell 规则散布到 Core |
| Protocol | Context、Candidate、Launcher、执行与 Web Manifest | 不包含 NSView 或其他 UI 对象 |
| Shortcut schema | HID usage、语义修饰位、动作身份与意图规则 | global-hotkey 对接 macOS Carbon 和 Windows RegisterHotKey |

GPUI 页面不是 AppKit 控件。系统能力缺失时返回明确禁用状态或安全空值，GPUI
继续渲染主题与其他页面。不得用 OS API 猜测业务授权，也不得让 Core 持有窗口对象。

## 启动与路径

`apps/gpui/src/main.rs` 验证产品系统版本并启动 GPUI application。
`apps/gpui/src/app.rs` 建立主窗口、状态栏、热键和运行时。
`apps/gpui/src/host/mod.rs` 解析：

- 数据目录：macOS 为 `~/Library/Application Support/Anywhere`，Windows 为 `%LOCALAPPDATA%\Anywhere`；
- 用户已批准 Web 包、应用历史自定义适配与模块存储：位于数据目录下的对应子目录；
- 显式覆盖：绝对路径、非根目录、不含 `..`；
- 显式数据目录（`ANYWHERE_DATA_DIR`）：自动跳过系统登录项修改。

路径或运行时初始化失败必须显示可诊断错误，不能猜测其他动态库、数据目录或产品
身份。仓库没有跨语言运行库加载步骤。

## 系统能力

`anywhere-platform` 的窄接口继续保持：

- 应用目录、入口识别和元数据；
- 前台应用、启动、图标和版本；
- 应用目录、选中文本、文件管理器和浏览器 Context；
- 剪贴板读写、代数与历史载荷；
- 系统文件索引、范围搜索与取消；
- URL、文件、目录、终端和指定应用打开；
- Accessibility 菜单发现与重放；
- 白名单只读 Git 操作。macOS 使用系统沙箱，Windows 使用 Job Object；详细边界见 [开发动作](developer-actions.md#git)。

能力实现按平台注入。未实现能力显式返回 unsupported、空值或既有安全默认，不
静默扩大权限。文件、目录、工作区文件和远程 URI 保持不同对象类型。

## 快捷键

动作快捷键 schema 4 使用物理 USB HID usage；语义修饰位为
Control=1、Alt=2、Shift=4、Meta=8。未知版本不执行其中绑定，也不覆盖当前文件。

- 迁移时保持 local / global 的原有作用域。
- 不支持的路由丢弃，不改成其他动作。
- Meta 在 Windows 表示 Windows 键，不隐式换成 Ctrl。
- 标准页面命令使用平台惯例的 Command / Ctrl。
- 注册失败恢复旧绑定；冲突时显示状态并要求重新选择。

控制器位于 `apps/gpui/src/host/hotkeys.rs`，由 GPUI 事件线程拥有。
Windows 的物理热键注册、布局和冲突矩阵尚待实机验收。

## 状态栏、窗口与预览

macOS 在 `apps/gpui/src/platform/macos/status_item.rs` 使用 `NSStatusItem` 提供恢复入口。
主窗口关闭执行 Quick Look / Web / 待执行资格清理后隐藏。Clipboard Shelf（floating）与 Running Tasks（normal）使用各自独立的 GPUI 窗口；Quick Look 在
`apps/gpui/src/platform/macos/preview.rs` 使用 `QLPreviewPanel`。

文件与剪贴板资源由 Rust 校验，GPUI 只管理当前选择和 preview generation。关闭
回调绑定资源与 generation，旧预览不能影响新目标。Windows 的预览请求
转给当前用户的外部 QuickLook。各平台生命周期详见 [预览](native-preview.md)。

## CI 与平台验证

`.github/workflows/portable.yml` 在 Ubuntu / Windows：

```bash
cargo check \
  -p anywhere-core \
  -p anywhere-platform \
  -p anywhere-plugin \
  -p anywhere-protocol \
  --all-targets --locked
```

Linux 还运行相同共享 crates 的测试。Windows 桌面 job 对 GPUI 和 Platform 运行
严格 Clippy，串行执行 Core、GPUI 和 Platform 库测试及 MVP 集成测试，并构建 x64 ZIP。macOS
workflow 运行完整 `scripts/check.sh` 和 macOS 打包。

## 后续桌面平台

新增桌面平台先建立独立产品 host 和最小能力矩阵：

1. GPUI / 系统窗口、数据路径、主题、IME、状态栏与应用生命周期；
2. 全局热键和 HID 映射，含冲突、失败回滚和重新注册；
3. 应用目录、前台身份、Context、剪贴板和文件搜索；
4. 资源打开、Accessibility 等价菜单能力与进程树；
5. Quick Look 等价预览、WebView 与登录项；
6. 签名、安装包和物理交互矩阵。

不支持的来源应在能力层返回明确结果；为赶进度让 Core 猜测平台或虚构成功会破坏
授权与执行资格。

桌面验证分别按 [macOS](test-acceptance-macos.md) 和 [Windows](test-acceptance-windows.md) 指南执行。
