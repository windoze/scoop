# M11 设计：函数类型、函数值与 closure

版本：1.0（已实现，2026-08-31）

对应 `docs/ROADMAP.md` 的 M11。目标：把 spec 8.1 的普通/挂起函数类型、lambda、匿名函数、局部函数与 callable reference 作为正式语言能力全链路落地，并在 MIR 完成可被 M9 GC 扫描、可与 M10 状态机组合的 closure conversion。M12 FFI 随后直接复用这里的函数类型与中性的 `::name` 源码语法；同一表达式在明确的 `FunPtr<F>` 期望类型下直接解析为 native address，不再为 `FunPtr` 发明另一套引用语法或不可作为值使用的伪签名类型。

## 0. 范围与关键决策

M11 交付以下闭环：函数类型可以出现在变量、参数、返回值、字段与 generic 实参中；普通和挂起 lambda 可以捕获 `val`、参数与 `this` 等不可重新绑定的 binding；局部函数可以递归并逃逸；顶层、局部及绑定成员 callable reference 可以作为函数值；函数值可以经 direct / virtual / interface 路径调用，并在真实挂起、异常与强制 GC 后继续使用。

- managed 函数值是引用对象，不是裸代码指针。基线表示为“对象头 + invoke entry + 不可变 capture 字段”；captured value type按 concrete layout直接内联，不生成独立对象头、identity或 hidden box；
- 函数类型按 spec 8.1.1 结构化定型，参数逆变、返回协变。普通与挂起函数类型无继承或转换关系；
- lambda 与匿名函数都支持普通/挂起形态。lambda 使用尾表达式返回且拒绝裸 `return`；匿名函数支持局部 `return`；
- 局部函数名只从声明处开始可见，但在自身 body 内可见，因而支持直接递归而不引入块级前向声明；
- M11 不允许捕获外层 `var`，无论 body 只读还是写入都在 HIR 诊断。需要快照时先显式绑定为 `val`；需要共享可变状态时显式捕获用户声明的引用对象。新的 reference/move capture 等找到合适模型后再以显式语法加入。`this` 不需要独立规则；它是 spec 3.3 的隐含不可变参数，捕获时与显式参数一样复制其值；
- 函数声明引用使用显式 `::`。该语法节点本身不编码 managed/native 类别；M11 的可用期望类型只会把它解析为 managed callable reference，M12 再增加 `FunPtr<F>` 上下文分支。普通值位置的函数名不隐式变成函数值，避免命名调用、重载集合与函数值之间出现上下文不稳定的解析；
- 先单态化，再 closure conversion，最后执行 M10 coroutine transform。suspend closure 的环境接收者因此会进入 hidden continuation ABI，而不是另造一套协程实现；
- M11 同步为 core 增加接受 `suspend () -> T` 与 `(Continuation<T>) -> Unit` 的协程原语重载；已有 `SuspendTask` / `SuspendRegistration` 协议保留，lambda 重载用普通 Scoop 适配器实现，不新增 intrinsic；
- M11 不实现 receiver function type、`Type::member` 未绑定成员引用、构造函数引用、lambda non-local return或GC-aware managed callback registration。M13以“静态C trampoline + opaque token + typed adapter”单独实现最后一项，不把closure转换成`FunPtr`；其余仍是明确的后续语言/库边界，不以TODO分支留在pipeline。

## 1. 语言子集

### 1.1 普通函数值

```
fun applyTwice(value: Int, op: (Int) -> Int): Int = op(op(value))

fun makeAdder(base: Int): (Int) -> Int {
    return { value: Int -> value + base }
}

fun main() {
    val addTwo = makeAdder(2)
    println(applyTwice(20, addTwo))       // 24
    println(addTwo.invoke(40))            // 42
}
```

函数值调用只有位置实参。声明方的形参名、缺省值与 `vararg` 信息不进入函数类型；若要保留不同元数或缺省参数行为，调用方必须显式写一个适配 lambda。

### 1.2 不可变 capture 与显式可变状态

```
struct Offset(val amount: Int)

fun makeAdder(offset: Offset): (Int) -> Int {
    return { value: Int -> value + offset.amount }
}

fun snapshot(): () -> Int {
    var value = 41
    val captured = value
    val result = { captured + 1 }
    value = 0
    return result                         // 返回 42；captured 按值保存
}
```

