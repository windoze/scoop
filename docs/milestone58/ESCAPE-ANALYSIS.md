# M58：逃逸分析的最小着手点

基于 `c3b8942db` 的 MIR 实现与现行值／GC 合同。结论：可以开始，首批做**函数内的分配来源追踪，并直接用于局部 box 消除**。即时闭包和简单 class 的字段替换复用这套分析逐步扩展；不先实现通用栈对象或跨过程逃逸系统。本文是设计，尚无生产 pass 或优化后的性能数据。

## 1. 现有能力与差的那一步

[optimization.rs](../../compiler/mir-lower/src/optimization.rs) 的 release 顺序为去虚拟化 → 局部内联 → 再去虚拟化。[flow.rs](../../compiler/mir-lower/src/optimization/flow.rs) 已从 `ClassAlloc`、`ClosureAlloc`、`Box` 得到 actual type；[dispatch.rs](../../compiler/mir-lower/src/optimization/dispatch.rs) 可选择实际 class／closure 的调用目标。这些事实没有回答“这个分配的所有引用去了哪里”，也不会把 payload／字段代入后删除分配。

[MIR](../../compiler/mir/src/control_flow.rs) 已显式表达分配、局部复制、`Retype`、拆箱、字段存取和调用，适合在普通优化末尾、LIR root plan 之前进行本期分析。当前可复用 typed local／class／callable ID、表达式 visitor、[CFG 边与 SCC](../../compiler/mir-lower/src/optimization/graph.rs)、内联与 [simplify](../../compiler/mir-lower/src/optimization/simplify.rs)。不需要新依赖，也不需要修改普通 `.slib` 格式。

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

首批只为要优化的 `Box` 创建 pass 私有的 typed allocation ID。每个 local 记录“可能来自哪些候选”，另记是否可能来自候选之外。候选信息、使用位置与保留原因仅在该函数优化期间存在，不是 persistent entity，不进入 IR／meta 格式或 runtime ABI。将来加入 closure／class 消费者时再登记相应分配种类，不先写无调用者的抽象接口。

第一版使用**流不敏感的来源集合**：扫描赋值、复制及合法 `Retype`，按来源包含关系建边，以稀疏 worklist 传播并集；重赋值不会从集合中删除旧来源。分支汇合保留所有可能来源；遇到外来值也保留已有候选。这样会牺牲一些优化机会，但不需要给整个 MIR 先建 SSA 或求跨过程固定点。

求解完成后，对引用流向逐项检查。某个 sink 接收一个 local，就保留其所有可能候选；地址被取得的 local 也必须覆盖它后来可能接收的候选，不能只看取地址时已经扫描到的赋值。只对发生变化的来源边传播，避免每轮复制整份 Module；分析耗时作为编译性能指标，不建立程序合法性配额。

来源集合用于判断可能逃逸；**唯一来源本身还不足以做值替换**。消费者还需证明创建点支配相应使用、没有外来来源、payload 快照在该路径有效。多个候选汇合、无法闭合的重赋值或 EH 先保留，后续有实际收益再增加精度。

### 3.2 转移与保留规则

| MIR 形态 | 第一版处理 |
| --- | --- |
| `Local`、普通局部赋值、合法 `Retype` | 传播可能来源，不因静态类型变成 Any／interface 而丢失 |
| 对已知候选的兼容 `Unbox`、可证明的 `IsInstance` | 交给 box 消费者；结果按 payload／Boolean 表达，不把 box 身份当成结果来源 |
| 返回／抛出候选引用、global／字段／数组／原子存储 | 保留对应候选；第一版即使目标容器可能局部，也保守保留 |
| 普通未内联调用、动态调用、extern／native、callback／Context 发布 | 保留所有传入候选；用 [Call::operands()](../../compiler/mir/src/control_flow/calls.rs) 同时遍历 receiver 与 args |
| identity、pin／handle、裸地址／pointer 存储／数据借用 | 保留对象；按 typed 操作和 RuntimeFn 区分，不按符号名字猜测 |
| struct／tuple／enum／Option 中嵌入候选引用，或捕获进 closure | 首批在构造处保守保留嵌入的候选，避免“藏进值聚合后逃逸”被漏掉；不把这些情况当成无引用标量 |
| 未支持但使用候选的操作 | 保留相关候选；仅因当前消费者不会改写而保留，不把合法源码诊断成错误 |

