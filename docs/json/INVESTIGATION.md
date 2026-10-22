# 序列化与 JSON 库改进调研：M55 现状与 M57 的作用

调研日期：2026-10-21。设计同步：2026-10-23。状态：调研与设计建议。M55 已实现；[M57](../milestone57/DESIGN.md)已按普通类型与 companion 的条件 interface 实现方案进入实施，完成范围以其进度记录为准。本文对未来能力的判断均以相应实现通过验收为前提。

本次重点是 M57 对默认 codec、泛型容器和 DTO 的帮助，同时检查现有 JSON 库的可用性与实现。与 [LLM client 调研](../llm-client/INVESTIGATION.md)衔接，但这些改进服务普通序列化和 JSON 使用，不新增 LLM 专用编译器行为。

## 1. 主要结论

1. **M55 解决调用处如何取得默认 codec，M57 可以解决泛型类型何时拥有默认 codec。** 两者组合后，现有 Json 泛型入口就能消费按元素能力提供 codec 的容器，无需在 Json 中逐个识别商业 API 或容器名称。
2. **M57 的收益需要标准库和普通用户类型主动采用。** 新增语言能力本身不会改写现有 HashMap、Option、ArrayList 或 DTO 的 companion；当前显式工厂也不能直接删除。
3. **编码与解码由两个条件实现块独立提供。** 两块可以同时生效，覆盖编码、解码、两者都有、无默认 codec 四种能力组合；无需复制容器定义或增加交叉特化分支。
4. **自动 codec 不等于 JSON 格式适配。** Map 的键值对序列、enum 的外层单 key、Option 的 null 规则都不会因 M57 自动变成某个 API 所需的对象、discriminator 或三态字段。
5. **JSON 的直接缺口仍是公开 DOM、便利入口、对象字段查找和 UTF-8／流式衔接。** 这些不依赖 M57，可以先做。M57 主要改善静态组合与易用性，不会自动加快 parser 或降低大文档内存占用。

## 2. 已有基础与 M55 可以立即利用的能力

### 2.1 当前库不是只有简单文本转换

现有 [Json API](../../sysroot/src/scoop.json/src/json.scoop)及[语言规范 11.13](../specs/SCOOP-SPEC.md)已经提供：

- 独立的 `ValueEncoder<T>`／`ValueDecoder<T>`，显式 codec 可替换默认策略。
- `Json.encodeToString(value)`、`Json.decodeFromString<T>(text)` 的 companion bound 重载；标量、普通派生 DTO、具有相应 companion 的日期／UUID 等类型已能使用。
- `encodeToValue(value, codec)`／`decodeFromValue(value, codec)`，typed 数据与 JSON 树之间无需文本中转。
- 不可变 `JsonValue`，能保存未知子树、精确数字 token 和对象顺序，再交给其他 codec 读取。
- 严格的完整文档检查、重复 key 检查、JSON Pointer 路径；未知字段仍检查 JSON 语法。
- 稳定且可复读的 Decoder；复用一个 decoder 组合多个 DTO、收集未知字段、尝试其他读取不会消耗节点。
- `Serialize` 的 rename、required、emitNull、ignore，以及缺失字段、显式 null 和默认值的既有区分。
- Option、数组、列表、Map、Set、Result 等显式 codec 组合。

因此应扩展现有协议和 JsonValue，不另建一个反射序列化系统或第二棵 `Any` JSON 树。[现有组合 Cone 测试](../../sysroot/src/scoop.json/tests/consumer/values-composition/main.scoop)已经展示“读取两个 DTO＋收集未知字段＋重新编码”，缺的是更直接的公共操作。

### 2.2 M55 已实现，尚未用到所有入口

当前字符串重载分别使用 `T.Companion : ValueEncoder<T>` 和 `ValueDecoder<T>`，而值树转换仍要求显式 codec。可增加同形重载：

```scoop
public fun <T> encodeToValue(value: T): JsonValue
    where T.Companion : ValueEncoder<T> =
    encodeToValue(value, T.Companion)

public fun <T> decodeFromValue(value: JsonValue): T
    where T.Companion : ValueDecoder<T> =
    decodeFromValue(value, T.Companion)
```

