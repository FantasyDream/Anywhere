> 历史记录：保留当时版本与验收边界，当前行为以[文档导航](../README.md)为准。

# V0.9 MVP — 内置功能模块与原生工具页

主线 `dev-0.9`。2026-10-14 用户确认新方向：第一方取消热加载模型，作为 App 的内置功能；第三方 WebView 插件放到最后。当前按 [模块迁移路线图](feature-module-roadmap.md) 一次实施一个阶段，**V0.9 工程收尾与本轮关键闭环验收已完成**。

## 目标

启动时静态装配内置功能，通过统一贡献接入 Context → Action → Candidate → Ranking。业务 Rust、展示 Flutter、系统差异 Platform；功能开关使用策略快照，不通过重新加载模块生效。页面使用统一容器和静态注册，以已有结果 / Detail 页面验证复用。2026-10-16 用户确认 JSON 整体延期：待 WebView 插件机制完成后使用 WebView 插件实现，移出本版验收目标。

原 V0.9 S1 Detail / KeyValue 已实现的代码与证据保留在 [开发记录](v0.9-development-records.md)，不是本轮已经完成架构迁移的证明。Form / List / Markdown 外部 UI DSL 不再继续开发。

## 当前阶段

**P01 已完成**：静态功能目录契约、集中注册入口和身份检查。适配旧 FirstPartyExtension，现有 9 个模块的身份和顺序保持；无效 / 重复 ID 在构造前拒绝，构造器的身份也复验。完整门禁 415 Rust / 238 Flutter 通过，6 Rust ignored；Native、fmt、Clippy、架构 / 生成规则、文档和 analyze 通过。记录 `build/validation/v0.9-p01-check.log`。

**P02 已完成**：5 个模块入口注册现有 9 个功能，Cargo 编译期自动发现并生成目录；显式顺序 / ID 决定装配顺序，非法 / 重复 ID 编译失败。生成快照检查接入本地门禁、macOS 打包与 portable CI。6 条目录回归与 4 条生成回归通过，覆盖新增 / 移除、跨模块重复声明、空声明、含中文 / 空格路径的实际编译，以及检查失败不覆盖快照。完整门禁 421 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p02-check.log`。CI 已配置，未在本机声称完成 Windows 验收。

**P03 已完成**：9 个内置功能的 metadata 使用静态 Rust FeatureDescriptor，内置 TOML、FirstPartyManifest 与对应解析器退役。设置状态显示统一使用 App 的 Cargo workspace 构建版本；外部 v1 / v2 / v3 包版本与审批契约保留。名称、操作列表、配置键、贡献身份和 Storage namespace 不变；描述校验与 Host 允许列表独立，执行仍检查授权。完整门禁 423 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p03-check.log`。

**P04 已完成**：收藏目录拆为 SavedDirectoryRepository / SavedDirectoriesService，由 FeatureServices 持有具体 service；删除 ExtensionState、runtime_with_state 和 Manager 的通用状态箱。模块装配不再接收原始存储，Actions / Source 只注入 service；Host 仍检查 Storage grant 并绑定原 namespace。查询只读最多 100 条的内存缓存，写入继续使用最新已提交记录的 SQLite 事务，打开前复验收藏与路径；数据库连接更换后重新绑定 service。新增多实例并发保存、旧候选打开拦截、未知数据版本不覆盖、坏数据库替换 / 修复回归。完整门禁 427 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p04-check.log`。

**P05a 已完成**：features.json / FeaturePreferences 保存内置开关，extensions.json / PluginPreferences 保存外部批准与开关并保留旧键为迁移备份。首次创建新文件迁移合法旧禁用键，已有新文件不再导入旧值，禁止迁移覆盖现有目标。两类配置独立只读恢复，每次只写对应文件，写前复验、保存失败不发布目录或策略。新增 FeaturePolicySnapshot / generation，仅内置策略与恢复资格变更推进，外部操作不推进。完整门禁 436 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p05a-check-final.log`。

