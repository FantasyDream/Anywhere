# 应用历史来源配置与脚本

状态：V0.14 第一轮原型，仅运行随 App 编译的官方来源。不是已开放安装的第三方插件协议。本轮先验证通用读取器、纯 JS 转换及每应用独立 Action / 共享页面；尚未接入浏览器 / Obsidian、自动发现配置或多 Profile 选择。

## 添加一个来源

在 `crates/anywhere-core/resources/application-history/<name>/` 放两个文件。构建时自动按目录收录，无需增加应用专用 Rust 解析分支或 Flutter 页面；修改后重新构建 App，不在运行中加载外部代码。

`source.json`：

```json
{
  "schema": 1,
  "id": "example",
  "title": "Example Editor",
  "file_hint": "recent.json",
  "applications": ["example.editor"],
  "action": {
    "title": "Example Recent Projects",
    "localized_titles": { "zh": "Example 最近项目" },
    "aliases": ["example history", "最近项目"]
  },
  "reader": { "format": "json" }
}
```

`applications` 是匹配安装应用的身份列表；当前 macOS 适配使用 bundle ID。它属于官方来源定义，转换脚本不能覆盖。宿主根据该列表与本次冻结的 Application Context 匹配 Action；不会读取实时焦点反推应用。只有找到唯一应用才允许打开；缺失或多个安装实例时不猜测。首轮没有提供安装实例选择器，遇到歧义应先明确降级。

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

`action` 定义独立入口的标题、中文名和检索别名，稳定 Action ID 为 `core.application_history.<source-id>.open`。在对应 Current App 首页无需输入关键词即出现，也可全局检索、单独绑定快捷键。页面仍为共享的 `application.history`，来源 ID 和标题由 Action 传入；页面只查询这个来源，关闭来源时显示配置提示，不退回其他应用。旧原型的统一 `core.application_history.open` 已移除，如曾绑定它，应重新选择具体应用 Action。

转换脚本作者只处理“原数据如何变成条目”。脚本不能定义 UI、来源权限、打开命令或写入命令 Usage。搜索、拼音、排序、过滤、键盘与主窗口布局由宿主统一提供。

## 配置读取位置

打开 **设置 → 应用历史**，为某来源填写实际元数据文件路径，再点击“启用来源”。填写草稿不会读取历史；保存时检查文件可打开性，进入页面时才解析正文。路径支持 `~/`，保存后使用规范绝对路径；空路径或“关闭来源”撤销该来源。

首轮只支持每个来源一个明确文件，未自动扫描用户配置。IDEA 选择对应版本的 `recentProjects.xml`；VS Code 选择对应用户数据目录的 `state.vscdb`。可以使用自建的合成文件验证，不能把路径示例当作已授予真实数据读取权限。

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

宿主独立打开只读连接，表读取受 authorizer 限制；写入、ATTACH、PRAGMA 和函数调用不能通过该查询入口执行。连接自己的 query_only、缓存和资源限额由宿主设置，不修改来源 journal 模式。当前 SQLite 输入只支持有界小结果，不适用于完整浏览器历史的流式查询；H2 将另补批次契约，不能用 LIMIT 截断后宣称搜完全部历史。

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
- `last_used_at` 可省略 / null，只在确有最近使用时间时提供 Unix 毫秒；无时间时保留源数组顺序。
- `history.localPath(value)` 是纯路径辅助函数，处理 `file:` URI 和 `~/`，不读取目标文件；无法转换时返回空值。实际存在性和类型在打开前重新检查。
- 不接受额外身份 / 权限 / 脚本动作字段。重复 key、相对路径、未知目标类型、非法时间等使该来源失败，不猜测修复成可执行对象。

## 运行和生命周期

当前只运行受信任、随 App 发布的纯转换脚本，使用独立 QuickJS Runtime，在受控 Rust 调用中按需创建和释放。没有 WebView、DOM、Node、模块加载器或文件 / 网络 / 进程 Host API。不是用于下载脚本的进程沙箱；第三方安装仍未开放。

原型限额为输入 8 MiB、输出 2 MiB、每来源 4,096 条、标题 4 KiB、目标 16 KiB、XML 50,000 节点 / 转换深度 64、JS 堆 32 MiB / 栈 256 KiB / 执行中断预算 250 ms，来源总截止 5 秒。JS 中断不等于所有 OS 文件 I/O 都能硬停止，worker 在实际返回前仍占用唯一读取槽。这些是输入和执行约束，不是整个 App RSS 上限。

页面查询串行合并最新请求，组合输入期间不搜索；会话和查询代数复验阻止旧结果发布，旧页面迟到取消不能清空新页面。隐藏取消读取，15 秒规则仍由 Launcher 管理；重新显示先读回配置和有效数据。执行 / 复制前重新读取对应来源，确认 key 与目标未变。

来源文件、格式或脚本失败显示来源不可用；其他来源仍可搜索。未知 schema 不当作真正空结果。关闭来源立即清除选择资格；配置只持久保存来源 ID 与文件位置，历史正文与查询结果不写入模块存储。

## 验证

```bash
cargo test -p anywhere-core application_history --locked
cargo build --release -p anywhere-core --example application_history_measure --locked
target/release/examples/application_history_measure
```

测量器仅生成 IDEA / VS Code 合成数据。可为一个全新的独立验收目录准备 500 + 500 条记录：

```bash
target/release/examples/application_history_measure --prepare "$PWD/build/history-fixture"
```

目标目录必须尚不存在，不覆盖个人设置；准备操作不启动 App 或记录真实剪贴板。性能输出包含 Rust 读取 / JS 转换 / 匹配，排除 Bridge、Flutter、显示和真实输入法；真实界面验收另记。
