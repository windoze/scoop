# Scoop HTTP 库规范

应用层路由、请求 Context、middleware 与结果执行属于独立的 [HTTP server 库](SCOOP-HTTP-SERVER-SPEC.md)，不进入本协议引擎。

版本：M53 已实施基线，含 M54 修订；扩展方法已实现，其余补充按 [M54 进度](../milestone54/PROGRESS.md)记录。既有实现见 [M53 设计](../milestone53/DESIGN.md)，新增范围见 [M54 设计](../milestone54/DESIGN.md)与第 9 节。本规范使用[库 API 风格](SCOOP-API-STYLE.md)、[异步 I/O](SCOOP-ASYNCIO-SPEC.md)和 [TLS](SCOOP-TLS-SPEC.md)的既有合同，不改变语言语义、runtime ABI 或编译阶段职责。

## 1. 分层与后端

`scoop:scoop.http:0.1.0` 提供字节消息、HTTP/1 增量编解码、HTTP/2 内存会话和协商数据处理，依赖 core、std、platform、url、idna、charset；后三者服务 M54 的字段、主机名与目标处理。`scoop:scoop.http.asyncio:0.1.0` 在普通 AsyncByteStream／AsyncCloseable 上协调传输、body 与连接生命周期，依赖 HTTP 和 asyncio。前者不依赖异步平台后端、TLS 或 socket；后者不负责连接 TCP、TLS 握手或证书校验。

HTTP/1 使用 llhttp 9.4.3；HTTP/2 使用 nghttp2 1.70.0 的完整上游协议核心，通过 `nghttp2_session_mem_recv2`／`nghttp2_session_mem_send2` 接入。帧、HPACK、HTTP messaging、流状态与 SETTINGS 校验交给 nghttp2；Scoop 不再重放一套接收校验。

协议引擎不增加连接池、自动重试、路由、Cookie jar、响应缓存或下载目的地选择；这些客户端策略由独立 client 库承担。M54 的字段、Cookie 数据、multipart、SSE 与 WebSocket 协议补充见第 9 节，内容压缩由独立 compression 库提供。HTTP/2 的库核心保持完整，普通消息接口不提供 server push 或通用扩展插件。客户端明确声明禁用 push；这不等于裁掉后端机器码。HTTP/3 不在本规范范围。

## 2. 消息、字段与错误

HttpField 的构造参数为 `List<Byte>`，名称和值各取得独立 `Array<Byte>` 快照，另有 neverIndex。字段集合保留顺序和重复项；ASCII 大小写无关查询返回每一项，不合并 Set-Cookie 等同名值。字段值允许后端实际接受的非 ASCII／非 UTF-8 字节。文本转换由调用者通过已有编码 API 显式执行。

方法保留 token 字节。HTTP/1 request-target 保留原有字节；HTTP/2 的 scheme、authority、path 与 CONNECT authority 单独表达。响应有版本、状态码和字段；HTTP/1 保留实际 reason phrase，HTTP/2 没有该数据，不生成默认文本。正常接收的请求／响应头必须具备该消息所需信息，不能把未完成的字段块当作消息交付。

公开 HttpRequestHead 使用 HttpRequestTarget 的 Http1(raw)、Http2(scheme, authority, path)、Connect(authority) 形状；各字节值为 Array 快照。HttpResponseHead 的 reason 是可选字节快照。HttpHeaderLimits 按字段名称和值的合计字节数、单项字节数、字段数量限制接收；Http1Limits 另限制起始行各组成数据的合计字节数，不把分隔符或 OWS 计为字段数据。

body 交付解开 chunked／DATA 定界后的内容字节；不按 Content-Type、Content-Encoding、字符集或百分号编码解释。trailer 独立于初始字段，能够区分未完成、已完成且为空、收到非空字段。信息响应、最终响应、body 结束和协议切换分别可观察。

这些快照表示后端交付的数据，不保证是原始报文镜像。llhttp 可能跳过起始 OWS 或展开接受的折行；nghttp2 会过滤部分字段，HPACK 表示不保留。需要原始报文的应用在原有传输上记录字节，HTTP 不增加另一套记录机制。

HttpException 保存操作、协议、作用域、stream ID、后端码及已复制的说明。HTTP/1 的消息／定界错误属于连接；HTTP/2 区分操作参数、单流错误与连接错误，RST_STREAM、GOAWAY 和后端忽略的普通字段以实际事件交付。底层 I/O 异常保留原类型。native 错误先复制再返回 Scoop 抛出，不穿越 C ABI 展开。

HttpException 的 isUnexpectedEof 明确表示该错误是否来自后端确认的输入截断；构造参数缺省为 false。HTTP/1 的无效 EOF 状态与 HTTP/2 的 EOF 错误由各自适配层按后端公开常量标记，保留原始错误码、说明与作用域。已正常完成的 EOF 不产生此错误，其他语法或协议错误不因随后调用 endInput 而变成截断。使用者据此区分断开的输入和其他协议失败，无须解析错误文本或在高层库绑定后端常量；是否重试仍由使用者的协议与调用策略决定。

## 3. HTTP/1 接收

