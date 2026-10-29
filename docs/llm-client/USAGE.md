# LLM client 使用示例与缺省参数

更新：2026-10-26。本文展示已实现的普通生成、流式、持久会话和辅助 API；可用入口见[库说明](../../sysroot/src/scoop.llm.client/README.md)，验证范围见[实施记录](PROGRESS.md)。Converse 已接入同一个 generate／openStream 入口，流式使用 AWS EventStream；Gemini／Messages 的 token 预计数已接入，实际发送／接收时间戳与 token 用量均可读取。服务商附加信息直接保留原文，不设公共计费入口。Messages provider 缺省选择 API 版本 2026-10-21 和 4096 输出上限，沿用相同的 model／request／generate／openStream 入口；版本与默认上限均可显式设置。示例使用现有 Scoop 的数组、enum、Option、挂起函数、作用域及 JSON codec 语法，不新增语言机制。文本投影、结构化输出和按位置的扩展已结合 [AI SDK 对照评审](AI-SDK-REVIEW.md)细化。

简单调用和完整调用共用 `generate(model, request, options = LlmCallOptions())`。不再保留先前草稿中的 `generate(model, prompt = ...)` 重载，也不增加 `generateText`、`SimpleClient` 或自动选择模型的入口。简单形态直接来自 request、消息和 options 的缺省字段。

## 1. 共同的数据与调用入口

