# Scoop 密码库规范

版本：M51 已实施基线，含第 8 节的 M54 补充；协议摘要已实现，证书 bundle 待实施。本文规定普通 Cone `scoop:scoop.crypto:0.1.0` 的合同；既有实现与实际验收分别记录于 [M51 进度](../milestone51/PROGRESS.md)和验收文档，新增范围与实际进度见 [M54 设计](../milestone54/DESIGN.md)和[进度](../milestone54/PROGRESS.md)。规范不代表全部功能已验收。

## 1. 分层与后端

库显式依赖 core、std、json，提供内存中的密码、密钥、证书和 JWS 操作，不依赖 platform、asyncio、libssl 或 libuv。文件、网络、系统 CA、在线 JWKS 和 claims 策略由调用者负责。安全随机与密钥生成使用系统熵。

三个目标统一使用 OpenSSL 3.5.9 LTS，Apache-2.0 许可证。上游完整源码归档位于 `sysroot/vendor/openssl/`，普通准备脚本 `scripts/build_openssl.py --target <triple>` 在仓库外构建；选项和来源见 vendor README。使用上游默认 provider、正常汇编与线程支持，关闭动态模块、engine、自动配置加载及共享库。不在运行时搜索或替换后端，不调用进程级 OPENSSL_cleanup。

生成的头文件与 `libcrypto.a`、`libssl.a` 位于 `sysroot/lib/<triple>/openssl/{include,lib}`。manifest 使用语言规范 12.2.1 的条件 include 选择目标头文件，crypto 声明 static `crypto`，TLS 声明 static `ssl` 并使用传递的 crypto 要求；最终命令使用 `--library-path <sysroot>/lib/<triple>/openssl/lib`。发行包必须包含这组目标资源，musl 不使用宿主 GNU 库或要求发行版 musl OpenSSL 包。源代码准备与应用构建分离，未准备头文件时普通 include 诊断报告目标路径缺失，准备步骤见 [M51 使用说明](../milestone51/USAGE.md)；不自动下载或退回系统头文件。

## 2. 数据、错误与生命周期

输入采用 `List<Byte>` 并按需要建立独立数组快照；输出采用独立 `Array<Byte>`。字符串为 PEM、ASCII 协议字段或 UTF-8 文本。数据仅在同步 core withDataPointer／withUtf8Bytes 块中借用，C 不保存 managed 引用；耗时操作仍是同步 API，不创建任务或线程池。

`CryptoException` 保存 operation、OpenSSL 原始 code 与已复制的 detail。后端返回正常验证不匹配时返回 Boolean false 或对应验证结果，非法参数、格式、状态及运行错误各按实际原因报错。错误队列在同一 C 调用中提取并复制，不在返回或切换线程后重读。普通 C ABI 使用 NativeSafe，不标记 GCLeaf。

Digest、SecretKey、PrivateKey、PublicKey、Certificate、TrustStore 均为显式 Closeable；提供幂等 close，默认 dispose 桥接到 close，关闭后拒绝使用。使用与关闭同一对象须由调用者串行化；上下文 finish 后不能再次 update／finish。持久 native 引用由接收对象保留，释放原 wrapper 不使引用失效。析构不放进 GC release hook，正确性由 use／try-finally 及显式关闭承担。

## 3. 摘要、MAC、AEAD 与随机

`scoop.crypto` 提供 `DigestAlgorithm { Sha256, Sha384, Sha512, Sha1, Md5, Sha512_256 }`；一次性 `digest(algorithm, data)` 与 `Digest(algorithm)` 上连续 update 后调用 finish 的结果相同，允许空输入。update 返回 Unit，finish 返回字节数组并结束上下文的计算状态。后三种为 M54 按第 8 节增加的协议互操作算法。

SecretKey 从字节建立自有 native 副本，支持显式导出及关闭时清除自有存储。`hmac(algorithm, key, data)` 与 `verifyHmac(algorithm, key, data, expected)` 支持 Sha256、Sha384、Sha512；校验长度一致时使用 CRYPTO_memcmp，不使用字符串或提前退出的逐字节比较。

`AeadAlgorithm { Aes128Gcm, Aes256Gcm }` 分别要求 16／32 字节 key；nonce 固定 12 字节，tag 固定 16 字节，AAD 和数据可空。`encrypt` 返回 `AeadCiphertext(ciphertext, tag)`；`decryptOrNone` 成功返回明文，认证不匹配返回 None，临时明文在失败时清除且不交付。调用者确保同一密钥下 nonce 不重复，库不建立 nonce 分配服务。

