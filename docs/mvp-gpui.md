# V0.14 GPUI 技术方案与验收

状态：开发预览，功能接入完成，自动化与物理桌面证据分开记录。最后核对：
2026-11-18。VoiceOver 明确不在当前验收范围。

本页定义当前交付范围和证据边界；实现边界见
[GPUI 产品架构与迁移契约](archive/gpui-migration.md)。

## 交付范围

| 区域 | 当前行为 | 自动化状态 |
| --- | --- | --- |
| 产品壳 | macOS 15+ 菜单栏后台应用、无边框浮动 Launcher、搜索图标菜单、全局热键 | Covered / Physical Partial |
| Launcher | Universal / Current App、Scope、菜单、查询、选择、对象动作、结果 | Covered / Physical Partial |
| 文件搜索 | Spotlight + 完整遍历、过滤、连续结果、Quick Look 路由 | Covered / Physical Partial |
| Clipboard | History、策略、独立 Shelf 窗口、复用与关闭清理 | Covered / Physical Partial |
| Settings | 热键、Context、目录、剪贴板、应用历史、Feature、语言、登录项、标题栏草稿路由 | Covered / Physical Partial |
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

当前工作树的实施、门禁、实机结果与保留理由维护在
[2026-11-18 工作树审查](archive/reviews/worktree-review-2026-11-18.md)。
2026-11-18 重新执行两套门禁：GPUI 278 项通过；`scripts/check.sh` exit 0，完整
Rust 工作区、SDK 54 项、JSON Workspace 31 项、严格 Clippy、原生边界与
115 份 Markdown / 941 个本地链接检查通过。随后重新构建、签名并从精确 App
路径启动隔离进程；完整物理矩阵仍有未执行项。自动化不能沿用为真实 IME、材质、
系统授权或辅助技术通过结论。

Build 16–167 及后续阶段的逐次结果、测量与当时的桌面证据已移至
[历史开发记录](archive/v0.14-gpui-development-records.md)。历史通过只适用于原构建，
不能证明本轮修改后的输入法、材质、Quick Look 或 WebView 行为。

## 本轮包与启动证据

- `build/Anywhere-review-20261117-hotkeys-fix.app` 构建为 `0.14.0+1`、arm64、最低 macOS
  15；严格 codesign、Designated Requirement、Info.plist 和动态依赖检查通过。
  签名主体是本地 `Anywhere Local Code Signing`，不是 Developer ID 或公证；
  `spctl --assess` 按预期拒绝，当前产物仅用于本机验收。
- `ANYWHERE_PREVIEW=1` 从该包直接启动后，Accessory 包显示
  `648×360.5 pt` 窗口，首行和输入区获得焦点，状态项与 Liquid Glass 材质安装
  日志正常。截图 `build/anywhere-review-preview-window-20261117.png`。
- 关闭预览并使用独立数据目录后，真实 Core 以非激活、隐藏的 Accessory 应用启动；
  四个 SQLite 的完整性检查和 `features.json` 解析通过，所有数据句柄均位于隔离
  目录。
- 旧实例占用默认快捷键时，审查实例记录注册失败、恢复旧绑定并降级状态菜单；
  使用独立 QA Bundle ID 和空闲的 `⌃⌥⇧J` / `⌃⌥⇧K` 时，两条热键注册成功。
- Preview 状态栏 Settings 曾被 `shell.live.is_none()` 错误拦截。修复后
  `open_settings` 返回真实结果，Starting/Failed 仍不切换页面；QA Preview 已实际
  显示七个设置分类和快捷键录制器，截图
  `build/anywhere-settings-fix-window-20261117.png`。
- 当前默认唤醒键为 `⌃⌥Space` / `⌥Space`。旧默认键对会在读取设置时迁移，
  Option-only 主键与 `Option+K` 类动作键均可录制；真实注册日志确认三个快捷键
  同时成功，截图 `build/anywhere-hotkeys-fix-settings-20261117.png`。
- Computer Use 在唯一 QA Bundle 中实际完成文本搜索、Return、Esc、不可用提示和
  首屏恢复。临时 QA 包装只解决同 Bundle ID 的自动化绑定，生产审查包身份与签名
  由上面的静态检查单独确认。

2026-11-18 发布前复验使用 `build/Anywhere.app`，`scripts/check-gpui.sh`
278 项通过。发布包的源码 commit、主程序和 ZIP 的 SHA-256、架构、最低系统及
签名身份记录在 `v0.14.0` Release 的 `package-provenance.json` 中；可变的本机
构建时间不写回源码文档。`v0.14.0` 已作为 GitHub pre-release 发布，未替换
历史 V0.3 的 `latest` 标记。

隔离验收使用精确 App 路径、Bundle ID `dev.anywhere.launcher` 和独立数据目录。
实机确认文件下一页加载与
跨页选择、系统 Quick Look 的 `⌘Y` / Esc、剪贴板查询与设置刷新、Settings
草稿确认和保存回读、热键重绑定后旧键注销 / 新键生效 / 原键恢复、
Running Tasks 启动 / 输出 / 完成 / 停止、JSON Workspace 合法与非法输入、
宿主 Reload / Back，以及 Cursor Application History 查询与 Esc 清空。
详细证据和身份记录保留在本地忽略的验收目录及
[工作树审查](archive/reviews/worktree-review-2026-11-18.md)。

