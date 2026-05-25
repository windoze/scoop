//! Executable audit baseline for the user-visible failure policy.
//!
//! This module freezes three boundaries for follow-up tasks:
//! - explicit frontend rejects for inputs outside the current language contract;
//! - already-cleared user-visible unsupported buckets that must stay cleared;
//! - internal bug sentinels that may remain only as impossible-state guards.

use std::fs;
use std::path::PathBuf;

const FAILURE_POLICY_AUDIT_FILES: &[&str] = &[
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/string.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/class_ctor.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/composed_call.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/continuation.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/effect_outcome.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/emit_entry.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/emitter.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/frame.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/gc_alloc.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/handle_boundary.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/handle_completion.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/lower_source.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_resume.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/mod.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/payload.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_types.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/step_case.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/symbol_naming.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/verification.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/wrapper.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/boxing.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/function.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/gc_locals.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/numeric.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs",
    "crates/scoopc_mir/src/mir/materialize/dispatch.rs",
    "crates/scoopc_mir/src/mir/materialize/entry.rs",
    "crates/scoopc_mir/src/mir/materialize/generic_mir.rs",
    "crates/scoopc_mir/src/mir/materialize/hir_calls.rs",
    "crates/scoopc_mir/src/mir/materialize/inputs.rs",
    "crates/scoopc_mir/src/mir/materialize/instance.rs",
    "crates/scoopc_mir/src/mir/materialize/mod.rs",
    "crates/scoopc_mir/src/mir/materialize/output.rs",
    "crates/scoopc_mir/src/mir/materialize/reachable.rs",
    "crates/scoopc_mir/src/mir/materialize/rewrite.rs",
    "crates/scoopc_mir/src/mir/materialize/run.rs",
    "crates/scoopc_mir/src/mir/materialize/seed.rs",
    "crates/scoopc_mir/src/mir/materialize/templates.rs",
    "crates/scoopc_mir/src/mir/materialize/utils.rs",
    "crates/scoopc_mir/src/mir/materialize/validation.rs",
    "crates/scoopc/src/pipeline/mir_stage.rs",
    "crates/scoopc_hir/src/stage.rs",
    "crates/scoopc_hir/src/typecheck/lower.rs",
    "crates/scoopc_hir/src/typecheck/when_pat.rs",
    "crates/scoopc_hir/src/typecheck/structs.rs",
];

const FAILURE_KEYWORDS: &[&str] = &["Unsupported", "todo!", "panic!", "unreachable!"];

const FRONTEND_REJECT_FORBIDDEN_TERMS: &[&str] =
    &["后端", "backend", "LLVM", "codegen", "Unsupported"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureClassification {
    FrontendReject,
    InternalBugSentinel,
    OutputDrift,
    EnvironmentToolchainLinkRuntime,
    StaleUserVisibleFailure,
}

impl FailureClassification {
    fn as_str(self) -> &'static str {
        match self {
            Self::FrontendReject => "frontend reject",
            Self::InternalBugSentinel => "internal bug sentinel",
            Self::OutputDrift => "output drift",
            Self::EnvironmentToolchainLinkRuntime => "environment/toolchain/link/runtime path",
            Self::StaleUserVisibleFailure => "stale user-visible failure",
        }
    }
}

const POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS: &[FailureClassification] = &[
    FailureClassification::InternalBugSentinel,
    FailureClassification::OutputDrift,
    FailureClassification::EnvironmentToolchainLinkRuntime,
];

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
        name: "output drift",
        meaning: "Post-HIR verifiers may fail when a later stage's output drifts away from the already-accepted HIR facts contract.",
    },
    FailureClassificationRule {
        name: "environment/toolchain/link/runtime path",
        meaning: "Toolchain, linker, runtime, and execution-environment failures may remain outside the HIR semantic barrier.",
    },
    FailureClassificationRule {
        name: "stale user-visible failure",
        meaning: "Previously user-visible `Unsupported*` buckets must stay replaced by real implementations, verifier failures, or earlier frontend rejects.",
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
        path: "crates/scoopc_hir/src/typecheck/when_pat.rs",
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
        path: "crates/scoopc_hir/src/typecheck/expr/infer.rs",
        marker: "Err(ExprTypeError::FunctionTypeCastNotSupported {",
    },
    SourceMarker {
        path: "crates/scoopc/src/pipeline/mir_stage.rs",
        marker: "fn mir_value_primitives_reject_open_function_type_cast_before_mir()",
    },
];

const EFFECTFUL_FUNCTION_TYPE_CAST_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/expr/infer.rs",
        marker: "return Err(ExprTypeError::EffectfulFunctionTypeCastNotSupported {",
    },
    SourceMarker {
        path: "crates/scoopc/src/pipeline/mir_stage.rs",
        marker: "function type runtime cast must be rejected before MIR",
    },
];

