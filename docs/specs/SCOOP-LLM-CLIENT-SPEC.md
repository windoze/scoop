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

以上类型均可使用 companion codec 保存和恢复，构造检查同样适用于解码。token 计数入口随其操作合同定义；不能由响应 usage 推断。当前数据描述不提前查询模型目录，也不建立通用规则 DSL。

### 2.3 原生工具、服务端工具与混用

`ModelCapabilities.clientNativeTools: Capability<NativeToolCapabilities> = Unknown` 描述供应商定义、应用执行的工具。`NativeToolCapabilities(tools = [])` 的每项 `NativeToolCapability(dialect, kind, support, version = None)` 保存精确 dialect／kind／version 的 `Capability<NativeToolConfigurationCapabilities>`。`forTool(dialect, kind, version = None)` 查询完整组合；字符串按原值区分大小写，未列项为 Unknown。version=None 表示请求未显式选择版本、由当前 dialect 选择缺省，不覆盖任意显式版本。重复组合在构造及解码时拒绝，不依赖 support 是否相同。

`NativeToolConfigurationCapabilities(fields = [])` 保存 `NativeToolFieldCapability(name, support: Capability<Unit>)`，`forField(name)` 描述原生 configuration 的一个顶层 JSON 成员是否可显式提供。成员名精确匹配，重复名称拒绝，未列成员 Unknown。它不解释成员值、嵌套对象或任意约束表达式；值域、必要字段及具体组合由 dialect 的普通编码检查承担。Unsupported 成员即使值为 null／false／零／空集合也不能发送。扩展仍按原有作用位置由 dialect 解释，不假定所有扩展字段都属于 configuration。

`ServerToolCapabilities(webSearch = Unknown, nativeTools = Unknown)` 通过 `ModelCapabilities.serverTools = ServerToolCapabilities()` 提供服务端工具信息。webSearch 是 `Capability<WebSearchCapabilities>`，nativeTools 是 `Capability<NativeToolCapabilities>`，其 `forNativeTool(dialect, kind, version = None)` 使用同一精确查询。`ModelCapabilities.forClientNativeTool` 查询客户端原生表。父项 Unsupported 向下保留原因，Unknown 向下保持 Unknown，父项 Supported 不会补齐未列项。两种执行方、通用 WebSearch 表示及原生工具表示分别描述，不按相似名称互相推断；它们也不继承 functionTools／customTools。通用 WebSearch 的支持信息针对当前绑定的 dialect 所选映射，显式原生版本由 Native 表描述。

`WebSearchCapabilities` 分别保存 allowedDomains、excludedDomains、includeSources 的 `Capability<Unit>`，以及 location 的 `Capability<SearchLocationCapabilities>`，全部缺省 Unknown。SearchLocationCapabilities 的 country／region／city／timezone 同样独立。提供空域名数组、false 或空 SearchLocation 仍是显式请求，按相应能力检查；省略的选项不检查，也不因能力 Supported 自动填入选项。

`ModelCapabilities.mixedTools = MixedToolCapabilities()` 分别描述 clientAndServer 与 multipleServerTools，均缺省 `Capability<Unit>.Unknown`。前者约束同一请求同时声明至少一个客户端工具和一个服务端工具；后者约束声明多个服务端工具，包括同种类的多个实例，不表示工具实际并发执行。该组描述一般混用限制；Supported 不保证任意种类、选项或生成模式的具体组合都成立，实际 dialect 继续检查已掌握的约束。

普通和流式入口复用这些请求检查：原生配置失败报告 `/tools/<index>/tool/configuration/<escaped-name>`，WebSearch 选项报告 `/tools/<index>/tool/options/...`，混用限制报告 `/tools`；工具整体不支持报告对应 `/tools/<index>`。能力检查只拒绝明确 Unsupported，Unknown 继续交给 adapter／服务端。全部类型提供普通 companion codec；仅保存数据，不保存查询回调或注册插件。

## 3. 公共请求数据

`LlmRequest(input, system = [], tools = [], options = LlmGenerationOptions(), extensions = [])` 为不可变请求快照。input 必填，普通消息使用 `LlmInputItem.Message(content, role = LlmRole.User, extensions = [])`；同一个入口用于最小文本、多模态和工具请求。请求不保存 endpoint、model、认证或本次调用的时间戳，也不隐藏查询／下载／执行操作。

### 3.1 归属与按位置扩展

`ApiProtocolId(value)` 标识一种线协议，`DialectId(protocol, value)` 标识该协议内的方言。两者按字段值比较；字符串为不拆分、不改写的公开语义标识，实际 adapter 提供其身份，不通过供应商展示名或 endpoint 域名推断。创建标识本身不会注册或创建适配器。

`DialectExtension(dialect, value)` 的 value 必须为不可变 JSON object，非对象在构造时抛 IllegalArgumentException。request、Message、ContentPart 和 ToolDefinition 各自的 extensions 缺省空数组。同一个位置不能重复同一 DialectId；不同位置可分别使用同一归属，不继承或合并其字段。构造 LlmRequest 时检查这些局部重复，报告 `LlmRequestException(path, message)`，path 是请求模型中的 JSON Pointer 形式位置。实际位置是否适用、归属是否匹配当前 adapter，以及与管理字段的冲突，留给绑定后的一次协议请求检查；此处不猜测未知扩展键的语义。

### 3.2 内容与历史输入

`ContentPart` 提供 Text(text)、Image(source)、Document(source, filename = None)、Audio(source)、Video(source)、VideoFrames(frames) 和 Native(dialect, value)。前六者均有缺省为空的 extensions。Native 为所选方言明确规定的原生内容，其 value 由 adapter 解释，不与通用内容字段进行隐式覆盖。

`MediaSource` 区分 Bytes(bytes, mimeType)、Url(url, mimeType)、FileReference(reference, mimeType)。bytes 是不可变数组，url 使用 Url 值；`LlmFileReference(dialect, value)` 保留方言拥有的文件 ID／URI 及必要原生服务上下文。文件引用不意味着已经上传或支持该媒体；保存／恢复不下载资源、不转码、不把 URL 自动转成 bytes。MIME、媒体种类与形式的组合由实际请求归一化检查一次，不能只凭构造了一个 MediaSource 宣称可发送。

`VideoFrame(source, offset)` 使用非负 Duration 表示帧相对视频起点的位置；负偏移在构造时失败。帧顺序按输入保存，不自动排序、提取或补帧。

LlmRole 为 User、Assistant、System、Developer。system 数组与消息中的 role 均由 protocol 明确映射；库不依据模型名重写角色。`LlmInputItem.ToolResult(call, content, isError = false, extensions = [])` 表示应用执行后的结果；`ClientToolCallReference(dialect, toolKey, id: String? = None, context = None)` 保存方言、请求内工具引用、可选的服务器 call ID 和必要原生上下文。None 表示协议未提供 ID，不能以工具名、数组下标或新生成的字符串代替。是否必须有 ID 由具体协议规定；Chat、Messages、Responses 的工具结果仍要求非空 ID，在编码前给出 /input/{index}/call/id 错误。历史中的旧调用不要求在本次 tools 中重新声明；回传合法性由协议检查。

