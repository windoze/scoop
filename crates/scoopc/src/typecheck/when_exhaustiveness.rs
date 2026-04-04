//! `when` 分支的穷尽性（exhaustiveness）检查（T0459）。
//!
//! 当前阶段目标：
//! - 将穷尽性从“单层 enum/Bool/Option”扩展到嵌套组合：
//!   - tuple 的多维组合（例如 `(Option<T>, Bool)`）；
//!   - enum/Option payload 中再包含 enum/Bool/Option/tuple 的组合。
//! - 仅覆盖“可枚举的构造器组合”（sum/product of finite constructors），
//!   不做无限域枚举与路径敏感分析。
//!
//! 设计要点：
//! - 通过“有限例子集合（example set）”来近似完整值域：
//!   - 对 Bool：例子为 `true`/`false`；
//!   - 对 enum/Option：例子为每个 variant（payload 递归展开）；
//!   - 对 tuple：例子为元素例子的笛卡尔积；
//!   - 对其他/不可枚举类型：例子为 `_`（表示“任意值”）。
//! - 对于每个例子，我们要求存在至少一个无 guard 的 arm pattern 能覆盖该例子；
//!   否则报告缺失的组合（以 pattern 语法形式呈现）。

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, TypeId, TypeKind, ValueTypeKind};

use super::expr::{ExprTypeError, lower_type_ref_with_enum_subst};
use super::lower::TypeLowering;
use tracing::warn;

/// 例子模式（example pattern）：用于描述“仍未被覆盖的一类值”。
///
/// - `Wildcard` 表示“该位置可以是任意值”（通常来自不可枚举/递归裁剪的类型）。
/// - 其它分支尽量保持与源码 pattern 语法一致，方便在诊断中呈现。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExamplePat {
    Wildcard,
    Bool(bool),
    Tuple(Vec<ExamplePat>),
    Variant { name: String, args: Vec<ExamplePat> },
}

impl ExamplePat {
    fn to_syntax(&self) -> String {
        match self {
            ExamplePat::Wildcard => "_".to_string(),
            ExamplePat::Bool(true) => "true".to_string(),
            ExamplePat::Bool(false) => "false".to_string(),
            ExamplePat::Tuple(elements) => {
                let inner = elements
                    .iter()
                    .map(ExamplePat::to_syntax)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({inner})")
            }
            ExamplePat::Variant { name, args } => {
                if args.is_empty() {
                    return name.clone();
                }
                let inner = args
                    .iter()
                    .map(ExamplePat::to_syntax)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}({inner})")
            }
        }
    }
}

