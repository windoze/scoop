# M46 离线基线覆盖记录

本记录按 [BASELINES.md](BASELINES.md) 的形状矩阵关联普通 Scoop 用例。它不是完整 API client 或新 manifest 入口。生产库只提供协议、Serialize 合成和 JsonValue；基线 DTO 与业务检查留在测试中。四组基线均已完成本机验收；Linux 验收仍待 M46-6。

## Cone 等价 JSON

用例：[m46-baseline-cone](../../tests/fixtures/m46-baselines/cone/fixture.toml)。完整配置及最小配置位于 [data.scoop](../../tests/fixtures/m46-baselines/cone/program/src/data.scoop)，运行断言位于 [main.scoop](../../tests/fixtures/m46-baselines/cone/program/src/main.scoop)，67 个错误输入及期望路径位于 [errors.scoop](../../tests/fixtures/m46-baselines/cone/program/src/errors.scoop)。

| 基线 | 实现及观察 |
| --- | --- |
| T1 | Manifest／Coordinate 的普通合成读取必需字段；strict wrapper 检查未知字段、null；schema 和 kind 由普通代码验证 |
| T2 | Dependency 的字符串／详细对象分支，Dependencies 的动态字符串 key；version、path／artifact 及互斥关系保留，错误类型和未知成员拒绝 |
| T3 | sources 的 None 与 Some(empty array) 独立保存；Selection 的 path／when 使用同一 typed codec |
| T4 | TestConfig 的 default 与空 sources；test-dependencies 与生产依赖使用相同数据模型，重复 coordinate 在测试字段路径报错 |
| T5 | Values 保存字符串或数组分支；Predicate 的 os／arch／env 按 AND／OR 匹配，空数组不匹配任何值；未知键、值与坏元素均拒绝 |
| T6 | Native 的 sources／include／两组 flags／cxx／libraries 使用普通 default；输出后重新 typed 解码保持结果 |
| T7 | Library 覆盖 default／dynamic／static／framework，name／kind／when 有各自检查；平台条件复用 Predicate |
| T8 | 普通校验函数检查 coordinate／SemVer、相对路径、locator 冲突、重复依赖、库名与 native 参数；保持原始 argv 数据，不访问文件系统或执行 native 编译／链接 |
| T9 | 每个固定表先拒绝已出现的 null，随后复用合成字段 codec；缺失用 default／None，显式空数组保持 Some(empty)，不把 null 当成省略 |

字段包装器先枚举并检查该表的输入，再将同一 decoder 交给普通合成 reader。这样直接验证稳定视图与 default 的组合，无需复制合成实现或使用 JsonValue 保存整份 manifest。文本样例直接表达 spec 的等价 JSON，没有模拟 TOML 文本替换、源码 span 或构建器。

## Chat Completions

用例：[m46-baseline-chat](../../tests/fixtures/m46-baselines/chat/fixture.toml)。[真实形状输入](../../tests/fixtures/m46-baselines/chat/program/src/chat_data.scoop)和[typed 断言／错误矩阵](../../tests/fixtures/m46-baselines/chat/program/src/chat_test.scoop)覆盖下表；普通模式运行全部往返和 12 个错误路径，moving GC 保留完整多模态请求的解码、编码和 GC 组合。

