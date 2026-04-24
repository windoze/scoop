# Continuation / EffectCtx / EffectOutcome 设计（T4017 实施基线）

> 状态：`T4017a` 文档收口基线  
> 目标：把 effect / continuation 运行时从“TLS side channel + 若干隐式桥接状态”迁到“显式 `EffectCtx` + 显式 `EffectOutcome`”  
> 边界：本文定义的是后续 `T4017b -> T4017f` 要落地的内部语义、职责分层与迁移顺序；当前代码里仍存在的 TLS 桥接字段只是过渡 transport / scratch，不再被视为权威语义模型。

## 1. 背景

当前实现把 continuation / effect 运行时拆成了两部分：

- 局部控制流状态放进 state machine frame / continuation 对象；
- 动态 effect 上下文与传播槽位主要通过 TLS side channel 承载。

这条路线能工作，但有两个明显问题：

1. 很多普通调用点在返回后都要检查 TLS active / perform slot，即使 callee 实际上不可能向外传播 effect。
2. continuation 的恢复、callee suspend state、pending continuation replay 等逻辑都需要依赖 TLS 做桥接，导致“局部状态”和“动态环境”边界不清楚。

本设计的目标不是改语言 surface，而是重新定义一个更清晰的内部语义与 ABI：

- `EffectCtx*` 表示当前运行时动态 effect 环境；
- `EffectOutcome<R>` 表示本次执行这一步到底是完成了，还是需要向外传播 effect/control signal。

## 2. 设计目标

本设计希望同时满足以下要求：

1. 保持当前语言语义不变：
   - deep、one-shot continuation；
   - `Continuation.resume(...): Answer`；
   - fresh continuation on re-suspend；
   - handler arm、`finally`、cross-thread resume 的语义保持一致。
2. 把 effect propagation 从“隐式 TLS side channel”改成“显式上下文 + 显式 outcome”。
3. 允许把大量“签名上非 `Pure`，但运行时实际不会 outward-effect”的函数重新走纯调用快路径。
4. 让 continuation 捕获与恢复明确地绑定到“捕获的动态 effect 环境”，而不是绑定到“当前线程上碰巧残留的 TLS 状态”。
5. 让 `perform`、`handle`、ordinary effectful call、`Continuation.resume(...)` 落到同一套统一模型下。

## 3. 基本判断

### 3.1 `eff E` 是静态上界，不是运行时承诺

语言层的 `eff E` 表示：

- 这个函数体允许或要求使用某个 effect row；
- 调用它时，caller 的 required effects 需要覆盖该 row。

但这不等于：

- 这个函数每次运行一定会 `perform`；
- 这个函数每次运行一定会 outward-suspend；
- 这个函数必须总是走慢 ABI。

因此需要区分两层事实：

1. `declared_effectful`
   - 签名层面非 `Pure`。
2. `body_may_outward_effect`
   - 经过 whole-function / whole-call-chain 分析后，运行时是否真的可能向外传播 effect 或 suspend。

只有后者才应该决定是否进入 effectful internal ABI。

### 3.2 执行模型是 eager 的

本设计下的 effectful / suspendable function 不是 Rust `async fn` 那种“先构造 future，再由外部 poll 驱动”的模型。

更准确的语义是：

- 调用发生时，函数体立刻开始运行；
- 运行过程中如果一切都能在当前边界内解决，则返回 `Complete(result)`；
- 运行过程中如果遇到当前边界无法自行解决的 effect/control transfer，则返回 `Propagate(signal)`。

因此它更接近 Kotlin `suspend` 的 direct-style eager 调用，而不是 Rust `async fn` 的 lazy future。

### 3.3 `EffectCtx` 不是状态机实例

`EffectCtx` 和 state machine frame 必须分开：

- frame / 状态机实例表示“这一个 computation 自己的局部状态”；
- `EffectCtx` 表示“这一个 computation 当前所处的动态 effect 环境”。

前者回答：

- 我执行到哪了？
- 我有哪些 lifted locals？
- 我恢复时要从哪个 state 继续？

后者回答：

- 我外面现在有哪些 active handler / delimiter？
- 当前 effect/control signal 若向外传播，应该交给谁？

这两者是不同层次的东西。

## 4. 核心抽象

### 4.1 `EffectCtx`

`EffectCtx` 表示 caller 在调用时传给 callee 的运行时动态 effect 环境。

最小概念上，它应该只包含 effect 动态环境，不应把 allocator、GC roots、线程注册等 runtime 基础设施硬塞进去。

一个最小草图：

```c
typedef struct ScoopEffectCtx {
  struct ScoopHandlerLink* handler_top;
} ScoopEffectCtx;
```

设计约束：

