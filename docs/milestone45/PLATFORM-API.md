# M45：scoop.platform 公共能力

状态：M45-1～8 已完成本机实现与验证；本文件的网络仍待实施。已实施公共合同见[平台库规范](../specs/SCOOP-PLATFORM-SPEC.md)，结果见[实施记录](PROGRESS.md)。所属任务与测试要求见 [M45 设计](DESIGN.md)，所有拥有资源的类型同时遵守 [资源协议](RESOURCES.md)。以下能力面向三个现有 POSIX target，不引入新的目标平台。

## 1. 公共类型、错误与同步语义

所有 API 是普通 Scoop 声明。size／offset／byte count 使用 Long，fd、errno、端口等按各自合法范围检查后才转换到真实 C 类型；裸 fd／pthread_t／native pointer 不作为普通 IO 用户必须操作的公共表示。

`scoop.platform.system` 提供统一错误数据：

- `PlatformErrorKind`：NotFound、AlreadyExists、PermissionDenied、NotDirectory、IsDirectory、InvalidInput、NoMemory、NoSpace、BrokenPipe、ConnectionRefused、ConnectionReset、AddressInUse、AddressUnavailable、TimedOut、Interrupted、WouldBlock、Unsupported、Other。
- `NativeError`：保存错误域与原始 Int code；首版域区分 PosixErrno 和 AddressResolution。pthread 返回的 errno 风格 code 直接保存，getaddrinfo 的 EAI_SYSTEM 还保留本次 errno。
- `PlatformException : Exception`：保存 operation、kind、可选 NativeError 及必要的路径／端点上下文。消息用于诊断，调用者按结构字段分支，不解析平台错误文本。
- IO 另有明确的 UnexpectedEofException／WriteZeroException，表示要求完整操作时遇到 EOF／无进度，不伪造一个 errno。

参数范围和对象状态错误使用 core IllegalArgumentException、IndexOutOfBoundsException、ArithmeticException、IllegalStateException；不全部包装成系统调用失败。OS 未知错误保留原始 domain/code 并归 Other；Unsupported 表示真实 OS 操作不具备该能力，不作为承诺了却没有实现的占位分支。

所有本期 IO 均为普通同步方法，可能阻塞。复合操作区分“已提交的外部进度”和“尚未执行”，不能在已读写部分数据后把整体称为原子失败。回调、字节 buffer 及资源本身各自遵守明确的所有权和线程规则。

## 2. OsString 与 Path

### 2.1 系统字符串

`OsString` 位于 `scoop.platform.system`，是保存 POSIX 原始字节的不可变 final class，实现基于字节内容的 Equals／Hash。构造时复制输入，读取 raw bytes 时返回独立 Array<Byte>。

提供 fromString、fromBytes、toBytes、decodeUtf8 和 decodeUtf8Lossy。String 输入编码为 UTF-8；decodeUtf8 严格校验并沿 core 抛 CharacterCodingException，lossy 必须显式选择。诊断展示使用明确的 display 方法，不能把有损结果反过来用于系统调用。

值本身可以保存任意字节。传入 pathname、argv、环境项等 C 字符串位置时拒绝内嵌 NUL；环境变量名还必须非空且不含 `=`。不把无效系统字符串静默截断为第一个 NUL 之前的部分。

### 2.2 路径

`Path` 位于 `scoop.platform.fs`，用不可变的原始字节表示当前 POSIX target 的路径，拒绝 NUL；不要求 UTF-8。String／OsString 工厂都保留实际内容，不自动 realpath 或读取文件系统。

提供 isAbsolute、fileName、parent、components、resolve、normalizeLexically、toOsString，以及严格／显式 lossy 文本转换。resolve 遇到 absolute child 使用 child；词法归一化只处理路径组件，不解析 symlink，不保证保持经过 symlink 的 OS 解析结果，也不当作访问权限检查。

Path 的 Equals／Hash 比较保存的字节，不读取磁盘、不隐式大小写折叠，不声称不同路径是否指向同一文件。canonicalize 是独立的实际文件系统操作，可以失败。OS 对相对路径的解析使用调用当时的 cwd。

首版只定义三个现有 POSIX target 的路径行为，Windows 驱动器、UNC 与 UTF-16 路径留待新 target 设计，不给当前实现添加无用途的双表示框架。

## 3. 字节流、缓冲与文本

