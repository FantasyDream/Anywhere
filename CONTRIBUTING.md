# 贡献指南

感谢参与 Anywhere。中文和 English 的 Issue / PR 都可以。

## 开始之前

1. 从 [AGENTS.md](AGENTS.md) 定位模块；阅读[项目状态](README.md#项目状态)、[路线图](docs/roadmap.md)和[架构](docs/architecture.md)。较大的交互、协议或架构调整先开 Issue 讨论。
2. 按[开发指南](docs/development.md)准备工具并构建。普通修改使用已提交的生成绑定；只有接口变化才重新生成。
3. 从最新 `main` 创建工作分支，PR 默认合并到 `main`；专项开发分支另有安排时，在 PR 中明确目标和依赖。

## 实现约定

- 按[代码规范](docs/code-style.md)使用 Google / 官方语言规则，首次开发安装隔离的规范工具。机械格式迁移与功能改动分开提交。
- Rust Core 管模型与算法；Feature 管领域；Application 管编排；Flutter 管呈现；平台专属实现留在适配层。
- 第一方功能通过静态贡献注册；新第三方工具优先使用 [Web SDK](docs/plugin-development.md)，不扩展旧 UI DSL。
- 保持 ActionId / CandidateId、Session / Revision、取消、执行资格、权限和资源预算的既有契约。身份迁移须保留旧数据的恢复路径。
- UI 复用设计 tokens、共享组件与本地化资源；新文案同时提供中英文。外部名称、用户内容和执行地址不翻译。
- 修复增加能复现原问题的回归测试；简单文案 / 样式清理无需堆砌实现镜像测试。实际键盘、IME、焦点与性能结论要注明验证环境。
- 提交 lock 文件及相应生成物，不提交构建包、缓存、本机配置、日志或用户内容。尽量保持一次 PR 聚焦一个目标。

## 提交前

在 macOS 仓库根目录运行：

```bash
scripts/check.sh
git diff --check
```

完整检查包含 Rust、原生 Host、Flutter、SDK / 示例、国际化、生成物、文档和架构规则。环境与较小检查入口见[开发指南](docs/development.md#检查与生成)。

PR 写明改动原因、受影响页面 / 模块、验证命令与结果；UI 变化附脱敏截图。未验证项直接列出，构建成功不能替代实机验收。不要为通过 CI 放宽权限检查、资源限额或忽略失败测试。

文档约定：现行行为维护在专题文档；历史实验、逐构建证据放入 `docs/archive/`；外部选型放入 `docs/research/`。移动文件时更新所有相对链接。

## 交流与安全

遵循[行为准则](CODE_OF_CONDUCT.md)。普通问题使用 Issue；含漏洞、凭据或私人数据的报告遵循[安全说明](SECURITY.md)。项目代码遵循 [MIT](LICENSE)，第三方材料保留其原许可证。
