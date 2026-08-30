# Scoop 语言规范

版本：0.4（草案）

## 1. 概述

Scoop 是一门静态类型、编译到原生代码（LLVM 后端）的编程语言。其语法以 Kotlin 的**核心语法**为基础，目标是：

- 尽量兼容 Kotlin 核心语法（类、接口、函数、泛型、控制流、协程等），不追求逐条全兼容；
- 排除一切与 JVM / JS / Kotlin Multiplatform 平台对接相关的扩展；
- 引入真正的**值类型**（struct / enum / tuple），并围绕值类型扩展解构与 `when` 的能力；
- 用 `Option<T>` 取代平台式的空指针语义；
- 提供基于核心库实现的字符串插值。

实现策略：Scoop 的泛型采用**单态化（monomorphization）**实例化，不依赖运行期类型擦除。

### 1.1 范围

本规范只覆盖：

- Scoop 的核心语法；
- 支撑核心语法所需的**最小核心库**（见第 11 章）。

标准库的其他部分（集合框架、IO、并发工具、序列库等）不在本规范范围内。

---

## 2. 与 Kotlin 的关系

### 2.1 包含的 Kotlin 核心语法

除本规范明确修改或排除的部分外，Scoop 尽量兼容 Kotlin 核心语法，包括但不限于：

- 声明：`class` / `interface` / `object` / `companion object` / `typealias` / 属性（`val` / `var`，含委托属性）/ 函数 / 扩展函数与扩展属性；
- 类特性：主构造函数与次构造函数、`init` 块、继承（单继承 + 接口实现）、抽象类、可见性修饰符（`public` / `internal` / `private` / `protected`）；
- 数据与函数：局部函数、lambda、匿名函数、函数类型、callable reference、默认参数、命名参数、可变参数（`vararg`）、中缀函数（`infix`）、运算符重载、尾递归（`tailrec`）；
- 泛型：类型参数、`in` / `out` 型变、类型投影、上界约束、`where` 子句；
- 控制流：`if` / `when` / `for` / `while` / `do-while`、区间与迭代、`break` / `continue` / `return`（含标签）、异常（`try` / `catch` / `finally` / `throw`）；
- 空安全运算符：`?.` / `?:` / `!!`（语义见第 7 章）；
- 类型运算符：`is` / `!is` / `as` / `as?`、智能转换（smart cast，见 2.3）；
- 协程：`suspend` 函数与挂起调用（见 8.2）；
- 上下文参数：context parameters（见 8.3）；
- 注解与反射以外的元编程语法（反射仅保留语法级支持，见 2.2）。

### 2.2 排除的内容

以下 Kotlin 功能**不属于** Scoop：

- 平台对接：
  - `expect` / `actual`（multiplatform）；
  - `@JvmStatic` / `@JvmField` / `@JvmOverloads` / `@Throws` 等一切 JVM 互操作注解；
  - `external` 声明（FFI 通过 `@Extern` 注解机制提供，见第 13 章）；
  - Java 互操作语义（SAM 转换、平台类型 `T!` 等）。
- 运行期反射：`KClass`、`::class` 的运行期反射 API、`kotlin.reflect` 体系（单态化泛型 + 无 JVM 运行期使其不成立）。
- `inline class` / `value class`（Kotlin 的 JVM 值类）：被 Scoop 的原生值类型（第 4 章）取代。
- `data class`：被 `struct`（4.1）取代；解构等能力由 4.6 的内建解构提供。
- Kotlin 的 `enum class`：被 Scoop 的 `enum`（第 4.2 节）取代。
- 空指针语义：`null` 字面量与 `T?` 的运行期 null 表示（见第 7 章）。
- 普通字符串的 `$` 插值：只有 f-string 支持插值（见第 6 章）。

### 2.3 语义调整

- 引用相等 `===` / `!==` 只适用于引用类型（见 4.4.2）。
- 智能转换（smart cast）仅用于 `is` / `!is` 类型检查；对 `Option<T>` 不提供"判非空后收窄"式的智能转换（见第 7 章）。

---

## 3. 类型系统总览

Scoop 的类型分为两大类：

- **引用类型（reference type）**：`class`、`interface`、`object`、函数类型、数组类型（第 10 章）等。其值是指向堆对象的 managed **ref value**；对象具有 identity，复制 ref value 只复制引用而不复制对象。
- **值类型（value type）**：`struct`、`enum`、`tuple`。无 identity，immutable，复制时复制完整值（实现可自行优化为内联或间接 ABI，语义上不可观察）。

### 3.1 顶层与底层类型

- `Any`：所有类型（引用类型与值类型）的根类型。值类型向上转型为引用类型时发生**装箱**（见 4.4.4）。`Any` **没有任何成员方法**：值相等走 `==` 的运算符决议（见 11.11），字符串化与哈希是独立的接口（`ToString` / `Hash`，见 11.11）——不把 `equals` / `hashCode` / `toString` 挂在类型根上（那是 Java 的遗迹）。
- `Nothing`：所有类型的子类型，无实例。值类型可以向下转型到 `Nothing`（实际上不可达，仅类型系统规则）。

### 3.2 泛型

- 泛型在编译期**单态化**实例化：每个具体类型实参生成一份专门的代码。
- 泛型调用与泛型值构造的类型实参由整组实参共同约束，推导结果不得依赖实参声明顺序。依赖期望类型的实参（如 `None`、空数组或嵌套泛型构造）可以由任意其他实参先绑定类型参数后再完成检查；运行期求值顺序仍严格保持源码顺序。
- 因此不存在类型擦除，也没有 `reified` 的运行期需求（见 8.4）。
- 型变规则（`in` / `out` / 投影）与 Kotlin 一致，在编译期检查；`Array<T>` 例外（见 10.4）。
- 声明点型变目前用于 interface 类型参数：不写修饰符表示不变，`out T` 表示协变，`in T` 表示逆变。对同一 interface 的两个应用，协变参数按同向子类型关系比较，逆变参数按反向子类型关系比较，不变参数必须相等；不同 interface 之间不存在由型变产生的子类型关系。
- interface 声明必须满足型变位置约束：方法返回类型是协变位置，方法参数类型是逆变位置；进入 `out` 类型实参保持位置，进入 `in` 类型实参反转位置，进入不变类型实参则要求参数不在该类型中出现。`out` 参数不得出现在逆变或不变位置，`in` 参数不得出现在协变或不变位置。违反约束是编译错误。
- 类型投影（使用点 `in` / `out`）沿用 Kotlin 语义，但不属于 M10 前置补齐范围；在其语法落地前，源码只能使用声明点型变。
- 除类型上界外，类型参数还可以用 `value` / `ref` 约束限定为值类型或引用类型（见 13.9）。
- **带类型参数的成员函数不参与虚分派**（类似 Rust 的 object safety）：单态化实例无法枚举，泛型成员函数不进入 vtable / itable。interface 可以声明泛型方法，但只能由具体 class / struct / enum 上同型的泛型方法实现；经 interface 静态类型调用它是编译错误。class 的泛型方法必须为 final（实现 interface 时写作 `final override`），值类型方法本来即为 final；因此所有合法调用都能静态确定实现并使用直接分派。
- 泛型宿主上的泛型成员函数同时拥有两组类型参数：宿主类型实参由接收者静态类型确定，方法类型实参由整组调用实参推导；两组实参共同组成该方法的单态化身份，顺序固定为“宿主类型实参在前、方法类型实参在后”。方法类型参数不得与宿主类型参数重名。

### 3.3 参数传递、receiver 与 `this`

Scoop 的源码函数/方法调用一律是 **pass-by-value**：每个实参表达式求值一次，再以所得值初始化 callee 中不可重新绑定的形参 binding。这条规则不按 value type / reference type 分叉：

- 实参为 value type 时，被复制的是完整值；callee 不获得调用方 binding/place 的别名。
- 实参为 reference type 时，被复制的是 ref value（当前实现中是一个 managed pointer-sized value），不是其指向的对象。调用方与 callee 因此持有两份相等的引用值，指向同一具有 identity 的对象；重新绑定任一引用 binding 不影响另一份引用，但通过它们对 referent 所做的合法修改对另一方可见。这与 Java 的引用参数语义一致，不是 pass-by-reference。

成员方法、计算属性 getter 及扩展函数的 receiver 使用同一规则。调用 `receiver.method(args...)` 时，receiver 只求值一次，并按值初始化隐含的、不可重新绑定的 `this: T` 参数；`this` 不是调用方 receiver binding 的别名。因此：

- value-type `this` 是 receiver 完整值的 method-local copy；值类型字段不可写，`this` 自身也不能被重新绑定。
- ref-type `this` 是 receiver ref value 的 method-local copy；它不能被重新绑定，但可以按 referent 类型的普通成员规则读写同一对象。
- 方法返回的 closure 若引用 `this`，按 8.1.3 捕获的就是这个隐含参数的值：value-type `this` 复制完整值，ref-type `this` 复制引用值。两者都不保留调用方 receiver binding/place。
- 经装箱值的 interface 分派调用值类型实现时，dispatch thunk 语义上用 box payload 的值初始化 value-type `this`；box identity 不会成为该 `this` 的 identity。

实现可以在不可观察时消除复制，或按目标 ABI 把大型 value 间接传递，但 IR 语义仍必须是参数值而不是调用方 place。当 value receiver 直接或间接含 `@InteriorMutable` value、出现 `addressOf(this)`，或其他 unsafe 能力可能观察存储时，必须物化独立的 method-local copy；所得地址不得指向调用方 binding 或 box payload，且有效期不超过当前 method activation。

---

## 4. 值类型

### 4.1 struct

`struct` 定义一个聚合值类型。

```
struct S(val f1: Int, val f2: String = "")
```

规则：

- 字段在主构造函数中声明，与 Kotlin 类的构造函数属性语法一致，但**只能用 `val`，不允许 `var`**（struct 不可变，`var` 字段无意义）。
- **字段不支持可见性修饰符，全部对外可见（public）**。
- **immutable**：struct 实例构造完成后不可修改。
- 支持主构造函数与次构造函数（secondary constructor），与 Kotlin 类相同。
- **不支持 `init` 块**。所有初始化逻辑必须位于构造函数中。
- 不支持继承其他类型，但可以实现 interface（见 4.4.3）。
- struct 没有 body 内可变状态；body 中可以声明成员函数、伴生对象（companion object 本身是引用类型的单例）与计算属性。

#### 4.1.1 构造

struct 只能通过构造函数构造：

```
val s1 = S(10)                 // 主构造函数，默认值可省略
val s2 = S(f1 = 10, f2 = "x")  // 命名参数
```

不提供 `S { f1: 10, ... }` 形式的 struct 字面量（该形式与尾随 lambda 存在解析歧义，已排除）。

#### 4.1.2 派生行为

struct 自动获得：

- 结构相等：`==` 按字段逐一比较（**条件派生**——仅当全部字段可比较时可用，见 11.11）；
- `toString()`：按字段生成（编译器派生的 `ToString` 实现，见 11.11）；**不**自动获得哈希——`Hash` 是 opt-in 接口（见 11.11）；
- 解构（见 4.6）：可按字段顺序或按字段名解构；
- 副本更新表达式（见 4.5）。

### 4.2 enum

