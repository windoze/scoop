# Effect Refactor

> 状态：讨论草案（已落地 MIR `SiteId` 基础设施）
> 目标：收口 effect lowering、continuation、handler dispatch、row/op specialization 的统一方向。
> 说明：本文尽量自包含；阅读时不应频繁依赖其他设计文档。

> 计划原则：该重构应直接面向目标形态设计，不以“最小 v1”或“先做半套兼容层”为目标。
> 可以一步很大，但不应故意保留一个语义/数据结构上明显会被下一步推翻的中间态。

> 管线原则：所有优化级别必须共用同一条编译管线。
> `O0` / debug build 不允许走单独的 lowering/codegen 通道；它们只能通过更低预算、更保守 facts、
> 或让某些优化 pass 退化为近似 no-op 的方式来实现“更快编译/更低复杂度”。

> 闭包原则：每个阶段都必须向下一阶段输出**语义上闭包**的信息包。
> 下一阶段在做语义判断、lowering 决策或优化决策时，只能依赖：
> (1) 当前阶段的显式输入；(2) 上一阶段显式产出的 facts/schema/table；(3) 明确的外部输入（如 target ABI、优化级别、feature flags）。
> 不允许为了补齐缺失语义而回看 HIR/AST/旧 pass 内部缓存。

## 1. 背景

当前实现里，单个 `handle` 内部的控制流已经部分被编译成状态机，但跨调用链的 effect 传播仍然带有较重的 runtime/TLS 痕迹，例如：

- 动态 handler 上下文依赖 handler stack / handler snapshot 一类运行时结构；
- `perform`/`resume` 的传播合同仍夹杂 bridge/TLS 中转；
- `may_outward` 目前更接近一个布尔门禁，而不是能够直接决定 `step/resume` 形状的精确信息；
- 对高阶函数、closure、间接调用，如果过早物化状态机，会过早冻结 ABI，压缩后续 devirtualization / inlining 的优化空间。

本轮讨论形成的总体方向是：

1. 把最终模型收口成“类似 Kotlin `suspend` 的统一状态机协议”，但中间结果不是单一的 `Suspended`，而是按 effect operation 分支的 richer `Step`。
2. 把 capture 时的 handler 链吸收到 continuation / state-machine 模型本身；只有这条链处理不掉的 residual effect op 才向 caller 暴露。
3. 语言层的 effect 检查仍按函数级 contract 进行，而不是按某个具体 resume/branch 单独做 path-sensitive typing。
4. 对 codegen 真正关键的摘要不应是 `bool may_outward`，而应是“这个具体函数实例可能向外暴露的 effect op 集合”。
5. row specialization 可以并入现有 monomorphization 主线，但 widening 只能发生在同一个 `allowed_row` 家族内部，不能跨不同函数类型做共享。
6. 规范化的动态 ABI 仍应按 `allowed_ops(allowed_row)` 展开成固定 `Step` 形状，但这一步应尽量推迟到 devirtualization / inlining 之后。

## 2. 核心直觉

### 2.1 这不是“不要状态机”，而是“状态机要更完整”

最终模型仍然是状态机，只是不能把状态机狭义理解成“`label + switch` 的函数体本地控制流”。

更完整的理解应当是：

- 状态机负责“从哪里继续执行”；
- continuation / captured chain 负责“继续执行时，尚未被 capture 链吸收的 effect op 应当怎样向外暴露”。

也就是说，capture 时的 handler 链不应再依赖 ambient TLS handler stack；它应当成为 continuation/state machine 图的一部分。

### 2.2 它更像 Kotlin `suspend` 的多分支版本

Kotlin `suspend` 可以粗略看成：

```text
Step<T> = Complete(T) | Suspended(K)
```

这里讨论的 effect 版更像：

```text
Step<T> =
  | Complete(T)
  | Op1(payload1, k1)
  | Op2(payload2, k2)
  | ...
```

这里的关键差异不是“有没有状态机”，而是：

- Kotlin 只有一个统一的 `Suspended` 分支；
- effect 版必须把 outward effect 的分支区分到 operation 级别，因为 payload 类型、handler arm 匹配、以及 `resume` 的参数类型都取决于具体 op。

因此，`Effect<E>` 级别的分支粒度是不够的；至少必须能区分到 `op_tag`。

## 3. 术语

本文对一个已经 materialize 的具体函数实例 `F` 使用如下术语：

### 3.1 `allowed_row(F)`

类型系统层面的函数级 effect contract。它来自 typecheck / inference 后该实例的闭合 effect row。

这是源码层函数类型的一部分。例如：

- `f(): T / E1`
- `f(): T / (E1 + E2)`

这两个函数类型当前语义上不兼容，不能互相替代。因此 `allowed_row` 不能在实现共享时被偷偷放宽。

### 3.2 `allowed_ops(F)`

`allowed_row(F)` 中所有 effect 的 op 全集：

```text
allowed_ops(F) = Ops(allowed_row(F))
```

这是该函数实例在语义上允许向外暴露的 operation 宇宙。

### 3.3 `actual_outward_ops(F)`

根据该函数实例的真实 body 语义，最终可能向 caller 暴露的 op 集合。

它由这些因素共同决定：

- body 内直接 `perform` 的 op；
- 每个 call site 调用到的 callee 实例实际可能 outward 的 op；
- 当前函数内部 `handle` 已经吸收掉哪些 op；
- handler arm body / `finally` / cleanup 自己又向外发出了哪些 op。

它应当满足：

```text
actual_outward_ops(F) ⊆ allowed_ops(F)
```

当 `StepSchema(F)` 已经建立之后，还需要一个更精确的 case 级对应物：

```text
actual_outward_cases(F)
```

它表示：

- 在 `StepSchema(F)` 的全部 canonical cases 中，哪些 case 对该实例真实可能向外暴露；
- 它是当前阶段 `needs_reentry`、site-level dispatch、以及后续 lowering 优化真正直接消费的集合。

两者关系是：

- `actual_outward_cases(F)` 是 authoritative 的 case-level 结果；
- `actual_outward_ops(F)` 可以视为把 `actual_outward_cases(F)` 投影到 op identity 后得到的 op 集。

因此：

- 类型系统/高层说明里继续保留 `actual_outward_ops(F)` 这个术语是自然的；
- 但真正进入 `StepSchema` / `CaseSet` / `needs_reentry` / site facts 的，应是 `actual_outward_cases(F)`。

另一方面，编译器在某次具体构建中真正产出并交给 lowering/codegen 使用的，不一定是完全精确的
`actual_outward_cases(F)`，而是一个允许保守放大的：

```text
resolved_outward_cases(F)
```

它应当满足：

```text
actual_outward_cases(F) ⊆ resolved_outward_cases(F) ⊆ cases(StepSchema(F))
```

其中：

- `actual_outward_cases(F)` 是语义上理想的精确结果；
- `resolved_outward_cases(F)` 是当前优化级别、当前预算、当前 pass 管线实际产出的保守结果；
- lowering、`needs_reentry`、site-level effect facts 应直接消费 `resolved_outward_cases(F)`；
- 在 `O0` / debug build 下，允许直接取 `resolved_outward_cases(F) = cases(StepSchema(F))`，也就是当前 schema 的全集。

### 3.4 `impl_ops(F)`

编译器最终为某个共享实现版本选择的 op 集合。它可以等于 `actual_outward_ops(F)`，也可以在同一 `allowed_row` 内做保守 widening，以减少版本数。

它应当满足：

```text
actual_outward_ops(F) ⊆ impl_ops(F) ⊆ allowed_ops(F)
```

### 3.5 `needs_reentry(F)`

一个独立于 op 集合的 lowering 摘要，用于回答：这个实例是否真的需要 resumable frame / reentry path / materialized state machine。

`needs_reentry` 不能和 `actual_outward_ops` 混为一谈：

- 一个函数可能允许 outward effect，但在当前优化阶段下并不需要单独物化 resumable machine；
- 一个函数也可能在语义上 effectful，但在 devirtualization / inlining 后被完全消去，不再留下独立 ABI 边界。

当前阶段的保守规则现已固定为：

```text
needs_reentry(F) = !resolved_outward_cases(F).is_empty()
```

前提是：

- 该判断发生在 `devirtualization + inlining` 之后；
- 此时高频、体积小、适合消去的 higher-order helper 理应已经尽可能被 inline/devirtualize 掉；
- 因而剩余的 outward effect callable，即便某些 case 在理论上还可以进一步证明“不需要真正 re-enter”，当前阶段也统一按 `needs_reentry = true` 处理；
- `O0` / debug build 仍走同一条分析/优化/late-lowering 管线，只是允许更早、更保守地把
  `resolved_outward_cases` widen 到当前 schema 的全集。

这是一条**保守但正确**的 lowering 规则：

- 只要 `resolved_outward_cases` 没有被完全收窄为空，就先假定需要 resumable/reentry lowering；
- 这样做不会破坏语义正确性；
- 未来若要针对 tail-resume、tail-perform、无状态保存等情形做特化优化，再把 `needs_reentry` 从 `true` 放松到 `false`；这些都属于未来优化场景，不属于本阶段实现范围。

因此，当前阶段 `needs_reentry` 更准确地说是一个“是否需要保守地进入 resumable lowering”的 flag，而不是对“是否存在真正 resumed-body re-entry path”的最精确语义描述。

### 3.6 `may_outward(F)`

`bool may_outward` 不再是主信息源，而应降级为派生量：

```text
may_outward(F) = !impl_ops(F).is_empty()
```

## 4. 已收敛的设计结论

### 4.1 Source-level effect 检查仍按函数粒度进行

尽管不同 suspension site 的实际 outward op 可能不同，当前语义仍然是：

- effect row 检查按函数级 contract 做；
- 每个 call site 只按被调函数的函数级 effect 信息做约束；
- 不要求为某个具体 branch / 某次 resume 单独建立 path-sensitive surface type。

因此，同一个具体函数实例 `F` 的 `step` 返回协议应当是固定的，不应按某个 suspend site 改变 surface type。

### 4.2 真正 authoritative 的 codegen 摘要是 op set，而不是 row 或 bool

对 codegen 来说，最终真正影响 `step/resume` 协议的不是：

- `allowed_row(F)` 本身；
- 也不是一个 `bool may_outward`；

而是 op 级别的 outward 集合。

row 仍然重要，但它主要负责：

- 类型系统中的函数级约束；
- 定义 `allowed_ops(F)` 这个上界。

