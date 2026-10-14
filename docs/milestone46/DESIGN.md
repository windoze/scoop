# M46 设计：序列化基础协议与 DTO 合成

状态：实施中，M46-1 已交付并完成本机针对性验证；其余批次待实施。日期：2026-10-14。调研基线：`e548b9ebe`。

依据：[语言规范](../specs/SCOOP-SPEC.md) 9.4、9.5、11.13，[编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md) 2.17，[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)及[路线图](../ROADMAP.md)。本文规定 M46 的目标合同、实现边界与实施顺序；[统一 Serialize 与 DTO](SERIALIZE.md)规定字段合成和 Option 默认行为；[基线与验收](BASELINES.md)给出外部格式的覆盖要求。实际进度与验证见[实施记录](PROGRESS.md)；每批实施前先同步相应 spec，再修改代码。

## 0. 范围与决定

M46 补齐普通 codec 组合所需的基础能力，并让常见 DTO 直接使用合成编解码。增强后的合成与普通手写 codec 共同支持 OpenAI Chat Completions／Responses、Anthropic Messages 和 `Cone.toml` 的完整数据形状。重点来自 Swift Codable／Foundation 的对照，不以本期自动合成达到 serde derive 的全部配置能力为目标。

1. `Decoder` 表示一个稳定的输入值。容器是该值的视图；允许复读字段、复读标量，以及一次候选解码失败后对同一值重新尝试。
2. 每次取得 unkeyed 解码容器时建立独立游标。保留现有 `element(): Decoder` 表面，明确取得子入口与解码子值的不同推进时机。
3. `SingleValueDecodingContainer` 增加 `isNull(): Boolean`。协议保留缺失字段、显式 null、有值的区别；普通 DTO 的可空字段按下述默认规则处理。
4. 同一个 `Encoder` 可重复取得同类容器，共享一个对象或序列的写入状态。保留单值单次写入、重复 key 错误和子值完整性检查。
5. 保留 `end()`，但区分解码视图的局部检查、编码当前写入的完成，以及父容器／根入口对输出值的最终提交。
6. `scoop.json` 提供不可变的 `JsonValue` 子树值及普通 codec，用于保留任意 JSON、未知字段和精确数字文本。这是格式库能力，不进入 core 数据模型。
7. 用统一的 `Serialize(rename, required, emitNull, ignore)` 替换 `SerialName`／`Transient`；从字段类型和 default 推导可缺省性。默认 Option codec 改为 None ↔ null、Some(v) ↔ v，父 record 决定是否省略字段，详见[配套设计](SERIALIZE.md)。此项按后续讨论加入，替代初稿中不改 annotation／Option 的限制；字段 codec 的选择优先级保持。
8. `Cone.toml` 使用等价 JSON 评估；不新增 `scoop.toml`，不迁移编译器的 manifest parser。API 基线使用离线数据与普通 Scoop codec，不交付 SDK、HTTP／SSE／WebSocket 实现或在线调用。
9. 字段 codec 的声明式绑定，以及 tagged／untagged／flatten／批量命名／未知字段策略的自动合成仍不在本期范围。

## 1. 三级边界与 Swift 对照

### 1.1 改动归属

| 层次 | 当前能力与缺口 | M46 责任 |
| --- | --- | --- |
| ① core 协议及语言合同 | 已有显式 `Encodable<T>`／`Decodable<T>`、三类容器、字段存在性、全量字符串 key、位置和异常；目前合同按一次选择、顺序消费描述，缺少 null 探测 | 增加 `isNull`，规定复读、独立解码游标、编码容器共享与 `end`／提交；定义普通 Serialize 注解，修改普通 Option codec |
| ② 编译器合成 encode／decode | 已生成普通 typed body，负责字段 codec、构造、default 与 enum 分支；Option 字段没有自然的缺失与省略规则 | 统一字段配置，合成可缺省／required／None 省略／ignore 补值；保持 codec 选择、定义处绑定和普通构造，验证合成 codec 的复读与容器组合 |
| ③ `scoop.json` Cone | 已有完整 JSON parser、内部数值 token 和树；`selected`、`consumed`、`pending`、`completed` 状态阻止复用，内部树没有公开的子值 codec | 实现新合同，保留完整文档语法检查，提供 `JsonValue` 和不经文本中转的值转换 |

