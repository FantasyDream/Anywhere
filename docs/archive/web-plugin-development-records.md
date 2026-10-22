# V0.11 Web 插件实施记录

历史基线与逐构建观察；不作为现行 API。当前契约见[Web 插件设计](../web-plugin-design.md)。

## 分阶段实施与退出条件

每步独立提交，先完成功能与单元 / 协议测试，再进行 UI 验收。每一阶段在文档中记录已完成 / 未验证内容，不用后续大阶段掩盖当前阻塞。

### 当前执行进度

2026-10-18：已开始 P0，独立探针与构建脚本（已退役，见[历史说明](web-plugin-p0.md)）接入锁定的 flutter_inappwebview 稳定版。已构建 Release App 并通过 deep / strict 签名；实际启动后的自定义协议页面完成 Unicode 回显、未开放能力拒绝，原生 frame / origin 经接收层验证。具体单元 / 导航回归和最新运行结果以探针记录为准。

P0 仍未整体验收：真实中文 IME / 保留快捷键、15 秒恢复、连续创建销毁的资源基线、完整网络 / iframe 攻击矩阵和 Windows 实机待验证。`connectBlocked` 不当作全面断网证据。正式 Launcher 尚无 Web 包安装入口，探针不接真实用户内容或 Rust 业务能力。

P1 包基础已实现：协议类型位于 `anywhere-protocol::web_plugin`，校验位于 `anywhere-plugin::web`。Manifest、稳定 `web:<pluginId>:<commandId>`、全部资源摘要、包限额、路径 / 链接 / ZIP 异常与图标校验新增 21 个回归测试。权限使用严格类型，未知字段 / 权限不会被默默接受；声明不授予访问能力。CLI 可检查 [无权限 Text Pad 示例](../../plugins/examples/web-text/README.md)，输出明确标记未安装、未批准。P0 的剩余验证仍是正式开放安装的前置条件。P2 已接入下述 Rust 主链路及 HostView 协议；以下保留 P0–P3 分阶段记录；P4–P6 的 macOS 本地开发预览实现与验收见文末，公开分发仍未开放。

```sh
cargo test -p anywhere-plugin --locked
cargo run -p anywhere-protocol --features web-schema --example generate_web_plugin_schema --locked -- --check
```

P1 阶段验证（2026-10-18）：`scripts/check.sh` 完整通过，Rust 508 项通过 / 6 项既有 ignored，Flutter 273 项通过；Clippy、架构边界、生成物漂移、文档链接与 macOS 原生检查均通过。另对开启 `web-schema` 的协议生成器运行 Clippy，通过。Text Pad 实际打包后经 CLI 读取，4 个资源 / 1,720 字节，稳定 Action ID 为 `web:dev.example.text-pad:open`；明确未安装、未批准。完整门禁日志为本机忽略文件 `build/web-plugin-p1-check.log`。本轮不改页面，未新增 UI 验收结论，P0 尚余事项仍保留。

#### P2 已实现的接入边界

- `application/web_plugins/catalog.rs` 只接收可信 Host 提交的已验证包与精确批准信息。核对 ID、摘要、权限和平台；发布前完整构造新 Registry，重复 ID / 动作冲突 / 来源变更失败保留旧目录与调用。最多 64 个包，只保留元数据，不常驻压缩包字节。
- Web 入口复用现有 `features::tools::EntrySource` 和 `core.launcher` 工具执行目标，与文件搜索 / 剪贴板一致；不新增 Scope 或常驻首屏按钮。Global 名称搜索、既有快捷键绑定、Current App 聚合与对象候选继续走原链。对象候选优先于同命令的空工作区；Web 不消费普通搜索文字，也不进入文本兜底。
- `ActionExecutor::WebCommand` 产生调用计划。Application 在创建 Invocation 前持有 Session 边界锁，再检查查询 / 目录 / 策略资格；排队的旧执行不能在隐藏或撤销后重新创建票据。`HostView::WebPlugin` 经 FRB DTO 只传 opaque Invocation ID；没有源码地址、磁盘路径或 Context 正文。
- `invocation.rs` 管理 Host 生成的随机调用 / Runtime 身份、文档 epoch 与目录注册资格。消息读取必须匹配实际 Host 绑定、主 frame、Runtime、文档 epoch 与精确 origin；这些元信息将由 P3 原生接收路径提供，不能取自网页消息正文。重载、替换页面、目录更新 / 撤销、来源策略或功能策略变更使旧资格失效。
- `projection.rs` 只投影当前动作的单个对象，`accepts` 与 `context.read` 分别检查。文本 / URL ≤ 1 MiB，文件集合 ≤ 128 项；超限返回明确 reason。文件只给名称、种类和随机句柄，路径只在 Rust 内持有；未知选择类型不通过同步 stat 猜测。无权限、无对象或不支持的类型不会退而给其他 Context。
- 当前应用是执行时从 Launcher 唤醒快照冻结的 ID / 名称，不包含 PID、进程身份、窗口标题或 AX 数据；须单独具有 `context.currentApp`。此阶段只提供可信 Rust Host 调用端口，尚未把任何系统能力暴露给 JS。
- 隐藏立即撤销调用资格，沿用原有 15 秒保留；恢复 / 重载须由 Host 重新绑定，不能复用旧文档请求。首次有效 ready 才记录一次入口 Usage，重载 / 恢复不重复。生产 `WebPluginSurface` / 页面工厂及 SDK 消息接线在 P3 / P4 完成前不开放包安装。