| 基线 | 实现及观察 |
| --- | --- |
| C1 | ChatMessage 分 developer／system／user／assistant／tool／function；文本短式和块数组由 ChatContent 保存。前两类普通校验只接受文本块，tool 必须有 tool_call_id，旧 function 的 content 必需但可 null，assistant 可以不带 content |
| C2 | ChatPart 分 text／image_url／input_audio／file／refusal；各分支委托合成 payload。样例交换 type 的顺序，保留 URL、base64、file_id／file_data 和 JSON 字符串 |
| C3 | ChatTool 使用嵌套 function／custom；ChatChoice 保留字符串或具名工具／allowed_tools 对象。函数调用 arguments 与 custom input 始终为字符串，错误对象值在原路径拒绝 |
| C4 | text／json_object／json_schema 分支，custom format 的 text／grammar 分支。Schema 的递归 properties、required、additionalProperties、null／布尔／数组及超 ULong 整数经 JsonValue 保留 |
| C5 | Strings 与 Numbers 分别以 String、Double 读取动态 object；错误 metadata／logit_bias value 在 key 路径拒绝。nullable temperature 按普通 Option 归并，stream_options 是 typed record |
| C6 | ChatCompletion、choice、message、tool calls、usage 细分、logprobs／bytes／top_logprobs、url annotation 均为 typed DTO；required nullable 字段保留显式 null。未知扩展包含四种精确数字，并通过合成固定字段与扩展 map 共同输出 |
| C7 | ChatChunk 使用独立 ChatDelta／CallDelta／FunctionDelta，不要求完整消息字段；五个 chunk 包括 role、两段 arguments、结束及空 choices 的 usage。字符串片段拼接后才解码 Location |
| C8 | ErrorEnvelope 是正常 typed 数据；CompletionReader 的 allow／deny／collect 均运行，错误 id 不因 collect 而放宽。未知 key 路径包含 JSON Pointer 转义；未知 role、已知坏 payload、缺失 required nullable 均失败 |

字段形状核对依据 [官方 Create 定义](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)（2026-10-14）。sampling／token limit／penalty 等数值、开关布尔值、模型及标识字符串、modalities 等标量数组归入 C5 的普通字段族；audio／prediction／stream options／缓存配置归入 C1～C4 的固定 record 与字符串／块数组族；旧 functions／function_call 归入函数定义及字符串／具名对象族。它们不增加新的 codec 机制，不为相同形状逐个复制 DTO 字段。请求、响应和 delta 保持独立 required 集合，未用一个全可选 record 合并所有分支。

## Responses

用例：[m46-baseline-responses](../../tests/fixtures/m46-baselines/responses/fixture.toml)。[请求／响应／事件输入](../../tests/fixtures/m46-baselines/responses/program/src/response_data.scoop)、[其他 item 形状](../../tests/fixtures/m46-baselines/responses/program/src/response_extra_data.scoop)、[typed 断言](../../tests/fixtures/m46-baselines/responses/program/src/response_test.scoop)和[12 个错误路径](../../tests/fixtures/m46-baselines/responses/program/src/response_failures.scoop)共同验证；moving GC 使用带函数 schema、简式消息、function call 和 metadata 的代表请求。

| 基线 | 实现及观察 |
| --- | --- |
| R1 | ResponseInput 保存字符串或数组；ResponseContent 保存字符串或块数组。ResponseItem 分无 type 的 Easy、显式 message 输入及带 id 的完整输出，同一 tag 再按可观察字段选择，输出 status 缺失不被当作未知扩展 |
| R2 | 混合序列的 15 个 item 覆盖 message、reasoning、function call／result、file／web search、computer call／screenshot、image generation、code interpreter、MCP call／list／approval／decision、compaction。output 数组重新作为同一 typed item 序列读取；opaque／encrypted 字符串和顺序保持 |
| R3 | FunctionSpec 的 type、name、parameters 直接同层输出；7 类工具分别读取专用 payload。Choice 保留模式字符串或具名对象；computer action、search action、code output、MCP error 还有独立的内部 discriminator |
| R4 | text.format 和 function parameters 的开放 schema 使用 JsonValue；PromptVariables 是动态 key 的 PromptValue 字符串／image／file union，已知 image_url 类型错误按完整路径拒绝 |
| R5 | conversation 的字符串／id 对象／null，input 短式与显式空数组，metadata 字符串 map 均运行。应用 DTO 的 instructions 带非 None default，显式 null 仍为 None 并写 null，错误数字不能回退为 default |
| R6 | ResponseBody 的 status／error／incomplete_details／usage／output 为 typed 字段；created／failed／incomplete／completed 状态作为正常数据读取。required nullable error／usage 保留 null；MCP protocol／HTTP 错误按 code／message 读取，只有 execution error 的开放 content 使用 JsonValue |
| R7 | 16 个完整事件覆盖 created／in_progress／completed／failed／incomplete、item／content 添加和结束、文本 delta／done、两段函数参数 delta／done、error 及未知扩展。已知坏 delta 或 error 失败，不进入 Unknown；未知事件原树保留；片段拼接后才读取 Location |

