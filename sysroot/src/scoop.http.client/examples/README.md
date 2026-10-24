# scoop.http.client 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [get.scoop](get.scoop) | client 作用域、GET、成功状态检查、受限文本读取 |
| [post_json.scoop](post_json.scoop) | 自动生成 codec 的请求/响应 DTO、JSON POST |

先在另一个终端启动 [HTTP server 的 JSON 示例](../../scoop.http.server/examples/json.scoop)：

```sh
scoop run --release ../../scoop.http.server/examples/json.scoop
```

再运行：

```sh
scoop run --release get.scoop -- 'http://127.0.0.1:8080/greet?name=Scoop'
scoop run --release post_json.scoop
```

GET 输出 `200` 和 `{"message":"Hello, Scoop!"}`；POST 输出 `Hello, Scoop!`。完成后在服务端终端按 Ctrl+C。

两个程序均接受第一个用户参数作为目标 URL。GET 的默认 URL 是 `http://127.0.0.1:8080/`，可与 [hello.scoop](../../scoop.http.server/examples/hello.scoop) 配对；POST 默认 `/greet`。改用 HTTPS URL 时，客户端使用系统信任。响应 body 只有一个消费位置，示例在 `withResponse` 内读完并关闭。
