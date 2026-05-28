# TODO-3：P3 SPEC_FIX 语义落地

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P3  
> 包目标：落地 SPEC_FIX 的 type/effect/lowering/sysroot/cone 语义变化，为 overload call-site resolution 提供稳定前提。

## P3：SPEC_FIX type/effect/lowering 语义落地

### [DONE] P3-T01：operator-positioned calls 必须要求 `operator` modifier

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A3
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §11.3
- 目标：
  - `a + b`、`a[i]`、comparison 等 operator-positioned calls 只解析到带 `operator` modifier 的函数/方法。
  - 普通 named call `a.plus(b)` 不受影响。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/expr/ops.rs::{operator_overload_method_name,scalar_operator_method_name,infer_operator_overload_binary_expr_type,collect_member_method_signatures_from_index}`
  - `crates/scoopc_hir/src/typecheck/expr/call/member_call.rs::{pick_most_specific_member_overload,is_strictly_more_specific_member_overload}` if operator reuses member selection
  - `crates/scoopc_hir/src/resolve/mod.rs::ModifierSet`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `sysroot/lib/scoop.unsafe/src/unsafe.scoop`
  - fixtures containing operator overload methods such as `plus`, `minus`, `compareTo`, `get`, `set`
- 必须实现的内容：
  1. Filter operator-positioned candidates by `operator` flag before overload specificity.
  2. If same-named candidates exist but none has `operator`, emit diagnostic explaining modifier is required.
  3. Update sysroot methods intended for operator syntax with explicit `operator`.
  4. Add negative fixture: method named `plus` without `operator` is callable as `x.plus(y)` but not via `x + y`.
  5. Add positive fixture: `operator fun plus` works through operator syntax.
- 必须遵从的约束：
  - Do not make all methods named `plus` / `compareTo` operators by convention.
  - Do not bypass overload resolution; after modifier filtering, P5 rules still select most specific candidate.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted operator fixtures.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Accidental operator capture by same-named utility functions is impossible.
- 依赖：P2-T06R
- 完成记录：
  - 改动范围：在 `FunSigOwned` 中贯通 resolver `ModifierSet::operator`，并在 operator-positioned unary/binary/comparison candidate collection 后、specificity/匹配前筛掉未标记 `operator` 的候选；同步 sysroot 中 intended operator methods 和相关 fixtures / HIR goldens。
  - 核心决策：普通 named call 路径不读取 `is_operator`，因此 `x.plus(y)` 仍可调用普通同名方法；只有 operator-positioned path 在存在 same-name 候选但无 `operator` 时发出 `scoop::typecheck::operator_modifier_required`。
  - 验证结果：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；targeted operator fixtures；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。完整 fixture 首次发现 4 个 HIR golden span 漂移，更新 golden 后单独复跑 4 个失败项通过，最终完整 fixture suite 通过（`fixtures: ok (1552)`）。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `SPEC_FIX.md` A3 与 `PLAN.md` P3 operator modifier gate：operator-positioned calls 只考虑 `operator` 候选，named calls 不受影响，并区分“候选存在但缺少 operator”和“没有候选”。

### [DONE] P3-T01R：Review operator gate 语义

- 参考：
  - P3-T01 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A3
- 目标：
  - 复核 operator-positioned call 只接受 `operator` 候选，且普通 named call 不受影响。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/expr/ops.rs`
  - `crates/scoopc_hir/src/resolve/mod.rs`
  - sysroot operator declarations
  - operator positive/negative fixtures
- 必须实现的内容：
  1. 确认 operator syntax 对未标记 `operator` 的 same-name method 报清晰错误。
  2. 确认 `x.plus(y)` 仍可调用未标记普通 method。
  3. 确认 sysroot 中 intended operator methods 已显式标注。
  4. 确认没有绕过 P5 overload selection 的 ad-hoc path。
- 必须遵从的约束：
  - 不得用方法名约定替代 modifier gate。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted operator fixtures。
- 完成条件：
  - A3 语义完整闭合。
