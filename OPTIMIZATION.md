# Optimization 设计（early MIR / ANF + summary-driven inlining / devirtualization 基线）

> 状态：设计草案 / 基线  
> 目标：引入一个最小 early MIR / ANF 作为中端优化承载层，并把结构驱动的 `summary-driven inlining`、通用 `devirtualization`、以及后续 `continuation escaping analysis` 放到这一层统一收口。  
> 边界：本文只定义方向、IR 形状、核心分析与 pass 顺序；本轮不涉及代码实现，也不把优化建立在函数名白名单、热点库函数开洞、或 `inline` 关键字之上。文中提到的 LLVM / statepoint / spill-writeback 等术语，只用于说明当前实现中的工程压力，不构成 early MIR 的语义前提；early MIR 本身必须保持后端无关，能够对接 LLVM、C、JVM bytecode、CLR IR 等不同 codegen 路径。

## 1. 背景

当前的几个性能瓶颈，本质上都指向同一个问题：优化发生得太晚，而且缺少一个适合做“结构驱动”分析的中间层。

典型症状有三类：

1. 小而固定模式的高阶函数，即使 body 很小，如果实参是一个可能 effectful 的函数值，当前也很难在真正需要的时机把它摊平。
   - 这会导致后续仍把它当作“可能间接调用 / 可能 suspend”的边界处理。
   - 一旦 effect / state machine 规划已经做下去，再想把这类调用边界消掉，收益就会大幅缩水。
2. 通过 interface 发起的方法调用，即使 receiver 的精确具体类型已经静态可知，当前也往往要等到很晚才进入运行时分发表达。
   - 这样即便最终可知 target 是 singleton，也已经来不及帮助更高层的 effect / state machine 规划。
3. `mem2reg` 方向当前不是近期主路径。
   - 现有 moving GC 仍依赖 stack-backed roots 的 spill / writeback 合同；在 safepoint 前后，GC ref 是否落在可写回栈槽里，直接关系到 relocate 后的正确性。
   - 因此短期更现实的方向不是“先让 GC root 进寄存器”，而是“先减少必须跨 safepoint 的调用边界”。

这三类问题放在一起看，结论很直接：

- 需要一个比 HIR 更接近执行、但又比 LLVM IR 更早的层；
- 这一层必须显式表达调用形态、值流向、控制流与 effect/control-transfer 边界；
- 优化必须在 closure object lowering、interface runtime dispatch lowering、以及 effect/state-machine 规划之前发生。

## 2. 设计目标

本文方案的核心目标不是“做一组局部特判”，而是建立一条可逐步扩展的通用路径。

### 2.1 通用性优先于热点特判

优化是否触发，不依赖函数名，不依赖 `map` / `filter` / `Iterator.next()` 这类特定 API，也不依赖“stdlib 白名单”。

触发条件只能来自结构事实，例如：

- callee body 可见且足够小；
- 某个函数值参数只在 callee 内被 `DirectCallOnly` 使用；
- 某个 receiver 的精确具体类型可知；
- 某个 interface call 的 target set 被收缩成 singleton；
- 某个 closure / continuation 经分析后不逃逸。

也就是说，初版能力可以保守，但保守的原因必须是“结构信息还不够”，而不是“只对若干名字开洞”。

### 2.2 自动优化由优化级别驱动

自动内联、去虚化等优化应由优化级别控制，而不是由语言 surface 上的 `inline` 关键字控制。

后续若保留 `@Inline`，它也只应是：

- 某些特殊场景下的 override / hint；
- 一个附加约束或调参入口；
- 而不是优化体系的主机制。

换句话说，默认路径应该是：

- `-O0` 以语义清晰和调试友好为主；
- `-O1` / `-O2` / `-O3` 按预算逐步打开这些中端优化；
- `@Inline` 不决定优化模型，只可能在少量场景下影响阈值或强制策略。

### 2.3 early MIR 必须后端无关

early MIR / ANF 的语义合同，必须定义在“语言语义 + 运行时抽象能力”这一层，而不是定义在“当前 LLVM 后端碰巧怎么 lowering”这一层。

这意味着：

