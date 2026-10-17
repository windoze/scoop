# M51 使用与目标资源

三个库均使用普通 Cone 依赖：`scoop:scoop.crypto:0.1.0`、`scoop:scoop.tls:0.1.0`、`scoop:scoop.tls.asyncio:0.1.0`。公开合同见[密码库规范](../specs/SCOOP-CRYPTO-SPEC.md)和 [TLS 规范](../specs/SCOOP-TLS-SPEC.md)。

## 准备 OpenSSL

从仓库根目录运行下面的命令。Darwin 使用 `aarch64-apple-darwin`；Linux 使用 `x86_64-unknown-linux-gnu` 或 `x86_64-unknown-linux-musl`。Linux 两个目标分别准备，不能共用生成头文件或静态库。

```sh
mkdir -p "$HOME/.cache/scoop/scratch"
export TMPDIR="$HOME/.cache/scoop/scratch"
python3 scripts/build_openssl.py --target aarch64-apple-darwin
```

脚本使用仓库中固定的 OpenSSL 3.5.9 完整源码，在仓库外配置与构建。`--build-dir`、`--prefix`、`--cc`、`--jobs` 可选择实际目录和工具；输入未更新且结果存在时直接复用。更换编译器环境后用 `--rebuild`。musl 工具链需要 musl libc 开发头和隔离的 Linux UAPI 头，配置要求见 [vendor README](../../sysroot/lib/scoop.crypto/vendor/openssl/README.md)。

生成资源位于 `sysroot/native/<triple>/openssl/include` 和 `lib`。目标发行包须一起提供这两个目录、普通 sysroot Cone／生产 `.slib` 和相同的路径配置；其中 `libcrypto.a` 与 `libssl.a` 是目标资源，不嵌入每个 `.slib`。发行包消费者直接使用随包资源，无需重新编译 OpenSSL，musl 也无需发行版的 OpenSSL 包。Linux 仍按既有工具链要求提供该目标的 unwind 与 libc 资源。

## 构建与验证

Cone manifest 按目标选择头文件；最终构建显式选择该目标的库目录，例如：

```sh
target/release/scoop test sysroot/lib/scoop.crypto \
  --sysroot sysroot --target aarch64-apple-darwin \
  --library-path sysroot/native/aarch64-apple-darwin/openssl/lib
```

把测试根换成 `sysroot/lib/scoop.tls`、`sysroot/lib/scoop.tls.asyncio` 或对应的 `consumer-tests` 可运行其他入口；`--profile release` 选择 release，`SCOOP_GC_STRESS_MOVE=1` 启用 moving GC。Linux 命令另传同目标 unwind 的 `--library-path`，musl 必要时传 `--cc` 包装器。TLS 的独立对端测试需要 PATH 中的 `openssl` CLI 和带 SSL 模块的 `python3`，它们只属于测试环境，生产链接始终使用 vendor 目标库。

生产分层为：crypto 使用 core／std／json 和 libcrypto；同步 TLS 增加 platform 和 libssl；异步 TLS 再增加 asyncio。各 Cone 的测试依赖不进入生产 `.slib`。

## API 示例入口

这些外部消费者通过生产 `.slib` 使用公开 API，也可作为可运行的最小示例：

- [摘要、HMAC 与 AES-GCM](../../sysroot/lib/scoop.crypto/consumer-tests/tests/memory.scoop)、[密钥与签名](../../sysroot/lib/scoop.crypto/consumer-tests/tests/keys.scoop)、[证书链](../../sysroot/lib/scoop.crypto/consumer-tests/tests/certificates.scoop)、[JWS／JWK](../../sysroot/lib/scoop.crypto/consumer-tests/tests/jws.scoop)。
- [同步 TLS 流](../../sysroot/lib/scoop.tls/consumer-tests/tests/stream.scoop)：普通 TCP、显式信任和身份、泛型资源管理与双向关闭。
- [异步 TLS 流](../../sysroot/lib/scoop.tls.asyncio/consumer-tests/tests/streams.scoop)：M49 名称解析、连接、accept、Job、`useAsync`、稳定字节缓冲与双向关闭。

密码对象通过 `use`／`close` 显式释放。TLS 包装在握手登记后负责底层传输的关闭，应用通过包装流收发；客户端必须提供 TrustStore 和 DNS／IP 身份。`shutdownGracefully` 等待双方 close_notify，普通 `close` 只完成本地收尾。JWS 验签只验证签名，调用方另行检查应用需要的 claims。
