# Launcher 隐藏后保留 15 秒

日期：2026-10-12，build 20。

## 操作规则

- 窗口隐藏后开始 15 秒计时；普通 Current App / Universal 唤醒都恢复原页面，不重新采集 Context。
- 保留查询、Scope、菜单位置、选中对象、父页面、文件类型、预览开关与列表位置。未完成查询在恢复时继续或重发，不展示旧输入的结果。
- 恢复取消旧期限；再次隐藏重新计时。达到 15 秒后清空 Launcher 会话，下次打开按入口重新采集环境和内容。
- 明确触发 Global Action shortcut 时仍新采集，避免在旧应用或项目上执行动作；Context 来源设置发生变化也新采集。文件范围设置变化立即撤销旧文件目标。
- 打开设置不重置这段期限；超时只释放 Launcher 状态，设置窗口和未保存草稿保持。应用退出立即结束会话。

## 生命周期与执行

隐藏立即使当前 Session 暂停执行，预览读取和已排队动作使用独立 execution epoch 校验。恢复原 Session 会生成新 epoch，旧排队动作不会因恢复而重新有效。已经启动的后台任务由 ExecutionSessionManager 持有，不受 Launcher 隐藏影响。

Flutter Timer 负责展示状态清理；Core 自己持有同样的超时，检查 Session / epoch / suspended，避免 Flutter 清理迟到或旧定时器伤及新会话。结束时整体替换 Session，释放输入、Context 正文、结果、执行表与容器容量，保留单调递增身份。快捷键设置的动态标签保存在独立最多 256 项的目录中，不保存旧执行目标、路径或脚本正文。

## 验证

- Core 暂停时钟：14 秒恢复、旧 deadline 不清新状态、下次 15 秒到期释放所有临时字段及容量，SessionId 单调递增。
- Core 饱和执行队列：隐藏前排队动作在恢复后仍拒绝执行。
- Flutter：文件查询 / 类型 / 预览 / 父页面在 14 秒恢复，跨普通唤醒入口不新建 Session；超过 15 秒清理并新建 Session。
- Settings：Launcher 状态超时清理，设置页面及未保存草稿保持。

自动化时序覆盖不等于系统休眠、离线卷和真实隐私撤销矩阵已逐项验收。

## 现场版本核对与失焦回归（2026-10-12）

收到“点击桌面后再次唤醒清空”反馈时，运行进程的可执行路径实际为 `build/Anywhere-dev-0.7-build19.app/Contents/MacOS/Anywhere`。其日志显示 08:53:13.264Z 失焦隐藏，08:53:17.660Z 再次唤醒，随后创建 Session 4；08:53:31.616Z 隐藏、08:53:34.539Z 再次唤醒创建 Session 5。间隔未到 15 秒，但 build 19 没有本页描述的保留实现。

确认任务列表为空后退出 build 19，启动 build 20；进程路径核对已切换。新增通过真实 WindowHost MethodChannel 的 `dismiss(generation)` / `toggle(currentApp)` 回归：14 秒恢复同一 Session 和查询，迟到旧 generation 不再隐藏新版 presentation，下一次隐藏 15 秒后新建 Session 且清空输入。该测试及原保留测试通过，Flutter analyze 无问题；未把桌面组合键注入当作真实物理键盘验收。本轮只补测试/记录，build 20 产品二进制无需重建。
