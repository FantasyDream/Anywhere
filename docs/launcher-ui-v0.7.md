# Launcher UI 与统一交互模型

日期：2026-10-10。当前实现：0.7.0+13。本文替代 build 9 的类型菜单、Tab 控件遍历与预览开关交互。Context 输入的既有契约继续有效。

## 核心决定

**一个 Launcher，两个主入口，工具通过普通 Action 打开。**

```text
Global Search：搜索应用、Actions，以及输入对应的已有 Context 能力
Current App：根据当前环境提供可用 Actions，Scope 过滤这些 Actions
       │
       └─ 普通 Action → 工具二级页面 → 返回原查询和位置
                           ├─ File Search
                           └─ 后续工具沿用同一模式
```

文件搜索不再常驻空白首页，不增加 Files Scope，也不成为第三个根入口。输入 `files`、`File Search` 或 `文件搜索`，选中并按 Return；或在 Settings → Shortcuts → Action shortcuts 中给 File Search 设置用户自己的快捷键。开启 Global 后，可在窗口关闭时直接进入文件页。设置显示友好的工具标签，内部 Launcher capability 不提供为新 Context 快捷键条件。普通 Action 的身份、快捷键、执行校验和 Usage 继续复用。

## 设计参考与取舍

| 参考 | 官方资料支持的行为 | Anywhere 的选择 |
|---|---|---|
| [Raycast Action Panel](https://manual.raycast.com/action-panel) | Return 执行主动作，⌘K / Ctrl K 展示更多动作，Esc 返回 | 固定选择 / 执行 / 返回手势；已有次级操作按需出现 |
| [Raycast Command Hotkeys](https://manual.raycast.com/command-aliases-and-hotkeys) | 命令支持用户自定义热键 | 文件搜索用已有 Action 快捷键设置，不强制占用新默认热键 |
| [Spotlight 筛选](https://support.apple.com/guide/mac-help/narrow-search-results-in-spotlight-mchl4d69efd3/mac) | 通过类别与类型缩小搜索范围 | 保留清晰搜索层级；文件页内部用与 Scope 相同的类型标签栏 |
| [Alfred File Search](https://www.alfredapp.com/help/features/file-search/) | 文件搜索与导航可通过关键词进入 | 工具入口可搜索；不为每个功能增加常驻按钮 |

这是对资料的产品取舍，不表示这些软件采用完全相同的入口结构。Anywhere 的主要特色继续是 Context-aware Actions。

## 整体功能审视

| 能力 | 位置与职责 | UI 约束 |
|---|---|---|
| Global Search | 找应用、动作和输入能力 | 空白时只有搜索栏；匹配后出现结果与执行按钮 |
| Current App | 使用当前环境里的 Context Actions | 保留既有 Scope 过滤；菜单仍在 Current App 内 |
| Selected text / Recent clipboard | 独立输入 Context | 显示来源和内容；15 秒有效期与原 Esc 链不变 |
| Folder / Git / 项目等 Scope | 缩小可用动作集合 | 只作过滤，不增加第二套工具导航入口 |
| Menu navigation | 浏览应用菜单路径 | 返回 / Enter / 左右键沿用原主窗口规则 |
| File Search | 普通 Action 打开的二级工具 | 不驻留首页；页面复用主搜索、结果、底栏和玻璃背景 |
| 文件操作 | 所选文件的真实操作 | 仅按 Actions / ⌘K 展开；退出保留查询与选择 |
| 输出结果 | 当前动作的结果 | 复用结果区，Copy / Back 按需出现 |
| Settings | 持久配置与草稿编辑 | 保留独立设置窗口，不混入搜索结果列表 |
| Running Tasks | 长任务生命周期与输出 | 保留任务窗口，不强行当作临时搜索页 |

主入口的唤醒热键不变。搜索图标的紧凑菜单提供 Global Search / Current App 切换，保留当前查询；选择当前模式也不会关闭窗口。空白 Global Search 隐藏 Scope 标签行，有查询时既有 Scope 过滤逻辑仍保留。没有新增 Context、Subject 或 Action 分组概念。

## 页面结构与视觉规则

```text
搜索 / 返回   [同一个输入框，工具或内容来源提示]             Esc
──────────────────────────────────────────────────────────────
已有 Scope 标签 / 文件类型标签栏（同一组件）
结果标题（仅菜单、捕获内容、操作选择与输出需要）
结果列表                                    │ 按需预览
──────────────────────────────────────────────────────────────
结果数量                                  Actions   Open / Run ↵
```

- 搜索字号 21 logical px，捕获内容 17；标题 15，辅助说明 11.5，行高 56。工具页与主页共享组件。
- 原玻璃背景、透明选中层、细描边、语义图标与共享色板保留；不加入大色块或独立页面装饰。
- 文件过滤为与 Scope 共用标签栏组件：All types、Files、Folders、Documents、PDF、Images、Audio、Video、Archives、Code。选中样式、间距和透明度与 Scope 完全相同；Ctrl Tab 切换到超出窗口的标签时自动滚动。主窗口玻璃背景不变。预览放在所选文件的二级 Actions 中，不再常驻开关。
- 文件操作列表与输出页面不再显示无效的文件过滤 / 预览控制。
- 底栏删除常驻方向键说明；主操作旁保留 Return 键帽，选择提示放在 Tooltip 中。没有可执行项时隐藏主按钮。
- 一个固定 Key 的 TextField，跨模式复用 EditableText 与 TextInput 连接，不在工具切换时换掉输入控件。

## 快捷键与返回规则

| 操作 | 行为 |
|---|---|
| ↑ / ↓（搜索框聚焦） | 选择当前列表的结果 |
| Tab / Shift Tab | 全程序取消裸 Tab 焦点遍历；Ctrl Tab 组合保留，用户定义的组合键按既有规则处理 |
| Return / 双击 / 底栏主按钮 | 执行或进入选中的结果 |
| 单击结果 | 立即选中，不执行；菜单文件夹继续单击导航 |
| ⌘K / Ctrl K | 文件页展开已有文件操作；Current App 空查询沿用菜单浏览 |
| Ctrl Tab / Ctrl Shift Tab | 主窗口切换 Scope；文件搜索页切换类型，均保留查询；操作选择 / 输出层不切换类型 |
| 用户定义的 Action 热键 | 同一解析与执行路径；可以给 File Search 配置 local / global 快捷键 |
| 文件预览中的 Esc / 返回 | 先关闭预览，保留查询、类型和所选文件 |
| 文件页 Esc（有查询） | 清空文件查询，留在文件页 |
| 文件页 Esc（查询为空） | 返回进入工具前的页面与查询 |
| 文件页返回按钮 | 直接返回父页面；若在操作选择 / 输出层，先返回该层的列表 |
| 文件操作列表 / 输出中的 Esc | 先关闭当前操作层，不清空文件查询 |
| 捕获内容 Esc | 回到 Current App 并恢复环境 Context |
| Current App 空查询 Esc | 进入完全空白 Global Search |
| Global Search 空查询 Esc | 关闭窗口 |

⌘P / Ctrl P 不被文件页默认占用。预览通过 ⌘K / Ctrl K 的二级 Actions 中的 Preview 行打开：方向键选择、Return / 主按钮或双击执行。预览仍使用内联详情区，Esc / 返回先关闭预览。次级操作取消后也保留原查询与选择。裸 Tab / Shift Tab 在统一窗口 builder 中被消费，覆盖 Launcher、Settings、Running Tasks 和弹窗；快捷键录入与组合键映射保留。主窗口原有编辑与中文 IME 行为保留；组合输入未提交时，不执行列表快捷键。

## 实现边界与后续扩展

- Rust ActionRegistry 注册工具入口；`EntrySource` 只在非空、无 Scope 的查询中提供可见工具候选。`binding_offers()` 独立提供快捷键执行目标，不能因为首页隐藏而删掉执行能力。
- 工具入口的内部 `core.launcher` capability 不进入用户 Context Graph / Scope，`home_eligible=false`。UI 不通过把某个 Action ID 特判为首页候选来“推广”新功能。
- `HostView` 是类型化导航结果。Flutter 保存父页面的输入 / Scope / 菜单路径 / 输入模式，退出时创建新 Revision。全局 Action 热键直接进入工具时，返回目的地为空白 Global Search。
- 文件搜索仍只在 `InputMode::FileSearch` 运行；普通查询不触发文件索引查询。类型过滤下推 Rust，先过滤再保存、计数和加载；不限制为前 64 个命中。允许范围由设置确定，搜索栏显示当前范围。类别按文件名后缀（不区分大小写）识别，不读取 MIME 或额外访问文件；目录不按后缀归类。Documents 包含 PDF、Office、纯文本等；Code 包含常见源码、配置及 Dockerfile / Makefile 等无后缀文件。未知后缀仍在 All types / Files 中可见。过滤后为空时提示切回 All types 或修改查询。
- 文件预览继续验证 Session / Revision / Candidate，单 worker、64 KiB 文本限制、过期请求隔离；不是 Quick Look，不声称图片 / PDF 渲染。
- 新工具先增加普通 Action 与必要的 HostView、工具状态；复用搜索栏、列表、详情、反馈和底栏。需要第二个真实工具时再提取通用工具导航状态，避免现在为假想功能建立空抽象。
- 新能力不默认增加 Scope、主入口、全局热键、常驻首页行或底栏按钮。确需默认快捷键时，应有跨页面一致的语义与冲突测试。

## 查询与输入连接（build 11）

File Search 入口只使用动作标题和显式别名 `files` / `文件搜索` / `搜索文件` 参与既有匹配，不再附带 `find files`、`folder`、`spotlight` 等隐藏词。可见候选与快捷键 Offer 继续独立；查询不匹配时入口隐藏，不撤销自定义热键。输入改变后，在匹配当前输入的新快照到达前不显示旧候选，避免旧入口短暂挂在列表里。

搜索输入的父级 `Expanded` 使用固定 `launcher-input-slot` Key，输入框自身继续使用固定 Key。真实异步执行会先出现加载图标，工具导航又同时改变两侧控件；只有输入框 Key 无法保护被重建的父容器。Host 模式切换或当前窗口 show 完成后按帧调用编辑器的 requestKeyboard，同时确认输入连接和原生文本接收者；不主动 unfocus，保留正在组合的文字。以代次、窗口可见性和当前模式拒绝过时恢复；普通查询更新不重新聚焦。跨帧测试覆盖 Return、双击、主按钮与额外的焦点丢失，直接用现有 TextInput client 输入，避免 `tester.enterText` 隐藏焦点问题。

## 验证

自动化检查：空白 Global Search 无常驻工具 / 底栏、真实入口中英文查询、隐藏入口下的快捷键解析和 Usage、用户实际按键进入工具、全局热键直接进入 / 返回、根入口切换保留查询、连续 Esc、直接返回、文件操作层控制隐藏、输入焦点 / 中文 IME、大字号、短窗口标签自动滚动、Ctrl Tab 正反循环与回绕、裸 Tab 不转移焦点、⌘K / Return 预览与分层返回、各类后缀与目录区分、过滤 / 预览、迟到预览与已有 Context / Scope 回归。

可选离屏检查（不启动 App）：

```sh
cd apps/flutter
flutter --no-version-check test --dart-define=ANYWHERE_CAPTURE_UI=true \
  test/file_search_page_test.dart
```

输出到 `build/ui-v0.7-build13`，仅检查布局与字体，不替代实机原生玻璃、GUI / 系统权限验收。历史离屏布局记录见 [V0.7 MVP](mvp-v0.7.md)；当前 V2 包与实机验证见 [build 14 记录](validation-v0.7-build14.md) 和 [0.7 收尾记录](v0.7-closeout.md)。


## 文件搜索引擎 V2（2026-10-10）

Settings → Search 显示统一引擎说明、隐藏开关、检索目录与明确排除名，移除引擎选择。旧 engine 字段仅为配置/CLI 兼容，Launcher 总是 Spotlight 快速候选 + Rust 完整遍历。

文件页底栏显示命中总数和 Searching / Search complete / Partial results；所有结果使用一个连续列表，不显示页码、翻页或刷新按钮。滚动距末尾三行以内自动追加内部批次；↓ 到已加载末项时追加并选择下一项，↑ 直接返回已加载的早期结果，最终首尾不循环跳转。追加保留选中项、滚动偏移及已展示对象的执行能力；失败保留原列表并反馈，可以再滚动/按 ↓ 重试。类型栏、Actions、Preview 与主 Launcher 保持原有规则。

## 2026-10-12：build 18，模式选择与浅色桌面对比度

### 问题与改动

- 空白 Global Search 的原生窗口高度只有 72 px。Flutter PopupMenu 的可视范围仍在这块窗口内，模式菜单第二项被裁切。模式选择改为主窗口内的两行导航选项；打开时 Host resize 到 181 px，收起后恢复原页面高度。复用同一输入框、玻璃、线性图标和透明选中态，不增加原生菜单或另一个窗口。
- 当前模式用勾标识，键盘高亮沿用候选样式；↑ / ↓ 移动、Enter 切换、Esc 只取消。点击同一入口也可收起，取消保留查询；点击当前模式不会把窗口关闭。Scope 和文件类型过滤语义不变。
- 选择模式属于主动操作，停止本次唤醒的延迟预填，避免在选择过程中被迟到的剪贴板 / 选中文本替换。输入查询、换根模式或关闭窗口清理模式选择状态。
- 浅色背景下 Launcher 添加中性炭灰半透明遮罩，保留原 native glass；没有新增 BackdropFilter、逐组件模糊或改动 Impeller。让背景的高亮图形与前景分离，消除此前浅灰玻璃与细小白字接近的层次。
- 路径 / 来源保持次级文字样式，选中时也不提升到标题亮度；选中仍为透明白色薄层与细边。共享次级提示色适度提亮以支持小字号。

### 验证范围

新增模式选择回归检查 72 → 181 px Host resize、两项均在面板内可点击，箭头 / Enter / Esc、原编辑器身份保持、取消不重跑查询及继续输入。对比度测试覆盖代表性浅色 native material 合成后的正文 / 次级文字 / 提示色与选中态；这是颜色模型检查，不是系统玻璃在所有桌面上的保证。实机视觉另行检查。

最终验证：Flutter analyze 无问题，180 项 Flutter 全量测试通过；随后新增的文件图标对比度回归与全部 7 项颜色测试通过。实机启动 build 18，模式选择两行均完整位于玻璃窗口内，已检查菜单与正文外观。箭头 / Enter / Esc 模式切换和取消高度恢复由 widget / Host resize 回归验证；本轮不把这些测试当作全部物理键盘验收。

build 18 App / arm64 ZIP / SHA-256 已生成；0.7.0+18、4 个主二进制 arm64、Impeller、deep / strict 签名与 ZIP 的 37 个普通文件 / 8 个符号链接一致性通过，签名身份与 build 17 相同。没有改 Rust / 原生功能代码，本轮未重复其功能测试；未提交推送。
