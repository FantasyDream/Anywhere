# Anywhere 手工验收清单

适用版本：2026-09-15 **V0.2**。原有 A–R case 保持编号；V0.2 新增 **V01–V12**，覆盖 Scope Bar、应用搜索、Git 动作、Availability、late Context 与结果预览。每节列的是操作和预期，不能据此判定已经通过；本清单不含任何实测结果。

V0.2 与原版的差异：无输入进入 Context Home，只显示当前环境动作（最多 7 条）；有输入同时搜索应用、动作和环境动作；搜索栏下方新增 Scope Bar；Git 目录出现 Git Status / Git Log / Git Diff；剪贴板已等于当前路径时 Copy Path 不显示。原有 B/E/H/X 中「有输入必须完全没有环境动作」的预期调整为「环境动作只在匹配输入时出现」。

**本次优先回归：K01/K02/K10/K12。** 已根据 5 份崩溃报告将 macOS 渲染器切回 Skia，结果待人工确认；提高了浅色背景下文字对比度。Windows 专用 18 项 case 见 [Windows 清单](windows.md)。

**外观回归：K01、K12、K14。** 完全退出旧进程再打开新版；先将浅色窗口置于 Launcher 后面检查四角，再换有明显色块/图案的背景，观察背景色是否经过模糊透出。搜索栏、展开列表、JSON 预览三种状态都应有相同的毛玻璃底层，四角外没有直角底板。若系统已开启「降低透明度」，请在结果中注明；原生材质遵循该辅助功能设置。

你已测到第 8 类 X。本轮先回归 **K01–K14、A03、A07、A08、B01/B04、D04、E01/E04、X08–X10、X11**，再继续尚未测完的 **H、P、R**。其余已通过项可保留上次记录，不要求全部重测；完整 case 仍全部列在下面。

关键变更：失焦即隐藏并结束 Session；无动作时隐藏整个动作区；有输入时只显示该输入的动作；X08/X09 已重写，X10 也不再依赖切到 Finder 后保留旧窗口。A03/D04/E04 的复制需要重新实测。

## 0. 准备

1. 使用 [Anywhere.app](</Users/luckincoffee/Documents/Github/Anywhere/build/Anywhere.app>)。如果后台仍是旧进程，从菜单栏选择 **Quit Anywhere**，再双击此应用。不要只关闭窗口代替退出。
2. 打开 Finder、一个可以粘贴纯文本的编辑器、Terminal 和默认浏览器。
3. 测试数据已经准备好，不需要你创建文件。

| 数据 | 路径 |
|---|---|
| 目录 A | `/Users/luckincoffee/Documents/Github/Anywhere/build/manual-validation/目录 A` |
| 目录 B | `/Users/luckincoffee/Documents/Github/Anywhere/build/manual-validation/目录 B` |
| 文本文件 | `/Users/luckincoffee/Documents/Github/Anywhere/build/manual-validation/目录 A/hello world.txt` |
| 可改名测试文件 | `/Users/luckincoffee/Documents/Github/Anywhere/build/manual-validation/目录 A/temporary.txt` |
| 插件样例 | `/Users/luckincoffee/Documents/Github/Anywhere/build/manual-validation/` |

Finder 中用 **⌘⇧G** 粘贴目录路径，再按 Enter。Launcher 中用 **⌘A → 粘贴** 替换输入；这里输入的是路径本身，不要带 Markdown 反引号或引号。

通用验收规则：

