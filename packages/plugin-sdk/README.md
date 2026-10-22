# @anywhere/plugin-sdk（开发中）

浏览器侧的最小 Web 插件 SDK，运行时无 npm 依赖，不启动 Node。已接入 macOS 本地 WebView、Host 确认 UI 与安装入口，也连接 Rust 合成测试宿主。当前仍是本地开发预览，完整安全 / 跨平台验收尚未完成。

**第一次开发插件：** 阅读仓库中的 [第三方插件开发指南](../../docs/plugin-development.md)。SDK `.tgz` 自带 `templates/minimal/` 和 `tools/package.py`，可在独立目录使用；当前没有公开 npm 发布版。

导出包只包含 `dist/`、类型、公共 CSS、模板、打包工具与本说明。业务通过 `@anywhere/plugin-sdk` / `@anywhere/plugin-sdk/styles/controls.css` 标准导入，不引用仓库源文件。模板的 `npm run build` 构建网页，`npm run pack` 生成 `.awplugin`；`vendor/` 中保留 SDK 包后，不再依赖原仓库位置。

## 默认入口：只写业务

```sh
npm exec --package=/绝对路径/anywhere-plugin-sdk-0.1.0.tgz -- anywhere-plugin init my-tool
cd my-tool
npm install --ignore-scripts
npm run build
npm run pack
```

创建器复制独立 SDK 快照到 `vendor/plugin-sdk/`，生成普通 HTML / JS 项目并拒绝覆盖现有目录；不依赖原仓库，不要求安装 Flutter / Rust。

```js
import {definePlugin} from '@anywhere/plugin-sdk';
const plugin = definePlugin({
  onLaunch({input}) { /* 只处理本次输入；不在恢复后重复执行 */ },
  // onError(error) 可选：缺省使用共享样式的错误提示。
});
// 在业务按钮中调用，处理取消 / 失败后再提示结果。
await plugin.host.clipboard.writeText('hello');
```

页面渲染后调用。启动就绪、首次输入、隐藏时撤销待处理请求、恢复后的新连接、退出释放均自动完成；`plugin.host` 是稳定门面，旧按钮处理器在恢复后也能使用新连接。`plugin.ready` 在首次启动处理结束后完成，错误通过 `onError` 报告；`plugin.dispose()` 幂等，可在前端框架卸载时调用。`onLaunch(invocation, {signal})` 的 signal 在页面销毁时中止；异步业务自行检查，避免卸载后更新状态。`onDispose` 用来清理业务资源。

背景、默认控件样式自动注入，无需 CSS 导入；模板的 `aw-tool` 提供标准工具栏、可伸展编辑区和状态栏。可使用任意标准 HTML / 前端框架，自定义业务布局；这里没有新增 UI DSL。底层 API 继续兼容，只有进阶集成才需自行管理它们。

## 开发与检查

要求 Node.js 22+；TypeScript 版本在锁文件固定，仅用于构建。

```sh
npm ci --ignore-scripts
npm run check
npm run build
npm test
```

完整 Rust / SDK 互通测试从仓库根执行：

```sh
scripts/check-web-sdk.sh
```

此脚本使用 `web_sdk_conformance` Rust 示例。它只有合成剪贴板与内存 SQLite，不访问真实系统剪贴板、用户文件或网络。单独执行 `npm test` 时，未设置测试宿主路径的 3 项互通测试会明确跳过；完整工程检查必须运行这些测试。

## 插件调用

### 自动匹配输入

命令可在 Manifest 声明 `"accepts": ["core.text"], "inputMatchers": ["json"]`，并申请 `context.read` 的 `core.text` 类型。宿主在用户输入、选中文本或 15 秒内的新剪贴板内容中识别严格 JSON 对象 / 数组，匹配时只展示 Action；执行后通过 `host.invocation.get()` 传入原文。插件无需写识别代码，匹配期间不创建 WebView、不执行 JS。

