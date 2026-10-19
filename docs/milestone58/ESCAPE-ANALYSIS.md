# M58：逃逸分析的最小着手点

基于 `c3b8942db` 的 MIR 实现与现行值／GC 合同。结论：可以开始，首批做**函数内的分配来源追踪，以局部 box 和内联后闭包消除为实际消费者**。内联、去虚拟化、逃逸分析和分配替换共同迭代，简单 class 的字段替换后续按收益扩展；不先实现通用栈对象或跨过程逃逸系统。本文是设计，尚无生产 pass 或优化后的性能数据。

## 1. 现有能力与差的那一步

[optimization.rs](../../compiler/mir-lower/src/optimization.rs) 的 release 顺序为去虚拟化 → 局部内联 → 再去虚拟化。[flow.rs](../../compiler/mir-lower/src/optimization/flow.rs) 已从 `ClassAlloc`、`ClosureAlloc`、`Box` 得到 actual type；[dispatch.rs](../../compiler/mir-lower/src/optimization/dispatch.rs) 可选择实际 class／closure 的调用目标。这些事实没有回答“这个分配的所有引用去了哪里”，也不会把 payload／字段代入后删除分配。

[MIR](../../compiler/mir/src/control_flow.rs) 已显式表达分配、局部复制、`Retype`、拆箱、字段存取和调用，适合在普通优化的每轮改写后、LIR root plan 之前进行本期分析。当前可复用 typed local／class／callable ID、表达式 visitor、[CFG 边与 SCC](../../compiler/mir-lower/src/optimization/graph.rs)、内联与 [simplify](../../compiler/mir-lower/src/optimization/simplify.rs)。不需要新依赖，也不需要修改普通 `.slib` 格式。

**不能直接复用 actual-type 合流作为逃逸事实。** 现有分析在不同实际类型汇合时得到 Unknown，这是去虚拟化所需的保守结果；但若一个局部值可能来自候选 A，也可能来自外部对象，逃逸分析必须仍记得 A。把它整体变成 Unknown 后忽略，会错误消除从该局部返回或存储的 A。

## 2. 三个问题要分别回答

| 问题 | 本期处理 |
| --- | --- |
| 引用是否可能离开当前函数或进入未知代码？ | 局部 may-escape 分析保守追踪所有候选来源 |
| 即使不逃逸，是否仍需要真实对象？ | `===`、pin／handle、地址观察、尚未替换的调用等仍要求保留；不把 NoEscape 当作删除许可 |
| 剩余使用是否都能由 payload／字段值表达？ | 当前消费者逐个证明并替换；全部成功才删除该分配 |

局部对象之间的 identity 比较可能完全不逃逸，第一版仍保留分配。相反，一个容器字段的标量返回不意味着容器逃逸；返回引用字段时，逃逸的是被返回的 referent，不应无条件把容器也当成返回值。第一版通过限制消费者避免引入完整堆 points-to；不能简单把每个表达式所有子节点的来源并入结果。

