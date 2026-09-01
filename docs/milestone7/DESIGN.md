# M7 设计：函数重载

版本：0.2（实现校准）

对应 `docs/ROADMAP.md` 的 M7。目标：顶层函数与方法的 overload resolution（按参数个数与类型分派），并把 `print`/`println` 从 `@Intrinsic` 内建迁移为 `scoop.core` 的普通重载定义。

## 0. 范围说明

重载本身不大，但它落在所有调用点上，且要把 M1 起最大的一个内建（print/println）完全库化。M7 的语义以 Kotlin 的重载决议为基准，取最小可用形态：M7 没有默认参数/vararg，所以**元数必须精确匹配**；`context` 参数（spec 8.3）未实现，不参与；运算符重载（`operator fun`）不在 M7。

## 1. 语言子集与语义

```
fun show(value: Int): String = "int"
fun show(value: String): String = "string"
fun show(value: Int, extra: Int): String = "two"

open class Shape(val name: String)
class Circle(val r: Int) : Shape("c")

fun tag(s: Shape): String = "shape"
fun tag(d: Describable): String = "desc"

fun main() {
    println(show(1))              // int
    println(show("a"))            // string
    println(show(1, 2))           // two
    val c = Circle(1)
    println(tag(c))               // shape（Shape 比 Describable 更具体）
    println(42)                   // core 的 println(Int) 重载
    println("x")                  // core 的 println(String) 重载
}
```

### 1.1 声明规则

- 同一作用域（顶层 / 同一宿主类型的方法）中允许同名函数，**签名必须可区分**：参数个数不同，或至少一个参数类型不同；
- 仅返回类型不同 = 重复声明诊断（"function `f` is already declared with the same signature"）；
- 泛型重载允许（各候选独立做类型实参推断，M3 机制）；
- override 兼容性规则不变（override 要求精确同签名）；接口实现同样按精确签名匹配。

### 1.2 决议规则（调用点，对齐 [Kotlin 规范 §Overload resolution](https://kotlinlang.org/spec/overload-resolution.html)）

按 Kotlin 的两段式：先按**作用域分层**构造候选集（取第一个非空层），再在候选集内选**最具体（MSC）**。

**第一步：候选集分层**（M7 子集形态，未实现的作用域机制随落地插入对应层）：

- 无显式接收者的调用 `f(...)`，按序：
  1. （预留）局部函数层——局部函数声明落地后插入；
  2. 成员层：当前宿主（`this`）的成员函数（方法体内的裸 `m()` 调用）；
  3. 顶层函数层：用户文件（M17 前的"同包"层；显式/星号 import 分层随 import 机制落地细化）；
  4. 隐式导入层：`scoop.core`（对照 Kotlin 的 implicitly imported callables——最低优先级）。
  **取第一个含有任何候选的层，其后层整层丢弃**（即使外层候选"更合适"——这是 Kotlin 的遮蔽语义）。
- 有显式接收者的调用 `x.f(...)`：M7 只有成员层（接收者类型及其基类链的成员函数；扩展函数落地后在其后插入扩展层）。
- c-level partition（"functions before properties, members before extensions"）：M7 只有函数与成员，自然满足。

**第二步：可应用性过滤**：候选须满足——元数精确相等（M7 无默认参数/vararg）；每个实参类型 `Ti <: Uj`（is_subtype，含 spec 4.4.4 的装箱）；泛型候选先按 M3 机制推断类型实参，失败即不可应用。（对照 Kotlin 的约束系统：非 lambda 实参即 `Ti <: Uj` 约束；lambda 约束 Scoop 暂未涉及。）

**第三步：MSC 选择**：成对检查——候选 A 支配 B 当且仅当 A 的每个参数类型 is_subtype 于 B 的对应参数类型（对应 Kotlin 的"A 可转发到 B"约束检查；M7 对泛型候选使用推断后的类型实参参与比较——这是 Kotlin fresh-variable 约束系统的简化：对单参数推断场景等价，复杂多泛型场景若出现偏差随用例扩展）。随后：

