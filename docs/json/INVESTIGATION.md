# 序列化与 JSON 库改进调研：M57 完成后的库适配

调研日期：2026-10-21。实现同步：2026-10-24。M55、[M57](../milestone57/PROGRESS.md)均已完成；M57 已交付 HashMap 的条件 codec、泛型字段 companion 投影及派生衔接。本轮补齐核心容器、JSON DOM／object codec 和默认入口，规范见[JSON库规范](../specs/SCOOP-JSON-SPEC.md)，本轮验证范围见第8节。字段级 codec 定制暂不实施。

本次重点是 M57 对默认 codec、泛型容器和 DTO 的帮助，同时检查现有 JSON 库的可用性与实现。与 [LLM client 调研](../llm-client/INVESTIGATION.md)衔接，但这些改进服务普通序列化和 JSON 使用，不新增 LLM 专用编译器行为。

## 1. 主要结论

1. **M55 解决调用处如何取得默认 codec，M57 解决泛型类型何时拥有默认 codec。** 两者组合后，现有 Json 泛型入口就能消费按元素能力提供 codec 的容器，无需在 Json 中逐个识别商业 API 或容器名称。
2. **M57 的收益需要标准库和普通用户类型主动采用。** HashMap 已随 M57 适配；本轮补齐 Option、Array、MutableArray 和 ArrayList。显式工厂继续保留，未适配的其他 std 容器不自动获得默认 codec。
3. **编码与解码由两个条件实现块独立提供。** 两块可以同时生效，覆盖编码、解码、两者都有、无默认 codec 四种能力组合；无需复制容器定义或增加交叉特化分支。
4. **自动 codec 不等于 JSON 格式适配。** Map 的键值对序列、enum 的外层单 key、Option 的 null 规则都不会因 M57 自动变成某个 API 所需的对象、discriminator 或三态字段。
5. **本轮补齐公开 DOM、便利入口和 String-key object codec。** 字段索引、UTF-8／流式衔接仍为独立后续工作。M57 主要改善静态组合与易用性，不会自动加快 parser 或降低大文档内存占用。

## 2. 已有基础与默认入口

### 2.1 当前库不是只有简单文本转换

现有 [Json API](../../sysroot/src/scoop.json/src/json.scoop)及[语言规范 11.13](../specs/SCOOP-SPEC.md)已经提供：

- 独立的 `ValueEncoder<T>`／`ValueDecoder<T>`，显式 codec 可替换默认策略。
- `Json.encodeToString(value)`、`Json.decodeFromString<T>(text)` 的 companion bound 重载；标量、普通派生 DTO、具有相应 companion 的日期／UUID 等类型已能使用。
- `encodeToValue(value, codec)`／`decodeFromValue(value, codec)`及对应默认重载，typed 数据与 JSON 树之间无需文本中转。
- 不可变 `JsonValue`，能保存未知子树、精确数字 token 和对象顺序，再交给其他 codec 读取。
- 严格的完整文档检查、重复 key 检查、JSON Pointer 路径；未知字段仍检查 JSON 语法。
- 稳定且可复读的 Decoder；复用一个 decoder 组合多个 DTO、收集未知字段、尝试其他读取不会消耗节点。
- `Serialize` 的 rename、required、emitNull、ignore，以及缺失字段、显式 null 和默认值的既有区分。
- Option、数组、列表、Map、Set、Result 等显式 codec 组合。

本轮扩展现有协议和 JsonValue，不另建反射序列化系统或第二棵 `Any` JSON 树。[现有组合 Cone 测试](../../sysroot/src/scoop.json/tests/consumer/values-composition/main.scoop)已经展示“读取两个 DTO＋收集未知字段＋重新编码”；新增的 DOM API 让这些操作更直接。

### 2.2 值树和 HTTP 默认入口

字符串与值树入口分别使用 `T.Companion : ValueEncoder<T>` 和 `ValueDecoder<T>`。本轮增加的值树重载为：

```scoop
public fun <T> encodeToValue(value: T): JsonValue
    where T.Companion : ValueEncoder<T> =
    encodeToValue(value, T.Companion)

public fun <T> decodeFromValue(value: JsonValue): T
    where T.Companion : ValueDecoder<T> =
    decodeFromValue(value, T.Companion)
```

两个方向各自转发原显式入口，不增加隐式参数或运行期查找。

