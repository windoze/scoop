# M38：只读值视图、TextWriter 与 debug printer

本文件从属于 [总体设计](DESIGN.md)，类型和成员描述见 [TYPE-INFO.md](TYPE-INFO.md)。本文件全部公共库类型放在 `scoop.core`。2026-10-14 修订明确与 M46 Serialize／Option 的边界；独立 inspector 和惰性观察模型保持。

## 1. 读取由普通方法提供

```scoop
public interface Inspectable<T> {
    public fun inspect(value: T): ValueView
}

public class InspectFunction<T> public constructor(
    private val body: (T) -> ValueView,
) : Inspectable<T> {
    public override fun inspect(value: T): ValueView = body(value)
}
```

`Inspectable<T>` 是普通 invariant generic interface，泛型参数属于接口，`inspect` 自身不是泛型虚方法。它表示“按这一实现观察 T”；T 实例不因此实现该接口。

调用者显式传入或调用 inspector，例如 `User.Companion.inspect(user)`。没有 `T.Companion` 泛型查找，没有从 `Any` 的动态类型自动寻找 inspector，没有全局 registry，也没有把调用者访问域传入方法的隐式参数。

TypeInfo 只提供结构描述，不自带 `inspect` 或通用字段读取函数。inspector 的实现使用 TypeInfo 为值视图补充类型、成员和注解信息，并以普通代码读取实例。

## 2. 访问控制完全复用现有规则

### 2.1 检查发生在普通成员读取的定义处

无论手写还是合成，`value.member` 使用所在方法的实际词法 owner、原 Cone/source file、成员访问域及 receiver 静态类型检查。private、internal、protected、public 与普通代码完全一致。

```scoop
public class Credentials public constructor(
    private val token: String,
) {
    public companion object : Inspectable<Credentials>
}
```

这里显式声明 companion 的 `Inspectable<Credentials>`，按第 3 节请求缺省实现。companion 处于宿主词法 owner 链中；合成方法读取 `value.token` 使用该普通访问关系。把相同实现放在类外的普通 object 中，并不会自动获得读取 private token 的权限。

若外部代码直接写 `value.token`，错误在这次成员读取处。若它请求缺省 inspector，错误定位到请求合成的声明并指出不可访问的成员；合成器不能静默跳过该字段、改成运行期错误或把 body 搬到 Credentials 内部。

原 Cone 的泛型 body 在下游具体化后仍使用定义处已经检查、绑定的成员。消费 Cone、最外层调用者、优化是否内联及 M37 的 `inline` 拼写都不改变可见性。

### 2.2 执行读取闭包是普通调用

inspector 可以返回捕获 value 的读取函数。只要函数 body 的读取在定义处合法，外部代码即可按函数的公开调用约定执行它；这与公开方法返回一个读取私有状态的 lambda 相同。

因此 `MemberView.read()` 的调用不重新执行成员可见性检查。它调用的是已经合法构造的普通函数，不是让编译器在这次调用处重新解释成员身份。公开 inspector 是否暴露内部状态是该类型的普通 API 选择。

不提供下列机制：

- 对运行期 FieldInfo 进行特殊的、按调用点变化的编译检查。
- 编译器自动把 core DebugPrinter 当成用户类型的 friend。
- `AccessContext`、caller scope、权限令牌或反射专用可访问性查询。
- 对 ordinary `for` 或 `if` 做静态字段展开，以隐藏不可访问分支。

注解和截断控制运行期是否执行读取函数，不改变函数 body 必须通过类型与访问检查这一事实。

## 3. 缺省 inspector 的合成

### 3.1 显式请求与形状

沿用语言规范 11.13 的 codec 模式：只有实现类型显式 adopt 实际 core `Inspectable<T>` 且缺少合法 inspect 实现时，才合成这一 requirement。用户实现、继承实现和 interface default 先按普通规则选择。编译器不自动给所有类型加 companion 或 conformance。

这是新增一个与 codec 同类的 body 合成入口，**不新增访问规则**。它输出能以普通源码表达的成员读取、调用、分支、闭包和对象构造；每次读取使用实际 inspector 声明的权限。

| 目标形状 | 缺省行为 |
| --- | --- |
| struct | 按声明序生成字段视图 |
| 普通 enum | 先匹配活动 variant，只为该 variant 生成 payload 视图；unit/named/positional 分开 |
| M37 整型 enum | 输出实际 enum/variant 身份与底层整数值，不从任意整数构造 enum |
| tuple | 按位置生成元素视图，没有虚构的名义字段 |
| 无显式 class 基类的普通 final class | 观察本 owner 的普通存储 property；包含 primary 与 body storage |
| computed/abstract property | 元数据仍存在，不自动求值，不作为存储成员加入缺省视图 |
| 带自定义 accessor 的存储 property、delegated property | 要求手写 inspector；不绕过 accessor 直接读隐藏 backing storage |
| open/abstract、有 class 基类、object/companion 作为目标 | 使用手写 inspector；M38 不扩大现有 codec 的自动 class 形状范围 |