`json` 自动识别上限 16 KiB、128 层；不匹配标量、注释、尾随逗号或不完整 JSON。超限 / 无匹配时保持原唤醒规则，仍可搜索插件名称打开后手动输入。匹配结果仅在当前识别过程复用，不保存正文。未声明匹配器的包保持原行为；插件停用、卸载或撤销后停止自动匹配。普通搜索关键词仍不会当作插件输入。

匹配器是宿主提供的有限格式规则，不接受插件 JS、正则或自定义 DSL；以后新增格式沿用同一声明入口。修改 Manifest 后需重新构建、导入并核对批准新的包摘要，宿主不会修改已批准安装内容。

当前也可声明 `json_escaped`，该能力在 V0.11 Build 23 接入并延续到 V0.12。它匹配带 / 不带外层引号的转义 JSON 容器，最多解开 4 层；编码原文仍受 16 KiB、解码容器受 128 层限制。它不匹配普通字符串 / 标量或已是正常 JSON 的对象，Invocation 保留编码原文。解码、格式化、复制仍由业务插件处理，Host 识别不执行插件代码。

### 原始 API（进阶集成，自行管理生命周期）

```ts
import { host } from '@anywhere/plugin-sdk';

// 页面先渲染，只有第一次有效 ready 记录一次入口 Usage。
await host.ready();
const invocation = await host.invocation.get();

const app = await host.context.getCurrentApp();
const prefs = await host.storage.get('preferences');
await host.storage.set('preferences', { indent: 2 });
await host.clipboard.writeText('Text to copy');
const clipboard = await host.clipboard.readText();
```

- `context.read` 只允许本次调用中声明的类型，不包含完整 Context Graph；文件只给显示名称与不透明句柄。
- `context.currentApp` 是唤醒前冻结的 ID / 名称，需要单独批准。
- 剪贴板读写分别需要对应权限，还需要 Host 对本次操作的确认。网页不能用 `approved`、`userGesture` 或自报 pluginId 获得资格。SDK Promise 等待 Host 的最终响应，不会自动重试副作用。
- `storage.local` 只能使用当前插件及来源绑定的键空间，不接受 namespace。值是 JSON；`get` 返回值或 `null`。
- 支持 `AbortSignal`。隐藏、重载、返回、撤销时 Host 负责撤销 Rust 文档资格并释放旧客户端；`createHost(transport)` 提供的实例也有 `dispose()`。
- 没有通用 `invoke`、Shell、后台监控、历史剪贴板、任意文件或网络 API。

## 协议与限额

`src/contract.ts` 来自 Rust 的请求 / DTO / 错误 schema 与配额常量，不手改；生成与漂移检查：

```sh
cargo run -p anywhere-protocol --features web-schema --example generate_web_sdk_contract --locked
```

帧 ≤ 64 KiB；8 个进行中请求、16 个排队请求；单文档最多 4,096 个请求 ID。普通调用 5 秒，Host 确认最多 30 秒；首次 ready 最多 10 秒。取消或超时无法撤回已经开始的 OS I/O，错误不承诺可以安全重试。

复制文本原始 UTF-8 ≤ 1 MiB；大文本每块 ≤ 4 KiB 原始字节，编码后 ≤ 32 KiB。结果 JSON 采用单个下载缓冲，编码后总量 ≤ 2 MiB，超过返回 `TOO_LARGE`，不静默截断。缓冲 5 秒过期，后台每秒清理；隐藏 / 销毁立即释放。SDK 按 UTF-8 边界切分，保留中文、表情和 BOM。

KV 每个命名空间的值合计 ≤ 1 MiB、最多 256 个键，单键值 JSON ≤ 64 KiB；调用仍受帧上限约束。配额检查与更新在同一个 SQLite 事务内，失败保留旧值。

Host 通过 `HostTransport` 注入一条只传字符串帧的消息连接。原生来源 / 主 frame / Runtime / 文档 epoch 由 Host 另行交给 Rust，不进入 SDK 请求正文。生命周期事件已接入；主题由 Host 注入 CSS tokens。返回 / 重载由 Host 提供，尚未开放 SDK navigation / actions API。