const USE_SITE_EFFECT_ROW_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/lower.rs",
        marker: "TypeLowerError::UseSiteEffectRowArgNotAllowed {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::use_site_eff_arg_not_allowed",
    },
];

const STRUCT_MUTABLE_FIELD_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/structs.rs",
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

const CALLING_CONVENTION_GENERIC_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/annotations.rs",
        marker: "AnnotationError::CallingConventionFunGenericsNotSupported {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/calling_convention_body_generic_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::calling_convention_fun_generics_not_supported",
    },
];

const TOP_LEVEL_VAR_STORAGE_POLICY_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/annotations.rs",
        marker: "AnnotationError::TopLevelVarRequiresThreadLocalOrGlobal {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/top_level_var_requires_threadlocal_or_global_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::top_level_var_requires_threadlocal_or_global",
    },
];

const SEALED_INTERFACE_USER_DEFINITION_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        marker: "SealedInterfaceUserDefinitionNotAllowed {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_user_definition_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_user_definition_not_allowed",
    },
];

const SEALED_INTERFACE_EMPTY_BODY_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        marker: "SealedInterfaceMustBeEmpty {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_body_member_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_must_be_empty",
    },
];

const SEALED_INTERFACE_SUPERTYPE_MUST_BE_SEALED_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        marker: "SealedInterfaceSupertypeMustBeSealed {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_supertype_normal_interface_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_supertype_must_be_sealed",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_supertype_class_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_supertype_must_be_sealed",
    },
];

const SEALED_INTERFACE_INHERITANCE_CYCLE_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        marker: "SealedInterfaceInheritanceCycle {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_self_cycle_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_inheritance_cycle",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_two_node_cycle_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_inheritance_cycle",
    },
];

const SEALED_INTERFACE_DECL_MUTUAL_EXCLUSION_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        marker: "SealedInterfaceMutuallyExclusiveBound {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_mutually_exclusive_sysroot_marker_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_mutually_exclusive_bound",
    },
];

const SEALED_INTERFACE_WHERE_MUTUAL_EXCLUSION_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/where_clause.rs",
        marker: "SealedInterfaceMutuallyExclusiveBound {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_mutually_exclusive_where_bound_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_mutually_exclusive_bound",
    },
];

const SEALED_INTERFACE_RUNTIME_TYPE_POSITION_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/lower.rs",
        marker: "SealedInterfaceBoundOnly {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_binding_type_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_param_type_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_return_type_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_type_argument_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_when_is_pattern_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_cast_as_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
];

const SEALED_INTERFACE_EXPLICIT_IMPLEMENTATION_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/typecheck/interfaces.rs",
        marker: "SealedInterfaceBoundOnly {",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_class_supertype_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
    SourceMarker {
        path: "tests/fixtures/typecheck/sealed_interface_struct_supertype_is_error.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::typecheck::sealed_interface_bound_only",
    },
];

const SPEC_UNCOVERED_ASYNC_AWAIT_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/resolve/mod.rs",
        marker: "AsyncAwaitSurfaceNotDefined {",
    },
    SourceMarker {
        path: "tests/fixtures/umb_fix/B-36-spec-uncovered/neg_async_await_surface_rejected.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::resolve::async_await_surface_not_defined",
    },
];

const SPEC_UNCOVERED_GENERATOR_YIELD_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc_hir/src/resolve/mod.rs",
        marker: "GeneratorYieldSurfaceNotDefined {",
    },
    SourceMarker {
        path: "tests/fixtures/umb_fix/B-36-spec-uncovered/neg_generator_yield_surface_rejected.scoop",
        marker: "// EXPECT-ERROR-CODE: scoop::resolve::generator_yield_surface_not_defined",
    },
];

