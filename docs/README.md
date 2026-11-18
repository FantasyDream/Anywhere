# 文档导航

当前源码为 **V0.14 GPUI 开发预览**。产品只有一个 macOS 15+ GPUI 界面实现；
Rust Core 负责 Context、匹配、排序、授权和执行。自动化、构建和物理桌面矩阵
分别记录，Development Preview 不等于正式发行。最后整理：2026-11-18。先从
[项目首页](../README.md)了解功能和平台状态。

## 使用与参与

| 文档 | 用途 |
|---|---|
| [V0.14.0 开发预览](https://github.com/FantasyDream/Anywhere/releases/tag/v0.14.0) | macOS 15+ Apple Silicon 下载、校验和、provenance 与已知限制 |
| [使用指南](usage.md) | 唤醒、权限、Scope、快捷键、搜索、剪贴板、项目工具和本地数据 |
| [开发与构建](development.md) | 环境、App / SDK 构建、测试、生成物和代码入口 |
| [完整测试用例与验收清单](test-acceptance-checklist.md) | 当前GPUI逐项用例、前置数据、优先级、18来源和环境矩阵、自动化追溯及结果模板 |
| [代码规范](code-style.md) | Google / 官方语言规范、隔离工具、自动检查与生成文件边界 |
| [发布检查](releasing.md) | 开发包、版本、签名 / 归档、验收与 GitHub 发布边界 |
| [贡献指南](../CONTRIBUTING.md) | 改动约定、提交与 PR 验证 |
| [更新记录](../CHANGELOG.md) | 用户可感知的变化与工程里程碑 |
| [帮助](../SUPPORT.md) / [安全报告](../SECURITY.md) | 问题报告、敏感信息和维护边界 |

## 当前设计契约

| 文档 | 用途 |
|---|---|
| [仓库阅读入口](../AGENTS.md) | AI / 开发者的代码地图、稳定边界与验证入口 |
| [架构](architecture.md) | Rust Core / GPUI / macOS 薄适配的分工、静态模块、快照、取消与执行 |
| [GPUI 技术方案与验收](mvp-gpui.md) | 当前功能范围、自动化证据、发布检查与物理桌验矩阵 |
| [Context 模型](context-model.md) | 类型、来源、Provider 图、等价与身份 |
| [默认输入](context-input-navigation.md) | 选中文本、15 秒剪贴板、InputMode 和 Esc |
| [Launcher 交互](launcher-interaction.md) | Scope、菜单、二级操作、快捷键和隐藏恢复 |
| [Current App 菜单](current-app-actions.md) | Hidden、定位、重扫、身份和安全降级 |
| [文件搜索](file-search.md) | 范围、权限、稳定连续结果、系统预览和临时存储 |
| [开发动作](developer-actions.md) | Git、项目识别、IDEA 来源和后台进程 |
| [系统预览](native-preview.md) | 按需窗口、空格输入边界、资源租约与跨平台接口 |
| [GPUI UI 设计](ui-design.md) | tokens、页面层级、材质、交互状态与可访问性 |
| [性能](performance.md) | 资源预算、Core 测量、`hotkey_to_visible_ms` 与历史样本边界 |
| [跨平台](platform-portability.md) | 平台能力、路径、快捷键与未完成的 Windows 接入 |

## 插件与后续计划

| 文档 | 用途 |
|---|---|
| [第三方插件开发](plugin-development.md) | 独立模板、SDK、统一样式、权限、打包与更新 |
| [Web 插件架构](web-plugin-design.md) | 包身份、能力代理、Context 输入、生命周期和系统边界 |
| [路线图](roadmap.md) | 后续功能、依赖与 1.0 门槛 |
| [应用历史设计](application-history.md) | V0.14：IDE / 编辑器 / 笔记最近对象与原应用打开；当前已有 18 个配置化来源 |
| [独立浏览器规划](browser-feature.md) | 后续单独实现历史、书签与 Tab 管理；尚未实现，版本待定 |
| [应用历史来源指南](application-history-sources.md) | 内置 / 用户配置、通用读取器、安装默认开关、自定义路径与当前限额 |
| [第三方 Context 调研](research/third-party-context-providers.md) | 已落地的派生类型边界、历史调研与后续进程隔离 |

## 历史与研究

- [Flutter 3.50 与内存调研实施方案](archive/flutter-3.50-memory.md)：界面迁移到 GPUI 前的历史方案、测量与 Flutter / Windows 边界，不描述当前产品。
- [历史验收、迁移与 Review](archive/README.md)：旧 MVP、已完成迁移、实验和逐构建证据。
- [研究资料](research/README.md)：外部产品、开源选型与未落地设计。

旧版本的通过数字、源码行号和本机构建材料只适用于当时环境，不覆盖当前状态。当前行为维护在专题文档；改变功能时同步其指南，避免复制一套版本号命名的使用说明。

阅读实现状态时，使用指南与专题契约说明当前行为，MVP 记录当前构建的验证程度，路线图描述后续范围。研究稿和设计中的“后续 / 验收要求”不能当成已实现接口；SDK 调用与额度还应核对生成的协议和开发指南。

`scripts/check-docs.py` 检查所有随仓库交付的 Markdown（包括根目录、GitHub 模板、SDK 与插件示例）、本地链接、标题、忽略路径和必要社区文件。移动文档时必须同步引用。构建包、日志与截图不属于随源码交付的文档资源。
