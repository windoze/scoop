# LLM API 覆盖与验收

本页对照当前公开入口、所属 Cone 的测试和[逐项实施记录](PROGRESS.md)，说明本次设计的交付范围。API 合同见[库规范](../specs/SCOOP-LLM-CLIENT-SPEC.md)，调用方式见[使用示例](USAGE.md)。

## 协议与方言

Provider 固定一种 protocol／dialect；模型 ID、URL、认证和 capability 由调用方明确绑定。同一供应商的不同 API 使用不同 provider，不按模型名称切换协议。下表的功能描述的是已实现的适配入口，不宣称任意模型或账号都支持相同能力。

| Provider | Protocol 与生成传输 | 已接入辅助操作 | 主要测试目录 |
| --- | --- | --- | --- |
| openai-chat | Chat Completions：HTTP／SSE | 模型列表／详情，文件操作 | [chat](../../sysroot/src/scoop.llm.client/tests/chat/) |
| deepseek-chat | Chat Completions：HTTP／SSE；独立 reasoning 回传规则 | 模型列表 | [chat](../../sysroot/src/scoop.llm.client/tests/chat/) |
| openrouter-chat | Chat Completions：HTTP／SSE；结构化 reasoning 与原生搜索 | 模型列表／详情 | [chat/openrouter](../../sysroot/src/scoop.llm.client/tests/chat/openrouter/) |
| openai-responses | Responses：HTTP／SSE／WebSocket | token count，读取／取消／删除，显式压缩，模型列表／详情，文件操作 | [responses](../../sysroot/src/scoop.llm.client/tests/responses/) |
| deepseek-responses | Responses：HTTP／SSE；独立 reasoning 与接续规则 | 模型列表 | [responses/deepseek](../../sysroot/src/scoop.llm.client/tests/responses/deepseek/) |
| anthropic-messages | Messages：HTTP／SSE；显式 API 版本 | token count，模型列表／详情，文件操作 | [messages](../../sysroot/src/scoop.llm.client/tests/messages/) |
| gemini-generate-content | generateContent：HTTP／SSE | token count，模型列表／详情，文件上传／列表／读取／删除 | [generate_content](../../sysroot/src/scoop.llm.client/tests/generate_content/) |
| gemini-interactions | Interactions：HTTP／SSE | 读取／取消／删除，GET 事件及显式游标接续，模型列表／详情，文件操作及 URI 登记 | [interactions](../../sysroot/src/scoop.llm.client/tests/interactions/) |
| bedrock-converse | Converse／ConverseStream：HTTP／AWS EventStream | 模型列表／详情，显式 SigV4 认证 | [converse](../../sysroot/src/scoop.llm.client/tests/converse/)、[aws](../../sysroot/src/scoop.llm.client/tests/aws/) |
| cohere-chat-v2 | Chat v2：HTTP／SSE | 模型列表／详情 | [cohere](../../sysroot/src/scoop.llm.client/tests/cohere/) |
| ollama-chat | Chat：HTTP／NDJSON | 模型列表与 POST 详情查询 | [ollama](../../sysroot/src/scoop.llm.client/tests/ollama/) |
| gemini-live | BidiGenerateContent：WebSocket；setup 确认 | 显式内容、媒体、工具结果与会话通知 | [gemini_live](../../sysroot/src/scoop.llm.client/tests/gemini_live/) |
| openai-realtime | Realtime：WebSocket；session.created／updated | 显式响应、会话 item、音频缓冲和函数／MCP 事件 | [realtime](../../sysroot/src/scoop.llm.client/tests/realtime/) |
| openai-gpt-live | GPT-Live：WebSocket；session.start／started | 音频、上下文、后端 Responses 命令与原始嵌套事件 | [gpt_live](../../sysroot/src/scoop.llm.client/tests/gpt_live/) |

OpenAI／Anthropic 文件操作包括上传、列表、元数据、下载和删除；Gemini 没有按已上传 file ID 下载的封装，实际 downloadUri 保留原文。会话 provider 的辅助资源可通过相应普通 API provider 显式访问，不自动切换入口。各辅助 API 的网络与清理测试集中在 [files](../../sysroot/src/scoop.llm.client/tests/files/)、[models](../../sysroot/src/scoop.llm.client/tests/models/) 和 [count](../../sysroot/src/scoop.llm.client/tests/count/)。

## 共同合同与使用方式

