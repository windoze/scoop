# PIPELINE_GAPS

## 状态更新（2026-05-12）

- 本文件已按当前代码、fixture 与 LLVM IR 单测重写，目标从“历史差距审计日志”改为“当前 live gap 账本 + legacy gap id 映射”。
- `LlvmEmitError::UnsupportedMainBody` / “暂不支持的 main 代码生成节点”已经是 LLVM backend 的通用 unsupported/assertion 桶，不再等价于“当前仍未实现的 feature 列表”。
- 机器可消费的 owner/gap id 仍保留在 `crates/scoopc/src/mir/placeholder_inventory.rs` 与 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`。其中 `PIPELINE_GAPS §...` 继续作为稳定 bucket id；部分 bucket 当前已关闭、改道或仅剩 guard 语义。
- 当前 `UnsupportedMainBody` 症状最集中的模块是 `crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`。其中相当一部分报错反映的是 typed contract 漂移、routing 失配或内部不变量断言，而不是缺少某个 LLVM primitive。
- 当前仍值得当成 live implementation gap 跟踪的主线，主要是以下几类：
- pre-MIR / MIR placeholder 与 handoff contract 仍有少量 open item。
- raw MIR 只覆盖一个受限 lowering 子集；残留 effect/control、`PerformResult`、dynamic call kind 仍依赖更早 verifier 或 routing。
- effect-refactor 主线的剩余缺口集中在 ABI routing、effect-typed callable adapter、cleanup/unwind contract。
- aggregate/composite 相关的 live hole 主要落在 enum 边角布局与 array/composite transport，而不是“完全没有 boxing”。

## 如何阅读

- 状态：`Open` 表示当前仍是 live gap 或高价值 guard bucket。
- 状态：`Partial` 表示主线已收口，但剩余 narrow surface、contract drift 或 residual unsupported 仍存在。
- 状态：`LegacyOnly` 表示源码里仍残留 legacy producer / 遗留分支，但 refactor 计划要求这些路径对 production/refactor pipeline 完全不可达；如果任何真实编译路径还能命中它们，那是实现错误，不是可接受的剩余 gap。
- 状态：`FrontendReject` 表示当前由前端显式拒绝；不是 codegen-stage blocker，但解禁前必须同步补 backend。
- 状态：`Closed/Re-scoped` 表示默认 pipeline 已收口，或问题已改写成更上游/更窄的 contract。
- 状态：`Historical` 表示保留 legacy 编号，但本轮未发现新的 machine-readable owner 或默认 pipeline blocker。
- 若某节写为 `Closed/Re-scoped`，并不表示仓库里已经不存在同名/近似 `UnsupportedMainBody.kind` 字符串；它只表示该编号不应再被理解为当前阶段的 live feature gap。

## 1. HIR / MIR Lowering 缺口

### 1.1 `comptime` block/if/for 语句仍是 Todo

- 状态：`Open`。
- 结论：这些 placeholder 仍由 MIR placeholder inventory 明确追踪，属于进入 runtime MIR 之前必须消除的前置实现项，而不是 LLVM 末端再补救的事项。
- 证据：`crates/scoopc/src/mir/placeholder_inventory.rs:76-101`，`crates/scoopc/src/pipeline/hir_preflight.rs:91-104`。

### 1.2 splice field `value.[field]` 仍是 Todo

- 状态：`Closed/Re-scoped`。
- 结论：静态 splice field 已在 HIR 阶段改写为 resolved member access；动态字段名改为 source diagnostic，不再以 `Todo` 形式漂到后端。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs:3269-3328`。

### 1.3 class literal / annotation class literal 运行期路径不闭合

- 状态：`Historical`。
- 结论：本轮未把它识别为当前默认 pipeline 的独立 codegen-stage owner；若重新开放该表面，应先补 machine-readable inventory 与 source-level contract。

### 1.4 顶层 `val` 在 MIR 中仍是 `Item::Todo`

- 状态：`Open`。
- 结论：`top-level val` 仍在 MIR placeholder inventory 中保留为必须消除的 item placeholder；它不是当前 LLVM lowering 要继续兼容的长期形态。
- 证据：`crates/scoopc/src/mir/placeholder_inventory.rs:67-75`，`crates/scoopc/src/mir/mod.rs:2987-2998`，`crates/scoopc/src/pipeline/mir_stage.rs:1438-1446`。

