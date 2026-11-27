# GPUI 图标、任务与内存审查

日期：2026-11-16。继异步与选择修复后，先清理历史构建产物并打包两个分支，
再继续本轮审查。main 为干净的 `624a339` Flutter 3.50.0 工作区；GPUI 为
`codex/experiment-gpui-kit` 的 `b428899` 及当前未提交修改。main 源码未修改。

## 问题与改动

| 编号 | 级别 / 类型 | 修改前的可达问题 | 修改 |
| --- | --- | --- | --- |
| O01 | P1，任务操作资格 | 筛选为零项时内部下标仍为 0，selected_row 返回未显示的首项；⌘. 经 stop_selected_execution 可停止该任务，迟到详情也可能被接受 | selected_row 复验当前筛选，零项返回 None；停止、移除、详情等调用者共享这条资格；失去选择时关闭旧任务菜单 |
| O02 | P2，重复 I/O / CPU | 图标返回 None、失败或关闭通道后移除 pending，立即补发同一项，形成没有等待的重试；隐藏窗口或工具页也继续补发 | 正负结果共用 128 项 LRU，失败缓存 30 秒；后续候选更新才允许重试；隐藏与工具页停止补发，返回 Launcher 补齐；保留 ticket 和 4 并发上限 |
| O03 | P2，UI 滚动 | 每次任务输出发布都对同一选择调用 reveal，用户滚动侧栏查看其他任务时被拉回所选项 | 只在选择身份变化或显式打开任务时 reveal，普通输出更新保留侧栏位置 |
| O04 | P2，新任务选择 | 选中任务尚未进入列表时，第一份旧快照就 take 掉 pending ID；下一份包含目标的快照无法再选中它 | 直到目标实际出现才消费 pending 选择；后续手动选择 / 筛选撤销旧意图，继续沿用清空不匹配筛选的已有行为 |
| O05 | 性能 / Rust 分配 | 渲染克隆整个详情，再为按钮和文本各复制一份输出；两路有界正文在重绘时反复深拷贝 | Host 把输出投影为 SharedString，详情、控件与按钮共享；仅实际复制时创建剪贴板字符串；保留原内容和 Core 限额 |

O01、O02 和 O03 均在修改生产逻辑前由回归复现失败；O03 测试使用真实的
920×600 测试 viewport，手动滚动到 -500 px 后收到新输出，旧实现回到 0 px。

实现入口：

- [icon_cache.rs](../../../apps/gpui/src/launcher/icon_cache.rs)：有界失败缓存及无等待 TTL 测试。
- [pages.rs](../../../apps/gpui/src/launcher/pages.rs)：可见选择与统一筛选。
- [view/mod.rs](../../../apps/gpui/src/launcher/view/mod.rs)：图标调度、侧栏位置与 pending 选择。
- [tasks.rs](../../../apps/gpui/src/launcher/view/pages/tasks.rs)、
  [dto.rs](../../../apps/gpui/src/host/dto.rs)、
  [runtime_support.rs](../../../apps/gpui/src/host/runtime_support.rs)：共享输出投影。
- [optimization_regressions.rs](../../../apps/gpui/src/launcher/view/tests/optimization_regressions.rs)：确定性状态与布局回归。

## 测量与自动化

新增 10 项回归：图标 None / Error / 关闭通道、失败缓存到期和容量、旧 ticket、
隐藏 / 工具页停止读取、空筛选资格、任务侧栏位置、迟到任务选择与手动导航优先级、Unicode / 空白
文本共享。任务文本 clone 的 Release 配对结果为每次申请 **262,206 B → 62 B**，
p50 **3.708 µs → 0.167 µs**。完整样本、环境、p95 和范围见
[性能指南](../benchmarks/gpui-2026-11.md#2026-11-16-图标请求与任务文本优化)与
[原始 JSON](../benchmarks/gpui-task-detail-2026-11-16.json)。

该测量不含 Core 查询、初次 Host 投影、显式剪贴板写入、GPUI 布局 / GPU / 窗口
合成，也不是 RSS、footprint 或整 App 的内存下降证据。没有新的实机性能数字。

最终 GPUI 228 项通过，完整工作区 Rust 为 911 passed / 6 ignored / 0 failed；
`scripts/check-gpui.sh`、`scripts/check.sh` 和 `git diff --check` 均通过。
两个包的签名、plist、架构、ZIP 内容和 SHA-256 已校验，详情维护在
[当前 MVP](../../mvp-gpui.md)。

## 构建清理与双版本交付

清理进入本轮时的旧 App、验收构建、日志、fixture 与编译产物。原
`build/Anywhere.app` 进程为 PID 95078，确认用户允许后通过
NSRunningApplication.terminate 正常退出；确认进程消失后才删除其 Bundle 和
独立验收数据目录。日常 Application Support 数据未操作。

只保留两个版本：main 的 `Anywhere-Flutter.app`（0.14.0+46，Impeller=true）和
当前工作区的 `Anywhere-GPUI.app`（0.14.0+1），各自 ZIP、SHA-256 与构建来源清单
集中在仓库的 `build/`。Flutter 使用 main 自己的构建 / 打包脚本和锁定的 3.50.0
SDK；GPUI 使用当前分支脚本。继续修复后重新构建并覆盖 GPUI 包，避免交付旧修复。
源码修改没有提交、合并或推送，两个分支没有混合。

## 人工验收清单

所有项目均为 **Pending**；本轮只退出旧实例，没有启动新包。使用独立绝对路径
的 ANYWHERE_DATA_DIR、测试热键和合成任务，记录构建来源、OS、缩放、语言。

| 项目 | 序列与预期 | 状态 |
| --- | --- | --- |
| 空筛选 | 启动两项可停止的合成任务，输入零匹配筛选，按 ⌘. / Enter、尝试更多操作；任务均继续，界面不显示隐藏任务详情。清空筛选后可正常停止明确选中的任务 | Pending |
| 任务滚动 | 多于一屏任务，所选任务持续输出；侧栏滚动到其他位置，等待多次输出更新，位置不回跳。方向键、点击或显式打开新任务仍滚入正确行 | Pending |
| 新任务回包 | 在旧列表可见时启动新任务，延迟其行进入快照；最终选中新 ID；不匹配旧筛选时清空筛选，不停留在旧任务详情 | Pending |
| 图标失败 | 使用缺图标的合成目录或暂不可读图标，查看读取日志和 CPU；30 秒内无立即重试。隐藏 / 打开 Settings 后不补发，返回 Launcher 后补齐；恢复图标后通过后续候选更新重试成功 | Pending |
| 长输出 | 两路各 128 KiB 合成输出，交替 Output / Diagnostics、上下滚动、Latest 与复制；换行、缩进、中文和截断提示正确，复制对应当前标签全文 | Pending |
| 物理资源 | 在同一构建重复任务选择 / 输出 / 关闭，独立采集主进程 footprint 与 helper 归属；检查趋势，不能把本轮 clone 字节数解释为进程内存 | Pending |
| 双版本包 | 分别用独立数据与不冲突热键验证 Flutter 和 GPUI 的启动、输入、Esc、Settings、预览和退出；不要同时让两个相同产品身份争抢默认热键 | Pending |

用户安排实际执行时间，结果需绑定当前构建；旧截图、单测或签名不能把本表改成 Pass。
