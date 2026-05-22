# Overload Resolution 设计

本文档来源于 2026-05-17 ~ 2026-05-22 的设计讨论，定义 Scoop 函数 / 方法 / 构造器重载（overload）的解析规则。目的是在修复当前 overload 相关 codegen bug 之前，先把"正确行为"钉死。

本文档只描述设计与规则，**不实现任何代码改动**。

Generic overloading（同名候选中至少一个含 type parameter）**有限支持**：仅允许"参数 shape 相同、仅 type parameter bound 不同"的形式，用于支持有限的"按 bound 偏特化"（partial specialization）。其它形式（TP 一致性约束、TP 数量不同等）仍在定义点 reject。详见 §4.2 与 §6.4。

虚方法（`open` / `abstract` / `override` / interface 方法）不可引入方法级 TP——理由与 C++ 不允许 `virtual template` 相同：Scoop 走 monomorphization，virtual generic 会让 vtable 槽位无法固定。详见 §4.5。

---

## 1. 背景：当前实现的偏离

Scoop 现状下的 overload 行为可由以下小程序触发：

### 1.1 Concrete-overload 串扰（codegen bug）

```scoop
package overload_concrete_bug

import scoop.core.*

fun f(x: Int): Int {
    return x + 1
}

fun f(x: Bool): Bool {
    return !x
}

fun main(): Int {
    return f(10)   // 只调 Int 版，Bool 版未被引用
}
```

当前行为：typecheck 放过；codegen 阶段报 `pure assignment local0 store failed: value_ty=Int target_ty=Bool: unsupported value coercion from Int to Bool`——`f(x: Bool)` 函数体内的 `x` 在 lower 阶段被 mistype 为 Int。**即便 main 不调用 Bool 版**也会触发，说明问题在 lowering 期同名函数符号 disambiguation，不在调用点。

期望行为：本应在 typecheck Phase D（specificity）选出 `f(Int)` 后正常 codegen；`f(Bool)` 函数体应当独立 lower。

### 1.2 Arity-overload 同款

```scoop
package overload_arity_bug

import scoop.core.*

fun g(x: Int): Int {
    return x + 1
}

fun g(x: Int, y: Int): Int {
    return x + y
}

fun main(): Int {
    return g(10) + g(2, 3)
}
```

当前行为：typecheck 放过；codegen 阶段报 `LLVM ABI query 缺少 callable 'overload_arity_bug.g' 的 published callable version`。

期望行为：两次调用按 arity 区分到不同 callable，分别 lower。

### 1.3 Generic + Concrete 同名（按 specificity 解析）

```scoop
package overload_gvc_ok

import scoop.core.*

fun h<T>(x: T): T {
    return x
}

fun h(x: Int): Int {
    return x + 100
}

fun main(): Int {
    return h(10)
}
```

当前行为：typecheck 报 `scoop::typecheck::ambiguous_overload`。

期望行为：两候选都适用；按 §6.4 的 effective type specificity，`h(Int)` 的有效类型是 `Int`、`h<T>(T)` 的有效类型是 T 的 bound（无 bound → `Any`），`Int <: Any` 严格更具体 → 选 `h(Int)`，返回 110。这是 §4.2 放宽后的典型场景。

---

## 2. 全局决议汇总

下列决议是本文档其他章节的前提。每条都在某节展开。

| 决议项 | 选择 | 章节 |
|---|---|---|
| Effect row 是否参与 overload signature | **不参与**（视为返回 / 输出的一部分） | §9 |
| 隐式 cast / widening | **无**（applicability 仅按 nominal subtype + function 子类型化等明示规则） | §6.1 |
| Constructor 与 fun overload 的关系 | 共用一套机制（实现可分可合，spec 视作同一） | §8.3 |
| Member method 的 receiver | 在 specificity 比较中**算第 0 个参数位** | §7.3 |
| Member method 可见性 | 是 applicability 之前的筛选条件 | §7.4 |
| Override / overload 区分 | 父类 method 必须 `open` 才能被覆盖；子类覆盖必须 `override`（详见 §7.1） | §7.1 |
| Resolution 是 static 还是 dynamic | **Static**——选哪个签名在 compile-time 决定；virtual dispatch 在选定之后再于 runtime 进行 | §7.2 |
| Lambda 参数 overload | 走普通 subtype-based specificity，不加额外 reject | §8.1 |
| Vararg 与非 vararg 同名重叠 | **定义点** reject（不放到 call site） | §8.2 |
| Generic overload（同 shape 仅 bound 不同） | 允许；按 effective type 参与 specificity | §4.2、§6.4 |
| Generic overload（参数 shape 不同 / TP 一致性约束） | **定义点** reject | §4.2 |
| 虚方法引入方法级 TP | **定义点** reject（vtable 不可定槽） | §4.5 |
| Specificity 算法 | A 更具体 iff ∀i: `A.eff_i <: B.eff_i` 且至少一处严格 | §6 |
| Scope 层叠 | local → member → extension → top-level → imported；外层完全 shadow | §5.1 |
| 歧义错误必须列出所有适用候选 | 是，含位置与不可比原因 | §10 |

