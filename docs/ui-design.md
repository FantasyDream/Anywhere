# 共享 UI 与原生材质

当前实现：V0.8；最后核对：2026-10-13。页面行为以 [Launcher 交互](launcher-interaction.md) 为准，本页只说明视觉组件与平台边界。

## 组件与布局

- 所有页面使用 `ui/AppStyle`、公共图标、按钮、keycap 和设置组件；领域模块只提供内容及动作。
- Launcher 使用显著输入区、紧凑 Scope 标签、结果行和留白底栏。选中态为中性的透明覆盖 / 边界，保留文字层级，不给每个图标套大底板。
- 文件页扩展相同结构，类型标签复用 Scope 风格；右侧固定预览。文件类型外观来自 `ui/file_type_appearance.dart`，不影响 Rust 分类或执行身份。
- Settings 使用紧凑侧栏、统一行背景和桌面开关，标题 / 描述 / 状态 / 控件保持同一行体系；错误和权限引导就近呈现。草稿、保存、取消由 SettingsController / 页面管理。
- Running Tasks 使用同一色板、紧凑列表与输出详情；窗口持久，支持关闭、最小化和调整大小。
- 快捷键和返回沿统一导航，不为文件工具增加裸 Space、Tab 或另一套预览模式。

不固定要求所有页面相同密度。输入、结果、可操作控件和状态需要可辨认；透明效果不能让内容与桌面混为一体。大字体、窄屏、长路径通过布局与溢出回归验证。

## 背景与可读性

Flutter 负责内容和控件，原生材质在其内容背后。Host `backdrop` 能力明确可用时允许透明底色；否则用不透明回退。共享页面不依赖 AppKit Widget。

搜索 / 二级 Actions / 模式选择器保持同一圆角、边界、内边距与文字层级。弹层需在实际窗口范围内渲染，不能把首屏小高度当作足够空间而裁掉菜单；模式选择以 Launcher 内导航层呈现。

浅色桌面下通过前景文字、局部覆盖和边界保持阅读对比。颜色、按钮尺寸和 keycap 的变更集中在公共样式，不由 Feature 各自绘制。

## macOS 实现

`WindowMaterial` 统一管理内容容器。源码在 compiler >= 6.2 且 macOS 26+ 的路径使用 `NSGlassEffectView.contentView`；其他路径使用 `NSVisualEffectView`，保留 macOS 12 的回退。

Flutter 内容是材质容器内部的真实子视图，不能只把玻璃作为同级背景然后假定获得相同效果。`WindowInputFocus` 维护内容 view / responder，窗口重父级、重复唤醒后重新连接输入；背景本身不接管业务键盘。

这说明当前代码如何使用系统 API，不代表未来 SDK、每种系统版本或所有辅助功能设置均已实机验收。没有把不再适用的“尚未接入玻璃”旧提案保留为现状。

## 生命周期与平台适配

Launcher / Settings 共享 Flutter engine；Running Tasks engine 按需创建。窗口尺寸来自 Host 展示状态，通过 DesktopHost 传 width / height，Rust 不决定像素。

其他平台可实现自己的材质，缺失能力仍渲染共享主题；Windows 窗口与输入仍需独立接入。材质、缩放、大字体、IME、重复唤醒、键盘焦点和 Reduce Transparency / 读屏分别验收，不以离屏截图替代原生测试。

文件预览的产品 / Quick Look 接口研究保存在 [预览调研](research/file-preview.md)，未接入的富预览与系统面板不作为现有 UI 能力。
