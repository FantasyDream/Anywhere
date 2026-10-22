# Web 插件 P0 探针（已退役）

2026-10-18，独立 Flutter / Swift 回显工程验证了 flutter_inappwebview 6.1.5 / macOS 1.1.2 能嵌入手工 assemble / swiftc 宿主。原工程只处理合成 probe.echo，不读取真实剪贴板或调用 Rust。

当时静态分析、7 项探针单元测试、本地 Unicode 回显和未开放方法拒绝通过；connectBlocked 只证明一次 fetch 失败，不证明完整网络隔离。真实 IME、资源释放、Windows 和攻击矩阵未被该探针完整验证。

2026-10-22 清理：正式 Web 宿主、Rust Broker、SDK 与示例已取代探针。删除 experiments/web_plugin_probe 与 scripts/build-web-plugin-probe.py，避免维护第二套桥、测试和锁文件；原始源码可从 Git 历史恢复。仍使用的 scripts/webview_native.py 保留为正式 App 的原生框架构建工具。

当前开发从[第三方插件指南](../plugin-development.md)开始；回归使用 apps/flutter/test/web_*、Rust Web 包 / Broker 测试和 SDK / JSON 示例测试。[实施记录](web-plugin-development-records.md)保留当时观察与限制。
