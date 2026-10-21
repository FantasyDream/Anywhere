# V0.11：本地 Web 工具预览

日期：2026-10-21。开发构建：`0.11.0+51`，既有功能基线为 V0.10 / Build 11。**当前是 macOS 本地开发预览，不代表完整第三方平台已通过发行验收。**

## 已实现

- `.awplugin` 受限 ZIP / Manifest / 资源 / PNG 校验，稳定的 `web:<id>:<command>` 身份。
- 显式 Context 输入、冻结的 Current App、统一 Action / Scope / Ranking / Usage；普通搜索词不自动成为插件输入。可选 `inputMatchers` 由宿主识别内容，JSON 对象 / 数组可自动匹配插件 Action，并沿用选中文本 / 15 秒剪贴板填入规则。
- 原生 WebMessageListener → Flutter → FRB → Rust broker；main-frame / origin / runtime / epoch / grant / digest 检查。JS 不持有审批或文档身份令牌。
- TypeScript SDK 的 ready、Invocation、当前应用、剪贴板单次确认、私有 KV、分块、取消、超时与配额；没有通用 invoke、Shell 或任意文件 / 网络接口。
- ToolPageShell 中惰性创建一个 WebView，复用原 Launcher 的返回状态。15 秒保留与撤权 / 重绑定有受控时钟测试；打开设置时保留隐藏的页面，超时销毁。旧页面延迟销毁不能关闭重载或恢复后的新文档。
- Host Actions 提供重载与返回，Esc / ⌘K 由宿主管理；组合输入不抢 Esc。WebKit、Flutter、原生文件选择器分别使用正确的编辑命令路由。
- Settings → Features → Web tools：原生文件选择、版本 / 摘要 / 权限检查、精确内容批准、启停 / 撤销、卸载、更新、上一批准版本回滚。启动校验安装包，资源读取重新核对摘要，私有 KV 单独落盘。
- 资源转发共享 2 个读取槽与 16 个等候名额；页面重载不放大并发缓冲。Web 命令的查询不执行 JS，不创建 WebView。
- 新 JSON Workspace：Preact + jsonc-parser + jsonpath-js + 同一 SDK，左侧自动格式化 / JSONPath 与 JavaScript 计算 / 固定公式前缀 / 右侧结果复制 / 错误位置 / 缩进偏好；输入与输出各 1 MiB、最大 128 层。不保存正文，不引入 JSON 专用 Rust 业务 API。
- 无框架 Text Pad 示例用于 SDK 交叉验证，第一方功能仍静态编译，不恢复原生热加载。

## 使用与产物

```sh
python3 scripts/build-web-examples.py
scripts/check.sh
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-0.11-preview-build51.app" scripts/build-macos.sh
scripts/package-macos.sh build/Anywhere-0.11-preview-build51.app
```

导入 `build/web-examples/dev.anywhere.json-workspace.awplugin` 后，在 Global Search 搜索 JSON Workspace。左侧输入自动格式化，上方默认使用 JavaScript 公式，也可切换 JSONPath，右侧展示结果。`⌘↵` 立即查询，`⌘⇧C` 请求复制当前结果；剪贴板读 / 写需要各自权限及每次 Host 确认。旧 `core.json.*` 身份和原生工作区不复用。

## 当前验收

| 项目 | 结果 |
|---|---|
| Rust 工作区 | 557 通过，6 项既有 ignored |
| Flutter | 333 通过，analyze 无问题 |
| SDK | 16 通过，包含实际 Rust broker 互通与文档传输回归 |
| JSON | 24 通过：格式化 / 精度 / 转义、JSONPath / JavaScript、预算 / 隔离、光标与文本复用 |
| 原生 / 架构 / 文档 / 生成物 / Clippy | 完整门禁通过 |
| macOS ARM64 App / ZIP | 已构建并完成 deep / strict 签名与包版本检查 |
| 实际桌面观察 | 文件选择、检查 / 批准、Action 搜索、重启保留批准、Web 页面绘制、系统粘贴、鼠标 / ⌘↵ 格式化、大整数保真、一次复制批准成功、Esc 取消、⌘K 面板、设置返回保留正文及输入焦点、隐藏后快速恢复、超时清空、键盘确认、重载、Esc 恢复查询 |

早期 GUI 使用同一页面资源的最小权限验证变体，仅有 copy / KV；后续 Build 17 与 JSON 0.2.0 使用已安装的正式示例包，更新保持原有权限。各轮范围以下方记录为准。详细过程和本地日志见 [设计与实施记录](web-plugin-design.md)。

## 尚未完成的发行验收

- 真实鼠标点击桌面引起的失焦隐藏，尚未独立确认；本轮通过原生菜单明确隐藏验证了恢复及过期。桌面工具读取隐藏的 Anywhere 时会重新唤醒，测试中需避免读取动作重置待测状态。精确 15 秒边界由受控时钟测试覆盖，GUI 分别验证界限内与界限外。
- 实际中文 IME、VoiceOver；当前桌面 AX 树未暴露 Web 正文，需要继续确认辅助功能接入。
- 完整的 CSP / 网络 / frame / 对话框攻击矩阵。现有 WebKit / Host 策略不等于 OS 进程或网络沙箱。
- 长时间 / 大文稿 / 最大负载的资源验证尚未覆盖。已完成同一进程累计 60 次打开 / 输入 / 返回，其中最后 30 次为不间断的一组；详情和限制见下文。
- Windows、公开分发 / 签名发布者、自动更新、开发服务器 / HMR、通用二进制通道；Terminal 继续延期。

这些项目未验收前，不将本版标记为完整插件平台完成。继续开发顺序是：先完成原生交互与隔离验收，再扩大可安装来源和 Host capabilities。


## 2026-10-19 连续生命周期补验

- 设置往返 2,377 ms，原文稿 / 输出保留，直接键入空格后输入字节数增加，⌘↵ 仍能格式化，无需先点击输入框。
- 原生 Universal Search 菜单明确隐藏后，761 ms 内恢复，继续键入 / 格式化成功。恢复后的复制请求可弹出 Host 确认，右方向键选择 Allow once、Enter 批准后显示 Copied output。
- 同一路径隐藏后超过 15 秒再唤醒（观察间隔 48,730 ms）：旧 Web 页面和查询清空，回到新 Current App 会话。再次打开 JSON，输入 / 输出均为 0 bytes。GUI 不用于宣称 15,000 ms 的精确时点，精确边界以单元测试为准。
- ⌘K → Enter 重载后，页面清空并重新获得编辑器焦点；Esc 仍恢复 JSON Workspace 查询。本轮未改业务代码，仅更新验收记录。


## 连续资源采样（已完成循环项）

先退出验收实例记录已有 WebKit PID，再重启，按新增 PID 区分本轮 helper 与其他软件的 WebKit。macOS XPC 进程的父进程是 launchd，因此使用 PID 差分而非仅按 PPID 遍历。采用 footprint 的 total footprint（包括压缩内存等），不与 RSS 混用。