- MIR 不能把 LLVM 特有概念编码进自身语义。
  - 例如 `llvm.experimental.gc.statepoint`、`gc.relocate`、LLVM address space、stackmap record 形状、或某种特定的 itable/vtable 内存布局，都不应成为 MIR 操作本身的定义前提。
- MIR 可以表达“抽象事实”，但不应表达“某个 backend 的落地机制”。
  - 例如它可以表达“这是一个 interface call”“这个调用可能分配”“这个值是 GC ref”“这个操作可能 suspend / perform / resume”；
  - 但不应直接表达“这里将来要产出 statepoint”“这里必须 spill 到某类 alloca”“这里依赖某个 LLVM helper ABI”。
- 同一个 early MIR，应能被多个 backend 消费。
  - 当前主要消费方是 LLVM；
  - 未来也可能对接 C、JVM bytecode、CLR IR，甚至更受限的 hosted/adapter 路径。

这一点和仓库里已经存在的 `gc-hosted` / GC capability matrix 方向是一致的：runtime/backend 已经在尝试把“具体实现”收敛到“抽象能力 + 后端适配层”，early MIR 也应该顺着这条线设计，而不是把当前 LLVM lowering 里的细节倒灌进中端语义。

### 2.4 优化必须早于 effect / state-machine 规划

如果一个调用最终能被内联、去虚化，或者被证明不会让 continuation 向外逃逸，那么这些事实必须在 effect/state-machine 规划之前可见。

否则就会出现：

- 先按“可能 suspend / 可能间接调用”做切分；
- 再在更晚阶段把调用边界消掉；
- 结果是语义上已经被抬升成 state machine，优化收益只剩下很小一部分。

### 2.5 先做最小通用层，再持续扩覆盖面

本文不主张一上来就引入大而全的新 IR，也不主张先做完整 SSA 或复杂 speculative optimization。

优先级应是：

1. 先引入一个最小 early MIR / ANF；
2. 先支持最保守、最结构化的 summary-driven inlining 与 devirtualization；
3. 再在同一层持续扩展结构覆盖面；
4. 后续把 continuation escaping analysis 也加到这一层。

## 3. 为什么是 early MIR / ANF

### 3.1 仅靠 HIR 不够

HIR 仍然过于接近语法与前端降糖结果，不适合作为这类优化的稳定承载层，原因主要有三点：

1. 求值顺序与局部绑定不够显式。
   - 对高阶调用、嵌套调用、effect/control-transfer 边界做统一分析时，ANF 化的显式绑定会简单很多。
2. 调用形态容易被前端降糖淹没。
   - 例如成员调用可能已经被改写成顶层调用形式；
   - 若不额外保留 dispatch 分类信息，后续很难统一判断它原本是 direct / virtual / interface / callable-value call。
3. HIR 更适合承载语义保真，不适合承载多轮中端重写。
   - 一旦把大量分析与变换继续压在 HIR 上，前端与中端的职责边界会越来越模糊。

### 3.2 仅靠 LLVM IR 太晚

LLVM IR 上当然也能做部分 inlining 和 devirtualization，但对当前问题来说，时机已经太晚：

1. interface dispatch 到那时通常已经表现为 itable / vtable 级别的低层访问；
2. effectful call、closure call、以及 state-machine planning 的关键信息已经部分固化；
3. 即使 LLVM 最终消掉一个间接调用，也未必还能反过来取消之前更高层的 state machine 变换。

因此，这里要的不是“LLVM 还能不能优化”，而是“哪些优化必须在 LLVM 之前先做完，才能影响更高层 lowering 形态”。

这里提 LLVM，只是因为它是当前主要 backend。更一般地说，任何“已经过于接近底层代码生成细节”的 backend IR，都不适合作为这些优化的主承载层。对未来的 C / JVM / CLR backend 也是同样的判断。

### 3.3 为什么是 ANF 风格

最小 early MIR 采用 ANF 风格有几个直接好处：

- 显式求值顺序；
- 显式局部绑定；
- 显式基本块与分支；
- 显式 call kind；
- 便于做值 provenance、参数 use summary、receiver exactness、以及 continuation escape 之类的分析；
- 不要求一开始就引入完整 SSA。

