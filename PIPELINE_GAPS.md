# HIR -> MIR -> Codegen Pipeline Gaps

日期：2026-05-06

范围：本文记录当前 `crates/scoopc` 中 HIR -> MIR -> LLVM/codegen pipeline 还不能稳定接收合法 Scoop 代码的缺口。内容来自只读代码审计、fixture/spec 交叉检查和现有 TODO 记录；本文不代表已经全部复现为最小失败用例。

本文重点关注“前端/typecheck 已经接受，或 spec/fixture 显示应被当前语言表面接受，但在 HIR lowering、MIR materialization、raw MIR codegen、effect-refactor late lowering 或 runtime ABI 阶段不闭环”的问题。少数仍被 typecheck 主动挡住的语言表面放在最后的“前端暂挡但与 pipeline coverage 相关”章节。

## 总览

当前 pipeline 的主要风险不是单个 backend intrinsic 缺失，而是以下几个系统性问题叠加。

- HIR/MIR lowering 仍会为若干合法或半合法表面生成 `Todo(...)` 节点。
- refactor direct-style MIR validator 明确不要求全 body 无 Todo，materialization 也会透传 Todo。
- raw MIR LLVM codegen 对 `Handle`、`ResumeUnwind`、`TypeCheck`、`Cast`、`Virtual/Interface/Resume` call kind、cleanup perform 等核心 MIR 结构不支持。
- effect-refactor late-lowering 依赖 P4/P5/P6 发布完整 boundary/ABI contract；ABI kind 必须由 actual outward effect set 决定：空集对外就是 plain function，即使函数内部使用并处理了 effect/control。
- 泛型、effect-row generic、default/named args、aggregate boxing、enum/array composite payload、function value/closure ABI 仍存在形状限制。
- spec/typecheck 已有证据的 `!!`、runtime reflection fallback、`@Extern` global variable、runtime `is/as/as?` 等表面还没有在新 MIR/refactor 主线下完全闭合。

## 严重程度定义

- 严重：合法代码会直接变成 Todo、MIR/codegen hard error，或会产生明显错误语义。
- 高：合法代码只能依赖 legacy/HIR fallback 或特殊 side table；一旦走 MIR-only/strict path 会失败。
- 中：只在特定组合、泛型实例、effect/closure/aggregate 形状下失败。
- 候选：代码或注释显示风险，但需要补最小 fixture 确认当前默认路径是否仍失败。

## 1. HIR / MIR Lowering 缺口

### 1.1 `comptime` block/if/for 语句仍是 Todo

严重程度：严重。

证据：

- `crates/scoopc/src/hir/lower/stmt.rs:165-169` 将 `ComptimeBlock`、`ComptimeIf`、`ComptimeFor` 降为 `StmtKind::Todo(...)`。
- `SCOOP_FULL_SPEC.md:1170-1209` 规定 `comptime for` / `comptime if` 应在编译期展开或裁剪。
- `SCOOP_FULL_SPEC.md:1334-1357` 在泛型 JSON 示例中使用普通函数体内的 `comptime if/for`。

影响：

合法的编译期控制流如果没有在更早阶段被完全解释/裁剪，进入 HIR 后会留下 `StmtKind::Todo`。MIR lowering 会把 HIR Todo 转为 `StatementKind::Todo`，raw MIR codegen 最终拒绝 `pass MIR statement todo`。

修复方向：

- 在 typechecked HIR 进入 MIR 前完成 comptime expansion/elimination。
- 或在 HIR lowering 中用已计算的 comptime result 重写成普通 HIR block/statement。
- validator 应禁止 runtime HIR/MIR 中残留 comptime Todo。

### 1.2 splice field `value.[field]` 仍是 Todo

严重程度：严重。

证据：

- `crates/scoopc/src/hir/lower/expr.rs:620-621` 将 `SpliceField` 降为 `ExprKind::Todo("splice_field")`。
- `SCOOP_FULL_SPEC.md:1298-1308` 定义 `.[field]` operator。
- `SCOOP_FULL_SPEC.md:1178-1185`、`SCOOP_FULL_SPEC.md:1339-1344` 在 comptime reflection 示例中使用 `value.[field]`。
- `tests/fixtures/typecheck/splice_field_access_string_lit_ok.scoop` 和 `tests/fixtures/comptime/splice_field_access_v0_basic.scoop` 表示前端/解释器已有部分接受证据。

影响：

泛型/comptime 生成代码中常见的 field splice 无法进入 MIR/codegen。若 comptime expansion 没有提前把它替换成普通 member access，就会在 HIR -> MIR 后成为 `Rvalue::Todo`。

修复方向：

- 在 comptime unroll 时把 `value.[field]` 解析为具体 field access。
- 对无法静态解析的 splice 在 typecheck/comptime 阶段 fail-fast，而不是留给 HIR Todo。

### 1.3 class literal / annotation class literal runtime fallback 不闭合

严重程度：中，候选。

证据：

- `crates/scoopc/src/hir/lower/expr.rs:111` 将 `ClassLit` 降为 `ExprKind::Todo("class_lit")`。
- `tests/fixtures/typecheck/annotation_args_const_expr_array_enum_classlit_ok.scoop` 使用 `String::class`。
- `tests/fixtures/comptime/annotation_access_v0_complex_args.scoop` 注释说明 class literal v0 视为类型名字符串常量。

影响：

如果 class literal 出现在 HIR runtime lowering 路径，而不是被 annotation/comptime 专门消费，会变成 Todo 并阻塞 MIR/codegen。

修复方向：

- 明确 class literal 是否只允许 annotation/comptime。
- 若允许 runtime fallback，应 lower 为稳定的 metadata/string/TypeMeta 表示。
- 若不允许，应在 typecheck 阶段诊断，而不是 HIR Todo。

### 1.4 顶层 `val` 在 MIR 中仍是 `Item::Todo`

严重程度：高。

证据：

