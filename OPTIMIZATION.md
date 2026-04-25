# Optimization 设计（early MIR / ANF + summary-driven inlining / devirtualization 基线）

> 状态：设计草案 / 基线  
> 目标：引入一个最小 early MIR / ANF 作为中端优化承载层，并把结构驱动的 `summary-driven inlining`、通用 `devirtualization`、以及后续 `continuation escaping analysis` 放到这一层统一收口。  
> 边界：本文只定义方向、IR 形状、核心分析与 pass 顺序，并补充当前代码库里 LLVM codegen 的职责错位、可拆模块边界、以及应上移到 early MIR / ANF 的内容；本轮不涉及代码实现，也不把优化建立在函数名白名单、热点库函数开洞、或 `inline` 关键字之上。文中提到的 LLVM / statepoint / spill-writeback 等术语，只用于说明当前实现中的工程压力，不构成 early MIR 的语义前提；early MIR 本身必须保持后端无关，能够对接 LLVM、C、JVM bytecode、CLR IR 等不同 codegen 路径。

## 0. 当前基线与 Guardrail（2026-04-25）

本节是 `T5000a` 的统一基线入口。后续 `T5000b+` 若需要回答“当前 LLVM codegen 的主要边界错位是什么”“`-O0` / debug build 的固定成本热点在哪里”“哪些重复工作必须优先迁出”，都应优先引用本节，再按需展开到第 10、11 节。

### 0.1 结论速览

当前仓库已经足够明确地暴露出四类基线事实：

1. LLVM codegen 的主要问题不是单点 bug，而是结构边界错位。
   - `codegen/mod.rs` 继续同时承担调用分发、builtin/sysroot lowering、closure lowering、class/object/enum lowering、GC lowering、runtime ABI glue、以及部分“实际上更像中端分析”的逻辑。
   - effect 相关 `state_machine_plan / segments / transform / emitter` 已经形成一个事实上的 middle-end 簇，而不只是“LLVM emitter 的前置准备”。
2. `MainCodegen` 不是单纯的函数级 emitter。
   - 它同时混合 module 级只读输入、function builder 状态、layout/type caches、effect planning 查询缓存、GC slot 状态与 effect emitter 上下文。
3. `-O0` / debug build 路径并不轻。
   - 当前即使在 `-O0`，LLVM pipeline 仍固定执行 `function(sroa),rewrite-statepoints-for-gc`；
   - `run_pass_pipeline` 还固定启用 `verify_each(true)`；
   - `debug_assertions` 下 effect middle-end 还会做额外 contract 校验与 round-trip 验证。
4. reachability、callee 解析和 effect/suspendability 查询仍有明显重复工作。
   - HIR reachability 之后仍需要因为 codegen 期目标物化而做 eager inclusion；
   - call target / monomorphized variant 解析仍发生在 codegen 调用路径；
   - higher-order outward-effect / suspendability summary 仍按 `MainCodegen` 实例临时拼装查询上下文。

换句话说，后续顺序必须是：

- 先把 LLVM backend 的边界收口；
- 再把不该留在 backend 的 program facts / effect analysis / monomorphization 语义迁出去；
- 然后再让 early MIR / ANF 接手后续优化。

### 0.2 结构体量基线

以下是 2026-04-25 直接对 `crates/scoopc/src/llvm/**/*.rs` 做行数统计得到的当前主热点：

- `crates/scoopc/src/llvm/codegen/mod.rs`
  - 17759 行；当前最大的职责聚合点。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - 10322 行；effect middle-end 的核心规划器。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - 5923 行；负责 LLVM emitter 落地。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
  - 5085 行；负责 plan 到 segments 的投影与约束。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
  - 4988 行；负责 canonical machine transform。
- `crates/scoopc/src/llvm/mod.rs`
  - 3835 行；同时承载 emit API、module build pipeline、reachability、pass pipeline 和测试。

几个直接 guardrail：

- effect 相关主簇（`effect/mod.rs`、`state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs`、`state_machine_emitter.rs`）总量已经接近 `crates/scoopc/src/llvm` 下全部 Rust 代码的一半，因此后续不能再把新的中端逻辑继续塞回这个簇里。
- `codegen/mod.rs` 已经大到不能继续作为“默认新增 lowering 入口”；新职责要么拆成独立 backend 模块，要么迁出 backend。
- `llvm/mod.rs` 不能继续同时承担 emit API、pipeline、reachability 与测试汇总；否则后续 `ProgramFacts` / MIR 接口会继续被顶层模块耦死。

### 0.3 `MainCodegen::new` 构造点基线

当前 `MainCodegen::new` 至少在下列路径显式重复构造：

