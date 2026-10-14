# V0.9 MVP — Host-rendered Plugin UI

主线 `dev-0.9`，源版本 Flutter / App `0.9.0+1`、Rust `0.9.0-dev.1`。起点为 [V0.8 工程收尾](mvp-v0.8.md) 的提交 `3e5a06a`，当前只完成 S1，**V0.9 尚未收尾**。

## 目标与分工

让插件以版本化数据描述简单界面，继续使用 Context → Action → Candidate → 执行链路。Rust 负责描述校验、内容插值、权限与执行；Protocol / Bridge 传输有界 DTO；Flutter 负责现有 Launcher 内的主题、布局、焦点与返回。不增加脚本运行时、任意 Widget 或 WebView。

## 切片

| 切片 | 交付范围 | 状态 |
|---|---|---|
| S1 Detail / KeyValue | v3 协议、ui.render、结构化文本视图、真实 TOML 示例、返回 / 生命周期 | 已实现，自动化证据见下方 |
| S2 Form | 有界字段、输入状态、Rust 校验、受限提交、失败保留输入、取消 / 返回 | 未实现，下一步 |
| S3 List | 稳定行 ID、键盘选择 / 过滤、有界分批数据与连续 UI、明确定义默认 / 二级动作 | 未实现；复用已有稳定列表，不重新发明搜索导航 |
| S4 Markdown | 受限语法、长度 / 链接策略，主题与读屏 | 未实现；禁用 HTML / 脚本，外链经过批准操作 |
| S5 收尾 | 至少一个完整表单工具、撤销 / 更新矩阵、实机交互和资源基线 | 未完成 |

Form 提交必须复用当前 Session / Revision / Candidate / catalog / grant 资格，校验不产生副作用；成功副作用只记录一次 Usage。List 操作必须绑定稳定行身份与当前视图版本，不能根据标题或旧下标猜测目标。

## S1 使用

安装 [Context Detail 示例](../plugins/examples/context-detail.toml) 到插件目录，在 Settings → Extensions Reload、Review permissions、Allow & enable。示例不自动安装、不影响现有配置。输入或捕获文本、URL、JSON、路径 Context 后选择 Inspect Context；插件 Scope 也可筛选这条动作。

Detail / KeyValue 共用 Launcher 材质和 AppStyle；标题、标签、正文有明确层级。`↑↓` / `PageUp` / `PageDown` 滚动，`Esc` 或 Back 返回原查询、Scope 和候选。隐藏保持 15 秒，过期或配置 / 授权变化后失效。内容只是纯文本，可选择复制；没有自定义回调或表单提交。

## 已有验证

- Manifest 回归：v1 / v2 拒绝 UI，v3 必须声明能力，未知字段 / schema / 类型 / 模板变量拒绝；模板与展开后的行数 / 内容限额分别验证。
- 执行回归：默认不授予 ui.render，直接 DTO 不能绕过 grant，普通执行返回结构化数据，超限失败不关闭窗口，旧 Revision / catalog 候选不能执行。
- 生命周期回归：修改 UI 定义即使不改变包版本也需重新批准，Reload / Revoke 使旧会话与候选失效。
- Host 回归：payload 必须匹配视图，迟到结果不打开已关闭窗口，错误保持查询；Esc 后搜索框恢复键盘输入，15 秒内恢复 / 到期重置，纯键盘滚动与裸 Tab 禁用。

完整 `scripts/check.sh`：**411 Rust / 238 Flutter 通过，6 Rust ignored**；Native、fmt、Clippy、架构 / 生成规则、文档与 analyze 通过。记录 `build/validation/v0.9-s1-check-final.log`。

当前定向日志位于本机 `build/validation/v0.9-ui-rust-tests.log` 与 `v0.9-ui-flutter-tests.log`，不是仓库随附材料。尚未进行最终 App 的实机插件安装 / 审批 / 返回矩阵，不能将 Widget 或 TestPlatform 测试写成实机验收。

## 开发包

`build/Anywhere-dev-0.9-build1.app` 与对应 arm64 ZIP 已生成；37 文件 / 8 链接逐项一致，四个主二进制为 arm64，Impeller 与 deep / strict 签名有效，长期 signing requirement 与 V0.8 Build 16 相同。记录 `build/validation/v0.9-s1-package-check.json`。使用时指定完整版本化 App 路径，不使用可能滞后的默认 Anywhere.app。

## 发布边界

S1 是第一个 Host UI 开发切片，不宣称 List / Form / Markdown 已完成。Windows 实机、辅助功能、长文本 / 帧时间 / 整 App RSS、正式签名 / 公证、升级与完整权限矩阵仍需验证。详见 [路线图](roadmap.md) 和 [协议](plugin-protocol.md)。