- `crates/scoopc/src/mir/lower.rs:907-910` 将 `hir::Item::Val` 降为 `Item::Todo { kind: "top-level val" }`。
- `crates/scoopc/src/mir/lower.rs:911` 对 HIR Todo 继续透传。

影响：

顶层 immutable value / initializer 不形成 MIR root。当前 backend 依赖 HIR side table、top-level const/value indexes 或 special handling；如果 pipeline 目标是 MIR-only analysis/codegen，top-level init 的 effect、依赖、runtime value semantics 都不完整。

修复方向：

- 为 top-level immutable value / const value 生成 MIR initializer body。
- 将 hidden init effect、dependency ordering、const/runtime split 纳入 MIR facts。
- 避免后端直接回读 HIR expr 作为长期主线。

### 1.5 `typealias`、package-level `comptime if`、`type`、`object` file item 仍是 Todo

严重程度：中高。

证据：

- `crates/scoopc/src/hir/lower/mod.rs:478-493` 将 `TypeAlias`、`ComptimeIf`、`Type`、`Object` 降为 `Item::Todo`。
- `crates/scoopc/src/mir/lower.rs:911` 将这些 Todo 继续透传。

影响：

虽然 member fun、object init、type layouts 目前大量依赖 side tables，但 MIR file 本身不是完整 program IR。任何 MIR-only pass 都无法从 MIR item graph 得到完整声明、object initializer、metadata 或 reflection 信息。

修复方向：

- 将 non-executable declaration 与 executable initializer 分离建模。
- 至少为 object init、type metadata、alias resolved form 发射非 Todo MIR/metadata item。

### 1.6 赋值 LHS 只覆盖 local、top-level 和 member access

严重程度：高。

证据：

- `crates/scoopc/src/mir/lower.rs:1658-1724` 中 `lower_assign_stmt` 仅处理 local var、top-level var、member access。
- 其它 LHS 形状落到 `StatementKind::Todo("assign lhs lowering pending")`。

影响：

合法复杂 lvalue 形态一旦通过 typecheck，例如未来 indexed assignment、desugared delegated property setter、atomic/member-place composite、safe-member setter 或其它 assignable place，MIR 会留下 Todo。raw MIR codegen 在 `crates/scoopc/src/llvm/codegen/mir_body.rs:2261-2264` 拒绝该 statement。

修复方向：

- 建立统一 place/lvalue MIR，覆盖 local、global、field、index、deref、property setter 等。
- 对未支持 lvalue 在 typecheck 或 HIR lowering 阶段 fail-fast，避免生成 Todo。

### 1.7 callable callee / ctor callee provenance 不完整会生成 Todo

严重程度：高。

证据：

- `crates/scoopc/src/mir/lower.rs:2500-2524` 中 callee 不是可识别 callable value 时生成 `Rvalue::Todo("call callee lowering pending")`。
- `crates/scoopc/src/mir/lower.rs:2530-2540` 中 unresolved name callee 未变成 enum variant/class ctor 时生成 `Rvalue::Todo("ctor call lowering pending")`。

影响：

合法 callable 表达式如果在 HIR/typecheck side table 中 provenance 丢失，MIR 无法决定 `Direct`、`Closure`、`FunValue`、`Virtual`、`Interface`、`Resume` 等 call kind。该问题直接影响 higher-order function、callable member reference、generic function value 和 constructor call。

修复方向：

- 在 typed HIR 中保存 authoritative callable binding。
- MIR lowering 只消费 typed binding，不再依赖表达式形状猜测。
- 对 unresolved/callable ambiguity 早诊断。

### 1.8 dynamic dispatch callee 拆解失败会生成 Todo

严重程度：中高。

证据：

- `crates/scoopc/src/mir/lower.rs:2683-2766` 负责 dispatch call lowering。
- `crates/scoopc/src/mir/lower.rs:2731-2737` 如果 `callee_fqn.rsplit_once('.')` 失败，生成 `Rvalue::Todo("dispatch callee lowering pending")`。

影响：

动态分派依赖字符串 FQN 拆解 owner/member。特殊命名、mangling、extension/interface edge case 或错误 side table 可能让合法 dynamic member call 降成 Todo。

修复方向：

- dispatch metadata 应来自 resolver/typecheck 的结构化 owner/member binding。
- 不应在 MIR lowering 阶段用字符串拆分恢复语义。

### 1.9 `Continuation.resume` 只接受 canonical callee shape

严重程度：严重。

证据：

- `crates/scoopc/src/mir/lower.rs:2610-2681` 中 `lower_resume_call_expr` 只接受 member access 或 top-level canonical callee。
- 非 canonical 形状生成 `Rvalue::Todo("resume lowering requires canonical callee shape")`。

影响：

合法 continuation resume 如果经过别名、function value、wrapper、extension 或其它间接 callee 表达式，MIR lowering 无法发布 `CallKind::Resume`。该问题会阻断 continuation escape/resume 组合。

修复方向：

- typecheck 应发布 resume call contract，而不是让 MIR 从语法形状恢复。
- MIR lowering 应按 contract 生成 `CallKind::Resume`。

### 1.10 `perform` 缺 typed contract 时生成 Todo terminator

严重程度：严重。

证据：

- `crates/scoopc/src/mir/lower.rs:2777-2845` lowering `perform`。
- `crates/scoopc/src/mir/lower.rs:2800-2813` 缺 canonical perform args/metadata 时生成 `Rvalue::Todo("refactor perform contract missing")` 和 `TerminatorKind::Todo(...)`。

影响：

effect op call 如果 P2 typed handoff/source path/span contract 漂移，就不能进入 P3 direct-style MIR validation、P4 effect facts 或 P5/P6 late lowering。

修复方向：

- perform site contract 必须由 typecheck/effect solver 以 stable site id 发布。
- MIR stage 应对缺 contract fail-fast，并给出 source diagnostic，而不是生成 Todo body。

### 1.11 `handle` 缺 typed contract 时生成 Todo terminator

严重程度：严重。

证据：

