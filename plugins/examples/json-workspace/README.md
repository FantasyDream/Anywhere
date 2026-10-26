# JSON Workspace

普通第三方 Web 包示例：Preact 页面、jsonc-parser 格式化、jsonpath-js 查询、SandboxJS 计算、Anywhere 浏览器 SDK；无 JSON 专用 Rust API，无 Node 运行时。当前包版本为 0.5.3，使用 Host 的共享工作区样式与图标。计算和结果复制直接可见；缩进、复制变体与去转义收在输入区的“JSON 操作”中。

源码使用 `@anywhere/plugin-sdk` 标准包导入；本仓库开发时用 npm `file:` 依赖连接 SDK，独立工程可改为 SDK `.tgz`。通用打包器与新项目模板见 [第三方插件开发指南](../../../docs/plugin-development.md)。

## 构建与使用

在仓库根运行 `python3 scripts/build-web-examples.py`。产物是 `build/web-examples/dev.anywhere.json-workspace.awplugin`。

三个复制 Action 仅在识别到 JSON 对象 / 数组或转义 JSON 时显示；去转义 Action 仅在识别到转义 JSON 时显示。直接搜索名称只提供可空白打开的 Workspace。

1. 启动当前 macOS 预览版，在 Settings → Features → Web tools → Import 选择该包。JSON 包与 App 独立更新，已安装的旧包不会随 App 构建自动替换。
2. 核对名称、版本、SHA-256 和权限，批准后立即可用。可在 Global Search 搜索 `JSON Workspace`；直接输入合法 JSON 对象 / 数组也会匹配该 Action，打开后自动填入原文。选中或 15 秒内复制相同内容时沿用 Launcher 的自动填入规则；只有匹配成功才进入内容模式。已有旧包需重新导入更新。
3. 左侧输入 JSON，停顿 300 ms 后自动格式化；不完整 / 非法内容保持原样并提示错误。格式化保留光标所在的 JSON token，中文组合输入期间不处理。只修改公式时复用已经格式化的文本，不重复扫描整个文档；修改内容或缩进后重新格式化。
4. 默认使用 JavaScript 公式，固定显示 `this` 前缀。在顶部模式标签可切换 JavaScript / JSONPath，或用 Ctrl+Tab / Ctrl+Shift+Tab 切换。`$` / `this` 固定显示并自动加入公式，只需输入后面的内容；留空展示整个文档，完整公式也可直接粘贴。两种模式分别保留当前公式。`⌘↵` 或公式框中的 Enter 立即运行，也会在停顿后自动更新结果。`⌘⇧C` 请求复制右侧当前结果；输入或公式修改后，旧结果立即停止展示和复制。
5. Paste / Copy 会经过 Host 单次确认。Host 的 `⌘K` 提供重载和返回，Esc 关闭面板或返回搜索；中文组合输入时不抢 Esc。
6. 输入 / 输出不写本地数据库；只保存缩进偏好。隐藏后保留 15 秒，恢复重新签发调用资格，超过期限销毁。

工作区沿用 File Search 的页框和左右分栏：模式标签与 Scope / 类型标签相同外观，输入区域更宽，结果在右侧。代码字体、行距、浅透明底色与文件预览共用参数；取消重复大卡片边框和常驻说明行。背景仍由 Host 提供。

## 查询示例

以 `{"users":[{"name":"Ada","age":30},{"name":"林","age":20}]}` 为例：

| 模式 / 输入内容（不写前缀） | 右侧结果 |
| --- | --- |
| JSONPath / 留空 | 整个文档 |
| JSONPath / `.users[0].name` | `"Ada"` |
| JSONPath / `.users[*].name` | `["Ada", "林"]` |
| JSONPath / `.users[?(@.age >= 25)].name` | `"Ada"` |
| JSONPath / `.missing` | `[]`，标题显示 0 matches |
| JavaScript / `.users.map(x => x.name)` | `["Ada", "林"]` |
| JavaScript / `.users.filter(x => x.age > 25)` | Ada 的对象组成的数组 |
| JavaScript / `.users.reduce((sum, x) => sum + x.age, 0)` | `50` |
| JavaScript / `.users.map(x => x.name).join(", ")` | `"Ada, 林"` |

单个匹配直接展示该 JSON 值，多个匹配组成数组；复制保留显示的 JSON 表达形式，包括字符串引号。标题区分“未匹配”与“匹配到一个空数组”。重复键按最后一个值选择；选择整个对象时仍保留其原始重复键。

输入与输出上限各为 1 MiB，嵌套最多 128 层；查询最多 50,000 个 JSON 值、10,000 个匹配、1,024 字符的路径，并限制 AST 复杂度和访问次数。超限报错、不截断。仅缓存一个文档和一条编译后的查询。

