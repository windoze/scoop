# Scoop HTTP/1 WebSocket 异步会话规范

版本：M54，实施中。本文细化 [WebSocket 规范](SCOOP-HTTP-WEBSOCKET-SPEC.md)第 7 节。当前先实施未启用扩展的会话；permessage-deflate 的传输组合随后在同一 Cone 完成，实际状态见 [M54 进度](../milestone54/PROGRESS.md)。

## 1. 接管与配置

`AsyncWebSocketSession.takeOwnership(transport, scope, role, options = WebSocketSessionOptions())` 为 suspend 工厂，transport 同时实现 AsyncByteStream 与 AsyncCloseable。调用者先完成并验证 RFC 6455 握手，再移交唯一的传输关闭责任。HttpTransport 已保存 HTTP/1 的完整切换后缀，会话直接消费它，不重新解析 HTTP、URL 或握手字段。未协商扩展的会话拒绝 RSV1。

`WebSocketSessionOptions(incomingLimits = WebSocketFrameLimits(), inputBufferBytes = 8192, outgoingFrameBytes = 8192, closeTimeout = Duration.seconds(5))` 保存接收帧／消息／事件限制、原生输入容量、发送片段大小与关闭握手期限。两项容量必须为正，outgoingFrameBytes 加最大 14 字节头部不得溢出 Long；有限输出容量取 max(outgoingFrameBytes, 125) 加头部，以容纳完整控制帧。closeTimeout 必须为正，计时复用 EventLoop 的 Duration 合同。incomingLimits 针对接收数据，发送流不以它假装已知完整消息长度。

工厂要求当前 EventLoop、活动 Job 和未关闭 scope。参数、局部资源和任务注册未完成前，传输仍由调用者拥有；接管完成后，任何失败都由会话关闭。会话在给定 scope 下保留自己的结构化任务和私有子作用域，输入、输出及关闭计时器属于这个 owner，不把任意调用者 Job 当成长期传输 owner。外部 scope 的取消结束该会话。

role、isClosed、peerClose、sentClose 与 failure 保存实际角色、资源终态、双方 Close 数据和失败原因。每个方向的应用操作串行使用，同方向并发误用立即报状态错误；接收与发送可以同时进行。状态查询取得一致快照，所有实际 I/O 要求原 EventLoop。关闭后的本地重复 close 不要求重新激活已经取消的 Job。

## 2. 发送消息

`beginMessage(type): WebSocketMessageWriter` 为 suspend 操作，type 使用 HTTP 的 WebSocketMessageType.Text／Binary。当前消息未 finish 或 close 时不能开始另一条消息。writer 实现 AsyncByteSink 与 AsyncCloseable，提供 type、isComplete、isClosed 及显式 `finish()`。

writeFrom 按实际输入返回短写量，最多接收 outgoingFrameBytes；每个非空调用编码一个有限的非最终数据帧，后续调用使用 continuation。每帧完整交给传输并 flush 后才返回；finish 输出最后一个 FIN 帧，空消息直接使用带 FIN 的 Text／Binary。文本校验跨片持续，finish 拒绝不完整 UTF-8。flush 只推进已有传输，不结束消息或补 FIN。

`sendText(text)`、`sendBinary(bytes)` 与 `send(type, source)` 是同一 writer 的完整发送便利入口，使用有限缓冲。字节列表先取得不可变快照；source 为借用的 AsyncByteSource，不由会话关闭。便利入口从开始到 finish 占用同一应用发送方向，真实等待仍使用 suspend；source 的失败或取消中止本次消息和 WebSocket。

非法 count、可用范围、消息身份和并发参数在接受输入前检查，不关闭健康连接。实际编码／传输开始后的失败或取消保留调用者 NativeByteBuffer 已被帧引擎读取的进度，不回滚或重发。finish 才表示消息成功结束；未完成 writer 的 close 中止整个 WebSocket，完成后只关闭这个 writer 视图。

## 3. 接收消息与背压

`receive(): WebSocketMessage?` 挂起等待下一条 Text／Binary 消息，控制帧由会话处理。WebSocketMessage 本身实现 AsyncByteSource 与 AsyncCloseable，提供 type、isComplete、isClosed、`readBytes()` 和只适用于 Text 的 `readText()`。正文结束由 FIN 和完整 UTF-8 校验决定；分片边界不变成消息 EOF。readBytes／readText 消费当前消息剩余内容，前者只在调用者显式要求时汇集完整剩余字节。

前一消息尚未完成或明确关闭时，不能领取下一条消息。消息对象关闭后不可再读；完成的消息不再持有取消监听。未完成消息的 close 中止整个 WebSocket，不无限排空网络。收到合法 Close 中断分片消息时，正文报告 InterruptedMessage，不伪造完整结束。