缺省 inspector 不采用 Serialize.rename／ignore／required／emitNull 改变其成员列表、名称或读取策略；debug 保留源码结构，序列化按其配置选择 wire 表示。DebugIgnore/DebugRedact 由 printer 解释。

没有字段能力、存在不可访问读取、无法满足形状或 dependency 有歧义时，在实现声明处报错。不能生成等待运行时再抛“尚未支持”的 body。

### 3.2 字段 inspector 的选择

为参与字段 F 选择 `Inspectable<F>`，复用现有 codec 合成的顺序和绑定时机：

1. 当前实现 class/struct 的 primary constructor 中，以 val 保存且满足完整接口 application 的显式依赖；多个匹配报歧义。
2. F 与被观察目标 T 相同则使用 this，支持正常递归结构。
3. F 是已知 nominal application，使用其正常可见、实际实现 `Inspectable<F>` 的 companion。
4. 核心 Option/Array/MutableArray/ArrayList 使用普通组合 inspector；Unit 与 tuple 使用普通 helper/InspectFunction。

选择在定义处完成，generic 实参替换不重新扫描 companion 或挑选依赖。裸类型参数需要显式 inspector；两个 binder 具体化后相等也不会改选实现。字段内部的 private 状态是否展示，由字段自己的 inspector 决定，外层 inspector 不自动继承其权限。

例如 `BoxInspector<E>(private val element: Inspectable<E>) : Inspectable<Box<E>>` 可以请求合成；它读取 Box 的 value 后调用保存的 element。`Box<E>` 自身不需要实现额外接口。

字段 inspector 的求值和调用放在该字段的惰性读取函数中，不在构造外层视图时递归建立整张对象图。正常递归先建立方法签名再生成 body，沿用现有值布局与单态化终止规则。

## 4. ValueView 数据模型

```scoop
public interface ValueView {
    public val type: TypeRef
    public val referenceIdentity: Option<Any>
    public fun shape(): ValueShape
}
```

`ValueView` 是异构值的只读观察表面，不是用 Any 保存所有字段再猜类型。读取适配函数保留真实 T/F，用普通带类型的读取取得值，再生成对应视图。

### 4.1 形状与惰性子节点

| 类型/分支 | 内容 |
| --- | --- |
| `ValueShape.Scalar(ScalarValue)` | Boolean、signed/unsigned 整数、Float、Double、Char、String、Unit |
| `ValueShape.Record(RecordView)` | 有序 MemberView 和可选的显式 base 视图 |
| `ValueShape.Variant(VariantView)` | 当前 VariantInfo 及 unit/named/positional payload |
| `ValueShape.IntegerEnum(IntegerEnumView)` | 当前 IntegerEnumVariantInfo（含底层整数值） |
| `ValueShape.Tuple(TupleView)` | 有序元素，按 index 惰性读取 |
| `ValueShape.Sequence(SequenceView)` | 逻辑 size 和 `element(index): ValueView` |
| `ValueShape.Opaque(OpaqueValue)` | 显式选择的不可展开表示，包含类型和简短的种类说明 |

ScalarValue 的 signed/unsigned carrier 可以分别用 Long/ULong，原数值类型由 `ValueView.type` 保留；Float/Double 不经另一精度中转。Char 与 String 分开，以便选择单引号或双引号展示。

| 相关类型 | 约定 |
| --- | --- |
| `ObservedMember` | `Field(FieldInfo)` 或 `Property(PropertyInfo)`，不混淆源码 property 与 backing field |
| `MemberView` | `info: ObservedMember` 和普通 `read(): ValueView` 函数；调用才读取当前实例值 |
| `RecordView` | members 保持本 inspector 的顺序；base 由显式 base inspector 提供，不复制 base 字段身份 |
| `VariantView` | 只保存已选 variant 的 payload，不暴露读取非活动存储的函数 |
| `TupleView` / `SequenceView` | 长度为 Long；一次 element 调用读取一个元素，不先构建整棵树 |

core 提供这些视图的普通构造入口，使用户可手写 inspector。合成实现使用相同的普通值和函数，不使用私有 runtime 读取通道。自定义视图的元数据和内容一致性是普通库 API 合同，不建立来源认证或重复的全图校验。

### 4.2 内建值与集合

core 为 Boolean、定宽整数、Float/Double、Char、String 提供具体 inspector，Unit 提供普通 singleton inspector。Option 和序列通过显式元素 inspector 组合。Option inspector 保留 None／Some 及其真实嵌套，不调用 M46 的 nullable Option codec；Some(None) 和 None 仍产生不同的观察结果。

