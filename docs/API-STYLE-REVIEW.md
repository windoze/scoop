# Scoop 库 API 风格 review

日期：2026-10-17。源码基线：`2a5a8734d`。本文是设计评审与建议，不是已经生效的规范。

建议以 **Kotlin 的日常调用词汇为主，保留 Scoop 的 Option、值类型、显式能力约束和 codec 值模型**。统一工作的重点是让名称可靠地表达返回值、求值方式、修改行为和资源责任。Rust、Swift 的设计可以按这些具体问题借鉴。

本次检查了 sysroot 中 10 个生产库的公开声明，并追踪相关实现、规范和部分消费者用法。这些库共有 311 个 Scoop 源文件；该数字是范围清单，不表示逐行审计了全部内部实现。评审覆盖 core、std、json、cli、test、platform、asyncio、crypto、tls、tls.asyncio。没有进行算法正确性、安全性或性能的全面审计，也没有运行编译测试；文中的调用兼容性判断来自公开类型签名和语言规则。

读取依据包括语言、runtime、实现规范和路线图，以及 std、platform、asyncio、crypto、TLS 专项规范。大部分差异已被当前规范明确规定；这里指出的是 API 设计值得调整之处，不将它们误报成实现违反现有规范。

| 库 | 当前主要风格 | 最值得处理的事项 |
| --- | --- | --- |
| scoop.core | Kotlin 语法与基础名称，Rust 式 Option、运算符接口和原子内存序，Swift 式编码协议名称 | 字节类型闭环、StringBuilder、数值转换名称、codec 角色、Set 重名、异常层级 |
| scoop.std | Kotlin collections / sequences / text / time，部分 Java 集合与日期 API | withIndex 的求值方式、subMap 的快照语义、Duration 分量、枚举拼写 |
| scoop.json | 显式 codec 值，静态 Json 门面 | 区分文本输出和 JsonValue 输出 |
| scoop.cli | 注解与 schema，显式 parser，Kotlin 式数据声明 | 保留清晰的 parse / parseCommandLine 区分；配置名称与其他库一致 |
| scoop.test | kotlin.test 式断言与生命周期 | 保留 expected / actual 次序；说明 Equals 约束带来的适用范围 |
| scoop.platform | Rust File / OsString / Command，Java NIO Files，双游标 buffer | searchPath 的启动副作用、命名参数、所有权、公开查询形式 |
| scoop.asyncio | Kotlin 协程任务树，加显式事件循环和流接口 | 同步／异步 API 的词汇与参数对应关系、关闭协议 |
| scoop.crypto | OpenSSL 能力的较薄封装，部分 JOSE 算法名称 | 失败返回约定、普通签名和 JWS 的格式区别、生成密钥的参数形状 |
| scoop.tls | 配置对象、显式传输包装和同步流 | 包装时接管传输、握手副作用、关闭与协议完成 |
| scoop.tls.asyncio | 与同步 TLS 共用引擎的 suspend 接口 | 与同步版本的名称、状态属性和生命周期保持可对应 |

下面的 P1 表示建议优先处理的调用／互操作风险，P2 表示统一风格时应处理的问题，P3 表示可以后续收敛的表面约定；不是编译器缺陷的严重度结论。

**R01 · P1：字节 API 的生产者和消费者不能自然组合。**

[String.toByteArray](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/strings.scoop:38) 返回 `MutableArray<Byte>`，而同一类型的安全 `fromUtf8` / `fromUtf8Lossy` 接收 `Array<UInt8>`。Byte 是 Int8；这里同时跨越可变性与有符号／无符号两个类型边界，`String.fromUtf8(text.toByteArray())` 不能按现有签名直接调用。std、文件、密码库的字节结果通常又是 `Array<Byte>`。

这已经影响库实现：[OsString.decodeUtf8](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/system/os_string.scoop:6) 为进入 core 解码接口，逐元素构造了另一个 `Array<UInt8>`。同时，[ByteArrayBuilder](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/bytes/byte_builder.scoop:42) 已明确用 `toByteArray` / `toMutableByteArray` 区分返回的可变性，String 却没有遵循这一命名。

