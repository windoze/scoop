# scoop.llm.client

LLM client 正在按[架构设计](../../../docs/llm-client/DESIGN.md)分项实现，进度见[实施记录](../../../docs/llm-client/PROGRESS.md)。目前落地的是公共 token 用量与 capability 数据；生成、provider 和流式入口仍在实施。

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
