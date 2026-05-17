//! Executable audit baseline for the user-visible failure policy.
//!
//! This module freezes three boundaries for follow-up tasks:
//! - explicit frontend rejects for inputs outside the current language contract;
//! - stale user-visible unsupported buckets that later tasks must eliminate;
//! - internal bug sentinels that may remain only as impossible-state guards.

use std::fs;
use std::path::PathBuf;

const FAILURE_POLICY_AUDIT_FILES: &[&str] = &[
    "crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/args.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/call.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/cast.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/const_pat.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/member.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/mod.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/op.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/operand.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/string.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/terminator.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/transport.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/types.rs",
    "crates/scoopc/src/llvm/codegen/mir_body/value_args.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/call_invoke.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/class_ctor.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/composed_call.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/continuation.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/effect_outcome.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/emit_entry.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/emitter.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/frame.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/gc_alloc.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/handle_boundary.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/handle_completion.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/lower_source.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/main_carrier.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/main_entry.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/main_resume.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/mod.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/payload.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/runtime_error.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/runtime_types.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/states.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/step_case.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/symbol_naming.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/verification.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body/wrapper.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs",
    "crates/scoopc/src/llvm/codegen/main/alloca.rs",
    "crates/scoopc/src/llvm/codegen/main/boxing.rs",
    "crates/scoopc/src/llvm/codegen/main/call.rs",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs",
    "crates/scoopc/src/llvm/codegen/main/const_eval.rs",
    "crates/scoopc/src/llvm/codegen/main/context.rs",
    "crates/scoopc/src/llvm/codegen/main/declare.rs",
    "crates/scoopc/src/llvm/codegen/main/expr_op.rs",
    "crates/scoopc/src/llvm/codegen/main/expr_value.rs",
    "crates/scoopc/src/llvm/codegen/main/frame.rs",
    "crates/scoopc/src/llvm/codegen/main/function.rs",
    "crates/scoopc/src/llvm/codegen/main/gc_locals.rs",
    "crates/scoopc/src/llvm/codegen/main/globals.rs",
    "crates/scoopc/src/llvm/codegen/main/identity.rs",
    "crates/scoopc/src/llvm/codegen/main/immut_value.rs",
    "crates/scoopc/src/llvm/codegen/main/literal.rs",
    "crates/scoopc/src/llvm/codegen/main/numeric.rs",
    "crates/scoopc/src/llvm/codegen/main/runtime_error.rs",
    "crates/scoopc/src/llvm/codegen/mod.rs",
    "crates/scoopc/src/mir/materialize/dispatch.rs",
    "crates/scoopc/src/mir/materialize/entry.rs",
    "crates/scoopc/src/mir/materialize/generic_mir.rs",
    "crates/scoopc/src/mir/materialize/hir_calls.rs",
    "crates/scoopc/src/mir/materialize/inputs.rs",
    "crates/scoopc/src/mir/materialize/instance.rs",
    "crates/scoopc/src/mir/materialize/mod.rs",
    "crates/scoopc/src/mir/materialize/output.rs",
    "crates/scoopc/src/mir/materialize/reachable.rs",
    "crates/scoopc/src/mir/materialize/rewrite.rs",
    "crates/scoopc/src/mir/materialize/run.rs",
    "crates/scoopc/src/mir/materialize/seed.rs",
    "crates/scoopc/src/mir/materialize/templates.rs",
    "crates/scoopc/src/mir/materialize/utils.rs",
    "crates/scoopc/src/mir/materialize/validation.rs",
    "crates/scoopc/src/pipeline/mir_stage.rs",
    "crates/scoopc/src/pipeline/hir_stage.rs",
    "crates/scoopc/src/typecheck/lower.rs",
    "crates/scoopc/src/typecheck/when_pat.rs",
    "crates/scoopc/src/typecheck/structs.rs",
];

const FAILURE_KEYWORDS: &[&str] = &[
    "UnsupportedMainBody",
    "Unsupported",
    "todo!",
    "panic!",
    "unreachable!",
];