- 恰好一个支配者 → 胜出；
- 互有支配或互无支配时，**非参数化（非泛型）候选优先**（Kotlin case 2/3 的第一附加步；其后的"更少默认值""无 vararg"两步 M7 无此概念）；
- 仍不唯一 → 诊断 "call to `f` is ambiguous: candidates ..."；无可应用候选 → 诊断 "no overload of `f` matches argument types (T1, T2)"（既有 unknown function/arity 诊断并入此形态，保持文案兼容——现有 fixtures 的 unknown-function 快照不应无故变化）。

**不适用的 Kotlin 规则**（注明原因）：整型字面量的 `Widen` 处理（Scoop 无整数字面量类型）；默认值/vararg 相关条款（M7 无此特性）；`OverloadResolutionByLambdaReturnType` 与 lambda 相关细化（无 lambda）；`invoke` 约定与 property-like callables（无属性调用约定）；@JvmOverloads 及一切平台相关项（用户已明确排除）。**不需要单独的装箱优先规则**——子类型支配天然覆盖（`Any` 是父类型，非装箱候选自动更具体）。

## 2. print/println 迁移（core 库函数，`Any.toString()` 分发——过渡基线）

> **后续设计变更（2026-08-28 定稿）**：spec 11.11 已改为接口化方案——`equals` 走 operator fun、`ToString` / `Hash` 为 opt-in 接口、`Any` 无任何成员。本节的 `Any.toString()` 分发形态作为**过渡基线**保留至现 M14“泛型上界约束与接口化”，届时改造为 `fun <T : ToString> print(v: T)`（单态化静态分发）并拆除 vtable 前三槽。

本节形态（当前实现）：**一个输出 intrinsic + `toString()` 分发**（与 Kotlin stdlib 及 spec 11.6 的 `add<T>` 语义一致），core 的 `io.scoop` 为：

```
// sysroot/lib/scoop.core/src/io.scoop
@Intrinsic("rt_write")
fun write(message: String)

fun print(message: Any) = write(message.toString())

fun println(message: Any) {
    write(message.toString())
    write("\n")
}
```

- `@Intrinsic` 的 `rt_print` / `rt_println` 条目**移除**（登记表只剩 `rt_write`）；HIR 对 print/println 的全部特殊检查删除——它们就是普通的 `Any` 参数函数；
- **`toString()` 成为真实可分发的方法**：`Any` 的 `equals`/`hashCode`/`toString` 由 hir-lower 合成为 `Any` 成员（vtable 固定槽 0..2），`x.toString()` 在 `Any` 接收者上经 vtable 分发；
- **装箱值类型的 vtable 槽 2 为逐类型实现**（mir-lower 生成 `scoop.tostring.<ty>`）：Int → `scoop_rt_int_to_string`、Boolean → `scoop_rt_bool_to_string`、其他值类型暂保持 Any 默认（"Object@hex"，结构化 toString 待 spec 定义格式后单独做）；**String 的 TD 补三槽 vtable**（槽 2 = `scoop_rt_string_identity`）——Int/String/Boolean 的输出行为与 M1–M6 完全一致；
- 备选方案（未采用）：六个按类型的重载 + 三个 intrinsic——被本形态取代（少 intrinsic、形状即最终形态）。

## 3. runtime 新增

- `const ScoopString *scoop_rt_int_to_string(int64_t v)`：格式化进临时缓冲，产出 ScoopString（always-leak）；
- `const ScoopString *scoop_rt_bool_to_string(bool v)`：`"true"` / `"false"`；
- `write` 语义 = 现有 `scoop_rt_print`（输出 String，无换行）；runtime 不需要新输出函数。

## 4. 各 stage 设计

### 4.1 HIR

