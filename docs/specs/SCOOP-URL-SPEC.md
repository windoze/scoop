# Scoop URL 库规范

版本：M54。提供独立普通 Cone `scoop:scoop.url:0.1.0`，依赖 core、std 和 [IDNA](SCOOP-IDNA-SPEC.md)。本库负责 URL 的解析、相对引用解析、组件修改与拼装，不执行 DNS、文件或网络 I/O。客户端行为另见 [HTTP client 规范](SCOOP-HTTP-CLIENT-SPEC.md)；实际目标验证范围见 [M54 进度](../milestone54/PROGRESS.md)。

## 1. 语法选择与实现

采用 RFC 3986 的 URI reference 语法、相对解析与点段规则，并按 RFC 3987 将 Unicode 输入映射为 URI。它适用于库和服务端调用；不声称实现浏览器 WHATWG URL 的全部容错行为。非法百分号、裸空白、控制字符、反斜杠和非法端口明确报错，不自动剥掉换行或把反斜杠改成路径分隔符。

使用 uriparser 1.0.2 的 C API 处理 URI 结构、相对解析和序列化。采用 BSD-3-Clause 的库部分，不携带上游测试、工具或文档依赖。第三方语法处理只集成一次，web client 不另写 URL parser。域名转换调用 `scoop.idna`，不选择内嵌第二套 IDNA 数据的 URL 后端。

Unicode 输入先按 IRI 规则把非 ASCII 标量编码为 UTF-8 百分号字节，再交给同一 URI parser；ASCII 结构分隔符保持不变。解析出的网络 host 在明确边界进行百分号解码、严格 UTF-8 和 IDNA 转换。此适配不重写 URI 状态机，不把整个输入先解码后再寻找分隔符。

`Url` 表示绝对或相对引用；允许无 authority 的绝对 URI 和非 HTTP scheme。通用 URI 的 registered-name 按组件保留；只有转换为网络 origin 时才要求 DNS 名或 IP。本库不猜测 scheme 专属语义，rootless path 也按 RFC 3986 的通用相对解析规则处理。HTTP client 只接受绝对 `http`／`https` URL，WebSocket 入口接受 `ws`／`wss`。

## 2. 值与公共入口

Url 为不可变、可比较与 hash 的普通值对象，持有 Scoop 组件快照，不持有需要用户关闭的 native parser。解析中的临时 native 存储在组件复制后释放。普通输入、输出使用 String、List 和 Array，不暴露 uriparser 指针或布局。

| API／数据 | 合同 |
| --- | --- |
| `Url.parse(text)`／`parseOrNone(text)` | 接受完整引用；后者仅把 URL 格式失败转为 None |
| `base.resolve(reference)` | 按 RFC 3986 第 5 节返回新值，reference 可为 String 或 Url |
| `url.toString()` | 生成该值的 ASCII URI；保留有语义影响的组件及已编码字节 |
| `url.toIriString()` | 生成 Unicode 展示文本；不用于 DNS、TLS、签名或 origin 比较 |
| `url.toBuilder()`、`builder.build()` | 显式修改与检查后构造新值；原值不变 |
| `scheme`、`authority`、`encodedPath`、`encodedQuery`、`encodedFragment` | 保存是否存在及编码后的独立组件；缺失和空串不同 |
| `httpOrigin()` | 验证 HTTP／WebSocket 网络目标并返回规范化 scheme、host、有效端口 |
| `httpRequestTarget()` | 返回 path 与可选 query；空 path 变为 `/`，不包含 fragment 或 userinfo |
| `httpAbsoluteTarget()` | 返回代理请求所需的绝对形式；使用相同网络目标校验并排除 userinfo 与 fragment |

authority 区分 userinfo、host 和显式 port。网络 host 用 `UrlHost { Dns(String), Ipv4(Array<Byte>), Ipv6(Array<Byte>) }` 表示，DNS 值为已经校验的小写 A-label，IP 为网络序字节快照。通用 registered-name 不能未经验证直接变成 Dns。IPv6 在 URI authority 中使用方括号，交给 DNS／TLS 时使用实际地址或不带括号的名称。

IPv4 只接受四段十进制的规范地址范围，拒绝十六进制、八进制和少段数字地址的浏览器兼容写法；不得在 URL 校验后让系统 resolver 将被当成域名的输入重新解释为另一种数字地址。IPvFuture 可以作为通用 URI 数据解析，M54 HTTP 网络目标不接受它。带 IPv6 zone identifier 的文本不属于本期 RFC 3986 profile，解析明确报错；不在 client 中去掉 zone 后继续连接。

port 必须为十进制 `0..65535`，显式空端口为格式错误；HTTP client 连接目标另拒绝零。origin 按 scheme、DNS A-label／IP 字节和有效端口比较，默认 HTTP／WS 为 80、HTTPS／WSS 为 443。省略默认端口与显式默认端口属于同一 origin；ws／http 等不同 scheme 不因此自动相等。userinfo、path、query、fragment 不属于 origin。