- 每次唤醒应有一个窗口，输入框可直接打字，无需先点鼠标。
- 使用 ↑ / ↓ 选中动作，按 Enter 执行。按标题选，不要假设动作永远在固定行：使用历史会影响同组顺序。
- URL/JSON/文件/目录均同时保留 Text 解释，因此出现 Search Google / Web Search 是正常的。
- **输入非空时不显示 Finder 环境动作**；清空或只输入空白才恢复本 Session 的环境动作。URL/JSON/路径保留自己的 Text 搜索动作，不属于环境残留。
- 打开/复制成功应隐藏窗口；Format JSON、Minify JSON 应在窗口中显示结果。
- 点击其他应用或切换到其他应用时，窗口立即隐藏。重新唤醒是新 Session：输入、选中项和结果重置，并重新捕获上下文。
- 无动作且无诊断时只有约 648×64 的搜索栏；动作行随数量增减，最多显示 7 行，更多动作可滚动。总高度不超过 520；错误提示或 JSON 预览也受此上限约束。

## 1. 启动、焦点、键盘

| 编号 | 操作 | 通过标准 |
|---|---|---|
| K01 | 从文本编辑器等非 Finder 应用前台退出并重新启动 Launcher，保持空输入；分别在浅色窗口和彩色图案背景前唤醒 | 出现约 648×64 的搜索栏；没有动作标题、空列表占位或底部动作栏；不是黑屏或 0×0；深色毛玻璃、细边框，四角外完全透出后方内容，没有直角黑底；背景颜色模糊透出，文字保持清晰 |
| K02 | 唤醒后直接逐字输入 `hello world`；逐字退格，长按退格删完；再粘贴相同文本并逐字删除；重复 3 次 | 全程不闪退；字符完整，焦点正常；动作结果正确，删空后按当前环境恢复；不能只验证粘贴成功 |
| K03 | ⌘A 后粘贴 `https://github.com` | 原输入被完整替换，冒号和两个斜杠都保留，出现 URL 动作 |
| K04 | ⌘A 后粘贴 `{"name":"foo","age":18}` | 引号、大括号、冒号都保留，出现 JSON 动作 |
| K05 | 对多个动作按 ↓、↑；在首行按 ↑、末行按 ↓ | 高亮跟随移动，首尾可以循环；多行列表选中项自动进入可见区域 |
| K06 | 选中一个复制动作，按 Enter | 执行高亮的动作，窗口隐藏；粘贴结果与对象一致 |
| K07 | 按 Esc；再次按 ⌘⌥Space | Esc 隐藏，快捷键重新显示；新一次输入为空并重新捕获当前前台上下文 |
| K08 | 窗口显示时再次按 ⌘⌥Space | 窗口隐藏；再按一次重新出现，没有额外窗口 |
| K09 | 从菜单栏选择 Open Anywhere，再选择 Quit Anywhere | 能显示窗口；Quit 真正退出，菜单栏图标消失 |
| K10 | 中文输入法连续输入 `你好世界`，↑/↓ 选词、Enter 确认；再删除已确认文字，组合期间按退格，反复中英文切换输入 | 不闪退；候选选择/确认不移动或执行动作；确认后得到文本动作，删除后正常更新 |
| K11 | 唤醒后分别点击另一个应用的窗口、用 ⌘Tab 切换应用 | 两种方式都立即隐藏 Launcher；不需要 Esc；再次唤醒输入为空 |
| K12 | 从非 Finder 应用唤醒，依次输入 `hello world`、`{"a":1}`，再清空 | 默认插件下分别为 0→2→5→0 个动作；窗口随之增高/降低，搜索栏顶边保持不动；空结果没有列表标题和底栏；解析期间可保留上次已提交列表但不可执行；结果提交后按新数量变化；无裁切或明显空白占位；展开/收起后的毛玻璃、四角和阴影跟随新尺寸，不残留矩形底板 |
| K13 | 按下面「长列表准备」临时加载样例，输入 `hello world`，滚轮向下，再用 ↑/↓ 循环首尾 | 默认仅此样例加 Google 插件时共 10 个动作；最多约 7 行可见，总高度 ≤520；剩余动作能滚动到；键盘高亮项始终在可见区域，所有动作可到达 |
| K14 | 输入 JSON 并 Format；选择预览文字尝试 ⌘C；在输入框用 ⌘A 替换为文本；观察布局和背景 | 预览可选择并复制；编辑输入后回到新动作列表；搜索框、列表、结果和底栏均无溢出，长副标题截断且不挤压标题；毛玻璃连续覆盖整窗，选中行仍清晰可辨，JSON 页四角没有直角背景 |

