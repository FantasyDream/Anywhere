# GPUI 修复复核与玻璃透明度方案（2026-11-16）

复核提交：`b428899c9ce70c92e12de24950405222ea3b6c28`，基线
`75dee3ebb18a0e4e8381785e8dc3eb73da7ec01a`，分支 `codex/experiment-gpui-kit`。
开始时工作区干净。本轮检查该提交的 22 个文件，完成 5 项临时 GPUI 诊断。
临时源码已清理，产品实现、运行中的 App 和用户系统设置未修改。

结论：确认 3 类新增 / 回归问题和 1 类模态修复残留。现有修复并非整体失效；
常规模态遮挡、Scope 居中、Notice 分区和长正文滚动已有实质改善。

## 1. Review 发现

### R1 · P2 · 正常模式的全窗遮罩使玻璃接近实色

位置：[design-tokens.json](../../../resources/design-tokens.json) 的 `contentScrim`，
[theme.rs](../../../apps/gpui/src/ui/theme.rs) 的 `content_scrim`，以及
[LauncherView](../../../apps/gpui/src/launcher/view/mod.rs) 根容器背景。

默认值从 `#a0303438` 改为 `#ed303438`。Root token 仍透明，原生 Glass 仍为
Regular / nil tint；退化来自覆盖原生材质的内容层，且影响全部产品页面。

| 正常模式 | 内容层 alpha | 下层原生材质的混合权重 |
| --- | --- | --- |
| 原值 A0 | 160 / 255 ≈ 62.7% | 95 / 255 ≈ 37.3% |
| 当前 ED | 237 / 255 ≈ 92.9% | 18 / 255 ≈ 7.1% |

下层材质的混合权重只剩原来的约 19%，足以解释用户反馈。
**7.1% 是留给原生材质的合成权重，不是整个窗口对桌面的实际透射率。**
Glass 自身还有采样、着色、折射和高光。

上次记录的页头 RGB 差从 21 降到 6，表明背景变化被压低；降低混合权重本来就会
产生这种结果，不能据此证明布局或 Metal 呈现时序的撕裂已解决。
建议按第 2 节恢复正常模式通透度，并修正文档中“7% 桌面透射”的说法。

### R2 · P2 · 原生 Esc 绕过新增的模态取消与焦点恢复

位置：`view/mod.rs::handle_settings_escape`、
[navigation.rs](../../../apps/gpui/src/launcher/view/navigation.rs) 的
`handle_settings_escape_from_platform`，以及
[platform/mod.rs](../../../apps/gpui/src/platform/mod.rs) 的 Settings Esc 监视器。

GPUI 新拦截器知道如何取消 Clipboard 确认；原生本地监视器先吞掉设置窗口的 Esc，
再经 `PlatformEvent::SettingsEscape` 调用旧的 `handle_key` 路径。旧处理器没有
Clipboard 确认分支，也没有调用新的焦点恢复 helper。

诊断结果：

- Clear 确认中发送 GPUI Esc：取消确认，仍为 Settings。
- 同一状态走生产原生入口：离开整个设置页，变为 Launcher。
- 脏设置关闭提示经原生 Esc 取消：提示消失，但原设置焦点未恢复，返回焦点仍被
  保存，焦点仍指向已卸载的模态按钮。

这是逻辑入口的可复现差异；源码已追踪到真实原生监视器，但本轮没有重新做
AppKit 物理键盘验收。

方案：取消按钮、GPUI Esc、原生 Esc 共用一个模态优先入口，先取消当前确认并
恢复焦点，再处理绑定选择和设置页退出。回归直接覆盖原生入口，不只发送 GPUI 按键。

### R3 · P2 · 快捷键录制仍可越过关闭确认框

位置：窗口按键拦截器中，`shortcut_recording` 分支位于 `settings_modal_open`
之前；`present_settings_close_prompt` 没有结束录制。

