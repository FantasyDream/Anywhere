# R2：SQLite 查询临时排序对照

日期：2026-10-18。分支：`codex/refactor-open-source`，生产基线 `cc6e228`。结论：**保留现有磁盘归并排序器，不把本轮 SQLite 原型接入生产。**

## 范围与实现

生产路径仍为 `filesystem/result_store.rs`。对照代码位于 `filesystem/result_store_probe/`，仅通过 `cfg(all(test, unix))` 编译；不增加运行时配置、公开 API 或 Cargo 依赖，不改变授权、遍历、匹配、Context、Scope、UI 或 15 秒恢复规则。

两者读取相同 `ScanStore` 原始记录流、复用实际 `NameQuery` / `RankedHit::compare` 规则和内部 64 条批次。SQLite 按查询创建、随 workspace 删除，不建长期文件索引：

- 原始路径以 BLOB 保留；路径排序按 Unix 的组件比较生成 key，避免直接按整个路径字节排序导致 `/a-b/report` 与 `/a/report` 次序错误。未做规范化或有损字符串去重。
- `score DESC, normalized, path_key` 索引管理未读行；每批仅取出有界结果，事务提交后移除已消费行。后到的高分记录参与未读尾部排序，不通过旧分数游标跳过。
- 建库每次事务最多 512 条 / 约 1 MiB 原始记录；只有成功提交才推进读取边界。取消或失败回滚当前事务，重试从上一次完整提交继续。
- `cache_size=-512`、`mmap_size=0`、`temp_store=FILE`；使用 DELETE journal 和 `synchronous=OFF`。与现有临时文件一样不要求断电持久化，但保留操作内失败的事务回滚。页缓存设置不是整个进程的内存硬上限。

Windows 路径排序没有在本原型中实现或验证，不能凭这组 Unix 测试直接替换跨平台生产后端。

## 正确性与失败回归

7 项正常单元测试均通过：

1. Unix 路径组件前缀、中文、emoji、任意非零路径字节与 `Path::cmp` 一致。
2. 17,001 条常规记录加 Exact / Prefix / Substring、规范化同名、非 UTF-8 父目录等记录，完整顺序与现有实现及排序参考一致。
3. 已展示的 provisional 前缀保持；后到 Exact 命中出现在未读尾部首位，完整返回且不重复。
4. 在后续写入事务中取消，以及消费部分结果后取消，再次读取不遗漏。
5. 用 SQLite `max_page_count` 触发真实 `SQLITE_FULL`，以及 `query_only` 触发只读写入失败；解除限制后可重试并完整返回。这不等于对所有 OS I/O 故障或进程崩溃完成验证。
6. 查询身份隔离、workspace 清理，以及 `EXPLAIN QUERY PLAN` 确认有序读取走索引。
7. 空数据、同批重复路径、已交付后再次收到同一原始路径均不重复交付。

## 可复现测量

```sh
python3 scripts/bench-result-store.py --output docs/benchmarks/result-store-r2.json
cargo build --release -p anywhere-core --example file_search_measure --locked
python3 scripts/bench-file-search.py --output docs/benchmarks/file-search-continuous-r2.json
```

macOS 27.0.1 / arm64，Release；每组各采样 3 次、两后端交替顺序、独立进程和 workspace；OS cache 不清。最终测量与编译 / 全量测试顺序执行。完整数据见 [排序对照](../benchmarks/result-store-r2.json) 和 [实际遍历 / 连续读取](../benchmarks/file-search-continuous-r2.json)。

排序对照不遍历真实目录，不包含 Spotlight、Flutter、Bridge 或 GUI；输入为相同合成原始路径。已有 7 条显示前缀，16 次提交场景在每次提交后读取一批，最后读完余下结果。所有样本的返回数量及完整显示顺序 SHA-256 均一致。

这里的“首批”是存储后端首次读取未读结果，包含 SQLite 打开 / 建表，不是 Launcher 输入到首屏时间；生产扫描器已发布的 top 64 不依赖该步骤。全量耗时包含建库 / 排序 / 持续读取，排除 fixture 写入及验证摘要。RSS 包含共用测试 harness 和已显示路径集合；临时文件大小在批次之间采样，**不是真实峰值**，不包括已删除的归并中间文件或短暂 journal。

下表均为三次样本中位数，单元格按“现有 / SQLite”排列：

| 记录数 / 提交次数 | 首次未读批次 ms | 全量 ms | 峰值进程 RSS MiB | 批间临时文件 MiB |
|---|---:|---:|---:|---:|
| 100 / 1 | 0.34 / 1.76 | 0.40 / 2.13 | 10.23 / 11.59 | 0.01 / 0.06 |
| 10,000 / 1 | 19.90 / 59.82 | 27.10 / 250.09 | 12.17 / 13.73 | 1.10 / 5.97 |
| 100,000 / 1 | 207.50 / 1384.60 | 284.16 / 3670.09 | 23.28 / 24.84 | 11.04 / 64.06 |
| 10,000 / 16 | 1.21 / 3.17 | 70.26 / 246.76 | 12.08 / 13.72 | 1.05 / 5.27 |
| 100,000 / 16 | 9.29 / 28.56 | 681.42 / 3283.06 | 23.34 / 24.86 | 10.99 / 59.32 |

独立空测试进程 RSS 单次基线：9.20 MiB。全部读取时已显示路径集合随结果数增长，SQLite 页缓存额度不能消除这部分共同开销。

协作取消测量在第 601 次检查请求取消，回调首次返回 true 至调用返回的中位耗时：现有 0.010 ms，SQLite 0.110 ms。它验证当前事务回滚 / 退出成本，不测异步排队等待，也不保证阻塞的 OS I/O 可在该时间内中断。

## 选型结论与边界

当前原型没有满足速度、低内存或临时磁盘收益门槛，不迁移生产。实验代码保留在测试模块中用于重现；没有加入一套可切换的生产引擎或维护长期数据库的配置。

SQLite 原型保留共同的原始输入文件，另存索引、规范化名称和路径 key；若将来尝试直接写入 SQLite、压缩 schema 或不同 journal 策略，需要重新测量并验证回滚。本轮结论只针对本实现和这些负载，不推导为 SQLite 不能实现快速搜索。

旧 `file_search_measure` 使用 `ScanStore.page`，旧文档中 10k 命中的 786–798 ms 属于该游标路径。该基准已改为产品实际使用的 `ScanStore.more`，新增不重复与完整数量检查；历史数据保留。**新连续读取数据是测量路径修正，不是本轮生产代码提速。**

完整 `scripts/check.sh`：490 Rust / 273 Flutter 通过，7 项 ignored（6 项既有，新增资源基准已用 Release 显式运行）。本轮没有产品 UI 改动，不另做 GUI 验收或打包；R1 Build 10 仍有效。下一阶段为 R3 应用 / Action 匹配对照。
