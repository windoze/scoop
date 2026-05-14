# PIPELINE_GAPS

## 状态更新（2026-05-12）

- 本文件已按当前代码、fixture 与 LLVM IR 单测重写，目标从“历史差距审计日志”改为“当前 live gap 账本 + legacy gap id 映射”。
- `LlvmEmitError::UnsupportedMainBody` / “暂不支持的 main 代码生成节点”已经是 LLVM backend 的通用 unsupported/assertion 桶，不再等价于“当前仍未实现的 feature 列表”。
- 机器可消费的 owner/gap id 仍保留在 `crates/scoopc/src/mir/placeholder_inventory.rs` 与 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`。其中 `PIPELINE_GAPS §...` 继续作为稳定 bucket id；部分 bucket 当前已关闭、改道或仅剩 guard 语义。
- 当前 `UnsupportedMainBody` 症状最集中的模块是 `crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`、`crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs`。其中相当一部分报错反映的是 typed contract 漂移、routing 失配或内部不变量断言，而不是缺少某个 LLVM primitive。
- 当前仍值得跟踪的主线与 guard，主要是以下几类：
- pre-MIR / MIR placeholder 主线已收口；当前主要剩 regression guard 与更下游的 contract drift，而不是新的 live lowering gap。
- raw MIR 只覆盖一个受限 lowering 子集；残留 effect/control、`PerformResult`、dynamic call kind 仍依赖更早 verifier 或 routing。
- effect-refactor 主线 ABI routing、effect-typed callable adapter、cleanup/unwind 已收口；对应 gap id 仅保留 guard / drift audit 语义。
- aggregate/composite 主线上的 enum/array boxing、shared descriptor transport、closure env/capture transport 与 cross-thread composite payload reuse 已收口；相关编号现在只剩 guard / frontend gate 语义。

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

- 状态：`Closed/Re-scoped`。
- 结论：runtime `comptime block/if/for` 现在必须在 HIR lowering 时被展开；typed HIR / direct MIR 主路径不再构造 `StmtKind::Todo("comptime_*")`。若 runtime comptime plan 缺失，lowering 会以前置 stage error 失败，而不是把 placeholder 漏到 MIR。
- 证据：`crates/scoopc/src/hir/lower/stmt.rs`，`crates/scoopc/src/hir/lower/placeholder_inventory.rs`，`crates/scoopc/src/pipeline/hir_preflight.rs`，`crates/scoopc/src/mir/placeholder_inventory.rs`。

### 1.2 splice field `value.[field]` 仍是 Todo

- 状态：`Closed/Re-scoped`。
- 结论：静态 splice field 已在 HIR 阶段改写为 resolved member access；动态字段名改为 source diagnostic，不再以 `Todo` 形式漂到后端。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs:3269-3328`。

### 1.3 class literal / annotation class literal 运行期路径不闭合

- 状态：`Historical`。
- 结论：本轮未把它识别为当前默认 pipeline 的独立 codegen-stage owner；若重新开放该表面，应先补 machine-readable inventory 与 source-level contract。

### 1.4 顶层 `val` 在 MIR 中仍是 `Item::Todo`

- 状态：`Closed/Re-scoped`。
- 结论：top-level `val` 现在统一通过 MIR `InitializerRoot` / `ExternGlobal` root model 暴露；`lower_for_dump` 与 direct MIR stage 都直接发布 canonical root，不再把顶层 `val` 变成 `Item::Todo`。
- 证据：`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/mir/mod.rs`，`crates/scoopc/src/pipeline/mir_stage.rs`，`tests/fixtures/mir_refactor/top_level_roots.mir`。

### 1.5 `typealias`、package-level `comptime if`、`type`、`object` file item 仍是 Todo

