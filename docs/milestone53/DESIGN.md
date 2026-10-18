# M53 设计：HTTP/1.x 与 HTTP/2 协议库

状态：已完成。日期：2026-10-18。本文落实 HTTP 协议层的接口边界、nghttp2 集成方式与本期裁剪范围；公共合同见 [HTTP 规范](../specs/SCOOP-HTTP-SPEC.md)，实际验证结果见[验收记录](PROGRESS.md)。

依据：[语言规范](../specs/SCOOP-SPEC.md)第 12～14 章、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)第 2.8 节、[库 API 风格](../specs/SCOOP-API-STYLE.md)、[M49 异步 I/O](../specs/SCOOP-ASYNCIO-SPEC.md)、[M51 TLS](../specs/SCOOP-TLS-SPEC.md)、[M52 分发设计](../milestone52/DESIGN.md)及[协议库兼容性调研](../http/INTERFACE-RESEARCH.md)。

## 0. 范围与决定

1. 提供 HTTP/1.0、HTTP/1.1 和 HTTP/2 的增量协议处理与异步流适配，支持客户端和服务端角色。复用既有字节容器、native buffer、FFI、Job、libuv 和 TLS，不增加编译器或 runtime 专属入口。
2. 协议引擎消费／产生内存中的字节，不拥有 socket 或事件循环；异步适配接管调用者提供的 `AsyncByteStream`。TLS 已完成，HTTP 层消费实际 ALPN 结果，不重复实现握手或证书处理。
3. HTTP/2 使用固定的 nghttp2 1.70.0，以内存接口接入。HTTP/1 采用 llhttp 9.4.3，首版接受第 2 节记录的已知方法集合限制，并提供明确诊断与文档。
4. 字段按有序字节项交付，保留重复字段与后端实际交付的值；`filename`／`filename*`、字段参数、字符集和百分号编码由调用者处理。协议库处理外层结构、消息定界与协议状态。
5. 接收兼容策略按 HTTP 版本和连接角色分别配置；版本协商、局部大小限制和后端驱动细节分别表达。
6. nghttp2 只构建 `libnghttp2.a`，关闭应用、示例、HPACK 工具、测试、文档及调试输出，通过既有 `native.libraries` 静态库路径消费。保留完整上游 HTTP/2 核心，不维护删除共享状态机功能的分支。
7. **本期不做函数级裁剪。** 使用现有 archive 成员选择，不修改最终链接器合同。
8. 本期不扩展为连接池、自动重试、Cookie、缓存、路由、文件下载命名或通用 HTTP 框架。QUIC／HTTP/3 需要独立传输与协议后端，不纳入 M53；nghttp2 项目的 HTTP/3 应用开关不改变这一边界。

## 1. 普通库分层

| 组成 | 职责 | 依赖与边界 |
| --- | --- | --- |
| `scoop:scoop.http:0.1.0` | 消息数据、协议配置、错误、HTTP/1 编解码和 HTTP/2 session 的内存驱动 | 使用 core、std、platform 的公开能力；C shim 与 nghttp2 静态库只在此集成一次，不依赖 libuv 或 TLS。 |
| `scoop:scoop.http.asyncio:0.1.0` | 在已有异步字节流上收发请求／响应，协调消息 body、背压、取消和关闭 | 依赖 HTTP 与 asyncio；接收普通 `AsyncByteStream` 和 `AsyncCloseable`，可使用原始 TCP 或已经建立的 `AsyncTlsStream`。 |

协议库自身不建立 TCP 连接、不调用 TLS 握手。应用沿已有 API 创建传输；使用 TLS 时，把实际 ALPN 字节传给 HTTP 协议选择入口。该入口只依赖协议字节，不要求基础 HTTP 或 asyncio 适配为此依赖 TLS 的 native 实现。

HTTP/1 decoder／encoder 与 HTTP/2 session 保持各自实际状态，不建立通用可插拔协议工厂。两者向异步适配提供推进、已消费字节、待发送字节、消息／流事件和终止状态；没有新输入、可写空间或状态变化时等待真实 I/O。

模块使用 M52 已迁移的普通 `src/<cone-name>` 布局，第三方源码集中到 `vendor`，目标资源按 canonical target triple 交付。

## 2. 后端选择与采用条件

### 2.1 HTTP/2

采用 nghttp2 1.70.0 的公开 API，使用 `nghttp2_session_mem_recv2`／`nghttp2_session_mem_send2`。session 管理帧、流状态、HPACK、SETTINGS、流控和 HTTP messaging 规则；Scoop 管理传输、应用 body 与资源生命周期。双方职责不重叠，Scoop 不再实现一套相同的 HTTP/2 状态机。