### 1.5 `typealias`、package-level `comptime if`、`type`、`object` file item 仍是 Todo

- 状态：`Closed/Re-scoped`。
- 结论：`typealias` / nominal `type` / `object` metadata 已有 HIR/MIR-owned roots；package-level `comptime if` 改为必须在 HIR lowering 前裁剪，而不是留给后端处理异常输入。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs:3132-3168`，`crates/scoopc/src/pipeline/mir_stage.rs:128`，`crates/scoopc/src/hir/lower/mod.rs:624`，`crates/scoopc/src/pipeline/hir_preflight.rs:108-117`。

### 1.6 赋值 LHS 只覆盖 local、top-level 和 member access

- 状态：`Closed/Re-scoped`。
- 结论：`lower_assign_stmt(...)` 现在已收紧为只消费 typed place contract；`assign lhs missing local` / `assign lhs lowering pending` 不再存在于 active MIR lowering path。剩余 legacy reason 仅保留在 placeholder inventory、preflight 禁词和 synthetic verifier/materializer 负例里，待 `P1-T02` 一并清理 active residual。
- 证据：`crates/scoopc/src/mir/lower.rs:2385-2476`，`crates/scoopc/src/pipeline/hir_stage.rs:1510-1685`，`crates/scoopc/src/pipeline/mir_stage.rs:488-565`。

### 1.7 callable callee / ctor callee provenance 不完整会生成 Todo

- 状态：`Closed/Re-scoped`。
- 结论：普通 direct/closure/fun-value/ctor 调用与 reflection runtime intrinsic 现在都必须先发布 typed call-site contract，缺失时会在 typed HIR stage 直接报错，而不会再由 `mir/lower.rs` 生成 `call callee lowering pending` / `ctor call lowering pending` / reflection Todo。为避免 compiler-generated helper calls 继续因同一 `CallSite(span)` 互相覆盖，array-builder/vararg builder 合成调用现在也发布可区分的 call span。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs:1510-1685`，`crates/scoopc/src/hir/lower/expr.rs:3712-4065`，`crates/scoopc/src/mir/lower.rs:3965-4707`，`crates/scoopc/src/pipeline/mir_stage.rs:605-731`。

### 1.8 dynamic dispatch callee 拆解失败会生成 Todo

- 状态：`LegacyOnly`。
- 结论：`dispatch callee lowering pending` 仍有源码 producer，但 refactor 主线不应再依赖它。typed dispatch contract 缺失后若还能落到这条分支，属于未完成的 refactor 实现错误。
- 证据：`crates/scoopc/src/mir/placeholder_inventory.rs:184-192`。

### 1.9 `Continuation.resume` 只接受 canonical callee shape

- 状态：`LegacyOnly`。
- 结论：`resume lowering requires canonical callee shape` 仍有 legacy producer，但 refactor 路径不应再使用无 contract 的 legacy resume 分支。任何 reachability 都表示 refactor resume contract 没有完全接管该表面。
- 证据：`crates/scoopc/src/mir/placeholder_inventory.rs:175-183`。

### 1.10 `perform` 缺 typed contract 时生成 Todo terminator

- 状态：`Historical`。
- 结论：本轮未将其视为独立 live bucket；当前相关风险已并入 `§3.1`、`§3.2` 与 `§5.1-§5.4` 的 effect routing / late-lowered contract 问题。

### 1.11 `handle` 缺 typed contract 时生成 Todo terminator

- 状态：`Historical`。
- 结论：与 `§1.10` 相同；当前更重要的是防止残留 `Handle` 形状进入 raw MIR 或 plain callable emission。

### 1.12 `with` copy-update 遗留分支仍能产生 Todo

- 状态：`Closed/Re-scoped`。
- 结论：typed frontend 现在发布显式 copy-update contract；`with_update` 若仍作为 raw 遗留分支残留，应在 preflight/HIR completeness 阶段拦截，而不是作为默认 LLVM gap。
- 证据：`crates/scoopc/src/typecheck/lower.rs:500`，`crates/scoopc/src/pipeline/hir_preflight.rs:111-113`。