这意味着 v1 可以先用“ANF locals + block params / 显式 merge”的方式工作，而不是先把整套中端基础设施做成 full SSA。

## 4. 最小 early MIR / ANF 形状

### 4.1 设计要求

最小 early MIR 至少需要满足以下要求：

1. 保留类型信息，尤其是：
   - 函数值类型；
   - receiver 的静态类型；
   - effect row / may-suspend 相关静态信息；
   - continuation / handler 相关类型信息。
2. 显式表示控制流。
   - `if` / `when` / loop / early return 不能继续只靠嵌套表达式隐含表示。
3. 显式表示调用种类。
   - 后续能否去虚化、能否把函数值调用改写成 direct call，首先取决于 call kind 是否可见。
4. 显式表示 effect/control-transfer 操作。
   - `perform` / `resume` 至少要在这一层有独立操作，而不是完全埋在更晚的 lowering 里。
5. 保持 backend-agnostic 语义边界。
   - 不把 LLVM statepoint、`gc.relocate`、address space、stackmap roots、C ABI 细节、JVM/CLR 特定调用指令形式等编码进 MIR；
   - 若需表达运行时交互，只表达后端无关的抽象事实，例如“可能分配”“可能调用受管运行时”“可能触发 safepoint/collector 进入”“值携带 GC ref 语义”等。

### 4.2 一个示意形状

下面是一种足够小、但已经够用的示意，不代表最终语法：

```text
function foo(%p0: T0, %p1: T1) -> R {
block B0:
  %v0 = DirectCall bar(%p0)
  %v1 = InterfaceCall Iterator.next(%p1)
  %v2 = FunValueCall %f(%v1)
  br_if %cond, B1(%v2), B2(%v0)

block B1(%x: U):
  %v3 = Perform Op(%x)
  return %v3

block B2(%y: U):
  return %y
}
```

关键点不是语法，而是这几个事实：

- 每一步计算都有稳定的绑定点；
- 控制流合并点是显式的；
- 调用种类在 IR 上可区分；
- `Perform` / `Resume` 这类 control-transfer 不再只是“某个普通调用的特殊 lowering”。

### 4.3 MIR 与 backend lowering 的边界

early MIR 可以也应该看见“足以驱动优化”的抽象语义，但不应该看见“某个 backend 如何实现这些语义”的细节。

例如：

- MIR 可以知道某个操作 `may_allocate`、`may_suspend`、`may_outward_effect`；
- MIR 可以知道某个值是 GC ref、某个 call 是 `InterfaceCall`、某个 continuation 可能逃逸；
- MIR 也可以知道某个 runtime 交互点是否构成“潜在的 collector/调度边界”。

但 MIR 不应该直接知道：

- 当前 LLVM 后端会不会把这里变成 `statepoint`；
- 具体 roots 是通过 stackmap、native roots、handles 还是 host-managed reference 表达；
- 某个 backend 采用哪种对象头布局、地址空间编号、调用约定或 helper 符号。

这些都应该留给更低层的 backend-specific lowering 去决定。也只有这样，同一份 early MIR 才能稳定对接多个 codegen 后端。

### 4.4 v1 不要求的东西

v1 不要求：

- 完整 SSA；
- 复杂 alias analysis；
- 完整的 region-based effect system；
- speculative guarded devirtualization；
- profile-guided inline heuristics。

只要足够支撑“保守但通用”的结构分析即可。

## 5. Call Kind 设计

这一层最重要的显式信息之一，是把不同调用形态区分出来。

建议最小集合如下：

### 5.1 `DirectCall`

`DirectCall` 表示目标函数已经静态唯一确定。

典型来源：

- 普通顶层函数调用；
- 已经解析为唯一具体成员实现的方法调用；
- 去虚化后的 call；
- higher-order beta-reduction 后被重新显式化的 call。

### 5.2 `VirtualCall`

`VirtualCall` 表示通过 class/object 层级分派的方法调用，dispatch 仍依赖 receiver 的运行时具体类型。

v1 可以保守处理；如果后续证明 receiver exact type 已知且 target singleton，则改写为 `DirectCall`。