### 3.1 共有协议

`scoop.platform.io` 提供 ByteSource 和 ByteSink，核心方法签名如下：

```scoop
public interface ByteSource {
    public fun readInto(buffer: NativeByteBuffer, count: Long): Long
}

public interface ByteSink {
    public fun writeFrom(buffer: NativeByteBuffer, count: Long): Long
    public fun flush()
}
```

count 必须位于当前 writableBytes／readableBytes 范围内。普通扩展提供省略 count 的便利调用，使用整个对应区间；接口不承担 owner 关闭，因此借用的标准流也可以实现它们。

- 正请求的 read 返回 1..count 或 EOF 的 0；write 允许短写，返回 1..count。count=0 是无进度的正常操作，不用于判断 EOF，不调用 native IO。
- read 推进 writePosition，write 推进 readPosition。一次失败且未交付字节不移动游标；完整方法逐次提交已经发生的进度。
- readFully、writeFully、copyTo 通过普通循环组合；readFully 短于请求时抛 UnexpectedEofException，正请求 write 无进度时抛 WriteZeroException，不能忙循环。
- readAllBytes／readText 等便利操作按实际表示和分配能力增长，失败报告真实错误；需要流式或自行限制数据量时使用基本协议，不给整个库增加任意固定配额。
- Array／MutableArray<Byte> 的直接读写重载内部可以用 core 的同步借用，不强制先分配另一个 off-heap buffer。

flush 只要求提交该 sink 自己的用户态缓冲，不等于 fsync 或持久化承诺。File 的原始 fd sink 没有用户态字节缓冲，flush 可无操作；需要文件持久化时显式 syncData／syncAll。

### 3.2 缓冲包装的所有权

BufferedSource／BufferedSink 使用 NativeByteBuffer 实现可复用缓冲，允许在内部按明确逻辑 fill、compact 和 ensureWritable。它们是 Disposable，只隐式拥有自己创建的 buffer；默认借用传入的 endpoint，不因为拿到 ByteSource／ByteSink 就关闭它。

需要连同底层一起关闭时，调用显式 takeOwnership 工厂；该路径要求底层同时满足相应 IO 协议和 Disposable，并将该引用的逻辑所有权交给包装。实现用普通 class／泛型 bound，不增加自动 move 或动态接口伪造。包装关闭后不允许继续通过旧别名操作已转交的 endpoint。

BufferedSink.dispose 先尝试 flush，再释放内部 buffer，并关闭明确接管的底层；其中一项失败不跳过后续清理，按资源协议保留失败。BufferedSource.dispose 不把未读数据写回或交回底层，丢弃其缓冲并释放自己拥有的资源。release hook 不 flush，也不遍历底层 managed 对象。

### 3.3 文本与标准流

提供显式 UTF-8 的 readText、writeText、readLine、writeLine 等便利操作；readLine 去除行尾 LF 或 CRLF，最后一行可以没有分隔符，只有无字节且 EOF 才返回 None。非法 UTF-8 默认严格失败，需要替代解码时显式选择 lossy。writeLine 在当前三个目标使用 LF。

文本包装只需要已实现的 core String／UTF-8 能力，不依赖 M38 TextWriter。跨底层读取分块的 UTF-8 序列不能被当作多个非法片段；解码状态保留到完整序列或真正 EOF。

standardInput、standardOutput、standardError 是分别借用 fd 0／1／2 的 ByteSource／ByteSink 视图，不实现 Disposable，不关闭进程标准描述符。需要独立拥有的端点时提供显式 duplicate 操作，关闭副本不关闭标准 fd。

既有 core print／println、write／writeError 与 libc stdio 的缓冲合同保持。混用 fd 写入与已有缓冲输出时不承诺自动排序，必要的 flush 必须显式完成；platform 不替换 core 输出或改变 scoop.test 的捕获逻辑。标准视图使用调用时的标准 fd，能够遵守测试捕获期间的重定向。

## 4. 文件、metadata 与目录

### 4.1 File

File 为 final class，实现 ByteSource、ByteSink 和 Disposable。open 接收 Path 与 OpenOptions；options 明确 read／write／append、create／createNew、truncate 及创建 mode。至少选择 read 或 write，append／truncate 必须允许写，互相冲突的配置在 syscall 前拒绝；createNew 使用 OS 的排他创建，不先 exists 再 open。

