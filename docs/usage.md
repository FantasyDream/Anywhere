# 使用指南

macOS 现行使用指南；应用历史属于 V0.14 开发预览。实际版本与验收状态见[项目首页](../README.md)和 [V0.14 记录](mvp-v0.14.md)。

## 1. 启动与授权

打开 `Anywhere.app`。首次启动会显示 Launcher，之后通过快捷键或菜单栏打开；菜单栏还提供 Settings 和 Running Tasks。

部分能力需要系统授权，可根据使用场景开启：

| 权限 | 用途 | 设置入口 |
|---|---|---|
| 辅助功能（Accessibility） | 读取与执行应用菜单、读取选中文本、识别支持的 IDE 项目窗口 | macOS 系统设置 → 隐私与安全性 → 辅助功能 |
| 自动化（Automation） | 读取 Finder 目录 / 选中项，以及 Safari、Chrome、Edge 的当前网页 | macOS 系统设置 → 隐私与安全性 → 自动化 |
| 目录访问 | 搜索用户指定的目录 | Anywhere Settings → Search |
| 完全磁盘访问（可选） | 扩大受保护目录的可访问范围 | macOS 系统设置 → 隐私与安全性 → 完全磁盘访问 |

文件搜索在启动和修改搜索范围时准备目录授权，查询只使用可访问的范围。受保护或无法读取的位置会跳过；完全磁盘访问需要用户在系统设置中开启。

未授权的来源不会阻止其他功能。例如，缺少辅助功能权限时隐藏应用菜单，仍可使用不依赖该权限的动作。在 Settings → Context 查看和开关应用环境来源；剪贴板的采集状态、自动填入与历史配置统一位于 Settings → Clipboard。

## 2. 三种操作方式

| 方式 | 如何进入 | 用途 |
|---|---|---|
| **Current App** | 默认按 `⌘⌥Space` | 操作当前环境，搜索应用菜单及适用的目录、Git、开发项目等动作 |
| **Global Search** | 默认按 `⌘Space`，或通过搜索图标菜单切换 | 搜索应用、动作和收藏目录，查找 File Search 等工具入口 |
| **内容输入** | Current App 唤醒时自动填入可识别的选中文本或新复制内容 | 操作这份内容，例如复制 URL、打开路径 |

Current App 空输入首页和各动作 Scope 共用同一顺序：当前来源应用内执行次数优先，同次数看最近使用，再用上下文和默认优先级兜底。首页只显示前几项，没有动作白名单或额外推荐加分；常用的复制分支名等动作也能进入首页。菜单的长期历史仍按身份安全边界隔离，关键词搜索和文件 / 应用历史列表的排序不受此规则改变。

Current App 聚合当前环境的所有可用动作。顶部的 **Folder、Git、Rust 等 Scope** 用于缩小动作范围；切换 Scope 保留查询。菜单保留在当前应用中，可通过 **Browse Menus…** 按层级浏览，也可直接搜索菜单名称。

内容输入的优先级是：

1. 当前应用中能解析为 URL、真实文件 / 目录等具体 Context，或被已批准且启用的 Web 工具命令识别的选中文本。
2. 距今不超过 **15 秒**、复制时间可确认，且满足同样识别条件的剪贴板内容。
3. 没有任何识别器或命令匹配的普通文本、无效对象或没有可用内容时，进入 Current App。

输入框会显示内容来源，只解析该内容，不混入前台应用的环境 Context。启动前已有、无法确认复制时间的剪贴板内容不会自动填入。

内容输入时按 `Esc` 清空内容并返回 Current App，再按 `Esc` 进入空白 Global Search。更多规则见 [Context 输入与导航](context-input-navigation.md)。

## 剪贴板历史

剪贴板有两个独立 Action，都可在 Settings → Shortcuts 配置快捷键：

- **Clipboard History / 剪贴板**：在 Launcher 搜索框下打开历史页，适合查找后复制。
- **Clipboard Shelf / 剪贴板悬浮窗**：在鼠标所在屏幕右下角打开约 420×560 的紧凑置顶窗口，避开 Dock。单击只选择，双击 / Enter / 选中行的复制按钮复制并保留窗口焦点；`⌘↵` 复制并返回原应用，随后可自行粘贴。可见时自动更新记录，最小化时停止刷新，重复触发唤回同一窗口并重新定位。可拖动、调整尺寸；打开页面不会自动开启采集。

悬浮窗没有固定页脚或条数；选中行右侧提供复制和“更多操作 · ⌘K”，复制反馈留在原行。预览、删除、刷新收在更多操作中；空列表的搜索区仍提供操作入口。

