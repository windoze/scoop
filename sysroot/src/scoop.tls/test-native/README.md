# TLS 平台信任测试辅助库

这个普通测试 Cone 提供隔离的平台信任锚、原生工作门控，以及由现有测试 CA 在内存中签发的短期证书。服务器模板保留 SAN 和用途扩展；有效证书从昨天起生效、七天后过期，expired 证书在昨天过期。

生产 TLS 继续校验当前时间及平台有效期策略。这些 helper 不修改机器的 keychain 或信任设置。固定 M51 证书有意覆盖多年并保持不变；macOS 即使接收显式信任锚也限制 TLS 叶证书有效期，因此平台正例使用 systemCertificatePem。

Windows 测试以内存证书 store 和 `hExclusiveRoot` 建立独立链引擎，沿生产 Crypt32／SSL 策略验证路径检查 DNS、IP、用途、时间和信任失败；不导入用户或机器的证书 store。原生工作门控使用 SRW lock 和 condition variable，继续占用实际 asyncio worker。

公开 Scoop 入口及其生命周期说明见 src/certificate.scoop。本 Cone 不属于发行内置库清单，也不进入 SYSROOT/docs 的生产库站点。