| 状态 | App + 新增 WebKit helper 合计 footprint | 新增 helper |
|---|---:|---|
| 冷启动搜索页 | 128.80 MiB | 无 |
| JSON 工具打开 | 192.11 MiB | GPU、WebContent、Networking 各 1 |
| 5 次打开 / 输入 / 返回后 | 152.58 MiB | GPU 1 |
| 10 次打开 / 输入 / 返回后 | 152.77 MiB | GPU 1 |
| 解锁后继续，同一进程 20 次后 | 136.31 MiB | GPU 1 |
| 同一进程 30 次后 | 145.80 MiB | GPU 1 |
| 额外不间断 30 次，累计 60 次后 | 143.61 MiB | GPU 1 |

首次 10 次后发生锁屏，之后在同一进程（PID 14465）补完 20 次，再额外执行一整组不间断 30 次，交互脚本耗时 34,175 ms。每次通过 AX 确认 Web 页面已打开、退出后存在原查询；发送合成 JSON 和格式化按键，不把每次输出都算作单独视觉校验。进程日志中该 Action 共 61 次执行开始，对应一次预热与 60 次确认循环；锁屏批次不计为额外成功次数。

返回采样时没有 WebContent / Networking 累积，保留一个相同 PID 的 GPU helper。footprint 未呈持续上升趋势，但样本受锁屏、压缩与系统回收影响，不能将跨阶段下降解释为代码优化收益或完全排除泄漏。本轮输入为小型合成 JSON，不能推断大型工具、长期运行或最大文本负载上限。原始日志在本机忽略文件 `build/web-cycle-samples.jsonl` 与 `build/web-cycles-*.json`。


## 完全退出后的释放检查

正常点击 Quit Anywhere 后，原主进程 PID 14465 和本轮 GPU helper PID 14609 均已消失；按启动前 PID 基线比较，没有遗留本轮新增的 WebKit 进程。其他软件原有的七个 WebKit PID 仍在，不计为 Anywhere 残留。检查未重新读取 Anywhere 的 AX 状态，以免桌面工具自动重启应用。记录为本机忽略文件 `build/web-after-quit-validation.json`。

这与“返回搜索”不同：返回搜索后 WebContent / Networking 消失，主 App 和 GPU helper 可继续驻留（最后采样合计 143.61 MiB）；隐藏 15 秒内还会按产品规则保留编辑状态。上述退出检查证明进程已结束，不将系统文件缓存或驱动内存的全机变化归为本 App 的精确回收量。

## Build 13：公共样式与 GPU helper 生命周期

完整 `scripts/check.sh` 通过：Rust 538 通过 / 6 项既有 ignored，Flutter 281 通过且 analyze 无问题，SDK 9 通过，JSON 5 通过；新增设计参数 Python 检查 4 通过。原生、Clippy、架构、文档与生成文件同步检查通过。

Flutter AppStyle 与 Web 插件改为从 `resources/design-tokens.json` 生成同一套颜色 / 透明度、字号 / 字重、间距、圆角和控件参数。Host 在插件资源之前注入主题；SDK 的可选 `styles/controls.css` 提供透明按钮、卡片、输入框、键帽与交互状态，JSON / Text Pad 复用它。玻璃仍由原生窗口提供，不增加 Web 模糊层。当前深色主题，无动态主题 API；第三方可自定义 CSS，因此不能强制其界面完全一致。

