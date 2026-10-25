# LLM client 使用示例与缺省参数

日期：2026-10-25。本文结合已实现的 Chat／Messages／Responses／Gemini generateContent／Interactions 普通与流式生成及后续 API 草案检查接口负担；可用入口见[库说明](../../sysroot/src/scoop.llm.client/README.md)，验证范围见[实施记录](PROGRESS.md)。Converse 已接入同一个 generate／openStream 入口，流式使用 AWS EventStream；Gemini／Messages 的 token 预计数已接入，实际发送／接收时间戳与 token 用量均可读取。服务商附加信息直接保留原文，不设公共计费入口。Messages provider 缺省选择 API 版本 2026-10-21 和 4096 输出上限，沿用相同的 model／request／generate／openStream 入口；版本与默认上限均可显式设置。示例使用现有 Scoop 的数组、enum、Option、挂起函数、作用域及 JSON codec 语法，不新增语言机制。文本投影、结构化输出和按位置的扩展已结合 [AI SDK 对照评审](AI-SDK-REVIEW.md)细化。

简单调用和完整调用共用 `generate(model, request, options = LlmCallOptions())`。不再保留先前草稿中的 `generate(model, prompt = ...)` 重载，也不增加 `generateText`、`SimpleClient` 或自动选择模型的入口。简单形态直接来自 request、消息和 options 的缺省字段。

## 1. 共同的数据与调用入口