普通赋值等纯传递边可以复用 visitor 收集，结果语义与 sink 必须按具体节点区分。例如字段读取的 object 是访问所需 receiver，不是字段值的同义来源；取数组长度也不会把数组引用作为整数结果传播。`AddressOf`、回调和原子操作不能靠只扫普通 Call 捕获。

第一版把所有尚存调用保守处理，因此返回别名的 `identity(x)` 不会成为漏洞：传入的 x 已被保留。若以后为实际热点增加 nocapture 摘要，还必须分别表达“只通过返回值传出”“字段／全局捕获”和“调用期间需要对象地址”。`@NoGC`、只读或同步返回均不能替代这些事实。[LLVM 22.1 LangRef](https://releases.llvm.org/22.1.0/docs/LangRef.html#parameter-attributes)的 `captures` 将 address／provenance 与返回位置分开，也不禁止 callee 在调用内解引用；本期不新增此类语言注解、产物摘要或 LLVM 属性推断。

### 3.3 循环内的动态实例

一个 allocation ID 表示静态创建点，不代表循环中每次执行都是同一个对象。不能因来源集合只有一个 ID，就把上一轮的别名替换为本轮的新 payload。

初版允许每轮独立的 box：创建、局部复制、检查与拆箱都在同一轮内完成，任何候选别名均不跨回边存活。用已有 CFG 遍历和必要的局部 liveness 检查这一条件；payload 快照在每次原创建点执行时重新赋值。若别名跨迭代、候选同时代表多个活实例，或无法证明边界，保留分配。无需为了避开此问题拒绝所有循环，否则会错过本期最直接的热路径。

对循环外创建、循环内反复使用的候选，仍需证明唯一创建支配所有使用，且不存在重绑定／地址观察；第一版可以保守保留复杂情形。不得以每个静态创建点一个缓存对象来“消除分配”。

## 4. 第一个消费者：局部 box 的 payload 替换

代表模式与现有 MIR 见 [ALLOCATION-ELISION.md](ALLOCATION-ELISION.md#1-局部装箱先做值替换)。建议第一笔实现直接完成以下闭环：

1. 在原创建点取得独立、typed 的 payload 快照，保持参数单次求值与顺序。原局部值后面重赋值不会改变该快照；被删除的 box 不授权删除 payload 求值中的副作用。
2. 通过上述来源／使用分析选择所有使用均可替换的候选。用现有 exact type 及 boxed 关系判断，不按类型名、字段大小或相同位表示推断兼容。
3. 用快照的值副本替换拆箱，折叠可证明的类型检查，复用现有 CFG 简化清理不可达的 cast-failure 路径。实际失败的转换、求值异常与其他可达副作用保持；不识别的转换保留原对象。
4. 删除已经无用的 box 和引用别名，保留仍需的 payload 值。一个候选存在未替换的可达使用就不删除分配，不做在罕见分支重新装箱的部分逃逸。
5. 从最终 MIR 正常产生 LIR roots 和 safepoints。含引用 payload 跨 GC 时仍需保活／重定位；值方法 receiver 取得独立副本，不别名到原 local 或 box payload。

生成检查与删除分配可作为一个函数内改写单元：先确定改写集合，再变更 CFG，最后简化。不能在分析尚使用旧 CFG 时边删边推断，也不为每个候选重新构建整模块类型／布局基础。优化模式不改变源码规则、实体身份或对外 ABI。

消费者接入 [devirt／inline 的有界迭代](ALLOCATION-ELISION.md#4-devirtinline-的交替迭代与固定点)。内联或 CFG 改写后，只重建受影响函数的候选／来源事实，不能跨 IR 版本继续使用旧 allocation ID 或 NoEscape 结论；未变化函数的结果可复用。这是同一阶段的局部反馈，不扩展成跨过程逃逸分析。

暂时保留身份可观察的 ZST box、pin／handle 和未知 boxed receiver 调用。整型 enum → 父整数的合法拆箱应明确实现既有关系，或保守保留；整数 → enum 不能通过相同 payload 伪造身份。带引用值不必一律排除，普通精确根机制已经承担其存储和移动职责。

## 5. 后续两个真实消费者

| 消费者 | 复用什么 | 新增的最小改写与限制 |
| --- | --- | --- |
| 创建后立即调用的小型 closure | 分配来源、别名／逃逸检查，现有 closure actual type 与 typed invoke 内联 | invoke 已展开后，用创建点捕获快照替换 `ClosureCapture`，才删除 `ClosureAlloc`；保留捕获求值顺序，未知 bridge／返回／存储／callback 保留 |
| 同函数内的小型普通 class | 分配来源、exact class、内联后的字段使用 | initializer 与相关方法正文已经可见且展开后，将字段读写替换成普通 typed locals；保留初始化顺序／副作用，不支持的父类、EH、release／地址／原子语义先保留 |

class 的 [lowering](../../compiler/mir-lower/src/cfg/expression.rs) 是 `ClassAlloc` → 独立 initializer 调用 → 必要的 `PublishReleaseReady`，不是一个纯字段字面量。未分析或未内联的 constructor 可以发布 receiver、抛异常或承担资源义务；仅看到分配结果局部使用，不能删除整段构造。第一批 class 候选应无 release hook，字段操作可以逐项替换；创建点的初始化和每次字段写入仍按原路径执行。

现有 [ListIterator](../../sysroot/src/scoop.core/src/iteration.scoop)、[RangeIterator](../../sysroot/src/scoop.core/src/ranges.scoop) 和 [StringIterator](../../sysroot/src/scoop.core/src/string_iterator.scoop) 是值得量化的实际候选：容器引用加少量游标字段，可能通过标量替换省掉一次局部分配。但它们还依赖调用去虚拟化、正文可见性／内联，以及循环中的字段状态；不能承诺首批 box pass 就能消除所有 `for` 的 iterator，也不把跨 Cone 通用内联加入前置条件。

若后续确实需要分析局部容器包住另一个候选，再增加对应 containment 边，并在容器逃逸时传播到所含引用。不要为第一版尚不接受的任意对象图先实现字段敏感堆分析。两类消费者都不直接扩展到 coroutine frame／adapter；协程包装仍按[前置同步内联](ALLOCATION-ELISION.md#22-第一阶段在-coroutine-lowering-前内联小型已知同步正文)处理。

## 6. 实施与验收

第一阶段一起提交分析与 box 替换，先改实现规范 2.19；语言 3.3／4.4.4、runtime 2.3 的值副本、identity 和 GC 合同不变。后续 closure／class 各自有真实源码消费者和独立收益对照后再扩展。OpenJDK 的[逃逸分析与标量替换研究](https://cr.openjdk.org/~cslucas/escape-analysis/EscapeAnalysis.html)说明了内联、合流、循环携带对象的精度问题，支持这种收窄起点；它不构成当前 JDK 或 Scoop 的性能结论。

| 验收层次 | 必要用例与指标 |
| --- | --- |
| 来源与保守性 | 别名、多次赋值、不同创建点汇合、候选／外来值汇合后返回、动态 receiver、取地址后赋值、嵌套聚合引用传出 |
| 替换语义 | 多次拆箱、原值重赋值、构造参数副作用、`is`／`as`／`as?` 成败、enum 父整数、ZST identity 保留 |
| 循环 | 每轮独立 box 正例；上轮别名、多个活实例、复杂退出保留；不能只测无循环的小函数 |
| 组合 | struct／enum／tuple 含引用跨强制移动、泛型单态化、真实 interface adapter 保留、异常与 release 不变 |
| 优化与性能 | 少量 typed MIR 断言 Box 是否消失；真实运行的 allocated bytes／对象数、minor 次数、耗时、代码体积与编译时间 |
| 后续消费者 | closure 捕获快照／副作用／逃逸；class constructor 发布／release／字段别名；迭代器的实际源码闭环 |

保守保留案例仍是合法程序，验证运行结果和分配是否按预期保留，不新增“优化失败”诊断。类型规则本身的 negative fixture 沿用正常诊断验收。全量 IR 快照、分析结果持久化和通用解释报告均非本期所需。

M56 的直接值／装箱对照只能表示这个模式的机会大小；本轮没有运行逃逸分析原型，不能预报某个固定加速倍数。正式结果应与实际 live set 的 GC 暂停一起报告：分配减少和 GC 次数下降不自动等于单次 full 暂停变短。
