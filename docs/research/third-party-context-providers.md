# 第三方 Context：实施状态与后续边界

2026-10-27 更新。[2026-10-20 调研原文](../archive/third-party-context-providers-2026-10-20.md)已归档，其中“未引入 QuickJS”和 worker 优先方案不代表当前实现。

## 已实现

- Manifest 静态声明后台解析器；Rust 校验类型所有权并注册到已有 Context 图，不复制第三方 Graph / Scope。
- 派生类型使用 `plugin.<package-id>.*`，不能冒充第一方类型或将路径字符串升级为文件权限。
- 输入经 `context.read` 授权投影，后台解析器只处理获准数据；不开放环境采集、任意文件、网络或进程访问。
- 使用按需 rquickjs，每项任务独立创建并释放 Runtime；纯 UI 包不启动后台 VM。
- 宿主负责身份、结果限额、取消和发布前的会话 / 目录版本校验。UI 与后台共享 SDK 类型和能力代理，不直接通信。
- JSON Workspace 已使用自有 Context 及后台复制命令；原生验收范围见 [Build 24](../benchmarks/acceptance-fixes-v0.14-build24-2026-10-27.json)。

公开接口和预算以 [插件开发指南](../plugin-development.md)为准，生命周期和信任边界见 [Web 插件架构](../web-plugin-design.md)，代码地图见 [运行时重构](../plugin-runtime-refactor.md)。

## 尚未实现

当前 QuickJS 在 App 进程内运行，堆 / 栈 / 执行预算不等同于操作系统级隔离。独立 worker、崩溃隔离、跨插件 Context 共享和完整环境 Provider 仍待后续设计；不因已能注册派生类型就宣称支持这些能力。
