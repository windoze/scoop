# Scoop TLS 库规范

版本：M51 已实施基线，含第 8 节的 M54 扩展。既有实现状态见 [M51 进度](../milestone51/PROGRESS.md)，新增范围与实施状态见 [M54 设计](../milestone54/DESIGN.md)及[进度](../milestone54/PROGRESS.md)。本规范与[密码库规范](SCOOP-CRYPTO-SPEC.md)共同规定三个普通库的边界。

## 1. 分层

`scoop:scoop.tls:0.1.0` 提供配置、公共协议引擎和同步流，依赖 crypto、platform 和实际使用的基础库，链接同一目标 OpenSSL libssl；不依赖 asyncio／libuv。`scoop:scoop.tls.asyncio:0.1.0` 依赖 TLS 和 asyncio，复用公共引擎与平台 NativeByteBuffer／NativeByteBorrow，不重复编译后端。

不提供 HTTP／DTLS／QUIC、自动重连、会话恢复 API、0-RTT、动态 SNI 或用户证书验证 callback，不新增 runtime ABI。客户端证书身份与系统信任按第 8 节在 M54 补齐。C 仅管理 SSL_CTX、SSL、有限 BIO pair、自有配置及必要的平台信任工作，不访问 runtime 私有头、不保存 managed 引用、不恢复 continuation。

## 2. 配置与身份

M51 的 TlsClientContext 显式接收 crypto TrustStore，M54 的系统来源工厂与可选客户端身份见第 8 节；TlsServerContext 接收 Certificate 链和 PrivateKey，创建时检查证书／私钥匹配并保留 native 引用。配置创建后不可变，允许多个连接共享；上下文实现 Closeable，close 后禁止新会话，已有 SSL 保留必要引用。单个对象关闭不执行 OPENSSL_cleanup。

客户端必须提供 TlsPeerName，DNS 与 IP 分开。DNS 非空、无 NUL、ASCII／A-label，分别设置 SNI 和主机名验证；IP 使用 4／16 字节 SAN 验证，不设置 DNS SNI。默认验证信任链、时间、用途和目标名称，没有跳过验证的高层开关。身份不从实际连接地址推导。TLS 不预先重复执行一次完整 crypto 链验证。

默认 TLS 1.2～1.3，允许显式限定为其中一个版本；使用上游正常密码配置。ALPN 为独立协议字节串列表，每项 1～255 字节，总编码长度至多 65535；服务端按自身顺序选共同项。双方配置 ALPN 而无共同项时报握手错误，未协商返回 None。协商信息提供实际 TLS 版本与可选 ALPN。

`TlsVersion { Tls12, Tls13 }`，`TlsOptions(minimum = Tls12, maximum = Tls13, alpn = 空列表)`。上下文构造为 `TlsClientContext(trustStore, options, identity = None)` 和 `TlsServerContext(certificates, privateKey, options)`；certificates 非空，顺序为叶证书、其后发送的中间证书，根无需发送。`TlsPeerName { Dns(String), Ip(IpAddress) }` 在 TLS 层将 platform 的地址转成 crypto 的内存身份。`TlsConnectionInfo(version, alpn)` 返回协商数据的独立副本。

## 3. 所有权与引擎

同步包装入口要求普通泛型约束 ByteSource、ByteSink、Closeable；异步要求 AsyncByteStream、AsyncCloseable。参数、状态和登记前取消检查成功后、开始握手前接管底层传输。此后的失败负责关闭传输与未交付 TLS 状态；接管前失败仍归调用者。同步工厂在 TlsEngine 会话创建成功后、分配流缓冲前接管，缓冲分配失败也负责关闭传输；异步工厂在握手请求成功登记时接管，登记前失败仍由调用者关闭传输。交付后调用者不得绕过包装层使用底层传输。

每个会话使用固定容量 BIO pair 和独立密文 staging buffer，不使用 SSL_set_fd 或 socket BIO。`scoop.tls.native` 提供有限同步握手、明文读写、shutdown、密文输入输出步骤。结果包含进度、WANT_READ／WANT_WRITE、close_notify 或已复制的错误。