- `crates/scoopc/src/mir/lower.rs:2847-2899` lowering `handle`。
- `crates/scoopc/src/mir/lower.rs:2862-2879` 缺 refactor handle contract 时生成 `Rvalue::Todo("refactor handle contract missing")` 和 `TerminatorKind::Todo(...)`。

影响：

合法 handler 表达式如果缺 site metadata/arm metadata，body/arm/finally CFG 不能被 effect facts 和 late-lowered state graph 消费。

修复方向：

- handle contract 应在 typed HIR/effect contract 阶段强制存在。
- 缺 contract 时 MIR stage fail-fast。

### 1.12 `with` copy-update fallback 仍能产生 Todo

严重程度：中。

证据：

- `crates/scoopc/src/hir/lower/expr.rs:3215-3313` 在缺 typecheck maps 时返回 `ExprKind::Todo("with_update")`。
- `crates/scoopc/src/hir/lower/expr.rs:3399-3466` 对 unsupported aggregate kind 返回 `Todo("with_update")`。
- `crates/scoopc/src/hir/lower/expr.rs:3574-3581`、`3640-3645`、`3683-3688` 仍有多个 fallback Todo。

影响：

已有 `with_update_*` run-pass 覆盖显示部分场景已通，但 lowering 仍依赖 typecheck 写回的 aggregate maps。map 缺失、enum update edge case 或 unsupported aggregate 会晚到 HIR/MIR Todo。

修复方向：

- 对 typed pipeline 强制要求 copy-update metadata 完整。
- 将 unsupported aggregate 在 typecheck 阶段诊断。
- 为 enum/tuple/struct nested paths 建立稳定 HIR lowering contract。

## 2. MIR Handoff / Materialization 缺口

### 2.1 refactor direct-style MIR validator 允许普通 Todo 通过

严重程度：严重。

证据：

- `crates/scoopc/src/mir/mod.rs:350-356` 注释说明验证器“不试图把当前整个 MIR 限制为完全无 Todo”。
- `crates/scoopc/src/mir/mod.rs:396-424` 只在 Todo reason 属于 forbidden effect Todo 时拒绝。
- `crates/scoopc/src/mir/mod.rs:447-502` 普通 `TerminatorKind::Todo(_)` 被允许。
- `crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs:136-153` refactor MIR stage 调用该 validator 后继续产出 stage output。

影响：

MIR stage 可能产出包含非 effect Todo 的 body，并进入 materialized MIR、effect facts 或 raw codegen。错误从“IR 边界”推迟到 LLVM lowering 或 runtime，定位困难。

修复方向：

- 区分 dump/debug MIR 和 production MIR。
- production/refactor pipeline 应要求 all executable MIR body 无 Todo。
- Todo reason 应带 source span 和 structured category。

### 2.2 MIR materialization 透传 Todo

严重程度：严重。

证据：

- `crates/scoopc/src/mir/materialize.rs:3713` 对 `StatementKind::Todo(_)` no-op。
- `crates/scoopc/src/mir/materialize.rs:3745` 对 `TerminatorKind::Todo(_)` no-op。
- `crates/scoopc/src/mir/materialize.rs:3918` 对 `Rvalue::Todo(_)` no-op。

影响：

generic materialization 会把不完整 MIR 克隆/实例化到 monomorphic snapshot。后续 pass 很难区分是模板缺口还是实例化缺口。

修复方向：

- materializer 应在 production mode 拒绝 Todo。
- 或在实例化结果上运行 stricter MIR verifier。

### 2.3 raw MIR codegen 最终拒绝 Todo

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:2261-2264` 拒绝 `StatementKind::Todo`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2362-2367` 拒绝 `TerminatorKind::Todo`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2485-2490` 拒绝 `Rvalue::Todo`。

影响：

当前 pipeline 是“前面放行，后面爆炸”的形态。合法代码只要在前端 metadata handoff 中有一个缺口，就会走到 unsupported main body，而不是在 MIR stage 给出有意义诊断。

修复方向：

- 把 Todo 禁止前移到 MIR validation。
- 保留 dump-only lowering 但不要作为 production input。

### 2.4 `Return { value: None }` contract 不一致

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1068-1074` support checker 拒绝 `Return { value: None }`，注释说明 generic MIR 仍保留隐式尾值约定，raw MIR bridge 不应误降成默认值。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2291-2303` actual codegen 遇到 `None` 又会生成 declared return type 的 default value。

影响：

如果 checker 没挡住，非 Unit 函数可能把缺失 return value miscompile 成 `false`、`0`、null 或其它默认值。

修复方向：

- MIR 应显式表达 tail value。
- 非 Unit `Return { value: None }` 在 verifier 中禁止。
- Unit return 可用 dedicated terminator 或 typed check。

### 2.5 generic template / MIR root 缺失是 hard error

严重程度：高。

证据：

- `crates/scoopc/src/mir/materialize.rs:237-247` 定义 `MissingGenericTemplate` 和 `MissingMirRootForTemplate`。

影响：

合法 generic callable 如果 HIR -> MIR 没有发射 root，尤其 member fun、extension、companion、object/member side-table 边界，materializer 无法形成 monomorphic MIR instance。

修复方向：

- 确保所有 generic callable 在 MIR root index 中有 canonical template。
- 对跨文件/member/extension generic root 建立统一 symbol key。

### 2.6 effect-row generic direct-call instance 推断依赖 site binding

严重程度：中高。

证据：

- `crates/scoopc/src/mir/materialize.rs:4104-4123` 中若 signature 有 effect param name，fallback inference 返回 `None`。

影响：

合法 `<eff E>` generic call 如果缺少 P2/HIR call-site binding，materializer 无法推断 instance，后续 codegen 找不到单态目标。

修复方向：

- effect-row args 应进入 call-site instance key。
- materializer 应能从 typed binding 读取 effect args，或给出明确 source diagnostic。

### 2.7 `TypeKind::Param` 仍可能到达 codegen

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/ty.rs:171-179` 遇到 `TypeKind::Param` 返回 `None` 并 warn。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:1801-1841` raw MIR `cg_ty_of_mir_type` 对 `TypeKind::Param(_)` 返回 `None`。
- `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs:5852-5856` ABI materialization 遇到未实例化类型参数直接 frontend error。

影响：

generic materialization 漏替换时会在 LLVM type lowering、frame slot layout、source ABI layout、call arg ABI 中失败。

修复方向：

- 在 materialized MIR snapshot 上验证无裸 type/effect param。
- 对 generic effect resume surface 的 erased carrier 例外做显式区分。

### 2.8 resume surface 对裸 type param 有特例，普通 source value 没有

严重程度：中。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs:5707-5721` 对 resume surface 中的 `TypeKind::Param` 使用 erased managed carrier。
- 同文件 `5852-5856` 对普通 source value 遇到 `TypeKind::Param` fail-fast。