这是拟添加到 Json 的方法，不是已经存在的重载。相同形式的普通 helper 已适合当前语言，不需要 M57。

同样可补 `RequestBody.fromJson(value, maxBytes)`、`HttpResponseBody.json<T>(maxBytes)`，由调用方向分别约束 encoder／decoder，转发到现有显式入口。容器 reader／writer 若确有重复调用，可以添加普通泛型扩展函数；不能在 Encoder／Decoder interface 上引入当前语言不支持的泛型虚方法。

少量有自身类型参数的工厂也可简化调用，例如 `arrayEncoder<T>() where T.Companion : ValueEncoder<T>` 返回 `Array<T>.encoder(T.Companion)`。它只自动取得当前元素的 codec；如果元素本身是没有默认 companion codec 的另一个容器，仍需显式组合。不能用大量“Array of Option of Array”重载模拟递归类型匹配。

### 2.3 M55 的其他约束不应被误用

- 公开成员 `requires` 可包装确有用途的转换方法，但核心序列化已有明确的 nominal codec 协议，不需要再建一套按同名 encode／decode 猜测的平行入口。
- `T.Companion : !ValueEncoder<T>` 同时要求 public companion 存在，不包含“完全没有 companion”的类型；它不是通用 fallback。
- 泛型正文的普通重载在定义处绑定。无约束 helper 不会在实例化时突然找到具体类型的更优 codec。
- 编码／解码能力属于 codec 值，数据实例不因此实现这些接口；`Any` 的运行期类型也不参与默认选择。

上述边界见 [M55 设计 10.3～10.4](../milestone55/DESIGN.md)和[语言规范 3.2.2～3.2.4](../specs/SCOOP-SPEC.md)。

## 3. M57 落地后的直接收益

### 3.1 泛型容器可以向外提供自己的默认 codec

当前嵌套 Map 需要逐层写策略：

```scoop
val codec = HashMap<String, HashMap<Int, String>>.encoder(
    String.Companion,
    HashMap<Int, String>.encoder(Int.Companion, String.Companion)
)
val text = Json.encodeToString(value, codec)
```

若 HashMap 按 M57 为已有 companion 声明条件接口实现，便可写：

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

方向独立有实际用途：请求 DTO 可能只能编码，响应 DTO 可能只能解码。不能因为响应没有 encoder 而拒绝 `List<Response>` 的解码，也不能要求只发送请求的应用补一个无用 decoder。

采用独立实现块后，以下未来泛型转发只需编码条件：

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

但这里有一项必须同步实现的连接：当前[字段 codec 选择器](../../compiler/hir-lower/src/class/inheritance/coding/codecs.rs)优先选择 primary val 依赖、递归 this、可命名 nominal 的 companion，再组合 core 容器和 tuple。它的 `coding_companion` 目前不把裸类型参数 T 的已知 `T.Companion` bound 当作默认来源。

因此，在 M57 实现块中声明 `T.Companion : ValueEncoder<T>` 后，**手写方法可以使用它；自动派生也想使用它，就必须补上对应的字段选择合同与 typed 投影引用**。否则会出现“手写 encode 合法，空的 ValueEncoder 实现却仍报缺少字段 codec”的断层。

建议在语言规范 11.13.4 明确：字段为类型参数时，定义环境已有相应 companion bound，才可选择这个投影；未知 T 仍不能猜测。显式完整字段 codec 继续优先，原有合法 private／internal companion 访问不改成 public-only。实例化只替换已选投影，不重新搜索 codec。

这个连接属于既有派生规则的实际扩展，不是要求另建自动注册表，也不应成为 HashMap 手写 helper 首个闭环的前置条件。[M57 第 8.2、12 节](../milestone57/DESIGN.md)已将它列入整体交付，实施时需要落实到这个具体入口。

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

“嵌套容器自动工作”必须逐层成立。例如仅完成 HashMap 后，`HashMap<String, Array<User>>` 仍可能在 Array 的默认 companion 能力处中断；不能把嵌套 HashMap 的成功扩大成所有容器组合均已支持。

## 4. M57 不会替 JSON 库完成的工作

### 4.1 Map 默认 wire 格式与 JSON object 是两件事

