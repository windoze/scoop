# Scoop HTTP 消息工具规范

版本：M54，实施中。实际实现与验证见 [M54 进度](../milestone54/PROGRESS.md)。本文补充 [HTTP 规范](SCOOP-HTTP-SPEC.md)第 9 节，所有能力在 `scoop.http` 或 `scoop.http.asyncio` 中交付，不在 web client 内另建协议实现。

## 1. 字段与标准数据

保持 HttpField 的有序重复字节模型。工具按具体字段语法解释字节，解析结果为不可变值；只在格式规定可合并时合并，Set-Cookie 保持逐项处理。未知参数保留，非法结构报告具体字段与位置，不把非 UTF-8 字节无条件转成 String。

| 工具 | 必须覆盖的合同 |
| --- | --- |
| `HttpMediaType` | type／subtype、参数、quoted-string 与转义，重复参数有明确报错或选择规则 |
| `HttpContentDisposition` | disposition、filename 与 RFC 8187 filename*，有效扩展参数优先；保留原字段和解析失败原因 |
| `HttpDate` | 生成 IMF-fixdate；接收 RFC 9110 允许的三种日期形状，旧两位年份按显式参考时间判断 |
| `HttpCacheControl`、`HttpEntityTag` | 指令与值、弱／强 ETag、引号与列表边界；不自己查询或存储缓存 |
| `HttpRange`、`HttpContentRange` | bytes 范围、suffix／开放终点、206／416 所需边界，受检 Long 算术 |
| `HttpAuthChallenge`、`HttpCredentials`、`HttpAuthenticationInfo` | 多 challenge、quoted 参数、token68、请求凭据和 Digest 完成参数；不获取凭据或执行密码计算 |
| `HttpRetryAfter` | delta-seconds 或 HTTP-date，解析与实际等待分开 |
| `HttpAge`、`HttpVary` | 缓存 Age 的饱和秒数及接收首值规则；Vary 的有序字段名、空项和通配符，不计算 freshness 或选择缓存条目 |

参数名大小写、重复值处理和扩展编码按字段所属 RFC 固定，不能用一个逗号 split 处理全部字段。扩展参数先在本层拆出 charset／language／百分号字节，再交给 URL 百分号 codec 与 charset 转换；优先满足 RFC 8187 的 UTF-8，历史 ISO-8859-1 兼容须显式记录。建议文件名只作为元数据，操作系统 Path 选择由调用者负责。

### 1.1 媒体类型与字节参数

`HttpMediaType(type, subtype, parameters)` 和 `HttpParameter(name, value: List<Byte>, quoted = false)` 是不可变值构造器。type、subtype 和参数名须为非空 ASCII token，统一为 ASCII 小写；参数值独立复制，允许 quoted-string 中的 HTAB、可见字节及 obs-text，拒绝其他控制字节。重复参数名（忽略 ASCII 大小写）一律报错，包括值相同的重复项；未知参数和顺序保留。

`HttpMediaType.parse`／`parseOrNone` 接收字节或 String；String 明确按 UTF-8 转成字段字节，不把接收的任意字节猜成 UTF-8。`parameters` 与 `parameter(name)` 交付去除引号和 quoted-pair 转义后的字节；`HttpParameter.wasQuoted` 记录解析时的引号形式，普通构造使用 quoted 参数，默认 false。显式 quoted 供 Basic realm、Digest 等要求发送 quoted-string 的字段使用；false 仍在非 token 值输出时自动加引号。`originalValue` 在解析结果中保存完整原始字节，在普通构造值中为 None。

解析接受前后 OWS 和 RFC 9110 parameters 中的空分号项，拒绝 type／slash／subtype 之间及参数等号两侧的空白；引号内的分号、逗号和转义字符不作为分隔符。字段内换行、非法 token、未闭合引号、坏 quoted-pair 和多余尾部均报错。`HttpFieldParseException` 保存 field、byteOffset 和 detail；偏移从原始字段值的第一个字节算起。parseOrNone 只吸收这种格式异常。

`toByteArray()` 生成可发送的规范字段值；保留参数次序，必要时加引号并转义引号／反斜杠，已解析的 quoted 参数继续使用引号。`toHttpField()` 生成 Content-Type 字段。两者不隐式解释 charset 参数，也不把 obs-text 转成展示字符串；字符解码由显式 charset 操作完成。

`HttpMediaType.parseFields(fields): HttpMediaType?` 解释单值 Content-Type：没有对应字段时返回 None，一条字段复用 parse；同名字段超过一条即报 HttpFieldParseException，即使值相同也不将它们拼成逗号列表。重复字段的 byteOffset 为第二条字段值的起点零；原始 fields 保持不变。高层只在需要媒体类型时调用，不因文本元数据无效而提前丢弃可按原始字节读取的响应。

### 1.2 Content-Disposition 与扩展参数

`HttpContentDisposition(disposition, parameters)` 与配对的字节／String parse、parseOrNone、parameter、toByteArray、toHttpField 遵守同样的不可变值合同。disposition 和参数名按 ASCII 小写保存；参数顺序和原始字段保留，重复名报错。按 RFC 6266 接收参数等号两侧 OWS，不接收空分号项。未知 disposition 与参数只保存，不猜测业务含义。

`HttpParameter.extended(name, text, language = "")` 生成 RFC 8187 的 `name*` 参数：name 是未加星号的非空 attr-char 序列，编码固定为 UTF-8，值由 URL 百分号 codec 编码。`decodeExtended(allowIso88591 = false)` 返回含 text、language 和实际 Charset 的 `HttpExtendedValue`；接收端默认只接受 UTF-8，显式开启兼容后另接受 ISO-8859-1。禁止 quoted-string 形式、缺失 charset／分隔符、非法 attr-char、坏百分号或无效编码，不做 RFC 2231 continuation 拼接。语言标签检查 RFC 5646 的结构、重复 variant／extension 和固定 grandfathered 形式，不查语言注册表，也不改变原大小写；空语言合法。