`enum` 定义一个带负载的代数数据类型（类似 Rust 的 enum），取代 Kotlin 的 `enum class`。

```
enum E {
    SimpleVariant,
    VariantWithValue(Int),
    CompositedVariant(Int, String),
    VariantWithNamedField {
        f1: Int,
        f2: String
    },
    VariantWithDefaults(val f1: Int, val f2: String = "hello")
}
```

规则：

- 变体（variant）形式：
  - 单元变体：`SimpleVariant`；
  - 位置参数变体：`VariantWithValue(Int)`，可携带多个值；
  - 命名字段变体：`VariantWithNamedField { f1: Int, f2: String }`；
  - 构造函数式命名字段变体：`VariantWithDefaults(val f1: Int, val f2: String = "hello")`，**规则同 struct 的主构造函数**：字段可带默认值，构造时可用命名参数。
- 两种命名字段形式（block 式与构造函数式）语义等价；构造函数式额外支持默认值。
- **变体字段不支持可见性修饰符，全部对外可见（public）**。
- **enum 自身不支持构造函数、不支持 `init` 块、不支持成员属性**；body 中只允许声明成员函数与伴生对象（变体的构造函数式声明是变体定义的一部分，不在此限）。
- 每个变体是一个构造器：`E.SimpleVariant`、`E.VariantWithValue(42)`、`E.VariantWithNamedField(f1 = 1, f2 = "x")`、`E.VariantWithDefaults(1)`、`E.VariantWithDefaults(f1 = 1)`。命名字段变体（含 block 式）一律以命名参数方式构造，不提供花括号构造形式（与 struct 字面量同样存在解析歧义）。
- **变体不是类型**：不能用作 `is` 的检查目标、变量类型或参数类型；判断与提取负载通过 `when` 模式（第 5 章）完成。
- 与 Kotlin enum class 的 entries 类似，变体名可以通过 `import some.package.E.*` 引入后不写前缀直接使用；`scoop.core.Option.*` 由核心库默认引入（见第 7 章），因此在上下文能确定类型时可以直接写 `Some(...)` 和 `None`。
- 在 `when` 匹配处，变体名可以省略 `E.` 前缀（见第 5 章）。
- 与 struct 一样：immutable、无 identity、可自动派生结构相等（条件派生，见 11.11）与 `toString()`（`ToString` 派生实现）。
- 命名字段变体的字段构造后只读。
- enum 可以实现 interface（见 4.4.3）。
- 泛型 enum 允许，例如核心库的 `enum Option<T>`（见 7.2）。

### 4.3 tuple

tuple 是匿名的积类型，无需声明。

```
val t1 = (1, "hello")                       // 类型自动推断为 (Int, String)
val t2: (Int, Float, String) = (42, 4.2, "world")
val u1: Unit = ()                           // Unit（0 元 tuple）字面量
val u2 = Unit                               // Unit 既是类型名也是该值的构造器
val s1 = (42,)                              // 1 元 tuple，类型 (Int,)
```

规则：

- tuple 类型记作 `(T1, T2, ...)`，tuple 字面量记作 `(e1, e2, ...)`。
- **0 元 tuple 写作 `()`**；其类型名为 `Unit`，`Unit` 既是类型名也是该值的构造器，`()` 与 `Unit` 等价。
- **1 元 tuple 必须写作 `(e,)`**（尾随逗号），类型记作 `(T,)`；`(e)` 是带括号的表达式 `e` 本身。消歧汇总：`()` = Unit；`(e)` = 括号表达式；`(e,)` = 1 元 tuple；`(e1, e2, ...)` = 多元 tuple。
- 元素通过解构（见 4.6）或位置访问：`val (a, b) = t1`、`t1._1`、`t1._2`（位置访问从 `_1` 开始）。
- tuple 是值类型：immutable、无 identity、结构相等。
- tuple 不支持实现 interface、不支持命名字段；需要命名请使用 struct。

### 4.4 值类型通用规则

以下规则适用于 struct、enum、tuple 三者。

#### 4.4.1 不可变性

值类型实例构造后不可修改。修改的唯一方式是通过副本更新表达式创建新值（见 4.5），或把新值重新绑定到 `var` 变量（这是重绑定，不是原地修改）。

#### 4.4.2 无 identity

值类型没有 identity：

- 对值类型使用 `===` / `!==` 是**编译错误**；
- 相等判断一律使用 `==`（结构相等）。

值类型装箱为引用类型后，其装箱结果按引用类型规则处理（见 4.4.4）。

#### 4.4.3 实现 interface

值类型可以实现 interface，但**不得因此获得可变性**：

- 实现 interface 方法必须使用 `override`；值类型方法始终为 final，不参与 vtable 分派，但装箱为 interface 后通过该 interface 的 itable 分派；
- 实现 interface 的成员函数、属性 getter 不得修改 `this` 的任何字段（值类型字段本来就不可写，此规则是自然推论）；
- 若 interface 契约要求可变行为（例如要求实现 `var` 属性的 setter），值类型实现它是**编译错误**。

```
interface Describable {
    fun describe(): String
}

struct Point(val x: Int, val y: Int) : Describable {
    override fun describe(): String = f"(${x}, ${y})"
}
```

#### 4.4.4 装箱与引用类型

- 值类型向上转型为**任何引用类型**（`Any`、它实现的 interface 等）时，自动**装箱**为堆上的引用对象（类似 Java 的 `int` → `Integer`）。
- 装箱后的对象：具有 identity（可用 `===` 比较）、immutable、`==` 仍为结构相等。
- **auto-boxing 只发生在 O(1) 场景**：单个值的转换（赋值/初始化、函数实参、返回值等单点转换）允许自动装箱；数组字面量的元素位置等批量场景不做自动装箱，需要显式 `as`（见 10.3）。
- `is` / `as` / `as?` 可用于判断与取回装箱前的值类型；`as?` 失败时返回 `None`（见第 7 章）。
- 值类型在类型系统上也是 `Nothing` 的父类型（可向下转型，语义不可达）。

### 4.5 副本更新表达式

副本更新表达式基于一个既有值创建"修改了部分字段"的新值：

```
struct S(val f1: Int, val f2: Int)

val s1 = S(10, 20)
val s2 = s1.{ f2: 30 }    // 新值：f1 = 10, f2 = 30
var s3 = S(1, 2)
s3 = s3.{ f1: 84 }        // var 重绑定，不是原地修改
```

规则：

- 形式：`baseExpr.{ field: expr, ... }`。`.` 之后只能出现标识符的既有语法使得 `{` 在该位置无歧义，因此不需要引入额外的关键字。
- `baseExpr` 必须是值类型（struct、enum 的命名字段变体值；tuple 不支持，因为 tuple 无字段名）。
- 块内用 `:` 给字段赋新值；未提及的字段从 `baseExpr` 拷贝。
- 结果为**新值**，`baseExpr` 本身不变。
- 块内只能给目标类型的字段赋值，不能引入其他语句（块内是字段绑定列表而非任意代码块）。
- 对 enum：只能用于命名字段变体的值，且不能改变变体种类；静态类型含多个命名字段变体时按所写字段解析目标变体，运行期值不属于该变体时抛出异常（具体异常类型由实现定义）。
- 不支持穿透 `Option` 的写法（`opt?.{ f: 1 }`）：先用 `when` 拆包，再做副本更新。

### 4.6 解构声明（destructuring）

`val` / `var` 声明中可以使用解构模式一次绑定多个变量：

```
struct S(val f1: Int, val f2: String)
val s = S(10, "hello")
val (n, _) = s                           // struct 按字段顺序解构，_ 跳过

val t = (1, 2, 3)
val (x, y, z) = t                        // tuple 按位置解构

val t2 = (4, 5, s)
val (a, b, { f1, f2: renamedF2 }) = t2   // 解构可以嵌套
```

规则：

- **tuple 模式** `(p1, p2, ...)`：按位置解构；每个位置可以是绑定名、`_`（跳过单个元素）、`..`（见下）或嵌套模式。不使用 `..` 时，模式的元数必须与被解构值的元数/字段数一致，否则编译错误。
- **字段模式** `{ f1, f2: renamed, ... }`：按字段名解构 struct（或命名字段的 enum 变体值）；写 `f1` 绑定同名变量，`f2: renamed` 绑定到 `renamed`；`_` 不用于字段模式。**未列出全部字段时必须以 `..` 结尾，否则是编译错误**（与 Rust 一致）。字段模式可带类型名前缀（`S { f1: x, .. }`），语义相同，前缀用于提高可读性或在必要时消除歧义。
- struct 既可按位置（`(n, _)`，按主构造函数字段顺序）也可按字段名（`{ f1, f2 }`）解构。
- 解构可任意嵌套：内层可以是 tuple 模式或字段模式。
- struct / tuple 的解构是语言内建能力，不依赖 `componentN`。
- **同样的模式规则适用于一切"赋值"位置**：除 `val` / `var` 声明外，还包括 `for` 循环变量（`for ((a, b) in pairs) { ... }`）与 lambda 参数（`{ (a, b) -> ... }`）。这些位置的语法消歧（例如 lambda 参数列表与单参数括号形式的区分）只允许在 parsing 阶段施加限制，对后续语义分析没有影响。

#### `..` 忽略其余字段

`..` 用于忽略连续的一组或所有其余字段（规则同 Rust）：

```
val t = (1, 2, 3, 4)
val (x, .., y) = t       // x = 1, y = 4
val (a, ..) = t          // a = 1

struct S(val f1: Int, val f2: Int, val f3: Int, val f4: Int)
val s = S(1, 2, 3, 4)
val S { f1: x, f2: y, .. } = s
```

- 在 tuple / 位置模式中：`..` 忽略任意数量（含 0 个）的连续元素，可出现在开头、中间或结尾，但**至多出现一次**（出现两次会产生歧义，是编译错误）；使用 `..` 时不要求模式元数与值的元数一致。
- 在字段模式中：`..` 只能出现在最后，表示忽略所有未列出的字段；**未列出全部字段时必须写 `..`**（列出全部字段时 `..` 可省略）。
- `..` 自身不绑定任何值。

#### `..` 与区间运算符的消歧

`..` 同时是区间运算符（`1..4`，见 11.8）。两者不会歧义：

- rest 模式只出现在**模式位置**（解构声明、when 模式分支、for 循环变量、lambda 参数）；
- 区间运算符只出现在**表达式位置**（普通表达式、when 的 `in 1..4` 分支等）；
- 模式位置的元素位只接受绑定名、字面量、`_`、`..`、嵌套模式，区间表达式本来就不是合法的模式元素。parser 在模式位置看到裸 `..` 即 rest；看到 `expr .. expr` 形式则报"模式中不允许区间表达式"。

---

## 5. `when` 表达式扩展

Kotlin 原有的 `when` 语法（等值匹配、类型匹配、区间、条件分支）原样保留。以下扩展对 **enum、tuple 与 struct** 生效，其模式的形式与功能同 4.6 的解构声明完全对齐。

`if`、`when` 与 `try` 都是表达式，也可在结果被丢弃的语句位置使用。作为值使用时，每个可正常结束的分支块以最后一个表达式的值作为该分支结果；空块或以非表达式语句结束的块结果为 `Unit`，以 `return` / `throw` 结束的路径不参与结果类型合并。外层有期望类型时，每个正常分支结果必须是其子类型；否则取所有正常分支结果的唯一可表达最小上界，存在多个不可比较的最小共同上界时退化为 `Any`。依赖期望类型的分支结果可由其他分支先确定类型，分支检查顺序不影响结果。

