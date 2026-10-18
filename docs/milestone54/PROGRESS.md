# M54 实施进度

状态：实施中。完整范围见[设计](DESIGN.md)，本页只记录实际实现与验证；Linux 与最终三目标验收尚未进行。

## 已完成的功能

| 功能 | 实现与边界 | 已验证范围 |
| --- | --- | --- |
| HTTP 协议摘要 | `scoop.crypto` 增加 SHA-1、MD5、SHA-512/256 的一次性与增量摘要；HMAC、签名及 JWS 算法集合保持原合同 | Darwin debug／release 与 moving GC：3 项新测试及 3 项既有摘要／MAC 测试；标准空输入与 abc 向量、WebSocket Accept、HTTP Digest 凭据、增量状态和 HMAC 参数边界 |
| HTTP/1 扩展方法 | 编码器与客户端响应上下文接受任意合法 token，HEAD／CONNECT 按精确大小写分类；服务端 llhttp 已知方法限制保留 | Darwin debug／release 与 moving GC：28 项 HTTP/1 测试，包括 4 项新增方法测试；合法 token 保真、非法 token 不改变状态、信息响应和混合方法关联 |
| 多证书 PEM | crypto 提供 `parseCertificatesPem` 与 `TrustStore.fromPem`；保留顺序、独立拥有证书，拒绝空包、坏片段与非证书内容，失败释放部分结果 | Darwin debug／release 与 moving GC：3 项 bundle 测试与 3 项既有证书测试；重复证书的独立关闭、显式信任链、临时 bundle 释放后的锚保活 |
| 独立 IDNA | `scoop.idna` 提供完整域名／单标签双向转换、OrNone、`IdnaName` 双表示快照、结构化错误及原始 RFC 3492 codec；固定 libidn2 2.3.8、libunistring 1.4.2、libiconv 1.19，无 ICU | Darwin debug／release 与 moving GC：10 项测试；官方 Unicode 15.1 的 6265 个输入与同版本映射表独立对照，无未解释差异 |
| 独立 URL | `scoop.url` 提供不可变 URI／IRI、组件 builder、相对解析、网络 origin、HTTP target、百分号／表单与有序重复 query；固定 uriparser 1.0.2，复用 IDNA 的已验证双表示 | Darwin debug／release 与 moving GC：12 项测试，包含 RFC 3986 第 5 节的 42 个解析向量、字节位置映射、IPv4／IPv6、IRI、空组件与编码边界 |
| 独立字符集 | `scoop.charset` 提供固定名称／别名、严格与有损内存转换、BOM／字节序配置及保留状态的增量引擎；固定 libiconv 1.19，108 种编码、352 个可查名称 | Darwin debug／release 与 moving GC：11 项测试；另有 864 组与 Python 标准编码器的独立分块对照，已验证全部名称能解析 |
| 压缩内存与增量引擎 | `scoop.compression` 提供 gzip／zlib／raw DEFLATE／Brotli／zstd 的完整内存操作和有状态编码／解码，显式字典、元数据、同步 flush、结束与局部限制 | Darwin debug／release 与 moving GC：12 项测试；384 组与独立 gzip／zlib／Brotli／zstd 工具的分块对照，另用独立工具验证 gzip 元数据与预共享 zstd 字典 |
| 压缩同步流 | 同一 Cone 的 CompressionSource／Sink、DecompressionSource／Sink 与两个复制扩展，共享增量引擎；支持短传输、单字节缓冲、明确 finish 和可选端点接管 | Darwin debug／release 与 moving GC：10 项新增测试及 12 项引擎回归；所有格式的增量流、连续适配、结束检查、实际进度和清理原因均通过 |
| 压缩异步流 | 同一 Cone 的 AsyncCompressionSource／Sink、AsyncDecompressionSource／Sink 与复制扩展；结构化操作、有限背压、取消后真实收尾和并发 close，共享同步流的格式状态 | Darwin debug／release 与 moving GC：12 项新增测试；五种格式、短读写、EOF／限制、所有权、失败隔离、延迟取消和取消 finally 清理全部通过 |
| DEFLATE 消息 | 同一 Cone 的 DeflateMessageEncoder／Decoder 负责 RFC 7692 的同步尾部、消息结束、协商窗口与 context takeover；HTTP 无须改写压缩字节 | Darwin debug／release 与 moving GC：6 项新增测试；504 组独立 zlib 消息互操作对照；现有全部 40 项 debug／release 测试及 384 组普通压缩对照回归通过 |
| HTTP 媒体类型 | `scoop.http` 提供 HttpMediaType、HttpParameter、原始字节快照、quoted-string 解析和可发送字段；格式异常保留字段名与原始字节位置 | Darwin debug／release 与 moving GC：4 项新增测试；HTTP 全部 58 项 debug／release 回归通过，涵盖大小写、空参数项、转义、重复、非法结构和非 UTF-8 字节 |
| HTTP 扩展文件名 | HttpContentDisposition、RFC 8187 参数编码／解码与 HttpFilename 选择结果；复用 url／charset，保留原始字段、失败原因及字节位置，ISO-8859-1 扩展兼容显式开启 | Darwin debug／release 与 moving GC：7 项新增测试；HTTP 全部 65 项 debug／release 回归通过，覆盖 UTF-8、BOM、普通 filename、回退、坏编码及语言标签结构 |
| HTTP 日期与 Retry-After | HttpDate 接收三种标准日期形状、只生成 IMF-fixdate，旧年份使用显式参考时间；HttpRetryAfter 区分非负秒数和日期，返回相对等待量 | Darwin debug／release 与 moving GC：6 项新增测试；HTTP 全部 71 项 debug／release 回归通过，覆盖 50 年边界、闰日、星期、秒 60、负 epoch、字段偏移及 Long 最大秒数 |
| HTTP 字节范围 | HttpRange 保留顺序与重复，解析后缀／开放终点并按实际表示长度求区间；HttpContentRange 表示部分响应与不可满足响应，区分未知总长度和零长度 | Darwin debug／release 与 moving GC：7 项新增测试及共享十进制解析的 2 项 Retry-After 回归；HTTP 全部 78 项 debug／release 通过，涵盖空表示的非零后缀、Long 边界、原始快照及非法结构 |
| Cache-Control | HttpCacheControl 合并多条字段并保留原始行、未知扩展与重复项；HttpCacheDirective 提供秒数及字段列表解释，严格单值查询报告重复，输出遵守标准参数形状 | Darwin debug／release 与 moving GC：7 项新增测试；HTTP 全部 85 项 debug／release 回归通过，覆盖 quoted 逗号、字节快照、转义位置、Long 饱和、无参数与空参数的区别 |
| ETag 与条件列表 | HttpEntityTag 保留 opaque 字节，提供弱／强比较；HttpEntityTagCondition 区分通配符与有序列表，并生成 If-Match／If-None-Match 字段 | Darwin debug／release 与 moving GC：6 项新增测试；HTTP 全部 91 项 debug／release 回归通过，覆盖 RFC 比较矩阵、字面反斜杠、非 UTF-8 字节、空标签／空列表、逗号和原始位置 |
| HTTP 认证字段 | HttpAuthChallenge、HttpCredentials 与 HttpAuthenticationInfo 共用字节语法，合并同名字段，保留未知方案、quoted 参数和 token68；HttpParameter 支持显式输出引号 | Darwin debug／release 与 moving GC：11 项新增测试；HTTP 全部 102 项 debug／release 回归通过，覆盖多方案与跨行参数、重复名、padding、非 UTF-8、字段方向及错误偏移 |
| 公共后缀 | PublicSuffixList 提供不可变 PSL 索引、普通／通配／例外匹配、ICANN／PRIVATE 选择和域名边界结果，直接复用 URL／IDNA 的规范名称；随库交付 2026-10-17 固定快照及许可 | Darwin debug／release：5 项新测试和 HTTP 全部 107 项通过；10398 条规则、77 个官方向量通过，4 项代表性规则测试通过 full moving，完整快照通过 minor moving |
| Cookie 消息与规则 | HttpCookiePair／Header／Attribute／SetCookie 保存有序字节数据；独立 Cookie 日期、标准属性、作用域、前缀、过期与显式 SameSite 判断，复用 URL／IDNA／PSL 和标准 Result | Darwin debug／release 与 moving GC：18 项新测试；HTTP 全部 125 项 debug／release 通过，覆盖无名值、重复与未知属性、控制字节／长度、溢出、IP／编码路径、公共后缀和拒绝原因 |
| Age 与 Vary | HttpAge 遵守 RFC 9111 首值选择与秒数饱和，HttpVary 保留字段名、重复与通配符；与 Cache-Control 复用列表及数值解析 | Darwin debug／release：7 项新测试与 HTTP 全部 132 项通过；新测试及 4 项 Cache-Control 共享解析回归通过 moving GC，Range／Retry-After 仍拒绝溢出 |
| SSE 内存解析 | SseDecoder 增量处理行结束、BOM、字段、ID／retry 和 EOF；UTF-8 复用 charset，有界行／data 与单个待交付事件，原生输入借用有明确消费量 | Darwin debug／release 与 moving GC：10 项新测试；HTTP 全部 142 项 debug／release 通过，涵盖 HTML 示例、单字节分块、坏 UTF-8、空 ID／data、背压、失败后借用释放与 EOF |
| SSE 异步事件源 | SseSource 借用或显式接管 AsyncByteSource，以 next 驱动有限预取；结构化读取、取消与并发关闭等待真实借用结束，复用同一内存 parser | Darwin debug／release：9 项新测试和 HTTP asyncio 全部 35 项通过；新增测试通过 moving GC，覆盖短读、EOF、I/O／解析失败、错误 loop、并发读取、同一清理结果与取消 finally |
| 代理请求与 CONNECT | httpProxyRequest／httpConnectRequest 复用 URL 生成正确 target 与 Host，httpConnectAccepted 覆盖全部 2xx；修复 llhttp 适配对 CONNECT 拒绝响应的过早切换 | Darwin debug／release：8 项新测试及 HTTP 150 项验证闭合；新增测试和既有 4 项异步切换测试通过 moving GC，覆盖 1xx、407、chunked／EOF、IPv6、H2 普通 CONNECT 与完整隧道后缀 |
| Multipart 内存编解码 | MultipartBoundary／PartHead、编码器与增量 decoder；保留二进制 part，单个待取事件与有限 boundary 前缀，生成已知长度及 RFC 7578 form-data 元数据，随机 boundary 复用 crypto | Darwin debug／release 与 moving GC：16 项新测试；HTTP 全部 166 项 debug／release 通过，另有 288 组 Python MIME 双向对照，覆盖短块、padding、未知长度、重复／折行字段、空 part、EOF 与局部限制 |

