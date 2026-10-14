# 架构

当前实现：V0.9 S1；最后核对：2026-10-14。使用入口见 [README](../README.md)，专题文档见 [文档导航](README.md)。

## 分层

```mermaid
flowchart TD
    UI[Flutter 界面 / Controller] --> Bridge[FRB API / DTO]
    UI --> Host[DesktopHost / 原生 Host]
    Bridge --> App[Application：装配与用例]
    App --> Core[core：模型、算法与契约]
    App --> Modules[features / extensions]
    Modules --> Core
    App --> Persistence[持久化适配]
    App --> Platform[Platform 系统能力]
    Modules --> Platform
```

| 层 | 职责 | 约束 |
|---|---|---|
| `anywhere-protocol` | Context 类型、展示状态、事件、声明式 Manifest | 共享传输模型，不保存 OS 对象 |
| `anywhere-core/core` | Context Graph、Action、候选、Ranking、SessionFacts、ExecutionSession、Storage 契约 | 不导入 application / features / extensions / persistence，不依赖 Flutter |
| `anywhere-core/application` | 激活、输入、采集协调、快照、执行、扩展生命周期与装配 | 协调用例，不决定像素或原生窗口外观 |
| `features` / `extensions` | 应用、文件工具和第一方领域能力 | 用 Provider / Bundle / Source 贡献，不另建 Launcher 流程 |
| `persistence` | Usage、扩展偏好、模块存储实现 | 数据读写与锁、事务、恢复策略集中管理 |
| `anywhere-platform` | OS-neutral Trait、上下文采集、系统搜索、资源打开、剪贴板、Git 与进程后端 | 原生调用与系统差异留在适配器 |
| `anywhere-plugin` | TOML 发现、校验、模板与有界 UI 数据生成 | 不执行任意脚本或决定界面布局 |
| `anywhere-bridge-flutter` | 初始化、公共 Facade、FRB DTO 转换 | 不复制识别、排序、执行规则 |
| Flutter / Native Host | 页面、导航、输入、焦点、设置、热键、窗口与授权 | Flutter 不匹配业务动作；Swift 不解析输入或运行项目 Action |

业务调用使用 `flutter_rust_bridge`；窗口、设置、热键与目录授权使用 `DesktopHost` / MethodChannel。系统提供 `paths` 和 `capabilities`，共享界面不猜动态库路径或材质能力。跨平台契约见 [平台接入](platform-portability.md)。

## 代码组织

```text
crates/anywhere-core/src/
├── core/           模型与算法；context / action / search / ranking / session / execution
├── application/    activate、update_query、execute、session_runtime、source_runtime
├── features/       applications、filesystem、text、装配贡献
├── extensions/     json、url、git、developer、saved_directories
└── persistence/    usage、extensions、module_storage

apps/flutter/lib/
├── launcher/       Controller、ActivationFlow、导航、Session / FileWorkspace 协调器
│                   ShortcutCoordinator、ObjectActionCoordinator、HostViewNavigator
├── plugins/        Host Detail / KeyValue 渲染（纯数据）
├── settings/       设置草稿、保存队列、快捷键与扩展状态
├── ui/             AppStyle、图标与公共组件
├── infrastructure/ DesktopHost、窗口、生命周期、图标缓存与运行策略
└── src/rust/       已生成的绑定

apps/flutter/macos/Runner/  原生窗口、设置存储、热键、授权、材质与任务窗口
extensions/                编译时嵌入的第一方 Manifest
plugins/examples/          外部声明式插件示例
resources/                 键码、文件类型与运行策略的公共配置
```

## 激活与 Context

1. Host 收到唤醒请求，先保留前台应用焦点。
2. Flutter 同步预留 SessionId，再异步激活；首屏尚未返回也可以取消。
3. Rust 冻结前台应用身份，启动独立的上下文采集、应用目录加载与历史读取。慢速系统调用放在 blocking worker。
4. InputMode 决定种子：Environment 使用应用环境；Content / Search 只解析显式输入；FileSearch 使用文件工作区；全局动作绑定使用 CapturedEnvironment。
5. Provider 顺序派生 Context，预算截断后后台继续；来源结果按会话资格补充。
6. 提交首份快照后显示窗口，后续通过 snapshot wait / delta 发布更新。

目录采集按 Session 单飞，不能因用户输入改变而再次读取“现在的前台应用”。失效应用身份安全跳过目录；不重试到已经取得焦点的 Launcher 或另一应用。详见 [Context 模型](context-model.md) 和 [默认输入策略](context-input-navigation.md)。

## 快照、取消与执行资格

| 身份 / 版本 | 区分什么 |
|---|---|
| SessionId | 激活会话 |
| Revision | 同一会话的输入 / 查询模式变更 |
| SnapshotSeq | 同一 Revision 的后续补充 |
| BusinessActionId | 使用记录与可持久配置中的动作 |
| ExecutionCandidateId | 已提交候选及其具体对象 |
| Catalog version / Execution epoch | 动作定义变更、隐藏与恢复后的执行资格 |
| Host generation | 原生显示、隐藏与迟到回调 |

