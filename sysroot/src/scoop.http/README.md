# HTTP 协议库

`scoop:scoop.http:0.1.0` 是普通内存协议库。它不连接 socket、不进行 TLS 握手；传入／取出的字节使用既有 NativeByteBuffer，调用者串行操作同一实例并显式 close。公开合同见 [HTTP 规范](../../../docs/specs/SCOOP-HTTP-SPEC.md)，完成范围见 [M53 记录](../../../docs/milestone53/PROGRESS.md)。

HTTP/1 的 Http1Decoder.client／server 提供 receive、nextEvent、endInput。客户端先用 expectResponse 按顺序登记请求方法；HEAD、信息响应、CONNECT 和 101 才能正确关联。解析到切换边界后，buffer 中保留未消费后缀；服务端拒绝候选升级时可调用 resumeHttp1。

Http1Encoder 固定客户端／服务端角色，以 beginRequest／beginResponse、writeBody、finish 生成字节。body 定界显式选择 Empty、Length、Chunked 或响应 Eof；每个 begin 都需要 finish。mustClose 和 isSwitched 告诉调用者该连接的后续处理要求，编码器不隐含发送或关闭传输。

HttpField 保留有序的名称／值字节快照与 neverIndex，findAll 不合并重复项。文本转换由调用者显式选择，例如 String.fromUtf8；不解析下载文件名、字符集或百分号编码。

llhttp 固定为 9.4.3。HTTP/1 只支持它的已知方法集合，CUSTOM 等扩展 token 会得到明确的 HttpException；开启接收宽容也不改变这个限制。默认接受合法非 ASCII 字段值。legacyHeaderValues 使用上游较宽的 lenient_headers；不公开存在已知 NUL 不前进路径的 header_value_relaxed。宽容改变的实际定界行为必须按规范解释，不能据其名称推断更强保证。
