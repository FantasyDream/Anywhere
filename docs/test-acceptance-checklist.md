# 当前 GPUI 完整测试用例与验收清单

编制日期：2026-11-17。对象为当前 `codex/experiment-gpui-kit` 工作区的 V0.14
macOS GPUI 产品，基线 HEAD 为 `b428899c9ce70c92e12de24950405222ea3b6c28`，
**包含未提交修改**。本清单覆盖现有功能、异常、权限、异步竞争、资源和发布边界，
不把路线图功能当作已交付功能。主分支 Flutter 包仅可作为旧交互对照，不是本清单
的被测对象。

这是一份待执行清单，不是通过报告。编制时没有启动、重新打包或实际验收 App。
已有 `build/Anywhere-GPUI.app` 属于前一轮打包快照，不包含最近一轮源码修复；
执行本清单前必须从本次待测工作区重新构建并记录身份。

当前执行记录见
[GPUI 当前构建验收执行记录](archive/reviews/gpui-acceptance-run-2026-11-17.md)。
该轮只完成部分条目，并在 macOS 锁屏处阻塞；未完成项仍然不能视为通过。

## 1. 使用规则与结果记录

### 1.1 优先级、方式与状态

| 字段 | 含义 |
| --- | --- |
| P0 | 启动阻断、错误目标执行、越权、数据破坏、正文泄露等必须阻断交付的问题 |
| P1 | 主功能、键盘、焦点、持久化、取消和主要异常路径 |
| P2 | 次要边界、外观、扩展环境与测量；仍需执行或明确说明缺口 |
| M | 真实 App / 系统 / 物理输入手工验收 |
| A | 使用自动化、合成数据或可控故障 / 排序测试验收 |
| H | 自动化验证状态和授权，再由真实 App 验证交互或 OS 副作用 |
| Pending | 未执行；本文所有分项的初始状态 |
| Pass / Fail | 已执行且有本次构建的预期 / 实际对照及证据 |
| Blocked | 缺少工具、应用、设备、故障注入或授权条件；不能当作通过 |
| N/A | 本次明确不支持的范围；必须写原因，不能代替未执行 |

A / H 是执行方式，**不表示该行已被现有单测逐项覆盖**。第 8 节列出可复用的
自动化入口；仍须核对具体断言。乱序和排队竞争优先使用可控 channel / 阶段门禁，
不以快速点击一次未出错代替确定性测试。M / H 涉及的物理 IME、焦点、VoiceOver、
Quick Look、WebKit 和内存趋势，不能从构建或状态单测推导。

每条用例按以下模板建立执行记录；一个用例在不同环境的结果分开记：

```text
Run ID / 执行人 / 时间：
用例 ID / 参数（来源 ID、目标类型、语言、显示器等）：
源码 HEAD / 未提交差异清单 / App 绝对路径 / 主程序 SHA-256：
App 版本 / 构建号 / 签名 / 架构 / macOS / 数据目录：
前置条件与实际操作：
Expected：
Actual：
Result：Pending / Pass / Fail / Blocked / N/A
证据：截图、录像、脱敏日志、测试命令和退出码、样本数据位置
缺陷编号 / 复现频率 / 修复后复验 Run ID：
```

### 1.2 验收顺序与完成条件

建议按“身份与隔离 → P0 冒烟 → 各模块主链路 → 异常与乱序 → 环境矩阵 →
性能 / 资源 → 发行包”执行。每轮至少覆盖启动、两种唤醒、实际打字与 IME、执行
一个无破坏动作、打开 / 返回工具、Settings 保存、隐藏恢复和退出。

当前开发预览的完成要求：范围内 P0 / P1 有本次结果，无未处理的错误目标执行、
越权或数据破坏；自动化门禁通过；人工缺口、P2 失败及 Blocked 逐项列明，不能
写成“全量验收通过”。18 来源真实打开、不同设备和系统版本的覆盖分别报告。
正式公开分发还需要 REL 组的发行条件，不能用本机签名替代公证。

### 1.3 覆盖概览与跳转

共 **322 条基础用例、26 个模块**，其中 P0 82 条、P1 225 条、P2 15 条；方式为
M 146 条、H 147 条、A 29 条。另有18个来源和环境参数矩阵，展开后执行次数会更多。
所有基础用例和来源矩阵初始均为 Pending。

