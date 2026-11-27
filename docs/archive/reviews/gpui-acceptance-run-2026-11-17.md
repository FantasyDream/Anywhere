# GPUI 当前构建验收执行记录

执行日期：2026-11-17（Asia/Shanghai）。本记录按
[完整测试用例与验收清单](../../test-acceptance-macos.md)执行，只记录本轮
实际观察。未执行项不继承旧构建、自动化测试或截图结果。

14:36 之后已从当前工作区重新打包并做了新的一轮实机复验，结果见文末
[修复后构建实机复验](#2026-11-17-1436-后修复构建实机复验)。上一轮
11:31 包相对 14:01–14:05 的源码修改已经过期，不能用该旧包判断两项修复
是否生效。

## 执行状态

本轮先完成工程门禁、发行包身份与部分真实 App 交互。复核 CUA 进程身份时发现
机器同时保留相同 Bundle ID / 显示名的旧 Flutter `Anywhere.app`，按应用名连接
会重新拉起该实例；因此第一轮无法证明归属的界面交互已撤回为 Blocked，不能作为
GPUI 结果。当前已准备唯一显示名与 Bundle ID 的 GPUI 包装副本，但执行中途再次
遇到 macOS 锁屏。当前记录不表示 322 条清单已全量通过。

## 被测身份

| 项目 | 值 |
| --- | --- |
| 源码 HEAD | `b428899c9ce70c92e12de24950405222ea3b6c28` |
| 工作区 | dirty；执行前 62 个 `git status --short` 条目，52 个已跟踪文件变更，新增 2,414 / 删除 735 行 |
| 工作区 diff | `build/acceptance-20261117-055013/evidence/worktree-diff.patch`，SHA-256 `5e416a51ba00b8b3f075847f7369c1333b594f58c0e949417d3e320e36365907` |
| App | `build/Anywhere-acceptance.app` |
| 版本 / 构建号 | `0.14.0` / `1` |
| Bundle ID | `dev.anywhere.launcher` |
| 主程序 SHA-256 | `4018ab159c179a2d676f611a167bbb296cd83b922259468ea938992f64da33ad` |
| 签名 | `Anywhere Local Code Signing`；`codesign --verify --deep --strict` 通过 |
| 架构 / 最低系统 | arm64 / macOS 15.0 |
| 执行环境 | macOS 27.2.1，arm64 |
| 隔离数据 | `build/acceptance-20261117-055013/data` |
| 隔离插件目录 | `build/acceptance-20261117-055013/bundled-plugins` |
| CUA 唯一包装副本 | `build/acceptance-20261117-055013/Anywhere GPUI Acceptance.app`；仅将显示名与 Bundle ID 改为唯一值，并从验收解压包复制后重签，用于避免同名 Flutter 实例 |
| 包装副本主程序 SHA-256 | `7b4dd7e67f54ef759f7b3c39dc7a11865fa3ac19b4f06107e5e7dd3d130c9404`（重签会改变原验收二进制摘要） |

`spctl --assess --type execute` 对本地签名包返回 rejected；该包不是
Developer ID / 公证发行物，符合当前开发预览边界，不能据此判定 REL-008 通过。

## 自动化门禁

| 命令 / 检查 | Actual | 结果 |
| --- | --- | --- |
| `scripts/check-gpui.sh` | 244 GPUI tests passed；本地化、资源、plist、格式与严格 Clippy 检查通过 | Pass |
| `scripts/check.sh` | workspace Rust 928 passed / 6 ignored / 0 failed；SDK、示例包、文档、菜单、架构与生成物检查通过 | Pass |
| `git diff --check` | exit 0 | Pass |
| Release 构建 | `scripts/build-macos.sh` 成功，产物 `0.14.0+1` | Pass |
| 严格签名 / plist / 架构 | `codesign --verify --deep --strict`、`plutil -lint`、`lipo -info` 通过 | Pass |
| 动态依赖 | 主程序未引用仓库 `target/`、`/Users/`、`/opt/` 或 `/usr/local/` 运行路径 | Pass |
| ZIP / 解压 | `Anywhere-acceptance-060814.zip` 的 `unzip -t` 无错误；SHA-256 `f42a5e3703283788855ae18ed005c6eb81b533eb8a625475cb225aa8a8e1f442`；解压后严格签名通过；解压前后主程序摘要一致 | Pass |
| 解压产物实际运行 | 进程路径来自 `package-verify-060814/Anywhere-acceptance.app`，无崩溃 | Pass |
| 交付包内容 | Bundle 仅含 plist、主程序、图标、本地化资源与许可证；未发现测试 SQLite / 日志 | Pass |
| REL-008 | 仅本机签名，未做 Developer ID、公证和干净机安装 | Blocked |

## 已观察的交互

| 用例 / 范围 | 实际操作与证据 | 结果 |
| --- | --- | --- |
| ENV-001 | 从本轮 Release 包和再解压副本启动，核对进程路径、版本、签名与主程序 SHA-256 | Pass |
| ENV-004 | 相对路径、根目录、含 `..`、不可创建目录均记录 `shared_runtime_unavailable`，未回退个人目录 | Blocked（fail-closed 日志通过；真实可见文案因锁屏未验） |
| ENV-005 | 修正 LaunchServices 启动方式后，进程只打开隔离数据目录下的 usage / clipboard / extension / web 数据库 | Pass |
| UI 交互组 | 第一轮 CUA 未与同名 Flutter 包严格隔离，无法把窗口、输入、Settings、Tasks 截图归因给 GPUI | Blocked（已撤回；使用唯一包装副本重测） |
| PERF-005 | Release `task_detail_measure`：1,000 次，p50 0.167 µs / p95 0.209 µs / max 2.417 µs；`projection_measure` 10,000 条，full projection p50 21.041 µs / p95 21.459 µs / max 24.75 µs | Pass（仅为合成分配 / 投影测量，不代表 RSS） |
| REL-005 | Bundle ID、版本、最低系统、arm64、签名与许可证与源码配置一致 | Pass |
| REL-006 | ZIP 完整性、解压签名、摘要一致，并从解压副本实际启动 | Pass |

短任务测试动作仅执行：

```text
npm Run · accept-short
> printf 'short-task-ok\n'
short-task-ok
```

## 缺陷记录

### ACC-2026-11-17-01：启动窗口 resize 重现 `RefCell already borrowed`

- 级别：P1。
- 可达条件：本轮每个正常启动都触发，包括原始构建、ZIP 解压副本、有效隔离目录
  和四个无效数据目录参数；至少 7 次启动均复现。
- Actual：初始窗口创建后立即记录：

  ```text
  ERROR : handling resize for window WindowId(1v1): RefCell already borrowed:
  attempted mutable App borrow ... gpui-fast-0.1.14/src/window.rs:2151:22;
  existing mutable borrow group began at ... gpui-fast-0.1.14/src/app.rs:320:33
  ```

- 影响：当前观察中 App 仍继续进入可交互状态，但这是此前物理记录明确要求消失的
  GPUI App 重入错误；不能把非致命现状视为窗口 resize 契约通过。
- 对照：v16 / Build 95 文档记录 Settings / Launcher 样式切换曾修复此类重入；
  当前错误发生在启动阶段，说明回归仍在。
- 证据：`evidence/app-restart-debug.log`、
  `evidence/extracted-app-launch.log`、`evidence/env-*/launch.log`。

### ACC-2026-11-17-02：热键注册失败只写日志，缺少可诊断的用户状态

- 级别：P1。
- 可达条件：默认配置 `⌥⌘Space` 注册失败：

  ```text
  WARN global_hotkeys_unavailable
  HotKey already registered: Modifiers(ALT | META), key: Space
  ```

- Actual：GPUI 进程启动日志记录失败后继续使用默认状态菜单文案；只有换到不冲突
  的测试键 `⌃⌥⇧O` / `⌃⌥⇧P` 后，日志才出现 `global_hotkeys_registered`。
- 影响：主入口可能实际不可用。代码路径尚未把启动注册结果作为设置页错误状态
  发布，但本轮 CUA 未能在无同名干扰的 GPUI 窗口中复验提示，故界面可见结果保持
  Pending；该场景直接对应 ENV-010 / HOT-001 的“不假报注册成功”。
- 证据：`evidence/app-restart-debug.log`、
  `evidence/extracted-app-launch.log`；冲突消除后的
  `evidence/extracted-app-noconflict.log` 显示注册成功。

## 2026-11-17 缺陷修复与复验边界

上述两个 P1 已完成源码修复。原始启动日志、包身份、失败观察与未完成的物理
验收状态仍保留；本节不将旧验收包标为已修复。本次按既有约定仅执行自动化，
没有启动 App 或使用 CUA，也未重新打包。

### ACC-2026-11-17-01

启动路径原来仅向 GCD 主队列入队；嵌套事件循环可能在外层 GPUI App 借用
结束前执行该回调。另一个已确认的不安全路径是 appearance observer 内直接
重装原生材质，调用期间仍持有 GPUI 更新借用。AppKit 的样式 / content view
操作可同步回调 resize，因而仅异步入队不足以保证重入安全。

两条路径现统一使用 `host/native_window.rs`：先让出本次调用，在可失败的
AsyncApp 窗口更新成功并释放借用后执行原生配置，再进入 GPUI 更新页面。
嵌套事件循环探测失败时等待 16 ms 重试，窗口关闭或 owner 释放时取消，避免
迟到操作和重试任务残留。没有通过延长固定启动等待或屏蔽错误日志规避问题。
Regular 材质和无背景蒙版契约保持不变。

新增四项自动化回归：同步 resize 能重新进入 GPUI、嵌套事件循环等待外层
借用释放、owner 释放取消、窗口关闭但 owner 仍被持有时取消并释放回调。
这验证调用边界，不替代原生 AppKit / 材质的实机复验。

### ACC-2026-11-17-02

HotKeyController 现在保留启动注册失败的完整错误，启动器接收该状态，设置页
持续显示中英文诊断和恢复方式；成功刷新设置快照不再抹掉诊断。没有草稿变更
时也显示“重试快捷键”。启动管理器创建失败时，后续保存 / 重试会重新创建；
全局事件分发读取当前控制器，恢复后能接收事件。

状态菜单从启动加载阶段开始仅显示本进程实际拥有的键位。注册失败的默认键
不再作为有效提示；更换失败后显示实际回滚的键位。用户配置保留在编辑器和
持久化中，成功注册才清除诊断。保留菜单入口及其他功能供用户恢复。

新增六项自动化回归：启动冲突可诊断并可重试、Global 冲突清理新注册的
Current App、修改失败回滚与重试、无管理器时不显示默认键位、设置刷新后
中英文诊断和无修改重试按钮仍可达、保存不发布未注册提示或误关设置页。
热键后端使用模拟注册表，不占用本机快捷键。

### ACC-2026-11-17-03：快捷键录制提示被固定宽度截断

- 级别：P2。
- 可达条件：Settings → Shortcuts，点击 Current App 或 Universal Search
  的快捷键录制按钮。
- Actual：修复前按钮固定宽 `98px`；英文显示成
  `Press a short...`，中文也只能显示截断的提示，录制态的主要操作文案不可
  完整读取。下方说明虽包含同类指引，但按钮本身的信息不完整。
- 修复：将录制按钮宽度改为 `132px`，保留原焦点、Esc 取消和录制行为。
- 结果：最终构建英文显示完整的 `Press a shortcut…`，中文显示完整的
  `请按下快捷键…`；`scripts/check-gpui.sh` 254 passed。
- 证据：`build/acceptance-postfix-20261117-1505/evidence/settings/`
  下的 `shortcut-recording-truncated.png`、`shortcut-recording-fixed.png`
  和 `shortcut-recording-fixed-zh.png`。

### 自动化结果

- `scripts/check-gpui.sh`：254 passed / 0 failed，格式、严格 Clippy、生成物和资源检查通过；日志 `/private/tmp/anywhere-acceptance-fix-check-gpui.log`。
- `scripts/check.sh`：workspace Rust 938 passed / 6 ignored / 0 failed，严格 Clippy、生成物、SDK / 示例、文档和架构门禁通过；日志 `/private/tmp/anywhere-acceptance-fix-check-full.log`。
- `git diff --check`：通过。
- 首轮 GPUI 全量曾在已有 `initial_clipboard_reads_cannot_replace_a_newer_search`
  中检测到真实 `anywhere-core` 工作线程唤醒确定性调度器；单项复核与本次
  GPUI 完整重跑通过。该次失败日志没有作为产品修复通过的证据；原测试仍含
  外部线程调度的非确定性限制。

## 2026-11-17 14:36 后修复构建实机复验

### 构建有效性纠正

复验开始时 `build/Anywhere.app` 的主程序时间为 11:31:04，但
`native_window.rs`、`hotkeys.rs`、`settings.rs` 与 `app.rs` 的修改时间在
14:01–14:05。先启动该旧包观察到的 resize 重入、状态菜单默认键位等日志
不能用来判断本轮修复，已从修复结论中撤回。

随后执行 `scripts/build-macos.sh`，取得两代有效构建：

| 构建 | 用途 | 主程序 SHA-256 |
| --- | --- | --- |
| 14:36:43 | ACC-01 / ACC-02 与路径参数复验 | `5fb0929fb8800f9c9f8cb2200f1220e87ae709cf802a09533b00400e4432e3db` |
| 15:08:19 | ACC-03 UI 修复后的最终包 | `e543c815dea7874435e1d0acb04997c0736e18772c7c9c96ab5dd9789ac92c6c` |

两代包均为 `0.14.0+1`、arm64、Bundle ID `dev.anywhere.launcher`；
`codesign --verify --deep --strict` 和 `plutil -lint` 通过。隔离进程环境
逐项核对为 `/tmp/` 下的数据 / 插件目录，没有打开个人
`Library/Application Support/Anywhere`。

### ACC-01：启动 resize 重入

| 场景 | 次数 | Actual | 结果 |
| --- | ---: | --- | --- |
| 14:36 构建，隔离目录直接冷启动 | 3 | 三次均完成初始化；无 `handling resize`、`RefCell already borrowed` 或 panic | Pass |
| 同构建，ZIP 解压副本冷启动 | 3 | 解压前后签名与主程序摘要一致；三次均无上述错误 | Pass |
| 最终 15:08 构建，隔离目录冷启动 | 3 | 三次均无上述错误，热键 `⌃⌥J` / `⌃⌥K` 均注册成功 | Pass |

对三次启动后的进程执行 `lsof`，只看到测试数据目录中的
`usage.sqlite3`、`clipboard-history.sqlite3`、`web-storage.sqlite3` 和
`extension-storage.sqlite3`。这一步证明启动日志与隔离边界，不证明物理
热键唤醒后的首 responder、真实 IME 或 Esc。由于 CUA 不能向 Carbon 全局
热键发送等效的底层按键，活体隐藏窗口无法被本次 CUA 会话唤醒，故
“热键唤醒 → 输入 → Esc”的完整物理链路仍为 Blocked。

### ACC-02：热键注册状态

- 与系统快捷键不冲突时，日志同时出现
  `global_hotkeys_registered current_shortcut=control+alt+KeyJ
  universal_shortcut=control+alt+KeyK`，随后状态菜单日志只列实际注册的
  `⌃⌥J` / `⌃⌥K`。
- 使用新数据目录和默认快捷键启动时，`⌥⌘Space` 仍被系统占用；日志记录
  `global_hotkeys_unavailable`，随后状态菜单标签为“当前应用 / 全局搜索”，
  不再伪报未生效的默认按键。
- 收尾时发现旧构建 PID `85187` 仍在占用测试键；终止旧实例后，只保留最终
  `build/Anywhere.app` 单实例，`⌃⌥⇧O` / `⌃⌥⇧P` 注册成功，状态菜单同步显示
  这两个实际键位。该实例已作为最终运行实例保留。
- Settings 中的中英文冲突诊断、刷新后保留、解除冲突后点击“重试快捷键”、
  保存失败回滚和旧键注销没有取得活体 UI 证据，保持 Pending；不能在本次
  记录中把自动化的模拟注册表结果当作物理 Pass。

### 路径参数 fail-closed

最终包依次使用相对路径、`/`、含 `..` 的路径和不可创建目录启动。每种
场景都记录 `shared_runtime_unavailable` 并保持进程可诊断：

```text
ANYWHERE_DATA_DIR must be an absolute directory
ANYWHERE_DATA_DIR must not be the filesystem root or contain '..'
Not a directory (os error 20)
```

四种场景均没有打开个人数据目录，也没有 resize 重入错误。可见错误文案仍受
活体窗口无法唤醒限制，因此只把 fail-closed 与日志路径记为 Pass；
ENV-004 的 UI 文案仍 Pending。

### Settings 界面检查（Preview 边界）

使用同一最终二进制的 `ANYWHERE_PREVIEW=1` 包装检查视觉和交互框架。该模式
不连接 live runtime，因此以下结果不用于真实存储、权限、任务或插件验收：

- General、Shortcuts、Context、Search、Clipboard、App History、Features
  七类均可达，控件没有消失。
- Launcher 输入框可聚焦并输入；无匹配结果态、逐级 Esc（清查询 →
  切 Root 模式 → 隐藏）和干净 Settings 关闭正常。
- 修改语言形成草稿后关闭会出现“继续编辑 / 放弃修改并关闭 / 保存修改”
  确认；Tab 在模态按钮间移动，Esc 取消后草稿仍保留。
- Settings → Shortcuts 录制按钮可进入录制态，Esc 可取消。
- 上述操作证明的是组件和键盘路由，不证明 live Settings 保存、启动项、
  快捷键注册、真实中文 IME 或历史存储。

### 自动化门禁

- `scripts/check-gpui.sh`：254 passed / 0 failed。
- `scripts/check.sh`：最终重跑 exit 0；日志中 Rust 测试合计 938 passed /
  6 ignored，格式、本地化、生成物、SDK / 示例、文档、菜单、架构和边界检查
  均完成。
- 首轮 `scripts/check.sh` 曾再次命中
  `initial_clipboard_reads_cannot_replace_a_newer_search` 的外部
  `anywhere-core` 调度线程唤醒检测；单项复跑通过，随后全量重跑未再出现
  该失败。该非确定性测试限制单独记录，不归因于本次单行 UI 修复。
- `git diff --check`：exit 0。

### 证据位置

- 构建身份：`build/acceptance-postfix-20261117-1505/identity.txt` 与
  `identity-final.txt`。
- 冷启动 / ZIP / 路径参数日志：
  `build/acceptance-postfix-20261117-1505/evidence/`。
- 七类 Settings、脏草稿确认和录制按钮修复前后截图：
  `build/acceptance-postfix-20261117-1505/evidence/settings/`。
- Preview 的进程日志与 Launcher 截图：
  `build/acceptance-postfix-20261117-1505/evidence/preview/`、
  `evidence/final-ui/ui-session.log`。

本轮没有把 ENV-010、HOT-001、SET-001、SET-004、SET-012、KEY-001、
真实 IME、Quick Look、Clipboard Shelf、Tasks、Web / JSON、
Application History、18 来源、外观矩阵、VoiceOver 或性能 / 发行矩阵汇总为
通过。没有新的 P0/P1 缺陷在本轮有效构建中复现；未执行的活体物理链路继续
保持 Blocked / Pending。

### 后续物理复验与剩余项

先用 `scripts/build-macos.sh` 从修复后的源码打包并记录新主程序摘要；使用独立
数据 / 插件目录，核对进程实际可执行路径和环境。按本报告已有隔离方法避免
绑定到旧 Flutter 实例。每个场景保留单独日志和进程身份；旧包摘要不能替代
本次修复的构建证据。

1. 冷启动与路径参数已按上表完成；仍需在物理热键唤醒后验证输入、中文 IME、
   Esc 和隐藏恢复。
2. 路径参数已 fail closed；仍需在可唤醒窗口里核对可见错误文案。
3. 在 Launcher / Settings / Tasks / Shelf 分别切浅色、深色和自动外观，
   切换减少透明度、增加对比度、减少动态效果；检查材质、字色、尺寸、物理
   输入和 Esc，无重入错误。Regular 无背景蒙版与系统通知对照仍需单独验收。
4. 人为占用默认 Current App 热键后启动：状态菜单仍可进入 Launcher / 设置，
   不标注未生效的默认键；中文和英文设置页均显示错误与恢复方式。刷新设置、
   切分类、离开再进入后仍有诊断，不影响其他功能。
5. 保持配置不变，释放冲突后点击“重试快捷键”：提示消失，菜单出现实际键位，
   物理热键可唤醒并可输入 / Esc；无需重启。单独占用 Global 键重复该场景。
6. 改为 `⌃⌥⇧O` / `⌃⌥⇧P` 保存：新键有效，旧键注销，菜单同步。再次改成
   冲突键并保存：显示错误，原键仍可用，菜单继续显示实际回滚的原键；解除
   冲突后重试恢复。最后关闭测试实例释放键位。

ENV-010、HOT-001、原生 resize 和其余 Blocked / Pending 项均未因此标为物理
Pass。开发签名、公证、干净机安装和本报告其余未执行矩阵继续保持原状态。

## 隔离边界与未完成事项

- CUA 首次连接时触发 LaunchServices 再次启动，最初进程未保留
  `ANYWHERE_DATA_DIR`，并同时启动了一个旧 Flutter `Anywhere.app`。发现后立即
  终止两个由本轮启动的实例，改用 `open --env` 后核对环境变量与实际打开文件。
  此前进程未执行业务动作，但个人 `Application Support/Anywhere` 数据库访问时间
  发生变化；后续交互均使用隔离目录。隔离验证不能沿用首次进程作为 Pass。
- 后续 CUA 按显示名重新绑定又会拉起旧 Flutter 包。第一轮窗口截图、Tasks、
  Settings 与输入记录无法严格归因给 GPUI，已全部撤回；`settings-wrong-shortcuts.png`
  和 `tasks-empty.png` 不作为 GPUI 缺陷证据。
- 已创建 `Anywhere GPUI Acceptance.app` 唯一包装副本，Bundle ID 为
  `dev.anywhere.launcher.qa`，二进制来自同一验收包但重新签名；日志确认隔离数据
  可打开、测试热键可注册，因再次锁屏尚未取得其 UI 证据。
- 执行到第二次锁屏时，Quick Look、Shelf、Clipboard、Application History、
  Web / JSON、设置保存、权限、18 来源、VoiceOver、多屏 / 材质与性能矩阵均未完成。
- 默认快捷键冲突日志已记录；无冲突快捷键的 GPUI 界面、设置录制与旧键注销要在
  解锁后用唯一包装副本继续。
- 本文的 Failed / Blocked 项不等于全量缺陷清单；只有本次实际观察才登记。

## 2026-11-17 重构后重新打包与最终复验

本节只适用于下面的最终包。14:36 / 15:08 构建、274 / 254 项测试数字和此前的
交互结论都没有被复制为本节通过项。执行时间跨到 2026-11-18 00:06
（Asia/Shanghai）。

### 包与进程身份

| 项目 | Actual |
| --- | --- |
| 源码 HEAD | `b428899c9ce70c92e12de24950405222ea3b6c28` |
| 工作区 | dirty；`git status --short` 292 条，`git diff --stat` 为 249 files / +8550 / -25550 |
| App | `build/Anywhere.app` |
| 版本 / 构建号 | `0.14.0` / `1` |
| Bundle ID | `dev.anywhere.launcher` |
| 主程序 SHA-256 | `d720cf2e726f6a48e07eda107ff152ff617da10c3387f400448b2ffdf1837c53` |
| ZIP | `build/Anywhere-macos-arm64.zip` |
| ZIP SHA-256 | `3b7d82aa5e6d9a55a843d83afbe5b021df905123c8b01ba95cd60cc993b88bbf` |
| 架构 / 最低系统 | arm64 / macOS 15.0 |
| 签名 | `Anywhere Local Code Signing`；deep / strict 验签通过 |
| 运行进程 | PID `77192`；精确路径 `build/Anywhere.app/Contents/MacOS/Anywhere` |
| 隔离数据 | `build/verification-20261117-refactor-run2/data` |
| 隔离插件目录 | `build/verification-20261117-refactor-run2/bundled-plugins` |
| 测试热键 | `control+alt+shift+KeyO` / `control+alt+shift+KeyP` |

`ps eww` 只显示上表隔离目录环境；`lsappinfo` 的 Bundle ID 为
`dev.anywhere.launcher`。本轮没有把旧 Flutter 实例或其它同名包作为 GPUI 结果。

### 新发现并修复的输入保留缺陷

`ACC-2026-11-17-04`（P1）：Global Search 输入 `qq` 后隐藏，超过 15 秒再唤醒；
修复前输入框仍显示 `qq`，窗口为折叠高度且结果列表为空。日志显示 15 秒任务已
建立新的 Core Session，但旧实现只重置 `LauncherModel`，没有同步 GPUI
`InputState`，因此视口显示的是模型之外的旧文本。

修复在保留期清理任务中把模型查询同步到当前 Launcher 编辑器，并补强
`hidden_workspace_expires_without_another_activation`：同时设置模型和真实
输入框值，超时后断言两者都为空。`scripts/check-gpui.sh` 的 266 项测试全部通过，
包含该回归。

最终包实测：

- 隐藏 2 秒后唤醒：窗口恢复 `648×137`，AX 值仍为 `qq`，QQ 音乐结果仍在。
- 隐藏 17 秒后唤醒：窗口为 `648×72`，AX 值与可视文本均为空，没有残留结果。
- 根页 Esc：第一次清空查询并回到 `648×72`，第二次隐藏窗口。

证据：`build/verification-20261117-refactor-run2/evidence/query-retained-2s.png`、
`retention-expired-clean.png`。

### 搜索前后透明度

同一后台 Cursor 窗口、同一屏幕位置和顶部锚点 `y=312`，空态窗口为
`648×72`，输入 `qq` 后为 `648×137`。截取相同 `680×104` 屏幕区域，并在无文字、
无图标的搜索头内部取 `500×70` 样本：

| 样本 | 空态均值 | `qq` 均值 | Delta |
| --- | --- | --- | --- |
| 搜索头 | RGB 42.28 / 43.11 / 45.18 | RGB 40.28 / 41.15 / 43.38 | -2.00 / -1.96 / -1.80 |
| 窗外背景 | RGB 19.68 / 19.68 / 19.68 | RGB 19.64 / 19.64 / 19.64 | -0.04 / -0.04 / -0.04 |

搜索头最大单通道差为 6/255；窗外背景差为 1/255，说明背景本身基本不变。视觉
截图没有原问题的整块变暗、额外矩形蒙版或第二层模糊；剩余小差值属于系统 Regular
玻璃随窗口尺寸和采样区域的轻微自适应。结果是“无额外透明度层”通过，不应写成
与系统通知逐像素一致；测试仅为 Dark Liquid Glass。

证据：`empty-global-region.png`、`query-qq-region.png`。

### 设置与 Web 工具

- 最终包打开 Settings，七个分类均可达；Shortcuts 显示实际测试键
  `⌃⌥⇧O` / `⌃⌥⇧P`。
- 同一工作树的前一代包已完成录制 Esc、脏草稿关闭确认、保存并实际重注册热键；
  保存后新键可唤醒，旧键不再触发。最终包没有重复完整热键冲突 / 回滚矩阵。
- 重启最终包并加载同一隔离插件的 receipt 后，搜索 `json` 可打开 JSON Workspace。
  空正文没有白底块；输入合法对象后左右两栏均完整格式化，输入 31 字节非法文本时
  显示 `EndOfFileExpected · line 1, column 8`。
- 在同一重构链的前一代包上已实际打开 Clipboard History、Clipboard Shelf 和
  Application History；Shelf 为 `layer=3`，切到 Cursor 后保持，Esc 关闭。
  最终包没有重新遍历这些工具页，因此只记录为前代同工作树证据，不冒充最终包的
  全页面矩阵。

证据：`webtool-final-empty.png`、`webtool-final-valid-json.png`；
前代包为
`build/verification-20261117-refactor-run1/evidence/webtool-valid-json.png`。

### 最终门禁

- `scripts/check-gpui.sh`：266 passed / 0 failed；格式、本地化、资源、plist
  与严格 Clippy 通过。
- `scripts/check.sh`：exit 0；全工作区 Rust、严格 Clippy、生成物、SDK 52 项、
  JSON Workspace 31 项、文档 112 个 Markdown / 898 条本地链接、原生菜单 /
  剪贴板 / 应用启动探针和架构边界均通过。
- `scripts/build-macos.sh`、`scripts/package-macos.sh`、deep / strict 签名和
  `plutil -lint` 通过。
- 证据索引：`build/verification-20261117-refactor-run2/evidence/identity.txt`、
  `pixel-diff.txt`、`check-full.log`。

### 本轮结论

- Pass：最终包构建、签名、ZIP、精确进程身份、Global Search 空态 / 展开、
  Regular 玻璃无额外蒙版、2 秒保留输入、17 秒清空输入、Esc 清空 / 隐藏、
  Settings 入口、JSON Workspace 空态与合法 / 非法 JSON 显示。
- Partial：设置完整保存 / 安全关闭矩阵、Clipboard 工具、Application History、
  Running Tasks、文件搜索、Quick Look、IME、Reduce Transparency、浅色外观、
  Increase Contrast 和多屏。
- Unverified：Developer ID、公证、干净机安装、浏览器来源、全部 18 个应用历史
  来源和 VoiceOver。

