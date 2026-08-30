# M3 设计：泛型单态化与 Option

版本：0.2（实现对齐）

对应 `docs/ROADMAP.md` 的 M3。目标：泛型函数的单态化、内建 `Option<T>`、`T?` 脱糖与 `?.` / `?:` / `!!`（spec 第 7 章）。

## 0. 范围说明：为什么 M3 包含函数参数与返回值

M1/M2 的函数无参数、无返回值；泛型脱离参数/返回值没有意义，所以 M3 顺带引入：函数参数（带类型标注）、返回类型标注、`return` 语句、表达式函数体（`fun f(x: Int): Int = x + 1`）。enum（spec 4.2）属 M4，因此 `Option<T>` 在 M3 是**编译器内建类型**（见 5.1）。

## 1. 语言子集

```
fun <T> identity(x: T): T = x

fun <T> unwrapOr(o: Option<T>, fallback: T): T {
    return o ?: fallback
}

fun describe(p: Point?): String {
    val x = p?.x            // Option<Int>
    if (x == None) {        // None 字面量比较
        return "empty"
    }
    return "x=" + "!"       // 返回类型匹配检查
}

fun main() {
    val a: Int? = Some(41)
    val b = a ?: 0
    println(identity(b + 1))            // 42，T 推断为 Int
    println(identity("hi"))             // T 推断为 String
    val p: Point? = Some(Point(1, 2))
    println(p?.x ?: 0)                  // 1
    val q: Point? = None
    println(q?.x ?: 0)                  // 0
    println(unwrapOr(a, 0)!! + 1)       // !! 取 Some 值
}
```

新增（相对 M2）：

- **函数**：参数列表 `(x: T, ...)`；返回类型标注 `: T`（缺省 `Unit`）；`return expr?` 语句；表达式函数体 `fun f(...) [: T] = expr`。
- **泛型函数**：`fun <T> f(...)`、`fun <T, U> ...`；类型参数无约束（`value`/`ref` bound 见 spec 13.9，本里程碑不实现）；调用处**只做类型实参推断**，不支持显式类型实参（`f<Int>(1)` 的 `<` 与小于号有解析歧义，留待后续里程碑）。
- **Option 与可空**：内建类型 `Option<T>`；构造器 `Some(expr)` 与值 `None`；类型标注后缀 `T?`（脱糖为 `Option<T>`，`T??` = `Option<Option<T>>` 不塌陷，spec 7.1）；`?.`（仅字段访问，方法尚不存在）、`?:`、`!!`。
- **返回规则**（M3 简化）：非 `Unit` 函数的 block 体必须以 `return expr` 结尾（不对分支做穷尽分析）；`Unit` 函数可无 `return`；表达式体不受此限。

明确不在 M3：`when`/解构（M4，故 M3 中 Option 只能通过 `?.`/`?:`/`!!`/`== None` 观察）；泛型 struct/enum 声明（M4 评估 Option 是否迁移为真正的核心库 enum）；`value`/`ref` bound；显式类型实参；异常（`!!` 失败是 trap，见 5.3）。

## 2. 各 stage 设计

### 2.1 AST / parser

- 新增：参数列表、返回类型标注、`return`、`fun` 后的类型参数列表 `<T, ...>`（`fun` 名后紧跟 `<` 无歧义）、表达式函数体 `= expr`；类型标注后缀 `?`（`TypeRefKind::Nullable`）；运算符 `!!`（后缀，优先级高于一元）、`?.`（后缀，与 `.` 同级）、`?:`（右结合，优先级低于 `||`）。
- `Some(...)` 按普通 Call 解析，由 HIR 识别；`None` 按标识符解析，由 HIR 识别。

### 2.2 HIR