如果 K03 / K04 失败，记录实际字符和动作，不要把它当作其他场景已经通过；可以手动逐字输入继续定位识别问题。

**K13 长列表准备**（只为测高度，完成后移走样例再继续其他 case）：

1. 退出 Launcher，复制已准备的样例：

```bash
mkdir -p "$HOME/Library/Application Support/Anywhere/plugins"
cp '/Users/luckincoffee/Documents/Github/Anywhere/plugins/manual-fixtures/manual-window-size.toml' \
   "$HOME/Library/Application Support/Anywhere/plugins/manual-window-size.toml"
```

2. 重启，执行 K13。样例增加 8 条 `Window Size 01`–`08` 文本复制动作，不会自动执行。
3. 测完退出，将插件目录里的 `manual-window-size.toml` 移到桌面或其他备份目录；重启。不要移动其他插件。

K12/K13 的精确动作数量以未装其他自定义插件为前提；已有插件不要求删除，记录实际数量并判断高度是否随之变化即可。

## 2. A：Finder 零输入上下文

1. Finder 打开目录 A，确保 Finder 在前台。
2. 按 ⌘⌥Space，保持输入为空。
3. 首次出现 Finder Automation 提示时，允许访问后继续。授权耗时与正常唤醒耗时分开记录。

| 编号 | 操作 | 通过标准 |
|---|---|---|
| A01 | 观察空输入列表 | 出现 Open Terminal Here、Copy Path、Open Directory；副标题是目录 A；标题栏类型含 Application/Directory；无需输入 |
| A02 | 选择 Open Terminal Here → Enter | Terminal 打开；在新终端执行 `pwd`，输出目录 A 的实际路径 |
| A03 | 先在编辑器复制 `clipboard-before-中文`；回到 Finder 目录 A，唤醒并选 Copy Path → Enter；回编辑器连续粘贴两次 | 两次都得到目录 A 的完整路径，中文和空格保留；剪贴板不为空；成功复制替换旧文本是正常行为。随后重新复制普通文字仍能粘贴 |
| A04 | 同样重新唤醒，选择 Open Directory → Enter | Finder 打开目录 A，而不是仓库根目录或 Launcher 所在目录 |
| A05 | Esc 后在 Finder 切换到目录 B，再唤醒 | 三个目录动作的副标题和执行目标更新为目录 B，没有残留目录 A |
| A06 | 从文本编辑器等非 Finder 应用唤醒，输入为空 | 不出现上一次 Finder 的目录动作；无自定义应用动作时只显示搜索栏，不显示 Context 占位 |
| A07 | 分别在 Finder 的「最近使用」和「共享」页面唤醒，保持空输入，再输入 `hello world` | 两个虚拟页面都不显示目录动作，不出现 alias 转换或 -1700 错误；空输入只有搜索栏；文本动作仍正常 |
| A08 | Finder 关闭所有文件窗口、桌面在前台时唤醒 | 以 Desktop 为目录，能提供目录动作；不崩溃；不要与「最近使用」混为一项 |

## 3. B：URL 识别与执行

先从 Finder 目录 A 唤醒，再输入：

```text
https://github.com
```