公开 `TlsEngine.client(context, peer)`／`server(context)` 创建显式 Closeable 会话。handshake、read、write、shutdown 返回 `TlsStep(kind, count)`，M51 的 kind 为 Done／WantRead／WantWrite／Closed，M54 为系统验证增加第 8.3 节的 WantVerification；read／write 使用 Unsafe NativeByteBorrow，调用者负责串行与借用有效期。receiveCiphertext／takeCiphertext 消费／填充 NativeByteBuffer 的实际前缀；inputCapacity、pendingOutput 为 BIO 当前可写／可读量，endInput 标记传输 EOF。connectionInfo 仅在完成握手后使用。引擎不执行传输，不保留临时 managed 数据地址。适配器使用的明文前缀上限为 16384 字节，内部 BIO 容量不属于 ABI。

`TlsException` 保存 operation、kind、OpenSSL code、verifyCode、systemError 和 detail；`TlsFailureKind` 区分 Configuration、Verification、Protocol、Truncated、UnconsumedData。错误数据在同次 C 调用中复制；底层 I/O 异常仍按原类型传播。

同一 SSL 的调用及错误读取串行。调用前清空当前线程错误队列，调用后立即保存 errno 并执行 SSL_get_error，再提取验证 code 与错误文本。锁内不执行底层阻塞 I/O、suspend、continuation 恢复或用户代码。WANT 重试遵守上游调用顺序，写重试保留原数据、长度和地址；等待输入的应用 read 不能阻止 write 推进。

先驱动协议产生的密文，再等待必要输入；WANT_READ 不得遗留握手输出。异步输出背压时仍允许必要输入推进；同步接口遵循底层 ByteSink 阻塞调用的返回合同，不自行增加并发 I/O 线程。没有新输入、输出空间或操作状态变化时等待真实 I/O，不空转。

## 4. 明文进度与借用

readInto 返回已初始化的明文数量并推进 buffer writer，writeFrom 返回 SSL 已接受的明文数量并推进 reader；允许有限前缀短传输。写成功后密文发送失败或取消不回滚已接受量、不重新加密或重发。flush 排出本层待发送密文并调用底层 flush，不等待对端确认。

零长度在状态、范围和取消检查后返回零，不推进网络。正 read 的零只表示有效 close_notify；原始 EOF 交给 BIO／SSL 判断，缺少关闭通知或截断记录报协议／截断错误。

明文操作按需持有 NativeByteBorrow 直到 SSL 重试及必要收尾结束，所有出口恰好 complete 一次并提交实际前缀。密文 staging buffer 交给底层前结束本层同步访问，避免重复借用。跨挂起只保留稳定 native allocation，managed owner 与 continuation 使用普通 GC 根，不 pin wrapper 或保留 Array／String 临时指针。

## 5. 同步与异步协调

同步 TlsStream 实现 ByteSource、ByteSink、Closeable，调用者串行使用，包括关闭；不创建线程池。DNS、连接和 accept 组合既有 platform API。

同步入口为 `TlsStream.takeOwnershipAsClient(transport, context, peer)` 和 `TlsStream.takeOwnershipAsServer(transport, context)`，握手成功后返回 TlsStream。其 isClosed 查询关闭状态，connectionInfo 提供实际协商信息，readInto／writeFrom／flush 采用既有同步协议。状态／范围错误在登记前抛出，连接保持可用；已开始的协议或传输操作失败执行本地收尾。close 幂等，主动 shutdownGracefully 完成后同样处于已关闭状态。

AsyncTlsStream 实现 AsyncByteStream、AsyncCloseable，固定归属建立它的 EventLoop。普通操作检查当前 Job 与所属 Context；允许不同 buffer 各一个应用 read 和 write 在途，同方向并发报状态错误，flush 占输出方向。连接协调底层每方向至多一个操作。通知在状态更新后、锁外执行，无活动请求时不遗留无归属后台任务，不为每次 SSL 步骤创建 Job。

