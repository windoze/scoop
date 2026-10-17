# HTTP 协议库

`scoop:scoop.http:0.1.0` 是普通内存协议库。它不连接 socket、不进行 TLS 握手；传入／取出的字节使用既有 NativeByteBuffer，调用者串行操作同一实例并显式 close。公开合同见 [HTTP 规范](../../../docs/specs/SCOOP-HTTP-SPEC.md)，完成范围见 [M53 记录](../../../docs/milestone53/PROGRESS.md)。

HTTP/1 的 Http1Decoder.client／server 提供 receive、nextEvent、endInput。客户端先用 expectResponse 按顺序登记请求方法；HEAD、信息响应、CONNECT 和 101 才能正确关联。解析到切换边界后，buffer 中保留未消费后缀；服务端拒绝候选升级时可调用 resumeHttp1。

Http1Encoder 固定客户端／服务端角色，以 beginRequest／beginResponse、writeBody、finish 生成字节。body 定界显式选择 Empty、Length、Chunked 或响应 Eof；每个 begin 都需要 finish。mustClose 和 isSwitched 告诉调用者该连接的后续处理要求，编码器不隐含发送或关闭传输。

HttpField 保留有序的名称／值字节快照与 neverIndex，findAll 不合并重复项。文本转换由调用者显式选择，例如 String.fromUtf8；不解析下载文件名、字符集或百分号编码。

HTTP/2 的 Http2Session.client／server 使用 nghttp2 1.70.0，通过 receive、takeOutput 和 nextEvent 驱动。submitRequest 返回 stream ID；有 body 或 trailer 时显式 endStream=false，再调用 writeBody／finish。writeBody 允许短写或因待发送容量已满而返回零，此时需要推进输出及对端的窗口更新。应用实际读取或丢弃 DATA 后调用 consume，reset 则明确放弃该流未消费数据。

HTTP/2 事件分别交付初始头、信息响应、DATA、trailer、单向 End、完整 StreamClosed、GOAWAY、SETTINGS 与后端忽略的字段。请求伪头使用 HttpRequestTarget.Http2／Connect，never-index 标记使用 Http2RequestIndexing；响应没有 reason phrase。receive 兼容选项、实际本地限制和向对端声明的 SETTINGS 分开配置，发送字段保持严格校验。客户端声明禁用 push。

llhttp 固定为 9.4.3。HTTP/1 只支持它的已知方法集合，CUSTOM 等扩展 token 会得到明确的 HttpException；开启接收宽容也不改变这个限制。默认接受合法非 ASCII 字段值。legacyHeaderValues 使用上游较宽的 lenient_headers；不公开存在已知 NUL 不前进路径的 header_value_relaxed。宽容改变的实际定界行为必须按规范解释，不能据其名称推断更强保证。