---

## 3. 术语

- **候选 (candidate)**：可见且尚未筛除的某个具名 callable。
- **签名 (signature)**：参数列表（参数类型 × arity）；**不包含**返回类型与 effect row。
- **签名等价**：参数列表完全相同；type alias 透明（`typealias I = Int` 后 `f(x: Int)` 与 `f(x: I)` 算等价）。
- **applicability**：候选签名能否接收当前调用的实参（参见 §5.3）。
- **specificity**：应用候选间的"更具体"偏序关系（参见 §6）。

---

## 4. Definition-time 规则

定义点（即每个 fun / constructor / method 声明被处理时）必须做的检查。这些检查与调用点 resolution 解耦，目的是让"无论怎么调用都不可能合法"的重定义在第一时间就被拒。

### 4.1 签名等价 → conflicting overloads

同 scope 内同名候选签名等价时，frontend reject：

```scoop
// 反例：两个都接收 (Int)
fun f(x: Int): String { ... }
fun f(x: Int): Int { ... }   // ← scoop::typecheck::conflicting_overloads
```

错误信息列出冲突双方的位置。

注意：返回类型不同 / effect row 不同**不算**区分——两者都不参与 signature。

**Type parameter 与 alpha 等价**：TP binder 的命名不参与 signature——alpha 等价的 TP 视为同一签名：

```scoop
// 反例：alpha 等价
fun f<T>(x: T): T { ... }
fun f<U>(x: U): U { ... }    // ← conflicting_overloads
```

更进一步，签名等价检查使用 §6.4 定义的 **effective type**——两个候选每个参数位的 effective type 都相同，则视为等价。这覆盖以下情况：

```scoop
// 反例：effective types 都是 (Any, Any)，无 call site 能区分
fun f<T>(x: T, y: T) { ... }
fun f<T, U>(x: T, y: U) { ... }   // ← conflicting_overloads

// 反例：bound 默认 Any，effective 与上一行的 <T> 等价
fun g<T: Any>(x: T) { ... }
fun g<T>(x: T) { ... }            // ← conflicting_overloads
```

### 4.2 Generic overload → 仅允许"同 shape 仅 bound 不同"

同 scope 同名候选含 type parameter 时，定义点按以下规则区分：

#### 4.2.1 允许：参数 shape 相同、仅 type parameter 的 bound 不同

```scoop
// ✓ 合法：同 shape，bound 不同；按 §6.4 specificity 解析
fun debugPrint<T>(x: T) = println(x.toString())            // 兜底
fun debugPrint<T: Debug>(x: T) = println(x.debugString())  // 特化

// ✓ 合法：concrete 与 generic 同 shape；concrete 是 bound 的"最紧"形式
fun h<T>(x: T): T { ... }
fun h(x: Int): Int { ... }
```

"参数 shape 相同"指：arity 相同；每个参数位的类型表达式在抽出 TP-bound 替换后形状一致。形式化：把每个参数类型中出现的方法级 TP `T` 替换为 `T` 的 declared bound（无 bound 视为 `Any`），得到 effective parameter type；两候选 effective 形状仅在 bound 紧度上不同 → 算同 shape。

#### 4.2.2 Reject：参数 shape 不同

```scoop
// ✗ TP 一致性约束（前者要求两参数同类型，后者无此要求）
fun f<T>(x: T, y: T) { ... }
fun f<T, U>(x: T, y: U) { ... }   // ← scoop::typecheck::generic_overload_shape_mismatch

// ✗ TP 出现的嵌套位置不同
fun g<T>(x: List<T>) { ... }
fun g<T>(x: T) { ... }            // ← 同上（但若 effective 类型一为 List<Any>、一为 Any，
                                  //   可正常 specificity 区分；本例 reject 仅当两者
                                  //   shape 在 §4.2.1 意义下既非"仅 bound 不同"又非真正不同 arity 时）
```

错误码 `scoop::typecheck::generic_overload_shape_mismatch`。错误信息提示"only differ-by-bound generic overloads are supported; rename the function or restructure"。

注意：**"参数 shape 不同"与 §4.1 "签名等价"是互斥分支**——§4.1 拦截 effective type 完全相同的情形（含 alpha 等价、`<T>` 与 `<T: Any>` 等价），§4.2.2 拦截 shape 不可对齐的情形。介于两者之间（同 shape、bound 不同）的才放过到调用点。

#### 4.2.3 Reject：bound 不可比的同 shape 重载，仍合法

bound 之间不可比（如 `T: Comparable` vs `T: Numeric`）**不在定义点 reject**——这种重载本身合法，是否产生歧义留给调用点的 §6.4 specificity 决定（实参实现两 bound 之一时无歧义；都实现时报 `ambiguous_overload`）。

#### 4.2.4 与虚方法的交互

虚方法本就不能引入方法级 TP（§4.5），所以本节的 generic overload 仅适用于：top-level fun、final 成员方法、extension fun、constructor。详见 §6.4 末尾的可适用范围表。

