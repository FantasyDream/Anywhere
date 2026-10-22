# V0.12 文档与实现一致性审查

日期：2026-10-23；基线：`codex/dev-0.12-i18n` 工作树、App 0.12.0 / Build 14。本次只修改文档，不改变运行行为。后续当前状态以[文档导航](../../README.md)和[当前 MVP](../../mvp-v0.12.md)为准。

## 发现与修正

| 过时或不准确的说明 | 核对依据 | 文档处理 |
|---|---|---|
| JSON 仍被写成整体延期、旧 json.document 像现役能力 | [Web 输入测试](../../../crates/anywhere-core/src/application/web_plugins/input_tests.rs)、[JSON 页面](../../../plugins/examples/json-workspace/src/main.jsx) | 路线图改为 Web 工具已接入；旧类型 / 动作身份明确为兼容保留 |
| 自动填入仅允许非 Text Context，遗漏 Web 命令识别 | [输入策略](../../../crates/anywhere-core/src/application/input_policy.rs)、Web 输入测试 | 使用指南、Context 与架构统一包含已批准 / 启用命令的 inputMatchers |
| 文件页仍禁止裸 Tab、Web 页脚仍写本地处理 | [Launcher 交互](../../launcher-interaction.md)、[Web 页面](../../../apps/flutter/lib/web_plugins/web_tool_surface.dart) | 统一分类切换规则；区分搜索页无通用页脚与 Web 宿主工具选项 |
| 菜单快捷键仍称会显示，历史 IDEA 读数像当前限制 | [候选行](../../../apps/flutter/lib/launcher/widgets/candidate_row.dart)、[菜单契约](../../current-app-actions.md) | 区分保留搜索 / 执行数据与隐藏键帽；将现场读数明确标成历史排障 |
| 架构目录仍指向 plugins/web、persistence/web_plugins，遗漏剪贴板与新工具注册 | [页面装配](../../../apps/flutter/lib/tools/builtin_tool_pages.dart)、[包持久化](../../../crates/anywhere-core/src/persistence/web_packages.rs) | 更新路径、职责表和静态页面组合，保留 Rust / Flutter / Host 边界 |
| SDK 示例使用 theme.subscribe / lifecycle.subscribe，声称支持 navigation.back / actions.set | [公开入口](../../../packages/plugin-sdk/src/index.ts)、[客户端](../../../packages/plugin-sdk/src/client.ts)、[生命周期](../../../packages/plugin-sdk/src/lifecycle.ts) | 使用 definePlugin 示例；明确实际 API、Host 自动主题与宿主操作范围 |
| 更新等待退出编辑、卸载默认保留 KV、存在安装 journal | [包管理用例](../../../crates/anywhere-core/src/application/web_plugins/packages.rs)、[持久化适配器](../../../crates/anywhere-core/src/persistence/web_packages.rs) | 改为目录发布立即撤销旧调用、卸载清理 KV、原子替换收据与失败恢复尝试 |
| 相同 ID 的不同来源更新已能识别发布者并隔离 Usage / 快捷键 | 包管理用例、[Web ActionId](../../../crates/anywhere-core/src/core/action/web_command.rs) | 明确本地安装来源不等于发布者认证；ActionId 仅由包 / 命令 ID 组成，跨发布者隔离未实现 |
| 首次 ready 记 Usage 仍写为建议，HMR / 内存压力回收 / 自动篡改隔离像现成功能 | [调用生命周期](../../../crates/anywhere-core/src/application/web_plugins/invocation.rs)、包持久化与 Web 页面 | 首次 ready 记录写为现有行为；HMR 等单独标未实现，资源摘要拒绝不夸大成持续监控 |
| 路线图仍从已完成的 0.9 / 0.10 / 0.11 开始安排下一步 | [当前 MVP](../../mvp-v0.12.md)、[历史验收](../README.md) | 区分已接入基础、当前 0.12 验收与后续发行 / 平台工作，研究稿保留原日期与历史边界 |

## 验证

- Rust Web 插件用例：36 项通过，包括输入、首次 ready、更新撤销、存储、包校验与 15 秒生命周期。
- SDK 类型检查通过；17 项 SDK 测试全部通过，包含 3 项 Rust 代理互通，没有跳过。
- 文档检查器自身的 3 项测试通过；全仓 Markdown 链接与格式差异检查通过。
- 本次没有重新构建或 GUI 验收；没有用文档修改覆盖 Build 14 尚未完成的真实界面验收状态。

公开分发的安全矩阵、已验证发布者、开发服务器 / HMR、Windows 宿主、第三方 ContextProvider、PTY / Terminal 仍不是本版已交付能力。历史通过数字与外部选型资料保持原日期，不作为本次实机或外网复核结果。
