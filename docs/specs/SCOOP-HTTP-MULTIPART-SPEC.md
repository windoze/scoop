# Scoop HTTP multipart 规范

版本：M54，实施中。本文细化 [HTTP 消息工具规范](SCOOP-HTTP-MESSAGE-SPEC.md)第 3 节；实际实现与验证见 [M54 进度](../milestone54/PROGRESS.md)。消息数据和内存引擎位于 scoop.http，异步适配位于 scoop.http.asyncio，client 只组合其公开能力。

## 1. Boundary 与 part 元数据

`MultipartBoundary(value)` 为不可变值，按 RFC 2046 §5.1.1 接收 1～70 个 ASCII bchars，最后一个字符不能为空格。值区分大小写，不包含线上前导两个连字符。`generate()` 使用 crypto 的 24 个随机字节生成带固定短前缀的十六进制 boundary；不读取或扫描上传源。显式 boundary 及嵌套 multipart 的选择由调用者保证不与内容或外层 boundary 冲突。

`fromMediaType(mediaType)` 要求 multipart 类型及一个有效 boundary 参数；复用已有媒体类型的参数解析，不另写 quoted-string 规则。`toMediaType(subtype = "form-data")` 生成带 boundary 的 HttpMediaType，必要的引号由同一字段编码器生成。boundary 的 ASCII 与长度检查不依赖平台 locale。

`MultipartPartHead(fields = 空列表, bodyLength = None)` 保存不可变、有序、可重复的 HttpField，以及调用者已知的正文长度。bodyLength 必须非负，None 表示未知；该值用于编码检查和完整长度计算，不自动生成 Content-Length。接收端始终按 boundary 定界，解析的 head.bodyLength 为 None，即使 part 内存在 Content-Length 字段也不改变此规则。

`MultipartPartHead.formData(name, filename = None, contentType = None, bodyLength = None)` 生成 RFC 7578 的 Content-Disposition: form-data，name／filename 使用显式 quoted 参数及 UTF-8 字节，复用 HttpParameter 的转义及控制字节检查。保留空名称／空文件名，不解释路径或读取文件。该工厂不生成 filename*；RFC 7578 §4.2 明确禁止在 multipart/form-data 中使用该扩展。通用 part 字段仍能原样保存其他 multipart 类型的 filename*，其解释由已有 Content-Disposition 工具完成。

## 2. 增量编码与长度

`MultipartEncoder(boundary)` 为串行使用的 Closeable。`beginPart(head)` 返回 boundary 行、字段及空行；`writeBody(buffer, count = buffer.readableBytes)` 返回并消费实际正文前缀，不把它当作字符或进行传输编码；`endPart()` 返回正文与下一个 delimiter 之间的 CRLF；`finish()` 返回最终 closing delimiter。输出只使用 CRLF，不生成 transport padding、preamble 或 epilogue。允许零 part 的空 multipart，这是 HTTP 表单的兼容接收与发送规则。

beginPart 与 endPart 必须成对，前一 part 未结束时不能开始下一 part。已知 bodyLength 在每次写入时检查上界、在 endPart 时检查精确长度；参数错误、状态错误及长度不符不消费输入、不改变阶段，调用者可以补足或改用正确参数。成功 finish 后不可继续编码，close 只释放本地状态，不补写 delimiter，不关闭调用者 buffer。finish、close 分别表达格式完成和本地释放。

`MultipartEncoder.contentLength(boundary, parts)` 根据同一头字段输出和 delimiter 计算完整编码长度。所有 part 正文长度已知时返回 Some(Long)，否则返回 None；所有加法检查 Long 溢出。它不读取正文，不把每 part 的 Content-Length 字段当作已知长度，也不扫描 boundary 碰撞。

## 3. 增量解码与背压

`MultipartDecoder(boundary, limits = MultipartLimits())` 为串行使用的 Closeable。`receive(buffer, count = buffer.readableBytes)` 消费实际前缀并返回字节数；`nextEvent()` 交付 MultipartEvent 的 PartStarted(head)、Data(bytes)、PartEnded、End。完整 head 之后才交付 part，Data 是独立字节快照；未知字段、非 UTF-8 字段值和正文不经 String 中转。嵌套内容保持为普通正文，可显式组合另一个 decoder。

