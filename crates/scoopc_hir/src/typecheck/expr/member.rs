use std::collections::HashSet;

use crate::ast;
use crate::resolve::Visibility;
use crate::span::Span;
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_utf8};
use crate::ty::{RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::infer::ExpectedTypeFrom;
use super::util::package_prefix;
use super::{ExprInferInputs, ExprTypeError};

use super::super::assignable::is_type_assignable;
use super::super::lower::TypeLowering;

struct MemberAccessInference {
    ty: TypeId,
    resolved: Option<ast::ResolvedMemberRef>,
}

/// 统一普通 member access 的“静态 resolved + typecheck 晚解析”入口。
///
/// 规则：
/// - 普通场景优先保留 resolver 已选中的成员目标；
/// - 若 resolver 因 receiver 只是另一个 member access 结果等原因无法写回 resolved，
///   则回退到“已推导出的 receiver 类型”做一次值成员晚解析；
/// - receiver lambda 的隐式 `this` 需要反过来优先使用晚解析结果，避免沿用 resolver
///   在外层 `this` 上留下的陈旧成员绑定。
pub(super) fn resolve_member_value_target_for_receiver(
    inputs: ExprInferInputs<'_>,
    receiver: &ast::Expr,
    receiver_ty: Option<TypeId>,
    member: &ast::MemberIdent,
    lower: &TypeLowering<'_>,
) -> Option<ast::ResolvedMemberRef> {
    let late_resolved = receiver_ty
        .and_then(|ty| resolve_member_value_target_from_receiver_ty(inputs, ty, member, lower));

    if inputs.is_current_lambda_this_expr(receiver) {
        late_resolved
    } else {
        member.resolved.clone().or(late_resolved)
    }
}

pub(super) fn infer_safe_member_access_expr_type(
    inputs: ExprInferInputs<'_>,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 receiver：保证其中的表达式也会被覆盖。
    let receiver_ty = inputs.infer(lower, receiver)?;

    let inner_ty = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
        _ => {
            return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                found: lower.fmt_type(receiver_ty),
                span: receiver.span.into(),
            });
        }
    };

    // T0152：safe member access 与普通 member access 共享同一套成员解析结果；
    // `?.` 只负责 unwrap `Option<T>` 并在最外层再包回 `Option<_>`。
    let resolved =
        resolve_member_value_target_for_receiver(inputs, receiver, Some(inner_ty), member, lower);
    let inferred = infer_member_access_with_receiver_ty(
        inputs,
        Some(inner_ty),
        member,
        resolved.as_ref(),
        lower,
    )?;
    if let Some(resolved) = inferred.resolved.clone() {
        lower.record_typechecked_member_resolution(member.span, resolved.clone());
        lower.record_safe_member_access_resolution(member.span, resolved);
    }

    Ok(lower.ty_option(inferred.ty))
}

pub(super) fn infer_elvis_expr_type(
    inputs: ExprInferInputs<'_>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = inputs.infer(lower, lhs)?;

    let inner_ty = match lower.type_kind(lhs_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
        _ => {
            return Err(ExprTypeError::ElvisLhsNotNullable {
                found: lower.fmt_type(lhs_ty),
                span: lhs.span.into(),
            });
        }
    };

    let rhs_ty = inputs.infer_in_expected(
        lower,
        rhs,
        inner_ty,
        ExpectedTypeFrom::new("Elvis `?:` 右操作数（由左侧 nullable 内层类型约束）"),
    )?;

    if !is_type_assignable(rhs_ty, inner_ty, lower, inputs.builtins) {
        return Err(ExprTypeError::ElvisRhsTypeMismatch {
            expected: lower.fmt_type(inner_ty),
            found: lower.fmt_type(rhs_ty),
            span: rhs.span.into(),
        });
    }

    Ok(inner_ty)
}

