# R3：应用 / Action 匹配对照

日期：2026-10-18。分支：`codex/refactor-open-source`；旧算法参考提交 `4198a40`。

结论：**保留原生匹配规则，移除每次候选比较中的临时 compact 字符串分配；不将 nucleo 或额外名称缓存接入生产，不增加模糊搜索模式。**

## 历史记录状态

本报告记录清理前的实验及原始数据。最终收尾已移除 nucleo dev-dependency、原型、冻结旧算法和专用 runner；需要重放时使用独立 checkout 的 `9d14251` 提交，不在当前工作树执行下方旧命令。生产流式比较、helper 回归与 `tests/search_fields.rs` 的产品规则回归继续保留。

## 实现与对照范围

生产改动仅在 `core/search/query.rs`：原先收集 ASCII 字母 / 数字到 String 再 `starts_with`，现在通过字节迭代器过滤并按查询需要比较，首次不同即返回。不增加常驻字段缓存，不改变 Query / SearchFields API、Context、Scope、候选身份、frecency、文件搜索或 15 秒恢复规则。

当时实验位于 `core/search/matcher_probe/`，由 `cfg(test)` 隔离；`nucleo-matcher = 0.3.1` 仅为 dev-dependency。`cargo tree --locked -p anywhere-bridge-flutter --edges normal` 验证生产依赖图没有 nucleo。原型中冻结一份旧字段算法用于差异测试，生产只有一个匹配器，无后端设置或重复运行路径。

| 对照项 | 内容 |
|---|---|
| legacy | 冻结的旧算法，比较时构建 compact String |
| existing | 当前生产算法，流式比较、不构建临时 compact String |
| prepared | 保留原生匹配，预存 compact 名称的控制组，用来区分缓存收益与库收益 |
| nucleo | 保留结构性匹配与等级，仅 ASCII 子串交给 nucleo；Unicode 回退原生 contains |
| fuzzy | 在上述规则未命中时，对同一可见字段的所有 token 做非连续匹配；仅记录体验差异 |

前三个实验组及 nucleo 兼容组保留 Exact / Prefix / WordPrefix / Substring 等级、单个字段内每个 token 均满足、metadata 只允许整体 Exact / Prefix 等规则。库数值分数不进入现有 Ranking，使用历史仍由原 ranker 处理。

## 两个不能直接替换的行为差异

1. **Unicode 子串不等价。** 在本次固定版本、关闭额外大小写 / 拉丁字母归一化、显式保留码点的配置下，`Cafe\u{301}` 的 `afe\u{301}` 后缀会被原始 `substring_match` 漏掉，Rust contains 可以命中。直接调用及兼容回退都有回归测试。这是本配置的可复现场景，不推广为所有版本或 API 的结论。
2. **缩写也会带来噪声。** `gchr` 命中 Google Chrome 有帮助，但直接 fuzzy 也让 `co` 命中 Google Chrome，违反现有短词搜索约束。原型不模糊匹配 metadata，也不把不同 alias 拼起来满足多个 token；仍不足以据此批准新增模糊搜索产品规则。

API 核对使用官方 [Matcher 0.3.1](https://docs.rs/nucleo-matcher/0.3.1/nucleo_matcher/struct.Matcher.html)、[Config](https://docs.rs/nucleo-matcher/0.3.1/nucleo_matcher/struct.Config.html) 与 [Utf32Str](https://docs.rs/nucleo-matcher/0.3.1/nucleo_matcher/enum.Utf32Str.html)。matcher 在循环外复用；禁用额外字符变换，显式构造码点序列，查询按字面值处理，不引入 pattern DSL。

## 正确性回归

- 新旧 compact 前缀逐项比较，包括分隔符、纯中文、混合文字、组合重音、emoji、空输入。
- 真实注册 Action 字段及合成应用 / 菜单字段，对中英文、多词、alias、标识符、缺失 token、空字段、长输入比较候选与 MatchLevel。
- 枚举部分 Unicode 名称的所有连续子串，包括孤立组合标记、ZWJ、大小写展开和字面标点；兼容方案与当前 / 冻结旧算法一致。
- 回归原始库差异、短词噪声、metadata 禁止模糊命中、禁止跨 alias 拼 token，以及 tier 与 frecency 的所有权。

新增 8 项功能回归通过（7 项原型 / 差异回归，1 项生产 helper 回归）。完整 `scripts/check.sh` 的结果见 [阶段记录](../open-source-refactor-plan.md)。

## 测量方法与结果

```sh
python3 scripts/bench-matcher.py --output docs/benchmarks/matcher-r3.json
```

macOS 27.0.1 / arm64、Release；每个后端每组独立运行 3 个进程，交替顺序。12 组查询各预热 30 次、测量 200 次；匹配时间包含 Query 解析与结果计数，不包含数据准备、Context 解析、授权、构造完整 Candidate、Ranking 排序、Bridge、Flutter 或 GUI。

1,000 / 10,000 个应用为合成名称与 alias；2,000 个 Action 字段为真实注册字段的重复采样，不表示软件有 2,000 个不同内置动作。SHA-256 在计时外记录每个匹配行索引与 MatchLevel；legacy / existing / prepared / nucleo 的所有样本候选集合与等级相同。fuzzy 单独记录新增结果，不以更多命中自动判为更好。

数值按各次进程的查询 p95 取中位数，再求 12 组查询的均值；单位 µs：

| 语料 | 旧算法 | 当前流式 | 名称缓存 | nucleo 兼容 | fuzzy 扩展 |
|---|---:|---:|---:|---:|---:|
| apps / 1,000 | 116.50 | 57.55 | 56.84 | 64.91 | 75.68 |
| apps / 10,000 | 1169.05 | 588.72 | 579.96 | 673.10 | 774.20 |
| actions / 2,000 | 396.90 | 215.57 | 212.23 | 230.42 | 269.77 |

1,000 个应用的独立进程峰值 RSS 中位数：旧算法 10.30 MiB，当前 10.27 MiB，名称缓存 10.52 MiB，nucleo 兼容 10.67 MiB。RSS 是测试进程指标，几十 KiB 的差异不作为整体内存下降结论；采用流式方案的内存依据是没有新增常驻缓存，代码路径也不再分配 compact String。

完整环境、准备耗时、每个查询的 p50 / p95、候选数、匹配等级摘要及进程峰值 RSS 见 [原始记录](../archive/benchmarks/open-source-matcher-r3-2026-10-18.json)。表格的“各查询 p95 均值”不是整个 Launcher 的 p95，也不表示整 App 提速相同比例。

## 收敛决定

- 采用流式 compact 前缀比较，避免热路径分配与全字段拷贝；不采用额外常驻缓存。
- nucleo 兼容组需要 Unicode 回退，当前又未优于无缓存的原生实现，因此不增加生产适配层。
- fuzzy 的短查询噪声没有完成产品规则设计，因此未接入；原始样本保留，当前短词边界由产品回归保障。
- 当时 R2 / R3 共用 `scripts/bench_probe.py` 的构建、隔离进程和 RSS 采样工具；R2 两个后端曾用 100 条记录验证共享 runner 的返回顺序一致。
- 最终清理移除测试原型、冻结旧算法和专用脚本，保留原始数据及不依赖原型的产品回归；历史代码从 `9d14251` 获取。生产不维护备用匹配后端。