## 2. MIR Handoff / Materialization 缺口

### 2.1 refactor direct-style MIR validator 允许普通 Todo 通过

- 状态：`Open`。
- 结论：`unterminated` 仍在 placeholder inventory 中保留为必须由 strict MIR 覆盖的 builder sentinel；production MIR 不应继续依赖“后续阶段一定会修好它”。
- 证据：`crates/scoopc/src/mir/placeholder_inventory.rs:103-111`。

### 2.2 MIR materialization 透传 Todo

- 状态：`Closed/Re-scoped`。
- 结论：materializer 现在会把 materialized statement / rvalue / terminator 的 `Todo` 直接升格为 `MirMaterializeError::MaterializedTodo`；旧“静默透传到 LLVM”结论已过时。
- 证据：`crates/scoopc/src/mir/materialize.rs:1045-1053`，`crates/scoopc/src/mir/materialize.rs:1445-1452`，`crates/scoopc/src/mir/materialize.rs:2063-2070`。

### 2.3 raw MIR codegen 最终拒绝 Todo

- 状态：`Open`。
- 结论：`pass MIR statement todo`、`pass MIR terminator`、`pass MIR rvalue` 仍是 downstream guard bucket。它们的存在是合理的，但 production MIR 不应把这些 guard 当主线能力使用。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1526-1529`，`crates/scoopc/src/llvm/codegen/mir_body.rs:1627-1632`，`crates/scoopc/src/llvm/codegen/mir_body.rs:1801-1804`。

### 2.4 `Return { value: None }` contract 不一致

- 状态：`Open`。
- 结论：raw MIR codegen 仍会把 `Return { value: None }` 降成 declared return type 的默认值；这不是稳定语义，应更早被 verifier 拒绝或被 MIR 显式改写。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1557-1568`。

### 2.5 generic template / MIR root 缺失是 hard error

- 状态：`Open`。
- 结论：这仍是 materialization contract 的 hard error，但它更像“上游根索引缺失”而不是 LLVM backend 实现缺口。
- 证据：`crates/scoopc/src/mir/materialize.rs:257-269`。

### 2.6 effect-row generic direct-call instance 推断依赖 site binding

- 状态：`Historical`。
- 结论：当前默认 pipeline 未把它暴露成新的 codegen-stage blocker；后续若放开更一般的 effect-row use-site surface，应结合 `§7.3` 重新复核 instance key 与 call-site binding。

### 2.7 `TypeKind::Param` 仍可能到达 codegen

- 状态：`Partial`。
- 结论：已修复当前可复现的 materialized direct-call contract drift：codegen 不再把 generic template body 当作 concrete materialized callable，pure direct-call lowering 会把有 concrete instance 的 generic template 路由到 materialized plain MIR call；call-site binding 中仍含 `TypeKind::Param` 时也不得覆盖已经具体化的 fallback FQN。`TypeKind::Param` 在 codegen 侧仍保留为高价值 guard，未来若有新 surface 命中应继续视为上游单态化/typed contract 漏洞。
- 证据：`crates/scoopc/src/llvm/codegen/mod.rs:1851-1947`，`crates/scoopc/src/llvm/codegen/mir_body.rs:94-121`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:1684-1716`，`tests/fixtures/run-pass/generic_fun_recursion.scoop:1-20`。

### 2.8 resume surface 对裸 type param 有特例，普通 source value 没有

- 状态：`Historical`。
- 结论：该问题仍体现出“resume surface 有 erased carrier 例外、普通 source value 没有”的 contract 分裂，但目前不是默认 pipeline 的顶级 blocker；若放开更一般的 generic effect surface，需要连同 `§2.7` 一并收口。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:5809-5856`，`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:5995`。

## 3. Raw MIR LLVM Codegen 缺口

### 3.1 `Handle`、`ResumeUnwind`、`Todo` terminator 不支持