`Offset` 以自身 concrete layout直接内联在 closure对象中；它没有独立对象头或identity，不通过 `Any`，也不产生额外的 `CaptureCell<Offset>`。若 value type内部含 managed ref，closure的递归扫描描述直接吸收这些字段。

以下代码是编译错误，即使 closure 只读取 `value`：

```
fun invalidCaptures() {
    var value = 0
    val read = { value }
    val write = { value = 1 }
}
```

需要共享状态时必须把 identity与allocation写进源码：

```
class Counter(var value: Int)

fun makeCounter(): () -> Int {
    val state = Counter(0)
    return {
        state.value = state.value + 1
        state.value
    }
}
```

这里 capture 的是不可重新绑定的 `val state`，即复制 `Counter` 引用；可变性来自程序显式创建的 reference object，而不是编译器把 `Int` binding静默heap-lift。

这项限制只针对 callable body 对外层词法局部 binding的自由变量引用。顶层/object 属性使用自身的 global/singleton storage；class 可变属性通过被捕获的 `this` 或显式引用对象访问。这些路径不需要编译器为局部value binding发明identity。

`this` 与显式参数完全一样按值捕获：ref-type `this` 把 ref value 复制进 environment，因而仍指向同一对象；value-type `this` 把完整值复制进 environment。两者都不会保留调用方 receiver binding 的存储位置。下例展示 value receiver：

```
struct Snapshot(val value: Int) {
    fun getter(): () -> Int = { this.value }
}

fun example(): Int {
    var source = Snapshot(1)
    val getter = source.getter()
    source = Snapshot(2)
    return getter()                       // 1
}
```

### 1.3 局部函数与匿名函数

```
fun factorialOf(value: Int): Int {
    fun factorial(n: Int): Int {
        return if (n <= 1) { 1 } else { n * factorial(n - 1) }
    }
    return factorial(value)
}

fun choose(negative: Boolean): (Int) -> Int {
    val op = if (negative) {
        fun(value: Int): Int { return 0 - value }
    } else {
        { value: Int -> value }
    }
    return op
}
```

局部函数声明建立词法 callable实体，本身不要求分配运行时 closure。direct call把不可变 capture作为类型化隐藏参数传给 lifted body；求值 `::factorial` 时才创建函数值并把这些 capture按值写入closure。自身递归继续调用同一 lifted body，不要求函数捕获自己。后声明的局部函数在前一个函数 body 内不可见；需要互相递归时必须改为显式对象/interface 协议，直到语言另行引入递归声明组。

局部 generic 函数可直接调用并按既有规则单态化；只有期望函数类型能唯一确定全部类型实参时才可取得其 callable reference。每个 concrete reference closure直接保存其不可变 capture，不建立额外的 generic environment object。lambda 与匿名函数本身不声明类型参数。

### 1.4 函数声明引用与 managed callable reference

```
fun inc(value: Int): Int = value + 1

interface Mapper {
    fun map(value: Int): Int
}

fun bind(mapper: Mapper): (Int) -> Int {
    return mapper::map
}

fun main() {
    val top: (Int) -> Int = ::inc
    val bound = bind(makeMapper())
    println(top(41))
    println(bound(41))
}
```

- `::name` 在最近的局部函数候选层解析，若该层不存在候选再查顶层函数；
- `receiver::member` 在创建时从左到右求值 receiver 一次并保存；调用时按保存 receiver 的动态类型执行 virtual / interface 分派；
- receiver 是创建点的普通表达式；它可以读取局部 `var` 并立即复制当前值。该源码可见的创建时快照不是 callable body 的自由变量 capture，后续重新绑定原 `var` 不改变已创建的 bound reference；
- `::extension` 的未绑定类型把扩展 receiver 放在第一个参数，`receiver::extension` 保存 receiver 并移除该参数；
- 无期望类型时，只有唯一非 generic 候选可决定引用类型，所得结果固定为 managed 函数值，不预判未来的 `FunPtr` 分支；有重载或 generic 候选时必须由期望函数类型唯一选中；
- 函数声明名出现在普通值位置时诊断并提示使用 `::name`。直接 `name(args...)` 保持 M7 的命名调用与重载语义。

### 1.5 suspend 函数值

```
fun makeTask(gate: Gate): suspend () -> Int {
    return suspend {
        val value = gate.awaitValue()
        value + 1
    }
}

fun main() {
    startCoroutine(makeTask(gate), Done())
    gate.complete(41)
}
```

