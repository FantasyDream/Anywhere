# 第三方 Web 插件 Phase 1 实施手册

配套[架构审查](web-plugin-evolution.md)。以 `964dd1e1` 为审查基线。本页给出实施默认值、任务顺序和验收标准，所有实施任务目前为 Pending。当前产品行为见[插件开发指南](plugin-development.md)和[Web 插件架构](web-plugin-design.md)。

完成后的两条用户路径为「取得 SDK → 创建项目 → 浏览器预览 → 校验与打包 → 在 Anywhere 审查并试用」和「对 Launcher 中的对象执行插件 Action → 在宿主授权下打开站点、读取选中文件或预览」。JSON Workspace 同时验证双平台快捷键和命令本地化。

## 如何接手

1. 从仓库根运行 `git status --short --branch`、`git log -1 --oneline` 和 `git diff --stat`。读 [AGENTS.md](../AGENTS.md)、[开发与构建](development.md)和本页。已存在的改动先辨认归属，保留用户工作。
2. 按编号执行。每次只取一个任务，先打开该节链接对应的入口和测试。已有部分实现时先跑该节验收，补齐未满足的行为。所有路径和命令均从仓库根解释，Windows 用对应 PowerShell 环境。
3. 每项依次做「现状验证 → 最小变更 → 生成物同步 → 局部测试 → 差异检查」。对顺序问题优先用可控的阻塞点和测试夹具，避免依赖睡眠。新任务可独立推进的条件见下表。
4. 在交接中记录任务编号、受测提交、实际改动、命令及结果、缺少的桌验环境。使用 Pending、已实现、自动化通过、桌验通过四种状态。仅关闭满足该节完成条件的工作。使用编译成功或 Mock 通过时，桌验栏仍留 Pending。

可把下面这段直接交给接手模型，替换任务编号。第一次执行用 01。

> 阅读 AGENTS.md 和 docs/web-plugin-phase-1.md。执行任务 01，先验证前置条件，再按该节编号修改。以本手册的版本和权限契约为准。使用该节列出的入口和检查命令。最后给出改动文件、实际结果和未完成项，供下一次接手。