格式化和结果提取保留整数精度、指数、原始转义、重复键和键顺序。数值过滤使用 JavaScript 数值：文档存在无法精确往返转换的数字时，拒绝数值比较；仍可直接按路径提取。也拒绝精度损失的查询数值字面量。当前不支持 `match` / `search` 正则过滤与脚本表达式。只接受严格 JSON，不接受注释 / 尾随逗号。处理在 Web 页面执行；没有后台 Worker，不宣称支持任意大文件。

自动匹配通过框架的 `inputMatchers: ["json", "json_escaped"]` 实现，上限为 16 KiB / 128 层，不匹配 `123`、`true`、`null` 等标量；这些内容仍可手动打开工作区处理。识别过程不启动 WebView、不构建 JSON 数据树，也不改变原文。

### JavaScript 的范围

支持同步计算表达式，包括属性 / 索引、算术、条件、模板字符串、箭头函数，以及 `map` / `filter` / `reduce` / `find` / `some` / `every` / `slice` / `sort` / `reverse` / `join` 等常用数组方法、基础字符串方法、`Math`、数值转换和 `Object.keys` / `values` / `entries`。每次处理分离的 JSON 副本，排序不会修改左侧源文档。

公式通过 Acorn 检查后交给 SandboxJS 解释执行，不使用原生 eval / Function，不改变 Host CSP；不接入 window / DOM、网络、文件、剪贴板或 Host API。只支持表达式和表达式形式的箭头函数，不支持声明、赋值、循环、函数块、new、async、正则、BigInt、动态代码或导入。单式最多 256 个 AST 节点、48 层和 20,000 个执行 tick；超限明确报错。执行与输出有预算，不是独立进程或硬堆内存沙箱。

JavaScript 使用常规 JS 数值；源文档存在无法精确往返转换的数字时拒绝计算，提示改用保留原值的 JSONPath。结果必须是 JSON 数据或顶层 `undefined`；不将非有限数值、函数等结果静默改成 null。JS 计算不会保留原始重复键 / 数字拼写 / 转义拼写，按 JS 值语义展示结果。

## 验证

`npm test` 覆盖精度 / 转义 / 重复键、scalar root、错误位置、预算与格式化幂等，以及路径 / 切片 / 过滤、空结果、原型隔离、查询预算、缓存恢复和格式化光标映射。生产浏览器集成与资源边界见 [平台设计](../../../docs/web-plugin-design.md)。同仓库的 Text Pad 不使用 UI 框架，两者走同一个 SDK 和 Host。

查询库固定为 [jsonpath-js 0.3.1](https://github.com/ashphy/jsonpath-js)，计算使用 [SandboxJS 0.9.7](https://github.com/nyariv/SandboxJS) 和 [Acorn 8.15.0](https://github.com/acornjs/acorn)，其 MIT 许可随插件运行资源一同打包。

启动 / 输入 / 隐藏恢复使用 SDK 的 `definePlugin`，默认控件与背景由 Host 注入，业务源码不再导入基础样式。


## JSON 复制与去转义 Actions

Launcher 可搜索以下命令；当输入对应内容时也会自动匹配。命令仍是普通 Web 插件入口，会打开工作区执行，不在后台偷偷读取剪贴板。

| Action | 行为 |
|---|---|
| Copy Formatted JSON | 复制整个左侧 JSON 的格式化文本，使用保存的缩进偏好 |
| Copy Minified JSON | 复制整个左侧 JSON 的无多余空白版本 |
| Copy Escaped JSON | 先压缩，再编码为一个带外层双引号的合法 JSON 字符串 |
| Unescape JSON | 解开外层字符串编码，将 JSON 放入左侧编辑器；不直接覆盖剪贴板 |

页面左侧底部的“JSON 操作”展开后提供这四项命令和缩进设置；右侧 Copy result 继续只复制查询结果。复制必须经 Host 本次确认，成功后才提示已复制；没有输入时只打开工作区，等待手动输入。

支持 `"{\"a\":1}"` 和 `{\"a\":1}` 等转义形式；自动匹配仍只接受对象 / 数组，原始编码文本最多 16 KiB、解码后嵌套最多 128 层、最多解开 4 层。普通 JSON 内字符串的合法转义不被替换。宿主只识别格式并原样传入，实际解码在插件内执行；工作区打开和 Paste 按钮会自动解码这些对象 / 数组。显式 Unescape 也可去除包裹 JSON 标量的一层编码，但普通搜索中的标量不会自动触发。

复制输出超过 1 MiB 时拒绝操作，不截断或写入部分结果。编码 / 解码只处理文本，不将文档转换成 JS 数值，因此格式化 / 压缩 / 转义复制保留大整数、指数和重复键。

UI 使用内嵌 i18next 中英资源，并通过 SDK 跟随宿主语言；切换文案不重建编辑器或重新计算正文。技术解析错误仍保留库提供的诊断。
