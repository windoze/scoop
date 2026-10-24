# scoop.tls.asyncio 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [server.scoop](server.scoop) | 事件循环上的 TLS 握手、NativeByteBuffer 写入和异步关闭 |
| [client.scoop](client.scoop) | 异步 CA 文件加载、localhost 校验、受限读取和关闭握手 |

使用与同步示例相同的 localhost 配置生成临时证书：

```sh
mkdir -p target
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
  -config ../../scoop.tls/examples/localhost.cnf \
  -keyout target/key.pem -out target/cert.pem
scoop run --release server.scoop -- target/cert.pem target/key.pem
```

看到 `Listening on` 后，在第二个终端进入同一目录：

```sh
scoop run --release client.scoop -- target/cert.pem
```

客户端输出 `Hello over async TLS!`，一次连接后双方退出。默认端口为 8444；在服务端两个文件参数之后、客户端 CA 参数之后追加相同端口可更改。证书过期后重新生成；私钥仅存放于已忽略的 `target/`。

服务端在进入事件循环前加载证书和私钥；客户端用 `openTlsClientContext` 异步读取 CA。TLS stream 接管 TCP transport，`useAsync` 处理异步清理；`shutdownGracefully` 完成协议关闭握手。
