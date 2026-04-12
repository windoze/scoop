# Effect 统一状态机设计（T2003u1）

> 状态：设计定稿  
> 适用范围：`T2003u1` 的架构收口基线；后续实现任务为 `T2003u2`～`T2003u6`。  
> 非目标：本文不定义新的语言语义，也不承诺 runtime 符号名稳定；它只定义 effect lowering 的统一内部模型与实现边界。

## 1. 背景与问题

当前 LLVM effect lowering 已经能覆盖不少回归，但主路径仍按源码形状拆成多套专门实现：

- `immediate_resume.rs`：围绕单个 distinguished immediate site 的栈上 state machine。
- `escape_continuation.rs`：围绕 escape continuation 的堆上 state machine。
- `mixed.rs` / `matrix.rs`：围绕 mixed-arm、site matrix、nested control-flow 逐项扩面的组合 lowering。
- `scan.rs`：为不同 lowering 维护不完全一致的 site 扫描与门禁。

这种路线的问题不是“还差几个 case”，而是结构上无法收敛：

- direct / indirect perform、nested handle、branch / loop、mixed-arm dispatch 被分散在多套 scanner / replay helper / cleanup 逻辑里；
- `top-level` / `nested block` / `if` / `while` / `same-stmt mixed` 变成 lowering 主线维度，而不是同一个 CFG 上的不同形状；
- runtime ABI 的真实约束（payload transport、handler stack、one-shot continuation、`finally`）被多套 lowering 分别定义，长期必然漂移。

因此，effect lowering 的主线目标调整为：

1. 先把一个 `handle` 统一建模成完整的、可恢复的状态机计划。
2. 再从同一个计划上派生 never-resume、immediate-resume、escape-continuation 三种运行模式。
3. 只有语言语义层面真实非法的组合才能保留诊断；不能再用“源码形状暂不支持”充当终态实现边界。

## 2. 统一 pass 的输入

统一 pass 的输入是“带类型信息、已完成前端语义收口的 `handle` 级 IR 视图”。它不要求前端直接暴露 CFG，但要求以下语义信息在 typed HIR 或后续中端中可稳定读取：

- `handle` body 的语句/表达式树，包含 `if`、`while`、nested block、nested handle。
- handler arms 的元数据：
  - 目标 operation；
  - binder 与 payload 类型；
  - arm 形态：non-resuming / immediate-resume / continuation-binder；
  - `finally` / cleanup 结构。
- suspension candidate：
  - direct perform；
  - 可能间接 perform 的调用点；
  - 调用一个已经状态机化 callee 的边界。
- source-level 顺序信息：
  - arm dispatch 必须按源码顺序匹配；
  - cleanup / `finally` 的进入和退出顺序必须可恢复。
- 局部变量与临时值的类型信息，用于 frame layout、GC trace 与 payload decode。

统一 pass 不再把“top-level val-bound perform”“nested block indirect site”“if-branch paired direct site”作为输入类型的一部分。它只接收“这里有一个 suspend site，它位于某个 CFG 边界上”的语义事实。

## 3. 统一 pass 的输出

统一 pass 的输出是一个模式无关的 `HandleStateMachinePlan`。命名不是 ABI；下面的字段是概念结构：

```text
HandleStateMachinePlan
- handle_id
- entry_state
- states[]
- suspend_sites[]
- arm_plans[]
- cleanup_scopes[]
- frame_layout
- dispatch_plan
- result_layout
```

### 3.1 `states[]`

每个 state 表示“从某个恢复点开始，到下一个控制边界为止”的一段顺序执行片段：

- 片段内允许普通表达式求值、赋值、局部声明与纯控制转移。
- 片段结尾必须以显式 terminator 结束，而不是依赖 emitter 隐式回填。
- terminator 只允许以下几类：
  - `Goto(next_state)`
  - `Branch(cond, then_state, else_state)`
  - `Suspend(site_id)`
  - `Return(value_slot)`
  - `PropagateRaise(payload_slot, cleanup_path)`

统一要求“每个恢复点唯一对应一个 state id”。不再让不同 lowering 自己拼 `dispatch_bb` / `resume_bb` / `tail_bb` 命名体系。

### 3.2 `suspend_sites[]`

所有挂起点统一进入同一个抽象：

```text
SuspendSite
- site_id
- source_span
- site_kind
- effect_op / call_target
- dispatch_order[]
- resume_target
- cleanup_path
- frame_capture_set
- binder_layout
```

其中 `site_kind` 至少覆盖：

- `DirectPerform`
- `IndirectCallMaySuspend`
- `CallStateMachineCallee`

它们共享以下不变量：

- 必须有唯一的 `resume_target`，即“恢复后从哪里继续执行”；
- 必须有统一的 payload encode/decode 规则；
- 必须显式携带 cleanup / `finally` 信息；
- 必须显式指明需要跨 suspension 保存的 slots 与 handler 栈语义。

direct / indirect site 的差异只体现在“如何触发 suspend”和“谁来填充 payload/dispatch 信息”，而不再体现在“是否走另一套主算法”。

### 3.3 `arm_plans[]`

