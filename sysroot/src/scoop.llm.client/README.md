# scoop.llm.client

LLM client 正在按[架构设计](../../../docs/llm-client/DESIGN.md)分项实现，进度见[实施记录](../../../docs/llm-client/PROGRESS.md)。当前支持 OpenAI／DeepSeek Chat 的普通／流式生成、SSE 增量解码、provider／model 绑定、能力检查、历史保存、结构化解码、token 用量与实际调用计时；其他协议继续实施。

最小生成调用使用同一套完整 API 的缺省参数。凭据和模型 ID 由应用显式传入：

```scoop
import scoop.asyncio.*
import scoop.llm.client.*

fun runHello(apiKey: String, modelId: String) {
    val model = LlmEndpoint(Providers.deepSeekChat(), LlmAuth.bearer(apiKey)).model(modelId)
    val loop: EventLoop = LibuvEventLoop()
    loop.run suspend { _ ->
        withLlmClient suspend { client ->
            val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
            println(result.text)
            println(result.response.usage.tokens.totalTokens)
            println(result.metadata.attempts[0L].request.sentAt)
            println(result.metadata.attempts[0L].response.receivedAt)
        }
    }
}
```

`LlmClient.open(scope)` 可复用于多个调用；传入 `Some(http)` 时借用已有 HTTP client。关闭会取消并等待本 client 的调用及完成回调，只关闭自己创建的 HTTP client。`LlmCallOptions.onFinished` 在成功、失败和取消后各执行一次，可保存失败尝试的 metadata；从本次生成或回调中等待自身 client 关闭会报错。

LLM 响应头和读取空闲时间缺省 300 秒，写入空闲时间 60 秒，总时限缺省关闭；网络／排池超时从 HTTP client 继承。缺省最多重试两次，仅适用于 dialect 明确识别的服务拒绝；每次重新认证并保留独立时间和 usage。普通 HTTP 重试、重定向、Cookie、缓存及 origin 自动认证在该请求上关闭，连接池、TLS、代理与既有 observer 继续复用。

最小请求的写法直接使用消息和设置的缺省值；添加服务端搜索或结构化输出仍使用同一个请求类型：

```scoop
import scoop.json.Json
import scoop.llm.client.*

val request = LlmRequest([Message([Text("用一句话解释值类型。")])])
val search = LlmRequest(
    input = request.input,
    tools = [Server(ToolKey("web"), WebSearch())],
    options = LlmGenerationOptions(maxOutputTokens = Some(1024L))
)
val saved = Json.encodeToString(search)
val restored = Json.decodeFromString<LlmRequest>(saved)
```

Message 默认 User，system／tools／extensions 默认空，生成参数默认省略。工具区分 ClientFunction、ClientCustom、ClientNative 与 Server；供应商规定格式的工具也可能需要应用执行。请求级、消息级、内容块级与工具级的扩展各自带 DialectId，作用于当前位置。保存格式保留原生 JSON 和媒体来源；服务端工具执行观察随相应 adapter 交付。

`LlmResponse` 保留候选和有序输出；text 只展示普通文本，clientToolCalls 只返回输入完整且具有协议形状的客户端调用。`decodeOutput<T>()` 使用 companion decoder，拒绝、不完整输出、候选／文本消息歧义均明确报错。多候选按服务器 index 选择，消息按 items 下标选择。`toInput()` 读取已保存的原生历史，不用展示文本替换 reasoning 或工具关联。

`ChatProtocol(OpenAiChatDialect())` 与 `ChatProtocol(DeepSeekChatDialect())` 的 `decodeResponse(json, request)` 解释完整 Chat 响应。DeepSeek reasoning 单独展示并完整进入历史；音频历史仅回传 ID。原始响应、未知字段和不能关联的工具调用保持可读。模型生成的无效 JSON 参数保存为 Invalid；截断输出中的调用保存为 Incomplete；两者均不进入待执行视图。外层格式错误使用带字段路径和原始正文的 LlmProtocolException。

同一 ChatProtocol 的 `createStreamDecoder(request, retention = StreamRetention.EventsOnly)` 接收 SSE parser 已定界的 `accept(event, data)`，返回带候选／item 位置的文本、reasoning、工具参数、完成调用和 usage 事件。缺省不累计普通文本；`StreamRetention.Result` 才保留完整结果与历史。正常 `[DONE]` 后可调用 `response()`，在输入结束时使用 `endInput()` 检查缺失终止。候选的 finish_reason 不等同整个流结束，usage 尾块仍会处理；EOF、服务错误或终止后新增数据均不能产生成功结果。

工具参数只有完成后才解释为可执行调用；无效或截断输入仍可观察。usage 是累计快照，只在正常 `[DONE]` 后标 Final，不把早到的候选结束当作总量已确定。未知片段保存在 Native 事件，Result 也通过 `nativeEvents` 保留必要片段；无法无损重建的消息不提供 history。这个 decoder 不执行网络 I/O；网络读取由 `LlmClient` 管理。

流式调用沿用同一 model、request 和调用选项；最小文本入口使用缺省 EventsOnly，通过同一流的文本视图读取：