换句话说：

- 语言层主合同：`allowed_row(F)`
- codegen 主合同：op 级 outward 集合

### 4.3 `Step` 必须至少区分到 op

`Effect<E>` 不够，因为：

- 同一 effect 内不同 op 的 payload 类型可能不同；
- handler arm 是按 op 匹配的；
- continuation 被恢复时的参数类型也可能按 op 不同。

因此，`Step` 至少要能编码：

- 哪个 op 发生了；
- 它的 payload；
- 对应的 continuation。

概念上可以写成：

```text
Step_F<T> =
  | Complete(T)
  | Op1(payload1, k)
  | Op2(payload2, k)
  | ...
```

实际底层表示可以不是字面上的枚举，但语义分支必须至少到 op 级别。

#### 4.3.1 `Step_F` 和 continuation 类型对同一个函数实例固定

这里的关键约束是：

- 对同一个具体函数实例 `F = (symbol, type_args, allowed_row)`，`Step_F` 是固定的；
- 该实例产生的 continuation 类型 `K_F` 也是固定的；
- 某条特定 call chain 上如果能证明“只有 op 子集可达”，那只是可达 case 的收窄，不是函数类型或 continuation 类型的变化。

也就是说，优化事实不能泄漏成新的 surface type。更准确的概念模型应当是：

```text
Step_F<T> =
  | Complete(T)
  | Case0(payload_tuple0, k: K_F)
  | Case1(payload_tuple1, k: K_F)
  | ...
```

其中：

- 分支本身按 op/case 区分；
- `k` 的 surface 类型固定为同一个 `K_F`；
- 不同 call chain 只会影响“哪些 case 实际可能出现”，不会改变 `Step_F` 或 `K_F` 的类型身份。

#### 4.3.2 payload 和 `resume` 参数在 MIR 语义上统一 tuple 化

为了避免在 MIR 层处理不同 op 的参数个数问题，当前方向是：

- effect payload 统一视为一个 tuple；
- `resume` 的参数也统一视为一个 tuple；
- 因此在 MIR 语义层面，`perform` 与 `resume` 都可视为“恰好一个参数”。

约定：

- 0 个参数用 `()`；
- 1 个参数用单元素 tuple；
- 多个参数直接用普通 tuple。

这只是 MIR/中层语义合同，不要求在当前阶段立即冻结具体内存布局；具体布局仍由后续 lowering/codegen 决定。

#### 4.3.3 case identity 应建立在 generic-specialized concrete op 上

这里的“特化”指 generic specialization，而不是 effect-op 子集 specialization。

例如：

- `Raise<String>.raise`
- `Raise<Int>.raise`

应被视为两个不同的 concrete op，因为它们的 effect 身份、payload tuple 类型、以及可匹配的 handler arm 语义都不同。

因此，`Step_F` 的 case identity 应至少区分到 generic-specialized concrete op。

当前方向是：

- 底层身份直接复用现有 monomorphic callable identity，也就是现有 `InstanceKey`；
- 但在 effect/case 相关 API 上，固定使用语义 newtype `ConcreteOpKey(InstanceKey)`，而不是裸露 `InstanceKey`；
- `case_tag` 则是某个 `StepSchema(F)` 内部的本地 ABI 判别值。

特别注意：

- `case_tag` 的编号应对同一个 `StepSchema(F)` 固定；
- 它不应因 `impl_ops` 子集变化而重新编号；
- `impl_ops` 只影响哪些 case 可达/会被具体实现用到，不影响 canonical `Step_F` 的类型或 tag 协议。

#### 4.3.4 类型信息必须显式记在 facts 里，而不是到处回查

仅仅知道 `concrete_op_key` 还不够。为了避免 codegen 再回 HIR/typecheck 到处反查 op 签名和类型替换，effect facts 应显式记录：

- `payload_tuple_ty`
- `resume_tuple_ty`

换句话说，`concrete_op_key` 负责身份，显式类型字段负责 lowering 合同；两者缺一不可。

### 4.4 当前阶段 `needs_reentry` 采用保守定义

当前已决定：在 `devirtualization + inlining` 之后，只要某个 callable instance 仍存在 outward case，
就把它视为需要 reentry / resumable lowering：

```text
needs_reentry(F) = !resolved_outward_cases(F).is_empty()
```

采用这条规则的理由是：

- 该阶段之后，很多只因高阶包装而引入的 outward effect helper 应已被消去；
- 对剩余 callable 而言，把 `needs_reentry` 统一保守地设为 `true` 在正确性上总是安全的；
- 这样可以避免一开始就把 tail-resume、tail-perform、no-state-capture 等优化条件混进 effect facts 主设计里；
- 这些优化以后仍可作为“把某些非空 outward case 从 `needs_reentry = true` 放松成 `false`”的单独后续工作处理，但不属于本阶段实现范围；
- `O0` / debug build 也不需要单独通道，只需让同一条 pass 管线在更低预算/更保守策略下产出更宽的 `resolved_outward_cases`。

这也意味着：

- 当前阶段不再把“`needs_reentry` 是否为真”视为一个完全独立待求解的复杂分析问题；
- authoritative 的语义目标仍然是 `actual_outward_cases`；
- 真正进入 lowering 的是 `resolved_outward_cases`；
- `needs_reentry` 只是基于 `resolved_outward_cases` 的保守 lowering 决策。

### 4.5 capture 链应是状态机/continuation 的一部分，而不是 ambient TLS

正确模型应当是：

- capture 时那条 handler 链被 absorb 进 continuation / state machine 图；
- 该链能够处理的 op，不应再重新掉回 caller site 的 ambient context 去处理；
- 只有 capture 链未处理掉的 residual op 才向 caller 暴露。

这意味着最终模型不应以 TLS handler stack 作为语义前提。

### 4.6 row specialization 可以并入 monomorphization 主线

row specialization 的性质与 monomorphization 类似：

- 它们都基于具体实例键；
- 都需要 reachable-driven materialization、去重、缓存和实例选择；
- 都会影响最终代码形状。

因此，一个自然的 surface 实例键是：

```text
SurfaceInstanceKey = (symbol, type_args, allowed_row)
```

这里故意使用 `allowed_row`，而不是 `actual_outward_ops`，因为函数类型兼容性由 row 决定。

### 4.7 widening 只能发生在同一个 `allowed_row` 家族内部

因为：

- `f(): T / E1` 和 `f(): T / (E1 + E2)` 不是兼容的函数类型；
- 因此实现共享不能通过“把窄 row 的函数偷偷映射到宽 row 的 surface 实例”来完成。

所以 widening 只允许发生在：

```text
actual_outward_ops(F) ⊆ impl_ops(F) ⊆ allowed_ops(F)
```

也就是：

- `allowed_row(F)` 固定不变；
- 变化的只能是同一 row 宇宙内选取哪个 op 子集作为实现版本。

### 4.8 不为内置 effect 或内置类型保留特权 bucket

尽管实际项目里真正常见的 outward effect 可能主要是 `Async`、`Raise` 这类少数 effect，但当前思路不应建立硬编码的预定义 bucket，例如：

- `Pure`
- `Async`
- `Raise`
- `Async + Raise`
- `Full`

更好的策略是：

- 默认按 `resolved_outward_cases` 投影得到的较窄 outward 子集选择版本；
- 当版本数过多、动态边界导致精度丢失、或者共享收益更高时，再在同一 `allowed_row` 内保守 widening。

这能避免给某些 built-in effect 以特殊地位，也更利于未来调整设计或做更高级优化。

### 4.9 动态边界的规范 ABI 应按 `allowed_ops(allowed_row)` 固定

对 closure、function value、vtable/itable、运行期来源不确定的 callee 等动态边界：

- 退化是不可避免的；
- 在编译期拿不到更精确来源时，只能按 signature 看到的 `allowed_row` / `allowed_ops` 进行保守对接。

当前方向是：动态边界的 canonical 形状不应退化成完全 erased 的 `Signal { tag, payload }` 协议，而应直接按 `allowed_ops(allowed_row)` 展开成固定的 `Step` 家族。

进一步地，当前更倾向把它实现成：

- 一个普通的 indirect call / interface call；
- 调用目标是对应的 effectful/state-machine function entry；
- 返回值是固定的 `Step_F`；
- 所需的 callable / interface 类型由编译器内部自动生成，不作为用户可见语法的一部分。

也就是说，dynamic boundary 在语义上不需要单独发明一套“effect 专用调用机制”；它只是普通 icall，只不过返回的是 `Step_F`。

这里还应进一步区分两种 surface：

1. 动态函数值 / callable object
   - 其 canonical dynamic surface 是类似：

```text
invoke(args_tuple) -> Step_F
```

2. continuation object
   - 其 surface 是：

```text
resume(resume_tuple) -> Answer / (Out + Raise<RuntimeError>)
```

   - 内部 lowered 后再对应到统一的 `... -> Step_F<Answer>` 驱动。

这两者都可以在实现层统一成：

- compiler-owned opaque object
- 普通 icall / interface-call
- tuple 输入
- `Step` 返回

但语义上不应混淆：

- 原始动态函数对象暴露的是 `invoke`/`call` surface；
- continuation 暴露的是 `resume` / 内部 reverse-resume-interface surface；
- “实现 resume interface”的说法更适用于 continuation，而不是原始函数值对象本身。

因此，canonical dynamic surface 绑定的也应是：

- 当前 callable type / `allowed_row` 对应的 `StepSchema` 全集；
- 而不是某个 direct/call-chain 分析得出的 `actual_outward_cases` 子集。

当前阶段对 dynamic callable 的实现组织也先固定为保守方案：

- 每个 effectful callable 都生成一个 canonical dynamic entry：

```text
invoke(args_tuple) -> Step_F
```

- 先不额外分裂“optimized direct body”和“canonical dynamic entry”两套不同 surface；
- dynamic callable object 先采用最普通、最易理解的 closure-like 形态，例如概念上的 `env_ptr + fn_ptr`；
- direct/static path 当前也可以直接调用这条 canonical entry，而不是急于再引入第二套更专门的入口协议。

这样做的好处是：

- 语义和实现都更简单；
- 不需要现在就设计额外 adapter/wrapper 层级；
- 因为它仍然是标准的 interface + icall 结构，后续仍保留继续做 devirtualization / inlining 的空间。

但这并不意味着要尽早物化这个 ABI；见下节。

### 4.10 `Step`/状态机的具体物化应当尽量后移