每个 arm 被建模为一个 dispatch entry，而不是直接和某个 emitter 绑定：

```text
ArmPlan
- arm_id
- op_fqn
- resume_mode
- binder_slots[]
- body_entry_state
- body_exit
- detach_policy
```

- `resume_mode` 只表达语义：
  - `NeverResume`
  - `ImmediateResume`
  - `EscapeContinuation`
- `body_exit` 说明 arm body 结束后是：
  - 直接返回 handle 结果；
  - 跳回某个 `resume_target`；
  - 暴露 continuation 对象并把控制权交给外部 caller。
- `detach_policy` 用于显式记录 arm body 执行期间需要临时移出的 sibling handler frames / captured handler stack，而不是在不同 lowering 中隐式手写。

### 3.4 `cleanup_scopes[]`

`finally` / cleanup 不再是 emitter 的“附带流程”，而是 plan 的一等结构：

- 每个 cleanup scope 记录：
  - 进入条件；
  - 退出条件；
  - 对应的 cleanup state 序列；
  - cleanup 完成后回到哪里。
- normal return、non-resuming propagate、arm body raise、resume 后继续执行再 raise，全部通过同一个 cleanup edge 图来表达。

这条规则的目标是保证：

- `finally` 恰好执行一次；
- 对同一条控制路径，cleanup 是否运行由 plan 决定，而不是由具体 emitter 的分支布局偶然决定；
- 旧路径里“先 detach handler 再 finally”“先 finally 再 re-raise”这类时序差异，必须在 plan 层统一。

### 3.5 `frame_layout`

frame layout 是所有运行模式共享的逻辑槽位定义，至少包括：

- `pc/state`：当前 state id；
- `resume_payload`：统一的 payload transport；
- `lifted_locals[]`：跨 suspension 存活的 locals / temporaries；
- `arm_binder_slots[]`：dispatch 后 arm body 读取的 binder；
- `cleanup_flags`：保证 cleanup 一次性的状态位；
- `one_shot_flag`：仅对 escape-continuation materialization 生效。

frame layout 先描述“逻辑上必须保存什么”，再由后续 simplification 决定：

- stack slot；
- heap object field；
- 或被证明可消除的纯 SSA 值。

## 4. 核心不变量

统一状态机 pass 的实现必须满足以下不变量。

### 4.1 形状无关

同一语义结构，无论出现在 top-level、nested block、`if`、`while`、nested handle 之后，必须进入同一个 plan builder 主算法。语法位置只影响 CFG 和 cleanup scope，不得决定是否换一套 lowering。

### 4.2 先完整、后化简

never-resume、immediate-resume、escape-continuation 都不能绕开完整 plan 构建。允许后续 pass 证明“这个路径不需要真正物化 continuation”，但不允许在构建前先按模式裁剪输入。

### 4.3 Resume target 唯一且显式

每个 suspend site 都必须有唯一 `resume_target`。如果一个 site 在语义上可能恢复到多个位置，那么这是 plan builder 的 bug，而不是 emitter 可自行推断的细节。

### 4.4 Cleanup 由图表达，不由 emitter 猜测

cleanup / `finally` 的执行条件、执行顺序、以及执行后续边都必须出现在 plan 中。LLVM emitter 只能按图发射，不能再在各模块中重建一套“异常时也许要跑 finally”的局部规则。

### 4.5 Payload transport 唯一

effect payload、resume value、callee suspend result 都统一走同一套逻辑 transport：

- `word`：标量位模式；
- `gc_ref`：GC 引用或 boxed aggregate。

任何新的 lowering 都不得再引入 `*_int` / `resume_word only` 这种旁路模型。

### 4.6 Handler stack 语义唯一

- 最近匹配 handler 优先；
- arm body 执行时，按 `detach_policy` 明确哪些 sibling frame 不在当前动态 handler 栈里；
- escape continuation 恢复时，安装的是捕获到的 handler stack 快照，而不是“当前线程碰巧还在”的 TLS 状态。

### 4.7 One-shot 语义唯一

“一个 continuation 最多恢复一次”必须是 plan 与 runtime 共享的语义约束：

- escape continuation materialization 必须携带 one-shot 状态位；
- double resume 的诊断/运行时错误路径必须和现有 `ContinuationAlreadyResumed` 约定对齐；
- immediate-resume 的同步折返路径虽然不暴露 continuation 对象，但仍必须遵守“一次 dispatch 只回到一个 resume_target 一次”。

## 5. 三种运行模式与化简边界

统一 plan 构建完成后，再做 mode-specific simplification。

### 5.1 Never-resume（non-resuming arm）

语义：arm body 不把控制权交还给被挂起的主体计算。

化简规则：

- 不物化 continuation 对象；
- 允许把 `Suspend(site)` + `ArmPlan(NeverResume)` 化简为现有 flag-based unwinding / catch block 路径；
- 但 cleanup edge、binder decode、detach/restore 规则仍必须来自统一 plan，而不是单独维护一套 non-resuming lowering 语义。

### 5.2 Immediate-resume

语义：arm body 同步调用 `resume(value)`，并在同一个动态调用链内回到 `resume_target`。