pub(super) fn infer_not_null_assert_expr_type(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    op_span: Span,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // T0421a：最小规则：
    // - `x!!` 的操作数必须是 nullable（`T?` / `Option<T>`）
    // - 结果类型为去掉 nullable 后的 inner type：`Option<T>` → `T`
    //
    // T0421b：`x!!` 的失败语义要求 `Raise<RuntimeError>`（静态 required effects）。
    let ty = inputs.infer(lower, expr)?;

    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let runtime_error = lower.lower_type_fqn_with_args(
                "scoop.core.RuntimeError".to_string(),
                Vec::new(),
                op_span,
            )?;
            let raise_runtime_error = lower.lower_type_fqn_with_args(
                "scoop.core.Raise".to_string(),
                vec![runtime_error],
                op_span,
            )?;
            lower.record_performed_effect(raise_runtime_error, op_span);
            Ok(inner)
        }
        _ => Err(ExprTypeError::NotNullAssertOperandNotNullable {
            found: lower.fmt_type(ty),
            span: expr.span.into(),
        }),
    }
}

pub(super) fn infer_splice_field_expr_type(
    inputs: ExprInferInputs<'_>,
    expr_span: Span,
    receiver: &ast::Expr,
    field: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // splice 字段访问：`receiver.[field]`（spec §6.4）
    //
    // HIR lowering 只消费这里发布的静态字段 contract，不再接受后续阶段补算字段名。
    let receiver_ty = inputs.infer(lower, receiver)?;

    let field_name = match infer_static_splice_field_name(inputs, field, lower)? {
        Some(field_name) => field_name,
        None => return Ok(inputs.builtins.any),
    };

    let contract =
        infer_splice_field_contract(inputs, receiver_ty, &field_name, field.span, lower)?;
    let field_ty = contract.field_ty;
    lower.record_splice_field_contract(expr_span, contract);
    Ok(field_ty)
}

fn infer_static_splice_field_name(
    inputs: ExprInferInputs<'_>,
    field: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<String>, ExprTypeError> {
    match &field.kind {
        ast::ExprKind::StringLit => {
            let raw = inputs.source.slice(field.span);
            match parse_string_literal_utf8(raw) {
                Ok(s) => Ok(Some(s)),
                Err(StringLiteralParseError::Invalid)
                | Err(StringLiteralParseError::InvalidUtf8)
                | Err(StringLiteralParseError::Interpolated) => {
                    Err(ExprTypeError::UnsupportedExpr {
                        kind: "splice field access（非法字符串字面量）",
                        span: field.span.into(),
                    })
                }
            }
        }
        ast::ExprKind::StructLit { fields, .. } => Ok(infer_splice_field_descriptor_name(
            inputs, field, fields, lower,
        )?),
        _ => {
            // 仍然递归 typecheck `field`，保证其中的表达式错误不会被“跳过”吞掉。
            let _ = inputs.infer(lower, field)?;
            Err(ExprTypeError::SpliceFieldNameNotStatic {
                span: field.span.into(),
            })
        }
    }
}

fn infer_splice_field_contract(
    inputs: ExprInferInputs<'_>,
    receiver_ty: TypeId,
    field_name: &str,
    field_span: Span,
    lower: &mut TypeLowering<'_>,
) -> Result<ast::SpliceFieldContract, ExprTypeError> {
    let (owner_fqn, field_ty) =
        resolve_splice_field_target(inputs, receiver_ty, field_name, field_span, lower)?;
    let field_fqn = format!("{owner_fqn}.{field_name}");
    let mutable = inputs
        .member_mutabilities
        .and_then(|member_mutabilities| member_mutabilities.get(&field_fqn))
        .copied()
        .unwrap_or(false);

    Ok(ast::SpliceFieldContract {
        receiver_ty,
        owner_fqn,
        field_name: field_name.to_string(),
        field_fqn,
        field_ty,
        mutable,
    })
}

fn resolve_splice_field_target(
    inputs: ExprInferInputs<'_>,
    receiver_ty: TypeId,
    field_name: &str,
    field_span: Span,
    lower: &mut TypeLowering<'_>,
) -> Result<(String, TypeId), ExprTypeError> {
    let receiver_fqn = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn.clone(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => n.fqn.clone(),
        TypeKind::Param(_) => {
            return Err(ExprTypeError::SpliceFieldNameNotStatic {
                span: field_span.into(),
            });
        }
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "splice field access（receiver 必须为名义类型）",
                span: field_span.into(),
            });
        }
    };

    let field_fqn = format!("{receiver_fqn}.{field_name}");

    // sysroot 跨文件特判：Pinned.value（与 infer_member_access_expr_type 保持一致）。
    if field_fqn == "scoop.core.Pinned.value" {
        return Ok((receiver_fqn, inputs.builtins.any));
    }

    let field_ty = inputs
        .struct_field_types
        .get(&field_fqn)
        .copied()
        .ok_or_else(|| ExprTypeError::UnsupportedMemberAccess {
            fqn: field_fqn.clone(),
            span: field_span.into(),
        })?;
    let field_ty = instantiate_member_value_type_from_receiver_ty(receiver_ty, &field_fqn, lower)?
        .unwrap_or(field_ty);

    Ok((receiver_fqn, field_ty))
}

