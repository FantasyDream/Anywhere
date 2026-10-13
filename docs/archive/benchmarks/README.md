# 历史基准样本

这些数据仅对采样时的构建、硬件与 fixture 有效。当前策略见 [性能](../../performance.md) 和 [文件搜索](../../file-search.md)。

| 样本 | 边界 |
|---|---|
| [长期索引原型，2026-10-10](file-search-index-prototype-2026-10-10.json) | 已被取代的实验，不代表生产路径；status 与 hardware 保留在原始 JSON |
| [V2 遍历器，2026-10-10](file-search-v2-2026-10-10.json) | 刚创建的目录、OS cache 未清；只读 Rust 独立进程，不含 Spotlight、Flutter 或 GUI |

整理时仅移动原始文件，没有修改数值或重新运行基准。内部批次 / 游标采样不是用户分页交互，也不反映后来的 Delta、共享分块及固定预览成本。
