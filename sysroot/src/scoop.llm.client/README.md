# scoop.llm.client

LLM client 正在按[架构设计](../../../docs/llm-client/DESIGN.md)分项实现，进度见[实施记录](../../../docs/llm-client/PROGRESS.md)。目前落地的是公共请求、token 用量与 capability 数据；生成、provider 和流式入口仍在实施。

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

Message 默认 User，system／tools／extensions 默认空，生成参数默认省略。工具区分 ClientFunction、ClientCustom、ClientNative 与 Server；供应商规定格式的工具也可能需要应用执行。请求级、消息级、内容块级与工具级的扩展各自带 DialectId，作用于当前位置。保存格式保留原生 JSON 和媒体来源；实际协议编码、能力检查与服务端工具执行观察随后续 adapter 交付。

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

MIME 参数也参与格式匹配；支持音频 `rate=24000` 不自动表示支持其他采样率。缺失上限使用 None，已知零上限仍保留。当前 capability API 负责描述、查询和保存；与实际请求、模型绑定及 dialect 规则的联动随生成入口实现。

以上是函数内的调用片段；应用直接使用 JSON 时也应声明 `scoop:scoop.json` 依赖。Codec 使用现有 companion 的缺省实现，Long 大整数在保存与恢复时保持精确。正式合同见 [LLM client 规范](../../../docs/specs/SCOOP-LLM-CLIENT-SPEC.md)。