悬浮窗支持完整键盘操作，无需 Tab：

| 操作 | 快捷键 |
| --- | --- |
| 选择上一条 / 下一条 | `↑` / `↓` |
| 按可见页移动、首条 / 末条 | `PageUp` / `PageDown`、`⌘↑` / `⌘↓` |
| 复制当前条目、前九项 | `Enter`、`⌘1–9`；复制后仍可继续选择 |
| 复制并返回原应用 | `⌘Enter`；不自动粘贴 |
| 聚焦搜索并选中查询 | `⌘F` |
| 打开操作列表 | `⌘K` 或右键条目；`↑↓` 选择、`Enter` 执行 |
| 开关内嵌预览 | `⌘Y`；查询为空时也可用空格 / `→` 打开、`←` 关闭 |
| 滚动预览 | `⌥↑↓` / `⌥PageUp` / `⌥PageDown` |
| 删除当前条目 | `⌘ShiftBackspace`；默认取消，`→` 选择删除、`Enter` 确认 |
| 刷新 | `⌘R` |
| 逐层退出 | `Esc` 先取消操作 / 删除确认，再关闭预览、清空查询，最后关闭窗口 |

预览保留列表并跟随选择，关闭后保留查询和条目位置；浏览列表时不加载完整正文 / 图片。鼠标悬停和拖动不会复制，删除确认绑定原条目，不因后台刷新而指向另一条。

- 默认不采集，在 **Settings → Clipboard → Record clipboard history** 开启后保存未来复制的文本、图片、文件列表、HTML / RTF 及可读取的自定义格式，关闭开关暂停。旧剪贴板不补录；macOS 需要允许剪贴板读取，后台不会反复弹权限申请。
- 输入检索内容，`↑↓` 选择，右侧始终预览，`Enter` / 双击 / 选中行按钮复制；复制反馈和“更多操作 · ⌘K”都在选中行，不显示固定页脚或条数。`⌘K` 或右键可复制、单条删除或刷新，空列表仍提供操作入口；采集开关、清空历史和 15 秒自动填入设置统一位于 **Settings → Clipboard**。`Esc` 关闭 Actions，再按 `Esc` 返回原搜索。
- 最多保存 1,000 条、128 MiB 数据，每条最多 16 MiB / 64 个格式；重复内容合并。历史期限与唤醒时的 **15 秒自动填入** 无关。
- 在“设置 → 剪贴板”中可暂停记录 5 / 15 / 60 分钟，或提前恢复。暂停期间的内容不补录，关闭记录会取消自动恢复；已有历史仍可使用。
- 展开“历史记录存储”可选择保留 1 / 7 / 30 / 90 天；默认沿用容量限制。缩短期限会确认并立即永久删除过期记录，之后延长期限也不会恢复。用量仅表示逻辑数据量，内容在本机明文保存；不影响系统剪贴板或原文件。
- 本地明文存储，只记录应用运行期间观察到的复制。文件列表仅保存原路径，恢复时原文件必须仍然存在；HTML 不执行脚本，图片只生成有界缩略图。文件承诺、延迟对象及任意多 item 私有格式不能保证完整恢复。开发及验收状态见 [V0.10 MVP](archive/mvp-v0.10.md)。

## Web 工具与 JSON Workspace（预览）

开发第三方插件请先看 [开发指南](plugin-development.md)：SDK 导出包内含独立模板和打包工具，不需要修改主程序或安装 Rust / Flutter。

在 Settings → Features → Web tools → Import 选择可信的本地 `.awplugin` 包，核对版本、内容摘要和权限后批准；随即可以搜索工具名称，无须重启。该区域还支持禁用 / 撤销、卸载、更新和恢复上一批准版本。批准更新会立即撤销旧 Web 调用，不会等待旧页面编辑完成；先保存需要保留的内容。卸载会清理该安装的私有 KV，没有“保留数据”选项。当前仅供 macOS 本地开发验收，不开放插件市场或自动更新；同 ID 的本地包更新不验证发布者身份。

运行 `python3 scripts/build-web-examples.py` 构建 [JSON Workspace](../plugins/examples/json-workspace/README.md) 与 [Text Pad](../plugins/examples/web-text/README.md)。导入 JSON 包后搜索 `JSON Workspace`：左侧输入停顿后自动格式化，上方默认使用 JavaScript，也可切到 JSONPath 并输入公式，`$` / `this` 自动补入，右侧展示和复制结果；`⌘↵` 立即查询，`⌘⇧C` 复制当前结果。输入 / 输出各最多 1 MiB，不保存正文，只保存缩进偏好。声明宿主 `inputMatchers` 的新版 JSON 包还支持自动识别对象 / 数组：直接输入 JSON 可匹配 Action，选择后带入原文；选中文本和 15 秒内新复制内容沿用自动填入规则。自动识别最多 16 KiB / 128 层，不将普通标量误当成 JSON。