值位置的 `if` 必须有 `else`；结果被丢弃时可以省略。值位置的 `when` 必须穷尽。`try` 的结果由正常完成的 try body 与各 catch body 共同决定；`finally` 的结果值始终丢弃，但其中的 `return` / `throw` 仍按第 8 章覆盖先前路径。

**统一规则**：仅当 subject 的静态类型是 enum / struct / tuple 时，`when` 才按模式匹配解析（下称**模式 when**）；其余 `when` 一律保持 Kotlin 的表达式语义——分支条件是普通表达式（等值比较）、`is` 检查、`in` 区间等，穷尽性也遵循 Kotlin 自身规则（表达式形式须穷尽，语句形式不强制）。两种解释的适用 subject 类型不相交，因此同一分支写法不会产生二义结果：parser 在分支条件位置同时接受模式语法与表达式语法，由语义分析按 subject 类型裁定。

模式 when 适用两条全局规则：

- **穷尽性**：模式 when（无论作为语句还是表达式）必须穷尽；编译器无法静态确认穷尽时是编译错误，此时应加上 `else` 分支兜底。带守卫的分支不计入穷尽性（编译器将其视为可能不匹配）。
- **绑定优先**：分支模式中的裸标识符一律是新的绑定，而不是对既有变量的引用；要匹配既有常量，需写字面量或限定名（如 `SomeObject.CONST`）。enum 匹配中未解析为变体名的裸标识符是匹配一切的通配绑定（效果等同 `else`），编译器应给出告警。

### 5.1 enum 变体模式

```
val v: E = ...

when (v) {
    SimpleVariant -> print("simple")
    VariantWithValue(n) if (n > 42) -> print(n)   // 解构 + if 守卫
    VariantWithValue(n) -> print(n)               // 上一分支的回退
    CompositedVariant(_, s) -> print(s)           // _ 省略不关心的字段
    VariantWithNamedField { f1, f2: renamedF2 } -> {
        print(f1)                                 // 按名绑定
        print(renamedF2)                          // 字段重命名绑定
    }
}
```

规则：

- 匹配对象是 enum 值时，分支条件可以是**变体模式**：
  - 单元变体：直接写变体名；
  - 位置参数变体：`Variant(p1, p2, ...)`，参数位可以是绑定名、字面量（等值匹配）、`_`（通配）、`..`（忽略其余，规则见 4.6）或嵌套模式；
  - 命名字段变体：`Variant { f1, f2: renamed, ... }`；写 `f1` 表示绑定同名变量，`f2: renamed` 表示绑定到 `renamed`；**未列出全部字段时必须以 `..` 结尾，否则是编译错误**；`_` 不能用于命名字段模式。
- 变体名可省略 enum 类型前缀（`E.`），编译器按被匹配值的类型解析；存在歧义时需写全限定名。
- 用变体模式匹配 enum 时，穷尽性要求覆盖全部变体（或提供 `else`）。

### 5.2 tuple 模式

```
val t = (1, 2, "hello")

when (t) {
    (x, _, s) if (x == 42) -> { print(x); print(s) }
    else -> { /* ... */ }
}
```

规则：

- 匹配对象是 tuple 时，分支条件可以是 **tuple 模式** `(p1, p2, ...)`；元素位规则同 4.6 的解构（绑定名、字面量、`_`、`..`、嵌套模式）。
- tuple 模式要求 subject 的静态类型就是 tuple；tuple 没有类型名，不能像 struct 模式那样作为类型测试使用。
- 不使用 `..` 时，模式的元数必须与被匹配 tuple 的元数一致，否则编译错误。
- tuple 无变体之分：一个元数一致、各位置均为绑定名或 `_`、且无守卫的 tuple 模式天然穷尽，不需要 `else`；含字面量或守卫时，按全局穷尽性规则处理（通常需要 `else` 兜底）。

### 5.3 struct 模式

```
struct S(val f1: Int, val f2: Int, val f3: Int)

val s = S(1, 2, 3)

when (s) {
    S { f1: 0, .. } -> print("f1 is zero")              // 字面量匹配 + ..
    S { f1, f2: y, .. } if (f1 == y) -> print("equal")  // 绑定 + 重命名 + 守卫
    S { f1, .. } -> print(f1)                           // 兜底，穷尽
}

when (s) {
    (x, 0, _) -> print("positional")   // 位置模式同样可用
    (x, ..) -> print(x)
}
```

规则：

- struct 模式的形式与 4.6 的解构声明完全对齐：字段模式（可带类型名前缀）或位置模式；元素位可以是绑定名、字面量（等值匹配）、`_`、`..` 或嵌套模式，与 enum 变体模式、tuple 模式一致。
- 字段模式未列出全部字段时必须以 `..` 结尾（同 4.6，与 Rust 一致）。
- 被匹配值的静态类型就是该 struct 时，类型名前缀可省略（`(x, ..)` 或 `{ f1, .. }` 均可）；各位置均为绑定名 / `_` / `..` 且无守卫的 struct 模式天然穷尽，不需要 `else`。
- struct 模式要求 subject 的静态类型就是该 struct（统一规则）。对父类型（`Any`、interface、装箱后的值等）的类型判断使用 Kotlin 的 `is` 分支（`is S -> ...`），智能转换后可在分支体内按 4.6 解构。

### 5.4 守卫（guard）

- enum 变体模式、tuple 模式与 struct 模式之后都可以跟 `if (condition)`；条件中可以使用该模式绑定的变量。
- 守卫为假时继续匹配后续分支。

---

## 6. 字符串插值

Scoop 的字符串插值是语言特性，由核心库的 `StringBuilder`（见 11.6）实现。

### 6.1 语法

```
val n = 42
val s = f"the number is ${n}"
val s1 = f"""this is a multi-line string,
this is the second line and the number is ${n+1}"""
```

- `f"..."`：单行插值字符串；
- `f"""..."""`：多行插值字符串（保留换行，与 Kotlin raw string 规则一致，允许 `${...}`）；
- `${expr}`：任意表达式；`$name` 不允许，必须带花括号（避免歧义，统一脱糖规则）。
- `$` 后紧跟 identifier-start 时按非法 `$name` 诊断并提示 `${name}`；`${` 以外的其他 `$` 是普通字面字符。单行 f-string 中 `\$` 同样产生字面 `$`；raw multiline f-string 不处理反斜杠转义。
- **只有 f-string 支持插值**。普通字符串（`"..."`、`"""..."""`）中 `$` 是普通字符，不触发任何插值或脱糖——这与 Kotlin 不同，迁移 Kotlin 代码时需注意。

### 6.2 脱糖

插值字符串脱糖为 `StringBuilder` 的链式调用：

```
val n = 42
val s = StringBuilder().add("the number is ").add(n).build()
val s1 = StringBuilder().add("""this is a multi-line string,
this is the second line and the number is """).add(n + 1).build()
```

规则：

- 脱糖在编译早期完成；`${...}` 中的表达式按普通代码类型检查。
- `add` 对实现 `ToString` 的类型可用（见 11.6 与 11.11），插入其 `toString()` 结果。

---

## 7. 可空性与 `Option`

Scoop 没有运行期 `null`。可空语义由核心库的 `Option<T>` 表达。

### 7.1 `T?` 的脱糖

类型 `T?` 是 `Option<T>` 的语法糖，编译期脱糖：

```
val a: String? = ...        // 即 Option<String>
```

注意：**`T??` 不是 `T?`**。脱糖是逐层的：`T??` = `Option<Option<T>>`，与 `Option<T>` 是不同的类型，`Some(None)` 与 `None` 可区分。这与 Kotlin（`T??` ≡ `T?`）不一致。

### 7.2 核心库定义

```
enum Option<T> {
    Some(T),
    None
}
```

`Option<T>` 是普通 enum，享有第 4.2 节与第 5 章的全部能力（解构、穷尽性检查等）。`scoop.core.Option.*` 由核心库默认引入，因此在上下文能确定类型时可以直接写 `Some(...)` 与 `None`，无需 `Option.None` 这样的前缀。

### 7.3 空安全运算符的语义

- `a?.b`：脱糖为 `when (a) { Some(v) -> Some(v.b); None -> None }`（结果类型为 `Option<B>`，其中 `B` 是 `b` 的类型）。
- `a ?: b`：脱糖为 `when (a) { Some(v) -> v; None -> b }`。
- `a!!`：`a` 为 `Some` 时取值；为 `None` 时抛出核心库异常 `UnwrapException`。
- 不支持 `a?.b = c` 形式的赋值：`a?.b` 的结果是临时 `Option`，对其赋值无意义。
- `null` 字面量不存在；表示"无值"使用 `None`。
- 取值的常规方式是 `when` 解构：`when (s) { Some(v) -> ...; None -> ... }`。**对 `Option<T>` 不提供智能转换**：`isSome()` 之类的判断不会收窄类型（此类收窄的语义存在隐蔽问题，暂不提供；后续如引入会单独修订本节）。

### 7.4 `Option` 的布局保证（niche 优化）

对具有**天然空位（niche）**的类型 `T`，编译器必须保证 `Option<T>` 不增加额外存储：`None` 编码为该空位，`Some(v)` 与 `v` 的表示相同。这是语言的固定特性，而非可选优化。适用类型：

- **引用类型**：空位是全 0 的机器字（GC 不会产生该引用值）；
- **`Ptr<U>` 与 `FunPtr<F>`**：空位是 `_rawPointer == 0u`（null 指针）。

因此 `Option<Ptr<T>>` / `Option<FunPtr<F>>` 可以直接出现在 C ABI 边界上表示可空指针（见 13.10），`Option` 包裹的引用类型与裸引用布局相同。对其他类型 `Option<T>` 带 tag，具体布局不保证。

---

## 8. 函数

函数语法整体与 Kotlin 一致：默认参数、命名参数、`vararg`、扩展函数、中缀调用、运算符重载、lambda 与尾随 lambda、函数类型 `(A, B) -> R` 等。

- 非 `Unit` 函数的块体必须在所有可达路径上返回值或以 `throw` 退出；无法静态证明时是编译错误。顺序语句从前向后分析，路径遇到 `return` / `throw` 后不再落空；`if` 必须有 `else` 且两支都不落空；穷尽的 `when` 必须每个可执行分支都不落空。循环在没有更强证明时按可能落空处理。
- `try` 的正常路径及每个 `catch` 路径都不落空时，整个 `try` 不落空；如果 `finally` 自身在所有路径上 `return` 或 `throw`，则它覆盖先前待执行的返回、异常或正常继续路径，整个 `try` 不落空。否则 `finally` 不改变先前路径是否落空。

### 8.1 函数类型、函数值与 closure

#### 8.1.1 函数类型

普通函数类型写作 `(P1, P2, ...) -> R`，挂起函数类型写作 `suspend (P1, P2, ...) -> R`。零参数函数必须写作 `() -> R`；参数名、缺省值与 `vararg` 不是函数类型身份的一部分。

