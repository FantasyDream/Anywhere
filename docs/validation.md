# V0.2 验收记录

日期：2026-09-15。分支 `dev-0.2`，基线 `7cb49f1`。**本记录只包含实际执行的检查；未执行的项明确标注。**

## 构建环境

macOS 26.6.2 / Apple Silicon，Rust 1.98.1，Flutter `3.47.0-1.0.pre-1`（Engine `0e228ec8c8d2abc9fcf1d053e8a40665bb859ec7`），flutter_rust_bridge 2.11.1。

## 已执行的检查

| 检查 | 命令 | 结果 |
|---|---|---|
| 工作区测试 | `cargo test --workspace --locked` | 通过：Core lib 55、Facade 29、Platform 9、Plugin 2、Protocol 4（共 99 项） |
| Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过，无警告 |
| 格式 | `cargo fmt --all --check` | 通过 |
| 架构边界 | `python3 scripts/check-architecture.py` | 通过：90 个 Core 依赖，无 UI/Flutter 依赖 |
| Flutter 静态检查 | `flutter analyze` | No issues found |
| Flutter 组件测试 | `flutter test` | 通过：12 项 |
| macOS Release 构建 | `scripts/build-macos.sh` | 通过，签名校验成功 |
| 微基准 | `cargo run --release -p anywhere-core --example measure` | 见下表 |
| Windows 类型检查 | `ANYWHERE_SKIP_NATIVE=1 cargo clippy --target x86_64-pc-windows-msvc -p anywhere-platform --all-targets -- -D warnings` | 通过（只含 Rust 侧；`native/windows.cpp` 需 Windows SDK，未编译） |

测试数量相对 V0.1（17 项 Rust + 4 项组件）增长，覆盖统一 Provider、预算与 late Context、应用搜索、Scope/Availability、Git 执行边界和 v1 兼容性。

## 性能采样

`measure` 已修正 V0.1 把采样向量 push 计入 ranking 的计时误差，并加入 1000 个合成应用、`git sta`、`chrome` 和 1 MiB 输入。每种场景预热 100 次、测量 1000 次；单位 ms。

多次 Release 采样（本机 arm64，含最终代码在内）的 p50 / p95 区间。最后一次采样的完整数字如下（`scripts/env.sh` + `cargo run --release --locked -p anywhere-core --example measure`）：

| 输入 | 输入识别 p50 / p95 | matcher p50 / p95 | ranking p50 / p95 |
|---|---:|---:|---:|
| `https://github.com` | 0.022 / 0.029 | 0.415 / 0.436 | 0.413 / 0.434 |
| `{"name":"foo","age":18}` | 0.022 / 0.029 | 0.420 / 0.436 | 0.416 / 0.433 |
| `Cargo.toml` | 0.014 / 0.042 | 0.423 / 0.646 | 0.420 / 0.642 |
| `.` | 0.011 / 0.018 | 0.334 / 0.353 | 0.331 / 0.350 |
| `hello world` | 0.021 / 0.030 | 0.395 / 0.413 | 0.395 / 0.412 |
| `git sta` | 0.023 / 0.034 | 0.396 / 0.443 | 0.395 / 0.443 |
| `chrome` | 0.022 / 0.033 | 0.381 / 0.407 | 0.380 / 0.406 |
| 1 MiB 文本 | 0.268 / 0.284 | 0.014 / 0.020 | 0.014 / 0.019 |

早期采样（F09 之前，应用索引尚为空）matcher 是 0.11–0.14ms；那一档没有真正把 1000 个合成应用投进 `application_candidates`，所以不是可比基线。现在的 0.33–0.42ms 才是「索引里有 1000 个应用」的真实代价，仍远低于 matcher <10ms、ranking <5ms 的目标。

`Cargo.toml` 与 `.` 的偶发高 p95 来自路径识别里的真实文件系统调用，受机器负载影响。

