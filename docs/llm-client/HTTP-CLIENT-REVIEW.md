# HTTP client API 评审：日常入口与 LLM 消费

日期：2026-10-21。范围：当前规范、公开 API、实现与已有 fixture 的静态检查。本次没有修改 HTTP 实现或重新执行网络验收；历史完成状态以 [M54 进度](../milestone54/PROGRESS.md)为准。LLM 总体方案见[调研文档](INVESTIGATION.md)。

## 1. 总体判断

现有设计基本合理，已经有可用的简易生命周期入口和大量合理缺省设置。最值得保留的是 `withHttpClient`／`withResponse` 的作用域、显式 EventLoop、完整响应与流式 body 的分离、HTTP/2 连接复用、取消等待真实收尾，以及明确的 body 重放资格。

易用性还有一层没有完成：创建 client 很简单，但取一段文本或发送并解析 JSON 仍要重复请求、状态检查、响应关闭、codec 和缓冲上限。LLM 使用还暴露出 SSE 失败正文、自定义认证字段重定向与请求级配置等实际问题。它们无需通过重写传输栈解决。

## 2. 已有简易入口

源码 [client.scoop](../../sysroot/src/scoop.http.client/src/client.scoop)提供：

```text
HttpClient.open(scope, config = HttpClientConfig())
withHttpClient(config = HttpClientConfig(), block)
client.execute(request)
client.withResponse(request, block)
client.openSse(request, options = HttpSseOptions())
client.withSse(request, options = HttpSseOptions(), block)
client.openWebSocket(request, options = HttpWebSocketOptions())
client.withWebSocket(request, options = HttpWebSocketOptions(), block)
```

请求有 get／head／post／put／patch／delete 工厂。`RequestBody` 已有文本、JSON、字节、表单、multipart、文件、source 与 producer；不需要应用手写 Content-Length 或传输定界。完整 quickstart 在 [README](../../sysroot/src/scoop.http.client/README.md)。

普通应用只需显式启动一次 EventLoop，之后可写：

```scoop
withHttpClient suspend { client ->
    client.withResponse(HttpRequest.get(Url.parse("https://example.com/"))) suspend { response ->
        response.requireSuccess()
        println(response.body.text(1_048_576L))
    }
}
```

这些是已有 API；应避免新增会在每次 get／post 内偷偷创建 loop、线程或连接池的同步便利函数。长期应用复用一个有明确 scope 的 client，不把共享资源寿命交给 GC。

## 3. 当前缺省值及评价

| 项目 | 实际缺省值 | 评价 |
| --- | --- | --- |
| HTTPS | 系统信任；TLS 1.2～1.3；ALPN Auto | 合理；不应默认跳过验证 |
| 明文 HTTP | HTTP/1.1，H2 prior knowledge／h2c 需显式选择 | 合理 |
| 池 | 每 origin 8 条、全局 64 条、每条 H2 本地上限 100 个流 | 有实用缺省且可调；仍服从 peer 限制 |
| 空闲回收 | 60 秒，max lifetime 未设 | 合理；不影响仍在消费的长响应 |
| DNS／TCP／TLS／代理握手 | 各 30 秒 | 合理的建连起点 |
| total | None | 对流式合理；并不代表完整调用永远不会挂起 |
| poolAcquire／responseHeaders／readIdle／writeIdle | 全为 None | 语义清楚，但普通调用可能无限等待；quickstart 应明确说明并给出有限等待示例 |
| 重定向 | 开启，最多 10 次，禁止 HTTPS 降级 | 通用 HTTP 可接受；LLM POST 和自定义凭据应另设策略 |
| 重试 | 最多 2 次，默认幂等方法，status 列表为空；100 ms 起步、30 秒 backoff 上限与 jitter | 并非默认重试所有 429／5xx，也并非默认重试所有 POST，README 值得明确 |
| Cookie／HTTP cache | 不配置 | 合理，按需开启 |
| 代理 | 默认直连，环境代理显式启用 | 可预测；需要在入门文档说明，不让 CLI 用户误以为自动继承代理 |
| 内容解码 | 自动 gzip／deflate／Brotli／zstd | 合理，原始字段与编码长度仍可取得 |
| SSE reconnect | None | 对 LLM 尤其合适；不能自动重放一次生成 |
| SSE 缓冲与上限 | 8 KiB 读取缓冲、64 KiB 单行、1 MiB 单事件 | 文本事件合适；base64 图像和较大的原生 item 可能超过，需要 LLM 侧明确覆盖 |
| 完整 bytes／text／JSON | `maxBytes` 必填 | 边界明确；作为高级入口保留，日常入口可提供有限缺省值 |

依据：[config](../../sysroot/src/scoop.http.client/src/config.scoop)、[timeouts](../../sysroot/src/scoop.http.client/src/timeouts/options.scoop)、[pool](../../sysroot/src/scoop.http.client/src/pool/options.scoop)、[retry](../../sysroot/src/scoop.http.client/src/retry/options.scoop)、[redirect](../../sysroot/src/scoop.http.client/src/redirect/options.scoop)、[SSE](../../sysroot/src/scoop.http.client/src/sse/api.scoop)及[消息规范 §5](../specs/SCOOP-HTTP-MESSAGE-SPEC.md)。

