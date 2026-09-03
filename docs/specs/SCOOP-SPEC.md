# Scoop 语言规范

版本：0.5（草案）

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
- function、class、struct、enum与interface都可以声明类型参数。generic class/struct/enum的constructor或variant、base/interface application、字段与成员都可以使用宿主类型参数，generic interface的父interface与成员也可以使用宿主类型参数。每个fully specialized nominal application生成独立的concrete identity和成员实现；class还生成对象布局、TypeDescriptor与分派表，struct/enum生成完整value layout与GC-free/扫描信息，interface生成独立TypeDescriptor与itable key identity。
- Scoop没有预定义`Self`类型、associated type或“当前实现者类型”的隐式占位符；`Self`也不是关键字，若出现在源码中只按普通名称解析。generic/interface契约若需要表达某个类型关系，必须用显式nominal type application或显式type parameter表示，编译器不执行`Self := 实现类型`替换。
- 泛型调用与泛型值构造的类型实参由整组实参共同约束，推导结果不得依赖实参声明顺序。依赖期望类型的实参（如 `None`、空数组或嵌套泛型构造）可以由任意其他实参先绑定类型参数后再完成检查；类型检查顺序不决定运行期求值顺序，显式实参与缺省表达式严格按 8.5.3 求值。
- 调用点可以写出完整的显式类型实参：`f<Int>(value)`、`Box<String>(value)`、`Enum.Some<Int>(value)` 与 `receiver.convert<String>()`。显式列表必须覆盖 callee 自己声明的全部类型参数，不支持部分写出后继续推断；泛型宿主的方法调用只写方法自己的类型参数，宿主前缀仍由 receiver 静态类型确定。没有显式列表时继续使用上一条的整组推断规则。
- generic type application在类型位置必须携带完整类型实参；不支持裸generic type或部分应用。generic class/struct constructor及enum variant构造可以在调用位置省略显式实参并由整组构造实参和期望类型推导，但推导结束后的类型仍是完整application。
- primary/secondary constructor与enum variant constructor不声明独立type parameter；构造调用中的显式/推导实参只对应nominal host。只有普通callable可以在generic owner参数之外再拥有一组callable参数。
- 因此不存在类型擦除，也没有 `reified` 的运行期需求（见 8.4）。
- 已落地的声明点型变用于interface类型参数：不写修饰符表示不变，`out T`表示协变，`in T`表示逆变。generic class/struct/enum的类型参数现阶段全部为invariant，不能写`in`/`out`；同一nominal declaration的两个application必须具有完全相同的类型实参才是同一类型，class的普通继承关系另行判断。non-interface declaration-site variance留待后续；value type型变还必须先规定不同concrete layout之间的转换语义。`Array<T>` / `MutableArray<T>`固定不变（见10.4）。
- 对同一interface的两个application，协变参数按同向子类型关系比较，逆变参数按反向子类型关系比较，不变参数必须相等；不同interface之间不存在由型变产生的子类型关系。
- 继承与implements闭包只合并完全相同的interface application；同一template的`I<Int>`与`I<String>`始终是不同契约、不同RTTI/itable identity。一个类型可以在继承图中到达二者，但必须分别满足其成员obligation；若替换后的签名无法由普通overload/override规则同时实现，则在实现类型定义处报错，不能按template id或擦除后的文本签名任选其一。
- interface 声明必须满足型变位置约束：方法返回类型是协变位置，方法参数类型是逆变位置；进入 `out` 类型实参保持位置，进入 `in` 类型实参反转位置，进入不变类型实参则要求参数不在该类型中出现。`out` 参数不得出现在逆变或不变位置，`in` 参数不得出现在协变或不变位置。违反约束是编译错误。
- 使用点`in` / `out` projection、star projection与capture conversion尚未进入当前语言子集；源码类型位置必须使用完整type argument，不能写`C<out T>`、`C<in T>`或`C<*>`。这些能力不能简单定义为擦除：Scoop允许type argument是具有不同layout/ABI的value type，每个fully specialized application又有独立TypeDescriptor与dispatch identity。未来规范必须分别定义projected member读写规则、subtyping/推导、RTTI/cast、跨Cone metadata，并决定unboxed value application是否禁止projection或需要显式existential boxing。
- 除类型上界外，类型参数还可以用 `value` / `ref` 约束限定为值类型或引用类型（见 13.9）。
- 类型上界在参数列表中写作 `T : Interface`，或在声明头后的 `where T : Interface` 子句中给出；同一参数可以具有多个不同的 interface application 上界。上界必须是带完整类型实参的 interface，当前不接受 class或另一type parameter作为上界，也不产生可作为普通表达式类型的交叉类型。`value` / `ref` kind bound与任一interface上界互斥（见13.9）。
- 类型实参必须同时满足参数的全部上界；class/interface按继承与声明点型变判断，value type同样只按显式声明的interface实现判断。泛型推导把上界纳入整组约束求解，不得先任选一个类型、再把bound失败降为警告或回退为`Any`。
- receiver为有界type parameter时，成员候选只来自其interface上界及继承闭包；不加入`Any`成员或实际类型未在bound中声明的能力。generic template中的bound member在实例化时解析为concrete direct / virtual / interface call；单态化不需要runtime dictionary，但不取消actual concrete type本来具有的动态分派语义。
- 每个合法且实参完整的interface application都是普通reference type，可以直接作为变量、参数、返回值、字段、cast目标和upper bound；Scoop没有trait object/existential的第二种interface形态，也没有object-safety分类或相关flag。所有合法interface成员都必须具有可进入itable的完整签名，并可经concrete、interface或bounded receiver调用。
- interface方法现阶段不能声明自己的type parameter；`interface I { fun <T> f(value: T) }`在声明处即为编译错误。interface宿主可以generic，例如`interface I<T> { fun f(value: T) }`，完整application `I<String>`中的方法可正常itable分派。这是method-level generic dispatch ABI尚未定义的功能边界，不是允许声明后再限制调用形态的object-safety规则。未来开放时必须同时支持interface与bounded receiver调用。
- non-interface generic method必须non-virtual。class generic method必须语义为final；generic method不能声明为open/abstract/override，不能实现或覆盖vtable/itable slot。struct/enum方法本来即为final。它们仍是带`this`的instance method，但所有合法调用都根据receiver静态类型与完整type argument使用direct dispatch；运行期派生class不能替换目标实现。
- class/struct/enum泛型宿主上的泛型成员函数同时拥有两组类型参数：宿主类型实参由接收者静态类型确定，方法类型实参由整组调用实参推导；两组实参共同组成该方法的单态化身份，顺序固定为“宿主类型实参在前、方法类型实参在后”。两组参数使用不同的semantic identity，方法类型参数不得与宿主类型参数重名。调用点显式列表只写method自身参数，并须整组省略或整组完整写出；两组参数的bound一起验证。interface方法现阶段没有第二组参数。
- top-level、local与extension generic function，以及上述non-interface generic method，都可以使用inline upper bound与`where`。generic method的callable reference必须由期望函数类型唯一确定method全部实参，得到的是某个concrete函数值；Scoop没有first-class polymorphic function value。
- callable自身的type parameter不能写声明点`in`/`out`；variance modifier只用于规范允许的nominal type parameter。callable参数与返回类型在推导中的方向由constraint solver处理，不通过声明点variance标记函数type parameter。
- `is` / `as` / `as?`检查完整的generic application identity，不擦除type argument：例如`Box<Int>`与`Box<String>`、`I<Int>`与`I<String>`是不同的检查目标。generic body中的type parameter在单态化后引用其concrete TypeDescriptor；当前没有裸generic或star-projected检查目标。
- 单态化必须结构上保证实例化闭包终止。同一generic callable递归SCC中的每个调用环，把宿主参数与callable参数组成的完整向量代回起点后必须逐项保持identity；普通直接/互递归因此复用同一concrete实例。参数替换非identity的环属于当前不支持的polymorphic recursion，在template定义检查时报错。非递归调用边仍可任意变换实参。编译器不得用递归深度、实例数量或超时阈值决定源码是否合法。

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
- 不自动获得`ToString`或`Hash`；两者都必须在struct声明中显式adopt并实现（见11.11）；
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
- 与 struct 一样：immutable、无 identity，可条件派生结构相等；`ToString`与`Hash`必须显式adopt并实现（见11.11）。
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
- tuple 是值类型：immutable、无 identity；当全部元素可比较时条件派生结构相等，Unit无条件满足结构相等（见11.11）。
- tuple 不支持在源码中显式声明implements列表，也不支持命名字段，因此不实现`ToString`或其他普通interface。需要命名或实现interface请使用struct。

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

