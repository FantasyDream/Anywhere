# 性能测量与证据边界

当前 GPUI 产品还没有形成可发表的整 App 性能基线。本页规定如何采集新样本、
如何解释现有诊断字段，以及哪些旧数据只能作为历史参考。

## 不能混用的指标

| 指标 | 能回答的问题 | 不能回答的问题 |
| --- | --- | --- |
| Core 查询耗时 | Rust 激活、匹配、排序或文件读取用了多久 | GPUI 是否已完成布局、栅格化或显示器呈现 |
| `hotkey_to_visible_ms` | 热键事件到展示调用经过多久 | 系统是否已经绘制首帧、输入法是否就绪 |
| 单次 RSS / footprint | 某一时刻进程保留多少页 | 是否存在泄漏或长期趋势 |
| WebKit helper 内存 | 特定 XPC 实例当时的资源占用 | 所有系统 XPC、GPU 或未来峰值 |
| 隐藏窗口资源 | 隐藏生命周期是否释放文档或对象 | 用户感知的显示延迟 |

性能结论必须说明样本尺寸、构建类型、硬件、macOS、语言、缩放、数据量和操作
序列。单次样本、编译耗时或一个 helper 的 RSS 不能写成整 App 承诺。

## 可重复的 Core 测量

仓库保留以下测量入口：

```bash
cargo run -p anywhere-core --example hot_path_measure --release --locked
cargo run -p anywhere-core --example file_search_measure --release --locked
cargo run -p anywhere-core --example clipboard_history_measure --release --locked
cargo run -p anywhere-core --example application_history_measure --release --locked
cargo run -p anywhere-core --example memory_measure --release --locked
```

文件搜索还可使用[脚本](../scripts/bench-file-search.py)；先读脚本参数，使用临时
或脱敏目录。报告至少包含 p50、p95、最大值、样本数和取消 / 错误计数，不保留
查询正文、剪贴板内容或真实用户路径。

GPUI 的剪贴板结果页和 Shelf 一次读取录制状态与匹配条目。设置页单独读取已存条数
和字节数。Tasks 合并选中任务在途期间的详情更新，减少重复拷贝 stdout 和 stderr。
整 App footprint 仍待实测。

## GPUI 桌面测量

GPUI 当前显式记录：

```text
hotkey_to_visible_ms
```

该字段从收到全局热键事件开始，到产品壳发起主窗口展示和 Launcher 激活结束。
它是产品代码路径诊断，不包含显示器实际呈现。需要获得端到端数据时，应在同一
构建上分别采集：

1. 热键事件到窗口 `isVisible` 状态；
2. 显示到首份可交互快照提交；
3. Rust 查询自提交输入到末次可接受 snapshot；
4. 隐藏、15 秒保留到期与再次显示后的窗口 / WebView / Quick Look 资源；
5. 主进程与已归属 helper 的 footprint，而非只记录一次 RSS。

物理 IME 候选选择、Quick Look 首帧、WebKit 内容呈现和窗口合成必须单独测量，
不能从 Rust 或 GPUI 状态测试推断。

### Launcher 隐藏与过渡验收

在同一台 Mac、同一份测试数据和同一缩放下对比优化前后的 Release 构建。记录构建编号、
PID、macOS 版本及显示刷新率。空白首页与保留会话分别采样，连续操作至少五轮。

| 操作 | 验收现象 |
| --- | --- |
| 热键或状态栏唤起，键入、快速删除，按 Esc 清查询 | 输入框获得焦点且可立即输入。窗口从顶边稳定调整，无中间空白边框、旧内容拉伸或清空时闪回 |
| 中文组字，切换 Current App、Global、文件搜索与 Settings | 组字时的键盘路由保持原样，切页后可立即操作目标页；打开系统「减少动态效果」后重复验证 |
| 空白页隐藏，再切到另一个前台应用立即唤醒 | 隐藏后旧快照和图标缓存释放，唤醒后捕获新的前台应用 |
| 带查询、较多结果或文件页隐藏 | 15 秒内恢复查询、选择和可继续的操作；超过 15 秒唤醒后开始新会话 |
| 保留会话期间从状态栏打开 Settings 并修改未保存的设置 | 保留期限结束后草稿仍可保存或取消，关闭设置后 Launcher 正常获得焦点 |

