# Scoop HTTP/1 WebSocket 规范

版本：M54，实施中。本文细化 [HTTP 消息工具规范](SCOOP-HTTP-MESSAGE-SPEC.md)第 6 节。握手与帧数据属于 scoop.http，传输、控制帧调度及压缩流组合属于 scoop.http.asyncio；实际实现和验证见 [M54 进度](../milestone54/PROGRESS.md)。

## 1. 升级握手

`WebSocketHandshake.create(url, protocols = 空列表, deflateOffers = 空列表, fields = 空列表, absoluteForm = false)` 创建不可变的 HTTP/1.1 GET 请求及本次提议。url 必须使用 ws／wss，不能带 fragment；URL 库提供规范 Host、编码 path／query 和 IDNA。默认 origin-form；absoluteForm 显式生成对应 http／https 的代理请求 target。userinfo 不进入 target 或 Host，不自动变成认证字段。请求 key 每次调用 crypto 生成 16 个随机字节，再用标准有 padding 的 Base64 编码。未显式提供 Origin 时不猜测浏览器来源。

protocols 保存按偏好排列、区分大小写、非空且不重复的 ASCII token。deflateOffers 可列出多个 RFC 7692 配置以表达回退顺序，空列表表示不提议压缩。额外 fields 保留顺序和重复，允许 Cookie／Authorization／Origin，但不能覆盖 Host、Connection、Upgrade、Sec-WebSocket-*、Content-Length 或 Transfer-Encoding。创建只返回消息，不执行网络或 TLS。

`WebSocketHandshake.parse(request)` 接收已有 HttpRequestHead，返回 request、key、protocols、extensions 的不可变快照。要求 HTTP/1.1、精确 GET、HTTP/1 target、单个非空 Host、包含 websocket 的 Upgrade 和包含 Upgrade 的 Connection。Key 与 Version 必须各有且只有一条，Version 为 13，Key 必须是能解码为 16 字节的标准 Base64。拒绝 Transfer-Encoding 与非零 Content-Length。普通请求 target、路由、Host 与实际监听端点的匹配仍由 HTTP／URL 和应用负责；握手不重复实现 URL parser。

`accept(response): WebSocketNegotiation` 校验本次请求对应的升级响应：HTTP/1.1、101、Upgrade 为 websocket、Connection 含 Upgrade，单条 Sec-WebSocket-Accept 必须等于本次 key 的 RFC 6455 SHA-1／Base64 结果。比较前只去除字段两端 OWS，不改变 key 或 Accept 的大小写，不接受重复 singleton。响应不含 HTTP body 定界字段。非 101 返回握手错误，HTTP 客户端可在调用 accept 前按正常 HTTP 处理认证、重定向或错误正文。

响应可不选择子协议；选择时只能有一个字段及一个原始提议中的精确 token。扩展响应必须是本期支持且能匹配本次有效提议的单个 permessage-deflate；未知扩展、重复选择、无提议启用或非法参数均失败。返回 `WebSocketNegotiation(protocol, deflate)`，其中 deflate 为可选的 WebSocketDeflateNegotiation，保存首个匹配的 offer、原始 response 及最终双向窗口／上下文配置，不执行传输移交。只有成功 accept 后，上层才可接管 101 的传输和完整后缀。

`response(protocol = None, deflate = None, fields = 空列表)` 为服务端生成合法 101；所选协议／压缩必须满足同一请求的校验规则，输出同一 Accept 算法及明确的 Upgrade／Connection 字段。额外字段遵守同样的保留字段规则。该工具不替服务端选择路由、权限、Origin 或认证策略。

字段语法错误使用 HttpFieldParseException，保留字段与字节位置；握手条件和协商不匹配使用 WebSocketHandshakeException，保存 detail 与可用的 field。普通构造参数错误使用 IllegalArgumentException。握手值不持有网络资源，失败不关闭调用者传输。

## 2. 扩展字段与 permessage-deflate

`WebSocketExtension(name, parameters)` 和 `WebSocketExtensionParameter(name, value = None, quoted = false)` 保存独立且有序的 token 数据，不归一化区分大小写的扩展名／参数名。值可以省略；显式空值不合法。parse／parseOrNone 接收字节或 String，parseFields 合并所有 Sec-WebSocket-Extensions 行；列表允许 HTTP 的空逗号项，但存在的字段必须至少包含一个扩展。quoted-string 复用 HTTP 字段 parser，展开后必须仍是 token。重复扩展与参数保留，交给具体扩展的协商规则判断；未知项只是数据，不提供任意扩展执行或插件接口。toByteArray／toHttpField 生成单个扩展字段。

