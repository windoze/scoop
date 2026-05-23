//! `when` 分支 pattern 的最小类型检查（T0427）。
//!
//! 当前阶段目标：
//! - tuple pattern 只能用于 tuple/Unit 类型；
//! - enum variant pattern 只能用于 enum（含 builtin `Option<T>`）；
//! - bind 模式会把变量类型注入到当前 arm 的局部环境，供 arm body 使用。
//!
//! 非目标：
//! - 穷尽性检查（T0428）
//! - guard/or-pattern/struct pattern 等更完整的 pattern 系统

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::expr::{EnumTypeSubstContext, ExprTypeError, lower_type_ref_with_enum_subst};
use super::int_literals::check_positive_int_literal_for_type;
use super::lower::TypeLowering;

/// 对一个 `when` 分支 pattern 进行最小类型检查，并返回该 pattern 引入的局部绑定类型表。
///
/// 返回值：`decl_span -> TypeId`（decl_span 与 resolver 写回的 `ResolvedValueRef::Local` 对齐）。
pub(super) fn infer_when_pat_bindings(
    source: &SourceFile,
    pat: &ast::WhenPat,
    subject_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<HashMap<Span, TypeId>, ExprTypeError> {
    let mut bindings = HashMap::new();
    check_when_pat(source, pat, subject_ty, lower, builtins, &mut bindings)?;
    Ok(bindings)
}

fn check_when_pat(
    source: &SourceFile,
    pat: &ast::WhenPat,
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    bindings: &mut HashMap<Span, TypeId>,
) -> Result<(), ExprTypeError> {
    match pat {
        ast::WhenPat::Else { .. } | ast::WhenPat::Wildcard { .. } | ast::WhenPat::Rest { .. } => {
            Ok(())
        }
        ast::WhenPat::IntLit { span, .. } => {
            if is_integer_pattern_subject(expected_ty, lower, builtins) {
                check_positive_int_literal_for_type(source, *span, expected_ty, lower, builtins)?;
                Ok(())
            } else {
                Err(ExprTypeError::WhenIntPatNotInt {
                    found: lower.fmt_type(expected_ty),
                    span: (*span).into(),
                })
            }
        }
        ast::WhenPat::StringLit { span, .. } => {
            if is_string_pattern_subject(expected_ty, lower, builtins) {
                Ok(())
            } else {
                Err(ExprTypeError::WhenStringPatNotString {
                    found: lower.fmt_type(expected_ty),
                    span: (*span).into(),
                })
            }
        }
        ast::WhenPat::BoolLit { span } => {
            if expected_ty == builtins.bool_ {
                Ok(())
            } else {
                Err(ExprTypeError::WhenBoolPatNotBool {
                    found: lower.fmt_type(expected_ty),
                    span: (*span).into(),
                })
            }
        }
        ast::WhenPat::CharLit { span } => {
            if expected_ty == builtins.char_ {
                Ok(())
            } else {
                Err(ExprTypeError::WhenCharPatNotChar {
                    found: lower.fmt_type(expected_ty),
                    span: (*span).into(),
                })
            }
        }
        ast::WhenPat::Or { span, pats } => {
            // 当前阶段（T0825）后端需要为 or-pattern 生成正确的控制流；
            // 为避免在“不同分支绑定集合不一致”时引入未定义语义，这里先限制：
            // - or-pattern 内不得引入任何 binder（含嵌套 bind）。
            if pats.iter().any(when_pat_contains_bind) {
                return Err(ExprTypeError::WhenOrPatternBinderNotAllowed {
                    span: (*span).into(),
                });
            }

            for p in pats {
                check_when_pat(source, p, expected_ty, lower, builtins, bindings)?;
            }
            Ok(())
        }
        ast::WhenPat::Is { ty, is_span } => {
            let target_ty = lower.lower_type_ref(ty)?;
            check_when_is_pattern_target(expected_ty, target_ty, *is_span, lower, builtins)?;
            Ok(())
        }
        ast::WhenPat::Bind { ident } => {
            // `x`：把 subject（或更深层期望类型）绑定到局部变量 `x`。
            bindings.insert(ident.span, expected_ty);
            Ok(())
        }
        ast::WhenPat::Tuple { span, elements } => {
            // `()`：允许匹配 Unit。
            if elements.is_empty() && expected_ty == builtins.unit {
                return Ok(());
            }

            match lower.type_kind(expected_ty) {
                TypeKind::Value(ValueTypeKind::Tuple(expected_elements)) => {
                    // 解析 `..`：parser 已保证它最多出现一次且必须出现在最后一个位置。
                    let (prefix_pats, has_rest) = match elements.last() {
                        Some(ast::WhenPat::Rest { .. }) => {
                            (&elements[..elements.len().saturating_sub(1)], true)
                        }
                        _ => (elements.as_slice(), false),
                    };

                    if has_rest {
                        if expected_elements.len() < prefix_pats.len() {
                            return Err(ExprTypeError::WhenTuplePatTooShort {
                                expected_at_least: prefix_pats.len(),
                                found: expected_elements.len(),
                                span: (*span).into(),
                            });
                        }

                        for (p, ty) in prefix_pats.iter().zip(expected_elements.iter().copied()) {
                            check_when_pat(source, p, ty, lower, builtins, bindings)?;
                        }

                        return Ok(());
                    }

                    if expected_elements.len() != elements.len() {
                        return Err(ExprTypeError::WhenTuplePatArityMismatch {
                            expected: expected_elements.len(),
                            found: elements.len(),
                            span: (*span).into(),
                        });
                    }

                    for (p, ty) in elements.iter().zip(expected_elements.iter().copied()) {
                        check_when_pat(source, p, ty, lower, builtins, bindings)?;
                    }

                    Ok(())
                }
                _ => Err(ExprTypeError::WhenTuplePatNotTuple {
                    found: lower.fmt_type(expected_ty),
                    span: (*span).into(),
                }),
            }
        }
        ast::WhenPat::Variant { path, args, span } => {
            // Upstream gate: parser guarantees a `WhenPat::Variant` path always
            // contains at least one segment (the variant ident). An empty path
            // here therefore violates the parser contract.
            let variant_ident = path
                .segments
                .last()
                .copied()
                .expect("when variant pattern path should contain at least one segment");
            let variant_name = source.slice(variant_ident.span);

            let (enum_fqn, enum_args, enum_source) =
                enum_instance_from_type(source, expected_ty, lower, path.span)?;

            if path.segments.len() >= 2 {
                let prefix_segments = &path.segments[..path.segments.len() - 1];
                let start = prefix_segments.first().unwrap().span.start;
                let end = prefix_segments.last().unwrap().span.end;
                let prefix_span = Span::new(start, end);
                let prefix_names: Vec<String> = prefix_segments
                    .iter()
                    .map(|segment| source.slice(segment.span).to_string())
                    .collect();
                let prefix_fqn = lower.resolve_type_path_fqn_by_name(&prefix_names, prefix_span)?;
                let prefix_matches = prefix_fqn == enum_fqn;
                if !prefix_matches {
                    return Err(ExprTypeError::WhenVariantPatEnumMismatch {
                        expected: lower.fmt_type(expected_ty),
                        found: prefix_fqn,
                        span: prefix_span.into(),
                    });
                }
            }

            // Upstream gate: `enum_instance_from_type` returns the enum FQN by
            // looking it up in the type env above; the env therefore must
            // contain a corresponding `enum_decl`. A missing decl here violates
            // that resolve/type-env contract.
            let decl = lower
                .env()
                .enum_decl(&enum_fqn)
                .cloned()
                .unwrap_or_else(|| {
                    unreachable!(
                        "enum decl `{enum_fqn}` should exist after enum_instance_from_type succeeded",
                    )
                });

            let variant = decl
                .variants
                .iter()
                .find(|v| v.name == variant_name)
                .cloned()
                .ok_or_else(|| ExprTypeError::WhenVariantPatUnknownVariant {
                    enum_fqn: enum_fqn.clone(),
                    variant: variant_name.to_string(),
                    span: variant_ident.span.into(),
                })?;

            let variant_fqn = format!("{enum_fqn}.{variant_name}");

            // 解析 `..`：parser 已保证它最多出现一次且必须出现在最后一个位置。
            let (prefix_pats, has_rest) = match args.last() {
                Some(ast::WhenPat::Rest { .. }) => (&args[..args.len().saturating_sub(1)], true),
                _ => (args.as_slice(), false),
            };

            let expected_arity = variant.fields.len();
            let found_arity = prefix_pats.len();
            if has_rest {
                if expected_arity < found_arity {
                    return Err(ExprTypeError::WhenVariantPatTooShort {
                        variant_fqn,
                        expected_at_least: found_arity,
                        found: expected_arity,
                        span: (*span).into(),
                    });
                }
            } else if expected_arity != found_arity {
                return Err(ExprTypeError::WhenVariantPatArityMismatch {
                    variant_fqn,
                    expected: expected_arity,
                    found: found_arity,
                    span: (*span).into(),
                });
            }

            // Upstream gate: `enum_instance_from_type` produced `enum_args`
            // from the same nominal instance whose declaration we just looked
            // up; arity must match by construction. A mismatch here means the
            // resolve/type-env contract drifted.
            if decl.type_params.len() != enum_args.len() {
                unreachable!(
                    "enum `{enum_fqn}` type-arg arity drift between declaration and instance",
                );
            }

            let type_param_set: HashSet<String> = decl.type_params.iter().cloned().collect();
            let subst: HashMap<String, TypeId> =
                decl.type_params.iter().cloned().zip(enum_args).collect();

            for (arg_pat, field) in prefix_pats.iter().zip(variant.fields.iter()) {
                let expected_field_ty = lower_type_ref_with_enum_subst(
                    EnumTypeSubstContext {
                        decl_file: enum_source.path(),
                        enum_source: &enum_source,
                        use_span: *span,
                        enum_fqn: &enum_fqn,
                        builtins,
                        type_param_set: &type_param_set,
                        subst: &subst,
                    },
                    &field.ty,
                    lower,
                )?;
                check_when_pat(
                    source,
                    arg_pat,
                    expected_field_ty,
                    lower,
                    builtins,
                    bindings,
                )?;
            }

            Ok(())
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WhenPatternTypeTestFold {
    AlwaysTrue,
    AlwaysFalse,
    Dynamic,
}

fn check_when_is_pattern_target(
    subject_ty: TypeId,
    target_ty: TypeId,
    at: Span,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    if when_pattern_type_test_fold(subject_ty, target_ty, lower, builtins)
        != WhenPatternTypeTestFold::Dynamic
    {
        return Ok(());
    }
    if when_is_pattern_dynamic_runtime_supported(target_ty, lower) {
        return Ok(());
    }

    match lower.type_kind(target_ty) {
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let target = lower.fmt_type(target_ty);
            if fun.effects.is_pure() {
                Err(ExprTypeError::WhenFunctionTypePatternNotSupported {
                    target,
                    span: at.into(),
                })
            } else {
                Err(
                    ExprTypeError::WhenEffectfulFunctionTypePatternNotSupported {
                        target,
                        span: at.into(),
                    },
                )
            }
        }
        _ => Err(ExprTypeError::WhenTypePatternRuntimeTestNotSupported {
            subject: lower.fmt_type(subject_ty),
            target: lower.fmt_type(target_ty),
            span: at.into(),
        }),
    }
}

// Keep this in sync with `mir/lower.rs::runtime_type_static_fold(...)` so the frontend gate and
// MIR metadata agree about which `when is T` shapes are compile-time constants.
fn when_pattern_type_test_fold(
    subject_ty: TypeId,
    target_ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> WhenPatternTypeTestFold {
    if subject_ty == target_ty {
        return WhenPatternTypeTestFold::AlwaysTrue;
    }
    if target_ty == builtins.any {
        return WhenPatternTypeTestFold::AlwaysTrue;
    }
    if target_ty == builtins.nothing {
        return WhenPatternTypeTestFold::AlwaysFalse;
    }

    match (lower.type_kind(subject_ty), lower.type_kind(target_ty)) {
        (TypeKind::Value(_), TypeKind::Value(_)) => WhenPatternTypeTestFold::AlwaysFalse,
        (TypeKind::Value(_), TypeKind::Ref(_)) => WhenPatternTypeTestFold::AlwaysFalse,
        (TypeKind::Ref(RefTypeKind::String), TypeKind::Value(_))
        | (TypeKind::Ref(RefTypeKind::Function(_)), TypeKind::Value(_))
        | (TypeKind::Ref(RefTypeKind::Union(_)), TypeKind::Value(_)) => {
            WhenPatternTypeTestFold::AlwaysFalse
        }
        (TypeKind::Ref(RefTypeKind::Nominal(nominal)), TypeKind::Value(_))
            if lower.nominal_decl_kind(&nominal.fqn) != Some(ast::TypeKind::Interface) =>
        {
            WhenPatternTypeTestFold::AlwaysFalse
        }
        _ => WhenPatternTypeTestFold::Dynamic,
    }
}

fn when_is_pattern_dynamic_runtime_supported(target_ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    matches!(
        lower.type_kind(target_ty),
        TypeKind::Ref(RefTypeKind::Any)
            | TypeKind::Ref(RefTypeKind::String)
            | TypeKind::Ref(RefTypeKind::Nominal(_))
    )
}

fn is_integer_pattern_subject(
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if ty == builtins.int || ty == builtins.uint {
        return true;
    }

    match lower.type_kind(ty) {
        TypeKind::Value(
            ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_),
        ) => true,
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => matches!(
            nominal.fqn.as_str(),
            "scoop.core.Int8"
                | "scoop.core.Int16"
                | "scoop.core.Int32"
                | "scoop.core.Int64"
                | "scoop.core.UInt8"
                | "scoop.core.UInt16"
                | "scoop.core.UInt32"
                | "scoop.core.UInt64"
        ),
        _ => false,
    }
}

fn is_string_pattern_subject(ty: TypeId, lower: &TypeLowering<'_>, builtins: BuiltinTypes) -> bool {
    ty == builtins.string || matches!(lower.type_kind(ty), TypeKind::Ref(RefTypeKind::String))
}

fn when_pat_contains_bind(pat: &ast::WhenPat) -> bool {
    match pat {
        ast::WhenPat::Bind { .. } => true,
        ast::WhenPat::Tuple { elements, .. } => elements.iter().any(when_pat_contains_bind),
        ast::WhenPat::Variant { args, .. } => args.iter().any(when_pat_contains_bind),
        ast::WhenPat::Or { pats, .. } => pats.iter().any(when_pat_contains_bind),
        ast::WhenPat::Else { .. }
        | ast::WhenPat::Is { .. }
        | ast::WhenPat::Wildcard { .. }
        | ast::WhenPat::Rest { .. }
        | ast::WhenPat::IntLit { .. }
        | ast::WhenPat::CharLit { .. }
        | ast::WhenPat::StringLit { .. }
        | ast::WhenPat::BoolLit { .. } => false,
    }
}

fn enum_instance_from_type(
    source: &SourceFile,
    ty: TypeId,
    lower: &TypeLowering<'_>,
    use_span: Span,
) -> Result<(String, Vec<TypeId>, SourceFile), ExprTypeError> {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            // builtin `Option<T>`：在内部类型系统里不是 nominal，但语义上是 enum。
            let enum_fqn = "scoop.core.Option".to_string();
            let enum_args = vec![inner];
            let enum_source = lower
                .env()
                .enum_decl(&enum_fqn)
                .and_then(|d| lower.env().source(&d.decl_file).cloned())
                .unwrap_or_else(|| source.clone());
            Ok((enum_fqn, enum_args, enum_source))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let enum_fqn = nominal.fqn.clone();
            if !matches!(
                lower.nominal_decl_kind(&enum_fqn),
                Some(ast::TypeKind::Enum)
            ) {
                return Err(ExprTypeError::WhenVariantPatNotEnum {
                    found: lower.fmt_type(ty),
                    span: use_span.into(),
                });
            }

            let enum_source = lower
                .env()
                .enum_decl(&enum_fqn)
                .and_then(|d| lower.env().source(&d.decl_file).cloned())
                .unwrap_or_else(|| source.clone());

            Ok((enum_fqn, nominal.args.clone(), enum_source))
        }
        _ => Err(ExprTypeError::WhenVariantPatNotEnum {
            found: lower.fmt_type(ty),
            span: use_span.into(),
        }),
    }
}