挂起 lambda 必须显式写 `suspend`。创建、保存或传递它是普通同步操作；调用时才要求挂起上下文。其 invoke 源码签名为 `(closure, args...) -> R suspend`，经 M10 变换后为 `(closure, args..., Continuation<R>) -> CoroutineStep<R>`。捕获字段属于 closure；只在一次调用中跨挂起点存活的局部值属于该次调用的 coroutine frame，两者不能混成同一对象。

core 增加以下普通源码重载：

```
fun <T> startCoroutine(
    task: suspend () -> T,
    completion: Continuation<T>
)

suspend fun <T> suspendCoroutine(
    registration: (Continuation<T>) -> Unit
): T
```

它们分别把函数值包进现有 `SuspendTask<T>` / `SuspendRegistration<T>` 适配器，再调用 M10 intrinsic 形态。同步完成、真实挂起、重复恢复与异常规则完全继承 spec 11.9，不复制 continuation 状态机。

### 1.6 型变与适配

```
open class Animal()
class Dog() : Animal()

fun acceptAnimal(value: Animal): Dog = Dog()

val exact: (Animal) -> Dog = ::acceptAnimal
val widened: (Dog) -> Animal = exact
```

`widened` 的赋值合法，因为函数参数逆变、返回值协变。MIR 必须生成显式 adapter closure，使 adapter 拥有目标函数类型的精确 invoke ABI，再把参数转换后调用来源 closure并转换返回值。不得只改静态类型后以错误签名间接调用。普通与挂起 adapter 分开生成；挂起 adapter 随后进入 coroutine transform。

对 `Any` / interface 静态类型执行函数类型 `is` / `as` 时，同样使用结构化型变关系。MIR 为程序实际要求的目标函数类型按需登记 bridge；成功的 `as` 或 smart cast view 取得相应 adapter，因此不同函数类型视图不承诺保持 `===`。TypeDescriptor 不枚举理论上无限的全部函数父类型，只携带 exact function signature及本编译图实际物化的 bridge表。

## 2. 类型与实体模型

### 2.1 函数类型身份

AST 使用结构化 `FunctionTypeRef`；HIR 把它正规化为 `Type::Function(FunctionTypeId)`。`FunctionTypeId` 的 canonical key 包含：

```
FunctionTypeKey {
    is_suspend,
    parameter_types,
    return_type,
}
```

- key 使用已解析的类型化 type id，不使用显示名或 FQN；
- 同一HIR消费者域内相同key只产生一个typed function-type id。`ExportFunctionTypeId`与`ConcreteFunctionTypeId`是不同类型；上游`.slib`导入时由完整结构签名显式重映射，不能把不同Cone或export/local-concrete两侧的arena index直接视为同一id；
- `ExportHir`必须包含公开签名及generic template中出现的函数类型，供下游HIR使用；`LocalConcreteHir`必须包含本Cone MIR所需且已完全特化的函数类型。MIR只接受后者，不从export function type临时物化concrete结果；
- 每个 concrete function type 有自己的 TypeDescriptor/type identity。具体 closure 类型记录 exact function type；MIR 对实际发生的 coercion / `is` / `as` 目标生成 typed bridge entry，以支持结构化函数子类型检查与安全调用；
- `FunctionTypeId`、命名 `FunctionId`、`LocalFunctionId`、`LambdaId`、`AnonymousFunctionId`、`CallableReferenceId`、`ClosureClassId` 与 `ClosureAdapterId` 全部是不同的新类型，禁止用一个整数或符号字符串混用。

### 2.2 函数值 ABI

所有 managed 函数值在普通 Scoop ABI 中都是一个引用机器字，指向具体 closure 对象。M11 未优化基线的对象概念形态为：

```
$Closure {
    header: ScoopObjectHeader,
    invoke_entry: CodePtr,
    captures: ...
}
```

invoke entry 的机器签名由 concrete function type 决定：

- 普通：`(closure_ref, P1, ..., Pn) -> R`；
- suspend：closure conversion 后先形成同样带 `suspend` 属性的源码 ABI，再由 M10 增加 `Continuation<R>` 并改为 `CoroutineStep<R>` 返回。

`CodePtr` 是编译器内部 metadata/code pointer，不是 managed ref，不能进入扫描描述或被 Scoop 源码读取。closure 调用是 managed 间接调用：可以分配、抛异常或触发 safepoint；不能因为 entry 是代码指针就标为 `@NoGC` 或 `nounwind`。