| 编号 | 操作 | 通过标准 |
|---|---|---|
| B01 | 从 Finder A 唤醒后输入 URL，观察整个列表 | 类型为 URL + Text；有 Open in Default Browser、Copy URL 和文本搜索；**没有** Open Terminal Here、Open Directory 或目录 A 的 Copy Path |
| B02 | Open in Default Browser → Enter | 默认浏览器打开 GitHub；Launcher 隐藏。网络加载失败和打开了错误地址要区分记录 |
| B03 | 重新输入同一 URL，Copy URL → Enter，再粘贴 | 得到 `https://github.com/`；末尾 `/` 是 URL 规范化，属于正确结果 |
| B04 | 将输入完全清空 | 回到本次 Session 捕获的目录 A 动作；不残留 URL 动作 |
| B05 | 输入 `HTTPS://github.com` | 仍识别为 URL；复制结果的 scheme 可规范化为小写 |

## 4. C：JSON 识别、格式化与复制

输入：

```json
{"name":"foo","age":18}
```

| 编号 | 操作 | 通过标准 |
|---|---|---|
| C01 | 观察列表 | 有 JSON + Text；出现 Format JSON、Minify JSON、Copy Formatted JSON，JSON 动作优先于文本动作；无 Finder 环境动作 |
| C02 | Format JSON → Enter | 窗口保留，显示可选择的多行 JSON；值仍为 name=foo、age=18 |
| C03 | 在格式化结果页按 ↑ 或 ↓ | 返回动作列表并移动选中项，之后仍可执行动作 |
| C04 | Minify JSON → Enter | 窗口保留，结果是合法单行 JSON；字段值不变 |
| C05 | Copy Formatted JSON → Enter，再到编辑器粘贴 | 窗口隐藏，剪贴板是格式化后的 JSON；不是原始压缩字符串 |
| C06 | 显示格式化结果时按 Esc | 直接隐藏窗口；重新唤醒不残留旧结果 |
| C07 | 在结果页改输入为 `hello world` | 结果预览消失，显示新文本的动作 |

字段顺序不作为通过条件。比如当前序列化可能输出：

```json
{
  "age": 18,
  "name": "foo"
}
```

## 5. D：真实文件

输入完整路径：

```text
/Users/luckincoffee/Documents/Github/Anywhere/build/manual-validation/目录 A/hello world.txt
```

| 编号 | 操作 | 通过标准 |
|---|---|---|
| D01 | 观察列表 | 有 File + Text；Open File、Reveal in File Manager、Copy Path 优先出现 |
| D02 | Open File → Enter | 用系统默认应用打开 `hello world.txt`，内容含 `Anywhere manual validation` |
| D03 | 重新输入文件路径，Reveal in File Manager → Enter | Finder 定位并选中 `hello world.txt`，不仅是打开父目录 |
| D04 | 先复制 `clipboard-before-中文`；重新输入文件路径，Copy Path → Enter，再到编辑器连续粘贴两次 | 两次都是完整文件路径，中文、空格、扩展名不丢失，不为空；随后普通文字的复制粘贴仍正常 |

## 6. E：显式目录独占与去重

1. 从 Finder **目录 A** 唤醒。
2. 输入 **目录 B** 的完整路径。

| 编号 | 操作 | 通过标准 |
|---|---|---|
| E01 | 从 A 唤醒并输入 B，观察整个列表 | 有 Directory + Text；目录动作全部针对 B；没有针对 A 的环境动作；通过副标题确认目标 |
| E02 | 选择 B 的 Open Directory → Enter | Finder 打开目录 B |
| E03 | 重新从 A 唤醒、输入 B，选择 B 的 Open Terminal Here → Enter | 新 Terminal 中 `pwd` 是目录 B |
| E04 | 先复制 `clipboard-before-中文`；同样输入 B，选择 B 的 Copy Path → Enter；连续粘贴两次 | 两次均为目录 B 的完整路径，不是 A，也不是空文本；中文和空格保留；随后普通文字复制粘贴仍正常 |
| E05 | 从 A 唤醒，输入同一个目录 A | 每个目录命令只出现一次，不因显式输入和环境 Context 相同而重复；仍保留文本搜索动作 |

## 7. F：文本与 Google 声明式插件

