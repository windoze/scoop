//! Enum-variant constructor call inference.

#![allow(dead_code)]

use super::*;

pub(super) fn lookup_enum_variant_decl_data(
    source: &SourceFile,
    lower: &TypeLowering<'_>,
    enum_fqn: &str,
    variant_name: &str,
) -> Option<(Vec<String>, SourceFile, EnumVariantInfo)> {
    let decl = lower.env().enum_decl(enum_fqn)?;
    let type_params = decl.type_params.clone();
    let enum_source = lower
        .env()
        .source(&decl.decl_file)
        .cloned()
        .unwrap_or_else(|| source.clone());
    let variant = decl
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?
        .clone();
    Some((type_params, enum_source, variant))
}

pub(super) fn resolved_qualified_enum_variant_value_fqn(
    source: &SourceFile,
    member: &ast::MemberIdent,
    lower: &TypeLowering<'_>,
) -> Option<(String, String)> {
    let ast::ResolvedMemberRef::Value { fqn } = member.resolved.as_ref()? else {
        return None;
    };
    let (enum_fqn, variant_name) = fqn.rsplit_once('.')?;
    lookup_enum_variant_decl_data(source, lower, enum_fqn, variant_name)?;
    Some((enum_fqn.to_string(), variant_name.to_string()))
}

pub(super) fn infer_specific_enum_variant_ctor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    target: EnumVariantCtorTarget<'_>,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let EnumVariantCtorTarget {
        enum_fqn,
        variant_name,
        callee_span,
    } = target;
    let Some((type_params, enum_source, variant)) =
        lookup_enum_variant_decl_data(source, lower, enum_fqn, variant_name)
    else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "enum variant ctor（缺少 enum 声明信息）",
            span: call_expr.span.into(),
        });
    };

    let variant_fqn = format!("{enum_fqn}.{variant_name}");
    let expected = variant.fields.len();
    let found = args.len();
    if expected != found {
        return Err(ExprTypeError::EnumVariantCtorArityMismatch {
            variant: variant_fqn,
            expected,
            found,
            span: call_expr.span.into(),
        });
    }

    let mut arg_types: Vec<TypeId> = Vec::with_capacity(args.len());
    for arg in args {
        arg_types.push(inputs.infer(lower, arg)?);
    }

    let type_param_set: HashSet<String> = type_params.iter().cloned().collect();
    let mut subst: HashMap<String, TypeId> = HashMap::new();
    for (idx, (field, found_ty)) in variant
        .fields
        .iter()
        .zip(arg_types.iter().copied())
        .enumerate()
    {
        let ast::TypeRef::Path(p) = &field.ty else {
            continue;
        };
        if !p.args.is_empty() || p.segments.len() != 1 {
            continue;
        }
        let name = enum_source.slice(p.segments[0].span);
        if !type_param_set.contains(name) {
            continue;
        }

        match subst.get(name).copied() {
            None => {
                subst.insert(name.to_string(), found_ty);
            }
            Some(prev) if prev == found_ty => {}
            Some(prev) if prev == builtins.nothing => {
                subst.insert(name.to_string(), found_ty);
            }
            Some(_prev) if found_ty == builtins.nothing => {}
            Some(prev) => {
                return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                    variant: format!("{enum_fqn}.{variant_name}"),
                    index: idx + 1,
                    expected: lower.fmt_type(prev),
                    found: lower.fmt_type(found_ty),
                    span: args[idx].span.into(),
                });
            }
        }
    }

    for (idx, (field, found_ty)) in variant
        .fields
        .iter()
        .zip(arg_types.iter().copied())
        .enumerate()
    {
        let expected_ty = lower_type_ref_with_enum_subst(
            EnumTypeSubstContext {
                decl_file: enum_source.path(),
                enum_source: &enum_source,
                use_span: call_expr.span,
                enum_fqn,
                builtins,
                type_param_set: &type_param_set,
                subst: &subst,
            },
            &field.ty,
            lower,
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                variant: format!("{enum_fqn}.{variant_name}"),
                index: idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: args[idx].span.into(),
            });
        }
    }

    let mut enum_args: Vec<TypeId> = Vec::with_capacity(type_params.len());
    for name in &type_params {
        let Some(id) = subst.get(name).copied() else {
            return Err(ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                enum_fqn: enum_fqn.to_string(),
                param: name.clone(),
                span: callee_span.into(),
            });
        };
        enum_args.push(id);
    }

    Ok(lower.lower_type_fqn_with_args(enum_fqn.to_string(), enum_args, call_expr.span)?)
}

