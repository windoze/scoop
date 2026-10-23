# Scoop HTTP server 库规范

状态：按[设计](../http-server/DESIGN.md)分步实施，已完成范围与验证记录见[进度](../http-server/PROGRESS.md)。本文规定普通 Cone `scoop:scoop.http.server:0.1.0` 的应用层合同。协议与异步传输继续遵守 [HTTP 规范](SCOOP-HTTP-SPEC.md)，不增加编译器或 runtime 特例。

## 1. 请求分派与依赖

HttpHandler 是 `suspend (HttpContext) -> HttpResult`，HttpNext 是 `suspend () -> Unit`，HttpMiddleware 是 `suspend (HttpContext, HttpNext) -> Unit`。应用共享对象通过构造参数或闭包传递；请求对象通过 HttpContext 或语言规范 8.3 的精确类型 Context 绑定传递。框架不提供服务容器、运行期函数参数扫描或自动事务提交。

RouterBuilder 配置路由、middleware、分组与 fallback；build 返回不可变 HttpApplication，并关闭该 builder 及其 RouteGroup 的修改入口。已注册的字段、路径和 middleware 列表保存快照。不同 application 互不修改配置；捕获的业务对象是否允许并发由应用负责。

HttpContext 属于一次请求，保存 request、response、route、pathParams 和 requestId。实际执行必须处于活动 Job 中，requestJob 为该请求任务。框架在执行全局 middleware 前绑定精确 HttpContext。离开请求管线恢复原绑定；挂起和恢复不结束绑定。应用 Context 的缺失是装配错误，默认 500，不是匿名用户的认证结果。

## 2. 路由与提取

方法为大小写敏感 HTTP token。路径按原始 `/` 分片，各片段至多百分号解码一次；编码斜线不形成路由层级。静态片段比较解码后的字节，参数保留原始和解码后的字节；String 访问使用严格 UTF-8。路径不折叠连续斜线、尾斜线或点片段，query 不参与路由。

路径形状支持静态片段、`{name}` 和仅出现在末尾的 `{*name}`。参数名须非空且同一路由不重复；普通参数匹配非空片段，末尾通配匹配非空后缀。畸形花括号和非法百分号为配置错误；请求中的非法百分号和文本转换错误为 400。

配置期按路径片段构造 trie。完整匹配按静态、参数、通配的顺序选择；静态分支没有完整匹配时允许回退。先选择路径资源，再选择方法；同方法、同路径形状的重复注册拒绝。不同方法可使用不同的参数名，取最终选择的端点声明。分组前缀不能包含末尾通配，同形状分组前缀重复注册拒绝，以免 fallback 和组级 middleware 有两个相同归属。

显式 HEAD／OPTIONS 优先；HEAD 否则使用 GET handler，但不执行响应 producer。普通路径 OPTIONS 默认 204，Allow 包括所有显式方法及适用的隐式 HEAD、OPTIONS，稳定按 token 排序。资源无相应方法时 405，不退回低优先级路径。OPTIONS * 只经全局 middleware，Allow 为全应用的方法并集。未配置的 CONNECT 为 501；非 origin-form 的普通 H1 target 不作为路径匹配，返回 400。

无端点匹配时采用最长适用分组的 fallback，等长按静态片段优先；默认 404。404 和默认 405／OPTIONS 执行这个最长适用分组的 middleware，正常端点执行其注册分组及端点 middleware。全局 middleware 覆盖所有这些结果。route 只在路径端点选定后可见，fallback 没有虚构的端点模板。

Query 复用 UrlQuery，普通 URI 的 `+` 保持加号。标量读取拒绝重复值；optional 只容许字段缺失，不把无等号或空字符串自动当作缺失。Path 参数缺失为程序错误，转换失败为 HttpRejection。DTO 提取遵守第 9 节，结构化 body 遵守第 4、8 节。

## 3. 管线与响应

middleware 按登记顺序进入，反向退出。每次只能等待 next 一次、respond 一次，或抛异常；重复／并发 next、未处理却正常返回、响应后再委托均报程序错误。next 的寿命包括 handler、结果执行、流式 producer、finish 和下游清理。局部并发必须在资源作用域内等待，不因 Context 绑定自动延长资源寿命。

HttpResult 保存状态码、HttpField 快照和 HttpBody。基础 body 为 Empty、Bytes 和 Stream；Stream 保存可选已知长度、挂起 producer 和完成时取得的 trailer。text、bytes、json、redirect 和 empty 是普通构造函数。JSON 使用显式或 companion ValueEncoder，不反射业务对象。

