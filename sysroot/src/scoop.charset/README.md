# scoop.charset

独立字符集转换库，依赖 core、std 和 platform，使用固定 GNU libiconv 1.19。提供内存 codec 与不执行 I/O 的增量引擎，不依赖 ICU、HTTP 或系统 locale。

```scoop
import scoop.charset.*

val encoded = Charset.Gbk.encode("中文")
val text = Charset.Gbk.decode(encoded)
val selected = Charset.forName("windows-1252")
val replacement = Charset.Ascii.encodeLossy("café")
```

`encode`／`decode` 严格报告非法输入、残缺尾部和不可表示字符；有损转换显式使用 `encodeLossy`／`decodeLossy`。解码替换为 U+FFFD，编码通过同一目标编码器写出 `?`，保留 ISO-2022-JP 等编码的切换状态。没有隐式 transliteration 或静默丢弃。

常用编码有具名实例，其余通过 `forName` 查找。名称大小写无关，按后端 canonical name 比较；另支持 UTF8／UTF16／UTF32 及带 LE／BE 的无连字符别名。可用名称见 [编码列表](ENCODINGS.md)，也可读取 `Charset.availableNames`。未知名称不会退回系统编码，`forNameOrNone` 只处理这一正常查找失败。

`CharsetDecodeOptions` 控制 BOM 的 Consume／Preserve／Require，以及无明确字节序时的默认值。默认消费匹配的开头签名，保留流中间的 U+FEFF；显式字节序与反向签名冲突时报错。UTF-16／UTF-32 的无字节序名称必须有输入 BOM 或明确的默认值。`CharsetEncodeOptions` 控制输出 BOM 与无字节序名称的编码选择；String 中已有的 U+FEFF 始终作为数据处理。

`CharsetTranscoder(source, destination, options)` 是 Closeable。`process(input, output, endInput)` 接受两个 `NativeByteBuffer`，推进其读写位置并返回 consumed、produced 和 NeedInput／NeedOutput／Finished。它保留分块之间的多字节、BOM 与状态编码信息，也支持仅一个字节的输出空间。空输入或输出不足不等于 EOF；endInput 所附带的输入尚未全部消费时，后续调用继续携带 true。结束后只可用空输入读取 Finished；错误后只能关闭。

同步和 suspend 的流调用者使用同一个增量引擎：先等待输入或输出资源，再执行短暂的同步转换步骤。引擎不保留 NativeByteBorrow，不替调用者关闭输入／输出 buffer，也不把 close 当作 finish。各方法串行调用；并发使用同一个实例会明确报错。不同实例可在多个 native 线程上同时使用。

`CharsetException` 保存操作、源／目标名称、错误分类、原始输入流中的字节偏移和后端原因。增量失败前可能已经输出部分有效内容；一次性操作只在完整成功后返回结果。

固定后端在 [libiconv vendor](../../vendor/libiconv/README.md) 维护严格转换补丁。准备过程与 IDNA 共享 `build_text_backends.py`，普通应用消费预编译 `.slib` 和目标静态库。完整合同见 [字符集规范](../../../docs/specs/SCOOP-CHARSET-SPEC.md)。