通过实际控件点击复现：设置有未保存修改 → 点击 Current App 快捷键录制 →
点击 Done 打开关闭确认 → 按 Control-J。确认仍打开，底层输入却从
`meta+alt+Space` 改成 `control+KeyJ`。

这属于模态修复遗漏的组合状态，不代表鼠标遮罩修复没有生效。
方案：让模态路由优先于录制器；打开确认时取消或暂停录制，不能修改其背后的草稿。
与 R2 共用入口，验证 Settings / Clipboard 确认和各种录制目标的组合。

### R4 · P2 · 小屏约束与尺寸收敛使用不同目标

位置：`view/mod.rs::sync_launcher_window_after_render` / `resize_current_window`，
以及 `platform/mod.rs::launcher_frame` / `resize_launcher_window_preserving_top`。

渲染端比较未经约束的 `desired_window_size`；原生端之后才按屏幕裁剪。
原生“尺寸相同则跳过”的判断也位于裁剪之前。

800×600 可用区域保留 16 点边距后，文件页实际只能高 568 点，渲染端仍要求
600 点。诊断重复两轮裁剪 viewport 通知，每轮都再次请求 648×600，
`window_size_changed(648×568, 648×600)` 始终为 true。

因此再次 render 就会排入无法达到的原始尺寸。输入、加载或光标刷新可能反复触发
无效原生布局；本轮未测量实际 CPU / GPU 开销，不宣称已经证明持续满载。

方案：区分自然内容尺寸与当前屏幕上的有效尺寸。比较、排队、原生应用及测试断言
统一使用有效尺寸；原生端计算最终 frame 后再判等。同时复查跨屏定位只改原点、
Tasks / Shelf 固定最小尺寸与可用区域的关系。

验收：大屏、小屏与跨屏下反复 render；达到有效尺寸后不再请求 resize。
加入有顺序控制的原生请求 / 回执模拟，使用真实 viewport 测量。

## 2. 推荐的玻璃方案

### 2.1 恢复通透基线，保留原生材质

维持当前一个原生材质 host、一个 GPUI 呈现栈和既有 `contentView` 层级。
按本轮用户确认，**正常模式的 `contentScrim` 改为 `#00303438`，即 ARGB alpha
为 0**；优先通过第 2.3 节的文字配色改善可读性，局部控件表面作为回退。
旧 A0 只作为历史对照，不再作为本方案的恢复目标。这里是待实施决策，当前产品
token 仍为 ED。

| 情况 | 候选处理 |
| --- | --- |
| 正常 Liquid Glass / HUD | `contentScrim = #00303438`，Root color 和 token 保持透明 |
| Increase Contrast | 独立保留较高覆盖率，例如当前 F2，遵循用户系统选择 |
| Reduce Transparency | 沿用现有不透明 fallback |
| 对象菜单、确认框 | 保留局部不透明 `menuSurface`，阻止底层文字透入 |

alpha 为 0 只撤去 GPUI 的全窗内容遮罩，不撤去原生 Regular Glass / HUD 的模糊、
折射和高光，不等于完全透明窗口。背景纹理、明暗和高度变化带来的材质适应会更
明显，固定浅色文字也更容易丢失对比度。背景绘制透明不改变现有命中或焦点路由；
确认框的 `modalScrim` 另有职责，不能跟着清空，R2 / R3 仍需独立修复。

修改 `resources/design-tokens.json` 后运行
`python3 scripts/generate-design-tokens.py`，同步 Rust 和 SDK CSS；不手改生成物。
透明整窗与局部可读性保护一起验收，不能只改 alpha 就判定完成。

### 2.2 优先调整语义文字颜色，保留局部表面回退

维持 Regular Glass 和 nil tint，由系统负责原生轮廓与材质适应。需要提高对比度
时，优先按材质外观及控件实际表面选择深色 / 浅色文字，配合文字层级和焦点
边界；正常整窗背景保持 alpha 为 0。若某个控件 / 行仅调整字色仍不能满足
可读性，再调整其局部表面。现有局部表面也含半透明值，不能默认它们在白底上
已足够，也不能仅凭背景 token 的 RGB 判定合成后一定是深色。

