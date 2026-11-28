# 历史验收与审查

以下内容保留原日期、版本和实验边界。**不作为当前使用或实现契约**；当前状态从 [文档导航](../README.md) 和 [V0.14 MVP](../mvp-gpui.md) 阅读。

## 版本记录

- [Windows 桌面验收（2026-11-27）](windows-acceptance-2026-11-27.md)保存 r16
  产物哈希、47 条桌面矩阵结果、工程门禁、r15 回归基线和仍未关闭的产品、
  环境、自动化及发布问题。

- [Windows 桌面抽测（2026-11-26）](windows-desktop-smoke-2026-11-26.md)记录 x64 本机包、工程回归失败和单机桌面抽测。

- [V0.14 方案与 Build 1–23 过程记录](v0.14-development-records.md)：旧设计、功能范围调整与逐构建证据；当前契约另见现行专题指南。

| 文档 | 保留价值 |
|---|---|
| [V0.3.x MVP](mvp-v0.3x.md) | 菜单长期身份、历史不误关联、执行不猜测的原验收边界 |
| [V0.4 Context 验收](v0.4-context-acceptance.md) | 来源与系统权限矩阵，工程检查与发布的区别 |
| [V0.5 MVP](mvp-v0.5.md) / [收尾](v0.5-closeout.md) | Developer / Process Session 范围、真实工具与交付证据 |
| [V0.6 MVP](mvp-v0.6.md) / [最终核对](v0.6-final-acceptance.md) | 快捷键、迁移、UI / 平台切片和物理输入待验收 |
| [V0.7 MVP](mvp-v0.7.md) / [收尾](v0.7-closeout.md) | 文件搜索演进，长期索引 / rg 方案被替代的过程 |
| [V0.7 build 14](validation-v0.7-build14.md) | 对应 fixture 与可见 App 的实测边界 |
| [V0.8 开发记录](v0.8-development-records.md) | S1–S3、build 4–16 的回归、包校验、已知实机边界 |
| [V0.9 开发记录](v0.9-development-records.md) | Detail / KeyValue 的协议、执行、Host 回归与开发包 |
| [V0.8 MVP](mvp-v0.8.md) | 旧模块 / 协议及验收边界，已被静态内置模块替代 |
| [V0.9 MVP](mvp-v0.9.md) / [模块迁移](feature-module-roadmap.md) | 已完成的第一方热加载退役和原生工具页迁移 |
| [V0.10 剪贴板记录](mvp-v0.10.md) | 剪贴板历史、存储 / 格式边界及初始验收 |
| [开源复用重构](open-source-refactor-plan.md) | 已完成的 R1–R4 与未采用原型 |
| [V0.11 MVP](mvp-v0.11.md) / [Web 实施记录](web-plugin-development-records.md) | Web 平台、JSON 与逐构建观察；不作为现行 SDK 文档 |
| [Flutter 3.50 升级与内存调研](flutter-3.50-memory.md) | GPUI 迁移前的 Flutter SDK、界面、内存与 Windows 共享层记录；不描述当前产品 |

## Review 与重构记录

- [全仓代码审查修复方案（2026-11-18）](reviews/code-review-remediation-plan-2026-11-18.md)：锁中毒策略、Token 调色板与 Web 主题生成、键盘分层与 action 迁移、`LauncherView` 拆分、通用列表行、Release 配置及测量门槛项的实施方案与 PR 顺序；撰写时未实施。

- [当前工作树综合审查（2026-11-18）](reviews/worktree-review-2026-11-18.md)记录文件分页、剪贴板、任务、共享快照投影、应用索引、原生桥接和 SDK 修复，以及桌面验收步骤。

- [GPUI Regular 玻璃调整（2026-11-17）](reviews/gpui-regular-glass-2026-11-17.md)：所有原生窗口使用 Regular、移除页面和普通结果行补色蒙版，保留系统辅助功能回退与交互层；通知对照待实机验收。

- [GPUI 构建验收执行记录（2026-11-17）](reviews/gpui-acceptance-run-2026-11-17.md)：按 322 条清单执行本轮构建的工程门禁、包身份、隔离启动、输入、Tasks 与缺陷记录；macOS 锁屏后的模块保持 Blocked / Pending。

- [GPUI 对象操作、设置与 Web 生命周期审查（2026-11-17）](reviews/gpui-lifecycle-review-2026-11-17.md)：Core 对象请求抢占、旧 Actions 回包、设置保存隔离与排队退出、Web Reload / 隐藏恢复；包含可控排序回归和待执行的人工清单。

- [GPUI 图标、任务与内存审查（2026-11-16）](reviews/gpui-performance-task-review-2026-11-16.md)：空筛选操作资格、图标失败重试、任务侧栏滚动、新任务选择与共享输出；包含配对分配测量、双分支包与人工验收清单。

- [GPUI 异步与选择审查修复（2026-11-16）](reviews/gpui-async-selection-review-2026-11-16.md)：首次剪贴板查询版本、旧操作 loading、在途预览授权、Shelf 选择同步、应用历史加载收尾与菜单开关；包含确定性回归和待执行的人工步骤。

- [GPUI Web 插件白底与输入误传排查（2026-11-16）](reviews/gpui-web-plugin-background-review-2026-11-16.md)：确认 macOS 透明 feature 未启用，以及名称搜索首次状态误传正文；透明 feature、执行 revision 基线、Web scheme 与固定深色面板已实施并补回归，保留物理验收边界。