HTTP 同步增加 `RequestBody.fromJson(value, maxBytes)`、`HttpResponseBody.json<T>(maxBytes)`，由调用方向分别约束 encoder／decoder，转发到现有显式入口。容器 reader／writer 若确有重复调用，可以添加普通泛型扩展函数；不能在 Encoder／Decoder interface 上引入当前语言不支持的泛型虚方法。

容器默认能力由各自的条件 companion 提供，避免在 Json 中枚举嵌套形状。Json 的 objectEncoder／objectDecoder 也分别提供显式元素 codec 和无参数 companion 重载。

### 2.3 M55 的其他约束不应被误用

- 公开成员 `requires` 可包装确有用途的转换方法，但核心序列化已有明确的 nominal codec 协议，不需要再建一套按同名 encode／decode 猜测的平行入口。
- `T.Companion : !ValueEncoder<T>` 同时要求 public companion 存在，不包含“完全没有 companion”的类型；它不是通用 fallback。
- 泛型正文的普通重载在定义处绑定。无约束 helper 不会在实例化时突然找到具体类型的更优 codec。
- 编码／解码能力属于 codec 值，数据实例不因此实现这些接口；`Any` 的运行期类型也不参与默认选择。

上述边界见 [M55 设计 10.3～10.4](../milestone55/DESIGN.md)和[语言规范 3.2.2～3.2.4](../specs/SCOOP-SPEC.md)。

## 3. M57 落地后的直接收益

### 3.1 泛型容器可以向外提供自己的默认 codec

显式嵌套 Map 策略继续可用：

```scoop
val codec = HashMap<String, HashMap<Int, String>>.encoder(
    String.Companion,
    HashMap<Int, String>.encoder(Int.Companion, String.Companion)
)
val text = Json.encodeToString(value, codec)
```

HashMap 已按 M57 为原 companion 声明两个条件接口，现在可直接写：

```scoop
val text = Json.encodeToString(value)
val copy = Json.decodeFromString<HashMap<String, HashMap<Int, String>>>(text)
```

内层 HashMap 先根据 Int／String 的 companion 得到自己的 codec 接口，外层再使用这个接口。最后 Json 只检查整个参数类型的 companion。条件关系在编译期证明，实际 codec 仍是普通 singleton／对象和普通调用。

这比仅给 Json 增加 Map 重载更有价值：HTTP、文件格式、其他序列化消费者也能复用相同的 `ValueEncoder<T>`／`ValueDecoder<T>`，无需分别递归发现容器类型。

### 3.2 默认能力应按两个方向独立提供

令 E 表示“本容器编码所需的全部元素 companion 实现对应 ValueEncoder”，D 表示解码所需的对应事实；HashMap 还始终保留原有 Hash／Equals 基础条件。

| 已确定的能力组合 | companion 的默认能力 |
| --- | --- |
| E、D 都成立 | 编码与解码 |
| E 成立、D 不成立 | 仅编码 |
| D 成立、E 不成立 | 仅解码 |
| E、D 都不成立 | 无默认 codec，保留原容器和显式工厂 |

这是两个条件实现块的四种结果组合，不是四份类型定义。E、D 同时成立时两块同时提供接口，没有唯一赢家或兜底分支。元素没有 public companion 时不满足对应条件，容器及其已有 companion 仍存在；未知条件则保持未知。

方向独立有实际用途：请求 DTO 可能只能编码，响应 DTO 可能只能解码。不能因为响应没有 encoder 而拒绝 `Array<Response>` 的解码，也不能要求只发送请求的应用补一个无用 decoder。

采用独立实现块后，以下泛型转发只需编码条件：

```scoop
fun <K, V> encodeMapDefault(value: HashMap<K, V>): String
    where K : Equals<K>, K : Hash,
          K.Companion : ValueEncoder<K>,
          V.Companion : ValueEncoder<V> =
    Json.encodeToString(value)
```

这是 M57 落地并完成 HashMap 两个条件接口适配后的例子。编码块的 guard 不包含 decoder 条件，不能因解码方向缺失而拒绝该调用。

### 3.3 泛型 DTO 和派生 codec 可以减少依赖传递

对 `Envelope<T>`、`Page<T>`、`Result<T, E>`，M57 允许按字段所需能力提供默认 codec，普通存储用途始终使用同一份数据定义。调用者也可继续传入显式元素策略，不需要把策略写进数据对象。

