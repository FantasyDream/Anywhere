# 文档导航

当前交付仍为 **V0.13 开发预览**，工程与该轮常规 macOS 验收已收尾。**V0.14 正在验证 H1 应用历史原型，尚未收尾**。最后整理：2026-10-25。先从[项目首页](../README.md)了解功能和平台状态。

## 使用与参与

| 文档 | 用途 |
|---|---|
| [使用指南](usage.md) | 唤醒、权限、Scope、快捷键、搜索、剪贴板、项目工具和本地数据 |
| [开发与构建](development.md) | 环境、App / SDK 构建、测试、生成物和代码入口 |
| [代码规范](code-style.md) | Google / 官方语言规范、隔离工具、自动检查与生成文件边界 |
| [发布检查](releasing.md) | 开发包、版本、签名 / 归档、验收与 GitHub 发布边界 |
| [贡献指南](../CONTRIBUTING.md) | 改动约定、提交与 PR 验证 |
| [更新记录](../CHANGELOG.md) | 用户可感知的变化与工程里程碑 |
| [帮助](../SUPPORT.md) / [安全报告](../SECURITY.md) | 问题报告、敏感信息和维护边界 |

## 当前设计契约

| 文档 | 用途 |
|---|---|
| [架构](architecture.md) | Rust / Flutter / Host 分工、静态模块、快照、取消与执行 |
| [Context 模型](context-model.md) | 类型、来源、Provider 图、等价与身份 |
| [默认输入](context-input-navigation.md) | 选中文本、15 秒剪贴板、InputMode 和 Esc |
| [Launcher 交互](launcher-interaction.md) | Scope、菜单、二级操作、快捷键和隐藏恢复 |
| [Current App 菜单](current-app-actions.md) | Hidden、定位、重扫、身份和安全降级 |
| [文件搜索](file-search.md) | 范围、权限、稳定连续结果、固定预览和临时存储 |
| [开发动作](developer-actions.md) | Git、项目识别、IDEA 来源和后台进程 |
| [共享 UI](ui-design.md) | 组件、材质、交互信息层级与平台回退 |
| [性能](performance.md) | 资源预算、取消、测量方法和历史样本边界 |
| [跨平台](platform-portability.md) | 平台能力、路径、快捷键与未完成的 Windows 接入 |

## 插件与后续计划

| 文档 | 用途 |
|---|---|
| [第三方插件开发](plugin-development.md) | 独立模板、SDK、统一样式、权限、打包与更新 |
| [Web 插件架构](web-plugin-design.md) | 包身份、能力代理、Context 输入、生命周期和系统边界 |
| [旧声明式协议](plugin-protocol.md) | v1 / v2 / v3 兼容行为；停止扩展 UI DSL |
| [V0.13 MVP](mvp-v0.13.md) | 本轮策略、收尾证据、已知 SDK / 资源边界及后续发布检查 |
| [V0.14 技术方案](mvp-v0.14.md) | 应用历史的具体分层、读取 / 查询 / 执行边界、开源复用与小步验收；H1 原型验证中 |
| [V0.12 MVP](mvp-v0.12.md) | 上一阶段工程与真实键盘验收基线 |
| [路线图](roadmap.md) | 后续功能、依赖与 1.0 门槛 |
| [应用历史设计](application-history.md) | V0.14：IDE / 编辑器 / 笔记最近对象与原应用打开；当前已有 18 个配置化来源 |
| [独立浏览器规划](browser-feature.md) | 后续单独实现历史、书签与 Tab 管理；尚未实现，版本待定 |
| [应用历史来源指南](application-history-sources.md) | 官方配置 / JS 转换、通用读取器、手动启用和当前限额 |
| [第三方 Context 调研](research/third-party-context-providers.md) | 尚未实现的类型注册、隔离和 worker 选型 |

## 历史与研究

- [历史验收、迁移与 Review](archive/README.md)：旧 MVP、已完成迁移、实验和逐构建证据。
- [研究资料](research/README.md)：外部产品、开源选型与未落地设计。

旧版本的通过数字、源码行号和本机构建材料只适用于当时环境，不覆盖当前状态。当前行为维护在专题文档；改变功能时同步其指南，避免复制一套版本号命名的使用说明。

阅读实现状态时，使用指南与专题契约说明当前行为，MVP 记录当前构建的验证程度，路线图描述后续范围。研究稿和设计中的“后续 / 验收要求”不能当成已实现接口；SDK 调用与额度还应核对生成的协议和开发指南。

`scripts/check-docs.py` 检查所有随仓库交付的 Markdown（包括根目录、GitHub 模板、SDK 与插件示例）、本地链接、标题、忽略路径和必要社区文件。移动文档时必须同步引用。构建包、日志与截图不属于随源码交付的文档资源。