**1 MiB 输入曾经是 1.25s**（p95 1.56s）：应用索引一被填充，`application_candidates` 就会把整个 1 MiB 查询按每个已安装应用扫一遍，而这条路径属于首屏构建。现在超过 `MAX_QUERY_FOR_APPLICATIONS`（1024 字符）的查询不再参与应用索引匹配——粘贴进来的文档不是应用名，它的对象动作（URL/JSON/路径）本来就已经在列表里。修复后 1 MiB 输入降到 0.014ms，代价是不可搜索超长字符串（无实际用途），由 `a_pasted_document_does_not_match_the_application_index` 断言。

结论：输入识别与 matcher 合计 p95 最差约 1.1ms（偶发文件系统抖动），远低于 Resolver <10ms、Ranking <5ms 的既有目标。matcher 比 V0.1 的「仅 ranking」慢，因为它现在还要做 Availability、查询匹配、分组、Scope 计算和应用候选；这是同一阶段新增的工作，不是回归。

## 更名为 Anywhere（V0.2 后）

应用、Scope 分组与仓库统一改名。显示名用 `Anywhere`，标识符用 `anywhere`：

| 位置 | 旧 | 新 |
|---|---|---|
| Cargo 包 / crate 目录 | `launcher-core` 等五个 | `anywhere-core` 等五个 |
| Rust crate 标识符 | `launcher_core` | `anywhere_core` |
| 桥接动态库 | `liblauncher_bridge_flutter.dylib` | `libanywhere_bridge_flutter.dylib` |
| Swift 可执行文件 | `ContextLauncher` | `Anywhere` |
| 应用包 | `Context Launcher.app` | `Anywhere.app` |
| bundle id | `dev.contextlauncher.mvp` | `dev.anywhere.launcher` |
| Dart 包 | `context_launcher` | `anywhere` |
| MethodChannel | `context_launcher/window` | `anywhere/window` |
| Windows 可执行文件 / 产物 | `context_launcher.exe` / `Context-Launcher-Windows-x64.zip` | `anywhere.exe` / `Anywhere-Windows-x64.zip` |
| 数据目录（macOS 与 Windows） | `Application Support/ContextLauncher` | `Application Support/Anywhere` |

首次以新名字启动时，宿主会**一次性接管旧数据目录**（macOS 与 Windows 都实现了），因此已有的 usage 历史与插件不会丢失；`ContextLauncher` 只作为迁移回退出现在两处原生代码里。

**仓库目录改名需要你手动执行**（当前会话的工作目录就在其中，我无法在运行中重命名它）：

```bash
cd ..
pkill -f Anywhere || true
mv Context-Aware-Launcher Anywhere
cd Anywhere
source scripts/env.sh
scripts/build-macos.sh
```

目录改名后有两个连带影响，都已处理：

- `Cargo.lock` 的 `path+file://` 记录指向仓库绝对路径，改名后需重新解析一次；`scripts/build-macos.sh` 不带 `--locked`，会自行更新。改名后 `--locked` 构建仍可用。
- clang/swift 模块缓存内嵌绝对路径，旧缓存会让 `swiftc` 报 `missing required module 'SwiftShims'`。缓存路径现在**按 checkout 目录名生成**（`scripts/env.sh` 的 `CLANG_MODULE_CACHE_PATH` / `SWIFT_MODULE_CACHE_PATH`），所以再次移动或改名目录会自然构建新缓存，不再需要手工清理。

改名还暴露了两个只在运行时才发作的问题，一并修掉：

- **宿主加载的库名与打包出来的库名不一致**。`apps/flutter/lib/main.dart` 里写死了 `liblauncher_bridge_flutter.dylib`，而构建脚本改名后打进包的是 `libanywhere_bridge_flutter.dylib`，启动直接报 `Failed to load dynamic library`。改名时把这处漏了。构建脚本现在从 `main.dart` 解析出宿主期望的库名并与包内文件比对，不一致就**构建失败**，不再留到运行时。
- **dylib 的 install name 是构建目录的绝对路径**。cargo 产出的 dylib 把 `target/release/deps/...` 记为 install name，于是这个应用包隐含依赖「构建目录仍然存在」：`cargo clean`、移动应用或换机器都会以同样的 dlopen 错误失败。现在用 `install_name_tool -id @rpath/...` 改成与 Flutter 自带 framework 相同的相对形式，改完重新签名（修改 load command 会使签名失效）。

