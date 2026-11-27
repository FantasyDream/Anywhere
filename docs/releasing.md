# macOS 开发包与发布检查

当前为 macOS 15+ GPUI 开发预览。脚本提供本机架构 App、持久本机签名、ZIP 与
SHA-256 校验；正式发行所需的 Developer ID、公证、干净机器升级和完整物理
辅助技术矩阵尚未完成。Windows x64 的 ZIP 构建见 [Windows 指南](windows.md#构建与打包)，运行状态和剩余分发工作见 [Windows 功能进度](windows-progress.md)。

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
4. 写入版本和构建号，签名并执行 deep strict 验证。macOS Release 的 `.app.dSYM` 放在 App 旁边；用 `dwarfdump --uuid` 核对它与主程序的 UUID。用户 ZIP 只包含 App。

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
- 菜单栏搜索图标、全局热键和主窗口隐藏 / 恢复；
- 独立 Clipboard Shelf 的复用、关闭、失焦与置顶；
- Quick Look 打开、关闭、唤醒与输入焦点恢复；
- WebView 固定 origin、IPC、ack、替换与 teardown；
- Accessibility 权限缺失与授予后的菜单行为；
- 登录项注册、批准与关闭；
- 材质、Reduce Transparency、主题、缩放和布局；
- VoiceOver 按 UI-011 验收。开发预览可记录缺口，正式分发需完成辅助技术矩阵。

记录设备、macOS、架构、构建号、数据目录、真实输入与未验证项。只使用脱敏材料；
不要让自动化意外执行真实业务动作。未执行项按清单记录为 Pending 或 Blocked。

## 5. GitHub 打包

Actions 中手动运行 **macOS checks**，或：

```bash
gh workflow run macos.yml --ref main
```

PR / push 只执行检查；手动运行在完整检查通过后构建当前架构 App，并上传 ZIP、
SHA-256、`package-provenance.json` 和独立 `.app.dSYM`，保留 30 天。工作流不创建或替换 Release。

仓库 Secrets 需要 `ANYWHERE_CI_SIGNING_P12` 与
`ANYWHERE_CI_SIGNING_PASSWORD`。工作流使用临时 keychain 中的独立
**Anywhere CI Development** 自签身份，运行后清理。CI 开发签名不是
Developer ID / 公证。

## 6. Release

当前开发预览已经发布：

| 版本 | 日期 | 状态 | 资产 |
|---|---|---|---|
| [`v0.14.1`](https://github.com/FantasyDream/Anywhere/releases/tag/v0.14.1) | 2026-11-24 | Pre-release；macOS 15+ arm64 | `Anywhere-macos-arm64.zip`、SHA-256、`package-provenance.json` |
| [`v0.14.0`](https://github.com/FantasyDream/Anywhere/releases/tag/v0.14.0) | 2026-11-18 | Pre-release；macOS 15+ arm64 | `Anywhere-macos-arm64.zip`、SHA-256、`package-provenance.json` |

`v0.14.1` 使用 annotated tag 固定最终源码，未替换历史 `latest`。现存 `latest`
仍是 2026-09-18 的 V0.3 历史预览，不包含当前 GPUI 产品；保留旧 tag 和资产，
不用其数据证明当前 macOS 或 Windows 支持。

上传新版时使用包含语义版本的不可变 annotated tag；构建号保留在 Info.plist
和 provenance 中。tag 一旦发布就不得移动或重写，并保留：

- 对应源码 commit；
- 本机架构；
- App ZIP；
- SHA-256；
- 已知限制和物理桌验结果；
- Developer ID / 公证状态的明确说明。

开发预览标为 pre-release。不要上传真实日志、剪贴板数据库、用户路径、签名私钥
或完整 Context。公开正式分发仍需 Developer ID 签名、公证、安装升级、权限身份
和发布安全矩阵；`SECURITY.md` 的私密报告与分支保护也需在 GitHub 设置中启用。

发布新版时把 `<version>` 与 `<final-main-commit>` 替换为本次不可变版本和提交。
`v0.14.0` 与 `v0.14.1` 一旦发布，不得重写对应 tag、替换其资产或重复创建
同名 Release。

```bash
VERSION=vX.Y.Z
FINAL_MAIN_COMMIT=<final-main-commit>
git tag -a "$VERSION" "$FINAL_MAIN_COMMIT" \
  -m "Anywhere $VERSION development preview"
git push origin "$VERSION"
gh release create "$VERSION" \
  build/Anywhere-macos-arm64.zip \
  build/Anywhere-macos-arm64.zip.sha256 \
  build/package-provenance.json \
  --title "Anywhere $VERSION — GPUI 开发预览（macOS arm64）" \
  --notes-file "build/release-notes-$VERSION.md" \
  --prerelease
```

发布后重新下载资产并复核 provenance，而不是只核对上传前文件：

```bash
gh release download "$VERSION" --dir /tmp/anywhere-release
cd /tmp/anywhere-release
shasum -a 256 -c Anywhere-macos-arm64.zip.sha256
ditto -x -k Anywhere-macos-arm64.zip app
codesign --verify --deep --strict app/Anywhere.app
```

`package-provenance.json` 的 `archive_sha256` 与 `executable_sha256` 必须分别
匹配 ZIP 和解压后的主程序。