- interface是普通nominal reference type；任何合法且完整的interface application均可承载class ref或装箱后的value，不存在trait object转换或“该interface是否object-safe”的额外判定；
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
- 装箱后的对象具有 identity（可用 `===` 比较）且immutable。装箱不额外赋予相等能力：`==`始终按装箱后表达式的**静态引用类型**查找成员operator equals；`Any`没有该成员，未声明equals的interface也不能比较。经`as`/模式匹配取回原value后才重新使用value type的结构相等规则。
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
- `a?.f(args...)`：脱糖为 `when (a) { Some(v) -> Some(v.f(args...)); None -> None }`。receiver `a`先求值且只求值一次；只有运行期进入`Some`分支后才会求值显式实参、物化`vararg`、执行缺省表达式并进入callee。方法、extension与property-like `invoke`的选择均在payload静态类型上按第8章完成。
- 安全调用**不展平**结果。若`f`返回`Option<R>`，`a?.f()`的类型是`Option<Option<R>>`；若返回`Unit`，结果是`Option<Unit>`。这遵守7.1的逐层`Option`语义，不采用Kotlin nullable type的幂等合并。
- `a ?: b`：脱糖为 `when (a) { Some(v) -> v; None -> b }`。
- `a!!`：`a` 为 `Some` 时取值；为 `None` 时抛出核心库异常 `UnwrapException`。
- 空安全表达式可以出现在`while`条件中；条件脱糖产生的临时绑定与其他条件求值步骤一起在每次条件检查时重新执行，不能提升到循环外。
- 不支持 `a?.b = c` 形式的赋值：`a?.b` 的结果是临时 `Option`，对其赋值无意义。
- `null` 字面量不存在；表示"无值"使用 `None`。
- 取值的常规方式是 `when` 解构：`when (s) { Some(v) -> ...; None -> ... }`。**对 `Option<T>` 不提供智能转换**：`isSome()` 之类的判断不会收窄类型（此类收窄的语义存在隐蔽问题，暂不提供；后续如引入会单独修订本节）。

### 7.4 enum 布局与 `Option` niche 保证

对具有**天然空位（niche）**的类型 `T`，编译器必须保证 `Option<T>` 不增加额外存储：`None` 编码为该空位，`Some(v)` 与 `v` 的表示相同。这是语言的固定特性，而非可选优化。适用类型：

- **引用类型**：空位是全 0 的机器字（GC 不会产生该引用值）；
- **`Ptr<U>` 与 `FunPtr<F>`**：空位是 `_rawPointer == 0u`（null 指针）。

因此 `Option<Ptr<T>>` / `Option<FunPtr<F>>` 可以直接出现在 C ABI 边界上表示可空指针（见 13.10），`Option` 包裹的引用类型与裸引用布局相同。对其他类型 `Option<T>` 带 tag，具体布局不保证。

niche表示的适用范围严格限于与上述`Option<T>`同构的enum：恰有两个variant，其中一个无字段，另一个恰有一个字段，且该字段的具体类型为引用类型、`Ptr<U>`或`FunPtr<F>`；variant名称、顺序以及字段采用位置或命名形式不影响判断。除此之外的enum一律使用tagged表示，不能从其他位模式、整数范围或用户不变量推导niche。

tagged enum的表示由tag、一个可选的**pure-value共享payload区**以及若干**ref-bearing独占连续slot**组成。递归检查后完全不含managed ref的多个variant可以复用同一块共享payload空间；每个直接或间接包含managed ref的variant则必须拥有自己的连续slot，不能与其他variant重叠。共享区或独占slot都按对应variant字段的正常值布局保存其全部字段。构造tagged enum时必须先把整个值（包括共享区、所有inactive ref-bearing slot和padding）清零，再写入tag及当前variant所用的payload；复制按完整值复制。GC只需无条件检查所有ref-bearing独占slot中的固定ref位置，全0 inactive slot不形成引用，扫描不读取tag、不选择variant。`Ptr` / `FunPtr`不是managed ref；在未采用niche的tagged enum中，包含它们但不含managed ref的variant仍属于pure-value共享区。

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

#### 8.1.4 函数声明引用表达式（callable reference）

`::` 引入**函数声明引用表达式**。这是中性的源码语法：`::name` 本身既不表示 managed closure，也不表示 native code pointer。parser 只保留引用形态与名称；语义分析根据上下文的期望类型类别一次性决定其结果：

- 期望类型是普通/挂起函数类型时，表达式创建本节定义的 managed 函数值；
- 没有期望类型时，只尝试推导 managed 函数类型，绝不自动推导为 `FunPtr`；
- 期望类型是 `FunPtr<F>` 时，只有语法形态为无 receiver 的 `::name` 才进入 8.1.5、13.10 的 native-address contextual resolution；该路径不先创建 managed 函数值。

在 managed 函数类型上下文或无期望类型的推导上下文中，支持以下形式：

- `::name`：引用可见的顶层函数或局部函数；
- `receiver::member`：创建绑定接收者的成员函数引用，`receiver` 在创建时求值且只求值一次；
- `::extension`：创建未绑定扩展函数引用，其函数类型把扩展接收者作为第一个普通参数；`receiver::extension` 则创建绑定形式。

若目标是 virtual / interface 成员，绑定引用在每次调用时仍按已保存接收者做动态分派，不能在创建时固定为当时的具体实现。managed 分支中的重载目标由期望函数类型与普通重载规则共同确定；没有期望类型时，只有唯一的非 generic 候选才能自行确定函数类型，所得结果仍是 managed 函数值。

`receiver::member` 的 receiver 是创建点的普通表达式；其中读取某个局部 `var` 会立即取得当前值，随后 closure 不可变地保存该 receiver。这是源码显式的创建时快照，不是 callable body 对该 `var` 的自由变量捕获。

Scoop 当前不提供 `Type::member` 的未绑定成员引用或构造函数引用。函数声明名出现在普通值位置时不会隐式变成函数值，必须显式写 `::`；直接写 `name(args...)` 仍走命名调用与重载决议。

#### 8.1.5 与 `FunPtr` 的边界

managed 函数类型与 `FunPtr<F>`（13.10）是不同类别的值：前者是可捕获、可经 GC 移动的引用对象，后者是 GC-free 的原生代码指针值。二者没有一般性的子类型关系、转换或相同调用 ABI。

唯一共享的是 8.1.4 的 `::name` **源码语法**。在上下文期望类型明确为 `FunPtr<F>` 时，满足约束的顶层命名函数引用直接解析为原生 callback 地址；这是对函数声明引用表达式的 native-address contextual resolution，不是从 managed 函数值到 `FunPtr` 的值转换。该路径不创建 managed closure，也不允许把 lambda、匿名函数、局部函数、绑定引用或已存在的函数值转换为 `FunPtr`。

需要让 native代码在稍后或 foreign thread中调用 managed closure时，必须使用 14.3 的 callback registration协议。该协议导出的是“静态 C trampoline + GC-free opaque context/token”的组合，closure本身仍是 managed函数值并由 runtime保活；它不是到 `FunPtr` 的隐式或显式值转换，也不放宽本节规则。

```
@NoGC
fun increment(value: Int): Int = value + 1

fun references() {
    val managed: (Int) -> Int = ::increment
    val native: FunPtr<(Int) -> Int> = ::increment
    val inferred = ::increment                    // 无期望类型：managed (Int) -> Int
    val invalid: FunPtr<(Int) -> Int> = inferred // 编译错误：不能事后拆出 native 地址
}
```

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
- 现阶段不支持 suspend FFI：挂起函数不得带 `@Extern`，其声明引用也不得在 `FunPtr` 上下文中解析为原生地址；M10 的 hidden continuation ABI 只用于编译器生成的 Scoop 托管调用，不是任何 FFI ABI。具体约束见 13.4、13.10 与 14.2。
- 具体的协程构建器（`launch`、`async` 等）、调度器与取消策略属于标准库，不在最小核心库范围内。最小核心库只提供 11.9 的启动、挂起与恢复原语。

### 8.3 上下文参数（context parameters）

- 支持 Kotlin 的 context parameters 语法：`context(ctx: Ctx)` 声明函数所需的隐式上下文，调用处由编译器从作用域解析。
- 上下文参数参与重载决议与类型推断，规则与 Kotlin 一致。

### 8.4 `inline` / `crossinline` / `noinline` / `reified`

这些关键字**仅出于 Kotlin 兼容性保留，无实际作用**：

- Scoop 泛型是单态化的，类型信息在每个实例化处天然可用，不需要 `reified` 来保留类型实参；
- 编译器自行决定内联策略，`inline` / `noinline` / `crossinline` 被接受但可忽略（允许作为提示，但不保证语义）；
- 不依赖语义内联的 Kotlin 代码可以保留这些关键字而无需修改；依赖 inline lambda non-local return 的代码不兼容，必须按 8.1.2 改为匿名函数或重写控制流。

### 8.5 调用实参、默认参数与 `vararg`

#### 8.5.1 参数声明

