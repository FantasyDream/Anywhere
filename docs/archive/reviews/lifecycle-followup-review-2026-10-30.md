# 生命周期、错误语义与查询分配复查（2026-10-30）

基线：`codex/dependency-upgrades@c98a8e7` 加上当前未提交的两轮修复及 Build 36 元数据。本次先复跑上一轮相关回归，再继续审查；未回退已有修改，未把已经修复的问题再次列为开放问题。

检查范围包括 Flutter Launcher / 设置 / 应用历史 / 剪贴板 / 任务输出 / 工具栈的状态与生命周期，Web 包安装 / 文档 / 资源池 / SDK / 后台执行链，以及 Rust 匹配 / 快照投影 / 持久化边界。以下仅列有当前代码与受控证据支持的新问题，不代表全仓每行或所有原生交互已经验证。

## 确认的问题

### R1 / P2：离开页面的旧导入流程可以取消新页面的插件审批

位置：`apps/flutter/lib/web_plugins/web_packages_section.dart:57–59`、`:127–134`；`apps/flutter/lib/infrastructure/web_package_backend.dart:15`；`crates/anywhere-core/src/application/web_plugins/packages.rs:94–95`。

导入流程在 `inspect` 返回后发现页面已卸载，会正常退出，但 finally 仍调用没有 token 参数的 `cancelReview()`。后端 `WebPackageStore` 只有一个共享 `pending`，取消时无条件清空；页面本地的 `_busy` 与 `_generation` 无法保护新页面取得的审批记录。设置切换页会卸载这一节，并不等待其安装流程。

受控 Widget 复现：旧页面开始 inspect，后端已生成旧 token，暂不交付响应；切换到新页面并开始新 inspect，显示新的批准对话框；此时交付旧响应。旧页面 finally 清空新 pending，随后点击新页面的批准按钮，安装因 review expired 失败。结果为 `pending_before_approval=null / installs=0`，期望新审批能完成。

建议让取消操作带上对应 review token，只清理该操作拥有的记录；同时明确包管理操作与页面展示的寿命，避免用增加 mounted 检查代替共享状态的所有权检查。批准时仍应坚持精确 token / 内容复验。本问题造成合法安装被拒绝，探针没有证明能够绕过批准。

### R2 / P2：后台 SDK 的输入写入已成功，却因附带读回失败改报写入失败

位置：`packages/plugin-sdk/src/background.ts:63–68`；预算入口为 `crates/anywhere-core/src/application/web_plugins/background_runtime.rs:48–65`。

后台 `input.set` 在宿主确认写入后，清空 snapshot 并 `await state()`。这使可选的状态同步失败成为整个写入 Promise 的失败；与 Web SDK 及当前开发指南中“读回失败不会把已经成功的写入改报为失败”的契约不同。

已用生产 QuickJS Runtime 和当前编译后的后台 SDK 确认一个明确触发条件：同一任务先完成 127 次合法业务读取，再执行 `input.set`。第 128 次业务调用已成功写入，SDK 隐式增加的第 129 次 state.get 被预算拒绝，最终结果为 `writeResult=QUOTA_EXCEEDED / acknowledged_writes=1`。探针使用合成宿主回调，不调用真实搜索框或读取个人存储；调用额度与 JS 执行均来自现有 Runtime。

这会让后台命令中止剩余步骤或显示失败，输入却已经改变，也可能诱发调用方重试。建议以写入确认决定写入 Promise，独立处理快照失效及订阅同步；读取失败不应污染成功写入的结果，也不应自动重试有副作用的调用。需要补充读回失败的回归，现有订阅者抛异常隔离测试没有覆盖这一分支。

### R3 / P3：全局应用命令搜索逐项重建标题搜索字段

位置：`crates/anywhere-core/src/core/action/matcher.rs:135–155`；注册时缓存位于 `core/action/descriptor.rs`。

每次非空、无 Scope 的查询，对所有带 owning_app 的动作都调用 `SearchFields::new(&action.title, &[], &[])`，重新分配字段 Vec 和小写标题；随后又使用注册时的 `action.search()` 匹配。标题检查还会执行完整的匹配回退，即使这个分支只接受 WordPrefix 或更高等级。

直接调用生产 ActionMatcher，预先注册并预热 1,000 条合成应用命令，以相同的未命中查询运行两条路径：无 Scope 时 **3,019 次分配 / 44,341 字节累计申请**；限定同一组时 **1,000 次 / 9,000 字节**，两次均返回 0 条。差异为每次查询约 **2,019 次 / 35,341 字节**，主要来自逐条标题字段构建，另有一次应用限定词处理。统计不包含注册、Query 创建及打印。

建议在动作注册时保留标题专用匹配数据，或提供不创建临时字段的标题字面匹配路径，保留 Finder 的 Find 等直接标题优先规则。不应直接用包含 aliases / keywords / search_title 的现有字段替代标题判断，否则会改变限定词语义；也无需增加另一套搜索服务。这里量化的是临时分配次数与累计申请字节，不是存活堆、峰值、RSS、泄漏或实机延迟。

## 验证与边界