M11 基线在每次执行 lambda、匿名函数或 callable reference表达式时分配所需closure；局部函数只有 direct call时lower为带隐藏capture参数的lifted function，不因声明本身分配closure。无捕获单例、栈分配、立即调用消除、devirtualization与closure合并都是后续优化；先保证所有实际函数值的identity、异常边与GC结构完整。

### 2.3 capture 描述

HIR 为每个可捕获 callable 输出结构完备的 `Capture`：

```
Capture {
    captured_binding: BindingId,
    concrete_type,
    first_use_span,
}
```

capture 字段按“外层词法层级 → 首次使用源码位置 → BindingId”稳定排序，以保证 MIR/LIR golden、布局与 mangling 可复现。对嵌套 closure 做传递捕获：内层使用某个外层不可变 binding时，中间 callable必须拥有足以在创建内层closure时提供该值的隐藏capture参数或environment字段，不允许codegen回读已退出的原生栈。

按值 capture 在 closure 创建时求值一次。引用类型复制引用，值类型复制完整值；若值类型内部含引用，TypeDescriptor 扫描描述递归覆盖这些字段。

对 `this` 做 capture 时，captured binding 必须统一指向隐含的 method-local parameter，不得指向调用方 receiver binding place。参数为 ref type 时 capture 字段保存 managed ref value，为 value type 时保存完整 concrete value；copy elision 只能作为不可观察的后端优化。

### 2.4 mutable capture 边界与成本模型

- capture analysis只要发现自由变量解析到外层词法局部 `var BindingId`，立即在引用位置诊断；read、write、复合赋值和通过局部函数的传递引用没有例外。诊断应指出 binding 名，并建议先绑定 `val` 快照或捕获显式引用状态。M11不生成`CaptureCell`、scope environment box或其他identity-bearing storage来挽救该程序；
- captured value type直接成为closure concrete class的内联字段，保持原size/alignment与递归扫描描述。closure对象本身已经是函数值所需的reference allocation，但value字段没有独立分配、对象头、TypeDescriptor或动态分派；
- captured reference type只是复制一个引用字段。通过该引用修改referent是普通显式对象语义，不等于重新绑定captured `val`；
- 需要捕获某个`var`的当前快照时，用户先写`val snapshot = value`；需要共享可变状态时，用户显式声明class或未来标准库的`MutableCell<T>`并捕获其`val`引用；
- 后续若引入`move`、reference capture或capture list，必须使用源代码可见的显式语法，并先定义ownership/lifetime、closure kind、并发及ABI成本；不得仅删除当前诊断后恢复隐式boxing。

### 2.5 managed callable reference closure

- 顶层函数引用：无 capture，invoke body direct-call 已选中的 `FunctionId`；
- 非 generic 局部函数：direct call调用lifted body并传入不可变capture；每次求值`::local`时创建closure并按值保存同一组capture，重复求值不保证相同identity；
- generic 局部函数：HIR先由direct call或期望函数类型产生concrete type arguments；lifted实例和concrete reference closure都使用完全具体化的capture字段，不建立擦除environment；
- 绑定 final 成员：在 callable reference 创建时按值capture receiver，invoke body 可 direct-call；ref receiver 保存 ref value，value receiver 按 concrete layout 内联保存，调用时都用该字段初始化 spec 3.3 的隐含 `this`；
- 绑定 virtual/interface 成员：capture receiver 与静态 dispatch contract，invoke body 保留 virtual/interface callee，不保存创建时查到的具体槽目标；
- 绑定扩展：capture receiver，invoke body direct-call 扩展函数；未绑定扩展不 capture，并把 receiver 作为第一个 source 参数；
- generic reference：HIR先由期望类型产生concrete type arguments并在`LocalConcreteHir`中生成对应实例；MIR只指向该concrete实例的映射结果。

### 2.6 名称遮蔽与候选层

M11 补齐 M7 预留的局部层：

1. 若最近词法作用域中存在同名 callable-value binding，`name(args...)` 解析为函数值调用，并遮蔽同名函数候选；
2. 否则按“局部函数层 → 成员层 → 调用点同侧顶层 → 对侧隐式导入”选择第一个非空候选层，再执行 M7 的可应用性与 most-specific 比较；
3. `::name` 不考虑普通值 binding，只在最近的局部函数层或顶层函数层中选择声明；对已有函数值使用其变量名本身，不写 `::value`；
4. 同一块内的局部函数可重载。处理到某一声明时，当前声明先加入候选层以允许自递归，后续声明尚不可见；声明后的语句能看到此前全部 overload。

