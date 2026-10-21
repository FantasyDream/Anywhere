# Web 插件 P0 探针

独立开发验证工程，采用 `flutter_inappwebview 6.1.5` / macOS implementation `1.1.2`。复现 Anywhere 的 Flutter assemble + 手工 Swift Host 打包方式，验证可复用库能否嵌入当前宿主技术栈。尚未接入正式 Launcher 的插件安装或 SDK。

## 运行

在本目录执行 `flutter pub get --enforce-lockfile`、`flutter analyze`、`flutter test`。在仓库根执行：

```sh
python3 scripts/build-web-plugin-probe.py
open build/WebPluginProbe.app
```

要求仓库指定 Flutter 版本、macOS 开发工具及既有持久签名身份。构建脚本复用 pub 锁定依赖，从固定版本和 SHA-256 验证的 OrderedSet 源码构建静态依赖；补齐通常由 CocoaPods 生成的 umbrella module / 原生注册，不安装全局 CocoaPods，也不改 pub-cache 源码。产物有独立 Bundle ID，不加载正式 App 数据。

`WebPluginProbe --self-test` 在页面完成合成检查后退出；外部测试进程必须设超时，并解析 `CHECKS:` 字段。退出本身不等于检查全部通过。

## 验证边界

- 本地自定义协议只提供两个固定资源，无任意磁盘读取或远程页面。
- 只有 `probe.echo` 方法；真实剪贴板、前台应用、Rust / FRB 和进程能力均不接入。
- 采用 `WebMessageListener`，读取原生 `securityOrigin` / `isMainFrame`。稳定版对自定义协议只接受 scheme 级规则，Dart 接收层继续严格验证实际来源；不使用通配来源或页面自报的 URL。
- 每个绑定只允许一次入口导航，重载创建新视图与 epoch。配有 Unicode、来源 / frame、旧实例、超额、重复请求和非法方法测试。
- Reload / Dispose、Host Actions 和 15 秒失焦保留只用于 P0 生命周期验证。原生 `⌘K` / Esc 路由、真实输入法、资源释放和辅助进程内存仍需逐项实机验收。
- CSP 与内容规则阻止 HTTP(S) 资源。`connectBlocked` 只是当前 fetch 失败的观察，**不证明所有网络通道已隔离**；正式安装第三方代码前仍须受控网络端点、导航 / frame / 资源攻击矩阵。
- 探针的桥校验不是正式 Rust 能力代理。后续生产接入沿用 [设计方案](../../docs/web-plugin-design.md)，不把探针回显器升级成通用后端调用代理。

当前只构建 macOS；Windows 的 API / 键盘 / profile / WebView2 Runtime 尚未实机验证。所有 UI 测试最后进行，先完成代码、静态分析及单元测试。

## 2026-10-18 验证记录

- 探针 `flutter analyze` 无问题，7 项测试通过；生产项目 `scripts/check.sh` 通过（487 Rust / 273 Flutter，6 项已有 Rust ignored）。
- 独立 Release 包已构建，deep / strict 签名验证通过；保持 Impeller。
- 单元 / 静态检查后实际启动：本地页面加载、原生来源 / 主 frame 接收检查、Unicode 回显和未开放方法拒绝通过。合成报告为 `echo=true, epoch=1, denied=true, connectBlocked=true`。
- `connectBlocked` 仅是 fetch 被拒绝的观察，不作全面网络隔离结论。真实键盘 / IME、15 秒实机恢复、长时间进程组内存及完整攻击矩阵仍待验证，P0 尚未整体验收。
- 本机日志：`build/web-plugin-probe-build.log`、`build/web-plugin-probe-self-test.log`、`build/web-plugin-probe-regression.log`；均为忽略的本机材料，不随源码交付。