安装更新、禁用或撤销会失效旧调用，当前没有延迟替换编辑页的模式。卸载会清理该安装的私有 KV；没有保留数据选项。Host 的本地安装身份不等于已验证发布者，Action ID 与使用记录 / 快捷键也不提供跨发布者身份隔离。

详细边界见 [Web 插件设计](../../docs/web-plugin-design.md)。


## 生产 Host 预览

V0.11 的 macOS 页面在加载包之前注入默认 transport，业务可直接导入 `host`。Host 通过原生 main-frame / origin 回调提供身份，JS 不携带授权票据。隐藏立即使未完成请求失效；`lifecycle.onVisibility` 可监听展示事件，自行恢复时再次 `host.ready()`；使用 `definePlugin` 的普通插件无需处理。事件本身不会延长调用资格。

页面复用 ToolPageShell，主题使用 `--aw-text` / `--aw-secondary` / `--aw-surface` / `--aw-border` / `--aw-accent` CSS tokens（当前为应用的深色外观）。Esc 和 ⌘K 由 Host 管理，插件不注册这两个全局键。导航返回与重载当前由 Host 按钮 / 菜单提供，没有通用 invoke、插件声明的原生 UI DSL 或第三方任意导航。

生产页与 JSON / 无框架 Text Pad 示例见 [平台实施记录](../../docs/web-plugin-design.md)。

## 背景自动适配（Host 基础能力）

**插件不需要编写背景适配代码，不需要导入 CSS、注册背景事件、调用 API 或声明额外权限。** Host 对所有 Web 工具执行同一套加载 / 恢复流程，与插件 ID 和前端框架无关。

- 玻璃与窗口底色由宿主提供；原生 WebView 画布支持时透明，不支持时统一深色回退。
- Host 自动设置 `html`、`body` 以及常见挂载根 `#app` / `#root` / `#__next` 的背景和默认文字颜色，覆盖前端模板自带的页面底色（包括加载时的内联背景）。
- 插件负责内容布局和局部组件。页面根节点属于宿主画布，不用再给它们配置背景或 `color-scheme`；普通卡片、图表等子组件仍可使用自己的业务颜色。
- 加载、重载与隐藏后恢复都会应用背景；过时的异步响应不修改已隐藏或替换的页面。没有轮询或整页 DOM 监听。

这属于样式契约，不是安全隔离：插件不要在加载后用脚本反复覆盖宿主保留的根节点样式。SDK 的公共控件 CSS 是可选的，**是否导入它不影响页面背景适配**。

## 统一样式

Flutter 与 Web 的设计参数来自同一份 [`resources/design-tokens.json`](../../resources/design-tokens.json)。Build 18 起，Host 在插件 HTML 之前自动注入默认控件 CSS：普通 button / input / textarea / select / checkbox / radio / label / kbd 均使用共享字体、字号、字重、尺寸、圆角、边界和交互状态，无需插件导入 CSS。当前仅提供与 App 一致的深色外观，没有动态主题订阅 API。

SDK 也提供相同 CSS 的导出，供独立浏览器预览使用，不要求前端框架。生产页不必重复导入：

```js
import '@anywhere/plugin-sdk/styles/controls.css';
```

```html
<main class="aw-ui">
  <nav class="aw-toolbar">
    <div class="aw-actions">
      <button class="aw-button--primary">Run</button>
      <button disabled>Copy</button>
    </div>
  </nav>
  <article class="aw-card"><textarea placeholder="Input"></textarea></article>
  <p class="aw-muted" role="status">Ready</p>
</main>
```

按钮、输入框、下拉框、键帽、卡片、焦点 / 悬停 / 禁用状态均使用 Host 的 `--aw-*` 参数。JSON Workspace 和无框架 Text Pad 都使用这份 CSS；各自 CSS 只处理工具布局。公共卡片内的编辑器自动继承单层表面，无需业务 CSS 再写透明背景。支持时页面保持透明，玻璃由原生窗口提供，不添加第二层模糊；不支持时使用同一套深色回退。