| 设计要求 | 实现与验收位置 |
| --- | --- |
| 缺省参数自然产生简单入口 | LlmRequest、User 消息及 LlmGenerationOptions；[缺省值测试](../../sysroot/src/scoop.llm.client/tests/request/defaults.scoop) |
| 模型 capability 明确描述输入、输出、形式及工具组合 | Supported／Unsupported／Unknown 与请求检查；[capability](../../sysroot/src/scoop.llm.client/tests/capability/)、[client](../../sysroot/src/scoop.llm.client/tests/client/) |
| 客户端函数／custom／原生工具与服务端工具区分 | 有序 ToolDefinition 与执行方分别编码；各协议的 tools、server_tools 和流事件测试 |
| 文本、媒体、签名、reasoning、工具及历史可保存 | 公共 companion codec 和按 dialect 归属的原生历史；各协议的 media、history、tool_results 测试 |
| 单次生成、流式、结构化结果共用 API | generate／openStream／withStream，textDeltas 与 decodeOutput；[network](../../sysroot/src/scoop.llm.client/tests/network/)、[response](../../sysroot/src/scoop.llm.client/tests/response/) |
| 实际计时、token 与发送／接收时间戳 | 每次 HTTP 尝试与每条会话消息分别采样；[计时](../../sysroot/src/scoop.llm.client/tests/response/timing.scoop)、[HTTP 时序](../../sysroot/src/scoop.llm.client/tests/response/http_timing.scoop)、[metrics](../../sysroot/src/scoop.llm.client/tests/metrics/) 及各 session 测试 |
| 附加信息保留原文 | native 与 dialect 扩展，不计算费用或推断缓存失效；兼容用例只验证缺失／改变形状不妨碍主线 |
| 真实连接有界关闭与所有权 | HTTP／WebSocket 共享寿命，借入 HttpClient 不被连带关闭；各协议网络和 session 测试 |

最小调用始终使用 endpoint.model、LlmRequest 和 client.generate；没有另一套 prompt／SimpleClient 入口。Messages 的必需输出上限来自 provider 的可配置缺省，其余未给出的生成参数保持省略。会话入口也是完整 open 方法的缺省参数加 useAsync，模型与凭据仍由应用提供。

[共同入口示例测试](../../sysroot/src/scoop.llm.client/tests/examples/common_entry.scoop) 将同一个已序列化请求交给 Responses、Messages 和 generateContent：复用 HTTP，混合声明客户端与服务端工具，保存结果及真实时间戳，并在认证前拒绝跨 dialect 的原生历史。它补充协议之间的组合验收；不会重新执行所有协议测试。

## 验证范围

普通测试全部使用纯协议数据或本地回环 mock。各功能的本机验证、nuc12 最小用例、生产库消费者和必要底层修复分别记录在 PROGRESS；这些是分批验收结果，不表示在最终提交上重新跑过完整套件。

本次收尾使用 release 生产库及独立消费者执行所属 Cone 的代表用例：本机 4 项缺省值与 1 项跨协议组合分批通过，2 个真实服务用例默认忽略；nuc12 仅跨协议组合 1/1 通过。没有编译器修改，不运行完整 fixture 或 Rust 测试套件。原始日志和临时消费者留在仓库外；提交前核对凭据，清理闲置产物时保留源码、日志与共享缓存。

真实互通与 mock 分开：

- 已有真实记录包括 DeepSeek Chat 的文本／流和工具历史接续、代理 gpt-6.1-sol 的 Chat／Responses、代理 databricks-claude-opus-5-5 的 Messages，以及 Google 的 Interactions、模型查询、文件和一次 Gemini Live 音频响应。具体操作与限制以 PROGRESS 为准。
- 其余入口按官方文档做本地 mock；缺少服务或 key 不阻塞本次 API 封装，但也不记作真实互通通过。
- 仓库内真实用例在 [tests/live](../../sysroot/src/scoop.llm.client/tests/live/) 并标记 @Ignore，执行前检查所需环境变量；此前仓库外的真实测试同样默认忽略。库本身不读取环境变量。
- 失效的官方 OpenAI key 不使用；新提供的图像代理配置属于可选验证资源，本轮未发图像生成请求。

范围到上述 LLM API 及相关辅助操作为止。模型选择、工具执行、Agent 循环、设备音频处理、云部署、账号管理、计费计算与缓存寿命判断由库外承担。
