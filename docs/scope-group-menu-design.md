# Scope / Menu 交互验收规范

状态：2026-09-30。以下保留最初用户指定的新交互方案，替代 Context 焦点导航。

后续修订优先于下文原方案：**Current App Scope 必须聚合当前环境全部可用 Action**，包括 Folder、Git、菜单、剪贴板与输入识别动作；不再按 app.<bundle_id> 限制动作搜索。其他 Scope 仍按领域 Group 硬过滤。Action 的 group_id 不改写，应用启动结果只属于 Universal。只要存在可见动作且有当前应用身份，即可展示 Current App；不要求菜单权限。Browse Menus 仍仅浏览当前应用菜单，首页仍展示少量推荐。

实现约定：Current App 内部保留首位槽位，但只有可见动作时才显示；其他迟到 Group 只追加。Menu 浏览状态中 `null` 表示首页，空路径表示顶层菜单，非空路径表示子菜单；从一级菜单按 Esc 返回应用首页，搜索时保留原路径。首页最多 7 个推荐动作加 Browse Menus，普通空查询首页最多 7 个动作。

---

# 1. UI 总体结构

Launcher 保持极简：

```text
┌──────────────────────────────┐
│ Search Input                 │
├──────────────────────────────┤
│ [VS Code] [Git] [Maven] ... │  ← Scope Bar
├──────────────────────────────┤
│ Result 1                     │
│ Result 2                     │
│ Result 3                     │
│ ...                          │
└──────────────────────────────┘
```

核心区域只有：

```text
Search Input
Scope Bar
Result List
```

原则：

```text
少层级
少视觉噪音
Keyboard First
Context First
```

---

# 2. Scope 的定义

Scope 使用：

```text
Action Group
```

而不是 Context。

关系：

```text
Context
↓
Action.accepts
↓
Available Actions
↓
按 group_id 聚合
↓
Scope
```

例如：

```text
git.repository
→ Git Status
→ Git Log

project.maven
→ Maven Test
→ Maven Package
```

UI 展示：

```text
[ Git ] [ Maven ]
```

而不是：

```text
[ git.repository ] [ project.maven ]
```

原则：

> Context 属于 Engine；Action Group 属于 UX；Scope 基于 Action Group。

---

# 3. ActionGroup 不绑定 ContextType

例如：

```text
git.status
accepts = git.repository

git.clone
accepts = core.url
```

都可以属于：

```text
group = git
```

因此不要设计：

```text
git group
→ git.repository
```

Group 只负责：

```text
组织
过滤
展示
Scope
```

---

# 4. Scope 是否显示

Scope 只由当前 Visible Action 决定：

```text
某 Group 至少有一个 Visible Action
→ 显示 Scope

没有 Visible Action
→ 不显示
```

Current App 也一样。

例如当前 VS Code 没有任何 Action：

```text
[ Git ] [ Folder ]
```

而不是显示一个空的：

```text
[ VS Code ]
```

---

# 5. 不显示 All Scope

UI 不显示：

```text
[ All ]
```

内部：

```text
selected_scope = None
```

代表：

```text
All / Universal
```

因此 UI：

```text
[ VS Code ] [ Git ] [ Maven ] [ Folder ]
```

没有任何 Scope 激活时，就是全局模式。

---

# 6. 两种 Launcher 唤醒方式

提供两个可配置快捷键。

## Current App Launcher

用于：

> 我想操作当前应用。

打开时：

```text
Current App 有 Visible Actions
→ 默认激活 Current App Scope

没有
→ scope = None
```

例如：

```text
[ VS Code ] [ Git ] [ Folder ]
  ^ active
```

---

## Universal Launcher

用于：

> 我要全局搜索。

打开时永远：

```text
scope = None
```

两个入口：

```text
共用同一个 Window
共用 Search
共用 Ranking
共用 Action Registry
```

不要实现两套 Launcher。

---

# 7. Scope 激活时的输入逻辑

Scope 激活后，输入不会自动退出 Scope。

例如：

```text
scope = VS Code
query = "ter"
```

表示：

```text
在 VS Code Actions 中搜索 "ter"
```

而不是 Universal Search。

核心原则：

> Scope 决定“在哪里找”，Query 决定“找什么”。

---

# 8. scope=None 时的搜索

如果：

```text
scope = None
```

且：

```text
query != empty
```

则进入：

```text
Universal Search
```

搜索：

```text
Applications
+
所有当前可用 Actions
+
未来其他 Candidate Sources
```

例如：

```text
terminal
```

可能得到：

```text
Terminal                   App
Open Terminal Here         Folder
VS Code · Toggle Terminal  VS Code
```

---

# 9. 无输入时的行为

## Current App Scope 激活

例如：

```text
[ VS Code ]
```

无输入时：

```text
不要展示所有 VS Code Menu Actions
```

而是展示：

```text
Top 5~8 推荐 Action
```

例如：

```text
Toggle Terminal
Command Palette
Format Document
New Window
Git: Commit
Browse Menus… >
```

这是：

```text
Current App Home
```