实施核对使用可访问的[官方 Responses Create CLI schema](https://developers.openai.com/api/reference/cli/resources/responses/methods/create)、[Responses 类型目录及展开 schema](https://developers.openai.com/api/reference/cli/resources/responses)和[流事件指南](https://developers.openai.com/api/docs/guides/streaming-responses)（2026-10-14）；HTTP Create 页面本次抓取失败，CLI reference 描述同一 `/responses` wire schema。其余工具版本／action／annotation／音频及进度事件映射到已经运行的 tag＋固定 payload、嵌套 tag、标量数组和可空 record 族；namespace、shell／apply_patch、tool permissions、container policy 等组合亦不引入不同 JSON 形状。没有把已知结构整体存为 JsonValue，也没有扩成 Responses SDK。

## Anthropic Messages

用例：[m46-baseline-messages](../../tests/fixtures/m46-baselines/messages/fixture.toml)。[请求／响应／完整事件](../../tests/fixtures/m46-baselines/messages/program/src/message_data.scoop)、[其他内容分支](../../tests/fixtures/m46-baselines/messages/program/src/message_extra_data.scoop)、[typed 断言](../../tests/fixtures/m46-baselines/messages/program/src/message_test.scoop)和[15 个错误路径](../../tests/fixtures/m46-baselines/messages/program/src/message_failures.scoop)共同验证；moving GC 使用带图像、任意工具 input 及 schema 的代表请求。

| 基线 | 实现及观察 |
| --- | --- |
| A1 | Content 保留消息字符串／Block 数组，SystemContent 独立保留字符串／TextOnly 数组；system 错误 type 拒绝，不能借通用内容 union 接受非文本块 |
| A2 | Block 的 text／image／document／thinking／redacted／tool use／tool result／server use／search／fetch／code result／search result／upload 家族均 typed。server_tool_use 先按 type，再在相同对象按 name 分派；五类 Citation、signature、encrypted_content／encrypted_stdout 均往返 |
| A3 | Source 的 base64／URL／text／content 分支及文档内递归块数组；media_type、data、url 均为正常字符串，错误 data／url 拒绝，不执行二进制或文档解析 |
| A4 | UserTool.input_schema 与 ToolUse.input 是开放 JSON；三类 ServerCall 的 input 分别为 query／url／code record，已知坏 query 不进入 Unknown。ToolResult 的字符串／递归数组，以及 server 搜索数组或错误、fetch／code 的结果或错误均运行 |
| A5 | 7 个普通／专有工具配置，auto／any／none／tool choice，enabled／disabled／adaptive thinking，cache_control、output_config、container 字符串／对象／null 和 skills 为独立 typed 组合 |
| A6 | MessageBody 独立 required 集合，stop_reason／stop_sequence 必需但可 null；Usage／container／diagnostics typed。CacheMiss 六分支与 null 独立往返，错误字符串或计数失败。ExtendedCodec 保留未知字段中的精确数字，输出后再 typed 解码保持结果 |
| A7 | 14 个完整事件包含 message start／delta／stop、block start／delta／stop、ping、error 和未知扩展；错误 envelope 为普通数据，未知完整事件可原树保存 |
| A8 | Delta 独立分 text／input_json／thinking／signature／citations；两段 partial_json 各自只作为字符串，拼接后读取 Location；signature 保持原内容，已知坏 partial_json／citation 保留嵌套路径 |

依据[官方 Create 定义](https://platform.claude.com/docs/en/api/messages/create)、[Messages 类型目录](https://platform.claude.com/docs/en/api/typescript/messages)、[Streaming Messages](https://platform.claude.com/docs/en/build-with-claude/streaming)及[cache diagnostics](https://platform.claude.com/docs/en/build-with-claude/cache-diagnostics)于 2026-10-14～15 核对。其余版本化工具、bash／text editor／tool search 结果、caller／tool reference、browser／computer toolset 配置属于已经运行的同层 tag、嵌套 tag、固定 record、typed 数组与可缺省字段族；数值采样参数、service tier、stop_sequences 等复用标量／数组形状。新增版本名不进入 core，不把这些家族当成网络执行器。复核时将 diagnostics 的 nullable 字段从错误的 String 草稿修正为 CacheMiss tagged union，增加了非 null 正负例，避免仅用 null 样例掩盖已知结构。

## B1～B16 组合证据

| 组合 | 直接证据 |
| --- | --- |
| B1 | [scalars](../../tests/fixtures/m46-decoding/scalars/program/src/main.scoop)、[records](../../tests/fixtures/m46-decoding/records/program/src/main.scoop)：null 探测、错误类型后复读及重复字段 |
| B2 | [decoding composition](../../tests/fixtures/m46-decoding/composition/program/src/main.scoop)：结构候选部分读取后重试及 default 次数；API 的短长式为真实组合 |
| B3 | [sequences](../../tests/fixtures/m46-decoding/sequences/program/src/main.scoop)：独立交错游标和保留 child 重试 |
| B4 | [encoding composition](../../tests/fixtures/m46-encoding/composition/program/src/main.scoop)及三 API tag／payload：共享同层容器、合成 end 后继续组合 |
| B5 | [values composition](../../tests/fixtures/m46-values/composition/program/src/main.scoop)及 Chat CompletionReader：allow／deny／collect 与重复 key 冲突 |
| B6 | 三 API 请求与 mixed 内容／item／事件；明确 tag 分支错误不回退 Unknown |
| B7 | [numbers](../../tests/fixtures/m46-values/numbers/program/src/main.scoop)、Chat／Messages 未知扩展：四种精确数字及递归值 |
| B8 | numbers 与 [values](../../tests/fixtures/m46-values/values/program/src/main.scoop)：受限数值失败后捕获、decodeValue 新根与延迟 typed 读取 |
| B9 | [encoding errors](../../tests/fixtures/m46-encoding/errors/program/src/main.scoop)、[committed](../../tests/fixtures/m46-encoding/committed/program/src/main.scoop)：pending、追加、过期 writer |
| B10 | [values errors](../../tests/fixtures/m46-values/errors/program/src/main.scoop)：非法未知子树及完整 parser 错误；M29 JSON 原规则回归 |
| B11 | Cone 的 67 个错误输入、全部 T1～T9 及准确路径 |
| B12 | Serialize／JsonValue 的三个 Cone [产物消费](../../tests/fixtures/m46-values/artifacts/fixture.toml)、保留 decoder／视图及 moving GC；各运行型新增 fixture 均有搬迁模式 |
| B13 | [fields](../../tests/fixtures/m46-serialize/fields/program/src/main.scoop)、[shapes](../../tests/fixtures/m46-serialize/shapes/program/src/main.scoop)、API tag／flatten：rename、required、emitNull、ignore、Option |
| B14 | [defaults](../../tests/fixtures/m46-serialize/defaults/program/src/main.scoop)与 [option](../../tests/fixtures/m46-serialize/option/program/src/main.scoop)：None／非 None／动态 default 及容器位置 |
| B15 | [codecs](../../tests/fixtures/m46-serialize/codecs/program/src/main.scoop)、[generics](../../tests/fixtures/m46-serialize/generics/program/src/main.scoop)、[无源码 Serialize](../../tests/fixtures/m46-serialize/artifacts/fixture.toml)：注入、声明处 T／T?、alias、重导出 |
| B16 | [three-state](../../tests/fixtures/m46-serialize/three-state/program/src/main.scoop)：用户 FieldState 的 Missing／Null／Value，无 core 扩展 |

矩阵关联本机已完成的分批测试，不表示 Linux 已通过。每批原始报告与失败修正记录均见 [PROGRESS.md](PROGRESS.md)；不同批次复用构建缓存，不反复跑全量。