Rust 的高层字节通常统一为 u8 切片／Vec，Swift 通常为 UInt8 集合／Data，Kotlin 使用 ByteArray。Scoop 可以继续选择 Byte，关键是同一层的表示能够衔接。

建议高层只读字节输入统一为 `List<Byte>`，快照结果为 `Array<Byte>`，需要可写输出时明确使用 `MutableArray<Byte>`；String 的安全 UTF-8 解码也遵循这个约定。`toByteArray` / `toCharArray` 返回只读数组，另设 `toMutableByteArray` / `toMutableCharArray` 表达可写结果。FFI 原始指针按其实际类型保留。影响语言规范 11.4、std 5、platform 2／5，以及字节结果的消费者。

**R02 · P1：Command.searchPath 实际启动了进程。**

[Command.spawn / searchPath](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/process/command.scoop:35) 分别执行 `start(false)` 和 `start(true)`，二者都返回 ChildProcess。`searchPath()` 很容易被理解为查找可执行文件或配置搜索策略；实际调用已经执行了程序。

Rust 的 Command 把启动行为放在 spawn / status / output，Swift 的 Process 使用 run，Java ProcessBuilder 使用 start。它们都让执行动作出现在名称中。

建议收敛为 `spawn(searchPath: Boolean = false)`，保留当前默认行为；也可在必须拆分入口时使用明确包含 spawn 的名称。不要为仅改变搜索策略的启动操作保留裸 `searchPath`。影响 platform 11.1 和进程消费者；无需调整进程执行机制。

**R03 · P2：同一个接口方法的公开参数名随静态类型改变。**

[ByteSource / ByteSink](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/io/streams.scoop:7) 的参数是 `buffer`，但 [BufferedSource.readInto](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/io/buffered.scoop:22) 改成了 `destination`，[BufferedSink.writeFrom](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/io/buffered.scoop:67) 改成了 `source`。语言规范 8.5 明确：命名调用使用静态接收者所见声明的参数名。

因此带 `buffer = ..., count = ...` 的接口调用，直接写在 BufferedSource / BufferedSink 上时不再匹配同一成员。另有同步 `copyTo(sink = ...)` 与异步 `copyTo(destination = ...)` 的差异。Swift 把参数标签视为调用设计的重要组成，Kotlin 的命名参数同样要求作者维护这些名称。

建议所有 override 复用接口参数名；内部字段可以改成 `internalBuffer` 等实现名称。跨同步／异步的同类方法也复用相同参数名与次序。统一 IO 使用 `buffer, count`、复制使用 `destination`、数组区间使用 `offset, count`。这是库声明的修正，不需要改变语言的 override 规则。

**R04 · P2：缺失值与可恢复失败的名称没有形成统一规则。**

评审时同时存在 [fromUtf8OrNone](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/strings.scoop:62)、std 的 `parseOrNull` / `getOrNull` / `mapNotNull`，以及返回 Option 的裸 [decrypt](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.crypto/src/cipher.scoop:23) 和 [verifyCompact](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.crypto/src/jws/compact.scoop:50)。同属验证操作，普通 verify 返回 Boolean，TrustStore.verify 返回专有的 `CertificateVerification { Verified, Failed(...) }`。

返回 Boolean、Option 和带错误数据的结果各有合理用途；问题是读者无法从统一约定判断具体用途。Rust 常通过 Option / Result 表达，Swift 使用 Optional / throws / Result，Kotlin 常用抛异常版本与 OrNull 版本配对。

建议统一使用与 Scoop 缺失值一致的 `OrNone` 拼写，保留 `fromUtf8OrNone`，将已有的 `parseOrNull`／`getOrNull` 改为 `parseOrNone`／`getOrNone`，`mapNotNull`／`filterNotNull` 改为 `mapNotNone`／`filterNotNone`。Scoop 没有 null，不能只为接近 Kotlin 而让名称与 Option 语义分离。解析／解码的普通入口抛格式错误，OrNone 入口只转换约定的格式或不匹配结果。`decryptOrNone`、`verifyCompactOrNone` 能直接提示当前的返回方式；若验证后还取得 payload，应在文档首句写清该动作。

