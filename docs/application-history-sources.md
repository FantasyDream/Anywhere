# 应用历史来源配置与脚本

状态：V0.14 开发预览。支持内置与可信的用户本机适配配置，使用同一读取、转换、Action 和页面契约；不是第三方插件商店或通用命令执行协议。已内置 18 个来源；Obsidian 最近笔记暂缓接入，多实例选择仍待接入。浏览器历史 / 书签与 Tab 管理统一进入[后续独立阶段](browser-feature.md)，不再属于本模块的 V0.14 收尾条件。

## 添加一个来源

1. 打开 **设置 → 应用历史 → 自定义适配 → 打开适配目录**。
2. 新建一个子文件夹，例如 `my-editor`，放入下面两个文件。
3. 重启 Anywhere，在同一设置页填写历史数据文件路径，或在配置中声明自动检测规则。
4. 全局搜索 Action 的标题即可使用；在对应应用中唤醒 Current App 也会匹配。

内置适配位于 `crates/anywhere-core/resources/application-history/<name>/`，与用户适配使用同样的文件格式、校验及执行流程。内置包预先附带常见路径规则；用户目录在 App 启动时读取一次，新增 / 修改配置需要重启，无需重新编译 App。第一方模块代码仍只绑定一次，不恢复热加载。

`source.json`：

```json
{
  "schema": 1,
  "id": "example",
  "title": "Example Editor",
  "file_hint": "recent.json",
  "config_paths": ["ExampleEditor/recent.json"],
  "applications": ["example.editor"],
  "action": {
    "title": "Example Recent Projects",
    "localized_titles": { "zh": "Example 最近项目" },
    "aliases": ["example history", "最近项目"]
  },
  "reader": { "format": "json" }
}
```

`applications` 是匹配安装应用的身份列表；当前 macOS 适配使用 bundle ID。它属于来源配置，转换脚本不能覆盖。宿主根据该列表与本次冻结的 Application Context 匹配 Action；不会读取实时焦点反推应用。只有找到唯一应用才允许打开；缺失或多个安装实例时不猜测。首轮没有提供安装实例选择器。

`transform.js`：

```js
export function transform(input) {
  return input.projects.map(project => ({
    key: project.path,
    title: project.name,
    target: {
      kind: 'directory',
      path: history.localPath(project.path)
    }
  }));
}
```

`action` 定义独立入口的标题、可选中文名和检索别名，稳定 Action ID 为 `core.application_history.<source-id>.open`。在对应 Current App 首页无需输入关键词即出现，也可全局检索、单独绑定快捷键。页面仍为共享的 `application.history`，只查询传入的来源。旧原型的统一 `core.application_history.open` 已移除，如曾绑定它，应重新选择具体应用 Action。

查询接口也必须明确给出来源 ID；空、未知或已关闭的来源返回未配置，不读取任何其他来源，不保留旧原型的“空来源查询全部”回退。每次查询最多处理所选来源的有界结果，设置页的“全部来源”仅用于配置筛选。

转换脚本作者只处理“原数据如何变成条目”。脚本不能定义 UI、来源权限、打开命令或写入命令 Usage。搜索、拼音、排序、过滤、键盘与主窗口布局由宿主统一提供。

### 复用解析器与覆盖内置适配

相同数据格式可省略 `transform.js`，在 `source.json` 中使用内置模板。例如某编辑器采用与 IDEA 相同的最近项目 XML：

```json
{
  "schema": 1,
  "id": "my-editor",
  "extends": "idea",
  "title": "My Editor",
  "applications": ["example.editor"],
  "action": { "title": "My Editor Recent Projects" }
}
```

`extends` 可引用任一内置来源，例如 `idea` 或 `vscode`，继承读取器、文件名提示和转换脚本；新应用默认手动配置文件位置，也可声明自己的 `config_paths`。模板不继承应用身份、Action 标题或路径规则，避免把新应用接到错误的配置上。内置适配也使用相同继承器：JetBrains 系列共享 XML 转换，Code 同类编辑器共享 SQLite 转换，没有逐应用复制脚本。其他格式提供自己的 `reader` 和 `transform.js`；现有读取器仅支持 JSON / XML / 有界 SQLite，不包含注册表、二进制 plist 或任意文件夹遍历。

