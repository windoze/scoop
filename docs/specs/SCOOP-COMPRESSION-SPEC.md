# Scoop 压缩库规范

版本：M54，实施中。内存与增量引擎已实现，同步／异步流和 RawDeflate 消息适配继续实施；实际验证见[进度](../milestone54/PROGRESS.md)。范围与实施次序见 [M54 设计](../milestone54/DESIGN.md)。公共命名、字节进度、资源和取消分别遵守[库 API 风格](SCOOP-API-STYLE.md)、[平台库](SCOOP-PLATFORM-SPEC.md)和[异步 I/O](SCOOP-ASYNCIO-SPEC.md)。

## 1. Cone、格式与后端

提供独立的普通 Cone `scoop:scoop.compression:0.1.0`，依赖 core、std、platform、asyncio。内存 API、同步流和异步流在同一个 Cone 中交付；异步声明位于 `scoop.compression.asyncio`，不再拆出第二个压缩 Cone。各入口共享一套 native codec，不依赖 HTTP，也不解释 Content-Encoding。

| 格式 | 固定后端 | 本期用途 |
| --- | --- | --- |
| Gzip、Zlib、RawDeflate | zlib-ng 2.3.3，使用原生命名 API | gzip 文件与 HTTP gzip；HTTP deflate；WebSocket permessage-deflate 的原始 DEFLATE |
| Brotli | Brotli 1.2.0 的 encoder、decoder、common | HTTP br 的压缩与解压 |
| Zstd | Zstandard 1.5.7 | 通用压缩的默认选择，以及 HTTP zstd |

`CompressionFormat { Gzip, Zlib, RawDeflate, Brotli, Zstd }` 表达不同格式；默认的通用压缩配置使用 Zstd level 3。各格式均支持压缩和解压，不以仅能解压冒充格式支持。gzip／zlib 包装层不同；HTTP 的 `deflate` 表示 RFC 1950 zlib 包装的 DEFLATE，不能遇错后静默改按 raw DEFLATE 重试。Zstd 不能代替另三种 HTTP 内容编码。

不提供 ZIP／tar 归档、文件目录管理、压缩包路径提取、并行压缩调度器或 HTTP 字典协商。本期的字典支持限于 zstd 的显式预共享字典；字典训练没有当前调用需求，不加入公开接口。

## 2. 配置、内存操作与错误

编码配置为按算法区分的普通值：Gzip／Zlib／RawDeflate 的 level、windowBits 和 strategy，Brotli 的 quality、windowBits 和 mode，Zstd 的 level、checksum 与可选 dictionary。参数范围按固定后端检查；不设计会把无关参数静默忽略的通用选项 Map。Gzip 默认使用不含路径、固定 mtime 的头，使同一配置与输入可重复；明确提供文件名等元数据时才写入相应字段。

`CompressionOptions` 的五个 variant 分别持有 `GzipOptions`、`DeflateOptions`、`DeflateOptions`、`BrotliOptions`、`ZstdOptions`。DEFLATE 默认 level 6、windowBits 15、Default strategy，编码窗口明确为 9～15；不接受后端会放大成 9 的 8 位编码窗口。Brotli 默认 quality 5、windowBits 22、Generic mode，分别接受 0～11、10～24 和 Generic／Text／Font。Zstd 默认 level 3、checksum true，等级按固定后端实际范围校验。

`GzipMetadata` 提供可选 filename／comment 字节、UInt 类型的 Unix 秒数（默认 0）和 headerCrc。文件名／注释按 RFC 1952 的字节字段保存，复制调用方输入并拒绝嵌入 NUL；不隐式截断路径或猜测文本编码。固定 OS 标记为 unknown，使相同配置的头部不随宿主变化。

解码配置包含所选格式、可选 zstd dictionary，以及实际所需的最大输出字节数和 zstd 最大窗口。缺省不设总输出上限；调用者可以在处理网络或文件数据时给出具体限制。这些限制只约束当前解码对象，不是跨库预算或计费。分配、计数和后端整数转换须做实际溢出检查。

`DecompressionOptions` 提供 format、dictionary、maxOutputBytes、maxWindowBytes 和 Automatic／Single 模式。字典与最大窗口参数仅适用于 zstd，其他格式收到这些选项时报参数错误；未指定 zstd 窗口时采用固定后端的默认 128 MiB。Single 令内存引擎在第一个 frame／member 结束时返回 Finished，并保留 input buffer 中的后缀；完整内存操作和流适配仍要求所给输入没有尾随数据。

必须提供以下表面；省略的具体配置字段遵守上表，不靠实现中的隐藏默认值决定格式。

