# Scoop 异步 I/O 库规范

版本：M49。本文规定普通 Cone `scoop:scoop.asyncio:0.1.0` 的合同；实现及三目标验证见 [M49 验收](../milestone49/ACCEPTANCE.md)。

## 1. 分层与目标

源码位于 `sysroot/lib/scoop.asyncio/`，显式依赖 core、std、platform，测试依赖 scoop.test；公共声明属于 `scoop.asyncio`。不默认导入，不引入 compiler-known role、runtime 调度入口或新的 ABI。任务树使用语言规范 11.9.1～11.9.2；路径、地址、错误、时间和 buffer 直接复用平台库与标准库。

首版支持 aarch64-apple-darwin、x86_64-unknown-linux-gnu、x86_64-unknown-linux-musl。LibuvEventLoop 使用固定的 libuv 1.53.0，源码、许可证和各目标 native 输入随 Cone 发布；不查找系统 libuv。C11 shim 只管理 native I/O、稳定存储及完成记录，worker／callback 不进入 managed 代码。TLS、HTTP、异步目录遍历、文件监听、进程／TTY 和任意 fd 接管不在本版范围。

Linux C 工具链同时需要所选 libc 的开发头文件和对应架构的 Linux UAPI 头文件（包括 linux/errqueue.h）。只带 musl libc 的 musl-gcc 包装器须补齐内核头文件搜索路径，可通过普通 --cc 包装脚本配置；不把 glibc 用户态头文件加入 musl 的搜索路径。

## 2. 根入口与事件循环

```scoop
public interface EventLoop {
    public fun run(block: suspend (CoroutineScope) -> Unit)
    public suspend fun sleep(duration: Duration)
    public suspend fun resolve(host: OsString, port: Int, family: AddressFamily? = None): Array<SocketAddress>
    public suspend fun connectTcp(address: SocketAddress): AsyncTcpStream
    public suspend fun listenTcp(address: SocketAddress, backlog: Int = 128, reuseAddress: Boolean = false): AsyncTcpListener
    public suspend fun bindUdp(address: SocketAddress, reuseAddress: Boolean = false): AsyncUdpSocket
    public suspend fun openFile(path: Path, options: OpenOptions = OpenOptions()): AsyncFile
}
public class LibuvEventLoop public constructor() : EventLoop
public fun <T> EventLoop.runWithResult(block: suspend (CoroutineScope) -> T): T
public fun currentEventLoop(): EventLoop
```

`run` 是同步根入口，调用方不得已有当前 Job 或活动 EventLoop binding。实例只接受一次运行；并发、重入、运行后重用均报 IllegalStateException，拒绝的调用不影响已接受运行。构造不分配 native loop，接受运行后才初始化；初始化失败清理已创建资源并关闭实例。

运行建立精确静态类型为 EventLoop 的 Context binding，再通过 startCoroutine 与 coroutineScope 启动正文。正文第一次执行前已具有正确 Job 与 scope。根及全部后代完成后继续关闭本次未关闭资源、排空操作及关闭 callback，最后关闭唤醒 handle 和 native loop。runWithResult 只在 run 成功后返回类型化结果；非取消异常优先于取消，正文与清理均失败按 ResourceUseException 组合。逃逸资源引用不能延长运行，run 返回后它们已经关闭。

每次用户 I/O 要求当前 Job 和所属 loop 的活动 binding；显式调用其他 loop 或已结束资源报状态错误。资源固定归属创建它的 loop。close 允许在已取消 Job 中进行，且关闭后再次 close 是空操作。`currentEventLoop` 缺少 binding 时抛 MissingContextException。

实际 native 操作仅在 run 所属线程执行。外线程提交和取消先进入受锁保护的命令队列，在同一锁内完成 uv_async_send；关闭在同一边界停止接受投递。驱动交替处理命令、NativeSafe uv_run、完成记录与一批 managed 恢复，内部锁外恢复 continuation；有待处理工作时不得阻塞。IO 挂起的恢复由所属 loop 投递；第三方挂起点仍可在其他线程恢复。uv_async 通知可合并，队列承担完整性。

## 3. 取消、完成与关闭

直接 I/O 属于提交时的当前 Job，不为每次操作创建子任务。登记前已取消则不提交。操作完成与取消在 managed 状态中选择一次结果；native 结束是独立事实。借用解除、实际进度提交及未交付资源清理完成后才恢复，包括取消结果。成功先选定则后来的取消不覆盖成功；取消先选定则迟到成功只执行清理。libuv 撤销返回值不替代真实 callback。

