# scoop.tls 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。示例使用本机回环 TCP，不需要公网服务。

| 文件 | 内容 |
| --- | --- |
| [server.scoop](server.scoop) | PEM 证书/私钥、同步 TLS 握手、发送数据和关闭握手 |
| [client.scoop](client.scoop) | 显式 CA、localhost 身份校验、明文读取和关闭握手 |
| [localhost.cnf](localhost.cnf) | 用于临时 localhost 自签名证书的 OpenSSL 配置 |

先用本机 OpenSSL 生成一天有效的示例证书，输出位于已忽略的 `target/`：

```sh
mkdir -p target
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -config localhost.cnf -keyout target/key.pem -out target/cert.pem
scoop run --release server.scoop -- target/cert.pem target/key.pem
```

看到 `Listening on` 后，在第二个终端进入同一目录：

```sh
scoop run --release client.scoop -- target/cert.pem
```

客户端输出 `Hello over TLS!`，双方完成 TLS 关闭握手后退出。默认端口为 8443；可以分别在服务端的两个文件参数之后、客户端的 CA 参数之后追加相同端口。

生成的私钥只用于本机示例，不加入版本控制。客户端将生成的证书作为显式信任根，同时检查 `localhost`；不关闭证书校验。证书过期后重新执行生成命令。一次运行只接受一个客户端；可与 [异步 TLS 示例](../../scoop.tls.asyncio/examples/README.md) 交叉配对。
