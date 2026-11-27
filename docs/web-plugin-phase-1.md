# 第三方 Web 插件 Phase 1 实施手册

配套[架构审查与定案](web-plugin-evolution.md)。以 `964dd1e1` 为审查基线。本页给出实施锁定契约、任务顺序与验收标准，全部 11 项实施任务目前为 Pending。当前已落地行为见[插件开发指南](plugin-development.md)和[Web 插件架构](web-plugin-design.md)。

**零历史兼容原则**：当前仅有仓库内参考包（`json-workspace`、`web-text`、`background-context`），本阶段直接收口到单一契约（`apiVersion: "2"`、`protocol: 2`、全线 `camelCase`、零依赖 Node 22 CLI），不维护 API 1 / Python 打包器 / 旧 SDK 别名双轨兼容。

完成后的两条用户路径为「取得 SDK → 创建项目 → 浏览器/后台沙箱验证 → 校验与打包 → 在 Anywhere 审查并试用」和「对 Launcher 中的对象执行插件 Action → 在宿主授权下打开站点、读取选中文件或原生预览」。JSON Workspace 同步验证双平台快捷键与中英文命令元数据。

## 如何接手

1. 从仓库根运行 `git status --short --branch`、`git log -1 --oneline` 和 `git diff --stat`。读 [AGENTS.md](../AGENTS.md)、[开发与构建](development.md)和本页。已存在改动先辨认归属，保留用户工作。
2. 按编号执行。每次只取一个任务，先打开该节链接对应的入口和测试。所有路径和命令均从仓库根解释，Windows 使用对应 PowerShell 环境。
3. 每项依次做「现状验证 → 最小变更 → 生成物同步 → 局部测试 → 差异检查」。并发与时序测试必须使用可控阻塞点与测试夹具，禁止依赖睡眠。
4. 在交接中记录任务编号、受测提交、实际改动、命令及结果、缺少的桌验环境。使用 Pending、已实现、自动化通过、桌验通过四种状态；编译或 Mock 通过时桌验栏仍留 Pending。

可直接把下面提示交给接手模型并替换任务编号（首次用 `01`）：

> 阅读 AGENTS.md 和 docs/web-plugin-phase-1.md。执行任务 01，先验证前置条件，再按该节编号修改。以本手册的版本、三通道交互和权限契约为准，使用该节入口与检查命令，最后汇报改动文件、实际结果与未完成项。

