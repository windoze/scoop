# M35 设计：when、Elvis 返回、尾随 lambda 与注解数组

状态：2026-10-10 实施与验收完成；实际过程见 [PROGRESS.md](PROGRESS.md)，范围与结果见 [ACCEPTANCE.md](ACCEPTANCE.md)。

日期：2026-10-10。

基线：M34 已完成的源码编译、跨 Cone 产物消费、debug/release 与分代 GC。语言行为以[语言规范](../specs/SCOOP-SPEC.md)为实施起点，阶段职责与产物约束见[实现规范](../specs/SCOOP-IMPL-SPEC.md)，类型检查、闭包、异常与 GC 契约见[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)。

本文定义 M35 的交付范围；实施按 spec 先行推进，完成状态以实际源码、产物及验收记录为准。

## 0. 交付目标与已确定的选择

M35 补齐以下四项可从真实源码编译、链接并运行的语言能力：

1. Kotlin 风格的 `when`：有 subject 的值、类型、成员关系匹配，无 subject 的条件分支，多条件、guard、subject 局部声明（包括 Scoop 的解构声明）、分支内 smart cast，以及语句和值位置的穷尽性与结果类型规则。
2. `expr ?: return ...`：`return` 可以进入表达式位置，使用 `Nothing` 表达无正常结果；与 `throw`、已有底类型调用及清理路径一致。
3. Kotlin 规则的尾随 lambda：最后一个实参可移到圆括号之外；圆括号内没有实参时可省略圆括号。适用性由完整的参数映射决定，不限制为“函数只有一个 lambda 形参”。
4. annotation class 的一维数组参数：支持 `Array<T>`、`[...]`、空数组、默认值和标量常量元素，并贯通静态描述及跨 Cone 元数据。

值匹配与模式绑定采用明确的语法分界：**普通 arm 使用表达式语义；只有以 `case pattern` 开头的 arm 使用模式语义并引入绑定。** 不再根据 subject 的静态类型选择两种解析方式，也不根据名字是否已声明猜测它是值还是绑定。头部的 `val pattern = expression` 沿用声明位置的 binding pattern，不需要 case。

Kotlin 兼容指这些语法与调用规则在 Scoop 现有类型系统中的行为。Scoop 继续使用 `Option<T>`、现有成员 `operator equals`、显式 `suspend` lambda 和禁止 lambda 裸 `return` 的规则。`sealed` 类型、receiver function type、label 与 inline non-local return 属于独立能力，不作为本里程碑的前置交付。

## 1. 基线缺口与实施起点

| 能力 | 基线实现 | M35 必须完成的变化 |
| --- | --- | --- |
| `when` 语法 | subject 必选；所有 arm 都进入 pattern parser；单 arm 只有一个 pattern | 在 AST 中区分普通条件与 `case`；支持无 subject、subject 普通/解构局部声明、多条件和完整 arm body |
| `when` 类型检查 | `check_pattern_subject` 仅接收 enum、tuple、struct 和定宽整数；Float/Char 等已有能力经过各自类型表示接入 | 去掉普通 `when` 的模式类型门槛；分别检查值、`is`、`in`、Boolean 条件和模式 |
| smart cast 与穷尽性 | 已有局部类型事实和递归 pattern matrix，但 guard 的事实没有完整进入 arm body；fallback 只有 else 或已证明不可达 | 复用并补齐分支事实传播；支持合法的语句落空；保留模式覆盖能力 |
| Elvis 返回 | parser 把 return 当语句；Elvis 要求右侧与 payload exact type 相同，并给两条路径都生成结果赋值 | 接入跳转表达式、底类型和正常完成分析，只合并实际产生结果的路径 |
| 尾随 lambda | 只向已经解析出的调用追加普通位置实参；裸 `f {}` 可能落到 infix invoke；泛型探测要求 `(` | 统一调用后缀；保留尾随来源；按每个候选的最后形参映射 |
| 注解数组 | AST、常量绑定与元数据均为标量；参数类型只保存非泛型 nominal identity | 增加静态数组值和完整 `Array<T>` 类型引用，贯通导出、读取与再次发布 |

主要定位：

- `when`：[AST](../../compiler/ast/src/statements.rs)、[parser](../../compiler/parser/src/stmt/control_flow.rs)、[类型与覆盖检查](../../compiler/hir-lower/src/stmt/when.rs)、[smart cast 分析](../../compiler/hir-lower/src/expr/analysis.rs)、[分支名字解析](../../compiler/hir-lower/src/expr/names.rs)。
- Elvis 与跳转：[表达式 parser](../../compiler/parser/src/expr.rs)、[语句 parser](../../compiler/parser/src/stmt.rs)、[Option 运算脱糖](../../compiler/hir-lower/src/expr/fields.rs)、[值块处理](../../compiler/hir-lower/src/stmt/blocks.rs)、[MIR 控制转移](../../compiler/mir-lower/src/cfg/control.rs)。
- 尾随 lambda：[primary parser](../../compiler/parser/src/expr/primary.rs)、[调用 AST](../../compiler/ast/src/expressions.rs)、[候选实参映射](../../compiler/hir-lower/src/call_resolution/arguments/mapping.rs)。
- 注解：[语法值](../../compiler/ast/src/annotations.rs)、[值 parser](../../compiler/parser/src/decl/annotations/values.rs)、[声明绑定](../../compiler/hir-lower/src/user_annotations/binding.rs)、[常量绑定](../../compiler/hir-lower/src/user_annotations/values.rs)、[产物映射](../../compiler/hir/src/production/annotations.rs)、[共享注解格式](../../compiler/hir/src/cross_cone_interface/annotations.rs)、[slib 注解读取](../../compiler/slib/src/cross_cone_hir_authority/annotations.rs)。

## 2. `when` 的源码语义

