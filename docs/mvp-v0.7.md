# V0.7 MVP：系统文件搜索

后续引擎重设计见 [文件搜索引擎 V2](file-search-engine-v2.md)：Spotlight 快速候选 + Rust 完整遍历 + 会话临时记录与连续加载；以范围内不遗漏为验收前提。此前长期 SQLite 文件名索引提案已被取代，历史存储实验不代表新方案性能。V2 已接入当前工作区；下文标注历史 build 的记录不代表 V2 包或实机验收。

日期：2026-10-12。分支 `dev-0.7`，基线为已推送的 `dev-0.6@3b21483`。当前开发版本 `0.7.0+22` / Rust `0.7.0-dev.22`。File Search Action → 主 Launcher 的二级搜索模式，S1/S2/S3 历史记录见后文；完整实机 GUI / 系统隐私矩阵尚未验收，不表示 0.7 全部验收完成。

build 21 对 Flutter / Rust 分工、授权事务和大列表传输的收尾见 [重构验证记录](refactor-boundaries-build21.md)。

build 22 的稳定结果顺序、固定预览和窗口适配见 [文件工具更新](files-fixed-preview-build22.md)。

## 范围与进度

| 切片 | 范围 | 状态 |
|---|---|---|
| S1 搜索与打开 | Action 入口与专用页面、异步系统索引、统一 Candidate/Ranking、限量与取消、打开文件/目录及执行复验 | 已实现，验证记录见下方 |
| S2 可理解与可配置 | 明确搜索来源与加载/失败反馈、可关闭来源、检索目录与排除策略的设置，结果的常用次级动作 | 已实现，自动化通过；本地 fixture 的结果操作 / 预览实机验证完成 |
| S3 收尾 | 真实索引/未索引/隐私排除、多窗口与连续输入验收，性能与资源上限核对 | build 15 工程与本机开发包收尾完成；完整权限 / 卷 / 物理键盘矩阵保留，见收尾记录 |

