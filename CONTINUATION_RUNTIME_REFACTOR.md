# Continuation Runtime Refactor

## Purpose

本文定义 continuation/effect 运行时收口的最终设计目标。这里讨论的不是“如何兼容现有 bridge”，而是**最终应该收口成什么形状**。

本文的结论是：

1. `ScoopContinuation` 不应继续由 `runtime/c/scoop_runtime.c` 定义和拥有。
2. continuation 的分配、one-shot 检查、resume driver、captured callee state 绑定，都应迁入 LLVM codegen 生成的 GC-aware helper。
3. continuation 内部不应再使用 stable handle，也不应再持有 native `malloc` 的 handler snapshot，因此不再需要 `release_fn`。
4. `EffectCtx`、handler snapshot、callee suspend state、pending continuation、effect outcome 都应改为**显式、GC 可见、可由 codegen 直接操作**的数据，而不是继续经由 TLS scratch / runtime bridge 函数中转。

这份文档刻意写成自包含形式。阅读时不需要频繁跳转其他设计文档；必要背景直接引用当前源码位置。

## Scope

本文覆盖：

1. continuation object model
2. continuation resume/alloc/discard 责任边界
3. captured effect context / handler context 表示
4. ordinary callee suspend state 的表示
5. explicit `EffectOutcome` 的 authoritative contract
6. 需要删除或迁移的 runtime C ABI
7. 需要修改的源码文件与原因

本文不覆盖：

1. 用户态 `Continuation` 语义本身的重新定义
2. parser / typechecker 表面语法改动
3. 普通 FFI / reactor 的 stable handle 合同
4. 线程 API 整体设计

## Current State

### 1. 当前 continuation object 仍归 runtime 所有

当前 `ScoopContinuation` 定义在 `runtime/c/scoop_runtime.c:1140-1178`，字段如下：

1. `resumed`
2. `resume_state_tag`
3. `captured_handler_stack_top`
4. `state_handle`
5. `step_fn`
6. `resume_word`
7. `resume_gc_ref`
8. `captured_callee_suspend_state_handle`

这里最关键的不是“字段多”，而是**ownership 在 C runtime**：

1. `state_handle` 通过 `scoop_handle_new/get/drop` 管理，见 `runtime/c/scoop_runtime.c:1285-1334`, `1524-1687`。
2. `captured_callee_suspend_state_handle` 也通过 stable handle 管理，见 `runtime/c/scoop_runtime.c:1336-1350`, `1553-1625`。
3. `captured_handler_stack_top` 指向 native heap 上的快照，见 `runtime/c/scoop_runtime.c:440-452`, `581-629`。

### 2. 当前 continuation 需要 `release_fn`

当前 continuation 的 `type_desc` 定义在 `runtime/c/scoop_runtime.c:1251-1262`，其 `release_fn = scoop_continuation_release`。

`scoop_continuation_release()` 会清理三类资源，见 `runtime/c/scoop_runtime.c:1267-1283`：

1. `state_handle`
2. `captured_callee_suspend_state_handle`
3. `captured_handler_stack_top`

这说明当前 continuation 不是“普通 managed object + traced fields”，而是“managed shell + runtime-owned side resources”。

### 3. 当前 handler context 仍是 raw TLS stack

当前 runtime 把 `EffectCtx` 物理落成 `handler_top` raw pointer，见 `runtime/c/scoop_runtime.c:336-368`。

handler frame 本身是：

```c
struct ScoopEffectHandlerFrame {
  struct ScoopEffectHandlerFrame *prev;
  uint32_t op_tag;
  uint32_t active;
};
```

定义位置：`runtime/c/scoop_runtime.c:434-438`。

当前线程的动态 handler 上下文保存在：

```c
SCOOP_THREAD_LOCAL ScoopEffectHandlerFrame *__scoop_effect_handler_stack_top = 0;
```

定义位置：`runtime/c/scoop_runtime.c:467-468`。

当前 codegen 在 `handle` 入口为每个 dispatch entry 分配一个**栈上** frame，再调用 runtime `push/pop`，见：

1. `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs:4848-4910`
2. `crates/scoopc/src/llvm/codegen/runtime_abi.rs:58-74`
3. `runtime/c/scoop_runtime.c:1023-1080`

因此 continuation 一旦 escape，原始栈上 frame 会失效，runtime 只能 clone 一份 native snapshot，见 `runtime/c/scoop_runtime.c:440-452`, `581-629`。

### 4. 当前 explicit outcome 仍然不是唯一 source of truth

当前已经有显式 `ScoopValueTransport` / `ScoopEffectSignal` / `ScoopEffectOutcome` 结构，见：