| API | 结果与边界 |
| --- | --- |
| `compress(data: List<Byte>, options): Array<Byte>` | 完整压缩并完成流，返回独立快照 |
| `decompress(data: List<Byte>, options): Array<Byte>` | 完整解压并检查结束状态；尾随数据遵守第 4 节 |
| `CompressionEncoder(options)`、`CompressionDecoder(options)` | 显式 Closeable 的增量内存引擎 |
| `CompressionDictionary.fromByteArray(data)` | 创建独立拥有的 zstd 字典；实现 Closeable |

引擎保留已经接收的配置与字典引用。关闭传入的字典 wrapper 不使正在使用它的引擎失效；native 引用或独立副本由压缩库自己管理。外部数组后续变动不能改变配置或字典。

`CompressionException` 保存 operation、format、kind、后端原码和已复制 detail。kind 至少区分 InvalidData、Truncated、DictionaryRequired、DictionaryMismatch、OutputLimit、WindowLimit 和 Backend。参数、状态、内存分配错误使用实际对应异常；native 分配失败对应 PlatformException／NoMemory，计数或分配大小溢出对应 ArithmeticException。底层 I/O 异常保留原类型。校验和不匹配属于 InvalidData，不交付一个宣称完整的成功结果。

## 3. 共享增量引擎

编码器与解码器以 NativeByteBuffer 的可读／可写前缀推进，返回本次 consumed、produced 和状态。状态区分 NeedInput、NeedOutput、Finished；发生进度后可以同时需要后续输入或输出，不以零进度表示 EOF。只有显式 endInput 或编码 finish 才能宣告不会再有输入。

编码器的 `process(input, output, count)` 接受原文，`flush(output)` 与 `finish(output)` 只推进已有状态。flush 返回 NeedInput 表示同步点已完全取走，finish 返回 Finished 表示结束标记已完全取走。解码器的 `process(input, output, count, endInput)` 在本次 count 尚未全部消费时要求后续继续携带 endInput。调用方可以使用比编码器输出单元更小的 buffer；库内保留有限的待交付字节，实际消耗与交付仍分别报告。

输入与输出使用不同 buffer；每次调用只消费合法前缀、初始化实际输出并推进相应游标。窗口、整数和后端单次输入范围均在真实边界检查。后端只持有自己的状态，不能保存 managed Array／String 的临时地址。临时 native 借用在该步返回前完成；输出进入异步下游前不得仍被 codec 借用。

一个引擎只允许串行推进，包括关闭。状态为 Open、Finishing、Finished、Failed、Closed；Finished 后不能继续追加输入，失败后只允许清理和读取已经保存的错误信息。不通过 reset 让残缺流成为另一条流；创建新的引擎开启新流。

`flush` 排出足以让解码端取得当前输入的格式内同步点，流仍可继续。`finish` 提交格式结束并排空输出；完成过程可能多次需要输出空间。`close` 只释放本地状态，不把未 finish 的流变成完整流，不隐含任何传输。参数错误在修改前报告；已接受的输入和已产生的输出不回滚。

RawDeflate 另提供 WebSocket 所需的消息边界操作：生成／识别同步 flush 边界、按 RFC 7692 处理末尾 `00 00 ff ff`，并支持协商后的 windowBits 与 context takeover。此能力属于压缩库；WebSocket 库负责 RSV1 和扩展协商，不访问 zlib 状态或自行修补压缩数据。发送方必须实际支持所协商的窗口，不能以更大的窗口冒充较小窗口。

## 4. 结束、连接流与尾随字节

gzip 解码默认支持连续 member；zstd 默认支持连续 frame 及标准 skippable frame。两者持续到输入来源正常 EOF，任一 member／frame 截断或损坏都报错。zlib、raw DEFLATE 与 Brotli 默认为单条流，完成后额外非空数据是尾随数据错误。

内存引擎在单条流完成时保留未消费后缀，由调用者选择继续解析下一项或报告尾随数据。一次性 API 与解压流遵循上述格式的默认边界，不能静默丢弃后缀。需要嵌在更大协议中的单流解码时，由显式单流模式报告已消费长度；下游协议仍拥有后缀。

源 EOF 时解码器尚未完整结束，抛 Truncated；压缩流的正常 EOF 只在结束标记已生成并全部取走之后交付。空原文与空压缩输入不同：压缩空原文产生该格式的合法空流，空压缩输入通常是截断；RawDeflate 的消息适配按其单独合同判断空消息。

校验错误可能在一部分明文已经交付后出现，流 API 不承诺事务回滚。只有成功读到完整 EOF 才能提交需要完整性判断的缓存或下载结果。

## 5. 同步与异步流