const FRONTEND_REJECT_SURFACES: &[FrontendRejectSurface] = &[
    FrontendRejectSurface {
        gap_id: "B-36 async/await intentionally undefined surface",
        definition_path: "crates/scoopc_hir/src/resolve/mod.rs",
        diagnostic_code: "scoop::resolve::async_await_surface_not_defined",
        message: "当前语言 contract 不定义 `async` / `await` 语法或内建 task runtime surface",
        markers: SPEC_UNCOVERED_ASYNC_AWAIT_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "B-36 generator/yield intentionally undefined surface",
        definition_path: "crates/scoopc_hir/src/resolve/mod.rs",
        diagnostic_code: "scoop::resolve::generator_yield_surface_not_defined",
        message: "当前语言 contract 不定义专用 generator / `yield` 语法；请使用 effect + resuming handler",
        markers: SPEC_UNCOVERED_GENERATOR_YIELD_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.1",
        definition_path: "crates/scoopc_hir/src/typecheck/expr/error.rs",
        diagnostic_code: "scoop::typecheck::when_or_pattern_binder_not_allowed",
        message: "当前语言 contract 下，when or-pattern 不允许引入 binder",
        markers: OR_PATTERN_BINDER_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.2",
        definition_path: "crates/scoopc_hir/src/typecheck/expr/error.rs",
        diagnostic_code: "scoop::typecheck::function_type_cast_not_supported",
        message: "当前语言 contract 下，除 `Any as? (...)->R / Pure!` 外不接受函数类型的 runtime cast：{from} -> {to}；请改用函数子类型/coercion，或先包进 nominal wrapper",
        markers: FUNCTION_TYPE_CAST_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.2",
        definition_path: "crates/scoopc_hir/src/typecheck/expr/error.rs",
        diagnostic_code: "scoop::typecheck::effectful_function_type_cast_not_supported",
        message: "当前语言 contract 下，带非 `Pure` effect row 的函数类型不能参与显式 `as`/`as?`：{from} -> {to}；effect row 只存在于编译期，不能作为 runtime cast 合同",
        markers: EFFECTFUL_FUNCTION_TYPE_CAST_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "type lowering use-site effect row target validation",
        definition_path: "crates/scoopc_hir/src/typecheck/lower.rs",
        diagnostic_code: "scoop::typecheck::use_site_eff_arg_not_allowed",
        message: "当前语言 contract 下，只有显式声明 effect row 形参的名义类型才允许 use-site effect row 实参（`eff ...`）；{name} 不满足该条件",
        markers: USE_SITE_EFFECT_ROW_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "PIPELINE_GAPS §7.5",
        definition_path: "crates/scoopc_hir/src/typecheck/structs.rs",
        diagnostic_code: "scoop::typecheck::struct_field_must_be_val",
        message: "当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`：{struct_fqn}.{field}",
        markers: STRUCT_MUTABLE_FIELD_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "P2 HIR barrier @CallingConvention non-generic gate",
        definition_path: "crates/scoopc_hir/src/typecheck/annotations.rs",
        diagnostic_code: "scoop::typecheck::calling_convention_fun_generics_not_supported",
        message: "`@CallingConvention` 当前不支持泛型函数",
        markers: CALLING_CONVENTION_GENERIC_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "P2 HIR barrier top-level var storage policy gate",
        definition_path: "crates/scoopc_hir/src/typecheck/annotations.rs",
        diagnostic_code: "scoop::typecheck::top_level_var_requires_threadlocal_or_global",
        message: "顶层 `var` 必须显式标注 `@ThreadLocal` 或 `@Global`：{var_name}",
        markers: TOP_LEVEL_VAR_STORAGE_POLICY_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface sysroot-only definition",
        definition_path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_user_definition_not_allowed",
        message: "`sealed interface` 只能在 trusted `syslib` cone 中定义：{fqn}",
        markers: SEALED_INTERFACE_USER_DEFINITION_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface empty body",
        definition_path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_must_be_empty",
        message: "`sealed interface` body 必须为空：{fqn}",
        markers: SEALED_INTERFACE_EMPTY_BODY_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface supertype restriction",
        definition_path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_supertype_must_be_sealed",
        message: "`sealed interface` 只能继承其它 sealed interface：{fqn} -> {super_fqn}",
        markers: SEALED_INTERFACE_SUPERTYPE_MUST_BE_SEALED_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface inheritance cycle",
        definition_path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_inheritance_cycle",
        message: "`sealed interface` 继承图存在循环：{cycle}",
        markers: SEALED_INTERFACE_INHERITANCE_CYCLE_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface declaration mutual exclusion",
        definition_path: "crates/scoopc_hir/src/typecheck/type_env.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_mutually_exclusive_bound",
        message: "sealed marker bound 不能同时蕴涵 AnyRef 与 AnyValue：{fqn}",
        markers: SEALED_INTERFACE_DECL_MUTUAL_EXCLUSION_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface where-bound mutual exclusion",
        definition_path: "crates/scoopc_hir/src/typecheck/where_clause.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_mutually_exclusive_bound",
        message: "where 约束不能同时要求 `{param}` 满足 AnyRef 与 AnyValue",
        markers: SEALED_INTERFACE_WHERE_MUTUAL_EXCLUSION_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface runtime type position rejection",
        definition_path: "crates/scoopc_hir/src/typecheck/lower.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_bound_only",
        message: "sealed marker `{name}` 只能作为 generic/where bound 使用，不能作为运行期类型位置",
        markers: SEALED_INTERFACE_RUNTIME_TYPE_POSITION_MARKERS,
    },
    FrontendRejectSurface {
        gap_id: "CLOSURE_FIX §2 sealed interface explicit implementation rejection",
        definition_path: "crates/scoopc_hir/src/typecheck/interfaces.rs",
        diagnostic_code: "scoop::typecheck::sealed_interface_bound_only",
        message: "sealed marker `{name}` 只能作为 generic/where bound 使用，不能显式实现或继承",
        markers: SEALED_INTERFACE_EXPLICIT_IMPLEMENTATION_MARKERS,
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
        site: "crates/scoopc_hir/src/typecheck/lower.rs::lower_struct_direct_field_infos",
        upstream_gate: "typecheck::check_file_headers rejects struct/class primary-ctor params without a type annotation via TypeHeaderError::MissingTypeAnnotation",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_hir/src/typecheck/when_pat.rs::WhenPat::Variant empty path guard",
        upstream_gate: "parser guarantees a `WhenPat::Variant` path always contains at least one segment",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_hir/src/typecheck/when_pat.rs::WhenPat::Variant missing enum decl guard",
        upstream_gate: "enum_instance_from_type returns the enum FQN by looking it up in the type env, so the env must contain a corresponding enum_decl",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_hir/src/typecheck/when_pat.rs::WhenPat::Variant type-arg arity guard",
        upstream_gate: "enum_instance_from_type produces enum_args from the same nominal instance whose declaration we just looked up; arity matches by construction",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs::codegen_early_return",
        upstream_gate: "typecheck::expr::stmt rejects return outside the immediately enclosing named function via ReturnNotInFunctionBody",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs::codegen_mir_store_member",
        upstream_gate: "typecheck assignment/member-store gate rejects non-writable member-store targets before LLVM codegen",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs::codegen_mir_member_place",
        upstream_gate: "typecheck assignment/member-store gate rejects immutable class member stores before LLVM codegen",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs::codegen_struct_lit",
        upstream_gate: "typecheck struct literal gate rejects missing required fields before LLVM codegen",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs::codegen_mir_make_struct",
        upstream_gate: "typecheck/MIR struct gate preserves required field coverage before LLVM codegen",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs::codegen_equality/codegen_bool_logic/coerce_value",
        upstream_gate: "typecheck/operator gate proves scalar operator operands, string equality operands, and coercion targets before LLVM codegen",
    },
    UpstreamGuardedSentinel {
        site: "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs::source_slice_at and main/literal.rs::codegen_literal",
        upstream_gate: "parser/typecheck literal gate proves literal source spans, target types, and string materialization contracts before LLVM codegen",
    },
];

