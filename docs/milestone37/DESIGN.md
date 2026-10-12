# M37 设计：语言语法与类型能力补齐

状态：实施中；基线 `ec9d8849d` 已包含 M35 正式验收及 [M36 类型完备性收紧](../milestone36/DESIGN.md)。各功能的实际完成与验证见 [PROGRESS.md](PROGRESS.md)，本文的设计范围不表示全部工作已完成。

日期：2026-10-11。

基线：沿用 [M35](../milestone35/DESIGN.md) 正式完成后的语言能力，以 [M36](../milestone36/DESIGN.md) 收紧输入与 IR 数据合同后的编译、跨 Cone 产物消费、debug/release 与 GC 能力为实施起点。实施前记录实际基线，不把调研时的未提交工作固定为版本契约。语言语义、阶段职责与运行时约束分别以[语言规范](../specs/SCOOP-SPEC.md)、[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)为依据。

本文由原 M36 顺延为 M37；M36 记录编译器输入与 IR 的类型完备性审查。本文记录 M37 的设计范围与已确定决策，下文列出的规范、路线图、代码、core 与 fixture 变更均为后续实施工作。

## 0. 交付范围与已确定的决策

M37 在 M36 完成后补齐以下语言能力。每项都要求完成源码编译、类型检查、必要的产物传递及实际执行，不能以 parser 接受语法代替功能完成；只有第一项兼容修饰符按明确决策止于语法支持。

| 项目 | M37 交付 |
| --- | --- |
| 兼容修饰符 | 接受 `inline`、`crossinline`、`noinline`、`reified`、`tailrec`，随后完全忽略 |
| 控制结构 | `if` / `else` / `while` / `for` 的单语句体、直接 `else if`、`do ... while` |
| 标签与跳转 | 循环标签、`break@label`、`continue@label`、局部 `return@label`，以及表达式位置的 `break` / `continue` |
| 声明推导 | 具名表达式体函数的返回类型推导，非局部属性的类型推导 |
| 函数值 | 构造器引用、未绑定成员引用、receiver function type / lambda |
| 类型别名 | 泛型、嵌套与局部透明 `typealias` |
| 整型枚举 | `enum E: Int/UInt/...`，限制值域的整型子类型，内存布局与调用 ABI 均与底层整型一致 |
| 引用类型 | `sealed class` / `sealed interface`、接口实现委托、`inner class`、局部类型与匿名 object |
| smart cast | 提前退出后的类型事实、循环与其他正常出口的事实合流、可证明稳定的局部 `var` |
| 用户注解 | 整型枚举 / 嵌套 annotation 参数、`@Target` 目标约束、更多声明目标、use-site target、type-use annotation、显式可重复注解 |
| 标识符 | Unicode 普通标识符与反引号标识符 |

以下三项是本设计的确定边界：

1. **五个兼容修饰符只有语法支持，完全忽略。** 不产生内联提示、尾调用提示、尾递归变换、调用限制、捕获限制、运行期类型保留或 non-local control flow 语义。
2. **明确不支持 interface 方法自身的泛型，以及任何泛型虚方法。** Scoop 采用单态化和固定签名的 vtable / itable；在这条技术路线上，没有合理方式把开放的 method-level 泛型调用纳入现有分派契约。本项是语言边界，不再作为后续待实现能力。
3. **明确不支持捕获外层词法局部 `var`，读取与写入均拒绝。** 不新增隐式 cell、reference capture、effectively-final 推导或其他绕过形式。后续可以讨论普通 core `class Cell<T>`，本里程碑不为此修改语言实现，也不以交付 Cell 为验收条件。

本设计保留 Scoop 的 `Option<T>`、值类型、nominal 泛型不变性、显式 `suspend`、单态化和现有 GC / 异常模型。M35 的 `when` / `case`、subject 解构、Elvis return/throw、Kotlin 尾随 lambda 参数映射及标量注解数组属于前置能力，不重新实现。raw / 三引号字符串、`..<` 与已有泛型扩展属性不再计为缺失项。

null / 平台类型、data class / value class、`lateinit`、nominal `in` / `out` / `*`、运行期反射沿用既有排除规则。IO、集合、高层并发和取消属于库设计，不加入本里程碑的编译器交付条件。

**non-local control flow 统一暂缓实现，不纳入 M37 的交付与验收范围。** 包括 non-local return、non-local break / continue 及其他跨 callable 边界的返回或跳转。此类能力涉及异常、`finally`、Context、`suspend` 等组合，影响面较大；待单独充分评估后，再决定是否支持及如何实现。M37 的 `return@label` 仅支持当前 callable 的局部返回；`break` / `continue` 及其标签形式仅能跳转到当前 callable 内的包围循环。lambda 裸 `return` 和跨 callable 的返回或跳转继续非法；既有异常传播与挂起语义保持。

## 1. 调研结果与实施起点

| 现状 | 需要完成的变化 |
| --- | --- |
| `if` body 只接受块及少数裸循环跳转；`while` / `for` 直接要求块 | 统一 control-structure body 的解析与作用域处理 |
| `do`、标签及表达式位置的 `break` / `continue` 被明确拒绝 | 建立完整的循环入口、条件入口及 typed 跳转目标 |
| 具名函数省略返回类型时直接填 `Unit` | 区分表达式体、块体与无 body 声明；在签名完成前推导表达式体结果 |
| 非局部 property parser 强制要求 `: T` | 从 initializer、表达式 getter 或已选定 delegate getter 推导类型 |
| 已有普通函数引用和绑定 receiver 引用 | 补齐构造器与未绑定成员目标，保留原调用和动态分派规则 |
| 函数类型只有普通参数列表 | 表达 receiver 类型与隐式 `this`，接入调用决议、捕获与产物 |
| alias 只允许 top-level 非泛型形式 | 在现有透明展开中接入参数、owner 与词法作用域 |
| `sealed` / `inner` 被拒绝；object 要求名称；继承项没有 `by` | 补齐声明种类、闭合继承信息、外层实例及普通委托转发 |
| smart cast 主要在条件分支的局部作用域生效 | 将事实沿实际正常出口传播，并正确处理写入和循环回边 |
| enum 只有代数数据类型形态，不具有整型父类型或显式整数值 | 增加整型枚举类别、值类型上转、标量表示与底层整型 ABI |
| 注解限既有类型/字段/property 目标及标量数组 | 接受整型枚举静态值与数组，允许标注 annotation class 并检查 `@Target`；不引入运行期注解对象 |
| 标识符扫描限 ASCII | 贯通 Unicode、转义拼写、名称解析及元数据 |

主要代码起点：

- 语法与声明：[控制结构](../../compiler/parser/src/stmt/control_flow.rs)、[语句](../../compiler/parser/src/stmt.rs)、[声明前缀](../../compiler/parser/src/decl/nominals/prefix.rs)、[属性](../../compiler/parser/src/decl/properties.rs)、[类型别名](../../compiler/parser/src/decl/type_aliases.rs)、[nominal 声明](../../compiler/parser/src/decl/nominals.rs)、[类型语法](../../compiler/parser/src/ty.rs)、[lexer](../../compiler/parser/src/lexer.rs)。
- 类型与调用：[函数签名](../../compiler/hir-lower/src/signatures.rs)、[方法](../../compiler/hir-lower/src/class/methods.rs)、[函数 body](../../compiler/hir-lower/src/stmt/body.rs)、[函数引用](../../compiler/hir-lower/src/expr/callable_references.rs)、[捕获](../../compiler/hir-lower/src/expr/captures.rs)、[smart cast](../../compiler/hir-lower/src/expr/smart_casts.rs)、[控制流](../../compiler/hir-lower/src/stmt/flow.rs)。
- 数据与后续阶段：[AST 声明](../../compiler/ast/src/declarations.rs)、[AST 函数类型](../../compiler/ast/src/types.rs)、[用户注解](../../compiler/hir-lower/src/user_annotations/targets.rs)、[共享注解格式](../../compiler/hir/src/cross_cone_interface/annotations.rs)、[MIR 控制转移](../../compiler/mir-lower/src/cfg/control.rs)。

