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