### 4.3 Vararg 与非 vararg 重叠 → 定义点 reject

同名候选中，若一个候选的 vararg 形式可以"cover"另一个候选的 arity，则定义点 reject：

```scoop
// 反例：vararg 0/1 元 cover 非 vararg 的 (Int)
fun a(x: Int): Int { ... }
fun a(xs: Int*): Int { ... }   // ← scoop::typecheck::vararg_overlaps_non_vararg
```

形式化："cover" 指在某个调用 arity 下两候选都适用。具体规则：若非 vararg 候选的 arity 落在 vararg 候选可接受的 arity 范围内（vararg 形式总能 0/1/2/... 元），且对应位置类型兼容，即视为重叠。

### 4.4 Override 边界（仅 method）

#### 4.4.1 父类 method 非 `open`，子类同 signature → reject

```scoop
class Parent {
    fun greet(): String = "hi"        // 非 open
}

class Child : Parent {
    fun greet(): String = "yo"        // ← scoop::typecheck::override_non_open_method
}
```

错误信息提示"父类该方法非 open，不可重写"——无论子类是否写 `override`。

#### 4.4.2 父类 method 是 `open`，子类同 signature 缺 `override` → reject

```scoop
class Parent {
    open fun greet(): String = "hi"
}

class Child : Parent {
    fun greet(): String = "yo"        // ← scoop::typecheck::missing_override
}
```

#### 4.4.3 子类带 `override` 但父类无匹配 signature → reject

```scoop
class Parent {
    open fun greet(): String = "hi"
}

class Child : Parent {
    override fun farewell(): String = "bye"   // ← scoop::typecheck::override_target_not_found
}
```

#### 4.4.4 子类同名但 signature 不同 → 合法新增 overload

```scoop
class Parent {
    fun greet(): String = "hi"               // 非 open，但只是普通方法
}

class Child : Parent {
    fun greet(times: Int): String = "..."   // 合法：不同 signature，新增 overload
}
```

`Child` 的 overload 集合中现在含两条 `greet`：继承自 `Parent` 的 `greet()` 与子类新增的 `greet(Int)`。两者按 specificity 各自参与 resolution。

### 4.5 虚方法不可方法级 generic

由于 Scoop 走 monomorphization（与 Java/Kotlin 的类型擦除不同），虚分派的 vtable 槽位必须在编译期固定。方法级 type parameter 在 monomorphization 后会展开成多个不同签名的实例，无法填入单个 vtable 槽——这与 C++ 不允许 `virtual template` 的理由完全相同。

以下声明位置 reject 任何"方法自己引入"的 type parameter：

- `open fun ...`
- `abstract fun ...`
- `override fun ...`
- `interface` body 内的 `fun ...`（默认 abstract）

但**类级 / interface 级 TP** 在方法签名里出现完全合法——它们在类 / interface 被 monomorphize 时已经被钉死，vtable 槽位仍是固定的。

合法 vs 非法对照：

```scoop
class Box<T> {
    open fun get(): T = ...                       // ✓ T 是类级
    open fun set(x: T): Unit = ...                // ✓ T 是类级
    open fun <U> map(f: (T) -> U): Box<U> = ...   // ✗ U 是方法级
    fun <U> mapStatic(f: (T) -> U): Box<U> = ...  // ✓ 非虚方法，方法级 TP 允许
}

interface Iter<T> {
    fun next(): T                                 // ✓ T 是接口级
    fun <R> fold(init: R, f: (R, T) -> R): R      // ✗ R 是方法级
}

abstract class Renderer {
    abstract fun render(x: Any): String           // ✓ 非 generic
    abstract fun <T> renderG(x: T): String        // ✗ T 是方法级
}
```

错误码：`scoop::typecheck::virtual_method_cannot_be_generic`。错误信息提示 "virtual methods cannot have method-level type parameters; use a class-level type parameter, or convert to a non-virtual / free-standing function"。

#### 4.5.1 与 §4.4 override 的连锁

因父类虚方法本就不能引入方法级 TP，"override 时 bound 是否能改变"的问题不存在——bound-based specialization（§6.4）天然不与虚分派组合。需要"按类型特化的虚方法"时，自然路径是把它拆成自由函数：

```scoop
// ✗ 想这样写
abstract class Renderer {
    abstract fun <T> render(x: T): String
    abstract fun <T: Debug> render(x: T): String
}

// ✓ 改成自由函数 + 实例方法分工
abstract class Renderer { abstract fun renderAny(x: Any): String }
fun <T> render(r: Renderer, x: T): String = r.renderAny(x)        // 兜底
fun <T: Debug> render(r: Renderer, x: T): String = x.debugString() // 特化
```

#### 4.5.2 不影响的位置

以下位置**不受**本节限制（仍可方法级 generic）：

- top-level `fun`
- final（无 `open` / `abstract` / `override` 的）成员方法
- `extension fun`
- `constructor`
- `companion object` / `object` body 内的 `fun`（语义上是静态分派，无 vtable）

---

## 5. Call-site 解析算法

