# Anywhere 产品路线图

最后更新：2026-11-11。当前为 **V0.14 macOS 15+ GPUI 开发预览**。功能已接入，
完整物理桌面矩阵、正式签名、公证和 Windows / Linux 桌面产物尚未完成。当前
范围与证据见 [GPUI 技术方案与验收](mvp-gpui.md)，版本以
[Cargo workspace](../Cargo.toml)和
[Info.plist](../apps/gpui/macos/Info.plist)为准。

## 产品目标

唤醒时理解用户正在操作的对象，提供可执行的下一步；需要搜索时，把应用、动作
和对象放入统一候选与导航。Rust 拥有业务、排序、授权和执行；GPUI 拥有页面、
输入、焦点与窗口；系统能力留在平台适配层。

## 当前基线

| 区域 | 状态 |
| --- | --- |
| macOS GPUI 产品壳 | 主窗口、状态栏、全局热键、独立 Clipboard Shelf 与独立 Running Tasks 窗口已接入；物理验收待完成 |
| Launcher / Scope / 菜单 | 已接入，包含 Current App、迟到 Context 和执行资格 |
| 文件搜索与 Quick Look | Rust 搜索、连续结果与 GPUI 路由已接入；系统焦点 / 格式矩阵待验收 |
| Clipboard | History、策略、Shelf 与按需预览已接入；长期资源和权限矩阵待验收 |
| Settings | 热键、Context、目录、剪贴板、应用历史、Feature、语言与登录项已接入 |
| 应用历史 | 18 个配置化来源、页面查询与对象动作已接入 |
| Running Tasks / Native Tools | 已有 typed 路由、详情和明确未知类型错误 |
| Web plugins | 安装审查、启停、WebView、IPC、ack、主题与 teardown 已接入 |
| 共享跨平台 crates | Linux / Windows 编译和 Linux 回归存在；尚无桌面 App |

“已接入”表示源码和自动化存在，不等于物理 IME、Accessibility、VoiceOver、
登录项、Quick Look、WebKit helper 或视觉材质已通过。

## 下一阶段

### V0.14 收尾

1. 完成 GPUI 快速门禁、完整 workspace 测试、Clippy、文档和架构检查。
2. 构建并验证 `Anywhere.app` 的 Bundle ID、最低系统、签名、资源和动态库。
3. 执行[物理桌验矩阵](mvp-gpui.md#物理桌验矩阵)，逐项记录结果。
4. 修复桌验发现的 IME、焦点、状态栏、Shelf、Quick Look、WebView 和权限问题。
5. 在不修改历史数字的前提下建立当前 GPUI 性能样本。

### V0.15 插件开发体验

- 增加隔离的浏览器 Host Mock 和可诊断包校验；
- 明确同名包更新对快捷键、Usage 与 KV 的影响；
- 补齐 Host / SDK 兼容表与安装回滚矩阵；
- 保留当前安装摘要批准和旧文档撤销规则。

后台插件继续使用按需 QuickJS。进程隔离、资源级沙箱和完整 HMR 需要单独设计，
不以 WebView 权限收窄代替。

### V1.0 macOS

V1.0 进入条件：

- 无权限、部分权限、慢来源和迟到结果下，基础搜索与其他动作仍可用；
- Current App 菜单、Finder、选中文本、剪贴板、文件搜索和应用历史经过真实
  应用验证；
- 文件、剪贴板、应用历史和 Usage 的有界存储 / 取消 / 失败恢复通过；
- Web 插件安装、授权、执行、撤销、更新和资源释放经过真实包验证；
- GPUI 物理 IME、焦点、Accessibility、Quick Look、WebView、VoiceOver、材质
  与多显示矩阵有记录；
- Developer ID 签名、公证、干净机器安装 / 升级 / 回退和性能基线完成。

自动更新、HMR、插件市场、完整终端 PTY 和 Windows 完整版不作为 V1.0 前置。

## 后续候选

| 能力 | 进入条件 |
| --- | --- |
| 浏览器历史 / 书签 / Tab | 单独验证浏览器接入、Profile、窗口和 Tab 身份；当前未实现 |
| 第三方 Context | 保持包命名空间、有界输入和 broker；完善进程隔离与兼容矩阵 |
| Terminal / PTY | 已验证 Web 工具需求后设计输入、尺寸、流控和高输出 |
| Windows / Linux GPUI | 各自产品 host、平台能力、IME、热键、预览和安装包矩阵 |
| Marketplace | 可信发布、签名、更新回滚、撤销与审核边界完成后 |
| OCR / Translate | 独立来源、权限和资源验证，不扩张当前 V1.0 |

## 稳定契约

1. 采集种子与 Provider 派生分开；冻结来源应用身份。
2. `BusinessActionId`、`CandidateId`、Session / revision 和 ExecutionSession
   身份分工明确。
3. Context 正文、文件路径和剪贴板内容不进入命令 Usage。
4. 查询发布和执行分别复验版本、资源和权限；成功副作用照常记录 Usage。
5. Context、搜索、进程、输出和缓存分别限额；取消不提前释放仍在运行的 I/O。
6. Launcher 短暂显示；Running Tasks、Shelf、Quick Look 和 Web 文档有各自寿命。
7. 不可用菜单隐藏；缺少 Accessibility 权限不阻断其他动作。
8. 文件、目录、工作区文件和远端 URI 不混为可猜测的字符串。

Menu 重扫和旧绑定安全见[Current App 菜单](current-app-actions.md)。

## 历史

已完成阶段的逐构建证据保留在[归档](archive/README.md)。历史构建、框架、性能
数字和远程发布包不能作为当前 GPUI 的实现或验收证明。