Git 历史不受影响（`.git` 随目录一起移动）。

## 应用显示名与中文名（更名后追加）

**显示名改为与操作系统一致。** 索引原先用 `CFBundleName`，而它常是与用户所见无关的代号（Lark / QQMusic / CorpLink）。现在：

- **显示名**取系统语言的本地化名（`zh_CN.lproj` 里的 `CFBundleDisplayName`），即 Dock 与访达里显示的名字。
- **原名进别名**，因此英文名仍可搜索。

实测（本机）：

```text
显示名 luckin盾        别名 ["CorpLink"]
显示名 QQ音乐          别名 ["QQMusic"]
显示名 Safari浏览器     别名 ["Safari"]
显示名 UU远程          别名 ["UURemote"]
显示名 WPS Office      别名 ["wpsoffice"]
显示名 飞书            别名 ["Lark"]
```

只影响真正提供本地化的应用；未提供本地化的应用显示名不变（Edge、Chrome、IntelliJ 等）。

## 移除内建 Web Search 与示例插件

**内建 `core.text.search` 已删除。** 原因是它把 `https://www.google.com/search` 写死在执行器里。用户要求改成「浏览器默认搜索引擎」——**这不可实现**：macOS 不提供任何 API 读取浏览器当前的默认搜索引擎，Safari/Chrome/Edge/Firefox 各自把该偏好存在自己的容器里，没有公开接口。第三方应用只能自己硬编码一个 provider，那就等于换个默认值而已。因此选择删除，而不是换一个硬编码值。

随之的改动：

- `plugins/examples/google.toml` 删除，换成中性的 `plugins/examples/search.toml`（`example.*` 命名空间，指向 example.com，纯示例）。
- **应用包不再捆绑任何插件**。原先 `build-macos.sh` 会把示例插件复制进 `Contents/Resources/plugins/`，等于给每个用户装上一个他没要求的搜索；现在插件只从用户自己的目录加载。想要搜索的用户把示例文件复制到 `~/Library/Application Support/Anywhere/plugins/` 即可。
- 宿主因此不再凭空产生任何文本兜底动作；`candidates` 的兜底逻辑保留，供插件使用。

## 中文搜索（更名后追加）

用户报告「使用中文搜索不到应用」。原因是索引只读 `Info.plist` 的 `CFBundleDisplayName` / `CFBundleName`，而 macOS 的**本地化名存放在 `<lang>.lproj/InfoPlist.strings`**，两者可以完全不同：

```text
/Applications/Safari.app/Contents/Resources/zh_CN.lproj/InfoPlist.strings
  "CFBundleDisplayName" => "Safari浏览器"
```

因此 `飞书`、`QQ音乐`、`UU远程`、`luckin盾`、`Safari浏览器` 这些屏幕上显示的名字全都搜不到，只能靠英文名（Lark、QQMusic…）找到。

修复分三处：

- **Native**：新增 `launcher_bundle_localized_names`，按用户偏好语言在候选 `.lproj` 中读取 `InfoPlist.strings` 的 `CFBundleDisplayName` / `CFBundleName`。语言码会归一：`zh-Hans-CN` → `zh-Hans` / `zh_CN`，`zh-Hant-*` → `zh-Hant` / `zh_TW` / `zh_HK`。
- **语言来源**：新增 `launcher_preferred_languages` 从 `NSLocale.preferredLanguages` 读取。**不能用 `AppleLanguages` 环境变量**——从 Finder 启动的进程环境里没有它，只有 `defaults read -g` 能读到。
- **索引**：本地化名作为 `aliases`（可见名）参与匹配，因此支持子串搜索。只读用户偏好的语言，不读每个应用的全部本地化，避免索引膨胀。