调用点的 overload resolution 按以下五个 phase 串行进行。任何 phase 失败都直接报对应错误，不回溯。

### 5.1 Phase A: 候选收集（scope 层叠）

按以下优先级层叠收集候选：

```
1. local         （函数体内可见的本地 fun / closure binding）
2. member        （receiver 类型的成员方法 + 继承自父类的方法）
3. extension     （当前 scope 可见的 extension function）
4. top-level     （同 package 的顶层 fun / constructor）
5. imported      （import 进来的 fun / constructor）
```

**外层 scope 完全 shadow**：在某层找到任何候选（无论是否最终适用）即停止下沉。这避免了"在文件顶部改 import 顺序导致行为变化"的混乱。

举例：

```scoop
package outer_shadow

import scoop.core.*

fun f(x: Int): Int = 1                    // top-level，不会进入下面 inner 的 resolution

fun outer(): Int {
    fun f(x: Int): Int = 2                // local，shadow 顶层
    return f(10)                           // 选 local 的 f，结果 2
}
```

### 5.2 Phase B: 可见性筛选

剔除当前调用 scope 不可见的候选（`private` / `protected` / `internal` 等访问修饰）。

**可见性是 applicability 之前的筛选条件**——不可见的候选根本不进 specificity 比较。否则会出现"调用一个看不见的更具体重载导致它的 friend 重载也被排除"的诡异错误信息。

### 5.3 Phase C: Applicability 筛选

每个候选检查"实参能否传给形参"：

1. **Arity 匹配**：参数个数（含 vararg 展开后）与实参数相符。命名参数与默认参数（如 Scoop 支持）按各自规则展开。
2. **类型 subtype**：每个实参的类型必须是对应形参类型的 subtype。**Scoop 不做隐式 widening**（无 `Int → Float64` 等），所以 subtype 关系仅来自：
   - nominal subtype（class / interface 继承链）
   - function 子类型化（参数逆变 / 返回协变 / effect row 子集）
   - tuple / struct 协变（如 spec 规定）
   - `Nothing` 是任何类型的 subtype（unreachable / throw 表达式的类型）
3. 不满足任一条件的候选被剔除。

筛选后若候选集为空 → `scoop::typecheck::no_applicable_overload`，列出所有同名候选 + 实参类型 + 每个候选不适用的具体原因。

### 5.4 Phase D: Specificity 比较

应用候选两两比较 specificity（详见 §6），选出**唯一最具体**：

> 候选 A 比候选 B 更具体（A ≻ B）当且仅当对所有参数位 i 有 `A.eff_i <: B.eff_i`，且至少一处严格 subtype。
>
> 其中 `eff_i` 是参数位 i 的 **effective type**（详见 §6.4）：concrete 类型用其本身；未替换 TP 用其 declared bound（无 bound 视为 `Any`）。

最具体候选是不被任何其他候选 ≻ 的候选。若存在唯一这样的候选 → 选定。

### 5.5 Phase E: 歧义检查

若 Phase D 后没有唯一最具体候选（即存在两个候选互不 ≻）→ `scoop::typecheck::ambiguous_overload`，列出所有适用候选 + 每个候选位置 + 哪些位置类型不可比（详见 §10）。

---

## 6. Specificity 规则细则

### 6.1 类型 subtype 比较

类型 A `<:` 类型 B 仅来自以下**明示**关系（无隐式 widening）：

- A == B（含 type alias 透明展开）
- A 是 B 的 nominal subtype（继承链）
- A、B 都是 function type，且 A 的参数类型逆变、A 的返回协变、A 的 effect row 是 B 的子集
- A、B 都是 tuple / struct，且每个对应位置满足 spec 规定的 variance
- A == `Nothing`

**没有** Int → Long widening、没有 Int → Float widening、没有 String → Any 之外的 implicit conversion。任何"看起来直觉上能转"的关系都不算 subtype。

### 6.2 Receiver 算第 0 个参数位

Member method 的 receiver 在 specificity 比较中算 0 号参数：

```scoop
class Animal { ... }
class Dog : Animal { ... }

class Animal {
    fun describe(): String = "animal"
}

class Dog : Animal {
    override fun describe(): String = "dog"
}

// 当 receiver 类型是 Dog 时，candidate 集合里 Dog.describe 比 Animal.describe 更具体
// （Dog <: Animal，receiver 位 strict subtype）
```

注意：这是 **static resolution**——按 receiver 的**静态类型**选签名。如果 receiver 是 `val a: Animal = Dog()`，调用 `a.describe()` 时静态选 `Animal.describe`，runtime 才 dispatch 到 `Dog.describe`（前提是该方法 `open`）。详见 §7.2。

### 6.3 Function 子类型化在 specificity 中的应用

```scoop
fun call(f: () -> Number): Number = f()
fun call(f: () -> Int): Int = f()

call({ 42 })   // { 42 } 类型 = () -> Int
               // 候选 1 适用（() -> Int <: () -> Number，返回协变）
               // 候选 2 适用（() -> Int <: () -> Int）
               // specificity: () -> Int <: () -> Number → 候选 2 更具体 → 选候选 2
```