1. `runtime/c/scoop_runtime.c:342-368`
2. `crates/scoopc/src/llvm/codegen/runtime_abi.rs:76-97`

但生产路径仍然依赖 bridge：

1. `scoop_effect_outcome_consume_current()` 从 TLS `__scoop_effect_active + perform_slot + callee_suspend_state` 物化 outcome，见 `runtime/c/scoop_runtime.c:840-868`
2. `scoop_effect_outcome_publish()` 再把 outcome 写回 TLS，见 `runtime/c/scoop_runtime.c:870-886`
3. `scoop_callee_suspend_state_publish/get/clear()` 仍然存在，见 `runtime/c/scoop_runtime.c:894-917`
4. codegen 侧仍显式声明并调用这些 bridge，见 `crates/scoopc/src/llvm/codegen/effect/contract.rs:64-233` 与 `crates/scoopc/src/llvm/codegen/effect/mod.rs:718-721`

### 5. 当前 continuation resume 仍由 runtime 驱动

当前 `Continuation.resume(...)` 的生产 lowering 最终调用 runtime `scoop_continuation_resume_with(...)`，见：

1. `crates/scoopc/src/llvm/codegen/effect/mod.rs:2188-2226`
2. `crates/scoopc/src/llvm/codegen/runtime_abi.rs:1240-1260`
3. `runtime/c/scoop_runtime.c:1772-1820`

runtime resume driver 当前负责：

1. one-shot 检查
2. 安装 captured handler stack
3. 读取 stable handle 得到 `state_ptr` 和 `captured_callee_suspend_state`
4. pin/unpin 这些 raw 指针
5. 调用 `step_fn`
6. 从 frame 读 answer transport
7. 用 TLS scope 传递 `pending_continuation`

实现位置见 `runtime/c/scoop_runtime.c:1553-1744`。

### 6. 当前 runtime API 面暴露过宽

`runtime/c/scoop_runtime_api.h:30-69` 当前仍导出大量 continuation/effect bridge 符号，包括：

1. `scoop_continuation_alloc`
2. `scoop_continuation_discard`
3. `scoop_continuation_resume`
4. `scoop_continuation_resume_into`
5. `scoop_continuation_resume_publish_pending_continuation`
6. `scoop_continuation_set_captured_callee_suspend_state`
7. `scoop_continuation_resume_u64`
8. `scoop_continuation_resume_with`
9. `scoop_continuation_try_resume`
10. `scoop_callee_suspend_state_{publish,get,clear}`
11. `scoop_effect_handler_stack_{push,pop,top,swap_top,...}`
12. `scoop_effect_outcome_{consume_current,publish}`

这意味着 production compiler 仍在直接依赖 runtime continuation bridge，而不是仅依赖 generic GC/thread substrate。

## Problems

### 1. stable handle 的存在是 ownership 放错层的结果

当前 `state_handle` 与 `captured_callee_suspend_state_handle` 的存在，不是 continuation 语义需要，而是因为 owner logic 在 C runtime 中：

1. runtime helper 内部不能像 generated code 一样天然依赖 stackmap-relocatable local roots
2. 因此只能把长期 owner 退化为 stable handle side table，见 `runtime/c/scoop_gc.h:257-275`

一旦 owner 迁回 codegen，这两个字段就可以改回普通 traced ref。

### 2. `release_fn` 的存在不是语义需要，而是 side resource 泄漏补丁

当前 `gc.rs` 生成 type descriptor 时，默认 `release_fn = NULL`，见 `crates/scoopc/src/llvm/codegen/gc.rs:1306-1316`。这本来就适合“纯 traced fields 的普通 managed object”。

continuation 现在之所以不能直接用这条路径，只是因为它当前还持有：

1. stable handle table entry
2. native `malloc` snapshot

只要把这两类 side resource 去掉，continuation 完全可以回到普通 traced object，不需要 `release_fn`。

### 3. handler context 现在不可被 GC 原生理解

当前 handler context 的真实物理表示仍然是：

1. 生产 codegen 在栈上 `alloca` 的 handler frame，见 `state_machine_emitter.rs:4848-4910`
2. runtime TLS `__scoop_effect_handler_stack_top` raw pointer，见 `runtime/c/scoop_runtime.c:467-468`
3. continuation escape 时由 runtime clone 成 native snapshot，见 `runtime/c/scoop_runtime.c:581-629`

这使得 handler context 不能像普通 object graph 那样被 tracing GC 和 statepoint 统一管理。

### 4. explicit outcome 已经有结构，但仍被 TLS bridge 稀释

当前 `ScoopEffectOutcome` 结构已经足够表达：

1. `Complete`
2. `Propagate`
3. payload transport
4. `resume_token`

