# V0.14.1 GPUI 交付范围与验收状态

最后核对：2026-11-24。当前源码面向 macOS 15+ 开发预览。已发布的
[`v0.14.1`](https://github.com/FantasyDream/Anywhere/releases/tag/v0.14.1) 是
Apple Silicon 开发预览包；上一版
[`v0.14.0`](https://github.com/FantasyDream/Anywhere/releases/tag/v0.14.0)
继续保留为不可变历史。当前源码包含外观、页面操作、键盘 / 焦点和文件、剪贴板、
应用历史、原生工具窗口与 Web 生命周期改动，JSON Workspace 示例为 0.7.3。
复验当前功能时应从受测源码构建 App 和示例包，并记录源码差异及包摘要。

## 交付范围

| 区域 | 源码已接入 | 分项用例 |
| --- | --- | --- |
| 产品壳与输入 | 菜单栏、浮动 Launcher、两种唤醒方式、Scope、Context 自动填入、应用搜索、Current App 菜单与对象动作 | ENV、LCH、KEY、CTX、APP、MENU、ACT、HOT |
| 文件和剪贴板 | 授权目录搜索、三种快批引擎、完整遍历、操作和 Quick Look；历史记录策略、格式恢复与独立置顶 Shelf | FILE、PRE、CLIP、SHELF |
| 设置与外观 | 七类设置、草稿与保存、语言、跟随系统 / 浅色 / 深色、登录项与功能开关 | SET、HOT、UI、DSL |
| 应用历史 | 18 个内置来源、自定义来源路径与适配、搜索、原应用打开或复制远端 URI | HIST、SRC |
| 项目与任务 | Git、Rust / Maven / Gradle / Node / Spring Boot、收藏目录、独立 Running Tasks、查看输出和停止 | GIT、DEV、TASK |
| Web 插件 | 本地包安装与授权、更新 / 回滚 / 撤销、WebView / SDK、按需后台 Context 与命令；Text Pad、JSON Workspace、JSON Summary | PKG、WEB、JSON、BG |
| 交付与资源 | 本机架构 App、开发签名、ZIP / SHA-256、独立调试符号；性能、跨模块竞争和升级 | PERF、REL、RACE |

职责与实现地图见 [架构](architecture.md)，用户入口见 [使用指南](usage.md)。

## 自动化证据

2026-11-24 在当前源码上执行 `scripts/check-gpui.sh` 和 `scripts/check.sh`，
均通过。GPUI 316 项通过；完整 Rust workspace、Web SDK 55 项和 JSON
Workspace 33 项通过，严格 Clippy、原生菜单、生成物、文档和架构检查通过。
这里记录命令级工程结果，逐用例执行由[完整验收清单](test-acceptance-checklist.md)记录。

并行 GPUI 门禁暴露了多个测试同时读取同一 `NSPasteboard` 时的 AppKit
枚举变更异常。平台层现将原生剪贴板上下文操作串行化，并增加锁行为回归；
修复后的完整门禁保持通过，但仍不据此宣称物理剪贴板矩阵已通过。

2026-11-17 至 18 的包身份、进程级启动、实机输入和其他结果见
[历史构建记录](archive/v0.14-gpui-development-records.md#2026-11-17-至-18-发布前包与桌面验收)
与[当轮工作树审查](archive/reviews/worktree-review-2026-11-18.md)。历史证据按
原构建归属。

## 物理桌验矩阵

[全量测试用例与验收清单](test-acceptance-checklist.md)统一维护 341 条用例、
18 个应用历史来源和环境矩阵。本文更新时未重新构建或启动 App，当前源码的
实机项目保持 **Pending**。按清单记录实际被测 App、设备、macOS、窗口与输入
证据及缺失环境；18 个来源的真实打开分别记录。旧 App 的桌面证据与工程门禁
结果不计入本轮实机用例。

重点覆盖真实中文 IME、热键与状态栏、原生焦点和多屏 / Space、系统权限、Quick Look、
登录项、WebKit 文档替换、外观与辅助技术。VoiceOver 有独立用例和正式分发门槛，
开发预览可单独记录其验收缺口。

## 发行边界

- Windows 11 x64 开发预览的功能和实机证据见 [Windows 指南](windows.md)。Linux 当前验证共享 Rust crates。
- Developer ID 签名、公证、干净机器升级与自动更新仍待完成。
- Web 后台使用进程内 QuickJS。第三方进程隔离、插件市场、浏览器历史 /
  书签 / Tab 管理和交互式 PTY 属于后续范围。
- 测量按[性能指南](performance.md)记录构建和资源归属。

开发预览交付按[发布检查](releasing.md)核对工程门禁、产物身份、签名和桌面
范围。范围内的 P0 / P1、物理缺口、Blocked 和正式发行专项在验收报告中分别列出。
