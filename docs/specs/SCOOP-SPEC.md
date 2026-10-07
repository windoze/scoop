# Scoop 语言规范

本文规定 Scoop 的语法、类型规则、求值行为、核心库接口与 FFI 语义。对象布局和 GC 契约见 [运行时规范](SCOOP-RUNTIME-SPEC.md)，编译阶段与产物契约见 [编译器与产物规范](SCOOP-IMPL-SPEC.md)。

## 1. 概述

Scoop 是一门静态类型、编译到原生代码的编程语言。其语法以 Kotlin 的**核心语法**为基础，目标是：

- 尽量兼容 Kotlin 核心语法（类、接口、函数、泛型、控制流、协程等），不追求逐条全兼容；
- 排除一切与 JVM / JS / Kotlin Multiplatform 平台对接相关的扩展；
- 引入真正的**值类型**（struct / enum / tuple），并围绕值类型扩展解构与 `when` 的能力；
- 用 `Option<T>` 取代平台式的空指针语义；
- 提供基于核心库实现的字符串插值。

Scoop 的泛型采用**单态化（monomorphization）**，不进行类型擦除。

### 1.1 范围

本规范只覆盖：

- Scoop 的核心语法；
- 支撑核心语法所需的**最小核心库**（见第 11 章）。

除 11.10 为字符串构建提供的最小 `List` / `MutableList` / `ArrayList` 和 11.14 的原子类型外，标准库的其他部分（其他集合、IO、高层并发工具、序列库等）不在本规范范围内。

### 1.2 并发安全与数据竞争

Scoop 不提供类似 Swift / Rust 的并发安全保障，不通过类型系统保证跨线程共享没有数据竞争，也不保证在编译期诊断此类竞争。需要并发安全时，程序必须显式使用 11.14 的原子操作或同步原语（如以后由库提供的 mutex / lock），并遵守其同步契约。

- **happens-before** 由线程内的求值顺序、原子或线程同步原语按其契约建立的跨线程同步关系及其传递闭包组成。原子性本身不等于对其他存储位置的访问也建立了同步。
- **数据竞争**：不同线程访问同一或重叠的存储位置，至少一个访问是写、至少一个访问是非原子访问，且这些冲突访问之间没有 happens-before 顺序。此规则同样适用于普通字段、数组元素、普通全局存储、`@Global` 及 native 内存。
- **数据竞争一律是未定义行为**：不保证内存安全、读到的值有效或不发生撕裂；由竞争引起的撕裂同样属于未定义行为。引用、标量、struct、tuple 和 enum 一视同仁，不提供单字不撕裂、逐字段有效或 tagged enum 之外的特殊保证。
- 跨线程发布对象和后续读写共享状态都必须满足相应的同步要求；普通引用和字段的 `val` 声明不自动提供同步。通过原子量发布对象时，必须使用能建立所需同步关系的内存序；读取原子引用不自动保护所指对象的后续可变状态。
- GC 保活、pin、safepoint、native 状态切换及 GC 写屏障不替代用户数据的同步，也不为有数据竞争的程序提供内存安全保证。语言对正确同步程序的值、布局和 GC 契约仍然适用。

---

## 2. 与 Kotlin 的关系

### 2.1 包含的 Kotlin 核心语法

除本规范明确修改或排除的部分外，Scoop 尽量兼容 Kotlin 核心语法，包括但不限于：

- 声明：`class` / `interface` / `object` / `companion object` / `typealias` / 属性（`val` / `var`，含委托属性）/ 函数 / 扩展函数与扩展属性；
- 类特性：主构造函数与次构造函数、`init` 块、继承（单继承 + 接口实现）、抽象类、可见性修饰符（`public` / `internal` / `private` / `protected`）；
- 数据与函数：局部函数、lambda、匿名函数、函数类型、callable reference、默认参数、命名参数、可变参数（`vararg`）、中缀函数（`infix`）、运算符重载、尾递归（`tailrec`）；
- 泛型：类型参数、完整类型实参、上界约束、`where` 子句与调用点类型推断；Scoop不采用Kotlin的声明点型变、使用点投影或star projection（见3.2）；
- 控制流：`if` / `when` / `for` / `while` / `do-while`、区间与迭代、`break` / `continue` / `return`（含标签）、异常（`try` / `catch` / `finally` / `throw`）；
- 空安全运算符：`?.` / `?:` / `!!`（语义见第 7 章）；
- 类型运算符：`is` / `!is` / `as` / `as?`、智能转换（smart cast，见 2.3）；
- 协程：`suspend` 函数与挂起调用（见 8.2）；
- 上下文参数：保留 `context(name: T)` 表面，采用 task-local 的运行期精确类型解析（见 8.3）；
- 编译期注解、静态类型描述，以及由显式 interface conformance 请求的方法合成（见 9.4～9.6、11.13）；不引入通用编译期执行语言。

### 2.2 排除的内容

以下 Kotlin 功能**不属于** Scoop：

- 平台对接：
  - `expect` / `actual`（multiplatform）；
  - `@JvmStatic` / `@JvmField` / `@JvmOverloads` / `@Throws` 等一切 JVM 互操作注解；
  - `external` 声明（FFI 通过 `@Extern` 注解机制提供，见第 13 章）；
  - Java 互操作语义（SAM 转换、平台类型 `T!` 等）。
- 运行期反射：`KClass`、`::class` 的运行期反射 API、`kotlin.reflect` 体系。9.6 的静态描述只在编译期消费，不提供运行期字段枚举、按名称取类型或反射构造。
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
- **值类型（value type）**：`struct`、`enum`、`tuple`。无 identity，immutable，复制时复制完整值。

### 3.1 顶层与底层类型

- `Any`：所有类型（引用类型与值类型）的根类型。值类型向上转型为引用类型时发生**装箱**（见 4.4.4）。`Any` **没有任何成员方法**：值相等通过 `Equality<T>` 的 `equals` 决议，字符串化与哈希分别通过独立的 `ToString` / `Hash` 接口（见 11.11）。
- `Nothing`：所有类型的子类型，无实例。结果类型为 `Nothing` 的表达式没有正常完成路径；它不是 `Unit`，也不产生可供装箱、复制或返回的值。`Nothing` 与 `Any` 均由 11.1 的真实 core intrinsic class 声明提供，名称查找、alias、可见性与依赖消费遵守普通声明规则。
- `Any` 使用 managed reference 表示；`Nothing` 的名义类别也是引用类型，满足 `ref` kind，但不存在合法的非空引用。两者没有源码构造入口、字段或成员，也不接受类型实参或显式继承／implements 列表。所有类型到 `Any`、`Nothing` 到所有类型的关系由对应 intrinsic 的顶／底类型语义产生，不要求或允许在普通声明的继承列表中写出这两个根类型。
- 从 `Nothing` 到任意目标类型的适配只保留原求值及其控制转移，不生成装箱、引用转换、目标值或正常返回。要求 Boolean 的条件、guard 和短路运算数同样接受 Nothing；短路分支仍遵守原求值规则。`LUB(Nothing, T) = T`；只有不正常完成的分支时，控制表达式的类型为 `Nothing`。类型名或同形普通 class 不会获得这些规则。
- 元组字面量具有预期元组类型时，`Nothing` 元素可占据对应的预期槽位；元素表达式仍保留 `Nothing` 类型和原求值，整个构造没有成功完成路径。这不使已存在的元组或不变泛型类型获得协变。
- `e is Nothing` 在 `e` 正常求值后为 `false`，`e !is Nothing` 为 `true`；`e as Nothing` 在求值后抛出 `ClassCastException`，`e as? Nothing` 为 `None`，不省略 `e` 的副作用。`Nothing` 可用于普通签名与泛型实参，例如 `() -> Nothing`、`Option<Nothing>`；不变泛型仍不因其底类型实参产生协变。`Nothing` 不提供可调用的成员或新的 C ABI storage 类型。

### 3.2 泛型

声明来自当前 Cone、普通依赖或 core 产物，不改变类型 application、bound、调用推断、默认值、成员或模式的语言规则；源码名称可达性与可见性仍按 12.4、9.1.5 检查。泛型正文在定义处绑定名称、重载和成员契约，实际类型替换不能重新选择定义处未选中的重载。普通声明中的封闭 application（如 `Box<Int>`、`I<Int>`）是完整类型，不因其原定义为泛型而失去普通类型的使用能力。

- 泛型在编译期**单态化**实例化：每个具体类型实参生成一份专门的代码。
- struct、enum 和 tuple 的内联值布局必须有限。字段或 payload 经实际内联的泛型形参返回同一值类型声明、且途中没有引用或指针边界时，在声明处报错；改变环上的类型实参不能消除此错误。此规则同样适用于来自依赖的泛型包装器。没有存入字段／payload 的 Phantom 参数以及仅位于引用或指针之后的参数，不构成内联布局依赖。
- function、class、struct、enum 与 interface 都可以声明类型参数。构造器、变体、基类与接口、字段及成员可以使用宿主类型参数。每个完整的名义类型 application 具有独立的类型身份；不同 application 不因布局相同而成为同一类型。
- Scoop没有预定义`Self`类型、associated type或“当前实现者类型”的隐式占位符；`Self`也不是关键字，若出现在源码中只按普通名称解析。generic/interface契约若需要表达某个类型关系，必须用显式nominal type application或显式type parameter表示，编译器不执行`Self := 实现类型`替换。`Encodable<T>`与`Decodable<T>`同样遵守此规则。
- 泛型调用与泛型值构造的类型实参由整组实参共同约束，推导结果不得依赖实参声明顺序。依赖期望类型的实参（如 `None`、空数组或嵌套泛型构造）可以由任意其他实参先绑定类型参数后再完成检查；类型检查顺序不决定运行期求值顺序，显式实参与缺省表达式严格按 8.5.3 求值。
- 调用点可以写显式类型实参：`f<Int>(value)`、`Box<String>(value)`、`Enum.Some<Int>(value)` 与 `receiver.convert<String>()`。列表仍须覆盖callee自己声明的全部参数位置，但任一位置可以写`_`请求继续推断，例如`convert<Int, _>(value)`或`Pair<_, String>(first, second)`；显式类型与`_`产生的fresh variable进入同一个candidate-local constraint system。`_`只在调用/构造的显式type-argument list中合法，不是类型，不能出现在变量、字段、返回类型、上界、cast目标或nominal type annotation中。泛型宿主的方法调用只列method自己的参数，宿主application仍由receiver确定；整组省略时继续使用普通推断。
- generic type application在类型位置必须覆盖全部参数位置；不支持裸generic type或少写参数，每个位置都必须是普通完整类型。`G<out T>`、`G<in T>`与`G<*>`均不是Scoop类型语法。generic class/struct constructor及enum variant构造产生exact application，可以整组省略实参或用`_`部分推断。
- primary/secondary constructor与enum variant constructor不声明独立type parameter；构造调用中的显式/推导实参只对应nominal host。只有普通callable可以在generic owner参数之外再拥有一组callable参数。
- 因此不存在类型擦除，也没有 `reified` 的运行期需求（见 8.4）。
- class、struct、enum与interface的nominal type parameter一律不变（invariant），声明处不能写`in`或`out`。同一nominal template的两个application只有全部类型实参逐项相等时才存在由该template产生的赋值/子类型关系；`G<S>`不会仅因`S <: T`成为`G<T>`的子类型。class的普通继承及interface的显式conformance仍可把一个concrete类型映射到声明中写出的某个exact base/interface application，例如`D : B<String>`仍使`D <: B<String>`，但不推导`D <: B<Any>`。
- 不变性同时适用于reference与value type argument：编译器不会为generic application的赋值、返回、分支合流或参数传递自动重建value layout，也不会把value argument装箱后改成另一个application。需要改变容器/包装类型实参时，程序必须显式构造、`map`/复制，或先把单个value显式转换为共同class/interface再构造目标exact application。
- 继承与implements闭包只合并完全相同的interface application；同一template的`I<Int>`与`I<String>`始终是不同契约、不同RTTI/itable identity。一个类型可以在继承图中到达二者，但必须分别满足其成员obligation；若替换后的签名无法由普通overload/override规则同时实现，则在实现类型定义处报错，不能按template id或擦除后的文本签名任选其一。
- Scoop不提供use-site `in`/`out` projection、star projection或wildcard capture。需要只读/只写抽象时，优先让消费操作本身成为带bound的generic callable，例如`fun <T : Animal> consume(values: Array<T>)`；需要保存未知application时，必须声明显式的非generic interface或用户实现的type-erased wrapper。语言不会隐式制造existential类型、runtime generic dictionary或capture-open dispatch。
- 除类型上界外，类型参数还可以用 `value` / `ref` 约束限定为值类型或引用类型（见 13.9）。
- 类型上界在参数列表中写作`T : Bound`，或在声明头后的`where T : Bound`子句中给出。同一参数至多有一个class上界，并可同时具有多个不同interface上界；class上界保证实际参数是该exact class application的引用子类型，成员候选包括class及其继承闭包。class/interface上界都必须是参数完整的exact reference application；不接受value type、函数类型、`Any`或另一type parameter，也不产生可作为普通表达式类型的交叉类型。`value` / `ref` kind bound与任一nominal上界互斥（见13.9）。
- 类型实参必须同时满足参数的全部上界；class/interface关系按普通继承、显式conformance及完整application identity判断。11.11 的值类型条件派生可额外产生真实的 `Equality<完整宿主类型>` conformance，其他接口仍须显式声明，不能按同名成员推导。泛型推导把上界纳入整组约束求解，不得先任选一个类型、再把bound失败降为警告或回退为`Any`。11.13的编码与解码能力属于显式codec值，不为目标数据类型增加特殊bound或结构型conformance。
- 上界的完整 interface application 可以引用被约束参数自身，例如 `T : Equality<T>`；它表示实际类型必须实现 `Equality<该实际类型>`，不是裸参数上界 `T : T` 或隐式 `Self`。所有形参先建立身份，再解析 bound 中的参数引用；普通继承环与非法 bound 规则仍适用。
- receiver为有界type parameter时，成员候选只来自唯一class上界、interface上界及其继承闭包；不加入`Any`成员或实际类型未在bound中声明的能力。generic template中的bound member在实例化时解析为concrete direct/virtual/interface call；单态化不需要runtime dictionary，但不取消actual concrete type本来具有的动态分派语义。 class 上界中的属性同样参与查询：存储、计算属性和函数值属性使用完整上界实参及其继承替换；读取、赋值、复合赋值和安全访问保留原 receiver 的访问域检查，getter／setter 在定义处选定。该规则与声明位于当前 Cone 或依赖无关。
- 每个合法且参数完整的exact class/interface application都是普通reference type，可以直接作为变量、参数、返回值、字段、cast目标和upper bound。Scoop不引入`dyn`/trait object语法、object-safety分类或可空witness；所有合法interface成员仍可经concrete、exact interface或bounded receiver调用。`Encodable<T>`、`Decodable<T>`及其方法也使用普通interface语义。
- interface 方法不能声明自己的类型参数；`interface I { fun <T> f(value: T) }` 是编译错误。interface 宿主可以是泛型，例如 `interface I<T> { fun f(value: T) }`，完整 application `I<String>` 中的方法遵循普通接口调用规则。
- non-interface generic method必须non-virtual。class generic method必须语义为final；generic method不能声明为open/abstract/override，不能实现或覆盖vtable/itable slot。struct/enum方法本来即为final。调用根据exact receiver application与完整method argument使用direct dispatch，运行期派生class不能override目标。
- class/struct/enum泛型宿主上的泛型成员函数，以及带宿主类型参数环境的companion上的泛型成员函数，都拥有两组类型参数：宿主类型实参由接收者的exact静态application确定，方法类型实参由整组调用实参推导；两组实参共同组成该方法的单态化身份，顺序固定为“宿主类型实参在前、方法类型实参在后”。两组参数使用不同semantic identity，方法参数不得与宿主参数重名。companion的宿主实参来自9.1.3规定的完整宿主application，不从方法实参反推。调用点显式列表只写method自身参数，可整组省略，或覆盖全部位置并在待推断位置写`_`；两组参数的bound一起验证。interface本身的方法不能声明第二组类型参数，其companion的方法属于普通object成员。
- top-level、local与extension generic function，以及上述non-interface generic method，都可以使用inline upper bound与`where`。generic method的callable reference必须由期望函数类型唯一确定method全部实参，得到的是某个concrete函数值；Scoop没有first-class polymorphic function value。
- 所有type parameter declaration都不能写`in`/`out`。callable参数与返回类型在推导中的方向由constraint solver处理，不通过声明点variance修饰符表达；普通函数类型自身的参数逆变/返回协变继续按8.1.1处理，它不是nominal generic application之间的variance。
- `is` / `as` / `as?`不擦除generic argument，generic nominal目标必须是参数完整的exact application。例如`Box<Int>`与`Box<String>`、`I<Int>`与`I<String>`是不同检查目标，前者不会仅因`Int <: Any`匹配`Box<Any>`。generic body中的type parameter在普通单态化后引用concrete TypeDescriptor；不存在裸generic、star或projected runtime descriptor。
- 单态化必须结构上保证实例化闭包终止。同一generic callable递归SCC中的每个调用环，把宿主参数与callable参数组成的完整向量代回起点后必须逐项保持identity；普通直接/互递归因此复用同一concrete实例。参数替换非identity的环属于禁止的polymorphic recursion，在template定义检查时报错。非递归调用边仍可任意变换实参。编译器不得用递归深度、实例数量或超时阈值决定源码是否合法。

#### 3.2.1 非泛型透明 `typealias`

跨 Cone 的 `Alias(...)` 与本地别名遵循相同规则：先按真实 typed target 展开，再对目标声明的构造器执行普通候选决议。外来 alias、本地指向外来类型的 alias 及其链式组合保留目标的构造器身份、默认参数、访问域与 GC effect，不生成转发构造器。

别名限定的构造调用保留目标 application 的全部固定实参，包括 `Alias.Variant(...)`；实参表达式不能重新推断并替换这些类型实参。即使调用结果被赋给 `Any` 或没有显式结果类型，payload 与构造参数仍按别名展开后的目标类型检查。

支持top-level、非generic透明alias，例如`public typealias UserId = UInt64`、`typealias Names = Array<String>`。右侧必须是声明点可访问、参数完整的普通类型，可以是nominal application、tuple或函数类型；alias不能声明type parameter、捕获外层type parameter或出现在nested/local位置。alias可以引用其他alias，但展开依赖图必须无环；直接或间接递归均为定义处错误。

alias只建立名称、declared visibility、source origin与导出实体，不建立新的type identity、nominal application、layout、TypeDescriptor、RTTI、boxing、单态化实例、overload差异或ABI分类。alias的类型语义等同于完全展开的目标type；分别以alias和目标声明的同形overload是重复签名。alias可用于目标本来允许出现的type及type-qualifier位置；`Alias(...)`、`Alias.Variant(...)`或static/companion member lookup先展开目标再执行普通决议，不产生额外候选。visibility默认仍为`internal`；完全展开目标type tree中每个被引用声明的effective access domain必须覆盖alias自身的effective domain。因此internal alias也不能暴露file-private目标，public alias只是同一signature-exposure规则的更宽特例。非泛型顶层 typealias 可经普通 import、public import 及链式 re-export 跨 Cone 使用，别名保持原声明身份，使用处展开到同一目标类型。不支持泛型、嵌套或局部 typealias。

### 3.3 参数传递、receiver 与 `this`

Scoop 的源码函数/方法调用一律是 **pass-by-value**：每个实参表达式求值一次，再以所得值初始化 callee 中不可重新绑定的形参 binding。这条规则不按 value type / reference type 分叉：

- 实参为 value type 时，被复制的是完整值；callee 不获得调用方 binding/place 的别名。
- 实参为 reference type 时，被复制的是 ref value，不是其指向的对象。调用方与 callee 因此持有两份相等的引用值，指向同一具有 identity 的对象；重新绑定任一引用 binding 不影响另一份引用，但通过它们对 referent 所做的合法修改对另一方可见。这与 Java 的引用参数语义一致，不是 pass-by-reference。

成员方法、计算属性 getter 及扩展函数的 receiver 使用同一规则。调用 `receiver.method(args...)` 时，receiver 只求值一次，并按值初始化隐含的、不可重新绑定的 `this: T` 参数；`this` 不是调用方 receiver binding 的别名。因此：

- value-type `this` 是 receiver 完整值的 method-local copy；值类型字段不可写，`this` 自身也不能被重新绑定。
- ref-type `this` 是 receiver ref value 的 method-local copy；它不能被重新绑定，但可以按 referent 类型的普通成员规则读写同一对象。
- 方法返回的 closure 若引用 `this`，按 8.1.3 捕获的就是这个隐含参数的值：value-type `this` 复制完整值，ref-type `this` 复制引用值。两者都不保留调用方 receiver binding/place。
- 经装箱值的 interface 分派调用值类型实现时，dispatch thunk 语义上用 box payload 的值初始化 value-type `this`；box identity 不会成为该 `this` 的 identity。

按值传递不得使 callee 取得调用方 place 的别名。当 value receiver 直接或间接含 `@InteriorMutable` value、出现 `addressOf(this)`，或其他 unsafe 操作观察存储时，观察到的必须是独立的方法局部副本；其地址不得指向调用方 binding 或 box payload，有效期不超过当前方法调用。

---

## 4. 值类型

### 4.1 struct

`struct` 定义一个聚合值类型。

```
struct S(val f1: Int, val f2: String = "")
```

规则：

- 字段在主构造函数中声明，与 Kotlin 类的构造函数属性语法一致，但**只能用 `val`，不允许 `var`**（struct 不可变，`var` 字段无意义）。
- **字段不支持可见性修饰符，declared visibility固定为public**；effective domain仍与struct owner取交集（9.1.5），因此默认internal struct的字段不会导出Cone。
- **immutable**：struct 实例构造完成后不可修改。
- 支持主构造函数与次构造函数（secondary constructor）。secondary参数不声明字段，必须通过`this(...)`直接或间接委托到唯一primary；具体参数、委托、初始化与失败规则见4.1.1和9.1.1。
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

struct可以在body声明secondary constructor：

```kotlin
struct NonZero(val value: Int) {
    constructor() : this(1) {}
}
```

secondary参数使用8.5的required/default/`vararg`协议但不能写`val`/`var`，也不声明自己的type parameter或返回类型。每个secondary必须以`this(...)`直接或间接终止于primary；省略delegation、使用`super`、自环或多constructor环都是定义处错误。委托先产生完整immutable struct值，再从最内层到最外层执行secondary body；body可以读取字段、执行同步副作用或`throw`，不能修改字段、`return`或发布尚未完成的`this`。任一delegation/body抛异常时整个构造不产生值。

#### 4.1.2 派生行为

struct 自动获得：

- 结构相等：全部字段可比较时，条件派生 `Equality<完整 struct 类型>` 及逐字段比较的 `equals` 实现，既可用于 `==`，也可满足接口 bound（见 11.11）；
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
- **variant及其字段不支持可见性修饰符，declared visibility固定为public**；effective domain仍与enum owner取交集（9.1.5）。
- **enum 自身不支持构造函数、不支持 `init` 块、不支持成员属性**；body 中只允许声明成员函数与伴生对象（变体的构造函数式声明是变体定义的一部分，不在此限）。
- 每个变体是一个构造器：`E.SimpleVariant`、`E.VariantWithValue(42)`、`E.VariantWithNamedField(f1 = 1, f2 = "x")`、`E.VariantWithDefaults(1)`、`E.VariantWithDefaults(f1 = 1)`。block 式命名字段变体只能以命名参数构造；构造函数式变体沿用 struct 主构造函数的参数规则，允许位置参数、命名参数及默认值。两者均不提供花括号构造形式（与 struct 字面量同样存在解析歧义）。
- 跨 Cone enum 使用相同的变体构造、import/typealias、期望类型和默认参数规则；泛型变体的宿主实参由显式实参、payload 与期望 enum application 按普通约束推导确定。`E.V` 中的裸泛型 `E` 是声明命名空间，不先构造缺失实参的类型。值构造按实际声明身份生成，不以 provider 来源授予额外资格。
- enum 拥有 generic companion 不改变上述变体规则：限定调用先按实际静态 binding 识别变体，只有目标属于 companion 时才要求完整宿主实参（9.1.3）。变体构造不访问或初始化 companion。
- **变体不是类型**：不能用作 `is` 的检查目标、变量类型或参数类型；判断与提取负载通过 `when` 模式（第 5 章）完成。
- 与 Kotlin enum class 的 entries 类似，变体名可以通过`import some.package.E.*`引入后不写前缀直接使用；`scoop.core.Option.*`由core prelude的typed default import引入（见第7章），`Some`/`None`不具有短名称特判。
- 表达式位的裸`V`/`V(...)`除普通可见候选外，还可由唯一的expected exact enum application `E<Args...>`引入：只在该enum内寻找同名variant。expected type仍未固定时，该构造与`None`、lambda、空数组一样进入8.6的candidate-local postponed检查；最终expected为`Any`/interface、多个enum或未解变量时，不扫描全程序猜测，必须写`E.V`或补type annotation。普通词法/import候选遵守既有分层并优先；contextual variant不能绕过遮蔽，也不能扩展为按返回类型选择普通函数。unit variant的contextual name只在普通value-name lookup没有找到实体时启用，词法value binding即使类型不适配也hard-shadow该回退；payload variant call继续服从8.6既有named-call与local-value shadow规则。
- 在`when`匹配处，变体名可以省略`E.`前缀，由subject的exact enum type解析（见第5章）；这与表达式位的contextual candidate是两个不同入口。
- 与 struct 一样：immutable、无 identity，可条件派生 `Equality<完整 enum 类型>` 及结构相等实现；`ToString`与`Hash`必须显式adopt并实现（见11.11）。
- 命名字段变体的字段构造后只读。
- enum 可以实现 interface（见 4.4.3）。
- 泛型 enum 允许，例如核心库的`enum Option<T>`（见7.2）；不同类型实参形成互不转换的exact application。

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
- tuple 是值类型：immutable、无 identity；当全部元素可比较时条件派生 `Equality<完整 tuple 类型>` 及结构相等实现，Unit 无条件实现 `Equality<Unit>`（见11.11）。
- tuple 不支持在源码中显式声明implements列表，也不支持命名字段，不实现`ToString`。除 11.11 的 Equality 派生外，不按 tuple 结构自动获得其他接口。编码与解码分别由普通`Encodable<(T1, T2, ...)>`、`Decodable<(T1, T2, ...)>`实现承担，见11.13；不为tuple添加companion或编码／解码的条件接口。需要数据值自身实现其他interface时使用命名struct显式声明。

### 4.4 值类型通用规则

以下规则适用于 struct、enum、tuple 三者。

#### 4.4.1 不可变性

值类型实例构造后不可修改。对具有字段名的struct，可通过副本更新表达式创建新值（见4.5）；enum与其他值类型必须显式重建。也可以把任意新值重新绑定到`var`变量（这是重绑定，不是原地修改）。

#### 4.4.2 无 identity

值类型没有 identity：

- 对值类型使用 `===` / `!==` 是**编译错误**；
- 相等判断一律使用 `==`（结构相等）。

值类型装箱为引用类型后，其装箱结果按引用类型规则处理（见 4.4.4）。

#### 4.4.3 实现 interface

值类型可以实现 interface，但**不得因此获得可变性**：

- interface是普通nominal reference type；任何合法且完整的interface application均可承载class ref或装箱后的value，不存在trait object转换或“该interface是否object-safe”的额外判定；
- 实现 interface 方法或只读属性必须使用 `override`；值类型方法和 computed `val` 的 getter 始终为 final，省略修饰符的 `override val` 同样正规化为 final，显式 open/abstract 属性仍是编译错误。它们不参与 vtable 分派，但装箱为 interface 后通过该 interface 的 itable 分派；
- 实现 interface 的成员函数、属性 getter 不得修改 `this` 的任何字段（值类型字段本来就不可写，此规则是自然推论）；
- 若 interface 契约要求可变行为（例如要求实现 `var` 属性的 setter），值类型实现它是**编译错误**。

跨 Cone 的值类型遵守同一接口关系：struct、enum 及具有 intrinsic 表示的值类型从实际声明取得其接口和父接口，赋值、参数、返回及显式转换均使用相同的子类型规则。导入不能丢弃接口关系；同名接口、同布局值类型或 provider 身份不能替代真实类型身份。

整数和 Boolean 的接口集合由实际声明确定，包括 `ToString`、`Hash` 及用户新增接口；intrinsic 表示不替代这些声明。声明来自依赖 Cone 时，装箱和接口转换遵守相同规则。

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
- 装箱后的对象具有 identity（可用 `===` 比较）且immutable。装箱不额外赋予相等能力：`==`始终按装箱后表达式的**静态引用类型**查找可用的 `Equality<T>.equals` 契约；`Any`不提供该接口，未继承适用 Equality application 的interface也不能比较。值类型已有的派生 Equality 使用普通装箱与接口调用规则；经`as`/模式匹配取回原value后，按原value的静态类型决议。
- **auto-boxing 只发生在 O(1) 场景**：单个值的转换（赋值/初始化、函数实参、返回值等单点转换）允许自动装箱；数组字面量的元素位置等批量场景不做自动装箱，需要显式 `as`（见 10.3）。
- `is` / `as` / `as?` 可用于判断与取回装箱前的值类型；`as?` 失败时返回 `None`（见第 7 章）。
- 值类型在类型系统上也是 `Nothing` 的父类型；向 `Nothing` 的检查与转换遵守 3.1，不会成功产生一个底类型值。

### 4.5 副本更新表达式

副本更新表达式基于一个既有struct值创建“修改了部分字段”的新值：

```
struct S(val f1: Int, val f2: Int)

val s1 = S(10, 20)
val s2 = s1.{ f2: 30 }    // 新值：f1 = 10, f2 = 30
var s3 = S(1, 2)
s3 = s3.{ f1: 84 }        // var 重绑定，不是原地修改
```

规则：

- 形式为非空`baseExpr.{ field: expr, ... }`。`.`之后的`{`进入专用字段列表，因此不需要新关键字；块内不能出现语句、rest或嵌套field path。
- `baseExpr`先求值且只求值一次，静态类型必须是exact declared struct value；enum（包括具有命名字段payload的variant）、class、interface、tuple、basic type、intrinsic value family与函数值均不支持。结果为完整新值，原值及其storage不发生原地修改。
- 更新字段不能重复。目标确定后，各RHS按源码从左到右各求值一次并必须可赋给对应exact field type；未提及字段从保存的base值复制，最终按字段声明顺序构造结果。任一RHS抛出或挂起时后续RHS不求值，已发生的外部副作用不回滚。
- 字段只由base的exact struct identity与源码字段名确定；未知或重复字段、以及RHS无法赋给对应exact field type，均为编译错误。RHS type不参与目标选择。
- enum不是副本更新目标；即使其某个命名字段variant恰好包含全部所写字段，也必须用`when`匹配并显式重建。编译器不为副本更新生成active-variant检查、payload投影或variant不匹配异常路径。
- 不支持穿透`Option`的写法（`opt?.{ f: 1 }`）：先用`when`拆包，再做副本更新。

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

本节定义 **binding pattern**，用于 `val` / `var` 声明、`for` 循环变量和 lambda 参数，必须对输入静态类型递归不可失败。第 5 章的 **match pattern** 额外允许 literal 与 enum variant；这些可失败模式在 binding 位置非法，嵌套在 tuple 或 struct 中也不例外。`for (Some(x) in values)` 不是过滤语法。

binding上下文中，除`_`以及4.3定义的内建Unit字面量写法`Unit`/`()`外，语法上只有一个普通标识符的pattern在任意嵌套深度都恒为新的binding；它不按subject type查询enum variant，也不查询import或既有value。例如`val None = o`合法并声明名为`None`的新binding，即使`o`的类型拥有或当前作用域导入了`None` variant。`Unit`与`()`仍是literal而不是binding；binding pattern不接收literal，所以二者在`val`/`var`、lambda与`for`中拒绝。对enum variant而言，只有显式写出variant shape的`E.V`、`V()`、`V(...)`或`V { ... }`（包括其限定形式）并按subject exact enum解析成功时，才会被识别为refutable variant pattern并在这些binding位置的任意深度拒绝；同形语法若解析为struct pattern，仍按本节的irrefutable struct规则处理。第5章“enum variant优先于binding”的规则只属于match pattern，不适用于本节。

规则：

- **tuple/struct固定元数位置模式**`(p1, p2, ...)`按位置解构；每个位置可以是绑定名、`_`、`..`或递归binding pattern。不使用`..`时元数必须与被解构值一致。class component位置模式不适用该元数规则，单独按下文处理。
- **字段模式**由`field`、`field: subpattern`和末尾可选`..`组成。`field`精确等价于`field: field`，展开后的RHS仍按所在上下文的完整规则分类；因此在本节的binding上下文中通常建立同名binding，但字段名恰为`Unit`时，RHS仍是内建Unit literal并被拒绝，需要写成`Unit: value`等显式不同绑定名。第5章的match上下文先应用同一个Unit literal特例，再对其他名称应用variant-first规则；若字段的exact enum type恰有同名unit variant，shorthand会匹配该variant，需要catch-all binding时应显式写成`field: value`等不同名称。`field: renamed`绑定到`renamed`；`field: _`显式忽略该字段。冒号右侧是完整递归pattern，不再只是rename。`_`不能作为字段key，但可以作为RHS。字段不能未知或重复，整个pattern内的binding名称必须唯一。
- 每个显式字段名必须属于 subject 的 exact type。位置模式按字段声明顺序匹配；命名字段模式按源码书写顺序递归执行。末尾 `..` 忽略未列出的字段，不读取这些字段。未列出全部字段时必须以 `..` 结尾；列全时可省略。字段模式可带类型前缀（`S { f1: x, .. }`），前缀必须解析到 subject 的 exact type。
- struct既可按主构造字段顺序位置解构，也可按字段名解构；tuple/struct投影是语言内建能力，不查找`componentN`。
- 普通class可以按位置使用9.3的`componentN` operator解构。每个实际位置按顺序选择并调用唯一typed operator；写出的N个位置精确调用`component1`至`componentN`。每次调用正常返回后，结果先保存到新的immutable hidden temporary，再depth-first处理对应子模式。class没有声明式总元数，因此该位置模式不能包含`..`；调用可以按普通规则抛出或在允许的上下文挂起，但pattern本身没有“匹配失败”分支。挂起调用以异常恢复时等价于在原调用点抛出，后续动作不执行；在不允许挂起的求值上下文中选到`suspend componentN`是effect错误。
- binding pattern的不可失败性递归成立：binding、`_`、rest补位不可失败；tuple/struct只有全部展开后的子模式不可失败时才合法；literal（包括`Unit`/`()`）与上述显式enum variant shape在binding位置均非法。lambda和`for`引入的binding不可重新绑定；`var`解构声明中只有用户可见的叶binding是mutable，subject、投影及component结果等hidden temporary始终immutable。
- 三种binding位置共享同一求值、投影与component计划；每个完整subject只求值一次并进入immutable hidden temporary，位置元素按源码从左到右、命名字段按源码书写顺序递归depth-first执行。任一component调用抛出时，已经发生的外部副作用不回滚，后续投影、调用和binding均不执行；调用挂起时保存subject与全部已完成的hidden temporary，恢复后从该调用返回之后继续，不重新求值subject或任何已完成步骤。

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