- `crates/scoopc/src/llvm/mod.rs`
  - 用于顶层声明阶段；
  - 用于每个 reachable top-level function body 的发射；
  - 用于入口 `main` 的 exit-code lowering。
- `crates/scoopc/src/llvm/codegen/mod.rs`
  - 用于 effect-call wrapper body；
  - 用于 top-level immutable value init；
  - 用于 closure body lowering；
  - 用于 object init lowering。

这不是普通的“构造几个轻量 helper”：

- `MainCodegen::new` 会重新收集 `known_effect_instances_by_effect_fqn`；
- 新实例会重新初始化 `known_fun_call_suspend_cache`、type/layout caches、局部 env 与 effect 相关状态；
- 因为这些缓存挂在 `MainCodegen` 实例上，closure / wrapper / init function 等路径很难共享 program facts 与分析结果。

因此，后续 refactoring 的最低要求不是“减少一点样板代码”，而是：

- 把 module 级只读输入、shared caches、function emitter 状态拆开；
- 让“可跨函数复用的 facts / summary / layout cache”不再随着 `MainCodegen::new` 一起反复重建；
- 新增代码不得继续默认引入新的 `MainCodegen::new` 调用点，除非能明确说明为什么共享上下文做不到。

### 0.4 `-O0` / debug build 固定成本基线

当前 `-O0` / debug build 的固定成本至少包括：

1. LLVM pass pipeline 不是空的。
   - `llvm_pass_pipeline_for_opt_level(OptLevel::O0)` 仍固定返回 `function(sroa),rewrite-statepoints-for-gc`。
   - 这意味着 `-O0` 路径已经天然带着 GC/statepoint 所需的 canonicalization 与 rewrite 成本。
2. `run_pass_pipeline` 固定启用 `verify_each(true)`，而且 passes 完成后还会再做一次 `module.verify()`。
   - 这让 `-O0` 路径仍承担显著的 IR 校验成本。
3. effect middle-end 的 debug 断言不是零成本。
   - `build_unified_lowering_contract` 在 `debug_assertions` 下会执行 segment builder contract 验证；
   - 同时还会做 plan/segments round-trip 的结构签名比对。

这组事实给后续阶段的明确约束是：

- 新的 interprocedural summary / devirt / inline / escape analysis 不能默认塞进 `-O0`；
- 若某个分析只服务 `-O1+`，其构建成本也不应在 `-O0` 先支付一遍；
- 调试期断言应与默认编译路径显式分层，而不是继续沿着 codegen 查询点无限叠加。

### 0.5 Reachability、callee 解析与查询重复工作基线

当前存在三类会直接拖累编译器固定成本、并持续模糊 backend 边界的重复工作：

1. reachability 之后仍要补扫 eager inclusion。
   - `collect_reachable_top_level_funs` 先做一轮 BFS；
   - 随后 `llvm/mod.rs` 还会因为 operator overload 目标在 codegen 期才物化，而额外扫描 `fun_index`，把 struct member methods 补进 reachable 集；
   - generic class member methods 的单态化变体也会再次通过 `fun_index` 全表扫描补入。
2. monomorphized callee resolution 仍发生在 codegen 调用路径。
   - `codegen_top_level_fun_call` 会在真正发射调用前，通过 `try_resolve_monomorphized_member_fqn` / `try_resolve_monomorphized_standalone_fun_fqn` 现场推断目标；
   - 这说明实例身份仍被 mangled FQN 和 codegen 查询逻辑共同承担，而不是由一个独立的 monomorphic instance 层承担。
3. higher-order effect / suspendability 查询仍按 `MainCodegen` 实例临时组装。
   - `ensure_known_fun_body_may_outward_effect_cache` 会收集多组 `HashMap` / `HashSet` 构成 `SuspendCallProgramFacts`；
   - `HandlePlanContext::from_codegen(self)` 说明 effect/state-machine planning 仍直接依赖 LLVM codegen 上下文；
   - `build_unified_lowering_contract` 仍是 codegen 查询路径上的即时分析，而不是稳定的 backend-agnostic 产物。
4. 已有 backend 外消费者仍通过源码级复用反向依赖 LLVM effect planner。
   - `crates/scoopc/src/effect_step_summary.rs` 直接 `include!("llvm/codegen/effect/state_machine_plan.rs")`，以复用 `compute_escape_continuation_direct_step_effect_rows_for_handle_in_program`；
   - 这说明 resumed-step / direct-step effect summary 已经不是 LLVM emitter 私有逻辑，只是当前仍靠“共享同一份源文件”维持语义一致；
   - 因此后续抽离不能靠复制一份近似实现解决，而必须把共享分析与 LLVM emitter 合同正式拆层。

这部分的 guardrail 很直接：

