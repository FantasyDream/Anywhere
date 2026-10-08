# Context 输入与默认唤醒

## 用户操作

不增加 Scope 或并列来源入口。Scope 的分类、Current App 聚合、筛选和切换沿用原设计。

| 默认唤醒时的内容 | 打开的页面 |
|---|---|
| 非空选中文本 | Global Search：自动填入选中文本，专用内容输入 UI |
| 没有选中文本，剪贴板复制年龄不超过 15 秒 | Global Search：自动填入剪贴板，专用内容输入 UI |
| 没有可用文本，剪贴板过期或年龄未知 | Current App：空输入，当前应用环境动作 |

内容 UI 标明 Selected text 或 Recent clipboard，并提示 Esc 返回 Current App。正文可以编辑；它是待处理对象，不作为动作标题查询。URL、JSON、路径等仍走同一 Context Graph、Registry 和 Ranking；只处理当前输入及派生 Context，不混入当前应用、网页、Finder 选择或其他捕获文本。

Esc 从内容页面清空并返回 Current App，重新解析当前应用环境；再次 Esc 进入完全空白 Global Search；再按 Esc 关闭窗口。菜单浏览、结果页及已有查询的返回逻辑仍保留。显式 Universal 唤醒直接进入空白搜索，绕过自动填入；显式全局动作快捷键独立使用完整采集图。

## 采集与生命周期

- 选中文本优先。采集尚未完成时短暂等待；500 ms 后可以回退到已捕获的新剪贴板。只在默认唤醒且用户尚未输入、移动选择或切换 Scope 时接受自动填入；用户操作、Esc、新 Session 或关闭取消资格。
- 剪贴板有效期固定 15 秒。年龄包含采集后到展示前的等待，重复打开/读取不延长有效期。启动前已有的剪贴板无法确认复制时间，不自动填入。
- macOS 每 250 ms 只观测 pasteboard changeCount；用单调时间记录变化区间起点，因此可能提前最多一个正常观测间隔过期，避免把旧内容当成新复制。快照前后 generation 不同则年龄未知。观察器不读取或保存正文；平台没有年龄能力时安全回退到 Current App。
- 本次成功采集的正文经 Capture DTO 提供给 Host；Settings 只保留来源、状态和捕获时间，丢弃正文与剪贴板年龄。正文不写入快捷键配置、Usage 或日志。

## 实现边界

`InputMode` 控制图的输入来源，独立于 Scope：

| 模式 | Context 图 | 查询 |
|---|---|---|
| Environment | 当前应用、Finder/浏览器环境及显式输入；排除自动文本来源 | 正常动作查询 |
| Content | 当前 UserInput 及派生 Context | 空查询，列出内容动作 |
| Search | 当前 UserInput 及派生 Context | 正常全局搜索 |
| CapturedEnvironment | 完整激活采集，仅显式全局动作及原 API 路径使用 | 既有查询 |

激活前选择 Environment，阻止首次或迟到文本采集混入 Current App。新输入、返回和模式变化产生新 Revision；Provider seeds 与提交处均按模式过滤，保留现有 Session / Revision / Catalog 执行门禁。模式切换期间隐藏旧列表；迟到采集仍可更新状态元数据。

## 回归验收

- Core：不同 URL 来源、实际执行目标、编辑、环境恢复、空白 Search、迟到采集只发布元数据、来源单次读取、原 Scope 测试。
- Flutter：15 秒边界、未知年龄、选中文本优先、显式 Universal、Esc 链、专用输入 UI、迟到采集与用户编辑、Settings 丢弃正文。
- 平台：启动基线年龄未知，新 generation 计时，重复读不续期，原生剪贴板有界读取。

2026-10-06：`scripts/check.sh` 完整通过，275 项 Rust 测试、107 项 Flutter 测试通过（4 项外部工具集成测试跳过），原生菜单/来源/Host、格式、Clippy、架构与前端分析通过。签名包 `0.6.0+9` 已检查新剪贴板自动填入、内容专用 UI、Esc 返回 Current App 后再进入空白 Global Search，以及旧复制内容不再次填入。自动化控制 TextEdit 的选区已建立，但系统实际前台仍为 ChatGPT/Obsidian，因此不将其计为 Selected Text 的实机捕获验收；选区优先和迟到保护由回归覆盖。

开发 App：`build/Anywhere-dev-0.6.app`；ZIP：`build/Anywhere-dev-0.6-macos-arm64.zip`。固定签名与 strict 验证通过，归档内容一致，SHA-256 校验通过。