- 依赖：P3-T01
- 完成记录：
  - 改动范围：复核 P3-T01 的 resolver/typecheck/sysroot/fixture 改动，并补齐 review 发现的 operator overload selection 缺口：`ops.rs` 在二元与 comparison operator-positioned 路径中，`operator` 候选过滤后会选择唯一 most-specific overload，而不是多个匹配时直接报 ambiguous；新增 `operator_overload_most_specific_after_modifier_gate_ok.scoop` 覆盖该路径。
  - 核心决策：`operator` modifier gate 仍发生在 applicability / specificity 前；普通 named call 路径不读取 `is_operator`，因此未标记的 `box.plus("hi")` 仍可作为普通成员调用；若过滤后的 operator 候选无唯一 most-specific，仍保留歧义诊断。
  - 验证结果：`cargo fmt`；`cargo build -p scoop -p scoopc`（fixture runner 会复用已有 `target/debug/scoopc`，因此显式重建）；`cargo clippy --all-targets -- -D warnings`；targeted operator fixtures（新增 most-specific fixture、modifier-required negative、modifier smoke、run-pass struct operator、plus/minus、bitwise/shift/inv）；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1553)`）。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `SPEC_FIX.md` A3 与 `PLAN.md` P3 operator modifier gate：operator-positioned calls 只考虑显式 `operator` 候选，same-name non-operator 会给清晰 modifier-required 诊断，普通 named call 不受影响，且 gate 后不再绕过现有 most-specific overload 选择语义。

### [DONE] P3-T02：将 `!!` 与 `as` failure 从 `Raise<RuntimeError>` 改为 `panic`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B3
- 目标：
  - `x!!` 和 `x as T` 的 failure path 是 assertion panic，不参与 effect system。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/expr/member.rs::infer_not_null_assert_expr_type`
  - `crates/scoopc_hir/src/typecheck/expr/infer.rs` `Cast` branch
  - `crates/scoopc_hir/src/hir/lower/expr/members.rs::{lower_not_null_assert_expr,synth_raise_null_assertion_failed,synth_raise_runtime_error_effect_ty}`
  - `crates/scoopc_mir/src/mir/lower/fn_lowering_expr.rs::{lower_cast_expr,lower_cast_as_expr_with_runtime_error_boundary,lower_cast_as_failure_raise}`
  - `crates/scoopc_mir/src/mir/lower/fn_lowering_call.rs::lower_direct_call_expr` for `scoop.core.panic` unreachable behavior
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs::emit_raise_runtime_error_variant`
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs::lower_runtime_error_boundary`
  - `sysroot/lib/scoop.core/src/core.scoop::{RuntimeError,panic,Raise}`
  - fixtures `not_null_assert*`, `runtime_typecheck_cast*`, `*as*.scoop`
- 必须实现的内容：
  1. Stop recording `Raise<RuntimeError>` effect edges for `!!` and `as` in typecheck.
  2. Change HIR lowering for not-null assertion failure to call `scoop.core.panic` with a stable message.
  3. Change MIR/runtime cast failure lowering from `Raise.raise(RuntimeError.ClassCastFailed)` to panic.
  4. Keep `as?` unchanged and returning `Option<T>`.
  5. Audit `RuntimeError`: if it remains used elsewhere, keep it; if only used by removed paths, delete or document remaining purpose.
  6. Add fixtures proving a function using `!!` or `as` no longer requires `Raise<RuntimeError>` in its effect row.
- 必须遵从的约束：
  - Do not implement panic by raising `RuntimeError` under the hood.
  - Preserve successful unwrap/cast behavior.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted fixtures for not-null assertion and runtime cast.
  3. `cargo test --all --all-targets`
- 完成条件：
  - `!!` and `as` failures panic and no longer poison effect rows.