交付 open、read／write、position、seek、size、truncate、syncData、syncAll、close，以及 Files 层的 readAllBytes／writeAllBytes、copy、move、delete／deleteRecursively、metadata、exists、createDirectory／createDirectories、readSymbolicLink／createSymbolicLink、canonicalize 和临时文件／目录创建。

- 打开的 fd 默认 close-on-exec，只有 Command 显式重定向需要的描述符传给子进程。
- seek／truncate 使用非负 Long 文件偏移；append 遵守 native append 语义，不能用 seek 到 EOF 再 write 模拟并发追加。
- metadata 包括文件种类、大小、可用时间戳和 POSIX 权限；不保证三目标都提供 creation time，不可得分量用 Option，不能填零当作有效时间。
- metadata／symlink 操作明确 followLinks；exists 只把 NotFound／路径中的 NotDirectory 转为 false，权限或 IO 错误照常抛出。先查询再操作不消除文件系统竞争。
- copy 不承诺事务原子；move 使用实际 rename 合同，跨文件系统错误不隐式变成 copy+delete。替换目标必须由操作选项明确，不能用有竞争的预检查假装排他。
- native 新文件 mode 仍受进程 umask 影响。deleteRecursively 不跟随 symlink 离开待删除目录树；不通过 GC 自动删除临时路径。

### 4.2 目录迭代与临时路径

DirectoryEntries 同时提供 Iterable／Iterator<DirectoryEntry> 和 Disposable，是共享一个 native 枚举状态的单遍对象。iterator 返回该单遍状态，next 返回 Option；跳过 `.`／`..`，不保证 OS 枚举顺序，每个返回项的名称是独立 OsString 快照。

读到 EOF 时自动关闭枚举句柄，后续 next 仍返回 None；提前 dispose 后再 next 抛 IllegalStateException。循环提前退出或正文抛异常必须由 use 清理，普通 for 不自动关闭任意 iterator。

createTempFile／createTempDirectory 在指定或系统临时目录下使用 OS 排他创建，返回实际 Path；不使用“生成名称然后检查是否存在”的流程。测试与调用者显式管理路径删除，关闭文件不等同于删除路径。

### 4.3 原始关闭

显式 close 先取出并清除 owned fd，再调用目标 shim。成功或失败都不对该整数无条件再 close；错误可以发生在描述符已经被消费之后，盲目重试可能关闭其他线程复用的新 fd。

EINTR 和关闭后的 native 状态由各目标实际合同处理，不在公共层一律重试。无法确定是否已消费的错误必须报告并终结该 owner，不承诺 OS 释放成功；release hook 也不能据此重放关闭。缓冲 flush 与 fd close 分别尝试，失败按资源协议保存。

## 5. 原始参数、环境与系统信息

`scoop.platform.env` 提供：

- `arguments(): Array<OsString>`，复制进程原始参数，包含 argv[0]；无参数 main 也可查询。复用既有 runtime 原始 argc／argv 能力，不调用 core internal 解码 helper。
- 环境单项查询与不可变快照，名称／值使用 OsString，区分变量缺失与空值；String 便利参数只负责 UTF-8 编码。
- currentDirectory、homeDirectory、temporaryDirectory 查询。home 不可确定时返回 None；临时文件／目录创建仍以实际 OS 成功结果为准。

既有 `main(Array<String>)` 的严格 UTF-8 启动行为不变；它可能在进入 main 前拒绝坏参数。需要原始字节的程序可用无参数 main，再读取 platform arguments。参数解析、短／长选项、schema、help 和退出策略均留给 M38 的 scoop.cli／应用。

本期不提供修改全进程环境或 cwd 的公开 API；子进程变化通过 Command 的明确配置完成。查询返回副本，但不能承诺与违反平台线程合同的外部 setenv 并发仍安全；不以一个仅覆盖 Scoop caller 的锁宣称解决全进程 native 访问。

`scoop.platform.system` 提供当前 target 的 OS／architecture 标识、系统版本和 logicalProcessorCount。处理器数是 OS 当前报告的在线逻辑 CPU 数，不承诺等于调度配额或容器 CPU budget；不另建平台能力注册中心。

## 6. Command、ChildProcess 与匿名管道

### 6.1 启动与配置

