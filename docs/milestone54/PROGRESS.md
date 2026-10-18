# M54 实施进度

状态：实施中。完整范围见[设计](DESIGN.md)，本页只记录实际实现与验证；Linux 与最终三目标验收尚未进行。

## 已完成的功能

| 功能 | 实现与边界 | 已验证范围 |
| --- | --- | --- |
| HTTP 协议摘要 | `scoop.crypto` 增加 SHA-1、MD5、SHA-512/256 的一次性与增量摘要；HMAC、签名及 JWS 算法集合保持原合同 | Darwin debug／release 与 moving GC：3 项新测试及 3 项既有摘要／MAC 测试；标准空输入与 abc 向量、WebSocket Accept、HTTP Digest 凭据、增量状态和 HMAC 参数边界 |
| HTTP/1 扩展方法 | 编码器与客户端响应上下文接受任意合法 token，HEAD／CONNECT 按精确大小写分类；服务端 llhttp 已知方法限制保留 | Darwin debug／release 与 moving GC：28 项 HTTP/1 测试，包括 4 项新增方法测试；合法 token 保真、非法 token 不改变状态、信息响应和混合方法关联 |
| 多证书 PEM | crypto 提供 `parseCertificatesPem` 与 `TrustStore.fromPem`；保留顺序、独立拥有证书，拒绝空包、坏片段与非证书内容，失败释放部分结果 | Darwin debug／release 与 moving GC：3 项 bundle 测试与 3 项既有证书测试；重复证书的独立关闭、显式信任链、临时 bundle 释放后的锚保活 |

正式 fixture `m54-protocol-digests` 在 Darwin 的 debug／release 两个变体通过，共 6 个进程；覆盖生产 `.slib` 跨 Cone 消费、moving GC 和构建缓存复用。该结果不代替尚未进行的 Linux 验证。

`m54-http1-methods` 通过真实 TCP 与独立 C 对端核对 CUSTOM、HEAD、head、connect 及 100／最终响应；debug／release、moving GC 与缓存复用通过。既有 `m53-http1-consumer`、`m53-http1-async-consumer` 同时回归通过，共 6 个变体、18 个进程。原生对端用例初次因消费后未重置测试缓冲区失败，修正用例后全部通过。

`m54-certificate-bundle` 的 debug／release 两个变体通过，共 6 个进程；从生产 crypto `.slib` 解析双证书、构造信任库、验证证书并拒绝坏尾部，覆盖 moving GC 与缓存复用。

## 后续工作

依次完成独立 IDNA／URL／字符集／压缩库、HTTP 消息与协议补充、TLS 与 asyncio 缺口，再完成统一 client、策略、存储与长连接组合。每个功能独立验证和提交，最终按设计完成三个目标的运行验收。

## 验证安排

复用 M53 已完成的工具与目标资源，改动的库通过正常准备／构建入口更新。先运行本机相关测试，待功能组合稳定后在 Linux 完整验证；普通库改动不重复运行无关编译器全量测试。原始日志、fixture 工作区与本期构建缓存保存在仓库外的 `scoop-work/m54`。

已清理本机旧 `target/debug/incremental`，释放约 8 GB；保留当前仍需复用的编译产物。后续按实际磁盘占用继续清理不再使用的 target 内容。