---

## scope=None

无输入时进入：

```text
Context Home
```

展示当前环境综合排名最高的少量 Action：

```text
Git Status
Open Terminal Here
Copy Path
...
```

不默认展示 Applications。

---

# 10. Menu Bar Actions 的定位

Menu Bar 是：

```text
Action Source
```

不是 Scope。

所以不要增加：

```text
[ Menu ]
```

例如 VS Code：

错误：

```text
[ VS Code ] [ Menu ]
```

正确：

```text
[ VS Code ]
```

Menu Item 都属于：

```text
app.<bundle_id>
```

这个 Action Group。

原则：

> Scope 按“用户想操作什么领域”划分，不按“Action 从哪里来”划分。

---

# 11. Current App Scope 的三种展示模式

为了避免 Menu Item 太多导致杂乱，Current App Scope 根据状态采用不同展示方式。

## 无 Query：推荐模式

只展示：

```text
Top 5~8
```

高价值 Action。

例如：

```text
Toggle Terminal
Command Palette
Format Document
New Window
Browse Menus… >
```

不要把 File/Edit/View/Help 下几百个 Action 全部铺出来。

---

## 有 Query：扁平搜索

例如：

```text
sidebar
```

直接搜索整个 VS Code Group：

```text
Toggle Primary Side Bar
View > Appearance

Toggle Secondary Side Bar
View > Appearance
```

搜索时忽略菜单层级。

也就是说：

```text
Search
→ Flatten Actions
```

---

## Browse Menus：层级浏览

用户选择：

```text
Browse Menus…
```

进入：

```text
File        >
Edit        >
Selection   >
View        >
Go          >
Run         >
Terminal    >
Help        >
```

进入：

```text
View
```

再显示：

```text
Appearance >
Editor Layout >
Toggle Panel
...
```

这是：

```text
Browse
→ Hierarchy
```

最终规则：

> 无输入看推荐，有输入全量搜索，需要探索时才进入原始菜单层级。

---

# 12. Menu Hierarchy 不等于 Scope

菜单层级属于：

```text
Action Hierarchy
```

而不是：

```text
Action Group
```

例如：

```text
Scope:
VS Code

Hierarchy:
View
└─ Appearance
   └─ Toggle Sidebar
```

内部 Action 可以保留：

```text
group_id = app.vscode

menu_path = [
  "View",
  "Appearance"
]

title = "Toggle Sidebar"
```

不需要把：

```text
View
Appearance
Terminal
```

变成新的 Scope。

---

# 13. Browse 与 Search 的关系

如果当前正在：

```text
VS Code
→ View
→ Appearance
```

然后开始输入：

```text
sidebar
```

推荐行为：

```text
暂时忽略当前 submenu
```

直接搜索：

```text
整个 VS Code Scope
```

而不是只搜索 Appearance 下的 Action。

即：

```text
Scope
→ 决定搜索边界

Submenu
→ 决定浏览边界

Query
→ 出现时打平当前 Scope
```

---

# 14. Menu Action 的推荐策略

Menu Actions 可以简单分为：

```text
推荐
可搜索
仅浏览
```

MVP 不一定需要真实字段。

只需要规则：

```text
Zero-query
→ Top N Ranking

Query
→ 所有 Action 可搜索

Browse Menus
→ 所有 Menu Item 可浏览
```

例如：

```text
Toggle Terminal
→ 推荐 + 搜索

About VS Code
→ 搜索 / 浏览

Services
→ 主要 Browse
```

---

# 15. Scope 切换

快捷键：

```text
Ctrl + Tab
→ Next Scope

Ctrl + Shift + Tab
→ Previous Scope
```

例如：

```text
[ VS Code ] [ Git ] [ Maven ] [ Folder ]
```

循环：

```text
VS Code
→ Git
→ Maven
→ Folder
→ VS Code
```

---

# 16. scope=None 不参与 Scope 循环

不会出现一个隐形：

```text
All
```

参与 Ctrl+Tab。

退出 Scope 使用：

```text
Esc
```

---

# 17. 切换 Scope 保留 Query

例如：

```text
query = "test"
scope = Maven

→ Maven Test
```

Ctrl+Tab：

```text
scope = Node

→ Run Test
```

再次：

```text
scope = Rust

→ Cargo Test
```

原则：

```text
切 Scope
≠
清 Query
```

---

# 18. Esc 优先级

普通状态：

```text
1. Query 非空
   → Clear Query

2. Scope 激活
   → Clear Scope

3. scope=None
   → Close Launcher
```

---

# 19. Browse Menu 后的 Esc 优先级

进入菜单层级后：

```text
1. Query 非空
   → Clear Query

2. submenu depth > 0
   → Back one level

3. Scope 激活
   → Clear Scope

4. Close Launcher
```

例如：

```text
VS Code
→ View
→ Appearance
```

Esc：

```text
View
```

再 Esc：

```text
VS Code Home
```

再 Esc：

```text
scope=None
```

再 Esc：

```text
Close
```

---

# 20. Enter / Arrow 在菜单层级中的行为