当前文件搜索使用 Spotlight 提前返回候选，Rust 完整遍历决定覆盖与连续加载；无需引擎选择或外部 rg，不建立长期文件索引。当前检索文件名，不检索文件正文。Apple 的元数据查询能力及查询阶段见 [NSMetadataQuery](https://developer.apple.com/documentation/foundation/nsmetadataquery)；当前 Adapter 使用系统自带 `/usr/bin/mdfind`，调用格式依本机 `mdfind -h`。

Windows 保留同一 Platform 接口和不支持时的降级；Everything Adapter 尚未实现，不作为近期 macOS 门槛。剪贴板历史、浏览器历史和书签不在本轮范围。

## 当前使用与边界（文件搜索 V2）

1. 在 Global Search 输入“文件搜索” / `files` / `File Search` 找到普通 Action，也可设置现有 Action 快捷键。打开同一 Launcher 的二级页面，普通查询、Current App / Folder / Git 等 Scope 与内容页不触发文件扫描；不新增 Files Scope。
2. 非空文件名支持一个字符和字面标点；大小写与 Unicode 规范统一，多个词必须同时命中文件名。普通名称最多 256 UTF-8 字节，路径最多 4096，控制字符查询拒绝。显式路径浏览直接子项、匹配路径前缀或定位具体文件，不扫描全树。
3. Spotlight 在约 40ms 防抖后启动，完整遍历在约 150ms 后启动；两路独立。Spotlight 无命中、失败或迟到不阻止遍历，不决定“搜索完成”。
4. 完整遍历逐项处理允许根内的文件和目录，不读正文，不读取 gitignore，不硬编码排除 Library / node_modules / build / target / 应用包。默认范围 Home；设置最多 8 个根、32 个明确排除名，隐藏开关有效。新安装隐藏开关默认开，旧设置保留。
5. 类型先于保存、计数和加载过滤；64 是内部批次大小，不是总命中上限。目录队列与所有命中写入查询临时文件，追加流式读取，遍历内存有界；界面与执行表仅积累已加载候选。没有 2 秒或 50,000 条的完整遍历成功截断。
6. 操作列表后保留已展示行顺序，遍历更新状态/计数；滚动接近末尾或 ↓ 到已加载末项时自动追加内部批次，选中对象与滚动偏移保持。没有页码、翻页按钮或刷新操作。Ctrl Tab 切换类型，⌘K 打开文件操作，右侧预览固定展示并跟随选择，Esc 返回规则不变。搜索栏显示范围，底栏区分 Searching / Complete / Partial。
7. 根或目录读取失败、无法按 Unicode 规则匹配的名称、临时存储失败均有错误计数和最多 8 条例子；有结果也不能冒充 Complete。取消/设置变化/新 Session 撤销旧批次、游标和执行目标；输入改变或关闭后回收临时目录，下一次首次查询清理上次异常退出遗留目录。
8. 稳定且可读的允许范围内，遍历无错误结束后所有匹配项可通过连续列表取得。排除范围、无法读取、并发改名/删除和阻塞 OS 调用不在绝对保证内；执行仍复验目标。源级索引输出预算只影响首批，网络卷 OS 调用不保证硬实时取消。

## 执行、身份与历史

- 新 `CandidateKind::File` 表示结果对象，不假装是菜单或 Context 动作。Flutter 沿既有图标/结果行显示文件名、父目录；主要按钮显示 Open。
- 文件结果的执行目标复用 `core.file.open` / `core.directory.open` 和当前 ActionRegistry。结果不加入 Context Graph、Context action offers、Scope 或快捷键对象列表，避免把搜索发现的对象当成当前环境。
- 业务动作 ID + OS 原始路径指纹形成结果 ID；每个结果有独立 execution_context_id，避免二级动作串到别的文件。文件名/父目录/路径只用于当前结果和执行；排名诊断的 top_candidates 对文件使用通用 `file` 标签，不记录名称。
- Search 设置变更撤销缓存批次、结果操作和旧配置的查询发布；下一次输入使用新配置。新输入清除缓存批次；Registry 重载后结果重新取得当前动作定义及 Availability，不沿用已撤销 handler。执行仍验证 Session/Revision/目录版本，再复验路径存在、规范路径和文件/目录类型；失效时失败且不记录成功 Usage。
- 成功打开沿既有 Usage 记录业务动作与 Context 类型，不写具体路径。首版文件排序不提供逐文件 frecency；后续如有需求，先定义独立的历史保留和隐私契约。

## 架构

```text
File Search Action
  → ActionResult.view / HostView::FileSearch
  → Flutter 二级页面 / InputMode::FileSearch
  → AsyncCandidateSource（来源自定 accepts 条件）
  → 防抖、取消与有界批次
  ├→ Platform.search_files_with_options → macOS mdfind 提前候选
  └→ ScanStore → 完整遍历 / 临时记录 / 连续加载
  → Session 的本 revision 批次与覆盖状态
  → File Search 保留稳定行顺序，Registry 复验当前动作
  → 快照 publication 复验
  → 既有执行与 Usage
```

同步 CandidateSource 继续用于内存来源。异步协调、Context 完成状态和进程会话相互独立；文件检索不阻塞 global action 等待当前 Context，不进入 Running Tasks。平台接口承载查询、目录/排除设置、数量限制和取消回调，Core/Flutter 不依赖 Spotlight 或 AppKit。

## 验收要求

- 连续输入/改 Scope/关闭/新 Session 不出现旧结果，也不能执行旧结果。
- Launcher 提供 File Search Action；专用页面只排名文件，不混入应用/Context 动作。批次有界、所有命中可连续加载，结果不进入 Context 首页/Scope/快捷键目标。
- 中文、空格、换行、重复、失效路径、类型变化、索引失败与超时均有确定行为；删除文件后不打开其他目标。
- 搜索失败与索引不可用不阻止已有能力；设置提供明确来源状态和关闭入口。
- 真机核对默认打开、次级动作、系统隐私排除/未索引、并发和进程清理，记录耗时/资源上限。只读查询和组件测试不能替代完整 GUI 验收。

## S1 验证记录（历史实现，交互已由 build 4 替代）

Core 回归：异步批次、过期查询与关闭、Scope/内容/短查询门禁、去重/限量/名称匹配、打开文件与目录、业务 Usage、删除目标无副作用、来源失败保留 Context 动作。原生 Adapter 回归：NUL 路径、Unicode/换行、canonical 去重/过滤、取消和超时回收进程。Flutter 回归：结果显示、Open、Enter 传递精确候选 ID。

本机只读 `cargo run -p anywhere-platform --example platform -- search-files roadmap` 返回 1 条有效索引命中，规范绝对路径及存在性检查通过；没有打开文件。

完整 `scripts/check.sh`：302 项 Rust、135 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生菜单/来源/Host/材质、fmt、Clippy、架构和 Flutter analyze 通过。新增回归也覆盖 Registry 撤销后的缓存失效、类型变化/符号链接重定向、截断/过大/失败输出拒绝。上述为 S1 当时的验证记录，S2 进度见下节；没有启动新版进行 GUI 或性能采样。

独立开发包：`build/Anywhere-dev-0.7-build1.app`、`build/Anywhere-dev-0.7-build1-macos-arm64.zip` 及 `.sha256`。版本对齐，四个主二进制 arm64，Impeller 保持启用；deep/strict 固定签名、ZIP 内容一致性和 SHA-256 校验通过，签名 requirement 与 0.6 build 23 一致。旧包保留，未启动新版。

## S2 交互与验证（2026-10-09，历史实现）

- Settings → Search 可开关 File search；两个多行字段分别设置检索目录和额外排除目录名。点击 Save file search 应用；未保存字段与 Hidden apps 草稿共用关闭/保存/丢弃决策。设置保存后重启仍有效，旧版本配置缺少新字段时保留默认行为。
- 检索目录必须是绝对路径；应用不会自行遍历未索引目录。目录已移除、索引进程错误或超时显示 File search unavailable，并保留其他结果；配置内容无效时启动只关闭文件来源，Launcher 其他功能仍可使用。不会根据空结果推断文件不存在。
- 选择文件结果后点击 Actions 或 ⌘K（使用 Ctrl 的平台为 Ctrl+K）。文件提供 Open / Copy Path / Reveal in File Manager；目录提供 Open / Copy Path / Open Terminal Here。次级操作列表保留查询，Esc 返回原文件选择，下一次 Esc 按已有查询/Scope 规则处理。
- 次级操作通过通用 `result_actions` 协议传递，按 execution_context_id 绑定原目标；具体操作集合在 filesystem Feature 中定义，不进入首页、Context Graph、Scope 或全局快捷键目标。当前 Registry、Availability、catalog version、设置 generation 和路径类型复验覆盖它们，执行继续共用 Usage。
- 除单进程/全查询 2 秒上限外，最多查询 8 个目录，每次进程输出最多 256 KiB；累计最多 64 个规范有效命中，8 个显示结果。配置目录会约束规范化后的目标，符号链接不能把结果引出配置范围；额外排除只匹配目录，允许同名普通文件。
- 回归新增：Loading/空结果/失败状态；关闭来源与更改配置阻止旧发布、撤销旧结果操作；Copy/Reveal 精确目标与删除后无副作用；配置格式/数量限制；排除目录、同名文件和符号链接逃逸；Dart/原生设置持久化；Search 草稿关闭决策；Actions → Esc 返回原查询 → Enter 精确操作。
- 当前只读真实索引查询 `search-files roadmap <本项目目录>` 返回 1 个有效命中；没有打开文件。

本轮最终 `scripts/check.sh` 通过：307 项 Rust 测试、139 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生、fmt、Clippy、架构和 Flutter analyze 通过。独立 `build/Anywhere-dev-0.7-build2.app` / `build/Anywhere-dev-0.7-build2-macos-arm64.zip` 已生成：0.7.0+2 版本对齐、Impeller 保持启用、签名及 ZIP 内容一致性、SHA-256 校验通过，签名 requirement 与 build 1 一致。旧包保留，新包未启动。S3 的实际 GUI 打开、系统隐私排除、未索引目录、连续操作与资源采样仍待验收；Windows Everything、剪贴板历史、浏览器历史/书签尚未实现。

## S3 自动化收尾与只读实机检查（2026-10-09，build 3）

### 交互修复

- 文件来源不可用时，底栏提供 Retry；沿用原查询发起新 Revision，重试期间保留其他来源的结果。现有 Scope、Content 模式和执行流程不变。
- Search / Hidden apps 保存时捕获本次提交的字段值；Host 保存回执不会覆盖等待期间的新输入。保存并关闭发现新草稿时保持窗口打开，提示继续保存，避免静默丢失编辑。
- 启动时无法应用文件设置的降级告警采用独立状态保存；刷新设置不会清除告警，成功保存并应用文件设置后才解除。无效设置仅关闭文件来源。
- Search 设置使用共享文件夹图标；检索目录提示按平台显示 macOS、Windows 或其他系统的路径格式。

完整 `scripts/check.sh` 通过：310 项 Rust、143 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生菜单/来源/Host/材质、fmt、Clippy、架构检查及 Flutter analyze 通过。

### 查询与进程边界

- Core 使用模拟不可立即取消的阻塞 OS 调用，连续 21 次查询修订只触发 2 次索引调用；旧调用释放前没有并发启动新 worker，峰值并发为 1，中间查询被防抖取消。另有新 Session 拒收旧 Session 批次的回归。
- 原生取消/超时回归记录实际 fixture 子进程 PID，验证进程已退出且已回收。测试使用静态 shell fixture；生产查询仍使用固定 `mdfind` argv，不经过 shell。
- Release Adapter 只读探针：项目目录内每个查询执行 12 次，返回路径存在且命中数不超过 64，没有调用默认打开。

| 文件名查询 | 首次调用 | 后续 11 次中位数 | 后续 P95 | 每次有效命中 |
|---|---:|---:|---:|---:|
| roadmap | 86.97ms | 30.38ms | 45.47ms | 1 |
| readme | 30.34ms | 30.42ms | 42.16ms | 1 |

首次样本不代表系统冷缓存；这些耗时只包含 CLI Adapter 查询，不包含 150ms 防抖、Context 解析或 Flutter 渲染，不能据此宣称 GUI 帧率或整 App 内存改善。可复现入口：`cargo run --release -p anywhere-platform --example platform -- search-files-benchmark roadmap <绝对检索目录>`。

独立子进程采样观察到 12 个 `mdfind` 子进程，观测峰值为 1，CLI 退出后这些 PID 均不再存在。另在 `/tmp` 临时目录新建唯一文件，确认文件存在但系统查询返回 0；该检查没有遍历目录作为替代，也没有修改 Spotlight 配置。它不等于真实系统隐私排除验收。

资源边界仍为单 worker、OS 查询总预算 2 秒、异步等待上限 3 秒、最多 64 个有效命中和 8 条显示结果。上述检查未启动新版 GUI、未打开用户文件、未变更系统索引/权限/隐私设置。

### 尚待实机验收

- 默认应用打开文件/目录；Actions / ⌘K 的 Copy / Reveal / Terminal 和失效目标反馈。
- 连续输入、切换 Scope、Content / Esc 返回、关闭后再次唤醒，不出现旧结果；设置保存过程中继续编辑和关闭的实际体验。
- 系统隐私排除目录、既有未索引目录、多个检索目录与多窗口操作。
- 明暗主题、真实背景下的玻璃可读性、连续交互流畅度，以及完整 Flutter App 的内存/帧率采样。

上述 GUI / 系统矩阵完成前，不将 V0.7 标为发布验收完成。Windows Everything、剪贴板历史、浏览器历史/书签仍未实现。

独立开发包 `build/Anywhere-dev-0.7-build3.app` / `build/Anywhere-dev-0.7-build3-macos-arm64.zip` 已生成：0.7.0+3 版本对齐，4 个主二进制 arm64，Impeller 启用；deep/strict 签名、ZIP 内容一致性及 SHA-256 校验通过，签名 requirement 与 build 2 一致。旧包保留，新包未启动。


## File Search 专用页面（2026-10-09，build 4）

- `core.files.search` 是注册到 ActionRegistry 的真实 Action，复用执行和 Usage。返回类型化 `HostView::FileSearch`，不通过输出文字或 Action 名称猜测导航。入口有中英文搜索别名；Host Action 也保留在当前执行表中，支持已有快捷键解析，与查询和 Scope 独立。
- 全局入口的内部 `core.launcher` capability 只用于执行，不加入 Context Graph / Context summary / Scope。进入 FileSearch Revision 不解析选中文本、剪贴板或环境 Context，也不运行应用搜索和其他异步来源；返回以新 Revision 重建原页面，取消旧查询。
- 页面复用既有玻璃背景、图标、透明按钮和选中效果。类型过滤只作用于本批最多 64 个命中；列表单击选择便于预览，双击 / Enter / Open 执行，Actions / ⌘K 复用原目标的常用操作。顶栏返回按钮 / Esc 返回父页。文件来源关闭或启动降级时提供说明；平台未注册来源时明确报告不可用。
- Preview 默认关闭，开启后按当前选择防抖 120ms。显示规范路径、文件大小、修改时间；文件夹仅显示信息，不枚举目录。普通 UTF-8 文本最多读取 64 KiB，大文本明确提示截断，二进制/图片/PDF 等只显示信息和无文本预览提示，不调用 Quick Look、WebView 或外部程序。
- 预览通过 Session / Revision / 当前文件 Candidate ID 获取目标，不接受 UI 传入任意路径；复验路径和文件类型，并在读前/读后检查查询、配置 generation 和 Action catalog。预览不执行动作、不记 Usage、不持久化内容、不进入 Context Graph。独立 worker 最多并发 1；等待 permit 和异步读取分别有 2 秒等待上限，异常 OS 调用仍可能继续阻塞 worker，permit 在实际 worker 结束时才释放。UI 忽略旧选择/关闭页面后的迟到响应，关闭页面释放预览状态。
- 新增自动化覆盖：真实入口执行/Usage/跨 Scope 快捷键；专用模式与普通查询隔离；不支持平台反馈；64 命中限量；文本截断/二进制/删除文件；过期或伪造预览目标；页面过滤、选择/打开、Actions、Retry、Esc 恢复父查询与迟到预览隔离。
- GUI 的真实玻璃效果、打开行为、系统隐私矩阵和整 App 性能仍由实机验收确认。本改动不增加隐藏文件扫描，也不改变现有默认排除。

最终 `scripts/check.sh` 通过：314 项 Rust、144 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查与 Flutter analyze 通过。已生成独立 `build/Anywhere-dev-0.7-build4.app` / `build/Anywhere-dev-0.7-build4-macos-arm64.zip`：0.7.0+4 版本对齐、4 个主二进制 arm64、Impeller 启用，固定签名与 build 3 一致，deep/strict、ZIP 内容与 SHA-256 校验通过。旧包保留，新包未启动。


## File Search 主界面扩展（2026-10-09，build 5）

- 移除独立 `FileSearchPage` 布局。所有搜索模式共用 `LauncherSearchArea`、结果列表、`CandidateRow`、提示栏和 `LauncherFooter`；背景、搜索框字号、行高与间距沿用主 Launcher。
- All / Files / Folders 复用 Scope 标签的 `LauncherFilterButton` 外观，在原标签栏位置显示；它们仍仅筛选文件结果，不新增或更改 Scope。
- 默认按主界面的结果行数计算窗口高度；预览开启时在结果区域右侧扩展信息栏，关闭后恢复紧凑高度。异步预览完成不重复调整同一高度。
- 保留单击选择、双击 / Enter / Open、Actions、Retry、Esc 和父查询恢复。新增自动化验证共享搜索栏 / 底栏、无额外工具栏、动态高度、点击返回以及选择不执行。

验证：`scripts/check.sh` 通过，314 项 Rust / 145 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查和 Flutter analyze 通过。独立 `build/Anywhere-dev-0.7-build5.app` / `build/Anywhere-dev-0.7-build5-macos-arm64.zip` 已生成，0.7.0+5 版本对齐、4 个主二进制 arm64、Impeller 启用，固定签名 requirement 与 build 4 一致，deep/strict、ZIP 内容一致性及 SHA-256 校验通过。旧包保留，新包未启动，实际 GUI 视觉与交互尚待用户确认。本轮未提交或推送。


## 文件搜索输入连接修复（2026-10-09，build 6）

- build 5 在普通搜索与文件搜索间改变 TextField 的 Key，重建 EditableText 时，旧组件释放共享 FocusNode 上的输入连接；即使 FocusNode 仍有焦点，新组件也可能没有有效的 TextInput client。自动化已复现进入后 `hasAnyClients == false`。
- 所有 Launcher 搜索模式采用固定 `launcher-search-input` Key，保留同一 EditableText 和输入连接，避免页面切换销毁编辑状态。
- 回归测试通过原有 TextInput client 发送输入，不使用会主动聚焦的 `tester.enterText`：覆盖键盘进入、直接输入、Esc 返回后继续输入、鼠标再次进入以及中文 IME 组合/提交。

验证：`scripts/check.sh` 通过，314 项 Rust / 146 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查和 Flutter analyze 通过。已生成独立 build 6 App / ZIP，版本 0.7.0+6、Impeller、4 个 arm64 主二进制、固定签名（与 build 5 一致）、deep/strict、ZIP 内容一致性及 SHA-256 校验通过。保留旧包，新包未启动实机验证；本轮未提交或推送。


## Launcher UI 与交互重设计（2026-10-09，build 7）

调研与完整行为契约见 [Launcher UI 与交互设计](launcher-ui-v0.7.md)。主 Launcher 和文件搜索沿用同一布局，调整搜索字体、结果层次、透明选中态、模式提示、筛选图标、空状态与底栏；预览提供名称、类型、易读大小、修改时间、完整位置和关闭按钮，展开结果区域至少 340 logical px。

候选统一单击选中，双击 / Enter / 主按钮执行；菜单导航仍单击进入。文件模式新增 ⌘P / Ctrl P 开关预览和 Ctrl Tab / Ctrl Shift Tab 切换类型过滤。用户自定义 Action 热键优先。保留 Scope、Context、15 秒剪贴板和 Esc 契约。测试覆盖 pointer down 选中后立即 Enter 的精确目标、输入连接 / IME、预览键与关闭、过滤循环及大字号。可选离屏渲染只检查布局与字体，不模拟原生玻璃或替代实机验证。

最终验证：`scripts/check.sh` 通过，314 项 Rust / 148 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查、Flutter analyze 和对比度检查通过。4 张离屏 UI 图已生成并复核。独立 `build/Anywhere-dev-0.7-build7.app` / `build/Anywhere-dev-0.7-build7-macos-arm64.zip` 已生成，0.7.0+7 版本、Impeller、4 个 arm64 主二进制、固定签名（与 build 6 一致）、deep/strict、ZIP 内容一致性和 SHA-256 校验通过。保留旧包，未启动实机 GUI，未提交或推送。


## 2026-10-09：入口与快捷键统一（build 8）

移除空白 Global Search 常驻 File Search 行。File Search 保持普通注册 Action，可输入 `files` / `文件搜索` 找到，已有 Action 快捷键设置支持用户自定义 local / global 打开。搜索图标的紧凑菜单切换 Global Search / Current App，保留查询，不增加 Files 根入口或 Scope。Scope / Context 与 15 秒捕获内容逻辑保留。

文件页减少为一个类型下拉与预览图标；操作选择 / 输出时隐藏无效控制。移除文件页默认 ⌘P / Ctrl P 以及 Ctrl Tab 过滤映射，底栏收起常驻方向键说明。文件页 Esc 先清空查询，再回父页；返回按钮直接回父页。全局 Action 快捷键直接进入工具时，回到空白 Global Search。详见 [统一交互模型](launcher-ui-v0.7.md)。

Core 将可见查询候选与独立 `binding_offers()` 分开，隐藏入口仍保留快捷键目标和 Usage。新增 / 调整自动化覆盖真实搜索入口、快捷键目录、空首页、用户按键进入、local / global 返回、根菜单切换、连续 Esc 和中文输入连接。

最终 `scripts/check.sh` 通过：314 项 Rust、153 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查、Flutter analyze 和对比度检查通过。文件 UI 的 11 项测试在离屏模式再次通过，5 张布局图已生成并复核。

已生成独立 `build/Anywhere-dev-0.7-build8.app` / `build/Anywhere-dev-0.7-build8-macos-arm64.zip`：版本 0.7.0+8、4 个 arm64 主二进制、Impeller、固定签名（与 build 7 相同）、deep/strict、ZIP 内容一致性与 SHA-256 校验通过。旧包保留，未启动实机 GUI，未提交或推送。

## 类型菜单与纯键盘操作（2026-10-09，build 9）

- 修复类型菜单透底与默认大行高：共享紧凑菜单项，32 px 行高、勾选标识、深色不透明浮层、细边框和轻阴影；Material 3 使用明确的 labelTextStyle 与系统字体。根入口菜单使用同一组件。主窗口玻璃背景不变。
- 新增 Documents、PDF、Images、Audio、Video、Archives、Code，保留 All types / Files / Folders。分类集中在 FileFilter，依据候选文件名后缀且不区分大小写；目录不会因名称后缀被当成文件。Code 支持 Dockerfile / Makefile / 常见点文件等。只过滤当前最多 64 条命中，不额外扫描、读取 MIME 或扩大索引范围；无匹配时提示切回 All types 或修改查询。
- Tab / Shift Tab 遍历实际按钮，结果列表不占用 Tab 停靠点；Enter 激活焦点控件，类型菜单内方向键选择并可滚动。菜单 Esc 仅关闭菜单并回到输入；工具栏 Esc 先恢复输入焦点，不清查询。输入框中的方向键、Enter、⌘K、Scope 快捷键和既有 Esc 契约保留，用户自定义 Action 热键仍优先。
- 修复父级按键处理抢走按钮 Enter / 方向键的问题，避免错误执行文件；预览开关 / 关闭、底栏 Actions / 执行 / Retry、返回均恢复输入焦点，使后续列表操作可连续完成。

验证：`scripts/check.sh` 通过，314 项 Rust / 158 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查、Flutter analyze 和对比度检查通过。补充纯键盘过滤 / 菜单取消 / 预览与关闭 / Actions / 返回、短窗口菜单滚动、大字号、后缀分类与目录区分覆盖。6 张离屏 UI 图输出到 `build/ui-v0.7-build9`，类型菜单与列表视觉已复核；离屏图不模拟系统原生玻璃。

已生成独立 `build/Anywhere-dev-0.7-build9.app` / `build/Anywhere-dev-0.7-build9-macos-arm64.zip`：版本 0.7.0+9、4 个 arm64 主二进制、Impeller、固定签名（与 build 8 相同）、deep/strict、ZIP 内容一致性与 SHA-256 校验通过。旧包保留，新包未启动实机 GUI，未提交或推送。S3 系统隐私与真实多窗口矩阵仍待验收。

## 类型标签与二级预览（2026-10-09，build 10）

本节替代 build 9 的文件类型弹出菜单、Tab 控件遍历和常驻预览开关。

- 文件类型与 Scope 共用 `LauncherFilterBar` / `LauncherFilterButton`：统一透明标签、选中效果、间距；保留十个类型。Ctrl Tab 正向、Ctrl Shift Tab 反向循环，保留查询且不重复检索；所选标签自动滚动可见。操作选择 / 输出层不切换类型，Context / Scope 数据模型保持原有契约。
- 统一窗口 builder 消费裸 Tab / Shift Tab，取消 Launcher、Settings、Running Tasks 和弹窗的默认焦点遍历。快捷键录入、组合键映射与用户自定义 Action 热键优先级保留。文件页无 Tab 键帽或 Tab 操作提示。
- Preview 移到所选文件的二级 Actions，通过 ⌘K / Ctrl K 打开；方向键选择，Return / 底栏 Preview 或双击执行。即使没有其他文件操作，也可进入 Preview。预览是显式 Host presentation entry，不伪造 Core Candidate、不通过执行接口写 Usage，仍调用 Core 的 Session / Revision / Candidate 校验与有上限的预览读取接口。
- 预览仍复用内联详情区，Esc / 返回先关闭该层并保留查询、类型和精确文件选择；再次 Esc 才清查询，再次空查询 Esc 返回父页。原文件次级动作继续通过真实 Core offers 执行；主窗口玻璃背景和捕获内容的 15 秒契约保留。

验证：`scripts/check.sh` 通过，314 项 Rust / 160 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查、Flutter analyze 和对比度检查通过。覆盖类型正反循环 / 回绕 / 自动滚动、裸 Tab 在输入和弹窗中不遍历、二级 Preview / 真实文件动作、仅有预览的目录、Esc / 返回精确恢复目标、大字号 / IME / 迟到请求与既有 Scope 回归。7 张离屏图在 `build/ui-v0.7-build10`，已复核标签栏和二级 Actions 视觉。

独立 `build/Anywhere-dev-0.7-build10.app` / `build/Anywhere-dev-0.7-build10-macos-arm64.zip` 已生成：0.7.0+10、4 个 arm64 主二进制、Impeller、固定签名（与 build 9 相同）、deep/strict、ZIP 内容一致性及 SHA-256 校验通过。旧包保留，未启动实机 GUI，未提交或推送。完整 S3 实机验收仍待完成。


## 搜索入口与跨帧输入修复（2026-10-10，build 11）

- File Search 去除 `find files` 隐藏别名及 `file` / `folder` / `spotlight` keywords，保留标题与 `files` / `文件搜索` / `搜索文件` 显式别名。新增 Core 测试覆盖命中和不命中查询、无关附加关键词，并验证不可见时快捷键 Offer 仍可解析。此修改收紧入口匹配，不改变通用搜索匹配器。
- 新查询等待期间隐藏上一输入的候选；当前快照必须与当前 input 一致才展示。补测入口出现后改为不匹配查询，异步结果未返回时旧入口已经消失。
- 用户本机已有日志中，File Search 的执行和空查询均已完成；该日志不支持文件检索 worker 卡死的判断。跨帧 Widget 测试复现：执行先渲染加载图标，Host 导航又同时替换返回控件与右侧提示，导致未固定身份的输入父容器被销毁，即使 TextField 自身 Key 固定仍重建 EditableText。现在固定输入父容器 Key，并在输入模式变更布局完成后重新连接焦点，附带代次 / 可见性 / 模式 guards。
- 补测真实延迟执行的 Enter、双击、主按钮以及额外的焦点丢失情形，断言同一 EditableTextState 和有效 TextInput client；通过原连接继续输入并验证 Esc 返回。立即返回的旧 fake backend 无法覆盖这个跨帧缺陷。

验证：`scripts/check.sh` 通过，315 项 Rust / 165 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生检查、fmt、Clippy、架构检查、Flutter analyze 和对比度检查通过。21 项文件页测试也在离屏模式通过，7 张布局图输出到 `build/ui-v0.7-build11`，已复核输入光标与文件列表布局。旧测试的输入数据已对齐实际查询，保留乱序响应、索引重发布与图标失败重试断言。

独立 `build/Anywhere-dev-0.7-build11.app` / `build/Anywhere-dev-0.7-build11-macos-arm64.zip` 已生成：0.7.0+11、4 个 arm64 主二进制、Impeller、固定签名 requirement 与 build 10 相同，deep/strict、ZIP 内容一致性与 SHA-256 校验通过。保留旧包，未启动新包进行实机 GUI 验证，未提交或推送。

## 唤醒输入与原生响应者修复（2026-10-10，build 12）

用户在运行 build 11 时再次报告 Current App 唤醒后不能输入。已读取实际运行路径和既有日志；窗口显示和 Context 快照均正常完成，后续没有输入查询记录。新增隐藏 AppKit 窗口测试复现了 Host 原调用的缺陷：即使外层 NSView 的 acceptsFirstResponder 为 false，makeFirstResponder(wrapper) 仍替换掉已工作的编辑视图。当前 Flutter SDK 的 FlutterViewController.view 是 FlutterViewWrapper；macOS 文本插件通过 TextInput.show 把自身设为窗口第一响应者。因此 build 11 的 Widget 焦点断言无法独立证明原生响应者正常。

- Launcher、Settings、Tasks 统一使用 WindowInputFocus：保留内容中的有效键盘接收者；缺失时选择接受键盘的内容子视图，不强制设为外层 wrapper。实现属于 macOS Host，Context / Action / Scope 不变。
- 移除模式切换时主动 unfocus 的做法。使用 EditableTextState.requestKeyboard 在布局完成后确认连接；已经有逻辑焦点时也会重新发送 TextInput.show，不以 FocusNode.hasFocus 替代文本接收者确认，不清除 IME 组合状态。
- Controller 的 windowPresentation 只在当前可见 Launcher 的 Host show 回复后递增。Widget 在显示完成后再次请求输入连接，覆盖字段先获得焦点、Host 后显示的顺序；关闭后迟到的 show 回复不递增或恢复输入。普通查询不触发焦点重置。
- 原生检查新增 wrapper 覆盖的复现、有效编辑视图保留、连续隐藏 / 恢复及窗口迁移。Flutter 新增 Current App / Global Search 各三次连续唤醒、延迟 Host show、无需点击直接英文输入、中文组合与提交、关闭后迟到回复；保留文件搜索跨帧执行、输入与返回测试。

验证：`scripts/check.sh` 通过，315 项 Rust / 167 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生响应者 / 材质 / Host / 菜单检查、fmt、Clippy、架构检查和 Flutter analyze 均通过。55 项相关 Widget 回归包含唤醒、文件页、Settings 与快捷键；设置页移除 Launcher 输入时，取消待执行的输入恢复，避免访问已停用的 Focus context。

独立 `build/Anywhere-dev-0.7-build12.app` / `build/Anywhere-dev-0.7-build12-macos-arm64.zip` 已生成：版本 0.7.0+12、4 个 arm64 主二进制、Impeller、固定签名 requirement 与 build 11 相同，deep/strict、ZIP 内容一致性及 SHA-256 校验通过。原生回归只使用隐藏 AppKit 测试窗口，没有启动新 App 进行实机 GUI 验收。保留正在运行的 build 11 和旧包，未提交或推送。


## ripgrep 文件名来源（2026-10-10，build 13 历史实现，已由 V2 替代）

- Settings → Search 可选择 System index、ripgrep、System index + ripgrep，并开启 Include hidden files。旧设置保持系统索引、不包含隐藏路径；引擎和隐藏开关随既有设置持久化，不丢失尚未保存的目录草稿。
- ripgrep 需要已安装的 `rg`，沿既有工具查找服务从 PATH、Homebrew 等标准目录定位；发布包不内置 rg。本机已找到 `/opt/homebrew/bin/rg`。缺失时单独的 ripgrep 来源不可用，合并模式继续使用可用的系统索引；两者都失败才报告来源不可用。
- 通过固定 argv 的 `rg --files --threads 1 --null --no-config --no-ignore --no-messages` 枚举指定目录；开启隐藏开关时增加 `--hidden`。只按文件名匹配，大小写不敏感，多个空白分隔关键词均需命中；用户查询不作为正则或 shell 指令。忽略 `.gitignore` 等规则，仍跳过 Library、node_modules、target、build、应用包与用户额外排除的目录。排除名称按字面值转义。
- ripgrep 只提供文件；目录仍由系统索引提供，单独使用 ripgrep 时 Folders 类别没有目录结果。隐藏开关允许隐藏路径，但无法保证 Spotlight 已索引隐藏对象；未索引文件应选 ripgrep 或合并模式。
- 扫描默认使用 Home，推荐设置明确的小范围目录。复用最多 8 个目录、64 条结果、16 KiB 单路径、单查询 worker、防抖及取消。单独扫描期限 2 秒；合并模式先查系统索引 700ms，再扫描 1600ms；扫描约 8 MiB 输出或 50,000 个条目后停止。流式读取只保留当前路径、有限命中及去重集合；不缓存全量文件清单。同步文件系统检查仍不承诺网络盘硬实时。
- 达到扫描预算时返回已有部分命中；没有命中则报告不可用。因此空结果/当前批次不能代表全目录都没有目标。结果类型过滤仍只处理当前批次。合并按规范路径去重并交错两个来源，避免索引批次把未索引文件全部挤掉；所有结果仍进入原 Candidate/Ranking/执行复验。
- 引擎或隐藏开关变更推进已有配置 generation，撤销旧结果和执行 Offer，阻止旧查询回写。取消、超时、达到结果数量或错误时终止并回收扫描进程；退出后先排空最后的管道字节。没有新增 Scope、默认快捷键、全局首页控件或 Running Task。

验证：真实 rg 已验证未索引临时文件、隐藏文件/目录、`.gitignore` 忽略文件及 node_modules 排除。自动化覆盖 NUL 路径、字面查询/排除、规范路径去重与越界符号链接拒绝、失败/不完整输出、取消/超时/数量限制进程回收、合并降级、引擎/隐藏设置改变撤销旧动作及设置往返兼容。`scripts/check.sh` 通过：320 个 Rust 测试、168 个 Flutter 测试，原生菜单/Host、fmt、clippy、架构检查及 analyze 通过；5 个环境依赖测试默认跳过，其中真实 rg 用例另行执行并通过。三引擎 Adapter 的 CLI 对比：临时目录里系统索引 0 条、ripgrep 2 条、合并 2 条（含隐藏文件），没有打开文件。

独立 `build/Anywhere-dev-0.7-build13.app` / `build/Anywhere-dev-0.7-build13-macos-arm64.zip` 已生成：版本 0.7.0+13、4 个 arm64 主二进制、Impeller、固定签名 requirement 与 build 12 相同；deep/strict、ZIP 内容一致性及 SHA-256 校验通过。未启动新 App 做实机 GUI 验收，未提交或推送。


## 文件搜索 V2 收尾（2026-10-10，当前工作区）

- 已替换 Launcher 的生产文件搜索链路：Spotlight 独立提前返回，Rust 同时完整遍历文件/文件夹，所有命中在查询临时文件中按批读取；无长期索引、rg 依赖或成功截断预算。引擎选择从设置移除，旧字段/诊断 CLI 兼容保留。
- 文件类型过滤下推查询；名称统一 Unicode 匹配，支持单字符、字面标点和显式路径。根/隐藏/排除是唯一明确范围条件，读取或存储错误有 Partial，不用索引结果数判断覆盖。
- 每页 64 项；方向键/滚动边界与底栏翻页，操作后冻结行顺序，刷新提示和范围说明复用 Launcher 组件。进度更新不将鼠标用户滚回所选行。原 Scope、快捷键与 Preview / Esc 契约保持。
- 流式 Source 保持旧来源的一次返回兼容；blocking worker 有界，Session/Revision/settings guards 隔离旧发布；去重追加原子提交。文件结果的次级动作使用独立 execution_context_id，原始路径身份避免显示转换碰撞，链接目标变化拒绝执行。

验证：`scripts/check.sh` 的原生菜单/Host/材质/输入焦点、fmt、334 个 Rust 测试、Clippy、架构检查通过；最终 UI 细化后 `flutter analyze` 与全部 171 个 Flutter 测试通过。默认跳过 6 项（5 项既有环境依赖 + 大目录 fixture）；50,101 条目大目录 fixture 已另行执行并通过。非 Unicode 身份断言通过；本机 APFS 拒绝创建非法字节名称，因此相应实际目录遍历 fixture 未在本机覆盖。

release 只读遍历/分页采样见 [V2 设计与证据](file-search-engine-v2.md) 与 `docs/benchmarks/file-search-v2-2026-10-10.json`；三组新建目录各 3 次，全部命中分页数量与期望一致。该数据不含 mdfind、Flutter、Candidate 装配或可见 GUI，不代表冷缓存/Home/网络卷表现。

本轮未启动新 App 做 GUI / 权限矩阵验收，也未生成、提交或推送新 App 包。现有 build 13 包仍是上节的历史 rg 实现；工作区中的 V2 需要后续重新打包。

### 2026-10-11 连续列表修订

- 移除页码、翻页按钮、刷新操作与用户分页；内部 64 条批次按滚动/方向键需求追加到同一列表。
- 保留已展示顺序、滚动偏移和选中对象；Rust 同时保留旧对象及次级动作的执行资格。
- 最新记录按未展示原始路径读取，覆盖扫描过程中晚发现且排名更高的匹配项。遍历器仍不将全部命中装入内存；已浏览候选与执行表的内存随加载数量增长，旧独立遍历器 RSS 不代表连续列表 App RSS。
- 验证：335 项 Rust 测试通过（6 项 ignored 未在本轮重跑），173 项 Flutter 测试通过；Rust fmt / clippy、架构检查、原生菜单/Host/窗口焦点检查与 Flutter analyze 通过。首次完整 gate 的 Flutter lint 两处花括号提示已修复，随后 Flutter analyze / 全量 Flutter test 重跑通过。
- 滚动回归额外修复 pointer-down 选中后排队的自动定位，避免拖动列表被拉回原处。
- 新 App 打包和可见实机交互未执行。

### 2026-10-11 build 14 打包与实机验证

已构建并启动 `Anywhere-dev-0.7-build14.app`，生成 arm64 ZIP 和 SHA-256。固定签名与 build 13 一致，deep / strict 及 ZIP 内容检查通过。保持 Impeller 和现有背景。

实机验证了唤醒输入、Global 的普通 File Search Action 入口、183 项连续列表滚动到底、方向键跨 64 项边界、隐藏文件、PDF / Folders 过滤、TXT 预览 / Esc、Copy Path 执行和 TextEdit 打开文件。修复文件工具页继承环境菜单超时提示的问题并重新打包。最终 `scripts/check.sh` 通过：335 项 Rust / 174 项 Flutter，6 项 ignored。

组合键的桌面注入没有形成可信的物理键盘验收；Home 全量 / 权限 / 网络卷等仍待验证。具体范围和结果见 [build 14 验证记录](validation-v0.7-build14.md)，不宣称整机所有场景已完成验收。

### 2026-10-12 build 15 工程收尾

功能、文档与开发包核对完成；统一当前连续列表说明，补充临时记录租约 / 清理回归。最终 337 项 Rust / 174 项 Flutter 通过，50,101 项大目录用例另行通过；真实 Carbon 注册与冲突回滚检查通过。最终 App 已启动验证唤醒输入、183 项列表跨批加载到底、隐藏文件操作 / 预览、140 / 3 项过滤、无匹配旧候选清除、分层 Esc 和再次激活输入。

App / ZIP / SHA-256、具体实机范围、未覆盖矩阵和提交状态见 [V0.7 工程收尾](v0.7-closeout.md)。搜索库选型暂不改动生产引擎。

### 2026-10-12 build 16 文件类型图标

列表与预览统一使用文件类型线性图标，按 basename / 目录标记分类，不新增文件读取或原生图标依赖。实机发现二级文件动作的 kind 同为 File，已显式保留操作图标并补回归。Flutter analyze 与 176 项测试通过，最终图标 / 文件页 29 项回归通过；16 类离屏布局与本机文件列表已复核。原快捷键与文本预览能力保持，富预览建议未实现。调研来源、图标规则和验证见 [文件预览与图标](file-preview-and-icons-v0.7.md)。

### 2026-10-12：build 17 右侧预览交互收尾

- 二级 Actions → Preview 继续复用主 Launcher，↑ / ↓ 与鼠标选择驱动右侧当前对象预览，120 ms 防抖与迟到请求隔离。
- 修复 Actions 短列表导致原文件列表偏移丢失；按激活 / Revision / 查询 / 类型保存位置，返回时复核后恢复。
- 关闭预览保留查询、选区、文件选择、类型与列表位置；预览文字有焦点时 Esc 一次关闭并回到输入。新文件预览从内容顶部开始。
- Flutter analyze 与全部 178 项 Flutter 测试通过，新增长列表 / 内容滚动 / 焦点 / 迟到读取回归；实机 build 17 验证 48 文件目录，选择 preview-12 → preview-14 跟随更新、Esc / 按钮关闭与继续输入通过。
- build 17 App / ZIP / SHA-256 已生成，arm64、Impeller、签名与包内容一致性通过。Rust / 原生功能代码未改，本轮未重复 Rust 功能测试；复杂格式预览未加入，未提交推送。

### 2026-10-12：build 18，模式选择裁切与浅色背景

- 将根模式 PopupMenu 改为主窗口内部两行导航，窗口随选择器展开至 181 px，避免空白 Global Search 的 72 px 窗口裁切选项。↑ / ↓、Enter、Esc 与点击均复用主导航逻辑；取消保留查询，同一根模式不会隐藏窗口。Scope 语义不变。
- Launcher 使用中性灰半透明遮罩稳定浅色桌面下的文字层次，保留 native glass；路径始终用次级文字，选中仍为透明薄层。没有新增模糊组件 / 原生 UI 依赖或更换 Impeller。
- Flutter analyze、180 项 Flutter 全量测试通过；随后追加文件图标对比度测试，7 项颜色回归全部通过。实机已查看两行菜单完整呈现与整体外观；键盘取消 / 切换、Host resize 与输入保持由 widget 回归验证。
- build 18 App / ZIP / SHA-256 已生成，版本 / arm64 / Impeller / 固定签名 / ZIP 内容一致性通过；Rust / 原生功能代码未改，本轮未重复其功能测试，未提交推送。

### 2026-10-12：build 19，授权前置与只查询授权范围

- 启动 / 保存搜索设置时恢复或请求目录授权，macOS 使用只读安全作用域书签；查询不调用授权端口。新范围授权前停用文件源并撤销旧目标，空授权不回退 Home。
- 文件搜索只使用有效授权 roots；显式路径、..、符号链接和 Spotlight 元数据候选均受范围 / 可读性约束。没有调整 Scope 语义或添加持久文件索引。
- PermissionDenied 单独计入受限位置跳过数，不当作 Partial 故障，不显示权限错误条和 Retry；其他 I/O 错误仍保持真实 Partial。无授权时给出设置页授权引导。
- Search 设置增加授权保存范围、Full Disk Access 系统设置入口；完全磁盘访问需用户手动授予，目录授权不能被描述为永久访问所有后代。
- `scripts/check.sh` 通过：340 Rust / 186 Flutter、原生授权等检查、Clippy / fmt / 架构检查通过；6 项既有测试 ignored。POSIX 不可读目录补强回归重跑通过。
- 实机 build 19 首次启动已显示系统目录授权窗口，授权范围留给用户选择；完整系统隐私授予 / 撤销矩阵未替用户执行。App / ZIP / SHA-256 及签名 / arm64 / Impeller / 包一致性通过，未提交推送。详见 `docs/file-search-access-v0.7.md`。

### 2026-10-12：build 20，架构审查修复与 15 秒恢复

- 修复授权旧回调覆盖设置与 Native 主线程磁盘工作；全链路 generation / epoch / 串行写入与 worker，保持搜索期间不主动申请授权。
- Launcher 隐藏立即暂停执行，原状态保留 15 秒；普通唤醒恢复原页面、Context、查询、Scope、过滤、预览与列表位置，超时清理。Core 独立期限防止旧执行复活，设置草稿和已有后台任务保留各自生命周期。
- 图标真实并发跨 clear / Session 保持最多 4；文件 more 使用临时外部排序与有序读位置，晚到匹配不遗漏，读取失败可重建；选中对象才生成次级动作，共享描述减少重复复制。
- sealed 导航状态、独立异步 Source 的 Replace / Append、统一 FeatureContribution、单资源生成文件类型规则已接入。未来外部插件、Host UI、PTY 和 Windows 能力未提前声明完成。
- `scripts/check.sh`：346 Rust / 189 Flutter 通过，6 Rust ignored；追加设置窗口超时草稿回归后 analyze / 190 Flutter 全量通过。受控 release more 测量 8,192 项累计 309.5 → 19.9 ms；非 App RSS / 帧率证据。
- build 20 已验证实际查询输入与按需二级 Actions，15 秒规则由确定性回归验证。App / arm64 ZIP / SHA-256 已生成；未提交推送。详见 [修复记录](review-v0.7-architecture-2026-10-12.md)、[状态保留](launcher-retention-v0.7.md)。
