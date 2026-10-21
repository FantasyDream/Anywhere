# V0.11 Build 47：内存变更验收与运行时修复

日期：2026-10-21。保留 Impeller、玻璃背景、Scope 和隐藏后 15 秒保留规则；没有提交或推送。

## 发现与修复

### 剪贴板原生异常导致进程退出

Build 38 的实机验收捕获 SIGABRT。系统崩溃报告的触发线程为 `clipboard-rs-watch`，栈包含 `__rust_foreign_exception` 和 Rust 线程退出路径。这证明了外部异常进入 Rust 边界，不能据此声称已经识别具体抛错的 AppKit selector。

- `clipboard-rs` 继续负责多格式读写。macOS 格式调用通过 `objc2::exception::catch` 转为不可用错误；`catch_unwind` 无法接住 Objective-C 异常，不能替代该边界。
- 原生权限 / 代数 / 大小预检在 Objective-C 中拦截 NSException，返回失败状态，不让异常穿过 C ABI。代数失败为 -1，不能参与正常内容读取。
- macOS 的变化监听改用受保护的 metadata-only changeCount 查询，维持 500 ms 间隔，不读取正文；关闭 token 会立即唤醒等待并停止。失败查询不发布复制事件，恢复后的已有内容先建立基线。Windows / Linux 仍使用库监听。
- 故障代数不被当作新复制；恢复内容的年龄未知时不自动填入。未扩大权限、增加采集内容或改动采集开关。

新增回归实际发送不支持的 NSObject selector，在生产异常边界中触发 Cocoa 异常并确认返回错误，随后正常调用仍成功；无需访问用户剪贴板。另有监听去重 / 查询失败 / 停止和复制年龄回归。

### 空设置窗口仍被持有

共享内容回到 Launcher 后，原实现仍缓存已经搬空的 Settings NSWindow。桌面工具曾抬起只剩通用窗口按钮的空白窗口，且没有对应的 `settings_visible` 事件。现在保存设置窗口的位置，搬回共享内容后关闭并释放空窗口；下次打开重新创建原生外壳，复用同一 Flutter controller / engine。

`LauncherWindowCoordinator` 只依赖 NSViewController 和 AppKit，不再依赖具体 Flutter controller 类型。原生测试使用真实窗口、autoreleasepool 和弱引用确认空窗口已释放，检查两次打开的位置与共享内容归属；没有为设置新增 engine。

### WebView 冷开正文偶发空白

Build 39–44 仍复现：ready 已完成、原生视图尺寸正确且可见，正文空白，点击普通 Host 标题或 Actions 后出现。临时加载层、焦点交接和把内容移出玻璃容器都未消除问题，已撤回这些尝试和临时诊断。

移除原生首帧截图调用的 Build 45，在独立进程冷启动中连续显示正文，无需点击 Host。Build 47 将该路径完整删除：不再调用 WKWebView.takeSnapshot，也不再维护 prepareWebDisplay 平台消息、Dart 等待接口或其两项失效测试。样式应用完成后直接保留 live surface，仅在 ready 后交接初始焦点。ready 会取消恢复探测 timer，避免成功页面 10 秒后再读取入口资源。

原生回归改为禁止画布配置创建截图，继续检查透明能力、不支持回退与编辑器焦点；Dart 保留外观超时、过期视图、隐藏与恢复竞争测试。独立进程重开属于抽样验证，不能推广为所有 WebKit / Flutter 版本或第三方页面的保证。

## 自动化证据

- Rust：557 通过，6 项既有 ignored；Clippy 通过。日志 `build/verify-rust39.log`、`build/verify-clippy39.log`。
- Flutter：326 通过，analyze 无问题。相对 Build 36 的 328 项，删除的是上述已废弃截图接口的两项测试。日志 `build/verify-flutter47.log`、`build/verify-analyze47.log`。
- 原生：窗口释放 / 位置 / 内容归属、输入焦点、透明画布 / 无截图 / 回退、剪贴板授权范围、设置和悬浮窗几何通过。日志 `build/verify-native47.log`。
- 架构检查通过，Core / Flutter / 平台边界保持。

## 内存结论的边界

Build 36 的受控测试仍证明隐藏过期后 4 MiB 闲置解码缓存归零、Undo 栈销毁、停帧时仍清理，14 秒内恢复不清理。此轮补充空原生窗口释放并删除离屏截图路径；没有建立完整 App 与 WebKit helper 的同场景内存曲线，不能报告整 App RSS 降幅或承诺 GPU helper 每次退出。

实机与交付的最终范围在 [当前 MVP](../../mvp-v0.11.md) 的 Build 47 记录中更新。


## Build 48 补充

后台监听与前台现在统一查询代数端口；发生错误时会失效 OBSERVATION 中的内容年龄，而不只是跳过一次历史采集。回调先执行代数查询，再取得观察锁，避免错误失效路径重入同一把锁。最终产物提升为 0.11.0+48；Flutter / 原生 Host 与已验收的 Build 47 相同。Rust 工作区与 Clippy 再次通过，见 `build/verify-rust48.log`、`build/verify-clippy48.log`。最终 App / ZIP 和实机范围以当前 MVP 为准。
