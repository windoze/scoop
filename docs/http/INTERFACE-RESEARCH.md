# HTTP 协议库兼容性调研与接口草案

日期：2026-10-17。状态：调研结论与公共接口建议，尚未实现或冻结 HTTP API。

实施范围与集成决定已整理到 [M53 设计](../milestone53/DESIGN.md)。M53 明确只采用官方纯静态库构建与现有 archive 成员选择，函数级裁剪不属于本期；本文第 7 节的函数／数据段消除仅保留为后续技术背景。

本文以已有 [异步 I/O](../specs/SCOOP-ASYNCIO-SPEC.md) 与 [TLS](../specs/SCOOP-TLS-SPEC.md) 为基础，考察成熟解析器实际提供的兼容能力。HTTP 作为普通库实现；没有新增编译器或 runtime 契约的需求。

`Content-Disposition` 的 `filename`／`filename*`、字符集选择及百分号编码由调用者处理。HTTP 库负责字段与消息的结构、消息定界和协议状态，并提供不强制文本解码的字段访问。

## 1. 调研范围与证据

| 库 | 本次基线 | 用途与核对方法 |
| --- | --- | --- |
| [llhttp](https://github.com/nodejs/llhttp/releases/tag/release/v9.4.3) | `release/v9.4.3` | HTTP/1 候选后端；核对公开头文件、生成的 C 状态机和辅助实现，并在本机运行原生探针。 |
| [nghttp2](https://github.com/nghttp2/nghttp2/releases/tag/v1.70.0) | `v1.70.0` | HTTP/2 候选后端；核对公开头文件、HTTP messaging 校验与 session 实现。 |
| [httparse](https://github.com/seanmonstar/httparse/tree/v1.10.1) | `v1.10.1` | 对照 HTTP/1 接收策略的粒度；核对 `ParserConfig` 与实现。不是拟引入的第二套生产解析器。 |
| [picohttpparser](https://github.com/h2o/picohttpparser/commit/465a7ff09fbd3432fe56c673451f5460154d1f07) | 上述固定 commit | 对照低层字节解析接口、折行与 trailer 行为；核对 C 头文件和实现。该基线是源码快照。 |

llhttp 与 nghttp2 使用调研时查询到的最新稳定发布版。nghttp2 在线文档当时已指向开发版本，本文以固定 tag 的源码为准。公开声明、实际生效范围和本次运行观察分别说明，不把所有文档描述视为已验证行为。

## 2. llhttp：有细分开关，但不能按名称推导完整行为

以下名称均省略 `llhttp_set_` 前缀；11 项开关默认都关闭。来源：[公开 API](https://github.com/nodejs/llhttp/blob/release/v9.4.3/include/llhttp.h#L774-L917)、[辅助状态处理](https://github.com/nodejs/llhttp/blob/release/v9.4.3/src/http.c)。

| 公开开关 | 作用与接口含义 |
| --- | --- |
| `lenient_headers` | 放宽字段值校验。本次验证还会接受旧式折行 `obs-fold`，因此不能称作“仅允许控制字符”。它不是任意非法字段的恢复开关；本次 `X : a` 即使开启它仍被拒绝。 |
| `lenient_header_value_relaxed` | 新的较窄开关。文档希望允许 NUL／CR／LF 之外的部分非法控制字节，而不连带放宽其他字段行为。本次验证接受 VT，但含 NUL 时出现不返回，详见第 6 节；当前不能直接列为可采用的能力。 |
| `lenient_chunked_length` | 放宽 `Transfer-Encoding` 与 `Content-Length` 并存检查。还影响没有最终 `chunked` 的请求：本次仅开启此项，就使 `Transfer-Encoding: gzip` 的请求走到 EOF 的 body 定界路径。不能承诺只改变 TE／CL 冲突检查。 |
| `lenient_transfer_encoding` | 放宽 `chunked` 之后还有其他传输编码等 TE 检查；影响消息定界。与上一项有交叠，开启后仍由实际状态机决定按 chunked 或 EOF 定界。 |
| `lenient_keep_alive` | 在已完成消息要求关闭的情况下，继续解析后面的消息。本次验证第二个请求仍产生完成事件。它不等于连接可以进入连接池复用。 |
| `lenient_data_after_close` | 在上述场景丢弃额外数据而不报错。本次只得到第一个请求。应与继续解析表达为互斥策略。 |
| `lenient_version` | 放宽文本起始行的版本检查。默认解析器允许的版本值包括 `0.9`、`1.0`、`1.1`、`2.0`；接受这些数字不代表实现了对应版本的完整协议。尤其不能据此处理二进制 HTTP/2。 |
| `lenient_optional_lf_after_cr` | 放宽部分 CR 后缺少 LF 的检查。本次接受只有 CR 的请求行，但字段行只有 CR 仍失败；不能概括成“所有裸 CR 均可接受”。 |
| `lenient_optional_cr_before_lf` | 放宽部分 LF 前缺少 CR 的检查。本次接受只有 LF 的字段行；全消息都用 LF 时还需要上一项。不能独立包装成精确的“仅允许 LF”承诺。 |
| `lenient_optional_crlf_after_chunk` | 允许 chunk data 后缺少与下一 chunk 分隔的 CRLF。与字段字符集无关，直接改变 chunk 定界接受范围。 |
| `lenient_spaces_after_chunk_size` | 放宽 chunk size 后的空格检查。应是独立选项，不连带放宽其他 chunk 语法。 |

另外有三项直接影响接口选型的事实：

- 普通字段值中的非 ASCII 字节不需要宽容开关。本次默认模式完整返回了 UTF-8 的“中文”字节；上层强制 ASCII 或隐式转成 `String` 才会额外损失表达能力。
- llhttp 的方法识别使用已知方法集合。本次 `CUSTOM / HTTP/1.1` 在默认模式和全部 11 项宽容开关打开时都返回 `HPE_INVALID_METHOD`。公开数据模型可以保存方法 token，但使用此版本后端不能承诺接收任意扩展方法；这是最终选型的明确取舍。
- `on_header_field`／`on_header_value` 是分段回调；回调边界不是字段边界。`on_headers_complete` 的跳过 body／暂停升级能力、EOF 输入以及保留未消费后缀，是协议适配合同，不是宽容开关。

## 3. nghttp2：少量接收容错、部分内建兼容行为，以及会话配置

### 3.1 真正改变字段接收行为的接口

来源：[option API](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/includes/nghttp2/nghttp2.h#L2980-L3260)、[非法字段回调](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/includes/nghttp2/nghttp2.h#L2038-L2112)、[实际 HTTP 校验](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/nghttp2_http.c)。

| 接口 | 实际范围 | 上层建议 |
| --- | --- | --- |
| `nghttp2_option_set_no_rfc9113_leading_and_trailing_ws_validation` | 允许部分字段值保留首尾 SP／HTAB。默认仍校验；伪头等特殊字段不受这一放宽影响。 | 正向命名为 `allowFieldValueEdgeWhitespace`，明确只作用于后端允许放宽的字段。 |
| `nghttp2_on_invalid_header_callback`／`callback2` | 对一部分非法普通字段选择忽略或 reset stream。返回 0 表示忽略该字段，不会把它交给正常字段回调，也不能原地修好后要求库重新校验。 | 提供 `Ignore`／`RejectStream`，被忽略且回调可见的字段作为独立事件交付，保留字节。 |
| `nghttp2_option_set_no_http_messaging` | 关闭 nghttp2 的 HTTP messaging 校验子集，仍保留帧、流、HPACK、client／server 模型等约束。 | 不加入普通 `HttpRequest`／`HttpResponse` 接收配置。该能力属于直接使用 nghttp2 会话和字段块的低层接口。 |

非法字段回调的边界需要直接写入合同：

- 默认没有安装两种 invalid-header 回调时，部分非法普通字段会被忽略。开启 HTTP messaging 校验不等于所有非法字段都会终止流。
- 非法伪头、大写字段名，以及 `Content-Length`、连接专用字段等若干消息错误不会进入这个可忽略回调。不能提供“忽略任何非法字段”的配置。
- 在本次版本源码中，两种回调返回 `NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE` 都走 HTTP header error 的 `PROTOCOL_ERROR` 处理。`callback2` 的头文件注释仍写 `INTERNAL_ERROR`，不能只复制注释作为上层错误合同。依据：[共同分发路径](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/nghttp2_session.c#L3281-L3309)、[错误映射](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/nghttp2_session.c#L3364-L3388)、[非法字段处理](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/nghttp2_session.c#L3576-L3661)。

还有无法用上述开关逐项控制的兼容行为：[实现](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/nghttp2_http.c#L333-L401)会接受并移除 `204` 响应中的单个 `Content-Length: 0`，注释明确说明是兼容广泛使用的服务器；成功 CONNECT 响应中的 `Content-Length` 也会移除。这些字段不会进入正常字段回调，也不是 invalid-header 回调能全部补回的数据。

### 3.2 其他选项应放在哪里

| 实际 API／配置 | 分类与设计结论 |
| --- | --- |
| `no_auto_window_update` | 适配层通过应用实际消费 body 的进度调用 `nghttp2_session_consume`，用于已有流的背压。不要把“自动返还窗口”当 HTTP 容错开关。 |
| `no_recv_client_magic` | 由适配层决定谁消费明文 H2 的 24 字节前言；开启后库仍检查后续 SETTINGS。它不表示接受缺少前言的普通 H2 连接。 |
| `no_auto_ping_ack` | 默认由库自动 ACK。普通消息 API 无需让应用承担这个会话细节。 |
| `peer_max_concurrent_streams` | 在收到对端初始 SETTINGS 前使用的发送并发上限，默认 100，之后被对端初始设置覆盖。属于连接参数。 |
| `max_send_header_block_length` | 默认 64 KiB，按 `nghttp2_hd_deflate_bound` 的发送上界计算。不是接收解压后字段大小限制。 |
| `max_deflate_dynamic_table_size` | 默认 4 KiB，实际与对端允许值取较小者；属于 HPACK 编码参数。 |
| `max_continuations` | 默认允许一个入站字段块后最多 8 个 CONTINUATION。会影响大字段块互操作，应作为具体 HTTP/2 本地限制说明。 |
| `max_settings`／`max_outbound_ack` | 分别默认 32 个 SETTINGS 项、1000 个待发送 SETTINGS／PING ACK。与字段语法宽容分开。 |
| `max_reserved_remote_streams` | 客户端收到的远端保留流默认上限 200，主要与 server push 有关；不是通用流总数上限。 |
| `stream_reset_rate_limit`／`glitch_rate_limit` | 上游已有的局部会话处理限制。记录依赖行为即可，不据此在 Scoop 新增通用资源计费或预算体系。 |
| `user_recv_extension_type`／`builtin_recv_extension_type` | 登记大于 `0x9` 的扩展帧类型；同类型用户处理优先。若确有扩展协议调用者，可按类型交付帧事件，不需要通用插件框架。 |
| `NGHTTP2_NV_FLAG_NO_INDEX` | 字段级 HPACK never-index 标记。数据模型应能表达并保留；其含义不是禁止压缩。 |
| `NGHTTP2_NV_FLAG_NO_COPY_NAME`／`NO_COPY_VALUE` | 内存生命周期约定，由适配层处理，不公开成字段语义选项。 |
| `no_closed_streams`／`server_fallback_rfc7540_priorities` | 此版本中已废弃，对 session 无效。不能加入一个看起来可用的公共兼容选项。 |

SETTINGS 协商与本地限制也要区分。例如 `SETTINGS_MAX_HEADER_LIST_SIZE` 不是自动完成的本地接收大小检查，`SETTINGS_ENABLE_CONNECT_PROTOCOL` 也不能单独代替扩展 CONNECT 的消息与应用处理。

## 4. 其他成熟解析器提供的对照

### 4.1 httparse 1.10.1

[`ParserConfig`](https://github.com/seanmonstar/httparse/blob/v1.10.1/src/lib.rs#L207-L410) 的七项布尔配置默认都是 false：

| 配置 | 范围 |
| --- | --- |
| `allow_spaces_after_header_name_in_responses` | 仅响应，允许字段名与冒号之间的 SP／HTAB。 |
| `allow_obsolete_multiline_headers_in_responses` | 仅响应，接受旧式折行；返回 value 仍可包含折行 CRLF，文档要求 HTTP 包装层做展开。 |
| `allow_multiple_spaces_in_request_line_delimiters` | 请求行分隔符允许多个 SP；不代表允许 HTAB／VT／FF 等所有空白。 |
| `allow_multiple_spaces_in_response_status_delimiters` | 响应状态行对应的多个 SP。 |
| `allow_space_before_first_header_name` | 允许首字段名前空白；文档引用 curl 的实际兼容问题作为依据。 |
| `ignore_invalid_headers_in_responses` | 尝试跳过非法响应字段行。NUL 与孤立 CR 仍失败；不是任意错误恢复。 |
| `ignore_invalid_headers_in_requests` | 请求方向对应的忽略策略，独立配置。 |

这说明请求与响应的宽容策略应能分别配置，也说明“接受字段”“展开折行”“丢弃非法行”是不同操作。llhttp 没有七项等价的独立 API，不能只为接口整齐而虚构映射，或在前面再写一套修复解析器。

### 4.2 picohttpparser

这份[公开头文件](https://github.com/h2o/picohttpparser/blob/465a7ff09fbd3432fe56c673451f5460154d1f07/picohttpparser.h)没有上述通用宽容配置。它返回字节 span，折行以 `name == NULL` 的后续项表示；[实现](https://github.com/h2o/picohttpparser/blob/465a7ff09fbd3432fe56c673451f5460154d1f07/picohttpparser.c#L307-L361)会跳过冒号后的起始 OWS，并删除值末尾的 SP／HTAB。

`phr_chunked_decoder.consume_trailer` 控制是否消费 trailer 部分；消费不等于向应用解析并交付 trailer。需要 trailer 的 HTTP 包装层必须明确保留与解析方式。小型字节解析器同样具有默认接受与归一化行为，不能据其返回 span 就承诺原始报文复现。

## 5. 据此调整上层接口

### 5.1 数据表示保证应用能自行解释字段

下面示意字段的存储形状。M53 的公共构造入口按库 API 风格接收 `List<Byte>` 并取得独立快照，读取形状保留 `Array<Byte>`：

```scoop
public struct HttpField(
    val name: Array<Byte>,
    val value: Array<Byte>,
    val neverIndex: Boolean = false
)
```

请求／响应保存有序 `List<HttpField>`，允许同名字段重复，trailer 单独交付。提供 ASCII 名称查询与显式文本转换便利入口；底层表示不要求字段值是合法 UTF-8，不自动合并同名项、百分号解码或解释参数。`neverIndex` 对应 HTTP/2 压缩元数据，HTTP/1 没有对应的线缆编码。

方法与 request-target 也保留 token／字节表示，不把方法的数据类型封死为标准方法枚举，不自动解码或重编码 request-target。后端接收范围仍须如实说明，尤其是 llhttp 的扩展方法限制。

字段 API 保证的是：对后端交付的值不增加隐式业务语义转换。它不能保证恢复原始线缆字节：llhttp 可能跳过值前 OWS、展开折行；picohttpparser 还会去掉尾 OWS；nghttp2 会过滤部分字段，HTTP/2 的字段名本来就要求小写。若应用要求报文级逐字节记录，应在输入字节流上记录；HTTP/2 解压后的字段也不包含原始 HPACK 编码。

发送接受调用者准备好的字段值字节，保留重复字段与相对顺序，并按所选协议完成外层结构与定界。接收宽容不会隐式放宽发送语法；比如调用者提供字段值字节，不表示可以用 CRLF 在一个字段里注入新的字段行。HTTP/2 伪头顺序及字段名小写由协议编码保证。

### 5.2 接收策略按协议和连接角色分别配置

建议配置形状如下。名称仍是草案；每项的接受范围以第 2、3 节为合同依据。

```scoop
public enum Http1AfterClose { Reject, Discard, ParseNextMessage }
public enum Http2InvalidRegularField { Ignore, RejectStream }

public struct Http1ReceiveOptions(
    val legacyHeaderValues: Boolean = false,
    val lenientChunkedLength: Boolean = false,
    val lenientTransferEncoding: Boolean = false,
    val lenientTextVersion: Boolean = false,
    val optionalLfAfterCr: Boolean = false,
    val optionalCrBeforeLf: Boolean = false,
    val optionalCrLfAfterChunk: Boolean = false,
    val spacesAfterChunkSize: Boolean = false,
    val afterClose: Http1AfterClose = Http1AfterClose.Reject
)

public struct Http2ReceiveOptions(
    val allowFieldValueEdgeWhitespace: Boolean = false,
    val invalidRegularField: Http2InvalidRegularField = Http2InvalidRegularField.Ignore
)
```

设计约束：

1. 客户端对响应、服务端对请求分别持有接收配置；代理的两条连接可以不同。配置在连接创建时固定，不设全局 `strict`／`lenient` 总开关，也不把后端数字 bitmask 变成公共 API。
2. HTTP/1 两个 after-close 开关映射为互斥枚举；`ParseNextMessage` 只改变解析行为，不覆盖传输关闭或复用决策。
3. 保留 `lenientChunkedLength` 等限定名称，完整记录后端联动；不把它们重新命名成实际无法兑现的单一语义。两个换行选项也不能被简化为“精确允许某一种换行”。
4. `legacyHeaderValues` 对应较宽的 `lenient_headers`；本次不把有已复现问题的 `header_value_relaxed` 加入可采用配置。后续若采用修复版本，再依据实际行为增加窄选项；不用宽选项伪装成等价替代。
5. HTTP/2 默认保留后端“忽略可忽略非法普通字段”的行为，同时提供被忽略字段事件；需要终止流的调用者显式选 `RejectStream`。事件应包含后端回调确实提供的名称、值、流 ID；不承诺观测所有被内建规则删除的字段。
6. `no_http_messaging` 不加入这个正常消息接口。不能关闭其依赖的消息检查，再把缺少方法、状态等必要信息的字段块伪装成完整请求／响应；本次也不为此复制一套 HTTP 校验器。

原生同步回调按预置策略立即选择忽略或拒绝，并将所需字节保存到 native 自有记录；Scoop 在解析调用返回后读取事件。用户处理函数不在这个同步决策点挂起。保持现有 libuv、TLS、NativeByteBuffer 与普通 FFI 边界即可。

响应解析入口还要取得对应请求的方法上下文，正确处理 HEAD 与成功 CONNECT；信息响应、最终响应、trailer 和协议切换分别可观察。这些属于正常 HTTP 消息处理，不能要求用户靠宽容开关补救。

### 5.3 协商、限制和扩展分别表达

- TLS 使用已有 ALPN 能力，并向 HTTP 层提供实际协商结果；H2 与 HTTP/1.1 的选择不是解析宽容。
- 明文客户端分别选择 HTTP/1、HTTP/2 prior knowledge 或 h2c Upgrade；服务端分别声明接受哪些入口。`allowHttp2` 一个布尔值不足以表达这些行为。
- h2c Upgrade 有现成实现支持：[curl `--http2`](https://curl.se/docs/manpage.html#--http2)、[nghttp `--upgrade`](https://nghttp2.org/documentation/nghttp.1.html)、[nghttp2 `session_upgrade2`](https://nghttp2.org/documentation/nghttp2_session_upgrade2.html)。是否启用由连接协议策略决定，不能因为属于旧协商方式就等同于非法报文宽容。
- 底层解析结果要交付已消费字节数、消息结束／升级状态以及剩余输入。切换到 H2 或其他协议时继续消费该后缀，不能把同一次 read 中多读的字节丢掉。普通 Upgrade 能力也不等于 HTTP 库需要实现升级后的应用协议。
- 本地 header 大小、CONTINUATION 数量等是具体限制，放在独立的协议限制配置中，按实际需要提供；不与兼容选项共用一个“严格等级”。
- 扩展帧处理采用 nghttp2 已有的类型登记与帧事件能力。没有具体使用者的扩展无需先建插件体系。

## 6. 原生探针结果与适用限制

本次使用固定发布源码，在 Darwin／arm64 上由 Clang 编译独立 C 探针；探针先经过 clang-format 与 `-Wall -Wextra -Werror` 语法检查。没有修改第三方源码。

| 输入／配置 | 观察结果 |
| --- | --- |
| 普通值包含 UTF-8“中文”，默认配置 | 成功，返回相同值字节。 |
| 普通值含 VT，默认／`header_value_relaxed` | 默认拒绝；窄开关开启后接受。 |
| 字段名和冒号之间有 SP，默认／宽字段模式／全部开关 | 都拒绝。新 API 注释把这种行为列为宽字段模式的示例，不能据此宣称实际支持。 |
| `Connection: close` 后再接一个请求 | 默认报 `HPE_CLOSED_CONNECTION`；keep-alive 宽容得到两条消息；discard 模式只得到一条。 |
| `CUSTOM` 方法，默认／全部开关 | 都报 `HPE_INVALID_METHOD`。 |
| 请求含 `Transfer-Encoding: gzip`，没有最终 chunked | 默认拒绝；单开 chunked-length 或 transfer-encoding 宽容均在 EOF 完成 body。 |
| 整条请求只用 LF | 只开 `optional_cr_before_lf` 仍失败；两个换行开关同时开启成功。 |
| 只有 CR 的请求行／字段行 | `optional_lf_after_cr` 使前者成功；后者即使两个换行开关开启仍失败。 |
| `X: a` 后有折行 ` b` | 默认拒绝；宽字段模式交付 `a b`。 |
| 值前两个 SP、值后 SP／HTAB | 默认交付值时已删除前 SP，保留尾 SP／HTAB。 |

发现一项需要在采用对应开关前解决的实际问题：输入字节为 `GET / HTTP/1.1\r\nX: a\0b\r\n\r\n` 时，默认配置返回 `HPE_INVALID_HEADER_TOKEN`；仅打开 `header_value_relaxed` 后，`llhttp_execute` 未返回，探针由 3 秒外部 alarm 终止。移除全部用户回调的最小程序仍复现。

固定版本的[生成状态机](https://github.com/nodejs/llhttp/blob/release/v9.4.3/src/llhttp.c#L2390-L2433)在 NUL 上从 relaxed value 返回 otherwise 分支，[flag 分支](https://github.com/nodejs/llhttp/blob/release/v9.4.3/src/llhttp.c#L8435-L8451)再跳回 relaxed value，没有推进输入或报错。这解释了本次观察。结论限定于所测版本与路径，不据此概括其他版本或整个 llhttp 的质量。

探针源码、下载副本与原始输出位于仓库外的 `/tmp/scoop-http-compat-8_hzk7ji`，不作为项目构建输入。nghttp2、httparse 与 picohttpparser 的结论来自对应固定源码；本次没有完成它们的动态互操作测试，也不是 Scoop HTTP 集成验收。

## 7. nghttp2 构建与最终产物裁剪

可以排除周边工具，并利用静态链接排除没有引用的库成员。需要分别处理“哪些目标参与构建”和“哪些代码进入最终程序”。

### 7.1 官方纯库构建

依据 v1.70.0 的 [CMakeOptions.txt](https://github.com/nghttp2/nghttp2/blob/v1.70.0/CMakeOptions.txt)、[顶层构建](https://github.com/nghttp2/nghttp2/blob/v1.70.0/CMakeLists.txt)与 [lib 构建清单](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/CMakeLists.txt)，`libnghttp2` 本身是独立 C 协议库，没有自带 socket、事件循环或 TLS 引擎。`nghttp`、`nghttpd`、`nghttpx`、`h2load` 是其他构建目标，只链接协议库不会把这些应用链接进来。

建议仅构建静态协议库。以下以体积优先的 `MinSizeRel` 为例，源目录按实际 vendor 布局指定：

```sh
cmake -S path/to/nghttp2 -B build/nghttp2 \
  -DCMAKE_BUILD_TYPE=MinSizeRel \
  -DENABLE_LIB_ONLY=ON \
  -DBUILD_SHARED_LIBS=OFF \
  -DBUILD_STATIC_LIBS=ON \
  -DBUILD_TESTING=OFF \
  -DENABLE_FAILMALLOC=OFF \
  -DENABLE_DOC=OFF \
  -DENABLE_DEBUG=OFF \
  -DENABLE_HTTP3=OFF
cmake --build build/nghttp2 --target nghttp2_static
```

`ENABLE_LIB_ONLY` 是关闭 `ENABLE_APP`、`ENABLE_EXAMPLES`、`ENABLE_HPACK_TOOLS` 的简写；它没有同时关闭文档和测试，因此上面单独关闭后两者。禁用 HPACK tools 只移除独立工具，协议库仍保留 HTTP/2 必需的 HPACK。这里的 `ENABLE_HTTP3` 面向项目应用，不会把 `libnghttp2` 的会话 API 变成 HTTP/3 引擎。

这些开关减少构建目标和分发内容；`libnghttp2` 中原本就不存在周边应用的 TLS／事件循环实现。

### 7.2 静态库成员选择与函数级消除

| 层次 | 可以达到的效果 | 当前 Scoop 边界 |
| --- | --- | --- |
| 目标选择 | 不构建工具、示例、测试、文档及其可选依赖组件 | 上述官方配置即可表达。 |
| archive 成员选择 | 不选入没有符号引用的 `.o`，例如调用面不需要的独立辅助模块 | 普通外部 `.a` 已按实际 undefined references 选择成员；参见[实现规范 2.8](../specs/SCOOP-IMPL-SPEC.md)与[成员选择实现](../../compiler/linker/src/program/native/selection.rs)。 |
| 函数／数据段消除 | 从已经选入的 `.o` 中继续删除不可达函数和数据 | 当前 Darwin 规范明确未启用 `dead_strip`／LTO，[实际参数](../../compiler/toolchain/src/final_link/darwin.rs)也没有启用。因此不能承诺仅靠静态链接就去掉所有未调用 API。 |
| 协议功能裁剪 | 从共享状态机内部去掉某类协议功能 | 上游没有提供 client-only、server-only、去掉 push／某个内建扩展等对应的独立 CMake 开关。关闭运行时功能不保证删除机器码。 |

nghttp2 的内存发送 API、回调发送 API以及 client／server 路径共处 `nghttp2_session.c`。只调用 `nghttp2_session_mem_send2`／`mem_recv2`，仍可能将同一对象中的其他函数带入最终程序；链接器也不能仅凭运行时设置证明共享分支永远不可达。

这里有一个现有集成方式的区别：将全部上游 C 文件作为 Cone `native.sources` 编译进 `.slib` 时，[当前实现](../../compiler/linker/src/program/native.rs)将这些 native 对象作为直接输入全量纳入；普通外部 `.a` 则支持按成员选择。若目标是先取得对象粒度的裁剪，优先通过既有 `native.libraries` 静态库路径消费一份 `libnghttp2.a`，不要同时再把同一套 C 对象直接编入 Cone。shim 只覆盖当前实际调用面，避免一个对象中的大量未用转发函数额外引入库引用。

更细粒度的常规办法是 Linux 的 `-ffunction-sections -fdata-sections` 配合最终链接 `--gc-sections`，以及 Darwin 的 `-dead_strip`。仅添加编译阶段的分段选项不够。

M53 采用上游完整 HTTP/2 核心、官方纯静态库构建和现有 archive 成员选择。帧／流状态、HPACK、流控与必需的 HTTP messaging 处理属于所需能力。本期不做函数级裁剪。

本节结论来自固定版本构建脚本与当前 Scoop 链接实现核对，未执行 nghttp2 完整构建。
