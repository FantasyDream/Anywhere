# V0.8 MVP：第一方模块与 Extension v2

状态：2026-10-14，分支 `dev-0.8`，起点 `dev-0.7@3b39763`。当前开发版本 App `0.8.0+13` / Rust `0.8.0-dev.13`。本文是当前范围与验收状态，逐构建证据见 [开发记录](archive/v0.8-development-records.md)。

## 目标与进度

| 切片 | 交付 | 状态 |
|---|---|---|
| S1 模块与协议 | 真实第一方模块、版本化 metadata、宿主能力检查、v1 兼容 | 工程实现与自动化完成 |
| S2 用户控制 | 模块开关、v2 明确批准 / 撤销、持久偏好、重载失效与快捷键同步 | 工程实现与自动化完成；macOS 模块开关、v2 审查 / 批准 / 撤销已实机检查，完整矩阵保留 |
| S3 模块服务 | Git / Developer 迁移、收藏目录 Source、隔离 Storage、额度 / 事务 / 恢复 | 工程实现与自动化完成；macOS 收藏目录闭环已实机检查，完整平台矩阵保留 |

不包括任意 Rust 动态库、Shell / JS runtime、Marketplace、插件 Widget / WebView 或 PTY。Windows App 未交付。

## 当前模块与契约

第一方模块：JSON、URL、Git、Maven、Gradle、Node、Rust、Spring Boot、收藏目录。metadata 在 `extensions/` 编译时嵌入，Rust 实现在 `anywhere-core/src/extensions/`；旧的空转发入口已移除，仓库代码统一使用实际模块路径。业务 ActionId 和外部插件协议保持兼容。

- 继续沿用原 ActionId、ProviderId、Context 类型、Scope、Usage 与快捷键，不做重复注册或身份迁移。
- 静态 FirstPartyExtension / FeatureContribution 不是外部稳定 ABI，受信任代码不因能力接口而成为进程沙箱。
- 外部 v2 仅支持复制、打开 URL / 文件及对应明确批准；v1 继续兼容，v2 不开放 execute_builtin。
- 生命周期原子替换贡献、推进资格版本、撤销旧会话 / 快捷键，已启动任务独立。
- 模块状态只保留有界持久数据，runtime 可重建；Storage 事务内读改写，Reload 重开连接 / 刷新状态。
- 配置损坏或未知 schema 保持只读与原文件，基础功能可用；修复后 Reload 恢复。

详细接口见 [扩展协议](plugin-protocol.md)，职责与 Source 生命周期见 [架构](architecture.md)。

## 收藏目录的实际闭环

Directory Context 中 Save Directory 保存 canonical path；Global Search 按目录名、saved / bookmark 查找并打开；⌘K → Forget Saved Directory 移除收藏，不删磁盘目录。

100 项 / 64 KiB，上限内存快照供查询、SQLite 持久化。普通名称不解析 forget / remove / unpin 操作前缀。打开复验收藏资格与真实路径；成功 Usage 使用公共 Ranking。两个独立 Storage 连接的并发更新、失败回滚和替换数据库恢复均有回归。

## 本阶段回归收口

| 问题 | 当前行为 |
|---|---|
| 多实例收藏写入覆盖 | IMMEDIATE 事务内读取最新状态后更新 |
| 配置已生效但响应失败 | finally 同步实际快捷键资格，不确定目录清空注册 |
| 普通目录名被当作删除 | 移除只在对象 Actions，查询只找目录 |
| Source 排序不读 Usage | 使用公共 Ranking Facts、Usage 与时钟 |
| 损坏 / 替换存储无法恢复 | Reload 重开共享句柄并刷新有界状态 |
| 目录应用身份失效显示内部警告 | 安全跳过目录，不把后台采集提示挂到 Global / Content |
| 空白 Global 恢复阻止 Current App | 空白首页新采集；查询、文件 / 二级页面仍保留 15 秒 |
| 普通搜索进入临时目录 | 平台识别系统临时路径 / macOS 更新缓存；遍历与 Spotlight 一致排除，显式指定路径可覆盖 |
| OneDrive 回收站超时产生搜索警告 | 普通搜索排除主目录 / 云盘提供商回收站，保留正常云盘文件与项目同名目录；显式路径可覆盖 |
| 遍历中临时子目录消失导致黄色警告 | 消失的子项安静跳过；搜索根消失与真实读写失败仍报告部分结果 |
| 新收藏目录默认动作为移除 | 二级列表固定对象默认动作在首位，其余保留公共 Ranking |
| 批准弹窗初始焦点不明确 | 初始聚焦 Cancel，Enter 取消，右方向键后 Enter 批准 |
| 文件异常都显示目录无法搜索 | 显示实际异常数量，悬停查看原因；最多 8 条诊断写日志；权限跳过不计错误 |
| 未识别选区 / 剪贴板打开内容页面 | 仅具体非 Text Context 允许自动填入；后台识别失败保持 Current App，迟到结果复验交互与到期时间 |
| 主目录简写不能产生路径 Context | `~` / `~/…` / `～/…` 共用 Rust 展开器，仍验证真实路径与文件搜索授权 |

菜单永久身份仍未由 AX 元数据证明；菜单历史 / 持久绑定继续隔离，不将此风险标为已消除。详见 [菜单安全边界](current-app-actions.md)。

