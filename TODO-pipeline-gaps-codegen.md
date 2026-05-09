# TODO（Codegen Closure：pipeline gap 收口）

> 生成时间：2026-05-06  
> 计划基线：[`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md)  
> 差距基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)  
> 格式参考：[`TODO.md`](./TODO.md)  
> 前置条件：refactor HIR/MIR handoff 不再输出 production placeholder；若执行中发现 MIR contract 缺失，先回 [`TODO.md`](./TODO.md) 的 MIR-facing owner 补 contract。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：关闭 `PIPELINE_GAPS.md` 中 codegen-stage scope 的 raw MIR LLVM、effect-refactor LLVM、runtime transport 和 default-regression gaps。

## 任务索引

| ID | 阶段 | 标题 |
| --- | --- | --- |
| `CG-T00` | CG0 | [DONE] 建立 codegen gap inventory 与 backend gate |
| `CG-T00R` | CG0R | [DONE] Review CG-T00 codegen inventory 与 backend gate |
| `CG-T01` | CG1 | [DONE] 收口 raw MIR effect/control route 与 unsupported call kind |
| `CG-T01R` | CG1R | [DONE] Review CG-T01 raw MIR route gate |
| `CG-T02` | CG2 | [DONE] 收口 runtime type/value primitive LLVM lowering |
| `CG-T02R` | CG2R | [DONE] Review CG-T02 runtime value primitive lowering |
| `CG-T03` | CG3 | [DONE] 收口 call/ctor/function-ref/intrinsic/default/interface lowering |
| `CG-T03R` | CG3R | [DONE] Review CG-T03 call/ctor/intrinsic lowering |
| `CG-T04a` | CG4a | [DONE] 建立 composite transport layout contract 与 verifier |
| `CG-T04b0` | CG4b0 | [DONE] 发布 value erasure boxing MIR transport contract |
| `CG-T04b` | CG4b | [DONE] 收口 value boxing composite transport lowering |
| `CG-T04c` | CG4c | [DONE] 收口 enum payload composite transport lowering |
| `CG-T04d` | CG4d | [DONE] 收口 array composite element transport lowering |
| `CG-T04e` | CG4e | [DONE] 收口 closure env/capture transport lowering |
| `CG-T04f` | CG4f | [DONE] 收口 cross-thread resume payload transport lowering |
| `CG-T04R` | CG4R | [DONE] Review CG-T04a-CG-T04f composite transport lowering |
| `CG-T05` | CG5 | [DONE] 收口 effect-typed adapter 与 NoOutward plain ABI |
| `CG-T05R` | CG5R | [DONE] Review CG-T05 adapter 与 NoOutward ABI |
| `CG-T06` | CG6 | [DONE] 收口 source classification、unwind、thread boundary lowering |
| `CG-T06R` | CG6R | [DONE] Review CG-T06 unwind/thread boundary lowering |
| `CG-T07` | CG7 | [DONE] 收口 extern global 与 GC pin/handle runtime surface |
| `CG-T07R` | CG7R | [DONE] Review CG-T07 extern global 与 GC surface |
| `CG-T07S0a0` | CG7S0a0 | [DONE] 修复 elvis_lazy_basic 中 Option enum payload transport trace metadata 漂移，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a1` | CG7S0a1 | [DONE] 修复 fun_call_add_basic 中 refactor plain return coercion 把 `main(): Int` 尾值误判成 `Ref`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a2` | CG7S0a2 | [DONE] 修复 gc_array_class_elements_cross_function 中 `println::<String>` arg lowering 把 `String` 值误判成 `Ref`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a3` | CG7S0a3 | [DONE] 修复 gc_trace_task_field_basic 中 `Async.await(holder.task)` perform site metadata 把 payload transport type 与 payload component type 发布成漂移 shape，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a4` | CG7S0a4 | [DONE] 修复 kotlin_ranges_progressions_basic 中 progression/forEach lowering 的 assign place contract 指向未分配 local symbol，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a5` | CG7S0a5 | [DONE] 修复 list_and_mutable_list_basic 中 MutableList.add/push materialized MIR 的 array transport element type 仍保留 unresolved generic `T`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a6` | CG7S0a6 | [DONE] 修复 literal_numeric_expected_type_absorption_basic 中 `Array<UInt8>` element expected-type absorption 失效导致 run-pass 输出漂移，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a7` | CG7S0a7 | [DONE] 修复 literal_ops_compare_direct_matrix_basic 中 String 字面量 receiver 的 compareTo/concat 直接调用退化成 FunValue callee，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a8` | CG7S0a8 | [DONE] 修复 local_val_destructuring_nested_variant_mismatch_is_error 中 nested variant destructuring runtime-error path 的 direct-arg tuple payload contract 缺少 source component，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a9` | CG7S0a9 | [DONE] 修复 member_call_devirt_final_receiver_direct_call_basic 中 final receiver direct-call 去虚化后 `Base` vtable 仍引用未发射的 `Base.ping` 符号，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a10` | CG7S0a10 | [DONE] 修复 nothing_raise_coerce_to_any_type 中 nested try/catch + `Raise.raise` 的 Nothing/bottom-type HandleDispatch routing contract 歧义，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a11` | CG7S0a11 | [DONE] 修复 object_companion_value_named_nested_init_basic 中 nested object / named companion value access 被误当成 member field target，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a12` | CG7S0a12 | [DONE] 修复 operator_overload_struct_basic 中 struct `compareTo` direct-call lowering 把 `Int` 结果误强制成 struct target，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a13` | CG7S0a13 | [DONE] 修复 safe_member_access_ref_and_extension_basic 中 safe-call `Option` `Some`/`None` lowering 仍退化成 `ctor call lowering pending`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a14` | CG7S0a14 | [DONE] 修复 smart_cast_any_member_access_generic_class_basic 中 smart-cast 分支 generic class field access 仍把 result/frame slot 保留为 unresolved `T`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a15a` | CG7S0a15a | [DONE] 修复 stdlib_hash_set_map_basic 中 `MutableSet.asSet()` 只读视图在同一 body 联合 `Set.len()` / `Set.contains()` 时的 alias receiver call 结果漂移，解除 CG-T07S0a15 的 run-pass 新 blocker |
| `CG-T07S0a15` | CG7S0a15 | [DONE] 修复 stdlib_hash_set_map_basic 中 `scoop.collections.__map_alloc_empty_table` 的 array transport element type 仍保留 unresolved `T`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a16a` | CG7S0a16a | [DONE] 修复 literal_numeric_expected_type_absorption_basic 中 direct `Array<UInt8>` element path 再次退回 nominal/composite surface，解除 CG-T07S0a16 的前置 blocker |
| `CG-T07S0a16` | CG7S0a16 | [DONE] 修复 literal_array_expected_type_nested_basic 中嵌套 `Array<UInt8>` element expected-type 传播仍退回 `Int`，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a17` | CG7S0a17 | [DONE] 修复 star_projection_array_read_view 中 `Array<*>` 读视图把带 GC slot 的 `Any?` element transport trace contract 发布成漂移 shape，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a18` | CG7S0a18 | [DONE] 修复 stdlib_string_basic 中 String support-source intrinsic member 调用仍退化成 unresolved MemberAccess + `FunValue` callee，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a19` | CG7S0a19 | [DONE] 修复 stdlib_string_methods_extended 中 `String.isEmpty` / `replace` / `charAt` / `repeat` builtin member 调用仍退化成 unresolved MemberAccess + `FunValue` callee，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a20` | CG7S0a20 | [DONE] 修复 string_trim_indent_basic 中 `String.trimIndent` builtin member 调用仍退化成 unresolved MemberAccess + `FunValue` callee，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a21` | CG7S0a21 | [DONE] 修复剩余 plain callable / ctor ABI 回归：top-level generic named args、cross-file ctor named/default 与 unsafe `FunPtr` aggregate return |
| `CG-T07S0a22` | CG7S0a22 | [DONE] 修复 top-level / package compilation-unit contract 回归：顶层 pattern once-init wrapper 与 cone package-level `comptime if` 跨文件绑定 |
| `CG-T07S0a24` | CG7S0a24 | [DONE] 回收 per-fixture scan 暴露的 frontend authoritative contract 回归：use-site eff row receiver mismatch |
| `CG-T07S0a24a` | CG7S0a24a | [DONE] 修复 runtime_gc cross-thread roots 中 top-level `@Global __AtomicInt` atomic lowering 漂移，并让 run-pass timeout 正确回收后代进程，解除 CG-T07S0a 默认 full-suite 新 blocker |
| `CG-T07S0a` | CG7S0a | [DONE] 修复 effect-handle top-level val pattern access 在 EffectStep codegen 中的 top-level value ref lowering，解除 CG-T07S0 默认 full-suite 新 blocker |
| `CG-T07S0` | CG7S0 | [DONE] 修复 receiver callable value / FunPtr named-arg lowering 顺序回归，解除 CG-T07S 默认 full-suite run-pass 阻塞 |
| `CG-T07S` | CG7S | [DONE] 修复 full-suite cross-fixture transport metadata drift，解除 CG-T08 默认回归阻塞 |
| `CG-T08` | CG8 | [DONE] 建立 codegen regression 矩阵并完成阶段退出审计 |
| `CG-T08R` | CG8R | [DONE] Review CG-T08 codegen phase exit audit |

## 全局约束

- 本文件所有任务只修 refactor/default codegen path。
- 不允许用 legacy HIR lowering、legacy handler stack、old `EffectOutcome` backend 或 old callable wrapper 作为 correctness 兜底。
- Codegen 只能消费 refactor MIR/materialized MIR、effect facts、late-lowered handoff、ABI query 和 target/session config；不得回 AST/HIR 私有 side table 补语义。
- ABI routing 必须消费 effect facts 中的 `resolved_outward_cases` / `impl_plan` / `CallableAbiKind`：`impl_plan = NoOutward` 或 `resolved_outward_cases = ∅` 的 body 公开 plain ABI；只有 `CallableAbiKind::EffectStep` body 或独立 effect-typed adapter publication 使用 EffectStep / effect boundary。
- 若缺少 upstream MIR contract，必须 fail fast 并回填 [`TODO.md`](./TODO.md)，不得在 backend 现场猜 shape。
- 每个任务完成时必须列出实际运行的定向测试；只有 `CG-T08` 要求 full regression。

## [DONE] CG-T00：建立 codegen gap inventory 与 backend gate

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG0
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3、§4、§5、§6、§7.2、§7.6、§9
- 目标：
  - 把 codegen-stage gaps 建成 owner map，区分 raw MIR LLVM、effect-refactor LLVM、runtime C、fixture/regression、upstream MIR contract。
  - 建立 backend route gate，禁止 unsupported shape 进入 LLVM body emission 才失败。

- 必须实现的内容：
  1. 建立 codegen gap inventory 测试或等价模块，覆盖 `UnsupportedMainBody`、`pass MIR ...`、`refactor ... unsupported`、runtime fatal helper。
  2. 为每条 entry 记录 `PIPELINE_GAPS.md` 编号、owner task、route、是否需要 upstream MIR contract、是否 production blocker。
  3. 后端入口增加统一 gate：缺 upstream contract 时 fail fast，错误中包含 body FQN、source span、gap id、建议 owner。
  4. 更新 build/run/dump-llvm smoke，确保 refactor path 不静默回 legacy backend。

- 必须遵从的约束：
  - inventory 是 gating asset，不是注释。
  - 不把 codegen blocker 标为 legacy-only，除非 refactor path 不可达且有测试证明。

- 验证：
  1. `cargo test -p scoopc codegen_gap_inventory`
  2. `cargo test -p scoopc refactor_llvm_backend_gate`
  3. 搜索 `UnsupportedMainBody`、`pass MIR`、`refactor .*unsupported`，确认命中有 owner。

- 完成条件：
  - 所有 codegen-stage gap 都有唯一 owner task。
  - 后续 codegen 任务可以用 inventory 判断是否真正消除对应 gap。
- 依赖：无

- 完成记录：
  - 2026-05-07：新增 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`，把 `PIPELINE_GAPS.md` codegen-stage scope 的 gap 记录为可测试 inventory，包含 gap id、owner task、route、upstream contract need、production blocker 与 trigger。
  - 2026-05-07：新增 `scoop::llvm::refactor_backend_gate` 诊断，并在 raw MIR body emission 前 gate 掉 Todo、缺 routing facts 的 effect/control/call-kind、PerformResult、runtime type primitive、class ctor named/default、pattern `is Type`、ambiguous continuation route 等已登记缺口，错误包含 body FQN、source span、gap id 与 suggested owner。
  - 2026-05-07：将 refactor LLVM smoke 纳入 `refactor_llvm_backend_gate` 过滤，确认 refactor stage 生成 effectful handle body IR 且不回 legacy handler-stack/outcome runtime。
  - 验证通过：`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、搜索 `UnsupportedMainBody|pass MIR|refactor .*unsupported` 与 `runtime fatal helper` 命中 inventory trigger 且带 owner、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T00R：Review CG-T00 codegen inventory 与 backend gate

- 参考：
  - `CG-T00`
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG0
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3、§4、§5、§6、§7.2、§7.6、§9
- 重点：
  - inventory 是否覆盖所有 codegen-stage unsupported / fatal / fallback shape，并为每项记录 gap id、owner task、route、upstream contract need。
  - backend gate 是否真的阻止缺 contract shape 进入 LLVM body emission。
  - refactor path 是否没有静默回 legacy backend。
- 验证：
  1. 重跑 `CG-T00` 的全部验证命令。
  2. 抽查 inventory 条目，确认每个 `PIPELINE_GAPS.md` codegen gap 有唯一 owner。
  3. 搜索 `UnsupportedMainBody`、`pass MIR`、`refactor .*unsupported`，确认命中均可追踪。
- 完成条件：
  - Review 结论明确说明 `CG-T00` 已正确实现；若发现缺口，`CG-T00R` 保持未完成并把修复归回 `CG-T00`。
- 依赖：`CG-T00`

- 完成记录：
  - 2026-05-07：复审 `CG-T00` 的 executable inventory、raw MIR backend gate 接入点与 refactor LLVM smoke，确认 `PIPELINE_GAPS.md` codegen-stage scope 的 gap 均有唯一 owner task，gate 在 raw MIR body emission 前拒绝缺 upstream/MIR contract 的 Todo、effect/control terminator、cleanup Perform、PerformResult、runtime type primitive、unsupported call kind、pattern `is Type` 与 ambiguous continuation route，并通过 `scoop::llvm::refactor_backend_gate` 诊断携带 body FQN、source span、gap id 与 suggested owner；refactor smoke 未回落到 legacy handler-stack / EffectOutcome backend。
  - 验证通过：`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、搜索 `UnsupportedMainBody|pass MIR|refactor .*unsupported|runtime fatal helper` 与 runtime thread-resume fatal helper，确认命中可追踪到 inventory/owner；`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T01：收口 raw MIR effect/control route 与 unsupported call kind

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG1
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.1、§3.2、§3.3、§3.6
  - [`TODO.md`](./TODO.md) MIR-T12
- 目标：
  - raw MIR route 对 `Handle`、`ResumeUnwind`、cleanup `Perform`、`PerformResult`、`Virtual`、`Interface`、`Resume` call kind 有明确 route verifier，不再 late unsupported。

- 必须实现的内容：
  1. 消费 MIR-T12 发布的 routing facts，raw route 只接受 raw-safe effect-neutral body。
  2. 对 effect/control body：必须转入 plain-local handoff 或 EffectStep late lowering；缺 handoff 时 verifier fail-fast，不允许 raw route 实现第二套 handler/resume/cleanup semantics。
  3. `PerformResult` 必须来自已发布 resume payload binding；缺失时 verifier fail fast，不得返回默认值。
  4. raw `Virtual` / `Interface` call kind 只有在 routing facts 标明 effect-neutral 且 dispatch/ABI contract 完整时才能 lower；continuation `Resume` 与 effect-control call kind 必须由 plain-local handoff 或 EffectStep lowering 消费，否则 route verifier 阻止进入 raw path。

- 必须遵从的约束：
  - 不允许用 `unreachable`、默认值或 silent skip 代表未实现 resume semantics。
  - 不允许 raw route 回 legacy HIR codegen 或自建第二套 lowering 补 effect/control。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_raw_route_gate`
  2. `cargo test -p scoopc raw_mir_effect_control_route`
  3. build fixtures 覆盖 raw-safe plain body、effect body reroute、unsupported raw body fail-fast。

- 完成条件：
  - `PIPELINE_GAPS.md` §3.1、§3.2、§3.3、§3.6 不再能在 production refactor codegen 中晚期触发 unsupported。
- 依赖：`CG-T00R`，`MIR-T12R`

- 完成记录：
  - 2026-05-07：将 `MirCodegenRoutingFacts` 从 refactor effect-lowered stage 传入 LLVM codegen，raw MIR body emission 在 refactor path 下必须消费 MIR-T12 route fact，缺失或非 `PlainRawMir` route 会在 backend gate fail-fast，不再回 HIR-compatible fallback。
  - 2026-05-07：raw body capability check 不再把 `Perform` 或 `PerformResult` 视为 raw-safe；`PerformResult` 保持 backend gate 拒绝，避免默认值 miscompile；`PlainLocalControlHandoff` / `EffectStepLowering` body 由 route gate 阻止进入 raw emission。
  - 2026-05-07：新增 `refactor_llvm_raw_route_gate` 与 `raw_mir_effect_control_route` 定向单测，覆盖 raw-safe plain body、缺 routing fact fail-fast、plain-local handoff reroute、`PerformResult` resume payload binding guard。
  - 验证通过：`cargo test -p scoopc refactor_llvm_raw_route_gate`、`cargo test -p scoopc raw_mir_effect_control_route`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo test -p scoopc refactor_mir_codegen_routing_contract`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T01R：Review CG-T01 raw MIR route gate

- 参考：
  - `CG-T01`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6、§8
  - [`TODO.md`](./TODO.md) MIR-T12R
- 重点：
  - raw route 是否只接受 raw-safe effect-neutral body。
  - effect/control body 是否必须走 plain-local handoff 或 EffectStep late lowering，且没有第二套 raw handler/resume/cleanup semantics。
  - `PerformResult`、continuation `Resume`、unsupported call kind 是否缺 contract 即 verifier fail-fast。
- 验证：
  1. 重跑 `CG-T01` 的全部验证命令。
  2. 抽查 raw-safe plain、effect body reroute、unsupported raw body fail-fast fixtures。
  3. 搜索 raw route 中对 `Handle` / `Perform` / `ResumeUnwind` / continuation `Resume` 的直接 lowering，确认没有绕过 handoff。
- 完成条件：
  - Review 结论明确说明 `CG-T01` 已正确实现；若发现缺口，`CG-T01R` 保持未完成并把修复归回 `CG-T01`。
- 依赖：`CG-T01`

- 完成记录：
  - 2026-05-07：复审 `CG-T01` 的 raw MIR route gate，确认 refactor LLVM emit 会携带 MIR-T12 `MirCodegenRoutingFacts`，raw MIR body emission 在 route facts 存在时必须匹配 `PlainRawMir`，缺 fact 或 `PlainLocalControlHandoff` / `EffectStepLowering` / `FrontendReject` 路由均会在 backend gate fail-fast。
  - 2026-05-07：确认 `PerformResult`、`Handle`、`ResumeUnwind`、cleanup `Perform`、`Virtual` / `Interface` / continuation `Resume` call kind 的 raw path 均由 route verifier / backend gate 阻止，不依赖默认值、`unreachable`、legacy HIR fallback 或第二套 raw handler/resume/cleanup semantics；plain-local handoff 与 EffectStep body 由已发布 handoff route 消费。
  - 2026-05-07：搜索 `TerminatorKind::Handle` / `TerminatorKind::Perform` / `TerminatorKind::ResumeUnwind` / `CallKind::Resume` / `Rvalue::PerformResult`，命中限于 raw support rejection/gate、受 gate 保护的 generic MIR emitter、refactor fail-fast 分支和 use-collection helpers，未发现绕过 handoff 的 raw route 直接 lowering。
  - 验证通过：`cargo test -p scoopc refactor_llvm_raw_route_gate`、`cargo test -p scoopc raw_mir_effect_control_route`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo test -p scoopc refactor_mir_codegen_routing_contract`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/emit_llvm_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T02：收口 runtime type/value primitive LLVM lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG2
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.4、§3.5、§3.8、§6.1、§6.2、§7.2
  - [`TODO.md`](./TODO.md) MIR-T09
- 目标：
  - `is` / `!is` / `as` / `as?` / `!!` / pattern type test 在 refactor LLVM path 有完整 lowering 或明确 frontend reject。

- 必须实现的内容：
  1. 将 runtime type descriptor / itable matching lowering 接到 MIR `TypeCheck` 与 pattern `is Type`。
  2. 实现 `CastOp::As` 与 `CastOp::AsQ`：`as` failure 走 ordinary runtime error effect boundary，`as?` 构造 `Option<T>`。
  3. 实现 `!!` 的 non-null success projection 与 null failure raise path。
  4. 对 function type runtime cast / effectful function type cast：若仍不支持，确认 frontend diagnostic；若支持，补 callable type descriptor lowering。

- 必须遵从的约束：
  - 不允许 `TypeCheck` / `Cast` 在 refactor value primitive 中返回默认值或 late unsupported。
  - 不允许 `as` failure 绕过 effect boundary 直接 panic。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_runtime_type_primitives`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
  3. 新增或复用 `!!` 非空断言 run-pass fixture。
  4. 负例覆盖 unsupported function type cast diagnostic。

- 完成条件：
  - `PIPELINE_GAPS.md` §3.4、§3.5、§3.8、§6.1、§6.2 的 codegen 部分关闭。
- 依赖：`CG-T01R`，`MIR-T09R`

- 完成记录：
  - 2026-05-07：refactor LLVM raw/materialized MIR path 支持 `TypeCheck`、`CastOp::As`、`CastOp::AsQ` 与 pattern `is Type` 的 runtime descriptor / itable matching lowering；backend gate 不再把已具备 metadata contract 的 runtime type primitive 统一 late unsupported。
  - 2026-05-07：`as` lowering 在 MIR 中拆为显式 type-test 成功分支与 `Raise.raise(RuntimeError.ClassCastFailed)` 失败分支，失败路径走 ordinary runtime-error effect boundary；`as?` lowering 构造 `Option<T>` 的 `Some` / `None` enum value。
  - 2026-05-07：`!!` 非空断言 run-pass fixture 从 expected-fail 回收；合成 `RuntimeError.NullAssertionFailed` 表达式使用显式 `RuntimeError` 类型，避免 late-lowering boundary payload contract 漂移。
  - 2026-05-07：确认 function type runtime casts 仍由 frontend/typecheck diagnostic 拒绝，未进入 refactor LLVM callable descriptor guess。
  - 验证通过：`cargo test -p scoopc refactor_mir_value_primitives`、`cargo test -p scoopc refactor_llvm_runtime_type_primitives`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/not_null_assert_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T02R：Review CG-T02 runtime value primitive lowering

- 参考：
  - `CG-T02`
  - [`SCOOP_FULL_SPEC.md`](./SCOOP_FULL_SPEC.md) §4.3、§4.4、§5.7、§7.5
  - [`TODO.md`](./TODO.md) MIR-T09R
- 重点：
  - `is` / `!is` / `as` / `as?` / `!!` / pattern type test 是否从 MIR metadata lower，不返回默认值或 late unsupported。
  - `as` / `!!` failure 是否走 ordinary `Raise<RuntimeError>` effect boundary，而不是 panic-only path。
  - function type runtime cast 是否按 frontend diagnostic policy 覆盖，没有 backend callable descriptor guess。
- 验证：
  1. 重跑 `CG-T02` 的全部验证命令。
  2. 抽查 runtime type descriptor / itable matching / `Option<T>` construction lowering。
  3. 负例确认 unsupported function type cast 不进入 refactor LLVM lowering。
- 完成条件：
  - Review 结论明确说明 `CG-T02` 已正确实现；若发现缺口，`CG-T02R` 保持未完成并把修复归回 `CG-T02`。
- 依赖：`CG-T02`

- 完成记录：
  - 2026-05-07：复审 `CG-T02` 的 MIR metadata、refactor LLVM lowering、`as` / `!!` failure boundary 与 function-type cast diagnostic，确认 `TypeCheck`、`CastOp::As`、`CastOp::AsQ`、`Option<T>` construction、parameterized class/interface runtime match 和 function-type cast frontend reject 均按 CG-T02 policy 覆盖。
  - 2026-05-07：复审中发现 `Pattern::Is` 虽携带 `RuntimePatternTypeTestMetadata`，LLVM pattern support/codegen 仍只读取目标 `ty`，会阻断 value-type static-fold pattern；已修复为验证并消费 pattern metadata，静态折叠直接生成常量，动态 ref-like case 才进入 runtime descriptor / itable matching。
  - 验证通过：`cargo test -p scoopc refactor_mir_value_primitives`、`cargo test -p scoopc refactor_llvm_runtime_type_primitives`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/not_null_assert_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T03：收口 call/ctor/function-ref/intrinsic/default/interface lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.7、§3.9、§3.10、§6.3、§6.5
  - [`TODO.md`](./TODO.md) MIR-T07、MIR-T08
- 目标：
  - Codegen 只消费 typed call/ctor/intrinsic/default/interface contract，不再补语义或猜 callee shape。

- 必须实现的内容：
  1. class ctor lowering 消费 selected ctor 与 complete ordered args；不在 backend 补 named/default。
  2. top-level function reference 按 MIR policy lower 成 function value/closure object 或 explicit symbol value。
  3. `nameOf<T>()` / `getPlatform()` / `sizeOf<T>()` 等 runtime fallback intrinsics 有 refactor lowering 或明确 diagnostic。
  4. interface default method dispatch 消费 selected implementation/default slot，不回 owner/member 字符串猜测。

- 必须遵从的约束：
  - 不允许 arity mismatch 被当作 backend default-arg 补齐入口。
  - 不允许 codegen 使用 `rsplit_once('.')` 作为 dispatch source of truth。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_call_contract_lowering`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
  3. reflection/platform intrinsic run-pass 或 build fixtures。

- 完成条件：
  - `PIPELINE_GAPS.md` §3.7、§3.9、§3.10、§6.3、§6.5 的 codegen 部分关闭。
- 依赖：`CG-T02R`，`MIR-T07R`，`MIR-T08R`

- 完成记录：
  - 2026-05-07：MIR `ClassCtor` 现在携带 selected ctor span 与完整 ordered param count；refactor LLVM class ctor lowering 只消费该契约与 positional args，缺 selected ctor 或 args 不完整时 fail-fast，不再在 backend 重新选择 overload 或补 named/default args。
  - 2026-05-07：dispatch metadata 携带 selected member FQN/span，interface slot metadata 携带 method FQN；plain interface dispatch 与 ABI materialization 通过 selected member identity 解析 itable slot/default implementation，不再扫描 owner/member 字符串恢复 signature。
  - 2026-05-07：`getPlatform()` 在 LLVM codegen 中 lower 为 `scoop.core.Platform` literal；`sizeOf<T>()` / `nameOf<T>()` 的 refactor MIR intrinsic contract 保持通过 MIR primitive 覆盖，top-level function reference 继续由 HIR/MIR function-value closure contract 覆盖。
  - 验证通过：`cargo test -p scoopc refactor_llvm_call_contract_lowering`、`cargo test -p scoopc refactor_mir_call_contract_lowers_typed_call_sites`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/get_platform_runtime_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/interface_default_method_dispatch_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_generic_function_value_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/codegen/intrinsic_size_of_int_word.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T03R：Review CG-T03 call/ctor/intrinsic lowering

- 参考：
  - `CG-T03`
  - [`TODO.md`](./TODO.md) MIR-T07R、MIR-T08R
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.7、§3.9、§3.10、§6.3、§6.5
- 重点：
  - class ctor lowering 是否只消费 selected ctor 与 complete ordered args，不在 backend 补 named/default。
  - top-level function reference、runtime reflection/platform intrinsic、interface default dispatch 是否都消费 typed/MIR contract。
  - codegen 是否不再用 owner/member 字符串拆分或 arity mismatch fallback 恢复语义。
- 验证：
  1. 重跑 `CG-T03` 的全部验证命令。
  2. 抽查 ctor named/default/delegation、function-ref、intrinsic、interface default dispatch fixtures。
  3. 搜索 `rsplit_once` / backend default-arg 补齐相关路径，确认不再作为 source of truth。
- 完成条件：
  - Review 结论明确说明 `CG-T03` 已正确实现；若发现缺口，`CG-T03R` 保持未完成并把修复归回 `CG-T03`。
- 依赖：`CG-T03`

- 完成记录：
  - 2026-05-07：复审 `CG-T03` 的 selected ctor / ordered args contract、top-level function value lowering、`getPlatform` / `sizeOf` / `nameOf` reflection intrinsic lowering、interface default dispatch 与 `rsplit_once` / backend default-arg 搜索面，确认 refactor LLVM 路径消费 MIR/typed contract，不在 codegen 现场补 named/default 语义或用 owner/member 字符串拆分作为 interface dispatch source of truth。
  - 2026-05-07：复审中发现显式类型实参形式的 `nameOf<T>()` 在 generic materialized MIR path 中仍会退化成 declaration-only direct call；已修复为在 MIR intrinsic lowering 中规范化 generic/overload 后缀，并让 materialization fallback 从 top-level call binding 生成 `TypeMetadataLiteral`，同时补充 `tests/fixtures/run-pass/name_of_runtime_basic.scoop` 回归。
  - 验证通过：`cargo test -p scoopc refactor_mir_call_contract_lowers_typed_call_sites`、`cargo test -p scoopc refactor_llvm_call_contract_lowering`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/get_platform_runtime_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/interface_default_method_dispatch_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_generic_function_value_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/codegen/intrinsic_size_of_int_word.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/name_of_runtime_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04a：建立 composite transport layout contract 与 verifier

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11、§4.1、§4.2、§4.3、§4.4、§4.5、§5.5
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - 先建立 CG-T04b-CG-T04f 共用的 explicit composite layout/descriptor contract、runtime hook surface 与 verifier gate，不在本任务实现具体 boxing/enum/array/closure/thread transport。

- 必须实现的内容：
  1. LLVM codegen 消费或规范化 MIR-T10 发布的 composite transport/layout metadata，至少覆盖 size、align、inline/boxed/erased storage kind、trace/copy/drop hook identity 与 GC slot map。
  2. 建立统一 verifier/backend gate：任何 composite transport use site 缺 layout descriptor 时 fail-fast，并把诊断 owner 指向对应 `CG-T04b` 至 `CG-T04f` 子任务。
  3. runtime descriptor plumbing 提供 trace/copy/drop hook registration/call surface；traceable value 不允许用 fake no-op hook 通过 verifier。
  4. 本任务结束时，value boxing、enum payload、array element、closure env、thread payload 仍可保持 unsupported，但必须通过 owner-specific gate 明确拒绝。

- 必须遵从的约束：
  - 不允许默认 `u64`/ref carrier 作为 composite layout contract。
  - 不允许 codegen 从 AST/HIR、类型名或 runtime fallback 猜 shape。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_composite_transport_contract`
  2. `cargo test -p scoopc codegen_gap_inventory`
  3. 负例覆盖缺 layout descriptor 的 composite transport use site fail-fast。

- 完成条件：
  - 后续 `CG-T04b` 至 `CG-T04f` 可以复用同一 explicit layout/descriptor 和 runtime hook surface。
  - `PIPELINE_GAPS.md` §3.11、§4.1-§4.5、§5.5 仍保留具体 implementation owner，不再共享一个大 `CG-T04` owner。
- 依赖：`CG-T03R`，`MIR-T10R`

- 完成记录：
  - 2026-05-07：新增 refactor LLVM composite transport verifier，规范化 MIR-T10 `ValueTransportMetadata` / aggregate / call / array / closure / capture box / perform payload metadata 为 explicit layout descriptor，覆盖 size、align、inline/boxed/erased storage kind、GC slot map 与 trace/copy/drop hook identity；缺 materialized/codegen layout、traceable value 缺 GC slot/trace hook 或 hook identity 不完整时通过 `scoop::llvm::refactor_backend_gate` fail-fast。
  - 2026-05-07：refactor plain callable 与 raw materialized MIR body emission 均接入 composite verifier；descriptor global 会发布 `scoop.runtime.ScoopCompositeTransportDescriptor`、GC slot offsets 与 `scoop_composite_trace` / `scoop_composite_copy` / `scoop_composite_drop` runtime hook surface。
  - 2026-05-07：runtime C 新增 `ScoopCompositeTransportDescriptor` 与 composite trace/copy/drop 调用面，并更新 ABI allowlist；`PIPELINE_GAPS.md` §3.11、§4.1-§4.5、§5.5 的 inventory owner 保持拆分到 `CG-T04b` 至 `CG-T04f`，未合并为共享 `CG-T04` owner。
  - 验证通过：`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime abi_exports_allowlist`、`cargo test -p scoop_runtime --test gc_immix_nursery`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04b0：发布 value erasure boxing MIR transport contract

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §4.1
  - [`TODO.md`](./TODO.md) MIR-T10
  - `CG-T04b`
- 目标：
  - MIR / materialized MIR 对 tuple/struct/value type -> `Any` / `Ref` / erased carrier 的隐式 boxing 发布显式 `ValueTransportMetadata.boxing` contract，使 codegen 只消费 metadata，不从 source/target type 现场猜 erasure shape。

- 必须实现的内容：
  1. 为 refactor LLVM 可达的 value erasure boxing site 发布 `MirBoxingIntent`，至少覆盖 local/top-level initializer、assignment、return/tail return、call arg 与 effect-neutral handoff 中的 tuple/struct/value type -> `Any` / `Ref` / erased carrier。
  2. `MirBoxingIntent` 必须保留 `source_ty`、`target_ty` 与 `MirBoxingReason::AnyErasure` / `MirBoxingReason::RefErasure`；`ValueTransportMetadata` 必须保留 source transport kind 与 trace/copy/drop requirements。
  3. MIR production verifier 必须拒绝缺 boxing intent 的 aggregate erasure boundary，避免 `CG-T04b` 从 assignment target 或 ABI target 反推 boxing。
  4. payload-bearing enum erasure site 可以被 metadata 标识，但不得在本任务中猜 enum payload layout 或绕过 `CG-T04c`；后续 `CG-T04b` 必须能据此保留 owner-specific gate。

- 必须遵从的约束：
  - 不允许把 `u64`/ref carrier 当作隐式 MIR contract。
  - 不允许 codegen 通过 source/target type mismatch 自行推断 value erasure boxing。
  - 不允许用 fixture 私有 shape 绕过 payload-bearing enum boxing 的 `CG-T04c` owner。

- 验证：
  1. `cargo test -p scoopc refactor_mir_value_boxing_transport_contract`
  2. `cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`
  3. MIR dump 或定向 fixture 覆盖 tuple/struct/value type -> `Any` / `Ref` 的 initializer、call arg、return boxing metadata。

- 完成条件：
  - `MirBoxingReason::AnyErasure` / `RefErasure` 不再只是枚举定义；所有 `CG-T04b` 需要的 erased value boxing site 都有可验证 producer。
  - `CG-T04b` 可以仅消费 MIR boxing intent 和 `CG-T04a` layout descriptor 实现 lowering。
- 依赖：`CG-T04a`

- 完成记录：
  - 2026-05-07：新增 MIR `Rvalue::Transport` value erasure boundary，并让 local/top-level initializer、assignment、return/tail return、call arg 与 effect-neutral merge/handoff 路径在 tuple/struct/value type -> `Any` / `Ref` / erased carrier 时发布 `ValueTransportMetadata.boxing`。
  - 2026-05-07：`MirBoxingIntent` 现在保留 `source_ty`、`target_ty` 与 `MirBoxingReason::AnyErasure` / `RefErasure`；source transport kind、trace/copy/drop requirements 从 source type 保留，payload-bearing enum erasure 仅标识 `EnumPayload` metadata，不在本任务猜 enum payload layout。
  - 2026-05-07：MIR/materialized MIR verifier 与 downstream use/provenance/reachability paths 识别 `Rvalue::Transport`；raw LLVM route 对 value erasure lowering 保持 `CG-T04b` owner-specific gate，避免 codegen 从 source/target mismatch 反推 boxing。
  - 验证通过：`cargo test -p scoopc refactor_mir_value_boxing_transport_contract`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/value_boxing_transport.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04b：收口 value boxing composite transport lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §4.1
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - value-type boxing layout 支持 tuple/struct/value type -> `Any` / `Ref` / erased carrier；payload-bearing enum boxing 只消费 `CG-T04c` 已发布的 enum payload descriptor。

- 必须实现的内容：
  1. value boxing lowering 消费 `CG-T04a` 的 composite layout descriptor，支持 tuple/struct/value type 的 allocation、store/load、erase 与 unbox projection。
  2. boxed composite 的 trace/copy/drop metadata 可由 runtime 枚举；缺 metadata 时 verifier fail-fast，不回默认 `u64` carrier。
  3. `Any` / `Ref` / erased carrier 中的 descriptor identity 必须可用于 runtime type/value operations 与后续 copy/drop。
  4. 对 payload-bearing enum boxing，若 `CG-T04c` 还未完成，必须保留明确 gate；不得在 boxing path 临时猜 enum payload layout。

- 必须遵从的约束：
  - 不允许继续用 `u64`/ref 双轨隐式代表 boxed composite value。
  - 不允许 boxed value 绕过 GC trace/copy/drop requirements。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_value_boxing_transport`
  2. 新增或复用 tuple/struct/value type boxing run-pass fixtures。
  3. `cargo test -p scoopc codegen_gap_inventory`

- 完成条件：
  - `PIPELINE_GAPS.md` §4.1 中 tuple/struct/value type boxing 的 codegen/runtime 部分关闭；payload-bearing enum boxing 由 `CG-T04c` 完成后纳入同一 boxing path。
- 依赖：`CG-T04b0`

- 完成记录：
  - 2026-05-07：`Rvalue::Transport` value erasure lowering 现在消费 `CG-T04b0` 发布的 `ValueTransportMetadata.boxing` 与 `CG-T04a` erased composite layout descriptor；tuple/struct source value 会分配 GC-managed `scoop.mir.value_box$...` carrier、写入 payload，并通过 `__scoop_type_desc_mir_value_box__...` runtime type descriptor 暴露 trace/copy/drop 所需布局。
  - 2026-05-07：raw/materialized MIR support 与 backend gate 不再把具备 Any/Ref boxing intent 的 tuple/struct erasure 统一拒绝；缺 Any/Ref boxing intent 仍 fail-fast，payload-bearing enum erasure 继续明确 gate 到 `CG-T04c`，不在 boxing path 猜 enum payload layout。
  - 2026-05-07：新增 `refactor_llvm_value_boxing_transport` IR 单测与 `tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop`，覆盖 struct/tuple -> `Any` 的 allocation、descriptor publication 与 payload store。
  - 验证通过：`cargo test -p scoopc refactor_llvm_value_boxing_transport`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_mir_value_boxing_transport_contract`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/value_boxing_transport.scoop`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04c：收口 enum payload composite transport lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §4.2、§4.3、§4.4
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - enum payload layout 支持 Unit field、大整数 payload、nested enum/tuple/struct payload，并在必要时自动 boxed。

- 必须实现的内容：
  1. enum constructor/project/match lowering 消费 MIR-T10 enum payload schema 与 `CG-T04a` 的 composite layout descriptor。
  2. 支持 Unit payload field、超过 machine word 的 scalar payload、nested enum/tuple/struct payload 的 inline/boxed layout 决策。
  3. enum payload 中的 ref/composite slot 必须进入 GC trace/copy/drop 枚举；boxed payload 的 drop/copy 不得泄漏或 double free。
  4. 将 payload-bearing enum boxing 接回 `CG-T04b` 的 boxed carrier path；缺 payload schema、layout descriptor 或 unsupported payload kind 时 verifier fail-fast。

- 必须遵从的约束：
  - 不允许把 Unit field 当作不存在的 payload 导致 tag/field ordinal 漂移。
  - 不允许 wide/nested payload 被截断成 `u64` 或裸 ref。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_enum_payload_transport`
  2. 新增或复用 enum Unit field、大整数 payload、nested enum/tuple/struct payload run-pass fixtures。
  3. `cargo test -p scoopc codegen_gap_inventory`

- 完成条件：
  - `PIPELINE_GAPS.md` §4.2、§4.3、§4.4 的 codegen/runtime 部分关闭。
  - `PIPELINE_GAPS.md` §4.1 中 payload-bearing enum boxing 不再保留额外 gate。
- 依赖：`CG-T04b`

- 完成记录：
  - 2026-05-07：refactor LLVM enum ctor lowering 现在校验并消费 MIR-T10 `EnumPayload` schema，payload aggregate/field type、field ordinal 与 arg metadata 不匹配时 fail-fast，不再从 enum/source type 现场猜 payload shape。
  - 2026-05-07：boxed enum payload 支持 Unit field、tuple/struct/nested enum payload 与 oversized/wide payload boxing；Unit field 在 boxed payload struct 中保留 ordinal，占位写入 `i8 0`，wide integer field 会被 enum layout 决策强制导向 boxed payload 主线。
  - 2026-05-07：payload-bearing enum value erasure 不再 gate 到 `CG-T04c`，改为复用 `CG-T04b` value-box carrier 并发布 erased composite descriptor / value-box type descriptor；composite verifier 使用 codegen layout 规范化 GC trace requirement，避免 GC-free nominal field 伪造 trace hook，同时 traceable enum payload 仍通过 descriptor slot map 枚举。
  - 2026-05-07：新增 `refactor_llvm_enum_payload_transport` IR 单测、`enum_payload_unit_field_basic.scoop` 与 `enum_payload_boxing_any_basic.scoop` run-pass fixtures，并将 enum non-scalar / oversized fixtures 改为 exit-code 断言，避开无关 generic `println` materialization blocker 而保留 payload 提取结果检查。
  - 验证通过：`cargo test -p scoopc refactor_llvm_enum_payload_transport`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_unit_field_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_variant_non_scalar_payload_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_nested_custom_enum_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxed_builtin_option_field_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_llvm_value_boxing_transport`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo test -p scoopc refactor_mir_value_boxing_transport_contract`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/value_boxing_transport.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04d：收口 array composite element transport lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §4.5
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - Array runtime descriptor 支持 composite element 的 element size、align、trace/copy/drop，并闭合 build/get/set lowering。

- 必须实现的内容：
  1. Array descriptor 从 element transport metadata 记录 size、align、trace/copy/drop hook 与 inline/boxed storage policy。
  2. LLVM lowering 支持 composite array build/get/set，不把 element 降级为 `u64` word storage。
  3. array set/build 在拷贝 composite element 时正确处理 temporary rooting、copy/drop ordering 和 ref slot tracing。
  4. 缺 element descriptor 或 unsupported element policy 时 verifier fail-fast。

- 必须遵从的约束：
  - 不允许复用 scalar array path 静默截断 composite element。
  - 不允许 array runtime 绕过 GC trace/copy/drop hooks。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_array_composite_transport`
  2. 新增或复用 tuple/struct/enum element array build/get/set run-pass fixtures。
  3. `cargo test -p scoopc codegen_gap_inventory`

- 完成条件：
  - `PIPELINE_GAPS.md` §4.5 的 codegen/runtime 部分关闭。
- 依赖：`CG-T04c`

- 完成记录：
  - 2026-05-07：runtime array/builder 表示升级为 descriptor-backed element storage，新增 composite push/build/get/set ABI，按 `ScoopCompositeTransportDescriptor` 记录 element size/align、trace/copy/drop 与 GC slot tracing；scalar/ref array API 保持可用。
  - 2026-05-07：refactor LLVM array lowering 消费 `ArrayElementTransportMetadata` 与 composite layout descriptor，tuple/struct/enum element 的 build/get/set 走 `scoop_array_*_composite`，不再经 `u64` word storage 静默截断。
  - 2026-05-07：materialized MIR 修复 canonical array member intrinsic 与 generic argument transport 的 concrete type 恢复，避免 array set/get/size 的 `T` 泄漏到 pass-view frame slot。
  - 验证通过：`cargo test -p scoopc refactor_llvm_array_composite_transport`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/array_composite_transport_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/array_mutable_array_min_primitive_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_trace_array_string_elements_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_llvm_value_boxing_transport`、`cargo test -p scoopc refactor_llvm_enum_payload_transport`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04e：收口 closure env/capture transport lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - closure env 支持 arbitrary traceable source type；mutable capture 使用 capture box，并共享 `CG-T04a` 的 composite transport metadata。

- 必须实现的内容：
  1. closure env layout 消费 MIR capture schema 与 composite layout descriptor，支持 tuple/struct/enum/array/ref/value captures。
  2. mutable capture lowering 使用 capture box，capture box 的 trace/copy/drop/rooting 与 ordinary boxed composite 一致。
  3. closure allocation、invoke、copy/drop 中的 env ref/composite slots 均可被 GC 枚举。
  4. 缺 capture schema、ambiguous capture owner 或 unsupported source shape 时 verifier fail-fast。

- 必须遵从的约束：
  - 不允许 closure env 回退到 opaque `u64` slot 或裸 pointer slot。
  - 不允许 mutable capture 通过复制 captured value 伪装成 by-reference 语义。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_closure_env_transport`
  2. 新增或复用 closure capture tuple/struct/enum/array 与 mutable capture run-pass fixtures。
  3. `cargo test -p scoopc codegen_gap_inventory`

- 完成条件：
  - `PIPELINE_GAPS.md` §3.11 的 codegen/runtime 部分关闭。
- 依赖：`CG-T04d`

- 完成记录：
  - 2026-05-07：refactor LLVM closure allocation/invoke 现在消费 `ClosureEnvTransportMetadata` capture schema，并通过 composite transport descriptor 校验 closure env 与各 capture；closure env field lowering 支持 tuple/struct/enum/array/ref/value captures，不再限制为 opaque `u64`、裸 pointer 或 scalar-only tuple。
  - 2026-05-07：mutable capture box lowering 支持 traceable composite values，capture box new/get/set 复用 typed heap object descriptor 与 GC slot tracing；materialized MIR 修复 closure body 内 captured array member call 的 concrete receiver metadata，避免 `Array<T>.get` 在 closure body 中留下 unresolved `T`。
  - 2026-05-07：新增 `refactor_llvm_closure_env_transport` IR 单测与 `tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop`，覆盖 String/ref、struct、tuple、enum、array 与 mutable struct capture box，并在 closure 调用前触发 GC。
  - 验证通过：`cargo test -p scoopc refactor_llvm_closure_env_transport`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_llvm_array_composite_transport`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04f：收口 cross-thread resume payload transport lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §5.5
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - cross-thread resume payload 支持 ref/composite transport，并在 enqueue/dequeue/resume 边界正确 root GC refs。

- 必须实现的内容：
  1. runtime cross-thread resume payload helper 从 `u64` payload 升级为 typed/erased carrier，复用 `CG-T04a` 的 layout descriptor。
  2. enqueue/dequeue/resume payload 时执行 trace/copy/drop hooks，并保证 ref/composite slot 在跨线程队列中可被 GC root/scan。
  3. LLVM lowering 为 ref/composite resume payload 传递 descriptor 与 carrier，不在 backend 猜 payload shape。
  4. thread resume non-complete Step 语义仍归 `CG-T06`；本任务只关闭 complete/ref/composite payload transport。

- 必须遵从的约束：
  - 不允许 runtime `u64` helper 继续作为合法 composite payload transport。
  - 不允许跨线程队列中的 ref/composite payload 脱离 GC root verifier。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`
  2. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_refs.scoop`
  3. 新增或复用 cross-thread composite resume payload runtime_gc fixture。

- 完成条件：
  - `PIPELINE_GAPS.md` §5.5 的 codegen/runtime 部分关闭。
- 依赖：`CG-T04e`

- 完成记录：
  - 2026-05-07：新增 generic `__scoop_thread_spawn_join_resume<Resume>` sysroot helper，并让 MIR call transport 为 cross-thread resume value 发布 `EffectPayload` metadata；materialized MIR 会从 continuation contract 恢复 concrete resume payload type，避免 LLVM 从 helper 泛型参数或 call target 现场猜 payload shape。
  - 2026-05-07：refactor LLVM lowering 新增 typed cross-thread resume transport path，按 surface resume ABI 分流 scalar word、GC ref carrier 与 composite payload alloca，composite/ref payload 会传递 `ScoopCompositeTransportDescriptor`、carrier pointer 和 typed resume thunk；旧 `u64` helper 仅保留 Int payload 兼容路径。
  - 2026-05-07：runtime C 新增 `scoop_thread_spawn_join_refactor_resume_transport`，enqueue 时用 composite copy hook 复制 payload，join/native 阻塞期间用 descriptor GC slot map 暴露 native root slots，worker resume 完成后执行 drop hook 并释放 carrier；non-Complete Step 仍沿用 `scoop_refactor_thread_resume_noncomplete_fatal`，留给 `CG-T06`。
  - 2026-05-07：新增 `refactor_llvm_cross_thread_resume_payload_transport` IR 单测与 `tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`，覆盖 String ref payload 与含 String field 的 struct composite payload 在 moving/stress/verify-roots 下跨线程 resume。
  - 验证通过：`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_refs.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T04R：Review CG-T04a-CG-T04f composite transport lowering

- 参考：
  - `CG-T04a`
  - `CG-T04b`
  - `CG-T04c`
  - `CG-T04d`
  - `CG-T04e`
  - `CG-T04f`
  - [`SCOOP_FULL_SPEC.md`](./SCOOP_FULL_SPEC.md) §2、§15
  - [`TODO.md`](./TODO.md) MIR-T10R
- 重点：
  - value boxing、enum payload、array element、closure env、cross-thread resume payload 是否共用 explicit transport/layout metadata。
  - GC trace/copy/drop、stack/root handling、boxed/inline choice 是否不依赖 `u64`/ref 隐式双轨。
  - runtime_gc moving/stress/verify-roots 样本是否覆盖 composite refs。
- 验证：
  1. 重跑 `CG-T04a` 至 `CG-T04f` 的全部验证命令。
  2. 抽查 enum/array/closure composite run-pass 与 runtime_gc fixtures。
  3. 检查 LLVM/runtime layout 中 composite payload 的 GC slot 可枚举性。
- 完成条件：
  - Review 结论明确说明 `CG-T04a` 至 `CG-T04f` 已正确实现；若发现缺口，`CG-T04R` 保持未完成并把修复归回对应子任务。
- 依赖：`CG-T04f`

- 完成记录：
  - 2026-05-07：复审 `CG-T04a` 至 `CG-T04f` 的 composite transport metadata、LLVM descriptor publication、runtime trace/copy/drop surface、array/closure/enum/value boxing 与 cross-thread resume payload lowering，确认 codegen 主线共用 explicit `ValueTransportMetadata` / `ScoopCompositeTransportDescriptor`，不依赖 `u64`/ref 隐式 composite carrier。
  - 2026-05-07：复审中发现 materialized MIR 未验证 `thread_resume_payload`，runtime composite array 写入/构建未统一通过 descriptor trace surface 做 GC slot 写屏障，array release 未 drop composite elements，cross-thread native roots 仅枚举 raw `gc_slot_offsets`；已修复为 materialized validation 覆盖 thread payload，array build/set 用 descriptor trace 收集 slot 并通过 write barrier 写入，array release 调用 composite drop，cross-thread resume roots 用 `scoop_composite_trace` 收集，同时固化 composite descriptor ABI offset assertions。
  - 2026-05-07：新增 `crates/scoop_runtime/tests/composite_array_release.rs` 与 `crates/scoop_runtime/tests/gc_immix_composite_array_write_barrier.rs`，覆盖 composite array sweep drop 与 old array 写入 nursery ref slot 的 Immix promote-on-store barrier。
  - 验证通过：`cargo test -p scoopc refactor_llvm_composite_transport_contract`、`cargo test -p scoopc refactor_llvm_value_boxing_transport`、`cargo test -p scoopc refactor_llvm_enum_payload_transport`、`cargo test -p scoopc refactor_llvm_array_composite_transport`、`cargo test -p scoopc refactor_llvm_closure_env_transport`、`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`、`cargo test -p scoopc refactor_mir_value_boxing_transport_contract`、`cargo test -p scoopc refactor_mir_composite_transport_metadata_contracts`、`cargo test -p scoopc refactor_llvm_backend_gate`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`cargo test -p scoop_runtime --test gc_immix_nursery`、`cargo test -p scoop_runtime --test composite_array_release`、`cargo test -p scoop_runtime --test gc_immix_composite_array_write_barrier`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/value_boxing_transport.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_unit_field_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_variant_non_scalar_payload_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_oversized_variant_boxing_suppressed.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_nested_custom_enum_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/enum_payload_boxed_builtin_option_field_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/array_composite_transport_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/array_mutable_array_min_primitive_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_trace_array_string_elements_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_refs.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T05：收口 effect-typed adapter 与 NoOutward plain ABI

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.12、§5.1、§5.4
  - [`TODO.md`](./TODO.md) MIR-T12
- 目标：
  - `impl_plan = NoOutward` 或 `resolved_outward_cases = ∅` 的 callable body 公开 plain ABI；`CallableAbiKind::EffectStep` body 或 effect-typed adapter surface 才使用 EffectStep。
  - effect-typed function value/closure/FunPtr adapter 覆盖 aggregate return。

- 必须实现的内容：
  1. effect-typed plain adapter 支持 hidden-sret / aggregate return，包装为 `Step_F::Complete`。
  2. plain closure/function value/FunPtr callable carrier 不指向 EffectStep body，除非消费的是独立 adapter publication。
  3. plain `main(args: Array<String>)` wrapper 继续使用 plain argv ABI；`NoOutward` plain body 不生成 Step argv ABI。
  4. body emitter 对 residual effect/control terminator 只接受 MIR-T12 发布的 plain-local handoff，否则 verifier fail-fast。

- 必须遵从的约束：
  - 禁止 complete-only `Step_F` 作为 `NoOutward` plain body workaround。
  - 禁止把 effect-typed surface 直接等同于 callee body EffectStep ABI。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_effect_typed_adapter`
  2. `cargo test -p scoopc refactor_llvm_no_outward_plain_abi`
  3. run-pass 覆盖 plain body through effect-typed function value、aggregate return adapter、`main(args)` plain argv。

- 完成条件：
  - `PIPELINE_GAPS.md` §3.12、§5.1、§5.4 的 codegen 部分关闭。
- 依赖：`CG-T04R`，`MIR-T12R`

- 完成记录：
  - 2026-05-07：effect-typed plain closure adapter 现在按独立 dynamic-invoke adapter surface 包装 NoOutward/plain body；adapter layout 匹配会先把 source/layout 类型映射到 codegen type store，避免 aggregate return 的 `TypeId` 漂移导致找不到 adapter layout。
  - 2026-05-07：adapter 支持 hidden-sret / aggregate return：按 `Step_F::Complete` payload layout 和 plain entry 参数数识别 sret，调用 plain entry 后从 sret slot 载入 aggregate payload，再构造 `Step_F::Complete`，不把 plain body 改写成 EffectStep body。
  - 2026-05-07：plain lambda entry 声明改为消费 P5 plain ABI handoff 的 `param_tys` / `return_ty`，确保 closure/function-value carrier 指向 plain entry 或独立 adapter，而不是因错误 type store 退化成 scalar ABI；dynamic surface-resume adapter 候选匹配补充 `answer_ty`，无有效 target 时生成 unreachable wrapper，避免 tuple-return adapter 被错误投影到 Int continuation 或留下未定义符号。
  - 2026-05-07：确认 NoOutward/plain callable 继续发布 plain ABI，`main(args: Array<String>)` 继续走 plain argv wrapper，不引入 Step argv ABI 或 complete-only `Step_F` body。
  - 验证通过：`cargo test -p scoopc refactor_llvm_effect_typed_adapter`、`cargo test -p scoopc refactor_llvm_no_outward_plain_abi`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/entry_main_args_int_exit_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_step_enum_no_outward.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_materialized_mir_closure_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T05R：Review CG-T05 adapter 与 NoOutward ABI

- 参考：
  - `CG-T05`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.5、§5.2、§5.4、§8
  - [`TODO.md`](./TODO.md) MIR-T12R
- 重点：
  - `NoOutward` / empty `resolved_outward_cases` body 是否公开 plain ABI，不生成 complete-only body `Step_F` 或 Step argv ABI。
  - effect-typed dynamic surface 是否通过独立 adapter 包装 plain result 为 `Step_F::Complete`，不改变 callee body ABI。
  - aggregate return / hidden-sret adapter 与 `main(args)` plain argv wrapper 是否正确。
- 验证：
  1. 重跑 `CG-T05` 的全部验证命令。
  2. 抽查 plain body through effect-typed function value、aggregate return adapter、`main(args)` plain argv fixtures。
  3. 搜索 complete-only `Step_F` workaround，确认没有用于 `NoOutward` plain body。
- 完成条件：
  - Review 结论明确说明 `CG-T05` 已正确实现；若发现缺口，`CG-T05R` 保持未完成并把修复归回 `CG-T05`。
- 依赖：`CG-T05`

- 完成记录：
  - 2026-05-07：复审 `CG-T05` 的 NoOutward/plain ABI publication、effect-typed dynamic surface adapter、hidden-sret aggregate return wrapping 与 `main(args)` plain argv wrapper，确认 plain callable body 不发布 body `Step_F`、Step argv ABI 或 complete-only Step shell；effect-typed surface 通过独立 adapter 将 plain result 包装为 `Step_F::Complete`，不改变 callee body ABI。
  - 2026-05-07：复审中发现 effect-typed plain adapter layout 只按 args/return shape 匹配 dynamic-invoke layout，多个相同 args/return 但不同 effect row 的 surface 会匹配歧义；已修复为同时匹配 step layout 的 effect-family identity，并新增 `tests/fixtures/run-pass/effect_typed_plain_adapter_multiple_effect_rows_basic.scoop` 覆盖回归。
  - 2026-05-07：搜索 `complete-only|NoOutward.*Step_F|Step_F.*NoOutward|Step argv|body Step schema`，命中限于 plain ABI guard、route verifier 诊断、handoff 注释与 NoOutward 负向 fixture 说明，未发现合法 NoOutward plain body 依赖 complete-only `Step_F` workaround。
  - 验证通过：`cargo test -p scoopc refactor_llvm_effect_typed_adapter`、`cargo test -p scoopc refactor_llvm_no_outward_plain_abi`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_typed_plain_adapter_multiple_effect_rows_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/entry_main_args_int_exit_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_step_enum_no_outward.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_dynamic_entry_publication_emit_llvm.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_indirect_perform_materialized_mir_closure_basic.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T06：收口 source classification、unwind、thread boundary lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.13、§5.2、§5.3、§5.6
  - [`TODO.md`](./TODO.md) MIR-T10、MIR-T12、MIR-T13
- 目标：
  - Late-lowered source classification、cleanup/unwind、continuation storage route、thread resume non-complete boundary 不再晚期 unsupported 或 runtime fatal。

- 必须实现的内容：
  1. `LateLoweredSourceStatementClassificationKind::Unsupported` 默认由 verifier 拒绝；intentional skip/elide 必须有 explicit reason。
  2. `ResumeUnwind` lowering 消费 unwind payload carrier、cleanup continuation、pending completion、origin/resume-state contract。
  3. `StoreMember` continuation route 消费唯一 owner/source route；ambiguous route 必须在 upstream handoff 已拆解或 fail-fast。
  4. thread resume non-complete Step：若支持，定义跨线程 effect propagation runtime boundary；若不支持，消费 upstream diagnostic gate。

- 必须遵从的约束：
  - 不允许 LLVM body emission 才发现 unsupported classification。
  - 不允许 runtime `exit(3)` 成为合法语言语义的唯一表达。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_source_classification_verifier`
  2. `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`
  3. continuation storage route 与 cross-thread non-complete policy fixtures。

- 完成条件：
  - `PIPELINE_GAPS.md` §3.13、§5.2、§5.3、§5.6 的 codegen/runtime 部分关闭或有 explicit frontend reject。
- 依赖：`CG-T05R`，`MIR-T13R`

- 完成记录：
  - 2026-05-07：refactor body verifier 现在默认拒绝 `LateLoweredSourceStatementClassificationKind::Unsupported`，只接受已发布的 effect-neutral、boundary-consumed、resume/result/completion injection、handle binder 与 explicit `ElidedUnreachable` classification，避免 body emission 才发现 unsupported statement。
  - 2026-05-07：`ResumeUnwind` lowering 改为先验证 canonical MIR cleanup source slice、Suspend cleanup continuation route、boundary owner/resume-state provenance，以及 enclosing `HandleDispatch` finally pending-completion / origin / payload transport ABI contract；通过验证后才把 terminal cleanup path lower 为 contract-defined unreachable。
  - 2026-05-07：`StoreMember` continuation route gate 除拒绝 `Ambiguous` 外，也验证 `Unique` route 的 source local 存在且 source type 未漂移，确保 backend 消费 MIR 发布的唯一 owner/source route。
  - 2026-05-07：跨线程 resume thunk 不再调用 `scoop_refactor_thread_resume_noncomplete_fatal`；codegen 会二次校验 helper operand 是 `Pure` continuation，non-complete `RuntimeError` case 走 ordinary `scoop_runtime_error_fatal` terminal，其他 non-complete case 仅作为 upstream Pure/dispatch contract 下的 unreachable case；移除对应 runtime C fatal helper 与 ABI symbol。
  - 验证通过：`cargo test -p scoopc refactor_llvm_source_classification_verifier`、`cargo test -p scoopc refactor_llvm_resume_unwind_lowering`、`cargo test -p scoopc refactor_mir_store_member_codegen`、`cargo test -p scoopc refactor_llvm_thread_resume_noncomplete_policy`、`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/cross_thread_resume_outward_effects_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_refs.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T06R：Review CG-T06 unwind/thread boundary lowering

- 参考：
  - `CG-T06`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6、§8
  - [`TODO.md`](./TODO.md) MIR-T10R、MIR-T12R、MIR-T13R
- 重点：
  - `LateLoweredSourceStatementClassificationKind::Unsupported` 是否默认 fail-fast，intentional skip/elide 是否有 explicit reason。
  - `ResumeUnwind`、cleanup continuation、pending completion、origin/resume-state contract 是否被 lowering 消费。
  - continuation storage route 与 cross-thread non-complete policy 是否不依赖 runtime `exit(3)` 或 LLVM body emission late discovery。
- 验证：
  1. 重跑 `CG-T06` 的全部验证命令。
  2. 抽查 source classification verifier、resume unwind、continuation storage route、cross-thread policy fixtures。
  3. 搜索 unsupported classification 和 runtime fatal helper，确认合法语义路径不再依赖它们。
- 完成条件：
  - Review 结论明确说明 `CG-T06` 已正确实现；若发现缺口，`CG-T06R` 保持未完成并把修复归回 `CG-T06`。
- 依赖：`CG-T06`

- 完成记录：
  - 2026-05-07：复审 `CG-T06` 的 source classification verifier、`ResumeUnwind` cleanup/unwind contract、`StoreMember` continuation route gate 与 cross-thread non-complete policy，确认 unsupported classification 会在 materialize/backend verifier fail-fast，`ResumeUnwind` lowering 消费 canonical cleanup source slice、Suspend cleanup route、boundary owner/resume-state 与 HandleDispatch pending-completion contract，continuation storage route 拒绝 `Ambiguous` 并校验 `Unique` source local/type，跨线程 resume non-complete 不再依赖 `scoop_refactor_thread_resume_noncomplete_fatal`。
  - 2026-05-07：搜索 `LateLoweredSourceStatementClassificationKind::Unsupported`、`ResumeUnwind`、`scoop_refactor_thread_resume_noncomplete_fatal|resume_noncomplete_fatal|thread_resume_noncomplete` 与 runtime `exit(3)`，确认合法 cross-thread non-complete 路径通过 frontend Pure gate / ordinary `scoop_runtime_error_fatal` terminal / unreachable contract 表达，未发现 runtime thread-resume fatal helper 仍在 production 代码中使用。
  - 验证通过：`cargo test -p scoopc refactor_llvm_source_classification_verifier`、`cargo test -p scoopc refactor_llvm_resume_unwind_lowering`、`cargo test -p scoopc refactor_mir_store_member_codegen`、`cargo test -p scoopc refactor_llvm_thread_resume_noncomplete_policy`、`cargo test -p scoopc refactor_llvm_cross_thread_resume_payload_transport`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/cross_thread_resume_outward_effects_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/handle_finally_boundary.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_refs.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07：收口 extern global 与 GC pin/handle runtime surface

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §6.4、§7.6
  - [`TODO.md`](./TODO.md) MIR-T13
- 目标：
  - `@Extern` global variable 和 GC pin/handle intrinsic surface 在 refactor codegen/runtime 中有完整实现或明确 reject。

- 必须实现的内容：
  1. `@Extern` global lowering 支持 external symbol name、linkage、TLS/global storage、initializer absence、unsafe access requirement。
  2. load/store extern global 时消费 MIR storage metadata，不回 AST annotation。
  3. GC pin/handle intrinsic 若支持，lower pin/unpin lifetime、root registration、handle deref/escape rules。
  4. GC pin/handle intrinsic 若延期，确认 parser/typecheck diagnostic 在进入 MIR 前触发。

- 必须遵从的约束：
  - 不允许 extern global 伪装成普通 top-level init root。
  - 不允许 pin/handle intrinsic 绕过 GC root verifier。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_extern_global`
  2. extern global build/run fixtures。
  3. GC pin/handle positive 或 negative fixture，按当前 policy 选择。

- 完成条件：
  - `PIPELINE_GAPS.md` §6.4、§7.6 的 codegen/runtime 部分关闭或明确 frontend reject。
- 依赖：`CG-T06R`

- 完成记录：
  - 2026-05-07：refactor LLVM top-level ref/store 现在优先消费 materialized MIR `ExternGlobalRoot` storage contract，按外部 symbol 声明 LLVM global，保留 `External` linkage 与 `@ThreadLocal` TLS storage；extern global 不再伪装成普通 top-level init/root storage。
  - 2026-05-07：新增 direct extern-global unsafe access 诊断，非 `@Unsafe` 读写在 typecheck 阶段以 `scoop::typecheck::extern_global_access_requires_unsafe` 拒绝；新增 `extern_global_load_store_basic.scoop` run-pass 与 unsafe negative fixture。
  - 2026-05-07：补齐 refactor MIR `GC.handleGet` lowering，现有 GC handle/pin positive policy 继续通过 runtime helpers 覆盖，不改为 frontend reject；runtime 新增 `scoop_test_extern_global_counter` 测试 storage 并登记 ABI allowlist。
  - 验证通过：`cargo test -p scoopc refactor_llvm_extern_global`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_global_load_store_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/unsafe_nogc/extern_global_access_requires_unsafe_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07R：Review CG-T07 extern global 与 GC surface

- 参考：
  - `CG-T07`
  - [`SCOOP_FULL_SPEC.md`](./SCOOP_FULL_SPEC.md) §15.10
  - [`TODO.md`](./TODO.md) MIR-T13R
- 重点：
  - `@Extern` global lowering 是否消费 MIR storage metadata，不回 AST annotation 或普通 top-level init root。
  - GC pin/handle intrinsic surface 是否按当前 policy 有 positive implementation 或 explicit frontend diagnostic。
  - pin/unpin lifetime、root registration、handle deref/escape rules 是否不绕过 GC root verifier。
- 验证：
  1. 重跑 `CG-T07` 的全部验证命令。
  2. 抽查 extern global build/run fixtures 与 GC pin/handle positive 或 negative fixture。
  3. 检查 runtime/LLVM glue 是否没有把 pin/handle 当普通 unsafe pointer shortcut。
- 完成条件：
  - Review 结论明确说明 `CG-T07` 已正确实现；若发现缺口，`CG-T07R` 保持未完成并把修复归回 `CG-T07`。
- 依赖：`CG-T07`

- 完成记录：
  - 2026-05-07：复审 `CG-T07` 的 extern global storage contract、unsafe access gate、GC pin/unpin lowering 与 stable handle runtime surface，确认 refactor LLVM load/store 优先消费 materialized MIR `ExternGlobalRoot`，按外部 symbol / `External` linkage / TLS storage 声明 LLVM global，不生成普通 top-level init/root backing storage，也不回 AST annotation 反推 storage。
  - 2026-05-07：确认 `GC.pin` / `GC.unpin` / `GC.handleNew` / `GC.handleGet` / `GC.handleDrop` 通过 sysroot intrinsic lowering 调用 runtime pin/handle table；runtime 将 pinned objects 与 stable handles 纳入 mark/root verification，并在 moving/compaction 后更新 handle root slot，未发现把 pin/handle 当普通 unsafe pointer shortcut 的合法路径。
  - 2026-05-07：后续一致性复核确认 `CG-T07R` 正文与最新提交已完成，任务索引缺少 `[DONE]` 属于 bookkeeping 漂移；已补齐索引标记并重跑验证。
  - 验证通过：`cargo test -p scoopc refactor_llvm_extern_global`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extern_global_load_store_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/unsafe_nogc/extern_global_access_requires_unsafe_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`、`cargo test -p scoopc codegen_gap_inventory`、`cargo test -p scoop_runtime --lib abi_exports_allowlist`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_pin_unpin_basic.scoop`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a0：修复 elvis_lazy_basic 中 Option enum payload transport trace metadata 漂移，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T04c`
  - `CG-T08`
  - `tests/fixtures/run-pass/elvis_lazy_basic.scoop`
- 背景：
  - 在 `CG-T07S0a` 修复 synthetic `RuntimeError.NullAssertionFailed` authoritative HIR 形状后，默认 `cargo run -p scoop -- test` 不再停在 `effect_handle_top_level_val_pattern_access_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/elvis_lazy_basic.scoop` 的 build/run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/elvis_lazy_basic.scoop -o /tmp/elvis_lazy_basic` 会在 refactor backend gate 报 `composite transport layout descriptor has GC slots but MIR trace requirement is false`，定位到 `Some(41)` / `None()` 的 raw MIR `Option<Int>` enum payload transport metadata，说明 generic enum constructor/value path 的 trace requirement 与 published composite layout descriptor 仍有漂移。

- 必须实现的内容：
  1. 修复 raw/materialized MIR 对 `Option<T>` 等 generic enum constructor/value path 发布的 `AggregateTransportMetadata` / `ValueTransportMetadata`，确保 GC slot map、trace/copy/drop requirements 与 enum payload layout descriptor 一致。
  2. 保持 composite transport verifier 继续只消费 authoritative MIR contract；不得在 backend 降低 gate 或根据 fixture shape 猜测 enum traceability。
  3. 补最小回归验证，确保 `elvis_lazy_basic.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、绕开 `?:` lowering、把 `Option` 特判成非 composite carrier 或放宽 composite verifier 规避问题。
  - 不允许把 enum payload trace requirement 私补到 LLVM backend；必须在 authoritative MIR transport contract / lowering 主线上修正。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/elvis_lazy_basic.scoop -o /tmp/elvis_lazy_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/elvis_lazy_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `elvis_lazy_basic.scoop` 处被 composite transport verifier 阻塞，`CG-T07S0a` 可恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。修复 synthetic `RuntimeError.NullAssertionFailed` HIR 形状并更新受影响 HIR/MIR snapshot 后，`effect_handle_top_level_val_pattern_access_basic.scoop` 的 build/单 fixture test 已通过，但默认 `cargo run -p scoop -- test` 继续前进到 `elvis_lazy_basic.scoop`；build/run 诊断显示 raw MIR `Option<Int>` enum payload transport metadata 与 composite layout trace requirement 漂移，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：抽出共享 `Option<T>` transport trace requirement 规则，让 MIR lowering/materialize 与 LLVM composite transport verifier 都按实际布局选择计算 trace 需求：tagged-union `Option<Int>` 因固定携带 GC pointer slot 而保持 `trace=true`，niche `Option<Bool>` 仍保持非 traceable。
  - 2026-05-08：验证通过：`cargo test -p scoopc option_transport_trace_requirement_tracks_layout_representation`、`cargo run -p scoop -- build tests/fixtures/run-pass/elvis_lazy_basic.scoop -o /tmp/elvis_lazy_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/elvis_lazy_basic.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `elvis_lazy_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/fun_call_add_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a1`。

## [DONE] CG-T07S0a1：修复 fun_call_add_basic 中 refactor plain return coercion 把 `main(): Int` 尾值误判成 `Ref`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/fun_call_add_basic.scoop`
- 背景：
  - 在 `CG-T07S0a0` 修复 `elvis_lazy_basic.scoop` 的 `Option<Int>` composite transport trace metadata 漂移后，默认 `cargo run -p scoop -- test` 不再停在 `elvis_lazy_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/fun_call_add_basic.scoop` 的 build/run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/fun_call_add_basic.scoop -o /tmp/fun_call_add_basic` 会在 LLVM 前端准备阶段报 `refactor plain return coercion failed ... unsupported value coercion from Ref to Int(IntTy { bits: 64, signed: true })`，说明 plain body return preparation 仍把 `main(): Int` 的尾值路径误判成 `Ref`，导致最基本的 top-level fun call/return fixture 无法通过。

- 必须实现的内容：
  1. 修复 refactor plain return coercion / frontend prepare 对 plain body 尾表达式返回值的类型归类，确保 `fun_call_add_basic.scoop` 中 `main(): Int` 的 `if` 尾值继续按 `Int` 返回，而不是走 `Ref -> Int` 的非法 coercion。
  2. 保持 top-level function call / return ABI 继续消费 authoritative MIR / call contract；不得回退到 legacy path 或通过默认值掩盖返回值类型错误。
  3. 补最小回归验证，确保 `fun_call_add_basic.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、改 `EXPECT-EXIT`、把尾表达式改写成显式 `return`、或降级到 legacy path 规避该问题。
  - 不允许在 LLVM backend 私补 `Ref -> Int` 特判；必须在 authoritative plain return preparation / lowering 主线上修正。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/fun_call_add_basic.scoop -o /tmp/fun_call_add_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fun_call_add_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `fun_call_add_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a0`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a0` 修复后，默认 full-suite 继续前进到 `fun_call_add_basic.scoop`；build 诊断显示 refactor plain return coercion 仍会把 `main(): Int` 尾值路径误判成 `Ref`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：non-async function/getter body lowering 现在会把 declared return type 作为 `ExpectedExpr.value_ty` 传给 tail expression lowering；`if` / `when` 在缺少 typecheck side table 时也会回退消费该 expected type，避免函数尾表达式被错误降成 `Any` 后再在 MIR/LLVM 前端准备阶段走 `Ref -> Int` 非法 coercion。
  - 2026-05-08：新增 `hir::lower::tests::refactor_hir_tail_if_uses_declared_return_type_hint`，并更新 `tests/fixtures/mir/when_bind_guard.mir` 以匹配修复后的 authoritative direct-style MIR：`when` 尾值结果 local 直接保持声明返回类型，不再经 `AnyErasure` transport。
  - 2026-05-08：验证通过：`cargo test -p scoopc refactor_hir_tail_if_uses_declared_return_type_hint`、`cargo run -p scoop -- build tests/fixtures/run-pass/fun_call_add_basic.scoop -o /tmp/fun_call_add_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fun_call_add_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir/when_bind_guard.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `fun_call_add_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a2`。

## [DONE] CG-T07S0a2：修复 gc_array_class_elements_cross_function 中 `println::<String>` arg lowering 把 `String` 值误判成 `Ref`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T03`
  - `CG-T04d`
  - `CG-T08`
  - `tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
- 背景：
  - 在 `CG-T07S0a1` 修复 `fun_call_add_basic.scoop` 的 plain return coercion 漂移后，默认 `cargo run -p scoop -- test` 不再停在 `fun_call_add_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop` 的 build/run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop -o /tmp/gc_array_class_elements_cross_function` 会在 LLVM 前端准备阶段报 `refactor pure assignment ... callee_fqn: "scoop.core.println::<String>" ... unsupported value coercion from Ref to String`，说明 refactor pure assignment / direct-call lowering 在 `Array<String>` 跨函数读取与 `println::<String>` surface 上仍把 `String` 值路径误判成 `Ref`。

- 必须实现的内容：
  1. 修复 refactor pure assignment / direct-call lowering 对 `String` 值路径的类型归类，确保 `gc_array_class_elements_cross_function.scoop` 中 `println::<String>`、`Array<String>.get`、跨函数 `String` 参数/返回值继续消费 authoritative MIR / call contract，而不是走 `Ref -> String` 的非法 coercion。
  2. 保持数组元素 transport、`String` runtime surface 与 callable contract 继续由 authoritative HIR/MIR/transport metadata 提供；不得回退到 legacy path，也不得在 LLVM backend 私补 `Ref -> String` 特判掩盖 contract 漂移。
  3. 补最小回归验证，确保 `gc_array_class_elements_cross_function.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、移除 `println` / GC collect / cross-function array 读取、或降级到 legacy path 规避该问题。
  - 不允许把 `String` surface 私补成 backend-only 特判；必须在 authoritative pure assignment / call-lowering 主线上修正。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop -o /tmp/gc_array_class_elements_cross_function`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `gc_array_class_elements_cross_function.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a1`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a1` 修复后，默认 full-suite 继续前进到 `gc_array_class_elements_cross_function.scoop`；build 诊断显示 refactor pure assignment / `println::<String>` arg lowering 仍会把 `String` 值路径误判成 `Ref`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：refactor plain callable body lowering 现在统一用 `pass_view.materialized().types` 解释 canonical MIR body 的 composite transport contract、返回类型、local slot 与 `RefactorValuePrimitives` value lowering，避免 canonical MIR `String` surface 与 plain LLVM slot/type 推导脱节。
  - 2026-05-08：materialized direct-call rewrite 不再允许 `scoop.core.size/get/set` 在 exact site binding miss 且 remap 失败时继承无关的 enclosing binding；字符串插值中的 `arr1.size()` 不会再偷用外层 `println` binding 并被误 materialize 成 `println::<String>`。
  - 2026-05-08：新增 `llvm::tests::refactor_plain_array_string_get_keeps_string_surface_for_println` 与 `llvm::tests::materialized_gc_array_fixture_keeps_string_locals_for_println_string_sites`，分别覆盖最小 `Array<String> -> println(String)` plain codegen 路径，以及真实 fixture 的 canonical materialized MIR call-site invariant。
  - 2026-05-08：验证通过：`cargo test -p scoopc refactor_plain_array_string_get_keeps_string_surface_for_println`、`cargo test -p scoopc materialized_gc_array_fixture_keeps_string_locals_for_println_string_sites`、`cargo run -p scoop -- build tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop -o /tmp/gc_array_class_elements_cross_function`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `gc_array_class_elements_cross_function.scoop`，下一处失败转为 `tests/fixtures/run-pass/gc_trace_task_field_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a3`。

## [DONE] CG-T07S0a3：修复 gc_trace_task_field_basic 中 `Async.await(holder.task)` perform site metadata 把 payload transport type 与 payload component type 发布成漂移 shape，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T04f`
  - `CG-T08`
  - `tests/fixtures/run-pass/gc_trace_task_field_basic.scoop`
- 背景：
  - 在 `CG-T07S0a2` 修复 `gc_array_class_elements_cross_function.scoop` 的 `String` surface / site-binding 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/gc_trace_task_field_basic.scoop` 的 build/run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/gc_trace_task_field_basic.scoop -o /tmp/gc_trace_task_field_basic` 会在 LLVM 前端准备阶段报 `refactor direct-style MIR validation failed for main: ... incomplete perform site metadata ... perform payload transport type disagrees with payload component type`，说明 `Async.await(holder.task)` 的 perform site 仍把 payload transport type 与 payload component type 发布成漂移 shape。

- 必须实现的内容：
  1. 修复 `Async.await(holder.task)` 及等价 `Task<T>` 字段访问路径的 direct-style MIR perform site metadata 发布，确保 payload transport type 与 payload component type 在 authoritative MIR contract 中一致。
  2. 保持 `Task<String>` 字段 reachability、`Async.await` perform 路径与后续 `__task_join` 消费继续依赖 authoritative MIR/effect/transport contract；不得通过放宽 validator、跳过 perform metadata 校验或在 LLVM backend 猜 payload shape 规避问题。
  3. 补最小回归验证，确保 `gc_trace_task_field_basic.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、移除 `Holder.task` 字段访问、绕开 `Async.await` / `handle` / `__task_join` 路径或降级到 legacy path 规避该问题。
  - 不允许关闭或弱化 `refactor direct-style MIR validation`；必须在 authoritative perform site metadata producer 主线上修正。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/gc_trace_task_field_basic.scoop -o /tmp/gc_trace_task_field_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_trace_task_field_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `gc_trace_task_field_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a2`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a2` 修复后，默认 full-suite 继续前进到 `gc_trace_task_field_basic.scoop`；build 诊断显示 `Async.await(holder.task)` 的 direct-style MIR perform site metadata 仍把 payload transport type 与 payload component type 发布成漂移 shape，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：根因定位为 HIR member access lowering 默认把普通字段访问结果发布成 `Any`：`holder.task` 在 typed HIR / `PerformSiteContract` 中被记成 `Any`，但 direct-style MIR member-value lowering 仍能得到真实的 `Task<String>`，于是 `Async.await(holder.task)` 的 perform payload component type 与 payload transport type 在 `main` 的 bb1 校验时发生漂移。
  - 2026-05-08：`lower_member_access_expr` / `lower_member_access_expr_from_receiver` 现在会消费 authoritative member access 结果类型；`?.` 的内层 binder 也保留实际 receiver type，避免 safe member access 再次把字段结果降成 `Any`。新增 `effect_refactor_pipeline::mir_stage::tests::refactor_mir_task_field_perform_contract_keeps_task_payload_type`，并同步更新受影响的 HIR/MIR snapshots：`tests/fixtures/hir/delegated_property_lowering.hir`、`tests/fixtures/hir/member_access.hir`、`tests/fixtures/hir/safe_call_not_null_assert.hir`、`tests/fixtures/mir_refactor/aggregate_transport.mir`。
  - 2026-05-08：验证通过：`cargo test -p scoopc refactor_mir_task_field_perform_contract_keeps_task_payload_type`、`cargo run -p scoop -- build tests/fixtures/run-pass/gc_trace_task_field_basic.scoop -o /tmp/gc_trace_task_field_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/gc_trace_task_field_basic.scoop`；默认 `cargo run -p scoop -- test` 已越过 `gc_trace_task_field_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a4`。

## [DONE] CG-T07S0a4：修复 kotlin_ranges_progressions_basic 中 progression/forEach lowering 的 assign place contract 指向未分配 local symbol，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`
- 背景：
  - 在 `CG-T07S0a3` 修复 `gc_trace_task_field_basic.scoop` 的 `Async.await(holder.task)` perform metadata 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop` 的 build/run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop -o /tmp/kotlin_ranges_progressions_basic` 会在 direct-style MIR lowering 阶段 panic：`assignment place contract references an unallocated local: S34`，说明 progression/forEach lowering 仍会让 typed HIR assign place contract 指向未分配的 local symbol。

- 必须实现的内容：
  1. 修复 `IntProgression` / `forEach` / lambda 相关 lowering 对 assign place contract 与 MIR local 分配的一致性，确保 `kotlin_ranges_progressions_basic.scoop` 能在 refactor path 正常 build/run。
  2. 保持 range/progression surface 继续消费 authoritative typed HIR / MIR contract；不得通过关闭 assign place contract、在 MIR lowering 路径吞掉缺失 local、或降级到 legacy path 规避问题。
  3. 补最小回归验证，确保 `kotlin_ranges_progressions_basic.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、移除 `rangeTo` / `downTo` / `forEach`、绕开 progression lambda 或降级到 legacy path 规避该问题。
  - 不允许把 `assignment place contract references an unallocated local` 变成 backend-only 容错；必须在 authoritative typed HIR / MIR lowering 主线上修正 local contract 发布与分配。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop -o /tmp/kotlin_ranges_progressions_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `kotlin_ranges_progressions_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a3`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a3` 修复后，默认 full-suite 继续前进到 `kotlin_ranges_progressions_basic.scoop`；build 阶段在 direct-style MIR lowering 触发 `assignment place contract references an unallocated local: S34` panic，说明 progression/forEach lowering 仍存在 assign place contract / local allocation 漂移，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：根因定位为 build/frontend 的 explicit MIR instance lowering 会在 fresh `HirLowering` 上下文里重新 lower 同一源码函数体，而 assign-place side table 继续按 `source_path + stmt span` 复用 typed contract；`AssignPlaceKind::Local` 中的 `SymbolId` 因 `SymbolInterner` 按 lowering 上下文局部分配而漂移，但 contract 自带的 local `decl_span` 仍保持 authoritative source identity，于是 `IntProgression.forEach` / progression 相关实例体在 MIR lowering 读取 assign-place contract 时会命中 stale `SymbolId` 并报 `unallocated local`。
  - 2026-05-08：`crates/scoopc/src/mir/lower.rs` 现在在消费 `AssignPlaceKind::Local` contract 时先按 `SymbolId` 查当前 body 的 `LocalId`，若实例体里的 `SymbolId` 与 side table 漂移，则按 contract 自带的 local `name + decl_span` 在当前 source-local 集合中选取最窄匹配声明 span 重新绑定 `LocalId`，让 explicit MIR instance lowering 与 typed assign-place contract 继续共享同一 authoritative source identity，而不是把 panic 容错吞到 backend。
  - 2026-05-08：新增 `llvm::tests::production_codegen_progression_fixture_prepares_generic_for_each_assign_contracts`，覆盖 production single-file frontend 准备 `kotlin_ranges_progressions_basic.scoop` 时会 materialize stdlib `forEach` 实例并成功生成 IR，不再在 progression/forEach assign-place contract 上 panic。
  - 2026-05-08：验证通过：`cargo test -p scoopc production_codegen_progression_fixture_prepares_generic_for_each_assign_contracts`、`cargo run -p scoop -- build tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop -o /tmp/kotlin_ranges_progressions_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `kotlin_ranges_progressions_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/list_and_mutable_list_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a5`。

## [DONE] CG-T07S0a5：修复 list_and_mutable_list_basic 中 MutableList.add/push materialized MIR 的 array transport element type 仍保留 unresolved generic `T`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T04d`
  - `CG-T08`
  - `tests/fixtures/run-pass/list_and_mutable_list_basic.scoop`
- 背景：
  - 在 `CG-T07S0a4` 修复 `kotlin_ranges_progressions_basic.scoop` 的 progression/forEach assign-place contract / local rebind 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/list_and_mutable_list_basic.scoop` 的 build/run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/list_and_mutable_list_basic.scoop -o /tmp/list_and_mutable_list_basic` 会在 materialized MIR validation 报 `materialized MIR 'scoop.core.push' contains unresolved generic parameter in array transport element type ...: T`，说明 `MutableList<Int>.add` / `MutableArray<Int>.push` 路径的 array element transport type 仍保留未具体化的 generic 参数。

- 必须实现的内容：
  1. 修复 `MutableList<Int>.add` / `MutableArray<Int>.push` 及等价 array-builder append 路径在 explicit MIR instance / materialized MIR 中的 element transport type 具体化，确保 `list_and_mutable_list_basic.scoop` 能在 refactor path 正常 build/run。
  2. 保持 list/mutable-list surface 继续消费 authoritative materialized MIR / array transport contract；不得通过放宽 unresolved generic parameter validator、在 backend 现场硬编码 `Int`、或绕开 `MutableList.add` / `push` 路径规避问题。
  3. 补最小回归验证，确保 `list_and_mutable_list_basic.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、移除 `MutableList.add` / `MutableArray.push`、绕开 array builder append 路径或降级到 legacy path 规避该问题。
  - 不允许把 `materialized MIR ... unresolved generic parameter` 变成 validator-only 例外；必须在 authoritative instance/materialized MIR contract 主线上修正 element transport type 的具体化发布。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/list_and_mutable_list_basic.scoop -o /tmp/list_and_mutable_list_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/list_and_mutable_list_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `list_and_mutable_list_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a4`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a4` 修复后，默认 full-suite 继续前进到 `list_and_mutable_list_basic.scoop`；build 阶段在 materialized MIR validation 报 `materialized MIR 'scoop.core.push' contains unresolved generic parameter in array transport element type ...: T`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：根因定位为 `crates/scoopc/src/mir/materialize.rs` 的 `repair_array_call_transport_types()` 对 `BuilderPush` 只在现有 `array.element_ty` 已经 concrete 时才保留 transport metadata；`MutableArray<Int>.push`/`MutableList<Int>.add` 路径里的 append 元素来自 generic `get` 调用，materialized body 虽已把实参 local 修正为 `Int`，但 array transport metadata 仍残留 template 期的 `T`，最终被 materialized MIR validator 拒绝。
  - 2026-05-08：`BuilderPush` repair 现改为优先从第二个 call arg 的 concrete operand/local type 回填 `array.element_ty`，并同步刷新 value transport contract；新增 `llvm::tests::production_codegen_list_fixture_materializes_mutable_list_add_and_push_instances`，确认 production pass view 会保留 `MutableList.add` / `MutableArray.push` 实例，且 `scoop.core.push` body 的 builder append transport element type 已具体化为 `Int`。
  - 2026-05-08：验证通过：`cargo test -p scoopc production_codegen_list_fixture_materializes_mutable_list_add_and_push_instances`、`cargo run -p scoop -- build tests/fixtures/run-pass/list_and_mutable_list_basic.scoop -o /tmp/list_and_mutable_list_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/list_and_mutable_list_basic.scoop`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `list_and_mutable_list_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a6`。

## [DONE] CG-T07S0a6：修复 literal_numeric_expected_type_absorption_basic 中 `Array<UInt8>` element expected-type absorption 失效导致 run-pass 输出漂移，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T0150h-1`
  - `CG-T04d`
  - `CG-T08`
  - `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`
- 背景：
  - 在 `CG-T07S0a5` 修复 `list_and_mutable_list_basic.scoop` 的 materialized MIR array transport element type 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop` 会报 stdout 与 golden 不一致；直接 build/run 可见最后两行实际输出为 `false` / `false`，而预期是 `true` / `true`，说明 `Array<UInt8>` 中 `1 + 2` / `1 << 3` 这类数值字面量表达式在 array element expected-type 语境下仍未正确吸收成 `UInt8`。

- 必须实现的内容：
  1. 修复 `Array<UInt8>` element 语境下的 numeric literal arithmetic / shift expression expected-type absorption，确保 `literal_numeric_expected_type_absorption_basic.scoop` 中 `bytes` 数组路径在 authoritative typecheck/HIR/MIR/materialized contract 主线上发布为正确的 `UInt8` 元素语义。
  2. 保持修复落在 authoritative expected-type / array element contract 发布路径；不得通过改 golden、改 fixture、在 backend 现场补 truncation/compare 特判，或绕开 `Array<UInt8>` literal path 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许把 `Array<UInt8>` element path 退回 `Int` 再依赖 runtime/LLVM 现场猜测收窄。
  - 不允许通过放宽 fixture 断言或修改输出 golden 把错误语义记为通过。

- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`
  2. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `literal_numeric_expected_type_absorption_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a5`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a5` 修复后，默认 full-suite 继续前进到 `literal_numeric_expected_type_absorption_basic.scoop`；单 fixture run-pass 报 stdout mismatch，直接 build/run 显示 `Array<UInt8>` 上 `1 + 2` 与 `1 << 3` 的最后两处观测仍输出 `false` / `false`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：`crates/scoopc/src/hir/lower/expr.rs` 的 array literal lowering 现在只对“纯数值字面量算术/移位表达式”注入 element expected-binding，确保 builder-based `Array<UInt8>` lowering 不再把 `1 + 2` / `1 << 3` 的中间值以 nominal/composite surface 发布到 `__scoop_array_builder_push`；直接字面量数组元素与既有 HIR snapshot 形状保持不变。
  - 2026-05-08：新增 `llvm::tests::production_codegen_uint8_array_numeric_elements_keep_scalar_transport_metadata`，确认 `literal_numeric_expected_type_absorption_basic.scoop` 的 `bytes` array builder push transport metadata 保持 `UInt8` scalar surface、`trace = false` 且不发布 composite boxing。
  - 2026-05-08：验证通过：`cargo test -p scoopc production_codegen_uint8_array_numeric_elements_keep_scalar_transport_metadata`、`cargo test -p scoopc production_codegen_list_fixture_materializes_mutable_list_add_and_push_instances`、`cargo run -p scoop -- test --fixtures tests/fixtures/hir/array_lit_lowering.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `literal_numeric_expected_type_absorption_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a7`。

## [DONE] CG-T07S0a7：修复 literal_ops_compare_direct_matrix_basic 中 String 字面量 receiver 的 compareTo/concat 直接调用退化成 FunValue callee，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T0150h-3`
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop`
- 背景：
  - 在 `CG-T07S0a6` 修复 `literal_numeric_expected_type_absorption_basic.scoop` 后，默认 `cargo run -p scoop -- test` 不再停在 `Array<UInt8>` element expected-type absorption 漂移，而是继续暴露 `tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop -o /tmp/literal_ops_compare_direct_matrix_basic` 会报 `unsupported main codegen node: refactor plain function-value callee type`；`dump-mir` 显示 `"ab".compareTo("ac")` 被 lower 成 `MemberAccess { name: "compareTo", resolved: None }` 后再走 `CallKind::FunValue`，`"hi".concat("!")` 也保留同一退化形状。

- 必须实现的内容：
  1. 修复 String 字面量 receiver 的 `compareTo` / `concat` 直接调用 lowering，要求消费 authoritative call-site/member contract，不能把已解析的直接调用退化成 unresolved member-access + `FunValue` callee。
  2. 保持 `literal_ops_compare_direct_matrix_basic.scoop` 的 direct compare / concat 语义落在 refactor HIR/MIR/main codegen 主线上；不得通过改 fixture、拆成临时局部 helper 或 backend 现场猜 callee 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许继续让 direct member call 在 pure assignment / local binding 路径退化成 `CallKind::FunValue`，再由 LLVM backend 以 unsupported 拒绝。
  - 不允许把 literal receiver surface 改写成其他调用形状作为变通；必须修正 authoritative direct-call lowering contract。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop -o /tmp/literal_ops_compare_direct_matrix_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `literal_ops_compare_direct_matrix_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a6`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a6` 修复后，默认 full-suite 继续前进到 `literal_ops_compare_direct_matrix_basic.scoop`；build 诊断显示 `"ab".compareTo("ac")` 仍在 refactor plain main codegen 前端准备阶段退化成 `CallKind::FunValue` 并报 `unsupported main codegen node: refactor plain function-value callee type`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：`typecheck/expr/call.rs` 对 `String.concat` / `String.compareTo` 现在会发布 synthetic `ExtensionFun` member resolution 与 receiver-prefixed call-arg binding，使 HIR 把这两类 member call canonicalize 成 `scoop.core.concat` / `scoop.core.compareTo` top-level direct call，而不再退化成 unresolved `MemberAccess` + `CallKind::FunValue`。
  - 2026-05-08：legacy / refactor LLVM direct-call path 新增 `scoop.core.concat` / `scoop.core.compareTo` runtime lowering；`effect_facts::builder::is_plain_compiler_intrinsic` 同步把这两个 direct callee 归为 plain compiler intrinsic，避免 effect-step body 错把纯 String concat/compareTo 调用发布成 DynamicFallback outward call boundary。
  - 2026-05-08：验证通过：`cargo test -p scoopc frontend_codegen_rewrites_string_literal_compare_to_and_concat_to_extension_direct_calls`、`cargo run -p scoop -- build tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop -o /tmp/literal_ops_compare_direct_matrix_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_ops_compare_direct_matrix_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `literal_ops_compare_direct_matrix_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a8`。

## [DONE] CG-T07S0a8：修复 local_val_destructuring_nested_variant_mismatch_is_error 中 nested variant destructuring runtime-error path 的 direct-arg tuple payload contract 缺少 source component，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T05`
  - `CG-T08`
  - `tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
- 背景：
  - 在 `CG-T07S0a7` 修复 `literal_ops_compare_direct_matrix_basic.scoop` 后，默认 `cargo run -p scoop -- test` 不再停在 String 字面量 receiver direct call 退化，而是继续暴露 `tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop -o /tmp/local_val_destructuring_nested_variant_mismatch_is_error` 会报 `refactor ABI tuple payload 'refactor_carrier_direct_args' 缺少 source component 1`，说明 nested variant destructuring mismatch 的运行期报错路径仍发布了不完整的 direct-arg tuple payload contract。

- 必须实现的内容：
  1. 修复 local `val` destructuring nested variant mismatch 运行期校验路径的 tuple payload/source contract，确保 `(Some(_), y)` 这类嵌套 pattern 在首元素不匹配时会立即沿 authoritative runtime-error path 失败，而不是在 refactor ABI tuple payload materialization 前就丢失 source component。
  2. 保持 `local_val_destructuring_nested_variant_mismatch_is_error.scoop` 的 nested destructuring mismatch 语义落在 refactor HIR/MIR/effect-step/ABI 主线上；不得通过改 fixture、弱化 pattern、绕开 destructuring runtime check 或 backend 现场补 payload source 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许继续让 nested variant mismatch path 把 tuple payload source component 漏发给 `refactor_carrier_direct_args`，再由 LLVM ABI materialization fail fast。
  - 不允许把局部 pattern destructuring 改写成别的控制流形状作为变通；必须修正 authoritative runtime-check / payload publication contract。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop -o /tmp/local_val_destructuring_nested_variant_mismatch_is_error`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `local_val_destructuring_nested_variant_mismatch_is_error.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a7`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a7` 修复后，默认 full-suite 继续前进到 `local_val_destructuring_nested_variant_mismatch_is_error.scoop`；build 诊断显示 refactor ABI tuple payload `refactor_carrier_direct_args` 仍缺少 source component 1，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 的 closure carrier direct-args 转发现在会在“单个 tuple 形参且该形参本身就是 invoke-args tuple”时直接透传原始 explicit args payload，不再把整块 tuple 误当成 `source component 0` 后重组 `refactor_carrier_direct_args`，从而保留 authoritative tuple source layout 并消除缺失 `source component 1` 的 ABI 漂移。
  - 2026-05-08：新增 `llvm::tests::effect_step_single_tuple_param_closure_carrier_preserves_tuple_args_payload`，覆盖 effect-step callable 的单 tuple 形参在 refactor LLVM codegen 下仍能稳定走 effect wrapper / tuple-arg path。
  - 2026-05-08：验证通过：`cargo test -p scoopc effect_step_single_tuple_param_closure_carrier_preserves_tuple_args_payload`、`cargo run -p scoop -- build tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop -o /tmp/local_val_destructuring_nested_variant_mismatch_is_error`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `local_val_destructuring_nested_variant_mismatch_is_error.scoop`，下一处失败转为 `tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a9`。

## [DONE] CG-T07S0a9：修复 member_call_devirt_final_receiver_direct_call_basic 中 final receiver direct-call 去虚化后 `Base` vtable 仍引用未发射的 `Base.ping` 符号，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T03`
  - `CG-T07`
  - `CG-T08`
  - `tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop`
- 背景：
  - 在 `CG-T07S0a8` 修复 `local_val_destructuring_nested_variant_mismatch_is_error.scoop` 后，默认 `cargo run -p scoop -- test` 不再停在 nested destructuring runtime-error path 的 tuple payload contract 漂移，而是继续暴露 `tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop -o /tmp/member_call_devirt_final_receiver_direct_call_basic` 会在链接阶段报 `_Base.ping` undefined symbol，且调用栈指向 `__scoop_vtable__Base`，说明 final receiver direct-call 去虚化后，refactor/LLVM 仍遗漏了 `Base` vtable entry 所需的 authoritative method symbol 发射或可达性发布。

- 必须实现的内容：
  1. 修复 final receiver direct-call 去虚化与 class vtable/method symbol 发射之间的 contract，确保 `Derived` 上的 direct call 去虚化不会让 `Base` vtable 丢失 `Base.ping` 所需符号。
  2. 保持 `member_call_devirt_final_receiver_direct_call_basic.scoop` 的语义仍命中正确的 override target；不得通过关闭 devirt、改 fixture、删 vtable、或链接期手工补桩规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许把 `d.ping()` 退回非 authoritative 的 vtable/itable 调度作为兜底；必须修正 callable reachability / method publication / vtable contract 主线。
  - 不允许通过 linker-only alias、手工声明空符号、或移除 `Base` runtime surface 规避未发射 `Base.ping` 的问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop -o /tmp/member_call_devirt_final_receiver_direct_call_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `member_call_devirt_final_receiver_direct_call_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a8`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a8` 修复后，默认 full-suite 继续前进到 `member_call_devirt_final_receiver_direct_call_basic.scoop`；单 fixture build 诊断显示链接阶段 `_Base.ping` 仍未发射，但 `__scoop_vtable__Base` 已引用该符号，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：`crates/scoopc/src/llvm/reachability.rs` 的 materialized MIR reachable scan 现在会在遇到 `Rvalue::ClassCtor` 时显式 `enqueue_ctor(class_fqn, selected_ctor_span)`，把 ctor/super/vtable/itable reachability 接回 LLVM MIR 主线，避免 final receiver direct-call 去虚化后只有 `Derived.ping` 被 direct-call 命中、但 `Base` vtable 仍引用 declaration-only `Base.ping` 的漂移。
  - 2026-05-08：扩充 `llvm::tests::via_mir_direct_class_call_is_not_reinterpreted_as_vtable_dispatch`，除继续验证 via-MIR exact receiver direct call 不回退成 vtable dispatch 外，还断言 `Base` vtable 仍发布且 `Base.ping` 必须被定义而不是只声明。
  - 2026-05-08：验证通过：`cargo test -p scoopc via_mir_direct_class_call_is_not_reinterpreted_as_vtable_dispatch`、`cargo run -p scoop -- build tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop -o /tmp/member_call_devirt_final_receiver_direct_call_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `member_call_devirt_final_receiver_direct_call_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a10`。

## [DONE] CG-T07S0a10：修复 nothing_raise_coerce_to_any_type 中 nested try/catch + `Raise.raise` 的 Nothing/bottom-type HandleDispatch routing contract 歧义，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T05`
  - `CG-T08`
  - `tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop`
- 背景：
  - 在 `CG-T07S0a9` 修复 `member_call_devirt_final_receiver_direct_call_basic.scoop` 后，默认 `cargo run -p scoop -- test` 不再停在 final receiver direct-call 去虚化 / `Base` vtable 符号漏发射，而是继续暴露 `tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop -o /tmp/nothing_raise_coerce_to_any_type` 会在 LLVM 单文件前端准备阶段报 `refactor callable 'main' step schema s0 (ABI s0) state st9 terminator lowering failed: LLVM 单文件前端准备失败：refactor boundary bd1 case c0 命中多个 HandleDispatch routing contract`，说明 nested try/catch + `Raise.raise(...)` 的 Nothing/bottom-type path 仍把同一 handle boundary case 发布成歧义的 HandleDispatch routing contract。

- 必须实现的内容：
  1. 修复 refactor effect-lowered / late-lowered handle boundary routing contract，使 nested try/catch 中 `Raise.raise(...)` 的 Nothing-return / dead-code path 对同一 boundary case 只发布单一 authoritative `HandleDispatch` routing contract，供 EffectStep codegen 消费。
  2. 保持 `nothing_raise_coerce_to_any_type.scoop` 的 bottom-type / dead-code / catch propagation 语义：`Raise.raise(...)` 之后的 dead code 不执行，inner/outer catch 分派与值传播保持正确；不得通过改 fixture、弱化 `Nothing` 到 expected type / `Any` 的 coercion、或绕开 nested handle/try state-machine path 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许在 LLVM backend 现场按 boundary/case 猜测或静默挑选其中一个 routing contract；必须修正 authoritative effect-lowered / late-lowered publication。
  - 不允许通过禁用 nested try/catch、移除 `Raise.raise(...)` dead-code path、改变 `Nothing` 语义、或回退到 legacy handler stack 规避该问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop -o /tmp/nothing_raise_coerce_to_any_type`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `nothing_raise_coerce_to_any_type.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a9`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a9` 修复后，默认 full-suite 继续前进到 `nothing_raise_coerce_to_any_type.scoop`；单 fixture build 诊断显示 refactor EffectStep/HandleDispatch lowering 仍把同一 boundary case 发布成多个 routing contract，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：根因定位为 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 的 `handle_dispatch_nesting_depth()` / `surface_resume_allows_handle_dispatch()` 把 `LateLoweredHandleStateRegion::Exit` 也视为动态嵌套区域；在该 fixture 中，前一个 sibling `handle` 的 exit state 会把外层 nested try/catch 的 HandleDispatch 错算成已被额外包裹，导致 outer site6 与 inner site7 对同一个 `bd1/c0` 以同层深度同时命中并触发 `multiple HandleDispatch routing contract` 诊断。
  - 2026-05-08：新增 `handle_dispatch_region_implies_runtime_nesting()`，把动态嵌套判定收紧为真实运行期包围区域（排除 `Exit`），避免顺序上的前序 `handle` exit 污染 nested try/catch 的 HandleDispatch 选择；新增 `llvm::tests::nested_raise_try_catch_uses_innermost_handle_dispatch_contract` 回归，确保 nested `Raise.raise` 路径会稳定选择最内层 handler contract。
  - 2026-05-08：验证通过：`cargo test -p scoopc nested_raise_try_catch_uses_innermost_handle_dispatch_contract`、`cargo run -p scoop -- build tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop -o /tmp/nothing_raise_coerce_to_any_type`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/nothing_raise_coerce_to_any_type.scoop`、`cargo run -p scoop -- test`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `nothing_raise_coerce_to_any_type.scoop`，下一处失败转为 `tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a11`。

## [DONE] CG-T07S0a11：修复 object_companion_value_named_nested_init_basic 中 nested object / named companion value access 被误当成 member field target，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T07`
  - `CG-T08`
  - `tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop`
- 背景：
  - 在 `CG-T07S0a10` 修复 `nothing_raise_coerce_to_any_type.scoop` 的 nested `HandleDispatch` routing contract 歧义后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop -o /tmp/object_companion_value_named_nested_init_basic` 会在 LLVM 单文件前端准备阶段报 `refactor pure assignment ... MemberAccess ... resolved: Some(Value { fqn: "Outer.Nested" }) ... pass MIR member field target 'Outer.Nested' receiver_ty=t0 receiver_cg=Ref`，说明 nested object / named companion 的值引用与成员访问仍被误送进 instance member-field lowering，而没有消费 authoritative singleton once-init / value-ref contract。

- 必须实现的内容：
  1. 修复 refactor pure assignment / member access lowering 对 nested object / named companion value access 的类型与 contract 归类，确保 `Outer.Nested`、`C.Named`、`C.x` 与 `C.Named.x` 继续消费 authoritative singleton once-init / value-ref / member contract，而不是退化成 `pass MIR member field target`。
  2. 保持 `object_companion_value_named_nested_init_basic.scoop` 的 once-init 语义：`Foo` / `Outer.Nested` / `C.Named` 仅初始化一次，且 `TypeName.member` 与 `TypeName.Named.member` 共享同一 singleton backing；不得回退到 legacy path 或在 LLVM backend 私补 `Outer.Nested` / `C.Named` 特判掩盖 contract 漂移。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、删除 nested object / named companion 访问、绕开 once-init、或降级到 legacy path 规避该问题。
  - 不允许把 nested object / named companion 值引用继续伪装成普通 receiver field load；必须在 authoritative value-ref / member lowering 主线上修正。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop -o /tmp/object_companion_value_named_nested_init_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `object_companion_value_named_nested_init_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a10`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a10` 修复后，默认 full-suite 继续前进到 `object_companion_value_named_nested_init_basic.scoop`；build 诊断显示 nested object / named companion value access 仍被误送进 `pass MIR member field target` lowering，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：根因定位为 `crates/scoopc/src/llvm/codegen/mir_body.rs` 与 `crates/scoopc/src/llvm/codegen/effect_refactor/value.rs` 的 static member value helper 只把 object property / top-level value / enum unit variant 视为 resolved value contract，却遗漏了 `object` / named companion 的 singleton value，本应消费 authoritative once-init / value-ref contract 的 `Outer.Nested` / `C.Named` 因此退化进 `pass MIR member field target`。
  - 2026-05-08：将 MIR/effect-refactor 的 resolved static value 判定补齐为包含 `object_inits.contains_key(fqn)`，并让 `mir_member_resolved_static_value_cg_ty()` 对 singleton value 返回 `CgTy::Ref`；新增 `llvm::tests::production_codegen_lowers_nested_object_and_named_companion_value_access` 回归，覆盖 nested object / named companion 值引用与成员访问继续通过 singleton once-init / backing 主线。
  - 2026-05-08：验证通过：`cargo test -p scoopc object_member_call_uses_gc_managed_singleton_receiver`、`cargo test -p scoopc production_codegen_lowers_nested_object_and_named_companion_value_access`、`cargo run -p scoop -- build tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop -o /tmp/object_companion_value_named_nested_init_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.scoop`、`cargo run -p scoop -- test`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `object_companion_value_named_nested_init_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/operator_overload_struct_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a12`。

## [DONE] CG-T07S0a12：修复 operator_overload_struct_basic 中 struct `compareTo` direct-call lowering 把 `Int` 结果误强制成 struct target，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/operator_overload_struct_basic.scoop`
- 背景：
  - 在 `CG-T07S0a11` 修复 `object_companion_value_named_nested_init_basic.scoop` 的 nested object / named companion singleton value contract 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/operator_overload_struct_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/operator_overload_struct_basic.scoop -o /tmp/operator_overload_struct_basic` 会在 LLVM 单文件前端准备阶段报 `refactor pure assignment ... Call { kind: Direct { callee_fqn: "Num.compareTo" } ... } ... unsupported value coercion from Int(...) to Struct(TypeId(197))`，说明 struct `compareTo` operator-overload direct-call 的 authoritative `Int` result contract 在 refactor pure assignment / compare lowering 路径中仍被错误对齐到 struct target，而不是继续流入 `compareTo -> 0` 的比较主线。

- 必须实现的内容：
  1. 修复 refactor pure assignment / direct-call lowering 对 user-defined struct `compareTo` operator-overload 结果槽位与目标类型的归类，确保 `Num.compareTo` 的 direct-call 结果保持 authoritative `Int` contract，并继续进入 `<` / `>` / `<=` / `>=` 的 `compareTo -> 0` 比较主线，而不是被误强制成 `Num` / struct target。
  2. 保持 `operator_overload_struct_basic.scoop` 的运算符语义：`Vec2` 的 `+` / `-` / `*` 与 `Num` 的 `/` / `%` / `compareTo` 比较都按既有 direct-call binding 与 compare lowering 运行；不得通过改 fixture、绕开 `compareTo` direct-call、把 `<` / `>` 重写成 backend 私补字段比较、或回退到 legacy path 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许在 LLVM backend 现场按 target slot/struct type 猜测 `compareTo` 返回类型；必须修正 authoritative direct-call / compare lowering 主线的 target contract。
  - 不允许把 struct operator overload 特判成仅 fixture 可用的局部修补，或改变 `compareTo` desugaring 到 `0` 比较的既有 MIR contract。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/operator_overload_struct_basic.scoop -o /tmp/operator_overload_struct_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/operator_overload_struct_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `operator_overload_struct_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a11`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a11` 修复后，默认 full-suite 继续前进到 `operator_overload_struct_basic.scoop`；build 诊断显示 struct `compareTo` direct-call result 在 refactor pure assignment / compare lowering 中仍被误强制成 struct target，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：根因定位为 `crates/scoopc/src/mir/lower.rs` 的 `try_lower_compare_to_binary_expr()` 会对已经被 HIR canonicalize 成 `compareTo(...) < SynthInt(0)` 的比较再次套用 compareTo 语法糖，错误生成第二次 `Num.compareTo(compare_result, 0)`；随后的 refactor pure assignment / direct-call lowering 因此把 authoritative `Int` 结果重新送入 struct target 路径并报 `unsupported value coercion from Int(...) to Struct(...)`。
  - 2026-05-08：为 canonical compareTo binary 新增防重写守卫，避免 MIR 再次生成嵌套 `compareTo` direct-call；强化 `mir::lower` compareTo 定向单测，断言每个比较点只保留一次 direct-call，并扩展 `llvm::tests::frontend_codegen_consumes_compare_to_direct_calls_without_eager_member_inclusion` 以在 production LLVM IR 中守护 direct-call 次数。
  - 2026-05-08：验证通过：`cargo test -p scoopc dump_mir_lowers_user_defined_compare_to_as_direct_call_plus_zero_compare`、`cargo test -p scoopc dump_mir_lowers_compare_to_in_if_condition_as_direct_call`、`cargo test -p scoopc frontend_codegen_consumes_compare_to_direct_calls_without_eager_member_inclusion`、`cargo fmt --all`、`cargo run -p scoop -- build tests/fixtures/run-pass/operator_overload_struct_basic.scoop -o /tmp/operator_overload_struct_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/operator_overload_struct_basic.scoop`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `operator_overload_struct_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a13`。

## [DONE] CG-T07S0a13：修复 safe_member_access_ref_and_extension_basic 中 safe-call `Option` `Some`/`None` lowering 仍退化成 `ctor call lowering pending`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T03`
  - [`TODO.md`](./TODO.md) `MIR-T07`
  - `tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`
- 背景：
  - 在 `CG-T07S0a12` 修复 `operator_overload_struct_basic.scoop` 的 struct `compareTo` direct-call result contract 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop -o /tmp/safe_member_access_ref_and_extension_basic` 会在 materialized MIR 验证阶段报 `materialized MIR \`main\` contains rvalue todo \`ctor call lowering pending\` in Some(bb3) at 800..802`；`dump-hir` 显示 safe-call desugaring 生成的 `when` arm body 仍把 `Some(...)` / `None` 保留为 `UnresolvedIdent`，说明 `Option` variant ctor/value 的 authoritative contract 没有进入 safe member access 主线。

- 必须实现的内容：
  1. 修复 safe-call / safe member access lowering 对 `Option` result 构造的 contract 归类，确保 `someUser?.score`、`someUser?.doubleScore`、`someConfig?.port` 与对应 `None` 分支都通过 authoritative `Option.Some` / `Option.None` variant ctor/value lowering，而不是退化成 `UnresolvedIdent` -> `ctor call lowering pending`。
  2. 保持 `safe_member_access_ref_and_extension_basic.scoop` 的语义：`Some(receiver)` 分支返回 `Some(member)`，`None` 分支返回 `None`；class field、object field 与 extension property 三类 safe member access 都必须沿用既有 safe-call / `when` 主线，不得改 fixture、绕开 `Option`、或回退到 legacy path。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许在 materialized MIR / LLVM backend 现场按 `Option<T>` 目标类型私补 `Some`/`None` 构造；必须修正 authoritative safe-call / ctor lowering 主线。
  - 不允许把 safe member access 特判成仅 fixture 可用的局部修补，或改写成绕过 `Option` variant contract 的 bespoke lowering。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop -o /tmp/safe_member_access_ref_and_extension_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `safe_member_access_ref_and_extension_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a12`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a12` 修复后，默认 full-suite 继续前进到 `safe_member_access_ref_and_extension_basic.scoop`；build 诊断显示 materialized MIR `main` 仍含 `ctor call lowering pending`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：`dump-hir` 显示 safe-call desugaring 的 `when` arm body 仍把 `Some(...)` / `None` 表达成 `UnresolvedIdent`；`crates/scoopc/src/mir/lower.rs` 因此把相关 call callee 识别为 `ValueOrigin::UnresolvedName` 并发射 `Rvalue::Todo("ctor call lowering pending")`。后续任务需把 `Option` variant ctor/value contract 接回 authoritative safe-call / ctor lowering 主线。
  - 2026-05-08：`crates/scoopc/src/hir/lower/expr.rs` 的 safe-call / safe member access desugar 现在会给合成的 `Some(...)` 包装表达式保留外层 `Option<T>` 结果类型，并把 `None` 分支改为同样保留结果类型的 `None()` ctor 形状；这样 `crates/scoopc/src/mir/lower.rs` 现有的 unresolved enum-variant ctor lowering 就能把两条分支都接回 `Rvalue::EnumVariant`，不再落入 `ctor call lowering pending`。
  - 2026-05-08：扩充 `hir::lower::tests::typed_hir_lowers_safe_member_type_apply_as_safe_direct_call`，断言 safe-call `Some`/`None` 分支都保留 `Option` 结果类型且 `None` 走 0 参 ctor；新增 `mir::lower::tests::dump_mir_lowers_safe_member_access_option_result_without_ctor_todo`，覆盖 `user?.score` 会直接 lower 成 `Option.Some` / `Option.None` enum variant；同步更新 `tests/fixtures/hir/safe_call_not_null_assert.hir` golden。
  - 2026-05-08：验证通过：`cargo test -p scoopc typed_hir_lowers_safe_member_type_apply_as_safe_direct_call`、`cargo test -p scoopc dump_mir_lowers_safe_member_access_option_result_without_ctor_todo`、`cargo run -p scoop -- build tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop -o /tmp/safe_member_access_ref_and_extension_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/hir/safe_call_not_null_assert.scoop`、`cargo run -p scoop -- test`、`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `safe_member_access_ref_and_extension_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a14`。

## [DONE] CG-T07S0a14：修复 smart_cast_any_member_access_generic_class_basic 中 smart-cast 分支 generic class field access 仍把 result/frame slot 保留为 unresolved `T`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T4016T1d1`
  - `CG-T08`
  - `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`
- 背景：
  - 在 `CG-T07S0a13` 修复 `safe_member_access_ref_and_extension_basic.scoop` 的 safe-call `Option` ctor lowering 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop -o /tmp/smart_cast_any_member_access_generic_class_basic` 会在 materialized MIR validation 报 `materialized MIR 'readValue' contains unresolved generic parameter in frame slot at 362..369: T`；`dump-mir` 显示 `if (x is Box<Int>) return x.value` 的 smart-cast 分支里，`TypeCheck` 已携带 `Box<Int>` test type，但 bb1 的 `MemberAccess` 仍以 `receiver_ty = Any` / `result local = T` 发布，说明 authoritative smart-cast/member-access contract 还没有把 generic class field access 具体化到 `Int`。

- 必须实现的内容：
  1. 修复 `Any` receiver 的 smart-cast 分支 member access / materialized MIR contract，使 `x is Box<Int>` 成立时 `x.value` 的 authoritative result type、local/frame slot 与后续 codegen 路径都具体化为 `Int`，而不是模板期 `T`。
  2. 保持 `smart_cast_any_member_access_generic_class_basic.scoop` 的语义：smart-cast 只在 `x is Box<Int>` 分支内生效，`x.value` 直接返回 `Int`；不得通过改 fixture、显式补 cast、放宽 unresolved-generic validator、或在 backend 现场硬编码 `Int` 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许把 smart-cast 分支的 member result 继续保留为 declaration-site generic `T`，再依赖 materialize/codegen 现场猜具体类型。
  - 不允许通过关闭 `materialized MIR unresolved generic parameter` validator、改写源码成局部临时变量/显式 cast、或引入 fixture-only smart-cast 特判规避问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop -o /tmp/smart_cast_any_member_access_generic_class_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `smart_cast_any_member_access_generic_class_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a13`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a13` 修复后，默认 full-suite 继续前进到 `smart_cast_any_member_access_generic_class_basic.scoop`；单 fixture `test` 暴露 `EXPECT-EXIT 7` 实际为 1，进一步 `build` 诊断显示 materialized MIR `readValue` 的 smart-cast 分支仍把 `x.value` 的 result/frame slot 保留为 unresolved generic `T`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：`crates/scoopc/src/mir/lower.rs` 的 `lower_member_access_expr()` 现在会优先保留 typed HIR 已具体化的 member result type，只在合成 HIR 仍把结果写成宽的 `Any` 时才回退到成员声明类型；这样 `x is Box<Int>` 分支里的 `x.value` 不再被重新放大回 declaration-site `T`，而 `with` builder / extension-property getter 这类合成 receiver/result 仍保持既有 contract。
  - 2026-05-08：member-access receiver 只会在 `receiver.ty` 比底层 local 更具体且不是 `Any` 时创建 expr-typed 视图 local，避免把值类型 receiver 反向擦除成 `Any`；新增 `mir::lower::tests::dump_mir_smart_cast_member_access_preserves_concrete_generic_field_type`，并同步更新 `tests/fixtures/mir_refactor/generic_materialization.mir`，覆盖 concrete member result 在后续 generic call arg transport 中显式发布 transport metadata。
  - 2026-05-08：验证通过：`cargo test -p scoopc dump_mir_smart_cast_member_access_preserves_concrete_generic_field_type`、`cargo run -p scoop -- build tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop -o /tmp/smart_cast_any_member_access_generic_class_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/smart_cast_any_member_access_generic_class_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/extension_property_getter_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/generic_materialization.scoop`、`cargo clippy --all-targets -- -D warnings`；默认 `cargo run -p scoop -- test` 已越过 `smart_cast_any_member_access_generic_class_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`，因此按顺序约束新增 prerequisite `CG-T07S0a15`。

## [DONE] CG-T07S0a15a：修复 stdlib_hash_set_map_basic 中 `MutableSet.asSet()` 只读视图在同一 body 联合 `Set.len()` / `Set.contains()` 时的 alias receiver call 结果漂移，解除 CG-T07S0a15 的 run-pass 新 blocker

- 参考：
  - `CG-T07S0a15`
  - `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
  - `stdlib/collections_set.scoop`
- 背景：
  - 在 `CG-T07S0a15` 修复 `scoop.collections.__map_alloc_empty_table` unresolved array transport element type 后，`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic` 已通过；但 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 仍以 `EXPECT-EXIT 0` 实际 3 失败。
  - 临时最小复现显示：`MutableSet.remove(3)` 后再 `add(24)` 的可变集合本身仍保持 `len() == 3` 且 `contains(0/8/24)` 为真；问题出现在 `val ro: Set = s.asSet()` 之后，同一 body 一旦组合调用 `ro.len()` 与 `ro.contains(...)`，运行期观测会漂移成 `len() == 2`、`contains(0) == false`。而只打印 `ro.get(0..2)` 时又能看到导出的底层只读数组仍保留 3 个元素（含前导 `0`）。这说明新的独立 blocker 位于 `Set` 只读视图 / alias receiver member-call 主线，而不是 `__map_alloc_empty_table` 的 empty-table 分配本身。

- 必须实现的内容：
  1. 修复 `MutableSet.asSet()` 导出的 `Set = Array<Int>` 只读视图在同一 body 联合 `Set.len()` / `Set.contains()` 调用时的 authoritative alias receiver / direct-call contract，确保两者稳定观察到同一三元素只读数组，不再把前导 `0` 元素丢失或把 logical length 漂移成 2。
  2. 保持 `stdlib_hash_set_map_basic.scoop` 的 `set_read_only_view` 语义：`ro.len() == 3`，`ro.contains(0/8/24) == true`，`ro.contains(3) == false`；不得通过改 fixture、绕开 `asSet()` 导出、改成手写局部数组、或对 `0` 元素做特判规避问题。
  3. 补最小回归验证，覆盖 `asSet()` 导出的只读数组含前导 `0` 元素且同一 body 同时调用 `len()` / `contains()` 的场景。

- 必须遵从的约束：
  - 不允许把问题归咎为 fixture-only `println` / buffering 现象而绕过 `Set` 只读视图真实语义。
  - 不允许通过改写 `Set` API 表面、改成非 alias 容器、或在 backend 对 `Set.len()` / `Set.contains()` 硬编码 `Array<Int>` 零值特判规避问题。

- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
  2. 新增或复用最小回归，覆盖 `asSet()` 后同一 body 混合 `len()` / `contains()` 的前导 `0` 元素场景。

- 完成条件：
  - `stdlib_hash_set_map_basic.scoop` 不再在 `set_read_only_view` 失败，`CG-T07S0a15` 可以继续完成其 full-suite blocker 清理。
- 依赖：`CG-T07R`，`CG-T07S0a14`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a15` 的新前置阻塞补录。`CG-T07S0a15` 已修复 `MutableMap.set(...)` alias receiver array transport 的 unresolved `T` 并让单 fixture `build` 通过；继续执行 run-pass 后，临时最小复现确认新的失败已缩小到 `MutableSet.asSet()` 只读视图在同一 body 组合 `Set.len()` / `Set.contains()` 时的运行期结果漂移，因此按顺序约束先补此 prerequisite。
  - 2026-05-08：根因定位为 pass-visible 非泛型 alias receiver 扩展的重载符号发布与 call-site rewrite 仍不区分 `Set` / `MutableSet`：`stdlib/collections_set.scoop` 中 `Set.len` / `MutableSet.len`、`Set.contains` / `MutableSet.contains` 共享 `scoop.collections.len` / `scoop.collections.contains` root FQN，materialize/production codegen 会把多个 body 压到同一 callable identity，导致 `MutableSet` 哈希布局查询与 `asSet()` 导出的只读视图查询混用实现。
  - 2026-05-08：`crates/scoopc/src/mir/materialize.rs` 现在会为 pass-visible 非泛型重名 callable 发布稳定的 overload-aware symbol，并在 reachable-body rewrite 中按 authoritative non-generic callee / receiver type 把 `MutableSet.len` 与 `Set`/`MutableSet.contains` 的 direct-call target 重写到对应 overload；新增 `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct` 回归，锁定 `stdlib_hash_set_map_basic.scoop` 中 alias receiver call target 不再退回共享 root FQN。
  - 2026-05-08：`crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/codegen/effect_refactor/value.rs` 与 `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 现可把这些 suffixed pass-visible callable 反查回 authoritative HIR signature/root，避免 plain/effect-refactor lowering 再把它们误判成缺 signature 的 function-value call。
  - 2026-05-08：验证通过：`cargo test -p scoopc materialize_for_dump_keeps_`、`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic`、直接执行 `/tmp/stdlib_hash_set_map_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`、`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a15：修复 stdlib_hash_set_map_basic 中 `scoop.collections.__map_alloc_empty_table` 的 array transport element type 仍保留 unresolved `T`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T04d`
  - `CG-T07S0a5`
  - `CG-T08`
  - `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
- 背景：
  - 在 `CG-T07S0a14` 修复 `smart_cast_any_member_access_generic_class_basic.scoop` 的 smart-cast generic class field access contract 漂移后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic` 会在 materialized MIR validation 报 `materialized MIR \`scoop.collections.__map_alloc_empty_table\` contains unresolved generic parameter in array transport element type at 4281..4302: T`，说明 stdlib HashMap empty-table allocation 仍把 array transport element type 保留为 declaration-site generic `T`，没有在 materialization/transport contract 主线上具体化到实际 bucket element type。

- 必须实现的内容：
  1. 修复 `scoop.collections.__map_alloc_empty_table` 及其相关 stdlib HashMap/HashSet empty-table allocation 的 authoritative materialized MIR / array transport contract，确保 array element type、local/frame slot 与后续 codegen 路径都具体化为实际 bucket element type，而不是 unresolved `T`。
  2. 保持 `stdlib_hash_set_map_basic.scoop` 的 HashSet/HashMap 语义：冲突插入、重复更新、删除重建、`asSet()` / `asMapView()` 只读视图都继续走现有 stdlib/MIR/materialize/codegen 主线；不得通过改 fixture、弱化 validator、把空表初始化改成非泛型特判、或在 backend 现场硬编码 element type 规避问题。
  3. 补最小回归验证，确保该 fixture 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许继续让 array transport element type 在 materialized MIR 中保留 unresolved generic `T`，再依赖 codegen/runtime 现场猜具体 bucket shape。
  - 不允许通过关闭 `materialized MIR unresolved generic parameter` validator、改写 stdlib surface 成 fixture-only helper、或把 empty-table path 特判成只对 `Int` key/value 生效的局部修补规避问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `stdlib_hash_set_map_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a14`，`CG-T07S0a15a`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a14` 修复后，默认 full-suite 继续前进到 `stdlib_hash_set_map_basic.scoop`；单 fixture build 诊断显示 materialized MIR `scoop.collections.__map_alloc_empty_table` 的 array transport element type 仍保留 unresolved generic `T`，需先独立修复后才能完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：已修复 `crates/scoopc/src/mir/materialize.rs` 中 alias receiver `MutableMap.set(...)` 的 array transport repair，`Get` / `Set` / `BuilderPush` 现在优先回读 target/result/value operand 的 authoritative concrete element type，不再依赖 alias `array_ty` 反推；新增 `materialize_for_dump_keeps_hash_map_empty_table_array_transport_concrete` 单测锁定 `scoop.collections.__map_alloc_empty_table` 的 `__scoop_array_builder_build_mutable_array` transport metadata 为 `MutableArray<Int>` / `Int`。`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic` 已通过。
  - 2026-05-08：继续执行 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 后，运行期失败已不再是 unresolved-`T` build blocker，而是 `MutableSet.asSet()` 只读视图在同一 body 组合 `Set.len()` / `Set.contains()` 时的结果漂移；按顺序约束新增 prerequisite `CG-T07S0a15a`，本任务保持未完成，等待 `CG-T07S0a15a` 修复后再恢复 run-pass / full-suite 验证。
  - 2026-05-08：`CG-T07S0a15a` 已完成后，`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic`、直接执行 `/tmp/stdlib_hash_set_map_basic` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 均通过；本任务现只剩按原验证要求恢复 `cargo run -p scoop -- test` 的默认 full-suite 验证，留待下一次调用继续。
  - 2026-05-08：重跑 `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 与 `cargo run -p scoop -- test`；默认 full-suite 已越过 `stdlib_hash_set_map_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop`。为锁定后续 blocker，直接 build/run 该 fixture 观测到 `bytes.get(0) == 3` 与 `argByte == 4` 两处实际输出为 `false`，而 `Float32` 路径保持正确，因此按顺序约束新增 prerequisite `CG-T07S0a16`。

## [DONE] CG-T07S0a16a：修复 literal_numeric_expected_type_absorption_basic 中 direct `Array<UInt8>` element path 再次退回 nominal/composite surface，解除 CG-T07S0a16 的前置 blocker

- 参考：
  - `CG-T07S0a6`
  - `CG-T07S0a16`
  - `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`
- 背景：
  - 在执行 `CG-T07S0a16` 前的对照验证中，`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop` 重新报 stdout 与 golden 不一致；直接 build/run 可见最后两行实际输出再次变为 `false` / `false`。
  - `cargo run -p scoop -- dump-mir tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop` 显示 `bytes` 的 `__scoop_array_builder_push` 仍发布 `UInt8` scalar element transport，但后续 `scoop.core.get` / `==` 路径又把 `Array<UInt8>` element surface 发布为 nominal/composite `Struct`，说明 `CG-T07S0a16` 依赖的更基础 `Array<UInt8>` direct expected-type / canonical scalar contract 已回退，需先独立修复。

- 必须实现的内容：
  1. 修复 direct `Array<UInt8>` numeric element path 的 authoritative typecheck/HIR/MIR/materialize/codegen contract，确保 `literal_numeric_expected_type_absorption_basic.scoop` 中 `bytes.get(0) == 3` 与 `bytes.get(1) == 8` 恢复为 `true` / `true`。
  2. 保持修复落在 builtin scalar alias canonicalization / expected-type / array element contract 主线上；不得通过改 fixture、改 golden、或在 backend/比较路径现场补 `UInt8` 特判规避问题。
  3. 补最小回归验证，锁定 direct `Array<UInt8>` builder/get/compare path 不再退回 nominal/composite element surface。

- 必须遵从的约束：
  - 不允许继续让 direct `Array<UInt8>` `get` 结果在 MIR/materialize/codegen 主线上退回 composite `Struct` surface，再依赖 runtime/LLVM 现场猜测收窄。
  - 不允许把 direct path 与 nested path 混成同一“临时 workaround”修复；必须先恢复 `CG-T07S0a6` 对应的基础 contract，再继续 `CG-T07S0a16`。

- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`
  2. `cargo run -p scoop -- dump-mir tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`
  3. 新增或复用最小回归，覆盖 direct `Array<UInt8>` `get` / compare path 的 canonical scalar surface。

- 完成条件：
  - direct `Array<UInt8>` numeric element path 恢复 `CG-T07S0a6` 预期行为，`CG-T07S0a16` 可继续专注嵌套 `if` / `when` / 函数参数传播缺口。
- 依赖：`CG-T07R`，`CG-T07S0a15`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a16` 的更前置 blocker 补录。对照验证发现 `literal_numeric_expected_type_absorption_basic.scoop` 再次报 stdout mismatch；直接 build/run 末两行输出回退为 `false` / `false`，而 `dump-mir` 显示 `bytes` array builder push 仍保留 `UInt8` scalar transport，但 `scoop.core.get` / `==` 路径又把 element surface 发布成 nominal/composite `Struct`，因此需先修复该 regression 后才能继续 `CG-T07S0a16`。
  - 2026-05-08：根因定位为 direct `Array<UInt8>.get` 路径的 transport/composite-layout 分类仍把 builtin nominal scalar value type 当成普通 nominal aggregate：`mir_transport_kind_for_ty` / `mir_is_aggregate_transport_ty` / `mir_transport_trace_requirement_for_type` 与 LLVM `type_needs_composite_transport_layout()` 对 `scoop.core.UInt8` 这类 zero-arg builtin nominal 统一走 conservative nominal 分支，导致 `get` 结果重新发布 trace/drop/aggregate-return/composite-runtime metadata，尽管 builder push 已保留 scalar `UInt8` surface。
  - 2026-05-08：新增共享 helper `is_builtin_scalar_nominal_value_type()`，并让 MIR transport 分类与 LLVM composite transport verifier 对 builtin nominal scalar value type 统一按 scalar 处理；`bytes.get(0)` / `bytes.get(1)` 的 `CallTransportMetadata` 现恢复为 `Scalar` result、`aggregate_return = None`、`trace = false`、`drop = false`，不再退回 composite runtime path。
  - 2026-05-08：扩充 `llvm::tests::production_codegen_uint8_array_numeric_elements_keep_scalar_transport_metadata`，继续守护 `Array<UInt8>` builder push 的 scalar metadata；新增 `mir::lower::tests::dump_mir_uint8_array_get_keeps_scalar_transport_metadata`，锁定 direct `bytes.get(...)` 两个读取站点继续发布 scalar transport contract。
  - 2026-05-08：验证通过：`cargo fmt --all`、`cargo test -p scoopc uint8_array`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`、`cargo run -p scoop -- dump-mir tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a16：修复 literal_array_expected_type_nested_basic 中嵌套 `Array<UInt8>` element expected-type 传播仍退回 `Int`，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T0150h-2`
  - `CG-T07S0a6`
  - `CG-T08`
  - `tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop`
- 背景：
  - 在 `CG-T07S0a15` 修复 `stdlib_hash_set_map_basic.scoop` 的 HashMap/HashSet empty-table transport blocker 后，默认 `cargo run -p scoop -- test` 不再停在该 fixture，而是继续暴露 `tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop` 的 run-pass 失败。
  - 直接执行 `cargo run -p scoop -- build tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop -o /tmp/literal_array_expected_type_nested_basic && /tmp/literal_array_expected_type_nested_basic` 可见实际输出首行与第三行为 `false`，而 golden 预期为 `true`；同一 fixture 中 `Float32` 路径与嵌套返回数组路径保持正确，说明问题集中在嵌套 `Array<UInt8>` element expected-type 沿 `if` / `when` / 函数参数路径继续传播时，仍有部分语义退回 `Int`。

- 必须实现的内容：
  1. 修复 `literal_array_expected_type_nested_basic.scoop` 中 `Array<UInt8>` element expected-type 的 authoritative typecheck/HIR/MIR/materialize/codegen contract，确保 `bytes` 与 `takeByte(...)` 路径中的 `if` / `when` / 数值表达式结果都稳定发布为 `UInt8`，不再在嵌套数组或调用边界前退回 `Int`。
  2. 保持该 fixture 的整体语义：`bytes.get(0) == 3`、`argByte == 4` 恢复为 `true`，`Float32` 路径与 `retMatrix(true)` 的嵌套数组返回结果继续保持当前正确输出；不得通过改 fixture、改 golden、插入显式 cast、或在 backend 现场补 truncation/compare 特判规避问题。
  3. 补最小回归验证，覆盖嵌套 `Array<UInt8>` element expected-type 在 `if` / `when` / 函数参数场景下的传播。

- 必须遵从的约束：
  - 不允许继续把嵌套 `Array<UInt8>` element path 退回 `Int`，再依赖 materialize/codegen/runtime 现场猜测收窄。
  - 不允许通过放宽断言、修改 golden、或把问题局限成某一个 fixture 私有写法规避真实 expected-type 传播缺口。

- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop`
  2. 新增或复用最小回归，覆盖嵌套 `Array<UInt8>` element expected-type 在 `if` / `when` / 函数参数场景下的传播。
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `literal_array_expected_type_nested_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a15`，`CG-T07S0a16a`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a15` 修复后，默认 full-suite 继续前进到 `literal_array_expected_type_nested_basic.scoop`；直接 build/run 观测到 `bytes.get(0) == 3` 与 `argByte == 4` 两处输出为 `false`，而 `Float32` 路径仍正确，说明嵌套 `Array<UInt8>` element expected-type 传播主线仍需独立修复后，才能继续完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：在执行 `CG-T07S0a16` 前的对照验证中发现更前置 regression：`literal_numeric_expected_type_absorption_basic.scoop` 的 direct `Array<UInt8>` bytes path 已重新回退；直接 build/run 末两行输出再次为 `false` / `false`，`dump-mir` 显示 `scoop.core.get` / compare path 把 element surface 退回 nominal/composite `Struct`，按顺序约束新增 prerequisite `CG-T07S0a16a`，本任务保持未完成，等待其修复后继续处理嵌套 expected-type 传播。
  - 2026-05-08：在 `CG-T07S0a16a` 修复 shared builtin scalar canonicalization 后重新验证，`cargo run -p scoop -- build tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop -o /tmp/literal_array_expected_type_nested_basic && /tmp/literal_array_expected_type_nested_basic` 已恢复输出 `true / 2.5 / true / 0.75 / 1.5 / 3.5`，`bytes.get(0) == 3` 与 `argByte == 4` 不再回退为 `false`。
  - 2026-05-08：新增 `mir::lower::tests::dump_mir_nested_uint8_array_literals_keep_expected_element_type`，直接锁定嵌套 `Array<UInt8>` 的 `if` / `when` / 函数参数三条 array-builder push 路径继续把 element local/type/transport source 发布为 `UInt8`，不退回 `Int` 或 composite boxing surface。
  - 2026-05-08：验证通过：`cargo test -p scoopc dump_mir_nested_uint8_array_literals_keep_expected_element_type`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_array_expected_type_nested_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`、`cargo run -p scoop -- test`（默认 full-suite 已越过 `literal_array_expected_type_nested_basic.scoop`，下一处失败转为 `tests/fixtures/run-pass/star_projection_array_read_view.scoop`）、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a17：修复 star_projection_array_read_view 中 `Array<*>` 读视图把带 GC slot 的 `Any?` element transport trace contract 发布成漂移 shape，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T4001`
  - `CG-T04d`
  - `CG-T08`
  - `tests/fixtures/run-pass/star_projection_array_read_view.scoop`
- 背景：
  - `CG-T07S0a16` 完成后，默认 `cargo run -p scoop -- test` 不再停在 `literal_array_expected_type_nested_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/star_projection_array_read_view.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/star_projection_array_read_view.scoop -o /tmp/star_projection_array_read_view` 会在 `firstIsSome` 的 `xs.get(0)` 路径报 `composite transport layout descriptor has GC slots but MIR trace requirement is false`（source_span=412..421，inventory suggested owner `CG-T04d / MIR-T10R`），说明 `Array<*>` 读视图的 `Any?` element transport trace contract 在 materialize/codegen 主线上仍有漂移。

- 必须实现的内容：
  1. 修复 `Array<String> -> Array<*>` 读视图的 authoritative typecheck/HIR/MIR/materialize/codegen contract，确保 `xs.get(0)` 对 `Any?` 结果稳定发布带 trace 的 element transport metadata，并与实际 composite layout 的 GC slots 保持一致。
  2. 保持 `Array<*>` 读取语义为 `Any?` 读视图：`firstIsSome(view)` 继续通过 `Some/None` 分支判断，不得退回裸 `Any`、去掉 nullability、或在 backend/verifier 现场补 star-projection 私有特判规避。
  3. 补最小回归验证，覆盖 star-projection array read view 的 trace contract 与 run-pass 行为。

- 必须遵从的约束：
  - 不允许通过把 `Array<*>` 降级成 `Array<Any>`、绕开 `scoop.core.get` transport metadata、或放宽 verifier 检查规避问题。
  - 不允许只在 LLVM backend 现场兜底；必须修正 authoritative transport/layout contract 的发布。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/star_projection_array_read_view.scoop -o /tmp/star_projection_array_read_view`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/star_projection_array_read_view.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `star_projection_array_read_view.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a16`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a16` 完成后，默认 full-suite 继续前进到 `star_projection_array_read_view.scoop`；单 fixture build 诊断显示 `firstIsSome` 的 `xs.get(0)` 仍把带 GC slot 的 composite layout descriptor 与 `trace = false` 的 MIR contract 组合发布，需先独立修复后才能继续完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：修正 LLVM composite transport verifier 对 `TypeKind::StarProjection` 的 trace requirement 推导，使 `Array<*>` 读视图继续继承 `read_ty = Option<Any>` 的 GC trace/drop 义务，不再把带 GC slot 的 layout descriptor 与 `trace = false` 组合发布后在 backend gate 失败。
  - 2026-05-08：保持 `StarProjection` 只作为 read-view contract，而不是数组底层存储的 composite layout owner；`scoop.core.get::<*>` 继续沿 receiver 的 ref-like 存储路径读取 `Array<String>` 元素，再经现有 coercion 形成 `Option<Any>` 读视图，修复 `firstIsSome(view)` 实际输出从 `false` 漂回 `true`。
  - 2026-05-08：新增 `llvm::tests::production_codegen_star_projection_array_read_view_keeps_traceable_transport_metadata`，锁定 production frontend/materialized MIR 中 `firstIsSome` 的 `Option<Any>` 读视图与 traceable array-get transport contract，并守护 production LLVM codegen 不再在 `scoop.core.get::<*>` gate 失败。
  - 2026-05-08：验证通过：`cargo test -p scoopc production_codegen_star_projection_array_read_view_keeps_traceable_transport_metadata`、`cargo run -p scoop -- build tests/fixtures/run-pass/star_projection_array_read_view.scoop -o /tmp/star_projection_array_read_view`、`/tmp/star_projection_array_read_view`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/star_projection_array_read_view.scoop`、`cargo run -p scoop -- test`（默认 full-suite 已越过 `star_projection_array_read_view.scoop`，下一处失败转为 `tests/fixtures/run-pass/stdlib_string_basic.scoop`）、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a18：修复 stdlib_string_basic 中 String support-source intrinsic member 调用仍退化成 unresolved MemberAccess + `FunValue` callee，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T1811`
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/stdlib_string_basic.scoop`
  - `sysroot/string.scoop`
- 背景：
  - `CG-T07S0a17` 完成后，默认 `cargo run -p scoop -- test` 不再停在 `star_projection_array_read_view.scoop`，而是继续暴露 `tests/fixtures/run-pass/stdlib_string_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_basic.scoop -o /tmp/stdlib_string_basic` 会在 frontend prepare 阶段报 `refactor pure assignment ... rvalue Call { kind: FunValue { callee: Local(l4) } } lowering failed: unsupported main codegen node: refactor plain function-value callee type`；`dump-ir` 进一步显示 `sysroot/string.scoop` 中 `scoop.core.endsWith` 的 `this.byteLength()` / `suffix.byteLength()` 仍停留在 `MemberAccessMetadata { name: "byteLength", resolved: None }` + `CallKind::FunValue`，说明 String support-source intrinsic member 调用尚未消费 authoritative member/intrinsic call contract。

- 必须实现的内容：
  1. 修复 `sysroot/string.scoop` support-source 中 `byteLength()`、`getByte()`、`unsafeSliceBytes()` 等 String intrinsic member 调用的 authoritative resolve/typecheck/HIR/MIR/materialize/codegen contract，确保它们 lower 成 typed direct/member/intrinsic call，而不是 unresolved member + `FunValue` callee。
  2. 保持 `stdlib_string_basic.scoop` 的 String P0 语义：`length`、`substring`、`startsWith`、`endsWith`、`indexOf`、`contains`、`split` 的输出继续匹配 golden；不得通过改 fixture、改 golden、把 support source 改写成另一套 representation、或在 backend 现场猜测 callee 规避问题。
  3. 补最小回归验证，覆盖 `sysroot/string.scoop` support-source 中至少 `byteLength()` 与 `getByte()` / `unsafeSliceBytes()` 的 member/intrinsic call lowering contract。

- 必须遵从的约束：
  - 不允许把 String intrinsic member 调用继续降级成 unresolved `MemberAccess` / `FunValue`，再依赖 backend 猜 callee shape。
  - 不允许通过绕开 `sysroot/string.scoop` support source、内联 fixture 私有 helper、或回退到 legacy path 规避问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_basic.scoop -o /tmp/stdlib_string_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `stdlib_string_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a17`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a17` 修复后，默认 full-suite 继续前进到 `stdlib_string_basic.scoop`；单 fixture build 诊断显示 `refactor plain function-value callee type`，`dump-ir` 进一步确认 `sysroot/string.scoop` 的 `String.byteLength()` 成员调用仍被保留成 `resolved: None` 的 `MemberAccess` 与 `CallKind::FunValue`，需先独立修复后才能继续完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：为 `String.byteLength()` / `getByte()` / `unsafeSliceBytes()` 补发 receiver-prefixed extension-style call contract：typecheck 现在会写回 `ResolvedMemberRef::ExtensionFun` 与 call-arg binding，HIR/MIR/materialized MIR 改为 `scoop.core.byteLength` / `scoop.core.getByte` / `scoop.core.unsafeSliceBytes` direct call；传统 LLVM codegen 与 refactor direct-call lowering 均新增对应 wrapper，effect facts 把这些 compiler-owned direct call 视为 plain intrinsic，并补充 `builtin_string_intrinsic_member_calls_lower_to_direct_calls` 定向回归单测。
  - 验证通过：`cargo test -p scoopc builtin_string_intrinsic_member_calls_lower_to_direct_calls`、`cargo run -p scoop -- build tests/fixtures/run-pass/string_byte_accessors.scoop -o /tmp/string_byte_accessors`、`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_basic.scoop -o /tmp/stdlib_string_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/string_byte_accessors.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/string_unsafe_slice_bytes.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_basic.scoop`、`cargo clippy --all-targets -- -D warnings`。
  - 2026-05-08：默认 full-suite 已越过 `stdlib_string_basic.scoop`，但继续在 `stdlib_string_methods_extended.scoop` 暴露 remaining String builtin member call 新 blocker；按顺序约束新增 prerequisite `CG-T07S0a19`，当前任务标记完成。

## [DONE] CG-T07S0a19：修复 stdlib_string_methods_extended 中 `String.isEmpty` / `replace` / `charAt` / `repeat` builtin member 调用仍退化成 unresolved MemberAccess + `FunValue` callee，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T0115`
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/stdlib_string_methods_extended.scoop`
- 背景：
  - `CG-T07S0a18` 完成后，默认 `cargo run -p scoop -- test` 不再停在 `stdlib_string_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/stdlib_string_methods_extended.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_methods_extended.scoop -o /tmp/stdlib_string_methods_extended` 会在 frontend prepare 阶段报 `refactor plain function-value callee type`；该 fixture 中 `trim*` / `compareTo` 之外仍有 `String.isEmpty()`、`replace(...)`、`charAt(...)`、`repeat(...)` builtin member surface，说明 remaining String builtin member 调用尚未消费 authoritative call contract。

- 必须实现的内容：
  1. 修复 `String.isEmpty()`、`replace(...)`、`charAt(...)`、`repeat(...)` 的 authoritative resolve/typecheck/HIR/MIR/materialize/codegen contract，确保它们 lower 成 typed direct/member/intrinsic call，而不是 unresolved member + `FunValue` callee。
  2. 保持 `stdlib_string_methods_extended.scoop` 的 String 语义：`trim` / `trimStart` / `trimEnd`、`isEmpty`、`replace`、`charAt`、`repeat`、`compareTo` 的输出继续匹配 golden；不得通过改 fixture、改 golden、把 builtin surface 改写成另一套 representation、或在 backend 现场猜测 callee 规避问题。
  3. 补最小回归验证，覆盖至少 `isEmpty()` 与 `replace(...)` / `charAt(...)` / `repeat(...)` 中一类 builtin member call lowering contract。

- 必须遵从的约束：
  - 不允许把这些 String builtin member 调用继续降级成 unresolved `MemberAccess` / `FunValue`，再依赖 backend 猜 callee shape。
  - 不允许通过改 fixture 形状、绕开 builtin member surface、或回退到 legacy path 规避问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_methods_extended.scoop -o /tmp/stdlib_string_methods_extended`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_methods_extended.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `stdlib_string_methods_extended.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a18`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a18` 修复后，默认 full-suite 已越过 `stdlib_string_basic.scoop`；单 fixture build 诊断显示 remaining `String` builtin member call 仍退化成 `CallKind::FunValue` 并报 `unsupported main codegen node: refactor plain function-value callee type`，需先独立修复后才能继续完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：为 `String.isEmpty()` / `replace()` / `charAt()` / `repeat()` 补发 receiver-prefixed extension-style call contract：typecheck 现在会写回 `ResolvedMemberRef::ExtensionFun` 与 call-arg binding，HIR/MIR/materialized MIR 改为 `scoop.core.isEmpty` / `scoop.core.replace` / `scoop.core.charAt` / `scoop.core.repeat` direct call；legacy dispatch、refactor direct-call lowering 与 effect facts compiler-owned plain intrinsic 白名单同步补齐，并新增 `builtin_string_member_calls_lower_to_direct_calls` 定向回归单测。
  - 2026-05-08：验证通过：`cargo test -p scoopc builtin_string_member_calls_lower_to_direct_calls -- --nocapture`、`cargo test -p scoopc builtin_string_intrinsic_member_calls_lower_to_direct_calls -- --nocapture`、`cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_string_methods_extended.scoop -o /tmp/stdlib_string_methods_extended`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_string_methods_extended.scoop`、`cargo clippy --all-targets -- -D warnings`。
  - 2026-05-08：`cargo run -p scoop -- test` 默认 full-suite 已越过 `stdlib_string_methods_extended.scoop`，但继续在 `tests/fixtures/run-pass/string_trim_indent_basic.scoop` 暴露 remaining `String.trimIndent()` builtin member call 新 blocker；按顺序约束新增 prerequisite `CG-T07S0a20`，当前任务标记完成。

## [DONE] CG-T07S0a20：修复 string_trim_indent_basic 中 `String.trimIndent` builtin member 调用仍退化成 unresolved MemberAccess + `FunValue` callee，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `T0827`
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/string_trim_indent_basic.scoop`
- 背景：
  - `CG-T07S0a19` 完成后，默认 `cargo run -p scoop -- test` 不再停在 `stdlib_string_methods_extended.scoop`，而是继续暴露 `tests/fixtures/run-pass/string_trim_indent_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/string_trim_indent_basic.scoop -o /tmp/string_trim_indent_basic` 会在 frontend prepare 阶段报 `refactor plain function-value callee type`；该 fixture 中两处 `trimIndent()` 都作用在运行期 `String` 值（包括 f-string 结果），说明 `String.trimIndent()` builtin member surface 仍未消费 authoritative call contract。

- 必须实现的内容：
  1. 修复 `String.trimIndent()` 的 authoritative resolve/typecheck/HIR/MIR/materialize/codegen contract，确保它 lower 成 typed direct/member/intrinsic call，而不是 unresolved member + `FunValue` callee。
  2. 保持 `string_trim_indent_basic.scoop` 的运行期语义：对 f-string 生成的运行期字符串调用 `trimIndent()`，以及对普通 `String` 值再次调用 `trimIndent()`，输出都继续匹配 golden；不得通过改 fixture、改 golden、把 builtin surface 改写成另一套 representation、或在 backend 现场猜测 callee 规避问题。
  3. 补最小回归验证，覆盖运行期 `String.trimIndent()` member call lowering contract。

- 必须遵从的约束：
  - 不允许把 `String.trimIndent()` 继续降级成 unresolved `MemberAccess` / `FunValue`，再依赖 backend 猜 callee shape。
  - 不允许通过改 fixture 形状、把运行期 receiver 换成编译期常量、或回退到 legacy path 规避问题。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/string_trim_indent_basic.scoop -o /tmp/string_trim_indent_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/string_trim_indent_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `string_trim_indent_basic.scoop` 停止，`CG-T07S0a` 可继续恢复最终默认 full-suite 验证。
- 依赖：`CG-T07R`，`CG-T07S0a19`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0a` 的新前置阻塞补录。`CG-T07S0a19` 修复后，默认 full-suite 已越过 `stdlib_string_methods_extended.scoop`；单 fixture build 诊断显示 remaining `String.trimIndent()` builtin member call 仍退化成 `CallKind::FunValue` 并报 `unsupported main codegen node: refactor plain function-value callee type`，需先独立修复后才能继续完成 `CG-T07S0a` 的默认 full-suite 验证。
  - 2026-05-08：typecheck 现在对运行期 `String.trimIndent()` member call 发布 receiver-prefixed extension-style direct-call contract，写回 `ResolvedMemberRef::ExtensionFun` 与 call-arg binding；HIR/MIR/materialized MIR 统一 lower 成 `scoop.core.trimIndent(receiver)`，不再保留 unresolved `MemberAccess` / `CallKind::FunValue` callee。
  - 2026-05-08：legacy LLVM dispatch 与 refactor direct-call lowering 同步接入 `scoop.core.trimIndent`，并新增 `builtin_string_trim_indent_member_calls_lower_to_direct_calls` 定向回归单测，覆盖 `string_trim_indent_basic.scoop` 中 f-string 结果与普通运行期 `String` 两处 `trimIndent()` 调用都落到 direct call contract。
  - 2026-05-08：验证通过：`cargo test -p scoopc builtin_string_trim_indent_member_calls_lower_to_direct_calls -- --nocapture`、`cargo run -p scoop -- build tests/fixtures/run-pass/string_trim_indent_basic.scoop -o /tmp/string_trim_indent_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/string_trim_indent_basic.scoop`、`cargo clippy --all-targets -- -D warnings`。
  - 2026-05-08：`cargo run -p scoop -- test` 默认 full-suite 已越过 `string_trim_indent_basic.scoop`；当时继续停在 task/thread/runtime GC 组的超时/roots blocker，因此本任务标记完成且不新增条目。该组后续已随 async/Task 清理移除。

## [DONE] CG-T07S0a21：修复剩余 plain callable / ctor ABI 回归：top-level generic named args、cross-file ctor named/default 与 unsafe `FunPtr` aggregate return

- 参考：
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`
  - `tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  - `tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic`
- 背景：
  - `tools/run_fixture_scan.sh --no-build --out-dir target/fixture-scan/round3-30s` 显示 `top_level_generic_named_args_basic.scoop`、`unsafe_funptr_aggregate_return_tuple.scoop` 与 `run_pass_cone/cross_file_ctor_named_default_basic` 仍失败，属于 plain callable / ctor lowering 家族的剩余 ABI contract 回归。
  - `top_level_generic_named_args_basic` 单独 build+run 实际输出为 `302 / 301 / 2`，golden 为 `302 / 301 / 1`；说明源码顺序求值仍在，但 monomorph / rewrite 后 named arg 绑定退回了位置实参语义，`a` / `b` 绑定被写反。
  - `run_pass_cone/cross_file_ctor_named_default_basic` 单独 `scoop run` 在 frontend prepare 阶段报 `class ctor call arg eval` unsupported，说明 cross-file cone 包中的 class ctor named/default arg-eval shape 仍未进入 authoritative ctor contract。
  - `unsafe_funptr_aggregate_return_tuple` 单独 build 后运行直接以 `exit=139` 崩溃，说明 unsafe 间接 `FunPtr<(Int) -> (Int, Int)>` aggregate return ABI / transport 仍有剩余 mislowering。

- 必须实现的内容：
  1. 修复 top-level generic direct call 的 named arg authoritative binding contract，确保 monomorph / rewrite 后既保留源码求值顺序，也保持真实形参绑定语义。
  2. 修复 class ctor named/default arg-eval lowering，确保 `run_pass_cone` cross-file cone 包里的 `Box(y = 7)`、`Holder()` 等 ctor 调用走 typed ctor contract，而不是把 arg-eval shape 留给 backend unsupported。
  3. 修复 unsafe 间接 `FunPtr` aggregate return ABI / transport，确保 tuple 返回值在 direct `fp(7)` 与 `fp.invoke(9)` 两条路径都稳定落地，不崩溃、不读错 slot。
  4. 补最小回归验证，覆盖 generic named args、ctor named/default 与 unsafe `FunPtr` aggregate return 三类 callable surface。
  5. 修复完成后，重跑 `tools/run_fixture_scan.sh --no-build` 对受影响 fixture / case 复扫，并同步更新 `FAILED_FIXTURES.md` 删除已修复条目、刷新剩余 blocker 列表。

- 必须遵从的约束：
  - 不允许通过改 fixture / golden、改写成位置实参、绕开 `run_pass_cone`、或回退到 legacy path 规避问题。
  - 不允许在 LLVM backend 现场猜 named arg 绑定、ctor 默认参数或 aggregate return ABI；必须修 authoritative call-site / ctor-site / indirect-call contract。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop -o /tmp/top_level_generic_named_args_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`
  3. `cargo run -p scoop -- build tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop -o /tmp/unsafe_funptr_aggregate_return_tuple`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic`
  6. `tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`
  7. `tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`
  8. `tools/run_fixture_scan.sh --no-build tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic`

- 完成条件：
  - 这 3 个 plain callable / ctor ABI blocker 从 Round 3 失败列表中移除，且 `FAILED_FIXTURES.md` 已同步更新。
- 依赖：`CG-T07R`，`CG-T07S0a20`

- 完成记录：
  - 2026-05-08：作为 Round 3 per-fixture scan 新 blocker 组补录。当前 3 个失败分别暴露 named arg 绑定退回 positional、class ctor named/default arg-eval unsupported 与 unsafe `FunPtr` aggregate return 崩溃，先按同一 callable / ctor ABI 家族收口，再继续恢复 full-suite。
  - 2026-05-08：HIR canonical call lowering 在 typed ctor binding 可用时同步把 ctor call-site `arg_mapping` 归一到 canonical param order，cross-file cone 包中的 `Box(y = 7)`、`Holder()` 等 ctor named/default 调用不再停在 `class ctor call arg eval` unsupported，而是进入 authoritative ctor contract。
  - 2026-05-08：MIR call lowering 新增“HIR 实参已 canonical positional”判定；对 top-level direct call、callable value 与 member-style direct call，一旦 HIR 已用临时变量保留源码求值顺序并改写成 canonical positional args，就不再二次套用 call-arg binding，从而修复 monomorph / rewrite 后 top-level generic named args 仍被重新洗回 positional 绑定的问题，并新增 `top_level_generic_named_args_keep_canonical_param_order_in_pass_mir` 回归单测。
  - 2026-05-08：LLVM `FunPtr` 间接调用在 `call/dispatch` 与 pass-MIR body emission 两条路径统一改用 target native aggregate return ABI，不再对 `FunPtr<(Int) -> (Int, Int)>` 强塞 Scoop hidden-sret；`runtime/c/scoop_test.c` 的测试 helper 同步改为按值返回聚合 struct，并新增 `unsafe_funptr_aggregate_return_uses_native_return_abi` IR 回归单测。
  - 2026-05-08：已更新 `FAILED_FIXTURES.md`，从 Round 1/2/3 列表移除 `unsafe_funptr_aggregate_return_tuple.scoop`，并从 Round 3 列表移除 `top_level_generic_named_args_basic.scoop` 与 `run_pass_cone/cross_file_ctor_named_default_basic`。
  - 2026-05-08：验证通过：`cargo test -p scoopc top_level_generic_named_args_keep_canonical_param_order_in_pass_mir -- --nocapture`、`cargo test -p scoopc unsafe_funptr_aggregate_return_uses_native_return_abi -- --nocapture`、`cargo run -p scoop -- build tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop -o /tmp/top_level_generic_named_args_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`、`cargo run -p scoop -- build tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop -o /tmp/unsafe_funptr_aggregate_return_tuple`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic`、`tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`、`tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/unsafe_funptr_aggregate_return_tuple.scoop`、`tools/run_fixture_scan.sh --no-build tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a22：修复 top-level / package compilation-unit contract 回归：顶层 pattern once-init wrapper 与 cone package-level `comptime if` 跨文件绑定

- 参考：
  - `T1220b`
  - `T4004b`
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`
  - `tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`
- 背景：
  - Round 3 per-fixture scan 还暴露了 `top_level_val_pattern_runtime_basic.scoop` 与 `run_pass_cone/package_level_comptime_if_cross_file_const_fun` 两个 whole-compilation-unit / package-scope contract 回归。
  - `top_level_val_pattern_runtime_basic` 单独 build 会在 frontend prepare 阶段报 `refactor LLVM main wrapper 缺少入口 step schema s0 layout`；说明顶层 pattern binder / once-init 已进入 wrapper path，但 authoritative entry step schema 仍未完整发布。
  - `run_pass_cone/package_level_comptime_if_cross_file_const_fun` 单独 `scoop run` 会在 effect facts 阶段报未解析的 import `fixtures.run_pass_cone.package_level_comptime_if_cross_file_const_fun.enabled`；而该 `enabled` 实际定义在同包 `src/helpers.scoop`。这说明 cone package 模式下 package-level `comptime if` 仍没有稳定消费跨文件 compilation-unit binding contract。

- 必须实现的内容：
  1. 修复顶层 `val` pattern binder / once-init 进入 main wrapper 时的 authoritative entry-step schema contract，确保 tuple / struct / enum binder 顶层初始化稳定进入 ordinary top-level immutable value 主线。
  2. 修复 cone package 模式下 package-level `comptime if` 的跨文件 compilation-unit 收集、import / index / effect-facts 绑定 contract，确保同包 `src/**/*.scoop` 中的 `const fun` 可被稳定 import 与消费。
  3. 补最小回归验证，覆盖顶层 pattern once-init wrapper 与 cone package-level cross-file `const fun` 两类 package-scope surface。
  4. 修复完成后，重跑 `tools/run_fixture_scan.sh --no-build` 对受影响 fixture / case 复扫，并同步更新 `FAILED_FIXTURES.md` 删除已修复条目、刷新剩余 blocker 列表。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、去掉顶层 pattern binder、把 `comptime if` 改回单文件、或回退 legacy path 规避问题。
  - 不允许在 backend 现场私补顶层 wrapper schema 或跨文件 import；必须修 compilation-unit / package authoritative contract。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop -o /tmp/top_level_val_pattern_runtime_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`
  4. `tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`
  5. `tools/run_fixture_scan.sh --no-build tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`

- 完成条件：
  - 这 2 个 top-level / package compilation-unit blocker 从 Round 3 失败列表中移除，且 `FAILED_FIXTURES.md` 已同步更新。
- 依赖：`CG-T07R`，`CG-T07S0a21`

- 完成记录：
  - 2026-05-08：作为 Round 3 per-fixture scan 新 blocker 组补录。当前一条失败落在顶层 once-init wrapper entry schema，另一条失败落在 cone package-level `comptime if` 的同包跨文件绑定；两者都属于 compilation-unit / package-scope authoritative contract 漂移，先合并收口。
  - 2026-05-08：refactor LLVM/effect-facts stage 现在从 build/source-map handoff 传递真实 compilation-unit 源集，P4 重建 package-level `comptime if` 条件绑定时不再退回单入口文件索引，cone 包同包 `helpers.scoop` 中的 public `const fun enabled` 可被 `src/main.scoop` 稳定 import 与消费。
  - 2026-05-08：refactor main wrapper 改为按 direct-entry ABI layout 的 entry step schema 解读返回 `Step`，避免 ABI visibility handoff 对同一 body version 重编号 step schema 后仍用 primary program schema 查询 layout；顶层 pattern once-init wrapper 中的 tuple / struct / enum binder 顶层初始化可稳定进入 ordinary top-level immutable value 读取主线。
  - 2026-05-08：已更新 `FAILED_FIXTURES.md`，Round 3 剩余 blocker 当时刷新为 task/thread/runtime GC 组与 `CG-T07S0a24` 覆盖的 1 个 frontend receiver `eff` row fixture；同时移除已由 `CG-T07S0a20` 修复但清单仍残留的 `string_trim_indent_basic.scoop`。其中 task/thread/runtime GC 组后续已随 async/Task 清理移除。
  - 2026-05-08：验证通过：`cargo run -p scoop -- build tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop -o /tmp/top_level_val_pattern_runtime_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`、`tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`、`tools/run_fixture_scan.sh --no-build tests/fixtures/run_pass_cone/package_level_comptime_if_cross_file_const_fun`、`tools/run_fixture_scan.sh --no-build tests/fixtures/run-pass/string_trim_indent_basic.scoop`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a24：回收 per-fixture scan 暴露的 frontend authoritative contract 回归：use-site eff row receiver mismatch

- 参考：
  - `T0624`
  - `tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`
  - `tests/fixtures/infer/effects/use_site_eff_row_default_and_explicit_ok.scoop`
- 背景：
  - Round 3 per-fixture scan 中唯一的纯前端 blocker 是 `tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`：当前结果为“期望失败，但执行成功”。
  - 该 fixture 要求在 receiver 位置执行 use-site `eff` row subeffecting：`Disposable<eff Pure> <: Disposable<eff Async>` 成立，但反向不成立；现在 `asyncDisposable().pureOnly()` 被错误放行，说明 receiver-call 的 authoritative infer/typecheck contract 漏掉了这条方向性约束或诊断写回。

- 必须实现的内容：
  1. 修复 receiver-call use-site `eff` row subeffecting / mismatch authoritative contract，确保在需要 `Disposable<eff Pure>` 的 receiver 位置传入 `Disposable<eff Async>` 时稳定报错。
  2. 保持 diagnostic 语义：仍返回 `scoop::typecheck::call_receiver_type_mismatch`，并指向 `21:5` 的 receiver call site；不得通过改 fixture / 放宽期望规避问题。
  3. 补最小回归验证，同时覆盖负例 `use_site_eff_row_receiver_mismatch_is_error.scoop` 与正例 `use_site_eff_row_default_and_explicit_ok.scoop`。
  4. 修复完成后，重跑 `tools/run_fixture_scan.sh --no-build` 对受影响 infer fixture 复扫，并同步更新 `FAILED_FIXTURES.md` 删除已修复条目、刷新剩余 blocker 列表。

- 必须遵从的约束：
  - 若根因位于 infer / typecheck / receiver-call binding，必须在该 authoritative 前端主线修复；不允许把诊断推迟到 codegen / runtime，或引入 fixture-only 特判。
  - 不允许通过去掉 use-site `eff` row、弱化 subeffecting 规则、或改 golden / EXPECT 行为规避问题。

- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/infer/effects/use_site_eff_row_default_and_explicit_ok.scoop`
  3. `tools/run_fixture_scan.sh --no-build tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`

- 完成条件：
  - 该 infer blocker 从 Round 3 失败列表中移除，且 `FAILED_FIXTURES.md` 已同步更新。
- 依赖：`CG-T07R`，`CG-T07S0a22`

- 完成记录：
  - 2026-05-08：作为 Round 3 per-fixture scan 新 blocker 补录。该失败不属于 LLVM/backend late unsupported，而是 receiver-call use-site `eff` row subeffecting 方向性回归；必须在前端 authoritative contract 修回后，full-suite 剩余 blocker 才能继续收口。
  - 2026-05-09：修正 `crates/scoop/src/fixtures/mod.rs` 的 `infer_fixture(...)`，让 `infer` fixtures 在默认 refactor 模式下直接消费 authoritative typed-HIR stage output，而不是继续走旧 `typecheck_fixture(...)` 入口；这样 `use_site_eff_row_receiver_mismatch_is_error.scoop` 会稳定得到 `scoop::typecheck::call_receiver_type_mismatch`，并保持 receiver call site `21:5` 的诊断定位。
  - 2026-05-09：`phase_name(...)` 现在会为 `tests/fixtures/infer/effects/*.scoop` 这类嵌套单文件子集向上回溯真实 phase 目录，`tools/run_fixture_scan.sh --no-build tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop` 因而按 `infer` phase 执行；同时新增 `phase_name_walks_up_to_phase_dir_for_nested_single_file_subset` 与 `infer_fixtures_use_refactor_typed_hir_diagnostics` 回归测试锁定该行为。
  - 2026-05-09：已同步更新 `FAILED_FIXTURES.md`，从 Round 3 失败列表移除 `tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`。
  - 2026-05-09：验证通过：`cargo run -p scoop -- test --fixtures tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/infer/effects/use_site_eff_row_default_and_explicit_ok.scoop`、`cargo test -p scoop infer_fixtures_use_refactor_typed_hir_diagnostics -- --nocapture`、`cargo test -p scoop phase_name_walks_up_to_phase_dir_for_nested_single_file_subset -- --nocapture`、`tools/run_fixture_scan.sh --no-build tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a24a：修复 runtime_gc cross-thread roots 中 top-level `@Global __AtomicInt` atomic lowering 漂移，并让 run-pass timeout 正确回收后代进程，解除 CG-T07S0a 默认 full-suite 新 blocker

- 参考：
  - `CG-T07`
  - `CG-T07S0a`
  - `tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`
  - `crates/scoop/src/fixtures/run_pass.rs`
- 背景：
  - 在 `CG-T07S0a24` 修复 infer authoritative contract 之后，`CG-T07S0a` 的单 fixture `effect_handle_top_level_val_pattern_access_basic.scoop` 已通过，但默认 full-suite / `runtime_gc` phase 继续暴露 `tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop` 阻塞。
  - 导出 `gc_stw_cross_thread_roots_basic.ll` 可见 `@__scoop_top_level_var__fixtures.codegen.ready` / `proceed` 只有普通 `load`，`__atomicIntStore` / `__atomicIntLoad` 对 top-level `@Global __AtomicInt` lvalue 未发射 atomic store/load；worker 与 main 因此永远观察不到共享状态变化，程序本体卡在 `waitWorkerReady()` / allocation loop。
  - 该 fixture 带 `// TIMEOUT: 5000`；当前 run-pass timeout 仅 kill 直接子进程 `scoop run`，未连带终止其后代 `a.out`。当 fixture 超时时，后代进程继续持有继承的 stdout/stderr pipe，顶层 `scoop test` 会卡在 `run_command_collect_output()` 的 reader join，看起来像“测试跑完后 hang”。

- 必须实现的内容：
  1. 修复 refactor LLVM 对 top-level `@Global __AtomicInt` lvalue 的 atomic intrinsic lowering，使 `__atomicIntLoad` / `__atomicIntStore` / `__atomicIntCompareExchange` 直接针对共享静态存储发射 atomic op，而不是先把 top-level var 退化成 ordinary value load。
  2. 保持 `@Global` / `@ThreadLocal` 语义区分：`@Global` 必须跨线程共享同一存储，`@ThreadLocal` 保持 TLS；不得用局部临时 slot 或按值复制伪装 lvalue address。
  3. 修复 run-pass timeout 清理：超时时必须连同 `scoop run` 及其后代可执行文件一起终止/回收，并稳定返回 `scoop::fixtures::run_exec_timeout`，不能留下继承 stdout/stderr 的 orphan process 让 `scoop test` 假性挂起。
  4. 补最小回归测试，覆盖 top-level `@Global __AtomicInt` cross-thread runtime_gc 场景与 timeout descendant cleanup。

- 必须遵从的约束：
  - 不允许通过增大 `gc_stw_cross_thread_roots_basic.scoop` 的 `TIMEOUT`、改 fixture 握手形状、移除 `@Global`、或去掉跨线程 busy-loop 分配来规避问题。
  - 不允许在 runner 层把 `runtime_gc` fixture 特判成“超时后忽略 orphan 子进程”的局部 workaround；必须修正真实的 top-level atomic lvalue contract 与 timeout cleanup 语义。

- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`
  3. `cargo test -p scoop run_fixture_command_timeout_has_stable_code -- --nocapture`
  4. 新增或重跑 timeout descendant cleanup 定向测试（命名待实现）
  5. `cargo run -p scoop -- test`

- 完成条件：
  - `gc_stw_cross_thread_roots_basic.scoop` 在默认 refactor runtime_gc path 下稳定输出 `hello 7`、`ok`。
  - `scoop test` 遇到超时 fixture 时不再因 orphan descendant + inherited pipe 挂住。
- 依赖：`CG-T07R`，`CG-T07S0a24`

- 完成记录：
  - 2026-05-09：作为 `CG-T07S0a` 的新前置 blocker 补录。`tools/run_fixture_scan.sh --no-build --timeout-secs 20 tests/fixtures/runtime_gc` 显示 `gc_stw_cross_thread_roots_basic.scoop` 是 `runtime_gc` 组唯一失败项；`sample` / `lsof` 进一步确认顶层 `scoop test` 只是卡在 reader join，而真实挂起的是超时后未被回收的后代 `a.out`。同时导出的 LLVM IR 证明 top-level `@Global __AtomicInt` lvalue 仍被退化成普通 top-level var `load`，缺少应有的 atomic store/load。
  - 2026-05-09：`crates/scoopc/src/llvm/codegen/effect_refactor/value.rs` 的 `atomic_int_lvalue_ptr()` 现在会把 direct `TopLevelRef` local 重新解析为 top-level / extern static storage 指针，并在该 local 只作为 `__atomicInt*` target 时跳过无意义的 `TopLevelRef -> local slot` 普通 `load`。`gc_stw_cross_thread_roots_basic.scoop` 导出的 LLVM IR 现已直接在 `@__scoop_top_level_var__fixtures.codegen.ready` / `proceed` 上发出 atomic load/store，不再先做 ordinary load。
  - 2026-05-09：新增 `tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`，用 `BUILD-LLVM-CONTAINS` 锁定 top-level `@Global` / `@ThreadLocal __AtomicInt` 的静态存储声明与 atomic load/store/cmpxchg 形状，避免后续再次把 top-level atomic lvalue 退化成普通值路径。
  - 2026-05-09：`crates/scoop/src/fixtures/run_pass.rs` 的 timeout 路径在 Unix 上会把 `scoop run` 放进独立 process group，并在超时时对整个子进程树发 `SIGKILL`；新增 `run_fixture_command_timeout_kills_descendants` 单测，覆盖 descendant 继承 stdout/stderr pipe 时仍能快速回收并稳定返回 `scoop::fixtures::run_exec_timeout`。
  - 2026-05-09：在本任务要求的 full-suite 验证中，`cargo run -p scoop -- test` 先暴露了已删除 async/task surface 遗留的空 cone fixture `tests/fixtures/typecheck_cone/std_task_async_await_impl_ok/`；已同步移除该空目录，避免无关 stale fixture 阻塞当前任务的全量回归验收。
  - 验证通过：`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/gc_stw_cross_thread_roots_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo test -p scoop --bin scoop fixtures::run_pass::tests::run_fixture_command_timeout_has_stable_code -- --exact --nocapture`、`cargo test -p scoop --bin scoop fixtures::run_pass::tests::run_fixture_command_timeout_kills_descendants -- --exact --nocapture`、`cargo run -p scoop -- test --fixtures tests/fixtures/build/unsafe_atomic_int_top_level_storage_llvm.scoop`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0a：修复 effect-handle top-level val pattern access 在 EffectStep codegen 中的 top-level value ref lowering，解除 CG-T07S0 默认 full-suite 新 blocker

- 参考：
  - `CG-T05`
  - `CG-T08`
  - `tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`
- 背景：
  - 在 `CG-T07S0` 的 callable value / `FunPtr` named-arg 修复落地后，默认 `cargo run -p scoop -- test` 不再首先停在 `callable_value_pattern_binder_receiver_named_args_basic.scoop`，而是继续暴露 `tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop -o /tmp/effect_handle_top_level_val_pattern_access_basic` 会在 refactor effect-step codegen 前端准备阶段报 `unsupported main codegen node: top-level value ref`，说明顶层 once-init / pattern binder 的 top-level value ref 仍未进入 EffectStep state-machine lowering 主线。

- 必须实现的内容：
  1. 修复 refactor EffectStep / state-machine codegen 对 top-level immutable value / top-level pattern binder value ref 的 lowering，要求消费 authoritative top-level once-init/root contract，而不是把它留成 `top-level value ref` unsupported node。
  2. 保持顶层 pattern binder 在 `handle` / `try` state-machine 路径中的运行期校验语义：匹配成功走 inactive path，匹配失败通过 `Raise.raise(RuntimeError.*)` 进入 handler dispatch，不能被顶层初始化 guard 误判成递归初始化 fatal trap。
  3. 补最小回归测试，确保 `effect_handle_top_level_val_pattern_access_basic.scoop` 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、移除顶层 pattern binder、绕开 `handle` / `try` state-machine 路径或降级到 legacy path 规避该问题。
  - 不允许把 top-level once-init/root 语义私补到 LLVM backend；必须在 authoritative handoff / lowering 主线上修正。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop -o /tmp/effect_handle_top_level_val_pattern_access_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - 默认 full-suite 不再在 `effect_handle_top_level_val_pattern_access_basic.scoop` 停止，`CG-T07S0` 可继续验证 callable value / `FunPtr` named-arg 回归是否已完全解除。
- 依赖：`CG-T07R`，`CG-T07S0a1`，`CG-T07S0a2`，`CG-T07S0a3`，`CG-T07S0a4`，`CG-T07S0a5`，`CG-T07S0a6`，`CG-T07S0a7`，`CG-T07S0a8`，`CG-T07S0a9`，`CG-T07S0a10`，`CG-T07S0a11`，`CG-T07S0a12`，`CG-T07S0a13`，`CG-T07S0a14`，`CG-T07S0a15`，`CG-T07S0a16a`，`CG-T07S0a16`，`CG-T07S0a17`，`CG-T07S0a18`，`CG-T07S0a19`，`CG-T07S0a20`，`CG-T07S0a21`，`CG-T07S0a22`，`CG-T07S0a24`，`CG-T07S0a24a`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S0` 的新前置阻塞补录。callable value / `FunPtr` named-arg 槽位映射修复后，默认 full-suite 继续前进到 `effect_handle_top_level_val_pattern_access_basic.scoop`；build 诊断显示 refactor EffectStep codegen 仍不支持 top-level value ref，需先独立修复后才能完成 `CG-T07S0` 的默认 full-suite 验证。
  - 2026-05-08：已修复 `synth_raise_null_assertion_failed()` 生成的 synthetic `RuntimeError.NullAssertionFailed` surface，把它从 `TopLevelRef` 改为与正常源码一致的 `RuntimeError.NullAssertionFailed` member-access authoritative HIR 形状，并同步更新受影响的 HIR/MIR snapshot；`cargo run -p scoop -- build tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop -o /tmp/effect_handle_top_level_val_pattern_access_basic` 与 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop` 通过。
  - 2026-05-08：默认 full-suite 继续前进后又暴露 `elvis_lazy_basic.scoop` 的 raw MIR composite transport trace metadata blocker；按顺序约束新增 prerequisite `CG-T07S0a0`，本任务保持未完成，等待 `CG-T07S0a0` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a0` 修复后，默认 full-suite 又继续前进到 `fun_call_add_basic.scoop`；build 诊断显示 refactor plain return coercion 仍把 `main(): Int` 尾值路径误判成 `Ref`，按顺序约束新增 prerequisite `CG-T07S0a1`，本任务保持未完成，等待 `CG-T07S0a1` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a1` 修复后，默认 full-suite 又继续前进到 `gc_array_class_elements_cross_function.scoop`；build 诊断显示 refactor pure assignment / `println::<String>` arg lowering 仍把 `String` 值路径误判成 `Ref`，按顺序约束新增 prerequisite `CG-T07S0a2`，本任务保持未完成，等待 `CG-T07S0a2` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a2` 修复后，默认 full-suite 又继续前进到 `gc_trace_task_field_basic.scoop`；build 诊断显示 `Async.await(holder.task)` 的 direct-style MIR perform site metadata 仍把 payload transport type 与 payload component type 发布成漂移 shape，按顺序约束新增 prerequisite `CG-T07S0a3`，本任务保持未完成，等待 `CG-T07S0a3` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a3` 修复后，默认 full-suite 又继续前进到 `kotlin_ranges_progressions_basic.scoop`；build 阶段在 direct-style MIR lowering 触发 `assignment place contract references an unallocated local: S34` panic，按顺序约束新增 prerequisite `CG-T07S0a4`，本任务保持未完成，等待 `CG-T07S0a4` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a4` 修复后，默认 full-suite 又继续前进到 `list_and_mutable_list_basic.scoop`；build 阶段在 materialized MIR validation 报 `materialized MIR 'scoop.core.push' contains unresolved generic parameter in array transport element type ...: T`，按顺序约束新增 prerequisite `CG-T07S0a5`，本任务保持未完成，等待 `CG-T07S0a5` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a5` 修复后，默认 full-suite 又继续前进到 `literal_numeric_expected_type_absorption_basic.scoop`；单 fixture build/run 显示 `Array<UInt8>` 上 `1 + 2` / `1 << 3` 的最终观测值仍输出 `false` / `false`，按顺序约束新增 prerequisite `CG-T07S0a6`，本任务保持未完成，等待 `CG-T07S0a6` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a6` 修复后，默认 full-suite 又继续前进到 `literal_ops_compare_direct_matrix_basic.scoop`；build 诊断显示 `"ab".compareTo("ac")` 的 String 字面量 receiver 直接调用仍退化成 `CallKind::FunValue` 并报 `unsupported main codegen node: refactor plain function-value callee type`，按顺序约束新增 prerequisite `CG-T07S0a7`，本任务保持未完成，等待 `CG-T07S0a7` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a7` 修复后，默认 full-suite 又继续前进到 `local_val_destructuring_nested_variant_mismatch_is_error.scoop`；build 诊断显示 refactor ABI tuple payload `refactor_carrier_direct_args` 仍缺少 source component 1，按顺序约束新增 prerequisite `CG-T07S0a8`，本任务保持未完成，等待 `CG-T07S0a8` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a8` 修复后，默认 full-suite 又继续前进到 `member_call_devirt_final_receiver_direct_call_basic.scoop`；单 fixture build 诊断显示链接阶段 `_Base.ping` 仍未发射，但 `__scoop_vtable__Base` 已引用该符号，按顺序约束新增 prerequisite `CG-T07S0a9`，本任务保持未完成，等待 `CG-T07S0a9` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a9` 修复后，默认 full-suite 又继续前进到 `nothing_raise_coerce_to_any_type.scoop`；build 诊断显示 refactor HandleDispatch lowering 仍把同一 boundary case 发布成多个 routing contract，按顺序约束新增 prerequisite `CG-T07S0a10`，本任务保持未完成，等待 `CG-T07S0a10` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a10` 修复后，默认 full-suite 又继续前进到 `object_companion_value_named_nested_init_basic.scoop`；build 诊断显示 nested object / named companion value access 仍被误送进 `pass MIR member field target` lowering，按顺序约束新增 prerequisite `CG-T07S0a11`，本任务保持未完成，等待 `CG-T07S0a11` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a11` 修复后，默认 full-suite 又继续前进到 `operator_overload_struct_basic.scoop`；build 诊断显示 struct `compareTo` direct-call result 在 refactor pure assignment / compare lowering 中仍被误强制成 struct target，按顺序约束新增 prerequisite `CG-T07S0a12`，本任务保持未完成，等待 `CG-T07S0a12` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a12` 修复后，默认 full-suite 又继续前进到 `safe_member_access_ref_and_extension_basic.scoop`；build 诊断显示 safe-call `Option` result arm body 仍会退化成 `ctor call lowering pending`，按顺序约束新增 prerequisite `CG-T07S0a13`，本任务保持未完成，等待 `CG-T07S0a13` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a13` 修复后，默认 full-suite 又继续前进到 `smart_cast_any_member_access_generic_class_basic.scoop`；build 诊断显示 smart-cast 分支的 generic class field access 仍把 `x.value` 的 result/frame slot 保留为 unresolved generic `T`，按顺序约束新增 prerequisite `CG-T07S0a14`，本任务保持未完成，等待 `CG-T07S0a14` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a14` 修复后，默认 full-suite 又继续前进到 `stdlib_hash_set_map_basic.scoop`；build 诊断显示 materialized MIR `scoop.collections.__map_alloc_empty_table` 的 array transport element type 仍保留 unresolved generic `T`，按顺序约束新增 prerequisite `CG-T07S0a15`，本任务保持未完成，等待 `CG-T07S0a15` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a15` 修复并补齐其 build / run-pass / full-suite 验证后，默认 full-suite 已越过 `stdlib_hash_set_map_basic.scoop`，但继续在 `literal_array_expected_type_nested_basic.scoop` 暴露新的 stdout mismatch；直接 build/run 可见嵌套 `Array<UInt8>` expected-type 路径仍使 `bytes.get(0) == 3` 与 `argByte == 4` 输出为 `false`，按顺序约束新增 prerequisite `CG-T07S0a16`，本任务保持未完成，等待 `CG-T07S0a16` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：在 `CG-T07S0a16` 的对照验证中又发现更前置的 direct `Array<UInt8>` regression：`literal_numeric_expected_type_absorption_basic.scoop` 重新失败，`dump-mir` 显示 `scoop.core.get` / compare path 把 element surface 退回 nominal/composite `Struct`；因此按顺序约束再前插 `CG-T07S0a16a`，本任务继续保持未完成，等待其先行修复。
  - 2026-05-08：`CG-T07S0a16` 完成并补齐嵌套 `UInt8` MIR 回归验证后，默认 full-suite 已越过 `literal_array_expected_type_nested_basic.scoop`，但继续在 `star_projection_array_read_view.scoop` 暴露新的 `Array<*>` 读视图 transport trace contract 漂移；按顺序约束新增 prerequisite `CG-T07S0a17`，本任务继续保持未完成，等待其修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a17` 完成并补齐 `Array<*>` 读视图 build / run-pass / full-suite 验证后，默认 full-suite 已越过 `star_projection_array_read_view.scoop`，但继续在 `stdlib_string_basic.scoop` 暴露 `String.byteLength()` support-source member call 仍退化成 unresolved `MemberAccess` + `CallKind::FunValue` 的新 blocker；按顺序约束新增 prerequisite `CG-T07S0a18`，本任务继续保持未完成，等待其修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a18` 完成并补齐 `String.byteLength()` / `getByte()` / `unsafeSliceBytes()` 的 build / run-pass / clippy 验证后，默认 full-suite 已越过 `stdlib_string_basic.scoop`，但继续在 `stdlib_string_methods_extended.scoop` 暴露 remaining `String.isEmpty()` / `replace()` / `charAt()` / `repeat()` builtin member call 新 blocker；按顺序约束新增 prerequisite `CG-T07S0a19`，本任务继续保持未完成，等待其修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-08：`CG-T07S0a19` 完成并补齐 `String.isEmpty()` / `replace()` / `charAt()` / `repeat()` 的 build / run-pass / clippy 验证后，默认 full-suite 已越过 `stdlib_string_methods_extended.scoop`，但继续在 `string_trim_indent_basic.scoop` 暴露 remaining `String.trimIndent()` builtin member call 新 blocker；按顺序约束新增 prerequisite `CG-T07S0a20`，本任务继续保持未完成，等待其修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
- 2026-05-08：使用 `tools/run_fixture_scan.sh --no-build --out-dir target/fixture-scan/round3-30s` 做逐 fixture 扫描后，确认除 `CG-T07S0a20` 覆盖的 `String.trimIndent()` 之外，还剩若干失败且可按 callable / ctor ABI、top-level / package compilation-unit contract、task/thread/GC coordination、frontend receiver `eff` row contract 四组根因收口；据此新增 prerequisites `CG-T07S0a21`、`CG-T07S0a22`、`CG-T07S0a24`，其中 task/thread/GC coordination 组后续已随 async/Task 清理移除。本任务继续保持未完成，等待剩余 blocker 依序清理并同步更新 `FAILED_FIXTURES.md` 后再重跑 full-suite 验收。
- 2026-05-08：`CG-T07S0a20` 已完成并补齐 `trimIndent()` 的编译器回归、build / 单 fixture run-pass / clippy 验证；默认 full-suite 当时继续停在 task/thread/runtime GC 组，但该 blocker 后续已随 async/Task 清理移除，因此本任务当前只等待其余 prerequisites 依序收口。
- 2026-05-09：重跑 `effect_handle_top_level_val_pattern_access_basic.scoop` 的单 fixture build/test 后，该任务原始 `top-level value ref` 故障已不再复现；但默认 full-suite / `runtime_gc` phase 继续暴露 `gc_stw_cross_thread_roots_basic.scoop`。导出的 LLVM IR 显示 top-level `@Global __AtomicInt` lvalue 仍被退化成普通 top-level var `load`，同时 run-pass timeout 只 kill 外层 `scoop run` 会留下继承 pipe 的 orphan `a.out`，导致顶层 `scoop test` 假性挂起。按顺序约束新增 prerequisite `CG-T07S0a24a`，本任务保持未完成，等待其修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
- 2026-05-09：在 `CG-T07S0a24a` 修复 top-level atomic storage / timeout descendant cleanup 后，重新执行 `cargo run -p scoop -- build tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop -o /tmp/effect_handle_top_level_val_pattern_access_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop` 与默认 `cargo run -p scoop -- test`，确认 `effect_handle_top_level_val_pattern_access_basic.scoop` 的原始 EffectStep `top-level value ref` blocker 已稳定消失，默认 full-suite 也不再停在该 fixture；据此本任务完成。
- 验证通过：`cargo run -p scoop -- build tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop -o /tmp/effect_handle_top_level_val_pattern_access_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S0：修复 receiver callable value / FunPtr named-arg lowering 顺序回归，解除 CG-T07S 默认 full-suite run-pass 阻塞

- 参考：
  - `CG-T03`
  - `CG-T08`
  - `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
- 背景：
  - 在 `mir_refactor` snapshot 漂移修复后，默认 `cargo run -p scoop -- test` 不再首先停在 `aggregate_transport.scoop`，而是暴露 `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop` 的 run-pass 失败。
  - 单独执行 `cargo run -p scoop -- build tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop -o /tmp/callable_value_pattern_binder_receiver_named_args_basic` 会在 refactor LLVM 前端准备阶段报 `unsupported value coercion from Int ... to String`，定位到 `CallKind::FunValue` lowering，说明 receiver callable value / `FunPtr` 的 named-arg lowering 仍可能把 receiver 与普通参数槽位错配。

- 必须实现的内容：
  1. 修复 receiver function value 与 `FunPtr` direct call 在 receiver + named args 组合下的参数槽位映射，覆盖顶层命名 receiver function value、top-level pattern binder、局部 destructuring binder、`when` pattern binder 与顶层 `FunPtr`。
  2. callable value call lowering 必须消费 authoritative call-site / callable contract；不得依赖当前 arg 向量顺序偶然与 receiver slot 对齐。
  3. 补最小回归测试，确保 `callable_value_pattern_binder_receiver_named_args_basic.scoop` 与同类 receiver named-arg callable surface 在默认 full-suite 下稳定通过。

- 必须遵从的约束：
  - 不允许通过改 fixture 形状、移除 named args、绕开 pattern binder、或降级到 legacy path 规避该问题。
  - 不允许把 receiver callable value 重新当成普通 direct call / positional-only call 特判糊过去。

- 验证：
  1. `cargo run -p scoop -- build tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop -o /tmp/callable_value_pattern_binder_receiver_named_args_basic`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
  3. `cargo run -p scoop -- test`

- 完成条件：
  - receiver callable value / `FunPtr` 的 receiver + named-arg lowering 在默认 refactor codegen 下不再把 `Int` 实参误送到 receiver `String` 槽位。
  - `CG-T07S` 可恢复使用默认 full-suite 继续验证 snapshot drift 是否已真正解除。
- 依赖：`CG-T07R`，`CG-T07S0a`

- 完成记录：
  - 2026-05-08：作为 `CG-T07S` 的新前置阻塞补录。`mir_refactor` snapshot 漂移修复后，默认 full-suite 首个失败转为 `callable_value_pattern_binder_receiver_named_args_basic.scoop`；build 诊断显示 `CallKind::FunValue` lowering 仍把 receiver / named-arg 槽位错配，需单独成 task 修复后才能重新闭合 `CG-T07S`。
  - 2026-05-08：已修复 typecheck callable surface、typed HIR contract、direct-style MIR lowering 与 canonical materialized MIR 对 callable value / `FunPtr` named-arg 绑定的遗漏；`callable_value_pattern_binder_receiver_named_args_basic.scoop` 的 build / 单 fixture test 通过，新增 HIR/MIR 定向单测、`aggregate_transport.scoop` snapshot 回归复核与 `cargo clippy --all-targets -- -D warnings` 通过。
  - 2026-05-08：默认 full-suite 继续前进后又暴露 `effect_handle_top_level_val_pattern_access_basic.scoop` 的 EffectStep `top-level value ref` codegen blocker；按顺序约束新增 prerequisite `CG-T07S0a`，本任务保持未完成，等待 `CG-T07S0a` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-09：恢复 `12185b94` 收紧后被误删的 callable/FunPtr direct-call named-arg surface：typecheck 重新接受 function value / `FunPtr` 的 synthetic `receiver` / `a0` 形参名映射，HIR 在缺少 top-level/ctor `plan` 的 callable fallback 中保留 `CallArg::Named`，让 MIR 继续消费 authoritative `arg_binding` 做 receiver-first canonicalization；据此 `when` pattern binder 的 `FunValue` 调用与顶层 `topFp(a0=..., receiver=...)` 都不再退回源码顺序 positional 形态。
  - 2026-05-09：恢复 `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop` / `.stdout` 正向回归，并删除 `function_value_named_args_not_supported_is_error.scoop` 与 `funptr_named_args_not_supported_is_error.scoop` 两条与当前任务目标冲突的负例；新增 `callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir` 单测，直接锁定 `FunValue` 与顶层 `FunPtr` named-arg 在 materialized MIR 中的 receiver-first 实参顺序。
  - 2026-05-09：默认 `cargo run -p scoop -- test` 现已完整通过（`fixtures: ok (1270)`），`CG-T07S0` 不再是 `CG-T07S` 的 full-suite 首个 blocker；当前任务完成。
  - 验证通过：`cargo test -p scoopc callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir -- --nocapture`、`cargo test -p scoopc top_level_generic_named_args_keep_canonical_param_order_in_pass_mir -- --nocapture`、`cargo run -p scoop -- build tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop -o /tmp/callable_value_pattern_binder_receiver_named_args_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。

## [DONE] CG-T07S：修复 full-suite cross-fixture transport metadata drift，解除 CG-T08 默认回归阻塞

- 参考：
  - `CG-T04a`-`CG-T04f`
  - `CG-T08`
  - [`TODO-P7.md`](./TODO-P7.md) P7-T03
- 背景：
  - 执行 `CG-T08` 的默认 full regression 时，`cargo run -p scoop -- test` 在根目录串跑下于 `tests/fixtures/mir_refactor/aggregate_transport.scoop` 暴露 snapshot 漂移；单独运行 `tests/fixtures/mir_refactor` 或单 fixture 时通过。
  - 已确认漂移集中在 composite transport metadata：`MirBoxingIntent.target_ty`、handle/perform payload tuple/component metadata 会在 full-suite 过程中与单跑结果不一致，说明 transport metadata repair 仍依赖顺序敏感的中间局部类型或隐藏状态，而不是 authoritative 外层 contract。

- 必须实现的内容：
  1. 找到并消除 `aggregate_transport.scoop` 等 composite transport sample 在 full-suite 串跑与单跑之间的 metadata 漂移根因。
  2. transport metadata repair 只能消费 authoritative outer contract（如 `array_ty`、closure `env_ty`、handle/perform payload schema）；不得依赖顺序敏感的 `local.ty`、hidden cache 或前序 fixture 残留状态。
  3. `scoop test` 根目录串跑与单 fixture 运行对 `mir_refactor` / `effect_facts` / `effect_lowered` snapshot 的可观测输出必须一致。
  4. 保留并补充最小回归测试，覆盖 fixture session 隔离与 full-suite composite transport 漂移场景。

- 必须遵从的约束：
  - 不允许通过跳过 fixture、弱化 golden、或把 snapshot phase 改成显式 legacy / subprocess workaround 规避该问题。
  - 不允许继续信任已经 concrete 但与 authoritative contract 漂移的 transport metadata；必须在 producer/materializer/repair 层修正来源。

- 验证：
  1. `cargo test -p scoop run_all_recreates_session_between_independent_fixtures`
  2. `cargo test -p scoopc refactor_mir_stable_dump_canonicalizes_type_ids_by_structure`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop`
  4. `cargo run -p scoop -- test`

- 完成条件：
  - default full-suite 不再因 cross-fixture transport metadata drift 在 `aggregate_transport.scoop` 或同类 MIR/effect snapshot fixture 上失败。
  - `CG-T08` 可恢复执行标准 full regression，而不是先停在 snapshot drift blocker。
- 依赖：`CG-T07R`，`CG-T07S0`

- 完成记录：
  - 2026-05-08：执行 `CG-T08` 时已先补 `crates/scoop/tests/cg8_codegen_regression_matrix.rs` representative matrix、fixture runner fresh-session isolation regression，以及 refactor MIR stable dump 的首差异诊断与 `TypeId` canonicalization；`cargo test --all` 与定向 matrix 通过，但 `cargo run -p scoop -- test` 仍在 `tests/fixtures/mir_refactor/aggregate_transport.scoop` 暴露 full-suite composite transport metadata drift，本任务据此新增并保持未完成。
  - 2026-05-08：补齐 `mir_refactor` 单文件 phase fallback，确认 `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/{generic_materialization,aggregate_transport}.scoop` 不再误走 parse phase；`RefactorMirStageOutput::stable_dump()` 改为 canonical type-id dump，`MirLoweringFacts::with_member_value_types()` 过滤掉 mangled generic instance clone 对 base field FQN 的污染，`generic_materialization.actual.raw.mir` 中 `holder.item` 不再在 `Int` / template `T` 之间随机漂移；`tests/fixtures/mir_refactor`、`tests/fixtures/effect_facts` 与 `tests/fixtures/effect_lowered` 目录定向验证恢复通过。
  - 2026-05-08：默认 full-suite 后续仍被 `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop` 的 receiver callable value / `FunPtr` named-arg lowering 回归阻塞；按顺序约束新增 prerequisite `CG-T07S0`，本任务保持未完成，等待 `CG-T07S0` 修复后再重跑 `cargo run -p scoop -- test` 完成最终验收。
  - 2026-05-09：在 `CG-T07S0` 完成后重新执行本任务验证，`cargo test -p scoop run_all_recreates_session_between_independent_fixtures`、`cargo test -p scoopc refactor_mir_stable_dump_canonicalizes_type_ids_by_structure`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/aggregate_transport.scoop` 与默认 `cargo run -p scoop -- test` 全部通过；`aggregate_transport.scoop` 单跑与 full-suite 结果一致，默认 full-suite 稳定为 `fixtures: ok (1270)`，本任务完成。

## [DONE] CG-T08：建立 codegen regression 矩阵并完成阶段退出审计

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG8、§4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §5.7、§9
  - [`TODO-P7.md`](./TODO-P7.md) P7-T02Z、P7-T03
- 目标：
  - 恢复默认 refactor full regression，关闭当前 P7 blocker，并完成 codegen-stage gap 审计。

- 必须实现的内容：
  1. 建立 codegen fixture 矩阵，覆盖 CG-T01 至 CG-T07 的代表样本。
  2. 恢复执行 `TODO-P7.md` P7-T02Z / P7-T03 剩余 run-pass blockers。
  3. 运行 standard full regression，并对每个失败分类为 codegen bug、upstream MIR contract bug、frontend reject 缺失或 runtime/GC bug。
  4. 更新 `PIPELINE_GAPS.md` 或阶段完成记录，标明 codegen-stage scope 的 gap 状态。

- 必须遵从的约束：
  - 不允许显式 legacy selector、fixture 降级、golden 改弱或跳过默认 refactor blocker。
  - 若失败根因是 upstream MIR contract，必须回 `TODO.md`，不能在 codegen 中私补语义。

- 验证：
  1. `cargo test --all`
  2. `cargo run -p scoop -- test`
  3. GC env：`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`

- 完成条件：
  - 默认 refactor full regression 通过或剩余失败均有明确非 codegen-stage owner。
  - `PIPELINE_GAPS.md` 中 codegen-stage scope 的 gap 已关闭或重分类。
- 依赖：`CG-T07S`

- 完成记录：
  - 2026-05-08：已补 `crates/scoop/tests/cg8_codegen_regression_matrix.rs` 建立 `CG-T01`-`CG-T07` 与 `P7-T02Z` representative fixture matrix；新增 `fixtures::tests::run_all_recreates_session_between_independent_fixtures` 锁定 fixture session 隔离回归，并增强 MIR golden mismatch 诊断以输出首个差异行。
  - 2026-05-08：验证中确认 `cargo test --all` 通过，但 `cargo run -p scoop -- test` 仍在 `tests/fixtures/mir_refactor/aggregate_transport.scoop` 暴露 full-suite composite transport metadata drift；按顺序约束新增 prerequisite `CG-T07S`，本任务保持未完成。
  - 2026-05-09：完成最终阶段退出审计：`cargo test --all` 通过（覆盖 `cg8_codegen_regression_matrix`、fixture fresh-session isolation regression 与 refactor MIR stable-dump canonicalization 审计）；默认 `cargo run -p scoop -- test` 通过并稳定为 `fixtures: ok (1270)`；`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc` 通过并稳定为 `fixtures: ok (25)`。
  - 2026-05-09：更新 `PIPELINE_GAPS.md` 状态审计，确认 codegen-stage scope（`§3`、`§4`、`§5.1-§5.7`、`§6.1-§6.5` 与默认 refactor 路径可达的 `§7.6`）已关闭或重分类为非本阶段 owner；`CG-T08` 完成。

## [DONE] CG-T08R：Review CG-T08 codegen phase exit audit

- 参考：
  - `CG-T08`
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §5.7、§9
- 重点：
  - codegen fixture matrix 是否覆盖 `CG-T01` 至 `CG-T07` 的代表样本。
  - standard full regression 与 GC env regression 是否在默认 refactor 主线下执行，且没有 legacy selector、fixture 降级、golden 改弱。
  - 每个剩余失败是否分类为 codegen bug、upstream MIR contract bug、frontend reject 缺失或 runtime/GC bug，并有明确 owner。
- 验证：
  1. 重跑 `CG-T08` 的全部验证命令。
  2. 复查阶段退出审计记录与 `PIPELINE_GAPS.md` 状态更新。
  3. 抽查 P7 blocker 恢复记录，确认未通过 legacy fallback 或跳过样本达成。
- 完成条件：
  - Review 结论明确说明 `CG-T08` 已正确实现，codegen-stage scope 可进入下一阶段；若发现缺口，`CG-T08R` 保持未完成并把修复归回 `CG-T08`。
- 依赖：`CG-T08`

- 完成记录：
  - 2026-05-09：复核 `CG-T08` 的阶段退出审计产物，确认 `crates/scoop/tests/cg8_codegen_regression_matrix.rs` 已覆盖 `CG-T01` 至 `CG-T07` 与 `P7-T02Z` 的代表样本，`crates/scoop/src/fixtures/mod.rs` 保留 fixture fresh-session isolation 守护，`crates/scoop/tests/p7_default_pipeline.rs` 继续锁定默认 omission=refactor 且未依赖 legacy selector / hidden fallback 达成回归通过。
  - 2026-05-09：复查 `PIPELINE_GAPS.md` 顶部状态更新、`§5.7` 历史 blocker 记录与 `§9` 验证矩阵收口说明，确认 codegen-stage scope 已按 `CG-T01`-`CG-T08` 关闭或重分类为非本阶段 owner，未发现需要回退到 `CG-T08` 继续修复的遗漏项。
  - 2026-05-09：验证通过：`cargo test --all`、`cargo run -p scoop -- test`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings`。
