# 当前工作树综合审查

2026-11-18。起点为 `codex/experiment-gpui-kit` 上的 `b428899`，以当前含未提交
改动的工作树为审查对象。按目录盘点 Rust Core、Application、Persistence、Platform、
Protocol、插件、GPUI 与工程脚本，深入检查异步回包、列表热路径、系统边界和现有回归。
此前的迁移与修复保留在工作树中。

| 优先级 | 确认的问题 | 修复 |
| --- | --- | --- |
| P2 | 文件分页的直接回包可覆盖已到达的 follow 快照，鼠标加载及迟到的键盘分页都可能改写刚移动的选择 | 分页接入同一快照门禁和专用控制器，新增行只在原键盘选择仍有效时接续 |
| P2 | 文件搜索翻完当前写入边界后，后续匹配可能无法继续加载 | 一次读取确定边界与进度，扫描新增命中后重新开放分页，尾部只剩已显示记录时结束 |
| P2 | 剪贴板写操作能把旧查询的行带回，也可能被较早读取的快照撤销展示 | 按查询代次消费写操作的结果，必要时重发当前搜索；设置直接读取状态与用量 |
| P2 | 任务停止或删除回包与实时列表分别发布列表，较晚到达的旧列表可能恢复已删除行 | 任务订阅单独发布列表，命令回包保留错误反馈；进行中的输出更新保留操作提示 |
| P2 | 等待主线程提交 Finder 显示超时后仍报成功，排队回调可迟到激活 Finder | 超时返回失败并取消未开始的主队列 block |
| P2 | Web SDK 先补发排队请求再转发取消，传输层满载时新请求可能收到 BUSY | 先取消已发送请求，再推进队列；取消排队请求继续只在本地处理 |
| P3 | 文件搜索每次进度变动都按 ID 扫描展示行，完整投影额外收集候选引用并复制去重 ID | 进度更新复用选择和 Scope；使用迭代与借用 ID 渲染，加载按钮明确显示在途状态 |
| P3 | 相同应用排除配置导致重复创建过滤目录及刷新快照 | 归一为相同集合时复用索引快照 |
| P3 | Spotlight 多复制一份原生结果，任务详情清空后仍持有旧日志索引 | 原生结果由 RAII 管理并借用读取，任务模型集中清理详情与输出缓存 |