```scoop
client.withStream(model, request) suspend { stream ->
    val text = stream.textDeltas()
    while (true) {
        val delta = text.next() ?: break
        print(delta.text)
    }
}
```

需要保存完整回答和历史时，使用 `client.openStream(model, request, retention = StreamRetention.Result)`；在 `stream.useAsync` 中读取，在关闭后通过 `stream.result()` 取得 LlmResult。关闭不会自动读完剩余输出；提前关闭只留下已观察的 metadata。原始 next 与 textDeltas 二选一，并发读取或切换入口明确失败。网络、协议、超时和取消都按异常传播；文本视图也处理 usage 尾块。

`stream.metadata` 每次取得不可变快照：HTTP 实际发送／响应头、首个事件／输出／文本和协议完成点分别记录。收到协议终止后停止 total 截止；资源清理后记录 finished 并执行一次 onFinished，此后 result 才可用。maxResponseBytes 同时约束流式 HTTP 正文的已读取字节，包含 SSE framing 和有限读取缓冲；不累计整条事件日志。

同一个 protocol 的 `encodeRequest(modelId, request, stream = false)` 生成 wire JSON；它本身不执行网络请求。最小文本只发送 model 和 messages，默认不设置温度、reasoning、token 上限或输出格式。两种 dialect 处理各自的媒体形式、生成参数、工具和历史；不按 model ID 切换字段。OpenAI Chat 的 Server(WebSearch) 映射请求级 web_search_options，无法表达的选项抛出带输入路径的 LlmRequestException。

DeepSeek 不同模型的 topP 生效模式不同。缺省省略 topP 时不需要额外设置；显式使用时，通过 `Providers.deepSeekChat(DeepSeekTopPMode.Thinking)` 或 `NonThinking` 选择已知规则。缺省 Unknown 会拒绝显式 topP，避免接受一个实际被服务端忽略的设置；不根据模型名称自动猜测。

```scoop
val protocol = ChatProtocol(DeepSeekChatDialect())
val request = LlmRequest([Message([Text("你好")])])
val wire = protocol.encodeRequest("application-supplied-model-id", request)
// Send wire through the application's transport, then decode the response body:
// val response = protocol.decodeResponse(responseBody, request)
```

`LlmCallMetadata` 可独立保存失败或取消记录；`LlmResult` 只组合已收尾成功的调用与模型响应。各次尝试分别保存 request.sentAt、response.receivedAt 等 UTC 视图及单调时间偏移，耗时不受实时钟回拨影响。发送和接收时间来自实际 HTTP 采样，信息响应不算最终接收；普通生成的 firstOutput／firstText 是完整响应解析后的观察时间。未提供的 body 完成／首字节 UTC 与 token 计数保留 None，耗时为零时不伪造速率。

`TokenUsage` 保存总量、非缓存输入、缓存读写、reasoning 与模态细项。未报告的计数缺省为 None，已知为零使用 Some(0L)。缓存和 reasoning 分项不自动加入总量，缺失的 totalTokens 也不会被补成一个看似完整的数字。

```scoop
import scoop.json.Json
import scoop.llm.client.TokenUsage

val usage = TokenUsage(inputTokens = Some(100L), cacheReadTokens = Some(60L))
val saved = Json.encodeToString(usage)
val restored = Json.decodeFromString<TokenUsage>(saved)
```

`ModelCapabilities()` 的每项能力缺省 Unknown。支持、明确不支持和未知采用 `Capability<T>` 的三个变体；输入／输出、普通流／实时会话、工具与生成控制分别描述。格式表记录精确的 MIME 与传递形式组合：

```scoop
import scoop.json.Json
import scoop.llm.client.*

val formats = MediaCapabilities([
    MediaFormatCapability(MediaFormat("image/png", Bytes),
        Supported(MediaLimits(maxBytesPerItem = Some(1048576L)))),
    MediaFormatCapability(MediaFormat("image/jpeg", Url), Supported(MediaLimits()))
])
val capabilities = ModelCapabilities(
    input = InputCapabilities(text = Supported(Unit), image = Supported(formats)),
    streaming = Supported(Unit)
)
val pngUrl = capabilities.input.forMedia(Image, MediaFormat("image/png", Url))
// pngUrl is Unknown: the declared support is for PNG bytes and JPEG URLs.
val restored = Json.decodeFromString<ModelCapabilities>(Json.encodeToString(capabilities))
```

MIME 参数也参与格式匹配；支持音频 `rate=24000` 不自动表示支持其他采样率。缺失上限使用 None，已知零上限仍保留。生成入口会拒绝已知不支持的输入和显式控制，并检查可观察的媒体字节数、数量及生成上限。Unknown 继续交给 adapter／服务端判断，不联网下载媒体或猜测隐藏 token 数。

以上是函数内的调用片段；应用直接使用 JSON 时也应声明 `scoop:scoop.json` 依赖。Codec 使用现有 companion 的缺省实现，Long 大整数在保存与恢复时保持精确。正式合同见 [LLM client 规范](../../../docs/specs/SCOOP-LLM-CLIENT-SPEC.md)。