至多有一个待取事件；待取事件存在时 receive 返回零。Data 每次最多 dataChunkBytes，未决 boundary 前缀最多 boundary 长度加四字节，内部正文暂存最多 dataChunkBytes 加一个 boundary 前缀。短输入在可确定的正文前缀形成后即可交付，不等整个 part 或整个消息。正文中的部分 boundary 匹配失败时完整归还那些字节；分块、CRLF 或二进制零字节不改变结果。

delimiter 只能出现在行首，包含其前面的 CRLF，该 CRLF 不属于正文。流开头及正文开头的行首同样可匹配 boundary。完整的 boundary 前缀后只允许两个关闭连字符、SP／HTAB transport padding 和规定的行结束；非法后缀报告格式错误，不将已完整匹配的 delimiter 重新解释为正文。发送端本就不得让完整 boundary 成为正文行前缀；部分匹配继续保真。

接收 preamble、epilogue、CRLF 跨块、无字段 part、空正文及 closing delimiter 后直接 EOF。忽略 preamble／epilogue 的内容但不无界保存在内存中。每 part 字段接受 RFC 822 continuation，展开为单个 SP，去除字段值两端 OWS，保持字段顺序、重复、名称大小写与实际非 UTF-8 字节；拒绝非法名称、裸 CR／LF、其他控制字节和没有前置字段的 continuation。Content-Transfer-Encoding 只作为字段保存，不隐式解码 base64 或 quoted-printable。

`endInput()` 幂等声明外层正文 EOF。没有完整 closing delimiter、未结束的字段或坏 CRLF 均报告 MultipartParseException；不补 delimiter、不补发未完成 part。isComplete 表示已经取得合法 closing delimiter，End 在 outer EOF 后交付；调用者通过正常消费读取并忽略 epilogue。isFinished 表示 EOF 已验证且所有事件已取走，与 isClosed 区分。提前 close 放弃本地未交付数据，不执行 I/O，不自动排空外层正文。

## 4. 限制与错误

`MultipartLimits(maxHeaderBytes = 65536, maxFields = 100, maxParts = 1024, maxBoundaryPaddingBytes = 1024, dataChunkBytes = 8192)` 使用 Long。前四项须非负，dataChunkBytes 须为正。maxHeaderBytes 计一个 part 的实际字段字节、折行及 CRLF；maxFields 计实际字段，maxParts 计已开始的 part，maxBoundaryPaddingBytes 计一个 delimiter 后的 SP／HTAB。body、preamble、epilogue 按消费推进，不建立全局内容预算。

格式错误使用 MultipartParseException，保存从输入起点计算的 byteOffset 与具体 detail；局部超限使用 MultipartLimitException，保存 kind 与 limit。receive 实际失败后释放原生 borrow、保存 failure，不能继续推进；参数错误不使 decoder 失效。close 仍可调用，清除未交付字节与字段引用，保留已有完成／错误快照。边界、长度计算和字段构造的普通参数错误不包装成网络错误。

## 5. 异步 part 源

`scoop.http.asyncio.MultipartSource(source, boundary, limits = MultipartLimits(), bufferSize = 8192)` 借用 AsyncByteSource；takeOwnership 工厂另要求 AsyncCloseable，在成功返回时接管。bufferSize 须为正，当前 EventLoop、活动 Job、配置或登记前取消检查失败不移交端点。构造不读取输入、不启动后台预取。

`nextPart(): MultipartPart?` 可挂起；返回不可变 head 与 `MultipartBody`，后者实现 AsyncByteSource／AsyncCloseable。当前 body 须读取到返回零、完成 discard，或关闭整个消息，才能推进下一 part；不能静默跳过未消费正文。一个 source 的 nextPart、body.readInto 和 body.discard 合计至多一个应用读取，参数、同向并发、错误 loop 和登记前取消错误不损坏流。

MultipartBody 只交付当前 part 的字节；分隔符、后续头字段和其他 part 正文不进入它。零长度读取检查参数和状态，不推进输入；完成后的 body 可再次读取零，head 与完成快照保留。nextPart 在外层 EOF 和 End 后返回 None，随后不重复读取。isFinished、isClosed、failure 公开 source 状态，body 另公开 isComplete／isClosed。内部复用 MultipartDecoder，每次预取最多 bufferSize，待交付 Data 与实际 metadata 仍受 decoder 的局部限制约束。