Web 工具复用 Launcher 页框，`⌘K` 打开宿主操作，Esc 返回搜索，隐藏后的输入仍保留 15 秒。剪贴板操作经宿主单次确认。资源来自已批准包，网络、frame、弹窗和导航均受限制；这些策略不等于操作系统进程沙箱。原生 IME / 安全 / Windows 验收边界见 [V0.11 MVP](archive/mvp-v0.11.md) 与 [实施记录](web-plugin-design.md)。原生 JSON 工作区已退役，新工具使用独立的 Web Action ID。

## 3. 键盘操作

以下为 macOS 默认键位。两个唤醒快捷键和 Action 快捷键都可以在 Settings → Shortcuts 修改。

| 按键 | 操作 |
|---|---|
| `⌘⌥Space` | 唤醒 Current App |
| `⌘Space` | 唤醒 Global Search |
| `↑` / `↓` | 选择候选项 |
| `Enter` | 执行动作、打开对象或进入子菜单 |
| `⌘K` | 打开选中对象的二级 Actions；支持菜单浏览时也可进入菜单 |
| `Tab` / `Shift+Tab` | 向后 / 向前切换 Scope；文件搜索中切换文件类型（兼容 Ctrl 组合） |
| `→` / `←` | 输入为空时进入 / 返回菜单；有输入时保留光标移动 |
| `Esc` | 清空查询、返回上一级或关闭窗口，具体顺序取决于当前页面 |
| `⌘W` / `⌘M` | 关闭 / 最小化 Settings、Running Tasks 独立窗口 |

Launcher 的 `Tab` / `Shift+Tab` 用于切换可见分类，不遍历控件；中文组词期间交给输入法。重复当前唤醒入口可隐藏窗口，点击其他窗口也会隐藏。

**隐藏后的状态恢复：**

- 有查询、文件搜索页、菜单 / 二级操作、执行输出或已选的非 Current App Scope 时，保留操作状态 **15 秒**。
- 空白 Global Search 或 Current App 首页再次唤醒，按本次入口重新采集前台应用。
- 超过 15 秒清理 Launcher 会话。已经启动的后台任务继续运行。

详见 [Launcher 交互与状态保留](launcher-interaction.md)。

## 4. 常用场景

| 场景 | 操作示例 |
|---|---|
| 启动应用 | Global Search 输入应用名，例如 `chrome`，选择后按 Enter |
| 当前应用菜单 | Current App 输入菜单名称，或进入 Browse Menus… |
| Finder 目录 / 文件 | 唤醒后选择 Open Terminal Here、Copy Path、Open Directory 或 Open File |
| Git 仓库 | 搜索 Git Status、Git Diff、Git Log、Git Staged Diff、Git Branches |
| 复制 Git 信息 | Copy Branch Name、Copy Commit Hash、Copy Short Commit Hash、Copy Repository Path、Copy Remote URL |
| 网页 / URL | 在支持的浏览器唤醒，或输入完整 HTTP(S) URL，选择打开或复制 |
| 收藏目录 | 目录 Context 中执行 Save Directory；Global Search 按名称、`saved` 或 `bookmark` 查找 |

选中收藏目录后按 `⌘K` → **Forget Saved Directory** 可移除收藏，不删除磁盘目录。当前最多保存 100 个目录。

相同目标的动作会合并来源；剪贴板已包含相同 URL 或路径时，对应复制动作隐藏。不同目标仍保留来源标注。

路径需要真实存在，支持绝对路径、`~` / `~/…` 和中文全角 `～/…`，主目录简写会展开为实际路径。文件名中的空格与波浪号保留；不展开 `~其他用户`、环境变量或 shell 表达式，也不把裸域名自动推断为 URL。IDEA 项目目录识别需要窗口暴露明确路径；VS Code、Cursor / ChatGPT 对话项目目录尚未接入。

## 5. 文件搜索

1. 打开 Global Search，输入 `files`、`File Search` 或“文件搜索”。
2. 选择 **File Search**，按 Enter 进入文件页面。
3. 输入至少一个字符，搜索文件名或路径；用类型栏筛选文件、文件夹、文档、PDF、图片、音频、视频、压缩包和代码等类型。
4. 用 `↑` / `↓` 选择。右侧固定预览跟随选择，Enter 打开，`⌘K` 查看对象操作。

