# 主题与示例内容清单

状态：首批 24 个主题已交付；第二、三节保留为后续内容规划。日期：2026-10-27。命令和编辑约定见 [DESIGN.md](DESIGN.md)，实际输出形态见 [SAMPLES.md](SAMPLES.md)。

首批表中的“复用”“新增”“拆出”记录内容来源与编辑方式，均已落实到[发布索引](../../sysroot/share/scoop/examples/catalog.toml)。后续规划未注册到索引。

路径约定：表中现有来源的链接指向真实文件。新增的语言/项目程序放在 `sysroot/share/scoop/examples/programs/<主题 ID>/`；原库的专门示例仍可放回该库 examples，再由 catalog 引用。

## 1. 首批 24 个主题

首批覆盖从单文件入门到数据处理、库调用、异步 HTTP 和项目测试。每个条目的“必须说明”是这页的编辑重点；其他细节通过相关主题继续查阅。

### 1.1 入门与项目

| ID | 用户的问题与完整程序 | 应观察到的结果 | 必须说明 | 来源与编辑工作 |
| --- | --- | --- | --- | --- |
| start/hello | 怎样编写和运行第一个程序？普通 main 打印问候 | 固定一行 Hello, Scoop! | 文件扩展名、普通 main、默认 core/std；check 与 run 各做什么 | 新增最小 main.scoop |
| start/single-file | 一个文件怎样使用额外的库？完整 JSON 编解码程序 | JSON 文本、解码字段和默认可选字段 | //! dep 必须在语言 token 前；精确坐标；import 仍需单独写；不读取相邻 Cone.toml | 复用 [typed.scoop](../../sysroot/src/scoop.json/examples/typed.scoop)，解释重点是输入组织；JSON 行为链接到 json/typed |
| start/cone | 怎样组织多个源文件的应用？Cone.toml、main 和普通辅助函数 | 跨文件函数调用的固定问候 | package/group 对齐；src 默认发现；同 Cone internal 可见；从项目根 run；用到 std 时显式依赖 | 新增完整两源码 executable Cone；说明现有 scoop new 的命令 |

### 1.2 语言基础