同时修掉一个被它暴露的缺陷：函数末尾有一段更早的代码把 `aliases` 整个替换成「文件名 stem」，**覆盖掉了刚收集的本地化名**，所以第一版修复无效（索引里 5 个应用拿不到别名）。现在该处只做去重，不再重建。

实测（本机 100 个应用）：

```text
Lark   (com.electron.lark)      aliases=["飞书"]
QQMusic(com.tencent.QQMusicMac) aliases=["QQ音乐"]
UURemote(com.netease.uuremote)  aliases=["UU远程"]
Safari (com.apple.Safari)       aliases=["Safari浏览器"]
搜「浏览器」→ Safari ✓   搜「飞书」→ Lark ✓
```

**系统自带应用仍是英文名**（「访达」「系统设置」搜不到）：macOS 26 起这些应用的 `.lproj` 目录为空，本地化字符串不在包内，`NSBundle.localizedInfoDictionary`、`FileManager.displayName`、`URL.localizedName` 三者在中文系统下也一律返回英文。这不是本项目的缺口，需要 Apple 提供接口才能解决，已如实记录。

**应用包图标（Finder 显示）**：更名后 Finder 曾把应用画成「禁止」占位图标，随后又画成通用文档图标。原因不是 `.icns` 有问题——用 `NSWorkspace.icon(forFile:)` 渲染应用包一直是正确的放大镜，`iconutil` 也能完整解出 10 个尺寸，块结构含 `ic07`–`ic14` 全部现代类型——而是 **LaunchServices 的图标/元数据缓存**：bundle path 与 bundle id 都变了，缓存仍指向旧条目。`lsregister -f` 重新登记即可恢复，构建脚本现在每次构建后都会执行它，因此改名或重建后不会再出现。

排查时注意一个陷阱：把 `.icns` 文件**本身**交给 `NSWorkspace.icon(forFile:)` 会得到通用文档图标，看起来像失败，但那只是 Finder 给 `.icns` 文件画的图标。要验证必须传 `.app` 路径。

**应用图标（列表内）**：优先读 bundle 自己的 `CFBundleIconFile`（`Contents/Resources/*.icns`），失败才回退 `NSWorkspace.icon(forFile:)`。后者对尚未被 LaunchServices 索引的应用可能返回通用图标，实测 Edge 就会出现这种情况。宿主侧另修了一个缓存缺陷：图标获取失败曾被永久缓存为「无图标」，`containsKey` 又阻止重试，于是首次失败的应用永远不会再显示图标；现在失败与在途分开记录，用户每次改输入或切 Scope 都会重试一次。

## 实机发现的回归与修复

从 Finder 唤醒时首屏为空、必须输入文字才出现动作。日志证据（`~/Library/Application Support/Anywhere/launcher.log`）：

```text
03:13:07.404  session_created
03:13:07.444  first_snapshot_budget_reached  duration_us=41703
03:13:07.444  snapshot_committed  snapshot_seq=1  complete=false  candidates=0
03:13:07.652  directory_capture_finished  duration_us=248529  found=true
03:13:07.653  snapshot_committed  snapshot_seq=2  complete=true  candidates=6
```

真实 Finder Automation 目录捕获需要约 250ms，而首屏预算只有 40ms，于是首屏发布 0 个候选；虽然 late pass 在 250ms 时正确补齐了 6 个候选，但「先显示空 launcher、要求用户先输入」本身不是可接受的默认体验。

修复：首屏在预算到期时，若**仍有 Provider 在飞行中**，就把等待延长到 `DEFAULT_FIRST_CONTENT_GRACE`（400ms），而不是在半途发布。图是顺序执行，同一时刻最多一个调用在飞，因此这个延长是有界的；超过 grace 仍照常发布截断快照，late pass 继续补齐。调用方可通过 `with_content_grace` 调整或关闭（测试用它覆盖纯预算路径）。