① 的合同变化必须进入语言规范，不能只放宽 JSON 实现。② 的字段分支生成为普通 typed body；Option 的值表示由普通 core codec 实现。两者均不增加 intrinsic、专用 IR 或 JSON 特判，第三层也不解释 Serialize。

### 1.2 Swift 提供的基础

| 问题 | Swift 的处理 | Scoop 的选择 |
| --- | --- | --- |
| 同一值尝试不同解码 | Codable 将解码容器描述为值的 view；Foundation `JSONDecoder` 可以在标量类型失败、结构候选部分读取后继续尝试 | 把稳定输入与失败不污染后续读取写成 Scoop 的明确合同 |
| 重复取得 keyed 编码容器 | Foundation 的同类请求包装同一个对象存储，可供多个手写 `encode(to:)` 共同写入 | 共享字段集合、待完成子值及重复 key 检查 |
| null 检测 | `decodeNil()`；还提供 keyed／unkeyed 的便利方法 | 最小新增是 single-value `isNull()`；keyed 经 `optional` 取得子入口，unkeyed 经 `element` 取得子入口 |
| Optional 字段 | 合成通常使用 `decodeIfPresent`／`encodeIfPresent`，缺失与 null 归为 nil，编码 nil 省略；Optional 自身的 codec 使用 nullable 值 | 默认 Option codec 使用 nullable 值；合成可缺省字段，增加 required／emitNull；保留 Scoop 已有的缺失时使用 default，并避免 None 省略后恢复成非 None default |
| unkeyed 游标 | Swift 的 typed `decode` 在成功后推进；重新从 decoder 取得容器可从头读取 | Scoop 是先 `element()` 再调用 codec，不能照搬 typed `decode` 的推进规则，见第 3 节 |
| 未知字段 | 手写 `CodingKey`、`allKeys` 和显式校验；Foundation 的通常行为是忽略未知字段 | 现有 `keys: List<String>` 已能枚举全部输入 key，无需新增协议 |
| 字段转换、日期、命名 | 手写 Codable；日期／key 策略属于 Foundation；`CodingKeys` 参与合成 | 统一 Serialize 承接单字段重命名与忽略；字段 codec 绑定及格式策略注入仍留后续 |
| enum／flatten | Swift 合成关联值 enum 有固定嵌套形状；其他 tag 方案、flatten、未知字段收集通常由手写 Codable 完成 | 补足手写实现需要的操作，合成扩展另行设计 |

2026-10-14 的本机 Apple Swift 6.4／Foundation 实测确认：`decodeNil` 后读取字符串、失败的整数读取后重读字符串、第一结构候选部分读取失败后用第二候选解码，以及多个 keyed encoder 写入同一对象均成立。这里区分协议公开表面和 Foundation 实现的实测行为，不把 Foundation 的全部实现细节当作所有第三方 Swift Decoder 的保证。