### 5.3 `InterfaceCall`

`InterfaceCall` 表示通过 interface 发起的分派调用。

这一类调用应该在 early MIR 层显式存在，而不是直接退化成更晚的 itable 访问细节。否则：

- 无法统一做 target-set shrinking；
- 也很难在更高层把它与后续内联联动起来。

### 5.4 `ClosureCall`

`ClosureCall` 表示调用一个已知是 closure object 的值。

它和一般 `FunValueCall` 的区别在于：closure body / captures / invoke target 通常更容易被本地分析恢复，因此是后续“non-escaping closure elision”的重要抓手。

### 5.5 `FunValueCall`

`FunValueCall` 表示调用一个函数值，但其 provenance 目前还不足以恢复成更具体的形态。

例如：

- 来自参数；
- 来自 join / phi；
- 来自更复杂的返回值传播；
- 或来自仍未解析的 callable wrapper。

后续如果 provenance analysis 收缩了它的候选来源，`FunValueCall` 可以继续被细化成 `DirectCall` 或 `ClosureCall`。

### 5.6 `Perform`

`Perform` 是 effect operation 的显式控制转移节点，不应与普通调用混在一起。

这样做的原因是：

- 它是否会向外传播、是否需要切 state machine，并不只取决于“这是个 call”；
- 它与 handler / continuation 的关系需要更高层语义，而不是更低层 ABI 细节。

### 5.7 `Resume`

`Resume` 表示 continuation 恢复操作，也应保留为显式控制转移节点。

这为后续 continuation escaping analysis 提供了稳定落点。

## 6. Summary 与核心分析

这个方案的核心，不是“对某些语法模式直接硬编码内联”，而是先建立一组可组合的 summary / analysis，再让优化 pass 按这些 summary 做改写。

### 6.1 Callee Summary

每个函数至少应维护一个保守 summary，v1 可以包含：

- `body_known`
  - 当前编译单元内 body 是否可见。
- `size_cost`
  - 一个简单的 body 大小/成本估计。
- `recursive_scc`
  - 是否处于递归 SCC 中。
- `may_outward_effect`
  - 运行时是否可能向当前边界外传播 effect/control-transfer。
- `may_allocate_closure`
  - 是否构造 closure 或其它对后续内联有显著影响的对象。
- `param_use_summaries`
  - 每个参数的使用摘要，尤其是函数值参数。
- `result_provenance`
  - 返回值是否只是某个已知值/函数值的转发。

这不是最终完备集合，但已经足够驱动第一版保守优化。

### 6.2 函数值参数使用摘要

对函数值参数，建议先做一个非常保守但可扩展的分类。一个可行的 v1 四态近似是：

- `Unused`
  - 参数未被使用。
- `ValueOnly`
  - 参数只被当作值搬运，不在当前函数体内直接调用，也没有发生明显逃逸。
- `DirectCallOnly`
  - 参数只出现在若干直接调用位点上，不被存储、不被返回、不被捕获、不被传给未知调用。
- `Escapes`
  - 只要出现存储、返回、捕获、传给未知 callee、放入 closure/environment、跨 `perform` / `resume` 边界传播等情况，就提升到这一态。

这个四态并不追求理论上最优雅的 lattice；v1 可以采取非常保守的 join 规则：

- 只要同时出现多种难以精确组合的用法，就直接上升到 `Escapes`；
- 先把“能稳定识别 `DirectCallOnly`”作为首要目标。

只要这个分类是结构驱动的，它就已经比“针对几个库函数名字开洞”更有延展性。

### 6.3 函数值 Provenance

除了参数使用摘要，还需要跟踪“一个函数值从哪里来”。

v1 可以先做保守分类：

- `DirectFunction(fqn)`
- `KnownClosure(lambda_id)`
- `Param(i)`
- `Join(set)`
- `Unknown`

这个 provenance 结果主要服务于两类改写：

1. 把 `FunValueCall` 收缩成更具体的调用种类；
2. 在高阶函数内联后，把原来“对参数的调用”直接重写为具体目标调用。

### 6.4 Receiver Exactness / Target-Set Analysis