### 2.1 subject、作用域与基本形式

支持三种入口：

```scoop
when (value) {
    expected -> onEqual()
    makeExpected() -> onComputedValue()
    else -> onOther()
}

when {
    ready && allowed -> start()
    retries > 0 -> retry()
    else -> stop()
}

when (val item = load()) {
    is String -> println(item.length)
    else -> println("other")
}
```

有 subject 时，先求值 subject 且只求值一次，保存该值供后续所有条件使用。各 arm 按源码顺序尝试，进入第一个成功 arm 后结束本次 `when`，没有 fallthrough 到后续 arm 的语义。

`when (val pattern [: Type] = expression)` 声明一个或多个不可变局部变量，复用语言规范 4.6 的 binding pattern。普通名字是最简单的 pattern；tuple/struct 位置解构、struct 字段与重命名、`_`、`..`、嵌套解构以及 class 的 component 位置解构，均按已有声明规则处理。可选类型标注约束初始化表达式的完整值，不是各叶 binding 的单独类型标注。Kotlin 本身的 subject 声明只接受单个名字；此处的解构是 Scoop 与既有局部声明保持一致的扩展。

```scoop
struct Point(val x: Int, val y: Int)

fun describe(point: Point): String {
    return when (val (x, y) = point) {
        Point(0, 0) -> "origin"
        case _ if x == y -> "diagonal"
        else -> "other"
    }
}
```

头部解构只建立局部名字，**arm 的 subject 仍是初始化后保存的完整值**。上例匹配的是原 Point，不是 x、y 中的某个值，也不是重新构造的 tuple；普通值、is、in 和 case 条件都继续作用于该 Point。头部还可以写成 `val Point { x, y } = point`。若 arm 需要根据解构后的名字判断条件，使用 guard 或完整的嵌套无 subject when；普通 arm 不会因此改成 Boolean 谓词。

求值顺序为：初始化表达式一次求值并按可选类型标注完成适配，保存为稳定 subject；按已有 binding 计划完成解构；随后才尝试第一个 arm。解构产生的投影或 component 调用不随 arm 重试。component 抛出时不尝试任何 arm，挂起后从原位置恢复；求值顺序、副作用与清理复用 4.6 的规则。

所有头部 binding 在全部条件、guard、body 和 else 中可见，离开 `when` 即失效。初始化表达式使用外层作用域，不能引用这次声明中尚未建立的任何绑定；重复名字、遮蔽和嵌套作用域按已有局部声明规则处理，case 的 arm 内绑定仍属于更内层作用域。

头部解构必须递归不可失败：`when (val Some(x) = option)`、`when (val (0, x) = pair)` 等仍为编译错误；需要可失败模式时放入 case arm。不可失败指没有模式不匹配分支，不排除 component 的正常异常或挂起行为。不接受 `var`、delegate 或 accessor。

无 subject 时，每个普通条件都是 Boolean 表达式，遵循现有底类型子类型规则。`is T`、`in xs` 与 `case pattern` 都需要 subject，不能直接作为无 subject arm 的头部；应写完整的 `x is T` 或 `x in xs` 条件。

### 2.2 普通条件：值、类型与成员关系

| arm 头部 | 含义 |
| --- | --- |
| `expression` | 使用已保存的 subject 作为左操作数，执行 `subject == expression` |
| `is T` / `!is T` | 对 subject 执行现有类型检查或其否定 |
| `in expression` / `!in expression` | 执行现有 `expression.contains(subject)` 或其否定 |
| `condition1, condition2` | 按源码顺序短路尝试，任一成功即选中这个 arm |
| `else` | 到达这里时无条件选中；至多一个且必须最后 |

多条件允许尾随逗号。条件表达式及其调用、异常或挂起行为只在执行到该条件时发生；不能预先计算后续 arm 的值、区间或集合。`in` 使用现有 `contains` 规则，不限于区间，也不要求为本特性新增 range 类型。

任意合法表达式都可以作为 subject，包括 String、Boolean、class、interface、函数值和泛型值。具体条件仍必须能按普通规则通过类型检查：`Any` 可以接受 `is String` 条件，但当 x 的静态类型为 Any 时，`when (x) { y -> ... }` 不会因此获得不存在的 `Any.equals`。类、数组或函数值没有适用的成员 `operator equals` 时，普通值条件照常报错；需要 identity 判断可以使用无 subject 的 `when { x === y -> ... }`。

值条件复用语言规范 11.11 的成员选择、参数适配、泛型 bound 和结构比较规则。既不交换左右操作数，也不使用 `Equality.equalTo`、地址比较或运行期动态方法作为 fallback。普通表达式的数字字面量与 contextual enum variant 可以使用比较目标提供的期望类型，但这不改变表达式与模式的语法分类。

有 subject 时，`when (flag) { predicate() -> ... }` 比较两个 Boolean 值；不会把 `predicate()` 偷换为无 subject 的谓词条件。

### 2.3 `case` 模式与普通值条件共存

```scoop
struct Point(val x: Int, val y: Int)

fun describe(point: Point, expected: Point): String {
    return when (point) {
        expected -> "expected"
        Point(0, 0) -> "origin"
        case Point(x, y) if x == y -> "diagonal"
        case Point(x, _) -> "other"
    }
}
```

这里 `expected` 读取外部变量，`Point(0, 0)` 构造值后调用普通相等操作，`case Point(x, y)` 才会解构并引入 `x`、`y`。自定义 equals 可以让值条件与结构模式产生不同结果，这是明确语法带来的正常区别。

`case` 在 when arm 头部作为上下文关键字；其他表达式和声明位置不新增保留字限制。若普通值表达式本身以名为 `case` 的标识符开始，使用 `(case)`、`(case())` 等分组形式消除歧义。parser 不查询符号表决定是否进入模式语法。