化简规则：

- 当 analysis 证明 continuation 不逃逸时，可以把 `frame_layout` 物化为栈槽或局部循环；
- `resume(value)` 不是重新构建第二套状态机，而是把 payload 写回统一 frame layout 并跳到 `resume_target`；
- 如果 mixed-arm 中只有部分 arm 是 immediate-resume，也只对这些 dispatch edge 做同步折返化简，不影响其它 arm 的 plan 表达。

### 5.3 Escape-continuation

语义：arm body 获得 continuation 对象，恢复动作可能在之后、在别的函数里、甚至在别的线程上发生。

化简规则：

- continuation 对象持有 heap frame、当前 `pc/state`、lifted locals、captured handler stack、one-shot 状态；
- `resume(value)` 安装 captured handler stack，写入统一 payload transport，然后驱动 step 函数从 `resume_target` 对应的 state 继续执行；
- step 函数与 immediate-resume 的“恢复后执行哪段代码”共享同一份 state plan，只是一个走同步折返，一个走外部驱动。

### 5.4 Mixed-arm handle

一个 `handle` 内可以同时存在上述三种 arm。统一方案要求：

- 构建期只生成一份 `HandleStateMachinePlan`；
- dispatch 后按命中的 `ArmPlan.resume_mode` 选择化简/物化策略；
- mixed-arm 的 sibling detach/restore、cleanup、dispatch order 都来自同一份 plan，而不是 `mixed.rs` 特有规则。

## 6. 与现有 runtime ABI 的对接

本文不引入新的语言 ABI，只要求统一 pass 与现有 runtime 语义严格对齐。

### 6.1 Payload transport

继续沿用现有双通道 payload 语义：

```text
AbiPayloadTransport
- word
- gc_ref
```

适用范围：

- non-resuming perform slot；
- `Continuation.resume(...)`；
- callee suspend / resume；
- arm binder decode。

统一 pass 只决定“哪个逻辑值进入哪个 transport 槽位”，不改变 runtime 已经接受的 `word + gc_ref` 形状。

### 6.2 Handler stack 与 TLS 状态

继续沿用现有 TLS handler stack / perform slot 模型：

- non-resuming perform 通过 TLS slot + flag 传播；
- escape continuation 在恢复前把 captured handler stack 安装到当前线程的 TLS 中；
- arm body / cleanup 运行前，必须先按现有约定 capture-and-clear perform slot，避免用户代码在脏 flag 状态下运行。

统一 pass 的职责是把这些边界显式编码进 `dispatch_plan` 与 `cleanup_scopes[]`，而不是重新定义 runtime 行为。

### 6.3 Continuation one-shot

runtime 仍负责最终的 one-shot enforcement；统一 pass 需要保证：

- 只有 escape-continuation materialization 会暴露 continuation 对象；
- 任何会再次进入同一 heap frame 的路径都先检查 one-shot 状态；
- immediate-resume 与 never-resume 不绕开这套语义，只是它们不需要对外暴露 continuation API。

## 7. 对现有代码路径的迁移要求

`T2003u2`～`T2003u6` 应按以下顺序迁移，而不是继续给旧路径补 case：

1. 新增统一 plan builder，输出可 dump / pretty-print / golden 的 `HandleStateMachinePlan`。
2. 让 direct perform、indirect call、nested control-flow、nested handle、multi-arm dispatch 都先接入 plan builder。
3. 在 plan 之上实现 never-resume / immediate-resume / escape-continuation simplifier。
4. LLVM emitter 改为消费 plan；旧的 `immediate_resume.rs` / `escape_continuation.rs` / `mixed.rs` / `matrix.rs` 先保留作回归对照，再逐步删除重复逻辑。

迁移期间允许保留旧模块，但不再允许：

- 为新的 top-level / nested / same-stmt 组合继续新增专用 scanner；
- 在旧 emitter 中引入新的 payload / cleanup / handler stack 语义分支；
- 用“当前源码形状不支持”替代 plan 层本应统一表示的合法语义。

## 8. 需要继续锁定的验收点

`T2003u1` 完成后，后续任务至少要验证以下事实：

- 同一语义程序在 top-level / nested block / `if` / `while` 中生成的 plan 结构一致，只是 state/edge 数量随 CFG 改变；
- mixed-arm 的 dispatch order、detach/restore、cleanup path 不再因 emitter 模块不同而漂移；
- direct / indirect / callee-resume 共享同一套 payload transport；
- 只有真实非法组合继续保留诊断；形状性门禁应随着 plan 迁移逐步删除。

## 9. 当前边界

本文刻意不解决以下问题；它们由后续任务承接：

- `T2003u2`：plan builder 的具体数据结构、builder 代码与 dump 测试；
- `T2003u3`：三种模式的化简 pass；
- `T2003u4`：LLVM emitter 切换；
- `T2003u5` / `T2003u6`：迁移剩余 mixed-arm 组合、删除结构性门禁、补 full matrix 与 GC stress。

前端 `do { ... }` / 分号规则（`T22`）不是统一状态机 pass 的前置条件，但后续 effect fixtures 迁移时需要与本设计共同收口。
