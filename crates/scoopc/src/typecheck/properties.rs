//! 属性声明的最小语义检查（T0431/T0432）。
//!
//! 当前阶段覆盖：
//! - class 属性（spec §10.1）：
//! - 默认 getter/setter 视为存在（因此可能生成 backing field）
//! - `field` 仅允许在“有 backing field”的属性 accessor 内使用
//! - value type（struct/enum）computed 属性（spec §10.2）：
//!   - 不允许 `var` / setter
//!   - computed 属性不允许 initializer（避免引入 backing field 语义）
//!
//! 说明：
//! - 该模块不做 codegen，也不展开 property → getter/setter 调用的 lowering；
//! - accessor body 的完整表达式类型检查留到更后续阶段（T04/T05 扩展）。

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;

#[derive(Debug, Error, Diagnostic)]
pub enum PropertyDeclError {
    #[error("`val` 属性不允许自定义 setter：{class_fqn}.{property}")]
    #[diagnostic(code(scoop::typecheck::val_property_setter_not_allowed))]
    ValPropertySetterNotAllowed {
        class_fqn: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("值类型（struct/enum）属性不允许 `var`：{type_fqn}.{property}")]
    #[diagnostic(code(scoop::typecheck::value_type_property_must_be_val))]
    ValueTypePropertyMustBeVal {
        type_fqn: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("值类型（struct/enum）computed 属性不允许 initializer（会生成 backing field）：{type_fqn}.{property}")]
    #[diagnostic(code(scoop::typecheck::value_type_property_initializer_not_allowed))]
    ValueTypePropertyInitializerNotAllowed {
        type_fqn: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("computed 属性不生成 backing field：{class_fqn}.{property} 不能引用 `field`")]
    #[diagnostic(code(scoop::typecheck::field_used_without_backing_field))]
    FieldUsedWithoutBackingField {
        class_fqn: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 检查一个文件内所有 type 声明的属性规则（含嵌套类型）：
/// - class（§10.1）
/// - value type：struct/enum（§10.2）
pub fn check_file_properties(
    source: &SourceFile,
    file: &ast::File,
) -> Result<(), PropertyDeclError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        let ast::Item::Type(ty) = item else {
            continue;
        };
        check_type_decl_properties(source, ty, &pkg_prefix)?;
    }
    Ok(())
}

fn check_type_decl_properties(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), PropertyDeclError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    match decl.kind {
        ast::TypeKind::Class => {
            if let Some(body) = &decl.body {
                for member in &body.members {
                    if let ast::TypeMember::Property(p) = member {
                        check_one_class_property(source, &type_fqn, p)?;
                    }
                }
            }
        }
        ast::TypeKind::Struct | ast::TypeKind::Enum => {
            if let Some(body) = &decl.body {
                for member in &body.members {
                    if let ast::TypeMember::Property(p) = member {
                        check_one_value_type_property(source, &type_fqn, p)?;
                    }
                }
            }
        }
        ast::TypeKind::Interface | ast::TypeKind::Effect => {}
    };

    // 递归处理 nested type（可能存在 nested class）。
    let Some(body) = &decl.body else {
        return Ok(());
    };
    for member in &body.members {
        let ast::TypeMember::Type(nested) = member else {
            continue;
        };
        check_type_decl_properties(source, nested, &type_fqn)?;
    }

    Ok(())
}

fn check_one_class_property(
    source: &SourceFile,
    class_fqn: &str,
    p: &ast::PropertyDecl,
) -> Result<(), PropertyDeclError> {
    let property = source.slice(p.name.span).to_string();

    // `val` 是只读属性：不应存在 setter。
    if matches!(p.kind, ast::ValKind::Val) {
        if let Some(setter) = &p.setter {
            return Err(PropertyDeclError::ValPropertySetterNotAllowed {
                class_fqn: class_fqn.to_string(),
                property,
                span: setter.span.into(),
            });
        }
    }

    // backing field 判定（当前阶段的最小实现）：
    // - 有 initializer → 生成 backing field
    // - 或者存在默认 accessor（省略 getter / setter）→ 生成 backing field
    //
    // 说明：
    // - spec §10.1 还提到“custom accessor 引用 `field` 也会生成 backing field”；
    //   该语义与初始化模型/loweing 更深度耦合，当前阶段仅做“禁止在 computed 属性中使用 `field`”的门禁。
    let has_default_getter = p.getter.is_none();
    let has_default_setter = matches!(p.kind, ast::ValKind::Var) && p.setter.is_none();
    let has_backing_field = p.init.is_some() || has_default_getter || has_default_setter;

    if !has_backing_field {
        let decl_span = p.name.span;
        if let Some(span) = field_use_span_in_accessor(source, decl_span, p.getter.as_ref())
            .or_else(|| field_use_span_in_accessor(source, decl_span, p.setter.as_ref()))
        {
            return Err(PropertyDeclError::FieldUsedWithoutBackingField {
                class_fqn: class_fqn.to_string(),
                property,
                span: span.into(),
            });
        }
    }

    Ok(())
}

