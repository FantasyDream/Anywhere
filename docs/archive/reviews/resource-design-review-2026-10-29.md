# 升级后正确性、资源开销与设计审查（2026-10-29）

修复前审查记录：基线为 `codex/dependency-upgrades@c98a8e7`，工作区另有 Build 36 的构建号与打包记录。当前运行包为 `0.14.0+36`。本轮只审查与复现，未修改业务实现，也未替换运行程序。

检查粒度为跨层调用与风险路径：Flutter 的 Launcher / 工具页 / 设置 / 图标 / 任务 / 历史会话，Rust 的来源准入、执行、持久化、资源准备与脚本转换，原生窗口 / 预览生命周期，以及插件 SDK 的缓存、事件和分块传输。未把入口审查当作全仓逐行穷尽；本轮重点是已有全量门禁未覆盖的时序和分配问题。

## 确认的问题

### R1 / P2：大状态传输期间丢失更新后，SDK 可能永久停在旧版本

位置：`packages/plugin-sdk/src/client.ts:159`。

大状态通知不含正文，客户端每条通知分别启动 `state.get`。Rust 每文档只允许一份分块缓冲区；前一次状态正在下载时，后一次读取会返回 `BUSY`。这里直接吞掉失败，没有保留“已知最新版本”或合并后续读取。旧传输完成后只接收旧数据，最后一条新通知已经消耗，后续 `input.get` / `context.get` 都继续命中旧缓存，直到出现另一条通知。

合成传输探针使用约 60 KiB 的第 2 版状态，在首个分块挂起时通知第 3 版，使第二次状态读取返回 `BUSY`。放行旧下载后，客户端仍读到第 2 版输入 `two`，未追到第 3 版。

建议：保存目标版本并只保留一个在途状态读取，完成或 `BUSY` 后按当前目标继续追赶；生命周期失效时停止。状态同步集中到一条路径，避免订阅初始化、通知和显式读取各自处理失效。

### R2 / P2：等待 input.set 成功后，input.get 仍能返回旧内容

位置：`packages/plugin-sdk/src/client.ts:345–352`。

`input.get` 直接使用状态缓存；`input.set` 只转发请求，没有更新或失效该缓存。宿主已成功提交输入，而状态事件尚在路上时，`await input.set('new'); await input.get()` 返回 `old`。状态推送与 RPC 响应是两条异步路径，没有保证推送先于 Promise 完成。后台 SDK 已在自身写入后刷新快照，前台语义不一致。

合成探针先缓存 `old`，宿主确认写入 `new`，延后状态事件，读回结果为 `old`。

建议：成功写入后从同一状态同步路径取得权威读回或失效缓存；不要以未来事件作为写后读取正确性的唯一条件。

### R3 / P2：长按 Enter 会重复打开项目或恢复剪贴板

位置：`apps/flutter/lib/features/application_history/page.dart:69`、`apps/flutter/lib/features/clipboard/clipboard_history_page.dart:103`。

共享工具页将 `KeyDownEvent` 和 `KeyRepeatEvent` 都交给页面；这两个页面对 Enter 不区分事件类型。每次操作完成、busy 清除后，后续重复事件又能提交一次。悬浮剪贴板已有“一次物理按下只复制一次”的处理与回归，主页面没有沿用。

两个 Widget 探针分别发送一次按下、两次重复和一次抬起，应用历史打开次数及主剪贴板复制次数均为 **3**，期望均为 1。实际打开 / 复制由测试后端计数，没有操作个人应用或剪贴板。

建议：方向键继续接受重复，Enter 等有副作用命令仅接受首次按下；将两处场景补入正式回归。

### R4 / P2：历史页四次重试与后端读取寿命不匹配，可能停在 busy 空页

位置：`apps/flutter/lib/features/application_history/session.dart:130–149`，服务侧准入位于 `crates/anywhere-core/src/features/application_history/service/query.rs:70`。

