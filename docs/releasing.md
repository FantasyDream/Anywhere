# 开发包与发布检查

当前为 macOS 15+ GPUI 开发预览。脚本提供本机架构 App、持久本机签名、ZIP 与
SHA-256 校验；正式发行所需的 Developer ID、公证、干净机器升级和完整物理
辅助技术矩阵尚未完成。

## 1. 确认范围与版本

- 核对 PR / commit、工作树和目标分支；不要混入未审查的本机修改。
- 更新 [CHANGELOG](../CHANGELOG.md) 以及[当前验收](mvp-gpui.md)中的范围、证据
  和未验证项。
- Cargo workspace 版本与 `apps/gpui/macos/Info.plist` 的
  `CFBundleShortVersionString` 必须一致；构建号必须是十进制字符串。
- 产品身份固定为：

```text
Name:       Anywhere
Bundle ID:  dev.anywhere.launcher
Executable: Contents/MacOS/Anywhere
Minimum OS: macOS 15.0
```

```bash
python3 scripts/check-versions.py
scripts/check-gpui.sh
scripts/check.sh
```

只有所有必需检查通过后才构建。工程检查不能替代 IME、Accessibility、VoiceOver、
Quick Look、WebView 或性能验收。

## 2. 构建、签名与打包

```bash
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-preview.app" scripts/build-macos.sh
scripts/package-macos.sh build/Anywhere-preview.app
```

构建默认使用 `Anywhere Local Code Signing`，也可通过
`ANYWHERE_CODESIGN_IDENTITY` 指定已有身份。本机签名不是 Developer ID，也不能
代替公证。首次建立身份见[开发指南](development.md#本机构建)。

构建流程：

1. 检查本地化、设计 tokens、entitlements 和 Info.plist。
2. 编译 workspace 版本 `anywhere-gpui` release 二进制。
3. 复制二进制、图标、原生文案与许可证。
4. 写入版本和构建号，签名并执行 deep strict 验证。

打包流程复查 Bundle 与 Info.plist、要求持久签名身份、读取主程序架构、生成
ZIP，并比较归档内 Info.plist、主程序和图标摘要。示例：

```bash
cd build
shasum -a 256 -c Anywhere-preview-macos-arm64.zip.sha256
```

Intel 机器使用实际生成的文件名。当前脚本构建本机架构，不保证 universal binary。

## 3. 包结构检查

```bash
codesign --verify --deep --strict build/Anywhere-preview.app
plutil -lint build/Anywhere-preview.app/Contents/Info.plist
otool -L build/Anywhere-preview.app/Contents/MacOS/Anywhere
```

另外检查：

- `CFBundleIdentifier` 是 `dev.anywhere.launcher`；
- `CFBundleExecutable` 是 `Anywhere`；
- `LSMinimumSystemVersion` 是 `15.0`；
- App 中不存在其他 UI framework、跨语言桥接动态库或旧产品可执行文件；
- 签名身份在重复构建间保持稳定；
- `Contents/Resources/Licenses/` 包含项目 MIT 文本；
- 数据目录仍解析到 `~/Library/Application Support/Anywhere/`。

## 4. 物理桌验

正常退出旧 App，再启动指定新 App，确认实际运行版本。至少执行
[物理桌验矩阵](mvp-gpui.md#物理桌验矩阵)：

- 中文 IME 中文字符、候选窗、Enter、Esc 与方向键；
- 状态栏 `A`、全局热键和主窗口隐藏 / 恢复；
- 独立 Clipboard Shelf 的复用、关闭、失焦与置顶；
- Quick Look 打开、关闭、唤醒与输入焦点恢复；
- WebView 固定 origin、IPC、ack、替换与 teardown；
- Accessibility 权限缺失与授予后的菜单行为；
- 登录项注册、批准与关闭；
- 材质、Reduce Transparency、主题、缩放和布局；
- VoiceOver 主要页面路径。

记录设备、macOS、架构、构建号、数据目录、真实输入与未验证项。只使用脱敏材料；
不要让自动化意外执行真实业务动作。未执行项保持 Unverified。

## 5. GitHub 打包

Actions 中手动运行 **macOS checks**，或：

```bash
gh workflow run macos.yml --ref main
```

PR / push 只执行检查；手动运行在完整检查通过后构建当前架构 App，并上传 ZIP、
SHA-256 和 `package-provenance.json`，保留 30 天。工作流不创建或替换 Release。

仓库 Secrets 需要 `ANYWHERE_CI_SIGNING_P12` 与
`ANYWHERE_CI_SIGNING_PASSWORD`。工作流使用临时 keychain 中的独立
**Anywhere CI Development** 自签身份，运行后清理。CI 开发签名不是
Developer ID / 公证。

## 6. Release

现存 `latest` 是 2026-09-18 的 V0.3 历史预览，不包含当前 GPUI 产品。保留旧
tag 和资产，不用其数据证明当前 macOS 或 Windows 支持。

上传新版时使用包含版本和构建号的不可变 tag，并保留：

- 对应源码 commit；
- 本机架构；
- App ZIP；
- SHA-256；
- 已知限制和物理桌验结果；
- Developer ID / 公证状态的明确说明。

开发预览标为 pre-release。不要上传真实日志、剪贴板数据库、用户路径、签名私钥
或完整 Context。公开正式分发仍需 Developer ID 签名、公证、安装升级、权限身份
和发布安全矩阵；`SECURITY.md` 的私密报告与分支保护也需在 GitHub 设置中启用。
