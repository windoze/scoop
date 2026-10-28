# Scoop 平台库规范

本文件规定普通 `scoop.platform` 库的公共合同，实施进度与三目标验收见 [M45](../milestone45/DESIGN.md)。语言、core、GC、FFI 与构建规则分别以 [语言规范](SCOOP-SPEC.md)、[运行时规范](SCOOP-RUNTIME-SPEC.md)和[实现规范](SCOOP-IMPL-SPEC.md)为准；平台无关的数据类型沿用[标准基础库规范](SCOOP-STDLIB-SPEC.md)。

## 1. 库与 native 边界

IO 端点、文件／进程观察权和 native buffer 使用 core Closeable.close，状态为 isClosed；继承的 dispose 默认转交 close，use 沿用语言规范 11.18。第 10 节的一般同步资源继续使用 Disposable。

`scoop:scoop.platform:0.1.0` 是普通 library Cone，位于 `sysroot/src/scoop.platform/`，生产依赖为 core 与 std，测试依赖为 scoop.test。应用显式声明依赖与 import；不默认导入，不改变 single-file 的 core-only 规则。公共逻辑用 Scoop 实现，C11 shim 只承担实际 OS、native allocation 与 ABI 操作，通过既有 native sources 发布，不新增 compiler-known role 或平台后端注册框架。

