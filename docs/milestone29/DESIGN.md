# M29 设计：静态类型描述与序列化基础设施

状态：实施中，2026-10-06 codec 协议修订已迁移至实现，正式总验收进行中；进度和实际验证见 [PROGRESS.md](PROGRESS.md)。此前实例编码和条件 conformance 的验收不代表本次修订已实现。

日期：2026-10-06。

依赖：M17 的参数/default 协议，M19 的正常构造与初始化，M20/M21 的泛型、property 和 interface 实现选择，M23 的共有 HIR/跨 Cone/`.slib`/ODR，以及 M25～M27 的异常、String/Char/List 和 Context。以当前完成 M27 的仓库为实现基线。本文件定义新的 M29，不沿用原 M29 的目标或实现计划。

2026-10-05 决策补充：companion 按完整宿主类型分别拥有 singleton，并可使用宿主类型参数。第 3.3 节取代 [M21 设计 §3.2](../milestone21/DESIGN.md) 的共享、非 generic companion 规则，也修订后续实现中不携带宿主实参的假设。此前 milestone 设计作为历史记录保留原文；后续修改由本设计及当前三份 spec 记录。泛型 companion 已实现并通过三平台验收；M29 其余能力的实现情况见实施记录。

2026-10-06 决策补充：`Encodable<T>` 与 `Decodable<T>` 均由 companion 或普通 codec 对象实现，encode 显式接收数据值。父子数据类型不经继承传递编码策略；泛型和核心容器改为两个方向的显式 codec 组合，撤销实例 Encodable、容器/tuple 条件 conformance。迁移范围见第 9.3 节；设计与规范先行，实施情况见 PROGRESS.md。