但 production path 仍把 TLS 当作“短暂 source of truth”，再由 bridge 函数往 outcome 来回搬运，见：

1. `runtime/c/scoop_runtime.c:840-917`
2. `crates/scoopc/src/llvm/codegen/effect/contract.rs:64-233`

这会持续保留 `active flag / perform slot / callee_suspend_state / pending_continuation` 这套 runtime-only bookkeeping。

### 5. 当前测试过多绑定到错误层次

现有 runtime tests 很多在直接调用这些 bridge C API：

1. `crates/scoop_runtime/tests/continuation_one_shot.rs:646-809`
2. `crates/scoop_runtime/tests/continuation_cross_thread_handler_stack.rs:100-189`
3. `crates/scoop_runtime/tests/effect_tls.rs:240-349`

这些测试验证了真实语义边界，但绑定在“当前 bridge 形状”上。最终收口后，语义必须保留，但测试入口应迁移到 generated IR / end-to-end fixture / compiler integration，而不是继续锁定 deleted C ABI。

## Design Goals

### Primary goals

1. continuation 内部不再使用 stable handle
2. continuation 内部不再使用 native heap handler snapshot
3. continuation 不再需要 `release_fn`
4. effect propagation 只以 explicit `EffectOutcome` 为 authoritative contract
5. `callee_suspend_state` 不再经 TLS scratch 中转
6. `pending_continuation` 不再经 TLS active resume scope 中转
7. production codegen 不再声明或调用 continuation/effect bridge runtime symbols

### Secondary goals

1. continuation object layout 仍保持 ABI-stable，但 ABI owner 从 C runtime 转到 codegen
2. cross-thread resume 继续成立
3. moving GC + stress + verify-roots 下的 continuation correctness 更容易验证
4. runtime 只保留 generic substrate，而不是 continuation-specific policy

## Final Design

### 1. Ownership Boundary

最终边界如下：

### Runtime keeps only generic substrate

runtime 只保留：

1. `scoop_alloc_typed` / object header / `ScoopTypeDescriptor`
2. GC trace/relocation infrastructure
3. `scoop_gc_write_barrier`
4. thread register/unregister / native boundary support
5. 通用同步、I/O、array、string、platform helpers
6. 必要的运行时错误常量约定

### Codegen owns continuation/effect runtime policy

codegen 负责生成：

1. `ScoopContinuation` object layout
2. `ScoopEffectCtx` object layout
3. `ScoopEffectHandlerNode` object layout
4. continuation alloc helper
5. continuation one-shot / resume driver
6. explicit outcome materialization and propagation logic
7. ordinary callee suspend-state transport
8. arm self-inactive 的 effect context 变换
9. captured outer handler context 的 dispatch logic

换句话说：**runtime 不再拥有 continuation 的 object model，也不再拥有 continuation 的 control driver。**

### 2. Authoritative Data Model

### 2.1 `ScoopContinuation`

最终 `ScoopContinuation` 是 codegen 拥有的普通 managed object。建议布局如下：

```c
typedef void (*ScoopGeneratedContinuationStepFn)(
    void *state_ref,
    uint64_t resume_word,
    void *resume_gc_ref,
    void *current_effect_ctx_ref,
    void *incoming_resume_token_ref,
    ScoopEffectOutcome *outcome);

typedef struct ScoopContinuation {
  ScoopGcObjectHeader hdr;
  _Atomic uint32_t resumed;
  uint32_t resume_state_tag;
  void *captured_effect_ctx_ref;
  void *state_ref;
  ScoopGeneratedContinuationStepFn step_fn;
  uint64_t resume_word;
  void *resume_gc_ref;
  void *captured_callee_suspend_state_ref;
} ScoopContinuation;
```

字段语义：

1. `resumed`: one-shot 标志，由 generated helper 直接用 LLVM `cmpxchg` 操作
2. `resume_state_tag`: 恢复 body state 的入口
3. `captured_effect_ctx_ref`: suspend 点捕获的动态 effect context，替代当前 `captured_handler_stack_top`
4. `state_ref`: 直接 GC ref，替代当前 `state_handle`
5. `step_fn`: 代码指针，不是 GC ref
6. `resume_word` / `resume_gc_ref`: continuation payload transport，沿用当前 shared transport contract
7. `captured_callee_suspend_state_ref`: 直接 GC ref，替代当前 `captured_callee_suspend_state_handle`

关键性质：

1. continuation 内不再拥有 native side resource
2. continuation 内不再拥有 stable handle
3. traced fields 全部可由 bitmap/trace_fn 表达
4. 因此 `release_fn = NULL`

### 2.2 `ScoopEffectCtx`