- 状态：`Open`。
- 结论：raw MIR bridge 仍只接受一部分 terminator；残留 effect/control terminator 进入 raw path 依然会失败。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1627-1632`。

### 3.2 `Perform` 不支持 cleanup unwind，且不使用 `resume_target`

- 状态：`Open`。
- 结论：raw MIR `Perform` 仍被视为“应先经 published late-lowered boundary lowering”的非法输入；这反映的是 routing / handoff contract 未闭合，而不是缺一个小的 LLVM helper。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1614-1626`，`crates/scoopc/src/llvm/codegen/mir_body.rs:4565-4580`。

### 3.3 `PerformResult` 在 raw MIR 中返回默认值

- 状态：`Open`。
- 结论：raw MIR `Rvalue::PerformResult` 仍直接返回 target 的默认值；只要这条路径可达，就属于潜在 miscompile。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1749-1753`。

### 3.4 `TypeCheck` / `Cast` raw MIR 不支持

- 状态：`Closed/Re-scoped`。
- 结论：当前受支持的 runtime `is/!is/as/as?` 已有 MIR lowering 和 run-pass 覆盖；该编号不再是默认 pipeline blocker。剩余缺口已缩小到函数类型 / effectful function type cast，见 `§7.2`。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:2365-2549`，`tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop:1-11`，`crates/scoopc/src/pipeline/mir_stage.rs:1043-1065`。

### 3.5 refactor effect-neutral cast/typecheck 支持不完整

- 状态：`Partial`。
- 结论：effect-neutral value primitive 现在能复用同一套 MIR cast/typecheck lowering 处理当前受支持的 runtime-ref surface；剩余未开放的 surface 已由 `§7.2` 前端挡住。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1808-1857`，`crates/scoopc/src/llvm/codegen/mir_body.rs:2365-2549`，`crates/scoopc/src/pipeline/mir_stage.rs:1043-1065`。

### 3.6 `Virtual` / `Interface` / `Resume` call kind raw MIR 不支持

- 状态：`Open`。
- 结论：raw MIR `codegen_mir_call` 仍拒绝这些 call kind；默认主线要么走 plain dispatch / published boundary，要么更早 fail-fast。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:4549-4554`。

### 3.7 `TopLevelRef` raw MIR 不覆盖普通函数引用

- 状态：`Closed/Re-scoped`。
- 结论：默认主线下顶层 callable value / `FunPtr` 已有 run-pass 覆盖；剩余风险只在“raw MIR 仍直接发射未规范化的函数引用”时出现，不再是默认 blocker。
- 证据：`tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop:1-33`。

### 3.8 MIR pattern `is Type` 只支持 ref/string

- 状态：`Partial`。
- 结论：当前 pattern runtime type test 仍要求 subject/target 落在 `Ref` / `String` 语义上；这对当前支持的类/接口 runtime test 够用，但 value-type / function-type pattern 仍未开放。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:4147-4217`。

### 3.9 class ctor raw MIR 不支持 named/default args

- 状态：`Partial`。
- 结论：当前 fixture 主线已经支持 class ctor named/default/delegation；但 backend 仍强依赖 selected ctor + ordered bound args contract，一旦上游 contract 漂移，仍会落到 unsupported。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:5728-5807`，`tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop:1-6`。

### 3.10 默认参数补齐只覆盖有限顶层函数

- 状态：`Partial`。
- 结论：它不再是默认 pipeline 的通用 blocker，但 raw/backend 仍假设“参数绑定已在更早阶段完成”。只要上游没有发布完整 ordered args / binding map，LLVM 仍会用 arity / arg binding guard 报错。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:4624-4649`，`crates/scoopc/src/llvm/codegen/mir_body.rs:7248-7285`。

### 3.11 closure env / capture shape 限制

- 状态：`Partial`。
- 结论：closure env 现在已经支持 tuple env 与标量/ref/aggregate capture 元素，也支持 mutable capture box contract；但 raw path 仍要求 env 是 `Unit` 或 `Tuple`，且必须发布 closure env transport contract。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:6333-6423`。

### 3.12 effect-typed closure/function-value adapter 限制