- 状态：`Closed/Re-scoped`。
- 结论：`typealias` / nominal `type` / `object` metadata 已有 HIR/MIR-owned roots；package-level `comptime if` 改为必须在 HIR lowering 前裁剪，而不是留给后端处理异常输入。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs:3132-3168`，`crates/scoopc/src/pipeline/mir_stage.rs:128`，`crates/scoopc/src/hir/lower/mod.rs:624`，`crates/scoopc/src/pipeline/hir_preflight.rs:108-117`。

### 1.6 赋值 LHS 只覆盖 local、top-level 和 member access

- 状态：`Closed/Re-scoped`。
- 结论：`lower_assign_stmt(...)` 现在已收紧为只消费 typed place contract；`assign lhs missing local` / `assign lhs lowering pending` 已从 active MIR lowering path、active inventory、preflight 禁词和 synthetic verifier/materializer 负例中移除。缺失 contract 时只允许暴露更早阶段诊断或 impossible-state failure。
- 证据：`crates/scoopc/src/mir/lower.rs:2385-2476`，`crates/scoopc/src/pipeline/hir_stage.rs:1510-1685`，`crates/scoopc/src/pipeline/mir_stage.rs:488-565`。

### 1.7 callable callee / ctor callee provenance 不完整会生成 Todo

- 状态：`Closed/Re-scoped`。
- 结论：普通 direct/closure/fun-value/ctor 调用与 reflection runtime intrinsic 现在都必须先发布 typed call-site contract，缺失时会在 typed HIR stage 直接报错，而不会再由 `mir/lower.rs` 生成 `call callee lowering pending` / `ctor call lowering pending` / reflection Todo。相关 legacy reason 也已从 active inventory、preflight 禁词和 synthetic no-Todo 负例中移除。为避免 compiler-generated helper calls 继续因同一 `CallSite(span)` 互相覆盖，array-builder/vararg builder 合成调用现在也发布可区分的 call span。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs:1510-1685`，`crates/scoopc/src/hir/lower/expr.rs:3712-4065`，`crates/scoopc/src/mir/lower.rs:3965-4707`，`crates/scoopc/src/pipeline/mir_stage.rs:605-731`。

### 1.8 dynamic dispatch callee 拆解失败会生成 Todo

- 状态：`Closed/Re-scoped`。
- 结论：dynamic dispatch 现在只通过 typed call-site / member contract lowering；旧的 `callee_fqn.rsplit_once('.')` owner/member 恢复和 `dispatch callee lowering pending` producer 已删除。typed dispatch contract 若缺失，会直接暴露为 impossible-state bug，而不是 `Todo(...)`。
- 证据：`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/pipeline/mir_stage.rs`，`crates/scoopc/src/pipeline_gap_audit.rs`。

### 1.9 `Continuation.resume` 只接受 canonical callee shape

- 状态：`Closed/Re-scoped`。
- 结论：`Continuation.resume` 现在只消费 typed receiver/payload contract；旧的 canonical callee shape fallback 与 `resume lowering requires canonical callee shape` producer 已删除。typed resume contract 若缺失，会直接暴露为 impossible-state bug，而不是 `Todo(...)`。
- 证据：`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/pipeline/hir_stage.rs`，`crates/scoopc/src/pipeline_gap_audit.rs`。

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

### 1.13 array literal synthetic helper call-site identity 会污染元素 call contract

- 状态：`Closed/Re-scoped`。
- 结论：array literal / synthetic vararg builder helper calls 现在会使用独立且稳定的 synthetic call-site span，不再复用元素表达式的用户 span。typed call-site contract 因此不再把 `__scoop_array_builder_push` helper contract 覆盖到元素自身的 enum ctor / direct-call 上；direct MIR 中 `Hit(Point(...))` / `Pair((...))` 会继续保持真实元素形状，而 `§4.4` / `§4.5` 现在只再表示剩余的 composite transport/backend residual。
- 证据：`crates/scoopc/src/hir/lower/mod.rs`，`crates/scoopc/src/hir/lower/expr.rs`，`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`。

## 2. MIR Handoff / Materialization 缺口

### 2.1 refactor direct-style MIR validator 允许普通 Todo 通过

- 状态：`Closed/Re-scoped`。
- 结论：`unterminated` 仍可作为 MIR builder 的局部 sentinel 存在，但它现在必须在 handoff 前被 strict verifier 拒绝；`validate_refactor_direct_style()` 与 materialized MIR 校验都不再允许它穿过 production/materialized 边界。
- 证据：`crates/scoopc/src/mir/mod.rs`，`crates/scoopc/src/mir/materialize.rs`，`crates/scoopc/src/mir/placeholder_inventory.rs`。

### 2.2 MIR materialization 透传 Todo

- 状态：`Closed/Re-scoped`。
- 结论：materializer 现在会把 materialized statement / rvalue / terminator 的 `Todo` 直接升格为 `MirMaterializeError::MaterializedTodo`；旧“静默透传到 LLVM”结论已过时。
- 证据：`crates/scoopc/src/mir/materialize.rs:1045-1053`，`crates/scoopc/src/mir/materialize.rs:1445-1452`，`crates/scoopc/src/mir/materialize.rs:2063-2070`。