`ScoopEffectCtx` 必须从“raw handler_top pointer wrapper”改成普通 managed object：

```c
typedef struct ScoopEffectCtx {
  ScoopGcObjectHeader hdr;
  void *handler_top_ref;
} ScoopEffectCtx;
```

这里的 `handler_top_ref` 指向 `ScoopEffectHandlerNode` 链。

`ScoopEffectCtx` 的职责：

1. 显式表示 suspend 点动态 handler/delimiter 环境
2. 作为 continuation capture 的普通 traced field
3. 作为 ordinary call / resume / nested handle 的 hidden input
4. 作为跨线程 resume 时可安全传递的 managed object graph

它不再落成 TLS raw pointer，也不再由 runtime `swap_top()` 安装/恢复。

### 2.3 `ScoopEffectHandlerNode`

`ScoopEffectHandlerNode` 是新的 managed handler registration 结点。建议布局如下：

```c
typedef void (*ScoopGeneratedHandlerDispatchFn)(
    void *owner_frame_ref,
    void *current_effect_ctx_ref,
    void *incoming_resume_token_ref,
    ScoopEffectOutcome *outcome);

typedef struct ScoopEffectHandlerNode {
  ScoopGcObjectHeader hdr;
  void *prev_ref;
  uint32_t op_tag;
  uint32_t flags;
  void *owner_frame_ref;
  ScoopGeneratedHandlerDispatchFn dispatch_fn;
} ScoopEffectHandlerNode;
```

字段语义：

1. `prev_ref`: 外层 handler 结点
2. `op_tag`: effect op identity
3. `flags`: 至少保留 `ACTIVE` bit；推荐定义为 immutable flags，而不是 runtime in-place mutation
4. `owner_frame_ref`: 对应 handle/delimiter 的 frame object；保证 outer dispatch 在原调用栈已返回后仍有 owner state 可用
5. `dispatch_fn`: outer dispatch loop 的入口；使捕获的 effect context 能在原调用栈消失后重新分发到最近匹配 handler

这比当前 `ScoopEffectHandlerFrame { prev, op_tag, active }` 更完整，也更适合被 continuation 捕获。

### 2.4 `ScoopEffectOutcome`

`ScoopEffectOutcome` 保持当前显式布局，不再改变抽象意义，仍然是：

1. `tag = COMPLETE | PROPAGATE`
2. `complete = ValueTransport`
3. `signal = { op_tag, effect_instance_key, payload, resume_token }`

其当前定义已经存在，见：

1. `runtime/c/scoop_runtime.c:342-368`
2. `crates/scoopc/src/llvm/codegen/runtime_abi.rs:76-97`

最终设计要求：**它成为唯一 authoritative propagation contract。**

### 2.5 `resume_token` 的最终含义

`resume_token` 统一承载“让外层继续推进本次 suspended computation 所需的 token”。

在最终设计中，它可以是：

1. fresh inner continuation
2. ordinary indirect callee suspend state object

但无论是哪一种，它都必须直接作为 GC ref 出现在 `EffectOutcome.signal.resume_token` 中，而不是先落到 TLS scratch 再搬运。

### 3. Hidden ABI

### 3.1 effect-capable ordinary call ABI

所有可能产生 effect propagation 或 ordinary suspend token 的 generated callable，都使用显式 hidden ABI：

```c
void f(
    <user args...>,
    void *current_effect_ctx_ref,
    void *incoming_resume_token_ref,
    <typed result slots...>,
    ScoopEffectOutcome *outcome);
```

语义：

1. `current_effect_ctx_ref`: 当前动态 handler/delimiter 环境
2. `incoming_resume_token_ref`: 显式替代当前 `__scoop_callee_suspend_state`
3. `<typed result slots>`: ordinary complete path 的返回值通道
4. `outcome`: authoritative effect/propagation channel

规则：

1. fresh ordinary call 传 `incoming_resume_token_ref = null`
2. ordinary callee resumed path 传先前显式保存下来的 suspend-state ref
3. `outcome.tag == COMPLETE` 时，result slots 有效
4. `outcome.tag == PROPAGATE` 时，result slots 忽略

### 3.2 step / dispatch ABI

当前 continuation `step_fn` 只有 3 个参数，见 `runtime/c/scoop_runtime.c:1135-1138`。最终应扩展为：

```c
void step_or_dispatch(
    void *state_ref,
    uint64_t resume_word,
    void *resume_gc_ref,
    void *current_effect_ctx_ref,
    void *incoming_resume_token_ref,
    ScoopEffectOutcome *outcome);
```

这两个新增 hidden input 的作用分别是：

