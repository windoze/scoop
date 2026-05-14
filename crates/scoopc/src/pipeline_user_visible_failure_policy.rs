//! Executable audit baseline for the user-visible failure policy.
//!
//! This module freezes three boundaries for follow-up tasks:
//! - explicit frontend rejects for inputs outside the current language contract;
//! - stale user-visible unsupported buckets that later tasks must eliminate;
//! - internal bug sentinels that may remain only as impossible-state guards.

use std::fs;
use std::path::PathBuf;

const FAILURE_POLICY_AUDIT_FILES: &[&str] = &[
    "crates/scoopc/src/llvm/codegen/mir_body.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body.rs",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs",
    "crates/scoopc/src/llvm/codegen/mod.rs",
    "crates/scoopc/src/mir/materialize.rs",
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
        path: "crates/scoopc/src/llvm/codegen/mir_body.rs",
        expected_count: 314,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/body.rs",
        expected_count: 43,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs",
        expected_count: 210,
    },
    UnsupportedMainBodyCount {
        path: "crates/scoopc/src/llvm/codegen/mod.rs",
        expected_count: 235,
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
        marker: "fn refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir()",
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

const STALE_USER_VISIBLE_UNSUPPORTED_MARKERS: &[SourceMarker] = &[
    SourceMarker {
        path: "crates/scoopc/src/typecheck/lower.rs",
        marker: "kind: \"struct direct field type\",",
    },
    SourceMarker {
        path: "crates/scoopc/src/typecheck/when_pat.rs",
        marker: "kind: \"when variant pattern（空路径）\",",
    },
    SourceMarker {
        path: "crates/scoopc/src/typecheck/when_pat.rs",
        marker: "kind: \"when variant pattern（缺少 enum 声明信息）\",",
    },
    SourceMarker {
        path: "crates/scoopc/src/typecheck/when_pat.rs",
        marker: "kind: \"when variant pattern（enum type args 数量异常）\",",
    },
];

const INTERNAL_BUG_SENTINEL_HITS: &[&str] = &[
    "crates/scoopc/src/llvm/codegen/mir_body.rs:5528:            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(",
    "crates/scoopc/src/llvm/codegen/effect_lowered/body.rs:4407:                    _ => unreachable!(\"receiver_cg matched float above\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:2942:                    _ => unreachable!(\"match arms cover array builder build intrinsics\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:2990:                            _ => unreachable!(\"build_operation only contains builder build cases\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3002:                            _ => unreachable!(\"match arms cover array builder build intrinsics\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:3518:                    _ => unreachable!(\"filtered by match\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:5459:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:5540:                    _ => unreachable!(\"value.ty matched float above\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8303:                    _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8322:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8382:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8399:                _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8433:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8465:            _ => unreachable!(\"filtered by caller\"),",
    "crates/scoopc/src/llvm/codegen/mod.rs:8944:            _ => unreachable!(\"cast_float only accepts Float64/Float32\"),",
    "crates/scoopc/src/mir/materialize.rs:6216:                    panic!(",
    "crates/scoopc/src/mir/materialize.rs:8830:            panic!(",
    "crates/scoopc/src/typecheck/lower.rs:2365:                    _ => unreachable!(\"nominal lowering produced non-nominal type\"),",
    "crates/scoopc/src/typecheck/lower.rs:2967:                    _ => unreachable!(\"nominal lowering produced non-nominal type\"),",
    "crates/scoopc/src/typecheck/lower.rs:3212:                unreachable!(\"is_value_only_enum implies first_super exists\");",
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
    assert_eq!(total, 802);
}

#[test]
fn pipeline_user_visible_failure_policy_tracks_other_stale_unsupported_markers() {
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