这些位置用于定位现有实现，不要求每个新语法建立一套独立 lowering。M35 收尾和 M36 收紧中的文件拆分或修复以最终基线为准。

## 2. 兼容修饰符与永久排除项

### 2.1 五个修饰符只解析后忽略

修饰符使用上下文关键字规则，在相应语法位置消费；其他位置仍可按普通标识符使用，反引号形式始终是标识符。

| 拼写 | 接受位置 | 解析之后的行为 |
| --- | --- | --- |
| `inline` | 函数、property 及 accessor 的修饰符位置 | 丢弃，不影响 body、字段、分派或优化 |
| `tailrec` | 函数修饰符位置 | 丢弃，不检查是否递归或是否处于尾位置 |
| `crossinline` / `noinline` | 值参数的修饰符位置 | 丢弃，不检查是否为函数类型，不改变调用或捕获规则 |
| `reified` | 类型参数的修饰符位置 | 丢弃，不依赖 `inline`，不改变 bound、单态化或类型检查 |

此处的“忽略”包括：不验证这些标记之间的语义组合、不因重复出现而增加语义诊断、不要求 `inline` 函数具有某种形状，也不为 `tailrec` 保证常量栈空间。普通声明本身的语法、参数、body、可见性及分派规则仍检查；修饰符不是允许写在任意 token 位置的通行证。

AST 可以为源码定位保留原始 token/span，但 HIR、MIR、LIR、共享签名和 ABI 不保存这五项的语义标志；不把它们转成 LLVM 属性或优化 hint。源码文本、位置及源码哈希自然可以不同，不能据此声称含标记和去掉标记的所有产物字节必须相同。

```scoop
inline fun <reified T> apply(noinline action: (T) -> T, value: T): T {
    return action(value)
}

tailrec fun ordinary(value: Int): Int = value + 1
```

以上声明分别等价于去掉相应标记后的普通声明。`inline` 不允许 lambda 裸 `return`，`reified` 不允许泛型虚方法，`crossinline` / `noinline` 不改变外层局部 `var` 的捕获禁令。挂起调用仍要求既有的显式 `suspend` 上下文。

后续实施必须修订语言规范 8.4 中“允许作为提示”的表述，并把 `tailrec` 纳入同一条完全忽略规则；不能保留文档允许 hint、实现又声称完全无作用的双重契约。

### 2.2 interface 方法泛型与泛型虚方法

以下声明明确非法：

```scoop
interface Invalid {
    fun <T> convert(value: T): T
}

open class Base {
    open fun <T> convert(value: T): T = value
}
```

interface 内的方法不能拥有自己的类型参数，包括 default body 和 private helper。任何需要建立、实现或覆盖 vtable / itable slot 的方法都不能再声明 method-level 类型参数；`open`、`abstract`、`override` 泛型方法仍报错。最终实现写成 `final override` 也不能把一个泛型方法变成合法虚槽实现。

保留以下正常能力：

- 泛型 interface 宿主，例如 `interface Converter<T> { fun convert(value: T): T }`；`Converter<String>` 的非泛型方法可以动态分派。
- 泛型 class 宿主上的非泛型虚方法，其签名由完整宿主 application 确定。
- top-level、local、extension 泛型函数，以及语义为 final、使用 direct dispatch 的 class/struct/enum 泛型方法。
- interface companion 中的普通泛型函数；它属于 object 成员，不是 interface 方法。

原因是开放的方法类型实参会要求虚槽处理无法在固定签名中穷举的专门化，而单态化与动态接收者的组合又必须跨 Cone 保持一致。M37 不为此引入 runtime generic dictionary、擦除后的共享 body、开放 specialization 注册或另一种 interface 类型。后续应将 ROADMAP 对此能力的“未来实现”预留改为明确排除，相关 negative fixture 长期保留。

### 2.3 外层局部 `var` 与 Cell 建议

外层词法局部 `var` 出现在 closure、匿名函数、局部函数或新局部/匿名 class body 的自由变量集合中时，直接报错；只读、没有后续赋值、立即调用、被 smart cast 或写了兼容修饰符均不构成例外。嵌套捕获也不能通过中间环境把该 `var` 变成可捕获的隐藏 `val`。

捕获 `val`、参数、不可变 `this` 继续按值复制。捕获一个 class 引用后修改其对象的普通 `var` property 是合法成员访问；顶层和 object 属性也不是外层词法局部捕获。`receiver::method` 在创建处读取局部 `var` 并保存其当前 receiver 值，仍属于既有的显式快照语义。

未来 core 可以讨论如下普通库形态；这只是 workaround 的表达方式，不是本次新增源码：

```scoop
class Cell<T>(var value: T)

val counter = Cell<Int>(0)
val increment = { counter.value += 1 }
```

这里捕获的是不可重新绑定的 `counter` 引用，修改的是对象 property。用户今天即可按普通 class 规则表达同样的结构。未来是否加入 core、名称和 API 如何确定由库设计决定；编译器不识别 `Cell` 名称、不自动生成 Cell、不自动重写 `var`，也不赋予该类同步或原子语义。

## 3. 控制结构、循环与标签

### 3.1 统一单语句体

`if` 的两个分支、`while` / `for` 的 body 接受块或一个完整语句/表达式。`else if` 是 else 分支中的嵌套 if；else 总是结合到最近的尚未匹配 if。每个分支和循环 body 都建立自己的词法作用域，单条局部声明不泄漏到外面。

```scoop
fun sign(value: Int): Int {
    if (value < 0) return -1
    else if (value > 0) return 1
    return 0
}

val label = if (ready) "ready" else "waiting"
while (hasNext()) consumeNext()
for (item in items) consume(item)
```

条件仍使用括号。值位置的 if 仍必须满足既有结果与正常完成规则：需要正常值的路径必须提供分支值，`Nothing` 路径不参与结果合并；语句位置可以丢弃值。普通函数的表达式体与控制结构中的块不能因为共享花括号而被当成 lambda。

parser 使用同一 control-structure body 入口，保留换行、分号、括号闭合和 dangling else 边界。条件/iterable 后的 body `{ ... }` 不得被前面的调用误吞为尾随 lambda；body 自己的 `consume { ... }` 则使用 M35 的完整尾随 lambda 规则。新语法不能改变已有源码的实参映射或求值顺序。

### 3.2 `do ... while`

支持 `do body while (condition)`，body 同样可为块或单语句。先执行 body，再执行完整 condition；body 至少执行一次。`continue` 跳到本轮 condition 的完整求值入口，`break` 跳到循环之后；condition 为 true 才进入下一轮 body。

```scoop
do {
    val item = readItem()
    consume(item)
} while (item.hasMore)
```

body 最外层直接声明的局部名字在 condition 中可见，condition 之后不可见；嵌套块中的名字仍按普通作用域处理。每轮建立新的 binding，捕获 `val` 得到对应轮次的值。到达 condition 的每条路径都必须已初始化其所读取的 binding；在声明前 `continue` 而 condition 读取该名字时，应在源码处报告未初始化路径。

condition 的临时值、默认参数求值、safe-call setup、异常和挂起都在每次条件检查时执行，不能提前到循环外。lowering 必须表达 body 入口、condition 入口和退出边；不能用会改变作用域或 `continue` 目标的朴素 `while (true)` 文本替换。

### 3.3 标签与局部返回

支持 `label@ while ...`、`label@ for ...`、`label@ do ...`，以及显式标记的 lambda / 匿名函数。`break@label` 和 `continue@label` 必须命中当前 callable 内的包围循环；无标签时仍选择最内层循环。标签属于独立的词法名称空间，同名标签由最近包围声明遮蔽，不跨 callable 查找跳转目标。

```scoop
outer@ for (row in rows) {
    for (item in row) {
        if (done(item)) break@outer
        if (skipRow(item)) continue@outer
        consume(item)
    }
}

visit(items) item@ { item ->
    if (skip(item)) return@item
    consume(item)
}
```

