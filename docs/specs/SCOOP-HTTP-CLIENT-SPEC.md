# Scoop HTTP client 库规范

版本：M54 实施中；实际完成与验证范围见[实施进度](../milestone54/PROGRESS.md)。范围、底层缺口和实施次序见 [M54 设计](../milestone54/DESIGN.md)。本规范基于 [HTTP 协议库](SCOOP-HTTP-SPEC.md)、[异步 I/O](SCOOP-ASYNCIO-SPEC.md)、[TLS](SCOOP-TLS-SPEC.md)、[URL](SCOOP-URL-SPEC.md)、[IDNA](SCOOP-IDNA-SPEC.md)、[压缩](SCOOP-COMPRESSION-SPEC.md)与[字符集](SCOOP-CHARSET-SPEC.md)的公共合同。

## 1. 目标与边界

提供普通 Cone `scoop:scoop.http.client:0.1.0`，公共 package 为 `scoop.http.client`。全部实现使用普通 Scoop，不声明 Extern／Intrinsic、不携带 native source、不调用依赖库的 Unsafe／native 私有入口。依赖库负责系统、密码和协议后端；纯 Scoop 指 client Cone 本身，不指整个网络栈没有 C 依赖。

统一接口覆盖 HTTP/1.0、HTTP/1.1 与 HTTP/2，提供连接复用、流式请求／响应、协商、超时、取消、重定向、重试、认证、代理、Cookie、缓存、文本／JSON／表单／multipart、SSE 与 HTTP/1.1 WebSocket。异步 I/O 是主要执行接口，不在内部运行嵌套 EventLoop、阻塞等候 suspend 结果或逐调用建立线程池。

本期不提供 HTTP/3／QUIC、HTTP/2 WebSocket／Extended CONNECT、跨 origin 的 HTTP/2 连接合并、证书固定／对端证书链检查接口、TLS 会话恢复控制／0-RTT、主动 HTTP/2 PING 保活。普通同 origin 连接池、HTTP/2 SETTINGS／GOAWAY 处理、闲置回收与 WebSocket Ping／Pong 属于本期。server push 保持禁用。不存在上述后续能力的占位枚举、配置或运行时“尚未支持”分支。

## 2. 客户端、调用和生命周期

| 入口 | 合同 |
| --- | --- |
| `HttpClient.open(scope, config): HttpClient` | suspend 创建，绑定当前 EventLoop，在给定活动 scope 下登记所有者并准备所需配置 |
| `withHttpClient(config, block)` | 泛型 suspend 便利入口，在内部 coroutineScope 中创建 client，执行 suspend block，并在 scope 正文退出前 close |
| `client.execute(request): HttpResponse` | suspend 至最终响应头可用，返回由调用者负责关闭的流式响应 |
| `client.withResponse(request, block)` | suspend 作用域借用；无论正文是否完整读取，退出均关闭响应 |
| `client.close()` | AsyncCloseable，本地关闭池、请求、协议会话及所拥有资源，并等待收尾 |

HttpClient 可在同一 EventLoop 的多个任务中并发使用，配置创建后不可变。open 失败清理已接管资源；传入的外部策略对象、证书配置与缓存是否接管由对应工厂明确，普通参数默认借用。实现不得用 GC 或 EventLoop 的最终扫尾替代正常关闭。

HttpClientConfig 保存协议策略、明文入口、HTTP/1 请求版本，以及所属库的 TCP、HTTP/1、HTTP/2 接收选项／限制／SETTINGS。HttpClientTlsConfig 保存 TLS 的 trust、minimum、maximum 和 identity；open 通过 TLS 的公开 suspend 工厂准备客户端上下文，client 关闭该上下文，外部信任库和身份仍由调用者拥有。HttpCallOptions 的 duplex 显式请求 HTTP/2 双向上传，onInformational 是在本次调用 Context 中执行的 suspend 响应头回调。无效的已知协议组合在网络提交前拒绝；选中 HTTP/1 连接后不能满足 duplex 时，在提交请求头前报错。

client 的所有者在一个正文仍活动的子 coroutineScope 中维持连接任务与调用任务，直至 close；不能在 scope 正文返回后再尝试登记新连接。调用 owner 持续到请求发送与响应消费均完成或放弃，不在 execute 返回头部时结束。调用者 Job 的取消登记维持到本次调用收尾；execute 不创建一个必须等 body 结束才能返回的临时 coroutineScope。显式响应须在 client 的结构化生命周期内使用并关闭，不把引用带出 withHttpClient；漏关资源由 client.close 收尾。

连接和请求分别持有自己的 Job。HTTP/2 共享 TLS 的读写归连接 owner；请求 body、头部等待和超时归调用 owner，单请求取消只关闭相应流。HTTP/1 未完成消息取消使连接不可复用并关闭。一次网络失败作为调用结果交付；内部候选连接／上传等子任务必须在任务内部捕获预期操作失败，不在 await 处才捕获已经传播到父 scope 的异常。producer 和用户回调的嵌套 coroutineScope 按语言规范 11.9.1 在子树收尾后从调用处交付失败，client 取得原失败而不取消其他请求。

调用任务在发起者的 Context 中创建，producer、认证和调用级回调保留该调用的动态 binding；共享连接驱动使用连接自己的 Context，不把第一条请求的业务 Context 用于所有后续流。切换 Job 不得丢失其他有效 binding。

close 拒绝新请求与新借用，唤醒池等待者，取消活动调用，关闭连接，并等待实际 I/O、native 借用和子任务结束。close 可在已取消 Job 的 finally 中完成，并发调用等待同一轮收尾，之后幂等；清理失败使用既有 ResourceUseException。正常响应消费完成会归还连接／流容量；显式关闭不执行无界 drain。

client.close 使尚未选定失败的活动调用报告 HttpClientClosedException；caller 或当前正文 reader 的 Job 取消保留 CancellationException。此前已经选定的操作失败不被后来的关闭或取消覆盖。

所有者的正文通过普通 continuation 保持活动；内部 coroutineScope 的最终 completion 在其子任务真实结束后才释放该正文并通知 close 等待者。连接创建由 client 的协调任务发起，请求 producer 和回调由 execute 所在 Context 发起；不需要公开 Context 快照、访问 Job 私有 join 或创建另一种任务调度器。操作失败在所属任务正文内转为本次调用的结果，不能作为未捕获子任务失败传播给 client 的其他调用。

响应头已经交付后，后台上传或收尾的失败由后续正文操作报告；如果没有继续读取，首次响应 close 报告尚未观察的失败。已经由读取交付的主失败及其包含的清理失败不在 close 重复报告，之后实际发生而尚未交付的清理失败仍保留；完成后的重复 close 幂等。

取消时先关闭实际协议、codec 和缓存资源以解除在途操作，再等待活动正文读取记录其结果与清理错误，随后交付调用完成和 client.close。底层资源的 close 已经返回，不代表借用它的上层读取／commit 回调已经完成；完成通知不能越过该结果交接，也不能使读取失败后的等待形成自等待。

## 3. 请求、响应与 body

`HttpRequest` 是不可变配置快照，含 method、Url、有序重复 HttpField、RequestBody 和每次调用的选项。method 为合法 token，提供常用方法工厂，同时支持扩展方法。URL、字段和 body 类型沿用所属库；client 不定义另一套大小写比较或字节字段模型。

普通构造的 method 为 String，保存其原始大小写并由 HTTP 的 token 规则检查；fields 复制并以 HTTP 的普通字段规则校验。Host／伪头与 Transfer-Encoding 由实际 URL 和协议生成，调用者不能提供这两套来源；显式 Content-Length 必须与已知 body 长度一致，或作为未知长度 producer 的受检长度。body 的 Content-Type 在字段缺省时补充，压缩 body 的 Content-Encoding 与手写同名字段互斥。请求中的 userinfo 不自动变成认证凭据，fragment 不进入 target。

`HttpResponse` 保存实际协议版本、最终 status、原始字段快照、最终 URL、重定向历史、body 与调用信息。HTTP/1 reason phrase 保留实际字节，HTTP/2 不补默认文本。非 2xx 默认是正常响应；`requireSuccess()` 显式抛包含状态与头部快照的 HttpStatusException，不暗中读完整错误 body。

信息响应与最终响应分开观察；100 负责 Continue 门控，103 等通过调用级事件交付。trailer 只在 body 完成后可用，区分未完成与完成且为空。仅收到头部或一部分 body 不能提交完整缓存条目、宣称下载完成或计算完整吞吐量。

RequestBody 至少提供 Empty、字节快照、文本、JSON、表单、multipart、文件、单次 AsyncByteSource、可重新打开的 source factory 和 suspend producer。source 或 producer 在真正开始发送该次 body 时才启动；工厂和 producer 可挂起，使用调用的 Job。每次返回的 body 资源由该次发送负责关闭；接受 body 参数不立即消耗其数据。

可重放 body 用能够每次创建新实例的 factory 表达，另保存可选 contentLength 和 trailer producer；不能只有一个 `replayable = true` 标志。字节／文本快照天然可重放，单次 source／producer 默认不可重放，可重放 producer 必须显式声明创建新的生产过程。文件 body 使用 asyncio 打开与读取，调用者须保持重试期间内容一致，重新打开不等于创建文件快照。

body 的已知长度必须与实际字节数一致，不能提前 EOF 或超长后继续复用连接。HTTP/1.1 未知长度采用 chunked，HTTP/2 采用 DATA／END_STREAM。HTTP/1.0 请求不使用 chunked；未知长度请求体需调用者给出长度或明确有限缓冲，不能悄悄读取整个上传文件。trailer 按各协议规则由 HTTP 库编码，HTTP/1.0 不接受 trailer。

响应 body 实现 AsyncByteSource／AsyncCloseable。同一 body 只能有一个消费者；bytes、text、JSON、copyTo 和其他解码视图共享这一次消费，不能同时读取 raw 与 decoded 两条伪独立流。便利入口明确传入 maxBytes，或使用调用方设置的有限缓冲上限；流式 copyTo 不要求把完整消息放入内存。取消及错误保留已交付的实际进度，不回滚文件或用户 sink。

JSON 便利函数通过现有 Json 与显式 ValueEncoder／ValueDecoder 工作，完整 JSON 校验遵守语言规范 11.13。typed JSON body 是明确的有限缓冲转换，不声称业务 codec 可挂起或能逐元素处理无限 JSON；大 body 使用流接口，格式能力若需扩展必须在 scoop.json 补齐。非 UTF-8 文本使用 scoop.charset，不在 client 内实现编码器。

### 3.1 Body 的基本工厂与发送