影响：

generic effect payload 可以在 resume surface 有 erased ABI，但普通 call args/source values/frame slots 仍不能承载裸 type param。合法 generic effect/callable 组合如果没有完全 monomorph，会在 ABI materialization 阶段失败。

修复方向：

- 明确哪些 ABI surface 允许 erased carrier。
- 其它 surface 必须在 P5/P6 前保证完全实例化。

## 3. Raw MIR LLVM Codegen 缺口

### 3.1 `Handle`、`ResumeUnwind`、`Todo` terminator 不支持

严重程度：严重。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1094-1097` support checker 返回 false。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2362-2367` codegen 返回 `UnsupportedMainBody { kind: "pass MIR terminator" }`。

影响：

direct-style MIR 中合法 `handle/finally/cleanup` 不能直接进入 raw MIR codegen，只能依赖 effect-refactor late lowering。如果某个 body 被选择到 raw MIR path，会失败。

修复方向：

- raw MIR codegen 要么实现这些 terminator，要么禁止带 effect/control terminator 的 body 进入 raw path。

### 3.2 `Perform` 不支持 cleanup unwind，且不使用 `resume_target`

严重程度：严重。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1081-1083` support checker 拒绝 `UnwindAction::Cleanup`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:3385-3399` cleanup unwind hard error。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:3401-3430` codegen 发送 effect signal 后走 non-resuming effect exit 并 `unreachable`，不跳转到 `resume_target`。

影响：

可恢复 effect 的 direct-style MIR 语义没有在 raw MIR codegen 中表达。穿过 `finally`/cleanup 的 perform 也不能 codegen。

修复方向：

- raw path 若要支持 direct-style effect，必须实现 handler stack、resume token、cleanup route。
- 否则 `Perform` body 应全部转给 effect-refactor Step lowering。

### 3.3 `PerformResult` 在 raw MIR 中返回默认值

严重程度：严重，潜在 miscompile。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:2445-2448` 对 `Rvalue::PerformResult` 只验证 effect instance 后返回 `default_value(span, target_cg)`。

影响：

如果 perform-result 没被 P5/P6 resume payload injection 消费，raw codegen 会丢失 handler resume 后的真实值。

修复方向：

- 禁止 `PerformResult` 进入 raw MIR codegen。
- 或实现 resume payload slot binding。

### 3.4 `TypeCheck` / `Cast` raw MIR 不支持

严重程度：严重。

证据：

- `crates/scoopc/src/mir/lower.rs:2147-2195` HIR `is/!is/as/as?` 会降为 MIR `Rvalue::TypeCheck` / `Rvalue::Cast`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:1218-1221` support checker 拒绝。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2485-2490` codegen 拒绝。
- `tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop:1-11` 要求 `is/!is/as/as?` 端到端 codegen。

影响：

合法 runtime type check/cast 只能依赖 legacy HIR codegen 或 partial refactor value primitive。MIR-only path 不通。

修复方向：

- 将 `crates/scoopc/src/llvm/codegen/mod.rs:6238-6522` 的 HIR typecheck/cast lowering 迁移到 MIR value primitive。
- `as` failure 应发布/使用 `Raise<RuntimeError.ClassCastFailed>` boundary。
- `as?` 应构造 `Option<T>`，并支持 class/interface/parameterized runtime match。

### 3.5 refactor effect-neutral cast/typecheck 支持不完整

严重程度：高。

证据：

- `crates/scoopc/src/effect_lowered/materialize.rs:3495-3519` 将 `Rvalue::TypeCheck` / `Rvalue::Cast` 分类为 `EffectNeutralValue`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2519-2540` 只支持一部分 `CastOp::As`，且 runtime ref/string cast 会报 `refactor value primitive runtime cast`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:2622-2630` 拒绝 `TypeCheck` 和 `CastOp::AsQ`。

影响：

late lowering 认为这些 rvalue 可作为普通 value primitive，但 LLVM lowering 仍无法生成代码，导致 refactor path 晚期失败。

修复方向：

- classification 应区分已支持 cast/typecheck 与需要 published boundary 的 runtime cast。
- `as` / `as?` / `is` 应共享 runtime type descriptor/itable matching implementation。

### 3.6 `Virtual` / `Interface` / `Resume` call kind raw MIR 不支持

严重程度：严重。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1530-1559` support checker 拒绝这些 call kind。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:4285-4335` codegen 返回 `pass MIR call kind`。

影响：

合法动态分派和 continuation resume 即使已经在 MIR 中显式建模，也不能走 raw MIR codegen。

修复方向：

- raw MIR call lowering 应实现 vtable/itable dispatch 和 continuation resume ABI。
- 或在 body routing 中强制这些 call kind 进入 refactor/dynamic boundary path。

### 3.7 `TopLevelRef` raw MIR 不覆盖普通函数引用

严重程度：中高。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1113-1118` `TopLevelRef` support 只检查 object init、top-level const、immutable value、var。

影响：

如果 higher-order top-level function value 在 MIR 中以 `TopLevelRef` 形式出现，而不是提前合成为 closure/function value object，raw MIR value ref 无法 codegen。

修复方向：

- 保证所有 function reference 在 HIR/MIR lowering 中规范化为 closure/function value。
- 或扩展 `TopLevelRef` codegen 支持 function symbol value。

### 3.8 MIR pattern `is Type` 只支持 ref/string

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1949-1969` 对 `Pattern::Is` 要求 subject 和 target codegen type 都是 `Ref` 或 `String`。

