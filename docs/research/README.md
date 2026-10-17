# 调研资料

这里保存日期明确的来源检查与后续方案，以各文档自己的核验日期为准。资料不证明对应库已经使用，也不把作者性能数字或推断当作 Anywhere 同机测量。

| 文档 | 内容 |
|---|---|
| [全功能开源库复用清单](open-source-reuse-audit.md) | 2026-10-17 重新核验；当前功能、可替换边界、接入成本与优先级 |
| [R2 SQLite 临时排序对照](result-store-r2.md) | 2026-10-18 同机原型、顺序 / 取消 / 失败回归及资源测量；保留现有生产后端 |
| [FSearch 评估](fsearch-evaluation.md) | 同名项目、macOS fork / Rust 实现、集成成本与索引取舍 |
| [搜索库补充](file-search-libraries.md) | zlob、ignore / walkdir / jwalk、fff-search 等候选与验证建议 |
| [文件预览](file-preview.md) | Raycast / Alfred / Spotlight 行为和公开 Quick Look 能力 |
| [编辑器 / Agent Bridge](editor-agent-bridge.md) | 当时可观察的 IDEA / VS Code / Agent 来源与后续 Companion 协议 |
| [原生模块 / Web 插件改造](plugin-architecture-native-web.md) | 2026-10-14 代码与官方接口核对；插件清单、自动注册、热加载边界及新 V0.9 建议 |
| [内置功能模块架构](builtin-feature-modules.md) | 最新待确认方案：取消第一方热加载，静态装配、功能策略、服务和原生页面 |

当前生产文件搜索使用 Spotlight + 完整遍历，预览为固定右侧元数据 / 有界文本；详见 [文件搜索](../file-search.md)。当前项目目录来源与未实现部分见 [开发动作](../developer-actions.md)。

研究结论若转为实现，更新当前专题、代码与验收；新的外部版本或性能结论需要另有验证记录。