**P05b 已完成**：注册表显式记录可配置贡献归属，动态动作继承 Factory 归属，Provider / Actions / 同步与异步 Sources / 快捷键资格在同一策略下筛选；基础文件与文本能力保持。Application 原子安装与 Manager 相同的策略 Arc，请求、快照提交和执行复验其身份。策略变化取消旧会话下的异步等待，拒绝旧候选、迟到结果与隐藏恢复；已启动 ExecutionSession 继续独立运行。保存失败保留旧策略与会话。新增 6 条回归；完整门禁 442 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p05b-check-final.log`。

**P05c 已完成**：BuiltinFeature / FeatureBindings 在启动装配，SettingsService 协调独立的 FeatureSettings / LegacyPluginSettings；删除旧 FirstPartyExtension / ExtensionRuntime / ExtensionManager 和 Reload API。开关过滤启动目录，动作与来源复用同一对象，数据修复只更换具体 service 的 repository。第二次装配同一 Application 被拒绝，失败启动可重试。旧声明式插件仅启动发现 / 注册，新批准需重启，撤销即时失效；刷新不发现新插件。设置页改为 Features / Refresh。完整门禁 445 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p05c-check-final.log`。

**P06 已完成**：FeatureContribution 自注册轻量 InputRecognizer，ContextProviderRegistry 同归属筛选、按原优先级排序；自动填入和测试 / 工具辅助识别共用目录，移除中央 JSON / URL 接线。Application 捕获当前规则 / 策略 / catalog 版本，识别在发布锁外执行，变更后拒绝迟到建议。超过 16 KiB 不解析，保留选区优先、剪贴板 15 秒、基础路径与 UI 编辑 / 隐藏保护。完整门禁 449 Rust / 238 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p06-check.log`。

**P07 已完成**：静态 ToolPageRegistry 注册具体 ToolPageSession，共用 ToolPageShell 的布局、返回、焦点 / IME / 键盘规则。内部文本结果与 Legacy Detail 都迁入该容器，Launcher 不解析 Detail 字段；新页面不新增 Controller 的领域分支。15 秒内保留页面和编辑模型，过期 / 返回释放；返回恢复原查询、选中项与输入焦点。完整门禁 449 Rust / 241 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p07-check.log`。

**P08 原生 JSON 已实现，验收未通过，现已延期**：JSON Workspace 注册为普通工具 Action / 原生页面；原 Format / Minify ActionId 保持并进入同一编辑页，Copy Formatted JSON 保持原行为。Rust 提供格式化、压缩、复制结果和错误行列，Flutter 管理输入 / 结果及快捷键。编辑或关闭拒绝迟到响应；关闭页面撤销排队操作而保持父查询。FeatureOperation 共用 session / policy / epoch、实际 worker admission 和 Usage。输入 256 KiB，结果 / 复制 512 KiB，格式化序列化有界。完整门禁 455 Rust / 244 Flutter 通过，6 Rust ignored；日志 `build/validation/v0.9-p08-check.log`。

**P09 已完成（2026-10-16）**：领域实现与入口统一到 features/；Host 授权、metadata、设置策略和类型化服务分离；旧重载框架 / 目录与旧结果渲染路径清理。JSON 原生工作区、内置 Provider / Actions / 识别、专用桥接以及仅服务它的 FeatureOperation / worker 框架移除；通用 ToolPageRegistry / Shell 和结果 / Detail 页面保留。交付 8 个可配置内置功能，旧 JSON 动作身份、matcher / Usage 归一、禁用键与用户历史保留迁移依据。

Build 5 修复的六条隐藏竞态回归保留。Build 6 在代码清理和完整单元检查完成后打包并验收：449 Rust / 251 Flutter 通过，6 Rust ignored；Native menu / host、fmt、Clippy、架构 / 生成 / 文档检查、Flutter analyze 全部通过，无 JSON 功能测试跳过规则。日志 build/validation/v0.9-final-check.log。新增退役入口回归验证动作、快捷键、Scope 与自动填入均不暴露 JSON，旧配置字节不变。

## 收尾验收结果

