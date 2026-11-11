# V0.14 GPUI 技术方案与验收

状态：开发预览，功能接入完成，自动化与物理桌面证据分开记录。最后核对：
2026-11-11。

本页定义当前交付范围和证据边界；实现边界见
[GPUI 产品架构与迁移契约](gpui-migration.md)。

## 交付范围

| 区域 | 当前行为 | 自动化状态 |
| --- | --- | --- |
| 产品壳 | macOS 15+ 主窗口、状态栏 `A`、全局热键、关闭后隐藏 | Covered / Physical Unverified |
| Launcher | Universal / Current App、Scope、菜单、查询、选择、对象动作、结果 | Covered / Physical Partial |
| 文件搜索 | Spotlight + 完整遍历、过滤、连续结果、Quick Look 路由 | Covered / Physical Partial |
| Clipboard | History、策略、独立 Shelf 窗口、复用与关闭清理 | Covered / Physical Partial |
| Settings | 热键、Context、目录、剪贴板、应用历史、Feature、语言、登录项 | Covered / Physical Partial |
| Application history | 18 个来源、配置与筛选、页面对象动作 | Covered |
| Tools | NativeTool 路由、Plugin Detail、未知类型错误 | Covered |
| Web plugins | 安装审查、启停、固定 origin、IPC、ack、主题与 teardown | Covered / Native Unverified |
| Packaging | `Anywhere.app`、固定 Bundle ID、签名、ZIP 和 SHA-256 | Scripted / Local Verified |

Covered 表示有代码路径和自动化回归，不表示真实窗口、键盘、系统权限或视觉
验收已经完成。

## 稳定边界

- Rust Core 拥有 Context、Action、匹配、排序、授权、执行资格和资源预算。
- GPUI 拥有页面、输入、选择、焦点、窗口和展示状态。
- AppKit、Quick Look、WebKit、状态栏、热键与登录项只作为薄系统适配器。
- Scope 过滤 Action，Context 决定适用性；Current App 聚合可用领域并包含菜单，
  菜单不单独占一个 Scope。
- 不可用菜单隐藏；缺少辅助功能权限只移除菜单，不阻断普通搜索与动作。
- 版本和身份检查在异步发布与副作用开始前复验；稳定 ID 不单独授权旧目标。
- 文件、目录、工作区文件与远程 URI 保持不同类型；不猜测工作区目录或应用。

## 自动化证据

2026-11-11 在 macOS 27.2.1（build 26B131）/ arm64、Rust 1.100.0-beta.7 上，
于 `codex/experiment-gpui-kit` 工作区完成以下验证：

| 门禁 | 结果 |
| --- | --- |
| `scripts/check-gpui.sh` | 通过；GPUI 27 项测试通过 |
| `cargo test --workspace --locked` | 707 passed、6 ignored、0 failed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过，无 warning |
| `scripts/check.sh` | 通过；格式、生成物、511 条双语消息、97 篇文档 / 642 个链接、原生探测、Rust、Web SDK 与示例、架构边界全部通过 |
| `scripts/check-web-sdk.sh` | 50 项通过，包括真实 Rust broker、二进制帧、取消、隐藏 / 恢复和 1 MiB 文本传输 |
| `scripts/check-web-examples.sh` | JSON 示例 30 项通过；三个插件包、TypeScript 后台 SDK、QuickJS 大文本和工具模板构建通过 |
| `git diff --check` | 通过 |

工作区测试计数由 Core 352、Core architecture 6、execution sessions 5、feature
catalog 5、MVP 195、search fields 3、GPUI 27、Platform 72、Plugin 34、Protocol
7 和 Protocol reexports 1 组成。忽略项均标注了需要外部工具或大型 fixture 的
显式原因。

当前自签发布包验证：

```text
scripts/build-macos.sh
0.14.0+1 -> build/Anywhere.app

codesign --verify --deep --strict --verbose=2 build/Anywhere.app
valid on disk; satisfies its Designated Requirement

scripts/package-macos.sh build/Anywhere.app
build/Anywhere-macos-arm64.zip
build/Anywhere-macos-arm64.zip.sha256

shasum -a 256 -c build/Anywhere-macos-arm64.zip.sha256
OK
```

包结构必须满足：

- `CFBundleIdentifier = dev.anywhere.launcher`
- `LSMinimumSystemVersion = 15.0`
- `CFBundleExecutable = Anywhere`
- 主程序存在且签名有效
- 不存在其他 UI 框架或跨语言桥接运行库

本次包已逐项核对：可执行文件为 arm64 Mach-O `Contents/MacOS/Anywhere`；
`otool -L` 只列出 macOS 系统框架与系统库；包内只有主程序、图标、许可、
双语原生文案，没有其他 UI framework 或跨语言桥接运行库。

