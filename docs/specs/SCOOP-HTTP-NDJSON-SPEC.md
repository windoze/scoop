# NDJSON 接收规范

本扩展为 Ollama 原生流提供 HTTP body 上的逐记录读取，沿用 [HTTP 消息工具](SCOOP-HTTP-MESSAGE-SPEC.md)的内存定界与异步适配分层。该层只处理记录边界和 UTF-8；JSON 语法、服务端 error、done 与模型内容由调用方通过既有 JSON 库解释。不新增 JSON parser、网络策略或后台读取。

## 1. 记录定界

`scoop.http.NdjsonDecoder(maxRecordBytes = 1048576)` 接收 UTF-8 字节，`receive(buffer, count = buffer.readableBytes)` 消费 NativeByteBuffer 的前缀并返回实际推进量。LF 和 CRLF 结束一条记录，CRLF 可跨输入边界；其他位置的裸 CR 是 LineEnding 错误。记录的 UTF-8 字符可以跨任意输入边界，完成时由 String.fromUtf8 严格解码，非法或残缺编码保留 CharacterCodingException，不用替换字符修复。

`nextRecord(): String?` 取走一条不含行结束符的原始记录。空行返回 Some("")，空格、BOM、JSON 转义、大整数原文均不改写，不忽略空行或先行解析 JSON。此行为让上层明确选择空行策略；Ollama decoder 可将空记录按非法 JSON 处理。

最多保存一条已完成记录，形成它后 receive 立即停止；未取走记录前再次 receive 返回零。当前记录缓冲只按实际字节增长，不累计已交付记录。maxRecordBytes 必须非负，限制不包括 LF／CRLF，显式零仍允许空记录；超过限制抛 NdjsonFramingException(RecordBytes, offset, message)，offset 为当前记录内的零基字节位置。裸 CR 同样使用 NdjsonFramingException(LineEnding, offset, message)。不增加跨记录的通用预算，正文总限制沿用调用方已有 source。

`endInput()` 声明 EOF；最后一条非空且没有换行的记录也会严格解码后交付，与 Ollama 官方 Go client 的 ScanLines 行为兼容。末尾裸 CR 仍是 LineEnding 错误。空输入、末尾已有换行都不额外产生记录。endInput 幂等，已完成但未取走的记录不丢失；isFinished 只在正常 EOF 且没有待取记录时为真。

非法参数或调用状态不破坏可继续使用的引擎；实际推进／EOF 验证失败保存原异常，释放当前记录，后续 receive／nextRecord／endInput 报状态错误。isClosed、failure 仍可查询；close 幂等清理当前及待取数据。实例串行使用，重叠操作明确拒绝；任何失败均结束本次 native buffer borrow，不阻止调用方继续使用缓冲。

## 2. 异步 source

`scoop.http.asyncio.NdjsonSource(source, maxRecordBytes = 1048576, bufferSize = 8192)` 默认借用 AsyncByteSource；`takeOwnership` 要求 AsyncCloseable，成功构造后才接管。bufferSize 必须为正，maxRecordBytes 沿用内存 decoder；构造、错误 loop、登记前取消或参数失败不读取、不关闭输入，已分配本地资源正常释放。

`next(): String?` 由消费者驱动，先取已解码记录和未消费缓冲，再向底层读取。只持有一个 bufferSize 缓冲和当前记录，不后台预取；正常 EOF 后反复读取直接返回 None。底层读取必须遵守实际 buffer 推进合同，失败／取消保留原类型；不自动重试、重连或关闭借用端点。

复用 MessageIo 的单消费者、结构化子作用域和关闭合同。close 取消并等待在途操作及 borrow，再释放本地资源，仅关闭接管的端点；并发 close 等待同一次收尾。提前关闭不排空余下记录，也不承诺底层位置等于最后交付记录的边界。isFinished、isClosed、failure 提供状态查询。

依据：[NDJSON 定界规范](https://github.com/ndjson/ndjson-spec)、[Ollama 流式协议](https://docs.ollama.com/api/streaming)、[官方 Go client 的 Scanner](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/api/client.go)。验证使用普通 scoop.test 的字节／异步 mock，不连接真实服务，不使用 fixture。