- Flutter 上一轮相关子集 **77 项通过**：`build/review-followup-oct30-baseline.log`。
- SDK 本轮重新构建；普通 Node 套件 **26 项通过、3 项 Rust 互通用例因未启用测试宿主而跳过**：`build/review-followup-sdk-baseline.log`。不将跳过计为通过。
- R1 的缺陷期望 Widget 探针失败：`build/review-followup-lifecycle_test.dart` / `.log`。使用实际页面及合成共享 Store，取消接口和 Store 的无条件清理另与当前 Dart / Bridge / Rust 调用链核对。
- R2 的 Node 探针及实际 QuickJS Runtime 探针均显示一次成功写入后 Promise 返回 QUOTA_EXCEEDED：`build/review-followup-background.mjs` / `.log`、`build/review-followup-background-runtime.rs` / `.log`；SDK 仅在临时文件中拼接为无模块加载依赖的输入，源码未修改。
- R3 的生产匹配器计数：`build/review-followup-search.rs` / `.log`。两个临时 Cargo example 运行后已移出源码树，原始材料留在被忽略的 build 目录。
- 完整 `scripts/check.sh` 本轮重新执行并 exit 0：Core **329 / 1 忽略**、MVP **195 / 4 忽略**、Platform **70 / 1 忽略**、Flutter **483 / 1 跳过**；启用真实 Rust 测试宿主后 SDK **29 项全部通过**，JSON **30 项通过**。规范、Clippy、原生自动检查、架构与生成一致性、文档检查通过。日志：`build/review-followup-full-check.log`。完整门禁通过不包含上述缺陷期望探针，也不表示新问题已修复。

本轮只新增审查记录及索引，未修改产品实现、提交 / 推送、重新打包或运行新 App。原生 GUI / 物理输入法、整 App 内存、远端 CI 及 Windows / Linux 运行仍未在本轮验收；不将局部时序探针或分配计数当作这些验收的替代。

## 后续修复与验证（2026-10-30）

用户要求修复后，R1–R3 已在 `codex/dependency-upgrades` 工作区完成；上文保留初始审查状态。

| 问题 | 最终处理与回归 |
|---|---|
| R1 | Flutter 保存本次 inspect 的 token 和对应后端，离开页面后的清理仍携带该 token；Bridge 透传 token，Rust 仅取消匹配记录，过期 / 重复取消为无操作。Widget 受控覆盖旧页面迟到清理后新审批仍成功、当前审批取消仍释放；Rust 覆盖旧 / 未知 / 重复 token、不影响新审批及取消后不能批准。 |
| R2 | 成功写入使缓存失效；无订阅者不主动读回，有订阅者尝试同步但不让读回错误改报写入失败。失败读取的 Promise 从缓存移除，后续 getter 可重新读取；真正写入失败仍原样拒绝，写入不自动重试。新增回归覆盖惰性读回、HOST_FAILURE / QUOTA_EXCEEDED 读回失败与后续读取、写入拒绝；原订阅者异常隔离继续通过。 |
| R3 | 直接标题判断复用注册时的主搜索字段，显示 / 搜索标题不同时才保留单独的小写显示标题；没有每查询字段构建或拼音回退。矩阵回归对照原标题匹配语义，覆盖别名 / 关键字 / 本地化隔离、不同或空搜索标题、空标题、中文、标点及 Unicode 大小写；原 Finder Find / 应用限定词端到端回归继续通过。 |

修复前，新 Widget 缺陷期望失败，新增的三条 SDK 惰性读取 / 读回错误期望也失败。修复后插件页面子集 **10 项通过**、后台 SDK 子集 **7 项通过**，两条 Rust 新回归与原应用限定词 MVP 回归均通过。

沿用上文生产 Runtime / SDK / 127 次先行读取的夹具，结果从 `writeResult=QUOTA_EXCEEDED / acknowledged_writes=1` 变为 **`writeResult=fulfilled / acknowledged_writes=1`**。这是同一次成功写入的结果修正，没有放宽宿主额度或重试写入。

沿用相同 1,000 条合成动作和未命中查询，生产匹配器的每次分配由 **3,019 次 / 44,341 字节** 降至 **1,019 次 / 9,451 字节**；减少 2,000 次分配、34,890 字节累计申请。限定组仍为 1,000 次 / 9,000 字节，两条路径结果均为空。统计不含注册 / Query 构造；普通标题复用现有小写文本，仍有按动作保存的可选字段元数据，显示名不同的额外缓存随该动作存活。不是零存储开销、RSS 或存活堆下降的声明。

完整 `scripts/check.sh` exit 0：Core **331 / 1 忽略**、MVP **195 / 4 忽略**、Platform **70 / 1 忽略**、Flutter **485 / 1 跳过**、SDK **33**、JSON **30**。规范、Clippy、原生自动回归、架构 / 生成一致性与文档检查均通过。审批取消 Bridge API 已重新生成两端绑定，Bridge crate 已编译；真实动态库 Dart FFI 回归另行运行 **1 项通过**，没有把普通 Flutter 套件跳过的该项计入 485。

探针临时 example 已删除，原始日志保留于 `build/lifecycle-fixes-*.log`。本轮未提交 / 推送、重新打包、启动新 App 或更新已安装插件；Build 36 不含本轮修复，后台 SDK 修复需重建并导入对应插件。GUI / 物理输入法、整 App 内存、远端 CI 与跨平台运行未新增验收。