尽管理论上的动态 ABI 是固定 `Step` 形状，但这一步应尽量放到：

- devirtualization 之后；
- inlining 之后；
- 重新分析 `resolved_outward_cases` / `needs_reentry` 之后。

原因很直接：

- `map` / `filter` / `fold` / `run` / `let` / `also` 这类小型高阶函数极常见；
- 它们本身很小，且经常应被 inline 掉；
- 如果过早把它们各自物化成状态机，成本和收益很可能不成比例。

因此更合理的管线是：

- 先保留较高层的 effect/callable 语义；
- 尽量先做 devirtualization + inlining；
- 只对剩余无法被消去的 effectful callable 物化 `Step` ABI 和状态机。

当前阶段在表示层上的推荐也已固定为：

- 在 late lowering 之前，继续使用保留 `perform / handle / resume` 的 direct-style MIR；
- 等 devirtualization / inlining / re-analysis 之后，再统一进入以 `Step_F` 为中心的 late-lowered 形态；
- 这是概念上的“两阶段 MIR”，但当前并不要求立刻拆成两套完全独立的 MIR 数据结构；
- 现阶段更推荐在同一套 MIR 上保留语义节点，并通过完整的 effect facts 驱动 late lowering。

同时，这里还要固定一条实现原则：

- 所有优化级别必须共用同一条 pass/lowering 管线；
- `O0` / debug build 不能切到单独的 effect lowering 通道；
- 不同优化级别的差异只体现在预算、分析精度、以及某些 rewrite/pass 是否因参数而退化成近似 no-op；
- 若某个极低成本的 rewrite 能显著降低后续编译复杂度，`O0` 仍然可以保留它，只要它仍属于同一条管线上的正常 pass 行为。

### 4.11 当前阶段先增强现有 MIR，而不是立即拆出独立 effect-aware MIR

尽管理想中的 effect 中层最终可能会演化为一层更独立的 effect-aware MIR，但当前阶段更合理的路线是：

- 先把 effect/continuation 相关的 authoritative facts 提升进现有 `MaterializedMir` / `pass_view` / summary；
- 让 LLVM codegen 逐步不再回看 HIR 或在 backend 现场重建同类查询；
- 等这些 facts 的形状稳定之后，再判断是否有必要物理拆出独立 IR。

当前判断依据是：

- 现有 MIR 已经承载了显式 CFG、`Call` / `Perform` / `Handle` 等语言级节点；
- 缺少的主要不是“另一层控制流表示”，而是更强的 effect facts；
- 此时如果过早新造一层 IR，容易先增加表示复杂度，而没有先解决“哪些事实应该稳定挂在中层”这个核心问题。

### 4.12 site-level effect facts 应建立在稳定 `SiteId` 之上

为了系统化地把 call-site / perform-site / handle-boundary 上的 effect facts 提升进 MIR，当前方向是：

- 先给 MIR 中的 `Call` / `Perform` / `Handle` 节点分配稳定 `SiteId`；
- `SiteId` 只要求在同一个 `Body` 内唯一；
- lowering 初始分配按构造顺序单调递增；
- 后续 MIR pass 若克隆出新的 site 节点，应为克隆体分配新的 `SiteId`；
- 未来的 site-level side table 可以用 `(callable/body identity, SiteId)` 作为稳定键。

这样做的原因是：

- effect facts 不只存在于函数级，还会落在 block/call-site/handle-boundary/resume-site 上；
- 如果没有稳定 site identity，side table 很容易退回到“靠 span 或 ad-hoc 匹配恢复节点”的脆弱方案；
- 先引入 `SiteId`，后续无论事实先挂 side table，还是未来真的拆成独立 effect-aware MIR，都可以复用这层身份。

当前已落地的最小实现包括：

- MIR `Rvalue::Call` 携带 `site_id`；
- MIR `TerminatorKind::Perform` 携带 `site_id`；
- MIR `TerminatorKind::Handle` 携带 `site_id`；
- `Body` 提供遍历/查询辅助，例如“枚举当前 body 内所有 site id”和“求下一个未使用 site id”；
- lowering 初始分配 `SiteId` 时按 body 内构造顺序单调递增；
- MIR inlining/clone 若复制出新的 `Call` 节点，必须为复制体分配新的 `SiteId`，不能复用原 site 身份；
- MIR dump / `.mir` fixtures 已显式包含这些 `site_id`，作为后续 side table 的稳定锚点。

这里特别强调：

- `SiteId` 只要求在同一个 `Body` 内唯一；
- 它不是全程序全局 id；
- 未来 side table 的自然键应是 `(callable/body identity, SiteId)`。

### 4.13 effect facts 应分层挂载在 function / block / site 上

当前讨论已经收敛到：effect facts 不应只是一份函数级布尔摘要，而应至少分层覆盖：

- function/callable 级；
- block 级；
- site 级。

推荐的分层如下。

#### 4.13.1 function / callable 级

这层适合进入 callable-level effect facts。典型字段包括：

- `declared_row`
- `invoke_args_tuple_ty`
- `allowed_ops`
- `step_schema`
- `resolved_outward_cases`
- `needs_reentry`
- `impl_plan`

其中：

- `step_schema` 给出该实例 canonical `Step_F` 的全部 cases/tag/type；
- `resolved_outward_cases` 给出当前优化级别/当前预算下真正交给 lowering 使用的 case 子集；
- `invoke_args_tuple_ty` 保证 dynamic `invoke(args_tuple) -> Step_F` surface 不必回到 HIR 重建参数形状；
- `impl_plan` 让 late lowering / codegen 直接知道当前实例选中了 `NoOutward` / `SingleCase` / `CanonicalFull` 哪一档；
- 若分析内部还维护一个更理想的 `actual_outward_cases` 目标，它不必直接暴露为 lowering contract 的一部分。

这层回答的是：

- 这个具体 callable instance 的函数级 effect contract 是什么；
- 它的动态 `invoke` surface 输入 tuple 形状是什么；
- 在当前构建参数下，这个实例最终按什么 case 子集向外暴露；
- 该实例 canonical `Step_F` 的 case/tag/schema 是什么；
- 它是否需要 reentry / resumable lowering。

#### 4.13.1a 推荐：effect facts 作为独立 side-table 子系统存在，而不是塞进 `InstanceSummary`

当前推荐不是继续把 effect 相关信息逐项塞进现有通用 `InstanceSummary`，而是直接建立一套独立的
`MaterializedEffectFacts`：

- `InstanceSummary` 继续承载通用优化事实（体积、闭包分配、参数使用、返回 provenance 等）；
- effect 相关的 schema/case/type/site facts 进入专门的 effect-facts 子系统；
- `pass_view` / codegen / 后续 MIR pass 统一通过这套 effect facts 查询，不再把 effect lowering 的关键合同分散在多个 ad-hoc 字段里。

这样做的原因是：

- `StepSchema`、`StepCaseFact`、site-level tuple type、handle 吸收集等信息明显比当前 `InstanceSummary` 的职责更专门；
- 如果继续把这些字段塞进 `InstanceSummary`，会让“通用优化摘要”和“effect ABI/schema”两类职责混杂；
- 本次计划不是做半套过渡，而是直接把 authoritative effect contract 独立出来。

这里再强调一条与上面的“闭包原则”配套的解释：

- “向下一阶段输出完整信息”并不等于把所有字段在每个 site 上重复复制一遍；
- 允许通过 `StepSchemaId`、`CaseTag`、`SiteId`、type-id 等稳定引用，把信息正规化地放在同一个 facts 包内部；
- 但不允许下一阶段为了语义/优化判断再回到 HIR、源码 span、旧查询缓存里重新推断缺失信息。

当前唯一认可的“例外”只有三类：

1. 诊断/调试元信息
   - 例如 source span、原始名字、dump/debug 输出所需的信息；
   - 这些可以额外回溯或单独携带，但不应参与语义或优化决策。

2. 显式外部输入
   - 例如 target ABI、平台信息、优化级别、feature flags；
   - 这些本来就不属于上一阶段 IR/facts 的内容。

3. 同一 facts 包内部的稳定引用
   - 例如 `StepSchemaId -> StepSchema`、`SiteId -> SiteEffectFacts`；
   - 这仍然算“只使用输入参数”，因为这些引用本身就是该阶段输入包的一部分。

除此之外，我不建议保留任何“为了方便实现，后端偶尔回 HIR 看一眼”的特例。

按这个原则，当前认为**最低必要**的一组 effect facts 至少包括：

- `StepSchema`
- `ContinuationSchema`
- `CallableEffectFacts`
- `BlockEffectFacts`
- `CallSiteEffectFacts`
- `PerformSiteEffectFacts`
- `ResumeSiteEffectFacts`
- `HandleSiteEffectFacts`

以及它们之间的稳定 identity：

- `StepSchemaId`
- `ContinuationSchemaId`
- `CaseTag`
- `ConcreteOpKey`
- `SiteId`

如果缺少这组信息中的任何一类，后续 pass 就很容易为了补齐：

- `Step_F` 的完整形状
- continuation 的 `Answer / Out / ResumeTuple`
- `perform` / `resume` / `handle` 的精确 case 对应关系
- dynamic callable 的 `invoke(args_tuple)` surface

而再次回到 HIR 或旧查询缓存里重建语义。

在组织方式上，当前也进一步固定：

- `StepSchema` / `ContinuationSchema` 作为 canonical truth 单独池化；
- callable 级 facts 单独挂在 `InstanceKey` 上；
- block/site facts 则按 body 嵌套组织，而不是在顶层做全局平铺；
- 结构性 rewrite 之后，直接重算受影响 body 的 effect facts，而不是做复杂的增量修补。

这条组织策略的目的，是让：

- schema 真正承担“规范事实”的角色；
- block/site facts 只保存局部结论与 schema 引用；
- facts 的生命周期与某个 materialized MIR snapshot 对齐；
- 下游阶段始终消费完整输入包，而不是修修补补的半增量状态。

#### 4.13.2 block 级

block 级不应携带“声明 row”这类概念，因为 block 本身不是 callable。它更适合挂：

- `ambient_cases`
- `outward_cases`
- 是否包含 suspend / resume / handle boundary

这层回答的是：

- 该 block 处于什么 effect 上下文上界之下；
- 该 block 自己会把哪些 case 向外推；
- 它是否跨越某个 effect/control boundary。

这层也应当进入 facts 包，而不是留到后续 pass 现算；其最自然的键是 `(callable/body identity, BasicBlockId)`。