`case` 后完整保留语言规范第 5 章已有的模式能力：enum unit/payload variant、tuple、struct 位置或字段形状、literal、binding、`_`、`..`、字段 shorthand、递归子模式及其类型检查。一个 `case` 前缀覆盖整棵模式，内部无需重复写 `case`。

case 内沿用既有裸名规则：Unit literal 优先；enum subject 下先解析同名 variant；其余裸名建立新绑定，不引用外层变量。裸名未命中 enum variant 时保留现有 warning；带 payload 的 variant 仍要求显式 payload shape。要比较外部值，使用普通 arm，或在模式后通过 guard 比较。

```scoop
when (option) {
    case Some(value) if value == expected -> use(value)
    case Some(value) -> useOther(value)
    case None -> onMissing()
}
```

模式绑定只在该 arm 的 guard 和 body 中可见，其他 arm 和 `when` 外均不可见。`case` 仍要求 subject 的静态类型适合该模式；对 `Any` 或 interface 中的值先使用 `is S`，再在分支内按现有 binding 规则解构。不给 class 新增 component-based match pattern。

同一 `when` 可以混用普通条件与 `case`，但每个 case arm 只有一个模式，不提供逗号连接的 pattern alternatives，也不在一个头部混写普通条件与模式。共享 body 的多普通条件照 2.2 处理；需要多套模式绑定时分别写 arm。这保留现有模式能力，不引入新的 OR-pattern 绑定规则。

普通 enum 值条件要满足整个比较操作的可用性；`case None` 或只检查 variant 的模式不依赖 enum 的 `equals` 是否可派生。含不可比较 payload 的 enum 因而仍能用结构模式分支。

### 2.4 guard、arm body 与 smart cast

有 subject 的单个普通条件或 `case` 可以接 `if expression`，括号可选；既支持 Kotlin 的 `is T if predicate -> ...`，也继续接受旧模式 guard 的 `if (predicate)`。

```scoop
when (val item = load()) {
    is String if item.length > 0 -> println(item.length)
    is String -> println("empty")
    else if canRetry() -> retry()
    else -> stop()
}
```

先检查主条件，成功后才计算 guard；guard 为假继续后续 arm。guard 的结果必须是 Boolean，或按底类型规则没有正常结果。带逗号的多条件 arm 不允许附加 guard；无 subject 的条件使用 `&&` 表达联合条件。`else if condition` 是带 guard 的后备 arm，可以有多个；它们不能充当无条件 else，后面仍可有其他 arm 和最终 else。

body 支持块或单个控制结构 body，包括表达式、赋值、`return`、`throw` 和当前循环内合法的 `break` / `continue`。块的结果和跳转规则沿用语言规范第 5 章与 8.7；块体不能被尾随 lambda parser 吞作条件表达式的一部分。单语句 arm 的换行结束当前表达式，下一行的 `is` / `!is` / `in` / `!in` 或带符号值可开始新 arm；括号内的表达式和已经写出、仍等待右操作数的运算符允许跨行。

smart cast 以当前执行路径已经建立的类型事实为依据：

- `is T` 成功后，在 guard 和 body 中以 T 视图访问稳定 subject；case 绑定经 guard 中的类型检查后也适用。
- 无 subject 的 `x is T && predicate(x)` 在右操作数及成功 body 中使用 T；`!`、`&&`、`||` 按各自的真假路径传播事实。
- 多条件进入同一 body 时，仅保留所有成功入口共有的事实；不能把 `is A, is B` 当成同时满足 A 与 B。
- 后续 arm 和 else 只能继承所有到达路径共有的事实。例如无 guard 的 `!is T` 失败能建立 T；`is T if check()` 失败既可能来自类型不匹配，也可能来自 guard 为假，不能一概断言“不属于 T”。
- arm 内的临时类型视图在退出相应作用域后恢复，不改变声明的原始类型，也不把整个函数升级为全局数据流推断。

可收窄对象包括不可变局部、参数、subject 的 `val` 绑定（含头部解构得到的叶 binding）及不可变模式绑定。叶 binding 以自身经过的类型检查建立事实，不因完整 subject 被收窄而自动推导新的字段类型。对可变局部、delegate、可能重新计算的属性/getter 不直接收窄原表达式；需要稳定视图时使用 `when (val x = expression)`。内部 subject 快照始终稳定，但快照的事实不能反推每次重新读取都可能变化的原属性。

类型检查与成功转换使用现有 exact type、泛型单态化、接口、装箱和拆箱通道，不引入 JVM 式类型擦除规则或新的 runtime 检查协议。同一路径上的多个已成功类型检查应同时保留其约束，访问成员或传参时使用相应的已知类型视图，并沿普通重载规则处理候选。分支内创建的闭包、匿名函数与局部函数可继续使用所捕获不可变 binding 的已知类型事实；捕获的原始存储类型与声明身份不改变。不为此新增用户可书写的 union/intersection type。`Option<T>` 仍不因 `isSome()` 或判空获得 payload smart cast。

### 2.5 穷尽性、正常落空与结果类型

| 使用形式 | 是否必须穷尽 |
| --- | --- |
| 任意 `when` 用作值 | 必须 |
| 含至少一个 `case` 的 when，用作语句 | 必须，保留 Scoop 模式 when 的既有要求 |
| 不含 case，subject 为 Boolean 或 enum，包括 `Option<T>`，用作语句 | 必须，与现代 Kotlin 对 Boolean/enum 的要求一致 |
| 其他普通 subject 或无 subject 的 when，用作语句 | 可以不穷尽；没有 arm 命中时正常继续 |

头部的 val 解构是分支之前的声明，不是一个 arm，不贡献覆盖，也不使普通 when 自动归类为“含 case”。穷尽性始终针对保存的完整 subject 和实际 arm 计算。