P2 验证（2026-10-18）：新增 10 个 Rust 目录 / 调用集成用例及 1 个桥转换用例。`scripts/check.sh` 完整通过：Rust 519 项通过 / 6 项既有 ignored，Flutter 273 项通过；Clippy、架构边界、生成物、文档和 macOS 原生检查通过。日志为本机忽略文件 `build/web-plugin-p2-check.log`。其中 15 秒边界使用受控时钟、业务输入为合成数据；不代表已完成真实 WebView、IME、安装权限或 Windows 验收。本轮未新增 UI 验收结论。

#### P3 已实现的 SDK / Rust 代理

- [SDK](../../packages/plugin-sdk/README.md) 是浏览器 ESM，运行时无 npm 依赖；只有具体方法，没有通用 Host invoke。请求类型、输入 DTO、错误码与配额从 Rust 生成 [协议 schema](../../resources/web-plugin-bridge.schema.json) 和 TypeScript contract，生成物漂移纳入完整检查。SDK 有 8 / 16 请求调度、AbortSignal 与 dispose，不自动重试复制或写入。
- `anywhere-plugin::web::wire` 只解析有界控制帧；Envelope 和方法参数严格检查未知 / 重复字段、嵌套与条目数，不信任 `pluginId`、`approved`、`userGesture`。Rust 从 Host 另行提供的文档身份判定权限；每文档最多 4,096 个请求 ID，防重放集合不无限增长。
- Application 的 `broker.rs` 规划明确能力，`rpc_dispatch.rs` 管理 blocking worker / 取消 / Host 确认，`rpc_state.rs` 和 `transfer.rs` 管理请求与一个有界文本缓冲。排队任务在执行前复验资格，读结果在交付前再次复验；隐藏 / 返回 / 重载 / 撤销取消旧文档请求。已开始的 OS I/O 不承诺可以撤回，错误也不承诺可安全重试。
- 剪贴板读取 / 写入分别检查声明与批准；请求只产生原生 `WebConfirmation`，不能序列化给页面当授权令牌。Host 决定是否批准，Rust 一次性消费已捕获的操作与正文；重复确认、取消或失效后确认均不能再次执行。测试宿主的确认仅作用于合成剪贴板，正式确认 UI 仍在 P4。
- `storage.local` 复用 `ModuleStorageHost`。Host 依据插件 ID + 来源身份绑定散列命名空间，不接受网页提供 namespace；配额与修改在同一事务内。Web 上限为值 1 MiB / 256 键，第一方仍保留原 256 KiB / 128 键。读写失败输出固定错误，SQLite 路径和底层堆栈不发给插件；更新同来源包可保留偏好，不同来源不继承。
- 文本 UTF-8 分块支持输入 / 剪贴板结果与复制；拒绝错序、超长、错误票据与过期传输。结果 JSON 另受编码后 2 MiB 约束，控制字符密集等内容超过该限制时返回 `TOO_LARGE`，不静默截断。当前实例最多一个缓冲；有 Native 确认待处理时不再分配第二个传输。弱引用维护任务每秒释放过期缓冲 / 确认，不需等下一次 RPC；重载 / 隐藏立即丢弃旧请求与缓冲，未 ready 文档最多 10 秒。

