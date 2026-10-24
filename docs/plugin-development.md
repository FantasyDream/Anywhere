# 第三方插件开发指南

插件就是一个本地网页：你负责内容和交互，Anywhere 负责窗口、背景、返回、权限和宿主能力。可以用普通 JavaScript，也可以用 React、Vue、Svelte 等框架。

**当前范围：V0.12 的 macOS 本地开发预览。** 下述简化入口已在 V0.11 Build 23 接入，当前 V0.12 继续支持，不能跨版本仅比较构建号。SDK 尚未发布到 npm，使用导出的 `.tgz` 安装。开发需要 Node.js 22+ 和 Python 3.10+；插件运行时不需要 Node、Python、Rust 或 Flutter。

## 1. 从模板开始

取得 SDK 安装包后，执行：

```sh
npm exec --package=/绝对路径/anywhere-plugin-sdk-0.1.0.tgz -- anywhere-plugin init my-tool
cd my-tool
npm install --ignore-scripts
npm run build
npm run pack
```

得到 `output/my-tool.awplugin`。模板可以输入文字、转成大写、请求复制，也能接收本次 Action 的文本输入。创建器不会覆盖已有目录。

**开始开发只需修改 `src/main.js` 的业务逻辑。** 需要调整内容布局时编辑标准 `index.html`；Manifest 中改名称、ID、命令和实际需要的权限。窗口、玻璃背景、字体、按钮、输入框、启动握手、隐藏恢复和未完成请求清理由宿主 / SDK 提供，不必再写。

SDK 自动保存在 `vendor/plugin-sdk/`，项目可独立移动。提交源码、Manifest、构建文件、npm 锁文件和 `vendor/`；忽略 `node_modules/`、`dist/`、`output/`。以后用 `npm ci --ignore-scripts` 重装构建依赖。使用自己的正式插件 ID，例如 `dev.yourname.my-tool`，不要发布模板的 `dev.example` ID。

## 2. 安装并试用

1. 打开 Anywhere → **Settings → Features → Web tools → Import**。
2. 选择 `.awplugin`，核对名称、版本和权限后批准。
3. 在 Global Search 搜索 **My Tool**，回车打开。
4. 输入文字，点 Uppercase；点 Copy text… 后，宿主会询问是否允许本次复制。

修改后重新执行 `npm run build`、`npm run pack`，再导入更新。**Reload 只重新加载已批准的包，不读取源码目录。** 当前不支持开发服务器和 HMR。

## 3. 开发时改哪里

```text
my-tool/
├─ manifest.json       身份、入口、Action 和权限
├─ src/main.js         业务逻辑
├─ index.html          标准 HTML 内容布局
├─ build.mjs           已配置好的构建入口
├─ package.json
├─ vendor/plugin-sdk/  SDK 快照，无原仓库依赖
├─ dist/               构建后的网页资源
└─ output/             可导入的 .awplugin
```

不需要引入 CSS 或手写原生消息桥。所有网页依赖应构建进包内，不使用 CDN 或远程页面。React / Vue 等框架也可以使用同一个 SDK，不受模板限制。

### Manifest 常用字段

| 字段 | 用途 |
|---|---|
| `id` | 换成自己的唯一 ID，如 `dev.yourname.text-transform`；发布后保持稳定 |
| `name`、`version` | 显示名称与版本；更新时递增版本，如 `0.1.1` |
| `entry` | 本地 HTML 入口，模板为 `dist/index.html` |
| `permissions` | 只声明实际使用的宿主能力 |
| `commands[].id` | 稳定的 Action ID；不要随显示名称一起改动 |
| `commands[].title`、`keywords` | Action 标题与搜索关键词 |
| `commands[].accepts` | 接受的 Context 类型，如 `core.text` |
| `commands[].entrypoint` | 传给同一网页的路由参数；多个 Action 可复用一个页面 |

同一个插件在匹配和使用记录分数相同时，按 Manifest 的命令顺序排列，建议把默认入口放在第一项。

`schemaVersion: 1`、`apiVersion: "1"`、`platforms: ["macos"]` 按模板保留。完整声明见 [最小模板](../packages/plugin-sdk/templates/minimal/manifest.json)。

## 4. 接收输入与调用宿主

页面渲染后调用 `definePlugin`，业务只处理输入和操作：

```js
import {definePlugin} from '@anywhere/plugin-sdk';

const input = document.querySelector('textarea');
const plugin = definePlugin({
  onLaunch({input: initial}) {
    if (initial.kind === 'text') input.value = initial.text;
  },
});

// 在按钮处理器里调用；成功后再提示已复制。
await plugin.host.clipboard.writeText(input.value);
```