`MultipartBody.discard(maxBytes): Long` 显式丢弃当前 part 的剩余正文，返回实际丢弃量。maxBytes 须非负；为区分恰好到达上限与仍有正文，最多额外读取一个正文字节，超限抛 MultipartDiscardLimitException，保存 maxBytes，并关闭整个 multipart source。该上限只约束此次丢弃的真实正文，不计字段或分隔符。超限与关闭同时失败使用 ResourceUseException 保留两者；其他 I/O、解析或取消错误照常传播。

关闭已完成的 body 只关闭该 body 的本地视图；关闭未完成 body 等同关闭整个 source，不自动排空网络。source.close 取消当前结构化读取，等操作及原生 borrow 真正结束，再释放 decoder／input／待交付 part 数据，并关闭所接管端点；借用模式保留端点。并发关闭共用一次收尾，取消的 finally 可完成。关闭后解除端点、等待者和当前 part 引用；借用模式同样放弃已预取前缀，不能承诺底层仍停在最后交付正文处。

## 6. 异步 part 输出

`scoop.http.asyncio.MultipartSink(sink, boundary, bufferSize = 8192)` 借用 AsyncByteSink；takeOwnership 工厂在成功返回时接管同时实现 AsyncCloseable 的端点。bufferSize 须为正，构造绑定当前 EventLoop 与活动 Job；配置或登记前取消失败不移交端点，不产生输出。

`beginPart(head): MultipartPartWriter` 可挂起，使用同一 MultipartEncoder 输出 boundary 和完整字段，之后返回当前 part 的 AsyncByteSink／AsyncCloseable。writer.head 保存原不可变元数据，isComplete 表示已成功 finish，isClosed 表示本地视图或整个 sink 已关闭。前一 part 必须 finish 后才能开始下一 part；文件、字节或其他流使用普通 copyTo／writeFrom 组合，不由 multipart 读取路径或创建后台任务。

writer.writeFrom 检查原始 count 与 buffer 范围，然后接受至多 bufferSize 的正文前缀；其已知长度检查与 HTTP/1 writer 相同，针对实际接受的前缀。返回值及调用者 buffer 的消费量均表示已交给编码器的正文量；输出随后失败时也保留这个实际消费量。一个操作最多暂存一个编码块及一个有限输出 buffer，正文不整段缓存；完整头字段仍属于调用者显式提供的元数据。零长度操作检查状态与参数，不调用端点。字段、正文和 delimiter 都检查下游短写及真实 buffer 进度；零进度写抛 WriteZeroException。

writer.finish 通过编码器检查已知长度并输出 part 的结束 CRLF，成功后解除当前 part。sink.finish 要求没有活动 part，输出 closing delimiter 并 flush 下游，然后设置 isFinished；两种 finish 均为单次完成动作，重复调用属于状态错误。flush 只向下游转发，不添加 delimiter；已完成且未关闭的 writer 仍可 flush。isFinished、isClosed、failure 由外层 sink 提供。

所有头部、正文、finish 和 flush 合计至多一个应用输出；错误 loop、登记前取消、参数／编码状态错误与并发误用不损坏流，实际 I/O、flush 或操作内取消后不能继续输出。close 的结构化等待、借用／接管、并发收尾及双失败与第 5 节一致。关闭未完成 writer 关闭整个 multipart sink，关闭已完成 writer 只关闭其视图；close 不补 delimiter、不 flush、不结束外层 HTTP body 或压缩流。调用者按顺序 finish multipart、压缩及 HTTP framing，再关闭相应资源；client 的重放策略另见 client 规范。

## 7. 验证

独立向量覆盖 boundary 字符与引号、RFC 报文、每字节分块、部分匹配保真、transport padding、重复与折行字段、二进制正文、空 part、EOF、局部限制和长度溢出。与独立 MIME 实现互操作后，再通过正式 .slib 消费者组合 HTTP 分块、压缩、moving GC 和构建复用；异步组合另覆盖短读写、背压、取消及所有权。

依据：[RFC 2046 §5.1](https://www.rfc-editor.org/rfc/rfc2046.html#section-5.1)、[RFC 7578](https://www.rfc-editor.org/rfc/rfc7578.html)、[RFC 822 §3](https://www.rfc-editor.org/rfc/rfc822.html#section-3)。