分别记录空闲、操作后可见、隐藏约 1 秒、隐藏约 16 秒和再次唤醒的主进程 footprint。
验证包含 WebView 的场景时单记该会话所属的 WebKit helper。比较每阶段的中位数和多轮峰值，
同时记录 `launcher_workspace_retention_expired` 与 `launcher_window_resized_atomically` 的时间。
自动化回归可验证 Rust 快照和缓存对象的所有权释放，进程 footprint 下降幅度以实测为准。

### 2026-11-13 合成投影基线

`projection_measure` 的旧样本输入了 10,000 条合成候选，但当时模型最多投影 7 行。
该样本在 Release 构建下预热 10 次并采集 100 次，只测 `LauncherModel::apply_shared` 的展示投影，不读取用户数据，
也不包含 Core 查询、GPUI 布局 / 栅格化、窗口合成或显示器呈现：

```bash
cargo run -p anywhere-gpui --example projection_measure --release --locked
```

本机为 Apple M5、10 核、16 GB，macOS 27.2.1，工具链
`rustc 1.100.0-beta.7`。同次运行的样本为：

| 场景 | p50 | p95 | max |
| --- | ---: | ---: | ---: |
| 共享候选列表，仅元数据变化 | 0.208 µs | 0.25 µs | 3.375 µs |
| 全新模型的完整可见投影 | 28.625 µs | 31.833 µs | 95.458 µs |

上表代表旧的紧凑投影。完整列表的历史样本见下节，当前文件候选与视口投影见 11 月 21 日的配对测量。机器、构建、fixture 或场景
改变时不得直接比较数值；比较最终实现与基线时还应记录 Core 发布、通道等待、
GPUI notify / resize 次数和实际首帧证据。

当前通信实现把激活、恢复、输入、分页与 follow 的 Launcher 快照统一为
`Arc<LauncherState>`，单次请求使用容量 1 的回复通道，Core execution、原生事件
与 Web IPC 使用有界通知。上述改动解决的是所有权、排队和正确性问题；现在没有
足够证据把合成投影微秒数换算成用户可见性能提升。

启动测量现在分两段解释：r5 隔离包的初始状态栏到 Ready 状态栏 / 热键替换约为
105 ms，从材质准备到 Ready 的日志跨度约为 134 ms；首份可执行快照仍由真实
Session / Core 查询决定。
这个单次样本不能代表慢磁盘、权限提示或冷 LaunchServices；后续 A/B 必须使用
同一个阻塞存储 fixture，并同时记录主线程是否被占用、Ready 延迟和首次可用快照。

### 2026-11-18 文件结果投影配对测量

Apple M5、16 GB、macOS 27.2.1、arm64、Rust 1.100.0-beta.7 Release。
同一 fixture 投影 10,000 行文件搜索结果，进度更新用例选中末行，两边各预热
10 次、采样 100 次。当时的 `projection_measure` 使用旧候选 fixture。

| 场景 | 修改前 p50 / p95 / max | 修改后 p50 / p95 / max |
| --- | ---: | ---: |
| 共享列表，仅进度等元数据变化 | 39.208 / 39.833 / 41.917 µs | 0.084 / 0.166 / 1.292 µs |
| 完整投影 10,000 行 | 1480.792 / 1920.875 / 2089.500 µs | 1318.416 / 1671.125 / 1830.667 µs |

共享列表保持同一行选择，省去按 ID 扫描及重建 Scope。完整投影按需迭代并使用
借用 ID 去重，省去候选引用数组与临时 ID 副本。数据只包含模型投影，未测量
Core、布局、首帧或整 App 内存。

## 2026-11-16 图标请求与任务文本优化

图标缺失、读取失败或通道中断会在原 128 项 LRU 中记录 30 秒的失败缓存，期间
保持普通图标占位，避免完成回调立即再次读取同一图标。到期后允许后续候选更新
重新请求，不启动独立重试计时器。隐藏窗口和其他工具页不补发 Launcher 图标请求；
返回 Launcher 后继续补齐。图标失败不覆盖页面业务提示，旧 ticket 不能结束新请求。

Tasks 的两路有界输出在 Host 投影为 GPUI `SharedString`，详情、当前输出控件和
复制按钮闭包共享同一文本。只在明确复制时构造系统剪贴板所需的字符串，不改变
Core 的 128 KiB / stream 限额、截断、换行或 Unicode 内容。

