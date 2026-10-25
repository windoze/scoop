# Scoop LLM client 规范

本库通过普通 `scoop.llm.client` Cone 实现。总体职责、协议覆盖和实施顺序见 [架构设计](../llm-client/DESIGN.md)与[实施记录](../llm-client/PROGRESS.md)。本规范随功能闭环补充正式数据与 API 合同；尚未落地的调用示例仍以[使用草案](../llm-client/USAGE.md)标明状态。

服务商附加信息（包括计费字段）全部保留在对应响应、usage 或事件的原生 JsonValue 中，由具体 dialect 按需提供只读访问。保留原始字段、数字 token、未知内容及缺失／null／零的区别，不推算或归一化这些信息，不为其增加公共数据模型、生成流程或统一扩展接口。缓存相关附加信息同样只保留原文，不推导 TTL、刷新规则、到期时间或有效性。实际 LLM 协议内容、token 统计与客户端计时仍按各自合同处理。

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

`MediaLimits` 的下列 ULong? 字段缺省 None，表示未知上限；非负性由类型表达，零仍是明确上限：

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

`ModelLimits(contextWindowTokens, maxInputTokens, maxOutputTokens, maxCandidates)` 均为 ULong?，缺省 None，零和 ULong.MAX_VALUE 均可明确保存；不同计量口径不相加，也不依据其中一项补齐其他项。

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

LlmGenerationOptions 的 temperature、topP 为 Double?；topK、maxOutputTokens、candidates 为 ULong?；stop 为 Array<String>?；parallelToolCalls 为 Boolean?，均缺省 None。Double 必须有限，整数的非负性由 ULong 表达，已填写的 candidates 必须大于零；各模型的范围、空 stop 的含义及功能组合由具体 adapter／capability 判断。不设跨模型的固定温度范围，不自动补采样参数。

reasoning 为 ReasoningOptions?，其中 enabled、effort、budgetTokens 均可缺失；预算为 ULong，effort 按原字符串保留，具体允许值由 dialect 定义。output 缺省 OutputFormat.Text，另外有 JsonObject 和 JsonSchema(schema, name = "response", description = None, strict = None)。schema 必须为 object；名称与 schema 子集按所选协议检查，不从 ValueDecoder 反推 schema。

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

`LlmAttemptMetadata(number, request = ..., response = ..., status = None, requestId = None, responseId = None, usage = LlmUsage(), http = LlmHttpTiming())` 的 number 从 1 开始。timeToHeaders、timeToFirstOutput、timeToFirstText、responseLatency 仅使用同一尝试的 sent 与相应响应点的 elapsed 差；缺失任一端点时为 None，相应响应点早于 sent 时构造失败，UTC 先后不作为检查条件。outputTokensPerSecond 仅在响应完成、usage 为 Final、outputTokens 已知且 responseLatency 为正时返回端到端平均速率，否则 None。HTTP 阶段视图见 §33。

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

`LlmGenerationProtocol` 继承共同的 LlmApiProtocol（§50），描述普通 HTTP JSON 生成：generationUrl(baseUrl, modelId)、encodeRequest(modelId, request, stream) 与 decodeResponse(value, request)；dialectId 与 decodeError(status, body, truncated) 由基础合同提供。ChatProtocol 实现该合同并固定其 ChatDialect。接口没有实时会话或尚无实现的操作占位；流式 reducer 随相应独立合同补充。

`LlmProvider(name, protocol, defaultBaseUrl = None)` 固定一个协议／方言组合。`Providers.openAiChat()` 与 deepSeekChat() 分别提供已实现的两个组合及官方 URL 缺省。`LlmEndpoint(provider, authentication, baseUrl = None, fields = [])` 绑定已有 API 端点；缺少官方缺省又未传 URL 时构造失败。URL 必须是绝对 http(s) 地址且无 fragment。协议在现有 base path 后追加操作路径，保留 query；不把 base path 当成要被替换的文件名，不额外推导或管理云部署。

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

`Providers.anthropicMessages(version = V20261021, defaultMaxOutputTokens = 4096UL)` 包装 MessagesProtocol 与 AnthropicMessagesDialect，缺省 base URL 为 https://api.anthropic.com/v1。`AnthropicMessagesVersion` 明确区分 V20230601 与 V20261021，决定 anthropic-version 及已知版本规则，不按模型 ID 推断。max_tokens 为必需字段；请求省略 maxOutputTokens 时使用 ULong 类型的 dialect 缺省，显式值（含零）覆盖它。LlmGenerationProtocol.defaultMaxOutputTokens() 返回 ULong?，默认上限也要通过模型的已知 maxOutputTokens 检查，不静默缩小请求或改变模型绑定。普通 generate 与第 14 节的 openStream 共用该默认值与能力检查。

`LlmGenerationProtocol.generationFields()` 缺省返回空数组，defaultMaxOutputTokens() 缺省 None。前者提供该协议／方言的固定请求头，后者用于能力检查中的有效缺省上限。LlmEndpoint.fields 组合固定头与应用字段，应用不能重复固定头名称而造成版本与编码规则不一致；认证看到组合后的真实字段。Chat 的空头和省略上限行为保持不变。MessagesDialect 提供 requestFields、defaultMaxOutputTokens、encodeOptions 与 encodeContent 的协议专用合同；MessagesProtocol 在 base URL 后追加 messages，并复用公共 HTTP 调用、重试、计时和完成通知。

顶层 system 保存为 text block 数组；input 的 User／Assistant 映射同名 role，System 保持原位置的 system role，Developer 没有映射。带内容的 System 不能是首条消息；初始指令使用 request.system。并非所有模型支持中途 system／assistant prefill，Unknown 不按模型名猜测。普通内容支持文本、JPEG／PNG／GIF／WebP 图片的 bytes／HTTP URL／同方言文件 ID，以及 PDF 的相应来源；text/plain 文档支持 UTF-8 bytes 或同方言文件 ID。system 只接受文本，直接 assistant 文档在新版失败；音频、视频和带时间戳的视频帧没有该协议的通用映射。Document.filename 不冒充供应商 title；需要标题时使用该 content part 的 title 扩展。

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

完整资源要求非空 id、status，以及可解码为 Instant 的 created／updated；可选 object 出现时必须为 interaction。steps 出现时必须为数组；取消等状态快照可以省略 steps，此时可读 items 为空，history 保持 None，不把未返回的步骤冒充已知空历史。created 映射 LlmResponse.createdAt，updated 留在 native；这些服务时间不代替客户端发送／接收采样，不检查服务时间的先后关系。model 可缺失，agent 不冒充 model。根部 error 不是成功资源；资源自身 status=failed 则是可观察的模型失败，errors 保留原文。

一个资源映射 index=0 的候选，items 与 steps 顺序一致。model_output 的 content 为 Message，thought 的 summary 为 Reasoning；thought.signature 保留为 Reasoning 中的 Native 元数据，并原样进入历史。user_input 与 function_result 保留 Native，不显示成模型回答。已知文本的 annotations 形成 Citation；未知 content 保留 OutputPart.Native。image／audio／video／document 的 data 使用 Base64 字节和明确 MIME，uri 使用当前 dialect 的 FileReference 并保存完整原生 content；无 MIME、缺少媒体内容或同时给出 data／uri 而不能无歧义投影时保留 Native，不猜格式、不下载资源。

completed 映射 Complete／Stop。requires_action 且存在尚未收到 function_result 的函数调用时映射 Complete／ToolCalls；只有这时、请求中存在唯一同名 ClientFunction 且 arguments 为 object，调用才进入可执行视图。其 reference 使用实际非空 id，并在 context 保存整个 function_call。参数非 object 保存为 Invalid，进行中／失败／截断状态保存为 Incomplete；无法唯一关联的调用保留 Native。已经有结果的调用及 completed 资源中的调用只保留 Native，不能再次执行。没有待回传函数的 requires_action 映射 Incomplete／Pause。failed 映射 Failed；incomplete、cancelled、in_progress 与未知状态映射 Incomplete，并保留具体状态为 Other，不把所有 incomplete 猜成输出上限。一次资源内已知调用的 ID 不得重复；客户端结果只与 function_call 关联，不能消费同 ID 的服务端调用。

google_search、url_context、file_search、code_execution、google_maps、mcp_server_tool、processing 的 call／result step 映射 ServerToolActivity，保持服务端执行方。call 为 InProgress，result 为 Completed 或显式 is_error=true 时的 Failed；id／call_id 保存真实关联。Google Search 的 image_search 使用 Native 种类，不能当作普通网页搜索；缺少本次 call 的搜索 result 也不猜搜索种类。MCP 以 server_name 匹配本请求的 Server.Native(mcp_server) 配置，不能因工具 name 相同就关联到 ClientFunction。能唯一定位时填 toolKey，否则保持 None；结果可以引用以前资源中的调用，不要求本资源重新包含 call。已在本资源出现的调用与结果种类必须一致，显式提供的工具／MCP 服务名称也不能相互冲突。服务端搜索的 HTML 建议与签名留在 native，不把它们拼进普通文本或解析成虚构来源。

url_citation／place_citation 的可用 URL 映射来源地址；file_citation 的 document_uri 映射当前 dialect 的文件引用，其他来源信息留在 native。已知引用的 start_index／end_index 表示 UTF-8 字节半开区间；省略 start 时按协议零值处理，仅有 start 而无 end 时不猜结束值。已报告范围必须非负、有序且不超出所属文本的字节长度。未知 annotation 不猜范围单位，保留 Native 来源。

usage.total_input_tokens 直接映射 inputTokens；total_output_tokens 与 total_thought_tokens 均已知时相加为包含 reasoning 的 outputTokens，后者同时是 reasoningTokens。total_tokens 保留报告值，不用它反推缺失明细；total_cached_tokens 为 cacheReadTokens，input 与 cacheRead 均已知时可相减得到 nonCachedInputTokens。total_tool_use_tokens 及其模态明细留在 native，不推断它们与输入总数互斥，不重复相加；缓存写入保持未知。输入／输出模态只映射有明确 modality 和 tokens 的条目，缺失条目字段不补零；缓存和工具细项不重复计入公共模态。已报告计数使用非负 Long，和的溢出及缓存超过输入均报字段路径错误。

completed、requires_action、failed、cancelled、incomplete 的 usage 为 Final；in_progress 与未知状态为 Partial。上述已知停止状态且返回 steps 数组时，history.items 按顺序保存原始 steps（包括签名和未知字段），不把完整资源或 previous_interaction_id 当作历史项；进行中、未知状态或 steps 缺失／null 时不提供可追加历史。序列化往返不联网、不执行工具、不发起后续 interaction。请求构造、显式服务状态继续和 SSE 另行定义。