- 在tuple、struct及enum variant的固定元数位置模式中：`..`忽略任意数量（含0个）的连续元素，可出现在开头、中间或结尾，但**至多出现一次**（出现两次会产生歧义，是编译错误）；使用`..`时不要求模式元数与值的元数一致。class component位置模式始终禁止`..`，不适用本条。
- 在字段模式中：`..` 只能出现在最后，表示忽略所有未列出的字段；**未列出全部字段时必须写 `..`**（列出全部字段时 `..` 可省略）。
- `..` 自身不绑定任何值。

#### `..` 与区间运算符的消歧

`..` 同时是区间运算符（`1..4`，见 11.8）。两者不会歧义：

- rest 模式只出现在**模式位置**（解构声明、when 模式分支、for 循环变量、lambda 参数）；
- 区间运算符只出现在**表达式位置**（普通表达式、when 的 `in 1..4` 分支等）；
- 模式元素只接受绑定名、字面量、`_`、`..` 或嵌套模式；裸 `..` 表示 rest，`expr .. expr` 是非法的模式元素。

### 4.7 零尺寸值类型（ZST）

零尺寸是完整值类型的结构属性，与目标无关；目标只决定其正 alignment 及 ABI。`sizeOf<T>() == 0uL` 的值类型称为 ZST；它仍保留自己的类型身份、泛型实参、方法、构造过程与 TypeDescriptor，不能因布局相同而与另一类型合并。

- `Unit` 和空的普通非 `@CLayout` struct 固定为 size 0、alignment 1；非空 tuple 或普通非 `@CLayout` struct 在全部字段都是 ZST 时也是 ZST，alignment 取字段最大值且为正的 2 次幂。ZST 字段不增加 aggregate size，可共享 offset；没有独立的源码 over-alignment 语法。tagged enum 仍保留 tag，`Option<ZST>` 必须能区分 `None` 与 `Some`。
- ZST必须是GC-free且递归scan为空；反向不成立。它没有可区分值的存储bit，但构造器、initializer、getter、函数调用、array literal元素、赋值RHS及用户方法的求值和副作用一律保留。复制ZST不复制字节，不得借此删除产生该值的求值；
- ZST值仍遵守4.4.2的“无identity”。需要`addressOf`的独立source place必须在其有效期内物化满足alignment的非null 1-byte **address token**；同时存活且语义上不同的address-taken place不得共址，生命周期不重叠时可复用。token只提供place地址，不把语言值的size改成1，也不允许读写该token作为payload。字段和array元素仍不在13.10现有`addressOf` lvalue集合内；
- Scoop ABI 不传递 ZST payload，但保留完整参数和结果类型身份。参数表达式仍按顺序求值；地址被观察时，callee 具有自己的 place token。`Unit` 的 void result 与其他 ZST result 不因此成为同一类型、重载或 callable。
- 装箱ZST仍分配带对象头的普通managed object，每次装箱产生普通ref identity；未装箱payload size为0、scan为空，但TypeDescriptor以运行时规范 2.2的`BoxedValue { ZeroSized }`保存对齐后的payload offset与非零minimum object allocation。不同exact ZST继续使用不同TypeDescriptor，即使最终对象大小相同。

---

## 5. `when` 表达式扩展

Kotlin 原有的`when`语法（等值匹配、类型匹配、区间、条件分支）原样保留。以下扩展对**enum、tuple与struct**生效。它们与4.6共享tuple/字段/rest结构语法，但这里使用可失败的match pattern，额外允许literal与enum variant递归出现在子模式中。class的`componentN`位置解构只属于4.6的binding位置，不进入match pattern或穷尽性判断。

`if`、`when` 与 `try` 都是表达式，也可在结果被丢弃的语句位置使用。作为值使用时，每个可正常结束的分支块以最后一个表达式的值作为该分支结果；空块或以非表达式语句结束的块结果为 `Unit`，以`return`/`throw`或8.7的`break`/`continue`结束的路径不参与结果类型合并。外层有期望类型时，每个正常分支结果必须是其子类型；否则取所有正常分支结果的唯一可表达最小上界，存在多个不可比较的最小共同上界时退化为 `Any`。依赖期望类型的分支结果可由其他分支先确定类型，分支检查顺序不影响结果。

值位置的 `if` 必须有 `else`；结果被丢弃时可以省略。值位置的 `when` 必须穷尽。`try` 的结果由正常完成的 try body 与各 catch body 共同决定；`finally` 的结果值始终丢弃，但其中实际离开当前finally的`return`/`throw`/`break`/`continue`仍按第8章覆盖先前路径。

**统一规则**：仅当 subject 的静态类型是 enum / struct / tuple 时，`when` 才按模式匹配解析（下称**模式 when**）；其余 `when` 一律保持 Kotlin 的表达式语义——分支条件是普通表达式（等值比较）、`is` 检查、`in` 区间等，穷尽性也遵循 Kotlin 自身规则（表达式形式须穷尽，语句形式不强制）。分支条件采用哪一种规则，由 subject 的静态类型确定。

模式when适用两条全局规则：

- **穷尽性**：模式 when 无论作为语句还是表达式，都必须覆盖输入类型的全部可能值；无法静态确定穷尽时是编译错误，并给出至少一个稳定、可重新解析的未覆盖示例，也可显式加 `else`。带 guard 的分支不贡献覆盖，因为 guard 可能为 false。
- **match中的绑定优先**：本条只适用于`when`的match pattern，不改变4.6的binding上下文。分支模式中的裸标识符不引用既有变量；只允许literal直接匹配值，匹配既有const须改用guard，限定名只用于enum variant。4.3的内建`Unit`/`()`首先按Unit literal分类；enum即使声明同名`Unit` variant，也必须以`E.Unit`或相应显式payload shape匹配。除此之外，enum subject下先按其exact type查询同名variant：命中unit variant时形成variant pattern，命中带payload的variant时报告缺少显式payload shape的错误，只有名称完全未命中时才建立匹配一切的新binding（效果同`else`）并给出非致命warning以避免拼写错误。相反，4.6中除`Unit`字面量外的普通裸标识符不执行这一步variant查询，即使名称相同也恒为新binding。

穷尽性按以下typed constructor递归定义：

- enum的有限constructor集合是其全部variant，variant payload继续作为子列检查；出现variant名称本身不代表覆盖其全部payload；
- tuple与struct各有一个product constructor，字段按位置/声明顺序展开；命名字段与`..`先补全为完整wildcard vector；
- `Boolean`有`false`/`true`两个有限constructor，`Unit`有一个constructor；
- binding、`_`、rest 补位与 `else` 均为 wildcard。定宽整数是有限域；不同无 guard literal 覆盖全部位型时可省略 wildcard，否则未覆盖示例必须是该 signed/unsigned 类型中的实际缺失值，unsigned 示例带 `u` 后缀。Char 的有限域只含 Unicode 标量，未覆盖示例不得落入 surrogate 区间。String 是无限域，有限 literal 分支仍需 wildcard 覆盖余值。Float/Double literal 按 11.2.2 的 IEEE 相等匹配，正负零覆盖同一值，始终需要 wildcard 覆盖剩余域。
- 多个arm可以组合覆盖product，例如`(true, _)`与`(false, _)`共同穷尽；`Some(0)`与`None`不穷尽`Option<Int>`；
- 运行期始终按源码first-match顺序工作：先匹配结构，成功后才求值guard，guard为false时从下一arm继续。穷尽proof不得改变该副作用顺序。

integer与floating literal pattern还允许unary minus直接作用于literal；括号不形成语义节点，空白或注释不影响识别。整数以subject的exact integer type复用11.2的fit、wrapping unary-minus与signed `MIN`边界；浮点以subject的exact precision复用11.2.2的直接舍入和符号位规则。其他常量表达式不属于literal pattern。

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
  - 命名字段变体：`Variant { f1, f2: subpattern, .. }`；`f1`是`f1: f1`的shorthand，冒号右侧可为binding、literal、`_`或任意嵌套match pattern；未列出全部字段时必须以`..`结尾。
- 变体名可省略 enum 类型前缀（`E.`），编译器按被匹配值的类型解析；存在歧义时需写全限定名。显式前缀遵守普通类型名称的包、import、嵌套声明和可见性规则。前缀直接命名泛型 enum 声明时，以 subject 的完整 application 确定类型实参，不要求在模式前缀重复写实参；前缀是 typealias 时，其展开后的完整类型必须与 subject 相同。
- enum穷尽性按全局pattern matrix递归检查variant payload；仅列出全部variant名称但payload覆盖不全仍不穷尽。

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
- tuple无sum variant，但各字段的有限constructor组合可由多个arm共同覆盖；是否需要`else`完全由全局pattern matrix决定，而不是只寻找单个irrefutable arm。

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
- 被匹配值的静态类型就是该struct时，type前缀可省略（`(x, ..)`或`{ f1, .. }`均可）；多个struct arm可以通过字段子模式组合证明穷尽，不要求存在单个全wildcard arm。
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
- **只有 f-string 支持插值**。普通字符串（`"..."`、`"""..."""`）中 `$` 是普通字符，不触发任何插值或脱糖。

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
- 按源码顺序交错执行每段表达式与对应 `add`：前一表达式及其 `toString()` 完成后，才开始下一表达式。表达式或 `toString()` 抛出时，后续段和最终 build 不执行；挂起表达式仅在原上下文允许时合法，builder 按普通 managed local 跨挂起保活。
- 脱糖引用实际 core `StringBuilder` 声明及普通构造、add、build callable，使用 hygienic temporary；用户同名声明、alias 或 import 不替换脱糖目标，普通用户书写的调用仍遵守名称查找。通用 add 使用普通 `T : ToString` bound，不为插值增加 Any 字符串化回退。
- f-string 不属于 `const val` 的常量表达式，即使只有文本段；普通字符串和 raw 字符串仍可作为 String 常量。实现可以在合法性检查后优化纯文本结果，但不改变其 const 可用性。

支持普通 `"""..."""` 与 `f"""..."""` 的 raw 多行字面量。普通单行字符串、单行 f-string 的文本段与字符字面量共用转义集合：`\t`、`\b`、`\n`、`\r`、`\'`、`\"`、`\\`、`\$`、四位十六进制 `\uXXXX` 和一至六位十六进制 `\u{...}`。每个 Unicode 转义必须是合法标量值；surrogate code point、超过 U+10FFFF、缺失数字/终止符及未知转义都在原 source span 诊断。raw 字面量不解释反斜杠转义。转义解码产生的美元符号始终是文本，不重新识别为插值起点。`${...}` 内的嵌套括号、字符串和注释遵守普通表达式词法规则。
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

`Option<T>`是普通invariant enum，享有第4.2节与第5章的全部能力（解构、穷尽性检查等）。即使`S <: T`，`Option<S>`也不是`Option<T>`的子类型；需要改变payload类型时必须显式`map`/重建。构造处存在`Option<T>`期望类型时，`Some(value)`和`None`按4.2适用于所有enum的contextual variant规则绑定该exact application，因此常见返回与赋值不需要先产生另一个`Option<S>`。`scoop.core.Option.*`由core prelude的typed default import引入；这只增加普通可见候选，不构成Option专用resolver或短名称特判。

导入 core 时，`T?`、安全调用、Elvis、`!!` 与 `as?` 使用产物中已解析的 Option 声明及 variant/payload 角色；其具体类型仍沿普通泛型 enum 路径产生。源码名称查找与这些语法角色分离，同名用户 enum 不改变 `T?` 的含义。省略初值的普通可变 Option 属性及可直接编码的 `None` 初值继续遵守9.1.3的静态初始化规则。

### 7.3 空安全运算符的语义

- `a?.b`：脱糖为 `when (a) { Some(v) -> Some(v.b); None -> None }`（结果类型为 `Option<B>`，其中 `B` 是 `b` 的类型）。
- `a?.f(args...)`：脱糖为 `when (a) { Some(v) -> Some(v.f(args...)); None -> None }`。receiver `a`先求值且只求值一次；只有运行期进入`Some`分支后才会求值显式实参、物化`vararg`、执行缺省表达式并进入callee。方法、extension与property-like `invoke`的选择均在payload静态类型上按第8章完成。
- 安全调用**不展平**结果。若`f`返回`Option<R>`，`a?.f()`的类型是`Option<Option<R>>`；若返回`Unit`，结果是`Option<Unit>`。这遵守7.1的逐层`Option`语义，不采用Kotlin nullable type的幂等合并。
- `a ?: b`：脱糖为 `when (a) { Some(v) -> v; None -> b }`。
- `a!!`：`a` 为 `Some` 时取值；为 `None` 时抛出核心库异常 `UnwrapException`。
- 空安全表达式可以出现在`while`条件中；条件脱糖产生的临时绑定与其他条件求值步骤一起在每次条件检查时重新执行，不能提升到循环外。
- 不支持 `a?.b = c` 形式的赋值：`a?.b` 的结果是临时 `Option`，对其赋值无意义。
- `null` 字面量不存在；表示"无值"使用 `None`。
- 取值使用 `when` 解构：`when (s) { Some(v) -> ...; None -> ... }`。对 `Option<T>` 不提供智能转换，`isSome()` 一类判断不会收窄类型。

### 7.4 enum 布局与 `Option` niche 保证

对具有**天然空位（niche）**的类型 `T`，编译器必须保证 `Option<T>` 不增加额外存储：`None`编码为该空位，`Some(v)`与`v`的表示相同。空位按定义不是合法的裸`T`值；所有安全构造、compiler-generated值与声明为返回裸`T`的FFI边界都必须维持该不变量，unsafe代码破坏它后行为未定义。这是语言的固定特性，而非可选优化。适用类型：

- **引用类型**：空位是全 0 的机器字（GC 不会产生该引用值）；
- **`Ptr<U>` 与 `FunPtr<F>`**：空位是内部raw pointer carrier的全0位模式；裸`Ptr`/`FunPtr`值必须非零，null pointer只编码为对应`Option`的`None`。

因此`Option<FunPtr<F>>`在`F`满足C ABI规则时可以直接表示可空函数指针；`Option<Ptr<T>>`只有在`T`本身有本章定义的portable C pointee representation，或`T == Unit`作为opaque `void *`例外时，才能直接出现在C ABI边界。`Option`的niche不绕过C-FFI-safe classifier，尤其`Option<Ptr<其他ZST>>`仍被拒绝。`Option`包裹的引用类型与裸引用布局相同，但引用类型本身仍不能进入C ABI。对其他类型`Option<T>`带tag，具体布局不保证。

niche表示的适用范围严格限于与上述`Option<T>`同构的enum：恰有两个variant，其中一个无字段，另一个恰有一个字段，且该字段的具体类型为引用类型、`Ptr<U>`或`FunPtr<F>`；variant名称、顺序以及字段采用位置或命名形式不影响判断。除此之外的enum一律使用tagged表示，不能从其他位模式、整数范围或用户不变量推导niche。

tagged enum的表示由tag、一个可选的**pure-value共享payload区**以及若干**ref-bearing独占连续slot**组成。递归检查后完全不含managed ref的多个variant可以复用同一块共享payload空间；每个直接或间接包含managed ref的variant则必须拥有自己的连续slot，不能与其他variant重叠。共享区或独占slot都按对应variant字段的正常值布局保存其全部字段。构造tagged enum时必须先把整个值（包括共享区、所有inactive ref-bearing slot和padding）清零，再写入tag及当前variant所用的payload；复制按完整值复制。GC只需无条件检查所有ref-bearing独占slot中的固定ref位置，全0 inactive slot不形成引用，扫描不读取tag、不选择variant。`Ptr` / `FunPtr`不是managed ref；在未采用niche的tagged enum中，包含它们但不含managed ref的variant仍属于pure-value共享区。

---

## 8. 函数

函数语法整体与 Kotlin 一致：默认参数、命名参数、`vararg`、扩展函数、中缀调用、运算符重载、lambda 与尾随 lambda、函数类型 `(A, B) -> R` 等。

- 非 `Unit` 函数的所有可达正常完成路径必须返回一个可赋给结果类型的值；抛出异常也可终止路径。无法静态确定时是编译错误。
- 显式结果为 `Nothing` 的函数不能正常落空、执行裸 `return` 或返回一个可正常产生的值；可通过 `throw`、另一个 `Nothing` 调用或可静态确定不结束的控制流终止。调用的无正常返回性质来自已解析的实际结果类型，同样适用于成员、依赖函数、函数值、默认值和泛型实例。它不蕴含 `@NoGC`、不抛异常或不挂起；`suspend () -> Nothing` 可以挂起，但不能成功完成并产生结果。
- `break` 与 `continue` 只改变其目标循环的控制流，不视为函数返回；循环按可能正常结束处理，除非可以静态确定不会结束。
- `finally` 正常结束时恢复进入前的正常继续、返回、异常或循环跳转。实际离开当前 `finally` 的新控制转移替换原动作；在 `finally` 内部已由循环或 catch 处理的转移不替换原动作。

### 8.1 函数类型、函数值与 closure

#### 8.1.1 函数类型

普通函数类型写作 `(P1, P2, ...) -> R`，挂起函数类型写作 `suspend (P1, P2, ...) -> R`。零参数函数必须写作 `() -> R`；参数名、缺省值与 `vararg` 不是函数类型身份的一部分。

- 函数类型是**引用类型**。函数值可以携带捕获环境，具有 identity，由 GC 管理；它不是一个裸代码地址。
- 函数类型的身份由挂起性、参数个数、每个参数类型和返回类型共同决定。普通与挂起函数类型之间不存在子类型关系或隐式转换。
- 同为普通函数类型或同为挂起函数类型时，参数类型逆变、返回类型协变：若 `A2 <: A1` 且 `R1 <: R2`，则 `(A1) -> R1 <: (A2) -> R2`。多参数逐项应用同一规则。
- 赋值、传参或 `as` 把函数值适配到不同但兼容的函数类型时，结果与来源不保证 `===`。函数类型的 `is` / `as` 按上述结构化子类型关系判断，不要求参数和返回类型逐项完全相等。
- 函数类型不保留形参名、缺省值或 `vararg` 调用约定。经函数值调用时只能按位置提供与类型元数相同的实参；声明中的 `vararg T` 在函数类型中表现为其实际参数类型 `Array<T>`。
- 函数值以 `f(args...)` 调用；`f.invoke(args...)` 是同一操作的显式写法。若 `f` 是挂起函数类型，调用点必须处于 8.2 允许的挂起上下文。
- 函数值不定义结构相等。可以用 `===` / `!==` 观察同一已保存函数值的引用 identity，但规范不保证对同一函数重复创建 callable reference 或无捕获 lambda 时得到相同 identity。

Scoop 当前不提供 Kotlin 的 receiver function type（`A.(B) -> R`）；扩展函数的 callable reference 按 8.1.4 显式表示为普通函数类型。

#### 8.1.2 lambda 与匿名函数

lambda 写作 `{ parameters -> body }`，挂起 lambda 写作 `suspend { parameters -> body }`。匿名函数写作 `fun(parameters)[: R] { body }`，挂起匿名函数写作 `suspend fun(parameters)[: R] { body }`，其中方括号表示返回类型可省略并由 body 推导。四种形式都会产生函数值，也都可以捕获外层词法环境。

- 每个 lambda 参数写作 `PatternSyntax [':' Type]`，必须满足 4.6 的 binding pattern 规则。一个完整模式对应一个源码参数和一个函数类型参数，不按解构后的 binding 数量展开。类型标注属于完整 subject；参数的 ZST 或间接 ABI 规则见 4.7 和 14.2。
- 有期望函数类型时，每个lambda parameter的完整subject type可由对应的一个期望参数类型给出；期望元数按source pattern数量匹配。无期望类型时，每个显式参数（包括composite pattern）都必须为完整subject写出type annotation。单参数 lambda 在期望元数为 1 且省略参数列表时隐式声明 `it`；无参数 lambda 使用 `{ body }`。
- lambda 或匿名函数已有显式参数类型时保留该类型，并按 8.1.1 的逆变检查其能否接收期望参数；匿名函数已有显式返回类型时保留该类型，并按协变检查结果。上下文只补全未标注的部分，随后按普通函数值规则适配，不能因直接写在实参位置而禁止既有函数类型型变。
- lambda 参数支持 4.6 的解构模式。传入的单个参数值作为该pattern的subject且只处理一次；解构失败不产生运行期分支，参数静态类型必须能按该模式解构，否则是编译错误。投影、component调用、hidden temporary、异常与挂起语义均遵守4.6，不改变函数的源码/函数类型元数或该参数对应的单个typed ABI classification entry。
- lambda 的值是 body 最后一个表达式的值；有期望结果类型时，按普通函数返回规则检查 subtype 并完成隐式适配，包括值装箱、引用上行转换与函数值型变，不要求结果类型逐字相同。期望返回 `Unit` 时最后一个表达式的值被丢弃。匿名函数使用普通函数的返回规则。
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
局部命名函数与普通函数使用相同的声明语法，并按上述规则捕获外层不可变 binding。局部函数名在其自身 body 及声明之后的词法作用域可见，因此允许直接递归；声明之前不可见，互相递归不能依赖后声明函数的前向可见性。局部函数可以直接调用，也可以通过 `::name` 取得函数值。

局部 generic 函数的直接调用按 3.2 单态化。取得其 callable reference 时，所有类型参数必须能由期望函数类型唯一确定；否则是编译错误，不存在“仍然 generic 的函数值”。

不提供隐式 reference capture、`move` capture 或 capture list。

#### 8.1.4 函数声明引用表达式（callable reference）

`::` 引入函数声明引用表达式；其结果由上下文的期望类型决定：

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
- 下列**声明自身拥有的运行期初始化上下文**都是非挂起上下文，不得包含挂起调用：顶层 `val` / `var` 的 initializer 与 delegate 表达式；`object` / `companion object` 的属性 initializer、delegate 表达式、`init` 块及基类/接口委托初始化；class 的属性 initializer、delegate 表达式、`init` 块、主/次构造函数体及构造委托；struct secondary constructor body / delegation；struct/enum 变体及构造函数的缺省表达式。全局初始化入口必须在进入 `main` 前同步完成；单例或实例初始化必须在对象可用前同步完成。class/struct初始化中的`this`按9.1.1作为受限initializing receiver，只能直接访问已经初始化的字段，不能作为普通值发布、捕获或用于任何方法分派。
- **求值归属按词法位置确定，而不是按最外层构造语法确定**：在 suspend 函数中显式写出的调用实参仍处于调用者的挂起上下文，因此 `C(awaitValue())` 合法——`awaitValue()` 先完成，随后普通构造过程同步执行；构造函数定义处的缺省表达式和构造体内部则仍为非挂起上下文。把 suspend lambda / `SuspendTask` 对象保存进字段也不等于执行它，其函数体在以后实际调用时按自身的挂起性检查。
- 属性没有隐式挂起能力：普通 getter / setter、计算属性，以及委托属性的 `getValue` / `setValue` 协议必须是非挂起 callable；即使属性读取发生在 suspend 函数中，也不能通过普通属性访问暗中挂起。
- `const val` 的约束更强：它没有运行期初始化过程，只允许 9.1.2 定义的编译期常量表达式，因此不允许普通或挂起函数调用。9.1.2 封闭列出的整数表示 intrinsic call 是编译器常量运算，不属于这里的普通函数/方法调用。
- 挂起性是 callable 签名的一部分：override / interface 实现的挂起性必须与被覆写声明完全一致；`suspend (A) -> R` 与普通函数类型 `(A) -> R` 不兼容。挂起性不作为同名声明的重载区分项，参数列表相同而只相差 `suspend` 的两个函数是重复声明。
- 调用挂起函数可能在当前调用栈内立即产生 `R`，也可能保存当前计算并返回到协程启动者，随后经 `Continuation` 恢复。无论采用哪条路径，源码都只观察到一次普通的 `R` 结果或一次在该调用点抛出的异常；挂起本身不是返回、异常或 `finally` 的退出原因。
- 调用点之前已经完成的实参和子表达式只求值一次；恢复后从调用点之后继续，源码从左到右求值顺序不变。跨挂起点仍存活的局部变量、参数及待执行的控制转移必须被保留。
- `try`/`catch`/`finally`的语义跨挂起点保持不变：恢复失败等价于在原挂起调用点`throw`；仅仅挂起不会执行`finally`；跨挂起保存的待执行动作包括正常继续、return、throw以及8.7的break/continue。finally正常结束后恢复原动作；只有实际从当前finally向外离开的新控制转移才覆盖原动作，finally内部被catch或只退出其内层loop的转移不覆盖。每个被退出的finally仍恰好执行一次。
- `Continuation` 是单次完成协议：一个挂起点只能由 `resume` 或 `resumeWithException` 中的一个成功完成一次；编译器生成的 continuation 对重复完成抛出 `IllegalStateException`。本规范不定义协程取消；放弃且永不恢复一个 continuation 不会隐式执行 `finally`。
- 不支持 suspend FFI：挂起函数不得带 `@Extern`，其声明引用也不得在 `FunPtr` 上下文中解析为原生地址； hidden continuation ABI 只用于编译器生成的 Scoop 托管调用，不是任何 FFI ABI。具体约束见 13.4、13.10 与 14.2。
- 具体的协程构建器（`launch`、`async` 等）、调度器与取消策略属于标准库，不在最小核心库范围内。最小核心库只提供 11.9 的启动、挂起与恢复原语。

### 8.3 Task-local Context