`verify` / `verifyHmac` 这类纯判定保持 Boolean；需要保留验证失败原因时，建议复用已有 `Result<Unit, CertificateValidationFailure>`。Map.get、Iterator.next、find、tryWait 等有成熟缺失语义的查询可以继续直接返回 Option，不要求所有可选结果一律追加后缀。`try` 保留给 tryLock、tryWait、tryResume 这类非阻塞尝试或竞争状态转换。影响 std 1.2／7、语言规范 11.4 和 crypto 对应章节。

**R05 · P1：资源包装的名字没有一致地说明接管责任。**

[BufferedSource / BufferedSink](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/io/buffered.scoop:15) 的构造器借用端点，`takeOwnership` 工厂才接管端点；[tlsClient / tlsServer](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.tls/src/wrap.scoop:7) 则在包装过程中执行握手并接管 transport，接管后的失败负责关闭它。异步 TLS 有相应行为。

此外，同步资源以 Disposable.dispose 为协议，普遍另有 close，状态通常叫 isDisposed；异步资源使用 AsyncCloseable.close / useAsync，AsyncTlsStream 的状态叫 isClosed。名称没有提供一组容易记住的对应关系。Scoop 没有 Rust 的 move 类型检查，调用方尤其需要在 API 表面看清关闭责任。

建议约定：作用域借用使用 `withX`；取走原对象持有的资源使用 `takeX`；接管调用者传入资源的工厂明确使用 `takeOwnership`，并说明责任转移发生在成功返回还是开始工作之前。TLS 可在 TlsStream / AsyncTlsStream 下使用包含 takeOwnership 与 Client / Server 角色的工厂名称，文档首句写明它完成握手后返回。

对关闭词汇，建议 IO 资源采用 Closeable.close 与 AsyncCloseable.close 的对应关系，状态统一 isClosed；Disposable.dispose 留给取消登记、同步原语等一般资源。可用现有 use 逻辑和普通接口默认方法桥接，不需要另建资源管理框架。useAsync 可以保留，用于明确需要等待清理的作用域。

还应把完成与释放的区别写进这组 API 的首要文档：[BufferedSink.dispose](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/io/buffered.scoop:93) 会尝试 flush，而 [TLS close](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.tls/src/stream.scoop:79) 不执行 TLS 双向关闭握手，`shutdownGracefully` 才执行。它们无需被强制改成相同协议行为，但用户必须能明确选择。影响语言规范 11.18、platform 资源章节、asyncio 3、TLS 3／6。

**R06 · P2：StringBuilder 和字节 builder 使用了两套词汇与链式规则。**

[StringBuilder](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/string_builder.scoop:1) 使用 `add(...): StringBuilder`、`build(): String`；[ByteArrayBuilder](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/bytes/byte_builder.scoop:20) 使用 `append(...): Unit`、`toByteArray()`；StringWriter 又使用 write / build。

Kotlin StringBuilder 的熟悉入口是 append / toString；Swift String 也使用 append。集合新增元素用 add、文本与字节尾部追加用 append，更容易从名称看懂。当前 build 并不结束 builder 的使用，重复调用会继续取得快照，因此也无需借用“完成构建”的语感。

建议 StringBuilder 使用 `append(...): StringBuilder` 与 `toString(): String`，显式实现 ToString；ByteArrayBuilder.append 也返回自身，输出继续用 toByteArray / toMutableByteArray。TextWriter.write 保持 Unit，StringWriter 的快照入口使用 toString。构建专用对象允许链式追加，通用 IO 写操作按实际进度返回数量；不要求所有 mutator 都返回自身。

StringBuilder 是 f-string 脱糖目标，不能只做文本替换；需要先同步语言规范 6／11.6／11.16，以及编译器对对应真实声明的选择和定向 fixture。