- 依赖：P3-T01R
- 完成记录：
  - 改动范围：typecheck 不再为 `!!` 与 `as` 记录 `Raise<RuntimeError>` performed effect；HIR `!!` 的 `None` arm 改为合成 `scoop.core.panic("null assertion failed")` direct call；MIR `RuntimeCastFailure` 改为 `Panic { message }`，`as` failure block 直接调用 `scoop.core.panic("class cast failed")`；LLVM HIR/MIR cast failure codegen 改为 runtime panic；effect-lowered plan 不再把 HIR `as` 当作 runtime raise boundary；同步 sysroot `RuntimeError` 注释、targeted fixtures、HIR/MIR goldens 与相关 run-pass/typecheck 用例。
  - 核心决策：`!!` 与 `as` failure 是 assertion panic，不进入 effect row，也不能被 `try/catch Raise<RuntimeError>` 捕获；`as?` 保持 `ReturnNone` / `Option<T>` 行为不变；`RuntimeError` 继续保留给显式 `Raise<RuntimeError>` 与 continuation one-shot 违规等现有路径，不再作为 `!!` / `as` 的 lowering 目标。
  - 验证结果：`cargo fmt`；`cargo build -p scoop -p scoopc`；`cargo clippy --all-targets -- -D warnings`；targeted `!!` / cast typecheck、HIR、MIR、run-pass panic fixtures；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1555)`）。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `SPEC_FIX.md` B3 与 `PLAN.md` P3 对 `!!` / `as` panic 语义的要求：两者失败路径不再执行 `Raise.raise(RuntimeError.*)`，成功 unwrap/cast 与 `as?` 行为保持不变，用户可见失败不再泄露旧 effect requirement。

### [DONE] P3-T02R：Review `!!` 与 `as` panic 语义

- 参考：
  - P3-T02 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B3
- 目标：
  - 复核 `!!` / `as` failure 不再通过 effect system，并且成功路径不回归。
- 必须检查的文件/位置：
  - P3-T02 修改的 typecheck/HIR/MIR/codegen files
  - `sysroot/lib/scoop.core/src/core.scoop`
  - not-null / cast fixtures
- 必须实现的内容：
  1. 确认 typecheck 不再为 `!!` / `as` 记录 `Raise<RuntimeError>`。
  2. 确认 lowering failure arm 调用 `panic`，不是 `Raise.raise`。
  3. 确认 `as?` 行为不变。
  4. 确认 `RuntimeError` 处理有明确结论。
- 必须遵从的约束：
  - 不得接受“panic helper 内部再 raise RuntimeError”的实现。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted not-null/cast fixtures。
- 完成条件：
  - B3 语义完整闭合。
- 依赖：P3-T02
- 完成记录：
  - 改动范围：复核 P3-T02 涉及的 typecheck、HIR lowering、MIR lowering/materialization、LLVM codegen、sysroot `panic` / `RuntimeError` surface 与 not-null / cast fixtures；本轮 review 未发现需要改动编译器代码的缺口，仅更新任务状态与完成记录。
  - 核心决策：`!!` 与 `as` failure 均已走 `scoop.core.panic` / runtime `scoop_panic`，不再向 required-effects 写入 `Raise<RuntimeError>`，也不能被 `try/catch Raise<RuntimeError>` 捕获；`as?` 仍保持 `ReturnNone` / `Option<T>` 语义；`RuntimeError` 继续保留给显式 `Raise<RuntimeError>` 与 `Continuation.resume` runtime-error 边界，非本任务的 pattern mismatch 残留已由后续 P3-T04 覆盖。
  - 验证结果：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；targeted `not_null_assert_no_required_effect_ok.scoop`、`runtime_cast_as_no_required_effect_ok.scoop`、`cast_as_and_asq_ok.scoop`、`not_null_assert_failure_panic.scoop`、`type_check_cast_as_failure_panic.scoop`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1555)`）。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `SPEC_FIX.md` B3 与 `PLAN.md` P3 review 要求：failure path 是 assertion panic 而非 effect operation，成功 unwrap/cast 行为与 `as?` nullable cast 行为未回归，`RuntimeError` 的剩余用途有明确边界。