`{ 42 }` 在 Scoop 中类型固定 `() -> Int`（不像 Kotlin 那种"按上下文期望反推"），所以这条 specificity 比较直接、无歧义。

### 6.4 Effective type 与 bound-based specialization

**核心原则**（用户视角）：

> 在 specificity 比较中，**更靠近实参类型的 concrete 类型 ≻ 更远的父类型**——无论"父类型"来自 concrete 声明还是 type parameter 的 bound。

形式化：参数位 i 的 **effective type** `eff_i` 按候选声明决定：

- 参数声明为 concrete 类型 `X` → `eff_i = X`
- 参数声明为未替换 TP `T` → `eff_i = T 的 declared bound`；无 bound 视为顶类型 `Any`
- 多重 bound `<T: A & B>` → `eff_i = A & B`（intersection）；自然地 `A & B <: A` 且 `A & B <: B`
- 复合类型中嵌入 TP（如 `List<T>`、`(T) -> U`）→ effective type 由对其中每个 TP 替换为 bound 后整体得到

应用 §5.4 的规则：A ≻ B iff ∀i: `A.eff_i <: B.eff_i` 且至少一处严格。

**直觉**：applicability 已经把所有候选锚定在实参 `R_i` 的祖先链上（每个 eff 都是 `R_i` 的 supertype）。pairwise "eff_A <: eff_B" 等价于"沿 subtyping 链上谁更靠近 `R_i`"。

#### 6.4.1 典型例子

```scoop
fun debugPrint<T>(x: T) = println(x.toString())            // 兜底
fun debugPrint<T: Debug>(x: T) = println(x.debugString())  // 特化

debugPrint(5)
// 候选 1 (T=Any) 适用：Int <: Any
// 候选 2 (T: Debug) 仅当 Int 实现 Debug 时适用
//   - 若 Int 未实现 Debug → 候选 2 不适用 → 选兜底
//   - 若 Int 实现 Debug → eff: Debug <: Any 严格 → 选特化
```

```scoop
fun f(x: Int): Int = ...                       // 候选 A：concrete
fun f<T: Number>(x: T): T = ...                // 候选 B：bounded
fun f<T>(x: T): T = ...                        // 候选 C：unbounded

f(5)
// 三者都适用；eff: Int <: Number <: Any
// A 在所有位严格更具体 → 选 A
```

```scoop
fun g<T: Comparable>(x: T) = ...
fun g<T: Numeric>(x: T) = ...

g(5)   // 假设 Int 同时实现 Comparable 与 Numeric
       // 两者都适用；eff: Comparable 与 Numeric 不可比
       // 无唯一最具体 → ambiguous_overload（错误码与 §5.5 一致）
```

```scoop
fun h<T: Animal>(x: T) = ...
fun h<T: Dog>(x: T) = ...     // Dog <: Animal

h(myDog)
// 两者都适用；eff: Dog <: Animal 严格 → 选 Dog 版
```

#### 6.4.2 关键边界：specialization 不"穿透"未替换 TP

Specificity 比较使用 **declared bound**，不使用 inferred substitution。这意味着 specialization 只在 declared bound 已经满足条件时才触发：

```scoop
fun helper<U>(x: U) {
    debugPrint(x)            // 调用点 U 的 declared bound = Any
                             // 候选 2 (<T: Debug>) applicability 检查 U <: Debug 失败 → 不适用
                             // → 选兜底
}

helper(myDebugThing)         // 仍然走兜底！specialization 不会"事后追上"
```

要让特化触发，调用者必须自己把 bound 写进签名：

```scoop
fun helperD<U: Debug>(x: U) {
    debugPrint(x)            // U: Debug → 候选 2 applicable，eff 更具体 → 选特化
}
```

**为什么这样设计**：在 monomorphization 模型下，`helper<U>` 的 SLIR body 在 lower 时已经按 declared bound 选定 overload；之后用任何具体 U 实例化都共享同一份 body。如果允许"按推断后的具体 T 重新选 overload"，要么需要 per-instantiation re-resolution（Rust specialization 路线，soundness 至今未解），要么放弃参数化原理（同一段代码可观察到不同 T 行为）。Scoop 选简单稳妥的路：**lexical resolution，by declared bound**。

代价：要让特化在某个调用栈上生效，bound 必须在整条传递链上显式标注。这对用户是可预测的、本地可推理的。

#### 6.4.3 已知边角：函数类型参数的 contravariance

当 TP 出现在函数类型参数位置（contravariant），bound 紧度与 specificity 的方向会"翻转"：

```scoop
fun f<T>(g: (T) -> Int) = ...           // eff: (Any) -> Int
fun f<T: Animal>(g: (T) -> Int) = ...   // eff: (Animal) -> Int

f(myDogToInt)   // myDogToInt: (Dog) -> Int
// 函数子类型化：(Any) -> Int <: (Animal) -> Int（参数位逆变）
// 形式上候选 1 严格更具体 → 选候选 1
```

