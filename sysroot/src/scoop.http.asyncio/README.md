# scoop.http.asyncio

普通 `AsyncByteStream`／`AsyncCloseable` 上的 HTTP 传输。调用者先完成 TCP 连接、TLS 握手和 ALPN 选择，再通过 `takeOwnershipAsClient`／`takeOwnershipAsServer` 交付传输和负责连接生命周期的 `CoroutineScope`。

`AwsEventStreamSource(source, bufferSize = 8192)` 从已有 body 读取校验后的 AWS 二进制消息。默认借用输入，takeOwnership 显式接管；next 驱动读取，close 不排空剩余正文，并等待实际在途读取结束后释放资源。header 保留类型，payload 保留原始字节，业务事件由上层解释；见 [EventStream 接收规范](../../../docs/specs/SCOOP-HTTP-EVENTSTREAM-SPEC.md)。

`NdjsonSource(source, maxRecordBytes = 1048576, bufferSize = 8192)` 从已有 body 按需读取逐行 UTF-8 记录。next 返回原始字符串或 EOF，保留空行，不解析 JSON。默认借用，takeOwnership 成功后接管；关闭和取消复用相同的 MessageIo 寿命合同。详见 [NDJSON 接收规范](../../../docs/specs/SCOOP-HTTP-NDJSON-SPEC.md)。

`AsyncHttp1Connection.sendRequest`／`sendResponse` 返回 writer，显式选择 Empty、Length、Chunked 或响应 Eof 定界，用 `writeFrom` 写入实际前缀，最后调用 `finish`；即使空 body 也需要 finish。接收头与 `AsyncHttp1Body` 一同交付，body 完成后可取得独立 trailer。信息响应逐条返回；服务端按请求顺序关联响应，HEAD、CONNECT 与 Upgrade 由内存编解码器处理。

同一方向只允许一个操作。未完成的 body 不能被下一条消息替换；提前关闭或取消 body 会关闭连接，不自动 drain。`close` 等待底层 I/O 和本地状态结束，支持取消后的 finally、并发等待与后续幂等调用。输入等待不借用调用者 buffer，已接受的发送前缀在失败时不回滚。

接受 101／成功 CONNECT 后调用 `takeTransport`，得到先交付未消费后缀的 `HttpTransport`。旧连接已经释放协议状态，关闭旧对象不会再次关闭移交的传输。HTTP/1 客户端可发送任意合法方法 token；服务端请求解析仍受 llhttp 9.4.3 已知方法集合限制。具体接收选项、错误作用域和协议合同见 [HTTP 规范](../../../docs/specs/SCOOP-HTTP-SPEC.md)。

`AsyncHttp2Connection.openRequest` 返回按 stream ID 关联的 `AsyncHttp2Stream`，服务端用 `acceptRequest` 取得请求头和流。`receiveResponse` 逐条取走信息／最终响应，`sendResponse` 提交服务端响应。需要 body 时将 endStream 设为 false，使用 `writeFrom`、`finish`；`readInto` 消费实际 DATA 并归还窗口。每流方向分别串行，其他流仍可并行推进；`flush` 在窗口打开、已接受输出写完并完成底层 flush 后返回。

H2 传输读写使用连接所属 scope 的 Job。请求取消或流的 `close` 只重置该流，不取消其他流共享的 TLS I/O；scope 取消或连接 `close` 才收尾整条连接。正常关闭流中仍未取走的完整响应和 body 可在连接结束后读取。`onEvent` 在 native 返回后的锁外交付 SETTINGS、忽略字段、流关闭与 GOAWAY 等通知；回调不可挂起，异常会结束连接。

H2 的 `goAway` 显式发送协议通知，连接 `close` 负责本地关闭。h2c 完成原 HTTP/1 body 与 101 后，使用 `takeTransport` 和 upgraded 所有权工厂；`upgradedStream` 是关联的 stream 1，后续请求从 stream 3 开始。明文服务端可用 `HttpCleartextProbe` 判别，再把 probe.prefix 与 buffer 未消费部分交给 `HttpTransport.takeOwnershipWithPrefix`，交付选定的协议连接。

ALPN 使用已建立 TLS 的实际结果。Scoop 的 Option 类型不隐含转换其容器载荷，可按只读列表输入规则显式映射后选择：

```scoop
val alpn: List<Byte>? = when (tls.connectionInfo.alpn) {
    case Some(bytes) -> Some(bytes)
    case None -> None
}
val protocol = selectHttpAlpn(alpn, [HttpProtocol.Http1, HttpProtocol.Http2])
```

HTTP/1 WebSocket 先用 `WebSocketHandshake` 校验握手并完成原 HTTP body，再把 `takeTransport()` 的结果交给 `AsyncWebSocketSession.takeOwnership(transport, scope, role)`。会话直接消费切换后缀，使用自己的结构化任务持续处理输入与控制帧。传入本次握手的 `deflate` 协商结果可启用 permessage-deflate，双向窗口与上下文按实际角色选择，本地不支持的编码窗口在接管前拒绝。

`sendText`、`sendBinary` 与 `send(type, source)` 发送完整消息；source 为借用输入。流式发送使用 `beginMessage` 返回的 `WebSocketMessageWriter`，`writeFrom` 按配置的有限片段返回短写量，`finish` 显式结束消息，`flush` 不补 FIN。`receive` 返回 `WebSocketMessage`，它本身是异步字节源，`readBytes` 与 Text 的 `readText` 只汇集剩余内容。消息数据受有限背压约束，前一条未完成时不能开始同方向的下一条。

已协商压缩时，发送入口的 `compress = false` 可让单条消息保持普通格式，消息视图的 `isCompressed` 返回实际选择。压缩流的 `flush` 提交消息内同步点，让对端在 FIN 前读出已有原文；`finish` 完成压缩边界后再结束消息。接收按解压后的块交付，`maxDecompressedMessageBytes` 默认跟随线上消息上限，并独立检查解压累计量。压缩格式与同步点全部调用 `scoop.compression`，控制帧不经过压缩器。

WebSocket 的 `ping` 等待匹配的 Pong，`pong` 发送非请求式回应；自动 Pong 与应用发送独立调度，并保持当前输出帧完整。`shutdownGracefully` 等待双方 Close、flush 与资源收尾，使用配置的关闭期限；`close` 执行本地关闭，取消 owner I/O 后等待真实原生借用归还，支持已取消的 finally 与重复关闭。未完成消息的 close 会中止会话。具体状态、错误与并发合同见 [WebSocket 异步规范](../../../docs/specs/SCOOP-HTTP-WEBSOCKET-ASYNC-SPEC.md)。

普通库不提供 TCP 连接、TLS 握手、连接池、自动重试或应用路由。初始参数与活动 Job 检查失败时传输仍归调用者，接管后的失败由库完成本地收尾。
