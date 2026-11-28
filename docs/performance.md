# 性能测量与证据边界

当前 GPUI 产品还没有形成可发表的整 App 性能基线。本页规定如何采集新样本、
如何解释现有诊断字段，以及哪些旧数据只能作为历史参考。

## 不能混用的指标

| 指标 | 能回答的问题 | 不能回答的问题 |
| --- | --- | --- |
| Core 查询耗时 | Rust 激活、匹配、排序或文件读取用了多久 | GPUI 是否已完成布局、栅格化或显示器呈现 |
| `hotkey_to_visible_ms` | 热键事件到展示调用经过多久 | 系统是否已经绘制首帧、输入法是否就绪 |
| 单次 RSS / footprint | 某一时刻进程保留多少页 | 是否存在泄漏或长期趋势 |
| WebView helper 内存 | 与本轮 WKWebView 或 WebView2 会话对应的子进程资源 | 全机 helper、GPU 或未来峰值 |
| 隐藏窗口资源 | 隐藏生命周期是否释放文档或对象 | 用户感知的显示延迟 |

性能结论必须说明样本尺寸、构建类型、硬件、操作系统版本、语言、缩放、数据量和操作
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

物理 IME 候选选择、预览首帧、WKWebView / WebView2 内容呈现和窗口合成按各平台
[macOS 桌面验收](test-acceptance-macos.md)或 [Windows 桌面验收](test-acceptance-windows.md)单独测量。

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

## 历史材料

逐构建数据见 [GPUI 2026 年 11 月测量记录](archive/benchmarks/gpui-2026-11.md)
与 [历史基准归档](archive/benchmarks/README.md)。引用样本时带上原构建和夹具。

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
