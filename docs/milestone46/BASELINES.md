# M46 基线与验收：API wire format 与 Cone.toml

状态：Cone 等价 JSON 已完成本机验收；三组 API 待实施，Linux 完整验收待 M46-6。日期：2026-10-14。协议与范围以[主设计](DESIGN.md)及[统一 Serialize](SERIALIZE.md)为准；本文列的是实施验收要求，不是已经通过的测试记录。

## 1. “完整支持”的含义

基线要求增强后的基础能够表达、读取和写出参考格式的全部数据形状，允许普通手写 codec 和业务 DTO。普通 record 的重命名、忽略及可缺省／可空字段优先直接使用 Serialize 与合成，不要求额外的三态 enum。采用两类证据：对参考 schema 的字段／联合分支作形状归类，以及使用真实形状的代表性 DTO 完成离线编码、解码和组合测试。相同形状可以复用测试，不为每个标量字段复制一份用例。

已有明确结构的消息、内容块、工具调用和配置字段必须有 typed 解码及错误检查；`JsonValue` 用于 schema 本来允许任意 JSON 的位置，以及应用选择保留的未知字段／未知扩展分支。仅对整个请求／响应做 JsonValue 文本往返，不能证明本基线通过。

wire 往返比较类型、数组顺序、字符串内容与数字；字段存在性和 null 按 DTO 声明的合同检查。普通 Option 字段允许把缺失与 null 归为 None，再按 required／emitNull／default 输出，不承诺逐字保留输入的字段存在性。schema 确实区分这些状态、或测试要求精确保留原 wire 时，使用协议查询及适当的 JsonValue 字段／对象保存信息；不能用默认归并通过这类验收。对象成员顺序不决定 API 语义；不要求恢复原始空白或转义拼写。输入 schema 允许的短式／长式可以由业务 DTO 选择一种规范输出，但需要保留的短长式差异应以普通 enum 保存，不能被协议层强制折叠。

具体模型标识、工具版本标识和 API 业务限制不进入 core。数据来源按本次调研日期记录；实施时对照引用页面记录新增形状或变化，不以在线文档的无限增长作为无法完成里程碑的理由。

## 2. 共同形状与基础操作

| 形状 | 实际出现位置 | 基础操作与责任 |
| --- | --- | --- |
| 固定字段 record、嵌套 record | 消息、内容块、usage、manifest 各配置表 | keyed 子入口，Serialize 字段合成及现有 codec 组合 |
| 缺失／显式 null／有值 | API 的可选请求参数、响应的可空字段；manifest 的缺省字段 | `optional` 与 `isNull`；父合成／手写 codec 决定是否输出字段；DTO 归并与原 wire 保留分别验证 |
| 字符串或数组／对象 | message content、Responses input、tool choice；dependency 和 `when` | 同一值尝试多个普通 codec，或按可观察字段选择分支 |
| tag 与 payload 同层 | `type`／`role` 的内容块、工具、事件 | 读取 tag 后再次对同一值解码；编码共享 keyed 状态 |
| tag 与 payload 分层 | Chat 的 `type`＋`function`，流事件的 `type`＋`delta` | 现有 field 子入口加嵌套 codec |
| 同一 tag 下继续分类 | 工具家族及按 name／action 区分的 payload | 多次读取字段、普通控制流和结构候选；不假设一个 tag 永远对应一个唯一结构 |
| 异构递归序列 | 多模态 content、Responses input／output、嵌套工具结果 | unkeyed 视图、逐元素 codec；失败重试复用已取得的 child |
| 动态字符串 key | OpenAI metadata／logit bias、JSON Schema properties、dependencies | `keys`＋`required`；由普通 codec 构造 object 形式的 map |
| 任意递归 JSON | 工具 schema／input、开放扩展字段 | `JsonValue` 子值 codec，保留数字 token；不经 Any／Double 中转 |
| JSON 文本作为字符串 | OpenAI function arguments、Anthropic partial_json | 字符串 codec；是否进一步解析由应用决定 |
| 严格未知字段与组合约束 | Cone manifest；应用自己的严格 DTO | 枚举 key、检查允许集合及普通业务校验；`end` 不代替这些规则 |
| 多个 codec 写入同一对象 | tagged payload、固定字段与扩展 map、flatten | 共享 keyed encoder，重复 key 冲突在写入处报错 |

以上是序列化结构的分类。Serialize 本期提供单字段名称映射、忽略、required 和 emitNull；Option 默认 codec 及字段合成直接承载普通可空 DTO。日期文本／时间戳转换仍由普通 codec 表达，字段 codec 的 annotation 绑定留待后续。

