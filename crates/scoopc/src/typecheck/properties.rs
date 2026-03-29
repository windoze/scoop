//! 属性声明的最小语义检查（T0431/T0432）。
//!
//! 当前阶段覆盖：
//! - class 属性（spec §10.1）：
//! - 默认 getter/setter 视为存在（因此可能生成 backing field）
//! - `field` 仅允许在“有 backing field”的属性 accessor 内使用
//! - value type（struct/enum）computed 属性（spec §10.2）：
//!   - 不允许 `var` / setter
//!   - computed 属性不允许 initializer（避免引入 backing field 语义）
//! - extension property（spec §10.3）：
//!   - 必须 computed（不生成 backing field）
//!   - 不允许 initializer
//!   - 不允许引用 `field`
//!   - 必须显式声明 getter；`var` 还必须显式声明 setter
//! - delegated property（spec §10.4）：
//!   - 仅允许出现在 class（struct/enum 禁止）
//!   - delegate 需要存在 `getValue`；`var` 还需要存在 `setValue`
//!   - 对 `getValue/setValue` 做最小签名检查（T0434b）：
//!     - `getValue(thisRef: T|Any, property: PropertyMeta): V`
//!     - `setValue(thisRef: T|Any, property: PropertyMeta, value: V): Unit`（仅 `var`）
//!
//! 说明：
//! - 该模块不做 codegen，也不展开 property → getter/setter 调用的 lowering；
//! - accessor body 的完整表达式类型检查留到更后续阶段（T04/T05 扩展）。

use miette::Diagnostic;
use thiserror::Error;

use std::collections::{HashMap, HashSet};

use super::TypeEnv;
use super::type_env::FileTypeContext;
use crate::ast;
use crate::resolve::{FunOverload, Index};
use crate::source::SourceFile;
use crate::span::Span;

const ANY_FQN: &str = "scoop.core.Any";
const PROPERTY_META_FQN: &str = "scoop.core.PropertyMeta";
const UNIT_FQN: &str = "scoop.core.Unit";

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

    #[error(
        "值类型（struct/enum）computed 属性不允许 initializer（会生成 backing field）：{type_fqn}.{property}"
    )]
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

    #[error("扩展属性不允许 initializer（不会生成 backing field）：{receiver}.{property}")]
    #[diagnostic(code(scoop::typecheck::extension_property_initializer_not_allowed))]
    ExtensionPropertyInitializerNotAllowed {
        receiver: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("扩展属性必须显式声明 getter（computed，无默认 accessor）：{receiver}.{property}")]
    #[diagnostic(code(scoop::typecheck::extension_property_getter_required))]
    ExtensionPropertyGetterRequired {
        receiver: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`var` 扩展属性必须显式声明 setter（computed，无默认 accessor）：{receiver}.{property}"
    )]
    #[diagnostic(code(scoop::typecheck::extension_property_setter_required))]
    ExtensionPropertySetterRequired {
        receiver: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("扩展属性不生成 backing field：{receiver}.{property} 不能引用 `field`")]
    #[diagnostic(code(scoop::typecheck::extension_property_field_not_allowed))]
    ExtensionPropertyFieldNotAllowed {
        receiver: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`val` 扩展属性不允许自定义 setter：{receiver}.{property}")]
    #[diagnostic(code(scoop::typecheck::extension_val_property_setter_not_allowed))]
    ExtensionValPropertySetterNotAllowed {
        receiver: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("值类型（struct/enum）不允许委托属性（`by`）：{type_fqn}.{property}")]
    #[diagnostic(code(scoop::typecheck::delegated_property_not_allowed_in_value_type))]
    DelegatedPropertyNotAllowedInValueType {
        type_fqn: String,
        property: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "委托属性的 delegate 缺少 `getValue`：{class_fqn}.{property}（delegate: {delegate_ty}）"
    )]
    #[diagnostic(code(scoop::typecheck::delegated_property_missing_get_value))]
    DelegatedPropertyMissingGetValue {
        class_fqn: String,
        property: String,
        delegate_ty: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`var` 委托属性的 delegate 缺少 `setValue`：{class_fqn}.{property}（delegate: {delegate_ty}）"
    )]
    #[diagnostic(code(scoop::typecheck::delegated_property_missing_set_value))]
    DelegatedPropertyMissingSetValue {
        class_fqn: String,
        property: String,
        delegate_ty: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "委托属性的 delegate 未找到匹配的 `getValue` 签名：{class_fqn}.{property}（delegate: {delegate_ty}，期望 {expected}，实际 {found}）"
    )]
    #[diagnostic(code(scoop::typecheck::delegated_property_get_value_signature_mismatch))]
    DelegatedPropertyGetValueSignatureMismatch {
        class_fqn: String,
        property: String,
        delegate_ty: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "委托属性的 delegate 未找到匹配的 `setValue` 签名：{class_fqn}.{property}（delegate: {delegate_ty}，期望 {expected}，实际 {found}）"
    )]
    #[diagnostic(code(scoop::typecheck::delegated_property_set_value_signature_mismatch))]
    DelegatedPropertySetValueSignatureMismatch {
        class_fqn: String,
        property: String,
        delegate_ty: String,
        expected: String,
        found: String,
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
    index: &Index,
    env: &TypeEnv,
) -> Result<(), PropertyDeclError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                check_type_decl_properties(source, file, ty, &pkg_prefix, index, env)?
            }
            ast::Item::ExtensionProperty(p) => check_one_extension_property(source, p)?,
            ast::Item::TypeAlias(_)
            | ast::Item::Fun(_)
            | ast::Item::Object(_)
            | ast::Item::Val(_) => {}
        }
    }
    Ok(())
}

