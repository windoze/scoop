# scoop.llm.client

LLM client 正在按[架构设计](../../../docs/llm-client/DESIGN.md)分项实现，进度见[实施记录](../../../docs/llm-client/PROGRESS.md)。当前支持 OpenAI／DeepSeek／OpenRouter Chat、Anthropic Messages、OpenAI／DeepSeek Responses、Gemini generateContent、Interactions、Bedrock Converse、Cohere Chat v2 与 Ollama Chat 的普通／流式生成；共用 provider／model 绑定、能力检查、历史保存、结构化解码、token 用量与实际调用计时。其他协议继续实施。

Gemini Live 通过 `Providers.geminiLive()` 绑定同一 endpoint／model，`BidiGenerateContentProtocol` 提供 setup、clientContent、realtimeInput、toolResponse 与消息解码。音频、文本、转写、工具取消、用量与原始通知可以序列化；不推断播放完成、会话空闲或自动执行工具。当前仅交付纯线协议，持久连接与消息计时继续接入；见[Live 示例](../../../docs/llm-client/USAGE.md#713-gemini-live-的纯协议入口)。

OpenAI Chat／Responses 与 Anthropic Messages 的文件操作使用同一 client：uploadFile、listFiles、retrieveFile、downloadFile 和 deleteFile。Gemini generateContent／Interactions 共用上传、列表、读取与删除入口，并提供 registerFiles 登记已有 GCS URI。上传结果提供可保存的可选文件引用，服务端附加字段保留原文；用法见[文件 API 示例](../../../docs/llm-client/USAGE.md#712-显式上传引用和读取文件)。

OpenRouter 使用 Providers.openRouterChat()，缺省部署 https://openrouter.ai/api/v1；模型 ID 由应用显式选择。普通生成和 SSE 保留 reasoning_details 的签名、加密块及完整历史，文件 annotations 可随 toInput 回传。客户端函数与 openrouter:web_search 服务端工具可以同时声明，模型没有报告执行活动时不从 usage 推断调用。图片／文件／音频／视频输入采用各自文档形状，输出图片与音频保持 Native；纯入口与范围见[规范第 39 节](../../../docs/specs/SCOOP-LLM-CLIENT-SPEC.md#39-openrouter-chat-dialect)。当前以本地 mock 验证，真实互通待可用凭据后补测。

普通 Cone 测试只使用纯 decoder 或回环地址上的 HTTP／SSE／EventStream／NDJSON mock，回复由测试指定，不连接供应商。真实 API 验证用例统一标记 `@Ignore`，默认跳过；Google 文件用例位于 tests/live/google_files.scoop，其余已验证用例保存在仓库外。独立运行器只有在显式选择一个用例且其所需 key／endpoint 环境变量齐备时才启用 `--ignored`，缺少配置报告 SKIPPED，不发请求。凭据只从环境读取，日志脱敏。

Responses 使用 `Providers.openAiResponses()` 创建同样的 endpoint／model，再调用 `client.generate` 或 `client.openStream`。`ResponsesProtocol(OpenAiResponsesDialect())` 也提供 encodeRequest／decodeResponse，保留有序 item、reasoning、消息 phase、工具 call ID、服务端搜索与引用；`toInput()` 返回可序列化的原始 output items。usage 区分缓存读、写与普通输入，缺失字段保持未知。

`Providers.deepSeekResponses(topPMode = Unknown)` 复用 ResponsesProtocol，使用独立的 DeepSeekResponsesDialect 和 https://api.deepseek.com 部署缺省。generate／openStream 的调用不变；reasoning.content 不要求 OpenAI 的 summary，函数、apply_patch custom 和服务端搜索共用原始历史及工具结果表示。每轮携带完整历史和成对的调用／结果，previous_response_id 等服务端状态参数在发送前拒绝。显式 topP 使用与 DeepSeek Chat 相同的生效模式设置，省略时不需要额外配置。当前 DeepSeek 适配 HTTP／SSE，supportsWebSocket 为 false；纯入口、网络 mock 与使用边界见[规范第 38 节](../../../docs/specs/SCOOP-LLM-CLIENT-SPEC.md#38-deepseek-responses-dialect)。

Responses 的最小请求只发送 model 和 input，生成参数缺省省略。request.system 编码为文本 instructions；需要缓存断点等内容块扩展时，在 input 中显式使用 System message。工具结果沿用解码所得的 reference，其 context 保留实际调用类型和所需标识；函数、custom、computer、local shell、shell 与 apply_patch 分别编码自己的结果形状，应用负责执行并提交结果。

Responses WebSocket 的纯协议入口提供 webSocketUrl、encodeWebSocketCreate、encodeWebSocketSteer 和 decodeWebSocketEvent。create 复用 LlmRequest，streamId／generate 缺省省略；steer 只接收用户文本、图片和文件。完整事件及附加字段保留在 native，Response、Steering、Error 和未知 Native 分开读取；公开 ResponsesStreamDecoder 接受已路由到该响应的 WebSocket 事件，复用 SSE 的输出、工具、usage 与历史聚合。纯协议入口不产生网络计时。

`client.withResponsesSession(model.endpoint)` 使用同一 `openResponsesSession` 入口的缺省设置管理持久连接。`session.create(model, request)` 和 `session.steer(model, responseId, input)` 返回实际发送时间；`session.receive()` 返回带实际接收时间的完整事件。记录可序列化，elapsed 相对于 session.startedAt，同一 endpoint 的多个模型可共享连接。应用用单个接收循环按实际 streamId／response ID 交给各自的 decoder，服务端错误和 steering 通知保持原文，不猜测请求归属。握手的认证、重试、计时及完成回调通过 handshakeOptions 设置，与生成消息的计时分开。会话随所属 client 关闭，借入 HTTP client 的所有权不变；示例见 [持久 Responses 会话](../../../docs/llm-client/USAGE.md#75-responses-持久连接与消息时间戳)。

`ClientToolCallReference.id` 是可选的服务端 ID，缺失时保持 `None`，不使用工具名或生成的字符串代替。应用回传工具结果时直接复用 reference；Chat、Messages 和 Responses 仍要求非空 ID，并在请求编码时检查。已保存的字符串 ID 可直接恢复，原生 context 与 ID 是否存在无关。

Gemini generateContent 使用 `Providers.geminiGenerateContent()` 和相同的 `client.generate`／`client.openStream`。模型 ID 用于 URL，最小正文只有 contents；认证由应用传入 `LlmAuth.header("x-goog-api-key", apiKey)`，库不读取环境。`GenerateContentProtocol(GeminiGenerateContentDialect())` 也提供纯 encodeRequest／decodeResponse／createStreamDecoder。消息保持多候选、文本／thought／媒体顺序及完整签名历史；工具结果复用 reference，连续结果组成一个 user Content，不补造缺失的调用 ID。服务端工具使用 WebSearch 或同 dialect 的 Native 声明。

generateContent 引用使用 UTF-8 字节范围；usage 分开统计工具输入与思考输出，未报告明细保持未知。CONTINUATION 的部分结果保留原生 token，不提供可直接追加的 history。显式 reasoning 预算和 level、JSON 输出及原生配置按对应字段编码；request.extensions 可补充 generationConfig.thinkingConfig.includeThoughts、toolConfig.includeServerSideToolInvocations 等嵌套选项，不能覆盖通用管理字段。

Gemini SSE 使用 :streamGenerateContent?alt=sse，正文沿用普通请求。finishReason 完成一个候选，EOF 才冻结整次调用的 usage；缺失停止原因的截断流仍失败。空文本签名、原始 Content 历史、累计 grounding 来源和尾部引用均保留，EventsOnly 不累计普通文本。函数调用等停止原因明确后才确定执行资格；Vertex 的 partialArgs 扩展保留 Native，不当作完整的 Gemini 函数调用。

Interactions 普通生成使用 `Providers.geminiInteractions()`，缺省部署为 Google v1，同样通过 endpoint／model／generate 调用；`InteractionsProtocol(GeminiInteractionsDialect())` 提供纯 encodeRequest／decodeResponse／createStreamDecoder。最小请求只有 model 和有序 input steps，不自动选择模型、保存配置或继续调用。媒体、函数结果、签名历史与服务端工具均使用 Interactions 自身的字段；普通请求的 options 缺省省略。

Interactions 的独立 step 格式保留 thought.signature、媒体、服务端工具、UTF-8 引用及原始 steps 历史；只有 requires_action 中未收到结果的完整函数调用进入待执行视图。进行中资源不提供可追加历史，服务时间不代替实际调用计时。缺失 token 保持未知，工具提示细项不重复加入输入总量。request.extensions 可显式配置 store／previous_interaction_id、generation_config 的原生选项，以及与公共 JSON 输出约束组合的媒体 response_format。

`client.retrieveInteraction(endpoint, id)`、`cancelInteraction` 和 `deleteInteraction` 显式操作已有资源。读取／取消返回 LlmResult，tools 缺省为空且只用于本地关联；缺失 steps 不产生伪造历史。删除返回 InteractionsDeletionResult，保存实际空正文或其他原始字节及 HTTP metadata。查询到的历史 usage 不成为本次操作的生成速率；见[使用示例](../../../docs/llm-client/USAGE.md#79-interactions-的读取取消与删除)。GET 事件流通过 openInteractionEvents／withInteractionEvents 显式读取，lastEventId 由实际交付事件更新；应用决定断点接续，不自动重连。

Interactions SSE 沿用 `openStream`／`withStream` 和缺省 EventsOnly，显式 Result 才保留最终结果。stream=true 与 alt=sse 由协议编码；有序 step 绑定文本、媒体、引用、工具和签名。step.stop 不开放客户端工具执行，只有最终 requires_action 决定待执行调用；interaction.completed 冻结 usage。最终资源可以省略服务时间，缺失值不会替换成客户端发送／接收时刻；未知增量保留 Native，无法完整重建时不提供可追加历史。

Bedrock Converse 通过 `Providers.bedrockConverse()` 接入同一个 `generate` 入口。部署地址、模型 ID 和认证显式配置，provider 不猜 region；API key 使用 `LlmAuth.bearer`。最小请求仅有 messages，模型 ID 保持 URL 中的一个路径段；推理参数、结构化输出、函数和系统工具各用协议原生字段。原始历史与连续工具结果可直接保存后接续，媒体 Bytes／S3 引用不触发隐式下载。AWS 凭据使用 `LlmAuth.awsSigV4(region, credentials)`，credentials 是返回 `AwsSigV4Credentials` 的 suspend 函数，每次尝试重新读取；service 缺省 bedrock。签名覆盖实际 URL、headers 和 body，不查找环境凭据；普通和 EventStream 调用共用此入口。该认证按官方文档与本地 mock 验证，真实 AWS 互通待补。纯响应入口 `ConverseProtocol(BedrockConverseDialect()).decodeResponse(json, request)` 同样可用。响应保存有序内容、reasoning 签名、媒体、引用、实际调用 ID 与原始消息历史；系统工具与客户端函数分别读取，不自动执行。源文档位置保持引用元数据，不转换成输出文本范围。usage 的普通输入、缓存读与缓存写全部已知时才计算总输入，服务端 latencyMs 只留在原文，不代替客户端时间戳。

Cohere Chat v2 使用 `Providers.cohereChat()` 和同一 endpoint／model／generate／openStream 入口，缺省部署为 https://api.cohere.com/v2，最小正文仅有 model 和 messages，流式自动增加 stream=true。文本、图片、thinking 控制、函数声明及原始历史按 v2 形状编码；RAG documents 通过请求扩展显式提供。函数 strict 统一映射全局 strict_tools，冲突值拒绝；Specific 只用于已仅声明目标函数的请求，不删除其他工具。工具结果复用原始 reference，普通错误作为应用提供的结果内容传入。

纯入口 `CohereChatProtocol(CohereChatDialect())` 同时提供 encodeRequest／decodeResponse。响应中的文本、thinking、tool_plan、引用和并行函数调用保持原有顺序；只有 TOOL_CALL 中完整且能关联声明的函数进入待执行视图。原始 assistant message 可保存后接续，未明确单位的引用位置保留原文，不猜测文本范围。usage.tokens 作为实际 token 统计，精确读取整数值的小数／指数写法；billed_units 留在 usage.native，不能替代实际 token 或金额。纯 decoder 不产生网络时间戳。

Cohere SSE 也可通过 createStreamDecoder(request) 单独消费，缺省 EventsOnly 不累计普通正文，显式 Result 保存最终结果与原始历史。文本、thinking、plan、并行函数和晚到引用使用稳定 item／part 位置；函数参数在 message-end 确认 TOOL_CALL 后才进入执行视图。缺失 ID 保持 None，未知增量保留 Native 并阻止不完整历史回传；debug 仅作元数据，裸 EOF 不能冒充消息完成。网络调用沿用实际发送／接收时间、最终 usage 与关闭合同。

Ollama 原生 Chat 使用 `Providers.ollamaChat()` 和相同的 endpoint／model／generate／openStream 入口，缺省部署为 http://localhost:11434/api；认证显式使用 none 或部署所需的方式。普通生成总是发送 stream=false，其他选项保持服务端缺省。图片使用内联 Bytes，文本编码为单个 content；推理档位按模型定义原样发送，公共采样设置可与 extensions.options 中的 seed 等原生设置组合。函数结果使用原始 reference 的真实名称与可选 ID，保存后仍可接续。

纯入口 `OllamaChatProtocol(OllamaChatDialect())` 同时提供 encodeRequest／decodeResponse。响应保存 thinking、普通文本、并行函数和原始 assistant 历史，未知消息字段保留 Native。工具 ID 可缺失，不用 index 生成 ID；只有 done=true、stop 且参数完整有效的声明函数进入待执行视图。空／未知停止原因保持未完成，load／unload 不生成 assistant 接续历史。输入总量包含缓存 token，只有明确报告缓存量时才计算普通输入；created_at 与原生 duration 是服务端数据，不冒充本地计时。

Ollama NDJSON 继续使用相同的流式保留与关闭合同，createNdjsonDecoder(request) 也缺省 EventsOnly。文本、thinking 和工具位置保持稳定，函数等最终 stop 才进入执行视图；累计 usage 替换已报告字段，缺失／null 不清除此前计数。终止记录的 created_at 与本地收发时间分开。空 messages 的 load／unload 若返回单个 application/json，也按同一 decoder 解释；未知消息变化不能推测合并时，保留 Native 并令历史不可用。

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

`metadata.attempts` 中的 `http` 保存实际排池、DNS、TCP、代理和 TLS 阶段。LlmPhaseTiming 的 started／finished 是相对逻辑调用起点的单调偏移，duration 在阶段结束时可读；它们可通过现有 companion codec 保存。未经过的阶段保持 None，连接复用不会产生虚构的建连耗时，失败和取消仍保留已有阶段。平均输出速率只在 usage 为 Final 且完整响应耗时为正时提供。

`client.countTokens(model, request)` 使用 Gemini GenerateContent、Anthropic Messages 或 OpenAI Responses dialect 的服务端计数接口，返回独立的 `LlmTokenCountResult`。count 保存输入计数、方法、范围、精确／估算性质及原文；Responses 标为 Exact，另两者为 Estimated。metadata 沿用真实收发时间、HTTP 阶段、重试和完成回调。普通生成不自动预先计数，预计数不写入 response.usage。没有该接口或显式不支持的模型在认证前失败；未提供 tokenizer 实现，不增加空的本地模式。Responses 保留输入、工具和显式历史引用，省略生成控制，未映射的根输入字段明确报错；DeepSeek Responses 不继承 OpenAI 的计数操作。Messages 的不支持输入明确报错，Gemini 的 cachedInputTokens 只表示缓存输入规模。

`client.retrieveResponse(endpoint, responseId)` 读取服务端 Responses 快照，`cancelResponse` 取消 background response，`deleteResponse` 删除存储的 response。读取与取消返回 LlmResult；tools 缺省为空，只用于本地关联；include 可显式请求加密 reasoning 等字段。删除的 ResponsesDeletionResult 保存实际确认与计时。三者使用原有 LlmCallOptions，不自动轮询或调度；历史 usage 不成为查询操作的生成速度。OpenAI Responses 提供这些操作，DeepSeek 在认证前拒绝；见 [使用示例](../../../docs/llm-client/USAGE.md#76-responses-的读取取消与删除)。

`client.compactResponse(model, request)` 显式压缩 OpenAI Responses 的上下文，返回可保存的 ResponsesCompactionResult。`toInput()` 保留完整压缩窗口，用于替换旧 input 后继续生成；system、tools 和生成 options 在下一轮显式提供。compaction.usage 与 metadata 保存此次压缩报告的用量和实际收发时间，原生附加信息保持 as-is。压缩请求复用 input、system 与适用的 extensions，tools／options 不发送到压缩端点；见 [使用示例](../../../docs/llm-client/USAGE.md#77-responses-的显式上下文压缩)。

LlmEndpoint 使用调用方配置好的 API 地址、headers 和认证。路径前缀与 query 原样参与协议请求，不管理云部署。认证可使用静态 key，或 `LlmAuth.bearer(suspend { acquireAccessToken() })`／`LlmAuth.header("api-key", suspend { acquireApiKey() })` 按尝试取得凭据；见 [端点用法](../../../docs/llm-client/USAGE.md#78-显式-api-地址与认证回调)。

最小请求的写法直接使用消息和设置的缺省值；添加服务端搜索或结构化输出仍使用同一个请求类型：

```scoop
import scoop.json.Json
import scoop.llm.client.*

val request = LlmRequest([Message([Text("用一句话解释值类型。")])])
val search = LlmRequest(
    input = request.input,
    tools = [Server(ToolKey("web"), WebSearch())],
    options = LlmGenerationOptions(maxOutputTokens = Some(1024UL))
)
val saved = Json.encodeToString(search)
val restored = Json.decodeFromString<LlmRequest>(saved)
```

Message 默认 User，system／tools／extensions 默认空，生成参数默认省略。工具区分 ClientFunction、ClientCustom、ClientNative 与 Server；供应商规定格式的工具也可能需要应用执行。请求级、消息级、内容块级与工具级的扩展各自带 DialectId，作用于当前位置。保存格式保留原生 JSON 和媒体来源；服务端工具执行观察随相应 adapter 交付。

`LlmResponse` 保留候选和有序输出；text 只展示普通文本，clientToolCalls 只返回输入完整且具有协议形状的客户端调用。`decodeOutput<T>()` 使用 companion decoder，拒绝、不完整输出、候选／文本消息歧义均明确报错。多候选按服务器 index 选择，消息按 items 下标选择。`toInput()` 读取已保存的原生历史，不用展示文本替换 reasoning 或工具关联。

`ChatProtocol(OpenAiChatDialect())` 与 `ChatProtocol(DeepSeekChatDialect())` 的 `decodeResponse(json, request)` 解释完整 Chat 响应。DeepSeek reasoning 单独展示并完整进入历史；音频历史仅回传 ID。原始响应、未知字段和不能关联的工具调用保持可读。模型生成的无效 JSON 参数保存为 Invalid；截断输出中的调用保存为 Incomplete；两者均不进入待执行视图。外层格式错误使用带字段路径和原始正文的 LlmProtocolException。

`Providers.anthropicMessages()` 使用 Messages protocol、Anthropic dialect、API 版本 2026-10-21 和 4096 的默认输出上限。请求显式 maxOutputTokens 覆盖该上限，模型已知能力也会检查默认值；需要旧协议版本时显式使用 `version = AnthropicMessagesVersion.V20230601`。新版本已移除 temperature／topP／topK。部署地址通过同一个 endpoint 的 baseUrl 参数设置，模型 ID 原样发送：

```scoop
val model = LlmEndpoint(Providers.anthropicMessages(), LlmAuth.bearer(apiKey)).model(modelId)
val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
```

Messages 支持图片、PDF、纯文本文档、function／原生工具及服务端 WebSearch。thinking、客户端调用、搜索活动和引用按 block 顺序保存；搜索开始与结果是共享同一服务器 ID 的两条活动。history 完整保留签名、加密搜索内容和未知字段，连续 ToolResult 合并为一条 user 消息。pause_turn 保持不完整状态，由应用决定下一步；搜索错误不会冒充整个 HTTP 请求失败。`MessagesProtocol(AnthropicMessagesDialect()).decodeResponse(json, request)` 也可单独解释完整响应，流式生成直接使用相同的 openStream／withStream。

同一 ChatProtocol 的 `createStreamDecoder(request, retention = StreamRetention.EventsOnly)` 接收 SSE parser 已定界的 `accept(event, data)`，返回带候选／item 位置的文本、reasoning、工具参数、完成调用和 usage 事件。缺省不累计普通文本；`StreamRetention.Result` 才保留完整结果与历史。正常 `[DONE]` 后可调用 `response()`，在输入结束时使用 `endInput()` 检查缺失终止。候选的 finish_reason 不等同整个流结束，usage 尾块仍会处理；EOF、服务错误或终止后新增数据均不能产生成功结果。

工具参数只有完成后才解释为可执行调用；无效或截断输入仍可观察。usage 是累计快照，只在正常 `[DONE]` 后标 Final，不把早到的候选结束当作总量已确定。未知片段保存在 Native 事件，Result 也通过 `nativeEvents` 保留必要片段；无法无损重建的消息不提供 history。这个 decoder 不执行网络 I/O；网络读取由 `LlmClient` 管理。

MessagesProtocol 提供相同的 createStreamDecoder 入口，按 content block 的 index 归并文本、thinking、签名、工具输入和引用，交付独立的 Citation 与 ServerToolActivity 事件。客户端参数在 block_stop 后仍等待最终 stop_reason，再决定完整性与执行资格；message_delta 的 usage 继续合并，到 message_stop 才为 Final。ping 不占用首个模型事件时间。未知 block 可原样保留；不能归并的未知 delta 或损坏 JSON 参数使 history 明确不可用，不把不完整消息冒充可回传历史。

ResponsesProtocol 同样提供 createStreamDecoder。文本、reasoning、工具输入、服务端活动与引用按 output_index／item_id 关联；done 快照不会重复交付已有文本，客户端调用等整轮终止后才确定执行资格。Result 保存完整 item 快照，尤其 reasoning 的 encrypted_content 与工具 context；EventsOnly 只记录已交付位置。response.completed／incomplete／failed 才结束流并冻结 usage，普通 EOF 与 error 事件仍报错；failed 是模型结果，可保存其不完整输出。未知片段保留 Native，最终完整快照用于恢复 history。

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

`stream.metadata` 每次取得不可变快照：HTTP 实际发送／响应头、首个事件／输出／文本和协议完成点分别记录。收到协议终止后停止 total 截止；资源清理后记录 finished 并执行一次 onFinished，此后 result 才可用。maxResponseBytes 同时约束流式 HTTP 正文的已读取字节，包含对应的 SSE／EventStream framing 和有限读取缓冲；不累计整条事件日志。

Converse 使用相同的流式入口与保留设置。协议将请求发送到 converse-stream，HTTP source 验证二进制 EventStream 的 CRC 与帧边界，业务 decoder 聚合文本、reasoning 签名、图片、引用和工具；messageStop 后继续读取 metadata，正常 EOF 才冻结 usage。服务端工具活动不会进入客户端待执行列表，无法归并的未知增量保留 Native 并使 history 不可用。纯入口是 `ConverseProtocol(BedrockConverseDialect()).createEventStreamDecoder(request)`，accept 接收已验证的 AwsEventStreamMessage；见 [ConverseStream 规范](../../../docs/specs/SCOOP-LLM-CONVERSE-STREAM-SPEC.md)。

自定义流式 decoder 的 `endInput()` 返回同样的 `LlmStreamUpdate`，可在 EOF 时交付最后的事件和 usage；成功返回时必须已经 `isFinished`。这些事件交付后才返回读取结束，最终结果、completed 时间和截止取消沿用正常事件的路径。EOF 本身不产生 firstEvent 时间。Chat、Messages、Responses 仍要求各自的显式终止事件，缺失时报告协议错误。

同一个 protocol 的 `encodeRequest(modelId, request, stream = false)` 生成 wire JSON；它本身不执行网络请求。Chat 最小文本只发送 model 和 messages，默认不设置温度、reasoning、token 上限或输出格式；Messages 另有必需的 max_tokens 默认值。各 dialect 处理自身的媒体形式、生成参数、工具和历史，不按 model ID 切换字段。OpenAI Chat 的 Server(WebSearch) 映射请求级 web_search_options，无法表达的选项抛出带输入路径的 LlmRequestException。

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

具体 dialect 按供应商明确的口径归一化：Anthropic 的 input_tokens 仅表示非缓存部分；三个输入部分齐全时才相加，再与已知 output_tokens 计算 totalTokens。缺失缓存细项时保留未知总量，搜索次数和 TTL 明细保留在 usage.native。服务商附加信息全部保持原生 JSON，不进入公共数据模型或生成流程；具体 dialect 按需访问原文，不校验或换算其内容。缓存附加信息同样仅原样保留，不估计寿命或判断有效性。

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
        Supported(MediaLimits(maxBytesPerItem = Some(1048576UL)))),
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

MIME 参数也参与格式匹配；支持音频 `rate=24000` 不自动表示支持其他采样率。模型和媒体上限、topK、maxOutputTokens、candidates 与推理 budgetTokens 使用 `ULong`；缺失值使用 None，已知零上限仍保留。生成入口会拒绝已知不支持的输入和显式控制，并检查可观察的媒体字节数、数量及生成上限。Unknown 继续交给 adapter／服务端判断，不联网下载媒体或猜测隐藏 token 数。

以上是函数内的调用片段；应用直接使用 JSON 时也应声明 `scoop:scoop.json` 依赖。Codec 使用现有 companion 的缺省实现，Long 大整数在保存与恢复时保持精确。正式合同见 [LLM client 规范](../../../docs/specs/SCOOP-LLM-CLIENT-SPEC.md)。

原生工具与服务端工具也有独立的能力描述；已知 WebSearch 可用但具体选项未知时，只需要填写工具本身：

```scoop
val capabilities = ModelCapabilities(
    serverTools = ServerToolCapabilities(webSearch = Supported(WebSearchCapabilities())),
    mixedTools = MixedToolCapabilities(clientAndServer = Unsupported("Use separate requests"))
)
val model = endpoint.model(modelId, Some(capabilities))
```

`clientNativeTools` 与 `serverTools.nativeTools` 各自保存精确的 dialect／kind／version 表，分别通过 `forClientNativeTool` 和 `serverTools.forNativeTool` 查询。version=None 只表示未显式选择版本；没有列出的版本或配置字段仍为 Unknown。原生配置的字段支持、WebSearch 的域名／地域／来源选项、客户端与服务端混用限制都会在 generate 和 openStream 的同一请求检查中生效，报错保留输入路径。显式空集合、false 和 null 不被当成省略；能力 Supported 也不自动填入请求选项或启用 adapter 无法表达的功能。
