# V0.5 MVP：Developer Actions 与可返回的进程会话

> 历史记录：仅描述文中日期与构建，不作为当前使用或实现契约。当前入口见 [文档导航](../README.md)。

日期：2026-10-06。分支 `dev-0.5`，基线为 `dev-0.4` 的 `b622811`。Rust 开发版本 `0.5.0-dev.3`，macOS App `0.5.0+11`。

V0.5 规划的功能与工程收尾已完成；2026-10-03 完整标准检查通过，四组真实工具和本机主要窗口生命周期此前验收通过。交付为开发包，不将这些结果等同于正式发布、跨平台或完整安装升级验收。2026-10-06 再次完成完整检查、四组真实工具验收和当前源码构建。逐项证据见 [最终功能核对](v0.5-closeout.md)，交付清单与复现入口见 [0.5 收尾记录](v0.5-closeout.md)。

## 功能与入口

```text
Finder / 用户输入的 Directory
  → 独立 ProjectProvider（轻量识别，不启动命令）
  → Project Context 与安全能力元数据
  → 统一 Action / Current App / 项目 Scope / Ranking
  → ExecutionSessionManager
  → Running Tasks 独立窗口
```

| 类型 | 识别标记 | 动作 | 首页推荐资格 |
|---|---|---|---|
| Git | 已有 Git Repository Context | Git Status / Diff / Log（原预览流程） | 复用原规则 |
| Rust | Cargo.toml | Cargo Check / Test；按 package / binary 明确列出 Cargo Run | Check |
| Maven | pom.xml | Maven Test / Package | Test |
| Gradle | build.gradle / build.gradle.kts | Gradle Build / Test | Build |
| Node | package.json | 所有支持的已声明脚本；npm / pnpm / yarn | Build（须已声明） |
| Spring Boot | 本地 Maven / Gradle 显式启用 Boot 插件 | Maven Spring Boot Run / Gradle Spring Boot Run | 无 |

首页及空查询 Current App 保持最多七条推荐；Current App 搜索聚合当前 Context 的全部适用动作。主动选择 Node、Rust 等功能 Scope 时，空查询展示该组全部适用动作，列表滚动，不再用首页推荐资格隐藏脚本或运行目标。App 菜单继续使用原菜单浏览入口。

## 识别、身份和执行契约

### 共用规则

- 每种项目独立查找最近的 manifest，最多检查 32 层祖先；同一目录可识别多种类型。一种识别失败不影响其他类型和已有 Folder/Git 动作。
- Provider 只读有界本地元数据；单份 manifest 最多 256 KiB。识别、搜索和 Scope 切换不执行构建工具，也不安装依赖。
- 静态 ActionBundle 提供固定动作；ContextActionFactory 仅从当前已捕获能力生成动态动作。生成的 Registry 属于本次快照，不向全局 Registry 累积项目脚本；Core 主流程没有 Node/Cargo 分支。
- ActionId 表示业务命令，Candidate 保留具体执行 Context 与凭证。执行前重新检查目录、manifest、脚本/manager/运行目标；配置删除或改变时返回失败，不创建虚假任务。
- 动态 ID 不依赖会话、扫描顺序、脚本正文或目录路径。已有 npm 五个常用动作保留原 ID；其他脚本由 manager 与 UTF-8 名称编码，Cargo Run 由 package 与 binary 编码。相同命令可以匹配不同项目，但执行使用选中的项目目录。
- Maven/Gradle 优先使用最近祖先 wrapper；无执行位时由 `/bin/sh` 执行 wrapper，参数单独传递。仅找不到 wrapper 时回退本机工具，wrapper 执行失败不静默切换版本。
- 工具路径支持继承 PATH、Cargo、SDKMAN current、Volta、NVM current/default、Homebrew 和系统目录；不读取 shell 初始化脚本。保留已有 JAVA_HOME，缺失时可补 SDKMAN current Java；不自动替用户切换 JDK。

### Node scripts