- 状态：`Open`。
- 结论：这仍是当前最重要的 live gap 之一。plain closure / function-value / `FunPtr` call 在 effect-typed surface 上仍会要求 adapter 或 published boundary；actual outward effect routing 也必须与之协同。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:5285-5375`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:1556-1560`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:1753-1760`。

### 3.13 `StoreMember` continuation route ambiguous 会失败

- 状态：`Open`。
- 结论：这是 upstream MIR contract gap，而不是 LLVM 想当然补上的逻辑；`Ambiguous` route 仍必须在更早阶段拆解或拒绝。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:8328-8403`。

## 4. Aggregate / Enum / Array / Boxing 缺口

### 4.1 tuple/struct 到 `Any`/`Ref` 没有通用装箱

- 状态：`Partial`。
- 结论：generic MIR composite value boxing 现在已经存在；当前真正的 live hole 已转移到 array/composite transport 与若干 enum layout 边角，而不是“完全没有 aggregate boxing”。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:2210-2259`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3025-3028`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3194-3197`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3742-3745`。

### 4.2 enum boxed payload 中 `Unit` field 会失败

- 状态：`Closed/Re-scoped`。
- 结论：boxed payload 现在会把 `Unit` field 写成零值占位，而不是直接失败；该编号不再是 live blocker。
- 证据：`crates/scoopc/src/llvm/codegen/enum_lowering.rs:170-171`。

### 4.3 大整数 enum payload 超过 payload word 会失败

- 状态：`Open`。
- 结论：inline enum payload 仍假设单 word 表示；超过 payload word 的整数 payload 依然 unsupported。
- 证据：`crates/scoopc/src/llvm/codegen/enum_lowering.rs:284-296`。

### 4.4 nested enum / tuple / struct payload 有 unsupported repr

- 状态：`Open`。
- 结论：nested enum 的某些 repr 以及 tuple/struct/non-scalar payload 仍未统一走 boxed path；ctor 与 pattern extraction 两侧都保留 explicit unsupported。
- 证据：`crates/scoopc/src/llvm/codegen/enum_lowering.rs:418-427`，`crates/scoopc/src/llvm/codegen/control_flow.rs:1217-1239`，`crates/scoopc/src/llvm/codegen/control_flow.rs:1364-1372`。

### 4.5 Array get/set 对 composite element 支持不足

- 状态：`Open`。
- 结论：这是当前 aggregate/composite 主线上最清晰的 live gap。refactor `Array.get` / `MutableArray.set` 仍要求 composite transport metadata；缺 metadata 或退回 `u64` 路径时，composite element 仍 unsupported。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3020-3028`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3194-3208`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3742-3745`。

## 5. Effect Refactor / Late-Lowered State Graph 缺口

### 5.1 ABI routing 仍可能按内部 effect/control 形状而非 actual outward effect set 分类

- 状态：`Open`。
- 结论：plain callable emission 仍把残留 `Perform` / `ResumeUnwind` / `Handle` / `Todo` 视为非法输入；这要求 actual outward effect set、handled-effect elimination 与 ABI routing 真正闭合。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:957-963`。

### 5.2 unsupported source classification 被 verifier 放行，lowering 才失败

- 状态：`Closed/Re-scoped`。
- 结论：verifier 现在已经把 `Unsupported` source classification 直接升格为 frontend-style error；旧“晚到 lowering 才炸”结论已过时。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:3101-3111`。

### 5.3 `ResumeUnwind` 只有空 cleanup placeholder 可接受

- 状态：`Partial`。
- 结论：`ResumeUnwind` contract 现在更严格地校验 cleanup state、source slice、origin/resume-state；call-boundary complete 后的 frame-free tail 也会在无 handle、无 resume/runtime-error/composed boundary、无后续 suspend/cleanup 时提前释放 frame root，避免普通返回路径把 internal frame / empty `EffectCtx` 继续暴露给用户可观测 GC。复杂 cleanup/unwind contract 仍未完全泛化。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:3368-3399`，`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:4493-4565`，`tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.scoop:1-31`。

### 5.4 outward-empty callable 不应被路由为 effect-step entry；`main(args)` 是当前症状