### 2.3 raw MIR codegen 最终拒绝 Todo

- 状态：`Closed/Re-scoped`。
- 结论：`pass MIR statement todo`、`pass MIR terminator`、`pass MIR rvalue` 仍保留在 raw MIR codegen 作为最终 impossible-state guard，但 production/materialized MIR 现在必须更早通过 strict verifier / materializer 拒绝 `Todo`、missing root 与 unresolved concrete param。`codegen_gap_inventory.rs` 中的 `§2.3` 仅再表示 upstream contract guard，而不是 live feature gap 或 production blocker。
- 证据：`crates/scoopc/src/mir/mod.rs`，`crates/scoopc/src/mir/materialize.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen_gap_inventory.rs`。

### 2.4 `Return { value: None }` contract 不一致

- 状态：`Closed/Re-scoped`。
- 结论：`Return { value: None }` 现在只允许用于 `Unit` 返回；production MIR 与 materialized MIR 都会拒绝 non-`Unit` 空返回，raw MIR codegen 也不再为它偷偷合成默认值。
- 证据：`crates/scoopc/src/mir/mod.rs`，`crates/scoopc/src/mir/materialize.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`。

### 2.5 generic template / MIR root 缺失是 hard error

- 状态：`Closed/Re-scoped`。
- 结论：materializer 现在会在 template catalog、call-site binding 和 request seeding 阶段把 missing generic template / missing MIR root 统一提升为 `MirMaterializeError::MissingGenericTemplate` / `MissingMirRootForTemplate` 的 source-level hard error；默认主线不再把它们当成 LLVM backend gap。
- 证据：`crates/scoopc/src/mir/materialize.rs`，`crates/scoopc/src/mir/materialize.rs:10324-10479`。

### 2.6 effect-row generic direct-call instance 推断依赖 site binding

- 状态：`Historical`。
- 结论：当前默认 pipeline 未把它暴露成新的 codegen-stage blocker；后续若放开更一般的 effect-row use-site surface，应结合 `§7.3` 重新复核 instance key 与 call-site binding。

### 2.7 `TypeKind::Param` 仍可能到达 codegen

- 状态：`Closed/Re-scoped`。
- 结论：successful materialized MIR 现在会验证并拒绝 frame slot、return、effect row、call target 与 transport metadata 中残留的 concrete-path `TypeKind::Param`；canonical `MaterializedMirPassView` 也只发布 concrete callable/root lookup。codegen 侧剩余的 `TypeKind::Param` 检测只再表示 monomorph miss / impossible-state guard，或 `§2.8` 的 resume-surface 特例，而不再是默认主线的正常结果。
- 证据：`crates/scoopc/src/mir/materialize.rs`，`crates/scoopc/src/mir/pass_view.rs`，`crates/scoopc/src/llvm/codegen/mod.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`tests/fixtures/run-pass/generic_fun_recursion.scoop:1-20`。

### 2.8 resume surface 对裸 type param 有特例，普通 source value 没有

- 状态：`Historical`。
- 结论：该问题仍体现出“resume surface 有 erased carrier 例外、普通 source value 没有”的 contract 分裂，但目前不是默认 pipeline 的顶级 blocker；若放开更一般的 generic effect surface，需要连同 `§2.7` 一并收口。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:5809-5856`，`crates/scoopc/src/llvm/codegen/effect_lowered/layout.rs:5995`。

## 3. Raw MIR LLVM Codegen 缺口

### 3.1 `Handle`、`ResumeUnwind`、`Todo` terminator 不支持

- 状态：`Closed/Re-scoped`。
- 结论：raw MIR route verifier 现在会在 body emission 之前拒绝 `Handle` / `ResumeUnwind`；`Todo` terminator 继续归入 `§2.3` 的 upstream impossible-state guard，而不再以 raw MIR backend unsupported 形态晚到 body emission 才炸。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen_gap_inventory.rs`。

### 3.2 `Perform` 不支持 cleanup unwind，且不使用 `resume_target`

- 状态：`Closed/Re-scoped`。
- 结论：raw MIR `Perform` 现在会在 route gate 处 fail-fast，并明确要求走 published late-lowered boundary；plain/materialized MIR emitter 不再尝试检查 cleanup/resume contract 后再晚期报 unsupported。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen_gap_inventory.rs`。

