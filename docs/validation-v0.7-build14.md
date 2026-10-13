# V0.7 build 14 打包与实机验证

日期：2026-10-11。当前分支：`dev-0.7`。版本：Rust `0.7.0-dev.14` / App `0.7.0+14`。

## 交付物

- `build/Anywhere-dev-0.7-build14.app`
- `build/Anywhere-dev-0.7-build14-macos-arm64.zip`
- `build/Anywhere-dev-0.7-build14-macos-arm64.zip.sha256`

使用 `scripts/build-macos.sh` 构建 arm64 release，保持 Impeller 和既有玻璃背景。签名 identifier 为 `dev.anywhere.launcher`，designated requirement 与 build 13 一致。deep / strict 签名检查通过；ZIP 内容逐文件 SHA-256 与 App 对比，并校验符号链接。

## 自动化检查

最终 `scripts/check.sh` 通过：335 项 Rust、174 项 Flutter 测试；6 项环境依赖 / 大目录测试本轮仍 ignored。原生菜单、Host、材质、窗口输入焦点检查，Rust fmt / Clippy / 架构检查和 Flutter analyze 通过。`git diff --check` 通过。

本轮实机发现文件工具页继承 Current App 菜单超时提示的问题：`LauncherViewState.notice` 现在在文件页仅展示该工具的查询覆盖问题和 Controller 操作错误，不展示无关的环境菜单诊断。新增回归测试验证完整文件查询仍显示匹配结果、没有菜单超时提示。临时键盘诊断代码已移除后重新构建最终 App。

## 可见 App 验证

通过桌面自动化启动真实 App，观察 AX 状态、截图和执行结果；没有仅以 Widget 测试代替这些验证。

测试目录：`build/validation/file-search-build14`，140 个 PDF 后缀文件、30 个 TXT 文件、10 个隐藏 TXT 文件、3 个目录，共 183 项。使用显式路径前缀查询该目录的直接子项，不代表默认 Home 范围的完整遍历验证。

| 操作 | 实际结果 |
|---|---|
| Current App 唤醒后无需点击直接输入，重复重新唤醒 | 输入框接收文本，结果随查询更新 |
| Esc 返回 Global → 输入 `files` → Enter | 进入既有 Launcher 文件工具页，输入焦点保留 |
| 输入测试目录路径前缀 | `183 found · Search complete` |
| 连续滚动跨过内部 64 条批次并到底 | 能看到最终隐藏文件 `.awverify-hidden-009.txt`；没有分页按钮、没有跳回顶部 |
| 方向键跨首批边界 → Actions | 到达 `awverify-064.pdf`，动作标题与路径匹配所选对象 |
| 后续加载后操作较早结果 → Copy Path | 动作针对 `awverify-000.pdf`，App 隐藏，执行日志显示复制动作完成；没有独立读取剪贴板内容 |
| 类型切换 PDF / Folders | 分别为 140 / 3 项，匹配过滤结果 |
| 查询 TXT 0 → Actions → Preview → Esc | 预览显示测试文本、70 B 与位置；Esc 关闭预览 |
| Enter 默认 Open File | TextEdit 打开 `awverify-text-000.txt`，窗口路径和可见内容匹配；测试文档未编辑并已关闭 |
| 单独 Tab 后继续输入 | 文本继续进入搜索框；未启动额外的 Tab 导航模式 |
| 最终提示修复后重新打包并启动 | 直接输入正常，进入文件页后查询仍返回 183 项，页面没有环境菜单错误提示 |

## 验证边界

- PDF fixture 仅用于名称 / 类型过滤，未验证真实 PDF 渲染。
- 本次没有验证 Home 全量、隐私权限矩阵、离线 / 网络卷、冷缓存大规模扫描、长时间泄漏或 UI 帧率。不把历史独立遍历器 RSS 等同于整 App 内存。
- 桌面自动化注入的组合键无法作为实机通过证据：原生事件携带 Control / Command 标志，但注入的修饰键变化事件 keyCode 为 0，Flutter `HardwareKeyboard` 状态未识别修饰键。对应逻辑的自动化测试通过；真实物理键盘的 `⌘K` / Scope 组合键及全局热键验收仍待进行。诊断片段位于 `build/validation/build14-key-injection-diagnostic.json`。
- 本轮未提交或推送。旧版本包保留。
