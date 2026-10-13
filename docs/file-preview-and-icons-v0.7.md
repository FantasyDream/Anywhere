# 文件预览调研与类型图标

日期：2026-10-13。当前更新包：V0.7 build 22。

当前预览已改为固定右侧面板，没有 Preview Action / 开关 / 关闭入口，详见 [稳定顺序与固定预览](files-fixed-preview-build22.md)。下面调研与 build 17 的交互记录作为历史背景保留。

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

## 3. Anywhere 的借鉴方案

### 保留当前导航契约

```text
输入查询 → ↑ / ↓ 选文件
             ├─ Enter / Open：打开
             └─ ⌘K / Ctrl K → Preview：右侧预览
                                 ├─ ↑ / ↓：跟随新选择
                                 └─ Esc：关闭预览，保留查询和选择
```

现有右侧预览已能跟随选择，120 ms 防抖，迟到请求隔离。采用详情与内容相邻的结构，避免预览成为第三套导航。根据此前交互约束，继续用二级 Action 入口，不新增裸 Space / Shift / Tab 操作；空格仍参与文件名输入，类型循环保持 Ctrl Tab。

### build 17：右侧预览交互收尾

- 预览通过二级 Actions 的 Preview 打开；右侧面板跟随键盘 ↑ / ↓ 与鼠标选中的候选，Enter 仍只负责打开文件。
- 从二级 Actions 返回文件列表时，按当前激活 / Revision / 查询 / 类型保存并恢复列表位置；新查询、换类型或新激活不会套用旧位置。异步恢复时再次校验这些条件。
- Esc 或关闭按钮只关闭面板，保留查询、文本选区、类型、选中文件与列表位置。预览文字获得焦点后，一次 Esc 也关闭面板并把焦点交回输入框。
- 换选中文件后，预览内容滚动位置从顶部开始；关闭会清除当前预览，迟到读取结果不会使已关闭面板复活，重新打开重新读取当前文件。
- 仍采用 120 ms 防抖与请求序号隔离，预览打开时冻结结果顺序；仅显示当前对象，不增加逐文件内容缓存。

### 后续内容能力建议，尚未实现

1. 右侧详情始终保持文件名 / 类型 / 大小 / 时间 / 位置，内容区域作为可选能力。
2. 第一阶段优先图片的有界缩略图、文本 / 代码；第二阶段再增加 PDF 首页 / 页切换。只加载当前选中对象，关闭 / 换查询释放，保持结果身份和 Revision 校验。
3. Office、音视频及其他复杂格式可另提供宿主系统预览动作；macOS 可采用 Quick Look，Windows 单独实现。宿主不支持时保留元数据与 Open，避免为了预览强制整个软件依赖一个平台。
4. 普通图片 / PDF 预览和类型图标是不同工作：图标不触发内容读取或解码，避免每条候选隐式增加 I/O、缩略图缓存和窗口负担。

当前仍是 64 KiB 有界文本预览 + 元数据；没有在本轮加入图片 / PDF / Office 渲染或系统 Quick Look。

## 4. 本轮已实现：文件类型图标

新增 `ui/file_type_appearance.dart`，仅根据候选 basename 和目录标记提供外观提示；不影响 Rust 搜索匹配 / 类型过滤 / 执行目标，不依赖原生图标查询。

| 类型 | 图标与配色 |
|---|---|
| 文件夹 | 蓝色线性文件夹，目录优先于名称后缀 |
| PDF | 淡红文档 |
| 文档 / 纯文本 | 蓝色 / 中性灰文本页 |
| 表格 / 演示 | 淡绿表格 / 杏色图表 |
| 图片 / 音频 / 视频 | 淡蓝图片 / 淡粉音符 / 淡紫胶片 |
| 压缩包 | 沙金归档盒，支持 tar.gz 等后缀 |
| 代码 / 配置 | 淡紫代码符号 / 灰蓝配置符号 |
| 电子书 / 字体 | 书本 / 字形 |
| 安装包 / 二进制 | 中性包盒 |
| 未知格式 | 原通用文件图标 |

常量映射复用已有 Cupertino 图标字体，透明底，无单独磁盘读取、图片解码或每文件位图缓存。列表与右侧预览共用外观与类型名称，大小写不敏感；隐藏配置、无扩展构建文件和未知格式有明确回退。类型名称为文件名提示，不是 MIME / 正文检测结果。

实机发现二级动作也使用 File candidate kind；已给结果行显式区分文件对象与文件操作，防止 Copy / Reveal / Terminal 图标被文件后缀外观覆盖。动作执行 / 图标仍表达其用途。

## 5. 验证

- Flutter analyze 与全部 176 项 Flutter 测试通过；最终操作图标修复后的 29 项图标 / 文件页回归通过，包含二级 Copy Path 图标断言。
- 测试验证名为 `照片.PDF` 的目录仍显示文件夹；PDF 文件与其预览共用外观；未知格式有回退；保留二级动作图标。
- 16 种示例用真实 CandidateRow 离屏渲染，输出 `build/ui-v0.7-build16/file-type-icons.png`，已检查形状、色彩与透明背景。这是字体 / 布局检查，不冒充原生玻璃验收。
- 本机启动 build 16，显式目录 fixture 返回 16 项，AX 类型标签与可见线性图标匹配；目录 `Folder.pdf` 仍显示文件夹。
- 最终修复后重新打包并启动：`main.rs` 列表 / 右侧预览均显示 Source code 外观与类型；二级 Actions 的 Copy / Reveal 仍为复制 / 文件夹图标，预览显示正确路径与文本，Esc 分层返回正常。
- fixture 内容为图标验证文本，即使名称为 PDF / HEIC 等也不是真实对应格式；不用于证明这些格式可渲染。

更新包为 `build/Anywhere-dev-0.7-build16.app` 与对应 arm64 ZIP / SHA-256。保留原背景、Impeller、搜索引擎和快捷键规则，本轮未提交或推送。

deep / strict 签名通过，与 build 15 使用相同 designated requirement；ZIP 的 37 个普通文件哈希与 8 个符号链接均与 App 一致。

## 6. build 17 验证

新增长列表回归：60 项结果，选择第 36 项后经 Actions 打开右侧预览，检查列表偏移；滚动预览再切换候选，检查新内容和顶部位置；焦点移到预览文字后 Esc 一次关闭，确认查询、选区、候选与列表偏移不变；关闭按钮后继续输入。另一项回归控制未完成的读取，关闭后再放行旧结果，验证不会复活且重新打开加载当前内容。

最终 Flutter analyze 无问题，全部 178 项 Flutter 测试通过；图标 / 文件页合计 31 项回归通过。新包 build 17 已启动实机验证：显式目录 48 个文本文件，长列表中选择 preview-12，经 Actions 打开右侧预览，列表位置保留；连续 ↓ 更新到 preview-14，路径与正文一致；Esc 关闭保留原查询和选中项；重新打开并点击关闭按钮，随后直接追加查询可得到对应唯一文件。测试验证预览文字获焦后的一次 Esc、文本选区保持和迟到结果隔离；未把这些 widget 检查冒充全部物理快捷键验收。

生成 `build/Anywhere-dev-0.7-build17.app`、对应 arm64 ZIP 与 SHA-256；deep / strict 签名有效，与 build 16 designated requirement 相同，ZIP 普通文件和符号链接与 App 一致，Impeller 启用。本轮 Rust / 原生功能代码未改，未重复运行 Rust 功能测试；没有新增图片、PDF、Office 内容渲染。