- 状态：`Open`。
- 结论：`main(args)` 的真正问题不是“要不要再发明一个 Step argv ABI”，而是 outward-empty callable 不应被错路由到 effect-step entry。当前 effect-step `main` wrapper 仍保留显式报错。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:481-493`。

### 5.5 cross-thread resume 只支持 u64 payload

- 状态：`Closed/Re-scoped`。
- 结论：runtime 现在已有 transport helper，能携带 `{word, gc_ref, payload_ptr}`；runtime GC fixture 也已覆盖 composite/ref payload。旧“只支持 u64 payload”结论已不再准确。
- 证据：`runtime/c/scoop_thread.c:37-40`，`runtime/c/scoop_thread.c:277-327`，`tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop:1-24`。

### 5.6 thread resume 后 non-complete Step 直接 fatal

- 状态：`Closed/Re-scoped`。
- 结论：production 代码不再依赖专门的 `thread_resume_noncomplete_fatal` helper；该策略已由 frontend Pure gate、ordinary runtime fatal terminal 和 unreachable contract 吞掉。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:6923-6936`，`crates/scoopc/src/typecheck/expr/error.rs:512-531`。

### 5.7 当前 P7 默认 refactor blocker 已完成收口（历史记录）

- 状态：`Closed/Re-scoped`。
- 结论：本节保留 purely for legacy id；默认 refactor blockers 不再以这节为当前 owner。

## 6. Spec / Fixture 相关项

### 6.1 `!!` 非空断言仍 expected fail

- 状态：`Closed/Re-scoped`。
- 结论：`!!` 已有 run-pass fixture；该节不再是 live blocker。
- 证据：`tests/fixtures/run-pass/not_null_assert_basic.scoop:1-20`。

### 6.2 runtime `is/as/as?` 在 MIR/refactor path 不闭合

- 状态：`Closed/Re-scoped`。
- 结论：当前受支持的 runtime cast/type-check 主线已通；剩余 function-type cast 被前端显式挡住，见 `§7.2`。
- 证据：`tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop:1-11`，`crates/scoopc/src/llvm/codegen/mir_body.rs:2365-2549`。

### 6.3 runtime reflection 路径 `nameOf<T>()` / `getPlatform()` 缺 codegen lowering

