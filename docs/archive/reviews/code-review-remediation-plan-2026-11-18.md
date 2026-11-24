# 全仓代码审查修复方案

2026-11-18。基线为 `codex/docs-v0.14-usage` 的 `3df3fe1`，加上同日尚未提交的
候选行 / 选中态 UI 修改（见 R0）。本文是**实施方案**，撰写时没有修改任何产品代码；
各项完成后应在对应专题指南更新现行说明，并在本文末尾追加实施记录。

审查依据：代码规模与热点、依赖树、Release 产物体积、一轮比默认严格的 Clippy
（`redundant_clone`、`too_many_lines`、`significant_drop_tightening` 等，仅作信号）、
锁使用的逐点分类，以及渲染、键盘、搜索、持久化、Web 主题路径抽读。未做运行时
profiling 和桌面实测；所有性能项都以“先测量、再决定”为门槛。

## 总览

| ID | 优先级 | 问题 | 方案摘要 | 主要风险 |
|---|---|---|---|---|
| R0 | 前置 | 候选行 / 选中态修改未提交 | 单独提交并完成桌面验收 | 无 |
| R1 | P1 | 锁中毒处理有 4 种写法，撤销路径会被静默跳过 | 按“授权拒绝 / 撤销必达 / 缓存重置”统一 helper，并加架构规则 | 改变中毒后的行为 |
| R2 | P1 | `variants` 中 94 个颜色未被使用且已漂移；对比度靠写死的 alpha | Token schema v3：生成完整调色板，`theme.rs` 只做选择 | 颜色回归 |
| R3 | P1 | Web `--aw-*` 映射为手写两份，缺少对比度 / 不透明回退 | 由生成器输出 Web 变量表，与 R2 同一 PR | Web 插件视觉回归 |
| R4 | P1 | 763 行单函数键盘分发，未用 GPUI action / keymap | 先按优先级分层（行为不变），再把固定命令迁到 action | 按键优先级回归、IME |
| R5 | P1 | `LauncherView` 104 个字段、`build` 705 行、滚动句柄双字段 | 按职责拆为普通子结构体，去除别名字段 | 大量机械改动 |
| R6 | P3 | 各列表行高 / 内边距 / 图标尺寸各不相同，约 500 处像素字面量 | 通用 `ListRow` 组件 + 行规格 token + 字面量棘轮 | 视觉回归 |
| R7 | P3 | 21 个函数超过 200 行 | 拆分后以 Clippy 阈值 200 锁定 | 机械改动 |
| R8 | P2 | 无 `[profile.release]`，产物 45.6 MB | Thin LTO / codegen-units / strip / dSYM 对照实验 | 构建时间、符号化 |
| R9 | P2 | 首页每帧整表克隆候选行及其字符串 | 行文本改为共享字符串，按下标渲染 | 低 |
| R10 | P2 | 拼音匹配逐字符查表并分配 | 测量达标才实施：字段级预计算读音 | 内存 |
| R11 | P2 | 64 处锁可提前释放，部分持锁读写 SQLite | 测量达标才实施：剪贴板仓库锁与状态锁分离 | 锁顺序 |
| R12 | P4 | 多余 clone、按值传参、排序重复查找 | 一次性清理 | 低 |

明确不处理：52 组重复依赖（来自 GPUI / objc2 上游，二进制只含少量 `gpui-pre`
共享组件，并未编进两套平台层）；本机 39 GB `target/` 属本地环境，可自行
`cargo clean`，不进仓库。

## 实施顺序与 PR 划分

1. **PR-0（R0）**：提交现有 UI 修改，独立验收，避免与后续重构混在一起。
2. **PR-1（R1）**：锁策略。只动 Core 与 `apps/gpui/src/host/`，不碰 UI。
3. **PR-2（R2 + R3）**：Token schema v3 与 Web 主题生成。第一步要求渲染值逐角色
   等价，视觉调整另起提交。
4. **PR-3（R4 阶段一）**：键盘分层，行为不变。先补特征测试再重构。
5. **PR-4（R5）**：`LauncherView` 状态拆分。依赖 PR-3 已把键盘逻辑分散到各控制器。
6. **PR-5（R6 + R7 + R9）**：通用列表行、长函数拆分、共享字符串。
7. **PR-6（R4 阶段二）**：固定命令迁移到 GPUI action。
8. **PR-7（R8）**：Release profile 实验与采用。
9. **PR-8（R10、R11，按测量结果）与 PR-9（R12）**。