`RequestBody` 保存可选 contentLength／contentType，以及实际的一次性生产过程或可重新创建内容的工厂；isReplayable 只报告该结构提供的能力。Empty、fromByteArray、fromText、fromJson、fromForm 产生可重复发送的独立字节快照；fromByteArray 接收 List<Byte>。fromText 使用显式 Charset（默认 UTF-8）严格编码并生成相应 text/plain 媒体类型。fromJson 接收 ValueEncoder 和 maxBytes，在调用现有 Json 完成编码后检查 UTF-8 结果长度；该上限约束接受的表示，不是 Json 编码过程的内存预算。fromForm 使用 URL 库的 FormUrlEncoded 与 UrlQuery。

fromSource 接收单次借用的 AsyncByteSource，调用者负责其完整生命周期；构造不读取，未发送时也不关闭。fromSourceFactory 的 suspend 工厂在每次实际发送时创建同时满足 AsyncByteSource／AsyncCloseable 的独立来源，该次发送负责关闭，正文和关闭同时失败用 ResourceUseException。fromFile 是后者的便利入口，每次用当前 EventLoop 打开指定 Path，不在构造时同步查询或保存文件快照；可显式提供已知 contentLength，缺省为未知。

fromProducer 接收一次性的 suspend `(AsyncByteSink) -> Unit`；fromProducerFactory 接收每次创建新 producer 的 suspend 工厂。producer 负责在返回前完成自己的子任务和释放自己打开的资源，借用的 sink 不能带出调用。单次来源或 producer 在发送实际开始时原子地标记已使用，失败后也不能再次发送；重复使用报 RequestBodyConsumedException。尚未实际发送、参数检查失败或入口已经取消不消耗该次机会。

`body.writeTo(destination, bufferSize = 8192L): Long` 是普通 suspend 的表示字节发送入口，借用 destination，不执行 HTTP 定界或关闭 destination；返回完整接受的表示字节数。writeTo 本身不补协议结束或额外 flush，producer 及所组合编码流的 finish／flush 仍按其合同向下游排出数据。内存快照和来源复制使用有限缓冲，工厂、producer 与 destination 调用均保留发起者 Context。已知长度通过实际输出进度检查；超长在提交超出部分前拒绝，提前结束和超长均报 RequestBodyLengthException，其中 expectedLength、transferredBytes 和 rejectedBytes 区分已发生进度与未提交数据。下游错误保留原异常，即使 producer 捕获下游错误后正常返回，本次发送也不转为成功。发送结束后保存的 sink 失效，同一 sink 不允许并发写入或 flush。

### 3.2 Multipart 与上传压缩

RequestBodyPart 以 body 和有序 fields 创建不可变 part head，bodyLength 来自该 body 的已知长度；formData(name, body, filename) 工厂使用 HTTP 库的 RFC 7578 字段生成器。构造复制 fields；缺少 Content-Type 时补充 body 的媒体类型，已经显式提供时保留。body 若包含编码描述则自动生成 Content-Encoding，此时再提供同名字段报配置冲突，不生成两套互相矛盾的编码声明。

`RequestBody.fromMultipart(parts, boundary = MultipartBoundary.generate(), subtype = "form-data")` 保存 part 列表快照及固定 boundary，每次发送创建新的 MultipartSink，按 part 消费顺序完成正文、part 结束和整个 multipart 结束。contentLength 使用 HTTP 库的 MultipartEncoder.contentLength；任一 part 长度未知则整体未知，不为求长度预读。只有所有 part 均可重放时整体可重放；包含单次来源的整体也只允许发送一次，重复调用在输出新 delimiter 之前失败。边界字节、quoted 参数与文件名规则全部由 HTTP 负责。嵌套 multipart 使用同一组合；调用者负责提供不会出现在 part 正文中的显式 boundary，缺省边界使用 HTTP 的安全随机生成器。

`body.compressed(options = CompressionOptions.Gzip(GzipOptions()))` 返回新的流式 body，复用 AsyncCompressionSink，先写完原 body 再显式 finish 压缩流，并关闭本次 codec；不接管 HTTP 输出端。支持 gzip、zlib 包装的 HTTP deflate、br 与 zstd，raw DEFLATE 不是 HTTP deflate 表示，配置时拒绝。contentType 保留，contentLength 改为未知，contentEncodings 按实际施加次序追加；重复压缩明确表示多层编码。原 body 的重放约束继续有效，非重放组合只允许一次发送。字典等外部配置按 compression 的借用规则保持有效；要用于认证摘要或重试，调用者须使各次产生的原文和编码选项一致，不把重新打开变化中的输入等同于字节快照。

### 3.3 每次发送的请求 trailer

fromProducerWithTrailers(trailerNames, contentLength, contentType, producer) 接收一次性的 suspend producer，签名为 `(AsyncByteSink) -> List<HttpField>`；它在同一次调用中写出正文并返回本次 trailer。fromProducerWithTrailersFactory 接收每次创建上述 producer 的 suspend 工厂，重放能力由实际工厂提供；不会把不同发送过程的摘要、计数或 trailer 状态放进共享全局回调。单次资格、Context、取消与来源关闭遵守 §3.1；等待 Continue 被最终响应拒绝时不启动该 producer。

trailerNames 通过 HTTP 库的 HttpTrailerDeclaration 复制和规范化，RequestBody 公开不可变名称数组。名称在构造时检查，实际字段在 producer 返回后复制并检查均已声明；允许空结果、少于声明的名称和有序重复字段。字段值及协议发送规则由 HTTP 编码器检查；client 不维护禁止字段名单。body 提供声明时，手写 Trailer 字段报冲突，实际请求头使用 body 的声明。

`body.writeWithTrailersTo(destination, bufferSize = 8192L): RequestBodyWriteResult` 借用目的端，返回 transferredBytes 和本次不可变 trailer 数组，不提交 HTTP 结束标记。已知正文长度按原规则受检；字节与 trailer 同属一次发送的结果。原 writeTo 继续返回正文长度，但有 trailer 声明时在消耗 producer 前拒绝，要求调用者选择能接收 trailer 的入口，避免组合过程中静默丢失尾部字段。

HTTP/1.1 中声明了 trailer 的请求始终使用 chunked，包括已知长度和空正文；已知或显式 Content-Length 仍作为本地长度断言，但不与 Transfer-Encoding 同时发送。HTTP/2 保留适用的 Content-Length，通过尾部 HEADERS 结束相应流；HTTP/1.0 在发送头部前拒绝。h2c 的原 HTTP/1 上传完成全部 trailer 后才移交，不在 stream 1 重放。

压缩组合保留声明与本次实际 trailer，在压缩流 finish 完成后才交给 HTTP；库不改写调用者的摘要含义，若摘要针对压缩后的字节，producer 须在相应编码层计算。Multipart part 没有 HTTP trailer 定界，RequestBodyPart 在构造时拒绝携带 trailer 的 body；整个 multipart 可由外层 trailer producer 包装。任何 producer、长度、声明或编码错误都按当前请求的失败路径收尾，不提交一个成功的消息结束来掩盖失败。

## 4. 上传、早期响应与背压

收响应与写请求可以双向推进。发送大 body 时仍必须能够处理早期 401／413、信息响应和 HTTP/2 WINDOW_UPDATE，不能先等待上传结束再启动响应读取。HTTP/2 的显式 duplex producer 可以在读取响应期间继续上传；每方向仍至多一个在途操作。HTTP/1 不增加 pipelining，早期最终响应后未能完整定界的请求使连接关闭。

最终响应头可用后无需等待 producer 结束才交付。需要停止上传时，协调器停止提交新数据，并按协议结束或在响应消费／放弃后关闭该 exchange；不能为了停止上传而先取消共享 TLS，或使尚需交付的早期响应 body 丢失。认证／重定向需要放弃旧响应时，可以按明确的 body.close 合同终止旧请求并开始下一跳。

`Expect: 100-continue` 可由调用者选择，另有独立等待间隔；100 或间隔到期才开始 body，先到最终响应则不启动 producer。间隔到期不是整个请求超时，也不能重发已接受的 body。HTTP/2 上仍处理该 HTTP 语义，但实际 DATA 和流控交给协议库。

HttpCallOptions 的 expectContinue 默认为 false；设为 true 时为有正文的请求添加 100-continue expectation。continueWait 默认 1 秒，必须非负，零表示请求头提交后立即允许上传。调用者显式提供的标准 100-continue 同样启用门控，并使用该次 continueWait；协议库负责区分未知 expectation、带值／参数的扩展和裸 100-continue，client 不拆分或重新解释字段字节。自动添加不重复已有的裸 100-continue，也不删除其他有效 expectation。已知长度为零时不自动添加且不等待；调用者给定的字段仍保留。HTTP/1.0 不能启用此门控，选中该版本后在提交请求头前报配置冲突。

等待从请求头提交后开始，103 等信息响应不打开门控；100 打开门控后仍由 onInformational 在调用 Context 中观察。到期后的迟到或重复 100 不再次运行 producer。最终响应若先于发送许可到达，即使 duplex 为 true 也不启动 producer，不打开文件或获取 source；该 body 的单次发送资格保持未使用。等待可取消，定时任务随调用收尾；H1 未完整定界的请求关闭连接，H2 只结束相应流。

原始字节、压缩字节、HTTP 定界和 TLS 数据各自记录实际进度。输出背压不能阻塞共享输入驱动；没有数据或容量时挂起。应用没有读取响应时，只保留协议窗口及明确的本地缓冲，不自动在后台无限收集响应。

## 5. 版本选择与连接池

`HttpVersionPolicy { Auto, Http1Only, Http2Only }` 为高层策略；Auto 默认用于 HTTPS，以 ALPN 提供 `h2`、`http/1.1`。协商 h2 使用 HTTP/2，协商 http/1.1 或允许的无 ALPN 结果使用 HTTP/1.1。未知 ALPN、Http2Only 下未选 h2、TLS 验证失败均报错，不尝试通过改协议掩盖失败。

HTTP/1 默认发 1.1，显式 `http1RequestVersion` 可选 1.0；正常接收服务器的 1.0 响应并按其定界、持久连接规则处理。Http1Only 表达协议族，不把收到合法 1.0 响应当成 TLS 协商失败。

明文默认 HTTP/1.1。`Http2PriorKnowledge` 与 `H2cUpgrade` 是显式明文选项，不能自动探测或重复发送有副作用的请求。h2c 复用 M53 的升级与 stream 1 合同；拒绝升级交付真实 H1 响应，Http2Only 则报告未满足策略并收尾，不偷偷重放。RFC 9113 已弃用 h2c Upgrade，因此它仅是显式互操作选项。

H2cUpgrade 要求 HTTP/1.1 请求版本，不能与 Http1Only 组合。每条新建明文连接只在首个实际请求上提议升级；提议字段和 SETTINGS 由 HTTP 库生成，非法 CONNECT 或已有升级字段按该库的规则拒绝。有效 101 是内部协议切换，不交付为最终响应，也不进入 onInformational；切换前后的普通信息响应仍在调用 Context 中交付。原请求完整发送、101 的空 body 消费完毕后，才把传输及其全部未消费后缀接管为 H2，并以原方法关联 stream 1，不重新发送请求或运行 producer。随后从 H2 stream 1 取得最终响应，后续请求从新的流发送。

