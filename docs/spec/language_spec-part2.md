# Scoop 语言规范 第 2 部分：类型系统、泛型与名义结构

版本：0.1 草案

本部分定义 Scoop 的类型分类、引用类型、值类型、nullable、装箱、泛型、variance、`where` 约束、对象与 class 初始化规则。涉及表达式求值、函数体、属性访问和模式匹配的规则见第 3 部分。

## 1. 类型总览

Scoop 的普通运行期类型分为引用类型和值类型；此外还有不属于任一运行期值类别的特殊类型：

```text
Types
├── Special types（无运行期值类别）
│   └── Nothing（bottom type，无 inhabitant）
│
├── Reference types（GC 管理、堆分配、按引用访问）
│   ├── class
│   ├── interface
│   ├── object / companion object 单例值
│   └── boxed value type
│
└── Value types（内联存储、copy 语义、不可变）
    ├── built-in scalar types
    ├── struct
    ├── enum
    ├── tuple
    └── Unit / ()
```

规则：

- 引用类型由 GC 管理，在赋值、传参和返回时复制引用。
- 值类型以内联值表示，在赋值、传参和返回时复制整个值。
- 所有值类型不可变；构造后不能修改字段。
- 值类型没有对象 identity，不能使用引用 identity 比较。
- 值类型可以包含引用字段，但编译器和运行时必须保证这些引用可被 GC 精确追踪。
- `Nothing` 没有运行期值或表示；它只用于描述永不正常返回的表达式或函数。

## 2. 根类型与特殊类型

### 2.1 `Any`

`Any` 是所有可作为普通值传递的引用根接口。值类型在需要赋给 `Any` 或接口类型时会被装箱。

```kotlin
val x: Any = 1
```

`Any` 本身是引用类型视图；值类型并不会因为可赋给 `Any` 而获得引用 identity。

### 2.2 `Nothing`

`Nothing` 是 bottom / uninhabited 类型。它没有运行期值，是所有类型的子类型，只能作为永不正常返回的表达式或函数的类型。常见来源：

```kotlin
effect Raise<E> {
    fun raise(error: E): Nothing
}

fun fail(): Nothing {
    Raise.raise("failed")
}
```

`return`、`break`、`continue`、`panic(...)` 或永不正常返回的表达式也可在类型推断中表现为 bottom。`Nothing` 既不是引用类型也不是值类型，不参与引用/值类型分类。

### 2.3 `Unit`

`Unit` 是 0 元 tuple 类型，写作 `()`。唯一值也是 `()`。无显式返回类型且正常完成的函数返回 `Unit`。

## 3. 内建标量类型

内建标量是语言内建值类型：布局与基本语义由编译器固定，可见声明由 sysroot 提供。

### 3.1 Bool

`Bool` 有两个值：

```kotlin
true
false
```

布尔运算使用 `!`、`&&`、`||`。`when` 对 `Bool` 可做穷尽性检查。

### 3.2 Char

`Char` 表示一个 Unicode scalar value。字符字面量见第 1 部分。

`Char` 支持：

- `==` / `!=`
- `<` / `<=` / `>` / `>=`，按码点顺序比较
- `when` 字符字面量模式

### 3.3 整数类型

Scoop 提供目标无关可见的一组整数值类型。

Word-sized：

- `Int`：目标原生有符号 word-sized 整数。
- `UInt`：目标原生无符号 word-sized 整数。

位宽等于目标指针宽度：

- 64 位目标：`Int` / `UInt` 为 64 位。
- 32 位目标：`Int` / `UInt` 为 32 位。

固定宽度：

- 有符号：`Int8`、`Int16`、`Int32`、`Int64`
- 无符号：`UInt8`、`UInt16`、`UInt32`、`UInt64`

标准别名：

```kotlin
typealias Byte   = UInt8
typealias Short  = Int16
typealias UShort = UInt16
typealias Long   = Int64
typealias ULong  = UInt64
typealias UIntPtr = UInt
```