同 ID 用户配置优先于内置定义，且只产生一个 Action。省略读取器或脚本时可以沿用原内置实现；同时省略 `config_paths` / `home_paths` 时也沿用原路径规则；显式声明任一组后，不混入另一组内置规则。覆盖同 ID 时必须保持应用身份列表一致；改为另一应用应使用新 ID，避免复用错误的快捷键 / 使用记录。用户包重复 ID、非法格式、越界文件或超额时，整个历史功能降级并在设置中提示，其他内置功能继续使用，不静默忽略坏覆盖。修正后重启。

默认用户目录是 App 数据目录下的 `application-history-sources`，以设置显示为准。最多 32 个有效来源；目录最多检查 64 个条目 / 32 个用户子目录，单个定义或脚本最多 64 KiB，总计最多 1 MiB。目录外的符号链接文件不加载。只放入自己编写或已审查的可信配置；QuickJS 的运行限额不是用于安装不可信下载代码的进程沙箱。

## 配置读取位置

没有手动设置时，仅默认开启已安装应用的 Action；未安装应用默认关闭，不因卸载残留的历史文件而开启。内置与用户来源采用相同规则。安装列表在启动时读取，只保存应用身份用于判断；安装或卸载应用后重启可重新计算默认值，不在逐字查询时扫描应用目录。默认值不落盘，设置其他来源也不会把它们固化成手动选择。

手动开关和自定义路径优先并持久保存：手动关闭不会因检测到应用而重开；手动启用即使当前未安装也保留，实际打开仍会重新检查应用及目标。已启用来源自动检测声明的配置目录或用户目录中的文件，无需先手动填写路径。来源较多时可按应用名 / 来源 ID 筛选，或只显示已启用来源；空结果有清除筛选入口。文件位置按需展开，未检测到位置的已启用来源直接显示配置。**设置 → 应用历史** 会显示来源模式与实际使用位置：

- **自动检测**：点击“重新检测”刷新；没找到文件时可直接填写自定义路径。
- **自定义路径**：修改路径后点击“保存自定义路径”。此路径优先，失效时不会静默换用另一配置。支持 `~/`，保存后显示规范绝对路径。
- **显示 Action 开关**：关闭后移除当前应用、全局搜索与快捷键候选，停止该来源读取并撤销旧执行资格；系统快捷键目录同步更新。重启后仍关闭，再次开启会保留原自定义路径。

已有手动路径与关闭状态保持不变。自动路径不永久钉死在某个 IDEA 版本目录：启动、打开对应历史页或读取设置时重新检测；每次输入查询不扫描目录。检测只检查文件位置、类型和可读性，使用 Action 时才读取历史正文，不导入或镜像历史。

文件位置与开关分开保存：关闭 Action 后仍可编辑、保存或改用自动位置；修改位置保留显式关闭状态。草稿与筛选由设置 Controller 持有，切换分类或暂时筛掉来源不丢失。路径编辑支持 Enter 保存、还原编辑；保存后返回路径输入焦点。关闭整个设置窗口时统一处理保存 / 继续编辑 / 放弃；保存失败保留草稿和可见提示，必要时切到对应来源。普通列表回读不会把尚未解决的保存失败当作已修复。

### 自动检测规则

来源包通过可选 `config_paths` 声明相对于系统用户配置根目录的路径，也可通过 `home_paths` 声明相对于用户目录的路径。两组均由相同的有界检测器处理；存在可读的 `home_paths` 候选时优先使用它，不因旧配置文件更晚修改而切回旧位置。Platform 提供根目录：macOS 为 `~/Library/Application Support`，Windows 为 `%APPDATA%`（缺失时使用用户目录下的 `AppData/Roaming`），Linux 为绝对 `XDG_CONFIG_HOME` 或 `~/.config`。通用扫描器不包含应用名称判断，不调用脚本或外部命令。