Command 保存 executable、参数、环境和 cwd 的快照。默认继承 stdio、环境与 cwd；可以为子进程显式配置 stdin／stdout／stderr 的 Inherit、Null、Pipe 或文件重定向，并控制 stderr 合并到 stdout。

参数数组与环境项作为独立 C 字符串传递，保留空参数、空格、引号及非 UTF-8 字节；不进行 shell 展开或自行拼接 shell command。启动明确路径；需要 PATH 搜索时使用显式 searchPath 入口，并采用本次子进程环境的 PATH，避免与父进程全局环境混淆。

实现用 posix_spawn 及目标支持的 file actions／cwd 扩展，不在已经启动 managed runtime 的进程中 fork 后继续执行 Scoop。每次创建的 pipe end、native argv／env 存储和 file-action 对象都有成功移交与失败回滚出口。

### 6.2 所有权与退出

ChildProcess 拥有其子进程观察／reap 责任和为本次启动创建的父端 pipes。stdio 属性返回借用视图；需要独立接管端点时使用显式 takeStdin／takeStdout／takeStderr，一次取出后 ChildProcess 不再关闭该端点。传入的 File 只借用到 spawn 完成，子进程取得自己的 OS 描述符。

提供 wait、tryWait、terminate、kill、communicate 和 dispose。ExitStatus 区分 Exited(code) 与 Signaled(signal)，不把信号编码成假退出码；重复 wait 使用已经保存的状态，不对可能复用的 pid 再 waitpid。

- wait 等待并 reap，允许抛系统错误；单纯非零退出码作为 ExitStatus 返回，不自动抛异常。
- terminate／kill 是调用者显式请求的操作；dispose 不隐式选择终止信号。
- dispose 尝试关闭仍拥有的父端 pipes，然后 wait/reap，可能阻塞；关闭失败不能跳过回收尝试。已经 wait 成功时只清理余下的 owned pipes。
- hook 最多对仍拥有的 pid 尝试一次非阻塞 waitpid(WNOHANG)；子进程仍运行时不等待、不注册后台任务，因而不能代替显式 wait。子 pipe owner 各自处理其 raw close。

### 6.3 通信与死锁

communicate 接收可选输入，将 stdin 写完后关闭，持续排空 stdout 和 stderr，最后 wait，返回独立字节数组及 ExitStatus。实现使用能够同时推进三方向的同步驱动，例如已 join 的专用 reader 线程；不能先读完 stdout 再读 stderr，也不能等待子进程结束后才开始读 pipe。

若 communicate 的驱动发生错误而无法完成通信，清理其已经启动的内部工作，必要时终止它负责的子进程以解除管道等待、reap 后再报告失败；不能留下仍依赖已释放 buffer 的读写线程。用户自管 pipes 时仍须自行安排进度，普通 writeFully／wait 不保证任意管道组合不会死锁。

匿名 Pipe 提供分别 owned 的 source／sink，独立实现 Disposable。关闭最后一个写端后读端看到 EOF；读端消失时写端报告 BrokenPipe。底层 pipe 与 socket 写入都遵守第 8 节的 SIGPIPE 策略。

## 7. Thread、Mutex 与 ConditionVariable

### 7.1 Thread

Thread.start 接收普通 `() -> Unit`，返回 owned Thread 对象。启动捕获创建时的 TaskContext，线程正文在其独立任务边界中执行；后续 binding 修改不反向修改创建者。使用已有 foreignCallback 的 OneShot、attach／detach 和异常边界，不新增线程 GC 协议。

提供 currentThreadId、yield、isFinished、join、dispose。成功 join 建立 worker 写入对 caller 的同步可见性；它等待整个 native worker 退出，不只等待正文设置一个完成标志。

线程正文的 Throwable 在 worker 的 managed 边界捕获并存到有正常根的 completion 对象。join 先完成 native 回收、将观察权置终结，再重抛该 Throwable；再次 join 可读取缓存结果，不重复 pthread_join。dispose 尚未终结时执行 join，已经终结时无操作，不重复报告过去的异常。self-join 在消费资源前拒绝。

join／dispose 的调用方须串行化；不承诺多个 caller 同时竞争回收一个 Thread。线程运行期间的 isFinished 查询与 completion 发布使用明确的原子同步。

### 7.2 callback 与 native 观察权

不要让 Thread 永久保留一份只能在 dispose 中释放的 callback observer ownership，因为 GC hook 不允许调用 callback runtime。

