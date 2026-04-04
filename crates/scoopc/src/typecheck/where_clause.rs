//! `where` 子句语义检查（T0458）。
//!
//! 目标（最小可回归子集）：
//! - 约束目标必须是**当前声明**的 type param（不允许“借用外层 type param”）。
//! - 诊断重复约束与“多重 class-like 上界”冲突。
//! - 为满足性检查（type instantiation 时）提供干净的输入：拒绝暂不支持的 bound 形式。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::lower::{TypeLowerError, TypeLowering};

#[derive(Debug, Error, Diagnostic)]
pub enum WhereClauseError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error("where 约束目标必须是当前声明的类型参数：{param}")]
    #[diagnostic(code(scoop::typecheck::where_target_not_in_current_decl))]
    TargetNotInCurrentDecl {
        param: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("where 约束的 bound 暂不支持携带类型实参：{bound}")]
    #[diagnostic(code(scoop::typecheck::where_bound_generic_not_supported))]
    GenericBoundNotSupported {
        bound: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("重复的 where 约束：{param} : {bound}")]
    #[diagnostic(code(scoop::typecheck::duplicate_where_constraint))]
    DuplicateWhereConstraint {
        param: String,
        bound: String,
        #[label("第一次约束在这里")]
        first: miette::SourceSpan,
        #[label("重复约束在这里")]
        second: miette::SourceSpan,
    },

    #[error("where 约束冲突：{param} 不能同时满足 {a} 与 {b}")]
    #[diagnostic(code(scoop::typecheck::conflicting_where_constraints))]
    ConflictingWhereConstraints {
        param: String,
        a: String,
        b: String,
        #[label("第一个约束在这里")]
        first: miette::SourceSpan,
        #[label("第二个约束在这里")]
        second: miette::SourceSpan,
    },
}

pub fn check_file_where_clauses(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &super::TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), WhereClauseError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);

    for item in &file.items {
        match item {
            ast::Item::Fun(fun) => check_fun_where_clause(source, fun, &mut lower, builtins)?,
            ast::Item::Type(ty) => check_type_decl_where_clause(source, ty, &mut lower, builtins)?,
            ast::Item::Object(obj) => {
                check_object_decl_where_clauses(source, obj, &mut lower, builtins)?
            }
            ast::Item::TypeAlias(_) | ast::Item::Val(_) | ast::Item::ExtensionProperty(_) => {}
        }
    }

    Ok(())
}

fn check_fun_where_clause(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), WhereClauseError> {
    let Some(w) = &fun.where_clause else {
        return Ok(());
    };

    let declared = collect_type_param_names(source, &fun.type_params);

    // `where` 的 bound lowering 允许看到该 fun 的 type params（用于识别/诊断 `T` 这类引用）。
    lower.push_type_params(&fun.type_params);
    let result = check_one_where_clause(source, w, &declared, lower, builtins);
    lower.pop_type_params(&fun.type_params);
    result
}

fn check_type_decl_where_clause(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), WhereClauseError> {
    lower.push_type_params(&decl.type_params);

    let result = (|| {
        if let Some(w) = &decl.where_clause {
            let declared = collect_type_param_names(source, &decl.type_params);
            check_one_where_clause(source, w, &declared, lower, builtins)?;
        }

        let Some(body) = &decl.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => check_fun_where_clause(source, fun, lower, builtins)?,
                ast::TypeMember::Type(nested) => {
                    check_type_decl_where_clause(source, nested, lower, builtins)?
                }
                ast::TypeMember::Object(obj) => {
                    check_object_decl_where_clauses(source, obj, lower, builtins)?
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }

        Ok(())
    })();

    lower.pop_type_params(&decl.type_params);
    result
}

fn check_object_decl_where_clauses(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), WhereClauseError> {
    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => check_fun_where_clause(source, fun, lower, builtins)?,
            ast::TypeMember::Type(nested) => {
                check_type_decl_where_clause(source, nested, lower, builtins)?
            }
            ast::TypeMember::Object(nested) => {
                check_object_decl_where_clauses(source, nested, lower, builtins)?
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }

    Ok(())
}

fn check_one_where_clause(
    source: &SourceFile,
    w: &ast::WhereClause,
    declared_type_params: &[String],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), WhereClauseError> {
    let declared_set: HashSet<&str> = declared_type_params.iter().map(|s| s.as_str()).collect();

    // (type_param_name, bound_type_id) -> first constraint span
    let mut seen: HashMap<(String, TypeId), Span> = HashMap::new();
    // type_param_name -> first class-like bound (type_id, span)
    let mut first_class_bound: HashMap<String, (TypeId, Span)> = HashMap::new();

    for c in &w.constraints {
        let param = source.slice(c.ty_param.span).to_string();
        if !declared_set.contains(param.as_str()) {
            return Err(WhereClauseError::TargetNotInCurrentDecl {
                param,
                span: c.ty_param.span.into(),
            });
        }

        let bound_ty = lower.lower_type_ref(&c.bound)?;

        // 约束上界暂不支持“泛型 nominal type”（后续需要更完整的泛型超类型实例化/子类型规则）。
        if nominal_type_id_has_args(bound_ty, lower) {
            let bound_text = source.slice(c.bound.span()).to_string();
            return Err(WhereClauseError::GenericBoundNotSupported {
                bound: bound_text,
                span: c.bound.span().into(),
            });
        }

        let key = (param.clone(), bound_ty);
        if let Some(prev_span) = seen.get(&key).copied() {
            return Err(WhereClauseError::DuplicateWhereConstraint {
                param,
                bound: lower.fmt_type(bound_ty),
                first: prev_span.into(),
                second: c.span.into(),
            });
        }
        seen.insert(key, c.span);

        // Kotlin-like：允许多个 interface bounds，但只允许一个 class-like bound。
        if is_interface_like_bound(bound_ty, lower, builtins) {
            continue;
        }

        match first_class_bound.get(&param).copied() {
            None => {
                first_class_bound.insert(param, (bound_ty, c.span));
            }
            Some((prev_ty, _prev_span)) if prev_ty == bound_ty => {}
            Some((prev_ty, prev_span)) => {
                return Err(WhereClauseError::ConflictingWhereConstraints {
                    param,
                    a: lower.fmt_type(prev_ty),
                    b: lower.fmt_type(bound_ty),
                    first: prev_span.into(),
                    second: c.span.into(),
                });
            }
        }
    }

    Ok(())
}

fn collect_type_param_names(source: &SourceFile, params: &[ast::TypeParam]) -> Vec<String> {
    params
        .iter()
        .map(|p| source.slice(p.name.span).to_string())
        .collect()
}

fn nominal_type_id_has_args(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => !n.args.is_empty(),
        TypeKind::Value(ValueTypeKind::Nominal(n)) => !n.args.is_empty(),
        _ => false,
    }
}

fn is_interface_like_bound(ty: TypeId, lower: &TypeLowering<'_>, builtins: BuiltinTypes) -> bool {
    if ty == builtins.any {
        return true;
    }

    match lower.type_kind(ty) {
        // `where T: U` 这类关系约束暂不计入 “class-like 上界” 冲突规则，
        // 避免在没有完整推断/求解时误报。
        TypeKind::Param(_) => true,
        TypeKind::Ref(RefTypeKind::Nominal(n)) => {
            matches!(
                lower.nominal_decl_kind(&n.fqn),
                Some(ast::TypeKind::Interface)
            )
        }
        _ => false,
    }
}
