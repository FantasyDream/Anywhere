# Launcher Scope 与菜单导航

状态：2026-09-30，按 [用户指定交互规范](scope-group-menu-design.md) 重新实现。原 Context 焦点方案已被替代。

## 分工

```text
Context Graph → Action.accepts → Availability
             → Current App 聚合全部 Action / 其他 Scope 按 group_id 生成
             → Scope 硬过滤 → Query Match → Ranking → Result List
```

Current App 是当前环境所有可用 Action 的总入口，包括菜单、Folder、Git、剪贴板与输入识别动作；各 Action 保留原 group_id。其他 Scope 是领域过滤器。Context 仍负责对象、能力、来源和执行参数；不作为可切换的 UI Scope。Action Group 不绑定 ContextType，同一 Git Group 可同时包含作用于仓库和 URL 的动作。无可见动作的 Group 不显示；Disabled 可见，Hidden 不进入候选、Scope 或菜单目录。应用启动结果是一级 Candidate，不生成 Scope。

## 两种入口

- Current App（⌘⌥Space）：有当前应用身份且环境中存在可见动作时默认选其 Scope，否则 `selected_scope = None`。
- Universal Search（加 Shift）：始终从 None 开始。
- 共用一个窗口、查询、Action Registry 和 Ranking。重复同一入口关闭；切换入口保留查询。
- Scope Bar 只显示动作分组和当前应用名，没有 All、Context 对象或 Menu chip。

## 首页、搜索、浏览

空查询的应用 Scope 展示最多 7 个推荐动作和 Browse Menus；其他首页展示少量推荐动作，不展示 Applications。菜单推荐使用通用标题规则和真实执行历史，低价值菜单仍可搜索或浏览。菜单属于 `app.<bundle_id>` Group，Action ID 与 Usage 身份不变。

查询在当前 Scope 全量搜索；在菜单子页输入后暂时忽略浏览路径，清空后恢复该页。Scope 为 None 时搜索应用、动作和其他 Candidate Sources。应用名查询不会带出整组菜单，应用名加命令关键词可直达动作。菜单行显示叶子标题，副标题显示父菜单路径。来自页面、剪贴板、选中文本和输入的动作显示实际执行对象的来源，避免同名动作看起来完全相同。

同一 Action 对相同规范化对象只保留一条候选，并合并 Context 根归属；不同对象保留各自候选和执行参数。Copy URL 与 Copy Path 共用剪贴板重复内容隐藏规则：当前页面 URL 已在剪贴板时隐藏 Copy URL；剪贴板来源本身也不提供重复复制。URL 比较使用解析后的规范化地址，忽略首尾空白、主机名大小写和默认端口，保留路径、查询参数及片段差异。Open in Default Browser 仍可用，相同 URL 合为一条，不同 URL 按 Current page / Clipboard 区分。剪贴板地址继续在显示层隐藏，不按标题误合并不同目标。

Browse Menus 只显示当前应用菜单的原层级；进入子菜单和切换 Scope 都不写 Usage。Enter 执行动作或进入子菜单；空输入时 → 可进入所选子菜单，← 可返回，输入非空时左右键留给光标。

## 状态与返回

Flutter 维护 `input / scopeId / actionPath / selectedId`；`actionPath = null` 是首页，`[]` 是菜单顶层，`[View, Appearance]` 是子页。结果由 Core 排序；导航项只持路径，不持执行参数。

Esc 顺序：清查询 → 返回菜单上一层（一级菜单回应用首页）→ 清 Scope → 关闭。清 Scope 不清上下文，切 Scope 不清查询。查询无匹配时 Enter 保持 Scope，不隐式扩大到全局。

用户未移动选择时可跟随 Top 1；移动后用稳定 Candidate ID 保持目标。迟到菜单只能在用户尚未操作时默认激活 Current App。活动 Scope 消失则清 Scope 和浏览路径；旧 Session/Revision 仍不能执行。

## Scope 顺序和后端边界

Current App 在 Session 中预留首位，只在有可见动作时展示。其他初始 Group 按执行 frecency、动作最大 base_priority、group_id 排序；已出现的槽位冻结，迟到 Group 追加，暂时不可见的 Group 保留原槽位供恢复。

Scope 元数据由完整可见动作产生，不因查询无匹配而消失。真正的查询通过 `Application::update_scoped_input` 传入 Group，ActionMatcher 使用同一 Scope 边界函数，在查询匹配和打分前过滤：Current App 包含全部可用 Action，其他 Scope 只包含对应 Group；应用索引和其他 Universal 来源不在 Scope 中运行。完整 Action Offer 留给菜单浏览及现有执行校验；核心业务仍由 Rust 承担。

详见 [架构](architecture.md#当前搜索与-scope-行为) 和 [验收规范](scope-group-menu-design.md)。
