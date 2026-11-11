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

## 历史材料

过去的界面 era 样本和逐构建 JSON 位于
[历史基准归档](archive/benchmarks/README.md)。其中的系统、构建、框架和指标
定义与当前 GPUI 产品不同，只用于解释当时的回归，不构成当前版本基线，数字和
结论也不应复制到发布说明。

## 回归门槛

当前门禁优先检查确定性、取消和资源所有权：

- 旧 Session / revision / snapshot 不覆盖新状态；
- 查询切换后旧结果不会继续追加；
- 隐藏立即撤销执行资格；
- Quick Look generation 防止旧关闭回调影响新预览；
- Web document 替换、teardown 和迟到 IPC 不污染新 handle；
- 状态栏、主窗口和 Shelf 复用同一清理路径。

出现疑似泄漏时，先重复操作序列并用 footprint 趋势与对象 / helper 归属比较，
再决定是否修改缓存或渲染路径。不能用关闭一次窗口后的单点下降证明资源已释放。
