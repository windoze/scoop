# scoop.asyncio 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [tasks.scoop](tasks.scoop) | 两个并发定时任务、await、取消与 finally 清理 |
| [tcp_echo.scoop](tcp_echo.scoop) | 同一事件循环内的 TCP listener/client、缓冲读写和半关闭 |

```sh
scoop run --release tasks.scoop
scoop run --release tcp_echo.scoop
```

任务示例输出 `42` 和 `Cancelled task cleaned up`。TCP 示例绑定 `127.0.0.1:0`，由系统分配空闲端口，输出 `Hello over TCP!` 后结束；不需要外部服务器。`useAsync` 关闭连接和 listener，`NativeByteBuffer.use suspend` 在挂起期间保持缓冲有效。
