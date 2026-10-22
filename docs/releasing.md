# 开发包与发布检查

当前为 macOS 开发预览。脚本提供本机架构 App、持久本机签名和 ZIP 校验，未提供完整的 Developer ID 公证、自动更新或全平台正式发行流程。

## 1. 确认范围与版本

- 核对 PR / commit、工作树和目标分支；不要把未审查的本机修改混入发行包。
- 更新 [CHANGELOG](../CHANGELOG.md)、当前专题与验收记录，明确新增、修复和未覆盖边界。
- Rust workspace 发布版本、Flutter `pubspec.yaml` 和原生 `Info.plist` 必须一致；Flutter 与原生构建号一致。
- JSON 等 Web 包有独立版本与摘要，不能把 App 更新当作已安装插件的自动更新。

```bash
python3 scripts/check-versions.py
scripts/check.sh
```

所有已知必需检查通过后再构建。不要将工程检查通过表述为完整的 IME、权限、VoiceOver、Windows 或性能验收。

## 2. 构建、签名与打包

在仓库根目录执行，选择明确的产物名：

```bash
ANYWHERE_APP_OUTPUT="$PWD/build/Anywhere-preview.app" scripts/build-macos.sh
scripts/package-macos.sh build/Anywhere-preview.app
```

构建默认使用 `Anywhere Local Code Signing`，可由 `ANYWHERE_CODESIGN_IDENTITY` 指定已有身份。本机签名不是公证；不要因为缺少开发签名而改为每次变化的 ad hoc 身份。首次建立身份按[开发指南](development.md#本机构建)操作。

构建将项目 MIT 与补充第三方许可放入 `Contents/Resources/Licenses/`。WebView 框架使用锁定依赖及验证摘要的源码；打包器复验签名和版本，对比归档内主程序、Info.plist 与 Rust 库的字节，并生成 `.zip.sha256`。

```bash
cd build
shasum -a 256 -c Anywhere-preview-macos-arm64.zip.sha256
```

示例为 Apple Silicon；Intel 使用对应文件名。打包脚本产出实际名称，不保证 universal binary。

## 3. 交付前验证

正常退出旧 App，再启动指定的新 App，确认实际运行版本。至少按改动范围检查：唤醒与输入、已有应用切回、Scope / 更多操作、文件查询与固定预览、剪贴板多格式恢复、设置保存与多窗口、Web 更新 / 撤销和隐藏 15 秒恢复。

记录设备 / 系统、构建、使用的数据范围、真实输入与模拟输入，以及未验证项。只使用脱敏验收内容，先完成单元与构建再进行 UI；不要让 UI 自动化意外执行真实业务动作。

## 4. GitHub 发布

代码合并、打 tag、上传 Release 是独立操作，按维护者确认的版本执行。开发预览应标记为 pre-release，说明平台 / 架构、安装及权限需求、已知限制和校验方法；上传对应 ZIP 与 SHA-256，不附真实日志、剪贴板数据库或本机签名材料。

公开正式分发还需单独完成 Developer ID 签名 / 公证、干净机器安装升级、权限身份、辅助功能与发布安全矩阵；当前 CI 不自动完成这些步骤。`SECURITY.md` 的私密报告按钮、分支保护和必需状态检查需由仓库维护者在 GitHub 设置中启用，提交配置文件不会自动开启。