同机配对合成测量使用 Apple M5 / 16 GB、macOS 27.2.1、arm64、Rust
1.100.0-beta.7 Release；每路 128 KiB，预热 100 次、采样 1,000 次：

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 每次详情 clone 申请的 Rust 堆字节 | 262,206 B | 62 B |
| clone p50 | 3.708 µs | 0.167 µs |
| clone p95 | 4.208 µs | 0.209 µs |
| clone max | 41.583 µs | 3.084 µs |

命令为 `cargo run -p anywhere-gpui --example task_detail_measure --release --locked`，
原始结果见 [配对样本](archive/benchmarks/gpui-task-detail-2026-11-16.json)。
剩余 62 B 是该 fixture 的任务元数据副本。这里只测详情 clone，排除初次 Host
投影、显式复制、Core 读取、文本布局、GPU 和原生呈现；申请字节不是 RSS 或
footprint，也不代表整 App 内存或界面延迟按同一比例下降。

Tasks 的筛选复用同一匹配规则；每行标题只做一次小写转换，目录只在标题不能
满足词项时按需转换一次。此处没有单独耗时测量，不据此给出整页性能数字。

## 2026-11-18 Release 构建配置、投影共享、拼音与剪贴板锁分离测量

同机测量环境：Apple M5（10 核）、16 GB、macOS 27.2.1、arm64、`rustc 1.100.0-beta.7`。

### R8 Release Profile 对照（保持 `panic = "unwind"`）

分别在 clean `--release` 后构建 `anywhere-gpui`、运行 `crates/anywhere-core/examples/measure.rs`
与 `scripts/bench-file-search.py`（3 次取中位数）：

| 方案 | 配置摘要 | Clean 构建耗时 | 主二进制体积 | `.dSYM` 体积 | `measure` 典型（`chrome` / `weixin` p95） | `file_search_measure`（10k / 40k / 10k全匹配 `complete_ms`） |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| A | 默认 release（无 `[profile.release]`） | 111.88 s | 43.91 MiB | — | 0.062 ms / 0.859 ms | 11.43 / 34.23 / 12.44 ms |
| B | `lto = "thin"`, `codegen-units = 1` | 119.33 s | 36.80 MiB | — | 0.052 ms / 0.854 ms | 11.32 / 34.04 / 13.67 ms |
| C | B + `strip = "symbols"`, `debug = "line-tables-only"`, `split-debuginfo = "packed"` | 151.39 s | 29.49 MiB | 186.78 MiB | 0.052 ms / 0.853 ms | 11.71 / 34.07 / 13.12 ms |
| D | C + `opt-level = "s"` | 115.82 s | 28.08 MiB | 150.27 MiB | 0.064 ms / 1.204 ms | 11.64 / 34.38 / 12.64 ms |

方案 D 因 `opt-level = "s"` 使 2,000 中文名拼音匹配 p95 劣化约 40%（0.85 ms → 1.20 ms）且应用匹配劣化约 23%，未采用；最终在 `Cargo.toml` 采用**方案 C**（体积缩减 32.8%，查询延迟与基线持平），并在 `scripts/build-gpui.sh` 与 CI 中将 `.app.dSYM` 保存在 `.app` 同级产物目录（已验证 `atos` 可还原符号与源码行号）。

### R9 候选行共享字符串与按需切片投影

`LauncherRow` 改为 `SharedString`，首页改为按可见下标渲染且不再每帧克隆整表。
以下是当日 `projection_measure` 的历史样本（10,000 候选、预热 10 次、采样 100 次，方案 C Release）：

- `metadata_only`：`p50 = 0.083 µs`，`p95 = 0.125 µs`，`max = 1.209 µs`；
- `full_projection`：`p50 = 1641.709 µs`，`p95 = 1715.208 µs`，`max = 1736.500 µs`。

### R10 拼音匹配门槛测量（未触发预计算）

在 `crates/anywhere-core/examples/measure.rs` 新增 2,000 条多样化中文名称（含 `ü` 音词条如「绿茶文档编辑器」「旅游攻略助手」「律师事务所」）× 7 组查询的采样：