HttpResponse 保存提交前的 set／add／remove 操作及 beforeCommit。set／remove 按字段名 ASCII 大小写无关地替换／移除；add 保留重复项，Set-Cookie 不合并。执行依次应用结果字段、暂存操作、普通非挂起 beforeCommit；回调接收可修改的 HttpResponseDraft，回调序列退出后草稿只读。状态依次为准备、已提交、已结束，失败可转为已中止。开始不可撤回的发送尝试前标为已提交，之后不再修改头或发送第二个最终响应。

HttpBodyWriter 是执行期间借用的 AsyncByteSink 视图，允许短写，flush 不代表客户端已经处理内容。producer 不能 finish 底层 HTTP 消息。原文已知长度在实际写入时检查，完成时检查精确长度；超长／过短均失败。trailer 回调为显式可选项；H1.1 带 trailer 的响应使用 chunked，不能同时提供 Content-Length，但仍检查 producer 声明的原文长度。H1.0 不支持 trailer，在提交前报程序错误。HEAD 不调用 producer 或 trailer 回调；204／304／205 等响应按 HTTP 消息内容规则处理。正常输出依次完成 producer、trailer 和底层 finish；结束后 writer 不可再用。

handler 和结果准备失败在终端内部映射，使错误结果仍在业务 Context 范围内执行。HttpRejection 携带明确结果；其他非取消失败默认 500，正文仅含稳定提示和 requestId。beforeCommit 失败不重放回调，以不应用原草稿的最小 500 收尾。mapper 或错误结果失败直接中止，不递归映射。最外层另外处理 middleware 自身失败。取消及提交后的异常保留实际原因并中止，发送后的清理异常不能改写已经发出的状态。

错误观察入口是普通非挂起函数，接收当前 context 和原始异常；观察器失败不得替代主异常或造成另一条正常请求失败。观察器与应用代码均不在框架内部锁内调用。

## 4. 字段、Cookie 与结构化正文

RequestHeaders 保留 head 的字段顺序与字节。getAllBytes 不合并同名字段，getSingleBytes 拒绝重复值；对应 String 方法严格解码 UTF-8。不同字段的列表／合并规则继续通过 scoop.http 中相应的解析器使用，不提供自动拼接任意 header 的入口。

RequestCookies 延迟复用 HttpCookieHeader.parseFields 的结果；getAllBytes／getAll 保留同名值顺序，getSingleBytes／getSingle 仅在缺失时返回 None，重复项为 400。ResponseCookies.append 追加独立 Set-Cookie；delete 必须明确传入 name、path 和可选 domain，生成该范围的 Max-Age=0、过去 Expires，不能清除其他未知范围的同名 Cookie。响应修改不影响本次请求。

