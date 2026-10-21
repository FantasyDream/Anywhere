# V0.11 架构与收尾 Review

日期：2026-10-20。范围：当前完整工作区，包含尚未提交的 Web 插件实现。基线 Build 19，修复构建 Build 20 / 21。以下区分代码检查、自动化测试与实际 UI 验收；不把既往构建的内存测量视为本轮结果。

## 审查结果与处理

| 级别 | 确认的问题 | 修复及回归 |
|---|---|---|
| P2 | 包管理把批准、目录发布、Application 调用放进 persistence；保存失败还会重新发布旧目录，使当前文档凭据失效 | [包协调服务](../../../crates/anywhere-core/src/application/web_plugins/packages.rs) 与 [磁盘仓储](../../../crates/anywhere-core/src/persistence/web_packages.rs) 分开；准备、保存均成功后才发布。新增保存失败后原文档仍可验证、原资源仍可用的测试；架构门禁禁止持久化适配器反向调用 Application |
| P2 | Web 页面异步创建返回后如已隐藏 / 销毁，未接收的绑定没有及时关闭；旧 native 回调和 ready 检查的迟到错误可能影响替换后的页面 | [WebDocumentLifetime](../../../apps/flutter/lib/web_plugins/web_document_lifetime.dart) 负责绑定接收与迟到关闭；Surface 为视图回调校验实际 controller，异步 ready 检查返回后复验代数。测试覆盖迟到、替换、销毁与已撤权清理失败 |
| P2 | SDK 取消只结束自身 Promise；文档传输仍保留等待原生回复的名额。原生长期不回复时连续取消会耗尽 32 个本地名额 | [传输](../../../apps/flutter/lib/web_plugins/web_bootstrap.dart) 在取消及原生发送异常时释放名额；SDK dispose 向已发送请求发送尽力取消，未发送的排队请求只在本地结束。运行实际 bootstrap 的测试连续执行 40 次无回复取消、40 次原生发送异常后仍可正常请求 |
| P2 | 设置页初始回读可能覆盖新安装列表，文件选择期间可重入，清理 review 失败会跳过解除 busy | [设置组件](../../../apps/flutter/lib/web_plugins/web_packages_section.dart) 经薄 Backend 调用；回读代数保护、选择开始即单飞、清理异常隔离。测试覆盖旧列表迟到、选择取消、清理异常；操作失败后回读实际状态，防止卸载已提交但数据清理失败时展示旧安装行 |
| P3 | 每个 Web 静态资源请求都会克隆完整 Invocation（文本最多 1 MiB），读文件 / 摘要持有管理锁；卸载 KV 清理还持有 Web 文档锁 | [Bridge 资源端口](../../../crates/anywhere-bridge-flutter/src/api/web.rs) 使用不复制输入的身份检查，锁内只取资源描述，锁外读取并校验摘要，返回前再验身份；SQLite 清理不持有文档锁。减少成本是代码路径结论，本轮没有新的 RSS / 帧率收益测量 |
| P3 | 新 inspect 验证期间仍保留上一份压缩包；新检查失败时旧 review 继续有效 | 新检查开始先释放旧 review；测试确认损坏的新包使旧审批失效，已安装内容不变 |

## 职责与清理

- Rust Core：Context / Action / Ranking / Usage / 执行资格；Application：包管理与能力协调；Persistence：收据、包文件和 SQLite；Platform：系统采集与执行。未发现需要将 Launcher 算法搬入 Flutter 或把 UI 布局搬入 Rust 的理由。
- Flutter `WebToolSurface` 负责平台视图与交互协调；绑定寿命、HTML / CSP 路径、消息 bootstrap、画布适配、Host 弹层各有独立模块。Web 包设置经可测试的类型化端口调用，不处理磁盘或批准规则。
- 删除只供测试使用的 Application Current App / ready 并行入口。使用记录测试改走实际 SDK 使用的 Broker 路径，避免一份生产实现、一份测试实现。
- 合并私有存储 namespace 生成；删除 4 个已被共享页面 / 预览参数替代、全仓无消费者的旧设计参数，重新生成 Dart / CSS。
- 保留 P0 独立探针作为隔离环境的开发复现工具；它不进入生产模块或主检查，也不承担正式授权。保留已使用的旧声明式插件兼容层，不以“旧”作为删除依据。

## 已检查并保留的设计

- LauncherController 仍是较大的协调门面；Session、文件工作区、快捷键、对象 Actions 已有独立协调器。当前不为行数拆分公共状态或复制版本检查。
- 文件搜索在 Rust 保持稳定顺序 / 连续增量；Flutter 消费 replacement / append / metadata 和不可变块，未引入分页或前端重排。
- 剪贴板历史落盘，详情按需读取并共享串行准入；图标缓存有并发与字节预算。本轮没有改变采集开关、留存策略或权限。
- Web RPC 仍有独立身份检查、执行队列、帧 / 文本 / 存储限额和超时。前端释放名额不替代 Rust 取消与授权。
- 关闭插件与隐藏窗口保持不同语义；隐藏 15 秒规则不变。没有杀死 WebKit GPU Helper 或承诺系统立即结束它。

## 验证

自动检查与本轮实际 UI 结果完成后记录于 [V0.11 MVP](../../mvp-v0.11.md)。本轮不扩大 Windows、真实 IME、长期内存及恶意网页完整隔离的验收范围。


## Build 21 补充

SDK 的生命周期模板从插件业务中收回到 `definePlugin`；JSON 与 Text Pad 使用同一实现，恢复后的稳定 Host 门面不保存旧文档资格。创建器负责独立项目和 SDK 快照，模板无需样式或可见性代码。新增 SDK 3 项生命周期测试和 Flutter 1 项 ready 确认测试；正式构建截图复验通过，完整范围与限制见 MVP 最新记录。