定时器取消等待 timer close callback。TCP read／UDP receive 停止订阅后由 shim 记录终止；已提交 request 等实际 callback。单独取消 TCP read／write、UDP receive 不关闭健康资源。TCP write 使用 1.53.0 的 uv_cancel 和 uv_write_nwritten 保留部分进度。shutdownOutput 提交后的取消终止整个 stream。UDP send 提交后等 callback；文件／DNS work 队列可撤销，已执行 syscall 必须等 after-work。迟到 open／connect／accept 成功先关闭未交付资源。

close 拒绝新操作，将未选定结果的在途操作以 IoClosedException 结束；先获胜的 Job 取消不被覆盖。close 不隐含 flush、sync 或对端确认，等待所有操作、借用和 native close callback 后完成；并发 close 等同一次收尾，后续重复 close 不重报旧错误。文件须先等活动 worker，再执行一次 raw close，不能重试已消费 fd。EventLoop 收尾继续关闭所有资源，即使某项失败。

```scoop
public class IoClosedException public constructor() : Exception
public interface AsyncCloseable { public suspend fun close() }
public suspend fun <R, T> R.useAsync(block: suspend (R) -> T): T where R : AsyncCloseable
```

useAsync 与 core use 一样在正文与清理均失败时抛 ResourceUseException。同步对应协议为 core Closeable.close；异步关闭继续使用独立 AsyncCloseable，不通过同步 Disposable 表达。GC hook 不承担驱动、取消、恢复、flush 或等待；正常生命周期由 close 与 run 收尾保证。

## 4. 流、buffer 与组合

```scoop
public interface AsyncByteSource {
    public suspend fun readInto(buffer: NativeByteBuffer, count: Long): Long
}
public interface AsyncByteSink {
    public suspend fun writeFrom(buffer: NativeByteBuffer, count: Long): Long
    public suspend fun flush()
}
public interface AsyncByteStream : AsyncByteSource, AsyncByteSink {}
public suspend fun AsyncByteSource.readInto(buffer: NativeByteBuffer): Long
public suspend fun AsyncByteSink.writeFrom(buffer: NativeByteBuffer): Long
public suspend fun AsyncByteSource.readFully(buffer: NativeByteBuffer, count: Long = buffer.writableBytes)
public suspend fun AsyncByteSink.writeFully(buffer: NativeByteBuffer, count: Long = buffer.readableBytes)
public suspend fun AsyncByteSource.copyTo(destination: AsyncByteSink, bufferSize: Long = 8192L): Long
```

count 非负且不超过 buffer 当前可用区间。零长度字节流请求在状态及取消检查后返回零，不执行 native I/O、不表示 EOF。正 read 允许短读，零表示 EOF；正 write 允许短写，零抛平台 WriteZeroException。readFully 不足抛平台 UnexpectedEofException；各扩展保留已提交进度，不回滚或重发。copyTo 不接管任一端关闭责任，不自动 flush。

省略 count 的基本操作扩展取 buffer 当前可用长度后转发。readFully／writeFully 先检查取消与 buffer 范围，零长度不调用端点；copyTo 要求 bufferSize 为正，内部 buffer 在退出时释放。组合扩展检查端点返回的进度在本次请求范围内，不把非法返回值当作正常结束。

read 推进 writer，write 推进 reader，返回前解除平台规范 4.5 的 NativeByteBorrow。取消／关闭也提交已知实际前缀。单次 native 调用选择后端可表示的合法前缀，不溢出截断。借用期间拒绝释放、扩容、清空、游标变更及重复借用；native 只保存稳定 allocation 指针，managed 表保活 owner。路径、DNS 等 managed 字节在提交 worker 前复制到请求自有 native 存储。

AsyncByteStream 允许使用不同 buffer 各一个 read 和 write 同时在途，同方向并发报状态错误。flush 负责本层已接受的积压输出向下游推进，不表示持久化；无用户态缓存的原始 TCP／文件仅检查状态及输出方向空闲。封装层可拥有自己的缓冲及关闭责任，接口不要求 fd 或 TCP 能力；可实现需要双向进展的后续 TLS stream。

## 5. 定时等待

sleep 使用 Duration，负值非法，零在取消检查后立即完成。正时长按单调截止时间等待；更新 libuv 缓存时间、向上舍入毫秒并在 callback 核对截止时间，不提前报告完成。不承诺精确唤醒时刻。取消停止并关闭 timer，收齐 native 关闭后恢复。

## 6. 网络

