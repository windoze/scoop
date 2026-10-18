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

## 3. 帧与异步边界

帧、消息、控制帧、UTF-8、关闭握手及 permessage-deflate 的传输合同继续遵循 [消息工具规范第 6 节](SCOOP-HTTP-MESSAGE-SPEC.md#6-http11-websocket)。帧引擎与异步 session 按各自实现进一步细化公共 API 和验收；握手工具完成不代表传输功能已完成。HTTP/2 WebSocket、Extended CONNECT 和 HTTP/3 不属于本期。

## 4. 验证

握手覆盖 RFC 6455 固定 Accept、独立随机 key、URL／代理 target、重复 singleton、错误状态／版本、大小写规则、协议匹配、扩展 quoted-string、多行提议及 RFC 7692 的非对称参数；与独立 WebSocket 实现验证两个握手方向。正式 .slib 消费者组合真实 HTTP/1 升级与同次读取的后缀、moving GC 和正常缓存复用。

依据：[RFC 6455](https://www.rfc-editor.org/rfc/rfc6455.html) §4、§9.1；[RFC 7692](https://www.rfc-editor.org/rfc/rfc7692.html) §7.1、§7.2。
