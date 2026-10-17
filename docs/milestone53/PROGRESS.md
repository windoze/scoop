# M53 实施记录

HTTP 后端与生命周期合同见 [HTTP 规范](../specs/SCOOP-HTTP-SPEC.md)。M47 延后；准备及打包继续只使用仓库维护脚本。原始日志、缓存和生成数据位于外置盘仓库已忽略的 `tmp/m53`，不提交。

## 后端源码与目标资源

- 固定 llhttp 9.4.3 的发布 C 源码、nghttp2 1.70.0 官方 tag 归档与许可证；不修改上游状态机或做函数级裁剪。nghttp2 用维护脚本构建纯静态核心，排除应用、工具、示例、测试和文档。
- 沿用已有 native 静态库解析，将 nghttp2 目标目录加入默认库搜索及手工打包清单。缓存先检查实际输入更新；命中不解包、不运行 CMake、不读回 archive。
- Darwin 实际构建与重复调用直接复用通过；Rust 格式化、toolchain lint、Python Ruff 及自身 C 代码的格式／严格编译检查通过。toolchain 单测 32 项通过；默认库根、OpenSSL 子目录和新增 nghttp2 子目录的实际更新／覆盖／两个 link 入口共 3 个 fixture 变体通过。
- 独立 native HTTP 消费者在 debug／release 与 moving GC 下通过，覆盖逐字节 llhttp chunk／trailer、已知方法限制、nghttp2 客户端／服务端 SETTINGS、逐字节帧与 HPACK 输入、请求／响应及重复字段，重复构建无 child 编译调用。首次运行遇到的上游未使用参数警告及 fixture Unsafe 块语法已修正并复验。
- GNU／musl 的后端构建和 HTTP 消费验证在后续功能批次统一执行；M52 的 Linux 全量回归已闭合，结果见 [M52 验收](../milestone52/PROGRESS.md)。已结束的 GNU 全量生成目录清理释放约 275.9 GiB，保留报告。

## HTTP/1 内存编解码

- 新增普通 `scoop.http` Cone：有序字节字段与异常上下文、客户端／服务端增量解码、请求方法关联、信息响应、HEAD／CONNECT／Upgrade、chunked／定长／EOF body、独立 trailer，以及严格发送编码器。字段与 body 在 native 返回后复制交付，切换保留未消费后缀。
- 覆盖全部公开 llhttp 接收选项及正反边界，已知方法限制在收发两侧明确诊断。记录该版本对非最终 chunked Transfer-Encoding 的实际 EOF 行为，不另加解析器补偿。
- Darwin 的 24 项库测试在 debug／release 与 moving GC 下全部通过。生产 `.slib` 的独立消费者两个变体通过，包含跨 Cone 异常、泛型持有字段、显式 GC、逐字节输入、body／trailer 和 warm 构建零 child 调用。Rust 格式／lint、自身 C 格式及严格编译检查通过。
- HTTP/2 会话、协商、异步连接和 GNU／musl 集成仍待后续批次完成。

## HTTP/2 内存会话

- 新增 nghttp2 内存会话、完整字段事件、请求／响应与双向 body、信息响应、trailer、RST_STREAM、GOAWAY 和 SETTINGS。普通字段与伪头均保留 never-index 标记；输入字段和输出片段跨 native 调用时各自拥有稳定存储。
- 每流待发送容量、对端流窗口和连接窗口分别决定进度；消费或明确 reset 时归还实际窗口，已关闭流仍有未消费 body 时保留必要记录。回调不调用 Scoop、不挂起、不保存 managed 引用。
- 17 项 HTTP/2 专项测试在 release 与 moving GC 下通过；包含 24 项既有 HTTP/1 测试的 debug 全部 41 项在 moving GC 下通过。覆盖逐字节 HPACK、重复／非 UTF-8 字段、并发流、HEAD／CONNECT、背压、关闭流的窗口归还、reset、接收选项、字段限制、SETTINGS／HPACK／CONTINUATION 连接错误和发送失败作用域。
- 生产 `.slib` 外部消费者与独立的原生 nghttp2 客户端／服务端双向互操作，在 debug／release、三字节传输、moving GC 下通过；cold 只编译用户 Cone，warm 零 child 调用。自身 C 代码严格编译、Rust 格式及 lint 通过。首次 Scoop 编译中的构造器访问、Unsafe 边界和不可变捕获问题已修正，后续库与外部消费者均复验通过。
- 为异步 flush 增加实际待输出查询，包含尚未序列化的字段块、受窗口限制的 DATA、结束标记和部分交付片段。18 项 HTTP/2 内存专项测试在 release／moving GC 下通过，覆盖零窗口时 wantsWrite 与实际 body 待发送状态的区别；C 严格编译与 Rust 格式／lint 通过。
- 协商与异步生命周期尚未完成，三目标 HTTP 集成留到这些功能就绪后统一验证。