**R07 · P2：Encodable / Decodable 的名字来自 Swift，角色却是显式 codec 值。**

[Encodable<T>](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/encoding.scoop:1) 接收另一个 value；[Decodable<T>](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/decoding.scoop:1) 返回 T。真正实现这些接口的通常是 companion 或独立 codec，而不是被编码的数据实例。`Inspectable<T>` 有同样的命名问题。

Swift Encodable 表示数据类型自身能够编码；Rust Serialize 也实现于数据类型。Kotlin 的 SerializationStrategy / DeserializationStrategy / KSerializer 则更接近 Scoop 当前的角色。照名字理解的用户容易把 Encodable<T> 误写在 DTO 本身上。

建议保留已经成立的显式 codec 设计，将接口明确命名为 `ValueEncoder<T>` / `ValueDecoder<T>` / `ValueInspector<T>`；格式侧的 Encoder / Decoder 保留。对应函数适配器名称一并调整。这样改变的是概念名称，不要求引入隐式 codec 查找或新的序列化框架。

[Json.encode / encodeValue](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.json/src/json.scoop:3) 还可以收敛为 `encodeToString` / `encodeToValue`，反向为 `decodeFromString` / `decodeFromValue`，让文本与已解析值两个边界直接可见。当前四个入口和显式 codec 参数本身是合理的。

该项迁移范围较大：语言规范 9.6／11.13／11.17、派生方法选择、core/std/json/cli 及消费者需共同调整。宜在基础命名规则确定后单独实施。

**R08 · P2：withIndex 和 subMap 的熟悉名字掩盖了求值／快照差异。**

[Iterable.withIndex](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/collections/windows.scoop:5) 通过 mapIndexed 立即生成 List。Kotlin 同名 API 返回惰性 Iterable；Rust enumerate 和 Swift enumerated 也提供逐项适配。因此 `source.withIndex().first()` 在 Scoop 中已经消费了全部 source，对很大、单遍或无限输入影响明显。Sequence 版本是惰性的，但调用者必须主动切换协议。

建议让 Iterable.withIndex 返回逐项包装的 Iterable，显式需要快照时调用 toList。这只需要普通迭代适配，不需要修改 for 或增加新的序列框架。

[SortedMap.subMap / SortedSet.subSet](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/collections/sorted_ranges.scoop:3) 返回独立快照，名字则来自 Java NavigableMap / NavigableSet 的背靠原容器视图；Kotlin/JVM 使用那些接口时也会带入这种预期。当前算法还扫描整个容器，不只是区间。

建议保留快照这一合理取舍，将入口命名为明确的 `copyOfRange`，或同等明确表达复制的名称；文档记录遍历复杂度。Map.keys / values / entries 继续是随源变化的 view，toList / toMap 继续是快照，asSequence 继续是适配。影响 std 2.3／3.1／3.2。

**R09 · P2：Duration.toComponents 暴露的不是 Kotlin 用户熟悉的分量。**

[Duration.seconds / toComponents](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/time/duration.scoop:5) 返回 floor-normalized 的内部表示。`Duration.nanoseconds(-1L).toComponents()` 为 `(-1L, 999999999)`，而同对象的 inWholeSeconds 为 0。

Kotlin 的秒／纳秒 toComponents 回调给出趋零的整秒与同号的余数，该例是 `(0L, -1)`；Swift Duration 的公开 seconds / attoseconds 分量也不是当前这组非负余数的命名暗示。Rust std::time::Duration 是无符号时长，不能直接作为负时长的先例。Scoop 使用有限、有符号时长是合理决定，问题在于公开分量的含义。

建议内部继续使用目前的规范化表示；公开 `toComponents` 返回与 inWholeSeconds 一致的分解。如必须公开原表示，应明确命名为 `floorSeconds` 和 `nanosecondsOfSecond`，或使用 `toNormalizedComponents`；不要同时让普通 seconds 表示向下取整分量、inWholeSeconds 表示趋零总秒数而只在深层规范中解释。影响 std 8.2 和读取这些字段的平台时间代码。

