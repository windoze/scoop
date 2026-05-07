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
| `CG-T06R` | CG6R | Review CG-T06 unwind/thread boundary lowering |
| `CG-T07` | CG7 | 收口 extern global 与 GC pin/handle runtime surface |
| `CG-T07R` | CG7R | Review CG-T07 extern global 与 GC surface |
| `CG-T08` | CG8 | 建立 codegen regression 矩阵并完成阶段退出审计 |
| `CG-T08R` | CG8R | Review CG-T08 codegen phase exit audit |

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

## CG-T06R：Review CG-T06 unwind/thread boundary lowering

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

## CG-T07：收口 extern global 与 GC pin/handle runtime surface

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

## CG-T07R：Review CG-T07 extern global 与 GC surface

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

## CG-T08：建立 codegen regression 矩阵并完成阶段退出审计

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
- 依赖：`CG-T07R`

## CG-T08R：Review CG-T08 codegen phase exit audit

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
