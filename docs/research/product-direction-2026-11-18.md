# Anywhere 产品发展调研

日期为 2026-11-18。建议把下一阶段投入集中在 **让用户从手上的对象出发，用键盘做完下一步**。
第一批面向经常在 Finder、IDE 或编辑器之间切换、使用中文或英文搜索的 macOS 开发者。
先验证复制路径、恢复项目和处理 JSON 是否值得用户在日常工作中反复唤醒 Anywhere，
以实机和试用反馈决定后续边界。版本承诺和发布门槛由[产品路线图](../roadmap.md)维护。

## 同类产品与定位

| 产品 | 官方资料确认的能力 | 对 Anywhere 的启发 |
|---|---|---|
| Spotlight | 系统内提供动作、快捷指令、quick keys 和剪贴板历史。 [动作](https://support.apple.com/en-afri/guide/mac-help/mchl4953dfeb/mac) · [剪贴板](https://support.apple.com/en-ge/guide/mac-help/mchl40d5b86b/mac) | 试用允许保留 Spotlight 默认键。先测无需迁移即可获得的收益。 |
| Alfred | Universal Actions 可直接处理其他应用里选中的文件、文本与 URL，并允许 Workflow 扩展。 [Universal Actions](https://www.alfredapp.com/help/features/universal-actions/) · [Workflows](https://www.alfredapp.com/help/workflows/) | 选中对象后弹出动作已有成熟方案。测试应比较从现有对象到任务完成的步骤、耗时和出错率。 |
| Raycast | 有前台应用菜单搜索、按对象提供的 Action Panel、扩展与 AI。 [应用菜单](https://manual.raycast.com/navigation) · [Action Panel](https://manual.raycast.com/action-panel) · [扩展](https://manual.raycast.com/extensions) · [AI](https://manual.raycast.com/ai/ai-chat) | 当前应用菜单也已有成熟方案。检验 Anywhere 的 Current App 是否能在同一入口内把菜单、项目和对象动作排到有用的位置。 |
| PowerToys Command Palette | Windows 已有可扩展的键盘命令面板，支持应用、命令、文件与剪贴板。 [Command Palette](https://learn.microsoft.com/en-us/windows/powertoys/command-palette/overview) | Windows 立项要有具体工作流需求和原生交付条件。通用启动功能的覆盖面不足以决定移植优先级。 |

上述对照基于官方功能文档，采用情况、速度和内存差异仍需实测。Anywhere 的产品假设
是同一个唤醒入口能同时利用前台应用、可用选中项和用户认可的近期输入，并保持明确的
执行目标与返回路径。中文动作名、拼音和首字母是可立即验证的体验，后续补词应来自
用户实际搜索失败的样例。

## 当前交付起点

- 当前工作树是 V0.14、macOS 15+ 原生 GPUI。核心流程已有实现，应用历史已有 18 个配置化来源。
  应用历史的多安装、路径和权限组合仍需验证。Windows / Linux 当前有共享 crate 的检查。
  [项目状态](../../README.md) · [GPUI 交付范围与证据](../mvp-gpui.md)
- 当前证据包括自动化、开发签名本地包和部分桌面实测。近期窗口过渡与隐藏释放改动的实体键盘、
  减少动态效果和进程 footprint 需要按对应构建测量。V0.14 验收页将 VoiceOver 排除在本轮范围外，
  V1.0 路线图将其保留为发布条件。 [测试矩阵](../test-acceptance-checklist.md) · [性能方法](../performance.md) · [路线图](../roadmap.md)
- GitHub Releases 页面当前列出 2026-09-18 的 V0.3 历史预览。本地 GPUI 试用与对外分发之间仍有签名、
  公证、干净机器安装和升级回退的工作。 [公开下载页](https://github.com/FantasyDream/Anywhere/releases) · [发布检查](../releasing.md) · [Apple 公证要求](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution/)

## 首批验证的工作流

| 真实任务 | 可复用的入口 | 优先验证的摩擦 |
|---|---|---|
| Finder 中选定文件或目录后复制路径，或对适用目标打开终端 | Current App 的文件与目录动作。 [使用指南](../usage.md) | 用户一眼能确认目标并选中动作。切换选中项、目标消失或缺少权限时有明确反馈，基础搜索仍可继续。 |
| 在 IntelliJ IDEA 或 VS Code 中恢复另一个最近项目或工作区 | 当前应用的最近项目入口及来源对象动作。 [应用历史](../application-history-sources.md) | 区分同名项目，验证本机目录、工作区文件和远端 URI 各自的行为。试验优先覆盖两个维护者实际使用的来源。 |
| 复制一段模拟响应 JSON，格式化后复制回编辑器 | 已导入且授权的 JSON Workspace，支持已选文本和符合条件的近期剪贴板。 [JSON Workspace](../../plugins/examples/json-workspace/README.md) · [使用指南](../usage.md) | 用合成数据验证取的是预期输入、输出有效、Esc 能回到原操作。授权被拒绝或内容过期时提供可理解的恢复路径。 |

首轮给任务说明和一次唤醒键提示。用用户原有方法作为对照，交替测试顺序，记录完成率、
耗时、按键或指针操作次数、错误目标、权限中断和返回结果。首次安装与熟悉后的任务分别记录，
避免把熟悉程度当作产品收益。建议先找 5 名试用者观察一周。可用 **至少 4 人独立完成同两项任务，
至少 3 人在不同天主动复用至少一项** 作为继续打磨的初始信号。样本仅用于发现摩擦和决定下一轮投入，
不能外推到整体留存。使用脱敏结果和自愿反馈，记录场景与错误类型即可。

## 建议投入顺序

| 优先级 | 交付 | 进入下一阶段的依据 |
|---|---|---|
| P0 · 建立可复现的 macOS 体验基线 | 固定一份候选构建，用实际热键和中文 IME 跑通唤醒、首键、Enter、Esc、权限拒绝与切换来源。覆盖空白/有查询两条隐藏路径、减少动态效果、Quick Look、Shelf 与 WebView。为状态栏入口和快捷键冲突提供可理解的恢复。 | 关键路径有对应构建、系统与架构的证据。无可复现的首键丢失、执行错对象、卡焦点或主流程崩溃。 [桌验清单](../test-acceptance-checklist.md) |
| P1 · 打通首次使用与试用安装 | 支持用户在授予可选权限前完成普通搜索。只在用户触发对应能力时说明权限及返回路径。沿现有发布流程生成带版本、校验值和已知限制的试用包，记录隔离数据与已有数据的安装、升级和回退。 | 正式对外分发使用 Developer ID、公证和经复验的包身份。对邀请试用用户按分发方式提供明确限制。三条任务的反馈可复现，重复使用信号达到首轮约定。 [发布检查](../releasing.md) |
| P2 · 收窄 V0.15 插件开发体验 | 让外部作者从现有示例独立建包、导入、调试权限错误并升级。以 JSON Workspace、Text Pad、JSON Summary 覆盖混合、UI、后台路径，围绕实际阻塞完善 Host Mock、包校验和兼容矩阵。 | 至少一个外部作者能按文档完成完整链路。证明撤销、替换和失败升级时旧调用失效，明确快捷键、Usage 和 KV 的处置。 [插件开发](../plugin-development.md) · [插件设计](../web-plugin-design.md) |

P0 的性能比较使用同机、同系统、同缩放和同一套脱敏数据。分别测冷启动与热唤醒，记录
热键到可输入、提交到可用候选的 p50 / p95，并记录丢键、误触发与画面闪动。隐藏前后、约 1 秒、
约 16 秒和再唤醒分别采主进程与对应 helper 的 footprint，观察多轮操作是否持续抬高基线，
同时核对空闲会话、保留会话和预览资源各自的释放行为。分别记录应用内诊断时间与实机端到端时间，
待拿到分布后再设绝对预算。过渡只服务于位置连续性，键盘响应、选择与焦点应先通过同一套动作验证。
[Launcher 隐藏与过渡验收](../performance.md#launcher-隐藏与过渡验收)

## 后续扩展的进入条件

| 候选 | 适合开始的条件 |
|---|---|
| 浏览器工作台 | 首轮反复出现找回标签页、书签或历史的需求时，保持三者在同一个独立功能阶段规划。先用合成 Profile 验证 Chrome / Edge 区分来源与同 URL 的标签页，验证授权与按需读取后再决定原生交付范围。 [浏览器规划](../browser-feature.md) |
| 自定义快捷动作 | 多人在重复组合相同现有动作时，先验证固定对象或保存常用命令能否解决问题。再决定是否扩展参数传递或动作组合，沿用执行前身份与权限复验。 [动作与执行边界](../architecture.md) |
| 插件市场 | 有持续的第三方作者和分发需求，包身份、兼容、签名、回退、撤销、资源隔离和维护成本都能验证后再立项。当前按需 QuickJS 和 WebView 继续经 Rust broker 授权。 [插件设计](../web-plugin-design.md) |
| AI、云同步、完整 PTY、Windows / Linux App | 分别绑定已知任务和维护者。AI 先验证显式选中对象的一次性处理，量化收益并明确输入与执行权限。跨平台先证明至少一条 macOS 上已有复用的流程可适配，另建自己的输入、预览和发布矩阵。当前投入优先保留给三条 macOS 工作流。 [路线图](../roadmap.md) |

现有 Rust 与 GPUI 分工足以承载第一批验证。功能接入继续使用静态 Feature、Context / Action
与已有页面，原生适配只承担系统能力。仅在多条真实工作流出现同一缺口时抽取共享能力。首个交付
单位建议是 **同一份试用构建、三张任务卡和可复现的失败记录**。 [架构](../architecture.md)