```scoop
public interface AsyncTcpStream : AsyncByteStream, AsyncCloseable {
    public val localAddress: SocketAddress
    public val peerAddress: SocketAddress
    public suspend fun shutdownOutput()
}
public interface AsyncTcpListener : AsyncCloseable {
    public val localAddress: SocketAddress
    public suspend fun accept(): AsyncTcpStream
}
public interface AsyncUdpSocket : AsyncCloseable {
    public val localAddress: SocketAddress
    public suspend fun sendTo(buffer: NativeByteBuffer, destination: SocketAddress, count: Long): Long
    public suspend fun receiveFrom(buffer: NativeByteBuffer, count: Long): Datagram
}
```

TCP EOF 不关闭输出。shutdownOutput 属于输出方向，提交后禁止新 write；成功后输入仍可读。真实读错误成为该方向终态，不重启失效的读订阅。listener 同时一个 accept，无等待者时保留后端待 accept 状态，不建立 managed 预接收队列。关闭 listener 不关闭已交付 stream。地址属性是创建时取得的值，不跨线程访问 native handle；这与 platform 同步 socket 每次调用 localAddress()／peerAddress() 查询 OS 状态的合同不同。

listenTcp 的 backlog 必须为正。监听使用 platform 的 socket_bind helper 先按 reuseAddress 配置和绑定新的 owned fd，再移交 uv_tcp_open／uv_listen；这样 false 不被 libuv 的隐式 SO_REUSEADDR 覆盖。移交前失败按 raw close 清理，移交后按 uv_close 清理。helper 错误保留 PosixErrno，libuv 错误使用 Libuv；不接管用户已有 socket。

UDP 每方向一个在途操作，发送保持完整 datagram，不拆包；空发送是真实空包。receive 要求正容量，有源地址的零字节是空包，无地址的零通知继续等待。Datagram 保留来源、实际交付长度及截断标志。超大报文保留实际错误。

bindUdp 同样先由 platform socket_bind 配置 reuseAddress 并绑定新的 fd，再通过 uv_udp_open_ex(handle, fd, 0) 移交；不使用隐式开启 SO_REUSEADDR 的 uv_udp_open。移交前后失败的关闭责任与 TCP 相同。sendTo 提交单个 uv_udp_send_t，成功 callback 表示完整报文已交给 OS；取消仍等待 callback 并提交实际发送长度，不能把取消当作撤回报文。

libuv 错误保留 NativeErrorDomain.Libuv 与原码；UV_EOF 按 EOF 解释，库主动取消／关闭不伪造 OS 错误。Linux 在实际 socket 输出提交及 uv_run 周围屏蔽线程 SIGPIPE，仅消费本段新增信号，再恢复旧 mask/pending；Darwin 输出前设置 SO_NOSIGPIPE。不改进程 signal handler。

## 7. DNS 与文件

resolve 保留 OsString 原始名称字节，port 检查 0..65535，family None 表示双族。使用 uv_queue_work 调用 platform 的 getaddrinfo helper，保留 AddressResolution code 和 EAI_SYSTEM 的立即 errno；不执行额外 IDNA 转换。无效内嵌 NUL 在提交 worker 前报参数错误。

```scoop
public interface AsyncFile : AsyncByteSource, AsyncByteSink, AsyncCloseable {
    public suspend fun syncData()
    public suspend fun syncAll()
}
```

openFile 使用 Path／OpenOptions 的已有合同。文件 open／read／write／sync／close 在 libuv worker 调用 platform 的 GC-free helper，保留 PosixErrno。顺序 read／write 共用文件位置，同一文件的 read／write／sync 同时最多一个。flush 不等于 sync；不使用 uv_fs 的额外 close／sync 语义。worker 只接收 native 存储，立即记录错误。已开始的阻塞操作可能推迟取消和 run 退出，不承诺固定时延。

## 8. 实现边界与验收

native request 持有预留完成节点；C callback 仅存状态和进度、入 native 队列，managed 在 NativeSafe 返回握手后消费。操作编号在完成消费前不复用。managed 使用普通带类型操作与结果，不以可空字段猜测不同事件。提交同步失败走同一释放路径。libuv 及相关系统库按语言规范 12.2.1～12.2.3 的普通 native 输入链接；三目标不启用 io_uring。

主要验收为同 Cone scoop.test：真实 loopback 动态端口、无公网 DNS、真实文件、Context、取消／关闭竞争、部分进度、原始错误、SIGPIPE 及 moving GC。竞争用明确同步点，测试源码和工具始终存在。详细边界见 [M49 设计第 14 节](../milestone49/DESIGN.md#142-异步-io)。