const INTERNAL_BUG_SENTINEL_HITS: &[&str] = &[
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:18:            panic!(\"codegen_mir_make_tuple: MIR verifier accepted non-tuple aggregate target type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:24:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:31:            panic!(\"codegen_mir_make_tuple: MIR verifier accepted tuple aggregate arity drift\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:40:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:64:                    panic!(\"codegen_mir_make_tuple: MIR verifier accepted valueless tuple element\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:126:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:240:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:246:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:252:            panic!(\"codegen_mir_make_struct: MIR verifier accepted struct without layout\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:255:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:270:                unreachable!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:275:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:320:                    panic!(\"codegen_mir_make_struct: MIR verifier accepted valueless struct field\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:347:            panic!(\"codegen_mir_tuple_get: MIR verifier accepted tuple get without operand type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:350:            panic!(\"codegen_mir_tuple_get: MIR verifier accepted tuple get on non-tuple type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:353:            panic!(\"codegen_mir_tuple_get: MIR verifier accepted tuple index drift\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:358:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:363:            panic!(\"codegen_mir_tuple_get: TypeStore equivalence verifier accepted unsupported tuple operand codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:369:                panic!(\"codegen_mir_tuple_get: MIR verifier accepted valueless tuple operand\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:440:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:482:            panic!(\"scoop_alloc_typed closure allocation must return a pointer\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:539:                panic!(\"scoop_alloc_typed closure env allocation must return a pointer\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:556:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:661:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:666:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:673:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:19:            panic!(\"codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:25:                panic!(\"codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:244:            panic!(\"codegen_bound_mir_call_args_from_signature: MIR call ABI verifier accepted arg binding drift\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:260:                    panic!(\"codegen_bound_mir_call_args_from_signature: TypeStore equivalence verifier accepted unsupported call arg type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:278:                    panic!(\"codegen_bound_mir_call_args_from_signature: MIR call ABI verifier accepted missing evaluated arg slot\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:32:                        panic!(\"codegen_mir_call: MIR call ABI verifier accepted non-function closure callee\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:40:                        panic!(\"codegen_mir_call: MIR call ABI verifier accepted non-function function-value callee\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:48:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:171:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:176:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:203:                panic!(\"codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted unsupported direct call return type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:327:                            panic!(\"codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted missing deferred return value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:345:            .unwrap_or_else(|| panic!(\"codegen_mir_class_ctor_call_at_site: verifier accepted missing class ctor call site\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:379:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:405:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:432:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:438:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:494:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:514:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:528:                panic!(\"codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier accepted unsupported function-value return type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:563:            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:680:                            panic!(\"codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier accepted missing deferred return value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:718:                        panic!(\"codegen_mir_plain_dynamic_call_with_policy: MIR verifier accepted non-function plain closure callee\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:732:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:737:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:749:                        panic!(\"codegen_mir_plain_dynamic_call_with_policy: MIR call ABI verifier accepted non-function plain function-value callee\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:758:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:763:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:775:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:780:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:817:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:836:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:116:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:121:            panic!(\"ensure_lir_source_closure_callable_defined: callable is not a closure body\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:162:        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:209:            panic!(\"declare_lir_source_closure_fun_with_signature: LIR source verifier accepted unsupported closure return type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:231:                    panic!(\"declare_lir_source_closure_fun_with_signature: TypeStore equivalence verifier accepted unsupported closure param type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:285:            panic!(\"declare_lir_source_plain_fun_with_symbol: LIR source ABI verifier accepted unsupported plain return type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:303:                panic!(\"declare_lir_source_plain_fun_with_symbol: TypeStore equivalence verifier accepted unsupported plain param type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:377:                        panic!(\"declare_lir_plain_fun_with_symbol: TypeStore equivalence verifier accepted unsupported plain param type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:399:                panic!(\"declare_lir_plain_fun_with_symbol: LIR facts verifier accepted unsupported plain return type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:480:            panic!(\"codegen_lir_source_closure_fun: LIR source ABI verifier accepted invalid CFG\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:488:                panic!(\"codegen_lir_source_closure_fun: LIR source verifier accepted unsupported closure return type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:502:                        panic!(\"codegen_lir_source_closure_fun: LIR source ABI verifier accepted missing sret param\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:535:                panic!(\"codegen_lir_source_closure_fun: LIR source ABI verifier accepted missing start block\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:575:            panic!(\"bind_mir_closure_params: MIR verifier accepted closure without env param\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:578:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:586:                panic!(\"bind_mir_closure_params: MIR verifier accepted missing env local slot\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:602:                    panic!(\"bind_mir_closure_params: MIR call ABI verifier accepted missing param local slot\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:605:                panic!(\"bind_mir_closure_params: TypeStore equivalence verifier accepted unsupported param type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:612:                        panic!(\"bind_mir_closure_params: MIR call ABI verifier accepted missing LLVM param\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:647:                        panic!(\"codegen_mir_closure_env_param: MIR verifier accepted non-tuple closure env\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:652:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:692:            _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:20:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:46:            panic!(\"codegen_mir_cast: MIR verifier accepted runtime cast metadata drift\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:70:            panic!(\"codegen_mir_cast_as: MIR verifier accepted invalid `as` failure contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:73:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:78:            panic!(\"codegen_mir_cast_as: MIR verifier accepted invalid `as` result contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:81:            panic!(\"codegen_mir_cast_as: MIR verifier accepted invalid `as` target contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:87:                panic!(\"codegen_mir_cast_as: TypeStore equivalence verifier accepted unsupported `as` target codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:92:                panic!(\"codegen_mir_cast_as: MIR verifier accepted unsupported `as` target type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:139:            panic!(\"codegen_mir_cast_asq: MIR verifier accepted invalid `as?` failure contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:142:            panic!(\"codegen_mir_cast_asq: MIR verifier accepted invalid `as?` result contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:145:            panic!(\"codegen_mir_cast_asq: MIR verifier accepted invalid `as?` target contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:151:                panic!(\"codegen_mir_cast_asq: TypeStore equivalence verifier accepted unsupported `as?` target codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:156:                panic!(\"codegen_mir_cast_asq: MIR verifier accepted unsupported `as?` target type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:159:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:166:                panic!(\"codegen_mir_cast_asq: TypeStore equivalence verifier accepted unsupported `as?` option codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:169:            panic!(\"codegen_mir_cast_asq: MIR verifier accepted invalid `as?` result type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:209:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:215:            .unwrap_or_else(|| panic!(\"codegen_mir_runtime_type_test_is_ok: TypeStore equivalence verifier accepted unsupported runtime type target\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:231:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:237:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:252:            _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:336:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:345:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:22:                        panic!(\"codegen_mir_const: parser/typecheck accepted invalid char literal\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:37:                    _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:62:                    _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:144:                    panic!(\"codegen_mir_pattern_match_value: MIR verifier accepted Int pattern for non-int subject\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:156:                    panic!(\"codegen_mir_pattern_match_value: MIR verifier accepted Char pattern for non-int subject\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:167:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:172:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:180:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:200:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:214:                    .unwrap_or_else(|| panic!(\"codegen_mir_pattern_match_value: MIR verifier accepted Bool pattern for non-bool subject\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:234:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:240:            .unwrap_or_else(|| panic!(\"codegen_mir_is_pattern_match: MIR verifier accepted unsupported pattern subject metadata type\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:242:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:256:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:262:            .unwrap_or_else(|| panic!(\"codegen_mir_is_pattern_match: MIR verifier accepted unsupported runtime target type\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:265:            .unwrap_or_else(|| panic!(\"codegen_mir_is_pattern_match: MIR verifier accepted non-codegen runtime target type\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:267:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:276:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:282:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:297:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:303:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:308:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:323:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:331:                panic!(\"codegen_mir_tuple_pattern_match: MIR verifier accepted unsupported tuple pattern element type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:352:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:357:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:369:                .unwrap_or_else(|| panic!(\"codegen_mir_variant_pattern_match: MIR verifier accepted unknown enum variant pattern\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:381:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:462:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:467:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:472:                        panic!(\"codegen_mir_pattern_extract: MIR verifier accepted tuple extraction field drift\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:486:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:496:                        .unwrap_or_else(|| panic!(\"codegen_mir_pattern_extract: MIR verifier accepted unknown enum variant extraction\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:550:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:157:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:163:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:293:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:328:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:374:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:416:            .unwrap_or_else(|| panic!(\"codegen_mir_plain_dispatch_call: verifier accepted missing dispatch receiver pointer\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:551:                    .unwrap_or_else(|| panic!(\"selected_mir_class_ctor_from_contract: verifier accepted missing selected ctor\")),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:555:                panic!(\"selected_mir_class_ctor_from_contract: verifier accepted unselected ctor contract\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:73:            .unwrap_or_else(|| panic!(\"codegen_mir_enum_variant_ctor_call: verifier accepted enum ctor TypeStore drift\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:79:            .unwrap_or_else(|| panic!(\"codegen_mir_enum_variant_ctor_call: verifier accepted unknown enum variant `{variant_name}`\"))",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:82:            panic!(\"codegen_mir_enum_variant_ctor_call: verifier accepted enum ctor arity drift\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:85:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:92:                panic!(\"codegen_mir_enum_variant_ctor_call: verifier accepted named enum ctor arg\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:130:            unreachable!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:136:            .unwrap_or_else(|| panic!(\"codegen_mir_store_member: verifier accepted non-codegen member store value type\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:140:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:145:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:150:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:178:                panic!(\"codegen_mir_store_top_level_var: extern LIR root is missing contract\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:181:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:197:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:240:                .unwrap_or_else(|| panic!(\"codegen_mir_member_place: verifier accepted missing class member receiver operand type\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:243:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:249:                    unreachable!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:282:            panic!(\"codegen_mir_member_place: verifier accepted non-local member store receiver\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:286:            panic!(\"codegen_mir_member_place: verifier accepted member receiver slot type drift\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:312:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:260:            panic!(\"mir_member_value_fqn_for_codegen: verifier accepted non-value member target\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:263:            panic!(\"mir_member_value_fqn_for_codegen: verifier accepted unresolved member target\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:275:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:282:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:287:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:349:            panic!(\"mir_closure_env_capture_element_cg_tys_from_contract: TypeStore equivalence verifier accepted unsupported closure env contract codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:352:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:360:                panic!(\"mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted non-tuple closure env\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:363:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:385:            _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:396:                        panic!(\"mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted missing closure env capture element\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:401:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/string.rs:15:            panic!(\"codegen_mir_unresolved_name_with_source_ty: MIR verifier accepted unsupported source type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:34:                panic!(\"bind_mir_params: MIR verifier accepted param arity drift\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:47:                    panic!(\"bind_mir_params: MIR verifier accepted unsupported param type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:121:                    panic!(\"bind_lir_source_params: LIR verifier accepted unsupported param type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:425:                        panic!(\"codegen_mir_rvalue: MIR verifier accepted closure env without codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:605:                    panic!(\"codegen_mir_effect_neutral_rvalue: MIR verifier accepted closure env without codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:72:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:209:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:215:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:220:            panic!(\"codegen_platform_literal: sysroot contract accepted non-Platform result type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:225:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:246:                    panic!(\"codegen_platform_literal: sysroot contract accepted unknown Platform field `{}`\", layout_field.name)",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:278:                panic!(\"codegen_platform_literal: verified Platform field materialized without LLVM value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:44:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:74:            panic!(\"mir_local_storage_cg_ty: MIR verifier accepted unsupported local type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:747:                panic!(\"mir_member_receiver_codegen_type_id: verifier accepted member receiver TypeStore drift\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:19:            panic!(\"codegen_mir_callable_value_args: MIR call ABI verifier accepted invalid closure argument binding\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:43:                    panic!(\"codegen_mir_callable_value_args: MIR call ABI verifier accepted missing closure argument\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:91:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:104:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:124:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:89:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:97:                        panic!(\"lower_dynamic_call_carrier: TypeStore equivalence verifier accepted unsupported FunPtr carrier codegen type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:109:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:123:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/composed_call.rs:481:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/composed_call.rs:491:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:431:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:453:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:462:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:469:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:479:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:441:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:470:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:485:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:568:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:663:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:681:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:690:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:700:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:717:            | mir::TerminatorKind::Todo(_) => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/payload.rs:80:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/payload.rs:388:                panic!(\"lower_completion_payload_as: effect verifier accepted unsupported completion payload target type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:84:                panic!(\"runtime_error_unit_variant_payload: verifier accepted missing RuntimeError LIR enum layout\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:91:                panic!(\"runtime_error_unit_variant_payload: verifier accepted missing RuntimeError variant `{variant_name}`\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:94:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:150:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:60:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:203:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:293:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:301:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:370:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/wrapper.rs:488:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:233:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:316:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:547:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:564:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:606:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:950:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1010:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1343:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1384:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1432:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1552:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1578:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1616:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1723:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2288:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2319:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2327:                    _ => unreachable!(\"filtered by match\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2352:            _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2380:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2398:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2422:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2435:                _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2654:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2939:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2949:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2973:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2985:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2996:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3011:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3017:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3038:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3120:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3130:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3404:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3425:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3738:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3761:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3809:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3942:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:48:            _ => unreachable!(\"cast_float only accepts Float64/Float32\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs:51:                        panic!(\"codegen_addressable_place: extern LIR root is missing contract\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs:105:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs:714:            unreachable!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:44:                    _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:49:            panic!(\"codegen_equality: typecheck gate accepted non-string rhs for string equality\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:60:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:89:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:98:                    panic!(\"codegen_equality: typecheck gate accepted incompatible float operands\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:105:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:117:            panic!(\"codegen_equality: typecheck gate accepted incompatible integer operands\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:126:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:148:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:293:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:14:            panic!(\"expect_insert_block: missing LLVM insert block while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:25:            panic!(\"expect_parent_function: block has no parent function while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:45:            panic!(\"expect_instruction_parent_block: instruction has no parent block while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:66:            panic!(\"expect_entry_block: function has no entry block while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:77:            panic!(\"expect_basic_value: call did not produce a basic value while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:89:            _ => panic!(\"expect_pointer_value: value was not a pointer while {context}\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:101:            _ => panic!(\"expect_int_value: value was not an integer while {context}\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:113:            _ => panic!(\"expect_float_value: value was not a float while {context}\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:125:            _ => panic!(\"expect_struct_value: value was not a struct while {context}\"),",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:136:            panic!(\"expect_cg_bool: value was not a bool payload while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:147:            panic!(\"expect_cg_int: value was not an integer payload while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:158:            panic!(\"expect_cg_float: value was not a float payload while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:169:            panic!(\"expect_cg_pointer: value did not publish a payload while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:181:            panic!(\"expect_cg_value: value did not publish a payload while {context}\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:469:            .unwrap_or_else(|| panic!(\"{context}: LIR facts are missing global root `{fqn}`\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:471:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:486:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:499:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:522:                panic!(\"source_slice_at: parser/typecheck accepted a span outside its source\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:525:            panic!(\"source_slice_at: parser/typecheck accepted an unsliceable source span\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs:140:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs:146:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:75:                panic!(\"codegen_type_check_expr: typecheck gate accepted non-runtime-ref operand\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:79:            panic!(\"codegen_type_check_expr: typecheck gate accepted valueless operand\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:82:            panic!(\"codegen_type_check_expr: typecheck gate accepted non-pointer runtime operand\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:118:                panic!(\"codegen_cast_as_expr: typecheck gate accepted non-runtime-ref target\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:127:                panic!(\"codegen_cast_as_expr: typecheck gate accepted non-runtime-ref operand\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:190:            panic!(\"codegen_cast_asq_expr: typecheck gate accepted non-Option `as?` result type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:198:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:209:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:215:            panic!(\"codegen_cast_asq_expr: typecheck gate accepted valueless `as?` operand\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:218:            panic!(\"codegen_cast_asq_expr: typecheck gate accepted non-pointer `as?` operand\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:243:            panic!(\"codegen_cast_asq_expr: verified Option Some produced no value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:251:            panic!(\"codegen_cast_asq_expr: verified Option None produced no value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:369:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:373:            _ => panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:17:                    panic!(\"codegen_var_ref: HIR verifier accepted an unbound local value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:130:            panic!(\"codegen_struct_lit: typecheck accepted non-struct struct literal type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:135:            panic!(\"codegen_struct_lit: typecheck accepted struct literal without nominal schema\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:140:            panic!(\"codegen_struct_lit: typecheck accepted struct literal without layout\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:150:                unreachable!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:200:                    panic!(\"codegen_struct_lit: typecheck accepted valueless struct field\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:221:            panic!(\"codegen_tuple_lit: typecheck accepted non-tuple tuple literal type\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:226:            panic!(\"codegen_tuple_lit: typecheck accepted tuple literal without tuple schema\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:230:            panic!(\"codegen_tuple_lit: typecheck accepted tuple literal arity drift\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:239:                panic!(\"codegen_tuple_lit: typecheck accepted unsupported tuple element type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:265:                    panic!(\"codegen_tuple_lit: typecheck accepted valueless tuple element\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:373:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:392:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:401:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:435:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:436:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:472:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:495:                        panic!(\"rebuild_value_from_storage_slot_with_explicit_frame: explicit-frame slot metadata ended before aggregate rebuild completed\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:643:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:701:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:804:            panic!(\"finalize_function_explicit_frame_lifecycle: verifier accepted missing explicit root frame descriptor global\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/function.rs:54:                    panic!(\"emit_function_return_block: function return context must publish return alloca\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/gc_locals.rs:85:            panic!(\"rematerialize_ptr_in_current_block: local slot GEP must publish source type\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/gc_locals.rs:305:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs:22:            panic!(\"defer_class_field_place: verifier accepted class field index drift\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs:144:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs:169:            panic!(\"declare_extern_global_storage: verifier accepted extern global initializer\");",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:376:            panic!(\"stable_closure_key_for_lir_source_callable: LIR source contract accepted missing closure callable\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:379:            panic!(\"stable_closure_key_for_lir_source_callable: callable is not a closure body\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:409:            panic!(\"direct_hir_closure_callable_fqn: closure codegen requires current callable owner\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:38:            panic!(\"codegen_decl_initializer_expr: typed HIR val must publish initializer\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:158:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:199:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:333:            .unwrap_or_else(|| panic!(\"codegen_top_level_immutable_value_init_fun_body: verifier accepted immutable value without initializer\"));",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:427:                panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:447:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:36:                        panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:40:                    panic!(\"emit_return: MIR return contract accepted missing return value\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:79:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs:211:                panic!(\"emit_raise_runtime_error_variant: verifier accepted missing RuntimeError unit variant `{variant_name}`\")",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs:214:            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:952:                    panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:962:                            panic!(",
    "crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:973:                                panic!(",
    "crates/scoopc_mir/src/mir/materialize/dispatch.rs:51:                    panic!(",
    "crates/scoopc_mir/src/mir/materialize/output.rs:40:            panic!(",
    "crates/scoopc_hir/src/typecheck/lower.rs:1242:                unreachable!(",
    "crates/scoopc_hir/src/typecheck/lower.rs:2487:                    _ => unreachable!(\"nominal lowering produced non-nominal type\"),",
    "crates/scoopc_hir/src/typecheck/lower.rs:3380:                    _ => unreachable!(\"nominal lowering produced non-nominal type\"),",
    "crates/scoopc_hir/src/typecheck/lower.rs:3631:                unreachable!(\"is_value_only_enum implies first_super exists\");",
    "crates/scoopc_hir/src/typecheck/when_pat.rs:211:                    unreachable!(",
    "crates/scoopc_hir/src/typecheck/when_pat.rs:260:                unreachable!(",
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
            FailureClassification::OutputDrift.as_str(),
            FailureClassification::EnvironmentToolchainLinkRuntime.as_str(),
            FailureClassification::StaleUserVisibleFailure.as_str(),
        ]
    );
    assert_eq!(
        POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS,
        &[
            FailureClassification::InternalBugSentinel,
            FailureClassification::OutputDrift,
            FailureClassification::EnvironmentToolchainLinkRuntime,
        ]
    );
    assert_eq!(
        FAILURE_KEYWORDS,
        ["Unsupported", "todo!", "panic!", "unreachable!"]
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
    println!(
        "post-HIR allowed failure classes:\n{}",
        POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS
            .iter()
            .map(|class| class.as_str())
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
    let plain_test_cfg = content.find("\n#[cfg(test)]");
    let compound_test_cfg = content.find("\n#[cfg(all(test");
    match [plain_test_cfg, compound_test_cfg]
        .into_iter()
        .flatten()
        .min()
    {
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
