# FSearch 替换评估

日期：2026-10-11。方法：官方仓库 / 文档查询、固定提交源码检查，与 Anywhere V2 生产链路对照。没有安装、启动外部索引服务或执行全盘索引，没有同机速度 / RSS 对比实验。本次只新增调研文档，不替换生产引擎。

## 结论

技术上可以接入，但不建议立即全量替换。

- 经典 C/GTK FSearch：不适合直接嵌入当前 Rust / Flutter Launcher。
- 其 macOS fork `NomaDamas/fsearch-mac`：CLI 可以接入，但全量内存索引、依赖和维护成本不符合现阶段优先级。
- 独立 Rust 项目 `noahdunnagan/fsearch`：值得做隔离原型；它有库 / JSON 接口、mmap 名称索引和 FSEvents 更新。但是替换意味着接受长期索引，并需要改造名称查询、范围、结果追加和完整性反馈。

若仍要求无长期索引且搜索期间稳定、可读范围不遗漏，保留当前完整遍历作为依据。若更重视百万级目录每次输入的即时响应，并接受长期索引，则 Rust FSearch 路线有价值。两种方案的资源和一致性成本不同，不能用一项查询延迟宣传数字证明整体更合适。

## 1. 同名项目

| 项目 | 检查提交 | 技术 / 接口 | 判断 |
|---|---|---|---|
| [cboxdoerfer/fsearch](https://github.com/cboxdoerfer/fsearch) | `cb6cd67f9ef12fedcf6a160b6a38faf492f7c989` | C、GTK3、GLib、PCRE2、ICU；经典 Everything 风格应用 | 可以借鉴，不能直接作为现有 Rust 搜索后端依赖 |
| [NomaDamas/fsearch-mac](https://github.com/NomaDamas/fsearch-mac/tree/db14e8a46bddbc1012e35ae563d9d76bd8ca2495) | `db14e8a46bddbc1012e35ae563d9d76bd8ca2495` | 经典项目 macOS fork，FSEvents + CLI + Unix socket | 进程协议可用，资源成本需谨慎 |
| [noahdunnagan/fsearch](https://github.com/noahdunnagan/fsearch/tree/ffcb37d9297e716fcfdcef3169d752fcf18e3b1b) | `ffcb37d9297e716fcfdcef3169d752fcf18e3b1b` | 独立 Rust / MIT 项目，库 API + JSON socket / stdio | 最值得验证的候选，当前仍需适配 |

经典项目及其 fork 为 GPL-2.0-or-later，Anywhere workspace 声明 MIT；直接复用代码 / 链接的授权兼容需要另行评估，不把分进程通信自动当作授权问题已经解决。Rust 项目的 [Cargo.toml](https://github.com/noahdunnagan/fsearch/blob/ffcb37d9297e716fcfdcef3169d752fcf18e3b1b/Cargo.toml) 声明 MIT、版本 0.1.0。

## 2. 为什么快，代价是什么

FSearch 的关键收益是索引建立后查询不再重复读取目录；并不复用 Spotlight。macOS fork 更新自己的数据库；Rust 项目用 `getattrlistbulk` 批量枚举、mmap 名称索引、名称去重与查询预过滤，FSEvents 维护变化。这一架构能减少当前每次新查询重复遍历的 I/O，具体性能收益仍需同机测试。

### 官方数字，仅作为作者报告

- macOS fork：[README](https://github.com/NomaDamas/fsearch-mac/blob/db14e8a46bddbc1012e35ae563d9d76bd8ca2495/README.md) 报告约 1800 万文件 / 260 万目录，首次扫描约 7 分钟、数据库约 1 GB、扫描内存约 4 GB；启用开发产物排除后仍为约 830 万文件、482 MB 数据库、2.2 GB 扫描内存。其排除会改变覆盖范围，不能用排除后的更小资源数满足当前全范围契约。
- Rust 项目：[固定提交 README](https://github.com/noahdunnagan/fsearch/blob/ffcb37d9297e716fcfdcef3169d752fcf18e3b1b/README.md) 报告 M4 Max、830 万文件，名称查询中位数 0.13 ms，首次完全可搜索 49 s、峰值内存 0.9 GB；Chromium 内容索引磁盘量 0.76 GB。不同指标涉及不同测试范围；不是 Anywhere 本机测量，也不是纯名称索引的独立资源报告。

Rust mmap 数据可由 OS 回收，不等于无内存占用。其 `walk::scan` 返回 `Vec<Listing>`，启动构建收集目录 / 条目；现有 `Engine` 还自动启动内容索引，不能假设直接依赖后自然只有很小的名称搜索开销。

## 3. 源码可确认的集成问题

### macOS fork

- [数据库加载](https://github.com/NomaDamas/fsearch-mac/blob/db14e8a46bddbc1012e35ae563d9d76bd8ca2495/src/fsearch_database_file.c#L1332) 为全部目录和文件分配数组，加载条目并恢复父关系 / 排序数组。长期磁盘索引并不意味着只按需读取少量候选到内存。
- [CLI](https://github.com/NomaDamas/fsearch-mac/blob/db14e8a46bddbc1012e35ae563d9d76bd8ca2495/src/fsearch_cli.c#L242) 默认结果限制 100，`--limit 0` 可输出全部；`run_search` 将输出累计在 `GString`。直接请求全部宽查询结果会增加输出内存；没有与现有连续列表对应的稳定分批游标契约。
- [构建](https://github.com/NomaDamas/fsearch-mac/blob/db14e8a46bddbc1012e35ae563d9d76bd8ca2495/src/meson.build) 中 CLI 与 GUI 链接同一 `libfsearch`，依赖 GTK3 等；当前 fork 不能凭 headless 名称推断可删除 GUI 依赖。

### Rust 项目

- [Engine](https://github.com/noahdunnagan/fsearch/blob/ffcb37d9297e716fcfdcef3169d752fcf18e3b1b/src/engine.rs#L36) 的 `Options` 只有 index dir、home、skip；`full_build` 使用 `/`，owner 初始化后启动内容索引。没有现成的名称索引开关或与 Anywhere 多根设置等价的建库范围接口。
- [walk](https://github.com/noahdunnagan/fsearch/blob/ffcb37d9297e716fcfdcef3169d752fcf18e3b1b/src/walk.rs#L79) 使用 macOS 属性 API，收集 `Vec<Listing>`；mount point 不递归。`gated` 默认可跳过 Documents / Desktop / Downloads / 云目录及 `/Volumes`，取决于权限；不应把 ready 直接映射成当前范围 Complete。
- `Engine::search` 返回一次性的 `Vec<Found>`；[Query](https://github.com/noahdunnagan/fsearch/blob/ffcb37d9297e716fcfdcef3169d752fcf18e3b1b/src/query.rs#L294) 默认 limit 50，内部 TopK 丢弃限制之外的项。增加 limit 并重新查询能拿更多，但还需固定索引代次、去重及顺序保证；不是现有稳定追加的直接替代。
- 名称查询默认 fuzzy / typo tolerance；`fold` 和 token 构造使用 ASCII 字节折叠，不能宣称已覆盖 Anywhere 的 NFC + full case fold 契约。需要中文、组合字符、Unicode 大小写等独立验收。
- 原生枚举 / FSEvents / 内存释放等调用属于 macOS 实现。Rust 语言本身不代表这份引擎可以原样用于 Windows。

这些是当前固定版本的接口 / 实现差异，并非断言该项目不能改造或索引一定会漏文件。

## 4. 与当前 V2 比较

| 维度 | 当前 V2 | FSearch 索引路线 |
|---|---|---|
| 首次使用 | 无独立建库；每次输入稳定后扫描 | 首次建库，随后查询快 |
| 重复搜索 | 重复遍历，范围越大成本越明显 | 主要查索引，减少重复 I/O |
| 内存 | 遍历缓冲有界，已浏览候选随数量增长 | 常驻索引 / mmap、变化 overlay、首次建库峰值 |
| 磁盘 | 查询期间的临时队列与命中记录 | 长期索引及其更新 / 恢复 |
| 完整性 | 稳定可读范围遍历完成后确认；错误 Partial | 需要范围覆盖、索引代次、事件追平、失败 / 离线反馈 |
| 维护 | 查询取消、临时清理、执行复验 | 还增加迁移、重建、事件丢失恢复、后台生命周期 |

当前链路和边界见 [V2 文档](file-search-engine-v2.md)。当前追加会再次读取命中记录，读取成本随命中量 / 已浏览批次数增长；索引后端能改善查询阶段，但当前候选 / bridge 累积的成本不会因替换枚举器自动消失。

## 5. 推荐推进方式

### 当前优先：保留无长期索引契约

借鉴 `getattrlistbulk` 批量目录读取，把它做成 `anywhere-platform` 的 macOS 枚举适配器，继续输出有界批次给 Rust 匹配 / 临时记录层。保留便携遍历 fallback，并在同机测试后决定是否采用。这样可减少部分属性调用，保留 UI / Scope / Actions / Preview 和错误覆盖契约；加速幅度尚未测量。

### 如果接受长期索引：验证 Rust FSearch

1. 在隔离 fixture 中做名称搜索原型，不默认全盘、不开内容索引、不创建登录服务。现成 Options 不满足这些条件，先改造后再运行。
2. 将引擎藏在现有 `FileSearchSource` 后端适配层：返回批次、query/index generation、范围覆盖、错误和取消信号；继续复用当前 Candidate / Ranking / 执行复验。
3. 保持连续列表：稳定快照 / 排序身份 + 内部追加，禁止用前 50 / 100 项代表全部。
4. 索引尚未就绪、历史断档、范围不覆盖或目录读取失败时反馈 Partial / Updating；保留遍历验证或补查，直到证明完整性契约。索引就绪本身不证明已包含全部可读取对象。
5. 比较相同范围的首次名称建库峰值、查询 p50 / p95、空闲 / 搜索 RSS、磁盘、事件追平；分别测 1 万 / 10 万 / 百万条目和中文名称。
6. 集合验收：隐藏 / gitignore / 依赖目录、多根 / 外接卷、权限错误、创建 / 改名 / 删除、停机后变化、事件丢失、宽查询结果超过首批、Unicode、执行前路径变化。

满足这些后再决定替换。现阶段最合理的候选是改造后的 Rust FSearch，而不是直接集成经典 GTK 引擎。