无 subject 的值形式必须有无条件 else；不把某个条件恰好写为 `true` 当作 else。无条件 else 覆盖剩余输入。任何带 guard 的 arm，包括 `else if`，都不贡献静态覆盖，即使 guard 看起来是常量真。

case 继续使用现有递归 constructor matrix：variant payload、tuple/struct product、Boolean、Unit、定宽整数、Char、String 与 Float/Double 的覆盖规则不退化。缺失覆盖保留稳定、可重新解析的 witness；guard 不替代实际覆盖。Float/Double 继续采用 IEEE 相等语义，不能把 NaN 或正负零按位相等处理。

普通条件只有具有确定语言语义的部分参与覆盖：Boolean 的字面量 `true` / `false`、可确定的内建标量 literal 比较，以及使用编译器已知结构相等的 enum unit variant 等，可以转成现有 checker 的覆盖事实。任意变量、const 引用、调用、`in` 条件或用户定义 equals 均不被当作构造器覆盖；不能由两个复杂 Boolean 常量表达式推导完备性。普通值条件仍以原表达式和已选 equals 执行，覆盖事实不改变求值行为。

对于纯 unit enum，列出全部 variant 且比较使用既有派生语义时可以穷尽；如果用户覆盖了 equals，应使用 `case` 或 else，不能假设相等具有自反性。带 payload 的 enum 按结构 case 递归覆盖最直接，列出若干 payload 构造值不能自动覆盖整个 variant。

普通 class/interface 的实现集合不是封闭集合，不能根据当前 Cone 中恰好出现的实现证明穷尽。现阶段不借 M35 实现 `sealed`，也不增加任意谓词、区间或类型逻辑的完备性证明器；不能从已有有限覆盖规则确定的剩余路径使用 else。

正常结束的分支按现有规则产生尾值，空块或以非表达式语句结束的块产生 Unit。存在外层期望类型时，所有正常结果必须能适配该类型；否则取正常分支的唯一可表达最小上界，多个不可比较的最小上界退化为 Any。return/throw/break/continue 和 Nothing 求值没有正常结果，不参与值合并；所有路径均不正常结束时，整体为 Nothing。

语句形式允许的落空是显式正常控制流，不是“证明不可达”，也不需要制造一个用于赋值的结果。不能为了省略 else 给值形式偷偷补 Unit。

## 3. Elvis 与跳转表达式

### 3.1 `return` 的位置与目标

```scoop
fun describe(value: Int?): String {
    val number = value ?: return "missing"
    return number.toString()
}

fun consume(value: Int?) {
    val number = value ?: return
    println(number)
}
```

return 在表达式位置具有 Nothing 类型。返回值按当前 callable 的返回类型检查，与 return 所处表达式的期望类型无关：上例 Elvis 的正常结果是 Int，而 `return "missing"` 返回 String。

parser 在表达式入口接受 `return [expression]`，同时统一 `throw expression` 在单行 arm 和 Elvis 等表达式位置的处理。语句入口复用相同语义，不为 `?: return` 增加只识别这一段 token 的特殊规则。保留 Elvis 的右结合优先级。

裸 return 仍受换行、分号、块结束或文件结束约束；表达式嵌套中还要正确处理 `)`、`]`、`,` 等结束位置，不能越过调用实参或数组元素边界吞入后续表达式。return/throw 的值表达式自身也可以先产生异常、挂起或无正常结果。

允许返回的目标沿用现有 callable 规则：普通块体函数和匿名函数返回自身；lambda 内裸 return、没有返回目标的顶层/初始化/default 表达式、以及不允许显式 return 的表达式体函数均报错。尾随 lambda 不改变其 callable 边界。`return` 无值仅对允许返回 Unit 的目标成立；不新增 label 或跨 lambda 的 non-local return。

### 3.2 Elvis 类型与求值

`lhs ?: rhs` 继续表示对 `Option<T>` 的 Some/None 分支，lhs 只求值一次，rhs 只在 None 路径求值。结果按语言规范 7.3 的 when 脱糖及第 5 章的正常分支合并规则计算：Some 路径提供 T；rhs 仅在能正常结束时提供其结果类型。外层期望类型和字面量推断按同一分支规则处理，不能保留基线的 exact-type 相等检查来拒绝合法子类型或 Nothing。

因此 `option ?: return value`、`option ?: throw error`、`option ?: fail()` 中，若 `fail(): Nothing`，右侧均不贡献正常结果，Elvis 的正常值仍来自 Some 的 payload。右侧的普通值分支则按正常类型适配或最小上界合并；不把 Nothing 改写成 payload 类型的伪表达式。

### 3.3 控制流与清理

复用现有 `ValueBlock`、完成性质及 return/throw 控制转移。HIR lowering 必须能区分“获得正常值”和“当前路径已经终止”；只在真正到达合并点的前驱上提供结果。不得为终止分支生成未初始化 local、Unit、零值、null、装箱或虚假的结果赋值。

表达式嵌在 receiver、普通实参、数组/tuple 元素或其他短路运算中时，先前已发生的求值保留；发生跳转后，后续实参、default 和外层调用不再执行。类型检查仍检查源码中的合法性，不用运行期不会到达掩盖非法 return 目标。

返回与异常继续走现有 finally、Context scope、native roots 和 coroutine 清理路径。suspend 函数执行 Elvis 右侧时可以挂起；挂起本身不等于完成返回。M35 不改变已有 break/continue 目标和清理规则，也不以新增其他跳转语法为验收前提。

## 4. Kotlin 规则的尾随 lambda

### 4.1 语法与参数映射

```scoop
fun run(prefix: String = "ready", action: () -> Unit) {
    println(prefix)
    action()
}

run("start") { println("body") }
run(prefix = "start") { println("body") }
run { println("body") }

fun choose(first: () -> Unit = {}, last: () -> Unit) {
    first()
    last()
}

choose({ println("first") }) { println("last") }
choose { println("last") }

fun visit(vararg values: Int, action: () -> Unit) {
    action()
}

visit(1, 2) { println("done") }
visit { println("empty") }
```