- `Type` 新增：`Param(TypeParamId)`（函数类型参数）、`Option(TypeId)`。`TypeParamId` 是有独立 Rust 类型的函数/泛型类型局部参数索引，不能与字段、变体等裸整数下标混用。
- 泛型函数只**定义处检查一次**（参数化类型下）：对类型参数不允许任何具体操作（不能比较相等之外的运算、不能 print——print 只接受 String/Int/Boolean，`T` 无约束无法证明，定义处即报诊断）。`==`/`!=` 对任意两侧同类型允许（含 `T` 与 `T`、`Option<T>` 与 `None` 的比较——M3 中 `== None` 靠此实现）。
- **调用处推断**：按实参类型逐一绑定类型参数；无法绑定（未在参数中出现的类型参数）或绑定冲突（同一参数推出两个不同类型）是诊断。
- HIR 输出新增（impl spec 2.2 的落地）：
  - **generic function list**：`Arena<GenericFunction>`，由独立的 `GenericFunctionId` 标识带类型参数的函数体；
  - **instantiation list**：`Arena<ResolvedGenericFunction>`，由独立的 `ResolvedGenericFunctionId` 标识 `(GenericFunctionId, Vec<TypeId>)` 实例化需求（含泛型函数互相调用产生的嵌套请求，HIR 负责重复合并；参数化的嵌套请求在外层实例降级时具体化）；
  - HIR 的普通调用与泛型调用通过 `Callable::{Function, Generic}` 区分，泛型调用直接携带 `ResolvedGenericFunctionId`，不再以可失配的 `FunctionId + Vec<TypeId>` 表示。
- `?.` / `?:` 脱糖为 HIR 控制流：引入隐藏临时局部变量保存接收者（只求值一次），`a?.f` → `if isSome(tmp) then Some(tmp.unwrap.f) else None`；`a ?: b` → `if isSome(tmp) then tmp.unwrap else b`。因此 HIR 需要内部 expr 节点 `IsSome` / `Unwrap` / `SomeWrap` / `NoneLiteral`（不经由源码语法）。
- `!!` → `Unwrap`（失败 trap 由 LIR/codegen 处理，见 2.4/5.3）。

### 2.3 MIR

- **单态化实例生成**：对每个 `(GenericFnId, type-args)` 生成实例体——类型参数替换为具体类型（`Param(i)` → `type_args[i]`，递归进入 tuple/Option），复制语句/表达式结构。非泛型函数直通。每个已生成实例由 MIR 专属的 `MonomorphizedFunctionId` 标识，不能与普通 `mir::FunctionId` 或 HIR 的解析后实例 id 混用。
- mangling 扩展（编码规则集中在 mir 的 mangling 模块）：实例符号 = `scoop.<name>$<type-args 编码>`；类型编码：Int→`I`、Boolean→`B`、String→`S`、Unit→`U`、struct→名字、tuple→`T<元素…>`、Option→`O<元素>`（如 `scoop.identity$I`、`scoop.unwrapOr$O$S` 待最终编码定）。同一 (fn, args) 在 Cone 内只生成一个实例（按 key 去重）。
- Option 相关节点（`IsSome`/`Unwrap`/`SomeWrap`/`NoneLiteral`）原样进入 MIR，不在此展开——表示方式（tag 还是 niche）是 LIR 的布局职责。
- MIR meta 增加实例清单（`MonomorphizedFunctionId` → 实际函数、符号、泛型来源、具体类型实参），golden dump 输出“符号 → 来源”，并供后续 `.slib` 导出。

### 2.4 LIR

- **Option 布局**（spec 7.4 的 niche 优化在此落地，属语言固定特性）：
  - `Option<Ptr 类型>`（String 及未来的引用类型）：表示为 `Ptr`，`None` = 0；
  - 其他 `Option<T>`：`{ i1 tag, T payload }`（自然对齐）。
  - LIR meta 的布局表按此计算（测试锁定：`Option<String>` size 8、`Option<Int>` size 16）。
