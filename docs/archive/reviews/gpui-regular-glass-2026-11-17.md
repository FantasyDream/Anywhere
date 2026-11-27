# GPUI Regular 玻璃与无背景蒙版

日期：2026-11-17。按本次提供的 macOS 系统通知截图调整当前 GPUI 源码。
使用系统公开的 Regular 材质作为实现，不推测 Notification Center 的私有结构，
不把单元测试或材质安装日志写成已达到逐像素一致。

## 修改

- Launcher 从 `SizeInvariant` / `NSGlassEffectViewStyle::Clear` 切到 Regular。
  Settings、Tasks、Shelf 同样为 Regular；移除可选择 Clear 的分支。
- 原生 `tintColor=nil`、appearance 跟随系统，只有一个 AppKit 玻璃容器。
  保留原圆角裁切与系统阴影，不叠加色罩、描边、渐变或第二层模糊。
- 所有 GPUI 产品页直接使用透明 window 背景，不再区分首页 contentScrim 与
  工具页 pageScrim。兼容 token 两者均为透明，Web 生成投影同步更新。
- 搜索头与普通结果行透明，移除原约70%深色 / 浅色补色卡片；选中 / hover
  使用普通低 alpha 高亮。Increase Contrast 不再把透明背景或结果行升为高
  alpha 蒙版，浅色材质也继续使用对应深色文字。
- 输入框、按钮、菜单和确认框仍有自身控件表面；确认框打开时的模态交互遮挡
  保留，以防点击穿透。该遮挡不用于修饰普通玻璃背景。
- macOS15–25 仍用 HUD；Reduce Transparency 仍按系统偏好回退不透明表面，
  不强行绕过辅助功能设置。Regular 随高度和背后内容变化的浓度保留为系统行为。

入口：[macOS 材质](../../../apps/gpui/src/platform/material/macos.rs)、
[窗口策略](../../../apps/gpui/src/platform/mod.rs)、
[页面背景](../../../apps/gpui/src/launcher/view/mod.rs)、
[主题](../../../apps/gpui/src/ui/theme.rs)、
[token 源](../../../resources/design-tokens.json)。生成文件通过
`python3 scripts/generate-design-tokens.py` 更新，没有直接修改生成投影。
现行契约见 [UI 设计](../../ui-design.md#材质与可读性)。

## 自动化与证据边界

修改前先更新已有断言并确认失败：普通工具页仍为约44%背景 alpha，Launcher
材质策略仍返回 SizeInvariant。生产修改后，两个断言和完整 GPUI 门禁通过，
GPUI 共244项。最终 `scripts/check.sh` 通过：工作区 Rust 为928 passed /
6 ignored / 0 failed；严格Clippy、格式、生成物、SDK / 示例、原生菜单探针与
架构门禁通过。文档校验为109个Markdown / 871条本地链接，`git diff --check`
通过。验收清单保留原322个唯一编号，本次更新UI-005的Regular预期。

这次没有启动 App、没有 CUA 或物理桌验，也没有重新打包；此前包与验收截图
不作为本次 Regular 材质的物理证据。当前可承诺的是材质 / 透明策略实现已改，
系统通知对照、文字可读性和真实绘制结果仍为 Pending。

## 人工验收

使用当前源码重新构建的指定包与隔离业务数据，记录 App摘要 / 签名 / 版本、
macOS、显示器 / 缩放、系统外观与辅助显示选项。完整流程沿用
[测试清单](../../test-acceptance-macos.md)；以下是本次必须重验的视觉序列。

| 场景 | 操作与预期 | 状态 |
| --- | --- | --- |
| 通知同背景对照 | macOS26+同屏、同外观、同壁纸 / 背后窗口打开系统通知和当前Launcher；比较材质浓度、折射、边缘与阴影，不能使用旧包或不同背景作结论 | Pending |
| 空态 / 展开 | 72px空态、单项、5/7项与长查询来回切；背景保持原生Regular，不出现额外矩形蒙版；允许系统自适应浓度，顶部锚点和键盘保持正确 | Pending |
| 全页遍历 | Launcher、File Search、History、Shelf、Application History、Tasks、Settings、Web、Detail逐页检查；搜索头和普通结果行无补色底，选中 / hover清晰 | Pending |
| 浅深 / 背景交叉 | 浅深系统外观、黑 / 白 / 彩色背景分别对照通知；检查标题、说明、图标、输入框和选中态，记录局部可读性失败 | Pending |
| 辅助显示 | Increase Contrast不增加整页 / 行蒙版，不错误切浅色字色；Reduce Transparency用明确不透明回退；Reduce Motion不破坏布局 | Pending |
| Web | Text Pad / JSON Workspace背景露出同一原生材质，无白底或第二层模糊；主题变化不重载、不丢正文 / IME / 选择 | Pending |
| 模态与控件 | 打开删除 / 清空 / 设置关闭确认，检查菜单与局部输入控件；模态仍阻止点击穿透，关闭恢复焦点；普通页面不残留遮挡 | Pending |
| 窗口生命周期 | 主窗 / Shelf / Tasks / Settings进出、切应用、多屏、缩放、15s恢复和退出；单一材质不嵌套，旧面板不残留，阴影和圆角正确 | Pending |

实机结果需单独写Actual / Pass / Fail与截图或录像。若与通知仍有可见差别，先
区分系统版本 / 外观 / 辅助设置、背景、窗口尺寸及GPUI文本渲染差异，不再加入
背景蒙版模拟匹配。