M57 已将定义环境中成立的 `T.Companion` 投影接入[字段 codec 选择器](../../compiler/hir-lower/src/class/inheritance/coding/codecs.rs)，手写和自动派生都可使用条件接口中的已知事实。未知 T 仍不能在实例化时猜测 codec。

本轮新增核心容器默认接口时，进一步明确字段选择顺序：完整字段依赖、递归 this 优先；核心容器递归选择元素策略并调用显式工厂；其他名义类型再使用已证明的 companion。这样新增默认 Array／Option codec 不会覆盖用户已经注入的元素策略或递归 this。两个方向保持独立，ValueInspector 的既有规则不变。

### 3.4 单一数据定义与 companion

M57 已改为在原 companion 的 body 中分别声明 `override interface ValueEncoder<Container<T>> where ...` 和对应解码块。数据类型、字段、构造器、普通 API、Serialize 配置和 companion 初始化始终只有原来一份，新增 codec 条件不会选择另一种数据表示。

两个方向可以调用普通共享 helper，不要求复制容器定义、抽出 HashMapImpl 或引入专门的条件 codec 基类。已有普通 codec class 的继承能力保持，但不是获取条件接口的必经路径；本期不再采用整份类型特化。

实现条件应依赖元素已有的 codec 能力，不能反过来以 `Container<T>.Companion` 已实现 codec 为依据证明该实现块自己的 guard，否则形成 M57 第 9 节拒绝的关系查询循环。

### 3.5 哪些类型能受益，哪些仍需单独处理

| 类型 | M57 与库适配后的前景 | 必须保留的边界 |
| --- | --- | --- |
| HashMap／LinkedHashMap、普通 Set／Deque | 普通 class，可按方向提供默认 codec；嵌套后继续组合 | 原 Hash／Equals bound、显式工厂、迭代顺序和重复 key 规则 |
| ArrayList | 普通 class，可在已有 companion 内提供条件接口 | 不让 codec 条件限制普通元素存储 |
| Option、Result | 当前是普通 enum，可按方向适配 | Option 的 Some／None、null 和字段省略语义；Result 的两支 wire 名保持 |
| 普通泛型 DTO | companion 的条件接口及既有 codec 合成可减少显式依赖 | 派生所需的字段来源在定义处成立；访问域、默认值顺序保持 |
| Array／MutableArray | 已有普通 companion 可按条件接口规则适配，无需改变 intrinsic 表示 | 逐类型完成库适配和验收后才记为支持；显式工厂继续可用 |
| List／Map 等接口静态类型 | 不因运行期对象是某个可编码容器而自动取得默认 codec | 数据实例的接口关系不提供 companion；即使接口声明自己的 companion，也须由其所属 Cone 明确默认 codec 与解码结果 |
| TreeMap／TreeSet／PriorityQueue | 可简化可确定的编码；解码仍可能需要显式配置 | Comparator 是运行期策略，元素有 decoder 不能替应用选择比较器 |
| tuple、Unit | 继续使用现有组合 helper 和 UnitEncoder／UnitDecoder | tuple 没有 companion，M57 不新增 tuple companion 或变长类型参数机制 |
| Any、外部类型、无 public companion 的类型 | 显式 codec、包装或用户自己定义的类型仍可编码 | 不增加动态 registry，不允许消费 Cone 给外部类型或 companion 追加实现 |

“嵌套容器自动工作”必须逐层成立。本轮覆盖 `HashMap<String, Option<Array<Int>>>` 和泛型 DTO；Array、MutableArray、ArrayList、Option 已适配。LinkedHashMap、Set、Deque、Result 等未在本轮增加默认条件接口，继续使用其显式工厂；List／Map 接口也没有因具体容器适配而自动获得 companion codec。

## 4. M57 不会替 JSON 库完成的工作

### 4.1 Map 默认 wire 格式与 JSON object 是两件事

当前[普通 map codec](../../sysroot/src/scoop.std/src/collections/map_codec.scoop)使用 `[[key,value], ...]`。即使 key 是 String，编码仍是 entry 数组。M57 默认 HashMap codec 按设计继续复用这个格式。

JSON API 客户端经常需要 `{"name": value}`。本轮在 JSON 层提供 objectEncoder／objectDecoder 和 JsonValue 对象工厂／builder。encoder 接收 Map<String,V>，decoder 明确返回保持输入顺序的新 LinkedHashMap<String,V>；默认 generic Map 继续使用 entry 数组。

