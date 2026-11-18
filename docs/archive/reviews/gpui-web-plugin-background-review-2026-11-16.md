# GPUI Web 插件白底与输入误传排查（2026-11-16）

范围：用户提供的 JSON Workspace 截图，以及 `b428899` 上当前包含透明 / 明暗
配色改动的工作树。当前工作区已有用户修改，本轮只做诊断和方案记录，未实施
产品修复。临时 Core 诊断源码已清理，并确认文件 SHA-256 与诊断前一致。

结论：确认两个独立缺陷，分别解释白底浅字和打开后立即出现的 `InvalidSymbol`。
JSON Workspace 是通过统一 SDK 运行的 Preact Web 页面；此处应修复宿主透明
配置和 Core 输入版本基线，无需重写插件或更换渲染器。

实施状态（同日）：第 1–3 节的修复已进入当前工作树；原生透明 feature、执行
revision 基线、普通控件 scheme 与 JSON 固定深色菜单语义变量均有自动回归。
当前 GPUI 包也已在白色桌面的 Dark 材质下确认 `json` 搜索打开空正文、无白底
WebView 块、固定深色菜单可读、合法 `[]` 可格式化、非法 JSON 显示准确错误，
返回再打开仍为空；切到其他应用后的 15 秒内恢复沿用同一 origin 和新 document
epoch，仍为空正文。第 4 节列出的 Light Glass、macOS 15、Reduce Transparency、
IME 和完整生命周期矩阵仍需继续验收。

## 1. 白底浅字：原生透明功能未编入当前依赖

入口：[Cargo.toml](../../../apps/gpui/Cargo.toml) 的 `wry` 依赖，以及
[ensure_web_view](../../../apps/gpui/src/launcher/view/mod.rs) 的 WebView builder。

代码已经调用 `.with_transparent(true)`；网页也由
[canvas_script](../../../apps/gpui/src/web_tool.rs) 将背景切为 transparent。
但锁定的 `lb-wry 0.53.3` 在 macOS 中只有启用 Cargo feature `transparent`
时，才会执行关闭 WKWebView 背景绘制的 `drawsBackground` 配置。该 feature 不在
默认集合中，GPUI 和 `gpui-webview 0.9.1` 当前均未启用它。

已检查实际 macOS 依赖图：

```sh
cargo tree -p anywhere-gpui --locked --offline --target aarch64-apple-darwin \
  -e normal -i lb-wry --format '{p} features=[{f}]'
```