fn check_type_decl_properties(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    prefix: &str,
    index: &Index,
    env: &TypeEnv,
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
                let value_types = collect_class_value_decl_types(decl);
                for member in &body.members {
                    if let ast::TypeMember::Property(p) = member {
                        check_one_class_property(
                            source,
                            file,
                            &type_fqn,
                            p,
                            &value_types,
                            index,
                            env,
                        )?;
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
        check_type_decl_properties(source, file, nested, &type_fqn, index, env)?;
    }

    Ok(())
}

fn collect_class_value_decl_types(decl: &ast::TypeDecl) -> HashMap<Span, &ast::TypeRef> {
    let mut out: HashMap<Span, &ast::TypeRef> = HashMap::new();

    // 主构造参数中的 `val/var` 字段（T0438）。
    if let Some(primary_ctor) = &decl.primary_ctor {
        for p in &primary_ctor.params {
            if p.kind.is_none() {
                continue;
            }
            let Some(ty) = p.ty.as_ref() else {
                continue;
            };
            out.insert(p.name.span, ty);
        }
    }

    // class body 内的属性声明。
    if let Some(body) = &decl.body {
        for m in &body.members {
            let ast::TypeMember::Property(p) = m else {
                continue;
            };
            let Some(ty) = p.ty.as_ref() else {
                continue;
            };
            out.insert(p.name.span, ty);
        }
    }

    out
}

fn check_one_class_property(
    source: &SourceFile,
    file: &ast::File,
    class_fqn: &str,
    p: &ast::PropertyDecl,
    value_types: &HashMap<Span, &ast::TypeRef>,
    index: &Index,
    env: &TypeEnv,
) -> Result<(), PropertyDeclError> {
    let property = source.slice(p.name.span).to_string();

    if let Some(delegate) = &p.delegate {
        // delegate 需要存在 `getValue`；`var` 还需要存在 `setValue`（spec §10.4）。
        //
        // 注意：
        // - 当前阶段仅做最小签名检查（T0434b），不生成 `$delegate` 字段/转发函数（见 T1210）；
        // - 若无法从 delegate expr 推导出 delegate nominal type，则保守放行（避免误伤未覆盖的表达式形态）。
        if let Some(delegate_ty) =
            delegate_expr_nominal_type_fqn(source, file, index, env, value_types, delegate)
        {
            // 属性声明头检查（T0404）已保证 type annotation 存在；这里仍保持健壮性。
            let property_ty_ref = p.ty.as_ref();
            let property_ty_fqn =
                property_ty_ref.and_then(|t| type_ref_to_fqn_in_file(source, file, index, t));

            if !type_has_method_named(index, env, &delegate_ty, "getValue") {
                return Err(PropertyDeclError::DelegatedPropertyMissingGetValue {
                    class_fqn: class_fqn.to_string(),
                    property,
                    delegate_ty,
                    span: delegate.span.into(),
                });
            }

            check_delegated_property_get_value_signature(
                source,
                file,
                index,
                env,
                class_fqn,
                &property,
                &delegate_ty,
                property_ty_fqn.as_deref(),
                delegate.span,
            )?;

            if matches!(p.kind, ast::ValKind::Var) {
                if !type_has_method_named(index, env, &delegate_ty, "setValue") {
                    return Err(PropertyDeclError::DelegatedPropertyMissingSetValue {
                        class_fqn: class_fqn.to_string(),
                        property,
                        delegate_ty,
                        span: delegate.span.into(),
                    });
                }

                check_delegated_property_set_value_signature(
                    source,
                    file,
                    index,
                    env,
                    class_fqn,
                    &property,
                    &delegate_ty,
                    property_ty_fqn.as_deref(),
                    delegate.span,
                )?;
            }
        }
    }

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
    let has_backing_field =
        p.delegate.is_none() && (p.init.is_some() || has_default_getter || has_default_setter);

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

    // 委托属性仅允许出现在 class（spec §10.4）。
    if p.delegate.is_some() {
        return Err(PropertyDeclError::DelegatedPropertyNotAllowedInValueType {
            type_fqn: type_fqn.to_string(),
            property,
            span: p.name.span.into(),
        });
    }

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

fn check_one_extension_property(
    source: &SourceFile,
    p: &ast::ExtensionPropertyDecl,
) -> Result<(), PropertyDeclError> {
    let receiver = source.slice(p.receiver.span()).to_string();
    let property = source.slice(p.name.span).to_string();

    // `val` extension property 不应存在 setter（与 class 属性保持一致，但错误信息更明确）。
    if matches!(p.kind, ast::ValKind::Val) {
        if let Some(setter) = &p.setter {
            return Err(PropertyDeclError::ExtensionValPropertySetterNotAllowed {
                receiver,
                property,
                span: setter.span.into(),
            });
        }
    }

    // extension property 不允许 initializer（不生成 backing field）。
    if let Some(init) = &p.init {
        return Err(PropertyDeclError::ExtensionPropertyInitializerNotAllowed {
            receiver,
            property,
            span: init.span.into(),
        });
    }

    // 必须 computed：extension property 的 default getter/setter 需要 backing field，因此不允许省略。
    if p.getter.is_none() {
        return Err(PropertyDeclError::ExtensionPropertyGetterRequired {
            receiver,
            property,
            span: p.name.span.into(),
        });
    }
    if matches!(p.kind, ast::ValKind::Var) && p.setter.is_none() {
        return Err(PropertyDeclError::ExtensionPropertySetterRequired {
            receiver,
            property,
            span: p.name.span.into(),
        });
    }

    // 无 backing field：禁止引用 `field`。
    let backing_field_decl_span = p.name.span;
    if let Some(span) =
        field_use_span_in_accessor(source, backing_field_decl_span, p.getter.as_ref()).or_else(
            || field_use_span_in_accessor(source, backing_field_decl_span, p.setter.as_ref()),
        )
    {
        return Err(PropertyDeclError::ExtensionPropertyFieldNotAllowed {
            receiver,
            property,
            span: span.into(),
        });
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
        ast::StmtKind::While { cond, body, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, cond)
                .or_else(|| field_use_span_in_block(source, backing_field_decl_span, body))
        }
        ast::StmtKind::For(f) => field_use_span_in_expr(source, backing_field_decl_span, &f.iter)
            .or_else(|| field_use_span_in_block(source, backing_field_decl_span, &f.body)),
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
        | ast::ExprKind::UnitLit
        | ast::ExprKind::ClassLit { .. } => None,
        ast::ExprKind::TupleLit { elements } => elements
            .iter()
            .find_map(|e| field_use_span_in_expr(source, backing_field_decl_span, e)),
        ast::ExprKind::ArrayLit { elements } => elements
            .iter()
            .find_map(|e| field_use_span_in_expr(source, backing_field_decl_span, e)),
        ast::ExprKind::SpreadArg { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
        }
        ast::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|p| match p {
            ast::InterpolatedStringPart::Text { .. } => None,
            ast::InterpolatedStringPart::Expr { expr } => {
                field_use_span_in_expr(source, backing_field_decl_span, expr)
            }
        }),
        ast::ExprKind::Block(b) => field_use_span_in_block(source, backing_field_decl_span, b),
        ast::ExprKind::UnsafeBlock { body, .. } => {
            field_use_span_in_block(source, backing_field_decl_span, body)
        }
        ast::ExprKind::SafeBlock { body, .. } => {
            field_use_span_in_block(source, backing_field_decl_span, body)
        }
        ast::ExprKind::Lambda(l) => {
            field_use_span_in_expr(source, backing_field_decl_span, &l.body)
        }
        ast::ExprKind::StructLit { fields, .. } => fields
            .iter()
            .find_map(|f| field_use_span_in_expr(source, backing_field_decl_span, &f.value)),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => field_use_span_in_expr(source, backing_field_decl_span, cond)
            .or_else(|| field_use_span_in_expr(source, backing_field_decl_span, then_branch))
            .or_else(|| {
                else_branch
                    .as_ref()
                    .and_then(|e| field_use_span_in_expr(source, backing_field_decl_span, e))
            }),
        ast::ExprKind::When { subject, arms } => {
            field_use_span_in_expr(source, backing_field_decl_span, subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    field_use_span_in_expr(source, backing_field_decl_span, &arm.body).or_else(
                        || {
                            arm.guard.as_ref().and_then(|g| {
                                field_use_span_in_expr(source, backing_field_decl_span, g)
                            })
                        },
                    )
                })
            })
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => field_use_span_in_block(source, backing_field_decl_span, body)
            .or_else(|| {
                arms.iter().find_map(|arm| {
                    field_use_span_in_expr(source, backing_field_decl_span, &arm.body)
                })
            })
            .or_else(|| {
                finally
                    .as_ref()
                    .and_then(|b| field_use_span_in_block(source, backing_field_decl_span, b))
            }),
        ast::ExprKind::Async { body } => {
            field_use_span_in_block(source, backing_field_decl_span, body)
        }
        ast::ExprKind::Spawn { body } => {
            field_use_span_in_block(source, backing_field_decl_span, body)
        }
        ast::ExprKind::Await { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
        }
        ast::ExprKind::Join { expr, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, expr)
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
        ast::ExprKind::TypeApply { callee, .. } => {
            field_use_span_in_expr(source, backing_field_decl_span, callee)
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
                updates
                    .iter()
                    .find_map(|u| field_use_span_in_expr(source, backing_field_decl_span, &u.value))
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

fn delegate_expr_constructor_type_fqn(delegate: &ast::Expr) -> Option<String> {
    let ast::ExprKind::Call { callee, .. } = &delegate.kind else {
        return None;
    };

    let ast::ExprKind::Ident(id) = &callee.kind else {
        return None;
    };

    let call = id.call.as_ref()?;
    let mut ctors: Vec<String> = call
        .candidates
        .iter()
        .filter_map(|c| match c {
            ast::CallCandidate::Constructor { ty_fqn } => Some(ty_fqn.clone()),
            ast::CallCandidate::Fun { .. } => None,
        })
        .collect();
    ctors.sort();
    ctors.dedup();

    if ctors.len() == 1 {
        return Some(ctors.remove(0));
    }

    None
}

fn delegate_expr_nominal_type_fqn(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    value_types: &HashMap<Span, &ast::TypeRef>,
    delegate: &ast::Expr,
) -> Option<String> {
    match &delegate.kind {
        ast::ExprKind::Call { callee, .. } => {
            // 1) 优先走旧逻辑：构造调用 → delegate nominal type（T0434b）。
            if let Some(ty) = delegate_expr_constructor_type_fqn(delegate) {
                return Some(ty);
            }

            // 2) 顶层函数调用：使用“返回类型的名义类型”作为 delegate nominal type。
            //
            // 说明：
            // - 该策略主要用于标准 delegates（`lazy/observable/vetoable`）；
            // - 当前阶段不做 overload resolution：只在候选集合可唯一确定一个顶层函数时才尝试推导。
            let ast::ExprKind::Ident(id) = &callee.kind else {
                return None;
            };

            // 2.1) 优先使用 resolver 写回的候选集合（比 `resolved` 更稳健）。
            if let Some(call) = id.call.as_ref() {
                let mut funs: Vec<String> = call
                    .candidates
                    .iter()
                    .filter_map(|c| match c {
                        ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                        ast::CallCandidate::Constructor { .. } => None,
                    })
                    .collect();
                funs.sort();
                funs.dedup();

                if funs.len() == 1 {
                    return top_level_fun_return_type_fqn(index, env, &funs[0]);
                }
            }

            // 2.2) fallback：若 resolver 已把 callee 绑定为唯一顶层函数，同样可用。
            let Some(ast::ResolvedValueRef::TopLevel { fqn }) = &id.resolved else {
                return None;
            };
            top_level_fun_return_type_fqn(index, env, fqn)
        }

        // 3) `val x: T by data`：从 `data` 的声明类型推导 delegate nominal type。
        //
        // 说明：
        // - 当前只覆盖 class 初始化语境内的“字段/属性引用”（其 binder span 可通过 `resolved` 的
        //   `decl_span` 定位到 ctor param 或同一 class 的属性声明）。
        ast::ExprKind::Ident(id) => {
            let Some(ast::ResolvedValueRef::Local { decl_span, .. }) = &id.resolved else {
                return None;
            };
            let ty = value_types.get(decl_span)?;
            type_ref_to_fqn_in_file(source, file, index, ty)
        }

        _ => None,
    }
}

fn top_level_fun_return_type_fqn(index: &Index, env: &TypeEnv, fun_fqn: &str) -> Option<String> {
    let overloads = index.by_fqn.get(fun_fqn).map(|syms| syms.fun.as_slice())?;
    // 对 delegated property 而言，我们只需要“返回的名义类型”（用于检查 `getValue/setValue`）。
    //
    // 允许同一 overload set 存在多个重载，但要求它们的返回名义类型一致：
    // - 标准 delegates（`lazy/observable/vetoable`）会通过重载提供不同调用形态，
    //   但它们的 delegate nominal type 应当稳定一致。
    let mut return_ty_fqn: Option<String> = None;

    for overload in overloads {
        let ret = overload.sig.return_ty.as_ref()?;
        let (decl_source, decl_ctx) = overload_decl_source_and_ctx(env, overload)?;
        let fqn = type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, ret)?;

        match &return_ty_fqn {
            None => return_ty_fqn = Some(fqn),
            Some(prev) if prev == &fqn => {}
            Some(_) => return None,
        }
    }

    return_ty_fqn
}