- 函数类型是**引用类型**。函数值可以携带捕获环境，具有 identity，由 GC 管理；它不是一个裸代码地址。
- 函数类型的身份由挂起性、参数个数、每个参数类型和返回类型共同决定。普通与挂起函数类型之间不存在子类型关系或隐式转换。
- 同为普通函数类型或同为挂起函数类型时，参数类型逆变、返回类型协变：若 `A2 <: A1` 且 `R1 <: R2`，则 `(A1) -> R1 <: (A2) -> R2`。多参数逐项应用同一规则。
- 赋值、传参或 `as` 把函数值适配到不同但兼容的函数类型时，实现可以产生一个 forwarding adapter 函数值；因此适配后结果与来源不保证 `===`。对函数类型的 `is` / `as` 仍按上述结构化子类型关系判断，不能退化成只比较参数/返回类型完全相等。
- 函数类型不保留形参名、缺省值或 `vararg` 调用约定。经函数值调用时只能按位置提供与类型元数相同的实参；声明中的 `vararg T` 在函数类型中表现为其实际参数类型 `Array<T>`。
- 函数值以 `f(args...)` 调用；`f.invoke(args...)` 是同一操作的显式写法。若 `f` 是挂起函数类型，调用点必须处于 8.2 允许的挂起上下文。
- 函数值不定义结构相等。可以用 `===` / `!==` 观察同一已保存函数值的引用 identity，但规范不保证对同一函数重复创建 callable reference 或无捕获 lambda 时得到相同 identity。

Scoop 当前不提供 Kotlin 的 receiver function type（`A.(B) -> R`）；扩展函数的 callable reference 按 8.1.4 显式表示为普通函数类型。

#### 8.1.2 lambda 与匿名函数

lambda 写作 `{ parameters -> body }`，挂起 lambda 写作 `suspend { parameters -> body }`。匿名函数写作 `fun(parameters)[: R] { body }`，挂起匿名函数写作 `suspend fun(parameters)[: R] { body }`，其中方括号表示返回类型可省略并由 body 推导。四种形式都会产生函数值，也都可以捕获外层词法环境。

- 有期望函数类型时，lambda 的参数类型可省略，由期望类型给出；无期望类型时，每个显式参数都必须写出类型。单参数 lambda 在期望元数为 1 且省略参数列表时隐式声明 `it`；无参数 lambda 使用 `{ body }`。
- lambda 参数支持 4.6 的解构模式。解构失败不产生运行期分支：参数静态类型必须能按该模式解构，否则是编译错误。
- lambda 的值是 body 最后一个表达式的值；期望返回 `Unit` 时最后一个表达式的值被丢弃。匿名函数使用普通函数的返回规则。
- lambda 中的裸 `return` 是编译错误。Scoop 不提供 Kotlin inline lambda 的 non-local return；需要提前返回时应使用匿名函数，其 `return` 只返回该匿名函数。
- `suspend` 必须显式写在 lambda 或匿名函数上；期望类型不会把普通 lambda 静默改为挂起 lambda。创建或保存挂起函数值本身不会挂起，只有调用其 body 时才检查挂起上下文。

lambda 参与重载决议时，每个候选先提供自己的期望函数类型，再据此检查参数与 body。lambda 的返回表达式可以验证候选是否适用，但不作为“仅按 lambda 返回类型选择重载”的额外优先规则；若完成既有重载优先级比较后仍有多个候选，则调用有歧义。

#### 8.1.3 捕获与局部函数

- closure、匿名函数与局部函数只允许捕获外层 `val`、参数、接收者 `this` 及其他**不可重新绑定**的 binding；对外层 `var` 的读取或写入都是编译错误。Scoop 已有显式 `val`，因此本规则不额外引入 Java 式“effectively final”流分析：需要捕获时应先显式绑定为 `val`。
- `this` 是 3.3 定义的隐含不可变参数，与显式参数使用同一条按值捕获规则：value-type `this` 复制完整值，ref-type `this` 复制 ref value。两者都不保留调用方 receiver binding/place；复制 ref value 不会复制其指向的对象。
- 上述“外层 `var`”特指 callable body 中作为自由变量引用的外层**词法局部 binding**。顶层/object 属性按其 global/singleton storage 访问；class 可变属性则通过被捕获的 `this` 或其他显式引用对象访问，二者都不会把局部 `var` 隐式提升成 cell。
- capture 一律按值发生在函数值创建时：引用类型复制引用，值类型复制完整值并直接内联保存于 closure environment，不为 captured value type 隐式生成 identity-bearing box、shared cell 或 `Any` 装箱。
- captured `val` 本身不能重新绑定；若其值是带可变状态的引用对象，仍可按该对象公开的普通成员规则修改 referent。需要多个 closure 共享可变状态时，程序必须显式捕获这样的引用对象；编译器不把局部 `var` 隐式提升成引用对象。
- capture 会把复制进 environment 的引用和值延长到 closure 不再可达；其中嵌套的 managed 引用由 GC 按字段类型递归扫描。
- 没有捕获的实现允许复用静态单例；实现也可以把不逃逸的 closure 消除或栈上展开，但这些优化不得改变 identity 被观察时的结果。

局部命名函数与普通函数使用相同的声明语法，并按上述规则捕获外层不可变 binding。局部函数名在其自身 body 及声明之后的词法作用域可见，因此允许直接递归；声明之前不可见，互相递归不能依赖后声明函数的前向可见性。局部函数可以直接调用，也可以通过 `::name` 取得函数值。

局部 generic 函数的直接调用按 3.2 单态化。取得其 callable reference 时，所有类型参数必须能由期望函数类型唯一确定；否则是编译错误，不存在“仍然 generic 的函数值”。

本版本不提供隐式 reference capture、`move` capture 或 capture list。以后增加新的 capture mode 时必须使用显式语法，并单独规定 lifetime、identity、并发和成本模型；不得静默放宽本节的 `var` 禁令。

#### 8.1.4 callable reference

以下表达式创建 managed 函数值：

- `::name`：引用可见的顶层函数或局部函数；
- `receiver::member`：创建绑定接收者的成员函数引用，`receiver` 在创建时求值且只求值一次；
- `::extension`：创建未绑定扩展函数引用，其函数类型把扩展接收者作为第一个普通参数；`receiver::extension` 则创建绑定形式。

若目标是 virtual / interface 成员，绑定引用在每次调用时仍按已保存接收者做动态分派，不能在创建时固定为当时的具体实现。重载目标由期望函数类型与普通重载规则共同确定；没有期望类型时，只有唯一的非 generic 候选才能自行确定函数类型。

`receiver::member` 的 receiver 是创建点的普通表达式；其中读取某个局部 `var` 会立即取得当前值，随后 closure 不可变地保存该 receiver。这是源码显式的创建时快照，不是 callable body 对该 `var` 的自由变量捕获。

Scoop 当前不提供 `Type::member` 的未绑定成员引用或构造函数引用。函数声明名出现在普通值位置时不会隐式变成函数值，必须显式写 `::`；直接写 `name(args...)` 仍走命名调用与重载决议。

#### 8.1.5 与 `FunPtr` 的边界

managed 函数类型与 `FunPtr<F>`（13.10）是不同类别的值：前者是可捕获、可经 GC 移动的引用对象，后者是 GC-free 的原生代码指针值。二者没有一般性的子类型关系、转换或相同调用 ABI。

唯一的互操作是 13.10 定义的**期望类型驱动转换**：在明确需要 `FunPtr<F>` 的位置，满足约束的顶层命名函数引用 `::name` 可以直接生成原生 callback 地址。该规则不先创建 managed closure，也不允许把任意 lambda、匿名函数、局部函数、绑定引用或已存在的函数值转换为 `FunPtr`。

### 8.2 `suspend` 函数

- `suspend` 修饰的函数是协程挂起函数，只能在另一个 `suspend` 函数或编译器认可的协程构建器内调用。`main` 本身必须是普通函数；从普通代码启动挂起计算使用 11.9 的 `startCoroutine` 或标准库构建器。
- 下列**声明自身拥有的运行期初始化上下文**都是非挂起上下文，不得包含挂起调用：顶层 `val` / `var` 的 initializer 与 delegate 表达式；`object` / `companion object` 的属性 initializer、delegate 表达式、`init` 块及基类/接口委托初始化；class 的属性 initializer、delegate 表达式、`init` 块、主/次构造函数体及构造委托；struct/enum 变体及构造函数的缺省表达式。全局初始化入口必须在进入 `main` 前同步完成；单例或实例初始化必须在对象可用前同步完成；它们都不能返回 `Suspended`、发布部分初始化对象或保存“尚未完成的初始化”。
- **求值归属按词法位置确定，而不是按最外层构造语法确定**：在 suspend 函数中显式写出的调用实参仍处于调用者的挂起上下文，因此 `C(awaitValue())` 合法——`awaitValue()` 先完成，随后普通构造过程同步执行；构造函数定义处的缺省表达式和构造体内部则仍为非挂起上下文。把 suspend lambda / `SuspendTask` 对象保存进字段也不等于执行它，其函数体在以后实际调用时按自身的挂起性检查。
- 属性没有隐式挂起能力：普通 getter / setter、计算属性，以及委托属性的 `getValue` / `setValue` 协议必须是非挂起 callable；即使属性读取发生在 suspend 函数中，也不能通过普通属性访问暗中挂起。未来若引入 suspend property，必须另行定义语法、类型与调用规则。
- `const val` 的约束更强：它没有运行期初始化过程，只允许 9.1.2 定义的编译期常量表达式，因此不允许任何普通或挂起函数调用。
- 挂起性是 callable 签名的一部分：override / interface 实现的挂起性必须与被覆写声明完全一致；`suspend (A) -> R` 与普通函数类型 `(A) -> R` 不兼容。挂起性不作为同名声明的重载区分项，参数列表相同而只相差 `suspend` 的两个函数是重复声明。
- 调用挂起函数可能在当前调用栈内立即产生 `R`，也可能保存当前计算并返回到协程启动者，随后经 `Continuation` 恢复。无论采用哪条路径，源码都只观察到一次普通的 `R` 结果或一次在该调用点抛出的异常；挂起本身不是返回、异常或 `finally` 的退出原因。
- 调用点之前已经完成的实参和子表达式只求值一次；恢复后从调用点之后继续，源码从左到右求值顺序不变。跨挂起点仍存活的局部变量、参数及待执行的控制转移必须被保留。
- `try` / `catch` / `finally` 的语义跨挂起点保持不变：恢复失败等价于在原挂起调用点 `throw`；仅仅挂起不会执行 `finally`；当计算随后正常返回、抛出或由 `finally` 覆盖退出时，`finally` 仍恰好执行一次。
- `Continuation` 是单次完成协议：一个挂起点只能由 `resume` 或 `resumeWithException` 中的一个成功完成一次；编译器生成的 continuation 对重复完成抛出 `IllegalStateException`。本规范不定义协程取消；放弃且永不恢复一个 continuation 不会隐式执行 `finally`。
- 现阶段不支持 suspend FFI：挂起函数不得带 `@Extern`，也不得转换为 `FunPtr`；M10 的 hidden continuation ABI 只用于编译器生成的 Scoop 托管调用，不是任何 FFI ABI。具体约束见 13.4、13.10 与 14.2。
- 具体的协程构建器（`launch`、`async` 等）、调度器与取消策略属于标准库，不在最小核心库范围内。最小核心库只提供 11.9 的启动、挂起与恢复原语。