## 3. OpenAI Chat Completions

参考范围为 `/v1/chat/completions` 的请求、普通响应及流式 chunk；按下表核对官方 Create／消息类型／streaming 定义中的全部形状。

| 编号 | 必须覆盖的 wire 情况 | 核心断言 |
| --- | --- | --- |
| C1 | `messages` 的 role 分支；content 的字符串、多模态块数组及 schema 允许的缺失／null | 不给所有 role 强套同一 required 字段集合；未知 role 的处理由 codec 明确决定 |
| C2 | 文本、image_url、input_audio、file 等内容分支；内部 URL／base64／文件标识与文本字段 | tag 可以位于对象开头或结尾；字符串不被错误解析为对象或二进制 |
| C3 | `tools` 中 type 与嵌套 function；`tool_choice` 的字符串和对象形式 | 编码保持实际嵌套关系，不能用 Scoop 默认 enum 外层形式替换 |
| C4 | `response_format` 的分支；function parameters 和 JSON Schema 的 properties／required／additionalProperties 等 | schema 中的递归对象、数组、布尔值、null、数字及动态 key 均可保留 |
| C5 | `metadata` 的字符串值 map、`logit_bias` 的数值 map，以及普通可选／可空参数 | 不把 metadata 泛化成任意 JSON 值；不同 map 按自己的 value codec 检查 |
| C6 | completion 的 choices／message／tool_calls／usage／logprobs／annotations，以及可空字段 | 已知字段具有 typed 读取；扩展字段可以收集并在重编码时保留 |
| C7 | chunk 的 delta 部分字段、tool call arguments 字符串片段、可空 finish_reason／usage、空 choices | chunk 不要求包含完整 message 的必需字段；末次 usage 与普通 delta 分别处理 |
| C8 | JSON 错误 envelope，以及客户端选择忽略、拒绝或保留未知字段的三种 codec | 格式错误、已知分支字段错误和 API 返回的错误数据不是同一件事 |

例如 Chat function tool 的 wire 片段是：

```json
{
  "type": "function",
  "function": {
    "name": "lookup",
    "parameters": {
      "type": "object",
      "properties": {"id": {"type": "string"}},
      "required": ["id"],
      "additionalProperties": false
    }
  }
}
```

验收按实际字段嵌套判定，不把文档使用的“externally tagged”措辞直接等同于 serde 的某一种固定表示。

## 4. OpenAI Responses

参考范围为 `/v1/responses` 的请求、Response 对象及完整流事件。基线包括普通文本之外的工具、推理、多模态和保留上下文所需数据。

| 编号 | 必须覆盖的 wire 情况 | 核心断言 |
| --- | --- | --- |
| R1 | `input` 的字符串或 item 数组；消息 content 的短式／块数组；schema 中省略 type 的简式消息 | 不强制所有 union 都有 tag；缺失 tag 的简式由普通 codec 处理 |
| R2 | input／output 的 message、reasoning、function call／result、文件／图像／计算机／搜索／代码执行／MCP 等 item 家族 | 区分各分支，混合序列保持顺序；重传 output item 不能只保留其中的文本 |
| R3 | function tool 的 type、name、parameters 位于同层；内置工具各自的 payload；tool_choice 的字符串／对象形式 | 通过共享 keyed encoder 组合 tag 和字段，与 C3 的嵌套形式区别明确 |
| R4 | `text.format`／tool parameters 的 schema，prompt variables 等动态成员 | 任意值只用于 schema 允许的位置；已知结构仍按 typed codec 读取 |
| R5 | conversation 的字符串／对象／null，instructions／previous_response_id 等缺失或可空参数，metadata 字符串 map | 保留所选 union 分支，字段存在性按 DTO／schema 的合同检查；显式 null 不触发 default，类型错误不能被当成缺省 |
| R6 | Response 的 status、error、incomplete_details、usage、output；reasoning 的 opaque／encrypted 字符串 | 错误和未完成状态能作为合法数据解码；不对 opaque 字符串作内容转换 |
| R7 | `response.created`、item／content 添加与结束、文本／函数参数 delta、完成／失败／未完成以及 error 事件 | 完整事件的不同 payload 由 `type` 分派；新增扩展事件可以保留原始子树 |

与上节对应的 Responses function tool 片段为：

```json
{
  "type": "function",
  "name": "lookup",
  "parameters": {
    "type": "object",
    "properties": {"id": {"type": "string"}},
    "required": ["id"],
    "additionalProperties": false
  }
}
```

