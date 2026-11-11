# 第一方模块与第三方 Web 插件：调研结论

核对起点：2026-10-14，代码基线 `dev-0.9@e4dbedc`。本页保留当时的取舍依据；
当前结构已经实施，现行契约见[架构](../architecture.md)和
[Web 插件设计](../web-plugin-design.md)。最后校准：2026-11-11。

## 已确定边界

- 第一方能力编译进 Rust Core，通过 FeatureDescriptor、Provider、ActionBundle、
  CandidateSource 和 typed HostView 贡献；没有原生热加载。
- GPUI 是唯一产品 UI。新第一方工具使用已知 `HostView::NativeTool` ID 和 GPUI
  页面，未知 ID 明确拒绝。
- 第三方使用本地 `.awplugin`，WebView 承载 UI，按需 QuickJS 承载有限后台任务。
- 两类能力共用 Rust Context、Action、Candidate、Ranking、Usage、Storage 与执行
  资格，但安装、信任、资源和生命周期边界独立。

## 当时放弃的方案

调查比较了：

- 通用 FirstPartyExtension runtime 和运行期重建；
- Form / List / Markdown 外部 UI DSL；
- 每个插件常驻独立页面进程；
- 用通用窗口 / 热键 / 托盘库替换全部产品生命周期；
- 把成熟库直接当作完整 Launcher 或安全沙箱。

这些方案要么重复 Context / Ranking / 授权，要么扩大 UI 协议面，要么无法证明
焦点、IME、辅助功能和资源释放。因此停止扩展外部 UI DSL，第一方转为静态贡献，
第三方 UI 改为固定 SDK 的 Web 页面。

## 实施结果

| 区域 | 当前实现 |
| --- | --- |
| 第一方注册 | Feature catalog 编译期生成，启动绑定一次 |
| 页面路由 | GPUI typed `HostView`，保留父查询和选择 |
| 旧 TOML | 兼容发现、批准与受限动作，不扩展 UI DSL |
| Web 包 | Manifest、resource hash、grant、review token 与安装记录 |
| Web UI | 固定 origin WKWebView、自动主题、版本化 IPC |
| 后台任务 | 按需 QuickJS、同一 Rust broker，当前进程内执行 |
| 生命周期 | Rust-owned document handle、generation、ack、cancel 与 teardown |

Web 页面不能直接取得文件或进程权限。第三方 Context 只能生成
`plugin.<package-id>.*`，脚本数据不转换为原生文件 authority。

## 留下的工程经验

1. 业务规则和 UI 状态分离，避免每个 UI 框架重写 Context / Ranking / 授权。
2. 安装批准覆盖实际资源摘要；更新资源需要重新审查。
3. Host 请求同时绑定包、命令、grant、document generation 和当前 Session。
4. 旧页面迟到 IPC、ack、状态、崩溃或关闭回调不得操作新文档。
5. 异步 I/O 的取消通知不能提前释放正在使用的 admission。
6. GPUI 只负责页面和焦点；WebKit、Quick Look、状态栏等由薄系统 adapter 管理。
7. 自动化只能证明接口和状态顺序，物理 WebView、IME、焦点与 helper 回收另做桌验。

实现步骤见[插件开发](../plugin-development.md)，验证状态见
[GPUI 技术方案与验收](../mvp-gpui.md#物理桌验矩阵)。
