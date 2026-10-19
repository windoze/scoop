# 局部装箱与协程包装消除：M58 调研

本文属于 [M58 设计](DESIGN.md)。2026-10-19 的源码样本基线为 `04d28e763`；在 `c3b8942db` 上复核，相关 MIR 优化与 coroutine lowering 顺序未变化。局部装箱作为本期[函数内逃逸分析](ESCAPE-ANALYSIS.md)的首个消费者；协程包装分别通过真正尾部调用的 completion 转交和已知同步正文的前置内联消除。目标是在保持值、GC、协程语义的前提下减少短命对象，本轮尚未实现生产优化。

## 1. 局部装箱：先做值替换

### 1.1 当前已经知道类型，但没有消除 box

现有 MIR release 顺序是去虚拟化、局部内联、再去虚拟化，见 [optimization.rs](../../compiler/mir-lower/src/optimization.rs)。[flow.rs](../../compiler/mir-lower/src/optimization/flow.rs) 已能把 `Box(value)` 映射到其实际 boxed class，不能笼统说编译器完全不知道 box 的类型；但这个事实目前用于调用目标选择，不包含 payload 替换或 box 生命周期分析。

本次从源码编译以下模式，release MIR 仍保留 `Box → Any 局部别名 → IsInstance → 条件抛错 → Unbox`：

```kotlin
struct Pair(val left: Int, val right: Int)

fun localBox(value: Int): Int {
    val boxed: Any = Pair(value, value + 1)
    val copied = boxed as Pair
    return copied.left + copied.right
}
```

constructor 已被局部内联，但 box、cast 隐藏局部值和异常分支仍在。因此只匹配字面上的 `Unbox(Box(v))` 不够，必须穿过真实 lowering 产生的局部复制及检查 CFG。[casts.rs](../../compiler/mir-lower/src/body/casts.rs) 在转换处先把输入求值一次，再建立 `Is`／异常路径；优化应复用这个顺序。

该源码与下节的协程样本在私有 sysroot 中完成 release 构建、MIR 检查和运行，输出 `41`、`42`。本轮确认的是当前产物形态，不是消除优化前后计时。已有一百万次 Long Pair 的直接值／局部 Any 对照见 [M56 原始实验记录](../milestone56/PROGRESS.md)：1.025／87.994 ms，192／32,000,192 allocated bytes，0／30 次 minor；它只表示此模式的机会，不能当成一个通用 pass 的实测加速比。

### 1.2 最小可交付范围

在现有 MIR 内联／去虚拟化之后、进入 LIR 和 root plan 之前，使用 [M58 函数内逃逸分析](ESCAPE-ANALYSIS.md)完成局部 box 替换，不再另设一套只匹配装箱语法的生命周期分析。初版以**单个静态 box 创建点**为候选，但必须区分循环中的动态实例：

1. 在创建点保存独立的 payload 值快照。即使源 local 后面重新赋值，拆出的值也必须是当时装箱的副本；有副作用的构造参数只能求值一次。
2. 沿 `Local`／普通复制／合法 `Retype` 传播可能来源的并集，重赋值和外来值汇合不能丢掉原候选。替换只接受唯一候选且没有外来来源、创建点支配使用的情况；允许多个别名和多次拆箱。取地址、循环携带别名、多个活实例和难以闭合的 EH 初版保守排除；每轮创建并用完的独立 box 可以接受。
3. 仅接受已证明兼容的 `Unbox`、类型测试和消除后不再需要的别名。按现有 exact type／boxed 继承关系判断，而不是比较类型名或 payload 大小。把可证明的类型测试折叠，交给已有 CFG 简化删除不可达的 cast-failure 路径。
4. 只有全部可达使用都可替换，才删除 box 分配和相应 Any 存储。不做“先删除，在罕见分支重新装箱”的部分逃逸处理。
5. LIR 从最终 MIR 正常生成 roots。如果 payload 含引用，继续按值扫描描述保活和重定位；不保留一个假的堆对象，不把栈地址强转为 managed object。