#### 4.13.3 site 级

site 级 facts 是当前最需要稳定身份锚点的一层。典型站点包括：

- 普通 `Call`
- `Perform`
- `Resume`
- `Handle`

注意：当前 MIR 中 `resume` 仍然是 `Rvalue::Call { kind: CallKind::Resume, ... }` 的一种调用形态，因此它可以继续复用 `Call.site_id` 作为身份；但在 facts 里，`Resume` 仍建议拥有独立的事实变体，而不是和普通 `Call` 混成一类。

这一层适合挂的 facts 包括：

- callee 的 `declared_row`
- 当前 site 使用的 `step_schema`
- 当前 site 已知的更精确 `resolved_cases`
- 精度来源（例如 `precise` / `widened` / `signature_fallback`）
- direct / candidate-set / dynamic-fallback 这类 target mode
- 当前 call/resume 所需的 `invoke_args_tuple_ty` / `continuation_schema`
- 当前 site 对应的 `case_tag`
- 当前 site 的 `payload_tuple_ty`
- 当前 site 的 `resume_tuple_ty`
- `handle` 站点实际吸收的 `handled_cases`
- `resume` 站点恢复后可能 outward 的 `resolved_cases`

这里同样遵循一条原则：类型信息应显式挂在 fact 中或通过显式 schema 引用拿到，而不是要求后端再回到别处二次查询。

未来若先走 side table 方案，这层最自然的键就是 `(callable/body identity, SiteId)`。

### 4.14 该重构按完整目标形态推进，不以“最小 v1”分步冻结中间结构

当前决定是：

- 这不是一个“先做最小可用版、以后再推翻 schema”的计划；
- 数据结构应当直接面向目标形态设计；
- 若某个结构显然只是临时过渡、且下一步一定会被推翻，则不应作为本轮设计目标的一部分。

这条原则具体意味着：

- 不再以 `bool may_outward_effect` 作为未来 effect facts 的中心；
- 不以“先把一些字段临时塞进 `InstanceSummary`”作为目标状态；
- 不以“先做一版不带显式 `StepSchema` 的轻量 effect summary，以后再补”作为推荐路线；
- 直接把 callable/block/site 级 effect facts、`StepSchema`、`StepCaseFact`、`SiteId` 键空间一起设计完整。

### 4.15 当前验证范围

与 `SiteId` 相关的当前验证包括：

- MIR-focused Rust tests 通过；
- `tests/fixtures/mir/**` 的 golden fixtures 已同步并通过；
- `SiteId` 已在 raw dump 中可见，便于后续 effect facts side table 开始直接引用。

这层验证的目标不是证明最终 effect lowering 已完成，而是确保：

- MIR 已经具备稳定 site identity；
- 现有 lowering / inlining / materialization 路径能保留并正确重分配 site 身份；
- 后续 effect facts 上移时不需要再补一次“节点身份基础设施”。

### 4.16 `function -> state machine` 的 late lowering 必须按统一算法进行

当前还要再固定一条原则：对 effectful function 的 state-machine transformation，不允许因为代码形状“看起来简单”就临时分叉成第二套 lowering。

具体来说：

- 所有 effectful function 都必须先进入同一套 effect facts 分析、`ImplPlan` 选择、以及 late-lowering 框架；
- 若某个实例最终被收敛到 `NoOutward`，那也应理解为这套统一框架下得到的退化结果，而不是因为它恰好长得像“简单函数”就绕开主 transformation；
- 一旦某个实例需要物化 effectful ABI / resumable machine，则其 `direct-style body -> state machine` 转化必须使用同一套通用算法；
- 不允许长期保留诸如“单个 `perform` 快路径”“线性 body 专用 lowering”“某种特定 `handle` 形状专用 lowering”“tail-`resume` 专用 lowering”这类按 code shape 分流的第二通道；
- 允许存在的差异只能来自已经显式化的 contract / facts / 外部输入，例如：`resolved_outward_cases`、`ImplPlan`、`StepSchema`、`ContinuationSchema`、site facts、优化级别与预算；
- 不允许因为某段代码恰好匹配某个 ad-hoc pattern，就在语义层切换到另一套 transformation 规则。

换句话说，当前不再把“这个函数长得像不像某个易处理模板”当作 lowering 的主判据；主判据只能是显式 facts，而不是 code shape。

## 5. 概念模型

### 5.1 函数级 contract

对具体函数实例 `F`：

```text
allowed_ops(F) = Ops(allowed_row(F))
actual_outward_ops(F) ⊆ allowed_ops(F)
actual_outward_ops(F) ⊆ impl_ops(F) ⊆ allowed_ops(F)
may_outward(F) = !impl_ops(F).is_empty()
```

其中：

- `allowed_row(F)` 是类型系统/函数类型的语义合同；
- `allowed_ops(F)` 是动态 ABI 的 op 宇宙；
- `actual_outward_ops(F)` 是分析得出的真实 outward op；
- `impl_ops(F)` 是 codegen 选定的具体实现版本摘要。

### 5.2 `Step_F`

概念上，针对具体函数实例 `F`，其动态 ABI 对应一个固定的 `Step_F`：

```text
Step_F<T> =
  | Complete(T)
  | Case(case_tag, payload_tuple, k: K_F)
```

如果采用显式展开的写法，则相当于：

```text
Step_F<T> =
  | Complete(T)
  | Case0(payload_tuple0, k: K_F)
  | Case1(payload_tuple1, k: K_F)
  | ...
```

其中：

- 这些分支的全集由 `allowed_ops(F)` 决定，而不是由某个具体 suspend site 决定；
- `k` 的内部对象类型对同一个 `F` 固定为 `K_F`；
- 不同 case 的差异主要体现在 `case_tag`、`concrete_op_key`、`payload_tuple_ty`、以及该 case 关联的 `ContinuationSchema`。

当前还进一步固定：`Step_F` 的物理表示直接采用编译器内部 `enum`。

也就是说：

- 每个 `StepSchema(F)` 对应一个内部专属 `enum` 类型；
- `Complete` 对应一个 `enum` variant；
- 每个 effect case 也各自对应一个 `enum` variant；
- 若某个 payload/answer 恰好是 `()`，则该 variant 在物理上可以自然退化成零载荷 variant。

之所以直接采用 `enum`，是因为：

- 不需要额外发明新的内部 carrier 结构；
- 语义上与 `StepSchema` 一一对应；
- 可以直接复用语言/后端现有的 `enum` lowering 与优化机会；
- 若未来对 `enum` 有额外优化，`Step_F` 也能自然受益。

更结构化地说，一个函数实例 `F` 应存在一份显式 `StepSchema(F)`：

```text
StepSchema(F) = {
  invoke_args_tuple_ty: ArgsTuple,
  complete_ty: T,
  continuation_obj_ty: K_F,
  cases: [
    { case_tag, concrete_op_key, payload_tuple_ty, continuation_schema },
    ...
  ]
}
```

这里的 `concrete_op_key` 应至少区分到 generic-specialized 的 concrete effect op；其底层表示可以直接复用现有 `InstanceKey`。

### 5.3 continuation

continuation 的更精确 surface 类型尚未最终钉死，但当前共识是：

- 它必须属于某个具体函数实例 `F`；
- 它恢复后暴露给 caller 的 residual surface contract 应当与该实例的函数级 contract 对齐；
- 它携带的上下文不能再依赖 ambient TLS handler stack，而应把 capture 链本身吸收到 continuation/state-machine 图里。

进一步地，当前对 continuation/`resume` 的工作模型是：

- 对同一个 `F`，内部 continuation 对象类型固定为 `K_F`；
- `resume(k, resume_tuple)` 的返回类型固定为 `Step_F<T>`；
- `resume_tuple` 的具体 tuple 类型、`Answer`、以及 outward step schema 由当前 case 关联的 `ContinuationSchema` 给出；
- 在 MIR 语义层面，`resume` 可统一视为“接收一个 tuple 参数”的调用，不必让 MIR 层再关心参数个数。

#### 5.3.1 源码级 `Continuation` 需要可见，但应暴露为接口

当前也已经基本收敛的一点是：源码级 `Continuation` 需要可见。

原因很直接：

- 如果用户要自己构建 executor / reactor / scheduler 一类架构；
- 或者要显式保存、转移、稍后再恢复某个 continuation；

那么语言 surface 上就必须存在一个可以被用户代码持有和传递的 `Continuation` 类型。

但这个 surface 类型不应暴露成编译器生成的 concrete continuation class，而应暴露成一个由编译器拥有的接口/抽象类型：

- 用户可以接收、存储、传递、恢复它；
- 但用户不应自己实现/伪造这个接口；
- 编译器生成的具体 continuation 对象类型继续保持内部私有。

因此，当前方向是：

- 源码层存在可见的 `Continuation<...>` surface；
- 该 surface 在语义上更接近一个 opaque / sealed interface，而不是 concrete type；
- 内部 lowering 仍可自由选择编译器生成的具体对象布局与方法表。

当前推荐的源码级接口形状可以概念上写成：

```text
sealed interface Continuation<in ResumeTuple, out Answer, eff Out> {
  fun resume(value: ResumeTuple): Answer / (Out + Raise<RuntimeError>)
}
```

其中：

- `ResumeTuple` 是当前 continuation 所对应 op 的恢复值 tuple；
- `Answer` 是该 continuation 所属剩余计算的最终答案类型；
- `Out` 是恢复之后仍可能继续 outward 的 residual effect row；它描述源码层 `Continuation<..., eff Out>` 的 effect 参数；
- `Raise<RuntimeError>` 是 `resume(...)` 方法本身额外暴露的 ordinary effect；除非 residual row 本来就包含它，否则它不应被反写进 `Continuation<..., eff Out>` 的 `Out` 参数。

源码层的交互语法当前也已经固定：

- 继续使用普通的方法调用 `k.resume(...)`；
- 不再为 continuation/resume 引入额外 keyword、专用控制流语法或第二套 surface 形式；
- 能用普通方法调用和现有 tuple 语义表达的内容，不再额外引入特殊规则。

也就是说，若某个 continuation 来自：

```text
perform E.op(payload): Resume
```

并且该点之后剩余计算的答案类型是 `Answer`，恢复后残余 row 是 `Out`，那么其源码级类型应接近：

```text
Continuation<ResumeTuple, Answer, eff Out>
```

其 `resume` 的 surface 语义为：

```text
resume(resume_tuple): Answer / (Out + Raise<RuntimeError>)
```