补充一个重试细节：现有实现会对明确未被 H2 处理的调用作有限的安全重试，这可包括 POST；不能笼统描述成“POST 永不重试”。普通连接故障重试和服务端 status 重试仍受方法与 body 重放条件限制。[实现](../../sysroot/src/scoop.http.client/src/retry/plan.scoop)

## 4. 具体发现与建议

### 4.1 SSE 支持 POST，但简易入口丢失握手失败正文

`openSse` 接受完整 HttpRequest，会保留 method、body、认证与调用 options。已有 [SSE fixture](../../tests/fixtures/m54-client-sse/consumer/src/network.scoop)包含 POST、单次 body、重连和压缩组合。无需新增一个独立的“LLM POST SSE transport”。

问题在错误入口：[openClientSseResponse](../../sysroot/src/scoop.http.client/src/sse/response.scoop)调用 `requireSuccess`，[openClientSse](../../sysroot/src/scoop.http.client/src/sse/operation.scoop)在失败时关闭响应；[HttpStatusException](../../sysroot/src/scoop.http.client/src/response.scoop)只有 head 与 URL。LLM 调用者因此取不到 `{"error": ...}` 中的详细 code、message 或 retry 信息。

**当前可用路径**：先 `withResponse`，检查 status 并有限读取错误正文；成功后校验 Content-Type，再用 `SseSource(response.body, ...)`。其构造默认借用 body，按作用域关闭 reader 和 response，即可复用已有 parser 与 HTTP 生命周期。

**API 改进建议**：优先补一个“将已有成功响应视为 SSE”的薄入口或有错误响应回调的作用域入口，保留读取失败 body 的机会。不要让所有 `HttpStatusException` 默认读取和保存完整正文，也不要在异常中隐式挂住未关闭响应。这个改进不是最初 LLM 接入的阻塞条件。

### 4.2 自定义认证字段没有跨 origin 剥离语义

[redirectedClientFields](../../sysroot/src/scoop.http.client/src/redirect/fields.scoop)在跨 origin 时只移除 `Authorization` 和 `Cookie`。放在普通 fields 中的 `x-api-key`、`api-key`、`x-goog-api-key` 或自定义 token header 会原样转发。

这对支持任意第三方认证是具体风险。建议 LLM 默认 `redirects.enabled = false`。HTTP 若要方便此类消费者，可增加调用者显式指定的跨 origin 不转发字段，或为凭据提供按目标重新生成的入口；不依赖不断增长的品牌 header 黑名单。`HttpField.neverIndex` 是 HPACK 策略，不能当作跨站敏感字段标志。

此项应有两个本地 origin 的网络回归，实际检查第二个对端收到的 header。无需来源认证、域名资格登记或其他额外机制。

### 4.3 有认证扩展点，但当前不是任意认证的统一入口

[HttpAuthenticator](../../sysroot/src/scoop.http.client/src/authentication/options.scoop)的公共工厂只有 Basic、Bearer、Digest，底层 provider 枚举封闭。标准 Bearer 的异步加载、刷新和并发合并已经完善；刷新只响应规定的 Bearer challenge，没有 token 到期字段。[规范 §8.2](../specs/SCOOP-HTTP-CLIENT-SPEC.md)

应用可通过 request fields、interceptor 或 `onAttempt` 提供其他认证；因此当前有接入方法，但不能把它宣传成完整 OAuth SDK，也不能让所有 API-key、query、SigV4 实现都伪装为 Bearer。

LLM 先提供独立、普通的认证接口，复用 HTTP 中适合的能力。只有遇到确实需要 HTTP 跟进尝试共同参与的能力时再扩展 HTTP 的公共合同，不为支持一种签名先建一套通用认证框架。

### 4.4 `onAttempt` 的范围、顺序和能力需要交代清楚

[onAttempt](../../sysroot/src/scoop.http.client/src/interceptors/attempt.scoop)提供实际 request target 和 fields 快照，可以每次尝试添加字段；**不能替换同名字段，也不能修改 URL／method／body**。它是 `HttpClientConfig` 的选项，`HttpCallOptions` 没有对应的请求级 hook。

[执行顺序](../../sysroot/src/scoop.http.client/src/lifetime/call_owner.scoop)是认证／Cookie／WS 准备、onAttempt、cache 判断，然后才等待连接池和建立连接。该设计允许凭据请求在占用池名额前完成，但也意味着带时间戳的签名可能在队列中变旧。

建议：

- 文档明确“逐次尝试快照”和“发送时刻”不同，避免业务在此假设已建立连接。
- LLM 签名接入先使用有限排池时间、每次重试重新签名和准确的 body 字节；不能对 HTTP 后续改写不知情。
- 如果真实 SigV4 等用例要求发送前最终签名，增加窄的最终签名入口，先取得凭据再占池，最后只执行已准备好的签名；不要简单把可能请求 token 的整个回调移到持有池资源之后。
- 若共享一个 HttpClient 接入多个动态 endpoint，使必要 hook 能按调用组合，避免每个 provider 复制 client 和连接池。是否新增请求级 hook 由实际消费者决定。