fn infer_splice_field_descriptor_name(
    inputs: ExprInferInputs<'_>,
    field_expr: &ast::Expr,
    fields: &[ast::StructLitField],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<String>, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let mut seen_fields: HashSet<String> = HashSet::new();
    let mut saw_name = false;
    let mut literal_name = None;

    for field in fields {
        let field_name = source.slice(field.name.span).to_string();
        if !seen_fields.insert(field_name.clone()) {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "splice field access（struct 描述符字段重复）",
                span: field.name.span.into(),
            });
        }

        if field_name == "name" {
            saw_name = true;
            let found_ty = inputs.infer_in_expected(
                lower,
                &field.value,
                builtins.string,
                ExpectedTypeFrom::new("splice field access 描述符的 `name` 字段"),
            )?;
            if !is_type_assignable(found_ty, builtins.string, lower, builtins) {
                return Err(ExprTypeError::StructLitFieldTypeMismatch {
                    struct_name: "splice field descriptor".to_string(),
                    field: "name".to_string(),
                    expected: lower.fmt_type(builtins.string),
                    found: lower.fmt_type(found_ty),
                    span: field.value.span.into(),
                });
            }

            if !matches!(field.value.kind, ast::ExprKind::StringLit) {
                return Err(ExprTypeError::SpliceFieldNameNotStatic {
                    span: field.value.span.into(),
                });
            }

            let raw = source.slice(field.value.span);
            literal_name = Some(match parse_string_literal_utf8(raw) {
                Ok(s) => s,
                Err(StringLiteralParseError::Invalid)
                | Err(StringLiteralParseError::InvalidUtf8)
                | Err(StringLiteralParseError::Interpolated) => {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "splice field access（非法字符串字面量）",
                        span: field.value.span.into(),
                    });
                }
            });
        } else {
            let _ = inputs.infer(lower, &field.value)?;
        }
    }

    if !saw_name {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "splice field access（struct 描述符缺少 `name` 字段）",
            span: field_expr.span.into(),
        });
    }

    Ok(literal_name)
}

