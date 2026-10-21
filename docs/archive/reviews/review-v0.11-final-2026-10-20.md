# V0.11 最终边界与分层审查

日期：2026-10-20。基线为完整工作区 Build 30；核心修复构建 Build 31；后续呈现核对至 Build 34，UI 尚有未通过项。用户要求包括代码 / 模块 / 插件 / 架构 / 分层 / UI；后续第三方 Context 为调研，未在本轮开放 API。

## 确认并修复

| 级别 | 代码证据与影响 | 处理 / 回归 |
|---|---|---|
| P2 | `WebPackageStore::set_enabled` 在 enabled 未变化时仍准备并发布新目录；目录替换会清空 invocation，导致重复启用也让已打开页面失效 | 同值设置直接返回，未知包仍拒绝。新增打开真实绑定后重复启用，验证原绑定仍有效的回归；修复前此测试失败 |
| P2 | `WebPackageRepository::stage` 遇到同摘要目录即返回；解压资产损坏后重新批准同一包显示成功，但页面仍在资源校验处失败 | 复用前重新验证归档摘要和全部解压资产；损坏时安装失败，不覆盖已安装目录 / 发布假成功。新增篡改资产后重新批准失败、卸载再安装恢复的回归；修复前此测试失败 |
| P2 | `WebViewAppearance.apply` 等待原生外观配置和 JavaScript 执行时没有超时；ready 流程等待它完成，原生永不回复可造成永久加载 | 两处等待分别限制为 2 秒；保留注入样式的可读回退和过期页面检查。测试模拟永不回复、迟到原生结果与脚本执行停滞，确认流程结束且不应用迟到配置 |
| P3 | 搜索 / Action Matcher 从 `anywhere-plugin::web::input` 调用输入识别，识别逻辑归属包加载模块；自动填入、匹配、执行复验的依赖方向不清楚 | 移到 `core/context/input_patterns.rs`，原始语法 / 深度 / 字节预算和四项格式回归一并迁移，三个入口继续复用同一实现。包校验模块不再导出输入识别器 |
| P3 | Context 投影另写一份 1 MiB 上限，可能与公开 Bridge 控制协议漂移；部分注释仍将已完成的 P5 写成未来工作 | 投影直接使用协议限额；更新模块职责和过期注释。更新当前架构 / 文档导航的版本及 Context 注册边界 |

重新安装失败的恢复入口是正常卸载 / 再导入；本轮未做“在使用中的损坏目录原地覆盖”或扩大文件权限，也不把磁盘恢复包装成新的插件能力。

## 审查后保留的边界

- Core：Context / Action / Ranking / 执行资格；Application：采集、目录、包管理与能力代理；Persistence：文件 / SQLite；Platform：系统差异。门禁继续阻止 Core 导入具体业务装配和 Persistence 反调 Application。
- 第一方仍为编译内功能，设置只改变贡献选择与资格。第三方 Web 包通过批准、最小 Invocation、显式 SDK / Broker 接入，不重建第一方生命周期，不开放动态 native 代码加载。
- SDK、闭合 RPC、Host 确认与包权限继续分离。命令匹配成功不等于取得资源读权；前端的取消 / 状态不能替代 Rust 资格与限额。
- 文件 / 剪贴板列表与预览有界；UI 不复制业务识别、排序或存储。文件顺序、隐藏后 15 秒保留、Current App 菜单 Scope 保持现行契约。
- AppStyle / 公共设计参数提供 Flutter 与 Web 默认样式；JSON 只保留业务布局与计算。真实首帧与焦点留在平台适配，超时和透明回退由 Host 负责。
- 未因文件行数单独拆分 Launcher / Web Surface 的版本状态。已有文档寿命、资源准入、画布、消息、Host 弹层等模块继续各司其职，避免拆分后复制同步条件。

## 第三方 Context

当前 Manifest 的 accepts / inputMatchers 不注册 Provider。新的不可信 Context 需要类型所有权、来源与权限标签、资源句柄、输出校验和取消 / 撤销，不能把内部 ContextProvider Trait 或 ResolvedContext 直接暴露给网页。

建议共享主 Graph / Action / Scope / Ranking，按提供方隔离权限和运行环境。UI 继续 WebView，无界面复杂派生先验证独立 QuickJS worker；类型 / 权限契约先行，不同时引入另一套 DSL 或第二个重型运行时。依据、技术比较和验收门槛见 [第三方 Context 调研](../../research/third-party-context-providers.md)。

## 验证边界

修复前两项包管理回归均失败，日志 `build/final-review-regressions-before31.log`；修复后 Web 相关回归与两个超时测试通过。全量工程检查及最终 App 记录以 [V0.11 MVP](../../mvp-v0.11.md) 的 Build 31 为准。

本轮没有新引擎内存性能数字，不宣称跨平台、所有第三方自定义 UI、OS 沙箱或任意代码安全已完成。QuickJS worker 仍为方案，未增加任何脚本执行权限。


## 未闭环的实机问题

WebView 冷开偶尔正文空白，点击 Host Actions 后可见。ready / 原生外观配置与 snapshot 成功不能证明最终组合首帧正确；单元测试没有覆盖这一系统合成路径。已撤回无效的强制重绘 / 帧等待 / 额外分隔线实验，未切换渲染器或终止系统 GPUHelper。加载信息改放 Host 底栏，保留透明画布和共享样式。

Mac 锁定中断了后续定位。该问题解决并完成冷开 / 重开 / 隐藏恢复验收前，不能宣布本轮 UI 全部收尾；本轮没有提交或推送。详情见 MVP 的 Build 32–34 记录。
