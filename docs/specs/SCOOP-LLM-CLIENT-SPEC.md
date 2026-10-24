# Scoop LLM client 规范

本库通过普通 `scoop.llm.client` Cone 实现。总体职责、协议覆盖和实施顺序见 [架构设计](../llm-client/DESIGN.md)与[实施记录](../llm-client/PROGRESS.md)。本规范随功能闭环补充正式数据与 API 合同；尚未落地的调用示例仍以[使用草案](../llm-client/USAGE.md)标明状态。

## 1. 公共 token 用量

`TokenUsage` 是不可变的归一化计数快照。以下构造参数均为 `Long?`，缺省 None：

| 字段 | 含义 |
| --- | --- |
| inputTokens | 输入总量，包含已明确归入输入的缓存读写部分 |
| nonCachedInputTokens | 已明确报告或按完整、不重叠口径得到的非缓存输入量 |
| outputTokens | 输出总量，包含已归入输出的 reasoning 部分 |
| totalTokens | 明确报告或按已知口径得到的输入与输出总量 |
| cacheReadTokens | 输入缓存读取细项 |
| cacheWriteTokens | 输入缓存写入细项 |
| reasoningTokens | 输出中的 reasoning 细项 |

None 表示没有该计数，Some(0L) 表示已知为零；构造和序列化往返不能把两者混同。出现的计数必须非负，负值在构造时抛 IllegalArgumentException；使用 Long 保存完整整数，不经 Double 转换。

`modality: Array<ModalityTokenUsage> = []` 保存有明确归属的模态细项。每项包含 direction（TokenDirection.Input／Output）、modality（TokenModality.Text／Image／Audio／Video／Document／Other(name)）及非负 Long tokens。细项不是额外总量，不自动与 inputTokens／outputTokens 相加。媒体秒数、搜索次数和金额不放进 token 计数。

TokenUsage 不根据字段名猜测供应商原生口径，不自动求和、补齐总量或校验未经声明的包含关系。原生字段到这些计数的映射由相应 dialect 完成；重试与流式快照的归属由调用和协议 reducer 管理。此处的数据构造器不进行跨尝试汇总，也不实现价格／预算框架。

上述类型的 companion 显式声明 ValueEncoder／ValueDecoder，方法使用语言现有的缺省实现合成。TokenUsage 与 ModalityTokenUsage 按普通记录编码，enum 使用普通 enum codec 表示。缺省为 None 的字段按语言规则省略，解码时缺失的计数字段恢复为 None，缺失的 modality 恢复为空数组；因此空对象也是有效的未知用量。Some(0L) 必须保留。格式库负责数字及形状检查，普通构造器负责其计数范围；不新增编译器 intrinsic 或另建序列化层。

## 2. 能力描述

`Capability<T>` 使用 Supported(T)、Unsupported(reason: String)、Unknown 三个变体。T 是该能力的明确细节；不需要细节时使用 Unit。Unknown 表示信息缺失，不是 Unsupported，也不自动升级为 Supported。companion 按 T 的 companion 条件实现编码和解码，使用普通 enum 数据模型。

`ModelCapabilities` 是一个模型在特定 provider／部署入口的不可变描述，不携带 API key，不以模型名推断协议，也不将能力支持当成请求选项。构造器各组缺省为空描述，各能力缺省 Unknown；空对象解码得到相同缺省值。输入、输出、采样、结构化输出与工具选择分别描述，未填写的值不从另一方向或相近能力继承。

### 2.1 媒体格式与限制

`MediaForm` 为 Bytes、Url、FileReference，描述媒体被直接提供、通过 URL 提供或通过服务端文件引用提供的形式。它用于输入与输出格式描述；文本内容单独表达。`MediaFormat(mimeType, form)` 保存完整 MIME 与形式；MIME 使用现有 HttpMediaType 解析器检查并保存解析结果，不重复解析不变值。不得使用 `*` 作为 type 或 subtype，也不使用 MIME 通配匹配。