| 项目 | 结果与边界 |
|---|---|
| 主输入 / 工具返回 | Build 6 可以直接输入并进入 File Search；Git Status 的共享结果页可返回，原 `~/` 查询保留且继续输入无需点击 |
| Current App / 菜单 | Finder 页面聚合 Folder / Git / Rust；Browse Menus 展示真实菜单层级，执行新建访达窗口后可恢复原菜单层级；ChatGPT 菜单也成功采集 |
| 文件搜索 | 主窗口扩展布局、固定预览、选择跟随、文件类型过滤及追加顺序已检查；本机 settings 查询返回 1592 项，455 个受限位置跳过，无权限申请弹窗或旧黄色错误横幅。受限位置未搜索不等于所有磁盘文件完整覆盖 |
| 真实组合键 | 用户确认 Ctrl+Tab 切换类型和 Command-K 二级 Actions 均正常（临时诊断构建，业务按键逻辑与 Build 6 相同）。自动化缺少左右修饰设备位，不能作为真实键盘失败的证据；所有诊断代码已移除，生产 App 无追踪字符串 |
| 15 秒恢复 | 设置短暂切换后恢复查询 / 输入；打开目录、执行 Finder 菜单产生真实 focus_lost，约 5.1 秒后唤醒保留原查询 / 选择或层级，并可继续输入。超过 15 秒后重新采集 Current App。六条异步竞争边界由确定性测试覆盖 |
| 设置与策略 | Build 6 显示 8 个内置功能及 Refresh，关闭 / 返回正常；开关、保存失败、迟到拒绝、数据恢复及后台任务跨策略继续运行由完整自动化回归覆盖，本轮未改变用户开关 / 目录授权 |
| 性能 | CPU / 10k–40k 文件遍历基准与整 App RSS / vmmap 采样已记录；见 [性能文档](../performance.md) 与 [原始样本](benchmarks/README.md)。不据此声明端到端帧耗时或长期无泄漏 |
| 包 | 0.9.0+6 App / arm64 ZIP / SHA-256 验证通过，稳定签名、源码版本与 ZIP 关键文件一致，保留 Impeller。构建和打包日志为 build/validation/v0.9-build6-build.log、v0.9-build6-package.log |

本轮没有新增已确认的非 JSON 阻塞缺陷。最后 UI 读数时系统锁屏，未绕过锁屏，锁屏后没有继续 UI 操作；上述关键闭环结果均来自此前实机操作。完整第三方应用 / 输入法矩阵、Windows、签名公证与发行安装升级、长时间 soak / 端到端帧耗时继续列入 1.0 发布验收，不伪装成本轮已经测完。

业务取消不强制中断已经开始的 OS blocking I/O；既有通用异步 Source options / metadata 的 Any、实际工作者 admission / 清理与资源复验保持。当前候选包为 Build 6；旧 Build 1–5 对应此前源码，不能用于验证这次 JSON 清理。

## 下一阶段

2026-10-16 用户调整优先级：0.9 收尾后先实施 **V0.10 剪贴板历史与搜索**，完成后再推进 Terminal 和其他能力。历史保存 / 搜索与现有自动填入 15 秒规则分开，细节见 [产品路线图](../roadmap.md)。本轮不提前开始该功能开发。

## 验收边界

- 保持现有 ActionId、ProviderId、ContextType、Scope、Usage、快捷键和存储 namespace。
- 功能关闭后识别、候选、Source 和快捷键一致失效；执行检查当前资格与资源。
- 菜单重扫、动态项目动作、数据刷新、取消 / admission 与已启动任务管理保留。
- 原生页面键盘、IME、Esc 和 15 秒恢复复用已有规则。
- JSON 工具、第三方 Web / 热加载、NativeProcessRuntime、Terminal 与 Recent Projects 不混入本轮。

每阶段的结果与过渡接口记录到 [开发记录](v0.9-development-records.md)。旧 Build 1 的包、测试和 GUI 边界只对应当时版本；本轮阶段未打包前不能把旧 App 当作新架构产物。正式发行、Windows、整 App RSS / 帧时间和完整系统输入矩阵继续单独验收。
