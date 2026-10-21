# 中英国际化与拼音：开源选型

日期：2026-10-21。范围为现有 Flutter / Rust / Web 分层；性能数字需要本项目测量，不按星数或框架名判断。

| 候选 | 来源与能力 | 选择 |
|---|---|---|
| Flutter 官方 flutter_localizations + intl / gen-l10n | [官方指南](https://docs.flutter.dev/ui/internationalization)：ARB、编译期类型化访问、参数 / 复数、系统控件与日期 | 采用；随固定 Flutter SDK 锁定 intl，不另写翻译引擎 |
| slang | [包文档](https://pub.dev/packages/slang)：代码生成、多资源格式与命名空间 | 当前仅两种语言，官方方案足够，不增加第二套 Flutter 资源模型 |
| Rust pinyin 0.11.0 | [源码](https://github.com/mozillazg/rust-pinyin)、[API](https://docs.rs/pinyin/0.11.0/pinyin/trait.ToPinyinMulti.html)、[特性](https://docs.rs/crate/pinyin/0.11.0/features)：MIT，字符读音与多音字 | 采用，关闭默认特性，仅 plain / heteronym；静态词典，按需匹配，无组合爆炸或常驻全盘拼音缓存 |
| rust-i18n | [项目](https://github.com/longbridge/rust-i18n)：编译期资源、fallback、插值，可显式传 locale | 暂不引入。Core 不存 UI 当前语言；先将 Host 文案交给官方 Flutter 呈现。后续若业务返回类型化 message key / 参数，再评估 Rust 侧必须翻译的真实需求 |
| Fluent | [FluentBundle](https://docs.rs/fluent/latest/fluent/bundle/struct.FluentBundle.html)：复合语法与运行时 bundle | 现阶段不需要额外的格式语法 / bundle 生命周期 |
| i18next | [官方入门](https://www.i18next.com/overview/getting-started)：框架无关、内嵌资源、复数 / 插值与语言切换 | Web 示例优先采用；宿主 SDK 只提供语言，不绑定翻译库或联网加载器 |

## 拼音接入边界

读音查询完全复用 pinyin；只实现与既有排名衔接的匹配逻辑。对普通 ASCII 名称跳过拼音。文字未命中时再检查全拼 / 首字母；词典支持的多读音通过有界状态集合匹配，不展开所有字符串。单 token 上限 256 字节，工作内存随 token 长度增长；不缓存历史查询或文件全文。

多音字按字符接受词典中的读音，不承诺词组语义消歧或人名读音判断。全拼和首字母均支持，中途省略任意字、模糊音、带调拼音及跨字段凑词不纳入。未知汉字保留，未命中的词不能被转换过程丢弃。

候选 literal 等级排在拼音等级之前，各等级间隔仍大于非查询权重上限。文件完整遍历共享匹配，Spotlight 未按拼音返回的文件可以在遍历阶段找到；这不改变权限、排除项或文件类型约束，也不承诺遍历首帧和索引同速。


## 首轮本机测量

使用现有 release `measure` 示例，1000 个合成中文名称，20 次预热 / 200 次测量：全拼 `chongqingkaifa` p95 0.301 ms、首字母 `cqkfgj` p95 0.462 ms、未命中 `not-present` p95 0.543 ms。ASCII catalog 的既有样本也输出在同一日志；未做同机旧提交 A/B，不声称性能提升比例。此处不含文件遍历、IPC、UI 帧、WebKit 或 App RSS。

匹配不保存拼音字符串全集；每次使用两个随 token 长度变化的状态向量，常见 ü 支持键盘 `v` 输入。保留精确字面匹配与未知字约束。原始本机忽略日志为 `build/i18n-cpu-measure.log`，可通过 `cargo run --locked --release -p anywhere-core --example measure` 重复测量。