同一 payload codec 可以被外围手写 codec 用于这两种嵌套位置。Responses 的同层组合必须能直接使用共享容器，不能先生成文本再剥掉大括号或复制合成实现。

## 5. Anthropic Messages

参考范围为 `/v1/messages` 的请求、Message 对象及流事件。Messages 中列出的工具和内容家族按结构分类，普通 tool input 的开放性与具体内置工具的结构约束分开处理。

| 编号 | 必须覆盖的 wire 情况 | 核心断言 |
| --- | --- | --- |
| A1 | messages 中的字符串／content block 数组；system 的字符串／TextBlockParam 数组 | 相同逻辑内容的短长式均可解码和按 DTO 选择编码 |
| A2 | text、image、document、thinking、redacted_thinking、tool_use／tool_result、server tool 与结果、citations 等内容家族 | 多层 discriminator 与递归内容由普通 codec 组合；保留 signature／opaque 字符串 |
| A3 | image／document source 的 base64、URL 等分支及其 media_type、data、url 字段 | data 仍是 wire 字符串；不引入二进制 decoder 特判 |
| A4 | `tools[].input_schema`、`tool_use.input`、tool result 的字符串／结构化内容；普通工具与专有工具配置 | 任意 JSON 输入能无损保存；已知结构错误不能自动转为未知工具 |
| A5 | tool_choice、thinking、cache_control、output_config、container 等配置分支 | 可空、字符串／对象及不同层级的 tag 均可表达；模型／版本值不进入 core |
| A6 | Message 的 content、stop_reason／stop_sequence、usage、container／diagnostics 等可空或扩展结构 | 可空及 required 字段按声明读写；原 wire 保留用例另行检查存在性，未知扩展数据可选保留，已知字段按类型检查 |
| A7 | message_start／delta／stop，content_block_start／delta／stop，ping 和 error | envelope 与嵌套 delta 分别分派；完整事件能独立解码 |
| A8 | text_delta、input_json_delta、thinking_delta、signature_delta、citations 等 delta 家族 | `partial_json` 是字符串片段，不能要求每个片段自身是合法 JSON |

API stream 示例中，下面是一个完整且合法的 JSON 事件，即使 `partial_json` 的值只包含半个工具参数对象：

```json
{
  "type": "content_block_delta",
  "index": 1,
  "delta": {
    "type": "input_json_delta",
    "partial_json": "{\"location\":"
  }
}
```

### 5.1 三组 API 的传输边界

M46 处理请求／响应 body 和拆帧后的完整 JSON 事件。SSE 的 event／data 行、Chat 的 `[DONE]` 标记、网络字节分块、WebSocket 帧和增量拼接由上层负责，不传给 `Json.decode` 当作单个 JSON 文档。

离线用例必须保留 JSON 事件及其中的字符串片段；可以在测试中按既定顺序拼接片段，再对完整参数文本调用 JSON 解码，证明没有错误的强制转换。不实现通用 SSE parser、网络重试或 SDK 流式状态机。

## 6. Cone.toml 的等价 JSON

此基线覆盖语言规范 12.2～12.2.4 的 schema 1 数据模型，包含生产／测试配置和 native 配置。TOML 表对应 JSON object，表数组对应 JSON array；字符串、整数和布尔值保持类型，字段缺失保持缺失。

这是序列化能力的测试表示，不增加 `Cone.json` 构建入口。TOML 特有的语法、注释、表声明顺序、日期字面量和源码 span 不由等价转换模拟；当前 Cone schema 没有要求用 TOML 日期值表示的字段。原编译器继续使用现有 TOML parser 及诊断定位。

### 6.1 覆盖全部配置族的样例

下列路径仅作为 manifest 数据，不要求测试机器上存在对应的依赖、源码或库：

```json
{
  "schema": 1,
  "cone": {
    "group": "dev.example",
    "name": "wire-demo",
    "version": "0.1.0",
    "kind": "library"
  },
  "dependencies": {
    "org.foo:bar": {"version": "1.2.3", "path": "../bar"},
    "org.acme:util": {"version": "2.0.0", "artifact": "../artifacts/util.slib"},
    "org.other:log": "3.1.0"
  },
  "sources": [
    {"path": "src/common"},
    {"path": "src/linux", "when": {"os": "linux", "arch": ["aarch64", "x86_64"]}}
  ],
  "test": {
    "sources": [{"path": "tests/common"}]
  },
  "test-dependencies": {
    "scoop:scoop.test": "0.1.0"
  },
  "native": {
    "sources": [
      {"path": "native/common.c"},
      {"path": "native/linux.c", "when": {"os": "linux", "env": ["gnu", "musl"]}}
    ],
    "include": ["native/include"],
    "c_flags": ["-DDEBUG=1"],
    "cxx_flags": ["-std=c++20"],
    "cxx": true,
    "libraries": [
      {"name": "z", "kind": "default"},
      {"name": "Security", "kind": "framework", "when": {"os": "darwin"}}
    ]
  }
}
```

