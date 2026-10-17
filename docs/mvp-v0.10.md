# V0.10 MVP：剪贴板历史与搜索

当前分支：`codex/dev-0.10`。开始日期：2026-10-16。延续 [路线图](roadmap.md)，先补齐内置搜索，再开发 Terminal。

## 分步实施

| 步骤 | 内容 | 验收 |
|---|---|---|
| C1 | SQLite 历史、多格式模型、去重和容量限制 | 旧 schema 迁移、原始格式恢复、检索、事务回滚、淘汰与删除 |
| C2 | clipboard-rs 平台读写 / 监听、采集与权限 | 默认关闭、暂停不补录、敏感格式与限额、变更中止 |
| C3 | 普通 Action、共享工具页、检索与固定预览 | 图片 / 文件 / 文字展示、键盘导航、过期响应与焦点 |
| C4 | 工程门禁、性能与真实界面验收 | 先代码和单元测试，再打包与实机测试；明确未验收边界 |

## 当前范围

2026-10-16 用户确认切换 `clipboard-rs` 并支持多格式，替代本日早期纯文本范围。分步记录见 [多格式改造](clipboard-multiformat-plan.md)。

- 纯文本、HTML、RTF、图片、文件列表，以及可读取为字节的自定义格式；一次复制的多个表示为一个历史条目，恢复保留格式。
- 默认关闭采集，开启前已有的内容和暂停期间的内容不补录。后台只在 macOS 系统始终允许读取时采集，不循环申请权限；敏感 / 隐藏 / 临时 / 自动生成标记跳过。
- 最多 1,000 条、128 MiB 内容及检索数据，每条最多 16 MiB、64 个格式，文件列表最多 4,096 个路径。总量超限在写入事务内淘汰最旧条目。额度不等于 SQLite 文件物理大小。
- 对格式名称与原始字节计算指纹，去重前再比较内容；相同数据更新复制顺序且保留 ID。不同格式的相同纯文本不错误合并。
- 检索大小写不敏感、各词包含匹配；列表只读摘要和类型。当前选择才读取正文 / 二进制；预览文字最多 64 KiB，图片缩略图最多 512 像素，解码分配限制 64 MiB。原始图片不常驻 Flutter，离开预览后清理其 ImageCache。
- 文件只保存路径引用，不备份正文。恢复前校验全部文件仍存在，缺失时拒绝部分恢复；不重放 Finder 剪切 / 移动元数据。可搜索路径，右侧预览完整路径列表。
- HTML / RTF 以纯文本或源码安全预览，不执行 HTML；无法预览的二进制显示类型 / 格式信息，仍可复制保存的数据。
- Return 复制，↑↓ 选择，⌘K Actions，Esc 返回，裸 Tab 不承担操作。Launcher 内历史页手动刷新，保留选择身份。15 秒唤醒填入规则与历史保存期限无关。
- 独立 Clipboard Shelf Action 打开置顶悬浮窗：不随焦点切换、复制或 Launcher 的 15 秒状态到期而关闭，显式关闭 / Esc 销毁窗口和独立 Flutter engine。复用历史存储、会话、组件及预览；两个页面的查询和选择独立。
- 悬浮窗可见时每秒串行刷新列表摘要，相同结果不重新解码图片、不通知重绘；新记录更新列表但保留选中身份和查询。最小化 / 隐藏 App 时停止刷新。Enter / 双击复制后，在窗口仍拥有焦点时返回上一个外部应用，不模拟粘贴，也不自动开启采集。
- 历史页仅提供搜索、预览、复制、单条删除和刷新；Settings → Clipboard 集中管理采集、清空历史和 15 秒自动填入。功能模块禁用仍在 Settings → Features；清空 / 暂停 / 禁用会撤销采集中数据。普通 Launcher 列表只展示 Clipboard History Action，不铺满历史内容。

## 架构与存储

```mermaid
flowchart LR
    OS[OS Clipboard] --> Library[Platform / clipboard-rs]
    Guard[macOS 权限与大小薄适配] --> Library
    Library --> Service[Rust HistoryService / epoch]
    Service --> Repo[HistoryRepository]
    Repo --> DB[SQLite 元数据 + 多格式 BLOB]
    DB --> Preview[Rust 有界文本 / 缩略图]
    Preview --> Bridge[类型化 Bridge]
    Bridge --> UI[Flutter 共享 ToolPage / 固定预览]
    UI --> Service
```