P3 验证（2026-10-18）：完整 `scripts/check.sh` 通过，Rust 532 项通过 / 6 项既有 ignored，Flutter 273 项通过，SDK 9 项通过，其中 3 项直接连接 Rust `web_sdk_conformance` 合成宿主，验证 Invocation / KV、Unicode 大文本往返及拒绝确认。新增 10 项 Rust broker 用例和 3 项 wire 用例，覆盖持久化、跨插件 / 来源隔离、配额回滚、隐藏后排队复制拒绝、定时释放及错误脱敏；日志为本机忽略文件 `build/web-plugin-p3-check.log`。开启 schema 特性的全部新代码另通过 Clippy。

本轮没有生产 WebView 或 GUI 验收结论；没有接真实系统剪贴板或用户内容。正式 FRB / 原生消息转发、Host 确认界面、主题与生命周期事件、Navigation / Actions、ToolPageShell 及焦点处理在 P4 接入；安装批准与持久存储工厂接线在 P5。P0 的完整网络 / frame / IME / Windows 验证仍是开放第三方安装的前置条件。

| 阶段 | 改动范围 | 必须通过的验收 |
|---|---|---|
| **P0 适配验证** | 独立开发探针，复用 flutter_inappwebview / 手工构建链；只使用合成数据，无用户内容能力 | release 包可启动；透明 / 不透明内容、缩放、中文 IME、真实键盘、⌘K / Esc、15 秒恢复、进程终止、主 frame 身份、资源 / CSP / 断网测试；确定实际支持的 OS / API 下限 |
| **P1 包与协议** | Manifest / SDK schema、包校验、稳定 ID、内容摘要；先无系统权限 | 路径穿越 / 链接 / ZIP bomb / 重复 ID / 版本不兼容 / JS 单独变化测试；校验失败零注册 |
| **P2 注册与调用** | Web 命令接 Action / Hotkey / HostView，Invocation 和 Context 投影 | Global / Current App / 对象动作沿现有逻辑；查询不执行 JS；旧候选、旧文档、错来源不能打开或读 Context |
| **P3 能力代理** | 最小 SDK、当前应用快照、主动读取 / 写入剪贴板、KV、文本分块、限额和撤销 | 伪造 pluginId、越权资源、跨插件存储、超限、超时、隐藏 / worker 前撤销均拒绝；用户手势不能自报，副作用不重复 |
| **P4 页面闭环** | ToolPageShell 集成、tokens、Actions、键盘 / IME、崩溃恢复 | 真键盘输入、复制 / 返回、查询选择恢复、浅 / 深色与白背景对比、无障碍标签；插件卡死仍可退出 |
| **P5 安装更新** | 设置入口、手工安装、批准、启停 / 撤销 / 卸载、回滚及开发重载 | 正在编辑时更新、App 中断安装、摘要变更、权限扩大、重载旧请求、连续开关；无 App 重启完成批准后的装载 |
| **P6 JSON 验证与收尾** | 用普通 Web 包实现 JSON 编辑 / 校验 / 格式化 / 复制，不添加 JSON 专用 Host API | 与一个不使用同框架的简易文本工具交叉验证 SDK；大文本错误与额度明确；全量门禁、资源基线、独立包构建和文档 |

P0 不能靠“页面显示成功”验收。网络 / Context 隔离或原生键盘有关键缺口时，停在开发模式，不面向用户开放第三方代码安装；协议、包校验等独立工作仍可继续。

macOS 先交付；Windows 从 P0 就保留接口与兼容矩阵，不以 Rust 编译通过代替 WebView2 实机验收。Web 包平台字段只有在对应平台真实跑通后才列入官方支持。

## 13. 下一版 MVP 与后续路线

**V0.11 的最小闭环：安装一个本地第三方 Web 包 → 授权 → 全局 / Context Action 搜索到它 → 打开统一二级页 → 得到最小输入 → 在网页操作 → 复制 / 保存偏好 → 返回 / 隐藏恢复 → 撤销或更新立即隔离旧实例。**