lambda 实参使用候选提供的 expected function type 做双向检查。每个候选的 lambda body 结果分开缓存，决议成功后只保留胜者 HIR；所有候选检查都不得改变源码运行时求值顺序或重复执行 desugaring 的副作用表达式。

## 3. AST / parser

- `TypeRefKind::Function { parameters, return_type, is_suspend }` 作为通用 type ref，可递归出现在 nullable、generic、array、字段与返回类型中；正确区分 `Unit`、`() -> R`、tuple 和带括号的普通类型；
- `Expr::Lambda { id, is_suspend, parameters, body, span }`，参数保存可选类型与解构 pattern；
- `Expr::AnonymousFunction { id, is_suspend, parameters, return_type, body, span }`；
- `Expr::CallableReference { id, receiver, name, span }`，`receiver = None` 对应 `::name`，有值对应 `expr::name`；这是中性的函数声明引用语法节点，parser 不尝试区分成员、扩展、局部目标或 managed/native 结果类别；
- block item 增加局部 `FunctionDecl`，复用顶层/成员函数参数与 body 语法，但拒绝可见性、virtual/override、`@Extern` 等不适用于局部函数的 modifier/annotation；
- lexer/parser 接受 `suspend { ... }` 与 `suspend fun(...)`，并保证孤立 `suspend`、缺失 `->`、空参数槽、非法 callable reference 有精确 span；
- 尾随 lambda 仍是普通实参语法糖，AST 保存实际实参顺序。`f(a) { ... }` 与 `f(a, { ... })` 进入同一 HIR 调用路径；
- lambda 的 `{ ... }` 只在表达式/实参位置解析为 lambda；控制结构要求 block 的位置仍解析为 block。无 expected type 的 `{ body }` 是零参数 lambda，不凭 body 中是否出现 `it` 猜元数。

## 4. HIR

### 4.1 双向定型

HIR 表达式继续从结构上保证每个节点都有完整类型。lambda/匿名函数 lowering 接受可选 expected type：

- expected type 存在时先检查 ordinary/suspend 标志和元数，再把参数类型写入 binding；显式参数类型必须相等；
- expected type 不存在时，lambda 的所有显式参数必须带类型，返回类型由 body 推导；零参数 lambda可直接推导；匿名函数按普通函数规则检查显式/推导返回类型；
- 期望 `Unit` 时插入显式 value discard，不把任意结果类型谎报为 `Unit`；
- lambda 中裸 `return` 直接诊断；匿名函数建立自己的 return target；外层函数的 return target 不泄漏进 lambda；
- `suspend` lambda/匿名函数建立独立 suspension context。创建表达式仍在外层当前 context 检查 capture 初始化，body 则按自身挂起性检查；
- 函数类型型变产生 `ExprKind::FunctionCoercion { source, adapter_id, target_type }`，不能只替换节点类型。

### 4.2 局部函数与 capture 分析

局部函数声明按源码顺序处理：先登记当前声明的完整 signature，再 lower 其 body，最后让其 binding 对后续 item 可见。body 可以解析自身 entity；不存在块级全量预声明。

普通名称/类型检查完成后，对 callable body 做词法 capture 分析并计算传递闭包：

1. 找出引用了哪个外层 `BindingId`，排除 global、top-level function 与类型名；
2. 检查 binding mutability；目标为`var`时无论read/write都在该引用span诊断，失败body不得进入MIR；
3. 将内层 callable 所需的不可变binding传递加入中间callable的隐藏capture参数/environment供给集合；
4. 以第 2.3 节规则稳定排序并写入不可缺失、全部`ByValue`的capture list；
5. 检查所有 capture type 已完全解析；generic 定义允许引用类型参数，但实例化后不得留下未具体化capture，也不得按`value/ref` kind改变capture规则。

### 4.3 函数值调用与重载

HIR call target 扩为互斥的类型化分支：

```
CallTarget = Direct | Virtual | Interface | CallableValue(FunctionTypeId)
```

`CallableValue` 节点保存 callee 表达式、concrete function type、普通/挂起属性及实参数组；不能伪装成无接收者 direct call。显式 `.invoke` 在 HIR 名称解析后正规化为同一节点，不进入普通成员重载表。