正式 fixture `m54-protocol-digests` 在 Darwin 的 debug／release 两个变体通过，共 6 个进程；覆盖生产 `.slib` 跨 Cone 消费、moving GC 和构建缓存复用。该结果不代替尚未进行的 Linux 验证。

`m54-http1-methods` 通过真实 TCP 与独立 C 对端核对 CUSTOM、HEAD、head、connect 及 100／最终响应；debug／release、moving GC 与缓存复用通过。既有 `m53-http1-consumer`、`m53-http1-async-consumer` 同时回归通过，共 6 个变体、18 个进程。原生对端用例初次因消费后未重置测试缓冲区失败，修正用例后全部通过。

`m54-certificate-bundle` 的 debug／release 两个变体通过，共 6 个进程；从生产 crypto `.slib` 解析双证书、构造信任库、验证证书并拒绝坏尾部，覆盖 moving GC 与缓存复用。

`m54-idna` 的 debug／release 两个变体通过，共 6 个进程；生产 `.slib` 的两个 native 线程并行调用、A-label／Unicode、非法 ContextO、原始 Punycode、moving GC 和缓存复用均通过。后端准备脚本的第二次调用直接复用三个目标静态库。

URL 的传输／展示组合需要同时取得域名两种表示，因此 IDNA 补充 `Idna.parse`：一次 lookup 后复用已验证的 ASCII 结果解码，返回 `IdnaName` 独立快照。新增测试及正式 fixture 均验证 native 临时数据释放、GC 与并行调用后的两种表示，URL 无须重复 IDNA 验证。

