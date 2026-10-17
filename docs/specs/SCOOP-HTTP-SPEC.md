# Scoop HTTP 库规范

版本：M53。实施范围见 [M53 设计](../milestone53/DESIGN.md)。本规范使用[库 API 风格](SCOOP-API-STYLE.md)、[异步 I/O](SCOOP-ASYNCIO-SPEC.md)和 [TLS](SCOOP-TLS-SPEC.md)的既有合同，不改变语言语义、runtime ABI 或编译阶段职责。

## 1. 分层与后端

`scoop:scoop.http:0.1.0` 提供字节消息、HTTP/1 增量编解码、HTTP/2 内存会话和协商数据处理，依赖 core、std、platform。`scoop:scoop.http.asyncio:0.1.0` 在普通 AsyncByteStream／AsyncCloseable 上协调传输、body 与连接生命周期，依赖 HTTP 和 asyncio。前者不依赖 libuv、TLS 或 socket；后者不负责连接 TCP、TLS 握手或证书校验。

HTTP/1 使用 llhttp 9.4.3；HTTP/2 使用 nghttp2 1.70.0 的完整上游协议核心，通过 `nghttp2_session_mem_recv2`／`nghttp2_session_mem_send2` 接入。帧、HPACK、HTTP messaging、流状态与 SETTINGS 校验交给 nghttp2；Scoop 不再重放一套接收校验。

不增加连接池、自动重试、路由、Cookie、缓存、下载命名、内容解压或 HTTP/3。HTTP/2 的库核心保持完整，本期普通消息接口不提供 server push 或通用扩展插件。客户端明确声明禁用 push；这不等于裁掉后端机器码。

## 2. 消息、字段与错误

HttpField 的构造参数为 `List<Byte>`，名称和值各取得独立 `Array<Byte>` 快照，另有 neverIndex。字段集合保留顺序和重复项；ASCII 大小写无关查询返回每一项，不合并 Set-Cookie 等同名值。字段值允许后端实际接受的非 ASCII／非 UTF-8 字节。文本转换由调用者通过已有编码 API 显式执行。

方法保留 token 字节。HTTP/1 request-target 保留原有字节；HTTP/2 的 scheme、authority、path 与 CONNECT authority 单独表达。响应有版本、状态码和字段；HTTP/1 保留实际 reason phrase，HTTP/2 没有该数据，不生成默认文本。正常接收的请求／响应头必须具备该消息所需信息，不能把未完成的字段块当作消息交付。

公开 HttpRequestHead 使用 HttpRequestTarget 的 Http1(raw)、Http2(scheme, authority, path)、Connect(authority) 形状；各字节值为 Array 快照。HttpResponseHead 的 reason 是可选字节快照。HttpHeaderLimits 按字段名称和值的合计字节数、单项字节数、字段数量限制接收；Http1Limits 另限制起始行各组成数据的合计字节数，不把分隔符或 OWS 计为字段数据。

body 交付解开 chunked／DATA 定界后的内容字节；不按 Content-Type、Content-Encoding、字符集或百分号编码解释。trailer 独立于初始字段，能够区分未完成、已完成且为空、收到非空字段。信息响应、最终响应、body 结束和协议切换分别可观察。

这些快照表示后端交付的数据，不保证是原始报文镜像。llhttp 可能跳过起始 OWS 或展开接受的折行；nghttp2 会过滤部分字段，HPACK 表示不保留。需要原始报文的应用在原有传输上记录字节，HTTP 不增加另一套记录机制。

HttpException 保存操作、协议、作用域、stream ID、后端码及已复制的说明。HTTP/1 的消息／定界错误属于连接；HTTP/2 区分操作参数、单流错误与连接错误，RST_STREAM、GOAWAY 和后端忽略的普通字段以实际事件交付。底层 I/O 异常保留原类型。native 错误先复制再返回 Scoop 抛出，不穿越 C ABI 展开。

## 3. HTTP/1 接收