该版本没有客户端专用、服务端专用或删除 push／内建扩展的独立构建开关。运行时关闭某项功能不等于裁掉机器码。

### 2.2 HTTP/1

llhttp 9.4.3 提供增量消息定界与实际需要的兼容选项。用户已确认首版接受其已知方法集合限制；HTTP spec、诊断和测试必须明确以下接收范围：

- 默认字段值可以包含非 ASCII 字节，UTF-8 不需要开启宽容模式。
- `CUSTOM` 等不在其已知方法集合中的扩展方法，在全部宽容开关打开后仍返回 `HPE_INVALID_METHOD`。公共数据模型保持方法 token，不使用后端方法枚举限制数据表示；但这不能补足后端接收能力。
- `header_value_relaxed` 在该版本遇到含 NUL 的字段值时存在已复现的不前进循环。本期不公开或开启这一选项，不用更宽的 `lenient_headers` 伪装成等价替代。

HTTP/1 首版拒绝后端已知方法集合之外的 token，并诊断为 llhttp 9.4.3 不支持的方法；正反测试覆盖常用方法和 `CUSTOM`。公共方法数据模型仍保留 token。不能通过改写请求方法、预修复报文或维护第二个解析器绕过该限制。

具体开关、默认值、文档与实现的差异，以及原生探针结果保留在[调研文档](../http/INTERFACE-RESEARCH.md)，不把一次宿主探针当作三目标集成验收。

## 3. 消息与字段接口

### 3.1 数据表示

| 数据 | 公共合同 |
| --- | --- |
| `HttpField` | 名称和值为独立字节快照，读取形状为 `Array<Byte>`；构造入口接收 `List<Byte>` 并取得快照。另保存 HTTP/2 的 `neverIndex` 标记。 |
| 字段集合 | 有序 `List<HttpField>`，允许重复名称；提供 ASCII 不区分大小写的查询，查询结果不合并同名值。 |
| 方法 | 保留方法 token 字节，可提供常用方法常量；后端支持范围在配置与文档中明确。 |
| request-target | HTTP/1 保留请求行中的目标字节，不隐式百分号解码、URI 规范化或重编码；HTTP/2 的 `:method`、`:scheme`、`:authority`、`:path` 按协议需要单独表达，CONNECT 使用对应目标形状。 |
| 响应 | 保存协议版本、状态码和实际可用的 reason phrase；HTTP/2 没有 reason phrase，不伪造默认文本。 |
| body | 增量字节流；解析消息定界和 chunked／DATA 后交付内容字节，不按 Content-Type 或 Content-Encoding 自动解释内容。 |
| trailer | 与初始字段分开交付，能够区分尚未到达、已经结束且为空和实际收到字段。 |

文本访问是显式转换，可由用户选择编码与转换失败策略。HTTP 库不解释 `Content-Disposition` 参数，不自动生成下载文件名，不合并 `Set-Cookie` 等重复字段。

`HttpField` 的保证是对后端交付的值不增加业务语义转换。它不是原始报文镜像：HTTP/1 后端可能跳过起始 OWS 或展开折行；nghttp2 会过滤部分字段，HTTP/2 的字段名要求小写，HPACK 编码形式也不会保留在字段值中。需要逐字节记录时，由应用在既有输入字节流上记录。

### 3.2 消息边界与发送

信息响应、最终响应、body、trailer 和协议切换分别可观察。响应解析持有对应请求的方法上下文，正确处理 HEAD 与成功 CONNECT；不得靠宽容选项补救上下文缺失。

发送接口接收用户准备的字段值字节，保留重复字段与相对顺序，按选中协议生成外层格式。伪头顺序、名称小写、body 定界和结束标记由协议层负责。接收宽容不隐式放宽发送语法；字段值不能借 CRLF 改变消息结构。

正常消息接口在交付头部时具有相应消息必需的信息，不把缺少方法或状态的字段块伪装成完整请求／响应。后端已经完成的接收校验不在 Scoop 层完整重放；Scoop 只处理自身表示、关联请求和输出所必需的检查。

## 4. 接收兼容配置

配置在连接创建时固定。客户端接收响应与服务端接收请求可以使用不同配置，代理的两条连接分别配置。没有全局严格等级，也不公开 C 数字 bitmask。

### 4.1 HTTP/1

以下是 llhttp 9.4.3 对应的配置形状；不静默忽略无法实现的选项。

