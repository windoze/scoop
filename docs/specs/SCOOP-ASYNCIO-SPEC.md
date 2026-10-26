# Scoop 异步 I/O 库规范

版本：M49／M54／M59。本文规定普通 Cone `scoop:scoop.asyncio:0.1.0` 的合同；既有能力三目标验证见 [M49 验收](../milestone49/ACCEPTANCE.md)，第 9 节扩展见 [M54 实施记录](../milestone54/PROGRESS.md)，第 10 节 TTY 见 [M59 实施记录](../milestone59/PROGRESS.md)。

状态说明：平台后端迁移已开始，第 11 节与配套设计是本次实现合同。第 1～10 节的公共语义继续适用，具体 libuv 实现描述随各批迁移同步替换；新 API 与后端的可用状态见 [实施记录](../asyncio-backends/PROGRESS.md)。详细拆分、平台映射与验收见 [EventLoop 后端设计](../asyncio-backends/DESIGN.md)。

## 1. 分层与目标

源码位于 `sysroot/src/scoop.asyncio/`，显式依赖 core、std、platform，测试依赖 scoop.test；公共声明属于 `scoop.asyncio`。不默认导入，不引入 compiler-known role、runtime 调度入口或新的 ABI。任务树使用语言规范 11.9.1～11.9.2；路径、地址、错误、时间和 buffer 直接复用平台库与标准库。

库内 C shim 的完成记录 kind 与 Scoop 解码保持一致：既有通用／网络事件保留原编号，使用 `0x000–0x0ff` 段；文件／目录事件使用 `0x100–0x1ff` 段，依次为 Metadata `0x100`、Position `0x101`、Temporary `0x102`、DirectoryName `0x103`；TTY 事件使用 `0x200–0x2ff` 段，Resize 为 `0x200`、Timeout 为 `0x201`。两类新增事件分别在自己的段内追加，保留已有编号；未分配值仍按非法完成记录处理。此编号仅用于该 Cone 的 native 接口，不属于 runtime ABI 或公共 Scoop 枚举身份。

首版支持 aarch64-apple-darwin、x86_64-unknown-linux-gnu、x86_64-unknown-linux-musl。LibuvEventLoop 使用固定的 libuv 1.53.0，源码、许可证和各目标 native 输入随 Cone 发布；不查找系统 libuv。C11 shim 只管理 native I/O、稳定存储及完成记录，worker／callback 不进入 managed 代码。TLS、HTTP 属于各自库，文件监听、进程和任意 fd 接管不在本版范围；M54 为实际缓存与下载补齐第 9 节的基础能力；M59 增加第 10 节的自有 TTY。

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
    public suspend fun openTty(): AsyncTty
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
public suspend fun AsyncByteSource.readAllBytes(): Array<Byte>
public suspend fun AsyncByteSource.readAllBytes(maxBytes: Long): Array<Byte>
```

count 非负且不超过 buffer 当前可用区间。零长度字节流请求在状态及取消检查后返回零，不执行 native I/O、不表示 EOF。正 read 允许短读，零表示 EOF；正 write 允许短写，零抛平台 WriteZeroException。readFully 不足抛平台 UnexpectedEofException；各扩展保留已提交进度，不回滚或重发。copyTo 不接管任一端关闭责任，不自动 flush。

省略 count 的基本操作扩展取 buffer 当前可用长度后转发。readFully／writeFully 先检查取消与 buffer 范围，零长度不调用端点；copyTo 要求 bufferSize 为正，内部 buffer 在退出时释放。组合扩展检查端点返回的进度在本次请求范围内，不把非法返回值当作正常结束。

readAllBytes 从当前位置读取到 EOF，以至多 8192 字节的原生缓冲处理短读，返回剩余内容的独立数组；它是调用者显式要求的内存汇集，不预先读取文件大小或接管输入端关闭责任。每次读取前及接受实际结果前检查取消，输入异常及取消保留实际原因，不返回伪装成完整结果的部分数组。底层调用实际结束后才释放内部缓冲；UTF-8 或其他字符集解释由调用者使用现有文本能力完成。

带 maxBytes 的重载要求上限非负，参数错误在读取和分配缓冲前报告；无参数入口保持显式读取全部内容的行为。结果至多包含 maxBytes 个字节，空输入在零上限下成功；恰好达到上限后仍需确认 EOF。为了区分完整内容与超限，最多额外读取一个字节，超限抛 ReadLimitException 并公开 maxBytes，不交付部分数组。输入实际位置保留在已读前缀之后，不回滚或关闭来源；调用者可继续读取剩余内容。计数和探测大小避免 maxBytes + 1 的溢出，Long.MAX_VALUE 也是合法上限。接受迟到结果时若已取消，等待真实借用结束后交付取消，不把迟到 EOF 或超限改成成功。

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
    public suspend fun metadata(): FileMetadata
    public suspend fun size(): Long
    public suspend fun position(): Long
    public suspend fun seek(position: Long)
    public suspend fun truncate(size: Long)
    public suspend fun syncData()
    public suspend fun syncAll()
}
```