提议期间，同 origin 的排队调用先等待协议结果。接受后按 H2 容量分配；拒绝后该连接按正常 H1 规则使用，不再次提议升级。Http2Only 下拒绝或畸形 101 以 HttpNegotiationException 失败并收尾。首个 h2c 请求必须先完成 H1 上传，因此不能使用 duplex；已升级连接上的后续请求可以使用 duplex。等待上传结束或接管传输时取消，必须终止本次调用并等待真实清理；共享 H2 驱动在 client 的 Context 中创建，stream 1 的取消归原调用 owner。

Http2Only 用于明文时必须选定允许的 H2 入口；与明文 Http1 配置冲突在网络提交前报告。TLS／明文策略和请求版本不能以无效组合静默覆盖彼此。

每跳请求均依据新 URL、代理和 TLS 配置选路。池键包括规范化 origin、代理路由、TLS 信任／客户端身份配置和协议策略；不同凭据或 TLS 配置的会话不能错误复用。不按 IP 或证书 SAN 合并 origin。

HTTP/1 每条连接同时一个 exchange；body 完整消费、输出完成且协议允许持久连接后才归还。HTTP/2 复用 stream，按 peer SETTINGS 的并发上限和本地上限分配容量；SETTINGS 降低上限时保留合法在途流，暂停新建。GOAWAY 停止在该连接上创建新流，保留能够完成的已接受流；最后处理 stream ID 作为重试判断的事实输入。

池等待可取消并可超时，公平交付可用容量。容量在登记时预留，取消／建立失败／迟到成功均归还；不能只在连接成功后计数而导致并发超限。默认每 origin 最多 8 条连接、全局最多 64 条、每条 H2 的本地并发流上限 100，并始终服从 peer 更低限制；这些为可配置的真实连接／流限制。

HttpClientConfig 的 pool 参数使用 HttpConnectionPoolOptions，提供 maxConnectionsPerOrigin、maxConnections、maxStreamsPerHttp2、idleTimeout 与 maxLifetime。前三者必须为正；idleTimeout 可为零以在空闲后立即关闭，maxLifetime 为 None 或正时长。池按等待顺序交付当前可用容量，某 origin 暂时无容量不阻挡其他 origin。连接建立期间已经占据对应连接名额；同一 origin 先等待正在建立的连接明确协议，再决定是否还需建立下一条连接。

HTTP/1 的复用资格直接查询协议库的 isReusable。H2 的请求容量保持到该调用的 producer、正文和清理均结束；取消其中一个调用只归还其流名额。达到最大存活时间的连接停止接收新调用，已有调用结束后关闭；GOAWAY 使用同样的停止分配路径。过期与空闲回收采用单调时钟和 EventLoop 定时器，不发送额外探测，也不建立另一套线程或调度器。

建立连接时首个调用的预留不被空闲或寿命回收撤销；交付后若连接已经过期，则让该调用完成并停止后续借用。对端在首次借用前关闭连接或通过 GOAWAY 拒绝新流时，向首个调用交付建立失败，不在池内无界重连；是否再次尝试由调用级策略决定。

空闲连接默认 60 秒回收，可设置最大存活时间以促使重新 DNS 查询。没有响应 body 未消费完的“空闲连接”。不把 getaddrinfo 结果伪装成含 TTL 的缓存：当前 resolver 不提供 TTL，使用调用方给定缓存时长须标明它是本地策略。HTTPS DNS records、Alt-Svc 和 HTTP/3 路由不在本期范围。

## 6. DNS、连接与代理

域名由 URL／IDNA 统一规范化后交给 asyncio resolver；DNS 与连接都可挂起。Happy Eyeballs 使用 asyncio 的地址族解析与 TCP 竞速能力，候选错误在各候选任务内部转为结果，选择成功后关闭所有未交付连接。默认连接尝试间隔 250 ms；保留底层无法立即撤销 getaddrinfo 的真实收尾时延。

TLS 始终验证 URL 的 DNS／IP 身份，不能按所连接 IP 替换主机名。默认使用 TLS 库的系统信任来源，支持显式 CA、证书链与客户端身份；mTLS 的实现归 TLS，不在 client 导出或操作 OpenSSL 对象。

支持显式 HTTP／HTTPS 代理、CONNECT 隧道、SOCKS5 和 SOCKS5 的本地／代理 DNS 两种模式。HTTPS 代理先建立到代理的 TLS；CONNECT 成功并接管全部后缀后，才建立目标 TLS。代理外层 HTTP 控制连接使用 HTTP/1.1；隧道中的目标协议可经 ALPN 选择 HTTP/2。SOCKS5 协商在 asyncio，CONNECT 与 absolute-form 报文在 HTTP 库。

Proxy-Authorization 只发往代理，不能进入隧道后的 origin 请求；Authorization 不能误用于代理。代理选择与凭据来源可通过 suspend 回调提供。环境代理通过显式启用的配置读取 HTTP_PROXY／HTTPS_PROXY／ALL_PROXY／NO_PROXY 等规则；默认不受环境变量影响。NO_PROXY 的域名后缀、IP、CIDR 和可选端口匹配由独立的代理配置函数处理，未知语法报配置错误；不实现 PAC 或系统自动发现服务。

### 6.1 显式代理与选路

HttpClientConfig.proxy 使用 HttpProxySelector，默认 direct()。HttpCallOptions.proxy 为 None 时继承，Some 时完整替换，包括使用 direct() 强制直连。HttpProxySelector 的构造参数为 suspend (HttpRequest) -> HttpProxy；fixed(proxy) 提供固定选择。每次实际尝试在取得池容量和生成 origin 认证字段前调用一次 selector，包括重定向、重试和认证 follow-up。回调位于原调用的 Context 与 Job 中，可挂起读取凭据；total、取消和 client.close 均等待其实际清理，不由连接池驱动调用用户代码。

HttpProxy 提供 Direct、Http(HttpProxyEndpoint) 和 Socks5(HttpSocks5Endpoint)。HttpProxyEndpoint 接收 URL、独立的 HttpClientTlsConfig 和可选 HttpCredentials authorization；URL 只允许 http／https，缺省端口分别为 80／443。HttpSocks5Endpoint 接收 socks5 URL、可选的 asyncio.Socks5Credentials 及 Socks5Resolution，缺省端口 1080、缺省解析模式 Proxy。两个 endpoint 均为不可变配置：拒绝 userinfo、非空且不为根的路径、query、fragment、空 authority 和零端口；网络主机直接由 UrlAuthority.networkHost() 取得，保留 URL／IDNA 的错误原因。

endpoint.authorization 表示调用者显式提供的代理凭据；selector 可先挂起取得凭据再返回 endpoint。字段由 HTTP 的 HttpCredentials 生成并标记 never-index，只用于该 endpoint 的外层请求。普通 HttpRequest.fields 不接受 Proxy-Authorization；origin 的 Authorization 不复制到自动生成的 CONNECT。SOCKS5 的用户名／密码快照、协议编码与错误均由 asyncio 处理，client 不解释或拼装 SOCKS 字节。

池以目标 origin、具体代理配置实例及转发／隧道方式区分连接；同一 selector 可返回已有不可变 endpoint 以复用连接，更换代理凭据或 TLS 配置使用新 endpoint。每 origin 的数量限制覆盖该 origin 的全部路由。路由改变且名额被其他路由的空闲连接占用时，先回收这些空闲连接再建立新路由；不复用错误路由，也不关闭仍在处理调用的连接。

### 6.2 转发与隧道

明文 HTTP/1.1 的普通请求经 HTTP／HTTPS 代理使用 absolute-form，并由 HTTP 库生成 Host；origin 的 Digest 按实际 absolute-form 签名。HTTPS、显式 HTTP/1.0、HTTP/2 prior knowledge、h2c Upgrade 和应用本身的 CONNECT 请求先建立代理隧道，再按原目标配置执行请求，不静默覆盖调用者选择的目标协议。代理外层始终使用 HTTP/1.1；HTTPS 代理只提议 http/1.1 ALPN，使用代理的 DNS／IP 身份和独立 TLS 配置。目标 TLS 使用原 URL 的身份与 client 的目标 TLS 配置。

自动 CONNECT 的 authority-form、Host、全部 2xx 成功判断以及拒绝响应定界由 HTTP 库负责。忽略合法信息响应，成功响应的空 body 关闭后通过 takeTransport 接管全部未消费后缀，再建立目标协议；不重新读取、丢弃后缀或新建 socket。没有被认证策略处理的非 2xx 结果以 HttpProxyConnectException 报告，保存代理 URL 和实际 HttpResponseHead，关闭代理连接，不无界读取拒绝 body。SOCKS5 接管同一 TCP 传输，完成协商后按相同的目标协议路径使用隧道。

代理传输的建立、取消、失败与迟到成功均遵守普通池名额和所有权规则。HTTPS 代理的 TLS 上下文在每条新代理连接建立时通过 TLS 的 suspend 工厂装载；已有 TLS 会话保留独立引用，工厂上下文随后关闭。复用现有代理连接不重新装载上下文。代理层不实现 native 工作线程、TLS 引擎、HTTP parser 或 SOCKS parser。

### 6.3 环境代理

`HttpProxySelector.fromEnvironment(variables: Map<String, String>, tls = HttpClientTlsConfig())` 在构造时解析给定配置并保存不可变路由；无 variables 的重载显式读取当前进程的相关环境变量。创建普通 HttpClient 不读取环境代理；环境后续变化不改变已经创建的 selector，同一个 endpoint 在多次选择间复用。tls 只配置 HTTPS 代理的外层 TLS。

读取 http_proxy／https_proxy／all_proxy／no_proxy 及各自的大写形式，小写存在时优先，包括空值；空代理值按没有该项处理，HTTP／HTTPS 没有专用代理时使用 ALL_PROXY。存在 REQUEST_METHOD 时忽略大写 HTTP_PROXY，小写 http_proxy 和其他代理项仍有效。未知环境变量不参与配置。当前进程重载仅对实际选中的值做严格 UTF-8 解码，未选择的大小写别名和无关变量不影响结果。

代理地址允许 http、https、socks5 和 socks5h；省略 `://` 时按 http 地址解释，IPv6 authority 使用方括号。socks5 使用本地目标解析，socks5h 使用代理解析并转换成既有 socks5 endpoint。userinfo 在原始冒号处分隔用户名和密码，再使用 URL 的百分号解码；缺省密码为空。HTTP 凭据按 UTF-8／NFC Basic 编码并显式首发，SOCKS 凭据直接使用 asyncio 的字节长度与协议规则。保存到 endpoint 的 URL 移除 userinfo，凭据不放进目标请求。非法代理配置在 factory 返回前报告，不等首次网络操作才处理。