| 编号 | 输入与操作 | 通过标准 |
|---|---|---|
| F01 | 输入 `hello world` | 出现 Search Google 和 Web Search；Search Google 副标题含 Google Search plugin |
| F02 | 选择 Search Google → Enter | 默认浏览器打开 Google，搜索词是完整的 `hello world`；窗口隐藏 |
| F03 | 重新输入 `hello world`，选择 Web Search → Enter | 内建搜索也能打开正确搜索词；不是只有插件能执行 |
| F04 | 输入 `hello 世界 &admin=true#x/%`，执行 Search Google | 搜索词整体保留；`&admin=true` 和 `#x` 不应变成额外 URL 参数或丢失的 fragment。浏览器解码后的 q 应等于整个输入 |

不要求外网搜索结果一定能加载；应检查确实唤起默认浏览器、目标地址和搜索词正确。

## 8. 输入边界与 Session

| 编号 | 输入/操作 | 通过标准 |
|---|---|---|
| X01 | 依次输入 `github.com`、`https://`、`https:///github.com`、`ftp://example.com` | 按普通 Text 处理，不出现 URL 专属动作，不崩溃 |
| X02 | 输入不存在的绝对路径，例如 `/nonexistent-launcher-manual-test/file.txt` | 只有 Text 解释和文本搜索动作；没有任何 File/Directory 动作，包括环境目录动作 |
| X03 | 输入 `{"a":}` | 按 Text 处理，不出现 JSON 动作，不崩溃 |
| X04 | 分别输入 `true`、`null`、`123`、`[]`、`"hello"` | 都识别为合法 JSON + Text，不只支持对象/数组 |
| X05 | 输入 `   `，再清空 | 不为纯空白提供文本搜索，保留本次环境 Context |
| X06 | 输入 `~/Desktop` | 当前版本不解释 `~` 或 shell 语法，应按 Text 处理；绝对路径才用于稳定验收 |
| X07 | 快速反复替换 URL → JSON → `hello world`，最后停在 `hello world` | 最终只有最后输入的语义动作，不被旧 URL/JSON 结果覆盖 |
| X08 | 快速连续替换 URL/JSON 后，立即点击编辑器使 Launcher 失焦，等待几秒；再从编辑器唤醒 | 失焦立即隐藏；旧输入的异步结果不能自动弹回窗口；重新唤醒输入为空，不残留旧动作或结果 |
| X09 | 从 Finder A 唤醒；点击 Finder 切到 B；再次按 ⌘⌥Space | 点击 Finder 时旧窗口已隐藏；再次唤醒是新 Session，输入为空，动作全部针对 B；不再要求保留 A 的旧 Session |
| X10 | 输入 `temporary.txt` 的完整路径；切到 Finder 将其改名为 `temporary-renamed.txt`；重新唤醒并输入旧路径，随后输入新路径 | 切到 Finder 时隐藏；旧路径只给文本搜索，不能执行旧文件动作；新路径给 File 动作。测完恢复原名 |
| X11 | 按复制或打开动作后立刻切到其他应用并重新唤醒，再输入 `hello world` | 新窗口不被上一 Session 的执行完成回调隐藏或覆盖；旧动作已经开始的系统副作用允许完成，不要求回滚 |

X07 / X08 / X11 是人工冒烟：每项建议重复 3 次。人工无法稳定控制异步完成顺序；不要为了制造竞态修改系统设置。原 X10 的「同窗口执行期间文件失效」交由已有平台失败自动测试覆盖，新的 X10 验证失焦后的重新识别。

## 9. 使用历史与持久化

整个测试使用同一个唤醒来源，例如始终先回到 Finder 目录 A 再唤醒；不同 source_app 的历史不会混用。