| ID | 用户的问题与完整程序 | 应观察到的结果 | 必须说明 | 来源与编辑工作 |
| --- | --- | --- | --- | --- |
| language/overview | Scoop 的常用写法长什么样？值类型、匹配与 Option 的短概览 | point=(3, 4)、point missing、Ada、guest、3 | Kotlin 差异速查；只是概览，每个概念有单独主题 | 复用 [values.scoop](../../sysroot/src/scoop.core/examples/values.scoop) |
| language/option | 怎样表示和处理缺失值？Some/None、when 解构、Elvis 和安全读取 | 有值、缺失、属性读取三条路径 | 没有 null；T? 是 Option；没有判空智能转换；安全调用保留嵌套层数 | 从 values 的可选值部分另写聚焦程序；[样稿](SAMPLES.md#1-languageoption) |
| language/struct | 怎样定义数据值并产生更新后的副本？Point 的构造、解构、字段更新 | 原 Point 保持不变，新 Point 只有指定字段不同 | val 字段；p.{ x: ... } 的实际语法；引用字段仍可引用同一对象；不自动得到 ToString/Hash | 新增完整程序，沿[语言 §4.1、§4.5、§4.6](../specs/SCOOP-SPEC.md#4-值类型) |
| language/enum | 怎样给几种结果携带不同数据？查找结果的 Found/Missing 与 when | 两个变体走各自的分支 | enum 与 Kotlin enum class 的差异；case 语法；穷尽性；载荷解构 | 从 [values.scoop](../../sysroot/src/scoop.core/examples/values.scoop) 拆出单独程序，不同时讲 Option |
| language/functions | 怎样定义和调用函数？同一问候函数展示表达式体、默认值、命名参数 | 默认问候与指定参数的固定输出 | 参数名参与命名调用；缺省表达式的求值；块体返回与表达式体；vararg 只简要引导 | 新增，无需外部库 |
| language/lambdas | 怎样传递回调和捕获状态？应用变换函数、捕获 val、用显式对象保存可变状态 | 回调计算结果和共享对象的变化 | 函数类型；尾随 lambda；捕获外层 var 非法；局部 return 标签；普通/suspend lambda 区别 | 新增；遵守[语言 §8.1](../specs/SCOOP-SPEC.md#81-函数类型函数值与-closure)，不把全部高阶函数形式塞进同一程序 |
| language/interfaces | 怎样让不同类型提供同一种能力？两个实现通过接口调用 | 对同一接口变量调用得到各自结果 | 显式 interface conformance；公开 contract 与 public override；值类型也能实现接口 | 新增小型实际能力接口与 class/struct 实现 |
| language/generics | 怎样让函数和容器处理不同类型？Box<T> 与受接口 bound 约束的普通函数 | 两种实际类型的结果 | 完整类型实参/推导；nominal 泛型不变性；单态化；不提供 interface 方法自身的泛型 | 新增；不从性能 fixture 取大型泛型链 |
| language/visibility | 怎样把库 API 提供给另一个 Cone？一个小库和一个消费者 | 消费者调用公开构造器与公开方法成功 | 默认 internal；public class 不自动公开 constructor；public interface 和 override 的修饰符；package 不决定 Cone 可见性 | 新增真实两个 Cone，含全部 manifest 和相对 path 依赖；用完整消费者验证公开调用；不扩展编译器测试 |
| language/companion | 怎样给类型提供工厂或 codec 能力？普通 companion 工厂通过类型名调用 | 工厂构造的数据字段 | object 的单例语义；companion 与实例成员；需要公开时显式 public；指向 json/typed 的实际用途 | 新增正常工厂程序，不把 JSON 合成作为唯一的 companion 解释 |
| language/errors | 怎样处理解析失败并保证收尾？一条成功路径和一条抛异常路径 | 成功值、捕获的错误说明、finally 的确定输出 | throw/catch/finally；Exception.message 的 Option；OrNone 和抛异常是各 API 的明确选择；不引入 Kotlin Result API 假设 | 新增受控输入程序；只捕获该任务的实际异常 |

### 1.3 数据与常用任务

| ID | 用户的问题与完整程序 | 应观察到的结果 | 必须说明 | 来源与编辑工作 |
| --- | --- | --- | --- | --- |
| text/strings | 怎样构造和遍历文本？f-string、字符迭代、StringBuilder | 插值结果与明确的 Unicode 标量/字节数 | 普通字符串不做 $ 插值；UTF-8、Char 与 Long 索引；字素与标量不同 | 从 [core collections](../../sysroot/src/scoop.core/examples/collections.scoop) 拆出文本例子，核对当前 String API 名称 |
| collections/list | 怎样累积元素并得到只读快照？ArrayList → Array，再修改原列表 | 快照仍为原值，列表为新值 | val binding 与容器可变性不同；Array/MutableArray；toArray 是浅快照；索引为 Long | 从 [core collections](../../sysroot/src/scoop.core/examples/collections.scoop) 拆出纯容器例子 |
| collections/map | 怎样统计词频并处理不存在的键？LinkedHashMap 计数 | red=2、blue=1，保持插入顺序 | get 的 Option；缺失值默认值；字典接口的 Hash/Equals 要求不能任意套给自定义类型 | 从 [std collections](../../sysroot/src/scoop.std/examples/collections.scoop) 拆出词频程序 |
| io/resources | 怎样在成功和异常路径释放句柄？临时文件的写入/读取置于 use 中 | 文件内容可读；受控异常之后仍完成关闭和临时目录清理 | use 与 useAsync；借用与拥有；close、flush/finish 的不同职责；正文与清理同时失败的规则 | 新增真实文件句柄程序，参考 [file_stream.scoop](../../sysroot/src/scoop.compression/examples/file_stream.scoop)，不引入压缩算法 |
| io/files | 怎样写入、追加和遍历文件？在临时目录中操作 notes.txt | 两行文本、bytes=26、notes.txt | Path/OsString；entries 的 use；临时目录 finally 清理；没有硬编码用户路径 | 直接复用 [files.scoop](../../sysroot/src/scoop.platform/examples/files.scoop) |
| json/typed | 怎样把 struct 编码成 JSON 并解码回来？User 的数组和可选字段 | JSON 文本、Ada: 9、no note | public companion 的 encoder/decoder；默认字段；显式 codec 是另一实际入口 | 直接复用 [typed.scoop](../../sysroot/src/scoop.json/examples/typed.scoop)；[样稿](SAMPLES.md#2-jsontyped) |
| async/tasks | 怎样在事件循环中启动和等待任务？两个定时任务与取消清理 | 42、Cancelled task cleaned up | 普通 main；显式 EventLoop/suspend lambda；await/join；取消和 finally；不承诺新线程 | 复用 [tasks.scoop](../../sysroot/src/scoop.asyncio/examples/tasks.scoop)；后续取消专题再独立编写 |
| http/get | 怎样发 HTTP 请求并读取正文？完整 client 加本机 hello server | 200、Hello, Scoop! | 启动/停止服务；系统信任与 HTTP 本机例子；响应只消费一次；正文上限；作用域关闭 | 复用 [get.scoop](../../sysroot/src/scoop.http.client/examples/get.scoop) 和 [hello.scoop](../../sysroot/src/scoop.http.server/examples/hello.scoop)；[样稿](SAMPLES.md#3-httpget) |
| cli/options | 怎样接收参数和 --help？问候程序解析姓名、次数和 Boolean flag | 两次 Hello, Ada!；另有帮助和无效参数路径 | argv[0]；parseCommandLine；run 的 -- 分隔；退出码 | 复用 [greet.scoop](../../sysroot/src/scoop.cli/examples/greet.scoop) |
| test/assertions | 怎样给 Cone 写测试？除法 API 与两个测试 | 两个测试通过，包含预期异常断言 | tests 源码、package、test-dependencies、@Test；assertEquals 的实际能力约束；filter/list 是测试命令选项 | 复用 [assertions Cone](../../sysroot/src/scoop.test/examples/assertions/Cone.toml) 及其生产/测试源码 |

首批主题已经独立编译与运行验证。related 的首批集合限定为以上 24 项，例如 json/typed 暂时不指向尚未交付的 json/tree。

## 2. 现有库示例的后续主题映射

这一批主要补内容组织与说明，按使用需求逐页交付。多个文件组成同一运行任务时登记为一个主题；低层协议页明确其适用问题，HTTP 分类仍先推荐高层 client/server。

### 2.1 集合、文本与时间

| ID | 现有来源 | 内容重点与编辑动作 |
| --- | --- | --- |
| collections/transform | [std collections.scoop](../../sysroot/src/scoop.std/examples/collections.scoop) | 从混合例子拆出 filter → sorted → joinToString；说明急切求值、新结果与就地修改的命名 |
| text/bytes | [text_and_bytes.scoop](../../sysroot/src/scoop.std/examples/text_and_bytes.scoop) | 文本、UTF-8、Base64、hex 与明确端序；数字解析拆给 text/numbers，避免混成“所有文本 API” |
| text/encoding | [encoding.scoop](../../sysroot/src/scoop.charset/examples/encoding.scoop) | 指定字符集、严格失败和显式有损转换；不把 NFC 作为隐式转码步骤 |
| text/unicode | [unicode.scoop](../../sysroot/src/scoop.charset/examples/unicode.scoop) | NFC 与字素簇；用分解音标和 emoji 显示和标量计数的差异 |
| text/regex | [captures.scoop](../../sysroot/src/scoop.regex/examples/captures.scoop) | 命名捕获、查找与完整匹配；Regex 的 use；日期形状不等于日历校验 |
| text/regex-replace | [replace_and_split.scoop](../../sysroot/src/scoop.regex/examples/replace_and_split.scoop) | replace 的字面值、replaceTemplate 的捕获展开、split；正确处理 $ |
| time/dates | [dates.scoop](../../sysroot/src/scoop.std/examples/dates.scoop) | 月末运算、闰年、日期差和 Duration；固定日期输出，不依赖本机时钟 |

### 2.2 JSON、URL 和 HTTP

| ID | 现有来源 | 内容重点与编辑动作 |
| --- | --- | --- |
| json/tree | [value_tree.scoop](../../sysroot/src/scoop.json/examples/value_tree.scoop) | 不可变值树、builder、数字 token；字段不存在的 None 与 Some(JSON null) |
| url/resolve | [resolve.scoop](../../sysroot/src/scoop.url/examples/resolve.scoop) | 相对 URL、IRI/URI、独立路径段和 request target；纯内存操作 |
| url/query | [query_and_form.scoop](../../sysroot/src/scoop.url/examples/query_and_form.scoop) | 重复参数、无值/空值；query 和表单对空格/+ 的不同规则 |
| url/domain | [domain_names.scoop](../../sysroot/src/scoop.idna/examples/domain_names.scoop) | 域名 ASCII/Unicode 形式、OrNone 失败；没有 DNS 查询 |
| url/punycode | [punycode.scoop](../../sysroot/src/scoop.idna/examples/punycode.scoop) | 编码与 IDNA 标签合法性分别处理，不能把 Punycode 成功当成域名有效 |
| http/post-json | [post_json.scoop](../../sysroot/src/scoop.http.client/examples/post_json.scoop) ＋ [server json.scoop](../../sysroot/src/scoop.http.server/examples/json.scoop) | 请求和响应 DTO、状态检查、body 单次消费、显式大小限制；完整两终端程序 |
| http/server | [hello.scoop](../../sysroot/src/scoop.http.server/examples/hello.scoop) | 最小 server、默认地址、路由和作用域；给 curl 请求与停止方式 |
| http/routes | [routing.scoop](../../sysroot/src/scoop.http.server/examples/routing.scoop) | 路径参数、路由组、middleware、fallback 的真实组合；正常和 404 请求 |
| http/server-json | [server json.scoop](../../sysroot/src/scoop.http.server/examples/json.scoop) | query 默认值、JSON body、自动 codec、400/415；共用源码，重点区别于客户端页 |
| http/sse | [server sse.scoop](../../sysroot/src/scoop.http.server/examples/sse.scoop) | 事件发送、flush、响应完成与 server 生命周期；curl -N |
| http/spa | [spa/server.scoop](../../sysroot/src/scoop.http.server/examples/spa/server.scoop) 及 public 资源 | 完整目录、cwd、静态文件、客户端路由回退；缺失资产与 API 仍为 404 |
| http/http1-codec | [http1.scoop](../../sysroot/src/scoop.http/examples/http1.scoop) | 内存协议编解码；正文/结束事件；不将其介绍成应用 HTTP client |
| http/sse-parser | [http/sse.scoop](../../sysroot/src/scoop.http/examples/sse.scoop) | 任意输入切片、多行 data、事件 ID、消费事件解除背压 |
| http/http1-transport | [async client](../../sysroot/src/scoop.http.asyncio/examples/client.scoop) ＋ [async server](../../sysroot/src/scoop.http.asyncio/examples/server.scoop) | 直接组合 TCP 和 HTTP/1；所有权移交、Content-Length、finish；一次会话后退出 |

### 2.3 并发、测试与命令行

| ID | 现有来源 | 内容重点与编辑动作 |
| --- | --- | --- |
| io/threads | [threads.scoop](../../sysroot/src/scoop.platform/examples/threads.scoop) | Thread、AtomicInt、join、dispose；固定最终计数，不承诺线程调度顺序 |
| async/tcp-echo | [tcp_echo.scoop](../../sysroot/src/scoop.asyncio/examples/tcp_echo.scoop) | 同一 loop 内的 listener/client、系统分配端口、缓冲、半关闭和 useAsync |
| test/lifecycle | [lifecycle Cone](../../sysroot/src/scoop.test/examples/lifecycle/Cone.toml) 及生产/测试源码 | BeforeTest/AfterTest、每个测试独立实例；实际两个用例通过 |
| cli/repeated-options | [repeated_options.scoop](../../sysroot/src/scoop.cli/examples/repeated_options.scoop) | 重复选项、枚举选择、Option 路径；例子只展示解析，不暗示写文件 |

### 2.4 日志

| ID | 现有来源 | 内容重点与编辑动作 |
| --- | --- | --- |
| log/basic | [hello.scoop](../../sysroot/src/scoop.log.console/examples/hello.scoop) | 应用安装 provider、库使用 facade、默认 stderr、flush；动态时间不作固定快照 |
| log/lazy | [levels.scoop](../../sysroot/src/scoop.log.console/examples/levels.scoop) | 级别过滤、字符串实参与闭包的求值差异；用计数展示实际行为 |
| log/format | [formatting.scoop](../../sysroot/src/scoop.log.console/examples/formatting.scoop) | 固定 Clock、自定义格式、包装函数转发 SourceLocation；源码行号保持实际值 |
| log/errors | [exceptions.scoop](../../sysroot/src/scoop.log.console/examples/exceptions.scoop) | cause、消息构造失败和调用前异常；清楚区分被记录的错误与传播到业务的异常 |
| log/configure | [configuration.scoop](../../sysroot/src/scoop.log.console/examples/configuration.scoop) | 替换/恢复 provider、NoopLogProvider；进程级配置与资源关闭责任 |
| log/provider | [memory.scoop](../../sysroot/src/scoop.log/examples/memory.scoop) | 自定义 LogProvider、线程安全责任、AtomicRef 更新回调可重试 |
| log/fallback | [fallback.scoop](../../sysroot/src/scoop.log/examples/fallback.scoop) | provider 失败与 stderr 关闭；先 build 再控制程序 fd，区分编译器诊断 |

### 2.5 数值、压缩与密码协议

| ID | 现有来源 | 内容重点与编辑动作 |
| --- | --- | --- |
| math/scalars | [scalars.scoop](../../sysroot/src/scoop.math/examples/scalars.scoop) | 受检算术、wrapping 对照、floor/ceil、浮点舍入；说明各操作解决的问题 |
| math/integers | [integers.scoop](../../sysroot/src/scoop.math/examples/integers.scoop) | 超过 128 位的整数、商余数、进制、字节表示与正模 |
| math/decimals | [decimals.scoop](../../sysroot/src/scoop.math/examples/decimals.scoop) | 金额示例；有效数字与 scale；精确除法、舍入模式、文本/浮点构造 |
| math/primes | [primes.scoop](../../sysroot/src/scoop.math/examples/primes.scoop) | 显式随机源、probable prime 和 certainty；固定种子演示不用于密钥生成 |
| compression/memory | [memory.scoop](../../sysroot/src/scoop.compression/examples/memory.scoop) | zstd/gzip 选择、解压上限、字节往返 |
| compression/stream | [file_stream.scoop](../../sysroot/src/scoop.compression/examples/file_stream.scoop) | 文件与压缩流组合；finish 完成格式，close 清理；借用和嵌套 use |
| crypto/digest | [digests.scoop](../../sysroot/src/scoop.crypto/examples/digests.scoop) | 一次性与分块 SHA-256 得到同一结果；避免泛称“加密” |
| crypto/hmac | [hmac.scoop](../../sysroot/src/scoop.crypto/examples/hmac.scoop) | 系统随机密钥、消息验证、SecretKey.use；区分动态 MAC 与固定 true/false |
| tls/sync | [同步 server](../../sysroot/src/scoop.tls/examples/server.scoop)、[client](../../sysroot/src/scoop.tls/examples/client.scoop)、[localhost.cnf](../../sysroot/src/scoop.tls/examples/localhost.cnf) | 两端完整程序、临时证书、显式 CA 与 localhost 校验、协议关闭；一次连接后退出 |
| tls/async | [异步 server](../../sysroot/src/scoop.tls.asyncio/examples/server.scoop)、[client](../../sysroot/src/scoop.tls.asyncio/examples/client.scoop)、同一证书配置 | 普通 main 进入 loop、异步加载与握手、transport 所有权和 useAsync；显示路径统一为本主题目录 |

### 2.6 终端与交互输入

| ID | 现有来源 | 内容重点与编辑动作 |
| --- | --- | --- |
| terminal/style | [styles.scoop](../../sysroot/src/scoop.terminal/examples/styles.scoop) | 样式、输出和恢复；重定向时控制序列是实际输出，不伪称纯文本 |
| terminal/parse-input | [parse_input.scoop](../../sysroot/src/scoop.terminal/examples/parse_input.scoop) | 内存字节解析与 Escape 等待；明确不要求 TTY |
| terminal/keys | [keys.scoop](../../sysroot/src/scoop.terminal.asyncio/examples/keys.scoop) | 真 TTY、按键与尺寸事件、raw mode 恢复、Escape 退出 |
| terminal/countdown | [countdown.scoop](../../sysroot/src/scoop.terminal.asyncio/examples/countdown.scoop) | 尺寸、异步命令与 timer；描述屏幕变化而非逐字匹配逃逸序列 |
| readline/repl | [repl.scoop](../../sysroot/src/scoop.readline/examples/repl.scoop) | 输入循环、EOF、Ctrl+C、:quit、内存历史；pipe 模式与交互模式分别说明 |
| readline/multiline | [multiline.scoop](../../sysroot/src/scoop.readline/examples/multiline.scoop) | Vi 模式、补全、校验与多行；给出可以逐步执行的输入序列 |

## 3. 需要专门新写的后续主题

这些主题有明确的现有语言/库依据，但缺少面向阅读者的专门程序。它们不是新增语言功能，也不进入首批完成条件；按实际使用中的缺口选择实施。

| ID | 示例要完成的任务 | 必须保留的边界或对照 |
| --- | --- | --- |
| language/control-flow | 遍历小数据，使用 if/when、区间、break/continue 和有标签的内层循环 | break/continue 的实际目标；不跨 callable |
| language/classes | 带可变属性的对象、构造校验与普通继承 | class 与 struct 的选择；公开构造器、初始化次序；不泄漏未初始化 this |
| language/properties | 计算属性、私有 setter 与使用处读写 | 属性的可见性、存储和访问器；getter/setter 不隐式挂起 |
| language/tuple | 返回两个结果并解构、忽略剩余项 | tuple 结构与 nominal struct 的区别；实际字段/位置语法 |
| language/option-nested | 显示 None、Some(None)、Some(Some(value)) 以及返回 Option 的安全调用 | 外层缺失与内层缺失；用完整 when 展示三条路径 |
| language/extensions | 为一个实际类型提供扩展函数与扩展属性 | import、静态 receiver、成员优先；不假装修改类型定义 |
| language/generic-constraints | 一个复用算法同时需要两项能力 | 使用现有约束语法和公共 API；条件/负向约束只在具体任务确实需要时引入 |
| language/equality-hash | 自定义值作为 Map key | 结构 == 与 Equals<T> conformance 分开；显式 Hash 与 ToString |
| language/operators | 普通值类型实现加法和下标读取 | 实际 operator/interface 合同；无猜测式数值基类 |
| language/context | 为调用链提供一个作用域内的服务，并在退出后恢复 | 精确静态类型 key、动态绑定、缺失错误；不当作隐式函数参数 |
| language/type-info | 通过公开静态描述打印类型/成员信息 | 只使用实际描述能力；不引入按字符串查找或反射调用 |
| language/sealed | 匹配一组受约束的子类型 | 声明位置与穷尽性；给完整声明和使用文件 |
| start/library | 构建自己的 library 并供另一个应用使用 | 两份完整 manifest、public API、实际产物/源码依赖选择 |
| start/source-selection | 为两个已有目标选择不同实现文件 | 真实 target 条件和完整源码集；不为未支持平台添加占位实现 |
| ffi/call-c | 通过普通 native 源码和 @Extern 调用一个 C 函数 | C 文件、Cone.toml、实际类型与 ABI；不借用测试专用注入入口 |
| text/numbers | 同一输入分别做受检解析和 OrNone 解析 | 正常缺失与异常边界；不静默捕获无关异常 |
| async/cancellation | 取消一个已启动任务，等待清理完成并观察异常传播 | 建立确定的任务已启动条件；取消协作点；finally 和 join |
| io/buffers | 循环读写时处理短读/短写与 EOF | 真实 buffer 生命周期；挂起时 use/useAsync 的实际选择 |

## 4. 检索词与别名编辑

| 查询表达 | 处理方式 |
| --- | --- |
| option、nullable、可选值 | 唯一的简短形式可作为 language/option 别名，其余作为关键词 |
| null | 搜索同时返回 language/option 和已发布的 json/tree，摘要说明领域区别 |
| data class、值类型、copy | 指向 language/struct 的检索词；正文给 Scoop 的实际语法 |
| map、字典、词频 | collections/map；函数式 map 另由 collections/transform 的摘要区分 |
| async、协程、await | async 分类或 async/tasks；线程词关联 io/threads |
| HTTP、requests、fetch | http 分类/GET 等任务的检索词，不暗示存在同名函数 |
| scoop.json、scoop.http.client、scoop.log.console | 一对一时可作为分类别名，多对一/多对多时作为关键词 |
| ValueDecoder、withResponse、ArrayList | 作为相关主题的 API 关键词，不按符号名重新推断语义 |

别名解决“知道另一个词，却不知道主题 ID”的问题；关键词允许多个相关结果。名称变更时优先保留确有使用价值的旧别名，避免为每种拼写增加平行正文。

## 5. 内容完成检查

每个条目进入 catalog 前应能回答：

1. 这一页到底帮助读者完成哪件事，源码是否只带必要的上下文？
2. 将全部展示文件按原路径保存后，所给命令是否真实可用？
3. 依赖、import、package、公开性、main、错误路径和清理是否完整？
4. 输出是否可观察，动态值和交互前提是否解释清楚？
5. 修改字段、URL、数据或回调时，需要保留的约束是否已写明？
6. 程序是否与 CLI 输出和测试共享同一份源码，related 是否已经交付？

内容审阅与普通编译/运行共同决定交付。覆盖某个 topic 的所有边界不是示例的职责，详细语言规则仍由规范及其专门测试维护。