`WebSocketDeflateOffer(serverNoContextTakeover = false, clientNoContextTakeover = false, serverMaxWindowBits = None, clientMaxWindowBits = Absent)` 表示请求提议。clientMaxWindowBits 使用 `WebSocketWindowBits.Absent／Any／Limit(bits)`，区分不提议、无值参数和带值参数。serverMaxWindowBits 可省略但不能无值；所有显式 bits 为 8～15，不允许前导零。`toExtension()` 生成 RFC 字段；`parseOrNone(extension)` 只把不属于该扩展或无效提议转为 None。

`WebSocketDeflate(serverNoContextTakeover = false, clientNoContextTakeover = false, serverMaxWindowBits = None, clientMaxWindowBits = None)` 表示响应配置。响应的窗口参数必须有值；未出现时实际窗口为 15。`toExtension()`／`parse(extension)` 处理字段，`offer.accepts(parameters)` 按 RFC 7692 判断能否接受。未知、重复、带值的 no_context_takeover 和非法窗口使该提议无效；服务端可跳过这项并选择另一个有效提议，不能因不支持扩展而否认整个合法 WebSocket 请求。

server_no_context_takeover 的请求要求必须被响应满足；服务器也可自行选择它。client_no_context_takeover 的请求只提示本地发送行为，服务器可以不回显，也可以在未提议时要求它；最终客户端配置取提议提示与响应要求的逻辑或。server_max_window_bits 的请求上限必须在响应显式满足；服务器可在未提议时声明自身更小的窗口。client_max_window_bits 未提议时，响应不得加入；带值提议的响应窗口不能超过该值，响应省略时仍将该提示用于本地发送窗口。匹配与本地压缩器的最终配置保留两者的区别，不把所有参数处理成简单的集合包含或对称回显。

HTTP 内存工具表达协议允许的 8～15 位窗口，不依赖 compression。异步组合在升级移交前根据自己的角色检查编码／解码能力，并根据本地提议提示与实际响应配置本地发送器；当前 compression 编码支持 9～15 位、解码支持 8～15 位。不能接受后再将 8 位编码悄悄改成 9 位，也不能在 HTTP 层手工处理 DEFLATE 尾部。

## 3. 帧与控制数据

`WebSocketOpcode` 为 Continuation、Text、Binary、Close、Ping、Pong；不将保留 opcode 当作普通数据。`WebSocketFrameHead(opcode, payloadLength, isFinal = true, isCompressed = false)` 保存不可变的帧元数据，payloadLength 为非负 Long。isCompressed 表示线上 RSV1，只能用于已启用压缩的 Text／Binary 首帧；continuation、控制帧及所有 RSV2／RSV3 均不能使用它。

`WebSocketClose(code = None, reason = "")` 表示关闭负载。无 code 时 reason 必须为空；有 code 时使用 RFC／IANA 已定义的 1000、1001、1002、1003、1007～1014 或应用范围 3000～4999，不允许保留的 1004／1005／1006／1015 或其他未定义协议值。reason 是至多 123 个 UTF-8 字节的 String，可为空。toByteArray 按网络字节序输出；parse 接收字节列表，拒绝单字节负载、超过 125 字节、非法 code 或非法 UTF-8，BOM 保留。

`WebSocketFrameLimits(maxFrameBytes = 16777216, maxMessageBytes = 67108864, dataChunkBytes = 8192)` 分别限制实际线上 frame payload、跨分片消息的线上 payload 总和和单次 Data 事件。前两项非负，dataChunkBytes 为正；不是全局资源预算。压缩消息的解压长度另由第 6 节的消息 validator 限制。

`WebSocketFrameException(closeCode, detail, byteOffset = None, cause = None)` 区分协议错误 1002、UTF-8 错误 1007、局部大小限制 1009。解码器的 byteOffset 为检测时已经消费的线上字节数，可位于 EOF 边界，不承诺定位头部中的具体错误字节；字符集原因保留其原始消息内偏移。配置／计数错误使用普通参数异常，关闭及同向并发误用使用状态异常。

## 4. 增量编码

`WebSocketFrameEncoder(role, compressionEnabled = false)` 为串行 Closeable。beginFrame(head) 返回完整且采用最短长度形式的 header；客户端每帧使用 crypto 新生成四字节 mask，服务端不生成 mask。writePayload(buffer, count = buffer.readableBytes) 返回并消费实际传入的 payload 字节，按跨调用的帧偏移应用 mask，不整帧缓存。调用者可以分多次提供任意大帧。finishFrame 检查精确长度并完成该帧，不产生额外字节。

帧之间维护 fragmentation：首帧只能在没有活动消息时出现，continuation 必须属于已有消息，控制帧可以在分片之间穿插。控制帧必须 FIN 且长度至多 125；Close 负载完整检查后才完成关闭状态，之后不能再发送数据帧或另一个 Close。尚未写完一个 frame 时不能插入其他帧；取消后的传输中止由异步 session 负责。

开始前的参数、头部及顺序错误不改变状态，count 越界或超过剩余长度不消费输入；调用者可纠正。实际文本／关闭负载错误使 encoder 失败，保留已经读取的 buffer 消费量，不能继续产生帧。isFrameOpen、isMessageOpen、hasCloseFrame、isClosed、failure 暴露实际状态。close 放弃未完成帧和字符状态，不补数据、FIN 或 Close。

