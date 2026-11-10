# V0.14：应用历史与动作排序

状态：**开发预览，尚未完成正式发布验收**。源码版本以 [pubspec.yaml](../apps/flutter/pubspec.yaml) 为准。本页只维护现行范围与验证程度，旧方案及 Build 1–23 经过见 [开发记录](archive/v0.14-development-records.md)。

## 已接入的功能

| 功能 | 当前实现 | 边界 |
|---|---|---|
| 每应用独立历史 Action | 18 个内置来源；在对应 Current App 与显式全局搜索中匹配 | 未安装时默认关闭；手动开关与路径优先 |
| 配置化适配 | 内置 / 可信用户包共用 source.json、transform.js、读取器和共享页面 | 用户同 ID 覆盖优先；启动加载，修改需重启 |
| 来源文件读取 | 有界 JSON、XML、只读 SQLite 与按需 QuickJS 转换 | 一次查询一个来源；不导入或镜像全部历史 |
| 路径检测 | 配置根目录规则；VS Code 优先新的 Home 共享存储，再回退旧位置 | 单来源仅选一份文件；非标准路径可手动设置 |
| 来源页面 | 搜索、拼音、选择、Enter 原应用打开、复制、刷新 | 历史页没有 ⌘K 绑定；更多操作从行内或右键进入 |
| 所选对象操作 | 在一个选中对象上解析 File / Directory Context，复用第一方 Native 动作 | 工作区保持 File，远端仅复制；不改变冻结 Current App |
| 首页与 Scope 排序 | 次数、最近使用、相关性 / 默认优先级、稳定 ID | 无首页白名单或额外加分；关键词匹配仍优先；菜单长期 Usage 仍隔离 |

现行操作和支持范围见 [应用历史](application-history.md)，配置与额度的唯一完整契约见 [来源指南](application-history-sources.md)，动作排序见 [Launcher 交互](launcher-interaction.md)。

## 明确未交付或未验收

- 多安装 / Profile 的显式选择器尚未实现。当前只允许唯一来源应用，歧义时拒绝；自定义路径用于固定一份来源文件，不能当作 Profile 路由。
- Obsidian 当前提供最近仓库。最近文件需要可靠的 Vault / 自定义配置目录关联，已暂缓接入；不把仓库列表或文件时间冒充最近笔记。
- 浏览器历史、书签与 Tab 管理已移至 [独立浏览器阶段](browser-feature.md)，不再属于本版实现范围。
- 18 个来源的完整真实应用、版本、多安装、权限和实际打开矩阵尚未验收；Windows / Linux 的原生运行也未验收。
- 本版未发布 GitHub Release。开发 App 的签名和打包成功不等于正式发行或全部实机路径通过。

## 代码入口

| 改动内容 | 首先阅读 |
|---|---|
| 新应用适配 | resources/application-history 中已有包，以及 [来源指南](application-history-sources.md) |
| 定义、继承与脚本 | application/application_history/ |
| 偏好、会话、查询与执行 | features/application_history/ |
| 来源文件与默认检测 | persistence/application_history.rs 及同名目录 |
| 对象的 Context / Registry 复用 | application/history_object_actions.rs |
| Bridge | anywhere-bridge-flutter/src/api/application_history.rs |
| Flutter 页面与设置 | features/application_history/、infrastructure/application_history_backend.dart、settings/application_history_* |

路径表中 Rust 相对路径均位于 crates/anywhere-core/src，Flutter 路径位于 apps/flutter/lib。完整分层见 [架构](architecture.md)，工作入口见 [AGENTS.md](../AGENTS.md)。

## 最新已验证基线

### Rust Context 采集状态收拢

`ContextCaptureService` 接管来源策略与 Finder / 选区 / 浏览器读取、迟到 Capture 发布；剪贴板请求、复制年龄和 SessionFacts 写入收归同一服务。Application 保留设置入口及执行 / Web 凭据撤销，激活仅编排剪贴板、Git 和 Usage 的独立刷新。公开 API、DTO、单次来源读取、输入模式及策略 → Session → Web 锁顺序保持一致，代码归属见[架构](architecture.md)，来源行为见[Context 模型](context-model.md)。

新增回归覆盖四种来源关后重开仍拒绝旧状态、在途剪贴板拒绝旧正文 / 复制年龄而新激活正常采集，以及排队请求不采用后来的策略。前两项在提取前通过；Review 用第三项抓到初版提取将令牌取得推迟到 worker 的差异，修正为排队前固定请求后通过。正常权限下 Application 单元回归 137 项、Context 来源 MVP 20 项、慢剪贴板独立 Usage 发布 1 项通过；受限环境初跑的 home 路径夹具因权限拒绝失败，正常权限复跑通过。

Review 核对策略封装、排队 / 读取 / 写入时序、事实与 Capture 发布条件、锁顺序和补充组完成语义，未发现剩余新增阻塞问题。完整 `scripts/check.sh` exit 0：Core 340 项通过 / 1 项忽略、MVP 195 项通过 / 4 项忽略、Flutter 524 项通过 / 1 项跳过、SDK 50 项及其余工作区、原生 Host、Clippy、架构、规范、生成物和文档门禁通过。完整日志为被忽略的 `build/context-capture-full-check.log`。

干净提交 `ec42746` 已构建为独立的 `Anywhere-0.14-context-capture-ec42746.app`（0.14.0+46 / arm64 / Impeller），持久本机签名、ZIP 内容及 SHA-256 校验通过。核实进程显式使用独立数据目录后，GUI 冒烟确认四种采集关闭时合成 URL 的手动输入仍生成打开 / 复制候选，Context 与 Clipboard 设置显示关闭状态，与隔离设置文件一致；未执行候选或修改个人设置，实例经菜单正常退出并用只读进程检查确认。启动时无关的文件搜索授权已取消；本轮未新增目录授权或真实来源开关 / 采集验收。自动输入遗漏冒号的轮次不计入结果，使用粘贴后核对完整 URL 才计为通过。

隔离日志 AXTree、Flutter ERROR 和 frame-time clamping 均为 0；stdout 有 `Resize timed out` 1 条，另有 IMK mach port、TSM 指示灯及窗口移动诊断各 1 条。本轮后续界面操作通过，但这些诊断的原因尚未定位，不记为零警告运行。来源及证据保留在被忽略的 `build/Anywhere-0.14-context-capture-ec42746.provenance.json` 和 `build/acceptance-context-capture-ec42746/verification.json`；不扩展为物理快捷键、输入法、VoiceOver、预览、内存或跨平台实机验收。

### Rust 应用索引所有权收拢

`ApplicationIndex` 接管完整 / 可见应用目录、排除偏好、扫描代次及完成通知；`Application` / `Resolver` 不再分别保存这些可变字段。扫描发布与过滤共用一个状态锁，Resolver 持有不可变快照并通过服务复验。首屏不等待扫描、每次激活重扫、失败保留旧目录、快捷键配置可见完整应用及 Session / Revision / epoch 契约保持一致。代码归属见[架构](architecture.md)。

专项 5 项通过：受控旧快照 / 排除变化竞态、旧扫描不能覆盖新完成、新扫描等待与失败通知、过滤变化不删除快捷键配置标签、部分 / 空目录成功与在途快照资格。Review 核对状态所有权、锁顺序、扫描完成通知和首屏 / 迟到重算路径，未发现新增阻塞问题。Core 全目标编译及完整 `scripts/check.sh` exit 0：Core 337 项通过 / 1 项忽略、MVP 195 项通过 / 4 项忽略、Flutter 524 项通过 / 1 项跳过、SDK 50 项及其余原生、Clippy、架构、规范、生成物和文档门禁通过。完整日志为被忽略的 `build/application-index-full-check.log`。

干净提交 `0962d79` 已构建为独立的 `Anywhere-0.14-application-index-0962d79.app`（0.14.0+46 / arm64 / Impeller），持久本机签名、ZIP 内容及 SHA-256 校验通过。显式核实进程隔离目录后，GUI 冒烟通过全局搜索 Calculator、保存 `com.apple.calculator` 后候选消失、重开设置回读、清空排除后候选恢复；两次保存均核对隔离目录落盘。未执行应用候选，未替换日常 App，偏好修改仅在隔离目录执行；测试实例经原生菜单正常退出并用只读进程检查确认。来源与证据保留在被忽略的 `build/Anywhere-0.14-application-index-0962d79.provenance.json` 和 `build/acceptance-application-index-0962d79/verification.json`。

恢复实测时曾出现不带隔离变量的实例，已在修改偏好前退出并显式重启；期间 Current App 激活打断的操作不计为全局搜索验收，部分 AX 文本未跟随可见编辑时以截图和落盘交叉核对。隔离日志 AXTree、Flutter ERROR 和 frame-time clamping 均为 0，另有 IMK mach port、TSM 键盘指示灯及窗口移动诊断各 1 条，原因未在本步定位，不记为零警告运行。本项未新增物理快捷键、输入法、VoiceOver、预览、内存或跨平台实机验收。

### 应用索引排除变化的发布复验

受控双等待回归复现“排除列表已更新、替代快照尚未完成”时旧计算仍发布被排除应用。现同时复验扫描代次和可见索引 Arc 身份，目录改变后旧准备结果重新计算，不依赖另一个快照先完成。原查询、排序和公开 API 保持一致。