- 普通value parameter可以是必需参数、带缺省表达式的默认参数，或`vararg`参数；一个参数列表至多有一个`vararg`。class/struct主构造参数及构造函数式enum variant字段使用同一规则；`vararg val x: T`在构造结果中声明的属性/字段类型是`Array<T>`。
- `vararg x: T`中的`T`是**元素类型**；在函数体、构造体及函数签名的实际参数类型中，`x`的类型是`Array<T>`。因此只相差`vararg x: T`与普通`x: Array<T>`的两个声明具有相同参数类型，不能据此形成重载。函数类型同样只保留实际参数类型`Array<T>`，不保留`vararg`调用约定（见8.1.1）。
- `vararg`可以有显式缺省表达式，其类型必须是`Array<T>`；没有显式缺省且调用处没有提供任何vararg元素时，产生一个新的空`Array<T>`。
- 参数名、默认值及是否具有默认值不属于函数签名，不能仅靠它们区分重载；但参数名和`vararg`调用约定属于源码调用接口，必须保留到调用决议完成。
- 默认参数值表达式是callable源码接口的一部分，并在**定义处完成解析**：名称解析、overload选择、类型检查、可见性检查、挂起性检查及所引用实体身份都使用声明方上下文。它可以使用当前callable的类型参数、隐含receiver以及此前声明的参数，不能引用自身或后声明的参数。
- default直接绑定的每个实体都必须在该callable的**全部合法调用位置**可访问，即callable调用域必须是该实体可访问域的子集。对导出的`public` callable，这意味着只能引用下游可见的`public`或经`public import` re-export的实体，不能引用声明Cone的`private`/`internal`实现；`internal`、private/member及local callable可以引用覆盖各自完整调用域的实体。该检查适用于已选择的callable/overload、constructor、property accessor、operator目标及nominal type，并在const folding与desugaring之前执行；不能靠编译期折叠绕过可见性。
- `abstract`函数和interface方法可以声明默认值。`override`声明不得重新声明默认值；它按静态可见的override关系继承唯一的默认来源。若一个override位置从互不相关的父声明继承到无法唯一确定的默认来源，必须在类型定义处诊断，不能任选一个表达式。override的`vararg`形态必须与被覆写声明一致。
- override参数名不参与签名匹配；命名调用使用调用点静态接收者所见声明的参数名，动态分派只选择函数体，不重新映射实参或替换默认来源。

#### 8.5.2 调用处实参映射

- 调用实参有位置实参`expr`、命名实参`name = expr`、vararg展开实参`*expr`和命名vararg实参`name = expr` / `name = *expr`。尾随lambda是语法上位于圆括号之后的最后一个显式实参，进入同一映射与求值过程。
- 实参到形参的映射针对每个overload候选独立完成。未知参数名、重复绑定、spread映射到非`vararg`、遗漏必需参数或多余实参都会使该候选不可应用；不能先按任意候选重排一份公共参数表。
- 在尚未进入“仅命名实参”尾部时，第`i`个实参可以是映射到第`i`个形参的位置实参，也可以写出该形参本来的名字。某个命名实参一旦跳过声明位置、或通过名字绑定到其他位置，后续实参必须全部使用名字。默认参数不能在一串位置实参中间被隐式跳过。
- 位置实参到达`vararg`位置后，后续未命名的普通/spread实参都属于该`vararg`；若其后还有其他形参，这些形参只能用命名实参提供。
- 命名实参直接映射到同名形参。对`vararg x: T`，`x = array`与`x = *array`都提供完整的`Array<T>`参数值，不能再为`x`提供其他元素；未命名位置中的多个普通元素和多个spread则可以混合。
- 函数值调用不具有声明参数名、默认值或`vararg`调用约定，只接受与函数类型元数完全相同的位置实参；需要保留声明侧便利语义时必须显式创建适配lambda（见8.1.1）。

#### 8.5.3 求值顺序与调用处实例化

一次已经选定目标的调用严格执行：

1. 若有显式receiver，先求值receiver且只求值一次；
2. 所有**显式实参表达式**按它们在调用源码中出现的顺序从左到右求值且各一次，不受命名映射后的形参顺序影响；尾随lambda位于圆括号内实参之后；
3. 按形参声明顺序物化完整参数值：复用已求值的显式结果、构造vararg数组，并对缺失参数在其声明位置实例化和求值缺省表达式；显式缺省表达式只在对应参数确实缺失时执行；
4. 按形参声明顺序把完整值传给callee，再进入函数体。

因此所有显式实参都先于任何缺省表达式求值；多个实际使用的缺省表达式按参数声明顺序求值。命名实参只改变“值属于哪个参数”，不改变其源码求值顺序。例如`f(y = n(), x = m())`先执行`n()`再执行`m()`，随后以`x = m()`的结果、`y = n()`的结果调用`f`；若只写`f(y = n())`且`x`有默认值，则先执行`n()`，再执行`x`的缺省表达式。

- 调用目标与完整type arguments确定后，缺省表达式在每个发生缺省的调用处以已经绑定的typed template进行hygienic展开并求值，不是在声明时计算或缓存；调用方显式提供参数时，对应缺省表达式完全不展开。展开结果进入普通表达式编译流程，但调用方不会对template中的名称、extension或overload重新做决议，因而调用方import、同名局部声明或后来新增的overload不能改变其含义。
- 这与单态化一致：缺省表达式可能依赖类型参数（如`fun <T> f(x: T, y: Array<T> = [])`），只有在调用目标与全部类型实参确定后才能生成具体代码。缺省表达式自身不能作为“猜出”尚未确定类型实参的来源；若显式实参、receiver、允许的期望类型及声明约束仍不能唯一确定实参，调用不成立。
- 缺省表达式可以引用此前参数，因为完整参数值按声明顺序物化；此前参数无论来自显式实参、vararg构造还是另一个缺省表达式都已经可用。
- 导出callable的default template只引用已经导出或re-export的typed实体，不携带private/internal hidden dependency closure。修改参数名、default body、绑定目标或vararg形态会改变其`.slib`接口metadata并使下游重新编译；被引用实体的可见域收窄到不再覆盖callable调用域时，声明方必须重新编译并报错。
- `suspend`函数的缺省表达式可以包含挂起调用；它在挂起调用点实例化，此前已经求值的receiver与显式实参必须按8.2保存。普通函数及所有构造器/variant的缺省表达式不能包含挂起调用；suspend caller显式写出的普通构造实参仍按8.2合法。
- 所有缺省表达式采用相同的实例化语义，不因表达式是literal、普通调用、构造、intrinsic或其他kind而改变。实例化后的每个表达式同时具有声明节点的**定义来源**和发生缺省的call expression的**求值来源**；前者用于定义处诊断与源码归属，后者供任何需要观察求值位置的语言设施使用，二者都必须存在且不能互相回退或覆盖。若该call expression本身来自另一个缺省表达式实例，其已有求值来源继续传入内层实例，因此无需识别具体表达式内容即可得到最外层实际调用来源。

#### 8.5.4 `vararg`值

- 普通位置元素`e`为`vararg x: T`贡献一个满足`type(e) <: T`的元素；spread表达式当前必须具有精确的`Array<T>`类型。Scoop的`Array`保持invariant，在use-site projection落地前不以`Array<S>`模拟`Array<out T>`；未来放宽必须显式定义逐元素转换、装箱与表示成本。
- 未命名vararg元素（包括spread）先各自按8.5.3求值，随后在形参物化阶段构造一个新的`Array<T>`；spread按元素顺序复制，可以与普通元素混合。即使唯一输入是`*array`也不与来源数组共享identity。
- 命名形式`x = array`或`x = *array`直接提供完整`Array<T>`值，不额外复制；它与“若干位置元素组成新数组”是两种不同的源码调用形态。
- 没有元素且没有显式默认值时构造新的空数组；有显式默认值时按普通缺省表达式求值并直接使用其结果。实现可以在identity不可观察时消除分配或复制，但不能改变`===`、异常、挂起点或求值顺序可观察到的结果。

### 8.6 调用决议与泛型约束求解

