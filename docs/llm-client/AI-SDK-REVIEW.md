# Vercel AI SDK 对照评审

日期：2026-10-24。依据：AI SDK 官方 v7 文档，以及 `vercel/ai` 的固定提交 [`e729d588b3291e22cfcd6cba32b6dcf7778ef21c`](https://github.com/vercel/ai/commit/e729d588b3291e22cfcd6cba32b6dcf7778ef21c)。该快照的 [`ai` package 版本](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/ai/package.json)为 7.0.152，provider 主接口为 V4；这里记录源码与文档观察，不把 main 快照当作已做互操作验收的发行包。本轮没有安装或运行 AI SDK。

结论：值得借鉴的是完整结果的数据视图、按位置附着的方言选项、原生工具的定义／执行区分，以及结构化输出与普通生成共用调用路径。Scoop 的 protocol／dialect／provider 分层、显式能力、单次生成职责和资源作用域继续保留。具体调整已并入[架构设计](DESIGN.md)和[使用示例](USAGE.md)。

## 1. 从同一个完整接口自然产生简单用法

AI SDK 的 `generateText`／`streamText` 接收 model、输入与可选设置，并返回包含内容、工具、来源和 usage 的结果；名字里的 Text 不表示只有一个 String 返回值。结构化生成也使用同一调用的 output 设置。[生成接口](https://ai-sdk.dev/docs/ai-sdk-core/generating-text)、[结构化输出](https://ai-sdk.dev/docs/ai-sdk-core/generating-structured-data)

Scoop 采用同一原则：仍是 `generate(model, request, options = ...)`，缺省字段形成最小调用。`result.text` 是完整内容的读取视图，不另设返回 String 的调用链；生成 JSON 只改变请求的 output 设置。既有 client 作用域不会因为输入简单就被隐藏创建到每次生成中。

AI SDK 同时支持 prompt 与 messages，并允许文本 content 使用字符串或 parts。我们保留一种类型化的 input 表示，用默认 User role、空 tools 和缺省 options 降低负担；不为对齐 JavaScript 调用字数再建立互斥的 prompt／messages 字段或字符串转换规则。[输入表示](https://ai-sdk.dev/docs/foundations/prompts)

## 2. 最需要补齐：定义来源与执行方是两个维度

固定源码中的 provider tool 表示供应商规定的 schema／原生配置；它并不自动意味着服务端执行。高层类型进一步区分 `ProviderDefinedTool` 与 `ProviderExecutedTool`，响应 tool call 也记录 `providerExecuted`。前者的具体例子是供应商定义输入输出格式、由应用执行的 shell 工具。[原生工具表示](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider/src/language-model/v4/language-model-v4-provider-tool.ts)、[工具类型与执行方](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider-utils/src/types/tool.ts#L280)、[响应工具调用](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider/src/language-model/v4/language-model-v4-tool-call.ts)

原来的 ClientFunction／ClientCustom／Server 缺少这个实际组合。本轮增加 ClientNative，使用四个有明确含义的变体：

| 定义 | 格式由谁规定 | 谁执行 |
| --- | --- | --- |
| ClientFunction | 应用声明 JSON Schema | 应用 |
| ClientCustom | 应用声明文本／语法合同 | 应用 |
| ClientNative | Dialect 适配供应商的原生工具合同 | 应用 |
| Server | Dialect 适配供应商的服务端工具合同 | 服务端 |

ClientNative 保存 ToolKey、原生工具种类／版本和配置，进入客户端待执行视图；不能假装成自定义 function schema，也不能进入 ServerToolActivity。执行方由具体协议事实与工具声明确定，不允许调用者用一个 Boolean 把任意原生工具切换到另一执行方。库仍不接收 execute 函数，不执行这些工具。

AI SDK 的执行、审批与多步循环属于更高一层。保留它们的输入／输出区分有价值，但这些调度行为不进入本次 LLM client 范围。[工具调用与执行](https://ai-sdk.dev/docs/ai-sdk-core/tools-and-tool-calling)

## 3. 方言扩展要附着在实际有语义的位置

AI SDK 在调用、消息和 part 上支持 providerOptions；源码的文本、reasoning、file 等 part 分别保存这些信息。缓存断点就是必须定位到某条消息或内容块的例子，单个 request 级配置无法准确表达。[选项作用位置](https://ai-sdk.dev/docs/foundations/prompts#provider-options)、[Prompt 数据定义](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider/src/language-model/v4/language-model-v4-prompt.ts)

Scoop 将 `extensions: Array<DialectExtension> = []` 明确放在 request、message、content part 和工具定义的适用位置。DialectExtension 携带实际 dialect 归属及不可变 JsonValue；常用配置可以由该 dialect 的普通类型化构造函数产生。命名空间按语义归属，不按可改名的 provider 展示名或网关域名推断。

这些是不同位置的协议数据，不是四层生成参数继承。父节点配置不自动复制到子节点，同名字段不按覆盖优先级偷偷合并；编码时由适配器检查该位置、当前模式和管理字段冲突。输出原生数据继续由不可变历史表示保存，不能把任意 output metadata 全量塞回下一轮请求。

## 4. 借鉴流的读取视图，保持单一消费者

AI SDK 提供 textStream 与完整事件流视图。固定源码中的普通 streamText 默认保留 replay 和最终内容，部分最终结果 getter 会主动消费流；另有共用执行引擎的 streamTextLowMemory。它们说明“读取视图”和“保留策略”必须分别定义。[结果读取接口](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/ai/src/generate-text/stream-text-result.ts)、[保留策略与低内存调用](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/ai/src/generate-text/stream-text.ts#L375)

Scoop 增加同一流上的 `textDeltas()` 投影视图，替换展示代码里反复出现的 TextDelta 分支。它的 next 读取同一 reducer，只交付普通文本 delta；工具、reasoning 和 usage 仍被处理，错误与取消仍传播。Delta 保留 candidate／item／part 关联，不暗中合并多个候选。

每个流只选择一个消费者：原始 next 或文本投影。投影不复制流、不保存 replay、不隐式切换 Result 保留策略，也不会因为只看文本就提前结束。完整结果仍在显式消费与关闭后读取；读取 result 本身不执行 I/O。EventsOnly／Result 继续作为同一入口的配置，不另设低内存生成入口。

## 5. 结构化输出共用生成、流式与工具合同

AI SDK 通过 output 描述所需结构，再解析／校验结果，结构化输出可与工具使用组合。这个共用调用路径的方向适合 Scoop，但 Zod 一类 schema 与类型推导机制不应直接变成 Scoop 编译器规则。[Output 文档](https://ai-sdk.dev/docs/ai-sdk-core/generating-structured-data)

具体化为 `LlmGenerationOptions.output`：缺省 Text，另外有 JsonObject 和 JsonSchema。Schema 控制发给服务端的约束；完整结果上的 `decodeOutput<T>(codec)` 或 companion 缺省重载负责本地 DTO 解码。两者在同一示例中出现，但不会从任意 ValueDecoder 自动推导 Schema。

Refusal、输出不完整、选中的输出不存在或 JSON 无效须明确报告；原始结果与 metadata 仍由调用者持有。多个候选或多个结构化输出时须显式选择，不把它们拼成一个 JSON。第一轮不为了增量展示引入自动补 JSON 或完整 partial-object 类型系统。

## 6. Usage 与计时：借鉴分项，保留未知值

AI SDK 的 usage 同时保存 input total、no-cache、cache read/write、output text／reasoning 和 raw 信息。这支持我们已有的“原始用量＋归一化用量”，并提示补出明确的非缓存输入分项。[V4 usage](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider/src/language-model/v4/language-model-v4-usage.ts)

Scoop 增加 `nonCachedInputTokens`，只在原生报告或已知的完整、不重叠口径下给出。缺失仍是 None；cache／reasoning 子项不重复加到总量。SDK 的 step 聚合不能直接当作我们的 attempt 聚合：一次 Agent step、一次模型响应和一次 HTTP 重试仍是不同范围。

SDK 已区分整次请求速率、输出阶段速率、step 与 response 耗时、首个输出；它的首输出包括 reasoning、工具输入和文件，验证了我们将 firstOutput 与 firstText 分开的必要性。不过固定源码的速率 helper 在计数未知或时长无效时返回 0，Scoop 保留 None，不把未知误报成零速率。[性能口径](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/ai/src/generate-text/step-result.ts#L60)、[速率计算](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/ai/src/generate-text/calculate-tokens-per-second.ts)

V4 response timestamp 的合同是供应商返回的响应生成开始时间，不能替代客户端 sentAt／receivedAt，更不能统一当作缓存写入时刻。我们的 UTC／单调计时、每次尝试的时间戳和 dialect TTL 规则继续保留；计费也是独立可选扩展。[响应时间定义](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider/src/language-model/v4/language-model-v4-response-metadata.ts)

## 7. 保留现有边界的几项比较

| AI SDK 中的机制 | Scoop 的取舍 |
| --- | --- |
| Provider 工厂与统一 doGenerate／doStream | 借鉴适配器职责；继续把 protocol、dialect、endpoint 和 model 分开 |
| supportedUrls 按 MIME 声明原生 URL 支持 | 属于输入形式的一部分，不能替代模型完整 capability；媒体转换／下载仍由应用显式选择 |
| providerOptions 与 warnings | 保留有归属的扩展和诊断；调用者明确请求且无法表达的能力仍报错 |
| 工具名称作为集合 key、toolOrder 稳定排序 | 保留有序工具集合与稳定编码；不另加排序选项或自动改变调用者顺序 |
| maxRetries 缺省 2 | 数字与现有建议相同；仍以可重试事实为前提，不复制 SDK 的流中恢复或多步执行策略 |
| responseMessages 与模型输入消息 | 对应我们的 toInput；UI message、展示文本和协议历史保持不同职责 |

这些比较依据 [V4 model 接口](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/provider/src/language-model/v4/language-model-v4.ts)、[streamText 设置](https://github.com/vercel/ai/blob/e729d588b3291e22cfcd6cba32b6dcf7778ef21c/packages/ai/src/generate-text/stream-text.ts#L330)、[生成参数参考](https://ai-sdk.dev/docs/reference/ai-sdk-core/generate-text)。它们不是要求实现 provider registry、middleware 链、UI 协议或 Agent runtime 的依据。

本轮落到设计中的增量是 ClientNative、按位置的 dialect extensions、单消费者文本投影、结构化输出示例和非缓存输入统计。正式 API 与实现仍按原实施顺序推进。

## 8. Gemini SSE 实施时的补充对照

2026-10-25 核对当前 [Google adapter](https://github.com/vercel/ai/blob/main/packages/google/src/google-language-model.ts)：其 streamGenerateContent 使用独立 URL 和 alt=sse，并在流的 flush 中交付最终 usage。Scoop 将这个需要落实到公共 endInput 更新合同，EOF 的结束事件、计时和结果保留沿用同一条网络处理路径；不为 Gemini 另建读取入口。

这个 adapter 的首候选处理、缺失调用 ID 时生成 ID，以及 CONTINUATION 的自动继续不直接移植。Scoop 保留实际候选 index 和可选服务器 ID，继续请求由应用显式决定；usage 也保留未报告计数的 None。纯文本与媒体按普通响应分组，使同一回答的多个 part 不影响默认结构化解码。

签名即使附着在空文本上也需要保存。历史按 [Google SDK 的流式 Content 顺序](https://github.com/googleapis/js-genai/blob/main/src/chats.ts)保存，不用展示文本的拼接替代；[官方签名规则](https://ai.google.dev/gemini-api/docs/generate-content/thinking#thought-signatures)要求保持签名 part 的边界。[FunctionCall 文档](https://googleapis.github.io/js-genai/release_docs/interfaces/types.FunctionCall.html)将 partialArgs／willContinue 标为 Vertex 扩展，因此当前 Gemini 解释器保留这些原文，不把未完成参数误判为无参数调用。
