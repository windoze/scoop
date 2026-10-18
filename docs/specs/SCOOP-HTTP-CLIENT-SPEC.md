# Scoop HTTP client 库规范

版本：M54 设计，尚未实施。范围、底层缺口和实施次序见 [M54 设计](../milestone54/DESIGN.md)。本规范基于 [HTTP 协议库](SCOOP-HTTP-SPEC.md)、[异步 I/O](SCOOP-ASYNCIO-SPEC.md)、[TLS](SCOOP-TLS-SPEC.md)、[URL](SCOOP-URL-SPEC.md)、[IDNA](SCOOP-IDNA-SPEC.md)、[压缩](SCOOP-COMPRESSION-SPEC.md)与[字符集](SCOOP-CHARSET-SPEC.md)的公共合同。

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

client 的所有者在一个正文仍活动的子 coroutineScope 中维持连接任务与调用任务，直至 close；不能在 scope 正文返回后再尝试登记新连接。调用 owner 持续到请求发送与响应消费均完成或放弃，不在 execute 返回头部时结束。调用者 Job 的取消登记维持到本次调用收尾；execute 不创建一个必须等 body 结束才能返回的临时 coroutineScope。显式响应须在 client 的结构化生命周期内使用并关闭，不把引用带出 withHttpClient；漏关资源由 client.close 收尾。

连接和请求分别持有自己的 Job。HTTP/2 共享 TLS 的读写归连接 owner；请求 body、头部等待和超时归调用 owner，单请求取消只关闭相应流。HTTP/1 未完成消息取消使连接不可复用并关闭。一次网络失败作为调用结果交付；内部候选连接／上传等子任务必须在任务内部捕获预期操作失败，不在 await 处才捕获已经传播到父 scope 的异常。core 的正常失败传播规则保持不变。

调用任务在发起者的 Context 中创建，producer、认证和调用级回调保留该调用的动态 binding；共享连接驱动使用连接自己的 Context，不把第一条请求的业务 Context 用于所有后续流。切换 Job 不得丢失其他有效 binding。

close 拒绝新请求与新借用，唤醒池等待者，取消活动调用，关闭连接，并等待实际 I/O、native 借用和子任务结束。close 可在已取消 Job 的 finally 中完成，并发调用等待同一轮收尾，之后幂等；清理失败使用既有 ResourceUseException。正常响应消费完成会归还连接／流容量；显式关闭不执行无界 drain。

## 3. 请求、响应与 body

`HttpRequest` 是不可变配置快照，含 method、Url、有序重复 HttpField、RequestBody 和每次调用的选项。method 为合法 token，提供常用方法常量，同时支持扩展方法。URL、字段和 body 类型沿用所属库；client 不定义另一套大小写比较或字节字段模型。

`HttpResponse` 保存实际协议版本、最终 status、原始字段快照、最终 URL、重定向历史、body 与调用信息。HTTP/1 reason phrase 保留实际字节，HTTP/2 不补默认文本。非 2xx 默认是正常响应；`requireSuccess()` 显式抛包含状态与头部快照的 HttpStatusException，不暗中读完整错误 body。

信息响应与最终响应分开观察；100 负责 Continue 门控，103 等通过调用级事件交付。trailer 只在 body 完成后可用，区分未完成与完成且为空。仅收到头部或一部分 body 不能提交完整缓存条目、宣称下载完成或计算完整吞吐量。

RequestBody 至少提供 Empty、字节快照、文本、JSON、表单、multipart、文件、单次 AsyncByteSource、可重新打开的 source factory 和 suspend producer。source 或 producer 在真正开始发送该次 body 时才启动；工厂和 producer 可挂起，使用调用的 Job。每次返回的 body 资源由该次发送负责关闭；接受 body 参数不立即消耗其数据。