对应[路线图](../ROADMAP.md)、[语言规范](../specs/SCOOP-SPEC.md) 9.4～9.6、11.13、[实现规范](../specs/SCOOP-IMPL-SPEC.md) 2.17 和[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 2.2。语言行为以已同步的规范为准；本文解释实现方式、展开结果及完成门。

## 0. 目标与交付边界

M29 交付两个相连的能力：编译期能取得每个类型的静态结构描述；companion 或普通 codec 类型显式实现 `Encodable<T>` / `Decodable<T>` 后，编译器按目标 T 补齐各自缺失的方法。类型描述包含字段名称、类型及 annotation，合成器将这些事实展开为普通类型化代码。数据类型本身无需实现编码或解码接口。

目标使用方式：

```scoop
import scoop.json.Json

public struct User(
    @SerialName("user_id") val id: Long,
    val name: String = "anonymous"
) {
    public companion object : Encodable<User>, Decodable<User> {}
}

fun main() {
    val text = Json.encode(User(id = 7L), User.Companion)
    val user = Json.decode(text, User.Companion)
    println(user.name)
}
```

缺省 JSON 输出为 `{"user_id":7,"name":"anonymous"}`。源码、独立 `.slib` 消费、链接与运行都必须走通；只输出一份描述文件或只演示编译器内存中的字段列表不算完成。

| 范围 | M29 决策 |
| --- | --- |
| 静态描述 | 所有合法类型都有完整的编译期 shape；泛型定义保留 binder，具体化后取得 exact 字段类型 |
| 静态描述的消费入口 | 共有 HIR 查询、已有 HIR dump，以及本里程碑的合成器；通用源码查询/遍历语法后续单独设计 |
| 用户选择 | companion/普通 codec 声明 `Encodable<T>` / `Decodable<T>`；两个方向独立 |
| 缺省实现 | 按目标 T 的 shape，在 codec 上独立合成普通方法；合法用户/继承实现优先 |
| 数据继承 | 父子类 companion 独立；所选 codec 决定策略，不按数据的运行时类型改选 |
| 构造 | companion/解码器的普通 decode 返回显式目标类型，最终调用正常 constructor/variant constructor |
| 泛型 companion | 每个完整宿主类型拥有独立 companion 类型、状态与 exactly-once 初始化；可使用宿主类型参数 |
| 格式边界 | keyed、unkeyed、single-value 协议；类型侧代码不绑定 JSON |
| 可运行格式 | 普通 JSON 库，以 String 输入/输出完成闭环 |
| runtime | 使用现有对象、普通调用、异常与 GC；没有反射字段表或运行期构造器登记 |

本里程碑不增加通用 CTFE、`static for`、编译器插件、源码可保存的 TypeInfo、任意“map 转对象”原语或运行期 `SerialDescriptor`。Map、Float/Double、ByteBuffer/IO、循环对象图的身份协议和开放多态注册也不作为前置能力。static reflection 的这一版是编译器可消费、可导出、可检查的静态描述基础设施，不宣称已提供面向库作者的通用编译期元编程语言。

## 1. 从 Swift 借鉴什么

Swift `Mirror` 的入口接收运行期 `Any`，children 也包含运行期值，而且 `CustomReflectable` 可以改变观察结果。这种机制不符合 Scoop 的目标。

Swift `Codable` 的协议拆分、缺失方法合成及格式容器机制可以采用：用户选择一个方向或全部手写，编译器在满足条件时补齐其余方法。Scoop 将 conformance 放在 companion/普通 codec 上，数据实例的继承不决定 codec；生成代码也不需要借助 `Mirror` 遍历字段。

Scoop 与 Swift 的具体差异是：

- `decode` 使用返回值工厂，不采用 initializer requirement。struct、enum、class 因而能使用同一个契约，enum 不需要伪造普通 constructor。
- `Encodable<T>` / `Decodable<T>` 使用普通显式目标参数，companion 可同时实现两个方向；encode 显式接收 User，decode 返回 User。companion 的泛型作用域及 singleton 归属见第 3.3 节，不增加 Self、static 成员或专用工厂 requirement。
- interface 方法继续不支持方法级泛型。字段类型留在生成代码中，由 `codec.encode(value, child)` 和 `codec.decode(child)` 递归；codec 分别满足完整的 `Encodable<F>` / `Decodable<F>`。
- `optional(key)` 只表示输入 key 缺失；显式 null、缺省参数和 Option 的分支分别处理。既不照搬 Swift 的 `decodeIfPresent`，也不复制其带初值 `let` 的合成规则。
- 源码字段与序列化形状分开。注解可以改 wire 名或排除字段，但不能修改原类型描述；同一目标类型的手写 codec 可以选择另一种数据形状。

因此，static reflection 负责“这个类型声明了什么”，正常构造规则负责“怎样产生合法的值”，合成器负责把二者连接为 `encode` / `decode` 的普通程序。无需先发明一个公开的反射构造函数。

## 2. 静态描述与 annotation

### 2.1 复用共有 HIR 的类型事实

当前已有 nominal field、enum variant、logical property、constructor/default 和跨 Cone source shape，缺少完整的用户 annotation 与统一的静态描述视图。M29 在这些事实之上补齐查询和数据，而不复制一套描述器类型系统。

| 对象 | 必须保留的事实 |
| --- | --- |
| 类型 | 类型类别、typed 类型引用、名义声明身份及 annotation；显示名称不是 identity |
| struct 字段 | 原字段身份、源码名称、完整类型表达式、有序 annotation、构造参数/default 关联 |
| enum | variant 身份、名称、annotation、payload 形式，以及每个 variant 自己的字段序列 |
| class | 直接 base、当前 owner 的存储字段、logical property 与构造参数关系；访问域和 accessor/存储类别不丢失 |
| interface | 父接口和 logical property 描述；没有虚构的实例字段 |
| tuple / Unit | 类型化的有序元素，位置名 `_1`、`_2` 等；Unit 为零个元素 |
| intrinsic 类型 | 实际表示种类；String、Array、Ptr 等没有普通字段不代表它们是空 record |
| 函数、指针、类型参数 | 原签名、pointee 或 binder/bound；没有伪造的字段 |

enum 的字段必须留在 variant 下：两个分支都叫 `value` 是合法的，不能合并成一个扁平字段列表。class 的基类字段通过 base 关系取得，保留其原 owner；隐藏的 delegate slot 也不能与用户 property 混成两份业务字段。computed property 有自身描述，但不因存在 getter 就成为存储字段。

泛型定义保存例如 `Box<T>.value: T` 的符号化类型，`Box<String>` 则取得 `value: String`。递归引用是图中的 typed reference，不递归展开成无限树。透明 alias 不创建新的 type identity；名义声明、类型表达式、exact application 和单态化 callable 继续使用各自的 id 类型。

描述不包含机器 offset、内存 tag/niche、读写字段的运行期函数指针或可执行 default 指针。需要字段访问和构造时，合成器仍通过对应 typed declaration 生成正常 HIR。普通可见性在原语义边界检查，不因字段出现在 `.slib` 中就变成 public。

property 描述保留 accessor 是否为普通存储生成的正文。跨 Cone 的 `StorageBody` 与无正文的 `Storage` 都表示普通存储访问，`Body` 表示自定义正文；前者仍保留原 callable 身份并沿普通调用路径消费。自动派生据此检查 custom getter/setter，不根据有无 callable 或分析正文推断，格式见实现规范 2.17。

HIR dump 必须能展示字段名称/类型/annotation、variant 层级及构造关系，供实现验收和用户检查。具体 dump 排版随已有 golden 机制确定，不在这里另定一种独立反射文件格式。

### 2.2 自定义 annotation 的最小表面

```scoop
public annotation class Description(val text: String)
public annotation class Version(val number: Int = 1)

@Description("Account data")
public struct Account(
    @Description("Stable identifier") val id: Long,
    @Version(number = 2) val name: String
)
```

annotation class 是只有常量参数的编译期声明。M29 接受 Boolean、String、Char、现有定宽整数，以及这些类型的字面量和已绑定 `const val`；参数 default 也必须是同一常量子集。无继承、泛型、方法、可执行初始化或 runtime annotation 实例。annotation 名称、参数名、类型、范围及访问规则在普通前端边界检查。

可标注名义类型、variant、struct/variant 字段及 class/interface logical property。primary parameter 上的注解只有在参数本身声明了 `val`/`var` 字段/property 时才落到该成员；普通构造参数不是字段。位置 payload 的字段也保留自定义 annotation，但序列化专用注解对它们有第 2.3 节的限制。

同一 target 的同一 annotation 不可重复，不提供 use-site target 或元注解执行。应用保存实际 annotation 声明引用及按参数声明序补齐的 typed 常量；不同 annotation 保留源码顺序。注解不继承、不向 accessor/backing storage 复制。核心已有 FFI 等注解保留各自的目标、共存和语义规则。

公开 annotation 声明的参数签名遵循普通可见性规则。随类型、模板或合成 body 必需的非 public 实现信息沿原 support 闭包保留，不增加源码名称可达性。消费方不能只凭 `@SerialName` 这几个字符认定它是核心注解。

### 2.3 默认序列化使用的两个注解

```scoop
public annotation class SerialName(val name: String)
public annotation class Transient
```

| 注解 | 作用与限制 |
| --- | --- |
| SerialName | 为 record 字段或 enum variant 选择 wire 名；不修改原字段/variant identity；不适用于 type、computed property、tuple 或位置 payload |
| Transient | 从默认编码/解码的存储字段集合中排除；不适用于 type、variant 或位置元素；派生 decode 时须有 default 或正常 class 初始化来源 |

两者不能标在同一 target。任一派生 record 内实际参与的字段名、enum 的 wire variant 名必须唯一。未识别的普通用户 annotation 仍保留在静态描述中，但不会被当作 serializer 扩展点执行。

不增加 `@Serializable` 开关：interface conformance 已经表达请求。也不在本里程碑加入 flatten、自动别名回退、运行期 discriminator 策略或任意 callback annotation。

## 3. 协议、companion 与实现选择

### 3.1 两个方向、统一的 codec 接收者

```scoop
public interface Encodable<T> {
    public fun encode(value: T, encoder: Encoder): Unit
}

public interface Decodable<T> {
    public fun decode(decoder: Decoder): T
}
```

两个方法的 this 都是 companion/普通 codec 对象。encode 的 value 是数据，decode 的结果是目标值；T 是普通类型参数，具体实现直接写 User、`Box<E>` 等完整类型。二者都是普通实例 interface。

```scoop
public struct User(val id: Long) {
    public companion object : Encodable<User>, Decodable<User> {}
}
```

上述声明在 User.Companion 上请求合成两个方法，均以 User 为描述目标。User 实例不因此实现任一接口，也不需要先有 User 才能解码。两个方向分别 opt-in；已有 companion 保留自己的名称、状态和成员，只补齐缺失方法，不自动创建 companion 或把 conformance 转移到数据类型。

| 声明与现有实现 | 编译器行为 |
| --- | --- |
| 未实现对应 interface | 不生成该方向的方法 |
| 已有合法用户/继承/default 实现 | 使用原实现 |
| `Encodable<R>` 实现者缺少 encode，R 的形状/依赖/读取访问合法 | 在该 codec 上合成接收 R 的普通方法 |
| `Decodable<R>` 实现者缺少 decode，R 的形状/依赖/构造完备 | 在该 companion/解码器上合成返回 R 的普通方法 |
| 能力缺失、依赖歧义或不可构造 | 定位目标字段/variant/参数并报错 |
| 显式 override 非法 | 正常报错，不删除后改用派生体 |

subinterface 可以继承核心接口，编译器只补核心 requirement，其他义务仍需实现。无关合法 overload 不屏蔽要求的签名。手写一个方向后，另一个仍按声明形状派生，编译器不推导任意用户程序的逆函数。

同一 codec 可以同时实现两个方向，普通组合 interface 也可继承这两个完整 application。M29 不增加内建 Codable 标记，两个 requirement 仍分别选择和合成。

internal/private 数据类型同样可以拥有 companion codec。手写与合成方法的签名按普通成员的 direct lookup domain 检查；public override 对接口槽的覆盖不把受 owner 限制的实现签名变为 public API。经接口调用使用该静态接口的签名及完整实参，公开声明仍不得泄漏不可见类型，见语言规范 9.1.5。

### 3.2 具体调用与泛型调用

```scoop
public struct Identifier(val value: Long) {
    public companion object : Encodable<Identifier>, Decodable<Identifier> {
        public override fun encode(value: Identifier, encoder: Encoder): Unit =
            Long.Companion.encode(value.value, encoder)

        public override fun decode(decoder: Decoder): Identifier =
            Identifier(Long.Companion.decode(decoder))
    }
}

public fun <T> writeValue(value: T, encoder: Encoder, codec: Encodable<T>): Unit =
    codec.encode(value, encoder)

public fun <T> readValue(decoder: Decoder, codec: Decodable<T>): T =
    codec.decode(decoder)
```

`Identifier.Companion.encode(value, sink)` 和 `Identifier.Companion.decode(source)` 都是普通 singleton 成员调用。无冲突时可写 `Identifier.encode(value, sink)` / `Identifier.decode(source)`；转发仍具有真实 receiver，沿原 exactly-once 路径初始化。

泛型入口显式接收相应 codec，例如 `Json.encode(value, Identifier.Companion)` / `Json.decode(text, Identifier.Companion)`。T 由普通实参推断，也可显式写出；数据 T 无需编码 bound。不增加 `T.encode`/`T.decode`/`T.Companion` 泛型查找、隐式 codec 或只有 value 的 Json.encode 重载。

两个接口都保持不变性，`Encodable<Base>` 不能赋给 `Encodable<Derived>`，解码方向同理。显式选择 Base codec 时，数据实参仍可按普通规则从 Derived 向上转换为 Base，表示采用 Base 策略。不同 application 的方法按普通 override/重载规则检查；decode 参数相同而结果不兼容时诊断冲突，不根据期望结果类型猜测调用。

### 3.3 companion、继承与泛型宿主

class 继承不继承 companion。Base.Companion 的 `Encodable<Base>` / `Decodable<Base>` 不会转移到 Derived.Companion，也不向数据实例增加 requirement。运行时数据类型不改变所选 codec；自动派生的字段按声明类型选择，不向基类 companion 回退。因此子类没有自己的 codec 时，不能借父类的实现悄悄省略子类状态。普通 codec class 自身继承的方法按原规则使用，参数及结果类型不自动改写。

**每个完整宿主类型拥有自己的 companion 类型与 singleton。** `Box<Int>.Companion` 和 `Box<String>.Companion` 是不同类型、不同对象；同一 `Box<Int>.Companion` 无论被访问多少次、经哪个别名或 Cone 访问，都代表同一个对象。companion 中的可变状态按完整宿主实参区分，即使没有用到 T、没有字段或布局完全相同，也不合并不同 application。

companion 可以使用直接宿主的类型参数及其 bound；这些参数进入字段类型、base/interface、方法签名、default 和初始化表达式中的类型位置。例如：

```scoop
public struct Box<T>(val value: T) {
    public companion object {
        public var cached: Option<T> = None

        public fun create(value: T): Box<T> = Box<T>(value)

        public fun <U> pair(first: T, second: U): (T, U) = (first, second)
    }
}

val integers = Box<Int>.Companion
val sameIntegers = Box<Int>.Companion
integers.cached = Some(1)
val strings = Box<String>.Companion
```

integers 与 sameIntegers 引用同一对象；strings.cached 仍为 None。T 来自宿主，pair 的 U 来自方法自身，两组实参共同决定方法具体化，不能重名。companion 自身不声明另一套类型参数，因此不写 `Companion<T>`。this 是 companion，不捕获某个 Box 实例、其 value 或其他宿主构造参数。

限定访问的规则如下：

- generic companion 的类型、值和成员都要求完整宿主实参：`Box<Int>.Companion`、`Box<T>.Companion.create(value)`，或无冲突时的 `Box<Int>.create(1)`。这里 T 可以是当前作用域的类型参数，随后单态化。`Box.Companion`、`Box.create(1)` 或宿主实参中的 `_` 不能形成完整 companion 引用；不从方法参数或期望结果反推宿主。
- 命名 companion 的 `Box<Int>.Factory` 与 `Box<Int>.Companion` 是同一个 application；透明别名 `typealias IntBox = Box<Int>` 的 IntBox.Companion 也指向它。import 只定位声明，不能补出缺失的宿主实参。没有新增带类型实参的 import 语法。
- 普通 static nested class/struct/enum/interface/object 继续使用独立作用域，含 companion body 中的普通嵌套声明。`Box.Nested` 不继承 T，也不随 Box 的实参复制；裸 generic 宿主仍可作这类声明的命名空间限定符。companion 仅继承其直接宿主已有的类型参数环境，普通静态嵌套声明仍是作用域边界。

每个实际 companion application 在首次非 const 访问时独立、线程安全地初始化一次，分别拥有 cell、published root、failure root 及副作用。某个 application 初始化失败不会改变其他 application 的状态；显式代码依赖仍走原循环检测与失败传播。同一 application 跨 Cone 物化时，其类型与初始化支持按现有 ODR 合并，不能变成每个 consumer 各有一份状态。

只构造 `Box<Int>` 不触发 companion 初始化，访问 companion 也不构造 Box 实例。类型引用、静态描述和 const 读取不执行初始化；const 访问仍须指定完整宿主实参。未具体化的 companion 模板没有运行期对象或 cell。

companion 可直接声明 `Encodable<Box<T>>` / `Decodable<Box<T>>`，encode 的数据参数及 decode 的返回类型都可写为 `Box<T>`。方法能否自动合成仍取决于第 6 节的字段 codec；按宿主具体化不会自动赋予裸 T 编码或解码能力。

object 可正常实现任一方向。对于 singleton、接口或基类目标，用户手写字段选择、已有对象或正常 constructor 的逻辑；自动派生仍按第 5 节限制，不增加运行期类型注册或子类发现。

## 4. Encoder / Decoder 与格式的分工

完整 interface 签名见语言规范 11.13.2。它们提供以下操作：

| 层级 | 编码 | 解码 |
| --- | --- | --- |
| 每个值 | `keyed()` / `unkeyed()` / `singleValue()` | 同样三个入口 |
| keyed | `field(name): Encoder`、`end()` | `keys`、`required(name)`、`optional(name)`、`end()` |
| unkeyed | `element(): Encoder`、`end()` | `hasNext`、`element(): Decoder`、`end()` |
| single-value | Boolean、Long、ULong、String、null 的写入 | 对应的类型化读取 |
| 路径 | `Encoder.path` | `Decoder.path` |

每个入口代表一个数据值，选一种容器，逐个完成 child，再处理下一项。single-value 恰好消费一个标量；keyed/unkeyed 在本层 end，格式根入口再确认完整文档消费。没有 `encode<T>` 这样的 interface 方法，也不把字段值擦成 Any 后再找其类型。

生成代码知道字段的静态类型和已选 codec，例如：

```scoop
val fieldValue = value.value
val target = fields.field("value")
elementEncoder.encode(fieldValue, target)
```

反方向同样使用定义处确定的字段 codec：

```scoop
val source = fields.required("value")
val value = elementDecoder.decode(source)
```

格式实现可以缓冲数据，以满足 keyed 按名称读取而不依赖输入顺序的契约。编译器不要求每种格式创建同一种中间树；某个 JSON 实现在库内部使用自己的 typed JsonValue 树是正常格式解析，不是对象反射。

`optional` 的 None 只代表 key 不存在。如果输入中 key 对应 JSON null，它返回 Some(child)，随后由 child 上的具体 codec 判断是否合法。`keys` 描述的是输入对象的 key，可以用于选择 enum 分支，不是类型元数据查询。

数据路径采用 JSON Pointer segment 形式：根为空串，数组第 3 项的 `name` 为 `/3/name`，key 内 `~`、`/` 分别转义为 `~0`、`~1`。这是格式无关的普通 String 路径。核心提供 `EncodingException(path, message)` 和 `DecodingException(path, message)`，沿既有 Exception 保存消息，供库 codec 及生成分支使用。

## 5. 编译器合成的方法

以下代码表示可观察的展开结果；临时变量名称与具体 HIR 排版不属于语言合同。所有示例沿普通字段、调用、when、异常和构造语义实现。

### 5.1 companion 中针对 struct 的 encode

第 0 节 User.Companion 的合成方法等价于：

```scoop
public override fun encode(value: User, encoder: Encoder): Unit {
    val fields = encoder.keyed()
    val id = value.id
    val idTarget = fields.field("user_id")
    Long.Companion.encode(id, idTarget)
    val name = value.name
    val nameTarget = fields.field("name")
    String.Companion.encode(name, nameTarget)
    fields.end()
}
```

this 是 User.Companion，字段从数据实参 value 读取。两个字段引用与 codec 在编译期已确定；按声明序读取数据、取得 child，再求值 codec 并调用，各执行一次。运行期不枚举字段、查询 annotation 或按字符串寻找 getter，companion 自己的状态不进入 User 的 wire 形状。

总是编码参与字段，包括值恰好等于 default 的字段。没有求 default 再执行 equals 的隐含操作；default 的副作用不因 encode 被触发。

### 5.2 companion 中的 decode 与 default

第 0 节 User.Companion 中合成的普通成员等价于下列代码，这里只展开 decode：

```scoop
public companion object : Decodable<User> {
    public override fun decode(decoder: Decoder): User {
        val fields = decoder.keyed()
        val idSource = fields.required("user_id")
        val id = Long.Companion.decode(idSource)

        val nameSource = fields.optional("name")
        val suppliedName: Option<String> = when (nameSource) {
            Some(source) -> Some(String.Companion.decode(source))
            None -> None
        }

        fields.end()

        val name = when (suppliedName) {
            Some(value) -> value
            None -> "anonymous"
        }
        return User(id = id, name = name)
    }
}
```

this 是 User.Companion；被描述和构造的类型是 `Decodable<User>` 中的显式参数 User，不能把 companion 自己的字段误作数据字段。Long.Companion/String.Companion 是普通核心解码器。

对有 default 的字段，外层 Option 保存输入是否提供了一个已解码值；字段本身也是 Option 时保持 `Option<Option<T>>`，不能以 None 字段值误判缺失。

一般算法固定为：

1. 按 constructor 参数顺序取得 child。没有 default 的字段使用 required，有 default 的使用 optional；缺失的可选字段不求值其 codec 表达式。
2. 对存在的字段求值已选 codec 并调用 decode，继续后续字段，最后完成容器检查。缺 required、类型错误、非法 null、越界或格式错误直接失败。
3. 按参数声明序补缺失项和 Transient 参数的 default。表达式仍在原定义处绑定，可引用前面的参数，只执行需要的项且至多一次。
4. 用完整实参调用选定的正常 constructor，复用原参数协议和默认表达式替换机制。

例如 end 的 default 为 `start + 10L` 时使用最终 start。较早缺失参数的 default 不能提前到后续已提供字段的 decode 之前；异常正常传播，不回滚已经发生的副作用。

生成 constructor 调用的来源锚定请求合成的 companion/解码器声明，default 保留定义处的名称与重载绑定。getCurrentSourceLocation 沿已有规则观察该调用来源，不为 Json.decode 增加运行期调用者位置参数。

### 5.3 enum 与 Option

```scoop
public enum Message {
    @SerialName("idle") Idle,
    @SerialName("text") Text(val value: String),
    @SerialName("point") Point(Long, Long)

    public companion object : Encodable<Message>, Decodable<Message> {}
}
```

| 值 | 缺省 JSON |
| --- | --- |
| `Message.Idle` | `{"idle":{}}` |
| `Message.Text("hello")` | `{"text":{"value":"hello"}}` |
| `Message.Point(3L, 4L)` | `{"point":[3,4]}` |

encode 在 codec 中对数据实参的 variant 作普通 when：写一个外层 key，然后用已选字段 codec 编码该 variant 的 record 或 sequence payload。decode 先检查外层恰好一个 key，再按编译期已知的 wire 名分支；读取并检查 payload、结束内外层容器，再按参数序补 default，最后直接调用 `Message.Text(...)` 或 `Message.Point(...)`。未知 variant 抛包含 decoder.path 的 DecodingException。

变体的 ordinal、内存 tag、niche 与 payload offset 不进入格式。源码顺序只影响编码器生成的分支顺序，不改变选中分支的 wire 名。位置 payload 和 tuple 必须恰好消费声明数量的元素；命名 payload 沿 record 的 default/未知 key 规则处理。

Option 也使用这一数据模型。当前 core 的 `Some(T)` 是位置 payload，因此：

| `Option<Option<Long>>` 的值 | 缺省 JSON |
| --- | --- |
| `None` | `{"None":{}}` |
| `Some(None)` | `{"Some":[{"None":{}}]}` |
| `Some(Some(7L))` | `{"Some":[{"Some":[7]}]}` |

不把这些值合并成同一个 null。record 中没有 default 的 Option 字段仍然 required；用户需要“缺 key 等于 None”时显式声明 `= None`。Unit 的单值 null 与 Option 的分支表示也不同。

### 5.4 class 通过正常构造建立对象

M29 的自动 class 范围仍是普通 final class、没有显式 class 基类；decode 还要求 primary constructor 与参与存储字段有明确的一一映射。协议移到 codec 不扩大自动派生范围；开放类或带基类的目标使用手写 codec。

```scoop
public class PositiveId(public val value: Long) {
    public companion object : Decodable<PositiveId> {}

    init {
        if (value <= 0L) {
            throw IllegalArgumentException(Some("value must be positive"))
        }
    }
}
```

生成工厂读取 value 并结束容器后执行 `PositiveId(value)`，因此上述校验仍执行，失败仍抛原异常。所有普通 init、分配、字段初始化、write barrier 和 release-ready 发布沿 M19/M24 路径发生。

encode 从数据实参读取目标 owner 合格的存储 property，访问权限按 codec 的正常词法位置检查。decode 要求参与状态都来自 primary 的 val/var 参数；额外 body 存储属性须明确 Transient 且能正常初始化，未参与的构造参数须有 default。带自定义 accessor、委托或无法映射的状态须显式处理；computed/abstract property不进入默认存储集合。多个 secondary constructor 不用于猜测“最适合解码”的构造路径。

仅请求 encode 时不检查 decode 的构造映射。请求 decode 且不满足条件时，用户可以直接手写返回该类型的 factory，选择正常 secondary constructor、校验或自定义转换。编译器不会开放未初始化对象、反射 setter 或 unsafe 填字段作为补救。

## 6. 泛型、核心类型与 codec 组合

### 6.1 两个方向都使用显式依赖

数据类型的 T 无需实现编码或解码接口。对于包含 value: T 的 Box，companion 虽可使用宿主 T，仍需在定义处提供字段 codec。其普通方法接收依赖，并传给普通 helper：

```scoop
public struct Box<T>(val value: T) {
    public companion object {
        public fun encoder(element: Encodable<T>): Encodable<Box<T>> =
            BoxEncoder(element)

        public fun decoder(element: Decodable<T>): Decodable<Box<T>> =
            BoxDecoder(element)
    }
}

public class BoxEncoder<E>(private val element: Encodable<E>) : Encodable<Box<E>>
public class BoxDecoder<E>(private val element: Decodable<E>) : Decodable<Box<E>>
```

两个 helper 缺失的方法分别合成为：

```scoop
public override fun encode(value: Box<E>, encoder: Encoder): Unit {
    val fields = encoder.keyed()
    val fieldValue = value.value
    val target = fields.field("value")
    this.element.encode(fieldValue, target)
    fields.end()
}

public override fun decode(decoder: Decoder): Box<E> {
    val fields = decoder.keyed()
    val source = fields.required("value")
    val value = this.element.decode(source)
    fields.end()
    return Box<E>(value)
}
```

调用为 `Json.encode(box, Box<Long>.Companion.encoder(Long.Companion))` 和 `Json.decode(text, Box<Long>.Companion.decoder(Long.Companion))`，也可使用普通 companion 转发。encoder/decoder 方法直接使用宿主 T，没有自己的类型参数；helper 的 E 由构造实参推断。元素 codec 保存在本次返回的对象中，不写入 companion 字段；同一个 `Box<Long>` 可以使用多个编码或解码策略。

若 generic 目标的全部参与字段已有合法 codec，companion 可直接实现相应完整接口并请求合成。例如 `Envelope<T>` 只有 Long 字段时，两个方向都可用 Long.Companion。这里的 value: T 则不同：单独声明 `Encodable<Box<T>>` 或 `Decodable<Box<T>>` 仍缺少字段能力，须在定义处报错；不等 T 替换为 Long 后再发现 Long.Companion。

编码能力不再通过数据的 `T : Encodable` 约束表达。普通用户为其他语义声明的泛型约束保持原义，编译器不因请求派生增删它们。两个方向分别选择 codec，不要求它们来自同一对象。

### 6.2 合成时确定字段 codec

合成 `Encodable<R>`.encode / `Decodable<R>`.decode 时都读取 R 的 shape，不把实现者的 element 等依赖当作数据字段。对实际参与的字段 F，两个方向分别在定义处依次选择：

1. codec class/struct 的 primary constructor 以 val 保存、且静态类型满足所需 `Encodable<F>` 或 `Decodable<F>` 的显式依赖。一个匹配时使用它；多个匹配报歧义，不按参数名/顺序猜测。
2. F 与 R 相同则使用 this，支持当前方向的递归调用；不先执行 body 来判断能力。
3. F 的可见普通 companion 若实现所需的相同 application，使用对应完整宿主 singleton，例如 `Envelope<E>.Companion`。不向 F 的基类 companion 回退，仅有同名方法也不算 conformance。
4. 核心 Option/Array/MutableArray/ArrayList 递归组合相应元素 codec；Unit 使用 UnitEncoder/UnitDecoder；tuple 按已知元素组合。其他缺失情况要求显式注入或手写。

依赖只来自明确存储的构造参数，不扫描任意 getter、companion property、Context 或整个作用域。用户 generic 类型的 encoder/decoder helper 由调用方显式调用，不按函数名发现工厂协议。相同字段类型可共享依赖；需要逐字段不同策略时手写相应方法。

选择保存实际声明引用。两个不同 binder 在具体化后恰好相同，仍使用定义处各自选中的参数，不重新选择或制造歧义。encode 按第 5.1 节读取数据、取得 child 后求值 codec；decode 按第 5.2 节先检查 child 是否存在。Transient 字段及未选中 variant 不求值字段 codec；组合在普通方法执行中进行，不预先建立递归 singleton 初始化链。

合法递归类型先登记合成签名再生成 body；裸 T 没有可展开形状，不能仅靠任一 codec 接口声明得到自动实现。非法内联值布局与参数变化的递归单态化继续由原规则诊断，不使用深度/实例数量阈值。

### 6.3 核心类型的默认实现

| 类型 | 编码 | 解码 |
| --- | --- | --- |
| Boolean、定宽整数、String、Char | 各自 companion 实现 `Encodable<Scalar>` | 同一 companion 实现 `Decodable<Scalar>` |
| Unit | 普通 object UnitEncoder | 普通 object UnitDecoder，不伪造 companion |
| `Option<T>` | `Option<T>.Companion.encoder(element)` | `Option<T>.Companion.decoder(element)`；两个 helper 均保留 enum 分支 |
| `Array<T>` / `MutableArray<T>` / `ArrayList<T>` | 各自完整宿主的 companion.encoder(element) | 各自完整宿主的 companion.decoder(element) |
| 非空 tuple | 显式 `Encodable<Tuple>` 实现或按元素组合 | 显式 `Decodable<Tuple>` 实现或按元素组合 |
| `List<T>` / `MutableList<T>` | 不自动提供，可调用普通 encodeList | 用户显式选择结果实现及 codec |

encoder 方法接收 `Encodable<T>`，decoder 方法接收 `Decodable<T>`，都返回持有依赖的普通对象。核心容器原有参数无编码 bound，未提供 codec 的元素类型仍可正常构造、索引和迭代；标量、Unit、容器和 tuple 数据值不因库提供 codec 而获得接口，不再有条件 conformance、条件成员或编码专用装箱分派。

ArrayList 只编码逻辑元素，capacity/backing/空闲槽不进入 wire。数组解码可用普通 ArrayList 收集元素，再调用既有快照构造获得正确长度和完整初始化。

tuple 没有名义 companion。用户可声明完整 tuple codec 的 object/class/struct 请求合成；自动组合的字段可分别生成普通闭包，经核心 `EncodeFunction<T>` / `DecodeFunction<T>` 包装。前者持有 `(T, Encoder) -> Unit`，后者持有 `(Decoder) -> T`，方法只调用函数值，完整签名见语言规范 11.13.4。复用现有 closure conversion、接口分派与 GC，不在 tuple 本身增加接口或按 arity 命名的类型。

List/MutableList 可由手写 codec 调用普通 `encodeList(values, element, encoder)`，其中 element 是显式 `Encodable<T>`。Any、函数和 Ptr/FunPtr 没有默认 codec；用户可手写相应完整接口实现。两个方向都对所选 codec 作普通调用或接口分派，不扫描数据的运行期类型。

## 7. JSON 闭环与错误语义

协议、标量/容器 helper 及异常声明属于现有 core；JSON 放在普通库 Cone `scoop.json`、package `scoop.json`，依赖 core。它不参与 core 协议自举，也不让编译器认识 Json 类名。

公开入口由普通 object 的非 virtual generic 方法提供：

```scoop
public object Json {
    public fun <T> encode(value: T, codec: Encodable<T>): String
    public fun <T> decode(text: String, codec: Decodable<T>): T
}
```

这里展示的是 API 签名；实现必须提供完整 body。encode 建立根 encoder、调用 `codec.encode(value, encoder)` 并完成输出；decode 建立根 decoder、调用 `codec.decode(decoder)` 并检查完整消费后返回结果。两个入口都要求显式 codec，核心普通类型与用户类型使用同一入口，没有 compiler JSON builtin。

首版 JSON 可使用普通 typed 数据树和 StringBuilder。对象成员可用有序的 Array/List 保存，无需引入 Map；数字 token 可保留原始文本后按请求类型转换，无需先有 Float/Double。允许这种实现选择不意味着协议强制所有格式先构建一份数据树。

| 情形 | 行为 |
| --- | --- |
| 输出字段顺序 | 默认 record 按声明序；sequence 按索引；不排序或依赖内存布局 |
| 输入字段顺序不同 | 按 key 读取，得到相同的构造实参 |
| 未知 record 字段 | 跳过其值，但仍要求合法 JSON；enum 外层仍必须只有一个 key |
| 重复 key | 拒绝；名称按解码后的 String 比较，不能 first/last wins |
| 缺少 required 字段 | DecodingException；Option 类型本身不表示 key 可省略 |
| 有 default 的字段缺失 | 读取完已提供字段后按声明序执行 default |
| 显式 null | 交给字段 codec；不等于缺失，不触发 default |
| tuple/位置 payload 长度不符 | 拒绝缺少或多余元素 |
| enum 未知 variant / 多个外层 key | DecodingException |
| 非法整数或越界 | 拒绝；Long/ULong 不经过 Double，窄整数另检查其范围 |
| Char | 字符串必须恰好包含一个 Unicode scalar |
| 尾部垃圾/第二个根值 | 拒绝；根值之后只允许 JSON 空白 |
| 用户 codec/default/constructor 抛异常 | 保留其普通异常，不转成 None、不再次尝试其他 constructor |

解析器接受完整 JSON 数字语法以便正确处理/跳过任意未知字段；整数 codec 只接受没有 fraction/exponent 的整数 token，例如 Long 的 `1e0` 仍不接受。String 必须处理必要转义、Unicode scalar 和合法 surrogate pair，拒绝孤立 surrogate 与非法控制字符。具体错误包含相应数据路径；格式层可附加输入字符位置，但不依赖 runtime reflection。

默认编码描述树形值，不保存对象 identity。两个字段指向同一对象可以产生两份编码，解码产生独立构造结果。循环对象图、跨对象引用及开放多态须有显式 codec/数据协议，M29 的默认派生不承诺处理这些输入。不为这部分另建全局对象注册、图恢复或通用资源预算。

## 8. 编译流水线与跨 Cone

### 8.1 复用普通声明、接口和调用

| 阶段 | M29 职责 |
| --- | --- |
| AST / parser | annotation 声明及使用目标；保留 `Box<T>.Companion` 与成员限定路径上的宿主实参，companion 声明仍用既有语法 |
| HIR 声明阶段 | typed annotation、完整 shape、companion 的宿主 binder/普通 interface 关系、实现选择和合成签名 |
| HIR body 阶段 | 读取接口目标 T 的 shape；确定两个方向的字段 codec 及访问域；展开普通字段读取、调用、default 和 constructor |
| Export HIR / 具体化 | 保存已选声明和完整 body/template；按宿主 application 替换 companion 成员与初始化模板，不重新寻找 codec |
| MIR | 既有接口/直接调用、每个 companion application 的 ensure/singleton read、控制流、构造、closure conversion 和异常 |
| LIR / codegen | 既有 receiver ABI、itable、布局、root、this 调整和 String 常量 |
| `.slib` reader / linker | 读入静态事实及普通 body；沿原 provider、Strong/ODR 和链接关系消费 |

两个方法从声明阶段起都有普通 codec receiver：encode 另有显式 R 数据参数，decode 返回明确 R。具体 companion 调用可走 direct path，经相应接口值调用正常使用 itable，不能绕过真实分派。不增加类型级 factory declaration、Self TypeId、Implementor 结果角色或专用 bound call。

核心接口在全部 core 声明登记后可用，不依赖 JSON 库或文件顺序。先完成普通用户/继承/default 选择，再登记缺失核心方法的合成签名，随后生成 body；不能因尝试生成失败而删掉 conformance、改变重载结果。

两个 body 都归实际 companion/codec 类型，保留目标 R、字段、依赖及 decode 所需 constructor/default 的 typed 引用。目标数据、codec receiver 及其依赖不能混淆；字段读取与构造都须在实现者正常访问域内合法，外部 helper 不因合成获得额外权限。

临时待办只在 HIR stage 内部存在。输出包含完整普通 body、类型及绑定，不留下待 MIR 解释的 Serialize/Deserialize 节点；新增元数据服务 annotation/shape 和普通合成声明，不另造工厂签名体系。

### 8.2 `.slib`、泛型与 ODR

外来类型提供相同的 shape、annotation、构造/default 及普通方法关系。companion 声明保留宿主 binder，引用保留完整宿主 application，沿原 typed owner、object type/value、forwarding 和 initialization unit 关系消费。两个方向的字段 codec 保存已选 companion application、参数 property 或普通 helper 引用，不保存让 consumer 重跑的名称搜索计划。

companion 声明、application 和 concrete 实体使用各自 typed identity。完整 application 由真实 companion 声明与宿主 application 确定，包含不影响布局的实参；字段、base/interface、方法、default 和初始化模板在同一参数环境下替换。const 折叠及普通 static nested 查找保留原规则，不能据此提前实例化或初始化全部 companion。

provider 的 body 直接消费，generic provider 的 body 普通单态化。consumer 不从字段名重新合成，也不在自己的作用域重解释 private/default。参数自由 companion 沿定义方 Strong；generic companion 的 cell、published/failure root、initializer/ensure 及 initialization registration 按完整 application 组成普通 ODR 初始化组，类型/itable 和成员代码沿原 type/callable ODR 关系关联。同一 application 经多个 consumer 或 image 使用时，所有引用必须合并到同一状态与登记记录；不同实参即使布局相同仍保持独立。不同 codec 声明即使处理同一个 R，也保持各自身份与状态。

annotation 参数、wire 名、字段/default/构造关系、显式 codec 绑定或 body 变化，进入相应接口/正文 fingerprint 并使消费缓存失效。身份沿原 typed declaration/application/callable 体系，不由首次 Json 调用点决定。

annotation、companion application、初始化物化及相关语义按实际变化升级编译产物 section、兼容版本与 cache fingerprint；旧产物拒绝并重建，不在设计阶段预占尚未落实的 wire 编号。reader 在真实边界检查格式、引用与合同，后续复用结果。没有反射 sidecar、工厂专用 wire family 或额外来源凭证/重复验证链。

### 8.3 普通对象生命周期与 GC

静态描述不发射为运行期字段枚举表，wire 名只是普通 String 常量。程序可包含普通 companion、携带元素 codec 的 helper 和闭包，它们不是目标类型的运行期反射描述。

companion 按完整宿主 application 独立使用既有 exactly-once gate，成员有真实 receiver，GC 扫描使用完成类型替换后的自身字段；同一 application 跨 Cone 使用同一 published/failure root。普通 codec 经正常 constructor 建立。显式 codec 实参遵守原调用求值顺序，自动组合的依赖在字段实际处理时取得，不增加隐式全局缓存或预建递归对象图。

encode 的 value 使用普通参数复制与 ABI，只有 codec 自身是值类型并适配到接口时才按正常规则装箱/调整 receiver。数据、codec receiver、依赖、闭包捕获、临时值及结果沿现有精确根/relocation/barrier 保活；class 解码目标经 M19 初始化与 M24 release-ready 发布，失败沿原异常/GC 路径清理。

这些是正常 Scoop 对象和方法，不改变 runtime 对象头、TypeDescriptor 结构、C ABI 或 metadata ABI。encode/decode 都使用普通接口槽；runtime 不根据数据的类型名或字段列表选择 codec。

## 9. 实施批次与完成门

各批次的实际进度记录在 [PROGRESS.md](PROGRESS.md)。每批先完成其授权范围的 spec/数据结构/实现，再格式化和 lint，随后运行相应真实 CLI、negative 与 stage golden；不以测试中的假实现替代源码、产物和正常运行。

| 批次 | 可验收结果 |
| --- | --- |
| M29-1：协议与 companion | companion 按宿主 application 具体化，成员可使用宿主参数，不同实参状态独立、同一实参跨 Cone 合并；普通 `Encodable<T>`/`Decodable<T>`、scalar/Unit codec 和显式 JSON 单值闭环；转发/分派、独立 `.slib` 运行及对应错误诊断 |
| M29-2：annotation 与静态 shape | 用户 annotation、字段/variant/property 归属、共有 HIR 查询及 dump；本地/外来/泛型 shape 一致，typed annotation 常量和实际依赖可导出、读入 |
| M29-3：record 派生 | struct 的自动 encode/decode、SerialName/Transient、合法手写优先、record JSON、缺 key/default 与初始化次序；跨 Cone User 示例闭环 |
| M29-4：variant 与序列 | enum、Option、tuple、Array/MutableArray/ArrayList，两个方向的显式 codec 组合、generic provider 与 tuple 闭包；JSON 序列及完整数字/Unicode/输入错误规则 |
| M29-5：class 与组合 | 合格 final class 派生、父子数据类型的 codec 独立、codec 自身的普通继承与初始化；组合 default/context/异常、递归依赖、跨 Cone 泛型/ODR 与 moving GC |
| M29-6：正式总验收 | 独立进程 provider/consumer 与 artifact-only link/run，公共 fixture runner 和全 workspace/fixture 验收；确认静态描述未成为运行期反射入口，记录实际结果 |

类型侧与格式侧在首批就有一个手写实现的运行闭环；之后每批加入真实支持的形状。一个类型不满足当前完整语言规则时给出明确编译诊断，不能输出带 TODO、空 body 或运行期“尚未支持”的派生实现。M29 只有第 0 节全部范围及下列组合通过后才可标记完成。

### 9.1 功能与错误覆盖

| 规则组 | 必需的真实用例 |
| --- | --- |
| 静态描述 | 每种类型类别；field/type/annotation；variant 内同名字段；class base/property/delegate 关系；tuple 位置；intrinsic 不能当空 record；递归图与泛型替换 |
| annotation | 默认/命名常量、alias/import、导出消费；重复注解/参数、缺参数、类型/范围错误、非法 target/常量、SerialName/Transient 冲突及 wire 名冲突 |
| companion | 完整宿主限定、命名 companion/别名/转发、宿主 T 与 bound、独立方法参数；不同实参的类型/状态独立、phantom/空 companion、每个 application 初始化与失败缓存；普通 static nested 保持独立；缺实参、`_`、参数重名、宿主实例捕获的错误 |
| codec 接口 | 两个方向的 companion/alias/显式 codec 调用及普通 itable 分派、不变性；encode 的 value 与 this 分离；错误 override、依赖缺失/歧义、外部 codec 的非法私有字段/构造访问；可写宿主 T 不等于自动获得 codec |
| 数据继承 | 手写 Base/Derived codec 独立，显式 Base 视图采用 Base 策略；Derived 缺少 codec 不向 Base companion 回退；codec 自身的继承/default 沿普通规则；自动 class 范围保持 |
| 实现选择 | 无 opt-in 不派生、两方向独立、subinterface、手写/继承/default 优先、错误 override 不回退、其他 requirement 不被补齐 |
| 构造与 default | 依赖前面参数的 default、带副作用的 default、输入乱序、缺 required、显式 null、Transient；constructor/init 抛出及完整发布 |
| 数据模型 | empty record、Unit、所有整数边界、Char/Unicode、enum 三种 payload、tuple 精确长度、多层 Option、数组内联值与引用元素 |
| 泛型 | 编码/解码均显式注入；数据不需编码 bound，未提供 codec 的元素容器仍可用；同一类型可选多个策略；不同 binder 具体化相同后不重选依赖；本地与跨 Cone 一致 |
| JSON 错误 | 语法、重复/未知 key、unknown variant、非法数值/转义/孤立 surrogate、路径转义、尾部垃圾、未消费根值 |
| 运行期组合 | 默认值及字段 codec 抛异常；codec 按实际字段/variant 求值，Context 在执行时解析；GC stress 下编码数据与 codec、解码临时值、List/Array 及 class constructor 中的 relocation |

每个声明或类型规则中的编译错误须有独立 negative fixture，断言位置与消息，而非仅检查非零退出。runtime 格式错误使用可捕获异常与路径断言。正常 round-trip 之外须检查指定 wire 形状、构造/default 副作用顺序；不能用“同一个错误的编码器与解码器恰好相互抵消”作为唯一正确性依据。

### 9.2 产物、stage 与最终验证

必须包含至少一组独立 provider 类型库、单独 JSON 库和 consumer 源码，以及 consumer 只拿 `.slib` 时的正常编译/链接/运行。泛型场景增加两个 consumer 物化同一 companion application，验证引用相等、状态共享、初始化只执行一次和类型/初始化支持的 ODR 合并；另一个宿主 application 则保持类型、状态与失败缓存独立。测试包括带 managed 字段的 companion、初始化中的闭包/default 与 moving GC。private/default/support 引用与 annotation 修改后的重建必须使用真实产物产生路径。

HIR golden 展示 resolved annotation、完整 shape、companion 的宿主 binder/application、普通 interface 及两个方向的 codec 绑定、已展开字段/构造调用；MIR/LIR golden 区分 codec receiver 与数据参数，展示普通调用/控制流/ABI及按 application 区分的初始化支持。产物只发射实际需要的 body、常量及普通类型/初始化记录，不再带容器/tuple 条件编码和 TupleEncoding callable，也不加字段枚举表或 codec registry；不把 runtime 现有 GC TypeDescriptor 的存在误报为反射。

完整验收包括仓库要求的格式化/lint、`cargo test --workspace`、公共 fixture runner 单测及 `python3 tests/run_fixtures.py --all`。实际结果和对应提交记录在 [PROGRESS.md](PROGRESS.md)；部分用例通过或快照更新不替代普通模式的正式总验收。

### 9.3 2026-10-06 协议迁移

2026-10-06 的文档修订先替换原实例编码设计，再按下列功能批次迁移实现并提交。实施记录中的旧提交和测试保留为历史，不能作为新协议的验收；当前实现与验证进度见 PROGRESS.md。

1. 将核心协议、scalar companion、UnitEncoder、Json.encode 及手写用例改为显式数据参数与 codec；两个方向均验证普通直接/接口调用。
2. 将派生 encode 的 owner 改为 codec，目标取 `Encodable<R>` 的 R；复用两个方向所需的字段依赖选择、普通访问检查与 body 构造，补齐泛型、递归及父子 codec 独立用例。
3. 提供容器 encoder helper 与 EncodeFunction，修改 encodeList；删除只服务旧方案的条件 conformance、条件成员、tuple 编码模板/生成键及编码分派记录，不保留平行兼容实现。普通 Unit 声明、值装箱、类型测试、shape support 和已有 GC 修复按其正常用途保留。
4. 按实际删除/修改的产物字段升级 section 兼容版本及缓存 fingerprint，旧产物重建，不在设计中预分配版本号。同步正负 fixture 与三个 stage golden，重新完成 macOS、Linux glibc/musl 正式验收后再标记 M29 完成。

## 10. 参考资料

- [Swift Mirror 实现](https://github.com/swiftlang/swift/blob/main/stdlib/public/core/Mirror.swift)：运行期 Any/children 模型，用来界定本方案不采用的机制。
- [Swift SE-0166：Swift Archival & Serialization](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0166-swift-archival-serialization.md)：协议拆分、容器抽象、CodingKeys 及编译器合成条件。
- [Swift Codable 合成实现](https://github.com/swiftlang/swift/blob/main/lib/Sema/DerivedConformance/DerivedConformanceCodable.cpp)：以具体字段解码及正常初始化语句构造派生 body。
- [Swift SE-0295：带 payload enum 的 Codable 合成](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0295-codable-synthesis-for-enums-with-associated-values.md)：以 variant 名及 payload 表达代数数据类型；Scoop 的位置 payload、Option 和 default 规则按本文定义。
- [Serde data model](https://serde.rs/data-model.html)：类型实现与格式实现之间使用明确数据形状的参考；不照搬 Rust 的 associated type 或 trait 机制。
