# Effect 统一状态机设计（T2003u1）

> 状态：设计定稿  
> 适用范围：`T2003u1` 的架构收口基线；后续实现任务为 `T2003u2`～`T2003u7`。
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

1. 先按控制流边界把一个 `handle` 所在的可恢复 region 切成统一的 segment list。
2. 再只从这份 segment list 构建完整的、可恢复的状态机计划。
3. 最后只从同一个状态机计划上派生 never-resume、immediate-resume、escape-continuation 三种运行模式并发射代码。
4. 只有语言语义层面真实非法的组合才能保留诊断；不能再用“源码形状暂不支持”充当终态实现边界。

## 1.1 重写立场与唯一实施顺序

这条路线明确是**完全重写**，不是继续修补旧的 shape-based emitter：

- `single arm`、`single perform`、`perform inside while`、`top-level`、`same-stmt mixed`、`nested block` 之类源码形状，只能作为覆盖样例的名字，不能再作为算法分流条件。
- 唯一允许的主流程是：
  1. `segmenting`：按控制流边界产出统一的 `HandleSegmentList`；
  2. `state-machine building`：只从 segment list 构建统一状态机；
  3. `emitting`：只从统一状态机与 simplification 结果发射 LLVM。
- 这三个阶段必须严格顺序推进；前一阶段没有对**所有合法情况**形成完整、可连接到下一阶段的统一表示之前，不允许进入下一阶段补临时例外。
- 实现期间不得给旧 `immediate_resume.rs` / `escape_continuation.rs` / `mixed.rs` / `matrix.rs` / `scan.rs` 新增功能；所有新能力只能落在统一分段、统一 builder、统一 emitter 主线。
- 在统一 emitter feature-complete 之前，可以以结构 dump、单元测试、定向 fixtures 为主，不要求每轮都跑 full suite；full matrix、`cargo test --all`、GC stress 的强制门槛统一放到最终覆盖性验收。
- 统一状态机变换完成后，旧的 shape-based emitter / scanner / dedicated matrix 路径必须**整体删除**；不保留 fallback、双轨或“以防万一”的 legacy 入口。

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

### 2.1 第一阶段输出：`HandleSegmentList`

第一阶段不是“先认出一堆特殊形状”，而是把可恢复 region 统一切成 segment list。概念结构如下：

```text
HandleSegmentList
- entry_segment
- segments[]
- edges[]
```

```text
Segment
- segment_id
- source_span
- ops[]
- terminator
- cleanup_scope_stack
- dispatch_context
```

分段规则必须满足：

- segment 的边界只由控制流与恢复语义决定，例如 branch split/merge、loop head/back-edge/exit、suspend site、arm dispatch entry/exit、nested handle entry/exit、cleanup entry/exit；
- segmenter 必须对所有合法组合复用同一套算法，而不是为 `single arm`、`perform inside while`、`if branch indirect site` 等形状建立独立分类器；
- state-machine builder 的唯一输入就是这份 segment list；builder 不允许回头重新查看源码形状来决定“这一类要走另一套构造器”。

## 3. 统一 pass 的输出

统一 pass 的第二阶段输出是一个模式无关的 `HandleStateMachinePlan`。命名不是 ABI；下面的字段是概念结构。它必须只由 `HandleSegmentList` 推导，不能绕过 segment list 再按源码形状重建另一套 plan：

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
- 每个 state 只能由 segment list 归并/映射得到；允许做纯结构性的合并或消除，但不允许绕过 segment list 重新按 `single arm` / `while` / `same-stmt` 造另一套 state。
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

### 4.1 先分段，再建模，再发射

`segmenting -> state-machine building -> emitting` 是唯一允许的阶段顺序：

- builder 只能消费 `HandleSegmentList`；
- emitter 只能消费 `HandleStateMachinePlan` 与 simplification 结果；
- 任何阶段如果还需要靠旧 emitter / scanner 打补丁来覆盖合法组合，说明该阶段尚未完成，不能把下一阶段当成新的主线。

### 4.2 形状无关

同一语义结构，无论出现在 top-level、nested block、`if`、`while`、nested handle 之后，必须进入同一个 plan builder 主算法。语法位置只影响 CFG 和 cleanup scope，不得决定是否换一套 lowering。

### 4.3 先完整、后化简

never-resume、immediate-resume、escape-continuation 都不能绕开完整 plan 构建。允许后续 pass 证明“这个路径不需要真正物化 continuation”，但不允许在构建前先按模式裁剪输入。

### 4.4 Resume target 唯一且显式

每个 suspend site 都必须有唯一 `resume_target`。如果一个 site 在语义上可能恢复到多个位置，那么这是 plan builder 的 bug，而不是 emitter 可自行推断的细节。

### 4.5 Cleanup 由图表达，不由 emitter 猜测

cleanup / `finally` 的执行条件、执行顺序、以及执行后续边都必须出现在 plan 中。LLVM emitter 只能按图发射，不能再在各模块中重建一套“异常时也许要跑 finally”的局部规则。

### 4.6 Payload transport 唯一

effect payload、resume value、callee suspend result 都统一走同一套逻辑 transport：

- `word`：标量位模式；
- `gc_ref`：GC 引用或 boxed aggregate。