`MediaFormat.matches(other)` 比较 MIME type、subtype、参数名以及参数值，忽略 type／subtype／参数名的 ASCII 大小写、参数顺序和参数值的引号拼写；参数值按解引号后的原字节精确比较。不忽略参数，不从无参数规则推断带参数的格式，也不将某种参数的大小写规则推广到所有参数。原始 mimeType 文本用于保存，解析缓存不参与 codec。

`MediaFormatCapability(format, support)` 指定一个精确组合的 `Capability<MediaLimits>`。`MediaCapabilities(formats = [])` 保存这组事实；重复组合在构造时失败，即使两项状态相同。`forFormat(format)` 只返回匹配项；未列出的组合返回 Unknown。它不是白名单，也不把所有 MIME 与所有形式求笛卡尔积。上层能力为 Unsupported 或 Unknown 时，查询返回相应状态；上层 Supported 也不能让缺失组合变为 Supported。

`MediaLimits` 的下列 Long? 字段缺省 None，表示未知上限；已知上限必须非负，零仍是明确上限：

| 字段 | 单位和范围 |
| --- | --- |
| maxBytesPerItem | 每个媒体内容的解码后字节数，不含 base64 膨胀 |
| maxItemsPerRequest | 一次请求中该媒体种类的数量 |
| maxPagesPerItem | 每个文档的页数 |
| maxDurationMillisecondsPerItem | 每个音频／视频内容的时长，毫秒 |
| maxWidthPixels／maxHeightPixels | 每张图片或每个视频帧的宽／高，像素 |

采样率、声道及编码等精确格式由 MIME 参数或相应 dialect 的真实选项表达；没有可靠通用语义的限制不塞进任意键值规则。库不读取远端资源来补齐未知大小，不自动 OCR、转码或转换媒体。

`InputCapabilities` 的 text 为 `Capability<Unit>`，image／pdf／audio／video／document 为 `Capability<MediaCapabilities>`；videoFrames 是独立的 `Capability<MediaCapabilities>`，格式描述其中帧图像的格式。`InputMediaKind` 与 `forMedia(kind, format)` 提供精确查询。PDF 表示原生 application/pdf，document 表示其他文档；输入 PDF 不由 document、image、上传能力或 videoFrames 推断。

`OutputCapabilities` 的 text 为 `Capability<Unit>`，image／audio／video／document 为 `Capability<MediaCapabilities>`；document 包括明确列明的 PDF。`OutputMediaKind` 与 `forMedia(kind, format)` 不读取输入描述。模型的视频输出不意味着输入视频或实时会话受支持。

### 2.2 生成特性与模型上限

`ModelCapabilities` 的 streaming、realtime、functionTools、customTools、parallelToolCalls、multipleCandidates、continuation、background 均为 `Capability<Unit>`，互相独立。`SamplingCapabilities` 分别描述 temperature、topP、topK、stop。`ToolChoiceCapabilities` 分别描述 auto、none、required、specific，支持一项不自动支持其他项。

`StructuredOutputCapabilities` 分别描述 jsonObject、jsonSchema、strictJsonSchema。其中 strictJsonSchema 为 `Capability<Unit>`，jsonSchema 为 `Capability<JsonSchemaCapabilities>`；后者的 `dialect: String?` 可保存明确的 schema dialect URI，`supportedKeywords: Array<String>?` 可保存明确可用的关键字集合。None 是未知，Some([]) 是已知空集合。单个关键字存在不意味着任意 schema／工具组合有效，实际适配器仍按协议检查完整请求。`ReasoningCapabilities` 分别描述 enable、effort、budgetTokens 的可配置性；不表示模型一定返回 reasoning 文本。

`ModelLimits(contextWindowTokens, maxInputTokens, maxOutputTokens, maxCandidates)` 均为 Long?，缺省 None，出现时必须非负；不同计量口径不相加，也不依据其中一项补齐其他项。