协议依据：[Interactions v1 参考](https://ai.google.dev/api/interactions-api-v1)、[官方 OpenAPI](https://ai.google.dev/static/api/interactions-v1.openapi.json)、[Interactions 状态与历史](https://ai.google.dev/gemini-api/docs/interactions-overview?hl=en)、[token 用量](https://ai.google.dev/gemini-api/docs/tokens)。

## 22. Gemini Interactions 请求与普通生成

InteractionsProtocol 实现 LlmGenerationProtocol；Providers.geminiInteractions() 包装 GeminiInteractionsDialect，缺省 baseUrl 为 https://generativelanguage.googleapis.com/v1。生成地址在部署路径后追加 interactions，已有查询参数保留；modelId 原样置入正文，不添加 models/、不按名称选择协议或查询目录。认证沿用 LlmAuth.header("x-goog-api-key", key)，库不读取环境。generate 复用现有能力检查、调用寿命、响应限制和实际发送／接收时间戳。

最小请求只包含 model 与 input。input 使用有序 step 数组，User／Assistant Message 分别为 user_input／model_output，内容使用 type 标记的 content；不合并独立消息。request.system 只接受文本并按顺序拼接为 system_instruction，省略时不发字段；消息中的 System／Developer 不能被移动到全局，明确报错。History 检查 dialect、无隐式 context、对象与非空 type 后原样追加 steps，签名及未知字段不重建。previous_interaction_id、store、background 通过本次 request.extensions 显式提供；不把 history 自动改成 ID 接续，不替应用保存或继承上一轮的配置。

文本编码为 type=text；图片／文档／音频／视频 Bytes 编码 data 与 mime_type，Url 编码 uri 与 mime_type，不下载、不转码。FileReference 接受同 dialect 的原始媒体 content object 或 URI 引用，uri 必须非空；缺省 type／mime_type 由当前内容种类和显式 MIME 补齐，已提供时要求与当前内容一致，保留其他字段；不将引用中的 Base64 数据悄悄改为 URI 引用。Native content 要求同 dialect 的对象与非空 type。VideoFrames、单独的文档 filename 没有对应字段，显式请求报路径错误。Message、content、tool 和 ToolResult 的扩展分别作用于对应 wire object，不能覆盖其 type／内容／关联管理字段。

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

## 27. Cohere Chat v2 完整响应

CohereChatProtocol(dialect) 提供纯 decodeResponse(value, request)；CohereDialect 解释服务方 usage，CohereChatDialect 使用 DialectId(ApiProtocolId("cohere-chat-v2"), "cohere-chat-v2")。v2 的 message／content、tool_plan、citation 和 usage 形状独立于 OpenAI Chat；请求与 SSE 在各自闭环中接入同一生成入口。

响应要求非空 id、finish_reason 和 assistant message。content 可省略或为 null；数组保持 wire 顺序，连续 text 块进入同一 Message 的多个 Text part，thinking 块成为 Reasoning，未知类型成为 Native。tool_plan 作为独立 Reasoning 放在 content 之后、tool_calls 之前，保留空字符串；普通 text 视图不包含 thinking 或 plan。原始响应、消息及未知字段原样保存；没有模型名或服务创建时间时不由请求或客户端时钟填充。

COMPLETE／STOP_SEQUENCE 映射 Complete／Stop，TOOL_CALL 映射 Complete／ToolCalls，MAX_TOKENS 映射 Incomplete／Length，ERROR／TIMEOUT 映射 Failed／Other(raw)，其他原因映射 Incomplete／Other(raw)。已知终止原因的 usage 为 Final，未知原因保持 Partial。COMPLETE、STOP_SEQUENCE、TOOL_CALL 和 MAX_TOKENS 提供原始 assistant message 的 history；失败与未知停止原因不提供可追加历史。thinking、tool_plan、citations、函数参数原文及额外消息字段随 history 保存。

tool_calls 的实际非空 ID 必须唯一。只有 type=function、name 和 arguments 已提供、请求中存在唯一同名 ClientFunction，且 finish_reason=TOOL_CALL 时才构造完整输入；无效 JSON 或非 object 参数为 Invalid，其他停止原因为 Incomplete。参数文本精确保存，不修复 JSON。缺失 function／name／arguments、未知类型或无法唯一绑定的调用保留 Native；显式空 name 和重复 ID 是协议错误。ClientCustom、ClientNative 与 Server 声明不参与函数绑定；document／tool 引用来源不意味着服务端执行，不生成 ServerToolActivity。

citations 的 content_index 指向原始 content 数组；TEXT_CONTENT、THINKING_CONTENT 与 PLAN 分别关联 text、thinking 和 tool_plan。缺少类型按 TEXT_CONTENT 解释，缺少索引时只关联唯一的同类文本块；无法唯一关联或未知类型的引用保留 Native，不猜位置。索引越界、负 start／end、end 小于 start 报带路径错误；缺失一端仍保存原文。公开 v2 合同未明确字符范围的 Unicode 单位，Citation.range 保持 None，完整引用对象单独保留 Native，不把索引冒充 UTF-8／UTF-16／Unicode scalar 范围。

每个 source 保留实际 id；缺少 id 时以该来源的 JSON 路径作为本响应来源表的局部标识，不把它解释为工具调用 ID。document 与 tool_output 都是应用定义对象，location 使用 Native 保存来源，不能根据其中任意 url 字段推断检索行为。来源对象中的实际字符串 title 可以作为显示标题；其余原文仍可读取。

TokenUsage 使用 usage.tokens.input_tokens／output_tokens，cached_tokens 是已包含在 input 内的缓存读。仅当 input 和 cached 都已知且 cached 不超过 input 时计算 nonCachedInputTokens；未报告缓存读不当作零。input 与 output 都已知时可精确相加得到 totalTokens，溢出明确失败。计数必须为非负、Long 范围内的整数值，允许精确的整数小数／指数写法，不经过 Double。cacheWrite 与 reasoning 细项没有报告时保持 None；billed_units 是另一种计费口径，只在 usage.native 中原样保留，不代替实际 token 计数或伪造金额。

依据：[Chat v2](https://docs.cohere.com/v2/reference/chat)、[reasoning](https://docs.cohere.com/docs/reasoning)、[工具引用](https://docs.cohere.com/docs/tool-use-citations)、[计费与实际 token 的区别](https://docs.cohere.com/docs/how-does-cohere-pricing-work)，以及[官方 SDK 响应定义](https://github.com/cohere-ai/cohere-python/blob/e6595a6d8f43f858327ef2dd5ddcccd96db4749b/src/cohere/v2/types/v2chat_response.py)和[引用定义](https://github.com/cohere-ai/cohere-python/blob/e6595a6d8f43f858327ef2dd5ddcccd96db4749b/src/cohere/types/citation.py)。参考 [Vercel usage 映射](https://github.com/vercel/ai/blob/39aae5425e9f9a25acce413cc7e6823c2b5a73c5/packages/cohere/src/convert-cohere-usage.ts)，沿用本库对未知缓存计数与精确整数的合同。

## 28. Cohere Chat v2 请求与普通生成

CohereChatProtocol 实现 LlmSseProtocol，CohereDialect 提供 encodeOptions(request) 与 decodeError。Providers.cohereChat() 包装 CohereChatDialect，缺省地址为 https://api.cohere.com；generationUrl 在 API 地址前缀后追加 v2/chat，保留查询。模型查询在同一前缀下使用 v1/models，版本属于具体操作路径，不改写调用方提供的地址。应用显式提供 LlmAuth.bearer 和模型 ID，不查模型目录、不读取环境。普通调用沿用 client.generate 的能力检查、寿命、usage 和实际时间戳；流式使用同一地址并添加 stream=true，接收合同见第 29 节。

最小请求只发送 model、messages。request.system 作为首条 system 消息，显式 System／User／Assistant 消息保持原位，Developer 无对应 role 而拒绝。content 使用有序 block 数组，不合并不同消息。History 检查同 dialect、无额外 context、assistant role 后原样追加，包括 thinking、plan、citations 和未知字段。已有 tool_calls 不需要在接续请求重新声明工具。

Text 编码 type=text；User 的 Image 支持 JPEG／PNG／GIF／WebP 的 Bytes 和 HTTP(S) Url，Bytes 编码 data URL，不下载 URL 或改写媒体。图片扩展作用于 image_url 对象，可指定 detail，不能覆盖 url；Text 扩展位于 block。FileReference、音视频和普通 Document 没有同等消息映射，明确拒绝。RAG documents 是全局检索上下文，由 request.extensions 显式提供，不把某条消息的 Document 挪到全局。Native 保留同 dialect 的非空 type 对象；已知 image_url、thinking、document 分别只适用于 user、assistant、tool，未知原生种类保留交给服务解释。

temperature 非负，topP 映射 p（0.01～0.99），topK 映射 k（0～500），maxOutputTokens 映射正数 max_tokens，stop 映射最多五项 stop_sequences；缺省全部省略，显式零或空数组在协议允许时保留。candidates 只接受缺省／1，parallelToolCalls 无开关。reasoning.enabled 映射 thinking.type=enabled／disabled，正数 budgetTokens 映射 token_budget 并启用 thinking；disabled 与预算冲突，effort 无对应字段。空 ReasoningOptions 不发送 thinking，不按模型名推断推理模式。

JsonObject 与 JsonSchema 分别为 response_format 的 json_object 和带 json_schema 的 json_object。name 仅接受公共缺省 response，description 与 strict=false 无等价字段而拒绝；None／true 保持约束输出。response_format 与 tools／documents 参数不能组合，扩展加入 documents 后也检查此规则。库不追加 JSON 提示，应用在消息中明确要求 JSON。

ClientFunction 映射 type=function 与 function.name／description／parameters，不改写 schema；函数名在本请求中唯一。函数扩展作用于 function 对象。strict 映射请求级 strict_tools：所有显式值必须一致，未指定的函数沿用此次请求的全局设置。Auto 省略 tool_choice，None 为 NONE，Required 为 REQUIRED 且需要工具。Specific 仅在请求已只声明目标函数时映射 REQUIRED；多工具集合不能原生指定单个函数，报告错误，不删除其他声明来模拟。ClientCustom、ClientNative 和 Server 没有 Chat v2 的已定义映射而拒绝；documents 或引用不构成服务端工具声明。

ToolResult 使用实际非空 call ID，context 存在时必须是同 ID 的 function call。每个结果成为独立 tool 消息，content 支持 Text 和同 dialect Native（包括 document block），不替应用执行工具或包装任意文本为 JSON。isError=true 无对应标记而拒绝，实际错误可以由应用作为结果内容提供。消息扩展不能覆盖 role、content、tool_call_id、tool_calls、tool_plan 或 citations。

根部扩展允许 documents、citation_options、seed、penalties、safety_mode 等未管理字段，不能覆盖路由、内容、工具、采样、thinking 或 response_format。已解析且含字符串 message 的 HTTP 429 服务拒绝可由现有策略重试；截断、非 JSON、其他状态保留错误与正文，不获得重试资格。

依据：[Chat v2 参数](https://docs.cohere.com/v2/reference/chat)、[图片输入](https://docs.cohere.com/docs/image-inputs)、[thinking 定义](https://github.com/cohere-ai/cohere-python/blob/e6595a6d8f43f858327ef2dd5ddcccd96db4749b/src/cohere/types/thinking.py)、[工具结果定义](https://github.com/cohere-ai/cohere-python/blob/e6595a6d8f43f858327ef2dd5ddcccd96db4749b/src/cohere/types/tool_message_v2.py)。借鉴 [Vercel 的协议映射](https://github.com/vercel/ai/blob/39aae5425e9f9a25acce413cc7e6823c2b5a73c5/packages/cohere/src/cohere-chat-language-model.ts)，同时遵守本库对原始历史、声明集合和显式选项的合同。

## 29. Cohere Chat v2 SSE

createStreamDecoder(request, retention=EventsOnly) 接收已经由 HTTP SSE parser 定界的 event／data，网络入口沿用 openStream／withStream、textDeltas、调用时间和关闭合同。SSE event 名为空或默认 message 时按 data.type 分派；其他名字必须与 type 相同。没有后台读取、自动重连或工具执行。

message-start 建立唯一消息；id 可缺失，后续 message-end 提供的 id 可以补充，已知 ID 不得变化，不用生成字符串代替缺失 ID。起始 delta／message 可缺省或为 null，已提供的非 null role 必须为 assistant。起始 message 中可有空 content／tool_calls／citations 数组、null 或缺省，保留这些实际状态；非空数组不是开始元数据而拒绝。tool_plan 的初始字符串与后续增量顺序相加。起始消息的额外字段保留在 history，不混入可见文本。

content-start／delta／end、tool-call-start／delta／end 和 citation-start／end 各有独立的非负 index 序列，从零开始连续新增；已开始的不同索引可以交错。重复开始、未开始就增量／结束、结束后追加及终止时仍有未结束块均报告协议错误。tool-plan-delta 没有 index，作为独立 Reasoning 累积；message-end 结束 plan。已声明的 content type 决定 text 或 thinking 的增量归属，不能把另一种字段接到该 block；缺失／未知 type 保留 Native，不猜成 text。

流式 item 位置按首次内容／plan／工具事件分配，之后不改变；相邻开始的 text 块共享一个 Message，并使用各自 partIndex，仍可拼接解析结构化输出。thinking 与 plan 不进入普通 text 视图。空起始集合不占用输出位置，已提供但没有增量的空 plan 在最终结果中保留。Result 保存内容及原生历史；EventsOnly 不累计普通 text／thinking／plan，只保留定位、待结束调用和引用关联所需状态。

函数 start 的真实 id 必须非空且本消息内唯一，type／name／arguments 的解释与完整响应一致。arguments 初始字符串和后续片段按序拼接，不修改空串、null 字符串、空白或大整数。只有 tool-call-end 结束且 message-end.finish_reason=TOOL_CALL 后才确定完整输入，JSON 只解析一次；无效或非对象 JSON 为 Invalid，其他停止原因为 Incomplete。未知调用类型、未绑定函数或缺失函数字段保持 Native；无法解释的工具增量使该调用不能执行，并使 history 不可用。待执行 reference 使用完整原始调用，分片事件中的 reference 仅用于关联。

citation-start 解码来源和边界一次，citation-end 才完成该引用。明确 content_index 且目标已经存在的引用、已存在 plan 的 PLAN 引用可以立即交付 typed Citation；缺少索引或指向尚未出现内容的引用延迟到 message-end 再判断唯一归属和边界。晚到引用不移动 item；来源、原文、无法关联的引用仍通过 Native 事件读取。范围的 Unicode 单位未明确，range 仍为 None；document／tool 来源不产生 ServerToolActivity。Result 的 history 保存原始 citations 数组，typed 引用复用已解码来源，不重新解析整份响应。

message-end 必须提供非空 finish_reason；COMPLETE、STOP_SEQUENCE、TOOL_CALL、MAX_TOKENS、ERROR、TIMEOUT 与完整响应采用同样的完成语义，未知原因保持 Incomplete。error 字符串保留 Native；非空 error 与非失败停止原因同时出现时报告协议错误。ERROR／TIMEOUT 是可观察的失败模型结果，不冒充成功或可追加历史。usage 使用终止事件的实际 tokens／cached_tokens，billed_units 仍只保留原文；已知停止原因标 Final，未知原因标 Partial。缺失计数保持未知，不能把 citation 或工具事件当作用量快照相加。

只有 message-end 完成整次流，content-end、tool-call-end、裸 [DONE] 或 EOF 都不能替代。EOF 未终止时失败，独立 error 事件失败；失败后不能读取成功结果。未知事件与未知增量保留 Native，debug 只作为元数据；无法解释为完整消息的变化使 history 不可用，已识别文本仍正常交付。已知 start 快照中的未知字段可以原样保存；不把未解释的 delta 合并成虚构的最终字段。结果不保存整条已知事件日志，结束后释放中间状态。

依据：[SSE 接口](https://docs.cohere.com/v2/reference/chat-stream)、[事件说明](https://docs.cohere.com/docs/streaming)、[工具流示例及起始空字段](https://docs.cohere.com/docs/tool-use-streaming)、[官方流响应类型](https://github.com/cohere-ai/cohere-python/blob/e6595a6d8f43f858327ef2dd5ddcccd96db4749b/src/cohere/v2/types/v2chat_stream_response.py)。参考 Vercel 的增量入口，保留本库的索引关联、最终执行资格、精确 usage 与可序列化 history 合同。

## 30. Ollama Chat 完整响应

OllamaChatProtocol(OllamaDialect) 提供 decodeResponse(value, request)，OllamaChatDialect 使用 ApiProtocolId("ollama-chat") 与同名 dialect。构造器拒绝不同 protocol 的 dialect。Ollama 原生 /api/chat 与 OpenAI 兼容入口采用不同 protocol；纯解码不执行网络、模型目录查询或环境读取。普通生成见 §31，NDJSON 接入见 §32。

完整响应要求非空 model、message object 和 done boolean。message.role 必须为 assistant，content 是允许空串的必需字符串；可选 thinking 作为 Reasoning，content 作为 Message，然后按 tool_calls 的数组顺序交付工具。message 的其余字段整体作为 Native 保留，包括没有 MIME 定义的 images，不猜测媒体格式。原始响应保存在 response.native。created_at 是可选的服务端 Instant，缺失／null 为 None，非法值报字段路径；它不产生客户端计时。原生协议没有响应 ID，response.id 保持 None，未知根字段 id 仍只是原文。

只有 done=true 且 done_reason=stop 表示完整模型输出；存在 tool_calls 时 finishReason 为 ToolCalls，否则为 Stop。length 表示 Incomplete／Length。load／unload 是完成的模型驻留操作，finishReason=Other，不能提供 assistant 接续历史或可执行调用。done=false、缺失／空的 done_reason 和未知原因均保持 Incomplete；缺失／空原因映射 Unknown，其余原文映射 Other。官方内部 ConnectionClosed 的 reason 字符串为空，因此不能以 done=true 或 HTTP 200 代替完整性判断。非 null error 字段表示服务端错误，在 /error 报 LlmProtocolException 并保留原文。

done=true 且原因 stop／length 时，history 保存原始 assistant message，包含 thinking、工具、未知字段以及空／null／缺失的实际状态。其他情况不提供可追加历史。输出对象和 history 继续使用现有 companion codec，模型绑定和认证不进入保存内容。

tool_calls 的每项要求 function object；其可选实际 id 非空且已提供 ID 不得重复，缺失／null ID 保持 None。function.index 只保留为原始位置元数据，已提供值必须非负，不把它合成调用 ID，也不以相同默认 index 合并不同调用。name 缺失／null、arguments 缺失、未绑定或存在多个同名声明时保持 Native；显式空 name 报错。只绑定 ClientFunction，不凭工具名称推断客户端原生工具或服务端执行。arguments 是原生 JSON 值，不是 JSON 文本字符串；完成 stop 下的对象产生 JsonArguments，null、数组、数值和字符串产生 Invalid，其他停止状态产生 Incomplete。缺失 arguments 不能补成 {}。reference.context 保留完整原始调用，包括真正的 ID／index／名称和精确参数。

OllamaDialect.readUsage 接收原始响应根对象。prompt_eval_count 是包含缓存部分的输入总数，prompt_eval_cached_count 是其中的缓存读取量；仅两者都已报告时计算 nonCachedInputTokens，并检查缓存量不超过输入。eval_count 是包括生成 thinking 的输出总数，不另外添加 reasoning；两项总数齐备时以检查过溢出的 Long 加法得到 totalTokens。计数要求非负 JSON integer，缺失／null 保持 None，显式零保留。缓存写入量、独立 reasoning 计数与各模态明细不推测。已知终止原因且 done=true 时 usage 为 Final，否则为 Partial。

usage.native 只投影实际出现的上述三个计数和 total_duration／load_duration／prompt_eval_duration／eval_duration，保留原值，不累计整份 message。duration 字段是服务端纳秒数据，本节不把它们改写为本地调用时长。未识别的根字段仍在 response.native；自定义 dialect 可从原始根对象读取自己的计数。

依据：[Chat API](https://docs.ollama.com/api/chat)、[错误](https://docs.ollama.com/api/errors)、[Message／ToolCall／Metrics 类型](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/api/types.go)、[DoneReason 定义](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/llm/server.go)、[完整响应聚合与 load／unload](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/server/routes.go)。

## 31. Ollama Chat 请求与普通生成

OllamaChatProtocol 实现 LlmGenerationProtocol；OllamaDialect 同时负责 encodeOptions(request) 和 decodeError(status, body, truncated)。Providers.ollamaChat() 只是该 protocol／dialect 的命名组合，缺省 base URL 为 http://localhost:11434/api。generationUrl 追加 chat 路径，保留部署 query；认证和模型 ID 仍显式提供。与其他 provider 共用 endpoint.model、LlmRequest、client.generate 和计时合同，不读环境、不探测服务、不查询模型目录。

正文要求非空 model，并按序编码 messages；普通请求始终带 stream=false，流式请求带 stream=true，因为 Ollama 的服务端缺省是流式。空 messages 有合法的模型加载／卸载含义，允许发送。非空 request.system 作为首个 system 消息；普通 Message 支持 User、Assistant、System，Developer 无对应角色而报请求路径。

Message.content 是单个字符串：按序连接 Text 和同属 dialect 的 Native JSON string；每个 Image 进入同一消息独立的 images 数组，保持图片顺序，不伪造文本中的图片占位。图片只接受 Bytes 和无参数的具体 image MIME，使用标准 Base64；URL／FileReference 不执行隐式读取或下载。Document／Audio／Video／VideoFrames 没有原生 chat 消息映射而报错。content 和图片字符串无 per-part metadata 位置，非空 part extensions 报错。消息 extensions 合并到消息对象，保留 thinking 等显式原生字段，但不可覆盖 role、content、images、tool_calls、tool_name、tool_call_id。history 要求相同 dialect、无隐式 context、assistant 消息，原样重放 thinking、实际调用 ID、JSON 参数和未知字段，不重建为公共文本。

ClientFunction 编码成 type=function 与 function.name／description／parameters；空 description 省略，JSON Schema 保留精确原值。wire name 不得重复；其他工具种类、显式 strict、parallelToolCalls 和非 Auto toolChoice 均无对应字段而报错，不通过删除工具来模拟选择。函数 extensions 位于 function object，不能覆盖 type、name、description、parameters、strict。

ToolResult 编码为 role=tool、tool_name、content 和可选 images；真实 call.id 存在时才发送 tool_call_id，绝不使用 ToolKey 或 function.index 代替。名称优先取 reference.context.function.name，校验原 context 中可选 id 与 reference.id 一致；无 context 时使用本次相同 ToolKey 的 ClientFunction 声明，缺少来源则失败。显式空 ID／空名称、错误的 context 形状、不同 dialect 均失败。isError=true 没有独立 wire 字段而拒绝；应用可在文本中提供服务需要的错误内容。结果 extensions 同样位于消息对象并服从管理字段限制。

temperature、topP、topK、maxOutputTokens、stop 分别进入 options.temperature／top_p／top_k／num_predict／stop；缺省不发送 options，不注入模型默认采样值或输出上限，显式零和空 stop 数组保留。temperature 非负，topP 在 0 到 1 之间，其他范围沿用公共构造器。candidates 只接受缺省或 1，不发送候选字段。reasoning.enabled 映射 think boolean，effort 映射非空、原样的模型自定义 think string；effort 与 enabled=false 冲突，budgetTokens 无对应字段而拒绝。没有推理选项时省略 think。Text 省略 format，JsonObject 使用字符串 json，JsonSchema 使用 schema 本体；无额外 name／description 位置，strict=false 无法表示非约束 schema 而拒绝。

request extensions 合并到根对象；options 子对象可加入 seed、num_ctx 等原生字段，并与公共采样字段组合。model、messages、stream、tools、think、format 以及 options 中的公共管理字段即使未发送也不能被扩展覆盖；自定义 dialect 已编码的字段同样不可被覆盖。keep_alive、runner、truncate 等其余字段保留显式原值，不自动设置。自定义 dialect 的 encodeOptions 不得覆盖 model、messages、stream。

HTTP 错误保留原始 body、截断状态和可解析 JSON，以 error 字符串作为消息；只有完整解析出的 429 服务拒绝可按公共重试设置重试，不将普通 500／503、截断或无法辨认的 body 当成安全重放依据。普通生成继续记录本地请求发送／响应接收 UTC 和单调时长，服务端 created_at 与各 duration 留在各自数据位置。

依据：[Chat API](https://docs.ollama.com/api/chat)、[原生请求、消息和 options 类型](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/api/types.go)、[服务端 ChatHandler](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/server/routes.go)。

## 32. Ollama Chat NDJSON

LlmNdjsonProtocol 扩展 LlmStreamProtocol，提供 createNdjsonDecoder(request, retention = EventsOnly)；LlmNdjsonDecoder 扩展 LlmStreamDecoder，accept(record: String) 消费一条已经定界的 UTF-8 JSON 文本。OllamaChatProtocol 实现该协议，与普通生成使用相同 chat URL 和显式 stream=true。网络层对 application/x-ndjson 使用 scoop.http.asyncio.NdjsonSource；application/json 则按既有正文限制读成单条完整 JSON 记录，允许正文内的排版换行，不当作多条 NDJSON。后者对应原生服务在空 messages 的 load／unload 路径直接返回 JSON 的行为。两种形式均复用同一 decoder、consumeLlmStream、计时、单消费者、取消和关闭合同，不另建流生命周期。

每条记录按原生 ChatResponse 解码：model 非空且在一次流中不变，message.role=assistant，content 为字符串，done 为 boolean。thinking、tool_calls、created_at 等可选字段沿用完整响应的形状检查。非法 JSON 保留原始文本并报 LlmStreamProtocolException；已解析但错误的字段报带原值和路径的 LlmProtocolException；非 null error 在解释消息前失败。空行不是 keepalive，按非法 JSON 处理。

content 与 thinking 是文本增量，分别产生 TextDelta 和 ReasoningDelta；同一字段的后续片段继续使用同一 item／part 位置。item 按首次建立的位置排序，不因后续片段移动；最初的空 content 不预占普通消息位置，终止时仍没有普通消息则补一个空 Message。明确的空 thinking 保留独立 reasoning 项，但不产生 firstOutput。EventsOnly 不积累普通文本，Result 保存完整文本、候选和原生接续历史。公共文本增量帮助类型由 Chat 和 Ollama 实际共用，不改动现有 Chat 语义。

tool_calls 每条记录提供完整的调用记录，按到达顺序追加，不按 function.index 合并；实际非空 ID 在整次 assistant 消息内唯一，缺失 ID 保持 None。函数归属、缺失字段和 JSON 参数的解释复用完整响应，参数只构造一次 JsonToolArguments，不在终止时重新解码全部调用。到达时可发送 ToolInputDelta；可执行 ClientToolCall 要等待 done=true 且 done_reason=stop。其他停止原因的已识别调用使用 Incomplete，不把部分流当成完整输入；未知／未绑定的调用保持 Native。未声明服务器执行就不生成 ServerToolActivity。

done=true 结束协议输入；stop、length、load、unload 以及空／缺失／未知原因的完成语义与 §30 相同，不以 done 本身保证模型输出完整。只有 stop／length 的 Result 可提供 assistant 历史，load／unload 不提供。普通 EOF 不能替代 done=true，失败后不能读取成功结果，结束后也不再接受新记录。末尾无换行的合法记录由底层定界器交付，仍必须满足上述终止规则。

Result 的历史连接已出现的 content／thinking 字符串，并原样追加 tool_calls；缺失／null 的可选片段不产生虚构文本或调用，明确的空字符串／空调用数组仍可保存。未知消息字段作为 Native 输出与事件保留；无法确认其跨记录合并语义时令 history 不可用，不猜测覆盖或拼接。未知根字段作为 Native 事件保留，不影响已识别消息的接续。正常记录的整份已知 JSON 不作为事件日志反复保存，结束后释放中间聚合状态。

根部 token 计数和服务端 duration 是累计报告，后续已报告值替换旧值而不相加，缺失／null 不抹去先前已报告值。OllamaDialect.readUsage 接收当前记录的根对象，其中这组指标按累计报告补齐；其他字段仍来自当前记录，以便 dialect 读取实际扩展。usage.native 只保存指标投影，不保存整份 message；只有已知终止原因才标 Final，未知原因保持 Partial。存在实际计数／指标或终止时交付 Usage 更新，缺失的初始计数继续为 None。

created_at 可以随每条记录变化，不作为不变身份检查；response.createdAt 使用终止记录实际提供的可选时间。它与 duration 均不改写本地 request.sentAt、response.receivedAt、firstEvent、firstOutput、firstText 或完成时间。输出标记只反映本次记录中新到达的文本／reasoning／调用等内容，累计文本和使用量更新不会重复触发首次输出计时。

依据：[NDJSON 流](https://docs.ollama.com/api/streaming)、[工具调用](https://docs.ollama.com/capabilities/tool-calling)、[按字段连接完整响应与逐帧服务时间](https://github.com/ollama/ollama/blob/d44c2581904a98430e348d89cc2af1529b1808a0/server/routes.go)。

## 33. HTTP 阶段计时

LlmPhaseTiming(started: Duration, finished: Duration? = None) 保存一个实际 HTTP 阶段相对本次逻辑调用起点的单调偏移，started 必须非负，已提供 finished 不得早于 started。duration 仅在 finished 存在时返回二者之差，显式零保留，正在进行的阶段保持 None。数据提供 companion codec，不保存进程内 TimeMark，不从 UTC 推算阶段偏移或用回调执行时间代替原采样。

LlmHttpTiming(queue = None, resolve = None, connect = None, proxyTls = None, proxyHandshake = None, originTls = None) 的各项均为 LlmPhaseTiming?，分别对应 HTTP 层实际观察到的排池、DNS、TCP、代理 TLS、CONNECT／SOCKS 握手和目标 TLS。LlmAttemptMetadata 在既有参数之后增加 http = LlmHttpTiming()，旧数据缺少该字段时恢复空缺省。连接复用或未执行的阶段保持 None，不填成零；不把可能重叠的阶段相加成一个虚构总耗时。

普通生成和流式调用从该尝试最近的不可变 HttpAttemptMetrics 直接投影上述阶段，使用相同的 LlmCallState 起点；重试之间不合并或继承阶段。snapshot 与 onFinished 均保存当时实际可观察的内容，之前取得的快照不随后续进展改变。HTTP 在失败或取消时也会结束已经进入的阶段，因此 finished 表示观察区间已经结束，不保证该阶段成功；调用结果仍由 outcome 和原异常表达。认证前失败等尚未取得 HTTP 观察的路径使用全空阶段，不伪造传输数据。

此投影复用既有请求级 observer，与借入 HttpClient 的原 observer 共存；不改变池、重试、超时或连接级首字节采样设置。底层尚未提供 UTC 的首字节和正文发送完成点继续保持 None。outputTokensPerSecond 还要求 usage.completeness=Final，避免把中途累计计数当作最终平均速率；模型因长度停止但完整返回了最终用量时仍可计算该端到端速率。

## 34. 显式输入 token 计数

LlmClient.countTokens(model, request, options = LlmCallOptions()) 返回 LlmTokenCountResult(count, metadata)。这是独立的服务器操作，不自动生成、不在 generate／openStream 前执行，也不以预计数覆盖 response.usage。当前适配 Gemini GenerateContent、Anthropic Messages 和 OpenAI Responses（§41）的计数接口；不提供没有 tokenizer 实现的本地模式或字符数回退。ModelCapabilities.serverTokenCount 缺省 Unknown；显式 Unsupported 在认证前失败，Unknown 允许由已选 adapter 和服务器判断。

LlmApiProtocol.tokenCounter() 缺省 None；LlmTokenCounter 定义 countUrl、encodeRequest、decodeResponse 三个纯适配操作。Messages／GenerateContent／Responses protocol 将选择委托给 dialect 的同名可选方法。内置 dialect 提供实际 counter，自定义 dialect 缺省不继承另一个服务商的计数语义。没有 counter 在认证前报告 LlmRequestException。

TokenCount 是可序列化的纯数据：dialect、请求的 model、可选 modelVersion、method=Server、scope=RequestInput、precision=Estimated、可选 inputTokens／cachedInputTokens、modality 和 native。TokenCountMethod 当前只有实际可用的 Server；TokenCountScope.RequestInput 表示提交给计数端点的完整输入表示，包括接口明确解析的输入引用，不保证覆盖未报告的服务端隐藏包装或未来工具输出。TokenCountPrecision 可为 Estimated 或 Exact；Gemini／Messages 相对于后续生成标为 Estimated，Responses 按该端点的明确合同标为 Exact。Exact 描述此次输入计数，不保证随后发生变化的历史、模型版本或工具执行仍有相同用量。缺失计数和版本保持 None，明确零保留，非整数、负数和 Long 溢出拒绝。modality 只保存输入方向明细，不重复加到总数；cachedInputTokens 是计数时的缓存输入规模，不是一次生成的 cacheReadTokens／cacheWriteTokens。LlmTokenCountResult 仅接受 Completed metadata，并提供 companion codec。

计数与普通生成共用逻辑调用寿命、认证前模型检查、发送字节、错误分类、重试、取消、总超时、HTTP 观察和 onFinished。每次重试重新取得认证；只有真实服务拒绝具有既有重试资格。结果保存实际发送／收到响应／完成时间和 HTTP 阶段，失败或取消仍通过 onFinished 交付 metadata。计数不产生模型 firstEvent／firstOutput／firstText、responseId、输出速率或生成 usage。onFinished 仍在调用上下文内、完成资源收尾后恰好执行一次。

共享 HTTP 响应观察优先读取单个 x-request-id，没有该值时读取单个 request-id，覆盖 Messages 的实际响应头；重复字段不拼接成虚构 ID。

Gemini 使用所选模型资源的 :countTokens，正文为 generateContentRequest，包含由普通生成编码器产生的完整 contents、systemInstruction、tools、generationConfig、原生扩展及明确 model；不同时提交简化 contents。裸 ID 规范为 models/ID，已有资源路径和部署前缀沿用。totalTokens、cachedContentTokenCount、promptTokensDetails 直接读取；cacheTokensDetails 校验并保留在 native 中，不重复求和。不能从 cachedContentTokenCount 推断缓存被读、被写或续期。

Messages 使用 /messages/count_tokens，相同部署、认证和版本头。正文复用普通请求的 messages、system、工具和原始历史编码，保留 tools／tool_choice／thinking／output_config；不发送 max_tokens、stream、采样或停止参数。已有显式生成选项仍经过模型能力检查，但计数不会注入缺省输出上限来约束 thinking 预算；显式输出上限仍参与预算一致性检查。请求扩展只允许计数端点的 cache_control 和 speed 等已支持输入字段，不静默丢弃其他扩展。当前端点不接受 MCP connector、除 advisor 外的 server tool、图片／文档的 URL 或 file 来源，编码前后在实际协议结构上检查并报错；检查工具结果／文档内容等协议容器，不递归解释工具参数中的任意业务 JSON。base64 媒体和支持的文本内容照常计数，缺失 input_tokens 保持未知。cache_control 不代表此次操作执行了 prompt caching。

依据：[Gemini countTokens](https://ai.google.dev/api/tokens)、[Messages count_tokens](https://platform.claude.com/docs/en/api/messages/count_tokens)、[Claude 计数范围与限制](https://platform.claude.com/docs/en/build-with-claude/token-counting)。普通验收仅使用纯 codec 与 loopback mock；真实 API 用例继续默认 Ignore，只有显式选择且配置对应凭据时才运行。

## 35. AWS SigV4 请求认证

LlmAuth.awsSigV4(region, credentials: suspend () -> AwsSigV4Credentials, service = "bedrock") 返回现有 LlmAuth 的实现。region 显式提供，service 缺省适用于 Bedrock Runtime；其他实际 AWS LLM 部署可显式指定其签名服务名。AwsSigV4Credentials(accessKeyId, secretAccessKey, sessionToken = None) 是一次凭据快照，不提供 codec，不进入模型、请求内容或调用 metadata。凭据源由应用提供，每次实际尝试调用一次，可挂起并返回更新后的临时凭据；库不自动查找环境、文件、实例角色或登录账号，不建立凭据缓存与刷新状态机。来源异常与取消沿既有认证路径传播，未提交时不产生发送时间戳。

签名复用最终 LlmAuthRequest 的 method、URL、headers 和准确 body 字节，使用 scoop.crypto 的 SHA-256 与 HMAC；不重新编码请求。凭据取得后读取当前 UTC，生成 X-Amz-Date、可选 X-Amz-Security-Token 及 Authorization；敏感认证字段使用 neverIndex。Host 由最终 URL 按 HTTP client 相同的 authority 规则推导（包括显式端口），只参与签名，仍由 HTTP 传输生成。路径按 SigV4 的非 S3 规则规范化并再次编码路径段；query 使用最终 URL 中已编码的键和值排序，保留重复项与空值。header 名小写并排序，同名值按原顺序以逗号连接，去除首尾 SP／HTAB 并折叠中间空白，保留其余原始字节。传输生成或易变的 content-length、hop-by-hop、user-agent 和追踪字段不参与签名；其他显式字段包括 x-amz-* 参与签名。请求已有 Authorization、X-Amz-Date、X-Amz-Security-Token 时明确拒绝冲突；显式 X-Amz-Content-Sha256 必须与实际 body 摘要一致，不接受 UNSIGNED-PAYLOAD。

普通生成、流式和已支持的显式操作共用认证入口，重试重新获取凭据并签名；一次响应的 EventStream 不改变请求签名方式。区域与服务名不从模型或 hostname 猜测；签名发生在排池前，不保证排队后签名仍未过期。调用者的 HTTP 扩展不应在签名后添加签名所需的 AWS 字段。此入口实现带完整 body 摘要的 SigV4 header 认证，不提供 presigned URL、SigV4a 或流式请求体签名；Bedrock API key 仍可用既有 bearer 入口。

依据：[AWS SigV4](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv-create-signed-request.html)、[Bedrock API key](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html)、[官方 Bedrock Runtime 定义](https://github.com/boto/botocore/blob/fecd1e19cf8e1c04b66480397455e1458867bf6d/botocore/data/bedrock-runtime/2023-09-30/service-2.json)及[官方 SDK 签名实现与测试向量](https://github.com/boto/botocore/tree/fecd1e19cf8e1c04b66480397455e1458867bf6d/tests/unit/auth/aws4_testsuite)。使用固定向量和本地网络 mock 验证签名、路径／query、临时 token、重试、普通及 EventStream 调用；未使用真实 AWS 凭据，真实互通另行验证。

## 36. Responses WebSocket 线协议

ResponsesProtocol.webSocketUrl(baseUrl) 在既有部署路径后追加 responses，将 http／https 转为 ws／wss，保留路径前缀和 query；已有 ws／wss 同样可用。URL 必须具有有效的 HTTP origin 且没有 fragment。协议与 dialect 不因传输变化而更换。

encodeWebSocketCreate(modelId, request, options = ResponsesWebSocketCreateOptions()) 复用普通 Responses 的输入、工具、生成设置、原生扩展和历史编码，只在最终对象加入 type=response.create 及显式 WebSocket 设置。options 的 streamId 和 generate 均缺省 None，分别使用服务端默认 lane 和正常生成；明确 generate=false 支持预热。streamId 仅允许 1～256 个 ASCII 字母、数字、下划线、连字符或点。stream／background 不属于该事件，显式 background（包括 false／null）报输入错误。type、stream_id、generate 不能从请求扩展或 dialect options 覆盖；普通 HTTP 请求同样拒绝这三个 WebSocket 专属字段。previous_response_id 等既有 continuation 字段仍由 request.extensions 显式提供，不根据 lane 或模型名补造。

encodeWebSocketSteer(previousResponseId, input: Array<LlmInputItem>) 只编码 type=response.steer、非空的 previous_response_id 和非空 user message 数组。共用普通输入和内容编码，支持文本、图片与文件；拒绝其他角色、工具结果、History 以及 message 上超出 type／role／content 的字段。编码后的 content 只允许字符串或 input_text／input_image／input_file 数组，检查实际协议容器，不递归解释文件等原生字段的业务内容。该入口不接受 streamId，目标 response 由应用明确指定；模型或当前执行模式是否接受 steering 仍由 capability 与服务器判断，不按模型前缀猜测。

ResponsesWebSocketEvent(dialect, native) 保存完整 JSON，提供 type、可选 streamId 与 kind 读取视图及 companion codec。kind 区分 Response（response.*）、Steering（优先识别 response.steer.*）、Error（error）和 Native（其他事件）；未知事件及所有附加字段保持原值。只解码路由必需的非空 type 和可选字符串 stream_id；null／缺失的 stream_id 保持 None，不校验其他通知或附加信息的内容。ResponsesProtocol.decodeWebSocketEvent(data: String) 对非法 JSON 报保留原文的 LlmStreamProtocolException，对错误路由字段报保留原值与路径的 LlmProtocolException。

ResponsesStreamDecoder 作为公开的单响应聚合器，构造参数为 protocol、request 和缺省 EventsOnly 的 retention；既有 SSE 工厂仍返回同一实现。新增 accept(ResponsesWebSocketEvent)，只接受同 dialect 的 Response 类事件，并在首次消费时绑定实际 streamId。错误的类别、归属或 lane 在改变聚合状态前失败；调用者仍可将正确事件交付给原解码器。已绑定 lane 的事件在移除路由字段 stream_id 后直接进入共用 JSON 聚合路径，不 stringify 后重解析，也不把 lane 当作输出内容或独立 Native 事件。序列、item／part 身份、工具执行资格、终止、usage 和历史恢复沿用 §17；每个 response 使用独立 decoder，不将后继响应或其他 lane 送入已结束的聚合器。

Steering 通知和 error 的作用范围由会话层处理；这组纯协议入口不会仅因收到通知而结束某个响应、自动创建后继、执行工具、重放请求或推断缓存状态。steered 仍是实际 incomplete／Pause 结果，后继 response.created 属于新的响应。纯编解码不伪造网络收发时间；网络会话的资源寿命与按实际消息采样由后续接入承担。

依据：[WebSocket 模式](https://developers.openai.com/api/docs/guides/websocket-mode)、[连接入口](https://developers.openai.com/api/reference/resources/responses/methods/connect)、[steering](https://developers.openai.com/api/docs/guides/steering)与[事件定义](https://developers.openai.com/api/reference/typescript/resources/responses)。验证使用协议 JSON mock 和现有 Responses 聚合器；真实服务互通待凭据可用后补测。

## 37. Responses WebSocket 网络会话

LlmClient.openResponsesSession(endpoint, handshakeOptions = LlmCallOptions(), webSocketOptions = HttpWebSocketOptions()) 返回 AsyncCloseable 的 ResponsesSession；withResponsesSession 使用同一入口与 useAsync。endpoint 必须选择 ResponsesProtocol，连接固定其部署、dialect 与认证，同连接可以使用该 endpoint 创建的不同 LlmModel。没有 provider／模型目录查询或模型名路由。会话由 LlmClient 持有并随 client 关闭，借入的 HttpClient 仍归调用者所有。打开期间随打开调用者取消，交付后会话寿命由显式关闭、所属 client 或底层连接结束控制。

握手为实际 HTTP GET，认证使用相应 http／https URL、最终显式 fields 和空 body；共用普通请求的认证冲突、错误正文、服务拒绝分类和重试规则。handshakeOptions 只作用于握手，包括总超时、响应头等待、失败正文上限与 onFinished；成功后的持久会话使用 webSocketOptions 的帧、消息、压缩和关闭设置。握手完成时交付独立的 LlmCallMetadata，保存在 session.handshake。认证或握手回调失败必须关闭已取得的连接。成功握手不生成模型输出、usage 或首文本时间，不能作为任何生成消息的收发时间。

session.create(model, request, options = ResponsesWebSocketCreateOptions()) 使用 §36 的编码器，验证模型属于同一个 endpoint、模型输入及 streaming capability。session.steer(model, previousResponseId, input) 验证相同 endpoint、模型输入与 realtime capability，然后编码 steering；目标 response 与模型的关联由调用者提供，服务器判断该 response 的实际执行状态。两者使用同一消息发送路径，不自动补 previous_response_id、执行工具或重放消息。调用者串行发送消息；可同时进行一个发送和一个接收，同方向重叠操作在改变连接状态前报错。

成功发送返回可保存的 ResponsesWebSocketSent(sessionId, command, timing: LlmRequestTiming)。command 是最终 JSON 消息，不包含握手认证。sent 在完整消息已编码、取得发送 writer 后、向传输提交首段内容前采样；bodyCompleted 在 writer.finish 成功后采样。该时间表示本地消息提交与完成，不能证明服务器接受或生成成功。发送失败沿原异常路径传播并结束连接，不推断是否可安全重发。

session.receive() 按需接收一个完整文本消息，返回可保存的 ResponsesWebSocketReceived(sessionId, event: ResponsesWebSocketEvent, received: LlmTimePoint)，正常 WebSocket 结束返回 None。received 在完整消息从传输取出后、JSON 解码前采样；它不是 TCP 首字节时间。session.id 与 startedAt 标识这条连接的本地时间轴，所有消息时间点的 elapsed 相对于该会话起点，使用单调时钟；UTC 与 Duration 均可保存，不保存活动 TimeMark。每个收到的事件独立采样，握手时间不混入消息记录。

接收循环按实际 streamId 和 response ID 将事件交付给各自的 ResponsesStreamDecoder，沿用 EventsOnly／Result、usage、工具和历史合同。同 lane 的显式队列、自动 steering 后继与控制事件保持服务器的实际顺序；receive 不把 error 自动归到某个正在运行或尚未开始的 create，也不伪造请求关联 ID。Error／Steering／Native 均作为原始事件交付，服务错误不自动关闭其他 lane。每个响应由其终止事件结束，连接 EOF 不能代替 decoder.endInput 的完整性判断。

消息读写、JSON 语法或路由字段失败会结束会话，取消会等待底层清理；已交付给操作调用者的同一个失败不在 useAsync 的 close 中重复抛出。显式 close 可取消正在等待的操作并等待真实释放；client.close 同样等待其会话。握手回调中关闭本次仍在打开的 client 继续遵守已有自等待保护。协议校验、能力检查与同方向重入错误在消息 I/O 前失败，已经打开的会话仍可接受后续合法操作。会话不保存事件 replay，不启动消费全部模型输出的后台任务，不自动重连、恢复队列或判断缓存是否失效。

验证使用本地 HTTP 升级与 WebSocket 服务端 mock，覆盖同连接交错响应、工具与历史、steering／错误通知、实际收发时间、认证重试、取消／关闭和握手失败清理；真实服务互通待凭据可用后补测。

## 38. DeepSeek Responses dialect

Providers.deepSeekResponses(topPMode = Unknown) 固定组合 ResponsesProtocol 与 DeepSeekResponsesDialect，标识为 (responses, deepseek-responses)，部署缺省 https://api.deepseek.com。使用既有 generate／openStream／withStream，模型 ID 原样发送，不查询目录或根据名称切换行为。topPMode 复用 DeepSeekTopPMode，只有显式 topP 才需要选择模型已知的生效模式。

最小请求只发送 model 与 input；至少需要 input item 或 system。system 映射 instructions，角色按 Responses 编码；DeepSeek 将 developer 解释为 user。文本与 JPEG／PNG／GIF／WebP 图片使用既有字符串、data URL、http(s) URL 或同 dialect 文件 ID。图片只用于 User／Developer 和工具结果；System／Assistant 中的图片报路径错误。Document、Audio、Video 和 VideoFrames 没有此接口的映射而拒绝。ResponsesDialect.encodeContent 增加缺省 None 的 role 参数，普通消息传入实际角色，None 表示工具结果；OpenAI 保持已有行为。原生内容及各位置扩展继续保存归属、原文和冲突规则，明确不支持的 input_file 不因 Native 而获得映射。

reasoning.effort 使用 none／low／high／max 及服务端接受的 minimal／medium／xhigh 别名，原值发送；enabled=false 映射 none，enabled=true 且无 effort 沿用该接口默认开启 thinking 的行为，不替用户选择推理档位。thinking 时显式 temperature 被拒绝；topP 按显式 topPMode 检查。parallelToolCalls 以及 topK／stop／多候选等无法表达的控制报错。text.format 的 JsonObject／JsonSchema 复用 Responses 映射，其他生成参数保持缺省省略。

ClientFunction 使用平铺 function，名称遵守 DeepSeek 的非空、至多 128 字符及字符集规则，schema 原样发送。Responses 文档未定义 function strict 控制，显式 strict 拒绝，不借用 Chat beta 的合同。ClientCustom 仅映射名称 apply_patch 的 Text 输入；自定义 grammar 无对应控制而拒绝。WebSearch 和同 dialect 的原生 web_search 为服务端工具；通用搜索的域名、地域与 includeSources 选项没有已定义映射而拒绝，原生配置仍通过普通扩展传入。其他 ClientNative／Server.Native、显式原生版本不映射。服务端搜索的模型支持由模型能力描述，不能根据模型名猜测。

Auto 省略 tool_choice，None／Required 使用同名字符串；Specific(function) 使用 type=function 与 name。Specific(custom／server) 只有本次声明仅包含该工具时才用 required 表达，不能删除其他声明来模拟。工具名称在请求中唯一；函数与 apply_patch custom 的实际输入、服务端搜索活动、引用及工具结果复用既有 Responses 解码与关联。

此接口无服务端 conversation 状态。每轮显式携带完整历史；已保存 output items 保持原样，reasoning 的 content 及搜索结果不丢弃。输入中的 function／custom call 和对应 output 必须按实际非空 call_id 配对，声明和结果各自不得重复且类别匹配，不插入或执行缺失的结果。旧 response ID 和 item_reference 不能代替历史。已知不支持的 previous_response_id、conversation、store、background、metadata、include、prompt、truncation、service_tier、safety_identifier、prompt_cache_key、prompt_cache_retention、context_management、stream_options、max_tool_calls 在扩展中显式提供时也拒绝；其余扩展由服务端解释。

DeepSeek reasoning 的 summary 缺失／null 合法，content 中的 reasoning_text 进入 Reasoning；OpenAI dialect 继续要求自己的 summary 字段。完整响应与 SSE 共用有序 item、reasoning channel 位置、客户端执行资格、服务端工具、usage 和历史聚合，缺少终止事件的 EOF 仍失败。input_tokens／output_tokens／total_tokens、cached_tokens 与 reasoning_tokens 沿既有 Responses 口径读取；缺失计数不补齐。服务时间与本地收发时间分开，其他服务端附加信息保持原文。

HTTP 429／503 按 DeepSeek 的限流／过载拒绝策略重试，保留原始错误与正文，每次重新认证并采样；模型 failed 响应不重试。当前文档定义的 DeepSeek Responses 入口是 HTTP／SSE；ResponsesDialect.supportsWebSocket 表示该适配器是否实现 WebSocket 操作，OpenAI 为 true、DeepSeek 为 false，不支持时在认证或发送前报错。它描述适配器操作，与模型的 streaming／realtime capability 分开。

验证使用官方响应形状和本地 HTTP／SSE mock，覆盖缺省请求、参数差异、图片、函数／custom／搜索组合、保存后完整历史回传、reasoning 流、usage 与实际计时；不依赖供应商服务或 key。依据：[DeepSeek Responses 兼容说明](https://api-docs.deepseek.com/guides/responses_api/)、[请求与响应参考](https://api-docs.deepseek.com/api/create-response/)、[工具调用](https://api-docs.deepseek.com/guides/tool_calls/)、[HTTP 错误码](https://api-docs.deepseek.com/quick_start/error_codes)。

## 39. OpenRouter Chat dialect

Providers.openRouterChat() 固定组合 ChatProtocol 与 OpenRouterChatDialect，标识 (chat-completions, openrouter-chat)，默认部署 https://openrouter.ai/api/v1。generate／openStream／withStream 的入口、缺省值、模型 ID 原样传递和能力检查不变。模型目录、路由、文件上传和 Agent 调度不隐式执行；服务方自己的附加字段保持原文。

推理选项映射 reasoning.enabled／effort／max_tokens；effort 与 budgetTokens 不能同时提供，显式禁用不能同时要求正的推理预算或非 none effort。不按模型名选择推理模式或替用户估算预算。temperature／topP／topK／stop／maxOutputTokens／parallelToolCalls 使用对应 Chat 字段，输出上限使用 max_tokens；JsonObject／JsonSchema 使用 response_format。candidates 只接受 1，省略时沿用服务默认值。根扩展不能覆盖这些已有控制；没有设置通用 reasoning 时，可通过扩展提供完整原生 reasoning 对象；其他 routing／plugins 等选项仍通过原生扩展显式提供。

客户端 function 与 openrouter:* 服务端工具共享 tools；前者使用既有函数声明和 call_id，后者的参数按原文发送，不进入客户端执行列表。WebSearch 映射 openrouter:web_search，allowedDomains／excludedDomains 与 location 放入 parameters；includeSources 无对应控制而拒绝。原生工具使用同 dialect、无显式 version 的配置；已知仅支持其他协议的 apply_patch／shell／bash／tool_search 不映射到 Chat。Specific(function) 使用具名函数；Specific(server) 只有声明中仅有该工具时才用 required 表达。没有文档对应形状的 ClientCustom／ClientNative 拒绝。服务端不报告逐次活动时，不从 usage 或 citations 生成工具活动。

输入保留单段纯文本的字符串缺省；图片使用既有 image_url，PDF／其他文档使用 file.file_data 的 data URL 或 http(s) URL，音频使用带格式的 inline Base64，视频使用 video_url 的 data URL 或 http(s) URL。没有明确映射的文件引用、PCM 格式和带时间戳帧序列通过 Native 显式表达，不猜测格式或伪造文件 ID。文本／媒体／工具各位置的扩展继续遵守归属和字段冲突规则。

完整响应优先将 reasoning_details 中 reasoning.text／reasoning.summary 投影为可读 Reasoning，encrypted 和未知块保留 Native；若没有非空 details，再读取 reasoning 或 reasoning_content。历史保存完整 assistant 消息，保留原始推理字段及 annotations（包括文件解析回传信息），不删除无签名文本，不补签名、reasoning_details 或结束原因。原生 tool-call 字段的执行资格仍以完整参数和实际结束状态为准。

ChatDialect.createStreamField(field, location, retention) 提供每个 candidate 内的可选字段 decoder，缺省 None；现有 Chat 与 DeepSeek 行为不变。ChatStreamFieldDecoder.fields 列出共同处理的字段，一次 accept 收到本次完整 delta，每个 decoder 每个 delta 至多调用一次，返回既有 LlmStreamUpdate。finish 返回一个有序 output item、该字段组的 historyFields 与历史是否可重建；协议仍负责公共消息、工具、结束原因、usage 和位置。字段组不能覆盖公共 role／content／tool_calls 等字段。它用于当前 OpenRouter 的结构化 reasoning 与原生 annotations 等具体差异，不建立通用事件规则或插件系统。

reasoning_details 的 text／summary 增量按连续块合并；不同 kind、实际 ID／index／format 或已完成的 text 签名开始新块，encrypted 和未知块各自保留，不按 index 排序或将其作为唯一身份。同一 delta 同时出现 details 与 reasoning 字符串时，可读视图优先 details，历史仍保留两者。签名可晚于正文；文本已开始不阻止继续交付 reasoning。显式空 details 保持空数组，未报告的字段不补出。未知块和额外字段经 Native 事件保留。EventsOnly 不累计正文／加密负载，Result 保存有序输出及可重放历史。

annotations 按实际数组增量保存，文件 annotations 随历史回传；未知形状原样交付，不验证服务附加信息内容。输出 images／audio 使用原生表示；音频分块按既有 Base64 规则累计，expires_at 只保留原值，缺少 ID 仍可交付结果，但不制造可回传的音频引用。ChatDialect.readNativeItems 选择完整响应的原生输出字段；canReplayMessage 缺省 true，在缺少音频引用时返回 false，普通及流式结果均不生成不完整的历史。HTTP 错误正文保留原文，限流／无可用服务的 429／503 可重试，已收到模型输出后不重放。token usage 使用 Chat 的实际计数，其他字段不进入公共计数或费用模型。

验证采用官方形状的本地 HTTP／SSE mock，包含混合工具、完整历史保存和恢复、独立签名块、跨 candidate 交错、usage 尾块、错误与真实时间戳；真实服务另行补测。依据：[reasoning](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)、[Chat API](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion)、[服务端工具](https://openrouter.ai/docs/guides/features/server-tools)、[WebSearch](https://openrouter.ai/docs/guides/features/server-tools/web-search)、[多模态](https://openrouter.ai/docs/guides/overview/multimodal/overview)及 [OpenRouter AI SDK reasoning 增量实现](https://github.com/OpenRouterTeam/ai-sdk-provider/blob/d8edda07b62de696d613ae0115823382465665cd/src/chat/index.ts)。

## 40. Responses 服务端状态操作

LlmClient.retrieveResponse(endpoint, responseId, tools = [], include = [], options = LlmCallOptions()) 显式读取已经创建的 response；cancelResponse(endpoint, responseId, tools = [], options = LlmCallOptions()) 显式取消 background response；deleteResponse(endpoint, responseId, options = LlmCallOptions()) 显式删除已存储的 response。三者使用同一 client 和 endpoint，不需要模型目录或重复提供 model ID，不执行轮询、任务调度或自动删除。background／store 继续通过已有的 Responses 请求扩展设置；服务端决定资源是否存在、可取消或可删除。

ResponsesDialect.supportsStoredResponses 缺省 false，OpenAiResponsesDialect 为 true，DeepSeek 保持 false。该属性描述适配器提供的操作，与模型的生成能力分开。非 Responses protocol 或未实现存储操作的 dialect 在认证和 I/O 前报 LlmRequestException。responseId 非空，作为一个原始 URL path segment 编码；保留 endpoint 的部署路径和 query。retrieve 使用 GET /responses/{response_id}，include 按重复 include[] query 发送，非空时覆盖部署 query 中的同名 include／include[]，空数组沿用部署 query，值不设封闭枚举；cancel 使用 POST /responses/{response_id}/cancel；delete 使用 DELETE /responses/{response_id}。这些操作不发送 JSON body，认证看到实际 method、URL、fields 和空字节正文。读取入口返回单个 JSON 快照，不启动流或恢复事件。

读取和取消返回既有 LlmResult，按同一 Responses decoder 解释文本、reasoning、客户端／服务端工具、终止状态、usage 与原始历史。tools 只用于本地关联，不发送给服务端，不把服务端返回的工具声明自动注册为客户端工具；缺省 [] 时未关联的调用仍保留 Native。queued／in_progress 保持 Incomplete 和 Partial usage；cancelled 保持 Incomplete 和 Final usage，不将取消冒充完整生成。原始错误及未知服务端附加字段继续保留，不校验计费或缓存等附加信息。

删除确认使用 ResponsesDeletion(dialect, id, deleted, native)，保留实际布尔值与完整原文，不将 HTTP 成功等同 deleted=true；wire object 必须为 response.deleted。ResponsesDeletionResult(deletion, metadata) 在本地调用清理完成后返回，两种数据支持 companion codec。ResponsesProtocol 提供 retrieveUrl／cancelUrl／deleteUrl 及 decodeDeletion 纯入口，供相同协议的独立消费和本地 mock 使用。

三种操作复用已有认证、服务拒绝重试、截止、调用方取消、client.close、回调与 HTTP 资源所有权。每个 attempt 的时间戳来自此次实际传输，completed 表示本次操作解析完成，responseId 仅取服务端实际 ID；查询／取消返回的历史 usage 只位于 LlmResponse，不填入此次 attempt 的生成用量、首个模型事件／输出／文本或输出速率。删除同样不产生生成用量。协议／解析错误不重试；HTTP 拒绝按既有 dialect 策略处理，每次重试重新认证。

本地 scoop.test 使用官方形状与 loopback HTTP mock，覆盖创建 background 后读取／取消／删除、工具历史、方法与部署路径、重试认证、实际计时及失败／取消清理；普通用例不连接供应商。真实服务用例继续默认忽略，凭据可用后再补互通验证。依据：[读取 response](https://developers.openai.com/api/reference/resources/responses/methods/retrieve)、[取消 response](https://developers.openai.com/api/reference/resources/responses/methods/cancel)、[删除 response](https://developers.openai.com/api/reference/resources/responses/methods/delete)、[background 模式](https://developers.openai.com/api/docs/guides/background)。

## 41. Responses 输入 token 计数

OpenAiResponsesDialect.tokenCounter() 提供实际 Responses counter；ResponsesDialect 的缺省实现为 None，DeepSeek Responses 不继承 OpenAI 的计数入口。通过同一 LlmClient.countTokens(model, request) 调用，复用 §34 的能力检查、HTTP 寿命、认证、服务错误、重试与计时。地址为 endpoint 部署路径下的 POST /responses/input_tokens，保留 query，model ID 原样发送；不要求模型列表或预先生成 response。

计数请求复用普通 Responses 的输入、工具与参数编码，保留 model、input、instructions、tools、tool_choice、parallel_tool_calls、reasoning、text、conversation、previous_response_id、personality、truncation。消息、system、图片、PDF 的内联数据／URL／同 dialect 文件引用、客户端／服务端工具、完整原始历史与工具结果保持同一 wire 表示；只选择根字段，不递归过滤工具参数或原生内容。显式 conversation／previous_response_id 交由服务端解析，不从本地补齐上下文或预测后续状态。

temperature、top_p、max_output_tokens、include、background、store、metadata、service_tier、user、safety_identifier、prompt_cache_key、prompt_cache_retention、stream_options、top_logprobs、max_tool_calls 只控制生成、返回或服务处理，不属于该计数请求，按根字段省略。其余未映射的根字段（包括 prompt、context_management 或未来输入字段）报带 JSON Pointer 的 LlmRequestException，不能静默丢掉可能影响输入的设置。现有请求编码的归属、字段冲突和工具规则继续生效；这些规则不解释服务端返回的附加信息。

响应 object 必须为 response.input_tokens，input_tokens 按 Long 读取并保留缺失／null、零和精确大整数；负数、非整数、溢出及非空 error 报 LlmProtocolException。TokenCount 使用 Server、RequestInput、Exact，native 保存完整响应；未报告的模型版本、缓存计数和分模态明细保持未知。额外响应字段缺失或改变形状不影响主线，不验证附加字段内容，不计算费用或缓存失效。实际生成的 usage 和此次调用 metadata 中的生成用量均不被改写。

普通验证使用官方形状的纯编码／解码测试和 loopback HTTP mock，覆盖多模态、工具历史、输入参数选择、精确计数、重试认证、真实收发时间与失败路径。没有可用服务或 key 时不增加真实请求，真实互通另行补测。依据：[输入计数 API](https://developers.openai.com/api/reference/python/resources/responses/subresources/input_tokens/methods/count)、[计数范围](https://developers.openai.com/api/docs/guides/token-counting)。

## 42. Responses 显式上下文压缩

`LlmClient.compactResponse(model, request, options = LlmCallOptions())` 返回 `ResponsesCompactionResult(compaction, metadata)`，调用部署路径下的 `POST /responses/compact`。应用显式决定调用时机及后续请求。`ResponsesDialect.supportsCompaction` 缺省 false，OpenAiResponsesDialect 开启；DeepSeek Responses 不继承此操作。`ModelCapabilities.serverCompaction: Capability<Unit>` 缺省 Unknown；明确 Unsupported 在认证前失败，Unknown 交给已选 dialect 和服务端。模型标识原样写入 body，不查询模型目录。

`ResponsesProtocol.compactUrl`、`encodeCompactionRequest`、`decodeCompaction` 提供相同的纯适配入口。请求只使用 LlmRequest 的 input、system 和 extensions；复用普通 Responses 的消息、多模态、工具结果及原生历史编码。tools 与 options 是下一次生成的声明，不发送至压缩端点，也不据其检查生成能力；仍检查实际提交的 system／input 的输入能力。请求扩展允许 previous_response_id、prompt_cache_key、prompt_cache_options、prompt_cache_retention、service_tier，原值发送；model、input、instructions 由公共字段管理，其他根扩展报路径错误，不能悄悄丢掉 conversation 等输入来源。压缩入口不推导缓存策略或寿命。

`ResponsesCompaction(dialect, id, output, createdAt = None, usage = LlmUsage(), native = None)` 是可序列化纯数据。响应 object 必须为 response.compaction，id 必须非空，output 必须为带非空 type 的对象数组；其中用户消息、compaction item 和未知 item 均完整保留，不解释或校验 opaque 内容。created_at 按服务端整数秒读取，缺失保持 None，零值保留。非空 error 与坏的核心字段报带原响应的 LlmProtocolException。usage 复用 dialect 的实际 token 统计并标为 Final，缺失计数保持未知；其余附加信息仅保存在 native，不验证内容或格式。

compaction 与 result 的 `toInput()` 返回包含完整 output 的同 dialect LlmHistory，用于替换调用方之前的 input。保存／恢复后仍可接入普通生成，保留顺序、角色、图片／文件和 opaque item；不会自动追加旧窗口、丢弃项或发起下一轮。instructions 由调用方在后续请求中明确提供。返回的 compaction id 仅标识这次操作，不把它写入后续 previous_response_id。

调用复用现有认证、服务拒绝重试、取消、截止、完成回调及借入 HTTP 的所有权。metadata 记录实际发送、接收和解析完成时间、服务端 request ID、compaction ID 与该操作报告的 usage；保留下来的旧消息不产生本次 firstEvent／firstOutput／firstText。最终输出吞吐仍按报告的 outputTokens 与实际本次延迟计算，缺失和零时长保持未知。ResponsesCompactionResult 仅接受 Completed metadata；其他既有显式操作的缺省 usage 不变。

普通 scoop.test 覆盖编码、上下文替换与 codec、附加信息兼容、实际 HTTP 方法与部署路径、重试认证、时间和 usage、失败清理与能力检查。只使用官方形状及 loopback mock，真实互通在可用凭据具备后补测。依据：[compact API](https://developers.openai.com/api/reference/python/resources/responses/methods/compact)、[上下文压缩](https://developers.openai.com/api/docs/guides/compaction)。

## 43. 显式 API 端点与认证字段回调

LlmEndpoint 只接收已有 API 的 baseUrl、headers 与认证，不包含部署类型、资源创建或按 model ID 选择云部署的规则。应用负责给出地址中的资源路径与 api-version 等 query；protocol 只追加该 API 的操作路径。Azure v1 与旧版 Chat 均可通过显式 URL 使用现有 provider，无需独立部署抽象。服务商过滤细节等附加字段继续保留原文，不解释其内容或有效性。

`LlmAuth.bearer(token: suspend () -> String)` 与 `LlmAuth.header(name, value: suspend () -> String)` 是已有静态工厂的回调重载。每次实际尝试在最终 URL／body 确定后调用一次，构造 auth 或 model 时不调用；每次重试重新取得值。空值和非法 HTTP 字段仍报错，认证字段标记 neverIndex；异常与取消沿现有调用寿命传播，未发送不产生 sentAt。库不缓存或自动刷新 token、不执行登录或读取环境；回调负责取得可用凭据，需要请求签名时继续实现公开 LlmAuth 接口。

普通 scoop.test 使用本地 mock 验证显式路径／query、原始模型名、工具历史与 SSE、动态认证及失败边界、实际收发时间。附加字段仅验证存在／缺失／形状变化不阻断核心结果；没有凭据时不请求真实服务。Azure URL 示例依据：[v1 API](https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle)、[旧版路径与认证](https://learn.microsoft.com/en-us/azure/foundry/openai/reference#rest-api-versioning)。

## 44. Interactions 状态快照、取消与删除

`LlmClient.retrieveInteraction(endpoint, interactionId, tools = [], options = LlmCallOptions())` 显式读取单个 JSON 状态快照；`cancelInteraction(endpoint, interactionId, tools = [], options = LlmCallOptions())` 请求取消已有后台 interaction。二者返回既有 LlmResult，由同一个 Interactions decoder 解释状态、输出、工具、usage 和原文。tools 仅用于本地关联，既不发送，也不从响应中的工具声明自动注册。模型 ID 不参与资源操作，不查询模型目录，不轮询或发起下一轮生成。

`InteractionsProtocol.retrieveUrl`、`cancelUrl`、`deleteUrl` 提供纯路径构造。interactionId 必须非空，按单个原始 path segment 编码；分别使用 GET `/interactions/{id}`、POST `/interactions/{id}/cancel`、DELETE `/interactions/{id}`，均发送空正文。路径前缀、认证 query 和其他参数保留；这三个资源操作清除 baseUrl 上的 stream、last_event_id、alt，快照读取采用协议缺省的非流式 JSON 表示。协议选择或空 ID 错误在认证和 I/O 前报告。GET 的 SSE 与 last_event_id 接续是独立的流操作，不混入本节的快照返回值。

`deleteInteraction(endpoint, interactionId, options = LlmCallOptions())` 返回 `InteractionsDeletionResult(native, metadata)`。该 API 成功时可以没有正文；native 保存实际收到的原始字节，空正文保留空数组，不强制 JSON 解码，也不合成 deleted=true、服务端 ID 或其他确认字段。返回类支持 companion codec。服务商附加信息只保留原值，不检查其内容或格式。

操作复用现有 HTTP 清理、认证、dialect 服务错误／重试资格、取消、总截止及完成回调。读取／取消报告的历史 usage 只在 LlmResponse 中，不成为本次 attempt 的生成用量、首输出时间或输出速率；attempt 仅记录实际 HTTP 收发和完成时间，并保留返回资源的真实 ID。删除没有返回资源 ID，attempt.responseId 保持 None。失败、取消与 client.close 仍等待清理后完成一次回调，借入的 HttpClient 保持原所有权。

普通 scoop.test 使用官方形状和 loopback HTTP mock 验证后台创建后的读取／取消／删除、可保存的工具历史、空正文和附加信息兼容、URL／method／认证字节、实际时间及失败清理。真实服务用例默认忽略，仅在凭据齐备时显式运行，实际覆盖见实施记录。依据：[Interactions v1 资源操作](https://ai.google.dev/api/interactions-api-v1#retrieving-an-interaction)、[官方 OpenAPI](https://ai.google.dev/static/api/interactions-v1.openapi.json)、[后台执行](https://ai.google.dev/gemini-api/docs/background-execution)。

## 45. Interactions 事件读取与显式接续

`LlmClient.openInteractionEvents(endpoint, interactionId, lastEventId = None, options = LlmCallOptions())` 返回 `InteractionsEventStream`；`withInteractionEvents` 在相同参数后接受 suspend block，按现有 useAsync 规则关闭。流提供 `next(): LlmNativeEvent?`、`lastEventId`、`metadata`、`isClosed` 和 `close()`，使用既有单消费者、按需读取和 client 所有权合同。每次 next 返回一个完整原始 JSON 事件，event 为其 event_type，value 保留 event_id 及所有其他字段。游标初始为调用参数，只在事件交付后按字符串 event_id 更新；缺失、null 或其他形状保持原文且不改变游标。关闭或失败后仍可读取最后已交付游标。

`InteractionsProtocol.retrieveStreamUrl(baseUrl, interactionId, lastEventId = None)` 在 §44 的资源路径上设置 `stream=true`、`alt=sse`，仅在 Some 时设置 last_event_id，保留其原始字符串并按 query 编码。GET 发送空正文，使用现有认证与明确服务拒绝的重试规则；协议和空 ID 错误在认证前报告。发生读错误后不自动重连，应用显式发起下一次调用，各次调用保留独立的实际收发时间。

`InteractionsProtocol.createEventDecoder()` 提供不依赖 I/O 的原生事件解码器，复用 LlmSseDecoder。它验证 JSON 事件的非空 event_type 和 SSE 名称一致性，保留完整原值，允许从任意步骤中间开始；interaction.completed 结束读取，提前 EOF／DONE 和 error 事件报错。它不重建缺失的前缀，不聚合 LlmResponse，response() 报 ResultNotRetained。完整记录仍可由应用交给已有 createStreamDecoder 聚合，前缀和后缀需来自同一次 interaction，是否保存由应用决定。

GET 的事件回放仅记录实际首事件和完成时间，不记录生成首输出／首文本、token 用量或速率；原始 usage 保留在事件里。attempt.responseId 只取服务端生命周期事件报告的 ID，不能由请求参数补造。服务端附加字段保持 as-is，不增加有效性或缓存失效判断。普通测试使用本地 mock 覆盖中断接续、序列化、单次关闭和核心错误边界；真实用例仍默认忽略。依据：[GET 参数](https://ai.google.dev/api/interactions-api-v1#retrieving-an-interaction)、[事件接续](https://ai.google.dev/gemini-api/docs/background-execution#streaming-pattern)。

## 46. 显式模型目录查询

`LlmClient.listModels(endpoint, query = UrlQuery(), options = LlmCallOptions())` 只读取一页，返回 `LlmModelListResult(page, metadata)`；`retrieveModel(endpoint, modelId, options = LlmCallOptions(), extensions = [])` 读取单个模型，返回 `LlmModelResult(model, metadata, requestedId)`。目录不参与 generate、模型绑定或 capability 检查，不自动翻页、选择模型、注册模型或缓存查询。代理没有目录入口时，显式查询照常报告服务错误，不影响生成。

`LlmApiProtocol.modelCatalog(): LlmModelCatalog?` 缺省 None，具体 protocol 委托 dialect 提供已支持的操作。LlmModelCatalog 提供纯 listUrl／decodeList；具有单模型 API 的目录同时实现 LlmModelLookup 的 retrieveUrl／decodeModel。此步接入 OpenAI Chat／Responses、DeepSeek Chat／Responses、Anthropic Messages、Gemini generateContent／Interactions。OpenAI、Anthropic 和 Gemini 支持单模型读取；DeepSeek 仅按已公布的模型列表 API 接入，不用下载整个目录模拟单模型读取。不支持的操作在认证和 I/O 前报 /provider 错误，并执行既有完成通知。

`LlmModelInfo(dialect, id: String?, native)` 保存服务端提供的模型标识与完整模型 JSON；`LlmModelPage(dialect, models, native)` 保存有序列表和完整分页响应。四个数据类型均有 companion codec。OpenAI／DeepSeek／Anthropic 从 data 数组的必需 id 读取；Gemini 从 models 数组的必需 name 读取并去除协议定义的 models/ 前缀，native 保留原始资源名。Gemini 的缺省或 null models 表示空页。协议允许省略标识时 id 为 None，不从请求补造服务端身份；单模型结果的 requestedId 单独保留调用方提交的非空标识。只验证协议规定的模型标识及列表结构；名称、能力详情、上下文长度、分页字段及服务商附加信息保留原值，既不据此推断已选生成 API 的 capability，也不验证附加内容的准确性。

上述服务的查询与单模型读取使用 GET、空正文和当前 endpoint 的认证及版本 header，追加 /models 或 /models/{id}；其他服务的路径与方法见第 47 节。模型 ID 必须非空并作为一个原始 path segment 编码；Gemini retrieve 也接受 models/ 前缀。保留 baseUrl 路径和 query，显式 query 按同名字段覆盖 baseUrl 的值，同时保留本次 query 的重复字段、空值和无值。没有 query 参数时不额外产生问号。不同分页字段不统一成猜测的 next cursor；调用方从 page.native 读取服务端原值并显式传入下一次 query。

`geminiModelsQuery(pageSize: ULong? = None, pageToken: String? = None)` 与 `anthropicModelsQuery(limit: ULong? = None, afterId: String? = None, beforeId: String? = None)` 仅构造既有 UrlQuery；缺省不发送参数，其他过滤参数可继续通过 UrlQuery 添加。数量按原无符号值发送，由 API 处理其自身上限，不建立本地配额体系。操作复用既有 HTTP、重试、超时、取消、借入所有权和完成回调；metadata 只记录实际操作计时，不合成生成 token 用量或 response ID。

普通 scoop.test 使用官方响应形状与本地 mock 验证缺省入口、显式分页、原文 codec、URL／认证／实际时间、错误清理，以及目录失败不影响生成；真实查询默认忽略并要求显式配置。依据：[OpenAI list](https://developers.openai.com/api/reference/resources/models/methods/list)、[OpenAI retrieve](https://developers.openai.com/api/reference/resources/models/methods/retrieve)、[Anthropic Models](https://platform.claude.com/docs/en/api/models/list)、[Gemini Models](https://ai.google.dev/api/models)、[DeepSeek Models](https://api-docs.deepseek.com/api/list-models/)。

## 47. 其他已接入服务的模型查询

OpenRouter、Cohere、Ollama 和 Bedrock 的模型列表／详情使用相同 client 方法，具体 protocol 继续委托 dialect。LlmModelLookup 增加纯 retrieveRequest(baseUrl, modelId, extensions = [])，返回 LlmModelLookupRequest 的 Get(url) 或 Post(url, body)；缺省实现使用 retrieveUrl 并拒绝无映射的非空 extensions。Ollama 使用 Post，其他目录使用 Get。网络层按所选请求编码实际方法与正文，沿用认证、重试、计时和关闭，不执行模型加载、拉取、创建或删除。

OpenRouter 列表使用 /models、data[].id；单模型使用 /model/{author}/{slug}、data.id。仅此 API 将 modelId 按唯一的 / 分隔成非空 author 与 slug，分别做 path segment 编码，保留版本后缀；不把这一规则应用到其他服务。单模型 native 保留包括 data 的完整响应。

Cohere 使用 /v1/models 与 /v1/models/{model}，条目位于 models 数组，模型标识来自可选的 name。name 缺省或 null 时 id=None；提供时必须是非空字符串。cohereModelsQuery(pageSize: ULong? = None, pageToken: String? = None, endpoint: String? = None, defaultOnly: Boolean? = None) 只构造原生 query，缺省省略，服务端负责参数组合及限值；其他参数仍可通过 UrlQuery 传入。

Ollama 列表使用 GET /tags 和 models[].model；详情使用 POST /show，正文仅默认携带 model，响应不提供标识，因此 model.id=None，requestedId 保持原请求值。ollamaModelOptions(verbose: Boolean? = None) 产生既有 DialectExtension，显式 true／false 均保留；其他原生选项可使用相同扩展，禁止覆盖 model。缺少附加字段、字段类型变化或新增字段不影响主线，详情响应仅要求 JSON object 且不能是 error 响应。

Bedrock 查询使用 /foundation-models、modelSummaries[].modelId，以及 /foundation-models/{modelIdentifier}、modelDetails.modelId。modelIdentifier 可为实际 ID 或 ARN，整体作为单个 path segment 编码。调用方显式提供模型查询服务的 URL 和认证；不从 runtime 域名、地区或模型名称推导另一地址。所有查询过滤字段仍通过 UrlQuery 传入；不包含云资源创建、部署或账号管理。单模型 native 保留包括 modelDetails 的完整响应。

依据：[OpenRouter 列表](https://openrouter.ai/docs/api/api-reference/models/list-all-models-and-their-properties)、[单模型](https://openrouter.ai/docs/api/api-reference/models/get-a-model-by-its-slug)、[Cohere 列表](https://docs.cohere.com/v2/reference/list-models)、[单模型](https://docs.cohere.com/v1/reference/get-model)、[Ollama 列表](https://docs.ollama.com/api/tags)、[详情](https://docs.ollama.com/api-reference/show-model-details)、[Bedrock 列表](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListFoundationModels.html)、[单模型](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_GetFoundationModel.html)。

## 48. OpenAI 与 Anthropic 文件 API

`LlmClient.uploadFile(endpoint, upload, options = LlmCallOptions())`、`listFiles(endpoint, query = UrlQuery(), options = LlmCallOptions())`、`retrieveFile(endpoint, fileId, options = LlmCallOptions())`、`downloadFile(endpoint, fileId, options = LlmCallOptions())` 和 `deleteFile(endpoint, fileId, options = LlmCallOptions())` 显式执行一次文件操作。它们服务于生成输入和生成文件的读取，不隐式上传媒体、轮询处理状态、自动翻页或清理服务端文件。

`LlmFileUpload(filename, bytes, mimeType = "application/octet-stream", extensions = [])` 是可保存的内存上传数据，不包含本地路径或凭据。文件名和 MIME 由应用提供，不探测文件内容。`LlmApiProtocol.fileApi(): LlmFileApi?` 缺省 None；Chat、Responses、Messages protocol 委托具体 dialect，当前 OpenAI Chat／Responses 与 Anthropic Messages 提供文件操作。`LlmFileApi` 提供纯 URL 构造、上传编码及文件／列表解码；上传编码返回 `LlmFileUploadBody(bytes, contentType)`。未接入的 dialect 在认证前明确报错，不因兼容同一种生成协议而推定支持文件 API。

三个 provider 使用相同路由：上传 POST /files、列表 GET /files、元数据 GET /files/{id}、删除 DELETE /files/{id}、内容 GET /files/{id}/content。ID 必须非空，作为单个原始 path segment 编码，保留 baseUrl 前缀和 query；列表 query 按 §46 的规则合并。Anthropic 继续使用 endpoint 的版本头，正式 Files API 不自动添加旧 beta 头。`openAiFilesQuery(limit: ULong? = None, after: String? = None, order: String? = None, purpose: String? = None)` 与 `anthropicFilesQuery(limit: ULong? = None, page: String? = None)` 仅构造原生 UrlQuery；其他过滤条件直接通过 UrlQuery 传入。

上传复用 scoop.http 的 MultipartEncoder，file part 携带实际 filename、Content-Type 和原始 bytes。OpenAI 缺省增加 purpose=user_data；调用方可通过所属 dialect 的 extension 明确指定 purpose。upload.extensions 的对象成员是原始 multipart 字段名与字符串值，既不猜测嵌套结构，也不把 JSON 当成表单值。例如 OpenAI 的 `expires_after[anchor]`／`expires_after[seconds]` 和 Anthropic 的 `expires_in_seconds` 由应用显式提供。不能覆盖 file part，不能跨 dialect 或重复提供扩展。仅检查表单可编码性，不解释服务端附加信息或计算过期时间。

上传只编码一次完整 multipart 正文；既有 HTTP 调用增加可选 contentType 参数，缺省仍为 application/json。认证回调看到最终 multipart Content-Type（含 boundary）及与实际发送完全相同的 bytes；明确服务拒绝后的重试复用相同 bytes／boundary 并重新认证。文件上传不允许 endpoint 的 Content-Type 覆盖编码器生成的值。读取和删除使用空正文；下载与删除响应按 maxResponseBytes 有界读取为原始 bytes，不要求 JSON 或 UTF-8。所有操作沿用现有超时、重试、取消、client／借入 HTTP 所有权和完成回调。

`LlmFileInfo(dialect, id, reference: LlmFileReference?, native)` 保存服务端实际 id、可直接交给同 dialect MediaSource.FileReference 的引用及完整 JSON；OpenAI／Anthropic 的 ID 同时提供文件引用，reference 恒为 Some。没有可用媒体 URI 的其他协议可以返回 None，不能由资源名称伪造 URI。`LlmFilePage(dialect, files, native)` 保存有序 data 列表和完整分页响应。上传／读取返回 `LlmFileResult(file, metadata)`，列表返回 `LlmFileListResult(page, metadata)`。`LlmFileContentResult(dialect, requestedId, bytes, metadata)` 与 `LlmFileDeletionResult(dialect, requestedId, native, metadata)` 分别保存下载正文和删除原始正文。上述持久数据均提供 companion codec；请求 ID 不冒充服务端文件 ID。

只验证必需 id、data 数组和 JSON 对象结构，HTTP 错误沿既有服务拒绝路径报告；filename、MIME、大小、状态、时间、downloadable、分页及供应商附加字段均保持原文，不因缺失或格式变化阻断文件引用与主线操作。下载能否成功由服务端决定，不以 cached metadata 推断。metadata 记录本次实际收发及完成时间，不把 file id 记为生成 responseId，也不合成 token usage。普通 scoop.test 用本地 mock 验证原始 multipart／认证、重试、保存后文件引用、分页、二进制内容、原始删除响应及取消清理；真实用例遵守默认忽略与环境配置要求。

依据：[OpenAI Files](https://developers.openai.com/api/reference/resources/files/methods/create)、[OpenAI 分页](https://developers.openai.com/api/reference/resources/files/methods/list)、[Anthropic Files](https://platform.claude.com/docs/en/build-with-claude/files)、[Anthropic 分页](https://platform.claude.com/docs/en/api/http/files/list)、[Anthropic 下载](https://platform.claude.com/docs/en/api/files/download)。

## 49. Gemini 文件 API

Gemini generateContent 与 Interactions 的 fileApi 接入 §48 的 uploadFile、listFiles、retrieveFile、deleteFile。官方 discovery 在 v1beta 与 v1 均公布这些操作，沿用对应 endpoint 的 API 版本；不改域名或推导云资源地址。GET／DELETE 使用 /files/{id}，id 接受完整 files/{id} 资源名或原始 ID；响应 LlmFileInfo.id 保存服务端实际 name（含 files/）。列表使用 files 数组，缺省、null 或空数组均为空页；geminiFilesQuery(pageSize: ULong? = None, pageToken: String? = None) 仅构造原生 query。

现有 LlmFileUpload 接收完整内存 bytes，因此使用 discovery 公布的单次 multipart 上传：在 baseUrl 的最后一个非空 path segment（API 版本）前插入 upload，追加 files 并设置 uploadType=multipart。保留此前路径前缀、authority 和其他 query；baseUrl 没有版本 segment 时在认证前报错。正文为 multipart/related，先发送 application/json 的 {file: {displayName: filename, ...extensions}}，再发送带实际 MIME 的原始文件 bytes；复用已有 MultipartEncoder。extensions 属于选定 dialect，作用于 file metadata 对象，可以携带 API 原生字段，不能覆盖 displayName；不将响应附加信息转换为本地策略。

LlmFileApi 增加 decodeUpload(value)，缺省调用 decodeFile；Gemini 上传响应从 file 字段读取资源，file.native 保留完整上传响应包装。读取／列表的单个文件保留原始 File 对象。只验证核心 name、列表和可选 uri 的协议类型；uri 缺省、null 或空字符串时 reference=None，不根据 state、error、expirationTime、mimeType 或 downloadUri 判断可用性、等待处理或计算有效期。其他字段无论缺少、增加或形状变化都保持原文。已有 URI 时，generateContent 引用为 {fileUri: uri}，Interactions 引用为 {uri: uri}，不根据 metadata 猜媒体种类。

Interactions 的 MediaSource.FileReference 也接受 URI 引用对象；发送时从实际 ContentPart 和显式 MIME 补齐缺省 type／mime_type。原始引用若已声明这两个字段，继续检查其与请求一致；其他原生内容原样保留，拒绝同时包含 data 的引用。完整输出历史的已有 typed media 引用仍按原文使用。此规则让同一文件 URI 能由调用方明确作为图片、文档、音频或视频使用，不自动推断模型 capability。

LlmClient.registerFiles(endpoint, uris: Array<String>, options = LlmCallOptions()) 显式登记已有 GCS URI，返回 LlmFileListResult。支持这一实际操作的 fileApi 实现 LlmFileRegistration 的 registerUrl／encodeRegistration，使用 POST /files:register 和 {uris: [...]}，复用 decodeList；不支持时在认证前报错。URI 字符串按调用方原值提交，服务端决定授权及可接受的 URI；不创建、复制或管理 GCS 对象。

Gemini Files 没有公布按 file ID 下载已上传文件的操作，downloadFile 在认证前明确报错；服务端实际提供的 downloadUri 保持 native 原值，不从 name 拼造下载 URL，也不自动访问它。所有已接入操作复用现有认证、重试、时间戳、关闭和完成通知。普通测试使用本地 mock；真实验证默认忽略、要求 Google key，并只上传少量合成文本、读取和删除，不触发生成或自动处理状态轮询。

依据：[v1 discovery](https://generativelanguage.googleapis.com/$discovery/rest?version=v1)、[v1beta discovery](https://generativelanguage.googleapis.com/$discovery/rest?version=v1beta)、[Files API](https://ai.google.dev/api/files)、[文件输入](https://ai.google.dev/gemini-api/docs/files)、[Google multipart 上传格式](https://developers.google.com/android-publisher/upload#multipart-upload)。

## 50. API protocol 基础合同与 Gemini Live 线协议

`LlmApiProtocol` 只定义 dialectId、requestFields、decodeError 及可选 fileApi／modelCatalog／tokenCounter；LlmProvider.protocol 使用该基础合同。LlmGenerationProtocol 继承它并保留普通生成专用方法，requestFields 缺省委托已有 generationFields。Endpoint 的固定头改用 requestFields。普通 generate 在进入编码／认证前要求 LlmGenerationProtocol，openStream 要求已有 LlmStreamProtocol；错误仍通过同一完成通知返回。会话协议无需实现假的 generationUrl 或 decodeResponse。调用纯生成适配器时使用具体 protocol，或先以 is 检查 LlmGenerationProtocol。

`Providers.geminiLive()` 包装 `BidiGenerateContentProtocol(GeminiLiveDialect())`，protocol ID 为 bidi-generate-content，dialect ID 为 gemini-live。缺省地址是完整 RPC 地址 https://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent；webSocketUrl 只将 http／https 转为 ws／wss，保留调用方提供的完整路径和 query。其他 API 版本／受限连接地址由应用显式提供。模型名仅按该 API 要求在缺少 models/ 时加此前缀，不根据名称选择版本或能力。凭据仍由 LlmAuth 提供。

本节定义纯协议入口：encodeSetup、encodeClientContent、encodeRealtimeInput、encodeToolResponse 与 decodeMessage；不执行网络 I/O，不产生本地收发时间或虚构会话完成状态。持久连接、setup 确认与消息计时使用 §51 的 client 会话合同。

`GeminiLiveSetup(system = [], tools = [], options = LlmGenerationOptions(), extensions = [])` 可保存，缺省 setup 只含 model。配置复用现有 Gemini 的 Content、Tool、生成参数及按位置扩展编码；system 仅文本，工具使用已有 ToolDefinition。Live 没有停止序列、结构化输出或根 toolConfig 的映射，因此显式 stop、非 Text 输出与非 Auto toolChoice 报请求错误。responseModalities、speechConfig、realtimeInputConfig、sessionResumption 等 API 原生设置通过所属 dialect 的 extensions 明确提供，不按模型名称推断。扩展仍不能覆盖已管理字段；configuration 只用于 setup，不成为后续内容的隐式缺省。

encodeClientContent(input, turnComplete = true) 编码 clientContent.turns 与明确的 turnComplete；input 使用已有消息及同 dialect 历史，复用 Gemini Content 编码，空 turns 允许显式结束输入。ToolResult 必须走 toolResponse，不能混入 clientContent。encodeRealtimeInput 接收可保存的 GeminiLiveInput：可选 text／audio／video、activityStart／activityEnd 标记、可选 audioStreamEnd 和 extensions。GeminiLiveBlob(bytes, mimeType) 保存原始媒体；audio 要求 audio MIME，video 是 image MIME 的单帧图像。发送按原 bytes 做 Base64，不采集设备、重采样、转码或安排帧率。未提供的字段省略，显式空文本／空 bytes 和 false 保留，不使用已弃用的 mediaChunks。

`GeminiLiveToolResult(call, response, parts = [], extensions = [])` 复用 ClientToolCallReference，response 是调用方提供的 JSON 对象（包括原生 scheduling 等字段），parts 是明确的内联媒体。encodeToolResponse(results, tools = []) 产生 functionResponses；必须有实际非空 call ID，name 取原始 context 或明确匹配的工具声明，id／name／response／parts 不能由扩展覆盖。调用方决定执行、取消与回传，不自动运行工具或调度下一轮。

`GeminiLiveMessage(dialect, event, native, usage = LlmUsage(), voiceActivity = None)` 与所有事件／内容类型提供 companion codec，完整 native 保留原始包装。GeminiLiveEvent 区分 SetupComplete、ServerContent、ToolCall、ToolCallCancellation、GoAway、SessionResumptionUpdate 与 Native；已知互斥消息不能同时出现，usage-only 或未知消息保留 Native。voiceActivity 可独立出现或与其他消息同现，按原值保留。

GeminiLiveContent 保存同一消息内的全部有序文本、reasoning、媒体、服务端工具内容及明确的 generationComplete／turnComplete／interrupted／waitingForInput／interactionStatus。转写分别保留 input、interim input、output 的可选 text 和原文，不把转写合并到模型文本。generationComplete、turnComplete 与 IDLE 各自保留；不推断播放完成，也不以 turnComplete 自动认定整个 session 空闲。搜索来源与活动复用既有输出表示，跨消息的 grounding 位置留在 native，不伪造当前分块内的引用范围。

独立 toolCall 消息按实际 id／name／args 生成已有客户端调用或原生未绑定内容；只有完整、已绑定且具有实际非空 ID 的调用进入可执行表示，缺少 ID 的调用和未知函数保留 Native，不自动注册。取消消息只返回实际 ids。goAway 与 resumption payload 原样交付，不自动恢复连接、保存会话或解释其有效期。每条 usageMetadata 独立保存为 Partial LlmUsage：inputTokens 仅在 promptTokenCount 与 toolUsePromptTokenCount 都已报告时合并，outputTokens 仅在 responseTokenCount 与 thoughtsTokenCount 都已报告时合并；缺失分量不假定为零。totalTokens 只取实际 totalTokenCount，不用它反推缺失分量或额外增加 reasoning／tool-use，也不在消息之间自动累加。cache 与 reasoning 分量、已知模态明细沿用现有 TokenUsage 表示，原始计数和其他信息均在 native。

普通测试使用文档形状与本地 mock，覆盖配置／媒体／工具往返、所有消息分支、核心错误、原始附加信息兼容，以及会话 provider 不能误走普通生成。依据：[Live API](https://ai.google.dev/api/live)、[输入输出与完成语义](https://ai.google.dev/gemini-api/docs/live-api/capabilities)、[工具调用与搜索](https://ai.google.dev/gemini-api/docs/live-api/tools)。

## 51. Gemini Live 持久连接与消息计时

LlmClient.openGeminiLiveSession(model, setup = GeminiLiveSetup(), openingOptions = LlmCallOptions(), webSocketOptions = HttpWebSocketOptions()) 返回 GeminiLiveSession；withGeminiLiveSession 在同一参数后接受 suspend block，组合原入口与 useAsync。协议、模型与 setup 固定于本连接，不提供中途改模型或重发 setup 的入口。缺省 setup 只含明确的 model ID。打开先检查 BidiGenerateContentProtocol、realtime capability、setup 的通用参数／system／工具能力，并编码完整配置；这些错误在认证与 I/O 前返回。

成功升级后先发送 setup，读取首个服务端消息并要求 SetupComplete，才交付会话；缺失确认、提前正常关闭、非法 JSON 或错误核心字段均为打开失败，原消息或原文本仍在异常中。客户端发送文本 JSON；Live 服务端的文本帧或二进制帧均按 UTF-8 JSON 解码，非法 UTF-8 作为协议错误。setupComplete 同条消息里的用量、voiceActivity 和其他字段完整保留。初始化不消耗确认之后的消息，不执行模型生成或工具。HTTP 的明确拒绝沿既有 dialect 规则重试，升级后 setup 失败不重连、不重放。

openingOptions 的 total 覆盖认证、HTTP 升级、setup 发送和确认等待，缺省 None；HTTP 等待与错误正文设置仍只作用于升级。onFinished 在本次打开结束时调用一次，失败先关闭已取得的 socket。session.opening 保存该次 LlmCallMetadata：attempt 只记录真实 HTTP 升级，不将 setup 的文本、用量或会话数据放进 HTTP attempt。成功后的消息大小、压缩和关闭设置由 webSocketOptions 管理，不把 opening 的总截止延续到整个会话。

会话公开 model、protocol、opening、setupSent、setupComplete、id、startedAt、url、isClosed 与 failure，并提供 sendClientContent(input, turnComplete = true)、sendRealtimeInput(input)、sendToolResponse(results)、receive() 和 close()。工具关联只使用 setup.tools；后续输入和工具结果媒体继续检查已声明 capability。realtimeInput.video 是单帧图像，使用 videoFrames capability；字节与单条消息数量检查复用既有媒体规则，不累计整段会话的输入规模。原生扩展不生成额外 capability 推断。

GeminiLiveSent(sessionId, command, timing) 和 GeminiLiveReceived(sessionId, message, received) 提供 companion codec；setupSent 与 setupComplete 也是这两类记录。发送点在 JSON 编码及取得 writer 后、首段提交前采样，完成点在 writer.finish 成功后采样；接收点在完整文本取出后、JSON 解码前采样。UTC 与单调 elapsed 使用同一 session 起点，HTTP opening 的时间轴与消息记录分开。时间不代表服务端接受、播放完成或缓存状态。

receive 按需交付一个完整消息，正常连接结束返回 None；turnComplete、generationComplete、interrupted、GoAway 或恢复通知不结束读取，也不自动改变应用工作流。每条 usage 保持 §50 的独立报告，不累加为整个会话用量。应用负责消费、工具执行、取消调用与决定是否新建连接。

Responses 与 Gemini Live 共用内部 WebSocket 所有权、升级认证／拒绝重试、收发采样、单读／单写和关闭实现。允许一读一写并行，同方向重入在改变连接前失败；请求编码和 capability 错误不关闭可用会话。实际消息读写、协议错误或调用方取消则结束会话并等待清理，已交付的失败不在 close 重复抛出。会话随所属 client 关闭，借入 HttpClient 的所有权不变。打开调用者取消只约束交付前，交付后由会话／client 的显式寿命控制；不设后台输出消费、自动重连或媒体设备操作。

普通 scoop.test 用本地 HTTP 升级／WebSocket mock 验证 setup 确认、媒体／转写、工具往返、时间与保存、错误及关闭，并回归共享 Responses 会话。真实 Live 用例 @Ignore，仅在显式选择且 key、model 已配置时运行少量输入。依据：[初始化与消息合同](https://ai.google.dev/api/live#BidiGenerateContentSetup)、[官方 Python 初始化顺序](https://github.com/googleapis/python-genai/blob/main/google/genai/live.py)、[官方 JavaScript 消息帧解码](https://github.com/googleapis/js-genai/blob/main/src/live.ts)、[会话通知](https://ai.google.dev/gemini-api/docs/live-api/session-management)。

## 52. OpenAI Realtime 请求线协议

Providers.openAiRealtime() 包装 RealtimeProtocol(OpenAiRealtimeDialect())，protocol ID 为 realtime，dialect ID 为 openai-realtime，缺省 baseUrl 为 https://api.openai.com/v1。webSocketUrl(baseUrl, modelId) 追加 realtime 路径，将 http／https 转为 ws／wss，以明确 modelId 设置唯一 model query，保留其他 query 与路径前缀。普通 generate／openStream 不接受该会话协议；不按模型名称切换 GPT-Live，也不自动增加旧版 beta header。

本节交付纯请求编码与可保存的数据，不执行网络 I/O。RealtimeDialect 定义 session 配置、response 请求、输入编码与 HTTP 错误解释；RealtimeProtocol 定义各条客户端命令的包装与 ID 字段。事件解码见 §53；会话连接及实际消息时间由后续会话合同接入，不用普通单次生成方法占位。

完整原生命令使用独立的 RealtimeNativeCommand(dialect, value) 与 encodeNativeCommand，提供 companion codec。它只检查归属及非空 type 对象后原样返回，用于直接表达 MCP 指定子工具等没有通用选项的原生设置，不与通用命令做隐式覆盖，也不声称已检查其所有服务端语义。

RealtimeConfiguration 的 instructions、tools、toolChoice、outputModality、maxOutputTokens、parallelToolCalls、reasoningEffort 均缺省 None，extensions 缺省空数组。None 表示本次没有设置，显式空 instructions／tools 与 false 原样编码。RealtimeOutputModality 是 Text 或 Audio；RealtimeTokenLimit 是 Limited(ULong) 或 Unlimited，后者编码原生 inf。所有数据提供 companion codec。音频格式／voice／VAD／转写、prompt、truncation 等原生配置由对应位置的 dialect extensions 传入，保留显式 null；不填造服务端缺省，不管理采集、播放或音频转码。

encodeSessionUpdate(configuration = RealtimeConfiguration(), eventId = None, tools = []) 输出 session.update，session.type 固定为 realtime；model 由连接 URL 指定，不能通过扩展更换。RealtimeResponseRequest(configuration = RealtimeConfiguration(), input = None, conversation = None) 用于 encodeResponseCreate；最小命令只有 response.create，input=None 使用服务器会话上下文，Some([]) 明确指定空输入，conversation 原样传入。配置中的扩展作用于 session 或 response 内，不隐式继承到其他位置；type、model、input、conversation 与所有已管理配置字段即使未输出也不能被覆盖。

工具复用 ToolDefinition 与 ToolChoice。客户端函数编码扁平 function 定义；strict、custom 和客户端原生工具没有此协议映射，显式请求报错。服务端工具支持同 dialect 的 Native(mcp) 声明，原生配置保留，具体服务调用由服务器执行；WebSearch 不能擅自替换为 MCP。tools=None 不改写已配置工具，Some([]) 明确清空；Specific 按配置内 tools 或调用方显式传入的 tools 上下文关联函数名或 MCP server_label，不将 ToolKey 当作 wire name。工具 key 与函数名不能重复；未找到明确关联时报路径错误。Required 可以作用于服务器已存在的工具，不要求每条命令重新声明。

encodeItemCreate(item, previousItemId = None, eventId = None) 只产生一条 conversation.item.create，input item 使用既有 LlmInputItem；多条 History 不能塞入一次 item.create。encodeResponseCreate 的可选 input 按顺序展开同 dialect History。新消息的 User／System 文本编码 input_text，Assistant 文本编码 output_text；Developer 没有对应角色而报错。图片只接受 PNG／JPEG 的 Bytes，编码 data URI；音频 Bytes 只接受协议的 raw PCM／PCMU／PCMA 媒体类型，按原字节 Base64 编码，不擅自修改连接音频格式。调用方须让音频字节与 session 的格式配置一致。媒体 URL、文件引用、文档、视频和带时间戳帧没有通用映射，明确拒绝；原生 content 和 history 保留同 dialect 的完整对象。conversation.item.create 按官方限制拒绝 assistant 音频，不能删除音频后伪装成成功回传。

ToolResult 编码 function_call_output，使用实际 call ID 和文本结果；已提供 context 时要求它是同 ID 的 function_call。结果没有独立 isError 标记，错误内容由应用显式放入文本，不能静默丢弃 isError=true。MCP approval 等原生 item 可通过同 dialect 的 History 显式提供，不自动批准或执行。

encodeAudioAppend(bytes, eventId = None)、encodeAudioCommit(eventId = None)、encodeAudioClear(eventId = None) 分别对应 input_audio_buffer 命令；提交音频不附加 response.create。encodeItemRetrieve／encodeItemDelete 使用实际 itemId；encodeItemTruncate(itemId, audioEndMilliseconds: ULong, contentIndex: ULong = 0UL, eventId = None) 直接编码应用选择的截断位置，不推算播放进度。encodeResponseCancel(responseId = None, eventId = None) 保留对默认响应或明确响应的选择。可选 eventId 原样传递，不生成确认关联或重放机制。output_audio_buffer.clear 是 WebRTC／SIP 操作，不作为该 WebSocket 协议的命令。

普通测试使用官方形状，覆盖最小命令、缺省与显式清空、工具混用、媒体／历史／工具结果、URL 与精确无符号数、扩展归属和错误路径；不连接官方 OpenAI，也不测试附加字段的内容准确性。依据：[客户端事件](https://developers.openai.com/api/reference/resources/realtime/client-events)、[WebSocket 连接](https://developers.openai.com/api/docs/guides/voice-websockets)、[会话与函数调用](https://developers.openai.com/api/docs/guides/realtime-conversations)。

## 53. OpenAI Realtime 服务端事件

RealtimeProtocol.decodeMessage(value, tools = []) 只解码一条完整 JSON 对象，返回可保存的 RealtimeMessage(dialect, type, eventId, event, native)。native 保存整条消息；未知事件、未知输出和服务商附加字段原样保留，不验证计费、速率、缓存或其他附加信息的内部形状。已解释的协议字段错误报告原始 JSON 路径与整条消息。RealtimeDialect.readUsage 提供该协议的 token 口径扩展。

RealtimeEvent 区分 session.created／updated、response.created／done、输出 item／content part、文本与音频转写的 delta／done、音频 delta／done、函数与 MCP 参数 delta／done、MCP 执行状态、输入音频转写和 error。会话 item 的 added／done／retrieved 保留原始 item，原生通知通过 Native 交付。实际 responseId、itemId、outputIndex 与 contentIndex 分别使用 RealtimeItemPosition 和 RealtimeContentPosition 表达，索引为 ULong，不合并不同 response 的输出。音频 delta 只解码原始 Base64 字节，不猜测媒体格式；格式由连接／响应的实际配置决定。done 只表示该字段流结束，不能推断播放完成或连接结束。

response.created 与 response.done 提供 LlmResponse 读取视图，保留 response 原文与各 output 顺序。只有 response.done 且 status=completed 的函数输出，才可能取得执行资格；还须关联调用方明确提供的函数声明、实际 call_id、完整参数对象与未标为不完整的 item。参数 done、output_item.done 与 conversation.item.done 均不独立产生可执行调用。未知函数或没有 call ID 的输出保持 Native；已关联但截断或无效的参数保留为 Incomplete／Invalid。服务器 MCP 调用只作为 ServerToolActivity，不进入客户端工具列表；审批、工具目录与未知服务端输出保持原文，不自动批准或执行。

response.done 的 status=failed 映射 Failed，cancelled／incomplete／in_progress／未知或缺失状态映射 Incomplete；max_output_tokens 与 content_filter 映射已知结束原因，其他原因保持 Other 或 Unknown。已结束响应的 usage 标为 Final，即使没有用量也不填零；创建事件的 usage 为 Partial。响应文本仅来自 assistant 的 output_text；音频 transcript 保留在原生 content 与独立转写事件中，不冒充文本模态输出。response.done 不带原始音频，不重建音频；含 assistant output_audio 的响应没有可重放 History，其余完整原始 output 可作为同 dialect 历史读取。

usage 的 input_tokens／output_tokens／total_tokens、input_token_details.cached_tokens 与文本／图片／音频模态细项按服务明确报告解释；cached 是 input 子集，普通输入可由两者相减。没有提供的计数保持 None，不从分项猜总数，不累计多个响应。缓存模态细项保持原文。输入转写的 usage 独立于 response；type=tokens 时读取 token，duration 或未知类型仅保存原文，不计算计费或将时长转换为 token。

error 事件保留原始错误对象并正常交付，不能仅因服务端 error 消息就关闭连接或自动重试。本节不聚合响应、不维护播放／缓存／会话恢复状态，网络寿命与收发时间另由会话入口负责。普通 scoop.test 覆盖交错响应位置、文本与音频、取消／截断调用、MCP 与函数隔离、精确用量、保存、未知附加信息容忍及协议错误路径。依据：[服务端事件](https://developers.openai.com/api/reference/resources/realtime/server-events)、[会话与函数调用](https://developers.openai.com/api/docs/guides/realtime-conversations)。