1. `current_effect_ctx_ref`: 替代当前 raw TLS handler stack
2. `incoming_resume_token_ref`: 替代当前 raw TLS callee suspend state

这样 step/dispatch 在 resume 时所需的全部动态环境都变成显式输入，不再依赖 runtime TLS 安装。

### 3.3 continuation resume helper ABI

最终保留 module-private generated helper：

```c
uint32_t __scoop_continuation_resume_with(
    ScoopContinuation *k,
    uint64_t resume_word,
    void *resume_gc_ref,
    <typed answer slots...>,
    ScoopEffectOutcome *outcome);
```

注意：

1. 这是 codegen 生成的内部 helper，不是 runtime public API
2. 它运行在 generated, GC-aware code 中，因此可以直接持有 relocatable roots
3. 它不再需要 stable handle 或 runtime-owned release logic

### 4. Effect Context Construction

### 4.1 entering `handle`

当前 codegen 在 `handle` 入口使用栈上 frame + `push/pop`，见 `state_machine_emitter.rs:4848-4910`。最终改为：

1. 为每个 dispatch entry 分配一个 `ScoopEffectHandlerNode` managed object
2. 将这些 node 作为 GC refs 存进当前 effect frame 的固定 slot，保证整个 handle 生命周期都被 rooted
3. 每个 node 的 `prev_ref` 指向 outer ctx 链上对应位置
4. 每个 node 的 `owner_frame_ref` 指向当前 handle 的 effect frame
5. 每个 node 的 `dispatch_fn` 指向当前 handle 的 generated dispatch loop
6. 生成一个 `ScoopEffectCtx` managed object，其 `handler_top_ref` 指向本 handle 最内层 node
7. body/arm/finally 以及所有 nested effect-capable 调用，都显式传入该 `ScoopEffectCtx`

这一步完全替代：

1. `scoop_effect_handler_stack_push`
2. `scoop_effect_handler_stack_pop`
3. `scoop_effect_handler_stack_top`
4. `scoop_effect_handler_stack_swap_top`

### 4.2 arm self-inactive

当前 active/inactive 语义借助 runtime frame `active` 位表达，见 `runtime/c/scoop_runtime.c:421-438`。最终建议如下：

1. `ScoopEffectHandlerNode` 视为 immutable object
2. 进入某个 matched arm body 时，codegen 构造一个 derived `ScoopEffectCtx`
3. 该 derived ctx 通过“复制 prefix 并将 matched node 标记为 inactive”实现 self-inactive，而不是原地修改共享 node
4. 该 derived ctx 作为 arm body 的 hidden `current_effect_ctx_ref`
5. 若 arm body 内再次 capture continuation，应捕获 derived ctx，而不是原 ctx

这样可以避免共享 node 图上的可变 alias 问题。

### 4.3 dispatch beyond the currently executing delimiter

最终设计不再假设“outer handler 一定还在原始调用栈上”。因此必须提供显式外层 dispatch 能力。

建议生成一个 module-private helper：

```c
void __scoop_effect_ctx_dispatch(
    ScoopEffectCtx *ctx,
    void *incoming_resume_token_ref,
    ScoopEffectOutcome *outcome);
```

其职责：

1. 从 `ctx.handler_top_ref` 开始向外寻找最近匹配、且 active 的 `op_tag`
2. 找到后，把当前 `outcome.signal` 填入目标 owner frame 的 incoming-signal slot
3. 调用匹配 node 的 `dispatch_fn(owner_frame_ref, ctx_for_arm_body, incoming_resume_token_ref, outcome)`
4. 若没有匹配 node，则保持 `PROPAGATE` 原样向更外层返回

这一步是当前 raw TLS handler stack 难以显式表达、但 escaped continuation 最终必须具备的能力。

### 5. Continuation Allocation

当前 suspend site 通过 runtime `scoop_continuation_alloc(...)` 分配 continuation，见 `state_machine_emitter.rs:2765-2782` 与 `runtime/c/scoop_runtime.c:1285-1334`。最终改为 codegen 直接生成：

1. `scoop_alloc_typed(desc, sizeof(ScoopContinuation))`
2. 写入 `resumed = 0`
3. 写入 `resume_state_tag`
4. 写入 `captured_effect_ctx_ref = current_effect_ctx_ref`
5. 写入 `state_ref = state_ptr`
6. 写入 `step_fn = dispatch_loop_fn`
7. 写入 `resume_word = 0`, `resume_gc_ref = null`
8. 写入 `captured_callee_suspend_state_ref = current site explicit token slot`

这一步直接替代：

1. `scoop_continuation_alloc`
2. `scoop_continuation_set_captured_callee_suspend_state`

并且不再需要：