Array、MutableArray、ArrayList 展示逻辑元素，不展示 capacity、backing、MaybeUninit 空闲槽。List/MutableList 可以通过显式普通 helper 和公开 size/get 观察；它们没有唯一具体容器类型，不从运行期实现类选择内部字段。

Any、普通 interface、函数值、Ptr/FunPtr、MaybeUninit 等没有通用的结构读取。调用者可显式选用 opaque inspector：Any/interface 显示静态类型，函数/指针显示种类，MaybeUninit 不读取潜在未初始化的 T。指针打印不自动解引用，核心原子类型也不通过隐藏字段读取；要展示原子值须显式使用其公开 load。

存在合法 Opaque 分支不意味着缺省合成遇到任意能力缺失就静默降级。缺少所需 inspector 时仍报编译错误，调用者通过普通依赖明确选择 opaque 策略。

### 4.3 静态类型、捕获与副作用

选择 `Inspectable<Base>` 表示按 Base 策略观察；运行期派生类不自动改变视图。调用者要展示更多内容，需选择相应实现，或按已有普通类型判断后手写分派。

值类型按值捕获；引用类型保存普通对象引用，不取得裸地址。`inspect` 建立浅视图，缺省实现不执行 getter 或提前读取全部子值。printer 对实际展示的成员只调用一次 read，对实际展示的元素只调用一次 element。

这是逐步观察，不承诺深拷贝或并发一致快照。手写 inspector 可调用普通方法并产生副作用或异常；这些行为按一般函数语义执行。被忽略、redact 或截断的子节点不再进入读取函数。

## 5. 文本 writer

```scoop
public interface TextWriter {
    public fun write(text: String): Unit
}

public class StringWriter public constructor() : TextWriter {
    private val builder: StringBuilder = StringBuilder()

    public override fun write(text: String): Unit {
        builder.add(text)
    }

    public fun build(): String = builder.build()
}
```

write 是普通、同步、可抛异常的接口方法，参数只有已生成的 String，不带方法级泛型。StringWriter 复用现有 StringBuilder。

printer 可以多次 write；只有拼接后的文本顺序属于输出合同，不保证分块边界或一次打印对应一个原子 IO 操作。writer 异常立即沿调用栈传播，此前输出可能已经写入。

printer 不调用 `print`/`println`，不自动换行、flush 或 close。stdout、stderr、文件和日志 writer 由应用或对应 IO 库实现；应用需要换行时显式 `writer.write("\n")`。CLI help 使用同一个接口。

## 6. DebugPrinter API 与输出

以下为签名目录：

| API | 行为 |
| --- | --- |
| `DebugPrinter.write(view: ValueView, writer: TextWriter, options: DebugOptions = DebugOptions()): Unit` | 逐步格式化到指定 writer |
| `DebugPrinter.format(view: ValueView, options: DebugOptions = DebugOptions()): String` | 使用 StringWriter，成功后返回完整文本 |

这两个入口没有隐式 inspector。调用写为：

```scoop
val writer = StringWriter()
DebugPrinter.write(User.Companion.inspect(user), writer)
val text = writer.build()
```

### 6.1 输出约定

| 值 | Compact 示例 |
| --- | --- |
| struct/class record | `User(name="Ada", age=37)` |
| unit/named/positional enum | `State.Idle`、`State.Failed(code=2)`、`Result.Ok(3)` |
| 整型 enum | `Color.Red(1)`，其中 1 是底层合法值 |
| tuple / Unit | `("Ada", 37)`、`Unit`；单元素 tuple 保留尾逗号 |
| Array/List | `[1, 2, 3]` |
| Option | `Option<String>.None`、`Option<String>.Some("out.txt")`，保留完整展示名与实际 variant |
| opaque | `<opaque Ptr<Int>>` 等明确的类型说明 |
| 环 | `<cycle Node>` |

类型展示名取 TypeRef；字段与 variant 取源码名称，不采用 Serialize.rename。String 双引号、Char 单引号，转义引号、反斜杠、换行、制表符及其他不可直接展示的控制字符。字符串截断按 Unicode scalar 进行，不截断 UTF-8 字节序列。

Float/Double 沿用 core 的数值展示规则，保留负零；不经过整数转换。值类型名称与集合外观不依赖裸内存布局。Pretty 模式在同一结构上换行并按 indent 缩进，不改变成员选择或读取顺序。

此格式用于调试，不是持久化编码合同，不替代 Encodable/Decoder。不自动实现 ToString，也不改变 `print`/`println` 的既有 ToString bound。

