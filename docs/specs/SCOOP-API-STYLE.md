# Scoop 库 API 风格

本文规定 core、std 和各官方库的公共 API 约定。语言语义与各领域合同仍见 [语言规范](SCOOP-SPEC.md)、[标准基础库规范](SCOOP-STDLIB-SPEC.md)、[平台库规范](SCOOP-PLATFORM-SPEC.md)及对应库规范。本约定用于设计和评审，不新增编译器命名检查或产物验证阶段。

日常调用词汇以 Kotlin 为主，类型与能力边界遵守 Scoop 自身规则。名称应让调用者判断操作、返回形状和必要的所有权责任。

| 项目 | 约定与例子 |
| --- | --- |
| 函数、属性、参数 | lowerCamelCase：currentSourceLocation、bufferSize、isClosed |
| 类型、普通 enum variant | UpperCamelCase，缩写按单词处理：Json、Cli、Tls、Uuid、Ip、Os、Io；ByteOrder.BigEndian、Month.January、AnnotationTarget.Function |
| 常量与配置实例 | 常量保留 Duration.ZERO、Uuid.NIL、Int.MAX_VALUE；具名配置实例使用 Base64.Default／UrlSafe |
| 外部标准名称 | JwsAlgorithm 的 HS256／RS256／PS256／ES256／EdDSA 和 Architecture.X86_64 保留公认拼写；其他普通 variant 不因此改成全大写 |
| 工厂 | 普通值构造用构造器；输入含义需要区分时用 fromNumber、fromIsoDayNumber、fromByteArray；资源动作用 open、spawn、generateRsa 等具体动词 |
| 参数 | 参数名属于源码 API；同义 override 与同步／异步入口保持名称和次序，主要操作回调放最后。流传输用 buffer／count，复制用 destination／bufferSize，数组区间用 offset／count |
| 属性与函数 | 对象已有的廉价状态用属性；外部查询、IO、资源分配、生命周期动作使用函数。File.size() 查询 OS，FileMetadata.size 是已取得的值；异步 socket 的地址属性保存创建时快照 |

命名参数由静态声明决定。领域术语可以不同：Map.get(key) 使用键的含义，IndexGet.get(index) 表达通用下标；通过对应接口调用时使用该接口的参数名。索引能力使用 IndexGet／IndexSet／IndexGet2／IndexSet2，集合接口使用 Set<T>，不靠类型参数数量消歧。

| 返回值与操作 | 约定 |
| --- | --- |
| 失败 | 普通解析或转换入口抛具体异常；OrNone 只把约定的正常失败变成 None；谓词返回 Boolean；需要失败载荷时使用 Result<T, E>。无关异常照常传播 |
| 数组与字节 | 高层只读输入 List<Byte>，独立快照输出 Array<Byte>，可写目标 MutableArray<Byte>；FFI 指针继续按实际 native 元素类型表达 |
| 转换与适配 | toX 创建与源后续结构修改无关的结果；toMutableX 明确可写；asX 表示适配或 view。容器快照是浅复制，引用元素仍引用原对象 |
| 集合求值 | 普通集合转换默认急切；Sequence 惰性。Iterable.withIndex() 是明确的惰性例外，需快照时调用 toList()；有序容器 copyOfRange 返回独立快照 |
| 修改与构建 | sort／sorted、shuffle／shuffled 区分就地与新结果；集合用 add，文本和字节 builder 用可链式 append，IO 用 read／write |
| 整数 | 普通转换对应 Byte／Short／Int／Long 和无符号类型名；二进制字段使用 read/writeInt8／Int16／Int32／Int64 及 UInt8／UInt16／UInt32／UInt64；索引、计数沿用 Long |
| Codec | ValueEncoder／ValueDecoder／ValueInspector 表达处理 T 的独立对象；Json 的 encodeToString／decodeFromString、encodeToValue／decodeFromValue 明示边界表示 |
| 资源 | 可关闭资源实现 Closeable；dispose 默认桥接 close。暴露关闭状态时使用 isClosed。一般清理能力仍为 Disposable；use 同时保存正文与清理异常 |
| 所有权 | 资源回调形式 withX { ... } 表示作用域内借用；takeX 取走，takeOwnership 接管。具体入口说明移交时点与失败责任；finish／shutdownGracefully 与本地 close 各表达实际动作 |

