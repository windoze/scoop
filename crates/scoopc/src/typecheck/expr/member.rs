use std::collections::HashMap;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_utf8};
use crate::ty::{BuiltinTypes, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::infer::infer_expr_type;

use super::{ExprTypeError, FunSigOwned};

use super::super::assignable::is_type_assignable;
use super::super::lower::TypeLowering;

pub(super) fn infer_safe_member_access_expr_type(
    source: &SourceFile,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 receiver：保证其中的表达式也会被覆盖。
    let receiver_ty = infer_expr_type(
        source,
        receiver,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let inner_ty = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
        _ => {
            return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                found: lower.fmt_type(receiver_ty),
                span: receiver.span.into(),
            });
        }
    };

    // 当前阶段最小规则：
    // - 仅支持 safe-call 的字段访问：`receiver?.field`，并且 field 必须是 struct 字段（T0408）。
    // - extension property / method 的语义留给后续任务；safe-call 的“调用”形式在 `Call(SafeMemberAccess)`
    //   分支中处理。
    let field_ty = match member.resolved.as_ref() {
        Some(ast::ResolvedMemberRef::Value { fqn }) => struct_field_types
            .get(fqn)
            .copied()
            .ok_or_else(|| ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            })?,
        Some(ast::ResolvedMemberRef::Fun { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionValue { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => {
            return Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            });
        }
        None => {
            // resolver 无法静态确定 receiver 类型（例如 receiver 为 `T?`）时不会写回 resolved；
            // 这里用“已推导出的 inner_ty”尝试补上最小字段查找。
            let name = source.slice(member.span);
            match lower.type_kind(inner_ty) {
                TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    let fqn = format!("{}.{}", nominal.fqn, name);
                    struct_field_types.get(&fqn).copied().ok_or_else(|| {
                        ExprTypeError::UnsupportedMemberAccess {
                            fqn,
                            span: member.span.into(),
                        }
                    })?
                }
                other => {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: match other {
                            TypeKind::Value(_) => "safe member access（非 struct 字段）",
                            TypeKind::Ref(_) => "safe member access（引用类型成员尚未支持）",
                            TypeKind::Param(_) => "safe member access（type param 暂不支持）",
                        },
                        span: member.span.into(),
                    });
                }
            }
        }
    };

    Ok(lower.ty_option(field_ty))
}

pub(super) fn infer_elvis_expr_type(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = infer_expr_type(
        source,
        lhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let inner_ty = match lower.type_kind(lhs_ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
        _ => {
            return Err(ExprTypeError::ElvisLhsNotNullable {
                found: lower.fmt_type(lhs_ty),
                span: lhs.span.into(),
            });
        }
    };

    let rhs_ty = infer_expr_type(
        source,
        rhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if !is_type_assignable(rhs_ty, inner_ty, lower, builtins) {
        return Err(ExprTypeError::ElvisRhsTypeMismatch {
            expected: lower.fmt_type(inner_ty),
            found: lower.fmt_type(rhs_ty),
            span: rhs.span.into(),
        });
    }

    Ok(inner_ty)
}

pub(super) fn infer_not_null_assert_expr_type(
    source: &SourceFile,
    expr: &ast::Expr,
    op_span: Span,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // T0421a：最小规则：
    // - `x!!` 的操作数必须是 nullable（`T?` / `Option<T>`）
    // - 结果类型为去掉 nullable 后的 inner type：`Option<T>` → `T`
    //
    // T0421b：`x!!` 的失败语义要求 `Raise<RuntimeError>`（静态 required effects）。
    let ty = infer_expr_type(
        source,
        expr,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

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
    source: &SourceFile,
    receiver: &ast::Expr,
    field: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // splice 字段访问：`receiver.[field]`（spec §6.4）
    //
    // v0 语义（与 TODO T1204 的 fieldsOf v0 保持一致）：
    // - 当 `field` 为字符串字面量时：等价于普通成员访问 `receiver.<name>` 并返回该字段类型；
    // - 其它情况（例如未来的 FieldMeta / comptime for binder）：当前阶段先保守退化为 `Any`，
    //   留给后续 comptime 展开/元数据补齐后再做更精确的约束与推导。
    let receiver_ty = infer_expr_type(
        source,
        receiver,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // 仍然递归 typecheck `field`，保证其中的表达式错误不会被“跳过”吞掉。
    let _ = infer_expr_type(
        source,
        field,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let field_name: Option<String> = match &field.kind {
        ast::ExprKind::StringLit => {
            let raw = source.slice(field.span);
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
        return Ok(builtins.any);
    };

    let receiver_fqn = match lower.type_kind(receiver_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn.clone(),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => n.fqn.clone(),
        TypeKind::Param(_) => return Ok(builtins.any),
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
        return Ok(builtins.any);
    }

    struct_field_types.get(&field_fqn).copied().ok_or_else(|| {
        ExprTypeError::UnsupportedMemberAccess {
            fqn: field_fqn,
            span: field.span.into(),
        }
    })
}

pub(super) fn infer_member_access_expr_type(
    source: &SourceFile,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
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
        Some(infer_expr_type(
            source,
            receiver,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?)
    };

    match member.resolved.as_ref() {
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

            let member_name = source.slice(member.span);
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
            Ok(ty)
        }
        Some(ast::ResolvedMemberRef::Value { fqn }) => {
            // `TypeName.NestedObject` / `Obj.NestedObject`：成员本身是一个 object 单例值。
            if lower.is_object_type(fqn) {
                return Ok(lower.lower_type_fqn_with_args(fqn.clone(), Vec::new(), member.span)?);
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
                    && nominal.fqn == "scoop.core.Platform" {
                        return Ok(builtins.string);
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
                    && nominal.fqn == "scoop.core.Pinned" {
                        return Ok(builtins.any);
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
                    && nominal.fqn == "scoop.core.GcHandle" {
                        return Ok(lower.lower_type_fqn_with_args(
                            "scoop.core.UIntPtr".to_string(),
                            Vec::new(),
                            member.span,
                        )?);
                    }
            }

            struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })
        }
        Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => {
            // T0112：Extension property getter — look up the getter function's return type.
            if let Some(sigs) = top_level_funs.get(fqn.as_str())
                && let Some(sig) = sigs.first() {
                    return Ok(sig.return_ty);
                }
            Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            })
        }
        Some(
            ast::ResolvedMemberRef::Fun { fqn }
            | ast::ResolvedMemberRef::ExtensionFun { fqn },
        ) => Err(ExprTypeError::UnsupportedMemberAccess {
            fqn: fqn.clone(),
            span: member.span.into(),
        }),
    }
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