NO_PROXY 是逗号分隔的列表，忽略项两侧空白和空项。`*` 匹配所有目标；普通域名匹配自身和子域，`.example.com` 与 `*.example.com` 只匹配子域。域名由 URL／IDNA 规范化，匹配按 HTTP 的完整标签规则，不把 example.com 当作 badexample.com 的后缀。IP 字面量精确匹配；CIDR 复用 platform.IpNetwork，只匹配 URL 中的 IP，不解析域名来决定绕过。IPv4 和 IPv6 不相互转换。

域名、IP 和 CIDR 可限制端口，按目标的有效端口精确比较；IPv6 带端口须使用括号，例如 `[::1]:443`、`[2001:db8::/32]:443`，IPv4 CIDR 可写 `192.0.2.0/24:80`。不带括号的多冒号项整体视为 IPv6 地址或网络，不猜测尾段为端口。非法端口、网络、通配位置、URL 路径／query／fragment 或其他未知语法均报配置错误。绕过判断在每次 selector 调用时针对该次 URL 执行，因此重定向和重试仍选择当前目标的路由。调用级 selector 在重定向改变方法并丢弃 body 后仍保留，不回退到 client 配置。

## 7. 超时和取消

总截止时间覆盖排队、DNS、连接、握手、所有重试／重定向／认证、上传、头部与响应 body 消费，直到调用结束。execute 返回响应头不停止总计时。计时使用单调时钟；服务器日期、Retry-After 的绝对日期和缓存日期使用 wall clock，两者不混用。

| 选项 | 覆盖范围与默认值 |
| --- | --- |
| total | 完整调用，默认 None；长流可明确保持无总截止 |
| poolAcquire | 等待池容量，默认 None，仍受 total 约束 |
| resolve／connect／tlsHandshake | 各自阶段默认 30 秒；一次候选连接序列共享 connect 截止 |
| proxyHandshake | CONNECT／SOCKS5 协商，默认 30 秒；不包含此前的代理选择、TCP 或外层 TLS |
| responseHeaders | 等待最终响应头，默认 None；启用时从该次请求头发出开始计时，包括并行上传 |
| readIdle／writeIdle | 有实际待处理读／写时连续无字节进度的时间，默认 None；应用处理数据或未发起读取不算停滞 |
| HttpCallOptions.continueWait | 默认 1 秒，到期允许发送 body，不单独令调用失败 |

每个阶段取自身上限与剩余 total 的较小者；重试、重定向和 token 刷新不重置总截止。收到无关 H2 控制帧或其他流的数据不能重置当前调用的 body 读停滞时间。

超时记录具体阶段与已发生进度，然后取消调用并等待资源安全收尾。超时不是系统操作的硬实时抢占；DNS／文件等已开始的 native work 仍须等实际完成。不得抢先恢复外部仍在借用调用方存储的操作。

H2 请求取消通过 stream close／RST_STREAM，其他请求继续；H1 取消未结束消息关闭连接。关闭整个 client 或连接错误才终止其全部 H2 流。超时与显式取消、网络错误保留区别，迟到完成遵守既有单次结果选择。

### 7.1 调用截止与头部等待

HttpClientConfig.timeouts 使用 HttpTimeoutOptions；调用层的 total／poolAcquire／responseHeaders 默认均为 None，建连阶段默认见 §7.2。HttpCallOptions.timeouts 为 None 时继承，Some 时完整替换该次配置。所有时长均为 None 或非负值，零表示进入对应阶段即到期。配置解释不增加其他尚未实现阶段的占位选项。

total 从 execute 登记本次调用开始计时，直到上传、响应消费及协议资源清理完成；交付最终头部、暂不读取 body 和来源关闭中的等待均不重置它。poolAcquire 从登记池等待开始，到已有连接的流／消息容量或新连接名额预留成功时停止，不把随后建连计入池等待。responseHeaders 从请求头提交完成开始，到最终头部到达时停止；并行上传、Continue 等待和信息响应回调占用同一截止，1xx 不延长它。

HttpTimeoutException 的 phase 使用 HttpTimeoutPhase.Total／PoolAcquire／ResponseHeaders，limit 为该阶段配置的时长，elapsed 为选择超时结果时的单调时钟经过量。requestBodyBytes、encodedResponseBodyBytes、responseBodyBytes 保存当时已由正文 I/O 返回确认的累计进度：分别为交给 HTTP 的请求表示字节、HTTP 交给解码输入的响应表示字节和正文视图读出的字节。它们不包括 HTTP 定界／TLS 字节或尚未由在途操作确认的进度，达到 Long 上界时饱和。

阶段完成与超时在同一调用状态边界选择结果；池名额选择、最终头部交付及正文 I/O 返回时也检查仍有效的单调截止，同步回调占住事件循环不会使已过期调用成功交付。多个阶段同时已到期时选择最早的截止，已有取消／操作失败优先保留。计时任务由调用登记并在资源清理时取消，调用 completion 等待其真实结束。超时不会抢先恢复正在借用缓冲区的读取／上传；响应头之前由 execute 报告，头部之后由后续正文操作或首次尚未观察失败的 close 报告。

### 7.2 DNS、TCP 与 TLS 阶段

HttpTimeoutOptions 的 resolve／connect／tlsHandshake 默认各为 Some(Duration.seconds(30))，None 关闭该阶段，零表示进入阶段即到期。它们与三个调用层选项一起继承或完整替换；复用已建立的连接不再次计时。HttpClientConfig.tcpConnect 提供竞速间隔和并发数，所属 asyncio 配置中的 resolveTimeout／connectTimeout 在 client 中统一由 HttpTimeoutOptions 的相应值覆盖。

DNS 名称直接使用 asyncio.connectTcp 的双族解析与候选竞速；client 不先串行解析、重写候选顺序或叠加一个覆盖 DNS 与 TCP 的混合时限。底层的解析截止只停止未完成的族，其他候选仍可成功。所有候选最终失败且存在 TcpResolutionTimeoutException 时，client 报 HttpTimeoutPhase.Resolve，cause 保留完整 TcpConnectException；连接截止映射为 Connect，cause 为原始 TcpConnectTimeoutException，limit／elapsed 保留底层选择时的值。

IP 字面量跳过 resolve，connect 从提交单个 TCP 连接前计时。TLS 的 tlsHandshake 从调用 AsyncTlsStream 接管连接前开始，到握手和身份验证完成时结束；阶段为 TlsHandshake，ALPN 的协议选择失败保留 HttpNegotiationException。明文请求不启动 TLS 计时。阶段结束检查仍有效的 total，握手或连接成功但截止已过时，关闭未交付资源再报告失败。

HttpTimeoutException.cause 为 None 或原始下层异常；附带清理失败时继续使用 ResourceUseException，不能丢失已选择的超时、取消或 native 原码。任何阶段到期都等实际连接、握手及取消收尾后再结束调用，未完成的连接名额随后才可复用。

HttpTimeoutOptions.proxyHandshake 默认 Some(Duration.seconds(30))，同样允许 None 和零。HttpTimeoutPhase.ProxyHandshake 覆盖自动 CONNECT 的请求提交、信息／最终响应与传输接管，或完整 SOCKS5 协商；普通代理转发的响应仍使用 responseHeaders 和 idle 配置。每次代理协商独立计时并受同一 total 约束。HTTPS 代理和目标的两个 TLS 会话分别使用 tlsHandshake 阶段。CONNECT 的目标域名由代理解析；SOCKS5 的目标 DNS 使用显式选择的本地或代理模式，client 不另行解析代理负责的域名。

### 7.3 读写停滞

HttpTimeoutOptions 的 readIdle／writeIdle 默认 None，采用相同的非负验证、继承和完整替换规则。client 将配置直接交给 HTTP 异步库：H1 的请求 writer 和每次响应读取按消息设置，H2 的每个请求流单独设置，h2c 的 stream 1 使用该次调用的配置。复用连接上的新请求重新选择本次配置，不继承上一次请求的时限。实际计时、缓冲读取、零时限、流控等待及应用停顿遵守 HTTP 规范 §7.1，client 不再维护一套 I/O 时钟。

HttpIoTimeoutException 映射为 HttpTimeoutPhase.ReadIdle／WriteIdle，保留原始 cause、limit 和 elapsed，并附带 §7.1 定义的三个已确认正文进度计数。ResourceUseException 只映射其主要原因，保留原清理异常。总截止、请求取消或其他已选择的失败不被迟到的 idle 结果覆盖。

H2 的后台 StreamFailure 按实际连接和 stream ID 关联到调用；最终响应头已经交付、producer 挂起且应用没有读取时，已排队输出的超时仍结束调用并取消 producer。流登记后查询一次 HTTP 的 failure 快照，衔接此前已经交付的通知。调用等待上传、正文适配和协议清理完成后释放名额，后续正文操作或首次尚未观察失败的 close 报告统一异常；同一原始失败不重复附加清理原因。单流失败不排空仍可用的 H2 连接，也不取消其他流。

## 8. 重定向、重试与认证

重定向默认开启，最多 10 跳，只允许 http／https；HTTPS 降到 HTTP 默认拒绝，可由调用者明确开启。301／302 默认仅将 POST 改为 GET，303 将非 HEAD 改为 GET，307／308 保持方法与 body。改为 GET 时移除原 body 相关字段；保留 body 时必须满足重放条件。Location 用 URL.resolve，处理循环和相对引用。

跨 origin 移除 Authorization 与调用者提供的 Cookie，按新 origin 重新选择凭据和 Cookie；代理凭据按代理路由重新选择。Host／:authority 由新 URL 生成。每次 follow-up 在取得下一条连接前关闭旧响应 body；不先无界 drain 旧 body。

`HttpRedirectOptions(enabled = true, maxRedirects = 10, allowHttpsToHttp = false)` 是不可变配置，maxRedirects 为非负 Long；HttpClientConfig.redirects 提供默认值，HttpCallOptions.redirects 可整组覆盖。只有 301／302／303／307／308 且含 Location 才跟随；关闭跟随或缺少 Location 时交付原响应。Location 的单值、URI 与 fragment 继承规则使用 HTTP 的 HttpLocation，client 只判断目标 scheme、origin、方法、重放和次数。循环按方法、规范 origin 和实际 request target 判断，忽略不发送的 userinfo／fragment；POST 到同 URL 的 GET 不被当作同一请求循环。

改为 GET 时使用 Empty，移除 Content-Length、Content-Type、Content-Encoding、Content-Language、Content-Location、Content-Range、Content-Disposition、Content-MD5、Digest、Content-Digest、Repr-Digest、Trailer 和 Expect，并关闭 duplex／Continue 门控。保留 body 时必须为真实可重放表示；单次 body 即使尚未启动，也不承诺能自动完成这一跳。重建请求从调用者字段快照出发，body 自动提供的编码与 trailer 声明由原工厂重新生成，不能作为手写字段再次加入并产生冲突。

