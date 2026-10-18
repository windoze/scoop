# Scoop HTTP 消息工具规范

版本：M54，实施中。实际实现与验证见 [M54 进度](../milestone54/PROGRESS.md)。本文补充 [HTTP 规范](SCOOP-HTTP-SPEC.md)第 9 节，所有能力在 `scoop.http` 或 `scoop.http.asyncio` 中交付，不在 web client 内另建协议实现。

## 1. 字段与标准数据

保持 HttpField 的有序重复字节模型。工具按具体字段语法解释字节，解析结果为不可变值；只在格式规定可合并时合并，Set-Cookie 保持逐项处理。未知参数保留，非法结构报告具体字段与位置，不把非 UTF-8 字节无条件转成 String。

| 工具 | 必须覆盖的合同 |
| --- | --- |
| `HttpMediaType` | type／subtype、参数、quoted-string 与转义，重复参数有明确报错或选择规则 |
| `HttpContentDisposition` | disposition、filename 与 RFC 8187 filename*，有效扩展参数优先；保留原字段和解析失败原因 |
| `HttpDate` | 生成 IMF-fixdate；接收 RFC 9110 允许的三种日期形状，旧两位年份按显式参考时间判断 |
| `HttpCacheControl`、`HttpEntityTag` | 指令与值、弱／强 ETag、引号与列表边界；不自己查询或存储缓存 |
| `HttpRange`、`HttpContentRange` | bytes 范围、suffix／开放终点、206／416 所需边界，受检 Long 算术 |
| `HttpAuthChallenge` | 多 challenge、quoted 参数、token68 和 Digest 参数；不获取凭据或执行密码计算 |
| `HttpRetryAfter` | delta-seconds 或 HTTP-date，解析与实际等待分开 |

参数名大小写、重复值处理和扩展编码按字段所属 RFC 固定，不能用一个逗号 split 处理全部字段。扩展参数先在本层拆出 charset／language／百分号字节，再交给 URL 百分号 codec 与 charset 转换；优先满足 RFC 8187 的 UTF-8，历史 ISO-8859-1 兼容须显式记录。建议文件名只作为元数据，操作系统 Path 选择由调用者负责。

### 1.1 媒体类型与字节参数

`HttpMediaType(type, subtype, parameters)` 和 `HttpParameter(name, value: List<Byte>)` 是不可变值构造器。type、subtype 和参数名须为非空 ASCII token，统一为 ASCII 小写；参数值独立复制，允许 quoted-string 中的 HTAB、可见字节及 obs-text，拒绝其他控制字节。重复参数名（忽略 ASCII 大小写）一律报错，包括值相同的重复项；未知参数和顺序保留。

`HttpMediaType.parse`／`parseOrNone` 接收字节或 String；String 明确按 UTF-8 转成字段字节，不把接收的任意字节猜成 UTF-8。`parameters` 与 `parameter(name)` 交付去除引号和 quoted-pair 转义后的字节；`HttpParameter.wasQuoted` 记录解析时的引号形式，普通构造值为 false。`originalValue` 在解析结果中保存完整原始字节，在普通构造值中为 None。

解析接受前后 OWS 和 RFC 9110 parameters 中的空分号项，拒绝 type／slash／subtype 之间及参数等号两侧的空白；引号内的分号、逗号和转义字符不作为分隔符。字段内换行、非法 token、未闭合引号、坏 quoted-pair 和多余尾部均报错。`HttpFieldParseException` 保存 field、byteOffset 和 detail；偏移从原始字段值的第一个字节算起。parseOrNone 只吸收这种格式异常。

`toByteArray()` 生成可发送的规范字段值；保留参数次序，必要时加引号并转义引号／反斜杠，已解析的 quoted 参数继续使用引号。`toHttpField()` 生成 Content-Type 字段。两者不隐式解释 charset 参数，也不把 obs-text 转成展示字符串；字符解码由显式 charset 操作完成。

### 1.2 Content-Disposition 与扩展参数

`HttpContentDisposition(disposition, parameters)` 与配对的字节／String parse、parseOrNone、parameter、toByteArray、toHttpField 遵守同样的不可变值合同。disposition 和参数名按 ASCII 小写保存；参数顺序和原始字段保留，重复名报错。按 RFC 6266 接收参数等号两侧 OWS，不接收空分号项。未知 disposition 与参数只保存，不猜测业务含义。