### [DONE] P3-T03：enum `with` mismatched variant 改为 panic

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C1
- 目标：
  - enum `with` update 指向的 variant 与 runtime variant 不匹配时 panic，而不是 silent no-op。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/expr/infer.rs::{infer_with_update_expr_type,resolve_with_update_enum_info}`
  - `crates/scoopc_hir/src/hir/lower/expr/canonical_call.rs::{lower_with_update_expr,build_with_enum_expr,build_with_copy_expr}`
  - any MIR lowering generated from HIR with-update branches
  - fixtures `*with_update*`, enum with update tests
- 必须实现的内容：
  1. Locate mismatch branch in enum `with` lowering that currently preserves original value.
  2. Replace mismatch branch with `panic("enum with variant mismatch")` or a stable equivalent message.
  3. Keep matching variant update behavior unchanged.
  4. Add run-pass/negative runtime fixture if runner supports expected panic; otherwise add IR/typecheck fixture proving panic call exists.
- 必须遵从的约束：
  - Do not change struct/tuple `with` semantics.
  - Do not route through `Raise<RuntimeError>`.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted with-update fixtures.
  3. `cargo test --all --all-targets`
- 完成条件：
  - enum variant mismatch can no longer silently preserve original value.
- 依赖：P3-T02R
- 完成记录：
  - 改动范围：`build_with_enum_expr` 的 enum with-update lowering 将“有 variant update 但运行期落到未更新 variant”的分支从保留原值改为 `scoop.core.panic("enum with variant mismatch")`；typecheck enum contract 注释明确 lowering 需要完整 variant 形状；同步 `with_update_enum_variant_payload_basic`，并新增 `with_update_enum_variant_mismatch_panic.scoop` expected-exit fixture。
  - 核心决策：空 `with {}` 仍保持 identity，避免把无目标 variant 的 no-op 表达式误判为 mismatch；只要当前 enum 层存在至少一个 variant update，未被该 update set 覆盖的 runtime variant 即视为 assertion mismatch 并 panic；struct / tuple with-update 路径不变，failure path 不经过 `Raise<RuntimeError>`。
  - 验证结果：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；targeted `with_update_enum_variant_mismatch_panic.scoop`、`with_update_enum_variant_payload_basic.scoop`、`with_update_enum_variant_payload_ok.scoop`、`with_update_tuple_nested_single_eval_basic.scoop`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1556)`）。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `SPEC_FIX.md` C1 与 `PLAN.md` P3 对 enum `with` mismatch panic 的要求：mismatched runtime variant 不再 silent preserve，matching variant update 行为保持不变，其他 aggregate with-update 未回归；阶段计划无变化，未修改 `PLAN.md`。

### [DONE] P3-T03R：Review enum `with` mismatch panic

- 参考：
  - P3-T03 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C1
- 目标：
  - 复核 enum with mismatch 从 silent preserve 改为 panic，且其他 with update 不回归。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/hir/lower/expr/canonical_call.rs`
  - `crates/scoopc_hir/src/typecheck/expr/infer.rs`
  - enum/tuple/struct with-update fixtures
- 必须实现的内容：
  1. 确认 mismatch branch 不再保留 original value。
  2. 确认 matching variant update 行为不变。
  3. 确认 struct/tuple `with` 不受影响。
  4. 确认 failure path 不走 `Raise`。
- 必须遵从的约束：
  - 不得接受只改 spec 未改 lowering 的结果。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted with-update fixtures。
- 完成条件：
  - C1 语义完整闭合。
- 依赖：P3-T03
- 完成记录：
  - 改动范围：复核 P3-T03 的 enum with-update typecheck contract、HIR lowering、panic helper 路径与 enum/tuple/struct with-update fixtures；本轮 review 未发现需要改动编译器代码的缺口，仅更新任务状态与完成记录。
  - 核心决策：`build_with_enum_expr` 在当前 enum 层存在 variant update 时，对运行期未命中 update set 的 variant arm 合成 `scoop.core.panic("enum with variant mismatch")`，不再返回 original value；匹配 variant 仍按 payload 字段重建，空 `with {}` 保持 identity，struct / tuple copy-update 路径不读 enum mismatch gate。
  - 验证结果：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；targeted `with_update_enum_variant_mismatch_panic.scoop`、`with_update_enum_variant_payload_basic.scoop`、`with_update_enum_variant_payload_ok.scoop`、`with_update_tuple_nested_single_eval_basic.scoop`、`with_update_simple.scoop`、`with_update_preserves_unchanged.scoop`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`（`fixtures: ok (1556)`）。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `SPEC_FIX.md` C1 与 `PLAN.md` P3 review 要求：enum `with` mismatched variant failure 是 panic 而非 silent preserve 或 `Raise<RuntimeError>`，matching enum update 与 struct/tuple with-update 行为未回归；阶段计划无变化，未修改 `PLAN.md`。