单位构造的 `5.seconds()` 与 Kotlin 的 `5.seconds` 不同，属于较小的表面差异。Scoop 已支持扩展属性，可以按统一属性规则选择；优先级低于分量含义。

**R10 · P2：整数转换名与对外类型名、文本解析名没有对齐。**

core 对 Int / Long 使用 [toInt32 / toInt64](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/integer_int8.scoop:103)，而文本使用 [toInt / toLong](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/text/signed_text.scoop:15)，并另外提供定宽别名。字节 reader / builder 又是 `readInt8/readInt16/readInt/readLong`、对应 write 方法的混合。

Kotlin 以 toInt / toLong 与类型拼写对应；Swift 以 Int32 / Int64 等类型构造表达转换；Rust 的 i32 / i64 也直接对应位宽。Scoop 的透明 alias 没有问题，常用 API 的主拼写需要一致。

建议普通数值转换主入口与常用类型名一致，提供 toByte / toShort / toInt / toLong 及 unsigned 对应名称。二进制格式 API 则成套使用 readInt8 / readInt16 / readInt32 / readInt64 及 write 对应名称，以表达固定字段宽度。已有透明类型 alias 可继续服务 FFI；文档明确主拼写，避免每个模块自行选择一套。计数使用 Long 是现有全库约定，可保留，不必为接近 Kotlin 而改为 Int。

**R11 · P2：core 的 Set 运算符接口与集合 Set 发生了真实重名。**

[core Set<Index, Value>](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/operators_access.scoop:5) 表示下标赋值，[collections.Set<T>](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/collections/sets.scoop:3) 表示集合。库自身已经需要 [import Set as IndexSet](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/collections/maps.scoop:3)。按语言规则，不能靠不同类型参数个数解决导入名称选择。

Rust 使用 Index / IndexMut，Swift 使用 subscript，Kotlin 没有这个与集合 Set 同名的公共运算符接口。建议将运算符接口成组改为 `IndexGet` / `IndexSet`、`IndexGet2` / `IndexSet2`，保留调用方法 get / set 和集合 Set 名称。影响语言规范 9.3／11.10 与实现这些接口的普通声明；无需改变下标语法。

**R12 · P2：ResourceUseException 的类型层级不符合 Exception 这个名称的预期。**

[ResourceUseException](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/resources.scoop:5) 直接继承 Throwable，而其他常规 `...Exception` 继承 Exception。因此 `catch (e: Exception)` 无法捕获 use 中正文与清理同时失败产生的这个对象，即使两次原始失败都是 Exception。

Kotlin use 会保留主异常并记录清理异常；Scoop 使用显式组合对象是可以保留的设计。建议至少让 ResourceUseException 继承 Exception，并保持 primaryFailure / cleanupFailure 两个字段。不要仅为对齐 Kotlin 而引入新的通用异常聚合体系。测试库 AssertionFailure 直接继承 Throwable 是另一个明确命名的角色，不需要机械改动。

影响语言规范 11.7／11.18、use / useAsync 和多资源清理的调用者。

**R13 · P2：密码 API 中的算法标识与参数形状需要更精确。**

[SignatureAlgorithm.ES256](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.crypto/src/signature.scoop:5) 的普通签名结果是 ASN.1 DER；[JwsAlgorithm.ES256](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.crypto/src/jws/compact.scoop:7) 的签名段是 64 字节 R || S，JWS 实现明确执行转换。实现符合当前规范，但 ES256 是 JOSE 的成熟术语，普通签名 API 借用它时容易让人误以为字节格式可以直接互换。

建议普通签名算法使用能表达密码方案及必要编码的名称，例如 `EcdsaP256Sha256Der`；RS256 / PS256 的普通方案也可使用对应的 RSA 方案名称。JwsAlgorithm 保留协议中的 RS256 / PS256 / ES256 / EdDSA。若保留现有普通算法名，至少把签名表示放进公共签名 API 的首句说明与类型设计，而不是只在 JWS 内部体现。