扩展参数先按 HTTP 语法分离 charset、language 和编码字节，再调用 url／charset；UTF-8 BOM 作为实际文字保留。`HttpFieldParseException.cause` 可保留原 URL／charset 格式错误；解析得到的参数诊断使用原字段及原始字节偏移，普通构造参数的诊断相对其 value 字节。不吸收内存、后端等非格式错误。

`HttpContentDisposition.filename(allowIso88591 = false)` 返回 `HttpFilename`：text 是可用建议名或 None，language 在有效扩展名被选中时为 Some（可含空字符串），extendedFailure 保存被忽略的无效 filename* 原因。有效 filename* 优先；其缺失或无效时使用普通 filename，普通值按 RFC 6266 的 ISO-8859-1 解码，不猜 UTF-8、不额外百分号解码。仅有无效扩展名时 text 为 None，失败原因仍存在。此结果不转换或清理操作系统路径，下载目标仍由调用者显式选择。

### 1.3 HTTP 日期与 Retry-After

`HttpDate.format(Instant): String` 只生成 IMF-fixdate，使用 UTC 和固定英文星期／月份词；低于秒的部分向下取整。`parse(value, reference: Instant, caseInsensitive = false)`／`parseOrNone` 接收字节或 String，返回 std 的 Instant，完整接收 IMF-fixdate、RFC 850 和 asctime 三种形状。仅字段前后可有 OWS，内部空格、数字宽度和 GMT 按各格式检查；日期须为真实日历日期，星期须相符，年限按 RFC 9110 §5.6.7 引用的 RFC 5322 §3.3 及四位年份语法限定为 1900～9999。格式错误的 field 为 HTTP-date，日期与 UTC 算术复用 std，不查询当前时钟或本地时区。

两位年份使用显式 reference 的 UTC 日历时间：以其后 50 个日历年为上界，选择不晚于上界、末两位相同的最近年份。比较包含月、日和时分秒；reference 的 UTC 年须在 1900～9999 内，只有旧式两位年份需要它。结果仍须满足日期及年份范围。这使世纪选择和 50 年边界可重现，不依赖运行机器的当前年份。

秒接受 00～60；std 的 Instant 不表示独立闰秒，输入 60 映射到同一分钟的 59 秒，不查闰秒表，不把缓存有效期向后延长。`caseInsensitive = true` 按 RFC 9111 的缓存接收建议匹配英文词和 GMT；它不允许其他时区或任意日期形状。输出始终使用规范大小写。

`HttpRetryAfter` 为 `DelaySeconds(Long)` 或 `At(Instant)`；parse／parseOrNone 与 HTTP 日期使用相同 reference 和大小写选项，格式诊断保留 Retry-After 字段名。delta-seconds 必须为非负十进制 Long，逐位检查溢出，不接受符号或小数。`toString()`／`toHttpField()` 输出合法字段；直接构造的负 DelaySeconds 在输出或计算时报告参数错误。

`delayFrom(now): Duration` 将 DelaySeconds 原样表示为 Duration，将日期转换成相对显式 now 的非负等待量，已经过去的日期返回 ZERO。该操作不读时钟、不休眠，不把合法的大秒数先转换成会溢出的纳秒或到期 Instant；实际等待与策略上限属于 client。

### 1.4 字节范围

`HttpRange(ranges: List<HttpByteRange>)` 保存不可变且非空的范围列表。`HttpByteRange` 为 `FromTo(first, last)`、`From(first)` 或 `Suffix(length)`，参数为非负 Long，显式终点不得小于起点；构造 HttpRange 时统一检查。保留重复、重叠和原顺序，不自动合并。`parse`／`parseOrNone` 接收字节或 String，按大小写无关的 bytes 单位解析；接受列表两端、逗号周围 OWS 及列表中的空项，但结果至少有一项。单位与等号之间及区间内部不插入空格。未知单位、非法项、负数和数值溢出报告 Range 字段与原始字节偏移，原始值保存于 originalValue。`toString()`／`toByteArray()`／`toHttpField()` 生成规范字段。

`resolve(completeLength: Long): HttpRangeResolution` 使用非负的已知表示长度，将可满足项转为实际的 `HttpResolvedRange(first, last)` 闭区间；终点截到表示末尾，超长后缀选择整个表示，仍保留输入顺序与重复。结果同时提供 ranges 和 isSatisfiable。按 RFC 9110 §14.1.2，非零 Suffix 在长度为零时仍可满足，此时 ranges 为空；Suffix(0) 不可满足。不得仅凭实际区间列表为空就判断 416。

`HttpResolvedRange` 检查端点及 `length = last - first + 1` 能用 Long 表示；Request Range 的大终点可以在 resolve 时截断，不提前按未截断长度拒绝。范围始终相对于实际表示的字节序列，包含其 content coding，不能擅自改成解压后偏移。

`HttpContentRange(range: HttpResolvedRange, completeLength: Long? = None)` 表示已有范围及已知／未知总长度；已知总长度必须大于 last。`HttpContentRange.unsatisfied(completeLength)` 表示 `bytes */length`，长度可为零。配对 parse／parseOrNone、originalValue 和输出方法接受／生成这两种形状；未知总长度使用星号，unsatisfied 的总长度必须已知。分隔符按字段语法检查，只有字段前后允许 OWS；非法边界、无法表示的区间长度或数值溢出报告 Content-Range 字段。工具不检查响应状态、不读取 body，也不决定断点续传或文件写入位置。

### 1.5 Cache-Control 指令

`HttpCacheDirective(name, value: List<Byte>? = None)` 保存 ASCII 小写指令名及可选的独立字节参数；空字符串参数与无参数不同。`HttpCacheControl(directives)` 保存不可变的有序列表。parse／parseOrNone 接收单个字节或 String 字段，parseFields 接收 HttpField 列表并合并所有 Cache-Control 行；originalValues 保留每条原始字段，普通构造为空。按 RFC 9111 接收 token／quoted-string 参数及转义，保留 wasQuoted、未知指令、重复项、顺序和非 UTF-8 参数；逗号只在引号外分隔，空字段及列表空项允许，等号周围不额外接收空白。