### 4.5 请求级 timeout 是完整替换，容易误清空配置

`HttpCallOptions.timeouts = None` 表示继承；`Some(HttpTimeoutOptions(...))` 完整替换。例如 client 设置 total 后，请求只想调整 readIdle，却构造一个新的 HttpTimeoutOptions，会把 total 恢复为 None。redirects／retries／authentication 等也采用相应的对象替换语义。

这个合同本身没有矛盾，不应静默改成无法表达“关闭超时”的逐字段合并。建议提供小型不可变派生方法或明确的配置复制入口，并在文档中给出“继承、修改一个值、显式关闭”三个例子。避免为此引入复杂配置 DSL。

`responseHeaders` 从请求头提交开始计算，包含上传阶段；简单地为全局设置很短的头部超时会截断合法的大文件上传。`readIdle` 只在实际等待读时计时，不把应用处理事件的时间算进去。这两个差异应保留。

### 4.6 简单 HTTP／JSON 操作可以更短

建议补少量普通便利函数：

```text
client.getText(url, maxBytes = 1 MiB, options = ...)
client.getJson(url, codec, maxBytes = 8 MiB, options = ...)
client.postJson(url, value, requestCodec, responseCodec, limits = ..., options = ...)
```

这些是待设计签名。它们应复用 execute／withResponse，完整消费后返回普通值并关闭响应；公开写明非 2xx 如何报告，以及 body 上限作用于解码后的实际字节。原有 execute／withResponse 保留给需要 status、headers、stream 和自定义错误处理的调用者。

现有 `Json` 已通过 M55 支持 companion codec 缺省，但 [RequestBody.fromJson](../../sysroot/src/scoop.http.client/src/body/body.scoop)与 [HttpResponseBody.json](../../sysroot/src/scoop.http.client/src/response/body.scoop)仍要求显式 codec。可以依照相同的 `T.Companion : ValueEncoder<T>`／`ValueDecoder<T>` 条件添加便利重载，不增加反射。

[HttpField](../../sysroot/src/scoop.http/src/model.scoop)当前使用字节输入，这对底层 HTTP 合理。高层可提供明确 UTF-8 的字符串字段构造，减少每次 `.toByteArray()`，但保留重复 header 的有序列表，不能把公共 header API 简化成丢重复项的 Map。

## 5. 对缺省值的具体建议

不建议为 LLM 改掉所有 HTTP 流的全局语义。建议明确两条使用路径：

| 路径 | 建议缺省 |
| --- | --- |
| 通用 execute／withResponse | 保留现有总截止可关闭和流式行为；补充有限等待示例与继承说明 |
| 返回完整文本／JSON 的新便利函数 | 提供有限响应上限；建议总截止 60 秒、排池 30 秒，可由显式 options 覆盖，不能覆盖调用者已经配置的更合适策略 |
| LLM client | 自己明确设置长等待、关闭默认 redirect／reconnect／Cookie／HTTP cache，配置较大事件上限并处理 provider 错误 |

新便利函数若引入 60 秒总截止，应先在 spec 中定义“调用者显式配置优先、只在缺省时补值”，并保证可以显式选择无总截止。不能用 `None` 同时表示“继承”“采用便利缺省”“关闭”，必要时给此入口一个很小且明确的 timeout 参数类型。

另一种更保守的选择是便利函数完全沿用 client timeout，并把有限截止放在 quickstart 配置中。它改动更少，但用户仍需主动读到配置。本次建议优先完成便利入口与文档，再根据真实日常用例决定是否改变默认截止；不把“全部阶段都有一个任意数字”当作设计正确性的标准。

## 6. 优先级

| 优先级 | 工作 | 是否阻塞 LLM 初始实现 |
| --- | --- | --- |
| 高 | LLM 禁止默认跨站 redirect；保留 provider 错误正文；明确重试只有一层负责 | 是，属于 LLM 行为合同；现有 HTTP 已有可用组合路径 |
| 高 | 确认 SSE 大行／事件上限，按多模态接口配置 | 是，完整多模态流所需 |
| 高 | JSON 便利 codec 和 HTTP quickstart 的默认值说明 | 不阻塞协议编码，但应与新库一起改善日常使用 |
| 中 | getText／getJson／postJson 等有限缓冲便利函数 | 否，显著改善易用性 |
| 中 | SSE 已有响应适配、明确的自定义敏感字段转发规则 | 否，有手工组合方案 |
| 按真实需要 | 请求级最终签名 hook、不可变配置派生方法 | 有对应认证／配置消费者后实施 |

实施后测试以真实结果为准：简易入口编译运行、非 2xx 正文、body 超限、POST SSE、取消后池复用、自定义 header 跨 origin、超时继承和覆盖、每次尝试签名与实际 body 一致。现有协议层已经证明的 framing、TLS 和 GC 合同不在 LLM 层重复全量验证。
