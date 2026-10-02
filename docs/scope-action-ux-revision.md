# Context 优先的导航设计

状态：2026-09-30，历史方案，已被用户指定的 [Scope / Menu 交互规范](scope-group-menu-design.md) 替代。下面的 Context 焦点规则不再描述当前实现。当前行为见 [Launcher Scope 与菜单导航](action-navigation-design.md)。

## 核心判断

Anywhere 的入口是**唤醒时用户正在操作的 Context**。Context Graph 由多个独立采集的根及其派生节点构成；它不是首页上的来源标签。一个根代表一次明确的操作焦点，例如 Finder 选中的文件、选中文本、浏览器当前页、前台应用或剪贴板内容。Provider 推导的 URL、JSON、Git 仓库和项目类型仍属于这条焦点链，给它增加可用动作和能力。

同一图里可以有多个可操作的根节点。“当前焦点”只是导航状态里指向其中一个根节点的引用，**不再定义独立的 ContextFocus/Subject 领域实体和第二套对象 ID**。前台应用是 Context，可承载菜单动作；从应用推导出的当前目录是独立的操作根。SessionFacts 等环境事实可以参与 Availability，但不因此成为可切换目标。搜索得到的应用、文件和将来的会话先是搜索结果；用户选中并要求更多操作时，可按需转成有明确来源的类型化 Context，不自动混进唤醒时的采集图。

截图里 `/Anywhere/build` 是当前目录根节点，Git 仓库 `Anywhere` 是由它推导出的能力和执行目标。`Folder` 与 `Git` 不能作为两个平级 Scope 来过滤它的动作；`Directory` 行也不应在已经展示当前 Context 时重复出现。顶部 `Directory · Git Repository · Clipboard · Text` 是图中节点罗列，无法告诉用户系统现在理解的操作焦点。

## 保留三种领域概念和一个导航状态

| 层次 | 含义 | 用户看到什么 |
|---|---|---|
| Context Graph | 本次 Session 捕获和推导出的全部 Context、来源、血缘和版本；根节点就是可操作对象 | 展示当前根的对象名、来源、派生能力；不直接展示整张图 |
| Active Context（导航状态） | 指向一个 Context 根节点的稳定引用，不持有第二份对象 | “访达选中的 build”“浏览器当前页”；仅多根时提供切换 |
| Action Offer | 某个 Action 作用于一个 Context 的机会；明确实际执行目标 | “查看 Git 状态 · 仓库 Anywhere” |
| Action Group | 动作多时的浏览分段，非可选 Context，非硬过滤 | 操作面板内的“文件操作”“Git”“应用菜单” |

未来的 Search Domain（应用、文件、操作等）只筛搜索结果类型，与当前 Context 正交。它不能代替 Context，也不复用 Action Group。现阶段无需在首页显示 Search Domain。

**归属原则：**派生节点默认附着于原焦点；只有独立采集、用户显式选中，或确实代表另一个独立目标时才形成新焦点。同一路径在 Finder Selection、当前目录和剪贴板中可以有不同来源身份，Core 保留它们；UI 可合并冗余展示，但不能丢掉来源和归属。

## Scope 与 Context 的关系

Context 是对象和事实的模型；Scope 若继续作为内部名称，只表示一次查询的边界，不拥有对象身份。推荐把它写成显式查询模式：

```text
Scope = Context(ContextKey) | Universal
```

`Context(build)` 取 `build` 根及其派生 Context 能执行的全部 Action Offer，所以目录操作和 Git 操作一起出现。`Context(访达)` 取前台应用根的动作，包括菜单。`Universal` 搜索应用、命令及可匹配的上下文动作。切换 Context 根会改变查询边界，但用户看到的是**当前对象切换**，无需再展示一条含 `Folder/Git/App` 的 Scope Bar。将来 `Files/Apps/Actions` 等筛选是在 Universal 结果中追加的 Search Facet，不能与当前 Context 混成同一排 chip。

Scope 不应由 `group_id`、菜单路径、Provider 类型或动作数量自动生成。它也不能成为 Action 的属性：同一 Git Status Offer 可以出现在目录 Context 的全部动作面板和 Universal 搜索里，执行身份与 Usage 仍是同一个。打开 Context Actions 面板相当于固定主根 `ContextKey`，面板中的分段和查询只影响展示，不改变执行目标。

### Menu Item Action 的位置