Context 是属于逻辑任务的动态绑定。声明入口按精确静态类型查找，调用点不做隐式实参解析。运行时契约见 [运行时规范第 9 章](SCOOP-RUNTIME-SPEC.md#9-task-local-context)。

```scoop
interface Logger {
    fun write(message: String)
}

context(logger: Logger)
fun report(message: String) {
    logger.write(message)
}

fun run(logger: Logger) {
    context(logger) {
        report("ready")
    }
}
```

#### 8.3.1 声明与名称

`context(name: T, _: U)` 是至多一个、非空的声明前缀，位于该声明的 annotation/modifier 之前；允许换行，不跨显式分号。每项必须有名称或 `_` 及类型，不接受默认值、`vararg` 或参数修饰符。它可以修饰现有合法的顶层、成员、扩展及局部命名函数，包括 ordinary、suspend、generic、abstract/interface 方法；也可以修饰没有存储、initializer 或 delegate 的计算/abstract property。property-level list 同时适用于其 getter 和已有 setter，不能分别给 accessor 添加另一份 list。

不能用此前缀修饰名义类型、constructor、`init`、release block、lambda、匿名函数、函数类型、stored/delegated property，或 `@Extern`、`@Intrinsic`、`@NoGC` 声明。既有的 generic virtual、interface method-level generic、属性挂起性等限制不因 Context 放宽。

- 具名项在该 list、普通值参数与实际 setter 参数的名称空间内必须唯一；`_` 可重复，但不建立名称。名称只在函数或 accessor 正文内可见，作为普通不可重新绑定的 local；不在 annotation、类型子句或普通参数默认表达式中引入名称。默认表达式仍按 8.5.3 在 caller 求值。
- 每个 list 的 key 必须不同。透明 alias 展开后相同也算重复；泛型定义处检查已经相同的类型表达式，完整替换后检查新合并的 key。后一检查适用于每个具体 callable/owner application，包括无正文的 abstract/interface contract；错误报告在触发该 application 的位置。
- requirement 的类型与 binding 表达式的静态类型都必须是非空 managed ref：包括 class/interface/object、String、数组、`Any` 及 ordinary/suspend 函数值。值类型、`Option<T>`、`Ptr`、`FunPtr` 不能直接作为 key。先按普通规则装箱或转成某个引用类型后，可按该引用类型绑定，Context 本身不插入装箱或上转型。
- 泛型类型表达式须在定义处即可确定为 ref；例如 `Array<T>`、函数类型、`T : ref` 或具有 class 上界的 T。无约束 T 或只有 interface 上界的 T 不能保证这一点，因为 value type 也能实现 interface。13.9 的 bound 组合限制保持。
- public/protected contract 中的 context type 遵守 9.1.5 的签名可见性；名称不是 key，不影响类型身份。

#### 8.3.2 精确 key 与入口取得

key 就是 alias 展开后的 canonical exact static type；包含完整 nominal/function type 实参及函数挂起性，不读取对象的运行期 TypeDescriptor，也不做 subtype、variance 或 interface 搜索。不同函数类型即使可按 8.1.1 转换，仍是不同 key。同型多角色需要不同 nominal wrapper，透明 alias 不创建新 key。

```scoop
interface Service
class ServiceImpl : Service

context(service: Service)
fun useService() {}

fun needsServiceBinding(impl: ServiceImpl) {
    context(impl) { useService() }       // ServiceImpl does not supply Service.
}

fun withServiceBinding(impl: ServiceImpl) {
    val service: Service = impl
    context(service) { useService() }    // Binds the exact Service key.
}
```

不同 key 的 binding 可以同时存在；安装 ServiceImpl 不遮蔽已有的 Service。上例 needsServiceBinding 只在外层也没有 Service 时抛缺失异常。

每次进入一个 source implementation body 时，按 list 顺序查找一次，先全部取得再执行用户正文。具名结果保存为普通 immutable local，匿名项只检查存在；未使用项也必须取得。第一个缺失项抛 11.7 的 `MissingContextException`，消息标识声明、参数名或从 1 开始的匿名序号、canonical type。函数内的 catch 不包围这个入口过程，caller 可以捕获该异常。

该次 activation 取得的 local 不随内部同型重绑定而变化，新调用进入时才取得新绑定。suspend body 只在 initial entry 取得，跨挂起保留 local；resume 不重取。无正文声明只有 contract；this-adjust thunk、函数引用 adapter 等进入同一实现，不另执行一次 lookup。body 内的普通 closure 若引用该参数，只按 8.1 捕获这个 local。

#### 8.3.3 结构化绑定表达式

`context(value) { body }` 是 compiler-known 表达式，body 是词法 block，不是 lambda，不产生 callable 或 suspension boundary。无 receiver 的这一完整形态优先于普通调用加 trailing lambda；`obj.context(...) { ... }` 仍是普通调用。括号与 block 之间按既有 control-flow header 规则处理换行和分号。一次只接收一个 value，多项用嵌套 scope。

1. value 在外层 Context 中按普通规则完整求值一次，以包含 smart cast 结果的静态类型确定 key。body 的 requirement 或整个表达式的 expected result type 不反向决定 value 的类型；expected result type 只传给 body。
2. value 正常完成后安装 binding；安装失败不得提交部分状态。body 在新 binding 下执行一次，其正常结果是整个表达式的结果。
3. 同 key 的内层 binding 覆盖外层，退出后恢复原值或 unbound。fallthrough、return、break、continue、throw 只有真正离开该 scope 才恢复，按词法嵌套恰好一次，与 8.7 的 finally/catch cleanup 使用同一顺序。
4. 挂起不是退出，不恢复 binding；body 继承所在 callable 的挂起性。恢复后的真正退出仍按上一项处理。scope 内的 finally 在该 binding 下执行；位于 scope 外的 finally 在恢复后的 Context 下执行。

Context 安装可能分配/GC，入口缺失可能构造并抛异常；它们不能用于 const、release-safe 或 NoGC 正文。lookup 成功本身不调用用户代码。

#### 8.3.4 声明契约与调用

ordered context list 进入导出声明及 override/accessor contract。完成宿主与 callable 类型替换后，override/interface implementation 的数量、顺序与 exact type 必须全部相同，名称可以不同。继承同一普通签名的多个冲突 context contract 是该 owner application 的编译错误，不能任选一个。contextual property obligation 只能由具有相同 contract 的计算/abstract property 承接，stored/delegated/constructor property 的隐式 accessor 不能满足它。

context list 不参与 overload applicability、MSC、泛型推断、普通或 suspend 函数类型、mangle、dispatch slot identity 或机器实参。只差 context list 的声明仍是重复声明。未声明 requirement 的中间函数可以调用 contextual function，不推导或传播静态 effect row；binding 缺失只在实际进入有 requirement 的 body 时抛出。函数引用创建时不查找、不捕获 Context，其类型不增加 requirement，实际调用才进入目标 prologue。普通实参、receiver 和 caller-side default 的求值仍先于这个 prologue。

#### 8.3.5 任务、协程与回调传播

- 普通调用、virtual/interface/closure 调用与同一协程的 direct suspend 调用共享当前 TaskContext。普通 closure 不因创建于 binding scope 而隐式捕获整个 Context。
- `startCoroutine` 在普通实参求值后、执行 task body 前，从当前有效 binding 建立独立 child TaskContext；task 与最终 completion 通知均在 child 下执行。parent、child 以后的重绑定互不影响，已绑定对象的 identity 仍共享。两种 overload 的行为相同（11.9）。
- 真正驱动 resume 时进入 frame 所属 TaskContext，在再次挂起、完成或失败后恢复 resumer 原 Context。只投递同步完成 payload 而未驱动 frame 的 resume 路径不切换 Context；完成权竞争仍遵守 8.2。
- 同步 outbound FFI 保持逻辑任务。`foreignCallback` 注册时捕获有效 binding 的快照，每次 invocation 从快照建立独立 TaskContext；包括同步重入及 Reusable 并发调用（14.3）。callback 的缺失异常遵守原有 catch/status 边界。
- 所有 eager initializer 与 main 共享根任务，但 binding 仍只能由实际执行的词法 scope 提供。无 binding 的 initializer 调用 contextual function 同样抛 `MissingContextException`。
- Context 不提供取消、调度、线程同步或资源释放。永不恢复的 continuation 不隐式执行 scope cleanup；相关对象不可达后按普通 GC 回收。

### 8.4 `inline` / `crossinline` / `noinline` / `reified`

这些关键字**仅出于 Kotlin 兼容性保留，无实际作用**：

- Scoop 泛型是单态化的，类型信息在每个实例化处天然可用，不需要 `reified` 来保留类型实参；
- 编译器自行决定内联策略，`inline` / `noinline` / `crossinline` 被接受但可忽略（允许作为提示，但不保证语义）；
- 不依赖语义内联的 Kotlin 代码可以保留这些关键字而无需修改；依赖 inline lambda non-local return 的代码不兼容，必须按 8.1.2 改为匿名函数或重写控制流。

### 8.5 调用实参、默认参数与 `vararg`

#### 8.5.1 参数声明

- 普通value parameter可以是必需参数、带缺省表达式的默认参数，或`vararg`参数；一个参数列表至多有一个`vararg`。class/struct primary constructor、class/struct secondary constructor及构造函数式enum variant字段使用同一规则；`vararg val x: T`在primary构造结果中声明的属性/字段类型是`Array<T>`。secondary参数不能写`val`/`var`，只作为constructor local存在。
- `vararg x: T`中的`T`是**元素类型**；在函数体、构造体及函数签名的实际参数类型中，`x`的类型是`Array<T>`。因此只相差`vararg x: T`与普通`x: Array<T>`的两个声明具有相同参数类型，不能据此形成重载。函数类型同样只保留实际参数类型`Array<T>`，不保留`vararg`调用约定（见8.1.1）。
- `vararg`可以有显式缺省表达式，其类型必须是`Array<T>`；没有显式缺省且调用处没有提供任何vararg元素时，产生一个新的空`Array<T>`。
- 参数名、默认值及是否具有默认值不属于函数签名，不能仅靠它们区分重载；但参数名和`vararg`调用约定属于源码调用接口，必须保留到调用决议完成。
- 默认参数值表达式是callable源码接口的一部分，并在**定义处完成解析**：名称解析、overload选择、类型检查、可见性检查、挂起性检查及所引用实体身份都使用声明方上下文。它可以使用当前callable的类型参数、源码存在的隐含receiver以及此前声明的参数，不能引用自身或后声明的参数。constructor default可以使用host type parameter与此前constructor参数，但constructor在source interface中没有`this` receiver：class对象尚未分配，struct值尚未形成，不能从default访问任何实例field。
- default直接绑定的每个实体都必须在该callable的**全部合法调用位置**可访问，即callable调用域必须是该实体可访问域的子集。对导出的`public` callable，这意味着只能引用下游可见的`public`或经`public import` re-export的实体，不能引用声明Cone的`private`/`internal`实现；`internal`、private/member及local callable可以引用覆盖各自完整调用域的实体。该检查适用于已选择的callable/overload、constructor、property accessor、operator目标及nominal type，并在const folding与desugaring之前执行；不能靠编译期折叠绕过可见性。
- default正文自己引入的local function随该正文一起展开，其合法直接调用不要求调用方重新lookup这一词法声明。callee必须对应正文中实际声明的同一个local function；正文外的private/internal callable仍按完整调用域检查，不能仅因它也是local function或同名函数而豁免。
- `abstract`函数和interface方法可以声明默认值。`override`声明不得重新声明默认值；它按静态可见的override关系继承唯一的默认来源。若一个override位置从互不相关的父声明继承到无法唯一确定的默认来源，必须在类型定义处诊断，不能任选一个表达式。override的`vararg`形态必须与被覆写声明一致。
- override参数名不参与签名匹配；命名调用使用调用点静态接收者所见声明的参数名，动态分派只选择函数体，不重新映射实参或替换默认来源。
- 默认值之间的依赖不受声明顺序、文件顺序或function/constructor/variant类别影响。仅当已选定调用实际省略某个参数时，才建立到该参数默认值的展开依赖；显式提供参数的普通调用不建立该边。源码默认值展开依赖必须无环，自环或跨声明环在定义处报编译错误，即使外部尚未调用该声明；按参数位置区分节点，不能因同一个callable出现在链中便误判成环。普通callee正文中的递归调用不属于该展开依赖图。

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

- 调用目标与完整type arguments确定后，缺省表达式在每个发生缺省的调用处以已经绑定的typed template进行hygienic展开并求值，不是在声明时计算或缓存；调用方显式提供参数时，对应缺省表达式完全不展开。调用方不会对template中的名称、extension或overload重新做决议，因而调用方import、同名局部声明或后来新增的overload不能改变其含义。
- 这与单态化一致：缺省表达式可能依赖类型参数（如`fun <T> f(x: T, y: Array<T> = [])`），只有在调用目标与全部类型实参确定后才能生成具体代码。缺省表达式自身不能作为“猜出”尚未确定类型实参的来源；若显式实参、receiver、允许的期望类型及声明约束仍不能唯一确定实参，调用不成立。
- 缺省表达式可以引用此前参数，因为完整参数值按声明顺序物化；此前参数无论来自显式实参、vararg构造还是另一个缺省表达式都已经可用。
- 导出callable的default template只引用已经导出或re-export的typed实体，不携带private/internal hidden dependency closure。修改参数名、default body、绑定目标或vararg形态会改变其`.slib`接口metadata并使下游重新编译；被引用实体的可见域收窄到不再覆盖callable调用域时，声明方必须重新编译并报错。
- `suspend`函数的缺省表达式可以包含挂起调用；它在挂起调用点实例化，此前已经求值的receiver与显式实参必须按8.2保存。普通函数及所有构造器/variant的缺省表达式不能包含挂起调用；suspend caller显式写出的普通构造实参仍按8.2合法。
- 所有缺省表达式采用相同的实例化语义，不因表达式是literal、普通调用、构造、intrinsic或其他kind而改变。实例化后的每个表达式同时具有声明节点的**定义来源**和发生缺省的call expression的**求值来源**；前者用于定义处诊断与源码归属，后者供任何需要观察求值位置的语言设施使用，二者都必须存在且不能互相回退或覆盖。若该call expression本身来自另一个缺省表达式实例，其已有求值来源继续传入内层实例，因此无需识别具体表达式内容即可得到最外层实际调用来源。

#### 8.5.4 `vararg`值

- 普通位置元素`e`为`vararg x: T`贡献一个满足`type(e) <: T`的元素；spread表达式必须具有exact `Array<T>`类型。`Array<S>`即使`S <: T`也不能作为该spread，调用方需要显式逐元素构造/转换目标数组。
- 未命名vararg元素（包括spread）先各自按8.5.3求值，随后在形参物化阶段构造一个新的`Array<T>`；spread按元素顺序复制，可以与普通元素混合。即使唯一输入是`*array`也不与来源数组共享identity。
- 命名形式`x = array`或`x = *array`直接提供完整exact `Array<T>`值，不额外复制；它与“若干位置元素组成新数组”是两种不同的源码调用形态。
- 没有元素且没有显式默认值时构造新的空数组；有显式默认值时按普通缺省表达式求值并直接使用其结果。实现可以在identity不可观察时消除分配或复制，但不能改变`===`、异常、挂起点或求值顺序可观察到的结果。

### 8.6 调用决议与泛型约束求解

- 调用决议先按词法/成员/import优先级建立候选层；无显式receiver与extension scope的完整层序见12.4.3。每层内继续按9.3.4的function-like/property-like c-level分区；对每个最终分区完成调用形态预过滤与候选各自的类型可应用性检查，再只在第一个含有至少一个可应用候选的分区中求最具体候选。显式类型实参数量、命名参数是否存在、spread/receiver形态等不依赖表达式类型的检查属于预过滤；某个更高层仅有同名但形态、类型或bound不适用的声明时，不得无条件遮蔽合法的下一层候选。当前Cone、上游`.slib`、exact/star import与core prelude只决定候选来自哪一层，不改变调用决议规则。
- 每个候选拥有独立的实参映射、fresh inference variables和constraint system。receiver、非postponed实参、显式fixed/`_`类型实参、函数/nominal invariant relation及upper bound共同产生等式与子类型约束；generic owner参数、callable自身参数和待推断变量保持不同identity，不能压平成一组后再按长度或span反推。
- 选择变量的唯一解前，等式与上下界必须相互传播：由`L <: V`和`V <: U`继续按同一类型关系约简`L <: U`，声明的class/interface bound也参与该过程。例如`R : Reader<T>`与实参确定的`Holder<Int> <: R`通过`Holder<Int>`的实际`Reader<Int>`父类型确定`T = Int`。该过程只使用已有约束；单独的声明bound不能成为补齐未知变量的猜测值，不能先选一个临时解再把它当作新的已知事实。
- 依赖候选期望类型的lambda、匿名函数、callable reference、裸enum variant（包括`None`）、空数组、数值字面量和嵌套generic构造作为postponed argument处理。求解器先用其余约束推进固定点，再用候选给出的完整期望类型检查postponed argument；lambda body结果只决定候选是否适用，不提供额外的“按lambda返回类型优先”规则。
- 候选声明、receiver 或显式类型实参已经确定的完整形参类型可直接作为实参表达式的上下文，包括结构化表达式的分支及参数已标注的 lambda 正文；不必先丢弃已有上下文再尝试合成类型。尚未确定的形参仍按上述固定点处理。
- 后续实参同样可以提供上述类型上下文，例如 `pack(*[], 42)` 的空 spread 可由另一元素确定为 `Array<Int>`。命名整数组、普通参数、构造参数和成员调用遵守同一规则；没有可用约束的空数组、裸变体或函数引用仍须报错。推断固定点没有进展时才尝试可独立确定类型的延期实参，其中整数字面量按既有默认阶梯确定类型，浮点字面量按11.2.2的后缀或默认Double规则确定类型；具有完整参数和结果标注的匿名函数也可提供自身类型。候选失败不得影响其他候选或延期实参的类型推断；运行期求值顺序始终遵守 8.5.3。
- constraint system必须同时满足kind/class/interface bound、函数类型型变、nominal application逐项相等、普通subtyping及装箱规则。一个候选只有在所有实例化参数得到唯一、可表达且满足bound的concrete解，并且全部显式与postponed实参都可赋给对应参数时才可应用；不得用`Any`、bound、默认false或任意首个类型补齐无解/多解变量。
- 外层期望类型可以在唯一callable目标已经不依赖返回类型选择时帮助固定只出现在返回结果中的类型参数，也可以为generic nominal构造提供宿主application；它不能使两个仅靠结果类型才能区分的overload变得合法或在多个候选间充当MSC比较项。普通函数签名仍不含返回类型，返回类型不同不能单独形成重载。
- 最具体候选使用独立于本次实际推断结果的pairwise forwarding constraint system：比较`A`是否至少与`B`同样具体时，把`A`的声明参数替换为fresh variables，再检查其每个由调用提供的参数（extension receiver也算）是否可按同一普通subtyping/nominal-invariance关系转发给`B`的对应参数，并同时加入双方声明bound。不能比较两边已经为当前调用猜出的concrete type arguments。
- 若唯一候选能转发给所有其他候选而反向不成立，则它胜出；互相可转发或互相都不能转发时，依次应用规范已有的附加规则：非参数化候选优先；在互相可转发的集合中实际使用更少默认值者优先，仍相同时无`vararg`者优先。命名/位置写法本身不参与优先级。仍不唯一即为歧义。
- 无外层expected type的分支、数组元素或其他LUB计算只有在同一generic template的全部类型实参逐项相等时才能保留该application；`G<A>`与`G<B>`不会合成为`G<LUB(A, B)>`。否则沿普通共同父class/interface/`Any`规则寻找上界，必要时装箱整个value。expected type存在时可让各分支直接按同一个exact target构造/检查，但不能把已经形成的不同application隐式转换到该target。多个互不可比较的nominal共同上界仍不产生交叉类型。
- 整数字面量按11.2持有candidate-local可表示type集合；assignment/return/argument/call-or-operator receiver等位置的exact expected integer type可提交一个可表示literal。literal receiver对integer representation intrinsic及11.8的四个core range member都适用：只枚举八个canonical integer owner，再在假设owner上执行普通core-member决议，不能在查找`and`/`shl`/`rangeTo`/`until`前抢先默认。该规则不枚举任意用户或extension member，也不扩展为普通类型的全局反向推断。fixed point后仍未被约束的无后缀literal按`Int` → `Long`、`u/U` literal按`UInt` → `ULong`选择第一个可表示其值的类型。普通MSC及上述附加规则仍并列时，默认阶梯产生的exact commit优于同族其他fit；多个非默认fit互不支配。literal fit不建立整数type间的subtyping/coercion，失败probe不得泄漏已提交type。
- 无匹配与歧义都是编译错误。诊断必须列出候选层、相关候选的完整签名及失败原因，包括参数映射、类型实参数量、未解变量、bound、实参类型或最具体候选并列。

### 8.7 循环控制

循环与跳转使用第 2 章的 Kotlin 核心语法；本节规定 `while`、`for`、`break` 和 `continue` 的求值与清理规则。

- `break`退出当前callable内词法最内层循环，`continue`进入其下一轮；函数、匿名函数、lambda和局部函数各自建立控制边界，不能跳到外层callable的循环。循环外使用是编译错误；
- `break`/`continue`不产生正常结果并终止当前路径；该路径不参与第5章的结果类型合并。循环正常结束的语句结果为`Unit`；没有更强证明时仍按可能落空处理；
- while的`continue`重新进入完整condition求值入口，包括该condition产生的temporary、safe-call/default setup和挂起调用；for的`continue`进入下一次`next()`；
- 控制转移离开scope时，按从内到外执行所有应执行的catch结束动作和`finally`。finally正常完成后恢复原动作；只有实际离开当前finally的return/throw/break/continue才覆盖原动作，内部被处理的转移不覆盖。新动作继续执行尚未经过的外层cleanup，不能再次进入当前finally。挂起不是退出，不执行finally。

`for (pattern in source)`严格遵守11.8的typed迭代协议；`source`、`iterator()`各求值一次，每轮只调用一次`next()`。每个`Some`建立新的只读binding scope，因此closure捕获对应轮次的值，而不是一个跨轮次覆写的隐藏mutable slot。pattern必须满足4.6的irrefutable binding规则。

---

## 9. 其他语法要点

### 9.1 类与对象

- `class` / `abstract class` / `interface` / `object` / `companion object`均为引用类型。`object`/companion的singleton语义、初始化和generic边界由9.1.3明确规定，不继承Kotlin/JVM的class-initializer ABI。
- 类可以实现 interface，可以继承一个类；struct/enum 不能被继承，也不能继承类。
- 类与成员方法默认均为 `final`。只有 `open` / `abstract` 类可以被继承；类可被继承不代表其方法自动可覆写，普通基类方法必须显式声明为 `open fun` 才能首次被覆写。
- `abstract fun` 隐含 `open` 且没有函数体，只能声明在 `abstract class` 或interface中。interface function有body时是default implementation。覆写class/interface member必须写`override`；`override`默认继续open，可用`final override`终止。
- final 方法不得被覆写。静态接收者上已知的 final 方法调用使用直接分派；open / abstract 类方法调用使用 vtable 分派，interface 方法调用使用 itable 分派。final override 仍替换继承来的 vtable 槽，以保证经基类引用调用时到达该实现。
- `sealed`：`sealed class` / `sealed interface` 保留（引用类型的受限继承）；值类型的等价物直接使用 `enum`。

#### 9.1.1 对象与属性初始化

**property实体与源码形态：**

- property是“显式声明type + getter + 可选setter”的逻辑实体，不等同于field。所有非局部property必须写显式type；同一owner内property名称唯一，不能按type、mutability或accessor重载。同一Cone的package namespace内public/internal top-level property名称唯一；split package中不同origin Cone的同名实体保持不同identity，并在同时导入时按12.4诊断。file-private property的owner包含source file，不同文件可以同名。function和property可以同名并按8.6/9.3.4分区；
- `val/var p: T = expr`是stored property并一定拥有backing storage；accessor语法可以全部省略，也可以自定义getter/setter。`val p: T get() ...`是无field的computed read-only property；interface以外的computed `var`必须同时提供getter和setter。`abstract val/var`把全部required accessor正规化为abstract slot；interface中每个required accessor独立分类，有body为default、未提供body为abstract，因而允许getter/setter混合default/abstract。`val/var p: T by expr`是delegated property，见9.2；
- Scoop永久不支持`lateinit`。唯一可以省略initializer的stored形态是无custom accessor的`var p: Option<T>`/`var p: T?`，其语义等价于在同一初始化位置写`= None`。`val p: T?`、`var p: T`仍须提供initializer或合法computed/abstract body。若需要optional backing field与custom accessor，必须显式写`= None`；generic type parameter不能因某个实例恰为Option而改变声明representation；
- getter无显式参数并返回property type；setter恰有一个不可重新绑定的参数（缺省名`value`）并返回Unit。`val`不能有setter；`var`可只customize一个stored accessor。`field`只在stored accessor的直接body内表示typed backing-field place，computed/abstract/delegated property及nested lambda/local function中不可用；
- primary-constructor property只有implicit field accessor。class/object的stored/delegated property进入common initialization sequence；struct/enum不能拥有stored/delegated member state，但可以声明computed `val`；interface不能有field、initializer、delegate或`init`；
- class property与method使用相同的final/open/abstract/override规则。property override要求名称与type exact相同；`var`可以override `val`并增加setter，`val`不能override `var`。stored/computed/delegated表示都可实现property contract。value type不能实现要求setter的`var`contract；
- property read要求getter可访问，write要求同一property的setter可访问。选中logical property后不能因缺少/不可见setter退回较低候选。receiver与右值各求值一次；assignment、`++`/`--`和复合赋值使用9.3.3的typed place。`?.property`只在Some分支执行getter并把结果再包一层Option，不展平嵌套Option；
- extension property只在top level声明且没有backing field，可以是computed或delegated property。generic extension写作`val <T> Receiver<T>.p: U ...`；全部type parameter必须只由receiver exact静态type与bound唯一确定，不能用result expected type或setter value补推断。generic delegated extension的每个成功使用的exact receiver application拥有program-wide唯一、lazy exactly-once的effective delegate storage与失败状态；其`GenericDelegateStorageSpecializationId`只由origin `PersistentExtensionPropertyId`和完整receiver concrete type arguments决定，不包含result expected type、setter value、re-export路径或使用它的Cone。不同application互不共享storage，多个Cone使用同一application也只执行一次delegate初始化；

**constructor与body member：**

- class primary constructor参数可以是普通参数，或以`val`/`var`同时声明stored property。普通primary参数只在base constructor arguments、class body property initializer与`init`中可见，不进入对象布局，也不能从普通member function或secondary constructor读取。secondary constructor参数不得写`val`/`var`，不声明自己的type parameter或返回类型；它与primary constructor都使用8.5的完整source argument protocol；
- class body可以声明上述property及任意多个`init { ... }`。同一class的primary parameter名称必须唯一；body property不能与primary property或同class其他property重名。遇到inherited同名property必须形成合法显式override，不能静默field shadow。body property可以与普通primary parameter同名；common initialization中无receiver名称优先解析到parameter，property storage用`this.name`直接访问。secondary parameter同样可按普通词法规则遮蔽property；
- 显式`class C(...)`（包括空参数列表）声明primary constructor。普通class既无显式primary也无secondary时拥有隐式internal零参数primary，再与owner effective domain取交集；public class不会因此隐式获得public construction API。若body声明了secondary而header没有参数列表，则不存在primary。intrinsic type省略源码representation不因此获得普通零参数constructor；
- secondary constructor写作`constructor(parameters) : this(args) { body }`或`: super(args)`。class有primary时每个secondary必须经`this`直接或间接终止于primary，不能直接`super`；class无primary时每条链必须终止于一次`super`，省略delegation等价于`super()`：存在direct base时解析其constructor，没有显式base时正规化为编译器根初始化。每个secondary恰好一条delegation edge，自环或多节点环都是定义处错误；
- 有primary的derived class在header用`Base(args)`完成base delegation。无primary、仅有secondary的derived class在header只声明bare base type，由每个terminal secondary执行`super(args)`；两处不能同时提供base arguments。interface不能携带constructor arguments，class最多有一个direct base；
- primary base delegation arguments可以读取全部primary parameter（写`val`/`var`的参数此时也仍以parameter value读取），但不能使用`this`或任何instance field；secondary `this`/`super` delegation只能读取该secondary自己的parameter，也不能访问`this`/field。delegation target的default同样没有source receiver；
- primary与全部secondary共同构成nominal constructor overload set，使用8.6的candidate-local mapping、constraint与MSC。constructor签名只由实际参数类型区分；参数名、default、`vararg`marker及相同的host返回类型不区分重载。abstract class不能普通构造，但其constructor可以作为derived `super`目标；constructor declaration reference不受支持。

**class初始化顺序：**

1. 调用处先选择唯一constructor与完整nominal type arguments，再按8.5求值、物化全部实参；显式实参属于caller，因此可以在suspend caller中先挂起；
2. 分配一次exact concrete对象，写入最派生TypeDescriptor并清零完整payload；随后所有`this`/`super` constructor initializer共享该对象，不重新分配或替换identity；
3. 每条constructor delegation先按其自身源码顺序求值显式实参，再物化目标default/`vararg`，然后direct调用typed目标；
4. terminal primary先完成direct base constructor，再按声明顺序把带`val`/`var`的primary参数写入本class字段；terminal `super` secondary先完成base constructor；
5. body stored initializer、optional synthetic None store、delegate initialization与`init`按照它们在class/object body中的源码顺序交错执行。method、computed/abstract property、nested declaration和secondary constructor本身不执行，也不改变相邻初始化项顺序；
6. terminal secondary body在common initialization之后执行；返回每一层`this` delegation后，再从最内层到最外层执行其余secondary body；最外层正常返回后构造表达式才产生对象引用。

base class的全部constructor body与初始化项先于derived自有字段。每个class的common property/`init`序列在一次构造中恰好执行一次。任一步骤抛异常时，后续初始化项与外层secondary body不执行，构造表达式不产生值，已发生的外部副作用不回滚；失败对象不可达并由GC正常回收，没有析构或runtime回滚。

**字段就绪与半初始化receiver：**

- base initializer正常返回后base storage就绪；primary property在对应compiler store后就绪；body stored/optional/delegate property只在其initializer与storage write正常完成后就绪。initializer与`init`只能direct读写已经就绪的inherited/primary/earlier backing field；self/forward read、通过`this`绕过顺序及提前写later `var`都是定义处错误。computed/delegated accessor不能以initializing receiver调用，分配时的全零payload不是合法源码默认值；
- class/struct constructor、class property initializer与`init`中的`this`是受限initializing receiver：只可用于已经就绪字段的direct read与mutable write。它不能作为普通值传参、返回、存储、捕获、装箱、转换、比较、取址或形成callable reference，也不能作为ordinary/extension/virtual/interface/`super` method receiver。读取field后得到的值是普通值，可以正常参与调用；
- 上述限制对base constructor同样成立，因而构造期间不能通过virtual dispatch观察derived未初始化字段，也不能依赖whole-program escape analysis判断某个final helper“可能安全”。需要init-safe callable时必须另行引入typed effect；
- `return`不能退出property initializer、`init`或constructor body；`throw`合法。其中声明的局部函数与匿名函数按8.1.2建立自身的返回边界，`return`只返回该函数；lambda中的裸`return`仍非法，嵌套callable也不能捕获initializing receiver。所有这些体及delegation都是ordinary、safe、non-suspend上下文；可以调用普通函数、分配、触发GC，也可以构造尚未执行的suspend task，但不能立即调用suspend函数。`startCoroutine`本身是同步普通builder，其启动计算不成为尚未完成的初始化步骤。

**`super`成员调用：**

- 普通member body中的`super.name<TypeArgs>(args...)`只在direct base application的class member层按8.5/8.6决议，不收集extension、property-like或interface default候选。winner强制direct调用base静态视图中的具体实现，即使最派生对象覆写同一virtual family也不经vtable；abstract且没有具体base实现的目标不可调用；
- `super<I>.member`的interface default规则见9.1.4。`super`不是一等表达式；裸`super`、`super.field`、`super::name`、safe/invoke/index形式非法。初始化上下文也不能以任一super形式绕过受限receiver。

普通property accessor、delegate访问和object/global initializer都同步且non-suspend；可以分配、GC和抛异常。需要异步取得值必须显式暴露`suspend fun`，不能藏在getter或delegate协议中。

#### 9.1.2 `const val`

- `const val`只允许声明在top level、`object`或`companion object`中；必须有显式type和initializer，不能是extension/local、`var`、delegate或带accessor。其type必须是`Boolean`、基本数值类型、`Char`或`String`。
- initializer 必须是编译期常量表达式：字面量、对其他 `const val` 的引用、由它们组成且可在编译期确定结果的内建一元/二元运算，以及通过 typed numeric intrinsic registry 精确解析到整数或浮点表示运算、显式数值转换的封闭 call。后一类包括整数的 `compareTo`、两类数值的 `inc`/`dec`、`equals`、`div`/`rem`、`toX`，以及浮点的分类和 `isTotallyOrdered` 方法；只有 exact typed registry identity 才使 call 成为常量表达式，用户声明或仅同名的 callable 不获得该能力。所有实参都必须是常量表达式；整数常量 `div`/`rem` 的除数为零是 const 定义错误，浮点常量除零按 11.2.2 产生 Infinity 或 NaN。const 依赖图存在循环是编译错误。
- 除上述封闭数值表示 intrinsic 外，函数/方法调用（包括 `toString`）、构造、普通属性读取、数组或其他对象分配、`throw` 及挂起调用都不是常量表达式。`const val` 不生成需要在程序启动或单例首次访问时执行的 runtime initializer；位于 `object` / `companion object` 中的 `const val` 引用本身不触发单例初始化。
- 导出的 `const val` 的类型和值属于 `.slib` 接口，下游 Cone 在编译期直接使用；值变化使依赖该值的编译缓存失效。const 没有 getter 或可寻址 storage，`addressOf(const)` 非法；可见性按声明检查，不因常量求值而绕过。

#### 9.1.3 `object`、companion、nested declaration与全局初始化

- `object O`同时声明一个nominal ref type与singleton value，二者identity类型化且不同。object不能自行声明type parameter或primary/secondary constructor，可以继承一个class并实现interface；base constructor后按9.1.1执行property/delegate/`init`。普通top-level/static nested object只有一个singleton；companion按下述完整宿主application区分singleton。`O`不是普通constructor，`O()`非法；
- 依赖 Cone 中的 object 遵守同一规则：类型位置引用原 nominal，值位置引用原 singleton value；二者来自同一声明/application时不构成值查找歧义。每次值访问先确保对应初始化单元成功，再读取其已发布根。转导出、默认参数展开、成员访问和下游再次发布都保留原实体身份；同一companion application跨Cone只对应一个逻辑对象，普通泛型物化及ODR必须合并其状态和初始化支持；
- class/struct/enum/interface/object至多有一个`companion object`，省略名称时为`Companion`。每个完整宿主类型拥有独立的companion类型与singleton：`Box<Int>.Companion`和`Box<String>.Companion`是不同类型、不同对象，各自拥有成员状态；同一`Box<Int>.Companion`的重复访问得到同一对象。即使companion不使用T、布局相同或没有字段，也不能合并不同宿主实参的对象或类型identity；
- companion的声明及成员可使用直接宿主的类型参数和已有bound，包括字段、基类/interface application、参数、结果、default和初始化表达式中的类型位置；例如`fun create(value: T): Box<T>`合法。它不捕获宿主实例、primary parameter或其他实例值，this始终是companion自身。companion没有另行书写的类型参数列表；内部普通generic function可声明自己的参数，按3.2与宿主参数分别绑定，不能重名；
- 实际引用generic companion的类型、值、成员或const时，宿主必须写出完整实参，例如`Box<Int>.Companion`、`Box<T>.Companion.create(value)`、无冲突时的`Box<Int>.create(1)`。T可以是当前作用域已声明的参数，随后正常单态化；缺少实参的`Box.Companion`/`Box.create(1)`及带`_`的宿主限定不形成完整companion引用，不从方法参数或期望结果推断宿主。const仍无runtime初始化，不能因此省略宿主实参。非generic宿主继续使用`User.Companion`/`User.create(...)`；
- 命名companion的`Box<Int>.Factory`与`Box<Int>.Companion`是同一application。透明类型别名如`typealias IntBox = Box<Int>`不产生新对象；`IntBox.Companion`与`Box<Int>.Companion`相同。裸generic名称仍可用作声明命名空间限定符，import只定位声明，不能代替形成companion类型/值所需的宿主实参；没有新增`Companion<Int>`或带类型实参的import语法。companion member不进入宿主instance lookup，不被派生class继承；
- 跨 Cone 的 companion、嵌套声明和转发按实际 host 的静态命名空间解析，保留完整宿主application。`Host.member`只查实际导出的转发binding，再使用成员所属companion application的真实receiver，不生成static副本。限定路径本身不构造或初始化host，const读取也不触发singleton初始化。companion属性赋值、复合赋值与自增均先求值并保存一次实际receiver，再求值右值并调用对应getter/setter；可见性、值遮蔽与本地声明相同；

- body可以声明static nested class/struct/enum/interface/object，包括companion body中的普通嵌套声明。它们没有implicit outer receiver或outer type parameter；需要关联时显式声明自己的参数。generic outer名称可作为owner qualifier而不构成裸generic application；`Box.Nested`仍是独立声明，不因`Box<Int>`/`Box<String>`复制。只有companion按直接宿主application参数化，普通static nested声明仍是外层类型参数作用域的边界。`inner class`、anonymous/local object/type及implicit outer instance capture不支持；
- ordinary top-level stored/delegated property可以是`val`或`var`、可以包含managed ref，使用compiler-managed hidden storage/accessor并进入global root表；它不可`addressOf`。`@Global`/`@ThreadLocal`仍只表示13.6的显式可寻址GC-free raw storage，`@Extern`仍只表示C data symbol；这些storage形态不能带普通accessor/delegate或与ordinary property混用；
- 泛型正文对普通顶层属性的读写始终操作声明方的同一份状态，包括 private/internal 属性、静态初值与运行时初值。不同类型实参、不同消费 Cone、再次发布以及嵌套 callable 不复制该属性的 storage、初始化单元或 GC root。模板保留定义处已解析的属性访问，不使该属性进入消费方的源码可见域；
- 需要运行期求值的顶层属性在全部 Cone 的运行时数据、GC 与主线程就绪后、`main` 前同步初始化一次。只有无需执行 Scoop 代码或读取普通属性、可以直接表示为静态初值的字面量、内建纯常量表达式、String 常量及 Option `None` 可省略运行期初始化；事后常量折叠不能改变可观察求值。
- 文件之间没有初始化源码顺序。同一 Cone 的 eager 初始化按 `PersistentInitializationUnitId` bytes 排序，跨 Cone 顺序见 12.3；identity 来自原 Cone、声明 owner、package、kind、name，file-private 声明另含标准化的 Cone-relative source identity，不依赖文件枚举、重导出路径或 host 路径。
- 访问另一个初始化单元时先确保目标初始化成功。初始化表达式、delegate、object 基类实参及 `init` 中的直接初始化依赖环是编译错误；经普通函数、构造器、默认值、动态调用或 FFI 形成的间接环由运行时检测。启动初始化失败时不执行 `main`。
- object/companion在首次非const访问时线程安全、同步初始化；static nested declaration或const引用不初始化外层。每个runtime unit状态为Uninitialized、Initializing(owner/dependency stack)、Initialized或Failed(rooted Throwable)。成功singleton只在完整初始化后release发布；失败不发布、记忆异常且不重试。同线程或跨线程wait-for环抛出`message`含稳定unit path的`IllegalStateException`；该异常若未在initializer内被普通`try`捕获才使unit失败。其他线程以可参与safepoint的方式等待terminal state；
- 每个实际使用的 companion application 独立执行上述 exactly-once 协议，初始化结果、失败状态和副作用互不串用。同一 application 跨 Cone 共享初始化状态；不同 application 只有显式初始化依赖才互相触发。只构造 `Box<Int>` 不初始化其 companion，只访问 companion 也不构造 Box 实例；单纯类型引用或静态描述查询不执行初始化。未具体化的 companion 模板没有运行期对象；
- exactly-once 状态管理完整初始化的发布，不允许属性缺少声明类型的合法值。静态初值在任何 managed 代码执行前有效；省略 initializer 的 Option 属性具有普通 `None` 值。

#### 9.1.4 Interface default implementation

- interface function有body时提供default，无body时形成abstract obligation；property按getter/setter slot分别判断。private interface member必须有body且只作词法helper，不进入itable、继承或override；
- 对每个typed slot先选择class hierarchy中最近的实际方法声明；具体声明成为实现，抽象声明保留该声明的义务并压制接口default。没有class声明时，删除被更specific subinterface覆写的interface候选，唯一剩余default获胜，只剩abstract即保留实际抽象声明，多个互不相关default则必须显式override。getter/setter独立选target，但property整体仍满足9.1.1的type/mutability规则；
- concrete class/object/value type不能留下abstract obligation；abstract class可以保留。itable entry显式指向class/value implementation、interface default或typed adjust thunk，不能按implements列表顺序选择；
- 普通member/default/accessor body中的`super<I>.function(args)`、`super<I>.property`与`super<I>.property = value`只direct调用当前owner显式列出的direct superinterface exact application上的concrete default。抽象target、间接/非父qualifier、extension/property-like/callable-reference/safe形式非法；初始化上下文仍禁止。interface default body也只能这样选择自己的direct superinterface。

#### 9.1.5 可见性与annotation

- 默认visibility是`internal`。所有允许visibility的声明在省略modifier时都具有internal可见性，不从owner/base/interface继承visibility；setter省略modifier按下条继承property visibility。该规则适用于top-level nominal/function/property/object及nominal中的method/property/nested declaration/constructor；local、parameter、`init`块与accessor parameter不能声明visibility。`main`按入口契约发现，不要求public，也不因internal进入`.slib`导出表面。top-level允许public/internal/private，其中internal表示当前Cone、private表示当前source file；member/nested允许public/internal/private，其effective domain还要与全部owner domain取交集。protected只允许class member/nested/constructor，表示声明class及subclass body可见；explicit receiver的静态type还必须是当前访问subclass或其子类；
- 继承class的object/companion在自身body与基类构造委托中同样作为subclass参与protected检查；当前访问owner及explicit receiver沿其typed backing-class关系判断继承，不改变其object词法owner或singleton identity。非subclass上下文、基类静态type或其他兄弟subclass的explicit receiver仍不得访问protected成员。
- protected检查保留词法嵌套上下文：当前nominal及其词法owner链中任一class（object/companion按typed backing class）都可提供subclass访问上下文。explicit receiver必须相对于同一个提供权限的class满足静态type约束。故class内private成员的词法域包含于该class的protected域，protected property可使用private setter；这不会使子类获得基类private setter的权限。
- effective domain中由外层nominal带入的protected约束只检查访问点的词法权限，不把nested类型实例的receiver当作外层class的receiver。仅当所访问的method/property/accessor自身声明为protected时，才相对于该成员的声明class执行explicit receiver静态type检查；public/internal/private成员即使位于protected nested类型中，也不会额外获得protected receiver限制。setter使用setter自身的visibility与logical property owner。
- 候选只有在当前访问点属于其 effective domain 后才进入 applicability/MSC，成功结果保留实际声明引用与访问域；同名但不可访问的声明只参与诊断，不会让该层遮蔽较低层。getter可见的logical property一旦选中后，setter缺失或不可见仍按9.1.1直接报assignment错误，不触发fallback；
- struct/enum字段与enum variant保持固定public representation visibility，但仍与owner domain取交集；只有外层value type显式public时才对下游公开表示。普通member/nested的direct lookup domain也是声明visibility与owner domain的交集。private member不被继承或override、不进vtable/itable；internal open member只可在同Cone override；
- interface member同样默认internal。internal interface可拥有internal abstract/default contract；public interface的abstract/default contract必须显式public，不能把遗漏modifier静默升级。private interface helper必须有body且不进itable/override，protected interface member非法；因此public interface不能携带下游不可见的hidden obligation；
- abstract member的slot contract必须覆盖owner的合法inheritance domain；public abstract/open class中的abstract member至少显式protected或public，internal/private owner则可使用internal obligation。下游不可见的abstract member不能用来把public type隐式变成sealed；已有body的internal open member不形成hidden obligation，下游继承但不能override；
- override省略modifier时仍为internal，不继承base visibility。coverage比较声明visibility形成的slot contract domain，而不是被concrete owner收窄的direct lookup domain；它必须覆盖全部被覆写slot，因而public contract需要`public override`，即使实现type是internal/private。此时直接名称访问仍受owner限制，经base/interface静态类型调用则服从public slot。显式`protected override`覆写protected槽时保留该槽原声明class的protected region，不把slot覆盖域重新锚定到实现子类；直接名称查找仍按实现声明的owner检查。此规则不允许protected实现收窄public槽。允许显式扩大。getter沿用property visibility；setter可声明不更宽的private/internal/protected visibility。选中property后setter不可见是assignment错误，不改选其他候选；
- declaration signature中的type、receiver、base/bound及annotation type必须覆盖该declaration的direct access/call domain。经base/interface调用时使用该静态声明的签名及其完整类型实参；override的slot coverage不把实现者的签名扩展为独立public API。因此internal/private实现者可为非public类型实现public泛型接口，公开接口声明与公开返回类型仍不能泄漏不可见类型。default直接绑定实体须覆盖实际展开该default的call domain。visibility在const folding、companion forwarding和desugaring前检查；default引用定义处绑定的声明；其访问域必须覆盖定义处及继承后实际允许调用的范围；
- class primary constructor需要modifier/annotation时写显式`constructor`关键字；无modifier的class header/explicit primary及class/struct secondary constructor均为internal。class primary property parameter可声明member visibility/override，普通parameter不可。class隐式零参数constructor为internal并与owner domain取交集；public class需要显式`public constructor`才提供public construction API。struct primary constructor与enum variant constructor属于固定public representation entry，只与owner domain取交集且不能单独声明visibility；
- property-level custom annotation不传播到accessor/backing/delegate storage；explicit accessor可单独标注。`@Unsafe`/`@Safe`可用于constructor/accessor并进入调用contract；`@NoGC`只在完整signature/body确实GC-free的explicit accessor或struct secondary constructor合法。class/object receiver为ref，不能满足NoGC。`@Extern`/`@Global`/`@ThreadLocal`及`@CallingConvention`仍限各自13章target；普通property/object/constructor不能伪装为native symbol。
- constructor的`@Unsafe`/`@Safe`控制该声明的参数缺省表达式、`this`/`super`委托表达式及secondary body；overload选择完成后才检查所选constructor的safety，不因safe调用上下文改选其他候选。class/object的property initializer、delegate与`init`属于共同初始化声明，使用独立safe上下文，不继承任一constructor的注解；需要unsafe操作时使用显式`@Unsafe` block。

#### 9.1.6 GC-free release block

普通 final class 可声明至多一个 `release { ... }`，在遗漏显式释放时兜底清理 native resource。

```scoop
@Extern(name = "free")
fun nativeFree(address: Ptr<Unit>)

final class NativeOwner public constructor(
    private var handle: Option<Ptr<Unit>>,
) {
    public fun close() {
        val owned = handle
        handle = None
        when (owned) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }

    release {
        when (handle) {
            Some(value) -> @Unsafe { nativeFree(value) }
            None -> {}
        }
    }
}
```

`release` 只在 type body 的 member 起始位置、后接 block 时作为 contextual keyword。`fun release()`、局部变量及普通参数中的同名 identifier 保持原义。release block 没有名称、参数、返回类型、visibility、annotation 或 modifier；不进入 lookup、overload、override、dispatch 或 callable reference。正常完成结果固定为 `Unit`；block 内的 `return`、`throw`、`try`、挂起及局部 callable 声明均为编译错误。`while`、`break`、`continue`、`when` 和值绑定沿用已有规则，但所有实际执行的操作须满足下述 effect。

**owner 与字段读取：**

- owner 必须是普通、可实例化的 final class，默认 final 也合法；`open`/`abstract`/`sealed`、interface、struct、enum、annotation class、intrinsic class、object/companion 及 compiler-generated class 均不能声明。`Throwable` 的继承闭包同样排除，因为异常记录会按值复制 payload。static nested 与 generic final class 合法；可以继承普通无 hook 基类并实现 interface，release 不继承、不覆写、不形成 hook chain。
- block 没有 managed `this`，只在独立的 reclaiming-receiver 上下文中读取本 owner 自己声明的 backing field。primary property 与 body stored property 均可读；即使 stored property 有 custom getter，此处仍直接读取 backing storage。computed、delegated、inherited property 及 primary constructor 的非 property 参数不提供这样的字段。局部绑定按普通词法规则遮蔽字段，不能用 `this` 绕过遮蔽。
- 字段读取取得普通值副本；字段赋值、复合赋值、更新及 `addressOf(field)` 均非法。对副本的值类型投影、解构、局部更新及合法 native pointer 操作遵守现有规则。源对象不能被传参、返回、存储、捕获、装箱、cast、比较、取址或取得 root/handle/pin；`super`、本对象的 getter/setter/method/extension 不能被调用。

**`ReleaseValue`：**

`ReleaseValue(T)` 是普通类型属性：exact `T` 必须 GC-free，并且其实际递归表示不包含已解析 compiler protocol 所指的 `PinnedPtr`、`GcHandle`、`FunPtr` 或 `ForeignCallback`。判定使用真实 typed declaration/representation，不使用 FQN、字段形状或 core 来源资格。它不建立新的源码 trait、annotation 或资源所有权类型。

Boolean、数值类型、Unit、只由合格字段组成的 struct/enum/tuple、相应 Option 均可用；enum 要求全部 variant 合格。`Ptr<Unit>` 可用，`Ptr<T>` 还要求 pointee `T` 满足同一条件，不能通过 `Ptr<GcHandle<...>>` 绕过限制。指针间接形成的合法递归类型按有限类型图求属性固定点，不能因回访节点就报错或无限展开。只检查参与表示的参数，phantom type argument 不额外受限；`sizeOf<T>()` 等纯布局查询也不因类型实参而产生运行时 `T` 值。

block 的字段读取、local、temporary、实参和结果都须满足该条件；未被读取的 managed 字段不影响 owner 合法性。generic release template 保存实际需要的 `RequiresReleaseValue` 条件，沿已有类型替换规则传播；闭合 application 即使只出现在签名、别名或字段中也必须满足，不能等到构造或回收才检查。依赖 `ref` bound 的不可能条件在 release 定义处诊断。`Ptr<Unit>` 和 integer 的合法性不证明其地址来源、生命周期或 ownership；通过 unsafe/native 代码访问 GC heap、隐藏 managed reference 或恢复源对象仍违反 unsafe/FFI 契约，编译器不增加通用来源追踪或防伪机制。

**release-safe 操作与调用：**

- release 是比普通 `@NoGC` 更窄的执行上下文，不是 `@NoGC` annotation target，也不新增公开 `@ReleaseSafe`。禁止 managed allocation/ref、boxing、String/Array/closure、异常、suspend、safepoint/poll、初始化 ensure、动态/接口/间接调用及 root/handle/pin/thread/GC runtime 操作。
- 直接调用的 Scoop helper 必须有实际 Scoop 正文、满足既有 NoGc 合同，并经定义方推导为传递 release-safe。普通顶层/扩展函数、值类型 direct method/accessor、值类型 secondary constructor 及编译器已有的纯存储 accessor 使用同一规则；值的 primary construction 和 enum assembly 仍为普通值操作。未标注且未由既有规则生成 NoGc 合同的 callable 不因正文看似简单而自动放行。任何 C/Scoop ABI extern 函数调用、native transition、TLS、managed 操作或捕获环境都会使 helper 不可用于 release；不为 helper 生成另一份 release-context body。
- 推导结果和泛型条件保存在已有 callable 接口中。依赖方直接消费该效果信息与真实 typed target，不重新读取非泛型 helper 源码或遍历其完整实现调用图；泛型正文仍在本次正常实例化中检查替换后的值和实际调用。没有额外调用资格、凭证或独立信任链。
- release block 本身可以在显式 `@Unsafe` 中直接调用 C ABI extern。投影后的 native 签名须满足 C-FFI-safe，全部 Scoop 参数及结果还须满足 `ReleaseValue`；此调用直接使用 native ABI 或 storage bridge，不执行 managed/native transition。允许 13.4.2 的 `captureErrno = true`：bridge 只在本次调用内清零、读取 libc 的 errno，并返回普通 `(R, Int)` 值；不访问 Scoop TLS 或 thread runtime，不需要 collector 保存、恢复其他调用的错误值。这不放宽一般 TLS 访问限制，也不改变 Scoop helper 的 release-safe 推导规则。同一 extern 在普通代码中的调用仍走既有 FFI 协议。C 实现必须不展开异常、不回调任何 Scoop 入口、不操作 GC/root/handle/pin/thread runtime、不保留 hook 的临时地址，也不等待已停顿的 mutator 或依赖其进展。这是调用处承担的 unsafe native 契约，不能由 `@Extern` 拼写、`nounwind` 或对象文件符号扫描证明。
- effect 在普通名称查找、唯一目标选择和完整实参展开之后检查。默认参数、operator/accessor、解构、`for`、`vararg` 等产生的操作一并检查；不能因 effect 不合格而退回另一个重载。运行期整数 `/`、`%` 仍按 11.2 调用可抛异常的 Managed 运算，即使受 `if` 保护或除数为常量也不允许；已合法求值的 GC-free const 可直接使用。不新增路径证明或循环执行预算。
- `@Unsafe`/`@Safe` 嵌套规则不变。TLS、singleton 和需要 ensure 的普通全局属性不可访问；GC-free const、无 ensure 的合格普通存储 accessor、非 TLS 的 GC-free raw/native global 可按原可见性、unsafe 和数据竞争规则访问。native global storage bridge 只进行普通存储操作；`@ThreadLocal` 不能借 helper 或 bridge 绕过限制。

**生命周期：**

1. 分配时内部 `RELEASE_READY` 为 0；完整最外层 constructor 正常返回后、构造表达式产生对象值前，由生成代码置 1。base/`this` delegation 不发布，异常出口不发布；本地、依赖与泛型构造遵守同一规则。构造失败前取得的 native resource 由构造/调用方显式清理。已经独立构造成功的子对象保持自己的生命周期。
2. collector 在对象逻辑死亡且 storage 即将真正回收时，先原子清除 ready，再同步调用 exact TypeDescriptor 的 hook，返回后才能 poison、复用或释放 storage。live、pinned、被 root/handle 保活的对象不调用；moving 的旧副本不表示逻辑死亡，ready 随存活对象搬迁。ABI 与顺序见 运行时规范 2.1、2.2、3.8。
3. best effort 不保证 GC 时机、对象间顺序、执行线程、native release 结果或退出时调用；正常 collection 一旦回收 ready 对象，就必须在存储失效前尝试一次且至多一次。shutdown 不补做全堆释放。
4. 显式 `close`/`release` 与 `try/finally` 仍负责确定性释放。显式路径先将字段置为 `None` 等 inert state，再释放取出的资源；hook 以后仍可运行，但不再释放该资源。并发关闭和 native handle 别名由库自身的同步/ownership 契约处理。语言不自动生成 close，也不公开 arm/disarm、手动 hook、重试或异常传播接口。

release 不提供 GC finalizer、对象图访问、对象复活或及时释放保证；不能依赖它完成 flush、事务、锁或要求特定线程的操作。

### 9.2 委托

- property delegate使用reflection-free协议。角色必须显式声明为ordinary、non-suspend、non-generic `operator fun`，不得带default/`vararg`：可选`provideDelegate(): D2`、必需`getValue(thisRef: R): T`，以及`var`必需`setValue(thisRef: R, value: T): Unit`。它们是与9.3普通operator不同的typed role；无role的同名函数不参与；
- role 查找沿普通成员优先、扩展按作用域分层的可应用性与最具体候选规则进行。同一层中的本地声明和依赖声明共同比较；import alias 不改变声明的 typed role，也不使扩展覆盖已有可应用成员；
- `R`对class/object member是owner type，对extension是extension receiver，对top-level/local是Unit。先求值`by`表达式一次，再可选调用一次无owner参数的provideDelegate并把effective delegate存入hidden field/global/local；之后每次access读取delegate并调用唯一get/set target。协议没有`KProperty`、property name或annotation metadata；需要这些值必须在`by`表达式中显式传入；
- 非局部delegate必须显式声明property type。local delegate省略type时，先在没有result expected type的条件下选出唯一`getValue`role，再以其concrete结果作为property type；`var`的`setValue`必须接受同一type，不能从多个get/set组合反向猜type或让setter改变getter结果；
- class/object 的 delegate 按 9.1.1 的共同初始化顺序求值；top-level 与 non-generic extension delegate 按 9.1.3 提前初始化。generic extension delegate 按 9.1.1 为每个 exact receiver application 保留独立、全程序共享的 lazy 初始化状态，首次访问时才求值 `by` 并发布 effective delegate；即使编译期已知使用点，也不能提前执行。`by` 与可选的 `provideDelegate` 使用定义处绑定的名称、重载及 receiver 类型实参；二者不取得本次访问的 receiver 值，get/set 才接收实际 `thisRef`。普通赋值先求值 receiver 与 RHS，再进入 setter 的初始化检查；复合赋值先执行 getter 及初始化检查，再求值 RHS。同一 application 跨 Cone 共享 delegate 和初始化结果，re-export 不复制状态。local delegate 是不可重新绑定的局部值。struct/enum member 不能使用 delegated property。delegate 调用同步，可以分配、触发 GC、抛异常，但不能挂起；
- 不支持 class/interface delegation 语法 `class C : I by impl`。

### 9.3 运算符重载与约定

#### 9.3.1 `operator`声明

参与运算符约定的函数必须显式写`operator` modifier；仅仅使用约定名称不会使普通函数成为运算符。除下述`equals`外，operator必须是成员函数或extension函数，可以是ordinary/suspend、generic或infix。operator标志属于override contract，override与被覆写声明必须完全一致。

参与运算符约定的声明必须满足以下名称和签名约束：

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
| `compareTo` | 1 | 返回`Long` |
| `equals` | 1 | 返回`Boolean`；见下述收紧规则 |
| `componentN`（`N`为正十进制整数） | 0 | 返回类型不限 |
| `iterator` | 0 | 返回值在`for`使用点满足11.8的`Iterator<T>`协议 |

固定元数角色的参数可以具有default，但operator语法提供的operand仍按8.5映射到对应参数；只有`get`、`set`和`invoke`能以`vararg`表达可变元数。名称不在表中、元数或返回约束错误、`component0`及非数字`component`名称都是声明处错误。

属性委托所需的`provideDelegate` / `getValue` / `setValue`不是本表的`set`下标角色；它们按9.2形成三个独立typed role，不能仅按名称或本表的普通operator identity参与delegate协议。

`equals`是本规范对Kotlin约定的有意收紧：`operator fun equals` 的根声明由 core `Equality<T>` 提供，实现／重声明必须关联到该接口的实际 slot，并遵守普通 override 规则。其恰有一个显式参数并返回 `Boolean`，不得为generic或suspend。顶层、局部、extension，以及未实现／继承对应 Equality application 的成员不得声明该operator。普通非operator函数仍可名为equals，但不参与`==`。不同 Equality application 可以形成普通成员重载，具体决议见11.11。

`equals`签名中的参数必须写普通显式类型。Scoop没有`Self`类型：core 声明 `interface Equality<T> { operator fun equals(other: T): Boolean }`，实现者选择 `Equality<Point>` 等 application。用户自己的相等接口可继承 `Equality<T>`，不能仅声明同形方法取得该operator契约；编译器不把interface中的任何名字隐式替换为实现者类型。

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
| `a < b` / `a <= b` / `a > b` / `a >= b` | 同一种 core 浮点类型按 11.2.2 直接执行 IEEE 比较；其他类型将 `a.compareTo(b)` 的 `Long` 结果与 0 比较 |
| `a == b` / `a != b` | 11.11的成员`equals`调用 / 对同一结果取反 |

receiver先于调用实参求值，因此`a in b`与`a !in b`按概念调用先求值`b`、再求值`a`；这是有意保留的Kotlin顺序。其他表项按书写的receiver再到operand顺序求值。`&&` / `||`仍是只接受`Boolean`的内建短路操作，`===` / `!==`仍是不可重载的引用identity比较；`=`, `?:`, `!!`, `is` / `as`及安全导航本身也不可重载。

core 浮点关系运算的例外由已解析 nominal owner 的 typed representation 决定；透明 alias 相同，用户定义的同名类型不获得该行为。两个 operand 从左到右各求值一次，literal 按 11.2.2 定型；不先默认为 Double，也不通过 `compareTo` 或 total-order 方法间接实现。该例外不增加可由用户重载的 operator 名称。

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

下标 place 的读写分别通过同一静态 receiver 的 `get` 与 `set` operator；receiver、全部 index 和右操作数均只求值一次。

#### 9.3.4 property-like `invoke`与候选分区

任意表达式`e(args...)`先定型`e`：若其类型是8.1的函数类型，执行内建函数值调用；否则只从其静态类型收集成员及可见extension `operator fun invoke`，并按普通调用决议。`FunPtr<F>`仍不提供Scoop侧`invoke`。

对名称调用，property-like callable表示“先读取该名称对应的值，再对结果应用一次上述invoke规则”。显式receiver调用的c-level分区顺序为：

1. member function-like callable；
2. member property-like callable + member `invoke`；
3. 各extension作用域中的extension function-like callable；
4. member property-like callable + extension `invoke`；
5. extension property-like callable + member `invoke`；
6. extension property-like callable + extension `invoke`。

local、parameter、capture、global、普通成员或扩展属性，以及 object/companion value 均可作为 property-like 来源。每个组合先选择 property，再以其结果类型选择 invoke；组合优先级取两部分中较低者。显式类型实参、命名、spread 和尾随 lambda 只转交 invoke，不作用于 property 读取。

为保留8.1.3已确定的词法遮蔽，最近词法作用域中同名的函数类型binding，或静态类型具有至少一个可见`invoke`operator的binding，先形成唯一local property-like层并遮蔽同名函数声明；其调用形态/类型不匹配时针对该binding诊断。普通不可调用binding不参与callable层。其他层仍遵守8.6的“第一个含可应用候选的分区”，不能因存在同名但不可应用的property无条件阻断后续函数。

#### 9.3.5 `infix`

`infix`函数必须是成员或extension函数，且恰好有一个required、非`vararg`的显式普通参数；可以同时是generic、suspend及`operator`。不满足声明形态、用于top-level非extension或local非extension函数都是声明处错误。`infix`标志与`operator`一样属于override contract。

`lhs name rhs`等价于`lhs.name(rhs)`，要求直接function-like候选带`infix`，或property-like候选最终选中的`invoke`同时带`operator infix`；其余决议与求值规则不变。infix调用左结合，必须显式写receiver（当前`this`上调用写成`this name rhs`）。其优先级从高到低位于range与Elvis之间：postfix、prefix、cast、乘法、加法、range、infix name、Elvis、`in`/`is`、比较、相等、`&&`、`||`、赋值。不同infix名称没有自定义优先级。

普通class可以通过`componentN` operator支持位置解构；每个实际需要的位置独立解析对应operator并只求值被解构值一次。struct/tuple仍走4.6的内建解构，不查找`componentN`。`iterator`只定义11.8中`for`脱糖的入口。

### 9.4 注解

自定义注解是编译期声明。语言核心注解（`@Intrinsic` / `@NoGC` / `@Extern` 等）遵守第 13 章；不内置平台相关注解，也不提供运行期 annotation 对象。

```scoop
public annotation class Description(val text: String)

@Description("Account data")
public struct Account(@Description("Stable identifier") val id: Long)
```

- annotation class是编译期声明，具有普通名称、typed declaration identity、可见性和import规则；不是可实例化的runtime class，不具有继承、interface、泛型参数、body或成员函数。参数为`val`，类型限于Boolean、String、Char、现有定宽整数，以及Float/Double；无参数声明可省略括号。
- 使用处采用`@Name(...)`或限定名称，沿普通符号解析选定实际声明。参数遵守位置/命名参数映射，可以有缺省常量；值限于上述类型的字面量、带符号数值字面量和已绑定的同类型`const val`。const val 的限定引用沿普通名称和完整宿主 application 规则，包括 `Box<Int>.Companion.NAME`；只读取已绑定的常量，不执行 singleton 初始化。不得执行任意函数、构造用户对象或把类型作为annotation值。整数范围、浮点字面量的目标精度与溢出、重复/缺失/未知参数及可见性错误在定义或使用处诊断；浮点常量以类型和原始位型保留，不用数值相等合并正负零或NaN。
- 自定义注解可标在名义类型、enum variant、struct/variant字段和class/interface的logical property上。主构造参数带`val`/`var`时注解属于该字段/property；普通值参数不因此成为可注解字段。不增加use-site target、可重复注解、注解继承、元注解执行或编译器插件API。同一target重复同一annotation声明是错误；不同注解按源码顺序保留。
- 注解的参数按声明序正规化为typed常量，包含已补齐的缺省参数。泛型application保留原声明的注解；不因具体化产生新的annotation声明，也不把宿主注解复制到字段、派生类、accessor或backing storage。logical property与实际存储的关系遵守9.1.1、9.1.5。
- 注解本身没有可执行副作用。普通用户注解仅进入9.6的静态描述；只有已规定语义的核心注解参与编译。导出的注解声明及应用保留实际类型/常量引用和必要依赖，读入`.slib`后不重新按短名称解释；这些数据不扩大普通源码可见性。

### 9.5 companion 与普通 codec 的编码、解码接口

两个方向都用显式目标类型参数表达，由companion或普通codec对象实现：

```scoop
public interface Encodable<T> {
    public fun encode(value: T, encoder: Encoder): Unit
}

public interface Decodable<T> {
    public fun decode(decoder: Decoder): T
}

public struct Identifier(val value: Long) {
    public companion object : Encodable<Identifier>, Decodable<Identifier> {
        public override fun encode(value: Identifier, encoder: Encoder): Unit =
            Long.Companion.encode(value.value, encoder)

        public override fun decode(decoder: Decoder): Identifier =
            Identifier(Long.Companion.decode(decoder))
    }
}

public fun <T> encodeTo(value: T, encoder: Encoder, codec: Encodable<T>): Unit =
    codec.encode(value, encoder)

public fun <T> decodeFrom(decoder: Decoder, codec: Decodable<T>): T =
    codec.decode(decoder)
```

- `Encodable<T>`与`Decodable<T>`都是普通invariant generic interface，T是普通显式类型参数。encode的数据参数和decode的结果直接写Identifier、`Box<E>`或tuple等完整类型；没有Self替换、associated type或static成员语法。
- `Identifier.Companion`实现这两个接口，Identifier实例不因此实现它们。两个方法的this都是codec；encode通过value取得待编码数据，decode返回目标值。实现选择、override、slot、可见性及异常均使用普通方法规则，缺省body的合成见11.13。
- `Identifier.Companion.encode(value, sink)`与`Identifier.Companion.decode(source)`都是普通成员调用；无冲突时可按9.1.3转发为`Identifier.encode(value, sink)`与`Identifier.decode(source)`。generic代码显式接收相应codec值；不增加`T.encode`/`T.decode`/`T.Companion`泛型查找、隐式codec参数或companion bound。
- companion随完整宿主application具体化，可声明`Encodable<Box<T>>`或`Decodable<Box<T>>`，但含T字段时仍须在定义处取得相应字段codec。需要调用方选择元素策略时，普通encoder/decoder方法显式接收元素codec并返回持有依赖的普通对象，见11.13.4；依赖不写入singleton的全局状态。
- class继承不继承companion。Base.Companion的`Encodable<Base>`或`Decodable<Base>`不会转移到Derived.Companion，也不会使数据实例获得接口。`Encodable<Base>`不能赋给`Encodable<Derived>`，解码方向同理；显式使用Base codec时，encode的数据实参仍允许普通Derived到Base的向上转换，表示按Base策略编码。运行时数据类型不改变所选codec；字段的自动选择也不向基类companion回退。
- 普通codec class自身继承的方法/default照常选择，参数和结果类型不会因数据类型继承自动改写。codec可以由companion、object或普通class/struct提供，可手写处理接口、基类或singleton目标；自动派生仍限于11.13的形状及普通访问规则。
- 不同interface application的成员按普通overload/override规则检查；尤其两个decode参数相同而结果不兼容时诊断冲突，不能仅按结果类型选择。两个方向可由同一codec或普通组合interface表达，不需要新增内建Codable标记或专用工厂requirement。

### 9.6 静态类型描述

每个合法类型都有可在编译期查询的结构描述。源码与依赖产物中的同一类型具有相同描述。

| 描述对象 | 内容 |
| --- | --- |
| 所有类型 | 类型类别、完整类型表达式或exact identity；名义类型的声明身份和注解 |
| struct | 按声明序排列的字段：独立字段身份、名称、类型、注解、对应构造参数及其已有default引用 |
| enum | 按声明序排列的variant及其注解；各variant独立的有序payload字段、名称/位置、类型、注解及构造关系 |
| class | 直接base关系、本owner的存储字段及logical property；保留声明顺序、访问域、存储/计算/委托类别和构造参数映射 |
| object / companion | 自身的base、存储字段和logical property；companion保留完整宿主application并替换宿主binder，不把宿主实例字段当作自身字段 |
| interface | 父接口、logical property及其类型/注解；不伪造实例存储字段 |
| tuple / Unit | 有序元素，位置名从`_1`开始；Unit为空积；没有源码注解或名义字段声明 |
| 核心intrinsic类型 | 标明其实际表示类别；String、Array、指针等不能因没有普通源码字段就被当作空record |
| 函数、指针、类型参数 | 类型签名、pointee或binder/bound；不伪造字段，也不推断可序列化能力 |

泛型定义的字段类型可以引用其binder；具体化后得到完整字段类型，透明alias展开后与目标共享类型描述。递归引用以typed type reference表示，不递归复制无限树。描述的名称仅用于显示或生成字段名常量，不作为实体identity；enum字段名的作用域是所在variant。class的基类字段经base关系取得，不扁平化成当前owner新字段，编译器生成的delegate slot等也不冒充源码property。

静态描述供 11.13 的方法合成使用，并随必要的 `.slib` 类型接口和模板保存。它不提供源码可执行的 `TypeInfo` 值、任意编译期循环、CTFE 或通用按字段构造原语。普通源码访问仍受 9.1.5 约束；没有运行期字段表、annotation 对象或按名称查类型、字段、构造器的入口。

---

## 10. 数组

Scoop 内置两个数组类型（引用类型，属于核心库）：

- `class Array<T>`：不可变数组（长度固定，元素不可写）；
- `class MutableArray<T>`：可变数组（长度固定，元素可写）。

Array 与 MutableArray 是 core 声明的 invariant generic class，类型实参、约束、成员与单态化遵守 3.2；其内存表示由 intrinsic 规定。

数组字面量、`vararg`、默认值和泛型正文在跨 Cone 时使用相同规则。隐式数组类型与显式 `Array<T>` / `MutableArray<T>` 引用同一实际声明；实参形式与求值顺序见 8.5。

### 10.1 值类型元素的内存保证

当 `T` 是值类型（struct / enum / tuple / 基本类型）时，`Array<T>` 与 `MutableArray<T>` 保证：

- 元素**不装箱**；
- 元素在内存中**连续布局**（在满足 pack/align 约束的前提下）。

当 `T` 是引用类型时，数组存储引用。

当`T`是ZST时，`Array<T>`/`MutableArray<T>`使用专门的zero-sized element storage：对象仍保存普通ref identity与精确`size: Long`，元素区起点仍按`alignOf<T>()`对齐，但任意长度都不分配element payload bytes。所有literal/assembly/spread输入仍按源码顺序求值并计算逻辑元素数。`get`先按普通调用规则求值receiver与index，再检查`0 <= index < size`并返回该exact ZST值；`set`先按普通调用规则依次求值receiver、index与RHS，随后执行bounds check，成功时不写物理字节。因而即使index越界，RHS的副作用或异常也不能因ZST被跳过。不同index表示不同逻辑元素，但不承诺不同物理地址；现有`addressOf`不能用于array元素。

ZST 数组仍按逻辑索引迭代。复制和互转产生具有相同 size 的新数组对象，独立引用身份不因 payload 为零而消失。逻辑 size 必须处于 `0..=INT64_MAX`，长度、assembly 求和及索引不得溢出。按长度构造仍须逐索引调用 initializer，保留其副作用与异常（10.6）。

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
  - 若任意元素是值类型，则其余能正常产生值的元素类型必须与之**完全相同**，否则是编译错误（值类型元素之间不做隐式向上合流，以保证 10.1 的内存布局保证成立）。`Nothing` 元素不限制其他元素的推断，不为缺少上下文的泛型表达式提供默认类型；所有元素均为 `Nothing` 时元素类型为 `Nothing`。这些元素仍按源码顺序求值，遇到无正常结果的元素后不构造数组或求值后续元素。
  - 若所有元素都是引用类型，则元素类型取全部元素类型的**最小上界（LOB）**，数组类型为 `Array<LOB>`；当前类型系统没有交叉类型，若存在多个互不可比较的最小共同上界，则 LOB 取 `Any`。当 LOB 为 `Any` 时编译器应给出警告。
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

### 10.4 不变性与转换

- `Array<T>`与`MutableArray<T>`遵守3.2的统一nominal不变性：即使`T` is-a `S`，`Array<T>`与`Array<S>`、`MutableArray<T>`与`MutableArray<S>`之间也没有subtyping。这保证每个exact application始终拥有确定的element layout，并使只读/只写API通过generic callable的bound表达，而不是通过容器projection表达。
- `Array<T>` 与 `MutableArray<T>` 之间**没有父子类型关系**，互转必须显式进行：
  - `m.toArray(): Array<T>`、`a.toMutableArray(): MutableArray<T>`；
  - 或以对方为参数的构造函数：`Array(m)`、`MutableArray(a)`。
- 转换构造的唯一必需参数名为 `source`，类型分别为 `MutableArray<T>` 和 `Array<T>`。显式类型实参、`_`、期望结果类型及固定 application 的 typealias 按普通构造规则确定 `T`；转换不改变元素类型，也不接受相同数组种类作为来源。普通名称查找取得实际数组声明后，转换候选与同层普通函数一起进行 8.6 的重载选择；导入别名及跨 Cone 使用保持同一规则。
- 互转总是生成新对象并按值或引用浅复制逻辑元素，后续修改来源或结果互不影响。ZST 不复制 payload 字节，但仍保留 size 和独立对象身份。

### 10.5 操作

- 下标访问 `a[i]`通过普通成员`operator fun get(index: Long): T`；`MutableArray`通过`operator fun set(index: Long, value: T): Unit`支持下标赋值`m[i] = v`。这些声明可以由intrinsic提供表示级实现，但候选选择、泛型实例化与operator identity遵守9.3，不建立按`Array`类型名放行的第二套解析规则。
- `vararg T`的spread和普通形参`Array<T>`都要求exact `Array<T>`；需要改变element type时，调用方显式逐元素构造/转换目标array。
- `size: Long` 属性；通过普通 `List<T>` conformance 继承 `Iterable<T>`，可用于 `for` 循环。`Array<T>` 和 `MutableArray<T>` 都只实现 `List<T>`，不实现要求可增删元素的 `MutableList<T>`（11.10）。`MutableArray` 自身仍提供 `set`；定长数组不提供以运行期失败代替实现的 `add` / `removeAt`。

### 10.6 按长度初始化

按长度构造返回固定长度的新数组：

```text
Array<T>(size: Long, init: (Long) -> T)
MutableArray<T>(size: Long, init: (Long) -> T)
```

- `size`、`init` 是公开参数名；显式实参、named argument、尾随 lambda、泛型推导及 alias 沿普通构造与 8.5.3 的求值规则处理，与 10.4 的 `source` 转换候选共同参与重载选择。
- 先各求值一次全部实参，再检查 `size >= 0`。负数抛 `IllegalArgumentException`，不调用 initializer。长度为零仍求值 `init` 表达式，但不调用它。
- initializer 是 ordinary 函数值，依次以 `0L` 到 `size - 1L` 调用，每个索引恰好一次；支持分配与抛异常，不支持在 initializer 内挂起。返回值必须可赋给 exact `T`，使用普通函数返回规则。
- 只有全部元素成功初始化后，完整数组才能成为构造结果；initializer 不接收正在构造的数组。第 i 次调用抛出时传播该异常，不调用后续索引，不返回半初始化数组，也不回滚已经发生的外部副作用。
- 每个可被普通代码访问的元素都必须是合法的 `T`。未填充的内部存储不是可读的 `T`，不能把清零当作任意 `T` 的默认构造。初始化期间的 GC 规则见 运行时规范 2.4。
- 对象大小的乘加、对齐与目标地址范围均沿现有 checked allocation 规则；溢出、对象过大或资源耗尽沿现有 fatal allocation failure，不发生整数 wrapping。
- 此 API 不引入数组原地 resize、公开未初始化数组或原始元素指针；`ArrayList` 通过替换自己持有的数组实现增长。

---

## 11. 最小核心库

核心库只包含核心语法运行所必需的类型与函数。命名空间为 **`scoop.core`**（默认导入 `scoop.core.*`，以及 `scoop.core.Option.*`，见 7.2）。

**核心库中的类除单独标明外均为 `final`**，不可继承（包括 `Array` / `MutableArray` / `ArrayList` / `String` / `StringBuilder` 等）。

### 11.1 类型层级根

core 源码正式声明以下两个类型；声明与 13.1 的 intrinsic 表示共同构成类型定义：

```scoop
@Intrinsic("core_any")
public abstract class Any {}

@Intrinsic("core_nothing")
public final class Nothing {}
```

`Any` 是所有类型的根，`Nothing` 是所有类型的子类型且无实例（3.1）。二者均没有成员、字段、构造器、父类型、companion 或 release block；不能把无字段声明视为可分配的空 class。`Any` 的 abstract 和 `Nothing` 的 final 是登记形状的一部分，顶／底关系不由普通 class 继承产生。`Any` 不提供 `equals`、`toString`、`hash` 或固定 vtable 槽（11.11）。

两个声明必须各存在一次，具有普通的源码位置、public binding 和 nominal identity，并随 core 产物发布。缺失或错误的声明在 core 编译／依赖消费边界报错，编译器不能在缺失时补建类型。普通同名声明仍按名称查找处理；仅有拼写 `Any`／`Nothing` 不赋予 intrinsic 语义。core 的源码位置或载入路径不是额外来源资格。

### 11.2 基本类型

均为值类型（struct 语义）：

- `Boolean`；
- 八种整数表示：8/16/32/64位二进制补码signed/unsigned标量。八个canonical源码声明分别是`Int8`/`Int16`/`Int`/`Long`与`UInt8`/`UInt16`/`UInt`/`ULong`；它们是八个不同的nominal type；
- 固定宽度与Kotlin风格名称通过3.2.1的透明alias对应：`Byte ≡ Int8`、`Short ≡ Int16`、`Int32 ≡ Int`、`Int64 ≡ Long`，以及`UByte ≡ UInt8`、`UShort ≡ UInt16`、`UInt32 ≡ UInt`、`UInt64 ≡ ULong`。`Int`/`UInt`在所有target上永久固定32位，`Long`/`ULong`永久固定64位；等价拼写不产生overload、RTTI、layout、mangling或ABI差异；
- 两种浮点表示：`Float`（IEEE 754 binary32）与 `Double`（IEEE 754 binary64）是不同的 canonical nominal type；`Float32 ≡ Float`、`Float64 ≡ Double` 是透明 alias，完整语义见 11.2.2；
- `Char`。

core中的对应声明是`public typealias Byte = Int8`、`public typealias Short = Int16`、`public typealias Int32 = Int`、`public typealias Int64 = Long`、`public typealias UByte = UInt8`、`public typealias UShort = UInt16`、`public typealias UInt32 = UInt`与`public typealias UInt64 = ULong`。当前语言不定义platform-native integer；本版本中要求保留64位数值范围的已有source/core API显式使用`Long`/`ULong`，这不把二者定义为target-native type。

整数literal接受十进制、`0b`/`0B`二进制、`0x`/`0X`十六进制及位于两个有效数字之间的`_`，不接受八进制。后缀规则为：无后缀候选域是可精确表示magnitude的signed整数，无其他约束时按`Int` → `Long`选择第一个可表示值的默认类型；`u/U`候选域是unsigned整数，无其他约束时按`UInt` → `ULong`选择；`l/L`精确固定为`Long`；`uL/UL`及大小写组合精确固定为`ULong`。radix前缀后必须有数字，separator不能位于首尾或紧邻前缀/后缀。各进制literal先表示非负数学magnitude，不按bit pattern自动重解释；超过数学值`2^64 - 1`的magnitude非法。

literal在8.6 winner commit前持有candidate-local可表示type集合。exact expected同符号族integer type可在值可表示时直接提交；该能力也适用于call/operator receiver，不建立整数type间的一般subtyping或conversion。无后缀不能适配unsigned，带`u`不能适配signed；`L`/`UL`已分别具有exact `Long`/`ULong`类型，不参与其他integer expected-type fit。一元`-`直接作用于无`u`literal时，二者作为完整负数学值检查边界，使每种signed `MIN`可表示；括号不形成语义节点，空白/注释不改变该关系。`-1u`则是普通unsigned `unaryMinus`。默认阶梯产生的exact commit在其他8.6规则后优于同族非默认fit；多个非默认fit仍歧义。

core整数的二元算术、逐bit运算和比较要求两个已定型operand为同一canonical type；literal可按上段直接提交。shift例外地要求左operand/result保持该integer type、count为canonical `Long`。不同width或signedness的非literal不隐式提升，必须显式转换。对宽度W：

- `+`、`-`、`*`、一元`-`、`inc`/`dec`及bit操作按`2^W`wrapping；一元`+`是同类型identity。signed结果按W位二进制补码解释，overflow不抛异常；
- `/`向零截断，`%`余数与被除数同号；除数为0抛`ArithmeticException`。signed `MIN / -1 == MIN`且`MIN % -1 == 0`；
- `and`/`or`/`xor`/`inv`逐bit工作。`shl`/`shr`/`ushr`的count为`Long`，有效count取其低`log2(W)`位；signed `shr`为算术右移，signed `ushr`和unsigned右移为逻辑右移；
- signed/unsigned比较分别使用数学有符号/无符号次序；`compareTo`统一返回canonical `Long`的`-1L/0L/1L`；
- const evaluator与运行期使用完全相同的width、wrapping、division和shift语义；const除零是定义错误，普通表达式仍按运行期异常执行。

每种integer提供到八种表示的显式`toInt8`/`toInt16`/`toInt32`/`toInt64`与`toUInt8`/`toUInt16`/`toUInt32`/`toUInt64`，并可提供alias拼写的转发名称。整数之间的转换total且不抛异常：数学源值先模`2^targetWidth`，再按目标signedness解释bit pattern。`toFloat`/`toDouble`遵守11.2.2，不改变上述整数转换规则。每个基本类型显式实现 `Equality<该类型>` 与 `ToString`；`Boolean`、`Char`和八种integer另显式实现`Hash`，`Float`/`Double`不实现`Hash`（11.11）。这些实现按owner的完整位宽工作，不经过装箱或`Any`分派。

八个canonical integer struct及`Byte`/`Short`/`Int32`/`Int64`/`UByte`/`UShort`/`UInt32`/`UInt64` alias都在core显式声明`public`，用户可调用成员同样显式`public`。`and`/`or`/`xor`/`shl`/`shr`是普通`infix` member，`inv()`是普通零参数member；signed类型另提供infix `ushr`，unsigned的`shr`已经是逻辑右移且不另设`ushr`。这些bit名称不带`operator` modifier，不增加9.3.1的operator约定。

每个integer kind的representation intrinsic surface固定包括`unaryPlus`/`unaryMinus`/`inc`/`dec`、`plus`/`minus`/`times`/`div`/`rem`/`compareTo`/`equals`、上述bit members及到八个integer kind的转换；另增加11.2.2的`toFloat`/`toDouble`。unsigned同样提供wrapping `unaryMinus`，所以`-1u`有定义。除显式转换的目标类型、`compareTo: Long`、`equals: Boolean`和shift count `Long`外，operand/result均为owner exact type。range members是普通core body，不属于该intrinsic集合。

除`div`/`rem`外，上述integer representation intrinsic均不分配、不抛异常，源码声明必须显式带`@NoGC`并登记为NoGc call target；`div`/`rem`因除零可能构造`ArithmeticException`，不得带`@NoGC`且登记为Managed。普通`toString`仍可分配并经普通typed Scoop-ABI core helper工作，不属于integer intrinsic operation集合。

#### 11.2.1 `Char`

`Char` 是具有独立 nominal identity 的 intrinsic struct，表示一个 Unicode 标量值：U+0000..U+D7FF 或 U+E000..U+10FFFF。它不是 UTF-8 byte、UTF-16 code unit 或用户感知的 grapheme cluster。内存表示为 32 位无符号 scalar，GC-free；没有普通字段或公开 primary constructor，也不与 Int/UInt 隐式互转。

- 单引号字符字面量在转义后必须恰好含一个标量值，例如 `'A'`、`'雪'`、`'😀'`、`'\u{1F600}'`；空、多标量、未闭合或含物理换行的字符字面量非法。转义规则与第 6 章一致，surrogate pair 不作为两次转义合成为一个字符。
- `Char.code: Int` 返回标量编号；普通 core extension `Int.toChar(): Char` 检查该编号属于上述域，否则抛 `IllegalArgumentException`。其他整数先显式转换为 Int；该转换仍遵守已有整数截断规则。
- Char 显式实现 `Equality<Char>`，提供同类型 `equals`、`compareTo: Long`，以及 `ToString` 和 `Hash`；相等和次序按标量编号，`toString()` 编码为一个标量的 String，`hash()` 返回编号对应的 Long。没有隐式数字算术或 CharRange。
- Char 字面量和同类型 const 引用可以用于 `const val`、默认值与递归 literal pattern；Char 普通方法调用不因此扩大既有 const-call 集合。模式覆盖遵守第 5 章的标量值有限域。
- C ABI 使用 `uint32_t` 承载 Char，外部实现仍须保证入站值是合法标量；其余 by-value、array、aggregate、Option、boxing 与 FFI 布局沿各自既有规则。Char 的源码身份不能以同宽 UInt 代替。

#### 11.2.2 `Float` / `Double`

`Float` / `Double` 是没有普通字段或公开 primary constructor 的 intrinsic struct，分别承载 IEEE 754 binary32 / binary64 的全部位型，包括 subnormal、正负零、Infinity 和 NaN。core 显式声明 `public typealias Float32 = Float` 与 `public typealias Float64 = Double`；alias 不产生新的类型身份、overload、companion、布局或 ABI。两种类型分别显式实现 `Equality<Float>` / `Equality<Double>` 及 `ToString`，不实现 `Hash`，不支持把标量按空 struct 解构。

**字面量与定型。** 十进制浮点字面量具有小数部分、指数部分或 `f/F` 后缀中的至少一项，例如 `1.0`、`.5`、`1e3`、`1f`、`1.5e-2F`。小数点后必须有数字；`1.` 不是浮点字面量，`1..2` 和 `1.toDouble()` 保留原有词法。指数 `e/E` 后可带 `+/-`，随后必须有十进制数字；`_` 只可位于同一数字段的两个数字之间。无十六进制/二进制浮点语法或 `d/D` 后缀；已有整数 `0x1f` 仍是十六进制整数。

- `f/F` 精确固定为 Float。无后缀浮点字面量参与 8.6 的 candidate-local expected-type fit；exact Float/Double 上下文按目标格式直接舍入，无其他约束时默认 Double。普通 MSC 规则后仍并列时，默认 Double commit 优先；失败候选不得泄漏类型或已舍入值。call/operator receiver 同样适用，只探测实际 core 浮点 owner 的相应成员，不枚举任意用户类型。
- 舍入使用 round-to-nearest, ties-to-even；不得先读成宿主 f64 再缩为 f32。目标舍入为 Infinity 的有限字面量是编译错误；下溢可成为 subnormal 或零，不因 inexact 舍入而拒绝。负号是独立一元运算，`-0.0`、`-0f` 保留负零。
- 已定型数值之间没有隐式提升；Float 与 Double、浮点与整数不能直接混合算术或比较。整数 token 不因 expected Float/Double 自动变成浮点值；使用 `1f`、`1.0` 或显式转换。透明 alias 仍是同一类型。

**算术与转换。** 同类型 `+`、`-`、`*`、`/`、`%` 和一元 `+/-`、`inc/dec` 使用目标精度；`inc/dec` 分别按同精度加/减 1。普通算术按最近偶数舍入，支持渐进下溢；浮点除零、无效运算和溢出产生 IEEE 结果，不抛整数的 ArithmeticException。`%` 是以向零截断商定义的余数，不是 IEEE `remainder`。有限非零值除以正负零得到相应 Infinity，零除零、Infinity 减自身、对零取余产生 NaN。一元 `+` 保留位型，一元 `-` 翻转 sign bit，包括 NaN 与零。

默认浮点环境为最近偶数舍入、异常 trap 关闭、保留 subnormal；不提供源码可观察的异常标志或动态 rounding mode。编译器不得假设无 NaN/Infinity、消除有语义差异的负零、重结合运算，或把独立乘加自动融合为一次舍入。

八种整数与两种浮点均提供 `toFloat()` / `toDouble()`；Float/Double 另提供到八种整数的 `toInt8`/`toInt16`/`toInt32`/`toInt64` 与 `toUInt8`/`toUInt16`/`toUInt32`/`toUInt64`。这些转换不分配、不抛异常：

- 整数到浮点从源数学值直接按目标精度最近偶数舍入，不经另一浮点格式；
- Float 到 Double 对有限值精确，Double 到 Float 最近偶数舍入，允许产生 Infinity 或下溢；同类型转换保持位型，跨格式转换保留零的符号，但不承诺 NaN payload 的映射；
- 浮点到整数向零截断，结果越界时饱和到目标整数的最小/最大值，NaN 转为 0；负数转无符号的下界为 0。按目标实际位宽饱和，不能先转 Long/ULong 再截断成窄整数。

**普通比较。** `equals` 与 `==` 使用 IEEE 数值相等，`!=` 是该结果取反；`< <= > >=` 直接使用 IEEE 有序关系：

| 输入 | `==` | `!=` | `<` / `<=` / `>` / `>=` |
| --- | --- | --- | --- |
| 任一 operand 为 NaN，包括同一 NaN | false | true | 全部 false |
| `-0.0` 与 `+0.0` | true | false | `<` / `>` 为 false，`<=` / `>=` 为 true |
| 其他值，包括 Infinity | 数值相等 | 数值不等 | 数学次序，`-Infinity < 有限值 < +Infinity` |

因此 `!(a < b)` 不等价于 `a >= b`；所有输入上的普通关系不构成全序。Float/Double 不提供返回 Long 的 `compareTo`，关系运算不经过该协议。已合法的泛型调用、alias 和派生的 struct/tuple/enum 字段相等均使用同一浮点语义；不能因为值在相同存储中就跳过字段比较，也不新增 `Any.equals`。需要全序的算法应显式使用下面的方法。

**显式全序。** 两种类型分别提供同类型参数的方法 `public fun isTotallyOrdered(belowOrEqualTo: Float): Boolean` / `public fun isTotallyOrdered(belowOrEqualTo: Double): Boolean`，调用为 `a.isTotallyOrdered(belowOrEqualTo = b)`。返回 IEEE 754 `totalOrder(a, b)`，是非严格的“全序小于等于”。其顺序为负 NaN、负 Infinity、负有限数、负零、正零、正有限数、正 Infinity、正 NaN；NaN 内部再按 IEEE 规则区分 sign、quiet/signaling 与 payload。相同位型对自身返回 true；正负零和不同 NaN 位型不合并。

若算法接收严格的先后谓词，使用 `!b.isTotallyOrdered(belowOrEqualTo = a)`，不能直接传入非严格方法。该方法及 `isNaN()` / `isInfinite()` / `isFinite()` 均为 NoGC intrinsic。它们不改变普通运算符，也不要求公开 `toBits` / `fromBits` / byte sequence / `transmute` API。

**常量与表示。** 两种 companion 提供 `const val NaN`、`POSITIVE_INFINITY`、`NEGATIVE_INFINITY`、`MAX_VALUE`（最大正有限值）、`MIN_VALUE`（最小正 subnormal）与 `MIN_NORMAL`（最小正 normal）。NaN 常量固定为正 quiet NaN，Float 位型 `0x7fc00000`、Double 位型 `0x7ff8000000000000`。9.1.2 的浮点 const 运算使用同样的目标精度；产生 NaN 的算术 const 结果使用该类型的固定 quiet NaN，一元 sign 操作和同类型复制/转换仍保留其规定的位型。运行期算术不承诺 NaN 的 sign/payload 与 const 相同；普通存储、传参、返回和复制保持位型，显式 totalOrder 总是比较当时的表示。metadata 保存 kind 与 raw bits，保留负零和已确定的 NaN 表示。

所有上述表示 intrinsic（包括浮点 `div/rem`）均为 `@NoGC`、ordinary；`toString` 和 codec 使用普通 core body，不属于 const-call 集合。`toString()` 输出不依赖 locale：有限值采用能按原精度读回相同位型的最短有效十进制数字，非零值规范化十进制指数在 `[-3, 7)` 时用定点，否则用科学记数法；始终有小数点和至少一位小数，科学指数用小写 `e`、不写正号或多余前导零。零分别为 `0.0` / `-0.0`，非有限值为 `NaN` / `Infinity` / `-Infinity`。不承诺 NaN payload 的文本往返。

**组合与范围。** const、默认参数、annotation、literal pattern、generic/跨 Cone、boxing、数组、普通 aggregate 与 FFI 都保留实际浮点类型。literal pattern 按目标精度定型并用 IEEE `==` 匹配，正负零覆盖同一值；浮点域的穷尽检查始终要求覆盖剩余值的 wildcard，不能通过枚举字面量覆盖 NaN。NaN 可由 `isNaN()` guard 判断，guard 不提供穷尽证明。核心 companion codec 与 JSON 规则见 11.13，C ABI 见 13.8。不引入 FloatRange、隐式数值提升、数值比较接口层次、Hash conformance、通用数学包或 128 位数值类型。

### 11.3 `Unit`

0 元 tuple 的类型名与值构造器（见 4.3）。

### 11.4 `String`

String 是 immutable 引用类型，持有合法 UTF-8 编码的 Unicode 标量序列，允许 U+0000，不隐含结尾 NUL。它显式实现 `Equality<String>`；`+`、相等、比较与 hash 均按内容工作。

| API | 语义 |
| --- | --- |
| `length: Long` | Unicode 标量值数量 |
| `byteLength: Long` | UTF-8 字节数 |
| `operator get(index: Long): Char` | 第 index 个标量值 |
| `slice(start: Long, endExclusive: Long): String` | 按标量索引取半开区间 |
| `iterator(): Iterator<Char>` | 按标量顺序迭代，普通 `Iterable<Char>` conformance |
| `toCharArray(): MutableArray<Char>` | 独立、可写的字符数组快照 |
| `String.fromChars(chars: List<Char>): String` | 普通 companion 方法；将字符序列编码为独立 String |
| `toByteArray(): MutableArray<Byte>` | safe 的 UTF-8 字节快照，Byte 按原始八位 bit pattern 保存 |
| `String.fromUtf8Unchecked(bytes: List<Byte>): String` | 标注 `@Unsafe` 的 companion 方法；复制调用方保证合法的 UTF-8 字节序列 |
| `String.fromUtf8(bytes: Array<UInt8>): String` | 严格解码；非法 UTF-8 抛出 `CharacterCodingException` |
| `String.fromUtf8OrNone(bytes: Array<UInt8>): String?` | 严格解码；非法 UTF-8 返回 `None`，成功返回 `Some(String)` |
| `String.fromUtf8Lossy(bytes: Array<UInt8>): String` | 将每个非法 UTF-8 子序列替换为 U+FFFD，保留合法内容 |
| `String.fromUtf8(pointer: Ptr<UInt8>, length: Long): String` | `@Unsafe` 的严格解码重载；读取指定长度的 native 字节 |

`get` 要求 `0 <= index < length`，`slice` 要求 `0 <= start <= endExclusive <= length`，否则抛 `IndexOutOfBoundsException`。`slice(length, length)` 合法并返回空串。索引不接受隐式数值转换；所有上述计数与边界直接使用 Long。`"A雪😀".length == 3L`，其 byteLength 为 8L。组合字符分别计数，不执行 Unicode normalization，因此 `"e\u0301"` 与 `"\u00E9"` 不相等且长度不同。

byteLength 为 O(1)；length 与索引定位为 O(byteLength)，完整顺序迭代为 O(byteLength)。String 实现 `Iterable<Char>`，不实现 `List<Char>`。

从 List 构造时按 11.10.3 取得完整元素快照，用户 getter 的异常照常传播。字符和字节数组转换均复制存储，后续修改数组不会修改 String。`fromUtf8Unchecked` 要求本次读取的完整序列为严格合法 UTF-8；违反此前置条件属于 unsafe 契约违例，不承诺可捕获异常。`toByteArray` 不要求 unsafe。

受检转换不要求调用者预先保证 UTF-8 合法。严格解码拒绝非法起始字节、孤立的 continuation 字节、截断序列、过长编码、代理项码点和超出 U+10FFFF 的码点；`CharacterCodingException.byteOffset: Long` 是首个非法子序列起始字节的零基偏移。`fromUtf8OrNone` 只把编码错误转为 `None`，不吞掉其他异常或分配失败。三种转换均保留 U+0000 和输入中原有的 U+FFFD，不执行 Unicode normalization；空输入返回空串。

`fromUtf8Lossy` 使用 Unicode 的 maximal subpart 替换规则，每个 maximal subpart 输出恰好一个 U+FFFD：从当前位置开始，若不能解出合法标量，消费仍可能成为某个合法 UTF-8 序列前缀的最长非空字节段；若首字节本身不能作为合法前缀，则只消费该字节。截断的合法前缀整体替换，不吞掉使当前序列失配的后续字节，该字节由下一轮继续解码。不得把任意一整段连续非法字节合并为一个替换字符，也不得逐字节替换一个仍合法的截断前缀。

| 输入字节（十六进制） | lossy 结果 | 说明 |
| --- | --- | --- |
| `E1 80 41` | `"�A"` | `E1 80` 为一个 maximal subpart，`41` 继续解码 |
| `F0 90 80` | 一个 U+FFFD | 截断的合法四字节序列前缀 |
| `80 80` | 两个 U+FFFD | 两个孤立 continuation 字节 |
| `C0 AF` | 两个 U+FFFD | 过长编码的两个字节都不能作为合法前缀 |
| `ED A0 80` | 三个 U+FFFD | 代理项编码不构成合法 Unicode 标量 |

pointer 重载要求 `length >= 0`；负长度抛出 `IllegalArgumentException`。非空区间必须在整个调用期间可读、内容稳定且未释放，managed 内存须按第 14 章保活并固定；零长度不解引用 pointer，但裸 `Ptr` 仍须满足自身的非零规则。受检和 lossy 只规定字节内容的处理，不修复无效地址、数据竞争或生命周期违例；结果大小与分配失败遵守现有 String/runtime 边界。转换返回的 String 不借用输入存储。

实现 `Equality<String>` 的成员 `public override operator fun equals(other: String): Boolean` 和 Hash 基于 UTF-8 内容；compareTo 按标量字典序（合法 UTF-8 的字节字典序给出相同结果）；`toString()` 返回自身。这些能力不来自 Any 或 TypeDescriptor 缺省槽。结果内容与源容器独立，但不要求空串、完整 slice 或其他相同不可变 String 具有不同引用身份。

### 11.5 `Option<T>`

```
enum Option<T> {
    Some(T),
    None
}
```

见第7章。`Option`与全部nominal generic一样保持invariant；niche表示只属于每个exact `Option<T>`。`scoop.core.Option.*`默认引入。

### 11.6 `StringBuilder`

字符串插值（第 6 章）的脱糖目标，提供以下公开接口：

```
public class StringBuilder {
    public fun add(part: String): StringBuilder
    public fun <T : ToString> add(part: T): StringBuilder
    public fun build(): String
}
```

公开零参数构造创建空 builder，也可由用户代码直接使用。两种 add 都追加到当前末尾并返回同一 builder；generic add 在本次调用中恰好执行一次 `part.toString()`，完成后才追加结果，不保存原始对象或延迟转换。转换失败不追加该 part，已发生的用户副作用不回滚；转换期间对同一 builder 的重入修改按普通调用顺序生效。

build 返回当前所有 parts 按顺序连接的 String 内容快照，空 builder 返回空串；它不清空、关闭或冻结 builder。重复 build 的内容相同，后续 add 不影响既有结果，不承诺结果引用身份不同。parts 不以固定槽数限制追加次数；增长、总 UTF-8 字节数与最终分配大小只受 10.6 的真实表示/分配边界限制。

`build` 的时间复杂度为 O(partCount + totalByteLength)。builder 不要求显式 close，不提供 off-heap 存储或 release hook 接口。

### 11.7 异常

- `Throwable`（引用类型，可被 `throw` / `catch`）及其最小子类：
  - `Exception(message: String?)`：通用异常基类；
  - `UnwrapException`：`!!` 失败时抛出（见 7.3）；
  - `ClassCastException`：`as` 失败时抛出；
  - `ArithmeticException`：整数除零等算术错误；
  - `CharacterCodingException(public val byteOffset: Long)`：严格 UTF-8 解码失败，保存首个非法子序列的零基字节偏移（11.4）；
  - `MissingContextException(message: String?)`：final 异常，contextual declaration 入口缺少精确类型 binding 时抛出（8.3）；
  - `IndexOutOfBoundsException`：数组下标越界（见 10.5）；
  - `IllegalArgumentException(message: String? = Some("illegal argument"))`：实参值违反普通core API的运行期前置条件；11.8的非正range step使用该异常；
  - `IllegalStateException(message: String? = Some("illegal state"))`：运行期状态协议被破坏；默认参数保持既有零实参调用，initialization cycle使用显式message报告稳定unit path。
- `try` / `catch` / `finally` / `throw` 语法与 Kotlin 一致。多个 `catch` 按声明顺序匹配；前一个 `catch` 的类型是后一个的父类型（含相等）时，后者不可达，是编译错误。
- `throw` 与 `catch` 的类型必须是 `Throwable` 的子类型。未捕获的异常导致进程终止：先输出异常诊断并刷新输出，再以退出码 `1` 结束；eager 初始化失败与异常逃离 `main` 使用同一规则，不调用用户 `toString` 生成诊断。程序因语言级 panic 终止时也使用退出码 `1`。入口的正常返回规则见 12.4.4；runtime 内部 ABI 错误的终止规则见运行时规范第 7 章。

跨 Cone 的 throw、catch 与类型转换使用实际 `Throwable`、`ClassCastException` 声明，同名普通 class 不能替代。catch 绑定是可返回、存储和捕获的普通 managed 异常对象，其生命周期不受 handler 限制；异常构造、默认参数、catch 顺序与 finally 均遵守本规范。引用类型转换成功时保留对象身份，强制 `as` 失败时抛出 `ClassCastException`。

- generic class可以继承`Throwable`；其每个exact application都是不同异常类型并拥有不同TypeDescriptor。`catch (e: Error<Int>)`只接收该exact application及普通派生class，`catch (e: Throwable)`仍可接收全部application；不存在`Error<*>`式通配catch。

### 11.8 迭代与区间

当前 Cone 与依赖 core 产物中的迭代遵守相同的 Iterator、next 和 Option 协议；用户同名声明不能替代这些协议实体。

`for` 循环、区间表达式的最小支撑：

```
public interface Iterator<T> {
    public fun next(): Option<T>
}

public interface Iterable<T> {
    public operator fun iterator(): Iterator<T>
}
```

`next()`直接返回`Option<T>`：有元素返回`Some(v)`，耗尽返回`None`。`Iterator<T>`与`Iterable<T>`都是invariant exact application；所有public modifier显式写出，sysroot不绕过9.1.5。

对`for (pattern in source)`：

1. `source`先求值且只求值一次；
2. 按9.3普通operator规则选择唯一零参数`iterator()`并调用一次；其返回协议不参与挑选另一个低优先级operator；
3. winner返回type的完整base/interface/bound闭包在去重相同diamond路径后必须恰好到达一个exact `Iterator<T>` application；零个或多个不同application均为编译错误；
4. iterator值按该conformance物化一次，每轮经core `Iterator<T>.next` exact interface target调用一次；用户同名`next`、`Some`或`None`不参与；
5. `Some(value)`建立该轮新的只读scope，以4.6的irrefutable binding pattern绑定后执行body；`None`正常退出。普通body末尾与`continue`都进入下一次`next()`，`break`退出循环。

概念展开为（名称仅作示意）：

```
val __it: Iterator<T> = source.iterator()
while (true) {
    when (__it.next()) {
        Some(__value) -> {
            val pattern = __value
            /* body */
        }
        None -> break
    }
}
```

展开实际使用hygienic temporary、typed loop/interface/variant identity，不进行源码名称查找。`iterator()`若为suspend，只能在允许挂起的上下文选择，可在首轮前挂起但仍只调用一次；core `next()`固定ordinary。每轮binding都是新值，closure捕获对应轮次，不共享一个反复覆写的隐藏`var`。Array/MutableArray必须以普通public conformance实现`Iterable<T>`。

integer range有四个canonical nominal type：`public final class IntRange : Iterable<Int>`、`public final class LongRange : Iterable<Long>`、`public final class UIntRange : Iterable<UInt>`与`public final class ULongRange : Iterable<ULong>`。四者是不同的nominal type，`LongRange`/`ULongRange`不是 alias。constructor与表示属性为core-internal，外部代码不能直接构造不满足step/方向不变量的实例；类型、分配、构造与成员本身仍遵守普通class规则，不是intrinsic或runtime opaque type。不提供 `CharRange`。

`Int8`/`Int16`/`Int`的四个range member返回`IntRange`，`Long`的四个成员返回`LongRange`；`UInt8`/`UInt16`/`UInt`返回`UIntRange`，`ULong`返回`ULongRange`。对这八个canonical integer owner分别令`O`为owner自己的exact type、`R`为上述结果type；每个owner必须按以下schema逐一声明四个普通、非generic、非suspend member，参数名`endpoint`属于可被named argument观察的public API：

```text
public operator fun rangeTo(endpoint: O): R
public operator fun rangeUntil(endpoint: O): R
public infix fun until(endpoint: O): R
public infix fun downTo(endpoint: O): R
```

每个range type `R` 都精确提供`public override operator fun iterator(): Iterator<E>`、`public infix fun step(value: E): R`与`public operator fun contains(value: E): Boolean`：`IntRange`的`E`/`R`为`Int`/`IntRange`，`LongRange`为`Long`/`LongRange`，`UIntRange`为`UInt`/`UIntRange`，`ULongRange`为`ULong`/`ULongRange`。它们都是member而非extension。range对象及其表示属性不可变；每次成功调用`rangeTo`、`rangeUntil`、`until`、`downTo`或`step`都构造fresh range对象，所以`step`结果与receiver不是同一引用。`step`结果保留first、方向与开/闭端点语义。每次`iterator()`都返回一个从first开始的全新cursor，不同cursor的进度彼此独立；cursor一旦耗尽，之后每次`next()`都返回`None`。Array/MutableArray同样以`public override operator fun iterator(): Iterator<T>`实现Iterable；internal array iterator的index固定为`Long`，`next`以显式`public override`满足slot，其effective domain仍受owner限制。

`Int8`/`Int16`的endpoint在已选core函数体内显式、无损扩展为`Int`，`UInt8`/`UInt16`同理扩展为`UInt`，所以窄range元素分别是`Int`/`UInt`。`Long`/`ULong`端点不截断且分别保留在`LongRange`/`ULongRange`中。这不建立一般隐式conversion，两个已定型且类型不同的endpoint仍不能混用。

- `a..b`从a以1升序并包含b，`a > b`为空；
- `a..<b`与`a until b`从a升序且不包含b，`a >= b`为空；
- `a downTo b`从a以1降序并包含b，`a < b`为空；
- `IntRange`/`LongRange`/`UIntRange`/`ULongRange` 的`step n`分别要求`n: Int > 0`、`n: Long > 0L`、`n: UInt > 0u`与`n: ULong > 0uL`，否则求值时抛`IllegalArgumentException`。它保留原方向、端点包含性与first，只替换步长绝对值；
- `contains`的value参数与该range的element type精确相同，只在值位于端点范围且与first的step对齐时为true；实现差值/对齐判断时不能让源码整数overflow改变结果；
- iterator先判断下一步是否越过endpoint或发生机器溢出，再更新current；不得依赖wrapping sentinel，因而MIN/MAX端点也必须正确终止。

range、iterator、`until`/`downTo`/`step`/`contains`均为普通public core声明，除9.3 operator展开和上述for协议外没有按type name识别的编译器旁路。`..`与rest pattern的消歧见4.6。

### 11.9 协程原语

支撑 `suspend` 语义的最小 core 形态如下：

```
interface Continuation<T> {
    fun resume(value: T)
    fun resumeWithException(exception: Throwable)
}

interface SuspendTask<T> {
    suspend fun run(): T
}

interface SuspendRegistration<T> {
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

- core 协程接口必须具有上述成员名称与签名；接口内的方法声明顺序不属于协议要求。编译器保留所选实际声明与派发槽，重新构建 core 后的消费者使用该产物保存的关系。
- `startCoroutine` 是最小协程构建器：启动 `task.run()` 后立即返回 `Unit`。若 task 在启动调用内完成，则返回前调用 `completion.resume(value)` 或 `completion.resumeWithException(exception)`；若 task 挂起，则在最终完成时调用。completion 恰好收到一次完成通知。
- `startCoroutine` 的两种入口均按 8.3.5 fork 当前有效 Context，task 与 completion 在 child 中执行，离开本次驱动后恢复 caller/resumer 的 Context。direct suspend 调用及 `suspendCoroutine` 的 registration 本身不 fork；只是投递同步完成 payload 的 resume 不取得另一份驱动权。
- `suspendCoroutine` 调用 `registration.register(continuation)`。registration 可以同步恢复 continuation，也可以保存它并在 `register` 返回后恢复；前者使 `suspendCoroutine` 在当前调用栈内继续，后者使其真正挂起。`register` 在尚未完成 continuation 时抛出的异常等价于 `suspendCoroutine` 在调用点抛出该异常。
- `register` 已同步完成 continuation 后又抛出属于状态协议错误，`suspendCoroutine` 以 `IllegalStateException` 失败；该 continuation 随即失效，之后不能再次成功完成。
- `SuspendTask` / `SuspendRegistration` 是不依赖 lambda 与函数引用的最小协议。函数类型 overload 由 core 中的普通 Scoop 适配器包装为这两个 interface 后调用同一底层原语，不另建 continuation 状态机；两种入口具有完全相同的同步完成、真实挂起、异常与单次完成语义。
- `launch`、`async`、dispatcher 等高层 API 可以在这些原语上由标准库提供，但不得改变 8.2 的单次完成与异常语义。
- core 原语不提供队列、线程切换或事件循环；调度器与取消不属于核心库。

### 11.10 数组与 List

`Array<T>` 与 `MutableArray<T>` 遵守第 10 章的固定长度与快照转换语义。List 与 ArrayList 是普通 core 类型，所有类型参数遵守 3.2 的不变性。

#### 11.10.1 `List<T>` 与 `MutableList<T>`

```scoop
public interface List<T> : Iterable<T> {
    public val size: Long
    public operator fun get(index: Long): T
}

public interface MutableList<T> : List<T> {
    public operator fun set(index: Long, value: T): Unit
    public fun add(value: T): Unit
    public fun add(index: Long, value: T): Unit
    public fun removeAt(index: Long): T
    public fun clear(): Unit
}
```

`List` 是只读访问接口，不承诺对象或元素不可变，也不产生快照。`ArrayList<T> <: MutableList<T> <: List<T> <: Iterable<T>`；以同一对象建立的 `List<T>` alias 会观察到其他 alias 的修改。`List<Derived>` 不是 `List<Base>` 的子类型，所有转换仍须满足 exact application 规则。数组字面量继续构造 `Array` 或 `MutableArray`，不会因这组接口改为构造 `ArrayList`。

`get` / `set` / `removeAt` 要求 `0 <= index < size`，按索引插入的 `add(index, value)` 要求 `0 <= index <= size`；失败抛 `IndexOutOfBoundsException`。普通 receiver 与实参先按源码顺序求值，因此越界操作也不能跳过 value 的副作用。追加保持顺序；插入后原后缀右移；`removeAt` 返回被移除的值并使原后缀左移；`clear` 使 size 变为零。`set` 不改变 size。所有操作都支持 `T` 本身是 `Option<U>`，`None` 是合法元素，不表示列表中没有该位置。

无 bound 的 List 接口不隐含任意 T 的相等、哈希或字符串化能力；不提供依赖这些能力的 `contains`、按值 `remove`、列表结构相等或 `ToString`。

#### 11.10.2 `ArrayList<T>`

`public final class ArrayList<T> : MutableList<T>` 是普通可实例化 generic class，具有公开构造形式 `ArrayList<T>(initialCapacity: Long = 0L)`。负 initialCapacity 抛 `IllegalArgumentException`；初始 size 总为零，预留容量不产生列表元素。所有接口成员显式以 public override 实现，size 只读。容量及 backing array 不属于公开表面。

移除或 clear 后，容器不继续保活已移除的引用。值类型元素内联存储，不因容器或 `List<T>` 分派而装箱；不保证容器内部的元素 stride 与 `sizeOf<T>()` 相同。

索引读写为 O(1)，追加摊还 O(1)，插入、删除及 clear 为 O(size)。逻辑 size 不超过 `INT64_MAX`，容量与实际分配遵守 10.6 的溢出和失败规则，不设其他固定元素数上限。操作不回滚用户实参的副作用，也不提供并发同步。

#### 11.10.3 快照与迭代

core 提供普通 generic extension `fun <T> List<T>.toArray(): Array<T>` 和 `fun <T> List<T>.toMutableArray(): MutableArray<T>`。调用时读取一次 size，按 `0L` 到 `size - 1L` 依次读取元素并创建独立数组；原列表和结果容器的后续修改互不影响，元素本身按值或引用浅复制。10.4 既有数组转换成员仍优先于 extension，保留其复制语义。自定义 List 的 getter 副作用、异常沿普通调用传播，不承诺并发快照。

Array、MutableArray 和 ArrayList 的 iterator 按索引逐次读取，每次 next 使用当时的 size，成功后进入下一索引；首次返回 None 后永久耗尽。多个 iterator 的进度独立。串行修改时，set 可被后续读取观察到；首次 None 前追加的元素可被遍历；插入和删除可能导致元素重复或跳过；clear 后下一次读取耗尽。iterator 不提供 fail-fast 异常或并发同步。

### 11.11 相等、字符串化与哈希约定

core 用普通接口表示值相等能力：

```scoop
public interface Equality<T> {
    public operator fun equals(other: T): Boolean
}

public fun <T : Equality<T>> same(left: T, right: T): Boolean = left == right
```

`Equality<T>` 的类型参数保持 invariant；它表示“可以与 T 比较”，没有隐式 Self，也不要求 T 反过来实现接口。`T : Equality<T>` 是同类型比较的普通 interface bound，可与独立的 `Hash` bound 一起约束集合 key。Equality 只提供比较操作，不保证所有值构成数学上的等价关系，不据此把 `x == x` 恒折叠为 true；Float/Double 的 NaN 规则继续适用。

- **`===` / `!==`（引用相等）**：identity 比较，仅适用于引用类型，不可重载（见 4.4.2）。
- **`==` / `!=`（值相等）** 的决议规则：
  - `lhs == rhs`先各求值一次，只从lhs静态类型的实际 `Equality<R>` conformances、接口继承或 type-parameter bounds 中收集对应的 `equals` 成员，按普通参数适配与成员overload规则选择唯一目标。`lhs != rhs`调用同一目标后对结果取反。没有可用 Equality 契约或决议歧义时是编译错误；不交换左右操作数，不使用 extension、地址比较、`Any.equals`或TypeDescriptor fallback。
  - **值类型：条件派生的结构相等**——对普通 struct、enum 和 tuple，在没有手写同签名成员且全部字段（元素）**可比较**时，编译器同时提供 `Equality<完整宿主类型>` conformance 和对应的 `equals` 实现。例如 `Point` 获得 `Equality<Point>`，`Box<Int>` 获得 `Equality<Box<Int>>`。字段可比较表示两个该字段静态类型的值执行`==`能从 Equality 契约选出唯一目标；不要求字段恰好实现 `Equality<字段类型>`，继承的其他适用 application 也按普通决议参与。struct/tuple逐字段按声明顺序短路；enum先比较tag，再只比较active variant payload；Unit无条件实现 `Equality<Unit>` 且结果恒为 true。任一字段不可比较时，派生接口与方法同时不存在，需要它们的使用点诊断首个失败字段/variant路径。
  - generic value type 的派生条件只来自实际字段／payload，不额外约束未存储的类型参数。`Box<T>(val value: T)` 可对任意合法 T 构造，只有字段比较条件成立的 application 才获得派生 Equality；开放字段保存其类型与 Equality 协议要求，在类型替换后按该派生规则确定实现。用户 generic body 中使用该能力仍必须由声明处已有 bound 保证，不能等具体化后另选未绑定的成员。派生条件随模板跨 Cone 保存；它是现有结构相等派生的组成部分，不把同形成员视为任意接口的实现。
  - 显式写入 implements 列表的 Equality 是普通无条件声明；若依赖派生正文满足它，字段条件必须在该类型的声明及 bounds 下成立，否则在定义处报错，不能把显式接口悄悄变成条件接口。未显式声明时按上述条件派生，不限制该类型原本合法的构造用途。
  - 用户手写相等时显式实现对应 `Equality<R>`，并声明合法的 `public override operator fun equals(other: R): Boolean`；该实现优先于同签名派生体，方法内容不再受字段可比较条件限制。其他参数类型的 Equality application 不屏蔽同类型的派生实现。手写同签名成员遵守普通 override 与冲突规则，不另生成第二个同签名方法；只有普通同名函数不赋予 Equality conformance。
  - 派生方法是该 Equality slot 的真实实现，可满足 bound、经普通接口调用或装箱后的 itable 调用，不是仅供 `==` 使用的隐藏候选。它遵守 value-type `this` 按值传递规则；tuple/Unit 的有效访问域由完整类型决定，tuple 保留全部元素类型的可见性约束，helper 所在文件或首次创建位置不增加源码访问限制。
  - Float/Double核心`equals`遵守11.2.2；含NaN字段的派生值可能不等于自身。派生相等必须保留字段语义，不能改用bitwise equality、`memcmp`或相同存储/identity的快捷返回；泛型具体化同样适用。
  - **引用类型**：class/object 通过普通声明显式实现或继承 Equality，不自动派生结构相等。只使用表达式静态类型可见的 Equality 契约；`Any == Any`非法，运行期对象另有实现不能补齐静态契约。继承 `Equality<Base>` 不自动产生 `Equality<Derived>`；两个 `Equality<Point>` 视图也不因此能彼此比较，其 equals 的参数仍是 Point。需要identity比较时显式使用`===`。
  - String、Boolean、Char、八种定宽整数、Float/Double 及 `Ptr<T>` 由 core 显式实现各自同类型的 Equality，alias 保持同一 conformance。intrinsic type 不按空字段结构派生；数组、函数、FunPtr 等没有既有相等实现的类型不因本接口获得比较能力。core Equality 的实际声明与 slot identity 决定本协议，用户同名接口不能替代它。
- **`ToString`（字符串化）**：接口`interface ToString { fun toString(): String }`。class/object/struct/enum都必须在声明中显式列出该interface并提供合法override；字段或payload实现`ToString`不会让宿主自动获得conformance。generic nominal type若在实现体中调用类型参数值的`toString()`，必须为相应参数声明普通`ToString`上界。tuple与Unit不能声明implements列表，因而不实现`ToString`。String与基础类型由core中的intrinsic nominal声明显式adopt，String实现返回自身。`print` / `println` 定义为`fun <T : ToString> print(v: T)`并经普通单态化bound call实现，不接受`Any` fallback，也不按成员同形或字段结构补齐conformance。
- **`Hash`（哈希）**：接口`interface Hash { fun hash(): Long }`。**没有任何缺省或派生实现**；Boolean、Char、八种定宽整数与String由核心库显式提供内容相关实现，其他类型显式opt-in。Float/Double及其alias不实现Hash，也不提供默认`hash()`；基本类型或值类型身份本身不产生conformance。相等的值必须产生相等hash；不以对象地址作为hash，也不承诺算法跨runtime版本保持相同数值。struct不自动获得哈希。此规则不新增Map key专用限制；有Hash bound的API仍按普通interface规则检查。

### 11.12 `SourceLocation` 与位置 intrinsic

```
struct SourceLocation(
    val file: String,
    val line: Long,
    val column: Long,
    val functionName: String,
    val typeName: String        // 不在任何类型内部时为空串
)

@Intrinsic("current_source_location")
fun getCurrentSourceLocation(): SourceLocation
```

- `getCurrentSourceLocation()` 返回所在表达式的标准**求值来源**所指示的源码位置（文件、行、列）与所处函数、类型的名称。该信息在编译期已知，不依赖调试信息；本intrinsic不建立独立的调用处传播机制。
- `file`保存可重现的canonical semantic source path，而不是host绝对路径或CLI operand spelling。manifest Cone中其形式为`group:name:version/<normalized Cone-relative source path>`；默认目录中的文件仍为`group:name:version/src/...`，12.2.2 的显式源码目录不截去其目录前缀。12.2的single-file Cone中恒为`scoop:single-file:0.0.0/main.scoop`。编译诊断可另行显示当次调用路径，但该display locator不改变`SourceLocation`的值。
- 可与 8.5 的缺省参数规则组合，取得调用者的源码位置：

```
fun trace(msg: String, loc: SourceLocation = getCurrentSourceLocation()) {
    // loc 是调用 trace 的位置，而非本函数内部
}
```

- 作为普通表达式出现在缺省参数template中时，它与template内其他表达式一样按8.5取得定义来源和求值来源；随后普通intrinsic语义读取求值来源，因而返回**最外层调用处**的位置，而不是default机制识别并重写本函数。多层函数转发时，每一层都必须以缺省参数继续转发`loc`（`fun warn(msg: String, loc: SourceLocation = getCurrentSourceLocation()) = trace(msg, loc)`），否则记录的是中间层的位置。
- 内联等优化（见 8.4）不得改变其结果：结果按源码中的调用处确定，与代码生成决策无关。

---

### 11.13 编码、解码与缺省实现

核心库在 `scoop.core` 提供以下协议和注解：

```scoop
public interface Encodable<T> {
    public fun encode(value: T, encoder: Encoder): Unit
}

public interface Decodable<T> {
    public fun decode(decoder: Decoder): T
}

public annotation class SerialName(val name: String)
public annotation class Transient
```

两者都是由companion或普通codec对象实现的普通实例interface，this始终是codec。请求默认实现的声明分别列出对应完整interface application；encode的数据参数和decode的结果由T确定。目标数据类型不会因codec存在获得接口，编译器不自动添加companion或向另一声明转移conformance。两个方向独立，也可由同一codec同时实现；不新增内建Codable标记，普通组合interface沿原规则使用。

#### 11.13.1 实现选择与合成条件

- 先按普通override/default规则选择合法的用户实现或继承实现；缺少实现时，才为当前实现类型上的核心requirement合成普通方法。两个方向独立决定，手写一个不影响另一个。错误的显式override仍然报错；无关的合法overload不占用requirement。
- 编译器只识别实际core协议声明的typed identity；用户同名interface或annotation没有特殊行为。用户subinterface可继承完整的`Encodable<T>`或`Decodable<T>`并增加其他requirement；编译器只补齐encode/decode，其他缺失方法仍是正常错误。
- 两个方向的被描述类型都是interface application中的T，不是codec的receiver类型。struct按源码字段处理，enum按variant/payload处理，tuple按元素处理。按11.13.4在定义处为每个参与字段分别确定`Encodable<F>`或`Decodable<F>`值，encode读取value的字段，不读取codec自身的依赖字段作为数据。缺失能力、歧义或不可构造形状报错，不生成运行期失败stub。
- 数据class的自动处理范围仍是普通final class且没有显式class基类。encode读取value所属目标owner的存储property；带自定义accessor或委托的property须显式处理。decode还要求目标有唯一primary constructor，所有参与状态来自其val/var参数；未参与的构造参数须有default，其他存储状态须明确Transient并能按既有规则初始化。字段读取和constructor调用都必须处于codec的正常访问域；合成不创建超出9.1.5的额外权限。
- computed/abstract property不作为存储字段参与派生。无参与字段的普通struct/class编码为record，不能把intrinsic类型当成空record。开放/抽象class、带class基类、singleton或其他不满足上述映射的目标使用手写codec；object可以实现任一方向，但自动构造不会尝试创建新的singleton。将编码移入codec不扩大class自动派生范围。
- 泛型目标如`Box<E>`可包含普通参数E，两个方向分别由显式`Encodable<E>`、`Decodable<E>`依赖提供字段能力，不要求E自身实现接口。companion可使用宿主参数并按9.1.3具体化，但不能等E具体化后再发现其codec。裸T没有可展开目标形状，不能仅凭声明`Encodable<T>`或`Decodable<T>`自动得到body。
- 合法递归类型先建立合成方法签名，再生成body；值布局和单态化终止性继续遵守3.2。已有接口实现选择在定义处完成，具体化不重新选重载、default或字段codec依赖。

核心Boolean、现有定宽整数、String、Char的companion分别实现具体的`Encodable<Scalar>`与`Decodable<Scalar>`；Unit使用普通object UnitEncoder与UnitDecoder。`Option<T>`、`Array<T>`、`MutableArray<T>`、`ArrayList<T>`及tuple的codec按11.13.4组合。标量、Unit、容器和tuple数据值不因此获得编码/解码接口；不提供条件conformance，容器的无bound用途不受影响。

List/MutableList没有唯一默认解码结果。核心库提供普通
`fun <T> encodeList(values: List<T>, element: Encodable<T>, encoder: Encoder): Unit`：
取得一个unkeyed容器，按values的普通迭代次序逐项取得child并调用
`element.encode(value, child)`，最后结束该容器。空列表同样结束容器；元素异常沿普通调用传播。该函数
只遍历逻辑元素，不编码容器的 capacity、backing 或空闲槽。

Any、函数、Ptr/FunPtr没有默认codec。用户可手写`Encodable<List<T>>`、
`Decodable<List<T>>`、`Encodable<Any>`或`Decodable<Any>`等实现；不从运行期类型名推断具体codec。

#### 11.13.2 容器协议与库边界

Encoder/Decoder提供三个入口，均为ordinary、可抛异常的非泛型interface方法：

```scoop
public interface Encoder {
    public val path: String
    public fun keyed(): KeyedEncodingContainer
    public fun unkeyed(): UnkeyedEncodingContainer
    public fun singleValue(): SingleValueEncodingContainer
}

public interface Decoder {
    public val path: String
    public fun keyed(): KeyedDecodingContainer
    public fun unkeyed(): UnkeyedDecodingContainer
    public fun singleValue(): SingleValueDecodingContainer
}

public interface KeyedEncodingContainer {
    public fun field(name: String): Encoder
    public fun end(): Unit
}

public interface KeyedDecodingContainer {
    public val keys: List<String>
    public fun required(name: String): Decoder
    public fun optional(name: String): Option<Decoder>
    public fun end(): Unit
}

public interface UnkeyedEncodingContainer {
    public fun element(): Encoder
    public fun end(): Unit
}

public interface UnkeyedDecodingContainer {
    public val hasNext: Boolean
    public fun element(): Decoder
    public fun end(): Unit
}

public interface SingleValueEncodingContainer {
    public fun writeBoolean(value: Boolean): Unit
    public fun writeLong(value: Long): Unit
    public fun writeULong(value: ULong): Unit
    public fun writeFloat(value: Float): Unit
    public fun writeDouble(value: Double): Unit
    public fun writeString(value: String): Unit
    public fun writeNull(): Unit
}

public interface SingleValueDecodingContainer {
    public fun readBoolean(): Boolean
    public fun readLong(): Long
    public fun readULong(): ULong
    public fun readFloat(): Float
    public fun readDouble(): Double
    public fun readString(): String
    public fun readNull(): Unit
}
```

一个Encoder/Decoder代表一个值；为该值选择一种container。每个field/element返回处理其值的子入口，按调用顺序完成子值，再继续下一个；单值container恰好读/写一个标量。容器end检查该层的完整性，根入口检查完整文档消费。数组下标及数量沿现有库使用Long。

path表示输入/输出数据位置，采用从空串根开始的JSON Pointer segment形式：字段名中的`~`、`/`分别转义为`~0`、`~1`，序列使用十进制索引。它是普通String，便于核心codec及合成分支构造带位置的错误，不包含运行期类型描述。核心库的EncodingException/DecodingException是Exception子类，构造参数为`path: String, message: String`，公开只读path；message沿既有Exception的`Option<String>`表示。

`optional(name)`只在key不存在时返回None；格式中的显式null仍返回Some(Decoder)，之后按字段codec处理。`keys`是输入数据中的键列表，不是类型描述。keyed读取不要求输入字段顺序与请求顺序相同；未知key可跳过但仍需是合法格式，重复key必须失败。unkeyed的element在耗尽时失败，end拒绝剩余元素；tuple据此检查长度。

类型相关调用留在合成body及普通泛型helper中：`codec.encode(value, child)`与`codec.decode(child)`；codec分别是满足`Encodable<F>`或`Decodable<F>`的普通值，F是字段的完整静态类型。不要求interface方法级泛型、Any中间树、runtime SerialDescriptor或反射字段访问。JSON、二进制格式等由普通库实现这些interface；合成方法遵守同一容器协议。

#### 11.13.3 缺省数据模型与构造

| Scoop类型 | 缺省编码形状 |
| --- | --- |
| Boolean / 整数 / String | 对应单值；有符号/无符号分别经过Long/ULong，窄整数decode检查范围 |
| Float / Double | 分别经writeFloat/readFloat或writeDouble/readDouble，不经整数或另一浮点精度中转 |
| Char | 恰好一个Unicode scalar的String |
| Unit | 单值null；不借用Scoop的Option表示 |
| struct / 合格class | 按声明序写入字段的keyed record |
| tuple / Array / MutableArray / ArrayList | 按位置或索引写入的unkeyed sequence |
| enum（包括Option） | 恰好一个key的外层record；key为variant名；命名payload为record、位置payload为sequence、unit variant的payload为record且编码时为空 |

`SerialName`只更改record字段或enum variant的wire名称，不更改源码名称、类型identity或字段类型。它不能用于位置payload/tuple、type本身或computed property。`Transient`排除存储字段/property，不适用于type、variant或位置元素；合成decode时该字段必须能从已声明default或正常class初始化取得值。两者不能同时标在同一target。参与同一record的key、同一enum的variant wire名须唯一；检查仅使用该方向实际派生的成员。

编码总是写出参与字段，包括等于default的值；不为了省略字段而求值default或调用equals。解码按构造参数顺序读取并类型化已提供字段，字段没有default时必须存在，包括Option字段；有default时仅记录缺失。完成container检查后，按参数声明序对缺失项执行已在定义处绑定的default，允许引用前面的参数，并且每次构造至多执行一次。输入显式null、错误类型或越界值不能触发default。Transient参数也按该顺序取default，最后调用选定的正常primary/variant constructor。命名字段的输入顺序不改变这些规则。

外层“字段是否存在”的Option与字段本身的Option值是两层独立值。缺省Option编码按普通enum保留None/Some区分；不自动展平为nullable单值。例如JSON中None为`{"None":{}}`，Some(None)为`{"Some":[{"None":{}}]}`。更紧凑的nullable协议由显式codec定义。

enum解码检查外层恰好一个key，未知variant失败；构造选中的variant，不使用内存tag、niche或ordinal作为wire tag。class解码先取得全部构造实参，再执行普通分配及完整初始化，不分配后逐字段反射填充、不调用setter修补半成品。失败沿普通异常/GC规则清理，不返回部分对象。

默认语义编码树形值：共享引用可展开为多份值，decode不保留原对象identity。循环图、跨对象引用及开放多态的discriminator由显式codec定义，不属于自动派生数据模型。

普通 JSON 库提供 `Json.encode<T>(value: T, codec: Encodable<T>): String`与`Json.decode<T>(text: String, codec: Decodable<T>): T`两个入口。调用如`Json.encode(value, User.Companion)`与`Json.decode(text, User.Companion)`，T按普通实参推断；两个入口都要求显式codec，没有从T或运行时数据类型隐式寻找codec的重载。JSON实现负责语法、Unicode/转义、重复key、数值范围和完整输入消费；整数不经过Double转换，整数字段不接受带小数部分或指数部分的数字token。record已可覆盖对象形式，array覆盖序列，不增加Map协议。格式错误抛带数据路径的EncodingException/DecodingException；用户codec、default和constructor抛出的普通异常照常传播，不改写为default或空值。

Float/Double的companion分别以普通core body实现`Encodable<Float>`/`Decodable<Float>`与`Encodable<Double>`/`Decodable<Double>`，透明alias复用同一codec。single-value 使用上表的两种精度入口，不为所有格式统一禁止非有限值。JSON仅编码有限浮点数，使用11.2.2的十进制表示并保留负零；编码NaN或Infinity抛带path的EncodingException。解码接受合法JSON整数、小数或指数token，保留原始数字文本并直接按目标精度最近偶数舍入，Float不得经Double中转。舍入溢出到Infinity抛DecodingException，下溢到subnormal或带符号零合法；非数值token不进行字符串/布尔值强制转换。JSON语法不接收NaN/Infinity拼写。两个精度的有限值文本往返保持位型，整数解析仍须精确并满足对应范围规则。

#### 11.13.4 编码、解码依赖与泛型组合

合成`Encodable<R>`.encode或`Decodable<R>`.decode时，目标R必须具有已知的struct/enum/tuple或合格class形状，R中的类型参数可以尚未具体化。两个方向分别按以下顺序确定每个参与字段F所需的`Encodable<F>`或`Decodable<F>`值；选择在定义处完成并保留实际声明引用，不在具体化时重新匹配：

1. 当前codec class/struct的primary constructor中，以val保存且静态类型满足所需完整interface application的显式依赖。没有匹配才继续；多个匹配诊断歧义，不按参数名称或顺序任选。companion没有constructor，不扫描其任意property、Context或整个词法作用域寻找依赖。
2. F与目标R相同则使用this，支持当前方向的递归调用。
3. F是可明确命名的名义类型，且它的可见普通companion实现了所需的`Encodable<F>`或`Decodable<F>`，则使用该值。F为`Envelope<E>`等完整application时保留宿主实参；不向F的基类companion回退，仅有同名encode/decode/encoder/decoder方法也不构成conformance。
4. F为核心Option/Array/MutableArray/ArrayList时，递归取得相应元素codec并调用完整宿主companion上预定义的普通encoder/decoder方法，例如`Array<E>.Companion.encoder(element)`。Unit使用UnitEncoder/UnitDecoder，tuple按元素递归组合普通codec。其余缺失情况报错，要求显式注入或手写body；不能在实例化时猜测裸类型参数的companion。

同一字段类型可以复用同一依赖，不同字段需要不同策略时手写相应方法。两个不同binder在具体化后恰好相同不触发重新选择或新的歧义；body继续使用定义处选定的参数。generic用户类型不按名约定自动调用companion.encoder/decoder；调用方显式组合其codec并传入需要它的provider。

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

两个helper缺少的方法分别合成：BoxEncoder从数据实参读取value字段并调用this.element.encode；BoxDecoder调用this.element.decode后正常构造`Box<E>`。入口可写`Json.encode(box, Box<Long>.Companion.encoder(Long.Companion))`或`Json.decode(text, Box<Long>.Companion.decoder(Long.Companion))`，也可使用普通companion转发。方法中的T来自宿主，helper的E由构造实参推断；encoder/decoder方法没有另行声明的类型参数。元素codec属于本次返回的对象，不写入companion字段；同一个数据类型可使用多个编码或解码策略，数据本身无需编码bound，其他用户声明的泛型约束保持原义。

Option/Array/MutableArray/ArrayList的encoder方法接收`Encodable<T>`并返回相应容器的编码器，decoder方法接收`Decodable<T>`并返回解码器。两个方向都由普通持有依赖的对象实现，数组只处理逻辑元素，Option保留enum形状。tuple没有companion，可由普通object/class/struct显式实现完整的tuple codec接口请求合成；自动组合tuple字段时，普通闭包分别由核心EncodeFunction或DecodeFunction适配，不增加tuple metatype或按arity命名的源码类型。

```scoop
public class EncodeFunction<T>(private val body: (T, Encoder) -> Unit) : Encodable<T> {
    public override fun encode(value: T, encoder: Encoder): Unit = body(value, encoder)
}

public class DecodeFunction<T>(private val body: (Decoder) -> T) : Decodable<T> {
    public override fun decode(decoder: Decoder): T = body(decoder)
}
```

这些codec、闭包及依赖都是普通typed值，按现有调用、分派、初始化和GC规则执行。encode按声明/元素顺序读取数据、取得child，再求值已选codec并调用；每个数据读取与child取得只执行一次。decode先取得实际存在的字段child，再求值codec；缺失的可选字段、Transient字段及未选中的variant不求值字段codec。组合在相应方法执行时进行，不提前成新的singleton初始化链。调用方显式传入的codec仍按原实参求值规则处理。没有全局codec registry、隐式witness参数或对运行期类型描述的查询。

### 11.14 原子类型与内存序

core 提供 `AtomicInt`、`AtomicLong`、`AtomicBoolean` 和 `AtomicRef<T : ref>` 四个 intrinsic final class，构造时接收对应类型的初值。复制原子对象的引用仍访问同一存储位置；不能继承这些类型，也不能在源码中直接访问其值字段。它们不实现 Equality、Hash 或 ToString，比较应显式读取值后进行。

```scoop
public enum MemoryOrder {
    Relaxed, Acquire, Release, AcqRel, SeqCst
}
```

首版开放全部五种内存序，默认 `MemoryOrder.SeqCst`，不提供 Consume。下表中的 `V` 分别表示 Int、Long、Boolean 或 AtomicRef 的实际 T；它是文档记号，不是新增的源码类型。

| 公共方法 | 语义 |
| --- | --- |
| `load(order: MemoryOrder = MemoryOrder.SeqCst): V` | 原子读取 |
| `store(value: V, order: MemoryOrder = MemoryOrder.SeqCst): Unit` | 原子写入 |
| `exchange(value: V, order: MemoryOrder = MemoryOrder.SeqCst): V` | 写入并返回旧值 |
| `compareAndSet(expected: V, value: V, successOrder: MemoryOrder = MemoryOrder.SeqCst, failureOrder: MemoryOrder = MemoryOrder.SeqCst): Boolean` | 比较并在相等时写入，返回是否成功 |
| `compareAndExchange(expected: V, value: V, successOrder: MemoryOrder = MemoryOrder.SeqCst, failureOrder: MemoryOrder = MemoryOrder.SeqCst): V` | 比较并在相等时写入，返回该次操作观察到的旧值 |

AtomicInt/AtomicLong 另提供 `fetchAdd`、`fetchSub`、`fetchAnd`、`fetchOr`、`fetchXor`，AtomicBoolean 另提供 `fetchAnd`、`fetchOr`、`fetchXor`；各方法接收一个 V 和默认 SeqCst 的 order，原子更新后返回旧值。整数加减按对应位宽 wrapping。CAS 为 strong，不允许无竞争时的伪失败；AtomicRef 的 expected 比较使用对象身份，不调用用户 equals。GC 移动对象不改变这一身份。

内存序必须是编译期常量，非法操作／内存序组合为编译错误，规则与 C11 对应操作一致：

| 操作 | 允许的内存序 |
| --- | --- |
| load | Relaxed、Acquire、SeqCst |
| store | Relaxed、Release、SeqCst |
| exchange、fetch 系列、CAS 成功 | 全部五种 |
| CAS 失败 | Relaxed、Acquire、SeqCst，且不能强于成功序 |

CAS 的合法失败序按成功序精确定义：Relaxed → {Relaxed}；Acquire → {Relaxed, Acquire}；Release → {Relaxed}；AcqRel → {Relaxed, Acquire}；SeqCst → {Relaxed, Acquire, SeqCst}。省略的参数仍取声明的 SeqCst 默认值，不根据成功序暗中改写失败序。

Relaxed 只保证该原子位置的原子性与修改顺序。release 写与读到该值或相应 C11 release sequence 的 acquire 读建立同步；每个原子位置的修改顺序、RMW 及其他同步规则遵循 C11。SeqCst 操作另有一个全序，该全序本身不额外建立跨线程 happens-before。原子对象自身也须按 1.2 正确发布；AtomicRef 不自动同步所指对象的后续普通字段访问。

`getAndUpdate`、`updateAndGet` 等便利方法由同一类中的普通 Scoop CAS 循环实现，更新函数可能重试，不增加 intrinsic。无符号宽度包装和任意值类型的 `Atomic<T : value>` 由普通库组合既有原子类型实现；这不赋予普通字段、数组元素或 aggregate copy 原子语义。首版不提供 `Ptr<T>` 所指 native 内存上的原子操作，后续按平台库的实际需求增加。

---

## 12. Cone模块、package与库

### 12.1 基本概念

- **Cone是module、分发、依赖、编译与静态链接的基本单位；package只是源码namespace。** Cone不等于package，二者不能由名称或目录互相推导。一个Cone可以包含多个package，同一package也可以由多个Cone贡献声明；后一情形称为split package，不会把不同origin的实体合并为同一identity；
- manifest source Cone是包含`Cone.toml`及按12.2.2选择的Scoop源码的独立目录；未配置源码清单时使用`src/`。`scoop build/run <file.scoop>`则把指定的唯一文件构造为12.2的synthetic executable source Cone。binary Cone是该语义单元编译得到的`.slib` artifact。面向构建的`scoop`可以将manifest Cone的上游依赖定位到source Cone或已经验证的binary Cone，二者必须声明同一Cone identity；低层single-Cone compiler `scoopc`只消费已生成并显式传入的上游`.slib`，不跟随source locator或递归构建其他Cone；
- 文件路径不决定package。source file至多声明一个`package` header，省略时属于root package；跨package引用必须按12.4导入或使用合法qualified path；
- package/FQN只参与名称组织与诊断，不是type、callable、property或其他实体的全局identity。相同package/name来自不同Cone时保持不同typed origin，并在同一lookup层相遇时按12.4诊断，而不是按依赖或链接顺序任选一个。

### 12.2 Cone identity与`Cone.toml`

每个 Cone 具有 canonical coordinate `group:name:version`。`group` 与 `name` 各由一个或多个 `.` 分隔的 lowercase ASCII segment 组成，每段精确匹配 `[a-z][a-z0-9-]*`；`version` 是 canonical SemVer 2.0.0 文本，禁止多余 `v` 与非法前导零，合法 pre-release/build metadata 原样参与 identity。不做 trim、大小写折叠、Unicode 归一化或路径别名解析，不满足语法的 coordinate 是错误。

`ConeIdentity` 由完整 canonical coordinate 确定；version 参与身份，因此两个版本即使源码相同也不是同一声明或类型的来源。artifact 内容具有独立的 `ArtifactFingerprint`，不能用它、package、FQN 或 manifest 路径代替 ConeIdentity。identity 与产物指纹的编码见编译器与产物规范 2.6。

manifest-backed生产source Cone的最小manifest schema为：

```toml
schema = 1

[cone]
group = "dev.example"
name = "my-lib"
version = "0.1.0"
kind = "library" # 或 "executable"

[dependencies]
"org.foo:bar" = { version = "1.2.3", path = "../bar" }
"org.acme:util" = { version = "2.0.0", artifact = "../artifacts/util.slib" }
"org.other:log" = "3.1.0"
```

- `schema`以及`[cone]`中的`group`、`name`、`version`、`kind`必需；`kind`只能是`library`或`executable`。拼错或未知的semantic field不能被静默忽略；
- dependency key是exact `group:name`，value必须给出exact version；字符串短式只省略locator。table value至多给一个`path` source-Cone locator或`artifact` `.slib` locator；二者都省略时，`scoop`在每个显式artifact search root下检查`<group>/<name>/<version>/cone.slib`，三项都使用canonical文本且`.`不拆目录。所有存在的候选必须具有相同完整`ArtifactFingerprint`，否则是ambiguous artifact错误；search-root顺序不能决定选择不同内容；
- 除 core 按 12.6 定位外，没有显式 locator 且全部显式 artifact search root 均无候选时，仅 `group = "scoop"` 的依赖继续检查 `<sysroot>/lib/<name>/Cone.toml`。其完整 coordinate 必须匹配所声明的 group/name/version，且为普通 library Cone；其他 group 不作此默认查找。已选择的显式来源损坏、歧义或不兼容时直接报错，不以 sysroot 覆盖。默认查找不注入新依赖、不改变名称可见性或实体身份；
- locator只用于当前构建查找，相对路径以当前manifest为基准；它不进入Cone identity、实体identity、`.slib` metadata、初始化顺序或源码诊断identity。locator解析到的source/artifact coordinate必须与dependency key/version canonical相等；
- dependency声明没有version range、`latest`、optional/dev/build dependency、feature、platform条件或dependency alias。`scoop`只把exact locator解析为12.3的resolved graph，不执行版本选择或冲突调停；`scoopc`仅验证当前manifest的semantic projection与命令行显式提供的binary dependency closure，不解析任何locator。native C++模式见12.2.1；
- `scoop build [root-input]` 与 `scoop run [root-input]` 允许省略 root：此时明确使用调用者当前目录的 `Cone.toml`，与显式传入该文件相同；缺失或无效时报告输入错误，不向父目录搜索，也不猜测某个 `.scoop` 文件。显式 root 继续按目录、`Cone.toml` 或单文件规则分类；低层 `scoopc build` 仍要求明确输入；
- `scoop:single-file:0.0.0`是另一reserved coordinate，用户manifest不得声明它。`scoop build/run`收到basename扩展名精确为`.scoop`、跟随symlink后目标为已存在regular file的operand时，构造`kind = executable`、source set恰好为该文件、logical source path恒为`main.scoop`、direct Cone dependency恰好为core的typed synthetic projection；symlink cycle、dangling link或最终目标非regular file是输入错误，resolved host path不进入identity。显式文件operand即使位于某Cone目录中也不读取相邻`Cone.toml`、其他`.scoop`、C/C++ source或blob；它不接受其他Cone dependency。该`.slib`可缓存，并可作为产生它的build/run或显式`scoop link --root-slib`的唯一executable root；但不能作为可分发artifact发布、作为dependency或被manifest artifact locator引用；
- manifest Cone的source identity是`(ConeIdentity, normalized Cone-relative source path)`，保留从Cone根开始的完整路径，不以显式源码根为基准截短；single-file source使用相同pair形态，但第一项固定为由reserved `scoop:single-file:0.0.0`计算的`ConeIdentity`，第二项固定为`main.scoop`。host绝对路径、CLI relative/absolute/symlink spelling、inode、mtime、目录枚举顺序与临时输出路径不进入语义identity；只有语言允许跨文件同名的file-private/hidden实体才把该source identity加入其declaration key。single-file artifact/cache key另外包含source content digest、core semantic/code fingerprints、compiler/schema/target/toolchain，不因共用reserved identity而碰撞；

#### 12.2.1 Native C++ 模式

manifest Cone 通过 `[native]` 中的 Boolean 字段 `cxx` 显式启用 C++，省略时为 `false`。例如：

```toml
[native]
cxx = true
cxx_flags = ["-std=c++20"]
```

- `cxx = true` 声明当前 Cone 需要 C++ 编译／链接支持；不能仅凭文件后缀、编译参数或待解析的符号隐式开启。选中的 `.cc`、`.cpp`、`.cxx` 或 `.C` native 源码要求该开关为 `true`，否则在 native 编译前报错。
- `.c` 文件仍按 C 编译；开启 C++ 不改变 C 文件的语言。`c_flags` 与 `cxx_flags` 是字符串数组，分别传给当前 Cone 的 native C 与 C++ 编译器，不向依赖者传播，也不改变 Scoop、generated-C bridge 或 runtime 的编译配置。参数不能覆盖 driver 管理的 target、sysroot、输入语言、编译阶段和输出位置。
- C++ 编译使用所选 target 的配套 C++ driver，例如 GNU 工具链的 `g++` 或 Darwin Apple Clang 的 `clang++`。缺少匹配的 C++ 编译器或运行库时报错，不能使用其他 target 或宿主机的工具链代替。
- C++ 链接需求写入 `.slib`，并沿最终程序的完整依赖闭包传递。即使 root Cone 自己没有 C++ 源码或未开启 `cxx`，只要闭包需要 C++，最终链接就必须使用配套的 C++ driver 与 C++ 运行库配置；`cxx = false` 不能取消依赖的要求。
- **当前不支持 C++ 与 musl target 的组合**，包括 musl 的静态和动态链接模式。当前 Scoop musl 工具链没有配套的 C++ 标准库与 C++ ABI 运行库；musl 提供 libc 本身，不包含这些库。这是当前工具链集成范围的限制，不是 musl 原理上不能支持 C++。musl 下当前 Cone 开启 `cxx`，或依赖闭包要求 C++，都必须给出明确的不支持诊断。
- C++ 支持目前覆盖 Darwin/AArch64 与 Linux/amd64 GNU。Scoop 与 C++ 之间通过现有 C ABI 和 `extern "C"` 包装函数互操作；C++ 异常必须在 native 一侧处理，不能穿越 Scoop 的 FFI 边界。工具链、产物与链接细则见实现规范 2.7～2.8，异常边界见运行时规范 5.5。
- single-file 模式不读取 `[native]`，不启用该配置。

#### 12.2.2 Scoop 源码选择与平台条件

manifest 顶层可使用 `[[sources]]` 表数组，每项具有必需的 `path` 和可选的 `when`。例如：

```toml
[[sources]]
path = "src/common"

[[sources]]
path = "src/os/linux"
when = { os = "linux" }

[[sources]]
path = "src/os/darwin"
when = { os = "darwin" }
```

- **默认模式**：未出现顶层 `sources` 字段时，递归收集 `src/` 下扩展名精确为 `.scoop` 的 regular files。`native.sources` 不改变这一默认规则。
- **显式模式**：出现 `sources` 字段后，它就是完整的 Scoop 源码选择清单，公共目录或公共文件也必须列出；不再隐式扫描或追加 `src/`。显式空数组、所有条件都不匹配或选中目录没有源码时，不回退到默认模式；最终源码集合为空是构建错误。
- **路径**：`path` 是 Cone 相对的目录或单个 `.scoop` regular file，不解释为 glob，不限于 `src/`。目录递归收集 `.scoop` 文件。路径归一化、非 UTF-8、选中路径不存在或类型不符、symlink 逃出 Cone 根或形成循环等错误在 parse 前报告。
- **条件**：省略 `when` 表示无条件；`os ∈ {darwin, linux}`、`arch ∈ {aarch64, x86_64}`、`env ∈ {gnu, musl, none}`，均按构建 target 求值。多个键取“与”，同一键的字符串数组取“或”；未知字段、键、值或错误的数据类型是 manifest 错误。该谓词也用于 `native.sources` 和 `native.libraries`，不为 Cone dependency 增加平台条件。
- **选择顺序**：先校验全部条目的字段、路径形式和 `when`，再按 target 筛选；只对选中的路径检查文件系统存在性、枚举源码并进行解析和语义检查。没有被任何匹配条目选中的文件不参与编译，不能由默认扫描补入。
- **重叠与重复**：同一 target 下选中的目录不能相同或互相包含，选中的单文件不能同时被选中目录覆盖。重复的规范化路径、或经 symlink 解析到同一源文件的重复选择，均报构建错误并指出相关条目；不静默去重，不定义先后覆盖。互斥条件在不同 target 选中同一路径不构成重复。
- **确定性**：最终文件按完整的 normalized Cone-relative `/` path 的 UTF-8 byte order 排序。相同文件的 source identity 不因显式/默认选择、清单顺序或源码根分组改变。源码选择与缓存规则见实现规范 2.1、2.7。
- **范围**：single-file 模式继续只编译指定文件，不读取相邻清单。平台条件仅选择文件或目录，不提供 Scoop 文件内条件编译、声明级 `@Target` 或 `expect`/`actual`。

#### 12.2.3 Native 系统库

`[[native.libraries]]` 以必需的 `name`、可选的 `kind` 和 12.2.2 的 `when` 声明逻辑 native 库；name 不是文件路径或 linker 参数，kind 沿用实现规范 2.8 的库种类。按 target 筛选后的要求写入 `.slib` 并沿依赖闭包传递，与非空 `@Extern(lib = ...)` 使用同一库合并、符号绑定和冲突规则。

库解析先检查全部显式 library roots；没有候选时允许使用所选 target 的平台 provider 默认目录。Linux 使用所选 native toolchain/sysroot 的系统库目录，Darwin 使用所选 SDK 的库和 framework 目录。交叉编译不回退到宿主工具链或宿主系统目录。显式候选损坏、不兼容或歧义仍报错，不能静默改用系统库。

平台默认查找只解析已经声明的逻辑库需求，不发现相邻 native 源码或任意库；空 lib 的默认 namespace 也不因此扫描系统目录。具体 target、link mode、候选格式、产物绑定和缓存规则见实现规范 2.8。

### 12.3 静态exact依赖图

依赖图必须在最终链接前完整确定。构建、运行与产物链接命令的输入、输出和兼容性要求见编译器与产物规范 2.7～2.8。

- 依赖边必须无环；同一图中同一 `group:name` 只能出现一个 version，同一 ConeIdentity 只能对应一致的产物与语义指纹。环、多个版本、同 identity 的内容冲突或 dependency coordinate 不匹配均为构建错误。
- executable 不能作为另一个 Cone 的依赖。一次程序构建恰有一个 executable root，其余节点均为 library；library 单独构建不需要 executable root。
- 除 core 自身外，每个未显式声明 core 的 Cone 都具有 12.6 的 core direct dependency；除此之外没有隐式依赖。core 可从普通 manifest 目录作为构建根，使用普通 library 规则，不加载另一份 core 或产生 self edge。sysroot 只提供默认定位位置。
- single-file 的图只含 core 与唯一 synthetic executable root，不发现其他 manifest 依赖；源码对其他 Cone 的 import 按不可达名称报错。`@Extern` 的逻辑 native library 需求不属于 Cone dependency。
- dependency path、manifest 枚举与输入顺序不影响初始化顺序。按 dependency-first 拓扑顺序处理 Cone；每一步从全部依赖已完成的 Cone 中，按 `(group UTF-8 bytes, name UTF-8 bytes, canonical version bytes)` 取最小者。
- eager top-level initialization 按上述 Cone 顺序执行，同一 Cone 内按 9.1.3 的 PersistentInitializationUnitId bytes 排序。object、companion 与 generic delegated extension 等 lazy unit 不提前执行。初始化期间读取另一 unit 时，先完成目标 unit 的初始化，因此目标可以早于其通常排序位置执行。
- 间接依赖的存在不使其声明自动成为源码候选。名称可到达性只由 12.4 的当前 Cone、direct dependency surface、re-export 与 core prelude 决定。
- 源码或依赖变化时，缓存不得复用不相容的结果；没有源码可重建的 stale dependency 是构建错误。
- 不提供 registry 下载、版本求解、lockfile、shared-library ABI、`dlopen`/`dlclose`、hot reload、image 卸载或运行期新增 Cone。

### 12.4 `package`、`import`、re-export与编译单元

#### 12.4.1 文件头语法

每个source file的顺序固定为：

```text
packageHeader? importHeader* declaration*

packageHeader  = package QualifiedName
importHeader   = public? import ImportSelector (as Identifier)?
ImportSelector = QualifiedName | QualifiedName . *
```

- `package`至多一次且必须先于所有import/declaration；import只允许出现在文件头。exact import可写`as`，alias只改变当前文件中的短binding名；star import不能写alias；
- 普通import只影响当前source file。`public import`同时建立当前文件的普通exact/star import，并在**当前文件package**下为当前Cone建立re-export binding；其destination name是`as` alias或target短名；
- `public`在这里是上下文关键字，只修饰import；不存在`internal import`或`private import`；
- import selector与qualified type path都先解析最长的可见package binding前缀，再沿static nested nominal、object或companion的typed owner edge查找；最长package前缀一旦选定便不回退到较短前缀重猜。exact import的终点必须是importable binding，star import的终点必须是importable namespace；不能把点连接的字符串直接当作FQN扫描全部artifact。top-level value/function表达式仍通过import后的短名或普通receiver语法访问，不新增dependency-coordinate-qualified源码名称。
- 限定类型路径的 package 前缀同时考虑当前 Cone 与 direct dependency 的公开包（含 re-export）。同一最长包中的当前与外来类型在同层合并，按 typed origin 去重并诊断不同实体的同名冲突；仅含 value binding 的公开包仍是可见包，不能因其中缺少目标类型而回退。仅供模板、布局或链接使用的 support dependency 不增加可见包。路径末端的普通类型、泛型 application 和 typealias 使用与短名查找相同的类型实参、可见性及展开规则。

#### 12.4.2 import可到达性与re-export

普通exact/star import selector只能解析：

1. 当前Cone中按visibility允许当前file访问的声明；
2. manifest direct dependency的public lookup binding；
3. 该direct dependency已经解析并公开的re-export binding；
4. core direct dependency的public/prelude binding。

import 指向原声明；经不同依赖路径或 re-export 到达同一声明时，视为同一 target，不因路径长度、输入顺序或加载顺序产生不同实体。

源码 selector 只能从当前 Cone 或 direct dependency 的可见包开始；选中实际名义 owner 后，可以继续访问其 public 静态嵌套 namespace，包括 object 与 companion。嵌套声明可以来自间接依赖，仍保持其原身份；这不使该间接依赖的其他 package 自动成为源码候选。

- `public import` 的每个最终 target 必须是依赖中的公开 binding；当前 Cone 声明不能用于 public import/re-export，也不能导出 internal、private 或 protected target。终点可以来自依赖的 re-export，或由已选名义 owner 到达的公开静态 namespace；snapshot 保存实际绑定引用，re-export 可以成链；
- re-export只建立destination package/name到origin typed实体的公开binding，不生成wrapper、forwarder、第二个TypeDescriptor、第二个typealias target、generic body或storage，也不扩大target member的visibility；
- public star在编译当前Cone时展开为逐项、已经解析的API snapshot；下游不重新执行上游的文本glob。target集合变化会改变当前Cone的re-export metadata/fingerprint；
- 同一 typed origin 经重复 import、多个 star 或钻石 re-export 到达同一层时合并为一个 target，并合并排序后的实际 binding 引用。终点相同的静态 owner 遍历不复制外层路径；实际绑定集合或依赖变化按既有规则影响 metadata／fingerprint。不同 origin 即使 package/name/signature 文本相同也不合并：非 overloadable 实体产生歧义；function/extension 可进入同一 overload 层，但展开后签名相同仍是冲突；
- split package合法，但不授予跨Cone可见性。exact selector命中多个不同origin的非overloadable实体时报告包含Cone coordinate的歧义；`as`只作用于已经唯一解析的exact selector，不能靠alias从一个本身歧义的selector中任选；
- 两个local public declaration/re-export在同一destination namespace形成不可重载冲突时，当前Cone本身即为定义错误，不能发布带歧义的`.slib`；
- public API签名通过typed dependency closure引用但未re-export的外部public type仍可供下游类型检查、推导、layout与member检查使用，却不会自动获得可书写的短名。希望只依赖当前Cone的用户直接写出该名称时，API作者应显式`public import`。

#### 12.4.3 名称与调用候选层

无显式receiver的名称/调用候选层从高到低为：

1. 词法binding与local function；
2. 隐含`this`的真实member；
3. 当前文件的exact import，包括alias与`public import`；
4. 当前package中可访问的本地声明与re-export binding；
5. 当前文件的全部star import；
6. core `.slib`提供的typed prelude；
7. 4.2/8.6由唯一expected exact enum application产生的contextual variant fallback。

显式receiver先查真实member；extension scope再按exact import → current package → star import → core prelude分层。property-like `invoke`在每层内继续使用9.3.4的c-level分区，property读写继续使用9.1.1的typed accessor/place规则，constructor、variant、operator、callable reference与typealias qualifier继续进入各自既有typed入口。

对callable层，每层独立执行8.6的shape filter、candidate-local applicability与MSC，只选择第一个至少含一个适用候选的层；某个高层只有不适用同名callable时继续到下一层。callable-value hard shadow等既有词法规则不变。type、object、property name等非overloadable lookup在同层出现多个不同origin时直接歧义。visibility 在 applicability 前按 9.1.5 检查，不产生额外访问凭证；声明顺序、dependency枚举、re-export链长与artifact加载顺序都不能作为tie-break。

跨Cone普通lookup只枚举public lookup surface。public open/abstract owner所需的protected constructor/member、abstract obligation、interface default source与override relation属于独立inheritance/slot surface，只能在符合访问规则的 subclass/implementation 上下文中使用，仍按实际声明、继承关系和接收者类型判断；generic template的internal/private hidden support也不进入普通import或名称候选。

#### 12.4.4 编译单元与entry

- Cone是独立的编译/静态链接单元，不默认按每个`.scoop`文件生成一个可依赖的语言模块。manifest Cone按12.2.2得到当前target的完整源码集合，`scoopc`消费该集合，不额外扫描`src/`；文件以normalized Cone-relative `/` path的UTF-8 byte order排序，空集合与路径冲突等构建错误在parse前报告。single-file mode是显式root-input例外：整个synthetic Cone恰好包含指定文件，它不会使每个普通source file获得可分发Cone identity；
- 同一Cone的全部source file一起建立语义环境，文件之间没有编译顺序。声明能否用短名访问仍由package/import与visibility决定，“同一Cone编译”不等于忽略namespace；
- `internal`精确表示origin Cone内可见；默认visibility及其他access domain见9.1.5；
- library不需要entry；其中名为`main`的普通声明不会因此获得entry linkage。executable root必须恰有一个符合下表的top-level ordinary、non-generic、non-suspend `main`，具有普通Scoop函数体；只在root Cone中发现，dependency中的`main`不参与竞争，entry可以保持internal；
- 最终程序从program metadata保存的typed root entry调用`main`；源码package或固定native符号名不决定entry。

| 入口签名 | 正常结束时的退出码 |
| --- | --- |
| `fun main(): Unit` | `0` |
| `fun main(args: Array<String>): Unit` | `0` |
| `fun main(): Int` | 实际返回的 `Int` 值 |
| `fun main(args: Array<String>): Int` | 实际返回的 `Int` 值 |

表中的参数名 `args` 不作要求；有参数时恰有一个普通参数，其类型是实际 core `Array<String>` 的 exact application，透明 alias 按普通类型规则展开。其他参数／返回形态不能充当入口；缺少入口、入口不唯一或候选签名不合法均为编译错误，诊断给出实际签名和允许的形态。

`args` 包含完整原生 argv：`args[0]` 是可执行程序自身的启动路径，后续元素才是用户传入的参数。这一点与 Java/Kotlin 的 `main(args)` **不一致**：Java/Kotlin 不在 `args` 中包含程序路径。Scoop 保留启动者传入的原生 `argv[0]` 字符串，不将其解析为绝对路径或规范化真实路径；`scoop run` 使用稳定的可执行输出路径。没有用户参数时 `args` 仍有这一项。参数按原有顺序保留，包括空字符串，不重新按空格拆分。

只有带参数的入口才在 eager 初始化完成后、调用 `main` 前构造 `Array<String>`。各项按严格 UTF-8 解码并复制为普通 managed String；非法 UTF-8 按启动失败报告参数下标（包括第 `0` 项），退出码为 `1`，不调用 `main`。无参数入口不构造该数组或要求原始参数为 UTF-8，可由平台库读取原始 argv 字节，见运行时规范 2.8、第 7 章。

返回 `Int` 的入口可以返回任意 `Int`，包括负数与 `Int` 的边界值；Scoop 将完整的有符号 32-bit 值作为退出码传递给目标进程退出机制，不额外限定为 `0..255` 或主动截断。当前 POSIX target 的父进程通常只能观察正常退出状态的低 8 位，这是操作系统的规则，不限制合法的 Scoop 返回值。四种入口因语言级 panic、未捕获异常或上述启动失败而结束时统一使用退出码 `1`；正常的 `Int` 返回值 `1` 不表示 gateway 调用失败。正常返回仍须完成运行时规范第 7 章的 shutdown 协议。

`main` 返回时仍有已 attach 的非主线程、活动 callback 或未释放的 callback token ownership，属于 shutdown 失败：报告剩余计数，刷新输出后以 `1` 退出，覆盖本次 main 的返回码，不等待线程自动结束。程序应在 main 返回前完成 join 和 token 释放；M33 不提供 daemon 线程。需要带着后台线程结束整个进程时可显式调用 core 的 `exit(code: Int): Nothing`，它先刷新输出再终止进程，不做上述 shutdown 检查、不等待 join，也不执行 finally 或 release hook；刷新允许等待，见运行时规范第 7 章。

### 12.5 编译产物 `.slib`

每个 Cone 产生 target-specific、版本化、确定性的 `.slib`，包含独立编译、下游类型检查、泛型实例化及链接所需的信息。library 的构建结果为 `.slib`；executable 也先产生完整 `.slib`，再与依赖闭包链接为程序。容器、metadata、版本、ABI、指纹与链接合同见编译器与产物规范 2.6～2.8。

跨 Cone 的声明保留原身份、可见性、签名、默认值、const、注解和完整类型关系。为继承、接口调用或泛型实例化保留的支持声明不扩大源码可见性；re-export 只增加名称 binding。透明 alias 不产生新的 exact type。

泛型、默认参数与词法 callable 正文保留定义处绑定的声明、类型及源码位置；消费方只替换实际类型实参，不重新选择 provider 的名称或重载。普通非泛型函数使用定义方的实现。跨 Cone 的构造、属性、接口、虚方法、`super`、模式及结构类型组合遵守与本地相同的语言规则。

不同 Cone 使用同一 generic 或 structural application 时，必须维持唯一类型身份及共享的状态、初始化和分派。不同声明或不同 application 不因名称、布局或机器码相同而合并。ZST 仍保留类型、求值、调用与对象身份。产物合并规则见编译器与产物规范 2.13。

所有 runtime image、类型、根、静态存储与初始化数据必须在首个 managed initializer 前登记；初始化顺序遵守 12.3。`.slib` 不提供运行期新增或卸载 Cone，也不承诺跨不兼容 schema、target 或 compiler ABI 的二进制兼容。

### 12.6 核心库

第11章的核心库是reserved library Cone **`scoop:scoop.core:0.1.0`**，以独立`.slib`提供，sysroot只提供默认查找位置；`scoop.core`同时是其源码当前使用的package和Cone `name`，这只是明确约定，不是package与Cone identity之间的语言推导规则。

core 是可修改、扩展和重建的普通 library Cone。intrinsic 与脱糖所需的声明按第 13 章解析；其余类型、可见性、ABI、产物与链接规则和普通库相同。sysroot 只提供默认 locator，不构成来源授权。

- 除core自身外，每个Cone都具有到该exact core Cone的direct dependency。manifest可以像其他依赖一样显式声明core的`path`、`artifact`或search-root locator；未声明时注入默认edge。先解析全图的显式声明，已有core节点则复用，仅当仍无core节点时才读取默认sysroot源码位置。显式locator失败按普通依赖报错，不回退到sysroot；不同core来源的冲突使用同一coordinate/content唯一性检查；
- core 自身不隐式依赖自身。core manifest 的显式 dependency 使用普通语法、定位与无环规则；core 到普通库的依赖若与该库到 core 的依赖形成环，按 12.3 报错。用户可在普通源码目录声明 core coordinate、修改或扩展源码，并重新构建 library；intrinsic 声明遵守第 13 章的名称、类型和唯一性规则；
- `scoop.core.*`与`scoop.core.Option.*`默认可见性来自core `.slib`中的typed prelude binding，不通过把core源码拼入用户AST、扫描package name或对`Some`/`None`写短名特判实现；
- core中的普通public API、generic template、non-generic alias、layout、TypeDescriptor与runtime binding遵守与其他library Cone相同的metadata、persistent identity和兼容检查。sysroot core与compiler的language/runtime、target及backend fingerprints不兼容时必须重建或拒绝，不能退回core与用户源码同单元编译。

---

## 13. 核心注解与 FFI

以下注解类定义于 `scoop.core`，随默认导入可用。它们修饰的约束大多在编译期检查，违反即为编译错误。

本章使用 **GC-free**：一个完整类型的表示不直接或间接包含任何 managed ref 时为 GC-free；基本标量、Ptr、FunPtr 及不含 ref 的值类型满足此条件。enum 只有全部 variant 都满足时才 GC-free。未替换完类型参数的泛型使用符号条件，每个实际 application 都须满足条件。GC-free 函数不读写或创建 ref value（13.2）。

### 13.1 `@Intrinsic`

```
annotation class Intrinsic(val name: String)
```

- 用于function/method：该函数是compiler intrinsic，由编译器生成实现；**函数体必须省略**；`name`是intrinsic的编译器内部标识。method必须仍按普通member语法、visibility与operator签名规则声明，不能用extension形态绕过登记shape。

- 也可用于登记表明确允许的core struct/class，声明一个**intrinsic type**：该类型的内存表示、字面量及ABI遵守本规范，源码声明其nominal interface与成员语义。例如：

```
@Intrinsic("core_int")
public struct Int : Equality<Int>, ToString, Hash {
    @Intrinsic("int_add")
    @NoGC
    public operator fun plus(rhs: Int): Int

    @Intrinsic("int_equals")
    @NoGC
    public override operator fun equals(other: Int): Boolean

    public override fun toString(): String = coreLongToString(this.toLong())

    public override fun hash(): Long = coreLongHash(this.toLong())
}

@Extern(name = "scoop_rt_long_to_string", abi = "scoop")
internal fun coreLongToString(value: Long): String

@Extern(name = "scoop_rt_long_hash", abi = "scoop")
internal fun coreLongHash(value: Long): Long
```

- intrinsic type不声明字段/primary constructor，也不等价于零字段普通struct/class；不能据此派生零大小布局、全等equals、`Int()`字符串、字段访问、解构、copy update或公开零参数constructor。编译器合成的literal/boxing/allocation entry不进入源码候选集；用户可调用constructor或转换来自显式声明或 registry 封闭规定的入口：10.4 的数组转换、10.6 的数组按长度初始化与 13.10 的 `Ptr<T>(raw: ULong)` unsafe construction entry；
- intrinsic type的implements列表、普通成员body、override/operator规则与普通类型一致；11.1 的两个无成员根类型遵守该节的封闭声明形状。单个成员也可像上例一样另用function intrinsic提供实现。intrinsic type覆盖八种canonical integer representation、Boolean、Char、Float/Double、String、`Any`/`Nothing`、`Array<T>`/`MutableArray<T>`以及13.10的`Ptr<T>`/`FunPtr<F>` family；integer登记项必须封闭地给出signedness与8/16/32/64位width，alias本身不能再次登记为intrinsic type。`core_int`/`int_*`表示32位canonical `Int`，64位signed表示使用独立的`core_long`/`long_*`；unsigned同理区分`UInt`与`ULong`。登记表可按同一契约增加其他compiler-represented value/reference type；
- intrinsic type 可以是 generic，但登记项必须完整规定 declaration kind、类型参数数量、bound 及表示；所有参数按 3.2 固定为 invariant。`Array<T>` 与 `MutableArray<T>` 各要求一个无 bound 参数，每个完整 application 是具有唯一类型身份的普通 generic class application，其元素表示遵守第 10 章；
- intrinsic 声明的 name、target、shape、signature 及唯一性必须满足登记项。

- 除非有单独说明，`@Intrinsic` 不能与其他任何注解共存。integer registry中除`div`/`rem`外的纯scalar operation与conversion是一个封闭例外：其声明必须同时带`@NoGC`；`div`/`rem`不得带。 Char code/相等/比较和 core 内部 unchecked code-point 构造均为 NoGC scalar operation，其中 unchecked 构造还必须标注 `@Unsafe`；公开 `Int.toChar` 在普通 core body 中先检查范围再调用。13.10列出的pointer intrinsic继续按该节例外组合`@NoGC`/`@Unsafe`。
- `name` 必须是编译器内置intrinsic登记表中的已知标识；未知`name`、错误annotation target、与登记shape/signature不符或同一intrinsic kind存在多个provider都是编译错误（用户不能声明自定义intrinsic）。阶段契约见编译器与产物规范 2.10。

### 13.2 `@NoGC`

```
annotation class NoGC
```

- 用于function/method及9.1.5允许的explicit accessor/struct secondary constructor：指明该callable不会/不应与GC有任何交互——有body的callable中不读写任何ref value，也不创建任何ref type实例。唯一无body的组合是`abi = "scoop"`的top-level `@Extern` function：此时`@NoGC`是由FFI作者承担的callee contract assertion，并使其`GcEffect`取`NoGc`；省略时取`Managed`。C ABI extern本身不能直接接收managed ref或调用Scoop GC，不接受`@NoGC`这一重复且易混淆的拼写；省略native状态切换须使用13.4.1的`@GCLeaf`并满足其更强的调用链契约。
- struct secondary constructor的`@NoGC`要求完整参数、构造结果、委托求值与body中的运行时值均为GC-free，`this`只能委托primary或另一个`@NoGC` secondary constructor；未标注的secondary即使body看似纯净也保持Managed合同。primary struct与enum variant的直接值组装可出现在`@NoGC`代码中，但实参求值与完整结果表示仍须满足GC-free约束。参数缺省表达式在caller求值，遵守caller的GC effect，不因callee的`@NoGC`而自动获得GC-free资格。
- 也可用于`struct`或`enum`，作为“该concrete value type必须GC-free”的静态契约。非generic声明在字段类型解析后立即验证；generic声明本身没有GC-free真假值，每个type parameter全部resolve后的实际类型分别验证。对fully specialized enum，契约同时要求enum整体及每个variant均为GC-free。`@NoGC`不能用于class/interface，因为它们是ref type。 当前声明与依赖声明的契约相同；完整 application 只出现在签名、别名、父类型或嵌套类型中也必须满足，不能等到访问字段或执行构造才检查。泛型使用把实际影响该契约的形参条件沿既有实例化关系传播，phantom 参数不因此成为 GC-free 条件。
- 编译期检查；不符合约束是编译错误。
- 无自定义正文、无需初始化 ensure 且值类型为 GC-free 的普通顶层存储访问器，只有读取或写入既有存储的操作，编译器生成的 callable 使用 NoGc 合同；这使同一属性的直接访问与跨 Cone 泛型访问保留相同 GC effect。带运行时初始化、含 managed ref 或有自定义正文的访问器不据此推导 NoGc；显式正文仍按其声明的 annotation 检查。
- generic `@NoGC` callable 可以在签名或 body 中使用类型参数；未特化的generic本身不被判为GC-free或非GC-free。每个实际影响参数、返回值、receiver、局部值或表达式表示的类型参数，都会形成“实例化实参必须GC-free”的类型化条件，并经generic调用链向外传播；只有type parameter全部解析后的具体实例才能用concrete type的GC-free flag验证并成为`@NoGC`实例。未参与运行时表示的phantom type parameter不产生条件。
- `T : value` 只保证实参是 value type，不保证其递归表示中不含 managed ref，因此不能代替上述 GC-free 条件；`T : ref` 则不可能满足该条件。当前没有单独的源码 bound 语法来声明 GC-free，条件由 `@NoGC` body及其调用图推导。
- 这样的函数可以安全地跨越 FFI boundary（例如作为 FFI 回调）。
- 该约束也意味着 `@NoGC` 的成员函数只能属于 value type：class method 有隐含的 `this` 参数，而 `this` 是 ref value。
- value type 的 `@NoGC` 成员可以实现未标注 NoGC 的普通接口方法，这是对实现体 GC effect 的收紧，其参数、结果、receiver 和 body 仍须满足全部 NoGC 约束。经具体类型直接选中该实现时保留 NoGC 合同；经 interface 或只有该 interface bound 的静态类型调用时，仍使用接口的 Managed 合同，不能因为某个实现是 NoGC 就把所有实现视为 NoGC。装箱／接口表入口按实现规范 2.9 适配；基本类型的 NoGC `equals` 因此可实现普通 `Equality<T>`，直接整数／浮点比较不增加 GC 操作。
- 8.3 的 contextual declaration 不得标注 `@NoGC`；`context(value) { ... }` 也不是 NoGC 操作。runtime 的无分配 lookup/restore leaf 不等于源码 `@NoGC`，它们仍读取或写入 managed ref。
- 9.1.6 的 release block 不是普通 callable 或 `@NoGC` target。定义方在已有 NoGc 检查上推导更窄的 release-call effect 与 `ReleaseValue` 条件，并通过普通 callable 接口供依赖使用；`@NoGC` 本身不保证没有 native transition、TLS 或 GC capability 操作。

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

- **unsafe callable**：C ABI `@Extern`函数，以及标注`@Unsafe`的function、9.1.5允许的constructor/accessor。Scoop ABI extern默认safe，但可显式加`@Unsafe`收紧调用条件（13.4、14.2）。
- **unsafe context**：标注`@Unsafe`的callable body，或标注`@Unsafe`的block。只有在unsafe context中才能调用unsafe callable、构造unsafe constructor或读写unsafe accessor；否则是编译错误。
- `@Safe`用于function、9.1.5允许的constructor/accessor和block，在unsafe context中重新引入safe约束。

### 13.4 `@Extern` 与 `@CallingConvention`

```
annotation class Extern(val lib: String = "", val name: String = "", val abi: String = "c", val captureErrno: Boolean = false)
annotation class CallingConvention(val name: String)
```

- `@Extern` 用于top-level、non-generic function：指明该函数是位于 `lib` 所指库中的 FFI function，符号名由 `name` 指定，`abi` 指定 ABI（见 13.8）。函数体必须省略。声明可以带普通Scoop默认参数；缺省表达式按8.5在定义处解析、调用处实例化，不进入native symbol的ABI，native调用始终接收完整参数列表。`vararg`在ABI中表现为一个普通`Array<T>`参数，因此只有该实际参数类型满足对应ABI classifier时才合法：C ABI因ref不安全而拒绝，Scoop ABI可以接受；这不表示支持C的`...`可变参数。
- `@Extern` function与member/local、generic或`suspend`均互斥，无论`abi`取值为何都在声明处报编译错误。尤其不能把generic extern的多个concrete ABI绑定到同一个native symbol。编译器不为这些非法声明生成receiver、type-argument或continuation bridge；hidden continuation ABI不得作为外部符号ABI暴露。
- `@Extern` 也可用于**全局变量**（`val` / `var`），访问库中的全局符号；extern `var` 仍须带 `@Global` / `@ThreadLocal` 且 GC-free（见 13.6），这些注解可以组合。extern 变量当前只支持 C data ABI，显式写 `abi = "scoop"` 是编译错误；Scoop ABI 只定义函数调用边界。
- **按 ABI 分类的边界类型约束**：`abi = "c"` 的 native 函数签名及 extern 变量必须满足 13.8 的 C-FFI-safe 约束，因而全部 GC-free；ref type 出现在这些边界上是编译错误。启用 `captureErrno` 时先按 13.4.2 将 Scoop 结果 `(R, Int)` 投影为 native 结果 `R`，外层 tuple 不作为 C aggregate 传递。`abi = "scoop"` 的函数复用普通 Scoop typed ABI，可以按第 14 章直接传递 managed ref，不套用 C-FFI-safe classifier。
- Scoop ABI extern的source contract非可选地携带独立`GcEffect`：未标注`@NoGC`时为`Managed=1`，显式合法标注时为`NoGc=2`。该轴只约束native callee能否进入GC/runtime/managed callback并进入ABI contract/fingerprint，与ordinary/suspend函数类型effect正交；extern仍禁止`suspend`。它不改变caller边界种类，两个值都按14.2走`NativeBorrowed`与caller-root publication，`NoGc`不能降级为普通Scoop `NoGc` callsite。
- 不支持 C varargs（`printf` 式可变参数）；需要时用 wrapper 函数绕行。
- `abi = "c"`（默认）的 extern 函数是 unsafe function，只能在 unsafe context 中调用（见 13.3）；`abi = "scoop"` 的 extern 函数例外，调用点不要求 unsafe context（见第 14 章）。
- `@CallingConvention` 用于 function，标明 calling convention（如 `cdecl` / `stdcall` 等，具体含义由实现确定）；可与 `@Extern` 组合使用，指定 FFI function 的 calling convention。

native symbol 必须非空且不含 NUL；省略 name 时使用声明名。空 lib 指默认 native namespace，非空 lib 是逻辑库名，不是路径或 linker 参数。函数、只读或可写 data、TLS、C/Scoop ABI、calling convention 和完整 native 签名都是 extern 合同的一部分；启用 errno 捕获时，native 签名按 13.4.2 投影，Scoop 声明另保留完整的 tuple 结果类型和捕获选项。

同一最终程序中，相同目标 native symbol 的声明必须具有相同合同；library、kind、TLS、可变性、ABI、calling convention、投影后的 native 参数/结果或 Scoop GC effect 的冲突均为链接错误。默认参数、参数名和透明 alias 不改变 native 签名；`captureErrno` 与 NativeSafe/GcLeaf 属于各 Scoop 声明的调用行为，不进入 native symbol 的 ABI 合并键。诊断应指出声明来源与首个不同字段；外部实现是否遵守声明仍由 FFI 作者负责。

Darwin 的目标 symbol 在逻辑名称前添加一个下划线，已有下划线不折叠；ELF 保持原 bytes。目标 ABI 与 native 产物不能跨平台或 glibc/musl 混用。完整物理 ABI、符号编码及链接规则见编译器与产物规范 2.4、2.8。

#### 13.4.1 `@GCLeaf`

```scoop
annotation class GCLeaf

@GCLeaf
@Extern(abi = "c", name = "native_max")
fun nativeMax(a: Int, b: Int): Int
```

`@GCLeaf` 为短小 C FFI 调用选择无状态切换的调用模式。GC-leaf 表示整个同步调用链不进入 Scoop GC 或 safepoint；本注解还要求避免阻塞等待，并用于预期短时完成的调用。普通 C ABI callee 不直接使用 Scoop GC，并不自动满足这份完整契约。

- 仅允许标注 `@Extern(abi = "c")` 函数；标在普通函数、Scoop ABI extern、变量、类型或其他声明上是编译错误。C ABI 已有的 top-level、non-generic、非 suspend、C-FFI-safe 和 unsafe 调用约束继续适用。`@GCLeaf` 不解除 C ABI extern 禁止 `@NoGC` 的规则。
- native 函数及其同步间接调用不得触发 Scoop GC、执行 Scoop safepoint、park、转换 Scoop 线程状态，或进入需要这些动作的 runtime API；不得回调 Scoop，包括经静态 FunPtr 或注册 trampoline 进入。可以调用满足相同约束的其他 C 函数；“leaf”不要求机器级上完全没有子调用。
- 不得主动进行可能阻塞的 I/O、mutex/condvar 等等待、join、sleep、无界忙等，或等待其他 managed 线程推进。应短时返回；“短时”不设固定纳秒或指令数阈值，编译器不插入计时检查。调用执行较慢会延迟 GC 停稳。
- 实际 native 调用保持 caller 当前线程状态，不执行 NativeSafe 进入/返回，不为本次调用建立 transition、safepoint 或 caller root frame。从活动 `MANAGED` 状态调用时，collector 等该线程之后到达正常 safepoint 才能开始扫描和移动；不能把未切换的线程视为 NativeSafe。
- 注解只作用于实际 native 调用。显式实参、缺省实参及其他调用前后表达式照常求值，保留其 GC、异常和 safepoint 行为。跨后续真正 safepoint 的引用仍须按正常规则保活和重读。
- 注解不改变 C ABI、参数/返回值的 C-FFI-safe 条件、native unwind 边界或指针有效期，也不取消已有 pin/借用义务；它不表示纯函数、无内存副作用或不需要用户数据同步。
- 外部实现是否满足无回调、无运行时重入和无阻塞等待等约束由 FFI 作者保证。违反属于 unsafe FFI 契约违例，不保证安全或必有诊断；编译器只检查声明和调用的静态规则，不证明任意 C/C++ 正文的行为。

未标注的 C ABI extern 默认使用 NativeSafe。`@GCLeaf` 是当前 Scoop 声明的调用模式，跨 Cone 使用和模板实例化必须保留；它不改变目标 native symbol 的物理 ABI。同一 C symbol 可以经普通声明或 GCLeaf 声明调用，两者仍须满足既有 ABI 一致性要求，标注方承担更强的调用契约；链接去重不能把一种调用模式传播到另一种声明。

物理 C ABI 调用路径按实现规范 2.4～2.5 选择，不增加源码注解或开关。未选择 errno 捕获的受支持标量签名直接调用 native symbol，省去 storage bridge 及其参数/结果内存往返；按值 C-layout struct 等保留桥接时仍可使用 `@GCLeaf`，其无 GC 边界开销的契约相同。显式 errno 捕获、pin/unpin 等操作仍有自身成本；`@GCLeaf` 不承诺每种 FFI 签名和选项都与直接 C 调用具有相同开销。

#### 13.4.2 `captureErrno`：随调用返回错误值

`captureErrno` 是编译期常量 Boolean，默认 `false`。只有 C ABI extern 函数可以启用；Scoop ABI extern 或 extern 变量设置 `captureErrno = true` 是编译错误。

```scoop
@Extern(name = "close", captureErrno = true)
fun closeWithErrno(fd: Int): (Int, Int)

@Extern(name = "close")
fun closeRaw(fd: Int): Int

@Unsafe
fun closeErrorCode(fd: Int): Int {
    val (result, error) = closeWithErrno(fd)
    if (result == -1) return error
    return 0
}
```

启用捕获的声明必须显式返回二元 tuple `(R, Int)`，透明 alias 展开后按相同规则检查。第一项 `R` 必须是原有 C ABI 允许的返回类型，第二项必须是 `Int`（即 `Int32`）；不接受错误元数、其他错误码类型或含 managed reference 的结果。`R = Unit` 表示 native `void`，Scoop 结果为 `((), capturedErrno)`。参数仍遵守普通 C ABI 规则，不增加源码参数，也不允许 native varargs。

| Scoop 声明 | 真实 C 函数结果 | Scoop 调用结果 |
| --- | --- | --- |
| `captureErrno = false`，返回 `R` | `R` 的 canonical C 表示；`Unit` 为 `void` | `R` |
| `captureErrno = true`，返回 `(R, Int)` | 仅 `R` 的 canonical C 表示；`Unit` 为 `void` | `(nativeResult, capturedErrno)` |

外层 tuple 是 Scoop 包装结果，不是 C 函数返回的 struct，也不扩展一般 tuple 的 C-FFI-safe 分类。`R` 为合法 C-layout struct 时仍按原 C ABI 返回该 struct；未启用捕获的普通 C extern 不能通过返回 `(R, Int)` 隐式启用捕获。

- **固定调用类型**：普通调用、解构、函数引用和泛型正文消费都看到声明中的 `(R, Int)`；不能根据接收变量数量或 expected type 选择捕获模式。忽略第二项或整个结果不会关闭声明要求的捕获。
- **捕获时机**：完成实参准备及适用的 NativeSafe 进入后，在实际 C 调用前立即将目标 libc 的 errno 清零；C 函数返回后立即将其复制到本次调用的 native 局部整数，再进行结果复制、表示转换或返回握手。M33 使用带捕获的 StorageBridge（实现规范 2.4～2.5），不会到 managed 侧再次查询 errno。未启用捕获的调用不增加清零或捕获步骤。
- **返回语义**：第二项是 C 函数返回时 errno 的快照，不自动抛异常、构造 error 对象或判断成功。失败条件由该 C API 的返回值契约决定；调用成功也可能留下非零 errno，调用失败也不自动补造错误码。若 C 函数在内部执行清理或回调而覆盖 errno，FFI 不能恢复更早的值，需由该 C 实现维护自己的返回契约。
- **值的生命周期**：结果是普通 GC-free tuple，每次调用独立。之后的分配、GC、release、其他捕获调用、嵌套调用或协程恢复不会自动覆盖已取得的值；协程保存它时沿普通局部值存入 frame，不依赖原 OS 线程。该保证只针对复制出的值，不保证 libc 的 errno 在之后仍保持不变。不提供线程级 `lastErrno()` 或对应的 Scoop runtime 槽位。
- **GCLeaf 与 release**：捕获可以和 `@GCLeaf` 组合，清零、快照和结果传递不引入 Scoop 分配、safepoint、park、线程状态切换或 TLS/thread runtime 访问。release block 可直接调用启用捕获的 extern，仍须满足 9.1.6 的全部参数、结果及 native 调用限制；不会因此允许普通 TLS 访问、带 extern 调用的 Scoop helper 或 managed 回调。
- **链接与产物**：同一 native symbol 可由返回 `R` 的普通声明和返回 `(R, Int)` 的捕获声明共同引用，只要投影后的完整 native 合同一致。捕获选项、Scoop 返回类型及适配关系随声明和调用进入 metadata、语义指纹和缓存；bridge 的生成与复用区分捕获行为及实际私有签名，native symbol 去重不能把一个声明的选项传播到另一个声明。

### 13.5 `@CLayout`

```
annotation class CLayout(val aligned: Long = 0L, val packed: Long = 0L)
```

- 用于 struct，指定该结构体内部成员的 align/pack 规范。
- `aligned` / `packed` 的缺省值 `0L` 分别表示不增加struct最小对齐以及不限制field自然对齐；非零值只能是 `1L` / `2L` / `4L` / `8L` / `16L`，其他值是编译错误。64位target profile不接受更大的显式对齐，不能截断或静默归一化。
- 带有此注解的 struct，其每个字段都必须递归具有稳定C表示、且字段的concrete type不能是ZST；因此它不能直接或间接包含任何ref type，也不能以空字段或零尺寸字段依赖C实现扩展。字段类型不依赖type parameter时在声明处检查；generic `@CLayout`字段依赖type parameter时，声明保存逐字段的`C-FFI-safe && NonZst`条件，每个fully concrete application在替换后检查。失败诊断位于该application/concretization点并同时指出原始字段路径；跨Cone导入不得提前接受、删除或重新解释该条件。无字段的`@CLayout`在声明处直接报错。

### 13.6 `@Global` / `@ThreadLocal`

```
annotation class Global
annotation class ThreadLocal
```

- 用于top-level `var`请求一个可寻址的raw global/TLS storage；两者互斥。该声明不再是9.1.3的ordinary property，不能带custom accessor/delegate，并继续只接受显式type与编译期static initializer；
- raw global/TLS必须GC-free，类型中不能直接或间接包含ref。`addressOf`只可用于这类本地raw storage或合法extern storage，不能取得ordinary property backing slot；
- ordinary top-level `val`/`var`不要求这两个annotation，可以含managed ref，由compiler-managed accessor、global root与initialization unit承载。一般成组全局状态仍建议用`object`表达；
- extern `var`仍必须用`@Global`/`@ThreadLocal`说明native symbol是否TLS并满足C-FFI-safe；extern `val`只读view不要求这两个annotation。

### 13.7 `@InteriorMutable`

```
annotation class InteriorMutable
```

- 用于 struct：标明该 struct value 内部的值可能会被改变，不能保证 immutability。常用场景：struct 带有 atomic 字段，或 FFI 会修改传给它的 struct 参数。
- 带有此注解的 struct 及其 value 只能在 unsafe context 中使用（见 13.3）。
- 该注解不为 value 引入 identity，也不改变按值复制边界：两个 copy 仍是两份独立存储。值类型成员方法仍按 3.3 取得独立的 `this` copy，不得因 ABI 指针传递而把调用方存储暴露给方法。

### 13.8 FFI ABI

Scoop 的 FFI 函数有两种 ABI：

- **C ABI**（`abi = "c"`，默认）：标准的 FFI function，由外部 lib / so / dylib / dll 提供。它对 Scoop 的类型系统和 GC 环境没有任何了解，也不能使用相关功能，用于直接引入外部库。native 参数与返回值必须是 C-FFI-safe：GC-free 且具有本章规定的稳定 C 表示。13.4.2 的捕获声明只把 tuple 第一项投影为 native 返回值，额外的 `Int` 由调用适配器产生。
- **Scoop ABI**：复用普通、非挂起 Scoop 函数的 typed machine ABI。ref 参数/返回值直接以 managed ref value 传递，value type按 Scoop 自身的 concrete ABI 传递；被调方能读取 TypeDescriptor，并可按第 14 章的 native-root 协议显式进入可能触发 GC 的 runtime 操作。它主要供 runtime 与 core 使用，不是通用 C library ABI。

C ABI callee本身不能直接接收managed ref或调用Scoop GC。未标注`@GCLeaf`时，caller按运行时规范3.5发布roots并切换到NativeSafe；C代码只有在持有14.3注册得到的静态trampoline与cookie时，才能经独立的反向边界进入managed callback，这不改变该C函数自身的参数ABI或赋予它Scoop ABI能力。标注`@GCLeaf`时，caller按13.4.1及运行时规范4.5保持原线程状态；更强的无回调、无safepoint和无阻塞等待契约使该调用不需要额外的root/transition。

Scoop ABI extern无论`GcEffect`为`Managed`还是`NoGc`，caller都发布可更新roots并切换到`NativeBorrowed`，返回后按epoch协议reload；effect只约束callee contract并参与native contract fingerprint。`NoGc`不得改写成无需root/transition的普通Scoop NoGC call，`@GCLeaf`也不适用于Scoop ABI。具体机器级序列由实现决定，但不得改变上述类型与GC契约（第14章）。

C没有跨当前支持profile可依赖的零尺寸object ABI。C-FFI-safe classifier因此只允许`Unit`作为函数返回并映射为C `void`；ZST不能作为C参数、callback参数、extern global/TLS或by-value result，也不能成为`@CLayout`字段。无字段的`@CLayout`、不依赖type parameter且已知含ZST/最终size为0的声明在声明处报错；依赖type parameter的字段按13.5在每个fully concrete application的concretization点检查，不能等到C bridge或native linker才失败。`Ptr<Unit>`仍是显式的`void *`/opaque handle例外；其他`Ptr<ZST>`可在Scoop unsafe代码中使用，但不能凭“pointer本身有C表示”自动获得一个不存在的C pointee object type。

ref type（如 `String`、`Array`、普通 class）不能出现在 C ABI 的边界上（13.4 的 C-FFI-safe 约束）；同一类型可以直接出现在 Scoop ABI extern 签名中。`PinnedPtr<T>` / `GcHandle<T>` 已是 GC-free 的显式边界值：它们适合 C ABI、跨调用保活或需要稳定裸地址的场景，不是 Scoop ABI direct-ref 调用的必经表示。

定宽integer在C ABI参数、返回、extern global/TLS及`@CLayout`字段中精确映射：`Int8`/`Int16`/`Int`/`Long`（即`Int8`/`Int16`/`Int32`/`Int64`）分别为`int8_t`/`int16_t`/`int32_t`/`int64_t`，`UInt8`/`UInt16`/`UInt`/`ULong`（即`UInt8`/`UInt16`/`UInt32`/`UInt64`）分别为`uint8_t`/`uint16_t`/`uint32_t`/`uint64_t`。transparent alias先展开；不依据Scoop拼写把它们推断为C的`int`/`long`/`intptr_t`。直接 C extern 调用按同一 canonical C signature 和目标 C ABI 确定窄 integer 实参/返回的符号或零扩展，并将完整计划传入 LIR/codegen；保留的 aggregate、callback 等 bridge 继续由所选 C toolchain 编译 canonical `stdint.h` source signature。两种路径须与目标 C ABI 一致，不能假定 Scoop typed ABI 恰好相同；物理调用计划与验收规则见实现规范 2.4～2.5。

`Float`/`Double`在C ABI参数、返回、extern global/TLS、`@CLayout`字段及相应pointer/callback签名中分别使用C `float`/`double`。两种alias先展开；Scoop的每个binary32/binary64位型均有效，FFI入站不拒绝NaN、Infinity或负零。满足直接调用条件的标量 C extern 按目标 C ABI 传递和返回 binary32/binary64；混合 aggregate 与 callback 等保留的 bridge 继续将同一 canonical C storage signature 交给目标 C compiler 分类。两条路径不得降低精度或改变位型有效性。Scoop 值布局与 GC 规则见运行时规范 6.1，直接调用见实现规范 2.4～2.5，浮点实现边界见 2.18。

### 13.9 `value` / `ref` 类型约束

`value` 与 `ref` 是上下文关键字，用作泛型的 type bound：

- `T : value`：`T` 必须是值类型（struct / enum / tuple / 基本类型）；
- `T : ref`：`T` 必须是引用类型（class / interface）。

```
fun <T : value> needValue(v: T) { ... }

needValue("hello")    // 编译错误：String 不是值类型
```

- 与类型上界语法同样可用于 `where` 子句。
- class/interface类型上界可直接写在参数上，也可写在声明头之后的`where`子句；同一参数至多有一个class上界，并可有多个不同interface上界。上界必须是参数完整的exact reference application，例如`T : Base<String>`或`where T : Producer<Animal>`；不接受value type、函数类型、`Any`或另一type parameter。class上界已蕴含`ref`，并把class成员及其继承闭包加入bounded receiver能力。
- `value` / `ref`约束与任一class/interface上界互斥：同一类型参数不能同时携带二者；同一个kind bound也不能重复出现在inline与`where`位置。多个class上界即使文本上存在继承关系也不允许，必须保留唯一class bound并把其余能力写成interface bound。
- 无约束的类型参数默认接受任何类型（与 Kotlin 一致）。

### 13.10 `Ptr` 与 `FunPtr`

二者定义于 `scoop.core`，是 FFI 的基础辅助类型，均为值类型。

受支持目标的 data pointer 与 code pointer 均为 64 位，null 的内部 carrier 为全零，合法非 null 地址可逐 bit 往返。`Ptr<T>(raw: ULong)` 是显式 unsafe 构造；FunPtr 没有整数构造或转换。这不产生隐式 pointer/integer 转换，也不把固定宽度 ULong 定义为 platform-native integer。不满足该表示契约的目标必须拒绝。

#### `Ptr<T>`

`Ptr` 能容纳一个 raw pointer；`addressOf` 用于取一个变量的内存地址：

```
@Intrinsic("core_ptr")
public struct Ptr<T : value> : Equality<Ptr<T>> {
    public override operator fun equals(other: Ptr<T>): Boolean {
        @Unsafe {
            return this.toULong() == other.toULong()
        }
    }

    @NoGC @Unsafe
    @Intrinsic("ptr_to_ulong")
    public fun toULong(): ULong

    @NoGC @Unsafe
    @Intrinsic("ptr_cast")
    public fun <U : value> cast(): Ptr<U>

    @NoGC @Unsafe
    @Intrinsic("ptr_load")
    public fun load(): T

    @NoGC @Unsafe
    @Intrinsic("ptr_load_offset")
    public fun load(offset: Long): T

    @NoGC @Unsafe
    @Intrinsic("ptr_store")
    public fun store(value: T)

    @NoGC @Unsafe
    @Intrinsic("ptr_store_offset")
    public fun store(offset: Long, value: T)

    @NoGC @Unsafe
    @Intrinsic("ptr_plus")
    public operator fun plus(offset: Long): Ptr<T>

    @NoGC @Unsafe
    @Intrinsic("ptr_minus")
    public operator fun minus(offset: Long): Ptr<T>
}

@Unsafe
@Intrinsic("address_of")
public fun <T : value> addressOf(v: T): Ptr<T>
```

- `core_ptr`登记为一个以exact GC-free value pointee type参数化、以data pointer表示的compiler-represented value family；它没有源码可见field或普通primary constructor，不能被字段访问、解构或copy update。源码`T : value`只表达kind，不能证明GC-free：每个concrete `Ptr<T>` application还必须递归证明`T`为GC-free；generic template中出现`Ptr<T>`时保存并向实例化者传播该typed deferred条件，所有type argument具体化后失败即为编译错误。`equals`是普通core body；其余上述方法均为compiler intrinsic且都是unsafe function（见13.3）。按13.1的规则`@Intrinsic`通常不得与其他注解共存；此处是单独说明的例外：这些intrinsic允许与`@NoGC`/`@Unsafe`组合。
- registry只为显式`Ptr<T>(raw: ULong)`提供一个call-shaped、`@NoGC @Unsafe`的特殊construction entry；它不是普通struct constructor，也不能由representation field合成。该entry是从integer显式制造data pointer的唯一形态，要求unsafe context及`raw != 0uL`前置条件。编译期常量零直接诊断；运行期值违反该unsafe前置条件时行为未定义。它不建立隐式conversion，未对齐、越界、悬垂地址、算术结果变为零及生命周期均由unsafe调用者负责。
- 该 construction entry 由普通名称查找得到的实际 `core_ptr` 类型声明提供，声明来自当前 Cone 或依赖时规则相同；导入别名保留该绑定，普通同名声明按既有遮蔽与重载规则处理。`T` 可显式给出，也可由期望的 `Ptr<T>` 类型推断（包括 `_`）；指向完整 `Ptr<T>` application 的非泛型 typealias 固定其类型实参，并作为非参数化候选参与选择。唯一值参数是 `raw: ULong`，允许位置或命名传参，不接受 spread、默认值或隐式整数转换。泛型正文及默认值保留显式构造与 pointee 类型，按既有规则向实例化者传播 GC-free 条件。
- unsafe context 与编译期常量非零检查对已选 construction entry 生效；不得因这些检查失败而跳过更具体的构造候选，转而调用同名的普通函数。未选候选不产生上述诊断或求值。
- 源码中的 `pointer + offset` / `pointer - offset` 分别按 `ptr_plus` / `ptr_minus` 的契约处理；`offset: Long` 以**元素个数**计，其数学byte displacement为`offset`与`sizeOf<T>()`数值的乘积，与非ZST C object pointer算术一致。带 `offset` 的 `load` / `store` 使用相同的元素偏移语义。对ZST pointee，任意offset的物理byte displacement恒为0且所得pointer bit值不变；`load`产生该exact ZST值，`store`不写payload byte，但receiver、offset、value仍求值，且unsafe调用者仍必须保证pointer非null、满足alignment/lifetime并指向相应逻辑place。算法不得用ZST pointer值变化表达迭代进度；`Ptr<Unit>`若需要逐byte移动必须先使用语义上正确的`Ptr<UInt8>`，不能把opaque `void *`自动当byte pointer；
- `addressOf` 是 intrinsic，且带 **lvalue 约束**：实参必须是参数、局部变量、全局变量，或值类型成员方法的 `this`，取的是该 place 实际存储的地址；对临时值、字面量、计算结果等非 lvalue 表达式调用是编译错误。对 `this` 取址时指向 3.3 规定的方法局部副本，不是调用方的 value 或 box payload。
- 泛型正文中的 `addressOf` 保留原 place 与 exact pointee type；尚未具体化的值参数按 `Ptr<T>` 的既有规则传播 GC-free 条件，不要求在泛型定义处得到具体布局。经过普通名称查找和重载选择后才应用这一 intrinsic 规则；命名参数和导入别名不改变取址对象，不得取为普通按值调用准备的临时副本。
- `Ptr<T>` 自身是值类型，因此满足 `value` 约束，可以出现在要求 `T : value` 的位置（包括 `Ptr<Ptr<T>>`）。
- **null与可空指针**：裸`Ptr<T>`没有null值，内部data-pointer carrier的全零位模式保留给`Option<Ptr<T>>.None`及inactive/zeroed storage。FFI边界上的可空data pointer必须用`Option<Ptr<T>>`表示；声明返回裸`Ptr<T>`的native函数返回null属于契约违反。布局由niche保证（见7.4）。
- **`void*` 与 opaque 类型**：`void*` 及 C 的 opaque handle（不完全类型指针）统一用 `Ptr<Unit>` 表示。

#### `sizeOf` / `alignOf`

```
@NoGC
@Intrinsic("size_of")
public fun <T : value> sizeOf(): ULong

@NoGC
@Intrinsic("align_of")
public fun <T : value> alignOf(): ULong
```

- 返回 `T` 的大小 / 对齐（字节数），编译期求值；ZST返回size 0和严格大于0的alignment。手工内存管理（配合 C 的 `malloc` / `free` 等）时不能把`malloc(0)`结果当成可取址ZST place，需按4.7自行提供至少1 byte且满足alignment的token。
- `T : value` 是布局查询的类型约束；含引用字段的值类型也可查询布局，不增加 GC-free 条件。泛型正文及默认值保留被查询的类型，单态化后按具体类型的实际布局求值。声明来自当前 Cone 或普通依赖时遵守相同规则。

#### `FunPtr<F>`

`FunPtr<F>` 声明一个 FFI 可用的 callback 指针，`F` 是 8.1 定义的**普通、非挂起且完全具体化**的函数类型：

```
@Intrinsic("core_fun_ptr")
public struct FunPtr<F>
```

- `core_fun_ptr`登记为一个由exact函数类型参数化、以code pointer表示的compiler-represented value family；它没有源码可见field、primary representation constructor或integer accessor。不能由零字段普通struct规则推出zero-size layout或任意构造能力。
- `core_fun_ptr`不提供任何源码constructor；`FunPtr<F>()`、`FunPtr<F>(0u)`与`FunPtr<F>(123u)`都没有合法candidate。裸`FunPtr<F>`始终非null；需要null或可选字段时使用`Option<FunPtr<F>>`并以`None`初始化。这保证7.4中的全零niche不是合法payload，`Some(value)`与`None`始终可区分。
- 用户不能直接构造`FunPtr`值；在期望类型明确为`FunPtr<F>`的位置使用顶层函数声明引用`::name`，由编译器直接生成非null原生callback地址。`::name`是8.1.4的中性源码语法，不具有固有的`FunPtr`类型；因此`val callback = ::name`仍推导为managed函数值，要保存原生地址必须由类型标注、参数类型、返回类型等上下文提供`FunPtr<F>`期望类型。目标声明签名必须与`F`**精确相同**，不应用8.1.1的函数类型型变。14.3的`ForeignCallback.function`是编译器生成的另一种非null值，指向必须与opaque context cookie配对使用的managed-callback trampoline；它只能从已注册值读取，不能用构造器仿造。声明返回裸`FunPtr<F>`的native函数返回null同样属于契约违反。
- `F` 的每个参数与返回类型必须满足 C-FFI-safe 约束；`Unit` 只允许作为返回类型。`FunPtr` 描述的是 C ABI callback 地址，不是 Scoop ABI managed callable；函数类型 `F` 在这里仅描述 native signature，本身不会作为 managed 引用穿越边界。
- 可空函数指针用 `Option<FunPtr<F>>` 表示（niche 优化见 7.4）。
- native-address resolution 的目标必须是带 `@NoGC` 的普通顶层命名函数，且**不能是 generic、挂起、extern、成员或扩展函数**。lambda、匿名函数、局部函数、任何绑定引用以及已存在的 managed 函数值都不能作为非 null `FunPtr` 的来源。违反这些约束是编译错误：FFI 回调不得与 GC 交互，generic 函数没有单一具体符号，挂起函数只有编译器内部的 hidden continuation ABI，而 closure 还需要原生 ABI 中不存在的 managed 环境参数。编译器不自动生成 closure 或挂起 callback wrapper。
- `FunPtr` 不提供 Scoop 侧 `invoke`；它只用于传递/存储 native callback 地址。由`::name`得到的静态callback仅允许原生方在发起extern调用的同一已注册线程上同步调用；保存后异步、跨线程或在Scoop程序退出后调用不隐式获得安全性。只有14.3 registration返回的`ForeignCallback.function + context`配对具备对应token/attach协议，单独保存或调用其中的function而不携带仍存活的配对context同样非法。

```
// C 侧：int64_t compare_int(int64_t a, int64_t b,
//                         int64_t (*cmp)(int64_t, int64_t))

@Extern(lib = "sample", name = "compare_int")
fun compareLong(a: Long, b: Long, cmp: FunPtr<(Long, Long) -> Long>): Long

@NoGC
fun cmp(a: Long, b: Long) = if (a > b) { 1L } else { 0L }

@Unsafe
fun caller() {
    compareLong(10L, 10L, ::cmp)
}
```

---

### 13.11 有作用域的数据指针借用

core 提供以下普通、非 suspend 的 unsafe 扩展函数：

```scoop
@Unsafe public fun <T : value, R> MutableArray<T>.withDataPointer(block: (Ptr<T>, Long) -> R): R
@Unsafe public fun <T : value, R> Array<T>.withDataPointer(block: (Ptr<T>, Long) -> R): R
@Unsafe public fun <R> String.withUtf8Bytes(block: (Ptr<UInt8>, Long) -> R): R
```

`T` 必须满足 `Ptr<T>` 的 GC-free 值类型约束。receiver 与 block 按普通调用规则各求值一次，随后同步调用 block，传入连续元素区指针及元素数量；String 传入 UTF-8 字节区及字节数，不包含终止零字节，也不保证字节区后存在零字节。返回值是 block 的完整返回值，异常沿普通异常路径传播。

对象在 block 执行期间保活且地址固定，包括 block 内部发生 GC 或进入 NativeSafe 的期间；正常返回或抛异常均结束借用。不同线程、嵌套调用可借用同一对象，各次借用独立结束。指针只在对应 block 内有效，保存并在借用结束后使用属于 unsafe 契约违规，不做额外的运行期逃逸检查。不可变 Array 与 String 的指针只能读取。借用不提供独占访问或数据同步，并发访问仍须遵守普通线程规则。零长度借用也返回非零、可比较但不可解引用的指针；零大小元素沿普通 `Ptr<T>` 规则操作。

数据区位置由实际目标布局确定，不允许库代码从对象地址加硬编码偏移。实现使用运行时规范 3.4 的线程局部 pin 帧，包含 immortal String literal；它与 14.1 的显式计数 pin 各自配对。

core 内部的三个 top-level borrow intrinsic 分别接收对应对象和上述 callback，必须标注 `@Unsafe`；由于 callback 可以分配和抛异常，不得标注 `@NoGC`。公开扩展函数通过普通 core 正文调用这些 intrinsic，不改变 13.1 的 annotation target 规则。

## 14. Scoop ABI FFI

Scoop ABI（见 13.8）供能识别 Scoop 类型信息并与 GC 交互的外部函数使用，主要消费者是 runtime 与核心库的实现者。

### 14.1 GC 基础设施 `scoop.core.gc`

```
package scoop.core.gc

struct PinnedPtr<T : ref>(val raw: ULong)
struct GcHandle<T : ref>(val raw: ULong)

@Unsafe fun <T : ref> pin(v: T): PinnedPtr<T>
@Unsafe fun <T : ref> unpin(p: PinnedPtr<T>): T
@Unsafe fun <T : ref> getGcHandle(v: T): GcHandle<T>
@Unsafe fun <T : ref> releaseGcHandle(h: GcHandle<T>): T
```

- **`pin`**：将对象固定在 GC 堆上（不移动、不回收），返回 `PinnedPtr`——其 `raw` 就是对象的实际地址，pin 标志记录在对象头（见 运行时规范 3.4），可以直接交给 FFI 当裸指针使用。同一对象可重复或由多个线程 pin，每次成功调用增加一次固定计数；复制 `PinnedPtr` 值不增加计数。固定对象会影响 GC 效率且可能造成内存泄漏，固定时间应尽可能短。
- **`unpin`**：按地址减少一次固定计数并取回对象，O(1)；只有计数归零才解除固定。每次 pin 必须恰好配对一次 unpin，不能因复制了 `PinnedPtr` 而额外 unpin。计数与固定地址不提供对对象内容的线程同步。
- **`getGcHandle`**：获取对象的 GC handle。handle 被视为对象的引用：对象存在未释放的 handle 时不会被回收，但 GC 可能在堆上移动它。一个对象可同时存在多个 handle，全部释放后才可能被回收。
- **`releaseGcHandle`**：释放 handle 并取回对象，不再阻止回收。
- `PinnedPtr` 与 `GcHandle` 是不同的类型，混用（如 `unpin` 一个 `GcHandle`）是编译错误。二者都是只含一个 `ULong`（即`UInt64`）字段的 GC-free 值类型，C ABI 与 `ULong` 一致，可以直接出现在 C ABI 签名中（13.4 的 C-FFI-safe 约束）。这一透明 C 表示不改变 Scoop typed ABI：二者仍按实际声明字段形成普通非空 struct，参数与返回按 14.2 使用 indirect aggregate；不能因 core 身份或 C 表示将其变成 Scoop 标量。
- 取舍：短期持有并需要裸指针时用 `pin`（摊还 O(1)，但阻碍 GC 移动）；长期保活且允许移动时用 `GcHandle`。
- Scoop ABI extern 的同步调用期间若只借用 direct ref，调用方不需要显式 pin 或 handle；被调方需要跨 safepoint或调用结束保存引用时才使用 14.3 的 native root、pin 或 handle机制。
- handle 取回对象时类型 `T` 来自 handle 的类型参数，编译器无法校验其真实性——这层正确性由 runtime 作者保证。

### 14.2 调用约定

Scoop ABI extern 调用是同步借用边界：

- 调用前保活所有跨调用存活的 managed 引用、direct-ref 实参及含引用的结果存储；发生 GC 后，调用者使用更新后的引用；
- 调用期间线程遵守 NativeBorrowed 协议，不能按普通 C ABI 的 NativeSafe 处理；callee 跨 safepoint 保存引用时遵守 14.3；
- native 返回含引用的值时，该值必须在重新允许 GC 前成为有效 root；
- `@NoGC` 只限制 callee 的 GC 行为，不取消 caller 的线程协调和保活义务；
- machine calling convention 为目标 C 调用约定，物理参数和结果遵守编译器与产物规范 2.4；
- 调用点不要求 unsafe context，除非声明显式带 `@Unsafe`；
- native callee 不得把 Scoop 异常展开回 managed caller；违反时终止进程。源码可见失败由 managed wrapper 在 native 返回后抛出。

Scoop ABI extern 的参数与返回值使用普通 Scoop typed ABI，不经过 C ABI storage bridge：ref value 是直接 managed pointer；aggregate/value return沿用普通 Scoop 函数的 typed return storage规则。Darwin/AArch64 与 Linux/amd64 profile 中，scalar、ref/raw pointer/function pointer 与 niche enum 直接传递；所有非空 tuple、ordinary struct、tagged enum 与异常记录都通过 caller-owned、按 exact layout 对齐的间接 storage 传递，间接返回 storage 位于所有源码参数之前；Unit 返回为 machine void，其他 ZST 参数/结果只保留 typed identity而不传payload。被调方必须按同一 physical signature 实现该 ABI，不能把同形 C struct 的按值参数/返回直接用作 shim，也不能假设 C 编译器会为它选择与 Scoop value ABI 相同的寄存器或栈位置。

这里的 Scoop ABI 仍是普通、单次进入并在返回前完成的 FFI 调用约定，不是 8.2 所述挂起函数的 hidden continuation ABI。`abi = "scoop"` 不放宽 `@Extern` 与 `suspend` 的互斥规则，也不提供自动 continuation / callback wrapper。

### 14.3 direct ref、native root 与 safepoint

本节的 root、pin、handle 和 safepoint 规则适用于 minor 与 full GC。Scoop ABI native 实现向 managed heap 写入引用时，必须遵守运行时规范 3.6 的写屏障；登记 native root 或 pin 不能替代代间引用记录。普通 C ABI 不接收 managed ref；显式 `gc.collect()` 请求 full collection。

- Scoop ABI FFI 函数的机器码中**没有 safepoint poll**（它可能是用其他语言写的）。传入的 direct ref 是调用期间的借用 managed value：在被调方尚未执行可能触发 GC 的 runtime 调用或回调 Scoop 代码前，可以直接读取，无需 pin；不得写入长期存储或在返回后继续使用。
- 若被调方需要让某个 direct ref 跨越可能触发 GC 的操作，必须先把它写入可寻址的 **native root slot** 并把对应 root frame登记到当前线程。含managed leaf的内联aggregate/value place不能被拆成登记后仍从旧aggregate读取的临时ref；它使用`RecursiveRegion { stable base, byte extent, NonEmptyRefScan }` frame，由collector以与TypeDescriptor相同的递归slot visitor原地更新。登记/移除两种root frame本身都不得分配或触发GC；操作返回后，被调方必须从slot或region base重新读取，不能继续使用登记前保存的裸指针/aggregate副本。
- root frame 必须按栈严格嵌套，并在所有正常/错误出口移除。需要把引用保存到本次调用之后时，使用 `GcHandle`；需要把稳定裸地址交给 C ABI 或跨 safepoint保持同一地址时，使用 `PinnedPtr`。两者都不是普通 direct-ref 参数的默认表示。
- GC 只在 managed safepoint 或显式 runtime 入口协调线程，不得在 Scoop ABI native code 的任意普通指令之间无握手地移动对象。
- `abi = "scoop"` 的调用点安全的前提是**被调方遵守上述契约**。这类底层 extern 声明按约定仅由 runtime / 核心库作者使用；违反借用、root frame或保活规则是 runtime ABI 错误。
- managed callback 注册接收 ordinary、非挂起 closure，返回 GC-free opaque token；token 保活 closure，native 代码不得保存其裸地址。泛型注册保留各实际 application 的完整函数签名。
- native 通过静态 C ABI trampoline 和显式 context/user-data 参数携带 token。callback 入口为未注册 foreign thread 建立线程状态，进入 managed 执行并调用实际 closure，返回时恢复原线程状态。参数与结果必须满足 C-FFI-safe；closure 内可正常分配及 GC。
- token 遵守显式 retain/release；callback 异常在反向边界内转换为状态和受管异常，不穿越 C frame。已释放 token 或 runtime 终止后调用属于 ABI 错误。native API 必须提供显式 context/user-data 参数。
- `FunPtr` 的静态 callback 仍仅允许同步、同线程、NoGC 调用。允许 foreign thread 和 managed closure 的注册协议不开放 suspend callback 或 suspend FFI。

注册成功前捕获当前有效 Context binding 的不可变快照；retain 共享快照。每次 invocation 从快照建立独立 TaskContext，返回或异常转换后恢复原 Context；Reusable 并发调用不共享可变绑定。全部 ownership 与活动调用结束后释放 closure、快照和 failure 引用。同步 native 往返保持外层 Context，反向 callback 使用注册时快照（8.3.5）。

core 提供以下注册接口：

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
    contextIndex: Long,
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
- `callback` 的实际类型必须是从 `F` 删除 `contextIndex` 对应参数后得到的 ordinary concrete 函数类型，返回类型保持不变。声明中的 `Any` 不放宽此要求，也不引入装箱；context cookie 不作为实参传给 closure；
- 泛型正文中的注册在每个实际 application 替换类型参数后检查完整签名；不同 application 不能以首次实例的类型代替。
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

## 15. 明确排除

以下内容不属于 Scoop：

对象不可达时不会执行 managed `finalize`、析构或允许对象复活的回调。native resource 应显式 close/release，并用 try/finally 保证确定性清理；9.1.6 的 release block 只提供同步、GC-free、非及时的兜底释放。

| 排除项 | 替代方案 |
|---|---|
| `enum class` | `enum`（4.2） |
| `value class` / `inline class` | `struct`（4.1） |
| `data class` | `struct`（4.1）+ 内建解构（4.6） |
| `null` / 平台类型 `T!` | `Option<T>`（第 7 章） |
| `lateinit` / 隐藏未初始化property状态 | `var p: T?` / `var p: Option<T>`（省略initializer即为`None`，9.1.1） |
| 省略visibility即public | 省略visibility即internal；对外API显式写`public`（9.1.5） |
| 普通字符串的 `$` 插值 | f-string（第 6 章） |
| struct 字面量 `S { f: v }` | 构造函数 / 命名参数（4.1.1） |
| `expect` / `actual` | 平台差异由 native 层与按 target 的源码选择承担（12.2.2） |
| JVM 互操作注解与 SAM 转换 | 无 |
| 运行期反射 | 编译期/单态化机制 |
| GC finalizer、析构回调与对象复活 | 显式 `release` / `close` + `try/finally`；受限GC-free `release { ... }`仅作兜底（9.1.6） |
| struct 的 `init` 块 / `var` 字段 | 构造函数内逻辑 / `val` |
| 对值类型使用 `===` | `==`（结构相等） |
| generic declaration/use-site `in`、`out`、`*` | nominal generic application始终invariant；使用带bound的generic callable或显式非generic接口（见3.2） |
| `Option<T>` 的智能转换 | `when` 解构（7.3） |
