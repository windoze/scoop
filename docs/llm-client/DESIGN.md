# LLM client 架构设计

日期：2026-10-24。状态：架构设计，尚未实施。本文确定 `scoop.llm.client` 的协议、方言、provider、模型能力、服务端工具、计时与 token 统计边界；计费信息作为 dialect 的可选扩展。协议覆盖调查见 [INVESTIGATION.md](INVESTIGATION.md)，传输细节见 [HTTP client 评审](HTTP-CLIENT-REVIEW.md)，具体调用与缺省参数见[使用示例](USAGE.md)。相关架构表述以本文为准，正式 API 签名在实现前写入库规范。

本库负责一次生成或显式会话的请求、响应、流、取消和协议数据回传。模型选择、自动切换、工具执行、Agent 循环和会话存储由应用承担。所有实现使用普通 Scoop Cone、现有序列化和 HTTP 能力，不改变语言、编译 pipeline 或 runtime。

[Vercel AI SDK 对照评审](AI-SDK-REVIEW.md)补充了原生客户端工具、按位置的方言扩展、流的读取视图和结构化输出用法；下文已纳入这些调整。

## 1. 核心对象与关系

| 概念 | 定义与职责 | 示例 |
| --- | --- | --- |
| API protocol | 请求／响应结构、操作、事件、定界方式与完成语义 | Chat Completions、Responses、Messages、Gemini Interactions、generateContent |
| API dialect | 在一个 protocol 内，处理服务特有的字段、选项、历史回传、工具及错误差异 | DeepSeek Chat 的 `reasoning_content` 规则 |
| Provider | 固定包装一组 protocol＋dialect，提供可命名、可复用的 API 适配 | `openai-chat`、`openai-responses`、`deepseek-chat` |
| Endpoint | 将 provider 部署到实际 URL、路径／query、headers 和认证环境 | 官方服务、Azure 部署、团队网关、本地服务 |
| Model | 在一个 endpoint 上使用某个 model ID 的绑定，并携带该入口下的能力描述 | 同一模型通过两个 provider 使用，形成两个独立绑定 |
| Capability | 描述该模型绑定能够使用的输入、输出、工具和请求特性 | PDF 输入、音频输出、JSON Schema、服务端 web search |

商业供应商名称只是展示和分组信息，不决定 API protocol。Provider 不等于供应商、域名、账号或模型；API key 不进入模型能力或可保存的消息数据。

关系固定为：`Provider = protocol + dialect`；`Endpoint = provider + deployment + authentication`；`Model = endpoint + model ID + capabilities`。Client 持有 HTTP 等资源，调用时接收 Model 和 LlmRequest，不在 request 内再放一份可能冲突的 provider／model 路由。

### 1.1 一个 provider 固定一个 protocol

| Provider | API protocol | API dialect |
| --- | --- | --- |
| `openai-chat` | Chat Completions | OpenAI Chat |
| `openai-responses` | Responses | OpenAI Responses |
| `deepseek-chat` | Chat Completions | DeepSeek Chat |
| `deepseek-responses` | Responses | DeepSeek Responses |
| `anthropic-messages` | Messages | Anthropic Messages |
| `gemini-interactions` | Gemini Interactions | Gemini Interactions |
| `gemini-generate-content` | generateContent | Gemini generateContent |

这些是命名与组合示例，不表示全部已实现。通用兼容入口也使用一个明确的基础 dialect；只有 URL 不同且语义相同的部署复用 provider，不为每个域名复制适配器。

同一供应商通过不同 protocol 提供模型时，暴露多个 provider。若某模型只支持 Responses，就只在相应入口声明支持；若同时支持 Chat 和 Responses，它可以同时出现在两个 provider 下，各自拥有不同的有效 capability。Provider 内部不得根据 model 名称、年代或失败响应自动改换 protocol。