## 协商与 h2c

- ALPN 按实际字节与显式允许列表选择；无协商的 HTTP/1 回退必须显式允许。明文前言支持逐字节判别，已消费前缀和 buffer 后缀均保留，截断或不允许的协议给出协商错误。
- h2c 准备／读取无 padding base64url SETTINGS，处理接受与拒绝响应。两个升级工厂使用 nghttp2_session_upgrade2 关联 stream 1，保留 HEAD 语义，不重复交付已完成的 HTTP/1 请求／body；101 与后续 H2 SETTINGS 同次输入时后缀完整交付。
- 本机 debug 与 moving GC 的全部 49 项测试通过；release 首轮 48 项通过，唯一预期差异修正后 4 项升级测试复验通过。固定后端对非法 SETTINGS 数值排队产生 GOAWAY 的行为已写入规范，格式错误继续直接诊断。Rust 格式／lint、自身 C 严格编译通过。
- 生产 `.slib` 消费者的 debug／release、moving GC、三字节 H2 传输、协商异常、前缀保留和 h2c stream 1／3 关联通过，重复构建零 child 调用。异步传输与三目标 HTTP 验收继续实施。

## HTTP/1 异步传输

- 新增普通 `scoop.http.asyncio` Cone。连接登记为 scope 子任务，接管已有字节流；按方向串行执行头、body 与 trailer 收发，保留短传输的实际前缀，支持信息响应、HEAD、EOF body、流水线与拒绝升级后继续 HTTP/1。
- 未完成 body 的关闭或取消关闭连接，不自动 drain。实际 I/O 结束后释放 native buffer；并发 close 等待同一次清理，正文与清理同时失败保留两个异常。接受 CONNECT／101 后可移交传输及已读取后缀，旧连接不再关闭新拥有者。
- 本机 debug 的首批 11 项测试及新增服务端移交用例通过；release 的全部 12 项在 moving GC 下通过。真实 loopback 覆盖三字节读／五字节写、跨线程取消、等待期间不借用调用者 buffer、并发关闭、提前拒绝升级、前缀保留与异常组合。初次语法推断和负例异常类型差异已修正并复验。
- 生产 `.slib` 的独立消费者在 debug／release、moving GC 下完成真实 HTTP 通信，cold 只编译用户 Cone、warm 零 child 调用。Rust 格式与 lint 通过。HTTP/2 异步多流及三目标验收继续实施。

## HTTP/2 异步多流与传输组合

- 新增客户端／服务端及 h2c 升级所有权工厂、按流关联的双向 body、信息／最终响应、trailer、流级 flush、RST_STREAM 和独立 GOAWAY。实际复制 body 后归还窗口；流窗口为零时继续推进共享输入，控制帧可独立完成发送。
- 共享传输使用连接 scope 的 Job，单流取消只重置对应流。连接关闭等待实际读写与底层清理，正常结束流中已完成的头和 body 可继续读取；native 批量输入中先完成的响应不因后续连接错误丢失。通知在锁外交付，异常结束连接；等待与关闭均解除不再需要的 Job 监听和 continuation。
- 本机 release 全部 26 项异步库测试在 moving GC 下通过；debug 首轮的 22 项通过及后续 11 项 HTTP/2 复验覆盖全部 26 项。首次发现的 reset 诊断被关闭状态覆盖，以及测试取消线程的自 join 问题均已修复闭合。内存 HTTP 库 debug 全部 50 项通过，Rust 格式／lint 与自身 C 严格编译通过。
- 真实 loopback 覆盖多流乱序响应、双向背压、短传输、零长度、队列限制、取消／并发关闭、原生协议错误、完整前缀保留、明文判别、h2c stream 1／3、TLS H2／HTTP/1.1 ALPN 及无 ALPN 的显式回退。TLS 上跨线程取消单流后，另一流继续收发且底层连接保持可用。
- 生产 `.slib` 外部消费者在 debug／release、moving GC 下通过，发送 70,000 字节 body 穿过默认流控窗口，验证乱序响应和 trailer；cold 只编译用户 Cone、warm 零 child 调用，共 2 个变体、6 个进程。GNU／musl 的全部 HTTP 库和外部消费者验证接续执行。