`HttpResponse.redirectHistory` 是按完成顺序排列的不可变 HttpRedirect 数组。每项保存 url、method、head、nextUrl、nextMethod，只保留已关闭响应的元数据；最终 response.url 为最后请求的 URL。HTTP/1 reason 和所有原始字段保持真实值。中间响应直接按协议 body.close 合同放弃，不初始化其内容解码器，不要求无用的压缩 body 完整有效。下一跳在上一跳的上传、协议关闭及来源清理真正结束后才开始；清理失败终止跟随。

`HttpRedirectException` 的 reason 区分 InvalidLocation、UnsupportedScheme、HttpsDowngrade、LimitExceeded、Loop 与 BodyNotReplayable，并保存当前 url／head、已解析的可选 destination 和原 cause。总截止从最初 execute 开始，每跳 owner 沿用同一个起始 TimeMark，涵盖中间清理与最终响应消费；重新取得连接前检查剩余时间。超时的三类确认字节数累计同一逻辑调用已经发生的发送与应用读取，不重置为最后一跳的计数。仅启用 resolve／connect／tlsHandshake 时，下一跳的网络阶段超时也保留前面已经确认的正文进度。

默认最多 2 次重试，网络原因须同时满足请求语义允许重试及 body 可重放。失败可能已提交请求时，仅默认重试幂等方法；POST 等需要应用明确提供策略。只有协议明确证明未处理，例如合适的 REFUSED_STREAM 或 GOAWAY 边界，才可对其他方法进行一次新的尝试，仍要求已消耗 body 可重新创建。状态码重试默认关闭，可配置 408／429／503 等、Retry-After、指数退避和抖动。

`HttpRetryOptions(maxRetries = 2, connectionFailures = true, methods = ["GET", "HEAD", "PUT", "DELETE", "OPTIONS", "TRACE"], statuses = [], initialDelay = 100 ms, maxDelay = 30 s, jitter = true, respectRetryAfter = true)` 是不可变配置。方法与状态保存数组快照，方法必须是合法 HTTP token，按精确大小写匹配；状态可显式选择 200～599。maxRetries 是非负 Long，initialDelay 非负且不大于 maxDelay；0 次关闭所有自动重试。HttpClientConfig.retries 提供默认值，HttpCallOptions.retries 可整组覆盖。次数跨重定向累计，HttpResponse.retryCount 保存最终响应之前实际发起的重试次数。

连接重试只接受实际 TCP／HTTP I/O 所报告的 BrokenPipe、ConnectionReset、ConnectionRefused，非空且所有实际连接尝试均属于上述类别的 TcpConnectException，HTTP/1 等待下一响应头时的 UnexpectedEofException，以及 HTTP/2 连接结束的 IoClosedException。HTTP/2 的 stream REFUSED_STREAM 与提交前的 Http2ConnectionDrainingException 表示明确未处理。判定不解析异常文字；只有最终主错误与实际 I/O 边界记录的原始对象一致，且没有清理失败，才进入策略选择。producer、信息响应回调、解析／验证和来源清理所抛的同类型异常不因此取得网络重试资格。连接池同时使用 HTTP asyncio 的 isDraining／isClosed，不向已停止接收请求的 H2 连接分配新流。

普通连接重试要求方法在 methods 中且 body 能重新创建。明确未处理时，未启动的单次 body 可保留；已经启动仍要求可重放。未列入 methods 的方法最多享有一次明确未处理的重试，不重置全调用的 maxRetries。状态重试只有显式选中状态、方法允许且 body 可重放时才生效；条件不满足则交付原响应。重定向判断先于状态重试；重试保持当前 URL、方法、请求字段和调用 Context，每次使用新的 body 生产过程，不自动缓存单次输入。

每次重试先放弃本轮协议响应，等待上传、来源关闭及连接池容量实际归还；中间响应不初始化内容解码器。退避使用 initialDelay 的指数倍增并饱和到 maxDelay，启用 jitter 时从零到该延迟取均匀抖动；计算不能因大重试次数或有效 Duration 溢出、无界空转。单条合法 Retry-After 由 HTTP 的字段工具解释为最低等待量，不受指数退避的 maxDelay 截短；重复或非法字段忽略。等待占用可取消的结构化 Job，client.close 会取消并等候它完成，其他调用可继续。total 从首次 execute 起算，涵盖退避且在下一次尝试前再次检查；失败轮次已经确认的正文进度继续累计。

连接失效、响应中途截断、任意超时、认证失败和任意 5xx 不能统一视为可重试。一旦响应 body 已向应用交付，不把另一响应拼接到它后面；后续恢复属于显式下载续传。退避通过 EventLoop 定时等待，可取消，且受 total 与剩余次数约束。

内建 Basic、Bearer 和 RFC 7616 Digest（MD5、SHA-256、SHA-512-256 及相应 sess 形式，qop auth／auth-int）。摘要算法在 crypto，字段语法在 HTTP；认证不能私自实现密码算法。Digest 的 auth-int 在发头前取得实际发送的实体字节摘要，包括已应用的 Content-Encoding，不包括 chunked 等传输定界。使用可重放表示的独立读取或显式给定摘要，重建的编码字节必须一致；单次流若无法满足则返回明确认证／重放错误，不偷偷落盘或无限缓冲。

凭据获取与 Bearer 刷新为 suspend 回调，按实际保护空间或 origin 合并同一次并发刷新；失败交付相关调用，不取消无关 origin。限制认证往返，处理 stale nonce 与 nonce-count，避免 401／407 无限循环。OAuth 登录、账号存储和浏览器交互属于应用，不建立另一套认证框架。

### 8.1 Basic 认证

HttpClientConfig.authentication 使用不可变 HttpAuthenticationOptions，保存按优先级排列的 providers 和 maxAttempts（默认 3，必须非负）；HttpCallOptions.authentication 为 None 时继承，Some 时完整替换。空列表关闭内建认证。maxAttempts 计算整个 execute 中的挑战重发，跨重定向累计；零关闭挑战重发，显式首发凭据仍可使用。HttpResponse.authenticationCount 保存最终次数，不计作网络 retryCount。

HttpAuthenticator.basic 绑定明确的 UrlOrigin、可选的原始 realm 字节和 suspend credentials 回调；回调接收 HttpAuthenticationRequest 的原始 request、可选 challenge 与当前认证次数，返回 HttpPasswordCredentials(username, password)。只有目标 origin 和配置的 realm 都匹配时才选择，多个候选按配置顺序、响应中的挑战顺序选择。realm 按原始字节精确比较，不做 Unicode 映射或把响应头整体解码。没有匹配项的 401 正常交付；调用者已提供 Authorization 时内建认证不覆盖它。URL userinfo 不自动成为凭据。

sendWithoutChallenge 默认 false；显式开启时可向配置 origin 的请求首发 Basic。缺省编码由 HttpBasicCharset.Iso8859_1／Utf8 表达，默认为前者；合法 charset=UTF-8 挑战按大小写无关匹配切换到 UTF-8，其他 charset 值忽略。UTF-8 的用户名／密码先调用 charset.normalizeNfc；两种模式均拒绝 CTL，用户名另拒绝冒号，密码保留冒号。编码失败保留 charset 原因，Base64 使用标准库，凭据字段使用 HTTP 公共类型并标记 never-index。

401 选择下一次认证后，先按协议放弃响应，等上传、来源清理与池名额归还，再在新调用 owner 中挂起获取凭据；总截止覆盖该回调，取消或 client.close 等待回调的真实 finally 完成。已经启动的单次 body 不能重发；Continue 等待期间尚未启动的 body 可以使用其原来的单次资格。相同 Basic 挑战已经提交凭据后仍被拒绝，以 Rejected 结束；首发凭据可以再响应一次正式挑战。已取得的凭据仅在同一 execute、同 origin 和挑战 URI 的目录路径范围内复用；显式首发绑定覆盖整个配置 origin。重定向始终从原请求字段重建，再按新目标选择，不能把自动添加的 Authorization 携带到另一个 origin 或无关路径。

HttpAuthenticationException 保存 reason、url、可选 head 和原 cause；reason 区分 InvalidChallenge、InvalidCredentials、BodyNotReplayable、Rejected 与 LimitExceeded。异常消息不拼接用户名、密码或凭据。回调错误原样传播，不能因它恰好属于某个 I/O 异常类型而自动重试；认证中间响应不启动无用的内容解码器。

调用取消或超时后，凭据回调 finally 抛出的非取消异常是实际清理失败；以 ResourceUseException 保留原调用失败与该异常。client.close 取消该回调时，同样等待真实收尾并报告清理失败，不能只留下取消原因或把回调异常归入网络重试。

### 8.2 Bearer 认证

HttpAuthenticator.bearer 绑定明确的 UrlOrigin、可选原始 realm，sendWithoutChallenge 默认 true。loadTokens 是 suspend HttpAuthenticationRequest → HttpBearerTokens 回调；可选 refreshTokens 接收 HttpBearerRefreshRequest 的原 request、实际 challenge、认证次数和被拒绝的 tokens，返回可选新 tokens，默认不提供刷新。HttpBearerTokens 保存 accessToken 和可选 refreshToken；只有 accessToken 进入 Authorization，使用 HTTP 公共 token68 语法与 never-index 字段，refreshToken 只交给回调。

跨调用缓存和正在进行的加载／刷新属于同一 HttpClient 中的同一个 provider 实例；一个实例表达同一凭据源，origin 必须精确匹配。初次并发加载只执行一次，后续请求使用缓存；正在刷新时，新请求等待同一次刷新。realm 仅作挑战选择，Bearer 挑战可以没有 realm，显式配置 realm 时仍按原始字节精确匹配。已取得的 Bearer 凭据覆盖该 provider 的 origin；重定向按新 origin 重新选择，手写 Authorization 继续优先，不通过 URL query 或正文传 token。

只有匹配的 401 Bearer 挑战且 error 缺省或为 invalid_token 才自动认证／刷新；403、insufficient_scope、invalid_request 及未知 error 正常交付原响应。Bearer 挑战不接受 token68 形式。尚未发送该 provider 凭据时先加载，已经发送时刷新；挑战重发计入既有 authenticationCount／maxAttempts，并遵守同一 body 重放与旧响应收尾规则。刷新返回 None 或仍为被拒绝的 accessToken 时以 Rejected 结束，非法 accessToken 以 InvalidCredentials 结束；回调的原异常不取得网络重试资格。

迟到的 401 若对应的 accessToken 已被同一 provider 的另一请求更新，直接使用当前 token，不再执行一次刷新。合并的工作使用首个发起请求的业务 Context 和独立的客户端子任务；每个等待者恢复自己的 Context、保持自己的 total 截止。一个等待者取消不取消其他等待者仍需的工作；最后一个等待者取消时，工作也取消，并在其 finally 和子树真正结束后交付该等待者的结果。此后到来的请求先等旧工作结束，再重新选择缓存或加载，避免同时运行同一 provider 的旧清理和新加载。