以上类型均可使用 companion codec 保存和恢复，构造检查同样适用于解码。服务端工具种类与版本、客户端原生工具、组合约束及 token 计数入口随各自公共工具／操作合同定义；不能由 functionTools 推断。当前数据描述不提前查询模型目录，也不建立通用规则 DSL。

## 3. 公共请求数据

`LlmRequest(input, system = [], tools = [], options = LlmGenerationOptions(), extensions = [])` 为不可变请求快照。input 必填，普通消息使用 `LlmInputItem.Message(content, role = LlmRole.User, extensions = [])`；同一个入口用于最小文本、多模态和工具请求。请求不保存 endpoint、model、认证或本次调用的时间戳，也不隐藏查询／下载／执行操作。

### 3.1 归属与按位置扩展

`ApiProtocolId(value)` 标识一种线协议，`DialectId(protocol, value)` 标识该协议内的方言。两者按字段值比较；字符串为不拆分、不改写的公开语义标识，实际 adapter 提供其身份，不通过供应商展示名或 endpoint 域名推断。创建标识本身不会注册或创建适配器。

`DialectExtension(dialect, value)` 的 value 必须为不可变 JSON object，非对象在构造时抛 IllegalArgumentException。request、Message、ContentPart 和 ToolDefinition 各自的 extensions 缺省空数组。同一个位置不能重复同一 DialectId；不同位置可分别使用同一归属，不继承或合并其字段。构造 LlmRequest 时检查这些局部重复，报告 `LlmRequestException(path, message)`，path 是请求模型中的 JSON Pointer 形式位置。实际位置是否适用、归属是否匹配当前 adapter，以及与管理字段的冲突，留给绑定后的一次协议请求检查；此处不猜测未知扩展键的语义。

### 3.2 内容与历史输入

`ContentPart` 提供 Text(text)、Image(source)、Document(source, filename = None)、Audio(source)、Video(source)、VideoFrames(frames) 和 Native(dialect, value)。前六者均有缺省为空的 extensions。Native 为所选方言明确规定的原生内容，其 value 由 adapter 解释，不与通用内容字段进行隐式覆盖。

`MediaSource` 区分 Bytes(bytes, mimeType)、Url(url, mimeType)、FileReference(reference, mimeType)。bytes 是不可变数组，url 使用 Url 值；`LlmFileReference(dialect, value)` 保留方言拥有的文件 ID／URI 及必要原生服务上下文。文件引用不意味着已经上传或支持该媒体；保存／恢复不下载资源、不转码、不把 URL 自动转成 bytes。MIME、媒体种类与形式的组合由实际请求归一化检查一次，不能只凭构造了一个 MediaSource 宣称可发送。

`VideoFrame(source, offset)` 使用非负 Duration 表示帧相对视频起点的位置；负偏移在构造时失败。帧顺序按输入保存，不自动排序、提取或补帧。

LlmRole 为 User、Assistant、System、Developer。system 数组与消息中的 role 均由 protocol 明确映射；库不依据模型名重写角色。`LlmInputItem.ToolResult(call, content, isError = false, extensions = [])` 表示应用执行后的结果；`ClientToolCallReference(dialect, toolKey, id, context = None)` 保存方言、请求内工具引用、服务器 call ID 和必要原生上下文。历史中的旧调用不要求在本次 tools 中重新声明；回传合法性由协议检查。

`LlmHistory(dialect, items, context = None)` 保存已经为回传选出的有序原生 item 和必要上下文；`LlmInputItem.History(history)` 将其加入请求。它不保存另一份可独立修改的通用文本视图。普通数据构造不保证跨方言兼容，协议解释和结果 toInput 投影随相应 adapter 实现。完整响应正文不能未经投影直接冒充 history items。

### 3.3 工具声明

ToolKey(value) 是请求内的本地引用，按值比较，不能用来替代服务器 call ID。`ToolDefinition` 的四个变体如下，各自的 extensions 缺省空数组，顺序按请求数组保存：