```scoop
public enum Http1AfterClose { Reject, Discard, ParseNextMessage }

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
```

- `legacyHeaderValues` 映射到较宽的 `lenient_headers`，包括已验证的旧式折行接受行为；不承诺允许冒号前空白或忽略任意非法字段行。
- `lenientChunkedLength`、`lenientTransferEncoding` 按后端真实语义处理。前者不仅影响 TE／CL 并存，还会影响没有最终 chunked 的请求定界；接口文档必须保留这一联动。
- 两个行结束选项分别对应实际 parser 开关，不包装成“所有裸 CR”或“仅允许 LF”的更强承诺。换行、chunk 终止和 chunk size 后空格分别配置。
- `lenientTextVersion` 只放宽起始行解析。M53 执行的协议仍限于已声明的 HTTP/1.0、1.1 和 HTTP/2；接受版本数字不代表实现对应版本，也不能将文本 `HTTP/2.0` 当作二进制 H2 会话。
- `afterClose` 将继续解析与丢弃额外数据表示为互斥策略。`ParseNextMessage` 不覆盖底层传输关闭和连接复用决策。

### 4.2 HTTP/2

```scoop
public enum Http2InvalidRegularField { Ignore, RejectStream }

public struct Http2ReceiveOptions(
    val allowFieldValueEdgeWhitespace: Boolean = false,
    val invalidRegularField: Http2InvalidRegularField = Http2InvalidRegularField.Ignore
)
```

首尾 SP／HTAB 的放宽仅适用于 nghttp2 允许放宽的字段，不适用于伪头等特殊字段。默认保留后端忽略一部分非法普通字段的行为；`RejectStream` 对其 invalid-header 回调确实覆盖的错误终止流。

被忽略且后端回调可见的字段通过独立事件交付名称、值和 stream ID，不进入正常字段集合。非法伪头、大写字段名和若干消息一致性错误不在这个可忽略范围内。同步 C 回调根据预置策略立即选择处理结果，复制事件数据；Scoop 用户处理函数在解析返回后运行，不提供可挂起的原地修复回调。

nghttp2 对 `204` 响应中的 `Content-Length: 0` 等内建过滤行为照实记录，不能承诺事件能补回所有被移除字段。普通消息接口不暴露 `no_http_messaging`，不为关闭后端规则另写一套 HTTP 校验器。

废弃且对 session 无效的 `no_closed_streams`、`server_fallback_rfc7540_priorities` 不进入公共配置。`no_recv_client_magic`、`no_auto_window_update`、自动 PING ACK 与 NO_COPY 标志属于适配层分工，不包装成用户兼容策略。

### 4.3 具体限制与扩展

字段大小、字段数量、HTTP/2 CONTINUATION 数量等局部限制与兼容配置分开。区分发送上限、接收上限和向对端声明的 SETTINGS；例如 `max_send_header_block_length` 按压缩上界计算，不能当作接收字段上限。

本期使用后端现有的扩展帧处理机制；没有实际应用调用者的扩展不增加通用插件框架。协议本地限制不扩展成跨层 CPU／内存计费或预算体系。

## 5. 版本协商与升级

| 入口 | 行为 |
| --- | --- |
| 已建立 TLS 的字节流 | 依据实际 ALPN 选择 `h2` 或 `http/1.1`；没有 ALPN 时仅在允许 HTTP/1.1 回退的策略下使用 HTTP/1.1，否则报协商失败。收到未允许的 ALPN 不通过试解析猜测协议。 |
| 明文 HTTP/1 | 直接使用 HTTP/1 decoder，保持连接内的消息定界与请求／响应关联。 |
| 明文 HTTP/2 prior knowledge | 客户端直接发送 H2 前言和 SETTINGS；服务端按显式允许的入口识别并消费前言，不能用 lenient-version 代替。 |
| 明文 h2c Upgrade | 作为显式可选协商路径，复用 HTTP/1 Upgrade 与 `nghttp2_session_upgrade2`。拒绝升级时继续处理真实 HTTP/1 响应，不自动重放原请求。 |

客户端选择明确的明文模式，服务端声明允许的入口集合；单个 `allowHttp2` 布尔值不足以表达两者。服务端前言识别支持分段输入，谁消费 24 字节 magic 是适配内部决定，不表示允许对端省略必要前言。

h2c 路径验证 Upgrade／Connection／HTTP2-Settings 所需的协议字段，处理初始 HTTP/1 请求 body 与 stream 1 的关联，然后移交会话。101、成功 CONNECT 或其他升级结果均交付未消费的输入后缀；同次 read 中多读的字节不能丢失。

