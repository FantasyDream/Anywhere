# GPUI 对象操作、设置与 Web 生命周期审查

日期：2026-11-17。审查基于 `codex/experiment-gpui-kit` 的 `b428899` 及当前
未提交工作区；保留已有改动，不修改 main / Flutter。

## 确认的问题与修改

| 编号 | 级别 | 可达问题 | 修改 |
| --- | --- | --- | --- |
| L01 | P2 | 两个对象请求同属一个 revision，较新请求已开始时旧请求仍能发布，递增 inputs_epoch，反过来使新请求失败；原代码只递增 object_action_epoch 而没有复验 | 请求冻结对象代次，发布前在 Session 锁内复验；分离准备和解析阶段，保留原匹配、排序和执行契约 |
| L02 | P2 | GPUI 对象动作回包直接 apply_shared_state，绕过 SnapshotGate；旧状态回滚 query / revision，同一会话的旧错误还会关闭后来打开的面板 | 绑定请求代次、父对象、页面、会话与 revision，检查面板仍打开；状态通过共享 cursor 门禁后发布 |
| L03 | P2 | Launcher 查询 / follow 错误直接清掉 settings.saving；即时开关也可在偏好保存期间提交并提前清掉标志，产生重复写入和错误的关闭决定；通用 Launcher 代次变化还会丢弃开关完成回包 | 偏好、录制与功能开关使用独立操作代次和回包通道；保存中禁用即时开关并在回调复验；实际快照负责成功 / 失败回滚，通用查询错误不改设置保存状态 |
| L04 | P2 | 保存成功后排队执行退出，执行时只检查仍在 Settings；期间“继续编辑”或新增草稿也会被退出覆盖 | 退出执行前复验关闭意图、保存状态和当前草稿，再消费退出意图 |
| L05 | P2 | Reload 先异步关闭旧文档再重绑；关闭先完成会删除 invocation，后续重绑失败；重绑先完成时旧关闭错误也可能污染新页面 | 先请求替换绑定，成功后丢弃旧原生视图并静默处理旧 handle 清理；保留当前文档以支持失败后重试，拒绝打开 / 恢复期间重入 |
| L06 | P2 | 普通 Web 打开回包仅绑定 Launcher 代次，返回后可能装入无主文档；恢复通道关闭时 busy 不结束；ready=false 还混淆首次加载与隐藏恢复，独立 Settings 入口没有关闭预览 / Web 资源 | 打开和恢复绑定各自请求代次及目标，旧回包释放自己的 handle；明确 needs_resume，隐藏只存元数据，恢复成功后才挂载 / 发可见 IPC；通道关闭结束 busy；Settings 入口先清理预览与 Web |

L01 按“先准备 A、再准备 B、A 先到发布点”的可控顺序复现；修复前 A 成功发布。
L02、L03、L04 和 L05 的首批 5 个 GPUI 回归均在修改前失败：revision 5 回滚到 4、
保存标志被清空、取消 / 新草稿后仍退出、Reload 立即清空当前文档。
另补一项入口回归，在原代码上复现进入 Settings 后 preview_open 仍为 true。
这些状态回归不代表真实 WebKit 已完成物理验收。

实现入口：

- [object_actions.rs](../../../crates/anywhere-core/src/application/object_actions.rs)：请求冻结、匹配与发布前复验，以及可控顺序测试。
- [view/mod.rs](../../../apps/gpui/src/launcher/view/mod.rs) 与 [model.rs](../../../apps/gpui/src/launcher/model.rs)：对象动作门禁和父对象身份、通用回包隔离。
- [settings/controller.rs](../../../apps/gpui/src/launcher/view/settings/controller.rs)：设置操作代次、失败回滚和退出前复验。
- [web_surface.rs](../../../apps/gpui/src/launcher/view/web_surface.rs) 与 [navigation.rs](../../../apps/gpui/src/launcher/view/navigation.rs)：Web 打开 / 恢复请求和隐藏生命周期。
- [lifecycle_regressions.rs](../../../apps/gpui/src/launcher/view/tests/lifecycle_regressions.rs)：16 项 GPUI 回归。

## 自动化证据

