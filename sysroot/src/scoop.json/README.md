# scoop.json

严格 JSON、不可变值树和基于 `ValueEncoder<T>`／`ValueDecoder<T>` 的类型编解码。
完整 API 合同见 [JSON 库规范](../../../docs/specs/SCOOP-JSON-SPEC.md)。

创建一个普通 executable Cone：

```toml
schema = 1

[cone]
group = "demo.json"
name = "json-example"
version = "0.1.0"
kind = "executable"

[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.std" = "0.1.0"
"scoop:scoop.json" = "0.1.0"
```

将以下内容保存为 `src/main.scoop`，运行 `scoop run --release`：

```scoop
package demo.json

import scoop.json.*

struct User(val name: String, val scores: Array<Int>, val note: String? = None) {
    public companion object : ValueEncoder<User>, ValueDecoder<User> {}
}

fun main() {
    val user = User("Ada", [1, 2])
    val text = Json.encodeToString(user)
    println(text) // {"name":"Ada","scores":[1,2]}
    val copy = Json.decodeFromString<User>(text)
    println(copy.name)

    val tree = Json.encodeToValue(user)
    println(tree["name"]!!.asString())
    println(Json.decodeFromValue<User>(tree).scores[1L])

    val output = JsonObjectBuilder()
        .add("user", tree)
        .add("extra", JsonArrayBuilder().add(JsonValue.nullValue()).build())
        .build()
    println(Json.stringify(output))

    val values = Json.decodeFromString("{\"answer\":42}", Json.objectDecoder<Int>())
    println(Json.encodeToString(values, Json.objectEncoder<Int>()))
}
```

编码和解码是独立能力。只发送的 DTO 可只声明 ValueEncoder；只接收的 DTO 可只声明
ValueDecoder。Array、MutableArray、ArrayList、Option 和 HashMap 的默认 codec 按元素
companion 的对应能力组合，不要求数据实例实现这些接口。没有默认能力时，继续传入显式
codec 或使用容器的 encoder／decoder 工厂。

`Json.parse(text)`返回完整校验后的值树，`Json.stringify(value)`直接输出已有树。
对象下标返回 Option：缺少字段为 None，显式 JSON null 是 Some，子值 kind 为
JsonValueKind.Null。数组下标使用 Long；错误种类抛 DecodingException，越界抛
IndexOutOfBoundsException。keys／members／elements 返回独立数组，子值保持不可变。
builder 的 build 是快照，后续 add 不影响旧值，重复对象字段报错。

数字保留原 token，可用 asNumberToken 读取或 fromNumberToken 受检构造，不必转换成
Double；typed 数值访问仍检查范围。fromNumberToken 不接受前后空白，parse 可接受文档
空白。字段名中的 `/` 和 `~` 在异常 path 中分别转义为 `~1` 和 `~0`。

标准库 HashMap 默认仍使用 `[[key,value], ...]`。需要 JSON object 时显式选
Json.objectEncoder／objectDecoder；编码接收 Map<String,V> 视图，解码返回新的
LinkedHashMap<String,V> 并保持输入字段顺序。两工厂都接受显式元素 codec，可替换默认
策略。普通 enum、Option 的既有 wire 规则不变，字段级 codec 注解不在当前范围。

HTTP client 可直接使用 RequestBody.fromJson(value, maxBytes) 和
response.body.json<T>(maxBytes)。maxBytes 仍须显式提供，解码仍消耗一次响应 body；
有自定义策略时使用原来的显式 codec 重载。