| 变体 | 字段与执行方 |
| --- | --- |
| ClientFunction | key、name、parameters、description（缺省空串）、strict（缺省 None）；应用执行，parameters 为 JSON Schema object |
| ClientCustom | key、name、format（缺省 Text）、description（缺省空串）；应用执行，CustomToolFormat.Grammar(syntax, definition) 保存供应商支持的语法合同 |
| ClientNative | key、NativeToolDefinition；格式由供应商定义，应用执行 |
| Server | key、ServerTool；服务端执行 |

`NativeToolDefinition(dialect, kind, configuration = {}, version = None)` 保存原生种类、配置和显式版本；configuration 必须为 object，执行方仍由所属 ToolDefinition 变体与实际协议确定。`ServerTool` 首先提供 WebSearch(options = WebSearchOptions()) 与 Native(tool)，后续通用工具按真实协议映射扩展，其他种类可由明确的 Native 描述承载。

WebSearchOptions 的 allowedDomains、excludedDomains 为 Array<String>?，location 为 SearchLocation?，includeSources 为 Boolean?，均缺省 None。None 表示未指定，显式空集合或 false 不被改写成 None。SearchLocation 保存可选 country、region、city、timezone，不猜测地域。选项只表达请求意图；adapter 必须能精确映射，不能静默删除不支持的域名限制或把地域换成另一口径。

ToolChoice 为 Auto、None、Required、Specific(ToolKey)，缺省 Auto。构造请求时拒绝重复／空 ToolKey、空客户端工具 name、非对象的 function parameters，以及 Specific 指向本次 tools 之外的 key。不同工具可具有相同展示／API name；实际 wire 命名冲突由 adapter 检查。Required 不表示每个工具都要调用。库不接收工具 execute 回调，也不自动执行工具或启动下一轮生成。

### 3.4 生成设置与保存

LlmGenerationOptions 的 temperature、topP 为 Double?；topK、maxOutputTokens、candidates 为 Long?；stop 为 Array<String>?；parallelToolCalls 为 Boolean?，均缺省 None。Double 必须有限，整数必须非负，已填写的 candidates 必须大于零；各模型的范围、空 stop 的含义及功能组合由具体 adapter／capability 判断。不设跨模型的固定温度范围，不自动补采样参数。

reasoning 为 ReasoningOptions?，其中 enabled、effort、budgetTokens 均可缺失；预算为非负 Long，effort 按原字符串保留，具体允许值由 dialect 定义。output 缺省 OutputFormat.Text，另外有 JsonObject 和 JsonSchema(schema, name = "response", description = None, strict = None)。schema 必须为 object；名称与 schema 子集按所选协议检查，不从 ValueDecoder 反推 schema。

所有上述请求数据提供 companion JSON codec，普通 record／enum 使用语言缺省合成；缺失字段按声明恢复默认值，None 与显式 false／零／空集合保持区别。JsonValue 保留数字 token、字段顺序和未知字段。构造检查同样适用于解码；协议 wire 编码另外实现，不把公共 enum 的保存格式直接发送给供应商。

## 4. 响应、读取视图与历史投影

`LlmResponse(dialect, candidates, id = None, model = None, createdAt = None, usage = LlmUsage(), native = None)` 保存一次模型响应。createdAt 是供应商明确报告的时间，不充当本地接收时间。candidates 保留协议提供的顺序，每个 `LlmCandidate(index, items, completion, finishReason, history = None)` 保存非负、响应内唯一的 index、有序输出、生成完整性、停止原因和可回传历史。LlmCompletion 为 Complete、Incomplete、Refused、Failed；协议响应已读完与模型生成完整是不同事实。

LlmFinishReason 为 Stop、Length、ToolCalls、ContentFilter、Refusal、Pause、Other(raw)、Unknown。Unknown 不被当作 Stop，未知原生原因使用 Other 保存。非流式及流式 reducer 共用这些最终数据；构造器不会根据有无文本推断成功。

### 4.1 有序输出和工具输入

