# Windows 11 验收记录（2026-11-27）

本记录保存 Anywhere Windows 集成分支在 2026-11-27 的最终候选包、工程门禁、
桌面观察和未关闭问题。源码基线为 `0f18732d5ee000718f40b5e25f3d75a2c0b0fe18`
加当时工作树中的 Windows 改动。最终候选是 r16。更早的 r8、r15 只用于追踪
固定缺陷和桌面行为，不冒充 r16 的通过证据。

本轮输入来自 CUA 桌面注入，使用一个 `2560x1440`、100% DPI 的活动桌面面。
它不能证明实体键盘、实体鼠标、系统 IME、双屏混合 DPI、读屏或跨注销行为。

## 产物身份

| 项目 | 值 |
| --- | --- |
| EXE | `package/expanded/Anywhere/Anywhere.exe` |
| EXE SHA-256 | `9C0DCBCC3EB21B275D8FABC1815BC428FDA25C97D0E1B2175A33E5CF2E2D6E1E` |
| EXE 大小 | `86472704` 字节 |
| ZIP | `package/Anywhere-windows-x64.zip` |
| ZIP SHA-256 | `D12F1B005E9AF91B79D75820C23795EDF739F612272AD9439EB2CCE393664A88` |
| ZIP 大小 | `41923973` 字节 |
| PE 架构 | Windows x64，machine `0x8664` |
| Authenticode | `NotSigned` |
| ZIP 完整性 | 28 个条目，ZIP 内外 EXE SHA-256 一致 |

r16 的证据目录有 27 个文件，最终 `SHA256SUMS.txt` 已逐项复算通过。构建产物
位于本地忽略目录 `build/windows-physical-20261127-r16/`，GitHub 检出不会包含
这些二进制、截图和日志，但本页保存了最终产物身份、结论和复验条件。

## 结果汇总

| 状态 | 数量 |
| --- | ---: |
| Pass | 2 |
| Fail | 0 |
| Blocked | 9 |
| Pending | 36 |
| N/A | 0 |
| 总计 | 47 |

r16 没有已知 P0 或 P1 Fail。r15 的扩展桌面基线为 Pass 15、Blocked 9、
Pending 23、Fail 0。两轮 EXE 不同，因此 r15 结果只列为回归参照。

最终工程门禁：

| 检查 | 结果 |
| --- | --- |
| Windows Release 构建 | exit 0 |
| 第一方 Clippy `-D warnings` | exit 0，保留第三方依赖警告 |
| `anywhere-platform` 串行测试 | 61 passed，0 failed |
| MVP 串行集成测试 | 177 passed，0 failed，4 ignored |
| Windows 菜单定向测试 | 4 passed |
| Node 包管理器定向测试 | 1 passed |
| QuickLook 预览定向测试 | 3 passed |
| 文件对象动作定向测试 | 1 passed |
| GPUI 检查 | exit 0 |
| 文档检查 | 通过 |
| `git diff --check` | 通过 |

4 个 ignored MVP 用例依赖真实 Node/npm/Corepack、Maven、Gradle 和 Spring
Boot 工具链，不计入 Pass。r16 主实例日志未出现 panic、ERROR、WARN 或
WebView `RefCell already borrowed`。

## 重点未修复问题

这里把已确认缺陷、已覆盖但仍有风险的行为、自动化能力限制分开记录。

