# MIR Refactor Phase Exit Audit

Date: 2026-05-07

Task: `MIR-T14`

Scope: this audit closes the MIR-facing portion of `PIPELINE_GAPS.md` for the refactor pipeline. It does not claim LLVM/runtime completion; remaining backend work is owned by `TODO-pipeline-gaps-codegen.md`, `TODO-P7.md`, or future frontend tasks as listed below.

## Verification Matrix

The phase exit matrix is intentionally MIR-only and uses targeted commands rather than full fixtures.

| Surface | Command |
| --- | --- |
| HIR completeness to strict MIR smoke | `cargo test -p scoopc --no-default-features refactor_hir_preflight` |
| Direct-style strict MIR no-placeholder verifier | `cargo test -p scoopc --no-default-features refactor_mir_no_todo` |
| Materialized MIR no-placeholder/no-param verifier | `cargo test -p scoopc --no-default-features refactor_materialized_mir` |
| MIR golden matrix | `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor` |
| MIR snapshot formatter shared by CLI/Rust tests | `cargo test -p scoop --no-default-features dump_mir` |
| MIR-stage lint | `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings` |

## Fixture Matrix

Every `tests/fixtures/mir_refactor/*.scoop` fixture now has a `.mir` golden generated through `RefactorMirStageOutput::stable_dump()`. The same stable dump surface is consumed by `scoop dump-mir`, the fixture runner, and Rust tests.

| Owner task | Representative MIR fixtures |
| --- | --- |
| `MIR-T03` | `refactor_hir_preflight` fixtures now all run strict MIR smoke; unsupported parser/frontend surfaces are diagnostics fixtures. |
| `MIR-T04` | `comptime_splice_class_with_update.scoop`, `comptime/splice_field_access_v0_basic.scoop` |
| `MIR-T05` | `top_level_roots.scoop`, `hir/refactor_decl_graph.scoop`, `hir/refactor_top_level_init.scoop` |
| `MIR-T06` | `assignment_places.scoop` |
| `MIR-T07` | `call_contracts.scoop`, `direct_and_fun_value_call.scoop` |
| `MIR-T08` | `dispatch_and_resume_call.scoop`, `handle_perform.scoop`, `handle_finally_boundary.scoop`, `continuation_resume_unit_sugar.scoop`, `effect_boundary_inside_expr_context.scoop` |
| `MIR-T09` | `runtime_typecheck_cast.scoop`, `not_null_assert.scoop`, `pattern_is_type.scoop` |
| `MIR-T10` | `aggregate_transport.scoop` |
| `MIR-T11` | `generic_materialization.scoop` |
| `MIR-T12` | `codegen_routing_contracts.scoop` |
| `MIR-T13` | `handle_finally_boundary.scoop`, `codegen_routing_contracts.scoop`, diagnostics listed below |

## Diagnostics Matrix

Unsupported/deferred surfaces that must not enter HIR/MIR are locked by diagnostics fixtures rather than MIR placeholders.

| Surface | Fixture |
| --- | --- |
| structured concurrency `spawn` | `tests/fixtures/parse/spawn_deferred_is_error.scoop`, `tests/fixtures/typecheck/spawn_deferred_is_error.scoop` |
| structured concurrency `join` | `tests/fixtures/parse/join_deferred_is_error.scoop`, `tests/fixtures/typecheck/join_deferred_is_error.scoop` |
| parser recovery missing statement | `tests/fixtures/parse/parser_recovery_missing_stmt_is_error.scoop` |
| untrimmed package-level `comptime if` | `tests/fixtures/resolve/package_level_comptime_if_untrimmed_is_error.scoop` |
| unsupported assignment syntax | `tests/fixtures/parse/assignment_call_lhs_is_error.scoop` |
| mutable local without initializer | `tests/fixtures/typecheck/local_var_missing_initializer_is_error.scoop` |
| illegal `break` / `continue` | `tests/fixtures/typecheck/break_not_in_loop_is_error.scoop`, `tests/fixtures/typecheck/continue_not_in_loop_is_error.scoop` |
| unsupported function type runtime cast | `tests/fixtures/typecheck/fn_type_cast_closed_pure_asq_is_error.scoop`, `tests/fixtures/typecheck/fn_type_cast_effectful_asq_is_error.scoop`, `tests/fixtures/typecheck/fn_type_cast_effectful_as_is_error.scoop` |
| cross-thread outward propagation | `tests/fixtures/typecheck/cross_thread_resume_outward_effects_is_error.scoop` |
| unsupported GC handle value-type surface | `tests/fixtures/typecheck/gc_handle_new_value_type_is_error.scoop` |
| or-pattern binder | `tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop` |