标量替换后的 payload 可以由 LLVM SROA／SSA 继续处理，但 LLVM 的 [SROA](https://llvm.org/docs/Passes.html#sroa-scalar-replacement-of-aggregates)主要分解局部 `alloca`，不会自动识别并消除 Scoop 的 GC 分配。把 GC 对象直接放到栈上还会涉及对象头、managed 地址、内部引用与扫描规则，明显超出这个起点。

## 3. 最小分析：保守、函数内、与消费者一起落地

### 3.1 表示与求解

首批只为要优化的 `Box` 与 `ClosureAlloc` 创建 pass 私有的 typed allocation ID。每个 local 记录“可能来自哪些候选”，另记是否可能来自候选之外。候选信息、使用位置与保留原因仅在该函数优化期间存在，不是 persistent entity，不进入 IR／meta 格式或 runtime ABI。两种分配共用来源传播与使用检查，各自实现 payload／capture 替换；将来确有 class 消费者时再登记其分配，不先写无调用者的抽象接口。

第一版使用**流不敏感的来源集合**：扫描赋值、复制及合法 `Retype`，按来源包含关系建边，以稀疏 worklist 传播并集；重赋值不会从集合中删除旧来源。分支汇合保留所有可能来源；遇到外来值也保留已有候选。这样会牺牲一些优化机会，但不需要给整个 MIR 先建 SSA 或求跨过程固定点。

求解完成后，对引用流向逐项检查。某个 sink 接收一个 local，就保留其所有可能候选；地址被取得的 local 也必须覆盖它后来可能接收的候选，不能只看取地址时已经扫描到的赋值。只对发生变化的来源边传播，避免每轮复制整份 Module；分析耗时作为编译性能指标，不建立程序合法性配额。

来源集合用于判断可能逃逸；**唯一来源本身还不足以做值替换**。消费者还需证明创建点支配相应使用、没有外来来源、payload 快照在该路径有效。多个候选汇合、无法闭合的重赋值或 EH 先保留，后续有实际收益再增加精度。

### 3.2 转移与保留规则

| MIR 形态 | 第一版处理 |
| --- | --- |
| `Local`、普通局部赋值、合法 `Retype` | 传播可能来源，不因静态类型变成 Any／interface 而丢失 |
| 对已知候选的兼容 `Unbox`、可证明的 `IsInstance` | 交给 box 消费者；结果按 payload／Boolean 表达，不把 box 身份当成结果来源 |
| 唯一来源 closure 的 `ClosureCapture` | 交给闭包消费者；以创建点捕获值替换，不把 closure 的来源误传给字段结果 |
| 返回／抛出候选引用、global／字段／数组／原子存储 | 保留对应候选；第一版即使目标容器可能局部，也保守保留 |
| 普通未内联调用、动态调用、extern／native、callback／Context 发布 | 保留所有传入候选；用 [Call::operands()](../../compiler/mir/src/control_flow/calls.rs) 同时遍历 receiver 与 args |
| identity、pin／handle、裸地址／pointer 存储／数据借用 | 保留对象；按 typed 操作和 RuntimeFn 区分，不按符号名字猜测 |
| struct／tuple／enum／Option 中嵌入候选引用，或捕获进 closure | 首批在构造处保守保留嵌入的候选，避免“藏进值聚合后逃逸”被漏掉；不把这些情况当成无引用标量 |
| 未支持但使用候选的操作 | 保留相关候选；仅因当前消费者不会改写而保留，不把合法源码诊断成错误 |

普通赋值等纯传递边可以复用 visitor 收集，结果语义与 sink 必须按具体节点区分。例如字段读取的 object 是访问所需 receiver，不是字段值的同义来源；取数组长度也不会把数组引用作为整数结果传播。`AddressOf`、回调和原子操作不能靠只扫普通 Call 捕获。

第一版把所有尚存调用保守处理，因此返回别名的 `identity(x)` 不会成为漏洞：传入的 x 已被保留。若以后为实际热点增加 nocapture 摘要，还必须分别表达“只通过返回值传出”“字段／全局捕获”和“调用期间需要对象地址”。`@NoGC`、只读或同步返回均不能替代这些事实。[LLVM 22.1 LangRef](https://releases.llvm.org/22.1.0/docs/LangRef.html#parameter-attributes)的 `captures` 将 address／provenance 与返回位置分开，也不禁止 callee 在调用内解引用；本期不新增此类语言注解、产物摘要或 LLVM 属性推断。

“传给尚未内联的调用”只是**当前正文下的保守保留原因**。调用被内联后应重新分析受影响函数，不能把旧 Escape 结论永久粘在分配点上。类似地，box 被捕获进 closure 时先保留 box；closure 消除后，下一轮按实际剩余使用重新分析 box，无需先加入完整 containment 图。

### 3.3 循环内的动态实例

一个 allocation ID 表示静态创建点，不代表循环中每次执行都是同一个对象。不能因来源集合只有一个 ID，就把上一轮的别名替换为本轮的新 payload。

初版允许每轮独立的 box：创建、局部复制、检查与拆箱都在同一轮内完成，任何候选别名均不跨回边存活。用已有 CFG 遍历和必要的局部 liveness 检查这一条件；payload 快照在每次原创建点执行时重新赋值。若别名跨迭代、候选同时代表多个活实例，或无法证明边界，保留分配。无需为了避开此问题拒绝所有循环，否则会错过本期最直接的热路径。

对循环外创建、循环内反复使用的候选，证明唯一创建支配所有使用，且没有重绑定／地址观察后可以替换。这正是 `forEach` 内联后的常见 closure：一次创建、多次读取同一捕获快照，不应因引用跨回边存活而一律拒绝。此条件与“循环内反复创建且多个动态实例跨回边存活”不同；复杂情形仍保留，不以每个静态创建点一个缓存对象来“消除分配”。

## 4. 第一个消费者：局部 box 的 payload 替换

代表模式与现有 MIR 见 [ALLOCATION-ELISION.md](ALLOCATION-ELISION.md#1-局部装箱先做值替换)。建议第一笔实现直接完成以下闭环：

1. 在原创建点取得独立、typed 的 payload 快照，保持参数单次求值与顺序。原局部值后面重赋值不会改变该快照；被删除的 box 不授权删除 payload 求值中的副作用。
2. 通过上述来源／使用分析选择所有使用均可替换的候选。用现有 exact type 及 boxed 关系判断，不按类型名、字段大小或相同位表示推断兼容。
3. 用快照的值副本替换拆箱，折叠可证明的类型检查，复用现有 CFG 简化清理不可达的 cast-failure 路径。实际失败的转换、求值异常与其他可达副作用保持；不识别的转换保留原对象。
4. 删除已经无用的 box 和引用别名，保留仍需的 payload 值。一个候选存在未替换的可达使用就不删除分配，不做在罕见分支重新装箱的部分逃逸。
5. 从最终 MIR 正常产生 LIR roots 和 safepoints。含引用 payload 跨 GC 时仍需保活／重定位；值方法 receiver 取得独立副本，不别名到原 local 或 box payload。

生成检查与删除分配可作为一个函数内改写单元：先确定改写集合，再变更 CFG，最后简化。不能在分析尚使用旧 CFG 时边删边推断，也不为每个候选重新构建整模块类型／布局基础。优化模式不改变源码规则、实体身份或对外 ABI。

消费者接入[内联、去虚拟化与分配消除的统一迭代](ALLOCATION-ELISION.md#4-内联去虚拟化与分配消除的统一迭代)。内联或 CFG 改写后，只重建受影响函数的候选／来源事实，不能跨 IR 版本继续使用旧 allocation ID 或 NoEscape 结论；未变化函数的结果可复用。这是同一阶段的局部反馈，不扩展成跨过程逃逸分析。

暂时保留身份可观察的 ZST box、pin／handle 和未知 boxed receiver 调用。整型 enum → 父整数的合法拆箱应明确实现既有关系，或保守保留；整数 → enum 不能通过相同 payload 伪造身份。带引用值不必一律排除，普通精确根机制已经承担其存储和移动职责。

## 5. 第二个消费者：内联后闭包消除

### 5.1 从 forEach 暴露的使用开始

2026-10-20 补充：将此项纳入 M58-B 首批范围，不局限于“创建后只调用一次”的 closure。标准库 [Iterable.forEach](../../sysroot/src/scoop.std/src/collections/traversal.scoop) 的正文就是 `for (element in this) action(element)`；[Sequence.forEach](../../sysroot/src/scoop.std/src/sequences/terminal_reduce.scoop) 再转发到这一操作。已知具体 closure 的 caller 内联这些小函数后，原来未知的 action 参数就变成可追踪的局部值。

以 `values: Iterable<Int>` 为例，以下示意可在调用与捕获使用全部闭合后变换：

```kotlin
val factor = 3
values.forEach { value -> consume(value * factor) }
```

```kotlin
val factor = 3
val capturedFactor = factor
for (value in values) consume(value * capturedFactor)
```

后者表达的是没有 closure 实例和环境字段的运行行为；普通常量折叠还可删掉不必要的局部值。迭代器、`consume` 本身及其分配仍各自按实际语义处理，不承诺整个 `forEach` 零分配。

变换依次消费已有能力：内联 forEach → 用 actual closure type 去虚拟化 `action(element)` → 内联 typed invoke → 将 `ClosureCapture` 替换为创建点取得的 typed 捕获快照 → 清理引用别名并删除 `ClosureAlloc`。它们可能分布在不同优化轮次，见[统一迭代](ALLOCATION-ELISION.md#43-初版调度与停止条件)。现有 [dispatch.rs](../../compiler/mir-lower/src/optimization/dispatch.rs) 已能选择实际 invoke；只去虚拟化而不展开正文时，direct 调用仍传 closure receiver，不能因此删掉其环境。

首版沿用已有内联选择，不强制展开任意大 lambda，不增加把捕获改成另一套函数参数的专化 ABI。forEach 尚未内联时，函数内分析可保守保留其 closure 实参；内联后再分析即可，无需先跨过程分析参数。正文不可见、内联增长限制或剩余调用导致环境仍被使用时正常保留。

**内联并不自动等于不逃逸，不逃逸也不自动允许删除对象。** 被内联的高阶函数若把 action 写入字段、返回给外部或交给未知调用，仍保留；局部 identity／地址观察也先保留。所有剩余使用都可改成普通值操作时才删分配。

语言规范 [8.1.3](../specs/SCOOP-SPEC.md#813-捕获与局部函数)只允许捕获不可重新绑定的 binding，外层局部 `var` 读写原本就是编译错误；不引入隐式 shared cell。值类型在创建点复制完整值，引用类型复制引用，捕获的引用对象仍可被修改。替换必须保存创建点、单次求值及原顺序，不能在每次 invoke 时重新读取 bound receiver 或其他创建表达式，也不能把引用对象的字段值冻结成创建时快照。

当前 [closure allocation lowering](../../compiler/mir-lower/src/body/expressions.rs) 按语义顺序携带 capture initializer 及物理字段索引；[closure 字段布局](../../compiler/mir-lower/src/closures/lambdas.rs) 可以重排字段。改写按前者求值，按字段索引匹配读取，不能按物理布局顺序重放 initializer。替换后的捕获值及其嵌套引用继续走普通精确 roots 和移动回写；loop 内的调用／poll 不因环境消失而失去保活。

[Kotlin inline functions](https://kotlinlang.org/docs/inline-functions.html)展示了内联高阶函数与 lambda 后省去函数对象的同类效果。Scoop 在现有普通优化中实现上述局部模式，不增加 inline 语法、non-local return 豁免或捕获规则。

### 5.2 分开检查实例分配与残留支持产物

删除 `ClosureAlloc` 首先省掉运行时对象。当前 [MIR closure 声明](../../compiler/mir-lower/src/closures/lambdas.rs)先建立完整 class／invoke；LIR 的 [descriptor](../../compiler/lir-lower/src/metadata/descriptors/functions.rs) 和 [layout](../../compiler/lir-lower/src/metadata/layouts.rs) 生成还会遍历所有 closure class。因此不能把“分配已消失”直接当作 TD、scan、layout、invoke／bridge 已不发射。

在普通迭代结束后，以既有 typed 引用与 Strong／ODR 输出归属检查这些生成支持项：若具体 closure 的全部实例与相关 callable／物理引用均已消失，且没有现行发布合同要求保留，则在物理输出选择时省掉其私有环境布局、TD／scan 和无用 invoke／bridge。其他创建点仍使用同一 class、invoke 仍有直接引用，或输出绑定仍需该项时保留；函数类型 TD 也可能因其他函数值或运行时类型检查而独立需要。

这一清理只闭合本次消除产生的无用支持项，复用已有实体身份与输出选择，不另建全程序 DCE 系统。保留的 MIR 记录仍结构完整，不通过空捕获类型、缺失 invoke 或伪造 layout 表示“优化掉了”。实现前同步修改实现规范 2.3／2.19 及相关物理输出条款；验收分别检查动态分配和最终支持产物，不把仅保留的语义身份记录算作运行时 footprint。

### 5.3 后续 class 消费者

同函数内的小型普通 class 可以复用分配来源、exact class 与内联后的字段使用：initializer 与相关方法正文可见且展开后，把字段读写替换成普通 typed locals；不支持的父类、EH、release／地址／原子语义先保留。

class 的 [lowering](../../compiler/mir-lower/src/cfg/expression.rs) 是 `ClassAlloc` → 独立 initializer 调用 → 必要的 `PublishReleaseReady`，不是一个纯字段字面量。语言规范 9.1.1 禁止发布或捕获 initializing receiver，但 constructor 仍可执行普通调用、分配、抛异常或承担资源义务；仅看到分配结果局部使用，不能删除整段构造。未内联 initializer 的保守保留来自尚需真实 receiver 和字段存储，不能误写为语言允许构造器直接发布 `this`。第一批 class 候选应无 release hook，字段操作可以逐项替换；创建点的初始化和每次字段写入仍按原路径执行。

现有 [ListIterator](../../sysroot/src/scoop.core/src/iteration.scoop)、[RangeIterator](../../sysroot/src/scoop.core/src/ranges.scoop) 和 [StringIterator](../../sysroot/src/scoop.core/src/string_iterator.scoop) 是值得量化的实际候选：容器引用加少量游标字段，可能通过标量替换省掉一次局部分配。但它们还依赖调用去虚拟化、正文可见性／内联，以及循环中的字段状态；不能承诺首批 box pass 就能消除所有 `for` 的 iterator，也不把跨 Cone 通用内联加入前置条件。

若后续确实需要分析局部容器包住另一个候选，再增加对应 containment 边，并在容器逃逸时传播到所含引用。不要为第一版尚不接受的任意对象图先实现字段敏感堆分析。两类消费者都不直接扩展到 coroutine frame／adapter；协程包装使用[前置同步内联](ALLOCATION-ELISION.md#22-在-coroutine-lowering-前内联小型已知同步正文)和[尾部 completion 转交](ALLOCATION-ELISION.md#23-真正尾部-suspend-调用直接转交-completion)。

## 6. 实施与验收

第一笔实现一起提交分析与 box 替换，再以同一迭代补齐 forEach 闭包消除，两者均属于 M58-B 首批交付。先改实现规范 2.19；closure 支持产物另同步 2.3 及输出条款。语言 3.3／4.4.4／8.1、runtime 2.3／2.6 的值副本、identity、捕获和 GC 合同不变。后续 class 有实际收益对照后再扩展。OpenJDK 的[逃逸分析与标量替换研究](https://cr.openjdk.org/~cslucas/escape-analysis/EscapeAnalysis.html)说明了内联、合流、循环携带对象的精度问题，支持这种收窄起点；它不构成当前 JDK 或 Scoop 的性能结论。

| 验收层次 | 必要用例与指标 |
| --- | --- |
| 来源与保守性 | 别名、多次赋值、不同创建点汇合、候选／外来值汇合后返回、动态 receiver、取地址后赋值、嵌套聚合引用传出 |
| 替换语义 | 多次拆箱、原值重赋值、构造参数副作用、`is`／`as`／`as?` 成败、enum 父整数、ZST identity 保留 |
| 循环 | 每轮独立 box；循环外一次创建的 closure 在内层循环反复调用；上轮别名、多个活实例、复杂退出保留 |
| 闭包 | forEach 运行零／一／多次，值捕获副本、引用捕获共享可变对象、bound receiver 单次求值；跨调用／poll 的移动 GC；逃逸与 identity 观察保留 |
| 交替迭代 | forEach 内联后才解析 invoke；invoke 内联后消除环境；捕获 box 的 closure 消除后重算来源并继续删 box；旧 Escape 不跨改写永久保留 |
| 组合 | struct／enum／tuple 含引用跨强制移动、泛型单态化、真实 interface adapter 保留、异常与 release 不变 |
| 优化与性能 | 少量 typed MIR 断言 Box／ClosureAlloc 是否消失；真实运行的 allocated bytes／对象数、minor 次数、耗时、代码体积与编译时间 |
| 支持产物 | 全部使用消失时不发射私有环境 TD／layout／scan 与无用 invoke／bridge；同 class 其他实例仍逃逸、callable 引用和发布绑定保留；不误删共享函数类型 TD |
| 后续消费者 | class constructor 副作用／异常／release／字段别名；初始化 receiver 的现行禁止发布规则；迭代器的实际源码闭环 |

保守保留案例仍是合法程序，验证运行结果和分配是否按预期保留，不新增“优化失败”诊断。类型规则本身的 negative fixture 沿用正常诊断验收。全量 IR 快照、分析结果持久化和通用解释报告均非本期所需。

M56 的直接值／装箱对照只能表示这个模式的机会大小；本轮没有运行逃逸分析原型，不能预报某个固定加速倍数。正式结果应与实际 live set 的 GC 暂停一起报告：分配减少和 GC 次数下降不自动等于单次 full 暂停变短。