`HttpParameter.extended(name, text, language = "")` 生成 RFC 8187 的 `name*` 参数：name 是未加星号的非空 attr-char 序列，编码固定为 UTF-8，值由 URL 百分号 codec 编码。`decodeExtended(allowIso88591 = false)` 返回含 text、language 和实际 Charset 的 `HttpExtendedValue`；接收端默认只接受 UTF-8，显式开启兼容后另接受 ISO-8859-1。禁止 quoted-string 形式、缺失 charset／分隔符、非法 attr-char、坏百分号或无效编码，不做 RFC 2231 continuation 拼接。语言标签检查 RFC 5646 的结构、重复 variant／extension 和固定 grandfathered 形式，不查语言注册表，也不改变原大小写；空语言合法。

扩展参数先按 HTTP 语法分离 charset、language 和编码字节，再调用 url／charset；UTF-8 BOM 作为实际文字保留。`HttpFieldParseException.cause` 可保留原 URL／charset 格式错误；解析得到的参数诊断使用原字段及原始字节偏移，普通构造参数的诊断相对其 value 字节。不吸收内存、后端等非格式错误。

`HttpContentDisposition.filename(allowIso88591 = false)` 返回 `HttpFilename`：text 是可用建议名或 None，language 在有效扩展名被选中时为 Some（可含空字符串），extendedFailure 保存被忽略的无效 filename* 原因。有效 filename* 优先；其缺失或无效时使用普通 filename，普通值按 RFC 6266 的 ISO-8859-1 解码，不猜 UTF-8、不额外百分号解码。仅有无效扩展名时 text 为 None，失败原因仍存在。此结果不转换或清理操作系统路径，下载目标仍由调用者显式选择。

### 1.3 HTTP 日期与 Retry-After

`HttpDate.format(Instant): String` 只生成 IMF-fixdate，使用 UTC 和固定英文星期／月份词；低于秒的部分向下取整。`parse(value, reference: Instant, caseInsensitive = false)`／`parseOrNone` 接收字节或 String，返回 std 的 Instant，完整接收 IMF-fixdate、RFC 850 和 asctime 三种形状。仅字段前后可有 OWS，内部空格、数字宽度和 GMT 按各格式检查；日期须为真实日历日期，星期须相符，年限按 RFC 9110 §5.6.7 引用的 RFC 5322 §3.3 及四位年份语法限定为 1900～9999。格式错误的 field 为 HTTP-date，日期与 UTC 算术复用 std，不查询当前时钟或本地时区。

两位年份使用显式 reference 的 UTC 日历时间：以其后 50 个日历年为上界，选择不晚于上界、末两位相同的最近年份。比较包含月、日和时分秒；reference 的 UTC 年须在 1900～9999 内，只有旧式两位年份需要它。结果仍须满足日期及年份范围。这使世纪选择和 50 年边界可重现，不依赖运行机器的当前年份。

秒接受 00～60；std 的 Instant 不表示独立闰秒，输入 60 映射到同一分钟的 59 秒，不查闰秒表，不把缓存有效期向后延长。`caseInsensitive = true` 按 RFC 9111 的缓存接收建议匹配英文词和 GMT；它不允许其他时区或任意日期形状。输出始终使用规范大小写。

`HttpRetryAfter` 为 `DelaySeconds(Long)` 或 `At(Instant)`；parse／parseOrNone 与 HTTP 日期使用相同 reference 和大小写选项，格式诊断保留 Retry-After 字段名。delta-seconds 必须为非负十进制 Long，逐位检查溢出，不接受符号或小数。`toString()`／`toHttpField()` 输出合法字段；直接构造的负 DelaySeconds 在输出或计算时报告参数错误。

`delayFrom(now): Duration` 将 DelaySeconds 原样表示为 Duration，将日期转换成相对显式 now 的非负等待量，已经过去的日期返回 ZERO。该操作不读时钟、不休眠，不把合法的大秒数先转换成会溢出的纳秒或到期 Instant；实际等待与策略上限属于 client。

### 1.4 字节范围