| 来源 | 相对于配置根目录的规则 |
|---|---|
| IDEA | `JetBrains/IntelliJIdea*/options/recentProjects.xml`、`JetBrains/IdeaIC*/options/recentProjects.xml` |
| VS Code | 优先 `home_paths: [".vscode-shared/sharedStorage/state.vscdb"]`；旧版回退 `Code/User/globalStorage/state.vscdb` |
| VS Code Insiders | `Code - Insiders/User/globalStorage/state.vscdb` |
| Cursor | `Cursor/User/globalStorage/state.vscdb` |
| Windsurf | `Windsurf/User/globalStorage/state.vscdb` |
| VSCodium | `VSCodium/User/globalStorage/state.vscdb` |
| PyCharm | `JetBrains/PyCharm*/options/recentProjects.xml`、`JetBrains/PyCharmCE*/options/recentProjects.xml` |
| WebStorm / GoLand / CLion / PhpStorm / RubyMine / DataGrip / RustRover | `JetBrains/<产品名>*/options/recentProjects.xml` |
| Rider | `JetBrains/Rider*/options/recentSolutions.xml`，兼容旧文件名 `recentSolution.xml` |
| Android Studio | `Google/AndroidStudio*/options/recentProjects.xml` |
| Sublime Text | `Sublime Text/Local/Session.sublime_session`，兼容 `Sublime Text 3/Local/Session.sublime_session` |
| Obsidian 最近仓库 | `obsidian/obsidian.json` |

两组合计最多 8 条规则、每条最多 8 层；每个根目录的通配枚举共享 512 个条目预算，禁止绝对路径、`..`、递归 `**` 以及逃出该根目录的符号链接。同组多个候选按文件修改时间降序、路径升序选择一份，不合并历史；这只是配置选择，不代表精确识别当前 IDE 安装版本。需要固定版本时使用自定义路径。