fn type_has_method_named(index: &Index, env: &TypeEnv, ty_fqn: &str, method: &str) -> bool {
    // NOTE: delegated property 的 `getValue/setValue` 可以来自 supertypes
    // （例如 `ReadWriteProperty` 继承 `ReadOnlyProperty` 的 `getValue`）。
    !method_overloads_in_type_hierarchy(index, env, ty_fqn, method).is_empty()
}

fn check_delegated_property_get_value_signature(
    _source: &SourceFile,
    _file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    class_fqn: &str,
    property: &str,
    delegate_ty: &str,
    property_ty_fqn: Option<&str>,
    use_span: Span,
) -> Result<(), PropertyDeclError> {
    let Some(property_ty_fqn) = property_ty_fqn else {
        // 解析/类型引用校验应当能确保 property type 可解析；这里避免 panic，保守放行。
        return Ok(());
    };

    let overloads = method_overloads_in_type_hierarchy(index, env, delegate_ty, "getValue");

    let mut found_sig = None;
    for o in &overloads {
        found_sig.get_or_insert_with(|| fmt_fun_sig(env, index, "getValue", o));
        if delegated_get_value_overload_matches(env, index, o, class_fqn, property_ty_fqn) {
            return Ok(());
        }
    }

    Err(
        PropertyDeclError::DelegatedPropertyGetValueSignatureMismatch {
            class_fqn: class_fqn.to_string(),
            property: property.to_string(),
            delegate_ty: delegate_ty.to_string(),
            expected: format!(
                "getValue(thisRef: {class_fqn}|{ANY_FQN}, property: {PROPERTY_META_FQN}): {property_ty_fqn}"
            ),
            found: found_sig.unwrap_or_else(|| "<no overload>".to_string()),
            span: use_span.into(),
        },
    )
}