| 模块 | 基础用例数 |
| --- | ---: |
| [ENV：身份、启动、权限与隔离](#61-env身份启动权限与隔离) | 12 |
| [LCH：Launcher、导航、Scope 与保留](#62-lchlauncher导航scope-与保留) | 14 |
| [KEY：物理键盘、IME 与焦点](#63-key物理键盘ime-与焦点) | 12 |
| [CTX：来源、默认输入与对象语义](#64-ctx来源默认输入与对象语义) | 14 |
| [APP：应用索引、启动与图标](#65-app应用索引启动与图标) | 8 |
| [MENU：Current App 菜单与安全降级](#66-menucurrent-app-菜单与安全降级) | 10 |
| [ACT：URL、原生对象动作、Usage 与输出](#67-acturl原生对象动作usage-与输出) | 10 |
| [FILE：文件搜索、授权、遍历与对象动作](#68-file文件搜索授权遍历与对象动作) | 22 |
| [PRE：Quick Look 与预览资源](#69-prequick-look-与预览资源) | 12 |
| [CLIP：剪贴板采集、历史与存储策略](#610-clip剪贴板采集历史与存储策略) | 16 |
| [SHELF：独立悬浮窗](#611-shelf独立悬浮窗) | 10 |
| [SET：设置、草稿、保存与登录项](#612-set设置草稿保存与登录项) | 16 |
| [HOT：快捷键、Context 分派与配置兼容](#613-hot快捷键context-分派与配置兼容) | 12 |
| [HIST：应用历史、来源设置与目标权威](#614-hist应用历史来源设置与目标权威) | 20 |
| [GIT：仓库识别与只读 Git 动作](#615-git仓库识别与只读-git-动作) | 10 |
| [DEV：项目识别、构建 / 运行与收藏目录](#616-dev项目识别构建--运行与收藏目录) | 12 |
| [TASK：后台进程与 Running Tasks](#617-task后台进程与-running-tasks) | 14 |
| [PKG：Web 包安装、批准、更新和撤销](#618-pkgweb-包安装批准更新和撤销) | 12 |
| [WEB：WebView、SDK、权限与生命周期](#619-webwebviewsdk权限与生命周期) | 17 |
| [JSON：JSON Workspace 与三个后台复制命令](#620-jsonjson-workspace-与三个后台复制命令) | 12 |
| [BG：第三方 Context 与有限后台执行](#621-bg第三方-context-与有限后台执行) | 10 |
| [DSL：内置模块开关与未知目标隔离](#622-dsl内置模块开关与未知目标隔离) | 3 |
| [UI：布局、材质、本地化与可访问性](#623-ui布局材质本地化与可访问性) | 12 |
| [PERF：性能、内存与资源趋势](#624-perf性能内存与资源趋势) | 10 |
| [REL：工程门禁、持久化升级与发行](#625-rel工程门禁持久化升级与发行) | 9 |
| [RACE：跨模块高风险组合回归](#626-race跨模块高风险组合回归) | 10 |

首轮冒烟建议至少执行：ENV-001/002/005、LCH-001/002/008/010/014、KEY-001、
ACT-002、FILE-001/005/017、CLIP-001/002/007、SHELF-001/006、SET-004/005、
HIST-002/008/011、DEV-010、TASK-001/005/006、PKG-001/003、WEB-001/012/015、
JSON-008、REL-005/006。此子集通过只表示冒烟通过，不代表322项完整验收。

## 2. 被测环境与测试数据

### 2.1 构建、隔离与身份

下面是**由执行者运行**的准备命令，本次编制没有执行构建或启动：

```bash
git rev-parse HEAD
git status --short
git diff --stat
scripts/check-gpui.sh
scripts/check.sh
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-acceptance.app" scripts/build-macos.sh
codesign --verify --deep --strict build/Anywhere-acceptance.app
plutil -lint build/Anywhere-acceptance.app/Contents/Info.plist
shasum -a 256 build/Anywhere-acceptance.app/Contents/MacOS/Anywhere

qa_root="$(mktemp -d "$PWD/build/acceptance-XXXXXX")"
mkdir -p "$qa_root/data" "$qa_root/bundled-plugins" "$qa_root/fixtures"
ANYWHERE_DATA_DIR="$qa_root/data" \
  ANYWHERE_PLUGIN_DIR="$qa_root/bundled-plugins" \
  "$PWD/build/Anywhere-acceptance.app/Contents/MacOS/Anywhere"
```

先正常退出旧实例并确认实际运行的可执行路径。`ANYWHERE_PREVIEW=1` 只适合
视觉预览壳，不得用于判定真实搜索、存储、插件、权限和任务通过。数据目录覆盖
只隔离业务文件，**不隔离系统剪贴板、AX / Automation 授权、全局热键和登录项**。
这些项目在专用测试用户或测试机器上执行；测试全局键先配置不冲突的键位。
有关路径约束见[独立验收数据](development.md#独立验收数据)。

当前[偏好保存实现](../apps/gpui/src/host/runtime/settings.rs)在显式数据目录下跳过
`login_item::apply`。SET-015 仍需对照 macOS 登录项验证实际行为。需要触发系统注册、
审批或失败后重试的用例使用专用测试用户的默认数据目录。

### 2.2 数据集

| 编号 | 内容与准备要求 |
| --- | --- |
| F0 | 上述隔离数据目录、脱敏证据目录、当前构建；保存初始剪贴板和系统设置基线，结束后由执行者恢复 |
| F1 | 授权 root 内创建普通 / 中文 / 空格 / 标点 / emoji 名称、隐藏目录、各文件类型、深层目录、内部链接、越界链接、断链、硬链接；root 外另建同名对象；不使用个人目录做删除或故障测试 |
| F2 | 剪贴板合成文本、URL、真实测试路径、严格 JSON、转义 JSON、中文、emoji、图片、HTML / RTF、多个文件引用；容量 / 64 格式边界由测试 helper 构造；不得使用密码或个人历史 |
| F3 | 临时 Git 仓库：有提交、未提交、未暂存 / 已暂存差异、无提交、detached HEAD、无 remote、多 remote、origin 多 URL、linked worktree；Git 身份只设在测试仓库，不访问远端 |
| F4 | 最小 Maven / Gradle / Node / Cargo / Spring Boot 项目，以及短任务、非零退出、持续输出、持续运行并派生子进程的测试任务；命令仅作用于测试目录；记录各工具版本 |
| F5 | 18 来源的合成 XML / SQLite / JSON、目录 / 文件 / workspace / remote / vault 目标、无效文件和损坏配置副本；原应用打开使用已安装的对应应用，合成解析通过不代替原应用打开 |
| F6 | 当前 JSON Workspace、Text Pad、JSON Summary；另备缺权限、损坏、更新 / 回滚、非法 Manifest、超限 / 超时和迟到回复的受控插件 fixture；只装入测试数据目录 |
| F7 | macOS 权限授予 / 拒绝 / 撤销，浅深外观、辅助显示选项、中文 IME、多屏 / 缩放 / 小屏 / Space、无网络；系统相关状态使用测试账户，不修改日常环境 |

F1 的规模分为少于一屏、跨页 200 项、10,000 项，以及专项的 50,001 个同查询
命中；大规模扫描待完成后才判断完整性。NFC / NFD 和大小写用例需考虑文件系统
是否允许相应名字同时存在，不要求在大小写不敏感盘上创建两个同名文件。

F5 可复用[来源解析回归](../crates/anywhere-core/src/application/application_history/adapters_tests.rs)
及[MVP 历史回归](../crates/anywhere-core/tests/mvp/application_history.rs)的结构。
SQLite 样本只建 `ItemTable(key, value)`，键为
`history.recentlyOpenedPathsList`，值含 `entries` 数组；目标分别使用
`folderUri`、`fileUri`、`workspace.configPath` 与 `vscode-remote:` URI。
JetBrains 样本使用唯一含 `additionalInfo` 或 `recentPaths` 的 component；
Sublime 样本使用 `folder_history` / `file_history` / `workspaces.recent_workspaces`；
Obsidian 使用 `vaults` 下 16 位十六进制 ID 和测试路径。测试文件与目标都须真实
存在；无安装应用的真实打开记录 Blocked，不能只测复制就勾选打开。

## 3. 环境执行矩阵

同一用例无需机械执行全部笛卡尔积，但以下每一类都须有覆盖记录。组合风险至少
补“中文 IME + WebView + 隐藏恢复”“多屏 + Shelf + Quick Look”“减少透明度 +
浅色外观 + 深色浮层”。

| 维度 | 必测参数 | 优先覆盖 |
| --- | --- | --- |
| 系统 / 架构 | macOS 15 最低版本、实际使用的新版本；arm64；声称支持 Intel 时补 x86_64 本机包 | 启动、签名、热键、AppKit、Quick Look、WebKit、登录项 |
| 语言 | 跟随系统、简体中文、English；切换后重启 | 文案、搜索别名、菜单、错误、插件；正文不被翻译 |
| 输入 | 英文、真实拼音 IME、组词 / 候选 / 取消、粘贴、连续按键 / repeat | Launcher、各搜索框、Settings 录制、Web 编辑器 |
| 权限 | 未授权、允许、拒绝、运行中撤销、重新授权；部分目录可读 | 菜单、选区、Finder / 浏览器、文件、剪贴板 |
| 显示 | 单屏、双屏、不同缩放、鼠标换屏、小可用区域、Dock / 菜单栏占位、全屏 Space | 位置、顶部锚点、尺寸、Shelf 右下角、焦点 |
| 外观 | 浅色 / 深色、壁纸变化、Reduce Transparency、Increase Contrast、Reduce Motion | 材质、文字、边界、浮层、窗口缩放 |
| 生命周期 | 冷启动、暖唤醒、失焦、隐藏小于 / 大于 15 秒、休眠唤醒、锁屏恢复、退出重启 | 输入、执行资格、数据持久化、资源 |
| 数据 / 故障 | 空、少量、跨页、容量边界、损坏、不可读、磁盘不足、工具缺失 | 空态、错误、重试、取消、原数据不被破坏 |
| 安装状态 | 首装、同签名替换、升级、包移位 / 解压后运行；多实例应用 | 路径身份、权限保留、配置迁移、拒绝歧义 |

## 4. 关键限额与边界取值

对每个硬限额至少取“0 / 1、上限减一、上限、上限加一”，时间类使用可控时钟
验证边界，再补真实等待。字节按 UTF-8 统计，不把字符数当字节数。

| 对象 | 当前源码限额 / 规则 | 用例组 |
| --- | --- | --- |
| 会话与自动填入 | 保留 15 s；近期剪贴板 15 s；选区优先等待 500 ms；读取不续期 | LCH、CTX、RACE |
| 文件范围 | 最多 8 roots、32 排除名；普通路径 4,096 B、排除名 128 B | FILE、SET |
| Action 绑定 | 最多 128 绑定、每意图最多 16 routes | HOT |
| 剪贴板 | 1,000 条、128 MiB；单条 16 MiB、64 格式；保留 1 / 7 / 30 / 90 天或容量限制 | CLIP |
| 开发任务 | 最多 4 个运行会话、保留 32 会话；stdout / stderr 各 128 KiB | TASK |
| 应用历史 | 输入 8 MiB、转换输出 2 MiB、4,096 条；标题 4 KiB、目标 16 KiB；单来源总截止 5 s | HIST、SRC |
| 历史脚本 | XML 50,000 节点 / 深度 64；JS 堆 32 MiB、栈 256 KiB、中断预算 250 ms | HIST |
| Web bridge | frame 64 KiB、chunk 4,096 B、正文 1 MiB、transfer 2 MiB；24 pending、8 running、4,096 request IDs、5 s 请求期限 | WEB |
| Web 包 | Manifest 128 KiB、归档20 MiB、2,000 entries、解压80 MiB、单资源16 MiB、压缩比200；图标另受256 KiB限制 | PKG |
| Web 私有KV | key128 B、序列化值64 KiB、命名空间1 MiB / 256 keys；不放宽原生模块256 KiB / 128 keys的预算 | WEB |
| JSON Workspace | 输入 / 输出各 1 MiB、深度 128；查询 50,000 值 / 10,000 matches / 路径 1,024 字符；自动识别原文 16 KiB、最多解码 4 层 | JSON |
| 后台插件 | 每任务 VM 堆 32 MiB、栈 256 KiB、输出 256 KiB；1,024 Host calls / 128 business calls；预填单任务 JS 75 ms、整轮 150 ms 后不再启动额外解析器 | BG |
| Launcher 图标 | 128 项 LRU、最多 4 并发；失败缓存 30 s，后续候选更新才重试 | APP、PERF |

上述限额来自 [runtime policies](../resources/runtime-policies.json)、
[Clipboard](../crates/anywhere-core/src/features/clipboard/history.rs)、
[Process sessions](../crates/anywhere-core/src/core/execution/mod.rs)、
[History model](../crates/anywhere-core/src/features/application_history/model.rs)、
[Web bridge](../crates/anywhere-protocol/src/web_bridge.rs) 和
[后台 VM](../crates/anywhere-core/src/application/web_plugins/background_runtime.rs)。
包和KV预算另见[包验证](../crates/anywhere-plugin/src/web/mod.rs)、
[KV回归](../crates/anywhere-core/src/application/web_plugins/broker_tests.rs)。
它们是单项资源约束，均不等于整个 App RSS / footprint 上限。

## 5. 当前实现与旧说明的易混点

| 项目 | 本次验收使用的依据 |
| --- | --- |
| 正常启动 | [app.rs](../apps/gpui/src/app.rs)的普通 Accessory 路径与无 Core 的 Preview 壳分开验；不凭旧“首次自动显示”文字认定当前正常进程已进入 Ready |
| 默认设置 | [HostPreferences](../apps/gpui/src/host/preferences.rs)当前搜索默认关闭、include_hidden=false，采集来源开关为 true、历史录制默认关闭；文件指南关于新配置隐藏文件的文字须与有效 Host 设置对照，差异记录，不默认为通过 |
| 剪贴板删除 | [input.rs](../apps/gpui/src/launcher/view/input.rs)当前 Shelf 使用 `⌘⇧Backspace` 或对象菜单 / 删除按钮；裸 Backspace / Delete 不应作为列表删除快捷键，编辑器内仍是文本编辑 |
| 应用历史键位 | 行内更多 / 右键打开对象菜单；不假定 `⌘K` 是历史行快捷键；PageUp / PageDown 按可见页移动 |
| 快捷键导入导出 | 旧使用指南提到导入 / 导出，但当前GPUI设置未发现可达UI入口；本清单只验现有绑定编辑、持久化与旧配置兼容，不虚构导入按钮 |
| JSON 示例 | 当前 0.7.1 包通过后台 resolver 产生一个 `.json` Context，三个复制 Action 在后台执行；没有单独 Unescape Action；旧 Host `inputMatchers` 兼容能力与此包实现分别验 |
| 安装批准 | Web 包批准即生效；旧 TOML v2 / v3 新批准下次启动生效；两个流程不能混用 |
| 登录项 | 隔离数据目录不代表隔离 OS 副作用；保存偏好调用系统适配器的边界列为重点实测项 |

## 6. 分模块测试用例

表内前置条件引用 F0–F7。每项初始为 Pending；实际结果填入第 1 节记录，不把
这里的预期当作已验证事实。

### 6.1 ENV：身份、启动、权限与隔离

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| ENV-001 | P0 / M | F0；退出旧实例，从指定绝对路径启动当前包，核对进程、plist、主程序摘要 | 运行的是待测二进制；无旧进程冒充、崩溃或重复主窗口 | Pending |
| ENV-002 | P0 / M | 全新隔离目录冷启动，经状态栏呼出 | 启动到 Ready 后可交互；不是只显示静态 Preview 壳 | Pending |
| ENV-003 | P1 / H | 启动存储 / 索引初始化被受控延迟，期间点击状态栏和热键 | UI 主线程可响应；Starting / Failed 明确，未就绪不执行旧候选 | Pending |
| ENV-004 | P0 / A | 数据目录为相对路径、根目录、含 `..`、不可创建路径 | 明确启动失败，不回退到个人数据目录 | Pending |
| ENV-005 | P0 / H | F0；操作搜索、历史、插件和设置，再检查实际存储位置 | 业务写入仅进入指定数据目录；个人库未被替换或清空 | Pending |
| ENV-006 | P1 / M | 无 AX 权限；搜索应用、URL、文件工具，再查菜单 | 菜单隐藏；其他不依赖 AX 的能力继续工作 | Pending |
| ENV-007 | P1 / M | AX 拒绝 → 授予 → 运行中撤销 → 再授予并重新唤醒 | 按真实权限重扫；旧菜单凭证不可执行，不要求重启恢复其他功能 | Pending |
| ENV-008 | P1 / M | Finder / Safari / Chrome / Edge 分别拒绝 Automation，再允许 | 对应来源降级且提示准确；其他来源不被连带关闭 | Pending |
| ENV-009 | P1 / M | 目录部分授权、全部取消、授权位置移走 / 不可读 | 只使用实际授权范围；空有效范围保持停用，不回退 Home | Pending |
| ENV-010 | P1 / M | 热键被系统或其他应用占用，使用状态菜单进入设置修改 | 冲突可见，仍能操作并恢复；不假报注册成功 | Pending |
| ENV-011 | P1 / M | 启动损坏 / 缺失的单个插件及一个不可用历史来源 | 错误局限于对应能力，Launcher 仍可启动和搜索 | Pending |
| ENV-012 | P1 / M | 连续启动、退出、重启并切换不同测试数据目录 | 无遗留旧实例；数据身份明确，日志与失败可定位本轮 | Pending |

### 6.2 LCH：Launcher、导航、Scope 与保留

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| LCH-001 | P0 / M | 从 Finder、TextEdit、浏览器等应用唤醒 Current App，无需点击直接输入 | 正确置前并获得物理键盘；输入和 Esc 均有效 | Pending |
| LCH-002 | P1 / M | 显式 Global Search 唤醒，外部应用存在选区和新剪贴板 | 进入空白名称搜索，绕过默认自动填入 | Pending |
| LCH-003 | P1 / M | 同入口重复唤醒；切换 Current / Global；窗口已显示时再唤醒 | 同模式根页可切换显示 / 隐藏；模式切换按入口更新，不重复创建窗口 | Pending |
| LCH-004 | P1 / H | 输入动作名称、中文别名、全拼、首字母和零命中查询 | 对应结果正确；零结果不执行上一份列表的选择 | Pending |
| LCH-005 | P1 / H | Current App 内有 Folder / Git / Developer / 菜单，切换 Scope | Current App 聚合可用领域，Scope 只过滤，不改变 Context；无独立 Menu Scope | Pending |
| LCH-006 | P1 / M | 有查询时 Tab / Shift+Tab 及 Ctrl 兼容组合切 Scope | 查询保留、可见 Scope 循环正确、所选标签滚入视口 | Pending |
| LCH-007 | P1 / H | 合成相同动作不同次数 / 最近使用 / relevance，空查询与关键词查询分别测 | 空查询按 Usage → 最近使用 → relevance / 默认优先级 → 稳定 ID；关键词相关性不被频次覆盖 | Pending |
| LCH-008 | P1 / M | 内容页 → Current 空根 → Global 空根连续 Esc；普通查询先 Esc | 内容先返回 Current；Current 再切 Global；Global 最后隐藏；普通查询先清空 | Pending |
| LCH-009 | P1 / M | 菜单 / 对象 Actions / Preview / 工具子页打开后按 Esc 和返回按钮 | 局部层优先关闭；父页 query、Scope、选择按各自契约恢复 | Pending |
| LCH-010 | P1 / M | 有查询或工具页失焦隐藏，分别 5 s / 14 s 内恢复 | 保留会话工作区；预览不自动重开，执行资格重新取得 | Pending |
| LCH-011 | P1 / H | 同序列等超过 15 s，并使旧到期通知晚于新激活到达 | 超期按新 Session；旧计时器不清空新会话或 Settings 草稿 | Pending |
| LCH-012 | P1 / M | 空白首页先在应用 A 唤醒，切应用 B 后再次唤醒 | 重新捕获 B，不沿用 A 的空白环境 | Pending |
| LCH-013 | P1 / H | 动作成功 close_window；同时已有 Preview / Web；再测试执行失败 | 先清理再隐藏；失败仍可操作，错误不冒充成功关闭 | Pending |
| LCH-014 | P1 / M | 状态栏依次 Search、Running Tasks、Settings、Quit，关闭菜单后键入 | 各入口到达正确窗口；菜单结束后目标 key window 正确，无 Dock 图标泄漏 | Pending |

### 6.3 KEY：物理键盘、IME 与焦点

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| KEY-001 | P0 / M | 各输入框用真实拼音输入中文，候选期间按 ↑↓、Enter、Esc、Space、Tab | 候选交给 IME，不提前执行、切 Scope、开预览或退出页面 | Pending |
| KEY-002 | P1 / M | 组合中点击工具、隐藏、再唤醒并继续输入 | 组合态正确结束 / 恢复，不残留 marked text 或永久吞键 | Pending |
| KEY-003 | P1 / M | Launcher / 文件 / 剪贴板 / 历史 / 任务 / 设置 / Web 中分别使用 ⌘A/C/V 与光标编辑 | 修改当前可见编辑器，不影响隐藏输入或背后候选 | Pending |
| KEY-004 | P1 / H | 长按 Enter 执行动作、长按 ⌘Y / Space 切预览 | 一次按下只触发一次副作用 / 切换，不因 repeat 重复执行 | Pending |
| KEY-005 | P1 / M | 含空格的查询正常输入；未选择列表时 Space | 空格是正文，未选中不误开预览 | Pending |
| KEY-006 | P1 / M | 编辑器有选择、面板按钮有焦点时 Space / Enter | 激活当前按钮或编辑行为，不执行背后对象预览 | Pending |
| KEY-007 | P1 / M | 预览关闭后用 `⌘L`、箭头、Home / End / Backspace 继续编辑 | 返回正确页面查询框；关闭预览不丢查询，不抢其他应用焦点 | Pending |
| KEY-008 | P1 / M | 带 Shift / Ctrl / Alt 的方向键在历史和任务编辑器操作 | 保留对应编辑语义；只有明确支持的导航组合改变选择 | Pending |
| KEY-009 | P1 / M | Settings 弹确认后 Tab / Shift+Tab、Enter、Esc、点击 scrim 外 | 焦点限制在当前模态，底层控件不被操作；取消后恢复原焦点 | Pending |
| KEY-010 | P1 / M | WebView 正文聚焦后 Esc；IME 组合时再次 Esc | 非组合时宿主受控返回，组合时不误退出；返回后能立即输入 | Pending |
| KEY-011 | P1 / M | Tasks / Shelf 为 key 时用全局键唤醒 Launcher，再切回 | 键盘归目标窗口；不同时路由到两页，不永久停在旧 responder | Pending |
| KEY-012 | P1 / M | 失焦、最小化恢复、锁屏恢复、多 Space 与热键交替 30 次 | 无“窗口可见但无法输入 / Esc”；记录失败频率与 responder 日志 | Pending |

### 6.4 CTX：来源、默认输入与对象语义

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| CTX-001 | P1 / M | Finder 分别选单文件、多文件、目录、无选中项后唤醒 | Context 类型、来源与动作对应真实对象；不混为单个任意路径 | Pending |
| CTX-002 | P1 / M | Safari / Chrome / Edge 当前有效 HTTP(S) 页；切 Tab / 关窗口后唤醒 | 读取被冻结来源应用的当前页；不把后续别的窗口当原对象 | Pending |
| CTX-003 | P1 / H | 有可识别选区且剪贴板也有效；再测不可识别选区 + 有效剪贴板 | 选区优先；无效选区允许回退有效剪贴板 | Pending |
| CTX-004 | P1 / H | 新复制后 0 s、14 s、边界 15 s、超时；重复读取 | 按源采集期限判断，不因反复唤醒 / 读取延长有效期 | Pending |
| CTX-005 | P1 / M | App 启动前已有剪贴板；启动后不复制直接唤醒 | 年龄未知不自动填入旧内容 | Pending |
| CTX-006 | P1 / H | 普通无识别文本、无效 URL、不存在路径、标量 JSON，未装匹配插件 | 保持 Current App；不把普通 Text 当作自动跳转依据 | Pending |
| CTX-007 | P1 / H | 选区读取延迟 >500 ms，期间有新剪贴板或用户已输入 / 导航 | 可回退近期剪贴板；用户交互后迟到识别不改页 / 覆盖输入 | Pending |
| CTX-008 | P1 / M | 进入内容页后修改正文，并检查来源标记与动作 | 只处理当前正文和派生 Context；不混入 Finder / 网页环境 | Pending |
| CTX-009 | P1 / H | 输入绝对路径、`~/`、全角 `～/`、带空格 / 波浪号文件名 | 仅规范支持的主目录语义；真实对象类型保留 | Pending |
| CTX-010 | P0 / H | 输入 `~其他用户`、变量、shell 表达式、裸域名、不存在路径 | 不执行 shell / 环境展开，不猜测路径或 URL | Pending |
| CTX-011 | P0 / H | file、directory、workspace、多根 workspace、remote URI 逐类派生动作 | workspace 仍是文件，不任选第一个根；remote 不变成本地路径 | Pending |
| CTX-012 | P1 / M | IDEA 有明确项目路径和 `.idea`；再测标题缺路径 / 窗口切换 / 不可读 | 仅明确且匹配冻结窗口时识别；失败不猜项目目录 | Pending |
| CTX-013 | P1 / H | 分别关闭 Finder / 选区 / 浏览器 / 剪贴板来源后保存再唤醒 | 相应来源停止参与；其他来源继续；旧采集不发布 | Pending |
| CTX-014 | P0 / A | 查看 Settings 快照、Usage、偏好和日志中的合成敏感标记 | 正文不被复制到设置、Usage、快捷键配置或诊断日志 | Pending |

### 6.5 APP：应用索引、启动与图标

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| APP-001 | P1 / M | 按应用显示名、中文名、英文别名、全拼 / 首字母搜索 | 可检索系统 / 第三方应用，界面语言不删除既有别名 | Pending |
| APP-002 | P1 / M | 应用未运行时启动，已运行时再次执行 | 启动或切回正确应用，不启动错误同名目标 | Pending |
| APP-003 | P1 / H | 新应用扫描 / 更新与查询同时完成，旧扫描故意后返回 | 只发布当前目录代次和排除配置对应的快照 | Pending |
| APP-004 | P1 / M | 设置排除应用，保存、搜索、重启，再移除排除 | 即刻 / 重启后的有效目录一致；排除不破坏其他功能 | Pending |
| APP-005 | P0 / H | 显示后删除 / 替换应用；同 Bundle ID 多安装实例 | 执行重新校验；不可用 / 歧义明确失败，不猜另一个应用 | Pending |
| APP-006 | P2 / H | 图标缺失、读取失败、通道关闭；持续结果更新 | 使用占位；30 s 内不无等待循环读取，不覆盖业务错误提示 | Pending |
| APP-007 | P2 / A | 图标旧 ticket 返回、新同 ID 请求在途；超过 128 个 key | 旧回包不结束新请求；LRU / 并发有界；TTL 到期仅后续更新触发重试 | Pending |
| APP-008 | P2 / H | 进入工具页 / 隐藏后再返回 Launcher | 不继续补发后台 Launcher 图标请求；返回后正常补齐 | Pending |

### 6.6 MENU：Current App 菜单与安全降级

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| MENU-001 | P1 / M | Finder、TextEdit、Chrome / Safari、IDEA 中搜菜单名称 | 菜单进入 Current App 和既有搜索；原应用快捷键不误显示为 Anywhere 绑定 | Pending |
| MENU-002 | P1 / M | Browse Menus 按层级用鼠标、Enter、←→、Esc 导航 | 层级与父路径正确；有查询时 ←→ 保留光标编辑 | Pending |
| MENU-003 | P1 / H | Disabled、隐藏、分隔符和不可发现菜单 | 不可用菜单隐藏；不得保留可执行旧行 | Pending |
| MENU-004 | P0 / H | 同标题不同路径、同级重复 identifier、IDEA 共享 `handleAction:` | 仅可唯一定位的标题 + 标识组合可执行；歧义拒绝 | Pending |
| MENU-005 | P0 / A | 快照后菜单改名、标识消失 / 复用、Enabled 改变、兄弟插入 / 重排 | 执行前逐级验证；不按同名旧候选误触 AXPress | Pending |
| MENU-006 | P0 / H | 目标应用退出重启 / PID 复用、焦点窗口改变、菜单注册换 token | 旧候选 / 排队执行失效；刷新后重新选择才能执行 | Pending |
| MENU-007 | P1 / H | AX 超时、不能完成、不完整树、权限中途撤销 | 时间 / 节点有界；不完整树不缓存成成功，不阻塞非菜单动作 | Pending |
| MENU-008 | P1 / H | 菜单执行失效后刷新，再执行新有效候选 | 显示准确失败并可恢复，不永久保留过期列表 | Pending |
| MENU-009 | P0 / A | 多次执行菜单后检查 Usage；尝试把菜单持久绑定 | 不恢复菜单长期 Usage，不允许通过包装动作绕过持久绑定限制 | Pending |
| MENU-010 | P1 / M | 执行测试应用中的无破坏菜单，如 Copy / Select All | 仅目标应用对应命令执行一次，成功反馈与真实副作用一致 | Pending |

### 6.7 ACT：URL、原生对象动作、Usage 与输出

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| ACT-001 | P1 / M | 输入测试HTTP(S) URL，执行Open in Default Browser；用本地测试服务避免外部请求 | 交给默认浏览器打开正确URL一次；识别阶段不联网 | Pending |
| ACT-002 | P1 / M | 从输入、选区、浏览器Context执行Copy URL，包含端口 / query / fragment | 复制当前规范URL，不混入来源说明或另一个窗口地址 | Pending |
| ACT-003 | P0 / H | javascript / data / mailto / file等scheme、裸域名、空authority、反斜线 / 内部空白 | 不成为HTTP(S) URL动作，不以任意scheme启动外部能力 | Pending |
| ACT-004 | P1 / H | 剪贴板等值 / 不等值、读取失败、已知值但年龄未知，分别查复制动作 | 已知等值才隐藏复制；值未知不误隐藏；年龄未知禁止自动填入，不等于已知剪贴板值也未知 | Pending |
| ACT-005 | P1 / M | Finder / 明确路径的directory执行Open Directory、Open Terminal Here、Copy Path | 每个动作使用该目录；不把终端打开当PTY内嵌功能 | Pending |
| ACT-006 | P0 / M | file执行Open File、Reveal、Copy Path；混合相近名称与不同目录 | 类型与目标准确，Reveal选择正确文件，不按同名路径猜测 | Pending |
| ACT-007 | P0 / H | Finder选择多个含中文、引号、空格路径，执行Copy Selected Paths as JSON | 输出合法JSON数组，包含正确冻结选择的全部路径，转义不丢失 | Pending |
| ACT-008 | P1 / H | 同目标来自多个来源与多个不同目标分别展示并执行 | 相同语义目标合并、不同目标保留来源；Scope不改变真实目标 | Pending |
| ACT-009 | P0 / H | 成功 / 失败 / 取消动作，在不同Scope / 来源应用后重启检查Usage | 只记录符合成功契约的稳定业务动作；不记正文 / 目标路径，不把菜单隔离解除 | Pending |
| ACT-010 | P1 / M | Git报告与旧Detail / KeyValue输出，复制长文本、返回父页，再执行失败动作 | 输出完整有界且可读；复制 / 返回作用当前结果，失败不继续显示旧成功反馈 | Pending |

### 6.8 FILE：文件搜索、授权、遍历与对象动作

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| FILE-001 | P1 / M | 搜索 File Search / 文件搜索 / `wjss`，进入并返回 | 普通 Action 可达；普通 Launcher 查询不隐式全盘搜索；父页恢复 | Pending |
| FILE-002 | P1 / H | 搜索关闭 / 无有效授权时查询；授权 F1 后重试 | 停用态明确；仅有效 roots 可搜索，不暗中启用 Home | Pending |
| FILE-003 | P1 / H | 单字符、多词、标点、大小写、NFC / NFD、中文 / 拼音 | 多词均命中 basename，标点按字面，归一规则一致，无漏掉未命中词 | Pending |
| FILE-004 | P1 / M | Tab / Shift+Tab 及 Ctrl 兼容组合切所有类型 | 查询与焦点保留；类型在计数和保存前过滤，IME 不误切换 | Pending |
| FILE-005 | P1 / M | 修改查询 / 类型后直接 Enter，再首次 ↑ / ↓ | 默认无选择；首次方向键选首项，未选中不打开旧对象 | Pending |
| FILE-006 | P1 / H | Spotlight 少量 / 空 / 失败，完整遍历含未索引匹配 | 均继续完整遍历，不以首批数量或 50,000 旧上限冒充完成 | Pending |
| FILE-007 | P1 / H | 已显示前缀后注入晚到高分项并连续加载 | 已显示前缀稳定、去重；新结果只归并未读尾部 | Pending |
| FILE-008 | P1 / M | 滚动末尾追加与键盘末尾追加，已有选中项 / 手动滚动 | 按需追加；鼠标加载不改选择或滚动，键盘选择正确跟随 | Pending |
| FILE-009 | P1 / M | 明确输入目录、现存文件、父目录内未完整文件名、`~/` | 分别列直接子项、定位文件、父目录匹配，仍受 roots 限制 | Pending |
| FILE-010 | P0 / H | `..`、root 外路径、内部 / 越界 / 断链、读取后链接换目标 | 路径与真实目标都校验；越界拒绝；不递归符号链接目录 | Pending |
| FILE-011 | P1 / H | 重叠 roots、重复命中、不同路径硬链接 | 重叠同路径去重；不同路径硬链接分别保留 | Pending |
| FILE-012 | P1 / M | 隐藏开关开 / 关，排除名增删，node_modules / build / target 中匹配 | 隐藏 / 排除按有效设置；不隐含 gitignore 或项目目录黑名单 | Pending |
| FILE-013 | P1 / H | 默认查询遇到系统 tmp、Trash、ShipIt；对这些位置再显式查询 | 默认按规则过滤；明确访问仍受授权 / 用户排除约束，不误排项目同名目录 | Pending |
| FILE-014 | P1 / H | 搜索中 roots / 隐藏 / 类型更改、旧扫描慢返回 | 撤销旧结果与执行资格；旧批次不追加到新列表 | Pending |
| FILE-015 | P1 / H | 不可读子目录、消失子项、根消失、临时磁盘不足 / 写失败 | 区分跳过与 Partial；保留已找到结果，原因有界且不冒充完整 | Pending |
| FILE-016 | P1 / M | 有已有结果时加载失败 → 重试 | 保留列表和当前查询；重试重扫 / 续交正确，无重复或漏项 | Pending |
| FILE-017 | P1 / M | Enter 打开、对象菜单复制 / Reveal / 可用目录动作 | 对正确对象执行；复制反馈与实际剪贴板对应 | Pending |
| FILE-018 | P0 / H | 候选出现后文件删除、改类型、授权撤销再执行 | 执行前拒绝旧目标，不误开替代路径、不记成功 Usage | Pending |
| FILE-019 | P1 / H | 对象 A 请求 Actions 后开 B；A 先发布 / 后回包，两种顺序 | Core 较早请求不得抢占 B；GPUI 旧快照 / 错误不回滚 B 或关闭新面板 | Pending |
| FILE-020 | P1 / H | 同 revision 的 snapshot_seq 乱序，再改 query / revision | 仅单调且满足当前请求的快照发布；重复 cursor 不重复覆盖 | Pending |
| FILE-021 | P2 / H | 深树、超长 / 不能表示的名称、超量 roots / 排除名 | 有界拒绝或正确 Partial，非权限问题不统一误报权限不足 | Pending |
| FILE-022 | P1 / H | 查询取消 / 隐藏 / 超期 / 异常退出后检查临时区，再重启 | 在 worker 实际结束后释放租约；无活租约废目录可回收，不删活查询 | Pending |

### 6.9 PRE：Quick Look 与预览资源

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| PRE-001 | P1 / M | F1 / F2；分别在文件、History、Shelf 选择后点眼睛 / ⌘Y / Space | 都走按需系统预览；无固定预览分栏，未选择不执行 | Pending |
| PRE-002 | P1 / M | 文本、中文文本、图片、PDF、系统支持媒体、多个文件引用 | 系统支持格式可看；不支持格式失败可恢复，不把应用状态卡住 | Pending |
| PRE-003 | P1 / M | Space、Esc、⌘Y、系统关闭按钮关闭；⌘L 回编辑器 | 仅关闭当前预览；查询 / 选择保留，焦点正确；按住不反复切换 | Pending |
| PRE-004 | P1 / H | A 预览在途，改选 B、改查询 / revision、换页面或 Session | 只有当前目标可显示；旧资源释放、旧错误不覆盖提示 | Pending |
| PRE-005 | P1 / H | 原生 show 排队但尚未执行时关闭；系统关闭与新请求交错 | 迟到 show 不重开；旧关闭回调不能关闭新面板 | Pending |
| PRE-006 | P1 / H | A 已显示，B 准备失败 / 慢，期间系统关闭 A | A 监视器仍有效；失败不丢失面板关闭与失活检测 | Pending |
| PRE-007 | P1 / M | 连续选择含一屏外的条目，混合鼠标与首尾快捷键 | 预览跟随最新选择，选中行可见，不误复制 | Pending |
| PRE-008 | P1 / M | 预览中切另一应用、隐藏 / 再唤醒 Launcher | 关闭并释放，不抢回外部焦点；保留期恢复不自动重开 | Pending |
| PRE-009 | P1 / M | 预览中打开 Settings / Web 工具或退出 App | 原面板关闭、资源释放，设置 / 新工具拿到正确键盘 | Pending |
| PRE-010 | P0 / H | 预览前删文件、改链接、撤销授权或删除原剪贴板条目 | Rust 再校验，不能把任意旧路径直接交给 Quick Look | Pending |
| PRE-011 | P1 / H | 临时文本 / 图片预览与多文件预览反复打开 / 关闭 100 次 | 同进程一个所有者；无不断累积的有效租约 / 私有临时文件 | Pending |
| PRE-012 | P2 / M | IME、按钮焦点、双屏、全屏 Space、休眠恢复下重复以上操作 | 无吞键、错误位置、残留面板；物理结果单独记录 | Pending |

### 6.10 CLIP：剪贴板采集、历史与存储策略

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| CLIP-001 | P0 / M | 新 F0；开历史页 / Shelf，不开启录制，外部复制 | 默认不持久采集；打开工具不会自动开启录制 | Pending |
| CLIP-002 | P1 / M | 开启录制前复制 A，开启后复制 B，关闭后复制 C | 只录未来允许的内容；不补录 A / C | Pending |
| CLIP-003 | P1 / M | F2 各格式复制、查询、明确选择、复制回系统 | 支持格式按能力恢复；文件引用不复制原文件，缺失原文件明确失败 | Pending |
| CLIP-004 | P1 / H | 重复相同内容，不同内容同摘要，连续高频复制 | 按真实 payload 去重并保持最新顺序；不合并不同对象 | Pending |
| CLIP-005 | P1 / M | 中文 / emoji / 空格检索，清空，快速改词；结果超过一屏 | 查询 / 选择正确；首次无默认执行选择；空态也能刷新 | Pending |
| CLIP-006 | P1 / H | 首次全量列表慢返回，新查询先完成；复制旧请求晚返回 | 旧列表不回灌；旧反馈不结束新 loading 或污染新页面 | Pending |
| CLIP-007 | P1 / M | 单击、双击、Enter、行内按钮、右键 / ⌘K 操作 | 单击仅选中；复制仅对应明确对象，成功后才显示反馈 | Pending |
| CLIP-008 | P0 / H | 打开 A 的删除确认后刷新 / 改选 B，再确认 | 确认仍绑定 A；取消不删除，不因下标变动删除 B | Pending |
| CLIP-009 | P1 / M | 编辑查询时裸 Backspace / Delete，再用菜单 / 按钮删除测试条目 | 文本正常编辑；列表不会因裸删除键误删记录 | Pending |
| CLIP-010 | P1 / M | 暂停 5 / 15 / 60 分钟、提前恢复；暂停中关闭录制 | 暂停不补录；关闭取消自动恢复；已有历史仍可查询 / 复制 | Pending |
| CLIP-011 | P0 / H | 保留期设为 1 / 7 / 30 / 90 天与容量限制，取消 / 确认缩短 | 缩短需确认，只删过期记录；延长不恢复已删，15 s 预填寿命不改变 | Pending |
| CLIP-012 | P0 / M | 清空历史先取消，再确认；检查系统剪贴板及原文件 | 只永久清理测试历史；不改系统剪贴板或原文件 | Pending |
| CLIP-013 | P1 / A | 1,001 条、128 MiB 总量、16 MiB 单条、64 格式上下边界 | 有界保存 / 淘汰，超限不产生半条记录；精确核对用量 | Pending |
| CLIP-014 | P1 / M | 允许 / 拒绝系统剪贴板读取、重复唤醒和后台运行 | 失败可见且不循环弹权限；不把拒绝记成成功采集 | Pending |
| CLIP-015 | P1 / H | 历史库关闭、写失败 / 锁定、复制失败后恢复再刷新 | loading 收尾，数据不被清空冒充空结果；恢复后可继续 | Pending |
| CLIP-016 | P0 / H | 退出重启、查本地 DB / 日志 / Usage 中合成标记 | 历史按配置本机持久化；明文边界明确；正文不外溢到 Usage / 日志 | Pending |

### 6.11 SHELF：独立悬浮窗

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| SHELF-001 | P1 / M | 从 Action / 快捷键开 Shelf，重复触发、最小化后再触发 | 独立置顶窗口复用；最小化先恢复，不生成多个实例 | Pending |
| SHELF-002 | P1 / M | 在各屏幕移动鼠标后打开，缩至 360×360 / 默认 480×520 | 定位当前鼠标屏幕右下角且在可用区域内，控件可达 | Pending |
| SHELF-003 | P1 / M | 外部应用点击、Launcher 唤醒、交通灯关闭、Esc | 普通失焦不隐藏；Esc 按局部层 / 查询 / 隐藏规则；关闭后可重开 | Pending |
| SHELF-004 | P1 / M | ↑↓、⌘↑↓、PageUp / PageDown 移动跨屏列表选择 | 首尾 / 分页正确并滚入视口，无越界；预览和复制反馈同步 | Pending |
| SHELF-005 | P1 / M | ⌘1–9 在 0 / 少于9 / 多于9 条时复制 | 仅对应存在的结果复制一次；无效数字不复制旧选择 | Pending |
| SHELF-006 | P1 / M | Enter / 复制按钮后继续输入和选择，再 ⌘Enter | 普通复制保留 Shelf 焦点；复制并返回在成功后才回原应用 | Pending |
| SHELF-007 | P0 / H | 复制并返回在途，切其他窗口 / 关 Shelf / 新操作后旧结果返回 | 旧复制可按已提交契约完成，但不能抢走当前应用焦点 | Pending |
| SHELF-008 | P1 / M | ⌘F 聚焦搜索、⌘R 刷新、⌘⇧Backspace 删除并取消确认 | 各快捷键只作用于当前窗口和当前条目；取消不删除 | Pending |
| SHELF-009 | P1 / M | 右键 / ⌘K / 选中行预览与更多操作菜单（复制、复制并返回、删除、刷新）及空列表菜单 | 控件和键盘入口一致；空列表可刷新，其他动作不执行旧对象 | Pending |
| SHELF-010 | P1 / H | 同时打开 History、Shelf、Tasks；搜索刷新、预览与关闭交错 | 查询与反馈独立；Quick Look按单一当前所有者交接，旧窗口回包不干扰新所有者 | Pending |

### 6.12 SET：设置、草稿、保存与登录项

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| SET-001 | P1 / M | General、Shortcuts、Context、Search、Clipboard、Application History、Features 逐类进入 | 七类可达、无丢失控件；当前值与有效配置对应 | Pending |
| SET-002 | P1 / H | 新目录读取默认值，保存重启后逐项对照 HostPreferences | 来源、语言、热键、搜索、隐藏及历史录制默认不被旧包配置冒充 | Pending |
| SET-003 | P1 / M | 修改各类偏好，切分类、筛选、再返回 | 草稿仍在；尚未保存的偏好不被普通回读覆盖 | Pending |
| SET-004 | P1 / M | 无草稿 Esc、Done 或原生关闭按钮；有草稿逐个入口再测 | 干净时按规则退出；脏草稿进入保存 / 放弃 / 继续编辑决定 | Pending |
| SET-005 | P0 / H | 脏草稿选择继续编辑 / 放弃 / 保存，分别触发关闭 / 唤醒 / ⌘Q | 意图正确；继续编辑保留、放弃恢复、保存成功后才按意图退出 | Pending |
| SET-006 | P1 / H | 保存中产生无关 Launcher / follow 错误，再尝试重复保存 | 不清掉设置 saving，不开始第二次设置操作 | Pending |
| SET-007 | P1 / H | 保存偏好期间点击录制 / 功能开关；先开关再立即关闭设置 | 即时开关禁用且回调拒绝重入；成功后关闭流程正确收尾 | Pending |
| SET-008 | P1 / H | 设置回包前 Launcher generation 变化或通道关闭 | 设置按独立操作代次收尾；断开显示错误，留在设置可重试 | Pending |
| SET-009 | P0 / H | 模拟写入失败、目录授权失败、系统效应失败后恢复重试 | 不假报保存成功；保留草稿和明确错误，实际开关快照回滚 | Pending |
| SET-010 | P0 / H | 保存期间再编辑；保存成功排队退出前继续编辑 / 新增草稿 | 新编辑不被旧保存覆盖，取消和新草稿阻止迟到退出 | Pending |
| SET-011 | P1 / M | 从预览 / Web 工具、隐藏状态栏分别打开 Settings，再关闭 | 原预览 / Web 清理，返回 Launcher 查询通道可用，15 s 内恢复 / 超时重启正确 | Pending |
| SET-012 | P1 / M | 语言 System / 中文 / English 轮换、保存 / 放弃 / 重启 | 预览、持久值、状态菜单及 Web 文案一致；query / 正文 / Action ID 不被翻译或清空 | Pending |
| SET-013 | P1 / M | 登录项开启 → 系统批准 / 拒绝 → 关闭 → 重启 | 仅系统确认 Enabled 才报告启用；RequiresApproval 有提示；关闭取消待批准注册 | Pending |
| SET-014 | P1 / M | App 移位 / 服务不可用 / 注册失败时保存开机启动 | 明确错误及有效状态；不能把持久布尔值当 OS 已注册证据 | Pending |
| SET-015 | P0 / H | 测试用户中记录系统登录项；设置 `ANYWHERE_DATA_DIR` 后切换开机启动并保存，再对照系统登录项 | 显式数据目录隔离生效，不调用 `SMAppService` 修改真实系统登录项 | Pending |
| SET-016 | P1 / M | 关闭并重开设置，检查错误提示、确认框和原输入焦点 | 不带入过期模态 / 录制态；错误只按真实成功清理，焦点回到可见控件 | Pending |

### 6.13 HOT：快捷键、Context 分派与配置兼容

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| HOT-001 | P1 / M | 录制 Current / Global 新键，保存再试旧键 / 新键 | 新键有效、旧键注销；状态菜单提示同步有效注册值 | Pending |
| HOT-002 | P1 / H | 无修饰符、非法键、两个入口相同、与 Action 绑定冲突 | 清晰验证错误，旧有效键可继续使用 | Pending |
| HOT-003 | P1 / M | 录制中 Esc、Tab、普通输入和中文 IME，取消后再录制 | 不误触全局动作；取消 / 焦点恢复正确，不残留录制状态 | Pending |
| HOT-004 | P1 / M | 普通 Action 配本地键 / Global 开关，分别在 Launcher 与外部触发 | 触发范围正确；非全局不会无窗口后台执行 | Pending |
| HOT-005 | P0 / H | 同一动作只有一个、多个、没有适用 Context 时触发 | 唯一目标执行；多个供选择；缺对象不执行旧目标 | Pending |
| HOT-006 | P0 / H | Context 快捷键按应用 / 类型定义多 route，更换来源应用 | 依据被冻结来源和真实 Context 分派，不根据当前 Anywhere 前台身份误派 | Pending |
| HOT-007 | P1 / M | Context 绑定名称、分派路由草稿编辑、重复 / 无效规则 | 保存校验准确，失败保留草稿，编辑不会默默丢失 | Pending |
| HOT-008 | P1 / H | 达到 128 绑定、16 routes 的上下边界 | 上限内持久化；超限拒绝，不部分写入 | Pending |
| HOT-009 | P0 / H | 动作 / 模块 / 历史来源 / 插件关闭或权限撤销后触发旧绑定 | 配置可保留，执行资格立即失效；重启不绕过撤销 | Pending |
| HOT-010 | P1 / H | F0中备份后放入旧版、部分无效绑定及整体损坏偏好；重启加载 / 保存 | 依当前兼容规则保留合法旧绑定、过滤无效项；整体损坏明确报错，不默默覆盖原文件 | Pending |
| HOT-011 | P1 / H | 动作目录加载失败、迟到加载、目录中绑定动作暂时缺失 | 不永久卡 loading；不可用规则可见，不误替换为同名动作 | Pending |
| HOT-012 | P0 / A | 菜单动作及其包装动作尝试绑定，目录 / Catalog 变化后执行排队请求 | Menu BindingPolicy 不可绕过；发布 / 执行前复验版本与 token | Pending |

### 6.14 HIST：应用历史、来源设置与目标权威

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| HIST-001 | P1 / H | F5；逐个 SRC 来源检查已安装 / 未安装默认状态、手动开关、重启 | 仅安装的默认开启；显式开关优先且不因刷新被改写 | Pending |
| HIST-002 | P1 / M | 对应 Current App、全局“应用名+最近项目”、独立 Action 绑定进入 | 固定同一来源，入口均可达，无新增独立 Scope | Pending |
| HIST-003 | P1 / H | 标准自动路径多个版本 / mtime 并列，VS Code shared 与旧库并存 | 按当前规则选一个；home_paths 优先；不合并多安装 / Profile 猜结果 | Pending |
| HIST-004 | P1 / M | 自定义路径、`~/`、无效路径；手动关闭后再保存路径 | 自定义优先且规范化；失效不静默改别处；保存路径不重新开启 Action | Pending |
| HIST-005 | P1 / M | 展开来源路径编辑器、筛选来源 / 仅已启用，保存自定义路径或切回自动检测 | 仅当前编辑卡片受影响，保存后状态立即更新，失败保留错误提示 | Pending |
| HIST-006 | P1 / H | 自定义路径保存失败再普通刷新，然后重试；并发实例保存不同字段 | 失败不被回读抹掉；基于最新配置合并，不覆盖另一个实例的字段 | Pending |
| HIST-007 | P1 / H | 无来源、已关闭、未配置、真正空历史、读取失败分别进入 | 分别显示准确状态并结束 loading，不把失败当真正空结果 | Pending |
| HIST-008 | P1 / H | 逐 SRC 的 XML / SQLite / JSON 合成数据读取、去重、时间 / 无时间排序 | 只展示来源记录；无时间保留原顺序，不借文件 atime 伪造历史 | Pending |
| HIST-009 | P1 / M | 标题、路径、中文 / 拼音检索，快速改词、IME 组合 | 按来源结果查询；组合不触发搜索，旧结果不回灌 | Pending |
| HIST-010 | P1 / M | ↑↓、PageUp / PageDown、Enter / 双击、Esc 清查询再返回、页头直接返回 | 分页按可见高度；按键不越界，查询和父页返回链准确 | Pending |
| HIST-011 | P0 / M | directory、file、workspace、Rider solution 在来源应用打开 | 固定目标应用；workspace / solution 保持文件语义，不猜首个目录 | Pending |
| HIST-012 | P0 / H | 本地目标复制 / 打开前删除、改类型、换 symlink；应用缺失 / 多安装 | 重新读取 key 和目标，拒绝变更 / 歧义；不误开另一对象 | Pending |
| HIST-013 | P0 / M | remote 条目执行 / 复制；Obsidian 同名 vault 的不同 ID | remote 只复制原 URI；vault 按校验 ID / 路径交给 Obsidian，不扫笔记树 | Pending |
| HIST-014 | P1 / H | 右键 / 更多操作派生 File / Directory、Git、Developer；执行后返回 | 只对明确对象按需派生；保留历史查询 / 选择，重新读有效来源 | Pending |
| HIST-015 | P0 / H | 来源关闭、刷新、改路径 / key / 目标、开另一对象菜单后执行旧动作 | 旧权威立即撤销；实际进程启动前也复验，不靠稳定 ID 放行 | Pending |
| HIST-016 | P1 / H | 查询中隐藏 / 换页 / 通道关闭，15 s 内恢复 / 超时 | 取消读取、loading 收尾；恢复读当前配置，不复活旧 Session | Pending |
| HIST-017 | P0 / A | 未知 schema、重复 key、相对路径、非法时间 / target、恶意 XML / SQL | 来源失败并保留其他来源；只读查询，不写源文件 / 扩展权限 | Pending |
| HIST-018 | P1 / A | 8 MiB / 2 MiB / 4,096 条与 XML / JS / 截止各边界；阻塞读未返回 | 超限 / 超时准确；旧等待者可取消；物理读槽至实际返回才释放 | Pending |
| HIST-019 | P1 / H | 新增用户 source / extends，同 ID 覆盖、移除 / 恢复，重启 | 可信用户适配按契约生效；设置稳定 ID 保留；不获得 Web 或原生文件权限 | Pending |
| HIST-020 | P0 / A | schema1→2 配置迁移、损坏 / 未来 schema；检查业务持久化 | 保留旧关闭 / 自定义路径，不覆盖未知数据；不把历史正文镜像进模块存储 | Pending |

### 6.15 GIT：仓库识别与只读 Git 动作

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| GIT-001 | P1 / H | F3 中根目录、子目录、普通非仓库、`.git` 文件和 worktree | 正确识别仓库根；识别不运行 Git，不把无效标记当仓库 | Pending |
| GIT-002 | P1 / M | Git Status、Log、Diff、Staged Diff、Branches 全部执行 | 显示对应只读报告；暂存差异为空有明确状态 | Pending |
| GIT-003 | P1 / M | 复制分支名、完整 / 短 hash、仓库根路径、remote URL | 剪贴板等于当前真实值；成功关闭 / Usage 正确 | Pending |
| GIT-004 | P1 / H | 无提交仓库、detached HEAD、无 remote 分别执行各复制 | 命名分支 / hash 能力按真实状态区分；失败不改剪贴板或记成功 | Pending |
| GIT-005 | P0 / H | 无 origin 的多个 remote、唯一 remote、origin 多 URL 和 insteadOf | 不猜歧义地址；按 Git fetch 规则选值，不发网络请求 | Pending |
| GIT-006 | P1 / H | Git 工具缺失、运行错误、超时 / 过量输出 | 命令正确禁用 / 失败且输出有界；复制仓库路径仍可用 | Pending |
| GIT-007 | P0 / A | 恶意路径 / Git 环境变量 / pager / diff / textconv / fsmonitor 配置 | 固定 argv、清理定位环境、不走 shell / 外部扩展，不执行注入 | Pending |
| GIT-008 | P1 / H | 查询后切换分支 / HEAD / remote，再执行复制 | 执行时重新读取；不把展示期缓存当当前 Git 值 | Pending |
| GIT-009 | P1 / M | 剪贴板已等于仓库根；同 Context 从不同来源到达 | Copy Repository Path 隐藏且目标合并；不同目标保留来源区分 | Pending |
| GIT-010 | P0 / H | 禁用 Git 模块或目录消失后触发旧候选 / 快捷键 | 旧请求失效，无写仓库动作，无错误成功 Usage | Pending |

### 6.16 DEV：项目识别、构建 / 运行与收藏目录

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| DEV-001 | P1 / M | F4 Maven 项目 Test / Package，存在 / 不存在 mvnw 两组 | 使用明确项目目录；优先 wrapper，工具和参数可在任务详情核对 | Pending |
| DEV-002 | P1 / M | Gradle Build / Test，存在 / 不存在 gradlew 两组 | 同上，不把项目脚本文本拼成任意 shell | Pending |
| DEV-003 | P1 / H | npm / pnpm / yarn 的 packageManager / 锁文件矩阵及冲突 / 未知值 | 明确选择支持的管理器；不猜未知工具，不执行清单注入 | Pending |
| DEV-004 | P1 / M | Node 多 scripts、带空格脚本名、删除 / 改名后执行旧候选 | 只生成有效声明脚本；执行前重读，过期脚本拒绝 | Pending |
| DEV-005 | P1 / M | Cargo Check / Test；单 bin、多 bin、workspace 多 package | 动作目标明确，不从多目标任选一个 Run | Pending |
| DEV-006 | P1 / H | Cargo bin 带 required-features，展示后改 feature / 清单 | 执行时重读并启用声明 feature，保留默认 feature；过期目标不误运行 | Pending |
| DEV-007 | P1 / H | Spring Boot 明确 Maven / Gradle 插件；普通项目无插件 | 只给确定有能力的项目 Run，不靠目录名猜 Boot | Pending |
| DEV-008 | P1 / H | 损坏 / 过大 / 非支持的项目元数据、无标记目录、工具不可执行 | 识别和元数据读取有界；降级准确，不阻塞普通文件动作 | Pending |
| DEV-009 | P0 / H | 项目目录 / 标记 / Catalog 改变后排队启动，或模块被关闭 | 真正启动前拒绝旧授权；不因持久 Action ID 继续执行 | Pending |
| DEV-010 | P1 / M | 成功启动短 / 长任务后立刻切页、隐藏、打开 Tasks | 返回 execution_session_id，对应任务可查看；不因 Launcher 生命周期停止 | Pending |
| DEV-011 | P1 / M | 目录执行 Save Directory；全局按名称 / saved / bookmark 查询再打开 | 收藏可持久检索，默认操作是打开目录，不变成历史活动聚合 | Pending |
| DEV-012 | P0 / H | 收藏更多操作 Forget、取消 / 执行；100 / 101 条、重复目标、目录删除 | Forget 只移收藏不删磁盘；容量 / 合并正确，失效打开失败 | Pending |

### 6.17 TASK：后台进程与 Running Tasks

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| TASK-001 | P1 / M | 启动任务、状态栏打开 Tasks、多次重复 / 最小化后再开 | 新建或复用独立 Tasks 窗口并选中新任务，窗口正确成为 key | Pending |
| TASK-002 | P1 / M | 零任务、全完成、Running / Stopping / Succeeded / Failed / Stopped | 空态、统计、退出码与状态对应真实进程 | Pending |
| TASK-003 | P1 / M | 持续 stdout / stderr，切换输出 / Diagnostics、复制输出和命令 | 两路不混淆；复制只含当前显示内容 / 准确命令，Unicode 与空白保留 | Pending |
| TASK-004 | P1 / H | 每路 128 KiB 上下、海量无换行 / 非完整 UTF-8 边界输出 | 有界尾部保留和截断提示，不死锁或无限增长 | Pending |
| TASK-005 | P0 / M | 停止按钮 / ⌘.，重复停止，停止派生子进程的长任务 | 仅停止选中有效任务及其受管进程组；不停止其他任务或外部进程 | Pending |
| TASK-006 | P0 / H | 筛选零命中时执行 ⌘. / 删除结果 / 接受旧详情 | 没有可执行选择；不能操作隐藏的首项 | Pending |
| TASK-007 | P1 / M | ⌘F 搜索标题 / 工作目录、多词、改筛选、Esc 清空再关闭 | 匹配与选择一致；清空不丢任务，筛选框 Enter 行为正确 | Pending |
| TASK-008 | P1 / M | ↑↓ 与 ⌘⌥↑↓、⌘R、⌘L，焦点在筛选 / 详情分别测 | 支持的导航在 Tasks 内生效；返回 Launcher 不触发隐藏页面快捷键 | Pending |
| TASK-009 | P1 / H | 新任务尚未进首份快照，后续出现；期间手动选另一任务 / 改筛选 | 等待目标出现才消费 pending；手动意图撤销旧自动选择 | Pending |
| TASK-010 | P2 / M | 手动滚动侧栏和输出查看旧段，期间持续新输出 | 侧栏不因相同选择更新回跳；只有 Follow 模式或明确跳末尾才跟随输出 | Pending |
| TASK-011 | P1 / M | 菜单复制命令 / 移除完成结果；对运行任务查看更多操作 | 运行中不可当作完成结果移除；移除不删除项目文件、不终止其他任务 | Pending |
| TASK-012 | P1 / H | 第 4 / 第 5 个并发任务，超过 32 个保留会话 | 并发和保留数受限，拒绝 / 淘汰规则明确，不遗留孤儿进程 | Pending |
| TASK-013 | P1 / M | 关闭 / 最小化 Tasks、隐藏 Launcher，再打开；最后退出 App | 隐藏和关窗口不停止任务；退出终止受管任务，无 PTY / 重启恢复承诺 | Pending |
| TASK-014 | P1 / H | 任务启动失败、详情通道错误、快速切任务后旧详情到达 | 错误可恢复；旧详情不覆盖当前选择，按钮不操作旧任务 | Pending |

### 6.18 PKG：Web 包安装、批准、更新和撤销

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| PKG-001 | P1 / M | `python3 scripts/build-web-examples.py` 生成当前 F6，导入合法包 | 审查显示名称、版本、摘要、权限；批准前不可执行 | Pending |
| PKG-002 | P0 / H | Inspect 后取消、重复取消 / 批准、旧 token 在新审批后返回 | 只能批准精确审查快照；旧 token 不影响新批准 | Pending |
| PKG-003 | P1 / M | 批准 → 搜索 / 执行 → 禁用 → 启用 → 重启 | Web 批准即生效；开关 / 持久状态正确，不要求走 TOML 下次启动流程 | Pending |
| PKG-004 | P0 / H | 同 ID 更新资源 / 权限，旧页面和后台 Action 在途 | 新摘要重新审查；批准立即撤销旧执行，不把旧实例升级为新权限 | Pending |
| PKG-005 | P1 / M | 更新失败 / 持久化失败后列表刷新，恢复上个批准版本 | 列表来自实际安装状态；失败不伪造成功、不丢原批准版本 | Pending |
| PKG-006 | P0 / M | 撤销 / 卸载，旧绑定再触发；卸载后重新装同 ID | 旧 grant / 文档失效；卸载清理该安装私有 KV，不删除他包数据 | Pending |
| PKG-007 | P0 / A | 损坏 ZIP、路径穿越、绝对 / 逃逸路径、重复资源、无入口、错误 schema及各包字节 / 数量 / 压缩比边界 | 有界拒绝，不把资源写出私有安装区 | Pending |
| PKG-008 | P0 / A | 审查后修改内容、篡改已装资源、摘要不符，资源晚返回 | 执行 / 资源发布前重新核验，不提供篡改后内容 | Pending |
| PKG-009 | P0 / A | Manifest 未知权限、非法类型命名空间、非法 runtime / resolver 声明 | 验证失败，不获得未声明能力或伪造第一方 Context | Pending |
| PKG-010 | P1 / M | 空列表、损坏包列表、开关失败、导入选择取消 / 连点 | 可恢复操作；取消不清空原列表，选择器不重入 | Pending |
| PKG-011 | P1 / H | 从 Settings 管理区和 Web Packages 页分别操作启用 / 列表选择 | 当前安装状态一致，旧 review / 错误不污染新操作 | Pending |
| PKG-012 | P0 / H | 无网络环境重复导入 / 使用本地包；同 ID 不同本地包 | 功能不依赖市场；明确本地批准不等于验证发布者身份 / OS 进程沙箱 | Pending |

### 6.19 WEB：WebView、SDK、权限与生命周期

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| WEB-001 | P1 / M | 名称搜索 Text Pad / JSON Workspace，打开后观察首帧、输入焦点和错误 | 真实 WebView 可用；搜索名称不误填为工具正文，空白入口可手工输入 | Pending |
| WEB-002 | P1 / H | 启动 Context / input 后同 revision 的 snapshot_seq 更新，再真正编辑新 revision | 初始、重复快照、新输入区分正确；旧快照不覆盖插件新草稿 | Pending |
| WEB-003 | P0 / H | context.read 允许 / 不允许、Input 读写权限组合、文件型 Context | 仅获准单对象 / Input；缺授权为空或拒绝；不暴露完整图、路径或正文文件权限 | Pending |
| WEB-004 | P0 / H | Source app / frontmost 权限组合；唤醒后切前台 | 冻结来源与此刻前台不混用；缺权限不推送秘密元数据 | Pending |
| WEB-005 | P0 / M | 批准剪贴板权限后显式读 / 写，再用缺权限包重复 | 获准调用不逐次弹框，成功才反馈；拒绝不写部分数据、不自动重试写入 | Pending |
| WEB-006 | P1 / H | storage get / set / remove、重开 / 更新 / 卸载，与另一个包比较 | 私有存储隔离；更新按命名空间契约保留，卸载仅清本包 | Pending |
| WEB-007 | P0 / H | fixture 尝试外部网络、iframe / popup / 下载 / 跳转、file URL | 平台和文档策略阻断；用可观察 fixture 验证，不能把网络本身不可达当策略通过 | Pending |
| WEB-008 | P0 / A | 非 main frame / 错 origin / 旧 handle / 错 epoch、非法 frame、重复 request ID | Broker 拒绝；旧文档不能操作当前 surface 或越权调用 | Pending |
| WEB-009 | P1 / H | 中文 / emoji 的 1 MiB 上下正文，begin/chunk/commit/cancel、超 frame | UTF-8 和事务预算正确；失败不部分提交，取消后后续写入仍可用 | Pending |
| WEB-010 | P1 / A | 并发 pending / running、request ID 配额、5 s 期限及 BUSY | 额度有界；只读同步可在预算内重试，写入不被偷偷重复执行 | Pending |
| WEB-011 | P1 / H | 大状态版本通知、在途 getter、input.set 回应早于 / 晚于推送 | SDK 合并同步追到最新；已确认写入不因读回失败改报写失败 | Pending |
| WEB-012 | P1 / M | 页头 Actions Reload / Back，键盘导航菜单，重载期间连点 | Reload 先绑定替换再清旧文档；不删 invocation 导致重载失败，不重入 | Pending |
| WEB-013 | P0 / H | 打开 / Reload / resume 后立即返回 / Settings / 替换页面，旧回复晚到 | 只释放旧 handle；旧资源、错误、close / ack 不影响新页面 | Pending |
| WEB-014 | P1 / H | 打开 / 恢复通道关闭、加载失败、再重试 | opening / busy 结束，有明确错误，能重新使用当前工具 | Pending |
| WEB-015 | P0 / H | 初次打开 / Reload / resume 期间隐藏，15 s 内再唤醒，重复隐藏 | 隐藏立即撤销执行；仅留元数据，恢复有效 epoch 后再挂载 / visible IPC | Pending |
| WEB-016 | P1 / M | Web 中文 IME、Esc 返回、失焦隐藏、超 15 s、退出、100 次循环 | 状态保留和销毁准确；不吞键、回滚正文或无限积累页面 / helper | Pending |
| WEB-017 | P0 / A | 私有KV的key / value / key数 / 总字节上下边界，超限后读取旧值并重试小写入 | 配额和序列化字节正确，失败原子且不改旧值；Web预算不扩张原生模块预算 | Pending |

### 6.20 JSON：JSON Workspace 与三个后台复制命令

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| JSON-001 | P1 / H | 无内容名称搜索；合法对象 / 数组 / 转义输入；标量与非法输入 | 名称只给空白 Workspace；匹配内容才有对应复制 Action；不提供独立 Unescape | Pending |
| JSON-002 | P1 / M | 初始空白打开与带内容打开；编辑 / 粘贴后等待 300 ms | 分别聚焦 JSON / 公式；自动格式化合法内容，组词不被处理 | Pending |
| JSON-003 | P1 / H | 普通 / 多层转义 / 局部粘贴，最多 4 层与 16 KiB 自动识别边界 | 仅完整支持的对象 / 数组解码；局部粘贴和字符串内合法转义不被乱替换 | Pending |
| JSON-004 | P1 / M | 切 JavaScript / JSONPath、Ctrl+Tab，留空 / 粘完整公式 / 只写后缀 | 前缀不重复，两个公式各自保留，空公式展示整个文档 | Pending |
| JSON-005 | P1 / H | 例文 users；查询首项、数组、过滤、missing、匹配到空数组 | 值 / matches 数量和类型正确；0 matches 与一个空数组不同 | Pending |
| JSON-006 | P1 / M | 公式 Enter / ⌘Enter、停顿自动更新；修改输入后立刻复制 | 运行当前版本；旧结果立即失去显示 / 复制资格，不复制上次结果 | Pending |
| JSON-007 | P0 / H | 大整数、指数、原始转义、重复键、键序、数值过滤精度边界 | 格式化 / 提取保留原始数字语义；不能精确往返时拒绝数值比较，不静默改值 | Pending |
| JSON-008 | P0 / H | Copy Formatted / Minified / Escaped 与右侧 Copy result 比较 | 前三者复制整个当前文档变体，后者复制查询结果；后台 Action 不创建 WebView | Pending |
| JSON-009 | P1 / M | JSON 操作菜单缩进、清空、复制变体；Enter / Space / ↑↓ / Tab / Esc | 控件键盘可达，关闭恢复焦点；缩进持久化，正文不持久化 | Pending |
| JSON-010 | P1 / H | 非法 JSON / JSONPath / JS、死循环、原型 / 全局 / 网络访问尝试 | 错误可定位、预算有界，旧结果不被误当成功，不获得 Host 任意能力 | Pending |
| JSON-011 | P1 / A | 各 1 MiB、深度128、50k值、10k匹配、路径1024字符等边界 | 超限明确报错而非截断；失败后小输入和后续复制仍可用 | Pending |
| JSON-012 | P1 / M | 主题 / 语言切换、隐藏恢复、Reload、重开与卸载再安装 | 文案跟随 Host；新草稿不被旧事件覆盖；只有缩进偏好按安装生命周期保留 | Pending |

### 6.21 BG：第三方 Context 与有限后台执行

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| BG-001 | P1 / H | 安装 JSON Summary，文本进入解析器并执行 Copy JSON Summary | 派生声明的扩展 Context；显式命令成功复制并关闭 Launcher，无工具页 | Pending |
| BG-002 | P0 / A | resolver 返回 core.file / 他包命名空间 / 未声明类型或任意文件描述 | 拒绝注册 / 输出，不产生原生文件授权 | Pending |
| BG-003 | P0 / A | 自动 resolver 调 input.set、剪贴板、确认、文件 / 网络 / 进程 / Node API | 自动解析只读且无副作用；被禁能力均拒绝 | Pending |
| BG-004 | P1 / H | 读获准 Context / Input / app；显式后台写剪贴板，与缺权限包比较 | 与 Web 共用 grant / Broker；缺权限拒绝，写成功才反馈 | Pending |
| BG-005 | P1 / A | requireContext、无 accepts 全局命令、仅后台包无 HTML、错误 entrypoint | 可用性符合声明；错误受控，不启动无主页面 | Pending |
| BG-006 | P1 / A | 不匹配 / 多解析器 / 超时，预填75 ms及150 ms整轮边界 | 新任务不无限启动；默认唤醒不被大量解析器拖住，失败可回退 | Pending |
| BG-007 | P0 / A | 死循环、超堆 / 栈 / 输出 / Host calls / business calls | 预算生效，任务结束释放 VM / 通知，不作为常驻后台监控 | Pending |
| BG-008 | P0 / H | 运行中隐藏、禁用、卸载、Catalog 变化，再让旧任务完成 | 立即取消旧资格；尚未获准的副作用不能执行，迟到结果不影响新UI；已提交OS调用按提交边界单独记录 | Pending |
| BG-009 | P1 / A | 分块 Unicode 写入、自己修改 Input 后订阅 / getter、读回失败 | 与 Web 相同确认语义；已成功写入不因同步失败改报失败，不缓存失败 Promise | Pending |
| BG-010 | P2 / H | 反复启动短任务、失败、重试，观察 VM / worker / 缓存 | 按需生命周期有界；明确当前为进程内 QuickJS，不声称 OS 进程隔离 | Pending |

### 6.22 DSL：内置模块开关与未知目标隔离

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| DSL-004 | P0 / H | 损坏 `features.json`、未知 NativeTool / 结果类型 | 进入只读恢复或显示明确不支持页面；不覆盖损坏文件、不打开任意原生工具 | Pending |
| DSL-005 | P0 / H | 关闭 URL / Git / 各项目模块 / 收藏 / 剪贴板 / 历史贡献并执行旧候选 | 注册、识别、动作和快捷键资格一起撤销，其他模块继续 | Pending |
| DSL-006 | P1 / H | 模块重新启用、重启、从旧 `extensions.json` 首次单向迁移内置禁用键 | 按持久配置恢复；`features.json` 生成后不再重复导入旧文件 | Pending |

### 6.23 UI：布局、材质、本地化与可访问性

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| UI-001 | P1 / M | 主页空态、3 / 5 / 7 行、长列表 / 长查询、Scope 溢出 | 无通用重复页脚 / 截断控件；行高和首屏数量符合当前布局，Scope 可达 | Pending |
| UI-002 | P1 / M | 短 / 长查询快速切换，录制窗口顶部边缘与列表首帧 | 顶部锚点不漂移；resize 无明显撕裂 / 错位；几何测试不替代录像 | Pending |
| UI-003 | P1 / M | 文件 / Settings / Tasks / Shelf 各尺寸，小屏、缩放和鼠标换屏 | 按可用屏幕约束；关键按钮 / 对话框可达，不裁掉内容 | Pending |
| UI-004 | P1 / M | Launcher 浮动、Settings / Tasks 普通窗口、Shelf 置顶，和其他 App 交替 | 窗口层级符合职责；Settings / Tasks 有关闭 / 最小化等正常窗口行为 | Pending |
| UI-005 | P1 / M | 深浅主题、壁纸切换、Reduce Transparency / Increase Contrast；macOS26+对照同背景的系统通知 | 26+所有窗口为无tint的Regular（旧系统HUD）；页面 / 搜索头 / 普通结果行不叠补色蒙版；Web无白底；减少透明度回退和局部控件可读 | Pending |
| UI-006 | P2 / M | Reduce Motion，快速唤醒 / 收起 / 内容变化 | 尊重系统效果；无残留动画、闪白或错误点击区域 | Pending |
| UI-007 | P1 / M | 长标题 / 路径 / 中英混排 / emoji / 底层长错误 / 多行提示 | 省略与滚动可理解；Notice 有界，失败原因可访问，不挤掉主内容 | Pending |
| UI-008 | P1 / M | 普通、hover、selected、focus、disabled、busy、success、error 所有状态 | 状态可区分；disabled 不可触发，复制等成功样式不早于实际成功 | Pending |
| UI-009 | P1 / M | 设置 / 删除 / 清空确认、菜单、tooltip 同时覆盖背景控件 | 无点击穿透；浮层不改变编辑区尺寸，Esc 只关最上层并恢复焦点 | Pending |
| UI-010 | P1 / M | 中英切换逐页、状态菜单、原生对话框与两个 Web 示例 | 文案覆盖且无明显溢出；第三方标题 / 用户正文 / 底层诊断可保持原文 | Pending |
| UI-011 | P1 / M | 开 VoiceOver，用键盘遍历主页、设置、Tasks、Shelf、确认和 Web | 名称 / role / 选中 / disabled / 展开状态可理解，主流程无需鼠标可完成 | Pending |
| UI-012 | P2 / H | 核对 shared tokens / l10n 生成物，主题变更后完整遍历控件 | 源配置与生成物同步；原生与 Web 控件共享规范，不出现第二套壳 | Pending |

### 6.24 PERF：性能、内存与资源趋势

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| PERF-001 | P2 / H | 同硬件、Release、语言 / 缩放 / 数据固定，冷 / 暖唤醒各采样≥30次 | 保存 p50 / p95 / max / 失败数；区分 hotkey_to_visible、可交互快照和真实首帧 | Pending |
| PERF-002 | P1 / H | 连续改词、切Scope / 类型、慢源并存、取消100轮 | UI 不被同步阻塞；旧工作不无限积压，记录 worker / queue 与取消完成 | Pending |
| PERF-003 | P2 / H | F1 从少量到10k / 50,001命中，记录扫描 / 追加 / 排序和临时区 | 完整性先成立；内存不收集全树；区分已加载结果增长与泄漏 | Pending |
| PERF-004 | P2 / H | 图标失败 / 过期、工具页 / 隐藏，观测请求次数与并发 | 保留128项 / 4并发 / 30s规则，无立即重试风暴或隐藏补发 | Pending |
| PERF-005 | P2 / A | 运行 projection_measure / task_detail_measure Release，固定 fixture重复 | 保留原始样本；共享文本不深拷贝正文，合成申请字节不写成 RSS 改善 | Pending |
| PERF-006 | P1 / M | 主进程空闲、激活、隐藏<15s、超期、30min空闲逐阶段测量 | 记录 footprint / RSS / CPU及对象归属，解释缓存平台期；无无因忙轮询 | Pending |
| PERF-007 | P1 / H | Quick Look / Web / Settings / History 开关100轮，每轮经历取消与超期 | 无持续增加的有效资源、源读槽、任务 / Web文档；helper按归属看趋势 | Pending |
| PERF-008 | P1 / H | 接近剪贴板容量、持续任务输出、大历史输入、同时多窗口 | 各项硬预算成立；无卡死、无限内存或错误跨窗口发布 | Pending |
| PERF-009 | P2 / H | 8h长稳：定时复制、搜索、任务、Web开关、休眠恢复 | 记录资源 / 错误 / 崩溃曲线；无不可恢复输入和未解释的持续增长 | Pending |
| PERF-010 | P2 / H | 与同机前一基线按相同脚本复测，记录所有差异 | 没有基线时只报告新样本，不能编造50MB / 固定毫秒承诺；显著劣化登记复现 | Pending |

以上次数是本清单建议的测量规模，不是产品 SLA。[性能指南](performance.md)
定义指标边界。没有当前同机基线时，不得把采样完成写成已满足未知性能 SLA。

### 6.25 REL：工程门禁、持久化升级与发行

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| REL-001 | P0 / A | 当前完整差异运行 check-gpui、check、严格Clippy、diff检查，保存命令退出码 | 所有必需门禁通过；忽略 / 跳过项目及原因明确，不搬用旧计数 | Pending |
| REL-002 | P1 / A | SDK打包、解包模板、Web / background示例构建、生成物与来源检查 | 新开发者可使用SDK；依赖锁 / 公共源 / 许可证 / Schema一致 | Pending |
| REL-003 | P1 / A | Linux / Windows portable CI或对应环境命令 | 仅共享Rust crate及测试目标的相应检查通过，不宣称已交付跨平台GPUI App | Pending |
| REL-004 | P0 / H | 复制旧测试数据升级当前包，验证偏好 / Usage / 收藏 / 历史策略 / 插件批准 | 按各自契约迁移，不混合三类历史；损坏 / 未来schema不被静默覆盖 | Pending |
| REL-005 | P0 / M | 构建目标包，核对 Bundle ID、版本、构建号、架构、最低系统、许可证和动态依赖 | 与源码 / provenance一致；无旧UI / 桥接产物，无本机target动态路径依赖 | Pending |
| REL-006 | P0 / M | 严格签名、ZIP / sha256、归档内plist / 主程序 / 图标一致性，解压运行 | 签名与摘要有效，归档保留必要结构，运行的确是该解压产物 | Pending |
| REL-007 | P1 / M | 同签名升级、App改位置、干净测试用户安装，重新授权 / 重启 | 正常恢复数据和热键；权限身份变化有可理解行为，不误用旧实例 | Pending |
| REL-008 | P1 / M | 正式分发专项：Developer ID、公证、Gatekeeper、干净机安装升级 | 未具备时记录Blocked / 未交付，不能以本机自签成功宣称正式发行完成 | Pending |
| REL-009 | P0 / H | 检查交付包 / 证据 / 日志中测试正文、DB、路径、签名材料 | 只交付允许资源和脱敏证据；不包含个人库、凭据、私钥或完整Context | Pending |

### 6.26 RACE：跨模块高风险组合回归

这些组合专门检查模块之间的顺序，不替代上面的单项。控制顺序失败必须保留
最小复现，修复后重新执行原序列。

| ID | 级别 / 方式 | 前置与操作 | 验收预期 | 状态 |
| --- | --- | --- | --- | --- |
| RACE-001 | P0 / A | 查询A后查询B；A较晚发布；重复Session、revision、seq、Catalog各类失效 | 任何旧版本均不能覆盖B或重新获得执行资格 | Pending |
| RACE-002 | P0 / H | 动作提交后阻塞worker，改目标 / token / Catalog / 权限再释放 | 真正副作用前再次拒绝；没有错误目标执行或错误成功Usage | Pending |
| RACE-003 | P1 / H | 文件Actions A→B，A先到发布点但B已准备；反过来B先回A后回 | Core与GPUI两层均保持B；新请求不被旧请求的inputs_epoch抢占 | Pending |
| RACE-004 | P1 / H | 剪贴板首次读→新查询→复制→新操作→隐藏→恢复，按不同顺序回包 | 查询、loading、反馈、选择与返回焦点均只由当前资格控制 | Pending |
| RACE-005 | P1 / H | Preview准备 / 原生show / 系统关闭 / Settings打开 / 新预览交错 | 旧请求不重开，旧关闭不关新预览；Settings不遗留资源 | Pending |
| RACE-006 | P0 / H | 设置保存→无关查询错误→请求退出→继续编辑 / 新草稿→保存回包 | 单一保存通道；新草稿 / 取消保留，不能迟到退出丢数据 | Pending |
| RACE-007 | P0 / H | Web打开 / Reload / resume→隐藏 / 撤销 / 新页→旧IPC / close / error | 旧句柄释放且无越权副作用，新页不被旧事件改写或关闭 | Pending |
| RACE-008 | P1 / H | 历史读取占槽→新查询等待→配置变化 / 隐藏→旧读返回 | 旧等待者撤销；槽直到物理I/O结束才释放，不超配worker | Pending |
| RACE-009 | P1 / H | 新任务启动但旧列表先回→手工选别项→新列表出现→旧详情返回 | 尊重最新人工意图，不能选错或停止隐藏任务 | Pending |
| RACE-010 | P0 / H | 多窗口同时开、权限撤销、App退出，后台任务 / Web / 预览均在途 | 退出不再发布新UI或开始未获准副作用；已提交OS调用按其契约清理，受管进程停止，不跨窗口误操作 | Pending |

## 7. 18 个应用历史来源参数矩阵

每一行分别展开：HIST-001 默认 / 开关、HIST-003 检测、HIST-008 解析、HIST-009
查询、HIST-011 / 013 类型化打开、HIST-012 目标复验、HIST-015 撤销、HIST-017
错误隔离。至少记录“合成解析结果”和“原应用真实打开”两个结果；安装不足只阻塞
对应实机项目，不将其他来源自动判定失败或通过。

`idea` / `vscode` 继承只说明共用解析器，应用身份、Action、检测路径和真实打开
仍逐来源验收。多版本 / 多安装须额外执行拒绝歧义；不能宣称已支持 Profile 选择。

| 参数 ID / source.id | 应用 / 解析家族 | 路径 / 目标重点 | 合成与设置 | 原应用打开 |
| --- | --- | --- | --- | --- |
| SRC-01 / idea | IntelliJ IDEA / XML | Ultimate / Community / EAP；recentProjects；directory | Pending | Pending |
| SRC-02 / pycharm | PyCharm / idea | Professional / Community / EAP；PyCharm* / PyCharmCE* | Pending | Pending |
| SRC-03 / webstorm | WebStorm / idea | WebStorm* / EAP；directory | Pending | Pending |
| SRC-04 / goland | GoLand / idea | GoLand* / EAP；directory | Pending | Pending |
| SRC-05 / clion | CLion / idea | CLion* / EAP；directory | Pending | Pending |
| SRC-06 / phpstorm | PhpStorm / idea | PhpStorm* / EAP；directory | Pending | Pending |
| SRC-07 / rubymine | RubyMine / idea | RubyMine* / EAP；directory | Pending | Pending |
| SRC-08 / datagrip | DataGrip / idea | DataGrip* / EAP；directory | Pending | Pending |
| SRC-09 / rustrover | RustRover / idea | RustRover* / EAP；directory | Pending | Pending |
| SRC-10 / rider | Rider / idea | recentSolutions / recentSolution；.sln / .slnx / .slnf 保持 file | Pending | Pending |
| SRC-11 / android_studio | Android Studio / idea | Google/AndroidStudio*；directory | Pending | Pending |
| SRC-12 / vscode | VS Code / SQLite | .vscode-shared优先；旧Code库；directory/file/workspace/remote | Pending | Pending |
| SRC-13 / vscode_insiders | VS Code Insiders / vscode | Code - Insiders；固定应用身份 | Pending | Pending |
| SRC-14 / cursor | Cursor / vscode | Cursor配置；不等于支持读取当前Agent会话目录 | Pending | Pending |
| SRC-15 / windsurf | Windsurf / vscode | Windsurf配置；remote仅复制 | Pending | Pending |
| SRC-16 / vscodium | VSCodium / vscode | VSCodium配置；workspace不任选根 | Pending | Pending |
| SRC-17 / sublime | Sublime Text / JSON | 4 / 3位置；folder_history / file_history / recent_workspaces；忽略缓冲区正文 | Pending | Pending |
| SRC-18 / obsidian | Obsidian / JSON | obsidian.json；vault ID区分同名；不提供最近笔记 | Pending | Pending |

来源真实定义在[内置来源目录](../crates/anywhere-core/resources/application-history)，
详见[来源指南](application-history-sources.md)。基础用例数不等于最终执行次数；
来源、权限、语言和显示环境的参数展开必须在 Run ID 中记录。

## 8. 自动化入口与功能追溯

以下是模块级入口，不表示单个文件覆盖该组全部人工行为。新增 / 缺失的边界可在
对应模块补可控回归；无法在本次执行准备时记录 Blocked。

| 用例组 | 当前行为 / 实现 | 可复用自动化入口 |
| --- | --- | --- |
| ENV、LCH、KEY | [app](../apps/gpui/src/app.rs)、[Launcher input](../apps/gpui/src/launcher/view/input.rs)、[导航](../apps/gpui/src/launcher/view/navigation.rs)、[交互契约](launcher-interaction.md) | [GPUI view tests](../apps/gpui/src/launcher/view/tests/mod.rs)、[session](../crates/anywhere-core/tests/mvp/session.rs) |
| CTX | [默认输入契约](context-input-navigation.md)、[context_capture](../crates/anywhere-core/src/application/context_capture.rs) | [context sources](../crates/anywhere-core/tests/mvp/context_sources.rs)、[capture tests](../crates/anywhere-core/src/application/context_capture/tests.rs) |
| APP、MENU | [应用索引](../crates/anywhere-core/src/application/application_index.rs)、[菜单契约](current-app-actions.md) | [menu](../crates/anywhere-core/tests/mvp/menu.rs)、[图标cache](../apps/gpui/src/launcher/icon_cache.rs)、`scripts/check-macos-menu.sh` |
| ACT | [URL动作](../crates/anywhere-core/src/features/url/actions.rs)、[原生文件动作](../crates/anywhere-core/src/features/filesystem/actions_file.rs)、[Usage契约](launcher-interaction.md) | [search](../crates/anywhere-core/tests/mvp/search.rs)、[Usage独立性](../crates/anywhere-core/tests/mvp/usage_independence.rs)、[review regressions](../crates/anywhere-core/tests/mvp/review_regressions.rs) |
| FILE、PRE | [文件搜索](file-search.md)、[预览](native-preview.md)、[对象动作](../crates/anywhere-core/src/application/object_actions.rs) | [file search](../crates/anywhere-core/tests/mvp/file_search.rs)、[scan](../crates/anywhere-core/src/features/filesystem/scan/tests.rs)、[异步回归](../apps/gpui/src/launcher/view/tests/async_regressions.rs) |
| CLIP、SHELF | [Clipboard service](../crates/anywhere-core/src/features/clipboard/service.rs)、[page model](../apps/gpui/src/launcher/pages.rs) | [service tests](../crates/anywhere-core/src/features/clipboard/service_tests.rs)、[persistence](../crates/anywhere-core/src/persistence/clipboard.rs)、[异步回归](../apps/gpui/src/launcher/view/tests/async_regressions.rs) |
| SET、HOT | [偏好](../apps/gpui/src/host/preferences.rs)、[settings controller](../apps/gpui/src/launcher/view/settings/controller.rs)、[hotkeys](../apps/gpui/src/host/hotkeys.rs) | [shortcut facade](../crates/anywhere-core/tests/mvp/shortcuts.rs)、[生命周期回归](../apps/gpui/src/launcher/view/tests/lifecycle_regressions.rs) |
| HIST、SRC | [历史契约](application-history.md)、[来源指南](application-history-sources.md) | [MVP history](../crates/anywhere-core/tests/mvp/application_history.rs)、[adapter tests](../crates/anywhere-core/src/application/application_history/adapters_tests.rs)、[读取/脚本tests](../crates/anywhere-core/src/application/application_history/tests.rs) |
| GIT、DEV、TASK | [开发动作](developer-actions.md)、[进程manager](../crates/anywhere-core/src/core/execution/mod.rs) | [git](../crates/anywhere-core/tests/mvp/git.rs)、[developer](../crates/anywhere-core/tests/mvp/developer.rs)、[execution sessions](../crates/anywhere-core/tests/execution_sessions.rs)、[优化回归](../apps/gpui/src/launcher/view/tests/optimization_regressions.rs) |
| PKG、WEB、BG | [插件设计](web-plugin-design.md)、[SDK指南](plugin-development.md)、[Web surface](../apps/gpui/src/launcher/view/web_surface.rs) | [package tests](../crates/anywhere-core/src/application/web_plugins/package_tests.rs)、[broker](../crates/anywhere-core/src/application/web_plugins/broker_tests.rs)、[input](../crates/anywhere-core/src/application/web_plugins/input_tests.rs)、[background](../crates/anywhere-core/src/application/web_plugins/background_tests.rs)、生命周期回归 |
| JSON | [JSON示例说明](../plugins/examples/json-workspace/README.md)、[示例源码](../plugins/examples/json-workspace/src) | JSON包的 `npm test`、`scripts/check-web-examples.sh` 的真实Host/SDK合成链路 |
| DSL | [架构](architecture.md)、[模块贡献](../crates/anywhere-core/src/features/contribution.rs) | [plugin](../crates/anywhere-core/tests/mvp/plugin.rs)、[extension lifecycle](../crates/anywhere-core/tests/mvp/extension_lifecycle.rs) |
| UI、PERF、REL | [UI规范](ui-design.md)、[性能](performance.md)、[发行](releasing.md) | `scripts/check-gpui.sh`、`scripts/check.sh`、[scripts tests](../scripts/tests)、[portable CI](../.github/workflows/portable.yml) |
| RACE | Session / revision / Catalog / 请求代次门禁 | async / lifecycle / optimization回归、[执行门禁](../crates/anywhere-core/src/application/execution_request.rs)、[Core对象动作测试](../crates/anywhere-core/src/application/object_actions.rs) |

上一轮 2026-11-17 的本地工程记录为 GPUI 244 项通过、工作区 Rust 928 passed /
6 ignored，完整门禁通过，见[生命周期审查](archive/reviews/gpui-lifecycle-review-2026-11-17.md)。
这只作为已有自动化入口的记录；**本清单编制未重跑这些工程测试，未继承其中任何
物理验收 Pass**。实际执行时记录本次命令、结果、源码差异和 App 摘要。

## 9. 当前不交付范围与最终报告

| 范围 | 处理方式 |
| --- | --- |
| Windows / Linux GPUI App、其他平台原生预览 | 未交付；不安排成功运行验收，只做已提供的共享crate检查 |
| PTY / 交互式终端、任务跨重启恢复 | 未实现；Tasks不按终端或恢复系统验收 |
| 浏览器历史、书签与Tab管理、Obsidian最近笔记、多个Profile / 实例选择 | 未交付；不能用最近项目 / vault列表冒充 |
| 插件市场、自动更新、第三方进程沙箱、Shell / Node / 任意文件与网络能力 | 未交付；需要验证边界拒绝，不提供成功能力用例 |
| 菜单长期Usage与持久快捷键 | 当前安全隔离，不把“恢复旧菜单频次”当预期 |
| 快捷键配置导入 / 导出UI | 旧指南有描述，但当前GPUI未发现可达入口；只验现有配置兼容，不虚构成功UI流程 |
| 正式公开签名 / 公证与完整发行安全矩阵 | 尚未完成，REL-008单独报告，不与开发预览本机包混为通过 |
| 整App固定内存或毫秒SLA | 当前无此承诺；按PERF组采集、同机对照与资源归属解释 |

最终报告至少列出：被测源码 / 包身份、环境、基础用例与参数实例数量、各优先级
Pass / Fail / Blocked / Pending / N/A 数、缺陷与复验、18来源真实覆盖表、物理
输入与权限覆盖、资源样本、未覆盖范围。失败项保留原复现序列；未执行不得从
旧截图、旧构建记录或单测通过数推定成功。
