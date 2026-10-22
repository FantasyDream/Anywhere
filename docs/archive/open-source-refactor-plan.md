> 历史记录：保留当时版本与验收边界，当前行为以[文档导航](../README.md)为准。

# 开源复用重构路线

分支：`codex/refactor-open-source`。开始：2026-10-18。基线为 V0.10 收尾提交 `dbf04bb`，已推送 `codex/dev-0.10`；V0.9 已收尾并在 `dev-0.9`。选型依据见 [库清单](../research/open-source-reuse-audit.md)。

目标：减少通用基础设施维护，在不改变 Context / Scope、稳定列表、授权 / 执行边界、存储身份及 15 秒恢复规则的前提下复用成熟库。每阶段先实现和单元门禁，最后做必要的 UI 验证；原型优于现有实现才进入生产。

| 阶段 | 范围 | 验收与后续入口 |
|---|---|---|
| R1 | encoding_rs / chardetng 文本预览；which 工具查找 | **已完成**，见下方 R1 验收。编码预览与工具查找已接入生产 |
| R2 | SQLite 查询临时记录 / 外部排序原型 | **对照已完成，保留现有生产排序器。** 原型通过正确性验证，但本机速度、RSS 和临时磁盘均无收益，见 [R2 记录](../research/result-store-r2.md)。实验源码已清理，历史提交与测量数据保留 |
| R3 | nucleo-matcher 匹配原型 | **对照已完成**，见 [R3 记录](../research/matcher-r3.md)。采用无缓存的原生流式比较，移除热路径临时字符串分配；不接入 nucleo 或非连续模糊搜索，生产匹配集合与等级保持 |
| R4 | 根据原型结果收敛 | **已完成本轮收敛**：R1 库进入生产，R2 保留原排序器，R3 采用无缓存原生优化；随后已移除旧匹配参考、未采用原型及专用依赖。完整门禁、Build 11 打包及下方界面回归通过 |

## R1 行为

- 文件仍通过同一资源验证和有界读取，Flutter / Bridge 接口不变。
- UTF-8 优先；UTF-8 BOM 去除，UTF-16LE / BE 仅凭 BOM 明确识别；UTF-32 不做错误的 UTF-16 解释。
- 源码及常见 txt / md / csv / tsv / log / rtf / svg 名称在 UTF-8 不成立时允许传统编码推测；界面明确显示 guessed 提示，不宣称自动检测永远正确。
- 未知类型仍按 UTF-8 或明确 BOM 预览；二进制控制符、解码错误和损坏的 BOM 内容不替换为乱码继续显示。
- 读取截断允许尾部未完成编码单元，解码输出独立限额，避免 UTF-16 / GBK 转 UTF-8 后扩大预览体积。
- `which` 仅检查经过原有 GUI 工具目录优先级生成的绝对候选，不读 shell 初始化文件、不通过拼接 PATH 丢失目录中的分隔符、不补工作目录或批处理后缀。
- 依赖固定由 Cargo.lock 记录，包内包含新依赖许可文件；无任意升级 Flutter SDK、热加载框架或新业务 DSL。

## 边界

R1 不包含 SQLite / nucleo 的生产替换，不能据此声明文件搜索更快或整 App 内存更低；R2 / R3 的最终结果与清理见下文。现有 V0.10 VoiceOver / 首次权限 / Windows 实机 / 长时间内存的待验收边界继续保留。


## R1 验收记录（2026-10-18）

- 完整 `scripts/check.sh`：483 Rust / 273 Flutter 通过，6 项既有 Rust ignored；原生、fmt、Clippy、架构、文档与 analyze 均通过，日志为本机 `build/validation/open-source-r1-check.log`。
- 新增回归：UTF-8 BOM、UTF-16LE / BE 与 emoji、GBK 推测、未知类型不猜测、损坏 BOM / 二进制拒绝、截断编码单元、解码扩容限额、可执行权限 / 查找顺序 / 字面路径与链接、祖先 wrapper。Windows 批处理排除有条件测试，本机没有执行 Windows 测试。
- Build 10：`build/Anywhere-open-source-r1-build10.app`，稳定签名校验通过。全量单元检查后，实机通过原 File Search Action 输入三个独立 fixture 的完整路径；UTF-16 的中文 / emoji 与明确编码提示、GBK 中文与 guessed 提示、二进制拒绝均正常。仅使用本机合成文件，没有修改剪贴板采集设置。
- 新分支上的 SQLite / nucleo 仍未接入生产，下一步从 R2 原型开始。Windows 实机、长期内存与此前辅助 engine VoiceOver 边界未在本轮补齐。

## R2 验收记录（2026-10-18）

- 复用现有 rusqlite，添加独立测试原型与 Release 基准，无生产 API / 依赖变更。7 项回归覆盖完整顺序、原始路径、冻结前缀、后到高分项、取消、SQLite 写满 / 只读错误、重复记录与查询隔离。
- 100 / 10,000 / 100,000 条记录，以及 16 次增量提交，每种后端各采样 3 次；全部返回数量与显示顺序摘要一致。当前 SQLite 方案的速度、RSS、临时磁盘均未达到替换门槛，因此不迁移生产。
- 修正旧文件基准：从旧 `ScanStore.page` 游标接口改为真实连续读取的 `ScanStore.more`。此前旧接口的 786–798 ms 不能代表当前连续读取路径，也不能把新测量解释为本轮生产提速。
- 完整 `scripts/check.sh`：490 Rust / 273 Flutter 通过，7 项 ignored（6 项既有，新增 1 项资源基准已单独显式运行）；原生、fmt、Clippy、架构、文档与 analyze 均通过。本机日志：`build/validation/open-source-r2-check.log`。
- 本轮不修改生产搜索或 UI，不另打包或打开真实用户文件；R1 Build 10 仍是当前应用包。后续入口为 R3 应用 / Action 匹配原型，继续采用有收益才接入的规则。