## Gap Audit

### PIPELINE_GAPS.md Section 1

| Gap | MIR disposition | Owner evidence |
| --- | --- | --- |
| 1.1 comptime block/if/for Todo | Closed before MIR; legal samples strict-MIR smoke. | `MIR-T04`, `refactor_hir_preflight`, `comptime_splice_class_with_update.scoop` |
| 1.2 splice field Todo | Closed before MIR as concrete member access or frontend diagnostic. | `MIR-T04`, `splice_field_access_v0_basic.scoop` |
| 1.3 class literal fallback | Closed as `TypeMetadataLiteral` in MIR. | `MIR-T04`, `comptime_splice_class_with_update.scoop` |
| 1.4 top-level `val` item Todo | Closed with initializer roots and value roots. | `MIR-T05`, `top_level_roots.scoop` |
| 1.5 typealias/type/object/comptime-if item Todo | Closed with metadata roots or frontend diagnostic. | `MIR-T03`, `MIR-T05`, `refactor_decl_graph.scoop` |
| 1.6 assignment LHS Todo | Closed via typed place contract or frontend diagnostic. | `MIR-T06`, `assignment_places.scoop` |
| 1.7 call/ctor callee Todo | Closed via typed call-site contract. | `MIR-T07`, `call_contracts.scoop` |
| 1.8 dispatch callee Todo | Closed via structured dispatch metadata. | `MIR-T08`, `dispatch_and_resume_call.scoop` |
| 1.9 continuation resume canonical-shape Todo | Closed via typed resume contract. | `MIR-T08`, `dispatch_and_resume_call.scoop`, `continuation_resume_unit_sugar.scoop` |
| 1.10 perform missing-contract Todo | Closed by strict site metadata contract. | `MIR-T08`, `handle_perform.scoop` |
| 1.11 handle missing-contract Todo | Closed by strict site metadata contract. | `MIR-T08`, `handle_perform.scoop`, `handle_finally_boundary.scoop` |
| 1.12 with-update Todo | Closed via copy-update contract and concrete aggregate MIR. | `MIR-T04`, `comptime_splice_class_with_update.scoop` |

### PIPELINE_GAPS.md Section 2

| Gap | MIR disposition | Owner evidence |
| --- | --- | --- |
| 2.1 production verifier allowed Todo | Closed by strict production verifier. | `MIR-T01`, `refactor_mir_no_todo` |
| 2.2 materializer propagated Todo | Closed by materialized verifier and rewrite gate. | `MIR-T02`, `refactor_materialized_mir` |
| 2.3 raw codegen late Todo rejection | MIR boundary closed; backend must not receive production Todo. | `MIR-T01`, `MIR-T02`, `CG-T00` backend gate follow-up |
| 2.4 non-Unit `Return { value: None }` | Closed by strict verifier. | `MIR-T01`, `refactor_mir_no_todo` |
| 2.5 missing generic template/root | Closed for MIR root publication and source-site materializer diagnostics. | `MIR-T05`, `MIR-T11`, `generic_materialization.scoop` |
| 2.6 effect-row generic args | Closed in call-site contract and instance key. | `MIR-T07`, `MIR-T11`, `generic_materialization.scoop` |
| 2.7 unresolved `TypeKind::Param` | Closed in materialized snapshot verifier. | `MIR-T02`, `MIR-T11`, `refactor_materialized_mir` |
| 2.8 erased resume carrier exception | Closed as explicit materialized verifier exception only for marked resume surfaces. | `MIR-T02`, `MIR-T11` |

### PIPELINE_GAPS.md Sections 3-7