## 5. 增量解码

`WebSocketFrameDecoder(role, compressionEnabled = false, limits = WebSocketFrameLimits())` 的 role 为本地角色：服务器只收 masked frame，客户端只收 unmasked frame。receive(buffer, count) 消费实际前缀；nextEvent 交付 WebSocketFrameEvent 的 FrameStarted(head)、Data(bytes)、FrameEnded、Ping(bytes)、Pong(bytes)、Close(value)、End。控制帧先完整检查至多 125 字节的负载，再交付单个专用事件，不混入 Data。

内部至多一个待交付事件、一个至多 14 字节的 header、dataChunkBytes 的正文和 125 字节的控制负载。存在事件时停止读取；Data 是独立字节快照。零长度 frame 同样交付开始和结束；短块可以在 receive 结束时交付，不必等整个配置块填满。FrameEnded 只表示一个 frame 完整，isFinal 的数据帧同时结束消息。header 的 16／64 位长度必须为最短形式，64 位最高位必须为零；未读 payload 前即可检查明确的长度限制。

文本验证跨 continuation 保持状态，控制帧不重置它。Close 可以中止正在分片的消息，保留关闭原因，不伪造 FrameEnded 或完成缺失的数据帧。收到 Close 后拒绝更多数据或重复 Close；Ping／Pong 仍作为有界控制数据处理。压缩首帧只在显式开启时接受；帧层不解压，不把压缩字节当作 UTF-8。

endInput 幂等声明 transport EOF；完整的 frame 边界可交付 End，截断 header／payload 或未结束的分片消息报错。没有收到 Close 的正常字节 EOF 不等于 WebSocket 正常关闭，异步 session 必须将其报告为异常断开。isFinished 只在 End 已交付时为真；hasCloseFrame、closeFrame、failure 和 isClosed 分别保留协议与本地状态。

receive、nextEvent 与 endInput 中的实际协议／字符错误保存 failure，释放本地字符状态，后续只能关闭；发生错误时原生 borrow 仍按实际已消费前缀解除。close 幂等丢弃未交付数据，不读取下游或伪造 End。纯内存操作的并发误用不损坏已有状态。

## 6. 消息验证与压缩组合

`WebSocketMessageValidator(type, maxBytes)` 接收 WebSocketMessageType.Text／Binary，按 receive(data: List<Byte>) 的实际消息字节累计大小。Text 使用 charset 的严格增量 UTF-8 转换并保留 BOM，跨任意输入块保留未完成字符；Binary 不做字符检查。finish 单次检查消息完整，close 只释放。byteCount 累计成功接收的完整数据块，失败时保留此前值；isFinished、isClosed、failure 保存完成、关闭与失败状态。超限或非法字符后不能复用，帧编码／解码器仍各自记录原生输入的真实消费量。

未压缩帧由引擎内部使用该 validator；压缩消息由 HTTP 异步适配或显式调用者在 compression 的 DeflateMessageDecoder 之后使用它。它只承担 WebSocket 的单条消息大小与 UTF-8 合同，不实现 Unicode 算法、压缩算法、任意预算或网络策略。相同的大小与字符错误码同时适用于分片普通消息及解压后的消息。

## 7. 异步边界

异步 session 的控制帧调度、关闭握手及 permessage-deflate 传输合同继续遵循 [消息工具规范第 6 节](SCOOP-HTTP-MESSAGE-SPEC.md#6-http11-websocket)，公共 API 与收尾细节见 [WebSocket 异步会话规范](SCOOP-HTTP-WEBSOCKET-ASYNC-SPEC.md)。握手或内存帧工具完成不代表传输功能已完成。HTTP/2 WebSocket、Extended CONNECT 和 HTTP/3 不属于本期。

## 8. 验证

握手覆盖 RFC 6455 固定 Accept、独立随机 key、URL／代理 target、重复 singleton、错误状态／版本、大小写规则、协议匹配、扩展 quoted-string、多行提议及 RFC 7692 的非对称参数；与独立 WebSocket 实现验证两个握手方向。正式 .slib 消费者组合真实 HTTP/1 升级与同次读取的后缀、moving GC 和正常缓存复用。

帧验证覆盖 RFC 固定 mask、最短长度、角色、保留位、空帧、大帧有限分块、控制穿插、跨帧 UTF-8、Close code、截断 EOF、实际消费量与失败后清理；与独立 WebSocket 帧实现互操作，并组合正式 compression 消息引擎校验解压大小及文本。

依据：[RFC 6455](https://www.rfc-editor.org/rfc/rfc6455.html) §4、§5、§7、§9.1；[RFC 7692](https://www.rfc-editor.org/rfc/rfc7692.html) §7.1、§7.2。
