# scoop.http.asyncio 示例

先完成[运行准备](../../../README.md#运行准备)。两个终端都进入本目录，先启动服务端：

```sh
scoop run --release server.scoop
```

看到 `Listening on` 后，在第二个终端运行：

```sh
scoop run --release client.scoop
```

| 文件 | 内容 |
| --- | --- |
| [server.scoop](server.scoop) | TCP 接管、请求正文消费、显式 Content-Length 与响应 writer |
| [client.scoop](client.scoop) | 请求头构造、空 body 的 finish、响应头与受限正文读取 |

客户端输出 `200` 和 `Hello, HTTP/1!`。服务端只处理一次连接，然后双方退出。默认监听 `127.0.0.1:8081`；可在两条命令后同时追加 `-- 9081` 使用其他端口。

此处显式组合 TCP 与 HTTP/1，没有 TLS。`takeOwnershipAsClient/Server` 将 transport 交给协议连接，`useAsync` 负责关闭；实际应用如果需要连接池、重试与自动协议选择，可参考 [HTTP client 示例](../../scoop.http.client/examples/README.md)。
