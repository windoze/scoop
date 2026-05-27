# Scoop 语言规范 第 3 部分：表达式、函数、属性、模式匹配与推断

版本：0.1 草案

本部分定义表达式语法、求值顺序、函数与 lambda、属性、委托属性、控制流、模式匹配、数组字面量、迭代语法、运算符和类型推断。类型本身见第 2 部分；效果语义见第 4 部分。

## 1. 表达式与语句

Scoop 使用表达式导向语义。`if`、`when`、`try`、`handle`、`do` 都是表达式。函数体和块由语句序列组成，尾表达式可作为块值。

普通 block 表达式必须写作 `do { ... }`：

```kotlin
val n = do {
    val base = 40
    base + 2
}
```

规则：

- 裸 `{ ... }` 在普通表达式位置总是 closure literal。
- 普通局部 block 必须写 `do { ... }`。
- `do { ... }` 的值是最后一个未以 `;` 终止的表达式。
- 若没有尾表达式，block 值为 `Unit`。
- 由语言结构引入的块，例如 `if {}`、`when {}`、`handle {}`、`try {}`、声明体，不受“裸 `{}` 是 closure”规则影响。

## 2. 变量绑定与赋值

```kotlin
val x: Int = 10
var y: Int = 10
y = 20
```

规则：

- `val` 声明不可重新赋值的绑定。
- `var` 声明可重新赋值的绑定。
- 值类型自身不可变；`var` 只表示变量 slot 可重新绑定。
- 局部 `val` / `var` 可从 initializer 推断类型。
- 函数参数是不可重新赋值的绑定。
- 顶层简单 `val` / `var` 的类型注解规则见第 1 部分。

赋值表达式只允许可赋值左值：

- 局部 `var`。
- 可变属性 setter。
- 可变顶层 `@ThreadLocal` / `@Global var`。
- 通过 `set`/索引赋值语义支持的目标。

对 `val`、值类型字段或只读属性赋值是编译错误。

## 3. 运算符优先级

从高到低：

| 优先级 | 运算符/结构 | 结合性 |
|---|---|---|
| postfix | 调用 `()`, 成员 `.`, safe-call `?.`, not-null `!!`, class literal `::class`, type application | 左 |
| prefix | `!`, unary `-`, `~` | 右 |
| multiplicative | `*`, `/`, `%` | 左 |
| additive | `+`, `-` | 左 |
| shift | `<<`, `>>` | 左 |
| range / relational / type | `..`, `<`, `<=`, `>`, `>=`, `is`, `!is`, `as`, `as?` | 左 |
| equality | `==`, `!=` | 左 |
| bitwise and | `&` | 左 |
| bitwise xor | `^` | 左 |
| bitwise or | `|` | 左 |
| logical and | `&&` | 左 |
| logical or | `||` | 左 |
| Elvis | `?:` | 右 |
| assignment | `=` | 右 / statement-like |

说明：

- `a ?: b ?: c` 解析为 `a ?: (b ?: c)`。
- `..` 构造 inclusive range，range 类型和完整迭代 API 属于库层；语言固定语法和优先级。
- `as` / `as?` 是类型 cast，见本部分 “类型测试与 cast”。

## 4. 调用、参数与重载

### 4.1 函数调用

```kotlin
foo()
foo(1, "x")
foo(x = 1, y = "x")
```

规则：

- 实参从左到右求值。
- 位置实参必须位于命名实参之前。
- 命名实参可重排。
- 有默认值的参数可省略。
- 省略参数时使用声明处默认值；默认值在调用点语义上求值。
- 参数绑定适用于普通函数、成员函数、构造器、次构造器委托和 `super(...)` 构造调用。

### 4.2 Default arguments

```kotlin
fun connect(host: String, port: Int = 80): Unit { ... }

connect("example.com")
connect("example.com", 8080)
connect(port = 8080, host = "example.com")
```

默认参数必须满足参数类型要求。默认参数表达式可引用位于其前方的参数；是否允许引用后续参数由实现诊断，但可移植代码不应这样写。