下列为已实现的请求构造签名；companion codec、构造检查、媒体、工具与历史数据见[库规范](../specs/SCOOP-LLM-CLIENT-SPEC.md#3-公共请求数据)。generate 与 openStream／withStream 已接入 Chat／Messages／Responses／Gemini generateContent／Interactions／Converse／Cohere Chat v2／Ollama Chat；countTokens 已接入 Gemini GenerateContent、Anthropic Messages 与 OpenAI Responses。

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
client.compactResponse(model, request, options = LlmCallOptions()) -> ResponsesCompactionResult
```

网络操作、client 创建和关闭都是 suspend；`withLlmClient`／`withStream` 的 block 是 suspend 函数。`withLlmClient` 只将 `open` 与既有 `useAsync` 的资源寿命组合起来，`withStream` 同理，不改变请求、缺省参数、错误或结果语义。

Provider 的官方 API 地址缺省值可供 endpoint 使用；`baseUrl = None` 表示采用该缺省。自定义 provider 没有地址缺省时必须传 URL，缺失在发送前报错。认证与 model ID 必须显式提供；本地无认证服务使用 `LlmAuth.none()`。库不查找环境变量、猜 API key 名称或选择默认模型。

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

OpenRouter Chat 使用相同的最小请求；modelId 原样传递，例如由应用配置选择组织／模型名称：

```scoop
val model = LlmEndpoint(Providers.openRouterChat(), LlmAuth.bearer(apiKey)).model(modelId)
val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
println(result.text)
```

同一请求可以加入客户端函数和 WebSearch 服务端工具，搜索使用当前 openrouter:web_search 工具接口。reasoning_details、签名和文件 annotations 随 toInput 保存／回传；流式保存历史时设置 retention = StreamRetention.Result。只需改变 provider，不需要另一套简易调用 API。OpenRouter 的路由或完整原生 reasoning 配置可通过 DialectExtension(OpenRouterChatDialect().id, value) 提供，不能覆盖已经设置的通用字段。

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
        options = LlmGenerationOptions(maxOutputTokens = Some(1024UL))
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

返回的 count 保存 inputTokens、Server 方法和 RequestInput 范围；未报告的计数或模型版本保持 None。Gemini／Messages 标为 Estimated，OpenAI Responses 按端点合同标为 Exact；Exact 描述本次输入，不保证后续历史变化或工具执行的用量相同。metadata 保存独立的调用时间和各次尝试。Gemini 提交完整 generateContentRequest；Messages 保留 system、工具、thinking、schema 和历史，对当前不接受的服务端工具及 URL／file 媒体在发送前报错。cachedInputTokens 表示计数时的缓存输入规模，不表示实际缓存命中。`countTokens` 不替应用发起后续 generate，也不覆盖响应报告的 usage；尚无实际本地 tokenizer，因此没有空的本地模式入口。

Responses 使用同一个入口，provider 选择自然决定计数协议：

```scoop
val model = LlmEndpoint(Providers.openAiResponses(), LlmAuth.bearer(apiKey)).model(modelId)
val counted = client.countTokens(model, request)
println(counted.count.inputTokens)
```

Responses 计数保留 system、多模态、工具、reasoning、输出 schema 和原始历史，也保留显式 conversation／previous_response_id；生成上限、采样、后台执行和存储等根设置被省略。未映射的输入字段会报路径错误，例如计数端点没有 prompt 模板参数，不能静默丢掉模板后只数剩余文字。DeepSeek Responses 没有此适配，会在认证前拒绝；真实服务互通待可用凭据后补测。

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

None 的语义按字段固定：`httpClient = None` 创建作用域内拥有的 HTTP client，Some 借入并复用；`baseUrl = None` 使用 provider 的 API 地址缺省；`readIdle = None` 关闭这个阶段的 idle 截止。它们均不兼任“继承上一层设置”。借入 client 的 TLS、代理和连接池配置保留；LLM 的请求级等待、重试、redirect 等策略按调用选项明确设置，不能通过复制连接池或覆盖原 observer 实现。

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

### 7.6 Responses 的读取、取消与删除

使用同一 client 和 endpoint 显式管理服务端 response，不需要再次指定模型。以下创建 background response，再读取一次当前快照：

```scoop
val created = client.generate(model, LlmRequest([Message([Text("分析这份材料")])],
    extensions = [DialectExtension(model.endpoint.provider.protocol.dialectId,
        Json.parse("{\"background\":true}"))]))
val responseId = created.response.id!!
val current = client.retrieveResponse(model.endpoint, responseId)
println(current.text)
```

当前快照可能仍为 Incomplete；应用决定何时再次读取。需要关联客户端工具时，传入创建时的 tools；需要加密 reasoning 等字段时，显式设置 include。缺省不注册服务端返回的工具声明，未关联的调用保留原文：

```scoop
val current = client.retrieveResponse(model.endpoint, responseId, tools = request.tools,
    include = ["reasoning.encrypted_content"])
val saved = Json.encodeToString(current)
```

取消 background response 和删除已存储 response 是独立操作，调用方按需执行：

```scoop
val cancelled = client.cancelResponse(model.endpoint, responseId)
val deleted = client.deleteResponse(model.endpoint, responseId)
println(deleted.deletion.deleted)
```

读取和取消返回既有 LlmResult，可保存、读取工具或恢复原始历史。删除返回 ResponsesDeletionResult，保留服务端实际 id、deleted 和 native。metadata 记录这次操作的实际收发与完成时间；历史 usage 位于 response.usage，不按查询耗时计算模型生成速度。认证、重试、取消和 onFinished 继续使用 LlmCallOptions。OpenAI Responses dialect 提供这些操作；DeepSeek 的无状态 Responses 在认证前拒绝。测试使用本地 mock，真实服务互通待凭据可用后补测。

### 7.7 Responses 的显式上下文压缩

OpenAI Responses 的压缩使用同一个 model 和 request，缺省沿用 LlmCallOptions。应用决定调用时机；返回的整个窗口用于替换旧 input，保留的消息、媒体和不透明压缩项一起进入下一轮：

```scoop
val compacted = client.compactResponse(model, request)
val saved = Json.encodeToString(compacted)
val restored = Json.decodeFromString<ResponsesCompactionResult>(saved)
val input = ArrayList<LlmInputItem>()
for (item in restored.toInput()) input.add(item)
input.add(Message([Text("继续上一项任务")]))
val next = LlmRequest(input.toArray(), system = request.system,
    tools = request.tools, options = request.options)
val result = client.generate(model, next)
```

压缩只提交 input、system 和该端点支持的原生扩展；tools 和生成 options 留给下一次生成。toInput 保留完整 output，后续请求显式提供 instructions，也不将压缩操作的 id 用作 previous_response_id。compaction.usage 是本次压缩的 token 用量，metadata 保存本次实际收发、完成时间与重试；保留下来的旧文本不记为本次首字输出。附加信息在 native 中原样保留。

ModelCapabilities.serverCompaction 缺省 Unknown；明确不支持会在认证前失败。是否实现该操作由已选 Responses dialect 决定，OpenAI 开启，DeepSeek Responses 不提供。纯适配入口为 ResponsesProtocol.compactUrl／encodeCompactionRequest／decodeCompaction。依据：[OpenAI 压缩文档](https://developers.openai.com/api/docs/guides/compaction)。

### 7.8 显式 API 地址与认证回调

由调用方提供已有 API 的 baseUrl、headers 和认证。比如 Azure v1 使用包含 `/openai/v1` 的地址，并明确选择 Responses 或 Chat provider：

```scoop
val endpoint = LlmEndpoint(Providers.openAiResponses(), LlmAuth.header("api-key", apiKey),
    Some(Url.parse("https://resource.openai.azure.com/openai/v1")))
val model = endpoint.model("deployment-name")
val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
```

旧版 Chat 的完整路径前缀和版本 query 同样由调用方显式配置，库只追加 chat/completions：

```scoop
val endpoint = LlmEndpoint(Providers.openAiChat(), LlmAuth.header("api-key", apiKey),
    Some(Url.parse("https://resource.openai.azure.com/openai/deployments/deployment-name?api-version=2024-10-21")))
val model = endpoint.model("deployment-name")
val result = client.generate(model, request)
```

普通生成、SSE 和工具历史共用同一组入口，库不创建或选择云部署。应用提供访问 token 时，可把 authentication 换成 `LlmAuth.bearer(suspend { acquireAccessToken() })`；轮换 API key 同样可用 `LlmAuth.header("api-key", suspend { acquireApiKey() })`。回调每次尝试执行一次，凭据获取和刷新由应用负责，库不查找环境变量或登录账号。测试仅使用本地 mock，真实互通待可用凭据后补测。Azure 示例依据：[v1 文档](https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle)。

### 7.9 Interactions 的读取、取消与删除

已有 interaction ID 可直接读取状态快照；端点仍使用 `Providers.geminiInteractions()`，不需要再次提供模型名。需要关联客户端工具时显式传入原声明，缺省为空：

```scoop
val snapshot = client.retrieveInteraction(endpoint, interactionId, tools = request.tools)
val saved = Json.encodeToString(snapshot)
println(snapshot.response.usage.tokens.totalTokens)
println(snapshot.metadata.attempts[0].response.receivedAt)

// Cancel a running background interaction when the application requests it.
val cancelled = client.cancelInteraction(endpoint, runningInteractionId)

// Delete a stored interaction when the application no longer needs it.
val deleted = client.deleteInteraction(endpoint, storedInteractionId)
println(deleted.metadata.attempts[0].status)
```

读取和取消返回 LlmResult，可保存原始步骤并按已有工具／历史规则继续使用。取消快照可能没有 steps，此时 history 保持未知。历史 token 用量位于 response.usage，不作为本次查询的生成速率。删除返回 InteractionsDeletionResult，native 保存实际响应字节；成功的空正文保持空数组，不合成确认字段。调用选项仍为同一 LlmCallOptions，库不轮询、自动取消或删除；GET 的事件读取与接续见下一节。依据：[Interactions 资源操作](https://ai.google.dev/api/interactions-api-v1#retrieving-an-interaction)。

### 7.10 Interactions 事件读取与显式接续

缺省从头读取已有 interaction 的原始事件；传入 lastEventId 可从服务端游标之后继续。每次调用独立记录实际 HTTP 时间，游标只随已交付事件更新：

```scoop
// savedCursor is None for the first read, or the last cursor saved by the application.
val stream = client.openInteractionEvents(endpoint, interactionId, lastEventId = savedCursor)
try {
    stream.useAsync suspend { opened ->
        while (true) {
            val event = opened.next() ?: break
            println(Json.encodeToString(event))
        }
    }
} finally {
    // Available after cleanup, including an interruption; the application can save this cursor.
    println(stream.lastEventId)
    println(stream.metadata.attempts[0].response.receivedAt)
}
```

`withInteractionEvents(endpoint, interactionId, lastEventId = savedCursor) suspend { stream -> ... }` 使用相同缺省值并按作用域关闭。流对象在失败后仍可读取 lastEventId，应用在自己的异常处理分支保存它。

`LlmNativeEvent.value` 保留完整 JSON，包括 event_id、usage 与服务商扩展。接续可能从 step.delta 开始，因此流不提供缺少前缀的聚合结果，也不会自动重连。应用若已保存完整前缀，可把前后事件交给同一个 `InteractionsProtocol.createStreamDecoder(request, Result)`；纯原始事件读取使用 `createEventDecoder()`。GET 回放中的 usage 不计入本次请求的生成用量。

### 7.11 显式查询模型目录

最小查询只需要 endpoint。列表返回一页，目录原文和实际查询时间均可保存；查询不会自动修改已有绑定的 capability，也不成为生成前提：

```scoop
val listed = client.listModels(endpoint)
for (model in listed.page.models) println(model.id ?: "(unnamed)")
println(listed.page.native)
println(listed.metadata.attempts[0].response.receivedAt)

val info = client.retrieveModel(endpoint, "caller-model")
val saved = Json.encodeToString(info)
val model = endpoint.model(info.model.id ?: info.requestedId)
```

OpenAI Chat／Responses、Anthropic Messages、两个 Gemini provider、OpenRouter、Cohere、Ollama 和 Bedrock 支持列表与单模型读取；两个 DeepSeek provider 目前只有文档公开的列表操作。代理未提供目录时，查询返回服务错误，仍可直接使用已知模型 ID 生成。model.id 只保存服务端实际提供的标识；协议允许缺失时为 None，requestedId 单独保存本次请求的值。

分页沿用具体 API 的 query 和原始返回值。Gemini 的数量是 ULong，缺省省略；page token 保持原字符串：

```scoop
val first = client.listModels(geminiEndpoint, geminiModelsQuery(pageSize = Some(20UL)))
when (first.page.native["nextPageToken"]) {
    case Some(token) -> {
        val next = client.listModels(geminiEndpoint,
            geminiModelsQuery(pageSize = Some(20UL), pageToken = Some(token.asString())))
        println(next.page.models.size)
    }
    case None -> {}
}
```

Anthropic 使用 `anthropicModelsQuery(limit = Some(20UL), afterId = Some(lastId))` 或 beforeId；first_id、last_id、has_more 保留在 page.native。其他过滤参数可用既有 UrlQuery 添加，重复字段和空值保持原样。库不自动翻页或统一推断游标，模型原生能力字段也不自动变成当前生成入口的 capability。Gemini 的 models/ 资源名前缀由适配器去除以得到可绑定的 id，完整 name 保留在 model.native。

Cohere 的 endpoint 使用 API 地址前缀，缺省 https://api.cohere.com，普通生成追加 v2/chat、模型查询追加 v1/models。自定义代理地址也传共同前缀；分页使用 `cohereModelsQuery(pageSize = Some(20UL), pageToken = Some(token), endpoint = Some("chat"))`，next_page_token 仍从 page.native 读取。Cohere 可省略 name，列表中对应条目的 id 保持 None。

Ollama 的详情读取由适配器编码为 POST /show。默认只传模型名称，需要 verbose 时使用同一个入口的原生扩展：

```scoop
val detail = client.retrieveModel(ollamaEndpoint, "caller-model:tag",
    extensions = [ollamaModelOptions(verbose = Some(true))])
println(detail.model.native)
println(detail.requestedId)
```

Ollama 详情没有服务端模型 ID，因此 detail.model.id 为 None。OpenRouter 单模型查询接受 author/slug（含 :free 等后缀），native 保留完整 data 包装。Bedrock 模型查询需要显式配置模型目录 API 的 endpoint，例如 https://bedrock.us-east-1.amazonaws.com，并使用对应的 SigV4 认证；库不改写既有 runtime URL。列表过滤参数用 UrlQuery 显式提供。

### 7.12 显式上传、引用和读取文件

OpenAI Chat／Responses 和 Anthropic Messages 使用同一个文件入口。以下函数接收应用已读入的 PDF bytes；模型仍由应用指定，文件操作不查询模型目录。

```scoop
suspend fun summarizePdf(endpoint: LlmEndpoint, modelId: String, pdf: Array<Byte>): LlmResult =
    withLlmClient suspend { client ->
        val uploaded = client.uploadFile(endpoint, LlmFileUpload("report.pdf", pdf, "application/pdf"))
        try {
            val document = Document(MediaSource.FileReference(uploaded.file.reference!!, "application/pdf"))
            client.generate(endpoint.model(modelId), LlmRequest([Message([Text("概括这份报告。"), document])]))
        } finally {
            client.deleteFile(endpoint, uploaded.file.id)
        }
    }
```

文件引用和结果有 companion codec，保存后可在同 dialect 中继续使用。上传缺省 MIME 是 application/octet-stream；上例明确声明 PDF。OpenAI 的 purpose 缺省为 user_data，Anthropic 不发送 purpose。库只执行明确调用，例子里的删除由应用安排。

```scoop
val metadata = client.retrieveFile(endpoint, fileId)
val page = client.listFiles(endpoint, openAiFilesQuery(limit = Some(10UL)))
// Pass the service's actual cursor explicitly; native fields are preserved unchanged.
val next = client.listFiles(endpoint, openAiFilesQuery(after = Some(page.page.native["last_id"]!!.asString())))
val content = client.downloadFile(endpoint, generatedFileId)
val bytes = content.bytes
```

Anthropic 的分页使用 `anthropicFilesQuery(limit = Some(10UL), page = Some(cursor))`，cursor 从 page.native 的 next_page 取得；上传的文件通常不能下载，生成文件可按实际服务合同读取。下载返回原始 bytes，不自动写本地路径或解析 JSON。所有文件调用都保留实际收发时间；服务端文件信息仍在 file.native／page.native，删除响应正文在 result.native。

原生上传选项直接使用 upload.extensions：对象成员是 multipart 字段名，值为字符串。例如 Anthropic 的 `expires_in_seconds`，OpenAI 的 `expires_after[anchor]` 和 `expires_after[seconds]`；需要时明确传入，不由库计算。尚无文件 API 适配的 dialect 会在认证前报告请求错误。

Gemini generateContent／Interactions 也使用 uploadFile、listFiles、retrieveFile 和 deleteFile。上传返回的 id 是 files/... 资源名；reference 使用服务端实际 URI，缺少 URI 时为 None。调用方明确指定媒体种类和 MIME：

```scoop
val uploaded = client.uploadFile(geminiEndpoint, LlmFileUpload("report.pdf", pdfBytes, "application/pdf"))
val reference = uploaded.file.reference ?: throw IllegalStateException(Some("The service returned no file URI"))
val input = Document(MediaSource.FileReference(reference, "application/pdf"))
val page = client.listFiles(geminiEndpoint, geminiFilesQuery(pageSize = Some(10UL)))
val registered = client.registerFiles(geminiEndpoint, ["gs://caller-bucket/existing.pdf"])
```

Gemini 上传扩展直接对应 file metadata 的 JSON 成员，displayName 来自 filename；其他原生字段按原值发送。registerFiles 只登记调用方提供的已有 GCS URI。文件状态和时间继续在 native 中原样保留，库不轮询、判断过期或推导 URI。Gemini Files 未公布按 ID 下载上传文件的端点，因此该 provider 的 downloadFile 在认证前报错；实际 downloadUri 留在 native。

Google 文件真实测试位于库 tests/live/google_files.scoop，默认 @Ignore，显式运行仍要求 GOOGLE_AI_API_KEY。它仅上传少量合成文本，读取后在 finally 中删除；普通文件测试均使用本地 mock。

### 7.13 Gemini Live 的会话与纯协议入口

Gemini Live 使用独立 provider，与普通生成共用 endpoint／model／认证绑定。最小 setup 自然只含 model，打开会话会发送 setup 并等待服务端确认；后续内容由应用显式发送和读取。

```scoop
val endpoint = LlmEndpoint(Providers.geminiLive(), LlmAuth.header("x-goog-api-key", apiKey))
val model = endpoint.model(modelId)

withLlmClient suspend { client ->
    client.withGeminiLiveSession(model) suspend { session ->
        val sent = session.sendClientContent([Message([Text("你好")])])
        println(sent.timing.sentAt)
        while (true) {
            val received = session.receive() ?: break
            println(received.received.at)
            when (received.message.event) {
                case ServerContent(content) -> { if (content.turnComplete == Some(true)) break }
                else -> {}
            }
        }
    }
}
```

例子选择在收到一次 turnComplete 后退出；连续对话可以继续 send／receive，并按消息内容处理媒体、转写或工具调用。sendRealtimeInput 接受原始音频或单帧图像 bytes，sendToolResponse 接受应用执行后的结果。session.opening 是 HTTP 升级与 setup 确认的调用记录；setupSent、setupComplete 及后续收发记录可保存，消息的 UTC／elapsed 共用 session 起点。openingOptions 的 total 只约束打开过程；会话在作用域退出或 client 关闭时清理。

同一协议的纯编解码也可以独立使用，以下命令只产生 JSON 数据：

```scoop
val protocol = endpoint.provider.protocol
if (!(protocol is BidiGenerateContentProtocol)) throw IllegalArgumentException()

val setup = protocol.encodeSetup(model.id)
val text = protocol.encodeClientContent([Message([Text("你好")])])
val audio = protocol.encodeRealtimeInput(GeminiLiveInput(
    audio = Some(GeminiLiveBlob(pcmBytes, "audio/pcm;rate=16000"))
))
```

音频响应等原生配置通过同一 setup 的 extensions 设置，不从模型名推断。客户端工具与服务端搜索仍使用已有 ToolDefinition：

```scoop
val dialect = GeminiLiveDialect().id
val tools: Array<ToolDefinition> = [
    ClientFunction(ToolKey("weather"), "weather", weatherSchema),
    Server(ToolKey("search"), WebSearch())
]
val configuration = GeminiLiveSetup(tools = tools, extensions = [DialectExtension(dialect,
    Json.parse("{\"generationConfig\":{\"responseModalities\":[\"AUDIO\"]},\"outputAudioTranscription\":{}}"))])
val command = protocol.encodeSetup(model.id, configuration)
val message = protocol.decodeMessage(receivedJson, tools)
val saved = Json.encodeToString(message)
val restored = Json.decodeFromString<GeminiLiveMessage>(saved)
```

将同一个 configuration 传给 `client.withGeminiLiveSession(model, configuration)` 即使用这些原生设置；无需另建配置或简易 API。真实测试 `tests/live/google_live.scoop` 默认 @Ignore，仅显式选择并配置 GOOGLE_AI_API_KEY 与 GOOGLE_AI_LIVE_MODEL 后运行，一次只发送一句短提示。

message.event 区分内容、函数调用、取消和会话通知。内容同时保留音频、文本、reasoning 与三类转写；turnComplete 和 interactionStatus 分别读取。应用执行完整的客户端调用后，用收到的 reference 构造 `GeminiLiveToolResult(call.reference, responseObject)`，再交给 encodeToolResponse。responseObject 原样包含结果及需要的 scheduling 字段；库不运行工具或恢复连接。每条 usage 独立保存，缺失分量保持未知；native 保存完整服务端消息。

### 7.14 OpenAI Realtime 会话与纯协议入口

Realtime 使用自己的 provider，模型由连接 URL 指定。打开会话等待 session.created；省略 configuration 时采用服务器默认值，显式配置时再发送 session.update 并等待确认。以下例子明确选择文本输出：

```scoop
val endpoint = LlmEndpoint(Providers.openAiRealtime(), LlmAuth.bearer(apiKey))
val model = endpoint.model(modelId)
withLlmClient suspend { client ->
    client.withRealtimeSession(model,
        RealtimeConfiguration(outputModality = Some(RealtimeOutputModality.Text))) suspend { session ->
        session.sendItemCreate(Message([Text("Hello")]))
        session.sendResponseCreate()
        while (true) {
            val received = session.receive() ?: break
            when (received.message.event) {
                case TextDelta(_, text) -> print(text)
                case ResponseDone(response) -> {
                    println(Json.encodeToString(response.usage))
                    break
                }
                else -> {}
            }
        }
    }
}
```

会话不会在提交音频或回传工具结果后自动生成。音频使用 sendAudioAppend(bytes, mimeType)，MIME 用于能力与格式检查，必须与实际配置一致；随后按需要显式 sendAudioCommit 和 sendResponseCreate。sendItemCreate 可以发送 ToolResult；sendResponseCancel、item 的 retrieve／delete／truncate 与完整原生命令也各有独立入口。

opening 保存打开调用的 HTTP 记录；sessionCreated、configurationSent／configurationUpdated 以及后续 RealtimeSent／RealtimeReceived 保存实际收发时间。缺省没有初始配置时，两个 configuration 记录为 None。receive 的 tools 缺省使用 initialConfiguration.tools；动态修改工具后，传入 Some(对应工具声明)，Some([]) 明确关闭本次工具关联。发送更新不会暗中更改这个上下文，收到 error 也不会自动关闭可用连接。会话与 client 的资源关闭合同与其他 WebSocket 入口相同。

初始化期间收到 error 时，open 抛出 RealtimeSessionRejectedException，received 保留原始消息及实际接收时间，并先清理连接。session.id 和收发记录的 sessionId 是本地调用 ID；服务器分配的 session ID 保留在 sessionCreated.message 的原始配置中。

同一个 protocol 也可独立产生命令并解码事件，纯协议操作不执行网络 I/O：

```scoop
val endpoint = LlmEndpoint(Providers.openAiRealtime(), LlmAuth.bearer(apiKey))
val protocol = endpoint.provider.protocol
if (!(protocol is RealtimeProtocol)) throw IllegalArgumentException()
val url = protocol.webSocketUrl(endpoint.baseUrl, modelId)

val item = protocol.encodeItemCreate(Message([Text("你好")]))
val generate = protocol.encodeResponseCreate()
val append = protocol.encodeAudioAppend(pcm24kBytes)
val commit = protocol.encodeAudioCommit()
```

最小 response.create 只有 type。音频 append／commit 与 response.create 是三条不同命令，由应用决定何时发送。使用 session 更新或单次 response 覆盖时，配置共用同一个类型，None 表示没有设置；空字符串、空工具列表、false 和原生 null 均保留。

```scoop
val dialect = OpenAiRealtimeDialect().id
val configuration = RealtimeConfiguration(
    instructions = Some("回答简短。"),
    tools = Some([ClientFunction(ToolKey("weather"), "weather", weatherSchema)]),
    outputModality = Some(RealtimeOutputModality.Audio),
    maxOutputTokens = Some(RealtimeTokenLimit.Unlimited),
    extensions = [DialectExtension(dialect,
        Json.parse("{\"audio\":{\"input\":{\"turn_detection\":null},\"output\":{\"voice\":\"alloy\"}}}"))]
)
val update = protocol.encodeSessionUpdate(configuration)
val oneResponse = protocol.encodeResponseCreate(RealtimeResponseRequest(configuration))
val saved = Json.encodeToString(configuration)
```

`RealtimeResponseRequest(input = Some([]), conversation = Some("none"))` 明确指定不使用历史且不写入默认对话；input=None 保留服务端会话上下文。函数结果使用既有 ToolResult 和实际 call ID；服务端 MCP 使用同 dialect 的 NativeToolDefinition。需要指定 MCP 子工具等完整原生命令时，使用独立的 RealtimeNativeCommand 与 encodeNativeCommand，内容原样保留。当前适配以官方文档与本地 mock 验证，没有使用失效的官方 OpenAI key。

每条服务端消息可独立解码，tools 使用应用已声明的工具；不使用工具时自然省略该参数。文本、音频字节与转写分别交付，response.done 提供完整响应的读取视图：

```scoop
val message = protocol.decodeMessage(Json.parse(receivedJson), tools)
when (message.event) {
    case TextDelta(position, text) -> println(position.item.responseId + ": " + text)
    case AudioDelta(_, bytes) -> println(bytes.size)
    case ResponseDone(response) -> {
        println(response.text)
        println(response.clientToolCalls.size)
        println(Json.encodeToString(response.usage))
    }
    else -> println(message.type)
}
val saved = Json.encodeToString(message)
```

参数 done 和 item done 不等于函数可执行；上例中的 `response.clientToolCalls` 只包含成功终态、完整参数和已关联声明的调用。MCP 留在 serverToolActivities 中。音频的 MIME 与采样率来自实际会话／响应配置，decoder 不猜格式、不拼接整段音频；输入转写的 usage 与响应 usage 分开。含音频的响应不提供可重放 History，因为 response.done 没有完整音频数据且 item.create 不接受 assistant 音频。`message.native` 保留完整事件，未知通知、附加信息和可恢复 error 仍可由应用读取。

### 7.15 OpenAI GPT-Live

GPT-Live 与 Realtime 使用不同 provider。打开时发送 session.start 并等待 session.started；最小配置只有显式模型 ID，音频格式等采用服务器缺省。pcm24kMonoBytes 是应用提供的 raw mono PCM16LE 音频：

```scoop
val endpoint = LlmEndpoint(Providers.openAiGptLive(), LlmAuth.bearer(apiKey))
val model = endpoint.model(modelId)
withLlmClient suspend { client ->
    client.withGptLiveSession(model) suspend { session ->
        val sent = session.sendAudioAppend(pcm24kMonoBytes, "audio/pcm;rate=24000")
        val received = session.receive()
        println(sent.timing.sentAt)
        when (received) {
            case Some(record) -> {
                println(record.received.at)
                when (record.message.event) {
                    case OutputTranscript(text, _, _) -> println(text)
                    else -> {}
                }
            }
            case None -> {}
        }
    }
}
```

连续交互使用同一 send／receive；一次 receive 仅表示一条事件，音频片段没有逐轮 audio-done。sendInstructionsAppend、sendThinkingAppend、sendCommentaryAppend 的 delegationId 缺省编码为 null，也可显式关联服务端提供的 ID。sendSessionClose 发送 API 关闭请求；需要完成确认时继续读取 SessionClosed。作用域退出或 close 则释放本地连接，不能替代服务端确认。

服务端 Responses 后端的配置使用已有工具定义，前端和后端模型 ID 分别明确提供：

```scoop
val dialect = OpenAiGptLiveDialect()
val tools: Array<ToolDefinition> = [
    ClientFunction(ToolKey("lookup"), "lookup", Json.parse("{\"type\":\"object\"}")),
    Server(ToolKey("search"), WebSearch())
]
val setup = GptLiveSetup(extensions = [
    dialect.responsesDelegation(backendModelId, tools)
])
// Pass setup to the same openGptLiveSession / withGptLiveSession entry point.
val protocol = GptLiveProtocol(dialect)
val start = protocol.encodeSessionStart(frontendModelId, setup)
```

audio／voice／store 等其他原生配置使用该 dialect 的扩展；session.update 只修改 API 允许更新的设置。Responses 后端的 reasoning 等扩展归属 OpenAiResponsesDialect，response.item.create 的工具结果也复用该嵌套协议的 reference 和实际 call ID。response.event 保留原文及 delegation_id；它的生命周期快照会省略输出内容，不能直接当作普通 Responses 的完整终态结果。库不执行工具或 delegation。

startSent、sessionStarted、GptLiveSent／GptLiveReceived 均可保存；消息 UTC 与 elapsed 使用同一会话起点。usage.updated 的音频时长、context_window 及附加信息保留原文，不换算 token、费用或缓存寿命。无可用 GPT-Live key 时使用本地 mock 验证，未用失效的官方 OpenAI key。

## 8. 缺省值与使用检查

| 位置 | 缺省行为 | 显式覆盖 |
| --- | --- | --- |
| Provider／认证／model ID | 必填；不选全局供应商、凭据或模型 | 选择另一绑定；无认证用 none |
| Endpoint URL | 采用所选 provider 的已知 API 地址缺省 | `baseUrl = Some(url)`；无缺省的 provider 必填 |
| Model capabilities | 省略描述时各项为 Unknown | `capabilities = Some(description)`；不改变 adapter 能否编码 |
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

这组例子收敛了四处设计：取消 prompt 重载；普通消息默认 User；生成参数只在 request 一处表达；流式完整结果在关闭后读取。认证、模型、工具业务函数仍要求明确提供，预计数通过显式调用触发。这些签名和生命周期边界已写入库规范；代表组合由[共同入口示例测试](../../sysroot/src/scoop.llm.client/tests/examples/common_entry.scoop)及各协议测试覆盖，验证范围见[覆盖矩阵](COVERAGE.md)。
