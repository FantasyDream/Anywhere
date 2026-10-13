# 调研资料

这里保存日期明确的来源检查与后续方案，**未在文档整理时重新联网核验**。资料不证明对应库已经使用，也不把作者性能数字或推断当作 Anywhere 同机测量。

| 文档 | 内容 |
|---|---|
| [FSearch 评估](fsearch-evaluation.md) | 同名项目、macOS fork / Rust 实现、集成成本与索引取舍 |
| [搜索库补充](file-search-libraries.md) | zlob、ignore / walkdir / jwalk、fff-search 等候选与验证建议 |
| [文件预览](file-preview.md) | Raycast / Alfred / Spotlight 行为和公开 Quick Look 能力 |
| [编辑器 / Agent Bridge](editor-agent-bridge.md) | 当时可观察的 IDEA / VS Code / Agent 来源与后续 Companion 协议 |

当前生产文件搜索使用 Spotlight + 完整遍历，预览为固定右侧元数据 / 有界文本；详见 [文件搜索](../file-search.md)。当前项目目录来源与未实现部分见 [开发动作](../developer-actions.md)。

研究结论若转为实现，更新当前专题、代码与验收；新的外部版本或性能结论需要另有验证记录。
