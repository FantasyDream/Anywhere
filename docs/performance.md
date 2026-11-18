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

上表代表旧的紧凑投影。当前完整列表的同机样本见下节。机器、构建、fixture 或场景
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
10 次、采样 100 次。命令仍为上面的 `projection_measure`。

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