`findAll(name)`／`contains(name)` 按大小写无关名称查询；`single(name)` 返回唯一项或 None，重复时以第二项的原始位置报错，不静默覆盖。语法解析不提前应用缓存策略；调用者可以用 findAll 实现 RFC 9111 允许的首值规则，或将严格查询／参数解释失败视为无效缓存信息。

`HttpCacheDirective.deltaSeconds()` 要求完整的非负十进制参数，接收已解码的两种参数形式；按 RFC 9111 §1.2.2 将溢出饱和到 Long.MAX_VALUE，溢出后的剩余字符仍须是数字。`fieldNames()` 解析 no-cache／private 等指令的字段名列表，接受列表 OWS 和空项，返回小写名称。无参数由 value 的 None 表示，例如无界 max-stale 与不限定字段的 no-cache；调用者不能把它当作空列表或零秒。这两个解释方法缺少参数或遇到非法内容时报告 HttpFieldParseException，解析项的偏移映射回原字段（含 quoted-pair），普通构造项的偏移相对于参数字节。

`toByteArray()`／`toHttpField()` 生成可发送的 Cache-Control。标准秒数指令使用 token，no-cache／private 的字段列表使用 quoted-string；已解析的未知参数保留引号选择并正确转义。输出时检查已知的秒数、字段列表和无参数指令的参数形状，不把读取到的无效标准参数重新生成为合法配置。未知扩展只按通用字节语法处理。工具不判断请求／响应方向、freshness、存储或重验证，也不查询时钟。

### 1.6 ETag 与条件列表

`HttpEntityTag(opaque: List<Byte>, isWeak = false)` 保存不可变的 opaque 字节及强弱标志。opaque 可为空，接受 RFC 9110 etagc 的可见字节（双引号除外）与 obs-text，拒绝空格、控制字节及 DEL。parse／parseOrNone 接收带引号的单个字节或 String 字段及前后 OWS；弱前缀必须精确为 `W/`。反斜杠是普通 opaque 字节，不做 quoted-pair、百分号或字符集转换。`toByteArray()`／`toHttpField()` 原样包围 opaque，生成 ETag 字段；不通过 String 中转任意接收字节。

`stronglyMatches(other)` 只在两者均为强标签且 opaque 逐字节相等时成立；`weaklyMatches(other)` 只比较 opaque。标签值不做大小写归一、Unicode 规范化或摘要推断。

`HttpEntityTagCondition(tags)` 保存 If-Match／If-None-Match 的有序标签列表，允许空列表和重复项；`wildcard()` 单独表示星号条件，不能与列表混合。parse／parseOrNone 接收字节或 String，并可指定用于诊断的 fieldName（默认 If-None-Match）；按列表语法忽略 OWS 与空项，只在标签引号外识别逗号。isWildcard、tags 和 originalValue 暴露不可变结果，`toByteArray()`、`toIfMatchField()`、`toIfNoneMatchField()` 生成对应字段。引号内的星号仍是普通 ETag。工具不把无 ETag 等同于无当前表示，不自行判断条件是否满足；实际表示是否存在及所需比较强度由调用者决定。

### 1.7 认证字段

`HttpAuthChallenge(scheme, parameters)` 与 `HttpCredentials(scheme, parameters)` 分别表示服务端 challenge 和请求凭据；各自的 `withToken68(scheme, token68)` 工厂表示 token68 形式。scheme 和参数名是非空 ASCII token，保存为小写；parameters 独立复制、按原顺序保存，名称不区分大小写且不得重复。token68 与参数列表互斥，token68 只按 RFC 9110 §11.2 检查字符及末尾 padding，不做 Base64 解码或认证有效性推断。未知 scheme、未知参数、空参数列表和空 quoted 值都可表达。

`HttpAuthChallenge.parse` 接收一个 challenge；`parseList` 接收 WWW-Authenticate 的整个列表，`parseFields(fields, fieldName = "WWW-Authenticate")` 合并多行再解析，可用 Proxy-Authenticate 作为字段名。`HttpCredentials.parse` 只接受一个 credentials 值，默认诊断字段为 Authorization，可显式选择 Proxy-Authorization。上述单值入口都有字节／String 和 parseOrNone 形式；列表入口的错误不静默删除无效项。

challenge 和 credentials 共用同一个字节语法实现。参数值使用 token／quoted-string，等号两侧接受 BWS；引号内逗号和 quoted-pair 不分隔项目。scheme 与实际数据之间要求一个或多个 SP，HTAB 只作为字段末尾或列表分隔符的 OWS，不替代这段 SP。列表接受空项；逗号后若是 `token BWS =` 则继续当前参数列表，否则开始下一个 challenge。token68 的等号只允许出现在末尾；例如 `Basic abc==` 是 token68，`Digest realm=x` 是参数。不能用逗号 split、Base64 解码成功与否或已知 scheme 名单来决定语法。

`parameters`、`parameter(name)` 和 `token68` 暴露不可变的已解析值；任意参数字节不经 String 中转。`toByteArray()` 生成合法字段值，保留参数顺序和引号选择；challenge 提供 `toWwwAuthenticateField()`／`toProxyAuthenticateField()`，credentials 提供 `toAuthorizationField()`／`toProxyAuthorizationField()`。特定方案的发送方通过 HttpParameter 的 quoted 选项遵守其额外格式要求，不在 client 中手工拼接或转义字段。

`HttpAuthenticationInfo(parameters)` 处理 Authentication-Info／Proxy-Authentication-Info 的无 scheme 参数列表，提供同样的参数查询、字节输出、parse／parseOrNone、parseFields 和两个明确的字段生成方法。重复参数包括跨行重复都报错，响应头与 trailer 由调用者分别传入。parseFields 按 RFC 的字段组合规则用逗号及一个 SP 连接同名行，因此 challenge 的参数可以分布在多行；其错误偏移相对于组合后的字段字节，单行 parse 的错误偏移相对于原字段。

本层只负责消息语法，不取得密码、刷新令牌、选择认证方案、计算 Digest、检查 rspauth 或执行网络往返。Basic／Bearer／Digest 的具体语义和 suspend 凭据提供者由 client 实现；摘要调用 crypto。

### 1.8 Age 与 Vary