旧纯文本 schema v1 升级为 v2，事务中保留历史 ID、顺序与采集偏好。复制从数据库按 ID 重新读取原始表示，并复验功能资格。正文不进入日志、Context、Usage 或通用插件存储。数据库本地明文，macOS 0600，SQLite DELETE journal / secure_delete / 512 KiB 页缓存；不宣称加密或安全擦除系统备份。

clipboard-rs 0.3.5 固定版本，macOS 监听仍使用 changeCount 轮询，不承诺捕捉间隔内每次复制。原生仅保留权限、代数和分配前大小检查；NSPasteboard 读取延迟数据本身仍可能触发源应用分配，大小检查不能预知这一系统开销。

“所有格式”以可序列化剪贴板数据为边界：文件承诺、应用内部指针、源应用退出后无法兑现的延迟对象，以及同一种格式在多个 pasteboard item 的任意组合，不能通用持久恢复；不会把这些情况描述为完整保真。Windows App 尚未交付，本轮跨平台库接入不等于完成 Windows 实机支持。

## 验收记录

### 旧纯文本 Build 4

461 Rust / 255 Flutter 通过（6 项既有 Rust ignored）；真实入口、输入、Actions 取消恢复、采集、搜索、预览、去重、跨应用恢复与 App 重启已验证。经用户明确同意临时采集后已暂停，留下三条测试标记记录。首次系统授权 / 拒绝 / 只允许一次、整 App 长时间运行仍未实测。