- 调用决议先按词法/成员/import优先级建立候选层，并在每层内按9.3.4的function-like/property-like c-level继续分区；对每个最终分区完成调用形态预过滤与候选各自的类型可应用性检查，再只在第一个含有至少一个可应用候选的分区中求最具体候选。显式类型实参数量、命名参数是否存在、spread/receiver形态等不依赖表达式类型的检查属于预过滤；某个更高层仅有同名但形态、类型或bound不适用的声明时，不得无条件遮蔽合法的下一层候选。多Cone及显式/星号import加入时只扩展候选层，不改变后续算法。
- 每个候选拥有独立的实参映射、fresh inference variables和constraint system。receiver、非postponed实参、完整显式类型实参、函数/nominal声明约束及upper bound共同产生等式/子类型约束；generic owner参数与callable自身参数保持不同identity，不能压平成一组后再按长度反推。
- 依赖候选期望类型的lambda、匿名函数、callable reference、`None`、空数组和嵌套generic构造作为postponed argument处理。求解器先用其余约束推进固定点，再用候选给出的完整期望类型检查postponed argument；lambda body结果只决定候选是否适用，不提供额外的“按lambda返回类型优先”规则。
- constraint system必须同时满足声明点kind/interface bound、函数类型型变、interface声明点型变、普通subtyping及装箱规则。一个候选只有在所有必需类型实参得到唯一、可表达且满足bound的具体解，全部显式与postponed实参都可赋给对应参数时才可应用；不得用`Any`、bound、默认false或任意首个类型补齐无解/多解变量。
- 外层期望类型可以在唯一callable目标已经不依赖返回类型选择时帮助固定只出现在返回结果中的类型参数，也可以为generic nominal构造提供宿主application；它不能使两个仅靠结果类型才能区分的overload变得合法或在多个候选间充当MSC比较项。普通函数签名仍不含返回类型，返回类型不同不能单独形成重载。
- 最具体候选使用独立于本次实际推断结果的pairwise forwarding constraint system：比较`A`是否至少与`B`同样具体时，把`A`的声明参数替换为fresh variables，再检查其每个由调用提供的参数（extension receiver也算）是否可转发给`B`的对应参数，并同时加入双方声明bound。不能比较两边已经为当前调用猜出的concrete type arguments。
- 若唯一候选能转发给所有其他候选而反向不成立，则它胜出；互相可转发或互相都不能转发时，依次应用规范已有的附加规则：非参数化候选优先；M17起，在互相可转发的集合中实际使用更少默认值者优先，仍相同时无`vararg`者优先。命名/位置写法本身不参与优先级。仍不唯一即为歧义。
- 整数字面量专用widen规则在定宽整数及其字面量类型正式落地前不生效；当前`Int`字面量只有`Int`类型。use-site/star projection及capture conversion进入类型系统后必须扩展同一个constraint/subtyping框架，不能另建一套projection-only overload resolver。
- 无匹配与歧义都是HIR编译错误。诊断必须列出所在候选层、每个相关候选的完整签名及其失败原因（形态映射、类型实参数量、未解变量、bound、实参类型或MSC并列），不能只报告“unknown function”或由下游根据缺失callee猜测失败原因。

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

#### 9.3.1 `operator`声明

参与运算符约定的函数必须显式写`operator` modifier；仅仅使用约定名称不会使普通函数成为运算符。除下述`equals`外，operator必须是成员函数或extension函数，可以是ordinary/suspend、generic或infix。operator标志属于override contract，override与被覆写声明必须完全一致。

HIR把通过验证的角色保存为封闭、类型化的operator identity；表达式决议不得再从函数名或签名反推能力。合法名称与声明约束如下：

| 角色 | 显式普通参数 | 额外约束 |
| --- | --- | --- |
| `unaryPlus` / `unaryMinus` / `not` | 0 | 返回类型不限 |
| `inc` / `dec` | 0 | 返回类型必须是receiver静态类型的子类型 |
| `plus` / `minus` / `times` / `div` / `rem` | 1 | 返回类型不限 |
| `rangeTo` / `rangeUntil` | 1 | 返回类型不限 |
| `contains` | 1 | 返回`Boolean` |
| `get` | 至少1个 | 返回类型不限；允许按8.5使用default/`vararg` |
| `set` | 至少2个 | 返回`Unit`；最后一个参数是写入值且不得为`vararg`，此前参数是索引 |
| `invoke` | 任意 | 使用完整8.5调用参数协议 |
| `plusAssign` / `minusAssign` / `timesAssign` / `divAssign` / `remAssign` | 1 | 返回`Unit` |
| `compareTo` | 1 | 返回`Int` |
| `equals` | 1 | 返回`Boolean`；见下述收紧规则 |
| `componentN`（`N`为正十进制整数） | 0 | 返回类型不限 |
| `iterator` | 0 | 返回值在`for`使用点满足11.8的`Iterator<T>`协议 |

固定元数角色的参数可以具有default，但operator语法提供的operand仍按8.5映射到对应参数；只有`get`、`set`和`invoke`能以`vararg`表达可变元数。名称不在表中、元数或返回约束错误、`component0`及非数字`component`名称都是声明处错误，不能以普通函数身份携带`operator`标志进入HIR。

属性委托所需的`provideDelegate` / `getValue` / `setValue`不是本表的`set`下标角色。它们的reflection-free签名及调用时机随9.2的属性委托一并规定；在该协议定稿前不能仅按名称赋予operator identity。

`equals`是本规范对Kotlin约定的有意收紧：可参与`==`的声明必须是名为`equals`的**成员**`operator fun`，恰好有一个显式参数并返回`Boolean`，且不得为generic或suspend。顶层、局部与extension equals不参与`==`。成员可以重载，也可按普通规则声明为final/open/abstract/override；具体决议见11.11。

`equals`签名中的参数必须写普通显式类型。Scoop没有`Self`类型：若interface需要表达“与某个类型比较”，应写成例如`interface EqualTo<T> { operator fun equals(other: T): Boolean }`，实现者显式选择`EqualTo<Point>`等application；编译器不把interface中的任何名字隐式替换为实现者类型。

#### 9.3.2 表达式展开

除单独说明外，operator展开后使用8.6的普通显式receiver调用决议，只保留具有对应typed operator identity的成员/extension候选，并完整复用8.5的default、`vararg`、泛型与求值协议：

| 源码 | 概念调用 |
| --- | --- |
| `+a` / `-a` / `!a` | `a.unaryPlus()` / `a.unaryMinus()` / `a.not()` |
| `a + b` / `a - b` / `a * b` / `a / b` / `a % b` | `a.plus(b)` / `a.minus(b)` / `a.times(b)` / `a.div(b)` / `a.rem(b)` |
| `a..b` / `a..<b` | `a.rangeTo(b)` / `a.rangeUntil(b)` |
| `a in b` / `a !in b` | `b.contains(a)` / `!b.contains(a)` |
| `a[i1, ..., iN]` | `a.get(i1, ..., iN)` |
| `a[i1, ..., iN] = v` | `a.set(i1, ..., iN, v)` |
| `a(args...)` | function-value call，或`a.invoke(args...)`（见9.3.4） |
| `a < b` / `a <= b` / `a > b` / `a >= b` | `a.compareTo(b)`的`Int`结果与0比较 |
| `a == b` / `a != b` | 11.11的成员`equals`调用 / 对同一结果取反 |

receiver先于调用实参求值，因此`a in b`与`a !in b`按概念调用先求值`b`、再求值`a`；这是有意保留的Kotlin顺序。其他表项按书写的receiver再到operand顺序求值。`&&` / `||`仍是只接受`Boolean`的内建短路操作，`===` / `!==`仍是不可重载的引用identity比较；`=`, `?:`, `!!`, `is` / `as`及安全导航本身也不可重载。

operator调用只考虑function-like operator目标，不能再通过property-like `invoke`递归寻找某个同名operator。一次`a(args...)`至多应用一次`invoke`约定；若选中的`invoke`返回另一个可调用值，必须再写一组显式括号才能调用。

#### 9.3.3 自增、自减与复合赋值

`++` / `--`的operand必须是可读写place。编译器先把local/global/field/下标place中的receiver与index从左到右各求值一次，再执行：

- prefix `++a` / `--a`：读取旧值，调用`inc` / `dec`，把适配后的新值写回，并以新值为表达式结果；
- postfix `a++` / `a--`：执行同样的单次读、调用和写回，但表达式结果是写回前的旧值。

对`a op= b`（`op`为`+ - * / %`），候选探测同时考虑对应`opAssign`与普通`op`：

1. 仅`opAssign`可应用时，读取place值并调用它；不写回，因此只读`val`也可以作为receiver；
2. 仅普通`op`可应用时，place必须可写，调用结果必须可赋给place静态类型，再写回；
3. 二者都可应用时报歧义，不能擅自偏好其中一个；二者都不可应用时报完整候选失败；
4. 整个语句的结果为`Unit`。

下标place的读写分别通过同一静态receiver上的`get`与`set`operator选择；receiver、全部index及右操作数都只求值一次。所有这些展开在HIR形成类型化place/evaluation plan后正规化为普通temporary、call与assignment；MIR不得重新解析operator或复制源码子表达式。

#### 9.3.4 property-like `invoke`与候选分区

任意表达式`e(args...)`先定型`e`：若其类型是8.1的函数类型，执行内建函数值调用；否则只从其静态类型收集成员及可见extension `operator fun invoke`，并按普通调用决议。`FunPtr<F>`仍不提供Scoop侧`invoke`。

对名称调用，property-like callable表示“先读取该名称对应的值，再对结果应用一次上述invoke规则”。显式receiver调用的c-level分区顺序为：

1. member function-like callable；
2. member property-like callable + member `invoke`；
3. 各extension作用域中的extension function-like callable；
4. member property-like callable + extension `invoke`；
5. extension property-like callable + member `invoke`；
6. extension property-like callable + extension `invoke`。

M18之前已有的local/parameter/capture、global、primary-constructor property及普通field都可作为property-like来源；extension property/object/companion加入后插入同一分区，不改变算法。每个组合先完成property选择，再以其结果类型建立独立invoke候选；最终优先级取property与invoke两部分中较低者。调用的显式type arguments、命名/spread/尾随lambda只转发给`invoke`，不作用于property读取。