- `EffectCtx` 只管“动态 effect 环境”；
- 不负责 GC allocator/cache、native roots、线程注册；
- 可以被局部 `handle` 派生出一个新的子上下文；
- continuation 恢复时捕获和恢复的也是这份上下文或它的快照。

### 4.2 `HandlerLink`

`HandlerLink` 表示动态 handler 链中的一个节点。它至少要表达：

- 处理哪个 `op_tag`；
- effect instance key 的匹配信息；
- 对应的 delimiter / 处理边界；
- outer handler 是谁；
- 本 handler 是 non-resuming 还是 resuming；
- 若需执行 arm / cleanup，运行时或编译器私有地如何进入对应逻辑。

这里不要求 runtime 退化成“解释器式通用 handler 执行器”，只要求它能表达“当前动态环境里有哪些 handler 正在等待 effect signal”。

### 4.3 `ValueTransport`

为了复用现有 `word + gc_ref` transport 思路，可保留一个统一值搬运槽：

```c
typedef struct ScoopValueTransport {
  uint64_t word;
  void* gc_ref;
} ScoopValueTransport;
```

它可用于：

- 普通返回值；
- perform payload；
- continuation resume payload；
- continuation answer。

### 4.4 `EffectSignal`

`EffectSignal` 表示“当前边界无法自行解决，必须交给外层处理”的事件。

一个最小草图：

```c
typedef struct ScoopEffectSignal {
  uint32_t op_tag;
  uint32_t effect_instance_key;
  ScoopValueTransport payload;
  void* resume_token;
} ScoopEffectSignal;
```

其中：

- `payload` 是当前 effect/control 事件携带的数据；
- `resume_token` 表示若外层需要恢复剩余计算，应使用什么对象继续。

`resume_token` 的物理形态不必固定为某一种：

- 可以是 raw continuation；
- 可以是 state machine frame + resume metadata；
- 也可以是更高层包装对象。

### 4.5 `EffectOutcome<R>`

语义层最关键的抽象是：

```text
EffectOutcome<R> =
  | Complete(R)
  | Propagate(EffectSignal)
```

含义：

- `Complete(R)`：本次执行已经在当前边界内正常完成；
- `Propagate(EffectSignal)`：本次执行尚未在当前边界内完成，必须把 signal 交给外层动态环境处理。

注意：

- `Complete` 不意味着“完全没有使用 effect 机制”；
- 它只意味着“当前边界对外已经没有剩余 obligation”。

一个函数可以内部 `handle` 掉若干 effect，最后仍然返回 `Complete(result)`。

### 4.6 `Continuation`

在本设计里，continuation 至少要捕获两部分状态：

1. 局部继续点
   - frame / state machine state；
   - state tag / lifted locals / cleanup progress。
2. 动态环境
   - `captured_ctx` 或等价的 handler context snapshot。

一个概念草图：

```text
Continuation {
  resumed_flag;
  captured_frame;
  captured_ctx;
}
```

因此 continuation capture 的语义不是“记住当前线程 TLS”，而是：

- 记住当前 computation 的局部状态；
- 记住当前 computation 所处的动态 effect 环境。

### 4.7 语义权威边界与过渡 transport

`EffectCtx` / `EffectOutcome` / `EffectSignal` 是后续 compiler/runtime contract 的权威语义模型；它们回答的是：

- 当前 computation 的动态 effect 环境是什么；
- 这一步 eager 执行是已经完成，还是需要继续向外传播；
- 若需要传播，外层究竟要处理什么 signal，以及如何恢复剩余计算。

与之相对，当前实现里仍存在的一些 TLS 字段和 helper 只是过渡 transport / scratch：

- `handler stack top`
- `effect_active + perform_slot`
- `callee_suspend_state`
- `pending_continuation`
- `continuation_resume_active`

这些运行时细节在迁移期间仍可继续存在，但它们不再是“effect 语义的 source of truth”。后续任何新路径都不能再把“当前线程 TLS 里碰巧是什么值”当成 continuation / effect 语义定义本身。

## 5. 统一执行模型

### 5.1 ordinary effectful function

普通 effectful function 的语义层签名可写成：

```text
EffectOutcome<R> f(EffectCtx* ctx, Args...)
```

含义：

- `ctx` 是调用发生时 caller 暴露给 callee 的动态环境；
- 调用是 eager 的；
- callee 要么返回 `Complete(R)`，要么返回 `Propagate(signal)`。

### 5.2 resumable state machine step

真正需要保存局部状态跨挂起恢复的 computation，会有显式 frame：

```text
EffectOutcome<Answer> step(
  EffectCtx* ctx,
  Frame* frame,
  ResumePayload payload
)
```

其中：

- `frame` 保存局部状态；
- `ctx` 保存外部动态环境；
- `payload` 是 resume 时送入的值。

### 5.3 `perform`