当前[普通 map codec](../../sysroot/src/scoop.std/src/collections/map_codec.scoop)使用 `[[key,value], ...]`。即使 key 是 String，编码仍是 entry 数组。M57 默认 HashMap codec 按设计继续复用这个格式。

JSON API 客户端经常需要 `{"name": value}`。建议在 JSON 所属层提供显式的 String-key object codec 或 JsonValue 对象构造入口；默认 generic Map 继续保存任意 key 的类型语义。

M57 不提供整份类型特化或接口实现特化，也不支持 `K == String` 条件；不能再增加一个 String-key 的重叠实现来覆盖默认格式。也不应将任意 key.toString() 作为对象 key：可能碰撞，且不可逆。

### 4.2 多态消息、字段策略和 JSON Schema

普通 enum 默认是外层单 key；LLM API 常见的 `{"type":"text","text":"..."}`、content 数组和开放扩展需要专门 codec。M57 能帮助取得该 codec，不能替它决定 discriminator、flatten 或未知 variant 的处理。

同样，M57 不改变 missing／null／value 三态，不使 Byte 数组自动变成 base64，也不从 custom codec 推出完整 JSON Schema。Schema 可以先用 JsonValue 表达；若后续需要自动生成，须单独定义 wire 策略与覆盖方式。

M57 只衔接已有 codec 合成与条件环境，不替代 M47 的生成器／默认值迁移。它不新增枚举任意字段生成正文的机制，也不能把构造 default 的源码复制到另一作用域后保证绑定不变。

### 4.3 流式与性能

M57 不改变 JSON 语法、字节读取、SSE／NDJSON 定界或工具参数增量。已知静态 codec 可以减少手工组合及部分临时策略对象，但 parser、字段索引、树表示和 I/O 仍须独立改进，不能预先声称有吞吐提升。

## 5. JSON 库本身建议如何改进

### 5.1 第一优先：公开不可变 DOM 与简单入口

保留 JsonValue 的现有 class 和共享不可变节点，补以下普通能力，而不是直接公开内部可变 ArrayList：

| API 能力 | 建议合同 |
| --- | --- |
| `Json.parse(text)`／`Json.stringify(value)` | 分别返回 JsonValue、直接渲染已合法的值；避免用户借助 identity codec 表达普通树操作 |
| `JsonValue.kind` | 明确 null／boolean／number／string／array／object |
| 对象读取与遍历 | key、成员顺序、contains、按 key 取得子值；缺失返回 None，存在的 JSON null 返回 Some(JsonValue) |
| 数组读取与遍历 | Long 下标、大小、不可修改的视图或独立快照；不暴露底层可变 storage |
| 标量读取 | 严格匹配类型；Long／ULong／Float／Double 复用现有范围和精度语义 |
| 受检构造 | null、Boolean、String、数值、数组、对象；对象重复 key 明确报错 |
| 动态数字 | 合法 number token 的受检工厂与读取，保留超出 ULong／Double 的数字，不统一转换为 Double |
| typed 转换 | 补齐 encodeToValue／decodeFromValue 的 M55 默认重载，保留显式 codec |

对象读取遇到非对象应报告类型不匹配，不能伪装成“key 不存在”。构造器只验证新输入并在边界取得快照；已合法的 JsonValue 子节点可共享，不能每次访问或再次输出都重新解析整棵树。

树保留 JSON 值、对象顺序和数值 token；仍不承诺保留原始空白、字符串转义拼写或签名所需的原始消息字节。应用需要原始字节时显式保存输入。

最初可提供小型对象／数组 builder，完成时发布不可变结果；不需要 JSONPath、通用 patch 引擎、可变树和观察者体系作为前置。

### 5.2 泛型 object codec 与常用组合

JSON 的 String-key object codec 可以接收显式 `ValueEncoder<V>`／`ValueDecoder<V>`，同时用 M55 提供元素有默认 companion 时的便利入口。解码返回何种具体容器应在签名中写清，例如保持输入顺序的 LinkedHashMap。

这项能力既能服务工具 schema 和原生扩展字段，也能服务普通 Web API，不依赖 M57 或新增 core Map 协议。它应保持字段冲突报错，不能合并已知字段与扩展时静默覆盖。