异步入口为 `AsyncTlsStream.takeOwnershipAsClient(transport, context, peer)` 和 `AsyncTlsStream.takeOwnershipAsServer(transport, context)`，完成握手后返回；返回的 AsyncTlsStream 提供 connectionInfo、isClosed 和 shutdownGracefully。连接以普通操作记录保留 continuation、所属 Job、进度和借用，每次实际传输通过公开 startCoroutine／Continuation 完成，传输结束后再决定可执行步骤与应用结果；不创建私有调度器、全局服务或逐步骤 Job。WANT_READ 的应用读可让写进入 SSL；待重试写及 WANT_WRITE 的读遵循上游重试次序。SSL 与错误提取在短锁内，实际传输、关闭、取消监听回调和 continuation 通知在锁外。

跨线程取消与第三方恢复使用连接自身的普通同步，不能调用 libuv 私有状态／投递函数；传输等待通过公开异步流。服务端逐连接握手，listener 的后续 accept 不依赖前一握手完成。

## 6. 取消与关闭

登记前取消不启动／接管；成功已获胜则迟到取消不覆盖。已登记但未完成的握手、读写、flush、shutdown 取消获胜时终止整条 TLS 连接。取消只选结果，仍等 native 步骤、底层操作／关闭、借用和资源收尾完成才恢复。其他活动请求以关闭结束，已选成功／取消保持原结果。

close 只保证本地释放，不隐含 flush 或对端关闭握手。拒绝新操作后关闭已接管传输并等其借用结束，再释放 SSL／BIO、提交明文进度、解除明文借用、释放 staging buffer。即使一步失败仍继续其他必要清理；正文与清理同时失败使用 ResourceUseException。

异步 close 可在已取消 Job 的 finally 中执行，收尾不可被提前取消；并发 close 等待同一次收尾，之后幂等。EventLoop 对原始 socket 的收尾不能代替 TLS wrapper 的 close；正常用法以 use／useAsync／try-finally 显式关闭。GC release hook 不驱动、不等待、不取消，也不调用 OpenSSL 析构链。

shutdownGracefully 独占读写方向，排出已接受输出，发出并等待对端 close_notify，成功后执行本地关闭。SSL_shutdown 返回零表示仍未双向完成；未消费应用数据不能静默丢弃，应报告错误并关闭。取消或失败同样本地收尾，等待上限由应用使用现有 Job／定时器控制。

## 7. 验证

各 Cone 的 scoop.test 与生产外部消费者覆盖算法、身份、TLS 1.2／1.3、ALPN、短传输、背压、双向并发、取消、关闭、moving GC 和独立 OpenSSL 对端。使用 loopback 动态端口、明确同步点和测试 CA，不依赖公网或系统信任。三个目标分别运行 debug／release／moving GC，不能以静态发射代替 Linux 运行；完整要求见 M51 设计第 12 节。

## 8. M54 身份与系统信任

### 8.1 客户端身份

提供不可变配置 `TlsIdentity(certificates: List<Certificate>, privateKey: PrivateKey)`，证书链非空，按叶证书到中间证书排序。创建时检查叶证书／私钥匹配，独立保留每张证书和私钥的 native 引用，实现 Closeable；显式关闭原 Certificate／PrivateKey wrapper 不使 identity 失效。isClosed 查询关闭状态，close 幂等；关闭后的身份不能配置新上下文。构造期间输入 wrapper 仍须有效，不允许其他使用者并发关闭它们。

`TlsIdentity.fromPem(certificates: String, privateKey: String)` 调用 crypto 的完整 bundle 与私钥 parser，在所有出口释放临时 wrapper。空链抛 IllegalArgumentException，已关闭输入抛 IllegalStateException，PEM 解析保留 CryptoException，证书／私钥不匹配以 TlsFailureKind.Configuration 报告；不通过导出／重新解析 PEM 来取得输入的所有权。