`perform` 本身不是“去调用某个 step”的同义词。它是当前执行单元内部产生 `EffectSignal` 的一种操作。

其语义更接近：

1. 构造 `signal`；
2. 查看当前活动环境能否处理；
3. 如果当前环境能处理，则在本层 dispatch；
4. 如果当前环境不能处理，则返回 `Propagate(signal)`。

因此：

- 不是所有 `perform` 都会让当前函数对 caller 提前返回；
- 只有当前边界无法自行消化它时，才会 outward-propagate。

### 5.4 `handle`

`handle` 的语义是“在当前 `ctx` 之上临时叠加一个新的 handler 环境，然后执行 body”。

可概念化为：

```text
ctx_local = push_handler(ctx_in, handler)
run body under ctx_local
```

若 body 内产生 signal：

- 若 `ctx_local` 能匹配，则本层处理；
- 若不能匹配，则把 signal 继续向外返回。

因此 `handle` 并不一定需要向 caller 返回一个“未完成态”；它首先尝试在本层消费 signal。

### 5.5 `Continuation.resume(...)`

`Continuation.resume(payload)` 的语义：

1. 检查 one-shot；
2. 取出 `captured_frame` 与 `captured_ctx`；
3. 在 `captured_ctx` 下调用：

```text
step(captured_ctx, captured_frame, payload)
```

然后：

- 若得到 `Complete(answer)`，则 `resume(...)` 返回 `answer`；
- 若得到 `Propagate(signal)`，则把这个 signal 继续向当前外层环境传播。

这里的关键点是：

- cross-thread resume 不依赖 resuming thread 当前的 effect TLS；
- 它只依赖 continuation 自己捕获的 `captured_ctx`。

## 6. “当前动态环境”与“调用点环境”

对 callee 来说，调用时传入的 `ctx` 确实就是它的“当前外层动态环境”。

但要注意一个细节：

- `ctx_in` 是 callee 入口时继承到的动态环境；
- callee 自己内部还可以通过 local `handle` 派生出一个新的 `ctx_local`；
- 真正决定当前 effect/control 事件如何被处理的，是“当前活动环境”，不一定只是最初 caller 传进来的 `ctx_in`。

因此更准确的表述是：

- callee 在继承到的动态环境下 eager 执行；
- 若它自己内部加了 handler，则在派生环境下继续执行；
- 若当前活动环境足以消化某个 signal，则继续本层执行；
- 若当前活动环境无法消化，则返回 `Propagate(signal)`。

## 7. 语义层与物理 ABI 层

语义层最清晰的形式是：

```text
EffectOutcome<R> f(EffectCtx* ctx, Args...)
```

但物理 ABI 不一定真的返回一个泛型 aggregate。

更可行的物理 lowering 形式通常是：

- 返回一个小 tag；
- `Complete` 的值走 out slot / sret；
- `Propagate` 的 signal 走另一组 out slot。

一个可选草图：

```c
uint32_t f(
  ScoopEffectCtx* ctx,
  Args...,
  ScoopValueTransport* out_complete,
  ScoopEffectSignal* out_signal
);
```

其中：

- `0` 表示 `Complete`；
- `1` 表示 `Propagate`。

这样做的原因：

- 避免在 LLVM/C ABI 层到处返回大 aggregate；
- 保留纯值返回、小对象返回、sret 的优化空间；
- 让语义模型与物理 ABI 解耦。

## 8. 与 Rust async / Kotlin suspend 的关系

若必须类比：

- 整体更接近 Kotlin `suspend`；
- 不像 Rust `async fn` 那样先返回 lazy future；
- 真正需要跨挂起保存状态时，又会有显式 frame/state machine，因此底层实现会带一点 Rust async 的状态机味道；
- 但语义上比两者都更接近 algebraic effects / delimited continuation。

可以概括为：

- 调用语义像 Kotlin；
- 局部状态保存形式部分像 Rust；
- effect/control transfer 语义本质上属于 continuation / effect handlers。

## 9. 现有 TLS 状态到新模型的映射

当前运行时里几类关键 TLS 状态，可大致映射如下：

1. `handler stack top`
   - 迁移到 `EffectCtx.handler_top`。
2. `effect_active + perform_slot`
   - 迁移到显式 `EffectOutcome::Propagate(signal)`。
3. `callee_suspend_state`
   - 迁移到 frame / continuation / resume token 内部。
4. `pending_continuation`
   - 迁移到 `EffectSignal.resume_token` 或 fresh continuation。
5. `continuation_resume_active`
   - 不再需要作为全局 TLS 标记；若仍有驱动期 bookkeeping，应局部化到 resume driver。

注意：