- 最多 128 个 script；提取非空字符串脚本的名称，不把正文传入 Context、使用历史或日志。名称须非空、最多 200 字节、无控制字符、不以 `-` 开头；空正文和非字符串值不产生动作。
- `packageManager` 的显式 npm/pnpm/yarn 声明优先；否则查找最近祖先的管理器声明或 lockfile / pnpm workspace 标记。没有标记默认 npm，冲突标记产生诊断，不猜测工具。
- 执行 `<manager> run <script>`，保留工具标准 pre/post hooks 行为。pnpm/yarn 可回退 Corepack；缺少工具、依赖或项目初始化时显示真实错误，不自动执行 install。
- pnpm 将 `/.../` 解释为匹配多个脚本，因此本版不支持这种脚本名并给出诊断；普通含冒号、空格的名称作为一个参数传递。
- dev/start 及其他长脚本可持续运行并停止。所有脚本都由用户显式选中后启动。

### Cargo Run

- 读取当前 package 和显式 workspace members，支持 glob 和 exclude；只遍历 workspace 根目录内的成员，最多 64 个 package、128 个运行目标，每次目录/glob 枚举最多 128 项。
- 支持显式 `[[bin]]`、`src/main.rs`、`src/bin/*.rs`、`src/bin/<name>/main.rs` 和 `autobins = false`；显式 bin 已占用的源码不会重复推断，缺失源码不提供 Run。
- 每个动作明确显示 package / binary；执行带 `--manifest-path`、`--package`、`--bin` 的 cargo run，不猜默认入口。[Cargo Run](https://doc.rust-lang.org/cargo/commands/cargo-run.html)。
- 不运行 cargo metadata 做识别，不解析依赖或执行 build.rs。畸形或超限 manifest 的运行目标不展示，Check/Test 保留以便查看 Cargo 的实际诊断。

### Spring Boot

- `project.maven` / `project.gradle` 推导 `java.spring_boot`；Maven 识别本地 build plugins 中显式声明的 Boot 插件，Gradle 支持字面量 id / apply plugin，排除注释、示例字符串和 apply false。
- 动作使用 wrapper 执行 `spring-boot:run` 或 `bootRun`，同样进入会话系统。[Maven Boot Run](https://docs.spring.io/spring-boot/maven-plugin/run.html)、[Gradle Boot Run](https://docs.spring.io/spring-boot/gradle-plugin/running.html)。
- 不推断有效 POM、激活 profile、远程父项目、Gradle version catalog 或任意 DSL 的效果；没有本地显式启用证据时不展示 Boot 动作。多模块依赖分析不是本版范围。

## 会话与窗口

- Execution Session 独立于 Launcher Session。关闭 Launcher、再次唤醒、切换查询、关闭或最小化任务窗口均不取消任务；重新进入可查看原任务和输出。正常退出 App 结束所属进程组；会话不跨 App 退出恢复。
- 状态包括运行中、停止中、成功、失败、已停止，带退出码、结束时间及独立 stdout/stderr；不同输出流不承诺精确时间顺序。
- 最多四个并发任务、32 个保留会话；达到保留上限只淘汰最早完成的结果。每个输出流保留最后 128 KiB，截断后继续排空管道并提示。
- 启动成功自动打开 Running Tasks；可选择任务、查看输出、Stop、移除已完成结果。窗口显示失败时保留任务并提示从菜单返回，避免重复启动。
- 隐藏/最小化窗口停止 Flutter 轮询，恢复后继续。初始化期间的可见性变化不会被迟到的 ready 响应覆盖；重新打开最小化窗口恢复显示。
- App 无可见窗口时重新打开 App 恢复 Launcher，可从菜单 Running Tasks 回到后台任务。

## 自动化验收记录

- 2026-10-03 `scripts/check.sh` 通过：**246 项 Rust 测试、64 项 Flutter 测试**；另四项真实工具测试默认忽略，此前已单独运行通过。fmt、Clippy、静态分析、架构和原生检查通过。本次未重复运行四组真实工具测试。
- Application 门面覆盖 Context → Scope → Action → Session → 状态/退出码/历史；Rust 真实临时 workspace 完成 Check/Test/多个 Run，真实测试失败返回非零。脚本或 bin 删除、manager 改变后拒绝旧动作，重新捕获后移除失效动作/空 Scope。
- Node 动态 ID 跨会话与重扫稳定，脚本正文不进入 Context；wrapper 参数、含空格/引号路径、畸形/超限配置、并发上限、输出截断、停止子进程和管理器关闭有回归覆盖。
- Flutter 覆盖自定义脚本和 Cargo Run 的 Scope 可发现性、旧详情迟到、隐藏停止轮询、初始化可见性竞态、操作失败及窗口打开失败。

### 四组真实工具验收：通过

| 组别 | 实际验证 |
|---|---|
| npm | 真实 Build/Test、文件产物、脚本移除后的隐藏/拒绝 |
| Maven / Gradle | 真实 Java 编译及 jar/class；JUnit 成功与故意断言失败，退出码和失败状态正确 |
| npm / pnpm / yarn | 自定义 check:types 成功、lint 非零失败、dev/start 长服务启动和停止；Yarn fixture 已按工具要求完成初始化 |
| Spring Boot | 真实 Boot 4.1.1 Maven 与 Gradle 应用启动，捕获 Java PID，Stop 后确认服务进程结束 |

Java 验收使用显式 JAVA_HOME 的 JDK 25、Maven、Gradle 9.7.1；Node 验收使用 npm、Corepack、pnpm 10.0.0、Yarn 4.9.2。默认 Java 8 与 Gradle 不兼容时曾正确失败，未把环境错误当成成功。停止真实服务时等待终止与回收完成后确认进程状态，不以瞬时 kill 探测判断泄漏。

```bash
# 需本机 Node/npm/Corepack、Maven/Gradle、兼容的 JAVA_HOME；首次执行可能下载工具缓存和构建依赖。
# Gradle 未在搜索路径时可指定 ANYWHERE_TEST_GRADLE_EXECUTABLE。
cargo test -p anywhere-core --test mvp --locked developer:: -- --ignored
```

## 本机原生窗口验收

平台：macOS 27.0.1 arm64，开发 App 0.5.0+6。

| 场景 | 结果 |
|---|---|
| 输入真实 Node 项目路径，找到 Scope 与自定义脚本 | 通过 |
| Finder 选中项目目录，空查询 Node Scope 展示全部脚本并执行 custom:check | 通过，真实输出与 Completed (0) 正确 |
| 启动 npm dev，独立窗口显示真实服务 stdout | 通过 |
| Launcher 与任务窗口切换焦点 | 通过，Launcher 隐藏，任务窗口可操作 |
| 最小化后从 Running Tasks 恢复 | 通过，任务和输出保留 |
| 关闭任务窗口、重新打开 App、从菜单返回任务 | 通过，新的窗口连接原任务和输出 |
| 点击 Stop 结束服务 | 通过，UI 显示 Stopped，服务 PID 已结束 |
| 有运行中的 Node 服务时正常退出 App | 通过，App 与服务子进程均结束 |

原生 GUI 验收通过 App 重开和菜单入口进行；全局唤醒快捷键未由这轮自动化证实。Finder 多选不同项目、不同机器精简 PATH、所有权限组合和安装升级完整矩阵未重复验收。任务窗口的 Flutter 内容未完整暴露给本次原生 AX 自动化，视觉与点击验收通过，但不能据此宣称读屏支持已验收。

## 通用窗口操作收尾（0.5.0+9）

- 设置页使用独立原生窗口，支持关闭 / 最小化、⌘W / ⌘M、拖动标题栏；固定布局禁用缩放。任务窗口支持关闭 / 最小化 / 调整大小，Window 菜单统一提供通用操作。
- 设置页与 Launcher 共用现有 Flutter 引擎，关闭时移回 Launcher；不增加设置专用引擎。最小化后重新打开设置保留草稿与滚动位置。
- Done、Esc、原生关闭、切换 Launcher 和退出 App 共用关闭保护：等待进行中的保存，未应用文本提示保存 / 放弃 / 取消；保存失败保留设置页与错误提示。
- 本机验证原生关闭 / 最小化 / 恢复、未保存草稿取消 / 放弃、退出取消、返回 Launcher 输入及任务进程生命周期；保存成功 / 失败、保存等待与快捷键录制的 Esc 行为由新增 4 项测试覆盖。

## 操作逻辑收尾（0.5.0+8）

- 任务按动作名和项目路径筛选，⌘F 查找、Enter 回列表、↑↓ 选择，选中项保持可见；Esc 先清筛选再隐藏。
- ⌘L 或窗口按钮返回 Launcher 并可立即输入；Scope 点击和菜单返回恢复搜索输入焦点，底部 Run / Open 可点击。
- 日志默认跟随末尾，向上滚动暂停跟随；Latest 恢复。Output / Diagnostics 分页独立复制，首次打开已失败任务默认显示诊断。
- Stop 与 Remove result 分离，移除放入更多操作；菜单操作固定绑定任务 ID，防止切换任务后作用于另一项。操作错误不会被自动刷新立即清掉。
- macOS Tasks 菜单提供查找、停止、刷新、返回 Launcher 和关闭窗口；任务窗口首次创建时设置原生键盘焦点，局部键盘事件处理保证 ⌘. 停止而不会被 AppKit 转为 Esc。
- 成功发现空菜单属于正常状态，不再向 Universal Search 显示“没有菜单动作”的警告；权限失败和超时仍保留诊断。
- 本轮 **60 项 Flutter 测试与静态分析通过**，Rust 空菜单回归通过。原生已验证首次打开无鼠标 ⌘F 输入、Esc 清筛选、⌘. 停止且保留窗口、服务进程结束、⌘L 返回并直接输入。
- 具体操作及验证边界见 [UI 交互收尾](../ui-design.md)。

## 任务窗口 UI 收尾（0.5.0+7）

- Launcher、设置和任务窗口复用同一主题、系统字体、蓝灰文本、图标底板与蓝色选中边框；去除默认 Material 紫色和宽大的 ListTile。
- 原生磨砂材质覆盖窗口及标题栏，Flutter 留出交通灯区域；任务列表、详情、日志面板的层级与原 Launcher 保持一致。
- 标题、状态、退出码与目录分开展示；命令及完整路径默认折叠，可展开选择复制。日志采用 Menlo，诊断输出使用柔和警示色。
- 小窗口的元数据可独立滚动，保留日志空间。新增最小 760×420 窗口下的长标题、长路径和长命令回归，验证展开后输出和 Stop 仍可使用。
- 0.5.0+7 实机确认标题栏与内容区磨砂连续、真实任务 Completed / Exit 0、命令展开、日志显示和 Remove 后空状态；55 项 Flutter 测试和静态分析通过。
- 本轮只调整 Host 展示和原生窗口材质，保留现有会话执行与停止规则。

## 交付与范围

- 开发 App：`build/Anywhere-dev-0.5.app`；ZIP：`build/Anywhere-dev-0.5-macos-arm64.zip`；校验文件：同名 `.zip.sha256`。版本 0.5.0，当前构建号 11，使用固定本机签名。原 0.4 App 单独保留。
- `scripts/package-macos.sh` 校验源码与 App 版本、固定签名及归档内的主程序/Rust 库一致性，避免打包旧版 App；从当前 checkout 构建与打包的命令见 [收尾记录](v0.5-closeout.md)。
- 本轮没有改变 bridge 公共 DTO 字段；沿用已有生成绑定。新增的 Context 类型走现有通用 Project 和字符串属性。
- 0.5 功能范围已交付。正式发布前仍需多机器安装升级/权限验收和读屏检查，不能把开发包当作已完成这些验证的正式发行版。
- 任意 Gradle task、Spring 多模块分析、交互式 stdin/PTY、任意自定义 shell、跨 App 退出存活的守护服务属于后续版本。Git 已有 Status/Diff/Log 继续原预览，不重复开发或强制迁移。

## 后续 Git 扩展（0.6.0+20）

后续 `0.6.0+20` 已扩展 Git 分支/提交哈希/仓库路径复制、本地分支和暂存区差异，仍复用原只读预览与执行路径，见 [Git 常用动作](../developer-actions.md)。

## 菜单身份安全修复（0.5.0+10）

菜单 ActionId 与扫描执行 token 分离，旧候选、排队请求及标题/标识变化不会执行替代动作。菜单 frecency 暂停读取和记录，保留旧数据；Developer Actions 与其他动作历史正常。长期菜单身份仍待验证，修复和限制见 [菜单动作说明](../current-app-actions.md)。本轮原生验证使用合成 AX 节点，未重复真实应用菜单与四组真实工具验收。

## 可维护性修复（0.5.0+11）

统一 Registry 重载失效凭证，拒绝排队中的旧动作与旧列表项；补持久绑定资格、内部身份边界。设置、退出、热键和窗口管理拆出独立职责，键码与标签由同一配置生成。Node/Cargo 项目元数据采用共享结构与版本校验，非法格式有诊断。完整检查通过：259 项 Rust、66 项 Flutter 测试及原生服务回归。尚未实现 V0.6 用户快捷键产品功能；真实窗口、物理热键与权限未在本轮重复验收。详见 [修复记录](reviews/review-maintainability-v0.5.md)。
