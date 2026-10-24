# LLM API client 调研

调研日期：2026-10-21。架构同步：2026-10-24。状态：调研与设计建议，尚未实施，不替代正式规范。Protocol／dialect／provider 的职责、模型 capability 和服务端工具以 [架构设计](DESIGN.md)为准，调用形态见[使用示例与缺省参数](USAGE.md)；本文保留协议覆盖、传输与部署调查。

目标是提供可以支撑 OpenCode 一类应用的 LLM API client。本文只讨论模型请求、响应、流式协议、认证、兼容适配与这些操作的资源寿命。HTTP 的源码评审与缺省值建议见 [HTTP client API 评审](HTTP-CLIENT-REVIEW.md)。

## 1. 结论

现有 `scoop.http.client` 已能承担主要传输工作，不需要另建 HTTP 栈。新增工作的主体是不同 LLM 协议的消息映射、增量解码、工具与推理状态回传，以及可独立组合的端点和认证。

建议实现普通 Scoop 库 `scoop.llm.client`，将 **API protocol、dialect、provider、模型能力、部署端点和认证**分开。一个 provider 固定包装一组 protocol＋dialect；同一供应商的多个 API protocol 使用多个 provider。提供简单的 `generate` 和有明确寿命的 `withStream`，同时保留完整结果与原生扩展。未知模型、第三方域名、自定义 provider 名称不需要加入内置名单才能使用。

“完整”不能以 provider 数量验收。同一家供应商可以提供几种协议，分别包装为不同 provider；同一种协议也会有不同部署与方言。真正的验收单位是协议、方言、消息类型、流式状态转换及它们的组合。不能承诺任意未知协议仅修改 URL 就兼容：已有组合通过配置接入，新的线协议通过公共适配接口接入。

当前资料还显示两项容易遗漏的变化：

- OpenAI Responses 已有独立于普通 SSE 的 WebSocket 模式，当前协议包含多路事件分发与运行中输入等能力；不能把所有 WebSocket 都当作实时语音。[O1][O2]
- Gemini Interactions 已成为官方推荐入口，`generateContent` 仍受支持。两者的消息和流式事件不同，应分别支持。[G1][G2][G3]

## 2. 职责边界

| 范围 | 本库承担的工作 |
| --- | --- |
| 一次模型生成 | 请求编码、普通与流式响应、完整结果、取消、错误、token usage、发送／接收时间戳、耗时及响应元数据 |
| 多轮所需的协议数据 | 输入历史、工具调用与结果、推理回传数据、显式 continuation ID；由调用者提供下一次请求 |
| 服务端原生工具 | 与客户端工具共用 tools 声明入口，按执行方区分；编码配置，返回活动、结果、引用及进度，不要求应用执行或补结果 |
| 客户端工具 | 工具声明、JSON Schema、选择方式、完整调用与参数增量、结果消息；不执行工具 |
| 多模态 | 文本、图像、音频、视频、文档、文件引用及相关元数据；保留原生支持差异 |
| 实时会话 | 已选协议的连接、发送、接收、结束和取消；不承担音频采集、播放、重采样或 UI |
| 生成相关资源 | 显式的文件上传／查询／删除、模型查询、服务端 token count、生成任务查询／取消，可作为独立 API 模块 |
| 其他推理 API | embeddings、rerank、独立图像／语音／转写接口可以使用独立请求与结果类型接入同一传输；不塞入聊天消息模型 |

以下工作由后续 Agent 层承担：模型选择和自动切换、工具调度与执行、循环调用模型、MCP client、文件或终端操作、会话数据库、上下文裁剪与摘要策略、记忆、RAG 检索和索引、权限交互、登录 UI、账号管理、跨供应商价格目录、预算／扣费／账单管理、TUI／GUI。单次调用的原生计费信息或费用估算可以作为 dialect 的可选扩展。

调用模型提供的原生 compaction／background 接口属于 API 操作；决定何时压缩、替换哪些历史、是否继续轮询或启动下一轮属于调用者。托管 Agent 产品的运行、环境和任务管理不因共用某个端点而自动进入本库。

API key、OAuth token 和请求签名是外部 API 的正常认证，不引入编译产物授权、来源证明、通用资源计费或新的编译器／runtime 机制。

## 3. 应覆盖的协议与部署

### 3.1 请求与响应协议

