# Scoop IDNA 库规范

版本：M61。提供独立普通 Cone `scoop:scoop.idna:0.1.0`，依赖 core、std；使用者包括 [URL 库](SCOOP-URL-SPEC.md)和 HTTP 域名处理。IDNA 不依赖 URL、HTTP、TLS 或 asyncio，不执行 DNS，也不维护公共后缀名单。迁移范围及实际验证进度见 [M61 设计](../milestone61/DESIGN.md)与[进度](../milestone61/PROGRESS.md)。

## 1. 实现选择

使用内部 Rust 静态库 `scoop_text_backend`：`idna` 1.1.0 提供 UTS #46 与原始 Punycode，`idna_adapter` 1.2.2 使用 ICU4X 2.3.0 的 normalizer 和 properties；完整依赖由独立 Cargo.lock 固定。普通文本 NFC 复用同一 normalizer，不引入 ICU4C、C++ 或第二份 Unicode 规范化实现。

固定的 normalizer／properties 数据来自 ICU release-78.1rc，Unicode 版本为 17.0.0。ICU4X 的包版本与 Unicode 数据版本分别记录。数据升级属于本库的正常更新，不在 URL 或 client 中追加另一套字符例外表。

## 2. 转换合同

采用 UTS #46 non-transitional，显式选择 `AsciiDenyList::STD3`、`Hyphens::Check` 及允许末尾根点的 DNS 长度规则。执行大小写与兼容映射、NFC、Punycode、A-label 有效性、Bidi、ContextJ 和连字符检查。ASCII 与已有 `xn--` 输入使用相同验证入口。

与 M54 的严格 IDNA2008 profile 不同，本版接受上游 UTS #46 允许的符号名称（如 `☃.example`），不另行执行 ContextO（如 `a·b.cat` 可接受），也不按旧表的 NV8／XV8 标记额外拒绝字符。这项变更不放宽 STD3、Bidi、ContextJ、连字符或 DNS 结构规则。

输入为 String，已有 core 的合法 UTF-8 保证；内嵌 NUL、控制字符、ASCII 下划线、空域名和空中间标签报 IdnaException。末尾最多一个根点，转换后保留；根域 `.` 及单个 UTS #46 Unicode 点分隔符的等价输入可以作为独立 IDNA 域名值，URL HTTP 目标另要求实际 host。DNS wire name 总长不超过 255 字节、每标签不超过 63 字节，均按 A-label 结果计算。

全角点等映射由后端完成。单标签入口拒绝任何转换后的分隔符。根域是本库既有局部边界，不对普通名称绕过上游检查。转换中不能删除非法 ASCII 字符来得到可连接字符串。

不提供 transitional 开关、IDNA2003、混淆字符检测或注册局政策。转换结果表示算法合法，不表示域名已注册、可解析或视觉上不混淆。

## 3. 公共 API

| API | 结果 |
| --- | --- |
| `Idna.parse(domain): IdnaName` | 一次验证同时取得规范 `ascii` 和 `unicode`，供传输与展示复用 |
| `Idna.toAscii(domain: String): String` | 验证后返回小写 ASCII／A-label，保留可选末尾根点 |
| `Idna.toAsciiOrNone(domain): String?` | 仅对无效域名返回 None |
| `Idna.toUnicode(domain: String): String` | 验证后返回规范 U-label；不把带错误标记的结果作为成功文本 |
| `Idna.toUnicodeOrNone(domain): String?` | 只转换正常格式失败 |
| `Idna.labelToAscii(label)`／`labelToUnicode(label)` | 单标签版本，拒绝分隔符，使用相同 profile |
| `Punycode.encode(text)`／`decode(text)` | RFC 3492 原始 codec，与域名验证分离 |

`parse` 使用 `Uts46::process` 的两个输出 sink，并对本次 ASCII 结果检查 DNS 长度；不能串联两次处理或为细分错误逐标签重跑。`IdnaName` 的构造归本库，两个字符串都是独立 Scoop 快照，不持有 native 状态。单一表示入口复用同一处理逻辑。

