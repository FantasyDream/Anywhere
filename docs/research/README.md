# 调研资料

这里保存日期明确的来源检查与后续方案，以各文档自己的核验日期为准。资料不证明对应库已经使用，也不把作者性能数字或推断当作 Anywhere 同机测量。

| 文档 | 内容 |
|---|---|
| [全功能开源库复用清单](open-source-reuse-audit.md) | 2026-10-17 重新核验；当前功能、可替换边界、接入成本与优先级 |
| [R2 SQLite 临时排序对照](result-store-r2.md) | 2026-10-18 同机原型、顺序 / 取消 / 失败回归及资源测量；保留现有生产后端 |
| [R3 文字匹配对照](matcher-r3.md) | 2026-10-18 同机候选 / 等级 / 性能比较；去除原生热路径分配，nucleo 仅留测试 |
| [FSearch 评估](fsearch-evaluation.md) | 同名项目、macOS fork / Rust 实现、集成成本与索引取舍 |
| [搜索库补充](file-search-libraries.md) | zlob、ignore / walkdir / jwalk、fff-search 等候选与验证建议 |
| [文件预览](file-preview.md) | Raycast / Alfred / Spotlight 行为和公开 Quick Look 能力 |
| [编辑器 / Agent Bridge](editor-agent-bridge.md) | 当时可观察的 IDEA / VS Code / Agent 来源与后续 Companion 协议 |
| [原生模块 / Web 插件改造](plugin-architecture-native-web.md) | 2026-10-14 历史方案；第一方 runtime 与原排期已废弃，Web 新方案见下文 |
| [内置功能模块架构](builtin-feature-modules.md) | 最新待确认方案：取消第一方热加载，静态装配、功能策略、服务和原生页面 |

当前生产文件搜索使用 Spotlight + 完整遍历，预览为固定右侧元数据 / 有界文本；详见 [文件搜索](../file-search.md)。当前项目目录来源与未实现部分见 [开发动作](../developer-actions.md)。

研究结论若转为实现，更新当前专题、代码与验收；新的外部版本或性能结论需要另有验证记录。

2026-10-18 的 [第三方 Web 插件设计](../web-plugin-design.md)是当前待实施提案：Terminal 延后；本地 Web 包、系统 WebView、SDK、Context / 能力隔离与安装更新按 P0–P6 推进。旧研究的历史代码说明不能当作现状。

- [第三方 Context 注册与运行方案](third-party-context-providers.md)：2026-10-20，类型与权限隔离、统一 Graph、QuickJS / Wasm 比较及分步验证；尚未接入。

- [中英国际化与拼音开源选型](i18n-pinyin.md)：Flutter 官方方案、Rust pinyin 与 Web 资源边界。