### [TODO] P3-T04：允许 refutable `val` pattern 并在 mismatch 时 panic

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C3
- 目标：
  - `val Some(x) = e` 等 refutable binding pattern 通过 typecheck；runtime mismatch panic。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/parser/pattern.rs::{parse_pattern,parse_variant_pattern,parse_tuple_pattern,parse_struct_pattern}`
  - `crates/scoopc_ast/src/parser/decls.rs::parse_val_decl`
  - `crates/scoopc_ast/src/parser/stmt.rs::parse_local_val_decl`
  - `crates/scoopc_hir/src/typecheck/val_pat.rs::infer_val_pat_bindings`
  - HIR lowering for val pattern bindings, likely `crates/scoopc_hir/src/hir/lower/patterns.rs` or adjacent pattern lowering module
  - MIR lowering for pattern destructure if separate
  - fixtures `*val_pattern*`, `*pattern*`
- 必须实现的内容：
  1. Remove `ValVariantPatRefutableNotAllowed` as a hard reject for val bindings.
  2. Mark refutable val pattern sites so lowering can synthesize mismatch fallback.
  3. Lower `val Some(x) = e` to equivalent `when`/branch shape with `panic("pattern mismatch")` fallback.
  4. Keep existing irrefutable tuple/struct destructuring behavior unchanged.
  5. Add fixtures for matching variant, mismatch panic path, and nested refutable patterns if supported by parser.
- 必须遵从的约束：
  - Do not use `Raise` or silent ignore for mismatch.
  - Do not accidentally allow refutable patterns where grammar still forbids patterns entirely.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted val pattern fixtures.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Refutable `val` patterns are useful and predictable, with panic-on-mismatch semantics.
- 依赖：P3-T03R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T04R：Review refutable `val` pattern

- 参考：
  - P3-T04 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C3
- 目标：
  - 复核 refutable val pattern 允许和 mismatch panic fallback 的完整性。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/val_pat.rs`
  - pattern HIR/MIR lowering files changed by P3-T04
  - val pattern fixtures
- 必须实现的内容：
  1. 确认 variant pattern 不再被 `ValVariantPatRefutableNotAllowed` 拒绝。
  2. 确认 mismatch fallback 是 panic。
  3. 确认 tuple/struct irrefutable destructuring 不回归。
  4. 确认 pattern binding scopes / variable types 正确。
- 必须遵从的约束：
  - 不得接受只放开 typecheck 但 lowering mismatch 未处理的实现。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted val pattern fixtures。
- 完成条件：
  - C3 语义完整闭合。