搜索窗口已去除通用页脚；文件页在类型筛选下方显示数量、进度与重试。选中行右侧提供打开和更多操作入口。结果连续追加，已显示的行不会因后续命中反复重排，没有分页操作。`Esc` 先清空文件查询，再返回父页面；返回按钮直接返回父页面。

在 Settings → Search 可以：

- 开关文件搜索，设置搜索目录（每行一个绝对路径，留空使用 Home）。
- 开关隐藏文件；新配置默认包含隐藏文件。
- 按目录名配置排除项，例如 `node_modules`。排除会缩小搜索范围。
- 为保存的搜索目录授权，按需要打开完全磁盘访问设置。

搜索结合 **Spotlight 快速结果与完整文件名遍历**，无需安装 `rg`。遍历包含未索引文件和被 `.gitignore` 忽略的文件；隐藏文件受设置控制。符号链接本身可列出，不沿链接继续遍历目标。可搜索范围受目录设置、排除项和系统权限限制。

当前预览提供元数据与有界文本读取，输入和解码后的 UTF-8 输出各最多 64 KiB；支持 UTF-8 / BOM 标记的 UTF-16，常见文本与源码可推测 GBK 等编码并显示提示。PDF、图片、音视频等尚无完整富预览。候选图标按文件类型分类，不生成逐文件缩略图。

可为 File Search 配置一个普通 Action 快捷键，并按需开启 Global，直接从其他应用进入文件页。

## 6. 开发项目与后台任务

目录 Context 会进一步识别项目类型，提供相应动作：

| 项目 | 动作 |
|---|---|
| Rust / Cargo | Cargo Check、Cargo Test、按明确 package / binary 目标运行 |
| Maven | Maven Test、Maven Package |
| Gradle | Gradle Build、Gradle Test |
| Node | `package.json` 声明的 scripts，按项目配置选择 npm / pnpm / yarn |
| Spring Boot | 项目声明支持时提供 Maven / Gradle Run |

Maven、Gradle 优先使用项目 wrapper。识别阶段只读取项目标记及必要元数据；真正启动命令发生在执行 Action 时。

构建、测试和运行进入后台进程会话。通过菜单栏 **Running Tasks** 查看状态、stdout / stderr、退出码或停止任务。关闭 Launcher / 任务窗口不停止任务；退出 Anywhere 会停止任务。

当前会话只在应用运行期间保留，尚不提供交互式 PTY 终端或应用重启后的任务恢复。

## 7. 自定义快捷键与扩展

**快捷键：** Settings → Shortcuts → Action shortcuts 支持固定动作绑定、Global 开关、按应用 / Context 分派的意图，以及配置导入 / 导出。触发时重新解析适用对象：一个目标直接执行，多个目标先选择，缺少 Context 时不执行。

动态应用菜单不显示原应用快捷键，当前不支持长期自定义绑定；菜单也暂不记录使用频次，避免不稳定菜单身份关联旧记录。

**扩展：** Settings → Features 管理第一方模块及已安装的声明式插件。

- 内置模块包括 URL、Git、Maven、Gradle、Node、Rust、Spring Boot、收藏目录、剪贴板历史。
- 用户插件放入 `~/Library/Application Support/Anywhere/plugins/`，支持 `*.toml` 或子目录中的 `manifest.toml`；安装或修改后重启；新批准下次启动生效，撤销立即失效。**Refresh** 只刷新设置和业务数据。
- v2 / v3 插件先 **Review permissions → Allow for next launch**，然后重启激活。撤销权限或关闭模块后，相关候选与快捷键立即失效；绑定配置保留以便重新启用。此批准流程与 Web 包批准后即时可用的流程不同。
- 现有外部 v2 支持声明式复制、打开 URL / 文件，v3 有 Detail / KeyValue 开发切片；这条 DSL 路线停止扩展。新插件使用 WebView，本地页面、安装入口和 [SDK](../packages/plugin-sdk/README.md) 已进入 macOS 开发预览；按 [开发指南](plugin-development.md) 开始。平台与发行边界见 [方案与进度](web-plugin-design.md)，Terminal 延后。

示例：[v1 搜索插件](../plugins/examples/search.toml)、[v2 复制插件](../plugins/examples/copy-v2.toml)、[v3 Detail 插件](../plugins/examples/context-detail.toml)。搜索示例中的 `example.com` 是占位地址，需要自行配置；应用不自动安装 Web Search 插件。

## 8. 本地数据

macOS 数据目录：`~/Library/Application Support/Anywhere/`。

