# V0.8 MVP — 第一方模块与 Extension v2

2026-10-13。分支 `dev-0.8`，起点 `dev-0.7@3b39763`；开发版本 `0.8.0+1` / Rust `0.8.0-dev.1`。0.7 工程验收完成，正式发布矩阵见 [收尾记录](v0.7-closeout.md)。

## 目标与进度

| 切片 | 必须实现 | 状态 |
|---|---|---|
| S1 模块与协议基础 | 至少两个真实第一方模块、版本化 metadata、宿主能力检查、v1 兼容和迁移回归 | 本轮已实现 JSON / URL，自动化与打包检查见下文 |
| S2 用户控制与生命周期 | 第一方模块状态/开关、外部 v2 能力授权与撤销、设置持久化、重载撤销旧动作/快捷键 | 待实现；当前外部 v2 默认拒绝权限，不能把 manifest 自报能力当作用户授权 |
| S3 扩展服务验证 | 用实际第一方模块验证 CandidateSource / Storage 服务，继续迁移 Git/Developer，冲突/失败/预算与兼容验收 | 待实现；不提前开放任意代码或所有系统能力 |

S1 是可检查的第一批，不代表 0.8 已全部完成。不增加任意 Rust 动态库、Shell/JS runtime、Marketplace、Host UI 或 WebView；V0.9/0.10 的前置条件保持独立。

## 当前代码结构

```text
extensions/
├── json/manifest.toml
└── url/manifest.toml
crates/anywhere-core/src/extensions/
├── mod.rs                 FirstPartyExtension 接口、宿主审批与装配
├── json/                  实际 Provider / Action 实现
└── url/                   实际 Provider / Action 实现
```

第一方 metadata 在编译时嵌入；运行时不读取 checkout 或用户提供的 Rust 二进制。迁出集中装配中的 Provider/Bundle 构建，统一贡献 `FeatureContribution`；旧 features/json 与 features/url 只保留兼容导出。ActionId（core.json.* / core.url.*）、ProviderId、Context 类型、Scope、快捷键和 Usage 继续沿用，不做身份迁移或重复注册。

内部 `FirstPartyExtension` / `FeatureContribution` 是静态 Rust 接口，**不是稳定的外部 ABI**。第一方是受信任的编译代码；窄服务接口便于审查，不等于进程级沙箱。公开声明式协议只描述已有受限操作。

## Manifest 与能力

第一方 metadata：

```toml
protocol_version = 2
id = "json"
name = "JSON"
version = "0.8.0"
capabilities = ["clipboard.write"]
```

外部声明式 v2 继续使用 `commands`；必需数值 `major.minor.patch` 包版本和明确能力清单。当前允许：`clipboard.write`、`url.open`、`file.open`；未知版本、未知/重复能力、缺失声明、非法命令/模板一律拒绝。

```toml
protocol_version = 2
id = "example_copy"
name = "Example Copy"
version = "1.0.0"
capabilities = ["clipboard.write"]

[[commands]]
id = "copy"
title = "Copy input"
accepts = ["core.text"]
action = { type = "copy", text = "{{context.value}}" }
```

- 能力请求与批准独立。Core 的 `register_plugin_with_capabilities` 由宿主传入审批结果；默认加载 v2 给空批准，错误可在既有 Plugins 状态中看到。S2 才接用户授权 UI/持久化，当前不能通过手改 manifest 开启访问。
- ScopedDeclarative 保存不可变批准，并在 HostServices 每次调用时检查；未批准请求不会落到 Platform，也不记录成功 Usage。
- v2 的 `execute_builtin` 暂不开放：它可间接取得菜单/进程等更大能力，不能用一个委托标签绕过细分授权。v1 继续既有委托契约与 Availability/身份复验。
- v1 不新增授权流程、不要求 package version/capability 字段；原始 DTO、解析后 DTO、序列化回读以及 core.json 旧 matcher 继续兼容。legacy 所需操作可以内部推导，保持与 v2 的显式批准模型区分。
- 这些是产品能力令牌，不替代 macOS TCC、目录访问、Session/Revision 或执行前资源复验。

## S1 验收与后续门槛

当前回归：两个第一方模块实际产生 Context/Action/Scope 并执行；v1 身份/匹配/序列化迁移不变；v2 缺失宿主批准时原子拒绝、明确批准后执行、Registry 重载撤销旧候选；执行时即使手工构造缺权 ScopedDeclarative 也不能产生副作用；声明冒用 builtin/未知能力被拒绝。

S2 必须补：授权界面的可理解展示、拒绝/撤销/重启、模块开关失败回滚、插件更新新增能力不自动继承批准、快捷键失效/恢复。S3 必须通过一个真实非文件 CandidateSource 和模块隔离的 Storage，验证预算、生命周期与跨模块隔离后再冻结公开服务协议。当前 Storage / 外部 Provider / CandidateSource 还未作为第三方协议开放。

## S1 检查记录（2026-10-13）

最终 `scripts/check.sh` 通过：**359 Rust / 201 Flutter**，6 Rust ignored；Native menu/Host/目录权限/材质/输入焦点、fmt、Clippy、架构护栏、生成规则与 Flutter analyze 通过。7 项新增 Rust 回归覆盖两模块实际执行、v1 raw/parsed/序列化兼容、v2 metadata 校验、缺权/允许/撤销和执行时服务门禁。

日志：`build/validation/v0.8-s1-check.log`、`v0.8-manifest-roundtrip.log`。开发包 `build/Anywhere-dev-0.8-build1.app` / arm64 ZIP / SHA-256 已生成：四个主二进制 arm64、Impeller、deep/strict 签名、与 0.7 build 22 一致的 designated requirement、ZIP 37 文件/8 链接逐项比较通过；记录 `v0.8-s1-build.log` / `v0.8-s1-package-check.json`。没有启动新版替换当前实例，未进行新的 GUI/系统授权验收。