| Gap | MIR-facing status | Later-stage owner |
| --- | --- | --- |
| 3.1 `Handle` / `ResumeUnwind` / `Todo` raw route | MIR publishes route features and refuses raw-unsafe route. | `CG-T01`, `CG-T06` |
| 3.2 `Perform` cleanup/resume target raw lowering | MIR publishes perform/resume/cleanup metadata and route facts. | `CG-T01`, `CG-T06` |
| 3.3 `PerformResult` default value risk | MIR routing verifier requires resume payload/binding contract. | `CG-T01` |
| 3.4 `TypeCheck` / `Cast` raw lowering | MIR metadata complete; no placeholder/default value. | `CG-T02` |
| 3.5 effect-neutral cast/typecheck lowering | MIR metadata and verifier complete; LLVM lowering remains backend scope. | `CG-T02` |
| 3.6 `Virtual` / `Interface` / `Resume` raw call kind | MIR call/site metadata complete and route-gated. | `CG-T01`, `CG-T03` |
| 3.7 top-level function reference | MIR call contract normalizes function values with metadata. | `CG-T03` |
| 3.8 pattern `is Type` | MIR pattern runtime metadata complete. | `CG-T02` |
| 3.9 class ctor named/default args | MIR consumes selected ctor and complete ordered args. | `CG-T03` |
| 3.10 default args | MIR consumes canonical ordered call args. | `CG-T03` |
| 3.11 closure env/capture shape | MIR transport metadata complete. | `CG-T04` |
| 3.12 effect/function-value adapter limits | MIR call ABI handoff metadata complete. | `CG-T04`, `CG-T05` |
| 3.13 ambiguous continuation route | MIR verifier rejects ambiguous publication. | `CG-T04` if backend support is later expanded |
| 4.1 aggregate boxing | MIR publishes boxing/transport intent. | `CG-T04` |
| 4.2 enum Unit payload field | MIR publishes payload schema; layout remains backend scope. | `CG-T04` |
| 4.3 wide enum payload | MIR publishes payload schema; layout remains backend scope. | `CG-T04` |
| 4.4 nested enum/tuple/struct payload | MIR publishes nested payload schema. | `CG-T04` |
| 4.5 composite array element | MIR publishes array element transport metadata. | `CG-T04` |
| 5.1 ABI routing by actual outward effects | MIR/effect handoff publishes route facts and ABI guard. | `CG-T05`, `CG-T01` |
| 5.2 unsupported source classification | Handoff verifier fails fast. | `CG-T06` |
| 5.3 `ResumeUnwind` cleanup contract | MIR/effect-lowered handoff publishes pending completion/unwind contract or rejects. | `CG-T06` |
| 5.4 outward-empty callable Step ABI drift | MIR/effect facts guard `NoOutward` and empty outward cases as Plain ABI. | `CG-T05` |
| 5.5 cross-thread resume u64 payload | MIR transport exists for payloads; current helper boundary remains runtime/codegen scope. | `CG-T06` |
| 5.6 thread resume non-complete Step fatal | Type/effect checker rejects unsupported outward propagation before MIR. | `CG-T06` if support is added |
| 5.7 default refactor run-pass blockers | Not a MIR placeholder gap after this phase; remains end-to-end backend/runtime regression scope. | `TODO-P7.md` P7-T02Z/P7-T03, `CG-T08` |
| 6.1 `!!` non-null assertion | MIR expresses success extract and failure raise path. | `CG-T02`, P7 runtime regression |
| 6.2 runtime `is/as/as?` | MIR metadata complete. | `CG-T02` |
| 6.3 `nameOf` / `getPlatform` fallback | MIR intrinsic/call contract complete. | `CG-T03` |
| 6.4 `@Extern` global variable | MIR extern global root and assignment place contract complete. | `CG-T07` |
| 6.5 interface default method | MIR dispatch metadata route-gated; codegen coverage remains backend scope. | `CG-T03` |
| 7.1 or-pattern binder | Frontend diagnostic; does not enter HIR/MIR. | Future frontend/MIR support task before codegen |
| 7.2 function type runtime cast | Frontend/typecheck diagnostic; does not enter MIR. | `CG-T02` if surface is enabled |
| 7.3 use-site effect-row type arg | Materializer supports effect args; currently unsupported type-ref remains frontend policy. | Future frontend task, `MIR-T11` contract if enabled |
| 7.4 structured concurrency `spawn` / `join` | Parser/frontend diagnostic; does not enter HIR/MIR. | Future structured concurrency plan |
| 7.5 mutable value-type fields | Current frontend restriction prevents unsupported MIR place surface. | Future place/value-type task before codegen |
| 7.6 GC pin/handle intrinsic surface | MIR GC intrinsic metadata exists for supported surface; unsupported value-type handle is diagnostic. | `CG-T07` |

## Exit Conclusion

The refactor direct-style MIR and materialized MIR boundaries now have executable tests and fixture goldens proving no production `Todo(...)` or unresolved generic param is accepted by the MIR-only matrix. Remaining failures described by `PIPELINE_GAPS.md` are either frontend diagnostics that intentionally stop before MIR, or later-stage LLVM/runtime work with explicit codegen/runtime owners.