pub(super) fn infer_enum_variant_ctor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let source = inputs.source;
    let variant_name = source.slice(callee.span);
    let candidates = lower
        .env()
        .find_visible_enum_variants_named(variant_name, source);

    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates.len() != 1 {
        let mut names: Vec<String> = candidates
            .iter()
            .map(|(enum_fqn, _)| format!("{enum_fqn}.{variant_name}"))
            .collect();
        names.sort();
        names.dedup();

        return Err(ExprTypeError::AmbiguousEnumVariantCtor {
            name: variant_name.to_string(),
            candidates: names.join(" | "),
            span: callee.span.into(),
        });
    }

    let (enum_fqn, variant) = candidates.into_iter().next().expect("len == 1");
    Ok(Some(infer_specific_enum_variant_ctor_call_expr_type(
        inputs,
        call_expr,
        EnumVariantCtorTarget {
            enum_fqn: &enum_fqn,
            variant_name: &variant.name,
            callee_span: callee.span,
        },
        args,
        lower,
    )?))
}

pub(in crate::typecheck::expr) fn try_infer_qualified_enum_variant_ctor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some((enum_fqn, variant_name)) =
        resolved_qualified_enum_variant_value_fqn(inputs.source, member, lower)
    else {
        return Ok(None);
    };

    Ok(Some(infer_specific_enum_variant_ctor_call_expr_type(
        inputs,
        call_expr,
        EnumVariantCtorTarget {
            enum_fqn: &enum_fqn,
            variant_name: &variant_name,
            callee_span: member.span,
        },
        args,
        lower,
    )?))
}

pub(in crate::typecheck::expr) fn infer_specific_enum_variant_ctor_call_expr_type_by_expected(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    target: EnumVariantCtorTarget<'_>,
    args: &[ast::Expr],
    expected_enum_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let EnumVariantCtorTarget {
        enum_fqn,
        variant_name,
        callee_span,
    } = target;
    let Some((type_params, enum_source, variant)) =
        lookup_enum_variant_decl_data(source, lower, enum_fqn, variant_name)
    else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "enum variant ctor（缺少 enum 声明信息）",
            span: call_expr.span.into(),
        });
    };

    if type_params.len() != expected_enum_args.len() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "enum variant ctor（expected enum type args 数量异常）",
            span: call_expr.span.into(),
        });
    }

    let variant_fqn = format!("{enum_fqn}.{variant_name}");
    let expected_arity = variant.fields.len();
    let found_arity = args.len();
    if expected_arity != found_arity {
        return Err(ExprTypeError::EnumVariantCtorArityMismatch {
            variant: variant_fqn,
            expected: expected_arity,
            found: found_arity,
            span: call_expr.span.into(),
        });
    }

    let type_param_set: HashSet<String> = type_params.iter().cloned().collect();
    let subst: HashMap<String, TypeId> = type_params
        .iter()
        .cloned()
        .zip(expected_enum_args.iter().copied())
        .collect();

    for (idx, (field, arg_expr)) in variant.fields.iter().zip(args.iter()).enumerate() {
        let expected_field_ty = lower_type_ref_with_enum_subst(
            EnumTypeSubstContext {
                decl_file: enum_source.path(),
                enum_source: &enum_source,
                use_span: call_expr.span,
                enum_fqn,
                builtins,
                type_param_set: &type_param_set,
                subst: &subst,
            },
            &field.ty,
            lower,
        )?;

        let found_ty = inputs.infer_in_expected(
            lower,
            arg_expr,
            expected_field_ty,
            ExpectedTypeFrom::new(format!(
                "enum variant `{enum_fqn}.{variant_name}` 第 {} 个参数",
                idx + 1
            )),
        )?;

        if !is_type_assignable(found_ty, expected_field_ty, lower, builtins)
            && !literal_absorbs_to_expected(arg_expr, expected_field_ty, source, lower, builtins)
        {
            return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                variant: format!("{enum_fqn}.{variant_name}"),
                index: idx + 1,
                expected: lower.fmt_type(expected_field_ty),
                found: lower.fmt_type(found_ty),
                span: arg_expr.span.into(),
            });
        }
    }

    let mut enum_args: Vec<TypeId> = Vec::with_capacity(type_params.len());
    for name in &type_params {
        let Some(id) = subst.get(name).copied() else {
            return Err(ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                enum_fqn: enum_fqn.to_string(),
                param: name.clone(),
                span: callee_span.into(),
            });
        };
        enum_args.push(id);
    }

    Ok(lower.lower_type_fqn_with_args(enum_fqn.to_string(), enum_args, call_expr.span)?)
}