菜单项不是新的 Context：它是**作用于前台应用 Context 的 Action**。`File › Export` 的路径是动作名称和面板内的浏览层级；它不形成 `Menu` Scope。这样，应用自己的其他动作和菜单动作能在同一个“访达”或“VS Code”上下文中找到，而当前目录的文件/Git 动作不会因同样来自该应用就被误归到菜单里。菜单 `Edit › Copy` 可能影响选中内容，但通用菜单发现并不因此自动把它复制到“选中文本” Context；只有动作显式声明对文本适用时，才在文本 Context 下给出相应 Offer。

当前版本已用前台应用 Context 根代替 `Current App` Scope；**菜单由应用 Context 承载**。应用面板可先列少量常用应用动作，再在“菜单栏”分段展示动态菜单；菜单多时支持按完整路径搜索，必要时只在该面板内部按 `File/Edit/View` 折叠。动态菜单默认不铺满首页，精确查询仍可直达。菜单不可用时继续 Hidden；没有辅助权限只移除菜单，不能让应用搜索、目录或 Git 能力消失。

## 唤醒和键盘交互

### Context 入口

现有“当前应用”唤醒入口应逐步变成“当前上下文”入口：优先展示用户明确选择的对象。首选焦点的建议顺序是 Finder 选中项或选中文本、浏览器当前页、当前目录、前台应用、剪贴板兜底；同一应用里有选中文本时它优先于该页面。具体权重可结合动作相关性和来源置信度调整，但**有选择就不让纯环境 Context 抢焦点**。前台应用的菜单焦点仍可切换到；没有任何可操作 Context 时保留应用/命令搜索。

Context 焦点栏显示**值、来源和派生能力**，不是类型列表。只有一个焦点时是静态标题；有两个以上可操作焦点时才出现切换控件。切换项使用实际内容或对象名，例如 `build · 访达`、`访达 · 菜单`、`选中文本`；`Git` 是 `build` 的能力标记，不是切换项。`Ctrl+Tab` 可沿用为焦点切换，但需同步修改提示，并在只有一个焦点时不显示无用切换。

```text
搜索“build”的操作…                         ⌘⌥⇧Space 全局搜索

当前上下文   📁 build
             /…/Anywhere/build · 来自访达 · Git 仓库 Anywhere

建议操作     在此打开终端
             复制路径
             查看 Git 状态                 作用于仓库 Anywhere

⌘K 全部操作 · Ctrl+Tab 切换上下文（仅多个焦点时）
```

当前 Context 标题承担对象展示，不再在结果中重复一条 `Directory` 对象行。若该 Context 有有价值的默认操作，可以在标题处明确显示；Enter 的行为必须可见且稳定。打开全部操作时，要同时看到目录操作和 Git 操作，不能因为当前选择了某个分组而只显示一半。

迟到的采集结果可以新增焦点与动作，但在用户已输入、移动选择或主动切换焦点后不得自动夺取焦点。Session/Revision、来源 generation 和已提交快照顺序继续保护这条规则。权限失败只移除依赖该权限的能力，其他焦点和动作继续可用。剪贴板正文不进入日志、Usage 或公开的副标题；重复的选中文本与剪贴板文本可在 UI 上压缩为一个优先焦点，内部仍保留两个来源。

### 查询与全局入口

Context 入口中的普通查询先筛**当前 Context 的动作**；搜索框明确写出对象，结果和空状态也标出筛选范围。无匹配时在空状态提供可按 Enter 使用的“搜索全部”，避免用户以为命令不存在。“搜索全部”入口和现有 Universal 快捷键搜索所有应用、命令及可匹配的上下文动作，并以当前 Context 作为排名信号，而不是隐形硬过滤。全局搜索结果若需要更多动作，先解析成 Context，再复用同一操作面板。

把文本输入框里的普通关键字视为查询，不应仅因产生 `core.text` Context 就自动把焦点切到“Text”。若用户粘贴或输入一个完整 URL、文件路径、JSON 等可识别对象，可显示“对这个输入执行操作”的明确入口；其 Context 来源仍是 UserInput，不能冒充唤醒时捕获的当前对象。返回面板后保留原焦点、查询和选中项。

## 多上下文与派生能力的规则

