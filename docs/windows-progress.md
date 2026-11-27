# Windows 功能实现进度

核对日期为 2026-11-27。源码基于 `0f18732` 加上当前工作树中的 Windows
实现，最终候选包为 r16。构建入口见 [Windows 开发预览指南](windows.md)，
实机步骤见 [Windows 桌面验收指南](test-acceptance-windows.md)。

本页把“代码已接入”“自动回归通过”和“桌面验收通过”分开记录。r16 的完整
证据、状态数和未执行条件保存在
`build/windows-physical-20261127-r16/report.md`。

## 已实现

| 功能 | 当前源码范围 | r16 实机与回归证据 |
| --- | --- | --- |
| Launcher、托盘与设置 | 全局 / Current App 双热键、通知区域菜单、隔离数据目录启动、设置持久化、独立 Shelf 与 Tasks、系统主题和透明效果降级 | r16 从隔离数据目录启动，两个热键均经桌面注入唤醒。r15 扩展桌面基线覆盖 Esc、Shelf、Tasks、设置冲突和重启；r16 只把不改变行为的样式整理纳入最终包 |
| 应用搜索与 Current App | 开始菜单快捷方式、Shell 打包应用、拼音别名、自定义目录、前台窗口身份、Explorer 目录 / 选中项与虚拟目录拒绝 | r15 基线实测 `jisuanqi`、计算器、设置、7-Zip、两个 Explorer 目标和“此电脑”；r16 自动回归通过 |
| 标准 Win32 菜单栏 | 递归初始化动态子菜单，读取完整菜单路径、命令 ID、启用状态和窗口身份，执行前重新验证 | r15 基线通过 7-Zip Help / About，r16 的 4 项 Windows 菜单单元测试全部通过 |
| 文件搜索 | Combined、Everything、Windows Search、Ripgrep、直接遍历；范围复核、隐藏项、类型过滤、排序、分页、打开、Explorer 定位和复制路径 | r15 基线完成 30 项分页、过滤和普通盘符路径展示；r16 文件对象动作、搜索与路径回归通过 |
| 剪贴板历史与 Shelf | 默认关闭；Unicode 文本、文件列表、PNG / DIB、HTML / RTF、排除标记、重新复制和独立 Shelf | r15 基线通过开启前不补录、中文文本筛选、重新复制、Shelf 预览与复制返回；r16 剪贴板平台回归通过 |
| 按需预览 | 当前用户运行 QuickLook 时预览一个文件；缺少依赖、已有外部窗口和旧预览释放可诊断 | r15 基线通过 QuickLook 4.5.0 的 PDF、PNG、TXT、缺失提示和外部窗口保护；r16 的 3 项预览回归通过。QuickLook 自有 WPF 窗口不能被 CUA 定位，关闭控件仍待人工复验 |
| Git 与项目任务 | Git 固定只读命令；Node 包管理器解析；Job Object 管理任务树；Running Tasks 输出和 Stop | r15 基线通过 Status、Diff、Staged Diff、Log、分支复制和非仓库目录。npm `Run` 经 Node JS CLI、`cmd.exe`、PowerShell 子进程运行并可停止；r16 的 Node 与平台回归通过 |
| Web 工具 | WebView2 宿主、独立窗口、权限审批、按需 QuickJS 和内置 JSON Workspace | r15 基线通过 JSON 输入 / 计算 / 复制、独立窗口重开和后台格式化动作；r16 完整平台与 GPUI 检查通过。插件安装批准与撤销矩阵仍未执行 |
| x64 构建与 ZIP | Windows 原生构建脚本与 POSIX 兼容打包脚本，ZIP 内含 EXE、许可证和 Windows 指南 | r16 是已验证的 x64 PE，ZIP 内外 EXE SHA-256 一致，28 个 ZIP 条目完整。当前包未签名 |

## 未实现

| 功能 | 现有边界 |
| --- | --- |
| Windows 自定义菜单与 Ribbon | 只支持标准 Win32 菜单栏。无菜单、低权限和自定义菜单只保留其他 Current App 动作 |
| 系统内置预览、多文件预览 | 依赖外部 QuickLook，一次预览一个路径 |
| 任意私有剪贴板格式、虚拟文件与延迟文件对象 | 只实现当前声明的文本、文件列表、图像、HTML 与 RTF 能力 |
| 浏览器历史 / 书签 / Tab 管理、Obsidian 最近笔记、多安装实例或 Profile 选择、交互式 PTY | 属于共享产品的后续范围 |
| 正式 Windows 分发 | 不包含安装程序、Authenticode 签名、自动更新或运行库随包部署，当前发布物为 x64 ZIP |