重载候选含lambda时采用“非lambda约束固定点 → 候选expected type → lambda body检查 → 既有most-specific比较”的流程。lambda return只判定候选可应用性，不新增按返回值选重载的优先级。失败诊断必须指出lambda参数元数、挂起性或返回类型的具体不匹配，而不是统一报无候选。M16将该流程纳入每个候选独立的fresh-variable/postponed-argument session，并替换M11调用既有MSC的实现细节，不改变本节lambda语义。

### 4.4 为 M12 预留的正式接口

M11 parser 已把 callable reference 保存为中性的完整表达式；由于 M11 尚无 `FunPtr` expected type，HIR 只会为其选择 managed resolution。M12 增加 `FunPtr<F>` expected type 后，HIR 必须在创建 managed callable entity 之前按期望类型类别分支：只有语法形态和 resolved target 都满足 spec 13.10 的 `::topLevel` 才直接产生独立 `ExprKind::FunctionAddress`：

- 该路径不生成 closure，不分配对象；
- 目标签名与 `F` 精确匹配，不走 managed function variance adapter；
- lambda、匿名/局部函数、绑定引用与已有函数值已经具有 managed 语义，不能“脱壳”成 native code pointer；
- 在 `@NoGC` body 中，普通 managed callable reference 仍是分配/引用操作；只有成功走 `FunctionAddress` 的 M12 native-address contextual resolution 才不违反 NoGC。

## 5. HIR concrete化与MIR变换顺序

跨HIR/MIR边界固定以下顺序，任何一步的输出都必须结构完备：

1. **HIR单态化**：HIR实例化generic命名/局部函数、closure body、capture type与所有concrete function type，输出不含type parameter的`LocalConcreteHir`；
2. **MIR local function lifting**：为局部函数生成带完整concrete capture参数的lifted body；direct call传值调用，只有callable reference需求才物化closure class；
3. **MIR closure conversion**：为每个lambda、匿名函数及实际求值的callable reference生成`ClosureClassId`、invoke function和确定布局的按值capture初始化；
4. **MIR variance adapter**：为实际发生的函数类型coercion生成`ClosureAdapterId`与forwarding invoke body；
5. **MIR call lowering**：`CallableValue`变成加载invoke entry后的managed indirect call，保留normal/unwind edge与从左到右求值；
6. **MIR coroutine transform**：对所有命名、lifted local、lambda、anonymous、reference adapter的concrete suspend body执行M10状态机变换；
7. **完整性检查**：MIR输出不再含source lambda/local function/callable reference、source suspend callable或抽象function coercion节点，并断言不存在generic、mutable capture或capture box实体。

局部函数 declaration没有运行时求值或隐式allocation。direct call和自递归调用lifted target并显式传递不可变capture值；求值`::local`才分配结构完备的closure。由于capture不可重新绑定，direct参数与reference字段之间不存在需要共享的location，且二者都保持value semantics。

MIR 新增类型化节点/实体至少包括：`LiftedLocalFunction`、`ClosureAlloc`、`ClosureCaptureInit`、`ClosureCall`、`ClosureClass`、`ClosureInvokeFunction` 与 `ClosureAdapter`。节点都携带 concrete source/target/capture type；code pointer、environment与结果不得统一存成`UInt`或`Any`，也不得出现`CaptureCell`/`BoxedCapture`回退节点。

## 6. LIR / codegen / GC

- LIR 把 closure当作普通managed class layout：对象头、非扫描的invoke code pointer、按concrete layout排列的不可变capture字段；生成完整`RefScan`。value capture内联于同一对象，不产生第二个allocation或object identity；
- `ClosureAlloc` 降为现有 managed allocation，capture字段只在对象发布前初始化一次；含引用字段仍进入递归扫描描述，构造后没有编译器生成的capture写入；
- `ClosureCall` 降为通用 typed indirect call：先求值 callee ref，再按源码顺序求值实参，加载 invoke entry 并把 callee ref 作为第一个隐藏参数；normal/unwind CFG 与普通 managed call一致；
- codegen 为 indirect managed call保留 GC statepoint 语义，不错误添加 `nounwind`、`readonly` 或 leaf 属性；调用返回后使用 relocated callee/capture 引用；
- 每个 closure/adapter concrete类型生成TypeDescriptor。invoke code pointer与静态metadata不出现在扫描描述中；capture内嵌struct/enum/tuple时直接吸收其layout与递归扫描，captured value没有独立TypeDescriptor；
- suspend closure 的 frame 若跨挂起点仍需 closure receiver，必须把该引用存入 frame并扫描。恢复后不能从已经退出的原生栈重建环境；
- M11 不新增 runtime function。closure经显式captured reference形成的对象环，以及continuation→closure→capture链，都由现有可达性GC自然处理；
- `Option<FunctionType>` 使用 spec 7.4 的引用 niche；`None` 是全零，`Some(closure)` 与普通引用表示相同。