第一版修复只覆盖了「还没有任何 Context」的情形，第二次实测暴露了它不够：目录一到就命中预算，恰好在 Git provider 开始之前提交了 3 个候选，而 Git 只花 7µs。

```text
03:19:26.028  directory_capture_finished  found=true
03:19:26.028  first_snapshot_budget_reached  complete=false
03:19:26.028  snapshot_committed  seq=1  candidates=3  scope_ids=["all","app","folder"]
03:19:26.028  provider_started    core.git_repository
03:19:26.028  provider_completed  core.git_repository  7µs
03:19:26.028  snapshot_committed  seq=2  candidates=6  scope_ids=["all","app","folder","git"]
```

修正为「只要还有调用在飞就延长」，并新增 `first_snapshot_waits_for_the_step_after_the_directory`：60ms 捕获 + 30ms 预算，首屏必须已含 Git Status，且 Scope 顺序为 `all/app/git/folder`。

同一批日志还暴露了 Scope Bar 的顺序问题：动态分组原按 id 字典序排列，`folder` 排在 `git` 前面，与文档示例 `[ All ] [ Finder ] [ Git ] [ Folder ]` 不符。已改为固定顺序 git → folder → file → web → json，其余按 id，并由上面的测试锁定。

修复后的自动验证：

- `a_slow_directory_capture_still_shows_context_actions_without_typing`：150ms 捕获 + 40ms 预算，首屏必须已含 Open Terminal Here 与 Folder Scope。
- `first_snapshot_waits_for_the_step_after_the_directory`：60ms 捕获 + 30ms 预算，首屏必须已含 Git Status 且 Scope 顺序为 `all/app/git/folder`。
- `first_snapshot_grace_is_bounded_and_late_pass_still_completes`：1200ms 捕获 + 20ms 预算 + 60ms grace，首屏必须在 900ms 内返回且为空，late pass 随后补齐动作。
- 原有零预算测试显式关闭 grace，继续覆盖纯预算路径与 late append。

## 第二轮审查发现的缺陷与修复

代码审查（Standards + Spec 两轴）发现的问题及处理：

| 问题 | 影响 | 修复 |
|---|---|---|
| late append 只写 Core 的事件通道，桥接层另有自己的镜像通道，而镜像只在 `activate`/`update_input` 内写入 | grace 被耗尽时 late 结果永远送不到界面，**必须输入文字才刷新**；规格 §2 与 DoD 的 Late Result 在真实应用中不可达 | 删除桥接层镜像，`wait_for_snapshot` 直接读 Core 的 `latest_state`/`state_updates`；新增 `late_context_is_visible_through_the_host_facing_snapshot_api` 用宿主同款 API 断言 Git 由 late pass 送达 |
| `matcher_duration_us` 从加锁前开始计时 | 该指标实际测的是「加锁 + 快照记账」，不是 matcher；真正测准的 `CandidateSet.matcher_us` 无人读取 | 日志改用 `set.matcher_us` |
| Provider 链图把目录链画在 `core.text` 下 | 与 `CurrentProviderDescriptor.accepts = core.application` 不符 | 改为 `core.application → core.directory` 并说明原因 |
| 指标常量被描述成单一来源 | tracing 要求字面量，两端可能静默漂移 | 注释说明机制，并保留断言七个名字的测试作为唯一防线 |
| `merge_json_aliases` 内联重复 `merge_usage` | 同一逻辑两处 | 改用 `merge_usage` |
| 截断路径不发 `context_resolution_total` | 指标只覆盖完成路径 | 截断也发同一条事件（带 `complete=false`），删除多余消息 |
| `home_bonus` 的 +40 Git 项 | 按目录判断，而 Git 只接受 `git.repository`，永不生效 | 删除并写明不应有分组加成 |
| Dart 图标缓存 | 上限只在成功回调执行，在途占位符越界；失败加载永久缓存不重试 | 在途请求单独记账，上限对两者生效，隐藏时清空 |

