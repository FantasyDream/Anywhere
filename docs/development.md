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

首次开发先运行 `scripts/setup-code-style.sh` 安装隔离工具；格式与检查方式见[代码规范](code-style.md)。App 构建本身不需要这些规范工具。

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

桥只导出 App 正在使用的接口。设置目录通过 `extensionStatus` / `refreshFeatures` 读取统一状态，包含内置功能和旧声明式插件；旧插件配置的读取与迁移留在 Rust 设置服务。不要在 Flutter 中重新转换一套旧插件状态，测试也注入同一组 DTO。

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

模板检查会在仓库外解压 SDK、创建项目、构建并检查 `.awplugin`，同时核对许可证和导出文档链接。可用 `python3 scripts/check-docs.py --package <解压后的SDK目录>` 单独检查交付文档，避免源码目录中的文件掩盖缺失内容。

## 测试与测量

- 文档 / 社区文件：`python3 scripts/check-docs.py`。
- 文档检查器：`python3 -m unittest discover -s scripts/tests -p test_docs.py`。
- Flutter：在 `apps/flutter` 执行 `flutter analyze`、`flutter test`。
- 原生：`scripts/check-macos-menu.sh` 和 `scripts/check-macos-host.sh`；依赖 macOS SDK 及 Flutter macOS 引擎资源，首次可运行 `flutter precache --macos`。
- 性能：[测量方法](performance.md)。不要把编译器、单个 worker 或 RSS 的一次快照当作整 App 性能承诺。

先完成代码、自动化与构建，最后做 GUI。真实中文候选、物理组合键、跨应用焦点与 VoiceOver 须单独记录；模拟组合键不稳定时，不据此修改生产按键语义。

### 独立实机测量（macOS）

验收实例可显式指定 `ANYWHERE_DATA_DIRECTORY`：必须是绝对路径，不能是根目录或包含 `..`。路径错误或创建失败时退出，不回退到个人数据。该模式不修改系统登录启动项；空目录下历史采集仍默认关闭，文件授权、日志与设置也使用独立目录。

```bash
mkdir -p build/acceptance-v0.13
ANYWHERE_DATA_DIRECTORY="$PWD/build/acceptance-v0.13" ANYWHERE_PROFILE_UI=1 \
  build/Anywhere-0.13-preview-build6.app/Contents/MacOS/Anywhere
```

`ANYWHERE_PROFILE_UI=1` 仅为本机显式诊断：记录操作类别、耗时与结果条数，不记录查询、路径、Context 或剪贴板正文；默认不创建测量任务或上传数据。Launcher / 文件查询测量从提交的输入开始，经过 Rust / Bridge，到匹配当前版本的 Flutter 帧结束；激活还等待原生窗口显示确认，取第一份 ready 快照，不等待迟到 Context；Clipboard History / Shelf 的内部查询分别记录为 clipboard_query / shelf_query，包含原有的 120ms 防抖，取可用列表首帧，不等待预览解码；后台刷新与输入法组合态不计入。每个独立 Flutter engine 通过自己的 Host 通道显式启用，工具窗不可见或退出时撤销在途计时。不包含输入法候选选择时间、GPU raster 或显示器实际呈现延迟。

确认验收实例 PID 后，在另一终端采集；示例 PID 需替换为实际值：

```bash
python3 scripts/measure-desktop.py --pid 12345 \
  --log build/acceptance-v0.13/launcher.log \
  --output build/desktop-measurement.json --duration 300 --interval 5 --footprint
```

采集器仅输出数值汇总：按操作报告实际样本数、p50 / p95 / 最大值，另记主进程与其子进程 RSS、首尾 Physical footprint。样本有上限，隐藏窗口、非法记录和超额样本单独计数；子进程归属只根据 PID 关系，不能冒充完整的 XPC helper 归属。两类内存指标不要混算；隐藏 / 显示、Settings / Shelf 生命周期和长时间趋势应分别记录。原始日志留在被忽略的 `build/`，不要直接提交含真实运行信息的日志。

桌面工具在正常退出后读取 App 状态可能通过 Launch Services 重新启动默认实例，且不会继承终端环境。验收中退出后应使用只读进程检查确认结束；重启必须再次显式传入独立数据目录，再绑定现有实例，避免自动重启触碰个人目录。

完整检查会下载 lock 文件中依赖及固定摘要的原生依赖，需要网络；本地首次构建也可能耗时较长。CI 的 macOS 工作流运行完整门禁；Linux / Windows 工作流验证共享代码，不打包原生 App。仓库文档有独立的轻量工作流。

CI 在 PR 和 main 更新时运行，避免同一次开发分支推送与 PR 重复计费。npm 锁文件只使用公共 registry；新增依赖时显式使用 `--registry=https://registry.npmjs.org`，不要提交本机 / 公司镜像地址。`scripts/check-npm-sources.py` 在下载依赖前检查该约定；依赖内容仍由 lock 的 integrity 验证。

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
