# 历史归档：旧声明式插件协议（v1 / v2 / v3）

归档说明（2026-11-17）：外部声明式 TOML 插件协议（v1 / v2 / v3 及 `DetailView`）已从现行产品下线，第三方扩展统一采用 `.awplugin` Web 插件与 `@anywhere/sdk`。现行架构与模块注册见 [架构](../architecture.md) 与 [Web 插件架构](../web-plugin-design.md)。以下内容保留历史兼容设计与迁移边界记录。

---

原整理日期：2026-10-23。本页区分内置 Rust 功能模块和历史外部 TOML 插件。

第一方已改为内置功能模块，按 [P01–P09](feature-module-roadmap.md) 完成迁移。新的第三方 Web 插件详见 [Web 平台设计与进度](../web-plugin-design.md) 与 [开发指南](../plugin-development.md)。

## 第一方模块

URL、Git、Maven、Gradle、Node、Rust、Spring Boot、收藏目录、剪贴板实现位于 `crates/anywhere-core/src/features/`。各模块使用静态 FeatureDescriptor 声明 ID、名称和系统操作，registration.rs 编译期注册；内置描述不经过 TOML、外部协议版本或包版本校验，不从用户目录加载任意 Rust 动态库。设置状态使用 App 的 Cargo workspace 版本，内置模块没有独立版本。

`BuiltinFeature` / `FeatureContribution` 贡献 Provider、ActionBundle、动态动作及启动绑定 Sources。模块开关同时撤销识别器、动作与来源。架构详见 [装配与生命周期](../architecture.md)。

## 历史外部 TOML 插件协议（已退役）

历史版本曾支持 `~/Library/Application Support/Anywhere/plugins/*.toml` 和 `<plugin>/manifest.toml`（v1 免批委托、v2 显式能力批准、v3 只读 `DetailView` 键值视图，原示例为 `plugins/examples/search.toml`、`plugins/examples/copy-v2.toml`、`plugins/examples/context-detail.toml`）。

`features.json`（schema 1）保存内置功能禁用 ID；首次启动仍保留从旧 `extensions.json` 的 `first-party:<id>` 禁用键单向只读迁移的兼容能力，新文件存在后不再重新导入旧值。

旧协议迁移与当时发布门槛见 [V0.9 记录](mvp-v0.9.md)，当前整体状态见 [GPUI 技术方案与验收](../mvp-gpui.md)。历史 v1 / 版本迁移验收保存在 [归档索引](README.md)。