### 4.3 Vararg 与 spread

```kotlin
fun logAll(vararg values: String): Unit { ... }

logAll("a", "b")
logAll(*items)
logAll(values = *items)
```

规则：

- `vararg` 参数接收零个或多个实参。
- 类型推断按参数元素类型处理每个 vararg 实参。
- `*expr` 是 spread 实参，要求 `expr` 的类型可作为 vararg 容器展开。
- spread 的具体容器类型由标准库/API 定义；语言只固定语法和参数绑定位置。

### 4.4 Trailing lambda

当调用的最后一个或多个参数为函数类型时，可把对应 closure 写在括号外：

```kotlin
users.filter { it.age >= 18 }
combine(1) { it + 1 } { it + 2 }
consume(do { computeValue() }) { x -> x + 1 }
```

规则：

- 只有裸 `{ ... }` closure literal 参与 trailing lambda。
- `do { ... }` 不会成为 trailing lambda。
- 多个 trailing lambda 按参数顺序绑定到末尾函数类型参数。
- `foo { ... }` 总是调用表达式，不是把局部 block 作为普通表达式使用。

### 4.5 重载决议

同名函数、成员函数、构造器和 extension 函数可形成 overload set。

规则：

- 仅返回类型不同不能构成合法重载。
- 重载按实参个数、命名参数、默认参数可用性、receiver 类型和实参类型筛选。
- 多个候选同时匹配时，选择更具体候选。
- 若没有唯一最具体候选，是 ambiguous overload 编译错误。
- 成员函数优先于同签名 extension 函数。
- Extension 重载可按 receiver 更具体或参数更具体选择。

示例：

```kotlin
fun f(x: Int): Int = 1
fun f(x: String): Int = 2

val a = f(1)
val b = f("hi")
```

## 5. 函数声明

```kotlin
fun add(a: Int, b: Int): Int = a + b

fun greet(name: String): String {
    return f"Hello, {name}!"
}
```

规则：

- 函数参数类型必须显式声明。
- Public 函数返回类型必须显式声明。
- Private/internal 非递归函数可省略返回类型，由所有返回路径推断。
- 递归函数必须显式声明返回类型。
- 函数可有 expression body：`= expr`。
- 函数可有 block body：`{ ... }`。
- 无显式返回类型且无非 Unit 返回路径时，返回 `Unit`。
- `return` 总是返回最近的具名函数。

### 5.1 Non-local return

Scoop 不支持 non-local return。Lambda 内的 `return` 不能返回外层函数。需要早退时使用显式循环、`return` 于当前函数、`break`、`continue` 或效果/异常语义。

## 6. Lambda 与函数值

Closure literal：

```kotlin
val f = { x: Int -> x + 1 }
val g: (Int) -> Int = { it + 1 }
val h = { println("hello") }
```

规则：

- 裸 `{ ... }` 是 closure literal。
- 参数可显式写在 `->` 前。
- 当期望类型明确且只有一个参数时，可使用隐式 `it`。
- Closure body 使用尾表达式规则。
- Closure 可捕获外层局部变量。
- 捕获 `var` 的具体表示由实现决定，但必须保留重新赋值语义。

Receiver lambda：

```kotlin
val block: String.(Int) -> Int = { n -> this.length + n }
```

规则：

- Receiver function type 写作 `T.(A, B) -> R`。
- Receiver lambda 内 `this` 类型为 `T`。
- Receiver function value 可作为普通值传递；调用语法可由实现支持为 `f(receiver, args...)` 或 receiver 形式。

### 6.1 函数类型与 effect

函数类型可带 effect row：

```kotlin
val pure: () -> Int = { 1 }
val io: () -> String / Raise<IOError> = { ... }

fun <T, eff E> run(block: () -> T / E): T / E {
    return block()
}
```

规则：