fn check_delegated_property_set_value_signature(
    _source: &SourceFile,
    _file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    class_fqn: &str,
    property: &str,
    delegate_ty: &str,
    property_ty_fqn: Option<&str>,
    use_span: Span,
) -> Result<(), PropertyDeclError> {
    let Some(property_ty_fqn) = property_ty_fqn else {
        return Ok(());
    };

    let overloads = method_overloads_in_type_hierarchy(index, env, delegate_ty, "setValue");

    let mut found_sig = None;
    for o in &overloads {
        found_sig.get_or_insert_with(|| fmt_fun_sig(env, index, "setValue", o));
        if delegated_set_value_overload_matches(env, index, o, class_fqn, property_ty_fqn) {
            return Ok(());
        }
    }

    Err(
        PropertyDeclError::DelegatedPropertySetValueSignatureMismatch {
            class_fqn: class_fqn.to_string(),
            property: property.to_string(),
            delegate_ty: delegate_ty.to_string(),
            expected: format!(
                "setValue(thisRef: {class_fqn}|{ANY_FQN}, property: {PROPERTY_META_FQN}, value: {property_ty_fqn}): {UNIT_FQN}"
            ),
            found: found_sig.unwrap_or_else(|| "<no overload>".to_string()),
            span: use_span.into(),
        },
    )
}