尾随 lambda 是本次调用的最后一个显式实参，固定映射到每个候选声明中的最后一个形参。该形参不能是 vararg，且必须能通过普通 lambda 类型检查接收该值，包括通过泛型约束与推断确定类型的情况。不能向前搜索“最后一个函数类型形参”，也不能把尾随 lambda 当作 vararg 的一个元素。

圆括号是否省略取决于本次源码调用是否还提供括号内实参，而不是函数声明有几个形参、几个函数类型形参。`f() { ... }` 与 `f { ... }` 使用相同参数映射；多个函数类型形参中，前面的可以显式传入或使用合法默认值。

候选映射按以下顺序完成：

1. 识别括号内实参与唯一的尾随 lambda，保留各自源码顺序和来源。
2. 针对每个候选，将尾随 lambda 固定到最后形参；括号内实参按原位置、命名、spread 与 vararg 规则映射。
3. 检查重复绑定、未知名称、多余实参、缺少必需参数与 vararg 合法性；填充实际需要的默认值或空 vararg。
4. 使用该候选的完整参数映射提供 lambda 的期望类型，执行现有泛型推断、body 检查和重载优先级规则。

尾随实参是“命名参数跳位后只能继续写命名实参”和“vararg 后的参数需命名”两条规则的明确例外。这个例外只适用于独立的尾随 lambda；括号内普通位置 lambda 不获得隐式跳过 default 或 vararg 的能力。

| 声明/调用情形 | 结果 |
| --- | --- |
| 前面有默认参数，最后是 action，调用 `f { ... }` | lambda 绑定 action；使用前面的默认值 |
| 前面有必需参数，调用 `f(value) { ... }` | 正常映射两个显式实参 |
| 前面有两个函数类型形参，其中第一个有默认值 | 可以只写尾随 lambda，绑定最后一个 |
| 前面有 vararg，调用 `f(1, *items) { ... }` | 普通实参与 spread 属于 vararg；尾随 lambda 属于最后形参 |
| 最后形参已由括号内位置或命名实参提供 | 重复绑定错误 |
| `fun f(action: () -> Unit, count: Int = 0)`，调用 `f { ... }` | 错误；不能跳过最后的 count 去绑定 action |
| 最后形参是 `vararg actions: () -> Unit` | 尾随 lambda 不适用；在括号内提供元素 |
| 前面仍有缺失的必需参数 | 错误；尾随语法不生成新的默认值 |

尾随写法本身不增加新的重载优先级。不能把它简单追加成普通位置实参，否则 default、命名参数跳位和 vararg 后参数都会映射错误。

### 4.2 所有普通调用入口与解析边界

统一支持普通函数、成员、extension、super 成员、构造调用、别名/限定名、显式泛型调用、函数值和已有 operator invoke。示例包括 `f<T> { ... }`、`receiver.f { ... }`、`receiver?.f { ... }`、`Factory { ... }` 和 `functionValue { ... }`；各入口仍遵守原有名称与候选选择规则，不要求 callee 标记为 infix。

函数值没有形参名、默认值或源码 vararg 约定，尾随 lambda 仍绑定函数类型的最后参数位置，其他位置必须完整提供。例如类型为 `(Int, () -> Unit) -> Unit` 的函数值支持 `value(1) { ... }`，不支持遗漏 Int 的 `value { ... }`。

调用后缀必须保留分组边界：`factory() { ... }` 把 lambda 交给 factory 调用；`(factory()) { ... }` 调用 factory 返回的值。不能因 parser 擦除了圆括号，把第二种错误地追加到内部调用。显式泛型实参后的 `>` 也应认可后续 lambda，不再只认可 `(`。

按 Kotlin 的调用后缀规则处理允许的换行，例如 `f()` 后换行再跟 lambda 仍可属于同一调用；不能一律要求 `{` 与 callee 同行。显式分号终止原表达式，return 等自身的换行规则保持有效。每次调用最多一个外置 lambda，连续两个块不能悄悄解释为调用返回值；需要该含义时显式分组或写出下一次调用。

普通 `{ ... }` 与现有显式 `suspend { ... }` 都可以占据尾随位置，后者可写为 `f(...) suspend { ... }` 或 `f suspend { ... }`。普通 lambda 不会因为期望类型是挂起函数而自动变为 suspend。既有非 lambda 的 infix 调用、copy-update 块、控制结构块与 lambda body 的边界均须保留。

### 4.3 求值、闭包与普通产物通路

沿用语言规范 8.5.3：receiver 先求值；显式实参按源码顺序求值，尾随 lambda 在括号内实参之后创建；再按形参顺序物化 vararg 和实际使用的 default；最后进入 callee。lambda 的创建和其 body 的执行是两件事，body 只在被调用时执行。

safe call 的 None 路径不计算括号内实参、不创建尾随 closure，也不执行 default 或 vararg 物化。capture、`it`、解构参数、返回类型型变与挂起限制使用已有 lambda 规则。

默认模板、泛型正文、跨 Cone 调用与再次发布必须得到同一参数映射和调用顺序。完成前端决议后，尾随来源不再需要进入 MIR 或 ABI；HIR 保存已经选定目标和按现有规则生成的完整调用。

## 5. annotation class 数组参数

### 5.1 类型与源码值

```scoop
annotation class MyAnnotation(val values: Array<String>)

@MyAnnotation(values = ["value1", "value2"])
class Example {}

annotation class Labels(val values: Array<String> = [])

@Labels
class Empty {}
```

