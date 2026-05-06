# Refactor HIR Completeness Handoff

This document freezes the completion contract for the effect-refactor typed HIR stage. It only applies to `--effect-pipeline refactor`; legacy HIR/MIR/codegen paths may still contain dump-only `Todo(...)` nodes.

## Final Invariants

1. Refactor typed HIR production output is a no-placeholder handoff: no `hir::Item::Todo`, `hir::StmtKind::Todo`, `hir::ExprKind::Todo`, `hir::ExprKind::Missing`, missing assignment place contract, or equivalent HIR fallback sentinel may reach a successful stage result.
2. `RefactorHirCompletenessVerifier` runs by default for refactor typed HIR stage construction and scans `hir::File`, lowered member functions, top-level init roots, object init roots, class init roots, and assignment place coverage.
3. Pure parser-known invalid or deferred surfaces are rejected before HIR, including user-facing `spawn`, user-facing `join`, assignment expressions, and named/spread arguments outside call argument lists.
4. Failures that require type, resolver, or comptime information are reported by typecheck, comptime expansion, or refactor HIR stage diagnostics with source spans instead of producing HIR placeholders.
5. Runtime HIR bodies no longer carry comptime control-flow placeholders; `comptime block`, statement/item `comptime if`, and `comptime for` are expanded, selected, or diagnosed before runtime lowering.
6. Splice field access `value.[field]` lowers through typed splice field contracts to ordinary member/place access, and non-static or unknown fields are diagnosed before HIR handoff.
7. Type aliases, nominal declarations, objects, and extension properties are represented in the HIR declaration graph instead of `Item::Todo` nodes.
8. Array literals, named/default/spread arguments, constructors, member/extension calls, closures, function values, `FunPtr`, virtual/interface dispatch, effect operations, continuation resume, and reflection/platform intrinsics publish typed canonical call-site contracts.
9. Runtime class literals and reflection/platform intrinsics have explicit HIR contracts or earlier diagnostics; LLVM/backend support is not used as a reason to leave HIR placeholders.
10. Copy-update and assignment statements publish typed aggregate/place contracts. Unsupported aggregate or place shapes fail before successful refactor HIR output.
11. Top-level init/storage handoff covers const values, runtime immutable values, runtime mutable globals, object init roots, dependency facts, and extern global contracts.
12. `refactor_hir_preflight` proves the HIR completeness fixture set reaches typed HIR with required side tables and runs representative direct-style MIR smoke without HIR-origin fallback reasons.

## Validation Matrix

| Area | Guard / fixture coverage | Required validation |
| --- | --- | --- |
| Placeholder inventory | `crates/scoopc/src/hir/lower/placeholder_inventory.rs` freezes remaining HIR placeholder constructors and owner classifications. | `cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory` |
| No-Todo verifier | `crates/scoopc/src/effect_refactor_pipeline/hir_completeness.rs` rejects HIR placeholders and missing assignment place contracts across all reachable refactor HIR bodies. | `cargo test -p scoopc --no-default-features refactor_hir_no_todo` |
| Parser surface gate | Parse fixtures for structured concurrency, assignment expression, and call-only named/spread syntax. | `cargo test -p scoopc --no-default-features parser_hir_surface_gate` |
| Comptime expansion | `tests/fixtures/hir/refactor_comptime_control_flow.scoop` and comptime unit tests cover body and package-level expansion. | `cargo test -p scoopc --no-default-features refactor_hir_comptime` |
| Declaration graph | `tests/fixtures/hir/refactor_decl_graph.scoop` covers typealias, nominal declarations, object, and extension property contracts. | `cargo test -p scoopc --no-default-features refactor_hir_decls` |
| Splice field | String literal, `FieldMeta`, reflection loop, non-static field, and unknown field fixtures cover typed splice contracts and diagnostics. | `cargo test -p scoopc --no-default-features refactor_hir_splice_field` |
| Call arguments and provenance | `tests/fixtures/hir/refactor_call_args.scoop` and `tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop` cover canonical args and call-site contracts. | `cargo test -p scoopc --no-default-features refactor_hir_call_args` and `cargo test -p scoopc --no-default-features refactor_hir_call_contracts` |
| Class literal and intrinsics | Runtime class literal plus `nameOf<T>()`, `sizeOf<T>()`, and `getPlatform()` fixtures cover explicit intrinsic contracts. | `cargo test -p scoopc --no-default-features refactor_hir_class_literal` |
| Copy-update | Struct, tuple nested path, enum variant payload, and unsupported aggregate fixtures cover `WithUpdateContract`. | `cargo test -p scoopc --no-default-features refactor_hir_with_update` |
| Assignment places | Local, top-level storage, member field, unsupported LHS diagnostics, and synthetic assignment coverage. | `cargo test -p scoopc --no-default-features refactor_hir_places` |
| Custom iterator and recovery sentinels | Custom iterator for-loop unit tests and parser recovery negative fixture cover remaining debug fallbacks. | `cargo test -p scoopc --no-default-features refactor_hir_for_loop` |
| Top-level init/storage | `tests/fixtures/hir/refactor_top_level_init.scoop` covers const/runtime values, mutable globals, object init, and extern globals. | `cargo test -p scoopc --no-default-features refactor_hir_top_level_init` |
| HIR to next-stage preflight | `crates/scoopc/src/effect_refactor_pipeline/hir_preflight.rs` enumerates the HIR completeness fixture set and representative MIR smoke subset. | `cargo test -p scoopc --no-default-features refactor_hir_preflight` |
| CLI HIR dump gate | `dump-hir --effect-pipeline refactor` goes through the verified typed HIR stage; legacy dump remains separate. | `cargo test -p scoop --no-default-features dump_hir` |