## 已完成的物理验收

2026-11-11 在 macOS 27.2.1（build 26B131）/ arm64 上使用独立数据目录和
`dev.anywhere.launcher.acceptance` 验收包实际完成以下序列：

- Launcher 能查询并打开 Clipboard Shelf；设置页的通用、快捷键、搜索、
  剪贴板、功能分类和关闭路径可见且可操作。
- 文件搜索查询没有隐式首项；首次 `Down` / `Up`、后续连续选择、
  `Enter`、`⌘K`、`⌘Y`、`Space`、Esc 分层返回和查询空格均实际执行。
- Clipboard History 与独立 Shelf 的查询空格、首次选择、连续切换和关闭后
  保留状态均实际执行；Shelf 首帧直接显示 5 条记录。
- Shelf 失焦到 Finder 后仍接收其可见区域的点击，确认
  `NSFloatingWindowLevel` 生效；Esc 先关闭 Quick Look、再关闭 Shelf，
  关闭后重新执行命令可再次显示数据。
- 文本 Quick Look 通过 `Space` 打开，使用 `Space` 或 Esc 关闭；关闭后
  查询、选择以及返回输入框的状态保持。验收日志中
  `quick_look_show_finished success=true`、`preview_open=true`，且未出现
  `RefCell already borrowed`。

## 物理桌验矩阵

以下项目必须使用有交互桌面的 macOS 15+ 本机并记录设备、系统、构建号和操作
结果。Rust 单测、编译、签名或截图本身不能将它们标记为通过。

| 项目 | 必须复现的序列 | 当前状态 |
| --- | --- | --- |
| 中文 IME | 组合输入、候选窗、字符输入、Enter、Esc、方向键；确认 GPUI 不抢占 | Unverified |
| 状态栏恢复 | 隐藏主窗口，点击状态栏 `A`，核对窗口、模式和焦点 | Unverified |
| 全局热键 | 两种默认模式唤醒；冲突、注销和重新注册后的行为 | Partial；无冲突快捷键注册成功且冲突会保留可用应用，CUA 合成按键未进入系统全局热键事件流；真实唤醒、注销和重注册未验证 |
| Clipboard Shelf | 独立置顶、重复打开复用、Esc、系统关闭、失焦和主窗口恢复 | Partial；置顶、首帧、失焦、Esc、关闭后重开已验证；同窗复用、系统关闭和主窗口恢复未验证 |
| Quick Look | 空格打开/关闭、连续切换、唤醒、关闭后焦点回到正确输入框 | Partial；文本文件和剪贴板文本、Space / Esc、选择保留已验证；媒体、多文件、唤醒与完整 IME 焦点未验证 |
| WebView | 包加载、固定 origin、IPC、ack、焦点、替换与 teardown、helper 回收 | Unverified |
| Accessibility | 权限缺失时菜单隐藏，其他动作可用；授权后重新扫描 | Partial；已授权状态发现 193 个菜单项并显示 159 个动作；撤销与重新授权未验证 |
| 登录项 | 注册、系统批准、关闭、重新打开与独立数据目录不访问系统项 | Unverified |
| 材质与布局 | Reduce Transparency、浅/深主题、缩放、窗口尺寸和可见行 | Partial；基线窗口、设置和多页面布局已验证；系统外观和缩放未验证 |
| VoiceOver | 主要页面、输入、结果、设置和返回路径 | Unverified |

物理验收发现缺陷时，先建立最小可复现序列。IME 或焦点问题不得用模拟按键直接
改写生产语义；修复后再重复原序列并保留未覆盖边界。

## 已知边界

- Windows / Linux 继续只验证共享 Rust crates，当前没有 GPUI 桌面产物。
- Developer ID 签名、公证、干净机器升级和自动更新尚未完成。
- 第三方插件的 UI 在 WebView 中运行；需要原生 WebView 的完整物理矩阵。
- Web 后台 QuickJS 仍为进程内执行；插件进程隔离尚未交付。
- 浏览器历史、书签、Tab 管理、插件市场与终端的 PTY 支持不属于本轮范围。
- 当前没有整 App GPUI 性能承诺；历史采样不能作为当前版本结论。

## 退出条件

本轮可标记为开发预览完成需要同时满足：

1. 快速 GPUI 门禁和完整工作区测试通过。
2. 完整 `scripts/check.sh` 通过，文档、本地化、设计参数和架构检查无漂移。
3. 发布 App 构建、签名、plist 和动态库检查通过。
4. 工作树中没有其他 UI 实现、桥接 crate、构建入口或现行文档依赖。
5. 物理矩阵逐项记录；未完成项保持 Unverified，不用自动化结果替代。