pub(super) fn infer_member_access_expr_type(
    inputs: ExprInferInputs<'_>,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // enum unit variant 值：`EnumName.Variant`（例如 `RuntimeError.NullAssertionFailed`）。
    //
    // 说明：
    // - resolver 会把该 member access 直接解析为一个 value FQN：`EnumFqn.Variant`；
    // - receiver（`EnumName`）在语义上只是“值命名空间的入口”，并非真正的运行期值；
    // - 因此这里在 typecheck 阶段直接返回 enum 类型，并跳过 receiver 的表达式 typecheck，
    //   避免把 enum type name 当作普通顶层值进行推导而报错（`UnsupportedTopLevelValueType`）。
    if let Some(ast::ResolvedMemberRef::Value { fqn }) = member.resolved.as_ref()
        && let Some((enum_fqn, variant_name)) = fqn.rsplit_once('.')
        && let Some(decl) = lower.env().enum_decl(enum_fqn)
        && decl
            .variants
            .iter()
            .any(|v| v.name == variant_name && v.fields.is_empty())
    {
        return Ok(lower.lower_type_fqn_with_args(
            enum_fqn.to_string(),
            Vec::new(),
            member.span,
        )?);
    }

    // 先递归类型检查 receiver：保证其中的表达式（如 `a().b` 的 `a()`）也会被覆盖，
    // 并在需要时为 tuple 元素访问提供 receiver 类型信息。
    //
    // 例外：`TypeName.member` 的 companion member access 中，receiver 可能不是一个“值表达式”，
    // resolver 会刻意保留 `Ident` 的未解析状态；此时跳过 receiver typecheck。
    let receiver_is_type_name =
        matches!(&receiver.kind, ast::ExprKind::Ident(id) if id.resolved.is_none());
    let receiver_ty = if receiver_is_type_name {
        None
    } else {
        Some(inputs.infer(lower, receiver)?)
    };
    let resolved =
        resolve_member_value_target_for_receiver(inputs, receiver, receiver_ty, member, lower);

    let inferred = infer_member_access_with_receiver_ty(
        inputs,
        receiver_ty,
        member,
        resolved.as_ref(),
        lower,
    )?;
    if let Some(resolved) = inferred.resolved.clone() {
        lower.record_typechecked_member_resolution(member.span, resolved);
    }

    Ok(inferred.ty)
}

pub(super) fn infer_member_access_ty_from_known_receiver(
    inputs: ExprInferInputs<'_>,
    receiver_ty: Option<TypeId>,
    member: &ast::MemberIdent,
    resolved: Option<&ast::ResolvedMemberRef>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    Ok(infer_member_access_with_receiver_ty(inputs, receiver_ty, member, resolved, lower)?.ty)
}