影响：

合法 pattern type test 如果涉及 value type、nominal payload、enum 或 aggregate，会被 raw MIR checker 拒绝。

修复方向：

- 明确 `is` pattern 对 value type 的静态/动态语义。
- 对 ref type 复用 runtime type descriptor；对 value type 尽量静态折叠。

### 3.9 class ctor raw MIR 不支持 named/default args

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:5237-5241` ctor 参数数量不等时报 `pass MIR class ctor default/named args`。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:5307-5313` 遇到 named arg 或 default param 报 `pass MIR class ctor named/default arg`。
- `tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop` 已要求 class ctor named/default/delegation 可运行。

影响：

如果 named/default ctor call 没有在 HIR/typecheck 层完全补齐并按参数顺序重写，strict raw MIR codegen 会失败。

修复方向：

- ctor call MIR 应携带 selected ctor 和 complete bound args。
- 或 raw MIR codegen 支持 name/default binding。

### 3.10 默认参数补齐只覆盖有限顶层函数

严重程度：高。

证据：

- `crates/scoopc/src/hir/lower/mod.rs:423-429` 只收集非泛型、非 receiver、非 vararg 的当前文件顶层函数。
- `crates/scoopc/src/hir/lower/expr.rs:4819-4828` 只处理 direct ident top-level call。
- `crates/scoopc/src/llvm/codegen/call/abi.rs:310-315` 和 `crates/scoopc/src/llvm/codegen/call/dispatch.rs:633-638` 要求实参与形参数量一致。

影响：

合法 generic/member/extension/default-arg call 如果没有被 HIR 重写为完整 args，后端不会补齐，最终 arity mismatch。

修复方向：

- typecheck 应输出 bound argument mapping，覆盖 top-level/member/extension/generic/ctor。
- HIR/MIR 应存储完整 ordered args 或 default thunk invocation。

### 3.11 closure env / capture shape 限制

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:1762-1780` raw MIR `MakeClosure` 要求 target ref、env operand supported、env shape supported、closure body supported。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:5805-5827` MIR closure env 只允许 `Unit` 或 tuple，元素只允许 `Unit/Bool/Int/String/Ref`。
- `crates/scoopc/src/llvm/codegen/closure/mod.rs:66-70` legacy closure codegen 拒绝 mutable capture。
- `crates/scoopc/src/llvm/codegen/closure/mod.rs:321-334`、`555-562` legacy capture 同样只接受 scalar/ref-like 类型。

影响：

合法捕获 `Float`、tuple、struct、enum 或 mutable var 的 closure 可能无法 codegen，尤其在 materialized MIR closure body/raw path 中。

修复方向：

- closure env 应使用统一 heap layout，支持 arbitrary GC-traceable source type。
- mutable capture 应通过 capture box 成为一等支持路径。

### 3.12 effect-typed closure/function-value adapter 限制

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs:852-868` effect-typed plain adapter 不支持 hidden-sret aggregate return。
- `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs:3287-3300` top-level `FunPtr` direct call 只接受 pure。
- `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs:3466-3478` top-level function-value direct call 只接受 pure。
- `crates/scoopc/src/llvm/codegen/mir_body.rs:4371-4415` refactor plain closure/FunPtr/function-value call 对 effect-typed surface 要求 adapter。

影响：

合法 effectful function value、effectful closure、返回 aggregate 的 effect-typed function value 需要按 materialized actual outward effect set 选择 plain 或 Step adapter/boundary。actual outward 为空的 callable/function value 不应因为 surface type 或内部已处理 effect/control 被强制走 Step；actual outward 非空或 effect row 未能闭合时，才需要完整 Step adapter/boundary。

修复方向：

- 完成 actual-outward 非空 callable 的 effect-step adapter for aggregate return。
- call facts 必须标明可 materialize 的 callee actual outward effect row；outward 空集发布 plain ABI，非空才发布 boundary/adapter。

### 3.13 `StoreMember` continuation route ambiguous 会失败

严重程度：中。

证据：

- `crates/scoopc/src/llvm/codegen/mir_body.rs:7771-7784` 对 `StoredContinuationRoutePublication::Ambiguous` 返回 `pass MIR ambiguous member continuation route`。

影响：

保存 continuation 到 member 时，如果 RHS 内包含多个可能 continuation route，后端没有明确 transport contract。合法多-owner/multi-continuation shape 会失败或需要更早诊断。

修复方向：

- StoreMember contract 应携带唯一 continuation owner/source。
- ambiguous 应在 effect solver/lowering 阶段拆解或报错。

## 4. Aggregate / Enum / Array / Boxing 缺口

### 4.1 tuple/struct 到 `Any`/`Ref` 没有通用装箱

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/mod.rs:7548-7697` `coerce_value` 支持 Unit/Bool/Int/String/Ref/Enum 等有限路径，但 tuple/struct -> Ref 没有通用装箱。

影响：

合法 aggregate value 作为 `Any`、interface-ish erased carrier、effect payload、Array element、closure capture 或 generic erased value 传递时可能失败。

修复方向：

- 实现 value-type boxing layout，包含 type descriptor、trace metadata、copy/drop 语义。
- Refactor ABI 中统一 aggregate source transport。

### 4.2 enum boxed payload 中 `Unit` field 会失败

严重程度：中。

证据：

- `crates/scoopc/src/llvm/codegen/enum_lowering.rs:163-175` boxed payload field 为 `CgTy::Unit` 时返回 `enum boxed payload field (unit)`。

影响：

合法 enum variant 如果多字段 payload 包含 Unit，会无法构造目标值。

修复方向：

- Unit field 应 elide 或在 payload layout 中占 0-size slot，并保持 field index mapping。

