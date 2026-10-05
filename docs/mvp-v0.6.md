# V0.6 MVP：Action Shortcuts

日期：2026-10-06。分支 `dev-0.6`，从已推送的 V0.5 收尾提交 `d03a1f2` 创建。Rust `0.6.0-dev.1`，macOS App `0.6.0+1`。**V0.6 开发中，已实现 Launcher 内快捷键这一切片。**

## 范围与进度

| 切片 | 功能 | 当前状态 |
|---|---|---|
| S1 Launcher 内快捷键 | 配置、录制、冲突、保存/移除、当前 Context 解析与执行 | 已实现，工程检查通过 |
| S2 全局意图快捷键 | 一组键按新捕获的 Context 和显式路由分派，Launcher 隐藏时也能触发 | 待实现 |
| S3 全局注册与降级 | 注册冲突/失败恢复、缺少 Context、权限失败、多目标和窗口/任务反馈 | 待随 S2 验收 |
| 后续配置交换 | 导入/导出 | 配置及路由语义稳定后再排期，不列入当前切片 |

V0.6 完成必须同时验收 S1～S3，不能将当前开发包视为整个 V0.6 完成。

## 当前操作

1. 打开 Anywhere，捕获正在操作的环境。项目脚本与 Cargo 运行目标在捕获该项目后进入配置列表。
2. Settings → Action Shortcuts → Add action shortcut，搜索并选择动作，录制组合键，Save shortcut。
3. 在 Launcher 可见、输入法未处于组合输入、没有结果文本预览时按下组合键。无需让该动作出现在当前查询或 Scope 的结果里。
4. 只有一个可用对象时直接执行；多个对象时显示这些精确候选，↑/↓ 和 Enter 或点击选择，Esc 取消选择。无 Context、Disabled、Hidden、动作失效或采集仍进行时不执行，给出反馈；加载/执行中的匹配组合键不会作为查询文字输入。
5. 设置中点击键位可修改，关闭图标移除。未在上次环境中发现的动态动作保留配置并提示，重新捕获该项目后可编辑；当前只能移除，不把旧动作配置悄悄改为另一个动作。

首批支持 Command / Option / Control 加 A～Z，可加 Shift。最多 128 项，每个业务动作一项，每组键一项。拒绝未修饰的文字、仅 Shift、常用编辑/窗口/菜单浏览命令及与 Anywhere 两个唤醒入口相同的组合键；修改唤醒键时也反向检查冲突。仅在 Launcher 内生效，不注册新的系统全局热键。

## 身份、存储和执行契约

- 原应用 `Candidate.shortcut` 继续用于展示；用户快捷键单独建模，不覆盖该 DTO 字段。
- 配置保存业务 ActionId、展示标题和键位，不保存 CandidateId、ContextKey、URL、路径或本次扫描 token。标题只用于显示，执行只使用业务 ID。
- Rust `shortcut_actions` 从当前基础 Registry 和上次已提交 Context 展开可绑定动作；只给配置界面动作身份、标题、Context 类型。菜单不进入列表。
- `resolve_action_binding` 检查 Session / Revision、当前目录版本、快照与补充采集完成状态，以及 Registry 的 `persistent_binding` 策略；在当前完整 Action Offer 中解析 Enabled 候选。缺少或不唯一目标不靠 Ranking 猜测。
- Core 解析后仍由原 `execute` 路径执行，再复验候选凭证、资源和权限；后台进程、返回任务窗口与 Usage 沿原流程。查询、Scope、迟到快照、关闭和新 Session 都使未完成的前端解析失效。
- 菜单长期身份未经验证，持久绑定仍明确禁止；手动写入菜单 ID 也不能通过 Core 分派。
- 原生 SettingsStore 持久化 `actionShortcutVersion = 1` 与可选 `actionShortcuts`。0.5 及更早文件缺少字段时按空配置读取；不支持的较新版本在 Dart 与原生保存边界被拒绝，不静默覆写。
- 编辑/窗口保留键码来自共享 keymap；Dart 校验交互，原生边界重复校验格式、数量、唯一性及唤醒键冲突。设置写入与现有保存队列、错误提示、关闭草稿保护共用。编辑弹窗打开时拒绝关闭/切换/退出，用户须先保存或取消编辑。

## S2 的实现要求

- 全局触发先冻结真正的前台应用身份、创建新 Launcher Session 并重新捕获 Context，不使用上次唤醒保存的对象。
- 意图配置只持久化明确的 ActionId 和应用/Context 匹配规则；匹配顺序与歧义规则需显式定义。没有唯一匹配、无权限或捕获超时时展示选择/错误，不执行兜底的另一条命令。
- 系统热键服务只负责注册和事件；Context 解析、可用性与执行复验仍在 Rust。分派请求有独立代次，重复触发、取消和迟到完成不得执行旧动作。
- 注册失败须保留/恢复先前可用组合；用户禁用、删除、插件重载和正常退出时清理注册。原有 Current App / Universal 唤醒入口及窗口保护继续有效。
- 在真实物理全局热键、权限组合、多目标、多次触发、后台任务和退出场景验收后，才能关闭 S2/S3。

## 本轮验证

- `scripts/check.sh`：263 项 Rust、73 项 Flutter 测试，fmt / Clippy / analyze、架构、原生菜单/采集与 Host 服务检查通过。
- Core facade：跨查询/Scope 解析、历史记录、两种 URL 对象不猜测、Hidden 复制、过期 Revision/Session、Registry 撤销、动态脚本配置目录、补充采集等待、Disabled 原因、菜单绑定拒绝。
- Flutter：旧设置与序列化、较新版本保留、冲突、精确多目标选择/Esc、关闭后迟到结果、缺少 Context、键盘触发/重复、设置搜索/录制/保存/移除及编辑弹窗关闭保护。
- 原生：业务 ID 和键位磁盘重开、旧字段缺失、冲突反向检查、重复键/动作、保留命令、未知版本拒绝；原有热键注册失败恢复继续通过。
- 当前源码 Release 构建、固定签名与 ZIP/SHA-256 校验通过。独立开发包 `build/Anywhere-dev-0.6.app`，0.5 包保留。
- 未进行新包真实 GUI、物理键盘/不同键盘布局、权限交互或 Windows 验收。测试模拟的按键和 Host 后端不能替代这些验收。
