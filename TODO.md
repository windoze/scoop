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
| `CG-T00R` | CG0R | Review CG-T00 codegen inventory 与 backend gate |
| `CG-T01` | CG1 | 收口 raw MIR effect/control route 与 unsupported call kind |
| `CG-T01R` | CG1R | Review CG-T01 raw MIR route gate |
| `CG-T02` | CG2 | 收口 runtime type/value primitive LLVM lowering |
| `CG-T02R` | CG2R | Review CG-T02 runtime value primitive lowering |
| `CG-T03` | CG3 | 收口 call/ctor/function-ref/intrinsic/default/interface lowering |
| `CG-T03R` | CG3R | Review CG-T03 call/ctor/intrinsic lowering |
| `CG-T04` | CG4 | 收口 aggregate/enum/array/closure/boxing transport lowering |
| `CG-T04R` | CG4R | Review CG-T04 composite transport lowering |
| `CG-T05` | CG5 | 收口 effect-typed adapter 与 NoOutward plain ABI |
| `CG-T05R` | CG5R | Review CG-T05 adapter 与 NoOutward ABI |
| `CG-T06` | CG6 | 收口 source classification、unwind、thread boundary lowering |
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

## CG-T00R：Review CG-T00 codegen inventory 与 backend gate

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

## CG-T01：收口 raw MIR effect/control route 与 unsupported call kind

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

## CG-T01R：Review CG-T01 raw MIR route gate

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

## CG-T02：收口 runtime type/value primitive LLVM lowering

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

## CG-T02R：Review CG-T02 runtime value primitive lowering

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

## CG-T03：收口 call/ctor/function-ref/intrinsic/default/interface lowering

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

## CG-T03R：Review CG-T03 call/ctor/intrinsic lowering

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

## CG-T04：收口 aggregate/enum/array/closure/boxing transport lowering

- 参考：
  - [`PLAN-pipeline-gaps-codegen.md`](./PLAN-pipeline-gaps-codegen.md) §2/CG4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11、§4.1、§4.2、§4.3、§4.4、§4.5、§5.5
  - [`TODO.md`](./TODO.md) MIR-T10
- 目标：
  - Composite source value transport 在 LLVM/runtime 中闭合，覆盖 closure env、boxing、enum payload、array element、cross-thread resume payload。

- 必须实现的内容：
  1. value-type boxing layout 支持 tuple/struct/enum/value type -> `Any` / `Ref` / erased carrier，包含 trace/copy/drop metadata。
  2. enum payload layout 支持 Unit field、大整数 payload、nested enum/tuple/struct payload，必要时自动 boxed。
  3. Array runtime descriptor 支持 element size、trace/copy、composite get/set/build。
  4. closure env 支持 arbitrary traceable source type；mutable capture 使用 capture box。
  5. cross-thread resume payload 支持 ref/composite transport，并正确 root GC refs。

- 必须遵从的约束：
  - 不允许继续用 `u64`/ref 双轨隐式代表所有 composite value。
  - 不允许 composite transport 绕过 GC trace/copy/drop requirements。

- 验证：
  1. `cargo test -p scoopc refactor_llvm_aggregate_transport`
  2. enum/array/closure composite run-pass fixtures。
  3. `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_refs.scoop`

- 完成条件：
  - `PIPELINE_GAPS.md` §3.11、§4.1-§4.5、§5.5 的 codegen/runtime 部分关闭。
- 依赖：`CG-T03R`，`MIR-T10R`

## CG-T04R：Review CG-T04 composite transport lowering

- 参考：
  - `CG-T04`
  - [`SCOOP_FULL_SPEC.md`](./SCOOP_FULL_SPEC.md) §2、§15
  - [`TODO.md`](./TODO.md) MIR-T10R
- 重点：
  - value boxing、enum payload、array element、closure env、cross-thread resume payload 是否共用 explicit transport/layout metadata。
  - GC trace/copy/drop、stack/root handling、boxed/inline choice 是否不依赖 `u64`/ref 隐式双轨。
  - runtime_gc moving/stress/verify-roots 样本是否覆盖 composite refs。
- 验证：
  1. 重跑 `CG-T04` 的全部验证命令。
  2. 抽查 enum/array/closure composite run-pass 与 runtime_gc fixtures。
  3. 检查 LLVM/runtime layout 中 composite payload 的 GC slot 可枚举性。
- 完成条件：
  - Review 结论明确说明 `CG-T04` 已正确实现；若发现缺口，`CG-T04R` 保持未完成并把修复归回 `CG-T04`。
- 依赖：`CG-T04`

## CG-T05：收口 effect-typed adapter 与 NoOutward plain ABI

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

## CG-T05R：Review CG-T05 adapter 与 NoOutward ABI

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

## CG-T06：收口 source classification、unwind、thread boundary lowering

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
