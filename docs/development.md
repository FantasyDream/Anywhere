# 开发与构建

## 环境

- macOS 12 或以上，以及 Apple Command Line Tools。
- PATH 上的 Rust stable / Cargo。
- 与 [.flutter-version](../.flutter-version) 一致的 Flutter SDK。
- Python 3.10+，用于生成与检查脚本。
- Node.js 22+ / npm，用于 SDK 类型检查与 Rust 互通测试；App 和插件运行时不需要 Node。
- 可复用的代码签名身份。

构建采用 Flutter assemble、swiftc 和 codesign，生成本机架构的 App，不需要 CocoaPods 或在 Xcode GUI 中打开工程。

## 本机构建

在仓库根目录执行：

```bash
source scripts/env.sh
# 首次开发时创建本机签名身份，已有身份可跳过。
scripts/setup-local-signing.sh
scripts/build-macos.sh
scripts/run-macos.sh
```

产物为 `build/Anywhere.app`。也可从 Finder 打开该 App；更新时先退出旧实例，避免同时运行多个开发包。

构建默认复用 `Anywhere Local Code Signing` 身份，避免每次构建改变签名身份。请保留该钥匙串证书；使用其他身份可设置 `ANYWHERE_CODESIGN_IDENTITY`。本机签名供本地开发使用，对外分发的签名与公证另行处理。

独立构建与打包：

```bash
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-dev.app" scripts/build-macos.sh
scripts/package-macos.sh build/Anywhere-dev.app
```

打包生成 App 对应的 ZIP 和 SHA-256 文件，并检查版本与签名一致性。

## 检查与生成

```bash
# 完整工程检查：Native、Rust、架构规则、Flutter analyze / tests。
scripts/check.sh

# 仅运行 Rust 测试。
cargo test --workspace --locked
```

修改 FRB 公开 API 后重新生成绑定：

```bash
cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked
cargo install cargo-expand --locked
scripts/generate-bridge.sh
```

已生成绑定随源码提交，普通构建不需要生成器。键码、文件类型、运行策略修改后使用对应的 `scripts/generate-*.py` 同步生成代码；完整检查包含漂移校验。

## 生成物与本地化

- FRB 公开 API：`scripts/generate-bridge.sh`。普通 UI / 文案修改不重新生成桥。
- 中英文资源：修改 `apps/flutter/lib/l10n/app_en.arb` 与 `app_zh.arb`，在 `apps/flutter` 执行 `flutter gen-l10n`。
- 原生文案：`python3 scripts/generate-native-localizations.py`，复用 ARB 与原生键目录。
- 设计 tokens、文件类型、键码和资源策略：运行对应 `scripts/generate-*.py`；提交源配置和生成物。
- 编译内功能目录：`cargo run -p anywhere-core --example generate_feature_catalog --locked`。

不要手改生成文件来掩盖源配置差异。完整检查会检测漂移。

## SDK 与示例

```bash
scripts/check-web-sdk.sh
scripts/check-web-examples.sh
```

SDK 类型、测试和独立模板需要 Node.js 22+ / npm；这些检查不安装或批准插件，不读取真实剪贴板。SDK 构建后可在 `packages/plugin-sdk` 执行 `npm pack` 导出。独立插件开发见[指南](plugin-development.md)。

## 测试与测量

- 文档 / 社区文件：`python3 scripts/check-docs.py`。
- 文档检查器：`python3 -m unittest discover -s scripts/tests -p test_docs.py`。
- Flutter：在 `apps/flutter` 执行 `flutter analyze`、`flutter test`。
- 原生：`scripts/check-macos-menu.sh` 和 `scripts/check-macos-host.sh`；依赖 macOS SDK 及 Flutter macOS 引擎资源，首次可运行 `flutter precache --macos`。
- 性能：[测量方法](performance.md)。不要把编译器、单个 worker 或 RSS 的一次快照当作整 App 性能承诺。

先完成代码、自动化与构建，最后做 GUI。真实中文候选、物理组合键、跨应用焦点与 VoiceOver 须单独记录；模拟组合键不稳定时，不据此修改生产按键语义。

完整检查会下载 lock 文件中依赖及固定摘要的原生依赖，需要网络；本地首次构建也可能耗时较长。CI 的 macOS 工作流运行完整门禁；Linux / Windows 工作流验证共享代码，不打包原生 App。仓库文档有独立的轻量工作流。

## 代码入口

| 目录 | 职责 |
|---|---|
| `crates/anywhere-core/src/core/` | Context / Action / Search / Ranking 契约和算法 |
| `crates/anywhere-core/src/features/` | 领域模块、编译期注册、具体服务 |
| `crates/anywhere-core/src/application/` | 会话、用例、装配、设置与 Web 能力代理 |
| `crates/anywhere-core/src/persistence/` | SQLite 和配置适配器 |
| `crates/anywhere-platform/` | OS 能力接口与原生实现 |
| `crates/anywhere-plugin/` | 包校验及旧声明式协议 |
| `crates/anywhere-bridge-flutter/` | FRB API 与 DTO 转换 |
| `apps/flutter/lib/` | 共享页面、交互、设置与 Web 宿主 |
| `apps/flutter/macos/Runner/` | 窗口、权限、热键、焦点和原生材质 |
| `packages/plugin-sdk/`、`plugins/examples/` | TypeScript SDK、模板与普通插件示例 |

依赖与并发细节见[架构](architecture.md)。提交约定见[CONTRIBUTING](../CONTRIBUTING.md)，打包后交付见[发布检查](releasing.md)。
