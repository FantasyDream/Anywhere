# V0.12 最终代码与仓库收尾审查

日期：2026-10-23；开发基线：`codex/dev-0.12-i18n`，App 0.12.0 / Build 15。范围包括未提交的国际化 / 交互修复、当前核心边界、构建 / CI、文档与项目规范；历史行为依据见[文档一致性审查](docs-alignment-v0.12-2026-10-23.md)。

## 本轮修复与清理

| 问题 | 处理与依据 |
|---|---|
| 应用目录达到 4,000 个上限后，仍继续识别同层剩余条目 | `catalog::walk` 到达额度后停止；回归验证条目数和实际识别次数均不超过上限 |
| 等待剪贴板设置保存失败时，关闭路径重新拼回英文提示 | SettingsController 使用同一类型化状态的本地化呈现；回归覆盖阻止关闭和中英切换 |
| 旧应用语言读取接口已无生产调用 | 移除 Platform Trait、Rust / C FFI 与 fake 中的旧 hooks；保留 BundleApplication.localized_names 和原生名称 helper 的现代 / 旧 SDK 测试 |
| 旧页脚留下无调用文案 | 删除 entriesCount / copyReturnButton，官方生成物同步；语言切换测试改用正在使用的格式计数文案 |
| WebView 原生框架仍使用 probe 包身份，缓存未包括 Flutter 版本 | 改为正式框架命名；固定 Flutter 版本进入构建缓存摘要，避免 SDK 升级后复用旧 ABI 产物 |
| 干净 checkout 的 Flutter 本地化生成失败 | 删除无消费者、依赖根 build 目录的报告输出；检查器使用真实生产配置和全新临时项目，已先复现失败再验证通过 |
| GitHub macOS 原生回归在旧 SDK 无法编译 | 将测试的 snapshot 禁用 guard 放到测试子类的稳定 Objective-C selector，保留无截图回归，不依赖 Swift SDK 的闭包 actor 标注 |
| JSON 示例锁文件含不可由 GitHub 访问的本机 npm 镜像 | 四个依赖分别核对公共 registry 的相同版本 / integrity 后只替换下载地址；增加来源检查和回归，检查脚本显式使用公共 registry |
| 独立模板 vendor SDK 未复制许可证 | 创建器复制 SDK LICENSE，仓库外模板检查同时比较 vendor 和安装后的许可证 |
| 开发分支 push 与 PR 重复运行相同 CI | push 限定 main，PR 保留完整检查；轻量仓库检查也覆盖依赖来源 |
| 架构说明仍写 Web 首帧使用一次 1×1 快照 | 按 NativeWebAppearance 的实际实现更新：受保护的画布开关与不透明回退，无快照 / 轮询 |

## 边界核对

- Rust 负责 Context、匹配 / 排序、资源和执行资格；Flutter 负责界面、焦点和语言呈现；系统应用名称、窗口、菜单、授权与剪贴板适配留在 Platform / Native Host。
- 第一方使用静态贡献；Web 插件沿包校验、Invocation、受控 Broker 和 SDK 接入。清理未恢复内置热加载、UI DSL 或原生 JSON 工作区。
- 现行文档与历史记录分开，补齐项目许可、贡献 / 安全 / 支持、Issue / PR 模板、依赖维护与文档 CI；贡献基线统一为 main。
- 未将构建包、缓存、日志、数据库或本机签名材料纳入提交。仓库中超过 1 MiB 的交付文件仅有既有应用图标。

## 验证与限制

- `scripts/check.sh` 通过：Rust 567 项通过 / 6 项既有 ignored；Flutter 383；SDK 17；JSON 25。
- 原生菜单 / 权限 / 焦点 / 名称回归、Clippy、架构、设计 / 协议 / 桥接生成物检查通过；484 条中英资源一致。
- 本地化检查在无 build、无 Flutter 项目缓存的临时目录使用生产 l10n.yaml，验证干净检出可生成与源码一致的接口。
- npm 公共来源的 2 项回归通过；SDK / JSON / 仓库外模板检查重跑通过，vendor 和安装后的 SDK LICENSE 均与项目一致。全新临时 npm 缓存下，两份锁文件均从公共 registry 实际安装成功，结束后临时缓存已删除。
- Build 15 构建与交付状态记录在[当前 MVP](../../mvp-v0.12.md)；远端 CI 与合并结果以对应 GitHub PR 为准。
- 本轮自动化与签名构建不代替物理键盘、VoiceOver、完整系统权限 / 安全发行矩阵或 Windows GUI 验收，不扩大正式发布承诺。

main 仍遵守开发预览范围。当前 GitHub 私有仓库套餐的 main 分支保护读取返回 403（需升级套餐或改为公开）；本轮保留私有可见性，使用普通 PR 和实际检查结果完成集成，不声称已配置分支保护。