[PrivateKey.generate(algorithm, rsaBits)](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.crypto/src/keys.scoop:11) 还接受与 P256 / Ed25519 无关的 rsaBits，KeyAlgorithm.Other 虽可作为查询结果却不能生成。建议使用 `generateRsa(bits)` / `generateP256()` / `generateEd25519()`，将无关或不可用组合移出调用表面；这组具体工厂已经有实际对应实现，不需要增加算法插件层。

**R14 · P3：其余命名可按明确的小规则收敛。**

| 当前表面 | 评价与建议 |
| --- | --- |
| MemoryOrder.Relaxed、DebugLayout.Compact；ByteOrder.BIG_ENDIAN、Month.JANUARY | 普通 enum variant 建议统一 UpperCamelCase。Kotlin 官方允许 UpperCamelCase 或全大写，Rust 用 UpperCamelCase，Swift 用 lowerCamelCase；这里应形成 Scoop 自己的一致选择，而不是把某种拼写判成错误。协议算法代码可保留公认拼写。 |
| Json / Cli / Tls / Uuid / Ip / Os / Io | 整体接近 Rust 的缩写单词化。可把它规定为 Scoop 规则；不必再按 Swift 的 URL / TLS 或 Kotlin 的两字母缩写规则逐个反复改名。 |
| Base64.Default / UrlSafe，Duration.ZERO / Uuid.NIL | 前者是具名配置实例，后者是常量值，可以按不同类别保留。无需把二者一律归为大小写冲突。 |
| [File.size / position](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/fs/file.scoop:25) 与 metadata() / seek() | 属性隐含执行 fstat / lseek 并可能失败；建议外部状态查询使用函数，或让用户从 metadata() 明确取得结果。普通存储属性保持 val。 |
| [TcpStream.noDelay / setNoDelay](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/net/tcp.scoop:11) | 查询和设置的表面不对称。由于这里访问外部状态，建议用 noDelay() / setNoDelay() 成对；若全库决定采用属性，则使用完整的 var 属性契约。 |
| [Month.of](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.std/src/datetime/month.scoop:20)、DayOfWeek.of | of 在 Java 生态常见，但这里缺少输入含义。建议 fromNumber / fromIsoDayNumber，与 fromEpochDays 等转换工厂相配。 |
| [OsString.toBytes](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/system/os_string.scoop:4)、IpAddress.toBytes，与 Uuid.toByteArray | 返回相同数组形状时统一 toByteArray；对应工厂统一 fromByteArray。SecretKey.exportBytes 的显式导出语义可以保留。 |
| [getCurrentSourceLocation](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.core/src/source_location.scoop:10) 与 currentThreadId / currentEventLoop | 建议 currentSourceLocation，与已有 currentX 函数成组。保留其默认参数记录调用点的语言语义。 |
| [Files.readText(path)](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/fs/files.scoop:10)、top-level resolve、File.open | Files 是 Java 静态工具类风格。以 Kotlin 为主线时，文件系统操作适合以 Path 扩展作为主入口；文件句柄仍由 File.open 创建。这里可以后续统一，无需为了表面一致重排 Cone 依赖。 |
| [OpenOptions.nativeFlags](/Volumes/Data/home/chenxu/repos/scoop/sysroot/lib/scoop.platform/src/fs/file.scoop:12) | 实际被 asyncio 使用，不能简单改 private。可归入明确的 native 支持 package，避免普通应用把 helper 位布局当作通用文件 API。沿用已有跨 Cone native 支持边界即可。 |

有几组与其他语言不同的地方，建议明确记录并保留。