Http1Decoder 的 client／server 工厂固定接收角色、选项和局部字段限制；对象实现 Closeable，调用者串行使用。receive 消费 NativeByteBuffer 中的实际前缀，nextEvent 取走已完成事件，endInput 明确交付 EOF。关闭后不能继续推进；关闭本身不执行 I/O。

receiveWithProgress 使用相同输入合同，返回 Http1ReceiveResult(count, startedMessages)。startedMessages 是本次消费中新开始的消息数，直接来自 llhttp 的 message-begin 回调，信息响应与最终响应分别计数；部分起始行也会开始一条消息，不必等到完整头部。零输入返回两个零，普通 receive 与该入口可以交替使用；已经由普通 receive 消费的开始不在下一次重报。该计数不改变消息事件，也不另行解析起始行或推测响应边界。

client 在发送每个请求时按顺序登记方法上下文。信息响应不移除此上下文，最终响应结束后才移除。HEAD 响应没有 body；成功 CONNECT 与 101 交付切换事件，保留同次输入的全部未消费后缀。没有对应请求上下文的响应是调用状态错误，不能靠接收宽容选项补足。

CONNECT 只有 2xx 响应按隧道成功处理；100／103 等信息响应仍等待同一请求的最终响应，407 等拒绝响应按普通 Content-Length、chunked 或 EOF 定界读取正文。llhttp 的方法状态主要服务请求端 CONNECT 暂停，不能将客户端的 CONNECT 上下文直接用于其无条件切换判断；适配层在取得响应状态后通过正常 headers-complete 回调声明成功切换，保留原始请求方法与响应字段。

服务端收到 Upgrade／CONNECT 请求时，切换事件表示 parser 停在该请求边界，是否接受由服务端响应决定。拒绝请求的升级后调用 resumeHttp1 恢复 HTTP/1；接受后移交后缀，不再恢复旧 parser。

解析支持 HTTP/1.0／1.1、连续消息、Content-Length、chunked、trailer 和 EOF 定界。起始行、字段、chunk 可以任意分段；字段回调片段必须组合到字段完成后才交付。只有需要 EOF 定界的消息在正常 EOF 时完成；截断定长／chunked 报文报错。

llhttp 9.4.3 的服务端请求解析只接受其已知方法集合，`CUSTOM` 等集合之外的方法明确诊断为该版本不支持的方法；公共 token 数据模型不改变这个接收限制。客户端方法登记接受任意合法 token，保留精确 HEAD／CONNECT 与普通方法的响应定界分类，非法 token 为参数错误且不登记上下文。HTTP/2 的方法不受 llhttp 限制。不能改写线上方法或预修复报文绕过后端。

接收选项在创建时固定，默认全部关闭：

```scoop
public enum Http1AfterClose { Reject, Discard, ParseNextMessage }
public struct Http1ReceiveOptions(
    val legacyHeaderValues: Boolean = false,
    val lenientChunkedLength: Boolean = false,
    val lenientTransferEncoding: Boolean = false,
    val lenientTextVersion: Boolean = false,
    val optionalLfAfterCr: Boolean = false,
    val optionalCrBeforeLf: Boolean = false,
    val optionalCrLfAfterChunk: Boolean = false,
    val spacesAfterChunkSize: Boolean = false,
    val afterClose: Http1AfterClose = Http1AfterClose.Reject
)
```

各布尔值映射到 llhttp 同义开关。legacyHeaderValues 实际使用较宽的 lenient_headers，包括旧式折行；不保证接受冒号前空白或忽略非法行。lenientChunkedLength 同时影响 TE／CL 共存以及没有最终 chunked 的请求，不能解释成单一冲突开关。两个行结束选项各自只放宽后端覆盖的位置，不承诺接受任意裸 CR／LF。

lenientTransferEncoding 接受 `Transfer-Encoding: chunked, gzip` 等非最终 chunked 顺序时，9.4.3 按 EOF body 交付后续字节；看起来像 chunk 的文本也会原样属于 body，不再次按 chunked 解码。

lenientTextVersion 只影响文本起始行的解析；后端接受的数字不扩展 M53 的执行协议。非 1.0／1.1 的文本版本仍报告不支持，文本 `HTTP/2.0` 不能启动二进制 H2。afterClose 的丢弃与继续解析互斥；继续解析不保证传输可复用。

本期不公开或开启 header_value_relaxed：9.4.3 对含 NUL 值的该路径有已复现的不前进循环。不能用 lenient_headers 冒充它。各实际边界见[后端调研](../http/INTERFACE-RESEARCH.md)。

## 4. HTTP/1 发送

Http1Encoder 生成起始行、字段、body 定界与结束字节，保持调用者字段的相对顺序。发送方法和字段名必须为合法 token；request-target、reason phrase、字段值不能通过 CR、LF、NUL 等非法结构字节注入报文。接收宽容选项不作用于发送。

编码器固定 HttpRole。beginRequest／beginResponse 返回头部字节，后者显式接收对应请求方法；writeBody 消费 NativeByteBuffer 的指定前缀并返回编码字节，finish 返回结束／trailer 字节。每次 begin 都需要 finish，包括空 body。Http1BodyFraming 为 Empty、Length(count)、Chunked、Eof；isMessageOpen、mustClose、isSwitched 分别表示当前消息、连接复用与协议切换状态。编码器实现 Closeable，不执行传输。