这与"bound 紧的更特化"的直觉相反。原因：T 在 `(T) -> Int` 中处于逆变位置，bound 越紧反而限制了可接受的 g 范围，effective type 反而"更宽"。

实践影响有限——bound-based specialization 的典型场景（如 `debugPrint<T: Debug>(x: T)`）TP 都在协变 / 不变位置。规避建议：**避免在仅以 contravariant 位置出现的 TP 上做 bound-based 重载**。Future Work（§11.1）可能引入更精细的 variance-aware specificity。

#### 6.4.4 可适用范围

bound-based specialization 仅对静态分派位置有意义（与 §7.2 的 static-resolution + dynamic-dispatch 分离一致）：

| 声明位置 | 可方法级 generic | 可按 bound 特化 |
|---|---|---|
| Top-level `fun` | ✓ | ✓ |
| Final 成员方法（无 open / abstract / override） | ✓ | ✓ |
| Extension `fun` | ✓ | ✓ |
| Constructor | ✓ | ✓（per-ctor TP）|
| `companion object` / `object` 内 `fun` | ✓ | ✓ |
| `open` 方法 | ✗ (§4.5) | N/A |
| `abstract` 方法 | ✗ (§4.5) | N/A |
| `override` 方法 | ✗ (§4.5) | N/A |
| `interface` 方法 | ✗ (§4.5) | N/A |

文档使用建议：用户想要"虚方法 + 按类型特化"时，自然路径是把它拆成 free function（见 §4.5.1 的例子）。

---

## 7. Member method 专项规则

### 7.1 Override 边界（与 §4.4 呼应）

简述（详见 §4.4）：

- 父类 method 必须 `open` 才能被子类 override；
- 子类 override 必须用 `override` 关键字；
- 缺少任一条件 / target signature 不存在 → 各自定义点 reject；
- 子类同名但 signature 不同 → 视为新增 overload（与 override 路径无关）。

### 7.2 Static resolution + dynamic dispatch 的分离

无论方法 `open` 与否，**overload resolution 始终按 receiver 的静态类型决定走哪个签名**。如果选定的是 virtual 方法，runtime 再 dispatch 到具体类的实现——但**选哪个签名是 static 的**。

```scoop
class Animal {
    open fun greet(): String = "hi"
    fun greet(times: Int): String = ...   // 非 open
}

class Dog : Animal {
    override fun greet(): String = "woof"
    // 子类未声明 greet(Int)
}

fun demo() {
    val a: Animal = Dog()
    a.greet()              // ① static: Animal.greet()；runtime: dispatch 到 Dog.greet → "woof"
    a.greet(volume = 0.5)  // ② static: Animal 的 overload 集合里没有 (Float) 签名 → typecheck error
                           //    即便 Dog 上有此签名（本例没有，但即便有）也不行——static 只看 a 的类型
}
```

② 是这条规则下用户最容易感到"意外"的地方，文档需要明确告知。

### 7.3 子类 overload 集合 = 父类继承 + 子类新增/override 后的合集

子类调用点的候选集合包含：

- 父类**未被 override** 的所有 visible 方法；
- 父类**被 override** 的方法对应的子类版本；
- 子类新增的 overload。

举例：

```scoop
class Animal {
    open fun greet(): String = "hi"
    fun greet(times: Int): String = ...     // 非 open，仍可继承
}

class Dog : Animal {
    override fun greet(): String = "woof"
    fun greet(volume: Float): String = ...  // 新增 overload
}

// Dog 的 overload 集合：
//   greet(): String          (子类 override 版本)
//   greet(times: Int): String (继承自父类，未被 override)
//   greet(volume: Float): String (子类新增)
```

### 7.4 可见性

- `private` member 仅在该类 body 内可见（同一文件 / 同一类）；
- `protected`（如 Scoop 支持）：仅该类与其子类 body 内可见；
- `internal`（如 Scoop 支持）：仅同 module 内可见；
- `public`：全局可见。

可见性参与 Phase B 筛选（§5.2）。具体修饰符语义以 Scoop 现有 access modifier spec 为准；本文档不重新定义。

---

## 8. 几个特殊形态

### 8.1 Lambda 参数 overload

Scoop 中 `{ 42 }` 类型固定 `() -> Int`，不像 Kotlin 那样按调用期望反推，所以 lambda 参数的 overload 走**普通 subtype-based specificity**，不需要额外 frontend reject：

```scoop
fun apply(f: () -> Int): Int = f()
fun apply(f: () -> Number): Number = f()

apply({ 42 })  // 选 () -> Int 版（更具体）
```

带 receiver 的 lambda、effect row 不同的 lambda 等情形都通过 §6.1 的 function 子类型化规则处理，不需要专门 phase。

### 8.2 Vararg overload

Vararg 与非 vararg 的 arity 重叠在**定义点** reject（§4.3）。这意味着调用点不会出现 "vararg 与非 vararg 都适用"的歧义：