该回归在旧逻辑上失败，修复后通过；完整 `scripts/check.sh` exit 0：Core 333 项通过 / 1 项忽略、MVP 195 项通过 / 4 项忽略、Flutter 524 项通过 / 1 项跳过、SDK 50 项及其余门禁通过。初次受限环境的 MVP 运行因 home 目录与原始文件名夹具被拒失败；正常权限的全量检查覆盖这些用例并通过。完整日志为被忽略的 `build/application-index-fix-full-check.log`，本项未新增打包或 GUI 验收。

### SDK 文本上传职责收敛

界面与后台 Adapter 共用 UTF-8 校验、惰性分块及上传事务，公开 API、Rust 协议、浏览器排队 / 取消生命周期、后台 8 KiB 直接写入阈值和各自错误优先级保持一致。合成大文本不再先生成完整字节副本或分块数组；实现归属见[架构](architecture.md)。

新增 16 项双适配器回归在迁移前后均通过，覆盖 Unicode 边界、完整 1 MiB、转义后超帧、非法代理项、复合无效输入、预取消、begin / chunk / commit 失败和分块间取消。真实 QuickJS / Rust broker 在 Release 配置、现有 250 ms / 32 MiB 限制下通过五组样例，核对合成剪贴板完整往返及无效输入后继续写入；该回归已接入示例检查。Debug 下完整 MiB 样例在旧、新实现中均触发 250 ms 预算，未修改运行时额度或据此宣称所有后台业务均能处理最大文本。

Review 已核对两种适配器的错误与取消顺序，未发现新增阻塞问题。完整 `scripts/check.sh` exit 0：SDK 50 项、Flutter 524 项通过 / 1 项跳过、Core 332 项通过 / 1 项忽略，其余 Rust 工作区、原生 Host、Clippy、架构、规范、生成物和文档检查通过；SDK `.tgz` 导出及仓库外两种独立模板的类型检查、构建 / 打包与后台互通通过。完整日志为被忽略的 `build/sdk-text-full-check.log`。本项未修改原生 App，也未新增 GUI、物理输入法或内存验收；已安装插件需使用更新 SDK 独立构建后导入。

### 原生设置与权限职责拆分

`SettingsHost` 接管权威设置、保存流程、快捷键目录复验、目录授权入口与状态查询，`LoginItemService` 独立适配系统登录启动；AppDelegate 只读设置并组装服务。现有 MethodChannel 方法和错误回复、部分保存失败的处理、目录 worker / epoch 及独立数据目录隔离保持原契约。实现归属见[架构](architecture.md)。

新增真实 Flutter 编解码边界回归覆盖无效请求无副作用、快捷键拒绝后的回滚与其他偏好落盘、登录失败及状态回读、磁盘失败不分发新语言、旧目录快照拒绝、设置关闭恢复失败与重试、授权被替换后的迟到回复、强制授权及隐私入口。原生 Host 回归与完整 `scripts/check.sh` exit 0：Flutter 524 项通过 / 1 项跳过，Rust Core 332 项通过 / 1 项忽略，其余工作区、Clippy、SDK、架构、规范和生成物门禁通过。Review 已核对迁移前后的请求字段、回复和副作用顺序，未发现新增阻塞问题。

干净提交 `d1cb2bc` 已构建为独立的 `Anywhere-0.14-settings-host-d1cb2bc.app`（0.14.0+46 / arm64 / Impeller），完整 AppDelegate 组装、本机持久签名、ZIP 内容和 SHA-256 校验通过。独立数据目录中，语言保存并重开、隔离登录偏好保存与回读、已有合成目录授权恢复、搜索偏好保存，以及动作候选读取 / 录制 / 保存 / 关闭设置的 GUI 冒烟通过；落盘结果已核对。测试实例正常退出，日志 AXTree 错误、Flutter ERROR 与 frame-time clamping 均为 0，未替换日常 App 或编辑个人设置。来源及证据保留在被忽略的 `build/Anywhere-0.14-settings-host-d1cb2bc.provenance.json` 和 `build/acceptance-settings-host-d1cb2bc/`。

本轮合成动作热键落入文本框，自动化 Universal 切换紧接着触发 currentApp 激活；这些操作不记为硬件快捷键或新增预览 / 过期验收，也未据此修改生产按键和窗口语义。登录项真实注册、macOS 12 兼容分支、物理输入法、VoiceOver 与全部格式矩阵仍未新增实机验证，上一轮帧时间诊断仍未定位；本轮日志未出现该消息不代表已修复。

### Native Host 通道职责拆分

快捷键录制通道与待回复请求由 `ShortcutRecordingHost` 持有，保留设置窗口资格、本地按键与 Carbon 唤醒键转换、失焦取消和请求 ID 隔离。新增真实 Flutter 编解码边界回归覆盖拒绝录制、替换、迟到取消、重复按键和 Host 销毁后的单次回复及解除通道。原生 Host 回归与完整 `scripts/check.sh` exit 0：Flutter 524 项通过 / 1 项跳过，其余工作区、Clippy、SDK、架构、规范及生成物检查通过。本项尚未新增打包或 GUI 验收。

Web 工具通道、活动与 Host 面板状态迁至 `WebToolsHost`，统一编辑 / 按键转交与辅助功能子树生命周期；保留 Flutter / Web 两种焦点目标、唯一 WebView 规则和组词边界。新增编码通道回归覆盖活动 / 非活动协议、原生画布、有无 WebView、非 key 窗口不抢输入、隐藏撤销与释放恢复；既有 Web 编辑及完整 Host 回归通过。完整 `scripts/check.sh` exit 0：Flutter 524 项通过 / 1 项跳过，其余门禁通过，本项尚未新增打包或 GUI 验收。

窗口展示代次与设置状态收归 `LauncherWindowCoordinator`，AppDelegate 不再写代次、移动共享视图或直接调整窗口尺寸。原生回归通过迟到 show / resize / hide、隐藏抢先返回、失焦幂等与设置失焦豁免、共享内容和空设置窗口释放；被拒请求也不会运行展示钩子。完整 `scripts/check.sh` exit 0：Flutter 524 项通过 / 1 项跳过，其余原生、工作区与规范门禁通过。本项源码检查不替代后续打包和实机流程。

干净提交 `74bd8b6` 已构建为独立的 `Anywhere-0.14-native-host-74bd8b6.app`（0.14.0+46 / arm64 / Impeller），完整 AppDelegate 组合编译、持久本机签名、ZIP 内容及 SHA-256 校验通过。首次独立预览实窗回归因锁屏未取得 active / key window；用户解锁后重跑 exit 0，覆盖首次输入竞争、立即重开、原生关闭 / 隐藏、XML 加载与关闭、隐藏 16 秒及 Host 释放。

同一测试包在独立数据目录中通过新组合键录制、Esc 取消、已注册组合键再次录制，以及无 Host 权限的合成 Web 工具输入、⌘K 进入 Host、Esc 返回 Web 继续编辑、原生 Undo / Redo 和返回搜索。合成 XML 的空格 / 眼睛预览及系统按钮关闭通过。最终退出后复核日志：AXTree 错误为 0，Flutter ERROR 为 1，对应一条 `Reported frame time is older than the last one; clamping`（时间差约 16.7 ms）。本轮操作未观察到界面失效；这条帧时间诊断的原因和性能影响尚未确认，不记为已修复或零错误运行。自动化未触发失焦或切换搜索模式的轮次不计为过期验收。用户随后按要求实际切换应用、等待至少 16 秒并唤醒，确认“正常：空白首页，无旧预览”；日志记录了真实失焦和预览关闭，约 97.7 秒后的下一份可见记录为新首页（`currentApp`），不将这份日志扩展为 Universal 快捷键硬件事件的独立证明。

隔离实例已正常退出，日常实例与设置未修改；录制键和合成插件仅用于独立测试目录。证据与来源保留在被忽略的 `build/acceptance-native-host-74bd8b6/` 和 `build/Anywhere-0.14-native-host-74bd8b6.provenance.json`。本轮仅补充自动化冒烟与用户操作确认，未改生产源码，也不扩展为完整快捷键事件矩阵、物理输入法、VoiceOver 或全格式验收。

### 设置保存与目录职责拆分

`SettingsSaveCoordinator` 接管 Host 设置、保存队列、权威回读与文件授权失效；Controller 保留草稿、关闭选择和反馈。新保存排队即撤销旧授权，销毁后不再执行后续写入或发布迟到回读。公开设置 API 与原生协议保持一致，状态归属见[架构](architecture.md)。

相关设置用例 48 项通过，包括并发保存、部分失败、Host 拒绝绑定、授权读回失效、保存期间的新草稿和关闭决策；新增受控顺序覆盖排队保存撤销授权以及销毁后阻止排队写入。完整 `scripts/check.sh` exit 0：Flutter 522 项通过 / 1 项跳过，其余工作区、原生 Host、Clippy、SDK、架构、规范与生成物检查通过。本项未打包或新增 GUI 验收。