fn delegated_get_value_overload_matches(
    env: &TypeEnv,
    index: &Index,
    overload: &FunOverload,
    owner_ty_fqn: &str,
    value_ty_fqn: &str,
) -> bool {
    let Some((decl_source, decl_ctx)) = overload_decl_source_and_ctx(env, overload) else {
        return false;
    };

    if overload.sig.params.len() != 2 {
        return false;
    }

    let this_ref_ty_fqn = overload
        .sig
        .params
        .first()
        .and_then(|p| p.ty.as_ref())
        .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t));

    if let Some(this_ref) = this_ref_ty_fqn.as_deref() {
        if this_ref != owner_ty_fqn && this_ref != ANY_FQN {
            return false;
        }
    }

    let Some(prop) = overload
        .sig
        .params
        .get(1)
        .and_then(|p| p.ty.as_ref())
        .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t))
    else {
        return false;
    };
    if prop != PROPERTY_META_FQN {
        return false;
    }

    let Some(ret) = overload.sig.return_ty.as_ref() else {
        // 无显式返回类型：按语言默认 `Unit` 处理，此时不可能匹配大多数属性类型。
        return value_ty_fqn == UNIT_FQN;
    };

    match type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, ret) {
        Some(fqn) => fqn == value_ty_fqn,
        // 返回类型也可能是类型参数（例如 `V`）：视为可匹配属性类型。
        None => true,
    }
}