`HttpAge(seconds)` 保存非负 Long 秒数，提供 seconds、originalValue、toString／toByteArray／toHttpField。parse／parseOrNone 接收字节或 String 及首值两端 OWS；完整十进制数溢出时按 RFC 9111 §1.2.2 饱和到 Long.MAX_VALUE，仍检查首值的所有字符。接收列表形式时遵守 §5.1 的首值规则：只解释第一个逗号之前的部分，丢弃之后的内容，不用后续有效数字挽救空或无效的首值。parseFields 返回第一条 Age 的解析结果或缺少字段时的 None，后续同名行不参与选择；parseFieldsOrNone 另将首值格式错误变为 None。普通构造的 originalValue 为 None，解析保留被选中的完整原字段；输出只生成一个规范数字。没有 Age 不表示 origin 一定被访问，工具不推算缓存年龄。

`HttpVary(names)` 保存按 ASCII 小写规范化的不可变、有序 token 列表，保留重复名和 `*`；isWildcard 表示任一项为星号，不能以列表还含其他字段名为由忽略它。parse／parseOrNone 接收字节或 String，接受 OWS、空字段和列表空项，拒绝引号、参数、非 ASCII 名称及缺少逗号的结构；parseFields 合并所有 Vary 行，originalValues 保留各条字段。contains(name) 按规范名称判断实际列表成员，不将星号解释成所有名称都在列表中。toByteArray／toHttpField 保留名称顺序并以逗号和 SP 输出；没有匹配行时返回空列表。Vary 只描述选择字段，是否能复用以及缺失字段与空字段的区别由缓存策略处理。

两个工具的格式异常继续使用 HttpFieldParseException 和原字段字节偏移；OrNone 只吸收格式错误。Vary 与 Cache-Control 字段名参数共享相同的 token 列表语法，Age 与缓存秒数共享饱和十进制解析，Range／Retry-After 的溢出拒绝规则保持不变。

### 1.9 通用字段操作与表示长度

`isHttpToken(value)` 接收 List<Byte> 或 String，检查非空 ASCII token；String 按 UTF-8 转成字节，不执行 Unicode 大小写折叠。`HttpField.hasName(name: List<Byte>)` 复用字段名的 ASCII 不区分大小写比较。`field.validate()` 检查普通字段名为 token，值只含 HTAB、可见字节或 obs-text；伪头、CR／LF／NUL 等控制字节拒绝。HttpField 本身继续保存原始字节，构造不隐式过滤接收端需要观察的数据。

`parseHttpTokenFields(fields, name: String): Array<String>` 只适用于 Connection、Content-Encoding、Trailer 等由大小写不敏感 token 列表组成的字段。它复用字段 reader，合并同名字段并保持值的原顺序与重复项，将 ASCII token 规范为小写，忽略逗号列表中的空项；无字段或只有空项返回空数组。quoted-string、参数、非 ASCII token、非法分隔和控制字节报 HttpFieldParseException，field 为所选名称，byteOffset 相对发生错误的那条字段值。该函数不解释 Accept 等带参数的列表，也不合并 Set-Cookie；调用方根据具体字段要求处理空列表。

`parseHttpContentLength(fields): Long?` 返回不存在或实际非负长度；接受前后 OWS、重复字段与逗号合并后数值完全相同的十进制值，包含前导零时按数值比较。空值／空列表项、符号、非法尾部、超过 Long 或互相矛盾的长度均报 HttpFieldParseException。它只解释字段，不推断 HEAD／204／304 的正文语义，不代替协议引擎的消息定界。

### 1.10 Expect 与 Continue

`httpExpectsContinue(fields): Boolean` 按 RFC 9110 §10.1.1 读取全部 Expect 字段，仅在存在不带值或参数的 `100-continue` 项时返回 true，名称不区分 ASCII 大小写。它接收逗号列表的空项、未知 expectation 的 token／quoted-string 值，以及值之后的 parameters；引号内逗号、分号与 quoted-pair 按同一个字段 reader 处理。未知 expectation 和参数保留原字段，不在本工具中实现扩展策略；未知参数允许重复，参数等号周围不接受空白。

缺少 Expect、只有空项或只有未知 expectation 时返回 false。`100-continue=value` 是带值的其他 expectation，不作为标准 Continue 信号。即使此前已经找到有效 100-continue，后续非法结构仍报 HttpFieldParseException，field 为 Expect，byteOffset 相对发生错误的那条原始字段值。工具不启动 producer、不查询时钟，也不决定何时发送 body；这些等待行为由 client 组合。

### 1.11 Trailer 声明与发送限制

`HttpTrailerDeclaration(names)` 表示发送方预先声明的 trailer 名称，复制输入、按 ASCII 小写规范化，并按首次出现顺序去重；名称必须为普通 HTTP token。空声明合法，toHttpField 生成 Trailer 字段，contains 按 HTTP 名称规则查询。接收的 Trailer 字段仍用 parseHttpTokenFields 读取，不以发送声明的限制丢弃原始接收信息。

requireDeclared(fields) 只检查本次实际 trailer 的名称均在声明内，允许少于声明的名称、空结果和有序重复字段，不合并值；字段值与实际协议的编码校验仍在发送边界进行。声明不创建全局字段注册表；按 RFC 9110 §6.5.1，调用者须确认扩展字段的定义允许放在 trailer 中。

声明与 HTTP/1、HTTP/2 编码器共用已知不可后置字段的检查：拒绝消息定界、连接／路由、认证／Cookie、请求控制与条件、响应控制及内容格式字段，包括 Content-Length、Transfer-Encoding、Connection、Host、Trailer、Authorization、Cookie、Expect、Range、If-* 条件、Cache-Control、Location、Content-Type、Content-Encoding 和 Content-Range。Authentication-Info、Content-Digest、ETag 及调用者已确认允许的扩展字段可以声明；扩展字段不会因此被赋予库不知道的语义。失败在本次协议结束标记提交前报告，编码器仍可接受修正后的合法 trailer。

### 1.12 响应内容语义