`return@label` 只返回当前 lambda、匿名函数或具名函数自身，不能从嵌套 callable 跳到外层 callable。lambda 可由显式标签命名；直接作为调用实参的 lambda 还可使用调用末段名称作为隐式标签，例如 `forEach { return@forEach }`。复杂 callee 表达式没有可确定名称时不生成隐式标签。显式标签优先于同一 lambda 的隐式标签，词法嵌套仍按最近目标解析。具名函数可以由自身名称标识局部返回目标。

带值的局部 return 按目标 callable 的结果类型检查，无值 return 提供 Unit。无显式结果类型的 lambda 将指向自身的 return 值与正常末尾结果共同用于推导；不把内层 callable 的 return 纳入外层。lambda 裸 `return` 继续非法，表达式体具名函数中禁止显式 return 的规则也保持。

标签不是通用 goto，不提供跳出任意块的 `break`。跨 callable 的 return、break、continue 等 non-local control flow 统一按第 0 节暂缓，`inline` 不构成例外。`this@label` 的 receiver 选择见第 5.2 节。

### 3.4 `break` / `continue` 进入表达式位置

与 M35 的 return/throw 相同，合法的 `break` / `continue` 及其标签形式具有 `Nothing` 类型，可用于 Elvis、分支、实参等普通表达式位置：

```scoop
while (hasNext()) {
    val item = readOptional() ?: continue
    consume(item)
}
```

类型和目标检查不以运行期可达性作为豁免。发生跳转后不执行后续实参、默认表达式或外层调用，也不产生伪造的正常值。AST 保留跳转的来源与标签；HIR 中目标必须已解析为对应种类的 typed identity，MIR 不再通过名字找目标。

全部转移复用现有 finally、catch 结束动作、Context 恢复、native root 与 coroutine 清理。跨出哪些 scope 由真实目标决定；finally 中的新转移可以覆盖待定转移，内部自行处理的转移不能错误覆盖。挂起本身仍不是离开循环或执行 finally。

## 4. 声明类型推导

### 4.1 具名表达式体函数

`fun answer() = 42` 推导为返回 Int，覆盖 top-level、member、extension 和 local 函数。带 `suspend`、泛型参数或普通 override 的声明使用相同推导，再按已有挂起、bound 和 override 规则验证。

规则如下：

- 显式 `: R` 优先，body 按该期望类型检查；省略类型的表达式体从表达式的正常结果推导，所有路径均无正常结果时为 Nothing。
- 省略返回类型的普通具名块体函数仍返回 Unit，不从其中的 return 语句推导为其他类型。无 body 的声明也不能凭调用处猜返回类型，沿用省略类型即 Unit 的声明规则。
- 不把调用者的期望结果类型反向用于选择被调用声明的返回类型；定义处只推导一次，导出的是确定签名。
- 泛型 body 在其已声明参数和 bound 下推导，结果可以包含这些参数；不等某个具体 application 出现后再改变模板签名。
- 空数组、没有期望类型的 `None`、未定型 lambda 或未解出的类型变量按现有推导规则诊断，不回退成 Any 或 Unit。
- 表达式体中仍禁止显式 return；`throw` 和返回 Nothing 的调用可以作为表达式体。

```scoop
fun answer() = 42
fun <T> identity(value: T) = value
fun describe(ready: Boolean) = if (ready) "ready" else "waiting"
```

### 4.2 属性推导

局部绑定已有推导。M37 扩展到 top-level、class/object/companion property，以及合法的 computed / extension / delegated property；推导来源按下表确定。

| 源码形态 | 类型来源 |
| --- | --- |
| `val/var p = expression` | initializer 的静态类型 |
| `val p get() = expression` | 表达式 getter 的结果类型 |
| 合法 computed `var` 的表达式 getter | getter 的结果类型；setter 按得到的 property 类型检查 |
| `val/var p by delegate` | 按既有 delegate 协议唯一选定的 getter 结果类型 |
| `const val P = expression` | 已有标量常量表达式的类型，随后执行 const 合法性检查 |

显式 property 类型总是优先。stored property 的 custom getter/setter 不覆盖 initializer 已推导的类型；setter 参数、赋值处右值和下游使用不能反向改变 property 类型。delegate 选择若依赖一个尚未知的 property 类型而无法唯一完成，要求源码补充类型标注。

没有 initializer、只有块体 getter、只有 abstract accessor 或没有其他独立定型来源时，必须显式写 property 类型。主构造参数、函数参数与 enum payload 参数仍要求原有的参数类型声明，不从默认值推导签名。省略 initializer 的 Option stored `var` 继续要求明确类型；推导不会放宽初始化规则。

`const val` 的推导只省去 `: T`，常量值域和允许的表达式不扩大，不加入数组常量、用户对象构造或通用 CTFE。普通 property 的 initializer 也只参与编译期类型检查，运行期求值仍发生在既有初始化位置，次数和顺序不变。

### 4.3 签名依赖、递归与可见性

先建立声明身份、名字和显式签名部分，再解析实际需要的推导依赖。函数结果、属性类型和 alias 展开之间可以共享现有的声明查询；不能按文件枚举顺序决定结果，也不能为了推导而执行源码。

声明签名推导环必须通过显式类型打断。例如 `fun f() = f()`，或两个推导 property 互相依赖，均报告相关声明及需要补充类型的位置。不对环内声明反复猜类型，不以迭代次数决定合法性。显式签名的普通递归函数保持合法，局部函数的前向可见性仍遵守原有规则。

推导完成后按原有 effective access domain 检查全部类型引用；省略类型不能泄漏 private/local/anonymous 类型或绕过公开接口约束。重载签名仍不以返回类型区分；调用解析使用已完成的声明结果，不通过只比较结果类型增加重载优先规则。

待推导状态只能存在于 hir-lower 的内部工作状态。成功 HIR、共享声明与后续 IR 中的签名必须完整，不能留下 `Option<TypeInfo>`、未解类型变量或供消费方补推导的 body。

## 5. 函数引用与 receiver 函数值

### 5.1 构造器和未绑定成员引用

```scoop
class Point(val x: Int, val y: Int) {
    fun total(): Int = x + y
}

val make: (Int, Int) -> Point = ::Point
val total: (Point) -> Int = Point::total
```

`::TypeName` 从正常可见的构造器集合中选择目标，类型别名先透明展开。abstract class、interface、object、annotation class 和其他没有普通构造入口的类型不能因此被构造。primary / secondary constructor 继续使用已有身份、访问规则及初始化实现，不额外复制初始化 body。

`Type::member` 表示未绑定成员引用，receiver 成为第一个函数参数；`receiver::member` 保持创建时求值一次并绑定 receiver 的行为。type qualifier 与值 receiver 按普通名称解析和类型语法区分，不凭是否以大写字母开头判定。object/companion 限定名使用 singleton value，包括完整泛型宿主 application、透明 alias 与限定路径。若源码名字经正常查询后仍同时形成无法唯一选择的目标，应报告歧义。

virtual/interface 引用每次调用都按实际 receiver 动态分派。构造器与成员的重载使用期望函数类型和既有候选规则；没有期望类型时，只允许能独立确定完整类型的唯一候选。泛型宿主和泛型 final 方法的全部实参必须在创建点确定，不产生多态函数值。

引用结果不保留声明参数名、默认值或 vararg 调用约定，必须提供完整实际参数列表；vararg 在函数类型中仍为 Array 参数。需要省略默认参数或改写参数次序时，使用显式适配 lambda。inner 构造器的绑定外层实例见第 7.3 节。

新引用形式创建普通 managed 函数值，使用现有 adapter、捕获字段和 GC 扫描。它们不自动进入 FunPtr 路径，也不放宽 `@NoGC`、FFI、suspend callback 或裸地址规则。

### 5.2 receiver function type 与 lambda

支持 `A.(P1, P2) -> R` 和显式 `suspend A.(P1, P2) -> R`。receiver 是独立的源码槽位，其类型必须完整；普通参数不含 receiver，因而 `A.() -> R` 的 lambda 没有普通参数，`A.(P) -> R` 在省略参数列表时仍可引入 `it`。