缺失值的命名使用 Scoop 的 `None`：可选结果的配对入口使用 `OrNone`，例如 `toIntOrNone`、`parseOrNone`、`getOrNone`；过滤可选值使用 `mapNotNone`、`filterNotNone`，只去掉外层 None 并提取 Some 的载荷，保留载荷中的 None。Map.get、Iterator.next、find 等已有明确缺失语义的查询无需统一追加后缀。JSON 格式的 null 与 FFI 的空指针按各自领域术语表达，不属于 Option API 的命名。

withPadding 是不可变配置派生，withIndex 是惰性迭代变换，二者均不属于资源回调形式。计时单位保留 `5.seconds()` 等受检函数；它们可能溢出，不能把它们当作免费读取的字段。Duration.floorSeconds 是内部规范化表示的整秒下界，inWholeSeconds 和 toComponents() 的整秒向零截断，分量余数与时长同号。

文件系统操作以 Path 扩展为入口，例如 `path.readText()`、`source.copyTo(destination)`、`path.metadata()`、`link.createSymbolicLink(target)`。File.open(path, options) 创建句柄；createTempFile／createTempDirectory 是顶层工厂。跨库 native 支持入口放在相应 native package，普通配置类型不承诺 native 位布局。

从 Rust、Swift 或 Kotlin 移植时，以下差异是 Scoop 的明确选择。

| 主题 | Scoop 的使用规则与差异 |
| --- | --- |
| 字符串 | String.length 计 Unicode 标量且为 O(n)，byteLength 计 UTF-8 字节且为 O(1)，整数下标定位为 O(n)。文本 `e` + U+0301 + `😀` 的长度分别是 Scoop 3、Rust str.len() 7 字节、Swift String.count 2 字素簇、Kotlin String.length 4 个 UTF-16 code unit。顺序处理字符优先迭代 |
| 迭代终止 | Iterator.next(): Option<T>，无需 Kotlin 风格的 hasNext()；返回 None 表示结束 |
| 嵌套可选值 | T? 是 Option<T>，Some(None) 与 None 不同；mapNotNone 只去掉一层。Map.getOrPut 只把外层 None 视为缺少键，已有 Some(None) 不触发缺省值生成 |
| Result | Result<T, E> 使用 Success／Failure，E 无异常上界；mapError、flatMap 与 getOrThrow 各按明确的载荷／异常约束使用 |
| 数组与能力 | Array 只读、MutableArray 可写；Any 没有通用 equals／hashCode／toString 回退。普通 struct 可派生 ==，但不自动获得 Equals<T> conformance；assertEquals 要求该显式能力，数组内容比较由实际元素规则决定 |
| 任务执行 | launch／async 可以在返回前同步执行正文，不承诺切换线程；具体 IO 驱动由 EventLoop 提供。CancellableContinuation.tryResume 返回 Boolean，一步选择并提交恢复，不使用 Kotlin 的 tryResume／completeResume 两阶段协议 |
| CLI | parse(arguments) 不含程序名，parseCommandLine(argv) 包含程序名；CliResult.Parsed／HelpRequested 表达正常控制流程 |

每个公共入口的领域文档应交代返回形状、求值方式、复制或借用、正常失败以及有实际影响的复杂度。普通 enum 源名字、反射／debug 名字与默认派生编码名字一起变化；显式协议格式独立规定。API 迁移必须同时修改规范、实现与实际消费者，验收使用真实编译诊断和运行结果。
