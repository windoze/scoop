//! `val` 解构绑定（destructuring declaration）的最小类型检查（T0430）。
//!
//! 覆盖范围：
//! - tuple pattern：`val (a, b) = expr`（支持 `..` 忽略剩余元素）
//! - struct pattern：`val Point { x, y } = expr`（支持字段重命名与 `..`）
//! - enum variant pattern：`val Some(x) = expr` / `val Result.Ok(v) = expr`（T0460）
//!
//! 非目标：or-pattern / guard 等更完整 pattern 语义

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, TypeId, TypeKind, ValueTypeKind};

use super::expr::{lower_type_ref_with_enum_subst, ExprTypeError};
use super::lower::TypeLowering;

/// 对一个 `val` 解构 pattern 进行最小类型检查，并返回该 pattern 引入的局部绑定类型表。
///
/// 返回值：`decl_span -> TypeId`（decl_span 与 resolver 写回的 `ResolvedValueRef::Local` 对齐）。
pub(super) fn infer_val_pat_bindings(
    source: &SourceFile,
    pat: &ast::Pattern,
    init_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<HashMap<Span, TypeId>, ExprTypeError> {
    let mut bindings = HashMap::new();
    check_val_pat(
        source,
        pat,
        init_ty,
        lower,
        builtins,
        struct_field_types,
        &mut bindings,
    )?;
    Ok(bindings)
}

fn check_val_pat(
    source: &SourceFile,
    pat: &ast::Pattern,
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    struct_field_types: &HashMap<String, TypeId>,
    bindings: &mut HashMap<Span, TypeId>,
) -> Result<(), ExprTypeError> {
    match &pat.kind {
        ast::PatternKind::Wildcard | ast::PatternKind::Missing => Ok(()),
        // `..` 的“吞掉剩余元素/字段”语义由外层 tuple/struct pattern 处理；
        // 落到这里时仅代表它不引入绑定。
        ast::PatternKind::Rest => Ok(()),
        ast::PatternKind::Bind(id) => {
            bindings.insert(id.span, expected_ty);
            Ok(())
        }
        ast::PatternKind::Tuple(elements) => check_tuple_pat(
            source,
            pat,
            elements,
            expected_ty,
            lower,
            builtins,
            struct_field_types,
            bindings,
        ),
        ast::PatternKind::Struct { path, fields, rest } => check_struct_pat(
            source,
            pat,
            path,
            fields,
            *rest,
            expected_ty,
            lower,
            builtins,
            struct_field_types,
            bindings,
        ),
        ast::PatternKind::Variant { path, args } => check_variant_pat(
            source,
            pat,
            path,
            args,
            expected_ty,
            lower,
            builtins,
            struct_field_types,
            bindings,
        ),
    }
}

fn check_tuple_pat(
    source: &SourceFile,
    pat: &ast::Pattern,
    elements: &[ast::Pattern],
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    struct_field_types: &HashMap<String, TypeId>,
    bindings: &mut HashMap<Span, TypeId>,
) -> Result<(), ExprTypeError> {
    // `()`：允许匹配 Unit（0 元 tuple）。
    if elements.is_empty() && expected_ty == builtins.unit {
        return Ok(());
    }

    let expected_elements = match lower.type_kind(expected_ty) {
        TypeKind::Value(ValueTypeKind::Tuple(ts)) => ts,
        _ => {
            return Err(ExprTypeError::ValTuplePatNotTuple {
                found: lower.fmt_type(expected_ty),
                span: pat.span.into(),
            });
        }
    };

    // 解析 `..`：parser 已保证它最多出现一次且必须出现在最后一个位置。
    let (prefix_pats, has_rest) = match elements.last().map(|p| &p.kind) {
        Some(ast::PatternKind::Rest) => (&elements[..elements.len().saturating_sub(1)], true),
        _ => (elements, false),
    };

    if has_rest {
        if expected_elements.len() < prefix_pats.len() {
            return Err(ExprTypeError::ValTuplePatTooShort {
                expected_at_least: prefix_pats.len(),
                found: expected_elements.len(),
                span: pat.span.into(),
            });
        }

        for (p, ty) in prefix_pats.iter().zip(expected_elements.iter().copied()) {
            check_val_pat(
                source,
                p,
                ty,
                lower,
                builtins,
                struct_field_types,
                bindings,
            )?;
        }
        return Ok(());
    }

    if expected_elements.len() != elements.len() {
        return Err(ExprTypeError::ValTuplePatArityMismatch {
            expected: expected_elements.len(),
            found: elements.len(),
            span: pat.span.into(),
        });
    }

    for (p, ty) in elements.iter().zip(expected_elements.iter().copied()) {
        check_val_pat(
            source,
            p,
            ty,
            lower,
            builtins,
            struct_field_types,
            bindings,
        )?;
    }

    Ok(())
}

fn check_variant_pat(
    source: &SourceFile,
    pat: &ast::Pattern,
    path: &ast::TypePath,
    args: &[ast::Pattern],
    subject_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    struct_field_types: &HashMap<String, TypeId>,
    bindings: &mut HashMap<Span, TypeId>,
) -> Result<(), ExprTypeError> {
    // 1) subject 类型必须是 enum（含 builtin `Option<T>`），以复用 `when` 的 variant destructuring 语义。
    let (enum_fqn, enum_args, enum_source) =
        enum_instance_from_type(source, subject_ty, &*lower, pat.span)?;

    // 2) 若 pattern 写了 `Enum.Variant(...)` 形式，先校验 enum 前缀与 subject enum 一致（避免误写）。
    if path.segments.len() >= 2 {
        let prefix_segments = &path.segments[..path.segments.len() - 1];
        let start = prefix_segments.first().unwrap().span.start;
        let end = prefix_segments.last().unwrap().span.end;
        let prefix_span = Span::new(start, end);
        let prefix_path = ast::TypePath {
            span: prefix_span,
            segments: prefix_segments.to_vec(),
            args: Vec::new(),
        };

        let prefix_ty = lower.lower_type_ref(&ast::TypeRef::Path(prefix_path))?;
        let prefix_matches = match lower.type_kind(prefix_ty) {
            TypeKind::Value(ValueTypeKind::Option(_)) => enum_fqn == "scoop.core.Option",
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn == enum_fqn,
            _ => false,
        };
        if !prefix_matches {
            return Err(ExprTypeError::ValVariantPatEnumMismatch {
                expected: lower.fmt_type(subject_ty),
                found: lower.fmt_type(prefix_ty),
                span: prefix_span.into(),
            });
        }
    }

    // 3) 取 variant 名（最后一段），并在 enum 声明中查找匹配的 variant。
    let Some(variant_ident) = path.segments.last().copied() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "val variant pattern（空路径）",
            span: pat.span.into(),
        });
    };
    let variant_name = source.slice(variant_ident.span);

    let decl = lower.env().enum_decl(&enum_fqn).cloned().ok_or_else(|| {
        ExprTypeError::UnsupportedExpr {
            kind: "val variant pattern（缺少 enum 声明信息）",
            span: pat.span.into(),
        }
    })?;

    let variant = decl
        .variants
        .iter()
        .find(|v| v.name == variant_name)
        .cloned()
        .ok_or_else(|| ExprTypeError::ValVariantPatUnknownVariant {
            enum_fqn: enum_fqn.clone(),
            variant: variant_name.to_string(),
            span: variant_ident.span.into(),
        })?;

    let variant_fqn = format!("{enum_fqn}.{variant_name}");

    // 4) 参数数量检查：允许 `..` 作为“忽略剩余 payload”的占位符（parser 已保证最多一次且必须在末尾）。
    let (prefix_pats, has_rest) = match args.last().map(|p| &p.kind) {
        Some(ast::PatternKind::Rest) => (&args[..args.len().saturating_sub(1)], true),
        _ => (args, false),
    };

    let expected_arity = variant.fields.len();
    let found_arity = prefix_pats.len();
    if has_rest {
        if expected_arity < found_arity {
            return Err(ExprTypeError::ValVariantPatTooShort {
                variant_fqn,
                expected_at_least: found_arity,
                found: expected_arity,
                span: pat.span.into(),
            });
        }
    } else if expected_arity != found_arity {
        return Err(ExprTypeError::ValVariantPatArityMismatch {
            variant_fqn,
            expected: expected_arity,
            found: found_arity,
            span: pat.span.into(),
        });
    }

    // 5) 计算 enum type params → subject 实例化 type args 的替换表，并据此 lowering 每个 payload 字段的类型。
    if decl.type_params.len() != enum_args.len() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "val variant pattern（enum type args 数量异常）",
            span: pat.span.into(),
        });
    }

    let type_param_set: HashSet<&str> = decl.type_params.iter().map(|s| s.as_str()).collect();
    let subst: HashMap<String, TypeId> = decl
        .type_params
        .iter()
        .cloned()
        .zip(enum_args.into_iter())
        .collect();

    for (arg_pat, field) in prefix_pats.iter().zip(variant.fields.iter()) {
        let expected_field_ty = lower_type_ref_with_enum_subst(
            &enum_source,
            pat.span,
            &enum_fqn,
            &field.ty,
            lower,
            builtins,
            &type_param_set,
            &subst,
        )?;
        check_val_pat(
            source,
            arg_pat,
            expected_field_ty,
            lower,
            builtins,
            struct_field_types,
            bindings,
        )?;
    }

    Ok(())
}