参考：[Codable 协议源码](https://github.com/swiftlang/swift/blob/main/stdlib/public/core/Codable.swift)、[SE-0166](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0166-swift-archival-serialization.md)、[SE-0167](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0167-swift-encoders.md)、[SE-0295](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0295-codable-synthesis-for-enums-with-associated-values.md)、[Foundation JSONDecoder](https://github.com/swiftlang/swift-foundation/blob/main/Sources/FoundationEssentials/JSON/JSONDecoder.swift)、[Foundation JSONEncoder](https://github.com/swiftlang/swift-foundation/blob/main/Sources/FoundationEssentials/JSON/JSONEncoder.swift)。这些链接的 main 分支会变化，以上结论按调研日期记录。

## 2. 稳定输入、字段复读与 null

### 2.1 Decoder 是值入口

一个 `Decoder` 绑定一个逻辑输入值及其 `path`。取得容器、成功读取、类型不匹配、数值越界、缺失字段或候选 codec 失败，都不消耗或改变这个值。正常保存的 decoder／视图由普通托管引用保持可用，不因取得下一个字段而失效。

| 操作 | 目标合同 |
| --- | --- |
| `keyed()` | 输入为 record 时返回视图；重复请求合法；形状不匹配抛 `DecodingException`，后续仍能取得其他视图 |
| `singleValue()` | 对任何合法输入值都能取得查询视图；具体标量读取再检查类型，不能在取得视图时把 record／sequence 永久排除 |
| `required(name)` | 每次定位同一个字段值；缺失在该字段路径报错，不记录“已读”状态 |
| `optional(name)` | 缺失为 `None`，存在为 `Some(child)`，包括显式 null；重复查询结果一致 |
| `keys` | 返回本层全部输入 key，与读取次数、读取顺序和所选 codec 无关；不提供“尚未消费的 key”语义 |
| `readBoolean`／整数／浮点／字符串／`readNull` | 成功后允许再次读取；不匹配或范围错误不污染 decoder。原有格式和数值转换规则保持不变 |
| keyed `end()` | 进行本层必要检查；不关闭输入、不禁止复读、不要求未知字段全部读取；重复成功调用无额外效果 |

容器请求不能把 decoder 锁定为一种类别。例如先 `singleValue().isNull()` 得到 false，再 `keyed()` 合法；先 `keyed()` 失败，再通过 single-value 视图 `readString()` 也合法。JSON 对象仍不能被强制转换成字符串，允许重试不改变成功条件。`keys` 暴露只读视图或独立快照，不能让调用方借返回的集合改变输入或后续查询结果。

不要求每次请求返回不同的对象 identity；要求可观察行为符合视图语义。JSON 应共享已解析的输入节点，不为每次候选重试重新解析或深拷贝整棵树。

### 2.2 最小新增接口

在现有 `SingleValueDecodingContainer` 中新增普通 requirement：

```scoop
public fun isNull(): Boolean
```

对于合法值，null 返回 true，其他值返回 false。查询不推进任何游标、不要求接着调用 `readNull()`，也不改变后续读取结果。`readNull(): Unit` 继续表示“必须为 null”，非 null 时抛错。

| 输入情况 | `record.optional("x")` | 子入口 `isNull()` | 业务 codec 可区分的状态 |
| --- | --- | --- | --- |
| 没有 `x` | `None` | 没有子入口 | 缺失 |
| `"x": null` | `Some(child)` | true | 显式 null |
| `"x": value` | `Some(child)` | false | 有值，随后按字段 codec 读取 |

普通 DTO 可以直接使用 `T?`／`Option<T>`：可缺省字段没有 default 时补 None，显式 null 经默认 Option codec 也得到 None；已有显式 default 仍仅在缺失时执行。是否省略字段由父 record 的合成或手写 codec 在调用 `field(name)` 前决定。required、emitNull 及防止 default 改变往返结果的规则见[统一 Serialize](SERIALIZE.md)。

这种 DTO 默认行为不保存“输入是否出现过 key”；协议仍允许手写 codec 完整观察三种输入状态。一般 DTO 不需要三态 enum；确实需要时，应用可以增加普通 `FieldState<T>` 的 Missing／Null／Value 分支，由父 record codec 或字段 helper 完成映射，详见[三态扩展约定](SERIALIZE.md)。这必须作为 M46 的扩展能力验收，不要求修改 core 协议或 JSON 实现。字段 map／JsonValue 仍是另一种保存原始输入的方式；两个状态的 Option 本身不承载三种状态。

`isNull` 不通过捕获任意异常实现为“失败即 null”。没有 null 的格式可直接返回 false；真实格式错误仍由格式实现报告。

### 2.3 重试的边界

复读保证的是输入位置及内容，不回滚用户代码执行。codec、default、constructor 对外部变量的写入或其他副作用仍按实际执行次数发生。M46 不增加事务、检查点或自动候选回退。

untagged codec 自行规定候选顺序、歧义选择与最终错误。例如先尝试一个结构候选，捕获其 `DecodingException` 后从同一 decoder 尝试另一个；普通业务异常不应被框架自动吞掉。带明确 tag 的已知分支如果字段损坏，应报告该分支错误，不能自动退化为未知分支并掩盖错误。

## 3. Unkeyed 解码视图

每次 `decoder.unkeyed()` 从同一个序列建立一个新游标，初始位置为 0。持有同一个容器引用的调用方共享该游标；不同请求得到的视图互不推进。

1. `hasNext` 只观察当前视图，既不推进，也不要求上一个子 decoder 已经被某个 codec 完成。
2. `element()` 成功取得当前子入口后，当前视图推进一位。到达结尾时抛错且不推进。
3. 子 codec 的成功或失败不会再修改父视图游标。要重试同一元素，保留该 `element()` 返回的 decoder；再次调用 `element()` 表示请求下一个元素。
4. 要重试整个序列，重新从原 decoder 取得 unkeyed 视图。第一候选读取了部分元素，不妨碍第二候选从头开始。
5. `end()` 检查当前视图是否到达末尾。还有元素时抛错且不推进；调用方仍可继续读取。成功后重复调用合法，也不影响其他视图。

例如输入 `[{"id":7,"text":"ok"},2]`：外层先取得元素 0 的 decoder，第一种结构解码读完 `id` 后失败，第二种结构仍能从同一子 decoder 读取 `id` 和 `text`；父游标始终只推进了一位，下次 `element()` 返回 2。

这里的“遍历到结尾”不表示每个子值的所有字段都已被业务代码访问。业务 codec 可以主动忽略某个合法子值。tuple／固定位置 payload 的长度检查仍由相应 codec 的读取次数和 `end()` 完成，不能因放宽复读而省去。

## 4. Encoder 的共享容器与提交

### 4.1 同类请求共享一个输出值

一个 `Encoder` 仍只产生一个值。首次取得 keyed／unkeyed／single-value 容器决定其类别；后续同类请求合法，跨类别请求抛 `EncodingException`。这里与可以尝试不同视图的 Decoder 有明确区别。

| 类别 | 重复取得容器后的行为 |
| --- | --- |
| keyed | 共享同一字段集合、待完成子值与 key 唯一性检查；第二个 codec 可以追加字段 |
| unkeyed | 共享同一元素序列及追加位置；不是从头建立一个新输出序列 |
| single-value | 共享同一次标量写入机会；取两次视图不等于可以写两次值 |

不保证各次返回值的对象 identity 相同，只保证共享状态。重复字段名始终是错误，包括不同视图、不同嵌套 codec 或未知字段 map 与固定字段之间的冲突；不采用覆盖或最后写入优先。

### 4.2 `end()` 完成当前写入，父容器提交子值

保留顺序完成编码子值的规则，明确两个边界：

- **当前值完成**：keyed／unkeyed `end()` 完成当前 pending 子值，标记本层已经写入的内容完整。该操作不永久关闭当前值；提交前重复 `end()` 合法。
- **当前值提交**：父容器开始下一个 field／element，或父容器 `end()` 接收最后一个子值时，检查并提交该子值；根入口在 codec 返回后检查并提交根值。提交后的值不能再写入或扩充。

提交前，多个 codec 可依次对同一个值写入。已有一次成功 `end()` 后再新增 field／element，会使本层重新需要 `end()`；旧的完成标记不能掩盖新开的空子值或未完成容器。仅重复取得视图，不算新增写入。

所有视图共享 pending 状态。任何视图开始下一子值或执行 `end()` 时，都必须看到上一视图留下的子值；未完成时抛错，不能跳过或隐式丢弃。单值在恰好一次成功写入后完成；空 record／sequence 也要显式 `end()`。

父容器接收已经完成的子值后，后续保留的子 encoder 不能再修改它。这样 `JsonValue` 和最终输出不受过期 writer 引用影响，也不必在根出口重新遍历每个子树确认没有迟到写入。必要检查随实际写入和提交逐层完成，已提交且不再变化的节点复用结果。

| 调用场景 | 结果 |
| --- | --- |
| A 取得 keyed，写自身字段并 `end()`；B 对同一 encoder 取得 keyed，追加字段并 `end()` | 一个包含两组字段的 record |
| A、B 的 keyed 视图交替写不同字段，最后 `end()` | 合法，保持实际写入顺序 |
| A 取得 field 子入口但没有完成；B 尝试开始下一字段 | 报未完成子值，不提交半个字段 |
| 一次 `end()` 后追加字段，但未再次 `end()` 就返回 | 根／父提交时报该层未完成 |
| 父容器已经推进，继续写原子 encoder | 报该值已提交，不能改变父输出 |
| 两个 single-value 视图各写一次 | 第二次写入报错 |

编码错误后不承诺回滚已经发生的写入，也不提供失败候选的重试。调用方可以丢弃该根 encoder 后重新开始；解码复读合同不能套用到编码。

### 4.3 组合的直接效果

手写 flatten codec 可以先写外层字段，再把同一 encoder 交给内部 record codec；内部合成 codec 的 `end()` 不妨碍其他字段继续追加。手写 internally tagged enum 可以先写 `type`，再把同一 encoder 交给 payload codec。adjacently tagged 继续使用独立 `tag` 和 `content` 子入口。

解码时各部分对同一 decoder 取得自己的 keyed 视图，读取 discriminator 不会消耗 payload 所需字段。严格未知字段策略必须检查组合后的允许 key 集合，不能让两个各自拒绝所有外来字段的 record codec 直接互相否定。

## 5. JSON 实现与任意子值

### 5.1 完整语法消费与业务读取分开

`Json.decode` 先解析一个完整 JSON 文档，继续拒绝尾随非空白内容、重复 key、非法数字语法及非法字符串。未知字段和被业务 codec 忽略的子树也必须是合法 JSON。

解析成功后，由业务 codec 决定访问哪些值。移除根层和 pending 子值上的“一次性完整消费”要求；不再用 `requireComplete()` 判定某个候选是否污染了整个文档。手写 codec 只读取 tag、忽略若干字段，甚至明确返回一个常量，均不构成 JSON 语法消费错误。

现有 `JsonDecoder` 的 `selected`／`completed`、keyed 的 `consumed` 以及阻止其他读取的 `pending` 状态应移除或按上述职责收敛。unkeyed 保留视图局部游标。encoder 按第 4 节保留必要的共享写入、完成与提交状态。两侧不共享一套一次性状态机。

JSON 的原有数值合同保持不变：整数不经 Double，含小数部分或指数部分的 token 不作为整数字段接受；Float／Double 按目标精度解析并检查溢出，非有限值不能编码。复读不增加字符串／布尔值强制转换。

### 5.2 `JsonValue` 的最小公共表面

API 的 JSON Schema、工具输入以及未知扩展字段允许任意递归 JSON。只用 `Long | ULong | Double` 的普通联合无法精确保留所有合法数字；例如大于 ULong 的整数、长小数和 `1e400` 都能是合法 JSON token。此缺口由 `scoop.json` 解决。

提供公共、不可变的 `JsonValue`，其 companion 以普通手写方法实现 `Encodable<JsonValue>` 和 `Decodable<JsonValue>`。它表示一个已经合法解析或完整编码的 JSON 值，不公开可变的内部数组，不提供未经校验的原始文本拼接入口。

沿用现有显式 codec 入口即可解析和输出该值：

```scoop
val value = Json.decode(text, JsonValue.Companion)
val textAgain = Json.encode(value, JsonValue.Companion)
```

在 `Json` 中增加两个普通泛型函数；以下只列新增公共签名：

```scoop
public fun <T> encodeValue(value: T, codec: Encodable<T>): JsonValue
public fun <T> decodeValue(value: JsonValue, codec: Decodable<T>): T
```

`encodeValue` 使用与 `Json.encode` 相同的根编码及提交规则，返回树值；`decodeValue` 为现有树值建立新的根 decoder，路径从空串开始。两者用于构造工具 schema、延迟解码已保存的未知字段，以及重新用已知 codec 解释一个子值，不经 stringify／parse 中转。文本入口和树值入口共用编码／解码实现。

`JsonValue.Companion.decode(child)` 可在任意 JSON 子入口保存该子树；之后仍可从原 child 进行 typed 解码。保存的值只持有不可变节点，不捕获整个 decoder 的游标或 codec。编码根据节点形状使用 JSON 自身的 keyed／unkeyed／single-value 路径：record／sequence 遵守第 4 节的共享及完成规则，标量只写一次，Number 节点使用本 Cone 内部的精确 token 写入能力。根／field／element 仍各自产生一个完整输出值。

该 codec 是 JSON 专用的：可在普通方法中以类型检查进入本 Cone 的内部 encoder／decoder 能力；不兼容的格式入口抛对应的带路径异常。无需把原始数字 token、`JsonValue`、`Any` 树、格式注册表或新的 operation role 放进 core，也不要求所有未来格式无损承载 JSON 的特有值域。

内部表示复用当前 JSON 的 Null／Boolean／Number／Text／Record／Sequence 节点结构；现有 internal `JsonValue` 可以改名为内部节点类型，避免另建一套平行树。数值节点保留合法 token，不能捕获时转为 Double、输出时再格式化。公开值没有可变访问路径，因此保存及复读可以共享节点；编码 builder 在提交前仍是独立的可变状态。

### 5.3 保留程度

要求保留 JSON 类型、对象字段、数组顺序、字符串内容和数字 token。对象输出使用保存的顺序，不承诺恢复原始空白、缩进和字符串转义拼写。`"{\"x\":1}"` 是字符串；`{"x":1}` 是对象，两者不能在捕获或再次编码时互换。

现有 `scoop.std` Map codec 的 pair-sequence 默认表示不变。需要 JSON object 的 `Map<String, T>` 时，普通 codec 遍历 keyed 容器并调用 value codec；未知字段 map 使用 `JsonValue` 作为 value 即可。生产 JSON Cone 继续只需 core，测试若使用 std Map 则显式增加测试依赖。

## 6. 在基础之上实现其他功能

| 用户能力 | 增强后普通代码的做法 | 本期不加入的自动规则 |
| --- | --- | --- |
| 常见 DTO 的重命名、忽略、可缺省／可空字段 | 本期直接使用 Serialize 与 Option 默认 codec；也可手写父 codec | 按任意条件 skip、比较 default 后省略及隐藏 presence 状态 |
| 特定字段 datetime ↔ string | 父 codec 在该 field 子入口显式调用所选 codec；也可用单独 wire DTO | 字段 codec annotation、函数／类型作为 annotation 参数 |
| internally tagged enum | 读取 tag，选 payload codec，再对同一 decoder 解码；编码时共享 keyed 状态 | tag／variant 映射的合成配置 |
| adjacently tagged enum | 一个字段读写 tag，另一个字段作为 payload 子入口 | 自动 `tag`／`content` 配置 |
| untagged enum | 普通控制流依次尝试 codec；保存同一值／元素 decoder | 候选排序、歧义规则、自动诊断聚合 |
| flatten | 多个 record codec 共享输入值或输出 keyed 状态；显式处理字段冲突和允许 key 集合 | 字段展开、默认值和冲突规则的合成 |
| 八种 rename-all 形式 | 显式 wire 名，或普通字符串命名 helper | lowercase、UPPERCASE、PascalCase、camelCase、snake_case、SCREAMING_SNAKE_CASE、kebab-case、SCREAMING-KEBAB-CASE 的批量合成 |
| allow／deny unknown fields | 忽略未选 key，或用 `keys` 检查允许集合 | 全局／类型级／字段级策略 annotation |
| collect unknown fields | 从 `keys` 取业务定义的剩余集合，以指定 value codec 写入 map；任意 JSON 用 `JsonValue` | 自动收集字段及其合成初始化规则 |

因此，本期也不需要新增 core `contains`、`decodeIfPresent`、unkeyed peek／rewind、`superDecoder`、动态字段反射或 `userInfo` 表。已有字段入口、普通泛型 helper、codec 构造参数与控制流足以表达本期基线。若后续要降低手写成本，再以真实重复用法设计相应便利 API。

## 7. 实施顺序与规范同步

| 批次 | 完整交付 | 同批验证 |
| --- | --- | --- |
| M46-1 | 先修订 Decoder／null 合同；更新 core requirement 和所有实际 implementor；JSON 解码改为稳定视图，包含独立 unkeyed 游标 | 标量／字段／结构候选复读，null 三态，游标与错误恢复；旧合成 default、tuple 长度和完整 JSON 语法回归 |
| M46-2 | 编码容器共享，明确当前完成与最终提交；保留重复 key、跨类别与未完成值错误 | 手写与合成 codec 的 flatten／tagged 组合，多视图交替写入，`end` 后追加，提交后禁止改写 |
| M46-3 | 按[统一 Serialize](SERIALIZE.md)迁移旧注解、修改普通 Option codec、完成可缺省字段与 None 省略的合成 | 类型／default／required／emitNull／ignore 矩阵、codec 求值顺序、Option 嵌套与容器组合；更新兼容性断言 |
| M46-4 | JSON 不可变 `JsonValue`、companion codec、`encodeValue`／`decodeValue`；复用内部树与数值 token | 任意子树、精确数字、未知字段 map、延迟 typed 解码、JSON 专用 codec 的格式不匹配错误 |
| M46-5 | 按[基线文档](BASELINES.md)实现普通 Scoop 的代表性 wire DTO／codec 和离线测试，完成参考 schema 的形状覆盖说明 | Chat／Responses／Messages 请求、响应、完整流事件与错误；完整 Cone manifest 字段与严格失败情况 |
| M46-6 | 现有三 target 的必要回归、普通 `.slib` 生产／消费与托管引用组合验收；更新完成状态及验收记录 | 无源码消费合成及手写 codec，跨 Cone `JsonValue` 调用，持有多视图期间 GC 正确性 |

规范同步位置：

| 文档 | 实施时的变更 |
| --- | --- |
| 语言规范 9.4 | 定义 Serialize、迁移旧注解；继续使用现有字符串／布尔参数，不扩展 annotation 值域 |
| 语言规范 11.13.2 | 新增 `isNull`；替换一次性选容器／消费描述，分别规定编码和解码、游标、`end` 与提交 |
| 语言规范 11.13.3 | 修订 Option 值表示、字段缺省／省略和 ignore；保留 default 延迟求值、普通 enum 外层形状与数值规则；补充 JSON 树值入口 |
| 语言规范 9.5、11.13.1／11.13.4 | 同步合成的字段存在性分支及求值顺序；保持显式 conformance、codec 选择优先级与定义处绑定，不新增绑定来源 |
| 实现规范 2.17 | 统一静态字段配置，生成普通 None／presence 分支并按新 core 合同调用；继续普通 typed body、普通成员导出与实例化，无 JSON 专用 IR |
| 运行时规范 | 仅核对引用，不新增 runtime ABI、native 后备、GC 对象种类或异常机制 |
| 标准基础库规范 | 核对组合使用新的 Option codec；Map 的 pair-sequence 外形、时间 codec 等其他默认合同保持 |

新的普通接口成员会要求现有第三方 `SingleValueDecodingContainer` 实现补齐 `isNull`，其实现按自身格式回答。相关 core／依赖产物按现有依赖和公共接口规则重建；不为一次普通接口变化另加 wire section、兼容 token 或重复的跨 stage 语义验证。

`Json.decode` 不再要求业务代码遍历整个输入值，是本期有意的兼容性变化。依赖旧“一次消费”错误的测试应改成稳定视图语义；格式非法、已知字段类型错误、合成必需字段缺失、固定序列长度和编码未完成等错误继续保留。注解迁名、默认 Option wire 格式及 Option 字段缺省行为也是有意的变化；具体迁移与信息保留边界见[统一 Serialize](SERIALIZE.md)。

## 8. 验收要求

新增协议特性各有独立 fixture，并有手写 codec、合成 codec、泛型组合的集成 fixture。core 新 requirement 的普通接口实现诊断有准确的 negative fixture；运行期错误断言异常种类和路径。JSON 行为与基线组合可由 `scoop.test` 承担，不为每个字段保存 IR／符号表快照。

验证按实际改动选择：Rust 变更先 fmt／clippy，再执行相关 crate 测试和正式 CLI fixture；fixture runner 有修改时执行其规则测试。普通库行为完成后不反复扩大到无关全量套件。最终在现有 Darwin／Linux GNU／Linux musl target 上运行本期适用测试及必要既有回归。

M46 完成必须同时证明：

1. 第 2～4 节的复读、null、游标、共享写入及错误边界成立；成功分支不受此前失败读取影响。
2. [Serialize 与 Option 设计](SERIALIZE.md)的缺失／null／省略／default 规则成立，普通用户三态 enum 可由父 codec 无损映射到缺失／null／非 null 值；普通 enum 外层单 key、tuple 精确长度、default 延迟求值和 Float／Double 精度保持原合同。
3. [API 与 Cone 基线](BASELINES.md)所列形状都可用普通 Scoop codec 组合表达，不以丢弃已知字段、整份输入存成 JsonValue 或改写 wire format 规避缺口。
4. JSON 的任意未知子树可以精确保留再写出；非法 JSON 即使处于未知字段也被拒绝。
5. 注解和合成扩展限于统一 Serialize 及约定的字段规则；不增加字段 codec 绑定、tag／flatten 等后续合成配置、反射／注册表、JSON intrinsic 或 runtime 特判。遇到真实 compiler／core 问题时在对应层修复并回归，不用库侧特殊路径规避。

API 数据的完整覆盖是离线 wire 能力验收；不以网络可用性、API key、模型权限、完整 SDK 或 TOML parser 为前置。