输出 feature 集合中没有 `transparent`。增加临时命令行
`--features wry/transparent` 后，同一个 `lb-wry` 实例出现该 feature，GPUI 和
`gpui-webview` 共用此实例；这一比较只检查依赖解析，没有修改 manifest 或运行 App。
上游说明：[lb-wry 0.53.3 features](https://docs.rs/crate/lb-wry/0.53.3)。

因此当前路径实际是：网页透明 → 原生 WebView 继续绘制自己的背景 → 下层 GPUI
玻璃被挡住。叠加深色主题的浅色文字，就能解释截图的大面积白底和近乎消失的
按钮 / 标签。CSS transparent 和 builder 参数存在，均不能单独证明原生透明。
该缺陷影响使用同一宿主的 Web 插件，不限于 JSON Workspace。

推荐在 `apps/gpui/Cargo.toml` 为现有依赖启用 feature：

```toml
wry = { package = "lb-wry", version = "=0.53.3", features = ["transparent"] }
```

继续保留 `.with_transparent(true)`、当前原生材质和文档透明策略。此处只需重新
构建 Host，不需要为白底问题重新导入或重写插件。该依赖通过已有的 KVC 实现
透明，当前方案不自行新增一套 AppKit 背景控制。

若暂时采用不透明 Web 表面，则让 Web 根使用明确的不透明深色背景和配套浅字，
同时停止把其 canvas 改成 transparent；这能恢复可读性，但 Web 区域不再通透。
提高整个 GPUI 窗口遮罩覆盖率无法消除位于其上方的原生 WebView 白底。

## 2. InvalidSymbol：搜索词经状态订阅进入正文

相关调用链：

1. [EntrySource](../../../crates/anywhere-core/src/features/tools.rs) 为名称搜索创建
   空的 Launcher Entry Context，`ResolvedContext::new` 的 revision 默认为 0。
2. [执行 worker](../../../crates/anywhere-core/src/application/execution_request/worker.rs)
   用该 Context 启动 Web invocation；
   [invocation.rs](../../../crates/anywhere-core/src/application/web_plugins/invocation.rs)
   把 `context.revision` 保存为 `launch_revision`。
3. [web_snapshot](../../../crates/anywhere-core/src/application/web_plugins/host_state.rs)
   看到真实 session revision 大于 0，就误以为有启动后的新输入，将允许读取的
   `UserInput / core.text` 搜索词发布为 `snapshot.context`。
4. SDK 的 `context.onChange` 会送出首次状态；插件的
   [workspaceInput](../../../plugins/examples/json-workspace/src/workspace-input.mjs)
   在用户尚未编辑时接受它，于是原本空白的 JSON 编辑器被填入 `json`。

已用当前 Core 的真实更新与执行入口、既有 `json-v0.5.4` 兼容包夹具和合成输入
完成定向复现；当前 0.7.1 示例同样声明了此处涉及的 `context.read / core.text`：

```text
Global 搜索 json，session revision = 1
候选 Entry Context revision = 0
invocation.input = Empty { reason: NoObject }
首次 snapshot.context = Text { source: UserInput, text: "json" }
```

再把这个状态交给插件当前的 `workspaceInput`，观测结果是输入 4 字节 `json`，
错误恰为 `InvalidSymbol · line 1, column 1`，与截图一致。合法合成 JSON 则正常
通过。问题不只在 JSON 解析器；名称搜索和正文被错误的版本比较混在一起。
具备 `context.read / core.text` 权限的空白工作区也可能遇到同类误传。

推荐修复：Web invocation 的启动基线使用**已提交执行请求的 session / revision**，
从执行 worker 显式传入，在现有锁与时效检查内冻结；Context 自身的解析 / 捕获
版本继续承担原有职责。首次状态订阅与启动处于同一输入 revision 时，保留冻结
的 `invocation.input`，名称搜索入口保持 Empty。真正发生更高 revision 的后续
输入更新时，才按现有授权与类型规则投影新 Context。

这项修复属于共享 Core，可同时影响不同前端。插件继续保留对用户手动输入非法
JSON 的错误提示，不能用隐藏错误条代替修复输入来源。

新增回归应覆盖：

- 在 revision 1 和更高 revision 搜索 `json` / `jsonpath` / 名称，首次 invocation
  与状态订阅都保持空正文。
- 从真实 JSON Context 打开时，首次状态保持选中的原文及数字 / 重复键语义。
- 启动后的真实输入更新仍能传播；同 revision 的 snapshot_seq 变化不误作新正文。
- Reload / 隐藏恢复保留文档和输入契约；替换文档后的旧回复仍被句柄 / epoch 拒绝。

## 3. 当前明暗配色改动还需补齐的 Web 规则

以下是当前工作树的源码问题，和截图的原生白底缺陷分别处理；尚未进行对应实机
明暗切换验收。

- [web-controls.css](../../../resources/web-controls.css) 为每个 button / select /
  input / textarea 写了 `color-scheme: dark`。即使根节点切到 light，控件自身的
  声明仍覆盖继承值。普通控件应跟随宿主选择的 scheme；固定深色弹层单独指定。
- JSON 的 `.json-menu-panel` 是 `role="group"`，使用固定深色 `menuSurface`，
  但内部按钮和缩进标签仍用根级 `--aw-text` / `--aw-secondary`。浅色玻璃选择
  深色文字时，该弹层会变成深底深字，现有 `[role="menu"]` 规则不会覆盖它。
  应在这个局部表面明确绑定 `--aw-text`、Primary / Secondary / Muted 和交互边界
  到现有 `--aw-on-dark-*` token，覆盖内部按钮、select 与标签，而不只设置父级
  `color`。若修改插件自身 CSS，需构建新包并显式更新安装；不改写校验后的包缓存。

首帧、ready、Reload 与外观变化应复用同一份背景 / 字色决策，按当前文档句柄
幂等应用。没有把“初始化脚本必然丢失”列为已确认原因：当前 lb-wry 会把初始
`eval` 排队到 navigation commit；是否存在首帧错色仍需加载过程实测。

## 4. 验证状态与实施顺序

本轮已完成：

- macOS 默认 feature 与临时启用 `transparent` 的依赖图对比。
- Core 全局搜索 → 执行 → 首次 Web snapshot 的一项定向诊断，确认关键词误传。
- 插件输入的两组观测：`json` 复现截图错误；合法合成 JSON 无此错误。
- 清理诊断后 Core `web_plugins::input_tests` 原有 9 项通过；JSON Workspace 原有
  30 项测试通过；GPUI `web_tool::tests` 原有 5 项通过。
  这些现有测试未覆盖原生 feature 和首次状态正文一致性，不能当作修复通过。
- 临时 Core 源码已撤下，诊断前后 SHA-256 相同。重放源码与输出保留在
  `build/gpui-web-plugin-review-20261116/`，不随 Git 交付。
- 只读进程快照发现一个 GPUI `v167` 验收包仍在运行，另有 Flutter App；截图本身
  不能确定构建身份。当前工作树新增的明暗配色不能自动视为已在该运行包中生效。
  本轮未启动、重启或操作 App，没有取得修复后二进制的原生 A/B 证据。
- 文档检查通过：103 个 Markdown、710 个本地链接；差异空白检查通过。

实施顺序：先启用透明 feature，修复 Web 启动 revision 基线并增加完整状态回归，
再补齐控件 scheme 与固定深色弹层的语义变量。运行受影响的 Core / GPUI / 插件
测试，再运行 `scripts/check-gpui.sh` 和 `scripts/check.sh`，之后由维护者安排
物理验收。

人工验收保持 Pending：

1. 核对新包的可执行文件、构建号和进程路径，使用隔离数据和合成 JSON。
2. 在白底、暗底和彩色桌面打开 JSON Workspace 与 Text Pad；普通模式无原生
   WebView 白块，明暗字色可读；Reduce Transparency 使用配套不透明背景。
3. 搜索 `json` 打开后正文为空，不显示由搜索词触发的错误；粘贴合法 JSON 能格式化，
   手动输入非法 JSON 仍显示准确错误。
4. 验证 JSONPath 和 JavaScript 查询、结果及合成内容复制；检查窄窗口、滚动、
   输入法组词、选区与光标，不用解析测试替代原生交互证据。
5. 切换系统明暗和辅助功能，检查菜单、缩进选择、placeholder、错误提示与键盘
   焦点。正文保持 4.5:1、语义图标 3:1；亮暗桌面交叉组合单独记录。
6. 覆盖首次加载、Reload、返回再打开、15 秒隐藏恢复、插件替换与关闭；旧文档
   事件不能改当前页面，换主题不清空已编辑正文。