| 编号 | 操作 | 通过标准 |
|---|---|---|
| H01 | 输入 JSON，记录当前顺序；执行 Minify JSON，再重新输入 JSON 触发排序 | 该命令的历史计数增加，并在尚未达到加分上限时得到加分；历史较少时通常会向前移动 |
| H02 | 重复执行同一命令数次，每次重新输入或重新唤醒后观察 | 频次加分生效，排序稳定；已经排在第一或达到上限时，不要求继续改变名次 |
| H03 | 退出应用，再启动并从同一个应用唤醒、输入同类对象 | 使用历史保留，没有因为重启而清零 |
| H04 | 先多次执行 Finder 目录动作，再输入 URL | URL 输入后没有任何环境目录动作；历史不能让隐藏的环境动作重新进入列表；URL 专属动作仍优先于 Text 搜索 |

如需精确确认计数，下面是只读查询，不会清空或修改历史：

```bash
sqlite3 "$HOME/Library/Application Support/Anywhere/usage.sqlite3" \
  'SELECT command_id, context_type, source_app, execute_count, last_executed_at FROM command_usage ORDER BY last_executed_at DESC LIMIT 20;'
```

记录执行前后对应行的 count。成功动作应增加 1，失败动作不增加。历史加分有上限，不能用「必须永远排到第一」作为所有历史条件下的断言。

## 10. 声明式插件的完整路径

这组会临时增加测试插件，建议 A–F 完成后再做，避免影响前面的动作列表。

### P01–P04：合法插件与四种 Action

1. 退出 Launcher。
2. 在终端执行以下命令，把已经准备好的一个合法插件放入插件目录：

```bash
mkdir -p "$HOME/Library/Application Support/Anywhere/plugins"
cp '/Users/luckincoffee/Documents/Github/Anywhere/plugins/manual-fixtures/manual-validation-valid.toml' \
   "$HOME/Library/Application Support/Anywhere/plugins/manual-validation-valid.toml"
```

3. 启动 Launcher，按下表执行：

| 编号 | 操作 | 通过标准 |
|---|---|---|
| P01 | 输入 `hello world`，选择 Manual Copy Text | 粘贴结果为 `COPY[hello world]`，验证 `copy` 和模板替换 |
| P02 | 输入 `hello world.txt` 的完整路径，选择 Manual Open File | 正确打开文件，验证插件的 `open_file` |
| P03 | 输入 JSON，选择 Manual Minify JSON | 显示合法单行 JSON，验证 `execute_builtin`；与内建 Minify JSON 一致 |
| P04 | 再执行 Search Google | Google 插件仍正常，覆盖 `open_url`；测试插件与内建命令共用列表/排序 |

### P05–P12：非法插件隔离

1. 退出 Launcher。
2. 将已准备好的 8 个非法样例复制进去：

```bash
cp /Users/luckincoffee/Documents/Github/Anywhere/plugins/manual-fixtures/invalid-plugins/*.toml \
   "$HOME/Library/Application Support/Anywhere/plugins/"
```

3. 重新启动。应能继续输入和执行合法命令，并显示 diagnostic；在日志中逐一检查下表错误。启动多条 diagnostic 时，以日志为准。

| 编号 | 文件 | 验证点 |
|---|---|---|
| P05 | manual-invalid-01-version.toml | 不支持的 protocol_version 被拒绝 |
| P06 | manual-invalid-02-duplicate.toml | 同一插件重复 command id 被拒绝，不能只注册其中一部分 |
| P07 | manual-invalid-03-action.toml | 未知 `run_shell` action 被拒绝，不能运行脚本 |
| P08 | manual-invalid-04-variable.toml | 未知模板变量被拒绝 |
| P09 | manual-invalid-05-field.toml | 缺少 action 必需字段被拒绝 |
| P10 | manual-invalid-06-matcher.toml | 非法 Context matcher 被拒绝 |
| P11 | manual-invalid-07-syntax.toml | TOML 语法错误被拒绝 |
| P12 | manual-invalid-08-builtin.toml | Text matcher 调 JSON builtin 的不兼容声明被拒绝 |