- 依赖：P3-T04
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T05：禁止 closure 捕获外层 `var`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B5
- 目标：
  - 捕获 `var` binding 的 closure 在前端报错，避免 snapshot semantics 造成 `makeCounter` 类 surprise。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/hir/lower/util/closures.rs::{compute_closure_captures,collect_declared_locals_in_expr,collect_used_locals_in_expr}`
  - typecheck local binding metadata for `val` vs `var`
  - closure inference in `crates/scoopc_hir/src/typecheck/expr/infer.rs::{infer_lambda_expr_type_without_expected,infer_lambda_expr_type_from_signature,try_infer_lambda_expr_type_by_expected}`
  - diagnostics definitions for closure capture errors
  - fixtures `*closure*capture*`, `*make_counter*` if present
- 必须实现的内容：
  1. Detect when a closure body references a binding declared outside the closure whose binding kind is `var`.
  2. Emit one clear diagnostic at the capture use or closure expression span.
  3. Diagnostic hint must mention explicit alternatives: `RefCell<T>` for shared mutable state, `val snapshot = ...` for read-only snapshot, fold/higher-order operators for accumulation patterns.
  4. Keep `val` capture behavior unchanged.
  5. Add negative fixture for `makeCounter`-style pattern and positive fixture for explicit `val snapshot`.
- 必须遵从的约束：
  - Do not implicitly box captured vars.
  - Do not change codegen closure environment layout except removing impossible captured-var cases.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted closure capture fixtures.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Closure capture rules no longer expose non-persistent `var` snapshot semantics.
- 依赖：P3-T04R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T05R：Review closure `var` capture 诊断

- 参考：
  - P3-T05 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B5
- 目标：
  - 复核 closure 捕获外层 `var` 的诊断完整性和 `val` capture 不回归。
- 必须检查的文件/位置：
  - closure capture analysis/typecheck files changed by P3-T05
  - closure capture fixtures
- 必须实现的内容：
  1. 确认跨 closure 边界引用外层 `var` 报错。
  2. 确认同 closure 内局部 `var` 使用不误报。
  3. 确认 `val` capture 仍可用。
  4. 确认 diagnostic hint 包含 explicit alternatives。
- 必须遵从的约束：
  - 不得通过隐式 boxing 修复旧语义。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted closure capture fixtures。
- 完成条件：
  - B5 语义完整闭合。
- 依赖：P3-T05
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T06：用 `ref` / `value` bound constraint kind 替换 `AnyRef` / `AnyValue` sealed marker

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C4
- 目标：
  - 删除 compile-time-only marker type 概念，把 reference/value restriction 表达为 generic bound constraint kind。
- 必须修改的文件/位置：
  - `sysroot/lib/scoop.core/src/core.scoop`，删除 `sealed interface AnyRef` / `AnyValue`，更新 `Atomic<T>` / `AtomicValue<T>` bounds
  - `crates/scoopc_hir/src/typecheck/type_env.rs::{SealedMarkerInfo,ANY_REF_MARKER_FQN,ANY_VALUE_MARKER_FQN,rebuild_sealed_marker_metadata}`
  - `crates/scoopc_hir/src/typecheck/lower.rs::{lower_bound_type_ref,sealed_marker_fqn,type_satisfies_sealed_marker,automatic_sealed_marker_for_type}`
  - `crates/scoopc_hir/src/typecheck/where_clause.rs::{check_file_where_clauses,check_one_where_clause}`
  - generic constraint satisfaction helpers used by call instantiation: `crates/scoopc_hir/src/typecheck/expr/call/generic.rs`
  - fixtures containing `AnyRef`, `AnyValue`, `Atomic<T>`, `AtomicValue<T>`
- 必须实现的内容：
  1. Introduce internal constraint kind for `T: ref` and `T: value`.
  2. Implement satisfaction check for reference class/interface/String/array/function refs and primitive/struct/enum value types according to existing type model.
  3. Handle `Nothing` consistently with subtype rules; document chosen behavior in completion record if non-obvious.
  4. Remove sysroot marker declarations and marker metadata rebuild if no longer needed.
  5. Reject uses of `ref` / `value` outside bound position if P2-T06 did not already enforce all contexts.
  6. Update sysroot and fixtures from `where T: AnyRef` / `AnyValue` to `<T: ref>` / `<T: value>` or equivalent where-clause form if chosen.
- 必须遵从的约束：
  - `ref` / `value` must not be expressible as runtime types, type arguments, casts, or supertypes.
  - Do not leave `AnyRef` / `AnyValue` as hidden aliases unless a documented compatibility requirement appears and design docs are updated first.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted `ref_value_bound` fixtures.
  3. targeted search: active sysroot/spec/compiler no longer treat `AnyRef` / `AnyValue` as declared types.
  4. `cargo test --all --all-targets`
- 完成条件：
  - Bound-kind constraints replace sealed marker types end-to-end.
- 依赖：P3-T05R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T06R：Review `ref` / `value` bound kind 替换结果

- 参考：
  - P3-T06 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C4
- 目标：
  - 复核 marker type 删除和 bound-kind constraint 的完整性。
- 必须检查的文件/位置：
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `crates/scoopc_hir/src/typecheck/type_env.rs`
  - `crates/scoopc_hir/src/typecheck/lower.rs`
  - `crates/scoopc_hir/src/typecheck/where_clause.rs`
  - generic constraint fixtures
- 必须实现的内容：
  1. 确认 `AnyRef` / `AnyValue` 不再是 active declared types。
  2. 确认 `ref` / `value` 不可作为普通类型使用。
  3. 确认 `Atomic<T>` / `AtomicValue<T>` 等 sysroot bounds 已更新。
  4. 确认 generic call instantiation 会检查 bound-kind satisfaction。
- 必须遵从的约束：
  - 不得接受 hidden marker aliases 或 runtime metadata footprint。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted `ref_value_bound` fixtures。
- 完成条件：
  - C4 语义完整闭合。
- 依赖：P3-T06
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T07：默认 visibility 改为 `internal` 并同步 sysroot / cone export

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P3
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C5
- 目标：
  - 无 visibility modifier 的 declaration 默认为 cone-internal；`public` 成为导出 API 的显式 opt-in。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/resolve/mod.rs::{Visibility,visibility_from_modifiers,ModifierSet::from_modifiers}`
  - `crates/scoopc_cone/src/scoopir/export.rs::{export_public_api_for_source,export_public_types_for_source,export_public_funs_for_source}`
  - `crates/scoopc_cone/src/visibility.rs::collect_non_public_symbols_for_cone_sources`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `sysroot/lib/scoop.collections/src/collections.scoop`
  - `sysroot/lib/scoop.unsafe/src/unsafe.scoop`
  - cross-package / cone fixtures under `tests/fixtures/**`
