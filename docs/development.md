# 开发与构建

首次定位代码先读 [AGENTS.md](../AGENTS.md)；它给出模块入口和稳定边界。详细
契约从[文档导航](README.md)进入。

## 环境

- macOS 15 或以上，以及 Apple Command Line Tools。
- [rust-toolchain.toml](../rust-toolchain.toml) 固定的 Rust / Cargo beta
  预发布工具链，包含 rustfmt 与 Clippy。
- Python 3.10+，用于生成与检查脚本。
- Node.js 22+ / npm，用于 Web SDK 与示例；运行 App 本身不需要 Node。
- 可复用的代码签名身份。

构建使用 Cargo release 产物、原生资源复制和 codesign，不需要 Xcode GUI、
CocoaPods 或其他 UI SDK。GPUI、Quick Look、WebKit 与 AppKit 依赖只在 macOS
构建；共享 Rust crates 另有 Linux / Windows 编译检查。

## 本机构建

```bash
source scripts/env.sh
# 仅首次创建本机签名身份时执行。
scripts/setup-local-signing.sh
scripts/build-macos.sh
scripts/run-macos.sh
```

产物为 `build/Anywhere.app`，Bundle ID 为 `dev.anywhere.launcher`，可执行文件为
`Contents/MacOS/Anywhere`。更新时先退出旧实例。构建默认使用
`Anywhere Local Code Signing`；可用 `ANYWHERE_CODESIGN_IDENTITY` 指定其他身份。

独立输出：

```bash
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-dev.app" scripts/build-macos.sh
scripts/package-macos.sh build/Anywhere-dev.app
```

输出路径必须位于当前 checkout 的 `build/` 内。路径工具拒绝 `..` 越界、父目录
链接逃逸和作为目标的符号链接；构建与清理旧 Bundle 前会复验。

## 检查

首次开发可运行 `scripts/setup-code-style.sh` 安装隔离格式工具；具体规则见
[代码规范](code-style.md)。

```bash
# GPUI、生成本地化、设计与 plist 的快速门禁。
scripts/check-gpui.sh

# 完整工程检查。
scripts/check.sh

# 单独运行 Rust 回归。
cargo test --workspace --locked
```

`scripts/check.sh` 包含生成物漂移、文档链接、架构边界、macOS 菜单、GPUI、
完整 Rust 测试、Clippy、Web SDK 和示例检查。最终提交还要运行：

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

POSIX CI 只编译和测试 `anywhere-core`、`anywhere-platform`、`anywhere-plugin`
与 `anywhere-protocol`。macOS CI 负责 `anywhere-gpui`、完整工程检查和打包；
不能把共享 crate 通过描述成跨平台桌面 App 已交付。

## 生成物

- 本地化：修改 `resources/localization/`，运行
  `python3 scripts/generate-gpui-localizations.py`。
- 设计 tokens：修改 `resources/design-tokens.json`，运行
  `python3 scripts/generate-design-tokens.py`。
- 原生文案：运行 `python3 scripts/generate-native-localizations.py`。
- 文件类型、运行时策略和 Web 协议：运行对应的 `scripts/generate-*.py` 或
  Cargo example。
- Web 协议 schema：
  `cargo run -p anywhere-protocol --features web-schema --example generate_web_plugin_schema --locked`。
- SDK 契约：
  `cargo run -p anywhere-protocol --features web-schema --example generate_web_sdk_contract --locked`。
- 编译内功能目录：
  `cargo run -p anywhere-core --example generate_feature_catalog --locked`。

生成文件不得手改。完整检查会用 `--check` 验证源配置与提交产物一致。

## SDK 与示例

```bash
scripts/check-web-sdk.sh
scripts/check-web-examples.sh
```

模板检查在仓库外解压 SDK、创建临时项目、构建 `.awplugin` 并检查许可证和导出
文档。`check-web-examples.sh` 还运行 Release 下的文本上传、Unicode 分块、
1 MiB 边界、JSON 转义、无效输入和失败后继续写入；使用合成授权，不读取用户
剪贴板或插件目录。

开发指南见[插件开发](plugin-development.md)，运行时边界见
[Web 插件设计](web-plugin-design.md)。

## 测试层次

- Rust Core / Application：确定性模型、顺序、取消、资格和存储回归。
- GPUI：真实 GPUI test support 下的输入、页面路由、设置草稿、窗口事件和
  teardown 状态。
- 生成器：从源文件生成到临时位置并比较；不手写期望生成物。
- App 包：签名、Info.plist、可执行文件、资源和动态库。
- 物理桌面：IME、焦点、材质、状态栏、热键、Quick Look、WebView、登录项、
  Accessibility 和 VoiceOver。

自动化通过不证明物理桌面行为。物理矩阵与当前状态见
[GPUI 技术方案与验收](mvp-gpui.md#物理桌验矩阵)。

## 独立验收数据

显式数据目录必须是绝对路径，不能是根目录或包含 `..`。路径错误或创建失败时
启动失败，不回退到个人数据。显式数据目录不访问系统登录项。

```bash
mkdir -p build/acceptance-gpui
ANYWHERE_DATA_DIR="$PWD/build/acceptance-gpui" \
  ANYWHERE_PREVIEW=1 \
  build/Anywhere.app/Contents/MacOS/Anywhere
```

`ANYWHERE_PREVIEW=1` 只建立无 Core 数据的 GPUI 预览壳并主动显示窗口，用于窗口
与视觉检查；它不能证明 Launcher、文件、剪贴板、插件或权限路径。普通发布包
仍从隐藏窗口和 `ActivationPolicy::Accessory` 启动。

真实 IME、跨应用焦点、Quick Look 与 WebView 检查必须在交互桌面执行。
先记录最小复现序列，再修改生产输入语义；模拟组合键不能替代中文候选与物理
焦点验收。

## 性能与资源

Core 测量、`hotkey_to_visible_ms`、footprint 与 WebKit helper 的证据边界见
[性能](performance.md)。不要用编译耗时、一次 RSS 或单个 worker 资源报告表示
整 App 性能。

## 发布包检查

```bash
scripts/build-macos.sh
codesign --verify --deep --strict build/Anywhere.app
plutil -lint build/Anywhere.app/Contents/Info.plist
otool -L build/Anywhere.app/Contents/MacOS/Anywhere
```

预期主程序为 `Contents/MacOS/Anywhere`，最低系统为 macOS 15.0。产物不得包含
其他 UI framework 或跨语言桥接运行库。ZIP、校验和与发布边界见
[发布检查](releasing.md)。

## 代码入口

| 目录 | 职责 |
| --- | --- |
| `crates/anywhere-core/src/core/` | Context / Action / Search / Ranking 契约 |
| `crates/anywhere-core/src/application/` | 会话、用例、设置、Web 与装配 |
| `crates/anywhere-core/src/features/` | 第一方领域能力 |
| `crates/anywhere-core/src/persistence/` | SQLite 与有界配置读取 |
| `crates/anywhere-platform/` | 系统能力接口与原生适配 |
| `crates/anywhere-plugin/` | 插件发现、验证与文档模型 |
| `apps/gpui/src/` | 产品壳、页面、输入、设置、系统适配与 WebView |
| `packages/plugin-sdk/`、`plugins/examples/` | TypeScript SDK、模板与示例 |
| `resources/` | 本地化、设计参数、Web theme、图标与运行策略来源 |

依赖与并发细节见[架构](architecture.md)。提交约定见
[贡献指南](../CONTRIBUTING.md)。