M57 不提供整份类型特化或接口实现特化，也不支持 `K == String` 条件；不能再增加一个 String-key 的重叠实现来覆盖默认格式。也不应将任意 key.toString() 作为对象 key：可能碰撞，且不可逆。

### 4.2 多态消息、字段策略和 JSON Schema

普通 enum 默认是外层单 key；LLM API 常见的 `{"type":"text","text":"..."}`、content 数组和开放扩展需要专门 codec。M57 能帮助取得该 codec，不能替它决定 discriminator、flatten 或未知 variant 的处理。

同样，M57 不改变 missing／null／value 三态，不使 Byte 数组自动变成 base64，也不从 custom codec 推出完整 JSON Schema。Schema 可以先用 JsonValue 表达；若后续需要自动生成，须单独定义 wire 策略与覆盖方式。

M57 只衔接已有 codec 合成与条件环境，不替代 M47 的生成器／默认值迁移。它不新增枚举任意字段生成正文的机制，也不能把构造 default 的源码复制到另一作用域后保证绑定不变。

### 4.3 流式与性能

M57 不改变 JSON 语法、字节读取、SSE／NDJSON 定界或工具参数增量。已知静态 codec 可以减少手工组合及部分临时策略对象，但 parser、字段索引、树表示和 I/O 仍须独立改进，不能预先声称有吞吐提升。

## 5. JSON 库适配与剩余工作

### 5.1 本轮：公开不可变 DOM 与简单入口

保留 JsonValue 的现有 class 和共享不可变节点，公开API如下；精确签名、异常和复杂度以 JSON 库规范为准，不公开内部可变 ArrayList：

| API 能力 | 当前合同 |
| --- | --- |
| `Json.parse(text)`／`Json.stringify(value)` | 分别返回 JsonValue、直接渲染已合法的值；避免用户借助 identity codec 表达普通树操作 |
| `JsonValue.kind: JsonValueKind` | 明确 null／boolean／number／string／array／object |
| 对象读取与遍历 | key、成员顺序、contains、按 key 取得子值；缺失返回 None，存在的 JSON null 返回 Some(JsonValue) |
| 数组读取与遍历 | Long 下标、大小、不可修改的视图或独立快照；不暴露底层可变 storage |
| 标量读取 | 严格匹配类型；Long／ULong／Float／Double 复用现有范围和精度语义 |
| 受检构造 | null、Boolean、String、数值、数组、对象；对象重复 key 明确报错 |
| 动态数字 | 合法 number token 的受检工厂与读取，保留超出 ULong／Double 的数字，不统一转换为 Double |
| typed 转换 | 补齐 encodeToValue／decodeFromValue 的 M55 默认重载，保留显式 codec |

对象读取遇到非对象应报告类型不匹配，不能伪装成“key 不存在”。构造器只验证新输入并在边界取得快照；已合法的 JsonValue 子节点可共享，不能每次访问或再次输出都重新解析整棵树。

树保留 JSON 值、对象顺序和数值 token；仍不承诺保留原始空白、字符串转义拼写或签名所需的原始消息字节。应用需要原始字节时显式保存输入。

JsonObjectBuilder／JsonArrayBuilder 的 build 发布独立结构快照；继续 add 不改变旧值。对象重复 key 报错，合法子树共享。本轮不包含 JSONPath、patch 或可变 DOM。

### 5.2 本轮：泛型 object codec；其他组合保持显式

Json.objectEncoder 接收显式 ValueEncoder<V> 或使用 V.Companion，返回 ValueEncoder<Map<String,V>>；objectDecoder 独立接收 ValueDecoder<V> 或使用 V.Companion，返回 ValueDecoder<LinkedHashMap<String,V>>。字段路径、重复 key 和元素异常沿用普通 keyed 协议。

这项能力既能服务工具 schema 和原生扩展字段，也能服务普通 Web API，不依赖 M57 或新增 core Map 协议。它应保持字段冲突报错，不能合并已知字段与扩展时静默覆盖。

现在已有稳定 decoder，定制 codec 可以先检查 discriminator，再在同一节点上读取具体 DTO；无须先 stringify 再 parse。未知字段收集、严格拒绝未知字段也已有组合路径，应补文档和适度 helper。

不能把全局“拒绝未知字段”简单实现为每个 `keyed.end()` 都检查未消费 key：当前允许多个 codec 读取同一 record，且 keyed end 不要求字段全部被读取。若提供严格 helper，它应取得该次检查明确的允许字段集合，保持原容器合同。