### 8.3 上下文参数（context parameters）

- 支持 Kotlin 的 context parameters 语法：`context(ctx: Ctx)` 声明函数所需的隐式上下文，调用处由编译器从作用域解析。
- 上下文参数参与重载决议与类型推断，规则与 Kotlin 一致。

### 8.4 `inline` / `crossinline` / `noinline` / `reified`

这些关键字**仅出于 Kotlin 兼容性保留，无实际作用**：

- Scoop 泛型是单态化的，类型信息在每个实例化处天然可用，不需要 `reified` 来保留类型实参；
- 编译器自行决定内联策略，`inline` / `noinline` / `crossinline` 被接受但可忽略（允许作为提示，但不保证语义）；
- 不依赖语义内联的 Kotlin 代码可以保留这些关键字而无需修改；依赖 inline lambda non-local return 的代码不兼容，必须按 8.1.2 改为匿名函数或重写控制流。

### 8.5 默认参数值

- 默认参数值表达式在**定义处完成解析**（名称解析、类型检查、可见性检查都在定义处的上下文中进行），在**调用处求值**：每次调用缺省该参数时，表达式在调用点重新求值并生成代码。
- 这与单态化一致：缺省表达式可能依赖类型参数（如 `fun <T> f(x: T, y: Array<T> = [])`），只有在调用处实例化时才能确定其具体代码。
- 推论：
  - 每次调用都重新求值，副作用随调用重复（与 Kotlin 一致；不同于 Python 的定义时一次求值）。
  - 求值顺序：按参数声明顺序，缺省表达式在其参数位置上与显式实参交错求值，全部求值完成后进入函数体；缺省表达式可以引用在它之前声明的参数。
  - 可见性按定义处检查：缺省表达式可以引用定义方的 `private` / `internal` 符号，即使代码在调用处生成；这些符号必须随 `.slib` 以可链接的形式导出（见 12.5），缺省表达式的代码体也因此进入 `.slib` metadata（与导出泛型同理）。
  - 修改一个 Cone 导出函数的缺省值会改变其 `.slib` metadata，下游 Cone 随之重编译（见 12.5）。
  - `suspend` 函数的缺省表达式可以包含挂起调用：调用 `suspend` 函数本来就要求调用处具备挂起上下文，因此无额外限制。
  - 普通函数与构造函数的缺省表达式不能包含挂起调用。构造函数默认参数仍遵循本节其他“定义处解析、调用处求值”的规则，但构造协议本身是同步的；suspend caller 显式写出的构造实参不受此限制（见 8.2）。
  - 典型应用：`getCurrentSourceLocation()` 作为缺省参数实现廉价的诊断/tracing（见 11.12）。

---

## 9. 其他语法要点

### 9.1 类与对象

- `class` / `abstract class` / `interface` / `object` / `companion object`：与 Kotlin 一致，均为引用类型。
- 类可以实现 interface，可以继承一个类；struct/enum 不能被继承，也不能继承类。
- 类与成员方法默认均为 `final`。只有 `open` / `abstract` 类可以被继承；类可被继承不代表其方法自动可覆写，普通基类方法必须显式声明为 `open fun` 才能首次被覆写。
- `abstract fun` 隐含 `open` 且没有函数体，只能声明在 `abstract class` 或 interface 中。覆写类或 interface 方法必须写 `override`；与 Kotlin 一致，`override fun` 默认继续保持 `open`，可用 `final override fun` 终止后续覆写，也可用 `open override fun` 显式强调继续开放。
- final 方法不得被覆写。静态接收者上已知的 final 方法调用使用直接分派；open / abstract 类方法调用使用 vtable 分派，interface 方法调用使用 itable 分派。final override 仍替换继承来的 vtable 槽，以保证经基类引用调用时到达该实现。
- `sealed`：`sealed class` / `sealed interface` 保留（引用类型的受限继承）；值类型的等价物直接使用 `enum`。

#### 9.1.1 对象与属性初始化

- 顶层属性在程序进入 `main` 前初始化完成；`object` / `companion object` 的初始化时机可以晚于程序启动，但触发初始化的访问必须等到属性 initializer、delegate 表达式、`init` 块及继承/委托初始化全部同步完成后才能取得该单例。初始化的精确顺序与循环初始化诊断另行规定，不影响 8.2 的“初始化过程不可挂起”规则。
- class 实例只在基类构造、构造委托、属性 initializer、delegate 表达式、`init` 块及构造函数体全部同步完成后才构造成功。struct 与 enum 值的构造同样同步完成。
- 初始化期间可以调用普通函数，也可以构造一个尚未执行的 suspend task；不能直接等待挂起结果。`startCoroutine` 是同步返回的普通 builder，因此类型系统不把调用它本身视为初始化器挂起；它启动的计算不是该属性或对象尚未完成的初始化步骤。
- 普通属性访问始终同步。需要异步/挂起地取得一个值时必须显式暴露 `suspend fun`，不能藏在 getter 或属性委托协议中。

#### 9.1.2 `const val`

- `const val` 只允许声明在顶层、`object` 或 `companion object` 中；其类型必须是 `Boolean`、基本数值类型、`Char` 或 `String`。
- initializer 必须是编译期常量表达式：字面量、对其他 `const val` 的引用，以及由它们组成且可在编译期确定结果的内建一元/二元运算。const 依赖图存在循环是编译错误。
- 函数/方法调用、构造、普通属性读取、数组或其他对象分配、`throw` 及挂起调用都不是常量表达式。`const val` 不生成需要在程序启动或单例首次访问时执行的 runtime initializer；位于 `object` / `companion object` 中的 `const val` 引用本身不触发单例初始化。
- 导出的 `const val` 的类型和值属于 `.slib` HIR metadata，下游 Cone 在编译期直接消费；值变化会使下游编译缓存失效。是否同时保留可寻址存储属于 ABI/`addressOf` 设计，不改变其“无 runtime initializer”的语义。

### 9.2 委托

- 类委托（`class C : I by impl`）与属性委托（`by lazy { ... }` 等）保留，语义与 Kotlin 一致。
- 值类型不支持类委托中的可变状态要求（同 4.4.3）。

### 9.3 运算符重载与约定

`+`/`-`/比较/索引/迭代/`invoke` 等运算符约定与 Kotlin 一致。解构约定：普通 class 可通过 `componentN` 运算符函数支持解构（与 Kotlin 一致）；struct / tuple 走内建解构（见 4.6）。

### 9.4 注解

支持自定义注解与注解使用语法；不内置平台相关注解。注解不携带运行期反射能力（见 2.2），编译期处理（如编译器插件）由实现定义。语言核心注解（`@Intrinsic` / `@NoGC` / `@Extern` 等）见第 13 章。

---

## 10. 数组

Scoop 内置两个数组类型（引用类型，属于核心库）：

- `class Array<T>`：不可变数组（长度固定，元素不可写）；
- `class MutableArray<T>`：可变数组（长度固定，元素可写）。

### 10.1 值类型元素的内存保证

当 `T` 是值类型（struct / enum / tuple / 基本类型）时，`Array<T>` 与 `MutableArray<T>` 保证：

- 元素**不装箱**；
- 元素在内存中**连续布局**（在满足 pack/align 约束的前提下）。

当 `T` 是引用类型时，数组存储引用。

### 10.2 数组字面量

```
val a = [1, 2, 3]                       // Array<Int>
val m: MutableArray<Int> = [1, 2, 3]    // MutableArray<Int>
```

- `[v1, v2, ...]` 是数组字面量；它具体构造 `Array` 还是 `MutableArray` 由**上下文推导**（类型标注、参数类型等）。
- 当上下文同时允许二者（例如无显式类型标注）时，使用 `Array`。
- 空数组字面量 `[]` 必须有显式的类型上下文，否则编译错误。

### 10.3 数组字面量的类型推导与检查

- **无显式类型上下文**（如 `val v = [v1, v2, v3]`）：
  - 若任意元素是值类型，则其余所有元素的类型必须与之**完全相同**，否则是编译错误（值类型元素之间不做隐式向上合流，以保证 10.1 的内存布局保证成立）。
  - 若所有元素都是引用类型，则元素类型取全部元素类型的**最小上界（LOB）**，数组类型为 `Array<LOB>`；当前类型系统没有交叉类型，若存在多个互不可比较的最小共同上界，则 LOB 取 `Any`。当 LOB 为 `Any` 时编译器应给出警告（属实现细节，可延后实现）。
- **有显式类型上下文**：每个元素的类型必须是上下文元素类型的子类型。

```
interface I
class A(val f: Int) : I

val arrayOfA = [A(10), A(20)]        // Array<A>
val i: I = A(10)                     // 引用类型 auto upcast
val arrayOfI: Array<I> = [A(10), i]  // 每个元素都是 I 的子类型
```

- **值类型元素不参与 auto-boxing**（auto-boxing 仅限 O(1) 场景，见 4.4.4；数组字面量不是 O(1) 场景）：

```
struct S(val f: Int) : I

val j: I = S(10)                         // O(1) 场景，auto-boxing
// val bad: Array<I> = [j, S(10)]        // 编译错误：数组字面量中不 auto-box
val good: Array<I> = [j, S(10) as I]     // 显式装箱
```

### 10.4 型变与转换

- `Array<T>` 与 `MutableArray<T>` 对 `T` **不变（invariant）**：即使 `T` is-a `S`，`Array<T>` 与 `Array<S>` 之间也不能自动 cast（不协变、不逆变）。这是 10.1 内存保证的直接推论。
- `Array<T>` 与 `MutableArray<T>` 之间**没有父子类型关系**，互转必须显式进行：
  - `m.toArray(): Array<T>`、`a.toMutableArray(): MutableArray<T>`；
  - 或以对方为参数的构造函数：`Array(m)`、`MutableArray(a)`。
- 互转**执行一次真正的复制（memcpy）**，结果是与原数组相互独立的快照：之后对原数组的修改不影响转换结果，反之亦然。
  - 不能像 Rust 那样转交（move）内存块：Scoop 没有 move 语义，转交意味着清空原 `MutableArray`，与引用语义冲突。
  - 也不能仅改写对象头复用原存储：在 LLVM + GC 的实现中，每个引用类型对象的头中带有 TypeDescriptor（类似 vpointer），就地改写它会破坏 GC 的状态，因此转换必须分配新对象并复制数据。

### 10.5 操作

- 下标访问 `a[i]`；`MutableArray` 支持下标赋值 `m[i] = v`。
- `size` 属性；实现 `Iterable<T>`，可用于 `for` 循环。

---

## 11. 最小核心库

核心库只包含核心语法运行所必需的类型与函数。命名空间为 **`scoop.core`**（默认导入 `scoop.core.*`，以及 `scoop.core.Option.*`，见 7.2）。

**核心库中的类除单独标明外均为 `final`**，不可继承（包括 `Array` / `MutableArray` / `String` / `StringBuilder` 等）。

### 11.1 类型层级根

- `Any`：所有类型的根。**没有任何成员方法**（见 3.1 与 11.11）。
- `Nothing`：所有类型的子类型，无实例。

### 11.2 基本类型

