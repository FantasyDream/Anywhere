# 文档导航

当前主线：V0.10；整理日期：2026-10-17。根 [README](../README.md) 是使用与开发入口；本目录保留当前设计契约，旧构建证据和调研分别放入 archive / research。

## 当前文档

| 文档 | 阅读用途 |
|---|---|
| [架构](architecture.md) | Rust / Flutter / Host 分工、模块装配、快照、取消和执行 |
| [Context 模型](context-model.md) | 类型、来源、Provider 图、等价与身份 |
| [默认输入](context-input-navigation.md) | 选中文本、15 秒剪贴板、InputMode 和 Esc |
| [Launcher 交互](launcher-interaction.md) | Scope、菜单、二级操作、快捷键和状态恢复 |
| [Current App 菜单](current-app-actions.md) | Hidden、定位、重扫、身份与历史安全降级 |
| [文件搜索](file-search.md) | 查询完整性、权限、稳定连续结果、固定预览和临时存储 |
| [开发动作](developer-actions.md) | Git、项目识别、IDEA 来源和后台进程 |
| [扩展协议](plugin-protocol.md) | 第一方模块、v1 / v2 / v3、Host UI、批准、存储与恢复 |
| [共享 UI](ui-design.md) | 组件、材质、输入与平台回退 |
| [性能](performance.md) | 资源预算、增量更新与测量边界 |
| [跨平台](platform-portability.md) | Host 路径 / 能力、快捷键 schema 与 Windows 接入 |
| [V0.9 MVP](mvp-v0.9.md) | 内置功能模块、Build 6 收尾结果与发行验收边界 |
| [多格式剪贴板改造](clipboard-multiformat-plan.md) | clipboard-rs、格式保真、存储迁移和边界 |
| [V0.10 MVP](mvp-v0.10.md) | 剪贴板历史、存储限额、采集策略、工具页和分步验收 |
| [模块迁移路线图](feature-module-roadmap.md) | 小阶段范围、验收、回滚与续接规则 |
| [V0.8 收尾](mvp-v0.8.md) | 上一阶段完成范围、最终构建证据和发布边界 |
| [路线图](roadmap.md) | 下一阶段依赖、范围取舍和 1.0 门槛 |

## 历史与研究

- [历史验收 / Review](archive/README.md)：指定版本与日期的证据，不是现行操作指南。
- [调研资料](research/README.md)：外部产品、库与未落地设计，结论只对原调研范围有效。

文件名不含 build 号的当前专题是统一依据。旧 MVP 不作为当前实现状态；历史记录中的源码行号只用于还原当时审查。

## 维护方式

- 产品行为变化时更新相应当前专题及 README，避免再创建多个版本号命名的现行指南。
- 逐构建测试、包哈希和现场测量进入 archive；注明版本、环境及未验证边界。
- 调研注明时间、来源、实验范围和是否接入，不把建议写成实现完成。
- 移动 / 删除文档时更新本索引和所有 Markdown 引用；构建产物及本机日志不是仓库随附材料；`scripts/check-docs.py` 已进入门禁，检查路径、标题锚点和忽略的构建材料引用。

本轮已合并 Scope / 菜单草案、预览开关、早期 UI 与性能记录、重复发布说明；Git 中已有的原文可以从相应历史版本恢复。