旧页面关闭只取消资格，无法立即中断正在进行的同步文件 / SQLite I/O。新页面的读取会遇到服务的 `try_lock` 忙碌状态；Flutter 只按 80 ms 间隔重试四次，总等待约 320 ms，而来源读取预算为 5 秒。四次结束后发布 busy、清空列表并停止 loading，没有在旧 worker 释放时继续当前查询。快速切换来源、旧读取稍慢即可进入这一状态，后续需要用户再次刷新或编辑查询。

受控探针让前五次读取忙碌，随后恢复可读并推进一秒：请求数停在 **5**，结果仍为 `busy / 0 rows / loading=false`。

建议：在服务侧提供可取消的准入 / 完成通知，保留最新查询资格；简化页面里的固定次数轮询。若保留重试，等待寿命应与后端预算一致，并在隐藏 / 销毁时退出。

### R5 / P2：转换输出先整体复制到 Rust，再检查 2 MiB 限制

位置：`crates/anywhere-core/src/application/application_history/script.rs:26–51`。

回调声明返回 Rust `String`，`bridge.call` 完成跨 QuickJS / Rust 的整串复制后才检查 `MAX_OUTPUT`。因此该限制能拒绝结果，但阻止不了已经发生的 Rust 大分配；此时还同时保留 VM 中的结果和 JSON 字符串。32 MiB VM 限制没有计算这份 Rust 缓冲区。

直接调用当前生产转换函数，以 `null` 作为输入、返回 8 MiB 字符串。最终正确拒绝结果，但分配器记录到单次 **8,388,610 字节** 的 Rust 分配，超过输出限制 **2,097,152 字节**。这里只测最大单次 Rust 分配，没有测整进程峰值、RSS 或泄漏。

建议：先用 JS 字符串句柄核对 UTF-8 字节长度或经有界转换读取，再创建 Rust String；继续保留 VM 总预算。检查应发生在所保护的大分配之前。

### R6 / P3：悬浮剪贴板空闲时仍每秒重建整份元数据

位置：`apps/flutter/lib/features/clipboard/clipboard_shelf_app.dart:55,78`、`apps/flutter/lib/features/clipboard/clipboard_history_session.dart:215`。

每秒轮询都执行完整 search，再在 Dart 端比较列表。`_sameItems` 减少了重绘，但执行时 SQLite 查询、Bridge 转换及 DTO 分配已经完成。即使采集关闭且历史完全不变，路径仍然运行。

1000 条合成元数据、采集关闭、无任何内容变化的 20 秒探针，共调用后端 **21 次**，构造 **21,000 行 DTO**。这是重复工作与短期对象分配的计数，不是驻留内存持续增长的证据。

建议：由 HistoryService 发布数据 / 状态修订号，先做轻量变化检查或订阅，再按变化重新查询；版本须涵盖采集、删除、清空、到期与偏好变化，保持多窗口同步。避免仅在 UI 收到完整数据后去重。

## 架构与可维护性判断

优先整理两条状态所有权：SDK 的缓存同步由一个版本驱动读取流程负责；历史 worker 的准入和释放由服务负责。对应的坏味道是静默吞掉新版本读取失败、跨层重复重试与 `80 ms × 4` 的孤立魔数。输出额度检查位置和“查询后去重”则属于防护 / 优化发生过晚。

本轮没有证据支持重写现有 Registry 或继续增加通用页面、重试、缓存框架。执行资格、过期发布、权限与资源配额仍是当前必要契约；上述建议集中于减少重复路径和把检查移到实际生效的位置。

## 复现材料与边界

- `build/review-review-probe_test.dart` / `build/review-flutter-probe.log`：三条故障期望失败，分别为两页长按与 busy 恢复；空闲读取计数通过。
- `build/review-sdk-probe.mjs` / `build/review-sdk-probe.log`：写后读取、分块期间更新两种状态问题。
- `build/review_transform_memory.rs` / `build/review-transform-memory.log`：直接复用生产 transform，记录最大 Rust 分配；临时 Cargo example 已移出源码树。