注意：`Byte` 是无符号 8 位整数。

整数运算：

- 算术：`+`、`-`、`*`、`/`、`%`
- 比较：`==`、`!=`、`<`、`<=`、`>`、`>=`
- 位运算：`&`、`|`、`^`、`~`
- 移位：`<<`、`>>`

语义：

- 有符号整数使用二进制补码表示。
- 除非某个 intrinsic API 另有规定，整数运算按 `2^bitWidth` 模ular arithmetic wrap-around。
- 有符号 `>>` 是算术右移，无符号 `>>` 是逻辑右移。
- 移位计数按目标类型位宽取模，避免目标相关未定义行为。

### 3.4 浮点类型

内建浮点类型：

- `Float64`
- `Float32`
- `Double` 是 `Float64` 的类型别名。

浮点字面量默认类型为 `Float64`；带 `f` 或 `f32` 后缀为 `Float32`。浮点类型支持算术、比较、一元负号，以及语言实现提供的基础 intrinsic。NaN、infinite 和具体舍入行为按目标平台的 IEEE 风格浮点实现处理；需要严格跨平台数值语义的库应另行约束。

### 3.5 String

`String` 是 GC 管理的引用类型，具备不可变、内容相等的值语义。字符串字面量、插值和 raw 字符串见第 1 部分。

字符串完整 API 属于标准库范围；语言只固定：

- 字符串字面量类型为 `String`。
- `String` 可参与 `==` / `!=` 内容比较。
- `f"..."` 插值字符串按表达式求值后构造 `String`。

## 4. 引用类型

### 4.1 Class

Class 是 GC 管理、堆分配的名义引用类型。

```kotlin
open class Animal(val name: String)
class Dog(name: String, val breed: String) : Animal(name)
abstract class Shape { abstract fun area(): Double }
sealed class Expr { ... }
```

规则：

- Class 默认 `final`。
- 标记 `open` 的 class 可被继承。
- `abstract` class 不能直接实例化，可声明 abstract 成员。
- `sealed` class 限制直接子类位于同一编译单元。
- Class 支持单继承和多接口实现。
- 子类继承 class 使用 `: Base(args...)`，实现接口使用 `: InterfaceName`。

### 4.2 构造器与初始化

Class 可在头部声明主构造器：

```kotlin
class User(val name: String, age: Int)
```

规则：

- 主构造器参数带 `val` / `var` 时成为属性。
- 不带 `val` / `var` 的构造器参数只在初始化阶段可见。
- 属性 initializer 和 `init { ... }` 块按源码顺序交错执行。
- 主构造器的 `val` / `var` 参数属性在属性 initializer 和 `init` 块前已经可用。
- `init { ... }` 中可见 `this` 与主构造器参数。
- 初始化阶段禁止读取尚未初始化的后置属性；这是前向引用错误。

示例：

```kotlin
class Box(x: Int) {
    val y: Int = x + 1

    init {
        val z = y
    }
}
```

次构造器：

```kotlin
class Box(x: Int, y: Int = 0) {
    constructor(s: String) : this(0, 0) {
        // body
    }
}
```

规则：

- 次构造器写作 `constructor(params...) [: this(...) | super(...)] { body }`。
- 当 class 有主构造器时，次构造器必须显式委托到 `this(...)`，不能直接 `super(...)`。
- 没有主构造器的 class 可用次构造器委托到 `super(...)`。
- 委托调用的参数可使用命名参数和默认参数。
- 被委托构造器或父构造器完成后，当前次构造器 body 执行。
- 次构造器 body 不是普通函数体；不允许用 `return` 从其中返回值。

初始化顺序概念上为：

1. 调用父类构造器。
2. 初始化主构造器参数属性。
3. 按源码顺序执行属性 initializer 与 `init {}`。
4. 若调用的是次构造器，执行该次构造器 body。