`onLaunch` 每个页面实例只执行一次，隐藏后恢复不会覆盖文稿。返回 / 重载时 SDK 自动释放连接；React / Vue 卸载组件时也可以调用 `plugin.dispose()`。如有异步业务，可用 `onLaunch(invocation, {signal})` 的信号判断页面是否已销毁。

不传 `onError` 时显示默认错误提示；需要业务提示时传入 `onError(error)`。业务按钮自身的异步操作仍应捕获失败、恢复按钮状态，取消复制后不能显示成功。SDK 自动处理协议和分块，不自动重试副作用。

返回、重载和工具选项由 Host 提供，SDK 当前没有 `navigation.back`、`actions.set` 或 `host.theme.subscribe`。主题 / 背景由 Host 自动注入；不需要注册页面动作来适配样式。

输入可能是 `text`、`url`、`resources` 或 `empty`。按工具名称打开时可以没有输入，不要把搜索关键词当成正文。`empty.reason` 会说明没有对象、未授权或超限等原因。

| 功能 | SDK 调用 | Manifest 权限 |
|---|---|---|
| 接收文本 | `onLaunch(invocation)` | `{"name":"context.read","types":["core.text"]}`，同时声明 `accepts: ["core.text"]` |
| 获取唤醒前的应用名称与 ID | `plugin.host.context.getCurrentApp()` | `{"name":"context.currentApp"}` |
| 读取剪贴板文本 | `plugin.host.clipboard.readText()` | `{"name":"clipboard.readText"}` |
| 复制文本 | `plugin.host.clipboard.writeText(text)` | `{"name":"clipboard.writeText"}` |
| 读写自己的小型设置 | `plugin.host.storage.get(key)`、`set(key, value)`、`remove(key)` | `{"name":"storage.local"}` |

剪贴板读写每次都需宿主确认，取消返回 `USER_CANCELLED`。成功后才显示“已复制”，不要自动重试写入。模板已处理错误和等待状态。

`context.read` 也支持 `core.url`、`core.file`、`core.directory`、`core.file_selection`，需要同时加入 `accepts`。文件只提供名称、类型和不透明句柄，目前不能读取文件正文或取得完整路径。

当前没有 Shell、Node API、任意文件访问、网络请求、图片剪贴板、后台监控或完整 Context Graph API。

### 自动匹配 JSON

给需要自动匹配的命令增加以下字段，并保留 `context.read` 的 `core.text` 权限：

```json
{
  "accepts": ["core.text"],
  "inputMatchers": ["json", "json_escaped"]
}
```

Host 会识别输入、选中文本和 15 秒内新复制内容中的 JSON，展示对应 Action；用户执行后才将原文传入页面，插件不用编写识别器。

`json` 识别严格 JSON **对象 / 数组**；`json_escaped` 识别经过字符串编码的对象 / 数组（允许省略外层引号，最多 4 层）。自动识别最多 16 KiB 原文、解码后 128 层；标量、不完整内容、注释和尾随逗号不匹配。识别期间不启动 WebView，Invocation 保留原文，具体解码由插件实现。不要另写监听剪贴板的代码。

## 5. 界面与生命周期

- **背景自动适配**：无需权限、API 或背景 CSS。不要给根节点设置不透明背景或覆盖 Host 的 `color-scheme`。
- **窗口由 Host 提供**：不用再画标题栏、关闭按钮、搜索框或返回按钮。
- 推荐直接保留模板的 `aw-tool`；需要自定义布局时使用 `aw-ui`、`aw-toolbar`、`aw-actions`、`aw-card`、`aw-muted`；按钮与输入框使用共享样式。
- 编辑器 / 查询工具可用 `aw-workspace`、`aw-workspace-toolbar`、`aw-workspace-panes`、`aw-pane` 和 `aw-editor`；分栏与代码区域跟随 File Search 的共享设计，不用再画两张独立大卡片。完整类名见 SDK README。
- 自定义局部组件时使用 `--aw-text`、`--aw-secondary`、`--aw-surface`、`--aw-border`、`--aw-accent` 等 CSS 变量。
- 常用图标也由 Host 提供，例如 `<button><span class="aw-icon aw-icon--copy" aria-hidden="true"></span>复制</button>`。无需图标依赖或样式代码；保留文字标签，仅图标按钮须提供 `aria-label`。完整列表见 SDK README。
- 使用标准 HTML 控件并为输入框提供标签；宿主负责 macOS 嵌入视图的无障碍连接与生命周期，无需插件注册原生节点或修改框架。实际 VoiceOver 可操作性仍需验收，不能用样式一致或单元测试代替。
- 保留 Esc 和 ⌘K 给 Host；局部快捷键不要拦截输入法组合输入。

