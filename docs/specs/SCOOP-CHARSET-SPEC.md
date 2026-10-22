# Scoop 字符集库规范

版本：M61（分步迁移）。提供普通 Cone `scoop:scoop.charset:0.1.0`，依赖 core、std、platform。它补齐 HTTP text、字段参数和认证凭据所需的字符集转换与 NFC 规范化，负责字符集识别、内存转换和增量状态；不依赖 URL、IDNA、HTTP、asyncio 或 ICU4C。M54 的原验收范围见 [M54 进度](../milestone54/PROGRESS.md)，迁移状态见 [M61 进度](../milestone61/PROGRESS.md)。

## 1. 后端与范围

内部共享 Rust 静态库 `scoop_text_backend` 使用 encoding_rs 0.8.42 负责遗留编码和 UTF-8／16 解码，Rust 标准库的 Unicode 标量与端序转换负责 UTF 编码、UTF-32、真正的 ASCII／Latin-1。三个目标使用同一固定实现与 43 个 canonical name，实际全集及别名、65 个退役名称和映射差异见 [ENCODINGS.md](../../sysroot/src/scoop.charset/ENCODINGS.md)。没有系统 iconv、进程 locale、CJK／ISO-2022 自实现或 GNU fallback。

支持 UTF-8、US-ASCII、ISO-8859-1、Windows-1252、UTF-16LE／BE、UTF-32LE／BE、GB18030、GBK、Big5、Shift_JIS、EUC-JP 和 ISO-2022-JP。遗留编码采用 encoding_rs 的 WHATWG 映射；GB18030 为带网页兼容差异的 2022 变体，不保留 2005／2022 年份别名。GBK 解码兼容四字节 GB18030，但编码限 GBK；Big5 可解码为多个标量；Shift_JIS、EUC-KR 包含厂商扩展；Windows 未定义字节、EUC-JP 反向集合与 CP1258 组合行为均以映射文档为准。严格转换拒绝当前映射中的非法输入／不可表示字符，不额外承诺一一映射或旧 GNU 行为。

NFC 使用 ICU4X `icu_normalizer` 2.3.0，与 IDNA 共享 normalizer 和 Unicode 17.0.0 数据。Readline Unicode 操作见 §2.1，全部使用同一 Rust 静态库。此库不改变 core String 的 UTF-8 表示，也不把 native 依赖加入仅依赖 core 的 `scoop.std`。语言检测、浏览器编码猜测与完整国际化服务不属于本库。

## 2. 公共合同

`Charset` 为不可变配置，提供常见编码的具名实例和 `forName`／`forNameOrNone`。普通未知名称抛 UnsupportedCharsetException，OrNone 仅转换这一正常查找失败。`decode(data: List<Byte>): String` 与 `encode(text: String): Array<Byte>` 返回独立快照，默认严格处理非法序列或不能表示的字符。

名称解析接受固定后端的 locale-independent 名称，拒绝空名、`CHAR`／`WCHAR_T`、宿主内部／交换字节序名称及 `//TRANSLIT`／`//IGNORE` 等转换后缀。`availableNames` 列出可用名称；`backendVersion` 记录固定版本。别名解析后以 canonical name 比较和 hash，不通过当前 locale 补全名称。

除后端名称外，显式补充不带连字符的 UTF8、UTF16、UTF16LE、UTF16BE、UTF32、UTF32LE、UTF32BE，分别映射到对应 UTF 标准名称；这些别名也包含在 availableNames 中。大小写规则相同，不进行任意删标点或模糊匹配。

显式有损入口名为 `decodeLossy`／`encodeLossy`：解码使用 U+FFFD，编码使用能在目标编码中表示的 `?`；不启用隐式 transliteration 或静默丢弃。状态编码的 shift 状态与错误恢复由转换器管理，调用者不能跳过字节并假定剩余状态仍然有效。

显式字节序的 UTF-16／32 不以 BOM 改变已经指定的字节序。BOM 是否作为签名消费由配置表达；默认仅消费与所选编码一致的开头签名，冲突时报错，不在流中间删除 U+FEFF。无明确字节序的名称须给出 BOM 或显式默认字节序，不取宿主字节序。

`CharsetDecodeOptions` 提供 `bom`（Consume／Preserve／Require）和可选 `defaultByteOrder`；Preserve 将匹配的签名作为 U+FEFF 数据保留，Require 在缺少签名时失败。无字节序的 UTF-16／32 优先采用输入签名，无签名时才使用显式默认值。显式字节序与反向签名冲突在所有模式下失败。非 Unicode 签名编码不能使用 Require。

`CharsetEncodeOptions` 提供 `emitBom` 和可选 `byteOrder`。无字节序的 UTF-16／32 编码必须选择字节序；其他明确编码不得被此选项悄悄改变。emitBom 只对 UTF-8／16／32 有意义。encode 的 String 输入始终按数据保留开头 U+FEFF，只有显式 emitBom 才另写签名。

`CharsetTranscoder(source, destination, options)` 为 Closeable 的增量引擎；process 使用 NativeByteBuffer 的实际可读／可写前缀，报告 consumed、produced、NeedInput／NeedOutput／Finished。endInput 明确输入结束，保留跨块的部分多字节序列和状态编码状态；严格模式的残缺尾部在结束时失败。所有计数为 Long，NativeByteBorrow 在本次同步步骤后结束。

`CharsetTranscodeOptions` 分别保存上述 decode／encode 选项，以及 malformedInput／unmappableOutput 的 Strict 或 Replace 策略。增量 `process(input, output, endInput = false)` 推进两个 buffer 的实际位置；endInput 只在本次提供的输入全部接收后生效，未消费完时后续调用继续携带 true。Finished 后允许空输入查询完成状态，不能继续追加数据。转换错误后只允许 close；close 幂等，资源最终释放不冒充成功 finish。

