# M51 实施进度

状态：实施中。开始日期：2026-10-17。基线：`8dad7d42e`（M50 合并）。范围与验收要求见 [设计](DESIGN.md)。

## 分批状态

1. Native 接入：已完成三目标 OpenSSL 3.5.9 静态资源准备及真实摘要、签名、证书校验和 TLS 握手。
2. 密码与密钥：已完成摘要、MAC、AEAD、安全随机、SecretKey、RSA／P-256／Ed25519 公私钥和签验。
3. 证书与 JWS：X.509 解析、属性、签名与链验证、五种 JWS 算法和公钥 JWK 导入已实现。
4. 共享 TLS 引擎与同步流：配置、上下文、有限 BIO 引擎已完成；同步传输适配待实施。
5. M49 异步 TLS：待实施。
6. 三目标组合及外部消费者验收：待执行。

## 已完成批次

- 目标条件 native include：复用现有目标谓词，按声明顺序选择生成头文件目录；预处理、选项与对象更新使用同一集合。manifest 解析记录升级为 v3。格式化与 release workspace clippy 通过；native manifest 9 项、正式 CLI 3 项／5 变体／9 进程通过，覆盖新条件路径、既有外部 include 和非法空路径诊断。首次 TOML 位置解码失败已定向修正并复测。

- OpenSSL vendor 与目标资源：未修改的 3.5.9 上游源码归档、许可证、普通准备脚本及两个正式 CLI fixture 已落地。Darwin、GNU、musl 各 2 项／4 进程通过；每项均执行真实程序。纯 crypto 覆盖 SHA-256 标准向量、Ed25519 签验与篡改，TLS 覆盖显式信任、名称验证、有限 BIO 握手及传输。musl 首轮缺 Linux UAPI 头，改用已有隔离 UAPI 包装器后通过，没有使用 GNU 用户态头文件或系统 OpenSSL。Python Ruff 与 C 警告检查通过。

- 内存密码第一批：SHA-256／384／512 增量与一次性摘要、HMAC、AES-128／256-GCM、安全随机及显式 SecretKey 生命周期。错误在 C 调用内保存，FFI 只用同步借用；认证失败不交付明文。C 格式化与严格告警检查通过；本机 5 项标准／组合测试、相同 5 项 moving GC、生产外部消费者 1 项通过。OpenSSL 准备命中直接返回，未重跑工具。

- 密钥与签名：PKCS#8／SPKI 的 PEM／DER、独立公钥提取、RSA PKCS#1／PSS、ECDSA DER 与 Ed25519；关闭后状态及 native 引用保留。PSS 固定 SHA-256／MGF1 SHA-256／32 字节 salt，不启用口令交互。C 格式化与严格告警检查通过；本机 4 项、相同 4 项 moving GC 和生产消费者 1 项通过，包含 RFC 8032 与 OpenSSL 3.6.5 CLI 固定向量、改变消息／签名、格式与算法错误。

- X.509：证书 PEM／DER、RFC 2253 名称、序列号、时间、DNS／IP SAN、公钥提取及证书签名检查；TrustStore 保留显式锚，按给定时间、用途、链和身份验证。纯 core 测试素材 Cone 提供固定 CA／中间／服务端／过期／错误用途证书。C 格式化与严格告警检查通过；本机 3 项、相同 3 项 moving GC 及外部消费者 1 项通过，覆盖源码内存输入、已关闭原锚后的引用保留、缺链／未知根／过期／用途／DNS／IP 错误。

- JWS／JWK：五种 Compact 算法、原始 signing input、ES256 DER／R‖S 转换，以及 RSA／P-256／Ed25519 公钥参数导入；复用 JSON 与严格无 padding Base64URL。C 格式化与严格告警检查通过；本机 5 项、相同 5 项 moving GC 和外部消费者 1 项通过，包含 RFC 8037、独立 Python cryptography 46.0.7 向量、篡改、算法／密钥角色、critical header、非法编码与私钥参数拒绝。

- 共享 TLS 引擎：不可变角色配置、TLS 1.2／1.3、显式 TrustStore／DNS／IP、ALPN、有限 BIO pair 和稳定明文借用。会话保留上下文及 ALPN 配置，错误在 SSL 调用内复制，未使用 socket BIO 或 runtime 私有接口。C 格式化、严格告警及本机 4 项、相同 4 项 moving GC、生产消费者 1 项通过。短片段与 16384 字节前缀覆盖 WANT_WRITE 重试；大块内容改用摘要断言后，该 moving 用例从约 29 秒降至约 0.7 秒，仍验证完整内容与游标进度。同步阻塞边界与异步背压并进的职责已明确。

## 验证与工作目录

本机逐功能格式化、lint 和定向验证；最后在 nuc12 的 `~/repos/scoop` 执行 Linux 完整验证。日志、报告及工作目录使用两端 `~/.cache/scoop-m51`，临时目录显式设为其中的 `scratch`，不使用 `/tmp`。定期清理无用 target 产物，保留正在复用的 release 工具。M47 文档修改不属于本任务。