任何新的 lowering 都不得再引入 `*_int` / `resume_word only` 这种旁路模型。

### 4.7 Handler stack 语义唯一

- 最近匹配 handler 优先；
- arm body 执行时，按 `detach_policy` 明确哪些 sibling frame 不在当前动态 handler 栈里；
- escape continuation 恢复时，安装的是捕获到的 handler stack 快照，而不是“当前线程碰巧还在”的 TLS 状态。

### 4.8 One-shot 语义唯一

“一个 continuation 最多恢复一次”必须是 plan 与 runtime 共享的语义约束：

- escape continuation materialization 必须携带 one-shot 状态位；
- double resume 的诊断/运行时错误路径必须和现有 `ContinuationAlreadyResumed` 约定对齐；
- immediate-resume 的同步折返路径虽然不暴露 continuation 对象，但仍必须遵守“一次 dispatch 只回到一个 resume_target 一次”。

### 4.9 禁止特殊分支

以下做法都不再允许作为 effect lowering 的主实现：

- `if handle.arms.len() == 1 { ... } else { ... }` 这种按 `single arm` / `multi arm` 选择不同主 emitter；
- `if inside_while { ... }`、`if top_level { ... }`、`if same_stmt_mixed { ... }` 这种按源码形状切换主算法；
- 为 direct / indirect / nested / mixed 各自维护不同的“先扫描、再 replay、再 cleanup”主流程。

如果某个优化只对特定情况成立，它也必须表现为**统一状态机上的化简**，而不是另一套构建或发射路径。

## 5. 三种运行模式与化简边界

统一 plan 构建完成后，再做 mode-specific simplification。唯一允许的化简维度是语义模式与已证明的数据流事实，而不是 `single arm`、`perform inside while`、`top-level` 等源码形状标签。

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

`T2003u2`～`T2003u7` 必须按以下强制顺序迁移，而不是继续给旧路径补 case：

1. 先完成 `segmenting`：为所有合法 handle 产出可 dump / pretty-print / golden 的 `HandleSegmentList`。完成标准是“所有合法组合都能被同一个分段算法表示”，而不是“先给 single-arm / while / nested-if 各补一个例外”。
2. 再完成 `state-machine building`：`HandleStateMachinePlan` 只能由 `HandleSegmentList` 推导；direct / indirect、single / multi-arm、block / if / while / nested handle 都必须走同一套 builder。
3. 再完成 simplification 与 unified emitter：LLVM emitter 只能消费 plan 与 simplification 结果，不再直接查看源码形状决定主路径。
4. 统一 emitter feature-complete 之前，允许以结构 dump、单元测试、定向 fixtures 作为主要验证；`cargo test --all`、full matrix、GC stress 的强制全量门槛统一放到最终覆盖性验收。
5. `T2003u6` 通过后，立即在 `T2003u7` 删除旧的 shape-based scanner / emitter / matrix 主实现；不保留 fallback、兼容开关或“以防万一”的双轨。

迁移期间的硬约束：

- 不给旧 `immediate_resume.rs` / `escape_continuation.rs` / `mixed.rs` / `matrix.rs` / `scan.rs` 新增任何功能；新能力只能落在统一分段、统一 builder、统一 emitter 主线。
- 不允许新增以 `single arm`、`single perform`、`perform inside while`、`top-level`、`same-stmt mixed` 等源码形状为入口条件的专用主路径。
- 如果临时复用旧 helper，它们也只能是统一主线内部的局部实现细节，且不得继续承担 source-of-truth 职责。
- 统一重写完成后，旧的按形状分流 lowering 必须整体消失；保留下来的只能是与旧算法无关的通用工具代码。

## 8. 需要继续锁定的验收点

`T2003u1` 完成后，后续任务至少要验证以下事实：

- 同一语义程序在 top-level / nested block / `if` / `while` 中只会改变 segment/state graph 的拓扑，不会切换到另一套 segmenter、builder 或 emitter；
- `single arm`、`multiple arms`、`perform inside while`、nested handle 等合法组合都走同一条 `segmenting -> builder -> emitter` 主线；
- mixed-arm 的 dispatch order、detach/restore、cleanup path 不再因 emitter 模块不同而漂移；
- direct / indirect / callee-resume 共享同一套 payload transport；
- feature-complete 之前允许以定向测试推进；full suite / full matrix / GC stress 的强制门槛集中在 `T2003u6`；
- 只有真实非法组合继续保留诊断；形状性门禁必须被删除，而不是长期保留；
- `T2003u7` 完成后，仓库中不再存在旧的 shape-based effect lowering 主实现。

## 9. 当前边界

本文刻意不解决以下问题；它们由后续任务承接：

- `T2003u2`：plan builder 的具体数据结构、builder 代码与 dump 测试；
- `T2003u3`：三种模式的化简 pass；
- `T2003u4` / `T2003u5`：把剩余合法组合完整接入统一的 `segmenting -> builder -> emitter` 主线；
- `T2003u6`：在 unified emitter feature-complete 之后执行 full matrix / full suite / GC stress；
- `T2003u7`：删除所有旧的 shape-based lowering 主路径。

前端 `do { ... }` / 分号规则（`T22`）不是统一状态机 pass 的前置条件，但后续 effect fixtures 迁移时需要与本设计共同收口。
