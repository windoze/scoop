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

## 后续工作

继续完成 HTTP 消息与协议补充、TLS 与 asyncio 缺口，再完成统一 client、策略、存储与长连接组合。每个功能独立验证和提交，最终按设计完成三个目标的运行验收。

## 验证安排

复用 M53 已完成的工具与目标资源，改动的库通过正常准备／构建入口更新。先运行本机相关测试，待功能组合稳定后在 Linux 完整验证；普通库改动不重复运行无关编译器全量测试。原始日志、fixture 工作区与本期构建缓存保存在仓库外的 `scoop-work/m54`。

已清理本机旧 `target/debug/incremental`，释放约 8 GB；保留当前仍需复用的编译产物。后续按实际磁盘占用继续清理不再使用的 target 内容。

独立数据与压缩库本机验收完成后，清理仓库外对应的 debug／release target，共约 390 MB；生产 sysroot 与仍需使用的 HTTP／crypto 构建产物继续保留。
