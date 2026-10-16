# M51 设计：密码、证书与 TLS 集成

状态：实施中；完成范围与验证记录见 [进度](PROGRESS.md)。日期：2026-10-17。

本设计接入 OpenSSL，交付三个普通 Cone：`scoop.crypto` 提供不承担文件／网络 I/O 的密码、密钥、证书和签名功能；`scoop.tls` 复用这些功能，提供公共 TLS 能力及 platform 同步传输；`scoop.tls.asyncio` 复用前两者并适配 M49 异步传输。调研时的代码基线为 `c64823c28`；M49 的三目标验收已经完成，后续 M50 重构仍在进行，不能将 M49 的历史结果视为当前 HEAD 的完整验收。

依据：[语言规范](../specs/SCOOP-SPEC.md) 11.9、12.2、13～14 章，[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 3.8、8～9 章，[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.7～2.8，[标准基础库规范](../specs/SCOOP-STDLIB-SPEC.md)第 5 章，[平台库规范](../specs/SCOOP-PLATFORM-SPEC.md) 4～5、12 章，以及[异步 I/O 规范](../specs/SCOOP-ASYNCIO-SPEC.md)。公共 API 与行为须先分别写入密码库和 TLS 库规范，再修改实现；本文件不宣称这些 API 已经存在。

## 1. 目标与范围

M51 同时覆盖内存中的密码操作和网络 TLS 传输，支持三个现有目标：`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。

- `scoop.crypto` 提供摘要、HMAC、认证加密、密钥导入／导出及生成、签名／验签、X.509 证书解析与验证，并提供 JWT 常用的 JWS 签名适配。
- `scoop.tls` 提供 TLS 1.2／1.3 客户端与服务端的公共上下文、协议引擎、身份验证、SNI、ALPN 和同步传输，复用 crypto 的密钥、证书与信任存储类型。
- 同步 TLS 接入 platform 的 `ByteSource`、`ByteSink` 与 `Disposable`；`scoop.tls.asyncio` 单独接入 M49 的 `AsyncByteStream`、`AsyncCloseable`、Job、取消及 `NativeByteBorrow`。
- 三个 Cone 分别发布生产 `.slib`，使用普通 native 依赖和公开 API。只使用密码、证书或 JWS 的应用无需依赖 TLS、platform、asyncio 或 libuv；只使用同步 TLS 的应用无需依赖 asyncio 或 libuv。

HTTP、DTLS、QUIC、连接池、Happy Eyeballs、代理、自动重连、TLS 客户端证书认证、动态 SNI 证书选择、用户回调式证书验证、会话恢复 API、0-RTT、FIPS 和自定义 provider 不属于首版交付。JWE、在线 JWKS 获取、完整 JWT claims／授权策略、在线证书状态查询、证书签发与 CA 服务同样不在本期范围。上游实现内部的正常 TLS 消息处理继续由 OpenSSL 完成，不自行重写协议或密码算法。

范围以本里程碑的实际库调用为依据。不增加语言关键字、compiler-known TLS 类型、专用 coroutine lowering、TLS runtime ABI、通用后端插件、资源预算或构建产物来源认证机制。

## 2. 已有基础与前置条件

| 已有能力 | M51 的使用方式 |
| --- | --- |
| core 的数组／字符串同步 FFI 借用 | 内存密码操作使用公开 withDataPointer／withUtf8Bytes，在单次同步调用内借用；无需依赖 platform 的 native buffer |
| std 的内存字节、Base64URL 与时间值，普通 scoop.json | 复用字节容器、无 padding 的 URL-safe 编码、显式验证时刻和 JWS protected header 解析；不重写编码器或 JSON parser |
| core 的 Job、CoroutineScope、Continuation 与取消登记 | 直接表达操作归属、一次结果选择和收尾等待；不新增调度器或取消 ABI |
| platform 的 NativeByteBuffer／NativeByteBorrow | 保存稳定 native 字节区、跨挂起保活和提交实际明文进度；不持有可移动 managed 对象的裸地址 |
| platform 的阻塞 DNS、TCP 和同步流 | 同步连接、监听和传输继续使用现有实现 |
| M49 的 EventLoop、DNS、TCP、文件与 UDP | 异步连接直接组合 resolve／connectTcp／listenTcp；文件可用于应用加载配置，UDP 不参与本期 TLS |
| AsyncByteStream 的独立读写方向、短传输、缓冲与关闭合同 | TLS 包装公开流接口；无需 fd、libuv handle 或私有 LoopState／LoopOperation |
| 普通 native include、C constructor、COMMON 和平台强弱符号处理 | 使用上游 OpenSSL 的正常编译产物；实际链接失败再定位所属层问题 |

依据见 [core 同步数据借用](../../sysroot/lib/scoop.core/src/data_borrow.scoop)、[Base64 声明](../../sysroot/lib/scoop.std/src/io/encoding/base64.scoop)、[JSON Cone](../../sysroot/lib/scoop.json/Cone.toml)、[M49 验收](../milestone49/ACCEPTANCE.md)、[EventLoop 声明](../../sysroot/lib/scoop.asyncio/src/event_loop.scoop)、[异步流声明](../../sysroot/lib/scoop.asyncio/src/streams.scoop)与[跨挂起借用](../../sysroot/lib/scoop.platform/src/memory/native_byte_borrow.scoop)。M49 已有外部消费者验证公开泛型、资源辅助函数与真实 native I/O；无需将这些基础能力重新列为 TLS 的待实现项目。

三目标 OpenSSL 构建、链接与最小握手已通过实际 Scoop CLI 验证，见 [M51 进度](PROGRESS.md)；完整库功能及互操作仍按各批次验收。基线使用 M50 合并后的 `8dad7d42e`，不继承历史结果作为本期通过记录。

若合法的 FFI、Cone 或普通 native 链接暴露底层缺陷，先在所属层建立最小复现，按需修订对应 spec、修复并回归，再继续受影响的 TLS 工作。不能通过访问私有状态、复制一套 buffer／任务实现或修改上游协议行为绕过缺陷。

## 3. 后端选择

首版选用 **OpenSSL 3.5 LTS**，三个目标统一 vendor 同一份固定版本的上游源码，分别构建目标产物。实施时选择该维护分支的稳定补丁版本，将精确版本、源码来源、许可证和构建选项记录到实际依赖中。版本更新按普通依赖维护处理，不由程序运行时寻找或替换系统中的任意 SSL 库。

| 候选 | 决定与理由 |
| --- | --- |
| OpenSSL 3.5 LTS | 首版后端。具有长期维护分支、完整 TLS 1.2／1.3、明确的非阻塞重试合同和 BIO pair，可通过普通 C ABI 适配同步及异步传输 |
| LibreSSL | 保留后续评估。其 libssl／libtls 可以实现相同方向的适配，但不是 OpenSSL 3 的 ABI 替代品，API 与维护节奏也不同；本期不同时交付第二套后端 |

公开的高层 API 表达密码、证书和 TLS 行为，不暴露 OpenSSL 结构布局、宏、provider 或原始 native 指针。`scoop.crypto` 对应 libcrypto，`scoop.tls` 使用 libssl 并复用前者；不引入后端注册、动态装载或可插拔协议引擎。

OpenSSL 的 `SSL_MODE_ASYNC` 服务密码引擎异步操作，不是网络异步 I/O；M49 集成不使用它。网络等待由 Scoop 流和事件循环承担。

## 4. 库分层与跨 Cone 边界

| Cone／目录 | 职责 |
| --- | --- |
| `scoop.crypto`／`sysroot/lib/scoop.crypto/` | 显式依赖 core、std 和用于 JWS 的 json，链接 libcrypto；提供内存密码操作、密钥、证书、信任存储、签名及 JWS 适配；不依赖 platform、asyncio 或 libssl |
| `scoop.tls`／`sysroot/lib/scoop.tls/` | 显式依赖 crypto、platform 及实际使用的基础库，声明 libssl 需求；提供 TLS 配置、上下文、连接信息、共享协议引擎和同步流；不依赖 asyncio／libuv |
| `scoop.tls.asyncio`／`sysroot/lib/scoop.tls.asyncio/` | 显式依赖 tls、asyncio 及直接使用的基础类型库；提供异步 TLS 流、双向推进、取消和异步关闭；复用 tls 的协议引擎，不重复编译 OpenSSL |
| 各 Cone 的 tests／consumer-tests | 通过 `scoop.test` 验证同 Cone 行为与外部生产产物消费；独立对端和同步辅助依赖仅进入测试构建 |

crypto 的内部 package 按 digest／mac、cipher、keys、signature、x509、jws 划分职责；它们仍是一个 Cone。TLS 的同步与异步适配则通过两个 Cone 隔离实际依赖，依赖方向为 async TLS → TLS → crypto，不依赖可选依赖机制或仅按 package 名称推测链接隔离。

platform 不反向依赖 crypto、TLS 或 asyncio；core 与 runtime 不因此增加 OpenSSL 依赖。TLS 的两种适配器都在当前传输上收发密文字节，不用 SSL_set_fd 接管 socket，也不另建 OpenSSL socket BIO 的平行实现。

跨 Cone 复用必须走实际公开边界。crypto 的密钥、证书和信任存储，以及必要的 Unsafe native helper／唯一 CLayout 类型集中定义在 crypto；TLS 取得受生命周期约束的 native 引用。共享 TLS 协议步骤及对应 native 类型由 `scoop.tls.native` 向异步适配提供最小公开入口。使用者不能访问上游 internal 成员、自行重声明同形 nominal 类型或反复导出／解析 PEM 作为内部传递方式；不导出整套 OpenSSL API。

crypto 的“不涉及 I/O”指不读写用户文件、不接受 Path／stream、不访问网络、不自动读取系统 CA 或下载证书／JWKS。密钥与证书由调用方提供内存数据；密码学随机数及密钥生成仍使用 OpenSSL 的安全随机源和必要的系统熵，不声称所有操作都是纯函数，也不使用 std 的普通 PRNG 代替安全随机数。独立证书验证的时间由调用方显式提供。

内存 API 不调用按路径读取 PEM／DER 或默认信任库的后端入口；后端初始化也不自动加载用户 OpenSSL 配置文件或外部 provider。TLS 和应用所需的文件／网络加载在其 I/O 层完成，再将结果传入 crypto。

crypto 和 TLS 的 C shim 分别管理其 OpenSSL 对象和 native 存储，异步 Cone 复用 TLS shim；Scoop 负责值类型 API、协议适配、传输和资源组合。C 不保存 managed 引用，不调用 suspend，不恢复 continuation。ALPN 等必要的 OpenSSL callback 只读取 native 自有配置并在 C 内返回，不进入 managed 代码。

## 5. Native 构建、链接与交付

**musl 的 OpenSSL 必须由 Scoop vendor、构建并随目标包交付，不能要求用户安装发行版提供的 musl 专用 libssl／libcrypto。首版三个目标统一采用这一交付方式，避免同时维护系统库和自带库两条生产路径。**

vendor 包含固定版本的上游源码、许可证和必要的构建说明；仓库普通构建脚本调用上游构建流程，产生各目标的静态 `libssl.a`、`libcrypto.a` 和匹配头文件。源码构建可从这份输入准备依赖，发行包直接提供目标产物。C shim 仍由 Cone 的普通 native sources 编译。首版使用上游默认 provider；不增加自定义 provider、动态模块部署或一套密码库配置框架。

发行版常见的 GNU／glibc OpenSSL 开发包不能作为 musl 的依赖来源；静态 archive 也需要匹配目标 libc 与构建配置，不能仅因扩展名是 `.a` 就跨 GNU／musl 使用。

| Scoop 目标 | 构建核对重点 |
| --- | --- |
| aarch64-apple-darwin | 从 vendor 源码按上游 Darwin arm64 配置构建；核对所选 SDK、deployment target、初始化记录及最终真实运行 |
| x86_64-unknown-linux-gnu | 从同一份 vendor 源码按 GNU 工具链构建；核对汇编、CPU 能力初始化和普通系统依赖 |
| x86_64-unknown-linux-musl | 使用 musl 工具链与 sysroot 独立构建并交付静态库和生成头文件；不使用宿主 GNU 库、头文件或 pkg-config 的 OpenSSL 结果；完成最终静态程序运行 |

源码与产物布局如下；`<triple>` 表示发布时选定的具体目标，不是 manifest 变量插值：

| 位置 | 内容 |
| --- | --- |
| `sysroot/lib/scoop.crypto/vendor/openssl/` | 固定版本的上游源码及许可证；crypto 与 TLS 共用这一份依赖 |
| `sysroot/native/<triple>/openssl/include/` | 该目标构建安装的公开头文件，包含匹配的生成配置头 |
| `sysroot/native/<triple>/openssl/lib/` | 该目标的 libssl.a、libcrypto.a；作为目标发行资源交付 |

各目标的上游构建目录和生成配置头相互独立，不能在源码目录内轮流覆盖同一份配置。archive 与生成头文件作为构建／发行产物管理，不要求将它们作为源文件提交。

构建与发布遵守以下边界：

1. 保留上游源码与必要许可证，使用其 Configure／生成头文件／汇编生成流程；不手工重写配置宏或密码算法。正常汇编和初始化对象通过普通 native 链接处理，不能默认以 `no-asm` 规避工具链缺陷。
2. crypto 和 TLS 的 shim include 输入明确指向上述当前目标的 include 目录；crypto 只声明 `crypto` 静态库需求，TLS 声明 `ssl` 并消费 crypto 的传递依赖，异步适配不重复声明另一套后端。需求通过 native.libraries 记录，最终链接使用指向同一目标 lib 目录的现有显式 library roots。只使用 crypto 的程序不要求 libssl；目标、头文件与库来自同一份实际构建。
3. 目标包必须同时提供这些头文件、archive 和普通路径配置；musl 消费者不需自行寻找或编译另一份 OpenSSL。生产 `.slib` 含 shim 对象及逻辑 native 库需求，不假定 `native.libraries` 会自动把整个第三方 archive 嵌入 `.slib`；native 目标资源是完整交付的一部分。
4. 源码构建先通过普通依赖准备命令生成所选目标的 OpenSSL 产物；使用发行包时直接消费随包产物。必要输入未准备时报缺失输入并指明准备步骤，不以宿主库或系统 OpenSSL 搜索结果替代。依赖准备不在每次应用构建中重新下载、配置或编译未变化的源码。
5. `scripts/build_openssl.py --target <triple>` 调用上游构建并安装上述目标资源；生成头文件通过普通 `native.include` 条件路径表选择，最终命令传入该目标 lib 目录作为 `--library-path`。发行包包含同一目标目录；不使用目标变量插值或任意构建脚本入口。musl 工具链需要其 libc 开发头和隔离的 Linux UAPI 头目录，具体包装方式见 vendor README。
6. libc、线程库等系统依赖按所选目标的真实构建声明，与自带 OpenSSL 的选择分开。构造／析构、COMMON、native 线程局部存储与 archive 选择交给现有工具链。缓存继续只判断输入更新，不增加产物复核、来源审计或 OpenSSL 专属链接白名单。

最小验证分别包含仅依赖 libcrypto 的摘要／签名运行，以及 libssl 上下文／连接创建和一轮客户端／服务端握手。仅编译头文件、生成 `.slib` 或成功调用版本函数不足以证明接入可用。普通源码、工具和原产物始终保留；外部消费者测试不通过删除或隐藏输入来验收复用。

## 6. 密码 API、证书与 TLS 复用

下列名称用于确定职责；完整 Scoop 声明、默认参数与错误合同在实现前分别写入 `SCOOP-CRYPTO-SPEC.md` 和 `SCOOP-TLS-SPEC.md`。以本期明确的算法和应用组合为范围，不逐项映射 OpenSSL 的全部 API。

### 6.1 内存密码操作与密钥

| 功能 | 首版范围与 API 合同 |
| --- | --- |
| 摘要 | SHA-256／384／512；支持一次性计算与 update／finish 的增量上下文，输入为内存字节，结果为独立字节值 |
| MAC | HMAC-SHA-256／384／512；显式密钥与数据，提供计算及校验；校验使用后端适合 MAC 的比较操作，不以普通字符串比较实现 |
| 认证加密 | AES-128-GCM／AES-256-GCM；显式 key、nonce、AAD，返回密文与 tag；解密只有 tag 校验成功后才交付明文，认证失败不暴露临时明文 |
| 密钥 | RSA、P-256、Ed25519 公私钥及对称密钥的类型化表示；PEM／DER 导入与导出，支持本期算法的密钥生成；私钥首版使用无交互口令的内存输入 |
| 签名／验签 | RSA PKCS#1 v1.5 + SHA-256、RSA-PSS + SHA-256、ECDSA P-256 + SHA-256、Ed25519；算法及密钥类型显式匹配，调用者提供待签名字节 |
| 安全随机字节 | 使用 libcrypto 的安全随机源；返回内存字节，不引入 platform 依赖；失败保留实际后端错误 |

算法使用 EVP 等上游成熟接口，不在 Scoop 中重写密码原语。nonce 长度、tag 长度、PSS 参数和 ECDSA 签名编码必须在正式 API 中明确，不能依靠隐含后端缺省值互操作。AEAD 的 nonce 由调用方显式提供，并按所选算法遵守同一密钥下不重复的要求；本期不建立全局 nonce 分配或持久化服务。

签名校验不匹配是正常校验结果，和密钥格式非法、算法／密钥不匹配及后端运行失败分开表达。增量上下文具有明确的 update／finish／关闭状态，要求串行使用；一次 finish 后不继续更新。密钥生成等可能耗时的操作仍是同步内存 API，调度由调用方决定，crypto 不创建任务或线程池。

输入复用 List／Array 字节接口，必要时取得独立数组快照；单次 native 调用通过 core 的 withDataPointer／withUtf8Bytes 借用，返回即结束，不依赖 NativeByteBuffer，也不跨挂起保留指针。native 密钥、证书和增量上下文使用普通对象与显式 Disposable 生命周期；关闭后拒绝继续使用，native 引用需要跨对象存活时显式保留。

### 6.2 X.509 证书与信任验证

Certificate 从 PEM／DER 内存数据解析，支持导出、读取 subject／issuer、序列号、有效期、SAN 和公开密钥。证书链作为显式证书集合传入，TrustStore 从调用方指定的信任锚建立，不读取文件、系统 CA 或远程地址。

区分三种操作：

1. **普通数据验签：** 使用公钥验证调用方给定数据和签名。
2. **证书签名检查：** 检查某张证书的签名是否与给定签发者公钥匹配；这个结果不等于证书可信。
3. **证书链验证：** 使用显式信任锚、中间证书、验证时刻和用途执行上游链构建及验证，检查签名、有效期、约束和适用的 key usage／extended key usage；需要验证服务身份时另给 DNS 名称或 IP 地址。

链验证返回明确的成功结果或结构化失败，保留适用的 OpenSSL 验证码与证书位置。调用方提交的最后一张证书不会自动变成可信根。首版不自动进行 AIA、OCSP、CRL 或其他网络获取，缺少中间证书按实际验证失败处理；完整的在线吊销检查不由此 API 承诺。

crypto 的身份参数使用 DNS 字符串或明确为 4／16 字节的 IP 地址值，不依赖 platform 的地址类型；TLS 在适配层将现有 IpAddress 转为该内存表示。证书解析和链验证沿用上游支持的证书算法，不因第 6.1 节的通用签名 API 只暴露部分算法而拒绝其他正常受支持的证书链。

TLS 直接使用 crypto 的 Certificate、PrivateKey、TrustStore 和身份验证配置。构建 TLS 上下文时保留必要 native 引用，SSL 握手使用同一套类型和后端验证能力；不先在 crypto 完整验证一次、再无条件重复重放同一链，也不让应用自己先验一次来替代握手中的身份验证。

### 6.3 JWT／JWS 签名适配

JWT 的签名用法由 `scoop.crypto.jws` package 提供，首版覆盖使用普通编码 payload 的 JWS Compact Serialization。复用 std 的 `Base64.UrlSafe.withPadding(ABSENT)` 和 scoop.json，支持 HS256、RS256、PS256、ES256，以及使用 Ed25519 密钥的 EdDSA。JWS 是 crypto Cone 内的内存适配，不新增 JWT Cone 或 I/O 依赖。

- 签名输入严格使用原始 `base64url(protected header) + "." + base64url(payload)` 字节；验签不先把 JSON 重新序列化，否则空白或字段顺序变化会改变签名输入。
- 验签入口要求调用方给出允许的算法及匹配密钥，protected header 的 alg 必须与之匹配；不根据 token 自称的算法任意切换密钥解释方式，不接受 none。
- HS256 使用对称 MAC 密钥，RSA／EC／Ed25519 使用相应公钥；JWS ES256 的固定宽度 R || S 与常见 ASN.1 DER 签名明确转换，PS256 的摘要、MGF1 与 salt 参数按 JWS 定义，Ed25519 使用该算法规定的签名方式。
- 不支持的 critical header 和扩展编码方式给出明确错误。kid 仅供调用方选择已提供的密钥，库不按 token 中的地址取密钥；公钥 JWK 可作为本期支持算法的内存导入格式，JWKS 获取和刷新留在调用方的 I/O 层。
- 入口和结果明确称为签名／MAC 验证，不将其描述为完整 JWT 有效性判断。exp、nbf、iss、aud、应用授权及是否接受某个 token 由调用方在验签成功后按自身规则判断；库不默认把 payload 当作已可信的业务结果。

JWS 支持围绕现有 JSON／Base64 和实际算法组合实现，不自建 JSON 解析器、通用认证中间件、远程密钥管理或凭证体系。

### 6.4 TLS API 与传输所有权

| API 方向 | 合同 |
| --- | --- |
| `TlsClientContext`／`TlsServerContext` | 按角色创建、配置后不可变、可供多个连接使用的上下文；消费 crypto 的证书、密钥和信任存储，管理 SSL_CTX；同步创建并显式释放 |
| `TlsPeerName` | 区分 DNS 名称与 IP 地址；客户端每次连接都提供待验证的身份，与实际连接地址分开 |
| 同步 client／server 包装入口 | 以普通泛型约束同时要求 ByteSource、ByteSink、Disposable；接管一个传输，完成握手后交付 TlsStream |
| 异步 client／server 包装入口 | 要求 AsyncByteStream 与 AsyncCloseable，在活动 Job／所属 EventLoop 中完成握手后交付 AsyncTlsStream |
| `TlsStream`／`AsyncTlsStream` | 分别实现同步字节流协议加 Disposable、异步字节流协议加 AsyncCloseable；读写进度均以明文字节计 |
| 连接信息 | 只读的协商 TLS 版本与可选 ALPN 结果；不暴露 native 指针或假设未协商的协议 |
| `shutdownGracefully` | 显式协议关闭；与保证本地释放的 close 分开，见第 11 节 |

同步流没有现成的 `ByteStream` 合并接口；异步 `AsyncByteStream` 也不继承 `AsyncCloseable`。API 使用普通泛型约束表达现有能力，不为方便签名修改 platform／asyncio 的基础接口。

首版固定接管底层关闭责任，不增加 borrowed／owned 两套模式。参数和配置检查成功后、开始握手前完成接管；此后的分配、握手、验证或取消失败由包装入口关闭已接管传输并释放未交付 TLS 状态。接管前失败仍由调用方拥有底层资源。包装期间调用方不得绕过 TLS 直接读写或关闭底层通道。

连接和监听由现有 API 组合：同步使用 platform 的 resolve／TcpStream／TcpListener，异步使用 EventLoop.resolve／connectTcp／listenTcp。服务端对 accept 得到的每条连接分别执行 TLS 握手，不让一次握手阻塞 listener 的后续 accept。M51 不实现另一套 DNS、地址重试或 listener。

上下文创建消费已经准备好的证书与密钥对象，属于显式同步准备工作，应在事件循环的请求处理前完成；异步握手不暗中读取证书文件或使用 worker 执行阻塞 socket TLS。文件配置由调用方或 TLS 层的 platform 便捷入口读取，再交给 crypto 的内存 API。上下文释放后禁止创建新会话，已有会话持有必要的 native 引用和配置，直到各自关闭。单个密钥、证书、上下文／连接关闭只释放其拥有的对象，不执行进程级 OPENSSL_cleanup。

## 7. 证书、身份与 ALPN

- **客户端验证默认开启。** 配置必须给出 crypto 的 TrustStore，或经 TLS 层显式文件加载得到的信任存储。验证链、有效期及目标名称均成功后才交付连接；首版不提供跳过验证的高层开关，测试使用自己的 CA。
- **文件与默认路径属于 I/O 层。** 若提供 OpenSSL 默认 CA 文件／目录的加载入口，它位于 TLS 层并明确要求 I/O，不进入 crypto。默认路径不等于 macOS Keychain；首版不承诺自动接入系统原生证书存储。显式 PEM 配置必须在三个目标上可用，读取路径复用 platform，错误保留其原始平台信息。
- **DNS 名称与 IP 地址分开。** DNS 连接设置 SNI，并单独配置名称验证；SNI 不代替证书验证。IP 连接验证证书的 IP SAN，不做反向 DNS，也不将 IP 字面量冒充 DNS SNI。DNS 名称使用非空、无 NUL 的 ASCII／A-label 输入，国际化名称转换不在本期重复实现。
- **身份不随解析结果改变。** 应用连接某个 DNS 名解析出的地址时，仍使用原始目标 DNS 名验证。连接目标 SocketAddress 与 TlsPeerName 是两个参数，不从对端地址猜测身份。
- **服务端身份在创建上下文时检查。** 接受 crypto 的证书链与私钥，验证两者匹配并保留所需 native 引用；PEM／DER 解析和密钥对象由 crypto 统一提供，不在 TLS 中复制实现，也不保留 managed 数组／字符串的借用地址。
- **ALPN 使用明确的协议字节串列表。** 每项长度为 1～255，编码总长符合协议限制；配置复制后不可变。服务端按配置顺序选择双方共同项；双方使用 ALPN 而没有共同项时报告协商失败。未协商时返回 None，不虚构默认协议；应用自行判断是否要求某个协议。
- **协议和密码配置保持有限。** 默认允许 TLS 1.2～1.3，使用上游正常密码算法与验证策略；不默认启用旧协议或 legacy provider，不提供任意 OpenSSL 配置字符串透传。需要新增具体配置时，以实际调用和对应验收为依据。

## 8. Native 边界、共享 TLS 引擎与错误

crypto 的 native helper 负责 EVP 密码操作、密钥及 X.509 对象；TLS 的 native helper 负责 SSL_CTX、SSL 和协议 BIO。二者来自同一份 vendor OpenSSL，并通过第 4 节的普通公开边界复用对象，不复制一份 libcrypto 或把 SSL 依赖反向放入 crypto。

每个 TLS 连接持有独立 SSL 和 BIO pair。一端交给 SSL 读写，另一端供 Scoop 适配器填入／取出密文；只在实际容量内推进，不让内存 BIO 因持续写入无限增长。SSL_CTX、SSL、BIO 与自有配置的创建失败、所有权转移和释放顺序须在 C helper 中覆盖全部出口。

共享 helper 提供有限同步步骤：握手、读取明文、写入明文、协议关闭，以及密文输入输出。步骤结果包含成功进度、需要输入／输出、收到 close_notify 或实际失败；需要重试是内部状态，不作为普通 TLS 异常暴露给应用。

两层的错误提取都在同一次 C 调用内完成。SSL 操作前清空当前线程 OpenSSL error queue，调用后立即保存 errno，在没有插入其他 OpenSSL 调用的情况下执行 SSL_get_error，再提取所需的错误队列和验证结果。EVP／X.509 操作按各自返回约定区分正常校验不匹配与运行失败，并立即提取原始错误；不对它们套用 SSL_get_error。不得先返回 Scoop、挂起或切换线程，再从另一调用中查询错误。native 错误字符串和数据在其有效期内复制，随后由 managed 侧构造结果或异常。

crypto 定义自己的普通错误类型及验证结果，不依赖 PlatformException；TLS 在此基础上表达握手配置、身份验证、协议和截断错误，保留操作、OpenSSL 原始码及适用的验证结果／errno。底层传输的 PlatformException、取消和 IoClosedException 按原合同处理，不伪造 POSIX／Libuv 错误，也不把 OpenSSL 错误塞入不相关的平台错误域。

OpenSSL 会分配内存、取得内部锁并使用线程局部状态；默认采用普通 C ABI extern 的 NativeSafe 调用方式，不因“没有调用 Scoop GC”就标记为 GCLeaf。C shim 不跨边界抛异常，不访问 runtime 私有头文件。密码、证书与 TLS 对象的释放也属于正常 native 调用，不把上游析构链直接放进 GC release hook；显式关闭与正常 native 引用生命周期承担资源释放。

## 9. 字节流推进与并发

### 9.1 同步与异步共用的协议步骤

1. 在短临界区内独占同一个 SSL，执行当前允许的协议步骤，记录明文进度、密文待发送状态和重试要求。
2. 取出需要发送的密文，通过底层 ByteSink 或 AsyncByteSink 推进；只消费底层实际已接受的前缀。
3. 需要输入时，通过底层 ByteSource 或 AsyncByteSource 取得密文，填入 BIO 后重试相应步骤。底层 EOF 先告知 BIO，由 TLS 引擎判断是正常通知还是截断。
4. WANT_READ 之前产生的必要输出也必须被驱动，不能只等待输入而把握手／协议消息留在 BIO 中。异步输出发生背压时，已经需要的输入仍须能够推进；同步 ByteSink 的阻塞等待按其原合同返回，不为同步包装新增并发 I/O 线程。
5. 只有新输入、输出空间或操作状态发生变化时才重试；没有进展则等待真实 I/O，不循环调用 SSL 消耗 CPU。

每次 SSL 调用及其错误读取必须串行；临界区不覆盖底层阻塞 I/O、suspend、continuation 恢复或用户代码。一次 SSL_read 可能需要输出，一次 SSL_write 可能需要输入，应用只发起读或只发起写时也必须完成协议所需的反向传输。

OpenSSL 的 WANT 重试规则约束下一次可执行的 SSL 操作，不能只靠释放互斥锁就任意交错调用。待重试写保留相同内容、长度及所需地址，遵守对应 WANT 状态允许的调用顺序；引擎协调同时保留应用另一方向可推进的机会。特别是一次正在等待输入的应用读，不能阻止应用写产生输出，否则会使正常请求／响应互相等待。

### 9.2 明文进度、缓冲与 flush

readInto 的成功计数表示已经初始化到调用方 buffer 的明文；writeFrom 的成功计数表示 TLS 已接受的明文。它们不是底层 socket 收发的密文字节数，也不表示对端已收到或处理。

单次写选择有限的合法明文前缀，允许短写；同一前缀尚在 WANT 重试时不消费下一段。首次 SSL_write 成功后记录其实际接受量，随后输出失败或取消也不回滚这段进度，不因密文部分发送而重新加密或重发已接受明文。

TLS 内部可以保留已接受数据产生的密文，并承担其存储生命周期。首版使用有限的 BIO pair 与独立 staging buffer，以实际背压挂起，不把应用所有输入复制到无界队列。具体内部容量不是产物 ABI，也不建立通用资源配额。

flush 排出本层已接受的待发送数据，再调用底层 flush；它不等待对端确认，不执行 close_notify。正 read 的零表示收到有效 TLS close_notify 后的输入结束；原始 TCP EOF 不直接作为正常 TLS EOF。零长度请求在状态、取消和范围检查后返回零，不驱动握手或网络 I/O。

### 9.3 异步并发与同步边界

AsyncTlsStream 允许不同 buffer 各一个应用 read 和 write 同时在途，同方向并发报状态错误；flush 占用应用输出方向。连接内使用普通操作记录和等待者协调可执行的 SSL 步骤及底层各一个 read／write，避免两个应用操作重复占用底层同一方向。

等待某一底层方向时，不持有妨碍另一方向或必要协议步骤的锁。完成通知与取消通知在状态更新后、锁外处理；没有活动请求时不遗留无归属后台任务。不为每个 SSL 调用创建 Job，不增加全局驱动服务。

异步流归属建立它的 EventLoop；操作检查现有 Job／Context 合同，传输等待由底层公开异步流完成。跨线程取消只进入普通受同步保护的状态协调，不能直接调用 libuv 或借助其内部投递函数。第三方挂起点可能跨线程恢复，同一 SSL 的串行性由连接自身保证，不依赖“永远在同一线程”的假设。

同步 TlsStream 首版要求调用方串行使用，关闭也不与同步操作竞争。它通过相同引擎执行阻塞传输，不为阻塞接口额外创建线程池或承诺异步流的并发合同。

## 10. TLS 借用、存储与 GC

区分两层实际借用：

- **明文层：** TLS 操作取得调用方 buffer 的 NativeByteBorrow，按需保留到 SSL 重试和必要收尾完成。read 使用 writable 借用，write 使用 readable 借用；记录该操作实际产生／接受的明文前缀，在全部出口恰好 complete 一次。
- **密文层：** 下游 readInto／writeFrom 对 TLS 自有 staging buffer 按已有协议取得借用。TLS 调用下游之前结束自己的同步访问；不能预先 borrow 同一 owner，再把它交给下游触发第二次借用。

BIO 数据先复制到自有 NativeByteBuffer 再交给下游，或从下游 buffer 填入 BIO；首版允许这一次必要复制。不能为了零拷贝新增 NativeByteBuffer 外部内存视图、任意指针 owner 或修改共享存储合同。

未完成的 SSL_write 重试保留同一份稳定数据；返回给调用方前，TLS 必须不再需要该借用地址。正常返回前解除借用；失败、取消和关闭也先停止相关 native 使用并提交已知进度，再解除借用和恢复等待者。`suspendCancellableCoroutine` 的立即取消不能代替这里对存储结束的等待。

操作记录和 continuation 按普通 GC 根保活 buffer、借用、连接及上下文。C 只持有独立 native allocation 的地址；managed wrapper 移动不改变这些地址，不 pin wrapper，不跨挂起保留 Array／String 的同步借用指针。

## 11. 取消、EOF 与关闭

### 11.1 取消

登记前已经取消时不启动操作、不接管新传输、不改变 TLS 状态。成功结果已经选定时，后到的取消不改写该结果。

首版对已登记且尚未完成的握手、读、写、flush 或优雅关闭采用统一的终止策略：取消获胜后终止整条 TLS 连接。该策略允许协议状态、TLS 内部缓冲或底层传输已前进，不承诺回滚，也不继承原始 TCP 单次读写取消后仍可复用的保证。

取消只选择结果；实际结束仍需等待正在执行的 SSL 步骤、底层操作及关闭完成。先记录明文实际进度，停止引擎和传输对存储的使用，解除借用并释放未交付资源，再向调用方报告取消。其他未获胜操作因连接关闭结束，已选定的取消或成功保持原结果，不恢复第二次。

### 11.2 本地 close

close／dispose 表示终止本地资源使用并完成释放，不隐含 flush 或等待对端的 TLS 关闭握手。

1. 将连接置为 closing，拒绝新操作，协调所有在途结果和正在执行的 native 步骤。
2. 关闭接管的底层传输，等待其操作、借用和 native 关闭结束；即使某步失败，也继续剩余必要清理。
3. 禁止后续协议步骤并释放 SSL／BIO；确认借用地址已无 native 使用者后，提交已知明文进度、结束 TLS 持有的借用，再释放自有 buffer 和上下文引用，随后完成关闭等待。
4. 异步 close 允许在已取消 Job 的 finally 中执行，必要收尾不被该 Job 的取消提前中断。并发 close 等待同一次收尾，完成后的再次 close 幂等；正文与清理同时失败沿用 ResourceUseException。

M49 的内部 LoopResource 登记覆盖它自己创建的 socket／file 等资源。实现 AsyncCloseable 不会自动把 TLS 的 SSL／BIO 登记给 EventLoop；run 收尾关闭底层 socket，也不能代替 TLS 本身的释放。正常用法以 use／useAsync 或 try/finally 明确关闭 TLS 包装层。

GC release hook 不负责取消、驱动、等待、恢复或调用 OpenSSL 析构链；本期不承诺遗漏 close 的 TLS native 资源由 GC 完整回收，也不新增异步析构服务。确定性资源正确性由上述显式生命周期承担。

### 11.3 协议结束与截断

收到有效 close_notify 后，read 才将已交付明文之后的正常输入结束报告为零。底层提前 EOF、缺少关闭通知或不完整记录按实际 OpenSSL 状态报告截断／协议错误；不默认启用把 unexpected EOF 当成正常关闭的兼容选项。

shutdownGracefully 是独占读写方向的显式操作：应用先完成自己的收发协议，TLS 排出已接受输出，驱动 SSL_shutdown 发出并等待对端 close_notify，成功后完成本地关闭。SSL_shutdown 返回零仅表示关闭尚未双向完成，不能据此立即宣布成功。

优雅关闭不替应用静默丢弃尚未消费的应用明文；遇到这类数据须报告未完成应用读取的错误并执行本地收尾。取消或协议失败同样进入本地 close。等待对端可能没有固定时限；异步应用使用现有 Job 和定时器组合控制等待，不在本里程碑新增通用超时框架。只需要释放资源时直接调用 close。

## 12. 验收

主要入口为 crypto、TLS 和 async TLS 三个 Cone 的 `scoop.test`，以及普通外部 consumer-tests。密码与证书用例以显式内存数据、标准向量和明确验证时刻执行；传输用例使用 loopback、动态端口、自有测试 CA／证书和明确同步点，不依赖公网、系统信任库或任意 sleep。只有现有测试入口无法表达某个具体编译／产物边界时才增加最小 fixture，并说明原因；测试基础设施缺陷先在所属层修复。

| 类别 | 必须覆盖的行为 |
| --- | --- |
| Native 接入 | 三目标上游构建、初始化、libcrypto 独立运行与 libssl 握手；静态 archive 的依赖闭合、头文件与目标匹配；公开 C helper 的正常消费 |
| musl 自带依赖 | 在未安装 musl 专用 OpenSSL 开发包的普通 Linux 构建机上，从 vendor 源码准备目标资源；外部消费者分别执行纯 crypto 和 TLS 构建／运行；不要求运行环境安装 libssl／libcrypto 共享库 |
| 摘要／MAC／AEAD | 标准向量、空输入、增量与一次性结果相同、上下文状态；MAC 匹配与不匹配；AEAD 的 nonce／AAD／tag、篡改后认证失败且不交付明文、非法参数 |
| 密钥与签名 | 支持算法的密钥生成及 PEM／DER 往返、公私钥职责、跨工具签名与验签、数据／签名改变、不匹配密钥和算法、native 对象关闭及引用保留 |
| 独立证书验证 | 内存 PEM／DER、属性与公钥、单张证书签名和完整链验证的区别；显式信任锚／中间证书／时刻／用途；过期、缺链、未知根及 DNS／IP 身份错误，无网络和文件加载 |
| JWT／JWS | HS256／RS256／PS256／ES256／EdDSA 标准或互操作向量、原始 signing input、Base64URL 无 padding、ES256 原始签名格式、PSS 参数、JWK 公钥导入；算法／密钥不匹配、none、未知 critical header 和修改后的 token 拒绝；签名结果不冒充 claims 检查 |
| 同步客户端／服务端 | TLS 1.2／1.3、platform TCP 双向传输、短读写、flush、证书和名称验证、正常关闭与失败释放 |
| 异步客户端／服务端 | 经 EventLoop 接口解析、连接、accept 和握手；多个连接同时推进；同步完成与真实挂起；只读／只写调用也能完成反向协议 I/O |
| 双向与背压 | read 等待输入时 write 仍可推进；不同 buffer 一读一写并发；同方向拒绝；有限 BIO、底层短传输和输出背压；WANT 重试不重复消费或重发 |
| 身份与配置 | 可信链、未知 CA、过期证书、DNS／IP SAN 匹配与不匹配、证书私钥不匹配、SNI 与验证身份独立；ALPN 成功、缺省和无共同项 |
| 取消与关闭 | 登记前取消、完成／取消两种获胜顺序；握手、read、write、flush、shutdown 中取消；close 与在途操作竞争；关闭等待期间的必要收尾 |
| 进度与 GC | 明文接受后密文发送失败、部分输入与取消、借用恰好完成一次、无重复借用；真实挂起期间 moving GC、上下文／连接／buffer 保活及失败路径释放 |
| EOF 与互操作 | close_notify、未经 TLS 关闭的 TCP EOF、截断记录、优雅关闭仍有应用数据；Scoop 两端互通，并分别与独立 OpenSSL 客户端／服务端互通 |
| 外部产物与分层 | 分别消费生产 crypto、TLS 与 async TLS `.slib`；纯 crypto 的生产依赖不引入 platform／asyncio／libssl／libuv，同步 TLS 不引入 asyncio／libuv；公开密钥／证书身份、跨 Cone native 引用、泛型包装和资源辅助函数与 GC 组合 |

密码操作使用上游或标准向量及跨工具产物验证，TLS 独立对端使用上游命令行工具或普通 native 测试依赖；不能只靠同一套 Scoop 实现签后自验或互相通信就宣布互操作完成。生产依赖边界以各自独立的消费者构建验证，不把测试框架的 test-dependencies 算作 crypto 的生产依赖。musl 用例保留宿主已有的库和工具，以明确选择的 vendor 目标输入及真实构建／运行验证交付，不通过删除或隐藏宿主库来证明未使用它。证书输入明确 SAN、有效期和信任关系，错误用例断言错误类别及必要原始信息，不锁定整段上游诊断文本。

三个目标均执行 crypto、同步 TLS、异步 TLS 和外部消费者的 debug／release／moving GC 组合，GNU 和 musl 分别记录真实执行结果。与 M50 合并后的相关 native、FFI、GC 和 CLI 回归按实际变更选择；不能用本机静态发射或零项筛选代替 Linux 运行，也不把重复全量测试当作每个小步骤的门槛。

原始日志、报告和临时工作目录放在仓库外。验收文档只保留执行提交、目标／模式、覆盖概要、失败闭合和仍有的限制；设计、已实现与已验收状态分别记录。

## 13. 实施顺序与文档落地

1. **M51-1：Native 闭环与规范定稿。** 核对稳定补丁版本并 vendor 上游源码，完成三目标产物准备、libcrypto 独立运行和 libssl 最小握手；落实目标包发布，先验证 musl 不依赖发行版 OpenSSL；确定三个 Cone 的公开边界，将库行为分别写入 crypto／TLS spec。
2. **M51-2：密码与密钥。** 完成摘要、HMAC、AEAD、安全随机、密钥和签名 API，以及同步 FFI、错误及显式资源生命周期；以标准向量、跨工具产物和纯 crypto 外部消费者验证。
3. **M51-3：证书与 JWS。** 完成内存证书／信任存储、独立链验证、JWS 签名和验签、支持算法的 JWK 公钥导入；验证名称、时间、算法／密钥匹配、签名格式及无 I/O 的生产依赖。
4. **M51-4：公共 TLS 引擎与同步流。** 复用 crypto 对象完成上下文、BIO pair、同步 client／server 包装、明文进度和两种关闭；验证 platform TCP、独立对端及失败资源释放，同步 TLS 不依赖 asyncio。
5. **M51-5：M49 异步适配。** 独立 async TLS Cone 使用既有流与借用合同完成握手、读写、flush、双向推进、取消和 close；覆盖真实挂起、并发、背压、跨 Cone 身份与 moving GC。
6. **M51-6：组合验收与交付记录。** 在实际实施提交上完成三个目标和三个 Cone 的模式组合、外部生产产物消费、互操作及必要回归，确认 vendor 目标资源足够下游使用，再将 M51 标记完成。

每批实现先同步对应规范，按修改语言完成格式化与 lint，再运行适用测试。文档分工如下：

- 新的 `SCOOP-CRYPTO-SPEC.md` 承载内存密码 API、算法／签名格式、密钥与证书、独立验证、JWS 及资源生命周期；新的 `SCOOP-TLS-SPEC.md` 承载两个传输 Cone 的公共配置、默认身份验证、所有权、进度、并发、取消和关闭合同。本文保留架构、依据、实施与验收安排。
- platform／asyncio 规范按需增加 TLS 使用关系的引用，保留已交付的基础流与借用合同；不因新封装层改变原始 TCP 的取消或 EOF 语义。
- 语言／实现规范仅在新增普通库入口说明或确有构建／FFI 合同变化时更新；runtime 无密码／TLS 专属对象、入口或 ABI 变化。
- 实施后另建精简进度与验收记录，区分 OpenSSL 实际验证和此前 M49 的历史验收。本次设计不提前创建通过记录。

## 14. 上游依据

- OpenSSL：[源码与维护分支](https://openssl-library.org/source/)、[发布策略](https://openssl-library.org/policies/releasestrat/)。
- 密码与证书：[EVP 摘要](https://docs.openssl.org/3.5/man3/EVP_DigestInit/)、[EVP 签名](https://docs.openssl.org/3.5/man3/EVP_DigestSignInit/)、[EVP 加解密](https://docs.openssl.org/3.5/man3/EVP_EncryptInit/)、[X.509 链验证](https://docs.openssl.org/3.5/man3/X509_verify_cert/)。
- JWS：[RFC 7515](https://www.rfc-editor.org/rfc/rfc7515)、[算法与签名格式 RFC 7518](https://www.rfc-editor.org/rfc/rfc7518)、[EdDSA／OKP RFC 8037](https://www.rfc-editor.org/rfc/rfc8037)、[JWT 实践 RFC 8725](https://www.rfc-editor.org/rfc/rfc8725)。
- TLS 调用与错误：[SSL_get_error](https://docs.openssl.org/3.5/man3/SSL_get_error/)、[SSL_write](https://docs.openssl.org/3.5/man3/SSL_write/)。
- 传输与关闭：[BIO pair](https://docs.openssl.org/3.5/man3/BIO_s_bio/)、[SSL_shutdown](https://docs.openssl.org/3.5/man3/SSL_shutdown/)。
- 验证与身份：[SSL_CTX_set_verify](https://docs.openssl.org/3.5/man3/SSL_CTX_set_verify/)、[SSL_set1_host](https://docs.openssl.org/3.5/man3/SSL_set1_host/)、[IP 验证参数](https://docs.openssl.org/3.5/man3/X509_VERIFY_PARAM_set_flags/)。
- LibreSSL：[发布记录](https://www.libressl.org/releases.html)、[Portable README](https://github.com/libressl/portable/blob/v4.3.3/README.md)。