### 3.3 `PerformResult` 在 raw MIR 中返回默认值

- 状态：`Closed/Re-scoped`。
- 结论：raw MIR `Rvalue::PerformResult` 现在会在 route gate 处被拒绝；原来的 default-value fallback 已删除，因此该 shape 不再以 silent miscompile 形式穿过 body emission。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen_gap_inventory.rs`。

### 3.4 `TypeCheck` / `Cast` raw MIR 不支持

- 状态：`Closed/Re-scoped`。
- 结论：当前受支持的 runtime `is/!is/as/as?` 已有 MIR lowering 和 run-pass 覆盖；该编号不再是默认 pipeline blocker。剩余缺口已缩小到函数类型 / effectful function type cast，见 `§7.2`。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:2365-2549`，`tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop:1-11`，`crates/scoopc/src/pipeline/mir_stage.rs:1043-1065`。

### 3.5 refactor effect-neutral cast/typecheck 支持不完整

- 状态：`Closed/Re-scoped`。
- 结论：effect-neutral value primitive 现在已把默认主线允许的 runtime `is/!is/as/as?` surface 收口为统一的 MIR metadata + LLVM lowering：类/接口/String/参数化 nominal 的 runtime-ref test 可执行，显然不可能的 value/ref 组合会在 MIR metadata 上静态折叠，函数类型 / effectful function-type cast 则继续由 `§7.2` 在前端明确拒绝。因此该编号不再表示“后端半支持”的 partial surface；剩余 `UnsupportedMainBody` 只表达 runtime cast/typecheck metadata 或 runtime-ref contract drift guard。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs:1252-1286`，`crates/scoopc/src/llvm/codegen/mir_body.rs:2651-2896`，`crates/scoopc/src/pipeline/mir_stage.rs:795-1017`，`tests/fixtures/mir_refactor/runtime_typecheck_cast.scoop:1-32`，`tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop:1-16`，`tests/fixtures/typecheck/fn_type_cast_effectful_as{,q}_is_error.scoop:1-15`。

### 3.6 `Virtual` / `Interface` / `Resume` call kind raw MIR 不支持

- 状态：`Closed/Re-scoped`。
- 结论：raw MIR route verifier 现在会在 body emission 之前拒绝 `Virtual` / `Interface` / `Resume` call kind，并把它们统一归类为 dispatch/resume handoff contract 缺失或 route bug，而不是模糊的 backend unsupported。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen_gap_inventory.rs`。

### 3.7 `TopLevelRef` raw MIR 不覆盖普通函数引用

- 状态：`Closed/Re-scoped`。
- 结论：默认主线下顶层 callable value / `FunPtr`、pattern binder 提取出的 callable，以及 `make(1)()` / `choose(mode)()` 这类“调用返回的 callable”都已在 typed HIR + materialized effect-facts handoff 上保留真实 callable surface；剩余风险只允许作为 regression audit，防止 raw MIR 再次晚期重建未规范化函数引用，而不再是默认 blocker。
- 证据：`crates/scoopc/src/typecheck/expr/call.rs`，`crates/scoopc/src/hir/lower/expr.rs`，`crates/scoopc/src/effect_facts/builder.rs`，`tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`，`tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`，`crates/scoopc/src/llvm/tests.rs`。

### 3.8 MIR pattern `is Type` 只支持 ref/string

- 状态：`Closed/Re-scoped`。
- 结论：`when` 的 `is Type` pattern 现在完整覆盖类/接口/String runtime test，以及可静态折叠的 value pattern；其余仍未开放的 dynamic value-type / function-type target 已在前端明确拒绝，不再晚到 LLVM backend unsupported。
- 证据：`crates/scoopc/src/typecheck/when_pat.rs`，`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`，`tests/fixtures/mir_refactor/pattern_is_type.scoop`，`tests/fixtures/typecheck/when_is_pattern_dynamic_value_runtime_test_is_error.scoop`，`tests/fixtures/typecheck/when_is_pattern_function_type_is_error.scoop`，`tests/fixtures/typecheck/when_is_pattern_effectful_function_type_is_error.scoop`。

### 3.9 class ctor raw MIR 不支持 named/default args