IDNA 校验闭合了固定后端的 ContextO、整个域名的 Bidi、RTL 数字混用、末尾 NSM 和 NFC 前过早限长问题；raw Punycode 在同一后端算法中补齐 ASCII 基本字符与末尾分隔符，并修正非法数字字符和代理码点检查。静态后端的私有 gnulib 重复符号各自使用前缀，未修改编译器链接规则。官方对照中 379 个输入的 ASCII／Unicode 输出相符，5747 个错误输入被拒绝；另 137 个按映射表 NV8／XV8 标记由严格 IDNA2008 拒绝，2 个根域输入按公开 profile 接受。原始测试数据、对照脚本和日志保存在仓库外。

`m54-url` 的 debug／release 两个变体通过，共 6 个进程；消费者只声明 URL 而通过其正式依赖取得 IDNA，在两个 native 线程与主线程中组合解析、IRI、builder、query、origin 和 moving GC，确认不可变值的复用与构建缓存命中。uriparser 后端准备、第二次直接复用及生产 `.slib` 构建均通过；相对解析结果显式取得 native 文本所有权，再释放 base／reference 临时状态。

`m54-charset` 的 debug／release 两个变体通过，共 6 个进程；生产消费者组合 UTF-16、ISO-2022-JP、GBK、URL 和 IDNA，使用单字节输出、两个 native 线程、moving GC 与缓存复用。严格后端补丁关闭默认丢弃 Unicode tag 和隐式 U+FFFD 替代，有损编码在同一目标编码器中生成 `?`，不破坏 shift 状态。BOM 分块、非法／截断输入、不可表示字符的原始偏移、错误后关闭与 borrow 清理已覆盖；共享后端更新后 IDNA 的 10 项回归通过，准备脚本再次直接复用三个库。

