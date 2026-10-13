# 文件搜索可复用库补充调研

日期：2026-10-12。范围：官方仓库、API 文档及关键源码检查；未安装或执行候选库，没有同机基准，性能判断为待验证的选型建议。生产搜索代码未变更。

## 结论

现有约束是可读取允许范围不遗漏、低内存、不维护长期索引、连续列表。对此优先评估 **zlob 的回调遍历接口**；如果优先降低构建和维护成本，则评估 **ignore**。它们替换的是文件枚举部分，继续复用当前 Unicode 匹配、临时记录、结果加载、执行复验和 Launcher UI。

若接受常驻目录索引，**fff-search** 是比整合独立搜索应用更直接的 SDK 候选，尤其适合当前项目目录反复搜索；它的默认覆盖策略不能直接用于不遗漏的 Home 搜索。尚未发现有证据证明某个现成库同时满足所有要求、可以无适配替换现有全链路。

## 候选比较

| 候选 | 类型 | 可以复用什么 | 需要注意 |
|---|---|---|---|
| [zlob](https://github.com/dmtrKovalenko/zlob) | Zig 实现，Rust / C API，无长期索引 | 并行目录遍历、回调输出、按需批量元数据、取消 / 跳过目录 | 构建增加 Zig / bindgen；默认忽略隐藏及 ignore 文件；目录工作队列在内存中；错误反馈需适配 |
| [ignore](https://docs.rs/ignore/0.4.33/ignore/struct.WalkBuilder.html) | Rust，无长期索引 | 多根、串行 / 并行遍历、过滤、错误结果、线程数配置 | 必须关闭默认 hidden / gitignore / ignore 等规则；不承诺内部队列恒定内存或比当前实现更快 |
| [walkdir](https://docs.rs/walkdir/2.5.0/walkdir/struct.WalkDir.html) | Rust，串行递归迭代器 | 简单遍历、保留错误、路径与链接处理 | 适合作为基线；`max_open` 达到限制后会缓存未返回条目，不能等同于硬内存上限 |
| [jwalk](https://docs.rs/jwalk/0.9.0/jwalk/) | Rust / Rayon，并行迭代器 | 按目录并行，回调过滤，控制线程池 | 默认隐藏过滤需关闭；API 的目录回调提供条目集合，宽目录 / 预读内存需实测 |
| [fff-search](https://docs.rs/fff-search/latest/fff_search/) | Rust 文件搜索 SDK，目录索引驻留内存 | 模糊匹配、文件 / 目录结果、内部批次、watcher、可选内容搜索 | 更适合项目目录；默认过滤和被忽略的读取错误不满足当前覆盖契约；全范围索引占用需测量 |
| [Cardinal 组件](https://github.com/cardisoft/cardinal) | macOS Rust 应用拆分组件 | FSEvents 包装、目录树与名称存储等 | `cardinal-sdk` 本身主要导出事件接口，不能当作完整搜索引擎；`fswalk` 构建目录树，并吞掉部分读取错误 |
| [nucleo-matcher](https://docs.rs/nucleo-matcher/latest/nucleo_matcher/) | Rust 匹配 / 评分库 | 已有候选的模糊评分 | 不负责发现文件、索引、权限、完整性；只可改善匹配体验 |

## 1. zlob：最值得做枚举原型

检查提交 `cfe0e7bc58a5834ca35205fc3747ca024a34c382`，Rust crate 1.6.5、MIT。

- [README](https://github.com/dmtrKovalenko/zlob/blob/cfe0e7bc58a5834ca35205fc3747ca024a34c382/README.md) 说明支持 Linux / macOS / Windows 原生目录读取，macOS 请求元数据时使用 `getattrlistbulk`。作者基准不能证明本机或不带元数据的名称搜索收益。
- [Rust Walker](https://github.com/dmtrKovalenko/zlob/blob/cfe0e7bc58a5834ca35205fc3747ca024a34c382/rust/src/walk.rs) 有 `run` / `run_serial`，不用 `collect` 即可在回调中消费条目；`WalkState::Quit` 可取消，`SkipDir` 可实现用户排除目录。原始路径身份和 Unicode 匹配继续由 Anywhere 处理。
- `RECOMMENDED` 默认开启 `GITIGNORE | SKIP_HIDDEN`，必须显式调整。默认跳过不可读目录；Rust 构建出的底层 options 未安装错误回调。至少开启 `ABORT_ON_ERROR` 并返回 Partial；若需继续其他目录且汇总错误，需补齐错误回调适配。
- [工作队列](https://github.com/dmtrKovalenko/zlob/blob/cfe0e7bc58a5834ca35205fc3747ca024a34c382/src/walker/queue.zig) 保存目录任务于 ArrayList；回调只避免全量结果列表，不等于内部任务队列也有硬上限。不能把它的 RSS 直接视为当前磁盘 FIFO 的等价替换。
- [build.rs](https://github.com/dmtrKovalenko/zlob/blob/cfe0e7bc58a5834ca35205fc3747ca024a34c382/rust/build.rs) 使用 Zig 和 bindgen；回调不能 panic 穿越 FFI。接入需控制线程数、处理异常 / 取消，并验证打包工具链。

因此建议先做隔离性能 / 集合原型，不能直接照 README 示例替换生产遍历器。

## 2. ignore / walkdir / jwalk：纯 Rust 备选

`ignore::WalkBuilder` 官方文档明确标准过滤默认启用，`standard_filters(false)` 能关闭 hidden、parents、ignore、git_ignore、git_global、git_exclude。对本项目应关闭这些默认规则，仅应用用户设置，并保留每个错误为 Partial；不能使用 `filter_map(Result::ok)` 吞掉错误。[API 文档](https://docs.rs/ignore/0.4.33/ignore/struct.WalkBuilder.html)

`walkdir` 提供深度优先迭代，错误会作为结果返回；但 FD 限制触发时把未消费条目保存到内存。[资源行为](https://docs.rs/walkdir/2.5.0/walkdir/struct.WalkDir.html#method.max_open)

`jwalk` 可以配置并行度，并在每目录回调处理条目；默认 `skip_hidden(true)`，需要关闭。并行库不代表所有文件树都更快，也不保证宽目录峰值符合现有 FIFO 方案。[API 文档](https://docs.rs/jwalk/0.9.0/jwalk/struct.WalkDirGeneric.html)

选型建议：ignore 用作可维护性 / 多根并行方案，walkdir 用作简单对照，jwalk 作为性能对照。三者均不负责 Launcher 排名、连续列表和执行安全。

## 3. fff-search：更完整，适合项目目录

检查提交 `d978416f77aec3f9928d56e00c9e56ba39ca1fda`。源码目录为 `crates/fff-core`，包名仍为 `fff-search`，0.11.0 / MIT，避免只依据网页旧目录名。

- [Cargo.toml](https://github.com/dmtrKovalenko/fff/blob/d978416f77aec3f9928d56e00c9e56ba39ca1fda/crates/fff-core/Cargo.toml) 默认使用纯 Rust ignore / globset；zlob 为可选 feature，因此可先不引入 Zig。支持跨平台的实现路径。
- [FilePicker](https://github.com/dmtrKovalenko/fff/blob/d978416f77aec3f9928d56e00c9e56ba39ca1fda/crates/fff-core/src/file_picker.rs) 可选择 base_path，关闭内容索引、内容 mmap 缓存和 watcher；与前次 FSearch 的整盘 / 内容索引耦合相比，更容易做纯名称原型。但是 FileItem / 目录 / 路径索引仍然驻留；关闭内容缓存不等于消除名称索引内存。
- 当前 [遍历](https://github.com/dmtrKovalenko/fff/blob/d978416f77aec3f9928d56e00c9e56ba39ca1fda/crates/fff-core/src/walk/ripgrep.rs) 尊重 gitignore / ignore / 全局排除，非 git 根开启隐藏过滤，并对读取错误直接 Continue。这些是项目搜索策略，不能照搬成 Home 范围完整搜索。
- 有 offset / limit 结果接口，可内部适配连续滚动，但 watcher 更新时仍需稳定代次和行身份，不把分页参数暴露到 UI。
- 作者 README 报告 14k 文件约 26 MB resident、500k 文件数百 MB；包含的配置 / 数据与纯名称原型不同，本轮没有验证这些数字。不能凭 SDK 低延迟宣传判断更省内存。[项目说明](https://github.com/dmtrKovalenko/fff/blob/d978416f77aec3f9928d56e00c9e56ba39ca1fda/README.md)

## 4. 为什么暂不选择完整应用 / 通用全文引擎

- Cardinal 的 [cardinal-sdk 导出](https://github.com/cardisoft/cardinal/blob/master/cardinal-sdk/src/lib.rs) 主要是 FsEvent、EventWatcher 等；[fswalk](https://github.com/cardisoft/cardinal/blob/master/fswalk/src/lib.rs) 的结果为 Node 树，缺失 / 不可读根也能返回空树，部分错误不单独汇总。需要较多改造才能满足目前错误和低内存契约。
- [Cling](https://github.com/FuzzyIdeas/Cling) 是完整 Swift 应用，官方 README 明确会维护范围内存索引，并有结果限制 / 质量过滤；适合借鉴交互与索引优化，不是现有链路的直接库替代。
- nucleo 只改善匹配，不扫描文件；不能用更好的 fuzzy matcher 解决未发现文件的问题。

## 下一步建议

仅对文件枚举层做同机对照：现有 Rust FIFO、ignore、zlob callback。使用相同隐藏 / 排除 / 链接契约，比对完整原始路径集合，再测首批耗时、完整遍历耗时、峰值 RSS、宽目录 / 深目录、取消、权限错误。zlob 先限制 1 / 2 / 4 个线程，不预设线程越多体验越好。

保留 Spotlight 快速候选、现有 Unicode 字面匹配、临时命中记录与连续列表。原型胜出才替换枚举器；如果需要项目级即时模糊搜索，再单独评估关闭内容功能的 fff-search，而不直接把全量 Home 文件常驻内存。