对方法调用，需要一个与函数值 provenance 类似的分析，但对象是 receiver。

关心的不是“这个方法名是不是热点”，而是：

- receiver 的精确具体类型是否静态可知；
- 对这个 dispatch site 来说，候选 target set 是否已经缩成 singleton。

可利用的信息包括：

- 新构造对象；
- smart cast 之后的具体类型；
- 精确 class/object 类型局部值；
- final / sealed 等语义信息；
- 已知的 itable / vtable 静态元数据。

一旦 `InterfaceCall` 或 `VirtualCall` 的 target set 收缩为 singleton，就应统一改写为 `DirectCall`。

### 6.5 Continuation Escaping Analysis

这一层之所以值得单独引入，不只是为了 inline / devirt，也是为了给 continuation 相关分析提供稳定落点。

最小 continuation escaping analysis 可以回答：

- 某个 continuation/closure 是否被存储、返回、跨边界传播；
- 某个 effectful 区段是否真的需要把 continuation 物化成可逃逸对象；
- 某个 handler/resume 路径是否只在局部结构内闭合。

这类信息在 HIR 上太语法化，在 LLVM 上又太晚；放在 early MIR / ANF 上最合适。

## 7. Summary-Driven Inlining

### 7.1 目标

这里的内联不是“给某些高阶库函数开 special case”，而是：

- 对所有 body-known、非递归、成本可接受的函数统一适用；
- 对所有满足结构条件的高阶调用统一适用；
- 优化效果首先体现在“消掉高层调用边界”，从而影响后续 effect/state-machine 规划。

### 7.2 基本触发条件

第一版可以非常保守，只在以下条件同时满足时考虑内联：

- callee `body_known = true`；
- callee 不在递归 SCC 中；
- `size_cost` 低于阈值；
- 当前优化级别允许；
- 对高阶场景，相关函数值参数使用摘要为 `DirectCallOnly`；
- 实参 provenance 可恢复成 `DirectFunction` 或 `KnownClosure`，或至少可收缩到足够小的候选集。

### 7.3 高阶场景的工作方式

考虑一个普通的小函数：

```text
fun twice(f, x) {
  val y = f(x)
  return f(y)
}
```

如果它的 summary 证明：

- `f` 的参数使用是 `DirectCallOnly`；
- `twice` 本身 body 很小；

那么当调用点传入一个 provenance 已知的函数值时：

1. 先把 `twice` 内联到调用点；
2. 再把原来对参数 `f` 的调用重写成具体 `DirectCall` 或更具体的 `ClosureCall`；
3. 若具体目标本身也足够小，再继续按预算内联。

这个流程对 `map` / `filter` / `forEach` / 任意用户自定义小包装函数都成立，只要它们呈现出同样的结构。

也就是说，早期受益最大的往往会是“高阶库函数”，但那只是因为它们经常满足这些结构条件，而不是因为编译器知道它们叫这个名字。

### 7.4 为什么它能帮助 effect/state-machine 规划

如果被内联的高阶函数体里原本有：

- 对函数值参数的调用；
- 一层薄包装的 effectful call；
- 局部 closure / continuation 只在当前结构内使用；

那么在内联后：

- 间接调用可能变成 direct call；
- 原本分离的两个 effect 边界可能合并；
- 原本必须保守切开的 state-machine segment 可能不再需要切开。

因此，inline 的收益不只是在 LLVM 级别少一个 call 指令，而是在更高层避免不必要的 state machine 变换。

## 8. 通用 Devirtualization

### 8.1 目标

去虚化的目标也必须是通用的，而不是：

- “专门优化 `Iterator.next()` / `Iterator.hasNext()`”
- 或“专门给某几个 interface method 加快路径”。

真正的规则应该是：

- 对所有 `VirtualCall` / `InterfaceCall` 统一做 target-set analysis；
- 只要 receiver 精确具体类型可知，且该调用位点的 target set 为 singleton，就改写为 `DirectCall`。

### 8.2 `InterfaceCall` 的规则

对 `InterfaceCall`，最关键的不是方法名，而是：