创建时建立普通 managed completion 对象，由 Thread 和 worker closure 按普通 GC 引用持有；worker 自己捕获失败并发布结果，因此 join 不需要通过保留 callback token 读取异常。OneShot worker ownership 在实际调用中按既有协议消费，创建失败由 managed 路径显式回滚。

native 线程控制记录只保存 native 数据，运行中的 worker 与 join 观察者各持有必要的原生存活权。Thread hook 可做非阻塞 detach／释放观察权，worker 完成后释放自己的一份；不跟随 managed 指针，不调用 callback release，不等待 mutator。这一记录只服务当前线程创建／join 的真实生命周期，不推广为通用共享资源框架。

没有 daemon 或 force-stop API。丢弃 Thread 并不保证程序能正常退出：worker 尚未结束时，现有 runtime 对未结束线程／callback 的 shutdown 诊断仍然生效。

### 7.3 Mutex 与 ConditionVariable

Mutex 为非递归 owned native mutex，提供 lock、tryLock、unlock、普通泛型 withLock；unlock 必须由持锁线程执行。withLock 通过正常／异常出口保证解锁，不提供 suspend 回调重载，因为协程恢复不保证仍在原 OS 线程。

ConditionVariable 在构造时绑定一个 Mutex，保活该关联；提供 signal、broadcast、await 及以 Duration 表示的超时等待。await 只能由持有该 mutex 的线程调用，原子释放 mutex 并等待，所有可恢复出口重新持有 mutex 后才返回或抛异常。

返回的通知／超时状态不证明用户条件已经成立。调用方必须在 while 谓词循环中等待，处理虚假唤醒和通知竞争。超时使用 monotonic 时间域；Linux 的 cond clock 与 Darwin 的相应等待接口由 target shim 处理，不把 realtime 校时当作经过时长。

Mutex 的成功 lock／unlock 和 cond wait 的重新取得锁提供用户数据的同步关系；GC safepoint 或 NativeSafe 切换不替代这些关系。

dispose 要求 mutex 未持有、cond 无等待者，也没有并发操作；不满足时拒绝并保留资源。hook 不解锁、不唤醒；只有 native 状态满足销毁前提时才销毁／释放存储，不能强行 free 忙碌的同步对象。相关活动调用保持 owner 为根，不能只把 native pointer 留在等待队列。

## 8. 阻塞网络与 SIGPIPE

### 8.1 地址与连接

`scoop.platform.net` 提供 IPv4／IPv6 地址、带端口的 SocketAddress、文本 parse／format、阻塞 resolve、TcpListener、TcpStream、UdpSocket。端口检查在 0..65535，bind 的 0 表示由 OS 分配；IPv6 保留必要的 scope id，不把地址文本当作长期身份。

DNS 通过 getaddrinfo 的真实错误域报告，可分配／阻塞的调用使用 NativeSafe。域名编码和 IP 字面量解析分开，首版不提供 IDNA、mDNS service discovery 或隐式网络访问的构造器。

TcpListener 提供 bind、localAddress、accept、dispose；TcpStream 提供 connect、local／peer address、ByteSource／ByteSink、shutdownInput／shutdownOutput 和 dispose。明确支持 reuseAddress、TCP noDelay／keepAlive 等基础选项；系统选项失败照常报告，不用成功 no-op 模拟。

TCP 是字节流，没有报文边界；read=0 表示对端输入方向 EOF，半关闭与整个 socket 关闭分开。读写遵守部分进度和无进度处理，不把一次 read 期望的长度当作对端承诺。

### 8.2 UDP

UdpSocket 提供 bind、localAddress、sendTo、receiveFrom 和 dispose。发送使用当前可读前缀形成一个 datagram，空 datagram 合法；接收的结果保存 source、实际复制字节数和 truncated 标志。

UDP 的零字节接收不是 EOF。接收缓冲不足时按 OS 报文规则消费该包并返回 truncated=true，游标只增加实际复制字节；不声称所有平台都能报告原报文的完整长度。零可写容量的 receive 在调用前拒绝，接收空报文可使用任意正容量 buffer。

单包发送不拆成多个 send，不用流式 writeFully 把一个报文拆成多个报文；超过 OS 限制或无法完成发送时报相应错误。数据提交以本次实际结果为准，不能在错误后悄悄再次发送整包造成重复。

### 8.3 SIGPIPE 与线程规则

