# scoop.http.asyncio

普通 `AsyncByteStream`／`AsyncCloseable` 上的 HTTP 传输。调用者先完成 TCP 连接、TLS 握手和 ALPN 选择，再通过 `takeOwnershipAsClient`／`takeOwnershipAsServer` 交付传输和负责连接生命周期的 `CoroutineScope`。

`AsyncHttp1Connection.sendRequest`／`sendResponse` 返回 writer，显式选择 Empty、Length、Chunked 或响应 Eof 定界，用 `writeFrom` 写入实际前缀，最后调用 `finish`；即使空 body 也需要 finish。接收头与 `AsyncHttp1Body` 一同交付，body 完成后可取得独立 trailer。信息响应逐条返回；服务端按请求顺序关联响应，HEAD、CONNECT 与 Upgrade 由内存编解码器处理。

同一方向只允许一个操作。未完成的 body 不能被下一条消息替换；提前关闭或取消 body 会关闭连接，不自动 drain。`close` 等待底层 I/O 和本地状态结束，支持取消后的 finally、并发等待与后续幂等调用。输入等待不借用调用者 buffer，已接受的发送前缀在失败时不回滚。

接受 101／成功 CONNECT 后调用 `takeTransport`，得到先交付未消费后缀的 `HttpTransport`。旧连接已经释放协议状态，关闭旧对象不会再次关闭移交的传输。HTTP/1 方法受 llhttp 9.4.3 已知方法集合限制；具体接收选项、错误作用域和协议合同见 [HTTP 规范](../../../docs/specs/SCOOP-HTTP-SPEC.md)。

HTTP/2 异步连接继续实施。普通库不提供 TCP 连接、TLS 握手、连接池、自动重试或应用路由。