会话最多保留一条尚未领取或消费的消息、一个 Data 块和有限的原生输入。大 frame 按 incomingLimits.dataChunkBytes 交付，不按广告长度分配整帧。没有应用读取时仍可处理前方的控制帧；控制帧位于尚未消费的数据之后时遵守线上顺序和背压，不通过无限预读绕过正文。复合读取便利入口也占用同一应用接收方向。

收到双方已完成关闭握手且资源已释放的终态后，receive 返回 None；异常 EOF、协议／I/O 错误或取消抛出实际原因。合法 Close 的 code 和 reason 始终可从 peerClose 取得，应用自行判断业务关闭原因。

## 4. Ping、Pong 与发送调度

`ping(payload = 空字节列表)` 发送 Ping 并等待相同 payload 的 Pong，`pong(payload = 空字节列表)` 发送非请求式 Pong；两者均为 suspend，负载最多 125 字节，不参与文本校验或压缩。ping 从发送到响应占用应用发送方向，因此同时最多有一条待匹配 Ping；调用者通过取消其 Job 限制等待。不能用不匹配的 Pong 完成本次 Ping，也不能在 Ping 尚未完成传输时提前报告成功。

自动 Pong 与应用发送方向分离。收到 Ping 后，在当前完整帧之后安排相同 payload 的 Pong；多个尚未发送的自动回应可以按 RFC 6455 合并为最新一项。不会把 Pong 插入一个帧的头部或正文中。调度最多持有当前有限输出帧、一个应用发送请求和一个自动 Pong，不建立无限消息队列。

Close 优先于尚未开始的普通数据帧，其次为自动 Pong，再推进应用数据或显式控制帧。已开始写出的帧必须保持完整，不能截掉尾部后拼接一个 Close。发生传输失败或取消时直接中止连接。无需执行用户回调就能处理 Ping／Pong／Close，回调及自动重连策略属于 client。

## 5. 关闭与失败

`shutdownGracefully(value = WebSocketClose(Some(1000)))` 启动关闭握手，等待双方 Close、自己的 Close 已 flush 和本地资源关闭，返回实际 peerClose。并发调用等待同一次握手，保留首次选择的发送 Close。收到 peer Close 后自动回应；关闭握手开始后拒绝新消息和新应用数据，尚未消费的正文可按有限块丢弃以推进到 Close，不能要求用户继续读正文才使关闭完成。

closeTimeout 从握手开始计时，对主动与被动关闭都有效；超时报告 WebSocketSessionException 的 CloseTimeout 并中止传输。Close 本身不能压缩、分片或超过 125 字节。接收到帧协议错误时，在能够保持当前输出帧完整的前提下，使用对应的 1002／1007／1009 关闭码进行有限的失败关闭；发送编码器或传输已经失败时直接中止。

`close()` 是本地 AsyncCloseable 收尾，不发送新的协议 Close、不等待 peer、不把未完成消息标为成功。它可在已取消的 finally 中调用。关闭先停止新工作并取消 owner 的待定 I/O，等待真实 read／write／flush 结束与 NativeByteBorrow 归还，再关闭所拥有的 transport 并释放帧引擎和原生缓冲。取消请求不等于实际完成，不能提前释放仍被借用的数据。

所有并发 close 等待同一结果，重复 close 重用已保存的清理失败；发生主失败及清理失败时保留标准 ResourceUseException 的两个原因。输入结束且没有 peer Close 是 UnexpectedEof，保留底层截断帧原因，不能作为正常 WebSocket 关闭。

`WebSocketSessionException(kind, detail, peerClose = None, cause = None)` 的 kind 使用 Closed、UnexpectedEof、InterruptedMessage、CloseTimeout。FrameException 继续保留协议 closeCode；普通传输原因和 CancellationException 不被无信息的状态异常替代。failure 只保存实际失败，正常握手或显式本地 close 不凭空制造失败。

## 6. 验证

库测试覆盖短读写、双向并发、同向拒绝、跨帧 UTF-8、空消息、单个大 frame 的有限交付、应用发送期间的自动 Pong、匹配 Ping、传输／flush 失败、错误进度、未完成 body 关闭、异常 EOF、握手超时、取消与真实借用延迟完成、重复清理失败。

正式消费者组合真实 HTTP/1 升级、同次读取的首帧后缀、移动 GC 与缓存复用，并与独立 WebSocket 对端收发分片和控制帧。压缩与 WSS 在其各自实现完成后补充真实组合；HTTP/2 WebSocket 与 HTTP/3 不在本期。