- `"weixin"`：p50 = 0.826 ms，p95 = 0.860 ms；`"wxkfgj"`：p50 = 0.904 ms，p95 = 0.945 ms；
- `"chongqing"`：p50 = 0.846 ms，p95 = 0.888 ms；`"lvcha"`：p50 = 0.829 ms，p95 = 0.869 ms；
- `"lvyou"`：p50 = 0.823 ms，p95 = 0.860 ms；`"feishu"`：p50 = 0.809 ms，p95 = 0.845 ms；
- `"missing"`：p50 = 0.906 ms，p95 = 0.948 ms。

所有查询 p95 均低于 1.0 ms 门槛，按方案约定暂不引入字段级预计算缓存，保留基线样本供后续回归对比。

### R11 剪贴板锁持有耗时与锁分离

在 `crates/anywhere-core/examples/clipboard_history_measure.rs` 增加 `continuous_2mib_image_contention` 压测（1,000 条既有记录 + 写线程连续写入 20 × 2 MiB PNG，读线程并发调用 `status()` 与 `snapshot()`）：

- **锁分离前**：`poll` 持锁 20/20 次超过 5 ms（p50 = 7.25 ms，p95 = 8.75 ms），直接阻塞 `status()`（p50 = 6.23 ms，p95 = 6.75 ms，20/22 次 > 5 ms），触发优化门槛；
- **锁分离后**（`HistoryService` 拆分 `state` 与 `repository` 并移除 `status()` 中的过期写操作）：`status()` 并发延迟降至 `p50 = 0.125 µs`、`p95 = 0.208 µs`、`max = 0.250 µs`（0/22 次 > 5 ms）；`snapshot()` 仍走 SQLite 检索（p50 = 9.04 ms，p95 = 9.63 ms），不再阻塞状态栏与设置状态读取。

## 2026-11-21 原生文件结果的所有权与视口投影

同一台 Apple M5、16 GB、macOS 27.2.1、arm64，使用 Rust 1.100.0-beta.7 Release。
两边均使用相同的 10,000 条合成文件候选、英文文本、路径、执行 ID 与堆分配计数器。
基线是本次修改前的 GPUI 工作树。前后两个独立可执行文件交错运行三轮，每个场景
每轮预热 10 次、测量 100 次。显示缩放不参与这个无窗口测量。

```sh
cargo run -p anywhere-gpui --example projection_measure --release --locked
```

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 新模型接入快照并读取前 8 行，p50 / p95 / max | 1785.500 / 2104.959 / 2714.250 µs | 133.708 / 162.875 / 492.583 µs |
| 末行已选中，仅快照序号变化，p50 / p95 / max | 0.083 / 0.125 / 2.042 µs | 0.125 / 0.250 / 2.333 µs |
| 接入文件结果后模型新增保留的 Rust 堆字节 | 4,889,319 B | 77,997 B |

p50、p95 分别取三个单轮统计量的中位数，max 取全部样本最大值。第一行含模型和快照外壳的创建与释放。
第二行也包含快照外壳的构造，当前 fixture 每次会构造两个空的共享候选数组，该场景的中位耗时略有增加。
堆计数在完整 Core 展示快照准备好后开始，以模型从预览行切换为文件结果前后的净差报告。构建去重表时的
临时分配、滚动期间的渲染、Core 扫描、GPU、进程 footprint 与物理首帧均需分别测量。
三轮输出见[配对样本](archive/benchmarks/gpui-file-projection-2026-11-21.json)。

Core 切换查询会放下旧文件结果，仍在读取旧快照的工作线程可自行结束。提交诊断保留候选总数，
类型与排名明细仅记录头部 6 项。下一项值得实测的是深分页时的 Core 峰值。
当 GPUI 仍持有上一页，追加可能触发完整展示向量的写时复制，执行表还单独保留复验过的文件候选。
可固定页数并让消费者有意保留上一版，测量每次追加的分配、峰值及取消后的回落；
若采用共享分块或执行索引，应分别验证排序、旧快照一致性、目标与授权随版本失效。

## 2026-11-23 进程内共享深化、原生 BGRA 图标管线与 SQLite FTS5 优化

本轮统一优化涵盖：

