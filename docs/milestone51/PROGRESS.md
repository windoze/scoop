# M51 实施进度

状态：已完成。开始及完成日期：2026-10-17。基线：`8dad7d42e`（M50 合并）。范围见 [设计](DESIGN.md)，最终验证结果见 [验收](ACCEPTANCE.md)。

## 分批状态

1. Native 接入：已完成三目标 OpenSSL 3.5.9 静态资源准备及真实摘要、签名、证书校验和 TLS 握手。
2. 密码与密钥：已完成摘要、MAC、AEAD、安全随机、SecretKey、RSA／P-256／Ed25519 公私钥和签验。
3. 证书与 JWS：X.509 解析、属性、签名与链验证、五种 JWS 算法和公钥 JWK 导入已实现。
4. 共享 TLS 引擎与同步流：已完成配置、上下文、有限 BIO 引擎及同步 ByteSource／ByteSink 适配。
5. M49 异步 TLS：共享引擎适配、双向协调、取消与确定性关闭已完成。
6. 三目标组合及外部消费者验收：Darwin／GNU／musl 库与消费者模式组合、Linux 全量 Rust 与 GNU 正式 CLI、musl 相关 CLI 均已通过。

## 已完成批次

- 目标条件 native include：复用现有目标谓词，按声明顺序选择生成头文件目录；预处理、选项与对象更新使用同一集合。manifest 解析记录升级为 v3。格式化与 release workspace clippy 通过；native manifest 9 项、正式 CLI 3 项／5 变体／9 进程通过，覆盖新条件路径、既有外部 include 和非法空路径诊断。首次 TOML 位置解码失败已定向修正并复测。

- OpenSSL vendor 与目标资源：未修改的 3.5.9 上游源码归档、许可证、普通准备脚本及两个正式 CLI fixture 已落地。Darwin、GNU、musl 各 2 项／4 进程通过；每项均执行真实程序。纯 crypto 覆盖 SHA-256 标准向量、Ed25519 签验与篡改，TLS 覆盖显式信任、名称验证、有限 BIO 握手及传输。musl 首轮缺 Linux UAPI 头，改用已有隔离 UAPI 包装器后通过，没有使用 GNU 用户态头文件或系统 OpenSSL。Python Ruff 与 C 警告检查通过。

- 内存密码第一批：SHA-256／384／512 增量与一次性摘要、HMAC、AES-128／256-GCM、安全随机及显式 SecretKey 生命周期。错误在 C 调用内保存，FFI 只用同步借用；认证失败不交付明文。C 格式化与严格告警检查通过；本机 5 项标准／组合测试、相同 5 项 moving GC、生产外部消费者 1 项通过。OpenSSL 准备命中直接返回，未重跑工具。

- 密钥与签名：PKCS#8／SPKI 的 PEM／DER、独立公钥提取、RSA PKCS#1／PSS、ECDSA DER 与 Ed25519；关闭后状态及 native 引用保留。PSS 固定 SHA-256／MGF1 SHA-256／32 字节 salt，不启用口令交互。C 格式化与严格告警检查通过；本机 4 项、相同 4 项 moving GC 和生产消费者 1 项通过，包含 RFC 8032 与 OpenSSL 3.6.5 CLI 固定向量、改变消息／签名、格式与算法错误。

- X.509：证书 PEM／DER、RFC 2253 名称、序列号、时间、DNS／IP SAN、公钥提取及证书签名检查；TrustStore 保留显式锚，按给定时间、用途、链和身份验证。纯 core 测试素材 Cone 提供固定 CA／中间／服务端／过期／错误用途证书。C 格式化与严格告警检查通过；本机 3 项、相同 3 项 moving GC 及外部消费者 1 项通过，覆盖源码内存输入、已关闭原锚后的引用保留、缺链／未知根／过期／用途／DNS／IP 错误。

- JWS／JWK：五种 Compact 算法、原始 signing input、ES256 DER／R‖S 转换，以及 RSA／P-256／Ed25519 公钥参数导入；复用 JSON 与严格无 padding Base64URL。C 格式化与严格告警检查通过；本机 5 项、相同 5 项 moving GC 和外部消费者 1 项通过，包含 RFC 8037、独立 Python cryptography 46.0.7 向量、篡改、算法／密钥角色、critical header、非法编码与私钥参数拒绝。

