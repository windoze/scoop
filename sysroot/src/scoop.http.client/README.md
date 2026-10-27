# scoop.http.client

统一的 HTTP/1.x／HTTP/2 客户端。请求、响应、连接池、认证、缓存、下载、SSE 和 HTTP/1.1 WebSocket 由纯 Scoop 实现；协议、TLS、I/O、URL、字符集和压缩使用各自的公共库。

## 开始使用

下面的程序使用事件循环、URL 和 client，在应用 `Cone.toml` 的依赖中声明这些库及基础库：

```toml
[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.std" = "0.1.0"
"scoop:scoop.asyncio" = "0.1.0"
"scoop:scoop.url" = "0.1.0"
"scoop:scoop.http.client" = "0.1.0"
```

```scoop
import scoop.asyncio.*
import scoop.http.client.*
import scoop.url.Url

fun main() {
    val loop: EventLoop = PlatformEventLoop()
    loop.run suspend { _ ->
        withHttpClient suspend { client ->
            val request = HttpRequest.get(Url.parse("https://example.com/"))
            client.withResponse(request) suspend { response ->
                response.requireSuccess()
                println(response.body.text(1_048_576L))
            }
        }
    }
}
```

默认使用系统信任。HTTPS 的 `Auto` 策略通过 ALPN 选择 HTTP/2 或 HTTP/1.1；明文默认 HTTP/1.1，需要 HTTP/2 prior knowledge 或 h2c upgrade 时显式设置 `cleartext`。可用 `HttpVersionPolicy.Http1Only`／`Http2Only` 限定版本。请求方法接收合法 HTTP token，版本特有限制在发送前报告。

`withHttpClient` 创建并关闭一个有明确作用域的 client。在长期运行的应用中复用同一个 client；也可使用 `HttpClient.open(scope, config)`，并在所属 `CoroutineScope` 的正文退出前关闭它。`close` 会取消并等待活动调用、连接与必要清理。

## 响应和 body

`execute` 在最终响应头到达后返回 `HttpResponse`；非 2xx 状态仍是可消费的响应，调用 `requireSuccess` 才将它转换为 `HttpStatusException`。`withResponse` 在正文完成或抛错后关闭响应，不应把其中的响应保存到作用域之外继续使用。直接调用 `execute` 时，由调用者通过 `useAsync` 或 `finally` 关闭结果。

`body.bytes(maxBytes)`、`body.text(maxBytes)` 和 JSON 便利操作都有明确的缓冲上限；较大正文使用 `body.readInto` 或 `copyTo` 增量消费。正文只有一个消费位置，不能先读完文本再从头读取字节，也不能并发启动两个读取操作。

自动内容解码支持 gzip、zlib 包装的 deflate、Brotli 和 zstd，并按相反顺序处理多层编码。`HttpContentDecoding.Raw` 保留编码表示。原始响应字段和 `wireContentLength` 始终保留，不能把编码后的 Content-Length 当作解码后的长度。`text` 使用明确字符集或 Content-Type 的字符集规则；正文完成后才能取得完整 trailer。

`RequestBody` 提供字节、文本、JSON、表单、multipart、文件及 source／producer 组合。重试和重定向同时检查方法语义与 body 的可重放性；需要重放时提供可重新打开的来源，不会私自把一次性来源全部缓冲或落盘。HTTP/2 支持显式 duplex，HTTP/1 不接受 duplex 请求。

## 配置与组合

| 入口 | 能力与使用边界 |
| --- | --- |
| `HttpClientConfig.pool` | HTTP/1 复用、HTTP/2 多路复用、容量等待、空闲回收与 GOAWAY 排空 |
| `timeouts`／请求级 `HttpCallOptions.timeouts` | total、排池、DNS、TCP、TLS、响应头和读写 idle；total 持续覆盖 body 与清理 |
| `redirects`／`retries` | 有限跳转与重试、方法重写、跨 origin 凭据剥离、Retry-After 和可取消退避 |
| `authentication` | Basic、Bearer、Digest 和 suspend 凭据／令牌刷新；代理认证使用独立的身份状态 |
| `tls` | 系统或显式信任、客户端证书身份、TLS 1.2／1.3；TLS 能力由所属库提供 |
| `proxy` | HTTP／HTTPS 转发、CONNECT、SOCKS5；环境代理需要显式选择，默认直连 |
| `cookieJar`／`cache` | 可选 suspend Cookie 存储、私有内存／磁盘响应缓存；默认不配置 |
| `download`／`resumeDownload` | 明确目标 Path、异步文件、完整后原子提交；续传显式提供部分文件和验证元数据 |
| `withSse`／`openSse` | HTTP/1／HTTP/2 事件源、Last-Event-ID、retry 和显式重连 |
| `withWebSocket`／`openWebSocket` | HTTP/1.1 WS／WSS、控制帧、分片、permessage-deflate 及可消费的握手拒绝响应 |
| `interceptors`／`onAttempt`／`observer` | 有序 suspend 回调、尝试快照、实际阶段时间、进度与脱敏摘要 |

直接使用 HTTP、TLS、文件或其他库的类型时，在应用 manifest 中声明相应直接依赖。配置对象、参数及错误语义见 [HTTP client 规范](../../../docs/specs/SCOOP-HTTP-CLIENT-SPEC.md)。

共享 client 的单次调用可以通过 `HttpCallOptions(observer = Some(observer))` 取得自己的事件；它与 client observer 共存，覆盖该调用的所有尝试。`event.metrics.requestHeadersAtUtc` 是实际提交请求头时采集的 `Instant`；`finalResponseHeadersAtUtc` 是网络最终响应头的时间，不受 103 等信息响应或回调延迟影响。缓存命中没有网络发送／接收时间。用对应的 `TimeMark` 求耗时，不用可能回拨的 UTC 时钟相减。

`response.metrics` 在开启观察时提供不可变快照，关闭后仍可读取最终快照。失败在响应交付之前发生时，可从 observer 的 Completed 事件取得本次已发生的阶段；未发生的时间为 None。请求级观察不改变连接级首字节采样配置。具体组合见 [请求观测测试](tests/observability/request.scoop)。

## 取消与资源寿命

网络、池、文件、退避及用户回调的等待可以挂起。取消会等待实际 I/O、借用、来源子任务和 `finally` 完成；HTTP/2 的单请求失败只终止该流，其他调用继续使用共享连接。用户的挂起回调不在共享 HTTP/2 驱动中执行。

响应头到达不代表调用结束：上传、正文、trailer、缓存提交和资源归还都属于原调用。WebSocket 握手成功后转为会话寿命并停止握手总截止；失败响应继续遵守普通响应合同。关闭回调自身所属的活动操作会形成自等待的情况有明确错误出口。

## 范围与示例

本期支持 HTTP/1.0、HTTP/1.1、HTTP/2 和 HTTP/1.1 WebSocket。HTTP/3、HTTP/2 WebSocket、跨 origin H2 连接合并、证书固定、TLS 会话恢复控制／0-RTT 和主动 H2 PING 留待后续。

URL 采用独立库的 RFC 3986／3987 profile，IDNA 固定 Unicode 15.1；兼容边界见 [URL](../scoop.url/README.md) 和 [IDNA](../scoop.idna/README.md)。压缩流见 [compression](../scoop.compression/README.md)。

可运行的跨库与网络组合见 `tests/fixtures/m54-http-client`、`m54-client-basic`、`m54-client-response`、`m54-client-download`、`m54-client-sse`、`m54-client-websocket` 和 `m54-client-observer`；实施与三目标验收状态见 [M54 进度](../../../docs/milestone54/PROGRESS.md)。