- 状态：`Closed/Re-scoped`。
- 结论：class ctor 的 selected ctor + ordered args contract 现在由 upstream handoff 显式冻结；LLVM 不再按 ctor arity 或缺失 binding 猜目标 ctor。direct class ctor、`super(...)` 与 `this(...)` 仍可按已发布 mapping 求值默认值，但不再允许 backend 自行恢复选中的 ctor。
- 证据：`crates/scoopc/src/llvm/codegen/class_ctor.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/pipeline/hir_stage.rs`，`tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop:1-50`。

### 3.10 默认参数补齐只覆盖有限顶层函数

- 状态：`Closed/Re-scoped`。
- 结论：default/named arg canonicalization 已在 typed HIR/MIR contract 上收口；top-level direct call、extension call 与 class ctor call 在进入 MIR/LLVM 前都必须带完整 ordered args。backend 若再次看到 arity drift，只会把它视为 upstream contract bug，而不再补齐或修复顺序。
- 证据：`crates/scoopc/src/pipeline/hir_stage.rs`，`crates/scoopc/src/pipeline/mir_stage.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`tests/fixtures/mir_refactor/call_contracts.scoop:1-42`。

### 3.11 closure env / capture shape 限制

- 状态：`Closed/Re-scoped`。
- 结论：closure env 现在统一发布 `ClosureEnvTransportMetadata` 与 descriptor-backed composite transport contract；默认主线接受的 tuple env、aggregate capture 与 mutable capture box 都走同一套 env layout / trace / load-store 规则，而 `Unit` / `Tuple` env 约束已经前移成上游 MIR 不变量，不再是 live backend gap。
- 证据：`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/llvm/codegen/composite_transport.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`，`tests/fixtures/runtime_gc/gc_trace_closure_capture_string_basic.scoop`，`tests/fixtures/runtime_gc/gc_move_enum_maybe_ref_closure_capture_basic.scoop`。

### 3.12 effect-typed closure/function-value adapter 限制

- 状态：`Closed/Re-scoped`。
- 结论：effect-typed closure / function-value / `FunPtr` surface 现在已经通过 published dynamic-invoke contract、local callable provenance 与 callable carrier target/adaptor 收口。actual outward-empty callable 会保留 plain ABI；actual outward 非空 callable 继续走显式 Step boundary / adapter，而不再把 effect-typed surface 直接退成 unsupported。
- 证据：`crates/scoopc/src/effect_facts/builder.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`，`tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop:1-22`，`tests/fixtures/run-pass/receiver_function_value_call_basic.scoop:1-24`，`tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop:1-31`。

### 3.13 `StoreMember` continuation route ambiguous 会失败

- 状态：`Closed/Re-scoped`。
- 结论：`StoreMember` 的 continuation route 现在已经明确冻结为 upstream MIR contract：`Ambiguous` 会在 production MIR verifier/materialized validation 上被拒绝，raw LLVM 只接受 `None/Unique`，且会继续校验 unique route 的 source-local/source-ty 一致性；LLVM emitter 不再现场猜测 route。
- 证据：`crates/scoopc/src/mir/mod.rs`，`crates/scoopc/src/mir/materialize.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`tests/fixtures/mir_refactor/assignment_places.scoop`。

## 4. Aggregate / Enum / Array / Boxing 缺口

### 4.1 tuple/struct 到 `Any`/`Ref` 没有通用装箱

- 状态：`Closed/Re-scoped`。
- 结论：descriptor-backed `Rvalue::Transport` / value erasure 现在已经能统一承载 tuple、struct 与 enum payload 的 `Any`/`Ref` 装箱；剩余分支只再表达“boxing intent / materialized layout contract 被破坏”的 backend guard，而不再是 live feature gap。
- 证据：`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen/composite_transport.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_value_boxing_transport`，`tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`。

### 4.2 enum boxed payload 中 `Unit` field 会失败

- 状态：`Closed/Re-scoped`。
- 结论：boxed payload 现在会把 `Unit` field 写成零值占位，而不是直接失败；该编号不再是 live blocker。
- 证据：`crates/scoopc/src/llvm/codegen/enum_lowering.rs:170-171`。

### 4.3 大整数 enum payload 超过 payload word 会失败

