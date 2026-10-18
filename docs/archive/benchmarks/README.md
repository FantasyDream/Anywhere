# 历史基准样本

这些数据仅对采样时的构建、硬件与 fixture 有效。当前策略见 [性能](../../performance.md) 和 [文件搜索](../../file-search.md)。

| 样本 | 边界 |
|---|---|
| [R2 SQLite 排序对照，2026-10-18](open-source-result-store-r2-2026-10-18.json) | 旧原型位于提交 `9d14251`；当前已移除，保留原始顺序 / 耗时 / RSS / 磁盘对照 |
| [R3 匹配对照，2026-10-18](open-source-matcher-r3-2026-10-18.json) | 旧原型位于提交 `9d14251`；采用原生流式比较，nucleo 未接入生产且依赖已清理 |
| [长期索引原型，2026-10-10](file-search-index-prototype-2026-10-10.json) | 已被取代的实验，不代表生产路径；status 与 hardware 保留在原始 JSON |
| [V0.9 整 App，2026-10-16](v0.9-app-2026-10-16.json) | Build 6 主进程 RSS 快照、可见 / 隐藏 vmmap 与原生可见事件，非长期或端到端帧耗时保证 |
| [V0.9 遍历器，2026-10-16](v0.9-file-search-2026-10-16.json) | 0.9 收尾源码，10k / 40k 刚创建文件，3 次采样；独立 Rust release 进程，不含 Spotlight / Flutter |
| [V0.9 CPU，2026-10-16](v0.9-cpu-2026-10-16.txt) | 1000 个合成应用，100 次预热 / 1000 次测量；不含窗口与端到端输入延迟 |
| [V0.10 剪贴板存储，2026-10-16](v0.10-clipboard-2026-10-16.json) | 1,000 条约 8 KiB 合成文本；独立 Rust Release，摘要检索、单条正文及峰值 RSS，不含 Flutter / 系统剪贴板 |
| [V0.10 多格式 schema，2026-10-16](v0.10-clipboard-multiformat-2026-10-16.json) | Build 5 schema v2；同样 1,000 条约 8 KiB 合成文本，独立 Rust Release；不含原生采集、图片解码与 Flutter |
| [V2 遍历器，2026-10-10](file-search-v2-2026-10-10.json) | 刚创建的目录、OS cache 未清；只读 Rust 独立进程，不含 Spotlight、Flutter 或 GUI |

2026-10-10 样本整理时仅移动原始文件，没有修改数值；2026-10-16 样本为 V0.9 收尾重新运行。内部批次 / 游标采样不是用户分页交互，也不反映后来的 Delta、共享分块及固定预览成本。