pub(super) fn check_when_exhaustiveness(
    source: &SourceFile,
    when_expr: &ast::Expr,
    subject_ty: TypeId,
    arms: &[ast::WhenArm],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    // 带 guard 的分支（`pat if cond -> ...`）在穷尽性检查中视为“不可覆盖”：
    // - 它们不计入覆盖集合；
    // - 也不应被视为 catch-all（因为 guard 可能为 false）。
    let arm_pats: Vec<&ast::WhenPat> = arms
        .iter()
        .filter(|arm| arm.guard.is_none())
        .map(|arm| &arm.pat)
        .collect();

    let has_catch_all = arm_pats.iter().any(|pat| pat_is_catch_all(pat));
    let has_else_keyword = arm_pats.iter().any(|pat| pat_contains_else_keyword(pat));

    // 对无法分析构造器集合的 subject 类型，保持原有规则：必须有 `else/_/bind` 兜底。
    if !is_analyzable_subject_type(subject_ty, lower, builtins) {
        if has_catch_all {
            return Ok(());
        }
        return Err(ExprTypeError::WhenMissingElse {
            subject: lower.fmt_type(subject_ty),
            span: when_expr.span.into(),
        });
    }

    // 生成 subject 的“有限例子集合”，并检查每个例子是否被至少一个 arm 覆盖。
    let mut visiting = HashSet::new();
    let examples = examples_for_type(
        source,
        subject_ty,
        lower,
        builtins,
        when_expr.span,
        &mut visiting,
    )?;

    let mut missing: Vec<ExamplePat> = Vec::new();
    'examples: for ex in examples {
        for pat in &arm_pats {
            if pat_covers_example(source, pat, &ex) {
                continue 'examples;
            }
        }
        missing.push(ex);
    }

    if missing.is_empty() {
        // 仅在“无需 else 也已穷尽”的情况下提示 else 冗余。
        if has_else_keyword {
            let without_else: Vec<&ast::WhenPat> = arm_pats
                .iter()
                .copied()
                .filter(|p| !matches!(p, ast::WhenPat::Else { .. }))
                .collect();
            if !without_else.is_empty() {
                let mut visiting = HashSet::new();
                let examples = examples_for_type(
                    source,
                    subject_ty,
                    lower,
                    builtins,
                    when_expr.span,
                    &mut visiting,
                )?;
                let redundant = examples.into_iter().all(|ex| {
                    without_else
                        .iter()
                        .any(|pat| pat_covers_example(source, pat, &ex))
                });
                if redundant {
                    warn!("`when` 已经穷尽；`else` 分支是冗余的");
                }
            }
        }
        return Ok(());
    }

    // 兜底分支存在时一定能覆盖所有例子，因此 missing 不应出现；这里做防御。
    if has_catch_all {
        return Ok(());
    }

    let subject = subject_display_name(subject_ty, lower);
    let mut missing_texts: Vec<String> = missing.into_iter().map(|m| m.to_syntax()).collect();
    missing_texts.sort();

    Err(ExprTypeError::WhenNonExhaustiveMissingVariants {
        subject,
        missing: missing_texts.join(", "),
        span: when_expr.span.into(),
    })
}

fn is_analyzable_subject_type(
    subject_ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if subject_ty == builtins.bool_ || subject_ty == builtins.unit {
        return true;
    }

    match lower.type_kind(subject_ty) {
        TypeKind::Value(ValueTypeKind::Option(_)) => true,
        TypeKind::Value(ValueTypeKind::Tuple(_)) => true,
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if matches!(
                lower.nominal_decl_kind(&nominal.fqn),
                Some(ast::TypeKind::Enum)
            ) =>
        {
            true
        }
        _ => false,
    }
}

fn subject_display_name(subject_ty: TypeId, lower: &TypeLowering<'_>) -> String {
    match lower.type_kind(subject_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if matches!(
                lower.nominal_decl_kind(&nominal.fqn),
                Some(ast::TypeKind::Enum)
            ) =>
        {
            nominal.fqn.clone()
        }
        _ => lower.fmt_type(subject_ty),
    }
}

const MAX_EXAMPLES: usize = 256;

