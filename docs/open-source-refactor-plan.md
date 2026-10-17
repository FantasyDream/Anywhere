# 开源复用重构路线

分支：`codex/refactor-open-source`。开始：2026-10-18。基线为 V0.10 收尾提交 `dbf04bb`，已推送 `codex/dev-0.10`；V0.9 已收尾并在 `dev-0.9`。选型依据见 [库清单](research/open-source-reuse-audit.md)。

目标：减少通用基础设施维护，在不改变 Context / Scope、稳定列表、授权 / 执行边界、存储身份及 15 秒恢复规则的前提下复用成熟库。每阶段先实现和单元门禁，最后做必要的 UI 验证；原型优于现有实现才进入生产。

| 阶段 | 范围 | 验收与后续入口 |
|---|---|---|
| R1 | encoding_rs / chardetng 文本预览；which 工具查找 | 本阶段实现。BOM、UTF-16、常见文本 / 源码的传统编码推测；输入和输出各限 64 KiB；未知二进制不猜编码。工具目录优先级、可执行权限、NVM / SDKMAN 与 wrapper 保持，Windows 不引入 PATHEXT 批处理回退 |
| R2 | SQLite 查询临时记录 / 外部排序原型 | **对照已完成，保留现有生产排序器。** 原型通过正确性验证，但本机速度、RSS 和临时磁盘均无收益，见 [R2 记录](research/result-store-r2.md)。原型仅在 Unix 测试构建启用，不进入 App |
| R3 | nucleo-matcher 匹配原型 | 先比较应用和 Action，不改文件搜索语义。保留 Exact / Prefix / WordPrefix 优先级、各词约束、标识符限制和 frecency，比较中英文与缩写的候选集合 / 排序。无确定收益则保留现有实现 |
| R4 | 根据原型结果收敛 | 只接入通过验收的后端，移除不再使用的旧代码 / 依赖；全量门禁、打包与界面回归后提交 |

## R1 行为

- 文件仍通过同一资源验证和有界读取，Flutter / Bridge 接口不变。
- UTF-8 优先；UTF-8 BOM 去除，UTF-16LE / BE 仅凭 BOM 明确识别；UTF-32 不做错误的 UTF-16 解释。
- 源码及常见 txt / md / csv / tsv / log / rtf / svg 名称在 UTF-8 不成立时允许传统编码推测；界面明确显示 guessed 提示，不宣称自动检测永远正确。
- 未知类型仍按 UTF-8 或明确 BOM 预览；二进制控制符、解码错误和损坏的 BOM 内容不替换为乱码继续显示。
- 读取截断允许尾部未完成编码单元，解码输出独立限额，避免 UTF-16 / GBK 转 UTF-8 后扩大预览体积。
- `which` 仅检查经过原有 GUI 工具目录优先级生成的绝对候选，不读 shell 初始化文件、不通过拼接 PATH 丢失目录中的分隔符、不补工作目录或批处理后缀。
- 依赖固定由 Cargo.lock 记录，包内包含新依赖许可文件；无任意升级 Flutter SDK、热加载框架或新业务 DSL。

## 边界

R1 不包含 SQLite / nucleo 的生产替换，不能据此声明文件搜索更快或整 App 内存更低；后续优先做 R2 的集合与资源基准。现有 V0.10 VoiceOver / 首次权限 / Windows 实机 / 长时间内存的待验收边界继续保留。


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