| 编号 | 等级 | 当前状态 | 问题或限制 | 影响与后续动作 |
| --- | --- | --- | --- | --- |
| WIN-OPEN-01 | P2 | 已确认，未修复 | 第三方 `third_party/gpui-fast-windows/src/fast/composition.rs` 保留 `Foundation::HWND` 未使用导入，Release 构建还报告一个未使用函数 | 第一方 Clippy 已通过，但依赖源码仍有告警。依赖升级或下一轮维护时清理，并重新构建验证 |
| WIN-OPEN-02 | P1 风险 | 未复现于 r16 | 隔离重复实例曾在 WebView 初始化期间出现 `RefCell already borrowed`。最终源码包含 gpui-fast 0.1.19 队列语义的窄回退和 AccessKit 销毁期保护 | r16 主实例完整日志未复现。仍需在正式默认单实例、独立 Web 窗口连续开关和插件授信流程中回归 |
| WIN-OPEN-03 | P1 覆盖缺口 | 未验证 | 默认数据目录的单实例 Mutex 没有执行。`ANYWHERE_DATA_DIR` 隔离路径会绕过该路径 | 需要专用默认目录用户从 Explorer 二次启动，确认原 PID 复用且热键只有一套 |
| WIN-OPEN-04 | P1 覆盖缺口 | 未验证 | 托盘 Exit 尚未用可点击的原生菜单完成。CUA 能看到菜单五项，但不能跨进程绑定 `#32768` 弹出菜单 | 必须人工点击 Exit，核对 PID 结束、热键释放、同目录重启和配置恢复 |
| WIN-OPEN-05 | P1 覆盖缺口 | 未验证 | QuickLook 自有 WPF 预览窗口不被 CUA 枚举 | 外部窗口保护已通过，QuickLook 自身关闭、改选和再次预览仍需人工操作 |
| WIN-OPEN-06 | P1 覆盖缺口 | 未执行 | Shelf 删除没有执行，因为该动作会删除本地测试数据 | 在隔离测试数据中确认删除单条、空列表和 Launcher 继续可用 |
| WIN-OPEN-07 | P1 覆盖缺口 | 未验证 | Explorer 标签页、浏览器版本和标签切换、Context 来源开关组合未完成 | 继续执行 W-CTX-01 至 W-CTX-04，核对真实目录、选中项、当前 URL 和停用来源 |
| WIN-OPEN-08 | P1 覆盖缺口 | 未验证 | Everything、Windows Search、Ripgrep、Combined、直接遍历的五引擎桌面矩阵未完成 | 逐引擎核对首批、结束、超时、停止和结果一致性 |
| WIN-OPEN-09 | P0 覆盖缺口 | 未执行 | 第三方 Web 插件取消批准、批准、停用、更新、撤销和旧文档隔离未完成 | 需要声明 `windows` 的隔离测试包，确认权限只来自当前版本 |
| WIN-OPEN-10 | P1 覆盖缺口 | 未验证 | Cargo、Maven、Gradle、Spring Boot 桌面任务矩阵未完成。r15 只实测了 Node/npm 父子任务和 Stop | 使用合成项目逐工具核对检测、工作目录、输出、失败和进程树回收 |
| WIN-OPEN-11 | P2 风险 | 未验证 | 第三方未隔离版本没有回归未知私有剪贴板格式、虚拟文件和延迟文件对象 | 当前契约只声明文本、文件列表、PNG/DIB、HTML 和 RTF，其他格式按不支持处理 |
| WIN-OPEN-12 | P1 环境限制 | Blocked | 实体微软拼音、真实键鼠、双屏混合 DPI 和读屏没有覆盖 | 需要真实硬件、系统候选窗和辅助技术观察，代理注入不能替代 |
| WIN-OPEN-13 | P1 环境限制 | Blocked | 登录项跨注销、Explorer 重启和缺 VC++/WebView2 Runtime 的干净 VM 没有覆盖 | 需要专用用户或可回滚 VM，记录启动失败、依赖安装和恢复 |
| WIN-OPEN-14 | P2 工具限制 | Blocked | r16 GPUI 窗口能由热键唤醒，产品日志也显示进入前台，但 CUA 窗口清单没有暴露主窗口 | 导致首字符、逐层 Esc、托盘左右击和部分对象操作无法继续。这是自动化定位限制，尚无证据判定为产品缺陷 |
| WIN-OPEN-15 | P2 工具链限制 | 未通过 | `python scripts/check-code-style.py` 缺少 `tools/code-style/node_modules/eslint/bin/eslint.js` | 不能把代码风格脚本报为通过。补装锁定依赖后重跑 |
| WIN-OPEN-16 | 发布缺口 | 未实现 | 当前只有未签名 x64 ZIP，没有 Authenticode、安装程序、自动更新或运行库随包部署 | 公开分发前完成签名、安装升级和干净系统验证 |