1. `state_handle`
2. `captured_callee_suspend_state_handle`
3. `captured_handler_stack_top`

### 6. Continuation Resume Algorithm

最终 `__scoop_continuation_resume_with` 的算法如下：

1. 以 LLVM 原子 `cmpxchg` 检查并设置 `k.resumed`
2. 若失败，直接构造 `Raise<RuntimeError::ContinuationAlreadyResumed>` 的显式 `EffectOutcome`
3. 将 `resume_word/resume_gc_ref` 写入 continuation 字段
4. 从 continuation 直接读取：
   - `state_ref`
   - `captured_effect_ctx_ref`
   - `captured_callee_suspend_state_ref`
   - `resume_state_tag`
   - `step_fn`
5. 若 `resume_state_tag` 已设置，则先写回 `frame.state_tag`
6. 调用：

```c
k->step_fn(
    k->state_ref,
    k->resume_word,
    k->resume_gc_ref,
    k->captured_effect_ctx_ref,
    k->captured_callee_suspend_state_ref,
    outcome);
```

7. 若 `outcome.tag == COMPLETE`，从 effect frame 读 delimiter answer transport，写入调用方 answer slots
8. 若 `outcome.tag == PROPAGATE`，则：
   - `outcome.signal.resume_token` 已经是 resumed body 本次 outward suspend 产生的 fresh token
   - 不再经过 `pending_continuation` TLS scope
   - 不再生成 replay-state object

这一步直接替代：

1. `scoop_continuation_try_resume`
2. `scoop_continuation_resume_common`
3. `scoop_continuation_resume_with`
4. `scoop_continuation_resume_into`
5. `scoop_continuation_resume`
6. `scoop_continuation_resume_u64`
7. `scoop_continuation_resume_publish_pending_continuation`
8. `ScoopContinuationResumeScope`
9. `ScoopContinuationResumeReplayState`

### 7. Explicit Resume Token Instead of TLS Callee State

当前 ordinary callee suspend state 通过：

1. `scoop_callee_suspend_state_publish()` 写入 TLS，见 `effect/mod.rs:718-721` 与 `runtime/c/scoop_runtime.c:894-917`
2. 最近 boundary 调用 `scoop_effect_outcome_consume_current()` 把它提升成 `EffectSignal.resume_token`，见 `runtime/c/scoop_runtime.c:840-868`

最终设计不再允许这条 bridge。

最终规则：

1. ordinary callee 一旦 suspend，直接把 suspend-state ref 写入 outgoing `EffectOutcome.signal.resume_token`
2. caller/boundary 如需捕获该 token，直接从 explicit outcome 读取
3. continuation 如需保存它，直接写入 `captured_callee_suspend_state_ref`
4. ordinary callee resumed path 通过 hidden `incoming_resume_token_ref` 接收它

因此删除：

1. `scoop_callee_suspend_state_publish`
2. `scoop_callee_suspend_state_get`
3. `scoop_callee_suspend_state_clear`

### 8. Explicit Outcome Instead of TLS Outcome Bridge

最终设计不再允许：

1. 先写 `__scoop_effect_active`
2. 再写 perform slot / TLS token
3. 再调用 `consume_current()` 物化 outcome
4. 需要时再 `publish()` 回 TLS

最终规则：

1. `perform` / `raise` / outward suspend site 直接构造 `ScoopEffectOutcome`
2. ordinary call boundary / handle dispatch / continuation resume 直接消费 `ScoopEffectOutcome`
3. 不存在 `consume_current()` / `publish()` 桥接函数

这意味着删除：

1. `scoop_effect_outcome_consume_current`
2. `scoop_effect_outcome_publish`

`scoop_effect_trace` 若仍用于诊断，可以作为 runtime-internal 辅助保留，但它不能再承担语义 transport 责任。

### 9. Why No `release_fn` Is Needed Anymore

最终 continuation / effect ctx / handler node 都只包含：

1. traced GC refs
2. 标量字段
3. 代码指针

它们不再拥有：

1. stable handle table entry
2. native heap snapshot
3. runtime sidecar 资源

因此：

1. `gc.rs` 现有的 bitmap-based descriptor 生成已经足够，见 `crates/scoopc/src/llvm/codegen/gc.rs:1260-1330`
2. `release_fn = NULL` 就是正确模型，见 `gc.rs:1306-1316`

换句话说，**移动 continuation 到 codegen 的前提不是“先教 codegen 生成 release_fn”，而是“让 continuation 不再需要 release_fn”。**

## Source Changes Required

### 1. `runtime/c/scoop_runtime.c`

删除或收空以下部分：