## 最近一次工程证据

Build 13 将选区 / 剪贴板自动填入限制为真实识别器产生的具体非 Text Context；普通文本、无效 URL / JSON 和不存在的路径不打开内容页面。桥接改为 worker 调用，Host 显示 Current App 后异步决定，复验用户交互、Session / 生命周期和 Rust 提供的剪贴板到期时间；迟到的新选区会重新判定来源。

Build 13 完整门禁：**403 Rust / 229 Flutter 通过，6 Rust ignored**；Native、fmt、Clippy、架构 / 生成规则、文档与 analyze 通过。记录 `build/validation/v0.8-context-prefill-check.log`。识别规则、选区无效时的剪贴板回退、真实路径 / Home 简写 / 删除路径，以及异步失败、编辑、关闭、来源变化和识别期间过期均有回归；15 秒工作区保留回归通过。没有本轮 GUI 验收。

Build 12 根据 Build 11 的真实日志补齐回收站排除：OneDrive 提供商根下 `.Trash` 读取返回 `os error 60`，不应参与普通文件名搜索。主目录 / 云盘提供商回收站在进入遍历前排除，Spotlight 候选执行同一规则；普通云盘文件、项目内部同名目录与显式回收站路径保留。日志测试的共享记录清空增加互斥保护，避免并行测试相互删除证据。

Build 12 最终完整门禁：**401 Rust / 221 Flutter 通过，6 Rust ignored**；Native、fmt、Clippy、架构 / 生成规则、文档与 analyze 通过。记录 `build/validation/v0.8-recycle-check-final.log`。本轮没有重跑真实 OneDrive 扫描或 GUI，不能把模拟文件树回归当作实机验收。

Build 11 完成代码与文档清理：移除四个旧领域转发入口、旧 Flutter 导出入口、未使用设置下拉组件、Launcher 设置转发方法及重复 getter；使用实际模块与 SettingsController 入口。旧配置迁移 / v1 插件支持保留。文档门禁验证 37 篇 Markdown、199 个本地链接，禁止引用忽略的构建产物。

Build 10 增加临时目录策略，测试验证遍历 / 索引过滤一致、正常缓存与项目目录保留，以及显式路径继续可搜索。完整规则见 [文件搜索](file-search.md#临时目录与回收站过滤)。

Build 11 完整门禁：**399 Rust / 221 Flutter 通过，6 Rust ignored**；Native、fmt、Clippy、架构 / 生成规则及 analyze 通过。记录 `build/validation/v0.8-final-check.log`。实际 Carbon 注册测试还验证了冲突回滚、撤销、暂停、关闭与跨进程冲突，记录 `v0.8-closeout-hotkeys-live.log`；它不证明实体键盘的按键送达。

- 配置生命周期、旧会话 / 动作失效、持久批准与改变定义重新确认、失败回滚、存储恢复均有 API / SQLite 回归。
- 初次收藏的二级列表错误把 Forget 排在 Open 前；先用测试复现，再在公共对象动作入口固定默认操作首位，其他动作保持 Ranking。
- 批准弹窗明确聚焦 Cancel；在关闭普通 Tab 的窗口配置下，Enter 取消、右方向键后 Enter 批准通过 Widget 回归。
- 消失的扫描子项与消失的配置根分别验证：前者跳过，后者报告部分结果；诊断详情缺失也不会隐去提示。
- 主目录输入 Content / Search / Environment 三种模式、路径空格 / 内部波浪号与授权范围回归仍通过；查询 / 文件工作区 15 秒保留与空白首页新采集回归仍通过。

本轮候选 Build 8 的真实 macOS GUI 检查已完成模块关闭、恢复开关、v2 权限内容 / 批准 / 撤销、文件工具输入、固定预览随选择切换，以及 Save → 搜索收藏 → Forget 闭环。最后发现的焦点与首项顺序修复进入 Build 9 并通过自动化；最终包界面复查因 Mac 锁屏暂未完成。具体工程记录见 [Build 9 收尾](archive/v0.8-development-records.md#build-9--工程收尾2026-10-13)。

Build 13 App / arm64 ZIP 已核验：37 文件 / 8 链接逐项一致，四个主二进制为 arm64，Impeller 与 deep / strict 签名有效，designated requirement 与 Build 12 相同。包检查记录 `v0.8-context-prefill-package-check.json`。

本地 GUI 测试插件已移除，原扩展偏好与收藏数据已恢复，Host 设置字节一致；成功测试动作产生正常 Usage。没有停止用户的开发任务或删除测试目录中的真实文件。

## 发布前仍需验收

- 真实系统键盘、重复唤醒与失焦、选区 / 剪贴板、IDEA / Finder / 浏览器组合。
- 最终 Build 13 的弹窗 / 对象顺序界面复查，以及其余 v2 能力、修改插件后的重新确认、异常恢复完整 GUI 矩阵。
- 完整系统隐私授权 / 撤销、网络 / 可移动卷、安装升级及签名 / 公证分发。
- 整 App RSS、帧时间、长列表与实际任务输出，以及辅助功能 / 读屏。
- Windows 完整 Host、系统适配器、安装包与实机矩阵。

版本推进见 [路线图](roadmap.md)。旧验收只对各自版本有效，见 [归档](archive/README.md)。
