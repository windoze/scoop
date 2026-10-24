# scoop.http.server

纯 Scoop 的应用层 HTTP server，复用 `scoop.http`、`scoop.http.asyncio`、TLS、异步文件和压缩库。路由、middleware 与资源寿命由普通函数和结构化任务表达，不使用反射或 DI 容器。

`RouterBuilder` 显式注册端点并构造不可变 `HttpApplication`。应用先创建 `AsyncTcpListener`，再用 `HttpServer.takeOwnership(listener, application, scope, options)` 交给 server；`shutdownGracefully` 排空，`close` 取消并等待实际清理。

| 能力 | 公共入口与行为 |
| --- | --- |
| 路由与请求管线 | 静态／参数／末尾通配路径、分组、HEAD／OPTIONS、`HttpContext`、`HttpMiddleware`、`HttpResult` |
| 传输与生命周期 | H1.0／H1.1、明文 H2 prior knowledge、TLS／ALPN、独立请求任务、容量、时限、GOAWAY、完成通知 |
| 提取 | 字段／Cookie、JSON／form、query DTO、`adapt`、静态 extractor |
| SSE | `sse`、逐事件 flush、可选有界心跳、`setDeadline` |
| Multipart | `body.multipart`、单次 part 流、限制、显式文件保存与外层 EOF 校验 |
| 压缩 | `responseCompression` 协商输出，`requestDecompression` 选择解码策略，`openRawStream` 显式读取原始正文 |
| 文件 | `file`／`staticFiles`、同句柄 metadata 与读取、条件请求、单 Range、多区间回退 |
| WebSocket | `webSocket`、显式 Origin／子协议、可选 permessage-deflate；HTTP 清理后启动独立会话，支持 WSS |

普通流式响应在请求 middleware 资源作用域内执行。WebSocket 的 `onSession` 在 HTTP 清理后执行，接收独立 `WebSocketContext`；需要的业务值应先复制，会话自行取得资源。`staticFiles` 沿普通 OS 路径与 symlink 语义工作，不提供文件系统隔离。

完整合同见 [HTTP server 规范](../../../docs/specs/SCOOP-HTTP-SERVER-SPEC.md)，总体设计及验证范围见 [设计目录](../../../docs/http-server/DESIGN.md)与[实施记录](../../../docs/http-server/PROGRESS.md)。服务端 mTLS／动态 SNI、HTTP/3 与 H2 WebSocket 不在本库当前范围内。

可运行的公开 API 示例保存在独立 [consumer-tests Cone](consumer-tests/Cone.toml)：

- [H1 typed query／JSON 与 gzip](consumer-tests/tests/application.scoop)
- [H2、SSE 与 gzip multipart](consumer-tests/tests/http2.scoop)
- [静态路径、范围和 304](consumer-tests/tests/files.scoop)
- [认证、Origin、子协议和 WebSocket 压缩会话](consumer-tests/tests/websocket.scoop)

先按仓库准备脚本说明准备目标 sysroot；consumer 使用已发布的目标 `.slib`，不会隐式重建系统库。验证应使用显式的工作目录、共享 cache 与 release profile，按当前变更选择 server 测试 filter 或上述小型 consumer Cone。所有创建临时文件的用例使用显式 `TMPDIR` 或当前目录。