- LIR 指令新增（或在既有指令上扩展）：`IsSome`（按布局：Ptr→`ptr != 0`，tag 型→extract tag）、`Unwrap`（Ptr→恒等，tag 型→extract payload）、`SomeWrap`（Ptr→恒等，tag 型→MakeAggregate(true, v)）、`NoneConst`（Ptr→0，tag 型→`{false, undef}`）。这些由 LIR 统一从 MIR 节点翻译，codegen 不需要知道 Option。
- 函数参数与返回值：LIR Function 增加参数列表（参数即带类型的 local，入口块由调用 ABI 传入——codegen 把参数 store 进对应 alloca 或直接作为 SSA 值，二选一，建议参数直接为 SSA 值、需要可写时由 MIR 引入本地副本）；`Terminator::Return` 带返回值（`Value`；Unit 返回空聚合或直接 void——统一策略：Unit 返回类型的函数 LLVM 层面返回 `{}` 还是 void 由 LIR 决定并固定，建议统一 void，`return expr` 中的 Unit 值不产生指令）。
- `!!` 的 trap：`Unwrap` 标记 `trap_on_none: bool`；LIR 生成 `CondBr isSome` → 正常 / 调 `scoop_rt_trap`（noreturn）块。

### 2.5 codegen

- 函数签名：参数与返回类型来自 LIR；`ret <value>` / `ret void`。
- 新指令的机械翻译（extractvalue/icmp/ptr 比较等）。
- runtime 新符号：`void scoop_rt_trap(ptr)`（noreturn，见 3.1）。

## 3. runtime 新增

### 3.1 trap

`void scoop_rt_trap(const char *message)`：写 stderr 后 `abort()`。M8 异常落地后由 `UnwrapException` 抛出路径取代（见 5.3）。

## 4. 测试计划

- **独立 fixture**（`tests/fixtures/m3-generics/`）：`identity<T>` 作用于 Int/String/struct/Option；多类型参数函数；`Some`/`None` 构造与 `T?` 标注；`?.` 链（`p?.x`）；`?:`；`!!` 正常路径；`T??` 双层 Option；`== None` 比较。
- **组合 fixture**：泛型函数返回 Option（`Option<T>` 经 `T?` 推断）；`Option<Point>` 字段访问；泛型函数互相调用（实例化闭包）；同一泛型函数多实例共存（mangling 正确性）。
- **negative fixture**：类型参数推断失败/冲突；`T` 上的非法操作（`x + 1` where `x: T`）；`!!`/`?.`/`?:` 作用于非 Option；`?:` 两侧类型不符；return 类型不匹配；非 Unit 函数缺 return。
- 布局 golden：LIR meta 中 `Option<String>`（niche，size 8）与 `Option<Int>`（tag，size 16）。
- trap 路径测试：`!!` 作用于 None 的 fixture 不能用现有 harness（退出码非 0、stderr 有信息）——harness 扩展：允许 fixture 声明期望失败（文件头注释 `// EXPECT-TRAP`），断言非零退出 + stderr 快照。

## 5. 临时决策（及退役里程碑）

1. **Option 是编译器内建类型**：M4（enum 落地）时评估迁移为真正的核心库 `enum Option<T>` 定义；迁移点只有一个（HIR 的内建识别处），布局与运算语义不变。
2. **`!!` 失败 = trap（`scoop_rt_trap` → abort）**：M8 由 `UnwrapException`（spec 11.7）取代；届时 `Unwrap` 节点的 trap 路径改接异常抛出。
3. **推断唯一来源是实参**：无返回类型反向推断、无显式类型实参；后续里程碑按需补。
4. **非 Unit 函数必须显式 `return` 结尾**：分支穷尽分析（if/else 双返回等）留待后续；`if` 作为表达式（Kotlin 特性）不在 M3。

## 6. 明确不做

- `when`、解构、enum（M4）；泛型类型声明（struct/enum 的类型参数）；`value`/`ref` bound；显式类型实参；异常；`?.` 后跟方法调用（M7 有方法后自然支持）。