```scoop
class Builder {
    fun append(value: String) {}
}

fun build(action: Builder.() -> Unit): Builder {
    val builder = Builder()
    builder.action()
    return builder
}

val result = build {
    append("hello")
}
```

期望 receiver function type 为 lambda 提供不可重新绑定的隐式 `this`；显式参数模式仍只对应普通参数。没有期望类型时，可以使用带显式 receiver 的匿名函数 `fun A.(parameters): R { ... }` 建立相同函数值。挂起 lambda/匿名函数仍必须显式写 `suspend`。

receiver 函数值支持 `f(receiver, args...)`、`f.invoke(receiver, args...)` 和 `receiver.f(args...)`。最后一种形式仍按正常成员、局部函数值和扩展候选优先级解析，不能覆盖已经由原有规则选中的实际成员。receiver 与普通实参按源码顺序各求值一次，值类型 receiver 沿语言规范 3.3 按值传递，不产生调用方 place 的隐式引用。

在 body 中，显式局部名字和参数遵守原有遮蔽规则；隐式 receiver 从内向外参与成员查找，各层复用现有候选适用性与重载规则。`this` 指当前最近 receiver；`this@label` 可以选中有明确名称的外层 class、扩展函数 receiver 或 receiver lambda。没有 receiver 的标签不能用于 `this@label`。Context 仍是既有的命名绑定，不加入隐式 receiver 列表。

receiver 是否存在及其完整类型属于源码函数类型的形状和身份，并在共享签名中保留，以便依赖消费者正确解析 lambda 和调用。receiver 函数值与把 receiver 放在首位的普通函数值允许双向适配：`A.(B) -> R` 与 `(A, B) -> R` 是两种可适配的函数类型，按展平后的参数逆变、结果协变和相同挂起性检查。仅把首参数改写为 receiver 不能形成新的重载区分。

这种函数值适配不扩大 nominal 泛型不变性，不自动转换已存在的 Array、Option 或其他容器中的函数值。函数类型的源码 receiver 形状必须在推导与元数据中保持明确；运行期 `is` / `as` 的函数兼容检查使用同样的参数顺序和适配关系，不能静态允许而运行期拒绝。既有函数值适配不保证 identity 相同的规则继续适用。

lowering 复用现有 closure/invoke 模型，将 receiver 展平为第一个实际源码参数，并与其他参数一起进行既有 ABI 分类；closure environment 等已有隐藏参数仍单独处理。receiver 的值类型复制、ZST 消参和 ref 传递沿用同一协议，不建立新的调用 ABI、DSL 执行器或另一种闭包。receiver 自身及外层 receiver 的捕获都服从第 2.3 节；隐式 `this` 不能让外层局部 `var` 绕过检查。

## 6. 泛型、嵌套与局部 typealias

```scoop
typealias Names<T> = Array<T>
typealias Transform<T> = T.(Int) -> T

class Catalog {
    typealias Entry<T> = (String, T)
}

fun <T> use(value: T) {
    typealias Entry = (String, T)
    val entry: Entry = ("value", value)
}
```

alias 可以声明自己的类型参数和正常 bound，右侧在定义处绑定为完整类型表达式；使用时替换实际参数并透明展开。类型位置必须提供全部参数，不能形成裸泛型、star 或 projection。经 alias 进行构造时，可以按原有构造推导规则解出 alias 参数，但固定在 alias 目标中的实参不能被覆盖；不能唯一求解时要求完整参数。

支持 top-level、nominal body 和局部块中的 alias。普通 static nested owner 保持独立类型参数作用域，嵌套 alias 不因位于泛型 class 中就自动捕获宿主参数；需要参数时显式声明。inner/companion 的实例化作用域与局部作用域允许引用各自本来可见的外层类型参数，使用限定名时必须提供所需的完整宿主 application。

局部 alias 在自身声明之后的词法范围内可见，不能建立运行期状态。它可以引用已可见的局部类型；类型推导、签名导出和可见性检查均针对展开后的实际类型。alias 内的名字不在使用处重新查找。

全部 alias 展开依赖必须无环，包含在泛型实参、函数类型或数组内部的递归引用也不能构成例外。泛型 alias 不意味着支持递归 alias，不以递归深度阈值代替此规则。

alias 始终不产生 nominal identity、layout、TypeDescriptor、RTTI、box 或新构造器。导出的泛型 alias 保存实际参数 binder、bound 和 typed target；局部 alias 在对外需要类型的地方展开，不导出局部名称。跨 Cone 使用与再次发布不重复绑定名称，也不把同形 alias 变成新的重载。

## 7. 引用类型与局部类型

### 7.1 sealed 继承与 when 穷尽性

`sealed class` 隐含 abstract，`sealed interface` 是受限继承的普通接口；两者均采用既有引用表示。sealed class 构造器只能为 protected/private，省略时采用 protected；不能直接构造。sealed 不使普通成员自动成为可覆写方法。

直接子类型必须位于 sealed 声明的同一 origin Cone、同一 package，且为可命名的非局部、非匿名声明。满足可见性的嵌套声明可以参与；re-export 不改变 origin。普通子类型可以是 final、open/abstract 或继续 sealed；非 sealed 子类型下面的继承仍按正常规则开放。

```scoop
sealed interface Result
class Success(val value: String) : Result
class Failure(val message: String) : Result

fun describe(result: Result): String = when (result) {
    is Success -> result.value
    is Failure -> result.message
}
```

when 的覆盖域来自 sealed 声明的完整直接子类型关系，而不是扫描当前消费 Cone 恰好引用的实现。继续 sealed 的节点可以递归展开；非 sealed 节点作为一个子类型家族，其下可能出现的所有实现可由 `is Child` 覆盖。多继承路径按实际 typed application 合并，不重复计算同一关系。

泛型 sealed 使用 exact application 和真实继承替换。例如 `Success<T> : Result<T>` 在 subject 为 `Result<Int>` 时对应 `Success<Int>`。若子类型含有无法由根 application 确定的额外参数，不能枚举几个实例就声称完整；只有能覆盖整个家族的类型条件才计为完整覆盖，否则需要 else。此规则不引入 existential、wildcard 或类型擦除。

带 guard 的分支不提供无条件覆盖；`!is` 的覆盖按同一闭合域求补，普通值条件仍受 M35 的相等与覆盖规则约束。不能从任意用户 equals 推出整个子类型家族已被覆盖。各 arm 的 smart cast、结果类型与不可达诊断复用既有 when 分析。

元数据保存必要的直接子类型身份及继承 application，使消费方能够完成穷尽性检查；未公开子类型的信息可以作为检查所需数据存在，但不因此获得源码可见性。不能在使用点访问所有叶类型时，else 仍是正常写法。增加子类型会改变闭合继承信息并触发既有依赖失效；不新增来源授权或专门的继承许可体系。

### 7.2 接口实现委托

```scoop
interface Writer {
    fun write(value: String)
}

class BufferedWriter(delegate: Writer) : Writer by delegate
```

`I by expression` 只能出现在 class/object 的接口继承项中，I 必须是完整 interface application。表达式的值按正常子类型和适配规则满足 I，构造时求值一次并保存；以后修改表达式原先引用的变量不更换 delegate。struct/enum 不增加隐式 delegate 状态，interface 本身也不能保存实现委托。

初始化顺序为：完成 base constructor，写入 primary property 字段，然后按继承列表顺序求值和保存各 interface delegate，最后执行 body 中原有的 property/init 序列。delegate 表达式可以读取 primary 参数及正常外层名字，不能读取当前未完整初始化的 `this` 或实例成员；无 primary 的声明也不能从某条 secondary 的参数作用域取值。object 的这一步进入原有 singleton 初始化流程。

委托成员按已有 typed slot 选择实现，规则为：

1. 当前 class 的显式实现/override 优先；继承来的具体 class 实现继续按既有成员规则满足 slot，不被隐式转发替换。
2. 尚未满足的 slot 由匹配的显式 delegate 提供转发，包括 abstract class obligation；显式 delegate 的转发优先于普通 interface default 选择。
3. 多个 delegate 同时提供同一未满足 slot 时要求显式 override，不按继承列表顺序选择。
4. 其余 slot 继续执行已有 interface default / abstract obligation 规则，具体 class 不能留下缺失实现。

