# 局部装箱与同步协程包装消除：M58 调研

本文属于 [M58 设计](DESIGN.md)。2026-10-19 的源码样本基线为 `04d28e763`；在 `c3b8942db` 上复核，相关 MIR 优化与 coroutine lowering 顺序未变化。局部装箱作为本期[函数内逃逸分析](ESCAPE-ANALYSIS.md)的首个消费者；同步协程包装采用独立的前置内联路径。目标是在保持值、GC、协程语义的前提下减少短命对象，本轮尚未实现生产优化。

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

初版可在现有优化序列末尾执行一次，然后复用 simplify；没有事实变化不重新全图求解。去虚拟化／内联后已经展开为独立值 receiver 的调用，可在普通使用分析闭合后受益；仍需 boxed receiver ABI 的接口 adapter 先保留。函数内来源／使用分析随后可以服务即时 closure 与简单 class；若真实样本主要卡在 adapter，再单独扩展对应 typed 转换，不先铺设跨过程逃逸框架。

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

## 2. 同步协程：优化必须早于状态机生成

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

### 2.2 第一阶段：在 coroutine lowering 前内联小型已知同步正文

建议顺序：完成 typed MIR 正文与调用点登记 → 选择局部直接 callee → 消除安全的同步 site → 转换 suspend ABI → 现有状态机生成 → 既有普通 MIR 优化。

初版只做同一 Cone、直接目标、正文已知的小函数。无 suspend site 的 source body 是起点；将它内联到 wrapper 后，如果 wrapper 也不再包含 site，就可以继续处理其直接 caller。递归／互递归先保留，复用已有局部代码增长选择，更新受影响 caller 的事实，不在所有函数上反复克隆整份 Module。

这里的事实是“本次调用不挂起且不观察／发布调用者的 hidden completion”，不是“纯函数”或“没有 GC”。普通分配、其他正常调用和副作用可以存在，只要保持原顺序并继续正常生成 safepoint；初版为复用现有 clone 能力可先限制无 EH 的小 CFG。未知外部 suspend 调用、动态派发未闭合、`suspendCoroutine` 等保留原 site；某次运行返回 Completed 不能作为静态证明。

取消仍按语言规范 8.2／11.9.1 由普通 core 库表达，并非每个 suspend 调用的隐式动作；保留实际取消检查与异常传播，不为了优化增加或删除此类调用。

前置内联优于直接复用物理 ABI 的一个实际原因是类型：wrapper 的 `Continuation<T>` 不一定能作为 callee 的 `Continuation<U>`。即使知道 callee 不用 completion，也不能传 null、伪造 interface 或任意重解释类型。先在 source signature 层替换正文，可以让已有无 site 路径工作，无需新增同步 ABI、发布摘要格式或第二个入口体系。

实现时需要复用已有 MIR clone 中的实际能力：实参单次求值、local／block／loop ID 重映射、return 汇合、typed callable 归属。新增入口同时更新 pre-coroutine call-site registry，不能只改 CFG 而留下旧 site。若某个模式必须复制 EH 或 coroutine 协议元数据，初版就保留调用，不为其扩建通用变换层。

### 2.3 第二阶段才考虑尾部转交

纯粹 `return child(args)` 可以另行研究直接传递 completion、原样返回 `CoroutineStep`，即使 child 真正挂起也可能省掉父包装；这是与“静态同步内联”不同的优化。

初版尾部候选应具有相同 exact 结果类型，无调用后值加工、catch／finally、Context 恢复、返回适配或独立完成状态义务。必须验证 source exception、同步 reentrant resume、失败传播、跨线程恢复和唯一完成协议。不能用语法上的最后一行推断这些条件，也不解除 LLVM 的机器 tail-call／tail-duplication 限制。

暂不做可逃逸 continuation 的栈化、全局 adapter 池或 frame 池。这些对象的唯一获胜／一次完成状态与寿命不同于普通局部 box，把它们统一交给一般逃逸 pass 会扩大正确性范围。

LLVM 的 [CoroElide](https://llvm.org/docs/Coroutines.html#coroelide) 面向 `llvm.coro.*` 表示，会在条件成立时把 frame 存储交给 caller 并专化调用。Scoop 自己生成 frame class、adapter、状态字段和普通 GC allocation，没有使用这套 coroutine IR；单独加入该 pass 不会识别上述包装。[Kotlin 协程规范](https://kotlinlang.org/spec/asynchronous-programming-with-coroutines.html#continuation-passing-style)也区分 CPS 调用的立即返回与挂起，但不能替 Scoop 的完成协议提供消除证明。

## 3. 推荐交付顺序与验收

| 顺序 | 最小范围 | 正向与保守保留用例 | 观察指标 |
| --- | --- | --- | --- |
| 1 | 单创建点局部 box 替换 | 别名／多次拆箱／原值重赋值／含引用 payload 跨移动 GC；身份、逃逸、pin、错误转换保留 | 每操作分配次数／bytes、minor 次数、kernel、代码体积 |
| 2 | coroutine lowering 前的小型同步 callee 内联 | 零／一／四层；不同返回类型；带普通 GC 的同步正文；未知／递归／真正挂起保留 | frame／adapter 的静态和动态数量、allocated bytes、吞吐、编译成本 |
| 3 | 同结果类型的简单尾部 suspend 转交 | 正常／异常／同步恢复／跨线程；finally／Context／返回适配反例 | 包装数、恢复次数、异常与 Context 内容、保留路径无回退 |

普通语义 fixture 断言结果和必要诊断，少量优化专测断言目标 Box／frame 分配是否存在及精确 root 合同，不为所有案例保存完整 MIR。原有 coroutine race、moving GC、接口／泛型组合不能被只执行同步成功路径替代。

先补实现规范 2.19 的局部替换与 pass 职责；协程还需明确对应 lowering 的处理顺序。语言规范 8.2／11.9、Context 8.3 与运行时 GC 合同保持原语义；若发现所需模式不能保持这些语义，应缩小优化范围。此次只提出方案，未修改规范中的现行行为或增加 ABI。

减少短命分配首先改善分配热路径和 GC 触发频率；删除 frame 也可能减少存活根／引用槽。**单次 full 暂停是否缩短仍取决于真实 live set**，不能把少做若干次 minor 等同于 full 的 p99 改善。应与 [GC 暂停工作量](GC-PAUSE-WORK.md) 的混合寿命负载一起验收。