`httpResponseAllowsContent(head, requestMethod): Boolean` 查询已解析消息的 HTTP 内容语义。HEAD、1xx、204、205、304 和成功 CONNECT 的响应返回 false；方法按字节精确匹配，其他扩展方法沿用普通响应规则。它不以 Content-Length 是否存在或大于零判断内容，也不把 CONNECT 隧道数据或升级后的新协议数据当成 HTTP 表示。

该查询供内容解码等上层组合使用，不代替各版本的报文定界、原始方法／状态检查或接收兼容选项。HEAD／304 的 Content-Encoding、Content-Length 等可以描述所选表示，但不能据此对实际空 body 启动解压；原始字段仍保留。

## 2. Cookie 与公共后缀

Cookie parser 接收单个 Set-Cookie 字段，按 RFC 6265 及现代 SameSite／前缀规则返回名称、值、Domain、Path、Expires、Max-Age、Secure、HttpOnly、SameSite 和未知属性。Cookie 请求字段的组装独立处理，不把多个 Set-Cookie 用逗号连接。域名使用 url／idna 的同一规范值。

提供无状态的 domain-match、path-match、默认路径、过期和前缀检查。Max-Age 优先于 Expires，正负边界和日期溢出按 Cookie 规则处理。Domain 不能扩大到公共后缀；IP host 使用精确地址匹配。SameSite 的站点输入来自调用者，不从连接地址或 TLS SNI 猜测页面上下文。

公共后缀工具使用 Public Suffix List 的 ICANN 与 PRIVATE 部分，支持普通、通配与例外规则，返回 public suffix 与可选 registrable domain。数据以固定快照随 HTTP 库交付，记录来源、日期、revision 和 MPL-2.0 许可；维护时更新，不在请求中下载。域名预先完成 IDNA 规范化，匹配器不复制 Unicode 算法。

PSL 数据访问是普通库逻辑，不生成 compiler-known 表或额外产物验证机制。Cookie jar 的内存状态、持久化和请求发送选择属于 client；这里不维护全局 Cookie 状态。

### 2.1 公共后缀匹配

`PublicSuffixList.bundled()` 读取随库固定交付的 PSL，返回可复用的不可变规则索引；`parse(text)` 可读取显式提供的 PSL 文本，保留 ICANN／PRIVATE 分区和可选的 VERSION／COMMIT 元数据。忽略空行、注释和规则后的空白说明，域名部分通过 idna 规范化；规则只允许普通名称、最左端 `*.` 和最左端 `!`，例外规则至少包含两个标签。重复规则合并。格式错误报告 PublicSuffixParseException 的行号，IDNA 后端运行错误照常传播。

`lookup(domain: String, includePrivate = true)` 规范化输入域名；`lookup(name: IdnaName, includePrivate = true)` 复用其已验证的 ASCII 值。两者返回 PublicSuffixMatch，包含规范 domain、publicSuffix、可选 registrableDomain、可选 rule 和 isPrivate。末尾根点不参与匹配或返回值；独立根域不是可查主机名。rule 为规范 A-label 规则，包含实际的通配／例外前缀；未命中数据时采用 PSL 规定的隐式 `*`，rule 为 None。先选例外，否则选标签数最多的规则；完整域名自身是公共后缀时 registrableDomain 为 None。

`lookup(origin: UrlOrigin, includePrivate = true): PublicSuffixMatch?` 直接消费 URL 已完成规范化的 DNS 主机名；IP 返回 None，不将地址的最后一个数字当作域名后缀。匹配器不重复 URL 的 IDNA 校验。`lookupOrNone(domain, includePrivate = true)` 只将无效域名或独立根域转换成 None，不吞掉后端错误。PSL 只判断注册边界，不判断域名是否已注册、是否可解析或是否受信任。

规则匹配用普通 Scoop 集合和字符串实现。原始 PSL 与 MPL-2.0 许可随 vendor 交付，维护脚本将相同规则和分区嵌入本 Cone 的普通静态数据；本地构建和每次请求不下载数据，不引入新的编译器资源机制。每个 PublicSuffixList 对象构造一次索引，之后只读，可在独立请求与线程间复用。version、revision 与 ruleCount 记录实际加载的快照；这些信息仅用于维护和诊断。

### 2.2 Cookie 字节与日期