- 必须实现的内容：
  1. Change no-modifier default from `Visibility::Public` to `Visibility::Internal`.
  2. Add explicit `public` to sysroot declarations that are part of exported API.
  3. Ensure `.cone` exporter only emits explicit public declarations into `api.scoopir`.
  4. Update symbol visibility JSON expectations / tests if they exist.
  5. Add fixtures proving internal declarations are visible within same cone but not exported / not visible downstream.
- 必须遵从的约束：
  - Do not change `private` file-scope meaning.
  - Do not export internal declarations just to preserve old behavior.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. cone/export targeted tests.
  3. `cargo test --all --all-targets`
  4. Inspect generated `api.scoopir` for a sample cone to confirm only public declarations participate.
- 完成条件：
  - Default internal visibility and explicit public export contract are active.
- 依赖：P3-T06R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P3-T07R：Review default internal visibility

- 参考：
  - P3-T07 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C5
- 目标：
  - 复核默认 visibility、sysroot explicit public 和 cone export 行为。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/resolve/mod.rs`
  - `crates/scoopc_cone/src/scoopir/export.rs`
  - `crates/scoopc_cone/src/visibility.rs`
  - `sysroot/lib/**/src/*.scoop`
  - cone/export fixtures
- 必须实现的内容：
  1. 确认无 modifier declaration 默认为 `internal`。
  2. 确认 sysroot public API 已显式 `public`。
  3. 确认 `.cone` `api.scoopir` 只导出 public declarations。
  4. 确认 same-cone internal 可见，downstream 不可见。
- 必须遵从的约束：
  - 不得通过 exporter 重新导出 internal 来维持旧默认 public 行为。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted cone/export fixtures。
  3. `cargo test --all --all-targets`
- 完成条件：
  - C5 语义完整闭合；P4 可基于新 visibility 规则实现 overload definition-time checks。
- 依赖：P3-T07
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：