均为值类型（struct 语义）：

- `Boolean`；
- 定宽整数：`Int8` / `Int16` / `Int32` / `Int64` 与 `UInt8` / `UInt16` / `UInt32` / `UInt64`（对应 C stdint 的定宽类型）；
- Kotlin 风格的整数名称：`Byte` / `Short` / `Int` / `Long` / `UByte` / `UShort` / `UInt` / `ULong`。其中 `Byte` ≡ `Int8`、`Short` ≡ `Int16`、`Long` ≡ `Int64`（无符号同理）；`Int` / `UInt` 的宽度由目标平台决定，但**编译器必须保证 `Int` 与 `Int32` 或 `Int64` 之一完全等价**（`UInt` 同理），并对每个目标平台明确该对应关系——FFI 场景中据此确定 C 类型映射；
- 浮点：`Float`（f32）/ `Double`（f64）；
- `Char`。

字面量、算术/位运算/比较运算与 Kotlin 一致。

### 11.3 `Unit`

0 元 tuple 的类型名与值构造器（见 4.3）。

### 11.4 `String`

- 引用类型，immutable，UTF-8 语义（编码细节由实现定义）。
- 支持 `+` 拼接、索引/切片、`length`（或 `size`）、比较等核心操作。
- `toString()` 返回自身（`ToString` 的恒等实现）。

### 11.5 `Option<T>`

```
enum Option<T> {
    Some(T),
    None
}
```

见第 7 章。`scoop.core.Option.*` 默认引入。

### 11.6 `StringBuilder`

字符串插值（第 6 章）的脱糖目标：

```
class StringBuilder {
    fun add(part: String): StringBuilder
    fun <T : ToString> add(part: T): StringBuilder   // 插入 part.toString()
    fun build(): String
}
```

也可由用户代码直接使用。

### 11.7 异常

- `Throwable`（引用类型，可被 `throw` / `catch`）及其最小子类：
  - `Exception(message: String?)`：通用异常基类；
  - `UnwrapException`：`!!` 失败时抛出（见 7.3）；
  - `ClassCastException`：`as` 失败时抛出；
  - `ArithmeticException`：整数除零等算术错误；
  - `IndexOutOfBoundsException`：数组下标越界（见 10.5）；
  - `IllegalStateException`：运行期状态协议被破坏；核心实现至少用它报告 continuation 的重复完成。
- `try` / `catch` / `finally` / `throw` 语法与 Kotlin 一致。多个 `catch` 按声明顺序匹配；前一个 `catch` 的类型是后一个的父类型（含相等）时，后者不可达，是编译错误。
- `throw` 与 `catch` 的类型必须是 `Throwable` 的子类型。未捕获的异常导致进程终止（默认行为：打印异常类型名后 abort）。

### 11.8 迭代与区间

`for` 循环、区间表达式的最小支撑：

```
interface Iterator<out T> {
    fun next(): Option<T>
}

interface Iterable<out T> {
    operator fun iterator(): Iterator<T>
}
```

`next()` 直接返回 `Option<T>`：有元素返回 `Some(v)`，耗尽返回 `None`。相比 `hasNext()` + `next()` 的双方法协议，这避免了 `hasNext()` 必须缓存下一个元素的问题。`T` 只出现在返回位置，因此 `Iterator` 与 `Iterable` 都是协变的（`out T`）。

`for (v in s)` 脱糖为（变量名仅作示意）：

```
val __it = s.iterator()
while (true) {
    when (__it.next()) {
        Some(v) -> { /* for 循环体 */ }
        None -> break
    }
}
```

- 循环体中的 `break` / `continue` 语义不变（`continue` 即进入下一轮 `next()`）。
- 循环变量的解构模式（`for ((a, b) in s)`，见 4.6）同样脱糖进 `Some(...)` 分支的模式位置。

区间：`IntRange` / `LongRange` / `CharRange`（`..` / `until` / `downTo` / `step`），实现 `Iterable`。`..` 与 rest 模式的消歧见 4.6。

### 11.9 协程原语

支撑 `suspend` 语义的最小 core 形态如下：

```
interface Continuation<in T> {
    fun resume(value: T)
    fun resumeWithException(exception: Throwable)
}

interface SuspendTask<out T> {
    suspend fun run(): T
}

interface SuspendRegistration<out T> {
    fun register(continuation: Continuation<T>)
}

fun <T> startCoroutine(task: SuspendTask<T>, completion: Continuation<T>)

fun <T> startCoroutine(
    task: suspend () -> T,
    completion: Continuation<T>
)

suspend fun <T> suspendCoroutine(
    registration: SuspendRegistration<T>
): T

suspend fun <T> suspendCoroutine(
    registration: (Continuation<T>) -> Unit
): T
```

- `startCoroutine` 是最小协程构建器：启动 `task.run()` 后立即返回 `Unit`。若 task 在启动调用内完成，则返回前调用 `completion.resume(value)` 或 `completion.resumeWithException(exception)`；若 task 挂起，则在最终完成时调用。completion 恰好收到一次完成通知。
- `suspendCoroutine` 调用 `registration.register(continuation)`。registration 可以同步恢复 continuation，也可以保存它并在 `register` 返回后恢复；前者使 `suspendCoroutine` 在当前调用栈内继续，后者使其真正挂起。`register` 在尚未完成 continuation 时抛出的异常等价于 `suspendCoroutine` 在调用点抛出该异常。
- `register` 已同步完成 continuation 后又抛出属于状态协议错误，`suspendCoroutine` 以 `IllegalStateException` 失败；该 continuation 随即失效，之后不能再次成功完成。
- `SuspendTask` / `SuspendRegistration` 是不依赖 lambda 与函数引用的最小协议。函数类型 overload 由 core 中的普通 Scoop 适配器包装为这两个 interface 后调用同一底层原语，不另建 continuation 状态机；两种入口具有完全相同的同步完成、真实挂起、异常与单次完成语义。
- `launch`、`async`、dispatcher 等高层 API 可以在这些原语上由标准库提供，但不得改变 8.2 的单次完成与异常语义。
- core 原语不提供队列、线程切换或事件循环；调度器与取消不属于核心库。

### 11.10 数组

`Array<T>` 与 `MutableArray<T>`，见第 10 章。

### 11.11 相等、字符串化与哈希约定

- **`===` / `!==`（引用相等）**：identity 比较，仅适用于引用类型，不可重载（见 4.4.2）。
- **`==` / `!=`（值相等）** 的决议规则：
  - **值类型：条件派生的结构相等**——当且仅当类型的所有字段（元素）**可比较**时，编译器派生逐字段比较。字段可比较指：值类型字段递归可比较、`String`（库提供的比较）、或其类型具有可用的 equals 运算符（如 class 按本条下款定义了 `==`）。任一字段不可比较时，对该类型使用 `==` 是编译错误（诊断指出不可比较的字段）。
  - **其他类型（class、interface、`Any` 等引用类型）：解析为该类型的 `equals` 运算符方法**（`operator fun`，见 9.3 的运算符约定）；不存在时是编译错误——**没有缺省实现**（与 `+` 等其他运算符一致：漏定义/漏引入在编译期暴露，而不是被缺省语义静默掩盖）。
  - equals 的决议**只考虑成员函数**（含编译器派生）；扩展函数不得参与 `==`——任何类型在任何 context 下 `==` 的语义唯一，不存在"换个 import 就换语义"或"缺省实现抢先于更合适的实现"。
  - 类型作者显式定义 equals 运算符时优先于编译器派生。
- **`ToString`（字符串化）**：接口 `interface ToString { fun toString(): String }`。值类型由编译器按字段派生实现（struct 见 4.1.2）；其他类型 opt-in 实现。`print` / `println` 的目标形态是 `fun <T : ToString> print(v: T)`（单态化静态分发；泛型上界见 2.1/3.2，落地排期见 ROADMAP）。
- **`Hash`（哈希）**：接口 `interface Hash { fun hash(): Int }`。**没有任何缺省实现**（缺省哈希大概率语义错误）；基本类型与 `String` 由核心库提供实现，其他类型 opt-in 实现。struct 不再自动获得哈希（4.1.2 的历史承诺已废止）。

### 11.12 `SourceLocation` 与位置 intrinsic

```
struct SourceLocation(
    val file: String,
    val line: Int,
    val column: Int,
    val functionName: String,
    val typeName: String        // 不在任何类型内部时为空串
)

@Intrinsic("current_source_location")
fun getCurrentSourceLocation(): SourceLocation
```

- `getCurrentSourceLocation()` 返回其**求值点**的源码位置（文件、行、列）与所处函数、类型的名称。该信息在编译期已知，不依赖调试信息。
- 典型用法是与 8.5 的缺省参数规则组合，在调试信息与诊断设施落地前提供廉价的 runtime diagnostic / tracing 机制：

```
fun trace(msg: String, loc: SourceLocation = getCurrentSourceLocation()) {
    // loc 是调用 trace 的位置，而非本函数内部
}
```

- 出现在缺省参数表达式中时，按 8.5 在调用处求值，因此返回**最外层调用处**的位置。多层函数转发时，每一层都必须以缺省参数继续转发 `loc`（`fun warn(msg: String, loc: SourceLocation = getCurrentSourceLocation()) = trace(msg, loc)`），否则记录的是中间层的位置。
- 内联等优化（见 8.4）不得改变其结果：结果按源码中的调用处确定，与代码生成决策无关。

---

## 12. 库与包（Cone）

### 12.1 基本概念

- Scoop 的包称为 **Cone**。每个 Cone 是文件系统上的一个独立目录，包含：
  - 元文件 **`Cone.toml`**；
  - 一个或多个 **`.scoop`** 源文件。
- Cone 是分发、依赖与编译的基本单位。

### 12.2 标识符

- 每个 Cone 有全局唯一的 identifier，参照 Maven/Gradle 的规范：`group:name:version`（如 `dev.scoop:scoop-core:0.1.0`）。
- `group` 采用反域名惯例，`version` 建议语义化版本。
- 版本解析、冲突调停（同一 group:name 的不同版本）由构建工具负责，不属于语言规范。

### 12.3 依赖关系

- Cone 可以依赖多个上游 Cone；依赖在 `Cone.toml` 中声明：

```toml
[cone]
group = "dev.example"
name = "my-lib"
version = "0.1.0"

[dependencies]
"org.foo:bar" = "1.2.3"
```

- **依赖关系必须无环**：依赖图中存在环是编译期错误。
- 只能 `import` 在 `Cone.toml` 中声明的直接依赖的内容；普通 `import` 只影响当前源文件，下游 Cone 无法经由你看到你导入的符号。
- **re-export 采用语言内建机制**（参考 Rust 的 `pub use`，而非 Gradle 的 `api`/`implementation`）：`public import` 把直接依赖中的符号再导出给自己的下游。

```
// 在 Cone A 中：
public import org.foo.bar.SomeType     // SomeType 成为 A 的导出表面的一部分

// 下游 Cone B 只依赖 A 即可使用 SomeType，
// 无需在自己的 Cone.toml 中声明对 org.foo:bar 的依赖
```