同一构建的定向复验已经从 Finder 所选 Node 目录取得四个 `npm Run` 动作，
也在文件结果进入浏览状态后通过注入 Space 打开系统 Quick Look。实机硬件 Space、
真实中文 IME 和任务“移除结果”的 GUI 用例仍待验。其余边界见
[2026-11-18 实机复验](archive/reviews/worktree-review-2026-11-18.md#2026-11-18-实机复验)。

## 物理桌验矩阵

下面保留各构建已有的物理证据摘要。当前源码的完整分项执行使用
[完整测试用例与验收清单](test-acceptance-checklist.md)：新清单统一从 Pending
开始，覆盖全部现有功能与18个来源，不继承旧构建的 Pass。

以下项目必须使用有交互桌面的 macOS 15+ 本机并记录设备、系统、构建号和操作
结果。Rust 单测、编译、签名或截图本身不能将它们标记为通过。

| 项目 | 必须复现的序列 | 当前状态 |
| --- | --- | --- |
| 中文 IME | 组合输入、候选窗、字符输入、Enter、Esc、方向键；确认 GPUI 不抢占 | Partial；当前 live Core 包实际显示候选窗，Space 提交“你好”并写入 Launcher editor；Enter、方向键和设置页完整组字矩阵仍待验 |
| 菜单栏恢复 | 启动后核对无窗口、无 Dock；隐藏 Launcher，点击菜单栏搜索图标，分别选择 Current App / Universal Search、Running Tasks、Settings 和 Quit | Partial；当前包物理点击搜索、Tasks、Settings 与菜单快捷键均进入正确入口，Tasks 重复点击复用同一 920×600 窗口；Dock 泄漏与所有休眠 / 唤醒组合仍待重复 |
| 全局热键 | 两种默认模式唤醒；冲突、注销和重新注册后的行为 | Partial；2026-11-18 当前包把 Current App 从 `⌃⌥⇧O` 改为 `⌃⌥⇧J` 并保存后，旧 J 不再唤醒、新 J 正常显示 Launcher；恢复 O 后旧 J 无效、O 再次唤醒。此前的双模式时序与 key-window 采样见旧构建记录；冲突与部分失败恢复仍待验 |
| Launcher 焦点 | 呼出后无需点击即可输入；置顶；失焦隐藏；根页 Esc 按 Content → Current App → Global → 隐藏返回；内部预览 / Web 焦点不误隐藏 | Partial；Build 123 实测 WebView 聚焦 Esc 返回且保留查询，Tasks / Shelf 为 key window 时热键呼出立即 `key=true`，Launcher 失焦隐藏；Current App 空白根页两次 Esc 链保持不变；Quick Look、多 Space 与预览豁免仍待验 |
| Settings 关闭 | 无草稿 Esc、脏草稿取消 / 放弃 / 保存、系统关闭和失败；普通窗口层级 | Partial；2026-11-18 当前包实测脏草稿 Esc 出现保留确认框，继续编辑保留草稿，保存并关闭后重开仍显示新值；剪贴板录制开关即时持久化，Settings 重开后历史条数与容量刷新。保存失败、系统关闭与输入法组字期间 Esc 仍待验 |
| Clipboard Shelf | 独立置顶、右下角定位、最小化恢复、迟到复制返回、Esc、系统关闭和失焦 | Partial；Build 123 实测 `layer=3`，切到 TextEdit 后保持，点击交通灯关闭才消失；Build 113 实测最小化复用不 panic、复制并返回 TextEdit 且无 destination warning；多 Space 和长时间关闭 / 恢复仍待验 |
| Quick Look | 空格 / `⌘Y` 开关、连续切换、`⌘L` 返回、唤醒、关闭后焦点回到正确输入框 | Partial；2026-11-18 当前包在文件搜索中用 `⌘Y` 打开系统 Quick Look，显示预期文本，Esc 后查询、选择与页面保留；剪贴板文本也完成预览 / Esc。合成 Space 受自动化字符输入标志影响，未作为物理 Space 通过；媒体、多文件、唤醒与完整 IME 焦点仍待验 |
| WebView | 包加载、固定 origin、IPC、ack、焦点、宿主 Reload / Back、Esc、替换与 teardown、helper 回收 | Partial；2026-11-18 当前包加载 JSON Workspace，合法输入格式化、非法输入显示准确 `InvalidSymbol`；宿主操作菜单的 Reload 更换 document epoch 并恢复空态，Back 返回原查询和结果行。聚焦 Esc IPC、旧文档替换、长时间循环与 helper 回收仍待验 |
| Application history | 来源加载、查询、Esc 清空、对象动作与外部打开 | Partial；2026-11-18 当前包加载 Cursor 最近项目与文件，测试查询只保留匹配项目和文件，Esc 清空查询并恢复列表；没有打开外部项目，迟到查询与对象动作仍待验 |
| Accessibility | 权限缺失时菜单隐藏，其他动作可用；授权后重新扫描 | Partial；Settings 焦点缺少 AX 节点的警告已修复，分类与录制器可在 AX 树中经 Tab 导航；撤销 / 重新授权仍未验证 |
| 登录项 | 注册、系统批准、关闭、重新打开与独立数据目录不访问系统项 | Unverified |
| 材质与布局 | Reduce Transparency、浅/深主题、缩放；无交通灯；72 / 3、5、7 行与顶部锚点；失焦隐藏 | Partial；当前包实测 `72 → 137 → 473 → 72` 顶部坐标固定为 y=312，473 是合法 7 行上限，Finder 失焦隐藏；520 只有布局 clamp 回归。Reduce Transparency、Increase Contrast、浅色环境、缩放和完整逐页截图仍待验 |

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