fn infer_member_access_with_receiver_ty(
    inputs: ExprInferInputs<'_>,
    receiver_ty: Option<TypeId>,
    member: &ast::MemberIdent,
    resolved: Option<&ast::ResolvedMemberRef>,
    lower: &mut TypeLowering<'_>,
) -> Result<MemberAccessInference, ExprTypeError> {
    match resolved {
        None => {
            // tuple 元素访问（spec §2.3.3）：`t.0` / `t.1` / ...
            //
            // 说明：
            // - tuple 并非名义类型，因此 resolver 阶段无法像 `Point.x` 一样写回成员 FQN；
            let Some(receiver_ty) = receiver_ty else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "member access（未 resolve）",
                    span: member.span.into(),
                });
            };

            let TypeKind::Value(ValueTypeKind::Tuple(elements)) = lower.type_kind(receiver_ty)
            else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "member access（未 resolve）",
                    span: member.span.into(),
                });
            };

            let member_name = inputs.source.slice(member.span);
            if let Some(new) = old_tuple_member_index_replacement(member_name) {
                return Err(ExprTypeError::TupleMemberOldSyntax {
                    old: member_name.to_string(),
                    new,
                    span: member.span.into(),
                });
            }

            let Some(idx) = parse_tuple_member_index(member_name) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "member access（未 resolve）",
                    span: member.span.into(),
                });
            };

            let ty = elements
                .get(idx)
                .copied()
                .ok_or_else(|| ExprTypeError::UnsupportedExpr {
                    kind: "tuple element access（index out of bounds）",
                    span: member.span.into(),
                })?;
            Ok(MemberAccessInference { ty, resolved: None })
        }
        Some(ast::ResolvedMemberRef::Value { fqn }) => {
            // `TypeName.NestedObject` / `Obj.NestedObject`：成员本身是一个 object 单例值。
            if lower.is_object_type(fqn) {
                return Ok(MemberAccessInference {
                    ty: lower.lower_type_fqn_with_args(fqn.clone(), Vec::new(), member.span)?,
                    resolved: Some(ast::ResolvedMemberRef::Value { fqn: fqn.clone() }),
                });
            }

            // sysroot 跨文件特判：Platform.*（spec §6.4 / TODO T1219）。
            //
            // 说明：
            // - 当前阶段 `struct_field_types` 只收集“当前文件内”的字段类型；
            // - `Platform` 定义在 sysroot，且其字段都是 `String`；
            // - 为了让 `getPlatform().arch/os/...` 在运行期语境可通过 typecheck，
            //   这里先做最小特判（后续可统一升级为“跨文件字段表”）。
            if fqn.starts_with("scoop.core.Platform.") {
                let Some(receiver_ty) = receiver_ty else {
                    return Err(ExprTypeError::UnsupportedMemberAccess {
                        fqn: fqn.clone(),
                        span: member.span.into(),
                    });
                };
                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) =
                    lower.type_kind(receiver_ty)
                    && nominal.fqn == "scoop.core.Platform"
                {
                    return Ok(MemberAccessInference {
                        ty: inputs.builtins.string,
                        resolved: Some(ast::ResolvedMemberRef::Value { fqn: fqn.clone() }),
                    });
                }
            }

            // spec §15.10：`Pinned.value`（early stage）。
            //
            // 说明：
            // - 当前阶段 `struct_field_types` 只收集“当前文件内”的 struct 字段类型；
            // - sysroot 的 `Pinned` 属于跨文件类型，因此这里对它做一个最小特判，以便
            //   `pinned.value` 在用户代码里可通过 typecheck（并支撑 T1008 的 run-pass fixture）。
            if fqn == "scoop.core.Pinned.value" {
                let Some(receiver_ty) = receiver_ty else {
                    return Err(ExprTypeError::UnsupportedMemberAccess {
                        fqn: fqn.clone(),
                        span: member.span.into(),
                    });
                };
                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) =
                    lower.type_kind(receiver_ty)
                    && nominal.fqn == "scoop.core.Pinned"
                {
                    return Ok(MemberAccessInference {
                        ty: inputs.builtins.any,
                        resolved: Some(ast::ResolvedMemberRef::Value { fqn: fqn.clone() }),
                    });
                }
            }

            // spec §15.10.1：`GcHandle.raw`（early stage）。
            //
            // 说明：
            // - 当前阶段 `struct_field_types` 只收集“当前文件内”的 struct 字段类型；
            // - sysroot 的 `GcHandle` 属于跨文件类型，因此这里对 `raw` 做最小特判，
            //   以便 `handle.raw` 在用户代码里可通过 typecheck（并支撑后续 FFI 桥接）。
            if fqn == "scoop.core.GcHandle.raw" {
                let Some(receiver_ty) = receiver_ty else {
                    return Err(ExprTypeError::UnsupportedMemberAccess {
                        fqn: fqn.clone(),
                        span: member.span.into(),
                    });
                };
                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) =
                    lower.type_kind(receiver_ty)
                    && nominal.fqn == "scoop.core.GcHandle"
                {
                    return Ok(MemberAccessInference {
                        ty: lower.lower_type_fqn_with_args(
                            "scoop.core.UIntPtr".to_string(),
                            Vec::new(),
                            member.span,
                        )?,
                        resolved: Some(ast::ResolvedMemberRef::Value { fqn: fqn.clone() }),
                    });
                }
            }

            let ty = inputs.struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })?;
            let ty = if let Some(receiver_ty) = receiver_ty {
                instantiate_member_value_type_from_receiver_ty(receiver_ty, fqn, lower)?
                    .unwrap_or(ty)
            } else {
                ty
            };
            Ok(MemberAccessInference {
                ty,
                resolved: Some(ast::ResolvedMemberRef::Value { fqn: fqn.clone() }),
            })
        }
        Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => {
            // T0112：Extension property getter — look up the getter function's return type.
            if let Some(sigs) = inputs.top_level_funs.get(fqn.as_str())
                && let Some(sig) = sigs.first()
            {
                return Ok(MemberAccessInference {
                    ty: sig.return_ty,
                    resolved: Some(ast::ResolvedMemberRef::ExtensionValue { fqn: fqn.clone() }),
                });
            }
            Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            })
        }
        Some(
            ast::ResolvedMemberRef::Fun { fqn } | ast::ResolvedMemberRef::ExtensionFun { fqn },
        ) => Err(ExprTypeError::UnsupportedMemberAccess {
            fqn: fqn.clone(),
            span: member.span.into(),
        }),
    }
}