- [GPUI 修复复核与玻璃透明度方案（2026-11-16）](reviews/gpui-ui-followup-and-glass-plan-2026-11-16.md)：针对 `b428899` 记录 4 类问题与定向诊断；正常模式全窗遮罩透明、语义字色、模态路由、有效尺寸与 Web 同步已实施，保留物理材质与辅助功能验收边界。

- [GPUI UI 审查与解决方案（2026-11-16）](reviews/gpui-ui-review-2026-11-16.md)：窗口 / 列表过渡范围、Scope、提示布局、Shelf、设置模态与剪贴板控件，以及真实 viewport 验证缺口；15 项已在工作区实施并补充自动回归与 v166/v167 真机证据，未覆盖的物理矩阵在文末逐项保留。

- [生命周期与错误语义复查（2026-10-30）](reviews/lifecycle-followup-review-2026-10-30.md)：旧页面取消新插件审批、后台 SDK 成功写入误报失败、全局命令查询的重复分配；三项已在升级分支工作区修复，保留初始复现与后续验证。

- [跨层正确性与资源审查（2026-10-30）](reviews/cross-layer-review-2026-10-30.md)：设置目录并发、后台输出分配、取消导入后的列表恢复、查询防抖与插件契约一致性；五项问题已在升级分支工作区修复，保留复现与后续验证。

- [升级后资源与设计审查（2026-10-29）](reviews/resource-design-review-2026-10-29.md)：SDK 状态一致性、历史查询恢复、长按重复操作、输出分配与空闲轮询；六项已在升级分支工作区修复，含修复前复现与后续验证。
- [Rust / Flutter 稳定工具链升级（2026-10-29）](reviews/toolchain-upgrades-2026-10-29.md)：Rust 1.99.1 / Flutter 3.47.8、可兼容依赖升级、安全扫描与上游约束；承接同日依赖整合。
- [依赖升级整合（2026-10-29）](reviews/dependency-upgrades-2026-10-29.md)：15 个升级分支、SQLite / FRB 兼容修复、隔离工具与真实 FFI 验证；升级保留在独立分支。
- [全仓分层 Review（2026-10-29）](reviews/repo-review-2026-10-29.md)：Core / Application / Features / Persistence / Platform / Bridge / Flutter / Native Host / SDK / 构建与文档的检查范围、五项修复及自动化证据。
- [V0.12 最终代码与仓库收尾（2026-10-23）](reviews/closeout-v0.12-2026-10-23.md)：目录额度、设置错误、旧接口 / 文案、干净检出、原生 SDK 兼容与 main 集成。
- [V0.12 文档与实现一致性审查（2026-10-23）](reviews/docs-alignment-v0.12-2026-10-23.md)：自动输入、交互、模块路径、Web SDK、安装 / 撤销与后续能力边界。
- [V0.11 Build 51 性能与交互收尾](reviews/closeout-v0.11-build51.md)：组词查询、预览复用、复制延迟、设置提示与 JSON 文本复用。

- [V0.11 Build 47 运行时验收](reviews/verification-v0.11-build47.md)：剪贴板异常边界、空设置窗口释放、移除 Web 首帧截图路径。
- [V0.11 Build 36 过期界面资源释放](reviews/memory-v0.11-build36.md)：闲置解码缓存、撤销历史与隐藏帧边界。
- [V0.11 Build 35 内存优化](reviews/memory-v0.11-build35.md)：大文本派生峰值、HTML 字节副本及测量范围。
- [V0.5 可维护性](reviews/review-maintainability-v0.5.md)。
- [V0.7 架构](reviews/review-v0.7-architecture-2026-10-12.md)。
- [Flutter / Rust 边界](reviews/review-flutter-rust-boundaries-2026-10-12.md)。
- [build 21 边界收尾](reviews/refactor-boundaries-build21.md)。
- [V0.8 综合 Review](reviews/review-v0.8-2026-10-13.md)。
- [V0.11 最终边界与分层 Review](reviews/review-v0.11-final-2026-10-20.md)：Build 31，包管理幂等性、损坏资源复用、WebView 超时与识别归属。

Review 的问题列表描述审查基线，修复记录描述其后变更；不能从旧“仍有问题”段落直接推断当前代码仍有该缺陷。菜单永久语义身份仍未验证的限制继续属于 [当前契约](../current-app-actions.md)，没有因归档而取消。

`build/validation`、截图、开发 App 与 ZIP 路径是当时本机材料，不保证另一台机器或 Git checkout 中存在。源码行号标为历史位置，避免跳转到后来版本的不同行。

## 历史基准数据

[基准说明](benchmarks/README.md) 保留长期索引原型与 V2 遍历器的两个 JSON 样本。原型未进入生产，遍历器采样也不包含 Flutter / Spotlight / 原生窗口，不能作为当前整 App RSS。

- [V0.11 架构与收尾 Review（2026-10-20）](reviews/review-v0.11-2026-10-20.md)

- [2026-10-20 第三方 Context 调研](third-party-context-providers-2026-10-20.md)：早期隔离与引擎选型建议，实施状态另见[当前说明](../research/third-party-context-providers.md)。

## 2026-11-17 归档整理

- [GPUI 迁移契约](gpui-migration.md)、[UI 对齐历史](gpui-ui-alignment-plan.md)、[旧声明式协议](plugin-protocol.md)和[早期重构切片](code-cleanup-and-gpui-runtime-plan.md)。
- [V0.14 历史开发与发布前验收记录](v0.14-gpui-development-records.md)与[GPUI 前内部构建变更](changelog-pre-gpui-2026-11-17.md)。
- [当前工作树实施记录](reviews/full-worktree-remediation-2026-11-17.md)。