- 省略 effect row 时为 `/ Pure`。
- 如果 `R1 ⊆ R2`，则 `(A) -> B / R1` 是 `(A) -> B / R2` 的子类型。
- Pure 函数可传给更 effectful 的期望函数类型。
- 参数逆变、返回协变和 effect row widening 在赋值、分支合并、泛型实例化和期望类型中应用。

### 6.2 函数值擦除

函数值 cast/boxing 到 `Any` 只允许 closed pure 函数类型：

```kotlin
val ok: (Int) -> String / Pure! = { x -> x.toString() }
val a: Any = ok

val bad: () -> Unit / Raise<RuntimeError> = {
    Raise.raise(RuntimeError.NullAssertionFailed)
}
val b: Any = bad // 编译错误
```

规则：

- 允许：`((A) -> B / Pure!) as Any` 以及隐式擦除到 `Any`。
- 禁止：任意非 `Pure!` 函数类型擦除到 `Any`，包括 open `/ Pure`。
- 不定义从 `Any` 回 cast 到函数类型的运行期 `as` / `as?`。
- 不定义函数类型之间的运行期 `as` / `as?`；使用静态函数子类型和期望类型。

## 7. Extension 函数

Extension 函数把 receiver 类型写在函数名前：

```kotlin
fun String.wordCount(): Int {
    if (this.isEmpty()) return 0
    return this.split(" ").size
}

fun <T> List<T>.secondOrNull(): T? {
    return if (this.size >= 2) this[1] else null
}
```

规则：

- Extension 静态分发：由 receiver 静态类型决定。
- Extension 可定义在引用类型、值类型、tuple、nullable 类型上。
- Extension 不能访问 receiver 的 `private` / `internal` 成员，除非正常可见性允许。
- 同签名成员函数优先，extension 被遮蔽。
- Extension 可泛型化，可声明 effect。
- Extension 可通过 import 引入。
- 编译模型上可等价为 receiver 作为第一个参数的静态函数。

## 8. 属性

### 8.1 基本属性

```kotlin
class User(private var _name: String, private var _age: Int) {
    var email: String = ""

    val displayName: String
        get() = f"{_name} (age {_age})"

    var name: String
        get() = _name
        set(value) {
            _name = value
        }

    val isAdult: Bool
        get() = _age >= 18
}
```

规则：

- `val` 属性有 getter。
- `var` 属性有 getter 和 setter。
- 有默认 accessor 或 accessor 引用 `field` 时，编译器生成 backing field。
- `field` 只在属性 accessor 内可用。
- 如果 accessor 不引用 `field` 且没有默认 backing 需求，属性是纯计算属性。
- 属性访问 `obj.prop` 语义上等价于 getter 或 setter 调用；直接字段访问只是优化。

### 8.2 值类型属性

Struct 可声明直接字段和 getter-only 计算属性；enum 可声明 getter-only 计算属性：

```kotlin
struct Point(val x: Int, val y: Int) {
    val magnitude: Double
        get() = sqrt((x * x + y * y).toDouble())
}

enum Shape {
    Circle(val radius: Int),
    Rect(val width: Int, val height: Int)

    val area: Int
        get() = when (this) {
            Circle(r) -> r * r * 3
            Rect(w, h) -> w * h
        }
}
```

规则：

- 值类型不可变，因此不允许 setter。
- Struct 直接字段参与布局、构造、解构和 `with`。
- 计算属性不参与布局和构造。

### 8.3 Extension 属性

```kotlin
val String.lastChar: Char
    get() = this[this.length - 1]

var StringBuilder.lastChar: Char
    get() = this[this.length - 1]
    set(value) { this[this.length - 1] = value }
```

规则：

- Extension 属性必须是计算属性，不能有 backing field。
- `var` extension 属性可有 setter。
- 编译模型上等价于带 receiver 的静态 getter / setter 函数。

### 8.4 委托属性

委托属性使用 `by`：

```kotlin
class Example {
    var text: String by TrimDelegate()
}
```

语言规则：