可重放 body 用能够每次创建新实例的 factory 表达，另保存可选 contentLength 和 trailer producer；不能只有一个 `replayable = true` 标志。字节／文本快照天然可重放，单次 source／producer 默认不可重放，可重放 producer 必须显式声明创建新的生产过程。文件 body 使用 asyncio 打开与读取，调用者须保持重试期间内容一致，重新打开不等于创建文件快照。

body 的已知长度必须与实际字节数一致，不能提前 EOF 或超长后继续复用连接。HTTP/1.1 未知长度采用 chunked，HTTP/2 采用 DATA／END_STREAM。HTTP/1.0 请求不使用 chunked；未知长度请求体需调用者给出长度或明确有限缓冲，不能悄悄读取整个上传文件。trailer 按各协议规则由 HTTP 库编码，HTTP/1.0 不接受 trailer。

响应 body 实现 AsyncByteSource／AsyncCloseable。同一 body 只能有一个消费者；bytes、text、JSON、copyTo 和其他解码视图共享这一次消费，不能同时读取 raw 与 decoded 两条伪独立流。便利入口明确传入 maxBytes，或使用调用方设置的有限缓冲上限；流式 copyTo 不要求把完整消息放入内存。取消及错误保留已交付的实际进度，不回滚文件或用户 sink。

JSON 便利函数通过现有 Json 与显式 ValueEncoder／ValueDecoder 工作，完整 JSON 校验遵守语言规范 11.13。typed JSON body 是明确的有限缓冲转换，不声称业务 codec 可挂起或能逐元素处理无限 JSON；大 body 使用流接口，格式能力若需扩展必须在 scoop.json 补齐。非 UTF-8 文本使用 scoop.charset，不在 client 内实现编码器。

## 4. 上传、早期响应与背压

收响应与写请求可以双向推进。发送大 body 时仍必须能够处理早期 401／413、信息响应和 HTTP/2 WINDOW_UPDATE，不能先等待上传结束再启动响应读取。HTTP/2 的显式 duplex producer 可以在读取响应期间继续上传；每方向仍至多一个在途操作。HTTP/1 不增加 pipelining，早期最终响应后未能完整定界的请求使连接关闭。

最终响应头可用后无需等待 producer 结束才交付。需要停止上传时，协调器停止提交新数据，并按协议结束或在响应消费／放弃后关闭该 exchange；不能为了停止上传而先取消共享 TLS，或使尚需交付的早期响应 body 丢失。认证／重定向需要放弃旧响应时，可以按明确的 body.close 合同终止旧请求并开始下一跳。

`Expect: 100-continue` 可由调用者选择，另有独立等待间隔；100 或间隔到期才开始 body，先到最终响应则不启动 producer。间隔到期不是整个请求超时，也不能重发已接受的 body。HTTP/2 上仍处理该 HTTP 语义，但实际 DATA 和流控交给协议库。

原始字节、压缩字节、HTTP 定界和 TLS 数据各自记录实际进度。输出背压不能阻塞共享输入驱动；没有数据或容量时挂起。应用没有读取响应时，只保留协议窗口及明确的本地缓冲，不自动在后台无限收集响应。

## 5. 版本选择与连接池

`HttpVersionPolicy { Auto, Http1Only, Http2Only }` 为高层策略；Auto 默认用于 HTTPS，以 ALPN 提供 `h2`、`http/1.1`。协商 h2 使用 HTTP/2，协商 http/1.1 或允许的无 ALPN 结果使用 HTTP/1.1。未知 ALPN、Http2Only 下未选 h2、TLS 验证失败均报错，不尝试通过改协议掩盖失败。

HTTP/1 默认发 1.1，显式 `http1RequestVersion` 可选 1.0；正常接收服务器的 1.0 响应并按其定界、持久连接规则处理。Http1Only 表达协议族，不把收到合法 1.0 响应当成 TLS 协商失败。