Http1Decoder 的 client／server 工厂固定接收角色、选项和局部字段限制；对象实现 Closeable，调用者串行使用。receive 消费 NativeByteBuffer 中的实际前缀，nextEvent 取走已完成事件，endInput 明确交付 EOF。关闭后不能继续推进；关闭本身不执行 I/O。

client 在发送每个请求时按顺序登记方法上下文。信息响应不移除此上下文，最终响应结束后才移除。HEAD 响应没有 body；成功 CONNECT 与 101 交付切换事件，保留同次输入的全部未消费后缀。没有对应请求上下文的响应是调用状态错误，不能靠接收宽容选项补足。

服务端收到 Upgrade／CONNECT 请求时，切换事件表示 parser 停在该请求边界，是否接受由服务端响应决定。拒绝请求的升级后调用 resumeHttp1 恢复 HTTP/1；接受后移交后缀，不再恢复旧 parser。

解析支持 HTTP/1.0／1.1、连续消息、Content-Length、chunked、trailer 和 EOF 定界。起始行、字段、chunk 可以任意分段；字段回调片段必须组合到字段完成后才交付。只有需要 EOF 定界的消息在正常 EOF 时完成；截断定长／chunked 报文报错。

llhttp 9.4.3 只接受其已知方法集合。`CUSTOM` 等集合之外的方法明确诊断为该版本不支持的方法；公共 token 数据模型不改变这个接收限制。HTTP/1 发送同样在开始前检查此限制，HTTP/2 的方法不受 llhttp 限制。不能改写方法或预修复报文绕过后端。

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

响应编码持有对应方法上下文，处理 HEAD、信息响应、204、304 和成功 CONNECT 的实际 body 规则。101／成功 CONNECT 之后旧编码器不再发送 HTTP 消息。已被本层接受的 body 前缀不因后续传输失败或取消而回滚或自动重发。

## 5. HTTP/2 内存会话

Http2Session 的 client／server 工厂固定角色、接收选项、本地限制和初始 SETTINGS。创建后可提交请求、信息／最终响应、body、trailer、RST_STREAM 与 GOAWAY；请求提交返回实际 stream ID。DATA provider 只读取已准备的 native 字节，缺少数据返回 deferred，补充字节后调用 resume_data。不在 C 回调内执行异步 I/O、用户回调或 continuation 恢复。

receive／takeOutput 使用 NativeByteBuffer 的实际前缀。mem_send2 返回的临时字节必须在下一次可能使指针失效的调用前复制到自有存储；不能跨挂起保存该指针。输出每次可短传输，未取走的输出保持有效。

请求／响应头、trailer、DATA、流结束、RST_STREAM、GOAWAY、SETTINGS 与忽略字段各自可观察。非法头或消息错误按后端真实作用域处理；普通消息接口不暴露 no_http_messaging。发送头根据消息数据生成合法伪头顺序和小写字段名，使用后端公开的名称／值检查；调用者不能把伪头混入普通字段。

DATA 收到后保持为未消费量。调用者实际读取或明确丢弃 body 时才调用 consume，归还相应的流与连接窗口。流已经结束／reset 而仍有未消费数据时，按上游 consume_connection 合同归还连接窗口，不能因单流结束阻塞其他流。不能重复消费或超过实际未消费量。

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

明文客户端明确选择 HTTP/1、H2 prior knowledge 或 h2c；服务端声明允许的入口。H2 前言识别支持任意分段，判别期间保留所有输入，最终仍交给唯一的协议消费者；不能丢失部分匹配后转入 HTTP/1 的字节。没有允许的协议、截断的 H2 前言或文本 HTTP/2 都不能静默回退。

h2c 显式生成／验证 Upgrade、Connection、HTTP2-Settings；HTTP2-Settings 采用无 padding 的 base64url SETTINGS payload。升级请求的 HTTP/1 body 必须完成，随后调用 nghttp2_session_upgrade2，把已发送／收到的请求关联到 stream 1，不重复交付或重发 body。HEAD 标记传给上游。拒绝升级时继续交付真实 HTTP/1 响应，不自动重放请求。