- `val p: V by d` 要求 delegate 提供可调用的 `getValue(thisRef, property): V`。
- `var p: V by d` 还要求 `setValue(thisRef, property, value: V)`。
- `thisRef` 表示宿主对象；顶层委托属性的 receiver 形态由实现定义。
- `property` 是属性元数据值，至少提供属性名和类型等编译期信息。
- 委托属性展开为隐藏 delegate 存储 + 属性 getter/setter。
- 委托属性只适用于引用类型（class/object）属性；值类型不可变且无 identity，不支持委托属性。

标准 delegate（如 lazy/observable/vetoable）的具体 API、线程策略和库实现不属于本文档范围。

## 9. 控制流

### 9.1 If

```kotlin
val x = if (cond) { 1 } else { 2 }
```

规则：

- `cond` 必须是 `Bool`。
- `if` 可作为表达式。
- 表达式位置通常需要 `else`，除非上下文允许 `Unit` 或控制流不继续。
- 分支类型通过 LUB 合并。
- 分支 required effects 合并。

### 9.2 While

```kotlin
while (cond) {
    body()
}
```

规则：

- `cond` 必须是 `Bool`。
- `while` 的值为 `Unit`。
- `break` 退出最近循环。
- `continue` 进入最近循环下一次迭代。

### 9.3 For

`for` 是基于迭代协议的语法糖：

```kotlin
for (x in xs) {
    body
}
```

概念脱糖：

```kotlin
val it = xs.iterator()
while (true) {
    when (it.next()) {
        Some(x) -> { body }
        None -> break
    }
}
```

语言规则：

- `x` 只在循环体内可见。
- `iterator()`、`next()` 和 loop body 的 required effects 按普通 effect 规则合并。
- 迭代协议的完整接口属于核心/标准库表面；语言只固定 `for (x in expr)` 的绑定和脱糖形状。

### 9.4 Return / Break / Continue

```kotlin
return value
break
continue
```

规则：

- `return` 只允许在函数体内。
- `return` 退出最近具名函数，不退出外层函数。
- `break` / `continue` 只允许在循环内。
- 这些控制流表达式可表现为 `Nothing`，用于分支类型合并。

## 10. When 与模式匹配

Scoop 使用 `when`：

```kotlin
when (value) {
    Circle(r) -> pi * r * r
    Rect(w, h) -> w * h
    else -> 0.0
}
```

模式种类：

- Enum variant：`Some(x)`、`None`、`Ok(value)`。
- 类型测试：`is Dog`。
- Tuple：`(a, b)`、`(0, _)`。
- Struct：`Point { x, y }`、`Point { x: px, y: py }`。
- 字面量：整数、字符、字符串、布尔。
- Or-pattern：`North | South`。
- Guard：`pattern if condition`。
- Wildcard：`_`。
- Rest：`..`，忽略剩余字段或元素。

示例：

```kotlin
when (pair) {
    (0, _) -> "starts with zero"
    (x, y) if x == y -> "equal"
    (x, y) -> f"different: {x}, {y}"
}

when (point) {
    Point { x, y } -> x + y
}
```

### 10.1 穷尽性

编译器检查以下类型的覆盖：

- `enum`
- `Bool`
- `Option<T>`
- 由上述类型组成的 tuple/nested pattern

非穷尽类型必须有 `else` 或 `_`：

- `Int`
- `String`
- 所有数值类型
- interface 类型
- 含 guard 的模式集合

如果穷尽类型所有 case 已覆盖但仍写 `else`，编译器应给出 redundant warning，以便未来新增 variant 时暴露问题。

### 10.2 解构绑定

`val` 支持解构：

```kotlin
val (a, b) = getPair()
val Point { x, y } = makePoint()
val Some(value) = maybeValue
```

规则：

- `val` 支持 tuple、struct 和 enum variant 解构。
- `var` 不支持解构模式；只能简单绑定。
- 顶层解构见第 1 部分。
- 若 enum variant 模式在运行期不匹配，语言必须有确定行为；当前规范建议将不可匹配的无保护顶层/局部解构视为编译期可证明错误或运行期 trap，具体诊断由实现决定。推荐在一般控制流中使用 `when` 覆盖失败分支。