fn examples_for_type(
    source: &SourceFile,
    ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    use_span: Span,
    visiting: &mut HashSet<TypeId>,
) -> Result<Vec<ExamplePat>, ExprTypeError> {
    // 递归/爆炸防线：遇到环或规模过大时，退化为 `_`（不可枚举）。
    if !visiting.insert(ty) {
        return Ok(vec![ExamplePat::Wildcard]);
    }

    let mut out = match () {
        _ if ty == builtins.unit => vec![ExamplePat::Tuple(Vec::new())],
        _ if ty == builtins.bool_ => vec![ExamplePat::Bool(true), ExamplePat::Bool(false)],
        _ => match lower.type_kind(ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                let mut per_elem: Vec<Vec<ExamplePat>> = Vec::with_capacity(elements.len());
                for t in elements.iter().copied() {
                    per_elem.push(examples_for_type(
                        source, t, lower, builtins, use_span, visiting,
                    )?);
                }

                match cross_product_checked(&per_elem) {
                    Some(combos) => combos.into_iter().map(ExamplePat::Tuple).collect(),
                    None => vec![ExamplePat::Wildcard],
                }
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                let inner_examples =
                    examples_for_type(source, inner, lower, builtins, use_span, visiting)?;
                let mut out = Vec::with_capacity(1 + inner_examples.len());
                out.push(ExamplePat::Variant {
                    name: "None".to_string(),
                    args: Vec::new(),
                });
                for ex in inner_examples {
                    out.push(ExamplePat::Variant {
                        name: "Some".to_string(),
                        args: vec![ex],
                    });
                }
                out
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if matches!(
                    lower.nominal_decl_kind(&nominal.fqn),
                    Some(ast::TypeKind::Enum)
                ) =>
            {
                let enum_fqn = nominal.fqn.clone();
                let enum_args = nominal.args.clone();
                let decl = lower.env().enum_decl(&enum_fqn).cloned().ok_or_else(|| {
                    ExprTypeError::UnsupportedExpr {
                        kind: "when exhaustiveness（缺少 enum 声明信息）",
                        span: use_span.into(),
                    }
                })?;

                // 将 enum 声明处的 type params 映射到当前实例化的 type args。
                if decl.type_params.len() != enum_args.len() {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "when exhaustiveness（enum type args 数量异常）",
                        span: use_span.into(),
                    });
                }

                let enum_source = lower
                    .env()
                    .source(&decl.decl_file)
                    .cloned()
                    .unwrap_or_else(|| source.clone());

                let type_param_set: HashSet<&str> =
                    decl.type_params.iter().map(|s| s.as_str()).collect();
                let subst: HashMap<String, TypeId> = decl
                    .type_params
                    .iter()
                    .cloned()
                    .zip(enum_args.into_iter())
                    .collect();

                let mut out = Vec::new();
                for variant in &decl.variants {
                    let mut field_examples: Vec<Vec<ExamplePat>> =
                        Vec::with_capacity(variant.fields.len());
                    for field in &variant.fields {
                        let field_ty = lower_type_ref_with_enum_subst(
                            &enum_source,
                            use_span,
                            &enum_fqn,
                            &field.ty,
                            lower,
                            builtins,
                            &type_param_set,
                            &subst,
                        )?;
                        field_examples.push(examples_for_type(
                            source, field_ty, lower, builtins, use_span, visiting,
                        )?);
                    }

                    match cross_product_checked(&field_examples) {
                        Some(args_combos) => {
                            for args in args_combos {
                                out.push(ExamplePat::Variant {
                                    name: variant.name.clone(),
                                    args,
                                });
                            }
                        }
                        None => {
                            // variant payload 组合过多：退化为 “该 variant + 全 wildcard payload”。
                            out.push(ExamplePat::Variant {
                                name: variant.name.clone(),
                                args: vec![ExamplePat::Wildcard; variant.fields.len()],
                            });
                        }
                    }

                    if out.len() > MAX_EXAMPLES {
                        out.clear();
                        out.push(ExamplePat::Wildcard);
                        break;
                    }
                }
                out
            }
            _ => vec![ExamplePat::Wildcard],
        },
    };

    visiting.remove(&ty);

    // 全局爆炸保护：超过阈值则退化为 `_`。
    if out.len() > MAX_EXAMPLES {
        out.clear();
        out.push(ExamplePat::Wildcard);
    }

    Ok(out)
}

fn cross_product_checked(lists: &[Vec<ExamplePat>]) -> Option<Vec<Vec<ExamplePat>>> {
    let mut acc: Vec<Vec<ExamplePat>> = vec![Vec::new()];
    for list in lists {
        if list.is_empty() {
            return Some(Vec::new());
        }

        let mut next: Vec<Vec<ExamplePat>> = Vec::new();
        for prefix in &acc {
            for item in list {
                let mut combined = prefix.clone();
                combined.push(item.clone());
                next.push(combined);
                if next.len() > MAX_EXAMPLES {
                    return None;
                }
            }
        }
        acc = next;
    }
    Some(acc)
}

fn pat_is_catch_all(pat: &ast::WhenPat) -> bool {
    match pat {
        ast::WhenPat::Else { .. } | ast::WhenPat::Wildcard { .. } | ast::WhenPat::Bind { .. } => {
            true
        }
        ast::WhenPat::Or { pats, .. } => pats.iter().any(pat_is_catch_all),
        _ => false,
    }
}