每个 PR 的门禁：受影响 crate 的定向测试 → `scripts/check-gpui.sh` →
`scripts/check.sh`。涉及外观、输入、窗口的 PR（2、3、5、6）另需按
[GPUI 验收](../../mvp-gpui.md#物理桌验矩阵)执行桌面检查，结果与自动测试分开记录。

## R0 提交现有 UI 修改

现状：工作区里有候选行副标题去重、Core 副标题不再拼接空详情、选中边框弱化、
hover 与选中区分、边框不跳动、标题与搜索框对齐等修改，`check.sh` 已通过（941 项）。

步骤：单独提交；按 `ui-design.md` 的对齐与选中规则做浅 / 深色、增强对比度、
减少透明度下的桌面检查。R6 会把同一规则推广到所有列表，因此 R0 必须先落地。

## R1 统一锁中毒策略

### 现状

`anywhere-core` 生产代码中的锁调用分类：77 处转为错误、26 处 `if let Ok` /
`let Ok … else` 跳过、21 处 `.lock().unwrap()`、10 处 `into_inner` 恢复、4 处
`expect`、16 处其他形式。问题不在写法多，而在语义没有区分：

- **撤销被跳过**：`launcher_service.rs:206`（保留期到期后的 Web 失效）与
  `set_capture_sources` 闭包中的 Web 失效使用 `if let Ok`，锁中毒时撤销不执行，
  违反“副作用立即撤销”。
- **设置被静默忽略**：`context_capture.rs:112` 在策略锁中毒时既不更新来源开关，
  也不调用撤销闭包；用户关闭来源后，旧授权仍可能有效。
- **清理被跳过**：`web_plugins/background_action.rs:40` 的 `Drop` 不关闭文档；
  `launcher_service.rs:263` 的 `replace_registry` 直接返回，旧 registry 与 Web
  执行权限都保留。
- **级联 panic**：应用历史服务 14 处、文件搜索 7 处 `.lock().unwrap()`，一次 panic
  会让后续所有请求 panic。

### 规则

| 类别 | 锁不可用时 | 适用 |
|---|---|---|
| 授权 / 发布 | 拒绝（返回错误或不发布），记录 `error` | 能力检查、候选发布、RPC 准入 |
| 撤销 / 清理 | 恢复守卫并**继续执行**，记录 `error` | Web 失效、文档关闭、会话放弃、来源关闭 |
| 纯缓存 | 恢复守卫并重置为默认值 | 目录 memo、offer / 投影 / 快照缓存 |
| 服务状态 | 转为错误返回，不 panic | 应用历史、文件搜索、扫描队列 |

撤销操作本身是幂等的，恢复中毒守卫后执行不会扩大权限，因此“撤销必达”优先于
“数据可能不一致”。

### 步骤

1. 新增 `crates/anywhere-core/src/core/sync.rs`，提供带锁名的四个 helper（Mutex /
   RwLock 各一组）：`lock_or_deny`（返回 `Result`）、`lock_for_revoke`（恢复并记录）、
   `lock_cache`（恢复并 `Default` 重置）、`lock_state`（返回 `Result`，供服务状态使用）。
   `core` 不依赖 application，符合现有架构规则。
2. 按附录 A 逐点替换。撤销点需重新审视调用顺序，确保仍遵守现有的
   capture → session → web 锁顺序（`set_capture_sources` 注释已有约束）。
3. `replace_registry` 在 Web 锁不可用时：不替换 registry，但通过
   `lock_for_revoke` 执行 `invalidate`，并返回 / 记录错误，而不是静默返回。
4. `apps/gpui/src/host/` 同样适用（如 `hotkeys.lock().ok()`、偏好读取）。
5. `scripts/architecture_rules.py` 新增 `check_lock_usage`：Core 生产代码禁止
   `.lock().unwrap()`、`.lock().expect(`、`.read()/.write()` 的同类写法；在
   `scripts/tests/test_architecture_rules.py` 增加正反 fixture，并由
   `check-architecture.py` 扫描。`if let Ok` 无法静态判定语义，靠第 6 步的测试覆盖。

### 测试

6. 每个撤销点一条中毒回归：在子线程持锁 panic 使锁中毒（沿用
   `core/execution/mod.rs` 现有 `catch_unwind` 写法），再调用撤销入口，断言 Web
   失效 / 文档关闭 / 来源关闭已发生。
7. 授权点断言中毒后返回拒绝或 `HostFailure`，而非 panic 或放行。
8. 应用历史与文件搜索断言中毒后返回错误，且后续调用不 panic。

完成标准：附录 A 全部处理；架构规则生效；`check.sh` 通过。
文档：在 [architecture.md](../../architecture.md) 的并发约定中写入上表。

## R2 Token schema v3：完整调色板

### 现状

- `resources/design-tokens.json` 的 `variants.light`（41）、`variants.contrast.dark`
  （26）、`variants.contrast.light`（27）共 94 个颜色，生成器只校验键名，不输出到
  Rust 或 CSS。
- 实际浅色使用另一组 `glassLight*`，两者已不一致（如 `keycapSurface`
  `#0a000000` 与 `#80ffffff`，`window` `#c8f5f6f8` 与 `#2efafbfd`）。
- 增强对比度由 `theme.rs` 的 `with_contrast(value, alpha)` 写死 alpha 实现（如
  `window` 0x4d / 0x85、`selection_outline` 0x8a / 0x68、`border` 0x66），
  `muted` 在对比度模式下直接改用 `secondary`。
- 同一角色按前缀散落为 `x`、`glassLightX`、`glassDarkX`、`onDarkX` 四种常量。

### 目标结构

```json
{
  "schemaVersion": 3,
  "palettes": {
    "opaqueDark": { "primary": "#fff9fafc", "...": "..." },
    "glassDark":  { "extends": "opaqueDark", "accent": "#ffd4e6ff" },
    "glassLight": { "extends": "opaqueDark", "primary": "#ff14181f" }
  },
  "contrast": {
    "opaqueDark": { "border": "#66ffffff" },
    "glassDark":  { "window": "#8516191e" },
    "glassLight": { "window": "#4dfafbfd" }
  },
  "fixed": { "menuSurface": "#ff2b2e35", "onDarkPrimary": "#fff9fafc" }
}
```

- `palettes` 按角色名（不带前缀）定义三种表面；`extends` 只允许一层，生成器展开后
  要求每个调色板角色集合完全相同。
- `contrast` 是对应调色板的覆盖层，生成 `*_CONTRAST` 三个完整调色板。
- `fixed` 收纳与外观无关的颜色（`menuSurface`、`modalScrim`、`onDark*`、
  `primaryButtonSurface` 等）。
- 删除 `variants` 与全部 `glassLight*` / `glassDark*` 前缀键。

### 步骤

1. 生成器输出 `pub struct Palette { pub primary: u32, … }` 与六个常量
   （`OPAQUE_DARK`、`GLASS_DARK`、`GLASS_LIGHT` 及其 `_CONTRAST`），以及
   `fixed` 常量模块；CSS 继续输出 `opaqueDark` 作为 `:root` 默认值。
2. **等价迁移**：`contrast` 的初始值由当前 `with_contrast` 的实际结果计算得出，而不是
   沿用已漂移的旧 `variants`。先写一个临时测试，把旧 `theme.rs` 每个角色函数在
   12 种组合（3 表面 × 对比度开关 × 生成前后）下的输出固化，迁移后逐项比对相等，
   通过后删除临时测试、保留新结构的不变量测试。
3. `theme.rs` 改为 `fn palette() -> &'static Palette`（按 `glass_tone`、
   `uses_opaque_fallback`、`uses_increase_contrast` 选择），角色函数只读字段；删除
   `with_contrast`、`glass_color`、`adaptive_surface` 及写死的 alpha。
4. `scripts/tests/test_design_tokens.py` 增加：角色集合一致、`extends` 无环且单层、
   `fixed` 与调色板不重名、浅色调色板正文对比度不低于 4.5:1、语义图标不低于
   3:1（按 `ui-design.md` 现有要求，以代表底色计算）。
5. 等价迁移合入后，再单独提交需要的视觉调整（例如真正设计过的对比度方案）。

完成标准：仓库中不再出现 `variants`、`GLASS_LIGHT_*` 常量与 `with_contrast`；
`generate-design-tokens.py --check` 通过；R0 后的界面像素级不变（以等价测试证明）。
文档：更新 [ui-design.md](../../ui-design.md) 的 Tokens 章节与
[development.md](../../development.md) 的生成器说明。

## R3 Web 主题变量改为生成

### 现状

`apps/gpui/src/web_tool.rs` 的 `web_theme_script(light_surface)` 为浅 / 深色各手写
约 40 行变量映射；只接受一个布尔值，因此增强对比度和减少透明度下 Web 页面与
GPUI 页面不一致，新增 token 时也容易漏改。

### 步骤

1. 生成器为 `Palette` 生成 `fn web_variables(&self) -> [(&'static str, u32); N]`，
   变量名沿用 `css_name(role)`，与现有 `--aw-*` 名称一致。
2. `web_theme_script(palette: &Palette, color_scheme)` 只拼接脚本；调用点
   （`launcher/view/mod.rs` 中 Web 主题同步处）改为传入 `theme::palette()`。
3. 确认辅助功能通知（增强对比度、减少透明度切换）也会触发 Web 主题重推，而不仅是
   `observe_window_appearance`；缺失则补上，并保持“更新变量不重载文档”。
4. 检查 `resources/web-controls.css` 的 `@media (prefers-contrast: more)` 规则与新注入
   的对比度调色板是否叠加过度，必要时只保留其中一处。
5. 测试：现有 `web_tool.rs` 主题测试扩展为六个调色板，断言每个调色板角色都出现在
   脚本中；旧文档句柄隔离测试保持不变。

完成标准：`web_tool.rs` 不再引用任何颜色常量名；JSON Workspace 与 Text Pad 在
四种外观组合下桌面检查通过（物理验收单独记录）。

## R4 键盘分发分层

### 现状

`apps/gpui/src/launcher/view/input.rs:39` 的 `handle_key_with_repeat` 763 行、
认知复杂度 107，由约 30 个顺序 `if`（重复键过滤、快捷键录制、各类弹层、用户快捷键、
预览、根导航、`⌘K` / `⌘R` / `⌘F` / `⌘S` / `⌘Q` / `⌘↵`、Shelf `⌘⇧⌫`、应用历史专属键、
修饰键过滤）和最后的 `match (screen, key)` 组成。仓库没有使用 GPUI 的 `actions!`、
`KeyBinding`、`key_context` 或 `on_action`；`tests/mod.rs` 中约 83 处测试直接驱动
键盘。

### 阶段一：分层（行为不变）

1. **先补特征测试**：对已知冲突写表驱动用例，固化当前优先级，例如：预览打开时的
   Esc / Space、弹层打开时的 `⌘K`、录制快捷键时的任意组合、用户快捷键与内置
   `⌘R` 同键、内容模式下的 Enter、IME 组合中的 Space。重构前这些测试必须通过。
2. 引入 `KeyInput { key, modifiers, shortcut, repeated }` 与
   `enum KeyFlow { Handled, Continue }`。
3. 按现有顺序拆为固定的处理阶段，每个阶段一个函数，放到对应控制器文件：
   `filter_repeat` → `recording` → `overlays`（各页面弹层）→ `user_shortcuts` →
   `preview` → `root_navigation` → `global_commands` → `screen_commands`（按
   `Screen` 分派到 `controllers/*`）→ `navigation`（方向键 / Enter / Tab）。
   `handle_key_with_repeat` 只按顺序调用这些阶段，第一个返回 `Handled` 即停止。
4. 不调整任何键的语义或顺序；如发现现有顺序有缺陷，记录后另起提交修复。

完成标准：入口函数不超过 60 行，单阶段不超过 120 行；全部键盘测试通过。

### 阶段二：固定命令迁移到 GPUI action

1. 用 `actions!` 定义固定命令（打开设置、对象动作、刷新、文件搜索、保存设置、退出、
   强制执行、Shelf 删除等），以 `key_context` 区分 `Launcher`、`FileSearch`、
   `Clipboard`、`ClipboardShelf`、`Settings`、`Tasks`、`WebTool`。
2. 页面根元素用 `on_action` 接入；阶段一中对应的 `global_commands` 分支删除。
3. **保留在手写分发中的**：用户自定义快捷键（运行时动态）、方向键 / Enter / Tab
   导航、预览 Space、快捷键录制。原因是它们依赖选择状态和 IME
   （`prefer_character_input`），且 GPUI Kit 的 `Input` 在自身 context 中绑定了
   方向键与 Enter，迁移会产生派发顺序冲突。
4. 验证：用 GPUI 测试的按键模拟走完整 keymap 派发，确认输入框获得焦点时
   `⌘K` / `⌘,` 仍生效、普通字符不被吞掉；物理中文输入法检查单独记录。
5. 可选：从绑定表生成设置页的快捷键说明，避免与文档重复维护。

文档：更新 [launcher-interaction.md](../../launcher-interaction.md) 的按键优先级说明。

## R5 `LauncherView` 状态拆分

### 现状

`LauncherView` 104 个字段，`build`（`launcher/view/mod.rs:589`）705 行。滚动句柄
有两套：`tool_list_scrolls.*` 与其基础句柄的别名字段 `file_search_scroll`、
`clipboard_scroll`、`clipboard_shelf_scroll`、`history_scroll`、`tasks_scroll`。

### 拆分方案

采用普通结构体而非新的 `Entity`，避免跨实体通知与借用改动；方法随字段迁移，
只读写自身字段的方法移到子结构体上。

| 子结构体 | 迁入字段（示例） |
|---|---|
| `SettingsEditor` | `settings`、`settings_draft`、四个设置输入框、`history_settings_*`、`settings_action_picker_*`、`settings_close_pending`、`settings_exit_intent`、六个确认框焦点句柄、`clipboard_settings_choice`、`settings_save_generation`、`settings_scrolls`、`settings_focus_handle` |
| `ShortcutEditor` | 四个快捷键输入框、`shortcut_recording`、`pending_action_shortcut*`、`context_shortcut_draft`、`shortcut_catalog*`、`action_shortcut_resolving` |
| `PreviewState` | `preview`、`preview_busy`、`preview_generation`、`preview_request_generation`、`preview_open`、`preview_selection_task` |
| `WindowPresence` | `presented`、`retained_workspace`、`hidden_at`、`retention_task`、`resize_pending` |
| `ScrollHandles` | `scope_scroll`、`notice_scroll`、`file_filter_scroll`、`result_scroll`、`tool_list_scrolls`、`web_packages_scroll`、`unsupported_tool_scroll` |
| `IconLoader` | `application_icons`、`application_icons_requested`、`application_icon_generation` |
| `TasksPageModel`（已有） | 迁入 `pending_task_selection`、`tasks_output_scroll`、`tasks_refresh_cancel` |

`clipboard_confirmation`、`pending_shortcut_execution` 等跨页字段按实际调用点确认
归属，不确定时留在 `LauncherView`。

### 步骤

1. 删除五个滚动别名字段，改为 `ScrollHandles::base(&UniformListScrollHandle)`
   访问器；同步修改测试中的 `view.file_search_scroll` 等引用。
2. 逐个子结构体迁移，每迁一个编译并跑 GPUI 测试；每个子结构体提供 `new(window, cx)`
   构造，`build` 只组合子结构体并注册订阅。
3. 订阅注册拆为 `build_subscriptions`，焦点句柄创建并入各自子结构体。

完成标准：`LauncherView` 不超过 50 个字段，`build` 不超过 150 行，无别名字段；
GPUI 测试全部通过。文档：更新 [architecture.md](../../architecture.md) 的 GPUI
实现地图。

## R6 通用列表行与行规格

### 现状

| 列表 | 行高 | 上下内边距 |
|---|---|---|
| Launcher / 文件搜索 | 56 | 2 |
| 剪贴板历史、应用历史 | 64 | 3 |
| Clipboard Shelf | 48 | 1 |
| Tasks（卡片） | 86 | 8 |

图标尺寸散落在 12–30 px；GPUI 非测试代码约 500 处数字像素字面量，集中在
`pages/tasks.rs`（92）、`settings/shortcuts.rs`（67）、`pages/clipboard_shelf.rs`（56）。
R0 中“统一透明边框防跳动、边框补偿内边距、标题与图标对齐”的规则目前只在
`candidate_row.rs` 生效。

### 步骤

1. Token 新增行规格：`rowHeight`（56）、`rowHeightComfortable`（64）、
   `rowHeightCompact`（48）、`rowInsetYRegular` / `Comfortable` / `Compact`
   （2 / 3 / 1）、`rowIconSlot`（32）、`rowIconSize`（24）、`rowIconGap`（16）、
   `rowPaddingX`（10）。初始值等于现状，本步不改变行高。
2. 新增 `apps/gpui/src/launcher/components/list_row.rs`：
   `ListRow::new(id).density(..).leading(..).title(..).subtitle(..).trailing(..)`
   `.selected(..).enabled(..)`，统一处理 `ListBoxOption` 角色与 aria、固定高度、
   `result_surface`、边框补偿、标题 / 副标题字号与截断、tooltip、尾部不可收缩；
   返回元素供页面继续挂事件。
3. 依次迁移 `candidate_row.rs`、`pages/clipboard.rs`、`pages/application_history.rs`、
   `pages/clipboard_shelf.rs`、`pages/web_packages.rs`、`settings/shortcuts.rs` 的
   列表行。Tasks 卡片保留自身布局，仅复用表面与文字样式。
4. 测试：把 R0 的对齐测试泛化到每个列表——选中与未选中时标题 x 坐标相同、标题 /
   副标题在行内、尾部操作不被覆盖、行高等于对应 token。
5. `scripts/check-code-style.py` 增加像素字面量棘轮：记录各文件当前 `px(<数字>)`
   计数于 `scripts/px-literal-budget.json`，只允许减少；本 PR 将三个热点文件
   降到 20 以下。

完成标准：所有列表行经由 `ListRow`；预算文件生效。是否统一不同列表的行高属于
视觉决策，另行确认后只改 token。文档：更新 [ui-design.md](../../ui-design.md)
的列表规则。

## R7 拆分超长函数

超过 200 行的函数：`pages/tasks.rs:30`（768）、`view/input.rs:39`（763，R4 处理）、
`view/mod.rs:589`（705，R5 处理）、`settings/shortcuts.rs:4`（516）、
`settings.rs:16`（480）、`app.rs:235`（476）、`pages/clipboard_shelf.rs:4`（371）、
`settings/shortcuts.rs:601`（364）、`pages/web_packages.rs:5`（362）、
`settings/application_history.rs:4`（337）、`pages/application_history.rs:4`（326）、
`view/mod.rs:2420`（325）、`pages/file_search.rs:4`（275）、
`view/candidate_row.rs:4`（254）、`pages/clipboard.rs:4`（202），以及 Core 的
`filesystem/scan/traversal.rs:16`（216）、`application/snapshot.rs:49`（204）。

步骤：按页面区块（页头、工具栏、列表、空态、弹层、页脚）拆为私有函数；`app.rs`
的 `run` 拆为主题 / 窗口 / 状态栏 / 热键 / 独立窗口注册五段。完成后新增
`clippy.toml` 的 `too-many-lines-threshold = 200`，在 `check.sh` 与
`check-gpui.sh` 的 Clippy 调用中启用 `-W clippy::too_many_lines`（配合
`-D warnings` 生效）。

完成标准：全工作区没有超过 200 行的生产函数；新阈值进入门禁。

## R8 Release 构建配置

### 现状

仓库没有 `[profile.release]`；`scripts/build-gpui.sh` 执行默认
`cargo build --release`。`build/Anywhere.app/Contents/MacOS/Anywhere` 为 45.6 MB，
其中 `__TEXT` 32.0 MB、`__LINKEDIT` 11.7 MB；没有生成 dSYM。

### 对照实验

| 方案 | 配置 |
|---|---|
| A | 现状 |
| B | `lto = "thin"`、`codegen-units = 1` |
| C | B + `strip = "symbols"`、`debug = "line-tables-only"`、`split-debuginfo = "packed"` |
| D | C + `opt-level = "s"` |

测量：二进制体积、clean release 构建时间、冷启动到首帧时间（同机重复 10 次取
中位数）、`scripts/bench-file-search.py` 与现有 `examples/measure.rs` 的搜索延迟、
崩溃符号化（用 dSYM 对人为 panic 的回溯做 `atos` 还原）。

### 约定

- 保持 `panic = "unwind"`：Core 依赖 `catch_unwind` 处理中毒与隔离。
- strip 发生在签名之前；dSYM 作为构建产物保存在 `build/` 并由 CI 上传，不放进
  `.app`。
- 选择体积与延迟都不劣化的方案；D 只有在延迟不劣化时采用。

完成标准：实验数据写入 [performance.md](../../performance.md)；采用的配置进入
`Cargo.toml`；`build-macos.sh`、`package-macos.sh` 与 CI 签名流程通过。

## R9 候选行共享字符串

现状：`launcher/view/mod.rs:2492` 每次渲染执行 `self.model.rows.clone()`，
`render_row` 再复制标题、副标题、快捷键；文件搜索的 `uniform_list` 每行同样
复制；`pages/web_packages.rs:22` 整表复制。

步骤：`LauncherRow` 的 `id`、`title`、`subtitle`、`shortcut`、`source_label` 改为
`SharedString`；首页改为按下标渲染；Web 包列表先提取渲染所需字段再遍历。用
`apps/gpui/examples/projection_measure.rs` 对比改前改后的分配次数与字节数，结果
写入 [performance.md](../../performance.md)。

## R10 拼音匹配预计算（测量门槛）

现状：`core/search/pinyin.rs` 在无字面命中时，对每个字符、每个读音、每个起点
扫描，并对含 `ü` 的读音分配 `replace('ü', "v")`。

门槛：在 `crates/anywhere-core/examples/measure.rs` 增加 2 000 个中文名称 × 常见
拼音查询的测量；单次按键的匹配耗时 p95 超过 1 ms，或分配显著时才实施。

方案：`SearchFields` 对非 ASCII 名称字段按需（`OnceLock`）预计算每个字符的
字面值、全拼、`v` 变体与首字母，`matches_token` 只读预计算结果；设置单字段
字符数上限以约束内存。现有拼音测试全部保留。

## R11 缩短锁持有时间（测量门槛）

现状：扫描报出 64 处可提前释放的守卫，集中在 `features/clipboard/service.rs`（9）、
`application/feature_catalog.rs`（6）、`core/execution/mod.rs`（5）。剪贴板服务
在同一把状态锁内执行 SQLite 读写（`record_payload`、`search`、`usage`）。

门槛：用 tracing span 记录持锁超过 5 ms 的次数，在大剪贴板库（数千条、含图片）
和连续复制场景下测量；只有确实阻塞 UI 请求才实施。

方案：剪贴板仓库单独加锁，状态锁只保护 epoch、策略和缓存；确定并在代码注释中
写明锁顺序（状态 → 仓库），用 epoch 校验替代长时间持有状态锁。其余位置按
Clippy 提示逐处缩短作用域，不改变语义。

## R12 工程清理

- 删除 19 处多余 `.clone()`（如 `app.rs:115`、`candidate_row.rs:66`、
  `execution_request.rs:121`、`filesystem/scan/scope.rs:203`）。
- 39 处按值传入但未消费的参数改为借用（Clippy `needless_pass_by_value`）。
- `core/search/scope.rs:102` 的 `sort_by_key` + `position().unwrap()` 改为先建立
  位置表，去掉 `unwrap` 和重复线性查找。
- 这些 lint 属于 pedantic / nursery，不加入门禁，只做一次性清理。

## 总体完成标准

- `scripts/check-gpui.sh` 与 `scripts/check.sh` 通过，测试数不少于基线（941）。
- 新增的架构规则、token 不变量、像素预算、函数长度阈值都进入门禁。
- 外观、键盘、Web 主题、列表的桌面验收按 [GPUI 验收](../../mvp-gpui.md#物理桌验矩阵)
  单独记录；自动测试通过不等于物理验收通过。
- 相关专题指南（`architecture.md`、`ui-design.md`、`launcher-interaction.md`、
  `development.md`、`performance.md`）只保留一份现行说明。

## 附录 A：R1 锁调用点

行号对应基线 `3df3fe1` 加 R0 修改。

**撤销 / 清理，改为 `lock_for_revoke`：**

- `application/launcher_service.rs:206`：保留期到期的 Web 失效
- `application/launcher_service.rs:353`：`set_capture_sources` 闭包中的 Web 失效
- `application/context_capture.rs:112`：来源开关更新与撤销闭包
- `application/launcher_service.rs:263`：`replace_registry`（见 R1 步骤 3）
- `application/web_plugins/background_action.rs:40`：`Drop` 关闭文档
- `features/clipboard/service.rs:271`：维护任务停止

**纯缓存，改为 `lock_cache`：**

- `application/session_runtime.rs:79`、`:86`：目录 memo、offer 缓存
- `application/snapshot/cache.rs:39`、`:82`
- `application/feature_catalog.rs:64`
- `application/session_runtime/publication.rs:120`：快捷键目录
- `features/app_actions/catalog.rs:49`、`:57`、`:83`、`:102`
- `features/filesystem/search.rs:345`、`:350`：投影缓存

**授权 / 发布，保持拒绝并补日志：**

- `application/context_capture.rs:119`、`:333`
- `application/context_capture/clipboard.rs:109`
- `application/file_workspace.rs:220`、`:411`
- `application/web_plugins/rpc_dispatch.rs:98`、`:275`（已返回 `HostFailure`）
- `application/application_index.rs:167`
- `features/filesystem/search.rs:166`、`features/policy.rs:56`（`is_ok_and` 即拒绝）

**服务状态，`unwrap` / `expect` 改为 `lock_state`：**

- `features/application_history/service.rs:100`、`:123`、`:152`、`:187`、`:197`、`:249`、`:280`、`:351`
- `features/application_history/service/query.rs:44`、`:57`、`:88`、`:140`、`:232`
- `features/application_history/service/execution.rs:32`
- `features/filesystem/search.rs:120`、`:142`、`:228`
- `features/filesystem/scan/pagination.rs:50`、`features/filesystem/scan/traversal.rs:208`
- `application/application_index.rs:127`、`:150`
- `application/feature_catalog.rs:120`、`application/native_features.rs:126`

其余 77 处已转为错误返回，10 处已恢复守卫，保持不变；实施时以
`check_lock_usage` 规则与本清单交叉核对。

## 实施记录（2026-11-18）

本方案全部项目在分支 `codex/docs-v0.14-usage` 完成实施与验证（均未执行 `git commit`）：

- **R0 / R1**：完成候选行与选中态 UI 对齐；新增 `crates/anywhere-core/src/core/sync.rs` 统一 `lock_or_deny` / `lock_for_revoke` / `lock_cache` / `lock_state`，补齐各撤销与中毒回归测试，并在 `scripts/architecture_rules.py` 启用 `check_lock_usage` 门禁。
- **R2 / R3**：完成 Token schema v3（`OPAQUE_DARK`、`GLASS_DARK`、`GLASS_LIGHT` 及其 `_CONTRAST` 与 `fixed`）与 `Palette::web_variables()` 生成，移除旧 `variants`、`GLASS_LIGHT_*` 与 `with_contrast`，辅助功能外观变化同步刷新 Web 主题且不重载文档。
- **R4（阶段一 + 阶段二）**：`apps/gpui/src/launcher/view/input.rs` 分层重构并补齐优先级特征测试；`apps/gpui/src/launcher/view/actions.rs` 使用 `gpui_kit::actions!`、`KeyBinding` 与 `key_context`（`Anywhere` / `Launcher` / `FileSearch` / `Clipboard` / `ClipboardShelf` / `Tasks` / `Settings`）承接全部固定命令，`surface_focus_handle` 保证非输入页派发路径完整，动态快捷键、方向导航、Preview Space 与录制留在手写预处理层。
- **R5 / R6 / R7 / R9**：`LauncherView` 拆分为 `SettingsEditor`、`ShortcutEditor`、`PreviewState`、`WindowPresence`、`ScrollHandles`、`IconLoader`（`apps/gpui/src/launcher/view/state.rs`）及 `TasksPageModel`，移除滚动别名字段；提取通用 `ListRow`（`apps/gpui/src/launcher/components/list_row.rs`）并落地 `scripts/px-literal-budget.json` 像素字面量棘轮；全工作区超 200 行函数全部拆分，启用 `clippy.toml` `too-many-lines-threshold = 200` 与 `-W clippy::too_many_lines`；`LauncherRow` 改用 `SharedString` 并按可见下标渲染。
- **R8 / R10 / R11 / R12**：完成 Release Profile A–D 对照实验，在 `Cargo.toml` 采用方案 C（`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, `debug = "line-tables-only"`, `split-debuginfo = "packed"`, `panic = "unwind"`），主二进制体积由 43.91 MiB 降至 29.49 MiB，`scripts/build-gpui.sh` 与 CI 保留 `.app.dSYM` 并验证 `atos` 符号化通过；R10 2,000 条中文名拼音测量 p95 均 `< 1.0 ms`，按门槛不触发预计算；R11 测量剪贴板 2 MiB 连续写入竞争超过 5 ms 门槛，已完成 `state` / `repository` 双锁拆分并将并发 `status()` 降至亚微秒级（0/22 > 5 ms）；R12 修复 `core/search/scope.rs` 位置映射查找，并完成全仓 `redundant_clone` 零警告及安全 `needless_pass_by_value` 清理。
- **自动化门禁与物理验收边界**：`cargo fmt --all`、`scripts/check-gpui.sh`、`scripts/check.sh`、`python3 scripts/check-docs.py` 与 `git diff --check` 全部通过（仅反映自动化验证）；外观对比度、真实中文 IME、Quick Look 硬件 Space、WebView 跨主题与窗口交互仍按 `docs/mvp-gpui.md` 物理桌验矩阵保留独立实机验收步骤。