fn delegated_set_value_overload_matches(
    env: &TypeEnv,
    index: &Index,
    overload: &FunOverload,
    owner_ty_fqn: &str,
    value_ty_fqn: &str,
) -> bool {
    let Some((decl_source, decl_ctx)) = overload_decl_source_and_ctx(env, overload) else {
        return false;
    };

    if overload.sig.params.len() != 3 {
        return false;
    }

    let this_ref_ty_fqn = overload
        .sig
        .params
        .first()
        .and_then(|p| p.ty.as_ref())
        .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t));

    if let Some(this_ref) = this_ref_ty_fqn.as_deref() {
        if this_ref != owner_ty_fqn && this_ref != ANY_FQN {
            return false;
        }
    }

    let Some(prop) = overload
        .sig
        .params
        .get(1)
        .and_then(|p| p.ty.as_ref())
        .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t))
    else {
        return false;
    };
    if prop != PROPERTY_META_FQN {
        return false;
    }

    let value_ty = overload
        .sig
        .params
        .get(2)
        .and_then(|p| p.ty.as_ref())
        .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t));

    if let Some(value) = value_ty.as_deref() {
        if value != value_ty_fqn {
            return false;
        }
    }

    // `setValue` 返回类型必须是 `Unit`（或省略返回类型）。
    match overload.sig.return_ty.as_ref() {
        None => true,
        Some(ret) => type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, ret)
            .is_some_and(|fqn| fqn == UNIT_FQN),
    }
}

