# Context 输入与默认唤醒

现行专题契约；文档整理：2026-11-18。Scope 与返回的完整规则见 [Launcher 交互](launcher-interaction.md)。

## 用户操作

不增加 Scope 或并列来源入口。Scope 的分类、Current App 聚合、筛选和切换沿用原设计。

| 默认唤醒时的内容 | 打开的页面 |
|---|---|
| 可解析为具体 Context，或被已批准且启用的 Web 工具（宿主匹配器或后台解析器）识别的选中文本 | Global Search：自动填入选中文本，专用内容输入 UI |
| 没有可识别选中文本，剪贴板复制年龄不超过 15 秒且满足同样的识别条件 | Global Search：自动填入剪贴板，专用内容输入 UI |
| 只有普通文本、对象解析失败、剪贴板过期或年龄未知 | Current App：空输入，当前应用环境动作 |

自动填入要求实际识别器产生 URL、真实文件 / 目录或第三方 `plugin.<package-id>.*` 等非 Text Context，或已批准且启用的 Web 工具通过声明的 `inputMatchers` 识别文本。仅有普通 Text 不满足自动跳转条件；无效 URL、不存在的路径和未被任何解析器或命令识别的文本不会打开内容页。识别失败的选区允许继续尝试有效剪贴板。手动输入和进入内容页面后的编辑仍沿用原图规则。

内容 UI 标明 Selected text 或 Recent clipboard。正文可以编辑；它是待处理对象，不作为动作标题查询。URL、路径等仍走同一 Context Graph、Registry 和 Ranking；只处理当前输入及派生 Context，不混入当前应用、网页、Finder 选择或其他捕获文本。

内容页的编辑器拥有焦点且没有修饰键时，`↑` / `↓` 切换候选动作，
`Enter` / `Return` 执行选中项；`Shift+Enter` 仍插入换行，其余修饰组合
保留编辑语义。

内容页第一次 Esc 返回 Current App，并清空内容输入；进入 Current App 后，空白
根页再次 Esc 切到空白 Global Search，Global 根页最后才隐藏。普通查询先清空，
菜单、对象 Actions、预览和 Web 内部层仍先退出所属局部层。显式 Universal
唤醒直接进入空白搜索，绕过自动填入；显式全局动作快捷键独立使用完整采集图。

## 采集与生命周期

- 可识别的选中文本优先。采集尚未完成时短暂等待；500 ms 后可以回退到已捕获的新剪贴板。只在默认唤醒且用户尚未输入、移动选择或切换 Scope 时接受自动填入；用户操作、Esc、新 Session 或关闭取消资格。
- 剪贴板有效期固定 15 秒。年龄包含采集后到展示前的等待，重复打开/读取不延长有效期。启动前已有的剪贴板无法确认复制时间，不自动填入。
- macOS 每 250 ms 只观测 pasteboard changeCount；用单调时间记录变化区间起点，因此可能提前最多一个正常观测间隔过期，避免把旧内容当成新复制。快照前后 generation 不同则年龄未知。观察器不读取或保存正文；平台没有年龄能力时安全回退到 Current App。
- 识别在 Rust 后台 worker 执行，路径检查不阻塞界面，窗口先正常显示 Current App。Host 单次只保留一个待识别请求；新捕获到达后重新判定，识别返回时复验交互 / 生命周期资格，并检查 Rust 提供的剪贴板到期时间。输入、Esc、关闭或新 Session 后的旧结果不能自动跳转。
- Current App 的页面提示只使用其实际消费的环境来源；选区 / 剪贴板读取失败导致自动填入回退时，保留 Settings 中的状态，不显示为 Current App 动作加载失败。真实 Finder / 浏览器和菜单异常提示保留。
- 本次成功采集的正文经 Capture DTO 提供给 Host；Settings 只保留来源、状态和捕获时间，丢弃正文与剪贴板年龄。正文不写入快捷键配置、Usage 或日志。

## 实现边界

`InputMode` 控制图的输入来源，独立于 Scope：

| 模式 | Context 图 | 查询 |
|---|---|---|
| Environment | 当前应用、Finder/浏览器环境及显式输入；排除自动文本来源 | 正常动作查询 |
| Content | 当前 UserInput 及派生 Context | 空查询，列出内容动作 |
| Search | 当前 UserInput 及派生 Context | 正常全局搜索 |
| FileSearch | 独立文件工作区，不解析环境 Context | 文件查询 |
| CapturedEnvironment | 完整激活采集，仅显式全局动作及原 API 路径使用 | 既有查询 |

激活前选择 Environment，阻止首次或迟到文本采集混入 Current App。新输入、返回和模式变化产生新 Revision；Provider seeds 与提交处均按模式过滤，保留现有 Session / Revision / Catalog 执行门禁。模式切换期间隐藏旧列表；迟到采集仍可更新状态元数据。

## 状态保留与验证

有查询或工具 / 二级操作时，隐藏后 15 秒恢复原内容和 Context。空白 Global / Current App 首页按本次入口新建 Session，重新采集前台应用。菜单、Scope 与来源定义不因恢复而改写。

回归覆盖来源优先级、过期 / 未知复制年龄、输入编辑、迟到结果不覆盖用户交互、Esc 链、环境恢复、来源单次读取和设置丢弃正文。历史真实采集与包验证边界见 [V0.6 记录](archive/v0.6-final-acceptance.md)，当前工程状态见 [V0.14 技术方案](mvp-gpui.md)，此前真实键盘基线见 [V0.12 归档](archive/mvp-migration/v0.12-flutter-era.md)。

JSON Workspace 作为内置 Web 插件随主程序自动启用，无需手动导入与批准（仍可在 Web tools 管理页禁用或启用，不支持卸载或被本地同 ID 包覆盖）。其按需后台 QuickJS 解析器识别 JSON 对象 / 数组或转义 JSON 容器并产出 `plugin.dev.anywhere.json-workspace.json`，唤醒时可自动进入内容页并在执行命令后带入原文；识别期间不启动 WebView。第三方插件也可按需声明宿主的固定 `inputMatchers`（`json` / `json_escaped`）。禁用或撤销后立即停止自动识别，没有其他识别器命中时回到 Current App；选区优先级和剪贴板 15 秒期限保持不变。插件声明与输入边界见 [开发指南](plugin-development.md#自动匹配-json)。

剪贴板自动填入开关及上次采集状态位于 Settings → Clipboard，与历史采集开关分别控制；Finder、选中文本和浏览器来源仍位于 Settings → Context。