- Finder 多选整体是一个焦点。集合子项供 Provider 识别，但普通首页不逐个展开；需要对某一个子项执行动作时再进入明确的目标选择。单选文件、目录各自是焦点。
- 选中文本是焦点；从文本识别的 URL/JSON 是同一焦点的能力。浏览器当前 URL 与该页选中文本是两个独立捕获的焦点，默认选中文本优先。
- 当前目录是焦点；它派生出的 Git、Maven、Node 等能力仍在这一焦点里。Git 命令的目标可能是祖先仓库；Action 行应说明实际执行目标，避免让用户以为 Git Status 只作用于 `/build`。
- 前台应用是菜单动作的焦点。应用 Context 仍可提供当前目录，但两者分开呈现；应用菜单不能出现在目录的 Action Panel 里。没有可用菜单时，不为了一个空面板显示应用焦点。
- 将来如“用选中文本在当前目录创建文件”这样的动作，可声明主 Context 和所依赖的辅助 Context；不能把整张图合成一个模糊对象，也不能按单一目标 ID 猜测全部执行参数。

## Core 到 Flutter 的调整

1. **移除第二套对象模型。** 已删除从 Candidate 反推对象的 `SubjectPresentation`，直接展示根 Context 的 `ContextPresentation`（补图标、用户化标题、能力摘要、默认操作）。Flutter 的 `Scope.contextKey` 只是导航指针。不能仅从已有 Action 反推 Context，否则无动作、迟到动作和权限变化会让对象消失或改名。
2. **修正血缘和身份。** Finder 多选采集已把子项连到 `FileSelection` 集合，Graph 插入时重映射父关系；真实 capture→graph→offer 链路有回归覆盖，Git 派生 Action 归属集合根。跨 Revision 的根 `ContextKey` 由来源和规范化值产生，不用会重排的 `cN`。
3. **让 Offer 明确关联。** Candidate 保留 Action ID、Candidate ID 和执行凭证，绑定根归属 `context_keys`、实际执行的 `context_id`、可选的辅助 Context 引用与 `execution_target_label`。主根用于归属和面板，执行 Context 用于能力与参数；二者不必相同。搜索来源继续产出 Candidate；需要展开操作时按需解析为类型化 Context，带搜索来源身份，不自动混入当前采集图。
4. **把 Scope 的语义改正。** 停止从 `group_id` 自动建 `Folder/Git` 等可见硬过滤。Context 入口按 `Scope.contextKey` 切换当前根；Action Group 只做面板分段。Current App 的归属改为应用根；保留菜单 Hidden、Action ID、Usage 和执行复验规则。未来 Search Domain 另建类型筛选，避免继续超载 `ScopeEntry`。
5. **由 Core 排序，由 Host 导航。** Core 返回每个可操作根的全部 Offer、少量推荐和实际执行目标；Flutter 维护唯一 Scope、查询、面板及选中项。切换当前根不改 Context Graph，打开面板和浏览分段不记录 Usage；只在执行 Action 后记录。首页和面板都不能再次按 `group_id` 隐式截断 Offer。

## 验收场景

- 截图场景：目录焦点是 `build · 访达`，Git 是能力；没有 `Folder/Git` 平级 chip 或重复的 `Directory` 行。操作面板同时包含复制路径、终端、Git Status，后者标明仓库 `Anywhere`。
- Finder 多选两个 Git 项：保留一个集合焦点；从子项目录推导出的 Git 能力归属该集合或进入明确的单项选择，不生成误导性的第二个目录焦点。
- 浏览器里选中文字且有当前 URL：首选选中文字，可切到页面；由文字识别的 URL 不再形成第三个重复焦点。没有选中文字时页面成为首选。
- 前台应用菜单多时：首页只给少量推荐，切到应用焦点后可搜索完整菜单路径；缺少辅助权限只隐藏菜单，其他 Context 可用。
- 慢来源在用户已切到其他焦点或输入查询后完成：可以加入可切换项，不能抢回当前焦点或复活旧 Session 的操作。
- Context 查询和 Universal 查询的范围在搜索框与空状态中可见；同一 Action 的执行、Usage 和稳定身份不因焦点切换而改变。

外部产品的对象操作与动作面板可作为交互参照，见 [Raycast Action Panel](https://manual.raycast.com/action-panel)、[Alfred Universal Actions](https://www.alfredapp.com/help/features/universal-actions/) 和 [uTools 超级面板](https://www.u-tools.cn/docs/guide/uTools-super-panel.html)。Anywhere 的 Context Graph 与多来源血缘仍以本项目模型为准。
