# 文件预览调研

调研日期：2026-10-12；整理日期：2026-10-13。以下是当时的公开资料与未实现方案，没有在本轮重新核验外部产品。

当前 Anywhere 是固定右侧元数据 / 文本预览，没有 Preview Action 或关闭开关；历史 build 17 的切换方案已删除。当前契约见 [文件搜索](../file-search.md)。

## 1. 其他产品的操作模型

| 产品 | 预览 / 详情 | 键盘操作 | 可借鉴点 |
|---|---|---|---|
| Raycast | Search Files 有文件元数据详情面板，另外提供 Quick Look 动作 | Enter 打开，⌘K / Ctrl K 进入动作面板，⌘Y / Ctrl Y 切换 Quick Look | 把快速确认内容与打开文件分开，详情 / 大预览分层 |
| Alfred | 文件导航右半部分显示预览；Actions panel 也可显示富预览 | Shift 触发更大的 Quick Look，Return 在默认应用打开 | 预览就近呈现，确认文件后仍用原打开动作 |
| Spotlight | 结果上直接进入 Quick Look | 空格预览；Return / 双击打开 | 预览是选中对象的辅助动作，打开保持独立 |

依据：[Raycast File Search](https://manual.raycast.com/file-search)、[Raycast Keyboard Shortcuts](https://manual.raycast.com/keyboard-shortcuts)、[Alfred Previews](https://www.alfredapp.com/help/features/previews/)、[Apple Spotlight](https://support.apple.com/en-gb/guide/mac-help/mchlp1008/mac)。这些是公开行为 / 接口，不是逆向确认它们的内部渲染器。

Raycast Windows 的官方说明明确 Quick Look 为浮动预览窗口，切换搜索结果时跟随选择；支持内容范围由宿主实现。[官方变更说明](https://www.raycast.com/changelog/windows/0-50)

## 2. 公开的实现接口

- Raycast 扩展 API 为 List.Item 提供 `quickLook` 路径 / 名称，`Action.ToggleQuickLook` 由宿主切换预览。插件描述目标，宿主负责显示，值得借鉴这种能力边界。[API](https://developers.raycast.com/api-reference/user-interface/actions#action.togglequicklook)
- macOS 官方提供 [QLPreviewView](https://developer.apple.com/documentation/quicklookui/qlpreviewview) 与 [QLPreviewPanel](https://developer.apple.com/documentation/quicklookui/qlpreviewpanel)：前者可作为预览视图，后者是独立面板，通过数据源提供预览对象。存在这些系统接口，不代表已确认 Raycast / Alfred 私有内部使用了哪个具体类。
- Quick Look 是平台能力。若以后使用，应放在 macOS Host / 平台适配层；不能把 AppKit 类型、系统面板生命周期直接加入 Rust 搜索 / Context 主流程，也不能据此认为 Windows 已有等价实现。

## 后续方案（尚未实现）

1. 右侧详情始终保持文件名 / 类型 / 大小 / 时间 / 位置，内容区域作为可选能力。
2. 第一阶段优先图片的有界缩略图、文本 / 代码；第二阶段再增加 PDF 首页 / 页切换。只加载当前选中对象，关闭 / 换查询释放，保持结果身份和 Revision 校验。
3. Office、音视频及其他复杂格式可另提供宿主系统预览动作；macOS 可采用 Quick Look，Windows 单独实现。宿主不支持时保留元数据与 Open，避免为了预览强制整个软件依赖一个平台。
4. 普通图片 / PDF 预览和类型图标是不同工作：图标不触发内容读取或解码，避免每条候选隐式增加 I/O、缩略图缓存和窗口负担。

当前仍是 64 KiB 有界文本预览 + 元数据；没有在本轮加入图片 / PDF / Office 渲染或系统 Quick Look。