property 按 getter/setter slot 转发，并继续验证整个 property 的类型与 mutability。调用者的参数映射和默认值仍由其静态声明确定；转发传递已经物化的实参，不再次执行 default。delegate 内部的 `this` 仍是 delegate 对象，不会变成宿主，也不会因宿主 override 而重定向其内部调用。

转发 body 是普通 typed 调用，保留 virtual/interface、suspend、异常和 GC 行为。delegate storage 使用既有字段布局和扫描，不创建运行期代理机制；构造期间提前访问仍服从现有初始化限制。泛型 I 宿主允许，interface 方法自身泛型仍禁止。

### 7.3 inner class

`inner class` 只允许声明在 class 的实例作用域中，拥有显式的外层 class 引用；不用于 struct、enum、interface、object/companion 的成员类型。普通 static nested 类型的规则不变。

inner 类型可以使用外层 class 的类型参数和实例成员，自己的类型参数独立绑定。完整类型写作 `Outer<A>.Inner<B>`，外层实参与 inner 自身实参共同确定 application；外层对象的运行期 identity 不参与类型身份。

通过 `outer.Inner(args...)` 构造时先求值并保存 outer，再执行普通构造实参协议；在已具有相应外层 receiver 的作用域中可省略显式 outer。每个 inner 实例有普通 typed 外层引用字段，`this` 指 inner，`this@Outer` 选择外层实例。该字段延长外层对象的可达期，沿现有 GC 扫描和布局处理。

`outer::Inner` 创建绑定外层实例的构造器引用，outer 在创建引用时求值一次。`::Inner` 只有在创建点能明确获得所需外层 receiver 时才合法；不生成没有外层实例的构造入口。outer 为局部 `var` 时，这种显式创建时读取仍是快照，不是 body 对该 var 的自由变量捕获。

### 7.4 局部类型与匿名 object

局部块允许具名 class、struct、enum、interface。名字在自身 body 及声明之后的词法作用域可见，不依赖后声明局部类型的前向查找；nominal body 中的成员仍按原有成员收集规则处理。局部声明不能声明 public/internal/protected/private 可见性。

局部 class 可以按第 2.3 节捕获可见的不可变运行期 binding，在每次构造时复制到本实例的普通隐藏字段；构造器引用则在创建引用时保存所需的捕获值。局部 struct/enum/interface 可以使用外层类型参数，但不能捕获运行期局部 binding：它们不因此新增隐式 value payload 或接口实例环境。

outer 和捕获值在构造入口作为已保存的隐藏值参数传入，构造委派实参通过这些参数读取。对应隐藏字段在 base constructor 完成后、当前 class 的 primary property 和 delegate 初始化前写入；不能通过尚未就位的字段读取它们。分配和 base constructor 期间这些值沿既有参数/root 规则保持可达，不放宽构造过程中访问未初始化实例的限制。

匿名 object 写作 `object { ... }` 或 `object : Base(args), I { ... }`，创建普通的匿名 final class 实例。每次求值创建新对象，base 实参、delegate、captured val、property 和 init 的求值遵守对应的构造顺序；匿名类没有另行声明的构造器。其成员和 override 按普通 class 规则检查。

```scoop
fun writer(prefix: String): Writer {
    return object : Writer {
        override fun write(value: String) {
            println(prefix + value)
        }
    }
}
```

匿名类型可保留在局部变量推导中，使局部代码访问其自有成员。非局部函数/property 的签名不能推导并暴露不可命名的匿名结果，需显式标注一个可见父类型；具名局部类型也不能通过推导越过其有效作用域。通过显式 interface/基类或 Any 返回对象是正常引用适配。

局部/匿名 class 同样拒绝捕获外层局部 `var`，不因对象在当前函数内立即使用而例外；sealed 的直接子类型也不能借局部或匿名声明绕过限制。不加入局部具名 object 的另一套 singleton 生命周期；局部对象使用匿名 object 表达式。

类型身份由实际源声明位置的词法 owner 和封闭的类型实参组成，不因函数调用次数产生新类型。同一匿名表达式在同一泛型 application 下每次执行产生不同对象，但类型相同。跨 Cone 泛型 body、默认值和再次发布必须保存必要的局部声明与 typed 引用，不导出局部名字，也不以 FQN 或消费方文件位置重新生成身份。

## 8. 正常出口与流敏感 smart cast

M35 的多类型事实和分支内 smart cast 是本节起点。M37 将事实传播到继续执行的正常路径：

```scoop
fun length(value: Any): Long {
    if (value !is String) return 0L
    return value.length
}
```

if 的已退出分支不进入后续事实交集，上例的继续路径知道 value 是 String。类似规则应用于 when、短路条件、循环、try/catch/finally 和合法跳转，但只基于已经证明的控制结果，不猜测普通调用是否会返回。

- `is` / `!is`、否定、短路与分支选择产生对应真假路径事实；多个正常前驱合流时只保留共同成立的事实。return、throw、break、continue 的真实目标决定事实流向。
- while/for/do 的 body、条件、回边、continue 和 break 出口分别处理；循环后不能只使用条件为 false 的事实而遗漏 break。循环分析在当前程序涉及的有限 binding/type 事实中求稳定结果，不增加通用路径预算。
- `finally` 的写入或新控制转移可以撤销/替换进入它之前的事实；挂起恢复沿既有局部值和控制状态继续，不能把挂起当作函数结束。
- 普通局部 `var` 可以在同一 callable 内获得 smart cast，但每次赋值都使旧值事实失效；循环回边与合流必须考虑可能写入。可经已暴露地址或其他现有机制间接写入的 binding 不按稳定局部处理。
- 可变 property、custom getter、delegate getter 和一般成员访问链仍不是稳定局部 binding；不以重复读取“看起来一样”推导其运行期值相同。
- 不可变捕获值和构造器参数与普通稳定局部采用相同的快照规则；when 保存 subject 后，来源与快照在值未改变的路径上共享类型事实，分支合流仍只保留共同事实。

允许局部 var 的流分析不改变捕获限制。创建 lambda 时只能复制允许捕获的不可变 binding 及其成立的类型视图；绝不能先把 var 的 narrowed view 包装成 hidden val 再放行捕获。

实现复用既有 BindingId、类型事实、完成性质及受类型检查的成员视图；多个 interface 事实不升级为语言可命名的一等交叉类型。不加入 contracts、用户可执行的流分析规则或第二套独立语义检查器。

## 9. 整型枚举与静态用户注解扩展

### 9.1 限制值域的整型枚举

`enum E: I` 声明一个限制值域的整型子类型。`I` 必须解析为已有八种 primitive integral type 之一，透明 alias 按实际类型处理；不是任意值类型、浮点、Boolean、Char、类型参数或另一个 enum。`E <: I` 是真正的子类型关系，`E` 同时保留独立的 nominal identity，不是 `I` 的 typealias。

```scoop
enum IntFlags: Int {
    Flag1 = 1,
    Flag2 = 2,
    LastFlag = Int.MAX_VALUE
}
```

#### 9.1.1 声明与合法值

- 每个 variant 必须显式指定一个编译期整数常量，不允许任何 payload 或字段，也不按声明顺序隐式编号。常量沿用已有 `const val` 表达式规则，可引用 `Int.MAX_VALUE` 等 core 常量；相关整数边界常量是本功能需要的普通 core 配套。
- initializer 按底层整型检查，值必须可精确表示；越界报错，不通过隐式截断或改变 signedness 适配。常量表达式自身的运算仍遵守既有整数规则。同一枚举的 variant 数值不得重复。
- `E` 的合法值仅为声明的 variant 值。合法底层整数不因此自动成为合法 `E`；类型检查不按数值恰好命中某个 variant 来缩窄类型。
- 未指定整型父类型的普通 enum 保留既有代数数据类型规则，包括纯 unit enum。注解参数通过已确定的枚举类别选择可接受类型，不在每个使用处重新遍历 variant 判断是否含 payload。