HttpRequestBody.json 使用显式 ValueDecoder 或 T.Companion；只接受 application/json 和 application/*+json。form 接受 application/x-www-form-urlencoded，复用 FormUrlEncoded；其加号为空格。两者检查唯一且语法合法的 Content-Type；缺失／不支持的媒体类型或 charset 为 415，畸形／重复字段为 400。只支持 UTF-8，未声明 charset 也按 UTF-8 处理。没有安装内容解码策略时，结构化入口对非 identity Content-Encoding 返回 415；bytes 和 openStream 保持原始字节。正文解析、UTF-8 和 codec 输入不匹配在提取边界变为 400；业务代码自行抛出的解码错误仍走 500 映射。

结构化入口先检查元数据，再取得单次消费权并执行有界聚合；不会为错误媒体类型读取上传。bytes、text、json、form、multipart 与 openStream 共用消费权。请求结束后不能选择新的消费方式或修改输入上限，以免在已结束的请求上创建无归属的 decoder 视图。JSON 仍使用现有完整值树 codec，不是流式 JSON 解码器；multipart 按第 8 节逐 part 读取。

## 5. 连接接入、限制与停机

HttpServer.takeOwnership(listener, application, scope, options) 检查选项和当前／父 Job，在父 scope 中登记自己的所有者后接管 listener。登记前失败仍由调用方清理；登记后失败由 server 清理。返回表示接受循环已建立。application 和可共享的 TlsServerContext 由调用方持有，不随 server 自动关闭。一个连接的握手、业务和清理失败在连接边界隔离；不取消其他连接。

明文默认 H1；allowCleartextHttp2 显式启用 prior-knowledge 探测，保留探测前缀后交给对应协议连接。TLS 在每条连接的独立任务完成握手，ALPN 支持 http/1.1 和 h2；allowTlsHttp2 可关闭 H2，allowHttp1Fallback 控制无 ALPN 的 H1 回退。证书／私钥及 ALPN 来自应用提供的 TlsServerContext；协商结果必须属于已启用协议。h2c Upgrade 不在当前接入范围。

每条 H1 连接顺序处理请求，每条请求进入独立 coroutineScope，创建独立 HttpContext、body 和 requestId。HttpConnectionInfo 提供只读本地／对端地址、协议与 TLS 信息；不把 X-Forwarded-* 当成已验证地址。已完成请求的业务 Context 不泄漏到下一条请求。HEAD 和无内容状态继续由协议层定界，未知长度 H1.1 为 chunked、H1.0 为 EOF。响应写出后才放弃未消费的输入；未读完时强制 Connection: close，不自动 drain 上传。

Expect 使用 HTTP 消息层的完整解析结果。包含未知 expectation 时默认 417，畸形语法 400；只有应用第一次读取非空 body 且尚未提交最终响应时，才发送一次 100-continue。100 不经过最终响应草稿或完成通知。应用直接拒绝不先发送 100；最终响应开始后不再从未读 body 触发 100。

默认 maxConnections=1024，maxActiveRequests=256；连接容量覆盖握手和 keep-alive，请求容量覆盖 handler、流式输出及其清理。登记、排空和容量判断在同一状态边界完成；无容量直接关闭新连接／当前 H1 连接，不为拒绝响应另建等待队列。requestBodyLimit=8 MiB，bufferedBodyLimit=2 MiB；路由可在选择 body 消费方式之前明确调整 body 总量上限，非法负值是配置错误。

TLS 握手总时限默认 10 秒；首条请求头从握手完成（明文为连接任务开始）到完整首部的总时限默认 10 秒，包含明文协议探测；H1 后续无活动请求时从开始等待到下一完整请求头的总时限默认 60 秒。bodyReadIdle 和 responseWriteIdle 默认 30 秒，复用异步 HTTP 层真实 I/O 进展计时；这些不是请求总时限。requestDeadline 默认关闭，启用后覆盖进入管线直到响应和请求子任务结束。超时执行协作式取消并等待清理，不在已关闭连接上补发 504。H1 没有独立的断开通知，TCP 半关闭仍可以等待响应。

HttpContext.setDeadline(Duration?) 允许端点或 middleware 在最终响应提交前覆盖本次请求的总时限；None 明确关闭，非负 Duration 始终从本次请求进入管线时计起，不从设置时重新计时。新的期限可以缩短或延长，也可以在默认关闭时启用；不改变连接或 I/O idle 规则。已到期、取消或结束的请求不能被恢复。期限观测与请求处于同一结构化作用域，覆盖管线及其子任务的实际结束；到期后等待取消清理，完成通知保留实际到期时配置的时长。

server 有 Running／Draining／Closed 三态。shutdownGracefully 先停止接受并取消无活动请求的 H1 连接；活动请求完成后关闭，不进入下一次业务分派。默认自然排空宽限 30 秒，届满后取消所有者和剩余连接，等待真实 I/O 与资源清理。close 直接执行强制取消并等待；并发等待共享同一所有者结果。awaitTermination 只等待。调用方取消关闭等待也不能跳过本地资源清理；清理失败保留实际异常。

onRequestCompleted 在响应、请求子任务和输入收尾结束后接收不可变快照：requestId、route、method、状态、耗时、接收／交付及输出字节和完成类别。onConnectionError 观察握手或连接边界的失败。观察函数非挂起、在内部锁外调用；自身失败不改变已发出的结果或取消其他请求。完成不承诺对端已处理数据。

## 6. H2 请求与排空

每个 H2 stream 在独立的请求 coroutineScope 中执行，拥有自己的 HttpContext 和 Job。共享协议驱动与 TLS 属于连接；请求异常、deadline 或对端 RST_STREAM 只取消对应请求。StreamFailure 通知映射到已登记的请求 Job；登记后读取一次 stream.failure 衔接提前到达的失败。通知仅修改框架状态和通知内部等待，不在协议锁内运行应用 handler。请求清理后移除登记；无需转移底层 stream 的接受 Job。

H2 的接收选项默认明确选择 Http2InvalidRegularField.RejectStream，Http2Limits 与 Http2Settings 直接复用既有类型，可显式覆盖。全局 maxActiveRequests 在启动业务任务前占用，直到资源与完成通知收尾后释放；超容量只关闭对应 stream，不建立拒绝任务队列。正常响应使用 HEADERS／DATA／END_STREAM 和独立 trailer，不写 Transfer-Encoding。100-continue 仍在首次实际 body 读取前按需发出。正常双向完成不追加 CANCEL；提前响应先完成输出，再放弃尚未消费的输入。

firstRequestHeadTimeout 覆盖首个完整请求之前的前言／首部等待；每个已识别的初始请求 HEADERS／CONTINUATION 块另外受 http2HeadTimeout 总时限控制，默认 10 秒。该时限由 HTTP 层 RequestHeadStarted／RequestHeadReceived 及真实终止通知驱动，不被其他 stream 或控制帧刷新。部分帧头及 padding／priority 前缀尚未被后端识别为新请求块时，首条请求仍由 firstRequestHeadTimeout 覆盖；后续在没有活动请求时由连接空闲时限覆盖。首部块超时结束共享连接，因为未完成的 HPACK 块阻止继续解析其他流。trailer 继续按已有 stream 的读取规则处理。

H2 keepAliveIdleTimeout 仅在首个请求之后、没有活动请求、已排队请求及正在接收的初始首部块时计时。长响应与只等待外部服务的活动请求不会因接受下一条请求的等待而超时；SETTINGS／PING 不延长真正空闲。总请求 deadline 与每流 bodyReadIdle／responseWriteIdle 各自保持独立合同。

优雅停机对已登记的 H2 连接请求 GOAWAY，保留接受 Job 和协议所有者。GOAWAY 边界覆盖底层已接受请求，acceptRequest 继续交付边界内队列，然后自然返回 None；这批请求仍受全局容量约束。停止接收 TCP、H2 连接登记与排空请求共用 server 状态边界，避免错过登记竞争。已开始发送的 GOAWAY 不因并发 shutdown 重复提交。宽限覆盖 GOAWAY 发送、请求执行和自然清理，届满后取消连接及所有剩余请求，并等待真实收尾。接受循环结束后仍等待请求子任务，再关闭协议连接。

## 7. SSE 输出

sse(options, produce) 构造普通 Stream 结果，状态为 200，Content-Type 为 text/event-stream; charset=utf-8，Cache-Control 为 no-cache，不设置固定长度。响应头提交并 flush 后才执行 producer；HEAD 不执行 producer 或启动心跳。SSE 不自动修改 requestDeadline，端点可在返回结果前显式调用 setDeadline。HttpRequest.lastEventId 复用单值字段提取，缺失返回 None，重复或非法 UTF-8 为 400；重放和历史存储由应用负责。

SseWriter 仅在 producer 执行期间可用，借用 HTTP body，不提供结束响应的入口。send(SseMessage) 与 comment(String) 默认在完整块写出后 flush；可明确传入 flush=false 合并发送，再调用 flush()。编码与短写复用 HTTP 消息库的 SseEncoder／SseSink，不缓冲完整事件副本。应用至多执行一个发送或 flush 操作，并发调用在写出前失败，不破坏正在发送的事件。

SseOptions.heartbeatInterval 默认 None，启用时必须为正 Duration。计时从最近完成的输出操作起算；空闲到期发送一个空注释块并 flush。心跳与应用操作在完整事件边界串行，正在写入或背压时跳过本次心跳，不积累队列；应用可等待已开始的心跳完成。心跳只属于 producer 的结构化作用域。producer 正常返回时停止新心跳，等待已开始的心跳写出，然后结束 HTTP body；失败或取消时取消并等待心跳，释放本地缓冲后中止。任何失败路径都不补发事件。H2 reset 和停机取消外部事件等待；H1 无 I/O 时的断开观测限制仍适用。

SseOptions.allowCompression 默认 false；明确设为 true 时参与第 10 节安装的响应压缩策略，flush 穿透压缩同步点后再刷新 HTTP 输出。

## 8. Multipart 上传

HttpRequestBody.multipart(options) 在读取前检查唯一 Content-Type、multipart/form-data、有效 boundary 和已安装的内容解码策略，然后占用 body 唯一消费权。未安装解码时非 identity 编码为 415；缺失或不匹配的媒体类型为 415，重复／非法字段及缺失／非法 boundary 为 400。构造不预读，不创建文件。RequestMultipart 借用选定的请求流和现有 MultipartSource；close 只释放本地视图，不自动 drain，也不提前关闭底层 HTTP body。请求执行器在管线结束后统一关闭本地视图，再收尾 HTTP 输入；用户也可在 producer／handler 中 useAsync 提前关闭。

nextPart 按线上顺序返回 RequestPart，完成外层 EOF 验证后返回 None。当前 part 必须读到 EOF 或完成显式 discard 后才能推进；旧流在推进后失效。part.head 保留原字段；nameBytes 是必需的 form-data name 参数，filenameBytes 可缺失或为空。空 name 保留；name／filename 的文本属性严格按 UTF-8 解码，不猜编码、不百分号解码、不解释路径、不优先使用 filename*。重复字段与附件分别交付，不折叠成 Map；缺失／非法 Content-Disposition 或 name 为 400。

part.openStream、text 和 copyTo 共用一次消费权。discard(maxBytes) 可显式丢弃尚未读取的余下内容；关闭未完成 part 终止整个 multipart 视图。RequestMultipartOptions 复用 MultipartLimits，另外默认 maxFieldBytes=64 KiB、maxPartBytes=8 MiB、bufferSize=8192；上限须非负，bufferSize 须为正。每个 part 的实际累计字节在所有入口之间共享，文本上限取调用上限、字段上限与 part 上限的最小值。copyTo(sink, maxBytes) 使用有限缓冲并处理短写，借用 sink，不隐式 flush 或关闭。discard 复用底层有界丢弃，仍受剩余 part 上限约束。所有总量上限至多多读一个字节来区分恰好到限和超限；超限为 413，并使当前 multipart 视图不可继续读取。外层头部、boundary、preamble 和 epilogue 仍计入 requestBodyLimit。

text 可显式传入 Charset，也使用 part Content-Type 中声明的有效 charset；两者冲突为 400，输入声明的不支持 charset 为 415。默认严格 UTF-8，BOM 行为复用 Charset.decode 的默认规则；只把实际文本输入错误映射为 400，后端故障保留原异常。_charset_ 是普通字段，嵌套 multipart 是普通正文，均不自动改变后续解析。格式错误／截断为 400，底层 MultipartLimits 超限为 413；只有读取本次输入产生的错误在提取边界转换，应用 sink、文件和业务错误保持真实原因。

保存文件由应用显式组合 AsyncFile、copyTo、createTempFile 和 moveFile。客户端 filename 不参与路径选择；需要失败不发布时，应用读取所有 part 及外层 EOF、关闭文件后才移动到选定目标，并在失败或取消时删除自己创建的临时文件。库不增加存储插件或后台清理服务。

## 9. Query DTO 与静态提取适配器

RequestQuery.decode(codec, options) 向现有 ValueDecoder 提供扁平 named record，另有使用 T.Companion 的泛型重载。不经过 JSON 或反射。QueryDecodeOptions.unknownFields 默认 QueryUnknownFields.Reject，可显式 Ignore。同名字段按严格 UTF-8 解码后的名称分组，保留首次出现顺序、每项顺序及从零开始的原 query 项位置；百分号只在 UrlQuery 中解码一次，普通 query 的加号仍为加号。点、方括号、逗号和 null 文本均无特殊含义。

required 缺失报 400；optional 缺失返回 None，默认值及 Option 的组合由原 codec 决定。取得现有字段算访问，枚举 keys 不算；keyed.end 在 Reject 模式检查未访问字段。检查不关闭容器或消耗输入，成功或失败后都能继续读取，再次 end；标量可复读，每次 unkeyed 创建独立游标。入口不额外强制 codec 调用 end 或重放容器检查。

字段允许标量或一层序列；标量要求恰好一项，序列按收到顺序交付，单项也可组成列表。name= 是空字符串，裸 name 在实际读取时为缺值，缺失字段不自动成为空列表。String 严格 UTF-8；Boolean 仅接受 true／false；整数复用十进制文本解析，有符号为 `[+-]?[0-9]+`，无符号为 `[+]?[0-9]+`。浮点复用按目标精度解析的有限十进制数，语法为 `[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?`；拒绝空白、非有限值及溢出，允许下溢与负零。未读取值不强制转换为文本。所有形状上的 isNull 均为 false；readNull、根标量／序列、嵌套 keyed／列表属于不支持的 codec 形状，抛 QueryCodecException，默认 500。

输入错误在 decode 边界转为 HttpRejection 的 400 结果，保留原 DecodingException 为 cause。path 遵守语言规范 11.13.3 的 JSON Pointer，序列使用字段内索引；错误详情另带原 query 项位置。自定义 codec 抛出的其他异常照常传播；handler 随后抛出的 DecodingException 仍是业务失败，默认 500。HttpRejection 的可选 cause 只用于观察原异常，不改变结果选择。

HttpExtractor<T> 是普通 `suspend (HttpContext) -> T`；adapt(extract, handle) 顺序等待提取并调用 `suspend (HttpContext, T) -> HttpResult`，失败不调用 handle。queryExtractor 的显式／companion 重载复用 query.decode；jsonExtractor 的对应重载复用 body.json，未指定 maxBytes 时采用该请求的 bufferedBodyLimit。多个来源由应用以普通 extractor 顺序组合；两次 body 提取仍受唯一消费权约束。不增加资源持有者、DI 或参数注入，Context 与流式响应仍在原请求管线和 middleware 作用域内执行。

## 10. 响应压缩

responseCompression(options) 是普通 middleware，在 next 前安装策略，退出时恢复外层策略；内层可覆盖外层，不能在提交后安装。ResponseCompressionOptions 默认仅 gzip，minimumBytes=1024、bufferSize=8192；编码列表保存快照并按顺序作为服务端偏好，拒绝重复编码和 RawDeflate，HTTP deflate 只对应 Zlib。阈值须非负、缓冲须为正；已有算法选项继续由 compression 的实际编码器构造校验，不另建参数校验器。字典等外部资源由应用持有。shouldCompress 默认接受 text/*、application/json、application/xml、+json／+xml；应用可显式提供媒体类型判定函数。不探测 body 或预读 producer。

表示选择在 beforeCommit 之后、HTTP 定界与提交之前进行。请求 Accept-Encoding 复用消息工具规范 1.14 的解析值，同一请求只解析一次；非法值在首次需要协商时返回 400，错误响应不再解析该字段。缺失或空字段选择 identity；最高质量和服务端顺序决定其他候选，显式 identity 参与比较，隐式 identity 是回退。已知原文长度小于阈值时优先 identity；identity 被禁止且有可用编码时可以越过阈值。未知长度直接按媒体类型选择。没有可接受表示时生成空 406，不运行原 producer 或再次协商该错误。

HttpResult.allowCompression 默认 true，可明确关闭动态变换。已有 Content-Encoding、206／Content-Range、禁止压缩的结果或 Cache-Control: no-transform 不动态变换；已有编码链的每一项仍须为客户端接受，否则为 406。204／205／304 和成功 CONNECT 等不携带表示内容的响应跳过协商；HEAD 按 GET 的表示规则选择但不运行 producer 或创建压缩器。304 保留已选表示的元数据。文件结果按其专属规则选择表示并禁止再次压缩。

参与协商的响应合并 Vary: Accept-Encoding，含最终选择 identity 或 406 的结果；字段名去重，Vary: * 保持。动态压缩写入 Content-Encoding 并移除原文 Content-Length；已知原文长度及显式长度一致性在输入侧检查，编码后的长度为未知，不完整执行 producer 来测长。HEAD 同样省略未知的编码后长度。动态压缩将强 ETag 弱化并保留弱标签；移除 Content-MD5、Digest、Content-Digest 和 Repr-Digest，响应 trailer 中同样处理标签和摘要，并从 Trailer 声明移除失效摘要。406 保留非表示字段，移除原表示的类型、长度、编码、范围、验证器、摘要及 trailer 声明。

执行器用现有 AsyncCompressionSink 借用 HTTP writer，再把受原文长度约束的 HttpBodyWriter 交给 producer；两侧字节数分别进入完成快照。flush 先排出压缩同步点，再 flush HTTP。正常顺序为 producer 完成、原文长度检查、压缩 finish、trailer／HTTP finish、释放本地压缩器；失败或取消只关闭，不补完压缩流。没有压缩时复用原 writer。任何实际编码器创建、输出或清理失败均按实际响应提交状态处理，不改写已发送的响应。

通用压缩策略不自动执行条件请求；应用做强验证前须已选择实际表示，或禁用动态压缩。文件表示的条件与范围由其专属实现处理。

## 11. 分步交付与验证

接入、超时与停机、SSE、multipart、WebSocket、压缩、文件和 DTO 的完整范围保持设计第 12～17 节，按完成的功能逐项扩充本规范。尚未实现的功能不公开占位 API。测试用所属 Cone 的 release 测试；只有 CLI／诊断／外部对端等必要边界使用 fixture。协议引擎的既有验证不在 server 重放。
