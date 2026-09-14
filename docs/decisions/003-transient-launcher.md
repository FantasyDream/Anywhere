# 003 — 短暂唤醒、输入独占与紧凑窗口

用户的第一轮手工验收修改了交互约定：失焦应立即关闭，有输入时不再混入 Finder 动作，空结果不保留列表空间。旧的跨应用保留窗口流程和 X08/X09 不再适用。

最小修改：

- AppKit 负责可见性和焦点：窗口失去 key 或应用失活，立即 orderOut，并向 Flutter 发送带窗口 generation 的 dismiss 通知。主动打开浏览器/文件同样会失焦。
- Flutter 的 dismiss/隐藏先使本地 generation 失效、清空状态，再请求 Core deactivate。过期输入、执行和尺寸回调不得重新显示窗口或关闭下一次唤醒；执行已开始的 OS 动作不回滚。
- 每次唤醒创建新 Session、捕获新前台上下文。有非空输入时，Application Facade 仅用 UserInput 来源 Context 匹配动作；清空输入恢复本 Session 的环境快照。Core 保留完整 Context Graph 和 source_app 历史键，Matcher/Ranking 共用，不在 Dart 隐藏业务动作。
- Flutter 决定展示所需高度，Host 仅限制并应用窗口尺寸。无动作时只保留搜索栏；每行 52pt，最多可见 7 行，额外动作滚动，窗口总高度最多 520pt。尺寸变化保持窗口顶边位置。
- 外观参考 [Raycast Compact Mode](https://www.raycast.com/blog/a-fresh-look-and-feel) 与 [Spotlight](https://support.apple.com/guide/mac-help/search-with-spotlight-mchlp1008/mac)：中性深色、细边框、紧凑行、弱化辅助文本和键帽。没有添加搜索应用、AI 等新能力。

兼容性：Rust 公共 DTO 和插件协议不变；窗口私有 MethodChannel 增加 dismiss、resize 和 generation。手工测试 B/E/H 的环境动作预期、X08/X09 的 Session 流程及失焦后文件失效测试一起更新。

后续外观修订：NSWindow 和 FlutterViewController 的背景分别设为透明，在现有 Flutter 视图后添加 NSVisualEffectView（hudWindow / behindWindow / darkAqua）。原生毛玻璃使用可拉伸的圆角 mask，内容保留同样的 12pt 裁切；Flutter 仅绘制半透明底色、边框与控件，避免实色遮挡系统背景。保留原控制器和输入响应链，不引入毛玻璃插件。窗口变高后刷新阴影。外观由用户按 K01/K12/K14 验证。
