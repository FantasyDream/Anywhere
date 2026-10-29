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

### 独立分支依赖升级 — 2026-10-29

`codex/dependency-upgrades` 从 `main@f1ed6d3` 合入 15 个 Dependabot 分支，包含 Rust、Flutter、插件 SDK、规范工具和 GitHub Actions；修复 SQLite 受检整数转换与可失败保护接口，同步 FRB 两端运行库及生成器至 2.13.0，并新增版本漂移检查。完整 `scripts/check.sh` 通过：Core 321 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471 / 1 跳过、SDK 25、JSON 30；真实 Rust 动态库构建及独立 Dart FFI 回归 1 项通过，覆盖普通 Flutter 套件跳过的显式动态库测试。范围与证据见[依赖升级记录](archive/reviews/dependency-upgrades-2026-10-29.md)。

升级保留在独立分支，未合入 main、推送远端或重新打包；源码版本仍为 `0.14.0+35`，已打包的 Build 35 对应升级前基线。GUI / 物理输入法、整 App 内存、跨平台原生运行和远端 Actions 未在本轮验收。

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