投影基准条件和数据见[性能](../../performance.md#2026-11-18-文件结果投影配对测量)。
按键、加载按钮或任务提示的回归从 GPUI 测试观察，原生资源超时采用合成
AppKit 边界测试。

## 验证结果

- `scripts/check-gpui.sh` 通过，278 项 GPUI 测试通过。
- `scripts/check.sh` 通过，完整 Rust 工作区、SDK 53 项、JSON Workspace 31 项、
  严格 Clippy、原生合成回归、架构规则、独立插件模板和真实 Rust broker 往返
  检查通过。
- 当前文档共 114 份 Markdown、923 个本地链接，文档检查与 `git diff --check` 通过。
- 重新构建的 `build/Anywhere.app` 为 arm64，内部版本 0.14.0+1，最低 macOS 15，
  使用本地 `Anywhere Local Code Signing`，严格签名验证与 plist 检查通过；不是
  Developer ID，也未公证。主程序 SHA-256 为
  `61ff8797ec56c91098b07952cacbf52983fba2d2a7334bf7cef0d5fcc09e5120`，
  ZIP SHA-256 为
  `7ba0f4d14909401c1048ab7b29b9d52918c5635de30cf7d7594554d1e264a361`。
- 已从精确路径
  `build/Anywhere.app/Contents/MacOS/Anywhere` 启动隔离进程；`lsappinfo` 确认
  PID 58350、Bundle ID `dev.anywhere.launcher` 与上述 App 路径。以下实机结果
  均使用 `build/verification-20261118-review-run1/data`，没有写入日常配置。

## 2026-11-18 实机复验

证据统一位于 `build/verification-20261118-review-run1/evidence/`，其中
`identity.txt` 保存进程、版本、架构、签名和摘要。

| 场景 | 实测结果 |
| --- | --- |
| 文件分页与 Quick Look | 300 个 `anywhere-page-*` 文件的首批显示 10 行；滚动后可加载下一批，方向键从第 019 项连续选中第 020 项且不跳回。在文件搜索页用 `⌘Y` 打开系统 Quick Look，显示 `pagination fixture 000`；Esc 关闭后查询、选择与结果行保留。证据为 `file-search-page.png`、`file-search-page2-selection.png`、`file-quick-look.png`、`file-quick-look-return.png`。 |
| 剪贴板 | 独立数据目录记录 `clipboard-alpha-20261118` 与 `clipboard-beta-20261118`；查询 `beta` 只显示 beta，快速切换查询后没有旧回包复活。Quick Look 显示 beta，Esc 恢复。重开 Settings 后显示 15 条、8.6 MiB，条数和容量已刷新。证据为 `clipboard-query-beta.png`、`clipboard-preview-beta.png`、`settings-clipboard-refresh.png`。 |
| Settings | 修改隐藏文件开关后按 Esc 出现“关闭前保存修改？”；继续编辑保留草稿，保存并关闭后重开仍为开启，再恢复为关闭并保存。剪贴板录制开关可即时持久化。证据为 `settings-close-confirm.png`。 |
| 全局热键重绑定 | 将当前应用快捷键由 `⌃⌥⇧O` 改为 `⌃⌥⇧J` 并保存；隐藏后旧 J 不再唤醒，新 J 显示 Launcher。恢复 O 后再次隐藏确认旧 J 无效、O 正常唤醒。配置文件最终恢复 `control+alt+shift+KeyO`。证据为 `hotkey-rebind-j.png`。 |
| Running Tasks | 从 Node 合成项目启动 `npm Run · accept-long`，窗口自动打开并显示运行中、命令详情、stdout/stderr 与 `start`。复制命令得到真实 npm 可执行路径；首次任务运行至 Completed / Exit 0，第二次启动后立即 Stop，界面显示“已停止”且输出保留。证据为 `running-task-live.png`、`running-task-completed.png`、`running-task-stopped.png`。 |
| JSON Workspace | 合法 JSON 正确格式化；附加 `abc` 后显示 `InvalidSymbol · line 10, column 2`，结果为 0 字节。宿主操作菜单包含 Reload / Back；Reload 更换 document epoch 并恢复空态，Back 返回原查询和 JSON 结果行。证据为 `json-workspace-valid.png`、`json-workspace-invalid.png`、`json-workspace-actions-menu.png`。 |
| Application History | Cursor 来源成功加载本机最近项目；用一个测试关键字筛选后只保留匹配项目与文件，Esc 清空查询并恢复完整来源列表。没有打开外部项目。证据为 `application-history-cursor.png`。 |

本轮仍未标记为通过：

- File Search 扫描过程中新增匹配的实时增量、鼠标“加载后立即上移”的竞争序列尚未重复；
  现有结论来自本页列出的确定性回归。
- Running Tasks 的“移除结果”未在 GUI 中执行，避免删除操作未经即时确认；停止与完成态
  已实测。
- Reduce Transparency、Increase Contrast、浅色材质、真实中文 IME、VoiceOver、多屏、
  Finder 超时、登录项与整 App footprint 仍待验。

### Finder 与 Space 定向复验

继续使用同一 `build/Anywhere.app`、二进制 SHA-256、PID 58350 和隔离数据目录。
以下结果来自 Computer Use 操作和系统窗口的 Accessibility 树。

- 在 Finder 只选中合成的 `node-finder-check` 目录，从 Finder 按 `⌃⌥⇧O` 唤醒
  Current App。空查询首页出现标注“所选文件”的路径动作和 Node Scope。点击 Node，
  四个 `npm Run` 动作 `alpha`、`beta`、`delta`、`gamma` 均以该目录为目标。
  首页只显示当前排序的前几项。Finder 选中项目通过 Automation Apple Events 读取，
  与 AX 菜单发现分属不同权限。原场景的遗漏未复现，原因仍未确认。
- 在 File Search 输入 `anywhere-finder-check`，等待文本文件命中后按 `↓`。确认行内
  预览按钮已出现、查询框仍有焦点，再用 Computer Use 发送 Space。系统 Quick Look
  显示夹具正文。按 Esc 后查询和选择保留，编辑器焦点恢复。真实键盘和中文组字中的
  Space 仍待人工验收。上一轮的 `prefer_character_input` 只是干扰假设。

## 桌面验收

使用独立测试目录和 [本机验收包配置](../../development.md#独立验收数据)。从当前
源码生成 `build/Anywhere.app` 后，以真实 Core 启动包内可执行文件。
打开文件所在目录需要真实权限；剪贴板录制与清除时只操作合成数据。

```bash
qa_dir="$(mktemp -d "$PWD/build/review-qa.XXXXXX")"
ANYWHERE_DATA_DIR="$qa_dir" \
  build/Anywhere.app/Contents/MacOS/Anywhere
```

| 场景 | 操作 | 预期 |
| --- | --- | --- |
| 分页与选择 | 给测试搜索根准备超过 128 个同前缀文件，搜索并滚动到底部。点击加载后立即用方向键上移；下一批用末行 ↓ 触发 | 按钮在读取时显示加载状态。鼠标加载或已移动选择时保持当前行；仍在末行等候时移动到新批第一项 |
| 增量结果 | 让包含多级目录和大量匹配的测试根跨多个扫描批次；扫描时反复翻到末尾并等待 | 发现更多匹配后重新出现加载入口。当前行保持稳定，同一个路径只出现一次 |
| 剪贴板 | 在独立数据目录开启记录，先后复制两段可区分文字。删除一项并立即改查询。用中英文各试一次 | 旧结果不会替换新查询，已删项不会复现，长查询输入保留。设置页暂停 / 修改保留期 / 清空只更新状态和用量 |
| Running Tasks | 启动一个持续输出的合成项目任务，复制命令或日志。停止后移除并观察同时运行的另一任务 | 后续输出不抹除复制提示。停止状态与移除结果经订阅到达，已移除任务不恢复 |
| Finder 与 Web | 从测试文件执行 Show in Finder。打开合成插件，在排队请求时取消当前请求，然后继续操作 | Finder 显示正确对象。已取消请求不处理迟到结果，队列中的下一项继续运行 |

真实中文 IME、原生焦点、VoiceOver、材质、WebKit 呈现与整 App footprint 仍需复测。

## 同日复审的桌面验收补充

使用包含复审修改的构建与独立测试数据目录。

| 场景 | 操作与预期 |
| --- | --- |
| 文件快批 | 在 Settings → Search 依次选择 SystemIndex、Ripgrep、Combined。用可区分的已索引与未索引测试文件检查早期候选，完整遍历后检查三种配置的最终范围和文件类型一致。快批可能晚于遍历，应结合平台调用证据判断。 |
| 文件分页 | 滚动触发下一批，在加载过程中选中末行并按 ↓，追加后应移到下一行。重复时先移走选择，再确认迟到回复保留当前选择。加载时切走并恢复 Launcher，检查按钮能继续响应。 |
| Running Tasks | 筛选为只显示旧任务，从 Launcher 启动不同标题的新任务。应复用 Tasks 窗口、清空筛选并显示新任务输出。在快速输出时切换任务、停止任务或关闭再打开窗口，检查详情始终对应当前任务并最终显示终态。 |
| 剪贴板 | 分别在 History 和 Shelf 查询同一批合成数据，再打开 Settings 查看存储用量。对一条记录执行删除并重复查询，设置重新打开后应显示新的条数和容量。 |

## 应用目录、设置与原生预览复审

| 场景 | 桌面操作与预期 |
| --- | --- |
| 应用目录 | 在应用目录扫描较慢的环境中快速多次唤起 Launcher。观察目录日志和线程活动，最多有一个实际扫描在运行。扫描结束前已有应用结果仍可搜索，最后一次请求完成后可搜索新安装应用。 |
| 搜索目录草稿 | 保存根 A。设置中删去 A，选择 B 并放弃草稿。搜索仍只访问 A。退出并重启后 A 仍能恢复。再次选择并保存 B，搜索再切换到 B。授权失败时重复检查 A。 |
| Quick Look | 从主窗口预览测试文件，打开剪贴板架并预览另一条合成内容。隐藏主窗口，剪贴板架预览继续显示。交换接管顺序，再关闭被接管的窗口。确认面板内容、焦点和临时预览资源对应最终所有者。 |
| 登录项 | 在专用测试用户的默认数据目录保存启动偏好并观察审批流程。审批完成或解决失败原因后，在通用设置直接重试。即使保存的搜索目录暂时不可访问，也能完成登录项重试。确认状态提示清除及系统登录项状态，再用显式数据目录测试隔离。 |
| 应用历史 | 用合成来源交替刷新与输入查询，待新结果显示后打开或复制某一条。只有当前查询结果可执行，迟到的早期查询不使当前结果失效。 |

本轮已经执行的桌面流程见“2026-11-18 实机复验”；其余行仍保留为待验，不能用
自动化结果替代。扫描并发数由受控测试验证，App footprint 尚无本轮测量样本。

剪贴板初始读取和随后查询的回归测试共用生产查询接收入口，并在 GPUI 测试线程按指定顺序投递回复，避免真实 Tokio 工作线程干扰确定性调度。