- 这里说的是 effect/continuation 相关 TLS；
- GC allocator、thread registration、native roots 等 runtime TLS 仍可独立存在；
- 本设计不要求“一次性消灭仓库中所有 TLS”，但 effect/continuation 相关 TLS 不再保留为语义载体；
- 对这个项目而言，没有为了兼容而保留 effect TLS 的要求；若最终仍有少量 TLS 残留，它们只能承担调试职责。

## 10. 对 latent effect 的处理

本设计明确允许以下场景被优化成纯快路径：

1. 签名是 `eff E`，但 body 完全不 `perform`。
2. higher-order 参数带 effect row，但当前函数体根本不调用该参数。
3. 函数通过 whole-call-chain 分析后，证明不会 outward-effect。
4. `handle` 存在，但 body 根本不会触发对应 effect。

因此内部应至少维护三层分类：

1. `declared_effectful`
   - 静态签名层面的事实。
2. `body_may_outward_effect`
   - whole-function / whole-call-chain 分析后的运行时事实。
3. `needs_resumable_frame`
   - 是否真的需要显式 continuation/state machine frame。

只有第 2 层及以上，才应进入 effectful internal ABI。

## 11. 迁移路径（对应 `T4017a -> T4017f`）

迁移按固定顺序推进，不做长期双轨桥接：

### 阶段 1：`T4017a` 文档收口

- 先统一 `CONTINUATION.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `docs/effect_unified_state_machine.md`；
- 明确 effect propagation 的权威叙事已经转向 `EffectCtx + EffectOutcome`；
- 保证后续实现任务不再以“TLS 是最终语义”作为出发点。

### 阶段 2：`T4017b` 分析分层与 fast path 边界

- 把现有 whole-function `may_suspend` / `known_fun_effects` 分析提升成普通 codegen 可消费的事实；
- 明确区分 `declared_effectful`、`body_may_outward_effect`、`needs_resumable_frame`；
- 让 direct / vtable / closure / funptr / itable call 只在 `body_may_outward_effect = true` 时保留传播检查。

### 阶段 3：`T4017c` 引入显式内部抽象

- 在内部 IR / codegen contract 中引入 `EffectCtx` / `EffectOutcome` / `EffectSignal`；
- 从这一阶段开始，不再新增任何依赖 effect TLS 作为 source of truth 的路径；
- 新增路径直接面向新协议，而不是延续 TLS-only 设计。

### 阶段 4：`T4017d` ordinary effectful call 切到新 ABI

- direct call、closure call、funptr call 先切；
- 让 ordinary effect propagation 不再依赖“call 后查 TLS active/perform slot”；
- 把显式 `ctx + outcome` 变成普通 call-like path 的内部 ABI 主线。

### 阶段 5：`T4017e` continuation replay / resume 状态迁移

- `Continuation.resume(...)` 改为显式消费 `EffectOutcome`；
- `pending continuation`、`callee suspend state`、`resume replay state` 迁移到显式 signal / frame / continuation metadata；
- cross-thread resume 完全建立在 captured ctx + captured frame 上，而不是 resuming thread 的 effect TLS。

### 阶段 6：`T4017f` 收尾删除 effect TLS 的主语义职责

- 把 vtable / itable / object init / top-level init / extern thunk 等剩余边界接到新协议；
- handler stack / perform slot 若仍保留，只能作为调试或局部 transport 实现细节；
- 最终把“effect propagation 的 source of truth”统一到 `ctx + outcome`。

## 12. 非目标

本设计当前不试图解决以下问题：

1. 改变语言 surface；
2. 改变 public `Continuation<Resume, Answer, eff E>` 语义；
3. 一次性删除 runtime 中所有 TLS；
4. 在本文中给出最终的 vtable/itable 物理布局；
5. 在本文中固定 `HandlerLink` / `EffectSignal` 的最终内存布局。

## 13. 需要继续确认的问题

后续实现前仍需明确：

1. `HandlerLink` 的最小物理表示；
2. `EffectSignal` 是否需要直接携带 fresh continuation，还是携带更低层 resume token；
3. pure ABI 与 effect ABI 的函数值桥接策略；
4. virtual / interface dispatch 下如何编码 effectful internal ABI；
5. extern/native 边界是否统一走 thunk；
6. `EffectOutcome` 的物理 ABI 是“返回 tag + out slots”还是其它等价形式。

## 14. 一句话总结

本设计把 continuation / effect 运行时从“TLS 隐式 side channel”改为“显式动态环境 + 显式执行结果”：

- `EffectCtx` 表示当前 computation 所处的动态 effect 环境；
- state machine frame 表示当前 computation 自己的局部继续点；
- `EffectOutcome` 表示这一步到底是完成了，还是必须把 signal 向外传播；
- continuation 捕获的是 `frame + ctx`，而不是“当前线程 TLS 的某几个槽位”；
- ordinary effectful call、`perform`、`handle`、`Continuation.resume(...)` 统一落到同一套 eager direct-style 语义之下。