LlmOutputItem 分为 Message(content, id = None)、Reasoning(content, id = None)、ClientToolCall(call)、ServerToolActivity(activity)、Native(dialect, value)。Message 与 Reasoning 的内容为 `Array<OutputPart>`，其变体包括 Text(text, citations = [])、Refusal(text)、Media(kind, source) 和 Native(dialect, value)。普通文本视图只读取 Message 中的 Text，保持 item／part 顺序；不拼入 reasoning、拒绝说明、工具参数或 Native 内容。

`ClientToolCall(reference, input)` 的 reference 使用第 3 节的 ClientToolCallReference。ClientToolInput 分为 JsonArguments(arguments)、Text(text)、Native(value)、Incomplete(rawText)、Invalid(rawText, reason)。`JsonToolArguments(rawText)` 在构造时解析一次并要求 object，其只读 value 保存解析结果；codec 只保存原文，恢复时从原文重建缓存，不能独立修改两份表示。协议适配器在参数完整时构造它；模型生成的 JSON 无效或不是 object 时保留 Invalid，不把整个外层响应当作格式错误。Incomplete 和 Invalid 的 isExecutable 为 false，其余变体只有在 adapter 确认输入的协议形状后构造，应用仍须验证业务 DTO。

call.decodeArguments<T>(codec) 只接受 JsonArguments，使用 Json.decodeFromValue；companion 缺省重载遵守同一合同。其他输入变体抛 LlmOutputException，JSON DTO 解码失败保留原 DecodingException。库不修复 JSON、不补齐缺失参数、不执行工具。

`ServerToolActivity(kind, status = Unknown, toolKey = None, id = None, action = None, result = [], sources = [], native = None)` 保存已观察到的服务端活动。ServerToolKind 为 WebSearch、WebFetch、FileSearch、CodeExecution、ImageGeneration、Native；状态为 Unknown、InProgress、Completed、Failed。缺失 id 不用 ToolKey 或数组位置伪造，缺失 action／结果不意味着失败。服务端活动始终不进入客户端待执行列表。

### 4.2 来源与引用

`LlmSource(id, location, title = None, native = None)` 保存来源，location 使用 LlmSourceLocation.Url(Url)、File(LlmFileReference)、Native(dialect, value)。来源 id 是来源表的引用，不是工具 call ID。`Citation(source, range = None)` 将来源连接到实际文本块；`TextRange(start, end, unit)` 为非负的左闭右开范围，end 不得小于 start。TextRangeUnit 明确区分 Utf8Bytes、Utf16CodeUnits、UnicodeScalars，适配器不得未经换算更换单位。范围未知时保持 None，不伪造覆盖整段文本的区间。

### 4.3 候选选择、结构化解码和保存

response.text、clientToolCalls、serverToolActivities 是首个候选的只读便利视图；没有候选时分别为空串／空数组。响应保留完整 candidates；客户端调用视图只返回 isExecutable 的调用。`candidate(index)` 按协议 index 查找，缺失时报告 LlmOutputException，不将 index 当作数组下标。

response.toInput(candidateIndex = None) 选定一个候选，仅返回其 LlmHistory，不从通用读取视图重建或混合 raw。未指定 index 时必须恰有一个候选；history 缺失时明确报错。返回的 History 不携带 usage、时间戳或 HTTP metadata。同方言历史能否在具体请求模式下回传，由 adapter 编码时检查；原始未知字段、reasoning、签名和工具关联保留在 history 中。

response.decodeOutput<T>(codec, candidateIndex = None, itemIndex = None) 选择一个候选和一个包含 Text 的 Message，将其中 Text 按顺序连接后严格解析 JSON，再调用 codec。候选不唯一、文本消息不唯一、选择越界／类型不符、拒绝、输出不完整或缺失均抛 LlmOutputException；无效 JSON 和 DTO 解码失败保持 DecodingException。itemIndex 指候选 items 的数组位置。自动选择跳过没有 Text 的消息，显式选择这类消息则报告 MissingOutput。不得拼接多个候选或多个 Message，也不自动去掉 Markdown 围栏或补全 JSON。候选消息中存在 Refusal 时不把该候选的其他文字当作结构化成功。companion 缺省重载不改变选择与错误语义。