- reachability 与 eager inclusion 不能继续依赖“backend 才知道真正 callee”这一前提；
- monomorphization 不能继续以 mangled symbol name + codegen 猜目标作为主路径；
- effect/state-machine planning 所需事实必须变成稳定 side tables / `ProgramFacts`，而不是从 `MainCodegen` 现场回捞。
- 非 LLVM feature 的 effect summary 消费者不能继续通过 `include!` backend 源文件来共享语义；共享分析需要有独立、后端无关的归属层。

### 0.6 `T5000aR` Review 结论

`T5000aR` 的 review 结论是：当前 baseline 已经足够支撑“先 codegen refactoring，再 early MIR”的顺序，不需要在 `T5000b` 之前额外插入新的前置任务。

理由有三点：

1. 四类关键热点都已经被同一份 baseline 覆盖。
   - `MainCodegen` 的职责混放与重复构造点已在第 0.2、0.3 节定位；
   - effect middle-end 的体量与 `HandlePlanContext::from_codegen` 依赖方向已在第 0.2、0.5 节定位；
   - reachability / eager inclusion / monomorphized callee resolution 的重复工作已在第 0.5 节定位；
   - `-O0` / debug build 固定成本已在第 0.4 节定位。
2. `effect_step_summary.rs` 的 `include!` 复用暴露的是同一类边界泄漏，而不是新的独立前置缺陷。
   - 它进一步证明 effect summary 已经有 backend 外消费者；
   - 但它并没有改变本轮顺序判断：仍应先收口 `llvm/codegen` 边界，再抽离 shared facts / effect analysis。
3. 当前还没有发现比 `T5000b` 更靠前、且不先解决就会阻塞后续顺序判断的结构性热点。
   - 换言之，下一步最有价值的工作仍然是拆 `MainCodegen` 与巨型模块，而不是再回到 baseline 调查层继续加一轮盘点。

因此，`T5000b` / `T5000c` 的直接 guardrail 应补充为：

- `T5000b` 要先把 `MainCodegen`、effect emitter 与 module/pipeline 边界拉直；
- `T5000c` 要把 `effect_step_summary.rs` 这类 backend 外消费者真正接到独立 shared facts / effect analysis 层，而不是继续靠 `include!` 共享 `state_machine_plan.rs`。

### 0.7 后续任务的最小验收护栏

从 `T5000b` 开始，每个后续任务都至少要满足下面这些护栏之一，否则说明它没有真正改善当前 baseline：

- 没有新增“必须挂在 `MainCodegen` 上才能工作”的中端分析入口。
- 没有新增 `MainCodegen::new` 重复构造点，或者显式减少了既有构造点的共享状态丢失。
- 没有把新的默认固定成本塞进 `-O0` / debug build。
- 没有继续依赖 eager inclusion、mangled FQN 重定向或 backend 侧临时分析来恢复本该更早可知的语义事实。
- 让下一阶段能够以 backend-agnostic 的事实层为输入，而不是继续从 LLVM builder / module / runtime ABI 现场取数。

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
- 已实例化后的 generic 调用；
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

### 5.8 Monomorphization 的位置

`monomorphization` 不应放在 LLVM codegen。

更合适的位置是：

- `typed HIR -> generic early MIR / ANF template`
- `generic early MIR / ANF template -> monomorphic MIR instance`
- `monomorphic MIR instance -> summaries / devirtualization / inlining / continuation escaping / effect planning`
- `最后再进入 LLVM / C / JVM / CLR backend`

也就是说，它应成为 **early MIR 内部的一道边界**，而不是 backend 的现场修补逻辑。

更准确地说，这里最好区分两个层次：

- `generic MIR template`
  - 从 typed HIR 降下来的模板 body；
  - 仍允许存在 type params；
  - 主要用于承载后续实例化所需的结构信息。
- `monomorphic MIR instance`
  - 用具体 type arguments 替换后的实例 body；
  - 从这里开始，后续优化看到的应该是具体 receiver type、具体 callee、具体 nominal specialization。

之所以要把它放在这里，而不是放在 LLVM codegen，有几个直接原因：

1. 它是 backend-agnostic 的语义工作。
   - 决定“某个 generic item 在给定 type arguments 下对应哪个实例”，不是 LLVM 特有问题。
2. 后续 devirtualization 和 inlining 依赖它。
   - 很多 receiver exactness、callee summary、call target shrinking 都需要建立在“实例已明确”之上。
3. 如果拖到 codegen，backend 就会被迫做中端决策。
   - 这会退化成“codegen 现场根据 mangled FQN 重定向目标”的模式。
4. 如果完全放在 HIR 上，又太早。
   - HIR 仍然过于接近语法和前端降糖结果，不适合作为后续多轮中端重写的稳定承载层。

因此，本方案里更推荐的表示不是“把实例身份直接编码成最终符号名字符串”，而是维护一个 backend-agnostic 的 `InstanceKey`。一个最小可行形状可以是：

