# Current App 菜单动作

## 交互约定

- 菜单动作直接归入现有 Current App Scope；`group_id = app`，不增加另一个应用 Scope。
- 没有 Accessibility 权限时不提供菜单动作；Finder 目录、Git、应用搜索和其他既有动作照常工作。只要这些动作仍使 Current App Scope 非空，该 Scope 就继续存在。
- 有意义但当前不可用的菜单保留为标准 Action，Availability 为 `Hidden`。它们不会进入候选或 Scope，不呈现灰色行。
- 分隔符、空标题、子菜单容器、没有执行能力的状态节点直接过滤。
- Query、Esc、双入口和 Session 内 Scope 排序保持原有规则。Group frecency 合计允许记录历史的独立 Action 的有界使用频率及最近使用权重，同一 Action 不因多个 Context 重复计分。未验证长期身份的菜单动作暂不参与使用历史与 Group frecency。

## 为什么不可用菜单采用 Hidden

“发现时丢掉”与“Hidden 后由 Matcher 排除”在界面上都不可见。Hidden 保留同一动作的身份和元数据，由统一 Availability 决定候选是否出现，后续刷新为 Enabled 时仍复用原 ActionId。菜单使用历史目前隔离，不能据此恢复旧频次。执行器仍重新确认应用实例、定位及 Enabled，Hidden 本身不是执行授权。

## 架构

- Platform 返回菜单元数据和可重建的 locator，不向 Rust 保存 AX Element。
- `features/app_actions` 负责元数据缓存、标准 Action 转换和普通 `ActionHandler` 实现；不增加菜单专用 Core executor。
- `application/app_actions.rs` 负责 Discovery 生命周期和异步发布。
- 每个 Session 持有统一的 `ActionRegistry` 不可变快照。内建、插件、菜单动作通过相同注册校验和 Matcher / Search / Ranking / Execution 链路。
- 首屏可复用当前应用实例和版本对应的缓存。每次打开后台刷新；显示窗口后的授权检查不会重复启动已授权的 Discovery。
- 每次发现分配单调递增 generation。缓存写入和 Session 发布均拒绝旧任务；执行失败失效旧 generation，且不能抹掉新 Session 的结果。
- 发布时同时校验 Registry 快照与 Session / Revision / snapshot sequence，避免搜索中途更新动作后提交旧列表。
- 动作目录重载另推进 catalog version，清除执行缓存并重算快照；菜单缓存与发现发布同时检查该版本。重载后的 Action 行带版本，旧行不能命中同 ID 的新实现；blocking worker 与进程启动前复验版本。它与菜单 discovery token 分别管理定义失效和扫描目标失效。
- locator 失效、菜单不可用或实例改变时返回失败，清除相应旧缓存；Session 仍活动时立即重新发现。

## 身份与定位边界

- 应用实例使用 bundle ID、PID、进程启动时间校验，不回退到同 bundle 的其他运行实例。
- Action ID 优先使用本次菜单中唯一的 AXIdentifier，否则使用完整语义路径；路径节点优先用 AXIdentifier，缺失时用标题。ID 不包含菜单位置索引，并保留完整 64 位哈希。
- 缺少稳定 AXIdentifier 时，无法可靠推断本地化前后是不是同一条命令：路径标题改变可能形成新 ID。不会承诺这一降级模式跨语言稳定。
- 每次执行重新逐级定位，标题与标识都须和所显示的快照一致。AXIdentifier 消失或出现新标识时不会按同名菜单执行；同一级重复标识或无法区分的重名节点不提供动作。稳定 AXIdentifier 的本地化仍保持 ActionId，但旧文案候选须刷新后重新选择。支持不同菜单路径中的重复标题。
- Discovery 限制遍历深度、访问节点数和总时长；AX 消息设置超时。超时或不完整树不会写成成功缓存，既有非菜单功能继续可用。
- 快捷键作为独立 Action / Candidate 字段经 bridge 传至 UI，同时保留为搜索别名；支持字符键及常见特殊键。
- 此字段只展示原应用快捷键。MenuAction 的 BindingPolicy 明确拒绝持久绑定；Registry 生成持久绑定时会检查该策略，`execute_builtin` 包装也检查其目标，不能绕过限制。

## 验证

`scripts/check.sh` 包含 Rust/Flutter 检查及 macOS 原生定位回归。原生测试使用模拟 AX 节点运行生产遍历和定位函数，不激活外部应用。

覆盖：Hidden 恢复、同一 Scope、未授权保留 Finder 动作、单次发现、同 Session / 跨 Session 过期结果、失效后刷新、PID/启动身份/版本变化、重复标题、兄弟插入、AX 标识本地化、快捷键、非菜单 Group recency 与菜单历史隔离。

Finder、VS Code、Chrome/Safari、TextEdit 的真实菜单、物理热键、授权交互以及首屏延迟仍需实机手工验收；自动化测试和打包通过不代表这些 GUI 项目已经验证。

## 2026-10-05 安全修复（dev-0.5 / App 0.5.0+10）

- 稳定 ActionId 用于业务身份；每个发现的 MenuAction 另持有不复用的执行 token，候选 ID 包含该 token。查询、排序和迟到的 Context 不改变同一目标的 token；新的菜单 Registry 创建新的 token，旧候选不能命中新目标。
- 阻塞执行工作开始时同时检查 Session/Revision 和当前 Registry 的 token，防止已排队请求在动作失效后触发副作用。
- 原生定位检查每级标题和标识；按下之前再检查叶子 Enabled、标题、标识及应用实例。读取 Enabled 期间标题变化也会失败，不调用 AXPress。
- AXIdentifier 与完整路径都可能被第三方应用重用，现有元数据无法证明长期语义身份。菜单动作暂停持久历史读取和写入（包括首页推荐与 Scope 排序），保留旧数据库记录；应用启动、文件夹、Git 和 Developer Actions 的历史照常使用。它是安全降级，不等于持久菜单身份契约已完成。
- 暂不将这些菜单 ActionId 用于长期自定义快捷键绑定。恢复历史或绑定前需要可验证的长期身份与显式迁移规则，禁止猜测关联旧记录。
- 完全相同的标题、路径和标识被应用悄悄改变语义，AX 元数据仍无法识别。本轮解决可观察的身份变化、旧候选替换和历史误关联，不能宣称验证了第三方命令的真实语义。

回归覆盖：标识复用、唯一/重复/另一命令唯一、标识消失、路径改名、同路径替换、SQLite 重开、旧候选拒绝、当前候选执行、排队期间失效、查询后 token 保持、菜单历史不改变 Scope 顺序，以及原生 AXPress 次数。真实应用菜单执行未在本轮重新验收。
