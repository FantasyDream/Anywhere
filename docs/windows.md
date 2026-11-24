# Anywhere Windows 开发预览

适用环境为 Windows 11 x64。使用 MSVC x64 目标构建，界面沿用 Anywhere 的 GPUI
Launcher。ZIP 可以解压运行。安装程序、Authenticode 签名和自动更新待接入。

## 运行

目标机器需要 Microsoft Visual C++ v14 x64 运行库。缺少 `VCRUNTIME140.dll`
或 `VCRUNTIME140_1.dll` 时，安装 [Microsoft Visual C++ Redistributable x64](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)。

1. 在 PowerShell 中运行 `Get-FileHash .\Anywhere-windows-x64.zip -Algorithm SHA256`，
   核对同名 `.sha256` 文件里的摘要。
2. 解压 ZIP，将 `Anywhere` 文件夹放在固定位置，然后打开 `Anywhere.exe`。
3. 默认 `Ctrl+Alt+K` 打开全局搜索，`Ctrl+Alt+Space` 打开当前应用。搜索框内用
   方向键选择、Enter 执行。全局热键冲突时从托盘打开设置重选。
4. 关闭 Launcher 后可通过热键或任务栏通知区域的 Anywhere 图标恢复。再次打开程序会恢复现有 Launcher。
   右键图标可打开设置或退出。若托盘初始化失败，Launcher 最小化到任务栏。

默认数据目录为 `%LOCALAPPDATA%\Anywhere`。可用绝对路径 `ANYWHERE_DATA_DIR`
启动独立数据环境，显式覆盖会跳过登录项注册。常规启动打开 Launcher。启用登录启动后，
Windows 从当前用户的 Run 注册项以 `--background` 启动。移动程序前可先关闭该选项，
从新位置打开后再启用。

## 功能范围

| 功能 | Windows 预览 |
| --- | --- |
| 应用启动 | 搜索用户和系统开始菜单的传统 `.lnk`，由 Windows Shell 打开。前台身份取自 Win32 进程路径和创建时间 |
| 文件搜索 | 通过 Everything 本地 IPC 查找文件名和目录名。按已授权目录查询，返回后复核权限与过滤条件，继续使用原有排序和分页 |
| 文件和链接 | 通过系统关联打开文件、目录与 HTTP(S) 链接。在 Explorer 中定位文件，可在目录打开新的命令提示符 |
| 剪贴板 | 文本读写、近期文本 Context、默认关闭的文本历史与独立 Clipboard Shelf。带排除监控标记的内容跳过 |
| 设置 | 自定义快捷键、系统 / 浅色 / 深色、不透明窗口、登录启动与数据隔离 |
| 待接入 | Explorer 选中项、UI Automation 菜单 / 文本、浏览器当前页、系统预览、全格式剪贴板、WebView2 插件、应用历史的 Windows 应用映射、Git 和项目任务进程树 |

## 文件搜索

安装 [Everything](https://www.voidtools.com/downloads/) 1.4.1 或更新的完整版，
让默认实例的桌面客户端在后台运行。使用普通用户启动客户端，NTFS 索引可由
[Everything Service](https://www.voidtools.com/support/everything/everything_service/) 提供。
Lite 版缺少 IPC，1.5 alpha 用户可改用默认实例。Anywhere 直接调用
[Everything 本地 IPC](https://www.voidtools.com/support/everything/sdk/ipc/)，
无需安装 SDK DLL 或配置 `es.exe`。

在设置中开启文件搜索并选择已被 Everything 索引的目录。普通查询按字面匹配
文件或文件夹名称，支持中文原文。显式输入路径仍可在授权范围内直接浏览。
Everything 未启动、索引未就绪或查询超时时，文件页显示原因并可重试。每次最多
接收 4,096 项索引候选，超过上限时显示部分结果，可缩小查询以继续定位。

## 构建与打包

在 Windows 安装 Visual Studio Build Tools 的 Desktop development with C++ 工作负载、
Windows SDK、仓库锁定的 Rust 工具链及 Python 3.10+。在仓库根目录运行：

```powershell
cargo clippy -p anywhere-gpui -p anywhere-platform --all-targets --locked -- -D warnings
cargo test -p anywhere-gpui --lib --locked windows_launcher
cargo build -p anywhere-gpui --release --target x86_64-pc-windows-msvc --locked
python scripts/package-windows.py target/x86_64-pc-windows-msvc/release/anywhere-gpui.exe
```

产物位于 `build/windows/`，含 ZIP、SHA-256、许可证与本说明。GitHub 的
Cross-platform compilation workflow 提供相同 x64 包。Windows 原生 CI 和桌面证据
分别记录。

macOS 上可用 `cargo-xwin`、LLVM 和 LLD 做交叉检查。预览二进制可沿用 release
优化，并让两个 GPUI DirectX 依赖在运行时编译着色器。原生 Windows 构建使用
SDK 的 FXC 预编译着色器。

```bash
cargo xwin build -p anywhere-gpui --release --target x86_64-pc-windows-msvc --locked \
  --config 'profile.release.package.gpui-pre-windows.debug-assertions=true' \
  --config 'profile.release.package.gpui-fast-windows.debug-assertions=true'
python3 scripts/package-windows.py target/x86_64-pc-windows-msvc/release/anywhere-gpui.exe
```

## Windows 11 实机验收

复验时记录 Windows 版本、显示器缩放、Git 提交或源差异及 ZIP SHA-256。
建议使用独立数据目录做日常验收。登录启动项只在正常数据目录下验收。

| 检查 | 操作和预期 | 状态 |
| --- | --- | --- |
| 启动与恢复 | 在装有 x64 运行库的 Windows 11 解压运行，按 Esc 或点击关闭后检查托盘，再分别用图标、托盘菜单和两个默认热键恢复 | Pending |
| 快捷键和焦点 | 从外部应用唤醒并输入，切回外部应用后 Launcher 隐藏。录入已占用热键后确认旧绑定仍可恢复 | Pending |
| IME 和 DPI | 在微软拼音中组词、选择候选、提交及取消。在两块不同缩放的屏幕检查窗口与列表 | Pending |
| 应用及文件 | 启动一个开始菜单应用。启动 Everything 并索引含中文和空格的测试目录，再在 Anywhere 授权该目录。以同名范围外文件、隐藏文件和不同扩展名验证过滤，超过 64 项时验证翻页，选中结果验证打开与定位。退出 Everything 后搜索并验证错误；重新启动后重试 | Pending |
| 剪贴板 | 复制测试文本，确认历史默认关闭。开启后记录两条文本，打开 Shelf 验证检索、复制、删除和返回原窗口 | Pending |
| 外观与登录 | 切换系统 / 浅色 / 深色并重启验证。在普通数据目录启用登录启动，注销后确认后台托盘恢复。关闭该设置后再次注销验证 | Pending |
| 退出与故障恢复 | 运行中再次打开程序，确认复用进程。从托盘退出并确认进程结束。重启 Explorer，确认托盘图标重新注册 | Pending |

Windows Run 键的调度行为见 [Microsoft Run 与 RunOnce 文档](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)。
托盘交互与 Explorer 重启机制见 [Microsoft 任务栏文档](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar)。
剪贴板隐私标记见 [Microsoft 剪贴板格式文档](https://learn.microsoft.com/en-gb/windows/win32/dataxchg/clipboard-formats)。