HttpClient.clearAuthenticationCache(authenticator) 同步使该 provider 的跨调用缓存失效，并取消其正在进行的凭据工作；失效前的回调结果不能重新填回缓存。相关调用继续等待真实清理，之后的请求重新加载。已发送请求保留本轮的实际字段。client.close 取消全部等待调用和凭据工作，并由已有客户端作用域等待其清理；回调清理失败遵守 8.1 的 ResourceUseException 规则，不影响其他 provider 或 origin 的正常请求。

清除缓存时立即记录相关调用的取消原因，后续 client.close 不覆盖它。共享回调的一次清理失败会交付每个仍在等待清理的调用；client.close 按实际共享工作汇总一次，不因等待者数量重复附加同一错误。

回调可以用同一 client 请求令牌端点；该子请求通过 HttpCallOptions.authentication = Some(HttpAuthenticationOptions()) 显式关闭内建认证，避免等待当前正在执行的 provider。加载／刷新发生在原请求取得连接名额之前，子请求使用已有连接池、Context、截止和取消合同，不提供另一套令牌请求传输。

### 8.3 Digest 认证

HttpAuthenticator.digest 绑定明确 origin、可选原始 realm 和 suspend credentials 回调，复用 HttpAuthenticationRequest／HttpPasswordCredentials。sendWithoutChallenge 默认 true，但只在已有适用挑战的情况下首发；没有 nonce 时正常发送未认证请求。algorithms 保存配置顺序的不可变快照，默认依次为 HttpDigestAlgorithm 的 Sha512_256、Sha512_256Sess、Sha256、Sha256Sess、Md5、Md5Sess；同一 provider 的多个 Digest 挑战按该顺序选择，provider 之间仍遵守配置优先级。preferIntegrity 默认 false，双方同时支持时先选 auth，显式开启后先选 auth-int；只提供一种可用 qop 时使用该种。未知算法或 qop 不被误解为另一算法；没有可用匹配项时交付原 401。

realm 和非空 nonce 必须存在，Digest 不接受 token68 挑战。缺省 algorithm 按 MD5 解释；没有 qop 时兼容旧式 response = H(H(A1):nonce:H(A2))，不伪造 auth。charset 缺省采用 ISO-8859-1；只有 UTF-8 是可接受的显式 charset，按大小写无关匹配并对用户名／密码应用 NFC。用户名拒绝 CTL 与冒号，密码拒绝 CTL；编码错误保留 charset 原因。realm、nonce、opaque 按实际字段字节参与计算和回送。userhash=true 时发送 H(username:realm)，否则非 ASCII 用户名使用 HTTP 的 username* 扩展参数；不同时发送 username 与 username*。

每次实际尝试重新计算 method、request-target 和 Authorization，不复用上一请求的完整字段。H(A1) 使用原始用户名、realm 和密码；sess 形式再以 H(A1)、nonce、cnonce 生成会话摘要。qop=auth 的 H(A2) 来自 method:uri，auth-int 再包含 H(entity-body)。摘要和随机 cnonce 均调用 crypto，十六进制输出及 HTTP 参数引号使用所属公共工具；Authorization 标记 never-index。算法、qop 和 nc 不输出为 quoted-string，用户名、realm、nonce、uri、response、cnonce 和 opaque 使用显式引号。

客户端按同一 provider、realm、算法和 charset 保存挑战／nonce 状态，nonce-count 为同一 nonce 分配唯一递增的八位十六进制值，从 00000001 开始，耗尽时报告 LimitExceeded，不环绕。接受新的挑战时使用其中的 qop、opaque 和保护路径；nonce 不变则保留计数及 cnonce，nonce 改变才重新开始。凭据回调在每个逻辑调用首次需要签名时运行，使用该请求的 Context、total 与取消边界；密码及其派生凭据不进入跨调用缓存。同一调用的 stale 重发可以复用已取得的凭据，算法／charset／realm 改变时重新取得。取消只结束所属调用，client.close 沿已有调用作用域等待所有凭据和预哈希清理。

Digest 保护空间缺省覆盖整个配置 origin；domain 参数中的 URI 前缀通过 URL 库解析，并与显式配置的 origin 求交，不按服务器提供的跨 origin URI 外送凭据。首次挑战、缓存首发被拒绝及带不同 nonce 的 stale=true 都计入既有 maxAttempts；正式响应挑战后的同 nonce 拒绝、没有 stale 的再次拒绝以 Rejected 结束。迟到的拒绝若对应旧 nonce，而缓存已有更新状态，先使用当前状态，不倒退覆盖它。所有重发继续遵守原 body 重放、来源关闭、总截止和重试分类规则。

RequestBody.withDigest(algorithm: HttpDigestAlgorithm, bytes: List<Byte>) 返回共享原生产过程的不可变表示，保存给定摘要的字节快照；MD5 为 16 字节，SHA-256／SHA-512-256 为 32 字节，sess 与对应普通算法使用同一实体摘要。长度错误在构造时拒绝，再次指定同一基础算法时替换旧值。摘要描述应用 Content-Encoding 后、HTTP 传输定界前的实体；compressed 产生新的表示时不继承原文摘要，multipart 的 part 摘要也不作为整个 multipart 的摘要。

auth-int 优先使用该表示明确给定的摘要；已知零长度表示可直接使用空实体摘要，其余情况仅对可重放 body 通过原 writeWithTrailersTo 流式预哈希。预哈希保持有限缓冲、原 Context 和真实来源清理，不计为已发送网络字节；随后的发送遵守原来的可重放表示契约。没有可用摘要的单次非空 body 报 BodyNotReplayable，不偷偷消费一次性来源、无限缓冲或落盘。显式摘要不赋予已消费的单次 body 重发资格。

收到 Authentication-Info 时使用 HTTP 公共 parser，检查实际请求的 qop、cnonce、八位 nc 和 rspauth。存在 qop 的确认必须携带对应 rspauth／cnonce／nc；非法参数、错配或摘要不符以 HttpAuthenticationFailure.InvalidResponse 结束，保留 url、已知响应头和原 cause。auth 的 rspauth 使用空 method 的 A2；auth-int 的响应实体摘要在 HTTP 定界之后、Content-Encoding 解码之前增量计算。头部和 trailer 共用同一次确认，读取至真实 EOF 并验证成功后才完成该校验。

有效 nextnonce 只更新其所属的当前 nonce 状态，并保留期间由新挑战更新的其他参数；迟到旧响应不能覆盖更晚的 nonce。未完整读取而主动关闭、重定向或重试放弃的 auth-int 响应不宣称通过正文校验，也不应用尚未验证的 nextnonce；不为认证无界 drain 中间 body。HttpClient.clearAuthenticationCache 对 Digest 清除已有挑战／nonce，已在进行的调用保留本轮选择，旧响应不能重新填回或改写清除后新建的缓存；后续请求重新取得挑战。没有 Authentication-Info 的响应仍按普通 HTTP 内容语义交付。

### 8.4 HTTP 代理认证

HttpProxyEndpoint.authentication 使用独立的 HttpAuthenticationOptions，默认空列表；其 providers 必须匹配代理 URL 的 httpOrigin，不能借用目标站点的认证配置。显式 authorization 优先，不被自动认证覆盖。Basic、Bearer 和 Digest 共用 §8.1～§8.3 的字段、字符集、凭据回调与算法实现；挑战来自 Proxy-Authenticate，凭据仅进入 Proxy-Authorization，确认来自 Proxy-Authentication-Info。SOCKS5 认证继续使用 asyncio 的协议和凭据类型。

HttpAuthenticationRequest 与 HttpBearerRefreshRequest 的 proxyUrl 在目标站点认证时为 None，在代理认证时为 Some(endpoint.url)；request 始终保存当前应用请求。attempt 为该认证作用域在整个 execute 中已经发生的挑战重发次数。HttpResponse.proxyAuthenticationCount 独立记录 407 重发，跨重定向、网络重试与代理路由变化累计，不增加 authenticationCount 或 retryCount。代理配置的 maxAttempts 限制这项累计值；零关闭挑战重发，显式首发凭据仍可使用。HttpAuthenticationException.url 指向实际认证对象：目标请求 URL 或代理 URL。

每次尝试先选择路由，再准备该路由和目标站点的凭据，随后申请连接池容量；准备可挂起，保持原请求 Context、取消和 total 截止。forward 407 沿普通响应 follow-up 处理。CONNECT 407 先放弃响应、关闭该代理连接并归还连接名额，下一次尝试才执行凭据获取或刷新；回调通过同一个 client、同一容量上限发起令牌请求时仍能推进。没有匹配项的 forward 407 正常交付；CONNECT 仍以 HttpProxyConnectException 保存代理 URL 和原始拒绝头部。回调异常、取消清理及已消费单次 body 的处理继续遵守 §8.1～§8.3，不能作为普通网络重试。

认证状态按目标站点或具体 HttpProxyEndpoint 实例区分。即使两者 URL 相同、使用同一个 provider 对象，也不能共享 Bearer token、刷新任务或 Digest nonce；两个 endpoint 实例亦相互独立。同一 endpoint 上的并发请求继续按 provider 合并 Bearer 加载／刷新，迟到的 407 使用已更新 token；Basic 的代理保护空间覆盖该代理，不按目标 URL 目录划分。更换路由只选取新路由适用的状态，代理凭据不能进入 direct 请求、其他代理或 CONNECT 隧道内的目标请求。clearAuthenticationCache(provider) 清除该 provider 在所有实际作用域中的缓存，并取消各自在途的 Bearer 工作；已在进行的 Digest 使用不能重新填回已清除的状态。

代理 Digest 的 method 与 uri 来自实际代理请求：forward 使用应用方法和 absolute-form；自动 CONNECT 使用 CONNECT 和含端口的 authority-form。CONNECT 的请求与成功响应均按空实体计算 auth-int，不能启动、预哈希或消费目标请求 body。代理挑战中的 domain 按 RFC 7616 忽略，不套用目标站点的 URI 路径保护规则。算法协商、stale、nonce-count、userhash、字符集与 nextnonce 仍使用同一实现，并保留上述作用域隔离。

CONNECT 成功头部中的 Proxy-Authentication-Info 在接管隧道之前校验，错误关闭代理传输并报告 InvalidResponse；合法 1xx 和 2xx 仍遵守 §6.2 的 CONNECT 语义。forward 响应可以同时确认目标和代理的 Digest，两种 auth-int 都对同一份 HTTP 定界后、Content-Encoding 解码前的表示字节增量计算，并分别验证各自的头部／trailer 和 nextnonce。只校验实际提交过相应凭据的响应；没有确认字段时正常交付。主动关闭或放弃未读完的实体不能宣称校验成功，也不能应用尚未验证的 nextnonce。

## 9. 内容、Cookie 和缓存

自动内容解码默认开启，Advertise gzip、deflate、br、zstd，前提是本构建实际支持对应 decoder。Content-Encoding 多层编码按逆序解开；不支持的编码报明确错误或由显式原始 body 模式交付，不能悄悄少解一层。调用者显式提供 Accept-Encoding 时保留原值，实际解码策略仍独立配置。

