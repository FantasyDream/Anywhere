# V0.7 MVP：系统文件搜索

日期：2026-10-08。分支 `dev-0.7`，基线为已推送的 `dev-0.6@3b21483`。开发版本 `0.7.0+1` / Rust `0.7.0-dev.1`。本版开发中，首个功能切片已接入；不表示 0.7 全部验收完成。

## 范围与进度

| 切片 | 范围 | 状态 |
|---|---|---|
| S1 搜索与打开 | 异步系统索引、统一 Candidate/Ranking、限量与取消、打开文件/目录及执行复验 | 已实现，验证记录见下方 |
| S2 可理解与可配置 | 明确搜索来源与加载/失败反馈、可关闭来源、检索目录与排除策略的设置，结果的常用次级动作 | 待实现 |
| S3 收尾 | 真实索引/未索引/隐私排除、多窗口与连续输入验收，性能与资源上限核对 | 自动化及只读索引检查已开始，完整实机验收待完成 |

macOS 使用系统 Spotlight 索引，不建立应用自己的文件索引。第一版检索文件名，不检索文件正文。Apple 的元数据查询能力及查询阶段见 [NSMetadataQuery](https://developer.apple.com/documentation/foundation/nsmetadataquery)；当前 Adapter 使用系统自带 `/usr/bin/mdfind`，调用格式依本机 `mdfind -h`。

Windows 保留同一 Platform 接口和不支持时的降级；Everything Adapter 尚未实现，不作为近期 macOS 门槛。剪贴板历史、浏览器历史和书签不在本轮范围。

## S1 使用与边界

1. 进入空白 Global Search，输入至少两个字符的文件名，匹配结果延迟加入同一列表；Enter 或 Open 使用系统默认应用打开文件，目录使用文件管理器打开。
2. 应用与当前 Context 动作先展示，文件来源延迟约 150ms 启动。它们使用同一查询匹配档位和排序，文件的非查询权重低于当前对象的动作与应用；最多保留 8 条文件结果，文件不会出现在空查询首页。
3. 文件搜索只在未选择 Scope 且非 Content 模式时运行。Current App、Folder、Git 等现有 Scope 继续只展示相应动作；不新增 Files Scope。自动填入选中文本/15 秒剪贴板内容的页面不启动文件检索。
4. 文件名查询最多 256 UTF-8 字节，跳过控制字符、带 `/` 的路径/URL和 `{` / `[` 开头的内容；明确对象仍由原 Context 路径处理。
5. 首批固定检索用户 Home 下的系统索引结果，排除隐藏路径、Library、node_modules、target、build、`.app` / `.framework` 的自身和内部路径。这是首版默认策略；范围/排除设置待 S2，不能将 Spotlight 返回空列表解释为文件不存在。
6. `/usr/bin/mdfind -0 -onlyin HOME -name QUERY` 使用固定 argv、不经过 shell；NUL 分隔支持带换行的文件名。流式读取最多 256 KiB、单路径最多 16 KiB、最多 64 个有效索引命中，canonical path 去重。过滤不存在/非文件/非目录对象，再按文件名匹配排序，保留 8 条。
7. 同时最多一个索引查询。查询进程最多运行 2 秒；新输入/Scope、关闭或新 Session 撤销旧查询，终止并回收进程。取消异步等待和来源超时也通知阻塞 worker；每次管道轮询约 10ms。普通文件系统 metadata/canonicalize 仍为同步 OS 调用，不承诺网络卷或 OS 调度异常下的硬实时耗时。
8. 新结果只提交给本次 Session/Revision，推进 inputs_epoch 并经已有快照复验发布。Core 不阻塞首个 Context 快照。异步来源本身有 3 秒等待上限，失败不移除应用与 Context 动作；当前仅安全日志反馈，用户可见来源状态待 S2。

## 执行、身份与历史

- 新 `CandidateKind::File` 表示结果对象，不假装是菜单或 Context 动作。Flutter 沿既有图标/结果行显示文件名、父目录；主要按钮显示 Open。
- 文件结果的执行目标复用 `core.file.open` / `core.directory.open` 和当前 ActionRegistry。结果不加入 Context Graph、Context action offers、Scope 或快捷键对象列表，避免把搜索发现的对象当成当前环境。
- 业务动作 ID + 规范路径指纹形成结果 ID。文件名/父目录/路径只用于当前结果和执行；排名诊断的 top_candidates 对文件使用通用 `file` 标签，不记录名称。
- 新输入清除缓存批次；Registry 重载后结果重新取得当前动作定义及 Availability，不沿用已撤销 handler。执行仍验证 Session/Revision/目录版本，再复验路径存在、规范路径和文件/目录类型；失效时失败且不记录成功 Usage。
- 成功打开沿既有 Usage 记录业务动作与 Context 类型，不写具体路径。首版文件排序不提供逐文件 frecency；后续如有需求，先定义独立的历史保留和隐私契约。

## 架构

```text
Global 查询
  → AsyncCandidateSource（来源自定 accepts 条件）
  → 防抖、取消与有界批次
  → Platform.search_files → macOS mdfind Adapter
  → Session 的本 revision 批次
  → ReadySource → 既有 CandidateSource / Ranking
  → 快照 publication 复验
  → 既有执行与 Usage
```

同步 CandidateSource 继续用于内存来源。异步协调、Context 完成状态和进程会话相互独立；文件检索不阻塞 global action 等待当前 Context，不进入 Running Tasks。平台接口承载查询、数量限制和取消回调，Core/Flutter 不依赖 Spotlight 或 AppKit。

## 验收要求

- 连续输入/改 Scope/关闭/新 Session 不出现旧结果，也不能执行旧结果。
- Context 动作、应用与文件共同排名，文件限量且不进入 Context 首页/Scope/快捷键目标。
- 中文、空格、换行、重复、失效路径、类型变化、索引失败与超时均有确定行为；删除文件后不打开其他目标。
- 搜索失败与索引不可用不阻止已有能力；设置提供明确来源状态和关闭入口。
- 真机核对默认打开、次级动作、系统隐私排除/未索引、并发和进程清理，记录耗时/资源上限。只读查询和组件测试不能替代完整 GUI 验收。

## S1 验证记录

Core 回归：异步批次、过期查询与关闭、Scope/内容/短查询门禁、去重/限量/名称匹配、打开文件与目录、业务 Usage、删除目标无副作用、来源失败保留 Context 动作。原生 Adapter 回归：NUL 路径、Unicode/换行、canonical 去重/过滤、取消和超时回收进程。Flutter 回归：结果显示、Open、Enter 传递精确候选 ID。

本机只读 `cargo run -p anywhere-platform --example platform -- search-files roadmap` 返回 1 条有效索引命中，规范绝对路径及存在性检查通过；没有打开文件。

完整 `scripts/check.sh`：302 项 Rust、135 项 Flutter 测试通过，4 项既有外部工具测试 ignored；原生菜单/来源/Host/材质、fmt、Clippy、架构和 Flutter analyze 通过。新增回归也覆盖 Registry 撤销后的缓存失效、类型变化/符号链接重定向、截断/过大/失败输出拒绝。S2/S3 与完整 GUI 验收仍开放；没有启动新版进行 GUI 或性能采样。

独立开发包：`build/Anywhere-dev-0.7-build1.app`、`build/Anywhere-dev-0.7-build1-macos-arm64.zip` 及 `.sha256`。版本对齐，四个主二进制 arm64，Impeller 保持启用；deep/strict 固定签名、ZIP 内容一致性和 SHA-256 校验通过，签名 requirement 与 0.6 build 23 一致。旧包保留，未启动新版。