随后由 `ShortcutCatalogCoordinator` 接管目录、资格及原生注册队列；保留 Host 拒绝 / 不确定资格时注销按键与重试恢复。新增回归在拆分前复现旧目录状态覆盖新保存警告，增加快照复验后通过。相关设置用例 49 项通过，完整 `scripts/check.sh` exit 0：Flutter 523 项通过 / 1 项跳过，其余门禁通过；本项未打包或新增 GUI 验收。

`FeatureCatalogCoordinator` 接管功能状态和变更队列，关闭设置等待队列排空。新增受控回归在拆分前复现“关闭已等待旧任务、来源协调随后排队”时提前关闭，修正后通过；失败后继续协调、权限撤销与恢复、迟到状态和销毁路径保持通过。相关设置及应用历史用例 83 项通过，完整 `scripts/check.sh` exit 0：Flutter 524 项通过 / 1 项跳过，其余门禁通过。关闭选择继续由 Controller 统一处理，不另建平行规则；本项未打包或新增 GUI 验收。

随后从干净提交 `ddb4478` 构建独立的 `Anywhere-0.14-settings-ddb4478.app`（0.14.0+46 / arm64 / Impeller），本机持久签名、ZIP 内容和 SHA-256 校验通过。在独立数据目录中，桌面冒烟通过语言保存并重开、搜索草稿取消关闭后保留、保存目录后搜索生效、Git 关闭再恢复、快捷键候选加载与取消，以及合成 XML 的空格 / 眼睛预览、Esc / 系统按钮关闭与输入恢复。AXTree 错误、frame-time clamping 和 Flutter ERROR 均为 0；隔离实例正常退出，日常 Build 46 保持运行。产物来源与证据保留在被忽略的 `build/Anywhere-0.14-settings-ddb4478.provenance.json` 和 `build/acceptance-settings-ddb4478/`。该提交的 [macOS 检查](https://github.com/FantasyDream/Anywhere/actions/runs/42201003642)、[Linux / Windows 共享检查](https://github.com/FantasyDream/Anywhere/actions/runs/42201003641)及[文档检查](https://github.com/FantasyDream/Anywhere/actions/runs/42201003631)通过；自动打包仅显式 dispatch 执行，本轮未触发。此项为自动化冒烟，未新增物理输入法、VoiceOver、全部格式或内存验收。

### 展示失败反馈的消息顺序回归

原生展示失败先发送 `closed`、后返回请求错误时，旧 Flutter 关闭处理会推进代次，导致错误提示被丢弃。现在 `closed` 携带明确的 `show_failed` 原因；当前请求提示一次，普通关闭、主动取消与旧请求保持原有失效规则，旧选择失败也不会撤销排队中的新选择。平台协议见[系统预览](native-preview.md)。

旧实现的受控顺序回归失败，修复后两种消息顺序、重复事件、主动取消、已替换及在途旧选择五项回归通过。相关 Flutter 用例 64 项通过；原生实窗覆盖无法转交键盘时的失败原因、单次关闭与编辑器恢复，既有首次打开、系统按钮、XML、隐藏 16 秒及 Host 释放也通过。完整 `scripts/check.sh` exit 0，Flutter 520 项通过 / 1 项跳过，其余工作区、原生 Host、Clippy、SDK、架构、规范与生成物检查通过。本项没有新增真实输入法或全部格式验收。

### 预览依赖辅助功能状态的焦点修复 — Build 46

Build 45 的用户先操作日志捕获连续失败：资源准备完成，原生输入是 `FlutterTextInputPlugin`，在焦点转交处返回 `preview_unavailable`，尚未到系统渲染。同一个进程随后经辅助功能读取与操作后能够预览，复现“自动化验证后才能用”的差异。核对固定 Flutter 引擎源码，FlutterView 在输入插件活跃时拒绝自动焦点；旧接收视图扫描依赖此时的 `acceptsFirstResponder`，辅助功能额外创建的原生文本控件掩盖了问题。

现先暂时结束 first responder，再查找内容接收视图，失败时恢复原输入。新增动态拒绝焦点的受控回归在旧逻辑上首次打开失败，修复后通过；保留未预热共享面板、快速重开、输入恢复和过期检查。完整 `scripts/check.sh` exit 0：Flutter 515 项通过 / 1 项跳过，Rust Core 332 项通过 / 1 项忽略，其余工作区、原生 Host、Clippy、SDK、架构、规范与生成物检查通过。

源码 `1638c2f` 已构建为 Build 46（0.14.0+46 / arm64 / Impeller），持久本机签名、ZIP 内容与 SHA-256 校验通过。Build 45 已正常退出，唯一日常 Build 46 进程已启动；启动后未读取该进程的辅助功能界面或预先打开预览。用户先操作并确认“能打开，过期唤醒也正常”；日志首次接管时键盘接收者为 `FlutterView`、面板可见，后续连续展示与资源保留成功，外部失焦约 38 秒后唤醒回到高度 72 的空白首页。源码、归档与失败 / 对照 / 回归证据保留在被忽略的 `build/Anywhere-0.14-preview-build46.provenance.json`、`build/acceptance-preview-build45/` 和 `build/acceptance-preview-build46/`。该确认覆盖此次文件预览与过期流程，未扩展为全部格式、输入法、VoiceOver、内存、跨平台或远端 CI 验收。

### 首次预览失败的继续诊断 — Build 45

用户随后反馈日常 Build 44 仍无法打开预览，而自动化验证后能够打开，因此 Build 44 的工程通过不能视为已解决打开失败。原生回归原先在首次请求前创建共享 Quick Look 面板，现补充未创建面板时的首次 XML 打开检查；该检查在 Build 44 行为上通过，说明尚不能把问题单独归因于面板初始化。当前机器启用了支持 XML 的 dotViewer Quick Look 扩展，但没有确认其为故障原因。

Build 45 增加请求、资源准备、原生控制权、展示、保留失败与关闭的阶段日志，只记录时间、代次、状态、错误类型和原生类名，不记录查询、路径或正文。该构建捕获了用户先操作的失败现场，随后完成同进程辅助功能操作对照，定位与修复见上节 Build 46；诊断包自身不包含焦点修复。

### 隐藏后预览状态与唤醒清理 — Build 44

用户报告文件搜索的 XML 预览失焦超过 15 秒后，空白首页旁重新出现旧的空预览窗口。代码检查发现 Quick Look 直接隐藏不一定发送 `willClose`，旧 Host 会话与前端状态可能继续有效；旧实现配合修正后的独立回归在此处失败。现观察隐藏并带资源 / generation 复验收尾，主动关闭期间抑制重复回调；Host 统一管理失焦关闭，并在唤醒 / 激活后清理无所属会话的系统面板。行为与边界见[系统预览](native-preview.md)。

完整 `scripts/check.sh` exit 0：Flutter 515 项通过 / 1 项跳过，Rust Core 332 项通过 / 1 项忽略，其余工作区、原生 Host、Clippy、SDK、架构、规范及生成物检查通过。最终独立原生回归通过系统隐藏、旧隐藏回调后立即重开、XML 关闭与加载中关闭、应用隐藏 16 秒后再激活及 Host 释放。测试程序改用正常非零退出报告失败，避免断言触发系统“PreviewTests 意外退出”弹窗；保留主队列调度机会并严格检查桌面激活前置条件。

Build 44（0.14.0+44 / arm64 / Impeller）本机持久签名、ZIP 内容和 SHA-256 校验通过。隔离数据目录与合成 XML 已实测空格打开、XML 渲染、原生菜单唤醒返回原查询及正常退出；AXTree、Flutter ERROR 与 frame-time clamping 均为 0。桌面工具操作计算器未产生 Launcher 失焦日志，该轮等待不能算作外部失焦 / 过期验收。旧诊断版已退出，已按明确路径启动唯一日常 Build 44；用户的物理唤醒键连续复测仍待确认。未新增完整输入法、VoiceOver、全部格式 / 内存 / 跨平台矩阵或远端 CI 验收。

### 系统预览关闭按钮竞态 — Build 43

旧 Build 42 出现 XML 预览无法关闭的现场，采样时 App 主线程仍在等待事件，没有发现主线程死锁；正常退出旧实例后，用户确认残留面板消失。独立实窗回归进一步复现系统关闭按钮的异步窗口期：动作返回后旧控制状态仍未结束，原先关闭后等待 500ms 的回归没有覆盖立即重开。Quick Look 的按钮会绕过普通 `windowShouldClose`，现以公开 AppKit target/action 接入已有同步收尾，结束时恢复原处理器，详情见[系统预览](native-preview.md)。

修改前真实系统按钮回归失败，修改后通过立即重开、按钮处理器恢复、XML 渲染后关闭、加载中关闭、隐藏 16 秒和 Host 释放。完整 `scripts/check.sh` exit 0，Flutter 514 项通过 / 1 项跳过，Rust Core 332 项通过 / 1 项忽略，其余工作区、原生 Host、Clippy、SDK、架构、规范及生成物检查通过；随后仅同步 Flutter / Native 构建号至 43。旧现场的长时间不可关闭状态尚未在受控环境完整重现，不能把关闭竞态回归当作所有 XML / 系统扩展路径已验收。

`7724530` 已构建为 `Anywhere-0.14-preview-build43.app`（0.14.0+43 / arm64 / Impeller），本机持久签名、ZIP 内容和 SHA-256 校验通过。独立数据目录与合成 XML 实测通过系统按钮关闭、立即空格重开、HTML 内容获得焦点后 Esc 关闭及继续编辑查询；日志中 AXTree、Flutter ERROR 和 frame-time clamping 均为 0。隔离实例正常退出，随后已按明确路径启动日常 Build 43，核对唯一运行进程；旧 Build 42 已退出。源码、归档摘要、旧现场采样和本轮证据保留在被忽略的 `build/Anywhere-0.14-preview-build43.provenance.json` 与 `build/acceptance-preview-build43/`。未新增物理输入法、完整 VoiceOver、格式 / 内存 / 跨平台或远端 CI 验收。

### Tooltip 语义树更新修复 — 2026-11-05

已在真实 CandidateRow / AppRowActions 组合中稳定复现上一轮 AXTree 错误：依次显示多个 Tooltip 时，相邻浮层锚点被固定 Flutter 3.47.8 合并，增量更新包含无法从根节点到达的节点。共享 `AppTooltip` 为每个显式提示建立独立语义容器，Launcher、工具、设置与任务页面统一使用；保留提示、标签与语义按钮操作。实现与升级复验边界见[架构](architecture.md#固定-flutter-版本的表面缓存保护)。

新增三项回归校验实际序列化增量树，覆盖五个相邻提示、鼠标悬停、中英文切换、列表移除重建、可访问按钮与导航执行、14 秒保留和两轮 16 秒过期后输入更新。原实现及仅修复操作按钮的中间实现均复现孤立节点，完整修复后通过。首次新包复测发现导航标签和点击动作被边界隔开，随后把它们移回同一节点并补充语义点击回归。修正后重新运行完整 `scripts/check.sh`，exit 0：Flutter 514 项通过、1 项跳过；Rust Core 332 项通过、1 项忽略，其余 Rust 工作区、原生 Host、Clippy、SDK、架构、规范及生成物检查通过。

最终源码 `3546e8d` 已构建为独立的 `Anywhere-0.14-refactor-3546e8d.app`（0.14.0+42 / arm64 / Impeller），持久本机签名、ZIP 内容及 SHA-256 校验通过。在独立数据目录与合成授权范围中，桌面复测通过导航语义点击、40 项文件结果、默认未选择与方向键选择、空结果过滤、中英文文本预览、Actions 返回、设置语言保存与过期后返回；行内预览后原生菜单唤醒正确返回原查询，再隐藏约 32.7 秒重开时旧查询和预览不恢复，随后 `after-expiry` 输入值在原生辅助功能树中正确更新。本次运行 AXTree 错误、frame-time clamping 和 Flutter ERROR 均为 0，验收实例正常退出；独立原生预览实窗回归也通过。产物、源码指纹与详细本机证据保留在被忽略的 `build/Anywhere-0.14-refactor-3546e8d.provenance.json` 和 `build/acceptance-ax-fix-3546e8d/`。本轮不包含物理全局快捷键、真实中文输入法组词、完整 VoiceOver、格式 / 内存 / 跨平台矩阵或远端 CI 验收。

### Launcher 内部职责整理 — 源码与打包验证

会话协调器拥有已接收快照与持续订阅，统一检查会话、revision、快照顺序与增量列表基线；Controller 保留导航和界面更新。展示文案使用调用方提供的当前语言，窗口尺寸与行坐标使用独立布局策略。实现入口见[架构](architecture.md#快照取消与执行资格)。未修改 Bridge 契约或构建号。

本轮完整 `scripts/check.sh` 通过：Flutter 511 项通过、1 项跳过，Rust Core 332 项通过、1 项忽略；原生 Host、Rust 工作区其他测试、Clippy、SDK、模板、架构及生成物检查通过。受控竞态覆盖旧会话订阅和等待中的列表替换。

2026-11-04 至 11-05，将 `ef9e574` 构建为独立的 `Anywhere-0.14-refactor-ef9e574.app`（0.14.0+42 / arm64 / Impeller），本机签名、ZIP 内容及 SHA-256 校验通过。在独立数据目录和合成文件范围中，经桌面自动化验证中英文搜索、文件类型过滤、默认未选择、原生文本预览、对象 Actions 返回、设置语言保存；日志记录失焦后约 7 秒恢复原查询，通过同一原生菜单入口隐藏后超过 17 秒再唤醒则进入新会话，旧查询与预览不恢复，后续可直接输入。原生独立实窗回归也通过输入恢复、快速重开、更新、系统关闭、隐藏 16 秒及 Host 释放。

该旧包运行日志出现 11 条 Flutter `Failed to update ui::AXTree` 错误和 1 条 frame-time clamping 消息；过期唤醒后窗口截图与输入可用，但辅助功能树中的输入值呈现不完整。Tooltip 语义树问题随后按上节定位并修复，旧包本身不包含修复。验收实例已正常退出。以上为自动化桌面冒烟验证，不标记为完整辅助功能验收；物理 ⌘Space 跨应用连续流程及真实中文输入法组词仍未验证，也不替代下述 Build 42 的完整实机矩阵。

### 隐藏与过期后的预览清理 — Build 42

程序性隐藏 / 清理预览不激活旧窗口；隐藏窗口的编辑器连接由正常唤醒路径恢复。Quick Look 若在会话结束后延迟显示，原生 Host 立即隐藏无有效关联的面板；过期后唤醒先清除旧预览，再走正常会话恢复 / 新建流程。

本轮完整 `scripts/check.sh` 通过，Flutter 504 项通过、1 项跳过；原生实窗回归通过输入恢复、立即重开、更新、系统关闭、隐藏后等待 16 秒仍保持关闭及 Host 释放。Build 42 本机签名、打包与归档校验通过，实际 App 已打开合成文本文件的系统预览。实际 App 的外部失焦 / 超时 / 物理 ⌘Space 连续验收因锁屏中断，尚未确认；原生回归不替代该项验收。

### 预览期间唤醒窗口闪退修复 — Build 40

Build 39 的真实键盘对照复现：快速查看打开时按 ⌘Space，搜索与预览窗口闪现后消失。预览按键路由没有排除普通按键上的修饰键；同时，预览背后的 Launcher 仍处于可见状态，同一唤醒模式会走切换隐藏。Build 40 将预览中的两个主唤醒入口改为先回到所属查询窗口，避免再次切换隐藏；普通预览键只接受无修饰键，保留 ⌘L / ⌘Y。

无需桌面的原生按键回归在旧逻辑上断言失败，修复后通过，覆盖带修饰键的空格、Esc、确认与方向键。完整 `scripts/check.sh` 通过：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 504 / 1 跳过、SDK 34、JSON 30。原生窗口回归新增返回所属窗口与焦点恢复检查；第一次运行被桌面激活前置条件挡住，修复后在活跃桌面重跑通过。Build 40 已完成签名 / 归档并按明确路径启动，唯一运行实例已核对；用户对 Build 40 使用真实 ⌘Space 复测，确认预览关闭后正常返回搜索窗口并保持显示；该结果针对本次闪现后消失场景，不代表全部预览格式、权限或跨平台矩阵均已验收。

### PR 原生测试的 Swift 兼容修复 — 2026-11-02

[PR #20](https://github.com/FantasyDream/Anywhere/pull/20) 首轮远端文档、Linux 和 Windows 检查通过；macOS 在预览测试的 `weak let` 声明处编译失败。较早 Swift 要求弱引用使用 `var`，本机 Swift 6.4 又将未修改的弱 `var` 报为警告，现行检查将警告视为错误。

释放观察改为弱捕获闭包，仍在清除强引用后断言 Host 已释放；不改变生产代码、降低警告要求或删除测试。Swift 规范、原生 Host 及完整 `scripts/check.sh` 在本机通过，测试数量与下述提交前基线相同；远端结果以 PR 检查为准。此前打包的新程序不受这项测试源码修复影响。

### 提交前复查与 beta 构建实测 — 2026-11-02

复查当前未提交的 Flutter 交互 / 设置、Rust 历史查询 / 项目动作、插件生命周期 / SDK 状态同步、依赖及验证文档。发现 CI 直接引用不存在的 `dtolnay/rust-toolchain@1.100.0-beta.7` 分支；按该 Action 的官方用法改为 `@master` 配合 `toolchain: 1.100.0-beta.7`，同步 macOS 检查、打包及共享平台检查。其余已审查调用链未发现新的提交阻塞问题；不代表全仓逐行或所有运行环境均已验证。

本次重新运行完整 `scripts/check.sh`，exit 0：Core **332 / 1 忽略**、MVP **195 / 4 忽略**、Platform **70 / 1 忽略**、Flutter **504 / 1 跳过**、SDK **34**、JSON **30**；规范、Clippy、架构、生成一致性、原生自动回归与文档检查通过。日志保留于本机临时目录的 `anywhere-precommit-check-20261102.log`，不随源码交付。此结果使用 Rust / Cargo `1.100.0-beta.7`；不能替代目标 stable 或远端 CI 验证。

此前本轮依赖升级后的 `0.14.0+38` / macOS arm64 包为 `build/Anywhere-rust-beta-deps-test.app`，区别于 2026-10-30 的同构建号旧包；ZIP SHA-256 为 `8ccba9b140df3f78659a4b838180f32d91d99525a94a45c1d3a122ed8146f6d6`，持久本机签名与归档校验通过，保留 Impeller。该包内 Release Rust 动态库的独立 Flutter FFI 回归 **1 项通过**；普通 Widget 套件中的 opt-in 跳过项仍单独记为跳过。

2026-11-02 已通过系统启动上述新版，使用现有配置实测全局搜索 / 文件搜索、方向键选择后空格打开项目 README 系统预览、Esc 关闭并保留查询与选择、眼睛按钮重开、更多操作按钮、文件夹类型过滤、设置打开 / 切页 / 关闭及隐藏后全局快捷键唤起。未复现预览闪退；临时空数据下取消目录授权也正确保持文件搜索停用。实测只覆盖这些场景，不包含剪贴板预览、物理输入法、长时间内存、完整格式矩阵或 Windows / Linux 运行。CI 修复和本节文档不改变运行包的应用代码；本轮无远端 CI、公证或公开发行验收，分支仍为 `codex/dependency-upgrades`。

### 剪贴板焦点、修饰方向键与 SDK 文本编码修复 — 2026-10-31 工作区

剪贴板历史和悬浮窗共用的结果列表加入 TextFieldTapRegion，点击选择不再被输入框的点击外部处理撤销焦点。Launcher 只用无修饰键的上下方向键移动候选；带 Shift / Command / Control / Alt 时交回输入框，并退出空格预览浏览状态，避免编辑查询后空格仍打开预览。

新增 9 项桌面平台回归在旧实现上全部失败，修复后均通过，相关 Flutter 测试共 108 项通过；覆盖选中第二条剪贴板记录后空格预览、Enter 复制、保留查询，以及修饰方向键不移动候选、不启用空格预览。SDK 补上原始 UTF-8 小于单帧上限、JSON 转义后超限仍正确分块的用例。

SDK 写入先用已有 UTF-8 字节长度排除必然分块的文本，再为可能直接发送的小文本检查 JSON 帧大小。调用实际 SDK 写入 1 MiB ASCII，完整内容和 256 个分块保持一致；减少一次 1,048,587 字节的完整 UTF-8 编码及对应 JSON 字符串构造。临时探针只观察 TextEncoder 的输入 / 输出大小，不代表 JS 峰值堆或整 App RSS；前后结果保留于被忽略的 `build/review-fixes-20261031-sdk-before.json` / `-after.json`。

完整 `scripts/check.sh` exit 0：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 502 / 1 跳过、SDK 34、JSON 30；规范、Clippy、架构、生成一致性、原生自动回归、Rust SDK 互通与文档检查通过。完整日志为被忽略的 `build/review-fixes-20261031-full-check.log`。

未重新打包或进行本轮 GUI / 物理输入法验收；已有 Build 38 不包含这些源码改动，Web 插件也需要使用更新 SDK 独立构建。工作区未提交 / 推送。

### 文件结果点击后空格预览修复 — Build 38

桌面端点击文件结果时，TextField 默认的点击外部处理会撤销搜索框焦点，导致选中行的眼睛按钮可用、空格却不能请求预览。文件结果区加入现有 TextFieldTapRegion，使点击选择保留键盘焦点；不改预览资源、原生窗口或空格输入边界。

新增真实 Launcher 页面回归：非空 `settings.xml` 查询、点击第二个同名结果、空格请求该项预览且查询不变。在旧实现上未请求预览，修复后 macOS / Windows / Linux 平台变体及相关测试共 55 项通过。完整 `scripts/check.sh` exit 0：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 493 / 1 跳过、SDK 33、JSON 30；规范、Clippy、架构、生成一致性、原生自动回归与文档检查通过。

`0.14.0+38` / macOS arm64 已重新编译，保留 Impeller；持久签名、与 Build 37 的签名身份一致性、ZIP 内容及 SHA-256 校验通过。正常退出 Build 37 后确认唯一运行实例为 Build 38。源码指纹和启动记录见[打包记录](benchmarks/package-v0.14-build38-2026-10-30.json)。本次未完成 Build 38 的额外 GUI / 物理输入法验收；仍为未提交工作区，无推送、分支合并或公开发布。

### 重新打包并启动 — Build 37

`0.14.0+37` / macOS arm64 汇总下述已验证的主程序修复与优化，仍保留 Impeller。沿用最近完整工程检查：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 490 / 1 跳过、SDK 33、JSON 30；检查后仅更新 Flutter / 原生构建号，再重新编译 Release App 与 Rust Bridge。

持久本机签名、严格签名检查、ZIP 内容与 SHA-256 校验通过，签名要求与 Build 36 相同。正常退出 Build 36 后启动明确路径的新包，已确认唯一运行实例为 Build 37。版本、源码工作区指纹、包摘要与启动记录见[打包记录](benchmarks/package-v0.14-build37-2026-10-30.json)。

仍位于 `codex/dependency-upgrades`，源码修复尚未提交 / 推送或合入 main；已安装 Web 插件不会随 App 打包更新。此次确认启动身份，未新增 GUI / 物理输入法、预览格式、整 App 内存或远端 CI 验收；本机开发签名不等于公证发行。

### 历史设置与读取器内存修复 — 2026-10-30 工作区

修复历史设置已显示时，保存并关闭失败后筛选框仍保留旧查询、列表却按失败来源筛选的问题。筛选编辑控件改由 AppHistorySettingsController 持有，页面不再保存第二份文本；新 Widget 回归在旧实现上失败，修复后包含草稿 / 关闭流程的相关 Flutter 50 项通过。

XML 节点与 SQLite 结果表直接移动已构造的 JSON 值，不再通过 `json!` 对子树 / 整表重复序列化；输入、取消与只读预算不变。以实际读取器对合成文件测量：XML 为 20 层、1,000 个条目，每条含 256 字节 ASCII 属性值；SQLite 为 1,000 行，含整数、256 字节文本与 null 列。序列化输出分别为 324,809 / 292,900 字节，优化前后完整 SHA-256 一致。

| 夹具 | 累计申请字节：前 → 后 | 峰值存活增量（字节）：前 → 后 |
|---|---|---|
| XML | 36,012,564 → 3,376,944 | 3,946,635 → 2,347,280 |
| SQLite | 1,902,634 → 968,690 | 1,869,627 → 935,683 |

统计沿用现有 Rust 堆测量方式，单独运行一个测试、预热后统计读取及 JSON 构造；不含 SQLite / C 堆、分配器开销、VM、脚本转换、Flutter 或 GPU，不是 RSS / physical footprint。临时测量入口已移除；原始日志与摘要保留于被忽略的 `build/review-round6-reader-*.log` / `.json`。

历史读取 / 转换回归 42 项通过；完整 `scripts/check.sh` exit 0：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 490 / 1 跳过、SDK 33、JSON 30；规范、Clippy、架构、生成一致性、原生自动回归与文档检查通过。完整日志为被忽略的 `build/review-round6-full-check.log`。

仍在 `codex/dependency-upgrades` 工作区，未提交 / 推送或重新打包，Build 36 未更新；Bridge API 未变，未单独重复 opt-in FFI。本轮未新增原生 GUI、整 App 内存、远端 CI 或跨平台运行验收。

### 插件消息与快捷键解析简化 — 2026-10-30 工作区

就绪与状态订阅回执合并为一个类型化判断，普通 RPC 不再为呈现逻辑重复解码回复正文；UTF-8 编码结果直接交给二进制消息，不再复制整帧字节。移除连续重复的同步页面检查与外层分支，保留每个异步发布点的当前页面检查、消息大小限制和原回执语义；未增加缓存、状态或协议。

直接调用实际旧 / 新生产函数，使用含 48,000 字节 ASCII 正文的普通回复，每轮 1,000 次、同机 Flutter test Debug 五轮耗时中位数为 118.469 ms → 45.818 ms。测量包括回执分类与二进制消息构造，不包含原生通道、WebKit、SDK 客户端、渲染或整进程内存；不能作为整 App 的性能 / RSS 结论。临时测量入口已移除，原始日志与汇总保留于被忽略的 `build/review-round4-transport-*.log` / `.json`。

多规则快捷键在同一会话锁内仅展开一次动作目录，普通绑定与规则解析复用同一候选筛选实现；使用既有候选 ID 索引定位 Context，原地保留匹配项。Flutter 添加规则按钮和 Rust 入口校验统一使用现有生成策略中的规则上限。64 个 Node 脚本、两条同时匹配规则的合成项目中，直接调用实际旧 / 新服务入口，每轮 200 次、Cargo test Debug 五轮耗时中位数为 95.748 ms → 57.761 ms；同样保留歧义选择且不执行动作。该测量不包含 Context 捕获、原生按键、动作执行或整进程内存。临时测量入口已移除，日志和汇总保留于被忽略的 `build/review-round5-shortcut-*.log` / `.json`。

相关 Flutter 消息、页面生命周期、资源准入与外观回归 24 项、快捷键 Rust 12 项 / Flutter 36 项通过；最新完整 `scripts/check.sh` exit 0：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 489 / 1 跳过、SDK 33、JSON 30；规范、Clippy、架构、生成一致性、原生自动回归与文档检查通过。最新完整日志为被忽略的 `build/review-round5-full-check.log`。

仍在 `codex/dependency-upgrades` 工作区，未提交 / 推送或重新打包，Build 36 未更新；Bridge API 未变，未单独重复 opt-in FFI。本轮未新增原生 GUI、整 App 内存、远端 CI 或跨平台运行验收。

### 任务输出与动作选择修复 — 2026-10-30 工作区

修复动作选择器滚动后修改 / 清空查询时首条结果不可见、任务搜索组词时 Esc 清空查询，以及未显示的输出流变化引起当前输出区滚动。新增两条真实 Widget 回归并扩展既有输出跟随测试，三种情况在修复前分别失败，修复后相关 Flutter 20 项通过。预览标识改用已有 hex 编码器，移除逐字节创建临时字符串的循环；原资源租约与文件生命周期 4 项测试通过。

完整 `scripts/check.sh` exit 0：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 489 / 1 跳过、SDK 33、JSON 30；规范、Clippy、架构、生成一致性、原生自动回归与文档检查通过。证据保留于被忽略的 `build/review-next-*.log`。交互契约维护于 [UI 设计](ui-design.md)。

修复仍在 `codex/dependency-upgrades` 工作区，未提交 / 推送或重新打包，Build 36 未更新；未改变 Bridge API，未单独重复 opt-in FFI。本轮未新增原生 GUI / 物理输入法、整 App 内存、远端 CI 或跨平台运行验收。

### 按代码简化约定清理 — 2026-10-30 工作区

清理开发项目内部重复的字段 / 数量 / 能力标记校验与版本层，包管理器从清单入口起保持为枚举；Cargo 动作只保存目标标识，执行时读取当前清单。删除旧 Node 标志、未使用的 UI 工具函数和纯转发封装；标准原子操作与 hex 解码替代手写循环，精简过时注释。剪贴板请求约束收敛到服务入口，数据库读回仍验证存储数据；后台 SDK 只在文本入口检查非法 UTF-16，分块阶段使用已确认的数据。

Flutter 审批测试只记录页面发出的调用，不再复制审批状态与 token 规则；这些规则继续由真实 Rust Store 测试覆盖。查询边界测试使用真实 SQLite 服务。专项 Flutter 80 项与剪贴板页面 44 项通过，Rust 剪贴板 44 项 / 开发模块 10 项、真实项目命令 8 项 / 4 忽略、动态身份注册 1 项及后台 SDK 7 项通过。

续审将 Flutter 剪贴板行选择改为同步调用，移除调用方的等待与异步转发；设置保存队列直接串接操作结果，保留通知监听者前公布队尾的顺序。设置入口校验移到保存主流程下方，删除被后续状态读取覆盖的前置读取和一层转发函数；草稿归属注释与当前实现一致。剪贴板完整内容比较仍保留，用于后台刷新时维持复制反馈；未新增状态或防御分支。现有真实页面 / 控制器专项回归 126 项通过，续审源码与测试（含注释）净减少 17 行。

最新完整 `scripts/check.sh` exit 0：Core 332 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 487 / 1 跳过、SDK 33、JSON 30；规范、Clippy、架构、生成一致性、原生自动回归与文档检查通过。首轮源码与测试（含注释）相对清理前净减少 131 行；证据保留于被忽略的 `build/eight-rules-*.log` 与续审 `build/logic-simplify-*.log`。未改变 Bridge API，未单独重复 opt-in FFI。

清理仍在 `codex/dependency-upgrades` 工作区，未提交 / 推送或重新打包，Build 36 未更新。本轮未新增原生 GUI / 物理输入法、整 App 内存、远端 CI 或跨平台运行验收。

### 应用选择与 Cargo Run 修复 — 2026-10-30 工作区

应用选择框搜索 / 清空查询后同步滚动位置，保证键盘选中项可见；Cargo Run 传递目标清单的 `required-features`，入口解析后按类型沿现有元数据流传递；项目动作编码复用已有 hex 实现。新增两条真实 Widget 回归在旧实现上均失败，修复后通过；现有真实 Cargo 工作区回归增加需要两个非默认 feature 的目标，旧实现实际运行失败，修复后运行成功，原普通目标、Cargo Test 成败与目标删除检查继续通过。行为契约见[开发项目动作](developer-actions.md)。

完整 `scripts/check.sh` exit 0：Core 331 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 487 / 1 跳过、SDK 33、JSON 30；规范、Clippy、原生自动回归、架构、生成一致性与文档检查通过。专项 Flutter 15 项、开发模块 Rust 10 项及真实 Cargo 回归通过。本轮未改变 Bridge API，未单独重复 opt-in FFI；Bridge crate 随全量检查编译。

沿用同一 35 字节字符串编码 1,000 次的夹具，生产编码器由 39,000 次分配 / 520,000 字节累计申请降至 1,000 次 / 70,000 字节；这是局部 Rust 分配统计，不是整 App RSS 或存活堆降幅。原始证据保留于被忽略的 `build/practical-fixes-*.log`。

修复仍在 `codex/dependency-upgrades` 工作区，未提交 / 推送、重新打包或替换 Build 36。本轮未新增原生 GUI / 物理输入法、整 App 内存、远端 CI 或跨平台运行验收。

### 生命周期与搜索分配修复 — 2026-10-30 工作区

插件审批取消携带对应 token，旧页面清理不再取消新审批；后台 SDK 区分写入结果与读回错误，失败读取不缓存，无订阅者时不额外读回；直接标题匹配复用注册字段并保持应用限定词语义。完整 `scripts/check.sh` 通过：Core 331 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 485 / 1 跳过、SDK 33、JSON 30；Bridge API 已重新生成并编译，真实动态库 Dart FFI 另行 1 项通过。受控回归与合成分配对照见[修复记录](archive/reviews/lifecycle-followup-review-2026-10-30.md#后续修复与验证2026-10-30)。

本轮未提交 / 推送、重新打包或替换 Build 36；已安装插件需使用更新后的 SDK 重新构建并导入。GUI / 物理输入法、整 App 内存、远端 CI 与跨平台运行未新增验收。

### 跨层审查问题修复 — 2026-10-30 工作区

修复设置目录并发造成的开关状态回退、取消插件导入后列表丢失、历史查询绕过防抖，以及后台 QuickJS 超限输出的大块 Rust 副本；统一插件自动识别的文档契约。完整 `scripts/check.sh` 通过：Core 329 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 483 / 1 跳过、SDK 29、JSON 30。相关 Flutter 77 项及后台 Runtime 4 项通过，受控时序与分配复测见[修复记录](archive/reviews/cross-layer-review-2026-10-30.md#后续修复与验证2026-10-30)。

本轮 API 未变，Bridge 随完整门禁编译，未单独重复 opt-in FFI；尚未提交 / 推送、重新打包或替换 Build 36。GUI / 物理输入法、整 App 内存、跨平台运行和远端 Actions 未新增验收。

### 升级后审查问题修复 — 2026-10-29 工作区

修复 SDK 大状态同步及写后读取、主历史页面长按 Enter 重复执行、应用历史读取槽等待、转换输出提前限额，以及剪贴板空闲全表轮询。完整 `scripts/check.sh` 通过：Core 328 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 476 / 1 跳过、SDK 29、JSON 30；Bridge 已重新生成并编译，真实 FFI 1 项独立通过。复现对应关系及合成资源计数见[修复记录](archive/reviews/resource-design-review-2026-10-29.md#后续修复与验证2026-10-29)。

本轮尚未重新打包、替换运行中的 Build 36 或提交 / 推送；修复仍在独立升级分支。上述自动验证不含新增 GUI / 物理输入法、整 App 内存、跨平台运行或远端 Actions 验收。

### 升级工具链与依赖后重新打包、启动 — Build 36

`0.14.0+36` / macOS arm64 包含升级分支 `c98a8e7` 的工具链与依赖修复，使用 Rust 1.99.1、Flutter 3.47.8 / Dart 3.13.7，保留 Impeller。持久本机签名、严格签名检查、ZIP 内容 / SHA-256 和包内 arm64 Flutter 引擎 UUID 核对通过；正常退出 Build 35 后启动明确路径的新包，已确认实际运行版本为 Build 36。产物、来源与校验值见[打包记录](benchmarks/package-v0.14-build36-2026-10-29.json)。

沿用下节完整工程检查与真实 FFI 结果；本轮只更新构建号并重新编译 App / Rust Bridge。启动身份确认不包含 GUI / 物理输入法、预览格式矩阵或进程内存验收。升级仍保留在 `codex/dependency-upgrades`，未合入 main 或推送；已安装的 Web 插件不会随 App 更新，上游编译期安全扫描告警仍按升级记录保留。

### Rust / Flutter 稳定工具链升级 — 2026-10-29

`codex/dependency-upgrades` 进一步升级至 Rust / Cargo 1.99.1、Flutter 3.47.8 / Dart 3.13.7，并更新可兼容依赖。适配 SHA-2、SQLite 和原子接口，新增回归保护已有插件存储身份。完整 `scripts/check.sh` 通过：Core 322 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471 / 1 跳过、SDK 25、JSON 30；跳过的真实 FFI 项另行运行通过。四个 npm 树审计无已知漏洞；Rust OSV 查询保留一项上游编译期依赖的版本范围告警，源码核对及通告场景未复现结果见[工具链升级记录](archive/reviews/toolchain-upgrades-2026-10-29.md)。

升级验证结束时，main 与已打包 Build 35 保持升级前基线，尚未重新打包、启动或推送；升级随后收录于 Build 36。Flutter SDK / FRB 固定的间接依赖没有强行替换；GUI / 物理输入法、进程内存、跨平台原生运行与远端 Actions 未在该轮验收。

### 独立分支依赖升级 — 2026-10-29

`codex/dependency-upgrades` 从 `main@f1ed6d3` 合入 15 个 Dependabot 分支，包含 Rust、Flutter、插件 SDK、规范工具和 GitHub Actions；修复 SQLite 受检整数转换与可失败保护接口，同步 FRB 两端运行库及生成器至 2.13.0，并新增版本漂移检查。完整 `scripts/check.sh` 通过：Core 321 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471 / 1 跳过、SDK 25、JSON 30；真实 Rust 动态库构建及独立 Dart FFI 回归 1 项通过，覆盖普通 Flutter 套件跳过的显式动态库测试。范围与证据见[依赖升级记录](archive/reviews/dependency-upgrades-2026-10-29.md)。

该轮结束时升级保留在独立分支，未合入 main、推送远端或重新打包；源码版本仍为 `0.14.0+35`，当时已打包的 Build 35 对应升级前基线，后续升级及打包见上节。GUI / 物理输入法、整 App 内存、跨平台原生运行和远端 Actions 未在该轮验收。

### 当前修复重新打包与启动 — Build 35

`0.14.0+35` / macOS arm64 汇总此前 Flutter 长列表、动作选择器、页面释放及全仓跨层修复，保留 Impeller。本机持久签名、严格签名检查、ZIP 内主程序 / Rust 库 / 版本一致性及 SHA-256 校验通过；正常退出 Build 34 后启动明确路径的 Build 35，已核对实际运行版本。产物与摘要见[打包记录](benchmarks/package-v0.14-build35-2026-10-29.json)。

工程基线沿用下节全仓 `scripts/check.sh` 结果；此后只更新 Flutter / Native 构建号并复验版本，构建时重新编译 App 与 Rust Bridge。SDK 和 Web 示例已在工程门禁中重新构建，App 打包不会替换已安装插件，更新插件仍需独立导入对应包。启动确认不包含 GUI / 物理输入法、预览格式矩阵或整 App 内存验收。

### 全仓分层 Review 与跨层修复 — 2026-10-29 工作区

按 Core、Application、Features、Persistence、Platform、Bridge、Flutter、Native Host、插件 / SDK、构建及文档入口核对边界。修复 Web 排队取消等待 worker、隐藏期间失败订阅恢复后意外生效、后台回调异常误报成功写入失败，以及包内 TTF / OTF / ICO 被宿主拒绝；构建与打包统一校验实际输出路径，并在删除旧 App 前再次复验。

完整 `scripts/check.sh` 通过：Core 321 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471、SDK 25、JSON 30；Clippy、原生 Host、架构、生成物和静态分析通过。构建输出路径新增四组回归并接入门禁。范围、五项问题与复现证据见[本轮审查记录](archive/reviews/repo-review-2026-10-29.md)。

审查结束时尚未重新打包；应用侧改动随后收录于 Build 35。GUI / 物理输入法、整 App 内存和完整跨平台发行矩阵未验收。

### Flutter 动作选择与历史页面释放复查 — 2026-10-29 工作区

普通动作快捷键与 Context 规则共用按需构建的动作选择器，修复中文名称检索和本地化显示；匹配结果只在查询、目录或语言变化时更新。应用历史共享在途取消请求，保留新查询等待取消完成的顺序；页面销毁立即清空结果及对象动作，迟到响应不恢复已销毁数据。

新增 9 项回归，覆盖两类对话框、目录替换、语言切换、取消失败恢复及销毁后的迟到结果。58 项定向测试及完整 `scripts/check.sh` 通过：Flutter 470、Core 320 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略，静态分析无问题。合成操作计数与范围见[性能指南](performance.md)和[本轮记录](benchmarks/flutter-review-v0.14-2026-10-29.json)。

该轮先完成 Flutter 源码及文档，随后收录于 Build 35。原生 GUI、物理输入法、帧时间、Dart 堆 / GC 和整 App 内存未实测，保留 Impeller。

### Flutter 长列表与交互生命周期复查 — 当前工作区

文件结果使用固定行高布局，深处跳转不再从首行逐个创建组件；文件选择索引直接复用候选 ID，每份不可变 ViewState 缓存选择位置。快捷键应用选择器仅在查询或目录变化时筛选，方向键不再重复扫描；修复快速输入后立即确认选中旧匹配项、组合态方向键打断输入，以及目录更新后的选择错位。文件工作区销毁时撤销在途加载的通知资格，任务输出复制失败也不再访问已经卸载的页面。

新增 9 项回归，覆盖已复现缺陷、索引共享和目录替换场景；完整 `scripts/check.sh` 通过：Flutter 461、Core 320 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略，静态分析无问题。合成计数中，跳到第 9,000 行的组件构建从 9,000 降至 19，应用选择器两次方向键的标题读取从 8,027 降至 27；50,000 个文件索引不再另存带前缀的 ID 文本。计数与限制见[性能指南](performance.md)和[本轮记录](benchmarks/flutter-review-v0.14-2026-10-28.json)。

该轮结束时运行的 Build 34 不包含这些改动，随后已收录于 Build 35。物理输入法、原生 GUI、帧时间、Dart 堆 / GC 和整 App 内存未在该轮实测。

### 原生预览控制权与快速重开修复 — Build 34

Build 33 的原生文本编辑器先于宿主接管 Quick Look，日志反复记录 `preview_control_unavailable`，导致面板打开即关闭。展示时暂时把键盘交给 Flutter 内容接收视图，关闭后恢复仍属于原窗口的编辑器；主动关闭改为同步隐藏，避免旧关闭动画打断新预览。共享面板复用时重新连接数据源，其响应链控制器只弱引用宿主，避免保留旧通道与引擎。现行契约见[系统预览](native-preview.md)。

完整 `scripts/check.sh` 通过：Flutter 452、Core 320 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略；独立原生窗口回归通过文本输入竞争、立即关闭重开、对象更新、系统关闭、输入恢复与宿主释放。Build 34 的持久签名、ZIP 内容与 SHA-256 校验通过，已正常退出旧实例并核对新程序运行路径。

本机构建实测文件文档、剪贴板历史和独立悬浮窗的文本预览、关闭与后续输入；文件系统关闭按钮及预览重开也通过，新进程未再记录控制权错误。完整格式、物理输入法与进程内存矩阵未在本轮验收。具体观察与产物校验值见 [Build 34 记录](benchmarks/native-preview-v0.14-build34-2026-10-28.json)。

### Flutter 列表与刷新优化 — 当前工作区

文件列表复用不可变分块的隐藏项检查，剪贴板刷新保持一个在途读取并合并最新请求；旧结果、输入法组合态、查询防抖和显式选择规则保持不变。新增四项回归，合成操作计数见[性能指南](performance.md)。本轮 `scripts/check.sh` 全程通过：Flutter 452、Core 320 / 1 忽略、MVP 195 / 4 忽略、SDK 联合测试 23，静态分析无问题。未据此宣称实机帧时间、Dart 堆或整 App 内存改善。

### 内存副本与快照生命周期审查 — 当前工作区

剪贴板去重改为逐行借用 SQLite BLOB 核对，应用历史结果与执行校验表共享不可变快照，取消后释放服务引用与旧列表容量。增加共享所有权和取消后旧句柄失效回归；Bridge 的 DTO 形状保持不变。Rust 分配器合成测量及限制见[性能指南](performance.md)，不将局部堆字节下降当作整 App RSS 或图形内存下降。本轮 `scripts/check.sh` 全程通过：Core 320 / 1 忽略、MVP 195 / 4 忽略、Flutter 448、SDK 联合测试 23；Bridge、Clippy、原生 Host、架构与生成物检查通过。原生 GUI / 长期泄漏矩阵未在本轮验收。

### 匹配与剪贴板热路径优化 — 当前工作区

匹配借用原 Query，并按单次 Context 复用应用名前缀解析；剪贴板指纹在 SQLite 写事务前计算一次，查询 / 插入复用，重复条目保留实际字节比较。增加只读合成测量入口 `hot_path_measure`，优化前后二进制交替测量的结果和限制见[性能指南](performance.md)。本轮 `scripts/check.sh` 全程通过：Core 319 / 1 忽略、MVP 195 / 4 忽略、Flutter 448、SDK 联合测试 23；Clippy、原生 Host、架构与生成物检查通过。不包含整 App 时延、内存或原生 GUI 验收。

### 可读性与冗余清理 — 当前工作区

Launcher 的当前工具页统一使用一个状态名称，Registry 与页面栈各自明确命名；移除无生产调用的剪贴板提示状态查询及空销毁重写。QuickJS 的消息解析 / 调用额度 / 宿主耗时核算由独立的私有适配器承担，VM 配置与脚本求值保持独立；SDK 后台文本限制读取生成的共享协议参数。已完成的运行时迁移清单删除，现行契约集中到架构 / 插件开发指南，验收继续由本页及归档记录；路线图同步已交付状态。

本轮 `scripts/check.sh` 全程通过：Core 319 / 1 忽略、MVP 195 / 4 忽略、Flutter 448、SDK 联合测试 23；静态分析、Clippy、原生 Host、架构与生成物检查通过。文档检查覆盖 88 份 Markdown 与 686 个本地链接。未进行原生 GUI、物理输入法或资源验收。

### 插件故障隔离与 SDK 契约修复 — 当前工作区

损坏归档按包隔离，管理页显示不可用状态并保留卸载 / 回滚；健康包继续加载，显式启用损坏包失败。后台文本传输独立计数，在保留 128 次业务调用上限的同时，总计最多 1024 次宿主调用，允许 1 MiB 文本分块提交。默认 SDK 门面和公开类型已转发取消参数，补充预取消无写入、完整文本提交及坏包重启恢复回归。插件架构与开发指南已同步。本轮 `scripts/check.sh` 全程通过：Core 319 / 1 忽略、MVP 195 / 4 忽略、Flutter 448、SDK 联合测试 23；Bridge 绑定已重新生成并编译，后台 TypeScript 示例在无 DOM 类型下编译与实际 QuickJS conformance 通过。原生 GUI、内存与公开发行矩阵未在本轮验收。

### 架构与交互审查修复 — 当前工作区

原生预览确认事件校验资源对应的选择版本，避免旧面板执行新对象；预览准备每个展示会话只保留一个在途请求与最新待处理目标，过期资源释放后再继续。悬浮剪贴板关闭预览保留滚动位置。Launcher 的工具页栈由 `ToolPageStack` 独立管理父页面保留、返回和销毁；执行资格仍由 Controller 管理。架构与系统预览指南已同步资源及并发契约。

新增可控回归覆盖旧面板确认、选择往返、慢请求合并、关闭后重开、滚动保留及页面栈销毁 / 嵌套上限。本轮 `scripts/check.sh` 全程通过：Flutter 447、Core 316 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、SDK 21、JSON 30；静态分析、Clippy、原生 Host、架构与生成物检查通过。尚未重新构建签名包或进行实机原生焦点、物理输入法与内存验收。

### 预览列表显式选择 — Build 32

文件搜索、剪贴板历史与悬浮剪贴板进入 / 修改查询后默认不选中候选项；文件类型变化也清除选择。首次 ↑ 或 ↓ 选中第一项，再按空格预览。无选择时不执行对象动作，追加 / 刷新保留仍存在的手动选择，目标消失恢复未选择。普通 Action、应用历史与二级 Actions 保持默认选中行为。剪贴板修改查询时立即撤销旧预览目标，避免在途结果返回前预览旧条目。

完整检查通过：Flutter 441、Platform 70 / 1 忽略、Core 316 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30；静态分析无问题，持久签名与归档校验通过。新增回归检查未选择时无执行 / 预览、首项方向键与空格预览、查询 / 类型重置，以及刷新保留显式选择。Build 32 的最终实机首项预览验收尚未完成；不沿用 Build 30 的实机结果作为本构建通过证明。

### 已打开的 IDEA 项目切换 — Build 31

macOS 平台适配对实际 JetBrains IDE 安装使用官方命令行打开协议，完整传递经校验的安装 / 目标路径；不按窗口标题猜测，不使用 PATH 中的另一套启动器。其他应用与 Obsidian URL 路径保持原行为，Core / Bridge 接口不变。全量检查通过：Flutter 439、Platform 70 / 1 忽略、Core 316 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30；签名及归档校验通过。

IDEA 2026.2.3 实机先确认目标项目已经打开、当前为另一个项目，再从历史页依次打开两个不同目标，原生窗口均切到对应完整目录；首次请求前后的 IDEA 可执行进程集合相同。没有编辑项目源码；私人路径观察仅保留于被忽略的 build 目录。其他 JetBrains 产品、冷启动、多 Profile / 安装与完整跨平台矩阵未在本轮实测。详见 [Build 31 记录](benchmarks/idea-project-switch-v0.14-build31-2026-10-28.json)。

### 按需系统预览 — Build 30

文件搜索、剪贴板历史与悬浮窗改为单列列表，删除固定正文 / 图片预览。Rust 按需准备经校验的原文件或私有临时资源；Flutter 共用窗口、键盘与输入连接恢复；macOS Host 接入 Quick Look，Windows / Linux 仅预留接口。完整行为见 [系统预览](native-preview.md)。

全量检查通过：Flutter 439、Core 316 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30；FRB 已重新生成，Bridge 和原生 Host 编译检查、持久签名及归档校验通过。实机确认旧 Build 27 / 28 无预览：检查控制权早于面板展示；失败提示也因使用缺失的 SnackBar 容器而不可见。Build 29 已修复，Markdown、中文文本、PDF、PNG、↑↓ 跟随、Space / Esc / 系统关闭按钮保留查询通过；关闭后的输入连接又发现缺陷，已在 Build 30 修复并补充回归。Build 30 实机已通过 Esc / 系统关闭按钮后的继续输入、空格重开、⌘L 返回编辑并输入空格、剪贴板历史 / 悬浮窗的原生文本预览、查询保留和复制。文本预览时只有一个私有临时目录，关闭后回到零。物理输入法、独立 ⌘Y 复测、剪贴板图片 / 富文本 / 多文件等完整格式矩阵及内存仍未验收。记录见 [Build 30 验收记录](benchmarks/native-preview-v0.14-build30-2026-10-28.json)，旧失败包见 [Build 27 记录](benchmarks/native-preview-v0.14-build27-2026-10-28.json)。

### 安装授权后直接复制 — Build 25 / JSON 0.7.1

所有第三方插件的剪贴板读写按安装时授予的权限执行，不再逐次弹确认；未声明权限、失效文档和隐式 Context 解析器仍不能越权访问。三个 JSON 复制动作在 Rust 调度的后台运行，成功后关闭 Launcher，不创建 Workspace 或 Flutter 工具页。全量检查通过：Flutter 434、Core 319 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30；桥接生成、编译及签名打包通过。已启动 Build 25、安装 JSON 0.7.1，实机通过三个复制动作、实际剪贴板粘贴核对、转义输入重新匹配与 Workspace 内直接复制；大整数、重复键、指数及中文保留。详见 [Build 25 记录](benchmarks/direct-clipboard-v0.14-build25-2026-10-27.json)。

### 单一 JSON Context — 插件 0.7.0

解析器将普通 / 转义对象或数组统一解码后注册为 `plugin.dev.anywhere.json-workspace.json`，Context 保留 JSON 文本中的数字与重复键，不生成额外转义类型。Workspace 和三个复制动作共同接受该类型；删除 Unescape 入口、工作区菜单操作、路由及文案。Workspace 保留名称搜索空白打开。全量检查通过：Flutter 435、Core 317 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30。真实插件包回归验证统一类型、原文 token 与候选一致性；本机已更新 0.7.0，并核对转义候选、解码预填、菜单与后台授权预览。详见 [0.7.0 记录](benchmarks/json-context-v0.7.0-2026-10-27.json)。

### 插件重构收尾 — JSON 0.6.3

删除 JSON 动作中未使用的标题 / 标签元数据，展示仍由 Manifest 和中英资源负责；旧 Context 调研与逐构建过程已归档。规范、架构、生成物、真实插件包和全量检查通过：Flutter 435、Core 317 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30。主程序仍为已验收的 Build 24；0.6.3 仅清理元数据，未另做原生安装验收，业务路径以以下 0.6.2 记录为准。完整审查及产物校验值见[收尾记录](benchmarks/plugin-closeout-v0.14-2026-10-27.json)。这不替代完整来源矩阵、跨平台或正式发布验收。

### 验收修复 — Build 24 / JSON 0.6.2

工作区与后台复制统一绑定插件文档 Context，保留通用使用次数排序；候选副标题隐藏不透明数据载荷，插件安装说明覆盖界面 / 后台两种运行时。真实编译包回归验证唤醒识别、剪贴板过期、原文传入与默认排序。全量检查通过：Flutter 435、Core 317 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30。App 签名及 ZIP 校验通过。已切换至 Build 24 并安装 JSON 0.6.2，实机通过原文带入、精度保护 / JSONPath、后台压缩与转义复制、取消不写入、去转义，以及更新后的授权说明。IDEA / VS Code 历史列表、方向键 / 翻页、对象菜单与返回查询均通过；合成目录和保留两个根目录的工作区已在 VS Code 实际打开。物理输入唤醒 / 15 秒保留、设置保存失败时序、完整来源矩阵、内存与跨平台未在本轮重新实测。详见 [Build 24 记录](benchmarks/acceptance-fixes-v0.14-build24-2026-10-27.json)。

逐构建过程归档到 [Build 1–23 记录](archive/v0.14-development-records.md)。