- `public import` 的符号可以被下游继续 `public import`（re-export 可以成链）；但被 re-export 的符号必须来自 `Cone.toml` 中声明的直接依赖。
- `public` 在这里是上下文关键字用法，与可见性修饰符不冲突；语法上只修饰 `import` 声明。
- 钻石依赖（同一 `group:name` 的多个版本出现在依赖树中）的选取与调停是构建工具的职责，规范不做要求。
- `Cone.toml` 的具体 schema 由构建工具定义，本节只给出最小示意。

### 12.4 编译单元

- Cone 是独立的编译/链接单元。**不对每个源文件单独编译**（不像 C 的 `.c` → `.o` 方式），而是把整个 Cone 的全部源文件一起处理，生成单体编译输出（类似 Rust 的 crate）。
- 同一 Cone 内的源文件之间没有编译顺序，声明互相可见（受可见性修饰符约束）。
- Kotlin 的 `internal` 可见性在 Scoop 中定义为 **Cone 内可见**。
- 源文件中的 `package` 声明仍是 Kotlin 式的命名空间机制，与 Cone 的目录边界无强制对应关系；惯例上同一 Cone 使用统一的包名前缀（如核心库统一使用 `scoop.core`）。
- 可执行程序的入口为顶层 `main` 函数（形式与 Kotlin 一致）；纯库 Cone 不需要入口。

### 12.5 编译产物 `.slib`

泛型是单态化的（见 3.2），泛型定义必须能导出给下游 Cone、在下游完成实例化，因此 Cone 的编译输出不是纯 `.o` / `.a`，而是 **`.slib`**（类似 Rust 的 `.rlib`），包含：

- 二进制编译结果（`.o`）：已编译的非泛型代码，以及在编译本 Cone 时已产生的单态化实例；
- 导出泛型所需的 metadata：导出的类型与函数声明、泛型体的中间表示（供下游单态化）、符号表，以及各导出类型的分派表结构（vtable / itable）、TypeDescriptor 符号与类型布局（供下游建表、继承与嵌套布局）等。

下游 Cone 编译时读取上游 `.slib` 的 metadata 完成导入解析、类型检查与泛型实例化；最终链接时合并各 Cone 的 `.o`。因此：

- 上游 `.slib` 的 metadata 变化会使下游的编译缓存失效（下游需要重编译），这是单态化的自然结果；
- 符号命名（name mangling）与 ABI 细节由实现定义。

### 12.6 核心库

核心库 `scoop.core`（第 11 章）本身是一个 Cone；每个 Cone 都**隐式依赖**它，无需在 `Cone.toml` 中声明。

---

## 13. 核心注解与 FFI

以下注解类定义于 `scoop.core`，随默认导入可用。它们修饰的约束大多在编译期检查，违反即为编译错误。

本章多处使用 **GC-free** 的概念：一个类型是 GC-free 的，当且仅当其类型定义中不直接或间接包含任何 ref type（基本类型、`Ptr`、`FunPtr`、`@CLayout` struct 等都是 GC-free 的值类型）；一个函数是 GC-free 的，当且仅当其不读写、不创建任何 ref value（见 13.2）。

### 13.1 `@Intrinsic`

```
annotation class Intrinsic(val name: String)
```

- 用于 function/method：该函数是 compiler intrinsic，由编译器生成实现；**函数体必须省略**；`name` 是 intrinsic 的编译器内部标识。

```
@Intrinsic("int_add")
operator fun add(lhs: Int, rhs: Int): Int
```

- 除非有单独说明，`@Intrinsic` 不能与其他任何注解共存。
- `name` 必须是编译器内置 intrinsic 登记表中的已知标识；未知的 `name` 是编译错误（用户不能声明自定义 intrinsic）。各 intrinsic 在编译 pipeline 中的展开阶段由实现大纲规定。

### 13.2 `@NoGC`

```
annotation class NoGC
```

- 用于 function/method：指明该函数不会/不应与 GC 有任何交互——函数中不读写任何 ref value，也不创建任何 ref type 实例。
- 编译期检查；不符合约束是编译错误。
- 这样的函数可以安全地跨越 FFI boundary（例如作为 FFI 回调）。
- 该约束也意味着 `@NoGC` 的成员函数只能属于 value type：class method 有隐含的 `this` 参数，而 `this` 是 ref value。

```
@NoGC
fun add42(n: Int) = n + 42

// 编译错误：String 是 ref type，违反 NoGC 约束
@NoGC
fun addString(s: String) = s + "world"
```

### 13.3 `@Unsafe` / `@Safe`

```
annotation class Unsafe
annotation class Safe
```

- **unsafe function**：`@Extern` 函数、标注 `@Unsafe` 的函数。
- **unsafe context**：标注 `@Unsafe` 的函数体，或标注 `@Unsafe` 的 block。只有在 unsafe context 中才能调用 unsafe function；在 unsafe context 之外调用是编译错误。
- `@Safe` 用于 function 和 block，用于在 unsafe context 中重新引入 safe 约束（其中的代码回到普通检查规则）。

### 13.4 `@Extern` 与 `@CallingConvention`

```
annotation class Extern(val lib: String = "", val name: String = "", val abi: String = "c")
annotation class CallingConvention(val name: String)
```

- `@Extern` 用于 function：指明该函数是位于 `lib` 所指库中的 FFI function，符号名由 `name` 指定，`abi` 指定 ABI（见 13.8）。函数体必须省略。缺省参数的解析规则由实现定义。
- `@Extern` 与 `suspend` **互斥**：无论 `abi` 取值为何，`@Extern suspend fun` 都是编译错误。编译器不为这种声明生成 continuation 参数、`CoroutineStep` 返回值或同步/挂起 wrapper；M10 的 hidden continuation ABI 不得作为外部符号 ABI 暴露。
- `@Extern` 也可用于**全局变量**（`val` / `var`），访问库中的全局符号；全局 `var` 的约束不变（仍须带 `@Global` / `@ThreadLocal` 且 GC-free，见 13.6），注解可以组合。
- **FFI-safe 类型约束**：extern 函数的签名（参数与返回值）与 extern 变量的类型必须是 GC-free 的；出现 ref type 是编译错误。按值传递的 struct 应带 `@CLayout` 以获得确定的布局。
- 不支持 C varargs（`printf` 式可变参数）；需要时用 wrapper 函数绕行。
- `abi = "c"`（默认）的 extern 函数是 unsafe function，只能在 unsafe context 中调用（见 13.3）；`abi = "scoop"` 的 extern 函数例外，调用点不要求 unsafe context（见第 14 章）。
- `@CallingConvention` 用于 function，标明 calling convention（如 `cdecl` / `stdcall` 等，具体含义由实现确定）；可与 `@Extern` 组合使用，指定 FFI function 的 calling convention。

### 13.5 `@CLayout`

```
annotation class CLayout(val aligned: Int = 0, val packed: Int = 0)
```

- 用于 struct，指定该结构体内部成员的 align/pack 规范。
- 带有此注解的 struct，其内部不能直接或间接包含**任何** ref type，否则是编译错误。

### 13.6 `@Global` / `@ThreadLocal`

```
annotation class Global
annotation class ThreadLocal
```

- 用于**全局** `var` 声明：所有全局 `var` 必须带有 `@Global` 或 `@ThreadLocal` 之一，否则是编译错误。
- 全局 `var` 必须是 GC-free 的：其类型定义中不能直接或间接包含任何 ref type。
- 一般情况下全局状态应使用 `object` 定义；这两个注解用于明确标出特殊场景。
- 全局 `val` 没有上述限制。

### 13.7 `@InteriorMutable`

```
annotation class InteriorMutable
```

- 用于 struct：标明该 struct value 内部的值可能会被改变，不能保证 immutability。常用场景：struct 带有 atomic 字段，或 FFI 会修改传给它的 struct 参数。
- 带有此注解的 struct 及其 value 只能在 unsafe context 中使用（见 13.3）。
- 该注解不为 value 引入 identity，也不改变按值复制边界：两个 copy 仍是两份独立存储。值类型成员方法仍按 3.3 取得独立的 `this` copy，不得因 ABI 指针传递而把调用方存储暴露给方法。

### 13.8 FFI ABI

Scoop 的 FFI 函数有两种 ABI：

- **C ABI**（`abi = "c"`，默认）：标准的 FFI function，由外部 lib / so / dylib / dll 提供。它对 Scoop 的类型系统和 GC 环境没有任何了解，也不能使用相关功能，用于直接引入外部库。
- **Scoop ABI**：能识别 Scoop 的类型信息，并能与 GC 交互。一部分 runtime function 使用此 ABI。

两种 ABI 的函数在进入和离开时可能需要不同的 enter/exit sequence，具体细节由实现定义。

ref type（如 `String`、`Array`、普通 class）不能出现在 C ABI 的边界上（13.4 的 FFI-safe 约束）；它们与外部库的互操作涉及 GC，由 Scoop ABI 规定（见第 14 章）。

### 13.9 `value` / `ref` 类型约束

`value` 与 `ref` 是上下文关键字，用作泛型的 type bound：

- `T : value`：`T` 必须是值类型（struct / enum / tuple / 基本类型）；
- `T : ref`：`T` 必须是引用类型（class / interface）。

```
fun <T : value> needValue(v: T) { ... }

needValue("hello")    // 编译错误：String 不是值类型
```

- 与类型上界语法同样可用于 `where` 子句。
- `value` / `ref` 约束与类型上界互斥：同一类型参数不能同时携带两者。
- 无约束的类型参数默认接受任何类型（与 Kotlin 一致）。

### 13.10 `Ptr` 与 `FunPtr`

二者定义于 `scoop.core`，是 FFI 的基础辅助类型，均为值类型。

#### `Ptr<T>`

`Ptr` 能容纳一个 raw pointer；`addressOf` 用于取一个变量的内存地址：

```
struct Ptr<T : value>(val _rawPointer: UInt) {
    @NoGC @Unsafe
    fun toUInt(): UInt = _rawPointer

    @NoGC @Unsafe
    @Intrinsic("ptr_cast")
    fun <U : value> cast(): Ptr<U>

    @NoGC @Unsafe
    @Intrinsic("ptr_load")
    fun load(): T

    @NoGC @Unsafe
    @Intrinsic("ptr_save")
    fun store(value: T)

    @NoGC @Unsafe
    fun load(offset: Int): T            // 等价于 (this + offset).load()

    @NoGC @Unsafe
    fun store(offset: Int, value: T)    // 等价于 (this + offset).store(value)

    @NoGC @Unsafe
    operator fun plus(offset: Int): Ptr<T>

    @NoGC @Unsafe
    operator fun minus(offset: Int): Ptr<T>
}

@Unsafe
@Intrinsic("address_of")
fun <T : value> addressOf(v: T): Ptr<T>
```