#### 9.1.2 上转、父类型运算与类型检查

`E` 可以在赋值、实参、返回值等普通位置隐式上转为 `I`。上转按值产生一个 `I`，不保留枚举来源；表示相同，不需要数值运算或装箱。`I` 到 `E` 的隐式 downcast 不成立，字面量、常量传播和期望类型推导也不能绕过这条规则。

整型枚举自身不定义或生成专属数学运算。使用父类型的算术、位运算或数值方法时，按 `I` 的既有成员签名决议；签名中的 `I` 不替换为 `E`，结果仍为方法实际声明的类型。`e + 1` 因而可以得到 `I`，不能得到一个未经构造的 `E`；比较等方法继续返回其既有结果类型。

```scoop
val e: IntFlags = IntFlags.Flag1
val raw: Int = e
val next: Int = e + 1

val invalid: IntFlags = 1 // Error: implicit downcast.
var current: IntFlags = IntFlags.Flag1
current = next           // Error: implicit downcast.
current += 1             // Error: the Int result cannot be assigned back.
current++                // Error: the Int result cannot be assigned back.
```

这些写回错误由普通赋值检查产生，不为算术结果新增运行期枚举值域检查，也不把枚举名中的 Flags 解释成额外的组合标志语义。

该父子关系参与既有重载、分支合流、函数值方差和接口继承。底层整型的实例成员与接口关系可按父类型使用；例如继承的是 `Equality<Int>`，不会把它自动替换成 `Equality<E>`。同一底层整型的不同枚举仍有不同身份，不能隐式互转；不同位宽或 signedness 的整型之间也不因此新增隐式提升。

普通泛型实例保留实际 `E` 身份。`Array<E>`、`MutableArray<E>`、`Option<E>` 等继续遵守 nominal 不变性，不能上转为对应的 `I` 实例；本项不扩大泛型上界可接受的类型类别。`when` 的 subject 为 `E` 时按完整 variant 集合检查穷尽性，上转为 `I` 后按整数值域检查，不保留枚举穷尽性事实。

#### 9.1.3 装箱与运行期类型关系

装箱使用装箱时的值类型。直接装箱 `E` 保留其 exact identity；先上转为 `I` 再装箱，箱子保存的是 `I`。同一数值的两个箱子不因此具有相同的类型。

```scoop
val e: IntFlags = IntFlags.Flag1
val raw: Int = e
val a: Any = e
val b: Any = raw

val aIsEnum = a is IntFlags // true
val aIsInt = a is Int       // true
val bIsInt = b is Int       // true
val bIsEnum = b is IntFlags // false
```

`is`、`as`、`as?` 及函数值适配必须使用一致的父子关系。从直接装箱的 `E` 中取回原枚举或按父类型取得 `I` 都合法；从实际装箱的 `I` 中不能仅凭整数值相同取出 `E`。这属于已存在值的类型判断与解箱，不增加按整数内容构造枚举的 downcast 入口。

#### 9.1.4 布局、ABI 与产物

`E` 的 size、alignment、数值位表示和按值调用 ABI 均与 `I` 一致；参数、返回值、字段与数组元素使用该标量表示，没有额外 tag 或 payload 区域。底层整型的 signedness、窄整数扩展和目标 C ABI 分类同样保留。未使用的整数值不成为本里程碑新增的 Option niche，`Option<E>` 沿用既有布局规则。

原生接口声明接收或返回 `E` 时须遵守其合法值域；可能产生未知数值的接口应以 `I` 表达，再通过显式分支选择合法 variant。ABI 相同不放宽值域，也不使 `Ptr<E>` 等不变泛型自动成为 `Ptr<I>`。正常复制、上转和调用不反复检查定义处已经确定的值域。

HIR 以明确的整型枚举类别保存底层整型身份和已检查的 variant 常量；子类型、成员与后续 lowering 消费该事实。MIR/LIR/codegen 使用标量表示，同时保留源码枚举身份。共享元数据保存完整 enum/variant identity、底层类型与数值，跨 Cone 编译、单态化、重载、装箱和静态注解不能把 `E` 擦成普通 `I`；机器签名相同也不合并两者的源码身份或符号。

### 9.2 参数值域

在 M35 标量及其一维数组基础上，增加以下静态原子值：

- 第 9.1 节整型枚举的 variant，按实际 enum application、variant identity 和底层整数值保存；其一维数组同样合法。普通 enum 即使全部 variant 均无 payload，也不属于注解参数允许的枚举类别。
- 另一个 annotation class 的完整静态应用，使用 `AnnotationName(...)` 语法，按该声明的参数、默认值和可见性绑定。

Array 的元素可以是任一合法注解原子类型，仍只有一维；嵌套 annotation 自己可以有数组参数，但不因此允许 `Array<Array<T>>`。非整型枚举（包括普通 unit enum 和 payload enum）、Option、Any、MutableArray、普通对象和类型字面量仍不是注解值。

```scoop
enum Level: Int { Info = 0, Warning = 1 }
annotation class Message(val text: String)
annotation class Rule(val level: Level, val message: Message)

@Rule(Level.Warning, Message("Check the input"))
fun validate(value: String) {}
```

上述 `Message(...)` 只构建静态数据，不是 runtime constructor call。注解没有泛型参数、继承、普通 body 或可执行成员。annotation 参数类型与默认值的递归依赖必须无环，错误在定义处定位；不使用任意深度限制代替结构检查。

所有值按实际参数类型正规化，保留 enum/annotation 身份、有序数组、默认值补全及元素源码位置。以枚举为期望类型时必须提供该类型的合法静态值，不能用底层整数或另一枚举的同值 variant 隐式替代；参数为其整型父类型时可按第 9.1 节上转。普通 const 值域不随此扩展，不能借嵌套 annotation 执行一般函数或初始化 singleton。

### 9.3 声明目标与 use-site target

在既有 nominal、enum variant、字段和 logical property 目标上，增加 annotation class、具名函数/方法、constructor、普通值参数、accessor、receiver、typealias 以及类型使用位置。标注 annotation class 使用普通注解应用规则，不增加独立的 meta annotation class 声明种类。参数解构上的注解属于完整源码参数，不自动复制给各叶 binding。局部具名声明和跨 Cone 模板使用同一规则。

显式 use-site target 定位实际存在的实体：

| target | 实际目标 |
| --- | --- |
| `property` | logical property |
| `field` | 已存在的 stored backing field 或值类型字段 |
| `get` / `set` | 已存在的 getter / setter |
| `param` | 对应的普通/构造参数，特别是主构造 property 的参数部分 |
| `setparam` | setter 的值参数 |
| `receiver` | 声明中的实际 receiver 参数 |
| `delegate` | delegated property 已有的 delegate storage |

不存在对应实体时直接报错，例如 computed property 的 `@field:`、val 的 `@set:`；注解不能反过来合成 backing field、setter 或 delegate。未写 use-site target 时沿用原目标映射，主构造 `val/var` 仍归属字段/property，不按注解内容自动改成参数或 storage。

type-use annotation 保存在完整类型使用树的对应位置，含泛型实参、函数 receiver、普通参数和结果位置。它不改变类型身份、子类型、重载、layout 或 ABI；alias 展开和泛型替换保留对应使用位置，不把一次使用的注解污染到共享 TypeId 的所有使用。

本次不加入文件目标、局部变量目标、任意表达式目标、注解继承、运行期 Retention 或处理器/plugin API。匿名表达式不是为了容纳注解而新建可公开查询的声明。

### 9.4 目标约束与可重复注解

以真实 core 注解声明提供 `@Target` 与无参数的 `@Repeatable`，以及描述本节支持目标的整型枚举 `AnnotationTarget: Int`。它们只有固定的编译期规则，复用现有核心注解身份绑定；同名用户声明不获得特殊作用，不建立元注解执行机制。