## HIR Completeness Fixture Set

The authoritative fixture set for the final preflight is `HIR_COMPLETENESS_FIXTURES` in `crates/scoopc/src/effect_refactor_pipeline/hir_preflight.rs`.

| Fixture | Required contract check | MIR smoke |
| --- | --- | --- |
| `tests/fixtures/hir/refactor_comptime_control_flow.scoop` | No-Todo typed HIR coverage | HIR-only |
| `tests/fixtures/hir/refactor_decl_graph.scoop` | Declaration graph | HIR-only |
| `tests/fixtures/comptime/splice_field_access_v0_basic.scoop` | Declaration graph for local type splice coverage | HIR-only |
| `tests/fixtures/hir/refactor_call_args.scoop` | Call-site contract | Run direct MIR smoke |
| `tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop` | Call-site, continuation resume, perform, handle contracts | Run direct MIR smoke |
| `tests/fixtures/typecheck/refactor_hir_class_literal_runtime_ok.scoop` | Declaration graph with runtime class literal contract | HIR-only; direct MIR support is later-stage work |
| `tests/fixtures/typecheck/reflection_runtime_fallback_v0.scoop` | Call-site intrinsic contract | HIR-only |
| `tests/fixtures/typecheck/get_platform_runtime_ok.scoop` | Platform intrinsic call-site contract | HIR-only |
| `tests/fixtures/typecheck/with_update_struct_field_ok.scoop` | Copy-update contract | Run direct MIR smoke |
| `tests/fixtures/typecheck/with_update_tuple_nested_path_ok.scoop` | Copy-update contract | HIR-only |
| `tests/fixtures/typecheck/with_update_enum_variant_payload_ok.scoop` | Copy-update contract | HIR-only |
| `tests/fixtures/typecheck/refactor_hir_assignment_places_ok.scoop` | Assignment place contract | Run direct MIR smoke |
| `tests/fixtures/typecheck/for_loop_iter_protocol_ok.scoop` | Call-site contract for custom iterator lowering | HIR-only |
| `tests/fixtures/hir/refactor_top_level_init.scoop` | Top-level init root and extern global contracts | HIR-only |

HIR-only entries are intentionally not narrowed to make later stages pass. Each entry documents that its HIR handoff is complete while direct MIR, late lowering, runtime, or LLVM support remains outside this stage.

## Todo Scan Classification

The required final scan is:

```bash
rg "Todo\(" crates/scoopc/src/hir crates/scoopc/src/effect_refactor_pipeline
```

Expected classifications for remaining matches:

- `crates/scoopc/src/hir/mod.rs`: enum variants and legacy dump-only documentation.
- `crates/scoopc/src/hir/lower/block.rs`, `crates/scoopc/src/hir/lower/expr.rs`, and `crates/scoopc/src/hir/lower/stmt.rs`: legacy HIR lowerer placeholder traversal or constructors tracked by `refactor_hir_placeholder_inventory`; the refactor production stage enters typed HIR lowering and is guarded by the verifier.
- `crates/scoopc/src/hir/lower/mod.rs` and `crates/scoopc/src/hir/lower/util.rs`: legacy/debug traversal, cleanup, or test helpers that tolerate placeholder nodes when operating on legacy `LoweredHir` values.
- `crates/scoopc/src/hir/lower/placeholder_inventory.rs`: executable inventory and scan patterns, not production output.
- `crates/scoopc/src/effect_refactor_pipeline/hir_completeness.rs`: verifier rejection paths for `Item::Todo`, `StmtKind::Todo`, `ExprKind::Todo`, and `ExprKind::Missing`.
- `crates/scoopc/src/effect_refactor_pipeline/hir_preflight.rs`: MIR fallback scan denylist and representative MIR smoke assertions.
- `crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs`: stable dump visitors that tolerate legacy-shaped values and unit tests that inject placeholders to assert verifier failures; successful refactor stage construction still runs the verifier.
- `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`: downstream traversal cases that tolerate impossible-after-verifier HIR placeholders; this is not an HIR production source.

Any new match outside these classifications must update the inventory, verifier, preflight, or TODO ordering before it is accepted.

## Later-Stage Gaps

The following known gaps do not block HIR completeness unless their root cause becomes missing HIR metadata or a HIR-origin fallback reason:

- Raw MIR or LLVM lowering for `Handle`, `ResumeUnwind`, `TypeCheck`, `Cast`, dynamic call kinds, and some reflection/class literal runtime values.
- Aggregate boxing, array composite element lowering, closure environment layout, Step ABI lowering, runtime helper coverage, and LLVM backend support.
- Full `cargo run -p scoop -- test` fixture matrix, P7/P8 regression goals, and legacy path removal.

The stage is complete when the targeted parser/typecheck/HIR/preflight validations pass, the final `Todo(` scan is classified as above, and `TODO.md` marks `HIR-T14` as done.