通用协议切换接口移交传输及残留字节的所有权，不继续让旧 HTTP decoder 消费该传输，也不要求 HTTP 库实现升级后的应用协议。

## 6. nghttp2 构建、链接与交付

### 6.1 固定源码与官方构建选项

使用 nghttp2 `v1.70.0` 官方源码，保留许可证。源码位置遵循 M52 的 `sysroot/vendor/nghttp2/`；每个既有目标分别生成 `libnghttp2.a` 与匹配公开头：

```text
sysroot/
  vendor/nghttp2/
  src/scoop.http/
  src/scoop.http.asyncio/
  lib/<canonical-target-triple>/nghttp2/
    include/nghttp2/
    lib/libnghttp2.a
```

目标继续为 `aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`。沿已有目标 C 工具链和 M52 显式 sysroot 准备流程构建，不混用 GNU／musl 资源。普通应用构建消费预编译结果，不自动下载或重建依赖。

以下是公共构建选项；源码、构建目录、安装 prefix 和交叉工具链由正常目标准备步骤提供：

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

`ENABLE_LIB_ONLY` 关闭 `ENABLE_APP`、`ENABLE_EXAMPLES`、`ENABLE_HPACK_TOOLS`；测试与文档另行关闭。上游在 release 类构建中保留的断言沿用其配置，不为缩减体积自行去掉正常检查。静态库消费沿用其公开头与 `NGHTTP2_STATICLIB` 编译定义。

nghttp／nghttpd／nghttpx／h2load 和 HPACK 工具不作为本期分发内容。关闭 HPACK 工具不会移除库内 HPACK，关闭项目 HTTP/3 应用能力也不改变所需 HTTP/2 核心。独立互操作测试可以调用开发环境中的工具，不把它们链接或打包进生产库。

