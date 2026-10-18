# Scoop HTTP 消息工具规范

版本：M54 设计，尚未实施。本文补充 [HTTP 规范](SCOOP-HTTP-SPEC.md)第 9 节，所有能力在 `scoop.http` 或 `scoop.http.asyncio` 中交付，不在 web client 内另建协议实现。

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