这与内部 lowered 语义：

```text
resume(resume_tuple): Step_F<Answer>
```

是一一对应的，只是源码层仍保持 direct-style `Answer / (Out + Raise<RuntimeError>)`
表示，而 `Out` 本身继续只表示 residual row。

这里还要额外固定一条边界：

- 若内部为了 one-shot 语义给 `resume` 对应的 `Step_F` / `out_step_schema` 保守补入普通 `Raise<RuntimeError>` case，
- 这并不自动意味着源码层 `Continuation<..., eff Out>` 的 `Out` 被扩大；
- `surface_ty` 与 `out_step_schema` 必须分别表达这两层 contract，不能直接互相回写。

当 `ResumeTuple = ()` 时，当前也直接固定提供：

```text
resume()
```

它只是：

```text
resume(())
```

的语法糖。

更进一步，这里不应把它当成 continuation 专用规则，而应提升成一个更一般的调用语法约定：

- 对任意 callable，只要它**恰好接收一个 `Unit` / `()` 参数**；
- 就允许把调用写成零参数形式；
- 其语义等价于显式传入 `()`。

也就是说，统一采用：

```text
f()    ==    f(())
```

前提是该 callable 的参数列表在类型上正好是单一 `Unit` 参数。

这样做的好处是：

- `k.resume()` 不再是 continuation 的特例，而只是一般调用规则的一个实例；
- 语言不需要为 continuation 单独引入特殊语法；
- 调用语法与 `Unit` / 空 tuple 的一般语义保持一致，且几乎没有额外实现成本。

在 codegen/ABI 层，这个规则也不应被理解成“必须真的保留一个物理 `Unit` 参数”。
按当前把 `Unit` / `()` 视为 0-arity tuple 的模型，这个 desugar 只是前端调用归一化：

- 语义上 `f()` 与 `f(())` 等价；
- 但它不应强迫后端额外引入有意义的内存布局或运行时载荷；
- 在合理的 lowered ABI 中，`Unit` / `()` 不应对实际对象布局或参数载荷造成实质影响。

同样地，`Unit` 类型的局部、参数和返回值在源码层仍然存在类型意义，但在 codegen 层通常不必被视为“真实值”：

- `Unit` / `()` 只有唯一值，不携带区分信息；
- 因此 codegen 不必专门 materialize 一个“Unit value”；
- 也不必仅仅为了满足 surface 语法而为 `Unit` 局部、参数或返回值保留独立存储/载荷；
- 它们更像编译期形状上的占位，而不是运行时有内容的对象。

这也意味着：

- `f()` 与 `f(())`
- `k.resume()` 与 `k.resume(())`

在 lowering 之后完全可以共享同一条无额外 `Unit` 载荷的实现路径。

因此，源码层关于 continuation 的核心交互形式已经可以视为完全收敛为：

```text
k.resume(...)
```

其中 `...` 要么是一个普通的 `ResumeTuple` 值，要么在 `ResumeTuple = ()` 时按一般的“单一 `Unit` 参数可写成零参数调用”规则省略。

这条设计同时满足：

- 用户代码可以与 continuation 交互；
- 编译器不必把内部生成类的布局、字段和实现细节泄露到语言 surface；
- 后续 devirtualization / inlining 仍然可以针对编译器已知的具体 continuation 实现类继续工作。

#### 5.3.2 authoritative 的“逆向” resume contract 按 op/case 划分；按 effect 分 interface 只是可选 packing

一个更稳的统一模型是：语义上先按具体 `op` / `case` 建立 reverse resume contract，而不是先把“整个 effect interface”当作唯一主键。

如果把一个 op 概念上写成：

```text
op : PayloadTuple(op) -> ResumeTuple(op)
```

那么对应的“逆向” upcall / resume method 可以概念上写成：

```text
op$ret : ResumeTuple(op) -> Step_F<T>
```

于是：

- outward 方向：`perform op(payload_tuple)`
- inward 方向：`k.op$ret(resume_tuple)`

可以被看成一对相反方向的接口动作。

这意味着：

- `Step_F` 描述 outward request 的 case 集合；
- continuation `K_F` 描述 inward/upcall 的实现对象；
- 每次 inward `resume` / upcall 实际只涉及一个具体 op/case；
- 因而 P5/P6 handoff 中 authoritative 的 identity 应是 `ConcreteOpKey`、`CaseTag`、`ContinuationSchema` 这类 per-op/per-case 稳定键，而不是“整个 effect interface”。

若实现层希望把同一 effect family 的多个 `op$ret` method 打包进一个内部 interface/vtable，也可以。例如，某个函数实例允许 `E1 + E2` 时，continuation 概念上可以视为实现了：

```text
E1$Resume<Step_F<T>> + E2$Resume<Step_F<T>>
```

这里的命名只是说明一种可能的 packing 方式；是否真的在实现中生成名为 `E$Resume` 的接口并不重要。真正需要固定的是：

- 每个 op/case 都有一个内部、编译器控制的 resume method contract；
- 每个 method 的返回类型都统一为同一个 `Step_F<T>`；
- method 的参数类型由对应 case 所关联 `ContinuationSchema.resume_tuple_ty` 决定。
- 若实现中保留“按 effect family 分组的 resume interface”，它也只是 compiler-owned packing / object-layout convenience；下游阶段不能要求先经这层分组才能恢复 per-op 语义。

#### 5.3.3 continuation 是编译器自动生成的内部对象

在 correctness-first 的模型里，continuation 可以被视为一个编译器自动生成的内部对象：

- 它的具体类型对同一个函数实例固定为 `K_F`；
- 它承载当前 `StepSchema(F)` / `allowed_row(F)` 下各个 case 的 internal resume contracts；
- 若实现中保留按 effect family 分组的内部 resume interface，则它可以实现这些 interface，但这只是 packing 方式，不改变 per-op authoritative identity；
- 它的方法体或等价 entry 负责把某次 upcall/resume 重新送回同一个 `Step_F<T>` 协议。

这有两个重要后果：

1. 语义上非常直接
   - `perform` 是 outward request
   - `resume interface` method 是 inward/upcall
   - `Step_F` 固定
   - `K_F` 固定

2. 实现上可以复用普通的对象/interface 优化
   - `k.op$ret(...)` 可以只是普通的 interface/virtual call，或其它 compiler-owned dispatch point
   - 后续可以直接送进 devirtualization、inlining、escape analysis、DCE 等通用 pass

#### 5.3.4 若保留 effect-level resume interface packing，continuation 仍需完整实现该 packing

如果实现选择把同一 effect 的多个 `op$ret` methods 打包成一个内部 resume interface，那么 continuation 为了满足这层 packing/vtable contract，仍应完整实现该 interface。

也就是说：

- 对当前 continuation 所属的那些 effect-level packing interfaces，method 集合应当是完整的；
- 不应因为某个具体 continuation 实例在当前 call chain 上“看起来不会用到某个 op”，就把该 method 从类型上删掉；
- 否则它就不再满足该 packing contract。

但这里还要额外固定一条主次关系：

- authoritative 的完整性来源仍应是当前 `StepSchema` 下对应 effect family 的 case/op 集，以及它们关联的 `ContinuationSchema`；
- 不能反过来把某个 effect-level interface 当成 per-op 语义的唯一 source of truth，再让下游阶段从 interface 分组里倒推 case/schema。

但这并不意味着每个 method 都必须有复杂实现。

对于一个具体 continuation 永远不会被合法调用到的 resume method，当前允许的 correctness-first 做法是：

- 保留该 method 以满足 interface 完整性；
- method body 直接放 `unreachable`；
- 再依靠后续 devirtualization / inlining / DCE 把这些方法或对应 vtable 分量消掉。

代价主要在于：

- 某些 escaping continuation 可能仍需保留完整 vtable；
- 但考虑到绝大多数 effect 的 op 数很少，这个负担通常是可接受的。

#### 5.3.5 这批内部 interface call 值得再送进一轮 devirtualization / inlining

一旦编译器把 continuation materialize 成承载 per-op resume contracts（必要时带 resume-interface packing）的内部对象，就会引入新的一批动态调用点，例如：

```text
k.op$ret(resume_tuple)
```

由于：

- 这些 interface 与实现类都是编译器内部生成、闭世界可见的；
- 构造点和使用点通常距离不远；
- continuation 对象常常不真正逃逸；

它们通常比用户自定义 interface 更容易被 devirtualize。

因此，推荐在这一步之后再跑一轮：

- devirtualization
- inlining
- DCE / escape-driven cleanup

从管线角度看，这一轮优化的意义是：

- correctness-first 的对象/interface模型先保证语义；
- 再让通用优化把它尽可能收敛回更直接的状态机/控制流形态。

#### 5.3.6 哪些部分是源码可见的，哪些仍是编译器内部机制

这里需要区分两层：

1. 源码可见层
   - `Continuation<...>` 作为用户可持有/传递/恢复的接口类型存在。

2. 编译器内部层
   - 前面讨论的 per-op resume contracts，以及可选的内部 `resume interface` packing
   - 编译器生成的具体 continuation object
   - 它们的 method 集、对象头、vtable、字段布局

其中，后者都应理解为：

- 编译器内部使用的 lowered 语义模型；
- 不必也不应作为用户语言层的显式 API 暴露；
- 其具体内存布局、对象头、vtable 形态仍然可以留给更晚的 lowering/codegen 决定。

#### 5.3.7 dropped continuation 表示被放弃的计算

当前语义上已经收敛为：

- 一个 dropped continuation 表示一个被放弃的计算；
- 它剩余的语言级计算都不再执行；
- 这包括任何尚未执行到的 pending `finally` / cleanup block；
- 这更接近“一个 raw continuation 永远不再被 resume”，而不是上层 coroutine cancellation / unwinding 语义。

同时，continuation 捕获到的所有引用都遵循普通 GC 可达性规则：

- 只要 continuation 仍可达，这些对象就按普通对象存活；
- 一旦 continuation 本身不可达，这些对象也按普通 GC 规则回收；
- 不需要 continuation 专用的生命周期 hack。

对于当前语言内部用于少数 unmanaged resource 类型的 `cleanup hook`，应明确与上述语义分层：

- 它不是语言级 surface 的一部分；
- 它不是“继续执行 dropped continuation 剩余计算”的机制；
- 它由 runtime/GC 在对象回收阶段按内部规则处理；
- 对调用时机没有任何语言语义保证，甚至可能永远不被调用；
- 它不得与 GC 发生任何交互，因此不存在 resurrection 一类问题。