| 协议族 | 主要形状与流式方式 | 必须保留的差异 |
| --- | --- | --- |
| OpenAI Chat Completions | `messages`、`choices`；SSE `delta` | candidate index、分段工具参数、末尾 usage、结束标记、refusal；兼容服务的 reasoning 扩展 |
| OpenAI Responses／Responses 兼容接口 | `input`／`output` item；带类型的 SSE 事件；独立 WebSocket 模式 | message、reasoning、function/custom tool call 与 output 分立；item ID、call ID、原生工具、原始 continuation 数据 |
| Anthropic Messages | `system`／`messages` 与 content blocks；SSE | block index、`tool_use`／`tool_result`、thinking 与 signature、原生工具结果、cache usage、`pause_turn` 等原生停止状态 |
| Gemini Interactions | 有序 steps、`previous_interaction_id`；SSE `step.*` | thought、function call/result、模型输出与服务端工具步骤；签名、`requires_action`、最终 usage |
| Gemini generateContent | `contents[].parts`、`systemInstruction`；`streamGenerateContent` | 多 candidate、`functionCall`／`functionResponse`、`thoughtSignature`、grounding、文件与多模态 parts |
| Amazon Bedrock Converse | `messages`／content blocks；ConverseStream | **AWS 二进制 EventStream**，不是 SSE；模型 ID／inference profile／ARN、additional model fields、reasoning、metadata |
| Cohere Chat v2 | messages、documents、tools；SSE | 独立的 content、tool-plan、tool-call、citation 事件及 usage；参数组合限制 |
| Ollama 原生 API | `/api/chat`，必要时 `/api/generate`；NDJSON | 默认流式、`done`／`done_reason`、thinking、JSON tool arguments、模型加载信息、流中 error |

协议依据分别见 [O1][O3]、[A1]、[G1][G2][G3]、[B1]、[C1]、[L1][L2][L3]。Bedrock EventStream 的实际解码实现也可对照 OpenCode 使用的 Smithy codec。[S4]

Bedrock `InvokeModel`／`InvokeModelWithResponseStream` 是另一组调用入口，具体请求内容取决于模型协议。需要访问 Converse 未暴露的模型能力时，以对应原生适配器加 Bedrock 传输封装支持，不能把 Converse 的 JSON 直接发送到 InvokeModel。

Mistral、DeepSeek、Moonshot、MiniMax、阿里云、智谱、火山方舟、Groq、Together、Fireworks、OpenRouter、各类网关与本地服务应优先复用实际提供的 Chat／Responses／Messages 协议。对于额外字段、工具 ID 约束、thinking 或原生内容，补充有真实请求样本的方言适配；不能把“兼容”理解为所有参数和消息完全相同。例如 OpenRouter 的 `reasoning_details` 需要保留结构，Mistral 的工具 ID 处理也不是通用 Chat JSON 的直接复制。[R1][S5]

### 3.2 部署与协议分别配置