1. `ScoopEffectHandlerFrame` / `ScoopCapturedHandlerStack` 当前 raw snapshot 实现，见 `runtime/c/scoop_runtime.c:421-452`, `581-629`
2. TLS `__scoop_effect_handler_stack_top`，见 `runtime/c/scoop_runtime.c:467-468`
3. TLS `__scoop_effect_active` / perform slot 作为语义 source of truth 的路径，见 `runtime/c/scoop_runtime.c:470-477`, `840-886`
4. TLS `__scoop_callee_suspend_state` 语义路径，见 `runtime/c/scoop_runtime.c:479-487`, `894-917`
5. `ScoopContinuationResumeScope` 与 TLS active resume scope，见 `runtime/c/scoop_runtime.c:489-506`
6. `ScoopContinuation` C struct 与其 `trace/release/alloc/discard/resume` 实现，见 `runtime/c/scoop_runtime.c:1112-1820`
7. `ScoopContinuationResumeReplayState`，见 `runtime/c/scoop_runtime.c:1391-1445`

保留：

1. `ScoopValueTransport` / `ScoopEffectSignal` / `ScoopEffectOutcome` 的共享布局定义，若仍有 C 侧需要
2. generic GC / thread / alloc substrate

### 2. `runtime/c/scoop_runtime_api.h`

从公共导出名单删除以下 continuation/effect bridge 符号，见当前列表 `runtime/c/scoop_runtime_api.h:30-69`：

1. 所有 `scoop_continuation_*`
2. `scoop_callee_suspend_state_*`
3. `scoop_effect_handler_stack_*`
4. `scoop_effect_outcome_*`

目标状态：runtime API allowlist 中不再出现 continuation-specific public C ABI。

### 3. `crates/scoopc/src/llvm/codegen/runtime_symbols.rs`

删除 continuation/effect bridge 相关 runtime symbol 常量，包括：

1. `SCOOP_CONTINUATION_ALLOC`
2. `SCOOP_CONTINUATION_DISCARD`
3. `SCOOP_CONTINUATION_RESUME_WITH`
4. `SCOOP_CONTINUATION_SET_CAPTURED_CALLEE_SUSPEND_STATE`
5. `SCOOP_CONTINUATION_RESUME_PUBLISH_PENDING_CONTINUATION`
6. `SCOOP_EFFECT_HANDLER_STACK_*`
7. `SCOOP_EFFECT_OUTCOME_*`
8. `SCOOP_CALLEE_SUSPEND_STATE_PUBLISH`

### 4. `crates/scoopc/src/llvm/codegen/runtime_abi.rs`

改动方向：

1. 删除 `declare_runtime_continuation_*` 一组声明，当前位置见 `runtime_abi.rs:1195-1280`
2. 删除 raw `llvm_effect_handler_frame_type()` 作为 runtime push/pop ABI 的用途，当前位置见 `runtime_abi.rs:58-74`
3. 保留并继续使用 `ScoopEffectOutcome` / `ScoopValueTransport` 的结构 ABI builder，见 `runtime_abi.rs:76-97`
4. 新增 codegen-owned `llvm_continuation_struct_type()`、`llvm_effect_ctx_object_type()`、`llvm_effect_handler_node_type()` 的最终布局
5. 这些 struct builder 仍属于 codegen ABI，不再要求 C runtime 存在对应 public struct

### 5. `crates/scoopc/src/llvm/codegen/gc.rs`

改动方向：

1. 为 `ScoopContinuation` / `ScoopEffectCtx` / `ScoopEffectHandlerNode` 生成 bitmap-based type descriptor
2. 不新增 `release_fn` 生成需求
3. continuation 迁入 codegen 后，`gc.rs:1306-1316` 当前的 `release_fn = NULL` 正好符合最终模型

### 6. `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`

这是最核心的代码生成改动点。

当前相关位置：

1. suspend terminator 分配 continuation：`state_machine_emitter.rs:2756-2867`
2. handle 入口注册 handler frames：`state_machine_emitter.rs:4848-4910`

最终改动：

1. suspend path 不再调用 `scoop_continuation_alloc`
2. 直接生成 `scoop_alloc_typed + field stores`
3. 不再调用 `scoop_continuation_set_captured_callee_suspend_state`
4. 不再调用 `scoop_continuation_resume_publish_pending_continuation`
5. handle 入口不再生成 stack `alloca` handler frame + runtime push/pop
6. 改为生成 managed `ScoopEffectHandlerNode` 和 `ScoopEffectCtx`
7. 所有 outward suspend / arm dispatch / continuation materialization 都显式读写 `EffectOutcome`

### 7. `crates/scoopc/src/llvm/codegen/effect/contract.rs`

当前这里仍是 legacy bridge contract 中枢，见 `effect/contract.rs:64-233`。