fn fmt_fun_sig(env: &TypeEnv, index: &Index, name: &str, overload: &FunOverload) -> String {
    let Some((decl_source, decl_ctx)) = overload_decl_source_and_ctx(env, overload) else {
        return format!("{name}(<unknown decl>): {UNIT_FQN}");
    };

    let mut params = Vec::with_capacity(overload.sig.params.len());
    for p in &overload.sig.params {
        let ty =
            p.ty.as_ref()
                .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t))
                .unwrap_or_else(|| "_".to_string());
        params.push(format!("{ty}"));
    }

    let ret = overload
        .sig
        .return_ty
        .as_ref()
        .and_then(|t| type_ref_to_fqn_in_ctx(decl_source, decl_ctx, index, t))
        .unwrap_or_else(|| UNIT_FQN.to_string());

    format!("{name}({}): {ret}", params.join(", "))
}

fn method_overloads_in_type_hierarchy<'a>(
    index: &'a Index,
    env: &TypeEnv,
    ty_fqn: &str,
    method: &str,
) -> Vec<&'a FunOverload> {
    let mut out: Vec<&'a FunOverload> = Vec::new();
    let mut worklist: Vec<String> = vec![ty_fqn.to_string()];
    let mut visited: HashSet<String> = HashSet::new();

    while let Some(ty) = worklist.pop() {
        if !visited.insert(ty.clone()) {
            continue;
        }

        let fqn = format!("{ty}.{method}");
        if let Some(syms) = index.by_fqn.get(&fqn) {
            out.extend(syms.fun.iter());
        }

        if let Some(supers) = env.direct_supertypes(&ty) {
            worklist.extend(supers.iter().cloned());
        }
    }

    out
}

fn overload_decl_source_and_ctx<'a>(
    env: &'a TypeEnv,
    overload: &'a FunOverload,
) -> Option<(&'a SourceFile, &'a FileTypeContext)> {
    let source = env.source(&overload.symbol.decl_file)?;
    let ctx = env.file_type_context(&overload.symbol.decl_file)?;
    Some((source, ctx))
}

fn type_ref_to_fqn_in_ctx(
    source: &SourceFile,
    ctx: &FileTypeContext,
    index: &Index,
    ty: &ast::TypeRef,
) -> Option<String> {
    match ty {
        ast::TypeRef::Path(p) => type_path_to_fqn_in_ctx(source, ctx, index, p),
        ast::TypeRef::Nullable { .. } => {
            // `T?` lowering 会变成 `Option<T>`；这里做最小等价映射，避免在签名检查中误报。
            let fqn = "scoop.core.Option";
            index
                .by_fqn
                .get(fqn)
                .is_some_and(|syms| syms.ty.is_some())
                .then(|| fqn.to_string())
        }
        ast::TypeRef::Tuple(t) if t.elements.is_empty() => Some(UNIT_FQN.to_string()),
        ast::TypeRef::Tuple(_)
        | ast::TypeRef::Star { .. }
        | ast::TypeRef::EffectRowArg { .. }
        | ast::TypeRef::Function(_) => None,
    }
}