选项依据：[上游 CMakeOptions](https://github.com/nghttp2/nghttp2/blob/v1.70.0/CMakeOptions.txt)、[库目标](https://github.com/nghttp2/nghttp2/blob/v1.70.0/lib/CMakeLists.txt)。

### 6.2 通过普通静态库路径消费

HTTP Cone 以 `native.libraries` 声明静态 `nghttp2`，公共头通过既有目标条件 include 选择；M52 默认目标库搜索根扩展到 `lib/<triple>/nghttp2/lib`。只在依赖闭包实际声明此库时解析，不因为分发包包含它就要求所有程序链接 HTTP。

Scoop shim 作为自身 native 源码编入 Cone，覆盖当前实际使用的调用面。nghttp2 上游 C 对象只由独立静态库提供一次，不再同时列入 Cone 的 `native.sources`。

普通 `.a` 沿现有 undefined-reference 闭合选择成员；`.slib` 内的 native 源码对象当前作为直接输入全量纳入。选择静态库消费就是为了使用已有成员粒度的筛选，不新增专用库 reader、导出认证或另一套链接流程。来源依据：[native 直接输入](../../compiler/linker/src/program/native.rs)、[archive 成员选择](../../compiler/linker/src/program/native/selection.rs)。

## 7. 内存驱动与异步生命周期

### 7.1 native 边界

同一协议实例的 native 调用串行。回调只处理 native 状态、预置策略和稳定存储，不保存 managed 引用、不挂起或恢复 continuation。返回 Scoop 后再交付事件、抛协议异常或恢复等待者。

输入遵守 `NativeByteBorrow` 的有效期，正确提交实际消费前缀。nghttp2 返回的临时发送指针不能直接跨下一次可能使其失效的调用保存；异步传输前转入自有输出存储，或在明确的有效期内完成对应借用。HTTP/1 分段字段回调需要组合到字段完成，回调片段不能误认为完整字段。

HTTP/2 DATA provider 只读取已准备的 native body 字节；缺少数据时按后端的 deferred／resume-data 机制等待上层供给，不能在 C 回调里读取异步流。输出进度代表本层实际接受量，取消或下游失败不回滚、不自动重发已接受数据。

### 7.2 连接与 body

异步连接固定归属创建它的 EventLoop，复用普通 Job、Context 和同步原语，不使用 libuv 私有入口。传输每方向至多一个操作，HTTP/2 可以在流之间复用连接；输出背压时仍允许必要输入推进，避免窗口更新相互等待。

HTTP/1 保留请求顺序与响应匹配，消息未定界完成前不能把剩余 body 当下一条消息。HTTP/2 按 stream 关联消息；流结束、RST_STREAM 与连接级 GOAWAY 分别交付，错误保留实际作用域、后端码和必要上下文。

应用消费 HTTP/2 body 后通过 `nghttp2_session_consume` 返还相应流控窗口；不能仅因收到 DATA 就把应用尚未消费的数据视为已消费。普通消息队列和 buffer 容量服务实际背压，不引入跨层计费。

### 7.3 所有权、取消与关闭

异步包装使用 `takeOwnership` 语义，登记成功时接管传输。登记前的参数、配置或取消失败保持调用者所有权；接管后的失败负责关闭未交付的连接与 native 状态。交付后调用者不得绕过 HTTP 层并发使用底层传输。

HTTP/2 的请求／流取消作用于对应 stream；共享底层读写属于连接所有者的结构化生命周期。不能直接取消某个请求正在借用的连接级 TLS read／write Job，因为现有 TLS 合同会因此关闭整条 TLS 连接。连接所有者取消则关闭整条连接并完成所有流收尾。

HTTP/1 在消息未完整消费时取消或提前关闭 body，关闭连接以结束其剩余定界责任；不进行无界自动 drain。已经明确完成消息边界的操作按实际连接状态处理。

`close` 负责本地收尾，不隐含 flush 或对端确认；并发 close 等待同一次收尾。关闭先拒绝新操作，等底层操作、native 借用和未交付资源结束后释放协议实例，允许在已取消 Job 的 finally 中完成。需要协议层优雅结束时使用独立动作，由应用通过现有 Job／定时器控制等待。

## 8. 实施顺序与规范衔接

1. 在新增的 HTTP 普通库 spec 中落定消息表示、协议入口、兼容选项与生命周期合同，闭合第 2.2 节的 HTTP/1 后端采用决策；交叉引用既有 asyncio、TLS、API 风格和 native 链接规则。本文不要求改变语言语义、runtime ABI 或最终链接器。
2. 接入固定 nghttp2 源码与三个目标的纯静态构建，完成普通库声明、目标 include 和分发资源路径；先建立独立 native 消费和内存会话闭环。
3. 实现字段数据、HTTP/1 增量编解码、HTTP/2 内存会话与必要事件，先验证解析／发送、分段输入与错误作用域，不同时展开额外应用层功能。
4. 接入异步流、TLS ALPN 结果、明文 prior knowledge 与可选 h2c Upgrade，完成 body 背压、取消、关闭和协议切换后的剩余字节移交。
5. 使用普通 Scoop 外部消费者完成三目标集成验收，并将新增 Cone 与静态资源纳入 M52 既有发布清单。打包和搬迁检查仍按 M52 的手工发布验证规则执行，不扩展为每次常规测试的要求。

实施先同步普通 HTTP spec；实际涉及 M52 目标搜索根与分发清单的变更，在其现有职责位置完成。保持 M48／M50 的缓存和验证边界，不新增准备回调框架、来源审计、产物收据或对最终文件的完整重复验收。

## 9. 验收范围

| 范围 | 需要覆盖的实际行为 |
| --- | --- |
| HTTP/1 增量处理 | 起始行／字段／chunk 任意分段、连续消息、EOF 定界、HEAD、信息响应、CONNECT、trailer；后端方法集合边界和所公开的兼容开关分别有正反例。 |
| HTTP/2 内存会话 | 独立 client／server 对端、HPACK 状态跨消息、并发 stream、SETTINGS、流控、RST_STREAM、GOAWAY、字段与消息错误的作用域。 |
| 字段灵活性 | 非 ASCII／非 UTF-8 的合法值字节、重复项和顺序、显式文本转换、never-index、忽略字段事件。 |
| 版本协商 | TLS ALPN 的 H2／HTTP/1.1 与无 ALPN 策略，明文 H1／H2，h2c 成功与拒绝，分段前言和升级后同次读取的残留字节。 |
| 异步组合 | 短读写、双向背压、body 部分消费、H2 单流取消后其他流继续收发、连接取消、并发关闭、moving GC 与 native buffer 借用收尾。 |
| native 交付 | 普通 Scoop 外部消费者链接目标静态库并完成 HTTP 通信。 |

主要测试位于各 Cone 的 `scoop.test`，使用真实 loopback 动态端口、明确同步点和既有测试 CA，不依赖公网。独立协议对端用于互操作；原始日志保存在仓库外。

Darwin／arm64、Linux／GNU amd64、Linux／musl amd64 分别运行适用的库与外部消费者测试，覆盖 debug／release 和 moving GC。文档与独立宿主探针不能替代实际目标运行；文档阶段只做引用和格式检查，不将尚未执行的验证写成通过。
