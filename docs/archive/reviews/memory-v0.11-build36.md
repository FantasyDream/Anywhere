# V0.11 Build 36：过期界面的资源释放

日期：2026-10-21。延续 Build 35 的低峰值优化，本轮处理隐藏后的引用寿命；保留 Impeller、玻璃背景和 15 秒工作区保留规则。

## 问题与修改

1. **解码缓存仍保留**：Launcher 丢弃快照与编码图标后，Flutter ImageCache 仍能持有已解码图片及 provider。隐藏且无保留工作区时，在重建完成后的帧末清掉当前引擎的闲置缓存。只调用 ImageCache.clear，不调用 clearLiveImages，也不全局强制清理其他引擎的资源。
2. **Undo 栈跨过期存活**：清空同一个 TextEditingController 不会销毁 EditableText 内部撤销栈。为搜索区增加由页面持有的 editor epoch，仅在工作区过期 / 明确丢弃时更新；原有控制器和 FocusNode 不更换。新编辑器不再持有过期查询的撤销记录，15 秒内唤醒仍复用原编辑器和历史。
3. **隐藏引擎停止帧刷新**：Flutter 在 hidden 生命周期下可以停用普通帧调度，单纯等待下一帧会把清理拖到再次唤醒。仅在发生上述释放边沿、且普通帧已停用时提交一次 warm-up frame，完成过期子树释放；不改变生命周期，不恢复持续刷新，不添加轮询。
4. **焦点与异步边界**：隐藏编辑器禁用 autofocus；初始空文本被标记为已同步，避免首次聚焦误发空查询。帧末复验当前 Controller、隐藏状态和保留状态；已经重新激活或切换 Controller 的页面不会被旧清理清空。Settings 可见时不清理其共享引擎缓存。

以上均为 Flutter 呈现层资源管理。Rust 继续决定 Session / Context / Action，未修改业务过期时长、搜索限额、索引内容、插件权限或原生渲染器。

## 可复现的回归证据

新增 `apps/flutter/test/desktop_image_retention_test.dart`，使用真实 ui.Image 构造 1024×1024、4 MiB 的闲置解码缓存，不访问用户剪贴板 / 数据库。

- 修改前工作区已过期，缓存统计仍为 4,194,304 B；修改后过期帧完成时为 0。
- 14 秒唤醒时缓存仍为 4 MiB，输入和同一个 UndoHistoryState 保持。
- 修改前过期查询的 UndoHistoryState 仍 mounted；修改后旧状态已卸载，新编辑器没有可撤销内容。
- 模拟 hidden 生命周期且 framesEnabled=false，过期后仍能完成缓存 / 编辑器清理，framesEnabled 仍为 false，也没有重新建立文本输入连接。
- 清理排队后立即唤醒，及 Settings 可见时工作区过期，都保留当前可用缓存。

原始复现与回归日志：`build/memory-idle-before36.log`、`build/memory-undo-before36.log`、`build/memory-idle-final36.log`。开发期间的完整回归发现隐藏编辑器重新接管输入及首次焦点误发空查询；均在本轮修正，未放宽原有焦点 / IME 断言。

最终 Flutter **328 项**通过，analyze 无问题，架构检查通过。日志 `build/memory-flutter36.log`、`build/memory-analyze36.log`、`build/memory-architecture36.log`。本轮没有修改 Rust / Native 逻辑，未重复其全量测试。

## 交付与验收范围

App / ZIP：`build/Anywhere-0.11-preview-build36.app`、`build/Anywhere-0.11-preview-build36-macos-arm64.zip`，版本 0.11.0+36。签名 / 包一致性见 `build/memory-build36.log`、`build/memory-package36.log`。没有提交或推送。

4 MiB → 0 是框架缓存的可控回归结果，**不是完整 App RSS / physical footprint 的实机降幅**；Dart GC、引擎、Metal、OS 缓存与仍在使用的图片均有独立寿命。过期后的再次显示需要重新解码闲置图片，15 秒内快速唤醒没有新增清理。

单元测试和构建后尝试桌面验收，工具报告 Mac 仍锁定，因此没有切换运行版本、完成 GUI 回归或采集新旧 App 同场景内存曲线。原有 WebView 冷开空白仍待定位，本轮不宣称修复。

后续验收：2026-10-21 解锁后已切换到 Build 36，检查了全局输入、JSON 自动格式化、快速隐藏恢复和超过 15 秒重新激活。验收同时发现空 Settings 窗口和剪贴板线程异常；后续修复与 WebView 冷开排查见 [Build 47 记录](verification-v0.11-build47.md)。未补充整 App 内存曲线。