现在已有稳定 decoder，定制 codec 可以先检查 discriminator，再在同一节点上读取具体 DTO；无须先 stringify 再 parse。未知字段收集、严格拒绝未知字段也已有组合路径，应补文档和适度 helper。

不能把全局“拒绝未知字段”简单实现为每个 `keyed.end()` 都检查未消费 key：当前允许多个 codec 读取同一 record，且 keyed end 不要求字段全部被读取。若提供严格 helper，它应取得该次检查明确的允许字段集合，保持原容器合同。

### 5.3 修复已能从源码确认的低效路径

| 当前路径 | 影响 | 建议 |
| --- | --- | --- |
| parser 每读一个对象 key 都遍历之前的成员 | n 个字段的重复检查累计为二次量级 | 保留有序成员，增加一次建立的 key 索引 |
| keyed encoder 新增字段时同样线性查重 | 大 schema／动态对象写出成本较高 | 使用相同的普通索引策略 |
| keyed decoder 每次 required／optional 线性查找，keys 每次重新建列表 | 逐字段解码与复读重复做工作 | 不可变 record 共享索引和只读 key 视图 |
| JsonValue 通过通用 encodeToString 先重建树再渲染 | 已有树又分配一棵同形树 | 优先提供直接 stringify；更深的节点复用优化须保持 writer 提交与重复字段合同 |
| HTTP JSON request 先完整生成 String 和 byte array，再检查 maxBytes，并取得发送快照 | 上限限制了最终发送大小，未限制之前的完整物化 | 后续提供有界 UTF-8 写出入口，让构造阶段就能停止，并减少中间表示 |

依据：[parser](../../sysroot/src/scoop.json/src/parser.scoop)、[encoding containers](../../sysroot/src/scoop.json/src/encoding_containers.scoop)、[decoding containers](../../sysroot/src/scoop.json/src/decoding_containers.scoop)、[JsonValue](../../sysroot/src/scoop.json/src/value.scoop)、[HTTP request body](../../sysroot/src/scoop.http.client/src/body/data.scoop)。以上是代码路径分析，尚无本轮性能基准。

JSON 当前仅依赖 core；若为索引使用现有 HashMap／HashSet，可显式依赖只依赖 core 的 std，不需要新 runtime。应先评估真实依赖与构建成本，不在 JSON 内另造完整哈希容器。小对象也应测量，避免索引的固定成本抵消收益。

### 5.4 UTF-8、流式与缺省设置

优先补 UTF-8 byte array 输入／输出，与 HTTP 的实际载荷一致。便利封装可以先复用现有严格 UTF-8 和 Json 实现，但要明确仍会物化完整文档；只有实现直接字节路径后才能声称减少了复制。

对 LLM 客户端，第一步并不需要“任意未完成 JSON 也可解码”：SSE 的完整 data 和 NDJSON 的完整记录各自解析；工具参数原文增量先保留，在调用完成后解析一次。不能每收到一个 delta 都从头解析全部前缀，也不能用补括号把未完成输入当成合法值。

真正的增量 tokenizer／parser 可以作为之后的独立能力，明确完整值、仍需输入、语法错误与 EOF。既有 Decoder 保证随机访问和失败后复读，不能暗中换成单向网络游标。ValueEncoder／ValueDecoder 目前是同步方法，异步 I/O 适配不能在内部阻塞 EventLoop 或偷偷调用 suspend。

建议继续采用严格 JSON、重复 key 报错、无数字／字符串自动强制转换、默认紧凑输出。pretty printing 可以是明确的写出选项。HTTP 对网络 body 设置有限字节上限；JSON 的解析／遍历深度只针对实际实现的栈边界设计，不扩展为跨库通用预算体系。

## 6. 对 M57 实施与验收的具体建议

M57 设计已列明 HashMap、本地／跨 Cone、嵌套查询、方向独立和显式 codec 保留；这些设计方向正确。序列化消费者还应推动以下几项具体验收：