- 状态：`Closed/Re-scoped`。
- 结论：enum layout 现在会在 lowering 前把超过 payload word 的整数 payload 统一改走 boxed composite transport；下游 large-int 分支仅剩“boxed payload contract 漂移”的 impossible-state guard，不再是默认主线 blocker。
- 证据：`crates/scoopc/src/llvm/codegen/layout.rs`，`crates/scoopc/src/llvm/codegen/enum_lowering.rs`，`tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`。

### 4.4 nested enum / tuple / struct payload 有 unsupported repr

- 状态：`Closed/Re-scoped`。
- 结论：non-niche nested enum、tuple、struct payload 现在会在 enum layout 阶段统一切到 boxed composite transport；ctor lowering 与 when/pattern extraction 共同消费同一套 boxed payload contract，原先的 unsupported repr 只再保留为 contract guard。
- 证据：`crates/scoopc/src/llvm/codegen/layout.rs`，`crates/scoopc/src/llvm/codegen/enum_lowering.rs`，`crates/scoopc/src/llvm/codegen/control_flow.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_enum_payload_transport`，`tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop`。

### 4.5 Array get/set 对 composite element 支持不足

- 状态：`Closed/Re-scoped`。
- 结论：array builder / get / set 现在都通过 `ArrayElementTransportMetadata` 与共享 descriptor-backed composite transport contract 传递 composite element；缺 metadata 或退回 `u64` decode 对 composite value 的旧路径已降为 backend contract guard。现有 cross-thread resume composite payload regression 继续复用同一套 descriptor helper，而没有重新分叉新的 transport contract。
- 证据：`crates/scoopc/src/mir/lower.rs`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_array_composite_transport`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs::refactor_llvm_cross_thread_resume_payload_transport`，`tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`，`tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`。

## 5. Effect Refactor / Late-Lowered State Graph 缺口

### 5.1 ABI routing 仍可能按内部 effect/control 形状而非 actual outward effect set 分类

- 状态：`Closed/Re-scoped`。
- 结论：callable ABI routing 现在以 actual outward effect set / published late-lowered callable ABI 为准，而不是以 surface 声明 effect row 或内部 control 形状为准。outward-empty callable 即使出现在 effect-typed surface 上，也会维持 plain ABI；只有 actual outward 非空或显式 adapter surface 才会发布 Step ABI。
- 证据：`crates/scoopc/src/effect_facts/builder.rs`，`crates/scoopc/src/llvm/codegen/mir_body.rs`，`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`，`crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`。

### 5.2 unsupported source classification 被 verifier 放行，lowering 才失败

- 状态：`Closed/Re-scoped`。
- 结论：verifier 现在已经把 `Unsupported` source classification 直接升格为 frontend-style error；旧“晚到 lowering 才炸”结论已过时。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:3101-3111`。

### 5.3 `ResumeUnwind` 只有空 cleanup placeholder 可接受

- 状态：`Closed/Re-scoped`。
- 结论：`ResumeUnwind` 现在只接受 published cleanup/unwind contract。cleanup state、origin/resume-state 来自 `Suspend { cleanup_state, resume_state, boundary_ids }` 的 cleanup route；source slice 来自 terminal cleanup state 的 canonical MIR cleanup terminator；ordinary return / effect-outcome return / plain return 会在真正返回前统一执行 handle-context cleanup，而 call-boundary complete 的 frame-free tail 只会在无后续 suspend/cleanup、无 handle completion、无 runtime-error/composed boundary 时提前释放 frame root。`ResumeUnwind` terminal 本身只作为 enclosing `HandleDispatch` pending-completion contract 的 sink，若直接落到该 state 则按 impossible-state `unreachable` 处理。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`，`tests/fixtures/run-pass/effect_raise_cleanup_gc_basic.scoop`，`tests/fixtures/run-pass/effect_handle_return_from_function_basic.scoop`，`tests/fixtures/run-pass/effect_handle_return_from_function_finally.scoop`，`tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.scoop`。

### 5.4 outward-empty callable 不应被路由为 effect-step entry；`main(args)` 是当前症状

- 状态：`Closed/Re-scoped`。
- 结论：outward-empty callable 现在不会再被误路由到 effect-step entry；`main(args)` 已通过 plain entry routing 正确落到 published plain shell，并把 `Array<String>` argv 交给 plain entry，而不是再走 effect-step wrapper。
- 证据：`crates/scoopc/src/llvm/codegen/effect_lowered/body.rs`，`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`，`tests/fixtures/run-pass/std_process_args_exit_basic.scoop:1-15`。

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