保存格式中的非空 id 继续编码为 JSON string，既有记录可直接解码；None 默认省略 id，缺失或 null 解码为 None，context 不因 ID 缺失而丢弃。工具输入是否可执行依据已完成的协议解释，不能把公共 ID 的 None 自动等同输入不完整。此差异有实际协议依据：[Gemini FunctionCall.id 是可选字段](https://ai.google.dev/api/generate-content#FunctionCall)。

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

function 工具调用按本次请求中唯一的同名 ClientFunction 关联 ToolKey，custom 调用按唯一同名 ClientCustom 关联。未知类型、未声明／无法唯一关联的工具调用保留为 Native，不进入可执行列表，不根据名称猜成服务端或客户端执行。合法外层调用中的 arguments 无效 JSON／非 object 保存为 Invalid；输出被截断、拒绝或停止状态未知时保留 Incomplete，即使已返回的参数恰好可解析也不进入待执行列表。已知 function／custom 调用的 ID 必须非空，同一候选内已关联的客户端调用 ID 必须唯一。其他外层类型、必填字段或数值错误抛 LlmProtocolException，保存 dialect、path、message 与原始响应。弃用的 function_call 无法提供现代工具 call ID，保留 Native，不伪造 ID。

stop 与 tool_calls／function_call 表示完整的该轮模型输出；length 表示 Incomplete；content_filter 在没有明确 refusal 时同样为 Incomplete；明确 refusal 为 Refused。未知停止原因使用 Other(raw) 并保守标记 Incomplete，不能据此返回结构化成功。每个候选的 history 只包含 assistant 消息；不包含 choices 包装、logprobs、usage 或服务器响应时间。

OpenAiChatDialect 的标识为 (chat-completions, openai-chat)，DeepSeekChatDialect 为 (chat-completions, deepseek-chat)。两者保留消息中的未知字段，去掉已知仅供输出的 annotations；audio 回传只保留其 id，不回传 data、transcript 或 expires_at。原始音频、annotations 与弃用 function_call 仍保留为 Native 输出与完整原生响应。DeepSeek 将 reasoning_content 单独投影为 Reasoning，并在 history 中完整保留，即使该轮没有实际调用工具；后续请求编码按是否携带 tools 应用回传规则。OpenAI 不把未知的 reasoning_content 猜成其已声明的 reasoning 输出。

两种 dialect 均从 prompt_tokens、completion_tokens、total_tokens 与 completion_tokens_details.reasoning_tokens 读取明确计数，缺失总量不自动补齐；audio_tokens 按输入／输出归属进入模态细项。OpenAI 从 prompt_tokens_details.cached_tokens 与 cache_write_tokens 分别读取缓存读写。inputTokens、cacheReadTokens 与 cacheWriteTokens 均已知时，nonCachedInputTokens = inputTokens - cacheReadTokens - cacheWriteTokens；缺失写入量不能默认为零，不能把写入量再次计入普通输入。已知读写之和不得超过输入总量，计算不得溢出；TTL 细项仍保存在 native。DeepSeek 优先读取 prompt_cache_hit_tokens、prompt_cache_miss_tokens；cached_tokens 是同义读缓存字段，两者并存时必须一致，OpenAI 的 cache_write_tokens 不参与 DeepSeek 解释。出现的计数必须非负，缓存读数不得超过已知输入总数。不把 cache 或 reasoning 再加进总量，也不从文本长度猜 token。完整响应中的 usage 记录为 Final，未返回的计数仍为 None。依据：[OpenAI 缓存读写统计](https://developers.openai.com/api/docs/guides/prompt-caching)。

协议依据：[OpenAI Chat](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[DeepSeek Chat](https://api-docs.deepseek.com/api/create-chat-completion/)、[DeepSeek thinking](https://api-docs.deepseek.com/guides/thinking_mode/)。请求编码见第 7 节；生成传输、模型能力联动与流式 reducer 继续按实施记录交付。

## 7. Chat 请求编码

`ChatProtocol.encodeRequest(modelId, request, stream = false)` 将公共请求编码为不可变 JSON object，不发网络请求。modelId 按原字符串传递，不用于选择协议或猜测能力；空 modelId 与没有消息的请求报告 LlmRequestException。system 内容在 input 前形成一条 system 消息；普通消息按 role 编码。最小调用只发送 model、messages，未提供的生成字段不补供应商默认值。stream 为 true 时发送 stream 和 include_usage，完整响应模式省略它们。模型 capability 的联动属于后续模型绑定入口。

ChatDialect 增加 encodeOptions(request) 和 encodeContent(content, role, path)；前者处理生成选项及工具在请求根部的映射，后者处理当前 role 的内容。protocol 固定管理 model、messages 和流模式，不允许 dialect 选项覆盖它们。通用选项无法精确表达时在对应公共输入路径报错，不静默删除。OpenAI 的 maxOutputTokens 映射 max_completion_tokens，DeepSeek 映射 max_tokens；两者不以 model ID 改写字段。

OpenAI 支持 User 的图片 bytes／http(s) URL、PDF bytes／本方言文件 ID、WAV／MP3 音频 bytes；音频 URL、图片文件 ID 和视频没有这套 Chat 通用映射。文件 ID 在 LlmFileReference.value 中为 JSON string。PDF bytes 以 data URL 放入 file.file_data，文件名仅用于 bytes；不下载远端文件。图片仅使用 JPEG、PNG、GIF、WebP。DeepSeek 的 User／Tool 支持这组图片的 bytes／URL／本方言文件 ID；文件 ID 编码为顶层 file_id 的 file part，不将其当成 PDF 支持。DeepSeek System／Assistant 仅接受无 part 扩展的文本，Developer 没有对应角色。OpenAI 非 User 消息只接受文本通用 part。Native part 必须属于当前 dialect 且为 object，按原值保留。

普通单个无扩展 Text 编码为字符串，其他允许的内容序列编码为 parts；需要纯字符串的角色按顺序连接文本。MIME 使用现有 HttpMediaType 解析一次，当前映射要求不带参数的明确格式，不忽略无法表示的参数。不自动下载、转码、丢弃 part 或把 VideoFrames 转成无时间信息的图片。已知 DeepSeek 外部图片 URL 超过 8192 字符时报告错误。

ClientFunction／ClientCustom 分别编码 function／custom 工具；DeepSeek 仅支持 function。function 名称在协议允许的 ASCII 字母、数字、下划线和横线范围内检查，OpenAI 上限 64、DeepSeek 上限 128；custom 名称不套用未经声明的 function 限制。同次调用 wire 名称不得重复。ToolChoice.Specific 按 ToolKey 找到精确 wire 工具，不用 key 替代 name。当前 Chat 没有已定义的 ClientNative 合同，不能假装成 function。ToolResult 使用原始 call ID 和 tool role；旧调用无需再次声明，其 dialect、context 和内容必须适合 Chat。Chat 没有通用 isError 标志，显式 isError = true 报错，由应用将实际错误内容作为结果提供。

OpenAI 的 Server(WebSearch) 编码请求级 web_search_options，可映射 approximate location；它不伪装成客户端工具。该入口不支持通用 allowedDomains、excludedDomains 或 includeSources 控制，显式提供即报错。只允许一项搜索声明；选择 Auto，其他 ToolChoice 无法同时精确控制该搜索入口时失败。Server(Native) 仅接受本 dialect 的无版本 web_search 原生配置；DeepSeek Chat 不将独立 Search API 假装成内置工具。编码搜索请求不伪造服务端工具执行记录。

DeepSeek reasoning 缺省按服务文档为开启；enabled 与 effort 冲突时拒绝。temperature 只在显式关闭 thinking 时可用，Required／Specific 也要求关闭 thinking。topP 的生效模式必须显式配置：`DeepSeekChatDialect(topPMode = DeepSeekTopPMode.Unknown)` 与 `Providers.deepSeekChat(topPMode = Unknown)` 缺省不猜模型行为，显式请求 topP 时报告路径错误；Thinking 模式允许 thinking 中的 0.95～1，NonThinking 模式允许关闭 thinking 时的 (0, 1]。当前官方文档分别为 deepseek-flash 和 deepseek-v4-pro 声明这两种行为；模式由应用的模型资料明确选择，不按 model ID 切换。未提供 topP 时无须配置模式。两种 dialect 的 reasoning budgetTokens 和 topK 无相应字段；OpenAI enabled = true 需要明确非 none 的 effort，不能替调用者选择强度。DeepSeek candidates = 1 可满足但不发送 n，其他数量失败；parallelToolCalls 的显式设置也失败。JSON Schema 只在 OpenAI 映射，DeepSeek JsonObject 使用同一 output 设置。

request、message、part、tool 的 extensions 只合并到当前位置；Chat 客户端工具扩展位于 function／custom 定义对象，搜索工具扩展位于 web_search_options。扩展归属必须匹配，任何托管字段均不能覆盖，即使这次缺省没有生成该字段；保留的别名也不能旁路通用设置。错误 path 指向公共请求和扩展键，使用 JSON Pointer 转义。History 只接受同方言 assistant 消息且 context 为空，直接使用已投影的不可变 item；保留 DeepSeek 的全部 reasoning_content，不从展示文本重建。编码输入不修改保存的请求或历史。

## 8. Provider、部署与模型绑定

内置 protocol 与 dialect 的普通构造器公开可用，包括 OpenAiChatDialect() 与 OpenAiResponsesDialect()；独立 Cone 可直接组合适配器或读取 dialect.id，不必先创建 provider。构造器可见性遵守语言规范 9.1.5 的显式声明规则，不从 public class 自动继承。

`LlmGenerationProtocol` 描述普通 HTTP JSON 生成：dialectId、generationUrl(baseUrl, modelId)、encodeRequest(modelId, request, stream)、decodeResponse(value, request) 与 decodeError(status, body, truncated)。ChatProtocol 实现该合同并固定其 ChatDialect。接口没有实时会话或尚无实现的操作占位；流式 reducer 随相应独立合同补充。

`LlmProvider(name, protocol, defaultBaseUrl = None)` 固定一个协议／方言组合。`Providers.openAiChat()` 与 deepSeekChat() 分别提供已实现的两个组合及官方 URL 缺省。`LlmEndpoint(provider, authentication, baseUrl = None, fields = [])` 绑定部署；缺少官方缺省又未传 URL 时构造失败。URL 必须是绝对 http(s) 地址且无 fragment，Chat 路径在现有 base path 后追加 chat/completions，保留 query；不把 base path 当成要被替换的文件名。

`endpoint.model(id, capabilities = None)` 创建不可变 LlmModel，保存 endpoint、原始 model ID 与能力；省略的能力使用 ModelCapabilities()，不联网查询目录。空 ID 失败，斜杠／冒号按模型数据保留，不解析年代或切换协议。相同 model ID 在不同 endpoint／provider 中形成独立绑定。认证、client 和模型绑定不提供消息 codec，不把密钥随历史保存。

`LlmAuth` 的 suspend fields(request: LlmAuthRequest) 返回本次认证字段。LlmAuthRequest 是最终 POST 的 method、Url、现有字段与不可变 body 字节；认证每次尝试重新调用，签名和发送共享同一 body。LlmAuth.none() 不增加字段，bearer(token) 产生 Authorization；自定义认证实现同一公开接口。认证字段不得与 endpoint 的同名字段重复，字段合法性复用 HttpField；不自动读取环境变量或替换借入 HTTP client 的身份。最终发送前的 HTTP 自定义 hook 仍由 HTTP 调用者负责，不把用户代码视为隐式签名保证。

## 9. 普通生成与调用寿命

`LlmClient.open(scope, httpClient = None)` 是 suspend 工厂，绑定当前 EventLoop 与给定活动 CoroutineScope。未传 HTTP client 时创建并拥有一个，传入时借用；close 取消并等待本 client 的调用及完成回调，关闭拥有的 HTTP client，保留借入者。重复关闭幂等，并发关闭等待同一轮收尾；从本 client 的生成／认证／完成回调中等待自身 close 明确失败。取消 owner scope 也启动相同收尾，不以 GC 替代网络资源关闭。

`withLlmClient(httpClient = None, block)` 只组合 open、useAsync 与 coroutineScope。`generate(model, request, options = LlmCallOptions())` 进行一次模型生成，返回 LlmResult；不运行工具、不建立 Agent 循环。它先记录调用开始，再检查已知模型能力和当前 adapter 能否表达请求。明确 Unsupported 的输入／控制在对应公共请求路径失败；Unknown 交给 adapter／服务端判断。可直接观察的媒体字节数及请求内数量遵守已知上限，不读取媒体来猜页数、分辨率或 token 数；Native 与原生 history 不伪装成已验证的通用模态。

LlmCallOptions 保存 timeouts、retries、maxResponseBytes（缺省 64 MiB）、maxErrorBytes（缺省 64 KiB）与可选 suspend onFinished(metadata)。两个大小上限只约束当前响应的实际内存读取，不构成通用资源预算。成功 JSON 使用严格 UTF-8 及现有 Json decoder；非 2xx 读取有限错误前缀并保存截断事实，随后关闭响应，不无界 drain。错误正文不要求合法 JSON。

LlmTimeouts 的 responseHeaders／readIdle 缺省 300 秒、writeIdle 缺省 60 秒、total 缺省 None；各项 None 明确关闭。排池、DNS、TCP、TLS 与代理握手继承 http.defaultTimeouts 的相应值。total 覆盖准备、认证、重试等待与生成，超时以 LlmTimeoutException 报告并等待真实清理。普通 HTTP 重试、redirect、Cookie、HTTP cache 与 origin 自动认证显式关闭；代理、TLS、连接池与原 observer 保留。

LlmRetryOptions 的 maxRetries 缺省 2，初始退避 100 毫秒，上限 30 秒；只重试实际非 2xx 响应经 dialect 明确判断为服务拒绝且关闭成功的结果，不重试连接结果不明、格式错误、取消或已经取得成功响应后的读失败。认证和 HTTP 用户回调抛出的异常不因类型为 LlmServiceException 就触发生成重试。退避采用指数增长，Retry-After 使用 HTTP 库解析；不会早于服务器要求的时刻重试。Chat 当前将 DeepSeek 的 429／503 和 OpenAI 明确 rate_limit_exceeded 的 429 归为可重试；quota 错误与未知服务错误不根据 5xx 类别猜测可重试。

每次尝试保存独立 LlmAttemptMetadata，未到发送点的尝试没有 sentAt。请求与最终响应头时间直接使用 HTTP 观察快照；1xx 不充当 receivedAt。完整响应解析后采样 completed；有非空输出时同时取得该完整响应的 firstOutput，普通文本非空时取得 firstText；不声称这些非流式采样是服务端逐 token 生成时刻。底层未提供 UTC 首字节／body 完成点时保持 None，不以回调执行时间冒充。

onFinished 在实际收尾后执行一次，成功、输入检查失败、服务拒绝、超时、调用者取消和关闭均得到独立快照；取消保持 CancellationException。回调在不受原取消阻断的收尾作用域中执行，不运行在共享 HTTP 驱动内。回调失败照常向调用者报告；已有操作失败时使用 ResourceUseException 保留两者，metadata 中的 outcome 仍描述已完成的生成／收尾事实。失败不返回伪造 LlmResult，已观察的时间与用量通过回调取得。

## 10. SSE 解码与 Chat 增量状态

`StreamRetention.EventsOnly` 缺省只保留协议身份、未完成工具输入、当前 usage 及结束状态；普通文本／reasoning 不累计，完成的工具参数在交付事件后释放。`Result` 显式保留完整候选和可恢复历史，不保存已处理的完整事件日志。两种模式使用同一 decoder 和同一套检查。

`LlmStreamProtocol` 扩展普通生成合同，提供缺省调用 generationUrl 的 `streamUrl(baseUrl, modelId)`；SSE 与 AWS EventStream 分别由 LlmSseProtocol、LlmEventStreamProtocol 描述。前者提供 `createStreamDecoder(request, retention = EventsOnly)`，后者提供 `createEventStreamDecoder`。两种 decoder 共享 LlmStreamDecoder 的状态、endInput 与 response 合同，各自接收已经定界的 SSE data 或 AwsEventStreamMessage。普通与流式路径不同的协议显式覆写 streamUrl，传输层不按模型名或服务商猜测路由。普通生成只用 generationUrl；认证接收选择后的最终 URL 和已编码正文，发送不再次改写地址。

`LlmSseDecoder.accept(event, data)` 接收已经由 SSE parser 定界的事件，返回 `LlmStreamUpdate(events, hasOutput, hasText, isKeepAlive = false)`；不执行 I/O。isFinished、responseId、usage 提供当前事实。`endInput(): LlmStreamUpdate` 检查 EOF 并交付只在输入结束时确定的终止／usage 事件；正常返回必须已经 isFinished，否则传输层报告协议错误。要求显式终止事件的协议仍在缺少该事件时失败，已正常结束时返回空更新。response() 仅在 Result 模式、正常终止且未失败时返回完整 LlmResponse。错误后 decoder 不继续接受数据，也不能读取伪造的成功结果。

`LlmEvent` 包括 TextDelta、ReasoningDelta、RefusalDelta、ToolInputDelta、ClientToolCall、ServerToolActivity、Citation、CandidateFinished、Usage 与 Native；Messages 的服务端活动与引用按第 14 节交付。LlmItemLocation 使用协议 candidate index 和候选内首次观察到的 item index；LlmTextDelta 另带 partIndex，Chat 的各个文本字段各占一个单 part item。事件和最终结果使用同一位置，不把多个候选拼接。ToolInputDelta 的 reference 在身份明确且能关联到请求中的客户端工具后才提供；它本身不保证完整输入可执行。完成调用复用普通 Chat 工具解释，无效／截断参数分别为 Invalid／Incomplete。

ChatProtocol 实现该合同。ChatDialect.streamedReasoningField 指明可按字符串片段拼接的 reasoning 字段；OpenAI 为 None，DeepSeek 为 reasoning_content。content／refusal／reasoning 与 function.arguments／custom.input 按到达次序拼接；工具用 candidate＋tool index 分开关联，id、type 和 name 为不变身份，允许重复相同值，不允许中途替换。未知工具不得推断为客户端执行。audio.data 的每个 Base64 片段先解码为字节，再按顺序合并并编码到保留结果，不能直接拼接带 padding 的 Base64 字符串；transcript 拼接、id 保持不变、expires_at 保存实际终值。弃用 function_call 的名称与参数保留为原生内容。

每个 chunk 的 choice index 必须非负且不重复，跨 chunk 的 id／model／created 若重复出现须一致。role 出现时只能为 assistant，未出现时按该输出协议的 assistant 角色构造历史。finish_reason 非 null 时完成该候选；此后不得再追加 delta，空 delta 的相同 finish_reason 可承载尾部 usage。未知停止原因保留 Other 并标为输出 Incomplete，不等同传输失败。

兼容服务的无候选注释帧可以使用空 id／model、created=0 且不含 usage；这一完整组合不声明响应身份，作为 `chat.annotation` Native 事件完整保留，不设置 createdAt，也不视为有效输出或终止。普通生成帧的真实身份变化仍失败，真实响应明确报告的 created=0 仍是有效 Unix epoch。该形状依据 [Azure Prompt annotation message](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/content-streaming#prompt-annotation-message) 及实际代理响应；不因此宣称支持 Azure 的全部异步过滤／部署功能。

OpenAI 的独立 usage-only chunk 和 DeepSeek 在最后一个 choice 同时报告的 usage 均被读取；计数是累计快照，已出现字段更新同字段，缺省／null 不擦除前值，嵌套明细按字段合并，不累加快照。usage 在正常 `[DONE]` 后标 Final，之前报告的快照保持 Partial；当前已观察候选均结束不能证明后续没有新候选或 usage。没有报告的计数仍为 None。只有 `[DONE]` 且至少一个候选、所有已观察候选均有 finish_reason 才正常终止；不能在某个 finish_reason 或普通 EOF 时提前成功。协议错误事件、根部 error、缺失终止或终止后继续输入明确失败，不进行自动恢复或重试。

Native 事件保留无法通用解释的字段／事件及其位置；Result 将这些必要片段保存在新增的 `LlmResponse.nativeEvents = []`，不冒充服务端一次完整 JSON 响应（流式 response.native 为 None）。未知消息／工具 delta 无法无损归并到可回传消息时，该候选的 history 为 None；原始片段仍可读取，toInput 报 HistoryUnavailable，不静默丢掉可能影响回传的字段。已知 audio、reasoning 与工具字段按已声明规则构造历史。

协议依据：[OpenAI streaming events](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events)、[DeepSeek Chat](https://api-docs.deepseek.com/api/create-chat-completion/)、[DeepSeek thinking](https://api-docs.deepseek.com/guides/thinking_mode/)。网络流的拥有关系、单消费者与收尾按同一 client 寿命合同接入，不能在纯 decoder 中启动后台读取。

## 11. 流式网络寿命与读取视图

`client.openStream(model, request, options = LlmCallOptions(), retention = EventsOnly)` 返回 `LlmStream : AsyncCloseable`；`withStream` 使用相同参数加 suspend block，仅组合 openStream 与 useAsync，返回 block 的值。打开前检查 streaming capability；明确 Unsupported 或协议没有已支持的流式合同时在认证／发送前失败，Unknown 不被当作不支持。SSE 成功响应必须为 text/event-stream，AWS EventStream 必须为 application/vnd.amazon.eventstream。服务拒绝的认证、有限错误读取与重试沿第 9 节，取得成功响应后的任何读取／解析失败均不重试，也不重新连接。

流属于 client 所在 EventLoop，并由 client 跟踪至实际清理和 onFinished 完成。打开过程受打开者取消控制；打开返回后由流的显式寿命接管。`next(): LlmEvent?` 的调用者取消会取消该生成，等待真实清理后保持 CancellationException。流只在消费者请求下一条事件时读取，允许相应定界 source 的有限缓冲和一个已解码事件批次；不后台预读完整输出。maxResponseBytes 约束读取的已解码 HTTP 正文字节（含 SSE／EventStream framing 与有限 read-ahead），超限沿用 ReadLimitException；SSE 单行和事件数据的上限也使用该值，避免额外隐含的较小限制。

`textDeltas()` 取得唯一的 `LlmTextStream` 读取视图，其 next 返回带位置的 LlmTextDelta。取得视图即选择文本入口；首次原始 next 选择原始入口，此后不能切换、重复取得视图或并发 next。文本入口驱动同一 reducer，过滤非普通文本事件但完整处理工具、usage、结束和错误。None 只表示正常协议终止；提前关闭、取消和失败不冒充 EOF。

metadata 每次返回不可变 LlmCallMetadata 快照。首个成功解释的非保活协议事件、首个有效输出和首个普通文本在实际处理该事件时采样，HTTP 发送／响应头仍取底层观测；无法观察的时间保持 None。正常协议终止记录 completed 与最终 usage，并停止 total 截止，随后消费者到显式关闭之间的等待不触发生成超时。此时 metadata 仍为 Active；实际资源清理后才记录 finished、最终 outcome 并调用一次 onFinished，包括打开失败。回调不能等待本流或所属 client 的 close。

正常 EOF 的 endInput 更新经过同一事件交付、结果保留、usage／completed 采样和截止取消路径；先交付其事件，再返回读取结束。EOF 自身不是模型事件，不设置 firstEvent。若 endInput 失败，保留已观察的部分用量和输出，不记录成功 completed，不取得完整结果，也不重试。

close 不自动消费剩余输出：未达终止时取消并等待收尾（outcome=Cancelled），已经终止时正常清理；重复关闭幂等。client.close 对已经终止的流正常清理，对未完成的流以 LlmClientClosedException 终止，并等待回调。读取失败由 next 交付后，useAsync 的 close 不重复交付同一失败；尚未交付的超时、拥有者关闭或回调失败由 close 交付。操作和回调都失败时以 ResourceUseException 保留两者。主动提前 close 自身不抛用于中止读取的 CancellationException。

`result()` 为关闭后的纯读取，要求 Result 保留、协议正常终止、资源清理及回调成功，否则报告原失败或 LlmOutputException（Incomplete／ResultNotRetained）。所有模式关闭后仍可读取 metadata；block 内旧快照不被改写。关闭前不能取得伪造的最终结果。

## 12. Messages 响应解释

`MessagesProtocol(dialect: MessagesDialect)` 固定使用 ApiProtocolId("messages")，拒绝其他协议归属。`AnthropicMessagesDialect` 的身份为 DialectId(ApiProtocolId("messages"), "anthropic-messages")。本节规定纯 `decodeResponse(value, request)`；普通生成按第 13 节，SSE 按第 14 节。MessagesDialect 分别负责 readReasoning(block, path)、readUsage(usage)、historyMessage(message, path) 及 decodeError；不共享调用状态。

完整响应的 type 必须为 message、role 必须为 assistant，content 是有序 block 数组，stop_reason 必须为非 null 字符串。单个响应产生 index=0 的候选；缺失的 id、model、usage 保持未知，createdAt 不从本地时钟补造。已知 block 验证其必要字段，失败报告 LlmProtocolException 的 JSON Pointer 路径和完整原文；未知 block 保存为 Native。text 投影为一个 Message，thinking／redacted_thinking 投影为 Reasoning；普通文本视图不混入 reasoning。

history 仅选取 role 和未经重建的 content，不携带顶层 id、usage、stop_reason 等响应元数据。完整保留 thinking.signature、redacted_thinking.data、工具输入、搜索的 encrypted_content／encrypted_index、citations 及未知 block／字段。库不解析或验证签名，不过滤空 thinking，不按模型名裁剪历史。序列化后的 toInput 使用同一份历史。依据：[thinking 回传](https://platform.claude.com/docs/en/build-with-claude/thinking)、[web search 回传](https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool)。

tool_use 按 name 唯一关联本次 ClientFunction，或同方言 ClientNative 的 configuration.name；带 toolset_name 的成员按第 13 节关联无 name 的原生 toolset。不匹配 ClientCustom 或 Server，无法唯一关联时保留 Native。function 输入从原生 JSON object 生成 JsonToolArguments，native 客户端输入保存为 Native object；非 object 为 Invalid，生成不完整时为 Incomplete，均不可执行。call reference.context 保存该调用原始 block，以保留 caller 等原生关联。客户端和服务端调用 ID 不能在同一响应中重复；ID 来自服务器，不从工具名称或 block 下标生成。

server_tool_use 独立产生 ServerToolActivity，绝不进入客户端执行列表。WebSearch 的 use block 为 InProgress，后续 web_search_tool_result 产生相同服务器 ID 的 Completed／Failed 活动；两项分别保留原始 block 位置，不把后来的结果搬到较早的位置。成功的空结果数组仍为 Completed，已知错误 object 为 Failed，未知结果 object 保持 Unknown。结果可以属于前一轮的服务端调用，此时不要求本响应中已有 use block，也不补造调用；能从当前响应确定名称时才关联请求 ToolKey。其他服务端 use 保留 Native kind 和原文；尚无通用结果解释的 block 保留 Native。依据：[server tool 生命周期](https://platform.claude.com/docs/en/agents-and-tools/tool-use/server-tools)。

web search 来源和文本中的 web_search_result_location 引用提供 URL、可选 title 与原文；没有原生来源 ID 时以响应内字段路径作为来源表 ID，不能用作工具调用 ID。其他引用保存 Native 来源。Citation.range 保持 None：document 的字符／页码范围指向源文档，不是生成文本中的范围，不得误填为 TextRange。搜索活动、来源和引用可通过普通响应 codec 保存。

end_turn／stop_sequence 映射 Stop，tool_use 映射 ToolCalls，均为 Complete；max_tokens 和 model_context_window_exceeded 为 Length／Incomplete；pause_turn 为 Pause／Incomplete；refusal 为 Refusal／Refused；未知值为 Other(raw)／Incomplete。任何暂停、拒绝或截断都不自动继续或重试。依据：[停止原因](https://platform.claude.com/docs/en/build-with-claude/handling-stop-reasons)。

Anthropic input_tokens 是非缓存输入；cache_read_input_tokens、cache_creation_input_tokens 为独立输入部分。三项都报告时才相加得到 inputTokens，再与已知 output_tokens 相加得到 totalTokens；缺失项不补零。output_tokens_details.thinking_tokens 归为 reasoningTokens；缓存 TTL 明细、服务端搜索次数和其他非 token 资料留在 usage.native。所有计数必须为非负 Long，归一化求和检查溢出。usage.Final 表示报告终结，不表示每项已知。依据：[缓存计数口径](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)、[Messages API](https://platform.claude.com/docs/en/api/messages/create)。

错误正文保留有界 bytes、解析成功的 native、error.type 与 error.message；截断或非 JSON 正文仍可报告 HTTP 失败。只有 HTTP 429／529 且对应 rate_limit_error／overloaded_error 才取得可重试资格，实际重试仍受第 9 节限制。HTTP 200 内的搜索失败是服务端工具结果，不是整个请求的 HTTP 错误。

## 13. Messages 请求与普通生成

`Providers.anthropicMessages(version = V20261021, defaultMaxOutputTokens = 4096L)` 包装 MessagesProtocol 与 AnthropicMessagesDialect，缺省 base URL 为 https://api.anthropic.com/v1。`AnthropicMessagesVersion` 明确区分 V20230601 与 V20261021，决定 anthropic-version 及已知版本规则，不按模型 ID 推断。max_tokens 为必需字段；请求省略 maxOutputTokens 时使用非负的 dialect 缺省，显式值（含零）覆盖它。默认上限也要通过模型的已知 maxOutputTokens 检查，不静默缩小请求或改变模型绑定。普通 generate 与第 14 节的 openStream 共用该默认值与能力检查。

`LlmGenerationProtocol.generationFields()` 缺省返回空数组，defaultMaxOutputTokens() 缺省 None。前者提供该协议／方言的固定请求头，后者用于能力检查中的有效缺省上限。LlmEndpoint.fields 组合固定头与应用字段，应用不能重复固定头名称而造成版本与编码规则不一致；认证看到组合后的真实字段。Chat 的空头和省略上限行为保持不变。MessagesDialect 提供 requestFields、defaultMaxOutputTokens、encodeOptions 与 encodeContent 的协议专用合同；MessagesProtocol 在 base URL 后追加 messages，并复用公共 HTTP 调用、重试、计时和完成通知。

顶层 system 保存为 text block 数组；input 的 User／Assistant 映射同名 role，System 保持原位置的 system role，Developer 没有映射。带内容的 System 不能是首条消息；初始指令使用 request.system。并非所有模型支持中途 system／assistant prefill，Unknown 不按模型名猜测。普通内容支持文本、JPEG／PNG／GIF／WebP 图片的 bytes／HTTP URL／同方言文件 ID，以及 PDF 的相应来源；text/plain 文档支持 UTF-8 bytes。system 只接受文本，直接 assistant 文档在新版失败；音频、视频和带时间戳的视频帧没有该协议的通用映射。Document.filename 不冒充供应商 title；需要标题时使用该 content part 的 title 扩展。

ToolResult 编为 user 消息中的 tool_result block，使用原始 call ID、content 和显式 is_error；连续 ToolResult 合并到同一 user 内容数组以保留并行工具结果。block 的扩展仍留在 block，不提升到 user 消息。旧调用不要求本轮重新声明工具。Messages call reference.context 保存的原始调用供应用读取；tool_result 线协议只需要其 ID，不把整个调用 context 发回结果字段。History 只接受同方言的 assistant 消息，保留原始 content；独立 history.context 没有隐式映射。

ClientFunction 映射 name、description、input_schema 与可选 strict，ClientCustom 没有对应映射。NativeToolDefinition 的 kind 加可选 `_version` 构成原生 type，configuration 中的 name 若出现须为非空字符串且不能覆盖 type；客户端／服务端归属保持显式。相同 wire name 的工具在发送前失败。无 name 的原生 toolset 也可发送，但 Specific(ToolKey) 不能代替其成员选择；响应的 toolset_name 与同方言、无 name 的 ClientNative.kind 唯一匹配时关联其 key，成员 name 留在调用 context，不与同名 ClientFunction 混淆。未知原生种类可按该显式表示发送，但已知 bash／text_editor／computer／browser／memory 不能声明为服务端，已知 web_search／web_fetch／code_execution／tool_search 不能声明为客户端。显式版本不被替换；通用 WebSearch 固定使用基础 web_search_20250305，其他版本使用 Native 声明。

WebSearch 将 allowedDomains／excludedDomains 映射 allowed_domains／blocked_domains，两者不得同时出现；location 映射带 type=approximate 的 user_location，必须至少提供一个字段。基础搜索固定返回来源结果，因此 includeSources=true 保持此行为，false 不能表达而报错。不能用扩展覆盖通用管理字段。Auto 缺省省略，None／Required／Specific 分别映射 none／any／tool；parallelToolCalls 通过 tool_choice.disable_parallel_tool_use 的反值精确表达，None 选择下不能同时设置并行开关。

新版明确拒绝 temperature／topP／topK；旧版分别映射 temperature／top_p／top_k，温度和 topP 范围为 0～1。单候选允许 None／1，其他值失败。stop 保持显式数组。reasoning.enabled=true 且未指定预算时使用 adaptive；显式 budgetTokens 使用 enabled 并要求至少 1024 且小于有效 max_tokens，不能与 enabled=false 共存。effort 独立映射 output_config.effort；手工预算模式不能强制选择工具。模型是否接受某种 thinking 模式仍由其 capability／服务端决定。adaptive 不保证每轮出现 thinking block；模型没有返回时不补造，返回时完整保留，见 [thinking 的逐轮选择](https://platform.claude.com/docs/en/build-with-claude/thinking-steering-and-cost)。

JsonSchema 映射 output_config.format 的 json_schema／schema；默认 name=response 无需线字段，其他名称和 description 没有对应位置；strict=true 由该格式自身保证，false 无法表达。JsonObject 没有等价开关。request、message、part、tool 的扩展只合并到各自位置，保留未知字段；已管理字段即使缺省省略也不能被扩展覆盖。Native content 是同方言原生 block，不触发媒体下载、转码或工具执行。

协议依据：[API 版本迁移](https://platform.claude.com/docs/en/api/migration-2026-10-21)、[Messages 使用和媒体来源](https://platform.claude.com/docs/en/build-with-claude/working-with-messages)、[文件来源](https://platform.claude.com/docs/en/build-with-claude/files)、[工具定义](https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools)、[thinking](https://platform.claude.com/docs/en/build-with-claude/thinking)、[结构化输出](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)。

## 14. Messages SSE

MessagesProtocol 实现 LlmSseProtocol，沿用第 11 节的网络寿命、读取入口与保留策略。每个 SSE data 是含 type 的 JSON object；显式 event 名须与 type 相同，空名或默认 message 由 data.type 分派。message_start 只出现一次，其 message 必须为 assistant、content 为空、stop_reason 未定；ID、模型和初始 usage 取实际字段。content block 按从零递增的 index 建立稳定 item 位置；delta／stop 只能引用已开始而未结束的 block，允许不同 block 的事件交错。LlmStreamUpdate 增加 isKeepAlive=false；Messages ping 返回 true，传输层不以它采样 firstEvent。firstEvent 只记录成功解释的非保活事件，损坏 JSON 不冒充已解码的模型事件。

text_delta、thinking_delta、signature_delta 分别拼接相应字段，签名保持不透明；citations_delta 逐项追加 citation。新增 LlmEvent.Citation(location, partIndex, citation) 与 ServerToolActivity(location, activity)，使引用与服务端活动在 EventsOnly 下也能读取。初始 block 内已有的文本与引用同样交付，签名／redacted thinking 通过 Native 事件可见。搜索调用与结果沿第 12 节分别交付共享 ID 的活动，不进入客户端工具执行列表。

tool_use／server_tool_use 的 input_json_delta 拼接 partial_json；初始 input={} 是流式参数的占位起点，有 delta 时用拼接结果替换，而非与 `{}` 连接。非空初始 input 不得同时追加流式参数。客户端调用在 block_stop 后保留必要参数，直到 message_delta 给出最终 stop_reason 才交付 ClientToolCall，防止随后 max_tokens／pause_turn 被误报为可执行调用。有效 JSON object 沿普通响应规则解释，损坏 JSON 为 Invalid，非完整候选为 Incomplete；无效原文通过事件和 ClientToolInput 保存，不能伪造合法 history。服务端输入不能解释为 object 时按协议错误报告。

message_delta 的 usage 是累计快照，与 message_start 按字段递归合并；未出现或 null 不擦除前值，不把快照相加。首次非 null stop_reason 要求所有 block 已结束；之后仅允许相同停止原因与尾部 usage／元数据。CandidateFinished 在该点交付，usage 仍为 Partial，只有 message_stop 才标 Final 并结束流。没有 block 的正常零输出响应也合法。重复 start／stop、停止原因变化、结束后内容、错误事件或缺失 message_stop 的 EOF 均失败，不自动重试或接续。

EventsOnly 不累计普通文本、thinking、签名或已完成的服务端结果；仅保留当前块状态、待确定完成性的客户端输入、调用关联和 usage。Result 以同一 reducer 保存完整 item 和原生 history，正文与签名仅按协议规则重建；不保存已知文本 delta 的完整日志。未知事件／字段保持 Native；未知 block 无 delta 时可原样回传，无法归并的未知 delta 则使该候选 history 不可用，并保留其开始状态与片段以供读取。流式 LlmResponse.native 为 None，不把重建的消息冒充一次完整服务响应。

协议依据：[Messages 流式事件](https://platform.claude.com/docs/en/build-with-claude/streaming)、[细粒度工具参数](https://platform.claude.com/docs/en/agents-and-tools/tool-use/fine-grained-tool-streaming)、[流式引用](https://platform.claude.com/docs/en/build-with-claude/citations)。

## 15. Responses 完整响应解释

ResponsesProtocol 接受 ResponsesDialect；OpenAiResponsesDialect 的标识为 (responses, openai-responses)。decodeResponse 接受 object=response 的正文，将 output 的实际顺序投影到唯一的 candidate 0；不把多条 message 合并成一条，不从顶层 output_text 反推内容。message 的 role 必须为 assistant，output_text 与 refusal 分别进入 Text／Refusal，reasoning 的 summary_text 与 reasoning_text 进入 Reasoning。未知 item／content 以 Native 保存。id、model、created_at 只取服务端返回值；created_at=0 有效，不填入本地发送或接收时间。

history 保存完整、原序的 output items，包含 message.phase、reasoning.encrypted_content、未知字段及工具上下文，不带响应外壳或原请求，不自动生成 previous_response_id。encrypted_content 是不透明数据，不能解析、校验或重写。原始 response 和 usage 分别保存在 native，JSON 数字保持既有精度。普通响应解码不创建流式事件。

status=completed 表示该轮完整；failed 为 Failed；incomplete、cancelled、queued、in_progress、未知或缺失状态为 Incomplete。incomplete_details.reason 的 max_output_tokens／max_messages 映射 Length，content_filter 映射 ContentFilter，steered 映射 Pause，其余值保留 Other，缺失为 Unknown。message、reasoning 和客户端调用显式返回非 completed 的 item status 时，即使根状态 completed 也不能当作完整结果；服务端工具自身失败不把随后正常完成的模型回答改成失败。refusal 单独投影为 Refused，并使同轮所有客户端调用失去执行资格；根 failed 优先保留 Failed。只有 completed、failed、cancelled、incomplete 根状态的 usage 为 Final，其余为 Partial；缺失 token 仍是未知。

output item 的 id 与客户端调用的 call_id 是不同标识空间，各自非空、各自不得在同轮调用中重复；工具结果 item 可以引用先前的 call_id，不参与调用声明去重。function_call、custom_tool_call 只唯一关联到请求中对应类别、对应名称的工具，带 namespace 的调用不能误配到无 namespace 的同名工具；无法关联时保留 Native。ClientToolCallReference.id 使用 call_id，context 保存完整原始 item，保留 item id、caller、namespace 等字段。完整 function arguments 必须是有效 JSON 对象，custom input 是原文；无效或未完成参数仍可观察但不进入可执行列表。

computer_call、local_shell_call、apply_patch_call 与明确 environment.type=local 的 shell_call，可唯一关联同 dialect 的 ClientNative 声明。local_shell_call 必须返回用于结果关联的 item id。computer 的单 action 与批量 actions 互斥，pending_safety_checks 与全部动作原样交给应用，库不执行动作或自动确认；其他原生输入分别保留 action／operation 对象。明确 container_reference 的 shell_call 归为服务端活动，环境缺失或未知时保留 Native，不推断为本地命令。

web_search_call、file_search_call、code_interpreter_call、image_generation_call 和容器 shell 分别投影为相应的 ServerToolActivity；服务端工具保持服务端执行方，不成为 ClientToolCall。活动保留真实 item id、status、action、结果与 native。WebSearch 关联请求中的通用搜索或同 dialect 的原生搜索声明；有多个匹配时不猜 toolKey。search action 的 sources 单独投影为来源，open_page／find_in_page 等动作仍保留原文。未知状态为 Unknown，不伪造完成或失败。

output_text.annotations 的 URL 引用与普通 file 引用分别投影为 Url／File 来源；容器文件和未知引用保留 Native 来源。官方文档未明确公共 TextRange 所需的 Unicode 单位及半开端点约定，故此处 range=None，原始索引保存在 source.native；file 的 index 不解释成字符范围。来源 ID 不冒充工具调用 ID，不自行下载来源内容。

usage 读取 input_tokens、output_tokens、total_tokens，以及 input_tokens_details 中的 cached_tokens／cache_write_tokens 和 output_tokens_details.reasoning_tokens。计数使用非负 Long，不从缺失的 total_tokens 推导总量；缓存读写与普通输入按第 6 节相同的 OpenAI 合同归一化，reasoning 是输出子集，不能再次加到总量。HTTP 拒绝保留状态、正文和原生错误；只有 HTTP 429 且 error.code=rate_limit_exceeded 可重试，额度不足或未知服务错误不取得重试资格。成功 HTTP 中的 failed response 是模型结果，不转成传输重试。

本节交付完整 response 解码；请求、网络与 SSE 按后续章节实施，不提供抛出占位错误的生成入口。协议依据：[Responses create](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Web search](https://developers.openai.com/api/docs/guides/tools-web-search)、[Computer use](https://developers.openai.com/api/docs/guides/tools-computer-use-integration)、[缓存读写统计](https://developers.openai.com/api/docs/guides/prompt-caching)。

## 16. Responses 请求与普通生成

Providers.openAiResponses() 固定组合 ResponsesProtocol 与 OpenAiResponsesDialect，部署缺省为 https://api.openai.com/v1；generationUrl 在部署 URL 后追加 responses，保留既有前缀与查询。认证、模型 ID、capability、调用寿命、重试和计时沿第 8～9 节，不查询模型目录、不按模型名推断或切换 protocol。最小请求只编码 model 和 input；默认不设置温度、reasoning、输出上限、存储或输出格式。

input 依次编码新 Message、显式 ToolResult 与原始 History。User／Assistant／System／Developer 保留角色；单一且无扩展的 Text 可使用字符串，其余内容为 input_text／input_image／input_file 数组。request.system 映射文本 instructions，多段文本直接连接；需要内容块扩展的 system 使用输入中的 System message，不把扩展静默丢弃。History 必须属于同 dialect，items 逐项保持原样，不重新编码 phase、reasoning 或服务端工具；单独的 history.context 不映射到隐式 conversation 或 previous_response_id。

图片支持 JPEG、PNG、GIF、WebP 的内联 bytes、http(s) URL 和本 dialect 文件 ID，分别编码 data URL／image_url／file_id。Document 编码 input_file，保留显式 MIME 的 data URL、file_url 或 file_id；filename 仅应用于内联文件。媒体输入不自动联网下载或上传，无法表达的 MIME 参数、音频、视频和有时间戳的视频帧报告输入路径。未知原生内容必须属于同 dialect，按原值保留。

temperature 范围 0～2、topP 范围 0～1，显式零值保留；maxOutputTokens 映射 max_output_tokens，candidates 只接受 1，topK 和 stop 没有对应字段而拒绝。reasoning.effort 显式映射到同名嵌套字段；enabled=false 可表达为 effort=none，enabled=true 需要显式非 none effort，enabled 与 effort 不得矛盾，budgetTokens 不映射。JsonObject／JsonSchema 进入 text.format，名称、schema、description、strict 按原值编码，Text 缺省省略。具体模型支持哪些值由 capability 和服务端决定，不猜模型规则。

ClientFunction 和 ClientCustom 使用 Responses 平铺的 function／custom 工具格式，保留调用方给出的 schema、strict、文本或 grammar 格式；同名客户端工具在编码时拒绝。ClientNative／Server.Native 保留配置与显式版本，type 由声明的 kind／version 决定；已知的执行方不允许互换。shell 必须明确 local 或 container_auto／container_reference 环境以确定执行方。通用 WebSearch 映射 web_search，支持 allowedDomains、location 和 includeSources；excludedDomains 无对应字段而拒绝。includeSources 控制根 include 中的 web_search_call.action.sources，false 编码空 include，缺省省略。工具选择 Auto 缺省省略，其余通过同一 ToolChoice 编码，不为搜索另设入口。

工具结果使用 reference.context 中保留的实际调用类型与关联信息，不能只把 item id 当成 call_id。function／custom 输出可为文本、图片或文件内容，保留 caller、name、namespace；isError 无通用 wire 字段而拒绝。computer 输出为显式截图，安全检查确认只接受应用提供的扩展，不自动回填 pending_safety_checks。local_shell 使用原调用 item id 和文本输出，shell 使用 call_id 及原生 output 数组，apply_patch 使用 call_id、文本 output 与由 isError 决定的 completed／failed 状态；其他原生调用不猜结果形状。所有工具结果都由应用提交，库不执行工具或自行继续下一轮。

request／message／part／tool 的扩展在各自位置合并，同 dialect 归属检查及已管理字段冲突规则沿第 3、7 节；已省略的管理字段也不能被覆盖。stream=true 仅影响 wire 编码；SSE 网络能力由单独实现的 LlmSseProtocol 提供。本节先交付普通生成，不用普通 HTTP 结果冒充流式结果。

## 17. Responses SSE

ResponsesProtocol 实现 LlmSseProtocol，使用第 11 节的同一网络入口、读取寿命和计时。SSE event 名为空或 message 时由 JSON type 分派，显式名称必须一致。response.created 建立响应身份；后续 queued／in_progress 更新实际元数据和累计 usage。存在的 sequence_number 必须为非负、严格递增的整数，但不要求连续，也不制造缺失序号。item 的 output_index 按从零递增的位置建立；后续事件同时检查 item_id，item ID 与工具 call_id 保持独立。

文本、拒绝、reasoning summary／content、函数／custom 参数分别交付相应 delta。content／summary part 按各自索引关联；同一 reasoning item 的通用 partIndex 使用首次观察顺序，最终展示 parts 使用相同顺序，原生 history 仍保持协议字段顺序。done 是完整快照，不再次交付已观察的文本；尚未通过 delta 报告的尾部文本可以从快照交付。已结束的 part／item 不接受新的 delta。引用按 annotation_index 交付一次，保留原始索引，不推测 TextRange 单位。

服务端搜索、文件搜索、代码执行、图像生成等进度及完成 item 交付 ServerToolActivity，无法通用解释的事件（包括媒体／工具原生片段）保持 Native。客户端调用等到 response.completed／incomplete／failed 的最终候选状态明确后才交付；只有完整且未拒绝的调用可以执行，参数解释沿第 15 节。output_item.done 并不保证整轮完成，也不提前冻结 usage。

response.completed、response.incomplete、response.failed 是终止事件，其 response.status 必须与类型一致；failed 是失败的模型结果，error 事件是流协议失败。只收到 item done 或普通 EOF 不构成终止，不依赖 Chat 的 [DONE]。终止 response 的完整 output 提供最终数据；已观察 item 的身份与顺序不得改变。Result 优先保留 output_item.done 的完整 item，尤其 encrypted_content；尚未结束的 item 使用终止快照，支持被截断的结果。完整快照使未知 delta 不必通过猜测合并来恢复 history。

EventsOnly 只保存关联、文本／引用的已交付位置、usage 和结束状态，不累计普通文本、reasoning、工具参数或已完成 item；处理终止事件时临时读取其完整 response。Result 保存完成 item 与必要 Native 片段，终止后生成 LlmResponse，native 为 None。两种模式共用检查与事件投影。usage 按已有累计快照规则合并，终止时标 Final，缺失项不补零；响应身份、UTC 与本地采样时间保持各自语义。

协议依据：[Responses streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events)、[Streaming responses](https://developers.openai.com/api/docs/guides/streaming-responses)。

## 18. Gemini generateContent 完整响应

GenerateContentProtocol 与 GenerateContentDialect 分别负责 contents／parts／candidates 的线格式和服务方解释；GeminiGenerateContentDialect 的标识为 DialectId(ApiProtocolId("generate-content"), "gemini-generate-content")。decodeResponse(value, request) 是不执行 I/O 的纯解码，不把 Interactions 的 step 或签名字段当作 generateContent part。请求编码与普通生成见第 19 节。

candidates 按 wire 顺序保存，index 缺省为协议的 0，不能用数组位置替代缺失的 index；负值和重复索引报错。没有候选时不伪造候选，promptFeedback、modelStatus 等信息保留在 response.native。responseId／modelVersion 映射 id／model，未报告的 createdAt 保持 None。content.role 可省略或为空，显式非 model 角色拒绝；content 与所有 parts、thoughtSignature、未知字段完整保留在 history 中。CONTINUATION 的 token 留在原文，候选为 Incomplete／Pause，history 不可用，因为继续该响应要求重复最初的输入，而不能追加部分输出。

STOP 表示 Complete，存在已关联的客户端调用时 finishReason 为 ToolCalls，否则 Stop；MAX_TOKENS 为 Incomplete／Length。安全、屏蔽、recitation、ESCALATION 与 PUP_LIMITED_DISABLED 停止为 Refused／ContentFilter；畸形调用、缺失签名及协议报告的生成失败为 Failed／Other。未报告或未指定的原因是 Incomplete／Unknown，其他原因保留 Other 并保持 Incomplete。完成状态在解释工具输入之前确定，未完成、拒绝和失败结果中的调用不进入可执行视图。

part 的已知 data 字段互斥；同时存在多个非 null 字段时报字段路径错误。相邻的普通文本／媒体组成同一 Message，相邻的 thought 内容组成同一 Reasoning，跨工具和未知 part 时保持原有顺序。thoughtSignature 本身不代表可展示的 reasoning，也不能因 text 为空而丢失。只有 thought=true 才进入 Reasoning；其中的原生调用不成为客户端可执行操作。未知 part 及签名单独出现的 part 保留 Native。

inlineData 解码 Base64 并按明确 MIME 分类为图片、音频、视频或 PDF／文本类文档；fileData 保留为本 dialect 的文件引用，不能自动下载、读取或猜测未报告的 MIME。其他媒体类型保持 Native。原始 MIME 参数、displayName、转写及媒体元数据仍可从 history／native 读取。

functionCall 只按 name 唯一关联本次请求的 ClientFunction；未声明或歧义调用保持 Native。id 是可选的，缺失／null／空字符串表示未提供服务器标识；非空 ID 在同一候选内不能重复，None 不参与去重。context 保存整个原始 part，包括签名和函数名。省略 args 表示无参数，按协议解释为空对象；显式非对象参数为 Invalid。完整对象参数为 JsonArguments，非 Complete 候选使用 Incomplete。functionResponse 是已有结果，保留 Native，不再次发起客户端执行。

executableCode／codeExecutionResult 是服务端代码执行活动，不能暴露为待执行函数；代码为 action，结果按明确 outcome 映射 Completed／Failed／Unknown。toolCall／toolResponse 同样为服务端活动，依 toolType 区分 WebSearch、WebFetch、FileSearch 或 Native；没有明确结果状态时使用 Unknown。所有服务器 ID 均按实际值保存，不从工具名或位置生成。只有能唯一关联到相应 Server 声明时设置 toolKey，ClientFunction 的同名声明不参与关联。

groundingMetadata 中的实际 web 查询、搜索入口或 web 来源可形成一条 WebSearch 活动；仅声明搜索工具不产生活动。来源表和 groundingSupports 按原索引关联，searchEntryPoint 原文留给应用展示。segment.partIndex 缺省为 0，startIndex 缺省为 0；明确 endIndex 时以 UTF-8 字节的左闭右开 TextRange 连接到对应文本 part，不改用字符下标。缺失范围保持 None，非法来源／part 索引和越界范围报路径错误。非文本 grounding 及其他来源元数据仍保留原文。citationMetadata 只有在唯一 content part 为文本时投影到该 part；无法确定归属时保留 Native，不把引用任意放到第一段文本。

usageMetadata 的 inputTokens 是 promptTokenCount 与 toolUsePromptTokenCount 之和，outputTokens 是 candidatesTokenCount 与 thoughtsTokenCount 之和；求和仅在两项均有报告时进行。已知 totalTokenCount 和其中一侧完整总数时，可按协议总数公式计算另一侧，但不据此填入未报告的明细。cacheReadTokens 对应 cachedContentTokenCount，nonCachedInputTokens 仅在完整 input 与 cacheRead 均已知时相减；cacheWriteTokens 保持 None。reasoningTokens 保留 thoughtsTokenCount，totalTokens 直接保留 totalTokenCount。所有实际计数检查非负与必要的加减溢出，缺失字段不补零；完整 HTTP 响应的 usage 为 Final，即使模型结果并不完整。

promptTokensDetails 与 toolUsePromptTokensDetails 均存在时，按模态合并为输入明细；candidatesTokensDetails 提供已报告的输出明细。cacheTokensDetails 是输入子集，仅保留原文，不再次加入通用模态总数。未知模态使用 Other，全部原生计数及未归一化明细保留在 usage.native。

协议依据：[generateContent API](https://ai.google.dev/api/generate-content)。参考 [Vercel Google usage 映射](https://github.com/vercel/ai/blob/main/packages/google/src/convert-google-usage.ts)的输入／工具与输出／思考拆分；本库仍保留未报告计数的 None，不自动继续请求或累加多次调用。

## 19. Gemini generateContent 请求与普通生成

GenerateContentProtocol 实现 LlmSseProtocol；Providers.geminiGenerateContent() 固定组合 GeminiGenerateContentDialect，缺省部署为 https://generativelanguage.googleapis.com/v1beta。简短模型 ID 前加 models，含斜线的 ID 按显式资源路径逐段编码，末段追加 :generateContent；保留部署前缀和查询，不查询模型目录。模型只参与 URL，不重复放入正文。普通与流式请求正文相同；SSE 规则见第 20 节。

最小正文只有 contents。User／Assistant 分别编码 user／model；request.system 编码只含文本的 systemInstruction，消息内 System／Developer 角色不能无声移动到全局。消息与 part 保持顺序，不跨历史项合并。历史检查 dialect 及 Content／parts 的基本形状后直接回传整个对象，保留未知字段与 thoughtSignature；不填补签名、不自动追加 CONTINUATION 的部分结果。

图片、文档、音频与视频 Bytes 映射 inlineData，URL 映射 fileData；保留完整 MIME 参数，不下载或转码。FileReference 使用同 dialect 的原始 fileData object，校验 fileUri 及显式 MIME 一致性；文档 filename 对应 displayName，已有且不同的名称拒绝覆盖。普通媒体的 MIME 与其内容种类必须一致；VideoFrames 没有等价的生成请求表达，显式报告路径错误。Native part 原样发送并检查归属及对象形状。ContentPart 扩展只影响对应 part；不能写入其他内容变体的管理字段。

采样、候选数、上限、停止序列置于 generationConfig，未指定时省略整个对象。temperature 范围 0～2，topP 范围 0～1，stop 最多 5 项；模型级限制仍由 capability 表达，不按模型名启用或删除选项。reasoning.budgetTokens 映射 thinkingBudget；effort 仅接受 minimal／low／medium／high 并映射相应大写 thinkingLevel，两者互斥。enabled=false 映射预算 0；enabled=true 需要明确的正预算或 effort，不能被解释为 includeThoughts。JsonObject／JsonSchema 使用 responseFormat.text 的 APPLICATION_JSON 和可选原始 schema；独立 name／description／strict 没有等价字段，显式非缺省值拒绝。

ClientFunction 编码 parametersJsonSchema，保留参数 schema，不转换为旧 OpenAPI Schema。多个函数名必须唯一；每个声明独立占一个 tools 项，扩展位于该 FunctionDeclaration。协议没有 per-function strict、parallelToolCalls 或 custom tool 格式，显式请求报错。ToolChoice.Auto 省略；None／Required／Specific 分别映射 functionCallingConfig 的 NONE／ANY／ANY+allowedFunctionNames；它们只适用于函数声明，不能借此控制服务端工具，混入服务端工具时非 Auto 选择拒绝。ClientNative 需另有已实现的调用／结果映射，本节不伪装成普通函数。

Server.WebSearch 映射 googleSearch，WebSearchOptions 中不能等价表达的显式域名、位置和来源开关报错；Server.Native 支持 codeExecution、googleSearch、googleSearchRetrieval、urlContext、fileSearch、googleMaps、mcpServers，其 configuration 为该工具字段的对象内容，mcpServers 单项转换为数组元素。执行方保持 server，重复单例种类报错，mcpServers 可重复声明。显式版本无对应字段，拒绝而不丢弃。请求扩展 toolConfig.includeServerSideToolInvocations 可显式要求返回服务端执行过程，不因工具声明自动开启。

ToolResult 使用 reference.context 中原始 functionCall.name，缺省 context 时可从当前 ClientFunction 的 ToolKey 解析；不把 ToolKey 作为 wire name。存在的 context ID 与 reference ID 必须一致，缺省 ID 保持省略。连续 ToolResult 组成一个 user Content 的多个 functionResponse part，以表达并行调用的整组结果。文本结果按原顺序置入 response.output，isError=true 使用 response.error；只有一个文本时用字符串，多个时用字符串数组，空结果用空数组。Bytes 媒体作为 functionResponse.parts.inlineData；没有协议对应形式的 URL、文件引用或 Native 内容明确拒绝，不下载、不把任意内容转成成功说明。ToolResult 扩展位于 functionResponse，不得覆盖 name／id／response／parts。

请求扩展在原位置合并。generationConfig、thinkingConfig、responseFormat、responseFormat.text 与 toolConfig 只允许补充未被通用 API 管理的字段；即使通用值省略，也不能用扩展绕过其校验。对未知扩展不推测语义。根部的 contents／systemInstruction／tools／model／stream 保留为管理字段。

LlmAuth.header(name, value) 表达固定认证头；它与 bearer 一样不读取环境，不进入可序列化请求或响应。Gemini 应用显式提供 x-goog-api-key。普通生成复用现有调用寿命、能力检查、实际 UTC 时间戳、错误正文边界和重试；Google error.status 作为服务错误 code，完整原文保留，仅 503／UNAVAILABLE 标记可重试，RESOURCE_EXHAUSTED 不被猜成短时限流。

协议依据：[generateContent](https://ai.google.dev/api/generate-content)、[ToolConfig](https://ai.google.dev/api/caching#ToolConfig)、[API key 请求头](https://ai.google.dev/gemini-api/docs/api-key)。借鉴 [Vercel Google 消息映射](https://github.com/vercel/ai/blob/main/packages/google/src/convert-to-google-messages.ts)中的有序 part 与 functionResponse 组织，保持本库的原生历史和显式媒体形式。

## 20. Gemini generateContent SSE

streamUrl 使用同样的模型资源路径，操作为 :streamGenerateContent；将 alt 查询设为 sse，其他部署查询保留。普通入口继续使用 :generateContent。请求正文相同，不加入 stream 字段；认证观察最终 SSE URL。createStreamDecoder 及客户端读取、保留与收尾均使用第 10、11 节的公共入口。

每个 message 数据是一个 GenerateContentResponse。候选按实际 index 独立归并，单块内重复／负索引拒绝；responseId／modelVersion 只接受一致的非空身份。显式 content.role 必须为 model 或空。根部 error 或 SSE error 失败；未知事件／字段保留 Native。不同候选的输出不能拼接到同一位置。

跨块相邻且 thought 状态相同的纯 text 片段可合并为一个展示 part；同一 Content 中的不同 part、带签名或其他元数据的 part 保持独立边界。相邻普通文本／媒体组成 Message，相邻 thought 内容组成 Reasoning，遵循第 18 节的分组；事件的 itemIndex／partIndex 与保留结果一致。空文本中的 thoughtSignature 仍保留；只含签名不算有效文本或首输出，也不单独创建另一条 Message。媒体与服务端工具使用第 18 节的解释，媒体通过 Native 事件交付原始 part 和 partIndex，并在 Result 中保留媒体输出。含 part 的原始 Content 按收到的顺序保存在 history.items，只有空 parts 的结束壳不生成一条空历史消息；不把展示文本拼接结果用于回传。CONTINUATION 与不能解释的增量函数参数不提供 history。

Gemini functionCall 的参数在一个 part 中完整提供，ToolInputDelta 交付其 JSON 原文；ClientToolCall 等候选 finishReason 后再确定执行资格。缺失 ID 保持 None，不生成本地 ID 冒充服务器标识；不同调用按位置区分。服务端代码执行、搜索等活动直接交付 ServerToolActivity。partialArgs／willContinue 属于 Vertex 的增量合同，本 Gemini 解释器将带这些非 null 字段的 functionCall 保留 Native，不把省略 args 猜成完整的空对象调用；完整响应同样遵守此规则。

groundingChunks 按到达次序追加，groundingChunkIndices 引用整个候选流的累计来源表；不以最新块覆盖旧来源。搜索活动按实际观察交付，不因声明工具生成事件。引用等 EOF 后按完整的展示 part、UTF-8 字节长度和来源表验证并交付 Citation；EventsOnly 只保留必要位置／字节数和未决引用，不累计普通文本。citationMetadata 仅在唯一展示 part 为文本时连接到该 part，否则保留 Native。Result 中的引用与事件位置一致。

非空、非 FINISH_REASON_UNSPECIFIED 的 finishReason 完成对应候选并交付一次 CandidateFinished；重复相同值可以携带尾部 metadata，已结束候选不能再增加内容。usageMetadata 是累计快照，沿第 10 节合并实际报告字段，始终在 EOF 前保持 Partial。只有干净 EOF、至少一个候选且所有候选都已结束，或服务明确报告 promptFeedback.blockReason 且没有候选时，endInput 才正常结束并交付最终 Usage。无候选的拦截不伪造候选。空流、缺少停止原因、冲突身份、非法 JSON 与半途网络中断均失败；不能仅因读取结束就把截断结果当作完整结果。

EOF 后停止总截止、记录 completed 并保留最终结果；firstEvent 仍来自实际 SSE 数据，清理及回调后才可读取 result。CONTINUATION 保留原生 token 和 Incomplete／Pause，不自动发起后续请求。EventsOnly 与 Result 使用同样的语义检查，不因保留策略改变工具执行资格或错误结果。

协议依据：[generateContent 流与 grounding](https://ai.google.dev/api/generate-content)、[签名回传边界](https://ai.google.dev/gemini-api/docs/generate-content/thinking#thought-signatures)、[Google SDK 流式历史](https://github.com/googleapis/js-genai/blob/main/src/chats.ts)、[FunctionCall 的 Gemini／Vertex 差异](https://googleapis.github.io/js-genai/release_docs/interfaces/types.FunctionCall.html)。参考 [Vercel Google SSE](https://github.com/vercel/ai/blob/main/packages/google/src/google-language-model.ts)的 EOF 收尾与 usage 处理，但保留多候选、未知计数和显式继续请求的合同。

## 21. Gemini Interactions 完整响应

InteractionsProtocol 与 InteractionsDialect 分别解释有序 step 协议和服务方用量／错误。GeminiInteractionsDialect 使用 DialectId(ApiProtocolId("interactions"), "gemini-interactions")，与 generateContent 独立。decodeResponse(value, request) 是不执行 I/O 的纯解码，普通请求与网络入口见第 22 节。以官方 v1 的公共字段为依据，未知 step、content、annotation 与新增字段保留原文。

完整资源要求非空 id、status、steps，以及可解码为 Instant 的 created／updated；可选 object 出现时必须为 interaction。created 映射 LlmResponse.createdAt，updated 留在 native；这些服务时间不代替客户端发送／接收采样，不检查服务时间的先后关系。model 可缺失，agent 不冒充 model。根部 error 不是成功资源；资源自身 status=failed 则是可观察的模型失败，errors 保留原文。

一个资源映射 index=0 的候选，items 与 steps 顺序一致。model_output 的 content 为 Message，thought 的 summary 为 Reasoning；thought.signature 保留为 Reasoning 中的 Native 元数据，并原样进入历史。user_input 与 function_result 保留 Native，不显示成模型回答。已知文本的 annotations 形成 Citation；未知 content 保留 OutputPart.Native。image／audio／video／document 的 data 使用 Base64 字节和明确 MIME，uri 使用当前 dialect 的 FileReference 并保存完整原生 content；无 MIME、缺少媒体内容或同时给出 data／uri 而不能无歧义投影时保留 Native，不猜格式、不下载资源。

completed 映射 Complete／Stop。requires_action 且存在尚未收到 function_result 的函数调用时映射 Complete／ToolCalls；只有这时、请求中存在唯一同名 ClientFunction 且 arguments 为 object，调用才进入可执行视图。其 reference 使用实际非空 id，并在 context 保存整个 function_call。参数非 object 保存为 Invalid，进行中／失败／截断状态保存为 Incomplete；无法唯一关联的调用保留 Native。已经有结果的调用及 completed 资源中的调用只保留 Native，不能再次执行。没有待回传函数的 requires_action 映射 Incomplete／Pause。failed 映射 Failed；incomplete、cancelled、in_progress 与未知状态映射 Incomplete，并保留具体状态为 Other，不把所有 incomplete 猜成输出上限。一次资源内已知调用的 ID 不得重复；客户端结果只与 function_call 关联，不能消费同 ID 的服务端调用。

google_search、url_context、file_search、code_execution、google_maps、mcp_server_tool、processing 的 call／result step 映射 ServerToolActivity，保持服务端执行方。call 为 InProgress，result 为 Completed 或显式 is_error=true 时的 Failed；id／call_id 保存真实关联。Google Search 的 image_search 使用 Native 种类，不能当作普通网页搜索；缺少本次 call 的搜索 result 也不猜搜索种类。MCP 以 server_name 匹配本请求的 Server.Native(mcp_server) 配置，不能因工具 name 相同就关联到 ClientFunction。能唯一定位时填 toolKey，否则保持 None；结果可以引用以前资源中的调用，不要求本资源重新包含 call。已在本资源出现的调用与结果种类必须一致，显式提供的工具／MCP 服务名称也不能相互冲突。服务端搜索的 HTML 建议与签名留在 native，不把它们拼进普通文本或解析成虚构来源。

url_citation／place_citation 的可用 URL 映射来源地址；file_citation 的 document_uri 映射当前 dialect 的文件引用，其他来源信息留在 native。已知引用的 start_index／end_index 表示 UTF-8 字节半开区间；省略 start 时按协议零值处理，仅有 start 而无 end 时不猜结束值。已报告范围必须非负、有序且不超出所属文本的字节长度。未知 annotation 不猜范围单位，保留 Native 来源。

usage.total_input_tokens 直接映射 inputTokens；total_output_tokens 与 total_thought_tokens 均已知时相加为包含 reasoning 的 outputTokens，后者同时是 reasoningTokens。total_tokens 保留报告值，不用它反推缺失明细；total_cached_tokens 为 cacheReadTokens，input 与 cacheRead 均已知时可相减得到 nonCachedInputTokens。total_tool_use_tokens 及其模态明细留在 native，不推断它们与输入总数互斥，不重复相加；缓存写入保持未知。输入／输出模态只映射有明确 modality 和 tokens 的条目，缺失条目字段不补零；缓存和工具细项不重复计入公共模态。已报告计数使用非负 Long，和的溢出及缓存超过输入均报字段路径错误。

completed、requires_action、failed、cancelled、incomplete 的 usage 为 Final；in_progress 与未知状态为 Partial。上述已知停止状态的 history.items 按顺序保存原始 steps（包括签名和未知字段），不把完整资源或 previous_interaction_id 当作历史项；进行中或未知状态不提供可追加历史。序列化往返不联网、不执行工具、不发起后续 interaction。请求构造、显式服务状态继续和 SSE 另行定义。

协议依据：[Interactions v1 参考](https://ai.google.dev/api/interactions-api-v1)、[官方 OpenAPI](https://ai.google.dev/static/api/interactions-v1.openapi.json)、[Interactions 状态与历史](https://ai.google.dev/gemini-api/docs/interactions-overview?hl=en)、[token 用量](https://ai.google.dev/gemini-api/docs/tokens)。

## 22. Gemini Interactions 请求与普通生成

InteractionsProtocol 实现 LlmGenerationProtocol；Providers.geminiInteractions() 包装 GeminiInteractionsDialect，缺省 baseUrl 为 https://generativelanguage.googleapis.com/v1。生成地址在部署路径后追加 interactions，已有查询参数保留；modelId 原样置入正文，不添加 models/、不按名称选择协议或查询目录。认证沿用 LlmAuth.header("x-goog-api-key", key)，库不读取环境。generate 复用现有能力检查、调用寿命、响应限制和实际发送／接收时间戳。

最小请求只包含 model 与 input。input 使用有序 step 数组，User／Assistant Message 分别为 user_input／model_output，内容使用 type 标记的 content；不合并独立消息。request.system 只接受文本并按顺序拼接为 system_instruction，省略时不发字段；消息中的 System／Developer 不能被移动到全局，明确报错。History 检查 dialect、无隐式 context、对象与非空 type 后原样追加 steps，签名及未知字段不重建。previous_interaction_id、store、background 通过本次 request.extensions 显式提供；不把 history 自动改成 ID 接续，不替应用保存或继承上一轮的配置。

文本编码为 type=text；图片／文档／音频／视频 Bytes 编码 data 与 mime_type，Url 编码 uri 与 mime_type，不下载、不转码。FileReference 接受同 dialect 的原始媒体 content object，要求 type、uri 和 MIME 与当前内容一致，保留其他字段；不将引用中的 Base64 数据悄悄改为 URI 引用。Native content 要求同 dialect 的对象与非空 type。VideoFrames、单独的文档 filename 没有对应字段，显式请求报路径错误。Message、content、tool 和 ToolResult 的扩展分别作用于对应 wire object，不能覆盖其 type／内容／关联管理字段。

当前 v1 generation_config 的公共映射为 max_output_tokens、stop_sequences、thinking_level 和 tool_choice；缺省省略整个配置。temperature、topP、topK、候选数、parallelToolCalls 与 reasoning.budgetTokens 没有对应开关，显式值不能静默忽略。reasoning.effort 接受 minimal／low／medium／high；enabled=true 需要显式 effort，enabled=false 没有精确的关闭表达，不能以 minimal 代替关闭。JsonObject／JsonSchema 映射 response_format 的 type=text、mime_type=application/json 及可选原始 schema；name 的缺省 response 不单独发送，非缺省 name、description、strict 显式提供时报错。

ClientFunction 编码 type=function、name、description 和 parameters，不改写 JSON Schema；名称在本请求的函数集合内唯一，strict 无对应字段。ClientCustom／ClientNative 没有本协议已定义的客户端调用映射，拒绝而不伪装成函数。ToolChoice.Auto 省略；None／Required 分别为 none／any；Specific 为 allowed_tools.mode=any 和实际函数名数组。Required 需要至少一个函数；非 Auto 选择只控制函数，不用于包含服务端工具的混合请求。

Server.WebSearch 编码 type=google_search；域名、位置和 includeSources 没有等价字段，显式选项报错。Server.Native 支持 google_search、code_execution、url_context、file_search、google_maps、mcp_server，其 configuration 的成员位于工具对象顶层，type 由 kind 管理。无 wire version，显式版本拒绝；单例工具不可重复，MCP 可以声明多个服务，已提供的 name 必须非空且不重复。server 工具与函数可在 Auto 模式共存，执行仍完全由服务端负责。

ToolResult 编码 function_result，使用 reference 的实际非空 id 作为 call_id；context 存在时必须为同一 function_call 且 ID 一致。name 来自 context，缺省 context 时使用同 ToolKey 的当前 ClientFunction 声明，不用 ToolKey 替代 API 名称。result 按原顺序编码 text／image content 数组，空结果保持 []；其他模态明确拒绝，Native 需为同 dialect 的 text 或 image content。isError=true 才发送 is_error=true。多个并行结果保持独立 step，不执行工具也不自动生成下一轮。

根部扩展可补充未管理的原生字段；generation_config 仅补充 seed、thinking_summaries、video_config 等未由通用选项管理的字段，不覆盖已有值，也不绕过已知公共参数的检查。response_format 扩展可以提供原生媒体格式对象或数组；其 type=text 项不得设置通用 API 管理的 mime_type／schema。如果通用选项已生成 JSON text format，则与扩展中的 text 配置合并，或作为独立 text format 加入媒体格式数组；没有通用 text 约束时保持原生对象／数组形状。媒体的原生配置不因公共 OutputFormat 缺省 Text 被删除。

协议依据：[Interactions v1 OpenAPI](https://ai.google.dev/static/api/interactions-v1.openapi.json)、[有状态与无状态函数接续](https://ai.google.dev/gemini-api/docs/function-calling)、[结构化输出](https://ai.google.dev/gemini-api/docs/structured-output)。SSE 与后台资源操作不由普通生成隐式启动。


## 23. Gemini Interactions SSE

InteractionsProtocol 实现 LlmSseProtocol，普通调用的缺省正文不变；流式调用显式添加 stream=true，地址在 interactions 查询中设置 alt=sse，保留部署路径和其他查询。createStreamDecoder(request, retention=EventsOnly) 与其余协议共用 openStream／withStream、寿命与实际时间采样。

按 event_type 分派 interaction.created、interaction.status_update、step.start／delta／stop、interaction.completed 和 error。非空 SSE event 名必须与 event_type 一致；event_id 是不透明恢复标记，保留原文，不解释为递增序号，不自动重连或继续。created 建立唯一 interaction 身份；created 中已提供的 steps 作为初始完整快照；后续步骤索引按首次出现顺序连续，已开始的不同步骤可以交错。重复开始、未开始就增量、结束后再增量、身份变化及未结束步骤缺失最终快照均报告协议错误。未知事件和未知 delta 保留 Native；没有可解释的最终 step 快照时，未知 delta 不产生可回传的重建历史；只含 type 等元数据的快照不能恢复未知文本、签名、工具参数或结果。

一个 step 对应一个 candidate 0 的 item，location 与最终 items 保持一致。model_output 的连续 text delta 合并到当前文本 part，切换媒体／其他内容后开始新 part；thought_summary 的 content 按协议作为新的 summary part，thought_signature 保存到所属 thought。签名不进入可见文本，空签名与只有签名的步骤仍保留。图片、音频、视频和文档 delta 作为各自原生 content 交付，不按 Base64 字符串拼接多个独立媒体内容。text_annotation_delta 关联该步骤最近的文本 part，保留原始注释，在步骤结束时按已收到文本的 UTF-8 字节数验证并交付 Citation；不为未知引用猜单位。EventsOnly 仅保留位置、长度及待完成关联所需数据，不累计普通文本或媒体正文。Result 复用处理增量时已经解码的媒体和引用，不在组装最终输出时重做相同验证。

function_call 的 start 提供 ID 与 name，arguments_delta 的字符串按序累积；start.arguments 是初始快照，有增量时由完整增量字符串取代，不能把空对象前缀拼到参数 JSON 前。step.stop 只结束参数传输；interaction.completed 的 requires_action 才决定完整且未被 function_result 回答的函数是否可执行。损坏 JSON／非对象参数分别保留 Invalid，未完成或失败状态保留 Incomplete，不调用客户端工具。原始参数无法表示为合法 step JSON 时不提供重建 history，原始片段仍由事件保存。

服务端工具的 call／result 保持各自 step 与实际调用 ID，使用同一工具归属解析。start 与已知 delta 可以只含部分字段；有参数时验证其形状，缺少结果的停止步骤保留无结果的服务端活动与原文，不虚构返回内容。结构化 arguments 更新已提供的成员，result 的数组／字符串增量分别按序追加／拼接，其他字段为当前快照；签名、原生字段、MCP server_name 与搜索类型均保留，服务端活动不会变为客户端调用。

usage 可来自 status_update.usage、step.stop.usage、step.delta.metadata.total_usage 或 lifecycle.interaction.usage，均为累积快照，后续已提供字段替换前值，缺失字段不清零。step_usage 仅作为单步原文，不累加到请求总量。interaction.completed 冻结已知停止状态的 Final usage；它自身可省略 created／updated／model／steps，客户端不补造服务时间。已提供 created 必须保持同一 Instant，updated 只验证格式而不要求时钟递增。status_update 的终止状态和裸 [DONE] 都不能替代 interaction.completed。

interaction.completed 可携带最终 steps；按步骤身份合并快照并交付尚未看到的文本、引用或工具内容，不重复已经交付的文本和引用。原始 steps 数组另外作为 lifecycle.steps Native 快照事件交付，媒体等原生内容可据此读取最终状态；重建 history 已保存最终步骤，不再为该快照另存一份 nativeEvents。只在快照中出现的服务端 result 同样交付 Completed／Failed 活动。没有 steps 时从已结束且可完整重建的步骤生成 history。最终状态沿用完整响应的 completion／finishReason 规则；未知状态保留 Incomplete 与 Partial usage，不提供可追加历史。完成事件后传输可结束，尾随 event:done / data:[DONE] 是可忽略哨兵；提前 DONE、error 或 EOF 都不能产生成功结果。结果保留模式才提供 response()，且保留原生历史和未知事件。

协议依据：[官方流式指南](https://ai.google.dev/gemini-api/docs/streaming)、[Interactions v1 OpenAPI](https://ai.google.dev/static/api/interactions-v1.openapi.json)。不自动执行工具、轮询后台资源或消费 continuation_token。

## 24. Bedrock Converse 完整响应

ConverseProtocol(dialect) 提供不执行 I/O 的 decodeResponse(value, request)。ConverseDialect 解释服务方 usage 与系统工具种类；BedrockConverseDialect 使用 DialectId(ApiProtocolId("converse"), "bedrock-converse")。本节交付完整 JSON 响应的读取，不把 ConverseStream 的 AWS 二进制 EventStream 当作 SSE；请求、部署和认证在各自实施时定义。

output 是 tagged union，message 的 role 必须为 assistant，content 按顺序投影为候选 0 的 items。连续文本 block 保持为同一 Message 中的多个文本 part，使一段完整 JSON 不因 wire 分块而在 decodeOutput 中成为歧义消息；reasoning、媒体和工具仍保持各自位置。content、reasoningContent、媒体 source 等 union 必须恰有一个成员；未知成员作为 Native 保留，不能与已知成员同时解释。根部、消息及结构体中的未知字段保留原文。响应没有公开的模型名、响应 ID 或创建时间字段，不从请求、HTTP 时间或 metrics.latencyMs 补造；metrics 与 additionalModelResponseFields 保存在 native。

text 是普通文本；reasoningText.text 是 Reasoning，签名及 redactedContent 保留原文，与整个消息一起保存到 history。图片、音频、视频、文档的已知 format 映射 MIME；bytes 解码 Base64，s3Location 保存同 dialect 的原始媒体 block 作为 FileReference，不下载。格式未知、文档格式缺失、原生文档 text／content、音频或图片 error 等无法无歧义投影的内容保留 Native。PCM 缺少位深／采样率、Opus 缺少容器约定时保持 Native，不猜通用 MIME；audio/opus 是 RTP payload 类型，不能用它替代 Ogg Opus 文件的 audio/ogg，见 [RFC 7845](https://www.rfc-editor.org/rfc/rfc7845.html#section-9)。

toolUse 的真实 toolUseId／name 必须非空，同一响应中的调用 ID 不得重复。没有 type 的调用只能与唯一同名 ClientFunction 关联；type=server_tool_use 则是 ServerToolActivity，未知 type 不进入可执行视图。只有 stopReason=tool_use、尚未收到结果且 input 为 JSON object 的客户端调用可执行；非 object 保留 Invalid，其他停止状态保留 Incomplete，未知或歧义绑定保留 Native。toolResult 使用实际 ID 关联，不能把已完成的调用再次执行；客户端结果保持 Native。服务端结果可以只引用此前调用，已知结果类型不得关联到本响应中不同执行方或不同名称的调用。

Bedrock 的 nova_grounding 映射 WebSearch，nova_code_interpreter 映射 CodeExecution，其他系统工具保持 Native 种类。Server.WebSearch 与 nova_grounding 绑定；Server.Native 通过同 dialect 的 kind=systemTool、configuration.name 精确绑定。结果名称只按明确的服务约定解释，例如 nova_code_interpreter_result；没有当前调用和已知类型时不猜所属工具。结果 status=success／error 分别映射 Completed／Failed，缺失或未知 status 为 Unknown。服务端结果中的 text、媒体、JSON 和搜索来源按原顺序保留，只有实际调用／结果才形成活动，不因工具声明或普通引用虚构搜索执行。

citationsContent.content 中的生成文本与该 block 的 citations 关联。一个或多个连续的单独 citationsContent 紧邻普通文本时，将引用连接到前一文本块，并保留每个原始引用 block；无法确定归属时保持 Native。web.url 形成 URL 来源，其他来源位置及 sourceContent 保留原文；源文档的字符／页／chunk 范围不冒充生成文本的 TextRange，range 保持 None。来源 ID 使用响应内字段位置，与工具调用 ID 独立。

end_turn／stop_sequence 映射 Complete／Stop，tool_use 为 Complete／ToolCalls，max_tokens／model_context_window_exceeded 为 Incomplete／Length，guardrail_intervened／content_filtered 为 Refused／ContentFilter，malformed_model_output／malformed_tool_use 为 Failed。未知停止原因保留 Incomplete／Other。已知停止原因提供完整消息的原生 history；未知停止原因或未知 output union 不提供可追加 history。序列化与读取历史不执行工具、不发起下一次请求。

usage.inputTokens 为 nonCachedInputTokens；它与 cacheReadInputTokens、cacheWriteInputTokens 均已知时相加得到公共 inputTokens。缺失 cache 字段不补零，totalTokens 保留服务端报告，不反推缺失细项；outputTokens 包含实际输出，未报告 reasoning 计数时保持 None。cacheDetails 是按 TTL 拆分的写入明细，保留原文、不重复相加。所有读取的计数使用非负 Long，求和检查溢出；已知停止原因的 usage 为 Final，未知原因是 Partial。服务计时、计数与客户端发送／接收时间各自保持含义。

协议依据：[Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html)、[ContentBlock union](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlock.html)、[缓存计数公式](https://docs.aws.amazon.com/bedrock/latest/userguide/prompt-caching.html)、[系统工具的调用与结果](https://docs.aws.amazon.com/nova/latest/userguide/tool-built-in.html)、[引用与生成文本](https://docs.aws.amazon.com/nova/latest/userguide/grounding.html)。参考 [Vercel usage 归一化](https://github.com/vercel/ai/blob/main/packages/amazon-bedrock/src/convert-amazon-bedrock-usage.ts)，但不将缺失 cache 计数补为零。

## 25. Bedrock Converse 请求与普通生成

ConverseProtocol 实现 LlmEventStreamProtocol；Providers.bedrockConverse() 包装 BedrockConverseDialect，没有隐含 region 或 baseUrl。应用在同一个 LlmEndpoint 中提供明确部署地址及 LlmAuth；接口支持的 Bedrock API key 使用 LlmAuth.bearer，库不读取环境。生成地址在部署路径后追加 model、原始 modelId 所在的单个路径段及 converse，保留查询参数；模型 ID、inference profile 与 ARN 不按名称改写或查询目录。ConverseStream 地址末段为 converse-stream，encodeRequest 的 stream=true 使用同样的请求正文，不发送 JSON stream 开关；流式接收见第 26 节。

最小正文只有 messages，modelId 只用于 URL。User／Assistant 分别为 user／assistant 消息；System／Developer 消息不能被挪到全局，初始系统内容通过 request.system 表达。连续 ToolResult 组成一条 user 消息中的多个 toolResult block，其他消息保持独立。History 检查 dialect、无隐式 context、assistant role 和 content 数组后原样追加，不重新解释签名或未知 block。generate 复用能力检查、请求寿命、响应限制、实际发送／接收时间戳与完成回调。

text 编码为单成员 union；system 允许 Text 或原生 system union。图片、音频、视频和文档 Bytes 使用 Base64 source.bytes；Url 只接受 S3 URI 并编码 source.s3Location，不下载 HTTP URL。FileReference 接受同 dialect 的原始媒体 block，检查类型、format／MIME 与 S3 引用后保留其他字段。已知 MIME 反向映射 format，不能表达的 MIME 参数明确拒绝；PCM／Opus 容器不明确、VideoFrames 等使用显式 Native，不能自动转码或猜格式。文档 name 优先使用明确 filename，缺省为中性的 document；它是服务端文档显示名，显式值不自动删后缀或清洗字符。原生文件引用没有单独 filename 时保留其 name。媒体 payload 扩展位于对应 image／document／audio／video 对象内，不能覆盖 format／source／name；Text 不接受额外字段扩展，Native 的原生配置直接置于 Native.value。图片和文档只能在 user 内容中使用；文档消息需要同条消息中的文本。已知协议限制在编码时报告请求路径，模型特定的大小／模态限制由 capability 和服务端决定。

inferenceConfig 映射 temperature、topP、maxTokens 和 stopSequences，缺省省略；temperature／topP 为 0～1，maxTokens 至少 1，停止序列最多 2500 项且每项非空。candidates 只接受 None／1，不发送候选字段。topK、parallelToolCalls、reasoning.enabled 和 reasoning.budgetTokens 没有统一开关，显式值不能静默忽略。reasoning.effort 映射 outputConfig.effort，接受 low／medium／high／xhigh／max；模型专属 thinking 设置经 additionalModelRequestFields 显式提供，不按模型名推断。JsonSchema 映射 outputConfig.textFormat 的 type=json_schema 与 structure.jsonSchema，schema 使用完整 JSON 字符串，name／description 原样保留；strict=false 无等价语义而拒绝，None／true 使用服务端的约束输出。JsonObject 没有无 schema 的对应模式，明确报错，不代写可能改变含义的 schema。

ClientFunction 编码 toolSpec，保留实际 name、inputSchema.json、非空 description 及显式 strict。Server.WebSearch 使用 dialect 的明确系统工具名，Bedrock 为 nova_grounding；域名、位置与 includeSources 选项没有对应字段时拒绝。Server.Native 支持 kind=systemTool，configuration 位于该对象内，必须提供非空 name；不存在 wire version，显式版本拒绝。函数与系统工具的 name 在本请求中唯一，名称使用协议允许的 ASCII 字符且最长 64。ClientCustom／ClientNative 没有独立的 Converse 映射，不伪装为函数。ToolChoice.Auto 缺省省略；没有工具的 None 同样省略，非空集合没有 none 开关。Required／Specific 只用于纯函数集合，分别映射 any／tool.name，不把服务端工具伪装成客户端强制调用。

ToolResult 使用 reference 的实际非空 id；context 存在时必须是同 ID 的客户端 toolUse block。可选 type=null 与缺省采用相同的客户端归属，与响应 companion 解码一致；其他显式 type 不作为客户端调用回传。旧调用无需重新声明工具，当前 ToolKey 不替代 wire ID。结果支持 Text、Image、Document、Video 及同 dialect 的原生结果 union，status 由 isError 映射 success／error。JSON 结果通过 Native({json: value}) 保留结构；音频和视频帧没有结果字段而拒绝。工具结果的扩展位于 toolResult 对象内，不能覆盖 toolUseId／content／status／type。

request.extensions 可以补充 additionalModelRequestFields、guardrailConfig、promptVariables、requestMetadata 等原生字段；inferenceConfig、outputConfig、toolConfig 只合并未管理字段，不覆盖公共选项、messages、system、tools、toolChoice 或路由。扩展不替应用判断 Prompt management 资源的模型约束，也不自动移走与该资源不相容的设置。已解析的 429／503 服务拒绝可按既有调用策略重试，原始错误体保留；解析失败、截断或明确不同错误类型不获得重试资格。

协议依据：[Converse 请求](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html)、[结构化输出](https://docs.aws.amazon.com/bedrock/latest/userguide/structured-output.html)、[官方 SDK service model](https://github.com/boto/botocore/blob/develop/botocore/data/bedrock-runtime/2023-09-30/service-2.json)。内置 SigV4 在其独立实施时接入。

## 26. ConverseStream

[ConverseStream 扩展规范](SCOOP-LLM-CONVERSE-STREAM-SPEC.md)规定 AWS EventStream 的业务事件解释、内容聚合、服务端工具与原生历史，复用第 10、11 节的流式入口、计时、保留及生命周期合同。