- 共享 TLS 引擎：不可变角色配置、TLS 1.2／1.3、显式 TrustStore／DNS／IP、ALPN、有限 BIO pair 和稳定明文借用。会话保留上下文及 ALPN 配置，错误在 SSL 调用内复制，未使用 socket BIO 或 runtime 私有接口。C 格式化、严格告警及本机 4 项、相同 4 项 moving GC、生产消费者 1 项通过。短片段与 16384 字节前缀覆盖 WANT_WRITE 重试；大块内容改用摘要断言后，该 moving 用例从约 29 秒降至约 0.7 秒，仍验证完整内容与游标进度。同步阻塞边界与异步背压并进的职责已明确。

- 同步 TLS：普通多接口泛型接管传输，真实 TCP 短传输、明文前缀、flush、close_notify、截断和显式关闭已闭合。状态／参数错误不接管或破坏现有连接；已接受明文在输出失败后仍提交游标，正文及关闭失败组合保留。新增 4 项普通测试、相同 4 项 moving GC、生产外部消费者 1 项通过。未消费明文在已缓冲及尚未读入 SSL 两种情况下均拒绝优雅关闭；强制关闭对端可能收到平台 connection reset，测试按底层实际合同断言。

- 异步集成发现并修复短路 lowering 缺陷：MIR 为右操作数生成的 unwrap／cast／bounds prelude 原先提前执行，导致合法的 `present(value) && value!!.field` 抛异常。现按既有语言规则将 setup 放在右侧条件内。格式化、workspace clippy、MIR crate 117 项通过；新增及既有短路正式 CLI 共 2 项／4 变体／12 进程通过，包含 debug／release／moving、未进入分支无副作用、实际进入后的求值顺序与异常。修复后的异步 TLS 双向与握手背压测试通过。

- 异步 TLS：普通 continuation／Job 操作记录串行 SSL、分别驱动每方向一个传输，遵守 WANT 重试；调用、取消监听及恢复不在 SSL 锁内。登记前检查、同方向互斥、已接受前缀、跨线程取消／第三方恢复、挂起期间借用、并发 close、已取消 finally、握手／read／flush／shutdown 取消及未消费数据均已覆盖。无逐步骤 Job、libuv 私有调用或 GC 析构。新增 11 项、相同 11 项 moving GC 和生产外部消费者 1 项通过，包括 TLS 1.2／1.3、握手输出背压时输入并进、50000 字节双向短传输及一个停滞握手不阻塞后续 accept。

- 独立互操作：新增 3 项测试。系统 OpenSSL CLI 验证库生成的 RSA PKCS#1／PSS、ECDSA DER 和 Ed25519 签名，并作为严格验证证书的 TLS 客户端；Python 标准库 SSL 作为独立服务端。TLS 1.2／1.3 双向数据、DNS／IP、DNS 有 SNI 而 IP 无 SNI、服务端顺序 ALPN 及双向 close_notify 均通过。本机独立对端使用 OpenSSL 4.0.3，生产仍使用 vendor 3.5.9；测试仅使用临时自有文件、动态 loopback 端口和明确就绪信号。

- 验收覆盖补齐：直接验证 TLS 过期证书、错误 IP 的原始验证 code，以及不足一个完整加密记录时的 EOF；新增写请求自身取消，确认已接受的 16384 字节和借用收尾。异步外部消费者增加 M49 名称解析后连接。Darwin 的 debug／release／moving GC 已通过 crypto 17 项、TLS 13 项、async TLS 12 项及外部消费者 4／2／1 项；debug 包含新增路径的定向补验。普通生产构建确认 crypto 仅有 core／std／json 依赖，同步 TLS 加入 crypto／platform，未触发新的 compiler 子调用。使用入口见 [USAGE.md](USAGE.md)。

- 全量验收准备发现既有 M45 native I/O 用例在 C 中写死临时目录，现改用 TMPDIR，未配置时使用用例工作目录。C 格式化、严格告警检查及 Darwin 正式 CLI 1 项／2 进程通过，GNU 完整和 musl 相关回归中的该用例也已通过。其他匹配仅为路径表示或缺省目录查询，不创建临时文件。

## 验证与工作目录

本机逐功能格式化、lint 和定向验证；最后在 nuc12 的 `~/repos/scoop` 完成 Linux 验证。日志、报告及工作目录使用两端 `~/.cache/scoop-m51`，临时目录显式设为其中的 `scratch`，不使用 `/tmp`。定期清理无用 target 产物，保留正在复用的 release 工具。M47 文档修改不属于本任务。