下列为已实现的请求构造签名；companion codec、构造检查、媒体、工具与历史数据见[库规范](../specs/SCOOP-LLM-CLIENT-SPEC.md#3-公共请求数据)。generate 与 openStream／withStream 已接入 Chat／Messages／Responses／Gemini generateContent／Interactions／Converse／Cohere Chat v2；countTokens 已接入 Gemini GenerateContent 与 Anthropic Messages。

```scoop
public class LlmRequest public constructor(
    public val input: Array<LlmInputItem>,
    public val system: Array<ContentPart> = [],
    public val tools: Array<ToolDefinition> = [],
    public val options: LlmGenerationOptions = LlmGenerationOptions(),
    public val extensions: Array<DialectExtension> = []
)
```

`LlmInputItem.Message(content, role = LlmRole.User, extensions = [])` 是普通 enum 变体；`content` 是 `Array<ContentPart>`。文本写成 `Text("你好")`，图片、文档等使用同一个集合的其他变体。这里借助 Scoop 已有的 expected-type enum 变体解析：`Message([Text("你好")])` 等价于显式写出 `LlmInputItem.Message([ContentPart.Text("你好")])`，不依赖 DSL、字符串隐式转换或另一套消息表示。

调用签名摘录如下；它们描述同一套完整 API，不是额外包装层：

```text
LlmEndpoint(provider, authentication, baseUrl: Url? = None)
endpoint.model(id, capabilities: ModelCapabilities? = None)

LlmClient.open(scope, httpClient: HttpClient? = None)
withLlmClient(httpClient: HttpClient? = None, block)

client.generate(model, request, options = LlmCallOptions()) -> LlmResult
client.openStream(model, request, options = LlmCallOptions(),
                  retention = StreamRetention.EventsOnly) -> LlmStream
client.withStream(model, request, options = LlmCallOptions(),
                  retention = StreamRetention.EventsOnly, block) -> T
client.countTokens(model, request, options = LlmCallOptions()) -> LlmTokenCountResult
```

网络操作、client 创建和关闭都是 suspend；`withLlmClient`／`withStream` 的 block 是 suspend 函数。`withLlmClient` 只将 `open` 与既有 `useAsync` 的资源寿命组合起来，`withStream` 同理，不改变请求、缺省参数、错误或结果语义。

Provider 的官方部署缺省值可供 endpoint 使用；`baseUrl = None` 表示采用该缺省。自定义 provider 没有部署缺省时必须传 URL，缺失在发送前报错。认证与 model ID 必须显式提供；本地无认证服务使用 `LlmAuth.none()`。库不查找环境变量、猜 API key 名称或选择默认模型。

下面各段是独立示例，共用这些 imports；凭据、model ID、媒体字节和业务函数均由应用传入。模型是否支持示例组合由绑定的 capability 和实际 API 决定，示例名称不构成模型能力声明。

```scoop
import scoop.asyncio.*
import scoop.collections.addAll
import scoop.http.client.*
import scoop.json.*
import scoop.llm.client.*
import scoop.time.Duration
import scoop.url.Url
```

## 2. 最小文本调用

下面使用 DeepSeek；Gemini 普通生成仅更换同一个 endpoint 的参数：

```scoop
val model = LlmEndpoint(Providers.geminiGenerateContent(),
    LlmAuth.header("x-goog-api-key", apiKey)).model(modelId)
```

两者都由应用提供 apiKey 和 modelId，继续使用下面相同的 `client.generate` 调用。

DeepSeek 的 Responses 使用独立 provider，沿用同一个最小请求和流式入口：

```scoop
val model = LlmEndpoint(Providers.deepSeekResponses(), LlmAuth.bearer(apiKey)).model(modelId)
val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
println(result.text)
```

其 reasoning、函数／apply_patch 调用和服务端搜索结果可以保存后通过 toInput 回传。后续请求带上完整历史及本轮客户端工具结果，不设置 previous_response_id。DeepSeek Responses 允许 User／Developer 的图片和工具结果图片，文件内容无对应映射；文档中会忽略的显式控制在编码时报告路径错误。只在显式 topP 时使用 `Providers.deepSeekResponses(DeepSeekTopPMode.Thinking)` 或 NonThinking；模型 ID 保持应用给出的原值。

使用 Interactions 时将 provider 换成 `Providers.geminiInteractions()`，其余调用不变。需要关闭服务端存储时，在同一个 request.extensions 中传入 `DialectExtension(GeminiInteractionsDialect().id, Json.parse("{\"store\":false}"))`；省略 store 时沿用服务端默认值。`result.toInput()` 保留原始 steps，`previous_interaction_id` 则由应用显式放入本次请求的扩展。

Converse 使用明确的部署地址与同一套调用；`baseUrl`、`apiKey` 和 `modelId` 都由应用配置提供。API key 认证采用 bearer，AWS 凭据使用下文的 `LlmAuth.awsSigV4`。模型 ID 可以是服务方支持的 ARN，库会将它完整编码为一个路径段。

```scoop
val model = LlmEndpoint(Providers.bedrockConverse(),
    LlmAuth.bearer(apiKey), Some(Url.parse(baseUrl))).model(modelId)
val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
```

需要流式展示时直接使用同一 model；缺省 EventsOnly，仍会读取尾部 usage。Converse 的 messageStop 结束内容，正常 EOF 才完成整次调用；保存历史时沿用下文的 `retention = StreamRetention.Result`。

```scoop
client.withStream(model, LlmRequest([Message([Text("你好")])])) suspend { stream ->
    val text = stream.textDeltas()
    while (true) {
        val delta = text.next() ?: break
        print(delta.text)
    }
}
```

```scoop
fun runHello(apiKey: String, modelId: String) {
    val model = LlmEndpoint(
        provider = Providers.deepSeekChat(),
        authentication = LlmAuth.bearer(apiKey)
    ).model(modelId)

    val loop: EventLoop = LibuvEventLoop()
    loop.run suspend { _ ->
        withLlmClient suspend { client ->
            val result = client.generate(model, LlmRequest([
                Message([Text("用一句话解释值类型。")])
            ]))
            println(result.text)
        }
    }
}
```

这里省略了官方 URL、user role、system、tools、生成选项和调用选项。时间戳与各次尝试的 usage 在 `result.metadata` 中，最终响应用量在 `result.response.usage`，不需要开启额外模式。`result.text` 是主 candidate 的普通文本视图；工具、reasoning、媒体、停止状态和其他候选仍保留在完整结果中，空文本不代表生成失败。

应用已有 EventLoop 时，直接在其中复用 client；上面的 loop 只展示程序边界，不会在每次 generate 内创建。`endpoint.model(id)` 不发网络请求；缺少准确能力资料的项为 Unknown。

## 3. 同一个请求增加多模态、system 和生成选项

```scoop
suspend fun compareDocument(
    client: LlmClient,
    model: LlmModel,
    imageBytes: Array<Byte>,
    pdfBytes: Array<Byte>
): LlmResult {
    val request = LlmRequest(
        input = [Message([
            Text("比较图片与文档中的结论，指出不一致处。"),
            Image(MediaSource.Bytes(imageBytes, mimeType = "image/png")),
            Document(MediaSource.Bytes(pdfBytes, mimeType = "application/pdf"))
        ])],
        system = [Text("根据输入材料作答，并说明材料不足之处。")],
        options = LlmGenerationOptions(maxOutputTokens = Some(1024L))
    )
    return client.generate(model, request)
}
```

`Document` 的 MIME 决定检查 PDF 或其他文档能力；不能把文件引用可用当作模型理解 PDF。显式不支持的图片／PDF／组合在编码前报告，库不静默 OCR 或删除输入。其他采样与 reasoning 参数仍省略，不因示例增加多模态就自动改变。

生成参数只放在 `LlmRequest.options`；timeout、重试和观察回调放在 `LlmCallOptions`。常用业务参数保存为普通不可变值后复用，不建立 client／endpoint／model／request 四层隐式生成参数合并。

### 3.1 同一调用返回结构化输出

```scoop
struct AnalysisReport(val summary: String) {
    companion object : ValueDecoder<AnalysisReport>
}

suspend fun summarize(client: LlmClient, model: LlmModel, source: String): LlmResult {
    val schema = Json.parse(
        "{\"type\":\"object\",\"properties\":{\"summary\":{\"type\":\"string\"}},\"required\":[\"summary\"],\"additionalProperties\":false}"
    )
    val result = client.generate(model, LlmRequest(
        input = [Message([Text(source)])],
        system = [Text("总结输入材料。")],
        options = LlmGenerationOptions(output = OutputFormat.JsonSchema(schema))
    ))
    val report = result.decodeOutput<AnalysisReport>()
    println(report.summary)
    return result
}
```

缺省 output 是 Text；需要 JSON 时只改变同一选项。Schema 发给模型，companion decoder 在本地解析完整结果；不自动从 decoder 生成 Schema。模型拒绝、输出不完整、选中输出不存在或无效 JSON 会明确报告，结果里的原始内容、usage 与 metadata 仍可读取。多候选或多结构化 item 时必须明确选择解析对象。

### 3.2 在实际内容块上配置缓存扩展

```scoop
suspend fun askWithContext(
    client: LlmClient,
    model: LlmModel,
    document: String,
    cacheControl: DialectExtension
): LlmResult {
    val request = LlmRequest([Message([
        Text(document, extensions = [cacheControl]),
        Text("根据前面的材料回答：主要结论是什么？")
    ])])
    return client.generate(model, request)
}
```

这里的 cacheControl 由所选 dialect 的配置构造，包含归属和具体协议数据。该示例适用于支持在文本块上设置缓存断点的入口；扩展在错误位置或错误 dialect 下不能被默默忽略。Request、message、part 和工具定义的 extensions 都默认空，各自只作用于自身位置，不存在父子字段继承。时间戳和 cache usage 仍在结果 metadata 中，扩展本身不保证命中。

## 4. 流式输出与完整结果

只显示增量文本时，使用相同的 model 和 request；流的缺省策略只保留协议所需的关联状态与当前 metadata，不累积完整输出、可完整回传的历史或原始事件日志。

```scoop
suspend fun printStreaming(client: LlmClient, model: LlmModel) {
    val request = LlmRequest([Message([Text("解释一下单态化。")])])
    client.withStream(model, request) suspend { stream ->
        val text = stream.textDeltas()
        while (true) {
            val delta = text.next() ?: break
            print(delta.text)
        }
    }
}
```

`textDeltas()` 是同一事件消费者的筛选视图，delta 保留 candidate／item／part 关联；这个展示例只请求一个默认候选。其他事件仍由 adapter 处理并更新 usage 和状态，不会提前截断 usage 尾块。投影的 next 与原始 stream.next 二选一，不能并行消费或中途切换，也不复制流或缓存 replay。`next()` 的 None 只在正常协议终止后返回，网络、协议和取消错误按类型抛出。

需要最终结果用于历史回传时，在同一个流入口显式选择 `Result`。这里使用 `openStream` 加通用 `useAsync`，以便在资源收尾后读取最终 metadata：

```scoop
suspend fun collectStreaming(client: LlmClient, model: LlmModel): LlmResult {
    val stream = client.openStream(
        model,
        LlmRequest([Message([Text("解释一下单态化。")])]),
        retention = StreamRetention.Result
    )
    try {
        stream.useAsync suspend { active ->
            while (true) {
                val event = active.next() ?: break
                when (event) {
                    case TextDelta(delta) -> print(delta.text)
                    else -> Unit
                }
            }
        }
    } finally {
        println(stream.metadata.finishedAt)
    }
    return stream.result()
}
```

`metadata` 是观察时刻的不可变快照，关闭后仍可读取；`result()` 是关闭后的纯数据读取，不再发网络请求。只有选择 Result、协议响应完整且收尾成功时它才返回 `LlmResult`；事件模式、提前退出、失败或取消不会伪装出完整结果。关闭不自动把未消费的生成读完。失败时上面的 finally 仍能读取已知 metadata；若 open 本身失败，则通过调用选项的 `onFinished` 取得失败记录。

`withStream` 保持普通作用域函数的泛型返回值，不把 block 内的快照偷偷改成关闭后的快照。需要完整计时的读取放在 `useAsync` 之后，或通过 `onFinished` 完成通知进行；响应终止与资源关闭的区别因此保持可见。

## 5. 客户端工具与服务端搜索、保存后继续

工具都在同一个 tools 数组中；服务端工具不要求一个假的执行函数。下例假定 model 支持这两类工具的组合，应用只执行自己声明的 `lookup_weather`，完成一轮客户端工具回传。

供应商定义格式、应用执行的 shell 等工具使用 ClientNative；它与 ClientFunction／ClientCustom 一样返回客户端调用。只有真正由服务端执行的工具才使用 Server。原生工具配置由 dialect 提供，库不自动执行任何客户端工具。

```scoop
struct WeatherArguments(val city: String) {
    companion object : ValueDecoder<WeatherArguments>
}

suspend fun answerWithTools(
    client: LlmClient,
    model: LlmModel,
    lookupWeather: suspend (String) -> String
): LlmResult {
    val weatherKey = ToolKey("weather")
    val tools: Array<ToolDefinition> = [
        ClientFunction(
            key = weatherKey,
            name = "lookup_weather",
            description = "Look up the current weather in a city.",
            parameters = Json.parse(
                "{\"type\":\"object\",\"properties\":{\"city\":{\"type\":\"string\"}},\"required\":[\"city\"],\"additionalProperties\":false}"
            )
        ),
        Server(key = ToolKey("web"), tool = ServerTool.WebSearch())
    ]
    val request = LlmRequest(
        input = [Message([Text("查询杭州天气，并搜索适合今天参观的展览。")])],
        tools = tools
    )
    val first = client.generate(model, request)

    val saved = Json.encodeToString(first)
    val restored = Json.decodeFromString<LlmResult>(saved)
    for (activity in restored.serverToolActivities) {
        println(activity.status)
    }
    if (restored.clientToolCalls.size == 0L) return restored

    val nextInput = ArrayList<LlmInputItem>()
    nextInput.addAll(request.input)
    nextInput.addAll(restored.toInput())
    for (call in restored.clientToolCalls) {
        if (call.toolKey != weatherKey) {
            throw IllegalStateException(Some("Unexpected client tool"))
        }
        val arguments = call.decodeArguments<WeatherArguments>()
        val answer = lookupWeather(arguments.city)
        nextInput.add(ToolResult(call = call.reference, content = [Text(answer)]))
    }

    return client.generate(model, LlmRequest(input = nextInput.toArray(), tools = tools))
}
```

`toolChoice` 默认 Auto，不因声明 web search 就强制搜索；需要指定时在 `LlmGenerationOptions(toolChoice = ToolChoice.Specific(ToolKey("web")))` 中明确填写。Server 活动始终不进入 `clientToolCalls`；该视图只含输入已完整且具有正确协议形状的客户端调用，具体 DTO 解码仍可能失败。

Schema 由应用提供，companion codec 只负责本地解码，不从任意 codec 猜 Schema。库的 `LlmResult`、metadata 和历史数据提供普通 companion codec，因此保存／恢复使用已有 Json 缺省 codec 入口。`toInput()` 保留原生 reasoning、服务端工具记录和关联顺序；本例使用默认单候选，多候选必须显式选择。`saved` 展示保存／恢复的数据边界，写入哪个文件或数据库由应用决定；模型绑定与凭据不在其中。

这段代码只继续一次，不隐藏 Agent 循环。没有客户端调用时保留原结果；第二次仍可能要求工具或服务器 continuation，调用者根据返回的状态决定下一步。解码或业务执行失败向调用者传播，示例没有将失败冒充工具成功结果。

## 6. 时间戳与 token

每个完整结果默认带 metadata；重试数据按尝试分别保留，不把第一次发送与最后一次响应拼接成一次网络耗时。

```scoop
suspend fun inspectUsage(client: LlmClient, model: LlmModel) {
    val result = client.generate(model, LlmRequest([
        Message([Text("简述这次请求的处理过程。")])
    ]))
    val metadata = result.metadata
    println(metadata.totalDuration)
    for (attempt in metadata.attempts) {
        println(attempt.request.sentAt)
        println(attempt.response.receivedAt)
        println(attempt.response.firstOutputAt)
        println(attempt.http.queue?.duration)
        println(attempt.http.connect?.duration)
        println(attempt.http.originTls?.duration)
    }
    val usage = result.response.usage.tokens
    println(usage.inputTokens)
    println(usage.nonCachedInputTokens)
    println(usage.outputTokens)
    println(usage.cacheReadTokens)
    println(usage.reasoningTokens)
    val savedMetadata = Json.encodeToString(metadata)
    println(savedMetadata)
}
```

`attempt.request`／`attempt.response` 是该次尝试的时间记录；未观察到的时间和计数为 Option，打印 None 表示未知。`result.response.usage` 保存最终响应的归一化用量、原生 usage 与完整性；各次尝试用量单独保留，不自动汇总重试。

`attempt.http` 直接保留 HTTP 层观察到的排池、DNS、连接、代理与 TLS 阶段。阶段的 started／finished 是相对调用起点的单调偏移，可以随 metadata 保存；duration 只在阶段结束时可用，失败或取消也可能结束一个阶段。复用连接时不会补造新建连接或 TLS 耗时；底层没有 UTC 的事件不使用当前时钟代替。outputTokensPerSecond 仅在用量为 Final 且响应耗时完整、非零时提供。

缓存相关附加信息保留在原生响应或 usage 中；库不估计 TTL 或判断缓存是否失效。请求／响应时间戳保持独立，应用可结合自己的信息使用。

```scoop
suspend fun countInput(client: LlmClient, model: LlmModel): LlmTokenCountResult {
    val request = LlmRequest([Message([Text("需要提前计数的输入。")])])
    return client.countTokens(model, request)
}
```

返回的 count 保存 inputTokens、Server 方法、RequestInput 范围和 Estimated 性质；未报告的计数或模型版本保持 None。metadata 保存独立的调用时间和各次尝试。Gemini 提交完整 generateContentRequest；Messages 保留 system、工具、thinking、schema 和历史，对当前不接受的服务端工具及 URL／file 媒体在发送前报错。cachedInputTokens 表示计数时的缓存输入规模，不表示实际缓存命中。`countTokens` 不替应用发起后续 generate，也不覆盖响应报告的 usage；尚无实际本地 tokenizer，因此没有空的本地模式入口。

服务商附加信息只保留原文。普通响应的 `response.native`、实际用量的 `usage.native` 及原生事件保存对应数据，由具体 dialect 按需访问。没有公共计费类型或配置，也不做内容校验、换算或估算。

## 7. 共享 HTTP、切换 protocol 和覆盖等待时间

```scoop
suspend fun useGateway(
    http: HttpClient,
    baseUrl: Url,
    apiKey: String,
    modelId: String
) {
    val endpoint = LlmEndpoint(
        provider = Providers.openAiResponses(),
        authentication = LlmAuth.bearer(apiKey),
        baseUrl = Some(baseUrl)
    )
    val model = endpoint.model(modelId)
    withLlmClient(httpClient = Some(http)) suspend { client ->
        val result = client.generate(
            model,
            LlmRequest([Message([Text("完成这项较长的推理。")])]),
            options = LlmCallOptions(
                timeouts = LlmTimeouts(
                    responseHeaders = Some(Duration.seconds(600L)),
                    readIdle = None
                )
            )
        )
        println(result.text)
    }
}
```

None 的语义按字段固定：`httpClient = None` 创建作用域内拥有的 HTTP client，Some 借入并复用；`baseUrl = None` 使用 provider 的部署缺省；`readIdle = None` 关闭这个阶段的 idle 截止。它们均不兼任“继承上一层设置”。借入 client 的 TLS、代理和连接池配置保留；LLM 的请求级等待、重试、redirect 等策略按调用选项明确设置，不能通过复制连接池或覆盖原 observer 实现。

若要使用 Chat protocol，显式改用 `Providers.openAiChat()` 创建另一个 endpoint／model 绑定。不能只替换 URL 后继续套用 Responses 的工具或原生历史；两个 provider 可以共享同一个 http。两个绑定的 capability 各自取得，未知模型不触发目录联网查询。

### 7.1 Cohere Chat v2 使用同一生成入口

Cohere 的普通生成只需更换 provider，缺省发送 model 和 messages；流式使用同一模型和请求，自动添加 stream=true。图片 detail 属于图片扩展，RAG documents 属于请求扩展，不把某条消息的文档自动挪到全局。

```scoop
suspend fun askCohere(client: LlmClient, apiKey: String, modelId: String): LlmResult {
    val model = LlmEndpoint(Providers.cohereChat(), LlmAuth.bearer(apiKey)).model(modelId)
    return client.generate(model, LlmRequest([Message([Text("解释值类型。")])]))
}
```

完整响应仍可单独解码。将原请求一并传入，函数调用才能绑定到应用声明的 ToolKey；返回值沿用相同的 text、usage、clientToolCalls、toInput 和 companion codec。

```scoop
fun readCohereReply(body: String, request: LlmRequest): LlmResponse =
    CohereChatProtocol(CohereChatDialect()).decodeResponse(Json.parse(body), request)
```

usage 读取实际 tokens，billed_units 另外保留在原生数据中。thinking 和 tool_plan 不混入 text；引用关联原始内容块，未定义单位的范围保留原文。这是纯 JSON 解码，不产生客户端发送／接收时间戳。

需要最终用量、历史和时间戳时显式保留 Result；只消费事件则使用缺省 EventsOnly，不累计普通正文。

```scoop
suspend fun streamCohere(client: LlmClient, apiKey: String, modelId: String): LlmResult {
    val model = LlmEndpoint(Providers.cohereChat(), LlmAuth.bearer(apiKey)).model(modelId)
    val stream = client.openStream(model, LlmRequest([Message([Text("解释值类型。")])]),
        retention = StreamRetention.Result)
    stream.useAsync suspend { opened ->
        val text = opened.textDeltas()
        while (true) {
            val delta = text.next() ?: break
            print(delta.text)
        }
    }
    return stream.result()
}
```

工具参数可以交错到达，ClientToolCall 等到 message-end 的停止原因确认后交付；晚到引用不会移动已发出的 item／part 位置。createStreamDecoder(request) 也使用相同缺省 EventsOnly，供已有 SSE source 的调用者使用；它接收已定界的 event／data，不自行创建连接。

### 7.2 Ollama 原生 Chat

原生 /api/chat 使用固定 protocol／dialect 组合，沿用同一个生成入口。缺省 URL 为 http://localhost:11434/api，认证与已部署的模型 ID 显式提供：

```scoop
suspend fun askLocalModel(modelId: String): String {
    val model = LlmEndpoint(Providers.ollamaChat(), LlmAuth.none()).model(modelId)
    return withLlmClient suspend { client ->
        client.generate(model, LlmRequest([Message([Text("你好")])])).text
    }
}
```

普通调用自动发送 stream=false；采样、think、输出上限与 keep_alive 继续使用服务端缺省。推理档位通过 reasoning.effort 显式提供，字符串由所选模型定义；图片使用 Bytes，函数结果直接使用解码后的 reference。原生设置仍放在同一 request 的扩展位置，例如与 temperature 合并 options.seed：

```scoop
val request = LlmRequest([Message([Text("返回一个简短的 JSON 对象")])],
    options = LlmGenerationOptions(temperature = Some(0.2), output = JsonObject),
    extensions = [DialectExtension(OllamaChatDialect().id,
        Json.parse("{\"options\":{\"seed\":7},\"keep_alive\":\"5m\"}"))])
```

已有原生响应也可以直接转为相同的 LlmResponse，将原请求传入以关联应用声明的函数：

```scoop
fun readOllamaReply(body: String, request: LlmRequest): LlmResponse =
    OllamaChatProtocol(OllamaChatDialect()).decodeResponse(Json.parse(body), request)
```

text、decodeOutput、usage、clientToolCalls、toInput 和 companion codec 与其他协议相同。thinking 保持独立，工具引用保留可选真实 ID 和原始 function 元数据；没有 ID 的调用不会获得编造的 ID。只有 done=true 且 stop 下完整、有效、能关联声明的函数才进入 clientToolCalls。

prompt_eval_count 包含缓存输入，只有实际报告缓存量时才计算普通输入。created_at 是服务端时间；usage.native 中的 duration 字段是服务端纳秒数据，纯解码不会生成客户端计时。空或未知 done_reason 保持未完成，load／unload 也不产生可直接追加的 assistant 历史。

流式沿用 openStream／withStream，不需要单独的 Ollama client 或简易入口；同一 model 绑定自动选择 NDJSON，正文发送 stream=true。保留结果与其他协议一样显式选择 Result：

```scoop
suspend fun streamLocalModel(client: LlmClient, modelId: String): LlmResult {
    val model = LlmEndpoint(Providers.ollamaChat(), LlmAuth.none()).model(modelId)
    val stream = client.openStream(model, LlmRequest([Message([Text("你好")])]),
        retention = StreamRetention.Result)
    stream.useAsync suspend { opened ->
        val text = opened.textDeltas()
        while (true) { val delta = text.next() ?: break; print(delta.text) }
    }
    return stream.result()
}
```

缺省 EventsOnly 不累积普通正文，但保留 usage 与计时。thinking、文本和工具拥有稳定的输出位置；工具记录按到达顺序追加，不以重复 index 合并，只有最终 stop 才开放完整函数输入。已有 NDJSON source 时可使用 createNdjsonDecoder(request)，其 retention 同样缺省 EventsOnly。

累计 token 快照只替换已报告字段，不求和；created_at 可以逐条变化，最终 response.createdAt 保存终止记录的实际时间，与 request.sentAt／response.receivedAt 分开。EOF 未收到 done=true 时失败；未知消息增量保持 Native，不能确定完整内容时不提供可追加历史。空消息触发 load／unload 时，原生服务返回的单个 application/json 响应也使用相同入口与完成规则。

### 7.3 Bedrock 的 SigV4 认证

认证仍放在 endpoint，普通和流式调用使用相同的 model。凭据源由应用提供，每次尝试重新调用；静态凭据也可以用返回已持有值的普通 suspend lambda 提供。服务名缺省为 bedrock，区域与部署 URL 显式传入。

```scoop
suspend fun useBedrock(
    client: LlmClient,
    baseUrl: Url,
    region: String,
    modelId: String,
    credentials: suspend () -> AwsSigV4Credentials
): LlmResult {
    val endpoint = LlmEndpoint(Providers.bedrockConverse(),
        LlmAuth.awsSigV4(region, credentials), Some(baseUrl))
    return client.generate(endpoint.model(modelId),
        LlmRequest([Message([Text("Explain this API briefly.")])]))
}
```

AwsSigV4Credentials 保存 accessKeyId、secretAccessKey 和缺省 None 的 sessionToken，不参与序列化。签名使用实际 URL、请求字段和 body，不再调用编码器；重试会重新取凭据和签名。持有 Bedrock API key 时直接使用既有 LlmAuth.bearer。签名在排池前完成，应用提供的 HTTP hook 不应随后添加签名所需的 AWS 字段。该入口按官方文档及本地 mock 验证，真实 AWS 互通尚待凭据就绪后验证。

### 7.4 Responses WebSocket 的纯协议入口

WebSocket 的 create 使用同一个 LlmRequest。省略 options 时使用默认 lane 和正常生成；已有模型 ID、tools、reasoning、原始历史和 previous_response_id 的表达方式与 HTTP 相同：

```scoop
val protocol = ResponsesProtocol(OpenAiResponsesDialect())
val request = LlmRequest([Message([Text("你好")])])
val create = protocol.encodeWebSocketCreate(modelId, request)
val warmup = protocol.encodeWebSocketCreate(modelId, request,
    ResponsesWebSocketCreateOptions(streamId = Some("main"), generate = Some(false)))
val steer = protocol.encodeWebSocketSteer(responseId,
    [Message([Text("请把重点放在接口兼容性上")])])
```

已有 WebSocket 传输时，decodeWebSocketEvent 读取完整 JSON 消息并保留原始字段。按 streamId 将 Response 类事件交给对应响应的 ResponsesStreamDecoder；Steering、Error 与 Native 由会话处理。每个 response 使用独立 decoder，SSE 与 WebSocket 共用聚合实现：

```scoop
val decoder = ResponsesStreamDecoder(protocol, request, StreamRetention.Result)
val event = protocol.decodeWebSocketEvent(receivedText)
when (event.kind) {
    case Response -> { decoder.accept(event); Unit }
    else -> handleSessionEvent(event)
}
```

上例中的 modelId、responseId、receivedText 和 handleSessionEvent 由应用提供；普通 JSON codec 可保存事件和最终 LlmResponse。默认 EventsOnly 只交付增量，完整结果显式选择 Result。纯协议入口不进行网络 I/O；持久连接使用下面的会话入口。

### 7.5 Responses 持久连接与消息时间戳

同一 endpoint 的多个模型和 lane 可以共用 ResponsesSession。以下使用 OpenAI Responses；dialect 的 supportsWebSocket 表明适配器是否提供会话操作，DeepSeek Responses 当前提供 HTTP／SSE。最小会话使用现有参数的缺省值；model 和 request 与普通生成相同：

```scoop
withLlmClient suspend { client ->
    client.withResponsesSession(model.endpoint) suspend { session ->
        val sent = session.create(model, request)
        println(sent.timing.sentAt)
        val decoder = ResponsesStreamDecoder(session.protocol, request, StreamRetention.Result)
        while (!decoder.isFinished) {
            val received = session.receive() ?: break
            println(received.received.at)
            when (received.event.kind) {
                case Response -> { decoder.accept(received.event); Unit }
                case Error -> throw Exception(Some(Json.stringify(received.event.native)))
                else -> println(Json.stringify(received.event.native))
            }
        }
        decoder.endInput()
        println(decoder.response().text)
    }
}
```

需要并行响应时，在 create 的 options 中设置 streamId，用一个接收循环按实际 streamId／response ID 分发到各自 decoder。发送方向保持串行，可与接收并行。运行中更新使用 `session.steer(model, responseId, input)`；accepted、pending、failed 等通知保留原文，自动后继使用新 decoder，工具结果由应用显式 create 回传。会话不会把一个通知当作当前响应的完成，也不会自动执行工具、重发输入或重连。

create／steer 返回 ResponsesWebSocketSent，保存实际 command 和本地提交／完成时间；receive 返回 ResponsesWebSocketReceived，保存完整事件与完整消息取出后的时间。两者均有 companion codec，elapsed 使用同一个 session.id／startedAt 的单调时间轴，适合保存并按实际响应关联。握手有单独的 session.handshake，不能拿它的时间代替消息时间。handshakeOptions 的超时、重试和 onFinished 只管握手；帧、消息和关闭设置用现有 HttpWebSocketOptions。

openResponsesSession 与 withResponsesSession 是同一个资源合同的两种写法。关闭 client 会取消并等待所属会话，借入 HttpClient 时保持其所有权；一次消息 I/O 的取消会结束该连接。服务端 Error 仍作为事件交付，其他 lane 可以继续；传输、JSON 或路由字段错误会结束连接并交付原异常。本地 mock 可验证这些行为，真实供应商互通在配置凭据后单独补测。

## 8. 缺省值与使用检查

| 位置 | 缺省行为 | 显式覆盖 |
| --- | --- | --- |
| Provider／认证／model ID | 必填；不选全局供应商、凭据或模型 | 选择另一绑定；无认证用 none |
| Endpoint URL | 采用所选 provider 的已知部署缺省 | `baseUrl = Some(url)`；无缺省的 provider 必填 |
| Model capabilities | 使用该入口的准确已知资料，其余 Unknown | `capabilities = Some(description)`；不改变 adapter 能否编码 |
| Message role | 新输入为 User | 指定其他 role；模型原生历史用 toInput 保留 |
| Request | input 必填；system、tools 为空 | 在同一 request 增加内容和工具 |
| 方言扩展 | request／message／part／tool 的 extensions 分别为空 | 在有协议语义的位置显式填写，不作父子继承 |
| 生成选项 | 采样、reasoning、结构化输出与 token 上限省略；toolChoice 为 Auto | 各可选数值用 Some；协议强制的输出上限由公开 dialect 缺省补齐，例如 4096 |
| 输出格式 | Text，共用 generate 和流式入口 | JsonObject／JsonSchema；完整结果使用 decodeOutput 解码 |
| 生成选项复用 | 仅 request 持有生成参数 | 复用 `LlmGenerationOptions` 普通值，不建立多层隐式合并 |
| 流式保留 | EventsOnly；始终保留计时与 usage，不保存逐事件历史 | Result 累积最终结果，不累积原始事件日志 |
| 流的读取 | 一个消费者，读取完整事件或 textDeltas 投影 | 文本视图保留位置与错误语义，不改变保留策略 |
| HTTP 所有权 | 未传入时每个 client 作用域拥有一个池 | Some 借入已有 client；不关闭借入对象 |
| 调用等待 | 响应头 300 秒、read idle 300 秒、write idle 60 秒、total None；排池／建连沿 HTTP 配置 | 传完整 `LlmCallOptions` 值；`LlmTimeouts` 未填字段采用自身缺省，None 明确关闭 |
| 重试 | 至多两次明确可重试的服务拒绝；已输出、结果不明和取消不重试 | 调用选项显式改变策略，不叠加 HTTP 重试 |
| 时间与 token | 默认采集关键时间戳、单调耗时和实际 usage；缺失不填零 | `onFinished` 观察失败／取消及实际收尾；预计数单独调用 |

`onFinished` 是 `LlmCallOptions` 中可选的完成观察回调，generate 和流式使用相同合同：每次逻辑调用实际收尾后通知一次，成功、打开流失败、消费失败和取消均保留已知 metadata，不重包装 CancellationException。回调不在共享传输驱动中执行，不参与生成请求的序列化。

这组例子收敛了四处设计：取消 prompt 重载；普通消息默认 User；生成参数只在 request 一处表达；流式完整结果在关闭后读取。认证、模型、工具业务函数仍要求明确提供，预计数通过显式调用触发。正式实施前将这些签名和生命周期边界写入库规范，再以真实 Cone 消费者验证。