1. **四种能力组合**：encode-only DTO、decode-only DTO、两方向、无 companion；不仅验证 Int／String 两方向齐全的简单情况。
2. **嵌套的真实边界**：嵌套 HashMap、Option／ArrayList 与普通 DTO；单独记录 Array／MutableArray 的支持路径，不把它们混入已完成声明。
3. **固定 companion 的状态**：接口条件与普通字段、构造和初始化独立；同一完整宿主跨 Cone 使用同一 singleton，关系查询不执行初始化。
4. **有 companion bound 的泛型字段派生**：手写调用与自动派生一致；显式完整字段 codec 仍胜过默认来源。
5. **运行期策略保持显式**：Comparator、自定义时间格式、field codec 覆盖、无默认 companion 的外部类型仍可使用旧路径。
6. **wire 行为保持**：Map entry 序列、Option 的 missing／null/default、Result／enum 形状、字段顺序和未知 JSON 回传均有真实结果断言。

两个实现块即可覆盖两种方向的组合。其他接口也可以独立增加，但不因语法已经可用就立即扩展 inspector、schema、hash 等全部能力；每项扩展应有当前消费者，共享 helper 只复用实际相同的算法。

## 7. 推荐实施顺序

| 顺序 | 工作 | 依赖 |
| --- | --- | --- |
| 1 | JsonValue 公开 DOM、parse／stringify、值树默认 codec 重载、可运行 quickstart | 当前语言即可；先修订 JSON 的公共合同 |
| 2 | String-key object codec、HTTP JSON 默认重载、有界 UTF-8 入口 | 当前 M55；分别修改所属库规范 |
| 3 | 复用有序字段索引，消除 JsonValue 的普通输出重建 | 独立性能工作，以实际对象尺寸和调用结果验证 |
| 4 | 随 M57 完成 HashMap 两个条件 codec 接口、四种能力组合和嵌套闭环 | M57 实现及标准库适配 |
| 5 | 其他容器适配及泛型 DTO 条件 codec 派生衔接 | M57 组合验收；逐类型声明已支持范围 |
| 按需求 | 增量 parser、流式字节读写、额外格式策略／schema 生成 | 独立的实际消费者与合同，不作为前几步的前置 |

公共行为变更先落到语言规范 11.13、相关 std／HTTP 规范；M57 的类型语义按其第 12 节同步三份权威规范。JSON 细节可以随后迁到独立规范以便维护，但本次调研不改写当前正式语义。

## 8. 依据与验证范围

本次读取了 M55、M57 设计与 M55 实施记录、三份权威规范的相关部分、std codec 规范、JSON 全部实现、core 编解码／容器 API、Map／Result helper、字段 codec 合成入口，以及 HTTP JSON body 的实际路径。

代表性现有证据：

- [M55 companion JSON 消费者](../../tests/fixtures/language/companion/json/program/src/main.scoop)：两个方向独立、泛型转发、命名 companion、显式替换。
- [M46 JsonValue](../../sysroot/src/scoop.json/tests/values/values/main.scoop)及[组合消费者](../../sysroot/src/scoop.json/tests/consumer/values-composition/main.scoop)：保存、复读、未知字段、字段冲突。
- [容器组合](../../sysroot/src/scoop.json/tests/consumer/serialization-containers/main.scoop)：显式容器策略、路径、完整容器检查。
- [M46 字段与默认值语义](../milestone46/SERIALIZE.md)：编码省略与解码 presence 的既有规则。
- [M57 条件 interface 实现设计](../milestone57/DESIGN.md)：本报告所有未来能力推论的前提。

另在本机 `aarch64-apple-darwin` 用现有 `scoop build`／`scoopc` 和已准备的 sysroot，构建并运行了一个仓库外的独立消费者。四组断言全部通过：

- M55 companion bound 的值树转换 helper，包含只编码、只解码的 DTO。
- 由元素 companion 构造显式 Array encoder／decoder 的普通泛型工厂。
- 嵌套 HashMap 的显式 codec 往返，确认 String key 仍使用 entry 数组 wire 形状。
- JsonValue 保留超出 ULong 的数字，按已有容器 API 区分缺失 key 与显式 null。

实验源码和构建日志保留在仓库外。这是当前能力的抽样运行，不是新 API 的正式 fixture 或性能基准。M57 尚未实现，其普通类型及 companion 的条件接口仅作设计推论，未声称通过编译或运行。

本轮未修改编译器、core、std、JSON 或 HTTP 的实现，也未重跑这些库的全量或三平台验收。
