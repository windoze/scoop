# 固定证书测试资料

固定 PEM 输入由 OpenSSL 3.6.5 CLI 生成。root 有效期为 2024-01-01 至 2036-01-01，intermediate 至 2035-01-01，server 至 2034-01-01。server 的 SAN 包含 localhost、127.0.0.1 和 ::1；expired 叶证书于 2025-01-01 过期，client 叶证书只允许 clientAuth。日期、用途和扩展都是测试输入的一部分。

包含的私钥是公开测试资料，不用于生产身份。这个普通 Cone 只依赖 core，由 crypto 和 TLS 消费者通过 test-dependencies 使用；每个 PEM 入口的用途说明附在源码声明上。本 Cone 不属于发行内置库清单，也不进入 SYSROOT/docs 的生产库站点。