`IdnaException` 保存 operation、kind、可选 code、detail 和可选 labelIndex。无法细分的上游有效性错误为 InvalidLabel，不提供虚构的后端数值或标签索引。包装层直接掌握的结构、ASCII 字符或长度错误可以分别报告 InvalidLabel、Disallowed、NameTooLong 及已知标签索引。原有 InvalidAlabel、Context、Bidi 类别保留，但不承诺新后端逐项区分它们。参数及后端运行故障为 Backend，不被 OrNone 吞掉。`backendVersion` 标明包装层、idna 与 ICU4X 版本；`unicodeVersion` 标明实际数据版本。

原始 Punycode 直接复用 `idna::punycode`，不执行 IDNA 校验、映射、大小写折叠、标签拆分或 DNS 长度限制。空串往返，纯 ASCII 编码保留末尾分隔符；内嵌 NUL 和标点属于数据。非法整数序列、溢出和非 Unicode 标量报格式错误。

输入借用只在同步 FFI 调用内有效。Rust 分配的输出通过同一库的 `scoop_text_free(data, size)` 释放，不能交给 C free；Scoop 在 finally 中释放复制前的 native 结果。零长度允许 null。库不保存 managed 指针，不创建线程或执行 I/O；不因 Rust 实现而增加 GCLeaf 标注。

格式失败使用返回状态。能显式返回的参数、长度或后端故障映射为 Backend；上游 Rust 的不可恢复分配失败及内部 panic 按 `panic=abort` 终止进程，不保证转成 Scoop 异常，也不跨 C ABI 展开。此边界不把普通无效域名变成 fatal 错误，不引入通用分配预算或 panic 捕获代理。

## 4. DNS、URL 与 TLS 的使用

URL 从已解析的 host 组件调用 IDNA；百分号解码和 IP 识别属于 URL。TLS 只接收已验证的 ASCII／A-label DNS 名或 IP，不从展示文本重算。Cookie 域使用同一 A-label，公共后缀判断由 HTTP 策略数据承担。

通用 DNS 的 OsString 接口保留原始名称语义，网络 client 显式传入规范名称。`_service._tcp` 等服务发现名不属于本库的 DNS hostname profile。

## 5. 构建与验收

维护脚本从固定 Rust package 与锁定依赖准备三个目标的 `libscoop_text_backend.a` 和 C 头文件；源码及许可沿 M52 布局交付。`idna` 使用 MIT／Apache-2.0，ICU4X 与 Unicode 数据使用 Unicode-3.0。普通应用只消费 `.slib` 和静态归档，不执行 Cargo 或探测 Rust 工具链。共同归档复用现有 native library 闭包去重与目标链接规则，见实现规范 §2.8。

采用 Unicode 17.0.0 `IdnaTestV2.txt`，按本规范的参数检查，独立根点按明确的 profile 边界处理。覆盖符号和 ContextO 的正向差异，STD3、Bidi、ContextJ、伪 A-label、连字符、分隔符、空标签、63／255 字节边界、大小写及 NFC；不以旧 GNU 输出作为新 profile 的唯一 oracle。

三个目标运行真实 C ABI 调用、跨 Cone 消费、并行独立调用和定向 moving GC。公开行为在 scoop.test 中验证，不增加 fixture。更新数据时重跑 IDNA 及 URL／Cookie 的相应组合。

## 6. 依据

- [idna 1.1.0](https://docs.rs/idna/1.1.0/idna/)、[UTS #46 API](https://docs.rs/idna/1.1.0/idna/uts46/struct.Uts46.html)。
- [ICU4X 2.3.0](https://github.com/unicode-org/icu4x/releases/tag/icu%402.3.0)、[固定 ICU 数据的 Unicode 版本](https://github.com/unicode-org/icu/blob/release-78.1rc/icu4c/source/common/unicode/uchar.h)。
- [UTS #46](https://www.unicode.org/reports/tr46/)、[RFC 3492](https://www.rfc-editor.org/rfc/rfc3492.html)、[RFC 5893](https://www.rfc-editor.org/rfc/rfc5893.html)。