## 待修复或待验证

| 项目 | 当前状态 | 复验入口 |
| --- | --- | --- |
| 托盘退出 | 菜单五项已实际显示，但 CUA 不能把 Windows 原生 `#32768` 菜单绑定为跨进程输入目标。r16 已做强制终止后的热键释放与重启辅助检查，不能替代托盘 Exit | [W-ENV-03](test-acceptance-windows.md#产物与启动) |
| 默认数据目录单实例 | 隔离 `ANYWHERE_DATA_DIR` 明确绕过产品单实例 Mutex，r16 未在专用用户的默认目录执行第二入口 | [W-ENV-04](test-acceptance-windows.md#产物与启动) |
| QuickLook 自有窗口关闭 | CUA 不枚举 QuickLook WPF 预览窗口。外部窗口保护已通过，预览自有控件需要可定位该窗口的桌面会话 | [W-PRE-02](test-acceptance-windows.md#预览) |
| Shelf 删除 | 界面删除本地数据需要动作时确认，本轮未执行；其他 Shelf 行为已在 r15 基线通过 | [W-SHELF-01](test-acceptance-windows.md#剪贴板与-shelf) |
| 物理 IME、真实键鼠、双屏混合 DPI、读屏 | 本轮输入来自代理桌面注入，桌面锁定为单一 CUA 捕获面，无法替代对应硬件和辅助技术阶段 | W-KEY、W-KEY-03、W-UI-01 |
| 登录项、Explorer 重启、干净 VM 缺运行库 | 需要专用用户或可回滚 VM，本轮未执行 | W-SET-03/04、W-ENV-05、W-WEB-03 |
| Explorer 标签页、浏览器矩阵与来源开关 | r15 已覆盖两个 Explorer 窗口和虚拟目录，标签页、浏览器版本矩阵与 Context 来源开关仍待执行 | W-CTX-01/02/03/04 |
| Windows Search / Everything / Ripgrep 引擎矩阵 | 产品和单一 Windows Search 单元路径已接入，r16 未对五个引擎逐一切换桌验 | W-FILE-04 |
| Web 插件安装、更新和撤销 | 内置 JSON Workspace 路径已验证，第三方可信包生命周期未执行 | W-WEB-02 |
| 第一方 Clippy | r16 源码已清除两处第一方告警，命令以 `-D warnings` 退出 0。第三方 `gpui-fast-windows` 仍有未使用导入告警，不属于当前产品源码 | W-ENV-02 |

## r16 修复范围

| 项目 | 源码行为 | 验证 |
| --- | --- | --- |
| Windows 标准菜单 | 读取和投递前递归发送 `WM_INITMENU` / `WM_INITMENUPOPUP`，动态菜单变化纳入复验 | 4 项菜单单元测试通过，7-Zip About 在 r15 基线完成桌面执行 |
| Node 包管理器 | Windows 优先真实 EXE，随后使用相邻 `node.exe` 直接运行 npm / Corepack JS CLI，不经过 `cmd.exe` 参数插值 | Node 定向测试、完整平台测试和 r15 实机任务链 |
| 路径展示 | 普通盘符路径展示时去掉冗余 `\\?\`，安全校验和执行继续使用原始对象 | 文件对象动作和 MVP 路径回归通过，四个旧断言已同步到当前契约 |
| 进程与任务 | 使用 Job Object 终止受管进程树；后台输出继续排空 | r15 npm 任务父子进程全部退出，r16 完整平台测试通过 |
| Clippy 整理 | 删除 `process/tooling.rs` 的冗余 `return`，将前台线程取值改为直接 `if` | r16 第一方 Clippy、Release 构建、平台与 MVP 回归均通过 |

## r16 工程结果

| 检查 | 结果 |
| --- | --- |
| Release 构建 | exit 0 |
| `anywhere-platform` 串行测试 | 61 passed，0 failed |
| MVP 串行集成测试 | 177 passed，0 failed，4 ignored |
| Windows 菜单定向测试 | 4 passed |
| Node 包管理器定向测试 | 1 passed |
| QuickLook 预览定向测试 | 3 passed |
| 文件对象动作定向测试 | 1 passed |
| GPUI check | exit 0 |
| 第一方 Clippy | exit 0 |

完整命令、输出和哈希清单位于 r16 证据目录。桌面验收仍按
[Windows 桌面验收指南](test-acceptance-windows.md)逐轮记录，不把构建、
自动测试或代理桌面注入等同于真实物理键鼠结论。