模型年代不作为 API 可用性的规则：OpenAI 官方迁移文档仍展示同一模型通过两种 API 调用，并说明两者的功能差异。模型与入口的具体兼容性由相应描述和服务器决定。[OpenAI 协议对比](https://developers.openai.com/api/docs/guides/migrate-to-responses)

默认 model catalog 只是便利数据。未知模型 ID、第三方 provider 名称和自定义 endpoint 可直接使用；`listModels` 是显式操作，生成前不强制联网查询目录。模型 ID 按原字符串传给部署规则，不拆解斜杠、冒号或 ARN。

### 1.2 配置与调用形态

以下为拟议调用形态，不是已存在的 API：

```scoop
val endpoint = LlmEndpoint(
    provider = Providers.deepSeekChat(),
    authentication = LlmAuth.bearer(apiKey)
)
val model = endpoint.model("deployment-model-id")

withLlmClient suspend { client ->
    val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
    println(result.text)
}
```

普通消息默认 User，request 的 system／tools 为空，生成选项默认省略。简单调用与复杂调用共用 `client.generate(model, request, options = LlmCallOptions())`，不提供 prompt 重载或另一种文本专用 client。`Message`／`Text` 是普通 enum 变体的类型上下文短写。生成参数只在 request 中表达，调用选项管理等待、重试与观察；常用值由应用直接复用，不建立多层隐式生成参数合并。

`endpoint.model(id)` 允许省略能力描述，此时未掌握的能力为 Unknown；显式描述使用 `capabilities = Some(description)`。Endpoint 未传 baseUrl 时采用 provider 的已知部署缺省，覆盖用 `baseUrl = Some(url)`；没有部署缺省的 provider 必须指定 URL。认证与 model ID 必填，不从环境中隐式查找凭据或选择模型。

流式请求使用 `client.withStream(model, request) suspend { stream -> ... }`；需要关闭后读取完整结果时使用同参数的 `openStream` 加 `useAsync`，并显式设置 `retention = StreamRetention.Result`。`withLlmClient`／`withStream` 只组合资源寿命，不另设生成语义。切换 provider 必须由应用显式选择另一个 Model，原生历史是否可迁移按第 5 节处理。

命名工厂提供常用组合与部署缺省值，公共构造入口仍允许用户提供自己的适配器。认证与连接池不按 provider 复制；多个 endpoint 可共享一个 HttpClient，借入资源的关闭责任沿用既有合同。

## 2. Protocol 与 dialect 的职责

### 2.1 Protocol 维护基础线协议

Protocol 实现请求和结果的 wire DTO、完整响应解码、流事件解析与聚合、ID／index 关联、终止判断，以及协议定义的 continuation。SSE、NDJSON、AWS EventStream、WebSocket 是传输和定界方式，不是区分商业供应商的依据。

例如 Chat adapter 负责 `messages`／`choices`、delta 与尾部 usage；Responses adapter 负责有序 item、类型化事件和响应终止。两个入口共享媒体、错误等实际相同的 helper，但不把一种协议的 DTO 发送到另一种协议。

普通完整响应和流式响应共用最终 item 的解释与归一化逻辑。流式状态留在每次调用的 reducer 中，不放在共享 provider 或 dialect 对象里；非流式响应无需先伪装成一串 delta。

### 2.2 Dialect 维护服务差异

| 差异 | Dialect 的处理位置 |
| --- | --- |
| 选项名、可用值、默认要求及模式组合 | 请求检查与 wire 映射 |
| reasoning、签名、缓存断点等原生字段 | 请求／响应 DTO 的扩展编码与解码 |
| assistant 历史与工具结果回传要求 | 历史投影和下一轮请求编码 |
| 工具 ID／名称限制及必要改写 | 调用与结果的同一显式关联映射 |
| 服务端工具种类、版本和参数 | 服务端工具映射与 capability 描述 |
| 错误正文、usage、停止原因的差异 | 响应解释和流式归一化 |
| Token 计数口径、缓存寿命及可选计费信息 | usage 归一化、缓存时间解释及 billing 扩展；本地计时由公共 client 负责 |

Protocol 使用协议专属的 dialect 接口，例如 Chat adapter 接受 `ChatDialect`，Responses adapter 接受 `ResponsesDialect`；provider 工厂完成这一普通类型组合。不能仅凭两个任意字符串拼接出可工作的适配器，也不把所有差异塞进一个接受 `Any` 的通用 hook。

一个 provider 只有一套确定的 dialect 行为。组合 helper 使用普通函数调用，不引入运行期中间件排序、插件下载或全局 provider registry。若差异改变了核心消息结构和状态转换，应作为另一个 protocol，而不是无限扩张原 dialect。

扩展字段按 protocol／dialect 归属传入。它们与适配器管理的字段冲突时失败，包括本次因 None 而未输出的已知字段；不能靠扩展绕过通用选项和字段规则。完整 native request 入口明确使用独立的请求表示，不与通用请求进行隐式覆盖。

`extensions: Array<DialectExtension> = []` 在 request、message、content part 和工具定义的适用位置分别提供。每项保存实际 dialect 归属与不可变 JsonValue；同一位置的同一归属不得重复。缓存断点、媒体细节等必须放在其协议要求的位置。它们不构成父子配置继承，不把 request 的扩展自动复制到 part，也不按展示名或网关域名猜测归属。常用扩展可由 dialect 的普通类型化函数构造，适配器一次检查实际位置、模式和字段冲突。

### 2.3 DeepSeek Chat 示例

DeepSeek Chat 复用 Chat Completions 的基础结构，但需要自己的 dialect。当前官方 thinking 文档规定：请求携带 `tools` 时，后续请求须完整回传历史各轮的 `reasoning_content`，包括没有实际调用工具的轮次；不携带 `tools` 时规则不同。[DeepSeek thinking mode](https://api-docs.deepseek.com/guides/thinking_mode/)

因此该 dialect 必须：

1. 分别收集 `content` 与 `reasoning_content`，流中二者分别更新，不把 reasoning 合并到普通文本视图。
2. 保存 assistant 消息的 reasoning、工具调用及必要原生字段；重新编码历史时根据本次请求模式采用对应回传规则。
3. 工具结果按原 call ID 关联；需要回传而历史已丢失的数据明确报错，不从可见文本猜测或补造 reasoning。
4. 将模式相关的选项限制落实到 capability 查询与请求检查，不把所有 DeepSeek 模型统一标为“支持 Chat 的全部参数”。

这些规则只负责 API 数据适配，库不解释或验证 reasoning 内容。同一供应商的 Responses 接口使用另一组 protocol＋dialect；其历史规则不从 Chat 自动继承。

## 3. Model capability

### 3.1 能力状态与作用域

每项能力采用三个明确状态：`Supported(details)`、`Unsupported(reason)`、`Unknown`。Supported 表示已知可通过当前模型绑定使用该能力，details 保存必要格式和限制；Unknown 表示缺少信息，不能等同于不支持。

能力属于“模型＋provider＋部署入口”，不是供应商品牌的单个 Boolean。例如同一个模型在 Chat 与 Responses 下的服务端工具、结构化输出和 continuation 可以不同；网关也可能只开放其中一部分。

能力描述按调用者显式配置、入口提供的描述、精确匹配的内置模型资料取得；显式配置可更新过时的目录信息。它不能使未实现的协议编码路径变成已实现，也不能覆盖适配器掌握的具体请求矛盾。没有准确资料时保持 Unknown，不按模型前缀猜测。

### 3.2 输入与输出定义

| Capability | 精确定义 |
| --- | --- |
| `input.text` | 在请求内容中接受文本 |
| `input.image` | 接受图像内容；不由“支持文件”推断 |
| `input.pdf` | 接受原生 `application/pdf` 文档；不等同于应用先提取文本或渲染成图片 |
| `input.audio` | 接受音频内容；格式、采样率和声道约束由具体输入形式描述 |
| `input.video` | 接受视频内容；容器、时长等约束单独描述 |
| `input.videoFrames` | 接受带时间位置的帧序列；不自动等同于视频文件或音轨输入 |
| `input.document` | 接受 PDF 之外的指定文档 MIME 类型；不是任意文件通配许可 |
| `output.text` | 生成文本内容 |
| `output.image` | 在该生成入口生成图像 |
| `output.audio` | 在该生成入口生成音频，明确编码和格式 |
| `output.video` | 在该生成入口生成视频 |
| `output.document` | 在该生成入口生成指定格式文档，包括明确支持的 PDF |

输入能力同时描述实际可用形式：文本、内联 bytes、URL、provider file reference。媒体 kind、MIME／编码与输入形式按组合描述；“支持 PNG bytes 和 JPEG URL”不能被合并成“PNG／JPEG 均支持任意形式”。未列明的形式为 Unknown，只有明确否定才是 Unsupported。

能力细节可携带 API 实际规定的字节数、图片数、页数、时长、分辨率、采样率等限制，使用明确单位；未知上限不填写虚构数值。PDF 与图片数量、输入音频与输出音频分别判断。文件上传成功只说明资源接口可用，不说明所选模型能够理解这个文件。

输入编码由 adapter 将 bytes 转成协议要求的 base64 等表示。库不自动 OCR、转码、拆视频或降级为文本；调用者显式完成转换后，按转换后的输入重新判断能力。服务端工具产生的文件或媒体记录在工具结果中，不因此宣称基础模型具备对应的直接输入／输出能力。

### 3.3 生成与工具能力

| Capability | 内容 |
| --- | --- |
| `streaming` | 生成期间增量返回事件；不意味着接受实时输入 |
| `realtime` | 所选会话接口允许双向交互；与普通生成流分开 |
| `functionTools` | 声明客户端函数工具并返回 JSON 参数调用 |
| `customTools` | 声明客户端文本或语法约束工具 |
| `clientNativeTools` | 按原生种类、版本及选项描述供应商定义、应用执行的工具 |
| `parallelToolCalls` | 一轮可产生多个可并行处理的客户端调用 |
| `toolChoice` | 分别描述 Auto、None、Required、指定工具的支持情况 |
| `serverTools` | 按第 4 节的具体工具种类、版本及选项描述支持情况 |
| `mixedTools` | 客户端与服务端工具、多个服务端工具的实际组合支持情况 |
| `structuredOutput` | 分别描述 JSON object 与 JSON Schema；Schema 子集和 strict 模式另列 |
| `reasoning` | 请求 reasoning 模式及其可配置选项；可展示文本与回传数据仍分开 |
| `sampling` | 分别描述 temperature、top-p、top-k、stop 等选项 |
| `multipleCandidates` | 生成多个候选，保留 candidate 关联 |
| `continuation` | 显式服务端上下文继续方式，与完整历史输入分开 |
| `background` | 服务端生成任务的显式提交、查询与取消 |
| `tokenCounting` | 分别描述服务端计数接口与本地 tokenizer 的适用范围；不与响应 usage 混为一谈 |

Context window、输入／输出 token 上限等作为已知的模型限制描述。时间戳与调用计时是公共 client 能力，不以模型 capability 为开关；usage 是否完整由服务端实际返回决定，计费信息走第 6.5 节的可选 dialect 扩展。能力支持不自动启用相应选项，例如支持 reasoning 不等于每次显式发送 reasoning 设置。

### 3.4 有效能力与请求检查

对一个实际请求，需要同时满足：当前 adapter 能表达、当前模型／部署提供、当前模式和功能组合允许。任一项明确不支持则给出带输入位置和原因的错误；全部已知满足才是 Supported；仍有缺失信息则是 Unknown。

未知模型在 adapter 可以编码请求时允许发送，由服务器判断未知部分。不因为目录缺项拒绝请求，也不把原生扩展支持误称为所有通用能力均已实现。调用者明确要求但无法映射的字段必须失败，不静默删除、切换 provider 或用另一工具代替。

查询接口允许针对请求模式给出有效能力，例如 reasoning 模式下的 temperature 或特定 toolChoice；请求检查复用同一组普通规则。单独支持 JSON Schema、图片和 web search 不保证三者组合支持。组合规则由相关 dialect 的普通函数表达，只覆盖实际协议约束，不建立通用规则 DSL 或约束求解器。

请求归一化后检查一次；后续未变化的 DTO 不重复完整检查。schema 与 codec 是不同职责：Schema 约束服务端生成，`ValueDecoder<T>` 负责本地 typed 解码，自定义 codec 不自动产生相符的 Schema。

## 4. 服务端工具

### 4.1 与客户端工具共用请求入口

`LlmRequest.tools` 是有序的 `ToolDefinition` 集合，其变体区分 `ClientFunction`、`ClientCustom`、`ClientNative` 和 `Server`。各定义有请求内唯一的 ToolKey；ToolKey 是本地引用，不冒充 API 工具名或服务器 call ID。

| 定义 | 谁执行 | 配置与返回内容 |
| --- | --- | --- |
| ClientFunction | 应用 | name、description、JSON Schema；模型返回参数，应用提交结果 |
| ClientCustom | 应用 | 文本／语法描述；保留原始输入和应用结果 |
| ClientNative | 应用 | 供应商规定原生工具合同，dialect 映射种类／版本、配置和输入输出；应用接收调用并提交结果 |
| Server | 服务端 | 工具种类和配置；返回工具活动、结果、引用及原生数据 |

Server tool 不要求应用提供实现函数或伪造 function Schema。即使名称也叫 `web_search`，它与客户端同名函数仍是不同的定义；若某协议的 wire 命名空间无法容纳二者，adapter 在编码时明确报告冲突。

工具的定义来源与执行方是两个维度。供应商定义的 shell 等客户端原生工具使用 ClientNative，不能因为有 provider 原生 ID 就归为 Server，也不要求应用重写它的 function Schema。ClientNative 的原生描述与 ServerTool.Native 一样由具体 dialect 解释；执行方必须符合该工具的实际协议，不能通过一个自由 Boolean 切换。无法判断的原生活动保留 Native 数据，不猜成可执行的客户端调用。

ToolChoice 使用 Auto、None、Required、Specific(ToolKey)，其范围是本次声明的工具集合。Required 表示至少调用集合中的一个工具，不表示每个都调用；Specific 指定精确工具。协议不能表达所要求的选择或混合组合时失败，不将 Specific(Server) 改成普通函数调用。

例如一次请求可以同时声明本地查询与服务端搜索，拟议数据形态为：

```text
tools = [
    ClientFunction(key = ToolKey("local"), name = "lookup_local", parameters = lookupSchema),
    Server(key = ToolKey("web"), tool = WebSearch())
]
toolChoice = Auto
```

模型可以自行使用服务端搜索，也可以返回 `lookup_local` 调用交给应用。两者共享响应顺序、引用和 usage；应用的工具执行循环只处理客户端调用。选择 `Specific(ToolKey("web"))` 时由对应 provider 编码原生搜索选择，支持情况按模型和模式判断。

### 4.2 通用工具种类与原生配置

| ServerToolKind | 通用语义 | 保留的差异 |
| --- | --- | --- |
| WebSearch | 搜索公开网页信息，取得来源与引用 | 搜索选项、域名过滤、地域、原生版本、返回内容 |
| WebFetch | 读取指定网页或 URL 内容 | URL 限制、抽取内容、引用、服务端缓存 |
| FileSearch | 搜索服务端管理的文件集合 | 文件／索引引用、过滤和检索选项 |
| CodeExecution | 服务端执行其托管代码工具 | 环境／容器引用、输出、生成文件与执行状态 |
| ImageGeneration | 服务端生成或编辑图像 | 输入引用、尺寸／格式、预览与最终产物 |
| Native | 已选 protocol／dialect 的其他工具 | 原生工具类型、配置 JsonValue 和原生结果 |

这里定义共有语义，不要求第一轮实现所有种类。WebSearch 是首个需要多个供应商映射的完整例子。Native 配置仍由所选适配器处理其合法请求位置和事件，不靠全局注册表或运行期下载代码扩展。

`ServerTool.WebSearch(options)` 提供有明确定义的共同选项，例如域名允许／排除集合、地域和返回来源要求。字段默认省略，具体能力中分别说明支持情况；不能精确映射的显式选项报错。搜索次数、上下文大小等不等价的供应商参数留在类型化 provider 选项或按 dialect 归属的 JSON 扩展中，不能互相猜测换算。

不同 API 的搜索实现保留如下差异：

| Provider 形态 | 请求映射 | 结果映射 |
| --- | --- | --- |
| OpenAI Responses | 原生 `web_search` 工具配置 | 搜索 item／action、来源、最终文本引用；搜索不等于一个客户端 function call |
| Anthropic Messages | 带原生版本的 web search 定义 | `server_tool_use`、搜索结果 block、引用和可能的工具错误 |
| Gemini Interactions | `google_search` 工具配置 | 搜索 call／result step、文本 annotations，以及需交给应用展示的原生搜索信息 |

这三种形态分别来自官方 [OpenAI web search](https://developers.openai.com/api/docs/guides/tools-web-search)、[Claude web search](https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool)、[Gemini grounding](https://ai.google.dev/gemini-api/docs/google-search)。同供应商的另一 protocol 需要自己的映射，不自动复制此表。

### 4.3 事件与完整结果

输出的有序 item 同时容纳 Message、Reasoning、ClientToolCall、ServerToolActivity 和 Native 内容。两类执行方参与同一响应和事件流；待执行视图包含 ClientFunction／ClientCustom／ClientNative 中输入完整且满足协议输入形状的调用，不包含服务端活动或参数损坏的调用。应用自己的 DTO／业务约束仍由应用解码和检查。

ServerToolActivity 保存能够观察到的工具引用、调用／item 关联、状态、输入或 action、结果内容、来源及原生数据。内部 ActivityRef、服务器 call ID、response ID、item ID 和 block index 保持不同类型或结构位置；服务端没有提供 call ID 时，不伪造一个可回传的服务器 ID。

| 事件或结果 | 合同 |
| --- | --- |
| ServerToolStarted／Progress | 仅在协议实际暴露开始／进度时产生；允许多次调用和交错活动 |
| ServerToolResult | 保存实际可见的文本、JSON、媒体、文件和原生结果；不保证服务端公开全部检索内容 |
| ServerToolFailed | 保存工具级错误；服务器仍可能继续输出解释或使用其他工具 |
| Citation／Sources | 连接到实际 candidate、item 和文本块；来源表与内联引用分别保存 |
| 完整 ServerToolActivity | 汇总实际已观察事实；缺失输入、结果或进度保持未知，不补空值冒充已返回 |

一些协议只暴露最终元数据，或者一个内建工具包含多个内部动作。适配器按实际证据记录，不为了统一生命周期虚构 started、completed 或一一对应关系。多个同类工具无法明确关联时保留原生位置，不按名称猜测归属。

工具声明不等于工具一定执行；不能等所有声明工具都有结果才完成响应。服务端工具失败也不自动等于整个生成失败。响应何时完成、是否因 `pause_turn` 等状态需要显式继续，由 protocol／dialect 判断；本库不自动启动下一次生成，也不要求应用为 ServerToolActivity 填写客户端工具结果。

引用保存来源 URL／文件、标题、定位和范围单位；不能混用字节、UTF-16 与 Unicode 标量下标。原生结果中的加密内容、搜索展示信息等作为数据保留。应用负责渲染，库不自动访问引用 URL。

## 5. 消息回传与序列化

### 5.1 通用视图和原生历史

用户新建的输入按通用 Message／ContentPart 表示；来自模型的输出同时提供通用读取视图与适配器维护的不可变原生历史数据。原生数据带 protocol／dialect 归属及必要的服务上下文，但不引入资格认证或信任链。

输出转成下一轮输入时使用明确的 `toInput()` 一类入口：它选取协议允许回传的 item／part，保留有序块、未知字段、工具关联和 reasoning；多 candidate 时须选定一个候选，不将候选串成同一轮历史，也不把整个 HTTP 响应原样当成请求。通用读取视图不独立修改后再与旧 raw 数据暗中合并。应用需要改写历史时创建新输入，并承担显式舍弃或转换原生信息的选择。

同 protocol 不保证不同 dialect 的历史可以互用。跨 provider 时，普通文本等可等价内容可以重新编码；遇到不可迁移的签名、continuation、文件引用或服务端工具数据必须明确报告。不能仅凭展示名称、相同 JSON 字段或相同模型字符串判断兼容。

消息与结果可以通过显式 JSON codec 保存和恢复，原生 JsonValue 不丢数字 token、字段及顺序。第 6 节的请求／响应时间戳、Duration、usage 和可选计费信息可作为独立 metadata 一同保存；它们不注入下一轮模型输入。能力描述和连接对象不随消息存储，认证信息也不进入其中。JSON 值保真不承诺原始空白和转义拼写；请求签名始终使用最终发送字节。

### 5.2 复用当前序列化能力

| 数据 | 编解码方式 |
| --- | --- |
| 稳定的 request／response DTO | 分别使用 ValueEncoder／ValueDecoder 和自动派生，按需使用 Serialize.rename |
| 协议的 discriminator union | 手写分派 codec，读取 type 后将同一稳定 Decoder 交给分支 DTO |
| 未知 item、扩展字段、schema、原生工具结果 | 不可变 JsonValue，可从原 child 捕获，再进行 typed 解码 |
| String-key object | Json.objectEncoder／objectDecoder 或对象 builder；不使用默认 Map entry 序列冒充 object |
| missing／null／value | 需要三态的字段由父 record codec 明确处理；不能依赖嵌套 Option 默认表示 |
| 媒体 bytes | adapter 显式进行 API 要求的 base64／引用编码 |

这些能力已经由 [语言规范 11.13](../specs/SCOOP-SPEC.md#1113-编码解码与缺省实现)及 [JSON 库规范](../specs/SCOOP-JSON-SPEC.md)提供。无需新增 LLM 专用反射、编译器派生规则、字段级 codec 注解或一套 JSON parser。

普通 typed 请求可使用 HTTP 已有默认 JSON 入口。若请求已在 JsonValue 中完成扩展合并，直接 stringify 并生成一次待发送字节；签名与发送消费同一表示，不为两者分别重跑 codec。后续有界 UTF-8 写出优化属于 JSON／HTTP 层，不阻塞初始适配。

工具参数的文本 delta 按 call 分别累积，完整时解析；JSON 外层事件出错属于协议错误，模型生成的参数不合法或不符合应用 DTO 则保留原文与解码错误。部分参数不进入可执行调用列表。完整 JSON 参数可通过普通泛型 `decodeArguments<T>(codec)` 及 companion 重载读取，结构化输出同理；它们不执行工具或自动重试生成。

`LlmGenerationOptions.output` 使用 Text、JsonObject 或 JsonSchema，缺省 Text 不强制发送额外 wire 选项。结构化输出继续使用同一 generate／流式路径，不增加 generateObject。Schema 约束服务端生成，结果的 `decodeOutput<T>(codec)` 或 companion 重载负责本地解码；多个候选或结构化 item 必须明确选择，refusal、缺失、不完整或无效 JSON 明确报告并保留原结果。部分 JSON 不补全成已完成 DTO。

## 6. 计时、token 与计费扩展

### 6.1 Request 与 response 时间戳

每次逻辑生成调用建立独立的 `LlmCallMetadata`，将发送的 request 与接收的 response 关联起来。时间戳属于本次执行的 metadata，不修改调用者可复用的 LlmRequest；同一个请求对象并发发送时，各自保存自己的时间。重试也不覆盖前一次尝试的记录。

默认采集以下关键边界，公开时间戳使用 `scoop.time.Instant`：

| 时间戳 | 采样边界 |
| --- | --- |
| `startedAt` | 进入本次逻辑调用，包含后续准备、凭据、排队和重试等待 |
| `request.sentAt` | 本次尝试的请求头交给 HTTP 协议层；表示本地提交，不声称服务器已收到 |
| `request.bodyCompletedAt` | 请求正文发送路径确认完成；未完成或底层未提供时缺失 |
| `response.receivedAt` | 收到本次尝试的最终 HTTP 响应头，不以 1xx 信息响应代替 |
| `response.firstEventAt` | 解码出首个模型协议事件，排除传输 keepalive；非流式响应不补造流事件 |
| `response.firstOutputAt` | 首次观察到非空模型输出，包括文本、reasoning、工具输入或媒体；不把单纯 role／状态事件当作输出 |
| `response.firstTextAt` | 首次观察到普通可展示文本，和 reasoning／工具输入分开 |
| `response.completedAt` | 完整模型响应及协议要求的尾部数据已接收、解析；取消或截断时保持缺失 |
| `finishedAt` | 本次调用成功、失败或取消后的实际收尾完成 |

HTTP 的首字节时间另作 `firstResponseByteAt`，可能包含信息响应，与上述 firstEvent／firstOutput 不是同一个指标。普通 HTTP 的 receivedAt 也不是响应正文读完的时间。WebSocket 在请求消息提交和首个归属于该响应的协议消息处分别采样，不借用握手时间；多路响应按实际关联分别记录。

Metadata 保留逻辑 call ID、各次尝试编号及已知的服务端 request／response ID；最终 response 关联产生它的那次 request 时间戳。未发送就失败时 sentAt 缺失；重试后不能用第一次发送与最后一次响应拼成单次网络延迟。失败和取消保留已观察到的时间与 usage，通过 `LlmCallOptions.onFinished` 完成观察入口交付；每次逻辑调用在实际收尾后通知一次，包括打开流前失败，不为附加指标改写 CancellationException。流对象可读取当前快照，在作用域关闭后发布最终完成通知；generate 等待实际收尾后返回包含最终快照的结果，不需要存储每条 delta 的历史。

`withStream` 保持普通作用域函数的返回值语义，block 内取得的快照不会在关闭后被隐式改写。选择 Result 保留模式的 `openStream` 在 `useAsync` 关闭后可通过 `result()` 读取完整结果及最终 metadata；该读取不执行 I/O，且要求协议完整与收尾成功。事件模式、提前退出或失败不能返回伪造的完整结果；所有模式在关闭后仍可读取已知 metadata。具体调用见[流式示例](USAGE.md#4-流式输出与完整结果)。

时间戳作为本地 metadata 默认保留，不擅自增加供应商未知的 JSON 字段或 HTTP header。若需要线上传递时间，使用 dialect 明确支持的字段并纳入正常签名流程。响应中的 server created time、HTTP Date 等独立保存为服务端时间，不覆盖本地 receivedAt，也不假定两个时钟同步。

### 6.2 耗时与 HTTP 衔接

关键边界同时保留同源单调读数，耗时使用 `TimeMark` 的差得到 `Duration`。UTC 时间用于保存和跨请求关联；实时钟回拨或校时不应导致负耗时，也不能用 wall clock 的先后顺序判定响应非法。持久化只保存 Instant、Duration 和相对调用起点的偏移，不序列化 TimeMark 或 Clock 对象；恢复后不把旧偏移当成当前进程的单调读数。[时间规范](../specs/SCOOP-STDLIB-SPEC.md)、[平台时钟](../specs/SCOOP-PLATFORM-SPEC.md)

| 指标 | 定义 |
| --- | --- |
| `totalDuration` | started 到 finished，包含准备、排队、重试与清理 |
| `responseLatency` | 同一尝试的 sent 到 completed；未取得完整响应时缺失 |
| `timeToHeaders` | 同一尝试的 sent 到 received |
| `timeToFirstOutput`／`timeToFirstText` | 同一尝试的 sent 到 firstOutput／firstText |
| HTTP 阶段耗时 | 复用已有 queue、DNS、connect、proxy、TLS 阶段数据；未经过的阶段保持缺失 |

这些是客户端观察到的耗时，包含相应网络、服务端工具和背压影响，不等同于模型纯推理时间。流式 chunk 可能包含多个 token，firstOutput 不声称是逐 token 的精确 TTFT。若提供平均输出速率，明确使用 `final outputTokens / responseLatency` 的端到端口径；计数缺失、响应不完整或分母为零时不提供该速率。

当前 [HTTP 可观察性合同](../specs/SCOOP-HTTP-CLIENT-SPEC.md#112-可观察性与错误)已有 TimeMark 阶段快照，但 observer 是 client 级，且未在每个采样点公开 UTC 时间。LLM 实施需要在 HTTP 所属规范先补窄的请求级采样与结果 metadata 合同：在实际事件产生处采集 wall／monotonic 时间，并与现有 client observer 共存，使借入共享 HttpClient 的调用也能取得自身数据。晚些执行的用户回调不能用当前时钟冒充事件发生时刻；`onAttempt` 位于排池前，不能充当 sentAt。不为计时复制连接池或替换调用者的 observer。

### 6.3 Token 统计与显式计数

每个响应保留原始 usage JsonValue 和归一化的 `TokenUsage`，各项使用非负 Long 或缺失值，缺失不填零：

| 计数 | 归一化口径 |
| --- | --- |
| `inputTokens` | 本次输入的总 token，包含能明确归属其中的缓存读取／写入部分 |
| `nonCachedInputTokens` | 明确未使用输入缓存的分项；仅在原生报告或完整、不重叠的计数口径下给出 |
| `outputTokens` | 本次生成的总 token，包含按该 API 口径归属输出的 reasoning；不只计算可见文本 |
| `totalTokens` | 同口径输入与输出之和；仅在所需项齐全且语义明确时推导 |
| `cacheReadTokens`／`cacheWriteTokens` | 输入缓存的读取／写入细分，可按原生 TTL 等继续保留细项；不再次加到已包含它们的 inputTokens |
| `reasoningTokens` | 输出中的 reasoning 细分，未报告时保持缺失，不从 reasoning 文本长度猜测 |
| 模态细分 | 服务端提供的文本、图像、音频等 token 细项；只在包含关系已知时归一化 |

Dialect 明确原生计数是总量还是子项。例如 Claude 的原生 input_tokens 不包含另外报告的 cache read／creation，归一化必须按其实际口径组合；不能对所有供应商套同一字段求和公式。[Claude cache usage](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)

工具调用次数、搜索次数、音频秒数等保留为带单位的 usage 明细，不强行转换成 token。整条响应级 usage 不按 candidate、item 或工具重复分摊，重复查询同一生成任务也不产生新的模型用量。每个 usage 快照标明 Partial／Final；累计更新替换相应字段，delta 只处理一次，未出现的字段不擦除已有值。完成状态不能把未返回的计数自动变成 Final 的零值。重试的 usage 分别属于各次尝试，最终响应不掩盖之前已知的用量；逻辑调用汇总只汇总明确、互不重复的项，并标记遗漏。

请求前的 `countTokens(model, request, mode)` 是显式操作，与 response usage 分开。mode 明确选择服务端计数接口或适用的本地 tokenizer，不在 generate 前偷偷联网或重复分词；结果保留方法、模型／tokenizer 版本、覆盖范围以及精确或估算的性质。计数针对实际 protocol／dialect 的输入表示，包括可计算的 system、工具 schema 和历史；媒体或隐藏包装无法计算的部分明确缺失。没有适用 tokenizer 时不默认用字符数冒充精确 token，也不承诺服务端预计数等于最终计费用量。预估值不能覆盖服务端报告值。

### 6.4 用时间戳估计 prompt cache 有效期

默认提供 request／response 时间戳与 cache usage 供应用判断。Dialect 可补充 `CacheObservation`：实际读／写事实、对应 request attempt、已知的缓存资源／前缀／断点范围、TTL 及起算／刷新规则，以及服务端明确返回的到期时间。只有总 cacheReadTokens 时，不猜测是哪一段历史命中，也不把整段会话的缓存一起续期。

缓存时间估计必须保留以下区别：

- TTL 从请求开始、写入或读取时刻起算，由具体服务规则决定；不能统一从 response.completedAt 起算。只有确认会刷新寿命的读取才更新对应缓存的估计。
- 精确到期时间、最短保留期、最长保留期和典型驱逐时间是不同信息。服务端报告的时间保留来源；本地根据时间戳推导的结果标为 estimate，并保留依据与能确定的时间区间。
- 缺少服务器采样时间时，sentAt 是请求起点的本地近似；receivedAt 或后续确认事件可帮助界定观测区间，不能当作精确的服务器写入时刻。`sentAt + TTL` 不等于已证明的到期时间。
- 当前进程内优先以同源单调时间判断经过时长；跨进程恢复只能使用保存的 UTC 时间作估计，保留校时与跨机器时差的不确定性。
- 时间窗口内不保证一定命中：前缀、模型、部署、缓存配置与服务端路由仍须相容。命中 token、计费覆盖和实际复用 KV／降低延迟也不能自动视为同一事实。

例如，采用“从请求开始计 5 分钟”的规则时，10:00 发起、10:04 完成的请求，其本地估计边界约为 10:05，而不是 10:09；生成耗时已经占用有效期。Claude 当前文档明确这一起算方式。OpenAI 的缓存寿命与刷新规则则依模型和策略不同，最低计费覆盖期也不保证 KV 复用或低延迟；应由各自 dialect 描述。[Claude 缓存寿命](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)、[OpenAI 缓存寿命](https://developers.openai.com/api/docs/guides/prompt-caching)

本库提供这些观测值与可选时间估计，不自动发送保活请求、重排历史或建立缓存调度器。Prompt cache 与 HTTP 响应缓存保持独立。

### 6.5 Dialect 的可选计费扩展

Dialect 可提供 `BillingInfo`，来源区分 ProviderReported 与 Estimated；未配置扩展或缺少依据时为缺失。扩展消费实际模型／部署、usage、缓存策略、服务等级和已提供的价格资料，返回金额、币种、覆盖范围及可解释的明细。计费不可用不影响生成、计时和 token 统计。

金额和费率采用精确十进制表示，保留币种与单位，不使用二进制浮点累积金额。输入、输出、缓存读写、服务端工具和媒体等费用只按对应服务规则计算；推理 token 若已包含在输出计价中，不能再次加收。未知费用项保留未知，部分估算不能伪装成完整总价。

Estimated 保留价格来源／版本或生效时间、模型与部署、计算依据和完整性；ProviderReported 保留服务端原始计费数据。网关价格可以与上游供应商不同，不能只按基础模型名称套价。重试费用分别关联对应尝试，来源不明的失败尝试不能记为免费，也不能重复累计流式 usage 快照。

扩展使用服务端返回信息或显式提供的价格快照，不在普通生成中自动抓取价格。不增加跨供应商价格目录、预算控制、扣费或账单管理；这些由应用承担。

## 7. 一次请求的执行路径

1. 应用选择 Model，构造输入、工具和选项；建立独立调用 metadata 并记录 startedAt，不进行隐式 provider 路由。
2. 组合显式缺省值，检查模型入口与本次模式的有效能力，并由 dialect 检查实际组合。
3. Protocol＋dialect 将输入、历史和工具映射到 wire DTO／JsonValue，完成扩展冲突检查。
4. Endpoint 确定实际 URL、query 与 body；取得凭据后对最终表示认证／签名。
5. Client 执行 HTTP 请求，在实际提交和接收边界采集本次尝试的时间戳；先处理 status 与有限错误正文，再消费完整 JSON 或 framing 后的事件。
6. 对应 reducer 更新原生 item 和结果，交付通用事件、服务端工具活动、引用与 usage，并记录首次输出及完整响应时刻。
7. 到达协议终止状态时记录完整结果；generate 等待收尾后返回结果及最终 metadata，流式调用在作用域关闭后发布最终计时、usage 和可选缓存／计费信息。应用决定是否执行客户端工具、继续服务器状态或发起下一轮。

取消、关闭、背压、超时和有限重试沿原调研及 HTTP 合同。默认关闭生成请求的 redirect、通用 SSE reconnect、Cookie 和 HTTP cache；一次生成只有一层负责重试。只有事件消费者主动需要完整结果时才保留相应累积数据，不保存整条原始事件日志。

同一流可选择原始 `next()` 或 `textDeltas()` 投影的 `next()` 作为唯一消费者；开始消费后不能换入口，不能同时建立多个 reader。投影仍驱动同一 reducer，仅筛选普通文本 delta，保留 candidate／item／part 关联；usage、工具、尾部事件、错误与取消按完整事件流处理。它不复制流、不建立 replay，也不改变 EventsOnly／Result 保留策略。该读取视图服务展示，不形成另一套生成入口。

## 8. 模块与实施顺序

初始使用一个 `scoop.llm.client` Cone。`model` 管公共输入／结果与 capability，`tools` 管工具声明／活动／引用，`metrics` 管调用时间与 usage 快照，`protocols` 管 wire DTO／codec／reducer，`dialects` 管对应协议的服务差异及可选缓存／计费解释，`providers` 管命名组合，`endpoint` 管部署和认证组合。模块使用已有库接口，不另建 HTTP 栈或 Agent runtime。

| 顺序 | 交付内容 | 验收出口 |
| --- | --- | --- |
| 1 | 固定公共数据、capability、工具、错误与调用 metadata；补 HTTP 请求级时间采样合同 | 可编码请求、可解码结果；发送／接收时间可取得，未知能力与明确不支持有不同结果 |
| 2 | Chat protocol、基础 dialect 和 DeepSeek Chat dialect | 同一基础协议的两种行为；工具两轮回传、计时与 token usage 闭环 |
| 3 | Responses、Messages 和 Gemini 的各自 provider，先实现 WebSearch | 客户端与服务端工具自然共存；活动、引用、usage 和原生历史保留 |
| 4 | 按协议矩阵补部署、其他服务端工具、显式 token count、缓存寿命与可选 billing 扩展、持久会话 | 每项声明能力有对应映射与测试，不以供应商品牌数量验收 |

模块边界为当前两个具体 dialect 和多个真实服务端工具形状服务，不先实现通用插件层、规则语言或全部供应商目录。新 protocol 可由普通公共适配接口实现；生成与实时会话使用各自合同，不在无关接口中堆放占位方法。

## 9. 必要验收

普通 API、codec 和 reducer 行为放在所属 Cone 的 `scoop.test` 中。需要外部进程控制、CLI、产物或链接行为时才使用 fixture；实际服务的 live smoke 单独记录，不替代可重复的本地协议测试。

| 维度 | 必须覆盖的行为 |
| --- | --- |
| Provider 划分 | 同供应商两个 protocol、同模型两个 provider、同 provider 两个 endpoint；不根据名称猜协议，不因目录缺项拒绝 |
| 使用与缺省参数 | 最小文本调用与多模态／工具共用同一 request；省略字段的实际 wire 行为、显式 None、借入 client 所有权、流关闭后的结果与 metadata |
| 视图与扩展 | 文本投影的单消费者、usage 尾块与错误传播；request／message／part／tool 扩展的归属、位置及往返；结构化输出与普通生成共用路径 |
| Dialect | 基础 Chat 与 DeepSeek Chat 的不同历史映射；带／不带 tools、无实际工具调用的历史轮次、交错 reasoning／content delta |
| Capability | 同 provider 不同模型、同模型不同入口；文本／图像／PDF／音频／视频各方向，MIME 与输入形式组合，Unknown 放行与 Unsupported 拒绝 |
| 组合约束 | reasoning＋toolChoice、structured output＋server tool、客户端与服务端工具混用；显式不支持项不被静默删除 |
| 工具执行方 | 三种 WebSearch 线格式；供应商定义但应用执行的 ClientNative；同名客户端工具、工具未执行、多次／交错执行、缺少 call ID、仅有结果元数据、工具错误后仍有模型输出 |
| 历史往返 | 响应解码→序列化保存→恢复→加入客户端工具结果→下一轮编码；保留 reasoning、原生字段、工具关联和 block 顺序 |
| 结果完整性 | citation 定位、usage 尾块、停止／继续状态、未知事件、无终止 EOF；坏参数与坏外层协议区分 |
| 计时与时间戳 | 排池不计成已发送、1xx 与最终响应头、长流、首次 reasoning／文本、并发复用请求、重试关联、未发送失败／取消、wall clock 回拨、metadata 保存与恢复 |
| Token 计数 | 累计与 delta、usage-only 尾块、部分与缺失计数、cache／reasoning 重叠、多个 candidate、重试用量；预估与报告分开，未知 tokenizer 不伪造精确结果 |
| 缓存时间 | 长响应消耗 TTL、读取是否续期、不同前缀／断点范围、最短保留与精确到期的区别、并发请求、跨进程时间估计；没有命中证据时不猜测续期 |
| 计费扩展 | 原生金额与估算、币种／单位、价格版本、部分费用、缓存与工具计价、网关差异、重试和快照不重复收费；缺省扩展不阻塞生成 |
| 传输组合 | 分块 framing、取消、提前退出、共享连接与借入 client 的寿命；签名和实际 body 一致 |

JSON 与 HTTP 已覆盖的基础规则不在每个 provider 重复全量验收。针对各协议锁定必要的小型 wire 样例和可观察结果，服务端工具测试不要求真的联网搜索或执行付费模型调用。