### 5.3 修复已能从源码确认的低效路径

| 当前路径 | 影响 | 建议 |
| --- | --- | --- |
| parser 每读一个对象 key 都遍历之前的成员 | n 个字段的重复检查累计为二次量级 | 保留有序成员，增加一次建立的 key 索引 |
| keyed encoder 新增字段时同样线性查重 | 大 schema／动态对象写出成本较高 | 使用相同的普通索引策略 |
| keyed decoder 每次 required／optional 线性查找，keys 每次重新建列表 | 逐字段解码与复读重复做工作 | 不可变 record 共享索引和只读 key 视图 |
| JsonValue 通过通用 encodeToString 先重建树再渲染 | 已有树又分配一棵同形树 | 本轮新增直接 stringify；通用 codec 路径仍遵守 writer 提交与重复字段合同 |
| HTTP JSON request 先完整生成 String 和 byte array，再检查 maxBytes，并取得发送快照 | 上限限制了最终发送大小，未限制之前的完整物化 | 后续提供有界 UTF-8 写出入口，让构造阶段就能停止，并减少中间表示 |

依据：[parser](../../sysroot/src/scoop.json/src/parser.scoop)、[encoding containers](../../sysroot/src/scoop.json/src/encoding_containers.scoop)、[decoding containers](../../sysroot/src/scoop.json/src/decoding_containers.scoop)、[JsonValue](../../sysroot/src/scoop.json/src/value.scoop)、[HTTP request body](../../sysroot/src/scoop.http.client/src/body/data.scoop)。以上是代码路径分析，尚无本轮性能基准。

JSON 本轮为 Map／LinkedHashMap 适配和 object builder 查重显式依赖 std。parser 和 keyed decoder 的索引仍待独立测量与实现，不在 JSON 内另造哈希容器。

### 5.4 UTF-8、流式与缺省设置

优先补 UTF-8 byte array 输入／输出，与 HTTP 的实际载荷一致。便利封装可以先复用现有严格 UTF-8 和 Json 实现，但要明确仍会物化完整文档；只有实现直接字节路径后才能声称减少了复制。

对 LLM 客户端，第一步并不需要“任意未完成 JSON 也可解码”：SSE 的完整 data 和 NDJSON 的完整记录各自解析；工具参数原文增量先保留，在调用完成后解析一次。不能每收到一个 delta 都从头解析全部前缀，也不能用补括号把未完成输入当成合法值。

真正的增量 tokenizer／parser 可以作为之后的独立能力，明确完整值、仍需输入、语法错误与 EOF。既有 Decoder 保证随机访问和失败后复读，不能暗中换成单向网络游标。ValueEncoder／ValueDecoder 目前是同步方法，异步 I/O 适配不能在内部阻塞 EventLoop 或偷偷调用 suspend。

建议继续采用严格 JSON、重复 key 报错、无数字／字符串自动强制转换、默认紧凑输出。pretty printing 可以是明确的写出选项。HTTP 对网络 body 设置有限字节上限；JSON 的解析／遍历深度只针对实际实现的栈边界设计，不扩展为跨库通用预算体系。

## 6. M57 已完成的验收与库组合回归

M57 的已完成范围见其进度记录；本轮库适配延续以下回归边界，不把已完成的语言能力写成未来计划：

1. **四种能力组合**：encode-only DTO、decode-only DTO、两方向、无 companion；不仅验证 Int／String 两方向齐全的简单情况。
2. **嵌套的真实边界**：嵌套 HashMap、Option／ArrayList 与普通 DTO；Array／MutableArray 的默认能力属于本轮库适配，不能用历史 M57 结果代替其验收。
3. **固定 companion 的状态**：接口条件与普通字段、构造和初始化独立；同一完整宿主跨 Cone 使用同一 singleton，关系查询不执行初始化。
4. **有 companion bound 的泛型字段派生**：手写调用与自动派生一致；显式完整字段 codec 仍胜过默认来源。
5. **运行期策略保持显式**：Comparator、自定义时间格式、field codec 覆盖、无默认 companion 的外部类型仍可使用旧路径。
6. **wire 行为保持**：Map entry 序列、Option 的 missing／null/default、Result／enum 形状、字段顺序和未知 JSON 回传均有真实结果断言。

两个实现块即可覆盖两种方向的组合。其他接口也可以独立增加，但不因语法已经可用就立即扩展 inspector、schema、hash 等全部能力；每项扩展应有当前消费者，共享 helper 只复用实际相同的算法。