压缩后端固定为 Zstandard 1.5.7、zlib-ng 2.3.3 和 Brotli 1.2.0，维护脚本与分发清单已接入，第二次准备直接复用三个后端。增量引擎允许单字节输出，正确保留有限的待交付字节；按实际 native 分配调整 GC 压力。测试覆盖各格式空流、截断、坏校验和、尾随数据、gzip／zstd 连续流与 skippable frame、输出／窗口上限、flush 后继续输入、字典关闭后的引擎保活，以及错误后诊断和资源清理。

`m54-compression` 的 debug／release 两个变体通过，共 6 个进程；从正式压缩 `.slib` 消费五种格式，组合 URL／IDNA、UTF-16、单字节输出、两个 native 线程与主线程、moving GC 和缓存复用。

`m54-compression-streams` 的 debug／release 两个变体通过，共 6 个进程；正式 `.slib` 与真实 File 组合，对五种格式逐段处理 17321 字节数据，验证不同大小缓冲、flush 后 finish、接管端点关闭、moving GC 和缓存复用。初次用例误把 argv[0] 当作传入目录，改用实际参数 argv[1] 后全部通过。

`m54-compression-async` 的 debug／release 两个变体通过，共 6 个进程；正式 `.slib` 在同一 LibuvEventLoop 上并行执行 gzip 和 zstd 的真实文件／TCP 上传下载，每条处理 12345 字节，同时取消另一条解压读取。moving GC、借用连接继续使用、其他传输完成及缓存复用均通过。延迟读写测试还确认 close 等待真实 borrow 完成、仅取消当前操作、拒绝同向并发，并保留异常路径的实际写出进度；取消的 Job 可在 finally 中完成资源关闭。

DEFLATE 消息对照包含编码窗口 9～15、解码窗口 8～15、两种 context takeover、单字节输入／输出、连续空消息和独立对端保留字典的后续消息；RFC 向量覆盖 BFINAL、零填充、截断和错误尾部。每消息输出限制与 totals 在正常 nextMessage 后重新开始，失败状态不能复用。固定 zlib-ng 的 raw 模式原本允许当前输出 buffer 内的距离超出协商窗口，vendor 补丁令严格上限使用实际 windowBits；独立距离 300 的向量在 8 位窗口被拒绝、9 位窗口正确解码。后端准备再次直接复用三个静态库。

`m54-deflate-messages` 的 debug／release 两个变体通过，共 6 个进程；正式 `.slib` 的两个 native 线程和主线程分别推进保留／丢弃历史的消息，组合非 ASCII 文本、短输出、重复空消息、独立压缩向量、BFINAL、moving GC 与缓存复用。

`m54-http-media-type` 的 debug／release 两个变体通过，共 6 个进程；生产 HTTP 与 charset `.slib` 组合 Content-Type、ISO-8859-1 正文和非 UTF-8 参数，按 7 字节输入推进 HTTP/1，覆盖 moving GC 和缓存复用。初次消费者误捕获可变局部变量并产生 enum catch-all 警告，改用显式集合及精确模式后全部通过。

`m54-content-disposition` 的 debug／release 两个变体通过，共 6 个进程；消费者只声明 HTTP 依赖，经其正式依赖取得 URL／charset，在逐字节的两条 HTTP/1 响应中处理中文扩展文件名、普通回退和解码失败原因，覆盖 moving GC 与缓存复用。UTF-8 与百分号错误保留底层原因并映射到原始字段字节位置；文件名仅为元数据，不选择文件路径。