明文默认 HTTP/1.1。`Http2PriorKnowledge` 与 `H2cUpgrade` 是显式明文选项，不能自动探测或重复发送有副作用的请求。h2c 复用 M53 的升级与 stream 1 合同；拒绝升级交付真实 H1 响应，Http2Only 则报告未满足策略并收尾，不偷偷重放。RFC 9113 已弃用 h2c Upgrade，因此它仅是显式互操作选项。

Http2Only 用于明文时必须选定允许的 H2 入口；与明文 Http1 配置冲突在网络提交前报告。TLS／明文策略和请求版本不能以无效组合静默覆盖彼此。

每跳请求均依据新 URL、代理和 TLS 配置选路。池键包括规范化 origin、代理路由、TLS 信任／客户端身份配置和协议策略；不同凭据或 TLS 配置的会话不能错误复用。不按 IP 或证书 SAN 合并 origin。

HTTP/1 每条连接同时一个 exchange；body 完整消费、输出完成且协议允许持久连接后才归还。HTTP/2 复用 stream，按 peer SETTINGS 的并发上限和本地上限分配容量；SETTINGS 降低上限时保留合法在途流，暂停新建。GOAWAY 停止在该连接上创建新流，保留能够完成的已接受流；最后处理 stream ID 作为重试判断的事实输入。

池等待可取消并可超时，公平交付可用容量。容量在登记时预留，取消／建立失败／迟到成功均归还；不能只在连接成功后计数而导致并发超限。默认每 origin 最多 8 条连接、全局最多 64 条、每条 H2 的本地并发流上限 100，并始终服从 peer 更低限制；这些为可配置的真实连接／流限制。

空闲连接默认 60 秒回收，可设置最大存活时间以促使重新 DNS 查询。没有响应 body 未消费完的“空闲连接”。不把 getaddrinfo 结果伪装成含 TTL 的缓存：当前 resolver 不提供 TTL，使用调用方给定缓存时长须标明它是本地策略。HTTPS DNS records、Alt-Svc 和 HTTP/3 路由不在本期范围。

## 6. DNS、连接与代理

域名由 URL／IDNA 统一规范化后交给 asyncio resolver；DNS 与连接都可挂起。Happy Eyeballs 使用 asyncio 的地址族解析与 TCP 竞速能力，候选错误在各候选任务内部转为结果，选择成功后关闭所有未交付连接。默认连接尝试间隔 250 ms；保留底层无法立即撤销 getaddrinfo 的真实收尾时延。

TLS 始终验证 URL 的 DNS／IP 身份，不能按所连接 IP 替换主机名。默认使用 TLS 库的系统信任来源，支持显式 CA、证书链与客户端身份；mTLS 的实现归 TLS，不在 client 导出或操作 OpenSSL 对象。

支持显式 HTTP／HTTPS 代理、CONNECT 隧道、SOCKS5 和 SOCKS5 的本地／代理 DNS 两种模式。HTTPS 代理先建立到代理的 TLS；CONNECT 成功并接管全部后缀后，才建立目标 TLS。代理外层 HTTP 控制连接使用 HTTP/1.1；隧道中的目标协议可经 ALPN 选择 HTTP/2。SOCKS5 协商在 asyncio，CONNECT 与 absolute-form 报文在 HTTP 库。

Proxy-Authorization 只发往代理，不能进入隧道后的 origin 请求；Authorization 不能误用于代理。代理选择与凭据来源可通过 suspend 回调提供。环境代理通过显式启用的配置读取 HTTP_PROXY／HTTPS_PROXY／ALL_PROXY／NO_PROXY 等规则；默认不受环境变量影响。NO_PROXY 的域名后缀、IP、CIDR 和可选端口匹配由独立的代理配置函数处理，未知语法报配置错误；不实现 PAC 或系统自动发现服务。

## 7. 超时和取消

总截止时间覆盖排队、DNS、连接、握手、所有重试／重定向／认证、上传、头部与响应 body 消费，直到调用结束。execute 返回响应头不停止总计时。计时使用单调时钟；服务器日期、Retry-After 的绝对日期和缓存日期使用 wall clock，两者不混用。