发布采用“锁内复制不可变输入和版本 → 锁外匹配 / 排序 → 锁内复验并发布”。索引、历史和 Registry 共享不可变快照；并发提交改变输入时重新计算，旧 Session / Revision 的结果直接丢弃。

执行仅接受当前已提交凭据，排队 worker 开始副作用前再次验证；进程计划生成后、spawn 前也检查版本。副作用成功后仍记 Usage，即使窗口已隐藏。菜单另有 discovery token，不能把可重扫的菜单 ActionId 当作永久身份；见 [菜单安全契约](current-app-actions.md)。

隐藏撤销待执行资格。已有操作保留 15 秒，恢复使用新 epoch；空白首页按本次入口新建 Session。已启动进程由 ExecutionSessionManager 持有，不随查询或窗口隐藏终止；应用退出会结束任务。交互详见 [Launcher 契约](launcher-interaction.md)。

## Sources 与文件工作区

`SourceCatalog` 原子持有基础和扩展的 `SourceSet`，同步 / 异步来源具有统一目录寿命。通用 `source_runtime` 负责 admission、取消、批次和迟到结果拒绝；`file_workspace` 负责文件类型、冻结、追加和进度。

普通应用 / Action / 收藏目录共用 Candidate 与公共 Ranking Facts、Usage、时钟。文件工具作为普通 Action 打开，结果有专门的稳定顺序与连续读取，不进入环境 Context Scope。

Spotlight 返回早期候选，Rust 完整遍历允许范围；查询临时存储保存命中并外部排序，已显示前缀保持稳定。对象二级动作按选中目标延迟生成，复验原执行对象。Host 接收 replacement、append 或 metadata delta，不重新匹配文件。

Flutter 的 `StableList` 共享 256 条不可变块，索引按块复用，仅为访问到的候选包装行；Revision 或 list version 改变时拒绝无效尾部。完整契约见 [文件搜索](file-search.md)。

## 第一方模块与持久化

`FirstPartyExtension` 通过 `FeatureContribution` 提交 Provider、ActionBundle 和动态动作；runtime 可贡献 Sources 与有界 ExtensionState。内部静态 Trait 不承诺第三方 Native ABI，也不代表进程隔离。

能力请求由 Host 审批，执行通过 `HostServices` 检查；Storage 的 namespace、字节 / 键额度由 Host 分配。持久化写入成功后发布配置，替换动作、Provider 与模块来源，推进版本并取消旧任务；释放发布锁后 reset 旧异步来源。基础来源保留，已启动 ExecutionSession 不因重载结束。

Manager 只缓存有界 ExtensionState，每次重建 runtime。Reload 重开共享存储连接并刷新状态。`ExtensionStorage.update` 在 SQLite IMMEDIATE 事务中读改写，失败不改数据库或内存快照。查询使用内存状态，必要的执行校验可以读取数据库。

配置损坏或 schema 未知时，扩展关闭且只读，基础 Launcher / 设置继续可用，不覆盖原文件。配置操作无论返回成功还是失败，都重新校验快捷键资格；不确定资格时注销按键。修复后 Reload 恢复。协议见 [扩展与插件](plugin-protocol.md)。

## Host-rendered UI（V0.9 S1）

外部 v3 Manifest 的 render_detail 使用 Protocol `ui::DetailView`；Plugin 负责 schema、模板与展开后的内容预算，Core 在普通执行 worker 内先检查 ui.render grant 后生成 `HostView::PluginDetail`。原 Session / Revision / catalog 校验与 Usage 路径不另建副本。

Bridge 将内容转成 typed `PluginDetailDto`，不将 UI JSON 塞进 output_text。HostViewNavigator 接收已校验执行结果，注册视图适配器；Flutter plugins 模块只渲染字段和纯文本，不匹配 Context 或重做插值。Launcher 保留底层查询 / Scope / 选中项，返回仅关闭视图；隐藏仍使用统一 15 秒生命周期。未知版本 / 超限在进入 UI 前失败。能力批准绑定完整定义，变更 / 撤销沿 ExtensionManager 失效旧会话。

## 扩展入口与验证

- 新 Context：定义类型和值，实现 ContextProvider 并贡献注册。
- 新 Action：ActionSpec / Handler / Bundle；执行能力与绑定资格显式声明。
- 新来源：CandidateSource / AsyncCandidateSource；沿用 Ranking 和生命周期。
- 新工具页面：HostViewNavigator 注册路由，复用导航、共享 UI 和对象 Actions。
- 新系统能力：扩展 Platform / DesktopHost，实现各平台适配器。

`scripts/check-architecture.py` 检查 Cargo 依赖及 core 模块反向导入；行为由公开 Facade、平台和 Widget 回归覆盖。完整门禁是 `scripts/check.sh`。自动化通过不代替真实键盘、权限、窗口、升级及 Windows 验收；当前记录见 [V0.9 MVP](mvp-v0.9.md)。
