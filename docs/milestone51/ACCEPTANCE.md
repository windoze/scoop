# M51 验收

状态：已完成。日期：2026-10-17。功能与测试基线为 `b6edcc762`，随后 `846208c04` 修正既有 native I/O 用例的临时目录；收尾文档不改变实现。公共合同见[密码库规范](../specs/SCOOP-CRYPTO-SPEC.md)、[TLS 规范](../specs/SCOOP-TLS-SPEC.md)，构建与示例见 [USAGE.md](USAGE.md)。

## 交付范围

| 能力 | 验证入口 |
| --- | --- |
| 目标条件 native include、OpenSSL 三目标资源 | [include fixture](../../tests/fixtures/m51-native-includes)、[native OpenSSL 探针](../../tests/fixtures/m51-native-openssl) |
| 摘要、HMAC、AES-GCM、安全随机、密钥与签名 | [crypto 测试](../../sysroot/lib/scoop.crypto/tests)、[独立 OpenSSL 签验](../../sysroot/lib/scoop.tls/tests/interop_signatures.scoop) |
| X.509 属性、证书签名、显式链／时间／用途／身份验证 | [证书测试](../../sysroot/lib/scoop.crypto/tests/certificates.scoop) |
| 五种 JWS 算法、原始 signing input、公钥 JWK | [JWS 测试及外部向量](../../sysroot/lib/scoop.crypto/tests/jws.scoop) |
| 共享有限 BIO 引擎、TLS 1.2／1.3、同步流与互操作 | [TLS 测试](../../sysroot/lib/scoop.tls/tests) |
| M49 异步双向流、背压、取消、借用与确定性关闭 | [async TLS 测试](../../sysroot/lib/scoop.tls.asyncio/tests) |
| 三个生产 `.slib` 的公开泛型、资源辅助函数及 native 引用 | [crypto 消费者](../../sysroot/lib/scoop.crypto/consumer-tests)、[TLS 消费者](../../sysroot/lib/scoop.tls/consumer-tests)、[async TLS 消费者](../../sysroot/lib/scoop.tls.asyncio/consumer-tests) |

三个普通 Cone 分别提供 17／13／12 项库测试，外部消费者分别有 4／2／1 项。测试覆盖标准向量和独立工具、参数／格式／身份失败、TLS 1.2／1.3、短传输、有限 BIO 背压、双向并发、跨线程取消／恢复、已接受明文进度、close_notify、截断和显式资源收尾。网络仅使用动态 loopback 端口和自有测试 CA，密码与证书验证使用显式内存输入。主动编写的库生产源文件最长 108 行，上游 vendor 保持原样。

## 验证结果

| 环境与范围 | 结果 |
| --- | --- |
| Darwin crypto／TLS／async TLS | debug／release／moving GC 各 17／13／12 项通过 |
| Darwin 外部消费者 | debug／release／moving GC 各 4／2／1 项通过 |
| GNU crypto／TLS／async TLS 及消费者 | debug／release／moving GC 各 17／13／12 项、消费者各 4／2／1 项通过 |
| musl crypto／TLS／async TLS 及消费者 | debug／release／moving GC 各 17／13／12 项、消费者各 4／2／1 项通过 |
| OpenSSL native 接入 | 三目标各 2 项、4 个真实进程通过 |
| Linux Rust workspace | release 单元／集成 4,956 项和 doctest 2 项通过，0 失败／忽略／筛除 |
| Linux fixture 公共规则 | 50 项通过 |
| GNU 正式 CLI | 4,164 项通过、30 项目标不适用；4,707 个变体、16,461 个实际进程 |
| musl 相关正式 CLI | 149 项通过、11 项目标不适用；217 个变体、815 个实际进程 |

本机每批先执行相应格式化、lint 和必要 native 告警检查，再进行定向验证；最终只组合本期三个 Cone 与消费者。Darwin debug 保留已通过结果并补验新增失败路径，release／moving GC 执行完整库组合。Linux 通过 fmt／workspace clippy 后构建配套 release 工具，执行完整 Rust 与 GNU CLI；musl 另覆盖本期及 native、M49、GC／优化和短路规则，避免重复前端全量。