### 4.3 继承与 override

规则：

- 只允许单 class 继承。
- 可实现多个接口。
- 覆写成员必须写 `override`。
- 被覆写成员必须是 `open` 或 `abstract`。
- 覆写方法的 required effect row 不能比基类/接口签名要求更多，见第 4 部分。
- 成员查找优先成员本身；同签名 extension 会被成员遮蔽。

### 4.4 Interface

接口可声明抽象成员和默认实现：

```kotlin
interface Hashable {
    fun hash(): Int
}

interface Printable {
    fun print() { println(toString()) }
}
```

规则：

- 引用类型和值类型都可以实现接口。
- 接口本身是引用类型视图；值类型赋给接口变量时会装箱。
- 接口可作为泛型 `where` bound。
- 接口方法参与动态分发，具体 ABI 是实现细节。

### 4.5 Object 与 Companion Object

`object` 声明 Kotlin 风格单例：

```kotlin
object Config {
    val port: Int = 8080
}
```

规则：

- `object Name { ... }` 声明一个名为 `Name` 的单例值和对应的单例类型。
- 单例不能通过构造器调用创建；`Name()` 是编译错误。
- 引用单例值或访问其成员会触发单例初始化。
- 单例初始化 once-only；多次读取不会重复执行 `init` 或属性 initializer。
- 跨线程并发访问同一单例时，初始化仍只能执行一次；具体同步机制是实现细节。
- `object` 可嵌套在 class 或 object 中，可声明属性、函数、嵌套类型和 `init` 块。

Companion object：

```kotlin
class User {
    companion object {
        val kind: String = "user"
    }
}

val k = User.kind
```

规则：

- Class 可声明一个 companion object。
- 未命名 companion 可通过 `ClassName.member` 访问其成员。
- 命名 companion 也可通过 `ClassName.CompanionName.member` 访问。
- 命名 companion 的 `ClassName.member` 和 `ClassName.CompanionName.member` 指向同一个 companion 实例；初始化共享 once-only 状态。
- 如果通过 `ClassName.member` 访问但 class 没有 companion object，则是解析错误。

## 5. 值类型

所有值类型共同规则：

- 不可变。
- 无继承。
- 可实现接口。
- 无 identity。
- copy semantics。
- 可嵌入引用字段，但必须保持 GC 精确可追踪。

### 5.1 Struct

Struct 是具名值类型，字段具名。

```kotlin
struct Point(val x: Int, val y: Int)

struct Color(val r: Byte, val g: Byte, val b: Byte, val a: Byte) : Hashable {
    fun hash(): Int = ...
}
```

字段规则：

- Struct 可实现接口。
- Struct 不能继承 struct 或 class。
- 直接字段可写在主构造器中，也可写在类型体中。
- 直接字段必须是 `val`。
- Struct 不引入 `var` 字段；值更新必须构造新值。
- 直接字段可有默认值。
- 默认值在构造点求值：先求显式实参，再求缺省字段，最后组装值。
- 直接字段参与布局、构造、解构和 `with` copy-update。
- 计算属性不参与布局和构造。

构造：

```kotlin
val p1 = Point(1, 2)
val p2 = Point { x: 1, y: 2 }

struct Offset(val x: Int = 1) {
    val y: Int = x + 1
}

val a = Offset()
val b = Offset(y = 5)
val c = Offset { x: 10 }
```

`StructName(...)` 和 `StructName { ... }` 都按字段绑定规则构造值。缺少必填字段或字段类型不匹配是编译错误。

### 5.2 Enum

Enum 是 tagged union 值类型。每个 variant 可携带 0 个或多个字段：

```kotlin
enum Color {
    Red,
    Green,
    Blue,
    Custom(val r: Byte, val g: Byte, val b: Byte)
}

enum Option<T> {
    Some(val value: T),
    None
}

enum Result<T, E> {
    Ok(val value: T),
    Err(val error: E)
}
```