P0–P5 属于平台基础；P6 作为 JSON Web 工具验证切片跟进，在没有真实复杂工具验收前不宣称插件平台完整。JSON 不是 Core 特例，首个官方示例与第三方包使用同一权限和调用路径。

再根据真实工具需求加入 Host HTTP / Secrets / 用户选文件。后台执行与第三方 CandidateSource 单独提案，不为了 API 看起来完整一次性开放。

Terminal / PTY 延后，继续复用现有 Process Sessions；以后需要终端时单独设计输入 / 输出 / resize / 背压与执行授权，Web 页面可以复用本阶段的 surface / 主题 / 键盘基础。Marketplace、签名自动更新、Code Agents / ACP 不成为当前 MVP 的前置工程。

## 14. 仍待原型证明的事项

1. 最低 macOS 12 与实际 WebKit 对自定义 origin、ES modules、强制 CSP、网络通道阻断的组合行为。
2. 当前透明窗口内原生 WebView 的层叠、缩放、可访问性和输入法；平台层不应覆盖 Flutter 的 Actions 浮层，必要时通过隐藏 / 裁剪 native surface 处理。
3. 可复用的 Flutter WebView 封装是否覆盖关键安全 / 焦点 API，以及手工发布链的真实接入成本。
4. Windows 非持久 profile / 本地资源响应、WebView2 Runtime 缺失提示、输入焦点与 DPI 的实机矩阵。
5. 页面 ready / Usage 的时点调整，不破坏现有成功副作用记账；冷热延迟和包含 helper 的实际内存。

这些不是已发现并修复的代码缺陷，而是进入执行前的技术门槛。实现以 P0 测量作最后选型依据，不将研究中的可能性写成已交付承诺。


## P4–P6 本地开发预览（2026-10-18）

### 已实现代码

- `apps/flutter/lib/web_plugins/`：单一 InAppWebView、ToolPageSession / ToolPageShell、透明页面及共享排版、原生消息身份转发、Host 单次剪贴板确认、10 秒 ready 失败提示、崩溃提示、⌘K 重载 / 返回、Esc 与组合输入保护。原生 Edit 命令按 Flutter / WebKit / 原生文件选择器的实际目标分发。
- `anywhere-bridge-flutter::api::web`：只有可信 Flutter Host 能管理包、绑定、资源和确认票据；JS 仍只能使用关闭枚举的 broker 方法。随机 origin、main-frame、runtime / epoch 与内容摘要共同检查，错误不携带内部路径。
- `application/web_plugins/packages.rs` 协调批准与目录发布，`persistence/web_packages.rs` 隔离磁盘读写：先捕获 / 校验，再精确批准；批准不重开导入路径。摘要目录保存压缩包与展开资源，启动校验压缩包、请求读取时核对资源大小 / SHA-256；常驻元数据，不常驻 ZIP。最多 64 包；批准内容写入原子 receipt 后发布，失败恢复原目录；禁用 / 撤销立即失效，保留上一批准版本供回滚，卸载清理包及私有 KV。
- 15 秒隐藏仍保留同一个 WebView / 编辑内容；恢复只允许 Host 持有的旧绑定重新签发 runtime / epoch，并保留文档 origin。旧请求、旧确认和旧传输不能因恢复而获得资格。超时由既有 Launcher 销毁页面。
- Entry HTML 由 Host 在所有包标记前插入 CSP；仅允许包内脚本 / 样式 / 图片，禁止 connect、frame、worker、object、base 和表单；原生还阻止非初始导航、弹窗、下载、权限与 JS 对话框。WebRTC / WebTransport 构造器在包脚本前移除，HTTP(S) 内容规则拦截作为补充。以上是 WebKit / Host 策略，**不等于 OS 网络沙箱或完整的恶意代码隔离**。
- `plugins/examples/json-workspace/`：Preact + jsonc-parser，严格校验、token 保真格式化 / 压缩、双栏编辑、复制、错误位置、1 MiB / 128 层边界；正文不保存，缩进偏好使用 SDK KV。JSON 无专用 Rust 分支。Text Pad 使用无框架 JS，复用同一 SDK。

### 构建与使用

```sh
python3 scripts/build-web-examples.py
# build/web-examples/*.awplugin
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-0.11-preview-build12.app" scripts/build-macos.sh
```