`secureRandomBytes(count)` 接受非负 Long，使用 RAND_bytes_ex 填满结果；零长度合法，负数为参数错误，分配或熵失败保留错误。

## 4. 密钥与签名

`KeyAlgorithm { Rsa, P256, Ed25519, Other }`。Other 表示证书等输入中的其他正常后端公钥类型，不提供其通用生成／签名操作。`PrivateKey.generateRsa(bits: Int = 2048)` 要求位数至少 2048 且为 256 的倍数；`generateP256()` 与 `generateEd25519()` 各生成对应类型，无多余位数参数。三个工厂均返回独立拥有的 PrivateKey；没有接收 KeyAlgorithm 的通用生成入口。PEM／DER 私钥使用未加密 PKCS#8，公钥使用 SubjectPublicKeyInfo；导入拒绝口令交互与尾随非空数据。RSA、P-256、Ed25519 公私钥分别表示，PrivateKey.publicKey 返回独立拥有的公钥对象。native public 导出不包含私钥成分。

PrivateKey 与 PublicKey 通过 fromPem(String)／fromDer(List<Byte>) 导入、toPem()／toDer() 导出；algorithm 属性返回上述分类。sign(algorithm, privateKey, data) 与 verify(algorithm, publicKey, data, signature) 均接收原始消息。格式非法为 CryptoException；不匹配算法及不合法 RSA 位数为 IllegalArgumentException。

`SignatureAlgorithm { RsaPkcs1Sha256, RsaPssSha256, EcdsaP256Sha256Der, Ed25519 }` 显式要求匹配密钥。RsaPkcs1Sha256 为 PKCS#1 v1.5／SHA-256；RsaPssSha256 为 PSS／SHA-256、MGF1 SHA-256、32 字节 salt；EcdsaP256Sha256Der 为 ECDSA P-256／SHA-256，签名使用 ASN.1 DER；Ed25519 按纯 Ed25519 对原始消息签名，不额外预哈希。普通方案名描述算法和签名表示，JWS 的 RS256／PS256／ES256 协议代码只出现在第 6 节的 JwsAlgorithm。验签不匹配返回 false；密钥类型不匹配抛参数错误。

## 5. X.509

`scoop.crypto.x509` 提供 Certificate 的内存 PEM／DER 解析、导出、subject／issuer、序列号、有效期、DNS／IP SAN 和独立 PublicKey；证书算法按 OpenSSL 正常支持范围处理。

Certificate 使用 fromPem／fromDer 和 toPem／toDer；单次导入恰好一张证书，拒绝尾随非空数据。subject／issuer 使用 RFC 2253 名称文本，serialNumber 使用十六进制文本，notBefore／notAfter 为 Instant。subjectAlternativeNames() 返回 `SubjectAlternativeName { Dns(String), Ip(Array<Byte>) }` 数组，其他 SAN 类别不投影到该数组；publicKey() 返回独立拥有的公钥引用。

`TrustStore.fromCertificates(anchors)` 保留锚的 native 引用，创建后不再修改。`verify(certificate, at, purpose, intermediates, identity)` 要求明确的时间和 `CertificatePurpose { ServerAuth, ClientAuth, Any }`，中间证书默认空数组，identity 默认 None；返回 `Result<Unit, CertificateValidationFailure>`，成功为 Success(Unit)，验证失败为 Failure(error)。失败数据含 code、depth、detail。DNS 身份匹配禁止部分标签通配符，其他链与名称规则由 OpenSSL 执行。

Certificate.verifySignature(issuerKey) 只检查证书签名。TrustStore 从显式锚建立，verify 接收叶证书、中间证书、明确 Instant、用途与可选 `PeerIdentity { Dns(String), Ip(Array<Byte>) }`。DNS 必须非空 ASCII／A-label 且无 NUL，IP 必须为 4／16 字节。结果复用 scoop.Result 的 Success(Unit) 和 Failure(error)，失败保存验证 code、depth 及原因。上游执行签名、时间、链约束与用途检查；最后提交的证书不隐式成为根，不读取系统信任、不执行 AIA／OCSP／CRL 网络获取。

## 6. JWS

`scoop.crypto.jws` 只提供使用普通编码 payload 的 Compact Serialization。支持 HS256／RS256／PS256／ES256／EdDSA；调用者显式给出允许的单一算法和匹配的签名／验证密钥，header alg 必须相同，拒绝 none。