`HttpRange(ranges: List<HttpByteRange>)` 保存不可变且非空的范围列表。`HttpByteRange` 为 `FromTo(first, last)`、`From(first)` 或 `Suffix(length)`，参数为非负 Long，显式终点不得小于起点；构造 HttpRange 时统一检查。保留重复、重叠和原顺序，不自动合并。`parse`／`parseOrNone` 接收字节或 String，按大小写无关的 bytes 单位解析；接受列表两端、逗号周围 OWS 及列表中的空项，但结果至少有一项。单位与等号之间及区间内部不插入空格。未知单位、非法项、负数和数值溢出报告 Range 字段与原始字节偏移，原始值保存于 originalValue。`toString()`／`toByteArray()`／`toHttpField()` 生成规范字段。

`resolve(completeLength: Long): HttpRangeResolution` 使用非负的已知表示长度，将可满足项转为实际的 `HttpResolvedRange(first, last)` 闭区间；终点截到表示末尾，超长后缀选择整个表示，仍保留输入顺序与重复。结果同时提供 ranges 和 isSatisfiable。按 RFC 9110 §14.1.2，非零 Suffix 在长度为零时仍可满足，此时 ranges 为空；Suffix(0) 不可满足。不得仅凭实际区间列表为空就判断 416。

`HttpResolvedRange` 检查端点及 `length = last - first + 1` 能用 Long 表示；Request Range 的大终点可以在 resolve 时截断，不提前按未截断长度拒绝。范围始终相对于实际表示的字节序列，包含其 content coding，不能擅自改成解压后偏移。

`HttpContentRange(range: HttpResolvedRange, completeLength: Long? = None)` 表示已有范围及已知／未知总长度；已知总长度必须大于 last。`HttpContentRange.unsatisfied(completeLength)` 表示 `bytes */length`，长度可为零。配对 parse／parseOrNone、originalValue 和输出方法接受／生成这两种形状；未知总长度使用星号，unsatisfied 的总长度必须已知。分隔符按字段语法检查，只有字段前后允许 OWS；非法边界、无法表示的区间长度或数值溢出报告 Content-Range 字段。工具不检查响应状态、不读取 body，也不决定断点续传或文件写入位置。

### 1.5 Cache-Control 指令

`HttpCacheDirective(name, value: List<Byte>? = None)` 保存 ASCII 小写指令名及可选的独立字节参数；空字符串参数与无参数不同。`HttpCacheControl(directives)` 保存不可变的有序列表。parse／parseOrNone 接收单个字节或 String 字段，parseFields 接收 HttpField 列表并合并所有 Cache-Control 行；originalValues 保留每条原始字段，普通构造为空。按 RFC 9111 接收 token／quoted-string 参数及转义，保留 wasQuoted、未知指令、重复项、顺序和非 UTF-8 参数；逗号只在引号外分隔，空字段及列表空项允许，等号周围不额外接收空白。

`findAll(name)`／`contains(name)` 按大小写无关名称查询；`single(name)` 返回唯一项或 None，重复时以第二项的原始位置报错，不静默覆盖。语法解析不提前应用缓存策略；调用者可以用 findAll 实现 RFC 9111 允许的首值规则，或将严格查询／参数解释失败视为无效缓存信息。

`HttpCacheDirective.deltaSeconds()` 要求完整的非负十进制参数，接收已解码的两种参数形式；按 RFC 9111 §1.2.2 将溢出饱和到 Long.MAX_VALUE，溢出后的剩余字符仍须是数字。`fieldNames()` 解析 no-cache／private 等指令的字段名列表，接受列表 OWS 和空项，返回小写名称。无参数由 value 的 None 表示，例如无界 max-stale 与不限定字段的 no-cache；调用者不能把它当作空列表或零秒。这两个解释方法缺少参数或遇到非法内容时报告 HttpFieldParseException，解析项的偏移映射回原字段（含 quoted-pair），普通构造项的偏移相对于参数字节。

`toByteArray()`／`toHttpField()` 生成可发送的 Cache-Control。标准秒数指令使用 token，no-cache／private 的字段列表使用 quoted-string；已解析的未知参数保留引号选择并正确转义。输出时检查已知的秒数、字段列表和无参数指令的参数形状，不把读取到的无效标准参数重新生成为合法配置。未知扩展只按通用字节语法处理。工具不判断请求／响应方向、freshness、存储或重验证，也不查询时钟。

### 1.6 ETag 与条件列表