设置 → Features → Web tools → Import，使用原生文件选择器，核对 ID / 版本 / SHA-256 / 权限后批准。每次本地更新均重新批准，无隐式增权，无 App 重启；⌘K 的重载只加载当前已批准版本，不监视任意开发目录。尚无签名发布者身份、自动更新 / 商店、Windows 生产适配、通用二进制通道或 Terminal。

### 验证记录

自动化与真实界面证据在本节后续记录；前述 P0–P3 是各阶段历史证据，不能用于代替生产 WebView 的验收。公开第三方安装 / 分发仍受 P0 完整安全与原生输入矩阵约束；当前界面明确标注本地开发预览。


### 本轮验收结果与未覆盖边界

- 自动化：Rust 工作区 538 项通过 / 6 项既有 ignored；SDK 9 项通过（3 项实际 Rust broker 互通）；JSON 5 项通过；Flutter 280 项通过（包含资源池 2 项、平台编辑页保留 2 项补测）。完整门禁以 `build/web-plugin-final-check.log` 为准。
- 实际 V0.11 / Build 12：原生文件选择、摘要 / 权限检查、批准后 Action 可搜索、重启后仍可搜索、打开 Web 工具、系统粘贴与 `⌘↵` 格式化已观察成功。合成 JSON 的 21 位整数原样出现在格式化输出。
- 复现并修正了 Impeller 下 Flutter physical-shape 裁剪遮住 AppKitView 的黑屏；平台视图页面改由原生窗口裁剪，普通页面继续原有裁剪。桥脚本在原生 listener 之后注入，避免初始化顺序依赖。
- GUI 使用相同构建资源的最小权限验收变体（`dev.anywhere.json-validation`，只有 copy / KV），没有为验证增加真实系统剪贴板读取权限。标准 JSON 示例保留显式 Paste 请求，真实读取未做 GUI 验证；合成 broker 测试已覆盖读 / 写确认。
- 首次连续验收被系统锁屏中断。2026-10-19 重试已跑通键盘复制 → Host 批准 → 页面显示 Copied output、⌘K 面板、设置快速返回保留正文及 Esc 恢复查询。随后定位并修复鼠标点击被前景边框阻断，最新构建的鼠标格式化 / Use as input / 复制确认和 Esc 取消已观察成功。详见下面补充记录；不等于完整 P4 发行验收通过。
- Windows、真实中文 IME / VoiceOver、完整网络 / frame 攻击矩阵、持续 30 次打开 / 返回及全进程树内存基线仍未完成。当前桌面 AX 树没有暴露 Web 正文，VoiceOver 是公开发行前的明确待办。不能据此宣布完整第三方安全平台已验收。


### 2026-10-19 生命周期收尾

- Settings 复用 Launcher 引擎时，平台编辑页留在稳定的 Offstage 槽位；不会因打开设置提前销毁。恢复继续使用同一个编辑器；15 秒过期仍销毁页面，不重建设置草稿。两项 Flutter 回归分别覆盖保留正文 / 查询和设置打开期间超时销毁。
- Native 页面延迟关闭改为在 Rust 服务锁内核对文档 epoch。重载 / 恢复后，旧绑定的销毁通知不能删除新文档；当前隐藏绑定仍可以正常销毁。新增确定性 Rust 测试覆盖这三条路径。
- 完整检查、最新 App 构建和 ZIP 打包日志分别为 `build/web-plugin-final-check.log`、`build/web-plugin-final-build.log`、`build/web-plugin-final-package.log`。2026-10-19 最初尝试时系统锁定，后续用户解锁后的新增 GUI 结果见下节。


### 2026-10-19 解锁后 GUI 重试