全部样例的标题 `INVALID SHOULD NOT APPEAR` 都不能进入动作列表；合法 Google 和 Manual Validation 插件应继续可用。不能只看应用没有崩溃，还要确认具体错误和非法命令未注册。

测完退出应用，将本次添加的 `manual-validation-valid.toml` 和 `manual-invalid-*.toml` 移回工作区或其他备份目录，再重新启动。不要移动你原本已有的其他插件。

## 11. 权限、错误、日志与性能

| 编号 | 操作 | 通过标准 |
|---|---|---|
| R01 | 首次 Finder 授权允许后，再正常唤醒 | 能读取目录；正常唤醒不反复要求授权 |
| R02 | 可选：在系统设置中暂时关闭此应用对 Finder 的 Automation 授权，重启并从 Finder 唤醒 | 显示明确失败提示，但 URL/JSON/文本输入仍可用。此项测完恢复原授权；不需要撤销 Codex 的任何权限 |
| R03 | 若任一实际执行发生失败，观察错误，再把输入改为 `hello world` | 失败有提示，不显示虚假成功；改输入后可以继续执行。若本轮未遇到执行失败，记「未测」，不要将 X02 的不存在路径识别当作执行失败 |
| R04 | 查看本次日志 | 能看到 Session、Revision、Resolver、Provider、Matcher、Ranking、Action；非法插件测试有 plugin_load_failure；平台失败有错误日志 |
| R05 | Finder 已授权，从同一实际目录执行「唤醒→Esc」10 次 | 记录各次 hotkey_to_visible_ms，报告中位数、最大值；目标 <100 ms。首次授权的几秒等待单列，不混进正常样本 |
| R06 | 输入 URL、JSON、文件、目录、文本，查看耗时 | 记录 input_context_resolved、input_actions_ready、ranking_completed；Resolver 目标 <10 ms、Ranking 目标 <5 ms，超标照实记录 |

日志位置：

```text
~/Library/Application Support/Anywhere/launcher.log
```

每次启动会覆盖日志，**发现问题后先保存相关日志，再重启**。可在 Terminal 实时观察：

```bash
tail -f "$HOME/Library/Application Support/Anywhere/launcher.log"
```

`duration_us` 除以 1,000 才是 ms；`hotkey_to_visible_ms` 已经是 ms。该 native 事件记录 show 调用完成，不单独证明首帧已被屏幕绘制，所以性能数据需配合你看到的窗口和响应一起判断。

## 12. 已有自动测试，不要求人工伪造内部状态

以下列出现有自动检查的范围，供定位失败时参考；本轮按照你的要求停止自行测试，人工项由你执行：

- 17 项 Rust 测试：Resolver、Matcher、Ranking、SQLite、Protocol 序列化、插件、Facade A–F、失败处理等。
- 7 项 Flutter widget 测试：原有 4 项，以及动态高度/滚动、失焦时输入结果失效、失焦时执行结果失效。
- 受控延迟 Provider 在 Revision / Session / Hide 变化后丢弃旧结果。
- Context Graph 深度限制、环路与去重；Provider 按当前版本的输入最多执行一次。
- 自定义 ContextTypeId 序列化、JSON 原始文本保留、插件原子注册、模板 percent-encoding 和大小限制。
- Core 依赖边界、fmt、clippy、Flutter analyze，以及 Release 构建与签名都有现有检查入口；具体已执行结果见验收记录。
- 新增 UTF-8 剪贴板平台 harness 覆盖 ASCII、中文路径、emoji/多行文本 × C/UTF-8 两种环境，运行时临时改写后恢复剪贴板。

可重新运行：