const FRONTEND_REJECT_FORBIDDEN_TERMS: &[&str] =
    &["后端", "backend", "LLVM", "codegen", "UnsupportedMainBody"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureClassification {
    FrontendReject,
    InternalBugSentinel,
    StaleUserVisibleFailure,
}

impl FailureClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::FrontendReject => "frontend reject",
            Self::InternalBugSentinel => "internal bug sentinel",
            Self::StaleUserVisibleFailure => "stale user-visible failure",
        }
    }
}

struct FailureClassificationRule {
    name: &'static str,
    meaning: &'static str,
}

const FAILURE_CLASSIFICATION_RULES: &[FailureClassificationRule] = &[
    FailureClassificationRule {
        name: "frontend reject",
        meaning: "Inputs outside the current language contract must fail early with a stable, source-located diagnostic.",
    },
    FailureClassificationRule {
        name: "internal bug sentinel",
        meaning: "`panic!`/`unreachable!` may remain only as impossible-state guards after upstream contracts hold.",
    },
    FailureClassificationRule {
        name: "stale user-visible failure",
        meaning: "User-visible `Unsupported*` buckets that later tasks must replace with a real implementation, verifier failure, or earlier frontend reject.",
    },
];

struct UnsupportedMainBodyCount {
    path: &'static str,
    expected_count: usize,
}

const STALE_UNSUPPORTED_MAIN_BODY_COUNTS: &[UnsupportedMainBodyCount] = &[
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/aggregates.rs",
        expected_count: 42,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/args.rs",
        expected_count: 15,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/call.rs",
        expected_count: 28,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/callable_lookup.rs",
        expected_count: 21,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/cast.rs",
        expected_count: 28,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/const_pat.rs",
        expected_count: 38,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/dispatch.rs",
        expected_count: 16,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/member.rs",
        expected_count: 50,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/mod.rs",
        expected_count: 6,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/op.rs",
        expected_count: 12,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/operand.rs",
        expected_count: 8,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/string.rs",
        expected_count: 19,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/terminator.rs",
        expected_count: 17,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/transport.rs",
        expected_count: 10,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/types.rs",
        expected_count: 4,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mir_body/value_args.rs",
        expected_count: 6,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/call_invoke.rs",
        expected_count: 4,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/composed_call.rs",
        expected_count: 2,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/main_carrier.rs",
        expected_count: 5,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/main_entry.rs",
        expected_count: 9,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/payload.rs",
        expected_count: 2,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/runtime_error.rs",
        expected_count: 3,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/states.rs",
        expected_count: 5,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body/wrapper.rs",
        expected_count: 1,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs",
        expected_count: 158,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/alloca.rs",
        expected_count: 7,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/boxing.rs",
        expected_count: 6,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/call.rs",
        expected_count: 12,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/coerce.rs",
        expected_count: 31,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/context.rs",
        expected_count: 4,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/declare.rs",
        expected_count: 8,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/expr_op.rs",
        expected_count: 39,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/expr_value.rs",
        expected_count: 19,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/frame.rs",
        expected_count: 12,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/function.rs",
        expected_count: 3,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/gc_locals.rs",
        expected_count: 11,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/globals.rs",
        expected_count: 11,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/identity.rs",
        expected_count: 6,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/immut_value.rs",
        expected_count: 18,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/literal.rs",
        expected_count: 25,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/numeric.rs",
        expected_count: 21,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/main/runtime_error.rs",
        expected_count: 4,
    },
];

#[derive(Clone, Copy)]
struct SourceMarker {
    path: &'static str,
    marker: &'static str,
}

struct FrontendRejectSurface {
    gap_id: &'static str,
    definition_path: &'static str,
    diagnostic_code: &'static str,
    message: &'static str,
    markers: &'static [SourceMarker],
}

const OR_PATTERN_BINDER_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc/src/typecheck/when_pat.rs",
        marker: "ExprTypeError::WhenOrPatternBinderNotAllowed {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop",
        marker: "// EXPECT-ERROR: 当前语言 contract 下，when or-pattern 不允许引入 binder",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop",
        marker: "// EXPECT-ERROR: 当前语言 contract 下，when or-pattern 不允许引入 binder",
    },
];

