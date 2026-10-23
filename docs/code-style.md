# 代码规范

采用 Google 的对应语言规范；没有独立 Google 工具的语言，使用官方生态工具。规则用于统一手写源码，架构、命名、错误处理和资源边界仍须 Review，自动检查不代表覆盖规范中的每条建议。

## 各语言的约定

| 语言 | 规范与工具 | 项目配置 |
|---|---|---|
| Rust | [Google Rust](https://google.github.io/styleguide/rustguide.html)、稳定版 rustfmt / Clippy | `rustfmt.toml`；100 列，保留现有资源与并发模型，不套用 Chromium 专属限制 |
| Dart / Flutter | [Effective Dart](https://dart.dev/effective-dart/style)、SDK formatter、flutter_lints | `analysis_options.yaml`；导入排序、优先单引号；格式版本跟随锁定的 Flutter SDK |
| TypeScript / JavaScript | [Google GTS](https://github.com/google/gts) 的 ESLint / Prettier 配置 | `tools/code-style/eslint.config.mjs`；SDK 类型感知检查，浏览器与构建 / 测试环境分开 |
| JSX | GTS 的 Prettier 样式，类型 / 业务由构建与测试验证 | `.prettierrc.cjs`；当前 JSX 仅检查排版，不声称覆盖 GTS 的全部语义规则 |
| Python | [Google Python](https://google.github.io/styleguide/pyguide.html)、[YAPF](https://github.com/google/yapf) Google preset | `.style.yapf`；4 空格 |
| Objective-C / C / C++ | [Google Objective-C](https://google.github.io/styleguide/objcguide.html) / [C++](https://google.github.io/styleguide/cppguide.html)、clang-format Google preset | `.clang-format`；2 空格 |
| Swift | [Google Swift](https://google.github.io/swift/)、[swift-format](https://github.com/swiftlang/swift-format) | `.swift-format`；2 空格、100 列；固定工具及兼容依赖版本 |
| Shell | [Google Shell](https://google.github.io/styleguide/shellguide.html)、shfmt / ShellCheck | Bash、2 空格；不为排版要求重写已有工具职责 |

不把 Google 建议扩大成框架迁移或功能改写。改变宽松比较、异步错误处理等语义规则时，先确认原有行为并跑相关回归；无法自动保证的规则通过 Review 检查。

## 安装与检查

仓库根目录执行，需 Node.js 22+、Python 3.10+；macOS 需 Swift 6.0+。首次安装需要网络：

```bash
scripts/setup-code-style.sh
python3 scripts/check-code-style.py
```

开发工具与依赖固定在 `tools/code-style/`，实际安装到被忽略的 `node_modules` / `.tools`，不修改全局包或 Git hooks，也不随 App / SDK 发行。macOS CI 安装同一套工具并检查所有语言；Swift 的完整格式检查在 macOS 执行。

Dart 检查会先解析 Flutter 依赖，再读取包配置进行格式化，确保干净 checkout 与已有工作区使用相同语言版本；不依赖本机残留的 `.dart_tool` 缓存。

```bash
# 应用全部格式化及可自动执行的 lint 修复，然后检查差异与测试。
python3 scripts/check-code-style.py --fix

# 仅检查某语言；可选 dart / web / python / native / swift / shell / rust。
python3 scripts/check-code-style.py --language web

# 完整工程检查，也包含代码规范。
scripts/check.sh
```

Dart 新规则可在 `apps/flutter` 执行 `dart fix --apply --code=directives_ordering,prefer_single_quotes`，再格式化。CI 不自动修改提交内容。

## 生成文件与提交

- 检查覆盖 Git 跟踪和未忽略的新源码。不能因路径含 `dist` 就排除：仓库中的 Web Text 示例包含手写发布入口。
- FRB、本地化、协议、tokens 等生成文件不手工套用其他语言格式器；修改其源配置并运行生成器，完整检查继续复验漂移。Rust 生成物仍遵循已有 rustfmt 流程。
- 规则接入、机械格式迁移、行为修复分开提交。避免在功能 PR 中混入无关文件的全量格式化。
- 不为通过检查删除回归、放宽权限或关闭整套 lint。平台互操作确需例外时，将原因和适用范围写在最小位置。