规则：

- Enum 是值类型，不可变，copy semantics。
- Variant 可携带字段。
- Enum 可实现接口。
- Enum 可声明函数和 getter-only 计算属性。
- Variant 构造值可在期望 enum 类型明确时使用短名，也可用限定名。
- Enum variant 支持 `when` 模式匹配和解构。

布局：

- 概念布局为 `tag + payload`。
- tag 大小按 variant 数量选择：不超过 256 个 variant 可用 `UInt8`，不超过 65536 个可用 `UInt16`；更大情况由实现决定。
- 编译器可对过大的 variant 自动装箱并发出 lint。

GC trace 安全：

- 对于 payload 中可能包含 GC 引用的 enum，GC 指针 slot 集合必须对该 enum 类型的某个单态化实例静态可枚举。
- 这个 slot 集合不能依赖运行期 tag。
- 每个 GC 指针 slot 在所有 variant 下都必须只包含 `null` 或合法 GC 对象指针。
- 非引用数据不能和 GC 指针 slot 重叠。
- 未使用的 GC 指针 slot 必须初始化并维持为 `null`。

因此，以下 tag-dependent union 布局是非法实现策略：

```kotlin
enum E {
    Ref(val s: String),
    Bits(val x: UInt64)
}
```

如果同一机器字在 `Ref` 中是引用、在 `Bits` 中是整数，静态 stack map / heap bitmap 无法安全追踪。实现可以把引用 payload 和非引用 payload 分离，或对不适合内联的 variant 进行装箱。

Niche 优化：

- `Option<ReferenceType>` 可用空指针表示 `None`，零额外开销。
- `Option<Bool>` 可用额外值表示 `None`。
- 对 GC 引用或运行期表示含 GC 指针的值类型，禁止使用非空 pointer-shaped niche，例如嵌套 `Option<Option<Ref>>` 不能用 `0x1` 这类非法地址表示额外状态。
- 无可用 niche 的 `Option<ValueType>` 使用显式 tag。

### 5.3 Value-only Enum

Value-only enum 是所有 variant 都无 payload、且内存表示等同指定整数底层类型的 enum，主要用于 FFI 和低层代码：

```kotlin
enum SomeEnum: Int {
    A = 0
    B = 1
    C = 2
}
```

规则：

- `:` 后底层类型必须是整数标量类型。
- Variant 不得声明字段。
- 每个 variant 有一个底层整数值。
- 面向 FFI 的 enum 建议显式写出所有值；隐式递增规则若存在，由实现定义。
- 类型系统中仍是独立名义类型，不会和底层整数类型隐式互换。
- ABI / 内存布局与底层整数类型完全一致。
- 可在 `@Extern` 签名和 `@CLayout` struct 中使用。

### 5.4 Tuple

Tuple 是匿名结构值类型，字段按位置编号。

Unit：

```kotlin
val u: () = ()
```

1 元 tuple 必须写 trailing comma：

```kotlin
val single: (Int,) = (42,)
val grouped: Int = (42)
```

多元 tuple：

```kotlin
val pair: (Int, String) = (1, "hello")
val triple: (Int, Bool, String) = (1, true, "abc")
```

字段访问：

```kotlin
val p = (10, "hi")
val x = p.0
val y = p.1
```

规则：

- Tuple 结构化类型：任意位置的 `(Int, String)` 是同一类型。
- Tuple 是值类型，不可变，copy semantics。
- Tuple 支持 `val` 解构和 `when` 模式。
- `()` 是 0 元 tuple 和 Unit 类型。

## 6. Nullable

Nullable 类型是 `Option<T>` 的语法糖：

```kotlin
val x: Int? = 42
val y: Int? = null
```

规则：

- `T?` 脱糖为 `Option<T>`。
- `null` 脱糖为 `None`。
- 非空值在 nullable 期望上下文中脱糖为 `Some(value)`。
- 嵌套不扁平化：`T??` 是 `Option<Option<T>>`，有 `Some(Some(v))`、`Some(None)`、`None` 三种状态。