`WIN-OPEN-14`、`WIN-OPEN-15` 和 `WIN-OPEN-16` 分别属于测试工具、验证环境和
发布能力。它们没有在 r16 状态表中被伪装成产品 Fail。

## 本轮已修复

| 问题 | 修复 | 验证 |
| --- | --- | --- |
| Windows npm 只接受 `.exe` / `.com`，找不到常见的 `npm.cmd` | 优先真实执行文件，再要求相邻 `node.exe` 并直接运行 npm / Corepack JavaScript CLI，避免经 `cmd.exe` 参数插值 | Node 定向测试、完整平台测试和 r15 npm 实机进程树 |
| Windows 动态标准菜单没有初始化子菜单 | 读取和投递前递归发送 `WM_INITMENU` / `WM_INITMENUPOPUP`，执行前仍复验身份 | 4 项菜单测试通过，7-Zip About 桌面基线通过 |
| 普通盘符路径在界面显示冗余 `\\?\` | 展示时使用可读路径，安全校验和执行继续使用原对象 | 文件对象动作与 MVP 路径回归通过 |
| WebView2 创建期间可能递归借用 GPUI App | 回退上游重入保护并延迟应用事件，同时让 AccessKit UIA 在窗口销毁后安全返回 | r16 主实例 Web 工具日志无重复借用 |
| r15 的菜单、路径、历史和快捷键回归断言漂移 | 同步当前路径契约并补齐 Windows 菜单与进程解析回归 | r16 平台与 MVP 全量串行测试通过 |
| r15 Release 还有两处第一方 Clippy 告警 | 删除冗余 `return`，把等价条件链改为直接 `if` | r16 第一方 Clippy `-D warnings` 通过 |

## r16 桌面覆盖

r16 自身确认：

- ZIP 与 EXE 对应，x64 PE 和包内容完整。
- 隔离数据目录启动、全局热键和 Current App 热键记录进入前台。
- Release、第一方 Clippy、平台测试、MVP、菜单、Node、预览和 GPUI 检查通过。

r15 基线继续覆盖以下行为，但在 GitHub 中仅作为历史证据引用：

- 计算器、设置、7-Zip 原名与拼音应用搜索。
- 两个 Explorer 窗口和虚拟目录不生成假路径。
- 7-Zip Help / About 标准菜单执行。
- 文件分页、类型过滤、隐藏项、打开、Explorer 定位和复制路径。
- 剪贴板开启前不补录、中文历史筛选、重新复制和 Shelf 复制返回。
- QuickLook 缺失提示、PDF/PNG/TXT 预览和外部窗口保护。
- Git 只读动作和非仓库目录回归。
- npm 到 PowerShell 子进程、Running Tasks 持续输出和 Stop。
- JSON Workspace 计算、复制、独立窗口和后台格式化。
- 设置队列保存、丢弃对话框和重启持久化。

## 证据复验

本地最终报告位于：

`build/windows-physical-20261127-r16/report.md`

本地证据目录：

`build/windows-physical-20261127-r16/evidence/`

复验时应先重新计算 EXE 和 ZIP 的 SHA-256，再确认报告中的包身份与本页一致。
桌面复验按 [Windows 桌面验收指南](../test-acceptance-windows.md)执行。macOS
步骤单独保存在 [macOS 桌面验收指南](../test-acceptance-macos.md)，不得把两个
平台的代理操作条件混为一份清单。