## 11. 类型测试、Smart Cast 与 Cast

### 11.1 `is` / `!is`

```kotlin
if (animal is Dog) {
    animal.bark()
}

if (animal !is Dog) return
animal.bark()
```

规则：

- `is` 对引用类型执行运行期类型测试，结果为 `Bool`。
- `!is` 是 `!(expr is Type)` 语法糖。
- `is` 不用于值类型 variant 判断；值类型用 enum pattern。

### 11.2 Smart Cast

Smart cast 是成功类型测试后的静态窄化。

适用位置：

- `if (x is T) { ... }` true 分支。
- `if (x !is T) return` 之后的继续路径。
- `&&` 右侧：`x is T && x.member`。
- `when` 的 `is T` 分支。

限制：

- 仅适用于 `val` 绑定和函数参数。
- 不适用于可重新赋值的 `var`。
- 仅适用于引用类型。
- 编译器必须能证明窄化期间没有并发或别名可变破坏。
- 不能 smart cast 时应报错并建议显式 cast。

### 11.3 `as` / `as?`

```kotlin
val dog = animal as Dog
val maybeDog = animal as? Dog
```

规则：

- `as` 是不安全运行期 cast；失败时执行 `Raise.raise(RuntimeError.ClassCastFailed)`，因此需要 `Raise<RuntimeError>`。
- `as?` 是安全 cast；失败返回 `None`，结果类型为 `T?`。
- `as` / `as?` 不执行值类型数值转换。
- 指针/整数转换不通过 `as` / `as?`，必须使用 unsafe intrinsic，见第 6 部分。
- 函数类型的运行期 cast 受第 6.2 节限制。

## 12. 数组字面量

```kotlin
val xs = [1, 2, 3]
val ys = ["hello", "world"]
val ps = [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]
```

规则：

- 若期望类型是 `Array<T>`，字面量类型为 `Array<T>`。
- 若期望类型是 `MutableArray<T>`，字面量类型为 `MutableArray<T>`。
- 无期望类型时默认 `Array<T>`。
- 元素类型 `T` 是所有元素表达式类型的 LUB。
- 空字面量 `[]` 必须有期望类型，否则无法推断 `T`。
- 元素从左到右求值。
- 数组分配后按源码顺序填充。
- 数组字面量内部不执行隐式逐元素装箱。

```kotlin
val a1 = [1 as Any, 2 as Any]
val a2 = [1, 2 as Any] // 编译错误：会隐藏对 1 的装箱
```

`Array` / `MutableArray` 完整 API 是标准库内容，不在本文定义。

## 13. Range 与 Progression 语法

Scoop 采用 Kotlin 风格 range/progression 语法：

```kotlin
a..b
a until b
a downTo b
a step n
```

语言固定：

- `a..b` 是 inclusive range 表达式。
- `until` 表示 half-open range。
- `downTo` / `step` 构造降序或指定步长 progression。
- 这些结构可作为 `for` 的 iterable。

具体 range 类型、边界溢出规则、性能和完整 API 由标准库定义。可移植语言语义只依赖它们满足 `for` 迭代协议。
`until`、`downTo` 和 `step` 属于标准库 API，不属于本文档涵盖范围，此处仅为示例。

## 14. Struct literal、Closure 与 `do` 消歧

普通表达式位置：

```kotlin
Point { x: 1, y: 2 }             // struct literal
val f = { println("hello") }     // closure
val u = do { println("hello") }  // local block
```

规则：

- `TypeName { field: expr, ... }` 是 struct literal。
- 裸 `{ ... }` 是 closure。
- `do { ... }` 是立即执行的局部 block。
- `val a = { println("hello") }` 总是把 closure 赋给 `a`。
- 若要把 block 求值结果赋给 `a`，写 `val a = do { ... }`。
- `combine(1) { ... } { ... }` 是一次调用加多个 trailing lambda。
- 后续 `do { ... }` 不会被并入前一调用，除非写入调用括号中。

