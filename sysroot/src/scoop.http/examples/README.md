# scoop.http 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。这是内存协议库，两个示例都不建立网络连接。

| 文件 | 内容 |
| --- | --- |
| [http1.scoop](http1.scoop) | HTTP/1 请求解码事件、正文与结束事件、204 响应编码 |
| [sse.scoop](sse.scoop) | 每次输入至多 3 字节的增量 SSE 解析、多行 data 和事件 ID |

```sh
scoop run --release http1.scoop
scoop run --release sse.scoop
```

HTTP 示例输出 `POST`、`Scoop`、`Request complete` 和编码后的 HTTP/1.1 204 响应。SSE 示例输出 `update #7`，然后输出 `first line` 和 `second line`。解码循环每次供给输入后消费待取事件，避免事件背压阻塞后续输入。
