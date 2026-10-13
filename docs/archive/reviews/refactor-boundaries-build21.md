# Flutter / Rust 边界重构收尾 — build 21

> 历史记录：仅描述文中日期与构建，不作为当前使用或实现契约。当前入口见 [文档导航](../../README.md)。

2026-10-12，`dev-0.7` 完整工作区；`0.7.0+21` / Rust `0.7.0-dev.21`。对照 [原始 Review](review-flutter-rust-boundaries-2026-10-12.md)，逐项修复与收口，不改变 Scope、Context、现有 UI 风格、连续文件列表和隐藏保留 15 秒的产品契约。

## 完成项目

| Review 项目 | 实现位置 / 调整 | 验证 |
|---|---|---|
| 授权旧 readback 覆盖新设置 | SettingsController 保存队列立即失效旧请求；授权每个 await 后复验，局部结果一次发布；过期返回也解除 busy | settings_transactions_test：设置/status 迟到、dispose、并发直接保存、结束 busy 通知 |
| app.* 合法插件误判菜单 | Handler / Candidate / DTO 明确 is_menu；空父路径真实菜单同样有效 | Rust architecture 合法 app.search 与顶层 MenuAction；Dart snapshot_delta_test |
| Scope 规则重复 | Rust 提供 Scope action_ids/menu_ids；Flutter 只按成员呈现首页/菜单，查询结果直接消费 | Current App/功能 Scope、菜单浏览、合法普通插件 UI 回归 |
| 文件对象与动作混用 | file_kind 与 CandidateKind 独立；二级行是 Action；图标消费对象语义，执行凭证仍验证真实目标 | Rust 文件操作/执行复验；Flutter 图标、过滤、精确对象与预览回归 |
| 预填与时效策略 | Rust input_policy 选择推荐输入；Flutter 仅判断用户是否已编辑与展示 | Rust selected 优先、未知复制时间、15 秒边界/迟到/整数溢出；Dart 预填/Esc 回归 |
| 共享限制 | runtime-policies.json 生成三端常量；file-types.json 生成协议枚举/wire/Rust 分类/Dart 标签 | 两个生成器 --check；Dart/Swift UTF-8 多字节边界一致性 |
| 文件领域耦合通用 Source | source_runtime 只管 admission/取消/timeout/批次；search_sources 与 file_workspace 分开；普通 request 不带文件 payload | 非文件 ConcurrentSource 用例；duplicate SourceId 拒绝，关闭 channel 后取消仍释放 permit |
| UI 状态职责过重 | SessionCoordinator 管身份、保留与 delta 合并；FileWorkspaceCoordinator 管请求隔离/返回位置；LauncherNavigation 管纯导航 | 保留/复唤醒/焦点、工具返回、旧更多/预览/二级请求、键盘回归 |
| Bridge 组织 | 内部 queries/files/shortcuts/settings/executions/conversions；公开 DTO/FRB 门面保留，生成代码重新生成 | 生成、编译、Flutter analyze、真实打包 |
| Context label 混入图算法 | read model 移到 Application context_projection；Core 保留图身份/根算法 | 原 Context/Scope/投影测试与架构护栏 |
| 累计列表重复构造/传输 | Arc 不可变候选数组；冻结进度复用执行表；追加仅复验新行；Replace/Retain/Append 两个端口 | Rust progress/delivery、Dart 10k 行复用、订阅先/加载先的真实 Controller 顺序、替换重同步 |

责任划分：Rust 决定 Context、匹配/排序、适用性、对象/菜单语义、业务时效和执行；Flutter 决定布局、图标颜色、用户导航、焦点/滚动和草稿；Native/Platform 提供系统服务。配置 UI 即时反馈和 Host/Rust 权威校验可以共存，限制来自同一资源。

## 测量与边界

Release 模式，同机单次微基准；每组 100 次：

| 已加载行数 | 完整 DTO 转换 | 仅进度 DTO 转换 | Core 进度提交 |
|---|---:|---:|---:|
| 1,000 | 18.825 ms | 0.033 ms | 0.054 ms |
| 10,000 | 116.904 ms | 0.013 ms | 0.023 ms |

仅进度包传输 0 行；追加测试传 64 行。微基准只说明这一代码路径，数字存在调度/缓存误差，不代表逐帧性能或整 App RSS。Core 测试同时断言 Arc 指针相同、序号/进度已更新、执行表未重建，避免只测“根本未提交”的空路径。

列表版本改变、数量越界时 Bridge 返回完整替换；客户端基于已改变的列表收到旧 tail 时拒绝合并并重新等待；订阅先到与 load-more 先到均不重复追加，↓ 可以继续选择新增对象。新的查询、配置或 catalog 仍有原凭证校验。不是通过候选 kind/file_kind 授权执行。

已加载候选/执行表仍随浏览数量增长；Arc 的追加 copy-on-write 和 Dart 追加列表仍有累计复制成本。未恢复分页，未加入长期文件索引，未以测试速度宣称恒定内存。

内部 Any payload、Source trait 和 Host read model 没有升级成稳定第三方 Extension API；V0.8 的外部协议、能力授权和插件运行时仍按 roadmap 单独验证。本次完成当前审查要求的收口，不提前实现全部未来功能。

## 验证

- 最终 `scripts/check.sh`：**352 Rust / 199 Flutter 通过，6 Rust ignored**；Native menu / Host / 目录授权 / 材质 / 输入焦点、fmt、Clippy、架构、生成检查与 Flutter analyze 通过。
- 新增关键回归：settings_transactions_test、snapshot_delta_test、文件 delta 竞争顺序、源 ID 唯一性、runner 取消释放 admission、UTF-8 配置边界。原文件路径/符号链接/动作重载/权限及 15 秒状态保留回归继续通过。
- 日志：`build/validation/refactor-build21-check.log`、`refactor-final-integration.log`、`refactor-final-codegen.log`、`refactor-dto-measure.log`、`refactor-progress-measure.log`。

本次未提交或推送 Git；保留工作区此前修改。

## 打包后实机复验

- 首次实机检查发现 Open File 二级行与原 File 对象共用 CandidateId，完整执行表重建后原对象被 Action 行覆盖，预览拒绝目标。修复为 `file-action:` 候选命名空间；业务 ActionId / Usage 不变。新增 Core 回归在同一个对象上重复打开操作面板、验证对象仍是 File、预览仍返回正确文本。
- 修复后重新启动包：Global Search 直接输入 files、Enter 进入二级工具、继续输入绝对目录；130 项 fixture 全部计数，滚动连续追加到 report-129.md；首项与末项 Preview 内容正确，↓ 跟随选择，Esc 关闭预览后查询仍保留；追加末项再次打开 Actions 返回精确路径。没有执行外部文件、Copy Path 或启动后台命令。
- 自动化原生焦点/Flutter 保留/Core 暂停恢复与超时回归通过。CUA 的外部 Finder 点击未产生可确认的 `launcher_dismissed/focus_lost` Host 事件，因此不能把这次工具点击记为真实桌面失焦的 15 秒 GUI 验收；物理鼠标/热键与完整系统隐私矩阵仍需实机验收。
- 最终包保留 Impeller；deep/strict 签名、与 build 20 同一 designated requirement、四个主二进制 arm64、ZIP 37 个文件与 8 条符号链接逐项比较及 SHA-256 核验。

产物：`build/Anywhere-dev-0.7-build21.app`、`build/Anywhere-dev-0.7-build21-macos-arm64.zip` 和 `.sha256`。包装日志与逐项结果：`build/validation/refactor-build21-package.log`、`refactor-build21-package-check.json`。
