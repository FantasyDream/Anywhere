# V0.11 性能与交互收尾

日期：2026-10-21。审查基线为 Build 48，主要修复在 Build 49；实机发现的设置提示问题由 Build 50 修正；最终 Build 51 包含渲染保护。JSON 包更新为 0.4.3。

## 发现与改进

| 范围 | 代码 / 测试直接支持的问题 | 处理 |
|---|---|---|
| 中文输入 | ClipboardHistorySession 在 composing 期间仍提交查询；开始组词前的搜索也可能迟到覆盖 | 组词时取消 debounce、使旧响应失效；提交后恢复一次查询，包含最终文本未变化的情况。后台刷新同样遵守 composing |
| 预览性能 | 重复选中同一条目、在列表边界重复按方向键，会清空预览并重新读取正文 / 解码图片 | 复用当前条目的预览或正在进行的请求；仍滚动到选中行。只保留当前预览，不新增历史正文缓存 |
| 复制反馈 | 复制后固定等待 search，且 search 等待图片预览；恢复目标应用也受预览延迟影响 | 元数据完成后停止搜索指示；复制直接报告写入结果，删除仍刷新。Rust copy 不修改历史表，后续采集 / 正常刷新更新元数据 |
| 隐藏与快捷键 | 两个剪贴板编辑器始终 autofocus；历史 Actions 未禁用隐藏状态；历史页 ⌘K 还会吞掉额外 Shift / Alt 修饰 | autofocus 跟随可见状态；隐藏或 composing 时禁止条目操作；历史页与悬浮页使用相同修饰约束 |
| 设置错误 | Build 49 实机显示原始 PlatformException / OSStatus，并把快捷键刷新错误挂到 Features 等无关页面 | 用简短恢复提示替代框架异常；自动刷新提示留在 Shortcuts，明确保存失败仍反馈。继续注销不确定的系统绑定，不删用户配置或绕过注册失败 |
| JSON 性能 | 只改公式时，run 也对整份输入执行 autoFormat；QueryEngine 的解析缓存没有覆盖格式化 | QueryEngine 复用单份已格式化文本与缩进；内容 / 缩进变化重新处理，清空或销毁一起释放，不缓存光标 / 撤销历史 |
| 共享样式 | 动态提示缺少无障碍 liveRegion；路线图 / UI 文档的当前构建号和 JSON 阶段描述过时 | 共享 AppNotice 补语义；同步现状，保持已有玻璃、Scope 和公共设计参数 |

修复前四项剪贴板回归全部失败，设置提示回归同样先失败，分别见本机忽略日志 `build/closure-regressions-before49.log`、`build/closure-settings-before50.log`。修复后均通过。

## 分层核对

- Rust 继续处理历史存储、内容识别、排序、配额、缩略图与执行资格；Flutter 只处理 composing、选择、预览呈现与反馈。正文不进入候选元数据，列表仍懒构建。
- Web 文档寿命、资源并发、消息与外观各有边界；本轮没有引入插件专用 Host RPC。JSON 格式化缓存属于普通包业务，Host / SDK 样式与玻璃仍由框架提供。
- 缓存复用基于历史 ID；SQLite 表使用 AUTOINCREMENT，正文按内容 fingerprint 去重，删除后不将旧 ID 分配给新内容。
- 架构门禁继续通过：Core 无 UI / Flutter 依赖，领域模块不反调 Application / Persistence，跨平台共享代码不选择原生 OS 实现。
- 不为视觉调整切换 Impeller；不通过关闭错误提示、杀 WebKit 共享进程或扩大权限来隐藏故障。

## 性能微测范围

本机 Node 对固定的 328,908 字节 JSON 做 3 次预热和 20 次重复格式化：原 autoFormat 中位约 21.64 ms、最大约 23.80 ms；已格式化文本复用路径中位约 0.008 ms、最大约 0.040 ms。输入为 6,000 行 `{id, name}` 合成数据。

这只测格式化步骤，不包含公式计算、WebKit 渲染、IPC 或 App RSS，不作为整 App 延迟 / 内存降低比例。缓存仍有界；本轮未重做长期内存曲线。

## 工程验证

- `scripts/check.sh` 完整通过（Build 49）：Rust 557 通过 / 6 项既有 ignored、Flutter 332、SDK 16、JSON 24；原生、Clippy、生成物、架构与文档检查通过。
- 设置页补修后重新执行 Flutter analyze 与完整测试：333 通过；加入渲染保护后再次完整执行 `scripts/check.sh`，333 / 557 / 16 / 24 项结果及各门禁均通过（Rust 6 项既有 ignored），日志 `build/closure-check51.log`。
- 原生检查包含菜单身份、剪贴板异常边界、目录授权、热键回退、窗口材质 / 输入焦点、空 Settings 窗口释放和 Web 编辑路由。
- macOS ARM64 Build 49 完成构建与 deep / strict 签名检查；最终产物与实机补验记录见下方。

## 实机已观察（Build 49）