默认目录依据 [JetBrains 文档](https://www.jetbrains.com/help/idea/directories-used-by-the-ide-to-store-settings-caches-plugins-and-logs.html)和 [VS Code 用户数据路径实现](https://github.com/microsoft/vscode/blob/main/src/vs/platform/environment/node/userDataPath.ts)。便携版、`--user-data-dir`、`idea.config.path`、非标准配置与超出枚举预算的目录仍使用自定义路径。每来源暂时仅支持一个文件，不宣称已支持多安装 / Profile 选择；Windows / Linux 默认根目录保留实现，原生运行未验收。可使用自建合成文件验证。

### 适配范围与证据

本轮参照上游的应用家族划分独立编写配置与转换，没有复制上游 GPL 代码。JetBrains XML 允许不同产品的 component 名称，但只接受唯一的已知近期记录结构；Rider 的 solution 作为文件打开。Code 同类编辑器使用同一受限查询，远端条目仍仅复制。来源中没有时间时保留原顺序，不借用文件访问时间制造“最近使用”。

Obsidian 本轮只列出 `obsidian.json` 登记的仓库，不扫描 Vault 文件树；同名仓库以稳定 ID 区分。[官方 URI 文档](https://obsidian.md/help/uri)支持按仓库 ID 打开。最近笔记、多 Vault 文件聚合延后，不再作为本次收尾的交付条件。官方最近文件 API 属于 Obsidian 插件环境；本机 1.14.4 的工作区元数据虽含最近文件列表，外部读取仍需明确绑定 Vault 和可自定义的配置目录。Sublime 会话文件可能包含编辑缓冲区数据，读取受总量上限约束，转换只提取 `folder_history` / `file_history` / `recent_workspaces`，不展示或保存缓冲区正文。

来源使用合成文件验证解析、检测、应用身份及分发；另有明确版本的 VS Code 本机只读探测，见 [当前验证基线](mvp-v0.14.md#最新已验证基线)。这些证据不等同于已在 18 个真实应用中完成实机打开。自动路径是标准位置，非标准安装、应用版本或格式变化需要自定义路径；未知结构报来源不可用。上游 Typora 适配没有 macOS 支持；浏览器需要批量读取、Profile 路由和更大的来源边界，未借此批次冒充完成。

配置位置依据 [JetBrains 目录规则](https://www.jetbrains.com/help/idea/directories-used-by-the-ide-to-store-settings-caches-plugins-and-logs.html)、[Android Studio 官方排错资料](https://developer.android.com/studio/troubleshoot)、[Obsidian 全局数据说明](https://obsidian.md/help/data-storage)。Rider 当前使用 `recentSolutions.xml`，见 [JetBrains 支持记录](https://rider-support.jetbrains.com/hc/en-us/community/posts/36380823627922-Rider-on-Windows-seems-to-rely-on-old-versions-of-user-data-in-Appdata-Local)。本机仅核对已安装应用的 bundle 元数据，没有为路径研究读取私人历史正文。

## 输入格式

| reader.format | 脚本收到的数据 |
|---|---|
| json | 已解析的 JSON 对象 / 数组 |
| xml | XML 树：`name`、`attributes`、`children`；顺序和重复节点保留，当前不投影文本节点 |
| sqlite | `{ rows: [...] }`，列名作为对象键；仅执行配置声明的受限参数化 SELECT |

SQLite 配置示例：

```json
{
  "format": "sqlite",
  "query": "SELECT value FROM ItemTable WHERE key = ?1",
  "parameters": ["history.recentlyOpenedPathsList"],
  "tables": ["ItemTable"]
}
```

宿主独立打开只读连接，表读取受 authorizer 限制；写入、ATTACH、PRAGMA 和函数调用不能通过该查询入口执行。连接自己的 query_only、缓存和资源限额由宿主设置，不修改来源 journal 模式。当前 SQLite 输入只支持有界小结果，不适用于完整浏览器历史的流式查询；独立浏览器阶段需另补批次契约，不能用 LIMIT 截断后宣称搜完全部历史。

## 返回条目

```json
{
  "key": "stable-source-object-key",
  "title": "Displayed title",
  "target": { "kind": "directory", "path": "/absolute/project" },
  "last_used_at": 1700000000000
}
```

- 返回条目数组。`key` 必须在本来源内唯一且稳定；不用标题或数组位置代替身份。
- `target.kind` 支持 `directory`、`file`、`workspace`，均使用绝对路径。`remote` 使用 `uri`，原型只保留 `vscode-remote` 元数据并支持复制，不打开或转成本地目录。
- `target.kind: "obsidian_vault"` 另带绝对 `path` 和 16 位十六进制 `vault_id`。只允许匹配到 Obsidian 的来源打开；宿主重新验证 ID 与路径，再构造固定的只读 `obsidian://open?vault=…` 请求并交给指定应用。脚本不能提供任意 URL 或新增笔记等写入指令。复制得到仓库路径。
- `last_used_at` 可省略 / null，只在确有最近使用时间时提供 Unix 毫秒；无时间时保留源数组顺序。
- `history.localPath(value)` 是纯路径辅助函数，处理 `file:` URI 和 `~/`，不读取目标文件；无法转换时返回空值。实际存在性和类型在打开前重新检查。
- 不接受额外身份 / 权限 / 脚本动作字段。重复 key、相对路径、未知目标类型、非法时间等使该来源失败，不猜测修复成可执行对象。

## 所选对象的更多操作

右键本地条目或点击行内“更多操作”时，宿主才构造已有 File / Directory Context 并派生 Git / Developer 动作，不在读取列表时解析每个项目。工作区保持 File；远端 URI 只复制。菜单复用现有第一方 Native 动作及其可用性，不运行新的脚本动作或扩大 Web grant。

对象候选保存内存中的来源句柄、规范路径与操作代数；每次执行前重新读取来源并验证 key / 类型化目标，拒绝已换掉的符号链接目标。计划后台任务后、真正启动进程前再次复验。刷新、来源停用、对象改变或打开另一对象菜单都会使旧资格失效；使用记录仍只记稳定业务 Action ID 与 Context 类型，不保存目标路径。

执行结果使用通用工具页导航。返回保留历史查询和选择，重新读取当前有效条目；隐藏取消读取，最终退出释放会话。工具页最多保留四层父页，超出拒绝继续嵌套，不作为无限结果缓存。当前代码与回归已接入，最终原生验收见 [V0.14 记录](mvp-v0.14.md)。

## 运行和生命周期

内置及用户纯转换脚本都使用独立 QuickJS Runtime，在受控 Rust 调用中按需创建和释放。没有 WebView、DOM、Node、模块加载器或文件 / 网络 / 进程 Host API。自定义配置只扩展此历史模块，不获得第三方 Web 插件的权限，也不注册全局 Context 类型；不支持 shell 命令或自定义可执行程序参数。

原型限额为输入 8 MiB、输出 2 MiB、每来源 4,096 条、标题 4 KiB、目标 16 KiB、XML 50,000 节点 / 转换深度 64、JS 堆 32 MiB / 栈 256 KiB / 执行中断预算 250 ms，来源总截止 5 秒。JS 中断不等于所有 OS 文件 I/O 都能硬停止，worker 在实际返回前仍占用唯一读取槽。这些是输入和执行约束，不是整个 App RSS 上限。

页面查询串行合并最新请求，组合输入期间不搜索；复制 / 打开期间提交的查询在操作结束后继续处理，全部较早取消完成后才开始新读取。会话和查询代数复验阻止旧结果发布；过期页面不自动重新申请会话，迟到取消不能清空新页面。首次申请会话失败后，显式刷新可以重试；关闭期间获得的迟到会话只释放，不继续读取来源。

首次进入页面及设置刷新时读回配置并重新检测路径；隐藏取消读取，15 秒规则仍由 Launcher 管理。保留期内重新显示会重新读取该来源的有效数据，沿用当前配置和路径；不把内容刷新当作重新检测安装或配置实例。执行 / 复制前重新读取对应来源，确认 key 与目标未变。开关修改在存储事务内基于最新记录合并，不覆盖其他实例刚保存的自定义路径。

来源文件、格式或脚本失败显示来源不可用；其他来源仍可搜索。未知 schema 不当作真正空结果。关闭 Action 立即清除选择资格；配置 schema 2 只持久保存来源 ID、模式、Action 开关与自定义文件位置，自动路径按需重新定位，历史正文与查询结果不写入模块存储。schema 1 的已保存路径迁移为自定义，省略的旧来源 IDEA / VS Code 保持关闭；之后新增的内置与用户来源按安装状态计算默认开关，不继承它们出现前的省略状态；损坏或未来 schema 不覆盖。移除用户适配后，其设置保留为未激活数据，不阻断其他来源；恢复相同稳定 ID 可继续使用原设置。

## 参考项目

已核对 [utools-recent-projects](https://github.com/LanyuanXiaoyao-Studio/utools-recent-projects) 的注册、JetBrains / VS Code / Obsidian 读取器及设置实现。它使用 TypeScript 应用适配类与逐应用开关；可以借鉴分族解析与支持矩阵，但不是直接可加载的通用配置库。其 `LICENSE` 为 GPL-3.0、`package.json` 标注 MIT，本项目没有复制其代码。保留本项目的只读读取、类型化打开、有界资源与真实来源顺序；不沿用 shell 拼接或把文件访问时间当作最近打开时间。

## 验证

```bash
cargo test -p anywhere-core application_history --locked
cargo build --release -p anywhere-core --example application_history_measure --locked
target/release/examples/application_history_measure
```

测量器仅生成 IDEA / VS Code 合成数据；分别对两个来源的 500 条记录执行空查询、文字、拼音、路径和未命中查询，每组 30 次。每条输出包含来源 ID，不再测量已移除的跨来源聚合查询；不同读取范围的历史样本不能直接用于宣称提速。可为一个全新的独立验收目录准备 500 + 500 条记录：

```bash
target/release/examples/application_history_measure --prepare "$PWD/build/history-fixture"
```

目标目录必须尚不存在，不覆盖个人设置；准备操作不启动 App 或记录真实剪贴板。性能输出包含 Rust 读取 / JS 转换 / 匹配，排除 Bridge、Flutter、显示和真实输入法；真实界面验收另记。

VS Code 1.143.1 的本机只读排查确认：旧数据库无近期记录键，新共享数据库包含 57 条记录（只记录数量，未导出私人条目）。迁移依据上游的 [APPLICATION_SHARED 读写实现](https://github.com/microsoft/vscode/blob/main/src/vs/platform/workspaces/electron-main/workspacesHistoryMainService.ts) 与 [共享存储位置](https://github.com/microsoft/vscode/blob/main/src/vs/platform/environment/common/environmentService.ts)。手动路径继续优先；便携版或 `--shared-data-dir` 使用自定义路径。数据库缺少近期记录键时显示来源不可用，避免将错误文件误报为空列表；`entries: []` 才表示明确的空历史。