## 7. core 迁移

在 `scoop.core` 中以普通源码加入两个泛型适配 `struct` 和两个 overload：

- `FunctionSuspendTask<T>` 保存 `suspend () -> T`，其 `run` 调用该函数值；
- `FunctionSuspendRegistration<T>` 保存 `(Continuation<T>) -> Unit`，其 `register` 调用该函数值；
- lambda 形态 `startCoroutine` / `suspendCoroutine` 只构造适配器并调用 M10 已有 overload；
- 两个适配类型是当前 `scoop.core` 的普通源码声明；现行 `struct` 规则不支持隐藏字段或 synthetic-only 可见性，因此不虚构 private ABI。它们只承载 overload 间的协议适配，源码实现仍必须通过普通 HIR/MIR，不能由 intrinsic registry 特判；
- 原 `SuspendTask` / `SuspendRegistration` 接口和 overload继续公开，供不需要 closure 或希望显式建模生命周期的代码使用。

M10 的所有旧 fixture 必须原样通过；新增 lambda fixture必须同时覆盖立即完成、真实挂起、同步 resume、异步 resume、异常恢复与跨挂起 `finally`，证明两套入口共享同一个完成协议。

## 8. 测试计划

### 8.1 parser / HIR unit

- nested function type、zero-arg、suspend、nullable、generic argument、tuple/parenthesized type消歧；
- lambda 显式/期望参数、`it`、零参数、解构、尾表达式、Unit discard、匿名函数 return；
- 无 expected type推导失败、元数/参数/返回/挂起性不匹配、lambda 裸 return；
- 局部函数声明后可见、自递归、同 scope overload、前向引用拒绝、generic reference expected type推导；
- `::topLevel`、`::local`、bound final/virtual/interface/extension、receiver只求值一次、局部 `var` receiver创建时快照、重载歧义；
- callable-value binding遮蔽函数候选、局部函数候选层优先级、显式 `.invoke` 与直接调用等价；
- capture列表、传递capture、`val`/参数/`this`按值capture、统一的method-local `this` 与调用方receiver binding place分离、ref/value capture layout与稳定字段顺序；外层`var`的read/write/传递capture逐一诊断；
- 函数类型逆变/协变及普通/挂起不兼容，coercion必须产生 adapter id。

### 8.2 IR / codegen golden

- AST dump锁定所有函数类型/值语法及span；HIR dump的每个表达式都有`FunctionTypeId`，capture/call target不缺失且capture target全部immutable；
- HIR dump分别锁定export template与local-concrete实例；MIR dump锁定local lifting → closure → adapter → coroutine的结果，不再出现generic、source lambda/reference/suspend call或任何capture box/cell；
- local recursion调用lifted target并传递capture参数；bound interface reference的invoke保留interface target；
- LIR layout/RefScan锁定code pointer不扫描、captured value type内联、嵌套引用递归扫描、无额外capture allocation及indirect call unwind edge；
- LLVM IR 锁定 closure ref隐藏首参、typed return storage、indirect managed statepoint与 suspend invoke hidden ABI。

### 8.3 端到端 fixture

- 普通：无捕获/捕获 lambda、返回 closure、函数参数/字段/generic容器、匿名函数早退、引用 identity；
- capture：primitive/struct/enum/tuple按值capture、含引用value type递归扫描、显式`val snapshot`、ref-type `this` capture 修改同一class referent、value-type `this` capture 保留完整副本且不受原receiver重绑影响、显式引用状态对象由多个closure共享、循环/条件内创建多个实例；
- local：递归 factorial、捕获外层、从外层返回、generic concrete reference、局部 overload；
- reference：top-level、bound class/virtual/interface/extension，局部 `var` receiver在reference创建后重绑不改变已保存receiver，dynamic dispatch正确；
- variance：参数逆变、返回协变、值/引用返回 ABI、adapter捕获来源 closure；
- suspend：立即完成、真实挂起、异常、catch/finally、嵌套 suspend closure、普通代码仅保存后由 builder启动；
- GC：在closure创建后、间接调用中、真实挂起后及显式captured对象更新后强制collection，验证内联value capture、嵌套引用及frame链；
- 组合：lambda 作为重载实参、closure 存入 `Option` / array / struct字段、bound reference抛异常、M1–M10全量回归。