HTTP 日期的 release 编译暴露了跨无写入分支的同槽根加载直到机器发射才合并的问题。codegen 在根计划定稿前复用 LLVM MemorySSA 消除重复加载，保留所有逻辑引用的恢复；没有放宽 stackmap 校验或改写 HTTP 源码绕过。最小 LLVM 用例及独立 Scoop 程序均在修复前复现计数不一致。全部 224 项 codegen 测试通过，其中根计数回归覆盖 Darwin、Linux gnu／musl 的机器发射以及分支确有写入的对照；`m54-gc-branch-loads` 的 Darwin debug／release 两个变体通过，共 6 个进程，验证显式收集、moving GC 与两个引用选择路径。HTTP 的 71 项 release 测试随后通过。

`m54-http-dates` 的 debug／release 两个变体通过，共 6 个进程；生产 HTTP `.slib` 的消费者逐字节解析 503 响应中的两种旧日期，计算 Retry-After 相对等待量，并验证大秒数、负 epoch、moving GC 与缓存复用。修复编译器后新增 6 项测试的 release moving GC 也全部通过；日期字段的年限与星期语义按 RFC 9110 引用的 RFC 5322 检查，时区与参考时刻始终显式。

`m54-http-ranges` 的 debug／release 两个变体通过，共 6 个进程；生产 HTTP 与 compression `.slib` 的消费者按 3 字节输入解析两条 206 响应，用 Content-Range 重组 gzip 表示并解压核对正文，再验证 416、空表示后缀、moving GC 与缓存复用。所有范围都相对于编码后的字节序列；没有在 HTTP 或 client 中另建压缩处理。

`m54-cache-control` 的 debug／release 两个变体通过，共 6 个进程；生产 `.slib` 的消费者逐字节解析带表示长度元数据的 304 响应，合并两条 Cache-Control，组合显式日期与 max-age，并验证字段名列表、未知参数、饱和秒数、moving GC 与缓存复用。Cache-Control 只处理消息数据，尚未以此宣告 client 缓存策略或存储实现完成。

`m54-entity-tags` 的 debug／release 两个变体通过，共 6 个进程；生产 `.slib` 生成包含非 UTF-8、逗号与反斜杠的 If-None-Match 请求，由 HTTP/1 服务端 parser 逐字节接收，再解析带弱 ETag 和 Cache-Control 的 304 响应。强弱比较、opaque 保真、通配符、moving GC 与缓存复用均通过；未把普通 quoted-string 的反转义用于 ETag。

`m54-http-auth` 的 debug／release 两个变体通过，共 6 个进程；生产 `.slib` 逐字节接收含跨行 Digest 参数和另一个 token68 challenge 的 401 响应，组合 crypto 核对 RFC 7616 §3.9.1 的 SHA-256 响应向量，生成并解析 Authorization，再处理多行 Authentication-Info。显式引号、代理字段、moving GC 与缓存复用均通过；此处只交付字段语法，client 的认证选择、凭据刷新和网络往返仍待实现。

`m54-public-suffix` 的 debug／release 两个变体通过，共 8 个进程；生产 `.slib` 在两个 native 线程和主线程之间复用同一个只读规则索引，组合 Unicode URL、已验证 IDNA 名称、IP 排除、通配／例外和私有后缀。完整快照在 minor moving 下验证，包含所有规则类型的小列表在 full moving 下验证，随后正常复用构建缓存。消费者直接声明其使用的 idna／url 依赖，未改变语言的直接依赖可见性规则。

完整 PSL 的逐分配 full moving 试跑因该诊断模式的全堆搬迁及永久隔离开销而主动终止；正式 GC 覆盖采用上述两种既有模式。没有修改 runtime、缩减内置 PSL 或以自生成结果替代官方向量。静态数据生成脚本通过固定版本 Ruff，C 数据接口通过格式化和严格警告检查；分发复用既有 vendor／license 收集路径。

`m54-http-cookies` 的 debug／release 两个变体通过，共 6 个进程；生产 `.slib` 逐字节接收八条独立 Set-Cookie，组合显式时间、规范 URL 和小型 PSL，检查过期、公共后缀、SameSite、编码路径，再生成并解析 Cookie 请求。日期中的逗号、同名值顺序、非 UTF-8 字节、快照、moving GC 与缓存复用均通过。接收兼容基线明确采用 RFC 6265 与 6265bis-22 草案规则，Cookie 日期不复用 HTTP-date 的严格形状；Cookie jar 的存储、替换和发送策略仍待 client 实现。

`m54-cache-metadata` 的 debug／release 两个变体通过，共 6 个进程；生产消费者逐字节接收 304 响应中的列表／重复 Age 与多行 Vary，组合 Date、Cache-Control，重新生成并解析响应字段。首值保留、名称顺序、通配符、饱和、moving GC 与构建复用均通过；这些工具只交付消息数据，不计算或存储实际缓存条目。