safe pipe／socket 输出不能因为对端关闭而默认杀死整个进程，应该返回 BrokenPipe。Linux socket 使用 MSG_NOSIGNAL，Darwin 使用相应 socket 选项；普通 pipe／fd 写入在 Linux 使用线程局部屏蔽与消费，在 Darwin 使用实际 F_SETNOSIGPIPE 文件状态选项。Darwin 的 fd 写入产生进程定向 SIGPIPE，不能仅靠屏蔽当前线程保证安全；选项对同一打开文件描述的别名可见，包括标准流的 native 别名。

Linux shim 保存／恢复本线程原有 mask，只消费本次写入产生且此前未 pending 的 SIGPIPE，保留已有 pending 信号与原始 errno；不全局安装 SIG_IGN、不覆盖用户 signal handler。spawn 使用明确的子进程信号状态，不把临时屏蔽泄漏给子进程。

本期不公开任意 signal handler、SIGCHLD 回调或异步通知框架。必要的 SIGPIPE 原始行为通过隔离子进程／native 专测验证，普通 BrokenPipe 行为仍由 scoop.test 断言。

同一 File／pipe／TcpStream 可以在不与 close 竞争、且每次使用独立或已同步 buffer 的条件下调用 OS 支持的并发读写；文件共享偏移和数据顺序遵守 OS 行为，不额外承诺复合操作原子。close／配置改变与 IO 的并发需要外部同步，首版不提供从另一个线程 close 来取消阻塞 IO 的保证。

## 9. 时钟、sleep 与 OS 熵

`scoop.platform.time.SystemClock` 实现现有 Clock，返回 Instant；`MonotonicTimeSource` 实现现有 TimeSource，返回以自身为 source 的 TimeMark。真实时钟失败使用 PlatformException，不填一个零读数伪装成功。

wall clock 可以前进或回拨；monotonic tick 按目标单位转换，保持当前来源读数非递减。TimeMark 继续只允许同源比较／相减，不把其任意原点转换成 Unix epoch。

sleep(Duration) 是 NativeSafe 的阻塞操作，负时长拒绝、零时长直接返回；中断后按 monotonic 截止时间／剩余时长继续，允许超过请求时长，不保证精确唤醒时间。yield 是独立线程操作。elapsedNow、measureTime／measureTimedValue 使用明确的 TimeSource，回调首版普通同步；不增加 timer queue 或 scheduler。

`scoop.platform.random.SystemRandom` 实现现有 Random，使用 OS 熵，提供批量字节填充及 NativeByteBuffer 尾部填充。Linux 使用 getrandom 等目标 API，Darwin 使用系统熵 API；按单次限制分块，处理 EINTR／短结果，失败不能退化为时间种子或普通 PRNG。

nextBits 遵守现有 Random 的位数与值域合同，批量填充避免逐字节请求 OS。NativeByteBuffer 填充逐次只提交成功初始化的前缀；后续错误保留已经完成的游标进度，未写尾部仍不可读。使用者可以把 SystemRandom 显式交给 `Uuid.random(random)`，不新增隐式全局随机源或把其他 PRNG 宣称为系统熵。

Duration／Instant／TimeMark／Random／Uuid 与 ByteOrder 保持 std 中的原实体。时区数据库、DST、密码学算法和种子管理服务不属于本期。

## 10. 实施与验收约束

所有公共能力先有本文件对应的规范合同，再用 Scoop 和必要 C shim 实现。不得把某 target 的已承诺功能留成 TODO／空方法／“尚未支持”分支，也不通过 host 端预计算测试结果交付库。GC 按 [buffer 第 6 节](NATIVE-BUFFER.md#6-gc-pressure-factor-提示接口) 明确合同忽略 pressure、采用 no-op，不属于未实现占位。

实施过程中遇到任何编译器或 core 缺口，先修复该层并建立必要回归，再继续依赖它的 platform 工作；不能改库 API、强迫使用者显式标注类型、降低泛型能力、转入 C 实现普通 Scoop 逻辑或增加特殊调用路径绕过。涉及语言、runtime 或 pipeline 合同变化时先改 spec。

正常行为与运行期错误主要通过本 Cone 的 scoop.test 测试；为 native 生命周期或进程边界增加的专测只覆盖其必要合同。三个目标都实际运行，不能以 Darwin 的成功代表 Linux libc 差异已经验证。