`HttpEntityTag(opaque: List<Byte>, isWeak = false)` 保存不可变的 opaque 字节及强弱标志。opaque 可为空，接受 RFC 9110 etagc 的可见字节（双引号除外）与 obs-text，拒绝空格、控制字节及 DEL。parse／parseOrNone 接收带引号的单个字节或 String 字段及前后 OWS；弱前缀必须精确为 `W/`。反斜杠是普通 opaque 字节，不做 quoted-pair、百分号或字符集转换。`toByteArray()`／`toHttpField()` 原样包围 opaque，生成 ETag 字段；不通过 String 中转任意接收字节。

`stronglyMatches(other)` 只在两者均为强标签且 opaque 逐字节相等时成立；`weaklyMatches(other)` 只比较 opaque。标签值不做大小写归一、Unicode 规范化或摘要推断。

`HttpEntityTagCondition(tags)` 保存 If-Match／If-None-Match 的有序标签列表，允许空列表和重复项；`wildcard()` 单独表示星号条件，不能与列表混合。parse／parseOrNone 接收字节或 String，并可指定用于诊断的 fieldName（默认 If-None-Match）；按列表语法忽略 OWS 与空项，只在标签引号外识别逗号。isWildcard、tags 和 originalValue 暴露不可变结果，`toByteArray()`、`toIfMatchField()`、`toIfNoneMatchField()` 生成对应字段。引号内的星号仍是普通 ETag。工具不把无 ETag 等同于无当前表示，不自行判断条件是否满足；实际表示是否存在及所需比较强度由调用者决定。

## 2. Cookie 与公共后缀

Cookie parser 接收单个 Set-Cookie 字段，按 RFC 6265 及现代 SameSite／前缀规则返回名称、值、Domain、Path、Expires、Max-Age、Secure、HttpOnly、SameSite 和未知属性。Cookie 请求字段的组装独立处理，不把多个 Set-Cookie 用逗号连接。域名使用 url／idna 的同一规范值。

提供无状态的 domain-match、path-match、默认路径、过期和前缀检查。Max-Age 优先于 Expires，正负边界和日期溢出按 Cookie 规则处理。Domain 不能扩大到公共后缀；IP host 使用精确地址匹配。SameSite 的站点输入来自调用者，不从连接地址或 TLS SNI 猜测页面上下文。

公共后缀工具使用 Public Suffix List 的 ICANN 与 PRIVATE 部分，支持普通、通配与例外规则，返回 public suffix 与可选 registrable domain。数据以固定快照随 HTTP 库交付，记录来源、日期、revision 和 MPL-2.0 许可；维护时更新，不在请求中下载。域名预先完成 IDNA 规范化，匹配器不复制 Unicode 算法。

PSL 数据访问是普通库逻辑，不生成 compiler-known 表或额外产物验证机制。Cookie jar 的内存状态、持久化和请求发送选择属于 client；这里不维护全局 Cookie 状态。

## 3. 表单与 multipart

application/x-www-form-urlencoded 使用 URL 库的 FormUrlEncoded，保留重复项和顺序，按明确 charset 编码。HTTP 层提供 Content-Type 与 RequestBody 所需的元数据，不重新定义加号、百分号或空值规则。

multipart 内存引擎按 RFC 2046／7578 增量处理 boundary、每 part 字段与内容，不把 body 当成文本。提供已知长度计算、编码与解码；仅所有组成数据长度已知时才宣告完整 contentLength。boundary 可显式指定，自动生成使用 crypto 随机字节；不扫描无限上传源预先寻找碰撞。

异步编码／解码在 scoop.http.asyncio 组合 AsyncByteSource／AsyncByteSink，支持每 part 文件／字节／流、filename／filename*、重复字段和嵌套内容的原始交付。每个 part body 在推进下一 part 前必须完成或明确放弃；提前关闭整体消息关闭所拥有的响应，不能无界排空网络。header 和 boundary 跨任意输入块仍正确识别，正文出现部分匹配不得丢字节。

单 part 字段大小、part 数和 boundary 长度按实际局部限制配置，不建立通用内容预算。multipart 的重放能力由各 part 的 body factory 组成，只要有已消费且不能重建的 part，整体不能自动重放。

## 4. CONNECT 与切换辅助

提供 HTTP 代理 absolute-form 与 CONNECT authority-form 的正确请求构造，复用 URL 的目标、端口和 IPv6 表示。CONNECT 的成功判断与定界在 HTTP 层，普通拒绝响应仍按正常消息交付；client 决定认证、重试和目标 TLS。

