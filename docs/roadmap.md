# Anywhere 产品路线图

最后更新：2026-10-14。当前主线为 `dev-0.9`；版本号表示依赖顺序，不代表发布日期。实现进度以 [当前 MVP](mvp-v0.9.md) 为准，历史构建与验收在 [归档](archive/README.md)。

## 产品目标

在唤醒时理解用户正在操作的对象，提供可执行的下一步；需要搜索时，应用、动作与对象进入统一候选与导航。Rust 负责业务、Flutter 负责交互、平台层负责系统能力。

当前主要验收 macOS。共享代码和编译检查不代表已经交付 Windows App，平台接入单独排期。

## 已有基础与下一阶段

| 阶段 | 范围 | 当前状态 / 进入条件 |
|---|---|---|
| V0.3.x App Actions | 菜单发现、执行、范围与权限降级 | 已接入；长期菜单身份仍有安全边界，历史与持久绑定隔离 |
| V0.4 Context Sources | Finder 选中项、选中文本、浏览器 URL、剪贴板 | 工程收尾；来源、默认输入和执行归属有明确契约，实机权限矩阵保留 |
| V0.5 Developer / Process Sessions | Git / Rust / Maven / Gradle / Node / Spring Boot，后台任务 | 功能与工程收尾，已有真实工具验收记录 |
| V0.6 Action Shortcuts | 本地 / 全局固定动作、应用 / Context 意图、导入导出 | 工程收尾；真实键盘与完整系统矩阵仍需记录 |
| V0.7 File Search | 普通 Action 入口、系统索引 + 遍历、稳定连续列表、固定预览 | 工程收尾；完整权限 / 卷 / 物理输入矩阵保留 |
| V0.8 First-party / Extension v2 | 模块贡献、明确能力批准、生命周期、真实 Source / Storage | S1–S3 工程收尾，macOS 关键闭环已实机检查；正式发布矩阵保留 |
| **V0.9 Host-rendered Plugin UI** | List、Detail、Form、KeyValue、Markdown 的受限描述与宿主渲染 | S1 Detail / KeyValue 已接入；下一步 Form 的输入、校验、提交，再扩展 List / Markdown，不注入任意 Widget |
| V0.10 PTY / Terminal | 进程会话上增加 PTY、输入、尺寸变化与终端视图 | 验证流控、高输出、返回 / 停止 / 退出生命周期 |
| V1.0 | Context-aware Launcher、搜索、多来源 Context、开发动作、扩展协议和可返回会话 | 满足下方发布条件 |

## 范围取舍

- 现有文件搜索复用 Spotlight 快速路径，加完整遍历，不维护长期文件索引；Clipboard History / Browser History 尚未纳入当前功能。
- 第一方内部 Rust 模块化不等于第三方 Native ABI。外部 v2 目前是受限声明式操作，插件 UI 属于下一阶段。
- 文件预览先保持元数据 / 有界文本。图片、PDF、系统 Quick Look 等逐项验证，不把图标分类当作富预览。
- 通用 Web 插件、Code Agent 管理、ACP、Clipboard Manager、Screenshot / OCR / Translate、Marketplace 不列入 1.0 门槛。
- Terminal 可使用专用 WebView / xterm.js 桥接，通用 React / Vue / Svelte 插件不是前置条件。输入、生命周期与吞吐需要独立验收。

## 架构演进

V0.8 以实际模块验证 ContextProvider / ActionBundle / CandidateSource / Storage / Capabilities；模块新增不得改 Launcher 主流程。V0.9 增加版本化受限 UI 描述，由 Host 渲染。V0.10 扩展 ExecutionSession 而非另建一套终端任务系统。

必须守住的契约：

1. 采集种子与 Provider 派生分开；冻结应用身份，多选保留集合。
2. BusinessActionId、CandidateId、Launcher Session / Revision、ExecutionSessionId 分工明确。
3. Context 来源可控，正文与路径不进入命令使用历史；来源错误不阻止基础功能。
4. 查询发布和执行分别复验版本、资源和权限；成功副作用照常记 Usage。
5. Context、搜索、进程、输出与缓存分别限额，取消不提前释放实际阻塞 worker 的 admission。
6. Launcher 短暂显示，任务 / Terminal 使用独立持久窗口。“持久”先指应用运行期间可返回，跨退出运行需单独决策。

无法可靠识别的动态菜单不承诺永久稳定 ID，不恢复旧历史或持久绑定；详见 [菜单安全](current-app-actions.md)。

## V1.0 发布条件

- 无权限、部分权限、慢来源与迟到结果下，基础搜索和其他动作仍可使用。
- Current App 菜单、Finder、选中文本、浏览器等来源经真实应用验证，复用统一动作与导航。
- Git 和各 Developer 类型有实际项目验收，长任务可返回、查看结果和停止，输出有界。
- 至少两个第一方模块使用同一装配接口；外部声明式插件与 Host UI 插件有版本兼容、能力拒绝和真实返回流程。
- 自动化、签名 / 公证、安装升级、权限降级、性能基线与手工交互矩阵均有记录。

工程检查和开发包完成不能替代发布矩阵。当前进度与证据见 [V0.8 MVP](mvp-v0.8.md)。