隐藏后页面最多保留 15 秒，未完成宿主请求会失效。`definePlugin` 自动取消待处理请求，恢复时重新就绪并保留文稿；插件无需监听可见性或重复读取输入。`onDispose` 可用于释放业务自己的计时器等资源。

超过保留时间、返回或重载后，页面可能重新创建。Storage 适合保存偏好，不要默认持久化用户正文。

导入批准新版本、禁用或撤销会立即失效旧调用，不提供“先保留旧编辑页、稍后更新”的模式。更新前保存需要保留的内容。卸载会清理该安装的私有 KV；目前没有保留数据选项。包的 ID / 名称不是发布者证明，同 ID 的本地更新仍需自行核对来源和全部权限。

## 6. 打包与排错

`npm run pack` 使用 SDK 自带打包器，只收集根目录的 `manifest.json` 和 `dist/`，不会打包源码、依赖和 SDK 安装包。`.awplugin` 是 ZIP，不要直接压缩整个项目目录后改名。

资源放在别处时，显式指定运行资源目录：

```sh
python3 node_modules/@anywhere/plugin-sdk/tools/package.py . \
  --assets dist --assets assets --output output/my-tool.awplugin
```

入口和图标必须包含在所选资源中；不支持符号链接。打包器不执行安装或授权，完整校验由 Anywhere 导入时完成。

| 问题 | 检查方式 |
|---|---|
| 浏览器里显示 `HOST_UNAVAILABLE` | 宿主 API 需在 Anywhere 中验证；普通浏览器只适合预览网页 |
| 搜不到 Action | 确认插件已启用，搜索标题 / 关键词，先回到 Global Search |
| 输入为空 | 检查 `accepts` 和 `context.read`；按名称打开时无输入是正常情况 |
| `PERMISSION_DENIED` | 修改权限后需重新打包、导入批准 |
| `INACTIVE` / `UNTRUSTED_DOCUMENT` | 页面已隐藏、返回或更新；停止旧请求，恢复后发起新操作 |
| `TOO_LARGE` | 单次文本上限 1 MiB；控制帧上限 64 KiB，文本分块由 SDK 处理；Storage 只存小型设置 |
| 修改后仍是旧页面 | 重新 build → pack → Import；仅 Reload 不会更新安装内容 |

Storage 合计最多 1 MiB / 256 个键，单值最多 64 KiB，实际请求还受控制帧限制。业务操作应捕获错误；可通过 `{signal: abortController.signal}` 主动取消。

## 维护者：导出 SDK

以下命令仅用于在 Anywhere 仓库中导出 SDK，插件作者不需要安装整个仓库：

```sh
mkdir -p build/plugin-sdk
cd packages/plugin-sdk
npm ci --ignore-scripts
npm pack --pack-destination ../../build/plugin-sdk
```

SDK 包含编译后的接口、类型、公共 CSS、模板和打包器。更多信息：[SDK 说明](../packages/plugin-sdk/README.md)、[JSON 示例](../plugins/examples/json-workspace/README.md)、[平台设计](web-plugin-design.md)。

## 7. 进阶用法

保留原始 `host` / `createHost` / `lifecycle` API，供已有集成或需要自主管理生命周期的框架使用；普通插件优先用 `definePlugin`，不要混用两套连接。完整 API 与共享布局类见 [SDK 说明](../packages/plugin-sdk/README.md)。这些是普通 Web API 与 CSS 工具类，不是插件 UI DSL。


## 跟随宿主语言

宿主设置支持跟随系统 / 简体中文 / English。SDK 的 `environment.language` 提供解析后的 `en` / `zh`，`environment.onLanguageChange` 订阅变化并返回取消函数；组件销毁时取消订阅。无需增加权限或调用 Rust，语言只属于呈现信息，不能用来验证调用者身份。资源随包发布，不增加联网加载器。

JSON 示例使用 i18next，语言切换只重绘文案，保留输入、公式和输出。插件可自行选择常规 Web 国际化库；旧插件不接入仍可运行。宿主不翻译用户正文、公式、命令输出或第三方包未提供的文案。