openFile 使用 Path／OpenOptions 的已有合同。文件 open／read／write／metadata／seek／truncate／sync／close 在 libuv worker 调用 platform 的 GC-free helper，保留 PosixErrno。顺序 read／write 共用文件位置，同一文件的读写、查询、位置／长度控制与 sync 同时最多一个。flush 不等于 sync；不使用 uv_fs 的额外 close／sync 语义。worker 只接收 native 存储，立即记录错误。已开始的阻塞操作可能推迟取消和 run 退出，不承诺固定时延。

## 8. 实现边界与验收

native request 持有预留完成节点；C callback 仅存状态和进度、入 native 队列，managed 在 NativeSafe 返回握手后消费。操作编号在完成消费前不复用。managed 使用普通带类型操作与结果，不以可空字段猜测不同事件。提交同步失败走同一释放路径。libuv 及相关系统库按语言规范 12.2.1～12.2.3 的普通 native 输入链接；三目标不启用 io_uring。

主要验收为同 Cone scoop.test：真实 loopback 动态端口、无公网 DNS、真实文件、Context、取消／关闭竞争、部分进度、原始错误、SIGPIPE 及 moving GC。竞争用明确同步点，测试源码和工具始终存在。详细边界见 [M49 设计第 14 节](../milestone49/DESIGN.md#142-异步-io)。

## 9. M54 基础能力补齐

### 9.1 文件与目录

AsyncFile 增加 suspend 的 metadata、size、position、seek(position)、truncate(size)，语义复用 platform File；读写、位置查询／变更、truncate 与 sync 共用该句柄的串行操作边界。seek／truncate 的参数和偏移范围在提交前检查，失败不凭空改变已记录位置。不能在异步方法体内直接调用同步 File API。

具体签名为 `metadata(): FileMetadata`、`size(): Long`、`position(): Long`、`seek(position: Long): Unit` 与 `truncate(size: Long): Unit`，均为 suspend 函数；size 复用 metadata 的实际 OS 查询。seek 是绝对非负位置，truncate 不改变文件位置；seek 越过 EOF 后写入的空洞和 append 行为沿 OS 合同。负参数在原生提交前抛 IllegalArgumentException，底层错误保留 PlatformException 的 errno 和原路径。

路径查询入口为 `EventLoop.metadata(path: Path, followLinks: Boolean = true): FileMetadata`，与句柄查询复用 platform 的同一原生结构及转换。路径先取得原生副本，完成事件携带独立的结果记录，managed 复制并释放该记录后才恢复 continuation。文件查询与位置／长度操作沿 OwnedFile 的串行登记、关闭和取消路径；排队取消不执行 syscall，已开始的操作等实际完成后结束。若取消或关闭获胜，已经发生的 seek／truncate 不回滚，之后观察到的文件位置与长度以 OS 为准。

文件系统入口沿 EventLoop.openFile 的方式增加 `metadata(path, followLinks)`、`moveFile(source, destination, replaceExisting)`、`deleteFile(path)`、`createDirectories(path)`、`openDirectory(path)` 与 `createTempFile(parent, prefix)`，返回类型复用 FileMetadata／Path 等既有值。move 的排他／替换和跨文件系统错误沿 platform 合同，不暗中 copy+delete。

路径修改的具体入口为 suspend 的 `EventLoop.moveFile(source: Path, destination: Path, replaceExisting: Boolean = false): Unit`、`deleteFile(path: Path): Unit` 与 `createDirectories(path: Path, mode: Int = 0x1ff): Unit`。deleteFile 删除文件、链接本身或空目录，不递归删除目录内容；moveFile 保留源内容，默认排他，替换时沿同一文件系统 rename 合同。move 失败的 context 为 destination，其他修改为对应 path；保留本次 syscall 的 errno。

createDirectories 逐层调用共享 mkdir helper，缺少父目录时继续创建父目录；已经存在的目录成功，普通文件占用目录位置或其他错误照常报告。mode 复用 platform 的 0..0xfff 校验并受 OS umask 影响，不修改已存在目录的权限。每次 mkdir 和确认已有目录的 metadata 都通过 worker 等待，层间取消不再提交下一项；已创建目录与已生效的移动／删除不回滚。所有路径在工作排队前取得 native 副本，排队取消不执行 syscall，已经开始则等待实际完成。

createTempFile 以一次 native 排他创建返回实际 Path 与拥有句柄的 AsyncFile，避免先创建路径再重新打开；结果为有名的普通 TemporaryFile 值。取消先获胜而 native 创建成功时，关闭未交付句柄并清理本次未交付的临时文件；已交付后是否删除由调用者负责。失败或清理错误保留真实原因，不执行目录来源审计。

具体入口为 suspend 的 `EventLoop.createTempFile(parent: Path? = None, prefix: String = "scoop-"): TemporaryFile`。TemporaryFile 是带只读 path／file 字段的普通 struct，实现 AsyncCloseable，其 close 只关闭同一个 AsyncFile，不隐式删除已交付路径。父目录默认沿 platform 的 TMPDIR 选择，prefix 拒绝 NUL 与路径分隔符；文件以 0600（受 umask 影响）、读写和 close-on-exec 创建，初始位置与长度均为零。

路径模板与排他 native 创建由 platform 共享 helper 提供；创建失败保留父目录 context 和立即 errno。迟到的成功结果先纳入正常 OwnedFile 生命周期，再执行不可由本次取消中断的 close 和 delete；两项清理都尝试完成，失败以 ResourceUseException 与原取消／关闭原因组合，不能遗失清理错误或向调用方交付部分结果。成功交付后，路径和文件句柄拥有普通文件的独立生命周期。

`AsyncDirectoryEntries` 为 AsyncCloseable 的单遍枚举器，`next(): DirectoryEntry?` 可挂起，正常 EOF 关闭句柄，提前退出须 close。名称在 native 返回前复制，跳过点目录，不一次性把任意大目录收集到数组。缓存淘汰或启动扫描使用此公共能力。

具体入口为 suspend 的 `EventLoop.openDirectory(path: Path): AsyncDirectoryEntries`；目录项复用 platform 的 DirectoryEntry，保持原始名称字节和 OS 枚举顺序，不额外查询每个条目的 metadata。同一目录同时最多一个 next，冲突报 IllegalStateException；正常 EOF 被记住，后续 next 等待同一次关闭后继续返回 None，显式提前关闭后的 next 报 IoClosedException。next 遵守所属 loop／Job 约束，close 沿已有资源清理合同允许取消中的 finally 和幂等关闭。

open／next／close 均由 worker 调用 platform 的目录 helper。next 在本次工作内复制一个名称，managed 取得独立快照并释放原生副本；关闭等待正在进行的 readdir 及结果消费结束。排队取消不推进目录，已经执行的 next 不回退 OS 位置，迟到名称仍被释放。open 取消后取得的句柄必须在恢复调用方前关闭；循环退出也收尾未关闭的目录，不把所有名称预先收集进数组。

上述工作使用既有 libuv worker 和 platform 的 GC-free 文件 helper；必要 helper 先在 platform 补齐，再由 asyncio 调度。路径、输入数据和输出存储在提交前取得稳定 native 副本，errno 在 worker 内立即记录。已开始的 syscall 取消仍等 after-work；迟到结果和 close 按第 3 节收尾。

### 9.2 地址族竞速与 SOCKS5

增加 `EventLoop.connectTcp(host: OsString, port, options)` 的域名重载，提供 RFC 8305 所需的双族解析、候选交错和延迟启动。默认 resolution delay 50 ms、connection attempt delay 250 ms，可配置；默认同时在途的 TCP 候选不超过 2 个。先获得的可用地址不必等待另一族全部解析完成，全部失败保存实际尝试错误。

选择一个成功连接后关闭其他未交付连接，等待竞速子任务和 native 结果收尾。候选普通失败在该任务内转成结果，不能传播并取消本来可能成功的候选。参数仍为 OsString，不隐式 IDNA；传入 SocketAddress 的既有接口保持不变。配置变化只影响本次连接，不建立全局 DNS 缓存、TTL 推测或 HTTPS records 实现。

具体入口为 `EventLoop.connectTcp(host: OsString, port: Int, options: TcpConnectOptions = TcpConnectOptions()): AsyncTcpStream`。TcpConnectOptions 为不可变配置，包含 `resolutionDelay: Duration = Duration.milliseconds(50)`、`attemptDelay: Duration = Duration.milliseconds(250)`、`maxConcurrentAttempts: Long = 2L`，以及默认均为 None 的 `resolveTimeout: Duration?`／`connectTimeout: Duration?`。所有配置时长必须非负，并行数必须为正。零间隔允许立即推进，零超时表示进入该阶段即到期；实际在途数仍受并行数限制，单地址入口不应用这些竞速策略。

两族分别调用既有异步 resolve，族内保留系统结果顺序并去除重复地址。IPv6 已有结果时立即开始；IPv4 先到且 IPv6 未结束时，最多等待一次 resolutionDelay，IPv6 失败或空结果提前解除等待。开始后优先选上次尝试的另一族，不因尚未返回的族阻塞已有候选。相邻尝试默认间隔 attemptDelay；任何候选失败可以提前开始下一项，达到并行上限时等待空位，迟到的旧定时通知不能越过新间隔。

全部候选失败抛 TcpConnectException，保留 host、port、`attempts: Array<TcpConnectionFailure>` 与 `resolutionFailures: Array<TcpResolutionFailure>`。两个记录分别含 address／family 及原始 cause，按各自完成顺序保存；解析成功但没有地址不伪造 OS 错误。此异常在调用方可正常捕获，不预先取消其 Job 或兄弟任务；参数及错误 loop 仍报原状态／参数异常。成功交付前发生总取消时关闭已选中的连接；清理失败以 ResourceUseException 保留，不遗失迟到连接的关闭错误。

resolveTimeout 从开始双族解析前取同一个单调时钟起点，直到两族各自交付结果。截止到期只取消尚未完成的族，并将其记为 TcpResolutionFailure，cause 为 TcpResolutionTimeoutException，保存 host／port／family、limit 与 elapsed；已经获得的候选仍可连接。迟到解析结果正常释放，不替换已结束族的候选，也不使成功的另一族失败。零解析超时不提交 DNS 工作，最终没有候选时仍通过 TcpConnectException 交付两族的明确超时原因。

connectTimeout 从第一个候选即将启动时开始，对整个候选序列共享，失败和交错启动不重置它。到期抛 TcpConnectTimeoutException，保存 host／port、limit／elapsed 和当时已记录的 attempts／resolutionFailures；零连接超时不提交 TCP 尝试。阶段完成与新尝试前也检查单调截止，已经选择的外部取消保持原异常。两类超时都取消有关任务并等待实际 native 工作与未交付 socket 清理完成，再返回调用方；计时器随竞速作用域结束，不留下长时间等待的子任务。

TcpConnectOptions.onEvent 是可选的普通 `(TcpConnectEvent) -> Unit` 回调，默认 None。事件保存 phase（TcpConnectPhase.Resolve／Connect）、kind（TcpConnectEventKind.Started／Finished）和单调 TimeMark。Resolve 覆盖双族解析，Connect 从首个候选开始；两个阶段可以重叠，各自至多通知一次开始与结束，不为每个候选保留事件历史。零截止仍表示进入过对应阶段；没有可用地址时没有 Connect 事件。成功连接导致未完成的解析／竞速任务取消时，未结束阶段的 Finished 在这些任务实际收尾后交付。

通知在 Scoop 连接流程、内部锁外执行，保留原 Context；未配置回调不增加阶段采样时钟。回调应快速返回，上层挂起 observer 可以只在这里记录状态，再于自己的调用步骤交付。回调抛出后停止后续通知，取消并等待连接任务，关闭未交付的 socket，再向调用者报告原异常；已有主失败保持在 ResourceUseException.primaryFailure，通知或实际关闭的新失败按发生顺序附加。这里不包装普通 TCP／解析错误，也不增加客户端私有 DNS 或连接竞速路径。

提供 SOCKS5 的普通异步握手适配，支持无认证和 RFC 1929 用户名／密码、CONNECT、IPv4／IPv6／ASCII 域名目的地、本地解析或代理解析。用户凭据和长度在写出前检查；reply 错误保留协议码。使用已有 AsyncByteStream，按 takeOwnership 规则接管，成功交付隧道与完整前缀，失败清理；不让 HTTP client 手写 SOCKS 报文或同步 DNS。

具体入口为 suspend 的 `AsyncSocks5Tunnel.takeOwnership(transport, destination: Socks5Address, credentials: Socks5Credentials? = None, resolution: Socks5Resolution = Proxy)`，transport 同时实现 AsyncByteStream 与 AsyncCloseable。Socks5Address 为 `Ip(SocketAddress)` 或 `Domain(String, Int)`，Socks5Resolution 为 Local／Proxy；返回的 AsyncSocks5Tunnel 实现同一异步流及关闭接口，并公开 boundAddress 和 isClosed。目标端口在 0..65535，域名为 1..255 字节的可打印 ASCII 且不含空格，不做 IDNA 或 DNS 标签规范化；IP 不能携带 SOCKS5 无法表示的非零 IPv6 scopeId。

Socks5Credentials 以 List<Byte> 接收用户名与密码，分别保存独立 Array<Byte> 快照，长度均为 1..255；`fromUtf8(username, password)` 是明确的文本便利入口，长度按 UTF-8 字节计算。无凭据时只提议无认证方法，有凭据时只提议用户名／密码方法，不自动降为无认证。协议不规定这些凭据字节的字符集，不输出凭据内容。

Proxy 模式把域名原样放入 CONNECT，Local 模式在写出握手前以当前 EventLoop.resolve 解析，并使用系统返回的第一个地址；空结果报明确失败，不伪造原生错误。调用者可显式传 Ip 选择其他候选；一次隧道握手只发送一次 CONNECT，失败后不能在同一流上改目标重试。IP 目的地不执行 DNS，Local／Proxy 均不更改已有地址。

配置、目标与活动 Job／loop 检查通过后接管传输；此前失败仍由调用者负责关闭。接管后所有解析、写入、flush、读取及取消失败都等待真实操作结束并关闭未交付传输，清理失败使用 ResourceUseException。握手使用有界缓冲及 readFully／writeFully，每一阶段排出输出；读取只消费当前回复的确切长度，因此和回复同次到达的隧道数据完整保留在底层流中，返回后直接从隧道读取，不丢弃或重复前缀。

Socks5Exception 保存 phase（Method／Authentication／Connect）及可选 replyCode；拒绝的方法、认证状态和 CONNECT REP 保留实际协议字节。非法版本、保留字段、地址类型或绑定域名报对应阶段的格式错误；底层 I/O、解析和取消保持原异常。交付后的流沿普通 AsyncByteStream 的双向、取消与背压合同；close 固定 loop 归属，可在取消 finally 中完成，并发关闭等待同一次底层收尾，之后幂等。

### 9.3 领域库的 native 阻塞工作

TLS 的系统信任装载及平台验证需要能够挂起等待 native 工作。为这一实际使用者，在 `scoop.asyncio.native` 提供受 Unsafe 约束的 NativeWork 资源与 `EventLoop.runNativeWork(work)`；它复用现有 DNS／文件的 uv_queue_work 提交、取消与完成机制，不建立第二个线程池。

NativeWork 持有调用库构造的稳定 native 参数、GC-free 执行入口、结果存储和释放入口。worker 不调用 Scoop 回调、分配 managed 对象或恢复 continuation；结果的实际类型与提取由调用库的普通 FFI 声明负责。工作返回后，managed 调用方才能取得结果并构造 Scoop 值。它不是可执行任意 suspend／Scoop lambda 的 runBlocking 或通用调度框架。

runNativeWork 期间借用 work，禁止重复提交和关闭；登记前失败不转移所有权。取消获胜也须等执行结束、复制错误和解除借用后返回，调用方在 finally 关闭 work。外部通知、Job 取消与 continuation 恢复保持锁外；native ABI 是普通库间接口，不新增 runtime functions 或 compiler-known 类型。

具体入口为 Unsafe 的 `NativeWork.takeOwnership(payload: Ptr<Unit>, execute: FunPtr<(Ptr<Unit>) -> Unit>, release: FunPtr<(Ptr<Unit>) -> Unit>)` 和 `EventLoop.runNativeWork(work): Unit`。payload 由领域库原生代码分配，执行／释放指针必须来自可在对应线程调用的原生函数；不能使用仅支持已注册 Scoop 线程同步调用的静态 Scoop callback，也不能把 ForeignCallback 的 trampoline 单独交给 worker。执行函数在 payload 中保存结果及错误，不跨 C 边界展开异常；释放函数销毁同一份输入与未交付结果。

work 是一次性资源：开始提交后，不论完成、提交失败或取消都不能再次运行；loop／Job 的前置检查失败不开始提交。正常等待返回后，通过 Unsafe 的 `work.withResult(block: (Ptr<Unit>) -> R): R` 在 managed 侧读取领域结果；回调不能挂起或让借出的指针逃逸。结果读取期间也禁止再次读取、提交或关闭，回调结束后解除借用。没有成功等待结果时 withResult 报状态错误；领域工作自身的失败仍由领域库从结果中解释。

work 实现 Closeable，提供 isClosed；未使用、完成或失败的 work 均可关闭，释放入口恰好调用一次。其本地状态采用普通同步保护；执行与释放原生入口、结果回调及 continuation 恢复均不在状态锁内运行。runNativeWork 不接管关闭责任，EventLoop 通过既有在途操作等待实际 after-work，调用者在 finally 中关闭 work。

### 9.4 验证

文件能力独立验证真实 metadata／seek／truncate、排他临时创建、同文件系统替换、目录分批遍历、worker 取消与迟到结果；组合磁盘缓存和下载续传。连接竞速验证单族迟到／失败、不同候选交错、成功竞争、总取消及失败不污染父 scope。SOCKS5 使用独立 loopback 对端覆盖分段、认证和两种 DNS 模式。

NativeWork 验证正常完成、登记失败、未开始撤销、运行中取消、异常清理和 moving GC，实际消费者包含 TLS 系统验证；不为仅存在于测试中的虚构工作扩展接口。三个目标运行对应库测试和生产消费者。

## 10. 自有 TTY

```scoop
public struct TtySize(val columns: Long, val rows: Long,
    val widthPixels: Long, val heightPixels: Long)
public enum TtyRead {
    Bytes(val count: Long), Resize(val columns: Long, val rows: Long), End, TimedOut
}
public interface AsyncTty : AsyncByteSink, AsyncCloseable {
    public val isClosed: Boolean
    public suspend fun receive(buffer: NativeByteBuffer, count: Long,
        timeout: Duration? = None): TtyRead
    public suspend fun size(): TtySize
    public suspend fun isRawMode(): Boolean
    public suspend fun enableRawMode()
    public suspend fun disableRawMode()
}
```

openTty 通过 platform 的共同 owner 取得实际终端与 resize 通知，每进程最多
一个活动 owner，包括同步 terminal 会话。失败清理所有已取得的资源；取消
选定后才完成的 open 先关闭未交付 owner。库不接管调用者 fd，不改变 stdin
的 O_NONBLOCK，不增加后台读取线程。

receive 同时等待字节与 resize，使用 libuv poll 观察自有 fd，实际 read 为
非阻塞 syscall。正 count 返回正 Bytes、End、Resize 或 TimedOut；count 为 0
在状态、参数和取消检查后返回 Bytes(0)，不表示 EOF。None 无限等待，非负
timeout 使用单调总时间；零不等待。resize 可合并为最后实际尺寸，通知本身
不占用字节 buffer。收到 End 后后续正 receive 继续返回 End。

同一资源允许各一个 receive 和 writeFrom 同时在途；模式和尺寸控制要求两个
方向都空闲。flush 沿无缓冲输出合同。所有原生操作在所属 loop 线程执行；
不把同步阻塞 poll／write 包在 suspend 函数内。SIGWINCH 沿终端规范的
Linux self-pipe／Darwin kqueue 实现，由 libuv poll 观察通知 fd，避免另装
libuv signal handler 改变应用原 disposition。

取消停止 poll／timer 后记录一次终止，借用按实际字节进度提交后才恢复。
取消已经选定时到达的 resize 保留给下一次 receive；End 保留为输入终态。
单次取消不关闭健康 TTY。关闭拒绝新操作，结束在途等待，等待借用释放和
全部 libuv close callback，再恢复 raw 并释放共同 owner；run 收尾同样执行。
原生错误保留 PosixErrno／Libuv 来源；恢复失败也继续消费所有资源。

disableRawMode 和 isRawMode 允许已取消 Job 在所属活动 loop 中进行清理，
不因此接受其他被取消的 I/O。模式的幂等、快照与恢复沿终端规范第 7 节。
async raw 作用域使用这两个入口和 useAsync，GC 不承担恢复。

## 11. 平台后端与宿主主循环（迁移合同）

本节规定当前迁移合同。各平台实施完成时，第 1～10 节中的具体 libuv 驱动、撤销、计时精度和原生错误描述应同步替换；其中任务归属、资源方向独占、实际进度、路径／地址、TTY、显式关闭与 buffer 借用合同继续适用。当前版本仍按第 1～10 节运行。

### 11.1 共享层与平台后端

`EventLoop` 保留现有公共方法；默认实现改为 `PlatformEventLoop`，Darwin 绑定系统 libdispatch，Linux 绑定 io_uring／固定 liburing。所有端点、文件、目录和操作只依赖同一共享 owner；Job／CoroutineScope 仍来自 core。native 后端管理原生资源、请求、完成记录，不拥有另一套任务树，也不直接执行 managed I/O continuation。

后端按目标构建选择，不公开后端注册表。迁移完成后移除 `LibuvEventLoop` 与 libuv 依赖，迁移全部实际调用方；不修改语言、runtime ABI、IR 或产物格式。平台能力与依赖版本见[设计第 7～10 节](../asyncio-backends/DESIGN.md#7-macoslibdispatch-后端)。

### 11.2 运行与附着

`PlatformEventLoop.run` 保留同步单次根入口合同。新增 `attach(host, onClosed): EventLoopSession`，接入调用线程所属的宿主主循环；两种入口共用一次启动机会，均拒绝当前 Job／活动 EventLoop binding 中的嵌套启动。接受入口后才取得 native 资源，初始化失败回滚并关闭实例。

session 暴露 `loop: EventLoop`、`isClosed: Boolean`、`post(block: (CoroutineScope) -> Unit): Boolean` 与 `requestClose()`。后三项可跨线程；post 异步排队，关闭后拒绝且不保留闭包。接受的投递在关闭／失败时可以丢弃，内部取消、完成和清理命令不能丢弃。post 执行在仍活动的结构化根作用域中，子任务沿现有 launch／async 合同；根在没有消息时挂起，持续到请求关闭或失败。

内部根在 attach 栈内捕获完整 Context 并挂起，用户投递从后续宿主通知开始；mailbox 通过根 continuation 恢复，不继承外线程 post 时的动态 Context。普通投递抛错作为根失败处理。requestClose 幂等、不阻塞，停止应用投递并把根取消交给 owner；mailbox 的取消恢复也排入 owner。主动关闭的取消不作为最终失败，其他失败沿现有组合规则保留。

两种模式共用非阻塞处理流程；run 在空闲时 NativeSafe 等待，attach 只在宿主通知时处理一轮。通知可合并，真实队列承担完整性。嵌套宿主循环不能递归进入处理流程；处理期间产生的新工作留待后续通知。库不抢占用户回调。

### 11.3 宿主接入合同

`scoop.asyncio.host.EventLoopHost` 提供 `isCurrentThread()` 与 `@Unsafe watchReadable(descriptor, ready): EventLoopWatch`。host 只监听、不读写或关闭 loop 借出的通知 fd；ready 在 owner 线程串行执行，注册时不得内联执行。安装失败不得留下活动监听或回调。

`EventLoopWatch.remove(onRemoved)` 在 owner 上异步注销，按宿主协议不得抛错；必须等待当前 ready 退出并退休排队通知，再通知移除完成，此后不得再进入 ready。loop 在移除完成前保留 fd 和 callback 上下文。

Darwin 提供主 dispatch queue 的 `MainDispatchHost`，Linux GUI 在自己的 fd watch 中监听 eventfd。宿主仍负责窗口、输入、显示刷新和其连接协议。使用主队列的适配器只在宿主服务该队列时取得执行机会，不保证任意自定义 run-loop mode 都运行。具体声明与异常出口见[设计第 3 节](../asyncio-backends/DESIGN.md#3-公共入口与宿主协议)。

本库排队恢复在 owner 线程进入 managed；任意第三方挂起点仍可能在其他线程恢复。接入 GUI 不新增协程拦截器或所有挂起函数的 UI 线程亲和性保证，外部回调通过 session.post 进入根作用域。

### 11.4 取消、借用与最终关闭

结果选择与 native 结束分别记录。取消／close 先选择结果后请求后端撤销；必须等数据操作、相关控制请求及未交付资源清理真实结束，提交已知实际进度并解除借用，再恢复一次 continuation。dispatch cancel 请求或 io_uring 取消 CQE 不能单独证明目标操作结束；成功先选定不被后续取消覆盖。

根及其全部后代完成后继续关闭遗留资源，并排空清理、完成和 ready。全部原生请求、worker 与 I/O callback 退出后，附着模式再注销宿主监听，最后销毁后端、关闭通知 fd 并在 owner 上执行一次 onClosed。宿主必须继续运行到该通知，不能先停止主循环再等待它。当前宿主移除通知的 callback lease 在其返回时释放，不等待自身退出。

onClosed 执行时 isClosed 为真，参数为运行／清理失败或 None。回调自身异常由宿主桥捕获并报告，不重复关闭、不跨 C 栈传播。GC release 不驱动循环、恢复任务或等待原生退出。阻塞 syscall 不能强制撤销时，显式关闭须等其真实返回。

### 11.5 平台能力与错误

Darwin 用 dispatch source 驱动 nonblocking 网络／TTY 与单调 timer，后台 work 处理普通文件、DNS、目录、路径与 NativeWork；source 全部退休后才消费其 fd。Linux 用 ring 完成网络、文件数据 I/O、fsync、timer 与 TTY readiness，其余阻塞工作使用 native worker。两者复用 GC-free 平台 helper 和同一资源语义。

Linux 新后端支持基线为内核 6.1，初始化实际探测所需操作与 feature；ring 不可用或能力不足明确失败，不静默回退其他后端。此要求仅针对 asyncio 新后端。完整操作集、队列满时的处理、取消退休、提交失败与 ring 销毁见[设计第 8 节](../asyncio-backends/DESIGN.md#8-linuxio_uring-后端)。

新后端的 syscall／负 CQE 错误保留 PosixErrno，DNS 保留 AddressResolution；正常 timer 到期不是错误，取消和 IoClosedException 仍由共享层选择。最终移除 Libuv 错误转换须随实际消费者迁移进行。迁移验收覆盖 Darwin、GNU、musl 的同一公共行为，以及宿主监听注销、Context、取消竞争和 native 内存寿命；本次设计不代表已完成这些验证。
