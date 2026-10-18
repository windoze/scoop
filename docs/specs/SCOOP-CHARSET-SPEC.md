# Scoop 字符集库规范

版本：M54 设计，尚未实施。提供普通 Cone `scoop:scoop.charset:0.1.0`，依赖 core、std、platform。它补齐 HTTP text 与字段参数所需的非 UTF-8 转换，负责字符集识别、内存转换和增量状态；不依赖 URL、IDNA、HTTP、asyncio 或 ICU。

## 1. 后端与范围

使用固定 GNU libiconv 1.19 的库部分，经普通 C API 集成。三个目标使用同一实现与编码集合，避免依赖 Darwin、glibc、musl 各自不同的 iconv 行为；源码、目标静态库、LGPL 许可及相应分发材料沿 M52 交付。native 库使用 libiconv 自身符号，不用进程全局 locale 或运行时后端搜索决定语义。

必须支持 UTF-8、US-ASCII、ISO-8859-1、Windows-1252、UTF-16LE／BE、UTF-32LE／BE、GB18030、GBK、Big5、Shift_JIS、EUC-JP 和 ISO-2022-JP。名称解析采用固定后端的受支持名称与显式别名，大小写无关；记录具体版本支持列表，不把未知名字退回系统默认编码。ISO-8859-1 按该编码本义处理，不采用浏览器将其当 Windows-1252 的隐式规则。

此库不改变 core String 的 UTF-8 表示，也不把 native 依赖加入仅依赖 core 的 `scoop.std`。Unicode 规范化、语言检测、浏览器编码猜测与完整国际化服务不属于本库。

## 2. 公共合同

`Charset` 为不可变配置，提供常见编码的具名实例和 `forName`／`forNameOrNone`。普通未知名称抛 UnsupportedCharsetException，OrNone 仅转换这一正常查找失败。`decode(data: List<Byte>): String` 与 `encode(text: String): Array<Byte>` 返回独立快照，默认严格处理非法序列或不能表示的字符。

显式有损入口名为 `decodeLossy`／`encodeLossy`：解码使用 U+FFFD，编码使用能在目标编码中表示的 `?`；不启用隐式 transliteration 或静默丢弃。状态编码的 shift 状态与错误恢复由转换器管理，调用者不能跳过字节并假定剩余状态仍然有效。

显式字节序的 UTF-16／32 不以 BOM 改变已经指定的字节序。BOM 是否作为签名消费由配置表达；默认仅消费与所选编码一致的开头签名，冲突时报错，不在流中间删除 U+FEFF。无明确字节序的名称须给出 BOM 或显式默认字节序，不取宿主字节序。

`CharsetTranscoder(source, destination, options)` 为 Closeable 的增量引擎；process 使用 NativeByteBuffer 的实际可读／可写前缀，报告 consumed、produced、NeedInput／NeedOutput／Finished。endInput 明确输入结束，保留跨块的部分多字节序列和状态编码状态；严格模式的残缺尾部在结束时失败。所有计数为 Long，NativeByteBorrow 在本次同步步骤后结束。

处理空输入与暂时无输入不等于 EOF，输出空间不足不等于格式错误。数据错误保存原始输入流中的字节偏移，不把每块起点当成零。`CharsetException` 保存操作、源／目标编码、kind、offset 和后端原因。输入有错误时可能已有输出，增量入口不提供回滚；一次性转换只在完整成功后交付结果。

引擎串行使用，不执行 I/O。HTTP 等流适配逐块调用它即可保持同步／异步流的相同编码语义，不得在上层另写 GBK、UTF-16 等 decoder。只需要现有 UTF-8 能力的库继续使用 core 的公开入口，无须重复引入转换实现。

## 3. HTTP 使用与验收

Content-Type 参数解析属于 HTTP；HTTP client 根据显式 charset、调用方选择和协议规定选择 Charset。本库不猜测媒体类型。JSON 的网络表示采用 UTF-8，SSE 采用其规定的 UTF-8；HTTP 历史字段的原始字节不被整体当成 UTF-8 解码。

固定向量覆盖编码双向转换、不等价字符、非法输入、不可表示输出、BOM 与字节序、任意分块及状态编码。与独立 libiconv 工具交叉验证，再组合短读 HTTP body、Content-Disposition 扩展参数和取消后的引擎清理。三个目标执行库测试和普通生产消费者，工具只用于开发验证。

依据：[GNU libiconv](https://www.gnu.org/software/libiconv/)、[固定版本发行](https://ftp.gnu.org/gnu/libiconv/)、[RFC 8187](https://www.rfc-editor.org/rfc/rfc8187.html)。