复用 std Base64.UrlSafe 的 Absent padding 与普通 scoop.json codec。signCompact 接收 protected header 原文和 payload；verifyCompactOrNone 验证原 token 两个编码段组成的 signing input，不重新序列化 header。验证成功返回原 header、payload 与可选 kid，签名不匹配返回 None；格式、编码、算法／密钥错误抛异常。拒绝不支持的 crit 和 b64=false。结果仅表示签名验证，不检查 exp／nbf／iss／aud 或授权。

ES256 在 JWS 边界转换为固定 64 字节 R || S，普通签名 API 仍使用 DER。PS256 保持第 4 节参数。公钥 JWK 仅支持 RSA n/e、EC P-256 x/y、OKP Ed25519 x，进行正常公钥参数检查；拒绝私钥字段，kid 只作为调用者的选择数据，不按 token 地址取密钥。

公开入口为 `signCompact(protectedHeader, payload, algorithm, key)` 与 `verifyCompactOrNone(token, algorithm, key)`。`JwsAlgorithm { HS256, RS256, PS256, ES256, EdDSA }`，签名密钥为 `JwsSigningKey { Mac(SecretKey), Signature(PrivateKey) }`，验证密钥为 `JwsVerificationKey { Mac(SecretKey), Signature(PublicKey) }`。成功值 `VerifiedJws(protectedHeader, payload, keyId)` 保留原 header 文本、独立 payload 数组和可选 kid。HS256 密钥至少 32 字节，其他密钥匹配第 4 节算法。JSON 必须是对象，alg 必须为匹配的字符串；kid 若出现须为字符串。任何 crit 字段均拒绝；b64 若出现须为 true。Compact 必须恰好三段，所有段使用无 padding、规范末尾比特的 Base64URL，允许空 payload。格式、编码和 header 错误为 JwsException，密钥角色或类型错误为 IllegalArgumentException，后端错误保持 CryptoException。

`importJwk(json)` 返回独立 PublicKey；RSA n/e 为非空、无多余前导零的 Base64urlUInt，P-256 x/y 和 Ed25519 x 各为 32 字节。导入只消费公钥参数，不以可选 alg／use／key_ops 元数据选择算法或决定应用策略；调用者仍必须在每次验证中给出算法。拒绝 d、p、q、dp、dq、qi、oth、k 私密参数和不支持的 kty／crv。JSON 解析与重复字段规则复用 scoop.json，不另设解析器。

## 7. 跨 Cone native 边界

必要的 CLayout 返回类型、已复制错误和 Unsafe 对象引用入口集中在 `scoop.crypto.native`，TLS 直接使用这些实际类型与 native 引用。Unsafe 使用者在借用有效期内调用，跨对象保留由 OpenSSL 引用计数承担；不通过导出再解析 PEM 传递，也不重声明同形 nominal 类型。该 package 不导出全部 OpenSSL API，不改变 runtime ABI。

## 8. M54 摘要与证书输入补充

DigestAlgorithm 增加 Sha1、Md5、Sha512_256，分别用于 RFC 6455 握手和 RFC 7616 Digest 认证。复用 OpenSSL 的实际算法，Sha512_256 是 SHA-512/256，不是将 SHA-512 结果截断。一次性与增量摘要均支持这些值，输出分别为 20、16、32 字节；不改变原有签名／JWS 默认算法集合。HMAC 的本期算法集合保持第 3 节规定，对不支持的算法给出参数错误。

新增 `parseCertificatesPem(text): Array<Certificate>`，解析包含多张证书的 CA／身份链 bundle。每个结果由调用者拥有，输入损坏、非法中间段或非证书尾随数据时报 CryptoException，并关闭已经创建但未交付的对象；空集合显式报错。提供 `TrustStore.fromPem(text)` 便利工厂，由 crypto 完成解析与 native 引用收尾，不让 web client 拆 PEM。

单证书 fromPem／fromDer 继续拒绝额外证书和尾随数据。bundle 解析不查询文件系统、不选择系统信任、不自动把发送链的最后一张证书提升为信任根；路径和系统信任入口在 TLS 所属层。

新增能力以标准摘要向量、独立 OpenSSL 输出、多证书与坏 bundle、部分失败清理及跨 Cone 消费验收。WebSocket、Digest auth-int 和 mTLS 组合测试调用这些公共入口，不携带客户端私有密码实现。