Apple 说明 Liquid Glass 会按尺寸和环境自适应；Regular 适用范围较广，Clear
更透明但缺少同样的适应行为，并要求特定内容和可读性条件。因此这次反馈无需
直接更换为 Clear。参考：[Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/)。

Apple 也建议审查会覆盖或干扰 Liquid Glass 的自定义背景。
参考：[Adopting Liquid Glass](https://developer.apple.com/documentation/TechnologyOverviews/adopting-liquid-glass)。

### 2.3 按表面选择文字颜色

按本轮用户决定，**通过调整字体颜色改善可读性，取消阴影 / 描边原型和相关
字形绘制扩展。** 普通文本与输入继续使用现有 GPUI 组件的颜色接口。

当前缺口已从源码确认：`platform/material.rs` 已保存 `WindowAppearance`，
macOS adapter 读取应用 / 窗口的 `effectiveAppearance`；但 `ui/theme.rs` 的
Primary / Secondary / Muted 仍返回固定浅色值，`app.rs` 又将 GPUI Kit 固定为
Dark。因此原生玻璃和 GPUI 文字没有使用同一套明暗决策。放进原生材质的 Metal
文字也不会自动变成 AppKit 的语义文字。

首版以现有材质外观作为玻璃区字色的选择信号，同时尊重控件自己的表面：

| 表面 | 文字策略 |
| --- | --- |
| 普通浅色 Glass / HUD | 正文用深色，说明和提示用较浅的深灰，均保持不透明 |
| 普通深色 Glass / HUD | 正文用接近白色，说明和提示用浅灰，避免额外降低 alpha |
| 现有不透明深色菜单、确认框、控件 | 保持深色表面对应的浅色字，不随根玻璃改为深色字 |
| Increase Contrast 的 F2 深色 scrim、Reduce Transparency 的深色 fallback | 使用深色表面字色，提高次要文字对比，不仅依据系统 Light / Dark |
| Accent / 选中 / 搜索高亮等局部表面 | 按实际背景单独选择前景，和普通正文语义分开 |

这符合 Apple 对自定义颜色提供明暗与高对比度变体的建议；其原生控件能按内容
适应的行为，不能直接推定为 GPUI Metal 文字已有的能力。
参考：[Apple Color](https://developer.apple.com/design/human-interface-guidelines/color)。

第一轮文字候选如下，均为 RGB 展示值，写入 token 时使用不透明 ARGB：

| 角色 | 浅色表面字色 | 深色表面字色 |
| --- | --- | --- |
| Primary / 正文、输入 | `#20242b` | `#ffffff` |
| Secondary / 说明 | `#3c434d` | `#f0f2f5` |
| Muted / 提示、placeholder | `#505965` | `#e1e5eb` |

按 WCAG sRGB 公式计算，浅色候选在 `#e0e0e0` 代表背景上的对比分别为
11.80 / 7.57 / 5.38:1；深色候选在 `#606060` 代表背景上分别为
6.29 / 5.61 / 4.97:1。这些是候选色的静态计算，**不是实际 Glass 的验收结果**。
正式值按合成后的材质背景调整，普通文字、说明和 placeholder 沿用 4.5:1 目标，
11–13 px 文字不能按大字的 3:1 放宽。参考：
[Contrast Minimum](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html)。

系统明暗外观不等于桌面或字下背景的实际亮度；同一窗口也可能横跨明暗内容。
单一固定字色总能遇到相近亮度的背景，改为中灰或统一加亮不能保证所有背景。
本方案明确是改善可读性：先按表面选择字色，仍失败的局部区域使用第 2.2 节
回退，正常整窗保持透明。现有外观信号不代表已经支持随桌面内容实时自动反色。

实施落点：

1. 在 `resources/design-tokens.json` 增加按浅 / 深表面区分的语义文字色，并生成
   Rust / SDK CSS。当前生成器只接受 `appearance: dark`，直接改成 `light` 会
   失败；可在既有 colors 结构增加明确角色，保留已有 SDK token 名称和默认
   含义，不为这次调整另建主题框架。
2. `ui/theme.rs` 统一选择当前玻璃区字色；为不透明表面、Accent 背景和选中文字
   保留各自的前景语义。逐个核对 `theme::primary()` 等调用者，不能把所有颜色
   一起切换后让深色菜单出现深色字。Warning / Danger / Success 等文字也要
   选择对应表面的可读变体，不只处理三个灰阶角色。
3. 启动时先准备材质环境，再配置 GPUI Kit 的颜色；当前代码顺序相反，不能读取
   尚未初始化的默认 Light 决定真实字色。当前产品只显式接入辅助功能显示通知，
   需通过 GPUI 已有的 `observe_window_appearance` 补上明暗外观订阅，再共用
   材质刷新与字色更新路径，同步产品文字与 Kit 的 foreground、placeholder、
   按钮文字、选区和光标颜色。不仅改根容器的 `text_color`，也要更新有显式颜色
   的输入和组件；主窗口及 Shelf / Tasks 的现存与新建窗口都要获得一致配色。
4. 当前 `platform_appearance_changed` 主要更新 Web canvas 和页面通知，Web
   tokens 也是静态注入。对当前有效文档同步语义 CSS 变量，更新必须绑定 Rust
   文档句柄，旧文档回调不能影响新文档；切换字色不通过重载文档丢弃输入。
   私有插件控件的固定配色单独验收，不能宣称共享 CSS 覆盖所有内容。

现有颜色接口足以承载这项修改，无需新增字形绘制路径；仍需验证主题切换不会
重置查询、IME 组词、选区、滚动位置或设置草稿，以及长列表和持续输入下没有
换色闪烁。颜色计算与小字号实际栅格化清晰度分别验收。

### 2.4 分开验收真实撕裂、明暗自适应和可读性

保留窗口 / 列表布局直接切换及无动画原生事务。先修复 R4 的重复尺寸请求，检查
旧请求失效、viewport 更新及新帧呈现的时序。以“没有旧画面拉伸、文字重叠、裁切
和过期尺寸回写”为硬要求；材质明暗变化单独记录，不通过接近实色的遮罩压平 RGB 差。

若同尺寸结果替换仍撕裂，分别 A/B `GPUI_VIEW_RETENTION=0` 和
`GPUI_SCROLL_LAYERS=0` 定位缓存路径；诊断开关不直接作为发行配置。

实施顺序：

1. 统一 R2 / R3 的模态入口和回归，修复 R4 的有效尺寸判等。
2. 将正常模式 `contentScrim` 设为 alpha 0 并生成投影，同时补足必要的局部表面，
   按第 2.3 节接入语义字色选择，保留辅助功能和局部弹层保护。
3. 同一源码、背景、位置下测试空态 → 7 条结果 → 清空，再测试两组等数量结果。
   分别记录布局、材质亮度与可读性。
4. 在 macOS 26+ Glass 与 macOS 15–25 HUD、系统明暗模式下，分别验收白底、暗底、
   高亮彩色、黑白花纹、背景滚动及系统辅助功能。普通文字保持 4.5:1、语义图标
   3:1；加入 Light 外观配深色桌面、Dark 外观配白底的交叉组合。先调整对应角色
   字色，不足时调整控件 / 行的局部表面后重测，不提高正常整窗 alpha。
5. 覆盖 Launcher、Settings、Tasks、Shelf、原生工具和受控 Web 内容；检查中英文
   小字、Secondary / Muted、placeholder、选中 / 未选中、搜索高亮、禁用状态、
   IME 组词、选区、光标、焦点环与 1× / 2×。单独检查密集中文、Emoji、换行、
   截断和滚动裁切，确认固定深色表面始终保留可读字色。
6. 窗口打开 / 隐藏、设置模态和 Web 文档打开时切换明暗外观与辅助功能。自动回归
   覆盖材质与表面不一致的配色决策、首次初始化顺序及旧文档更新失效；物理输入、
   IME 和材质对比度由维护者安排验收，保持 Pending，不自动启动 App。
7. 连续帧必须包含实际变化的 UI 状态。先确认录屏不是静态重复帧，再判断是否撕裂；
   端点截图和一次原子 resize 日志保留各自证据边界。

方案优先恢复通透度并消除错误布局更新，不另建 UI 或动画框架。本轮尚未实施该方案。

## 3. 验证与证据

- 5 项临时诊断完成：录制中的确认框、两种 Esc 入口、原生取消后的焦点、裁剪尺寸
  重复请求、长无空格输入。诊断观测当前行为，不代表发现的问题已修复。
- 负向对照：2,048 个连续无空格字符能换行，文字高 432 点，纵向滚动范围为
  219.5 点，未复现新的裁切缺陷；不继续将该场景按旧 UI07 报为未修复。
- Clear 只打开合成状态的确认框，没有接入 live runtime 或删除个人历史。
- 证据目录：`build/gpui-ui-followup-review-20261116-b7hgvf_2/`，保留临时源码、
  观测补丁、提交信息和输出，不随 Git 交付。
- 查看了之前保留的 v167 端点截图；本轮没有新增原生桌面录屏或透明度实机测量。
- 清理诊断源码后，`scripts/check-gpui.sh` 通过，含原有 200 项 GPUI 测试和严格 Clippy；
  `scripts/check.sh` 完整通过，含 workspace 测试、生成物、文档、原生、SDK / 示例及
  架构边界检查。该结果不消除上述未被原有回归覆盖的组合状态问题。
- 本次文档补充后重新检查通过：102 个 Markdown、698 个本地链接；
  `git diff --check` 通过。
- 后续透明遮罩与文字配色方案仅补充文档、当前调用路径核对及代表色计算；未修改
  产品 token、未实现明暗字色切换、未新增实机或性能通过结论。上述完整代码门禁对应
  原复核源码，不充当透明方案的验收结果。

## 4. 后续实施记录

本节记录复核之后的工作区实施，不改变上文对 `b428899` 的原始观察。

- R1：正常模式 `contentScrim` 已改为 `#00303438`；Increase Contrast 仍使用
  `#f2...`，Reduce Transparency 仍走不透明 fallback，`modalScrim` 和固定
  深色菜单表面保持独立。
- R2 / R3：关闭确认、Clipboard 确认和快捷键录制现由模态优先路由统一处理。
  按钮、GPUI Esc、原生 `SettingsEscape` 先取消确认并恢复焦点；打开关闭确认时
  终止录制，组合键不能修改背后草稿。回归覆盖原生入口和 stale recorder。
- R4：Launcher 自然尺寸先按当前显示器 `visible_bounds` 与 16 px 边距裁剪；
  render 比较、延迟排队和原生 frame 判等使用一致目标。原生在计算最终受限
  frame 后比较 origin / size，尺寸相同时只更新屏幕定位。
- 玻璃配色：新增浅 / 深玻璃语义色、状态色、交互表面和固定 `onDark*` 角色。
  启动先准备材质再安装 GPUI Kit 主题；窗口外观和辅助功能通知刷新主窗口、
  Settings、Tasks、Shelf 与 Web 文档。Web CSS 变量绑定当前 Rust 文档句柄，
  不重载页面。

自动回归包括 206 项 GPUI 单元测试、生成物 / Clippy / SDK / 示例与仓库检查。
当前透明候选尚未完成 macOS 26+ Glass、15–25 HUD、真实桌面明暗交叉、辅助功能、
IME、1× / 2× 和连续帧录屏验收；这些结论保持 Pending。
