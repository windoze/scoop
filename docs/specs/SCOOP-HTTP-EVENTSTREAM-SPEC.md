# AWS EventStream 接收规范

本扩展为 Bedrock ConverseStream 提供已有 HTTP body 上的二进制消息读取，沿用 [HTTP 消息工具](SCOOP-HTTP-MESSAGE-SPEC.md)的内存引擎与异步适配分层。不改变编译器、runtime ABI 或 HTTP 连接策略。

## 1. 消息与类型

`scoop.http.AwsEventStreamDecoder()` 解析 `application/vnd.amazon.eventstream`。输出 `AwsEventStreamMessage` 保存 headers 与原始 payload 字节；`header(name)` 按区分大小写的名称查询可选值。headers 保留线上顺序，名称必须是非空、至多 255 字节的严格 UTF-8，同一帧内不能重复。

`AwsEventStreamHeaderValue` 区分 Bool、Int8、Int16、Int32、Int64、Bytes、Text、Timestamp、Uuid；整数使用相应的有符号类型，Timestamp 使用由有符号 epoch milliseconds 构造的 Instant，Uuid 使用既有 scoop.uuid.Uuid。Bytes／Text 的 wire 长度是 unsigned 16-bit，允许空值，Text 严格解码 UTF-8。固定类型代码 0～9 以外的值不能推测长度或跳过。消息与 header 构造器由解码器内部使用；本步不增加没有发送方调用的编码器。

规范的 timestamp 描述和伪代码存在 signed／unsigned 差异，字符串长度伪代码也与 uint16 前缀描述不同；这里依据其 int64 伪代码及[官方 SDK 的 Int64、uint16 实现](https://github.com/smithy-lang/smithy-typescript/blob/a90c7be279b3b1e6459b0f2806bfc360800922f1/packages/core/src/submodules/event-streams/eventstream-codec/HeaderMarshaller.ts)保留负时间和完整前缀范围，不截断值。

## 2. 增量读取与失败

所有多字节字段采用大端。读取 8 字节长度字段及 4 字节 prelude CRC 后，先验证 prelude CRC，再使用 totalLength／headersLength 定界；totalLength 至少 16，headersLength 不大于 totalLength−16。最后的 message CRC 覆盖整个前缀、前缀 CRC、headers 和 payload，不包括自身；验证成功后才解释 headers 并交付消息。CRC32 复用仓库已有的 zlib-ng 2.3.3 `zng_crc32_z`，scoop.http 显式声明相应 native 链接，不依赖测试库的链接副作用。

`receive(buffer, count = buffer.readableBytes)` 消费 NativeByteBuffer 的实际前缀，返回已消费字节数；任意字段和 UTF-8 可以跨输入边界。最多保存一条待交付消息，形成消息后立即停止，`nextMessage()` 取走它之前 receive 返回零。只按实际到达字节增长当前帧的存储，不按远端声明长度预分配整个消息，也不累计已交付帧。应用的正文限制由已有 source／调用设置承担；不将服务端专用的 24 MiB payload 或 128 KiB header 限制强加给客户端。

`endInput()` 幂等地声明 EOF；没有未完成帧时正常结束，已完成但未取走的消息仍可取。残缺前缀、header、payload 或 CRC 都是 UnexpectedEof，不补齐消息。`isFinished` 只在正常 EOF 且没有待交付消息时为真。`close()` 幂等释放缓冲与待交付消息，不读取或关闭调用者的输入；isClosed、failure 仍可查询。

`AwsEventStreamException` 保存 kind、当前帧内 offset 与说明；kind 区分 Length、PreludeChecksum、MessageChecksum、Header、UnexpectedEof。非法参数或调用状态不破坏尚可使用的引擎；实际推进或 EOF 验证失败保存原异常，后续 receive／nextMessage／endInput 报状态错误，close 仍有效。串行使用同一实例，在重叠推进时明确拒绝。CRC 或解析失败不交付当前帧，已交付消息不回滚。

## 3. 异步 source

`scoop.http.asyncio.AwsEventStreamSource(source, bufferSize = 8192)` 默认借用 AsyncByteSource，`takeOwnership` 要求 AsyncCloseable 并在构造成功后接管。构造不读取输入，bufferSize 必须为正。配置、EventLoop、活动 Job 或登记前取消检查失败时不取得端点所有权，已分配的本地资源照常释放。

`next()` 由消费方驱动，先读取已有消息与缓冲，再向底层读取；只维持一个 bufferSize 缓冲和解码器的当前帧，没有后台预取任务。正常 EOF 后重复读取直接返回 None。底层读取必须符合缓冲推进合同；解析／I/O 失败及取消保存原类型，不自动重试或重连。isFinished、isClosed、failure 提供状态快照。

复用 MessageIo 的单消费者、结构化子作用域与关闭合同。错误 loop、登记前取消和并发读拒绝不破坏源；实际读取开始后的失败终止推进。close 取消并等待在途读取及 borrow 结束，再释放本地资源，仅关闭所接管端点；并发 close 等待同一次收尾，后续幂等。借用源关闭时可放弃已预取前缀，不承诺底层位置等于最后一帧结尾。

HTTP 状态、媒体类型以及 `:message-type`、`:event-type`、`:exception-type` 等 header 的业务含义由调用方解释。该层不解码 JSON、不合并 Converse 内容、不生成服务端工具活动。

依据：[AWS EventStream 规范](https://smithy.io/2.0/aws/amazon-eventstream.html)。测试使用 scoop.test，包含官方 SDK 的独立固定字节向量、分块与背压、字段和 CRC 错误、EOF、借用／接管及取消；不使用编译器 fixture 或真实服务。