新增 16 项 GPUI 和 1 项 Core 回归。GPUI 使用受控 channel 返回顺序、独立测试
窗口与合成对象；Core 直接控制真实准备 / 发布阶段，不依赖线程睡眠或概率竞争。
覆盖同 revision cursor、较新面板不被旧错误关闭、Launcher / follow 错误隔离、
设置失败回滚与断开、代次变化后完成、即时开关重入、取消 / 新草稿退出、Web
打开乱序 / 返回后回包、打开与恢复断开 / 重试，以及隐藏期间只保留文档元数据。

最终 `scripts/check-gpui.sh` 与 `scripts/check.sh` 均通过：GPUI 共 244 项；完整
工作区 Rust 为 928 passed / 6 ignored / 0 failed。严格 Clippy、格式、生成物、
106 个 Markdown / 745 条本地链接、SDK / 示例、原生菜单探针与架构检查通过。
`git diff --check` 通过。

本轮没有启动或重新打包 App；现有 `build/` 双版本包属于前一轮源码快照，
未宣称包含本轮修改。检查生成的 SDK / 示例目录已清理，保留两种版本的原包。

## 人工验收执行单

全部为 **Pending**。由维护者安排时间，先使用当前源码重新构建，记录 revision、
未提交差异、构建号、macOS / 显示器与语言。按
[独立验收数据](../../development.md#独立验收数据)设置测试数据目录和专用热键；
开关、草稿和插件均使用测试配置，保留日常数据。

| 场景 | 操作 | 预期 | 状态 |
| --- | --- | --- | --- |
| 对象操作乱序 | 文件页对 A 打开更多操作，立刻返回并对 B 打开；重复快速查询、清空和换 Scope | 二级动作只属于当前对象，query / revision 不回滚，旧错误不关闭当前面板 | Pending |
| 同对象重开 | 对 A 连续打开、Esc 关闭、重开，混合鼠标和 ⌘K | 第二次打开正常收尾，旧请求不改其动作或提示，返回恢复 A 的选择 | Pending |
| 设置互斥 | 编辑普通偏好后保存，保存期间点击剪贴板录制 / 内置功能开关；再先切开关并立即关闭 Settings | 保存中开关禁用，结束后可正常操作；成功后按关闭意图退出，不永久停在 Saving | Pending |
| 设置错误 | 隔离配置中制造可控写入失败或拒绝测试权限，再恢复环境重试 | 保存标志结束、错误可见、草稿保留；开关按实际状态回滚，失败不自动退出 | Pending |
| 保存后继续编辑 | 从关闭提示保存，交替点继续编辑、切换语言和修改草稿；分别测试关闭、唤醒和退出意图 | 取消能阻止排队退出；新草稿仍留在 Settings，可再次保存，不丢失修改 | Pending |
| Web Reload | 打开合成 Web 插件并编辑内容，多次 Reload，重载中继续点 Reload；随后测试一次可控加载失败再重试 | 正常生成替换文档，旧 handle 不关闭新页面；打开 / 恢复期间不重复请求，失败后仍可重试 | Pending |
| Web 返回与替换 | 刚打开 / Reload 就 Esc 返回，再执行另一工具；给旧页面制造迟到回复和关闭；从 Web 工具或 Quick Look 打开 Settings | 不出现无主文档，新页面不接收旧结果或错误，不被旧关闭回调退出；Settings 不遗留预览面板和 Web 执行资格 | Pending |
| Web 隐藏恢复 | 首次打开、Reload、恢复期间分别切换应用，15 秒内反复唤醒；再测试超时 | 隐藏立即撤销执行资格；保留期内恢复有效 epoch 后继续，隐藏时不新挂载或发送 visible=true；超时按新会话处理 | Pending |
| 原生键盘与焦点 | 上述序列使用中文 IME、Esc / Enter、方向键、状态栏和全局热键重复 | 组词不被错误触发，返回 / 关闭后焦点正确，快捷键没有跨页重复执行 | Pending |

逐项补 Actual、Pass / Fail、日志或截图。单测、构建、此前的包和截图均不能替代
本表中当前源码的原生窗口、WebKit、物理 IME 与焦点证据。