因此：

- dropped continuation 的语义仍然是“剩余语言级计算被放弃”；
- `cleanup hook` 只是 GC/runtime 内部机制；
- 二者不应在语义层面混为一谈。

### 5.3.8 Managed ABI 的能力边界与 effect/continuation 无关

当前关于 extern / FFI / Managed ABI 的边界也已经基本明确：

- 当前没有支持 `effectful extern` 的计划；
- Managed ABI 的目标是规范化现有 runtime API，并保证 FFI 与 GC 的正确交互；
- 它是一个“通用但不泛用”的 surface，不负责把语言的全部能力导出到 FFI；
- 它的能力边界是明确的，不应被视为 effect/continuation 机制的一部分。

这意味着：

- FFI 边界不需要 `Step_F` / `StepSchema`；
- FFI 边界不需要 continuation / resume interface；
- FFI 边界不需要 effect context；
- Managed ABI 只处理 managed object 的传入/传出，以及 GC-safe 互操作。

进一步地，错误报告模型也已经明确：

- Managed ABI 不能依赖 effect 返回 runtime error；
- 它必须使用显式错误码与 `Option<ref>` 一类约定来处理和报告错误；
- `ContinuationAlreadyResumed` 一类语言内部运行时错误，不应成为 Managed ABI 的 effect 交互机制。

若 Managed ABI 支持 callback to Scoop，则当前约束也已经固定：

- callback target 必须是 `Pure!`；
- 不考虑经由 Managed ABI 回调到 effectful callable；
- 因而也不需要让 `Step_F` / continuation / effect context 穿过该边界。

这也与现有 spec 中“non-pure effectful function type 不能 cast 到 `Any`”的约束一致。

### 5.3.9 语言内部 runtime error 统一视作普通 effect 分支

当前语义上还应进一步固定：

- `ContinuationAlreadyResumed` 一类语言内部运行时错误，应统一视作普通 effect 分支的一部分；
- 更具体地说，它们在上层语义上应等价于普通的 `Raise<RuntimeError>` 一类 effect 传播；
- 这是正确性的基础，不应在语言层再引入第二套“特殊运行时错误通道”。

这意味着：

- ordinary call boundary
- continuation resume
- handle dispatch
- 以及其它 effect 传播边界

都应当能够以统一的 effect/outcome/`Step_F` 语义处理这类 runtime error。

后续若在 codegen/runtime 层面对这些错误做特化优化，例如：

- 直接内联为 trap-like fast path
- 特定场景下避免完整构造某些对象
- 利用已知不可恢复/不可捕获条件走更短路径

这些都只应被视为**后期实现优化**，不能改变上层语义仍然把它们看作普通 effect 分支这一前提。

### 5.4 推荐的数据结构形态

当前推荐直接面向目标形态设计如下结构。

#### 5.4.1 schema identity

```text
StepSchemaId
ContinuationSchemaId
CaseTag
ConcreteOpKey(InstanceKey)
ImplPlan = NoOutward | SingleCase(CaseTag) | CanonicalFull
```

约束：

- `StepSchemaId` 标识某个具体函数实例 `F` 的 canonical `StepSchema(F)`；
- `ContinuationSchemaId` 标识某个 continuation surface / resume contract 的规范形状；
- `CaseTag` 只在该 schema 内部有意义；
- `ConcreteOpKey` 表示 generic-specialized concrete effect op 的语义身份；其底层持有 `InstanceKey`；
- `CaseTag` 不因 `impl_ops` 子集变化而重新编号。

#### 5.4.2 case/schema

```text
StepCaseFact {
  case_tag,
  concrete_op_key,
  payload_tuple_ty,
  continuation_schema,
}

ContinuationSchema {
  resume_tuple_ty,
  answer_ty,
  out_step_schema,
  surface_ty,
}

StepSchema {
  invoke_args_tuple_ty,
  complete_ty,
  continuation_obj_ty,
  cases: [StepCaseFact],
}
```

约束：

- `cases` 应按稳定顺序存储；
- `continuation_obj_ty` 对同一个函数实例固定为内部 continuation 对象类型 `K_F`；
- `ContinuationSchema` 负责给出源码层 `Continuation<ResumeTuple, Answer, eff Out>` 所需的完整 contract；
- `ContinuationSchema.surface_ty` 的 effect 参数必须继续表示源码层的 residual `Out`，不能仅因为 `resume(...)` 方法类型额外带有 `+ Raise<RuntimeError>`，或因为 `out_step_schema` 为 one-shot 语义保守包含 ordinary `Raise<RuntimeError>` case，就把该 runtime-error 上界反写进 `surface_ty`；
- `ContinuationSchema.out_step_schema` 则负责给出 internal `resume(...) -> Step_F<Answer>` 协议的 canonical step 上界；它允许在不改变 `surface_ty` 的前提下保守携带 compiler-generated one-shot runtime-error case；
- `payload_tuple_ty` 必须显式保存，不依赖外部反查；
- `resume_tuple_ty`、`answer_ty`、`out_step_schema` 则通过 `continuation_schema` 提供。

其中若某个 `payload_tuple_ty` 或某个 `ContinuationSchema.resume_tuple_ty` 恰好是 `()`：

- 该 case/continuation 在类型/事实层仍然显式记录为 `()`，以保持 schema 的统一性；
- 但这不要求后端真的物化一个 `Unit` payload/resume 值；
- codegen 可以把它视为零载荷 case。

#### 5.4.3 case 子集

由于同一个 schema 下只会使用 case 子集，因此应有一个显式“schema + tag 子集”的结构，而不是裸 tag 列表：

```text
CaseSet {
  schema: StepSchemaId,
  tags: [CaseTag],
}
```

约束：

- `tags` 有序、去重；
- 不能把不同 schema 的 `CaseTag` 混在一起；
- `actual_outward_cases`、`handled_cases`、`resolved_cases` 等都应建立在 `CaseSet` 上。

#### 5.4.4 callable-level facts

```text
CallableEffectFacts {
  declared_row,
  invoke_args_tuple_ty,
  step_schema,
  resolved_outward_cases,
  needs_reentry,
  impl_plan,
}
```

说明：

- `declared_row` 保留函数类型层面的语义合同；
- `invoke_args_tuple_ty` 让 dynamic `invoke(args_tuple)` surface 不必回 HIR 恢复参数形状；
- `step_schema` 给出 canonical `Step_F`；
- `resolved_outward_cases` 表示当前构建参数下真正交给 lowering 使用的 outward case 子集；
- `needs_reentry` 在当前阶段按保守规则由 `resolved_outward_cases` 派生：只要该集合非空即为 `true`。
- `impl_plan` 明确当前实例落在 `NoOutward` / `SingleCase` / `CanonicalFull` 哪一档。

若分析内部还维护 `actual_outward_cases` 作为理想精确目标，那它可以是内部分析状态，但不必成为 lowering side table 的必填字段。

这里不再重复保存：

- `complete_ty`
- `continuation_obj_ty`
- 每个 case 的 tuple 类型

因为这些都已经 authoritative 地存在于 `StepSchema` 中。

#### 5.4.5 block-level facts

```text
BlockEffectFacts {
  ambient_cases,
  outward_cases,
  has_suspend_boundary,
  has_handle_boundary,
}
```

说明：

- block 级 facts 反映该 block 在当前 callable 内的 effect/control 边界状况；
- block 不是 callable，因此不保存 `declared_row` 这类函数级 contract。

#### 5.4.6 body-level facts

```text
BodyEffectFacts {
  blocks,
  sites,
}
```

推荐职责：

- `blocks`: `BasicBlockId -> BlockEffectFacts`
- `sites`: `SiteId -> SiteEffectFacts`

推荐组织方式：

- `BlockEffectFacts` 与 `SiteEffectFacts` 不在顶层全局平铺；
- 它们作为某个 materialized body 的局部 effect facts 一起存在；
- 若某个 pass 对 body 做了结构性改写，则直接重算该 body 的 `BodyEffectFacts`；
- 不要求跨 pass 维护复杂的增量修补逻辑。

#### 5.4.7 site-level facts

```text
SiteEffectFacts =
  | Call(CallSiteEffectFacts)
  | Perform(PerformSiteEffectFacts)
  | Resume(ResumeSiteEffectFacts)
  | Handle(HandleSiteEffectFacts)
```

其中 `Resume` 当前仍视为 `CallKind::Resume`，因此可以继续复用 `Call.site_id` 作为身份；但在 facts 中单独列为 `Resume` 变体。

```text
EffectPrecision = Precise | Widened | SignatureFallback

CallTargetMode = KnownInstance | CandidateSet | DynamicFallback

CallSiteEffectFacts {
  kind,
  target_mode,
  invoke_args_tuple_ty,
  target,
  callee_schema,
  resolved_cases,
  precision,
}

PerformSiteEffectFacts {
  emitted_case,
  payload_tuple_ty,
  captured_cont_schema,
}

ResumeSiteEffectFacts {
  continuation_schema,
  resume_tuple_ty,
  answer_ty,
  out_step_schema,
  resolved_cases,
}

HandleArmEffectFacts {
  handled_case,
  payload_tuple_ty,
  continuation_schema,
  arm_outward_cases,
}

HandleSiteEffectFacts {
  result_ty,
  handled_cases,
  body_outward_cases,
  arm_facts: [HandleArmEffectFacts],
  finally_outward_cases,
}
```

说明：

- `CallSiteEffectFacts` 用于表达 direct/dynamic call 在当前 site 的 schema/case 精度；
- `PerformSiteEffectFacts` 指出该 perform 站点实际发出哪个 case，以及它捕获的 continuation schema；
- `ResumeSiteEffectFacts` 让后续阶段不必回 HIR 就知道 `resume` 的输入 tuple、答案类型和 outward schema；
- `HandleSiteEffectFacts` 负责描述当前 handler 吸收哪些 case，以及 arms/finally 再次 outward 什么。

这里的设计取向是：

- 允许字段间通过 `StepSchemaId` / `ContinuationSchemaId` 做正规化引用；
- 但凡是下游高频要直接问的问题，例如 `resume` 的输入/输出 contract、`perform` 捕获的 continuation schema、`handle` 的结果类型，都应显式落在 facts 中；
- 不要求下游再回 HIR 或重新跑类型替换来恢复这些信息。

#### 5.4.8 顶层容器