```scoop
public enum AnnotationTarget: Int {
    CLASS = 1,
    ANNOTATION_CLASS = 2,
    ENUM_VARIANT = 3,
    FIELD = 4,
    PROPERTY = 5,
    FUNCTION = 6,
    CONSTRUCTOR = 7,
    VALUE_PARAMETER = 8,
    PROPERTY_GETTER = 9,
    PROPERTY_SETTER = 10,
    TYPEALIAS = 11,
    TYPE = 12
}

public annotation class Target(val allowed: Array<AnnotationTarget>)

@Target(allowed = [AnnotationTarget.CLASS, AnnotationTarget.FIELD])
annotation class SomeAnnotation(val text: String)
```

AnnotationTarget 的成员为 `CLASS`、`ANNOTATION_CLASS`、`ENUM_VARIANT`、`FIELD`、`PROPERTY`、`FUNCTION`、`CONSTRUCTOR`、`VALUE_PARAMETER`、`PROPERTY_GETTER`、`PROPERTY_SETTER`、`TYPEALIAS`、`TYPE`。CLASS 覆盖普通 class/struct/enum/interface/object；annotation 声明单独使用 ANNOTATION_CLASS。use-site 的 receiver 和 setparam 属于 VALUE_PARAMETER，delegate storage 属于 FIELD，get/set 分别属于 PROPERTY_GETTER/PROPERTY_SETTER；TYPE 表示类型使用位置。目标由实际被标注的实体确定。

`@Target` 的参数为 `val allowed: Array<AnnotationTarget>`，约束自定义 annotation class 的合法目标；未写时允许本节支持的目标，空数组表示不允许任何应用位置。重复/未知目标及用在错误声明上的核心元注解在源码处诊断。Target 和 Repeatable 自身只用于 annotation class 声明。允许目标不改变上一节的默认 use-site 映射，必要时源码显式写出目标。

标有 `@Repeatable` 的 annotation 可以在同一实际 target 上重复应用，保留源码顺序和重复值；没有该标记时，同一实际 annotation 声明的重复应用仍报错，alias 或 re-export 不能绕过。不同 use-site target 分别检查，不隐式复制或继承。无需 JVM 式 container annotation。

注解应用进入已有静态描述和必要的共享声明/body 元数据；新增目标需要相应的实际描述入口，不能只让 parser 接受后丢弃。跨 Cone 读取和再次发布使用已绑定 typed 常量与目标，不重跑名称查找、default 求值或用户代码。描述数据不会扩大被描述声明的源码可见性。

## 10. Unicode 与反引号标识符

普通标识符采用 Unicode `XID_Start` / `XID_Continue`，另外允许 `_` 作为起始字符；关键字仍按实际拼写识别。实施使用成熟 Unicode 标识符库，依赖版本在正常实现审查时确定，不维护手写字符范围表。

反引号将内部文本作为一个标识符，例如 ``val `when` = 1``、``fun `check input`() {}``。内部不能为空，不能含反引号、换行、NUL 或控制字符；不执行反斜杠转义。被包裹的关键字不再触发语法规则，包含标点的名字也不拆成 package/member 路径片段。

```scoop
fun 合并(左: Int, 右: Int) = 左 + 右
val `when` = 合并(1, 2)
```

标识符按实际 Unicode 码点序列比较，不静默执行大小写折叠或规范化。普通 `name` 和反引号包裹的同一文本是同一名称；是否写反引号不产生第二个实体。仅源码解析需要保留转义信息，元数据里的名称保留真实文本和已解析的结构。

span/诊断继续使用明确的 UTF-8 位置规则，不能用字符数量误当 byte offset。package/import、alias、成员查询、命名参数、导出与再次发布均使用同一名称规则；符号 mangling 使用已有结构化身份并正确编码名称，不拼接一个含点字符串充当真实 owner。FFI 外部符号名继续使用其独立的既有规则。

## 11. 各阶段、实体与产物约束

| 阶段 | 必须完成的输出 |
| --- | --- |
| parser / AST | 正确区分控制 body、lambda、label、receiver 类型、alias 参数、整型枚举的底层类型与显式数值、nominal 形态及注解目标，保留实际源码位置 |
| hir-lower 声明处理 | 完成签名推导、类型展开、整型枚举值域与父子关系、继承、capture 与访问域检查；忽略五个兼容修饰符 |
| HIR / shared interface | 所有签名和使用类型完整；receiver 形状、sealed 子类型、整型枚举类别/底层类型/variant 常量、局部 nominal、alias binder、注解目标和值均具有真实 typed 引用 |
| hir-lower body / MIR | 选择好 callable 与跳转目标；整型枚举上转、父类型运算和装箱类型关系明确；循环入口、正常结果、finally/Context 和 suspend 路径准确 |
| LIR / codegen | 整型枚举采用底层整型的标量布局与调用 ABI，完成父类型解箱；按既有分类处理 receiver、ZST、值复制与 GC 描述 |
| slib / driver / linker | 发布、消费、模板实例化、re-export 与再次发布一致；完整保留整型枚举身份、底层类型和数值，不要求下游重做定义处常量求值或语义检查 |
| core | 补齐整型枚举所需的整数边界常量（如 Int.MAX_VALUE）、注解目标/可重复性声明及必要静态描述；Cell 仍是独立的未来库议题 |

沿用全局唯一的类型化实体 ID。nominal template、具体 application、callable specialization、loop/return target、property/accessor/storage 和源码参数不能混用身份；局部/匿名类型与新合成转发应归属真实 owner，不使用 FQN fallback 或消费方重新生成的声明。

stage 只通过相应 IR/meta crate 通信。parser 只处理语法，不查询符号表猜模式、receiver 或推导结果；MIR/codegen 不重新做重载决议、捕获检查或 sealed 覆盖。格式与引用合法性在既有边界验证，同一未变化数据不在各 stage 重放完整语义检查。

多数交付使用现有函数值、对象、itable/vtable、异常与 GC 模型。仅在实际编码变化时升级相应 meta/section 版本，并让旧产物按既有版本检查明确失效；不预先规定全链路统一升版，不增加没有消费者的通用 schema/插件层，也不为语言功能附加授权、证明或资源计费机制。

## 12. 规范与兼容性迁移

实施按 spec 先行。首批先核对 M35 的最终规范、M36 完成后的数据合同和回归，再按功能批次修订以下内容；本次仅列清单，不修改这些文件。

| 文档 | 后续修订内容 |
| --- | --- |
| 语言规范 2、3.2、3.2.1 | 语法能力列表、整型枚举的子类型与既有泛型不变性、明确的泛型方法禁令、泛型/嵌套/局部 alias、标识符规则 |
| 语言规范 4.2、4.4、7.4、11.2、13.8 | 整型枚举声明与值域、上转及隐式 downcast 禁令、父类型运算结果、装箱判型与解箱、整数边界常量、底层整型 ABI；保留既有 Option niche 范围 |
| 语言规范 5、8.7、8.8 | sealed when 覆盖、单语句体、do/while、标签、Nothing 跳转和出口事实 |
| 语言规范 8.1、8.4、8.5 | 函数结果推导、receiver 函数值、引用目标、永久捕获边界、五个完全忽略的修饰符 |
| 语言规范 9.1、9.2、9.4、9.6 | 属性推导、sealed/inner/local/anonymous 类型、接口委托、整型枚举注解参数、annotation class 目标、Target/Repeatable 与静态描述 |
| 语言规范的明确排除列表 | interface 方法自身泛型、泛型虚方法、捕获外层局部 var；不能与正文保留相反的未来承诺 |
| 实现规范 | 推导发生的阶段、局部实体归属、整型枚举的完整表示/子类型/标量 ABI、共享签名/注解/继承数据和实际格式变化 |
| 运行时规范 | 整型枚举的布局、装箱类型关系与父类型解箱；核对新增字段、函数 receiver 与 GC/初始化映射，仅修改确实变化的契约 |
| ROADMAP | 登记 M37；删除已完成项的过期缺口；将两项明确拒绝能力移出待实现列表，Cell 只保留为库建议 |

新增语法以前多为明确错误；应替换相应旧 negative fixture，并保留边界错误。表达式体函数和 const/property 的省略类型会改变原先“必须显式标注或默认为 Unit”的行为，需在规范中直接说明，不做静默兼容模式。