- 符号表：`functions_by_name: name → Vec<FunctionId>`（声明期查重：同签名重复诊断）；方法表同理（宿主内）；
- 调用解析重写为 1.2 的决议算法（函数调用、方法调用、`println` 等 core 调用同一机制）；**分层实现**：候选按（成员 → 用户顶层 → core 隐式导入）分层，取第一个非空层，再在该层内做可应用性与 MSC——实现时 core 文件与用户文件的函数声明需带层标记（file index 已可区分）；callee 名解析不到任何候选 → 沿用现有 "unknown function" 文案；
- 泛型候选的实例化请求照 M3 机制按胜出候选记录；
- 删除 intrinsic 的 print/println 特殊签名规则（登记表只保留 name → runtime 符号映射与"仅 sysroot"检查）。

### 4.2 MIR

- 决议结果已经是唯一 FunctionId；单态化实例按 `(GenericFunctionId, concrete type args)` 去重，不能只按 `name + type args` 去重——两个泛型重载可能推导出相同类型实参。只有同一限定名下存在多个泛型定义时，实例符号才追加 Cone 内的泛型定义 discriminator（如 `scoop.pick$I.g0` / `.g1`），普通泛型函数继续使用 `scoop.<name>$<type args>`；
- 删除 print 六变体的 RuntimeFn 映射，改为 `rt_write`/`rt_int_to_string`/`rt_bool_to_string` 三个新映射（RuntimeFn 变体相应增删：去 PrintString/PrintInt/PrintBoolean/PrintlnString/PrintlnInt/PrintlnBoolean，加 Write/IntToString/BoolToString）。

### 4.3 LIR / codegen

- 无结构变化（runtime 函数直调）；`scoop_rt_int_to_string` / `scoop_rt_bool_to_string` 的声明与签名 `ptr(i64)` / `ptr(i1)`。

## 5. 测试计划

- **独立 fixture**（`tests/fixtures/m7-overload/`）：按类型/元数重载的基础决议；方法重载；子类型最具体选择（`f(Shape)` vs `f(Describable)`）；装箱优先（`f(Int)` vs `f(Any)`，Int 实参选前者）；core 迁移（`println(42)`/`println("x")`/`println(true)` 走 core 重载）；
- **组合 fixture**：泛型与非泛型重载（`fun <T> id(x: T)` vs `fun id(x: Int)`）；两个泛型重载以相同具体类型实参生成不同实例；重载 + when + interface 数组分发；core 的 `println(intToString(...))` 间接验证；
- **negative fixture**：同签名重复声明；歧义调用（如 `f(A)` 与 `f(B)` 对同时 is-a A 和 B 的实参）；无可应用候选；override 签名因重载存在但不匹配基类；
- **迁移回归**：M1–M6 全部 fixture 原样通过（print/println 行为不变）。

## 6. 明确不做与部分实现（随 M7 落地后记入 ROADMAP 第 4 章 backlog）

**明确不做**：

- 默认参数/vararg 对决议的影响（随函数默认参数里程碑重新评估决议规则）；
- 运算符重载（`operator fun`，spec 9.3）；
- `context` 参数参与决议（spec 8.3）；
- 返回值参与决议（Kotlin 也不做，非待办）；
- lambda 相关的决议细化（`OverloadResolutionByLambdaReturnType` 等，随 lambda）；
- `invoke` 约定与 property-like callables。

**涵盖但只部分实现**：

- 候选集分层的完整层级：M7 只实现（成员 → 用户顶层 → core 隐式导入）三层；局部函数层、显式 import / 星号 import 分层随相应机制落地后插入；
- 泛型候选的 MSC 比较用"推断后的类型实参参与比较"，是 Kotlin fresh-variable 约束系统的简化（单参数推断场景等价；复杂多泛型场景随用例扩展）；
- `print`/`println` 的迁移完成后，`write`/`intToString`/`boolToString` 仍是 `@Intrinsic`。M12 将 `write(String)` 迁为直接接收 managed ref的 Scoop ABI `@Extern`，不经 C ABI、`PinnedPtr` 或 storage bridge；数值转换原语由后续对应里程碑迁移；
- 歧义/无匹配诊断的候选列表展示（首版只报主消息，候选明细随用例补充）。