- 状态：`Closed/Re-scoped`。
- 结论：`nameOf<T>()` 与 `getPlatform()` 现在都有 runtime lowering；同时 MIR lowering 侧已删除 `sizeOf` / `nameOf` legacy Todo fallback，只再接受 typed intrinsic contract。`getPlatform()` 仍有 IR 级断言确保不会退回 declaration-only call。
- 证据：`crates/scoopc/src/mir/lower.rs:3965-4023`，`crates/scoopc/src/llvm/codegen/mir_body.rs:2294-2307`，`crates/scoopc/src/llvm/codegen/mir_body.rs:2311-2342`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:1657-1665`，`tests/fixtures/run-pass/name_of_runtime_basic.scoop:1-14`，`tests/fixtures/run-pass/get_platform_runtime_basic.scoop:1-16`，`crates/scoopc/src/llvm/tests.rs:406-430`。

### 6.4 `@Extern` global variable 没有 extern storage/linkage model

- 状态：`Closed/Re-scoped`。
- 结论：`@Extern` global 现在已有 MIR root、external linkage、TLS storage 与“无 initializer” contract；IR 单测已锁定行为。
- 证据：`crates/scoopc/src/llvm/codegen/mod.rs:2981-3025`，`crates/scoopc/src/llvm/tests.rs:434-477`。

### 6.5 interface default method codegen 覆盖需确认

- 状态：`Closed/Re-scoped`。
- 结论：interface default method dispatch 现在已有 run-pass 与 IR 单测，默认 pipeline 不再把它视为候选 gap。
- 证据：`tests/fixtures/run-pass/interface_default_method_dispatch_basic.scoop:1-23`，`crates/scoopc/src/llvm/tests.rs:420-426`。

## 7. 前端暂挡或未来表面

### 7.1 or-pattern 带 binder 被 typecheck 拒绝

- 状态：`FrontendReject`。
- 结论：or-pattern binder 仍由 typecheck 明确拒绝；这不是当前 codegen blocker，但放开前必须补完整 binder/control-flow 语义。
- 证据：`crates/scoopc/src/typecheck/when_pat.rs:89-97`。

### 7.2 function type runtime cast / effectful function type cast 暂不支持

- 状态：`FrontendReject`。
- 结论：这仍是当前最明确的前端挡板之一；默认 pipeline 会在 MIR 之前拒绝 function-type runtime cast。
- 证据：`crates/scoopc/src/pipeline/mir_stage.rs:1043-1065`。

### 7.3 use-site effect row type arg 暂不支持

- 状态：`FrontendReject`。
- 结论：use-site `eff ...` type arg 仍在 type lowering 时被拒绝；materializer / ABI 若要支持该表面，需要一并补 instance key 与 effect args transport。
- 证据：`crates/scoopc/src/typecheck/lower.rs:1364-1367`。

### 7.4 `spawn` / user-facing `join` 是延期表面

- 状态：`Closed/Re-scoped`。
- 结论：最小 `scoop.thread` 表面已经存在并有 run-pass 覆盖；更高层的 executor / structured concurrency 仍可作为未来工作，但本节不再是当前 codegen gap。
- 证据：`tests/fixtures/typecheck/std_thread_api_surface_ok.scoop:1-22`，`tests/fixtures/run-pass/std_thread_basic.scoop:1-34`，`crates/scoopc/src/llvm/codegen/intrinsics/thread.rs:1-120`。

### 7.5 struct mutable fields 当前被前端限制

- 状态：`FrontendReject`。
- 结论：`struct` 字段 `var` 仍被前端拒绝；放开前仍需要统一 value-type place/store/write-barrier 语义。
- 证据：`crates/scoopc/src/typecheck/structs.rs:35-39`。

### 7.6 GC pin/handle intrinsic surface 仍有限制

- 状态：`Partial`。
- 结论：支持子集已经存在，包含 `GC.pin/unpin` 与 `GC.handleNew/Get/Drop` 的 typed MIR contract 和 LLVM lowering；但更一般的 surface 仍由前端限制，以避免 root/pairing/shape contract 漂移。
- 证据：`crates/scoopc/src/typecheck/expr/error.rs:488-517`，`crates/scoopc/src/pipeline/mir_stage.rs:1975-2060`，`crates/scoopc/src/llvm/codegen/mir_body.rs:3522-3829`。

## 8. 建议收口顺序

1. 先把 pre-MIR / MIR handoff contract 收紧：`§1.1`、`§1.4`、`§2.1`、`§2.3`、`§2.4`、`§2.7` 应继续向更早阶段 fail-fast，避免再把 `UnsupportedMainBody` 当成生产输入校验器。
2. 收口 raw MIR residual effect/control routing：`§3.1`、`§3.2`、`§3.3`、`§3.6` 应保证“要么先转 late-lowered/published boundary，要么 upstream verifier 明确拒绝”。
3. 完成 effect-refactor 的 ABI/routing 主线：优先处理 `§3.12`、`§5.1`、`§5.3`、`§5.4`，确保 actual outward effect set 真正决定 callable ABI。
4. 统一 aggregate/composite transport：优先处理 `§4.3`、`§4.4`、`§4.5`，避免 enum/array/effect payload 各走一套孤立的特殊规则。
5. 保持前端 gate 与 backend 能力同步：`§7.1`、`§7.2`、`§7.3`、`§7.5`、`§7.6` 只要放开其一，就应同步补齐 MIR contract、layout 与 runtime 语义，而不是依赖 `UnsupportedMainBody` 才暴露错误。

## 9. 建议验证矩阵

- 基线：`cargo test --all`。
- fixture 主线：`cargo run -p scoop -- test`。
- GC/runtime 主线：`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`。
- MIR / placeholder / handoff 相关变更：补跑 `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_source_classification_verifier`。
- effect-refactor / cleanup / cross-thread 相关变更：补跑 `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`、`cargo test -p scoopc refactor_llvm_thread_resume_noncomplete_policy`、`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`。
- reflection / extern global / interface default method 相关变更：补跑 `cargo test -p scoopc llvm_tests`，并关注 `getPlatform()`、`@Extern` global、interface default method dispatch 的 IR 断言。