### 4.3 大整数 enum payload 超过 payload word 会失败

严重程度：中。

证据：

- `crates/scoopc/src/llvm/codegen/enum_lowering.rs:282-291` 单字段 int payload 若 bit width 大于 payload word，返回 `enum payload larger than word`。

影响：

合法 `Int128` / `UInt128` 等大整数 payload enum variant 不能用当前 inline payload 表示。

修复方向：

- 大 payload 自动 boxed。
- 或为 payload word 扩展 multi-word representation。

### 4.4 nested enum / tuple / struct payload 有 unsupported repr

严重程度：中高。

证据：

- `crates/scoopc/src/llvm/codegen/enum_lowering.rs:416-425` 对 nested unsupported enum repr、tuple、struct payload 报 unsupported/non-scalar。

影响：

某些 nested enum、struct/tuple payload 组合无法构造或匹配，除非布局选择了已支持的 boxed/niche 主线。

修复方向：

- enum layout 应统一决定 boxed/inline，并让 ctor/match extraction 按 layout 工作。
- 对 non-scalar payload 提供 boxed path。

### 4.5 Array get/set 对 composite element 支持不足

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs:2245-2321` `Array.get` 对 ref/string 走 ref runtime，其它走 `array_get_u64`。
- `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs:2325-2396` `MutableArray.set` 对 ref/string 走 ref runtime，其它走 `coerce_u64_word` + `array_set_u64`。
- `crates/scoopc/src/llvm/codegen/effect/mod.rs:1621-1734` `coerce_u64_word` 对 tuple/struct 报 `u64 word from composite value`，对部分 enum 也有限制。

影响：

`Array<Tuple>`、`Array<Struct>`、多数非 scalar `Array<Enum>` 不能稳定 codegen。

修复方向：

- Array runtime descriptor 应支持 element size、trace function、copy function。
- get/set 应按 element layout copy/load，而不是强制 u64/ref 双轨。

## 5. Effect Refactor / Late-Lowered State Graph 缺口

### 5.1 ABI routing 仍可能按内部 effect/control 形状而非 actual outward effect set 分类

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs:842-848` plain callable terminator 遇到 `Perform`、`ResumeUnwind`、`Handle`、`Todo` 报 `refactor plain callable effect/control terminator`。

影响：

函数 ABI 不应由 body 内是否出现 direct-style effect/control MIR 决定，而应由 actual outward effect set 决定。actual outward effect set 为空的函数对外必须表现为 plain function；内部使用并完全处理的 effect/control 应在 late lowering 中被消化为 plain body 可发射的局部控制流。如果这类函数仍被路由到 Step ABI，或 plain body emission 仍看到残留 `Perform` / `Handle` / `ResumeUnwind`，说明 handled-effect elimination、effect facts 或 ABI routing 不闭合。

修复方向：

- routing 阶段按 actual outward effect set 选择 ABI：空集为 plain，非空才为 Step/effect boundary。
- 对 outward 为空但内部有 handled effect/control 的函数，late lowering 必须先消除或局部化 residual effect/control terminator，再进入 plain body emission。
- verifier 应拒绝两类漂移：outward 为空却发布 Step ABI；plain ABI body 中仍残留未消化的 effect/control terminator。

### 5.2 unsupported source classification 被 verifier 放行，lowering 才失败

严重程度：中高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs:2250-2258` verifier 对 `LateLoweredSourceStatementClassificationKind::Unsupported` 返回 Ok。
- `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs:4491-4509` source slice lowering 遇到 `Unsupported` 才报错。

影响：

不支持的 MIR statement 不会在 ABI verifier 阶段暴露，而是在 LLVM body emission 中晚期失败。

修复方向：

- verifier 应默认拒绝 unsupported classification。
- 如果有 intentional unsupported placeholder，必须带 explicit skip/elide reason。

### 5.3 `ResumeUnwind` 只有空 cleanup placeholder 可接受

严重程度：高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs:2513-2523` 只有 cleanup state 且无 successors 时 Ok，否则报缺 published unwind payload / cleanup continuation contract。

影响：

需要真实 unwind payload、finally pending completion、cleanup continuation 的合法 state graph 没有完整 codegen path。

修复方向：

- 定义 unwind payload carrier。
- 将 cleanup continuation、pending completion、origin/resume-state 纳入 published contract。

### 5.4 outward-empty callable 不应被路由为 effect-step entry；`main(args)` 是当前症状

