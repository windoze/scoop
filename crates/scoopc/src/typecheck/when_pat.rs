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
use crate::ty::{BuiltinTypes, TypeId, TypeKind, ValueTypeKind};

use super::expr::{ExprTypeError, lower_type_ref_with_enum_subst};
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
    check_when_pat(
        source,
        pat,
        subject_ty,
        lower,
        builtins,
        &mut bindings,
    )?;
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
        ast::WhenPat::Else { .. }
        | ast::WhenPat::Wildcard { .. }
        | ast::WhenPat::IntLit { .. }
        | ast::WhenPat::StringLit { .. }
        | ast::WhenPat::BoolLit { .. } => Ok(()),
        ast::WhenPat::Is { ty, .. } => {
            // 当前阶段仅保证 TypeRef 可 lowering（运行期语义与 smart cast 留给后续阶段补齐）。
            let _ = lower.lower_type_ref(ty)?;
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
        ast::WhenPat::Variant { name, args, span } => {
            let variant_name = source.slice(name.span);

            let (enum_fqn, enum_args, enum_source) =
                enum_instance_from_type(source, expected_ty, lower, name.span)?;

            let decl = lower
                .env()
                .enum_decl(&enum_fqn)
                .cloned()
                .ok_or_else(|| ExprTypeError::UnsupportedExpr {
                    kind: "when variant pattern（缺少 enum 声明信息）",
                    span: (*span).into(),
                })?;

            let variant = decl
                .variants
                .iter()
                .find(|v| v.name == variant_name)
                .cloned()
                .ok_or_else(|| ExprTypeError::WhenVariantPatUnknownVariant {
                    enum_fqn: enum_fqn.clone(),
                    variant: variant_name.to_string(),
                    span: name.span.into(),
                })?;

            let expected_arity = variant.fields.len();
            let found_arity = args.len();
            if expected_arity != found_arity {
                return Err(ExprTypeError::WhenVariantPatArityMismatch {
                    variant_fqn: format!("{enum_fqn}.{variant_name}"),
                    expected: expected_arity,
                    found: found_arity,
                    span: (*span).into(),
                });
            }

            // 将 enum 声明处的 type params 映射到当前 subject 的实例化 type args。
            if decl.type_params.len() != enum_args.len() {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "when variant pattern（enum type args 数量异常）",
                    span: (*span).into(),
                });
            }

            let type_param_set: HashSet<&str> = decl.type_params.iter().map(|s| s.as_str()).collect();
            let subst: HashMap<String, TypeId> = decl
                .type_params
                .iter()
                .cloned()
                .zip(enum_args.into_iter())
                .collect();

            for (arg_pat, field) in args.iter().zip(variant.fields.iter()) {
                let expected_field_ty = lower_type_ref_with_enum_subst(
                    &enum_source,
                    *span,
                    &enum_fqn,
                    &field.ty,
                    lower,
                    builtins,
                    &type_param_set,
                    &subst,
                )?;
                check_when_pat(source, arg_pat, expected_field_ty, lower, builtins, bindings)?;
            }

            Ok(())
        }
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
            if !matches!(lower.nominal_decl_kind(&enum_fqn), Some(ast::TypeKind::Enum)) {
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