为保留8.1.3已确定的词法遮蔽，最近词法作用域中同名的函数类型binding，或静态类型具有至少一个可见`invoke`operator的binding，先形成唯一local property-like层并遮蔽同名函数声明；其调用形态/类型不匹配时针对该binding诊断。普通不可调用binding不参与callable层。其他层仍遵守8.6的“第一个含可应用候选的分区”，不能因存在同名但不可应用的property无条件阻断后续函数。

#### 9.3.5 `infix`

`infix`函数必须是成员或extension函数，且恰好有一个required、非`vararg`的显式普通参数；可以同时是generic、suspend及`operator`。不满足声明形态、用于top-level非extension或local非extension函数都是声明处错误。`infix`标志与`operator`一样属于override contract。

`lhs name rhs`等价于`lhs.name(rhs)`，要求直接function-like候选带`infix`，或property-like候选最终选中的`invoke`同时带`operator infix`；其余决议与求值规则不变。infix调用左结合，必须显式写receiver（当前`this`上调用写成`this name rhs`）。其优先级从高到低位于range与Elvis之间：postfix、prefix、cast、乘法、加法、range、infix name、Elvis、`in`/`is`、比较、相等、`&&`、`||`、赋值。不同infix名称没有自定义优先级。

普通class可以通过`componentN` operator支持位置解构；每个实际需要的位置独立解析对应operator并只求值被解构值一次。struct/tuple仍走4.6的内建解构，不查找`componentN`。`iterator`只定义11.8中`for`脱糖的入口；M18接受并类型化该operator声明，`for`及range core类型在相应基础语言里程碑实现。

### 9.4 注解

支持自定义注解与注解使用语法；不内置平台相关注解。注解不携带运行期反射能力（见 2.2），编译期处理（如编译器插件）由实现定义。语言核心注解（`@Intrinsic` / `@NoGC` / `@Extern` 等）见第 13 章。

---

## 10. 数组

Scoop 内置两个数组类型（引用类型，属于核心库）：

- `class Array<T>`：不可变数组（长度固定，元素不可写）；
- `class MutableArray<T>`：可变数组（长度固定，元素可写）。

它们是由core源码提供nominal identity、由`@Intrinsic`提供representation family的invariant generic class，使用与普通`class C<T>`相同的类型application、约束、成员解析和单态化规则。编译器可以为字面量、内联元素区、下标和转换保留typed专用操作，但不得再建立一个与core class声明平行的数组类型身份。

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

- 下标访问 `a[i]`通过普通成员`operator fun get(index: Int): T`；`MutableArray`通过`operator fun set(index: Int, value: T): Unit`支持下标赋值`m[i] = v`。这些声明可以由intrinsic提供表示级实现，但候选选择、泛型实例化与operator identity遵守9.3，不建立按`Array`类型名放行的第二套解析规则。
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

字面量、算术/位运算/比较运算与 Kotlin 一致。每个进入已实现语言子集的基本类型必须同时提供同类型值相等、`ToString`与`Hash` core实现（11.11）；这些实现按具体value工作，不经过装箱或`Any`分派。

### 11.3 `Unit`

0 元 tuple 的类型名与值构造器（见 4.3）。

### 11.4 `String`

- 引用类型，immutable，UTF-8 语义（编码细节由实现定义）。
- 支持 `+` 拼接、索引/切片、`length`（或 `size`）、比较等核心操作。
- 实现内容相等的成员`operator fun equals(other: String): Boolean`与内容相关`Hash`；`toString()`返回自身（`ToString`的恒等实现）。这些能力均是String的具体core contract，不来自`Any`或TypeDescriptor缺省槽。

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
- generic class可以继承`Throwable`；其每个完整application是不同异常类型。`catch (e: Error<Int>)`只匹配该exact application，`catch (e: Throwable)`仍匹配所有application。当前没有`Error<*>`式通配catch，因为star projection尚未支持（3.2）。

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

区间：`IntRange` / `LongRange` / `CharRange`（`..`调用`rangeTo`，`..<`调用`rangeUntil`；`until` / `downTo` / `step`使用普通infix函数），实现 `Iterable`。`..` 与 rest 模式的消歧见 4.6。

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
  - `lhs == rhs`先各求值一次，再只从lhs静态类型收集成员`operator fun equals`候选，按普通成员overload规则选择唯一目标；`lhs != rhs`调用同一目标后对结果取反。不存在交换左右操作数、extension、地址比较、`Any.equals`或TypeDescriptor fallback。
  - **值类型：条件派生的结构相等**——编译器可以额外提供一个参数类型等于lhs完整静态value type的`operator fun equals`候选：例如`Point.equals(other: Point)`，generic template `Box<T>`中则是`Box<T>.equals(other: Box<T>)`，实例化后得到`Box<Int>.equals(other: Box<Int>)`。这些都是普通typed nominal application，不存在`Self`占位符。当且仅当类型的所有字段（元素）**可比较**时生成：字段可比较表示对两个该字段静态类型的值执行`==`能选出唯一目标；基本类型具有核心实现，其他value type递归应用本规则。struct/tuple逐字段按声明顺序短路；enum先比较tag，再只比较active variant payload；Unit恒等。任一字段不可比较时，该派生候选不存在，诊断指出首个失败字段/variant路径。
  - 用户声明参数类型为当前完整宿主application的同签名`equals`时取代派生体；其他参数类型的equals overload不屏蔽该同类型候选。派生方法也是普通成员，遵守value-type`this`按值传递规则。
  - **引用类型**：只使用该class/interface静态类型声明或继承的成员operator equals；不存在时是编译错误。`Any`没有成员，因而`Any == Any`非法；运行期对象另有equals不能补齐静态契约。需要identity比较时显式使用`===`。
  - equals 的决议只考虑成员函数（含编译器派生）；扩展函数不得参与——import不能改变某类型`==`的语义。
- **`ToString`（字符串化）**：接口`interface ToString { fun toString(): String }`。class/object/struct/enum都必须在声明中显式列出该interface并提供合法override；字段或payload实现`ToString`不会让宿主自动获得conformance。generic nominal type若在实现体中调用类型参数值的`toString()`，必须为相应参数声明普通`ToString`上界。tuple与Unit不能声明implements列表，因而不实现`ToString`。String与基础类型由core中的intrinsic nominal声明显式adopt，String实现返回自身。`print` / `println` 定义为`fun <T : ToString> print(v: T)`并经普通单态化bound call实现，不接受`Any` fallback，也不按成员同形或字段结构补齐conformance。
- **`Hash`（哈希）**：接口`interface Hash { fun hash(): Int }`。**没有任何缺省或派生实现**；基本类型与String由核心库提供内容相关实现，其他类型显式opt-in。相等的值必须产生相等hash；不以对象地址作为hash，也不承诺算法跨runtime版本保持相同数值。struct不自动获得哈希。

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

- `getCurrentSourceLocation()` 返回所在表达式的标准**求值来源**所指示的源码位置（文件、行、列）与所处函数、类型的名称。该信息在编译期已知，不依赖调试信息；本intrinsic不建立独立的调用处传播机制。
- 典型用法是与 8.5 的缺省参数规则组合，在调试信息与诊断设施落地前提供廉价的 runtime diagnostic / tracing 机制：

```
fun trace(msg: String, loc: SourceLocation = getCurrentSourceLocation()) {
    // loc 是调用 trace 的位置，而非本函数内部
}
```

- 作为普通表达式出现在缺省参数template中时，它与template内其他表达式一样按8.5取得定义来源和求值来源；随后普通intrinsic语义读取求值来源，因而返回**最外层调用处**的位置，而不是default机制识别并重写本函数。多层函数转发时，每一层都必须以缺省参数继续转发`loc`（`fun warn(msg: String, loc: SourceLocation = getCurrentSourceLocation()) = trace(msg, loc)`），否则记录的是中间层的位置。
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
- 下游HIR所需的export metadata：导出的非泛型声明语义接口、泛型声明与template body及其类型化依赖闭包、`const val`值，以及作为callable接口在调用处展开的hygienic typed default template。default template只引用已导出/re-export实体，不携带private/internal hidden dependency closure；
- 后续stage所需的MIR/LIR metadata：符号表、各导出类型的分派表结构（vtable / itable）、TypeDescriptor符号与类型布局（供下游建表、继承与嵌套布局）等。

本Cone为了生成`.o`而建立的fully concrete HIR函数体和类型实例只供本Cone的MIR消费，不属于`.slib` export metadata。下游HIR需要的“concrete信息”是导出的非泛型语义接口，而不是上游本地实例体；两者必须具有不同的实体身份，不能共享Cone内arena id。

下游 Cone 编译时读取上游 `.slib` 的 metadata 完成导入解析、类型检查与泛型实例化；最终链接时合并各 Cone 的 `.o`。因此：