`response.fields` 始终保存收到的字段；`wireContentLength` 表示编码后的长度，解码后 body 的 contentLength 只有确实已知时才有值。原始 body 模式交付完成 HTTP 定界后的编码字节，不包含 chunked／DATA 帧。Range 下载默认请求 identity；要对编码表示做 Range 时必须显式选择原始 body 和一致的表示条件。

上传压缩显式启用，复用 scoop.compression 的异步流，更新 Content-Encoding 与长度／定界信息；不能保留原文 Content-Length。multipart 边界、字段参数、filename*、HTTP 日期和媒体类型在 HTTP 库，二进制字节与字符集在各自 codec。下载目的 Path 必须由调用者指定；可提供解析后的建议文件名，不据此自行选择写入位置。

Cookie 默认关闭，提供显式 MemoryCookieJar 和可由调用者实现的 suspend CookieJar 接口。支持 host-only／Domain／Path、Secure、HttpOnly 元数据、Expires／Max-Age、替换与过期、cookie 前缀以及公共后缀限制。PSL 使用固定版本的 ICANN 与 PRIVATE 部分，数据和匹配属于 HTTP 域名工具；IDNA 不承担公共后缀政策。非浏览器客户端没有隐式页面上下文：启用 SameSite 发送判断时必须显式给出 site／顶层导航信息，未给信息时采用限制性规则，不虚构当前站点。浏览器脚本可见性和分区 Cookie 不在本期。

提供 RFC 9111 私有响应缓存，默认关闭，内置内存和磁盘实现，另有 suspend HttpCache 接口。处理 Cache-Control、Age／Date、Expires、Vary、ETag／Last-Modified 与 304 合并；no-store、private 和带 Authorization 请求按私有缓存语义处理，Vary: * 不复用。默认只自动存储完整 GET 响应，不启发式缓存 POST、不组合不完整 range 条目。

内置缓存归属单个 client 的私有上下文；显式共享存储时须区分缓存分区和认证上下文。与某个 Authorization／客户端身份关联的条目不能只凭相同 URL 命中另一个身份的请求，凭据本身不作为磁盘文件名或日志数据。

缓存保存未自动解压的表示字节与对应字段，键保留 URL query 的实际顺序和 Vary 所需请求字段。成功读完 body 后才提交条目；中途取消／解码错误不发布完整条目。磁盘写入用 asyncio 临时文件、关闭／按配置 sync 后同文件系统替换；原始网络与缓存 I/O 都可挂起，不能调用同步 Path API。HTTP 缓存的响应验证与 M48 编译缓存是不同功能，不能据此恢复编译产物复验。

下载便利入口接受显式 Path、覆盖策略和 suspend 进度回调；默认写入同目录临时文件，成功完成并关闭后再替换目标，取消保留原目标。显式续传模式保留部分文件与已确认的 URL、表示编码、长度和强 ETag，下一次使用 Range／If-Range；206 必须从期望偏移开始且验证相同实体，200 则按策略从头重建，416 只有在长度和验证条件证明已完整时才成功。实体变更、无可靠验证条件或范围不符时不得把新旧内容拼接。seek／truncate／metadata／替换全部使用 asyncio，缓存与续传共享格式工具，不共享不完整条目。

### 9.1 响应解码配置与生命周期

HttpClientConfig 的 decoding 使用 HttpDecodingOptions；mode 为 HttpContentDecoding.Automatic（默认）或 Raw，bufferSize 默认 8192 且必须为正，maxWindowBytes 为 None 或正数，直接限制每层所属 compression decoder 的窗口。HttpCallOptions.decoding 为 None 时继承 client 配置，Some 时完整替换该次配置。

没有手写 Accept-Encoding 时，Automatic 添加 gzip、deflate、br、zstd；Raw 不自动添加编码提议。带 Range 的请求在缺省时添加 identity，显式 Accept-Encoding 始终保留。解码模式独立于调用者的提议：Raw 可以接收显式请求的压缩字节，Automatic 也会解释服务器实际返回的编码。

Content-Encoding 的多行列表、大小写和空项由 HTTP 工具解释，按逆序建立 decoder；identity 不增加一层，x-gzip 按 gzip 兼容，deflate 严格指 zlib 包装，不猜测 raw DEFLATE。Automatic 遇到不支持的编码抛 HttpUnsupportedContentCodingException，非法列表保留 HttpFieldParseException；这些配置解释发生在最终响应构造时。Raw 不解释编码字段。HEAD、1xx、204、205、304 和成功 CONNECT 的内容语义从 HTTP 库查询，不因表示元数据而启动解压。

每个 decoder 在首次非零读取时创建，只借用下一层输入。响应的 body 始终是本次选择的一条视图，不提供可同时读取的 raw／decoded 副本。解码后的 contentLength 为 None；没有解码且允许内容时使用 wireContentLength，元数据或协议切换响应不从该字段推导 body 长度。fields 和 wireContentLength 始终保留实际接收值。

压缩流必须读到验证结束，才将该响应标记为完整并公开 trailer；底层已经收到全部编码字节不提前归还本次调用容量。格式／截断／窗口错误保留 compression 的异常，并按调用失败收尾；H2 只取消相应流。放弃 body、client.close 和 caller 取消均关闭已创建的 decoder，等待真实读取与借用结束，再完成协议资源清理。

### 9.2 有界读取、文本与 JSON

HttpResponseBody 提供 readAllBytes(maxBytes)／bytes(maxBytes)、text(maxBytes, charset = None, options = CharsetDecodeOptions(), malformedInput = CharsetErrorAction.Strict)、json(codec, maxBytes) 与 copyTo(destination, bufferSize = 8192)。前三类便利读取的上限计入交付视图的字节，因此自动解压后按解码字节计数；超限沿用 asyncio 的 ReadLimitException 和至多一个额外字节的检查。copyTo 使用有限缓冲且不接管或关闭目的端。

每次便利读取在整个 suspend 过程独占 body，同一时间的其他读取报忙；普通 readInto 仍按单个操作独占。它们复用同一消费位置，顺序调用不会重放已读内容。当前 reader Job 的取消登记覆盖整个便利操作，保留该调用的真实 I/O 收尾。

text 优先使用显式 Charset，否则通过 HttpMediaType.parseFields 读取单一 Content-Type 的 charset；没有 charset 时使用 UTF-8。未知字符集和坏媒体类型在消费正文前报告，调用者可以显式选定 Charset；不猜测 HTML／XML 内容。BOM、字节序及严格／替代处理直接使用 charset 的选项，不在 client 编码转换。

json 使用严格 UTF-8、按 charset 默认规则消费匹配 BOM，再调用 Json.decodeFromString 与显式 ValueDecoder；不按 Content-Type 的 charset 将 JSON 解释为其他编码。完整 JSON 与 typed codec 错误保留其原类型。文本和 JSON 的业务解析错误不改写响应字段，也不把已经完整消费的 HTTP 消息变成连接错误。

### 9.3 Cookie jar

HttpClientConfig.cookieJar 为 None 时关闭自动 Cookie，Some 时借用调用者提供的 CookieJar；client.close 不关闭或清空外部 jar。CookieJar 的 `suspend load(request: HttpRequest): HttpCookieHeader` 和 `suspend save(request: HttpRequest, response: HttpResponseHead)` 可用于内存或应用自定义存储。HttpCallOptions.useCookies 默认 true，false 同时跳过本次调用的加载与保存；手写 Cookie 字段优先，跳过加载但仍保存响应。调用选项在认证、重试和改变方法的重定向后保留。

每次实际尝试在凭据准备完成后、连接池排队前加载，输出字段使用 HTTP 的 never-index Cookie 构造器。最终响应头到达后、重定向／认证／状态重试判断前保存，因此下一次尝试可使用中间响应的新 Cookie。信息响应、自动 CONNECT 的响应和转发代理的 407 不作为目标站点 Cookie 来源。用户回调在本次调用的 Context 中执行，不在锁或 H2 共享驱动中挂起；受 total、取消与 client.close 约束，等真实清理结束后才完成。存储异常原样传播，不取得网络重试资格。

MemoryCookieJar 是线程安全的纯 Scoop 实现，constructor 接收 `enforceSameSite = false`、`suffixes = PublicSuffixList.bundled()`、`clock = SystemClock`、`maxCookies = 3000`、`maxCookiesPerDomain = 180` 和 `maxLifetimeSeconds = 34560000`；两个数量上限必须为正，期限上限非负。读取时淘汰过期项，按路径长度降序、同长度按创建先后输出；成功选中的项更新最后访问时刻。存储键是原始名称字节、规范 host、hostOnly 和原始已编码路径，替换保留原创建次序。超过规范 host 的数量上限或总上限时，先移除过期项，再按对应范围内最早访问、创建先后的顺序淘汰。短锁只保护内存状态；字段解析、PSL 与时钟回调在锁外执行。

逐条 Set-Cookie 使用 HTTP parser 和 resolve 接纳；格式错误与正常策略拒绝忽略该条，原始响应字段仍可观察，后端和存储错误照常传播。过期的新值删除相同键；普通会话 Cookie 保留到显式清理或 jar 生命周期结束。明文来源的非 Secure Cookie 不能覆盖或遮蔽名称相同、域名作用域重叠且新路径匹配旧路径的既有 Secure Cookie，过期删除同样服从此规则。HttpOnly 是保存的元数据，本接口代表 HTTP 请求，不提供脚本读取接口。`snapshot(): Array<HttpCookie>` 返回未过期项的不可变创建顺序快照，`remove(cookie): Boolean` 按完整键删除，`clear()` 清空，`clearSession()` 删除没有 expiresAt 的项。

`HttpCookieContext(site: Url? = None, isTopLevelNavigation = false)` 由调用者显式提供，通过 HttpCallOptions.cookieContext 传入。MemoryCookieJar 仅在 enforceSameSite=true 时使用它：站点由 HTTP 的 schemeful-site 工具比较有效 scheme 与可注册域，含 PRIVATE PSL；IP 或无可注册域的 DNS 名称按完整 host 匹配，端口不参与。缺少上下文或 site 时按跨站处理，未给导航信息时不假定顶层导航。安全方法仅为精确大写的 GET／HEAD／OPTIONS／TRACE；Strict、Lax／Default、None 的发送与设置复用 HTTP 现有规则。上下文固定来自调用者，每跳针对当前 URL 判断，不从 socket、重定向来源或 TLS SNI 猜造页面站点；不实现浏览器的脚本、worker、分区和 Lax-Allowing-Unsafe 例外。

### 9.4 私有响应缓存