```scoop
// 反例（定义点 reject）
fun a(x: Int): Int = x
fun a(xs: Int*): Int = ...   // ← scoop::typecheck::vararg_overlaps_non_vararg

// 合法：vararg 与非 vararg 不重叠
fun b(): Int = 0                       // arity 0
fun b(x: Int, ys: Int*): Int = ...     // arity ≥ 1
// 调用点 b() 无歧义，b(1) 走 vararg 版（首参 + 0 vararg）
```

如果用户用 spread operator（如 `b(*xs)`）能在 call site 区分两条候选——这种用法仍**不被认为**是合理的 overload 区分手段，定义点直接 reject 优先于让 call site 用 spread 兜底。

### 8.3 Constructor overload

Constructor 在 spec 层面与 fun overload 共用同一套机制：

- 同 class 多个 constructor → overload 集合，按 §4 - §6 规则；
- 签名等价 → §4.1 conflicting overloads；
- 含 ctor 级 type parameter（区别于 class 级）→ 按 §4.2 / §6.4 处理：参数 shape 相同、仅 bound 不同允许；其它 shape 不同 reject；
- class 级 type parameter 在 ctor 签名里出现属于 class 实例化时已钉死的部分，不算"ctor 级 generic"；
- 调用点 `Foo(...)` 的 resolution 走 §5 五 phase。

实现层面是否将 constructor 归一化为 fun 的 sugar、或单独路径——属于实现选择，不影响 spec 行为。

`Foo(...)` 表达式与同名 top-level `fun Foo(...)`（如有）的优先级关系：constructor 在 type 名空间，fun 在 value 名空间——通常 type 名优先于 value 名，但具体规则以 Scoop 现有 namespace spec 为准。本文档假设 `Foo(...)` 永远 resolve 到 constructor（如类型存在），不与同名 fun 重载。

---

## 9. Effect row 与 overload 的协调

### 9.1 不参与 signature

```scoop
// 反例：仅 effect row 不同
fun f(x: Int): Int / Pure!   = ...
fun f(x: Int): Int / Read!   = ...   // ← scoop::typecheck::conflicting_overloads
                                      // （signature 等价：参数列表 (Int) 相同）
```

定义点等价检查（§4.1）也不看 effect row。理由（与 return type 不参与 overload 同构）：调用者上下文期望（callee 的 effect 期望）不应让 callee 通过它分裂——任何"想按 effect 选 impl"的诉求请走显式参数化（传一个 effectful function 或 handler），不要用隐式 overload 分裂。

### 9.2 Effect 检查在 resolution 之后

调用点的 effect row 校验在 §5 五 phase**全部完成、最具体候选选定之后**进行：

1. 选出唯一最具体候选 C；
2. C 的 effect row 与当前调用 scope 的 effect 期望按 effect 子系统规则校验；
3. 若 effect 不匹配 → 报 effect 错误（**不**回退到其他候选）。

这条与 return type 校验同构——选完候选才看返回类型是否匹配 binding 期望，不可比就报类型错误，不回头改选别的 overload。

### 9.3 Override 时 effect row 协变 / 收紧

override 路径下子类的 effect row 是否可严格收紧（例如父类 `Read!` → 子类 `Pure!`），属于 effect 子系统的 spec，不在本文档范围。**但 override target 是否匹配（§4.4.3）仅按 signature 比较，不看 effect row**——effect row 协变是 signature 等价之后的次级约束。

---

## 10. 错误诊断要求

所有 overload 相关 reject 必须满足：

### 10.1 列出所有相关候选

`ambiguous_overload`、`no_applicable_overload`、`conflicting_overloads` 错误必须列出**全部**相关候选，每条标注：

- 候选位置（file:line:col）；
- 候选完整 signature；
- 该候选与当前调用 / 与其他候选不可比的具体原因。

### 10.2 不可比原因要可定位

歧义错误必须指出"为什么没有唯一最具体"，并显式区分 concrete 类型与 TP bound 在 effective type 中的来源。具体形式：

```
error: ambiguous overload for h(5)
  candidates:
    - h<T: Comparable>(x: T): T       defined at file_a.scoop:10:1
    - h<T: Numeric>(x: T): T          defined at file_b.scoop:5:1
  reason:
    - both candidates are applicable: Int satisfies Comparable, Int satisfies Numeric
    - effective param types Comparable (from T's bound) and Numeric (from T's bound)
      are incomparable; no candidate is strictly more specific
  hint: add a more specific overload, or rename to disambiguate
```

涉及 concrete 与 generic 混合时：

```
error: ambiguous overload for h((1, 2))
  candidates:
    - h(x: Int, y: Number): Int       defined at file_a.scoop:10:1
    - h(x: Number, y: Int): Int       defined at file_b.scoop:5:1
  reason:
    - both candidates are applicable
    - position 1: Int <: Int (cand A) vs Int <: Number (cand B): A more specific here
    - position 2: Int <: Number (cand A) vs Int <: Int (cand B): B more specific here
    - cross-incomparable; no candidate is strictly more specific
```