- 上游 `.slib` 的 metadata 变化会使下游的编译缓存失效（下游需要重编译），这是单态化的自然结果；
- 符号命名（name mangling）与 ABI 细节由实现定义。

### 12.6 核心库

核心库 `scoop.core`（第 11 章）本身是一个 Cone；每个 Cone 都**隐式依赖**它，无需在 `Cone.toml` 中声明。

---

## 13. 核心注解与 FFI

以下注解类定义于 `scoop.core`，随默认导入可用。它们修饰的约束大多在编译期检查，违反即为编译错误。

本章多处使用 **GC-free** 的概念：一个类型是 GC-free 的，当且仅当其完全确定的表示中不直接或间接包含任何 ref type（基本类型、`Ptr`、`FunPtr`、不含ref的`@CLayout` struct等都是 GC-free 的值类型）；一个函数是 GC-free 的，当且仅当其不读写、不创建任何 ref value（见 13.2）。GC-free布尔属性只属于不含未解析type parameter的concrete type或fully specialized generic type；每个这样的type实体都必须具有非可选的`gc_free: bool`，不存在“未知”或缺失状态。尚未完全特化的generic declaration不是concrete type，没有GC-free真假flag；编译器可以保存“哪些实参必须GC-free”的符号条件，但该条件不是flag。每个fully specialized enum的每个variant都必须具有非可选`gc_free: bool`，enum自身的flag恒等于所有variant flag的逻辑AND。

### 13.1 `@Intrinsic`

```
annotation class Intrinsic(val name: String)
```

- 用于function/method：该函数是compiler intrinsic，由编译器生成实现；**函数体必须省略**；`name`是intrinsic的编译器内部标识。

```
@Intrinsic("int_add")
operator fun Int.plus(rhs: Int): Int
```

- 也可用于登记表明确允许的core struct/class，声明一个**intrinsic type**：该类型的representation、literal lowering、ABI及内部构造机制由编译器提供，源码声明其nominal interface与成员语义。例如：

```
@Intrinsic("core_int")
struct Int : ToString, Hash {
    @Intrinsic("int_equals")
    operator fun equals(other: Int): Boolean

    @Intrinsic("int_to_string")
    override fun toString(): String

    override fun hash(): Int = this
}
```

- intrinsic type不声明字段/primary constructor，也不等价于零字段普通struct/class；不能据此派生零大小布局、全等equals、`Int()`字符串、解构或公开零参数constructor。编译器合成的literal/boxing/allocation entry不进入源码候选集；任何用户可调用constructor或转换仍须显式声明；
- intrinsic type的implements列表、普通成员body、override/operator规则与普通类型一致；单个成员也可像上例一样另用function intrinsic提供实现。初始intrinsic type至少覆盖进入已实现语言子集的Int/UInt/Boolean、String以及`Array<T>`/`MutableArray<T>`，并允许登记表按同一契约增加其他compiler-represented value/reference type；
- intrinsic type可以是generic，但其登记项必须完整规定declaration kind、type-parameter数量/variance/bound及representation family。`Array<T>`与`MutableArray<T>`各要求一个无bound、invariant参数；它们的每个fully specialized application仍是普通generic class application，只是对象布局、元素stride和GC扫描由携带concrete element type的typed intrinsic representation产生。不得同时保留普通class application与独立built-in array type两种identity；
- 生产语言只允许指定的`scoop.core` provider声明intrinsic。编译器测试可以通过实现内部的、按输入provider授权的策略绕过这一条来源检查；该能力不是源码、manifest或稳定CLI的一部分，也不放宽以下name、target、shape、signature与唯一性规则。

- 除非有单独说明，`@Intrinsic` 不能与其他任何注解共存。
- `name` 必须是编译器内置intrinsic登记表中的已知标识；未知`name`、错误annotation target、与登记shape/signature不符或同一intrinsic kind存在多个provider都是编译错误（用户不能声明自定义intrinsic）。各intrinsic在编译pipeline中的展开阶段由实现大纲规定。

### 13.2 `@NoGC`

```
annotation class NoGC
```

- 用于function/method：指明该函数不会/不应与GC有任何交互——函数中不读写任何ref value，也不创建任何ref type实例。
- 也可用于`struct`或`enum`，作为“该concrete value type必须GC-free”的静态契约。非generic声明在字段类型解析后立即验证；generic声明本身没有GC-free真假值，每个type parameter全部resolve后的实际类型分别验证。对fully specialized enum，契约同时要求enum整体及每个variant均为GC-free。`@NoGC`不能用于class/interface，因为它们是ref type。
- 编译期检查；不符合约束是编译错误。
- generic `@NoGC` callable 可以在签名或 body 中使用类型参数；未特化的generic本身不被判为GC-free或非GC-free。每个实际影响参数、返回值、receiver、局部值或表达式表示的类型参数，都会形成“实例化实参必须GC-free”的类型化条件，并经generic调用链向外传播；只有type parameter全部解析后的具体实例才能用concrete type的GC-free flag验证并成为`@NoGC`实例。未参与运行时表示的phantom type parameter不产生条件。
- `T : value` 只保证实参是 value type，不保证其递归表示中不含 managed ref，因此不能代替上述 GC-free 条件；`T : ref` 则不可能满足该条件。当前没有单独的源码 bound 语法来声明 GC-free，条件由 `@NoGC` body及其调用图推导。
- 这样的函数可以安全地跨越 FFI boundary（例如作为 FFI 回调）。
- 该约束也意味着 `@NoGC` 的成员函数只能属于 value type：class method 有隐含的 `this` 参数，而 `this` 是 ref value。

```
@NoGC
fun add42(n: Int) = n + 42

@NoGC
fun <T> identity(value: T): T = value

@NoGC
struct NativePair(val x: Int, val y: UInt)

// 编译错误：字段直接包含ref
@NoGC
struct BadNativeValue(val text: String)

val number = identity<Int>(42)          // 合法：Int 是 GC-free
val text = identity<String>("managed") // 编译错误：String 是 ref type

// 编译错误：String 是 ref type，违反 NoGC 约束
@NoGC
fun addString(s: String) = s + "world"
```

### 13.3 `@Unsafe` / `@Safe`

```
annotation class Unsafe
annotation class Safe
```

- **unsafe function**：C ABI `@Extern` 函数、标注 `@Unsafe` 的函数。Scoop ABI extern 默认是 safe callable，但声明可显式加 `@Unsafe` 收紧调用条件（13.4、14.2）。
- **unsafe context**：标注 `@Unsafe` 的函数体，或标注 `@Unsafe` 的 block。只有在 unsafe context 中才能调用 unsafe function；在 unsafe context 之外调用是编译错误。
- `@Safe` 用于 function 和 block，用于在 unsafe context 中重新引入 safe 约束（其中的代码回到普通检查规则）。

### 13.4 `@Extern` 与 `@CallingConvention`

```
annotation class Extern(val lib: String = "", val name: String = "", val abi: String = "c")
annotation class CallingConvention(val name: String)
```

- `@Extern` 用于 function：指明该函数是位于 `lib` 所指库中的 FFI function，符号名由 `name` 指定，`abi` 指定 ABI（见 13.8）。函数体必须省略。声明可以带普通Scoop默认参数；缺省表达式按8.5在定义处解析、调用处实例化，不进入native symbol的ABI，native调用始终接收完整参数列表。`vararg`在ABI中表现为一个普通`Array<T>`参数，因此只有该实际参数类型满足对应ABI classifier时才合法：C ABI因ref不安全而拒绝，Scoop ABI可以接受；这不表示支持C的`...`可变参数。
- `@Extern` 与 `suspend` **互斥**：无论 `abi` 取值为何，`@Extern suspend fun` 都是编译错误。编译器不为这种声明生成 continuation 参数、`CoroutineStep` 返回值或同步/挂起 wrapper；M10 的 hidden continuation ABI 不得作为外部符号 ABI 暴露。
- `@Extern` 也可用于**全局变量**（`val` / `var`），访问库中的全局符号；全局 `var` 的约束不变（仍须带 `@Global` / `@ThreadLocal` 且 GC-free，见 13.6），注解可以组合。extern 变量当前只支持 C data ABI，显式写 `abi = "scoop"` 是编译错误；Scoop ABI 只定义函数调用边界。
- **按 ABI 分类的边界类型约束**：`abi = "c"` 的函数签名及 extern 变量必须满足 13.8 的 C-FFI-safe 约束，因而全部 GC-free；ref type 出现在这些边界上是编译错误。`abi = "scoop"` 的函数复用普通 Scoop typed ABI，可以按第 14 章直接传递 managed ref，不套用 C-FFI-safe classifier。
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