选中真正 Action：

```text
Enter
→ Execute
```

选中 Submenu：

```text
Enter / →
→ Enter submenu
```

返回：

```text
← / Esc
→ Previous level
```

但左右箭头是否最终使用，应避免与输入框光标行为冲突。

MVP 使用 Enter + Esc 即可。

---

# 21. Scope 排序

规则：

```text
1. Current App Scope 固定第一
2. 其他 Scope 按 Group Frecency
3. base_priority
4. group_id 稳定兜底
```

---

# 22. Group Frecency

来源于：

```text
真正执行旗下 Action
```

例如：

```text
Git Status
Git Diff
Git Log
```

执行后提高：

```text
group = git
```

的 Frecency。

不要因为：

```text
用户只是切换到了 Git Scope
```

就增加分数。

---

# 23. 当前 Session 内 Scope 顺序冻结

例如启动：

```text
[ VS Code ] [ Folder ]
```

Later：

```text
Git Context
```

才出现。

当前 Session：

```text
[ VS Code ] [ Folder ] [ Git ]
```

只追加，不重排。

下一次打开：

```text
再根据最新 Frecency 排序
```

可能：

```text
[ VS Code ] [ Git ] [ Folder ]
```

避免用户操作时 Scope 位置突然跳动。

---

# 24. Scope 是 Hard Filter

如果：

```text
scope = Git
```

则只有：

```text
group_id = git
```

的 Action 参与后续：

```text
Query Match
Ranking
Presentation
```

其他 Group 不参与。

Scope 不是 Ranking Boost。

---

# 25. Action Availability

支持：

```text
Enabled
Disabled(reason)
Hidden
```

规则：

```text
Enabled
→ 正常显示

Disabled
→ 显示但不可执行

Hidden
→ 不显示
```

Hidden Action：

```text
不参与 Scope 生成
```

因此：

```text
Current App 所有 Action Hidden
→ Current App Scope 不显示
```

---

# 26. Result List 展示

结果尽量只暴露用户有用的信息：

```text
Icon
Title
Subtitle / Source
Shortcut
Disabled 状态
```

例如：

```text
Toggle Sidebar
View > Appearance
```

或者：

```text
Git Status
Git
```

不要暴露：

```text
ContextType
Provider
CandidateType
group_id
```

这些属于内部架构。

---

# 27. Application 与 App Action 的关系

Application 是一级 Candidate：

```text
Visual Studio Code
```

App Actions 默认属于：

```text
app.<bundle_id>
```

搜索：

```text
vscode
```

优先：

```text
Visual Studio Code
```

不要把所有 VS Code Menu Action 都一起平铺出来。

只有 Action 自身强匹配时才提升。

例如：

```text
vscode terminal
```

可以：

```text
VS Code · Toggle Terminal
Visual Studio Code
```

---

# 28. Menu Action 的 subtitle

Menu Action 推荐使用：

```text
title = Menu Item

subtitle = Menu Path
```

例如：

```text
Toggle Sidebar
View > Appearance
```

这样即使两个 Action 同名，也能区分。

---

# 29. Selection Stability

不要只保存：

```text
selected_index
```

而是保存：

```text
selected_action_id
```

Late Action / Late Context 导致结果更新时：

```text
用户还没移动 Selection
→ 可以重新选择 Top 1

用户已经移动过
→ 保持 selected_action_id
```

避免 Enter 目标突然变化。

---

# 30. 键盘操作总表

```text
输入
→ Search

↑ / ↓
→ Select Result

Enter
→ Execute / Enter submenu

Ctrl + Tab
→ Next Scope

Ctrl + Shift + Tab
→ Previous Scope

Esc
→ Clear Query
→ Back submenu
→ Clear Scope
→ Close
```

---

# 31. 最终状态模型

UI 状态可以收敛成：

```text
LauncherState {
    query,
    selected_scope,
    available_scopes,
    action_path,
    results,
    selection
}
```

其中：

```text
selected_scope = None
→ Universal / Context Home

selected_scope = Some(group)
→ Scoped Action Search
```

`action_path`：

```text
[]
→ Scope Home

["View"]
→ View Menu

["View", "Appearance"]
→ Nested Menu
```

---

# 32. 最终交互原则

整个产品可以浓缩成几句话：

> **Scope 是 Action Group，不是 Context。**

> **Context 决定当前有哪些 Action；Scope 决定搜索哪些 Action。**

> **Current App 默认只展示少量高价值 Action，不展示全部 Menu Item。**

> **Menu Bar 是 Action Source，不是 Scope。**

> **无输入看推荐，有输入全量搜索，需要探索时再 Browse Menus。**

> **Scope 决定搜索范围，Query 决定搜索内容，Submenu 只决定浏览路径。**

最终目标不是让用户看到“系统收集到了多少 Action”，而是让用户觉得：

```text
我想执行的东西
通常就在最前面

不知道名字
可以 Browse

知道大概叫什么
直接输入

想换操作领域
切 Scope
```

这就是整个 Launcher UI 的核心交互模型。