`m54-sse-decoder` 的 debug／release 两个变体通过，共 6 个进程；生产 HTTP `.slib` 将 SSE 正文编码成三字节 chunked 块，消费者逐字节完成 HTTP/1 定界后交给 SSE。UTF-8、CRLF、控制块、ID 清除、坏字符替换、不完整尾部、快照、moving GC 与缓存复用均通过。解析器不执行网络或重连，异步 SseSource 与 client 重连策略另行完成。

`m54-sse-async` 的 debug／release 两个变体通过，共 6 个进程；正式 `.slib` 通过真实 TCP 在 HTTP/1 chunked 与 HTTP/2 小窗口上传输 SSE，组合短读写、trailer、ID／retry 与不完整尾部。取消另一个等待事件的 H2 stream 后，正常事件流与后续请求继续完成；moving GC、所有权关闭和缓存复用均通过。库回归前按正常构建入口刷新了使用旧 crypto 依赖指纹的 TLS 预编译资源，未改动其源码或跳过依赖检查。自动重连仍由后续 client 实现。

`m54-http-proxy` 的 debug／release 两个变体通过，共 6 个进程；真实 TCP 对端直接检查线上请求字节，在同一连接依次返回普通正文、407 拒绝正文及带隧道后缀的 201 成功响应。userinfo／fragment 排除、编码路径、认证字段、非 200 的成功状态、忽略成功 CONNECT 的 Content-Length、关闭旧对象后继续双向通信、moving GC 与缓存复用均通过。

CONNECT 新测试先复现了 llhttp 将所有 CONNECT 响应视为切换的问题。HTTP 适配现在保留真实请求上下文，在 headers-complete 中仅依据 2xx 声明 CONNECT 成功，未改写线上方法。全部 HTTP 回归确认其余功能正常；EOF 用例改为显式控制辅助函数的 EOF 后，相关 3 项定界测试单独复验通过。C 改动经过格式化和严格编译检查。

`m54-multipart` 的 debug／release 两个变体通过，共 6 个进程；生产 `.slib` 组合需要引号的 boundary、中文 form-data 元数据、非 UTF-8 字段、部分匹配正文、嵌套 multipart、gzip 与 HTTP/1 chunked／trailer。单字节事件、移动 GC 和缓存复用均通过。相同消费者另读取 Python EmailMessage 生成的 144 组输入，再把 Scoop 输出交给 BytesParser，两个方向共 288 组通过；原始向量与输出保存在仓库外。

Multipart 的通用字段保留扩展参数，form-data 工厂遵守 RFC 7578，不生成 filename*。接收端按 delimiter 定界，不信任 part 的 Content-Length；closing delimiter 与外层 EOF 分别可见，preamble／epilogue 按消费丢弃。此批仅完成内存引擎，异步 part 流与 client 重放组合继续独立完成。

## 后续工作

继续完成 HTTP 消息与协议补充、TLS 与 asyncio 缺口，再完成统一 client、策略、存储与长连接组合。每个功能独立验证和提交，最终按设计完成三个目标的运行验收。

## 验证安排

复用 M53 已完成的工具与目标资源，改动的库通过正常准备／构建入口更新。先运行本机相关测试，待功能组合稳定后在 Linux 完整验证；普通库改动不重复运行无关编译器全量测试。原始日志、fixture 工作区与本期构建缓存保存在仓库外的 `scoop-work/m54`。

已清理本机旧 `target/debug/incremental`，释放约 8 GB；保留当前仍需复用的编译产物。后续按实际磁盘占用继续清理不再使用的 target 内容。

独立数据与压缩库本机验收完成后，清理仓库外对应的 debug／release target，共约 390 MB；生产 sysroot 与仍需使用的 HTTP／crypto 构建产物继续保留。

GC 编译器修复验收后，清理本轮 codegen／driver 的 13 个增量构建目录，释放约 505 MB；继续保留配套命令、正常依赖产物和当前 HTTP 构建缓存。

Cookie、缓存字段及 SSE 本机测试推进后，清理此前已经结束的 52 个正式 fixture target 目录，释放约 2.4 GiB；原始日志、报告、源 fixture 和编译缓存保留，当前库 target 与生产 sysroot 继续复用。

SSE 与代理组合验收后，清理对应 6 个已经结束的 fixture target 目录，释放约 446 MiB；当前 multipart 消费者、库 target、日志和构建缓存继续保留。