初版把 box 替换接入第 4 节的有界局部优化迭代，先化简调用与 CFG，再分析实际剩余使用；没有事实变化不重新求解。去虚拟化／内联后已经展开为独立值 receiver 的调用，可在普通使用分析闭合后受益；仍需 boxed receiver ABI 的接口 adapter 先保留。函数内来源／使用分析随后可以服务即时 closure 与简单 class；若真实样本主要卡在 adapter，再单独扩展对应 typed 转换，不先铺设跨过程逃逸框架。

### 1.3 必须保留的语义

| 使用 | 初版处置 | 原因 |
| --- | --- | --- |
| 精确类型的局部拆箱、`is`／`as`／`as?` | 可在类型事实完整时替换 | 保留成功值与失败行为，不能一律删除检查 |
| 返回、写入对象／数组／global、捕获到逃逸 closure | 保留 box | 引用身份和生命周期可能被外部观察 |
| `===`、pin／handle、native 暴露、未知调用 | 保留 box | 初版不推理可观察身份；`@NoGC` 调用也不代表不逃逸 |
| ZST 装箱 | 只有身份不可观察才可删除 | 现有合同仍要求各次可观察装箱有独立 identity，不能全局共享一个 box |
| 含引用 struct／enum／tuple | 保留 payload 中的引用及复制语义 | box 不可变不等于其字段引用指向的对象不可变 |
| 整型 enum 向父整数拆箱 | 按既有 boxed TD 父子关系处理，或初版保留 | enum → 底层整数允许；反向不能按数值伪造 enum 身份 |
| 失败转换、构造异常、求值副作用 | 保持原执行路径 | 删除分配不授权删除真正会发生的异常或重复求值 |

运行时规范 2.3、语言规范 3.3／4.4.4 是主要约束。装箱后的值字段不可变，调用值方法取得独立 receiver 副本；分析对象应是“装箱点的值快照”，不能让 receiver 直接别名到原变量或 box payload。普通局部值仍可由后续 LLVM SROA 分解，不需要先设计新的栈对象 ABI。