LlmResponse、候选、输出、工具输入、来源和历史提供 companion codec。序列化保存不可变数据及其归属，不保存 adapter、连接或执行函数；生成协议的历史投影负责选择可回传字段，toInput 始终使用该投影结果。输出视图没有修改接口，不存在编辑通用文本后自动与旧原生历史合并的行为。

## 5. 用量与计时快照

`LlmUsage(tokens = TokenUsage(), completeness = Partial, native = None)` 保存一次响应的 token 统计、Partial／Final 完整性和原生用量。Final 表示该次报告已终结，不表示每个计数都已知；None 不因此变成零。响应整体用量不复制到每个 candidate。

`LlmTimePoint(at, elapsed)` 同时保存 UTC Instant 与相对本次逻辑调用起点的非负 Duration。elapsed 来自同一单调时钟；at 可因校时回拨，二者不互相推导。它可保存／恢复，但不恢复为当前进程的 TimeMark。

`LlmRequestTiming(sent = None, bodyCompleted = None)` 与 `LlmResponseTiming(received = None, firstResponseByte = None, firstEvent = None, firstOutput = None, firstText = None, completed = None)` 保存观察点，并分别提供 sentAt／bodyCompletedAt 与 receivedAt／firstResponseByteAt／firstEventAt／firstOutputAt／firstTextAt／completedAt 的 Instant? 视图。未观察到或底层没有提供的采样保持 None。

`LlmAttemptMetadata(number, request = ..., response = ..., status = None, requestId = None, responseId = None, usage = LlmUsage())` 的 number 从 1 开始。timeToHeaders、timeToFirstOutput、timeToFirstText、responseLatency 仅使用同一尝试的 sent 与相应响应点的 elapsed 差；缺失任一端点时为 None，相应响应点早于 sent 时构造失败，UTC 先后不作为检查条件。outputTokensPerSecond 仅在响应完成、outputTokens 已知且 responseLatency 为正时返回端到端平均速率，否则 None。

`LlmCallMetadata(callId, startedAt, attempts = [], finished = None, outcome = Active)` 保存逻辑调用及尝试序列。LlmCallOutcome 为 Active、Completed、Failed、Cancelled；已结束时须有 finished，Active 时不得有 finished。尝试编号递增，不能覆盖或重排已有尝试。finishedAt 与 totalDuration 是 finished 的只读视图。调用前失败可有空 attempts；这里不根据 outcome 伪造响应完成或缺失 usage，也不把重试用量直接相加成最终响应用量。

`LlmResult(response, metadata)` 组合完整响应和已收尾调用记录，metadata.outcome 必须为 Completed。它提供相同的 text、clientToolCalls、serverToolActivities、toInput 与 decodeOutput 便利入口并提供 companion codec。失败／取消的 metadata 可独立保存；网络执行层负责实际采样与完成通知，这些数据构造器不读取时钟、不发起请求。

## 6. Chat 响应解释

`ChatProtocol(dialect: ChatDialect)` 固定使用 ApiProtocolId("chat-completions")，拒绝与该协议不符的 dialect.id。ChatDialect 的响应合同包含 readReasoning(message, path)、readUsage(usage) 与 historyMessage(message, path)；输入为已经解析的不可变 JsonValue，返回明确的输出内容、TokenUsage 或单个可回传消息，不接受 Any，也不执行网络操作。path 用于保留嵌套字段的诊断位置。请求映射与流式入口随各自实现补充。

`decodeResponse(value, request)` 读取非流式 Chat 的 choices，每项必须有 index、message 和非 null 的 finish_reason；message.role 必须为 assistant。id、model、created 与 usage 未提供时保持未知。原始整个响应保存在 LlmResponse.native。每个候选按 Reasoning、Message、工具调用的稳定次序投影；Chat 完整响应没有这些字段之间的生成时序，该次序不冒充时间记录。缺少 content、refusal 和 reasoning 时不伪造文本。