- 在同一 Build 12 的本地验证包中，合成 JSON 输入、键盘格式化、鼠标格式化 / Use as input 均成功。键盘复制请求进入 Host 确认，点击 Allow once 后页面显示 Copied output；鼠标 Copy output 也进入同一个 Host 确认，Esc 取消后页面显示 USER_CANCELLED。
- ⌘K 打开宿主 Actions，Esc 关闭；从设置快速关闭并唤醒时，原 JSON 正文 / 输出仍保留，返回搜索恢复 JSON Workspace 查询。自动焦点未恢复的问题已补页面加载、恢复和窗口 presentation 后的焦点同步，后续验证再次被系统锁定中断，尚未实机复验。
- 鼠标缺陷根因：平台页面的 Material 前景边框生成全尺寸 Flutter overlay，原生 hitTest 返回 FlutterView，WebKit 没有收到事件。平台页改为 borderOnForeground=false 后，按钮实际响应；保留原生窗口裁剪。临时点击 / 视图树诊断已删除，未保留无效的 EagerGestureRecognizer 尝试。
- 排查对照了 Flutter 3.47.4 的 [原生命中排除区域实现](https://github.com/flutter/flutter/blob/3.47.4/engine/src/flutter/shell/platform/darwin/macos/framework/Source/FlutterMutatorView.mm)。此记录以本机复现及修改后的实际按钮响应为依据，不把所有 Flutter / WebView 组合都归因为同一个问题。
- 最新 Flutter analyze 无问题、280 项测试通过；点击修复构建记录为 `build/web-click-build.log`，全量检查和重新打包记录为 `build/web-plugin-final-check.log` 与 `build/web-plugin-final-package.log`。当时独立隐藏后 15 秒边界、设置返回后的键盘继续输入尚待验收；后续结果见下面的连续生命周期补验。实际中文 IME / VoiceOver、攻击矩阵与内存循环仍是待验收项。


### 2026-10-19 连续生命周期补验

- 同一已签名 Build 12，设置往返 2,377 ms 后保留原文稿 / 输出，直接键入空格和 ⌘↵ 格式化成功；这次确认了恢复后的自动焦点。
- Universal Search 菜单明确执行隐藏，761 ms 内唤醒后继续编辑成功。恢复后的 SDK 复制仍经 Host 确认，Right → Enter 批准后页面显示 Copied output。
- 菜单隐藏后等待超过 15 秒，实际观察间隔 48,730 ms；唤醒得到空查询的新 Current App 页面，再打开 JSON 的输入 / 输出均为 0 bytes。精确 15 秒边界由受控时钟测试证明，不从此 GUI 样本推断精确销毁时间。
- ⌘K → Enter 重载使页面回到空白且编辑器获得焦点；Esc 恢复原 JSON Workspace 查询。
- 验收工具读取隐藏应用的 AX 状态会触发应用 reopen，不能先读取 Anywhere 再等待过期。Finder 的后台按键 / 点击尝试没有可靠的失焦隐藏证据，未计为真实点击桌面的验收；本轮改用菜单明确隐藏，并在等待期间完全不读取 Anywhere。
- 本轮未改业务代码，不重复构建或全量单测；现有全量门禁仍是 Rust 538 / Flutter 280 / SDK 9 / JSON 5 通过，文档链接检查另行通过。当前仍是本地开发预览，Windows、实际中文 IME / VoiceOver、完整攻击矩阵和全进程树内存循环未验收。


### 连续资源采样进度

生命周期补验通过后，退出 / 重启本地 Build 12 记录已有 WebKit PID，并按新增 PID 采样（XPC 的 PPID 为 launchd）。主 App 与本轮新增 WebKit helper 的 total footprint：冷搜索页 128.80 MiB，打开 JSON 192.11 MiB，返回 5 次后 152.58 MiB，返回 10 次后 152.77 MiB。打开时 GPU / WebContent / Networking 各一，返回采样时只剩一个 GPU helper。原始数据为 `build/web-cycle-samples.jsonl`，各阶段 footprint JSON 同目录保存。

完成 10 次打开 / 合成文本输入 / 返回并校验原查询后，下一批第 11–15 次操作被系统锁定中断。没有据此宣称 30 次验收完成或完全排除泄漏；这里只能说明已完成样本中未见 WebContent 数量累积。大型文档、长时间运行及最坏峰值没有被此小文本样本覆盖。


### 资源循环补验完成

解锁后保持同一 App PID 14465，先补完至累计 30 次，随后额外执行不间断 30 次（单批交互脚本 34,175 ms），累计确认 60 次。每次均确认 Web 页面打开、Esc 返回原查询；合成文本与格式化按键按相同步骤发送，没有逐次视觉校验格式化输出。该进程日志中 Action 执行开始共 61 次，对应一次预热和 60 次循环。

20 / 30 / 60 次返回后的 App + 新增 WebKit helper 合计 footprint 分别为 136.31 / 145.80 / 143.61 MiB；采样中仅保留同一个 GPU helper，没有累积 WebContent 或 Networking。最后一组满足不间断 30 次的循环项，但不代表长时 / 大文稿 / 最大负载均已证明无泄漏。跨锁屏阶段的数值下降受系统压缩与回收影响，不能归因为代码优化。本轮没有改业务代码，新增实机证据已同步 V0.11 MVP。

## Build 13：公共样式与 GPU helper 生命周期

Flutter AppStyle 与 Web 插件改为从 `resources/design-tokens.json` 生成同一套颜色 / 透明度、字号 / 字重、间距、圆角和控件参数。Host 在插件资源之前注入主题；SDK 的可选 `styles/controls.css` 提供透明按钮、卡片、输入框、键帽与交互状态，JSON / Text Pad 复用它。玻璃仍由原生窗口提供，不增加 Web 模糊层。当前深色主题，无动态主题 API；第三方可自定义 CSS，因此不能强制其界面完全一致。

关闭工具会销毁其 WebView / 文档资格，不直接终止应用共享的 GPU helper。WebKit 的 GPUProcessProxy 复用现有图形进程，并有临时保留机制；当前上游源码含一分钟保留窗口，但不能将其当作本机 OS 版本的固定退出时限。创建独立 WKProcessPool 也不能保证独占进程；Apple 已说明此属性不再影响进程分配。[WebKit 实现](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/GPU/GPUProcessProxy.cpp)、[Apple 文档](https://developer.apple.com/documentation/webkit/wkwebviewconfiguration/processpool)。

此前 Build 12 的本机证据：60 次开关只保留同一个 GPU helper，正常退出主 App 后该 helper 消失。该证据不扩展为所有系统版本或长期运行的无泄漏保证；Build 13 的样式变更不加入主动杀 WebKit 进程的逻辑。

画布透明度由 Host 的平台适配结果控制，默认使用公共深色底色，防止默认白画布与白字冲突。macOS 的 WebView 库 1.1.2 仅清除 scroll-bounce 的 `underPageBackgroundColor`；当前另以存在性检查保护 WebKit 的 `drawsBackground` SPI，并隔离在 `NativeWebAppearance`，不修改 pub 缓存或 Rust。该兼容路径不是公开 Apple API，后续 SDK / OS 升级必须重新验收；接口不可用时保留可读的深色回退。原生测试覆盖真实 WKWebView 透明开关、焦点与接口缺失分支。

### 透明 WebView 的背景层次

```text
原生窗口玻璃（唯一模糊层）
  └─ Flutter：统一的窗口底色 / ToolPageShell
       └─ WebView：关闭原生不透明画布
            └─ HTML / body：透明
                 └─ 按钮、输入框、卡片：共享参数的局部半透明表面
```

原生画布配置通过单独的 `configureWebAppearance` 通道返回实际支持情况，不依赖窗口是否为 key，也不改变 first responder。编辑焦点仍走单独的 `focusWeb`，并保留 key-window 检查。macOS 从本窗口的 content root 查找 WKWebView，兼容 Flutter 将平台视图放在渲染视图旁边的结构；不遍历其他窗口。Dart 检查页面代数、可见性和 Host 弹层后再更新 `--aw-web-canvas`，过时回调不能改新页面。支持时复用 Flutter 背景，不支持时保持共享深色回退，不让白字落在默认白画布上。

Build 14 将画布配置从焦点恢复回调中移出：加载完成后直接设置原生背景与 `--aw-web-canvas`。Flutter 空闲时，post-frame 回调不保证被及时调用；编辑焦点恢复现在显式请求帧。原生视图是否存在与透明支持布尔值进入不含内容的诊断日志，便于区分不支持、视图尚未附着和 CSS 覆盖。JSON 卡片内的编辑器保持透明，避免两层半透明色叠加。

SDK 6.1.5 / macOS 1.1.2 的创建与加载回调可持有不同 Dart facade；判断同一 WebView 要比较底层 `platform` 对象。该行为有使用真实 SDK facade 的回归测试，不依赖同一个 Dart 包装对象恰好被复用。

Build 14 实机已确认原生透明配置生效（`web_view_found=true`、`transparent=true`），正文透出同一 Flutter / 原生玻璃背景；输入、键盘格式化、鼠标 Use as input、Esc 保留查询和重新打开通过。详细证据范围见 V0.11 MVP 的 Build 14 实机补验。

## Host 背景适配的模块边界（Build 15）

| 模块 | 职责 |
|---|---|
| `web_document_theme.dart` | 注入公共参数、根画布与默认文字规则，生成背景应用脚本 |
| `web_document_policy.dart` | CSP / 来源 / 资源限制；将框架主题加入受保护文档 |
| `web_view_appearance.dart` | 查询平台透明能力、统一回退、异步有效性检查、SDK facade 身份 |
| `web_tool_surface.dart` | 在加载与恢复时调用背景模块，提供当前页面生命周期条件 |
| `NativeWebAppearance.swift` | 隔离 macOS WebKit 透明兼容；不介入键盘焦点或插件业务 |
| SDK `styles/controls.css` | 可选公共控件；卡片内编辑器单层底色，与根画布适配独立 |

背景是所有 Web 工具的默认 Host 能力，不是插件权限或插件 API。插件无需导入背景代码 / CSS，普通 HTML 与常见框架挂载根自动处理。html / body / `#app` / `#root` / `#__next` 的画布与默认文字归 Host，普通内部组件仍保留各自样式。加载时以 Host 样式和内联 important 声明确保前端模板背景不遮住宿主；这不是阻止恶意脚本修改 DOM 的安全边界。恢复已有页面也重新应用背景，不依赖再次触发 load 事件。

使用固定数量根节点选择器，不遍历全部 DOM，不增加 observer、后台任务或持续轮询；不读取插件文本内容。所有原生结果在修改文档前重新检查可见性 / owner / generation / 平台视图身份。

Build 15 已通过无公共控件 CSS、根节点自带 inline important 白底的普通 HTML 实机验证，以及无背景业务代码的 JSON / Preact 页面和设置往返恢复验证；回归包只用于测试，验收后已卸载。详见 V0.11 MVP 的 Build 15 验证。


### Build 17：原生回复保真修复（2026-10-19）

JSON 自动匹配后空白输入在生产 WebView 中复现。已提交候选和 Rust 投影保留原文，但固定版本 macOS 依赖的字符串回复经过 JavaScript 字面量插值时丢失反斜杠，网页 JSON 响应解码失败并超时。宿主增加 `web_message_transport.dart`，统一将控制回复编码为受限 UTF-8 `ArrayBuffer`；`webBootstrap` 校验字节预算并严格解码，SDK 接口仍接收字符串帧，插件无需修改。此适配不开放通用二进制 API。

通过实际启动脚本与 SDK 的自动化回归覆盖转义 / Unicode / 存储值 / 错误 / 非法帧 / 隐藏恢复；Flutter 另验证原生 WebMessage 使用二进制及 64 KiB 的 UTF-8 预算。完整工程检查通过；Build 17 使用已安装 JSON 0.1.1 实机验证自动填入 108 bytes、格式化输出 129 bytes 成功。细节见 [V0.11 MVP](mvp-v0.11.md)。


### SDK 与独立插件工程解耦（2026-10-19）

- JSON 示例业务源码改用 `@anywhere/plugin-sdk` 与公开 CSS 导入，不再跨目录引用仓库中的编译文件；仓库开发的 npm `file:` 连接只属于构建装配。
- SDK 可导出为 `.tgz`，只包含浏览器产物 / 类型、公共 CSS、最小工程模板和开发用打包器。第三方工程将 SDK 安装包保存在自己的 `vendor/`，无需依赖 Anywhere 的目录、Flutter / Rust 源码或运行时 Node。
- `tools/package.py` 是不依赖仓库的开发工具：只打包明确选定的运行资源，按块写入、限制文件数量与预算、拒绝符号链接，并在失败时保留旧输出。完整 Manifest / 资源验证和批准仍由 Host 负责，没有第二套授权实现。仓库示例也使用同一打包器。
- `scripts/check-plugin-starter.sh` 从导出的 SDK 在仓库外新建工程，按开发指南安装 vendor 依赖、构建与打包，并用真实 Host 验证器检查产物；该检查加入完整门禁。插件作者的操作步骤见 [开发指南](../plugin-development.md)。
