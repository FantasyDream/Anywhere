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

### 2026-11-13 合成投影基线

`projection_measure` 使用 10,000 条合成候选，在 Release 构建下预热 10 次并
采集 100 次。它只测 `LauncherModel::apply_shared` 的展示投影，不读取用户数据，
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

该表用于发现重复投影回归，不是整 App 性能承诺。机器、构建、fixture 或场景
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