提供 CompressionSource、CompressionSink、DecompressionSource、DecompressionSink，即从原文 source 读取压缩字节、向压缩 sink 写入原文，以及反方向的两个解压适配。同步适配实现 ByteSource／ByteSink 与 Closeable；异步适配分别使用 Async 前缀，实现 AsyncByteSource／AsyncByteSink 与 AsyncCloseable。只读 source 不伪造 write 或 flush 能力。

| 操作 | 必须遵守的合同 |
| --- | --- |
| `readInto(buffer, count)` | 返回交付给调用者的字节数；解压 source 交付明文，压缩 source 交付压缩字节 |
| `writeFrom(buffer, count)` | 返回从调用者接受的字节数；压缩 sink 接受明文，解压 sink 接受压缩字节 |
| `flush()` | 推进本层可提交输出并调用下游 flush；不等于 finish 或磁盘 sync |
| `finish()` | sink 完成当前编码／解码输入，检查结束并写完输出；成功后不可再写，不关闭借用端点 |
| `compressTo`／`decompressTo` | 复制至 EOF、完成格式状态并返回输入／输出 Long 计数；不接管端点，不隐含文件持久化 |

source 默认借用输入，sink 默认借用输出，仅拥有自身引擎和缓冲。`takeOwnership` 工厂要求端点同时实现对应关闭协议；成功交付时移交关闭责任，交付前的失败只清理新建状态。关闭接管的包装层时继续关闭端点；借用形式不关闭端点。下游仅有字节协议时也可使用借用形式。

bufferSize 约束实际中间缓冲，不限制总数据量。适配器以有限前缀推进，正确处理短读、短写、压缩膨胀和没有输出的输入；不得先读取完整源再伪装成流。异步包装的下游 I/O、上游 I/O 和等待均通过 suspend 入口，不能在 EventLoop 线程调用同步文件／socket API。codec 每次推进是有限的同步计算，不宣称转移到其他线程或提供硬实时抢占。

异步操作要求活动 Job 和所属 EventLoop。单个适配器同一方向最多一个操作；等待真实输入、输出空间或关闭完成时挂起，不循环轮询。无下游进度时传播背压，不无限积累输出。

## 6. 取消与清理

登记前取消不推进引擎或端点。已开始的操作在取消获胜后进入失败终态，提交已知实际进度，结束底层操作与 buffer 借用后才恢复。包装层不能在仍被 native 使用时释放 buffer；不能用 suspendCancellableCoroutine 的立即取消代替 I/O 收尾。

异步 close 可在已取消 Job 的 finally 中完成；并发 close 等待同一轮收尾，之后幂等。先拒绝新操作，再完成活动操作的本地收尾、释放 codec 与缓冲、关闭明确接管的端点。借用端点因其自身取消合同而终止的可能性照常保留。

close 不补写结束标记。需要完整压缩内容的调用者必须正常调用 finish 或使用 compressTo；正文取消时不能在 finally 中强行 finish、继续消费源或无界排空网络。多个必要清理步骤继续执行，正文与清理同时失败使用 ResourceUseException。GC 不执行 flush、finish、取消或 I/O。

## 7. 构建与验收

三套源码、版本和许可证进入 M52 vendor 布局；三个目标分别提供静态库与头文件。zlib-ng 使用自身前缀，避免与其他依赖的 zlib 符号相撞。压缩 Cone 各声明一次 native 库，异步适配不复制对象。上游命令行程序、测试和 benchmark 不进入生产包；维护脚本负责准备，普通应用不下载、不探测或重建后端。

验收使用固定格式向量及独立 gzip／Brotli／zstd 工具双向互操作，不能只用同一 wrapper 压缩后再解压。覆盖空流、任意分块、短传输、flush 后继续写、finish、截断、坏校验和、尾随数据、连续 member／frame、字典、输出与窗口限制、原始 DEFLATE 消息边界、取消、提前关闭和 moving GC。

每个同步能力须有异步对应场景；验证流式压缩上传、流式解压下载和 HTTP/2 并发下载之间的背压与取消隔离。三个目标运行库测试和普通跨 Cone 消费者；独立工具仅为开发验证依赖。

## 8. 依据

- [zlib-ng 2.3.3](https://github.com/zlib-ng/zlib-ng/releases/tag/2.3.3)、[zlib manual](https://zlib.net/manual.html)。
- [Brotli 1.2.0](https://github.com/google/brotli/releases/tag/v1.2.0)、[RFC 7932](https://www.rfc-editor.org/rfc/rfc7932.html)。
- [Zstandard 1.5.7](https://github.com/facebook/zstd/releases/tag/v1.5.7)、[RFC 8878](https://www.rfc-editor.org/rfc/rfc8878.html)。
- [RFC 9110 内容编码](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.4)、[RFC 7692](https://www.rfc-editor.org/rfc/rfc7692.html)。
