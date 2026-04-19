use crate::ast;
use crate::resolve::Visibility;
use crate::span::Span;
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_utf8};
use crate::ty::{RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::infer::ExpectedTypeFrom;
use super::{ExprInferInputs, ExprTypeError};

use super::super::assignable::is_type_assignable;
use super::super::lower::TypeLowering;

struct MemberAccessInference {
    ty: TypeId,
    resolved: Option<ast::ResolvedMemberRef>,
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
    let resolved = if inputs.is_current_lambda_this_expr(receiver) {
        resolve_member_value_target_from_receiver_ty(inputs, inner_ty, member, lower)
    } else {
        member.resolved.clone().or_else(|| {
            resolve_member_value_target_from_receiver_ty(inputs, inner_ty, member, lower)
        })
    };
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
    receiver: &ast::Expr,
    field: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // splice 字段访问：`receiver.[field]`（spec §6.4）
    //
    // v0 语义（与 TODO T1204 的 fieldsOf v0 保持一致）：
    // - 当 `field` 为字符串字面量时：等价于普通成员访问 `receiver.<name>` 并返回该字段类型；
    // - 其它情况（例如未来的 FieldMeta / comptime for binder）：当前阶段先保守退化为 `Any`，
    //   留给后续 comptime 展开/元数据补齐后再做更精确的约束与推导。
    let receiver_ty = inputs.infer(lower, receiver)?;

    // 仍然递归 typecheck `field`，保证其中的表达式错误不会被“跳过”吞掉。
    let _ = inputs.infer(lower, field)?;

    let field_name: Option<String> = match &field.kind {
        ast::ExprKind::StringLit => {
            let raw = inputs.source.slice(field.span);
            match parse_string_literal_utf8(raw) {
                Ok(s) => Some(s),
                Err(StringLiteralParseError::Invalid)
                | Err(StringLiteralParseError::InvalidUtf8)
                | Err(StringLiteralParseError::Interpolated) => {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "splice field access（非法字符串字面量）",
                        span: field.span.into(),
                    });
                }
            }
        }
        _ => None,
    };

    let Some(field_name) = field_name else {
        return Ok(inputs.builtins.any);
    };

    let receiver_fqn = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn.clone(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => n.fqn.clone(),
        TypeKind::Param(_) => return Ok(inputs.builtins.any),
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "splice field access（receiver 必须为名义类型）",
                span: receiver.span.into(),
            });
        }
    };

    let field_fqn = format!("{receiver_fqn}.{field_name}");

    // sysroot 跨文件特判：Pinned.value（与 infer_member_access_expr_type 保持一致）。
    if field_fqn == "scoop.core.Pinned.value" {
        return Ok(inputs.builtins.any);
    }

    inputs
        .struct_field_types
        .get(&field_fqn)
        .copied()
        .ok_or_else(|| ExprTypeError::UnsupportedMemberAccess {
            fqn: field_fqn,
            span: field.span.into(),
        })
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
    let resolved = if inputs.is_current_lambda_this_expr(receiver) {
        receiver_ty
            .and_then(|ty| resolve_member_value_target_from_receiver_ty(inputs, ty, member, lower))
    } else {
        member.resolved.clone()
    };

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

fn infer_member_access_with_receiver_ty(
    inputs: ExprInferInputs<'_>,
    receiver_ty: Option<TypeId>,
    member: &ast::MemberIdent,
    resolved: Option<&ast::ResolvedMemberRef>,
    lower: &mut TypeLowering<'_>,
) -> Result<MemberAccessInference, ExprTypeError> {
    match resolved {
        None => {
            // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
            //
            // 说明：
            // - tuple 并非名义类型，因此 resolver 阶段无法像 `Point.x` 一样写回成员 FQN；
            // - 这里在 typecheck 阶段通过 receiver 的推导类型来支持最小 tuple 元素访问语义。
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
    inputs.struct_field_types.contains_key(direct_fqn)
        || lower.is_object_type(direct_fqn)
        || direct_fqn == "scoop.core.Pinned.value"
        || direct_fqn == "scoop.core.GcHandle.raw"
        || (receiver_ty_fqn == "scoop.core.Platform"
            && direct_fqn.starts_with("scoop.core.Platform."))
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
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok()
}