Null-safe 运算符：

- `x?.member`：若 `x` 是 `Some(v)`，计算 `v.member` 并包装为 `Some(...)`；若 `None`，结果为 `None`。
- `x?.call(args...)`：只有 `x` 是 `Some(v)` 时才求值 `args...` 并调用。
- safe-call 结果总是 nullable。
- `x ?: y`：若 `x` 是 `Some(v)`，结果为 `v`；否则求值并返回 `y`。右侧按需求值。
- `x!!`：若 `Some(v)` 返回 `v`；若 `None`，执行 `Raise.raise(RuntimeError.NullAssertionFailed)`，因此需要 `Raise<RuntimeError>`，除非被处理。

## 7. 装箱

值类型赋给接口类型或 `Any` 时会装箱为 GC 管理对象：

```kotlin
val n: Any = 42
val h: Hashable = 42
```

规则：

- O(1) 装箱可隐式发生：值类型赋给接口/`Any`、传给接口/`Any` 参数、作为接口/`Any` 返回值。
- O(n) 容器元素装箱必须显式进行。`Array<Int>` 到 `Array<Any>` 或 `List<Int>` 到 `List<*>` 这类转换不能隐式逐元素装箱。
- 数组字面量内部不做逐元素隐式装箱，见第 3 部分。

## 8. `with` Copy-update

值类型不可变；`with` 表达式创建修改后的副本。Scoop 保留 `with` 作为值类型的更新机制，不引入 struct `var` 字段或 mutating method，原值不会被修改：

```kotlin
val p2 = p with { x: 5 }

val line2 = line with {
    start.x: 1,
    start.y: 2,
}

val result2 = result with {
    Ok.point.x: 5,
    Err.code: 42,
}
```

规则：

- 语法：`expr with { path: value, ... }`。
- 所有右侧表达式都相对于原始值求值；更新之间没有顺序依赖。
- path 可任意深：`a.b.c`。
- 对 enum payload 更新时，path 第一段必须是 variant 名。
- Enum `with` 保留当前运行期 variant。当前 variant 对应的更新会重建该 variant 的 payload；其它 variant 的更新被忽略。
- 返回新值，原值不变。

`with` 关键字不再用于 Kotlin 的 scope function；库可提供其它名字或 extension 函数，但不属于语言语法。

## 9. 泛型

泛型声明：

```kotlin
fun <T> identity(value: T): T = value

struct Box<T> {
    val value: T
}
```

实现模型：

- 泛型通过单态化实现；编译器为具体类型实参生成专门代码。
- 对引用类型实参，编译器可以共享代码，因为运行期布局都是 GC 指针。
- 对值类型实参，通常需要专门代码，因为布局不同。
- 不需要 `reified`；类型信息在编译期可用。

### 9.1 类型参数与 variance

类型参数默认 invariant。

```kotlin
interface Producer<out T> {
    fun produce(): T
}

interface Consumer<in T> {
    fun consume(value: T)
}
```

规则：

- `out T` 使类型构造器在 `T` 上协变。
- `in T` 使类型构造器在 `T` 上逆变。
- `out` 类型参数只能出现在 public API 的 out-position，例如返回类型。
- `in` 类型参数只能出现在 public API 的 in-position，例如参数类型。
- 实现内部可在类型检查能证明安全的地方使用更宽松规则。

Scoop 限制：

- variance 产生的子类型关系只在对应类型实参为引用类型时适用。
- 当类型实参是值类型时，由于内存布局不同，不应用 variance 子类型；需要显式装箱或转换。

```kotlin
val catProducer: Producer<Cat> = ...
val animalProducer: Producer<Animal> = catProducer

val intProducer: Producer<Int> = ...
val anyProducer: Producer<Any> = intProducer // 编译错误，需要显式转换
```