本期目标为 `aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。路径、参数与环境使用 POSIX 原始字节。阻塞操作走 NativeSafe FFI，只有符合语言规范 14 章的短操作才标记 GCLeaf。native 返回的错误在调用边界保存，不能晚些时候重新读取 errno。

库行为通过同 Cone 的 `tests/` 和 `scoop test` 验证；编译错误、真实产物消费及 GC／ABI／进程边界保留必要 fixture。普通库代码不要求重复整个编译器验收。原始验证日志不进入仓库，仅记录结果概要。

## 2. 系统字符串与路径

### 2.1 OsString

`scoop.platform.system.OsString` 是不可变 final class，实现 `Equals<OsString>` 和 `Hash`。内部保存原始字节；相等和 hash 只取决于字节内容，既不 Unicode 归一化，也不读取外部状态。

- `fromString(value: String): OsString` 按 UTF-8 编码。
- `fromByteArray(value: List<Byte>): OsString` 复制完整输入。
- `toByteArray(): Array<Byte>` 返回独立副本。
- `decodeUtf8(): String` 严格解码，非法输入抛 CharacterCodingException，保留相对于整个字节序列的错误位置。
- `decodeUtf8Lossy(): String` 显式替换坏序列；`display(): String` 使用该有损形式供诊断展示。展示结果不能作为原始系统调用输入的替代品。

OsString 可以保存任意字节，包括 NUL。实际进入 C 字符串位置的 API 在调用前拒绝内嵌 NUL；环境变量名另须非空且不含 `=`。拒绝抛 IllegalArgumentException，不能静默截断或改写输入。

Windows 原生字符串需要无损保留全部 UTF-16 code unit，包括非配对 surrogate。`OsString` 增加以下显式转换，已有字节工厂与 UTF-8 解码合同保持不变：

- `fromUtf16(value: List<UShort>): OsString` 复制输入并使用 [WTF-8](https://wtf-8.codeberg.page/) 表示：配对 surrogate 编码为一个普通 UTF-8 scalar，非配对 surrogate 保留其三字节编码，NUL 也保留。
- `toUtf16(): Array<UShort>` 将 UTF-8／WTF-8 转成独立 code-unit 数组，不替换非配对 surrogate。非法或非最短字节序列、超过 U+10FFFF 的值，以及分别编码的相邻高／低 surrogate 对，抛 CharacterCodingException；byteOffset 指向该非法序列的首字节。
- 两个转换在所有 target 上可用，互转保留任意 UTF-16 输入。有效 Unicode 的存储字节与 `fromString` 相同；相等和 hash 继续只取决于保存的字节。任意 POSIX 字节不能保证可转换为 UTF-16。

Windows OS 返回的原生名称使用上述无损表示；进入 Windows wide API 时需可转换为 UTF-16，不经过 ANSI code page 或有损 String。这个系统字符串表示不扩展 managed `String` 的 Unicode scalar 合同，也不放宽程序启动参数的严格解码规则。具体 Windows OS 接口的实施与验收见 [Windows targets](../windows-targets/DESIGN.md)。

### 2.2 Path

`scoop.platform.fs.Path` 是不可变 final class，实现 `Equals<Path>` 和 `Hash`。`fromString(String)` 与 `fromOsString(OsString)` 工厂保留原始字节并拒绝 NUL，不查询文件系统。相等和 hash 比较保存的字节，不折叠大小写、分隔符或 symlink。

| API | 合同 |
| --- | --- |
| `isAbsolute: Boolean` | 首字节是否为 `/`；空路径为 false |
| `components(): Array<OsString>` | 忽略空组件，保留 `.` 与 `..`；根由 isAbsolute 单独表示 |
| `fileName: OsString?` | 忽略尾部 `/` 后的最后组件；空路径、根、`.` 或 `..` 没有普通文件名 |
| `parent: Path?` | 去掉最后一个非空组件与尾部 `/`；单组件相对路径的 parent 是空路径，空路径与根没有 parent |
| `resolve(child: Path): Path` | absolute child 直接替换 base；否则连接字节并补必要的 `/`；空 child 保留 base |
| `normalizeLexically(): Path` | 消除空组件与 `.`，用 `..` 抵消前一个普通组件；相对路径保留未配对的 `..`，absolute 路径不越过根；空的相对结果为 `.` |
| `toOsString(): OsString` | 返回原始字节的不可变系统字符串 |
| `decodeUtf8()`／`decodeUtf8Lossy()`／`display()` | 与 OsString 对应操作一致 |

词法归一化不解析 symlink，不保证保持 symlink 路径的 OS 解析结果，不是权限检查。canonicalize 是后续文件系统 API 的显式操作。相对路径在实际 OS 调用时按当时的 cwd 解析。

以上表格描述 POSIX 词法规则。Windows 按目标而非宿主使用以下规则，构造仍只拒绝 NUL，不访问文件系统或替 OS 检查名称是否合法：

- 普通路径同时识别 `/` 和 `\` 分隔符。`C:\a`、完整 UNC `\\server\share\a` 是 absolute；`C:a` 为指定盘符的相对路径，`\a`／`/a` 为当前盘符的 rooted 路径，二者的 isAbsolute 均为 false。
- 盘符和 UNC server/share 属于前缀，components 不包含前缀或根。fileName／parent 不越过前缀或根；`C:a` 的 parent 为 `C:`，`C:\a` 的 parent 为 `C:\`，UNC 根没有 parent。
- resolve 的带前缀 child 替换 base，包括 drive-relative child；没有前缀但 rooted 的 child 保留 base 的前缀并替换其余内容。普通 relative child 以 `\` 连接，只有 base 恰为 `C:` 一类 drive-relative 前缀时不插入分隔符。空 child 保留 base。
- normalizeLexically 保留盘符／UNC 前缀，使用 `\` 连接组件，消除重复分隔符和 `.`，折叠 `..`；rooted 路径不越过根，drive-relative 路径保留未配对 `..`。空的普通 relative 结果为 `.`；只剩 drive-relative 前缀时保留该前缀。
- `\\?\` 与 `\\.\` 命名空间路径保留前缀，只将 `\` 作为分隔符；前者识别 drive 与 `UNC\server\share`，其他名称以命名空间后的首组件作为前缀。其 normalizeLexically 返回原路径，避免改写交给 OS 的字面名称。向这种 base 连接普通 child 时，将 child 的分隔符写为 `\`，不折叠其 dot 组件。比较与 hash 始终使用原始字节，不把不同拼写当成相等路径。

## 3. 系统错误

`scoop.platform.system.PlatformErrorKind` 包含 NotFound、AlreadyExists、PermissionDenied、NotDirectory、IsDirectory、InvalidInput、NoMemory、NoSpace、BrokenPipe、ConnectionRefused、ConnectionReset、AddressInUse、AddressUnavailable、TimedOut、Interrupted、WouldBlock、Unsupported、Other。未知错误归 Other，必须保留原始错误数据；Unsupported 仅用于 OS 实际报告不支持。

`NativeErrorDomain` 区分 PosixErrno、AddressResolution、Win32、Winsock 与 NtStatus。普通值类型 `NativeError(domain, code, systemCode: Int? = None)` 保存原始 Int 错误码；AddressResolution 的 POSIX EAI_SYSTEM 还保存同次失败的 errno，Windows 保存地址解析函数直接返回的 WSA／EAI 码，不另取 errno。只读属性 `kind` 依据目标系统的实际常量分类，不假设两个 target 的 errno／EAI 数值相同；当前目标没有的错误域分类为 Other，仍保存原值。pthread 直接返回的 errno 风格错误属于 PosixErrno，不能改读线程 errno。NtStatus 保存 BCrypt 等 API 直接返回的 NTSTATUS 全部 32 位，不将它当作 GetLastError 或翻译成另一错误码；未知值仍归 Other。

Win32 保存失败调用后立即取得的 GetLastError，Winsock 保存立即取得的 WSAGetLastError；DWORD 的完整位模式保存在 Int 中。native shim 在任何清理、回调或返回 managed 代码之前保存错误，不将 Windows 错误翻译成伪造 errno，也不通过后续独立 FFI 调用重新读取线程错误。Windows CRT 本身产生的 errno 仍属于 PosixErrno。EOF、pending、取消和 close 是具体操作协议的结果，不因有相应 native code 就一律当成失败。错误域分类不会改写原始 code。

asyncio 的 syscall 与 io_uring 负结果均转换为 PosixErrno；保留实际 errno，不再提供失去消费者的 Libuv 错误域。EOF 与主动取消／关闭由 I/O 协议解释。

`PlatformException(operation: String, kind: PlatformErrorKind, nativeError: NativeError? = None, context: OsString? = None)` 继承 Exception，以上四项为只读字段。调用方按结构字段处理错误；message 只供诊断，不作为可解析协议。context 用于保存实际路径或端点文本等调用上下文，不覆盖原始 code。

非法参数、越界、算术溢出、对象状态错误分别沿用 core 的 IllegalArgumentException、IndexOutOfBoundsException、ArithmeticException、IllegalStateException，不包装成伪造的 OS 错误。IO 的 EOF／无进度错误由对应 IO 协议另行规定。

## 4. NativeByteBuffer

`scoop.platform.memory.NativeByteBuffer` 是实现 Closeable 的普通 final class，独占连续 native 字节存储。class 引用复制共享同一 owner、游标和关闭状态，不复制存储。`allocate(capacity: Long = 0L)` 创建精确请求容量、两个游标为零的对象；零容量不分配。负容量抛 IllegalArgumentException，native 分配失败抛 kind 为 NoMemory 的 PlatformException，保留调用边界捕获的 errno。字节区不保存 managed reference，也不承诺清零。

### 4.1 游标、初始化与释放

open 对象保持 `0 <= readPosition <= writePosition <= capacity`，只有 `[0, writePosition)` 已初始化。只读属性为 capacity、readPosition、writePosition、readableBytes、writableBytes 和 isClosed；不存在扩大 writer 的 safe setter。关闭后仅 isClosed、close 可调用，其余操作及查询抛 IllegalStateException。

| 操作 | 合同 |
| --- | --- |
| clear() | 游标归零，保留 allocation，不清零；旧数据不能再由 safe API 读取 |
| compact() | 以 memmove 语义将未读区移到起点，readPosition=0，writePosition=原 readableBytes |
| skip(count: Long) | 消费可读前缀；负值非法，超出可读区越界 |
| ensureWritable(count: Long) | 显式保证尾部空间，不隐式 compact；成功增长保留内容和游标，失败保留地址、容量、内容与游标 |
| close() | 幂等；先把 owner 置 inert，再 free；不 flush，不调用用户代码 |

仅 ensureWritable 会扩容，普通写入在空间不足时越界。增长倍率不是合同，容量至少为 writePosition+count；所有大小运算在地址计算前检查 Long 溢出并抛 ArithmeticException。首版没有缩容、共享 view、slice 或 duplicate。GC-free release hook 只在正常回收时兜底 free；不保证触发时间，不代替显式释放。hook 只读字段，显式关闭后 hook 不重复释放。

### 4.2 字节、数组与数值

- readByte()/writeByte(Byte) 顺序访问单字节；getByte(index: Long)/setByte(index: Long, value: Byte) 只访问已初始化的绝对下标，不改游标。
- getBytes(index: Long, count: Long): Array<Byte> 返回已初始化绝对区间的独立副本，不改读写游标，也可读取已消费的前缀。count 非负，index 位于 `[0, writePosition]`，且 count 不超过 `writePosition-index`；writer 位置仅允许空区间。负 count 抛 IllegalArgumentException，区间越界抛 IndexOutOfBoundsException。范围检查与复制使用一次状态同步，借用期间仍可读取已初始化区域；关闭后即使空区间也抛 IllegalStateException。
- readBytes(count: Long): Array<Byte> 返回独立副本，分配与复制成功后才消费输入。
- readInto(array: MutableArray<Byte>, offset: Long = 0L, count: Long = array.size-offset) 复制到受检数组区间；writeBytes(array: List<Byte>, offset: Long = 0L, count: Long = array.size-offset) 接收只读字节列表。先验证两侧区间，在现有 writable 借用内、状态锁外取得所选输入区间的快照后复制，完成后才提交游标；用户 getter 可查询缓冲区，重入修改遵守 4.3 的借用规则；count 为负抛 IllegalArgumentException，数组区间非法抛 IndexOutOfBoundsException。
- read/writeInt8、UInt8、Int16、UInt16、Int32、UInt32、Int64、UInt64、Float、Double 提供相应定宽读写；write 的首参数为 value，除单字节外都必须显式传入 `scoop.bytes.ByteOrder`。浮点按位型编码，包括 NaN 与负零。

顺序读写先检查完整宽度，不留下半个数值或半次游标变化。超出可读／可写范围抛 IndexOutOfBoundsException，负数量抛 IllegalArgumentException。任意 byte offset 都合法，不依赖数值自然对齐；实现逐字节组合或复制到对齐局部，禁止对未对齐的 typed pointer 直接 load/store。

### 4.3 同步 native 借用

`@Unsafe fun <R> withReadableBytes(block: (Ptr<Byte>?, Long) -> R): R` 提供当前可读前缀，不消费游标；调用方承诺只读。`@Unsafe fun withWritableBytes(block: (Ptr<Byte>?, Long) -> Long): Long` 提供可写尾部。两个入口都恰好调用 block 一次，零长度传 None/0，非空传 Some(pointer)/length。

writable block 必须初始化返回值 n 指定的完整前缀；仅正常返回且 `0 <= n <= writableBytes` 时提交 writePosition。无效 n 抛 IllegalArgumentException，抛异常或无效返回都不提交游标，已修改的尾部仍不能 safe 读取。库只检查范围，不证明 native 初始化事实。

block 是普通非 suspend callback，可分配 managed 对象、抛异常及进行同步 C 调用。owner 跨 callback 与 finally 保持 live root，GC 可移动 wrapper；native 地址保持稳定，不 pin wrapper。finally 清除借用状态。借用中拒绝 close、ensureWritable、clear、compact、skip、顺序读写、绝对写和嵌套借用，修改前抛 IllegalStateException；只读属性及 getByte／getBytes 仍可用。

pointer 不得逃出 callback、保留给异步操作或跨挂起使用；writable callback 不读取未初始化尾部或改写借用区外字节。同步与异步借用共同使用 4.5 的状态同步边界；callback 在状态锁外执行。不增加逃逸分析、静态 borrow checker 或 managed ownership 类型。

### 4.4 GC pressure

成功提交容量 c 后通过 core `gcAdjustPressure(+c)` 提示，增长只报告 new-old；实际释放后报告 -c。失败、clear、compact、读写和借用不报告变化；同一份所有权不得重复扣减。显式关闭先清空字段，hook 只读字段并在 collector reclaim 路径释放，二者共用同一个 GC-free 入口。

提示遵守语言规范 14.1 与运行时规范 3.1；首版 GC 完全忽略提示是合法实现。buffer 不触发 GC，也不在 malloc/realloc 失败后收集重试，不维护 RSS、分配峰值、通用预算或资源账本。

### 4.5 跨挂起 native 借用

NativeByteBuffer 新增普通公开 `@Unsafe borrowReadable(count: Long): NativeByteBorrow` 和 `@Unsafe borrowWritable(count: Long): NativeByteBorrow`。count 非负且不超过当前可读／可写区间；登记与所有同步访问使用同一状态锁。登记成功后锁定整个 owner 的借用状态，禁止另一借用及 4.3 列出的所有变更，包含零长度变更和 close；查询与已初始化字节的只读访问继续允许。状态锁不跨 callback 或挂起持有，复合的多个用户操作仍需调用方自行同步。

`NativeByteBorrow` 是不可由调用方构造的普通 final class，持有 owner、独立 native allocation 的稳定地址、方向及公开只读 `length: Long`。`@Unsafe pointer(): Ptr<Byte>?` 返回本次区间的首地址，零长度返回 None；在借用结束后调用抛 IllegalStateException。它可以跨挂起保活 owner，不 pin 可移动的 managed wrapper。readable 调用方只能读取已初始化前缀，writable 调用方只写借用的尾部。

`@Unsafe complete(count: Long): Unit` 表示 native 已不再使用该地址，提交实际进度后解除借用；readable 推进 reader，writable 推进 writer。writable 的 count 前缀必须已被初始化。取消或其他异常也先按实际已知进度完成借用再向调用方结束等待；complete 本身不取消 native 操作。只有一个 complete 调用能消费本次借用，再次或并发重复调用抛 IllegalStateException。count 不在 `0..length` 时不提交进度、解除借用并抛 IllegalArgumentException；失败后不可重试同一借用。

取得借用后，调用方必须持有它直到 native 收尾完成，并在全部出口恰好消费一次；不能在 complete 后继续使用先前取得的指针。丢弃借用不执行隐式异步清理。分配或登记失败不改变 owner 的借用状态。同步 4.3 入口保持原签名与游标合同；它们通过同一借用机制在 callback 的正常／异常出口完成收尾。

## 5. 同步字节流、缓冲与 UTF-8

公开参数名属于库 API：override 保留接口的参数名，同步／异步对应入口使用相同名称与次序。流传输使用 buffer、count；copyTo 使用 destination、bufferSize；数组区间使用 offset、count。命名调用按语言规范 8.5 的静态声明解析，不改变 override 的语言规则。

`scoop.platform.io.ByteSource` 声明 `readInto(buffer: NativeByteBuffer, count: Long): Long`；ByteSink 声明 `writeFrom(buffer: NativeByteBuffer, count: Long): Long` 与 flush()。协议不包含关闭；拥有资源的实现另实现 Closeable。count 非负且不超过当前可写／可读区间，零请求不执行 native IO、返回零，也不表示 EOF。正请求允许短传输；read 返回零表示 EOF，write 无进度抛 WriteZeroException。成功 read 提交 writer，成功 write 提交 reader；失败不撤销之前已经发生的外部进度。

普通扩展提供省略 count 的调用、readFully/writeFully、copyTo、readAllBytes、readText/readTextLossy、readLine/readLineLossy、writeText/writeLine。完整读取遇到 EOF 抛 UnexpectedEofException；两种 IO 异常直接继承 Exception。完整操作逐次提交进度，后续失败保留此前游标，不忙循环。copyTo 返回复制的 Long 字节数；bufferSize 默认 8192，必须为正，不是总数据配额。高层只读字节输入为 List<Byte>，快照输出为 Array<Byte>，可写目标为 MutableArray<Byte>。writeFrom(array, offset, count) 检查区间并取得所选输入区间的快照，用户 getter 失败发生在传输之前；readInto 的数组目标保持可写。writeAllBytes 接收 List<Byte>。flush 只提交用户态缓冲，不等于文件持久化。

UTF-8 在完整结果或整行组装后解码，跨 native 分块的编码序列不会被提前判错。默认严格解码；有损版本显式命名。readLine 去除 LF 及紧邻它的 CR，EOF 前无 LF 的最后一行保留全部字节；仅 EOF 且本行无字节返回 None。writeLine 追加 LF。`Utf8Writer(sink: ByteSink)` 是借用 sink 的 TextWriter 适配器，write 转交 UTF-8 完整写入，flush 转交 sink，不拥有或关闭 sink。

`BufferedSource(source, capacity: Long = 8192L)`、`BufferedSink(sink, capacity: Long = 8192L)` 为 final class 和 Closeable，容量必须为正。默认仅拥有自己的 NativeByteBuffer，借用 endpoint。`takeOwnership<T>(endpoint: T, capacity)` 要求 T 同时符合对应 IO 协议与 Closeable；成功返回才移交逻辑关闭责任。没有共享使用／并发关闭保证，调用方须同步；重入操作在修改前拒绝。关闭后操作抛 IllegalStateException，isClosed 可查询，close 幂等。

BufferedSource 不把未读数据交回 endpoint；BufferedSink.close 尝试 flush，随后无论是否失败都释放 buffer 并关闭明确接管的 endpoint。多个失败以 ResourceUseException 按发生顺序组合；逻辑关闭后不重复清理或报告旧异常。flush 失败保留尚未输出的缓冲和已发生的外部进度，允许未关闭的 caller 明确重试。GC hook 不 flush 或访问 managed endpoint，内部 buffer 自行兜底释放。

standardInput、standardOutput、standardError 是使用调用时 fd 0/1/2 的借用视图，不实现 Closeable。duplicateStandardInput/Output/Error 返回独立拥有、close-on-exec 的 File 副本，与原描述符共享 OS 文件位置／状态，关闭副本不关闭标准 fd。标准流重定向后，后续操作使用新 fd；与 core libc stdio 混用时需要显式 flush，不承诺自动排序。

### 5.1 终端库共用的 native TTY 资源

M59 的同步 terminal 与 asyncio TTY 共用 platform 中的 native TTY 实现，
包括独立打开、每进程独占、实际 termios 快照、尺寸、非阻塞字节 I/O、resize
通知和关闭。打开自有 TTY 句柄不取得或改变进程的控制终端，包括没有控制终端
的会话首进程。此处只提供两个库实际使用的 C helper，不增加公开 Scoop fd 接管
或通用 signal API。原生实现及同步公开合同见 [终端规范](SCOOP-TERMINAL-SPEC.md)第 7 节。

TTY 与通知 fd 属于同一个 opaque owner；同步 poll 和异步平台后端分别借用，
不能另行关闭。异步必须先退休所有异步观察与控制请求、结束 native 操作，
再恢复 termios 并释放 owner。每进程独占状态由该共同实现维护，因此同步与
异步会话不能各自成功打开一个竞争读取者。该限制不约束普通 stdout 输出。

## 6. 文件与目录

### 6.1 File 与 OpenOptions

`scoop.platform.fs.File` 为 final class，实现 ByteSource、ByteSink、Closeable。`File.open(path, options: OpenOptions = OpenOptions())` 创建 close-on-exec fd。OpenOptions 为值类型，默认 read=true，其余 write/append/create/createNew/truncate 标志默认为 false，mode 默认为 0666；至少允许一种读写，append/truncate/create/createNew 要求 write，append 与 truncate 冲突，mode 只允许 POSIX 07777 位。createNew 使用原生排他创建，优先于 create；实际 mode 仍受 umask 影响。

除字节协议外，File 提供 position()、size() 外部查询函数与 isClosed 状态属性，List<Byte> 写入／MutableArray<Byte> 读取重载，以及 seek(position: Long)、truncate(size: Long)、syncData、syncAll、metadata、duplicate、close。seek 为绝对非负偏移，truncate 非负且不改变当前文件位置；append 通过 OS append 标志实现。flush 对原始 fd 无用户态缓冲可提交，但仍检查 open 状态。同步操作都保活 owner 直到 native 返回和退出清理，不能仅保留整数 fd。与关闭不竞争、各次使用独立或已同步 buffer 时，同一 File 允许 OS 支持的并发 IO；共享偏移与数据顺序遵守 OS 行为。活动调用期间重入 close 会拒绝，但关闭／配置改变与 IO 的跨线程竞争仍须调用方同步，不提供通过 close 取消阻塞 IO 的保证。

显式关闭先清除 owned fd，再调用一次 close；成功或失败均终结 owner，不无条件重试，包括 EINTR。当前 Linux 的 fd 在 close 错误报告前已被消费；Darwin 的普通 fd 关闭同样沿实际内核消费路径，不能由错误码假定仍可重试。hook 只兜底尝试 raw close，不重放已消费的 fd，不 flush 或等待用户代码。close 结果不能证明远程文件已经持久化，必要时先显式 sync。

File.size()／position() 和同步 socket 的地址／选项查询访问当前 OS 状态，可能失败，因此采用函数。isClosed 只读取对象自身状态，保持属性；FileMetadata.size 等已取得的值也保持属性。

文件协调提供 `File.lock(shared = false)`、`tryLock(shared = false)`、`unlock()`
与作用域 `withLock(shared = false) { ... }`。Unix 使用 flock advisory 整文件锁：
排他锁互斥，共享锁允许并行读取；tryLock 仅竞争失败返回 false，其他 OS 错误传播，
阻塞操作重试 EINTR。锁与打开的文件描述关联，复制 descriptor 共享锁，最后关闭释放。
withLock 在正文所有出口解锁，正文与解锁同时失败沿 ResourceUseException；
调用者不得嵌套或并发操作同一 owner 的锁作用域。它不阻止不遵循锁协议的其他写入者。

### 6.2 文件系统操作

`scoop.platform.fs` 提供 Path 扩展 readAllBytes/readText/readTextLossy、writeAllBytes/writeText、copyTo(destination, replaceExisting = false)、moveTo(destination, replaceExisting = false)、delete/deleteRecursively、metadata/exists、createDirectory/createDirectories、readSymbolicLink/createSymbolicLink(target)、canonicalize、entries。接收者是被读取、修改或作为源的 Path；File.open(path, options) 仍负责创建文件句柄。createTempFile/createTempDirectory 是同 package 的顶层工厂。普通 import 即可使用这些入口，不保留 Files 工具 object。

- writeAllBytes/writeText 默认创建或截断，支持显式 append；copyTo 默认不替换，replaceExisting=true 才允许覆盖。copyTo 不承诺原子性或复制全部 metadata，不允许把同一 OS 文件作为源与目标后先截断源内容。
- moveTo 默认不替换，使用 Darwin renamex_np(RENAME_EXCL) 或 Linux renameat2(RENAME_NOREPLACE) 的真实排他操作；replaceExisting=true 使用 rename。跨文件系统与不支持的错误照常报告，不以有竞争的 exists 预检查模拟排他，不暗中 copy+delete。
- Path.metadata(followLinks=true) 返回 FileMetadata。FileKind 区分 Regular、Directory、SymbolicLink、BlockDevice、CharacterDevice、Fifo、Socket、Other；metadata 包含 size、permissions、accessedAt、modifiedAt、changedAt 和可选 createdAt，时间复用 scoop.time.Instant。Darwin 使用 birth time，Linux stat 不提供 birth time 时为 None；不可用时间不以零冒充有效值。
- exists 仅把 NotFound 和路径组件的 NotDirectory 转为 false，权限与其他 IO 错误继续抛出。查询不消除之后操作的文件系统竞争。
- createDirectory 默认 mode=0777，受 umask 影响；createDirectories 接受已存在目录，其他类型照常报错。delete 删除单个文件、symlink 或空目录；deleteRecursively 不主动跟随 symlink，调用方须排除并发替换目录树，API 不提供抵抗恶意路径替换的来源审计或安全沙箱。
- Path.createSymbolicLink(target) 保留 target 字节；readSymbolicLink 不解析目标，canonicalize 才解析实际路径与 symlink。所有 Path 都保留原始名称字节；实际文件系统可以拒绝非法编码，库保留该 OS 错误与原始 context，不提前损坏或重新编码路径。
- createTempFile/createTempDirectory(parent: Path? = None, prefix: String = "scoop-") 使用 OS 排他创建，prefix 不含 NUL 或 `/`。默认 parent 使用系统临时目录查询，失败照常报告；返回实际 Path，不因关闭或 GC 自动删除。临时文件 mode 为 0600，目录为 0700，仍受 umask 约束。

### 6.3 DirectoryEntries

`DirectoryEntries` 实现 Iterable<DirectoryEntry>、Iterator<DirectoryEntry> 和 Closeable，是共享 native 枚举状态的单遍对象；由 path.entries() 创建。iterator 返回自身，next 返回 Option；跳过点目录，不排序。DirectoryEntry 保存独立 OsString name 与组合后的 Path path，不把 readdir 临时指针交给调用者。

EOF 时自动关闭枚举句柄，之后 next 仍返回 None；提前 close 后再 next 抛 IllegalStateException。提前退出循环或正文失败由 use 清理，for 不自动关闭 iterator。native 枚举和复制名称期间保活 owner；不支持重入／并发枚举或关闭。hook 只兜底 closedir。

### 6.4 native 传输、信号与错误

阻塞 open/read/write、文件 metadata／同步、目录和路径操作使用 NativeSafe。read/write 只在没有交付字节的 EINTR 时重试，一次传输受平台 ssize_t／实际 syscall 上限限制。errno 在本次失败后立即保存，任何信号状态恢复或返回握手都不能覆盖它。

供 asyncio 的实际文件工作复用，`scoop.platform.native.NativeFileMetadata` 公开既有 GC-free CLayout 记录：kind、permissions 为 Int，其余 size 及四组秒／纳秒为 Long；创建时间纳秒为负表示不可用。普通 `decodeFileMetadata(data, error, context)` 将它转换为 FileMetadata，复用同步文件的种类、时间和错误映射；失败记录的 kind 为负，error 是原调用中保存的 errno。共享 C 头声明既有 metadata、seek、truncate helper，不在 asyncio 再实现 stat 字段转换或 OS 偏移规则。

共享路径头公开既有 `path_mkdir`、`path_delete`、`path_move` helper，分别沿 mkdir、remove 和排他／替换 rename 的 OS 合同。`scoop.platform.native.requireFileMode(mode: Int): Unit` 统一同步文件、目录与 asyncio 的权限参数检查；有效范围为 0..0xfff，非法值抛 IllegalArgumentException，不增加独立的文件系统规则。

`scoop.platform.native.temporaryPathPattern(parent: Path, prefix: String): Path` 检查 prefix 并构造尾部为 XXXXXX 的路径模板；默认父目录复用既有 `scoop.platform.env.temporaryDirectory()`，这些步骤不创建文件。同步临时路径工厂与 asyncio 共用这些步骤。共享 `scoop_platform_file_temporary(char *pattern)` 通过 mkostemp(O_CLOEXEC) 一次排他创建并返回仍然打开的读写 fd，实际名称写回调用者拥有的模板；文件初始权限为 0600、长度与位置为零。原同步 createTempFile 复用该 helper 后关闭 fd，异步工厂则交付文件 owner。

`native/directory.h` 声明既有 directory_open／directory_next／directory_close helper，供 asyncio 调度同一 opendir／readdir／closedir 实现。next 跳过点目录，返回名称借用到本句柄的下一次读取或关闭之前；NULL 且 errno 为零表示 EOF，NULL 且 errno 非零表示错误。异步调用者必须在同一次串行工作内复制名称，不跨工作保留 readdir 的内部存储。

safe fd 输出不因断开的 pipe/socket 默认杀死进程。Linux 普通 fd 写入仅屏蔽当前线程的 SIGPIPE，保存旧 mask/pending，只消费本次写入新增的 SIGPIPE，再恢复 mask；不消费已有 pending，也不安装进程级 handler。Darwin 内核的普通 fd 写入发出进程定向 SIGPIPE，使用真实 F_SETNOSIGPIPE 选项抑制；该选项属于打开文件描述，同一描述的别名可见，标准输出也在实际写入当前 fd 时设置。socket 专用输出使用对应 MSG_NOSIGNAL/SO_NOSIGPIPE。所有目标保持用户 signal handler，不以成功 no-op 模拟保护。

目标依据：[Linux close](https://man7.org/linux/man-pages/man2/close.2.html)、[Linux rename](https://man7.org/linux/man-pages/man2/rename.2.html)、[Darwin fd 生命周期](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_descrip.c)、[Darwin 写入与 SIGPIPE](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/sys_generic.c)。这些边界按实际 OS 合同实现，不推导新的 compiler/runtime 协议。

## 7. 原始参数、环境与系统信息

`scoop.platform.env.arguments(): Array<OsString>` 复制包括 argv[0] 的原始进程参数，复用 runtime 的只读 argc/argv 入口。无参数 main 可以保留坏 UTF-8 参数；有参数 main 的启动解码规则保持语言／runtime 规范不变。

`environmentVariable(name: OsString): OsString?` 及 String 便利重载区分缺失与空值，名称非空且不含 NUL 或 `=`。`environment(): Map<OsString, OsString>` 返回独立只读快照；按第一个 `=` 拆分，不损坏原始字节。仅包含有非空名称和分隔符的环境项，同名项以第一个为准，与单项查询一致。没有进程全局环境修改 API；调用者不能同时通过外部 native 代码违反 libc 环境访问的线程合同，库内锁不能代替该前提。

currentDirectory(): Path 查询实际 cwd，不读取 PWD；homeDirectory(): Path? 优先使用非空 HOME，否则查询当前真实 uid 的 passwd 目录，没有记录或空目录时返回 None，实际查询错误抛 PlatformException。temporaryDirectory(): Path 使用非空 TMPDIR，否则为 `/tmp`。查询返回字节副本，不保证路径存在、可访问或之后仍可创建文件；不提供全局 chdir。

`scoop.platform.system` 提供 OperatingSystem(Darwin/Linux/Windows)、Architecture(Aarch64/X86_64)、operatingSystem 与 architecture 只读值，标识当前构建目标。POSIX 的 systemVersion(): String 返回 uname 的系统 release，logicalProcessorCount: Long 返回 OS 报告的在线逻辑 CPU 数；失败照常抛系统错误，不推导容器配额或调度能力。

## 8. 时钟与阻塞 sleep

`scoop.platform.time.SystemClock` 实现 scoop.time.Clock，now 返回实时钟的 Instant；`MonotonicTimeSource` 实现 TimeSource，markNow 返回以自身为 source、以 CLOCK_MONOTONIC 读数构造 Duration 的 TimeMark。实时钟可以回拨，monotonic 不与 epoch 混用，同源比较仍由 std 检查。native 读取失败抛 PlatformException，不伪造零读数；转换遵守 std 的范围规则。

Windows 实时钟使用 GetSystemTimePreciseAsFileTime，将 1601 epoch 的 100 ns tick 转为 Unix epoch 的规范化秒／纳秒；monotonic 使用 QueryPerformanceCounter／QueryPerformanceFrequency，整数缩放不溢出或丢失整秒精度。native 时钟结果同时保存本次错误码，Windows 使用 Win32，POSIX 使用原 errno。

sleep(duration: Duration) 拒绝负值，零直接返回。sleep 与 ConditionVariable 的 native 时间分量使用 Duration.floorSeconds／nanosecondsOfSecond；toComponents 是面向调用者的趋零分解，不用于替代规范化的 native 分量。正值通过 NativeSafe 等待；使用 monotonic 截止时间在 EINTR 后继续，允许调度造成延迟，不承诺精确唤醒。截止时间超出目标表示范围时报告溢出错误；不以通用超时配额限制有效时长。等待不阻止其他 mutator 推进或 GC。

Windows 将剩余时长按毫秒向上取整后 Sleep，单次最多 INFINITE-1 毫秒；每次醒来重新读取 QPC，直到 monotonic 截止时间，避免 tick 粒度导致提前返回，也不把有限时长转换成无限等待。超出秒截止时间表示范围报告 ERROR_ARITHMETIC_OVERFLOW；不改变全局 timer resolution。

TimeMark.elapsedNow() 为 `source.markNow() - this`。measureTime(source: TimeSource = MonotonicTimeSource, block: () -> Unit) 与 measureTimedValue<T>(source: TimeSource = MonotonicTimeSource, block: () -> T) 同步调用正文一次，后者返回 TimedValue<T>(value, duration)；正文失败直接传播，不额外读取时钟覆盖原异常。不支持 suspend、timer queue 或调度器。

## 9. 系统熵

`scoop.platform.random.SystemRandom` 实现 scoop.random.Random，nextBits 按 0..32 的既有合同返回位值；0 不调用 OS。覆盖 nextBytes(size) 与 nextBytes(MutableArray, fromIndex, toIndex) 的批量路径，不逐字节请求。fill(buffer: NativeByteBuffer, count: Long = buffer.writableBytes) 检查当前尾部容量，返回成功填充数量；零请求不执行 native 操作。

Linux 使用 getrandom、Darwin 使用 getentropy，按目标单次上限分块并重试未产生数据的 EINTR；允许短结果后继续。NativeByteBuffer 每次只提交本次成功的前缀，后续失败保留已提交进度，未初始化尾部仍不可读；数组失败保留已写区间。native 正请求无进度作为 IO 错误处理，不忙循环或退化为 PRNG。所有系统熵操作使用 NativeSafe，可以与 GC 并行。Uuid.random(SystemRandom) 与既有 Random 默认方法沿普通接口组合，不增加隐式随机源或密码学算法。

Windows 使用 BCryptGenRandom 的 BCRYPT_USE_SYSTEM_PREFERRED_RNG，按 ULONG 长度范围分块；失败同次返回原 NTSTATUS，成功后才提交本段进度。按 target 声明系统 bcrypt 导入库，不管理额外 provider handle 或使用 CRT PRNG 代替系统熵。

目标依据：[getentropy](https://man7.org/linux/man-pages/man3/getentropy.3.html)、[nanosleep](https://man7.org/linux/man-pages/man2/nanosleep.2.html)、[clock_nanosleep](https://man7.org/linux/man-pages/man2/clock_nanosleep.2.html) 以及当前目标 SDK。环境、时钟和熵均不新增 runtime ABI。

## 10. 线程与同步

`scoop.platform.concurrent.Thread.start(block: () -> Unit): Thread` 启动 owned native 线程，捕获启动时 TaskContext。普通 completion 对象保存正文原始 Throwable 和原子完成标志；OneShot callback 自行捕获失败，worker ownership 在调用中消费，不额外保留只能由 dispose 清理的 callback observer。创建失败释放未转移的 callback ownership 和 native 控制记录。

Thread 实现 Disposable，提供 isFinished、join、dispose。isFinished 表示 managed 正文已完成，以 release/acquire 发布；join 仍等待整个 pthread 退出并回收 native 观察权，再抛出正文原始异常。重复 join 使用缓存结果，失败也保留对象身份；dispose 在未回收时 join，已回收时不重复抛旧异常。self-join 报 native 错误且不消费观察权。调用方串行化 join/dispose；isFinished 不访问可能被 join 释放的 native 记录。

Thread hook 只做非阻塞 pthread_detach 并释放 native 观察权。控制记录由 worker 与观察者分别持有原生存活权，最后一方释放；不保存 managed 指针、不等待正文、不操作 callback ownership。丢弃 Thread 不承诺 worker 已完成或 runtime 正常退出；程序需显式 join/use。currentThreadId(): ThreadId 返回有 Equals/Hash 的不透明 pthread 标识，线程结束后允许复用，不等同于进程 id；yield() 请求让出当前 native 线程。

Windows 使用所选 CRT 的 `_beginthreadex`，join 以 WaitForSingleObject 等待后 CloseHandle，hook 只 CloseHandle 并释放观察权。两份 native 存活权与 callback 规则保持；currentThreadId 使用系统 thread ID，yield 使用 SwitchToThread，没有可切换线程不算失败。创建和分配失败保留 CRT 的实际 errno；join、mutex 与 condition 的 Windows 失败属于 Win32，self-join 报 ERROR_POSSIBLE_DEADLOCK 且保留观察权。

Mutex() 为 final Disposable class，使用非递归 ERRORCHECK mutex，提供 lock、tryLock、unlock、withLock<T>(block: () -> T)、isDisposed。tryLock 忙时返回 false；重入 lock 和非持有者 unlock 报 native 错误。withLock 在 finally 解锁，支持泛型结果和异常，只有普通同步回调，没有 suspend 重载。

ConditionVariable(mutex: Mutex) 保活并绑定一个 Mutex，提供 await()、await(timeout: Duration): Boolean、signal、broadcast、dispose 和 isDisposed。await 要求当前线程持有该 mutex，原子释放／等待／重新取得；超时重载 true 表示唤醒，false 表示超时。任何返回都不能证明谓词成立，调用方使用 while 循环。负时长拒绝，零超时仍执行释放／重新取得；Linux 使用 CLOCK_MONOTONIC cond clock，Darwin 使用相对等待接口，避免 realtime 校时改变超时。超出目标时间表示范围按实际错误报告。

同步对象的活动调用保活 owner；ConditionVariable.await 同时保活相关 Mutex，native 等待使用 NativeSafe。dispose 要求没有并发操作，mutex 未持有、cond 无等待者；不满足时拒绝并保留对象。关闭成功才清除 owner，重复关闭无操作。Mutex 的 native 持有状态避免对忙碌 mutex 调用具有未定义行为的 destroy；hook 不解锁、不唤醒，只在合法销毁前提满足时释放，忙碌对象不被强行 free。禁止外部 native 代码直接操作这些私有同步句柄。

Windows mutex 使用 SRW exclusive lock，并记录 owner 以实现相同的非递归／错误检查合同；同线程 tryLock 返回 false，重入 lock、非持有者 unlock、持有时 destroy 分别报告 ERROR_POSSIBLE_DEADLOCK、ERROR_NOT_OWNER、ERROR_BUSY。condition 使用 SleepConditionVariableSRW，唤醒后恢复 owner。定时等待按毫秒向上取整，单次最多等待 INFINITE-1 毫秒，不把有限等待误转成 INFINITE；以 GetTickCount64 的 monotonic 经过时间判断总时长是否到期，单次到期但总时长未到时按允许的伪唤醒返回，让调用方重查谓词，不在库内重新等待而丢失两次等待之间的通知。无法用 64-bit 毫秒表示时报告 ERROR_ARITHMETIC_OVERFLOW。零超时仍调用原子释放／等待／重获操作。

库内部同步 FFI 以普通 C-layout `{status: Int, code: Int}` 同次返回结果：status 为 0（成功）、1（tryLock 忙或 wait 超时）或 -1（失败），code 保存实际错误。POSIX 的 pthread 返回码直接保存，Windows 在失败后立即保存 GetLastError，不借 errno 转运。该 private native 结果只服务库的实际同步调用，不进入 runtime ABI。

依据：[POSIX mutex 销毁前提](https://pubs.opengroup.org/onlinepubs/9799919799/functions/pthread_mutex_destroy.html)、[条件等待](https://pubs.opengroup.org/onlinepubs/9799919799/functions/pthread_cond_clockwait.html)、[Darwin 相对条件等待](https://github.com/apple-oss-distributions/libpthread/blob/main/src/pthread_cond.c)。这些 API 复用既有 callback、Context、原子与 GC 合同，不新增线程 runtime 协议或强制停止机制。

## 11. 匿名管道与子进程

`scoop.platform.process.suspendCurrentProcess()` 在 Unix 向本进程发送 SIGTSTP，
按操作系统 job-control 规则暂停，收到 SIGCONT 后返回。调用者先恢复终端模式；
被忽略/屏蔽的信号及 orphan process group 沿 OS 行为，不改写进程信号处理策略。

### 11.1 管道与命令快照

`scoop.platform.io.Pipe.open()` 返回 source: PipeSource 与 sink: PipeSink，分别实现 ByteSource／ByteSink 及 Closeable，各自拥有一个 close-on-exec 描述符。端点提供 isClosed、close；sink.flush 只检查状态。关闭最后一个写端后读取返回 EOF；读端全部关闭后写入按第 6.4 节报告 BrokenPipe。端点没有文件定位语义；借用视图只实现 IO 协议，关闭责任由 owner 保持。

`scoop.platform.process.Command(executable: Path, arguments: Array<OsString> = [], environment: Map<OsString, OsString> = 当前环境快照, workingDirectory: Path = 当前目录, stdin: Stdio = Inherit, stdout: Stdio = Inherit, stderr: Stdio = Inherit, mergeError: Boolean = false)` 保存不可变配置与输入集合的副本。Stdio 区分 Inherit、Null、Pipe 和 Redirect(file: File)；File 只借用至 spawn 完成。mergeError 将 child fd 2 复制为配置后的 fd 1，与非 Inherit 的 stderr 配置冲突。

spawn(searchPath: Boolean = false): ChildProcess 启动进程；默认按明确 executable 启动，传入 searchPath = true 才启用 PATH 搜索。搜索模式下含 `/` 的 executable 仍按路径启动，否则按命令环境中的 PATH 从左至右尝试，空项为 child cwd，缺失 PATH 使用 `/bin:/usr/bin`。NotFound／NotDirectory 继续，PermissionDenied 保留到全部候选失败后报告；其他错误立即报告。argv[0] 保留原始 executable，其余参数逐项传递，不拼接 shell 命令。环境名非空且不含 NUL／`=`，其他系统字符串不含 NUL；空参数与非 UTF-8 字节原样传递。

使用 posix_spawn 和真实 cwd file action，不在 managed 进程中 fork 后执行 Scoop。输入字符串只在本次同步 native 调用期间借用；pipe、临时 fd、副本、argv/env 指针表及 file actions 在失败时回滚。重定向先复制到非标准 fd，再设置 child 的 0/1/2，避免别名覆盖。Darwin 使用 CLOEXEC_DEFAULT 与显式标准流动作，Linux 使用 O_CLOEXEC 管道；库不提供继承任意 fd 的接口。child 使用空信号屏蔽集与默认 SIGPIPE，父进程 mask／handler 保持原样，不把临时屏蔽传给 child。

### 11.2 观察、端点转移与释放

ChildProcess 实现 Closeable，拥有子进程观察／reap 责任和本次创建的父端 pipes。stdin: ByteSink?、stdout/stderr: ByteSource? 返回借用视图；takeStdin/Stdout/Stderr 返回相应 owned 端点并清除 child 的关闭责任，缺失或已取出返回 None。对象操作与配置／关闭由调用方串行化，不提供竞争回收同一 pid 的保证。

等待与发送信号的活动 native 调用保活 ChildProcess 至返回清理完成，不能只保留整数 pid；活动观察期间重入 close 会拒绝。GC hook 不得提前回收该观察权，尤其不能在 kill 的 native 调用中先 reap 再让系统复用 pid。

wait(): ExitStatus 等待并回收；tryWait(): ExitStatus? 非阻塞查询。ExitStatus 区分 Exited(code: Int) 和 Signaled(signal: Int)，非零退出不是系统调用失败。成功后缓存结果，重复等待不再次调用 waitpid；terminate/kill 显式请求 SIGTERM/SIGKILL，已回收时无操作，不能对旧 pid 发信号。外部代码不得回收该 child；若 OS 报 ECHILD，则清除观察权并保存该失败，防止后续操作误用复用的 pid。

close 逐一关闭仍拥有的父端 pipes，再尝试 wait；关闭失败也不能跳过后续关闭／回收，多个失败按 ResourceUseException 组合。close 不隐式终止进程，可能阻塞；成功后重复关闭无操作。hook 最多尝试一次 waitpid(WNOHANG)，不等待、不终止、不启动后台 reap，pipe owners 各自负责 raw close。因此 GC 不能替代显式 wait/use。

### 11.3 communicate

communicate(input: List<Byte>? = None): ProcessOutput 收集 stdout/stderr 的独立字节数组及 ExitStatus。提供输入要求配置 stdin Pipe，已经 take 的配置 pipe 拒绝自动通信；输入列表的快照与参数检查发生在消费任何端点之前，getter 失败照常传播。未配置输出 pipe 时对应数组为空，合并 stderr 时只收集 stdout。

通信接管仍拥有的 pipes，用 poll 与 nonblocking fd 同时推进 stdin 写入、stdout/stderr 排空，写完立即关闭 stdin，读至 EOF 分别关闭输出，最后 wait。该驱动只服务当前通信，不引入事件循环或异步框架；等待通过 NativeSafe，EINTR 重试，EAGAIN 回到 poll，不忙循环。所有数据在 Scoop 中积累，C shim 只封装系统调用，不预计算业务结果。

通信驱动失败时关闭其端点，显式 kill 并 reap 本次负责的 child，再传播原异常及必要清理失败；没有遗留工作线程或跨调用 buffer 借用。正常 communicate 不终止 child。普通用户自管 pipes／wait 仍需安排三方向进度；继承管道的后代进程可能延迟 EOF，API 不承诺超时或管理整个进程树。

依据：[POSIX spawn 与 file actions](https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_spawn.html)、[Linux posix_spawn](https://man7.org/linux/man-pages/man3/posix_spawn.3.html) 以及当前 Darwin SDK 的 spawn 扩展。上述能力均为普通 platform 库，不增加 runtime 进程管理协议。

## 12. 阻塞 DNS、TCP 与 UDP

### 12.1 地址与解析

`scoop.platform.net.AddressFamily { Ipv4, Ipv6 }` 区分 IPv4／IPv6。不可变 IpAddress 保存地址位与 IPv6 scopeId: UInt，实现 Equals、Hash、ToString；fromByteArray 接受 List<Byte> 的 4 或 16 字节快照副本，IPv4 不接受非零 scope。toByteArray 返回独立数组。parse(String) 只解析 IP 字面量，IPv6 可带 `%数字` scope，不解析接口名或域名；toString 输出标准数字文本与非零 scope。提供 IPV4_ANY、IPV4_LOOPBACK、IPV6_ANY、IPV6_LOOPBACK。

`IpNetwork(address: IpAddress, prefixLength: Int)` 表示 IPv4／IPv6 CIDR 网络，实现 Equals、Hash、ToString；address 保存按前缀掩码清除主机位后的网络地址，prefixLength 分别限于 0..32 和 0..128。网络不绑定接口，构造拒绝非零 scopeId，`contains(address)` 按同一地址族的地址位匹配，不比较被查询地址的 scopeId，也不把 IPv4-mapped IPv6 当作 IPv4。`parse(String)` 接受一个 IP 字面量、一个 `/` 和无符号十进制前缀长度，复用 IpAddress.parse，不进行 DNS；非法地址、缺少前缀及越界均报 IllegalArgumentException。toString 输出规范化网络地址和前缀长度。CIDR 的解析、规范化与包含判断属于 platform，代理等上层策略直接组合此类型。

SocketAddress(address: IpAddress, port: Int) 检查 0..65535，实现 Equals／Hash／ToString。parse 接受 `IPv4:port` 或 `[IPv6%scope]:port`；端口为十进制数字，不接受缺失、负值或溢出。bind 端口 0 由 OS 分配，返回后的 localAddress() 查询给出实际值。

resolve(host: OsString, port: Int, family: AddressFamily? = None) 与 String 重载使用阻塞 getaddrinfo，返回实际地址数组，不承诺排序或去重。名称与 IP parse 分开，不做 IDNA 或隐式构造器联网；名称字节不含 NUL。错误保存 AddressResolution 的 EAI code；仅 EAI_SYSTEM 另保存同次 errno。native 结果在复制后 freeaddrinfo，包括复制失败路径；不使用外部 DNS 服务作为测试前提。

### 12.2 TCP 与端点所有权

TcpListener.bind(address, backlog: Int = 128, reuseAddress: Boolean = false) 返回 owned listener；backlog 必须为正，实际队列长度由 OS 决定。提供 localAddress()、accept(): TcpStream、isClosed 与 close。TcpStream.connect(address) 返回 owned stream，提供 localAddress()、peerAddress()、ByteSource／ByteSink、shutdownInput、shutdownOutput、isClosed、close，以及 noDelay()／keepAlive() 查询和 setNoDelay(value)／setKeepAlive(value) 设置。

所有 socket 为 close-on-exec。创建、配置、bind/listen/connect 或 accept 后配置失败都会关闭未移交的 fd；显式关闭复用第 6.1 节的单次消费语义，hook 仅 raw close。活动 accept/read/write/地址查询／配置保活 owner，操作与 close／配置改变的竞争由调用方同步。stream 支持 OS 允许的并发读写，但不保证复合操作原子。

TCP 是字节流：允许短读写，正请求读取零表示输入 EOF，写入无进度抛 WriteZeroException；游标只提交实际字节。shutdownOutput 只关闭输出方向，之后仍可读取；shutdownInput 与整个关闭区分，按 OS 报告错误。flush 只检查状态，不承诺对端收到或持久化。可恢复的 read/write/accept EINTR 重试；连接创建若失败或被中断，关闭本次 fd 并报告实际错误，不重复使用未确定状态的 socket。

### 12.3 UDP 与 SIGPIPE

UdpSocket.bind(address, reuseAddress: Boolean = false) 返回 owned socket，提供 localAddress()、isClosed、close。sendTo(buffer: NativeByteBuffer, destination: SocketAddress, count: Long = buffer.readableBytes): Long 把当前可读前缀作为一个 datagram，零长度合法；一次 syscall 成功才消费相应字节，不拆包、不在失败后自动重发。OS 拒绝过大报文时保留 errno 与未消费游标；异常的短发送先提交实际进度，再报告没有伪造 errno 的 PlatformException。

receiveFrom(buffer: NativeByteBuffer, count: Long = buffer.writableBytes): Datagram 返回 source: SocketAddress、count: Long、truncated: Boolean。count 参数必须为正且不超过可写尾部。使用 recvmsg 的实际复制长度与 MSG_TRUNC；只提交复制字节，不把截断后的长度称为原包总长。空 datagram 返回 count=0、truncated=false，仍消费该报文，不代表 EOF；不足容量时消费整包并标记截断，下一次接收是下一包。

socket 输出在 Linux 使用 MSG_NOSIGNAL，Darwin 在新建／accept 后设置 SO_NOSIGPIPE；错误路径不安装或修改全局 signal handler。pipe/fd 的既有策略不变。DNS、连接、accept、传输和可能阻塞的操作使用 NativeSafe，复用已有精确根、借用与 FFI ABI。地址的 C 对应结构只是普通 @CLayout 值，不新增 compiler/runtime 协议、连接池、异步调度或网络安全框架。

依据：[getaddrinfo](https://man7.org/linux/man-pages/man3/getaddrinfo.3.html)、[recvmsg／MSG_TRUNC 与空包](https://man7.org/linux/man-pages/man2/recvmsg.2.html) 及目标 socket headers。验收使用 loopback、动态端口与显式线程同步。

### 12.4 普通 native helper 复用

`native/socket_native.h` 是本库网络 helper 的 C11 声明边界，供 asyncio 等显式依赖者复用。Scoop 对应的公开 `@CLayout NativeSocketAddress(family: Int, port: Int, scopeId: UInt, a: UInt, b: UInt, c: UInt, d: UInt)` 由 platform 定义，复用同一 C 符号的 Scoop 声明使用该类型身份，不自行重声明同形 nominal。ScoopSocketAddress 的字段及 encode／decode、socket_bind 均保持现有 C ABI；family 为 4／6，port 为主机序 0..65535，地址 a～d 为按网络字节顺序组装的主机 UInt32 值，scope 为 IPv6 scope id。socket_bind 返回 owned close-on-exec fd，reuse 精确配置 SO_REUSEADDR，TCP 在返回前完成 bind/listen；失败返回 -1 并保留 errno，内部未交付 fd 已关闭。

这是普通 GC-free C helper 边界，不提供 managed File／TcpStream 的私有句柄访问。使用者自行拥有新获得的 fd，可交给其事件后端；只能移交一次。native 头文件作为实际 include 输入参与普通构建，预编译 `.slib` 消费不再次编译这些 helper。错误、对象身份与缓存沿既有合同处理，不增加 runtime API 或特殊 native dependency 机制。

`OsString.withNativeString<R>((Ptr<Byte>) -> R)` 与 `Path.withNativePath<R>((Ptr<Byte>) -> R)` 是 fs package 的公开 Unsafe 同步扩展：检查 NUL、保持原始字节并追加终止 NUL，指针只在 callback 内有效，不得保留或交给尚未完成的异步操作；异步使用者须在 callback 内复制到自己的 native 存储。`scoop.platform.native.fileOpenFlags(options: OpenOptions): Int` 供同步 File 与 asyncio 的 native 适配复用 6.1 的参数检查，返回 file_open helper 的稳定位组合（read=1、write=2、append=4、create=8、createNew=16、truncate=32），不是系统 O_* 值。OpenOptions 只表达文件打开配置，不暴露 native 位布局成员；应用通过 File.open 或 EventLoop.openFile 使用配置。

`copyNativeString(pointer: Ptr<Byte>): OsString` 同样是 fs package 的公开 Unsafe 同步函数。调用方保证指针指向在整个调用期间稳定、可读且以 NUL 终止的存储；函数复制首个 NUL 之前的原始字节，不做 UTF-8 转换、不保留指针、不释放输入，返回独立快照。原生结果的分配和释放继续由拥有该结果的库负责，asyncio 用此入口接收临时名称和目录项。

`native/io.h` 声明 fd_read／fd_write／fd_close；`native/files.h` 声明 file_open／file_sync。read／write 使用 4～6 章的短传输及 EINTR 规则，close 只调用一次 raw close，syncData 在 Darwin 可用 fsync 实现。`socket_native.h` 还声明 resolve／resolve_next／resolve_address／resolve_free；ScoopResolution 保存 owned addrinfo 链、原始 EAI code、可选的同次 errno。以上函数不进入 managed 代码，可直接从工作线程调用；错误必须在返回时立即保存，资源只能释放一次。