| 主题 | Scoop | Rust | Swift | Kotlin | 评审意见 |
| --- | --- | --- | --- | --- | --- |
| 迭代终止 | next(): Option<T> | next(): Option<T> | next() -> Element? | hasNext() + next() | Scoop 模型完整，保留；无需补一套 hasNext。 |
| 可选值层次 | T? 是 Option<T>，Some(None) 与 None 不同 | 嵌套 Option 保留层次 | 嵌套 Optional 可保留层次 | nullable 类型不重复套层 | 保留层次。名称使用 mapNotNone／filterNotNone，文档说明只去除一层。 |
| Result | Result<T,E>，Success / Failure，E 无异常约束 | Result<T,E>，Ok / Err | Result<Success,Failure>，Failure: Error | Result<T>，失败载荷为 Throwable | 当前是自洽的组合；保留 mapError / flatMap / getOrThrow 的明确角色。 |
| 数组与集合可变性 | Array 只读，MutableArray 可写，List 是只读接口 | 借用与可变性由类型／所有权表达 | 集合为值类型，修改受变量可变性控制 | Array 本身可写，List / MutableList 分接口 | Scoop 的区分合理，重点修复同名转换返回值的不一致。 |
| 普通相等、打印能力 | 显式 Equals / ToString / Hash；Any 无通用方法 | 显式 trait | 显式协议及部分合成 | Any 提供 equals / hashCode / toString | 保留能力边界，不为模仿 Kotlin 添加 Any 回退。 |
| 时间值的 class / struct | 需要不变量的时间值使用不可变 class | 常用 struct | 常用 struct | 常见值类或不可变类型 | 当前 Scoop struct 可做公开字段的副本更新；不能只为外观接近 Swift 就改掉封装。 |
| suspend 与调度 | launch / async 不承诺线程调度，EventLoop 显式提供 IO 驱动 | async 与 executor 分离 | 由任务和 executor 执行 | kotlinx.coroutines 构建器与 dispatcher 协作 | 保留显式调度边界，在构建器文档注明可能在返回前执行正文。 |
| CancellableContinuation.tryResume | Boolean 结果，一步选择并提交恢复 | 无完全对应的 std API | 无完全对应的标准接口 | kotlinx.coroutines 的相关底层接口分 tryResume / completeResume | 记录移植差异即可，不引入 Kotlin 的两阶段状态协议。 |

字符串尤其需要独立说明。对内容为 `e`、U+0301 组合重音、`😀` 的同一文本：

| 表达式 | 结果 | 计量单位 |
| --- | --- | --- |
| Scoop String.length | 3 | Unicode 标量 |
| Rust str.len() | 7 | UTF-8 字节 |
| Swift String.count | 2 | 扩展字素簇 |
| Kotlin String.length | 4 | UTF-16 code unit |

Scoop 的 length 和整数下标定位是线性操作，byteLength 是常量时间；这应出现在公开 API 文档中。当前 Char／UTF-8 模型明确，不建议在本次风格统一中改变字符语义。按字符顺序处理优先使用 iterator，不能把 Kotlin 的随机索引复杂度预期直接带入 Scoop。

Map.getOrPut 对已有值 `Some(None)` 的处理也应记录：Scoop 只把外层 None 视为缺少键，Kotlin 的 nullable value 则可能把 null 当作需要缺省值的情况。保留 Scoop 的区分，不为名称兼容而丢失状态。

test 的 assertEquals(expected, actual) 次序和 assertFailsWith<E> 用法与 kotlin.test 接近，值得保留。需要说明的是 `assertEquals` 要求显式 Equals<T>：List / Array 没有自动结构相等，普通 struct 的派生 `==` 也不自动带来该接口。可以按实际测试需求补充有元素约束的内容比较断言或显式比较回调，避免让用户为了断言而重写类型；不需要扩展语言的自动 conformance。

CLI 的 `parse(arguments)` 与 `parseCommandLine(argv)` 分别处理不含／包含程序名的参数数组，命名清楚；`CliResult.Parsed / HelpRequested` 表达正常控制流程，适合保留专用 enum。它与只有成功／失败两支的证书验证结果承担不同职责，不必机械合并为 Result。

建议形成以下一页式公共约定，作为后续修改 spec 的依据。