旧纯文本存储性能基线见 [性能文档](performance.md#v010-剪贴板存储基线2026-10-16)，不能外推为当前多格式性能。

### 多格式 Build 5

已接入平台库、旧记录迁移、多格式 BLOB / 内容比较、容量限制、有界图片与文字预览、文件路径展示、完整格式恢复和依赖许可文件。专项测试覆盖旧数据迁移、格式顺序去重、不同表示不合并、文件引用、图片缩略图、自定义数据、指纹不匹配、隐私 / 大小预检及 UI 图片 / 文件切换。完整门禁通过 **472 Rust / 256 Flutter**（6 项既有 Rust ignored），原生、fmt、Clippy、架构、生成目录、文档及 analyze 均通过，日志为本机 `build/validation/v0.10-build5-check.log`。已打包签名 Build 5；旧三条测试记录迁移、保持暂停，以及通过新平台库恢复旧文本并跨应用粘贴均已实测。经用户确认临时开启采集，图片、多文件、RTF 与浏览器 HTML 的采集、预览和恢复均已实测：先用其他内容覆盖剪贴板，再从历史恢复；Preview 能从恢复的剪贴板新建图片，Finder 能粘贴两个文件且文件内容逐字节一致，TextEdit 粘贴 RTF / HTML 均保留粗体。结束后已暂停采集，记录只包含本次测试样本。新 schema 纯文本负载基线见 [性能文档](performance.md#多格式-schema-v2-基线build-5)。

### Build 6 验收修复

实机发现 AppKit 附加的纯文本兼容类型使迁移记录复制后出现重复条目，已在平台采集层统一为 UTF-8；HTML / RTF 与自定义数据不因此合并丢失。鼠标点选条目后显式恢复搜索焦点，底部复制 / 采集按钮也复用 Actions 的焦点恢复逻辑。

完整门禁 **473 Rust / 257 Flutter** 通过（6 项既有 Rust ignored），日志为本机 `build/validation/v0.10-build6-check.log`。专项回归覆盖旧文本兼容类型去重和鼠标选择后 Return 复制。Build 6 已签名打包并完成实机复验：鼠标选择后 Return 复制成功；经本轮验收授权临时开启采集，重复复制迁移文本两次，均更新原 32 字节条目且总数保持 8 条，没有新增兼容类型副本。再次暂停后完整退出并重启 Build 6，8 条旧记录、暂停设置、图片缩略图和两个完整文件路径仍正常。既有 Build 5 测试重复条目保留，没有擅自清空历史；最终采集为暂停。真实记录见本机 `build/validation/v0.10-build5-live.json` 与 `build/validation/v0.10-build6-live.json`。首次系统权限矩阵及整 App 长时间内存仍未实测。


### Build 7：独立剪贴板悬浮窗

新增普通 Action `core.clipboard.shelf`（Clipboard Shelf / 剪贴板悬浮窗），现有 `core.clipboard.history` 仍在 Launcher 搜索框下展示。两个入口共享同一 Rust HistoryService / 数据库，查询、选择和窗口生命周期独立。原生只负责窗口与焦点，Flutter 复用历史页及固定预览。悬浮窗重复触发复用窗口，可置顶、调整大小、最小化和显式关闭。复制反馈位于底部，不改变列表布局。

完整门禁 **473 Rust / 264 Flutter** 通过（6 项既有 Rust ignored）。最终 Flutter 再跑全套 264 项，复制反馈 / 路由最终专项 32 项通过，analyze 无问题；日志为本机 `build/validation/v0.10-build7-check.log`、`v0.10-build7-final-flutter.log`、`v0.10-build7-final-focused.log` 和 `v0.10-build7-final-analyze.log`。新增覆盖：两个 Action 的路由与禁用、独立窗口打开失败回退、未变化结果不重复解码和重绘、新记录保留选择、重复复制保留查询、复制反馈不挤动列表、最小化停止刷新、启动握手过期响应以及页面销毁。

实机已通过新 Action 打开悬浮窗，验证搜索、键盘选中和复制、切换外部应用仍保持显示、不同条目连续恢复并粘贴到 TextEdit、复制成功不挤动列表、Esc 显式关闭及重新打开。复用现有 8 条测试记录，全程未开启采集、未清空历史；真实新复制后的自动更新及最小化刷新通过单元测试，未再次开启系统采集实测。当前桌面工具不能读取辅助 Flutter engine 的控件树，因此使用键盘和截图完成以上验证；VoiceOver 兼容性及整 App 长时间内存仍未验收，不宣称已解决。


### Build 8：配置与使用页面分离

Settings → Clipboard 集中管理历史采集、清空历史、15 秒自动填入及对应采集状态 / 系统隐私设置入口。两个使用页面统一移除采集栏和配置菜单，仅保留搜索、固定预览、复制、单条删除和刷新；暂停状态只在底部显示文字。清空有单独确认，默认焦点为取消。功能模块总开关仍由 Settings → Features 管理。

设置使用独立 ClipboardPreferencesBackend 和轻量 ClipboardHistoryStatusDto，不查询历史列表。使用页后端已移除采集和清空接口。Settings 关闭等待采集变更和状态读回；失败时不把旧值显示为已保存状态。修复 TextEditingController 仅改变光标 / 选区时错误清空查询结果的问题。

完整门禁 **473 Rust / 269 Flutter** 通过，6 项既有 Rust ignored；原生、格式、Clippy、架构、文档及 analyze 均通过，日志为本机 `build/validation/v0.10-build8-check.log`。新增覆盖配置读回 / 失败、关闭等待写入、清空确认、使用页移除配置和光标变化保留结果；既有权限状态与系统设置跳转测试已迁至 Clipboard 分类。

Build 8 实机验证 Settings → Clipboard 显示采集开关、自动填入及清空；采集保持 off、自动填入保持 on，未更改用户设置。历史页移除采集栏，Actions 只包含 Copy / Delete Entry / Refresh，仍显示 8 条旧测试记录和固定预览。真实清空与开启采集未执行，逻辑由单元测试验证。Build 7 的 VoiceOver 与长时间整 App 内存待验收边界继续保留。


### Build 9：结构整理与重构方案核对（2026-10-17）

对照 [P01–P09 路线图](feature-module-roadmap.md)，本轮按职责整理剪贴板新增代码，保持 ActionId、页面 ID、数据库 schema / 路径、用户开关与快捷键不变。

| 重构目标 | 当前实现与直接证据 |
|---|---|
| 第一方静态模块，移除热加载 | `features/clipboard/registration.rs` 参与 Cargo 编译期目录；`features/catalog.rs` 与 `application/native_features.rs` 保持启动装配，未加入加载 / 重载生命周期 |
| 服务与数据适配分离 | `features/clipboard/history.rs` 定义 repository，`service.rs` 管理资格和用例，`content.rs` 生成预览；`persistence/clipboard.rs` 仅执行存储 / 查询，移除重复的 `detail` 预览入口 |
| Flutter 只管理交互和展示 | 页面 / 会话 / 预览 / 悬浮窗移至 `lib/features/clipboard/`；原始格式恢复、匹配、配额和图片生成留在 Rust |
| 领域局部注册，Launcher 保持通用 | Flutter 功能目录的 `registration.dart` 提交页面和窗口工厂，`tools/builtin_tool_pages.dart` 只组合；Controller 使用通用 `ToolPageRegistry`，没有新增剪贴板业务分支 |
| 类型化端口与配置分离 | `infrastructure/clipboard_history_backend.dart` 仅提供使用接口；`clipboard_preferences_backend.dart` 和 Settings 专门处理配置；不引入消息总线或 Any 状态容器 |
| 可取消资格、并发和内存有界 | 图片解码在存储锁外执行，服务在读取正文之前串行准入；完成后重新检查策略 epoch 与条目存在性；展示会话合并快速选择，正文临时预览先截断再分配 |
| 回归门禁与测试隔离 | `scripts/check-architecture.py` 新增 Feature 不导入 Application / persistence 的检查及规则测试；测试夹具移至 `test/support/clipboard_fixtures.dart`，测试入口不互相导入 |

修复后台搜索开始后用户改变选择却被旧选择覆盖的问题；新记录到达不重新解码当前未变正文。后台刷新临时失败保留可用结果和选择，后续成功会清除提示。删除 / 清空 / 禁用与解码并发时，丢弃失效预览。设置在通知 busy 之前登记 pending 操作，避免关闭回调漏等保存与读回。

新增回归覆盖：刷新期间切换选择、失败后的后台自动恢复、快速选择合并、页面销毁后的迟到结果、跨窗口共用预览准入、解码中删除 / 清空 / 禁用、Unicode / 非法 UTF-8 富文本截断、图片尺寸 / 文件引用预览，以及设置通知中的保存等待。存储基准示例也改为通过真实 HistoryService 获取预览。

完整门禁 **476 Rust / 273 Flutter** 通过（6 项既有 Rust ignored），原生、fmt、Clippy、架构、文档及 analyze 均通过。最终单复数文案修正后，Flutter 全量 273 项与 analyze 再次通过。日志在本机 `build/validation/v0.10-build9-check.log`、`v0.10-build9-final-flutter.log`、`v0.10-build9-final-analyze.log`。

实机验证普通 Action 路由、搜索输入、快速方向键选择后的最终预览、图片缩略图、独立悬浮窗的搜索 / 预览与 Esc 关闭，以及 Settings → Clipboard。保留 8 条既有测试记录，采集 off、自动填入 on；未开启采集、清空或修改设置。开发包为 `build/Anywhere-dev-0.10-build9.app`，签名与 ZIP 校验见本机构建结果。

**核对结论：当前新增功能符合已接受的模块拆分方案，无需重做整体架构。** 旧 `PluginDetail` / DTO 名称作为外部协议兼容仍保留，不代表恢复第一方插件 DSL；第三方 WebView 与 JSON 按路线图延期。辅助 Flutter 窗口的 VoiceOver、首次系统权限矩阵、Windows 实机与整 App 长时间内存仍未验收，本轮不将工程收尾等同于正式发行矩阵全部完成。


### 2026-10-18 提交前复核

`dev-0.9` 已由 `c797656` 完成模块迁移收尾，后续兼容性修复到 `9d38888`，并已在 GitHub。当前待提交工作实际为 V0.10 剪贴板历史及 Build 9 的结构整理；沿用既有版本号，不回写为 V0.9。

提交前重新执行 `scripts/check.sh`：476 Rust / 273 Flutter 通过，6 项既有 Rust ignored；原生、格式、Clippy、架构、文档和 Flutter analyze 均通过。新日志为本机 `build/validation/v0.10-closeout-2026-10-18.log`。沿用上节 Build 9 的签名包与真实界面验收，本日未重新运行权限 / 长时间内存 / VoiceOver 矩阵。

本次先提交当前版本，再从该提交创建独立的开源复用重构分支；编码预览、工具查找以及后续 SQLite / matcher 原型按 [开源库清单](research/open-source-reuse-audit.md) 分阶段实施。