### 8.4 negative / runtime-error

- 普通 lambda 调 suspend、普通函数调用 suspend function value、普通/挂起函数类型互赋；
- lambda 无 expected type且参数缺类型、错误 `it` 元数、裸 return、匿名函数缺返回；
- lambda/匿名函数/局部函数读取、赋值或复合赋值外层`var`；generic capture不得因具体`T`为ref而放宽；
- 局部函数声明前调用、不可见 capture、无法具体化 generic reference；
- `Type::member`、构造函数引用、非法 receiver reference、把普通函数名直接当值；
- 函数类型调用使用命名实参、错误元数、缺省参数幻想或错误返回类型；
- 依靠 lambda 返回类型仍无法消除的 overload歧义；
- 直接把 closure/lambda转为 `FunPtr` 的语法在 M11 仍为未知 FFI 能力，M12 落地后必须给明确边界诊断。

## 9. 实现顺序与验收门

1. **类型与语法**：FunctionTypeId、function type parser、lambda/anonymous/reference/local function AST；完成 parser/HIR negative；
2. **普通无捕获函数值**：top-level reference、无捕获 lambda、callable-value call、LIR indirect call；
3. **捕获与局部函数**：immutable capture analysis、`var`诊断、value type内联layout、local lifting/递归、逃逸closure与GC扫描；
4. **bound reference与重载**：local candidate layer、virtual/interface/extension reference、lambda expected-type决议；
5. **函数型变**：subtyping、typed adapter closure、值/引用 ABI组合；
6. **suspend整合**：suspend lambda/reference/adapter进入 M10 transform，core lambda overload与全部恢复路径；
7. **完整回归**：强制 GC、异常、组合/negative/golden、M1–M10全量。

每一步先完成 unit/golden，再进入端到端。只把 lambda 编译成内部 direct function、不支持逃逸不算 closure 完成；只有同步 suspend lambda、没有真实 suspension 与恢复不算 suspend整合完成；只支持函数类型作为 `FunPtr` 实参而不能作为 managed 值不算 M11 完成。

## 10. 明确不做

1. **不实现 receiver function type**：扩展 reference 使用普通首参数/绑定形式；以后若增加 `A.(B) -> R`，需单独定义 receiver 调用与型变。
2. **不实现未绑定成员/构造引用**：`Type::member`、`::Type` 保留为正式诊断；现有 bound reference已覆盖 M12 callback前置需求之外的主要闭包场景。
3. **不实现 lambda non-local return**：`inline` 在 Scoop 没有语义效果；lambda 裸 return固定非法，匿名函数承担提前返回。
4. **不实现mutable capture**：外层`var`一律不能capture；不提供隐式boxing、shared cell、effectively-final推断、reference/move capture或capture list。显式状态对象是当前替代方案。
5. **不做 closure FFI**：managed closure不是 `FunPtr`；M12只接顶层 `@NoGC ::name`，runtime spec 4.3 的 GC-aware registry另行设计。
6. **不做 closure优化**：singleton、栈分配、escape analysis、direct-call消除、adapter合并都在正确基线之后。
7. **不新增调度器**：lambda形态只适配 M10 continuation原语；`launch` / `async` / dispatcher / cancellation仍属标准库。
8. **不把函数类型擦除成通用接口**：所有签名、capture、adapter与 suspend结果保持 concrete typed identity。

## 11. 文档同步项

- **spec 3 / 8.1 / 8.2**：函数类型的引用类别、型变、lambda/匿名函数、capture、局部函数、reference、suspend规则已正式化；
- **spec 11.9**：增加函数值形态协程原语重载，保留 M10 adapter协议；
- **spec 13.10**：`FunPtr<F>` 复用 8.1 的 ordinary concrete function type与中性 `::name` 语法，并明确 native-address resolution 与 managed closure隔离；
- **impl spec 2.2–2.5**：补充 FunctionTypeId、capture analysis、单态化 → closure → coroutine顺序、typed indirect call与扫描描述；
- **runtime spec 2.6 / 4.3**：managed closure内联保存value capture且不生成capture cell；native callback registration由M13作为独立能力实现；
- **ROADMAP M7/M10 backlog**：局部函数候选层及函数类型/lambda项改由 M11承接；M12 FFI设计删除临时 function-signature-only补丁。