关闭工具会销毁其 WebView / 文档资格，不直接终止应用共享的 GPU helper。WebKit 的 GPUProcessProxy 复用现有图形进程，并有临时保留机制；当前上游源码含一分钟保留窗口，但不能将其当作本机 OS 版本的固定退出时限。创建独立 WKProcessPool 也不能保证独占进程；Apple 已说明此属性不再影响进程分配。[WebKit 实现](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/GPU/GPUProcessProxy.cpp)、[Apple 文档](https://developer.apple.com/documentation/webkit/wkwebviewconfiguration/processpool)。

此前 Build 12 的本机证据：60 次开关只保留同一个 GPU helper，正常退出主 App 后该 helper 消失。该证据不扩展为所有系统版本或长期运行的无泄漏保证；Build 13 的样式变更不加入主动杀 WebKit 进程的逻辑。

画布透明度由 Host 的平台适配结果控制，默认使用公共深色底色，防止默认白画布与白字冲突。macOS 的 WebView 库 1.1.2 仅清除 scroll-bounce 的 `underPageBackgroundColor`；当前另以存在性检查保护 WebKit 的 `drawsBackground` SPI，并隔离在 `NativeWebAppearance`，不修改 pub 缓存或 Rust。该兼容路径不是公开 Apple API，后续 SDK / OS 升级必须重新验收；接口不可用时保留可读的深色回退。原生测试覆盖真实 WKWebView 透明开关、焦点与接口缺失分支。

## Build 14：修正正文仍使用不透明底色

此前透明度配置与编辑焦点共用 post-frame 恢复路径。WebKit 的加载完成回调可在 Flutter 空闲时到达，而 `addPostFrameCallback` 本身不会申请下一帧；因而不能依赖该回调及时配置页面。

现在在 `onLoadStop` 直接配置已加载 WebView 的原生画布与 HTML / body 底色，独立于焦点恢复。焦点恢复显式申请帧，保留可见性、页面代数和 Host 弹层检查。仅记录是否找到原生视图及透明支持布尔值，不记录页面或输入内容。JSON 编辑器不再在半透明卡片上叠加第二层输入区底色。实际背景与操作仍需以 Build 14 的实机结果为准。

补充修正：固定 SDK 为创建事件与加载事件生成不同 Dart facade 的情况。视图身份比较底层 `platform` 对象，不能比较包装对象引用。新增回归测试用实际 SDK 包装同一 platform，确认新 facade 仍可配置；同 id 的另一 platform 不被接受。Flutter 282 项回归通过，analyze 无问题。

### Build 14 实机补验

在原生更新对话框中检查并更新最小权限 JSON 验证包（仅复制确认与私有 KV，无剪贴板读取权限）；随后退出旧进程，启动准确路径 `build/Anywhere-0.11-preview-build14.app`。

- 日志出现 `web_canvas_configured`，`web_view_found=true`、`transparent=true`。
- 截图确认工具正文外层与标题 / 底栏连续使用同一宿主玻璃底色；不再有完整的深色或白色 Web 画布。卡片保留单层局部半透明底色。
- 合成 JSON 输入、`⌘↵` 格式化成功；21 位整数原样保留。
- 鼠标点击 Use as input 生效：格式化输出移入输入，输出清空，未被透明图层拦截。
- Esc 返回后截图确认保留原查询 `JSON Workspace`；重新打开后再次记录透明画布已配置。

Flutter 282 通过且 analyze 无问题；原生背景 / 焦点 / 不支持回退检查、设计参数 Python 4 项、JSON 5 项、示例包构建通过。App 与 ZIP 已使用持久身份签名并进行归档版本与关键文件一致性验证。本次颜色修正未重新执行此前 60 次内存循环，不扩大其结论。

## Build 15：背景适配成为插件框架基础能力

- `web_document_theme.dart`：统一注入基础参数与根画布规则，配置 html / body / 常见挂载根的背景与默认文字颜色，不依赖插件 SDK 或 CSS 类。
- `WebViewAppearance`：统一协调平台透明能力、深色回退与文档样式应用，处理原生接口不可用和视图销毁。SDK facade 身份判断归入该模块。
- `WebToolSurface`：只负责加载 / 恢复时调用框架模块及页面可见性 / 代数校验，不包含背景 JS 或插件特例。
- `NativeWebAppearance`：保留 macOS 平台细节；Rust 不增加主题或 JSON 专用接口。
- SDK 公共控件处理卡片内编辑器的单层底色；JSON 和 Text Pad 示例去掉业务层背景 / body 重置代码。

框架回归覆盖透明 / 不支持 / 缺少原生适配 / 失败回退、隐藏与恢复竞争、过时回调、已销毁视图，以及无 SDK CSS 的普通 HTML / React、Vue、Next 挂载根的注入规则。

### Build 15 验证

完整 `scripts/check.sh` 通过：Rust 538 通过 / 6 项既有 ignored，Flutter 292 通过且 analyze 无问题，SDK 9、JSON 5、设计参数 Python 4 通过；原生 / Clippy / 架构 / 文档 / 生成文件检查通过。日志为本机忽略文件 `build/framework-background-check.log`。

实机先测普通 HTML：从 Text Pad 生成仅供测试的零权限包，去掉公共控件 CSS，在 html / body / #root 上保留模板式白底、黑字及 inline important。插件脚本只有普通输入计数与 ready，没有背景适配调用。加载后宿主自动覆盖根画布，截图确认透出同一玻璃背景，根文字保持可读；默认 textarea 自有控件表面不被全局清空。该临时包完成验证后已从本地目录卸载。

随后更新无业务背景代码的 JSON 示例。页面保持相同透明背景，输入与 ⌘↵ 格式化成功；设置页往返 1,759 ms 后输入 / 输出保留，背景仍正确，原生日志再次记录透明配置。当前验证只针对 macOS 本地构建，不扩展为 Windows 或全部第三方自定义 DOM 的发行验收。


## 2026-10-19 Build 16：声明式输入匹配

- Web 命令可选声明 `inputMatchers: ["json"]`，宿主检查 `accepts` 与已批准的 `context.read` 均包含 `core.text`。旧 Manifest 不声明时保持原行为；不运行插件识别脚本或自定义正则。
- 直接输入严格 JSON 对象 / 数组会匹配 JSON Workspace；选中文本或 15 秒内剪贴板 JSON 可触发现有内容输入模式。用户执行候选后才向页面传入原文，未执行时不创建 WebView。普通文本、标量、不完整 JSON、超过 16 KiB / 128 层内容不自动匹配，仍可通过工具关键词手动打开处理。
- 停用 / 卸载 / 撤销与持久化恢复沿用已发布 Action Catalog，不留下单独注册的识别器。搜索 Scope、剪贴板有效期、隐藏状态 15 秒规则不变。
- 完整 `scripts/check.sh` 通过：Rust 546 通过 / 6 项既有 ignored；Flutter 292 通过；SDK 9、JSON 5、静态 / 原生 / 架构 / 文档 / 生成物检查通过。本轮新增 8 项 Rust 测试，覆盖格式与预算、Manifest 权限约束、完整 Launcher 查询 / 内容模式执行、原文保真、误匹配、优先级 / 过期以及停用重启。
- 本轮证据为代码与自动化测试，未重复桌面 UI 验收；前述实机记录属于其注明的历史构建。新 JSON 包版本为 0.1.1，需在新宿主中重新导入批准，不能修改或绕过旧批准摘要。日志：`build/web-input-final-check.log`。


## 2026-10-19 Build 17：修复原文传入 Web 页

- Build 16 实机复现：候选已绑定 `core.text` 原文，但页面最终出现 `Host request: TIMEOUT`，编辑器保持 0 bytes。Rust 内部投影测试未经过真实原生回复，无法覆盖该路径。
- 根因是固定版本的 macOS WebView 依赖在字符串回复的 JavaScript 插值中只处理单引号，没有保留反斜杠，导致内层 JSON 转义损坏。Host 的 `web_message_transport.dart` 改用 UTF-8 `ARRAY_BUFFER` 回复，文档启动传输层严格解码后再交给 SDK；不修改包缓存、不要求插件改代码。单帧 64 KiB、主 frame / origin / epoch / grant 校验与隐藏撤权不变。
- 新增 2 项 Flutter 测试与 3 项真实 Host bootstrap / SDK 测试，覆盖 JSON 转义、Unicode、存储响应、错误响应、帧预算、无效 UTF-8、隐藏与恢复。传输回归在修复前失败，修复后通过。完整检查通过：Rust 546 / 6 项既有 ignored，Flutter 294，SDK 12，JSON 5，Clippy / analyze / 原生 / 架构 / 文档 / 生成物检查通过。
- 全部改动与自动化检查完成后，退出 Build 16 并启动 Build 17；沿用用户已安装的 JSON 0.1.1 包，未改批准内容和权限。Global Search 输入合成 JSON → 出现一个 JSON Workspace 候选 → Enter 打开 → 编辑器自动填入 108 bytes → 鼠标 Format 得到 129 bytes 输出，路径转义、引号、换行、中文 / 表情与 19 位整数均可见且未损坏。本轮未重测选中文本 / 剪贴板采集；这些资格规则仍由上一轮测试覆盖。
- 本机日志：`build/web-reply-final-check.log`、`build/web-reply-build.log`、`build/web-reply-package.log`；App / ZIP 为 `Anywhere-0.11-preview-build17`。


## 2026-10-19 JSON 0.2.0：自动格式化与 JSONPath

- 左侧有效 JSON 在停顿 300 ms 后自动格式化；不完整输入保留原样并显示位置，格式化通过 token 映射保留光标，组合输入期间暂停处理。
- 上方 JSONPath 输入、右侧只读查询结果；单个匹配展示值，多个匹配组成数组。修改源文档 / 路径 / 缩进后旧结果失效，避免复制旧结果。默认 `$` 展示文档，Enter / ⌘↵ 立即查询，⌘⇧C 请求复制。
- 使用固定版本 jsonpath-js 0.3.1，无新增 Host 能力或权限。保留原始值片段以避免大整数、指数与转义改变；数值精度不满足比较条件时明确拒绝。限制查询长度、AST、文档值数、访问次数、匹配数和输出，不支持正则 / 脚本过滤。仅保留一个文档和一条编译查询。
- 完整 `scripts/check.sh` 通过：Rust 546 / 6 项既有 ignored，Flutter 294，SDK 12，JSON 12；原生 / Clippy / analyze / 架构 / 文档 / 生成物与独立插件模板检查通过。日志：`build/json-query-final-check.log`。新包约 78 KiB，包含新增依赖许可证。
- 全部代码与自动化检查完成后，在 Build 17 设置页更新既有 JSON Workspace 到 0.2.0，批准摘要 `5c56b671cef326387f568eac6194d64b7e2dfa1dd1ca73476960ef42b69628d2`，权限未扩大。实际 GUI 确认：Launcher 合成 JSON 自动匹配并带入，左侧自动格式化、19 位整数未改变；`$.users[*].name` 得到两个名称；Copy result 经单次确认成功，再用系统粘贴核对相同数组；不完整 JSON 保留且输出 / 复制禁用；非法路径提示错误，恢复合法路径后结果正常；未匹配显示 0 matches 和 `[]`。
- 本轮没有重新验证真实中文 IME、最大负载 GUI 或内存循环。宿主未改动，不需要重建 App；已构建并更新 `.awplugin`。


## 2026-10-19 Build 18 / JSON 0.3.0：JS 计算与统一默认控件

- JSON 工具支持 JSONPath / JavaScript 计算两种模式。公式框固定展示 `$` / `this`，字段只编辑后缀；也接受完整公式粘贴，两种模式分别保留草稿。左侧继续自动格式化，右侧结果失效与复制确认规则不变。
- JavaScript 用 Acorn 检查同步表达式后交给 SandboxJS；eager 编译所有箭头函数 AST，并禁用动态编译，执行 tick 不因惰性函数解析而绕过。超限直接抛错，避免解释器的暂停选项把超限误显示成 undefined。允许常用 map / filter / reduce / 算术等计算，不提供脚本、循环、赋值、DOM 或 Host API。操作预算、结果预算与精度拒绝策略见 JSON 工具 README；不宣称硬堆内存隔离。
- 公共默认控件从 `resources/web-controls.css` 生成到 Host 和 SDK，Host 自动注入。按钮 / 字段 / 下拉框重置浏览器默认外观；字体、字号、字重、尺寸、圆角、边界、焦点和键帽使用与 Flutter 相同的参数。Flutter 键帽改为读取公共参数，数值保持原有值。背景处理与 CSP 均未放宽；第三方自定义 CSS、系统原生下拉弹出层和文字栅格化不保证像素完全一致。
- 完整检查通过：Rust 546 / 6 项既有 ignored，Flutter 295（含新增默认控件注入回归），SDK 12，JSON 18。JSON 测试进程禁止字符串代码生成；覆盖 JS 计算、固定前缀、原文隔离、全局 / 构造器访问拒绝、数值精度、执行 / 输出预算及恢复。Clippy、analyze、原生、架构、文档和生成物同步检查通过。日志：`build/json-expression-final-check.log`。
- 全部变更与自动化检查完成后，构建并签名 Build 18，退出旧进程，进程路径确认指向 `build/Anywhere-0.11-preview-build18.app`。在设置页更新既有 JSON 工具到 0.3.0，权限保持不变，批准摘要 `c94c1c29d26e5ed5032f965f09d05d37c7d92a70b424645a3fff203948cf3e1e`。
- 实际 GUI：JSON 原文匹配并带入 / 自动格式化；粘贴 `$.users[*].name` 后仅展示固定 `$` 与后缀，返回两个名称；切换 JS 后固定 `this`，粘贴完整 `this.users.map(x => x.name)` 不重复前缀并成功返回数组；`.users.reduce((sum, x) => sum + x.age, 0)` 返回 50；切回 JSONPath 后原公式与结果恢复。按钮、字段、下拉触发器、键帽及透明背景完成视觉检查。原生选择菜单在连续点击 / 方向键 / Enter 操作后正常切换。
- 设置页导入验收中曾因窗口未处于 native key 状态而无响应；通过系统 AX Raise 将设置窗口置前后原生选择器正常打开，未绕过 `NSApp.keyWindow` 检查。不能将此自动化观察写成原生焦点问题已修复。
- 本轮未重复真实中文 IME、Windows 或长期内存循环验收。

## 2026-10-19 Build 19 / JSON 0.3.1：工作区与文件搜索统一布局

- Launcher 和工具页复用同一 Header / Footer，统一标题字号与字重、返回图标、Esc 入口、边距和底栏高度。Web 页移除底部重复的 Back / Esc；窗口与 File Search 同为 920 × 600。
- 公共设计参数统一模式标签、代码字体与行高、预览圆角和右栏比例。Host / SDK 提供通用工作区工具栏、模式标签、双栏、编辑器与错误提示样式；JSON 仅保留公式行布局，宿主不依赖 JSON 元素或业务逻辑。
- JSONPath / JavaScript 改用 Scope 风格标签；Ctrl+Tab / Ctrl+Shift+Tab 切换并保留公式草稿。双栏取消外层卡片与重复说明，以细分隔线和代码表面区分编辑 / 结果区域；聚焦时不再出现方形亮边。
- 全量 `scripts/check.sh` 通过：Rust 546 / 6 项既有 ignored，Flutter 296，SDK 12，JSON 18；analyze / Clippy / 原生 / 架构 / 文档 / 生成物和独立插件模板检查通过。新增组件回归验证共享框架尺寸、窄窗口布局、唯一 Esc 入口与隐藏时禁止操作。日志：`build/workspace-layout-final-check.log`。
- 自动检查后构建、签名并启动 Build 19，进程路径已核对。设置页把既有 JSON 包更新到 0.3.1，权限未扩大，批准摘要 `bd2c2cae5ee0a2d3ecdd699fdf36cff35a49c4dd564851068937a3e9e74ef5aa`。
- 实际 GUI 确认：Launcher JSON 自动匹配并带入 / 格式化；`.users[*].name` 返回两个名称；JavaScript `.users.reduce((sum,x)=>sum+x.age,0)` 返回 50；Ctrl+Tab 切回 JSONPath 后原公式与结果恢复；右上 Esc 返回查询页。File Search 输入 `design-tokens.json` 找到仓库文件并显示右侧代码预览；两页同尺寸截图确认标题、标签、底栏、代码字体和表面风格一致。
- 本轮未重复真实中文 IME、Windows、复制确认或长期内存循环验收。App / ZIP 为 `Anywhere-0.11-preview-build19`；构建与打包日志为 `build/workspace-layout-build19.log`、`build/workspace-layout-package19.log`。


## 2026-10-20 Build 20 / JSON 0.3.2：架构审查与清理

详细审查、具体缺陷与边界见 [收尾 Review](archive/reviews/review-v0.11-2026-10-20.md)。本轮分离包管理 / 磁盘仓储、页面绑定 / 视图 / Host 弹层 / 传输，修复保存失败误撤权、迟到绑定和视图回调、前端取消名额泄漏、设置回读竞争与清理失败状态。资源读取改为锁外 I/O 和前后身份复验，不复制 Invocation；私有 KV 清理不持有文档锁。重复 ready / Current App 门面和 4 个无使用者的旧设计参数已移除。

完整 `scripts/check.sh` 通过：Rust 548 通过 / 6 项既有 ignored，Flutter 303 通过且 analyze 无问题，SDK 13、JSON 18、设计参数 Python 4、架构规则 Python 4 通过；Clippy / 原生 / 文档 / 生成物 / 插件模板检查通过。日志：`build/review-v0.11-final-check.log`。新增回归覆盖保存失败保留文档、替换审批、迟到绑定清理、设置异步覆盖 / 清理异常 / 部分完成后读回，以及 40 次取消和发送异常后的传输恢复。使用记录测试走真实 Broker，不保留仅测试使用的并行实现。

本轮没有新的 RSS / 帧率或长期循环测量，不将移除分配 / 缩短锁范围宣称为特定内存或帧率收益；没有主动结束 GPU Helper。Windows、真实 IME、最大负载和完整恶意网页隔离仍按既有预览边界标注。


### Build 20 桌面验收进度

通过正常设置 UI 更新既有 JSON 包到 0.3.2，权限不扩大，批准摘要 `f07f77e321777e60c754f4ed9f08bd397e04772e71b35517f404b999b4756dd9`。进程路径确认指向 Build 20。打开后出现标题 / 底栏，但本轮捕获的插件正文为空白，不能视为通过。临时诊断确认 HTML / CSS / JS 读取成功、来源与视图检查通过、DOM 已生成且有 Host 请求；隔离 WKWebView 测试确认页面控件具有可见样式和非零尺寸。这些观察不足以证明 Flutter 嵌入后的绘制正常。

继续检查原生视图布局前 macOS 锁屏，桌面工具无法自动解锁，已请求用户解锁。本轮临时诊断源码已移除，正式代码复跑完整检查；当前 Build 20 的 GUI 验收未完成，空白现象待解锁后继续复验，不标注为已修复或可发布。构建 / 签名与 ZIP 版本检查通过，分别记录于 `build/review-v0.11-build20.log`、`build/review-v0.11-package20.log`。没有提交或推送代码。


## 2026-10-20 Build 21 / JSON 0.3.3：开发入口与桌面收尾

- SDK 新增 `definePlugin`：自动就绪、首次 Invocation 分发、隐藏后的待请求取消、恢复的新连接与退出释放。业务只处理 `onLaunch`，稳定 `plugin.host` 门面在恢复后继续可用；不重新导入文本覆盖文稿。原始 API 保留给进阶集成，没有新增 UI DSL、运行时依赖或 Rust 领域接口。
- 创建器 `anywhere-plugin init my-tool` 生成可移动的标准 HTML / JS 项目、构建 / 打包入口和独立 SDK 快照；拒绝覆盖已有目录。模板不需要 CSS、原生桥或可见性事件代码。默认按钮 / 输入框 / 背景由 Host 自动提供，特殊布局仍可使用标准 CSS / 任意前端框架。
- JSON Workspace 与无框架 Text Pad 复用新入口；JSON 不再重复导入 Host 控件 CSS。开发指南改为从创建命令与业务代码开始，底层协议留在进阶说明。
- Host 显示明确的加载状态；只有同一请求的 Rust `lifecycle.ready` 成功回复才能关闭提示，网页自报请求或失败回复不能关闭。新增回归覆盖错 ID、错误、其他方法与非法回复。

### 验证结果

完整 `scripts/check.sh` 通过：Rust 548 passed / 6 既有 ignored，Flutter 304 passed、analyze 无问题，SDK 16 passed（含 3 项真实 Rust 合成宿主互通），JSON 18 passed。原生、架构、生成物、Python、文档门禁通过。日志：`build/review-v0.11-final21.log`。

导出的 SDK 在仓库外通过创建、安装依赖、构建、打包、Host 包校验；检查创建器拒绝覆盖已有目录。另使用文档中的 `npm exec --offline --yes --package=... -- anywhere-plugin init ...` 成功创建独立项目。日志：`build/review-starter21.log` 与完整门禁日志；不将“能构建”当成任意第三方框架都已验收。

正式 Build 21（无临时诊断代码）进程路径已确认。通过正常设置 UI 将既有 JSON 包更新为 0.3.3，权限不扩大，批准摘要 `0b1845bffdd325468ba9b9e3589a063a1f5c2c6d79ad8d62ddca5bc5a985fa39`。实际截图确认：

1. 输入合成 JSON 后自动匹配 Action，打开后原文带入且自动格式化。
2. 加载提示消失后正文可见，透明画布与共享控件显示正常；JSONPath `.users[*].name` 得到 Ada / Lin，JavaScript `.values.reduce((sum,x)=>sum+x,0)` 得到 6。
3. 在约 5 秒的连续操作中进入设置、关闭设置、再次唤醒，文稿与 `.review` 公式仍保留，结果为 `"build21"`；恢复后的复制请求正常进入 Host 单次确认，取消后回到原页面。

Build 20 记录的持续空白在本轮正式构建中未再复现。原生布局诊断显示 WebView 和祖先视图尺寸非零、可见；首次截图可捕获加载中的短暂空白，随后正文正常出现。不能据此断言此前现象的唯一根因，只确认本轮具体复验通过并补上了加载反馈。自动化 AX 树仍未列出 Web 内控件，本轮这些交互依据真实截图和坐标验证，不能据此宣称 VoiceOver 已完整验收。

正式 App：`build/Anywhere-0.11-preview-build21.app`；构建 / 签名日志：`build/review-v0.11-build21.log`；打包校验：`build/review-v0.11-package21.log`。不修改 Impeller、不结束系统管理的 GPU Helper；本轮未增加 RSS / 帧率测量。没有提交或推送代码。


## 2026-10-20 Build 22 / JSON 0.3.4：首帧复验

Build 21 / JSON 0.3.3 的上述具体场景通过后，JSON 0.3.4 最终复验再次捕获正文持续空白，Host 面板仍响应。因此 Build 21 不作为空白问题已消除的证据。SDK 已将 client、lifecycle、默认插件入口拆成单向依赖；JSON 主动取消不再显示错误横条。SDK / 示例重跑通过，日志为 `build/review-sdk-final21.log`、`build/review-examples-final21.log`。0.3.4 更新权限未变，批准摘要 `2e07201cdb8b114630338f917fa8f382b9068b0dac1d9bde01a947912c84342f`。

隔离诊断确认：空白时 DOM / 样式 / 尺寸正常，原生窗口 key 且未被遮挡；WebKit 自己取得的快照具有完整内容，随后窗口绘制恢复。该观察将问题定位到嵌入显示衔接，不证明具体系统内部根因。临时正文 / 完整快照诊断已从生产源码移除。

Build 22 在 Rust 接受 ready 后先应用 Host 画布，然后通过公开 WebKit API 做一次 1×1 的绘制同步，再结束加载状态。只处理首次绑定，不轮询、不保留快照；迟到响应仍检查文档和视图身份，失败显示重载提示。隐藏恢复不额外申请授权或重导入输入。新增测试覆盖有界配置、视图缺失 / 脱离窗口、原生失败与隐藏后的迟到响应。

Build 22 最终完整 `scripts/check.sh` 通过：Rust 548 passed / 6 既有 ignored、Flutter 306 passed、SDK 16 passed（互通无跳过）、JSON 18 passed，analyze / Clippy / 原生 / 架构 / 生成物均通过。最后一次纯函数复用整理后，原生检查重跑通过；日志为 `build/review-v0.11-final22.log`、`build/review-native-final22.log`。

正式构建 / 签名通过：`build/Anywhere-0.11-preview-build22.app`，日志 `build/review-v0.11-build22.log`；ZIP 与源码版本 / 二进制一致性检查记录于 `build/review-v0.11-package22.log`。进程路径已核对。开始桌面复验时可在 Global Search 输入测试 JSON 并得到 Action，准备执行时 Mac 锁屏，桌面工具无法继续；日志最后记录 focus_lost，未形成首次打开 / 重载的新验收结论。已请求解锁，**Build 22 首帧修复仍待真实桌面复验，不标记为已发布或空白问题已消除。** 临时诊断 App、正文日志代码及完整快照产物已清理。


## Build 23 / JSON Workspace 0.4.0：JSON Actions

新增普通 Web 命令 Copy Formatted JSON、Copy Minified JSON、Copy Escaped JSON、Unescape JSON，沿用旧的 open 身份，新增命令使用独立稳定 ID。前三项在工作区处理后请求 Host 单次复制确认；去转义只更新编辑区。页面按钮复用相同业务函数，右侧 Copy result 保持查询结果语义。通用 Web 命令在匹配 / 使用分数相同时按声明顺序排列，避免新增复制助手因字母顺序替换默认 Workspace；不改 Scope 或 Action 身份。

Host 新增有界 `json_escaped` 输入匹配器：识别最多 4 层编码后的对象 / 数组，原文 16 KiB、解码后嵌套 128 层。不执行 Web 代码，Invocation 保留原文；普通字符串 / 标量不自动触发。解码和三种复制变换属于 JSON 插件，没有新增 JSON 专用 Rust 执行 API、系统权限或隐式剪贴板读取。插件复制输出沿用 1 MiB 上限。

完整 `scripts/check.sh` 通过：Rust 551 passed / 6 既有 ignored、Flutter 306 passed、SDK 16 passed（含真实 Rust 互通）、JSON 23 passed；analyze / Clippy / 原生 / 架构 / 生成物 / 文档检查通过。新增回归覆盖普通与转义 JSON 的动作集合、原文完整传递、全部入口路由、声明顺序、四层编码、字节 / 深度限制、大整数与重复键保真。日志：`build/json-actions-final23.log`。

正式 Build 23 进程路径已核对，通过设置更新既有 JSON 包到 0.4.0，权限未扩大，批准摘要 `a429875eaaa61f746660eefa5a35d9049514569d6894966ed0396d9cbad60507`。真实桌面验证：

1. 普通 JSON 显示 Workspace 与三项复制动作，不出现 Unescape；Workspace 排在首位。
2. 从 Launcher 执行 Copy Minified JSON 后原文带入，Host 确认展示正确压缩文本；批准后页面显示 Copied minified JSON。测试使用非敏感合成内容。
3. 页面 Copy Escaped / Copy Formatted 按钮分别请求带外层引号的转义文本 / 两空格缩进文本；大整数 `9007199254740993` 保持原样。两项确认取消后页面正常恢复。
4. 输入带外层引号的转义 JSON，出现全部五项；执行 Unescape 后编辑区还原为对象，字符串内的 `\\n` 保持 JSON 转义，大整数未变，不触发自动复制确认。
5. 本轮上述两次独立打开均在加载结束后显示完整正文、共享控件和透明画布，未复现 Build 22 记录的持续空白。这是具体场景复验，不能据此证明长期绘制问题或系统根因已完全排除；本轮未重复长期内存、真实 IME 或 Windows 验收。

App / ZIP 为 `build/Anywhere-0.11-preview-build23.app`、`build/Anywhere-0.11-preview-build23-macos-arm64.zip`，版本与签名 / 打包一致性通过；日志 `build/json-actions-app23.log`、`build/json-actions-package23.log`。插件包为 `build/web-examples/dev.anywhere.json-workspace.awplugin`。没有提交或推送代码。


## Build 25：Web 字体显示

共享 `web-controls.css` 将强制 `-webkit-font-smoothing: antialiased` 改为系统默认 `auto`，自动生成到 Flutter Host 与 SDK 默认样式。保留原有字号、字重、文字颜色、行距与背景，不添加描边 / 阴影或 JSON 专用样式。曾在 Build 24 实机尝试额外 CSS 字重补偿，观感偏粗，最终撤回补偿和对应生成器改造；Build 24 不作为交付版本。

最终生成物校验、设计参数 Python 4 项、Flutter 外观 / 工具页面 17 项、SDK 16 项、JSON 23 项及独立模板构建通过；Flutter analyze 无问题。本轮仅调整呈现样式，没有重复 Rust 完整测试或长期性能验收。记录：`build/web-type-analyze24.log`、`build/web-type-flutter25.log`、`build/web-type-sdk25.log`、`build/web-type-examples25.log`。

正式 Build 25 进程路径已核对，直接使用既有 JSON 0.4.0，无需重装插件。真实桌面打开含中英文合成 JSON 的工作区，确认正文、按钮、标签和透明画布显示正常，文字观感较强制灰度平滑更厚实清晰；字号和布局保持原有层级。跨引擎显示仍不承诺逐像素一致，其他系统 / 显示器及全部第三方自定义字体未逐一验收。

App / ZIP：`build/Anywhere-0.11-preview-build25.app`、`build/Anywhere-0.11-preview-build25-macos-arm64.zip`；构建 / 签名 / 版本一致性通过，日志 `build/web-type-build25.log`、`build/web-type-package25.log`。没有提交或推送代码。


## Build 26：剪贴板历史与悬浮页布局

- 历史页搜索区垂直居中，使用共享页框标题 / 模式字号，显式设置搜索提示字号并去掉表单的默认最小高度，修复顶边标签与过小提示文字。默认历史窗口为 960×600，悬浮窗口为 880×580，保留悬浮窗 720×420 最小尺寸。
- 列表与预览由约 56:44 调为约 68:32；预览限制为 220–340 宽，列表保留完整行高和更宽摘要。普通记录列表只读取元数据，不额外解码行缩略图。
- 复制成功反馈移到底栏，使用成功图标与简短文字；切换目标 / 修改查询时清除旧反馈，不再用警告横条挤压列表。返回 / Esc 只保留页头入口；复制、Actions 与键盘语义不变。
- 预览按内容类型整理：文本用系统等宽字体、自动换行与局部表面；短内容自然高度，长内容内部滚动。图片适配可用范围；文件有对应类型图标、文件名与可选择的完整路径，少量文件自然高度，大量文件保持惰性列表。展示复制时间、保存数据量与格式数量，原始格式名移至提示，不把引用路径字节数当成文件本体大小。
- 列表、预览、展示辅助函数分离；鼠标操作按条目身份重新定位，避免悬浮页刷新插入记录后按旧索引复制另一个目标。Rust 仍负责识别、存储、权限及有界预览；未增加原文件读取或图片行缓存。

完整 Flutter 308 项通过，analyze 无问题；架构与原生 Host 检查通过。新增回归覆盖搜索输入区几何 / 提示字号、列表宽度、短文本表面高度、复制反馈前后布局保持、选择后清除反馈，以及 720×420 / 1.5 倍文字下的长预览和键盘操作。既有多格式预览、悬浮窗重复复制、焦点 / IME、隐藏刷新与释放测试保留。日志：`build/clipboard-ui-all-flutter26.log`、`build/clipboard-ui-analyze26.log`、`build/clipboard-ui-architecture26.log`、`build/clipboard-ui-native26.log`。本轮没有修改 Rust 领域功能，未重复 Rust 全量与长期内存测量。

正式 Build 26 进程路径已核对。真实桌面通过动作打开历史页，确认顶部排版、列表与预览比例、短文本自然高度；方向键选择文件条目后显示对应图标与自动换行路径。悬浮页使用相同布局，真实 Esc 可关闭窗口。没有为验收开启 / 关闭采集或复制用户已有记录；复制反馈和图片的本轮具体回归由上述 Flutter 测试覆盖，不宣称重新完成所有真实格式的系统剪贴板恢复验收。

App / ZIP：`build/Anywhere-0.11-preview-build26.app`、`build/Anywhere-0.11-preview-build26-macos-arm64.zip`；构建 / 签名 / 版本与二进制一致性通过，日志 `build/clipboard-ui-build26.log`、`build/clipboard-ui-package26.log`。没有提交或推送代码。

## Build 27–30：统一呈现与紧凑剪贴板

最终交付为 Build 30。悬浮剪贴板改为约 400×520 的右下角单列窗口，按屏幕可用区域定位；共享历史会话与列表，默认不加载正文预览，通过 Actions 进入详情，返回保留查询 / 选择 / 滚动。窗口不因复制或焦点切走自动关闭。原生几何测试覆盖 Dock 可用区域、非零 / 负坐标和小屏约束。

列表选中、悬停、Scope / 类型标签、侧栏与 Web Actions 统一为透明覆盖和细边框；选中项悬停不再叠加第二层底色。Settings / Tasks 使用同一玻璃遮罩。通知使用语义图标 / 边框与主文字正文，预览标题、代码字体、按钮、键帽从公共参数生成。紧凑开关提升为公共 AppToggle，Web 包设置沿用相同尺寸，恢复 / 卸载使用带文字且可换行的操作。

Web 默认工具栏、页脚和权限操作支持换行；窄于 560 的工作区上下分栏，矮窗口可滚动；编辑焦点更明确，错误正文与公式错误状态相互关联。JSON 0.4.1 保留窄窗口的缩进设置，错误显示使用共享通知规则。Host / SDK 自动注入样式，插件只保留业务布局。

验证：Flutter 完整 320 项通过，analyze 无问题。新增覆盖长提示加权限入口的 36 高布局、320 宽且放大文字的权限确认、窄设置页长插件名称；既有对比度回归同时检查 Launcher 与独立窗口。SDK 16 项、包工具 Python 6 项、JSON 23 项和独立模板构建通过；公共参数生成检查、架构 / 文档检查及签名 / 打包一致性通过。日志为 `build/ui-consistency-flutter30.log`、`build/ui-consistency-analyze30.log`、`build/ui-consistency-sdk29.log`、`build/ui-consistency-examples29.log`、`build/ui-consistency-build30.log`、`build/ui-consistency-package30.log`。SDK / JSON 检查后仅修改 Flutter 设置组件和版本文档，没有再变更 Web 业务代码。

实机：核对运行 Build 30；检查插件管理开关与操作、任务空态、紧凑剪贴板。Build 29 检查文件搜索命中公共设计参数文件并展示路径、元数据和正文预览；Build 30 更新同权限 JSON 0.4.1，验证自动格式化、JSONPath 两条匹配结果、非法公式提示及修正后的恢复，Web 透明画布与 Flutter 页框衔接正常。没有复制 / 删除用户历史记录或修改采集配置。本轮未逐一验证 Windows、每种第三方自定义 CSS、全部显示器组合或 Web 窄窗口的实机尺寸，不承诺跨引擎逐像素一致。

App / ZIP：`build/Anywhere-0.11-preview-build30.app`、`build/Anywhere-0.11-preview-build30-macos-arm64.zip`；JSON 更新包为 `build/web-examples/dev.anywhere.json-workspace.awplugin`。没有提交或推送代码。


## Build 31：最终边界审查

复查 Core / Application / Persistence / Platform / Bridge / Flutter / SDK 的职责，详见 [审查与修复记录](archive/reviews/review-v0.11-final-2026-10-20.md)。确认并修复重复启用插件导致当前文档失效、损坏解压资产被重新安装静默复用、WebView 原生外观 / JS 调用无回复时永久等待；输入识别从包加载模块迁到 Core Context 层，投影统一引用协议文本限额。

修复前两个包管理回归均失败，修复后通过。完整 `scripts/check.sh` 通过：Rust 553 项通过 / 6 项既有忽略，Flutter 322 项通过，SDK 16 项 / JSON 23 项 / Python 包工具 6 项通过；包含 Clippy、原生 Host、架构、生成物与独立模板检查。日志 `build/final-review-check31.log`。外观超时回归使用受控时间模拟永不回复及迟到配置；不依赖真实系统恰好发生原生失联。

[第三方 Context 调研](research/third-party-context-providers.md) 明确当前 accepts / inputMatchers / context.read 不构成 Provider 注册，建议统一 Graph、隔离所有权 / grant / runtime，类型与校验先行再做独立 QuickJS worker 原型。未新增第三方注册接口、JS 执行器或权限。

App / ZIP 为 `build/Anywhere-0.11-preview-build31.app`、`build/Anywhere-0.11-preview-build31-macos-arm64.zip`，日志 `build/final-review-build31.log`、`build/final-review-package31.log`。本轮不提交或推送代码；实际界面检查与打包结果在完成后补记。


### Build 32–34：实机验收未通过项

Build 31 后的实机冷开页面仍复现 WebView 正文空白；Host ready 与原生外观配置已完成，点击 Host Actions 后正文出现。因此 JS ready / 1×1 原生 snapshot 不能证明最终组合画面正确，不能把该项记为通过，也不能归因于某个引擎缺陷而没有进一步证据。

已撤回没有消除问题的全画布 snapshot、原生强制重绘、等待 Flutter 帧再 snapshot 与额外画布分隔线尝试。保留一个独立的呈现简化：加载提示移到底栏，不再覆盖原生编辑区；ready 后按现有焦点恢复入口交接输入。Build 34 的完整 Flutter 322 项和 analyze 通过，日志 `build/final-review-flutter34.log`、`build/final-review-analyze34.log`；这些测试不覆盖真实 Flutter / WebKit / macOS 合成。

最终检查期间 Mac 锁定，无法继续桌面验证。当前 **不是完整 UI 验收通过的发行版本**；后续需在解锁后定位冷开空白，覆盖首次打开、返回再开、隐藏恢复和 Actions 关闭后的输入。临时 native 视图树诊断仅用于本机排查，不保留在源码，也不发布带诊断的测试 App。

清理后复核：原生 Host 全部检查通过（`build/final-review-native-clean34.log`），外观 / 文档策略 Flutter 回归 20 项通过（`build/final-review-appearance-clean34.log`），文档链接与 diff 空白检查通过。以上不替代尚未通过的冷开画布实机验收。

## Build 35：降低临时内存峰值

剪贴板派生索引借用原文，ASCII 就地转小写；Web HTML 直接使用编码后的 typed bytes。16 MiB ASCII 合成输入的独立进程物理足迹峰值由 66.1 MiB 降到 34.0 MiB，前后各三次一致；这不是整 App 待机降幅。Rust 相关 41 项、Flutter 相关 48 项、analyze、Clippy 通过。测量与边界见 [内存优化记录](archive/reviews/memory-v0.11-build35.md)。

App / ZIP：`build/Anywhere-0.11-preview-build35.app`、`build/Anywhere-0.11-preview-build35-macos-arm64.zip`；版本 0.11.0+35。未重新完成 GUI 验收，WebView 冷开空白仍待定位。没有提交或推送。

## Build 36：隐藏过期后的资源释放

过期 / 明确丢弃工作区后，在帧末清理当前引擎的闲置解码缓存，并重建已过期搜索编辑器以释放 Undo 历史。15 秒内唤醒继续复用；Settings 可见和新的激活不会被旧清理影响。隐藏生命周期已停用普通帧时，只提交一次清理帧；隐藏编辑器不自动聚焦，首次焦点不提交多余空查询。

新增五项回归覆盖 4 MiB 缓存过期归零、快速唤醒、Undo 生命周期、停帧和 Settings 共享引擎。Flutter 全部 328 项、analyze 与架构检查通过。App / ZIP 为 `build/Anywhere-0.11-preview-build36.app`、`build/Anywhere-0.11-preview-build36-macos-arm64.zip`。完整证据及未完成实机范围见 [内存优化记录](archive/reviews/memory-v0.11-build36.md)。Mac 仍锁定，未切换运行版本；没有提交或推送。


## Build 47：内存变更验收与运行时修复

- 修复实机捕获的剪贴板监听 SIGABRT：Cocoa 异常在 Platform 转为失败状态，macOS 代数监听使用受保护的 500 ms 查询；格式读写继续使用 clipboard-rs。新增真实 Objective-C 异常、停止监听和代数恢复回归，不读取用户剪贴板 fixture。
- Settings 搬回共享内容后关闭并释放空原生窗口；只保存位置，不额外创建 Flutter engine。原生弱引用回归确认窗口释放。
- 删除未能证明最终绘制正确的 WebKit 首帧截图路径、平台消息和 Dart 等待接口。初始焦点在 ready / 外观配置后交接，ready 取消多余的延迟资源探测。没有保留临时加载层、玻璃分层改动、焦点实验或诊断输出。
- 自动化：Rust 557 通过、6 项既有 ignored；Clippy 通过；Flutter 326 通过且 analyze 无问题；原生、架构和文档检查通过。Flutter 减少的两项测试专属于已删除的截图接口，外观回退和过期回调测试仍保留。
- 实机：Build 45 无截图变体的两次独立进程冷开、最终 Build 47 的首次打开与返回后再开均直接显示正文，无需点 Host 标题 / Actions。Build 47 粘贴后自动格式化、Settings 正常显示、快速往返保留正文通过。Build 36 已补验全局输入和 15 秒隐藏规则；Build 38 检查文件搜索命中新建源码文件并展示固定预览。后续桌面出现外部应用焦点切换后，没有把受干扰的附加操作记为通过。
- 交付：`build/Anywhere-0.11-preview-build48.app`、`build/Anywhere-0.11-preview-build48-macos-arm64.zip` 及校验文件，版本 0.11.0+48。持久身份 deep / strict 签名与归档关键文件一致性检查通过；日志 `build/verify-build48.log`、`build/verify-package48.log`。当前运行此构建，没有提交或推送。

上述为本机已复现场景的修复与回归，不是所有 WebKit / Flutter 版本的保证。没有新增整 App 与 helper 的同场景内存曲线，不把闲置缓存归零或原生窗口释放等同于完整 RSS 降幅。详细边界见 [运行时验收记录](archive/reviews/verification-v0.11-build47.md)。


### Build 48：故障后的剪贴板年龄

最终核对补齐一个故障恢复边界：后台与前台统一通过代数端口查询；元数据查询失败时同时失效剪贴板年龄，避免服务恢复后的旧内容被前台读回误记为刚复制。观察回调先读代数再取观察锁，避免失败清理重入同一锁。监听的失败 / 恢复基线与 15 秒规则不变。最终交付为 Build 48，包含以上 Build 47 修复；Flutter / 原生 UI 源码相同。Rust 557 通过、6 项既有 ignored，Clippy 通过，日志 `build/verify-rust48.log`、`build/verify-clippy48.log`。


Build 48 已实际启动并核对进程路径，Current App 页面正常。附加冷开验证被桌面工具明确提示手动操作打断，未记为新一轮通过；沿用 UI 源码相同的 Build 47 验收范围。此次生成的中间测试 App / ZIP 已移到 `build/archive/verification-2026-10-21/`，顶层交付使用 Build 48；没有改动此前 Build 35 / 36 的材料。

## Build 50：性能与交互最终核对

剪贴板搜索在组词期间暂停，旧响应失效；重复选择复用当前预览，复制结果不等待图片或历史刷新。隐藏页不抢焦点，Actions 的可见性 / 修饰键约束统一。快捷键注册错误使用恢复提示并限定页面。JSON 0.4.2 只改公式时复用已格式化文本，内容 / 缩进变化继续重新处理。

工程验证与实机边界见 [收尾 Review](archive/reviews/closeout-v0.11-build51.md)。完整检查通过后，实机发现的设置提示已修复，并重新执行 Flutter analyze / 333 项测试。

### Build 51：渲染尺寸保护

Build 50 实机捕获一次 Impeller 纹理尺寸不一致崩溃，不能记作最终验收通过。已在固定 SDK 的真实后备缓存中复现错误尺寸复用；Build 51 在原生适配中校验并拒绝不匹配的表面，保留正常分配、Impeller 和隐藏状态策略。详见同一收尾 Review；引擎内部方法兼容性需要随 SDK 升级继续核对。

Build 51 完整门禁再次通过，并已完成持久身份签名与 App / ZIP 构建。实机启动和 5 次搜索 / 历史页往返正常，保护安装成功，观察期间无新增纹理验证错误或 Build 51 崩溃报告。最后的设置页补验因手动状态保护取消；不将未完成的该项画面、真实 IME 或 VoiceOver 记为已通过。JSON 0.4.2 新包已构建与测试，本轮未安装到用户目录。


### 最终补验与工程收尾

Build 51 的设置页补验完成，Shortcuts / General / Features 显示正常；正式安装 JSON Workspace 0.4.3 后，默认 JavaScript / `this`、正文首次绘制及 Esc 保留查询均已观察。此记录替代上方相应的中断状态；完整证据与发行边界见[收尾 Review](archive/reviews/closeout-v0.11-build51.md)。本地预览工程收尾完成，后续进入中英国际化与拼音检索；发行矩阵不据此视为全部通过。