### 6.2 字段与规则矩阵

| 编号 | 范围 | 必须保留或检查 |
| --- | --- | --- |
| T1 | 顶层及 cone | schema 整数 1；group／name／version／kind 必需；kind 的 library／executable 字符串分支；逐层拒绝未知字段 |
| T2 | dependencies／test-dependencies | 动态 `group:name` key；value 为 version 字符串或详细对象；详细对象要求 version，path／artifact 至多一个；拒绝未知成员和错误类型 |
| T3 | 生产 sources | 缺失表示默认 src，显式数组表示完整选择清单，空数组不能回退成缺省；条目要求 path，when 可省略 |
| T4 | test 与 test.sources | test 缺失及 sources 缺失保留默认 tests；显式空数组关闭测试源码选择；test 表内未知字段报错 |
| T5 | when | 只允许 os／arch／env；每个值是字符串或字符串数组；多个 key 为 AND，同一 key 数组为 OR；空数组表示没有可匹配值，不回退成无条件 |
| T6 | native | sources／include／c_flags／cxx_flags／libraries 默认空数组，cxx 默认 false；各字段类型准确；native.sources 使用同一 path／when 形状 |
| T7 | native.libraries | name 必需；kind 缺失或 default、dynamic、static、framework；when 沿用相同谓词；拒绝未知成员 |
| T8 | 跨字段业务规则 | 依赖 coordinate／version 语法、locator 冲突、生产与测试重复依赖、路径形式、target 值、native flag／library 名的合法性由普通校验函数处理 |
| T9 | null 与 default | 等价数据中显式 null 不是 TOML 的“省略字段”；不能把 null 解码为 default、缺失 sources 或空配置 |

T8 的意义是解码后的 DTO 完整保留现有 manifest 语义所需信息，并可在普通代码中校验；不把 coordinate、文件系统枚举、路径存在性、依赖解析或 native linking 规则搬进 core／JSON。已有 Rust manifest 实现是规则参考，不在 M46 中移植整套构建器。

T9 不采用普通 API DTO 的 missing／null 归并。非可空且带 default 的字段直接由合成实现；以 Option 保存“未提供”的配置字段在普通 manifest codec 中检查 child 的 isNull 并拒绝，再读取值。两种方式均不要求三态 enum；显式空数组继续与缺失分别处理。

测试转换可以使用宿主现有 TOML parser 取得逻辑值，再按上述规则形成等价 JSON；不能用正则替换 TOML 文本。缺失、显式空数组、短式／长式和字符串／整数区别必须保留。至少包含最小 manifest、上述完整配置、各类缺省／空配置以及逐层未知字段失败。

## 7. 关键组合与失败验收

