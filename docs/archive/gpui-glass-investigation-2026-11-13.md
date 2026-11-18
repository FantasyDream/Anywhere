# GPUI Liquid Glass 根背景排查（2026-11-13）

## 结论与范围

已复现的阻挡原因是产品未清空 GPUI Kit 的组件 Root 背景，不是 GPUI / Metal
不能承载原生 Liquid Glass。透明页面、非 opaque 的 NSWindow / CAMetalLayer
均不足以证明没有绘制纯色背景。Root 仍可画一个覆盖整个窗口的不透明 quad。

修复位于 `apps/gpui/src/app.rs` 的 `configure_component_theme()`：同时设置
`theme.colors.background` 和 `theme.tokens.background` 为透明。真正影响当前
Root 绘制的是 token 的 `background`，只改代表色或 LauncherView 均不够。

本次确认了真实 GPUI + 原生 glass 在同一窗口的彩色背景上可以正常透色。
整窗桌面采样、各页文字对比度和辅助功能矩阵尚未完成验收。

## 环境与源码证据

- 系统报告 macOS 27.2.1 / build 26B131，SDK 27.2，arm64。
- gpui-kit / gpui-component 0.9.1，GPUI Fast 0.1.14。
- `gpui-component/src/root.rs` 的 `WindowState::style()` 对 Root surface 调用
  `.bg(cx.theme().tokens.background)`；它包在产品页面外层。
- GPUI macOS backend 的 `set_background_appearance(Transparent)` 会关闭 Metal
  opaque 标志；MetalRenderer 的清屏 alpha 也相应为 0。这些标志并不阻止后续
  Root quad 写入 alpha 1 的像素。
- 产品此前仅设置了窗口 Transparent、透明 window token 和 LauncherView，
  `configure_component_theme()` 没有设置组件 Root background。