pub(in crate::typecheck) fn lower_type_ref_with_enum_subst(
    ctx: EnumTypeSubstContext<'_>,
    ty: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    lower.with_decl_file_context(ctx.decl_file, |lower| match ty {
        ast::TypeRef::Path(p) => {
            // 单段名且无 type args：可能是对 enum type param 的引用（例如 `T`）。
            if p.segments.len() == 1 && p.args.is_empty() {
                let name = ctx.enum_source.slice(p.segments[0].span);
                if ctx.type_param_set.contains(name) {
                    return ctx.subst.get(name).copied().ok_or_else(|| {
                        ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                            enum_fqn: ctx.enum_fqn.to_string(),
                            param: name.to_string(),
                            span: ctx.use_span.into(),
                        }
                    });
                }
            }

            let segments: Vec<String> = p
                .segments
                .iter()
                .map(|id| ctx.enum_source.slice(id.span).to_string())
                .collect();

            let fqn = match lower.resolve_type_path_fqn_by_name(&segments, ctx.use_span) {
                Ok(fqn) => fqn,
                Err(TypeLowerError::UnresolvedType { name, span }) => {
                    let Some(builtin_fqn) = implicit_builtin_type_fqn(&name) else {
                        return Err(TypeLowerError::UnresolvedType { name, span }.into());
                    };
                    builtin_fqn.to_string()
                }
                Err(other) => return Err(other.into()),
            };

            let mut eff_arg: Option<EffectRow> = None;
            let mut args: Vec<TypeId> = Vec::with_capacity(p.args.len());
            for a in &p.args {
                match a {
                    ast::TypeRef::EffectRowArg { row, .. } => {
                        if eff_arg.is_none() {
                            eff_arg = Some(lower.lower_effect_row_expr(Some(row))?);
                        }
                    }
                    _ => args.push(lower_type_ref_with_enum_subst(ctx, a, lower)?),
                }
            }

            Ok(lower.lower_type_fqn_with_args_and_eff(fqn, args, eff_arg, ctx.use_span)?)
        }
        ast::TypeRef::Tuple(t) => {
            if t.elements.is_empty() {
                return Ok(ctx.builtins.unit);
            }
            let mut elements: Vec<TypeId> = Vec::with_capacity(t.elements.len());
            for e in &t.elements {
                elements.push(lower_type_ref_with_enum_subst(ctx, e, lower)?);
            }
            Ok(lower.ty_tuple(elements))
        }
        ast::TypeRef::Nullable { inner, .. } => {
            let inner = lower_type_ref_with_enum_subst(ctx, inner, lower)?;
            Ok(lower.ty_option(inner))
        }
        ast::TypeRef::Star { .. } => Ok(lower.ty_star_projection()),
        ast::TypeRef::EffectRowArg { .. } => Err(TypeLowerError::UnsupportedTypeRef {
            kind: "use-site effect row arg (`eff ...`)",
            span: ctx.use_span.into(),
        }
        .into()),
        ast::TypeRef::Function(f) => {
            let receiver = match &f.receiver {
                Some(r) => Some(lower_type_ref_with_enum_subst(ctx, r, lower)?),
                None => None,
            };

            let mut params = Vec::with_capacity(f.params.len());
            for p in &f.params {
                params.push(lower_type_ref_with_enum_subst(ctx, p, lower)?);
            }

            let return_ty = lower_type_ref_with_enum_subst(ctx, &f.return_ty, lower)?;

            let effects = match &f.effects {
                None => EffectRow::pure(),
                Some(e) if e.terms.is_empty() => EffectRow::pure(),
                Some(e) => {
                    let mut terms: Vec<TypeId> = Vec::with_capacity(e.terms.len());
                    for term in &e.terms {
                        let term_ref = ast::TypeRef::Path(term.clone());
                        let ty = lower_type_ref_with_enum_subst(ctx, &term_ref, lower)?;

                        let ok = match lower.type_kind(ty) {
                            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => matches!(
                                lower.nominal_decl_kind(&nominal.fqn),
                                Some(ast::TypeKind::Effect)
                            ),
                            _ => false,
                        };
                        if !ok {
                            return Err(TypeLowerError::EffectRowItemNotEffect {
                                item: ctx.enum_source.slice(term.span).to_string(),
                                found: lower.fmt_type(ty),
                                span: term.span.into(),
                            }
                            .into());
                        }

                        terms.push(ty);
                    }
                    EffectRow::new(terms)
                }
            };

            let effects_closed = f.effects.as_ref().is_some_and(|r| r.closed);
            Ok(lower.ty_function(receiver, params, return_ty, effects, effects_closed))
        }
    })
}

