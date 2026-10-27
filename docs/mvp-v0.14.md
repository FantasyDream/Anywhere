# V0.14：应用历史与动作排序

状态：**开发预览，尚未完成正式发布验收**。源码版本以 [pubspec.yaml](../apps/flutter/pubspec.yaml) 为准。本页只维护现行范围与验证程度，旧方案及 Build 1–23 经过见 [开发记录](archive/v0.14-development-records.md)。

## 已接入的功能

| 功能 | 当前实现 | 边界 |
|---|---|---|
| 每应用独立历史 Action | 18 个内置来源；在对应 Current App 与显式全局搜索中匹配 | 未安装时默认关闭；手动开关与路径优先 |
| 配置化适配 | 内置 / 可信用户包共用 source.json、transform.js、读取器和共享页面 | 用户同 ID 覆盖优先；启动加载，修改需重启 |
| 来源文件读取 | 有界 JSON、XML、只读 SQLite 与按需 QuickJS 转换 | 一次查询一个来源；不导入或镜像全部历史 |
| 路径检测 | 配置根目录规则；VS Code 优先新的 Home 共享存储，再回退旧位置 | 单来源仅选一份文件；非标准路径可手动设置 |
| 来源页面 | 搜索、拼音、选择、Enter 原应用打开、复制、刷新 | 历史页没有 ⌘K 绑定；更多操作从行内或右键进入 |
| 所选对象操作 | 在一个选中对象上解析 File / Directory Context，复用第一方 Native 动作 | 工作区保持 File，远端仅复制；不改变冻结 Current App |
| 首页与 Scope 排序 | 次数、最近使用、相关性 / 默认优先级、稳定 ID | 无首页白名单或额外加分；关键词匹配仍优先；菜单长期 Usage 仍隔离 |

现行操作和支持范围见 [应用历史](application-history.md)，配置与额度的唯一完整契约见 [来源指南](application-history-sources.md)，动作排序见 [Launcher 交互](launcher-interaction.md)。

## 明确未交付或未验收

- 多安装 / Profile 的显式选择器尚未实现。当前只允许唯一来源应用，歧义时拒绝；自定义路径用于固定一份来源文件，不能当作 Profile 路由。
- Obsidian 当前提供最近仓库。最近文件需要可靠的 Vault / 自定义配置目录关联，已暂缓接入；不把仓库列表或文件时间冒充最近笔记。
- 浏览器历史、书签与 Tab 管理已移至 [独立浏览器阶段](browser-feature.md)，不再属于本版实现范围。
- 18 个来源的完整真实应用、版本、多安装、权限和实际打开矩阵尚未验收；Windows / Linux 的原生运行也未验收。
- 本版未发布 GitHub Release。开发 App 的签名和打包成功不等于正式发行或全部实机路径通过。

## 代码入口

| 改动内容 | 首先阅读 |
|---|---|
| 新应用适配 | resources/application-history 中已有包，以及 [来源指南](application-history-sources.md) |
| 定义、继承与脚本 | application/application_history/ |
| 偏好、会话、查询与执行 | features/application_history/ |
| 来源文件与默认检测 | persistence/application_history.rs 及同名目录 |
| 对象的 Context / Registry 复用 | application/history_object_actions.rs |
| Bridge | anywhere-bridge-flutter/src/api/application_history.rs |
| Flutter 页面与设置 | features/application_history/、infrastructure/application_history_backend.dart、settings/application_history_* |

路径表中 Rust 相对路径均位于 crates/anywhere-core/src，Flutter 路径位于 apps/flutter/lib。完整分层见 [架构](architecture.md)，工作入口见 [AGENTS.md](../AGENTS.md)。

## 最新已验证基线

### 安装授权后直接复制 — Build 25 / JSON 0.7.1

所有第三方插件的剪贴板读写按安装时授予的权限执行，不再逐次弹确认；未声明权限、失效文档和隐式 Context 解析器仍不能越权访问。三个 JSON 复制动作在 Rust 调度的后台运行，成功后关闭 Launcher，不创建 Workspace 或 Flutter 工具页。全量检查通过：Flutter 434、Core 319 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30；桥接生成、编译及签名打包通过。已启动 Build 25、安装 JSON 0.7.1，实机通过三个复制动作、实际剪贴板粘贴核对、转义输入重新匹配与 Workspace 内直接复制；大整数、重复键、指数及中文保留。详见 [Build 25 记录](benchmarks/direct-clipboard-v0.14-build25-2026-10-27.json)。

### 单一 JSON Context — 插件 0.7.0

解析器将普通 / 转义对象或数组统一解码后注册为 `plugin.dev.anywhere.json-workspace.json`，Context 保留 JSON 文本中的数字与重复键，不生成额外转义类型。Workspace 和三个复制动作共同接受该类型；删除 Unescape 入口、工作区菜单操作、路由及文案。Workspace 保留名称搜索空白打开。全量检查通过：Flutter 435、Core 317 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30。真实插件包回归验证统一类型、原文 token 与候选一致性；本机已更新 0.7.0，并核对转义候选、解码预填、菜单与后台授权预览。详见 [0.7.0 记录](benchmarks/json-context-v0.7.0-2026-10-27.json)。

### 插件重构收尾 — JSON 0.6.3

删除 JSON 动作中未使用的标题 / 标签元数据，展示仍由 Manifest 和中英资源负责；旧 Context 调研与逐构建过程已归档。规范、架构、生成物、真实插件包和全量检查通过：Flutter 435、Core 317 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30。主程序仍为已验收的 Build 24；0.6.3 仅清理元数据，未另做原生安装验收，业务路径以以下 0.6.2 记录为准。完整审查及产物校验值见[收尾记录](benchmarks/plugin-closeout-v0.14-2026-10-27.json)。这不替代完整来源矩阵、跨平台或正式发布验收。

### 验收修复 — Build 24 / JSON 0.6.2

工作区与后台复制统一绑定插件文档 Context，保留通用使用次数排序；候选副标题隐藏不透明数据载荷，插件安装说明覆盖界面 / 后台两种运行时。真实编译包回归验证唤醒识别、剪贴板过期、原文传入与默认排序。全量检查通过：Flutter 435、Core 317 / 1 忽略、MVP 195 / 4 忽略、SDK 21、JSON 30。App 签名及 ZIP 校验通过。已切换至 Build 24 并安装 JSON 0.6.2，实机通过原文带入、精度保护 / JSONPath、后台压缩与转义复制、取消不写入、去转义，以及更新后的授权说明。IDEA / VS Code 历史列表、方向键 / 翻页、对象菜单与返回查询均通过；合成目录和保留两个根目录的工作区已在 VS Code 实际打开。物理输入唤醒 / 15 秒保留、设置保存失败时序、完整来源矩阵、内存与跨平台未在本轮重新实测。详见 [Build 24 记录](benchmarks/acceptance-fixes-v0.14-build24-2026-10-27.json)。

逐构建过程归档到 [Build 1–23 记录](archive/v0.14-development-records.md)。