| 选项 | 覆盖范围与默认值 |
| --- | --- |
| total | 完整调用，默认 None；长流可明确保持无总截止 |
| poolAcquire | 等待池容量，默认 None，仍受 total 约束 |
| resolve／connect／tlsHandshake | 各自阶段默认 30 秒；一次候选连接序列共享 connect 截止 |
| responseHeaders | 等待最终响应头，默认 None；启用时从该次请求头发出开始计时，包括并行上传 |
| readIdle／writeIdle | 有实际待处理读／写时连续无字节进度的时间，默认 None；应用处理数据或未发起读取不算停滞 |
| expectContinue | 默认 1 秒，到期允许发送 body，不单独令调用失败 |

每个阶段取自身上限与剩余 total 的较小者；重试、重定向和 token 刷新不重置总截止。收到无关 H2 控制帧或其他流的数据不能重置当前调用的 body 读停滞时间。

超时记录具体阶段与已发生进度，然后取消调用并等待资源安全收尾。超时不是系统操作的硬实时抢占；DNS／文件等已开始的 native work 仍须等实际完成。不得抢先恢复外部仍在借用调用方存储的操作。

H2 请求取消通过 stream close／RST_STREAM，其他请求继续；H1 取消未结束消息关闭连接。关闭整个 client 或连接错误才终止其全部 H2 流。超时与显式取消、网络错误保留区别，迟到完成遵守既有单次结果选择。

## 8. 重定向、重试与认证

重定向默认开启，最多 10 跳，只允许 http／https；HTTPS 降到 HTTP 默认拒绝，可由调用者明确开启。301／302 默认仅将 POST 改为 GET，303 将非 HEAD 改为 GET，307／308 保持方法与 body。改为 GET 时移除原 body 相关字段；保留 body 时必须满足重放条件。Location 用 URL.resolve，处理循环和相对引用。

跨 origin 移除 Authorization 与调用者提供的 Cookie，按新 origin 重新选择凭据和 Cookie；代理凭据按代理路由重新选择。Host／:authority 由新 URL 生成。每次 follow-up 在取得下一条连接前关闭旧响应 body；不先无界 drain 旧 body。

默认最多 2 次重试，网络原因须同时满足请求语义允许重试及 body 可重放。失败可能已提交请求时，仅默认重试幂等方法；POST 等需要应用明确提供策略。只有协议明确证明未处理，例如合适的 REFUSED_STREAM 或 GOAWAY 边界，才可对其他方法进行一次新的尝试，仍要求已消耗 body 可重新创建。状态码重试默认关闭，可配置 408／429／503 等、Retry-After、指数退避和抖动。

连接失效、响应中途截断、任意超时、认证失败和任意 5xx 不能统一视为可重试。一旦响应 body 已向应用交付，不把另一响应拼接到它后面；后续恢复属于显式下载续传。退避通过 EventLoop 定时等待，可取消，且受 total 与剩余次数约束。

内建 Basic、Bearer 和 RFC 7616 Digest（MD5、SHA-256、SHA-512-256 及相应 sess 形式，qop auth／auth-int）。摘要算法在 crypto，字段语法在 HTTP；认证不能私自实现密码算法。Digest 的 auth-int 在发头前取得实际发送的实体字节摘要，包括已应用的 Content-Encoding，不包括 chunked 等传输定界。使用可重放表示的独立读取或显式给定摘要，重建的编码字节必须一致；单次流若无法满足则返回明确认证／重放错误，不偷偷落盘或无限缓冲。

凭据获取与 Bearer 刷新为 suspend 回调，按实际保护空间或 origin 合并同一次并发刷新；失败交付相关调用，不取消无关 origin。限制认证往返，处理 stale nonce 与 nonce-count，避免 401／407 无限循环。OAuth 登录、账号存储和浏览器交互属于应用，不建立另一套认证框架。