Build 19 起，编辑 / 查询工具可直接使用 `aw-workspace` → `aw-workspace-toolbar` / `aw-workspace-panes`，以及 `aw-pane`、`aw-pane-header`、`aw-pane-body`、`aw-editor`、`aw-pane-footer`。这些类提供与 File Search 一致的轻量工具栏、左宽右窄分栏、分隔线、代码字体 / 背景及留白；`aw-tabs` / `aw-tab` 的选中状态用 `aria-pressed` 表达。标题、返回、Esc 和宿主 Actions 仍由 Host 负责，不在插件里再绘制。

`styles/tokens.css` 仅供独立浏览器预览使用，生产插件不要复制固定颜色覆盖 Host。插件仍可以自定义局部组件 CSS；根画布由 Host 统一管理，局部控件仍不要求第三方页面像素级一致。

常用图标无需导入库或编写 SVG：

```html
<button><span class="aw-icon aw-icon--copy" aria-hidden="true"></span>复制</button>
```

Host 提供 `copy`、`paste`、`run`、`clear`、`format`、`minify`、`escape`、`unescape`、`check`、`chevron-down`，使用 `aw-icon aw-icon--名称`。图标继承文字颜色、统一尺寸与禁用态；保留按钮文字，纯图标按钮需提供 `aria-label`。图标属于呈现资源，不增加桥接 API、权限或运行时依赖。

修改公共样式参数后，在仓库根运行：

```sh
python3 scripts/generate-design-tokens.py
python3 scripts/generate-design-tokens.py --check
```

通用控件规则只修改 [`resources/web-controls.css`](../../resources/web-controls.css)，再运行生成脚本；生成的 Dart 与 SDK CSS 不手改。规则使用低优先级选择器，第三方可覆盖局部布局 / 组件；宿主不能保证任意第三方自定义组件与 Flutter 像素一致。`scripts/check.sh` 检查同步与透明度转换，并运行 Flutter / SDK / 示例回归测试。主题只属于呈现层，Rust 继续负责权限、生命周期和调用执行。

画布透明度由 Host 的平台适配结果控制，默认使用公共深色底色，防止默认白画布与白字冲突。macOS 的 WebView 库 1.1.2 仅清除 scroll-bounce 的 `underPageBackgroundColor`；当前另以存在性检查保护 WebKit 的 `drawsBackground` SPI，并隔离在 `NativeWebAppearance`，不修改 pub 缓存或 Rust。该兼容路径不是公开 Apple API，后续 SDK / OS 升级必须重新验收；接口不可用时保留可读的深色回退。原生测试覆盖真实 WKWebView 透明开关、焦点与接口缺失分支。

宿主 macOS 回复使用受限 UTF-8 `ArrayBuffer`，在文档传输层还原成 SDK 字符串帧，避免原生字符串插值损坏 JSON 转义。插件的 API 与输入处理代码不变；单帧仍为 64 KiB，非法 UTF-8 / 超限帧不交付。该适配不开放通用二进制或流接口。

SDK 实现分为 `client.ts`（具体能力、队列与分块）、`lifecycle.ts`（展示事件）、`plugin.ts`（默认连接寿命），公开入口仅重导出；业务插件不需要依赖这些内部路径。


## UI language

`environment.language` returns the Host's resolved `en` or `zh` language. Use
`environment.onLanguageChange(listener)` for live changes and call its returned
unsubscribe function when the page is disposed. Keep translated resources inside
the package; any normal Web translation library can be used. The JSON example
uses i18next. This metadata needs no permission, conveys no native identity, and
does not change Invocation input, grants, command IDs or stored data. Old hosts
without the metadata default to English; old plugins keep their existing UI.

低频命令和设置可用标准 `<details class="aw-disclosure"><summary>更多操作</summary><div class="aw-disclosure-body">…</div></details>` 在原位置展开；Host 自动提供箭头、焦点与间距，浏览器负责键盘和展开语义，不需要自定义菜单框架。主要操作继续直接显示。