fn check_one_value_type_property(
    source: &SourceFile,
    type_fqn: &str,
    p: &ast::PropertyDecl,
) -> Result<(), PropertyDeclError> {
    let property = source.slice(p.name.span).to_string();

    // 值类型不可变：属性不允许 `var`（即使语法上能解析）。
    if matches!(p.kind, ast::ValKind::Var) {
        return Err(PropertyDeclError::ValueTypePropertyMustBeVal {
            type_fqn: type_fqn.to_string(),
            property,
            span: p.name.span.into(),
        });
    }

    // 值类型 computed property：只允许 getter-only（setter 禁止）。
    if let Some(setter) = &p.setter {
        return Err(PropertyDeclError::ValPropertySetterNotAllowed {
            class_fqn: type_fqn.to_string(),
            property,
            span: setter.span.into(),
        });
    }

    // 仅当声明了 accessor（当前只可能是 getter）时，视为 computed property：
    // - computed 不应引入 backing field，因此不允许 initializer。
    if p.getter.is_some() {
        if let Some(init) = &p.init {
            return Err(PropertyDeclError::ValueTypePropertyInitializerNotAllowed {
                type_fqn: type_fqn.to_string(),
                property,
                span: init.span.into(),
            });
        }
    }

    Ok(())
}

fn field_use_span_in_accessor(
    source: &SourceFile,
    backing_field_decl_span: Span,
    acc: Option<&ast::AccessorDecl>,
) -> Option<Span> {
    let acc = acc?;
    match &acc.body {
        ast::AccessorBody::Block(b) => field_use_span_in_block(source, backing_field_decl_span, b),
        ast::AccessorBody::Expr(e) => field_use_span_in_expr(source, backing_field_decl_span, e),
        ast::AccessorBody::Missing => None,
    }
}

fn field_use_span_in_block(
    source: &SourceFile,
    backing_field_decl_span: Span,
    b: &ast::Block,
) -> Option<Span> {
    for stmt in &b.stmts {
        if let Some(span) = field_use_span_in_stmt(source, backing_field_decl_span, stmt) {
            return Some(span);
        }
    }
    None
}

fn field_use_span_in_stmt(
    source: &SourceFile,
    backing_field_decl_span: Span,
    stmt: &ast::Stmt,
) -> Option<Span> {
    match &stmt.kind {
        ast::StmtKind::Empty
        | ast::StmtKind::Break { .. }
        | ast::StmtKind::Continue { .. }
        | ast::StmtKind::Missing => None,
        ast::StmtKind::Expr(e) => field_use_span_in_expr(source, backing_field_decl_span, e),
        ast::StmtKind::Val(v) => v
            .init
            .as_ref()
            .and_then(|e| field_use_span_in_expr(source, backing_field_decl_span, e)),
        ast::StmtKind::Return { value, .. } => value
            .as_ref()
            .and_then(|e| field_use_span_in_expr(source, backing_field_decl_span, e)),
        ast::StmtKind::While { cond, body, .. } => field_use_span_in_expr(source, backing_field_decl_span, cond)
            .or_else(|| field_use_span_in_block(source, backing_field_decl_span, body)),
        ast::StmtKind::ComptimeBlock { body, .. } => {
            field_use_span_in_block(source, backing_field_decl_span, body)
        }
        ast::StmtKind::ComptimeIf(ci) => {
            field_use_span_in_comptime_if(source, backing_field_decl_span, ci)
        }
        ast::StmtKind::ComptimeFor(cf) => {
            field_use_span_in_expr(source, backing_field_decl_span, &cf.iter)
                .or_else(|| field_use_span_in_block(source, backing_field_decl_span, &cf.body))
        }
    }
}