接收算法以 RFC 6265 §5 为基础，采用 [draft-ietf-httpbis-rfc6265bis-22](https://www.ietf.org/archive/id/draft-ietf-httpbis-rfc6265bis-22.html) 的控制字符、无名称值、4096 字节名称／值上限、1024 字节属性值上限、SameSite 和前缀规则。该草案是本期明确选择的兼容基线，不把它称为已发布 RFC；浏览器文档、worker 和非 HTTP 脚本 API 不属于本库。

`HttpCookiePair(name, value)` 保存不可变字节；构造器另接受 String 并显式按 UTF-8 编码。名称保持大小写，值保留双引号、反斜杠和百分号字节，不执行 quoted-pair、百分号或字符集解码。名称不得含等号，名称和值不得含分号或除 HTAB 外的控制字节；普通构造不接受两端 WSP，以免发送时产生与构造值不同的意义。接收端按算法去除两端 WSP，允许内部空白和 obs-text；无等号时表示空名称及完整值。名称与值合计不得超过 4096 字节，空名称与空值可作为解析数据表达，但不能成为被接纳的 Cookie。

`HttpCookieHeader(pairs)` 保留 Cookie 请求字段中的顺序与重复名称。parse／parseOrNone 解析分号分隔的字节／String 字段，忽略空项；parseFields 将多条 Cookie 按 Cookie 自身的分号规则处理。`toByteArray()`／`toHttpField()` 按原顺序输出 `; ` 分隔的 cookie-string，空名称时只输出值；不合并或去重。Cookie 的生成字段设置 neverIndex。空名称序列化遵循接收算法的表示，不能据此承诺所有非规范 Cookie 均能解析回相同的名称／值分界。

`HttpCookieAttribute(name, value = None)` 保存属性名和值的独立字节，None 与空值不同；未知属性、重复属性和顺序保留。`HttpSetCookie(pair, attributes)` 表示一条完整 Set-Cookie，配套 parse／parseOrNone、originalValue、toByteArray／toHttpField。属性以分号分隔，等号只分隔第一次；属性引号不赋予转义语义。解析时拒绝整条字段中的禁止控制字节；超过 1024 字节的属性值仍可从原属性列表观察，但不参与标准属性解释。Set-Cookie 输出也设置 neverIndex，各条字段始终独立处理。

`HttpCookieDate.parse`／`parseOrNone` 按 Cookie 日期 token 算法接收字节或 String，无需参考时刻：忽略星期、分隔符与未识别 token，按规定顺序选择首次出现的时分秒、日、月、年，接受各 token 的规定尾部。两位年份 70～99 映射到 1970～1999，00～69 映射到 2000～2069；年份限 1601～9999，日历日期必须存在，秒最大 59。不能复用 HTTP-date 的三种严格形状或 50 年参考窗口。格式错误报告 Expires 字段及字节位置；format 生成 UTC 的 IMF-fixdate 形状，支持同样的 Cookie 年限。UTC、日历和最终日期构造复用 std。

### 2.3 标准属性与作用域

HttpSetCookie 的 expires、maxAgeSeconds、domain、path、secure、httpOnly 和 sameSite 在构造／解析时解释一次。无效 Expires／Max-Age 仅忽略本项，不覆盖较早的有效属性；其余重复标准属性采用最后一个可用项，Secure／HttpOnly 只判断属性是否存在。Max-Age 接受完整的可选负号十进制，数值溢出饱和到 Long 边界后仍检查剩余数字。Domain 去掉一个前导点并进行 ASCII 小写转换，Path 保留字节；二者仍须在具体 URL 上决定作用域。SameSite 为 Default／Strict／Lax／None，缺失或未知值为 Default，参数匹配不区分 ASCII 大小写。

`expiryFrom(now, maxLifetimeSeconds = 34560000): Instant?` 使用显式 now 计算过期：Max-Age 优先，零或负值表示立即过期，缺少两种有效属性表示会话 Cookie（None）。持久期限最多为配置的相对上限，默认 400 天；配置必须非负。所有加法受检并在 std Instant.MAX 饱和，不先把大秒数转换成纳秒，也不查询时钟。

`resolve(url, now, suffixes, maxLifetimeSeconds = 34560000)` 返回标准库的 `Result<HttpCookie, HttpCookieRejection>`，Success 表示接纳，Failure 表示正常拒绝。结果 HttpCookie 保存 pair、规范 host、hostOnly、path、expiresAt、secure、httpOnly 与 sameSite；该操作无全局状态。缺失／空 Domain 产生 host-only；非空 Domain 必须是 ASCII，并由 idna 或 platform 的 IP parser 验证及规范化，禁止尾随根点。域名按完整标签边界匹配，IP 按地址精确匹配；Domain 为公共后缀时，只有它等于当前 DNS host 才退化为 host-only，否则拒绝。PSL 同时使用 ICANN 与 PRIVATE 部分。

没有以 `/` 开头的 Path 使用 URL 已编码路径的默认目录；路径匹配不做百分号解码或目录规范化。只有 https／wss 被本工具视为安全传输。Secure Cookie 不从明文 URL 接纳，SameSite=None 还要求 Secure。`__Secure-`／`__Host-` 按接收算法不区分 ASCII 大小写检查，但名称本身保持原大小写；前者要求 Secure，后者还要求解析后的 host-only、Path 属性存在且最终路径为 `/`。空名称的值不得冒充这两种前缀。拒绝原因区分空名称／值、非法域名、域名不匹配、公共后缀、不安全传输、SameSite 和前缀问题，不把正常拒绝变成网络失败。

### 2.4 发送与 SameSite 的无状态判断

`HttpCookie.isExpired(now)` 和 `matches(url, now)` 判断期限、主机／域名、路径和 Secure。`HttpCookieRules.domainMatches`、`defaultPath`、`pathMatches` 是相同算法的独立入口；domainMatches 使用已规范化 UrlHost，defaultPath／pathMatches 使用已编码路径字节。它们不读取 DNS、Cookie jar 或文件。

`allowsSameSite(isSameSiteRequest, isTopLevelNavigation, isSafeMethod)` 在 Strict 时只接受同站请求，在 Lax／Default 时另接受使用安全方法的顶层导航，None 不限制。`allowsSetting(isSameSiteRequest, isTopLevelNavigation)` 表达对应的设置规则。调用者必须传入真实的站点与导航判断；本层不将“同一个连接”解释为同站，不提供 Lax-Allowing-Unsafe 的时间例外。没有浏览器上下文时，client 按其显式 Cookie 策略提供限制性的输入。Cookie 的存储键、排序、替换、持久化及明文覆盖既有 Secure Cookie 的检查仍属于有状态 Cookie jar。

## 3. 表单与 multipart

application/x-www-form-urlencoded 使用 URL 库的 FormUrlEncoded，保留重复项和顺序，按明确 charset 编码。HTTP 层提供 Content-Type 与 RequestBody 所需的元数据，不重新定义加号、百分号或空值规则。

Boundary、part 元数据、增量内存引擎和局部限制的详细合同见 [multipart 规范](SCOOP-HTTP-MULTIPART-SPEC.md)。form-data 工厂按 RFC 7578 生成普通 filename，不生成 filename*；通用字段与其他 multipart 类型的扩展参数仍由 Content-Disposition 工具处理。

multipart 内存引擎按 RFC 2046／7578 增量处理 boundary、每 part 字段与内容，不把 body 当成文本。提供已知长度计算、编码与解码；仅所有组成数据长度已知时才宣告完整 contentLength。boundary 可显式指定，自动生成使用 crypto 随机字节；不扫描无限上传源预先寻找碰撞。

异步编码／解码在 scoop.http.asyncio 组合 AsyncByteSource／AsyncByteSink，支持每 part 文件／字节／流、filename／filename*、重复字段和嵌套内容的原始交付。每个 part body 在推进下一 part 前必须完成或明确放弃；提前关闭整体消息关闭所拥有的响应，不能无界排空网络。header 和 boundary 跨任意输入块仍正确识别，正文出现部分匹配不得丢字节。

单 part 字段大小、part 数和 boundary 长度按实际局部限制配置，不建立通用内容预算。multipart 的重放能力由各 part 的 body factory 组成，只要有已消费且不能重建的 part，整体不能自动重放。

## 4. CONNECT 与切换辅助

提供 HTTP 代理 absolute-form 与 CONNECT authority-form 的正确请求构造，复用 URL 的目标、端口和 IPv6 表示。CONNECT 的成功判断与定界在 HTTP 层，普通拒绝响应仍按正常消息交付；client 决定认证、重试和目标 TLS。

成功 CONNECT、101 Upgrade 的异步移交始终包含全部未消费后缀以及 AsyncByteStream／AsyncCloseable 所有权。旧连接停止协议处理，关闭旧对象不关闭已移交传输。不得把失败响应的一部分 body 当成隧道数据。

### 4.1 代理请求与 CONNECT

`httpProxyRequest(url, method = "GET" 的字节, fields = 空列表, version = Http11)` 构造 HTTP/1.0 或 HTTP/1.1 的 absolute-form 请求。url 必须为具有有效网络主机与端口的 http／https URL；target 复用 URL 的 httpAbsoluteTarget，保留编码路径、查询和显式端口，去除 userinfo 与 fragment。method 接受除精确 CONNECT 外的任意合法 token，不改写大小写。Host 复用 UrlOrigin 的规范主机，URL 显式给出端口时保留该端口，否则省略默认端口；userinfo 不进入 Host，也不隐式生成 Authorization。

`httpConnectRequest(url, fields = 空列表, version = Http11)` 只使用 url 的有效网络 origin，支持 http／https／ws／wss，忽略路径、查询与 fragment。CONNECT target 通过 UrlOrigin.authority(true) 生成，始终含实际端口，IPv6 带方括号；HTTP/1 使用 authority-form 与相同 Host，HTTP/2 使用已有普通 CONNECT 的 :authority 模型，不包含 :scheme／:path。两种构造器均检查普通字段的 token／控制字节，按原顺序保留重复项和 neverIndex；Host 由工具生成，调用者不得另传 Host 或伪头。它们不选择代理、读取环境、生成凭据、连接或发送请求。

`httpConnectAccepted(response)` 在状态码为 200～299 时返回 true，其余状态返回 false。调用者必须将它用于已关联 CONNECT 请求的响应；它不替代请求上下文、最终响应选择或协议定界。拒绝响应继续通过正常 body 读取，成功后仍须完成空 body 才调用 AsyncHttp1Connection.takeTransport。移交复用第 4 节及基础 HTTP 规范的既有生命周期合同，HTTP/2 的普通 CONNECT 保持为单个双向 stream，不进行整个连接移交。

## 5. SSE

`SseDecoder` 为增量内存 parser，负责 UTF-8、开头 BOM、CR／LF／CRLF、注释、空行分隔、data 多行连接、event、id 与十进制 retry。UTF-8 错误按 SSE 规定替换，跨块残缺序列保留到后续输入；最终 EOF 按标准处理未完成事件，不自行补空行。

`SseEvent` 保存 event、data、id 等实际字段；id 的空值与未出现不同，含 NUL 的 id 按协议忽略，非法 retry 不替换已有重连间隔。parser 不连接网络、不休眠、不维护自动重连任务。

异步 `SseSource` 借用或显式接管 AsyncByteSource，`next(): SseEvent?` 可挂起，消费驱动读取。maxLineBytes／maxEventBytes 约束实际累计文本，超出报具体错误。HTTP 状态、媒体类型、Last-Event-ID 和重连策略由 client 管理。

### 5.1 增量解析与事件数据

`SseDecoder(limits = SseLimits(), initialLastEventId = "")` 实现 Closeable，串行使用。`receive(buffer, count = buffer.readableBytes)` 消费 NativeByteBuffer 的实际前缀并返回字节数；`nextEvent(): SseEvent?` 取走一个事件。引擎至多保存一个待交付事件，形成事件后立即停止消费；在取走它之前 receive 返回零，不无界累计事件队列。零长度仍检查状态与参数，非法参数不使引擎失效；解析推进失败后保存 failure，后续推进报状态错误，close 仍可调用。

`SseLimits(maxLineBytes = 65536, maxEventBytes = 1048576)` 的两个值须非负。maxLineBytes 限制一行的原始字节数，不含 CR／LF，开头 BOM 计入；maxEventBytes 限制当前待派发 data 合并后的 UTF-8 字节数，包含行间 LF，不含 event／id 元数据。元数据每条本身受行上限约束。超限报告含 kind、limit 的 SseLimitException；这些限制针对实际行和事件，不计算通用成本。

CR、LF 与 CRLF 可以跨任意输入边界。UTF-8 的 CR／LF 字节不会出现在有效多字节序列内部，因此先保留完整行字节，再由 charset 执行有损 UTF-8 解码；跨块残缺字符不会提前替换，单行内的非法序列使用同一字符集实现处理。只剥去整个流开头的一个 BOM，后续 BOM 保留。字段名逐字、区分大小写；冒号只分隔第一次，值开头最多剥去一个 SP，不剥 HTAB 或更多空格。

`SseEvent` 保存 event、data、id、lastEventId 四个不可变文本值；event 缺失或为空时为 `message`。data 字段按 LF 连接，有一个空 data 字段也会派发 data 为空的事件；没有 data 字段的块不派发事件。id 为本块最后一个有效 id 的可选值，None 表示本块未提供，Some("") 表示显式清除；含 NUL 的 id 被忽略。lastEventId 是派发时的有效 ID。decoder 的 lastEventId 也在无 data 的空行分界处更新；ID buffer 在完整块之间保留，event／data 和本块 id 标记在每个空行后重置。initialLastEventId 供调用者显式延续已有流的状态，不得含 NUL／CR／LF。

`retryMilliseconds` 保存最后一个有效 retry，初始为 None；它在行结束时更新，不要求该块有 data。值只允许非空 ASCII 十进制，零合法，超过 Long 时饱和到 Long.MAX_VALUE，溢出后的其他字符仍使整项无效。注释、未知字段、坏 retry 不改变已有状态；实际等待与上限由 client 决定。

`endInput()` 幂等地声明 EOF，丢弃未结束的行和没有最终空行的块，不补发事件，也不提交不完整块的 id；已形成但未取走的事件继续可取。isFinished 在 EOF 且没有待交付事件时为 true。close 释放行与事件引用，不关闭调用者 buffer，不执行 I/O；lastEventId、retryMilliseconds 和 failure 继续作为结果快照可读。重连、HTTP 状态、URL 和应用回调均不进入这个内存引擎。

### 5.2 可挂起事件源

`scoop.http.asyncio.SseSource(source, limits = SseLimits(), initialLastEventId = "", bufferSize = 8192)` 借用 AsyncByteSource；`SseSource.takeOwnership` 另要求 AsyncCloseable，并在工厂成功返回时接管。bufferSize 必须为正，配置、当前 EventLoop／活动 Job 和登记前取消检查失败时端点仍归调用者，已分配的本地资源释放。没有后台读任务，构造不读取输入。

`next(): SseEvent?` 在同一 EventLoop 和活动 Job 上执行，每次最多一个应用读取。它先消费已有缓冲和 decoder 事件，再按需要挂起读取 source；完整 EOF 返回 None，后续 next 不重复读取。复用内存 parser 的行、UTF-8、ID、retry 和限制规则，不重新解释正文；source 每次返回的字节数必须与缓冲区实际写入一致。已读取但尚未解析的前缀最多为 bufferSize，decoder 最多保留一个事件，不无界预取。借用模式在关闭时也会放弃已预取的前缀，不能假定底层位置仍停在最后交付的事件末尾。

isFinished、isClosed、lastEventId、retryMilliseconds 和 failure 公开已有状态；前两者区分正常 EOF 与本地关闭，metadata 在关闭后仍可读。解析或实际读取失败／取消后保存 failure，不能继续推进；参数、错误 loop、登记前取消或同向并发失败不损坏尚未启动的流。下层 I/O 异常保留原类型，SSE 解析失败保留具体异常，已消费事件不回滚。

每次 next 的实际 I/O 使用结构化子作用域。close 先拒绝新读取并取消该操作，等待它及原生 buffer borrow 真正结束，再释放 decoder／buffer 和关闭所接管端点；借用模式不关闭 source。并发 close 等待同一次收尾，后续幂等，取消的 finally 仍可完成；正文与清理同时失败由 useAsync 保留为 ResourceUseException。关闭解除端点、Job 和等待 continuation 的引用，不自动读取剩余正文、不重连、不执行用户回调。HTTP/1 未完成 body 的放弃、HTTP/2 单流取消继续由下层 HTTP body 的原有合同处理。

## 6. HTTP/1.1 WebSocket

握手、扩展字段与协商的详细 API 见 [WebSocket 规范](SCOOP-HTTP-WEBSOCKET-SPEC.md)；实际实施进度仍分功能记录。

握手工具生成和校验 RFC 6455 的 Upgrade、Connection、Sec-WebSocket-Key／Accept／Version、子协议与扩展字段，SHA-1 及随机数调用 crypto。成功响应必须绑定本次 key，不能只看到状态 101 就移交。响应选中的子协议／扩展必须属于本次有效提议。

帧引擎独立于 socket，处理 FIN／opcode／RSV、16／64 位长度、mask、分片和控制帧，正确检查最高长度位、控制帧大小、UTF-8 与 Close code。客户端发帧使用新的随机 mask；接收方向检查对端的 mask 角色。不能把单个大 frame 强制全量缓存；分片消息可分块交付，UTF-8 验证跨片连续。

`AsyncWebSocketSession` 位于 scoop.http.asyncio，显式接管升级传输与前缀。send／receive／ping／shutdownGracefully 为 suspend，close 为本地 AsyncCloseable 收尾；文本、二进制、分片与消息流使用明确的接口。每方向至多一个应用操作，内部控制帧按协议穿插；消息和内部队列都有实际有限容量及背压。

控制帧处理与用户回调分离。收到 Ping 能在应用发送过程中安排 Pong；Close 握手有独立超时。已接受的消息不能因取消而重发；异常中断发送会结束该 WebSocket 连接。close 可在取消的 finally 中完成，不等待无限期 peer Close，不自动重连。

permessage-deflate 显式开启，协商 client／server 的 no_context_takeover 与 max_window_bits；只有后端实际支持的组合才能接受。协议引擎负责 RSV1、压缩消息与控制帧的边界，异步适配调用 compression 的 RawDeflate 消息接口，不自行剥尾、构造同步 flush 或访问 zlib。接收大小限制按解压后的消息累计量判断，压缩后的长度不能替代它。

不实现 HTTP/2 Extended CONNECT、WebSocket over H2 或 HTTP/3。这些不是升级失败后的自动回退路线。

## 7. 验证与依据

所有字段 parser 有合法、非法、重复、quoted、非 ASCII 和分块向量。Cookie／PSL 覆盖通配与例外、国际域名、私有后缀、过期与前缀。multipart 覆盖 boundary 部分匹配、二进制内容、超限与提前关闭；SSE 覆盖 UTF-8、行尾和 EOF；WebSocket 与独立对端互操作，覆盖握手、mask、控制帧、分片、压缩、背压与取消。

这些库测试独立于 client 的策略测试；真实组合再覆盖 CONNECT 后缀、上传 multipart、SSE over H2 与 WebSocket over TLS。三个目标使用 debug／release／moving GC 和跨 Cone 消费者验证，不以文档存在表示已实现。

依据：[RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html)、[RFC 6265](https://www.rfc-editor.org/rfc/rfc6265.html)、[Public Suffix List](https://publicsuffix.org/list/)、[RFC 8187](https://www.rfc-editor.org/rfc/rfc8187.html)、[RFC 7578](https://www.rfc-editor.org/rfc/rfc7578.html)、[SSE 标准](https://html.spec.whatwg.org/multipage/server-sent-events.html)、[RFC 6455](https://www.rfc-editor.org/rfc/rfc6455.html)、[RFC 7692](https://www.rfc-editor.org/rfc/rfc7692.html)。
