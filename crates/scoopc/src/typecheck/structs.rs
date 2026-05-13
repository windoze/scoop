//! struct 声明的最小语义检查（T0409）。
//!
//! 当前阶段只关心 “字段（field）”：
//! - 收集 struct 字段列表（用于后续 typecheck/codegen 扩展的基础）
//! - 检查重复字段名
//! - 约束字段必须是不可变（`val`）
//!
//! 说明：
//! - 字段的“类型合法性”（TypeRef lowering、unresolved type、arity 等）由
//!   `typecheck::check_file_type_refs`（T0403）负责；
//! - 字段缺少 `: Type` 的错误由 `typecheck::check_file_headers`（T0404）负责。

use miette::Diagnostic;
use thiserror::Error;

use std::collections::HashMap;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;

#[derive(Debug, Error, Diagnostic)]
pub enum StructDeclError {
    #[error("struct 字段重复定义：{struct_fqn}.{field}")]
    #[diagnostic(code(scoop::typecheck::duplicate_struct_field))]
    DuplicateStructField {
        struct_fqn: String,
        field: String,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
    },

    #[error("struct 字段必须是 `val`，不允许 `var`：{struct_fqn}.{field}")]
    #[diagnostic(code(scoop::typecheck::struct_field_must_be_val))]
    StructFieldMustBeVal {
        struct_fqn: String,
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 检查一个文件内所有 struct 声明的字段规则（含嵌套类型）。
pub fn check_file_struct_decls(
    source: &SourceFile,
    file: &ast::File,
) -> Result<(), StructDeclError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        let ast::Item::Type(ty) = item else {
            continue;
        };
        check_type_decl_structs(source, ty, &pkg_prefix)?;
    }
    Ok(())
}

fn check_type_decl_structs(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), StructDeclError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        check_one_struct_fields(source, decl, &type_fqn)?;
    }

    // 递归处理 nested type（可能存在 nested struct）。
    let Some(body) = &decl.body else {
        return Ok(());
    };
    for member in &body.members {
        let ast::TypeMember::Type(nested) = member else {
            continue;
        };
        check_type_decl_structs(source, nested, &type_fqn)?;
    }

    Ok(())
}

fn check_one_struct_fields(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    struct_fqn: &str,
) -> Result<(), StructDeclError> {
    let mut seen: HashMap<String, Span> = HashMap::new();

    // 1) 主构造参数：对 struct 一律视为 direct field。
    //    `val`/省略前缀都表示不可变字段；显式 `var` 必须在这里被静态拒绝。
    if let Some(primary_ctor) = &decl.primary_ctor {
        for p in &primary_ctor.params {
            if matches!(p.kind, Some(ast::ValKind::Var)) {
                let field = source.slice(p.name.span).to_string();
                return Err(StructDeclError::StructFieldMustBeVal {
                    struct_fqn: struct_fqn.to_string(),
                    field,
                    span: p.name.span.into(),
                });
            }
            insert_field_name(source, struct_fqn, p.name.span, &mut seen)?;
        }
    }

    // 2) type body property：struct 字段只允许 `val`，且不支持 initializer。
    let Some(body) = &decl.body else {
        return Ok(());
    };
    for member in &body.members {
        let ast::TypeMember::Property(p) = member else {
            continue;
        };

        if matches!(p.kind, ast::ValKind::Var) {
            let field = source.slice(p.name.span).to_string();
            return Err(StructDeclError::StructFieldMustBeVal {
                struct_fqn: struct_fqn.to_string(),
                field,
                span: p.name.span.into(),
            });
        }

        insert_field_name(source, struct_fqn, p.name.span, &mut seen)?;
    }

    Ok(())
}

fn insert_field_name(
    source: &SourceFile,
    struct_fqn: &str,
    name_span: Span,
    seen: &mut HashMap<String, Span>,
) -> Result<(), StructDeclError> {
    let field = source.slice(name_span).to_string();
    if let Some(prev) = seen.get(&field).copied() {
        return Err(StructDeclError::DuplicateStructField {
            struct_fqn: struct_fqn.to_string(),
            field,
            first: prev.into(),
            second: name_span.into(),
        });
    }
    seen.insert(field, name_span);
    Ok(())
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}