const FUNCTION_TYPE_CAST_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc/src/typecheck/expr/infer.rs",
        marker: "Err(ExprTypeError::FunctionTypeCastNotSupported {",
    },
    SourceMarker {
        path: "crates/scoopc/src/pipeline/mir_stage.rs",
        marker: "fn mir_value_primitives_reject_unsupported_function_type_cast_before_mir()",
    },
];

const EFFECTFUL_FUNCTION_TYPE_CAST_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc/src/typecheck/expr/infer.rs",
        marker: "return Err(ExprTypeError::EffectfulFunctionTypeCastNotSupported {",
    },
    SourceMarker {
        path: "crates/scoopc/src/pipeline/mir_stage.rs",
        marker: "function type runtime cast must be rejected before MIR",
    },
];

const USE_SITE_EFFECT_ROW_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc/src/typecheck/lower.rs",
        marker: "TypeLowerError::UseSiteEffectRowArgNotAllowed {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::use_site_eff_arg_not_allowed",
    },
];

const STRUCT_MUTABLE_FIELD_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc/src/typecheck/structs.rs",
        marker: "StructDeclError::StructFieldMustBeVal {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop",
        marker: "// EXPECT-ERROR: 当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop",
        marker: "// EXPECT-ERROR: 当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`",
    },
];

const FRONTEND_REJECT_SURFACES: &[FrontendRejectSurface] = &[
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.1",
        definition_path: "crates/scoopc/src/typecheck/expr/error.rs",
        diagnostic_code: "scoop::typecheck::when_or_pattern_binder_not_allowed",
        message: "当前语言 contract 下，when or-pattern 不允许引入 binder",
        markers: OR_PATTERN_BINDER_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.2",
        definition_path: "crates/scoopc/src/typecheck/expr/error.rs",
        diagnostic_code: "scoop::typecheck::function_type_cast_not_supported",
        message: "当前语言 contract 下，显式 `as`/`as?` 不接受函数类型的 runtime cast：{from} -> {to}；请改用函数子类型/coercion，或先包进 nominal wrapper",
        markers: FUNCTION_TYPE_CAST_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.2",
        definition_path: "crates/scoopc/src/typecheck/expr/error.rs",
        diagnostic_code: "scoop::typecheck::effectful_function_type_cast_not_supported",
        message: "当前语言 contract 下，带非 `Pure` effect row 的函数类型不能参与显式 `as`/`as?`：{from} -> {to}；effect row 只存在于编译期，不能作为 runtime cast 合同",
        markers: EFFECTFUL_FUNCTION_TYPE_CAST_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "type lowering use-site effect row target validation",
        definition_path: "crates/scoopc/src/typecheck/lower.rs",
        diagnostic_code: "scoop::typecheck::use_site_eff_arg_not_allowed",
        message: "当前语言 contract 下，只有显式声明 effect row 形参的名义类型才允许 use-site effect row 实参（`eff ...`）；{name} 不满足该条件",
        markers: USE_SITE_EFFECT_ROW_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.5",
        definition_path: "crates/scoopc/src/typecheck/structs.rs",
        diagnostic_code: "scoop::typecheck::struct_field_must_be_val",
        message: "当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`：{struct_fqn}.{field}",
        markers: STRUCT_MUTABLE_FIELD_MARKERS,
    },
];

// P7-T02 final state: every previously-stale user-visible `Unsupported*` site
// in this audit list has been replaced by an internal-bug sentinel guarded by
// an upstream contract (see `INTERNAL_BUG_SENTINEL_HITS`). Reaching any of
// those sites now means the upstream gate drifted, not that the user wrote
// "not yet supported" syntax.
const STALE_USER_VISIBLE_UNSUPPORTED_MARKERS: &[SourceMarker] = &[];

/// Maps each previously-stale user-visible `Unsupported*` site to the upstream
/// frontend gate that makes it impossible on legal input. The actual sentinel
/// now lives in [`INTERNAL_BUG_SENTINEL_HITS`].
struct UpstreamGuardedSentinel {
    site: &'static str,
    upstream_gate: &'static str,
}