fn field_use_span_in_comptime_if(
    source: &SourceFile,
    backing_field_decl_span: Span,
    ci: &ast::ComptimeIf,
) -> Option<Span> {
    field_use_span_in_expr(source, backing_field_decl_span, &ci.cond)
        .or_else(|| field_use_span_in_block(source, backing_field_decl_span, &ci.then_branch))
        .or_else(|| {
            ci.else_branch.as_ref().and_then(|b| match b.as_ref() {
                ast::ComptimeIfElse::Block(block) => {
                    field_use_span_in_block(source, backing_field_decl_span, block)
                }
                ast::ComptimeIfElse::If(inner) => {
                    field_use_span_in_comptime_if(source, backing_field_decl_span, inner)
                }
            })
        })
}

fn field_use_span_in_expr(
    source: &SourceFile,
    backing_field_decl_span: Span,
    e: &ast::Expr,
) -> Option<Span> {
    // 先检查当前节点（保证遇到最外层的 `field` 优先报错）。
    if let ast::ExprKind::Ident(id) = &e.kind {
        if source.slice(id.span) == "field" {
            if let Some(ast::ResolvedValueRef::Local { decl_span, .. }) = &id.resolved {
                if *decl_span == backing_field_decl_span {
                    return Some(id.span);
                }
            }
        }
    }

    match &e.kind {
        ast::ExprKind::Missing
        | ast::ExprKind::Ident(_)
        | ast::ExprKind::IntLit
        | ast::ExprKind::StringLit
        | ast::ExprKind::UnitLit => None,
        ast::ExprKind::TupleLit { elements } => elements
            .iter()
            .find_map(|e| field_use_span_in_expr(source, backing_field_decl_span, e)),
        ast::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|p| match p {
            ast::InterpolatedStringPart::Text { .. } => None,
            ast::InterpolatedStringPart::Expr { expr } => {
                field_use_span_in_expr(source, backing_field_decl_span, expr)
            }
        }),
        ast::ExprKind::Block(b) => field_use_span_in_block(source, backing_field_decl_span, b),
        ast::ExprKind::Lambda(l) => field_use_span_in_expr(source, backing_field_decl_span, &l.body),
        ast::ExprKind::StructLit { fields, .. } => fields.iter().find_map(|f| {
            field_use_span_in_expr(source, backing_field_decl_span, &f.value)
        }),
        ast::ExprKind::If { cond, then_branch, else_branch } => {
            field_use_span_in_expr(source, backing_field_decl_span, cond)
                .or_else(|| field_use_span_in_expr(source, backing_field_decl_span, then_branch))
                .or_else(|| else_branch.as_ref().and_then(|e| field_use_span_in_expr(source, backing_field_decl_span, e)))
        }
        ast::ExprKind::When { subject, arms } => {
            field_use_span_in_expr(source, backing_field_decl_span, subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    field_use_span_in_expr(source, backing_field_decl_span, &arm.body)
                        .or_else(|| arm.guard.as_ref().and_then(|g| field_use_span_in_expr(source, backing_field_decl_span, g)))
                })
            })
        }
        ast::ExprKind::MemberAccess { receiver, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, receiver)
        }
        ast::ExprKind::SpliceField { receiver, field } => {
            field_use_span_in_expr(source, backing_field_decl_span, receiver)
                .or_else(|| field_use_span_in_expr(source, backing_field_decl_span, field))
        }
        ast::ExprKind::SafeMemberAccess { receiver, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, receiver)
        }
        ast::ExprKind::Call { callee, args } => {
            field_use_span_in_expr(source, backing_field_decl_span, callee).or_else(|| {
                args.iter()
                    .find_map(|a| field_use_span_in_expr(source, backing_field_decl_span, a))
            })
        }
        ast::ExprKind::NamedArg { value, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, value)
        }
        ast::ExprKind::NotNullAssert { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
        }
        ast::ExprKind::Unary { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
        }
        ast::ExprKind::Binary { lhs, rhs, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, lhs)
                .or_else(|| field_use_span_in_expr(source, backing_field_decl_span, rhs))
        }
        ast::ExprKind::Assign { lhs, rhs, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, lhs)
                .or_else(|| field_use_span_in_expr(source, backing_field_decl_span, rhs))
        }
        ast::ExprKind::TypeCheck { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
        }
        ast::ExprKind::Cast { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
        }
        ast::ExprKind::WithUpdate { base, updates, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, base).or_else(|| {
                updates.iter().find_map(|u| {
                    field_use_span_in_expr(source, backing_field_decl_span, &u.value)
                })
            })
        }
    }
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