- **C ABI**（`abi = "c"`，默认）：标准的 FFI function，由外部 lib / so / dylib / dll 提供。它对 Scoop 的类型系统和 GC 环境没有任何了解，也不能使用相关功能，用于直接引入外部库。参数与返回值必须是 C-FFI-safe：GC-free 且具有本章规定的稳定 C 表示。
- **Scoop ABI**：复用普通、非挂起 Scoop 函数的 typed machine ABI。ref 参数/返回值直接以 managed ref value 传递，value type按 Scoop 自身的 concrete ABI 传递；被调方能读取 TypeDescriptor，并可按第 14 章的 native-root 协议显式进入可能触发 GC 的 runtime 操作。它主要供 runtime 与 core 使用，不是通用 C library ABI。

C ABI callee本身始终是 GC leaf，不能直接接收managed ref或调用Scoop GC；多线程runtime下 caller仍须按 runtime spec 3.5 发布roots并切换到native-safe状态。C代码只有在持有14.3注册得到的静态trampoline与cookie时，才能经独立的反向边界进入managed callback；这不改变该C函数自身的参数ABI或赋予它Scoop ABI能力。Scoop ABI调用按普通managed call生成，不切换到GC-free native-safe状态。具体机器级序列由实现决定，但不得改变上述类型与GC契约（第14章）。

ref type（如 `String`、`Array`、普通 class）不能出现在 C ABI 的边界上（13.4 的 C-FFI-safe 约束）；同一类型可以直接出现在 Scoop ABI extern 签名中。`PinnedPtr<T>` / `GcHandle<T>` 已是 GC-free 的显式边界值：它们适合 C ABI、跨调用保活或需要稳定裸地址的场景，不是 Scoop ABI direct-ref 调用的必经表示。

### 13.9 `value` / `ref` 类型约束

`value` 与 `ref` 是上下文关键字，用作泛型的 type bound：

- `T : value`：`T` 必须是值类型（struct / enum / tuple / 基本类型）；
- `T : ref`：`T` 必须是引用类型（class / interface）。

```
fun <T : value> needValue(v: T) { ... }

needValue("hello")    // 编译错误：String 不是值类型
```

- 与类型上界语法同样可用于 `where` 子句。
- interface类型上界可直接写在参数上（`T : ToString`），也可写在声明头之后的`where`子句；同一参数可以有多个不同interface上界。当前类型上界只接受完整的interface application，不接受class、value type、函数类型、`Any`或另一type parameter。
- `value` / `ref` 约束与interface类型上界互斥：同一类型参数不能同时携带两者；同一个kind bound也不能重复出现在inline与`where`位置。
- 无约束的类型参数默认接受任何类型（与 Kotlin 一致）。

### 13.10 `Ptr` 与 `FunPtr`

二者定义于 `scoop.core`，是 FFI 的基础辅助类型，均为值类型。

#### `Ptr<T>`

`Ptr` 能容纳一个 raw pointer；`addressOf` 用于取一个变量的内存地址：

```
struct Ptr<T : value>(val _rawPointer: UInt) {
    @NoGC @Unsafe
    @Intrinsic("ptr_to_uint")
    fun toUInt(): UInt

    @NoGC @Unsafe
    @Intrinsic("ptr_cast")
    fun <U : value> cast(): Ptr<U>

    @NoGC @Unsafe
    @Intrinsic("ptr_load")
    fun load(): T

    @NoGC @Unsafe
    @Intrinsic("ptr_load_offset")
    fun load(offset: Int): T

    @NoGC @Unsafe
    @Intrinsic("ptr_store")
    fun store(value: T)

    @NoGC @Unsafe
    @Intrinsic("ptr_store_offset")
    fun store(offset: Int, value: T)

    @NoGC @Unsafe
    @Intrinsic("ptr_plus")
    fun plus(offset: Int): Ptr<T>

    @NoGC @Unsafe
    @Intrinsic("ptr_minus")
    fun minus(offset: Int): Ptr<T>
}

@Unsafe
@Intrinsic("address_of")
fun <T : value> addressOf(v: T): Ptr<T>
```

- `Ptr` 的上述方法均为编译器 intrinsic，且都是 unsafe function（见 13.3）。按 13.1 的规则 `@Intrinsic` 通常不得与其他注解共存；此处是单独说明的例外：这些 intrinsic 允许与 `@NoGC` / `@Unsafe` 组合。
- 源码中的 `pointer + offset` / `pointer - offset` 分别按 `ptr_plus` / `ptr_minus` 的契约处理；`offset` 以**元素个数**计（步进 `offset * sizeOf<T>()` 字节），与 C 的指针算术一致。带 `offset` 的 `load` / `store` 使用相同的元素偏移语义。
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

- `_rawPointer` 存放实际的函数指针值（与 `Ptr` 同样以 `UInt` 容纳 raw pointer）。缺省构造产生 null 指针（`0u`），因此 `FunPtr` 可以声明为 struct 字段、先以 null 填充。用户源码中独立的静态非null地址只能由编译器对`::name`执行native-address contextual resolution生成（见下）；14.3 的`ForeignCallback.function`是编译器生成的另一种非null值，指向必须与opaque context cookie配对使用的managed-callback trampoline，不是静态目标地址。
- 除缺省构造（null）外，用户**不能直接构造** `FunPtr` 值；在期望类型明确为 `FunPtr<F>` 的位置使用顶层函数声明引用 `::name`，由编译器直接生成原生 callback 地址。`::name` 是 8.1.4 的中性源码语法，不具有固有的 `FunPtr` 类型；因此 `val callback = ::name` 仍推导为 managed 函数值，要保存原生地址必须由类型标注、参数类型、返回类型等上下文提供 `FunPtr<F>` 期望类型。目标声明签名必须与 `F` **精确相同**，不应用 8.1.1 的函数类型型变。`ForeignCallback.function`只能从已注册值读取，不能用构造器仿造。
- `F` 的每个参数与返回类型必须满足 C-FFI-safe 约束；`Unit` 只允许作为返回类型。`FunPtr` 描述的是 C ABI callback 地址，不是 Scoop ABI managed callable；函数类型 `F` 在这里仅描述 native signature，本身不会作为 managed 引用穿越边界。
- 可空函数指针用 `Option<FunPtr<F>>` 表示（niche 优化见 7.4）。
- native-address resolution 的目标必须是带 `@NoGC` 的普通顶层命名函数，且**不能是 generic、挂起、extern、成员或扩展函数**。lambda、匿名函数、局部函数、任何绑定引用以及已存在的 managed 函数值都不能作为非 null `FunPtr` 的来源。违反这些约束是编译错误：FFI 回调不得与 GC 交互，generic 函数没有单一具体符号，挂起函数只有编译器内部的 hidden continuation ABI，而 closure 还需要原生 ABI 中不存在的 managed 环境参数。编译器不自动生成 closure 或挂起 callback wrapper。
- `FunPtr` 不提供 Scoop 侧 `invoke`；它只用于传递/存储 native callback 地址。由`::name`得到的M12静态callback仅允许原生方在发起extern调用的同一已注册线程上同步调用；保存后异步、跨线程或在Scoop程序退出后调用不隐式获得安全性。M13只有14.3 registration返回的`ForeignCallback.function + context`配对具备对应token/attach协议，单独保存或调用其中的function而不携带仍存活的配对context同样非法。

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
- `PinnedPtr` 与 `GcHandle` 是不同的类型，混用（如 `unpin` 一个 `GcHandle`）是编译错误。二者都是只含一个 `UInt64` 字段的 GC-free 值类型，ABI 与 `UInt64` 一致，可以直接出现在 C ABI 签名中（13.4 的 C-FFI-safe 约束）。
- 取舍：短期持有并需要裸指针时用 `pin`（O(1)，但阻碍 GC 移动）；长期保活且允许移动时用 `GcHandle`。
- Scoop ABI extern 的同步调用期间若只借用 direct ref，调用方不需要显式 pin 或 handle；被调方需要跨 safepoint或调用结束保存引用时才使用 14.3 的 native root、pin 或 handle机制。
- handle 取回对象时类型 `T` 来自 handle 的类型参数，编译器无法校验其真实性——这层正确性由 runtime 作者保证。

### 14.2 调用约定

Scoop ABI FFI 的 caller side（Scoop 托管代码一侧）必须生成 typed native-borrowed call 框架：

- 调用前把所有调用后live ref及direct-ref实参写入显式、可更新的caller-root frame；含ref返回值使用调用前已清零并一同发布的typed result storage；
- machine call仍由statepoint rewrite处理并带唯一ID，但其`gc-live`/`gc.relocate`必须为空；冻结managed栈段只以caller-root frame为真相来源，不能同时从另一份statepoint spill恢复引用；
- M12 单mutator实现不插线程状态转换；M13多mutator实现发布live caller roots并在调用期间进入`native-borrowed`，返回managed前检查GC epoch。它不得进入C ABI使用的`native-safe`，也不得省略direct-ref callee在显式runtime入口所需的native-root协议；
- native返回含ref结果时，caller在仍处于`native-borrowed`且本线程尚不能被视为quiescent时立即写入已发布result storage；leave/epoch握手后只从caller-root/result slot reload，再移除root frame；
- `@NoGC`只保证native callee不调用GC/runtime/managed callback；M15多mutator moving实现仍保留native-borrowed transition、caller-root publication与返回reload，不能把该边界降为无root的普通NoGC call；
- machine callconv 初版使用 LLVM 默认 callconv `0`；
- 调用点本身**不要求 unsafe context**（`abi = "scoop"` 的 extern 函数不是 unsafe function，见 13.4）。