const POST_UPSTREAM_VALIDATION_GUARDS: &[UpstreamGuardedSentinel] = &[
    UpstreamGuardedSentinel {
        site: "crates/scoopc/src/typecheck/lower.rs::lower_struct_direct_field_infos",
        upstream_gate: "typecheck::check_file_headers rejects struct/class primary-ctor params without a type annotation via TypeHeaderError::MissingTypeAnnotation",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc/src/typecheck/when_pat.rs::WhenPat::Variant empty path guard",
        upstream_gate: "parser guarantees a `WhenPat::Variant` path always contains at least one segment",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc/src/typecheck/when_pat.rs::WhenPat::Variant missing enum decl guard",
        upstream_gate: "enum_instance_from_type returns the enum FQN by looking it up in the type env, so the env must contain a corresponding enum_decl",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc/src/typecheck/when_pat.rs::WhenPat::Variant type-arg arity guard",
        upstream_gate: "enum_instance_from_type produces enum_args from the same nominal instance whose declaration we just looked up; arity matches by construction",
    },
];

const INTERNAL_BUG_SENTINEL_HITS: &[&str] = &[
    "crates/scoopc/src/llvm/codegen/mir_body/call.rs:721:            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:2866:                    _ => unreachable!(\"match arms cover array builder build intrinsics\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:2914:                            _ => unreachable!(\"build_operation only contains builder build cases\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:2926:                            _ => unreachable!(\"match arms cover array builder build intrinsics\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3109:                    _ => unreachable!(\"filtered by match\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:4210:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/main/alloca.rs:48:            _ => unreachable!(\"cast_float only accepts Float64/Float32\"),",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs:74:                    _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs:93:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs:153:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs:170:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs:204:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/main/coerce.rs:236:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/mir/materialize/dispatch.rs:51:                    panic!(",
    "crates/scoopc/src/mir/materialize/output.rs:37:            panic!(",
    "crates/scoopc/src/typecheck/lower.rs:1212:                unreachable!(",
    "crates/scoopc/src/typecheck/lower.rs:2395:                    _ => unreachable!(\"nominal lowering produced non-nominal type\"),",
    "crates/scoopc/src/typecheck/lower.rs:3281:                    _ => unreachable!(\"nominal lowering produced non-nominal type\"),",
    "crates/scoopc/src/typecheck/lower.rs:3526:                unreachable!(\"is_value_only_enum implies first_super exists\");",
    "crates/scoopc/src/typecheck/when_pat.rs:209:                    unreachable!(",
    "crates/scoopc/src/typecheck/when_pat.rs:258:                unreachable!(",
];

#[test]
fn pipeline_user_visible_failure_policy_records_scope_keywords_and_rules() {
    let repo_root = repo_root();
    for path in FAILURE_POLICY_AUDIT_FILES {
        assert!(
            repo_root.join(path).is_file(),
            "failure-policy audit file should exist: {path}"
        );
    }

    let rule_names = FAILURE_CLASSIFICATION_RULES
        .iter()
        .map(|rule| rule.name)
        .collect::<Vec<_>>();
    assert_eq!(
        rule_names,
        vec![
            FailureClassification::FrontendReject.as_str(),
            FailureClassification::InternalBugSentinel.as_str(),
            FailureClassification::StaleUserVisibleFailure.as_str(),
        ]
    );
    assert_eq!(
        FAILURE_KEYWORDS,
        [
            "UnsupportedMainBody",
            "Unsupported",
            "todo!",
            "panic!",
            "unreachable!",
        ]
    );

    println!("audit files:\n{}", FAILURE_POLICY_AUDIT_FILES.join("\n"));
    println!("failure keywords:\n{}", FAILURE_KEYWORDS.join("\n"));
    println!(
        "classification rules:\n{}",
        FAILURE_CLASSIFICATION_RULES
            .iter()
            .map(|rule| format!("{}: {}", rule.name, rule.meaning))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn pipeline_user_visible_failure_policy_freezes_frontend_reject_surfaces() {
    for surface in FRONTEND_REJECT_SURFACES {
        let definition = read_repo_file(surface.definition_path);
        assert!(
            definition.contains(surface.diagnostic_code),
            "missing frontend reject diagnostic code `{}` in {}",
            surface.diagnostic_code,
            surface.definition_path
        );
        assert!(
            definition.contains(surface.message),
            "missing frontend reject message `{}` in {}",
            surface.message,
            surface.definition_path
        );
        for forbidden in FRONTEND_REJECT_FORBIDDEN_TERMS {
            assert!(
                !surface.message.contains(forbidden),
                "frontend reject `{}` should not read like backend unsupported: {}",
                surface.gap_id,
                surface.message
            );
        }
        for marker in surface.markers {
            assert_source_marker(*marker);
        }
        println!(
            "frontend reject: {} [{}] {}",
            surface.gap_id, surface.diagnostic_code, surface.message
        );
    }
}

#[test]
fn pipeline_user_visible_failure_policy_tracks_stale_unsupportedmainbody_counts() {
    let mut total = 0usize;
    for baseline in STALE_UNSUPPORTED_MAIN_BODY_COUNTS {
        let observed = production_source_text(baseline.path)
            .match_indices("UnsupportedMainBody")
            .count();
        assert_eq!(
            observed,
            baseline.expected_count,
            "{} count drifted in {}",
            FailureClassification::StaleUserVisibleFailure.as_str(),
            baseline.path
        );
        total += observed;
        println!(
            "{}: {} UnsupportedMainBody hits in {}",
            FailureClassification::StaleUserVisibleFailure.as_str(),
            observed,
            baseline.path
        );
    }
    assert_eq!(total, 746);
}

#[test]
fn pipeline_user_visible_failure_policy_tracks_other_stale_unsupported_markers() {
    assert!(
        STALE_USER_VISIBLE_UNSUPPORTED_MARKERS.is_empty(),
        "P7-T02 final state: every previously-stale user-visible Unsupported* site should have been replaced by an upstream-guarded internal-bug sentinel"
    );
    for marker in STALE_USER_VISIBLE_UNSUPPORTED_MARKERS {
        assert_source_marker(*marker);
        println!(
            "{}: {} => {}",
            FailureClassification::StaleUserVisibleFailure.as_str(),
            marker.path,
            marker.marker
        );
    }
}

#[test]
fn pipeline_user_visible_failure_policy_documents_upstream_guards() {
    assert!(
        !POST_UPSTREAM_VALIDATION_GUARDS.is_empty(),
        "post-upstream-validation guards should remain documented"
    );
    for guard in POST_UPSTREAM_VALIDATION_GUARDS {
        println!("upstream guard: {} <- {}", guard.site, guard.upstream_gate);
    }
}

#[test]
fn pipeline_user_visible_failure_policy_forbids_production_todo_macros() {
    let observed = collect_production_line_hits(|line| line.contains("todo!"));
    assert!(
        observed.is_empty(),
        "production failure-policy audit roots must not contain todo!: {}",
        observed.join("\n")
    );
}

#[test]
fn pipeline_user_visible_failure_policy_tracks_internal_bug_sentinels() {
    let observed = collect_production_line_hits(|line| {
        line.contains("panic!") || line.contains("unreachable!")
    });
    assert_eq!(observed, expected_lines(INTERNAL_BUG_SENTINEL_HITS));
    println!(
        "{} hits:\n{}",
        FailureClassification::InternalBugSentinel.as_str(),
        observed.join("\n")
    );
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_repo_file(path: &str) -> String {
    fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn production_source_text(path: &str) -> String {
    let content = read_repo_file(path);
    match content.find("\n#[cfg(test)]") {
        Some(cutoff) => content[..cutoff].to_string(),
        None => content,
    }
}

fn assert_source_marker(marker: SourceMarker) {
    let content = read_repo_file(marker.path);
    assert!(
        content.contains(marker.marker),
        "missing marker `{}` in {}",
        marker.marker,
        marker.path
    );
}

fn collect_production_line_hits(matches: impl Fn(&str) -> bool) -> Vec<String> {
    let mut hits = Vec::new();
    for path in FAILURE_POLICY_AUDIT_FILES {
        let content = production_source_text(path);
        hits.extend(
            content
                .lines()
                .enumerate()
                .filter(|(_, line)| matches(line))
                .map(|(index, line)| format!("{path}:{}:{line}", index + 1)),
        );
    }
    hits
}

fn expected_lines(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|line| (*line).to_string()).collect()
}