function 工具调用按本次请求中唯一的同名 ClientFunction 关联 ToolKey，custom 调用按唯一同名 ClientCustom 关联。未知类型、未声明／无法唯一关联的工具调用保留为 Native，不进入可执行列表，不根据名称猜成服务端或客户端执行。合法外层调用中的 arguments 无效 JSON／非 object 保存为 Invalid；输出被截断、拒绝或停止状态未知时保留 Incomplete，即使已返回的参数恰好可解析也不进入待执行列表。同一候选内已关联的客户端调用 ID 必须唯一。其他外层类型、必填字段或数值错误抛 LlmProtocolException，保存 dialect、path、message 与原始响应。弃用的 function_call 无法提供现代工具 call ID，保留 Native，不伪造 ID。

stop 与 tool_calls／function_call 表示完整的该轮模型输出；length 表示 Incomplete；content_filter 在没有明确 refusal 时同样为 Incomplete；明确 refusal 为 Refused。未知停止原因使用 Other(raw) 并保守标记 Incomplete，不能据此返回结构化成功。每个候选的 history 只包含 assistant 消息；不包含 choices 包装、logprobs、usage 或服务器响应时间。

OpenAiChatDialect 的标识为 (chat-completions, openai-chat)，DeepSeekChatDialect 为 (chat-completions, deepseek-chat)。两者保留消息中的未知字段，去掉已知仅供输出的 annotations；audio 回传只保留其 id，不回传 data、transcript 或 expires_at。原始音频、annotations 与弃用 function_call 仍保留为 Native 输出与完整原生响应。DeepSeek 将 reasoning_content 单独投影为 Reasoning，并在 history 中完整保留，即使该轮没有实际调用工具；后续请求编码按是否携带 tools 应用回传规则。OpenAI 不把未知的 reasoning_content 猜成其已声明的 reasoning 输出。

两种 dialect 均从 prompt_tokens、completion_tokens、total_tokens 与 completion_tokens_details.reasoning_tokens 读取明确计数，缺失总量不自动补齐；audio_tokens 按输入／输出归属进入模态细项。OpenAI 从 prompt_tokens_details.cached_tokens 读取缓存输入，并在 inputTokens 与 cacheReadTokens 均已知时按其包含关系得到 nonCachedInputTokens。DeepSeek 优先读取 prompt_cache_hit_tokens、prompt_cache_miss_tokens；cached_tokens 是同义读缓存字段，两者并存时必须一致。出现的计数必须非负，缓存读数不得超过已知输入总数。不把 cache 或 reasoning 再加进总量，也不从文本长度猜 token。完整响应中的 usage 记录为 Final，未返回的计数仍为 None。