- 确认实际进程路径为 Build 49；当前应用页正确捕获 Edge，主列表与 Scope 正常。
- 历史页直接键入查询可筛选；对一条既有合成 JSON 验证 Up 边界与 Return 复制，底栏显示 Copied；Actions 打开 / Esc 关闭后可以继续输入，查询未丢失。
- JSON Web 工具从冷创建进入加载态，下一次观察时正文正常可见，无需点击 Host 触发重绘；玻璃、页头 / 底栏与原生历史页保持公共样式。
- Settings 的真实错误暴露问题在此阶段发现后补修，不将 Build 49 的该画面记作成功验收。

尚未验收所有系统版本、真实中文输入法矩阵或完整 VoiceOver；合成 composing 回归和动态语义不等于这些实机验收。

## Build 50 崩溃与 Build 51 保护

Build 50 实机启动后，失焦隐藏期间发生 SIGSEGV。诊断文件为本机 `~/Library/Logs/DiagnosticReports/Anywhere-2026-10-21-153409.ips`，faulting thread 是 `io.flutter.raster`，栈顶 `impeller::Canvas::SetupRenderPass`；前两条错误是纹理与描述尺寸不符、缺少 resolve texture。此构建不作为可推荐的最终产物。

[Flutter 上游 PR #192522](https://github.com/flutter/flutter/pull/192522)描述了相同调用栈与尺寸错误；本轮查看时仍未合并。进一步核对本机 Flutter 3.47.4 的 `FlutterSurfaceManager.mm`，并对 SDK 的真实 release `FlutterBackBufferCache` 执行 A / B / A 用例，确认持有中的 A 与空闲 B 混存时，A 请求会返回 B。这比只按崩溃栈推测缩放时序多了一项直接复现证据。

Build 51 的 `NativeFlutterSurfaceGuard.m` 在引擎启动前对取缓存结果校验尺寸。不匹配时返回 nil，复用 FlutterSurfaceManager 的正常分配路径。它不操作缓存私有字段，不修改全局 SDK / Flutter 二进制，不切换 Impeller，也不改变 15 秒状态保留或现有隐藏尺寸策略。安装前检查类、方法与 CGSize ABI，安装结果与首次拒绝写入有界诊断。

这是对固定 SDK 内部 Objective-C 方法的局部兼容适配，不是永久公共 API。SDK 升级需要运行实际框架回归；上游修复进入所用版本后，应删除此保护与构建接线。不能从这一保护推导所有引擎崩溃均已解决。

新增原生回归直接加载实际 Flutter release 框架：先验证未保护路径的错误尺寸复用，再验证保护后拒绝错误尺寸，保留同尺寸复用、全部占用、空缓存及重复安装行为。全部原生测试通过，日志 `build/closure-native51.log`。

## 最终 Build 51 实机与产物

- 实际 App 启动，日志确认 `flutter_surface_guard installed=true`；Current App 页正常显示。
- 5 次连续 Global Search → Clipboard History → 返回搜索，均通过状态检查，覆盖页面尺寸来回切换。该过程无 Impeller validation 日志，检查时没有 Build 51 新崩溃报告；不外推为长时间或全部窗口场景的保证。
- 最后打开 Settings 的两次操作被桌面工具的手动状态保护取消，随后停止桌面输入。设置文案 / 页面限定 / 恢复后的回归通过，但 Build 51 该项实机画面未再次确认。
- JSON 0.4.2 已构建并通过普通包检查及 24 项测试；本轮 Web 画面观察使用此前已安装包，没有将新包自动安装或把微测当成已安装插件的 UI 测量。
- 最终 App 为 `build/Anywhere-0.11-preview-build51.app`，并提供同名 `-macos-arm64.zip` / `.sha256`。Build 49 / 崩溃的 Build 50 已移入 `build/archive/closeout-2026-10-21/`，避免误启动；旧构建不删除，便于比对。
- 完整构建 / 签名 / 归档检查记录在本机忽略日志 `build/closure-build51.log`、`build/closure-package51.log`。本轮未提交或推送。

## 提交前补验

后续在同一 Build 51 中完成此前中断的设置页检查：Shortcuts、General、Features 均正常显示，未出现原始框架错误或跨页面的快捷键提示。没有人为制造系统快捷键冲突；故障提示与恢复范围由回归测试覆盖。

通过正式导入流程更新 JSON Workspace 0.4.3，核对版本、五个稳定命令身份、原有权限和摘要 `4f6ac3a70dbeff053b430ffff2053eea490bdd66dffe45a4c320d762f41e6f1d`。返回搜索并打开工具，直接显示正文，默认选中 JavaScript、固定 `this` 前缀；Esc 返回后保留原查询。这轮补验替代上方“设置页未再次确认 / 新包未安装”的当时状态记录。

V0.11 本地开发预览的工程收尾完成。真实 IME / VoiceOver、完整安全攻击矩阵、Windows 与公开分发仍属于单独的发行门槛，不把它们记作本轮已验收。