## 7. 当前范围与后续顺序

| 状态 | 工作 |
| --- | --- |
| M57 已完成 | HashMap 两个条件 codec、四种能力组合、泛型 DTO 字段投影和条件派生 |
| 本轮实现 | Option／Array／MutableArray／ArrayList 默认 codec，保留显式元素依赖与递归 this |
| 本轮实现 | JsonValue DOM、受检工厂和快照 builder、parse／stringify、值树默认重载 |
| 本轮实现 | String-key object codec、HTTP JSON 默认重载和 quickstart |
| 后续按实际测量 | parser／keyed decoder 字段索引、UTF-8／流式入口 |
| 暂不实施 | 字段级 codec 定制、额外 enum wire 策略／schema 自动生成 |

公共合同已分列于语言规范11.13、JSON库规范和HTTP client规范；实现选择的变化同步到编译器与产物规范。没有新的runtime ABI。

## 8. 依据与验证范围

本次读取了 M55、M57 设计与 M55 实施记录、三份权威规范的相关部分、std codec 规范、JSON 全部实现、core 编解码／容器 API、Map／Result helper、字段 codec 合成入口，以及 HTTP JSON body 的实际路径。

代表性现有证据：

- [M55 companion JSON 消费者](../../tests/fixtures/language/companion/json/program/src/main.scoop)：两个方向独立、泛型转发、命名 companion、显式替换。
- [M46 JsonValue](../../sysroot/src/scoop.json/tests/values/values/main.scoop)及[组合消费者](../../sysroot/src/scoop.json/tests/consumer/values-composition/main.scoop)：保存、复读、未知字段、字段冲突。
- [容器组合](../../sysroot/src/scoop.json/tests/consumer/serialization-containers/main.scoop)：显式容器策略、路径、完整容器检查。
- [M46 字段与默认值语义](../milestone46/SERIALIZE.md)：编码省略与解码 presence 的既有规则。
- [M57 验收记录](../milestone57/PROGRESS.md)：已完成的语言能力与三目标验收范围。

最初调研另在本机 `aarch64-apple-darwin` 用现有 `scoop build`／`scoopc` 和已准备的 sysroot，构建并运行了一个仓库外的独立消费者。四组断言全部通过：

- M55 companion bound 的值树转换 helper，包含只编码、只解码的 DTO。
- 由元素 companion 构造显式 Array encoder／decoder 的普通泛型工厂。
- 嵌套 HashMap 的显式 codec 往返，确认 String key 仍使用 entry 数组 wire 形状。
- JsonValue 保留超出 ULong 的数字，按已有容器 API 区分缺失 key 与显式 null。

最初调研的实验源码和日志保留在仓库外，仅说明当时的 helper 可用性。M57 现已完成，不再将其条件接口或派生衔接记为设计推论。

本轮于2026-10-24在`aarch64-apple-darwin`使用当前源码重建工具链并显式准备开发sysroot，完成以下验证：

- `cargo fmt --all`与`cargo clippy --workspace --all-targets`通过；HIR lower单元测试 **2054项通过**。测试使用opt-level 1、debug 0，保留debug assertions和overflow checks，未设置RUST_MIN_STACK。
- JSON Cone的release测试 **90项通过**，包含新增的11项：核心容器默认能力与独立方向、嵌套泛型DTO、显式元素依赖优先级、object codec、DOM类型／数值／错误路径和builder快照；既有递归、字段省略与Map entry格式回归同时通过。
- HTTP client的release JSON定向测试 **4项通过**，包含新增的2项默认入口测试，覆盖仅编码／仅解码类型、UTF-8字节上限、响应一次消费、显式替换和原有错误传播。
- 条件接口与companion JSON的正式CLI fixture **17项通过**，覆盖定义错误、条件不可用和预编译库消费。
- [JSON quickstart](../../sysroot/src/scoop.json/README.md)的完整示例独立构建运行通过；JSON与HTTP client的`scoop doc --no-deps --deny-doc-warnings`生成通过。

首次未优化的HIR测试在既有可见性用例上发生默认测试线程栈溢出；增加线程栈的单项探测通过，随后上述优化配置下全量通过。该失败与复验结果一并保留在仓库外日志中。本轮未运行其他平台或HTTP client的全部行为测试，不将历史M57三目标验收外推为本轮结果。
