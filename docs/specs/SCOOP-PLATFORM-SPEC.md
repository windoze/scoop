# Scoop 平台库规范

本文件规定普通 `scoop.platform` 库的公共合同，实施进度与三目标验收见 [M45](../milestone45/DESIGN.md)。语言、core、GC、FFI 与构建规则分别以 [语言规范](SCOOP-SPEC.md)、[运行时规范](SCOOP-RUNTIME-SPEC.md)和[实现规范](SCOOP-IMPL-SPEC.md)为准；平台无关的数据类型沿用[标准基础库规范](SCOOP-STDLIB-SPEC.md)。

## 1. 库与 native 边界

`scoop:scoop.platform:0.1.0` 是普通 library Cone，位于 `sysroot/lib/scoop.platform/`，生产依赖为 core 与 std，测试依赖为 scoop.test。应用显式声明依赖与 import；不默认导入，不改变 single-file 的 core-only 规则。公共逻辑用 Scoop 实现，C11 shim 只承担实际 OS、native allocation 与 ABI 操作，通过既有 native sources 发布，不新增 compiler-known role 或平台后端注册框架。

本期目标为 `aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。路径、参数与环境使用 POSIX 原始字节。阻塞操作走 NativeSafe FFI，只有符合语言规范 14 章的短操作才标记 GCLeaf。native 返回的错误在调用边界保存，不能晚些时候重新读取 errno。

库行为通过同 Cone 的 `tests/` 和 `scoop test` 验证；编译错误、真实产物消费及 GC／ABI／进程边界保留必要 fixture。普通库代码不要求重复整个编译器验收。原始验证日志不进入仓库，仅记录结果概要。

## 2. 系统字符串与路径

### 2.1 OsString

`scoop.platform.system.OsString` 是不可变 final class，实现 `Equals<OsString>` 和 `Hash`。内部保存原始字节；相等和 hash 只取决于字节内容，既不 Unicode 归一化，也不读取外部状态。

- `fromString(value: String): OsString` 按 UTF-8 编码。
- `fromBytes(value: Array<Byte>): OsString` 与 MutableArray 重载都复制输入。
- `toBytes(): Array<Byte>` 返回独立副本。
- `decodeUtf8(): String` 严格解码，非法输入抛 CharacterCodingException，保留相对于整个字节序列的错误位置。
- `decodeUtf8Lossy(): String` 显式替换坏序列；`display(): String` 使用该有损形式供诊断展示。展示结果不能作为原始系统调用输入的替代品。

OsString 可以保存任意字节，包括 NUL。实际进入 C 字符串位置的 API 在调用前拒绝内嵌 NUL；环境变量名另须非空且不含 `=`。拒绝抛 IllegalArgumentException，不能静默截断或改写输入。

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

## 3. 系统错误

`scoop.platform.system.PlatformErrorKind` 包含 NotFound、AlreadyExists、PermissionDenied、NotDirectory、IsDirectory、InvalidInput、NoMemory、NoSpace、BrokenPipe、ConnectionRefused、ConnectionReset、AddressInUse、AddressUnavailable、TimedOut、Interrupted、WouldBlock、Unsupported、Other。未知错误归 Other，必须保留原始错误数据；Unsupported 仅用于 OS 实际报告不支持。

`NativeErrorDomain` 区分 PosixErrno 与 AddressResolution。普通值类型 `NativeError(domain, code, systemCode: Int? = None)` 保存原始 Int 错误码；AddressResolution 的 EAI_SYSTEM 还保存同次失败的 errno。只读属性 `kind` 依据目标系统的实际常量分类，不假设两个 target 的 errno／EAI 数值相同。pthread 直接返回的 errno 风格错误属于 PosixErrno，不能改读线程 errno。

`PlatformException(operation: String, kind: PlatformErrorKind, nativeError: NativeError? = None, context: OsString? = None)` 继承 Exception，以上四项为只读字段。调用方按结构字段处理错误；message 只供诊断，不作为可解析协议。context 用于保存实际路径或端点文本等调用上下文，不覆盖原始 code。

非法参数、越界、算术溢出、对象状态错误分别沿用 core 的 IllegalArgumentException、IndexOutOfBoundsException、ArithmeticException、IllegalStateException，不包装成伪造的 OS 错误。IO 的 EOF／无进度错误由对应 IO 协议另行规定。

## 4. NativeByteBuffer

`scoop.platform.memory.NativeByteBuffer` 是实现 Disposable 的普通 final class，独占连续 native 字节存储。class 引用复制共享同一 owner、游标和关闭状态，不复制存储。`allocate(capacity: Long = 0L)` 创建精确请求容量、两个游标为零的对象；零容量不分配。负容量抛 IllegalArgumentException，native 分配失败抛 kind 为 NoMemory 的 PlatformException，保留调用边界捕获的 errno。字节区不保存 managed reference，也不承诺清零。

### 4.1 游标、初始化与释放

open 对象保持 `0 <= readPosition <= writePosition <= capacity`，只有 `[0, writePosition)` 已初始化。只读属性为 capacity、readPosition、writePosition、readableBytes、writableBytes 和 isDisposed；不存在扩大 writer 的 safe setter。关闭后仅 isDisposed、dispose 和 close 可调用，其余操作及查询抛 IllegalStateException。

| 操作 | 合同 |
| --- | --- |
| clear() | 游标归零，保留 allocation，不清零；旧数据不能再由 safe API 读取 |
| compact() | 以 memmove 语义将未读区移到起点，readPosition=0，writePosition=原 readableBytes |
| skip(count: Long) | 消费可读前缀；负值非法，超出可读区越界 |
| ensureWritable(count: Long) | 显式保证尾部空间，不隐式 compact；成功增长保留内容和游标，失败保留地址、容量、内容与游标 |
| dispose()／close() | 幂等；先把 owner 置 inert，再 free；不 flush，不调用用户代码 |

仅 ensureWritable 会扩容，普通写入在空间不足时越界。增长倍率不是合同，容量至少为 writePosition+count；所有大小运算在地址计算前检查 Long 溢出并抛 ArithmeticException。首版没有缩容、共享 view、slice 或 duplicate。GC-free release hook 只在正常回收时兜底 free；不保证触发时间，不代替显式释放。hook 只读字段，显式关闭后 hook 不重复释放。

### 4.2 字节、数组与数值

- readByte()/writeByte(Byte) 顺序访问单字节；getByte(index: Long)/setByte(index: Long, value: Byte) 只访问已初始化的绝对下标，不改游标。
- readBytes(count: Long): Array<Byte> 返回独立副本，分配与复制成功后才消费输入。
- readInto(array: MutableArray<Byte>, offset: Long = 0L, count: Long = array.size-offset) 复制到受检数组区间；writeBytes 提供 Array<Byte> 与 MutableArray<Byte> 两个同形重载。先验证两侧区间，完成后才提交游标；count 为负抛 IllegalArgumentException，数组区间非法抛 IndexOutOfBoundsException。
- read/writeInt8、UInt8、Int16、UInt16、Int、UInt、Long、ULong、Float、Double 提供相应定宽读写；write 的首参数为 value，除单字节外都必须显式传入 `scoop.bytes.ByteOrder`。浮点按位型编码，包括 NaN 与负零。

顺序读写先检查完整宽度，不留下半个数值或半次游标变化。超出可读／可写范围抛 IndexOutOfBoundsException，负数量抛 IllegalArgumentException。任意 byte offset 都合法，不依赖数值自然对齐；实现逐字节组合或复制到对齐局部，禁止对未对齐的 typed pointer 直接 load/store。

### 4.3 同步 native 借用

`@Unsafe fun <R> withReadableBytes(block: (Ptr<Byte>?, Long) -> R): R` 提供当前可读前缀，不消费游标；调用方承诺只读。`@Unsafe fun withWritableBytes(block: (Ptr<Byte>?, Long) -> Long): Long` 提供可写尾部。两个入口都恰好调用 block 一次，零长度传 None/0，非空传 Some(pointer)/length。

writable block 必须初始化返回值 n 指定的完整前缀；仅正常返回且 `0 <= n <= writableBytes` 时提交 writePosition。无效 n 抛 IllegalArgumentException，抛异常或无效返回都不提交游标，已修改的尾部仍不能 safe 读取。库只检查范围，不证明 native 初始化事实。

block 是普通非 suspend callback，可分配 managed 对象、抛异常及进行同步 C 调用。owner 跨 callback 与 finally 保持 live root，GC 可移动 wrapper；native 地址保持稳定，不 pin wrapper。finally 清除借用状态。借用中拒绝 dispose、ensureWritable、clear、compact、skip、顺序读写、绝对写和嵌套借用，修改前抛 IllegalStateException；只读属性及 getByte 仍可用。

pointer 不得逃出 callback、保留给异步操作或跨挂起使用；writable callback 不读取未初始化尾部或改写借用区外字节。借用标志只处理重入，不是跨线程锁；共享对象由调用方同步。不增加逃逸分析、静态 borrow checker 或 managed ownership 类型。

### 4.4 GC pressure

成功提交容量 c 后通过 core `gcAdjustPressure(+c)` 提示，增长只报告 new-old；实际释放后报告 -c。失败、clear、compact、读写和借用不报告变化；同一份所有权不得重复扣减。显式关闭先清空字段，hook 只读字段并在 collector reclaim 路径释放，二者共用同一个 GC-free 入口。

提示遵守语言规范 14.1 与运行时规范 3.1；首版 GC 完全忽略提示是合法实现。buffer 不触发 GC，也不在 malloc/realloc 失败后收集重试，不维护 RSS、分配峰值、通用预算或资源账本。