- `load` / `store` / `cast` 是编译器 intrinsic，且都是 unsafe function（见 13.3）。按 13.1 的规则 `@Intrinsic` 通常不得与其他注解共存；此处是单独说明的例外：这些 intrinsic 允许与 `@NoGC` / `@Unsafe` 组合。
- `plus` / `minus` 的 `offset` 以**元素个数**计（步进 `offset * sizeOf<T>()` 字节），与 C 的指针算术一致；带 `offset` 的 `load` / `store` 以其定义。
- `addressOf` 是 intrinsic，且带 **lvalue 约束**：实参必须是参数、局部变量、全局变量，或值类型成员方法的 `this`，取的是该 place 实际存储的地址；对临时值、字面量、计算结果等非 lvalue 表达式调用是编译错误。对 `this` 取址时指向 3.3 规定的方法局部副本，不是调用方的 value 或 box payload。
- `Ptr<T>` 自身是值类型，因此满足 `value` 约束，可以出现在要求 `T : value` 的位置（包括 `Ptr<Ptr<T>>`）。
- **null 与可空指针**：`_rawPointer == 0u` 表示 null 指针；FFI 边界上的可空指针用 `Option<Ptr<T>>` 表示，布局由 niche 优化保证（见 7.4）。
- **`void*` 与 opaque 类型**：`void*` 及 C 的 opaque handle（不完全类型指针）统一用 `Ptr<Unit>` 表示。

#### `sizeOf` / `alignOf`

```
@NoGC
@Intrinsic("size_of")
fun <T : value> sizeOf(): UInt

@NoGC
@Intrinsic("align_of")
fun <T : value> alignOf(): UInt
```

- 返回 `T` 的大小 / 对齐（字节数），编译期求值；手工内存管理（配合 C 的 `malloc` / `free` 等）时使用。

#### `FunPtr<F>`

`FunPtr<F>` 声明一个 FFI 可用的 callback 指针，`F` 是 8.1 定义的**普通、非挂起且完全具体化**的函数类型：

```
struct FunPtr<F>(val _rawPointer: UInt = 0u)
```

- `_rawPointer` 存放实际的函数指针值（与 `Ptr` 同样以 `UInt` 容纳 raw pointer）。缺省构造产生 null 指针（`0u`），因此 `FunPtr` 可以声明为 struct 字段、先以 null 填充；非 null 的 `FunPtr` 只能由编译器在 callable reference 转换时生成（见下）。
- 除缺省构造（null）外，用户**不能直接构造** `FunPtr` 值；在期望 `FunPtr<F>` 的位置使用顶层函数引用 `::name`，由编译器完成上下文转换。该转换要求声明签名与 `F` **精确相同**，不应用 8.1.1 的函数类型型变。
- `F` 的每个参数与返回类型必须满足对应 extern ABI 的 FFI-safe 约束；`Unit` 只允许作为返回类型。函数类型 `F` 在这里仅描述 native signature，本身不会作为 managed 引用穿越边界。
- 可空函数指针用 `Option<FunPtr<F>>` 表示（niche 优化见 7.4）。
- 被转换的目标必须是带 `@NoGC` 的普通顶层命名函数，且**不能是 generic、挂起、extern、成员或扩展函数**。lambda、匿名函数、局部函数、任何绑定引用以及已存在的 managed 函数值都不能转换。违反这些约束是编译错误：FFI 回调不得与 GC 交互，generic 函数没有单一具体符号，挂起函数只有编译器内部的 hidden continuation ABI，而 closure 还需要原生 ABI 中不存在的 managed 环境参数。编译器不自动生成 closure 或挂起 callback wrapper。
- `FunPtr` 不提供 Scoop 侧 `invoke`；它只用于传递/存储 native callback 地址。初版 callback 契约仅允许原生方在发起 extern 调用的同一已注册线程上同步调用；保存后异步、跨线程或在 Scoop 程序退出后调用需要 14.3 的 GC-aware 注册协议，不能由 `FunPtr` 隐式获得。

```
// C 侧：int compare_int(int a, int b, int (*cmp)(int, int))

@Extern(lib = "sample", name = "compare_int")
fun compareInt(a: Int, b: Int, cmp: FunPtr<(Int, Int) -> Int>): Int

@NoGC
fun cmp(a: Int, b: Int) = if (a > b) { 1 } else { 0 }

@Unsafe
fun caller() {
    compareInt(10, 10, ::cmp)
}
```

---

## 14. Scoop ABI FFI

Scoop ABI（见 13.8）供能识别 Scoop 类型信息并与 GC 交互的外部函数使用，主要消费者是 runtime 与核心库的实现者。

### 14.1 GC 基础设施 `scoop.core.gc`

```
package scoop.core.gc

struct PinnedPtr<T : ref>(val raw: UInt64)
struct GcHandle<T : ref>(val raw: UInt64)

@Unsafe fun <T : ref> pin(v: T): PinnedPtr<T>
@Unsafe fun <T : ref> unpin(p: PinnedPtr<T>): T
@Unsafe fun <T : ref> getGcHandle(v: T): GcHandle<T>
@Unsafe fun <T : ref> releaseGcHandle(h: GcHandle<T>): T
```

- **`pin`**：将对象固定在 GC 堆上（不移动、不回收），返回 `PinnedPtr`——其 `raw` 就是对象的实际地址，pin 标志记录在对象头（见 runtime spec 3.4），可以直接交给 FFI 当裸指针使用。固定对象会影响 GC 效率且可能造成内存泄漏，固定时间应尽可能短。
- **`unpin`**：按地址清除 pin 标志并取回对象，O(1)；之后该对象可以正常参与 GC。
- **`getGcHandle`**：获取对象的 GC handle。handle 被视为对象的引用：对象存在未释放的 handle 时不会被回收，但 GC 可能在堆上移动它。一个对象可同时存在多个 handle，全部释放后才可能被回收。
- **`releaseGcHandle`**：释放 handle 并取回对象，不再阻止回收。
- `PinnedPtr` 与 `GcHandle` 是不同的类型，混用（如 `unpin` 一个 `GcHandle`）是编译错误。二者都是只含一个 `UInt64` 字段的 GC-free 值类型，ABI 与 `UInt64` 一致，可以直接出现在 FFI 签名中（13.4 的 FFI-safe 约束）。
- 取舍：短期持有并需要裸指针时用 `pin`（O(1)，但阻碍 GC 移动）；长期保活且允许移动时用 `GcHandle`。
- handle 取回对象时类型 `T` 来自 handle 的类型参数，编译器无法校验其真实性——这层正确性由 runtime 作者保证。

### 14.2 调用约定

Scoop ABI FFI 的 caller side（Scoop 托管代码一侧）必须生成 ordinary managed call 框架：

- conservative root spill；
- ordinary call site，由 statepoint rewrite 处理 safepoint；
- 不插 `enter_native` / `leave_native`；
- callee 不标记为 `gc-leaf-function`；
- machine callconv 初版使用 LLVM 默认 callconv `0`；
- 调用点本身**不要求 unsafe context**（`abi = "scoop"` 的 extern 函数不是 unsafe function，见 13.4）。

这里的 Scoop ABI 仍是普通、单次进入并在返回前完成的 FFI 调用约定，不是 8.2 所述挂起函数的 hidden continuation ABI。`abi = "scoop"` 不放宽 `@Extern` 与 `suspend` 的互斥规则，也不提供自动 continuation / callback wrapper。

注意：以上是**用 LLVM 实现时**需要的策略（LLVM GC / statepoint 体系的术语），描述的是参考实现的代码生成要求，不是语言语义本身。

### 14.3 safepoint 与 GC 语义

- Scoop ABI FFI 函数的机器码中**没有 safepoint poll**（它可能是用其他语言写的）：GC 不会在其指令流的任意位置隐式触发；GC 只能在它**显式调用 runtime**（分配对象、回调 Scoop 代码）时、在 runtime 内部发生。
- 并行 GC 实现可能由其他 thread 触发 GC；FFI 函数自身需确保正确性：跨越任何可能触发 GC 的 runtime 调用时，用 handle 引用对象（或在 pin 之后使用裸指针），不得在可能触发 GC 的调用前后持有未 pin 的裸指针。
- `abi = "scoop"` 的调用点安全的前提是**被调方遵守上述契约**。这类底层 extern 声明按约定仅由 runtime / 核心库作者使用；runtime 应校验 handle 的合法性，对非法 handle 的行为由实现定义。
- GC-aware 回调：Scoop ABI FFI 通过单独的 runtime 提供 callback Scoop closure 的功能，在 closure 中 GC 重新生效；具体机制由 runtime 定义。注意 `@NoGC` 的 `FunPtr`（13.10）不能用于这类回调——它要求的恰恰是不与 GC 交互。

### 14.4 示例

runtime 侧（C，示例而非真实代码；`scoop_runtime_*` 函数由 runtime 定义）：

```c
// 参数与返回值都是 pinned object pointer：pin 标志在对象头，对象不移动、不回收
ScoopObjectHeader *scoop_concat_string(ScoopObjectHeader *ps1, ScoopObjectHeader *ps2)
{
    // 读取 s1 / s2 的长度与数据位置（对象已 pin，直接使用裸指针）
    size_t s1_len = ...;
    size_t s2_len = ...;
    const char *s1_data = ...;
    const char *s2_data = ...;

    // 在 GC 堆上分配，分配即固定；可能触发 GC，但 ps1 / ps2 / ps_ret 均已 pin，保持有效
    ScoopObjectHeader *ps_ret = scoop_runtime_alloc_pinned(ScoopString, s1_len + s2_len);
    char *buf = ...;    // 分配完成后再计算数据位置

    memcpy(buf, s1_data, s1_len);
    memcpy(buf + s1_len, s2_data, s2_len);
    // 设置返回字符串的长度等状态 ...

    return ps_ret;    // 由 Scoop 侧 unpin
}
```

Scoop 侧：

```
// abi = "scoop" 的 extern 不是 unsafe function（13.4）
@Extern(name = "scoop_concat_string", abi = "scoop")
fun _scoop_concat_string(s1: PinnedPtr<String>, s2: PinnedPtr<String>): PinnedPtr<String>

fun scoopConcatString(s1: String, s2: String): String {
    @Unsafe {
        val ps1 = pin(s1)
        val ps2 = pin(s2)
        try {
            val pr = _scoop_concat_string(ps1, ps2)
            return unpin(pr)
        } finally {
            unpin(ps1)   // 防止异常路径泄漏 pin
            unpin(ps2)
        }
    }
}
```

- pin 与 unpin 之间必须用 `try` / `finally` 保护，否则异常路径会泄漏 pin（对象永远无法移动/回收）。
- `unpin(pr)` 直接返回 `String`（`PinnedPtr` 带类型参数），无需强转。
- 整条链路不经过 handle 表；`GcHandle` 留给需要长期保活且允许移动的场景。

---

## 15. 明确排除（再次汇总）

为方便实现者，以下内容 Scoop 不支持：

| 排除项 | 替代方案 |
|---|---|
| `enum class` | `enum`（4.2） |
| `value class` / `inline class` | `struct`（4.1） |
| `data class` | `struct`（4.1）+ 内建解构（4.6） |
| `null` / 平台类型 `T!` | `Option<T>`（第 7 章） |
| 普通字符串的 `$` 插值 | f-string（第 6 章） |
| struct 字面量 `S { f: v }` | 构造函数 / 命名参数（4.1.1） |
| `expect` / `actual` | 无（单平台） |
| JVM 互操作注解与 SAM 转换 | 无 |
| 运行期反射 | 编译期/单态化机制 |
| struct 的 `init` 块 / `var` 字段 | 构造函数内逻辑 / `val` |
| 对值类型使用 `===` | `==`（结构相等） |
| `Array<T>` 的协变/逆变 | 显式转换或重新构造（见 10.4） |
| `Option<T>` 的智能转换 | `when` 解构（7.3） |