网络 DNS 名可保留表示绝对名称的末尾根点用于序列化和解析。证书名称比较使用去掉该根点的 A-label；origin、Cookie 和连接池统一使用去掉根点后的 DNS 名，禁止各层各自选择规范化方式。根域本身、空 host 和非 DNS registered-name 不能成为 HTTP 连接目标。

authority 保存 host 的网络解释结果与失败原因。DNS 同时需要传输和展示表示时调用 `Idna.parse`，复用 `IdnaName` 的两种快照；origin、展示和字段访问不重新执行 IDNA。通用 registered-name 的网络解释失败不使 URI 本身失效，`httpOrigin` 才报告对应 host 错误。支持的网络 scheme 在序列化时采用已取得的规范 A-label。

## 3. 编码、参数与拼装

builder 分开提供接收原文与接收已编码组件的入口：`appendPathSegment` 按 UTF-8 编码单段，`appendEncodedPathSegment` 校验百分号格式但不二次编码；query 和 fragment 同理。原文单段中的 `/`、`?`、`#` 作为数据编码，不能被当成新结构；整体 `setPath` 则保留 `/` 作为路径分隔符。

`UrlQuery` 使用有序重复项，参数值为可选文本或字节，区分 `a`、`a=`、缺少 a；保留重复 key 的顺序。原始 query 无须符合键值形式，只有调用参数接口时才按明确的 `&` 与 `=` 规则解释。设置／删除参数和整体替换 encodedQuery 是不同操作。

参数操作返回新快照：`add` 在末尾追加，`remove` 删除所有同名项，`set` 删除所有同名项后在末尾追加新值；`getAll` 保留匹配项的原顺序。文本 getter 严格按 UTF-8 解码，任意字节可通过 `nameBytes`／`valueBytes` 访问。

通用百分号 codec 提供字节输入／输出和明确的 UTF-8 文本入口，解码结果不必是合法 UTF-8。`+` 在普通 URL query 中保持加号；只有 `FormUrlEncoded` 明确按表单规则将空格编码为加号、将加号编码为 `%2B`。表单编码与解析在 URL 库中复用，HTTP 库只负责 application/x-www-form-urlencoded 的媒体类型和消息 body。

转义的保留字符不会为了展示而解码后再序列化。`%2F` 不变成路径分隔符，`%26` 不变成参数分隔符。scheme 与网络 DNS host 规范化大小写；百分号 hex 输出统一大写。不会自动排序 query、合并重复项、删除空 query／fragment、折叠连续斜杠或进行文件系统路径规范化。

相对解析按标准移除真正的 `.`／`..` 路径段，`%2E` 是已编码数据，不作为点段。已构造且未改变的组件不在每次访问中重新完整解析；修改只检查新输入与组合后真正受影响的结构。

## 4. 资源名称与 HTTP 边界

Location 可以是绝对、相对、仅 query 或仅 fragment 的引用，均由 `resolve` 处理。重定向是否允许该 scheme、是否跨 origin、是否转发认证由 client 决定；URL 库只返回规范值。解析不读取环境变量或当前目录，不自动从缺少 scheme 的 `example.com` 猜成 HTTPS。

userinfo 能解析和显式序列化，但 HTTP client 不把它自动转成 Authorization；请求构造时拒绝含 userinfo 的网络 URL 并要求显式认证配置。日志展示入口 `redacted()` 移除 userinfo，并按调用者指定参数名隐藏 query 值，不修改用于实际传输的 Url。

文件下载目标由调用者提供 Path。Content-Disposition 的文件名只是 HTTP 元数据，不能直接用 Url 或 server 文件名覆盖本地文件。URL 不承担文件系统来源审计、沙箱或路径权限检查。

## 5. 错误、交付与验证

`UrlParseException` 保存 component、输入 UTF-8 字节位置和说明；无法精确归属单字节的组合错误使用可选位置，不伪造 index。IDNA 失败作为 host 错误的明确原因保留；后端运行错误不被 parseOrNone 吞掉。没有 DNS 名称是否存在的预检查。

uriparser 源码与许可按 M52 vendor 布局交付，三个目标提供静态库和头文件；仅构建库所需的窄字符 API，不引入系统 locale。C 返回前复制解析结果，错误路径也释放临时存储。所有 native 依赖通过普通 Cone manifest 传递。

独立库测试覆盖 RFC 3986 的正常／异常相对解析向量、Unicode host、A-label、IPv4／IPv6、显式默认端口、根点、空组件、有序重复 query、表单加号、百分号双重编码、非法 UTF-8、userinfo 和 CR／LF 注入。验证 parse／build／resolve 的组合结果及与上游 uriparser 的交叉行为；不要求该 RFC profile 与浏览器容错结果一致。

## 6. 依据

- [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986.html)、[RFC 3987](https://www.rfc-editor.org/rfc/rfc3987.html)。
- [uriparser](https://uriparser.github.io/)、[1.0.2 发布](https://github.com/uriparser/uriparser/releases/tag/uriparser-1.0.2)。
- [独立 IDNA 规范](SCOOP-IDNA-SPEC.md)。