native 引擎保留成熟库的 decoder／encoder 与有限的跨块字节，逐编码单元传递 Unicode 标量；Big5 等解码出的多个标量关联到同一源单元。源错误采用上游无替换接口报告的长度与已消费字节定位，Replace 输出 U+FFFD；不可表示输出在同一个编码器中编码 U+003F，不使用 HTML 字符引用。明确 EOF 的残缺尾部为 IncompleteInput，其余非法编码单元为 MalformedInput。不可表示字符的偏移指向对应源单元起点，包括该单元之前尚未产生字符的状态转义；偏移不因分块变化。

处理空输入与暂时无输入不等于 EOF，输出空间不足不等于格式错误。数据错误保存原始输入流中的字节偏移，不把每块起点当成零。`CharsetException` 保存操作、源／目标编码、kind、offset 和后端原因。输入有错误时可能已有输出，增量入口不提供回滚；一次性转换只在完整成功后交付结果。

引擎串行使用，不执行 I/O。HTTP 等流适配逐块调用它即可保持同步／异步流的相同编码语义，不得在上层另写 GBK、UTF-16 等 decoder。只需要现有 UTF-8 能力的库继续使用 core 的公开入口，无须重复引入转换实现。

`normalizeNfc(text: String): String` 对完整 String 执行 Unicode NFC，返回拥有独立生命周期的 String。规范分解、组合顺序、组合排除和 Hangul 组合均使用 ICU4X；不做兼容性折叠、大小写映射、空白删除或 BOM 消费。空字符串、内嵌 NUL 与 U+FEFF 都是普通文本。各次调用没有共享可变状态；Scoop 复制 Rust 结果后在 finally 中经 `scoop_text_free(data, size)` 释放。显式返回的参数、长度及后端故障为 CharsetException.Backend，无后端原码时为 None；上游不可恢复分配失败或内部 panic 按 `panic=abort` 终止进程，不保证转成可捕获异常，也不跨 C ABI 展开。它不对任意流分块分别规范化，因为组合字符可以跨块关联；调用者明确提供完整文本。

### 2.1 Unicode 分段、宽度与大小写

Readline 所需的 Unicode 基础能力使用 ICU4X 2.3.0 的 segmenter、casemap、properties
及 unicode-width 0.2.2，数据基线均为 Unicode 17.0.0，无 GNU 后备实现。
`graphemeBoundaries(text)` 返回扩展字素簇边界的 Unicode 标量索引（含 0 和末尾，
空串返回 [0]），`graphemes(text)` 返回各簇的独立 String 数组；不使用缺少上下文的
逐字符 break 判断。分段采用 UAX #29 扩展字素簇规则，底层 UTF-8 byte offset 只在
适配边界转换为标量索引。`unicodeScalarWidth(character)` 使用 unicode-width 的非 CJK
单标量宽度；控制字符（包括 NUL）返回 -1，组合字符通常为 0，东亚宽字符为 2，
少数上游定义的宽符号可为 3（例如 U+17D8）；不把返回值限制在 0..2，也不读取进程 locale。
`isUnicodeAlphanumeric` 明确定义为 ICU 的 Alnum（Alphabetic 或 Decimal_Number），
包括具有 Alphabetic 属性的组合符号；这取代原 GNU ISO C 分类，非十进制且非 Alphabetic
的数字符号（例如 ²、½）不属于该集合。
`unicodeLowercase/unicodeUppercase` 使用完整字符串上下文与 und locale，允许一个标量
展开为多个标量，保留希腊词尾 sigma 等上下文规则；`unicodeCasefold` 使用默认的非 Turkic
完整折叠。空串和内嵌 NUL 是普通数据，拥有的结果经 `scoop_text_free(data, size)` 释放，
显式错误及不可恢复 panic／OOM 遵守 NFC 的边界。终端序列与字素簇显示策略属于 readline，
不改变 core String 的标量索引语义。

## 3. HTTP 使用与验收

Content-Type 参数解析属于 HTTP；HTTP client 根据显式 charset、调用方选择和协议规定选择 Charset。本库不猜测媒体类型。JSON 的网络表示采用 UTF-8，SSE 采用其规定的 UTF-8；HTTP 历史字段的原始字节不被整体当成 UTF-8 解码。

Basic／Digest 在选择 UTF-8 凭据时按 RFC 7617／7616 调用 normalizeNfc，再编码和计算凭据；字段语法、凭据选择和摘要分别保留在 HTTP、client 和 crypto。NFC 的独立向量覆盖组合、重排、排除、Hangul、非兼容折叠、NUL／BOM，并组合字符集往返与实际认证字段和摘要。

固定向量覆盖编码双向转换、不等价字符、非法输入、不可表示输出、BOM 与字节序、任意分块及状态编码。用官方映射数据和明确的迁移差异交叉验证，再组合短读 HTTP body、Content-Disposition 扩展参数和取消后的引擎清理。三个目标执行库测试和普通生产消费者，工具只用于开发验证。

依据：[encoding_rs](https://docs.rs/encoding_rs/0.8.42/encoding_rs/)、[WHATWG Encoding](https://encoding.spec.whatwg.org/)、[UAX #15](https://www.unicode.org/reports/tr15/)、[RFC 8187](https://www.rfc-editor.org/rfc/rfc8187.html)、[RFC 7616](https://www.rfc-editor.org/rfc/rfc7616.html)、[RFC 7617](https://www.rfc-editor.org/rfc/rfc7617.html)。