body 定界显式选择无 body、已知长度、chunked 或响应 EOF。Content-Length 与 Transfer-Encoding 必须与所选定界一致；缺少所需字段时由编码器添加，不允许冲突的重复定界。chunked 仅用于 HTTP/1.1；EOF 定界不用于请求。发送的字节数不得超过已知长度，完成时检查剩余长度；trailer 只随 chunked 结束交付。

HTTP/1 与 HTTP/2 的发送 trailer 使用同一组已知字段限制，见[消息工具 §1.11](SCOOP-HTTP-MESSAGE-SPEC.md#111-trailer-声明与发送限制)。普通名称／值的校验和协议定界仍由各自编码器执行，不将 trailer 合并到初始字段；字段错误不能提前提交消息结束。

响应编码持有对应方法上下文，处理 HEAD、信息响应、204、304 和成功 CONNECT 的实际 body 规则。HEAD 与 CONNECT 按精确大小写识别，`head`／`connect` 等 token 使用普通方法规则。101／成功 CONNECT 之后旧编码器不再发送 HTTP 消息。已被本层接受的 body 前缀不因后续传输失败或取消而回滚或自动重发。

## 5. HTTP/2 内存会话

Http2Session 的 client／server 工厂固定角色、接收选项、本地限制和初始 SETTINGS。创建后可提交请求、信息／最终响应、body、trailer、RST_STREAM 与 GOAWAY；请求提交返回实际 stream ID。DATA provider 只读取已准备的 native 字节，缺少数据返回 deferred，补充字节后调用 resume_data。不在 C 回调内执行异步 I/O、用户回调或 continuation 恢复。

submitRequest／submitResponse 的 endStream 表示头部后立即结束发送；需要 body 或 trailer 时设为 false，随后 writeBody／finish。信息响应不结束流，必须显式使用 endStream=false。writeBody 只接受每流待发送容量内的前缀；返回零表示等待实际输出或窗口进展。finish 表示没有更多 DATA，可附独立 trailer。接收端通过 Request／Response／Data／Trailers／End 事件区分阶段，End 前没有 trailer 时也交付空 Trailers。StreamClosed 保存实际 HTTP/2 错误码，GoAway 另保存方向、最后 stream ID 与调试字节；不能把正常单向 End 等同于整个流关闭。

`Http2Event.RequestHeadStarted(streamId)` 仅在后端确认开始接收新的初始请求首部块时交付，在完整的 HPACK／CONTINUATION 首部块之前可观察；同一块的 CONTINUATION 不重复开始，响应及 trailer 不产生此事件。该事件只表示后端已识别开始，不表示请求合法或已接受。成功以 Request 结束，拒绝／失败以实际 StreamClosed 或连接终止结束；部分 9 字节帧头尚未被后端识别时不提前报告。实现复用 nghttp2 的请求首部块开始回调，不另外解析输入；尚未读完的 padding／priority 前缀可能延后此观测。后端忽略的已关闭流 HEADERS 不开始新的请求首部计时。

请求 HEADERS 已排队但尚未发送时，对端 GOAWAY 可以使后端拒绝打开该流。nghttp2 随后的实际 StreamClosed(REFUSED_STREAM) 是该次拒绝的终止结果；包装层不先将同一次 START_STREAM_NOT_ALLOWED 未发送回调重复报告为 Failure。该流的待发正文与 trailer 随关闭释放，已接受的其他流仍可完成；其他发送错误保留其实际 Failure，不一律解释成请求未处理。

收到 GOAWAY 且现有流均结束后，nghttp2 可以丢弃尚未发出的 SETTINGS ACK，并以 SESSION_CLOSING 报告该帧未发送。此正常收尾不产生新的 Failure，已经接收的 GOAWAY、响应、正文和流结束事件仍然有效；其他帧及其他错误码继续按实际发送失败处理。

finish 在没有待序列化 DATA 时不等待流或连接的发送窗口。实现可用带 END_STREAM 的空或非空 trailing HEADERS 表示结束，在本流已提交的初始头／信息响应／最终响应序列化之后排入输出；先前已序列化的 DATA 片段仍按原顺序完整取出。有待发送 DATA 时，结束和 trailer 必须等这些字节发送完毕，不能通过提前结束绕过流量控制。无论使用空 DATA 还是 trailing HEADERS，接收端只交付一次 Trailers 和 End。

Http2Settings 声明 headerTableSize、maxConcurrentStreams、initialWindowSize、maxFrameSize、maxHeaderListSize；客户端另固定 ENABLE_PUSH=0。Http2Limits 使用 HttpHeaderLimits，并分别给出 maxContinuations、maxSendHeaderBlockBytes、maxPendingBodyBytes。伪头的 never-index 标记通过 Http2RequestIndexing(method, scheme, authority, path) 和响应的 neverIndexStatus 保存，普通字段仍使用 HttpField.neverIndex。

receive／takeOutput 使用 NativeByteBuffer 的实际前缀。mem_send2 返回的临时字节必须在下一次可能使指针失效的调用前复制到自有存储；不能跨挂起保存该指针。输出每次可短传输，未取走的输出保持有效。

receiveWithProgress 使用相同的 buffer／count 合同，返回 Http2ReceiveResult(count, progress)。progress 是独立的 Http2InputProgress(streamId, count) 数组，按接收顺序报告本次新确认归属的 HEADERS／CONTINUATION／DATA 字节；包括帧定界、填充和压缩首部字节，不包含连接前言、其他控制帧或被后端忽略而未报告归属的帧。同流相邻进度可合并，同一数组也可多次出现同流。该记录表示协议输入进展，不是已解码正文大小。

帧头完整且 nghttp2 已确认归属时，一次报告其 9 字节；如果帧头跨调用接收，早先尚未归属的帧头字节也在此时报告。帧头尚不完整时不能据此延长任一流的读等待。后续 payload 依据 mem_recv2 的真实消费量报告，因此分段 HPACK／CONTINUATION 不必等完整首部事件才可观察进展。归属和长度来自 nghttp2 回调，适配层按已知边界分段提交输入，不另写帧解析或校验；被后端忽略的帧不影响下一条有效流的归属。receive 与 receiveWithProgress 可交替使用，零容量不推进并返回空进度；普通 receive 不创建进度数组。消息事件、窗口和错误仍遵守同一会话合同，关闭或 reset 不使已经返回的进度快照失效。

takeOutputChunk 使用相同的 buffer／count 合同，返回 Http2OutputChunk(count, streamId, kind)。每个非空片段只属于一个流和一种 Http2OutputKind：Headers 包含首部及其 CONTINUATION，Data 包含 DATA 的定界和内容，Control 包含其他帧及连接前言；streamId 是实际帧的流号，连接级内容为零。零容量或暂无输出返回 count=0、streamId=0、Control，不推进零容量操作。归属由 nghttp2 的序列化回调保存，跨短取出保持稳定，不另外解析输出字节；takeOutput 仍只返回 count，两种入口可交替消费同一输出位置。此 count 是取出量，实际网络写入进度须由调用方按该片段的真实传输返回量累计。

hasPendingOutput(streamId) 查询该流尚未序列化的头、已接受的 DATA 和结束标记，以及会话仍持有的该流 Headers／Data 片段；其他流和 Control 片段不使本流保持待发送。流已关闭或 reset 时，之前已序列化而未取完的片段仍须完整取走。bufferedOutputBytes 单独返回会话已经序列化、尚未取走的所有片段字节数，供 GOAWAY 等控制输出在其他流继续发送时独立完成 flush。流窗口为零时即使 wantsWrite 为 false，该流仍可有待发送 body。查询不推进协议、不执行 I/O；已经交给调用者的 buffer 仍由调用者负责写完和 flush。

请求／响应头、trailer、DATA、流结束、RST_STREAM、GOAWAY、SETTINGS 与忽略字段各自可观察。非法头或消息错误按后端真实作用域处理；普通消息接口不暴露 no_http_messaging。发送头根据消息数据生成合法伪头顺序和小写字段名，使用后端公开的名称／值检查；调用者不能把伪头混入普通字段。

DATA 收到后保持为未消费量。调用者实际读取或明确丢弃 body 时才调用 consume，归还相应的流与连接窗口。流已经结束／reset 而仍有未消费数据时，按上游 consume_connection 合同归还连接窗口，不能因单流结束阻塞其他流。不能重复消费或超过实际未消费量。

reset 显式放弃该流尚未消费的 DATA，并归还连接窗口；随后不再对这些字节调用 consume。对端 reset 仍通过实际 StreamClosed 事件交付，由使用者消费或放弃已交付的 DATA。endInput 标记传输 EOF；仍需要协议输入的会话以 nghttp2 的 EOF 错误结束，不通过另写帧解析器猜测被截断的位置。close 释放本地状态，不隐含发送 GOAWAY。

```scoop
public enum Http2InvalidRegularField { Ignore, RejectStream }
public struct Http2ReceiveOptions(
    val allowFieldValueEdgeWhitespace: Boolean = false,
    val invalidRegularField: Http2InvalidRegularField = Http2InvalidRegularField.Ignore
)
```

allowFieldValueEdgeWhitespace 只放宽 nghttp2 允许的普通字段首尾 SP／HTAB；不作用于伪头等特殊字段。忽略策略仅覆盖 invalid-header 回调实际交付的错误。被忽略字段以独立事件保存名称、值和 stream ID，不进入正常字段集合；RejectStream 在该同步回调中按预置策略终止流。

非法伪头、大写字段名及若干消息一致性错误不属于可忽略范围。nghttp2 会接受并移除 204 的 Content-Length: 0、成功 CONNECT 的 Content-Length 等字段，忽略事件不能补回全部内建过滤。以固定版本的实际行为为准。

接收字段字节数、数量、CONTINUATION 数量、发送字段块上限和待发送 body 容量分别作为具体本地限制。SETTINGS 是向对端声明的协议值，不代替本地接收检查；发送 HPACK 压缩上界也不作为接收大小限制。不把这些限制扩展为通用 CPU／内存预算或计费。

自动 PING ACK、禁用自动窗口更新和 NO_COPY 生命周期属于适配实现；无效／废弃的 no_closed_streams 等选项不进入公共配置。上游已有的会话限制保持其默认行为，除非本规范明确选择实际需要的选项。

## 6. 协商与切换

TLS 协议选择入口只接收实际 ALPN 字节。`h2` 与 `http/1.1` 必须属于调用者允许的协议；未协商时只有显式允许回退才能选 HTTP/1.1。未知 ALPN 或不允许的结果报协商错误，不通过试解析猜测。

selectHttpAlpn 接收可选 ALPN 字节、允许的 HttpProtocol 列表和显式回退选项。尚未选择协议时使用 HttpNegotiationException 保存操作与说明，不给错误虚构已选 HTTP 版本。HttpCleartextProbe 接收允许的 HttpProtocol 列表，逐前缀识别 prior knowledge；protocol 为已选结果，prefix 为已消费前缀快照，buffer 的其余部分仍归调用者。选择完成后不继续消费输入。

明文客户端明确选择 HTTP/1、H2 prior knowledge 或 h2c；服务端声明允许的入口。H2 前言识别支持任意分段，判别期间保留所有输入，最终仍交给唯一的协议消费者；不能丢失部分匹配后转入 HTTP/1 的字节。没有允许的协议、截断的 H2 前言或文本 HTTP/2 都不能静默回退。

h2c 显式生成／验证 Upgrade、Connection、HTTP2-Settings；HTTP2-Settings 采用无 padding 的 base64url SETTINGS payload。升级请求的 HTTP/1 body 必须完成，随后调用 nghttp2_session_upgrade2，把已发送／收到的请求关联到 stream 1，不重复交付或重发 body。HEAD 标记传给上游。拒绝升级时继续交付真实 HTTP/1 响应，不自动重放请求。

prepareHttp2Upgrade 为普通 HTTP/1.1 请求添加升级字段，显式拒绝 CONNECT、已有 Upgrade／HTTP2-Settings 或 Connection: close 冲突。readHttp2UpgradeSettings 验证对端升级字段并解码 payload，实际 SETTINGS 值由 upgradedServer 的 nghttp2 调用检查。http2UpgradeAccepted 区分有效 101 与普通拒绝响应，http2UpgradeResponse 构造接受响应。upgradedClient 的 settings 必须与准备请求时相同，客户端固定声明 ENABLE_PUSH=0；upgradedServer 另外接收本端 SETTINGS。两个工厂均只在 HTTP/1 请求 body 完成之后调用，stream 1 的请求头与 body 不再次作为 H2 事件交付。

升级 payload 的格式错误可直接从工厂抛出 HttpException；格式完整而数值非法的对端 SETTINGS 仍可能按 nghttp2 的接收合同产生本地 GOAWAY，使用者继续通过会话事件处理连接错误，不把它改写为另一套 SETTINGS 检查。

101、成功 CONNECT 及其他协议切换可移交底层 AsyncByteStream／AsyncCloseable 和未消费后缀。移交后旧连接停止协议处理并释放自己的状态；新拥有者先消费后缀，再读取传输。关闭旧连接不能再次关闭已移交资源。

## 7. 异步连接、body 与关闭

异步入口使用 takeOwnership 命名，固定 EventLoop 归属，并接收实际结构化生命周期所需的 CoroutineScope。配置、参数、活动 Job 和登记前取消检查失败时，传输仍归调用者。成功登记后接管，后续失败关闭未交付的连接与 native 状态。交付后不得绕过 HTTP 包装层并发使用传输。

H1 按顺序关联请求与响应，同一方向只允许一个消息 body 操作；未定界完成的 body 不能作为下一条消息。关闭或取消未消费完的 body 关闭连接，不执行无界自动 drain。信息响应不结束对应请求。H2 用 stream ID 分开关联请求、响应和双向 body，单流可以独立结束或取消。

AsyncHttp1Connection 的 takeOwnershipAsClient／takeOwnershipAsServer 接收传输与 CoroutineScope。sendRequest／sendResponse 返回 AsyncHttp1Writer，每条消息用 finish 完成；receiveRequest 在正常 EOF 返回 None，receiveResponse 逐条交付信息／最终响应及 AsyncHttp1Body。body 在消费完数据及结束标记后公开已完成状态和 trailer，之后读取返回零；显式 close 未完成的 body 会关闭连接。服务端按接收顺序使用对应请求方法发送响应，拒绝升级后恢复 HTTP/1；接受切换后通过 takeTransport 移交前缀及传输。H1 的实际读写属于调用操作的 Job，连接另登记一个负责存活与本地关闭的 scope 子任务。

HTTP/1 客户端工厂可选 captureTimings（默认 false）。开启后，Http1IncomingResponse.firstByteAt 保存包含该响应开始字节的底层 read 完成时的单调 TimeMark，缺省关闭时为 None；信息响应各有自己的值。开始边界来自 receiveWithProgress，时间随待交付的响应开始记录按序保存，跨多次读取的部分起始行和同批读取的流水线响应均正确配对。同次读取的新响应可以具有相同时间。此值表示 HTTP 可见输入的观测时间，不声称是网卡接收时间，也不把完整头部到达时间代替首字节。

AsyncHttp1Connection.pendingResponseFirstByteAt 返回下一条尚未交付完整响应头的开始时间，尚无开始或未开启采样时为 None。该只读状态在输入失败后仍可读取，使头部尚未完整就超时／EOF 的调用也能保留已经确认的首字节；完整响应交付后，时间归入 Http1IncomingResponse，连接上的查询继续指向下一条待交付响应。

客户端最终响应已经定界完成且协议要求关闭连接时，若请求 writer 仍未结束，先交付响应并保留该 writer 的收尾机会；不在响应返回之前使它失效。连接立即失去复用资格，当前 writer 正常 finish 后执行本地关闭，显式 close／取消仍放弃未完成的上传并关闭连接。没有活动 writer 时按原有规则关闭。此规则同样覆盖空响应、EOF 定界和请求声明的 Connection: close，不掩盖实际传输写入失败或非法消息长度。

`AsyncHttp1Connection.isReusable` 是已知协议状态的只读快照：连接开放、没有在途读写或未结束的消息／请求关联，且没有已知 EOF、关闭要求或协议切换时为 true。新建的空闲连接也可以为 true；完成所有信息／最终响应和双向正文之前为 false。它复用本连接的编解码状态，不发送探测、不读取 socket，不保证尚未观察到的对端关闭不会使下一次操作失败。池使用这一事实判断复用资格，不能自己重建 HTTP/1 持久连接或切换规则。

H2 连接所有者作为调用者提供的 CoroutineScope 的子任务登记。共享传输读写使用该所有者的 Job；请求／流等待使用请求自己的 Job。请求取消只发出对应 RST_STREAM、丢弃该流剩余 body 并释放其等待，不取消共享 TLS read／write 的 Job。连接所有者取消关闭整条连接并完成所有流收尾。每次 native 步骤不另建 Job。

AsyncHttp2Connection 的 client／server 所有权工厂接收相同的 HTTP/2 options、limits、settings，以及可选的普通 onEvent 回调。openRequest 返回 AsyncHttp2Stream，acceptRequest 返回请求头与流；客户端流的 receiveResponse 逐条返回信息／最终响应，服务端流用 sendResponse 发送。流实现 AsyncByteStream／AsyncCloseable，readInto 与 writeFrom 允许双向并行，同一方向只能有一个等待；finish 提交结束标记和 trailer，flush 等待该流已接受的输出进入底层传输并完成其 flush，不等待对端确认。close 放弃本流余下数据并提交 CANCEL，不关闭共享连接。

HTTP/2 客户端及 upgraded-client 工厂同样可选 captureTimings（默认 false）。开启后 AsyncHttp2Stream.firstResponseByteAt 保存第一次由 receiveWithProgress 确认归属到该流的输入观测 TimeMark，尚无该流输入或未开启时为 None；这个值包含可能的信息响应，后续最终响应不重置。帧头尚未完整归属时不预先填入时间，其他流和连接控制帧不能填入本流的值。共享驱动只更新固定的流状态，不在此调用挂起 observer。两种协议未开启采样时沿原接收路径运行。

HTTP/2 的 upgraded 所有权工厂另外接收已完成 HTTP/1 请求的方法与协商 SETTINGS，upgradedStream 交付关联的 stream 1；服务端这一流的请求输入已经完成。goAway 提交并发送协议结束通知，关闭仍由独立的 close 负责；停止接收新请求且已无排队请求时 acceptRequest 返回 None。SETTINGS、忽略字段、流关闭、GOAWAY 和后端失败通过 Http2Notice 回调交付，回调在 native 返回后、锁外执行，不可挂起；回调抛出异常会结束连接。

服务端异步层转发 `Http2Notice.RequestHeadStarted(streamId)`；当完整请求已进入有界交付队列时另发 `RequestHeadReceived(streamId)`，排队拒绝不发后者。两者与流故障／关闭通知保持事件顺序，并在 acceptRequest 恢复之前交付相应的已入队通知。高层可据此结合自身活动请求状态管理首部总时限与真正空闲，不用其他流流量或控制帧刷新首部计时，也不轮询底层私有状态。

`AsyncHttp2Connection.isDraining` 是本地已提交或已收到 GOAWAY 的只读状态快照，在对应通知之前更新；它不表示已有流都已结束，也不替代 isClosed。仍开放但已经 draining 的连接在 openRequest 提交新流之前抛出 Http2ConnectionDrainingException，不分配 stream ID 或发出新请求。已经提交的流继续按 GOAWAY 的最后 stream ID 和实际流事件完成；对端明确未处理的流保留 REFUSED_STREAM 的原始错误码，不把任意连接失败解释成未处理。

`AsyncHttp2Connection.isClosing` 是连接已经开始本地关闭的只读状态快照：显式 close、所有者取消或连接失败选择终止时立即为 true，完成收尾后仍为 true。isClosed 只在实际传输与协议资源收尾完成后为 true。连接池不得把两者之间的关闭等待期当作可接受新请求的状态；这一判断不执行 I/O，也不改变 GOAWAY 和实际连接失败的分类。

Http2Notice.StreamFailure(streamId, cause) 在已存在的流因协议错误、请求取消、I/O 或时限而异常结束时交付一次原始 Throwable，包括连接结束时尚未正常完成的流；即使没有正在等待的 read／write／receiveResponse，也必须交付。它在流状态定稿后、锁外执行，不取代描述线上事件的 StreamClosed 或后端 Failure。正常完成和调用者显式 close 本流不额外产生 StreamFailure；close 仍按原有合同放弃数据并完成本地清理。AsyncHttp2Stream.failure 是该流已选择的终止原因快照，显式 close 的本地原因是 IoClosedException，正常完成不设置失败；异常结束后的快照与通知引用同一原因，后续 close 不覆盖它。使用者可在登记流之后查询一次快照，衔接此前已经交付的通知，不需要轮询或另建计时器。

HTTP/2 已正常关闭的流保留尚未被应用取走的完整响应头与 body，即使连接随后结束也可读取；连接已经释放后不再归还无效窗口。未完成或收到错误 reset 的流在关闭时释放排队数据并报告相应异常。

异步服务端待交付请求数量不超过本端 SETTINGS 的 maxConcurrentStreams，超出时拒绝对应流。每流最多暂存 16 条尚未被应用取走的信息／最终响应头，超过时以 ENHANCE_YOUR_CALM 结束该流。DATA 由实际连接／流窗口背压，应用消费或显式放弃后才归还窗口。上述限制仅约束实际消息队列，不扩展为通用资源预算。

同一 native 实例在短临界区中串行推进；底层 I/O、用户代码、Job 通知和 continuation 恢复在锁外。传输每方向至多一个操作；输出背压时仍允许输入推进，以便接收 WINDOW_UPDATE、响应与关闭事件。没有输入、输出空间或状态变化时等待实际事件，不空转。

body readInto／writeFrom 采用既有 NativeByteBuffer 前缀合同，允许短传输。等待到达的数据时无需借用调用者 buffer；实际复制时才同步借用。跨挂起只保留稳定 native allocation，底层借用结束后才复用或释放缓冲。零长度经状态、范围和取消检查后返回零，不推进网络。

### 7.1 读写停滞时限

HttpIoTimeoutOptions 的 readIdle／writeIdle 均为可选非负 Duration，默认 None；None 关闭对应计时。所有权工厂的 ioTimeouts 提供缺省配置，在接管前验证；连接的只读 ioTimeouts 属性可查询该配置。H1 的 sendRequest／sendResponse 可覆盖本条消息的 writeIdle，receiveRequest／receiveResponse 可覆盖本条消息的 readIdle；返回的 writer／body 保留该时限。H2 的 openRequest 可传入本流 ioTimeouts，服务端接受的流和 h2c 的 stream 1 使用所有权工厂的配置。覆盖在消息或流入口验证，只影响对应消息或流，不修改复用连接上其他调用的配置。

计时只在有待处理的 I/O 时开始。H1 从本次操作首次调用底层 read／write／flush 开始，以每次真实正数短读／短写重置；当前操作结束后停止。读取已缓冲正文、零长度读取、应用处理数据和两次操作之间的停顿不启动读计时。flush 不虚构字节进度，消息结束与底层 flush 仍受尚未完成的写等待约束。零时限在需要 I/O 时立即到期；已有缓冲可直接完成的读取不因零时限失败。

H2 的读计时仅在该流等待响应头或 body 时有效，按 receiveWithProgress 的本流输入推进；尚未完整到能确认归属的帧头不延长任何流的时限。写计时覆盖该流已接受但尚未发送／flush 的首部、DATA 和结束标记，包括等待流窗口的期间；writeFrom 只把数据入队时，后台写计时继续有效。实际传输按 takeOutputChunk 的流号与类别归属，其他流和 Control 不重置本流时钟。应用暂停 producer 且没有已排队输出时不计写停滞，应用没有发起读取时不计读停滞。完成边界在恢复调用方前停止不再有待处理 I/O 的计时，调用方随后同步处理数据也不延续已经结束的等待。

HttpIoTimeoutException 保存 direction（HttpIoDirection.Read／Write）、protocol、streamId（H1 为零）、limit 和自上次进度以来的 elapsed。同步完成的传输也在完成边界检查已到期时限，迟到字节不把超时改成成功。H1 超时关闭该连接，H2 超时以 CANCEL 结束对应流并保留共享连接；由同次关闭产生的 IoClosedException 不覆盖已选择的超时，独立 I/O 错误和 ResourceUseException 的清理原因仍保留。

计时采用普通 CoroutineScope 子任务、EventLoop.sleep 与单调时钟；有进度时调整下一次剩余等待，不为每个 native 步骤创建任务。操作／流结束后取消计时，close 等待所属定时 I/O 的真实收尾；已取消 Job 的清理仍可完成。这里不增加另一套调度器，client 只传入配置并映射错误。

### 7.2 关闭与所有权

close 只保证本地收尾，不隐含 flush 或对端确认；先拒绝新操作，关闭接管的传输，等待实际 I/O、native 借用和未交付资源结束，再释放协议状态。并发 close 等待同一次收尾，后续幂等。允许在已取消 Job 的 finally 中完成；清理不能因该取消提前退出。正文和清理同时失败使用 ResourceUseException。

HTTP/1 已因本次操作的任务取消选择关闭原因后，底层读取／写入因同一次关闭返回的 IoClosedException 不替换原 CancellationException；独立的协议或 I/O 故障仍保留原类型。由此产生的清理失败仍通过 ResourceUseException 保留，取消结果也必须等实际关闭及借用结束后交付。

协议优雅结束为独立操作，由应用使用现有 Job／定时器控制等待。GC release hook 不驱动协议、不等待网络，不代替显式 close。所有权移交与关闭都必须解除不再需要的 continuation、Job 监听和 buffer 引用。

HttpTransport.takeOwnershipWithPrefix 接管已有传输与字节前缀快照，支持调用者将 HttpCleartextProbe 消费的前缀和输入 buffer 的后缀一起交给选定的协议连接。这个前缀适配器没有后台任务；读取先返回前缀，之后使用原传输，关闭只收尾本地资源。协议连接仍按上述规则登记自己的 scope 子任务。

## 8. 构建、交付与验证

两个 Cone 使用 M52 的 `sysroot/src` 布局。llhttp 固定源码放入 vendor，以普通 native.sources 编译；nghttp2 的官方源码归档、许可放入 vendor，通过仓库维护脚本生成 `lib/<triple>/nghttp2/include` 和 `lib/libnghttp2.a`。仅构建静态协议库，关闭上游应用、示例、工具、测试、文档、调试输出与 HTTP/3 应用选项。

HTTP Cone 只声明一次 nghttp2 静态库，通过既有目标条件 include 和 native.libraries 消费。默认目标 native 搜索根增加 nghttp2/lib，未声明 HTTP 的程序不额外链接它。源准备和打包仍在 scripts，不增加 scoop 或 driver 子命令。普通构建不下载或重建第三方依赖；缓存按实际输入更新，命中不读回或复验产物。

验证覆盖独立 llhttp／nghttp2 对端、常规与非法报文、任意分段、字段快照、重复项、非 ASCII、每项公开兼容开关、H2 HPACK／流控／并发流／错误作用域、ALPN、prior knowledge、h2c 接受与拒绝，以及切换后缀。异步组合使用真实 loopback、短读写、双向背压、单流与连接取消、并发关闭和 moving GC。

Darwin/AArch64、GNU amd64、musl amd64 运行适用的 debug／release 库测试及生产产物消费者。独立工具只属于开发验证，不进入分发包。发布包按 M52 规则手工验证，不为每次常规测试重新打包；原始日志和工作数据不提交。

## 9. M54 协议层补齐

### 9.1 扩展方法与上传协调

Http1Encoder、客户端 expectResponse 方法登记及异步 sendRequest 接受任意合法 method token，保留原始大小写和字节。响应定界上下文只区分精确的 HEAD、CONNECT 与普通方法；这一语义分类不是将线上 method 改写成 GET。移除以 llhttp 请求方法枚举校验编码器输入的限制，不能让 web client 为扩展方法强制选择 H2 或自己发送请求行。

服务端请求解析仍受 llhttp 的实际能力限制，不为 M54 客户端需求另外维护请求解析器；独立 native 对端用于验证扩展方法的真实发送与响应接收。HEAD、成功 CONNECT、普通扩展方法、大小写不同 token 和 100／最终响应的关联须组合验证。

H1／H2 异步连接须支持请求写入与响应头读取同时推进，暴露 100 Continue 与早期最终响应的事实。若 M53 现有协调在这些组合上存在缺陷，在 scoop.http.asyncio 修复并回归；client 只实现等待策略。H2 的 SETTINGS、GOAWAY、stream ID、关闭与流容量事件继续使用现有 Http2Notice，不重复解析帧或依赖私有 nghttp2 状态。

### 9.2 消息工具与长连接协议

[HTTP 消息工具规范](SCOOP-HTTP-MESSAGE-SPEC.md)属于本规范的 M54 补充，规定字段参数、日期、认证／缓存字段、Cookie／PSL、multipart、CONNECT 辅助、SSE 与 HTTP/1.1 WebSocket 的公共合同。普通消息数据与内存 parser 位于 scoop.http，相应异步流适配位于 scoop.http.asyncio。

[AWS EventStream 接收规范](SCOOP-HTTP-EVENTSTREAM-SPEC.md)补充同一 body 上的二进制消息读取，为 ConverseStream 提供帧边界、typed headers 与 CRC32 校验。内存 decoder 位于 scoop.http，借用／接管 source 位于 scoop.http.asyncio；校验复用已有 zlib-ng，服务事件含义留在调用方。

[NDJSON 接收规范](SCOOP-HTTP-NDJSON-SPEC.md)补充逐行 UTF-8 记录定界，为 Ollama 原生流提供 LF／CRLF 分块、单记录背压和借用／接管 source；JSON 与服务事件仍由调用方解释。

基础 HTTP Cone 按实际使用增加 url、charset 与 crypto 的普通依赖，仍不依赖 asyncio、TLS 或 socket。异步 HTTP Cone 增加 compression 依赖，用于 WebSocket 消息压缩；由于 compression 在一个 Cone 内交付异步适配，基础 HTTP 引擎不反向依赖它。HTTP client 只组合这些公开能力。

这些新增能力分别验收后才作为 client 前置条件完成；M53 的历史验收不被改写为 M54 已通过。HTTP/2 WebSocket、Extended CONNECT 与 HTTP/3 明确留待后续。