HttpClientConfig.cache 默认为 None；Some(HttpCacheOptions(storage, partition = None, clock = SystemClock)) 借用指定存储，不在 client.close 清空或关闭它。未指定 partition 时，每个 client 生成自己的随机分区；显式使用相同 partition 才能跨 client 或进程重启复用。HttpCallOptions.cacheMode 默认为 Default；Bypass 完全跳过本次缓存，Reload 不直接复用新鲜条目但允许条件验证，OnlyIfCached 只接受按请求指令可用的缓存，未命中返回空的 504。请求的 only-if-cached 指令具有同样含义。HttpResponse.cacheStatus 区分 Network、Hit、Revalidated、Unsatisfied；合成 504 使用 HTTP/1.1，没有网络尝试。

自动缓存仅针对无正文、无 Range、无调用者条件字段的 GET。手写 If-Match、If-None-Match、If-Modified-Since、If-Unmodified-Since 或 If-Range 由服务器解释；client 不替调用者合并条件响应。请求 no-store 同时跳过读取和写入，no-cache 和未给 Cache-Control 时的 Pragma: no-cache 要求验证。非法或互相矛盾的缓存控制按不能直接复用处理，不使原本有效的网络响应失败。带 Authorization 的响应可进入私有缓存，private 不排除存储；共享缓存指令 s-maxage／proxy-revalidate 不改变本地私有缓存的新鲜度。Vary: *、no-store、206、304 和 Content-Range 不作为完整表示存入。

键由分区、规范 origin 加实际已编码 path／query，以及身份摘要组成；不包含 fragment／userinfo，不重排 query。身份摘要区分实际 Cookie、Authorization 和本地 TLS 叶证书。内建 Digest 使用稳定的基础 H(A1) 及算法／保护空间，避免 nonce／nc 的正常变化使同一身份无法命中；手写认证字段按实际字节区分。请求凭据与请求 Cookie 原文不写入缓存元数据；响应字段仍保留原始值，Vary 保存字段名和有序、带长度分隔的字段值 SHA-256，包括缺失与空字段的区别。请求字段包含自动生成的 Accept-Encoding、Content-Length 与有效 Host；代理凭据不作为源站身份。

新鲜度使用 RFC 9111 的 apparent age、Age、响应延迟和驻留时间，响应优先 max-age，其次 Expires；没有显式期限且状态允许启发式缓存时，采用 Date 与 Last-Modified 间隔的十分之一，至多一天。缺少 Date 时用收到响应的时刻补充；无效／重复日期、无效年龄及重复／无效期限不能产生新鲜条目。多条 Age 或列表使用 HTTP 库按 RFC 9111 选取的首值。请求 max-age／min-fresh／max-stale 缩小或明确放宽可接受范围，no-cache／must-revalidate 不允许借 max-stale 绕过验证。命中响应更新 Age，保留存储的实际协议版本、状态、表示字段与 trailer。

过期条目优先使用 ETag／If-None-Match，否则使用 Last-Modified／If-Modified-Since。304 先按真实网络响应完成认证确认，再将其端到端字段合入旧头部；缺少 Date 时使用本次接收时刻，Age 重新计算，不将旧 Set-Cookie 再交给 jar。验证响应的长度、编码或已有 ETag 与原表示不相容时，放弃合并并进行一次不使用缓存条件的正常请求，不把不同表示拼接。新的 no-store 或 Vary: * 使旧条目失效。只有最终交付的完整表示才提交更新；中途关闭可以保留旧的、仍需验证的完整条目。成功的非安全方法使当前分区的目标 URL 及同 origin 的 Location／Content-Location 失效；成功 HEAD 保守地使相应 GET 失效。

HttpCache 的 suspend lookup(HttpCacheRequest)、begin(HttpCacheRecord) 和 invalidate(partition, url) 分别返回可选 HttpCacheSnapshot、可选 HttpCacheWriter 和 Unit。request 保存 key 与字段快照；record 保存 key、头部、发送／接收时刻及 Vary 摘要，fromResponse 工厂生成字段摘要和可存储头部。lookup 返回匹配的完整表示或 None；snapshot 拥有独立 body、bodyLength 和 trailer，关闭后释放读取资源。writer 是 AsyncByteSink／AsyncCloseable，commit(trailers) 在完整写入后原子发布，close 只清理，不隐式提交；已知 Content-Length 必须等于实际表示长度。缓存容量不足时 begin 可返回 None，或 writer 消费输入而放弃存储。读取、写入、commit 与 close 均可挂起，在调用的 Context、total 和取消边界内执行；实际存储错误向当前调用传播，不取得网络重试资格。

MemoryHttpCache 的 maxBytes 默认 32 MiB、maxEntryBytes 默认 8 MiB、maxEntries 默认 1024；容量均必须为正，按正文大小计数，按最近访问顺序淘汰。DiskHttpCache.open(directory, maxBytes = 256 MiB, maxEntryBytes = 64 MiB, maxEntries = 1024, maxMetadataBytes = 1 MiB, syncOnCommit = false) 使用专用目录，按包含元数据的文件大小计总容量；maxEntryBytes 约束正文，maxMetadataBytes 约束单条元数据，超限放弃存储。两者提供 entryCount／sizeBytes，以及清空全部条目的 clear；invalidate 和 clear 同时阻止此前未提交的相应 writer 发布。快照在条目替换、淘汰或清空后仍可读完。容量统计索引中的完整条目，在途临时写入与已借出的旧快照按各自生命周期释放。单个实例支持并发任务和多个 client，共享磁盘目录的多个同时活动实例不属于该工厂的协调合同。

磁盘条目是正文、带长度元数据和固定尾部组成的单文件；open 读取边界明确的元数据重建索引并清理本实例目录的遗留临时文件，格式错误报告 HttpCacheFormatException。写入同目录临时文件，完整结束后按选项 sync、关闭并改名为独立条目，再发布内存索引。旧文件在最后一个快照关闭后删除；取消和失败通过 core.nonCancellableScope 等待实际异步文件关闭、删除，不开启脱离调用的后台任务。提交后的旧文件清理失败仍报告真实错误，但已发布的新条目始终是完整表示。磁盘缓存不进行文件来源审计，也不与编译缓存的更新判断共享验证政策。

## 10. SSE 与 WebSocket

SSE 通过普通 execute 和 HTTP/1.1／HTTP/2 响应流工作，要求适用的成功状态与 text/event-stream，使用 HTTP 库的 UTF-8 增量行／事件 parser。支持 data 多行、event、id、retry、注释与 CR／LF 分段；异步 `next(): SseEvent?` 按消费者读取推进。可选重连使用 Last-Event-ID、服务端 retry 与 client 策略，204 停止；取消或显式关闭不得触发自动重连。

WebSocket 仅实现 RFC 6455 的 HTTP/1.1 Upgrade，`wss` 使用仅提供 http/1.1 的独立 TLS 连接。即使同 origin 已有 H2 池连接，也不尝试 RFC 8441。握手验证、SHA-1、mask 随机值、帧编解码、分片、UTF-8、Ping／Pong、Close 和消息上限分别使用 HTTP／crypto 的公共能力。

`withWebSocket(request, block)`／显式 open 返回 AsyncCloseable session，支持 suspend send、receive、ping 和 close handshake；应用读写可并行，同方向串行。有限缓冲与协议处理允许控制帧推进，消息发送不会把任意大 payload 先收集到内存。permessage-deflate 为显式扩展，协商与帧状态在 HTTP，raw DEFLATE 与 context takeover 在 compression。

Upgrade 成功后把未消费后缀与传输所有权交给 WebSocket session，连接不回到 HTTP 池。HTTP 握手失败仍交付可检查的 HTTP 响应并由调用方关闭；升级成功后的重连由应用明确发起，不能重放应用消息。

## 11. 扩展、错误与可观察性

提供 suspend 的应用级 interceptor、凭据／代理选择回调与调用事件观察器。应用 interceptor 每个逻辑调用执行一次；需要每次网络尝试签名或观察时使用明确的 attempt 回调，能够看到重定向、重试后的最终方法、URL 和字段。chain 的单次 proceed 与 follow-up 次数由正常调用状态检查，不提供任意替换协议引擎的插件体系。

用户回调在短锁和 native callback 外执行。H2 的共享驱动不能挂起等待某个调用的 observer；事件以有限的调用状态／队列交付，积压时阻止该调用继续相应步骤，不无限分配或停住整个连接。回调抛出影响当前调用，正常向调用者传播；不能吞掉用户异常再自动重试。

记录队列、DNS、连接、TLS、头部、首字节和结束时刻，以及各方向实际字节数、选择的协议、连接是否复用和尝试原因。默认日志不输出 Authorization、Proxy-Authorization、Cookie 或 body；URL 使用明确的脱敏展示。连接信息不在本期增加证书链检查接口。

`HttpClientException` 用于高层策略失败，保存 stage、attempt、Url 的适当展示、已知进度及原因；下层 HttpException、TlsException、PlatformException 保留可区分的类型与原码。`HttpTimeoutException` 标明阶段；`CancellationException` 不被包装成网络故障。重定向上限、不可重放、认证循环、未知内容编码与池关闭分别有可识别原因，不靠解析 message 分支。

## 12. 默认调用形态

以下示例只使用公开接口，所有 I/O 留在已有 EventLoop 内：

```scoop
fun main() {
    LibuvEventLoop().run suspend { _ ->
        withHttpClient(HttpClientConfig()) suspend { client ->
            val request = HttpRequest.get(Url.parse("https://example.com/data"))
            client.withResponse(request) suspend { response ->
                response.requireSuccess()
                val bytes = response.body.readAllBytes(maxBytes = 1048576L)
                print(bytes.size)
            }
        }
    }
}
```

提供 open／execute 是为了显式跨函数管理资源，作用域形式是推荐用法。示例省略 import，库必须同时交付可独立编译的完整用例。同步根入口只属于应用启动；client 不在每次 get/post 内自行 run。

## 13. 参考

- [Ktor 响应与流式资源](https://ktor.io/docs/client-responses.html)、[Ktor 客户端引擎](https://ktor.io/docs/client-engines.html)。
- [OkHttp calls](https://github.com/square/okhttp/blob/master/docs/features/calls.md)、[connections](https://github.com/square/okhttp/blob/master/docs/features/connections.md)。
- [Go net/http](https://pkg.go.dev/net/http)、[reqwest](https://docs.rs/reqwest/latest/reqwest/)、[HTTPX async](https://www.python-httpx.org/async/)。
- [.NET HttpClient 使用指南](https://learn.microsoft.com/en-us/dotnet/fundamentals/networking/http/httpclient-guidelines)。
- [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html)、[RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html)、[RFC 9113](https://www.rfc-editor.org/rfc/rfc9113.html)、[RFC 8305](https://www.rfc-editor.org/rfc/rfc8305.html)。
- [RFC 7616](https://www.rfc-editor.org/rfc/rfc7616.html)、[RFC 6455](https://www.rfc-editor.org/rfc/rfc6455.html)、[RFC 7692](https://www.rfc-editor.org/rfc/rfc7692.html)、[SSE](https://html.spec.whatwg.org/multipage/server-sent-events.html)。