pub(super) fn resolve_member_value_target_from_receiver_ty(
    inputs: ExprInferInputs<'_>,
    receiver_ty: TypeId,
    member: &ast::MemberIdent,
    lower: &TypeLowering<'_>,
) -> Option<ast::ResolvedMemberRef> {
    let receiver_ty_fqn = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) | TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            n.fqn
        }
        _ => return None,
    };

    let member_name = inputs.source.slice(member.span);
    let direct_fqn = format!("{receiver_ty_fqn}.{member_name}");
    let direct_exists = member_value_exists(inputs, lower, &receiver_ty_fqn, &direct_fqn);
    let ext_candidate =
        find_extension_property_candidate(inputs, lower, &receiver_ty_fqn, member_name);

    if direct_exists {
        return Some(ast::ResolvedMemberRef::Value { fqn: direct_fqn });
    }

    ext_candidate.map(|fqn| ast::ResolvedMemberRef::ExtensionValue { fqn })
}

fn member_value_exists(
    inputs: ExprInferInputs<'_>,
    lower: &TypeLowering<'_>,
    receiver_ty_fqn: &str,
    direct_fqn: &str,
) -> bool {
    member_value_symbol_visible_from_use_site(inputs.source, lower, direct_fqn)
        || lower.is_object_type(direct_fqn)
        || direct_fqn == "scoop.core.Pinned.value"
        || direct_fqn == "scoop.core.GcHandle.raw"
        || (receiver_ty_fqn == "scoop.core.Platform"
            && direct_fqn.starts_with("scoop.core.Platform."))
}

fn member_value_symbol_visible_from_use_site(
    use_source: &crate::source::SourceFile,
    lower: &TypeLowering<'_>,
    fqn: &str,
) -> bool {
    let use_cone = lower.index().cone_of_source(use_source);
    lower
        .index()
        .by_fqn
        .get(fqn)
        .and_then(|syms| syms.value.as_ref())
        .is_some_and(|symbol| match symbol.visibility {
            Visibility::Public => true,
            Visibility::Internal => symbol.decl_cone == use_cone,
            Visibility::Private => symbol.decl_file == use_source.path(),
        })
}

