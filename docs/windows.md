# Anywhere Windows 开发预览

适用环境为 Windows 11 x64。ZIP 可解压运行，由本地构建或 GitHub Actions 提供。
GitHub Release `v0.14.1` 的下载包面向 macOS Apple Silicon。
[V0.14.1 Release](https://github.com/FantasyDream/Anywhere/releases/tag/v0.14.1) ·
[仓库文档导航](https://github.com/FantasyDream/Anywhere/tree/main/docs)

## 运行

1. 在 PowerShell 运行 `Get-FileHash .\Anywhere-windows-x64.zip -Algorithm SHA256`，
   与同名 `.sha256` 文件核对。
2. 解压后把 `Anywhere` 文件夹放在固定位置，打开 `Anywhere.exe`。缺少
   `VCRUNTIME140.dll` 或 `VCRUNTIME140_1.dll` 时安装
   [Visual C++ Redistributable x64](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)。
   Web 工具还需要系统上的
   [WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。
3. `Ctrl+Alt+K` 打开全局搜索，`Ctrl+Alt+Space` 打开 Current App。方向键选择，
   Enter 执行，Tab / Shift+Tab 切换 Scope，`Ctrl+K` 打开可用的对象操作。
   热键冲突时可从托盘右键菜单进入设置。
4. Esc 和关闭窗口会隐藏 Launcher。可通过热键、通知区域图标或再次运行
   `Anywhere.exe` 恢复；从托盘菜单退出进程。托盘初始化失败时回退到任务栏最小化。

默认数据目录为 `%LOCALAPPDATA%\Anywhere`。以绝对路径设置 `ANYWHERE_DATA_DIR`
可启动隔离数据环境。显式覆盖会跳过登录项更新和默认环境的单实例机制。
在正常数据目录启用登录启动后，当前用户的 Run 注册项以 `--background` 启动
Anywhere。移动程序前先关闭该选项，移动后从新位置运行并重新启用。

桌面代理若只能枚举任务栏窗口，可在验收会话中同时设置
`ANYWHERE_QA_SHOW_IN_TASKBAR=1`。该开关仅让 Launcher 出现在任务栏，便于检查
界面和发送输入，不改变默认的托盘式窗口样式。

## 应用搜索

在 **设置 → 搜索 → 应用搜索位置** 中添加文件夹、快捷方式（.lnk 或 .url）或程序
（.exe），保存后后台更新索引。文件夹本身及其启动项均可搜索，目录最多向下扫描
8 层，跳过链接目录。删除条目并保存后移除自定义来源。

指向同一目标且启动参数、工作目录和提权方式相同的快捷方式合并成一条结果，原名称
仍可作为搜索别名。开始菜单中已有的入口优先保留。

使用标准 Win32 菜单栏的程序可通过 Current App 搜索并执行当前窗口的菜单项。

## 文件搜索

在 **设置 → 搜索** 开启文件搜索，添加可搜索的目录并选择快批引擎。默认 Combined
优先使用有结果的 Everything，随后尝试 Windows Search 和可用的 `rg.exe`。
选直接遍历可跳过快批。索引无结果、不可用或超时时，完整遍历继续；显示的结果仍需
通过授权范围与现有文件校验。

Everything 可用 [voidtools 完整版](https://www.voidtools.com/downloads/)的默认
桌面实例。普通用户客户端可配合 Everything Service 索引 NTFS。Lite 版缺少 IPC。
Anywhere 使用本地 IPC，运行时无需 SDK DLL 或 `es.exe`。Windows Search 仅能加速
系统已索引位置，Ripgrep 需要可发现的本机程序。
[Everything 版本说明](https://www.voidtools.com/support/everything/installing_everything/)

## 剪贴板、预览与 Web 工具

在 **设置 → 剪贴板** 开启历史采集，搜索 Clipboard History 或 Clipboard Shelf
查看记录。Explorer 的文件列表最多保存 4,096 个路径，序列化上限为 16 MiB；
所有路径可访问后才可重新复制并粘贴，恢复使用复制语义。可采集 Unicode
文本、文件列表、PNG / DIB 图片，也可随文本记录可读的 HTML / RTF。

在文件结果或剪贴板可预览条目上，选中一条后按 `Ctrl+Y` 或使用预览按钮。
查询编辑器聚焦、已有选择且 IME 未组词时也可按 Space。预览需要当前用户的
[QuickLook for Windows](https://github.com/QL-Win/QuickLook) 正在运行。

`JSON Workspace` 随应用提供，也可从 **设置 → 功能 → Web 工具** 导入支持
`windows` 的可信本地插件包，核对摘要和权限后批准。Web 工具依赖 WebView2，
可通过页头按钮在独立窗口打开。

## 构建与打包

在 Windows 安装 Visual Studio Build Tools 的 Desktop development with C++ 工作负载、
Windows SDK、仓库锁定的 Rust 工具链及 Python 3.10+。在仓库根目录运行：

```powershell
cargo clippy -p anywhere-gpui -p anywhere-platform --all-targets --locked -- -D warnings
cargo test -p anywhere-core -p anywhere-gpui -p anywhere-platform --lib --locked -- --test-threads=1
cargo test -p anywhere-core --test mvp --locked -- --test-threads=1
cargo build -p anywhere-gpui --release --target x86_64-pc-windows-msvc --locked
python scripts/package-windows.py target/x86_64-pc-windows-msvc/release/anywhere-gpui.exe
```

产物位于 `build/windows/`，包含 ZIP 和 `.sha256` 文件。GitHub Actions 的
Cross-platform compilation 工作流从 Windows job 上传对应提交的 x64 ZIP 和校验文件。

在 macOS 可使用 `cargo-xwin`、LLVM 和 LLD 交叉构建。以下配置让两个 GPUI DirectX
依赖在运行时编译着色器；原生 Windows 构建默认使用 SDK 的 FXC 预编译。

```bash
cargo xwin build -p anywhere-gpui --release --target x86_64-pc-windows-msvc --locked \
  --config 'profile.release.package.gpui-pre-windows.debug-assertions=true' \
  --config 'profile.release.package.gpui-fast-windows.debug-assertions=true'
python3 scripts/package-windows.py target/x86_64-pc-windows-msvc/release/anywhere-gpui.exe
```