| 任务 | 产出 | 前置 |
| --- | --- | --- |
| [01](#01-记录基线与保留回滚) | 基线、同摘要重复导入与停用启用、损坏 fail-closed | 仓库工具链 |
| [02](#02-收口-api-2-与单一生成契约) | `apiVersion: "2"`、`protocol: 2`、`MethodSpec`、元数据与仓内示例迁移 | 01 |
| [03](#03-交付零依赖-node-22-cli) | 统一 `anywhere-plugin` CLI、重复键扫描、确定性 ZIP、退役 `package.py` | 02 |
| [04](#04-交付双-mock-与项目模板) | Web Mock、裸 VM 后台 Mock、4 种模板、精简 vendor 与 Agent Skill | 03 |
| [05](#05-迁移跨平台快捷键与示例) | 平台元数据、同源键帽与监听、`json-workspace` 迁移 | 02（Mock 验收依赖 04） |
| [06](#06-后台生命周期完成语义与诊断) | 拆分 `RuntimeLifecycle`、返回值与 Usage、结构化错误与诊断环缓冲 | 02 |
| [07](#07-三通道宿主窗口命令与外链) | `WebDispatchOutcome`、`HostInteraction`、WebView 遮挡处理、外链确认与交接尾声 | 06 |
| [08](#08-选中文件的身份与读权限) | `depth == 0` 单文件选区、双平台原生身份、句柄租约、`stat`/`readText` | 02、06 |
| [09](#09-把预览接到原生窗口) | 共享 `PreviewStore`、16 MiB 私有快照、`WebDocument` 预览与焦点归还 | 07、08 |
| [10](#10-统一插件安装与升级审查) | 四入口统一审查、防僵尸锁、风险组合提示、macOS UTI 与 Windows SID 管道 | 01、02 |
| [11](#11-打包验收与交付) | SDK 产物与校验和、`ANYWHERE_DATA_DIR` 验证、双平台验收矩阵与文档同步 | 前序任务 |

## 实施时共用的契约

### 版本、序列化、身份与额度

- **单一当前版本。** Manifest 固定 `schemaVersion: 1`、`apiVersion: "2"`，每包权限上限为 `16`；导入旧 `apiVersion: "1"` 包直接拒绝并返回带重建提示的明确错误。桥协议升级为 `protocol: 2`，全线字段统一 `camelCase`（修正 `requestId`、`pluginId`、`commandId`、`typeId`、`maxBytes`、`modifiedMs`、`closeWindow`）。SDK 移除 `plugin.host`、`context.getCurrentApp` 历史别名。
- **编辑器、归档与防重复键。** 源 Manifest 允许顶层 `$schema`，`anywhere-plugin pack` 剥离 `$schema` 并以 UTF-8 + LF + 末尾换行写入归档。Rust 仅读取归档字节并直接反序列化至 `deny_unknown_fields` 强类型 DTO（严禁先解析为 `serde_json::Value`），为 `localizedTitles` 等 `BTreeMap` 接入自定义 `unique_map` 拒绝重复键；Node CLI 内置零依赖 JSON 重复键扫描器并与 Rust 共用夹具。
- **单源 `MethodSpec` 与三通道模型。** `anywhere-protocol` 维护封闭的 `MethodSpec` 表（方法名、允许角色、所需权限、超时档位、参数与返回 schema），统一生成 `web-plugin-bridge.schema.json` 与 `contract.ts`（含 `Calls`、`CallResults`、`methodTimeouts`）。宿主调用拆为三条通道：数据 RPC（5s，`web_rpc_workers`）、单向宿主意图（`WebDispatchOutcome`）、双向宿主交互（`HostInteraction` + `oneshot`，等待前释放 worker 许可，同文档/窗口确认互斥，第二个并发确认返 `BUSY`）。
- **身份与额度。** 稳定命令 ID `web:<package-id>:<command-id>` 承载绑定与 Usage；每次执行复验安装、摘要、会话、`document_epoch` 与 `requestId`。按 UTF-8 字节计费：`readText` 上限 1 MiB（原文件在上限内但 JSON 编码超桥传输上限时返 `TOO_LARGE`）；原生预览私有快照硬上限 16 MiB。文件路径、URL 参数、用户选区正文与原始异常文本均不写入默认日志。

### 能力和运行角色

`WebInvocation.capabilities`（`urlOpen`、`resourceRead`、`previewOpen`、`windowControl`）与 `WebResource.{readable, previewable}` 仅作非权威 UI 提示；所有调用由 Rust broker 按角色、安装 grant 与当前代次逐次仲裁。SDK 拆出 WebView（`definePlugin`）、显式后台 Action（`anywhere-background`，无 `window`/`preview`）与只读 resolver（`defineResolver`）三套类型门面。

| 调用 | 通道 / 超时 | Web 页 | 显式后台 Action | 自动 resolver | 成功结果 |
| --- | --- | --- | --- | --- | --- |
| `window.back` / `window.close` | 单向 `WebDispatchOutcome` / 5s | 当前文档 | 拒绝（用返回值控制） | 拒绝 | 宿主已受理窗口动作 |
| `url.open` | 双向 `HostInteraction` / 30s | `url.open` grant + 确认 | 同左 | 拒绝 | 系统已受理外链交接 |
| `resources.stat` / `resources.readText` | 数据 RPC / 5s | `resources.read` + 单文件句柄 | 同左 | 拒绝 | 校验身份后的元数据 / 文本 |
| `preview.open` | 双向 `HostInteraction` / 5s | 独立 `preview.open` + 单文件句柄 | 拒绝 | 拒绝 | 原生预览已受理 |

- **外链。** 权限为 `{ "name": "url.open", "origins": ["https://issues.example.com"] }`：最多 16 个精确 `http`/`https` origin（拒绝尾部斜杠、路径、查询、片段、userinfo、通配子域、空数组），或单独 `["*"]`（禁止混用）。运行时 URL ≤ 8 KiB，拒绝 userinfo / 无主机 / 非 `http(s)`。确认卡高亮 Rust 规范化后的 `scheme://host[:port]`（非 ASCII 域名显式展示 Punycode）与路径查询，支持 `Enter` 确认、`Esc` 取消；确认后直接打开同一份 `url.as_str()`，V0.15 无“记住站点”开关。
- **资源。** 严格限定启动时来自 Finder / Explorer 的原始单个普通本地文件（`source == FinderSelection && depth == 0 && source_context_id.is_none()`），排除多选展开项、目录、符号链接与重解析点。声明 `resources.read` 或 `preview.open` 须同时声明 `context.read`（含 `core.file`）且命令 `accepts` 接受文件 Context。`stat` 返回 `handle`、`name`、`kind` 及可选 `bytes`、`modifiedMs`（超 JS 安全整数时省略）；`readText` 的 `maxBytes` 为 `1..=1 MiB`（缺省 1 MiB），以上限 +1 字节判超限，允许空文件，拒绝非法 UTF-8 与含 NUL 内容。
- **错误。** 角色或权限不符返 `PERMISSION_DENIED`；参数非法返 `INVALID_REQUEST`；并发确认返 `BUSY`；句柄失效、对象替换或消失返 `EXPIRED`；平台不支持返 `UNSUPPORTED_METHOD`；用户取消确认返 `USER_CANCELLED`；文档失活沿用 `INACTIVE` / `UNTRUSTED_DOCUMENT`。SDK 对宿主副作用永不自动重试。
- **后台反馈。** 显式后台 Action 仅接受 `null` / `undefined` 或 `{ message?, closeWindow? }`（`deny_unknown_fields`）。`message` ≤ 512 UTF-8 字节、纯文本渲染。因 V0.15 无 OS Toast：**当 `message` 非空且未显式指定 `closeWindow` 时默认 `closeWindow = false`**（留在原搜索页展示提示）；无 `message` 时默认 `closeWindow = true`；同时传入非空 `message` 与 `closeWindow: true` 由 CLI / Mock 发告警。失败与取消始终保持窗口打开。

## 01 记录基线与保留回滚

1. 运行下方检查，区分本地环境问题与基线差异，仅在对应改动范围内修正格式；把仓内打包源文本换行固定为 LF 并同步内置包。
2. 在 `package_tests` 中加入 `A → B → 重复导入 B → 重开 store → 回滚 A`，以及同摘要已停用包和缓存资产损坏用例。调整 `approve` 顺序，先调用 `repository.stage(&package)` 验证完整性：
   - **缓存资产损坏**：保持现有 **fail-closed** 契约（`persistence/web_packages.rs`），拒绝原地覆盖，要求用户卸载或先回滚再重装；
   - **完好且已启用**：做幂等 no-op，保留 `previous`、安装 `source` 与在用窗口，不触发无意义目录重发布；
   - **完好但已停用**：审查卡明确提示“该版本已安装且当前处于停用状态”，主按钮为“批准并启用”；批准后恢复启用并保留 `previous` 与 `source`，取消则保持停用。
3. 完成条件：重复导入不丢失回滚点、不触发无效目录发布，损坏缓存维持 fail-closed，重启后状态一致。

入口见[包审批](../crates/anywhere-core/src/application/web_plugins/packages.rs)、[包缓存](../crates/anywhere-core/src/persistence/web_packages.rs)、[包测试](../crates/anywhere-core/src/application/web_plugins/package_tests.rs)和[参考包生成器](../scripts/build-web-examples.py)。

```sh
cargo fmt --all -- --check
cargo test -p anywhere-core --lib --locked application::web_plugins::package_tests
scripts/check-web-examples.sh
```

## 02 收口 API 2 与单一生成契约

1. 在 `anywhere-protocol` 与 `anywhere-plugin` 直接将 Manifest 升级为 `schemaVersion: 1`、`apiVersion: "2"`（权限上限 `16`），导入 `apiVersion: "1"` 旧包直接返回带重建指引的明确错误。桥协议升级为 `protocol: 2`，全线字段统一 `camelCase`（修正 `requestId`、`pluginId`、`commandId`、`typeId`、`maxBytes`、`modifiedMs`、`closeWindow`）。
2. 在 `anywhere-protocol` 建立唯一 `MethodSpec` 表（方法、允许角色、所需权限、超时档位、参数与返回 schema），同步驱动 `generate_web_plugin_schema` 与 `generate_web_sdk_contract` 产出归档严格 schema、允许顶层 `$schema` 的编辑器 schema、`web-plugin-bridge.schema.json` 与 `contract.ts`（含 `Calls`、`CallResults`、`methodTimeouts`）。为 `WebInvocation` 增加只读 `capabilities`、为 `WebResource` 增加 `{ readable, previewable }` 提示。
3. Rust 仅直反序列化强类型 `deny_unknown_fields` DTO（禁止先转 `serde_json::Value`），为 `localizedTitles` 等 `BTreeMap` 接入自定义 `unique_map` 拒绝重复 JSON 键。命令元数据加入 `localizedTitles`（Phase 1 渲染 `en`/`zh`，允许合法短 BCP-47 标签保留兼容，≤ 200 字节无控制字符，回退 `title`）、`subtitle`（≤ 200 字节，回退 `manifest.name`）与协议层第三方图标白名单 `icon`（`catalog.rs` 对未知图标显式回退 `"code"`，不落入 `presentation.rs` 的默认命令图标）。
4. 同步更新 GPUI 中英文权限说明与生成物；移除 SDK 历史别名 `plugin.host` 与 `context.getCurrentApp`；把仓内三个示例包（`json-workspace`、`web-text`、`background-context`）一步迁至 API 2 / protocol 2。完成条件：生成检查通过，API 2 夹具双端互通且标题翻译不改变 Action ID。

入口见[Manifest DTO](../crates/anywhere-protocol/src/web_plugin.rs)、[桥协议](../crates/anywhere-protocol/src/web_bridge.rs)、[语义校验](../crates/anywhere-plugin/src/web/manifest.rs)、[目录注册](../crates/anywhere-core/src/application/web_plugins/catalog.rs)、[图标映射](../apps/gpui/src/launcher/presentation.rs#L416)、[生成说明](development.md#生成物)及[权限文案映射](../apps/gpui/src/host/runtime_support.rs)。

```sh
cargo run -p anywhere-protocol --features web-schema --example generate_web_plugin_schema --locked
cargo run -p anywhere-protocol --features web-schema --example generate_web_sdk_contract --locked
cargo test -p anywhere-protocol -p anywhere-plugin --locked
cargo test -p anywhere-core --lib --locked application::web_plugins
```

将上述两个生成命令末尾追加 `-- --check` 再运行；修改权限文案后运行 `python3 scripts/generate-gpui-localizations.py` 及 `--check`。

## 03 交付零依赖 Node 22 CLI

1. 将 SDK 的 `bin` 指向统一 Node 22 CLI `anywhere-plugin`：`init [--ts | --background | --hybrid]`、`validate [dir] [--built]`、`pack [dir] --output <file.awplugin>`、`test [dir]`。**正式退役 `packages/plugin-sdk/tools/package.py`**，更新仓内构建脚本改用 `anywhere-plugin pack`。
2. `validate` 源码阶段内置零依赖 JSON 重复键扫描器（与 Rust 共用重复键夹具），检查字段位置、类型、权限组合、未知图标告警及 `{ message, closeWindow: true }` 告警；`--built` 额外校验构建产物入口、图标与资源预算。`pack` 先执行构建校验，剥离 `$schema` 并以 UTF-8 + LF + 末尾换行写出归档 Manifest，落盘临时文件成功后再原子替换目标 `.awplugin`。
3. ZIP 写入器仅依赖 `node:zlib`（`deflateRawSync` + `crc32`），零第三方依赖：固定 UTF-8 路径与标志 `0x0800`、**禁止设置 Bit 3 Data Descriptor**（对齐 `archive.rs` 头一致性校验）、**禁止写扩展字段**（避免 `preflight.rs` 拒绝 `0x7075`）、固定字节排序、固定时间戳 `(2026,1,1,0,0,0)` 与模式 `0o100644`，压缩比超 200 倍自动回落 Stored，严守 entry 数、单资源、压缩与解压上限，拒绝符号链接、Unicode / 大小写冲突与 Windows 保留名。
4. 用同一批合法 / 非法夹具对照 Node CLI 与 Rust 检查器，验证固定输入跨平台 ZIP 哈希一致、未构建提示、缺资源报错与二次打包重试。完成条件：仓库外仅用 Node 22 即可生成、构建并打出 Rust 可导入的确定性 `.awplugin`。

入口见[SDK 包清单](../packages/plugin-sdk/package.json)、[现有创建器](../packages/plugin-sdk/tools/create.mjs)、[ZIP 边界](../crates/anywhere-plugin/src/web/preflight.rs)、[归档校验](../crates/anywhere-plugin/src/web/archive.rs)和[仓库外测试脚本](../scripts/check-plugin-starter.sh)。

```sh
scripts/check-web-sdk.sh
scripts/check-plugin-starter.sh
```

## 04 交付双 Mock 与项目模板

1. 交付 `@anywhere/sdk/mock` 浏览器与测试导出，复用 SDK transport 提供启动、订阅、内存 KV、合成剪贴板/文件、隐藏恢复、撤销、超限、外链确认与取消模拟；调试面板支持切换平台、语言、主题与故障场景，正式 bundle 零注入。
2. 交付 `@anywhere/sdk/mock/background` 裸 `node:vm` ES2023 沙箱（剥离 QuickJS 不存在的 `URL`、`TextEncoder`、`fetch`、`setTimeout`、`console`），由 `anywhere-plugin test` 驱动校验缺失导出、resolver 越权调用与后台返回值契约。
3. SDK 拆出三套无历史别名的清晰门面：`definePlugin`（WebView）、`anywhere-background`（显式后台 Action，不含 `window`/`preview`）与只读 `defineResolver`（编译期仅暴露只读方法，Rust 运行时继续仲裁）。提供 Web、`--ts`、`--background`、`--hybrid` 四种 API 2 模板及 `npm run dev`。
4. 精简 `vendor/plugin-sdk/`（仅包含 `dist/`、`styles/`、`schemas/`、`tools/`、`package.json`、`README.md`、`LICENSE`，不复制 `templates/`）；单源维护插件开发 Skill 并在 `init` 时投影至 `.agents/skills/anywhere-plugin-dev/`。完成条件：四种模板均可在仓库外独立运行 `dev`、`test`、`validate`、`pack` 并通过 Rust 导入检查。

入口见[SDK transport](../packages/plugin-sdk/src/client.ts)、[SDK 测试](../packages/plugin-sdk/test/client.test.mjs)、[QuickJS 入口](../packages/plugin-sdk/src/background.ts)、[旧模板](../packages/plugin-sdk/templates/minimal)和[混合示例](../plugins/examples/json-workspace)。

```sh
scripts/check-web-sdk.sh
scripts/check-plugin-starter.sh
```

## 05 迁移跨平台快捷键与示例

1. 与语言元数据一同向 WebView 注入 Host platform；SDK 优先读宿主，在浏览器优先读 Mock 值，缺省按浏览器平台推断。新增 `environment.formatShortcut` 与 `environment.matchesShortcut`，Mock 切换平台即时重绘键帽并切换监听。
2. 匹配规则统一：`Mod` 在 macOS 映射 Meta、在 Windows 映射 Ctrl，显式支持 Ctrl / Alt / Shift；校验精确修饰键集合，忽略 IME 组词、AltGr、已阻止与长按重复事件；保留 `json-workspace` 的 Ctrl+Tab、中文组词与原生复制粘贴。
3. 迁移 `json-workspace` 的键帽、帮助文案、监听与中英文 `localizedTitles`，同步仓内三个示例包归档。完成条件：双平台同源规格测试通过，切换语言后命令 ID、快捷键与 Usage 延续；物理 IME 与焦点列为两端桌验项。

入口见[平台键盘辅助](../apps/gpui/src/platform/keyboard.rs)、[Web 引导脚本](../apps/gpui/src/web_tool.rs)、[SDK 环境](../packages/plugin-sdk/src/environment.ts)、[JSON Workspace 监听](../plugins/examples/json-workspace/src/main.jsx#L266)和[帮助文案](../plugins/examples/json-workspace/src/i18n.mjs)。

```sh
scripts/check-web-sdk.sh
npm --prefix plugins/examples/json-workspace test
python3 scripts/build-web-examples.py --sync-builtin
scripts/check-web-examples.sh
```

## 06 后台生命周期完成语义与诊断

1. **拆分 `Invocation` 生命周期**：移除 `background_action.rs` 的伪造 `lifecycle.ready` 与提前 Usage；将 `Invocation` 拆为 `RuntimeLifecycle::Webview { ready, bound_at }`（保留 10s ready 超时）与 `RuntimeLifecycle::Background { state: Running | AwaitingInteraction | HandoffCompleted }`（无 10s ready 清理，避免 30s `url.open` 确认被 `prune()` 误杀）。
2. **返回值与 Usage**：严格按 `deny_unknown_fields` 解析后台返回值（`null`/`undefined` 或 `{ message?, closeWindow? }`，纯文本 ≤ 512 字节）；`message` 非空且省略 `closeWindow` 时默认 `closeWindow = false` 留在搜索页展示，无 `message` 默认 `true`。`execute.rs` 在合法返回后记一次 Usage（写入失败走既有 `warning` + `close_window = false`）；解析失败不重跑已完成的宿主副作用。普通 Web 页面保持有效 ready 时记一次 Usage。
3. **结构化诊断与环缓冲**：在 `background_runtime.rs` 通过 `ctx.catch()` 与中断原因原子标志细分 `MISSING_EXPORT`、`SCRIPT_EXCEPTION`、`CANCELLED`、`CPU_BUDGET_EXCEEDED`、`MEMORY_LIMIT`、`OUTPUT_TOO_LARGE`、`INVALID_RETURN`。增加内存有界（每包最近 10 条）、**零用户正文内容**的诊断环缓冲，同时记录后台 Action 与静默 `background_provider.rs` resolver 失败并在设置 → Web 工具页展示；仅在开发 / `ANYWHERE_DATA_DIR` / debug 模式下附加 ≤ 1 KiB 脱敏堆栈。
4. 完成条件：从真实 Launcher Action 入口跑通成功留页提示、成功关闭、缺失导出、脚本异常、超预算、取消与 Usage 写失败；失败与解析错误不记 Usage，陈旧页面不接收反馈。

入口见[后台命令](../crates/anywhere-core/src/application/web_plugins/background_action.rs)、[后台解析器](../crates/anywhere-core/src/application/web_plugins/background_provider.rs)、[QuickJS](../crates/anywhere-core/src/application/web_plugins/background_runtime.rs)、[调用生命周期](../crates/anywhere-core/src/application/web_plugins/invocation.rs)、[执行用例](../crates/anywhere-core/src/application/execute.rs)和[GPUI 结果](../apps/gpui/src/launcher/view/mod.rs#L2056)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins
scripts/check-web-sdk.sh
scripts/check-gpui.sh
```

## 07 三通道宿主窗口命令与外链

1. **单向宿主意图（`WebDispatchOutcome`）**：将 `dispatch_web_frame` 返回值由原始 JSON 字符串升级为结构化 `{ reply_frame, host_action: Option<WebHostAction> }`，绑定 `(handle, document_epoch, request_id, window_generation)`，经 frontend runtime 交给 GPUI。主窗口 `back` 返回上页、`close` 清理后隐藏；独立工具窗两者均关闭该工具窗。JS 无法用回复字符串伪造窗口控制。
2. **双向宿主交互（`HostInteraction`）与 WebView 遮挡处理**：接入 `url.open` grant、审查文案、8 KiB 参数与 origin 校验。通过窗口/会话级异步通道向 GPUI 发起外链确认并以 `oneshot` 回传：
   - 等待交互前**释放 `web_rpc_workers` 信号量许可**，并在同文档/窗口上加单确认互斥锁（第二个并发确认返 `BUSY`）；
   - `rpc_state.rs` 与 SDK `client.ts` 按 `MethodSpec` 对齐方法级超时（数据 RPC 5s、外链确认 30s），用户确认等待不计入 QuickJS CPU 预算；
   - **WebView Z-Order / Airspace 修复**：在 WebView 上弹出 GPUI 确认卡时调用原生 `view.set_visible(false)`，**严禁**派发 `visibility_script(false)`（否则 `web-bootstrap.js` 会立刻以 `INACTIVE` 中止确认中的 RPC），确认或取消后恢复 `set_visible(true)`；
   - 确认卡高亮规范化 `scheme://host[:port]`（非 ASCII 域名显式展示 Punycode）与路径/查询（超长截断显示查询字节数并可展开），支持 `Enter` 确认、`Esc` 取消；确认后复验 grant 与代次并直接打开同一份 `url.as_str()`。
3. **后台外链交接尾声（Handoff Drain）**：系统受理外链后将后台 invocation 标为 `HandoffCompleted`；默认浏览器抢焦触发 `suspend()` 时不硬杀 QuickJS 剩余 250ms CPU 尾声，仅拒绝新宿主调用（返 `INACTIVE`），让脚本正常返回并记录一次 Usage；显式取消、停用、卸载或目录发布仍立即硬取消。
4. 完成条件：可控迟到回复测试覆盖 A 切 B 后 A 的 `back`/`close`/确认全部失效；覆盖主窗/独立窗、并发 `BUSY`、越界站点、用户取消、后台交接失焦及 resolver 拒绝。

入口见[broker](../crates/anywhere-core/src/application/web_plugins/broker.rs)、[RPC 状态](../crates/anywhere-core/src/application/web_plugins/rpc_state.rs)、[RPC 调度](../crates/anywhere-core/src/application/web_plugins/rpc_dispatch.rs)、[frontend runtime](../crates/anywhere-core/src/application/frontend_runtime/mod.rs#L408)、[GPUI Web 事件](../apps/gpui/src/host/runtime/web.rs#L95)、[回复控制器](../apps/gpui/src/launcher/view/controllers/web.rs#L331)、[前端挂起](../resources/web-bootstrap.js#L142)和[平台打开接口](../crates/anywhere-platform/src/traits.rs#L279)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins::broker_tests
scripts/check-web-sdk.sh
scripts/check-gpui.sh
```

## 08 选中文件的身份与读权限

1. **严格单文件选区边界**：仅当 `source == FinderSelection && depth == 0 && source_context_id.is_none()` 时在可信选区入口捕获原生文件身份，排除多选展开子项，移除 `projection.rs` 跟随符号链接的 `path.is_file()`。File Search `⌘K` 留待后续评估。
2. **双平台原生身份校验**：
   - **macOS**：校验祖先路径无符号链接，`O_RDONLY | O_NOFOLLOW | O_NONBLOCK` 打开后核验 `S_ISREG`、拒绝 `SF_DATALESS` iCloud 占位，捕获 `(st_dev, st_ino)` 与短期只读 FD（每个活动 invocation 至多 1 个）；
   - **Windows**：校验祖先无符号链接/联接点，使用 `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` + `FILE_FLAG_OPEN_REPARSE_POINT` 打开并按 **reparse tag** 过滤（拒绝 `SYMLINK`、`MOUNT_POINT`、`APPEXECLINK` 与未知标签；对已落盘 OneDrive `IO_REPARSE_TAG_CLOUD*` 执行二次复验打开，拒绝未下载占位），通过 `GetFileInformationByHandleEx(FileIdInfo)` 获取 128 位文件 ID；对无稳定文件 ID 的卷（FAT32/exFAT/SMB）安全降级为仅元数据。
3. **句柄寿命与分块读取**：15 秒内同 WebView `resume` **保持原句柄字符串稳定**（由新 `document_epoch` 与每次调用的原生身份复验把关，避免已启动前端持有失效句柄）；仅新建 `tool_window` 重签新句柄（新 WebView 重新执行 `onLaunch`）。实现 `resources.stat` 与 `resources.readText`（1 字节至 1 MiB，上限 +1 字节判超限，允许空文件，拒绝非法 UTF-8、NUL 字节与 JSON 转义后超桥上限），取消或撤销时释放传输与 FD。
4. 完成条件：覆盖单文件、多选展开拒绝、同路径替换、伪造/旧 `epoch` 句柄、目录、链接、OneDrive/iCloud 占位、空文件、含 NUL、非法 UTF-8、上限/+1 字节与卡住 I/O 取消；任一平台未验证身份前只暴露元数据并保持该平台 Pending。

入口见[选区采集](../crates/anywhere-core/src/application/context_capture.rs#L271)、[对象投影](../crates/anywhere-core/src/application/web_plugins/projection.rs)、[文档绑定](../crates/anywhere-core/src/application/web_plugins/invocation.rs)、[分块](../crates/anywhere-core/src/application/web_plugins/transfer.rs)及[平台 traits](../crates/anywhere-platform/src/traits.rs)。

```sh
cargo test -p anywhere-platform --locked
cargo test -p anywhere-core --lib --locked application::web_plugins
scripts/check-web-sdk.sh
```

## 09 把预览接到原生窗口

1. **修复独立工具窗 `PreviewStore` 并复用 08 租约**：在 `tool_window.rs` 让独立 Web 工具窗共享父级已配置的 `PreviewStore`（移除空 `Default::default()`）。`preview.open` 使用独立 `preview.open` grant 与通道 3 双向交互（5s 受理超时），通过宿主内部消息传递预览目标，JS 永不接触真实路径。
2. **16 MiB 私有只读快照与安全命名**：从已验证 FD 复制私有只读快照（硬上限 **16 MiB**，超出返 `TOO_LARGE`）至 `<preview-root>/preview-<rand>/<sanitized-name-with-ext>`，保留扩展名供 macOS Quick Look 识别，并剥离 `|`、`\r`、`\n` 及路径分隔符以兼容 Windows QuickLook 窗口标题匹配。
3. **预览目标与焦点归还**：扩展 `controllers/preview.rs` 新增 `PreviewTarget::WebDocument` 与动态 `owner_title`（区分主 Launcher、独立 Web 工具窗与 Shelf）；关闭原生预览后立即释放快照并把焦点归还原 WebView。未安装预览程序或受理失败返回可区分错误。
4. 完成条件：覆盖主窗/独立窗预览、超 16 MiB 拒绝、选区变化、快速切换、旧回调迟到、关闭与卸载清理；两端留存 macOS Quick Look / Windows QuickLook 实物桌验记录。

入口见[PreviewStore](../crates/anywhere-core/src/features/preview.rs)、[独立工具窗](../crates/anywhere-core/src/application/tool_window.rs#L44)、[文件预览用例](../crates/anywhere-core/src/application/file_preview.rs)、[GPUI 预览](../apps/gpui/src/launcher/view/controllers/preview.rs)、[Windows 预览](../apps/gpui/src/platform/windows/preview.rs)和[原生预览契约](native-preview.md)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins
scripts/check-gpui.sh
```

## 10 统一插件安装与升级审查

1. **统一审查卡与防僵尸锁**：扩展 Web Package Review，展示包 ID、版本、摘要、命令数与前 8 条命令、后台活动、权限增删及范围变化，并对高风险权限组合（`resources.read`/`clipboard.readText` + `url.open`、`origins: ["*"]`、明文 `http://`）给出醒目风险提示。审批严格绑定本次 inspect 捕获的摘要。同时间只保留一份待审查卡，收到新包时提示先完成/取消或切换；**隐藏 Launcher 或离开 WebPackages 页自动取消未批准 `pending`**，避免僵尸锁阻塞后续双击。仅对支持空输入的 Web 命令显示“立即打开”。
2. **四入口统一路由**：选择器、设置页拖放、`.awplugin` 文件对象动作（Core 校验 `core.file` 后发出类型化 `HostView`，由 `ApplicationRuntime::inspect_web_package` 接管）与系统双击统一进入同一 inspect / approve 流程；扩展名只作入口过滤，归档完整性一律由 Rust 最终验证。
3. **双平台系统打开交接**：
   - **macOS**：在 `Info.plist` 同时声明 `UTExportedTypeDeclarations`（`dev.anywhere.plugin-package`）与 `CFBundleDocumentTypes`；编码前先核验 `gpui_kit` 的 `on_open_urls` 回调挂载；冷启动事件缓存至 runtime 就绪，热启动复用同一入口。
   - **Windows**：便携 ZIP 通过系统“打开方式”关联 EXE；启动入口在单实例检查前解析本地路径，复用 `windows/preview.rs` 的 `current_user_sid()` 建立当前用户 DACL 命名管道 `\\.\pipe\dev.anywhere.launcher.handoff.<SID>` 交接给主实例，仅接受单一本地包、拒绝 URL 与多余参数，并尊重 `--background` 与 `ANYWHERE_DATA_DIR`。
4. 完成条件：覆盖四入口取消/拒绝/成功、旧 token、审查中再打开、隐藏自动清锁、损坏包及中文/空格路径，以及双平台冷/热启动；批准前零插件 Action 发布。

入口见[审查 UI](../apps/gpui/src/launcher/view/pages/web_packages.rs)、[Window 导航](../apps/gpui/src/launcher/view/controllers/navigation.rs)、[包审查存储](../crates/anywhere-core/src/application/web_plugins/packages.rs)、[对象动作](../crates/anywhere-core/src/application/object_actions.rs)、[App 入口](../apps/gpui/src/main.rs)、[macOS Info.plist](../apps/gpui/macos/Info.plist)和 [Windows 单实例](../apps/gpui/src/platform/windows.rs#L60)。

```sh
cargo test -p anywhere-core --lib --locked application::web_plugins::package_tests
scripts/check-gpui.sh
```

## 11 打包验收与交付

1. 在维护者打包脚本用 `npm pack --json` 获取实际 SDK 归档名，生成 SHA-256 文件并核验包内容（确认已剔除 `templates/` 冗余、不含 `package.py`、包含单源 Skill 与生成 schema）。CI 将 SDK 与校验和放入 Actions 产物，发行说明写明 Release 上传步骤、版本对应与下载地址。
2. 从最终 SDK 归档在仓库外创建 Web、`--ts`、`--background`、`--hybrid` 四个项目，依次验证 `npm run dev`、`anywhere-plugin test`、`validate`、`pack` 与 `ANYWHERE_DATA_DIR` 隔离真实宿主导入；若 Release 已发布则从实际下载 URL 复验一遍。另加一条回归断言：旧 `apiVersion: "1"` 包被新 Host 明确拒绝并给出重建指引。
3. 使用同一批 Text Pad、JSON Workspace、工单跳转与单文件 CSV 夹具在 macOS 与 Windows 执行隔离数据目录验收，记录 App 构建身份、SDK 版本、包摘要、OS / WebView 版本，以及物理 IME、WebView 遮挡确认卡、焦点归还、Quick Look 与系统双击冷/热启动实测结果；缺少平台环境时按项保留 Pending。
4. 同步更新插件开发指南、架构文档、发行文档、SDK README 与两端验收文档，并将新权限、示例、生成检查与 SDK 资产纳入常规门禁。

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

自动化先跑完所有可验证断言，桌验按 macOS 与 Windows 两列留存证据；前一行失败时记录缺口与复现步骤，独立项继续验证。

| 夹具 | 操作 | 预期 |
| --- | --- | --- |
| Text Pad（API 2） | 新 CLI `init` → `validate` → `pack` → 导入运行；另喂入旧 `apiVersion: "1"` 样本 | API 2 包无 `$schema` 正常安装运行，双平台 ZIP 确定性一致；旧 `apiVersion: "1"` 被明确拒绝并提示重建 |
| JSON Workspace（API 2 混合包） | 搜索中英文与拼音，打开后测试格式化/聚焦/复制/粘贴与中文 IME；测试后台复制与诊断环缓冲 | 切换语言命中同一 Action ID 且键帽/监听跨平台一致；IME 组词无误触；后台脚本失败不记 Usage 且在设置页可见结构化诊断 |
| 工单跳转（后台 + Web 外链） | 选中 `PROJ-123` 分别取消、并发触发、确认和访问未声明站点；测试 WebView 遮挡确认卡与后台外链失焦尾声 | 并发确认返 `BUSY`；WebView 隐藏原生层不误杀 RPC；确认只交接一次规范化 URL（IDN 显示 Punycode）；后台交接后失焦仍正常返回并记一次 Usage |
| 单文件 CSV（读取与预览） | Finder/Explorer 单选读取与预览；测试多选展开项、同路径替换、超过 1 MiB / 16 MiB、15s `resume` 与独立窗口 | 仅单文件通过原生身份校验；`resume` 复用稳定句柄且复验身份；独立窗口共享 `PreviewStore`、关闭预览归还焦点并释放私有快照 |
| 安装与升级 | 依次走四入口、审查中隐藏清锁、批准 A → B → B → 回滚 A、停用包再导入与损坏缓存重装；双平台系统双击冷/热启动 | 批准前零注册；高风险组合显式告警；重复导入不丢回滚点；停用包走“批准并启用”；损坏缓存 fail-closed；双平台冷/热启动达审查卡 |

完成条件：候选 App、SDK、Node CLI 与仓内三个示例包均从同一提交重建，自动化与双平台必测项均有可查证结果；缺少实机时可交接工程通过状态，桌面验收栏保留 Pending。

每项交接使用单行记录，至少包含「编号 / 受测提交 / 局部命令与结果 / macOS 桌验 / Windows 桌验 / 仍待处理项」，涉及新增能力时附 SDK 版本与包摘要。解包开发模式、自定义宿主动作菜单与动态搜索保留在后续规划。
