# V0.14：应用历史与动作排序

状态：**开发预览，尚未完成正式发布验收**。源码版本以 [pubspec.yaml](../apps/flutter/pubspec.yaml) 为准。本页只维护现行范围与验证程度，旧方案及 Build 1–15 经过见 [开发记录](archive/v0.14-development-records.md)。

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

Build 20：Core 307 项通过 / 1 项既有忽略，MVP 195 项通过 / 4 项既有忽略，Flutter 433 项通过；桥接编译、原生、Web、Clippy、架构、规范与生成物检查通过。新增回归覆盖来源配置的完整发布、失败发生时机相同的关闭选择、不同列表行高与逗号输入的草稿一致性。

本轮落实架构评审的四项建议：来源保存与动作发布由应用层单一入口拥有；SettingsController 统一关闭决策，SettingsEdits 拥有搜索 / 隐藏应用草稿和顺序保存；Launcher 和应用历史复用公共 ObjectActionCoordinator；历史会话独占 rowExtent，页面渲染、翻页与定位不再各写一个常量。目标校验与各页面菜单语义继续留在原有所有者。现行入口见 [架构](architecture.md) 与 [AGENTS.md](../AGENTS.md)。

Build 20 已打包，签名及归档校验通过。最后的原生设置页打开 / 关闭冒烟验收因 Mac 锁屏待补，当前最后确认运行的是 Build 19；不把 Mock 原生关闭测试当作实机通过。具体范围与校验值见 [Build 20 重构记录](benchmarks/architecture-refactor-v0.14-build20-2026-10-26.json)。

此前 [Build 19 IDEA 剪贴板修复](benchmarks/idea-clipboard-v0.14-build19-2026-10-26.json)已实机确认长格式采集成功，[Build 18 JSON 命令记录](benchmarks/json-command-availability-v0.14-build18-2026-10-26.json)确认处理动作仅在有匹配内容时显示，[Build 17 默认样式记录](benchmarks/shared-ui-defaults-v0.14-build17-2026-10-26.json)记录普通控件与共享样式验证；这些实现继续包含在本轮完整回归中。

只读实测曾在 Build 13 确认本机 VS Code 新共享存储中的 57 条记录；本轮没有重新读取私人历史。更早的 [Build 16 清理记录](benchmarks/closeout-v0.14-build16-2026-10-26.json)、[Build 15 排序记录](benchmarks/launcher-ranking-v0.14-build15-2026-10-26.json)与 [VS Code 读取记录](benchmarks/application-history-storage-v0.14-build13-2026-10-26.json)保留各自版本的验证边界。本轮未创建 GitHub Release，也未把开发预览写成正式发行。