- base item
- owner / nominal specialization（若有）
- type arguments
- 必要时的 receiver / method specialization 维度

随后：

- summaries 应按 `InstanceKey` 挂载；
- call graph / reachability 也应按 `InstanceKey` 维护；
- backend 再把 `InstanceKey` 映射成 LLVM 符号名、C 符号名、JVM method 形状或 CLR 元数据形状。

在实例化策略上，v1 也不应做成“全量 eager monomorphization”，而应采用：

- reachable-driven
- on-demand
- cached by `InstanceKey`

这样既能保持结构正确，也不会在 `-O0` / debug build 下先把编译器自身成本拉爆。

## 6. Summary 与核心分析

这个方案的核心，不是“对某些语法模式直接硬编码内联”，而是先建立一组可组合的 summary / analysis，再让优化 pass 按这些 summary 做改写。

### 6.1 Callee Summary

每个**可优化的单态实例**至少应维护一个保守 summary，而不是只按语法层面的函数名维护一份 summary。v1 可以包含：

- `instance_key`
  - 对应的 monomorphic instance 身份。

- `body_known`
  - 当前编译单元内该实例 body 是否可见。
- `size_cost`
  - 一个简单的 body 大小/成本估计。
- `recursive_scc`
  - 该实例是否处于递归 SCC 中。
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

1. `HIR -> generic early MIR / ANF template`
2. canonicalization
   - 展平嵌套表达式；
   - 恢复显式 call kind；
   - 建立基本块与局部绑定。
3. instance collection / monomorphization
   - 以 reachable-driven、on-demand、可缓存的方式收集 `InstanceKey`；
   - 从 generic template 生成 monomorphic MIR instance；
   - backend 尚未介入符号名或 ABI 细节。
4. 初始 summaries
   - `size_cost`
   - `may_outward_effect`
   - 参数使用摘要
   - provenance 初值
5. receiver exactness / target-set analysis
6. devirtualization
7. summary-driven inlining
8. higher-order beta-reduction / `FunValueCall` 细化
9. non-escaping closure simplification
10. continuation escaping analysis
11. 重新计算 summaries，并按预算做一到两轮迭代
12. effect / state-machine planning
13. 再进入更低层 lowering 与 LLVM codegen

这个顺序里最重要的约束是：

- devirt / inline / effect planning 消费的应是 **monomorphic MIR instances**，而不是仍带 type params 的 generic template；
- effect/state-machine planning 必须发生在这些中端收缩之后；
- 否则 inline / devirt 的大部分价值都拿不到。

## 10. 当前代码库调查结论

这一节把设计落回当前仓库，回答两个问题：

- 现有 LLVM codegen 里，哪些东西只是“文件太大、职责太杂”，但本质上仍属于 backend；
- 哪些东西已经越过 backend 边界，应该上移到 early MIR / ANF 或独立的中端分析层。

### 10.1 当前热点与边界错位

当前最重的几个文件大致如下：

- `crates/scoopc/src/llvm/codegen/mod.rs`
  - 约 17759 行，是当前最主要的巨型职责聚合点。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - 约 10322 行，已经不只是“LLVM emitter 前的准备”，而是一个事实上的 effect middle-end。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - 约 5923 行。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
  - 约 5085 行。
