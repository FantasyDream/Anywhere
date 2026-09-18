# Current App 菜单动作

## 交互约定

- 菜单动作直接归入现有 Current App Scope；`group_id = app`，不增加另一个应用 Scope。
- 没有 Accessibility 权限时不提供菜单动作；Finder 目录、Git、应用搜索和其他既有动作照常工作。只要这些动作仍使 Current App Scope 非空，该 Scope 就继续存在。
- 有意义但当前不可用的菜单保留为标准 Action，Availability 为 `Hidden`。它们不会进入候选或 Scope，不呈现灰色行。
- 分隔符、空标题、子菜单容器、没有执行能力的状态节点直接过滤。
- Query、Esc、双入口和 Session 内 Scope 排序保持原有规则。Group frecency 合计各独立 Action 的有界使用频率及最近使用权重，同一 Action 不因多个 Context 重复计分。

## 为什么不可用菜单采用 Hidden

“发现时丢掉”与“Hidden 后由 Matcher 排除”在界面上都不可见。Hidden 保留同一动作的身份和元数据，由统一 Availability 决定候选是否出现，后续刷新为 Enabled 时仍复用原 ID 和使用历史。执行器仍重新确认应用实例、定位及 Enabled，Hidden 本身不是执行授权。

## 架构

- Platform 返回菜单元数据和可重建的 locator，不向 Rust 保存 AX Element。
- `features/app_actions` 负责元数据缓存、标准 Action 转换和普通 `ActionHandler` 实现；不增加菜单专用 Core executor。
- `application/app_actions.rs` 负责 Discovery 生命周期和异步发布。
- 每个 Session 持有统一的 `ActionRegistry` 不可变快照。内建、插件、菜单动作通过相同注册校验和 Matcher / Search / Ranking / Execution 链路。
- 首屏可复用当前应用实例和版本对应的缓存。每次打开后台刷新；显示窗口后的授权检查不会重复启动已授权的 Discovery。
- 每次发现分配单调递增 generation。缓存写入和 Session 发布均拒绝旧任务；执行失败失效旧 generation，且不能抹掉新 Session 的结果。
- 发布时同时校验 Registry 快照与 Session / Revision / snapshot sequence，避免搜索中途更新动作后提交旧列表。
- locator 失效、菜单不可用或实例改变时返回失败，清除相应旧缓存；Session 仍活动时立即重新发现。

## 身份与定位边界

- 应用实例使用 bundle ID、PID、进程启动时间校验，不回退到同 bundle 的其他运行实例。
- Action ID 优先使用本次菜单中唯一的 AXIdentifier，否则使用完整语义路径；路径节点优先用 AXIdentifier，缺失时用标题。ID 不包含菜单位置索引，并保留完整 64 位哈希。
- 缺少稳定 AXIdentifier 时，无法可靠推断本地化前后是不是同一条命令：路径标题改变可能形成新 ID。不会承诺这一降级模式跨语言稳定。
- 每次执行重新逐级定位。AXIdentifier 消失时不会回退到同名菜单；同一级没有可区分标识的重名节点不提供为可执行动作。支持不同菜单路径中的重复标题。
- Discovery 限制遍历深度、访问节点数和总时长；AX 消息设置超时。超时或不完整树不会写成成功缓存，既有非菜单功能继续可用。
- 快捷键作为独立 Action / Candidate 字段经 bridge 传至 UI，同时保留为搜索别名；支持字符键及常见特殊键。

## 验证

`scripts/check.sh` 包含 Rust/Flutter 检查及 macOS 原生定位回归。原生测试使用模拟 AX 节点运行生产遍历和定位函数，不激活外部应用。

覆盖：Hidden 恢复、同一 Scope、未授权保留 Finder 动作、单次发现、同 Session / 跨 Session 过期结果、失效后刷新、PID/启动身份/版本变化、重复标题、兄弟插入、AX 标识本地化、快捷键和 Group recency。

Finder、VS Code、Chrome/Safari、TextEdit 的真实菜单、物理热键、授权交互以及首屏延迟仍需实机手工验收；自动化测试和打包通过不代表这些 GUI 项目已经验证。