fn check_struct_pat(
    source: &SourceFile,
    pat: &ast::Pattern,
    path: &ast::TypePath,
    fields: &[ast::StructPatternField],
    has_rest: Option<Span>,
    subject_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    struct_field_types: &HashMap<String, TypeId>,
    bindings: &mut HashMap<Span, TypeId>,
) -> Result<(), ExprTypeError> {
    // 1) 解析/确认 pattern 的 struct 类型。
    let pat_ty = lower.lower_type_ref(&ast::TypeRef::Path(path.clone()))?;
    let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(pat_ty) else {
        return Err(ExprTypeError::ValStructPatNotStruct {
            found: lower.fmt_type(pat_ty),
            span: path.span.into(),
        });
    };
    let struct_fqn = nominal.fqn.clone();

    if !matches!(
        lower.nominal_decl_kind(&struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::ValStructPatNotStruct {
            found: lower.fmt_type(pat_ty),
            span: path.span.into(),
        });
    }

    // 2) subject 类型必须匹配该 struct（允许 `Nothing` 作为 bottom）。
    if subject_ty != pat_ty && subject_ty != builtins.nothing {
        return Err(ExprTypeError::ValStructPatTypeMismatch {
            expected: lower.fmt_type(pat_ty),
            found: lower.fmt_type(subject_ty),
            span: pat.span.into(),
        });
    }

    // 3) 收集该 struct 的“直接字段”（不包含 nested type 的字段）。
    //
    // 说明：`collect_struct_field_types` 会为 nested struct 生成形如：
    //   `Outer.Inner.x`
    // 对于 `Outer { ... }` 的 struct pattern，我们只接受 `Outer.<field>` 这一层。
    let prefix = format!("{struct_fqn}.");
    let mut expected_fields: HashMap<String, TypeId> = HashMap::new();
    for (field_fqn, field_ty) in struct_field_types {
        let Some(rest) = field_fqn.strip_prefix(&prefix) else {
            continue;
        };
        if rest.contains('.') {
            continue;
        }
        expected_fields.insert(rest.to_string(), *field_ty);
    }

    // 4) 逐项检查字段：
    // - 字段名不可重复（即使重命名为不同 binder 也不允许重复字段）
    // - 字段必须存在于 struct 声明中（当前阶段仅支持当前文件内可查询的 struct）
    // - 递归检查字段 value pattern，并把 bind 模式注入局部类型表
    let mut seen_fields: HashMap<String, Span> = HashMap::new();
    let mut mentioned_fields: HashSet<String> = HashSet::new();
    for f in fields {
        let field_name = source.slice(f.name.span).to_string();

        if let Some(prev) = seen_fields.get(&field_name).copied() {
            return Err(ExprTypeError::ValStructPatDuplicateField {
                struct_name: struct_fqn.clone(),
                field: field_name,
                first: prev.into(),
                second: f.name.span.into(),
            });
        }
        seen_fields.insert(field_name.clone(), f.name.span);
        mentioned_fields.insert(field_name.clone());

        let Some(expected_field_ty) = expected_fields.get(&field_name).copied() else {
            return Err(ExprTypeError::ValStructPatUnknownField {
                struct_name: struct_fqn.clone(),
                field: field_name,
                span: f.name.span.into(),
            });
        };

        match &f.value {
            Some(v) => check_val_pat(
                source,
                v,
                expected_field_ty,
                lower,
                builtins,
                struct_field_types,
                bindings,
            )?,
            None => {
                // shorthand：`Point { x }` 等价于 `Point { x: x }`
                bindings.insert(f.name.span, expected_field_ty);
            }
        }
    }

    // 5) 若没有 `..`，要求覆盖 struct 的全部直接字段（与 struct literal 的早期规则保持一致）。
    if has_rest.is_none() {
        let mut missing: Vec<String> = expected_fields
            .keys()
            .filter(|name| !mentioned_fields.contains(*name))
            .cloned()
            .collect();
        missing.sort();
        if !missing.is_empty() {
            return Err(ExprTypeError::ValStructPatMissingFields {
                struct_name: struct_fqn,
                fields: missing.join(", "),
                span: pat.span.into(),
            });
        }
    }

    Ok(())
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
                return Err(ExprTypeError::ValVariantPatNotEnum {
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
        _ => Err(ExprTypeError::ValVariantPatNotEnum {
            found: lower.fmt_type(ty),
            span: use_span.into(),
        }),
    }
}