TlsClientContext 的最后一个可选参数为 identity，默认 None，旧的 `TlsClientContext(trustStore, options)` 调用仍合法。向 SSL_CTX 配置身份时取得独立 native 引用，因此 identity 关闭不影响已创建上下文，上下文关闭也不影响已有 SSL。身份的配置／关闭使用短临界区，不在其中执行 I/O 或用户代码。服务端原构造入口使用同一身份检查与 OpenSSL 配置路径，不增加另一套证书链复制实现。对端请求证书时按 TLS 协议发送配置的身份，不动态执行用户选择 callback。

客户端身份只影响本地配置与发送，服务端证书链、用途和 URL 名称验证继续执行。不同 identity 的连接不得在上层误用同一池键。mTLS 不扩展为证书固定、对端证书链查询或 TLS session 管理接口。

### 8.2 信任来源

引入 `TlsTrustSource { System, Store(TrustStore) }`；显式 Store 沿用 M51 的 OpenSSL 验证。CA 文件便利入口读取字节后调用 crypto 的 TrustStore.fromPem；同步入口在 TLS，异步入口在 TLS asyncio，后者使用 AsyncFile。System 不代表跳过验证或使用空 TrustStore。

Linux 的 System 装载目标运行环境的系统 CA 配置，保留加载失败、权限和格式错误；路径属于平台集成数据，不能使用构建机前缀或打包机证书。Darwin 的 System 使用系统信任验证，尊重系统／管理员／用户的 TLS 信任设置，不能将导出的根证书列表宣称为完整等价的系统政策。默认禁止验证过程触发额外 AIA／OCSP／CRL 网络获取，与现有离线验证边界一致。

系统信任集成和平台 native 链验证属于 scoop.tls。Darwin 通过目标条件链接 Security／CoreFoundation；Linux 继续使用目标 OpenSSL 和实际 CA 数据。普通 platform 只提供所需文件／OS 原语，不依赖 crypto，crypto 保持内存库。系统策略失败统一保存 TlsFailureKind.Verification 与实际平台原因，不能自动改用另一套更宽松验证。

System 的配置装载提供同步与 suspend 工厂；`scoop.tls.asyncio.openTlsClientContext(trust, options, identity)` 通过 asyncio 的 native work 等待可能阻塞的系统操作。所有输入先保留 native 副本／引用，worker 不访问 managed 对象；取消后关闭未交付上下文，不能在 web client 的 EventLoop 线程直接读取系统 keychain 或同步 CA 文件。

### 8.3 验证等待与共同引擎

需要系统异步验证时，公共引擎增加明确的 WantVerification 步骤，利用 OpenSSL client 的 retry-verify 机制暂停握手。TLS 层建立拥有证书链与策略输入的验证工作，取得完成结果后继续同一握手；不在 native certificate callback 内 suspend，不从 worker 恢复 Scoop continuation。

同步适配执行并等待同一验证工作，异步适配通过 asyncio native work 挂起。共同引擎、输入／输出背压和 SSL 重试规则保持一致，验证工作存活期间持有必需 native 引用。此工作仅供 TLS 适配组合，不作为新的用户证书 callback、证书链检查或信任插件 API。

每条验证路径只运行其真正负责的信任链检查；OpenSSL 仍检查握手和证书持有证明，系统策略路径不再机械重跑一次等价的全链验证。DNS／IP 名称、时间与用途是必要验证输入，不能因切换后端遗漏。系统失败、关闭或取消后等待验证工作结束才释放其引用；M51 的 TLS 单操作取消关闭整条 TLS 连接的合同保持不变。

### 8.4 验证

使用独立 OpenSSL mTLS 对端验证合法身份、缺少身份、错误私钥／链、服务器拒绝、共享配置关闭和取消。显式 CA 文件覆盖多证书、非法 bundle 与读取失败。系统信任测试在各目标验证装载／验证路径，并以隔离测试信任配置覆盖成功、拒绝和名称不符，不修改开发机真实系统信任设置，不以依赖公网站点作为成功条件。

补充验证工作暂停／恢复、运行中取消、迟到成功、系统错误与 moving GC。再组合 HTTPS／H2：一个请求取消只 reset HTTP 流，不能取消连接 owner 的 TLS read／write。上述检查独立于 M51 已完成记录。