材料只用合成数据，保存在忽略的 build 目录。本轮没有修复上述问题、没有提交或重打包，也没有进行原生 GUI、物理输入法、真实进程内存、远端 CI 或跨平台运行验收。没有将此前完整门禁通过作为这些新场景通过的证明。


## 后续修复与验证（2026-10-29）

用户确认修复后，R1–R6 已在 `codex/dependency-upgrades` 工作区完成；以上复现和未修复说明保留为修复前记录。main 未改变，运行中的 Build 36 未替换。

| 问题 | 修复与回归 |
|---|---|
| R1 | SDK 保存目标版本，订阅、通知与读取共享一个同步 Promise；大状态传输期间的新通知合并后继续追赶。仅 `BUSY` 的只读同步限时重试，销毁取消计时器。受控分块回归最终读到第 3 版。 |
| R2 | 写入成功推进缓存失效代数，在途读也必须重新取得当前快照；不自动重试写入，不将后续读失败改报为写入失败。无推送与写入期间已有读取两种回归均通过。 |
| R3 | 两个主历史页 Enter 仅在 KeyDown 执行，重复事件仍消费，方向导航保留重复。原先的按下 / 两次重复 / 抬起夹具均从 3 次操作变为 1 次。 |
| R4 | Rust 用共享信号量管理真实读取寿命，交互查询可取消地等待准入，最多 5 秒；新查询、会话或配置变化唤醒并撤销旧等待。阻塞 worker 持有名额直到实际结束，执行验证仍忙碌即拒绝。Flutter 删除 80 ms × 4 轮询；等待超过旧窗口、取消与超时回归通过。 |
| R5 | 转换返回 JS 字符串句柄，借用 VM UTF-8 缓冲区，先检查 2 MiB 再直接解析，不创建整份 Rust String。Unicode / 超限回归与同一分配探针通过。 |
| R6 | 服务在同一锁内检查过期、存储写入计数、状态、策略和查询身份；版本未变返回 None，Bridge 不构造列表 DTO。Flutter 仅对当前已接收查询做条件轮询，查询变化与失败恢复完整读取。采集、去重顺序、删除、清空、过期、暂停恢复、权限提示与禁用的版本回归通过。 |

合成前后对比：

| 场景 / 口径 | 修复前 | 修复后 |
|---|---:|---:|
| 同一 8 MiB 超限转换输出，最大单次 Rust 分配 | 8,388,610 字节 | 176 字节 |
| 1,000 条不变剪贴板元数据，初始加载与 20 次空闲轮询构造的 DTO 行数 | 21,000 | 1,000 |

空闲轮询次数仍为 21（初始加载加 20 次轻量检查），以同步其他窗口的变化。分配探针不包含 QuickJS 的 C 堆、进程峰值、RSS 或 GC；VM 自身仍受 32 MiB 预算约束，不能将 176 字节理解为整次转换总内存。

验证：完整 `scripts/check.sh` exit 0；Core 328 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 476 / 1 跳过、SDK 29、JSON 30。相关 Flutter 子集 83 项通过；Bridge 重新生成并编译，真实动态库 FFI 1 项独立通过（普通套件的 opt-in 跳过项）。规范、Clippy、架构 / 生成一致性、原生自动回归与文档检查通过。

本地忽略目录证据：`build/resource-fixes-full-check.log`、`build/resource-fixes-flutter-tests.log`、`build/resource-fixes-sdk-tests.log`、`build/resource-fixes-bridge-generation.log`、`build/resource-fixes-bridge-build.log`、`build/resource-fixes-ffi.log`、`build/resource-fixes-transform-memory.log`。临时分配探针 example 已移出源码树。

没有重新打包、替换运行程序或提交 / 推送。当前 App 和已安装的 Web 插件不包含本轮修复；SDK / 示例已由工程门禁重建，安装包更新仍需单独导入。未进行新增 GUI、物理输入法、整 App 内存、远端 CI 或跨平台运行验收。当前机制说明分别维护在来源指南、插件开发指南、架构与性能页。