参数必须仍为 val；在 M29 已允许的 Boolean、String、Char、定宽整数、Float、Double 标量之外，增加一维 `Array<T>`，T 必须属于上述标量集合。普通 typealias 展开后按实际类型判断，`Array` 使用实际 core 声明身份，不按短名字匹配。

数组值写作 `[...]`，支持位置或命名实参、空数组、尾随逗号，以及相同形式的参数默认值。每个元素使用原有注解标量常量规则：literal、合法有符号数字 literal，或已解析到同类型标量 const val 的引用。类型由参数元素类型提供，包括空数组；保留顺序和重复元素。

数值范围、Float/Double 目标精度与原始 bits、Char Unicode scalar、String 内容及 const 可见性沿用既有规则。元素必须各自带有可定位的源码 span，元素类型、溢出、非法引用等错误指向具体元素，而非统一归到整条 annotation。

不接受 `MutableArray<T>`、多维数组、`Array<Any>`、Option/enum/用户对象元素、嵌套 annotation 对象、spread、数组工厂调用或任意运行期表达式。该范围是在当前注解标量域上增加数组容器；不顺带扩展全部 Kotlin/JVM 注解参数种类。

数组默认值在声明处绑定，使用处按原有参数规则补齐，位置/命名重复、未知参数及缺失必需值的诊断保持一致。内建 annotation 各自已有的参数形状不因 parser 能读取数组而自动扩张。

### 5.2 静态数据与完整类型

注解数组是编译期静态数据，不执行 Array 构造器，不分配 managed 数组，没有可观察的数组 identity，也不进入 GC root 或运行期反射表。普通代码中的数组表达式与 annotation 的常量数组使用各自已有的语义入口。

保留现有 `CanonicalConstValueV1` 的标量用途，为 annotation 值增加封闭的“标量/数组”表示；数组变体保存明确的元素类型与有序标量元素，默认值和已绑定应用使用同一表示。不要把普通 const property 的值域扩大为 Array：`const val` 的标量限制不变，注解数组只能引用其中合法的标量元素。

参数类型不能继续只存 `PersistentTypeId`。复用已有完整 signature type key，保留真实 `Array` generic owner、元素 application 和 alias 展开后的身份；使用处、导出与 import 不能丢失 T，也不能通过 FQN 补回。

共享格式继续按参数声明顺序保存补齐后的应用，数组内部保持元素顺序。默认值、空数组及跨 Cone 标量 const 引用在静态描述查询与 A→B→C 再次发布后必须保持相同数据，不要求消费者重新执行表达式或解析提供方名称。

格式 reader 在已有读取边界检查实际类型引用、数组/标量 tag 与 payload 一致性，不增加源码常量求值器或在后续 stage 重放注解类型检查。

## 6. AST、HIR、MIR 与阶段职责

| 层次 | 必要变更与输出不变量 |
| --- | --- |
| AST / parser | when subject 明确区分缺省、表达式和 val 声明；val 保存完整 binding pattern、可选类型标注与初始化表达式；arm 区分普通条件列表、case 与后备条件；保留独立 guard、各条件位置、跳转表达式、尾随实参来源及注解元素 span |
| hir-lower | 完成普通操作决议、subject 快照、头部不可失败解构、arm 模式绑定、路径类型事实、覆盖、正常完成和调用参数映射；输出已确定的类型与引用 |
| HIR / LocalConcreteHir | 扩展现有 when 条件与 fallback 表示；保留谓词局部求值、case 绑定与结果控制流；注解保存完整类型和静态数组数据 |
| MIR | 把有序谓词、模式测试、guard 与落空/跳转转成现有 CFG；结果只来自正常前驱，清理走既有 transfer 通路 |
| LIR / codegen | 消费既有比较、类型检查、closure、branch、return 与异常操作；不解释源码 case、尾随语法或注解表达式 |
| 共享 HIR / slib | 默认模板及导出泛型正文贯通新的条件与完成形态；注解数组类型和值可发布、读取、重导出；仅升级实际变化的格式 |

现有 when 是模式专用节点，不能只放宽 parser 后继续把所有 arm 填进 `Pattern`。在现有结构中使用封闭的 typed 条件分类：普通条件保存已解析的 Boolean 求值及其局部 setup，case 保存已有 typed pattern；多普通条件在节点内保留顺序和短路边。无 subject 的 when 不伪造 Unit subject，使用明确的条件形式。

头部 val 解构复用已有不可失败 binding planner，在所有 arm 之前输出相同的投影/component 与局部绑定步骤。binding 计划和 when 条件共用同一个稳定 subject，不能分别 lowering 两次初始化表达式，也不能从叶 binding 重建 subject。临时值与叶 binding 沿已有捕获、异常和挂起保存通路处理。

guard 只在主条件成功后的路径执行，case 模式投影与 binding 也只在相应匹配成功后建立。复用[现有模式决策步骤](../../compiler/mir-lower/src/body/patterns.rs)处理 test 与 payload 提取，普通条件走既有表达式/CFG lowering；不建立另一套模式引擎或通用中间决策语言。

fallback 至少明确区分源码无条件 else、语句合法落空、以及前端已确定的不可达。现有 `ExhaustivenessProof` 只承载普通类型检查结论，按实际新增的覆盖形式调整；不增加 stage 间的授权、凭证或重复证明机制。

表达式 lowering 复用正常值与终止路径的封闭区分，不用缺失 type/value 的 Option 代替成功 IR 的结构完备性。具体化、替换、捕获、完成性质、dump、默认模板导出和跨 Cone 正文读写的既有遍历都要覆盖新增分支，不能在通道中留下待后端猜测的语法节点。

stage 继续只通过输入/输出 IR crate 通信。源码语义在前端确定，IR/meta crate 保持数据与必要格式/引用约束，不引入新的 stage 依赖、通用预算或不服务当前功能的插件框架。

## 7. 源码迁移与产物兼容性