成功 CONNECT、101 Upgrade 的异步移交始终包含全部未消费后缀以及 AsyncByteStream／AsyncCloseable 所有权。旧连接停止协议处理，关闭旧对象不关闭已移交传输。不得把失败响应的一部分 body 当成隧道数据。

## 5. SSE

`SseDecoder` 为增量内存 parser，负责 UTF-8、开头 BOM、CR／LF／CRLF、注释、空行分隔、data 多行连接、event、id 与十进制 retry。UTF-8 错误按 SSE 规定替换，跨块残缺序列保留到后续输入；最终 EOF 按标准处理未完成事件，不自行补空行。

`SseEvent` 保存 event、data、id 等实际字段；id 的空值与未出现不同，含 NUL 的 id 按协议忽略，非法 retry 不替换已有重连间隔。parser 不连接网络、不休眠、不维护自动重连任务。

异步 `SseSource` 借用或显式接管 AsyncByteSource，`next(): SseEvent?` 可挂起，消费驱动读取。maxLineBytes／maxEventBytes 约束实际累计文本，超出报具体错误。HTTP 状态、媒体类型、Last-Event-ID 和重连策略由 client 管理。

## 6. HTTP/1.1 WebSocket

握手工具生成和校验 RFC 6455 的 Upgrade、Connection、Sec-WebSocket-Key／Accept／Version、子协议与扩展字段，SHA-1 及随机数调用 crypto。成功响应必须绑定本次 key，不能只看到状态 101 就移交。响应选中的子协议／扩展必须属于本次有效提议。

帧引擎独立于 socket，处理 FIN／opcode／RSV、16／64 位长度、mask、分片和控制帧，正确检查最高长度位、控制帧大小、UTF-8 与 Close code。客户端发帧使用新的随机 mask；接收方向检查对端的 mask 角色。不能把单个大 frame 强制全量缓存；分片消息可分块交付，UTF-8 验证跨片连续。

`AsyncWebSocketSession` 位于 scoop.http.asyncio，显式接管升级传输与前缀。send／receive／ping／shutdownGracefully 为 suspend，close 为本地 AsyncCloseable 收尾；文本、二进制、分片与消息流使用明确的接口。每方向至多一个应用操作，内部控制帧按协议穿插；消息和内部队列都有实际有限容量及背压。

控制帧处理与用户回调分离。收到 Ping 能在应用发送过程中安排 Pong；Close 握手有独立超时。已接受的消息不能因取消而重发；异常中断发送会结束该 WebSocket 连接。close 可在取消的 finally 中完成，不等待无限期 peer Close，不自动重连。

permessage-deflate 显式开启，协商 client／server 的 no_context_takeover 与 max_window_bits；只有后端实际支持的组合才能接受。协议引擎负责 RSV1、压缩消息与控制帧的边界，异步适配调用 compression 的 RawDeflate 消息接口，不自行剥尾、构造同步 flush 或访问 zlib。接收大小限制按解压后的消息累计量判断，压缩后的长度不能替代它。

不实现 HTTP/2 Extended CONNECT、WebSocket over H2 或 HTTP/3。这些不是升级失败后的自动回退路线。

## 7. 验证与依据

所有字段 parser 有合法、非法、重复、quoted、非 ASCII 和分块向量。Cookie／PSL 覆盖通配与例外、国际域名、私有后缀、过期与前缀。multipart 覆盖 boundary 部分匹配、二进制内容、超限与提前关闭；SSE 覆盖 UTF-8、行尾和 EOF；WebSocket 与独立对端互操作，覆盖握手、mask、控制帧、分片、压缩、背压与取消。

这些库测试独立于 client 的策略测试；真实组合再覆盖 CONNECT 后缀、上传 multipart、SSE over H2 与 WebSocket over TLS。三个目标使用 debug／release／moving GC 和跨 Cone 消费者验证，不以文档存在表示已实现。

依据：[RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html)、[RFC 6265](https://www.rfc-editor.org/rfc/rfc6265.html)、[Public Suffix List](https://publicsuffix.org/list/)、[RFC 8187](https://www.rfc-editor.org/rfc/rfc8187.html)、[RFC 7578](https://www.rfc-editor.org/rfc/rfc7578.html)、[SSE 标准](https://html.spec.whatwg.org/multipage/server-sent-events.html)、[RFC 6455](https://www.rfc-editor.org/rfc/rfc6455.html)、[RFC 7692](https://www.rfc-editor.org/rfc/rfc7692.html)。