fn type_path_to_fqn_in_ctx(
    source: &SourceFile,
    ctx: &FileTypeContext,
    index: &Index,
    path: &ast::TypePath,
) -> Option<String> {
    let segments = path
        .segments
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>();
    let local = segments.join(".");

    let mut candidates: Vec<String> = Vec::new();

    // 1) 同包优先：pkg + local
    if !ctx.pkg_prefix.is_empty() {
        candidates.push(format!("{}.{}", ctx.pkg_prefix, local));
    }

    // 2) 直接使用 local（允许显式写 FQN：`scoop.core.Any`）
    candidates.push(local.clone());

    // 3) 对单段名字，应用 import 规则（显式 import / star import）
    if segments.len() == 1 {
        let name = segments[0];

        // 显式 type import。
        if let Some(list) = ctx.imports.ty.explicit.get(name) {
            candidates.extend(list.iter().cloned());
        }

        // 通配 import：`import foo.bar.*`
        for prefix in &ctx.imports.star {
            candidates.push(format!("{prefix}.{name}"));
        }
    }

    candidates.sort();
    candidates.dedup();

    for fqn in candidates {
        let Some(syms) = index.by_fqn.get(&fqn) else {
            continue;
        };
        if syms.ty.is_some() {
            return Some(fqn);
        }
    }

    None
}

fn type_ref_to_fqn_in_file(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ty: &ast::TypeRef,
) -> Option<String> {
    match ty {
        ast::TypeRef::Path(p) => type_path_to_fqn_in_file(source, file, index, p),
        ast::TypeRef::Nullable { .. } => {
            // `T?` lowering 会变成 `Option<T>`；这里做最小等价映射，避免在签名检查中误报。
            let fqn = "scoop.core.Option";
            index
                .by_fqn
                .get(fqn)
                .is_some_and(|syms| syms.ty.is_some())
                .then(|| fqn.to_string())
        }
        ast::TypeRef::Tuple(t) if t.elements.is_empty() => Some(UNIT_FQN.to_string()),
        ast::TypeRef::Tuple(_)
        | ast::TypeRef::Star { .. }
        | ast::TypeRef::EffectRowArg { .. }
        | ast::TypeRef::Function(_) => None,
    }
}

fn type_path_to_fqn_in_file(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    path: &ast::TypePath,
) -> Option<String> {
    let segments = path
        .segments
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>();
    let local = segments.join(".");

    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut candidates: Vec<String> = Vec::new();

    // 1) 同包优先：pkg + local
    if !pkg_prefix.is_empty() {
        candidates.push(format!("{pkg_prefix}.{local}"));
    }

    // 2) 直接使用 local（允许显式写 FQN：`scoop.core.Any`）
    candidates.push(local.clone());

    // 3) 对单段名字，应用 import 规则（显式 import / star import）
    if segments.len() == 1 {
        let name = segments[0];
        for import in &file.imports {
            let import_path = import
                .path
                .iter()
                .map(|id| source.slice(id.span))
                .collect::<Vec<_>>()
                .join(".");

            if import.has_star {
                candidates.push(format!("{import_path}.{name}"));
            } else {
                let local = import
                    .alias
                    .as_ref()
                    .map(|id| source.slice(id.span))
                    .or_else(|| import.path.last().map(|id| source.slice(id.span)))
                    .unwrap_or("");
                if local == name {
                    candidates.push(import_path);
                }
            }
        }
    }

    candidates.sort();
    candidates.dedup();

    for fqn in candidates {
        let Some(syms) = index.by_fqn.get(&fqn) else {
            continue;
        };
        if syms.ty.is_some() {
            return Some(fqn);
        }
    }

    None
}