严重程度：中高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs:380-384` 如果入口是 effect-step callable 且有 argv array，报 `refactor LLVM effect-step main wrapper 尚未发布 Array<String> argv Step ABI`。
- `SCOOP_FULL_SPEC.md:1104-1110` 规定 program boundary 必须 outward `Pure!`；effects 只能在 entry point 内部被完全处理。
- `crates/scoopc/src/typecheck/expr/stmt.rs:380-399` 和 `448-485` 对 `ProgramBoundaryKind::Main` 强制 closed pure row。

影响：

任何 actual outward effect set 为空的函数，即使内部使用并完全处理了 effect/control，对外也应表现为 plain callable。`main(args: Array<String>) / Pure!` 只是当前最显眼的症状：如果 pipeline 把它分类为 effect-step callable，backend 会在 argv Step ABI 处晚期报错；但真正缺口是 outward-effect facts、handled-effect elimination 或 plain/step ABI routing 不闭合，不是需要给 `main` 或 outward-empty 函数增加 Step ABI 支持。

修复方向：

- 修正 effect facts / late-lowering / ABI routing：actual outward effect set 为空的 callable 一律发布 plain ABI。
- 已被函数体完全处理的 effect/control 不应影响该函数对外 ABI；若 plain body emission 仍看到 residual effect/control terminator，应在 verifier 阶段作为 lowering 缺口 fail-fast。
- `main` wrapper 只接受 outward `Pure!` 的 plain entry；如果 ABI contract 仍显示 program boundary 有 outward cases，应在 verifier/codegen handoff fail-fast。
- `main(args)` 的 argv array 继续通过 plain entry ABI 传入；不要为 program boundary 或 outward-empty callable 引入 Step argv ABI 作为语义主线。

### 5.5 cross-thread resume 只支持 u64 payload

严重程度：中高。

证据：

- `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs:1755-1821` helper 名称和 ABI 都固定为 `__scoop_thread_spawn_join_resume_u64`，要求 i64 resume payload ABI。
- `runtime/c/scoop_runtime.c:1845-1862` runtime C struct 和 function pointer 都是 `resume_value: uint64_t`。

影响：

合法非整型、ref、tuple、struct、enum resume payload 无法通过当前跨线程 resume helper。

修复方向：

- cross-thread resume helper 使用 generic transport `{word, gc_ref}` 或 full Step payload ABI。
- runtime 线程 thunk 应 root GC refs 并支持 composite payload。

### 5.6 thread resume 后 non-complete Step 直接 fatal

严重程度：中。

证据：

- `runtime/c/scoop_runtime.c:2628-2631` `scoop_refactor_thread_resume_noncomplete_fatal` 直接 `exit(3)`。

影响：

跨线程 resume 后如果继续向外传播 effect，当前 runtime boundary 没有语言级 handler 可以接管，只能 fatal。某些合法 effectful continuation path 无法表达。

修复方向：

- 定义跨线程 effect propagation contract。
- 或在 type/effect checker 阶段禁止该 surface，并给出明确诊断。

### 5.7 当前 P7 默认 refactor blocker 仍未收口

严重程度：严重。

证据：

- `TODO-P7.md:798-810` 明确列出剩余默认 refactor run-pass blockers。
- `TODO-P7.md:831-835` 记录当前仍阻塞在 higher-order handled effect、multi-owner continuation schema、resume 后 finally/raise contract 等问题。

影响：

默认 refactor 主线尚未完成 full run-pass regression。合法 effect/continuation/GC/task 组合仍可能失败。

修复方向：

- 按 `TODO-P7.md` P7-T02Z / P7-T03 收口剩余 blockers。
- 禁止恢复 legacy fallback、缩小 fixture 或改弱 golden。

## 6. Spec / Fixture 已暴露的具体缺口

### 6.1 `!!` 非空断言仍 expected fail

严重程度：严重。

证据：

- `tests/fixtures/run-pass/not_null_assert_basic.scoop:1-6` 当前 `EXPECT: fail`，注释说明 LLVM lowering 仍收口成 `when arm type mismatch`。
- `crates/scoopc/src/hir/lower/expr.rs:4214-4272` 将 `expr!!` 降成 `when Some(v) -> v; None -> Raise.raise(RuntimeError.NullAssertionFailed)`。
- `SCOOP_FULL_SPEC.md:818-835` 规定 `!!` 失败通过 `Raise<RuntimeError.NullAssertionFailed>` 表达。

影响：

typecheck 已接受的 nullable assertion 不能端到端运行。问题可能落在 HIR `when` arm result typing、`Nothing` coercion、Raise effect boundary 或 enum pattern/lowering 组合。

修复方向：

- 确认 `None -> Raise.raise(...)` 的 `Nothing` 能向 result type coercion。
- 确保 `Raise<RuntimeError>` 在 try/catch 下被 facts/late lowering 捕获。
- 回收 `not_null_assert_basic.scoop` 的 expected fail。

### 6.2 runtime `is/as/as?` 在 MIR/refactor path 不闭合

严重程度：严重。

证据：

- `tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop:1-11` 要求 `is/!is/as/as?` 端到端。
- `crates/scoopc/src/llvm/codegen/mod.rs:6238-6522` legacy/HIR codegen 已有 runtime type check/cast 实现。
- MIR/raw/refactor 缺口见本文 3.4 和 3.5。
- `TODO-P7.md:805-807` 明确要求修复 runtime type-check/cast 与 parameterized interface/class matching 的 refactor frame/layout gap。

影响：

默认 refactor 主线无法完全承载 runtime type check/cast，尤其 parameterized interface/class matching、`as` failure 的 `Raise<RuntimeError.ClassCastFailed>`、`as?` Option 返回。

修复方向：

- 把 legacy HIR implementation 迁移/抽象为 MIR value primitive。
- Runtime type descriptor 和 itable parent-chain matching 应服务 raw MIR 与 refactor path。

### 6.3 runtime reflection fallback `nameOf<T>()` / `getPlatform()` 缺 codegen lowering

严重程度：高，候选。

证据：

- `sysroot/core.scoop:408-445` 声明 `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()` 等 reflection/platform intrinsics。
- `tests/fixtures/typecheck/reflection_runtime_fallback_v0.scoop:1-20` typecheck 接受 runtime `nameOf<Point>()`。
- `tests/fixtures/typecheck/get_platform_runtime_ok.scoop:1-13` typecheck 接受 runtime `getPlatform()`。
- `crates/scoopc/src/llvm/codegen/call/dispatch.rs:235-236` 只看到 `scoop.core.sizeOf` 特判；未见 `nameOf` / `getPlatform` 对应 LLVM lowering。

影响：

这些 const/intrinsic 在 runtime context fallback 下可能通过 typecheck，但 codegen 找不到普通函数定义或 intrinsic lowering。

修复方向：

- 为 runtime fallback 定义稳定 lowering。
- 或把非 fallback 的 intrinsic 限制在 comptime 并在 typecheck 诊断。

### 6.4 `@Extern` global variable 没有 extern storage/linkage model

严重程度：高。

证据：

- `SCOOP_FULL_SPEC.md:2408-2434` 规定 `@Extern` 可用于 global variable，且可与 `@ThreadLocal` 组合。
- `tests/fixtures/typecheck/extern_var_no_initializer_ok.scoop:1-9` typecheck 接受 `@Extern(name = "x") var v: Int`。
- `crates/scoopc/src/hir/mod.rs:857-880` `TopLevelVarStorage` 只有 `ThreadLocal` 和 `Global`。
- `crates/scoopc/src/hir/lower/mod.rs:1452-1486` 只收集 `@ThreadLocal/@Global var`。
- `crates/scoopc/src/llvm/codegen/mod.rs:2837-2857` 总是创建 internal global，并设置 initializer。

影响：

extern global declaration 没有 external symbol name、linkage、initializer absence、unsafe access gating 的 codegen representation。合法 extern var 不能正确链接到 C symbol。

修复方向：

- 扩展 `TopLevelVarStorage`，加入 extern/global/TLS extern symbol metadata。
- LLVM global 应使用 external linkage，不生成 initializer。
- 访问 extern var 应遵守 unsafe context。

### 6.5 interface default method codegen 覆盖需确认

严重程度：候选。

证据：

- `tests/fixtures/typecheck/interface_default_method_not_required_ok.scoop` 表明 typecheck 允许 default method not implemented。
- `crates/scoopc/src/typecheck/interfaces.rs:8-10` 注释写“暂不要求 codegen”。
- 但 `crates/scoopc/src/itable.rs` 和 `crates/scoopc/src/llvm/tests.rs` 已有部分 itable/default support 迹象。

影响：

如果当前 default method dispatch 仍不完整，合法 interface default method call 可能在 dynamic dispatch/codegen 时失败或选错实现。

修复方向：

- 补 run-pass fixture 覆盖 interface default method direct/interface dispatch。
- 确认 itable slot 对 default implementation 的 symbol/linkage/receiver ABI。

## 7. 前端暂挡但与 Pipeline Coverage 相关

这些项目当前可能在 typecheck 阶段已经被拒绝，因此不是“已通过前端但 codegen 失败”的直接缺口。但它们对应的 MIR/codegen 能力也未闭合，后续放开前端时需要同步处理。

### 7.1 or-pattern 带 binder 被 typecheck 拒绝

证据：

- `crates/scoopc/src/typecheck/when_pat.rs:89-98` 对含 binder 的 or-pattern 返回 unsupported。
- `tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop:1-17` 锁定当前错误。

影响：

一致 binder set 的合法 or-pattern 目前不能进入 HIR/MIR。MIR pattern 有 `Pattern::Or`，但 binder extraction/control-flow 还未覆盖。

### 7.2 function type runtime cast / effectful function type cast 暂不支持

证据：

- `crates/scoopc/src/typecheck/expr/error.rs` 中相关 runtime cast/effectful function cast diagnostics 显示该表面被挡住。

影响：

函数值相关 `is/as/as?` 后续放开时，需要 function object runtime type descriptor、effect row matching 和 callable adapter。

### 7.3 use-site effect row type arg 暂不支持

证据：

- `crates/scoopc/src/typecheck/lower.rs` 中 use-site `eff ...` type arg 会被 `UnsupportedTypeRef` 拒绝。

影响：

effect-parameterized type usage 无法进入 typed HIR/MIR。放开后 materializer/ABI 必须支持 effect args in instance key。

### 7.4 `spawn` / user-facing `join` 是延期表面

证据：

- `SCOOP_FULL_SPEC.md:859`、`919-929` 说明 executor framework、spawn/join 用户表面 intentionally deferred。
- `crates/scoopc/src/hir/lower/expr.rs:576-580`、`612-616` 对 `Spawn` / `Join` 生成 Todo。

影响：

这不是当前必须接收的合法代码，但 AST/HIR 中已经有壳。若后续启用 structured concurrency，需要 task runtime、scheduler、effect boundary、GC roots 一起补齐。

### 7.5 struct mutable fields 当前被前端限制

证据：

- `crates/scoopc/src/typecheck/structs.rs` 中 struct primary ctor `var` 字段被拒绝。

影响：

如果将来允许 mutable value-type fields，需要 MIR place、copy-update、member store、GC trace/write barrier 的统一模型。

### 7.6 GC pin/handle intrinsic surface 仍有限制

证据：

- `crates/scoopc/src/typecheck/expr/error.rs` 中 GC pin/handle 相关 diagnostics 限制 ref/handle 形状。
- `TODO-P7.md:802-804` 仍把 `GC.pin` / `GC.unpin` member-function callee shape 列为 remaining dynamic member/intrinsic callable-value lowering blocker。

影响：

更一般的 GC handle/pin surface 放开后，需要 HIR/MIR intrinsic binding、native roots、moving GC update contract 一起闭合。

## 8. 建议收口顺序

1. 先建立 production MIR verifier：executable MIR 中禁止 `Item::Todo`、`StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`，dump-only path 例外。
2. 收口 typed handoff contract：call site、perform site、handle site、resume site、default/named args、dispatch owner/member 都从 typecheck/effect solver 结构化发布。
3. 把 runtime typecheck/cast 从 HIR codegen 迁移为 MIR/refactor 共用 value primitive。
4. 修复 P7 当前 blocker：dynamic member/intrinsic callable-value、runtime cast/parameterized matching、effect/continuation/finally contract。
5. 统一 aggregate transport：boxing、enum payload、array element、closure env、effect payload 都走同一 layout/trace/copy 模型。
6. 为 top-level values/object/type metadata 建立 MIR-level representation，减少后端读取 HIR side table。
7. 最后处理延期/前端暂挡表面，例如 or-pattern binders、structured concurrency、mutable value fields。

## 9. 建议验证矩阵

基础验证：

```bash
cargo test --all
cargo run -p scoop -- test
cargo run -p scoop_tools -- spec-fixtures check
cargo clippy --all-targets -- -D warnings
```

定向 fixture 建议：

```bash
cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/not_null_assert_basic.scoop
cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop
cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop
cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop
cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/std_process_args_exit_basic.scoop
cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/entry_main_args_int_exit_basic.scoop
```

P7 默认 refactor 收口后，应继续执行 `TODO-P7.md` 中 P7-T03/P7-T04 的完整矩阵，特别是 default refactor run-pass、spec-fixtures、moving GC/stress/verify-roots。
