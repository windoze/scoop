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