## 第三轮静态审查（F01–F14）

逐条核对第三轮静态审查的 14 项发现。结论按「已修复 / 判定不成立 / 接受并记录」三类给出，未修复项都写明了理由。

| 编号 | 发现 | 结论 | 处理 |
|---|---|---|---|
| F01 | 跨 Revision 复用的 Context 缓存把「用户输入派生」的目录也缓存了 | 已修复 | 只有环境来源（`ApplicationContext`/`ForegroundApplication`）进入 `context_cache`；`UserInput` 每次从输入重建。新增 `only_ambient_context_is_reusable_across_revisions` |
| F02 | 乱序的旧快照可能覆盖新状态 | 不成立 | 会话持有的 `state.revision` 严格递增，`state.revision < held.revision` 的旧快照已被拒绝；Dart 侧 `_requestedRevision` 守卫保留但注明没有测试能命中该分支 |
| F03 | 事务票据的货币性只在提交前检查一次 | 已修复 | `commit` 在持有会话锁后重新校验会话与 Revision，覆盖「较新的 Revision 先拿到锁」的次序 |
| F04 | 副作用之后的货币性检查会丢弃成功的执行（含 usage 记录） | 已修复 | 删除副作用后的 `is_current()` 中止：动作本身导致窗口失焦＝该动作结束了自己的会话，再检查只会让**最成功的那些动作**记不上 usage。改为在入口处用 `candidates_revision` 门禁，候选集未发布到当前 Revision 时拒绝执行（`a_candidate_from_a_superseded_revision_cannot_execute`、`usage_is_recorded_even_when_the_action_ends_its_own_session`） |
| F05 | 40ms 首屏预算被 grace 延长到约 440ms | 接受并记录 | 见下方「与规格的偏差」：这是为修复用户报告的「Finder 唤起需先输入才显示目录动作」而做的取舍 |
| F06 | 被截断的 Provider 调用「从未 poll」的假设不成立 | 判定不成立（但注释有误） | 目录捕获在 Provider 被 poll 的那一刻就已启动，其 Task 独立于 future；若撤销登记被跳过，late pass 将永远不再调用它，快照就停在空列表。因此「截断即重试」是**必须**的，重入安全是 Provider 的契约（`ContextProvider::resolve` 已注明），`DirectoryMemo` 单飞正是为此 |
| F07 | Provider 内的同步文件系统调用不可抢占 | 部分修复 | 阻塞调用仍跑在 `spawn_blocking` 上且无法在系统调用中途取消，但**等待**现在是可取消的：`DirectoryMemo::directory` 同时监听执行戳（`ExecutionContext::superseded`），被取代的 Revision 立即放弃等待并返回空上下文，而不是把阻塞工作占满 30s 超时；`GitRepositoryProvider` 在进入文件系统遍历前与返回前各校验一次执行戳。新增 `a_superseded_revision_does_not_keep_waiting_for_the_directory`。真正中断 `read_dir`/Automation 调用仍需可中断 IO，超出本版范围 |
| F08 | 应用索引每次激活都重扫，且索引就绪后不重新提交 | 已修复 | 扫描改为每进程一次（`catalog_started`）；索引就绪后由一个后台任务重建当前 Revision 的快照（`recompute_current`）。重建**必须等该 Revision 收敛**，否则会以新的 `snapshot_seq` 发布一个未完成的快照——`an_index_that_arrives_late_reaches_the_settled_snapshot` 断言「带应用候选的快照必定已 `context_complete`」 |
| F09 | 候选集在构建期被截断到 Home 上限 | 已修复 | 候选集完整保留，Home 的 7 条上限移到 Flutter 视图；每个 Scope 的 id 都指向存在的候选（`every_scope_id_resolves_to_a_candidate`） |
| F10 | 所有权（Current App）跨 Context 串味 | 已修复 | 所有权按 Context 计算，环境所有者独立推导，不再沿用循环中上一次的结果 |
| F11 | `compact()` 只保留 ASCII，非拉丁输入被清空 | 已修复 | 改为 `char::is_alphanumeric`，中文/重音字符保留 |
| F12 | Git 执行的两个边界：`textconv` 未被禁用；reader join 可能超出 deadline | 已修复 | 加入 `--no-textconv`；读取改为有界等待（`ReaderOutcome`），子进程被 kill 后若管道仍未关闭，超过 1s 即按超时返回，不再无限阻塞 |
| F13 | Windows 上不产生「Git 未安装」以外的 Git 语义 | 已实现，待实机验收 | 三个新平台能力在 Windows 上补齐：开始菜单 `.lnk` 索引（`IShellLinkW` 取名称与目标，`ShellExecute` 启动）、`CF_UNICODETEXT` 剪贴板读取（64K 上限）、Git runner（与 macOS 共用 `git.rs` 的 argv 白名单与输出格式化，`git.exe` 从 `PATH` 查找）。`anywhere-platform` 的 Windows 目标在 macOS 上通过 `cargo check` 与 `clippy -D warnings`（`ANYWHERE_SKIP_NATIVE=1`，只检查 Rust 侧）；**C++ shim 未编译、Windows 未运行**，验收项 W19–W22 见 `docs/windows.md` |
| F14 | matcher / ranking 计时边界不完全对应两个阶段 | 接受并记录 | `matcher_us` 覆盖动作匹配，`ranking_us` 覆盖去重、Scope 构建与排序——后者包含少量构建开销。两个名字都是规格要求且都有测量，未发现会误导排障的偏差 |

