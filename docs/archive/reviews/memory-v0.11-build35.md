# V0.11 Build 35 内存优化

日期：2026-10-20。本轮保留 Impeller、玻璃背景、隐藏后 15 秒状态保留和已有搜索 / 预览预算。

## 观察与修改

- 正在运行的 Build 34（PID 85027）隐藏状态采样：`vmmap -summary` 显示 physical footprint 约 107.3 MiB、峰值 206.6 MiB；其中 owned unmapped (graphics) resident 约 45.5 MiB。这是单次样本，进程已使用过 WebView 且系统已锁定，不是干净启动基准。不能只看较低的 RSS，或把虚拟地址保留当作实际占用。其他 App 也使用 WebKit，不能将所有 GPU / WebContent 进程相加归给 Anywhere。
- 剪贴板 `features/clipboard/content.rs::metadata` 原先在 payload 之外持有一份完整纯文本，再生成完整搜索字符串和小写字符串。现在通过 Cow 借用 payload 内的纯文本；ASCII 索引就地转小写，Unicode 保留 `str::to_lowercase` 的完整语义，不改为逐字符转小写。不截断搜索内容、改变摘要上限、改变存储格式或降低捕获限额。
- Web 文档策略直接返回 UTF-8 编码后的 Uint8List，CustomSchemeResponse 复用该数组；移除 HTML 加载时再做 Uint8List.fromList 的整份字节复制。CSP、宿主主题注入位置、UTF-8 校验及页面生命周期不变。跨 FRB / native / WebKit 的其他复制仍存在，本轮不宣称零拷贝传输。

## 合成测量

在同一机器、同一 release 编译配置下，独立进程直接调用生产 metadata 函数，传入 16,777,210 字节 ASCII 合成文本（A 重复并以 TailNeedle 结尾）；不读取系统剪贴板或用户数据库，不打开 Flutter。输入与输出在测量期间保持存活。优化前后各运行三次 `/usr/bin/time -l`，每组结果一致：

| 指标 | 优化前 | 优化后 |
|---|---:|---:|
| peak memory footprint | 69,288,512 B / 66.1 MiB | 35,652,112 B / 34.0 MiB |
| maximum resident set size | 75,857,920 B / 72.3 MiB | 42,221,568 B / 40.3 MiB |
| 搜索索引字节数 | 16,777,240 | 16,777,240 |
| 摘要字符数 | 120 | 120 |

物理足迹峰值减少约 32.1 MiB。这是极大 ASCII 文本的派生阶段收益，**不是 App 待机减少 32 MiB，也不代表 Unicode / 富文本 / 图片获得同等降幅**。未据短任务 wall clock 比较宣称启动或吞吐提升。

原始日志：`build/clipboard-memory-before.log`、`build/clipboard-memory-after.log`，编译日志为 `build/clipboard-memory-probe-build-{before,after}.log`。临时探针从 Cargo examples 删除，仅在 ignored build/ 保留测量源码与二进制；未增加生产计时、分配器或常驻采样器。

## 验证及限制

- Rust clipboard 相关 41 项通过；新增覆盖希腊词尾 Σ、İ 的 Unicode 大小写语义、中文、额外格式名称及摘要之外的正文尾部可搜索。
- Flutter 相关 48 项通过；新增验证 typed bytes 保留中文 / emoji / 重音字符、仍拒绝非法 UTF-8；既有 CSP、外观、剪贴板、悬浮窗与图标缓存回归保持通过。
- Flutter analyze 与 Core all-targets Clippy 无问题。日志：`build/clipboard-memory-tests.log`、`build/memory-flutter-tests.log`、`build/memory-flutter-analyze.log`、`build/memory-clippy35.log`。
- 已构建签名 App；打包日志为 `build/memory-build35.log`、`build/memory-package35.log`。
- 未进行新旧完整 App 同场景内存对比、长期泄漏或 GUI 回归。上一轮 WebView 冷开空白仍是未通过项，本轮没有修复或重新判定其状态。

现有图标 2 MiB 编码缓存、8 MiB Flutter 解码缓存、512 像素剪贴板缩略图、预览串行准入、Web 资源并发限制、隐藏后过期清理及悬浮窗关闭后销毁独立引擎都已存在。本轮没有通过继续压缩这些预算牺牲交互；更大的待机降幅需单独测量引擎 / 图形资源寿命与唤醒延迟的取舍。