Scoop ABI extern 的参数与返回值使用普通 Scoop typed ABI，不经过 C ABI storage bridge：ref value 是直接 managed pointer；aggregate/value return沿用普通 Scoop 函数的 typed return storage规则。被调方必须按同一签名实现该 ABI，不能假设 C 编译器为同形 struct 选择的 ABI 与 Scoop value ABI 相同。

这里的 Scoop ABI 仍是普通、单次进入并在返回前完成的 FFI 调用约定，不是 8.2 所述挂起函数的 hidden continuation ABI。`abi = "scoop"` 不放宽 `@Extern` 与 `suspend` 的互斥规则，也不提供自动 continuation / callback wrapper。

注意：以上是**用 LLVM 实现时**需要的策略（LLVM GC / statepoint 体系的术语），描述的是参考实现的代码生成要求，不是语言语义本身。

### 14.3 direct ref、native root 与 safepoint

- Scoop ABI FFI 函数的机器码中**没有 safepoint poll**（它可能是用其他语言写的）。传入的 direct ref 是调用期间的借用 managed value：在被调方尚未执行可能触发 GC 的 runtime 调用或回调 Scoop 代码前，可以直接读取，无需 pin；不得写入长期存储或在返回后继续使用。
- 若被调方需要让某个 direct ref 跨越可能触发 GC 的操作，必须先把它写入可寻址的 **native root slot** 并把对应 root frame登记到当前线程。登记/移除 root frame本身不得分配或触发 GC；collector 必须扫描并在移动对象时更新这些 slot。操作返回后，被调方必须从 slot 重新读取引用，不能继续使用登记前保存的裸指针副本。
- root frame 必须按栈严格嵌套，并在所有正常/错误出口移除。需要把引用保存到本次调用之后时，使用 `GcHandle`；需要把稳定裸地址交给 C ABI 或跨 safepoint保持同一地址时，使用 `PinnedPtr`。两者都不是普通 direct-ref 参数的默认表示。
- GC 只在 managed safepoint或显式 runtime 入口协调线程；实现不得在 Scoop ABI native code的任意两条普通指令之间无握手地移动对象。未来的并行/并发 collector 必须把 native root frame 与线程握手纳入同一协议，不能通过要求所有 Scoop ABI 参数预先 pin 来回避该契约。
- `abi = "scoop"` 的调用点安全的前提是**被调方遵守上述契约**。这类底层 extern 声明按约定仅由 runtime / 核心库作者使用；违反借用、root frame或保活规则是 runtime ABI 错误。
- **GC-aware managed callback** 使用独立 registration协议：注册操作接收 ordinary、非挂起的 managed closure及与其 concrete函数类型匹配的 compiler-generated invoke adapter，返回 runtime-owned、GC-free的 opaque token。token内部以 `GcHandle` 保活 closure；native代码不得缓存 closure裸地址。
- native侧通过静态 C ABI trampoline和显式 context/user-data槽携带该 token。trampoline进入 runtime后 attach尚未注册的 foreign thread、切换到 managed执行状态、从 handle重新取得 closure并调用 typed adapter；返回 native前恢复线程状态。参数与返回值必须满足 C-FFI-safe约束，closure body内 GC正常生效。
- token具有显式 retain/release与 use-after-release错误边界；callback抛出的 Scoop异常必须在反向边界内捕获并转换为 status/受管异常handle，不得展开穿越 C frame。运行时终止后调用 token是 ABI错误。首版只支持原生API提供显式 context/user-data槽的形态；无此槽的任意 closure导出需要后续动态 trampoline或 slot registry。
- 该协议不改变 `FunPtr`（13.10）：M12 的 `FunPtr` callback仍是静态、同步、同线程、`@NoGC`路径。M13 落地 registration、foreign-thread attach/detach和多 mutator STW协调；suspend closure与 suspend FFI仍不支持。

M13 的 core 源码形态为：

```
enum ForeignCallbackMode { Reusable, OneShot }
enum ForeignCallbackState { Registered, Active, Completed, Failed }

struct ForeignCallback<F>(
    val function: FunPtr<F>,
    val context: Ptr<Unit>
)

@Unsafe
@Intrinsic("foreign_callback_register")
fun <F> foreignCallback(
    callback: Any,
    contextIndex: Int,
    mode: ForeignCallbackMode
): ForeignCallback<F>

@Unsafe
@Intrinsic("foreign_callback_retain")
fun <F> retainForeignCallback(callback: ForeignCallback<F>): ForeignCallback<F>

@Unsafe
@Intrinsic("foreign_callback_release")
fun <F> releaseForeignCallback(callback: ForeignCallback<F>)

@Unsafe
@Intrinsic("foreign_callback_state")
fun <F> foreignCallbackState(callback: ForeignCallback<F>): ForeignCallbackState

@Unsafe
@Intrinsic("foreign_callback_failure")
fun <F> foreignCallbackFailure(callback: ForeignCallback<F>): Throwable?
```

- `F` 必须在registration调用处显式给出，是ordinary、非挂起、完全具体化且逐项C-FFI-safe的native callback函数类型。`contextIndex`和`mode`必须为编译期常量；被选参数必须精确为`Ptr<Unit>`；
- `callback: Any`只是core声明无法表达“从`F`删除context参数”的占位，不执行装箱。HIR删除`F`中`contextIndex`对应的参数，以所得ordinary concrete函数类型检查managed closure；context cookie不作为实参传给closure，返回类型保持不变；
- `ForeignCallback<F>`是GC-free值，但不是单个C ABI聚合。调用native API时分别传`function`与`context`；有效值只能由`foreignCallback`产生，用户不能直接构造；
- `ForeignCallback`是compiler-validated core类型，其`F`可在core声明内作为deferred callback signature用于`FunPtr<F>`；每个实际应用仍必须具体化为合法函数类型。这不引入一般性的`function` kind bound，用户generic类型不能据此用未约束参数绕过`FunPtr`检查；
- `foreignCallback`与`retainForeignCallback`各产生一份逻辑ownership。普通值复制只是借用别名，不增加计数；每份ownership恰好release一次。`Reusable`由调用者在native API完成unregister并确认不再回调后释放；`OneShot`在native创建成功后把worker ownership转移给callback入口，创建失败则仍由调用者释放。join侧若需观察结果，必须在转移前另retain observer ownership；
- callback抛出时，trampoline按C签名返回全零值，token保存首个managed异常。完成同步后，observer用`foreignCallbackState` / `foreignCallbackFailure`读取并可在Scoop侧重新抛出；最终release通常置于`finally`。stale token、signature不匹配、one-shot重复调用或runtime终止后调用均为runtime ABI错误；
- `foreignCallback`只接受普通closure，不接受`suspend`函数值。普通callback可以捕获并调用`Continuation<T>.resume`；这仍不是suspend callback或suspend FFI。

### 14.4 示例：直接输出 managed `String`

Scoop 侧直接声明 managed-ref 签名；不需要 wrapper、pin 或 unsafe block：

```
@Extern(name = "scoop_rt_write", abi = "scoop")
fun write(message: String)
```

runtime 侧按 Scoop 的 `String` 对象布局直接接收引用：

```c
void scoop_rt_write(const ScoopString *message)
{
    fwrite(message->data, 1, message->len, stdout);
}
```

该函数只在调用期间读取 `message`，不分配、不调用可能触发 Scoop GC 的 runtime入口、不回调 Scoop代码，也不保存引用，因此无需 native root frame。若以后在写入前后增加任一可能触发 GC 的操作，必须先按 14.3 把 `message` 放入 native root slot，并在操作后重新读取更新后的值。

---

## 15. 明确排除（再次汇总）

为方便实现者，以下内容 Scoop 不支持：

其中GC finalizer是永久排除项，不是尚未实现：对象不可达时不会执行`finalize`、析构方法或任意managed回调，也不存在对象复活语义。native resource应显式`release`/`close`并用`try/finally`保证正常路径清理；未来只可能增加runtime spec 3.8所限定的GC-free release hook作为非及时、不可复活的兜底。该hook只能附着于具有唯一managed identity的`ref` owner；值类型的复制语义与隐式release ownership不兼容。

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
| GC finalizer、析构回调与对象复活 | 显式 `release` / `close` + `try/finally`；未来仅有GC-free release hook兜底 |
| struct 的 `init` 块 / `var` 字段 | 构造函数内逻辑 / `val` |
| 对值类型使用 `===` | `==`（结构相等） |
| `Array<T>` 的协变/逆变 | 显式转换或重新构造（见 10.4） |
| `Option<T>` 的智能转换 | `when` 解构（7.3） |