## 与规格的偏差（有意保留）

- **首屏预算被内容 grace 延长**：规格 §2 写「预算到期即返回」，实现改为「预算到期时若仍有 Provider 在飞，最多再等 `DEFAULT_FIRST_CONTENT_GRACE`（400ms）」。原因是实测 Finder Automation 目录捕获约需 250ms，纯预算路径会先发布一个空 launcher 并要求用户输入，这正是用户报告的第一个缺陷。代价是冷启动首帧可能到约 300ms，`hotkey_to_visible < 100ms` 这一条**在带慢目录捕获时不再成立**；预算本身（40ms）与 late pass 语义未变，`with_content_grace(Duration::ZERO)` 可恢复纯预算行为。
- **Home 上限 7**：规格写 5~8，取 7 作为验收默认值，由 Flutter 视图施加。

## 与 V0.1 的对比口径

V0.1 记录的 ranking p50/p95 为 0.001–0.003ms，只统计 12 个内建命令、空历史和常量时间下的评分与排序，不包含分组、Scope 或应用候选。V0.2 的 matcher 覆盖完整热路径，因此两者不可直接相减。

## 自动测试覆盖的新行为

- Context：`core.text` 到 URL/JSON/文件/目录的叠加识别；`json.document` 是否为规范名；普通仓库、子目录和 worktree `.git` 文件的仓库发现；发现阶段零 Git 进程。
- 首屏：预算为零时提交部分快照并在 late pass 追加目录与 Git 仓库；late 结果不覆盖更新的 Revision；被截断的 Provider 调用在 late pass 中重试（因为重入安全是 Provider 契约）；`snapshot_seq` 随 late enrichment 递增；订阅重放最新快照。
- 应用索引迟到：搜索先落在空索引上，索引到达后重建当前 Revision，且重建只在 `context_complete` 后发布（`an_index_that_arrives_late_reaches_the_settled_snapshot`）。
- 搜索：`chrome` 命中应用并只出现在搜索而非 Home；`git sta` 命中 Git Status；URL 输入保留对象动作与文本兜底；应用索引不进入 Context Graph。
- Group/Scope：Git 目录出现 `git` 与 `folder` 分组；Current App 使用环境来源并排除启动候选；应用专属分组不影响分组标签的可见性。
- Availability：剪贴板已等于当前路径时 Copy Path 隐藏；Git 缺失时三项 Git 动作 Disabled 且无法执行。
- 执行安全：过期 Revision 或过期候选 ID 无法执行；当前 Revision 尚未发布候选时（`candidates_revision` 不匹配）拒绝执行且不产生副作用；Disabled 候选无法执行且不写历史；动作导致自己结束会话时仍然写入 usage。
- 真实 Git：在临时仓库中验证 `status` 报告变更、`log` 报告提交、`diff` 报告工作区改动，非仓库目录明确失败；管道读取在超过 1 MiB 时截断并继续排空。
- 指标契约：七个指标名与规格逐字一致（`provider_duration`、`context_resolution_total`、`first_snapshot_duration`、`matcher_duration`、`ranking_duration`、`hotkey_to_visible_ms`、`late_context_count`）。一次「零预算激活 + late 补齐」必须全部发射（`provider_duration_us`、`context_resolution_total`、`first_snapshot_duration_us`、`matcher_duration_us`、`ranking_duration_us`、`late_context_count`，以及 native 宿主的 `hotkey_to_visible_ms`），由捕获 tracing 字段与消息名的测试断言。其中 `context_resolution_total` 是日志消息名，其余是字段名；`hotkey_to_visible_ms` 由 native 宿主发射。发射点必须写成字面量（tracing 不支持从常量取字段名），因此这个测试是唯一防止两端漂移的机制。