| 编号 | 用例 | 必须观察到的结果 |
| --- | --- | --- |
| B1 | 同一 scalar 先探测 null，再以错误类型读取，最后以正确类型读取；同一字段重复查询和解码 | 最终成功且数据路径一致；错误读取不锁定视图 |
| B2 | 第一结构候选已读若干字段／嵌套元素后失败，第二候选从同一值成功 | 不出现“读了两次”“子值未完成”等来自旧候选的状态错误 |
| B3 | 同一序列两个视图交错读取；一个元素的两个候选共享同一 child | 两游标独立；父游标只在 element 成功取得时推进一次 |
| B4 | 外围手写 tag codec 与内部合成 payload codec 共同编解码；交换输入字段顺序 | 合成 codec 的 end 不阻止后续组合；读取 tag 不消耗 payload 字段 |
| B5 | 两个部分 flatten 加一个未知字段 map，分别 allow／deny／collect | 固定字段正确；collect 能保留任意子值；冲突 key 报错，deny 以组合允许集合检查 |
| B6 | API 请求中的可缺省／可空字段、短长式、同层／分层 tag，与混合 content／output 序列同时出现 | DTO 合成输出符合 Serialize 规则；要求原 wire 保留的用例仍保留字段存在性；已知坏 payload 不能进入 Unknown 逃避错误 |
| B7 | 在未知字段／工具 schema 内保存 `18446744073709551616`、`0.12345678901234567890123456789`、`1e400`、`-0`，及嵌套数组／对象 | 作为 JsonValue 捕获和再次输出不改变数字 token；按受限数值 codec 读取仍按既有规则检查／舍入 |
| B8 | 先以 Long 或 Double 解码越界值失败，再以 JsonValue 保存；对子值用 decodeValue 延迟解释 | 重试成功，不 stringify／parse 中转；新根解码路径从空串开始 |
| B9 | 编码 end 后追加未结束字段，跨视图留下未完成子值，父提交后继续修改子 encoder | 均按主设计的具体边界报错，旧完成标记和视图别名不能隐藏错误 |
| B10 | 非法未知子树、重复对象 key、尾随第二个根值、坏 Unicode／转义 | 即使业务 codec 完全不访问出错字段，JSON 文档仍失败 |
| B11 | Cone 的所有固定表逐层注入拼错 key、缺少必需值、错误类型、显式 null；path／artifact 同时出现；依赖重复 | 对应层拒绝，路径指向出错字段；不改用宽松未知字段策略通过测试 |
| B12 | 容器／子 decoder／JsonValue 被保存，期间发生正常 GC；手写与合成 codec 经普通 slib 被消费 | 托管引用及跨 Cone typed 调用正确，无特殊 native 保活或反射入口 |
| B13 | 普通 DTO 的 rename、Option 缺失／null／Some、required、emitNull、ignore 与手写 tag／flatten 组合 | 不需要 presence enum；None 省略发生在创建 child 之前；required 可空字段仍必须出现 |
| B14 | Option 的非 None default、直接 None default、动态 default，以及数组／tuple 中的 Option | 编码不执行 default，None 不会往返成非 None default；数组 None 保留位置并写 null |
| B15 | 相同 DTO 的完整 Option codec 注入、定义处 T／T?、alias，以及无源码消费 Serialize | 显式 codec 优先，字段规则不在实例化时重选；按真实 core／注解 identity 处理 |
| B16 | 应用自定义 FieldState enum 的 PATCH DTO，经普通父 codec／字段 helper 处理缺失、null、字符串值及非 null record 值 | Missing／Null／Value 分别往返；Missing 不创建 field，Missing／Null 不调用元素 codec；结构值复用合成 codec，已知错误值保留路径；无需修改 core／JSON |

输入字段含 `~`、`/` 时，继续断言现有 JSON Pointer segment 转义。untagged 失败的最终错误由示例 codec 决定，但各候选错误必须保持自身实际路径；不新增 core 自动聚合错误协议。

基线测试中的 API DTO 和 manifest DTO 是能力验证用的普通 Scoop 源码。可放在 `scoop.json` 的同 Cone 测试中并按职责拆分文件；公开库只交付主设计约定的基础 API，不发布只为这组测试存在的工厂、注册表或 API client。

## 8. 调研来源

以下官方页面已于 2026-10-14 实际读取，用于本次形状分析；不根据模型可用性或 SDK 便利属性推断 wire schema。OpenAI `output_text` 等 SDK 聚合属性不作为 JSON 根字段要求。

- [OpenAI Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- [OpenAI Create a model response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- [OpenAI Chat 与 Responses 迁移对照](https://developers.openai.com/api/docs/guides/migrate-to-responses)
- [OpenAI Streaming API responses](https://developers.openai.com/api/docs/guides/streaming-responses)
- [Anthropic Create a Message](https://platform.claude.com/docs/en/api/messages/create)
- [Anthropic Messages 类型定义](https://platform.claude.com/docs/en/api/http/messages)
- [Anthropic Streaming Messages](https://platform.claude.com/docs/en/build-with-claude/streaming)

Cone schema 以[语言规范](../specs/SCOOP-SPEC.md) 12.2～12.2.4 及现有实现为依据：[manifest 结构](../../compiler/manifest/src/semantic/parse.rs)、[依赖短长式与 locator](../../compiler/manifest/src/semantic/parse/dependencies.rs)、[source／when](../../compiler/manifest/src/selection/parse.rs)、[native 配置](../../compiler/manifest/src/native/parse.rs)、[native libraries](../../compiler/manifest/src/native/libraries.rs)。序列化现状参照 [core decoding](../../sysroot/lib/scoop.core/src/decoding.scoop)、[core encoding](../../sysroot/lib/scoop.core/src/encoding.scoop)及 [JSON 入口](../../sysroot/lib/scoop.json/src/json.scoop)。
