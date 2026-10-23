# scoop.charset

独立字符集转换库，依赖 core、std 和 platform，字符集转换使用固定 Rust encoding_rs 0.8.42，NFC 使用与 IDNA 共享的 Rust ICU4X 2.3.0。readline 的字素分段、属性及完整大小写使用 ICU4X，单标量列宽使用 unicode-width 0.2.2，统一为 Unicode 17.0.0。提供内存 codec、NFC 与增量引擎，不依赖 HTTP 或系统 locale。

```scoop
import scoop.charset.*

val encoded = Charset.Gbk.encode("中文")
val text = Charset.Gbk.decode(encoded)
val selected = Charset.forName("windows-1252")
val replacement = Charset.Ascii.encodeLossy("café")
val username = normalizeNfc("Jose\u{0301}") // José
```

`encode`／`decode` 严格报告非法输入、残缺尾部和不可表示字符；有损转换显式使用 `encodeLossy`／`decodeLossy`。解码替换为 U+FFFD，编码通过同一目标编码器写出 `?`，保留 ISO-2022-JP 等编码的切换状态。没有隐式 transliteration 或静默丢弃。

常用编码有具名实例，其余通过 `forName` 查找。名称大小写无关，按后端 canonical name 比较；另支持 UTF8／UTF16／UTF32 及带 LE／BE 的无连字符别名。可用名称见 [编码列表](ENCODINGS.md)，也可读取 `Charset.availableNames`。未知名称不会退回系统编码，`forNameOrNone` 只处理这一正常查找失败。

`CharsetDecodeOptions` 控制 BOM 的 Consume／Preserve／Require，以及无明确字节序时的默认值。默认消费匹配的开头签名，保留流中间的 U+FEFF；显式字节序与反向签名冲突时报错。UTF-16／UTF-32 的无字节序名称必须有输入 BOM 或明确的默认值。`CharsetEncodeOptions` 控制输出 BOM 与无字节序名称的编码选择；String 中已有的 U+FEFF 始终作为数据处理。

`CharsetTranscoder(source, destination, options)` 是 Closeable。`process(input, output, endInput)` 接受两个 `NativeByteBuffer`，推进其读写位置并返回 consumed、produced 和 NeedInput／NeedOutput／Finished。它保留分块之间的多字节、BOM 与状态编码信息，也支持仅一个字节的输出空间。空输入或输出不足不等于 EOF；endInput 所附带的输入尚未全部消费时，后续调用继续携带 true。结束后只可用空输入读取 Finished；错误后只能关闭。

同步和 suspend 的流调用者使用同一个增量引擎：先等待输入或输出资源，再执行短暂的同步转换步骤。引擎不保留 NativeByteBorrow，不替调用者关闭输入／输出 buffer，也不把 close 当作 finish。各方法串行调用；并发使用同一个实例会明确报错。不同实例可在多个 native 线程上同时使用。

`CharsetException` 保存操作、源／目标名称、错误分类、原始输入流中的字节偏移和后端原因。增量失败前可能已经输出部分有效内容；一次性操作只在完整成功后返回结果。

`normalizeNfc(text)` 用 ICU4X 的 Unicode 17.0.0 数据 对完整 String 做 Unicode NFC，处理组合顺序、组合排除与 Hangul，返回拥有独立生命周期的字符串。它保留内嵌 NUL 和 BOM，不做兼容性折叠或大小写转换，也不隐含在 encode／decode 中执行。Basic／Digest 的 UTF-8 凭据可以先调用此函数，再编码与计算摘要。不要对流的任意分块分别调用它，因为组合字符可以跨块关联。

固定后端和许可在 [Rust vendor](../../vendor/text_backend/README.md) 维护。准备过程与 IDNA 共享 `build_text_backends.py`，普通应用消费预编译 `.slib` 和目标静态库。完整合同见 [字符集规范](../../../docs/specs/SCOOP-CHARSET-SPEC.md)。

M61 按用户确认缩减到 43 个 canonical name，65 个旧名称及其别名退役。GB18030 年份、CJK 扩展和 Windows 单字节映射差异见编码列表；不保证严格转换可逐标量往返。名称／NFC 的 Rust 输出在复制后由同一后端释放；上游不可恢复 OOM／panic 终止进程，不保证转为 Scoop 异常。详见 [迁移进度](../../../docs/milestone61/PROGRESS.md)。

`graphemeBoundaries`／`graphemes` 使用扩展字素簇，公开索引仍按 Unicode 标量计数。大小写映射使用完整字符串上下文和 und locale；默认完整折叠不采用 Turkic 特例。`isUnicodeAlphanumeric` 对应 Alphabetic 或 Decimal_Number，包括字母性组合符号。`unicodeScalarWidth` 使用非 CJK 宽度，NUL／控制字符为 -1，少数符号可为 3 列；终端的字素显示策略仍由 readline 负责。GNU 文本后端已全部移除。