旧 `when` 按 subject 类型隐式进入 pattern 的行为被 `case` 取代，需要在实施时统一迁移 core、普通库、示例、spec 脱糖、Rust 测试中的源码和文件 fixture：

- 旧 binding、wildcard、variant payload、tuple/struct 结构 arm 增加 case。尤其 `Some(v)` 不能在新语义下继续暗中绑定 v。
- literal 与 unit variant 若继续保持模式语义也增加 case；若有意改为普通值条件，单独确认 equals 可用性、覆盖和副作用，不能机械删去前缀。
- `val`/`for`/lambda 参数等 binding 位置的模式不加 case，规则保持原样。
- 更新旧的“String subject 非法”“Array 注解参数非法”等 negative fixture，并为新的真实错误提供相应反例。
- HIR/MIR/LIR golden 随实际控制流变化更新，保留 M22 递归覆盖、M30 浮点模式及已有清理/求值顺序回归。

这是一次明确的源码兼容性变更，不维持通过 subject 类型启用的旧解析模式。可确认旧 pattern 形状的位置给出添加 case 的迁移提示；普通调用值条件本身合法时，不能为了兼容自动改成解构。

新的 typed when 条件可能进入默认值及导出泛型正文；注解参数完整类型和数组值也会改变共享 HIR 编码。实施时根据实际编码修改提升受影响的 profile/schema 版本，并更新相关指纹与构建缓存规则。版本号在落地时分配，本设计不预占字段号或建立两套长期兼容 reader。

旧 profile 按普通版本规则明确拒绝并提示重建。重新构建 core 和相关依赖，使源码、导出模板、注解描述和消费方一致；修改元素、默认值或参数类型必须使相关下游缓存失效。没有字段变化的 MIR/LIR 格式不因为里程碑编号而机械升级。

本设计预计不需要新的 runtime function、TypeDescriptor 能力、目标 ABI 或 GC 协议。若实施发现现有普通控制流、类型检查或 roots 的缺陷，在相应现有机制中修正；新增契约必须有本里程碑真实程序的必要性并先修订对应 spec。

## 8. 后续 spec 修订清单

| 文档与位置 | 实施前需要明确的内容 |
| --- | --- |
| 语言规范 4.6、第 5 章 | 删除按 subject 静态类型切换 arm 语义的统一规则；定义 case、普通条件、subject 普通/解构声明、guard、smart cast、覆盖与语句落空；将 subject val 纳入共享 binding 规则与求值计划，明确匹配原完整值；迁移全部模式例子 |
| 语言规范 3.1、7.3、8.1.2、8.7 | return/throw 表达式、Nothing 与正常完成；Elvis 分支结果合并及 return 目标；保持 lambda 与循环边界；更新 Option 脱糖中的 case |
| 语言规范 8.5.2～8.5.3 | 尾随来源、最后形参映射、默认/命名/vararg 例外、函数值调用、换行与求值顺序 |
| 语言规范 9.1.2、9.4 | 注解数组的合法元素与静态数据规则；明确普通 const val 仍为标量 |
| 实现规范 2.1～2.4、2.12、2.17 与实际共享格式条目 | AST/HIR 完备表示、when fallback、无正常值路径、注解完整类型/数组数据及实际编码变更 |
| 运行时规范的引用方 | 核对现有 is/转换、异常清理、closure 与 GC 契约的引用；只有实际契约改变才修订正文 |

本表为实施步骤。相关章节移动时同步引用方，不能用本设计默默覆盖仍冲突的正式 spec。

## 9. 实施批次

| 批次 | 交付 | 验收出口 |
| --- | --- | --- |
| M35-1 规范与 case 迁移 | 先修订受影响 spec；case 语法、旧模式源码迁移、现有 pattern 语义保持 | core 重建；模式正反例和原递归覆盖 fixture 通过 |
| M35-2 普通 when | subject/val/头部解构/无 subject、值/类型/成员关系、多条件、guard、smart cast、落空与覆盖 | 各类 subject 的真实 CLI 运行，头部共享 binding 计划与原值匹配，作用域/类型/覆盖反例及 HIR/MIR/LIR golden |
| M35-3 跳转与 Elvis | return/throw 表达式、正常结果合并、求值与清理 | 有值/裸 return、Nothing 调用、finally/Context 与挂起组合运行 |
| M35-4 完整尾随调用 | 调用后缀、逐候选映射、default/命名/vararg/多函数参数、所有调用入口 | 类型推断与歧义诊断、源码求值顺序、safe call、导出默认值和泛型调用 |
| M35-5 注解数组 | 声明/应用/默认数组、静态描述、完整类型和共享格式 | 元素诊断、空数组与默认值、跨 Cone 查询、重导出及旧格式拒绝 |
| M35-6 组合与回归 | 三个命令产物、依赖重建、功能组合、正式 CLI 与回归 | debug/release、适用 GC 与平台验收，记录实际覆盖及结果 |

每批以正常源码到相应运行/诊断结果为出口，不能以只增加 AST variant、只通过 Rust 编译或内部构造成功代替完成。共享格式随对应可消费实现落地，不把无关的 runtime 或产物重构插入主线。

## 10. 验收矩阵