| 任务 | 产出 | 前置 |
| --- | --- | --- |
| [01](#01-记录基线与保留回滚) | 基线、同摘要重复导入、参考包可复现 | 仓库工具链 |
| [02](#02-落定版本与生成契约) | v1 / v2 兼容、Manifest 与命令元数据 | 01 |
| [03](#03-交付-node-cli) | Node 校验、打包、项目创建与互通测试 | 02 |
| [04](#04-交付-mock-与项目模板) | Mock、开发入口、混合模板及 Agent Skill | 03 |
| [05](#05-迁移跨平台快捷键与示例) | 平台元数据、同源键帽与监听、内置示例 | 02。Mock 验收依赖 04 |
| [06](#06-后台完成语义与诊断) | 后台结果、正确 Usage、可定位脚本错误 | 02 |
| [07](#07-宿主窗口命令与外链) | 关闭与返回、定域或逐次确认的外链 | 06。弹窗与窗口各自验证 |
| [08](#08-选中文件的身份与读权限) | 原生身份、句柄、stat、受限文本读取 | 02、06 |
| [09](#09-把预览接到原生窗口) | 独立 grant、原生预览、关闭与释放 | 07、08 |
| [10](#10-统一插件安装与升级审查) | 审查卡、拖放、原生对象动作、双平台系统打开 | 01、02 |
| [11](#11-打包验收与交付) | 发布候选、兼容矩阵、桌验记录与文档 | 前序任务 |

## 实施时共用的契约

### 兼容、身份与额度

- **版本。** 保留 `schemaVersion: 1`、桥的 `protocol: 1`。本阶段的新声明与操作使用 Manifest 的 `apiVersion: "2"`。新宿主同时接受 API 1 和 2。API 1 继续检查旧字段、旧权限名、八项权限上限及旧后台返回语义。API 2 权限上限为 16。CLI 以 Manifest 的 API 作为目标，给出最低宿主兼容信息。只有包含新行为的项目才生成 API 2。基础模板提供 API 1 产物的路径。
- **编辑器与归档。** 源 Manifest 可包含顶层 `$schema`。Node 打包器生成归档 Manifest 时仅移除此字段，按固定 UTF-8 / LF 与末尾换行序列化。Rust 只读取归档的实际内容并计算摘要。保留包内未知字段拒绝，包括显式写入默认值的 API 2 专用字段。需要判断字段存在时，在给默认值前检查序列化输入。重复 JSON 键应在折叠对象前报错，Node 和 Rust 用同一类夹具核对。
- **身份。** 稳定的 `web:<package-id>:<command-id>` 用于绑定和 Usage。批准绑定导入捕获的摘要。执行还须通过当前安装、会话、文档及请求检查。热键使用当前候选。只要摘要、权限或资格失效就撤销旧文档与调用，沿用现有目录发布规则。
- **额度。** 桥沿用现有请求、帧、传输、并发和超时常量。按 UTF-8 字节计数。文件 readText 上限 1 MiB。用已有大结果传输，并验证 JSON 编码后的上限，原始文件在上限以内但编码后的响应超过传输额度时返回 `TOO_LARGE`。文件路径、URL 参数、插件输入以及原始异常正文均不进入默认日志。

### 能力和运行角色

允许调用还需当前 API 版本、安装 grant、适用对象及宿主可用性。下表是实施默认值。所有能力经 Rust 检查，SDK 和 Mock 给作者提供相同的输入、返回值与错误语义。

| 调用 | Web 页 | 显式后台 Action | 自动 resolver | 成功结果 |
| --- | --- | --- | --- | --- |
| `window.back` / `window.close` | 当前文档 | 拒绝。使用后台返回值 | 拒绝 | 宿主接受当前窗口命令 |
| `url.open` | `url.open` grant，宿主确认 | 同左 | 拒绝 | 系统已受理打开请求 |
| `resources.stat` / `resources.readText` | `resources.read` grant、当前文件句柄 | 同左 | 拒绝 | 类型与有界元数据 / 文本 |
| `preview.open` | 独立 `preview.open` grant、当前文件句柄 | 拒绝 | 拒绝 | 当前原生预览已受理 |

- **外链。** 权限形状为 `{ "name": "url.open", "origins": ["https://issues.example.com"] }`。允许准确的 HTTP(S) origin，按 scheme、规范化主机、有效端口比对，最多 16 项。通用链接工具可显式声明单独的 `["*"]`。审查区区分具体站点和任意 HTTP(S)。所有打开请求都经宿主确认，显示包名和完整目的地。URL 最长 8 KiB，拒绝用户信息、无主机及其他 scheme。浏览器只负责接收外链，桥返回受理结果。
- **资源。** 首版实例的授权对象是启动时明确从 Finder / Explorer 选中的单个普通本地文件。文件列表、目录和其他来源继续可显示元数据，真正的读取和预览以可验证的单文件租约为准。授予 `resources.read` 或 `preview.open` 需同时声明 `context.read` 的 `core.file`，命令通过 `accepts` 取得文件 Context。后来出现的查询或 resolver 数据不能产生授权。stat 返回 `handle`、`name`、`kind`，可选 `bytes`、`modifiedMs`。元数据超出 JS 安全整数范围时省略。readText 的可选 `maxBytes` 是 1 至 1 MiB 的整数，省略则取 1 MiB。用上限加一字节判断超出，允许空文件，拒绝非法 UTF-8 和含 NUL 的内容。
- **错误。** 调用角色或授权不符返回 `PERMISSION_DENIED`。参数非法返回 `INVALID_REQUEST`。句柄撤销、对象替换或对象消失返回 `EXPIRED`。宿主没有相应适配返回 `UNSUPPORTED_METHOD`。有效文档内用户取消确认返回 `USER_CANCELLED`。RPC 已失活依旧沿用当前 `INACTIVE` / `UNTRUSTED_DOCUMENT` 规则。SDK 对原生副作用不自动重试。
- **后台反馈。** API 2 显式 Action 接受 `null` / `undefined`，或仅含 `message`、`closeWindow` 的对象。`message` 最多 512 字节，以纯文本显示。`closeWindow` 默认为 `true`。需要用户看见消息时插件返回 `closeWindow: false`，由原搜索页显示。失败和取消保持打开。旧 API 的任意可序列化返回值继续忽略，沿用原关闭行为。

## 01 记录基线与保留回滚

1. 运行下方检查，区分本地环境问题和基线差异。前次已观察到 Rust 格式差异和 JSON Workspace 内置包的 Manifest CRLF/LF 差异。重新确认后记录具体文件。仅在对应改动范围内修正格式。把仓库中用于打包的 Manifest 换行固定为 LF，使用仓库生成脚本同步内置包。
2. 在现有 package tests 中加入 A → B → 重复导入 B → 重开 store → 回滚 A。另保留已有损坏资源重装测试。调整 `approve` 时先验证资源完整。同摘要、已启用且资源完好的安装沿用 receipt、上一版和文档。停用的同摘要包保持停用，由启用入口显式启用。同摘要内容损坏时沿用现有修复路径并保留回滚点。
3. 运行局部检查。完成条件是重复导入不会触发无意义的 catalog 发布，坏包仍可修复，原始摘要审查和回滚回归通过。

入口见[包审批](../crates/anywhere-core/src/application/web_plugins/packages.rs)、[包测试](../crates/anywhere-core/src/application/web_plugins/package_tests.rs)和[参考包生成器](../scripts/build-web-examples.py)。

```sh
cargo fmt --all -- --check
cargo test -p anywhere-core --lib --locked application::web_plugins::package_tests
scripts/check-web-examples.sh
```

## 02 落定版本与生成契约

1. 先从现成的 Text Pad / JSON Workspace 包取得 API 1 夹具。增加 API 1 仍可导入、API 1 声明新字段或权限时报错、API 2 合规可导入及未知 API 被拒的测试。分别测试源码 Manifest 和最终归档。
2. 在协议 DTO、Manifest 校验和生成器实现版本范围。命令元数据为 `localizedTitles`、`subtitle`、`icon`。Phase 1 支持 en / zh，缺省回退到 `title`。副标题缺省为插件名。命令图标为受支持的内置图标名，未知值回退 `code`。包图标仍走现有 PNG 规则。由 catalog 投影到已有 ActionDefinition，使现有 aliases、拼音和展示生效。
3. 在本任务只公开已接通的元数据。后续任务每接通一个调用，就同步其 WebCall、错误与 TS 类型。保持背景结果中的 `closeWindow`、读取参数中的 `maxBytes`、元数据中的 `modifiedMs` 在线上使用 camelCase。新增权限同时更新 GPUI 中英文权限说明和生成物。
4. 让 Manifest 生成器同时投影给仓库和 SDK。归档 schema 保持严格。编辑器版 schema 允许源字段 `$schema`。对字段、版本、缺省值及权限数组做双向序列化验证。完成条件是 v1 与 v2 都能用各自的夹具进入真实导入器，标题翻译不会改变 Action ID。

入口见[Manifest DTO](../crates/anywhere-protocol/src/web_plugin.rs)、[语义校验](../crates/anywhere-plugin/src/web/manifest.rs)、[目录注册](../crates/anywhere-core/src/application/web_plugins/catalog.rs)、[生成说明](development.md#生成物)及[权限文案映射](../apps/gpui/src/host/runtime_support.rs)。

```sh
cargo run -p anywhere-protocol --features web-schema --example generate_web_plugin_schema --locked
cargo run -p anywhere-protocol --features web-schema --example generate_web_sdk_contract --locked
cargo test -p anywhere-protocol -p anywhere-plugin --locked
cargo test -p anywhere-core --lib --locked application::web_plugins
```

将上述两个生成命令末尾追加 `-- --check` 再运行。修改权限文案后运行 `python3 scripts/generate-gpui-localizations.py` 及同命令的 `--check`。

## 03 交付 Node CLI

1. 将 SDK 的 bin 指向统一的 Node CLI。先把 `init` 接到原创建器，保留默认 Web 和 `--background` 参数及已有目录拒绝行为。后续模板统一用 `npm run pack` 调用新 CLI。
2. `anywhere-plugin validate [dir]` 默认检查源 Manifest，提示 JSON 字段与权限组合错误。`--built` 额外核对构建入口、图标和资源。 `pack [dir] --output <file.awplugin>` 先走构建产物校验，再生成临时归档，成功后替换目标文件。把规则差异定位在校验器，Rust 导入保持最终权威。
3. 沿用当前打包资源白名单和 `--assets`，默认收集 `manifest.json`、`dist/`。生成 ZIP 时固定 UTF-8 路径、字节排序、时间与普通文件 mode。使用标准 raw DEFLATE 和 CRC，压缩比超出现有 200 倍边界时使用 Stored。遵守包 entry 数、单资源、压缩与展开总量上限。拒绝符号链接、Unicode / 大小写冲突及 Windows 保留名称。维护者脚本可继续使用 Python 校验旧兼容包。
4. 用同一批合法 / 非法样本跑 Node CLI 与 Rust 检查器。对相同资源字节比较内容摘要，对同 Node 版本和固定输入比较 ZIP 哈希。对初次未构建、资源缺失、二次打包及失败后重试分别回归。完成条件是在仓库外用 Node 完成生成、构建和打包，得到可由 Rust 导入的包。

入口见[SDK 包清单](../packages/plugin-sdk/package.json)、[现有创建器](../packages/plugin-sdk/tools/create.mjs)、[原打包器](../packages/plugin-sdk/tools/package.py)、[ZIP 边界](../crates/anywhere-plugin/src/web/preflight.rs)、[归档校验](../crates/anywhere-plugin/src/web/archive.rs)和[仓库外测试脚本](../scripts/check-plugin-starter.sh)。

```sh
scripts/check-web-sdk.sh
scripts/check-plugin-starter.sh
```

## 04 交付 Mock 与项目模板

1. 将 Mock 作为独立的 `@anywhere/sdk/mock` 浏览器与测试导出。用现有 SDK transport 提供启动与订阅、临时 KV、合成剪贴板和文件。模拟隐藏恢复、撤销、超限与取消。新能力后续随真实实现接入 Mock。调试面板显示当前语言、平台、主题、合成输入与结果码。
2. 补齐 `init --ts` 和 `init --hybrid`。混合项目包含 Web 入口、纯读 resolver、显式复制 Action 和共享设置。resolver 的 TypeScript 接口只展示读取方法，QuickJS 的运行期权限仍由 Rust 校验。普通 Web / TS 模板的基础包可用 API 1。使用多语言声明的混合模板明确标为 API 2。
3. 给 Web / TS / 混合模板提供本地 `npm run dev`，预览构建和产物构建用各自入口。浏览器预览从 Mock 读取 fixture，正式 bundle 中无 Mock 注入。原始后台模板用 QuickJS conformance 运行。开发或 CI 的样本均使用临时目录和合成数据。
4. 将插件开发 Skill 放在单一源位置，构建 SDK 时复制，项目创建时将其投影到 `.agents/skills/anywhere-plugin-dev/`。附普通作者指南，按客户端验证 Skill 是否发现。完成条件是四种模板均能在仓库外构建，混合包的 Web 与后台分别工作，导出的 SDK 含 schema、Mock、CLI、模板、Skill、README、LICENSE。

入口见[SDK transport](../packages/plugin-sdk/src/client.ts)、[SDK 测试](../packages/plugin-sdk/test/client.test.mjs)、[QuickJS 入口](../packages/plugin-sdk/src/background.ts)、[旧模板](../packages/plugin-sdk/templates/minimal)和[混合示例](../plugins/examples/json-workspace)。

```sh
scripts/check-web-sdk.sh
scripts/check-plugin-starter.sh
```

## 05 迁移跨平台快捷键与示例

1. 与语言元数据一起向 WebView 注入 Host platform。SDK 优先读宿主。在浏览器中优先读 Mock 值，缺失时按浏览器平台推断。添加统一的 `environment.formatShortcut` 和 `environment.matchesShortcut`。Mock 切换平台后触发预览重绘。
2. 匹配使用相同规格。 `Mod` 在 macOS 对应 Meta，在 Windows 对应 Ctrl。Ctrl、Alt、Shift 可显式声明。比较确切修饰键，忽略组词、AltGr、已阻止或重复的按键。JSON Workspace 保留 Ctrl+Tab、编辑器组词和标准复制粘贴。
3. 一并修改 JSON Workspace 的键帽、帮助提示、键盘监听及 Manifest 中英文标题。同步内置包。完成条件是同一规格在两个平台的格式化、执行、复制、聚焦均匹配，语言切换后命令 ID、快捷键与 Usage 延续。物理 IME、按键及焦点分别按两端桌验。

入口见[平台键盘辅助](../apps/gpui/src/platform/keyboard.rs)、[Web 引导脚本](../apps/gpui/src/web_tool.rs)、[SDK 环境](../packages/plugin-sdk/src/environment.ts)、[JSON Workspace 监听](../plugins/examples/json-workspace/src/main.jsx#L266)和[帮助文案](../plugins/examples/json-workspace/src/i18n.mjs)。

```sh
scripts/check-web-sdk.sh
npm --prefix plugins/examples/json-workspace test
python3 scripts/build-web-examples.py --sync-builtin
scripts/check-web-examples.sh
```

## 06 后台完成语义与诊断

1. 移除后台的模拟 ready 和提前 Usage。后台经过正常 admission 后运行导出函数。按 API 版本解释返回值。由执行用例完成 Usage、返回 `ActionResult`。普通 Web 页面仍在有效 ready 时记一次 Usage。解析返回值失败时不自动重跑已调用过的宿主操作。
2. 给异常提供结构化类别，区分缺少导出、JS 异常、取消、执行预算和内存错误。只能从可信运行时状态判定的类别使用专用码。无法区分时用通用脚本失败。开发调试可显示有界的堆栈位置，截断至最多 1 KiB，默认日志只保留包 ID、入口及类别。
3. 将 API 2 的文本反馈接到 GPUI 原搜索页，验证 `closeWindow: false` 保留输入与选择。默认关闭沿用预览、Web 清理顺序。
4. 完成条件是四类夹具从真实 Launcher Action 入口通过。旧 API 返回任意对象仍成功并关闭，新 API 合规结果成功，脚本失败或解析失败不记 Usage，写入 Usage 失败按已有动作警告处理。已经成功的动作即使导致失焦也只记一次，陈旧页面接收不到新的反馈。

入口见[后台命令](../crates/anywhere-core/src/application/web_plugins/background_action.rs)、[QuickJS](../crates/anywhere-core/src/application/web_plugins/background_runtime.rs)、[执行用例](../crates/anywhere-core/src/application/execute.rs)和[GPUI 结果](../apps/gpui/src/launcher/view/mod.rs#L2056)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins
scripts/check-web-sdk.sh
scripts/check-gpui.sh
```

## 07 宿主窗口命令与外链

1. 先实现窗口命令。Rust broker 按当前文档和 request ID 签发一次性宿主意图，经 frontend runtime 与 `LiveEvent` 交给 GPUI。控制器根据自身当前窗口和文档代次消费意图。正常返回和关闭均复用现有导航及清理方法。SDK 的成功是受理，随后立即销毁页面时 Promise 也可能随页面消失。页面不能用成功回复字符串自行触发窗口动作。
2. 接入外链 grant、审核说明与参数校验。确认 UI 走当前交互窗口，后台 Action 使用发起动作的 Launcher 窗口。由宿主保存本次不可变目标。沿用取消信号，确认等待单独限时 30 秒，释放阻塞 worker，普通 RPC 继续用现有超时。对齐 SDK 等待期限、Rust request 生命周期与 QuickJS 等待，不把用户思考时间计入脚本执行预算。确认后复验 grant、文档、request ID，再调用平台打开 API。外链造成失焦时保留已受理的效果，旧窗口的回复、焦点和关闭仍受代次保护。
3. 用可控的迟到回复验证插件 A 更换成 B 后 A 的 back、close 或确认失效。覆盖主窗口 back / close、独立窗口关闭、重复确认、目的地越界、用户取消及 resolver 拒绝。完成条件是一项用户批准只产生一次系统打开，失败时原窗口有可理解的反馈。

入口见[broker](../crates/anywhere-core/src/application/web_plugins/broker.rs)、[RPC 调度](../crates/anywhere-core/src/application/web_plugins/rpc_dispatch.rs)、[frontend runtime](../crates/anywhere-core/src/application/frontend_runtime/mod.rs#L408)、[GPUI Web 事件](../apps/gpui/src/host/runtime/web.rs#L95)、[回复控制器](../apps/gpui/src/launcher/view/controllers/web.rs#L331)和[平台打开接口](../crates/anywhere-platform/src/traits.rs#L279)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins::broker_tests
scripts/check-web-sdk.sh
scripts/check-gpui.sh
```

## 08 选中文件的身份与读权限

1. 在可信选区采集入口为单文件捕获平台对象身份。macOS 使用设备和 inode，Windows 使用卷及文件 ID。平台适配负责拒绝链接、重解析点、目录和特殊文件，使用原生可信打开后所得的身份。有条件时为短期租约保留经验证的只读文件句柄，避免仅凭数字 ID 误认已被替换的文件。身份只存在 Rust 进程内，并随原选区进入已提交调用。已有其他来源继续投影元数据。
2. 在 invocation 将这个对象签发为本次文档或后台调用的文件租约。使用前按句柄查租约并复验当前执行资格。阻塞工作由现有 worker 管理，打开后在同一个文件描述符上验证身份，再 stat 或读取。恢复和独立开窗只从仍有效的原对象重新签发，SDK 收到新投影。旧 token 无法用于新 epoch。资格缺失时允许粘贴作为业务降级。
3. 实现 stat / readText 及 SDK 类型。复用现有响应分块，权限撤销或文档取消时释放传输和文件资源，底层 I/O 结束后释放 worker 配额。对会卡住的 I/O 用可控假平台测取消。完成条件是只读一次当前授权文件，并且更换路径内容、切换窗口、撤销授权不会递送旧数据。
4. 补齐普通对象、同名文件、同路径替换、伪造 token、目录、链接、过期 epoch、空文件、有 NUL、非法 UTF-8、上限与上限加一字节，以及 JSON 转义导致的分块边界。分别跑 macOS / Windows。某平台无法证明身份时只交付元数据，并把该平台的读取与预览标记 Pending。

入口见[选区采集](../crates/anywhere-core/src/application/context_capture.rs#L271)、[对象投影](../crates/anywhere-core/src/application/web_plugins/projection.rs)、[文档绑定](../crates/anywhere-core/src/application/web_plugins/invocation.rs)、[分块](../crates/anywhere-core/src/application/web_plugins/transfer.rs)及[平台 traits](../crates/anywhere-platform/src/traits.rs)。

```sh
cargo test -p anywhere-platform --locked
cargo test -p anywhere-core --lib --locked application::web_plugins
scripts/check-web-sdk.sh
```

## 09 把预览接到原生窗口

1. 在 broker 复用 08 对象身份与租约，用单独的 `preview.open` grant。用内部事件传递预览资源和一次性请求身份。JS 长度受限的 JSON 帧只收到成功或错误。窗口控制器用同一 epoch 和绑定检查消费请求。
2. 调用现有 PreviewStore 及系统预览适配。如果平台预览接收路径，使用由已验证打开文件生成的宿主私有快照，并沿用预览额度和租约。清理时关闭面板后释放快照。对超过预算、平台未安装预览程序或预览无法受理，给出可区分的错误，插件可返回自身页面。
3. 覆盖主窗与独立窗分别预览、选区变化、快速切换、旧回调迟到、关闭和卸载。分别留存 macOS Quick Look / Windows QuickLook 实物结果。完成条件是只有当前获准对象进入预览，旧请求无法抢焦点或关闭当前面板，路径始终留在宿主内部。

入口见[PreviewStore](../crates/anywhere-core/src/features/preview.rs)、[文件预览用例](../crates/anywhere-core/src/application/file_preview.rs)、[GPUI 预览](../apps/gpui/src/launcher/view/controllers/preview.rs)和[原生预览契约](native-preview.md)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins
scripts/check-gpui.sh
```

## 10 统一插件安装与升级审查

1. 先扩展现有 Web Package Review 和唯一的审查入口。展示包 ID、版本、摘要、命令数及前 8 条命令、后台活动、增删权限和范围变化。权限对比按标准化范围计算。审批只使用该次 inspect 捕获的包。已有卡片时保留其 token，新的导入请求提示先完成或取消当前审查。批准成功后可对确实支持空输入的 Web 命令提供“立即打开”。
2. 依次接选择器、设置页拖放、原生对象动作。对象动作从 Finder / Explorer 或 File Search 的当前 `core.file` 候选读取已复验的原生路径，识别 `.awplugin` 后只打开审查。文件过滤、路径保真、取消和失败提示统一。文件名扩展只作入口过滤，归档仍由 Rust 验证。
3. macOS 在 Info.plist 注册文档类型，接入实际支持 Finder 文档的原生打开事件。冷启动事件先缓存到 runtime 就绪，热启动复用同一入口。Windows 便携 ZIP 用系统“打开方式”关联，启动入口在判定单实例前解析本地路径。已有主实例时通过有界、当前用户的本机交接传给主实例，随后只触发 inspect。重复打开沿用同一审查状态，无须重复安装。只接收一个本地包，拒绝 URL 和额外命令参数。
4. 分别走每条入口的取消、拒绝、成功、旧 token、审查中再打开、损坏包及路径含中文和空格。冷、热启动都必须到达批准前审查。分别覆盖文件被替换和同摘要重复导入。完成条件是四条入口都显示同一不可变摘要；系统双击的效果在两端实测，批准前没有发布插件 Action。

入口见[审查 UI](../apps/gpui/src/launcher/view/pages/web_packages.rs)、[Window 导航](../apps/gpui/src/launcher/view/controllers/navigation.rs)、[包审查存储](../crates/anywhere-core/src/application/web_plugins/packages.rs)、[对象动作](../crates/anywhere-core/src/application/object_actions.rs)、[App 入口](../apps/gpui/src/main.rs)、[macOS Info.plist](../apps/gpui/macos/Info.plist)和 [Windows 单实例](../apps/gpui/src/platform/windows.rs#L60)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins::package_tests
scripts/check-gpui.sh
```

## 11 打包验收与交付

1. 在维护者的打包脚本用 `npm pack --json` 获取实际 SDK 文件名，生成 SHA-256 文件并检查包内容。CI 将 SDK 和校验和放入 Actions 产物，发行说明明确 Release 的资产上传步骤、版本对应关系和下载地址。方案执行到发布候选即可验证本地或 CI 产物，实际发布按现有发行流程进行。
2. 从最终 SDK 归档，在仓库外用同一个归档创建 Web、TS、后台、混合项目。对浏览器预览、CI validate、pack 和真实 App 导入记录结果。若 Release 已发布，再从实际下载 URL 重做同样检查。验证 API 1 旧包 / 新宿主、API 1 新包 / 旧宿主、API 2 新包 / 新宿主及 API 2 新包 / 旧宿主拒绝四格。
3. 用同一批 Text Pad、JSON Workspace、工单跳转和单文件 CSV 夹具验证 macOS 与 Windows。记录 App 构建身份、SDK 版本、实际包摘要、系统 / WebView 版本和每项结果。使用独立数据目录。对物理 IME、焦点、Quick Look、系统双击冷启动及热启动逐项留证据。缺平台或依赖时按项保留 Pending。
4. 只把已交付的接口与支持范围同步进插件开发、架构、发布、SDK README 及两端验收文档。把新权限、示例、生成检查和 SDK 资产列入常规门禁。按任务编号汇报结果，基线失败须给出可复验的命令、首个错误及影响范围。

入口见[发行流程](releasing.md)、[macOS CI](../.github/workflows/macos.yml)、[跨平台 CI](../.github/workflows/portable.yml)、[macOS 桌验](test-acceptance-macos.md)及[Windows 桌验](test-acceptance-windows.md#webview2-与插件)。

```sh
python3 scripts/check-docs.py
scripts/check-web-sdk.sh
scripts/check-web-examples.sh
scripts/check-gpui.sh
scripts/check.sh
git diff --check
```

### 逐项验收矩阵

对于同一组夹具，自动化跑完可验证的断言，桌验另填 macOS 和 Windows 两列。按以下顺序检查，前一行失败时记录缺口和复现操作，能独立验证的后续行继续执行。

| 夹具 | 操作 | 预期 |
| --- | --- | --- |
| API 1 Text Pad | 原包导入新 Host。再从新 CLI 生成 API 1 基础包，导入旧 Host | 两个方向均能安装和运行。编辑器字段留在源码，产物里没有 `$schema` |
| API 2 JSON Workspace | 新 Host 中搜索中英文和拼音，打开后使用聚焦、计算、复制与粘贴快捷键。旧 Host 尝试导入 | 新 Host 命中相同业务 ID，实际键位与键帽一致，IME 组词时无误触。旧 Host 在发布插件动作前拒绝 |
| 工单跳转 | 选择合成文本 `PROJ-123`，通过命令和快捷键执行。分别取消、确认和尝试未声明站点 | 无对象时隐藏依赖 Context 的命令。取消保持原窗口。确认只交接一次预期 HTTPS URL。未声明站点不调用平台打开 |
| 单文件 CSV | 在文件管理器选中 CSV，通过插件读取和预览。再使用同名文件、原路径替换及窗口重开 | 文件身份相符才得到正文或预览。撤销使迟到结果无效。独立窗口的返回只关闭该窗，系统预览释放临时资源 |
| 安装与更新 | 依次走四种入口，取消一轮，再批准 A → B → B → 回滚 A。系统打开分别在冷启动、已运行时执行 | 批准前无注册。审查显示正确摘要、命令和权限。重复导入不丢回滚点，拒绝或取消后既有安装仍可用 |

完成条件是候选 App、SDK 和示例包均可从同一提交重建，兼容矩阵、自动化和两端必测项都有可查证结果。缺少实机时可交接已通过的工程阶段，整阶段的桌面验收仍为 Pending。

每项交接可使用一行记录。至少填写「编号 / 受测提交 / 局部命令与结果 / macOS 桌验 / Windows 桌验 / 仍待处理的文件和复现」。包含新增能力的任务同时写明 SDK 版本和使用的包摘要。解包开发模式、自定义宿主动作菜单和动态搜索的产品讨论保留在后续规划。