```text
MaterializedEffectFacts {
  step_schemas,
  continuation_schemas,
  callable_facts,
  bodies,
}
```

推荐职责：

- `step_schemas`: `StepSchemaId -> StepSchema`
- `continuation_schemas`: `ContinuationSchemaId -> ContinuationSchema`
- `callable_facts`: `InstanceKey -> CallableEffectFacts`
- `bodies`: `InstanceKey -> BodyEffectFacts`

其中 `bodies` 的意图是：

- 每个 materialized callable/body 作为一个局部 facts 单元存在；
- 其 block/site facts 不再在顶层跨 body 平铺；
- 这样更符合“facts 绑定 MIR snapshot，并随结构性 rewrite 重算”的生命周期模型。

这里的设计目标是：

- 把 effect ABI/schema/type/site 合同集中在一个 authoritative 容器里；
- 避免让 codegen 再从 HIR、summary、call-site side table 等多个地方拼装 effect lowering 所需事实；
- 让未来是否拆出单独 effect-aware MIR 变成“表示层演化”问题，而不是“effect 合同还没集中下来”的问题。

同时，这里也明确一条生命周期规则：

- `MaterializedEffectFacts` 绑定到当前的 materialized MIR snapshot；
- 一旦某个 pass 对某个 body 做了结构性改写，该 body 对应的 `BodyEffectFacts` 应重算；
- 不要求后续阶段消费一个“部分更新、部分过期”的 facts 容器。

### 5.5 整函数 `direct-style -> state machine` 转化

前面已经固定了 `StepSchema`、`ContinuationSchema`、callable/block/site facts，以及“late lowering 才物化 `Step` ABI”的总方向。当前还需要把缺失的一环说清楚：在 late effect lowering 时，如何把一个具体函数实例 `F` 的整函数 body 统一转成状态机。

这里的目标不是“只把某个 `handle` 的内部改写成局部状态机”，而是：当 `F` 仍需独立存在并进入 effectful lowering 时，把 **整个函数实例** 视为转化对象。现有“单个 `handle` 内部状态机”更应被理解为这个统一过程中的局部子区域，而不是长期并存的另一套 lowering 语义。

#### 5.5.1 转化输入与输出

当前推荐把这一步固定为：

- 输入是当前 materialized MIR snapshot 与 `MaterializedEffectFacts`；
- 这一步只消费已经显式化的 facts/schema/table，不回 HIR/AST/typecheck 内部缓存补语义；
- 输出是某个具体函数实例 `F` 的内部 state-machine 实现形态，它的 outward `Step` 协议由 `StepSchema(F)` 决定，continuation/re-entry 协议由关联的 `ContinuationSchema` 决定；
- 实现层可把它理解为“编译器生成的 state-machine object/frame + 对应 entry/step/resume dispatch 代码”，但这只是物化形态，不影响其必须由统一 transformation 产生这一点。

#### 5.5.2 切分点不是只有 `perform`，而是所有 suspend/dispatch boundary

整函数转化的第一步，是确定 `F` 中哪些位置是必须切开的 boundary。当前应固定：切分点集合不只包括 `perform site`，而应包括所有可能把控制权交给外界、并在之后通过 resume/re-entry 继续的 boundary。

典型包括：

- `perform` site；
- outward cases 非空的 call/invoke site，包括 direct known callee、candidate-set union，以及 dynamic fallback；
- `k.resume(...)` 这类 continuation resume site；
- ordinary runtime error outward 的 boundary；
- 会把 suspension/dispatch 向外层传播的 nested `handle` boundary。

其中还要明确：

- nested `handle` 并不一律强迫外层一起切；
- 若某个 inner handle 能在自身内部完全吸收/闭合 suspend-dispatch 行为，它可以继续作为自洽的内部子机存在；
- 只有那些 `may_suspend_outward` 的 nested handle boundary，才需要被外层状态机当作真正的切分点看待。

#### 5.5.3 转化单位是整个函数 CFG，切分过程按 boundary 递归向外进行

当前应把整函数转化过程固定成一种统一的 region/CFG segmentation 算法：

- 先在函数 body 中定位 boundary；
- 再从这些 boundary 出发，把所在的局部 region 切开；
- 若某个 boundary 位于条件分支、循环、局部 block、表达式求值上下文、或 nested region 内部，则外层 region 也必须继续被显式化并参与切分；
- 这一过程递归向外进行，直到整个函数 body 都被重写成“可编号状态 + 显式边”的形式。

这里可以把“转成 `if/goto` 再切”理解成一种实现手段，但语义上真正要固定的是：

- 每个 boundary 都必须拥有唯一的 owner state；
- 每次 boundary 之后的继续执行位置都必须拥有唯一的 resume state；
- boundary 之间的普通 straight-line 代码形成 state segment；
- 条件、循环、局部返回、以及 nested region 不再依赖源码形状维持控制流，而要被显式记录为 state edge / dispatch / branch。

尤其要强调一个容易遗漏的点：boundary 不一定正好落在 statement 边界上。它也可能处在“尚有未完成求值上下文”的表达式内部，例如：

- 某个 call 的实参求值过程中；
- 某个 `if` 条件或分支表达式内部；
- 某个更大表达式的中间子表达式位置。

这时，未完成的 evaluation context 也必须一起纳入转化：

- 要么被显式拆成额外的 state；
- 要么被改写为 resume 后继续执行的显式后缀路径；
- 不能只支持“boundary 恰好是独立语句”的简单 shape。

#### 5.5.4 每个 boundary 形成统一的“切开-保存-恢复”骨架

对任何一个 boundary，当前都应当使用同一类骨架来理解：

- 进入 boundary 前，当前 state 执行到该点；
- 在 boundary 处，按 `StepSchema` / site facts 决定 outward case、payload、以及 continuation schema；
- 若 outward 发生，则把必要状态保存进 frame/object；
- 若之后发生 resume/re-entry，则从对应的 resume state 继续；
- resume 后的后缀代码仍然作为普通 state segment 执行，直到下一个 boundary 或函数完成。

也就是说，late-lowered body 的核心形状应当统一成：

```text
state_before
  -> boundary(site)
  -> resume_state
  -> next states ...
```

这里的 boundary 可以对应 `perform`、effectful call、`resume`、runtime raise、或 outward nested-handle boundary；但它们都必须落入同一个统一的分段/保存/恢复模型，而不是各自维护互不相干的 code-shape 特判路径。

#### 5.5.5 frame 提升标准是“跨切分点存活并在之后继续被访问的值”

“所有跨切分点存活并访问的 local var 需要提为 state machine class 的 member”这个方向是对的，但这里需要把“值”的范围说得比“源码 local var”更宽。

当前应固定为：凡是跨越某个 boundary、并在其后的 resume/re-entry/cleanup 路径上仍会被读取的值，都必须进入 frame。实现层可以把这些 lifted values 理解为 state-machine class 的 members；在当前文档里，更中性的说法是 frame/object fields。

这类值至少包括：

- 源码级 local var；
- 编译器引入的临时值与中间表达式结果；
- 不同控制流分支在汇合后仍要继续使用的值；
- `handle` arm binder、resume payload、replayed answer/result slot 一类由 boundary 引入的逻辑槽位；
- 状态机本身需要的系统字段，例如 state tag、resume payload carrier、cleanup flag、one-shot flag、completion tag 等。

同时也要明确：

- 是否 lift 的判据是“是否跨 state-machine cut 存活并在之后继续可见/可读”，不是“是否词法上定义在外层 scope”；
- 一个值若不跨任何 boundary live，则它不必仅仅因为函数整体被转成状态机就自动成为 frame field；
- 反过来，一个并非源码具名 local 的中间结果，只要跨切分点存活，也必须按同一规则进入 frame。

#### 5.5.6 `return` / `break` / `continue` / `finally` / cleanup 也属于状态机合同

整函数转化不能只盯着 suspend/resume 本身。为了保持语义闭包，以下控制转移也必须纳入同一个 transformation：

- `return`；
- `break` / `continue`；
- `handle finally` / cleanup；
- handler arm 结束后回到哪个续点；
- dropped continuation 导致的“剩余计算被放弃”。

因此，在 late-lowered 形态里：

- 这些行为都应被表示成显式 state edge、cleanup phase、或 completion path；
- 不能把它们留成“等 emit 时再凭源码结构临时猜”；
- 也不能因为某个函数恰好是线性的、没有循环、或只有一个 `perform`，就绕开这套显式表示。

#### 5.5.7 统一算法优先，shape-specific fast path 只能作为后续优化

当前最后还要再强调一次：这套 transformation 的职责是提供一个对所有 effectful function 一致适用的、语义闭包的 lowering 主模型。

因此：

- 不把“单 `perform`、单 `resume`、tail-`perform`、tail-`resume`、无显式局部、线性 block”这类 code shape 当成另一套 lowering 入口；
- 这类情形如果将来确实值得优化，也只能作为统一 transformation 之后的压缩/消除/特化优化；
- 它们可以让某些状态、frame 字段或 dispatch 分支在后续 pass 中被优化掉；
- 但不能改变“先按统一规则完成整函数 segmentation、frame lifting、boundary lowering，再做优化”的主次关系；
- `O0` / debug build 也必须遵守同一条 transformation 管线，只允许在预算和后续优化力度上更保守，而不是切到另一条 code-shape-driven 通道。

## 6. 计算 `actual_outward_cases` 与 `resolved_outward_cases`

在语义/理想分析层面，真正希望得到的精确结果仍然是：

```text
actual_outward_cases(F)
```

但在实现层面，真正交给 lowering / `needs_reentry` / site facts 使用的是：

```text
resolved_outward_cases(F)
```

两者应满足：

```text
actual_outward_cases(F) ⊆ resolved_outward_cases(F) ⊆ cases(StepSchema(F))
```

`needs_reentry(F)` 不再是一个需要独立求解的第二分析，而只是对 `resolved_outward_cases(F)` 的派生量：

```text
needs_reentry(F) = !resolved_outward_cases(F).is_empty()
```

`actual_outward_ops(F)` 则可以视为把 `actual_outward_cases(F)` 投影到 op identity 后得到的较粗粒度结果。

因此，本节真正讨论的是：

- 如何由函数 body 的结构递归定义理想的 `actual_outward_cases(F)`；
- 如何在同一条 pass 管线内、按当前优化级别和预算，计算出保守的 `resolved_outward_cases(F)`。

当前阶段推荐的具体求解策略已经固定为：

