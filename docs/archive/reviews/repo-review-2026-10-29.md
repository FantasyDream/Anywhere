# 全仓分层 Review（2026-10-29）

基线为包含此前各轮修改的未提交工作区，HEAD 为 `215c79190752aea2c9b605d9f047de8a59c7dabb`，源码 `0.14.0+34`。本记录保留当次观察；当前契约以专题文档为准，当前验证程度见 [V0.14 MVP](../../mvp-v0.14.md)。

## 检查范围

覆盖粒度为模块入口、跨层调用与风险路径；生成文件通过生成漂移和编译检查，第三方依赖源码沿用固定版本及现有工程检查。没有把测试通过解释为逐行穷尽全部缺陷。

| 层 | 本轮重点阅读与核对 |
|---|---|
| Core | Context 识别、Action 执行接口、排序 / Usage、后台任务数量与有界输出 |
| Application | Session 快照及执行版本、异步 Source 生命周期、Web 请求 admission / 取消 / 发布、后台解析 |
| Features | 文件工作区与扫描准入、开发项目识别、收藏目录缓存、应用历史和剪贴板资源引用 |
| Persistence | Usage、模块 KV 事务 / 配额、配置原子写入、应用历史有界读取、Web 包资源校验 |
| Platform | 子进程及管道退出、Git argv / 超时、路径与私有存储、应用目录发现 |
| Bridge | 查询、文件、后台任务 DTO 适配及快照传递；本轮没有公开 API 变更 |
| Flutter | Web 资源策略与包格式一致性、异步页面状态及既有列表回归 |
| macOS Host | 设置应用与保存、焦点返回、窗口关闭握手、共享 engine 移动与文件授权 worker |
| 插件 / SDK | 包与授权边界、前台隐藏 / 恢复、后台宿主调用、订阅错误传播及传输配额 |
| 工程与文档 | 构建 / 打包 / 生成脚本、工作流范围、当前说明和历史证据区分 |

## 确认并修复的问题

| 优先级 | 原问题及影响 | 修复与证据 |
|---|---|---|
| P1 | 构建输出仅检查文本前缀，`build/../outside.app` 可通过清理前的路径判断 | 构建 / 打包共用实际路径校验；四组回归覆盖新旧目标、越界、叶链接、父链接及 build 根链接。实际构建 / 打包入口对越界目标提前失败，未执行删除 |
| P2 | Web 请求已取消，仍在等待占满的 worker，保留请求记录及参数直到空位或超时 | 在途取消同时唤醒 admission；阻塞全部 8 个 worker 时，显式取消和隐藏两种场景都无需释放 worker 即完成清理，且没有复制副作用 |
| P2 | 隐藏期间 SDK 订阅注册抛错后，回调仍进入恢复集合 | 仅在绑定成功后登记；合成 40 次失败注册，恢复后的意外回调从 40 次变为 0 次，正常恢复后新订阅仍可使用 |
| P2 | 后台 SDK 的一个订阅回调抛错，使已成功的输入写入返回失败并阻断后续监听器 | 每个回调独立处理异常；受控写入只发生一次，Context / Input 的其他监听器均收到更新 |
| P2 | 校验通过的包内 TTF / OTF / ICO 在 Flutter 资源适配层被拒绝 | 补齐被动资源类型；文档入口、路径、CSP 与 Manifest PNG 图标约束继续单独校验 |

构建路径问题来自原清理判断的源码审查，没有执行旧脚本的危险删除分支。其他问题均先由新增回归复现，再修改实现。错误输入框等此前 Flutter 测试记录不作为本轮证据。

## 验证

- Rust Web / 后台相关定向测试：54 项通过。
- SDK 前台、后台、客户端及真实文档启动传输：21 项通过。
- Flutter Web 文档策略：8 项通过。
- 构建路径边界：4 项通过；构建 / 打包入口对越界目标均返回失败。
- 完整 `scripts/check.sh` 退出码 0：Core 321 / 1 忽略、MVP 195 / 4 忽略、Platform 70 / 1 忽略、Flutter 471、SDK 25、JSON 30；Clippy、原生 Host、架构、生成物与静态分析通过。原有忽略项保留，没有通过关闭检查规避问题。

原始日志留在被忽略的 `build/`：`repo-review-rpc-before.log`、`repo-review-rpc-after.log`、`repo-review-sdk-before.log`、`repo-review-background-before.log`、`repo-review-sdk-final.log`、`repo-review-web-policy-before.log`、`repo-review-web-policy-after.log`、`repo-review-web-core-final.log`、`repo-review-invalid-output.log`、`repo-review-invalid-package.log`、`repo-review-full-check.log`。

## 验收边界

本轮没有重新打包或替换正在运行的 App，没有进行 GUI / 物理输入法、整 App 帧时间、RSS / physical footprint 或长期泄漏验收。取消无法中断已经开始的同步 OS I/O，其许可仍到实际返回才释放。Windows / Linux 原生 App、公开发行与完整辅助功能矩阵仍按 MVP 列为待验收。
