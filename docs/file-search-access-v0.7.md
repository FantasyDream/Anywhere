# V0.7 文件搜索授权

日期：2026-10-12；当前开发包 build 20，build 19 验证保留在下文。

## 用户流程

1. 启动时先恢复已保存的目录授权。没有覆盖当前范围的授权时，显示系统文件夹选择窗口；可选择配置范围内需要搜索的文件夹，支持多选和只授权其中一部分。默认范围为 Home。
2. 选中后保存只读安全作用域书签，保留本次进程的资源访问租约。以后启动恢复书签；过期、被撤销、不可读或新范围需要再次准备。
3. 保存搜索设置时重新准备访问范围。相同已审阅范围的普通设置保存不反复弹窗；修改范围、新启用文件搜索或显式点击 Authorize saved directories 才补办需要的授权。
4. 取消或没有任何有效授权时，文件搜索保持停用，不把空 roots 当成 Home 回退；Apps / Current App 等其余功能可继续使用。
5. Settings → Search 提供 Authorize saved directories 重新审阅保存范围，Full Disk Access… 打开系统设置入口。前者针对已保存设置，编辑范围后先 Save file search。

完全磁盘访问不能由应用自动授予，必须用户在系统设置中启用；目录选择也不等同于完全磁盘访问。依据：[Apple 文件访问控制](https://support.apple.com/en-ca/guide/security/secddd1d86a6/web)、[Apple 文件访问 API](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox)。操作系统仍可独立保护某些系统 / 应用数据；没有声称一次文件夹授权永久覆盖每个后代。

## 搜索与权限的边界

- 搜索代码不调用授权方法。授权端口只由启动、设置保存和显式审阅调用。
- 等待授权前先停用 Rust 文件源并撤销旧文件查询 / 目标，再发布实际有效 roots。拒绝、传输失败或空授权不能扩大搜索范围。
- macOS Host 保存原生安全作用域书签；Flutter 只处理目录列表和能力标志，Rust 接受平台无关的 `restrict_to_roots` 策略。无对应授权能力的其他 Host 保留现有平台搜索行为。
- 显式路径先做规范化范围检查，随后校验真实目标；`..`、符号链接、候选与快速索引链接不能绕出授权范围。macOS 的 /var、/tmp、/etc 系统别名在平台层规范化，不探测用户目标后再猜范围。
- 扫描遇到 PermissionDenied 时继续其他目录，计入 `inaccessible_count`，不塞进异常警告或 Retry。底部显示受限位置跳过数；真实 I/O / 存储错误仍保留 Partial，避免把失败伪装成完整覆盖。
- 没有全盘预扫、权限索引、修改文件 POSIX 权限或自动开启系统隐私权限。恢复书签和当前根目录检查在启动 / 设置阶段进行；不可读子位置仍依赖操作系统返回，不能保证操作系统未来新增的保护策略永不自行显示提示。

## 验证

- Native 测试用临时目录和注入的选择回调，验证组件边界、链接逃逸、保存恢复、取消、有限授权及同范围提示去重；不替用户授予系统隐私权限。
- Dart 端控制尚未完成的授权响应，验证先撤销搜索、只启用有效目录、空范围不回退 Home、失败后停用及跨平台能力隔离。
- Rust 验证受限目录计数与可读匹配共存、真实 POSIX 不可读目录、显式路径 / .. / 链接拒绝。
- Widget 验证 438 个受限位置只在底部注明，没有权限错误警告 / Retry，已有匹配仍能执行。

系统文件夹选择交互、受保护系统目录与完全磁盘访问的真实授予 / 撤销矩阵需要用户操作，本轮不会把临时目录书签测试当成这些系统隐私场景已验收。

最终 `scripts/check.sh` 通过：340 项 Rust 测试、186 项 Flutter 测试、原生 Host / 授权 / Material / 焦点检查、Clippy、格式和架构检查通过；6 项既有外部工具 / 大夹具 / 系统注册测试 ignored。最后补强的 POSIX 受限目录测试单独重跑通过，包含 Spotlight 元数据不能绕过目录可读检查。

实机启动 build 19，已显示 Authorize File Search 系统目录选择窗口、Authorize / Cancel 与保存访问范围说明；停在该授权步骤，未替用户选定或扩大系统访问范围。App / arm64 ZIP / SHA-256 已生成，版本、Impeller、4 个 arm64 主二进制、deep / strict 签名以及 ZIP 37 个文件 / 8 个符号链接与 App 一致；签名身份与 build 18 相同。本轮未提交推送。

## build 20 异步授权修复

- 每次准备访问推进 Native epoch；旧面板取消、旧 worker / 回调复验版本，不能重新发布旧 ready/roots。磁盘服务使用串行 worker，AppKit 和 UI 回复在主线程。
- Dart 所有配置写共用串行队列，并在执行写入前检查 generation；新的撤销不会被旧完成覆盖。Settings 授权端口可注入，旧授权状态不覆盖新保存或 dispose。
- 5 秒 watchdog 让当前准备失败并保持停用；已经开始的同步 OS I/O 仍占用 worker，完成后过期结果释放租约，不额外放开并发。
- Native 排队旧工作 / 过期授权，Dart 逆序返回 / 撤销，Settings 配置变更隔离有回归。完整 gate 通过，真实系统隐私授予矩阵的边界不变。