## 15. Operator overloading

Scoop 支持 Kotlin 风格 operator overloading，通过约定函数名解析：

| 运算符 | 函数名 |
|---|---|
| `+` | `plus` |
| `-` | `minus` |
| `*` | `times` |
| `/` | `div` |
| `%` | `rem` |
| `&` | `and` |
| `|` | `or` |
| `^` | `xor` |
| `~` | `inv` |
| `<<` | `shl` |
| `>>` | `shr` |
| `a[i]` | `get` |
| `a[i] = v` | `set` |
| ordering | `compareTo` |

规则：

- 内建标量运算由语言/编译器固定。
- 用户类型运算符通过成员或 extension 函数解析。
- 是否要求显式 `operator` 修饰符由实现阶段决定；本规范只固定语义映射。

## 16. Class literal

（暂定规范，不要求实现）

Class literal 写作：

```kotlin
String::class
MyType::class
```

规则：

- `::class` 左侧必须是类型名路径。
- 在编译期上下文中，class literal 可作为类型元信息或稳定类型名常量使用。
- 在注解参数中，class literal 是合法编译期常量表达式。
- 运行期完整 reflection 不由 class literal 引入；动态字段访问、动态方法调用、动态实例创建不属于 Scoop 0.1 语言能力。

## 17. 类型推断

Scoop 使用 constraint-based bidirectional type inference，不使用 Hindley-Milner Algorithm W。

设计原则：

- 函数签名是 API 边界，显式类型提高文档性并支持 Cone 分发。
- 函数体是实现细节，局部推断尽量减少样板。

注解规则：

| 位置 | 是否必须 |
|---|---|
| 函数参数类型 | 必须 |
| Public 函数返回类型 | 必须 |
| Private/internal 非递归函数返回类型 | 可推断 |
| 递归函数返回类型 | 必须 |
| 局部 `val` / `var` | 可推断 |
| Lambda 参数类型 | 通常可由期望类型推断 |
| 泛型类型实参 | 通常可推断 |
| Public 函数 effects | 必须，除非省略表示 `Pure` |
| Private/internal 函数 effects | 可推断 |
| 顶层简单 `val` / `var` | 当前语言边界要求显式类型 |
| 顶层解构 `val pattern` | 可从 initializer 推断整体类型 |

### 17.1 局部变量推断

```kotlin
val x = 42
val name = "hello"
val pair = (1, "x")
```

### 17.2 Lambda 参数推断

```kotlin
val adults = users.filter { user -> user.age >= 18 }
val names = users.map { it.name }
val comparator: (User, User) -> Int = { a, b -> a.age - b.age }
```

参数类型从期望函数类型向下传播。

### 17.3 泛型实参推断

```kotlin
fun <T> id(x: T): T = x

val a = id(1)       // T = Int
val b = id("text")  // T = String
```

泛型实参由实参类型、期望返回类型、receiver 类型和 `where` 约束共同决定。

### 17.4 Return type 推断

Private/internal 非递归函数可省略返回类型：

```kotlin
internal fun greet(name: String) {
    if (name.isEmpty()) return "Hello!"
    return f"Hello, {name}!"
}
```

Public 函数和递归函数必须写返回类型。

### 17.5 LUB 与分支合并

分支表达式类型不同时，编译器计算 least upper bound：

```kotlin
open class Animal
class Dog : Animal
class Cat : Animal

val pet = if (cond) Dog() else Cat() // Animal
```

规则：

- 引用类型按继承/接口计算 LUB。
- 值类型若无公共可用上界，必须显式转换或装箱。
- `Nothing` 可合并到任何目标类型。

### 17.6 推断算法概念

编译器概念上执行：

1. 遍历 AST，为表达式创建类型变量。
2. 从语法产生约束。
3. 根据赋值、调用、分支、lambda 期望类型、泛型约束求解。
4. 对未解类型变量、冲突约束和 ambiguous overload 报错。

Effect 推断见第 4 部分。