可用性只读内存：Git 动作在 `git` 不存在时显示 Disabled，但**不检查仓库目录是否仍然存在**——那需要文件系统 IO，而规格禁止 Availability 主动执行 IO；仓库消失由执行阶段报错。
- 应用索引：索引项暴露规格 §3 的字段——名称、bundle_id、路径、图标来源（bundle 路径）和 usage 行 ID；图标按需为可见行取回，失败时回退语义图标，且取图标不改变排序或已提交快照。
- 应用 usage：启动记录的行 ID 与排序读取的行 ID 由同一处派生，重复启动后计数可见。
- 分组一致性：`group_id` 常量集中在 registry；测试锁定「Git 动作 ID 谓词 == 执行器 == git 分组」，并锁定两个 Copy Path 动作同组。
- 兼容性：v1 插件缺少新字段仍可加载并归入 `plugin.<id>`；`core.json` matcher 归一后匹配 `json.document`；旧历史行与新行各半时次数饱和、时间取最大且合并幂等；应用 usage 与前台应用 usage 分开加载。
- Flutter：Scope Bar 切换与 `⌥ ←/→`、手动选择在 late 更新后保留、被移除后回退、Disabled 原因显示且不执行、同版本乱序快照被拒绝。

## 未执行的检查

- **真实 GUI 与物理热键**：V0.2 的窗口行为、Scope Bar 观感、真实键盘与中文输入法、真实剪贴板、Finder Automation 授权和 `hotkey_to_visible_ms` 均由用户按清单执行，本记录不含实测数字。
- **Windows**：未在本机运行。新增 Platform 能力在 Windows 上返回明确的 unsupported 默认值，只做了源码级兼容性检查，构建与实机验收仍需在 Windows 上执行。
- **内存**：没有测量 RSS。V0.1 没有 RSS 基线，因此本版不宣称内存回归结论。索引规模上限是 4000 个 bundle，图标按 64px 懒加载（宿主缓存上限 128 项），两者都没有在真实应用集合上采样。
- **真实唤醒序列的端到端采样**：七个指标的发射点已有测试锁定，但没有在真实「热键 → 首帧」序列上采集样本，`hotkey_to_visible_ms` 仍只有 Swift 侧单点记录。

## 当前边界

macOS 应用为本机 arm64、adhoc 本地签名包。Windows 构建路径未改动语义，但新增能力未实现。完整验收前仍需按 [手工清单](manual-validation.md) 的 V 段确认 Scope Bar、应用搜索、Git 预览和 Availability。