协议依据：[OpenAI Chat](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[DeepSeek Chat](https://api-docs.deepseek.com/api/create-chat-completion/)、[DeepSeek thinking](https://api-docs.deepseek.com/guides/thinking_mode/)。请求编码见第 7 节；生成传输、模型能力联动与流式 reducer 继续按实施记录交付。

## 7. Chat 请求编码

`ChatProtocol.encodeRequest(modelId, request, stream = false)` 将公共请求编码为不可变 JSON object，不发网络请求。modelId 按原字符串传递，不用于选择协议或猜测能力；空 modelId 与没有消息的请求报告 LlmRequestException。system 内容在 input 前形成一条 system 消息；普通消息按 role 编码。最小调用只发送 model、messages，未提供的生成字段不补供应商默认值。stream 为 true 时发送 stream 和 include_usage，完整响应模式省略它们。模型 capability 的联动属于后续模型绑定入口。

ChatDialect 增加 encodeOptions(request) 和 encodeContent(content, role, path)；前者处理生成选项及工具在请求根部的映射，后者处理当前 role 的内容。protocol 固定管理 model、messages 和流模式，不允许 dialect 选项覆盖它们。通用选项无法精确表达时在对应公共输入路径报错，不静默删除。OpenAI 的 maxOutputTokens 映射 max_completion_tokens，DeepSeek 映射 max_tokens；两者不以 model ID 改写字段。

OpenAI 支持 User 的图片 bytes／http(s) URL、PDF bytes／本方言文件 ID、WAV／MP3 音频 bytes；音频 URL、图片文件 ID 和视频没有这套 Chat 通用映射。文件 ID 在 LlmFileReference.value 中为 JSON string。PDF bytes 以 data URL 放入 file.file_data，文件名仅用于 bytes；不下载远端文件。图片仅使用 JPEG、PNG、GIF、WebP。DeepSeek 的 User／Tool 支持这组图片的 bytes／URL／本方言文件 ID；文件 ID 编码为顶层 file_id 的 file part，不将其当成 PDF 支持。DeepSeek System／Assistant 仅接受无 part 扩展的文本，Developer 没有对应角色。OpenAI 非 User 消息只接受文本通用 part。Native part 必须属于当前 dialect 且为 object，按原值保留。

普通单个无扩展 Text 编码为字符串，其他允许的内容序列编码为 parts；需要纯字符串的角色按顺序连接文本。MIME 使用现有 HttpMediaType 解析一次，当前映射要求不带参数的明确格式，不忽略无法表示的参数。不自动下载、转码、丢弃 part 或把 VideoFrames 转成无时间信息的图片。已知 DeepSeek 外部图片 URL 超过 8192 字符时报告错误。

ClientFunction／ClientCustom 分别编码 function／custom 工具；DeepSeek 仅支持 function。function 名称在协议允许的 ASCII 字母、数字、下划线和横线范围内检查，OpenAI 上限 64、DeepSeek 上限 128；custom 名称不套用未经声明的 function 限制。同次调用 wire 名称不得重复。ToolChoice.Specific 按 ToolKey 找到精确 wire 工具，不用 key 替代 name。当前 Chat 没有已定义的 ClientNative 合同，不能假装成 function。ToolResult 使用原始 call ID 和 tool role；旧调用无需再次声明，其 dialect、context 和内容必须适合 Chat。Chat 没有通用 isError 标志，显式 isError = true 报错，由应用将实际错误内容作为结果提供。

OpenAI 的 Server(WebSearch) 编码请求级 web_search_options，可映射 approximate location；它不伪装成客户端工具。该入口不支持通用 allowedDomains、excludedDomains 或 includeSources 控制，显式提供即报错。只允许一项搜索声明；选择 Auto，其他 ToolChoice 无法同时精确控制该搜索入口时失败。Server(Native) 仅接受本 dialect 的无版本 web_search 原生配置；DeepSeek Chat 不将独立 Search API 假装成内置工具。编码搜索请求不伪造服务端工具执行记录。

DeepSeek reasoning 缺省按服务文档为开启；enabled 与 effort 冲突时拒绝。temperature 只在显式关闭 thinking 时可用，topP 只在 thinking 中且 0.95～1 范围内可精确表达；Required／Specific 也要求关闭 thinking。该规则来自方言，不通过模型名称推断。两种 dialect 的 reasoning budgetTokens 和 topK 无相应字段；OpenAI enabled = true 需要明确非 none 的 effort，不能替调用者选择强度。DeepSeek candidates = 1 可满足但不发送 n，其他数量失败；parallelToolCalls 的显式设置也失败。JSON Schema 只在 OpenAI 映射，DeepSeek JsonObject 使用同一 output 设置。

request、message、part、tool 的 extensions 只合并到当前位置；Chat 客户端工具扩展位于 function／custom 定义对象，搜索工具扩展位于 web_search_options。扩展归属必须匹配，任何托管字段均不能覆盖，即使这次缺省没有生成该字段；保留的别名也不能旁路通用设置。错误 path 指向公共请求和扩展键，使用 JSON Pointer 转义。History 只接受同方言 assistant 消息且 context 为空，直接使用已投影的不可变 item；保留 DeepSeek 的全部 reasoning_content，不从展示文本重建。编码输入不修改保存的请求或历史。