1. dispatch site 的 interface method slot 已知；
2. receiver 的 exact concrete class 已知；
3. 根据 `class × interface × slot -> impl` 的静态信息，候选实现已缩成唯一目标。

满足这三点时，就应把：

```text
InterfaceCall I.m(receiver, args...)
```

改写成：

```text
DirectCall ConcreteType.m(receiver, args...)
```

这一规则对所有 interface method 都成立，而不是只对 `Iterator` 成立。

### 8.3 `VirtualCall` 的规则

对 class/object 层级分派，规则同样是结构性的：

- receiver exact type 已知；
- 或当前层级规则能证明 override target 唯一；
- 则 `VirtualCall` 改写成 `DirectCall`。

第一版不需要做复杂 speculative devirt；静态 singleton target 就已经足够带来可见收益。

### 8.4 为什么 devirtualization 也应放在这一层

原因和内联完全一致：

- 如果去虚化发生在 LLVM 级别，通常已经来不及影响 effect/state-machine 规划；
- 如果去虚化发生在 early MIR 层，它可以立刻喂给后续的 summary-driven inlining；
- 两者联动后，`InterfaceCall -> DirectCall -> Inline` 这一链条才真正成立。

## 9. 与 Effect / State-Machine Planning 的关系

这套设计的关键不是“加一个中端 IR”本身，而是要把它放在正确的位置上。

建议顺序如下：

1. `HIR -> early MIR / ANF`
2. canonicalization
   - 展平嵌套表达式；
   - 恢复显式 call kind；
   - 建立基本块与局部绑定。
3. 初始 summaries
   - `size_cost`
   - `may_outward_effect`
   - 参数使用摘要
   - provenance 初值
4. receiver exactness / target-set analysis
5. devirtualization
6. summary-driven inlining
7. higher-order beta-reduction / `FunValueCall` 细化
8. non-escaping closure simplification
9. continuation escaping analysis
10. 重新计算 summaries，并按预算做一到两轮迭代
11. effect / state-machine planning
12. 再进入更低层 lowering 与 LLVM codegen

这个顺序里最重要的约束是：

- effect/state-machine planning 必须发生在这些中端收缩之后；
- 否则 inline / devirt 的大部分价值都拿不到。

## 10. 优化级别与 `@Inline`

### 10.1 优化级别建议

一个可行的起点是：

- `-O0`
  - 构建 early MIR / ANF，但只做必要 canonicalization；
  - 不做跨函数 inlining；
  - 不做激进去虚化；
  - 优先保证调试与诊断可读性。
- `-O1`
  - 开启保守 summaries；
  - 开启静态 singleton target 的 devirtualization；
  - 开启小函数、非递归、body-known 的 summary-driven inlining。
- `-O2`
  - 开启高阶场景的 `DirectCallOnly` 参数内联；
  - 开启一到两轮 summary / devirt / inline 迭代；
  - 开启 non-escaping closure simplification 与基础 continuation escape 分析。
- `-O3`
  - 仍然是同一套机制，只放宽预算和阈值；
  - 不是引入一批“只有 O3 才识别某些名字”的新特判。

### 10.2 `@Inline` 的位置

如果未来保留 `@Inline`，建议它的语义保持极窄：

- 作为一个 override / hint；
- 只影响阈值或强制策略；
- 不改变语言语义；
- 也不构成优化体系的基础假设。

更直接地说：

- 没有 `@Inline`，优化也应该按结构自动发生；
- 有 `@Inline`，只是少量特殊场景下帮助编译器越过默认预算。

## 11. Mem2reg 与 Safepoint 方向

### 11.1 近期不把 `mem2reg` 作为主路径

在当前实现现实下，`mem2reg` 不是这轮设计的主目标。

原因不是它永远不值得做，而是当前 LLVM + moving GC 路径里，它和 roots / safepoint 合同直接耦合：

- 现有 moving GC 依赖 stack-backed local roots；
- ordinary safepoint 前后存在显式 spill / relocate / writeback 合同；
- 如果贸然把 GC roots 提升为寄存器值，而没有新的准确 root 表达与 relocate 机制，正确性就会先出问题。

因此，v1 的重点应是：

