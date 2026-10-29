# 依赖升级整合（2026-10-29）

升级基线为 `main@f1ed6d3`（`0.14.0+35`）。按用户要求，合并与验证保留在 `codex/dependency-upgrades`，不更新 main 或正在运行的 Build 35，也不推送远端。15 个 Dependabot 分支均以保留历史的方式合入该升级分支，旧分支中的过时配置通过当前解析器重新生成锁文件。

## 升级范围

| 依赖 | 升级前 | 升级后 |
|---|---|---|
| cc | 1.4.6 | 1.9.0 |
| libc | 0.2.189 | 0.2.191 |
| roxmltree | 0.20.0 | 0.21.1 |
| rusqlite | 0.32.1 | 0.40.2 |
| actions/cache | v4 | v6 |
| actions/checkout | v4 | v7 |
| 插件 SDK TypeScript | 5.9.3 | 7.0.2 |
| Acorn | 8.15.0 | 8.19.0 |
| globals | 14.0.0 | 17.13.0 |
| clang-format | 19.1.7 | 23.1.3 |
| platformdirs | 4.4.0 | 4.13.5 |
| shfmt-py | 3.12.0.2 | 4.2.0 |
| cupertino_icons | 1.0.8 | 2.0.0 |
| flutter_lints | 5.0.0 | 6.0.0 |
| Flutter Rust Bridge | 2.11.1 | 2.13.0 |

Flutter SDK 保留 3.47.4，Impeller 保持启用。FRB 的 Rust 运行库及生成器随 Dart 端一起升至 2.13.0；生成器和 Python / Node 规范工具安装在项目隔离目录，未替换全局环境。

## 实际发现与处理

- **SQLite 受检整数转换改为显式功能。** 初次全工作区编译报 `usize` 的 `ToSql` / `FromSql` 缺失。保留现有 `bundled`、`hooks`、`limits`，补充新版 `fallible_uint`，继续使用库提供的受检转换，没有用截断强转替代额度检查。
- **SQLite 保护设置变为可失败操作。** 编译发现五处忽略的 Result，涉及输入长度、SQL 长度、表授权和进度取消回调。全部传播错误，设置失败时拒绝读取；应用历史取消、拒绝访问、XML / SQLite 适配回归继续执行。
- **FRB 分支只升级了 Dart 依赖。** 同步 Rust 运行库，使用同版生成器重新生成两端绑定并编译动态库。新增四处版本一致性检查及生成器启动保护，三项回归覆盖一致、任一端过期和元数据缺失；真实动态库通过生成 Dart API 返回 `pong`。
- **clang-format 23 排版规则变化。** 修正两个 Objective-C 函数指针类型的空格，保持原生行为。
- **TypeScript 7 与工具链分工。** SDK 的检查、声明生成和构建使用 7.0.2；GTS / ESLint 工具仍使用自身 5.9 编程接口。官方 [TypeScript 7 说明](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)描述其编程接口限制，未把隔离工具也强行升级到同一版本。

FRB 的版本与生成边界参考[官方 2.13.0 发行记录](https://github.com/fzyzcjy/flutter_rust_bridge/releases/tag/v2.13.0)。Actions 的变化已核对 [checkout](https://github.com/actions/checkout) 和 [cache](https://github.com/actions/cache) 官方说明；远端 runner 的真实执行结果仍需单独记录。

## 验证证据

- 应用历史定向回归：Core 42、MVP 5，通过。
- 升级后的 SDK TypeScript 检查 / 构建及新规范工具检查通过。
- 真实 Rust 动态桥接库构建完成，Dart FFI 定向回归 1 项通过；不初始化个人数据或原生窗口。
- 版本一致性回归 3 项通过；四处版本均为 2.13.0。
- 完整 `scripts/check.sh` 通过：Core 321 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471 / 1 跳过、SDK 25、JSON 30；Clippy、原生 Host、架构、生成物、规范和静态分析通过。Flutter 跳过项为需要显式动态库路径的 FFI 测试，已在上述独立运行中通过。

日志保留在被忽略的 `build/`：`deps-full-check.log`（最终完整检查）、`deps-native-bridge-build-final.log`、`deps-bridge-runtime-test-final.log`、`deps-bridge-generation-final.log`、`deps-history-tests.log`、`deps-cargo-check.log`（升级初次错误）、`deps-python-tools.log`、`deps-style-node.log`、`deps-flutter-analyze.log`、`deps-web-style.log`。

本轮验证依赖解析、编译、类型检查、实际 FFI 及现有回归，不包含新 App 启动、GUI / 物理输入法、进程内存、Windows / Linux 原生 GUI 或远端 GitHub Actions 实跑。当前正在运行的 Build 35 仍对应升级前源码。