fn pat_contains_else_keyword(pat: &ast::WhenPat) -> bool {
    match pat {
        ast::WhenPat::Else { .. } => true,
        ast::WhenPat::Or { pats, .. } => pats.iter().any(pat_contains_else_keyword),
        _ => false,
    }
}

fn pat_covers_example(source: &SourceFile, pat: &ast::WhenPat, ex: &ExamplePat) -> bool {
    match pat {
        ast::WhenPat::Or { pats, .. } => pats.iter().any(|p| pat_covers_example(source, p, ex)),
        ast::WhenPat::Else { .. }
        | ast::WhenPat::Wildcard { .. }
        | ast::WhenPat::Bind { .. }
        // `..` 在语法上不应独立出现，但当作“任意”处理能避免穷尽性误判为缺失。
        | ast::WhenPat::Rest { .. } => true,

        ast::WhenPat::BoolLit { span } => match ex {
            ExamplePat::Bool(v) => match source.slice(*span) {
                "true" => *v,
                "false" => !*v,
                _ => false,
            },
            ExamplePat::Wildcard => false,
            _ => false,
        },

        // 当前阶段不把 `is T` / 字面量（除 Bool）计入“可保证覆盖”的集合：
        // 它们只覆盖值域的一部分，因此不用于证明穷尽。
        ast::WhenPat::Is { .. }
        | ast::WhenPat::IntLit { .. }
        | ast::WhenPat::StringLit { .. } => false,

        ast::WhenPat::Tuple { elements, .. } => match ex {
            ExamplePat::Tuple(ex_elems) => tuple_pat_covers_example(source, elements, ex_elems),
            _ => false,
        },

        ast::WhenPat::Variant { name, args, .. } => match ex {
            ExamplePat::Variant {
                name: ex_name,
                args: ex_args,
            } => {
                if source.slice(name.span) != ex_name {
                    return false;
                }
                variant_pat_covers_example(source, args, ex_args)
            }
            _ => false,
        },
    }
}

fn tuple_pat_covers_example(
    source: &SourceFile,
    pat_elems: &[ast::WhenPat],
    ex_elems: &[ExamplePat],
) -> bool {
    // 解析 `..`：parser 已保证它最多出现一次且必须出现在最后一个位置。
    let (prefix_pats, has_rest) = match pat_elems.last() {
        Some(ast::WhenPat::Rest { .. }) => (&pat_elems[..pat_elems.len().saturating_sub(1)], true),
        _ => (pat_elems, false),
    };

    if has_rest {
        if prefix_pats.len() > ex_elems.len() {
            return false;
        }
        for (p, ex) in prefix_pats.iter().zip(ex_elems.iter()) {
            if !pat_covers_example(source, p, ex) {
                return false;
            }
        }
        return true;
    }

    if prefix_pats.len() != ex_elems.len() {
        return false;
    }
    for (p, ex) in prefix_pats.iter().zip(ex_elems.iter()) {
        if !pat_covers_example(source, p, ex) {
            return false;
        }
    }
    true
}

fn variant_pat_covers_example(
    source: &SourceFile,
    pat_args: &[ast::WhenPat],
    ex_args: &[ExamplePat],
) -> bool {
    // 解析 `..`：parser 已保证它最多出现一次且必须出现在最后一个位置。
    let (prefix_pats, has_rest) = match pat_args.last() {
        Some(ast::WhenPat::Rest { .. }) => (&pat_args[..pat_args.len().saturating_sub(1)], true),
        _ => (pat_args, false),
    };

    if has_rest {
        if prefix_pats.len() > ex_args.len() {
            return false;
        }
        for (p, ex) in prefix_pats.iter().zip(ex_args.iter()) {
            if !pat_covers_example(source, p, ex) {
                return false;
            }
        }
        return true;
    }

    if prefix_pats.len() != ex_args.len() {
        return false;
    }
    for (p, ex) in prefix_pats.iter().zip(ex_args.iter()) {
        if !pat_covers_example(source, p, ex) {
            return false;
        }
    }
    true
}