Protocol＋dialect 由 provider 固定，URL 和认证由 endpoint 配置。比如 `openai-chat` 与 `openai-responses` 是两个 provider；同一模型可通过两个入口建立各自的 Model 绑定。具体 API 支持不按模型新旧推断，定义见[架构设计第 1 节](DESIGN.md#1-核心对象与关系)。

| 部署 | 独立配置内容 |
| --- | --- |
| 官方云 API | 默认 URL、版本 header、API key header；使用什么模型由每次请求指定 |
| Azure | 资源 URL、deployment 名称、认证；现代 `/openai/v1/` 与旧版 deployment 路径分别配置。v1 不再要求日期式 `api-version`，不能全局强加旧规则。[Z1] |
| Vertex AI | 项目、区域、publisher／model 路径、访问 token；Gemini 和 Claude 使用各自协议 |
| Bedrock | region、runtime／其他入口、模型或 profile 标识；SigV4 或接口支持的 Bedrock API key。当前 API key 不能覆盖全部双向流接口。[B2] |
| 第三方网关 | 任意 base URL、路径前缀、query、headers、认证和方言；网关自身的 provider routing 参数保留为其扩展。[R2] |
| 本地推理 | 明确选择协议、任意模型字符串、默认可无认证；不能要求伪造 API key |

不根据模型名称前缀推断协议，不把 `provider/model` 当作必须拆分的全局编码。`organization/model`、包含冒号的本地模型名、deployment 别名和 ARN 都应由所选端点规则解释。显示名称与发送到服务器的模型标识分离。

`listModels` 是可选的显式操作。调用 `generate` 不先联网下载目录，不因模型不在目录中而拒绝。context window、输入／输出模态、工具支持等描述可由调用者、静态说明或服务端查询提供，但不是模型调用的注册条件。

### 3.3 实时与持久连接

完整协议覆盖还需要独立的会话接口，不应硬塞进单次 `generate`：

- Responses WebSocket：创建／继续响应、按响应与 lane 路由事件、运行中输入、请求级错误与连接级错误、断连后的显式恢复。当前文档的连接内缓存不能被当作持久会话数据库。[O2]
- OpenAI Realtime 与 GPT-Live：分别编码其会话和音频事件。当前 GPT-Live 没有与 Realtime 完全相同的逐轮音频结束事件，不能虚构统一的“播放结束”。[O5]
- Gemini Live：WSS 双向音频、图像与文本、工具事件、转写、打断、会话寿命和恢复信息。[G4]

本项目现有 WS／WSS 足以作为这些 WebSocket 适配器的传输基础。WebRTC、SIP 和设备处理不成为普通 LLM client 的前置依赖；需要这些接入时在对应传输／媒体库另行实现。AWS 等服务的原生双向二进制接口也必须独立适配，不能声称有 WebSocket 即已覆盖。

## 4. 消息与结果模型

### 4.1 不能只有 role 与字符串

统一模型应包含有序 message／item 与有序 content part。通用角色包括 system、developer、user、assistant 和 tool；provider 对角色位置和工具回合的约束由适配器表达。无法等价映射的 developer 指令、会话中途 system 变更等不能静默降级为 user 文本。

| 内容或数据 | 必要表示 |
| --- | --- |
| 文本 | 原始文本、块位置、可选引用与 annotations；便利的 text 视图不混入推理或工具参数 |
| 图像 | bytes、URL 或 provider file reference，MIME、detail／resolution；输入和生成结果均可表达 |
| 音频 | bytes／引用、编码与必要格式、转写、speaker／时间信息；支持增量音频和文字交错 |
| 视频 | 文件／URL 或帧序列、时间戳、MIME 与 provider 原生参数；不自动做视频解码 |
| 文档与文件 | PDF／文本等内容、文件名、MIME、文件引用、页码或位置引用 |
| 推理 | 可展示的文本／摘要与不透明回传数据分开；保留 encrypted content、signature、redacted thinking 和必要的原生 item |
| 函数调用 | 调用 ID、名称、可选 namespace、参数原文、解析后的 JSON、执行归属；可有多个并行调用 |
| 自定义工具调用 | 支持原始文本／受语法约束的输入，不能强制一切参数都是 JSON object |
| 工具结果 | 匹配调用 ID 的文本／JSON／多模态结果，明确错误结果；不转换成普通 assistant 文本 |
| 服务端工具 | 调用、结果、进度、错误和引用；与等待本地应用执行的调用区分 |
| 拒答与内容过滤 | 独立状态和内容，保留原生原因；空文本不自动解释为拒答 |
| 引用 | URL／文档／文件来源、标题和范围；保留范围单位，不能把 UTF-16、字节和 Unicode 标量下标混用 |
| 原生扩展 | 带协议名称的原生 item／part／字段；已知内容类型也能附带未识别字段 |
| continuation 数据 | response／interaction ID、原生 continuation token、compaction item 等；只表达协议数据 |

`ToolCallId`、响应 ID、item ID 与 candidate／block index 应有不同类型或清晰的结构位置。服务端没有 call ID 时保存名称／位置关联，库内关联标识不能冒充服务端生成的 ID。方言要求改写工具 ID 时，调用和结果必须采用同一显式映射。

### 4.2 保留原生内容才能正确往返

Responses 的 reasoning item、Claude thinking signature、Gemini thought signature 不是 UI 文本附件；丢失它们可能破坏下一次工具结果回传。应同时提供：

1. 方便应用处理的通用消息与事件。
2. 可序列化并原样回传的 provider 原生数据。

这些数据只在相应 protocol／dialect 和允许的部署／模型上下文中复用。跨 provider 切换不能把签名当成通用推理；不支持的状态或模态要报告给调用者，由后续 Agent 决定如何处理。库不重新验证、生成或解密服务端签名。原生历史由适配器保存，通用字段提供读取视图；修改历史须创建新输入，不与旧 raw 隐式覆盖。具体往返合同见[架构设计第 5 节](DESIGN.md#5-消息回传与序列化)。

未知字段和事件不应直接导致整个流失败；可交付带原始类型和内容的扩展事件。涉及未知控制语义、无法确定终止或无法构造完整结果时，明确报告协议能力不足，不能把漏掉的内容当作成功。

### 4.3 选项、能力与 usage

- 通用选项覆盖输出 token 上限、stop、采样、工具选择、并行调用、结构化输出及 reasoning；只统一能明确解释的语义。
- temperature、top-p、top-k 等默认省略，保留模型默认行为；不向每个模型强行发送同一组采样参数。
- JSON object、JSON Schema 输出和 tool schema 分别表示。传入 schema 不代表每个 provider 支持全部 JSON Schema 关键字或任意工具组合。[C1]
- 能力区分“支持”“不支持”“未知”，属于模型在具体 provider／endpoint 下的绑定。文本、图像、PDF、音频、视频等输入与输出分别描述，并区分 MIME、输入形式和请求模式；工具及组合能力见[架构设计第 3 节](DESIGN.md#3-model-capability)。未知模型不等于不支持；明确不支持的功能在发送前给出具体错误。不得静默删除调用者明确要求的字段、工具或模态。
- 少量成熟能力使用类型化选项，变化快的原生配置通过按协议分组的 JSON 扩展传入。扩展与通用字段冲突应报错，完整原生请求入口另行提供，不依赖隐藏的覆盖顺序。
- prompt cache 的控制参数、服务级别与网关路由参数属于对应 API 的请求能力，应能显式传入并保留。缓存断点的选择、自动插入与历史重排属于上层策略；不能为了统一接口而删除调用者已经配置的缓存信息。
- usage 保存原生数据与可解释的规范化计数：input、output、total、cache read/write、reasoning、各模态、服务端工具使用等。未知计数用缺失值，不能填零。
- 区分累计快照与增量。Claude 的流式 usage 是累计值；最终 usage 不应与前面快照累加。不同 provider 的 cached／reasoning 计数归属也不能用同一公式猜测。[A1][G2]
- 保留原始停止原因、模型实际返回名称、请求 ID、HTTP status／headers 和 warnings。调用计时、请求／响应时间戳与 token 统计属于公共能力；计费信息由 dialect 可选扩展提供，保留报告／估算的区别，不建立通用预算引擎。
- 请求前的 token count 是显式操作，区分服务端计数和本地 tokenizer，保留覆盖范围与精确／估算性质；不能用预估覆盖响应 usage，也不在 generate 前隐式联网计数。

### 4.4 时间戳、缓存估计与计费信息

每次调用及其重试保存独立 metadata，为 request 标记 sentAt，为 response 标记 receivedAt，并记录首次事件／输出／文本、完整响应及实际收尾时刻。UTC Instant 用于保存和跨请求关联，同源单调时间用于计算耗时；流式响应头到达、内容完成和资源关闭是不同边界。同一 LlmRequest 并发复用时不互相覆盖时间戳。

这些数据结合 cache read／write usage 和 dialect 提供的 TTL 起算／刷新规则，可用于估计缓存有效时间。不能统一使用“响应完成＋TTL”，也不能把缓存最低保留期、精确到期时间和实际 KV 命中视为相同事实。ProviderReported 与 Estimated 计费信息分别保留来源、币种、精确十进制金额及完整性。详细定义见[架构设计第 6 节](DESIGN.md#6-计时token-与计费扩展)。

## 5. 流式协议与寿命

### 5.1 三层处理

```text
HTTP response body / WebSocket message
  → SSE / NDJSON / AWS EventStream framing
  → 对应 provider 的事件与状态转换
  → 通用 LlmEvent + 必要的原生数据
```

SSE 复用 `scoop.http` 的 `SseDecoder` 与 `scoop.http.asyncio.SseSource`。NDJSON 和 AWS EventStream 是实际缺口，需要实现对应的增量解码；不能把 HTTP chunk 当作一条 JSON，也不能把二进制 EventStream 交给 SSE parser。

事件至少覆盖响应／candidate／item 开始，文本、推理、工具参数和媒体增量，引用，usage 更新，完整 item，最终完成，warning 及原生扩展。结束状态与停止原因分开，保留 needs-action、incomplete 和 continuation 等差异。

`next()` 每次消费一个事件，单个流只有一个 reader。服务端 error 事件转换为保存其原始内容的类型化异常；网络失败、协议失败和 `CancellationException` 保留区别。不要同时发一个“成功结束”和一个失败，也不要把同一 error event 既交付为终止事件又再次当成独立异常计数。

### 5.2 必须处理的边界

1. UTF-8 跨块、SSE 多行 data／注释／空事件／CRLF、NDJSON 行跨块，以及一个网络读取中包含多个事件。
2. 参数 JSON 可能暂时不合法。原始 delta 可立刻交付；只有完整调用才产生可用于执行的参数。可选的部分解析结果必须标注未完成，不能用补括号伪造完整输入。
3. 多个 tool call、candidate 或 WebSocket lane 的事件可以交错，按实际 ID／index 聚合。不能只有一个全局字符串缓冲区。
4. 工具调用不等于响应立即终止。当前 Responses 异步工具可在调用后继续输出；本库只交付完整调用与后续事件，执行和调度仍属于应用。[O4]
5. finish reason 不一定是最后一条事件：Chat 的 usage-only chunk、Bedrock 的 metadata 等仍需读取。文本停止、item 完成、模型响应完成、HTTP EOF 是不同边界。
6. HTTP 200 中仍可能出现 provider error，Ollama 等也会在流中返回 error。没有协议要求的终止状态就 EOF，必须报告中断。[A1][L3]
7. 媒体增量须区分完整资源、局部图像预览、音频片段与累计快照；不能把重复快照当字节追加。
8. 服务端显式 continuation 与重新提交一个生成请求不同。通用 SSE reconnect 默认关闭，`Last-Event-ID` 不能自动当作模型生成的恢复凭证。

### 5.3 聚合、取消与背压

提供一个复用流式状态转换的结果累积器，供 `generate` 和需要完整结果的流式调用使用。只消费事件时不默认保存整条原始事件历史；`generate` 只保留最终结果需要的数据。字符串用 builder 累积，避免每个 delta 重新扫描或复制全部历史。

错误应携带已知 ID、停止位置、是否已交付输出和原始 provider 错误。事件模式的调用者保留已收到内容；选择了累积器时可取得明确标注未完成的结果。未完成工具参数不能以完整 ToolCall 返回。

遵守现有 EventLoop／Job／AsyncCloseable：`withLlmClient`、`withStream` 在退出时等待真实关闭；借入 HttpClient 时不关闭借入对象。`openStream` 与 `useAsync` 是同一资源合同的显式组合，选择 Result 时在关闭后读取完整结果及最终 metadata；block 内的快照不因作用域退出而暗中变化。提前退出流关闭对应响应，H2 单流取消不影响其他请求。WebSocket 共享读任务只分发事件，不直接执行用户挂起回调。

背压需要有限的实际帧、事件和发送队列。为具体媒体、SSE 行或 AWS frame 设置可配置字节上限，不能扩展成跨阶段通用资源预算。长音频／长输出允许调用者增量处理，不默认全部 materialize。

## 6. 认证与任意第三方 provider

### 6.1 最小且够用的接口

| 认证形式 | 需要的能力 |
| --- | --- |
| 无认证 | 本地服务或已在其他位置处理认证的网关可直接使用 |
| Bearer／其他 scheme | 固定凭据或 suspend token source；scheme 与模型解耦 |
| 自定义 API-key header | `x-api-key`、`api-key`、`x-goog-api-key` 或任意名称，不枚举品牌 |
| query 或组合 header | 显式构造参数；正常 URL 编码；避免重复冲突 |
| OAuth／短期访问 token | 注入异步 token source，支持到期信息与刷新；单个凭据源并发刷新合并 |
| 签名 | 取得实际 method、最终 URL／query、签名相关 headers 和准确 body 字节；支持 SigV4 及第三方签名函数 |
| mTLS | 复用 HTTP／TLS 的客户端身份；不同身份使用适当配置的 HttpClient |

不把 key 存进模型对象。认证输入以实际目标请求为准，而不是 provider 显示名。API-key 登录 UI、浏览器 OAuth、device flow UI、refresh token 持久化、keychain 或配置文件查找由应用提供；库只执行已配置的凭据获取和请求认证。

现有 HTTP Bearer 已支持匹配挑战的刷新和并发合并，应复用适用的部分。但它要求匹配的 `401 Bearer` challenge，不是“任意 401 JSON 都能刷新”。不带该 header 的服务、签名认证及 API-key 轮换需要 LLM 认证适配，不能假装已被覆盖。

### 6.2 实际请求顺序

```text
调用选项合并与协议映射
  → 确定部署 URL、query 与最终 body 表示
  → 获取该目标所需凭据
  → 对实际发送表示生成认证／签名
  → 执行一次 HTTP 请求
  → 解析响应或按明确策略重新尝试
```

重试时重新取得有效凭据并重新签名；签名之后的 body、URL 或被签名 header 不能再被其他适配器改写。HTTP `onAttempt` 当前是 client 级、只添加字段，且在排池之前执行；需要考虑排队导致签名或 token 过期，不能把它描述为“发包前最后一刻”的 hook。详见 HTTP 评审。

静态 API-key header 会在现有 HTTP 跨 origin 重定向中被保留。LLM 请求默认关闭重定向；显式允许重定向的认证策略必须重新判断新 URL 并生成凭据。这里需要的是正常的凭据作用域，不需要 provider 身份认证体系。

### 6.3 三种扩展方式

1. **配置现有 provider**：选择固定的 protocol＋dialect，再配置 base URL、路径／query、认证、headers 和任意模型 ID。生成参数保存在 request，协议强制要求的缺省由 dialect 提供。
2. **扩展已知协议的方言**：处理具体字段、历史回传、服务端工具、错误、工具 ID 或终止差异，与该 protocol 组成新的 provider；以真实 wire 样例和独立 Cone 测试验证。
3. **实现新的协议适配器**：公共 Scoop interface 接收完整请求并返回完整结果／事件流，复用 HTTP、生命周期与错误设施，再包装为 provider。

不需要运行期下载插件或全局 provider registry。普通 Scoop Cone 实现接口即可接入。接口签名遵守 Scoop 的规则，不引入泛型虚方法、反射或编译器对 provider 的特判。

## 7. 简单入口与建议缺省值

下面是**拟议 API 形态**，尚不存在这些类型或方法；示例用于说明调用负担，正式签名应在实施前写入规范。

```scoop
val endpoint = LlmEndpoint(
    provider = Providers.openAiChat(),
    baseUrl = Some(Url.parse("https://gateway.example/team/v1/")),
    authentication = LlmAuth.bearer(apiKey)
)
val model = endpoint.model("organization/custom-model")

withLlmClient suspend { client ->
    val result = client.generate(model, LlmRequest([Message([Text("你好")])]))
    println(result.text)
}
```

运行在应用已有的 EventLoop 中，不隐藏启动另一个 loop。`generate` 返回完整结果，`text` 只是指定主 candidate 的文本视图，其他 candidate、工具、推理、媒体与 usage 仍在结果中。原生 API 不支持多 candidate 时不能假装支持 `n`。

所有完整生成使用 `client.generate(model, request, options = LlmCallOptions())`；request 的 input 必填，新消息默认 User，system／tools 为空，生成选项省略。复杂调用只在同一个 request 增加内容；不另设 prompt 重载、文本专用返回值或简单 client。生成参数只在 request 一处表达，调用选项管等待、重试和观察，应用可直接复用普通配置值。

流式调用使用 `withStream(model, request) suspend { stream -> ... }` 与 `next()`；需要关闭后读取最终结果时使用同一资源合同的 `openStream`／`useAsync`。tools 同时容纳客户端函数／文本工具和服务端工具，结果与活动按执行方区分，见[架构设计第 4 节](DESIGN.md#4-服务端工具)。长期运行应用复用 client，也能显式借入已有 HttpClient。命名的 provider 工厂只提供常用组合与默认配置，不限制模型字符串。完整例子与缺省覆盖见[使用示例](USAGE.md)。

以下数值是实施时的起始建议，应写清可覆盖方式并用真实用例确认，不把它们当作供应商承诺：

| 项目 | 建议 |
| --- | --- |
| provider／认证／model | 显式提供；endpoint URL 可采用 provider 的已知部署缺省，没有库级默认 provider 或默认模型，不隐式查询环境凭据 |
| request | input 必填；新消息默认 User，system／tools 为空；生成参数只在 request 表达 |
| 采样与 reasoning | 默认不发送；模型有独立默认值，调用者明确选择时再映射 |
| 输出长度 | 通用请求默认省略；协议强制要求时，适配器提供公开可改的缺省值，例如 4096，而不是隐含无限或固定模型目录 |
| TLS／HTTP | 使用系统信任与 HTTP Auto；复用现有连接池 |
| 排池／建连 | 排池建议 30 秒；DNS、TCP、TLS、代理握手沿用各 30 秒 |
| LLM 等待 | 响应头／read idle 初始建议各 300 秒，write idle 60 秒，total 默认 None；长推理或大上传可覆盖／关闭对应阶段 |
| 重试 | 至多 2 次已明确可重试的服务拒绝，如 429／明确 overload；未交付输出时才可重试。普通已发送 POST 的结果不明、部分输出和取消不自动重试 |
| 退避 | 可取消的指数退避与 jitter；遵守 Retry-After，不因本地 backoff 上限而提前重试 |
| 重定向／SSE reconnect | 默认关闭，避免改变 POST 语义、外送自定义凭据或重复生成 |
| Cookie／HTTP cache | 默认不用；LLM prompt cache 是 API 参数，不能与 HTTP cache 混为一谈 |
| 服务端状态 | 通用历史模式默认无自动服务器会话；适配器提供时显式选择 `store`。需要留存的 continuation／background 模式由调用者开启 |
| 错误正文 | 保留有限前缀，例如 64 KiB，明确 truncated；不先对错误 body 无限读取 |
| 普通完整 JSON | 建议从 8 MiB 上限起；大媒体使用显式更大上限、引用或增量消费 |
| 多模态流事件 | 必须可覆盖当前 SSE 的 64 KiB 行上限；可从 16 MiB 的实际行／事件上限评估，按媒体接口的真实尺寸决定 |
| 日志 | 默认不记录 prompt、媒体、认证、URL query 或完整响应；保留请求 ID、状态、耗时与 usage 的观察入口 |
| 计时与 token | 默认采集关键时间戳与实际 usage，不保留逐 delta 日志；缺失计数不填零，预计数显式调用 |
| 计费信息 | Dialect 可选扩展；未配置时保持缺失，使用服务端报告或显式价格快照，不自动查询价格 |

LLM 若管理重试，应在对应 HTTP 请求上关闭重复的 HTTP 重试，避免嵌套放大。现有 HTTP 内部有 H2 “未被处理”的安全重试分类；若后续要复用这类事实，应使用具体的类型化合同，不能靠错误字符串判断。对于无法确认发送状态的传输失败，默认返回给调用者。

## 8. 与 Scoop 现有能力的衔接

| 能力 | 已有基础 | 新增工作 |
| --- | --- | --- |
| HTTP／TLS／代理／取消 | `scoop.http.client` 与所属协议、TLS、asyncio 库 | 复用，按生成请求选择 options |
| 计时与时间戳 | 平台 Clock／TimeSource、HTTP 单调阶段指标 | 补 HTTP 请求级 wall／monotonic 采样合同；LLM 记录首次输出与完整响应，按尝试关联 |
| JSON 编解码 | 默认／显式 codec、不可变 JsonValue DOM、builder、object codec、HTTP 默认 JSON 重载均已提供 | 稳定 wire DTO 复用派生；discriminator、三态字段、历史回传与媒体表示由适配器明确处理 |
| SSE | `SseDecoder`、`SseSource`、POST `openSse` | 复用 framing，新增 provider 状态转换；使用可保留错误 body 的响应入口 |
| NDJSON | 普通异步字节流 | 增量行／记录解码，明确 EOF 与错误记录 |
| AWS EventStream | 字节、整数编码及加密基础 | 必需的帧长度、header、CRC 和 payload 解码；选择维护中的实现或小型协议模块，不重建 AWS SDK |
| 签名 | `scoop.crypto` 的 digest、HMAC、签名；URL 和时间库 | SigV4／具体外部 API 签名适配，确保签的是实际 body |
| WebSocket | WS／WSS、分片、控制帧、背压 | Responses、Realtime／Live 等各自会话状态 |
| JSON Schema | JSON 值与 codec | 请求中的 schema 表达和 provider 子集映射；不以完整通用 schema 引擎作为第一轮必需依赖 |

`JsonValue` 已有公开动态操作，可保存未知内容、直接 stringify，或使用 encodeToValue／decodeFromValue 与 typed 值转换。稳定 Decoder 允许先读取 discriminator，再将同一入口交给分支 DTO；不需要 stringify 后重新 parse。部分工具参数解析若要新增增量 JSON 能力，应放在 JSON 所属层并证明实际收益；最初交付原文 delta 与完成后解析即可。

[序列化与 JSON 库调研](../json/INVESTIGATION.md)已同步 M55／M57 及核心容器、DOM／String-key object codec 的实现。它们不自动改变 Map、enum 或多模态字段的 wire 格式，具体边界见[架构设计第 5.2 节](DESIGN.md#52-复用当前序列化能力)。

初始采用一个 LLM client Cone 内的消息、事件、协议、认证和媒体子模块。按真实依赖需要再拆 Cone，不给每个商业品牌复制一整套 client，也不先搭建通用插件框架。

## 9. OpenCode 与 AI SDK 的可借鉴之处

OpenCode 网站仍说明使用 AI SDK 与 Models.dev，并提供自定义 provider、base URL、headers 和模型配置。[S1] 调研取得的 **dev 提交 `a13c48b90c885f3518a3a831533cb42151ac4920`** 已包含独立的 `packages/ai`，其 Protocol、Endpoint、Auth 和消息／事件定义可直接对照。[S2][S3][S4]

这两类资料不能混称为同一已发布实现。本次结论以明确注明的文档或源码快照为依据，不声称已经验证 OpenCode 发布版本的全部 provider。

值得借鉴的是协议与部署分离、公共扩展接口、丰富的工具／推理事件，以及原始 usage 和回传数据的保存。AI SDK 同样公开了可实现自定义 provider 的语言模型规范。[S6]

2026-10-24 已按 AI SDK v7 文档及固定源码快照完成[进一步对照](AI-SDK-REVIEW.md)：补齐供应商定义、应用执行的 ClientNative，明确 request／message／part／tool 的方言扩展位置，增加同一消费者的文本投影与结构化输出示例，并保留非缓存输入用量。Provider-defined 与 provider-executed 不能视为同义；SDK 的 response timestamp 也不能替代本地发送／接收时间。对应调整已同步架构和使用示例。

不应照搬它们的全部架构：Scoop 不需要 Effect runtime、JavaScript 的运行期包加载、模型目录前置校验、工具执行器、自动 cache placement 或一套额外 Agent runtime。服务器提供的功能也不应仅因参考实现暂时未支持而从本库数据模型中排除。

## 10. 实施顺序与验收

下列是同一目标的实施顺序，不代表完成最早一阶段即可宣称“完整支持”。不在本轮擅自指定新里程碑编号或承诺工期。

| 阶段 | 工作 | 可运行的出口 |
| --- | --- | --- |
| 1. 规范与公共合同 | 固定 protocol／dialect／provider、capability、消息／事件／错误、服务端工具与调用 metadata；复用 JSON／HTTP 入口并补窄的时间采样合同 | 可编码请求与解码结果；发送／接收时间可取得，以 Chat 和 DeepSeek dialect 验证具体差异 |
| 2. 主流生成闭环 | Chat、Responses、Messages、Gemini 两种 API；文本、多模态、客户端工具、服务端 WebSearch、reasoning、完整与流式结果 | 应用提供工具结果的两轮请求；服务端工具活动与引用；保留原生历史并继续生成 |
| 3. 部署与协议扩展 | Bedrock、Cohere、Ollama、NDJSON／EventStream、Azure／Vertex、网关与常见方言、灵活认证 | 任意 endpoint＋model 可配置；签名／刷新和真实流式错误闭环 |
| 4. 持久会话与辅助 API | Responses WS、Realtime／Live、文件与 token count 等模型 API、缓存寿命及可选 billing 扩展；独立推理接口按其类型接入 | 双向会话取消与关闭、媒体事件、显式 continuation／任务操作、缓存与费用解释 |
| 5. 兼容与文档验收 | 协议／方言矩阵、组合 Cone 测试、必要的外部进程 fixture、三目标消费者、可运行 quickstart 和扩展示例 | 每项已声明能力有实际证据，未验证服务明确标注 |

正式实施前先更新相关 spec，再写实现。具体拆分与首个闭环见[架构设计第 8 节](DESIGN.md#8-模块与实施顺序)。HTTP、JSON 或 TLS 的公共合同变化落到所属规范，不用 LLM 专用绕行掩盖底层缺口；通用 HTTP 便利函数不作为生成闭环的前置要求。

必要的验收组如下：

- 每种协议都有独立请求编码、非流式结果、流式重组和错误用例；对端按实际线格式返回，不只 mock 通用接口。
- 文本、图像／文档、音频／视频、工具与推理各有独立用例，再覆盖“图像＋工具＋签名回传”“推理＋并行工具＋第二轮”等组合。
- UTF-8／JSON／SSE／NDJSON／EventStream 在任意字节边界拆分；多个 frame 合并读取；AWS 错 CRC、长度和提前 EOF。
- 多个工具参数交错、缺失／延后到达的 ID、错误 JSON、custom tool 文本、完成前取消；不把未完成输入交付成可执行调用。
- usage-only 尾块、累计 usage、空 choices／content、refusal、200 中的 error、缺少终止标记、未知可忽略事件及未知必要内容。
- 请求／响应时间戳、wall clock 回拨、重试关联与并发复用、长流占用缓存 TTL、读取是否刷新、计数缺失／重复累计、服务端报告与本地预估、可选计费的部分与未知费用。
- 两个自定义 provider 名称、同一模型字符串在两个端点、同一服务 URL 的 Chat／Responses 分别绑定两个 provider、含路径前缀的 base URL、带斜杠／冒号的模型 ID、无目录也可调用。
- 同一模型在不同 provider 下的 capability、PDF 与图像的区别、媒体格式／输入方式、模式与工具组合限制；服务端工具未执行、多次活动、工具级失败和引用，且不进入客户端执行列表。
- Bearer、自定义 header、无认证、query、异步到期刷新、并发刷新合并、取消刷新、JSON 401 无标准挑战、签名 body 与实际发送字节相同。
- 自定义凭据跨 origin 重定向、307／308 body 重放、429 与 Retry-After、未知发送结果不重试、已产出内容不自动重放。
- HTTP/1／HTTP/2 流消费、背压、提前退出、共享 H2 单流取消、借入 HttpClient 的关闭责任。
- WebSocket 相互独立的响应／会话、交错事件、错误作用域、断连、结束事件与最终 usage；预录媒体字节即可验证协议，不要求真实设备。
- 官方服务的 live smoke 与第三方互操作另做明确记录；本地协议测试通过不等于访问过每家商业 API。

普通 API、codec 和协议行为使用所属 Cone 的 scoop.test；fixture 仅承担必须由外部进程、CLI、产物或链接覆盖的行为。实现验收沿用开发 sysroot 和三个支持目标；不移走源码／工具来证明复用，不把普通功能验收变成 GC stress。当前仅完成资料与源码调研及架构设计，没有调用付费模型 API，也没有运行或宣称通过新增 client 的测试。

## 11. 来源

以下网页在调研中读取了正文；OpenCode 源码链接固定到本次读取的提交。规范数值、接口版本和服务能力仍可能变化，实施时应再次核对涉及的具体合同。

[O1]: https://developers.openai.com/api/docs/guides/migrate-to-responses
[O2]: https://developers.openai.com/api/docs/guides/websocket-mode
[O3]: https://developers.openai.com/api/docs/guides/streaming-responses
[O4]: https://developers.openai.com/api/docs/guides/async-tool-calling
[O5]: https://developers.openai.com/api/docs/guides/voice-websockets
[A1]: https://platform.claude.com/docs/en/build-with-claude/streaming
[G1]: https://ai.google.dev/gemini-api/docs/interactions-overview?hl=en
[G2]: https://ai.google.dev/gemini-api/docs/streaming?hl=en
[G3]: https://ai.google.dev/api/generate-content?hl=en
[G4]: https://ai.google.dev/gemini-api/docs/live-api?hl=en
[B1]: https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html
[B2]: https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-use.html
[C1]: https://docs.cohere.com/v2/reference/chat-stream
[L1]: https://docs.ollama.com/api/streaming
[L2]: https://docs.ollama.com/api/chat
[L3]: https://docs.ollama.com/api/errors
[R1]: https://openrouter.ai/docs/guides/best-practices/reasoning-tokens
[R2]: https://openrouter.ai/docs/guides/routing/provider-selection
[Z1]: https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle
[S1]: https://opencode.ai/docs/providers/
[S2]: https://github.com/anomalyco/opencode/blob/a13c48b90c885f3518a3a831533cb42151ac4920/packages/ai/README.md
[S3]: https://github.com/anomalyco/opencode/blob/a13c48b90c885f3518a3a831533cb42151ac4920/packages/ai/src/route/protocol.ts
[S4]: https://github.com/anomalyco/opencode/blob/a13c48b90c885f3518a3a831533cb42151ac4920/packages/ai/src/protocols/bedrock-event-stream.ts
[S5]: https://github.com/anomalyco/opencode/blob/a13c48b90c885f3518a3a831533cb42151ac4920/packages/ai/src/protocols/utils/mistral-tool-id.ts
[S6]: https://ai-sdk.dev/docs/foundations/providers-and-models

| 标记 | 资料及用途 |
| --- | --- |
| [O1]、[O2]、[O3] | OpenAI 官方文档：Responses／Chat 的差异、SSE、WebSocket |
| [O4]、[O5] | OpenAI 官方文档：异步工具调用与两种实时语音协议 |
| [A1] | Claude 官方流式文档：block、thinking signature、累计 usage、流中错误 |
| [G1]、[G2]、[G3]、[G4] | Gemini 官方文档：Interactions、generateContent 与 Live |
| [B1]、[B2] | AWS 官方文档：ConverseStream、API-key 认证与适用限制 |
| [C1] | Cohere 官方 Chat v2 流式参考 |
| [L1]、[L2]、[L3] | Ollama 官方原生 API、NDJSON 和流中错误 |
| [R1]、[R2] | OpenRouter 官方 reasoning 数据与 provider routing 扩展 |
| [Z1] | Azure 官方 v1 API 与认证／部署说明 |
| [S1]、[S2]、[S3]、[S4]、[S5] | OpenCode 网站及固定 dev 源码快照 |
| [S6] | AI SDK provider 架构与自定义适配接口 |