Apple 规定 glass 的内容由 `contentView` 承载，不保证任意 sibling 的合成顺序。
这解决内容承载方式；它不自动把 GPUI Root 画出的不透明 quad 变透明。
[contentView 契约](https://developer.apple.com/documentation/appkit/nsglasseffectview/contentview)

## 已运行的对照

| 对照 | 唯一关键变化 | 观察 |
| --- | --- | --- |
| 纯 AppKit regular / clear | 两种公开 glass style，彩色条纹位于同窗后方 | 均显示玻璃；regular 模糊更强，clear 有明显边缘折射 |
| AppKit clear + Metal | contentView 内加入持续清屏 alpha 0 的 CAMetalLayer | 彩色条纹与玻璃高光保留 |
| clear + Metal + clipping | 再开启 layer 的 18 px 圆角和 masksToBounds | 仍显示玻璃 |
| 真实 GPUI + glass，Root 默认 | 页面透明、窗口 Transparent、同窗原生彩色条纹 | 默认浅色 Root 覆盖条纹，白色文字也被白底淹没 |
| 同一 GPUI probe，Root 透明 | 仅将 `theme.tokens.background` 设置为透明 | 条纹透出，GPUI 文字可见，玻璃高光和边缘保留 |
| 纯 AppKit 整窗 Regular | 同一纯原生整窗实验，仅 Clear 改为 Regular | 原生灰色对比表面与白色文字恢复；证明样式影响可读性，与 GPUI 无关 |
| 产品 +81 | 透明 Root + Regular 原生材质 | 黑色底消失，原生灰色对比表面、白色文字与圆角显示正常；桌面采样仍需独立确认 |
| 产品 +80 | Root colors / token 均透明，其余材质路径保持 | 原有黑色整窗底色消失；独立窗口截图呈透明合成的浅底，浅色文字对比度仍需真实桌面复核 |

前四项原生控制使用临时独立 AppKit fixture，未启动 Rust Core 或读取用户数据。
真实 GPUI 对照使用仓库 example。观察来自实际运行后的 CUA 窗口截图。
整窗纯 AppKit 玻璃也做了控制，但它的独立窗口截图呈浅底，因此不能据此宣称
桌面背景采样成功或失败，更不能归因于 GPUI。

## 可复现 GPUI 对照

仅 macOS 26+，使用项目已锁定依赖，不启动 Anywhere 的业务 runtime：

```sh
cargo run -p anywhere-gpui --example native_glass_probe --locked
GLASS_PROBE_CLEAR_ROOT=1 cargo run -p anywhere-gpui --example native_glass_probe --locked
```

分别关闭上一窗口后再运行下一项，避免把同名窗口混淆。默认分支刻意保留组件
Root 的背景作为负对照；带环境变量的分支才是透明版本。彩色条纹是同窗原生
底图，不能用这项测试替代桌面采样、输入法或实际 Launcher 页面验收。

## 修复与回归

- 全局组件 Root 始终透明；Reduce Transparency 的不透明背景继续由产品页面
  的 fallback surface 绘制。局部卡片、输入控件和选中态保留自己的背景。
- 主题初始化的第二次更新设置 Root token，防止加载 Dark 主题时恢复默认纯色。
- 原有主题测试检查 color 与完整 ThemeToken；新增回归实际渲染组件 Root，
  检查 `painted_quads()`，禁止透明内容外仍出现有填充色的 Root quad。
- 整窗 Launcher 使用 Regular style，由原生材质提供文字对比；Clear 在同窗彩条
  上能显示折射，但在整窗浅底对照中使白字失去对比。不能用 GPUI 纯色遮罩补偿。
- 原生材质仍由 AppKit 拥有。此次修复不需要更换 GPUI、渲染器或引入私有 API。

后续应在 Root 已透明的前提下对照原 Flutter：分别检查亮 / 暗 / 彩色桌面，
比较 Clear / Regular 的文本可读性与折射；若整窗桌面效果不足，应继续定位原生
窗口材质与采样方式，而不是给 GPUI 再铺一层深色底色掩盖问题。

## 四角亮边的后续对照

用户反馈 Spotlight 和原 Flutter 没有 GPUI 的明显白角。此前将该现象全部解释为
系统 Glass 高光并不充分，以下单变量实验发现窗口样式本身会增加边缘亮度。
本次只调查原因，没有修改产品窗口样式或替换正在运行的产品。

源码差异：

- 当前 GPUI Launcher 使用 `Titled | FullSizeContentView | NonactivatingPanel`，
  隐藏标题和交通灯并未移除 Titled。Apple 的
  [窗口样式说明](https://developer.apple.com/documentation/appkit/nswindow/stylemask-swift.struct/fullsizecontentview)
  区分了内容占满窗口与去掉窗口外围元素。
- Flutter 工作树 `Anywhere-flutter-3-50-memory` 的 `Main.swift` 使用
  `LauncherWindow(.borderless)`，背景 clear，自定义类允许成为 key/main window。
  `WindowMaterial.swift` 在独立外层 NSView 上裁切，然后承载 Regular Glass。
  本机 `launcher.log` 的 2026-11-13T02:05:10Z 启动记录为 `backend=liquidGlass`，
  因而不能将差异归因为 Flutter 只使用旧的 HUD blur。该日志不证明实验时旧实例
  仍在运行，也不代替同状态的 Flutter 截图。
- 锁定依赖 `gpui-fast-macos 0.1.14/src/window.rs` 的 `build_window_class()`
  已为 GPUIWindow 和 GPUIPanel 注册 `canBecomeKeyWindow=yes`、
  `canBecomeMainWindow=yes`，不能套用普通 Borderless NSWindow 的默认焦点限制。

独立原生实验位于 `/tmp/anywhere-corner-probe/`，只使用 AppKit，不加载 GPUI、
Flutter 或 Core，不读写产品数据。648×100 pt panel 使用 Regular Glass、无 tint、
18 pt 圆角、近透明窗口背景；通过原生按钮切换 frame style、外层 host 和 layer
mask。CUA 确认各状态，并保存 1296×200 px 窗口截图。截图无 alpha，已合成在
白底上，因此数据描述截图边缘，不能推导全桌面采样或 GPU 图层透明度。

| 状态 | 左上圆角切点 `(36, 0)` 的 sRGB |
| --- | --- |
| Titled，Glass 直接作为 contentView，mask 开 | 205 / 205 / 205 |
| Titled，外层 host 裁切，mask 开 | 205 / 205 / 205 |
| Borderless，Glass 直接作为 contentView，mask 开 | 163 / 163 / 163 |
| Borderless，外层 host 裁切，mask 开 | 163 / 163 / 163 |
| Borderless，外层 host，mask 关 | 165 / 165 / 165 |

此处是固定位置像素样本，不是整圈平均亮度。原生截图仍保留细微材质亮边；
实验支持 Titled 窗口合成增加边缘亮度，尚不证明生产窗口四角异常全部来自同一
原因。改变外层 host 与 mask 未在该点产生同等幅度差异，暂不应先改裁切或加深
背景。Spotlight 私有窗口实现未获得验证，不能从参考截图推断它具体使用了哪个
类或效果参数。

建议下一步对真实 GPUI Launcher 做同窗口 A/B：仅切换为 Borderless +
NonactivatingPanel，保留原生 Glass、透明 Root 和现有圆角；检查白底四角、
首次呼出 key focus、中文 IME、失焦隐藏、动态高度，以及进出 Settings 后的恢复。
Settings / Tasks 的正常标题栏另行保留。通过物理对照后再决定是否调整外层 host
与裁切；不以 tint、自绘深色边框或背景蒙层遮盖窗框问题。
