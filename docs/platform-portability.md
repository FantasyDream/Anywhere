# 跨平台架构与接入契约

目标：共享 Flutter 展示与 Rust 业务；只有必须与操作系统交互的接口使用平台适配器。

## 已落地

| 层 | 共享职责 | 系统边界 |
|---|---|---|
| Flutter UI | 所有页面、控件、导航、Context 输入、Scope、交互状态 | DesktopEnvironment 提供字体、命令键与背景能力 |
| Flutter Host port | DesktopHost 窗口/设置契约，RuntimePaths 启动契约 | MethodChannel Host 实现窗口生命周期、焦点、托盘与注册热键 |
| Rust Core | Context Graph、Action、候选、搜索、Ranking、持久使用记录、Execution Sessions | Platform 采集/执行；不判断 Finder ID 或启动 Unix shell |
| Rust 平台服务 | 目录遍历、Git 执行、工具解析、ProcessRequest | 应用入口识别、OS Context 采集、打开资源、进程树/管道后端 |
| 快捷键配置 | schema 4 物理 HID usages 与语义修饰位、动作身份与意图规则 | macOS 转换 Carbon；其他系统转换各自注册格式 |

Flutter 控件不是 AppKit 控件。原生背景是可选能力，不影响共享页面运行；未确认背景能力时不透明显示。Mac 玻璃、Windows 背景材质或无材质可以挂在同一个 Flutter 内容层下。

## 启动协议

主窗口 Host `paths` 返回字符串映射：

- `library`：动态库绝对路径，扩展名/目录由打包 Host 决定。
- `data`、`plugins`：Rust 初始化的绝对目录。
- 既有来源设置、排除应用和启动状态继续保持原字段。

任务窗口同样支持 `paths`，只需 `library`，Rust Application 仍由主窗口初始化并共用。

两个窗口的 Host `capabilities` 返回布尔字段：

- `backdrop`：内容背后确实有系统材质、且用户未要求降低透明度。
- `shortcutRecording`：支持设置窗口内原生按键录制。

未实现方法时回退为不透明背景与 Flutter 按键录制。路径缺失必须停止初始化并展示可诊断错误，不能猜测加载其他动态库。

## 快捷键迁移

`actionShortcutVersion=4`：`keyCode` 表示 USB HID keyboard usage，`modifiers` 的位为 Control=1、Alt=2、Shift=4、Meta=8。只支持当前 keymap 声明的键位。

- 1 → 4：保持 local；丢弃当时不支持的 routes。
- 2 → 4：保持 global；丢弃当时不支持的 routes。
- 3 → 4：保留 global、动作 ID、展示元数据、所有意图规则。
- 物理键不会因当前键盘布局字符变化而改变。
- Meta 在 Windows 表示 Windows 键；导入时不隐式替换为 Ctrl，避免意外抢占或改变动作意义。
- 标准页面命令单独使用平台惯例：Command / Ctrl。
- 未知版本不执行其中的绑定、不覆盖配置；macOS Host 使用安全的默认唤醒键。
- 当前导入/导出入口若继续扩展，必须使用 schema 4；不得直接输出 Carbon 字段。

## 必须保留的系统实现

应用发现、前台身份、文件管理器选中项、选中文字、浏览器地址、剪贴板变化时间、菜单定位/执行、全局热键、窗口焦点与系统权限无法仅靠通用绘制层实现。这些接口已有适配边界；共享 Core 不应新增应用 ID 判断或 OS API。

`bundle_id` / `appBundleId` 目前是兼容字段名，其值视为平台提供的稳定应用标识；不要求其他平台伪造 Apple bundle。`FinderSelection` 也是兼容协议名，来源语义为文件管理器选中项。

## Windows 接入顺序与当前边界

本次调整没有提供 Windows App 或宣称系统功能已适配。后续接入：

1. 添加 Windows Flutter runner 与 Host；实现 paths/capabilities、窗口/设置/任务生命周期。
2. WindowsPlatform 实现应用目录、前台身份、剪贴板、打开文件/目录/URL。
3. 实现全局热键注册与 HID 映射；继续校验冲突与失败回滚。
4. 接入选中文件、文本、浏览器、菜单的系统能力；不支持的来源应明确报告，不能虚构成功。
5. 添加 Windows 进程树与非阻塞输出后端，保持取消、退出、子进程清理语义，再启用开发任务。当前 unsupported 后端在 spawn 前拒绝；工具解析不悄悄执行 .cmd/.bat。
6. 可选接入系统背景材质；不支持时沿用不透明主题。

`.github/workflows/portable.yml` 增加 Linux/Windows Rust 目标编译和共享 Flutter 检查，Linux 还运行 Rust 回归。它验证共享代码，不代替 Windows 窗口、权限、实际菜单、后台任务或安装包验收。

## 本次验证（2026-10-07）

本机 macOS：Rust 全量 277 项通过，4 项既有测试保持 ignored；Flutter 122 项通过，analyze 无问题；Clippy（warnings 为错误）、格式与架构边界检查通过。原生菜单/选区回归、Host 服务/快捷键迁移/回滚与材质回归通过；完整 Swift Host 类型检查通过。

新增验证包括：非 Finder 文件管理器采集、版本 1–3 转换、版本 3 意图保留、版本 4 磁盘通用编码、未知版本禁止覆写、物理键与布局字符分离、Windows Ctrl 页面命令/保留编辑键、PowerShell 复制命令、Host 返回 DLL 路径、无背景能力时不透明显示。

新增 Linux/Windows CI 尚未在 GitHub 执行；没有构建或验证 Windows App。未进行新版 macOS 窗口的 GUI 验收，也未重新打包发布产物。