## 9. 内容、Cookie 和缓存

自动内容解码默认开启，Advertise gzip、deflate、br、zstd，前提是本构建实际支持对应 decoder。Content-Encoding 多层编码按逆序解开；不支持的编码报明确错误或由显式原始 body 模式交付，不能悄悄少解一层。调用者显式提供 Accept-Encoding 时保留原值，实际解码策略仍独立配置。

`response.fields` 始终保存收到的字段；`wireContentLength` 表示编码后的长度，解码后 body 的 contentLength 只有确实已知时才有值。原始 body 模式交付完成 HTTP 定界后的编码字节，不包含 chunked／DATA 帧。Range 下载默认请求 identity；要对编码表示做 Range 时必须显式选择原始 body 和一致的表示条件。

上传压缩显式启用，复用 scoop.compression 的异步流，更新 Content-Encoding 与长度／定界信息；不能保留原文 Content-Length。multipart 边界、字段参数、filename*、HTTP 日期和媒体类型在 HTTP 库，二进制字节与字符集在各自 codec。下载目的 Path 必须由调用者指定；可提供解析后的建议文件名，不据此自行选择写入位置。

Cookie 默认关闭，提供显式 MemoryCookieJar 和可由调用者实现的 suspend CookieJar 接口。支持 host-only／Domain／Path、Secure、HttpOnly 元数据、Expires／Max-Age、替换与过期、cookie 前缀以及公共后缀限制。PSL 使用固定版本的 ICANN 与 PRIVATE 部分，数据和匹配属于 HTTP 域名工具；IDNA 不承担公共后缀政策。非浏览器客户端没有隐式页面上下文：启用 SameSite 发送判断时必须显式给出 site／顶层导航信息，未给信息时采用限制性规则，不虚构当前站点。浏览器脚本可见性和分区 Cookie 不在本期。

提供 RFC 9111 私有响应缓存，默认关闭，内置内存和磁盘实现，另有 suspend HttpCache 接口。处理 Cache-Control、Age／Date、Expires、Vary、ETag／Last-Modified 与 304 合并；no-store、private 和带 Authorization 请求按私有缓存语义处理，Vary: * 不复用。默认只自动存储完整 GET 响应，不启发式缓存 POST、不组合不完整 range 条目。

内置缓存归属单个 client 的私有上下文；显式共享存储时须区分缓存分区和认证上下文。与某个 Authorization／客户端身份关联的条目不能只凭相同 URL 命中另一个身份的请求，凭据本身不作为磁盘文件名或日志数据。

缓存保存未自动解压的表示字节与对应字段，键保留 URL query 的实际顺序和 Vary 所需请求字段。成功读完 body 后才提交条目；中途取消／解码错误不发布完整条目。磁盘写入用 asyncio 临时文件、关闭／按配置 sync 后同文件系统替换；原始网络与缓存 I/O 都可挂起，不能调用同步 Path API。HTTP 缓存的响应验证与 M48 编译缓存是不同功能，不能据此恢复编译产物复验。

下载便利入口接受显式 Path、覆盖策略和 suspend 进度回调；默认写入同目录临时文件，成功完成并关闭后再替换目标，取消保留原目标。显式续传模式保留部分文件与已确认的 URL、表示编码、长度和强 ETag，下一次使用 Range／If-Range；206 必须从期望偏移开始且验证相同实体，200 则按策略从头重建，416 只有在长度和验证条件证明已完整时才成功。实体变更、无可靠验证条件或范围不符时不得把新旧内容拼接。seek／truncate／metadata／替换全部使用 asyncio，缓存与续传共享格式工具，不共享不完整条目。

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
    LibuvEventLoop().run { scope ->
        withHttpClient(HttpClientConfig()) { client ->
            val request = HttpRequest.get(Url.parse("https://example.com/data"))
            client.withResponse(request) { response ->
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