公开声明的推导类型与显式类型一样属于源码接口，initializer/body 变化可能改变导出签名；sealed 的直接子类型变化同样影响穷尽性。两者使用正常签名/接口依赖失效，不新增另一套发布机制。未变化的 M35 语义通过已有 fixture 回归。

## 13. 实施批次

各批次纵向完成语法、语义、必要元数据和 CLI fixture，再进入下一批；不先合并一批尚无后端出口的 AST 占位节点。

| 批次 | 内容与完成条件 |
| --- | --- |
| M37-1 | 核对 M35 正式验收及 M36 完成后的基线；规范固定三项决策；五个修饰符解析后忽略，禁止项诊断及合法相邻能力有 fixture |
| M37-2 | Unicode/反引号标识符贯通词法、名字与产物；单语句控制 body 和 else-if 完成解析与执行 |
| M37-3 | do/while、循环标签、局部返回标签和 break/continue 表达式；完成异常、finally、Context、suspend 组合 |
| M37-4 | 函数结果与属性推导；定义处依赖、递归诊断、泛型及跨 Cone 签名完成 |
| M37-5 | 构造器/未绑定成员引用、receiver 函数值与 this 标签；尾随 lambda、重载、方差与 GC 组合完成 |
| M37-6 | 泛型/嵌套/局部 alias，inner、局部类型及匿名 object；完整身份、捕获与产物实例化完成 |
| M37-7 | sealed 覆盖与接口实现委托；泛型宿主、override/default、初始化和依赖消费完成 |
| M37-8 | 在新控制结构上完成正常出口 smart cast、局部写入失效与循环合流，复用 M35 类型事实 |
| M37-9 | 整型枚举声明与常量、真正子类型/上转、父类型运算、装箱判型/解箱、底层整型布局与 ABI、跨 Cone 与独立 C 互调完成 |
| M37-10 | 在整型枚举基础上完成 enum/嵌套 annotation 参数、annotation class 目标、Target/type-use/可重复性、core 描述和跨 Cone 静态数据 |
| M37-11 | 全量回归、真实 CLI debug/release、跨 Cone 再次发布、GC 与支持平台验收；按实际结果记录完成 |

每批实现先修改对应 spec，再修改代码；一批代码完成后先格式化/lint，再验证。依赖只因当前功能的真实复用引入；不能把单态化、捕获、GC 或元数据重写成无关的新框架。

## 14. 验收矩阵

每项新增语言能力应至少有独立的 positive fixture、覆盖明确错误规则的 negative fixture，以及与其他特性组合的 fixture。诊断断言实际源码位置与信息；HIR/MIR/LIR golden 锁定必要结构，最终结果以正式 CLI 编译、链接与运行闭环为准。

| 能力 | 正向与组合覆盖 | 必须拒绝或保持的边界 |
| --- | --- | --- |
| 五个修饰符 | 各合法语法位置、混合/重复标记、非递归 tailrec、无 inline 的 reified、去除标记后相同语言行为 | lambda 裸 return、非法虚方法、var 捕获不因标记放行；IR/LLVM 不产生相关语义标志或 hint |
| 单语句 body | 多层 else-if、dangling else、声明作用域、尾随 lambda、if 表达式和 Nothing | 缺少正常分支值、错误 body 边界、名字泄漏 |
| do/label/jump | body 至少一次、condition setup、continue 条件入口、外层 break、Elvis jump、局部 return、finally/Context/suspend | 未初始化的 condition 读取、不存在/错误种类的标签、跨 callable 跳转 |
| 声明推导 | top/member/local/extension、const、getter/delegate、泛型结果、override、定义处调用绑定、公开签名消费 | 推导环、无定型来源、可见性泄漏、块体具名函数误推非 Unit、参数默认值反推签名 |
| 引用与 receiver | 构造重载、泛型 final 方法、动态分派、value receiver、解构普通参数、适配与运行期函数检查 | 缺失泛型实参、构造不可构造类型、经引用省略默认参数、suspend 静默转换、FunPtr 越界 |
| alias | bound、构造推导、nested/local scope、receiver 函数别名、跨 Cone 链和再次发布 | 递归展开、非法宿主参数捕获、不可见目标、裸 generic、同形重载 |
| sealed | 完整子类型家族、sealed 中间节点、generic exact application、多接口路径、跨 Cone when | 外部 Cone/package 直接继承、local/anonymous 直接子类型、guard 假穷尽、只枚举额外泛型参数的少数实例 |
| interface by | 多个接口、property、默认参数、delegate default、显式 override、泛型宿主、初始化异常与 GC | delegate 歧义、错误类型、缺失 slot、未初始化 this、value type 隐式状态、interface 方法自身泛型 |
| inner/local/object | 多层 outer、外层泛型、不同实例同类型、local generic body、捕获值复制和引用存活、构造器引用 | 缺失 outer、var 捕获、static nested 偷捕 outer、local/anonymous 类型泄漏 |
| smart cast | 提前退出、短路、when 正常出口、loop break 合流、局部赋值、try/finally、M35 多接口捕获 | 旧 var 事实未失效、不稳定 getter 假设、把 narrowed var 当可捕获 val |
| 整型枚举 | 八种底层整型与 alias、边界常量、上转、父类型运算结果、重载/合流/函数值适配、直接及上转后装箱、接口继承、穷尽性、布局与 ABI、跨 Cone 和 C 互调 | 非整型底层、payload、缺少显式数值、越界/重复值、字面量或写回的隐式 downcast、跨枚举互转、容器协变、按整数值命中恢复枚举身份 |
| 注解 | 整型枚举/嵌套值与数组、默认值、annotation class 目标、Target/use-site/type-use/repeatable、有序重复、再次发布 | 非整型枚举（含普通 unit enum）、整数或异类 enum 隐式替代参数、多维数组、递归 schema、不存在目标、错误 Target、非法重复、任意执行 |
| 标识符 | 中文/其他 Unicode、转义关键字、命名参数、package/import、alias、成员、导出和诊断位置 | 空/未闭合反引号、控制字符、码点/字节位置混淆、同文本重复声明 |
| 永久语言边界 | generic interface 宿主、非泛型虚方法、final 泛型方法、val 引用对象的成员修改 | interface 方法自身泛型、任何泛型虚方法、任何外层局部 var 自由变量捕获 |

至少增加以下完整组合链：

1. sealed 泛型 interface + 匿名实现的普通非 sealed 子接口 + when guard/smart cast + receiver lambda；验证开放子家族和封闭根覆盖没有混淆。
2. do/while + 标签跳转 + `?: continue` + try/finally/Context + 显式 suspend；验证 continue 恢复到条件入口，清理不重复。
3. 泛型 alias + 推导结果的函数 + 构造器/成员引用 + 普通依赖/re-export/再次发布；验证绑定、完整签名和 origin 一致。
4. inner/局部 class + captured val/value type + interface by + 老对象引用新对象的 GC 场景；验证普通字段扫描、写屏障和对象存活，不生成 mutable capture box。
5. Unicode/反引号名称 + 命名参数/尾随 lambda + 整型枚举/嵌套注解/Target/type-use + 跨 Cone 静态描述；验证源码名字、调用映射、目标约束和静态数据均能闭环。
6. 整型枚举 + 父类型参数/返回与函数值适配 + 直接及上转后装箱 + 跨 Cone 再次发布与独立 C compiler 互调；验证源码类型身份、合法上转、完整数值和底层整型 ABI 一致。

正式验收沿仓库既有流程执行：`cargo fmt --all`、`cargo clippy --workspace --all-targets` 后运行适用 Rust 测试；构建 scoop/scoopc/scoop-linker 三个配套命令，执行 fixture runner 的 unittest 与 `python3 tests/run_fixtures.py --all`。Python 若有变更，先执行仓库规定版本的 Ruff format/check。完整验收还须包含 debug/release 和实际支持平台的运行结果；未运行的平台不能记录为通过。

设计文档落地不代表上述验收已完成。M37 结束条件是范围内能力与边界通过实际验证，而不是新增了所有关键字、更新了 golden 或积累了新的验证机制。
