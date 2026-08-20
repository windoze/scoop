# Mbed TLS SHA-256

这里保留 [Mbed TLS 3.6.6](https://github.com/Mbed-TLS/mbedtls/releases/tag/mbedtls-3.6.6)
中 SHA-256 所需的上游文件，采用其 Apache-2.0 许可选项；完整上游许可证见 [LICENSE](LICENSE)。
上游 `library/` 和 `include/` 文件保持原样，不对第三方文件应用 Scoop 的格式化或拆文件规则。

只编译 `library/sha256.c`、`library/platform_util.c`。
`scoop_sha256_config.h` 仅启用 SHA-256，关闭其他 crypto/TLS/PSA 能力，并将编入的符号放到
`scoop_mbedtls_` 前缀下，避免与用户 native library 的 Mbed TLS 符号冲突。
部分配置 headers 虽然名称包含 SSL/X509，但属于上游 `build_info.h` 的必需 include，
没有启用或编译对应库。

编译时设置 `MBEDTLS_CONFIG_FILE="scoop_sha256_config.h"`，将本目录与 `include/`
加入 header search path。Scoop adapter 在 `runtime/src/platform/image/portable_sha256.c`，
仅服务原有 runtime canonical fingerprint；没有新的 hash 算法、产物格式或授权用途。
所有源文件、headers 和配置由普通 runtime 构建输入追踪。

更新时从上述上游 release tag 取同名文件，重新运行 SHA-256 向量、canonical stackmap
指纹与 glibc/musl 构建测试。[上游最小模块使用说明](https://mbed-tls.readthedocs.io/en/latest/kb/how-to/using-loose-modules-without-the-full-library/)
说明了按模块裁剪配置的方法。普通 Scoop 构建不下载这些文件。