| 文件 / 目录 | 内容 |
|---|---|
| `settings.json` | 唤醒键、Action 快捷键、来源开关及搜索设置 |
| `usage.sqlite3` | 动作 ID、Context 类型、来源应用、成功执行次数与时间 |
| `features.json` | 内置功能开关，启动时从旧配置迁移 |
| `extensions.json` | 外部插件开关和能力授权；旧第一方键保留为迁移备份 |
| `extension-storage.sqlite3` | 按模块隔离的持久数据，例如主动收藏的目录路径 |
| `plugins/` | 用户安装的旧声明式插件 |
| `clipboard-history.sqlite3` | 开启采集后保留的多格式剪贴板内容与检索数据 |
| `web-packages/` | 已导入的 Web 包、资源及 `receipts.json` 安装账本 |
| `web-storage.sqlite3` | Web 插件按命名空间隔离的 KV 数据 |
| `launcher.log` | 运行诊断和耗时日志，每次启动覆盖 |

Usage 不保存输入正文或文件路径。Context 正文属于临时会话；文件搜索命中存入查询临时文件，切换查询或会话清理时释放，不维护长期文件索引。

## 语言与拼音

分支 `codex/dev-0.12-i18n` 已接入官方 Flutter 中英国际化框架；设置 → General / 通用 → Language / 语言可选择跟随系统、简体中文或 English。已覆盖主窗口、文件 / 剪贴板、任务、设置、原生菜单及 Web 宿主；第三方菜单、用户内容和底层诊断保留原文。验收范围见 [V0.12 计划](mvp-v0.12.md)。

应用和动作的中文显示名称 / 既有别名、文件名称支持全拼与首字母匹配，例如 `weixin` / `wx`、`kaifawendang` / `kfwd`。系统应用支持自身提供的中文名称：“提醒事项”可用 `Reminders` 或 `txsx` 查找，“文件搜索”可用 `wjss` 查找，搜索别名不受界面语言限制。文件搜索继续完整遍历授权目录；拼音不扩大权限、排除项或类型范围。切换语言不改变查询、ActionId 和快捷键；不转换文件名 / 路径、外部应用菜单或全部剪贴板正文。


## 应用历史（V0.14 原型）

已适配的 18 个来源仅默认开启已安装应用的 Action；未安装应用默认关闭，手动开关优先。安装或卸载应用后重启可更新默认判断。已开启来源检测标准配置目录中的历史文件，包括 JetBrains 系列、VS Code / Insiders / Cursor / Windsurf / VSCodium、Sublime Text 和 Obsidian 最近仓库。

设置中的“应用历史”使用紧凑来源列表；按应用名筛选，或只显示已启用来源。展开某个应用可重新检测、保存自定义路径或还原尚未保存的编辑。关闭 Action 不删除位置，也不阻止预先配置；修改位置不会重新开启已关闭的 Action。路径草稿在筛选和切换分类时保留，关闭设置前可选择保存、继续编辑或放弃；保存失败会留在设置并显示对应来源，新的编辑不会被先前保存覆盖。

在对应应用唤醒 Current App，会自动匹配其“最近项目”入口；也可全局检索应用名 + “最近项目”，或通过独立 Action 快捷键打开。进入后固定查询该来源，支持标题、路径、中文和拼音；↑↓选择、PageUp / PageDown 按可见页移动，Enter 或双击在来源应用打开，远端条目直接复制 URI。右键或行内更多操作查看所选对象适用的文件 / 目录、Git、项目动作，或刷新；历史页不绑定或显示 ⌘K；远端仍只复制 URI。动作结果返回后保留查询和选择，不改变 Current App。Esc 先清空查询，再返回父页面；页头返回按钮直接返回。没有新增默认全局快捷键或 Scope。

当前每来源使用一个自动检测或自定义文件。自动检测只检查文件位置与可读性，使用对应 Action 时才读取历史正文。远端条目只支持复制；目标已失效、来源格式不支持、对应应用缺失或有多个安装实例时不会猜测打开。浏览器历史 / 书签 / Tab 管理尚未实现，已移至[独立浏览器阶段](browser-feature.md)；Obsidian 最近笔记及多实例仍待实现。Obsidian 当前列出已登记的仓库，并按仓库 ID 用 Obsidian 打开；不遍历全部笔记冒充历史。非标准配置位置、便携版或额外启动参数指定的数据目录可手动配置。接入文件、脚本和限额见[来源指南](application-history-sources.md)。

用户可在“应用历史 → 自定义适配”打开目录，新增 `source.json` / `transform.js` 并重启；同 ID 用户定义覆盖内置适配，或用 `extends` 复用内置解析器。格式及资源限制见[来源指南](application-history-sources.md)。
