# Windows x64 交付与人工验证

**状态：Windows x64 已由 GitHub Actions 成功编译并生成 ZIP；尚未进行 Windows 实机运行验收。** 仓库为 [FantasyDream/Anywhere](https://github.com/FantasyDream/Anywhere)，保持私有。[成功构建记录](https://github.com/FantasyDream/Anywhere/actions/runs/34870418394) 对应提交 `e82a5ec`。构建成功不代表人工 case 通过。

## GitHub Actions 下载

打开 [Package Windows x64](https://github.com/FantasyDream/Anywhere/actions/workflows/windows.yml)，选择成功的运行，在 Artifacts 下载 `Context-Launcher-Windows-x64`。私有仓库需要登录有访问权限的 GitHub 账户。

本次 [Windows x64 产物](https://github.com/FantasyDream/Anywhere/actions/runs/34870418394/artifacts/10359256591) 下载后，解压其中的 `Context-Launcher-Windows-x64.zip`，运行 `context_launcher.exe`。

相关源码推送到 main 会自动构建，也可以选择 Run workflow 手动打包。工作流只编译打包，不运行测试；产物保留 30 天。不要把构建成功标记为全部人工 case 通过。

## 本地构建

准备 Windows 10/11 x64、Rust stable MSVC 工具链、Flutter 3.47.x Windows SDK、Visual Studio 2022/Build Tools 的「使用 C++ 的桌面开发」工作负载和 Windows SDK。官方说明见 [Flutter Windows 环境](https://docs.flutter.dev/platform-integration/windows/setup)。当前 runner 使用 Flutter 3.47 的 `set_impeller_switch` API。

将源码包完整解压，在仓库根目录的 PowerShell 执行：

```powershell
.\scripts\build-windows.ps1
```

若本机脚本策略要求确认，按本机管理规则运行脚本；不必修改全局策略。该脚本只构建、打包，不运行测试或启动应用。使用现有 PATH 中的 Rust/Flutter，生成：

```text
build\Context-Launcher-Windows-x64.zip
apps\flutter\build\windows\x64\runner\Release\anywhere.exe
```

打包包含 Flutter 引擎、Dart 数据、Rust DLL、Google 插件及可用的 Visual C++ 运行库。分发整个 ZIP，不能只复制 EXE。第一次构建如失败，提供首个错误及前后日志，不能将其标记为可用版本。

## Windows 行为

- Ctrl+Alt+Space 打开/隐藏；↑/↓ 选择，Enter 执行，Esc 或失焦隐藏。托盘图标左键打开，右键打开/退出；热键冲突显示提示。
- 唤醒前捕获应用及 HWND，Shell COM 查询该资源管理器的可见活动视图；实际目录提供目录动作。Home/此电脑等虚拟页面不返回目录，不沿用上一次目录。
- Open Terminal Here 打开 PowerShell 并切换到目标目录；Reveal in File Manager 定位并选中文件。打开 URL/文件使用系统默认程序，复制使用 CF_UNICODETEXT。
- 非空输入仅显示该输入的动作，清空后恢复本 Session 的环境快照；每次唤醒是新 Session。
- Windows 字体为 Segoe UI；Windows 11 请求系统圆角/背景材质，旧版系统使用深色底色。具体渲染效果、资源管理器标签页识别均待实机确认。
- 数据、插件、日志分别在 `%LOCALAPPDATA%\Anywhere\usage.sqlite3`、`plugins`、`launcher.log`。日志每次启动覆盖，故障后先留存。

## 人工 case

所有项目初始均为「未测」。先在 PowerShell 准备专用数据：

```powershell
$caseRoot = Join-Path $env:TEMP 'Anywhere-Manual'
New-Item -ItemType Directory -Force (Join-Path $caseRoot '目录 A'), (Join-Path $caseRoot '目录 B') | Out-Null
Set-Content -LiteralPath (Join-Path $caseRoot '目录 A\hello world.txt') -Value 'Anywhere manual validation 中文' -Encoding utf8
$caseRoot
```

记录输出的实际根路径；下表 A/B/文件分别指其中的 `目录 A`、`目录 B`、`目录 A\hello world.txt`。打开资源管理器后，用 Ctrl+L 粘贴实际路径、Enter 导航。没有执行的 case 保留「未测」。

| 编号 | 操作 | 通过标准 |
|---|---|---|
| W01 | 解压完整应用包，从文本编辑器前台启动；Esc 后按 Ctrl+Alt+Space | 有且只有一个窗口；无 DLL/资源缺失；输入可直接打字；托盘可退出；重启后仍正常 |
| W02 | 资源管理器打开 A，唤醒并保持空输入 | 有 Terminal / Copy Path / Open Directory，副标题目标为 A；无需手动输入路径 |
| W03 | 执行 A 的 Open Terminal Here | PowerShell 打开，`Get-Location` 为 A，中文与空格正确；Launcher 隐藏 |
| W04 | 从 A 唤醒并 Copy Path，去编辑器连续粘贴两次 | 两次都是 A 的完整有效路径；不是空文本；后续普通文字复制粘贴仍正常 |
| W05 | 输入测试文件完整路径并 Open File | 默认程序打开正确文件，内容完整；不是只打开目录 |
| W06 | 同样输入文件并 Reveal in File Manager | 资源管理器打开父目录并选中该文件；该动作不再叫 Reveal in Finder |
| W07 | 从 A 唤醒，输入 B；分别执行 Open Directory、Terminal、Copy Path；每次重新唤醒输入 | 全部针对 B，列表不混入 A；清空输入才恢复本次环境目录 |
| W08 | 输入 `https://github.com`，分别执行打开/复制；输入 `hello 世界 &admin=true#x/%` 执行 Google 搜索 | 默认浏览器地址正确；Copy URL 可规范化末尾 `/`；搜索词整体保留；无目录动作残留 |
| W09 | 输入 `{"name":"中文","age":18}`，分别 Format、Minify、Copy Formatted JSON | JSON 预览和复制有效；↑/↓ 能返回动作列表；复制后可在编辑器粘贴 |
| W10 | 连续手打英文、用中文输入法组词/确认；粘贴后逐字退格、长按退格、Ctrl+A 删除 | 不闪退；候选确认不执行动作；删除后状态正确；输入符号不丢失 |
| W11 | 先从 A 唤醒，再从文本编辑器唤醒 | 第二次无输入时没有旧 A 的目录动作；输入为空 |
| W12 | 分别从资源管理器主页、此电脑、回收站、搜索结果页唤醒 | 虚拟页面没有目录动作、没有旧路径残留；仍能输入文本/URL；若系统对应视图实际映射目录，记录视图和返回路径供核对 |
| W13 | 打开 A/B 两个资源管理器窗口，交替从各窗口唤醒；Windows 11 再分别选 A/B 标签页后唤醒 | 返回真正前台窗口及活动标签页目录；不固定取第一个窗口/标签页；未有 Windows 11 时标签页子项记未测 |
| W14 | 唤醒后点击其他窗口或 Alt+Tab；再唤醒；输入文本/JSON/清空 | 失焦隐藏，新 Session 不残留；高度按动作数量变化，空动作仅搜索栏；迟到结果不重新弹窗 |
| W15 | 临时复制 `plugins/manual-fixtures/manual-window-size.toml` 到用户插件目录并重启，输入文本，用鼠标滚动及方向键循环 | 默认共 10 个动作，最多约 7 行可见，额外动作可到达；测试后退出并移走此样例，再启动 |
| W16 | 从同一应用反复执行 JSON Minify，退出重启后再次输入 | 历史保留，同类动作有排序加分；URL 输入仍不出现环境目录动作 |
| W17 | 输入不存在路径、非法 URL、非法 JSON；将实际文件改名后再重新唤醒输入新旧路径 | 不崩溃；不存在对象不给文件动作，新路径重新识别；实际执行失败有错误提示，随后可继续输入 |
| W18 | 在浅色和彩色背景前观察空搜索栏、列表、JSON 预览；重复 W10；查看日志 | 字体和副标题清晰；圆角不露出黑色直角块；无闪退；日志有 Core Session/Revision、动作事件及 hotkey_to_visible_ms。未支持背景材质的系统记录版本，不按 Win11 毛玻璃验收 |

其他插件合法/非法隔离、输入边界和历史细项可继续复用 [完整清单](manual-validation.md) 的 X/H/P/R，替换系统快捷键、路径和用户插件位置。Windows 适配并未改变 Core 规则；不能因为 Mac 通过就将 Windows 对应项标为通过。