- `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
  - 约 4988 行。
- `crates/scoopc/src/llvm/codegen/control_flow.rs`
  - 约 2488 行。
- `crates/scoopc/src/llvm/codegen/gc.rs`
  - 约 1984 行。
- `crates/scoopc/src/llvm/codegen/runtime_abi.rs`
  - 约 1567 行。
- `crates/scoopc/src/llvm/mod.rs`
  - 约 3835 行。

需要注意两点：

- 一部分行数来自内联测试，尤其是 `llvm/mod.rs` 和 `effect/state_machine_*` 几个文件；
- 但即便扣掉测试，生产代码本体也仍然显示出明显的职责错位，尤其是 `codegen/mod.rs` 与 `state_machine_plan.rs`。

换句话说，这不是单纯“测试太多导致文件看起来大”，而是：

- 一部分逻辑确实应该继续拆成更小的 LLVM codegen 模块；
- 另一部分逻辑已经不该继续停留在 LLVM backend。

### 10.2 仍属于 LLVM backend，但应先做工程性拆分的部分

下面这些内容本质上仍然是 backend-specific lowering，只是当前边界过差，应该先整理模块化形状。

1. 调用 lowering 主簇
   - 当前 `codegen_call`、`codegen_top_level_fun_call`、vtable/itable lowering、callable-value call、extern/native call、实参绑定、ABI 细节都挤在 `codegen/mod.rs`。
   - 建议拆成 `call/` 目录，例如：
     - `call/direct.rs`
     - `call/virtual.rs`
     - `call/interface.rs`
     - `call/callable.rs`
     - `call/extern_native.rs`
     - `call/args.rs`
     - `call/abi.rs`
2. builtin / sysroot lowering
   - 当前 `String` builtin、`print`、`io`、`env`、`time`、`fs`、`process`、`path`、`sync`、`thread`、`channels`、`array`、`atomic` 都堆在 `codegen/mod.rs`。
   - 这部分适合拆成 `intrinsics/` 或 `sysroot_lowering/` 目录，按 domain 分文件。
3. closure lowering
   - closure object 布局、capture env、lambda body codegen、resume thunk 等职责已经形成独立簇。
   - 适合拆成 `closure/` 目录，例如 `closure/object.rs`、`closure/env_layout.rs`、`closure/body.rs`。
4. class / object / enum lowering
   - class ctor、object init、enum value lowering 三类逻辑已经足够独立。
   - 适合拆成 `class_ctor.rs`、`object_init.rs`、`enum_lowering.rs`。
5. GC lowering
   - `gc.rs` 里现在混了 debug intrinsics、statepoint/root spill、type descriptor/vtable/itable global 构造、write barrier/store exact。
   - 适合继续拆成 `gc/statepoints.rs`、`gc/type_desc.rs`、`gc/write_barrier.rs`、`gc/debug_intrinsics.rs`。
6. runtime ABI glue
   - `runtime_abi.rs` 里同时声明了 string / fs / sync / thread / gc / effect / continuation 等多种 runtime 符号。
   - 适合按 ABI 领域拆分，例如 `runtime_abi/gc.rs`、`runtime_abi/effect.rs`、`runtime_abi/continuation.rs`、`runtime_abi/string.rs`。
7. 控制流 lowering
   - `control_flow.rs` 里 `block`、`if`、`when`、pattern binding / condition 也是一个已经成型的子系统。
   - 即便暂时不迁出到 MIR，也适合先拆成 `control_flow/block.rs`、`control_flow/if.rs`、`control_flow/when.rs`、`control_flow/pattern.rs`。
8. `llvm/mod.rs`
   - 当前把 emit API、module build pipeline、reachability、pass pipeline、以及大量测试混在一起。
   - 适合拆成 `llvm/emit_api.rs`、`llvm/pipeline.rs`、`llvm/reachability.rs`，测试移到更独立的位置。

这一层拆分的目标不是“把大文件拆成更多大文件”，而是先把真正属于 LLVM lowering 的职责从主文件中清出来，避免后续 MIR/ANF 迁移继续受到 `codegen/mod.rs` 的组织方式拖累。

### 10.3 不应继续留在 LLVM 的部分

下面这些内容已经不是“如何发射 LLVM IR”的问题，而是“如何理解和重写语言级中端结构”的问题，应上移到 early MIR / ANF 或独立的 backend-agnostic 中端层。

1. 调用目标解析与重定向
   - generic member / standalone function 的 monomorphized callee 解析，本质上既是 call target resolution，也是 instance materialization 的一部分。
   - 这应成为 MIR/ANF 上 `ResolvedCallee` / `CallKind` / `InstanceKey` 的一部分，而不是 codegen 现场再推断，更不应依赖 mangled FQN 字符串做核心语义判定。
2. 具体类型恢复
   - `resolve_expr_concrete_type`、`resolve_member_access_concrete_type`、`resolve_call_result_type` 这类逻辑，本质上是在弥补前中端没有把 value provenance 和 concrete type 记录下来。
   - 这说明 MIR/ANF 节点需要显式保存这些信息，而不是让 backend 反推。
3. devirtualization 的判定部分
   - “receiver exact type 是否已知”“target set 是否收缩成 singleton”是中端分析问题。
   - 真正留给 backend 的只应是：在确定 `DirectCall` / `VirtualCall` / `InterfaceCall` 之后，如何生成相应底层代码。
4. operator overload 目标确定
   - 当前 operator overload 的目标 materialization 仍发生在 codegen 阶段，这直接导致 `llvm/mod.rs` 里还要用 eager inclusion 补 reachable 集。
   - 这类“把语义调用点具象成具体 callee”的工作应前移。
5. `state_machine_plan / segments / transform`
   - 这三部分已经构成一个 effect middle-end。
   - 其中大部分工作是 HIR/MIR 结构分析、resume path 重写、summary 计算、segment 投影与 canonical machine 构造，不应继续挂在 LLVM codegen 目录里。
   - 真正属于 backend 的主要是 emitter。
6. higher-order effect / suspendability summary
   - 当前对函数值 `may_outward_effect` / `may_suspend` 的分析，实际上就是后续 summary-driven inlining、continuation escaping analysis 的基础设施雏形。
   - 这应与 early MIR / ANF 上的 summary 体系合并，而不是继续作为 `MainCodegen` 的现场查询逻辑。
7. `when` / pattern lowering
   - 当前 LLVM 直接从 HIR 生成复杂 `when` CFG，意味着 pattern matching 还没有在更早层正规化。
   - 随着 MIR 已经具备最小 block/local/CFG 骨架，这部分应该尽早进入 MIR。

一个重要准则是：不要把本应迁出的中端逻辑“换个 LLVM 子模块名字继续留在 `llvm/codegen/` 下面”。那样只会得到更碎的目录，不会得到更清晰的层次边界。

### 10.4 `MainCodegen` 的拆层建议

当前 `MainCodegen` 同时承载：

- module 级只读输入；
- layout / type / effect 相关 cache；
- 当前函数 builder 状态；
- 局部环境与返回上下文；
- GC root slot 状态；
- effect / continuation runtime function 状态。

这会带来两个问题：

- 结构上，一个类型同时扮演“module context”“function context”“analysis cache”“effect emitter context”；
- 性能上，closure body、object init、wrapper function 等路径反复 `MainCodegen::new`，导致共享事实与缓存难以持久化。

更合理的边界大致是：

- `ModuleCodegenCx`
  - 持有 module / context / target / shared readonly program facts。
- `FnCodegenCx`
  - 持有当前函数 builder、env、return context、临时 slot、局部 control-flow 状态。
- `SharedAnalysisCache`
  - 持有可跨函数复用的 layout、summary、known effect instance、callee metadata 等缓存。
- `EffectCodegenCx`
  - 持有当前 effect/state-machine emitter 的专用上下文。

更关键的是：后续 effect/state-machine 的 planning 层，不应再依赖 `MainCodegen` 本体。它应当只依赖一个 backend-agnostic 的 `ProgramFacts` / `EffectAnalysisCtx`。

## 11. 编译器自身性能调查

除了生成代码质量，这轮调查还暴露出编译器自身，尤其是 debug / `-O0` 路径上的几个结构性热点。

### 11.1 当前高概率热点

1. `MainCodegen::new` 的重复构造
   - 顶层函数、closure body、object init、wrapper function 等路径都会重新构造一个完整 `MainCodegen`。
   - 这让很多原本可以共享的 program facts / caches 难以持久化。
2. 按查询临时重建分析上下文
   - 某些 higher-order effect / suspendability 查询每次都会现场组装大量 `HashMap` / `HashSet` / analysis view。
   - 这类成本与最终是否真的需要做重写往往并不严格绑定。
3. reachability 的重复扫描与 eager inclusion
   - 当前既有 HIR reachability 扫描，又有因为 backend 才决定 call target 而产生的补扫、补入 reachable 集。
   - 这对 `-O0` 同样是固定成本。
4. effect middle-end 的 debug 校验成本
   - `build_unified_lowering_contract` 在 `debug_assertions` 下会做 builder contract 验证和 segment round-trip 验证。
   - 这些检查在开发期有价值，但也会抬高编译器自身 debug build 的常数成本。
5. O0 路径并不是真正的“轻量空转”
   - 即使 `-O0`，当前 LLVM backend 仍要跑固定的 `SROA + rewrite-statepoints-for-gc`，并打开 `verify_each`。
   - 这意味着如果中端再把很多昂贵分析做成“默认总会执行”，调试编译的反馈时间会很容易恶化。

### 11.2 对后续设计的直接要求

这些热点意味着，优化设计不仅要改善生成代码，还要避免把编译器自己做慢。

因此后续设计应满足：

- 共享 program facts 与 summaries 应尽量一次构建、多处复用，而不是在 codegen 查询点重复拼装；
- MIR/ANF 上的 `CallKind`、receiver exactness、provenance、summary 结果应显式挂在 IR 或稳定 side tables 上，而不是让 backend 反复推断；
- monomorphic instance 的收集与实例化应 reachable-driven、on-demand，并按 `InstanceKey` 缓存，而不是全量 eager clone 或 codegen 现场临时解析；
- `-O0` 路径应只保留必要 canonicalization 与必须的语义准备，不应默认执行多轮 interprocedural summary / devirt / inline 迭代；
- 需要昂贵验证的 pass，应明确区分“开发期断言”和“默认编译路径”；
- codegen 边界整理本身就应被视为一个 compiler-performance 任务，而不只是代码风格整理。

### 11.3 对落地顺序的含义

这也解释了为什么第一步应从 codegen refactoring 开始，而不是直接把更多优化规则叠到现有 `llvm/codegen` 上：

- 如果不先拆边界，新的 summary / devirt / escape analysis 很容易继续长在 `MainCodegen` 上；
- 如果不先抽 program facts，early MIR 即使引入了，也可能仍然被迫从 LLVM codegen 反向取信息；
- 如果不先整理共享缓存与上下文，优化还没做强，编译器自身的固定成本就会先上升。

因此，从当前仓库状态出发，“先做 codegen refactoring，再引入 early MIR / ANF”不是偏好问题，而是更稳妥的工程顺序。

## 12. 优化级别与 `@Inline`

### 12.1 优化级别建议

一个可行的起点是：

- `-O0`
  - 构建 early MIR / ANF，并完成必要的按需实例化、canonicalization 与 call classification；
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

这里还需要补一条工程约束：

- codegen 边界整理、program facts 抽取、以及 early MIR 的建立，本身不是“高优化级别才开启”的可选项；
- 真正受优化级别控制的，是 summary / devirt / inline / escape analysis 的预算、轮数与激进程度。

### 12.2 `@Inline` 的位置

如果未来保留 `@Inline`，建议它的语义保持极窄：

- 作为一个 override / hint；
- 只影响阈值或强制策略；
- 不改变语言语义；
- 也不构成优化体系的基础假设。

更直接地说：

- 没有 `@Inline`，优化也应该按结构自动发生；
- 有 `@Inline`，只是少量特殊场景下帮助编译器越过默认预算。

## 13. Mem2reg 与 Safepoint 方向

### 13.1 近期不把 `mem2reg` 作为主路径

在当前实现现实下，`mem2reg` 不是这轮设计的主目标。

原因不是它永远不值得做，而是当前 LLVM + moving GC 路径里，它和 roots / safepoint 合同直接耦合：

- 现有 moving GC 依赖 stack-backed local roots；
- ordinary safepoint 前后存在显式 spill / relocate / writeback 合同；
- 如果贸然把 GC roots 提升为寄存器值，而没有新的准确 root 表达与 relocate 机制，正确性就会先出问题。

因此，v1 的重点应是：

- 先减少 safepoint 数量；
- 先减少必须跨 safepoint 存活的函数值、receiver、closure、continuation；
- 先减少“本可被 inline/devirt 消掉，但目前仍残留”的调用边界。

### 13.2 early MIR 对这条线的帮助

虽然 v1 不直接做 `mem2reg`，但 early MIR 仍然能为这条线打基础。

需要强调的是：这里讨论的是“当前 LLVM backend 下的近期工程优先级”，而不是要把这些约束上升成 MIR 本身的语义定义。对未来的 C / JVM / CLR / hosted backend 来说，roots、collector entry、safepoint 的落地机制都可能不同；early MIR 只需要保留足够的抽象语义，让各 backend 在自己的 lowering 阶段完成映射。

在这个前提下，early MIR 的帮助主要体现在：
- 它让“哪些操作会形成潜在 collector / 调度边界”更早可见；
- 它让“哪些调用本可被消掉”更早可见；
- 它让后续若要研究更精细的 root liveness / safepoint sinking / backend-specific barrier placement，有一个比 HIR 更稳定的分析层。

换句话说，近期路线是：

- 先通过 inline / devirt / closure simplification 降低 safepoint 压力；
- 再视 GC root 合同演进情况，决定是否继续推进更激进的 `mem2reg` / register-root 研究。

## 14. 分阶段落地建议

### 14.1 第零阶段：整理现有 LLVM codegen 边界

这一阶段的目标不是引入新优化，而是把“还能继续留在 backend 的部分”和“已经应迁出的部分”先分开。

需要优先完成的事情包括：

- 拆 `MainCodegen` 的上下文层次，避免它继续同时承担 module / function / cache / effect emitter 四类职责；
- 把调用 lowering、builtin/sysroot lowering、closure lowering、class/object/enum lowering、GC lowering、runtime ABI glue 从 `codegen/mod.rs` 的巨型聚合形态里拆出去；
- 把 `llvm/mod.rs` 中的 emit API、pipeline、reachability、测试边界拆开；
- 明确一个规则：新的 backend-agnostic 分析，不再继续直接挂到 `llvm/codegen/` 下。

这一阶段本身就能改善两件事：

- 后续 early MIR / ANF 的迁移路径会更清晰；
- 编译器自身的固定开销不会继续因为巨型上下文与重复构造而恶化。

### 14.2 第一阶段：抽离 backend-agnostic 的 program facts / summaries

在真正引入 early MIR 之前，先把当前已经“像中端分析”的部分从 LLVM codegen 依赖里解耦出来。

优先对象包括：

- callee target resolution 的共享事实；
- receiver exactness / target-set shrinking 所需的静态事实；
- higher-order function value provenance；
- `may_outward_effect` / `may_suspend` 相关 summary；
- effect/state-machine planning 所需的 `ProgramFacts` / `EffectAnalysisCtx`。

这里的关键不是先换消费者，而是先换依赖方向：

- 当前是中端分析从 `MainCodegen` 取信息；
- 目标应变成 LLVM codegen 与 future MIR pass 都依赖同一份 backend-agnostic facts / side tables。

### 14.3 第二阶段：最小 early MIR / ANF

只做最小承载层，不追求一开始就很强：

- 显式基本块；
- 显式局部绑定；
- 显式 call kinds；
- 显式 `Perform` / `Resume`；
- 保留足够的类型、dispatch 与 provenance 元信息。

这一阶段还应开始把当前 backend 里晚做的几类“语义决定”前移：

- 调用分类；
- 初始 callee resolution；
- `when` / pattern lowering 的最小正规化入口。

这里还要明确一个结构边界：

- 这一阶段先产出 generic MIR template；
- 后续优化主流程不应直接消费仍带 type params 的模板 body。

### 14.4 第三阶段：monomorphization / instance materialization

这一阶段把 generic MIR template 转成 monomorphic MIR instances。

核心要求是：

- 以 `InstanceKey` 作为实例身份，而不是以最终 mangled 符号名作为语义身份；
- 以 reachable-driven、on-demand 的方式收集实例；
- 对实例化结果做缓存，避免重复克隆与重复建图；
- 让后续 summary / call graph / devirt / inline 都按实例工作。

这一阶段结束后，后续 pass 消费的主体应是：

- monomorphic MIR instance

而不是：

- generic template
- codegen 现场推断出来的“临时单态目标”

### 14.5 第四阶段：summary 基础设施

先做最保守的跨函数摘要：

- `body_known`
- `size_cost`
- `recursive_scc`
- `may_outward_effect`
- 函数值参数使用摘要
- 基础 provenance

这里的实现要求是：

- summary 应能稳定挂在 MIR 或 side tables 上；
- 不应继续以“codegen 查询时现场重建分析上下文”的方式提供。

### 14.6 第五阶段：通用 devirtualization

先统一处理所有 `VirtualCall` / `InterfaceCall`：

- 只要 target set 静态为 singleton，就改写为 `DirectCall`；
- 先不做 speculative guard；
- 先不按名字区分热点。

这一阶段还应把当前 backend 中的“去虚化判定逻辑”彻底上移，让 LLVM backend 只消费已经分类完成的调用节点。

### 14.7 第六阶段：summary-driven inlining

先支持：

- body-known；
- 非递归；
- 小体量；
- `DirectCallOnly` 参数；
- 实参 provenance 可知；
- 非逃逸 closure / 函数值的最保守重写。

这一阶段即使能力有限，也已经是通用方案，而不是特判方案。

### 14.8 第七阶段：continuation / closure 逃逸分析

在同一层继续扩展：

- non-escaping closure elision；
- continuation escaping analysis；
- 对 effect/state-machine 规划提供更细粒度输入。

这一阶段应直接复用前面的 summary / provenance / call-kind 基础设施，而不是另起一套专用机制。

### 14.9 第八阶段：迭代扩展覆盖面

后续扩展方向应该是：

- 扩展结构识别能力；
- 改善 summaries 的精度；
- 改善 provenance / target-set shrinking；
- 引入更成熟的 budget / profitability 模型；
- 继续减少 effect/state-machine planning 前仍残留的不必要调用边界。

而不是继续累积“又支持了几个特殊函数名”。

## 15. 非目标

本文明确不把以下方向作为 v1 目标：

- 不做基于函数名或 stdlib API 名字的白名单内联；
- 不把 `inline` 关键字当作主机制；
- 不优先做 `mem2reg` / register-root 改造；
- 不要求一开始就引入完整 SSA；
- 不要求一开始就支持 speculative guarded devirtualization；
- 不要求一开始就做完整全程序优化。

## 16. 总结

这份设计的核心，不是“先把 `map` / `filter` / `Iterator.next()` 优化掉”，而是先建立一个足够小、但语义位置正确的中端层：

- 它能显式表达调用形态；
- 它能承载 summary-driven inlining；
- 它能对所有 receiver exact 的 interface/class 调用统一做 devirtualization；
- 它能为 continuation escaping analysis 提供稳定落点；
- 它还能在当前 GC 合同不变的前提下，通过减少调用边界和 safepoint，为后续性能优化创造空间。

因此，第一步不是扩更多特判，也不是继续把中端逻辑往 LLVM 子模块里分摊，而是先整理现有 codegen 边界，再把 early MIR / ANF 这一层立起来。只要这一层存在，后面的优化能力就可以沿着“结构覆盖面”持续扩张，而不是沿着“函数名白名单”持续堆积。
