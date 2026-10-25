# Scoop JSON 库规范

`scoop:scoop.json:0.1.0`是显式依赖core和std的普通Cone，公开package为`scoop.json`。类型codec、字段派生和默认表示遵守[语言规范11.13](SCOOP-SPEC.md#1113-编码解码与缺省实现)。本库不增加语言语法、运行期反射或runtime ABI；字段级codec定制不在本次范围。

## 1. 文本、值树与typed入口

Json保留显式`encodeToString(value, codec)`、`decodeFromString(text, codec)`、`encodeToValue(value, codec)`、`decodeFromValue(value, codec)`。四个入口均有省略codec的普通泛型重载：编码要求`T.Companion : ValueEncoder<T>`，解码要求`T.Companion : ValueDecoder<T>`，分别转发T.Companion。显式codec不会被默认策略替换。

`Json.parse(text: String): JsonValue`严格解析一个完整JSON文档，包括所有子树的语法、转义、重复key和尾随输入；错误为带JSON Pointer path的DecodingException。`Json.stringify(value: JsonValue): String`直接渲染已合法的值树，默认紧凑输出，不重新编码、复制或解析整棵树。它保留对象字段顺序、数组顺序及数字token，不承诺原始空白或字符串转义拼写。

encodeToValue遵守根encoder完成／提交规则。decodeFromValue以空串作为新的根path，不保留该JsonValue在来源文档中的位置。用户codec、构造器和default抛出的异常照常传播。已有String入口的整数、浮点、null和未知字段规则不变。

## 2. 不可变JsonValue

JsonValue仍为内部节点构造器不可公开调用的class，companion仍实现自身的ValueEncoder／ValueDecoder。节点和引用子树均不可由公开API修改；JSON格式专属codec不能交给其他格式backend。

`JsonValueKind`有Null、Boolean、Number、String、Array、Object六个variant。

| API | 合同 |
| --- | --- |
| `kind: JsonValueKind` | 当前JSON种类，O(1) |
| `size: Long` | 对象字段数或数组元素数，O(1)；其他种类报DecodingException |
| `asBoolean()`／`asString()` | 严格匹配种类，无字符串／布尔／数值隐式转换 |
| `asLong()`／`asULong()`／`asFloat()`／`asDouble()` | 复用typed解码的范围、精度和负零规则，整数拒绝小数和指数token |
| `asIntegralLong(): Long` | 显式读取数值恰为 Long 整数的 number；允许小数／指数写法，不接受 string 或经过浮点舍入 |
| `asNumberToken(): String` | 返回原合法数字token，不经浮点转换，支持超出ULong／Double范围的数值 |
| `get(name: String): JsonValue?`、`containsKey(name)` | 仅接受object。缺少key为None；显式null为Some(JsonValue)，kind为Null。线性查找 |
| `get(index: Long): JsonValue` | 仅接受array，O(1)；越界抛IndexOutOfBoundsException |
| `keys(): Array<String>` | 按对象保存顺序返回独立数组，O(n) |
| `members(): Array<(String, JsonValue)>` | 按对象保存顺序返回独立成员数组，O(n)，共享不可变子节点 |
| `elements(): Array<JsonValue>` | 按数组顺序返回独立元素数组，O(n)，共享不可变子节点 |

两个get均为operator。错误种类抛根path为空串的DecodingException，不能把非object伪装成缺少key；子值单独访问按新根处理。

companion提供`nullValue()`、`fromBoolean(value)`、`fromString(value)`、`fromLong(value)`、`fromULong(value)`、`fromFloat(value)`、`fromDouble(value)`工厂。浮点仅接受有限值，拒绝时抛EncodingException，保留负零。`fromNumberToken(text)`只接受恰好一个JSON数字token，不接受任何前后空白或其他值；非法输入抛DecodingException，合法输入保留原token。

`fromArray(elements: List<JsonValue>)`和`fromObject(members: List<(String, JsonValue)>)`在调用时复制集合结构并共享子节点。object保持输入顺序，重复name抛EncodingException，path指向该字段并按JSON Pointer转义，不静默覆盖。

asIntegralLong 直接解释已经合法的十进制 number token：`12.0`、`1.2e1`、`9007199254740993.0` 保持精确；非整数、超出 Long 范围或非 number 抛根路径 DecodingException。正负零及带任意合法指数的零均为 0，不把非零小数下溢为零。处理不分配指数规模的缓冲、不按指数逐次乘除；工作量以已有 token 长度为界，最多累积 Long 容纳的十进制位数。原 token 不变，asLong／typed Long 解码仍执行原有不带小数或指数的词法合同。

`JsonArrayBuilder()`提供可链式`add(value: JsonValue)`；`JsonObjectBuilder()`提供可链式`add(name: String, value: JsonValue)`并在重复name时抛EncodingException且不改变已有成员。两者的`build(): JsonValue`每次发布独立的结构快照，之后继续add或再次build不改变旧值；共享不可变子节点，不重复校验已经合法的子树。它们不提供可变DOM、JSONPath或patch协议。

## 3. String键object codec

| Json工厂 | 返回类型 |
| --- | --- |
| `objectEncoder<V>(codec: ValueEncoder<V>)` | `ValueEncoder<Map<String, V>>` |
| `objectDecoder<V>(codec: ValueDecoder<V>)` | `ValueDecoder<LinkedHashMap<String, V>>` |
| `objectEncoder<V>() where V.Companion : ValueEncoder<V>` | 同上，转发V.Companion |
| `objectDecoder<V>() where V.Companion : ValueDecoder<V>` | 同上，转发V.Companion |

encoder按Map的迭代顺序写keyed字段，key直接作为字段名，value交给所选codec。decoder要求object，按输入顺序解码并返回新的LinkedHashMap；重复key沿完整JSON解析规则拒绝。两方向独立，不捕获或改写元素codec异常，path沿当前child传播。encoder接收Map视图，HashMap和LinkedHashMap都可按普通子类型转换传入；decoder的具体返回类型不由调用者猜测。标准库Map的默认entry序列codec保持原义，不自动将String键特化成object。

```scoop
val data = Json.parse("{\"answer\":42,\"note\":null}")
val answer = data["answer"]!!.asLong()
val hasNote = data.containsKey("note")
val output = JsonObjectBuilder()
    .add("answer", Json.encodeToValue(answer))
    .add("items", JsonArrayBuilder().add(JsonValue.nullValue()).build())
    .build()
val text = Json.stringify(output)

val values = Json.decodeFromString("{\"answer\":42}", Json.objectDecoder<Int>())
val restored = Json.encodeToString(values, Json.objectEncoder<Int>())
```

## 4. 与其他库的边界

Array、MutableArray、ArrayList、Option的默认codec由core的普通条件companion提供，HashMap由std提供；JSON只消费已成立的接口。HTTP默认JSON入口见[HTTP client规范](SCOOP-HTTP-CLIENT-SPEC.md#92-有界读取文本与-json)，保留maxBytes、严格UTF-8和一次消费规则。本次不增加UTF-8／流式JSON、pretty printing或字段级codec注解。
