# Rust / Flutter 稳定工具链与依赖升级（2026-10-29）

基线为 `codex/dependency-upgrades@cc93777`，承接[前一轮 15 个 Dependabot 分支整合](dependency-upgrades-2026-10-29.md)。本轮继续保留在该分支；`main@f1ed6d3` 与此前打包的 Build 35 未更新。

## 工具链与主要依赖

| 项目 | 升级前 | 本轮版本 |
|---|---|---|
| Rust / Cargo | 1.98.1 | 1.99.1 |
| Flutter stable | 3.47.4 | 3.47.8 |
| Dart | 3.13.3 | 3.13.7 |
| TOML | 0.8.23 | 1.1.8+spec-1.1.0 |
| SHA-2 | 0.10.9 | 0.11.1 |
| rusqlite | 0.40.2 | 0.41.1 |
| libsqlite3-sys | 0.38.2 | 0.39.1 |
| zip | 9.0.5 | 9.1.3 |
| Tokio | 1.53.1 | 1.54.1 |
| cc | 1.9.0 | 1.10.0 |
| roxmltree | 0.21.1 | 0.21.2 |
| vector_math | 2.4.2 | 2.4.4 |
| vm_service | 15.3.0 | 15.4.0 |
| Dart web | 1.1.1 | 1.1.2 |
| 后台示例 / 模板 TypeScript | 5.9.3 | 7.0.2 |
| GTS / ESLint 的 TypeScript | 5.9.3 | 6.0.3 |
| i18next | 26.5.4 | 26.5.11 |
| Preact | 11.0.1 | 11.1.0 |
| platformdirs | 4.13.5 | 4.14.0 |

工具链分别经 [Rust 官方发行说明](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/)、Flutter 官方 stable 的实际升级 / `upgrade --verify-only`、版本元数据和 `3.47.8` Git 标签核对。Flutter 版本列表见[官方归档](https://docs.flutter.dev/install/archive)。更新现有干净 SDK，准备 macOS 引擎；未继续准备与本项目无关的移动端组件。Impeller 保持启用。

新增 `rust-toolchain.toml` 固定 Rust 1.99.1 / rustfmt / Clippy，CI 使用同一版本；`.flutter-version` 更新为 3.47.8。另补齐 macOS 打包 job 漏掉的 checkout v7。Rust 解析器本轮共更新 / 新增 60 个包版本；其他兼容的间接依赖以各生态锁文件为准。所有 Rust 直接依赖均核对 crates.io 最新稳定版本，Dart 直接依赖均为 pub.dev 当日最新稳定版；FRB 2.13.0、QuickJS 0.14.0、clipboard-rs 0.3.5、WebView 6.1.5 / macOS 1.1.2 保持当前最新稳定版。

## 兼容修复

- SHA-2 0.11 的摘要数组不再提供原来的十六进制格式化实现。统一使用已在锁文件中的 `hex` 库进行编码，删除两段手写编码循环；新测试锁定已有插件存储 namespace，避免升级后读不到旧数据。文件校验和、历史条目身份及包摘要保持小写、定长十六进制。
- SQLite 0.41 的授权 / 进度设置需要可变连接，列名直接返回迭代器。适配连接和迭代方式，继续保留受检整数功能、只读授权、查询预算与取消。
- Rust 1.99 将 `AtomicU64::fetch_update` 更名为 `try_update`；保持原内存顺序及计数溢出拒绝行为。
- 后台示例与独立模板使用 TypeScript 7；最新 ESLint parser 声明支持 TypeScript `<6.1.0`，隔离规范工具升级到 6.0.3。未用不兼容的 TypeScript 7 编程接口替换该工具依赖。

## 安全检查与上游约束

四个 npm 依赖树审计：SDK、JSON 示例、后台示例与规范工具均为 **0 项已知漏洞**。规范工具原先通过 GTS → Inquirer → external-editor 引入旧 tmp，发现[上游路径穿越通告](https://github.com/raszi/node-tmp/security/advisories/GHSA-ph9p-34f9-6g65)后，给 external-editor 定向指定修复版 tmp 0.2.7。实际临时文件创建 / 正文 / 清理及拒绝越界 prefix 的检查通过，规范工具全量检查也继续验证兼容性；没有使用全局强制升级。

255 个 Rust registry 包的 OSV 查询仍命中 **1 项版本范围告警**：[GHSA-qwgh-2vcv-g2f7](https://github.com/RustCrypto/utils/security/advisories/GHSA-qwgh-2vcv-g2f7)，对应 block-buffer 0.10.4。唯一依赖链为 `flutter_rust_bridge_macros 2.13.0 → md-5 0.10.6 → digest 0.10.7 → block-buffer 0.10.4`，属于编译期宏，不链接为 App 的运行依赖。最新稳定 FRB 仍约束旧 digest，不能直接升级到修复版 block-buffer 0.12.1。

本机检查 0.10.4 源码，其游标是独立 `pos: u8`，与通告描述的“将游标内嵌在数据缓冲区”结构不同，也没有 `ReadBuffer`。使用通告的 EagerBuffer 输入和 panic 回调，捕获 panic 后游标仍为 2，reset / 继续处理通过；未运行 Miri。因此这里记录扫描命中、源代码差异和未复现结果，**不宣称告警已修复或该版本完全安全**，也没有添加忽略规则或修改上游源码。

另有三处上游约束继续保留：

- `generic-array 0.14.7` 由上述编译期链的 crypto-common 0.1.7 精确固定，无法直接升至 0.14.9。
- Flutter SDK 固定 material_color_utilities 0.13.0；最新 0.13.1 不可解析。
- Flutter 测试 SDK 固定 test_api 0.7.12；最新 0.7.15 不可解析。

## 验证

- Rust 全工作区 / 测试目标编译和 Clippy 零警告通过。
- 应用历史回归 Core 42 / MVP 5、持久存储 namespace 回归 1、插件包回归 34，通过。
- SDK TypeScript 检查 / 构建、25 项 SDK 测试、30 项 JSON 测试、真实 QuickJS conformance、SDK 独立模板创建 / 编译 / 打包，通过。
- Flutter 3.47.8 下本地化生成、静态分析和新版 Rust 动态库的实际 Dart FFI 调用通过。
- 完整 `scripts/check.sh` 通过（退出码 0）：Core 322 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471 / 1 跳过、SDK 25、JSON 30；原生 Host、规范、生成物、架构和静态分析均通过。Flutter 跳过项为需要显式动态库路径的 FFI 测试，已单独运行通过。

日志均位于被忽略的 `build/`：`latest-rust-update.log`、`latest-cargo-update.log`、`latest-cargo-check-final.log`、`latest-clippy-final.log`、`latest-flutter-upgrade-final.log`、`latest-flutter-precache.log`、`latest-flutter-version.json`、`latest-pub-upgrade.log`、`latest-pub-outdated.json`、`latest-sdk-check.log`、`latest-examples-check.log`、`latest-bridge-build.log`、`latest-bridge-runtime-test.log`、`latest-audit-*.json`、`latest-rust-osv-audit.json`、`latest-block-buffer-probe.log`、`latest-full-check.log`。

本轮未重新打包或启动 App、未推送或合入 main。自动化结果不替代 GUI / 物理输入法、进程内存、Windows / Linux 原生运行或远端 Actions 验收；已有 Build 35 不含本轮升级。
