# Text Pad

无 UI 框架的普通 Web 包，用于交叉验证 Web 工具接入。只请求 SDK ready，不声明 Host 内容或存储权限；页面文字只留在 WebView。

仓库根执行 `python3 scripts/build-web-examples.py`，生成 `build/web-examples/dev.example.text-pad.awplugin`。SDK 构建产物复制到被忽略的 `dist/sdk/`，不手工维护第二份源码。通过 Settings → Features → Web tools → Import 核对摘要后安装，搜索 Text Pad 打开。

本例没有通用 invoke、网络 / 文件 / Shell 能力。格式与批准规则见 [平台设计](../../../docs/web-plugin-design.md)。
