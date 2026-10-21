# 历史验收与审查

以下内容保留原日期、版本和实验边界。**不作为当前使用或实现契约**；当前状态从 [文档导航](../README.md) 和 [V0.11 MVP](../mvp-v0.11.md) 阅读。

## 版本记录

| 文档 | 保留价值 |
|---|---|
| [V0.3.x MVP](mvp-v0.3x.md) | 菜单长期身份、历史不误关联、执行不猜测的原验收边界 |
| [V0.4 Context 验收](v0.4-context-acceptance.md) | 来源与系统权限矩阵，工程检查与发布的区别 |
| [V0.5 MVP](mvp-v0.5.md) / [收尾](v0.5-closeout.md) | Developer / Process Session 范围、真实工具与交付证据 |
| [V0.6 MVP](mvp-v0.6.md) / [最终核对](v0.6-final-acceptance.md) | 快捷键、迁移、UI / 平台切片和物理输入待验收 |
| [V0.7 MVP](mvp-v0.7.md) / [收尾](v0.7-closeout.md) | 文件搜索演进，长期索引 / rg 方案被替代的过程 |
| [V0.7 build 14](validation-v0.7-build14.md) | 对应 fixture 与可见 App 的实测边界 |
| [V0.8 开发记录](v0.8-development-records.md) | S1–S3、build 4–16 的回归、包校验、已知实机边界 |
| [V0.9 开发记录](v0.9-development-records.md) | Detail / KeyValue 的协议、执行、Host 回归与开发包 |

## Review 与重构记录

- [V0.11 Build 51 性能与交互收尾](reviews/closeout-v0.11-build51.md)：组词查询、预览复用、复制延迟、设置提示与 JSON 文本复用。

- [V0.11 Build 47 运行时验收](reviews/verification-v0.11-build47.md)：剪贴板异常边界、空设置窗口释放、移除 Web 首帧截图路径。
- [V0.11 Build 36 过期界面资源释放](reviews/memory-v0.11-build36.md)：闲置解码缓存、撤销历史与隐藏帧边界。
- [V0.11 Build 35 内存优化](reviews/memory-v0.11-build35.md)：大文本派生峰值、HTML 字节副本及测量范围。
- [V0.5 可维护性](reviews/review-maintainability-v0.5.md)。
- [V0.7 架构](reviews/review-v0.7-architecture-2026-10-12.md)。
- [Flutter / Rust 边界](reviews/review-flutter-rust-boundaries-2026-10-12.md)。
- [build 21 边界收尾](reviews/refactor-boundaries-build21.md)。
- [V0.8 综合 Review](reviews/review-v0.8-2026-10-13.md)。
- [V0.11 最终边界与分层 Review](reviews/review-v0.11-final-2026-10-20.md)：Build 31，包管理幂等性、损坏资源复用、WebView 超时与识别归属。

Review 的问题列表描述审查基线，修复记录描述其后变更；不能从旧“仍有问题”段落直接推断当前代码仍有该缺陷。菜单永久语义身份仍未验证的限制继续属于 [当前契约](../current-app-actions.md)，没有因归档而取消。

`build/validation`、截图、开发 App 与 ZIP 路径是当时本机材料，不保证另一台机器或 Git checkout 中存在。源码行号标为历史位置，避免跳转到后来版本的不同行。

## 历史基准数据

[基准说明](benchmarks/README.md) 保留长期索引原型与 V2 遍历器的两个 JSON 样本。原型未进入生产，遍历器采样也不包含 Flutter / Spotlight / 原生窗口，不能作为当前整 App RSS。

- [V0.11 架构与收尾 Review（2026-10-20）](reviews/review-v0.11-2026-10-20.md)