101、成功 CONNECT 及其他协议切换可移交底层 AsyncByteStream／AsyncCloseable 和未消费后缀。移交后旧连接停止协议处理并释放自己的状态；新拥有者先消费后缀，再读取传输。关闭旧连接不能再次关闭已移交资源。

## 7. 异步连接、body 与关闭

异步入口使用 takeOwnership 命名，固定 EventLoop 归属，并接收实际结构化生命周期所需的 CoroutineScope。配置、参数、活动 Job 和登记前取消检查失败时，传输仍归调用者。成功登记后接管，后续失败关闭未交付的连接与 native 状态。交付后不得绕过 HTTP 包装层并发使用传输。

H1 按顺序关联请求与响应，同一方向只允许一个消息 body 操作；未定界完成的 body 不能作为下一条消息。关闭或取消未消费完的 body 关闭连接，不执行无界自动 drain。信息响应不结束对应请求。H2 用 stream ID 分开关联请求、响应和双向 body，单流可以独立结束或取消。

H2 连接所有者作为调用者提供的 CoroutineScope 的子任务登记。共享传输读写使用该所有者的 Job；请求／流等待使用请求自己的 Job。请求取消只发出对应 RST_STREAM、丢弃该流剩余 body 并释放其等待，不取消共享 TLS read／write 的 Job。连接所有者取消关闭整条连接并完成所有流收尾。每次 native 步骤不另建 Job。

同一 native 实例在短临界区中串行推进；底层 I/O、用户代码、Job 通知和 continuation 恢复在锁外。传输每方向至多一个操作；输出背压时仍允许输入推进，以便接收 WINDOW_UPDATE、响应与关闭事件。没有输入、输出空间或状态变化时等待实际事件，不空转。

body readInto／writeFrom 采用既有 NativeByteBuffer 前缀合同，允许短传输。等待到达的数据时无需借用调用者 buffer；实际复制时才同步借用。跨挂起只保留稳定 native allocation，底层借用结束后才复用或释放缓冲。零长度经状态、范围和取消检查后返回零，不推进网络。

close 只保证本地收尾，不隐含 flush 或对端确认；先拒绝新操作，关闭接管的传输，等待实际 I/O、native 借用和未交付资源结束，再释放协议状态。并发 close 等待同一次收尾，后续幂等。允许在已取消 Job 的 finally 中完成；清理不能因该取消提前退出。正文和清理同时失败使用 ResourceUseException。

协议优雅结束为独立操作，由应用使用现有 Job／定时器控制等待。GC release hook 不驱动协议、不等待网络，不代替显式 close。所有权移交与关闭都必须解除不再需要的 continuation、Job 监听和 buffer 引用。

## 8. 构建、交付与验证

两个 Cone 使用 M52 的 `sysroot/src` 布局。llhttp 固定源码放入 vendor，以普通 native.sources 编译；nghttp2 的官方源码归档、许可放入 vendor，通过仓库维护脚本生成 `lib/<triple>/nghttp2/include` 和 `lib/libnghttp2.a`。仅构建静态协议库，关闭上游应用、示例、工具、测试、文档、调试输出与 HTTP/3 应用选项。

HTTP Cone 只声明一次 nghttp2 静态库，通过既有目标条件 include 和 native.libraries 消费。默认目标 native 搜索根增加 nghttp2/lib，未声明 HTTP 的程序不额外链接它。源准备和打包仍在 scripts，不增加 scoop 或 driver 子命令。普通构建不下载或重建第三方依赖；缓存按实际输入更新，命中不读回或复验产物。

验证覆盖独立 llhttp／nghttp2 对端、常规与非法报文、任意分段、字段快照、重复项、非 ASCII、每项公开兼容开关、H2 HPACK／流控／并发流／错误作用域、ALPN、prior knowledge、h2c 接受与拒绝，以及切换后缀。异步组合使用真实 loopback、短读写、双向背压、单流与连接取消、并发关闭和 moving GC。

Darwin/AArch64、GNU amd64、musl amd64 运行适用的 debug／release 库测试及生产产物消费者。独立工具只属于开发验证，不进入分发包。发布包按 M52 规则手工验证，不为每次常规测试重新打包；原始日志和工作数据不提交。