## R3 验收记录（2026-10-18）

- 对照冻结旧算法、生产流式比较、名称预处理控制组、nucleo 兼容组及 fuzzy 扩展组；1,000 / 10,000 个合成应用和 2,000 个注册 Action 字段，各三次进程采样。
- 兼容组完整候选集合 / 匹配等级摘要一致；Unicode 子串回退、短词噪声、各词 / 字段边界、frecency、长输入与流式前缀新增 8 项回归。
- 生产仅去除 compact 临时字符串分配，不增加名称缓存或匹配后端设置。nucleo 固定版本仅供测试使用，生产正常依赖图不含它，也不引入完整 picker。
- R2 / R3 共用构建及资源测量 runner，已回归 R2 两个后端的完整顺序。资源基准在全量门禁之外单独显式运行，计时不与编译 / 全量测试重叠。
- 完整 `scripts/check.sh`：498 Rust / 273 Flutter 通过，8 项 ignored（6 项既有，2 项资源基准已分别显式运行）；原生、fmt、Clippy、架构、文档与 analyze 均通过。本机日志：`build/validation/open-source-final-check.log`。

## R4 收敛与 Build 11（2026-10-18）

以下为 Build 11 构建时的验收，之后的源码 / 文件清理见末节。

- 生产保留 encoding_rs / chardetng / which；不增加生产 SQLite 搜索后端、nucleo、完整 picker、名称常驻缓存或搜索模式设置。原生匹配旧分配代码仅作为 `cfg(test)` 下的差异基线保留；原型无运行时入口，生产正常依赖图确认没有 nucleo。
- 无公共 Bridge / 插件协议变更，无额外 feature flag 或热加载机制。Rust 继续负责匹配，Flutter / Host 负责现有展示与平台边界。
- 版本 `0.10.0+11`；包为 `build/Anywhere-open-source-final-build11.app`、对应 `-macos-arm64.zip` 和 `.sha256`。构建采用原有持久签名；deep / strict、资源与 ZIP 内关键文件一致性均通过。旧 Build 10 已退出，验证进程为 Build 11，最终只保留它运行。
- 完成全部代码与单元门禁后，通过真实 Build 11 UI 验证：`co` 未误命中 Chrome、`visualstudioc` 命中 Visual Studio Code、粘贴 `visualstudiocode 不存在` 无应用候选、`file se` 命中 File Search 并能打开 / 输入、UTF-16 合成文件固定预览显示中文 / emoji 与明确编码提示、返回主页面保留原查询。
- 测试输入已清空；剪贴板历史采集保持关闭，未调整用户设置。Unicode 多词采用粘贴提交（工具会恢复原剪贴板）；不把自动化工具的 Unicode typeText / AX setter 当作真实 IME 验收。没有补做 Windows、首次权限、辅助 engine VoiceOver 或长期内存验收。
- 本机证据：`build/validation/open-source-final-build.log`、`open-source-final-package.log`、`open-source-final-live.json`、`open-source-production-dependencies.txt`。同机 CPU / RSS 原始记录随文档保存；不把字段基准解释为整 App 性能承诺。

## 最终源码与文件清理（2026-10-18）

- 删除未采用的 SQLite / nucleo 原型、冻结旧 matcher、三个专用测量脚本，以及更早的长期索引 Python 原型。移除 nucleo dev-dependency 和 lock 条目；当前工作树不保留实验后端、加载入口或备用匹配路径。
- 将有价值的行为检查整理为 `crates/anywhere-core/tests/search_fields.rs`：可见名称 / alias / metadata 边界、缺失词与短词、Unicode / 组合重音 / emoji 子串、注册 Action 全名精确匹配。现有流式前缀、生产外部排序、取消 / 失败重试、tier 与 frecency 回归继续保留。
- 原型专属测试随废弃实现移除；历史计数不是当前门禁数量。原始 R2 / R3 数据移到 `docs/archive/benchmarks/`，报告标注已归档；完整实验从独立 checkout 的 `9d14251` 回放，不要求当前项目继续维护实验依赖。
- 更新 README、路线图与 MVP 的收尾状态；`ScanStore.page` 仍用于生产扫描失败后的已提交结果恢复，未作为“旧接口”误删。
- 清理本机 25 个旧 App / ZIP / checksum，逻辑体积约 427 MiB；保留当前 Build 11、对应 ZIP / checksum、验证记录及剪贴板测试文件引用。构建缓存继续用于开发；不清空用户数据或历史数据库。
- 本次没有生产行为、UI、存储 schema、权限或版本变更，沿用已验收的 Build 11；最终门禁见下方记录。
- 最终 `scripts/check.sh` 全部通过：487 Rust / 273 Flutter，6 项既有 Rust ignored；原生、fmt、Clippy、架构、文档与 analyze 均通过。Rust 数量由 498 调整为 487，是移除 14 项原型专属测试并新增 3 项独立产品回归；不是将失败用例设为 ignored。日志：`build/validation/v0.10-final-cleanup-check.log`。
