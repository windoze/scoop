# ConverseStream 扩展规范

本规范补充 [LLM client 第 26 节](SCOOP-LLM-CLIENT-SPEC.md#26-conversestream)。ConverseProtocol 使用既有 [AWS EventStream decoder/source](SCOOP-HTTP-EVENTSTREAM-SPEC.md)，通过相同的 client.openStream／withStream 入口运行，不增加独立的简易 client。

## 1. 传输与结束

POST 地址的末段为 converse-stream；模型 ID 仍是一个编码后的路径段，查询参数保留。正文与普通 Converse 生成共用编码，省略 stream 字段。成功响应媒体类型为 application/vnd.amazon.eventstream，body 字节限制包含二进制定界。SSE 与 EventStream 共用尝试、认证、重试、按需读取、计时及完成回调；仅定界 source 和纯 decoder 不同。

LlmEventStreamDecoder.accept(message) 接收已验证 CRC 的消息。`:message-type=event` 的非空 `:event-type` 选择业务事件，payload 必须是严格 UTF-8 JSON object；可选 `:content-type` 必须为 application/json。exception／error 消息终止调用，保留可读取的错误内容，不取得重试资格。其余 header 以显式类型和值保留为 Native；未知业务事件也保留原文。缺失或非文本的控制 header、非法 UTF-8／JSON、非法事件顺序明确失败，不解释为普通文本。

messageStart 只出现一次，role 必须为 assistant。contentBlockIndex 从零连续建立，后续事件只能引用对应未停止的 block；允许不同 block 交错。toolUse、toolResult、image 使用 start；text、reasoningContent、citation 可以由第一条 delta 建立 block。block 的已知内容种类不能中途替换，文本 block 可以附加 citation。未知增量保留 Native，不推测其状态转换；已建立的文本或 reasoning 仍可继续交付已知片段，但不再提供可回传历史。空 start 没有可判断的 union 类型，明确拒绝。

contentBlockStop 完成对应 block；messageStop 要求全部 block 已停止，保存实际 stopReason 并交付 CandidateFinished。metadata 保存累计 usage、metrics 与其他服务字段，可以在 messageStop 前后到达；不把 latencyMs 当作客户端耗时。只有 messageStart、messageStop 和正常 EOF 才确认整个流结束，usage 此时为 Final；缺少 metadata 时计数保持未知。messageStop 后的服务错误或损坏帧仍失败。重复停止、内容在 messageStop 后到达、缺少 messageStop 或残缺帧不能产生成功结果。

## 2. 内容、位置与历史

每个 wire block 有稳定的 item 位置；连续普通文本 block 进入同一 Message 的连续 part，事件和 Result 的位置一致。text 与 reasoning 文本按片段顺序交付；只有 Result 累积普通文本。reasoningContent.text 和 signature 分别拼接，签名原样回传；redactedContent 与 image source.bytes 的每个 Base64 片段先解码为字节，再合并，不能直接连接带 padding 的 Base64 文本。redacted 与明文 reasoning 不混用。已知图片在 Result 中投影为媒体，流式通过 Native 交付原始增量；未知 source／error 保留 Native，无法无损归并时不提供 history。

citation 以当前 block 的文本为关联目标；独立 citation block 可连接到前一个文本 block，连续独立引用保持同一关联。每条引用保留原始对象，已有 URL／原生来源映射复用普通 Converse 规则；文档位置不是生成文本范围。不带可关联文本的引用保留 Native。Result 将已知引用组合为 citationsContent，未知事件／字段仍保存在 nativeEvents；不能推测如何回传的内容增量使 history 不可用。

Result 保留完整候选和可恢复的 assistant message，不冒充一次完整服务 JSON 响应：response.native 为 None，服务元数据和必要扩展位于 nativeEvents。没有模型名、响应 ID 或服务创建时间时保持 None。未知停止原因沿普通 Converse 的 Incomplete／Other 语义且不提供 history。EventsOnly 不保留普通文本、图片正文或完整原始事件日志，仍完成必要的工具参数聚合和 usage 观察。

## 3. 客户端与服务端工具

toolUse 的实际 ID、name、type 在 start 建立，ID 不得重复；调用归属复用普通 Converse 的请求工具绑定。input 是 JSON 文本片段，不在片段阶段执行。已停止的完整调用只有 messageStop=tool_use、绑定唯一且未收到结果时才形成可执行客户端调用；无效 JSON／非 object 分别保留 Invalid，其他停止原因保留 Incomplete。无法解析的参数不伪造可回传 JSON，history 不可用。

server_tool_use 在开始时交付 InProgress 活动，停止时携带已解析的输入；toolResult start 与其 ID 关联，delta 的 text／json 按实际顺序保存，停止时根据 status 交付 Completed／Failed／Unknown。工具类型、来源与执行方的校验复用普通 Converse 的规则。服务端活动不进入客户端执行列表，已收到结果的客户端调用也不再次执行。未知类型保留 Native；声明工具本身不生成执行活动。

依据：[ConverseStream API](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html)、[事件生命周期](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html)、[内容增量 union](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlockDelta.html)、[官方 SDK service model](https://github.com/boto/botocore/blob/develop/botocore/data/bedrock-runtime/2023-09-30/service-2.json)。参考 [Vercel Bedrock 流式适配](https://github.com/vercel/ai/blob/39aae5425e9f9a25acce413cc7e6823c2b5a73c5/packages/amazon-bedrock/src/amazon-bedrock-chat-language-model.ts)的内容位置及结束后汇总方式；按本库的显式工具归属、未知值和历史合同实现。

验证使用所属 Cone 的 scoop.test 与 loopback mock，覆盖二进制输入、跨事件聚合、错误／EOF、usage、时间戳与历史接续。真实服务用例保持默认跳过；不使用编译器 fixture。