GNU 合并执行的 async TLS moving GC 用时约 630 秒，单独的大块传输用例约 30 秒通过。musl 的该模式按六个既有测试组分别运行，全部 12 项约 59 秒通过；各用例内的组合行为保持，减少同一进程累积带来的执行开销。

GNU CLI 首段以四个 worker 执行，保留 52 项通过和 30 项目标不适用结果后，将未完成项提高到八个 worker。Rust 验收结束后保留已通过的 748 项，再以十二个 worker 完成其余项。源码、原产物和缓存保持存在，续跑使用原公共输入与缓存及新的用例工作目录；最终按用例名称合并核对全部 4,194 项，无遗漏、重复、失败或中断。musl 的 160 项相关选择也全部结束，无失败或中断。

## Native 资源与互操作

三个目标均从同一份 OpenSSL 3.5.9 LTS 原始归档准备静态库及匹配配置头，使用正常汇编和默认 provider。musl 使用已有工具链包装器补充隔离 Linux UAPI 头，不使用 GNU 用户态头或系统 OpenSSL；生成资源位于各目标 `sysroot/native/<triple>/openssl`，准备与消费命令见使用说明。

实际运行的 musl crypto、TLS 和 async TLS 程序均为静态 ELF，未含动态段；GNU 程序的动态依赖不包含 libssl／libcrypto。目标资源足以完成库与消费者构建、链接和运行。

OpenSSL CLI 验证 Scoop 生成的 RSA PKCS#1／PSS、ECDSA DER、Ed25519 签名，并作为严格验证证书的 TLS 客户端；Python 标准库 SSL 作为独立服务端，验证 DNS／IP、DNS 有 SNI 而 IP 无 SNI、服务端顺序 ALPN 和双向关闭。Darwin 独立对端为 OpenSSL 4.0.3，Linux 为 3.5.5，生产始终链接 vendor 3.5.9。crypto 另验证 RFC 8032／8037、NIST、RFC 4231、OpenSSL 3.6.5 CLI 和 Python cryptography 46.0.7 固定向量。

普通生产构建的实际依赖图确认：crypto 仅有 core／std／json，同步 TLS 另加 crypto／platform；asyncio 只在独立异步 Cone 中进入依赖。生产构建和外部消费者沿普通 `.slib` 路径完成，不靠隐藏源文件、工具或产物验收。

## 失败闭合与独立修复

- 异步 TLS 集成暴露了 MIR 短路右侧的 unwrap／cast／bounds setup 提前执行。按既有语言规则将 setup 保留在右侧分支内，未改变 IR 或 runtime ABI。[独立 fixture](../../tests/fixtures/m51-short-circuit-checks)覆盖未进入右侧、嵌套短路、实际进入时的求值顺序与异常，另通过 MIR crate 117 项和既有短路回归。
- 首次 musl OpenSSL 准备缺 Linux UAPI 头，改用隔离 UAPI 工具链配置后实际构建、摘要、签名和 TLS 握手通过，没有关闭汇编或改写上游源码。
- 为遵守本次临时目录要求，[既有 native I/O 用例](../../tests/fixtures/m45-platform-native-io/probe.c)改用 TMPDIR，未配置时在用例工作目录创建文件；Darwin 定向 CLI、GNU 完整回归和 musl 相关回归中的该用例均通过。

## 工作目录与边界

两端日志、报告及临时工作目录均在 `~/.cache/scoop-m51`，TMPDIR 显式使用其中的 `scratch`。原始日志和逐次报告不进入仓库。开工时本机 target 约 73 GiB，清理旧 debug 与 validation 数据后降至约 1.8 GiB，最终保留当前复用的 release 工具后约 3.3 GiB；Linux target 约 1.6 GiB。验收全部结束后回收 GNU／musl 正式 CLI 的临时用例目录与缓存约 280 GiB，保留报告、suite 日志及库构建所需的工具和目标资源。用户原有 M47 两份文档修改未纳入本任务提交。

显式关闭由应用 `use`／`useAsync`／`try-finally` 承担；GC 不驱动 OpenSSL 析构或协议收尾。JWS 只验证签名，claims 由应用检查。HTTP、mTLS、QUIC／DTLS、动态 SNI、会话恢复 API、在线 JWKS／证书状态和其他未列入 M51 的能力仍按设计范围处理。