fn find_extension_property_candidate(
    inputs: ExprInferInputs<'_>,
    lower: &TypeLowering<'_>,
    receiver_ty_fqn: &str,
    member_name: &str,
) -> Option<String> {
    fn normalize_collections_alias(fqn: &str) -> &str {
        match fqn {
            "scoop.core.List" => "scoop.core.Array",
            "scoop.core.MutableList" => "scoop.core.MutableArray",
            "scoop.collections.Set" => "scoop.core.Array",
            "scoop.collections.MapView" => "scoop.core.Array",
            "scoop.collections.MutableSet" => "scoop.core.MutableArray",
            "scoop.collections.MutableMap" => "scoop.core.MutableArray",
            _ => fqn,
        }
    }

    let receiver_ty_fqn_norm = normalize_collections_alias(receiver_ty_fqn);
    let use_cone = lower.index().cone_of_source(inputs.source);
    let imports = lower.imports();
    let is_visible = |ext: &crate::resolve::ExtensionPropertySymbol| -> bool {
        match ext.visibility {
            Visibility::Public => true,
            Visibility::Internal => ext.decl_cone == use_cone,
            Visibility::Private => ext.decl_file == inputs.source.path(),
        }
    };
    let receiver_matches =
        |ext: &crate::resolve::ExtensionPropertySymbol| match ext.receiver_ty_fqn.as_deref() {
            Some(ext_receiver) => {
                ext_receiver == "scoop.core.Any"
                    || normalize_collections_alias(ext_receiver) == receiver_ty_fqn_norm
            }
            None => false,
        };

    let mut candidates: Vec<String> = Vec::new();

    for ext in &lower.index().extension_properties {
        if ext.decl_cone == use_cone
            && ext.pkg_prefix == lower.pkg_prefix()
            && ext.name == member_name
            && receiver_matches(ext)
            && is_visible(ext)
        {
            candidates.push(ext.fqn.clone());
        }
    }

    for prefix in &imports.star {
        for ext in &lower.index().extension_properties {
            if ext.pkg_prefix == *prefix
                && ext.name == member_name
                && receiver_matches(ext)
                && is_visible(ext)
            {
                candidates.push(ext.fqn.clone());
            }
        }
    }

    if let Some(imported) = imports.value.explicit.get(member_name) {
        for imported_fqn in imported {
            for ext in lower
                .index()
                .extension_properties
                .iter()
                .filter(|ext| ext.fqn == *imported_fqn)
            {
                if receiver_matches(ext) && is_visible(ext) {
                    candidates.push(ext.fqn.clone());
                }
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    candidates.into_iter().next()
}

fn parse_tuple_member_index(text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    if !text.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    text.parse::<usize>().ok()
}

fn old_tuple_member_index_replacement(text: &str) -> Option<String> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(digits.to_string())
}

/// 依据 receiver 的具体 nominal 实例，把成员声明类型重新 lower 成使用点结果类型。
fn instantiate_member_value_type_from_receiver_ty(
    receiver_ty: TypeId,
    member_fqn: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some((owner_fqn, concrete_args)) =
        find_member_owner_nominal_instantiation(receiver_ty, member_fqn, lower)?
    else {
        return Ok(None);
    };
    let Some(type_ref) = find_member_decl_type_ref(lower, &owner_fqn, member_fqn) else {
        return Ok(None);
    };
    let Some(sym) = lower.env().type_symbol(&owner_fqn).cloned() else {
        return Ok(None);
    };

    let ty = lower.lower_type_ref_in_decl_file_with_bindings(
        &sym.decl_file,
        sym.type_param_names
            .iter()
            .cloned()
            .zip(concrete_args.iter().copied()),
        &type_ref,
    )?;
    Ok(Some(ty))
}

/// 沿 receiver 及其已具体化的 direct supertypes 查找成员所属 nominal 的具体实例。
///
/// 该 helper 同时服务：
/// - member value type 的使用点重新实例化；
/// - member direct-call 的 owner-specialization 实例请求记录。
pub(super) fn find_member_owner_nominal_instantiation(
    receiver_ty: TypeId,
    member_fqn: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<(String, Vec<TypeId>)>, ExprTypeError> {
    let Some((member_owner_fqn, _)) = member_fqn.rsplit_once('.') else {
        return Ok(None);
    };

    let mut stack = vec![receiver_ty];
    let mut visited: HashSet<TypeId> = HashSet::new();

    while let Some(cur) = stack.pop() {
        if !visited.insert(cur) {
            continue;
        }

        let (nominal_fqn, nominal_args) = match lower.type_kind(cur) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
            | TypeKind::Ref(RefTypeKind::Nominal(nominal)) => (nominal.fqn, nominal.args),
            _ => continue,
        };

        if nominal_fqn == member_owner_fqn {
            return Ok(Some((nominal_fqn, nominal_args)));
        }

        stack.extend(lower.instantiated_direct_supertypes(cur)?);
    }

    Ok(None)
}

/// 从成员声明处找回原始 `TypeRef`，供后续在声明处文件上下文中重新 lower。
fn find_member_decl_type_ref(
    lower: &TypeLowering<'_>,
    owner_fqn: &str,
    member_fqn: &str,
) -> Option<ast::TypeRef> {
    let member_name = member_fqn.strip_prefix(owner_fqn)?.strip_prefix('.')?;
    if member_name.contains('.') {
        return None;
    }

    let sym = lower.env().type_symbol(owner_fqn)?;
    let source = lower.env().source(&sym.decl_file)?;
    let file = lower.env().file_ast(&sym.decl_file)?;
    let decl = find_type_decl_by_fqn(source, file, owner_fqn)?;

    find_member_type_ref_in_type_decl(source, decl, member_name)
}

fn find_member_type_ref_in_type_decl(
    source: &crate::source::SourceFile,
    decl: &ast::TypeDecl,
    member_name: &str,
) -> Option<ast::TypeRef> {
    if let Some(primary_ctor) = &decl.primary_ctor {
        for param in &primary_ctor.params {
            let name = source.slice(param.name.span);
            if name != member_name {
                continue;
            }
            let ctor_param_is_member = matches!(decl.kind, ast::TypeKind::Struct)
                || (matches!(decl.kind, ast::TypeKind::Class) && param.kind.is_some());
            if ctor_param_is_member {
                return param.ty.clone();
            }
        }
    }

    let body = decl.body.as_ref()?;
    for member in &body.members {
        let ast::TypeMember::Property(prop) = member else {
            continue;
        };
        let name = source.slice(prop.name.span);
        if name == member_name {
            return prop.ty.clone();
        }
    }

    None
}

fn find_type_decl_by_fqn<'a>(
    source: &crate::source::SourceFile,
    file: &'a ast::File,
    target_fqn: &str,
) -> Option<&'a ast::TypeDecl> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                if let Some(found) =
                    find_type_decl_in_type_decl(source, ty, &pkg_prefix, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::Item::Object(obj) => {
                if let Some(found) =
                    find_type_decl_in_object_decl(source, obj, &pkg_prefix, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }

    None
}

fn find_type_decl_in_type_decl<'a>(
    source: &crate::source::SourceFile,
    decl: &'a ast::TypeDecl,
    prefix: &str,
    target_fqn: &str,
) -> Option<&'a ast::TypeDecl> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if type_fqn == target_fqn {
        return Some(decl);
    }

    let body = decl.body.as_ref()?;
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                if let Some(found) =
                    find_type_decl_in_type_decl(source, nested, &type_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::TypeMember::Object(obj) => {
                if let Some(found) =
                    find_type_decl_in_object_decl(source, obj, &type_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            _ => {}
        }
    }

    None
}

fn find_type_decl_in_object_decl<'a>(
    source: &crate::source::SourceFile,
    obj: &'a ast::ObjectDecl,
    prefix: &str,
    target_fqn: &str,
) -> Option<&'a ast::TypeDecl> {
    let local_name = match (&obj.name, obj.kind) {
        (Some(name), _) => source.slice(name.span).to_string(),
        (None, ast::ObjectKind::Companion) => "Companion".to_string(),
        (None, ast::ObjectKind::Object) => return None,
    };

    let obj_fqn = if prefix.is_empty() {
        local_name
    } else {
        format!("{prefix}.{local_name}")
    };

    let body = obj.body.as_ref()?;
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                if let Some(found) =
                    find_type_decl_in_type_decl(source, nested, &obj_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::TypeMember::Object(nested) => {
                if let Some(found) =
                    find_type_decl_in_object_decl(source, nested, &obj_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            _ => {}
        }
    }

    None
}
