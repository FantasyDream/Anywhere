# 文件结果稳定顺序与固定预览 — build 22

日期：2026-10-13。版本：`0.7.0+22` / Rust `0.7.0-dev.22`，分支 `dev-0.7`。

## 交互

- 相同查询与类型下，已经展示的文件保持位置。Spotlight / 遍历后续批次即使发现更高排名结果，也只把新目标追加到已有行后面，不再替换并重排整个列表。
- 首轮自动追加最多 64 项；滚动或 ↓ 继续按现有连续加载机制追加，不分页、不遗漏可加载的剩余命中。用户已经操作列表时继续保留已展示行及选择，进度更新不挪动行。
- 修改关键词、文件类型、搜索范围或重新开始查询才重新取得排序。Registry / 权限撤销可以删除无资格目标；不存在的文件执行仍失败，不能以“稳定列表”跳过执行复验。
- 文件工具右侧固定显示预览，空查询/无结果时显示提示；选中文件后自动加载，120ms 防抖、迟到响应隔离继续生效。操作面板预览保持原文件，选择 Copy/Open 等操作不会把预览改成操作行。
- 没有 Preview Action、开关或关闭按钮。⌘K 仅打开文件操作；Esc 从操作层返回结果，从文件结果清空查询，再按一次返回上级。焦点在可选预览文字时使用同样的 Esc 路径。

## 布局

文件工具请求 **920×600** 的窗口；在空状态、结果加载、操作面板间保持尺寸。返回主搜索恢复 **648** 的原宽度和原高度逻辑。Native 根据屏幕可用空间约束窗口，并在宽度改变时保持水平中心；Flutter 预览宽度按可用宽度分配约 38%，最大 360、最小 220，窄屏可继续显示结果和预览。背景材质、行/按钮/Scope 样式沿用已有实现。

窗口大小来自 Flutter 的展示状态，通过平台无关 DesktopHost 的 width/height 端口传递；Rust 不决定像素。排序稳定性由 Rust file_workspace 的目标去重/追加实现，UI 不复制匹配或排名算法。文件列表的合法追加保留 candidate_list_version，原 delta 传输继续有效。

## 验证

- `scripts/check.sh`：**352 Rust / 201 Flutter 通过，6 Rust ignored**；Native menu/Host/权限/材质/焦点、生成检查、fmt、Clippy、架构和 Flutter analyze 通过。
- Core streaming 测试覆盖未交互及显式冻结、正常结束及中途失败：晚到的更高排名项不能改变旧前缀；列表版本保持；新行追加；覆盖/失败状态仍正确。
- Flutter 覆盖自动预览、操作面板原目标、Esc 从预览文字清空查询、迟到预览取消、连续加载后的选择/滚动、920×600 稳定布局/端口参数、退出宽度恢复，以及 **640×480 + 1.35 倍文字** 的无溢出布局。
- 截图检查：`build/ui-v0.7-build22/files-empty.png`、`files-results.png`、`files-actions.png`、`files-preview.png`。这些是 Flutter 离屏布局截图，不代表实机原生玻璃材质。
- 新包已完成构建和静态包核验；本轮截图为离屏 UI 验证，未切换当前正在使用的 build 21，未记作 build 22 的实机窗口尺寸验收。
- 全量日志：`build/validation/files-build22-check.log`；专项：`stable-file-order-tests.log`、`fixed-preview-ui-tests.log`。本次未提交或推送 Git，保留此前工作区修改。

产物：`build/Anywhere-dev-0.7-build22.app`、`build/Anywhere-dev-0.7-build22-macos-arm64.zip` 和 `.sha256`。签名、版本、arm64、Impeller、ZIP 一致性记录见 `build/validation/files-build22-package-check.json`。