OpenJDK 的[逃逸分析与标量替换研究报告](https://cr.openjdk.org/~cslucas/escape-analysis/EscapeAnalysis.html)讨论了内联、控制流汇合及循环携带对象对消除效果的影响。这是历史机制研究，用来解释为何先限制局部模式，不作为当前 JDK 能力或 Scoop 加速幅度的结论。

## 2. 协程包装：在状态机生成前区分同步与尾部调用

### 2.1 当前顺序造成的成本

当前 [pipeline.rs](../../compiler/mir-lower/src/pipeline.rs) 先执行 `transform_suspend_abis()`，再执行 `coroutine::transform()`；普通 `optimization::run()` 则在 [current.rs](../../compiler/mir-lower/src/current.rs) 的 `lower_with_dependencies()` 返回后运行。普通内联器的 [protocol_entries](../../compiler/mir-lower/src/optimization/inline/policy.rs) 还主动排除了 coroutine entry、driver、resume 和 start helper。不能期待简单增大当前 inline 阈值解决此问题。

`transform_suspend_abis()` 把返回类型改成 `CoroutineStep<T>` 并追加 `Continuation<T>`。完全没有 suspend site 的函数已经是 `CoroutineLowering::Immediate`，不建 frame。当前 [coroutine.rs](../../compiler/mir-lower/src/coroutine.rs) 根据登记的调用点分析 site，不因为 callee 本身是 Immediate 就自动忽略它。

本次 release 样本：

```kotlin
suspend fun leaf(value: Int): Int = value + 1
suspend fun wrapper(value: Int): Int = leaf(value) + 1
suspend fun outer(value: Int): Int = wrapper(value) + 1
// SuspendTask.run() 返回 outer(39)，由 startCoroutine 启动。
```

MIR 中 leaf 没有 frame；wrapper、outer、Task.run 各有一份 `CoroutineFrame` 和一份 `CoroutineAdapter` 定义及相应分配路径。同步执行得到 42，但不会因此免掉包装。这里不把 Task、用户 Completion 或 start helper 自身分配算作可消除对象。

在 [sites.rs](../../compiler/mir-lower/src/coroutine/sites.rs) 中，site 在调用 callee **之前**保存 live values、发布 suspended state、分配 adapter 并将其传给 callee；返回 Completed 后才争取状态所有权、提取值并继续执行。调用期间可能发生同步恢复、跨线程竞争或 continuation 逃逸，因此不能把 adapter 延迟到“发现返回 Suspended 之后”再分配。

已有十万次同步启动的源码对照为：零／一／四层包装分别 6.107／12.361／30.731 ms，2.40／13.60／49.60 MB 分配、2／13／47 次 minor，见 [M56 原始实验记录](../milestone56/PROGRESS.md)。本次没有重新计时，也没有把源码改写的差值报成编译器优化结果。

### 2.2 在 coroutine lowering 前内联小型已知同步正文

建议顺序：完成 typed MIR 正文与调用点登记 → 有界迭代目标解析／去虚拟化、同步正文内联与 site 清理 → 识别尾位置并转换 suspend ABI（尾调用保持原 step 转交）→ 为仍需恢复状态的函数生成状态机 → 普通 MIR 有界优化迭代。两个优化位置分别承担同步 site 消除和普通调用／分配优化，见第 4 节。

初版只做同一 Cone、直接目标、正文已知的小函数。无 suspend site 的 source body 是起点；将它内联到 wrapper 后，如果 wrapper 也不再包含 site，就可以继续处理其直接 caller。递归／互递归先保留，复用已有局部代码增长选择，更新受影响 caller 的事实，不在所有函数上反复克隆整份 Module。

这里的事实是“本次调用不挂起且不观察／发布调用者的 hidden completion”，不是“纯函数”或“没有 GC”。普通分配、其他正常调用和副作用可以存在，只要保持原顺序并继续正常生成 safepoint；初版为复用现有 clone 能力可先限制无 EH 的小 CFG。未知外部 suspend 调用、动态派发未闭合、`suspendCoroutine` 等保留原 site；某次运行返回 Completed 不能作为静态证明。

取消仍按语言规范 8.2／11.9.1 由普通 core 库表达，并非每个 suspend 调用的隐式动作；保留实际取消检查与异常传播，不为了优化增加或删除此类调用。

前置内联优于直接复用物理 ABI 的一个实际原因是类型：wrapper 的 `Continuation<T>` 不一定能作为 callee 的 `Continuation<U>`。即使知道 callee 不用 completion，也不能传 null、伪造 interface 或任意重解释类型。先在 source signature 层替换正文，可以让已有无 site 路径工作，无需新增同步 ABI、发布摘要格式或第二个入口体系。

实现时需要复用已有 MIR clone 中的实际能力：实参单次求值、local／block／loop ID 重映射、return 汇合、typed callable 归属。新增入口同时更新 pre-coroutine call-site registry，不能只改 CFG 而留下旧 site。若某个模式必须复制 EH 或 coroutine 协议元数据，初版就保留调用，不为其扩建通用变换层。

### 2.3 真正尾部 suspend 调用：直接转交 completion

2026-10-20 补充：这一项应作为 M58-C 的优先基础优化，不必等同步内联完成。只要函数的所有可达挂起点都处于真正尾位置，且没有自己的恢复／清理义务，就可以不生成该函数的状态机、frame 和 site adapter；**不要求 callee 同步完成，也不要求内联其正文**。

例如：

```kotlin
suspend fun wrapper(value: Int): Int {
    val adjusted = value + 1
    return child(adjusted)
}
```

可以按现有 ABI 降为以下伪代码：

```text
wrapper(value: Int, completion: Continuation<Int>) -> CoroutineStep<Int>:
    adjusted = value + 1
    return child(adjusted, completion)
```

child 返回 Completed 时，wrapper 原样返回；child 返回 Suspended 时，wrapper 也原样返回，后续由 child 沿同一个 completion 恢复原 caller。wrapper 没有恢复入口，也不再次通知 completion。普通调用前计算、参数单次求值与正常 Managed GC 合同保留；这不意味着整个调用链没有状态机或分配。

初版按实际 CFG／cleanup 信息识别，采用以下边界：

| 情形 | 处置 |
| --- | --- |
| `return child(args)`，source 结果 exact type 相同，无后续工作 | 直接转交，callee 可以真正挂起 |
| 多个分支分别尾调用，或某分支直接返回普通值 | 可以整体无帧：普通值路径构造 Completed，尾调用路径原样返回 step；每条挂起路径分别满足条件 |
| `return child(args) + 1`、拆箱／转换／丢弃结果后返回 Unit | 仍有结果处理，首版保留原 lowering；物理布局相同不等于 `Continuation<T>`／`CoroutineStep<T>` 可互换 |
| 先 `val x = first()` 再 `return child(x)`，或 `return child(first())` | first 是非尾部挂起点，函数仍需保存恢复状态；最后一行是尾调用不足以消除整个状态机 |
| 包围调用的 catch／finally、Context restore、pending cleanup | 保留，异常恢复和真实完成后仍有本函数工作；不把“返回 Suspended”误当源码作用域退出 |
| 结果 ABI 完整且匹配的动态／外部 suspend 目标 | 机制上同样可以转交，无需知道 callee 正文；首批先闭合同一 Cone 的直接调用，再按已有 typed ABI 扩展 |

前置同步内联可以使原来的非尾部 site 消失，进而让整个函数成为尾调用包装。因此在前置迭代化简后再判定 tail 资格，但 tail 转交本身不必展开 child，也没有内联导致的代码增长。首版只交付“整函数全部可达挂起点均为尾部”的无帧路径；已经需要状态机的函数，只省其最后一个 site 是另一项优化，不作为前置。

当前 [transform_suspend_abis](../../compiler/mir-lower/src/pipeline/coroutines.rs) 将普通 `return value` 包成 Completed，之后 [coroutine::transform](../../compiler/mir-lower/src/coroutine.rs) 对存在 site 的函数建立状态机，尚未区分 tail。实施时应先识别尾位置，在 ABI 转换时追加原 completion，并让相应 return 原样交付 callee 的 step；正常返回仍构造 Completed。同步更新对应 call-site 记录，避免状态机生成器再次为这些调用创建 adapter，不能只删除 site 而留下缺 hidden 参数的调用或嵌套的 Completed。

**无帧不等于静态不挂起。** 当前 `CoroutineLowering::Immediate` 用于没有 site 的立即返回路径。加入尾转交后，lowering 的无帧表示与“已证明同步”的分析事实必须分开；不能因 frame／site 消失就把 wrapper 当成同步叶子传播给非尾部 caller。用现有 MIR 表示做必要调整即可，不改变对外 continuation ABI，也不另建证明记录。

尾转交沿用 callee 和原 caller 的单次完成协议，不能新建一个省掉一次性检查的 `suspendCoroutine` 入口。初版仍保留原语的 registration／同步完成协调。需分别验证立即 Completed、真正 Suspended 后成功／失败、同步恢复、跨线程恢复、重复完成及 register 已完成后抛错；包围调用的 catch／finally／Context 反例继续走原实现。

这也不要求 LLVM 发射机器 tail call。普通 `call; return` 已能省掉包装的堆状态；现有 stackmap、frame pointer、tail-call／tail-duplication 限制保持，深层同步调用或尾递归仍不保证常数栈空间。[Kotlin 编译器的协程代码生成说明](https://github.com/JetBrains/kotlin/blob/master/compiler/backend/src/org/jetbrains/kotlin/codegen/coroutines/coroutines-codegen.md#tail-call-optimization)也以全部挂起点处于尾位置为条件省掉 continuation class／状态机；Scoop 仍按自己的 exact `CoroutineStep<T>` 和完成协议验收。

暂不做可逃逸 continuation 的栈化、全局 adapter 池或 frame 池。这些对象的唯一获胜／一次完成状态与寿命不同于普通局部 box，把它们统一交给一般逃逸 pass 会扩大正确性范围。

LLVM 的 [CoroElide](https://llvm.org/docs/Coroutines.html#coroelide) 面向 `llvm.coro.*` 表示，会在条件成立时把 frame 存储交给 caller 并专化调用。Scoop 自己生成 frame class、adapter、状态字段和普通 GC allocation，没有使用这套 coroutine IR；单独加入该 pass 不会识别上述包装。[Kotlin 协程规范](https://kotlinlang.org/spec/asynchronous-programming-with-coroutines.html#continuation-passing-style)也区分 CPS 调用的立即返回与挂起，但不能替 Scoop 的完成协议提供消除证明。

## 3. 推荐交付顺序与验收

| 顺序 | 最小范围 | 正向与保守保留用例 | 观察指标 |
| --- | --- | --- | --- |
| 1 | 单创建点局部 box 替换 | 别名／多次拆箱／原值重赋值／含引用 payload 跨移动 GC；身份、逃逸、pin、错误转换保留 | 每操作分配次数／bytes、minor 次数、kernel、代码体积 |
| 1（同一优化循环） | forEach 等内联后的 closure 环境替换 | invoke 去虚拟化／内联后以捕获快照替换字段；循环外创建、多次调用；未知使用、identity、共享 class 的其他实例保留 | ClosureAlloc、allocated bytes、残留 TD／layout／scan 与 invoke／bridge、编译成本 |
| 2 | 同结果类型、全部挂起点均为尾部的 completion 转交 | 一／多层包装与分支；立即／真正挂起／异常／同步恢复／跨线程；前序非尾部挂起、catch／finally／Context／返回适配反例 | 无自身 frame／adapter、包装分配 bytes、恢复与唯一完成、保留路径无回退 |
| 3 | coroutine lowering 前的小型同步 callee 内联 | 零／一／四层；不同返回类型；带普通 GC 的同步正文；未知／递归／真正挂起保留 | frame／adapter 的静态和动态数量、allocated bytes、吞吐、编译成本；内联后新成为尾调用的组合 |

普通语义 fixture 断言结果和必要诊断，少量优化专测断言目标 Box／ClosureAlloc／frame 分配是否存在及精确 root 合同，不为所有案例保存完整 MIR。原有 coroutine race、moving GC、接口／泛型组合不能被只执行同步成功路径替代。

先补实现规范 2.19 的局部替换与 pass 职责；协程还需明确对应 lowering 的处理顺序。语言规范 8.2／11.9、Context 8.3 与运行时 GC 合同保持原语义；若发现所需模式不能保持这些语义，应缩小优化范围。此次只提出方案，未修改规范中的现行行为或增加 ABI。

减少短命分配首先改善分配热路径和 GC 触发频率；删除 frame 也可能减少存活根／引用槽。**单次 full 暂停是否缩短仍取决于真实 live set**，不能把少做若干次 minor 等同于 full 的 p99 改善。应与 [GC 暂停工作量](GC-PAUSE-WORK.md) 的混合寿命负载一起验收。

## 4. 内联、去虚拟化与分配消除的统一迭代

2026-10-20 补充：将 **inline、devirt、逃逸分析、堆分配消除与 closure 消除串在同一个有界优化循环中**。内联改变可见使用，分配替换又暴露新的调用目标和字段值，因此不能只迭代 devirt／inline，最后再做一次逃逸分析。以下是源码复核与设计调整，没有新增性能测量或生产 pass。

### 4.1 现有内联已经迭代，但没有与 devirt 交错

[inline.rs](../../compiler/mir-lower/src/optimization/inline.rs) 的每个 caller 内部已经循环选择并展开直接调用，直到没有合格候选、累计增长额度用完或达到嵌套深度限制。因此并非每次 `inline::run()` 只展开一层；当前每个 caller 的增长额度为 `min(original_cost / 2 + 96, 384)`，展开深度限制为 6。正文按一次调用开始时的模板快照消费，避免函数遍历顺序意外带入已经展开的调用链。

但该内部循环只接受直接目标，不执行去虚拟化。典型机会是 caller 把一个具体 class 作为 interface 实参传给小型 helper：helper 内原本不知道 receiver 的实际类型，内联进 caller 后才可证明其目标。现有最后一次 devirt 会将该调用改成 direct，随后却不再内联它；目标正文中的字段、值 receiver 或拆箱使用也就无法继续暴露。

反复调用 devirt 而不改变 IR 通常不会多得到事实，因为单次类型传播已经求解到稳定。产生新机会的是**内联、常量／CFG 化简或标量替换改变了输入**：下一次 devirt 可解析新的目标，下一次 inline 又暴露新的局部操作。仅增加轮次仍不会凭空消除 Box／ClosureAlloc，需要本期的 payload／capture 替换消费者。

forEach 是直接例子：在独立的泛型正文中 action 只是函数参数，尚不知道本次调用传入哪一个 closure；caller 中先内联 forEach，随后才能用具体 closure 类型解析 invoke，并内联 lambda 正文。函数内逃逸分析在此之前把 action 实参保守保留即可，调用展开后重算事实，不必先建立跨过程参数分析。内联的是已知调用点的正文，不能把一个函数值的创建本身当作调用展开。

### 4.2 固定点存在于什么意义上

设一次普通优化改写组合为 `F = 化简 ∘ 局部分配消除 ∘ 化简 ∘ 内联 ∘ 去虚拟化`，其中分配消除基于本轮最新正文的来源／逃逸／使用分析，包含 box 和 closure 两个消费者。若执行后没有实际改写，便得到本组规则及当前策略下的固定点；不同执行顺序或内联选择可能到达不同结果，它不是全局最优代码。启发式带有剩余增长额度／展开历史时，F 的状态还包括这些信息，不能只比较 MIR 却在下一轮重新初始化策略状态。

要区分两类固定点：

- **固定 IR 上的分析**：类型／来源集合在有限事实域中按既定合流传播，可以求解到稳定。未知事实、多个可能来源与控制流汇合仍必须保持保守性。
- **改变 IR 的优化组合**：内联会复制调用、分配与 CFG，分析域随之变化；不能直接套用有限数据流域的收敛论证。没有递归的有限调用图可以完全展开而终止，但展开量可能很大；递归或去虚拟化后才显露的调用环可能持续产生候选。

采用不反向重新虚拟化／装箱的局部规则、有限的内联展开范围和跨轮不重置的既有增长限制，可以使允许的改写最终耗尽。此时得到的是**策略约束下的局部固定点**。若先触及轮次上限则只是提前停止，不能记录为“已收敛”；剩余调用与分配照常保留，程序行为不变。

LLVM 的 [DevirtSCCRepeatedPass](https://llvm.org/doxygen/classllvm_1_1DevirtSCCRepeatedPass.html)也是在间接调用变成直接调用后重跑 SCC pass，并限制最大迭代次数。[接口源码注释](https://llvm.org/doxygen/CGSCCPassManager_8h_source.html)说明，每次可能只改善一个调用点，重复次数会很大。Scoop 可以采用这种调度思路，无需复制 LLVM 的完整 CGSCC pass manager。

### 4.3 初版调度与停止条件

普通 MIR 优化阶段可先对照最多 1／2／3／4 轮，以 3 轮作为待验证的起始选择；不是已经确定的性能最优值。每轮大致为：

```text
affected_functions = optimizable_functions
for round in 0 .. max_rounds:
    changed = devirtualize(affected_functions)
    changed |= inline(affected_functions, existing_inline_state)
    changed |= simplify(affected_functions)
    facts = analyze_allocation_uses(affected_functions)
    rewrites = plan_box_and_closure_elision(facts)
    changed |= apply_local_elision(rewrites)
    changed |= simplify(affected_functions)
    if not changed:
        break
    affected_functions = changed_and_dependent_callers(changed)
```

逃逸分析是为当前改写提供事实的分析步骤；堆分配消除与 closure 消除是实际消费者，复用同一来源／别名检查，分别替换 payload 与捕获字段。这里的计划只是函数内一次改写所需的私有数据，不引入新的 IR 或通用 pass 框架；没有可替换使用时，即使 NoEscape 也保留对象。首版通过值替换删掉对象，不把所有 NoEscape 对象自动改成栈对象。

循环从 devirt 或 inline 开始只是调度选择：已有直接 forEach 可以在第一轮 inline，新暴露的 closure 调用在下一轮 devirt／inline。下表是可能需要多轮的依赖关系示意，不要求实现机械地每轮只做一步：

| 轮次 | 改变的输入与新机会 |
| --- | --- |
| 1 | forEach 内联到 caller，action 关联到具体 closure；invoke 调用仍在时保守保留环境 |
| 2 | 具体 invoke 去虚拟化并内联，捕获读取可替换，closure 分配和别名消失 |
| 3 | 某个 box 原先只因被 closure 捕获而保留；该 sink 消失后重新分析，剩余使用若全是可替换的拆箱／检查，就继续删除 box |

内联也可能暴露真实字段存储或未知调用，使分配仍需保留；不能把“高阶函数已经内联”当作不逃逸证明。旧 Escape／NoEscape 结论、candidate ID 和改写位置只对应分析时的正文；一次替换改变使用关系后，不用旧事实继续推导新的删除。首版先应用无冲突的已确认改写，新增机会交给下一轮，或在确有需要时局部重算。

各步返回实际发生改写的函数集合即可，不能把“运行过分析”算作 changed，也不靠序列化／哈希整份 MIR 检测变化。新直接目标、内联与可达性变化使相关分析失效；更新被修改函数，callee 正文变化若影响调用点选择则通知其实际 caller。没有变化的函数不重复求解，不另建通用查询框架。

最后一轮若仍有改写但达到上限，则保留剩余调用与分配，记录为达到限制；不能使用已失效事实追加未经分析的消除。普通迭代结束后，再按实际引用清理本次消除留下的私有 closure 支持产物，然后进入 LIR roots／布局生成，详见[实例与支持产物的区别](ESCAPE-ANALYSIS.md#53-分开检查实例分配与残留支持产物)。

不能在外面简单循环调用当前 `inline::run()`：它每次克隆整份函数表、重新设定 caller cost／remaining，并将所有块的 depth 置零，会绕过原来的累计增长与嵌套限制。应在**同一个内联优化阶段**中保留原始 cost、已使用增长额度与展开历史；化简删掉代码不自动返还额度。第一版可复用该阶段入口的稳定模板，在 caller 内交错去虚拟化／内联；若以后消费更新过的 callee 正文，需要连同其展开历史处理，不能把深层已展开正文计为一次深度。

去虚拟化可能揭示原直接调用图没有的递归边。新目标参与内联前，应刷新受影响的递归判断或检查实际展开祖先；不能一直沿用第一轮仅按 direct call 得到的 SCC 结果。保留现有保守递归策略，不靠反复重置轮次或增长额度处理递归。上述限制仅决定优化选择，不进入源码合法性、产物兼容性或 runtime ABI。

### 4.4 同步协程要在正确的阶段迭代

同样的交替思路适用于 `leaf → wrapper → outer`：叶子同步正文内联并清掉 site 后，wrapper 也可能成为无 site 的同步候选，再处理 outer。对有限、非递归的候选调用链，已确认同步的事实可以沿 caller 传播到固定点；使用 caller 工作队列可以避免为每层包装扫描全部函数。

但普通优化的位置仍晚于状态机生成，且 `protocol_entries` 排除协程 entry／driver／resume／start helper。**在现有位置多跑几轮不会绕过这两个限制。** 前置迭代必须作用于具有完整 typed 正文和调用点信息的 source-signature 阶段；复用局部分析／clone 能力，而非把依赖完整 Module 的普通优化器原样放进尚未完成的 lowering shell。

前置每轮只内联符合第 2.2 节条件的已知同步 callee，同时更新 suspend site registry、异常出口与 local／block 身份，再传播受影响 caller 的同步事实。一个“不会挂起”的分析结论本身不删除 frame／adapter，仍要实际消除对应 site；超过增长／轮次限制而留下的调用继续走现有协议。正式状态机只在前置迭代结束后生成一次，不反复生成再拆除。

第 2.3 节的尾转交在前置化简后独立判断；它省掉本函数的恢复状态，却不证明 child 或 wrapper 不挂起。不能将尾转交移除的 site 混入“已内联同步 site”的结果，误用于后续同步性传播。

### 4.5 新增验收

增加一组需要交替才能消除调用的真实源码：interface helper 内联后暴露具体 receiver、两级间接调用链、拆箱／类型检查化简后出现新的机会，以及 forEach 内联 → invoke 展开 → closure 消除 → 捕获 box 再消除。检查第二／三轮是否实际减少剩余调用与分配，并验证高阶函数真实保存 action 的反例。另覆盖无变化早停、去虚拟化后显露递归、跨轮增长／深度不重置，以及参数副作用只执行一次。

协程使用零／一／四层同步包装、普通 GC 副作用、动态目标、递归和真正挂起反例，分别观察前置迭代消失的 site 与最终 frame／adapter。比较 1／2／3／4 轮的编译耗时、峰值内存、产物体积、剩余调用／分配和运行收益，并区分无变化停机与达到限制停机；只对实际变化正文刷新分析，不增加全量 IR 快照或编译过程证明。

## 5. 尚未消除的 runtime 调用：IR 候选

2026-10-20 核对生产代码后，普通对象的 TLAB cursor 检查／推进、条件 poll、单引用字段卡表屏障已有 codegen 快路径；TLAB 完成发布仍调用 runtime，范围屏障等也并非全部内联。不能把 M56 已完成的部分再次列为新收益。

| 候选 | 当前路径与建议 | 边界 |
| --- | --- | --- |
| box／unbox | LIR 已有专门指令，codegen 仍调用 C；unbox 还取得 heap lock、查询 exact size 并检查 shape。先做 M58-B 的局部消除，剩余静态布局已知的操作再考虑分配／类型检查／load／copy IR | 含引用 payload 必须跨分配 safepoint 保活并恢复；保留必要动态类型检查，C 边界与生成代码职责分别表达，不复制热路径校验 |
| String byteLength／byteAt | `strings.c` 前者只读 len，后者为边界检查加 byte load，适合直接生成 IR，暴露重复读取与检查消除机会 | 字符数量 length 要扫描 UTF-8，不等同于 byteLength；UTF-8 解码等复杂算法先保留 C |
| Array clone | codegen 已知 source／target 的元素和布局，当前仍调用 C clone；可复用分配 IR、memcpy 与必要范围屏障 | source 必须在目标分配后使用 relocation 更新地址；名义 Array／MutableArray TD 仍分别正确 |
| is-instance／itable lookup | 先利用 exact receiver 与 devirt 消除；剩余可考虑 exact TD 指针比较快路径 | 父类、接口、函数类型变体等动态关系不能全部退化成指针相等；复杂慢路径保留 runtime |

这部分为源码核对与后续候选，没有宣称迁移后的性能结果，也不扩大 M58 的交付门槛。优先在 typed MIR／LIR 保留语义，codegen 只负责最终布局／GC 相关发射；不把上游语义分析搬到 codegen，不一次性将整个 C runtime 展开。收益依据应是消失的调用、重复检查、root frame 与复制，以及 LLVM 获得的静态信息；GC 协调、分配慢路径、异常和 OS 操作继续留在 runtime。