错误信息中**不允许出现** `UnsupportedMainBody`、`backend`、`LLVM`、`codegen` 等内部术语（参见 `pipeline_user_visible_failure_policy.rs::FRONTEND_REJECT_FORBIDDEN_TERMS`）。

### 10.3 候选位置必填

任何形式的 overload 错误信息中候选位置（file:line:col）都不可省略。这是 debug 体验的下限。

---

## 11. Future Work

### 11.1 Generic overloading 的更高级形式

§4.2 + §6.4 已覆盖"参数 shape 相同、仅 bound 不同"的形式（含 concrete 与 generic 混合，以 effective type 统一比较）。未支持的更高级形式：

- **TP 一致性约束**：`f<T>(x: T, y: T)` 与 `f<T, U>(x: T, y: U)` 互不覆盖（前者要求两参数同类型，后者无此要求）。要让 specificity 模型理解"同 TP 出现多次 = 一致性约束"，需要在比较时引入 TP-position 等价类，超出当前简单 effective-type 模型。当前在 §4.2.2 定义点 reject。
- **Inferred substitution 参与 specificity**：当前 specificity 只看 declared bound（§6.4.2），不看推断后的具体类型。这避免了 Rust specialization soundness 难题，但代价是 specialization 不能"穿透"未替换 TP。如果未来要让特化跨越 generic 上下文自动触发，需要后端支持 per-instantiation re-resolution 与配套 monomorphization 改造。
- **Variance-aware specificity**：当前规则在 contravariant 位置的 TP 上会出现"bound 紧反而 effective 更宽"的反直觉结果（§6.4.3）。Future 可在 specificity 比较中按位置 variance 调整方向，或加一条额外 frontend reject。
- **Higher-kinded TPs / type constructor 重载**：完全未触及。

### 11.2 Variance / starprojection

Scoop 目前的 generic 体系是否支持 declaration-site variance（`<in T>` / `<out T>`）会影响 specificity 算法。未来引入时本文档 §6.1 的 function 子类型化条款与 §6.4 的 effective type 计算需要相应扩展（用户声明的 variance 替代默认 invariant 规则）。这条与 §11.1 末尾的 "variance-aware specificity" 互补：前者是声明侧、后者是比较算法侧。

### 11.3 Operator overload 与 desugar 时机

如果 Scoop 有 `operator fun plus(...)` 之类，operator 在表达式中的 desugar（如 `a + b` → `a.plus(b)`）必须**先 desugar 后 overload**——否则 operator 的 overload 选择会绕过本文档规则。需要在实现时确认 desugar pass 与 overload resolution 的执行顺序。

### 11.4 Lambda overload 的额外保守 reject

如果未来发现 §8.1 的 subtype-based specificity 在 function-type-only 区分的 overload 上产生用户预期外的结果，可加一条额外 frontend reject："同名候选间，仅在 function type 上区别，且 specificity 比较结果不显著时拒"。当前不预防，问题出现再加。

---

## 12. 决议汇总表

与 §2 呼应，便于快速查询。

| 决议项 | 选择 | 错误码（如适用） |
|---|---|---|
| Effect row 参与 signature | 不参与 | `conflicting_overloads`（仅 effect row 不同时也算冲突） |
| 隐式 cast / widening | 无 | — |
| Constructor overload 机制 | 与 fun 共用 §4 - §6 | — |
| Member method receiver 在 specificity 中位置 | 第 0 位 | — |
| Member method 可见性参与 phase | Phase B（applicability 之前） | — |
| 父类非 open 而子类同 signature | 定义点 reject | `override_non_open_method` |
| 父类 open 而子类缺 override | 定义点 reject | `missing_override` |
| `override` 但无父类匹配 signature | 定义点 reject | `override_target_not_found` |
| 子类同名但 signature 不同 | 合法新增 overload | — |
| Resolution 静动态分离 | static 选签名，runtime dispatch | — |
| Lambda overload 额外限制 | 无（走普通 specificity） | — |
| Vararg 与非 vararg arity 重叠 | 定义点 reject | `vararg_overlaps_non_vararg` |
| Generic overloading（同 shape 仅 bound 不同） | 允许；按 effective type 参与 specificity | — |
| Generic overloading（参数 shape 不同 / TP 一致性约束） | 定义点 reject | `generic_overload_shape_mismatch` |
| 虚方法（open/abstract/override/interface）引入方法级 TP | 定义点 reject | `virtual_method_cannot_be_generic` |
| 签名等价（含 TP alpha 等价、effective type 完全相同） | 定义点 reject | `conflicting_overloads` |
| Specificity 算法 | A ≻ B iff ∀i: A.eff_i <: B.eff_i 且至少一处严格（eff = concrete 或 TP bound） | — |
| Scope 层叠 | local → member → extension → top-level → imported；外层完全 shadow | — |
| 无适用候选 | call site reject | `no_applicable_overload` |
| 无唯一最具体 | call site reject | `ambiguous_overload` |
| 错误信息列出所有候选 + 位置 + 不可比原因 | 必须 | — |
| 错误信息禁用内部术语 | 必须（FRONTEND_REJECT_FORBIDDEN_TERMS） | — |