M46 的稳定容器和编码复用没有提供本视图需要的原对象 identity、惰性成员读取或源码 variant 信息。DebugPrinter 继续使用 Inspectable／ValueView；实现可以与 codec 合成共享普通类型、成员和依赖查询，但不把 debug 改成对 Encodable 输出树的再次格式化。

### 6.2 输出选项

`DebugOptions` 是普通 struct，具有 `layout: DebugLayout = Compact`、`indent: String = "  "`、`maxDepth: Long = 8L`、`maxItems: Long = 32L`、`maxStringLength: Long = 256L`。

三个数字必须非负，在向 writer 写入前检查；不合法时抛 IllegalArgumentException。根深度为 0，展开子节点时深度加一。达到深度上限时输出类型与省略标记，不读取其子节点；maxItems 限制每个容器实际展示的成员/元素，maxStringLength 限制每个字符串保留的 scalar 数量。

Ignore 后的成员不占展示项；redact 成员算一个展示项。达到 items 上限后不读取剩余值，并输出剩余数量或明确省略标记。这些限制只是 debug 文本截断选项，不决定程序是否合法，不进入编译器或 runtime 的通用资源预算。

### 6.3 DebugIgnore 与 DebugRedact

```scoop
@Target([AnnotationTarget.FIELD, AnnotationTarget.PROPERTY])
public annotation class DebugIgnore

@Target([AnnotationTarget.FIELD, AnnotationTarget.PROPERTY])
public annotation class DebugRedact(val replacement: String = "<redacted>")
```

printer 通过实际 annotationKey 读取 MemberView 上的成员注解，然后才决定是否执行 read：

1. 有 DebugIgnore：省略整个成员，不读取值。
2. 否则有 DebugRedact：保留成员名，把 replacement 作为带转义的字符串输出，不读取值。
3. 否则按普通方式读取和递归展示。

两者同时存在时 Ignore 优先。注解只作用于其实际 target；logical property 的 DebugRedact 不从 backing field、getter 或 constructor parameter 自动复制。合成 class 视图绑定 PropertyInfo，因此应在 property target 上标注。

编译器不解释这些注解来豁免访问检查、缺失的字段 inspector 或非法表达式。所有已生成 read body 都必须合法，即使 printer 最后没有调用它。

## 7. 循环引用与 GC

`referenceIdentity` 只为被观察的实际引用对象提供 `Some(originalReference)`；值类型视图返回 None。不能把 ValueView wrapper 或临时 box 的 identity 当成原值 identity。

printer 在当前递归路径保存这些普通引用，以既有 `===` 比较是否再次进入同一对象。遇到祖先对象输出 cycle；离开节点后从路径移除。共享对象若不在当前路径上仍可再次展开，不被误判为循环。

路径中的引用、视图捕获及读取闭包全部按普通 GC 保活和 relocation 处理；不以裸地址、地址转整数或自行缓存对象头作为 identity。测试必须覆盖读取函数、String 构造或 writer 回调中发生 GC/搬迁。

手写继承视图可以同时提供 derived 部分与 base 部分。base 是同一对象的静态投影，printer 在同一已进入的节点内处理，不为该 base 投影再次执行 enter/cycle 检查；base 的普通字段若回指原对象，仍然检测到循环。

异常退出通过正常函数 cleanup 结束本次打印，不复用残留路径。printer 不持有跨调用的全局 visited 集合，也不要求为对象建立永久编号。

## 8. 验收

- 手写与合成 inspector 的 public/internal/private/protected 结果一致，包含同 Cone、跨 Cone、companion、继承 receiver 和 generic 再次发布。
- 负例必须证明：把调用移入目标类型、写 inline 或在运行期分支中跳过 read，不能使外部非法成员读取合法。
- struct、enum 活动 payload、tuple、Option、核心集合、显式 base 投影和 opaque 策略具有实际运行用例；不靠 Any.toString 回退。
- 同一个 DTO 同时请求 Encodable／Decodable／Inspectable，验证 Serialize.rename／ignore／None 省略只改变序列化，debug 仍显示源码字段及 Option variant；Some(None) 与 None 不合并。DebugIgnore／DebugRedact 仍只控制 printer 的惰性读取。
- 读取计数验证惰性行为：ignore/redact/深度/items 截断不执行对应 read，展示字段恰好一次；相关 inspector 依赖也不提前求值。
- StringWriter 检查拼接文本；另一个测试 writer 在指定 write 抛异常，验证立即传播和已有部分输出，不把分块次数锁成公共 API。
- 对象自环、互环、非循环共享、含引用的值类型、数组与 GC stress 覆盖 rooting/relocation，确认不读取未初始化槽。
- core-only 应用可以完成全部 debug 流程，其依赖图中没有 scoop.cli。