### 9.2 Star projection

`Type<*>` 是 unknown type argument 视图，类似 `Type<out Any?>` 的读取视图。若源容器保存的是值类型元素，转换到 star projection 视图需要显式装箱。

具体集合类型的 star projection API 是库层内容；语言只规定类型参数擦除和装箱不能隐藏 O(n) 成本。

### 9.3 Effect row 参数

泛型参数列表可引入一个 effect row 参数：

```kotlin
interface Disposable<eff E = Pure> {
    fun dispose(): Unit / E
}
```

使用处：

```kotlin
val d0: Disposable = ...
val d1: Disposable<eff Async> = ...
val d2: Disposable<eff (Async + Raise<IOError>)> = ...
```

规则：

- `eff` 只在 `<...>` 泛型参数/实参列表中作为上下文关键字。
- 一个泛型列表最多有一个 `eff` 子句。
- `eff` 子句必须出现在最后。
- 本草案中 `eff` 子句一次只引入或提供一个 effect row 参数。

Effect row 语义见第 4 部分。

### 9.4 Where 约束

泛型函数和泛型类型可使用 Kotlin 风格 `where` 子句：

```kotlin
interface Show {
    fun show(): String
}

fun <T> display(x: T): String where T: Show {
    return x.show()
}

class Box<T>(val value: T) where T: Show
```

规则：

- `where` 子句由一个或多个 `T: Bound` 约束组成，以逗号分隔。
- 约束目标必须是当前声明的类型参数。
- Bound 可以是普通名义类型，也可以带类型实参：`T: Producer<String>`。
- 重复的完全相同约束是编译错误。
- 冲突 class bound 是编译错误；例如同一 `T` 同时要求两个不相关 class 上界。
- 使用泛型声明时，实际类型实参必须满足所有 bound。
- 在泛型声明体内，类型参数值可使用 bound 接口/类提供的成员。

示例：

```kotlin
interface Producer<T> {
    fun produce(): T
}

class StringProducer() : Producer<String> {
    fun produce(): String = "ok"
}

class Reader<T>(val p: T) where T: Producer<String> {
    fun read(): String = p.produce()
}
```

Annotation class 不允许类型参数、effect 参数或 `where` 子句。

## 10. 类型别名

`typealias` 引入等价类型名：

```kotlin
typealias UserId = Int
typealias Handler<T> = (T) -> Unit
```

规则：

- 类型别名在类型检查时展开为其目标类型。
- 类型别名不创建新的名义类型。
- 泛型类型别名可有类型参数。
- 类型别名与同一类型命名空间内的 class/struct/enum/interface/object 名冲突。

## 11. Function Type

函数类型见第 3 部分的函数语义；类型层面写法为：

```kotlin
(A, B) -> C
T.(A, B) -> C
(A, B) -> C / R
T.(A, B) -> C / R
```

规则：

- `T.(A) -> R` 是 receiver function type；receiver lambda 内 `this: T`。
- 省略 effect row 时默认为 `/ Pure`。
- Effect row 可为 open 或 closed，见第 4 部分。
- 函数类型子类型包含参数逆变、返回协变和 effect row widening。
- 运行期 cast 不定义在函数类型上；函数值擦除到 `Any` 只允许 closed pure 函数类型，见第 3 部分。

## 12. GC-free 类型

部分低层语义要求 GC-free value type：

- `Ptr<T>` 的 pointee。
- `@CLayout` struct。
- 普通 `@Extern` C ABI 参数、返回值和外部全局变量。
- 顶层 `@ThreadLocal` / `@Global var`。

一个类型是 GC-free，当且仅当：

- 它是值类型。
- 它直接和间接字段都不含引用类型。
- 它的运行期表示中不包含 GC 指针；包括 `Option<RefType>` 这种 niche 表示也算含 GC 指针。

GC-free 限制由静态类型检查强制执行。