- 状态：`Closed/Re-scoped`。
- 结论：`GC.pin/unpin`、`GC.handleNew/Get/Drop` 与 `GcHandle.raw: UIntPtr` callback/native token round-trip 的最终支持面已经固定：默认主线接受的引用对象 pin、stable handle create/get/drop、callback token 回传都具备 typed MIR contract、LLVM lowering 与 runtime GC 回归；值类型 `pin/handleNew`、非 `Pinned` 的 `unpin`、非 `GcHandle` 的 `handleGet/drop`，以及把 `Pinned` 当 ordinary `@Extern` ABI token 的用法，都以前端明确诊断拒绝。因此该编号不再表示 partial backend gap，只再保留“前端 gate + token/root contract”一致性的 guard 语义。
- 证据：`sysroot/core.scoop:220-259`，`crates/scoopc/src/typecheck/expr/call.rs:5741-6068`，`crates/scoopc/src/typecheck/expr/error.rs:479-517`，`crates/scoopc/src/pipeline/mir_stage.rs:1915-2012`，`crates/scoopc/src/llvm/codegen/mir_body.rs:3799-4137`，`tests/fixtures/run-pass/gc_pin_unpin_basic.scoop:1-38`，`tests/fixtures/runtime_gc/gc_pin_unpin_move_stress_matrix.scoop:1-91`，`tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop:1-27`，`tests/fixtures/runtime_gc/gc_handle_token_roundtrip_callback_basic.scoop:1-55`，`tests/fixtures/runtime_gc/gc_handle_stale_callback_token_is_error.scoop:1-42`，`tests/fixtures/typecheck/gc_handle_new_value_type_is_error.scoop:1-15`，`tests/fixtures/typecheck/gc_unpin_requires_pinned_is_error.scoop:1-15`，`tests/fixtures/typecheck/gc_handle_get_requires_handle_is_error.scoop:1-15`，`tests/fixtures/typecheck/gc_handle_drop_requires_handle_is_error.scoop:1-15`，`tests/fixtures/typecheck/extern_fun_gc_handle_raw_token_roundtrip_ok.scoop:1-29`，`tests/fixtures/typecheck/extern_fun_signature_with_pinned_is_error.scoop:1-12`。

## 8. 建议收口顺序

1. pre-MIR / MIR handoff contract 已基本收紧；后续只需继续保持 `§2.3` / `§2.7` 的 guard-only 语义，不要再让 `UnsupportedMainBody` 承担 production 输入校验角色。
2. raw MIR residual effect/control routing 已收口；后续保持 `§3.1`、`§3.2`、`§3.3`、`§3.6` 的 gate-only 语义，避免这些 shape 回流到 plain/materialized MIR emitter。
3. effect-refactor 的 ABI/routing 主线已收口；后续保持 `§3.12`、`§5.1`、`§5.3`、`§5.4` 的 guard-only 语义，确保 actual outward effect set 继续决定 callable ABI。
4. `§1.13` 已收口；后续继续保留 array literal synthetic helper call-site identity 的 regression guard，避免 composite transport 入口再次在 backend 前失真。
5. aggregate/composite 的 enum/array boxing 主线已收口；后续保持 `§4.1`、`§4.3`、`§4.4`、`§4.5` 的 guard-only 语义，并继续让 cross-thread resume payload 复用同一套 descriptor-backed contract。
6. 保持前端 gate 与 backend 能力同步：`§7.1`、`§7.2`、`§7.3`、`§7.5`、`§7.6` 只要放开其一，就应同步补齐 MIR contract、layout 与 runtime 语义，而不是依赖 `UnsupportedMainBody` 才暴露错误。

## 9. 建议验证矩阵

- 基线：`cargo test --all`。
- fixture 主线：`cargo run -p scoop -- test`。
- GC/runtime 主线：`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`。
- MIR / placeholder / handoff 相关变更：补跑 `cargo test -p scoopc refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_source_classification_verifier`。
- effect-refactor / cleanup / cross-thread 相关变更：补跑 `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`、`cargo test -p scoopc refactor_llvm_thread_resume_noncomplete_policy`、`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`。
- reflection / extern global / interface default method 相关变更：补跑 `cargo test -p scoopc llvm_tests`，并关注 `getPlatform()`、`@Extern` global、interface default method dispatch 的 IR 断言。