- 只使用当前阶段已有的 facts（`StepSchema`、`CallableEffectFacts`、`BlockEffectFacts`、各类 site facts）；
- 不回 HIR / typechecker / 旧查询缓存补语义；
- 先按 body 计算局部贡献，再在实例调用图上按 SCC/worklist 做统一传播；
- 当预算耗尽时，对受影响实例直接 widen 到 `cases(StepSchema(F))`，而不是继续深挖。

更具体地说，概念上可把每个实例的 outward case 分为两部分：

```text
resolved_outward_cases(F) = local_cases(F) ∪ call_edge_cases(F)
```

其中：

- `local_cases(F)` 来自本地 `perform`、arm/finally outward、以及本地 `handle` 的吸收结果；
- `call_edge_cases(F)` 来自所有调用边传播进来的 outward cases。

概念上可按以下规则理解：

- `perform E.op(...)` 向当前上下文贡献“该 concrete op 对应的 case”；
- `call g::<...>(...)` 向当前上下文贡献被调实例的 outward case 集；
- `handle body with arms`：
  - body 里被当前 `handle` 吸收掉的 case 不再向外暴露；
  - arm body / cleanup / `finally` 再次 outward 的 case 仍然要计入外层；
- 条件分支、循环、nested block、nested handle 都只影响结构组合，不改变“按函数汇总 outward case 集”的基本原则。

对于调用边，当前规则也已经固定：

- direct known callee：并入该 callee 的 `resolved_outward_cases`
- candidate set：并入所有候选 callee 的 `resolved_outward_cases` 并集
- dynamic fallback：直接取当前 callable type / `StepSchema` 的全集

因此，对 indirect call / callable value 的保守回退，不再是单独未定问题，而是这一求解规则的组成部分。

这一步的关键点是：

- per-call-site effect row 只是分析输入；
- 它的作用是帮助求出整个函数实例的最终 outward 结果；
- 它不应直接进入 `step` 的 surface type。

实现策略上，应固定以下原则：

- 所有优化级别共用同一套 SCC/dataflow 管线；
- `O0` / debug build 不能切到单独的“debug effect analysis”通道；
- 不同优化级别的差异只体现在预算和 widening 时机；
- 当预算耗尽、动态边界无法继续精化、或当前优化级别明确要求偏向编译速度时，可直接把
  `resolved_outward_cases(F)` widen 到 `cases(StepSchema(F))`；
- 因此在 `O0` / debug build 下，允许把大量实例直接保守地解析成当前 schema 全集，而不必额外求精。

预算本身当前也建议只使用朴素、可解释的上限，而不引入深度启发式或 profile：

- `max_scc_nodes`
- `max_scc_edges`
- `max_scc_iterations`
- `max_candidate_union_size`

一旦超过预算：

- 不继续尝试更深的精化；
- 直接把受影响实例或整个 SCC widen 到各自 schema 全集；
- 保证结果仍满足 `actual_outward_cases ⊆ resolved_outward_cases ⊆ cases(StepSchema)`。

在当前保守 `needs_reentry` 规则下，问题的复杂度也显著下降：

- 不再需要同时对“outward 集合”和“reentry 精确条件”做双重互递归求解；
- 语义上理想的 fixed-point 目标是 `actual_outward_cases(F)`；
- 编译器真正必须稳定产出的只是一个保守的 `resolved_outward_cases(F)`；
- `needs_reentry` 只是对 `resolved_outward_cases(F)` 做一次非空判定得到的 lowering 派生量。

## 7. 版本与实现共享

### 7.1 surface 实例

源码/类型系统层看到的实例键应以 `allowed_row` 为准：

```text
SurfaceInstanceKey = (symbol, type_args, allowed_row)
```

### 7.2 body 版本

实现层可以进一步区分 body 版本，例如：

```text
BodyVersionKey = (symbol, type_args, allowed_row, impl_ops, needs_reentry)
```

这里保留 `allowed_row` 是为了明确：

- 实现共享不能跨不同 surface 函数类型进行；
- widening 只在同一个 `allowed_row` 家族内部发生。

### 7.3 widening 策略

当前方向是：

当前阶段不采用复杂启发式、profile 或深层挖掘式的版本搜索，而是固定为一个**纯 facts 驱动**的三档方案：

1. `NoOutward`
   - 条件：`resolved_outward_cases = ∅`
   - 含义：当前实例 outward case 已完全消除；
   - 结果：不进入 effectful state-machine 路径，`needs_reentry = false`。

2. `SingleCase(case_tag)`
   - 条件：`resolved_outward_cases` 恰好只包含 1 个 case；
   - 含义：当前实例保守地只会 outward 一个已知 case；
   - 结果：允许生成单 case 的窄版本，避免多分支 case dispatch。

3. `CanonicalFull`
   - 条件：除以上两种外的所有情况；
   - 含义：直接回退到当前 `StepSchema(F)` 的 canonical 全集；
   - 结果：`impl_cases = cases(StepSchema(F))`。

也就是说，当前阶段只保留：

- 空集特化
- 单 case 特化
- 其余一律 canonical full

而不做：

- 任意子集版本
- 小于某阈值就特化的启发式
- profile-guided widening
- 复杂版本共享搜索
- 任何需要额外深挖掘或成本模型调参的方案

这仍满足：

- 不引入内置 effect 特权 bucket；
- 不允许通过 widening 改变源码层的函数类型；
- 版本选择完全由已知 facts 直接推导。

在优化级别上的当前约定是：

- `O0` / debug build：除 `NoOutward` 外，其余非空情况一律使用 `CanonicalFull`；
- 较高优化级别：允许 `SingleCase(case_tag)`；
- 更复杂的子集特化留待后续明确需求时再扩展。

当前实现范围也据此固定：

- 本轮实现只覆盖上述三档规则；
- 不把“未来是否扩展更多子集版本”作为当前待解问题；
- 若将来确有明确收益，再在此基础上扩展，不影响当前 facts/ABI 主设计。

## 8. 推荐编译管线

推荐的高层流程如下：

1. `typecheck / inference`
   - 得到具体函数实例的 `allowed_row`；
   - 函数类型兼容性仍按 row 语义判定。
2. `surface instance materialization`
   - 按 `(symbol, type_args, allowed_row)` 形成 surface 实例。
3. `effect analysis`
   - 在统一管线内建立/更新 `StepSchema`；
   - 以 `actual_outward_cases` 为理想目标，按当前优化级别和预算产出 `resolved_outward_cases`；
   - 由 `resolved_outward_cases` 派生 `needs_reentry`。
4. `devirtualization + inlining`
   - 尽量消掉 closure/function-value 的动态边界；
   - 让小型高阶函数尽可能被内联，不必各自独立物化状态机；
   - 在 `O0` / debug build 下，这些 pass 仍在同一条管线中运行，但可以因预算/参数而退化为近似 no-op。
5. `re-analysis`
   - 在 inlining / devirtualization 之后重新收敛 `resolved_outward_cases` 与 `needs_reentry`。
6. `late effect lowering`
   - 对剩余仍需独立存在的 effectful callable，按 `resolved_outward_cases` 选择当前阶段的三档 `ImplPlan`（`NoOutward` / `SingleCase` / `CanonicalFull`）；
   - 用统一的“boundary segmentation + frame lifting + explicit resume-state”算法把整函数 `direct-style` body 转成状态机，而不是按某些特定 code shape 走专用 lowering；
   - 物化 canonical `Step` ABI；
   - 将 continuation materialize 为承载 per-op resume contract 的对象/entry 形态；若实现中保留按 effect 分组的 internal resume interface，则它只是一层可优化掉的 packing。
7. `post-effect-object devirtualization + inlining`
   - 对新引入的 `k.op$ret(...)` 这类内部 interface call 再跑一轮 devirtualization / inlining / DCE；
   - 尽可能把 correctness-first 的对象/interface模型收敛回更直接的状态机/控制流形态；
   - 这一轮属于 late-lowered representation 上的窄后处理，不重新回到高层 effect 语义分析，也不重新选择 `ImplPlan`。
8. `LLVM / backend emit`
   - direct/static 路径尽量直接命中具体实现版本；
   - 真正无法消掉的动态边界使用按 `allowed_ops(allowed_row)` 固定的 canonical `Step` 家族。

## 9. 明确不采用的方向

当前讨论已经基本排除了以下方向：

- 把 `bool may_outward` 当成最终 authoritative 摘要；
- 只按 effect row 做 `Step` 分支，而不区分具体 op；
- 依赖 ambient TLS handler stack 作为最终语义模型；
- 跨不同 `allowed_row` 共享 surface 实现版本；
- 预定义少数内置 effect bucket 作为主要 widening 策略；
- 在 devirtualization / inlining 之前就尽早冻结所有高阶函数的状态机 ABI。

## 10. 后续计划

当前阶段的核心设计已经基本收敛。下面这些方向不属于本阶段实现范围，但后续可以继续推进：

1. 物理拆分“两阶段 MIR”
   - 当前只在概念上区分 direct-style MIR 与 late-lowered `Step` 形态；
   - 若后续实现压力表明单一 MIR 类型过于拥挤，再考虑物理拆成两套独立表示或单独的 effect-aware MIR。

2. 提升 `needs_reentry` 的精度
   - 在当前保守规则 `!resolved_outward_cases.is_empty()` 之上，后续可加入更精准的放松优化；
   - 典型目标包括 tail-resume、tail-perform、无状态保存等情形。

3. 扩展 `ImplPlan` 超出当前三档
   - 当前只实现 `NoOutward` / `SingleCase` / `CanonicalFull`；
   - 后续若有明确收益，再考虑更细的子集特化、版本共享或代码体积导向的策略。

4. 加强 late-lowered representation 的后处理优化
   - 当前只保留一轮较窄的 post-effect-lowering `devirtualization + inlining + DCE`；
   - 后续可继续增强对 internal interface/icall、continuation object、adapter/wrapper 的收缩能力。

5. 后端与布局优化
   - 继续优化 `Step_F` enum 的布局、零载荷 variant、`Unit` 消除、tag lowering 等；
   - 对 runtime error 的普通 effect 分支语义保持不变，但可在 codegen/runtime 层加入更短路径的 fast path。

6. facts 维护策略的工程优化
   - 当前约定结构性 rewrite 后直接重算对应 body 的 effect facts；
   - 若后续编译成本证明有必要，再考虑更细粒度的局部重算或增量维护，但不能破坏“阶段输出语义闭包”的原则。