- 先减少 safepoint 数量；
- 先减少必须跨 safepoint 存活的函数值、receiver、closure、continuation；
- 先减少“本可被 inline/devirt 消掉，但目前仍残留”的调用边界。

### 11.2 early MIR 对这条线的帮助

虽然 v1 不直接做 `mem2reg`，但 early MIR 仍然能为这条线打基础。

需要强调的是：这里讨论的是“当前 LLVM backend 下的近期工程优先级”，而不是要把这些约束上升成 MIR 本身的语义定义。对未来的 C / JVM / CLR / hosted backend 来说，roots、collector entry、safepoint 的落地机制都可能不同；early MIR 只需要保留足够的抽象语义，让各 backend 在自己的 lowering 阶段完成映射。

在这个前提下，early MIR 的帮助主要体现在：
- 它让“哪些操作会形成潜在 collector / 调度边界”更早可见；
- 它让“哪些调用本可被消掉”更早可见；
- 它让后续若要研究更精细的 root liveness / safepoint sinking / backend-specific barrier placement，有一个比 HIR 更稳定的分析层。

换句话说，近期路线是：

- 先通过 inline / devirt / closure simplification 降低 safepoint 压力；
- 再视 GC root 合同演进情况，决定是否继续推进更激进的 `mem2reg` / register-root 研究。

## 12. 分阶段落地建议

### 12.1 第一阶段：最小 early MIR / ANF

只做最小承载层，不追求一开始就很强：

- 显式基本块；
- 显式局部绑定；
- 显式 call kinds；
- 显式 `Perform` / `Resume`；
- 保留足够的类型与 dispatch 元信息。

### 12.2 第二阶段：summary 基础设施

先做最保守的跨函数摘要：

- `body_known`
- `size_cost`
- `recursive_scc`
- `may_outward_effect`
- 函数值参数使用摘要
- 基础 provenance

### 12.3 第三阶段：通用 devirtualization

先统一处理所有 `VirtualCall` / `InterfaceCall`：

- 只要 target set 静态为 singleton，就改写为 `DirectCall`；
- 先不做 speculative guard；
- 先不按名字区分热点。

### 12.4 第四阶段：summary-driven inlining

先支持：

- body-known；
- 非递归；
- 小体量；
- `DirectCallOnly` 参数；
- 实参 provenance 可知；
- 非逃逸 closure / 函数值的最保守重写。

这一阶段即使能力有限，也已经是通用方案，而不是特判方案。

### 12.5 第五阶段：continuation / closure 逃逸分析

在同一层继续扩展：

- non-escaping closure elision；
- continuation escaping analysis；
- 对 effect/state-machine 规划提供更细粒度输入。

### 12.6 第六阶段：迭代扩展覆盖面

后续扩展方向应该是：

- 扩展结构识别能力；
- 改善 summaries 的精度；
- 改善 provenance / target-set shrinking；
- 引入更成熟的 budget / profitability 模型；

而不是继续累积“又支持了几个特殊函数名”。

## 13. 非目标

本文明确不把以下方向作为 v1 目标：

- 不做基于函数名或 stdlib API 名字的白名单内联；
- 不把 `inline` 关键字当作主机制；
- 不优先做 `mem2reg` / register-root 改造；
- 不要求一开始就引入完整 SSA；
- 不要求一开始就支持 speculative guarded devirtualization；
- 不要求一开始就做完整全程序优化。

## 14. 总结

这份设计的核心，不是“先把 `map` / `filter` / `Iterator.next()` 优化掉”，而是先建立一个足够小、但语义位置正确的中端层：

- 它能显式表达调用形态；
- 它能承载 summary-driven inlining；
- 它能对所有 receiver exact 的 interface/class 调用统一做 devirtualization；
- 它能为 continuation escaping analysis 提供稳定落点；
- 它还能在当前 GC 合同不变的前提下，通过减少调用边界和 safepoint，为后续性能优化创造空间。

因此，第一步不是扩更多特判，而是先把 early MIR / ANF 这一层立起来。只要这一层存在，后面的优化能力就可以沿着“结构覆盖面”持续扩张，而不是沿着“函数名白名单”持续堆积。