```bash
cd /Users/luckincoffee/Documents/Github/Anywhere
source scripts/env.sh
scripts/check.sh
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## 13. V0.2 新增验收

前置：按第 0 节启动 V0.2 应用。准备两个目录：一个普通目录（如 `~/Documents`），一个 Git 仓库（仓库内任意子目录都可以）；测试用的 `~/.gitconfig` 之外的 Git 配置保持默认。

| 编号 | 操作 | 通过标准 |
|---|---|---|
| V01 | 在 Finder 打开普通目录，⌘⌥Space | Scope Bar 显示 `[ All ] [ Finder ] [ Folder ]`；前三项是 Open Terminal Here、Copy Path、Open Directory；All 高亮。唤醒到出现列表约 0.1–0.4s 属正常：首屏会等目录解析完成，而不是先给一个空窗口 |
| V02 | 在 Finder 打开 Git 仓库（或其子目录），⌘⌥Space | Scope Bar 出现 `[ Git ]` 且排在 Folder 之前；前三项是 Git Status、Open Terminal Here、Git Log |
| V03 | 承接 V02，按 ⌥ → 或点击 `[ Git ]` | 只剩 Git 分组的三项；再按 ⌥ → 回到 All |
| V04 | 点击 `[ Finder ]` | 只显示 Finder 该环境的动作；不出现「打开某个应用」的候选 |
| V05 | 输入 `chrome` | 第一项是 Google Chrome；Enter 打开或激活它，窗口关闭 |
| V06 | 输入 `git sta` | 第一项是 Git Status；按 Enter 后窗口保持打开，预览区出现 `git status` 文本，底部标签显示 Result 而不是 JSON |
| V07 | 承接 V06，按 ↑ 回到列表，再执行 Git Log、Git Diff | 两者都显示文本预览；Git Diff 内容与 Terminal 中同一仓库的 `git diff` 一致 |
| V08 | 在 Dock 或菜单栏查看是否有终端窗口弹出 | V06/V07 不应打开 Terminal 或额外窗口 |
| V09 | 用 Terminal 把当前目录路径复制到剪贴板（`pwd \| pbcopy`），回到该目录的 Finder 窗口，⌘⌥Space | Copy Path 不出现在列表；Open Terminal Here 仍可用；把剪贴板换成其他内容后重新唤醒，Copy Path 恢复 |
| V10 | 在 Git 仓库中输入 `git status` 全文与 `status` | 两种输入都能把 Git Status 排到第一项 |
| V11 | 唤醒后立刻快速输入 `abc`，不要等待 | 先出现基于已有 Context 的列表，随后自动刷新为完整结果；列表刷新时不会把已按 ↑/↓ 选中的行跳走 |
| V12 | 在 V11 的 late 刷新期间按 Enter | 执行的是当前高亮行；若该动作需要仓库而仓库尚未解析完成，应给出明确失败信息而不是虚假成功 |
| V13 | 反复唤醒若干次（首次 Automation 授权、系统繁忙时会触发），观察是否有哪一次窗口先出现空列表 | 即使某次首屏为空，列表也应在几百毫秒内**自动**补全，不需要输入任何字符。若必须输入才出现，说明 late 结果没有送达界面——请保存日志并回报 |

若机器未安装 Git，V02/V06/V07 的三项 Git 动作应显示为不可用并给出原因（V09 仍应通过）；请在结果中注明。

## 14. 结果回传

你不必重测所有已通过项：先完成页首重点回归，再继续 H/P/R。下面是结果模板，失败用编号回报即可：

```text
K03：失败
操作：唤醒后 ⌘A，粘贴 https://github.com
实际：输入框没有变化
预期：完整 URL + 两个 URL 动作
时间：16:30:20
可重复：3/3
补充：手打是否正常；鼠标点击输入框后是否正常；截图/相关日志

A01–A06：通过
B01–B05：未测
V01–V12：未测
V05：失败（输入 chrome 后第一项是 Web Search）
R05：10 次，中位数 xx ms，最大 xx ms
```

没有执行的项保留「未测」。只有 A–F 的识别与实际动作、键盘流程、V01–V12 的 V0.2 行为，以及必要错误恢复都确认后，才能把实机 V0.2 验收标记为完成。