| 类别 | 必需正例与组合 | 必需反例或结构断言 |
| --- | --- | --- |
| when 普通值 | String、Boolean、整数/Char/Float、struct/tuple/enum 值、显式 equals 的 class/interface、泛型 bound、变量与计算值 | 缺少/歧义 equals；未定义名字不得变成绑定；Boolean subject 不把 Boolean 值 arm 当谓词 |
| when 控制 | subject 一次求值、无 subject、val subject、in/!in、自定义 contains、多条件短路、换行/逗号、普通与 case 混合 | 非 Boolean 条件/guard；无 subject 的缩略 is/in/case；多个/非末尾无条件 else；多条件带 guard |
| subject 解构声明 | tuple/struct 位置与字段、重命名、嵌套、`_`/rest、class component、完整值类型标注；绑定用于全部条件/guard/body/else；跨 Cone/default/泛型及捕获 | literal/enum variant 可失败模式、重复名字、错误字段/元数/类型、缺失 component 或非法挂起；初始化时/when 外不可见；仍匹配原完整值，头部不贡献覆盖；初始化和 component 各按规则执行一次，抛出不进入 arm，挂起不重做 |
| case 与覆盖 | enum payload、tuple/struct 字段与 rest、nested pattern、guard 失败继续、Boolean/Unit/整数/Char/浮点既有矩阵 | 错误 shape/arity/字段、scope 泄漏、非法 case subject、非穷尽及 witness；NaN、自定义非自反 equals 不伪造覆盖 |
| smart cast | subject val、参数、case 绑定；is/!is、逻辑与/或/非、guard 到 body、后续 arm 共同事实、装箱值与接口/泛型 exact type | 多入口不同类型不能取任一类型；guard 失败不等于主条件失败；可变/getter/delegate 不稳定；离开 arm 不泄漏 |
| when 值结果 | 不同正常分支的 subtype/LUB、Unit 块、部分/全部终止、语句合法落空、return/throw 单行 body | 值位置缺 else/覆盖；Boolean/enum 语句非穷尽；含 case 语句非穷尽；不允许用补 Unit 掩盖缺失 |
| Elvis / 跳转 | 有值与裸 return、右结合、rhs Nothing/throw、返回类型不同于 payload、嵌套实参、正常类型合并 | 非 Option lhs、错误返回类型、非法 return 目标、lambda 裸 return；终止分支无假结果，后续求值不执行 |
| 尾随 lambda | 单 lambda、多函数形参、前置 default、named 跳位、vararg/spread、泛型、constructor/member/extension/super/invoke/函数值、显式 suspend | 最后形参不适用、缺 required、重复绑定、最后为 vararg、多外置 lambda、重载歧义、普通/挂起类型不匹配 |
| parser 边界 | 裸名/限定名/显式类型参数、跨行尾随、分组后返回值调用、safe call、既有 infix/copy update/控制块 | 分号后不能追加原调用；when body 不被当尾随 lambda；return 不吞后续参数或下一行 |
| 注解数组 | 每种合法标量、空/单/多元素、顺序与重复值、trailing comma、默认、named/positional、alias、标量 const、Float 原始 bits | 元素类型/溢出/span、非法维度/元素/容器、运行期表达式/spread、重复/缺失参数；const val 不获得数组能力 |
| 跨 Cone 与格式 | 默认模板与泛型正文包含新 when/Elvis/尾随调用；注解 A→B→C、静态描述、再次发布、artifact-only 链接运行 | 原实体与完整 Array application 保留；数组变化使缓存失效；旧 profile/非法 tag/类型引用按边界诊断 |
| 求值、EH 与 GC | 条件/contains/guard 的副作用与异常；Elvis finally/Context；尾随捕获、safe call；suspend 恢复；引用跨 minor/full moving GC | 未选路径不求值；原 subject 不重读；清理恰好发生；closure/frame/root 不遗失；不错误标记 suspend 为无返回 |
| golden | HIR 的 typed 条件/调用映射/注解数据，MIR 的短路/正常合并/清理边，LIR 的既有比较/类型检查/closure 路径 | 新语法不残留到后端解释；正常落空与不可达区分；终止路径不提供 payload |

每项特性有独立 fixture，并增加真实组合：在 is arm 的 smart cast 后调用带默认/vararg 的尾随 lambda；在 case guard 失败后继续值匹配；在跨 Cone 泛型/default 中使用 Elvis 和 when；带数组 annotation 的类型经现有静态描述与普通 codec 通路消费。源码错误规则逐项有 negative fixture，断言位置和信息。

实施完成门遵守仓库流程：每批代码先 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，再执行适用 Rust 测试；最终运行 `cargo test --workspace`，构建 `scoop`、`scoopc`、`scoop-linker`，执行 runner 单元测试和 `python3 tests/run_fixtures.py --all`。Python 如有改动，先使用 AGENTS.md 指定版本的 ruff。复用现有 fixture schema 和 runner。

Darwin/AArch64 执行完整适用验收；Linux GNU/musl 使用已有可用环境验证受影响的源码、产物、异常和 GC 组合。记录实际 target、profile、命令和执行结果，未运行的平台不记为通过。最终选择范围、去重覆盖与环境补验见 [ACCEPTANCE.md](ACCEPTANCE.md)。

## 11. Kotlin 规则参考

- [Conditions and loops](https://kotlinlang.org/docs/control-flow.html)：有/无 subject、值与多条件、subject 局部变量、类型/成员关系检查及 guard。
- [Kotlin specification: expressions](https://kotlinlang.org/spec/expressions.html)：when 结果与匹配、跳转表达式、Elvis 的语义依据；Scoop 的 Option 和相等差异按本文明确处理。
- [Kotlin specification: overload resolution](https://kotlinlang.org/spec/overload-resolution.html#call-with-trailing-lambda-expressions)：单个尾随 lambda 与最后形参、default/vararg 的调用映射。
- [Functions](https://kotlinlang.org/docs/functions.html#parameters-with-default-values)：默认形参与最后 lambda 的括号省略规则。
- [Kotlin 1.7 compatibility guide](https://kotlinlang.org/docs/compatibility-guide-17.html#make-when-statements-with-enum-sealed-and-boolean-subjects-exhaustive-by-default)：Boolean/enum/sealed subject 的语句穷尽要求；同页说明复杂 Boolean 常量表达式不再作为相关覆盖推断依据。Scoop 已有 enum/Boolean 适用，sealed 能力仍单独排期。