最终改动：

1. 删除 `begin_legacy_effect_boundary()` / `finish_legacy_effect_boundary()`
2. 删除通过 `top()` / `swap_top()` 捕获当前 effect ctx 的方式
3. 删除通过 `consume_current()` / `publish()` 中转 outcome 的方式
4. 改为显式 local slot 存放 managed `current_effect_ctx_ref` 与 explicit `ScoopEffectOutcome`

### 8. `crates/scoopc/src/llvm/codegen/effect/mod.rs`

当前两处直接暴露 bridge 依赖：

1. `emit_publish_callee_suspend_state` 仍调用 `scoop_callee_suspend_state_publish()`，见 `effect/mod.rs:700-721`
2. `resume_continuation_with_payload()` 仍调用 runtime `scoop_continuation_resume_with()`，见 `effect/mod.rs:2188-2226`

最终改动：

1. ordinary callee suspend path 直接构造 outgoing `EffectOutcome.signal.resume_token`
2. `Continuation.resume(...)` 调用 generated internal helper，而不是 runtime symbol

### 9. Tests

以下测试目前验证了真实语义，但入口绑定在将被删除的 bridge ABI 上：

1. `crates/scoop_runtime/tests/continuation_one_shot.rs:646-809`
2. `crates/scoop_runtime/tests/continuation_cross_thread_handler_stack.rs:100-189`
3. `crates/scoop_runtime/tests/effect_tls.rs:240-349`

最终状态：

1. continuation 语义测试迁到 `scoopc` IR tests + run-pass fixtures + end-to-end GC stress fixtures
2. `effect_tls.rs` 这类 bridge-shape 测试删除或收缩为纯诊断 TLS 测试
3. 不再允许测试直接依赖 deleted continuation/effect bridge C ABI

## Invariants

最终设计必须满足以下不变量：

1. continuation 内部没有 stable handle
2. continuation 内部没有 native `malloc` side resource
3. continuation type descriptor 没有 `release_fn`
4. 所有跨 safepoint / 跨 resume / 跨线程长期存活的数据，都以 traced heap field 或 generated root slot 形式出现
5. `EffectOutcome` 是唯一 propagation source of truth
6. `resume_token` 永远显式存在于 `EffectOutcome.signal.resume_token`
7. 不再存在 continuation-specific runtime TLS 安装/恢复步骤
8. runtime public ABI 中不再出现 continuation bridge 符号

## Validation

### Compiler/IR validation

必须新增或更新 IR 测试，锁定以下事实：

1. generated continuation object 通过 `scoop_alloc_typed` 分配
2. generated continuation type descriptor 只使用 bitmap，不使用 `release_fn`
3. suspend path 直接写 `captured_effect_ctx_ref` / `state_ref` / `captured_callee_suspend_state_ref`
4. IR 中不再出现：
   - `@scoop_continuation_alloc`
   - `@scoop_continuation_resume_with`
   - `@scoop_continuation_set_captured_callee_suspend_state`
   - `@scoop_callee_suspend_state_publish`
   - `@scoop_effect_handler_stack_push`
   - `@scoop_effect_outcome_consume_current`

### End-to-end semantic validation

必须覆盖以下语义矩阵：

1. one-shot double resume
2. resume 后继续外传 effect，且 `resume_token` 仍正确
3. ordinary indirect callee suspend/resume 不再依赖 TLS scratch
4. escaped continuation 在 handle 已返回后仍能通过 captured outer effect ctx 重新命中最近 handler
5. cross-thread resume
6. moving GC + stress + verify-roots

### Runtime validation

runtime 层只需要继续验证 generic substrate：

1. `scoop_alloc_typed`
2. type descriptor tracing
3. thread register/unregister
4. enter_native/leave_native
5. generic stable handle 合同

runtime 不再负责 continuation-specific correctness regression。

## Final Summary

最终收口版的核心不是“把 `struct ScoopContinuation` 从 C 文件搬到 Rust 文件”，而是：

1. 把 continuation 从“runtime-owned shell + side resources”改成“codegen-owned ordinary managed object”
2. 把 handler context 从“stack alloca + raw TLS + native snapshot”改成“managed `EffectCtx` + managed handler node graph”
3. 把 `callee_suspend_state`、`pending_continuation`、`active flag + perform slot` 这些 runtime scratch 中转，改成 explicit hidden ABI 和 explicit `EffectOutcome`

做到这三点后：

1. stable handle 不再是 continuation 内部实现需要
2. handler snapshot 不再需要 native `malloc/free`
3. `release_fn` 不再需要
4. continuation-related code 也就真正从 runtime 收回到了 codegen