| 维度 | 建议约定 |
| --- | --- |
| 主风格 | lowerCamelCase 函数／属性，UpperCamelCase 类型与普通 enum variant；集合词汇以 Kotlin 为主 |
| 参数名 | 属于公开 API；override、同步／异步对应入口保持一致；主要操作回调放最后 |
| 状态与动作 | 廉价、稳定的对象状态用属性；外部查询、IO、分配资源和生命周期变化用函数并写清动作 |
| 失败 | 解析的普通入口抛出具体异常；OrNone 只转换约定的正常失败；谓词返回 Boolean；需要失败载荷时使用已有 Result |
| 复制与适配 | toX 为独立结果；toMutableX 明确可写；asX 表示适配／view；copyOfRange 明确区间复制 |
| 修改 | sort / sorted、shuffle / shuffled 成对；集合 add，文本／字节 builder append；IO read / write 表达实际传输 |
| 所有权 | withX 借用，takeX 取走，takeOwnership 接管；文档写清失败前后的责任；finish／协议 shutdown 与本地 close 区分 |
| 字节 | 高层输入 List<Byte>，快照输出 Array<Byte>，可写目标 MutableArray<Byte>；保持输入输出可直接衔接 |
| 数值 | 普通转换与类型名对应；二进制字段成套使用显式位宽名称；计数／索引沿用 Long |
| 文档 | 每个公共入口写明返回形状、求值方式、复制／借用、正常失败及必要复杂度；不重复堆叠内部 ABI 细节 |

推荐的实施顺序是先完成 R01／R02／R03／R04／R12，使跨库调用与失败处理可预测；随后成组处理 R05／R06／R07／R08／R09／R10／R11／R13；最后处理 R14 中的表面统一。资源所有权 R05 虽然优先级高，但涉及多个库，适合单独成组迁移。

每组先修订对应 spec，再修改实现和实际消费者。参数名、默认值、返回类型及 codec 协议名称属于源码接口变化，需要下游重编译；枚举名字可能进入派生编码，源名字和 wire 名是否一起迁移应分别决定。优先使用项目内实际调用方完成一次迁移，只有存在明确的外部兼容需要时再保留短期别名。回归围绕字节往返、经接口／具体类的命名调用、完整与短传输、资源关闭、惰性消费和 codec 组合；不需要新增风格证明或多阶段重复验证机制。

本次对照的上游依据：

- [Kotlin coding conventions](https://kotlinlang.org/docs/coding-conventions.html)：命名、枚举拼写、属性与函数、扩展函数；网页抓取超时后核对了 [JetBrains 官方文档源码](https://github.com/JetBrains/kotlin-web-site/blob/master/docs/topics/coding-conventions.md)。
- [Rust API Guidelines: Naming](https://rust-lang.github.io/api-guidelines/naming.html)：大小写、as / to / into、getter 与所有权名称。
- [Swift API Design Guidelines](https://www.swift.org/documentation/api-design-guidelines/)：副作用、修改／非修改成对命名、术语含义、参数标签和属性复杂度。
- [Kotlin collections 源码](https://github.com/JetBrains/kotlin/blob/master/libraries/stdlib/common/src/generated/_Collections.kt)：withIndex 的惰性 Iterable。
- [Kotlin Duration 源码](https://github.com/JetBrains/kotlin/blob/master/libraries/stdlib/src/kotlin/time/Duration.kt)：toComponents、inWholeSeconds 和余数符号。
- [Kotlin Result 源码](https://github.com/JetBrains/kotlin/blob/master/libraries/stdlib/src/kotlin/util/Result.kt) 与 [CancellableContinuation 源码](https://github.com/Kotlin/kotlinx.coroutines/blob/master/kotlinx-coroutines-core/common/src/CancellableContinuation.kt)：结果形状和恢复接口。
- [Kotlin StringBuilder](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin.text/-string-builder/) 与 [Rust Command](https://doc.rust-lang.org/std/process/struct.Command.html)：追加／字符串化和进程启动的实际入口。

上述 Kotlin 源码链接指向本次读取的 upstream master 页面，用于核对既有习惯，不作为 Scoop 的版本兼容承诺。