1. **Core 热路径分配**：`UsageKey` 改为 `Borrow<dyn UsageKeyRef>` 零分配查询。文件搜索用增量 `displayed_file_paths` 避免翻页全量扫描，执行索引指向独立的已复验候选，原始候选保留稳定 ID；`published_state: Option<Arc<LauncherState>>` 在 watch 通道与查询回复间共享同一只读快照。
2. **原生 BGRA 图标管线与 GPU 纹理回收**：macOS 光栅化输出预乘 BGRA8，GPUI 封装成 `Arc<RenderImage>` 前转换为直通 alpha，省去 PNG 编解码；LRU 缓存上限为 96 项（隐藏缩减至 32 项），淘汰与 idle 清理调用 `window.drop_image(...)`。
3. **GPUI 运行时简化与虚拟列表**：`LauncherView` 直接在 GPUI executor 上监听 `watch::Receiver<Option<Arc<LauncherState>>>`，取消多余的 Tokio 中继任务与 `mpsc` 通道；Tokio blocking 线程池限定 `max_blocking_threads(16)`；主 Launcher 列表统一升级为 `uniform_list`。
4. **SQLite WAL + FTS5 trigram 与外部排序**：剪贴板 SQLite 使用 WAL、2 MiB 页缓存和 FTS5 `trigram` 索引。v5 迁移启用 FTS5 `secure-delete`，重建旧索引并截断 WAL；文件搜索非测试构建外部排序阈值为 4 MiB / 4096 条每 run。

以下为修复候选 ID、图标 alpha 和索引删除前的同机 Release 测量样本（macOS arm64，不含 GPUI 布局、GPU 栅格化、OS 合成与整 App footprint）：

| 测量入口 | 场景 | p50 | p95 | max / 备注 |
| --- | --- | ---: | ---: | --- |
| `clipboard_history_measure` | 1,000 × 8 KiB 文本，精确查询 `"entry-0999"`（1 命中） | 106 µs | 112 µs | max 155 µs（FTS5 trigram 索引命中） |
| `clipboard_history_measure` | 1,000 × 8 KiB 文本，未命中 `"no-such-entry"`（0 命中） | 11 µs | 12 µs | max 13 µs（FTS5 快速裁剪） |
| `clipboard_history_measure` | 1,000 × 8 KiB 文本，混合 `"中文 rust"`（1,000 命中） | 2,013 µs | 2,383 µs | max 2,559 µs（FTS5 + `instr` 混合验证） |
| `clipboard_history_measure` | 连续 2 MiB 图片竞争写入下 `status()` | 0.084 µs | 0.209 µs | 0/22 次 > 5 ms |
| `hot_path_measure` | 1,000 菜单项匹配 `"example operation"` / `"operation"` / `"missing"` | 493.9 / 357.3 / 71.8 µs | 863.8 / 386.8 / 77.0 µs | 100 样本 |
| `projection_measure` | 10,000 候选仅元数据更新 | 0.208 µs | 0.375 µs | max 2.25 µs |
| `projection_measure` | 10,000 候选完整投影前 8 行 | 182.542 µs | 212.334 µs | 净保留堆字节 77,997 B |

物理 IME、Liquid Glass 材质、Quick Look 首帧、WebView 合成及长期进程/GPU footprint 趋势仍需在真实 macOS 桌面环境独立验收。

## 历史材料

过去的界面 era 样本和逐构建 JSON 位于
[历史基准归档](archive/benchmarks/README.md)。其中的系统、构建、框架和指标
定义与当前 GPUI 产品不同，只用于解释当时的回归，不构成当前版本基线，数字和
结论也不应复制到发布说明。

## 回归门槛

当前门禁优先检查确定性、取消和资源所有权：

- 旧 Session / revision / snapshot 不覆盖新状态；
- 查询切换后旧结果不会继续追加；
- 激活、恢复、输入、分页和 follow 统一投递共享 `Arc<LauncherState>`，不得重新
  引入 owned 快照双分支或 host 无界状态队列；
- 隐藏立即撤销执行资格；
- Quick Look generation 防止旧关闭回调影响新预览；
- Web document 替换、teardown 和迟到 IPC 不污染新 handle；
- 状态栏、主窗口和 Shelf 复用同一清理路径。

出现疑似泄漏时，先重复操作序列并用 footprint 趋势与对象 / helper 归属比较，
再决定是否修改缓存或渲染路径。不能用关闭一次窗口后的单点下降证明资源已释放。
