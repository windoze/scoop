//! 表达式类型检查（早期最小子集）。
//!
//! 已覆盖能力：
//! - （T0405）字面量最小推导：
//!   - `1` → `Int`
//!   - `"..."` / `f"..."` → `String`
//!   - `true` / `false` → `Bool`（当前阶段以 ident 语法承载）
//!   - `()` → `Unit`
//! - （T0406）变量引用（ident）类型推导：
//!   - 局部 `val/var`（通过 resolver 写回的 `ResolvedValueRef::Local`）
//!   - 函数参数（同样视作 `Local` 绑定）
//!   - 顶层 `val/var`（`ResolvedValueRef::TopLevel`，当前仅支持当前文件内可查询的顶层变量）
//! - （T0407）函数调用（`callee(args...)`）：
//!   - 参数数量检查
//!   - 参数类型匹配
//!   - 当前仅支持“当前文件内”的顶层函数（无重载/无默认参数/无命名参数）
//!
//! 说明：该模块以“可回归、可扩展”为目标，逐步把更多 `ExprKind`/`StmtKind` 纳入 typecheck。

use miette::Diagnostic;
use thiserror::Error;

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::TypeEnv;
use super::lower::{TypeLowerError, TypeLowering};

#[derive(Debug, Error, Diagnostic)]
pub enum ExprTypeError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error("暂不支持的表达式类型检查：{kind}")]
    #[diagnostic(code(scoop::typecheck::unsupported_expr))]
    UnsupportedExpr {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的模式绑定（pattern binding）")]
    #[diagnostic(code(scoop::typecheck::unsupported_pattern_binding))]
    UnsupportedPatternBinding {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("无法获取局部绑定的类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unknown_local_value_type))]
    UnknownLocalValueType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的顶层值引用类型推导：{fqn}")]
    #[diagnostic(code(scoop::typecheck::unsupported_top_level_value_type))]
    UnsupportedTopLevelValueType {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("初始化表达式类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::initializer_type_mismatch))]
    InitializerTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不可调用：{callee}")]
    #[diagnostic(code(scoop::typecheck::callee_not_callable))]
    CalleeNotCallable {
        callee: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用参数数量不匹配：{callee} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::call_arity_mismatch))]
    CallArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用参数类型不匹配：{callee} 第 {index} 个参数期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::call_arg_type_mismatch))]
    CallArgTypeMismatch {
        callee: String,
        index: usize,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("调用 receiver 类型不匹配：{callee} 期望 receiver 为 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::call_receiver_type_mismatch))]
    CallReceiverTypeMismatch {
        callee: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`?.` 的 receiver 必须是 nullable（`T?` / `Option<T>`），但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::safe_access_receiver_not_nullable))]
    SafeAccessReceiverNotNullable {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("Elvis `?:` 左操作数必须是 nullable（`T?` / `Option<T>`），但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::elvis_lhs_not_nullable))]
    ElvisLhsNotNullable {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("Elvis `?:` 右操作数类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::elvis_rhs_type_mismatch))]
    ElvisRhsTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的成员访问：{fqn}")]
    #[diagnostic(code(scoop::typecheck::unsupported_member_access))]
    UnsupportedMemberAccess {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不允许的显式类型转换：{from} -> {to}")]
    #[diagnostic(code(scoop::typecheck::invalid_cast))]
    InvalidCast {
        from: String,
        to: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("不可赋值：`{name}` 不是可变变量（必须声明为 `var`）")]
    #[diagnostic(code(scoop::typecheck::assignment_target_not_mutable))]
    AssignmentTargetNotMutable {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "`with` 的 base 必须是值类型（struct/tuple/enum），当前实现仅支持 struct；但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::with_update_base_not_supported))]
    WithUpdateBaseNotSupported {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持嵌套字段路径更新：{path}（当前仅支持单段字段名）")]
    #[diagnostic(code(scoop::typecheck::with_update_nested_path_not_supported))]
    WithUpdateNestedPathNotSupported {
        path: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{struct_name}` 不存在字段：{field}")]
    #[diagnostic(code(scoop::typecheck::with_update_unknown_field))]
    WithUpdateUnknownField {
        struct_name: String,
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{struct_name}.{field}` 更新值类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::with_update_field_type_mismatch))]
    WithUpdateFieldTypeMismatch {
        struct_name: String,
        field: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("struct literal 的类型必须是 struct，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_not_struct))]
    StructLitNotStruct {
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`{struct_name}` 不存在字段：{field}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_unknown_field))]
    StructLitUnknownField {
        struct_name: String,
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("struct literal 字段重复：{struct_name}.{field}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_duplicate_field))]
    StructLitDuplicateField {
        struct_name: String,
        field: String,
        #[label("重复写在这里")]
        second: miette::SourceSpan,
        #[label("第一次写在这里")]
        first: miette::SourceSpan,
    },

    #[error("`{struct_name}.{field}` 初始化值类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_field_type_mismatch))]
    StructLitFieldTypeMismatch {
        struct_name: String,
        field: String,
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("struct literal 缺少字段：{struct_name} 还需要 {fields}")]
    #[diagnostic(code(scoop::typecheck::struct_lit_missing_fields))]
    StructLitMissingFields {
        struct_name: String,
        fields: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("返回类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::return_type_mismatch))]
    ReturnTypeMismatch {
        expected: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("缺少返回值：函数返回类型为 {expected}")]
    #[diagnostic(code(scoop::typecheck::return_value_required))]
    ReturnValueRequired {
        expected: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`return` 只能出现在普通函数体内（lambda 的 non-local return 不支持）")]
    #[diagnostic(code(scoop::typecheck::return_not_in_function_body))]
    ReturnNotInFunctionBody {
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

#[derive(Debug, Clone)]
struct FunSigOwned {
    receiver: Option<TypeId>,
    params: Vec<TypeId>,
    return_ty: TypeId,
}

/// 对一个文件的表达式（当前阶段：顶层 `val/var` initializer）做最小类型检查。
///
/// 说明：
/// - 当前只覆盖能明确推导的字面量；
/// - 暂不进入函数体（后续任务会逐步接入 block/stmt/局部作用域等）。
pub fn check_file_exprs(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);

    // 顶层 `val/var` 的类型表：用于在表达式里引用顶层变量时查询其声明类型。
    //
    // 当前阶段约束：
    // - 只支持“当前文件内”的顶层变量（因为 typecheck phase 目前只解析单文件 AST）；
    // - 顶层变量必须有显式类型注解（由 `typecheck::check_file_headers` 保证）。
    let top_level_types = collect_top_level_value_types(source, file, &mut lower)?;
    let top_level_funs = collect_top_level_fun_signatures(source, file, &mut lower, builtins)?;
    let struct_field_types = collect_struct_field_types(source, file, &mut lower)?;

    for item in &file.items {
        match item {
            ast::Item::Val(v) => check_top_level_val_initializer(
                source,
                v,
                &mut lower,
                builtins,
                &top_level_types,
                &top_level_funs,
                &struct_field_types,
            )?,
            ast::Item::Fun(fun) => check_fun_body_exprs(
                source,
                fun,
                &mut lower,
                builtins,
                &top_level_types,
                &top_level_funs,
                &struct_field_types,
            )?,
            ast::Item::Type(_) | ast::Item::TypeAlias(_) => {}
        }
    }

    Ok(())
}

fn check_top_level_val_initializer(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let Some(init) = &v.init else {
        return Ok(());
    };
    let Some(ty_ref) = &v.ty else {
        // 顶层 val/var 缺少类型注解会在 `check_file_headers`（T0404）中报错；
        // 这里保持健壮性，不重复报错。
        return Ok(());
    };

    let expected = lower.lower_type_ref(ty_ref)?;
    let found = infer_expr_type(
        source,
        init,
        lower,
        builtins,
        &HashMap::new(),
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if expected == found {
        return Ok(());
    }

    Err(ExprTypeError::InitializerTypeMismatch {
        expected: lower.fmt_type(expected),
        found: lower.fmt_type(found),
        span: init.span.into(),
    })
}

fn infer_expr_type(
    source: &SourceFile,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    match &expr.kind {
        ast::ExprKind::IntLit => Ok(builtins.int),
        ast::ExprKind::StringLit | ast::ExprKind::InterpolatedString { .. } => Ok(builtins.string),
        ast::ExprKind::UnitLit => Ok(builtins.unit),
        ast::ExprKind::TupleLit { elements } => {
            if elements.is_empty() {
                return Ok(builtins.unit);
            }

            let mut element_types = Vec::with_capacity(elements.len());
            for e in elements {
                element_types.push(infer_expr_type(
                    source,
                    e,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?);
            }

            Ok(lower.ty_tuple(element_types))
        }
        ast::ExprKind::StructLit { ty, fields } => infer_struct_lit_expr_type(
            source,
            expr,
            ty,
            fields,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Ident(id) => {
            infer_value_ident_type(source, id, lower, builtins, locals, top_level_types)
        }
        ast::ExprKind::MemberAccess { receiver, member } => infer_member_access_expr_type(
            source,
            receiver.as_ref(),
            member,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => infer_safe_member_access_expr_type(
            source,
            receiver.as_ref(),
            member,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Call { callee, args } => infer_call_expr_type(
            source,
            expr,
            callee,
            args,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Cast {
            expr: inner,
            op,
            op_span,
            ty,
        } => {
            let from_ty = infer_expr_type(
                source,
                inner,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            let target_ty = lower.lower_type_ref(ty)?;

            if !is_cast_allowed(from_ty, target_ty, lower) {
                return Err(ExprTypeError::InvalidCast {
                    from: lower.fmt_type(from_ty),
                    to: lower.fmt_type(target_ty),
                    span: (*op_span).into(),
                });
            }

            match op {
                ast::CastOp::As => Ok(target_ty),
                ast::CastOp::AsQ => Ok(lower.ty_option(target_ty)),
            }
        }
        ast::ExprKind::TypeCheck {
            expr: inner, ty, ..
        } => {
            // `is`/`!is` 本身是一个表达式：结果类型为 `Bool`。
            //
            // 当前阶段只做最小检查：
            // - 确保被检查的表达式可推导类型（用于回归覆盖）；
            // - 确保目标类型引用可 lowering（否则应报 type lowering 错误）；
            // - 运行期语义与更强的类型关系约束留到后续阶段（PLAN §4.4 / TODO T0413+）。
            let _ = infer_expr_type(
                source,
                inner,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            let _ = lower.lower_type_ref(ty)?;
            Ok(builtins.bool_)
        }
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式结果类型：当前阶段（T0414）只实现最小规则：
            // - 递归类型检查 subject 与每个 arm body（保证覆盖其中的表达式）；
            // - 若所有分支 body 的类型相同，则结果为该类型；
            // - 若存在多个非 `Nothing` 的分支且类型不一致，则 fallback 为 `Any`（真正的 LUB 规则留到后续任务实现）。
            // - 若所有分支都是 `Nothing`（不可达），则整体结果为 `Nothing`。
            //
            // 额外：`Nothing` 是 bottom type（T0420a），因此：
            // - `Nothing` 与任意 `T` 的 LUB 至少应为 `T`；
            // - 这里先做一个最小 special-case：忽略 `Nothing` 分支参与比较。
            let _ = infer_expr_type(
                source,
                subject,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;

            let mut result: Option<TypeId> = None;
            for arm in arms {
                // 先确保 pattern 内的 TypeRef 可 lowering（为后续类型规则做铺垫）。
                if let ast::WhenPat::Is { ty, .. } = &arm.pat {
                    let _ = lower.lower_type_ref(ty)?;
                }

                let arm_ty = infer_expr_type(
                    source,
                    &arm.body,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;

                // `Nothing`：不可达分支（例如后续 `Raise.raise`），不影响最小 LUB 推导。
                if arm_ty == builtins.nothing {
                    continue;
                }

                match result {
                    None => result = Some(arm_ty),
                    Some(prev) if prev == arm_ty => {}
                    Some(_) => return Ok(builtins.any),
                }
            }

            // 若所有分支都是 `Nothing`，则 `when` 整体也是不可达的。
            Ok(result.unwrap_or(builtins.nothing))
        }
        ast::ExprKind::WithUpdate { base, updates, .. } => infer_with_update_expr_type(
            source,
            base,
            updates,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::Binary { lhs, op, rhs, .. } => match op {
            ast::BinaryOp::Elvis => infer_elvis_expr_type(
                source,
                lhs,
                rhs,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            ),
            _ => Err(ExprTypeError::UnsupportedExpr {
                kind: "binary expression",
                span: expr.span.into(),
            }),
        },
        ast::ExprKind::Missing => Err(ExprTypeError::UnsupportedExpr {
            kind: "missing",
            span: expr.span.into(),
        }),
        other => Err(ExprTypeError::UnsupportedExpr {
            kind: expr_kind_name(other),
            span: expr.span.into(),
        }),
    }
}

fn infer_struct_lit_expr_type(
    source: &SourceFile,
    struct_lit_expr: &ast::Expr,
    ty: &ast::TypePath,
    fields: &[ast::StructLitField],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先把 TypeName lowering 为一个 nominal value type（struct/enum）；并进一步约束必须是 struct。
    let struct_ty = lower.lower_type_ref(&ast::TypeRef::Path(ty.clone()))?;

    let (struct_fqn, struct_name) = match lower.type_kind(struct_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => (nominal.fqn, lower.fmt_type(struct_ty)),
        _ => {
            return Err(ExprTypeError::StructLitNotStruct {
                found: lower.fmt_type(struct_ty),
                span: ty.span.into(),
            });
        }
    };

    if !matches!(
        lower.nominal_decl_kind(&struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::StructLitNotStruct {
            found: struct_name,
            span: ty.span.into(),
        });
    }

    // 收集该 struct 的“直接字段”（不包含 nested type 的字段）。
    //
    // 说明：`collect_struct_field_types` 会为 nested struct 生成形如：
    //   `Outer.Inner.x`
    // 对于 `Outer { ... }` 的 struct literal，我们只接受 `Outer.<field>` 这一层。
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

    // 逐项检查：
    // - 字段名不可重复
    // - 字段必须存在于 struct 声明中
    // - 字段初始化表达式类型必须可赋值给字段类型（最小 assignable 规则）
    let mut seen: HashMap<String, Span> = HashMap::new();
    for f in fields {
        let field_name = source.slice(f.name.span).to_string();

        if let Some(prev) = seen.get(&field_name).copied() {
            return Err(ExprTypeError::StructLitDuplicateField {
                struct_name: struct_name.clone(),
                field: field_name,
                first: prev.into(),
                second: f.name.span.into(),
            });
        }
        seen.insert(field_name.clone(), f.name.span);

        let Some(expected_ty) = expected_fields.get(&field_name).copied() else {
            return Err(ExprTypeError::StructLitUnknownField {
                struct_name: struct_name.clone(),
                field: field_name,
                span: f.name.span.into(),
            });
        };

        let found_ty = infer_expr_type(
            source,
            &f.value,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(ExprTypeError::StructLitFieldTypeMismatch {
                struct_name: struct_name.clone(),
                field: field_name,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: f.value.span.into(),
            });
        }
    }

    // 当前阶段（T0423）约束：struct literal 必须显式提供所有字段（不支持默认值/可选字段）。
    let mut missing: Vec<String> = expected_fields
        .keys()
        .filter(|name| !seen.contains_key(*name))
        .cloned()
        .collect();
    missing.sort();
    if !missing.is_empty() {
        // 尽量把错误定位到右花括号 `}`（缺字段通常发生在结尾）。
        let close_brace = if struct_lit_expr.span.end > 0 {
            Span::new(struct_lit_expr.span.end - 1, struct_lit_expr.span.end)
        } else {
            struct_lit_expr.span
        };

        return Err(ExprTypeError::StructLitMissingFields {
            struct_name,
            fields: missing.join(", "),
            span: close_brace.into(),
        });
    }

    Ok(struct_ty)
}

fn infer_with_update_expr_type(
    source: &SourceFile,
    base: &ast::Expr,
    updates: &[ast::WithUpdateField],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 base：保证 `p with { ... }` 中的 `p` 自身也会被覆盖。
    let base_ty = infer_expr_type(
        source,
        base,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // 当前阶段（T0415）仅支持 struct 字段更新：
    // - base 必须是名义值类型，并且其声明 kind 为 `struct`
    // - update path 仅允许单段字段名（不支持 `a.b.c`）
    let (struct_fqn, struct_name) = match lower.type_kind(base_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => (nominal.fqn, lower.fmt_type(base_ty)),
        _ => {
            return Err(ExprTypeError::WithUpdateBaseNotSupported {
                found: lower.fmt_type(base_ty),
                span: base.span.into(),
            });
        }
    };

    if !matches!(
        lower.nominal_decl_kind(&struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::WithUpdateBaseNotSupported {
            found: struct_name,
            span: base.span.into(),
        });
    }

    for u in updates {
        // 嵌套 path 语义（并行求值 + 多层拷贝）留到后续任务；当前只支持 `x: value`。
        if u.path.segments.len() != 1 {
            let path = u
                .path
                .segments
                .iter()
                .map(|seg| source.slice(seg.span))
                .collect::<Vec<_>>()
                .join(".");
            return Err(ExprTypeError::WithUpdateNestedPathNotSupported {
                path,
                span: u.path.span.into(),
            });
        }

        let field = source.slice(u.path.segments[0].span).to_string();
        let field_fqn = format!("{struct_fqn}.{field}");
        let Some(expected_ty) = struct_field_types.get(&field_fqn).copied() else {
            return Err(ExprTypeError::WithUpdateUnknownField {
                struct_name: struct_name.clone(),
                field,
                span: u.path.span.into(),
            });
        };

        let found_ty = infer_expr_type(
            source,
            &u.value,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if found_ty != expected_ty {
            return Err(ExprTypeError::WithUpdateFieldTypeMismatch {
                struct_name: struct_name.clone(),
                field,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: u.value.span.into(),
            });
        }
    }

    Ok(base_ty)
}

fn is_cast_allowed(from: TypeId, to: TypeId, lower: &TypeLowering<'_>) -> bool {
    if from == to {
        return true;
    }

    // spec §4.4：`as`/`as?` 不做值类型转换；当前阶段也不实现 boxing/unboxing，
    // 因此只允许在引用类型之间做运行期检查式转换。
    lower.is_ref(from) && lower.is_ref(to)
}

/// 检查“found 是否可赋值给 expected”（最小子集）。
///
/// 当前阶段仅实现两条最小子类型规则（用于 `return`/不可达分支等）：
/// - `Nothing <: T`（对任意 T，bottom type）
/// - 任意引用类型 `R` 都可赋值给 `Any`（`R <: Any`）。
///
/// 其余更完整的子类型系统（接口、类继承、值类型装箱等）
/// 会在后续任务中逐步补齐。
fn is_type_assignable(
    found: TypeId,
    expected: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if found == expected {
        return true;
    }

    // `Nothing`：不可达/空类型，可以视为任意类型的子类型。
    //
    // 说明：即使 `Nothing` 是值类型，也允许“赋值到引用类型”，因为这种赋值在运行时不会发生：
    // 表达式求值不会返回一个 `Nothing` 的值（它只能通过 `raise/return` 等控制流中止）。
    if found == builtins.nothing {
        return true;
    }

    // spec §2：`Any` 是所有引用类型的顶类型。
    if expected == builtins.any && lower.is_ref(found) {
        return true;
    }

    false
}

fn infer_value_ident_type(
    source: &SourceFile,
    id: &ast::ValueIdent,
    _lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // `true/false` 当前阶段仍以 ident token 形式存在，但语义上属于字面量。
    let name = source.slice(id.span);
    if name == "true" || name == "false" {
        return Ok(builtins.bool_);
    }

    let Some(resolved) = id.resolved.as_ref() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "ident（未 resolve）",
            span: id.span.into(),
        });
    };

    match resolved {
        ast::ResolvedValueRef::Local { decl_span, .. } => locals
            .get(decl_span)
            .copied()
            .ok_or_else(|| ExprTypeError::UnknownLocalValueType {
                name: name.to_string(),
                span: id.span.into(),
            }),
        ast::ResolvedValueRef::TopLevel { fqn } => {
            top_level_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedTopLevelValueType {
                    fqn: fqn.clone(),
                    span: id.span.into(),
                }
            })
        }
    }
}

fn infer_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    callee: &ast::Expr,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    match &callee.kind {
        ast::ExprKind::Ident(id) => {
            let callee_name = source.slice(id.span);
            let Some(resolved) = &id.resolved else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_name.to_string(),
                    span: id.span.into(),
                });
            };

            let (callee_fqn, callee_span) = match resolved {
                ast::ResolvedValueRef::TopLevel { fqn } => (fqn.clone(), id.span),
                ast::ResolvedValueRef::Local { .. } => {
                    // 当前阶段（T0407）只支持直接调用“顶层 fun symbol”，
                    // 不支持通过值调用（函数值/闭包等）。
                    return Err(ExprTypeError::CalleeNotCallable {
                        callee: callee_name.to_string(),
                        span: id.span.into(),
                    });
                }
            };

            // 当前阶段仅支持“当前文件内”的顶层函数调用类型检查（无重载、无默认参数）。
            let Some(sig) = top_level_funs.get(&callee_fqn) else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_fqn,
                    span: callee_span.into(),
                });
            };

            // receiver fun（扩展函数）不能以 `f(args...)` 的形式被直接调用。
            if sig.receiver.is_some() {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_fqn,
                    span: callee_span.into(),
                });
            }

            if args.len() != sig.params.len() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: callee_fqn,
                    expected: sig.params.len(),
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }

            for (idx, (arg, expected_ty)) in args.iter().zip(sig.params.iter().copied()).enumerate()
            {
                // 先做表达式类型推导，再对比参数类型。
                let found_ty = infer_expr_type(
                    source,
                    arg,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;

                if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
                    return Err(ExprTypeError::CallArgTypeMismatch {
                        callee: callee_fqn,
                        index: idx + 1,
                        expected: lower.fmt_type(expected_ty),
                        found: lower.fmt_type(found_ty),
                        span: arg.span.into(),
                    });
                }
            }

            Ok(sig.return_ty)
        }
        ast::ExprKind::MemberAccess { receiver, member } => infer_member_call_expr_type(
            source,
            call_expr,
            receiver.as_ref(),
            member,
            args,
            false,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => infer_member_call_expr_type(
            source,
            call_expr,
            receiver.as_ref(),
            member,
            args,
            true,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        other => Err(ExprTypeError::UnsupportedExpr {
            kind: expr_kind_name(other),
            span: callee.span.into(),
        }),
    }
}

fn infer_member_call_expr_type(
    source: &SourceFile,
    call_expr: &ast::Expr,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    safe: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 receiver：保证 `a?.b()` 中的 `a` 自身也会被覆盖。
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

    let actual_receiver_ty = if safe {
        match lower.type_kind(receiver_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
            _ => {
                return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                    found: lower.fmt_type(receiver_ty),
                    span: receiver.span.into(),
                });
            }
        }
    } else {
        receiver_ty
    };

    // 当前阶段只支持“扩展函数调用”（T0312）：`receiver.member(args...)`。
    // - 若 resolver 已写回 `ExtensionFun`，优先使用；
    // - 否则（例如 `receiver` 为 `T?` 时 resolver 无法静态确定 receiver 类型），
    //   尝试在“当前包”内按同名顶层 fun 查找 receiver fun。
    let callee_fqn = match member.resolved.as_ref() {
        Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => fqn.clone(),
        Some(ast::ResolvedMemberRef::Fun { fqn })
        | Some(ast::ResolvedMemberRef::Value { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: fqn.clone(),
                span: member.span.into(),
            });
        }
        None => {
            let name = source.slice(member.span);
            if lower.pkg_prefix().is_empty() {
                name.to_string()
            } else {
                format!("{}.{}", lower.pkg_prefix(), name)
            }
        }
    };

    let Some(sig) = top_level_funs.get(&callee_fqn) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    let Some(expected_receiver_ty) = sig.receiver else {
        // 不是扩展函数：`receiver.member()` 形式不应绑定到普通顶层函数。
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
        return Err(ExprTypeError::CallReceiverTypeMismatch {
            callee: callee_fqn,
            expected: lower.fmt_type(expected_receiver_ty),
            found: lower.fmt_type(actual_receiver_ty),
            span: receiver.span.into(),
        });
    }

    if args.len() != sig.params.len() {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_fqn,
            expected: sig.params.len(),
            found: args.len(),
            span: call_expr.span.into(),
        });
    }

    for (idx, (arg, expected_ty)) in args.iter().zip(sig.params.iter().copied()).enumerate() {
        let found_ty = infer_expr_type(
            source,
            arg,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(ExprTypeError::CallArgTypeMismatch {
                callee: callee_fqn,
                index: idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.span.into(),
            });
        }
    }

    let ret = if safe {
        lower.ty_option(sig.return_ty)
    } else {
        sig.return_ty
    };

    Ok(ret)
}

/// 收集“当前文件内”的顶层 `val/var` 声明类型（FQN → TypeId）。
///
/// 说明：
/// - 顶层变量的类型注解由 `typecheck::check_file_headers` 强制要求，因此这里可以直接做 lowering；
/// - 该表用于处理表达式中的 `ResolvedValueRef::TopLevel`（变量引用）。
fn collect_top_level_value_types(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, TypeId> = HashMap::new();

    for item in &file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };

        let ast::ValBinding::Name(name) = &v.binding else {
            // 顶层 pattern binding 会在 headers check 中报错；这里仅保持健壮性。
            continue;
        };

        let Some(ty_ref) = &v.ty else {
            continue;
        };

        let local_name = source.slice(name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };

        let ty = lower.lower_type_ref(ty_ref)?;
        map.insert(fqn, ty);
    }

    Ok(map)
}

/// 收集“当前文件内”的顶层 `fun` 声明签名（FQN → FunSig）。
///
/// 当前阶段（最小子集）：
/// - 不处理 type param / overload / default param；
/// - 未显式标注 return type 的函数，暂视为 `Unit`；
/// - 扩展函数（receiver fun）会被收集，用于 `receiver.member()` 与 `receiver?.member()` 调用的类型检查。
fn collect_top_level_fun_signatures(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<HashMap<String, FunSigOwned>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, FunSigOwned> = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let local_name = source.slice(fun.name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };

        let mut params = Vec::with_capacity(fun.params.len());
        for p in &fun.params {
            let Some(ty_ref) = &p.ty else {
                // headers check 已保证参数类型注解存在；这里保持健壮性。
                continue;
            };
            params.push(lower.lower_type_ref(ty_ref)?);
        }

        let receiver = match &fun.receiver {
            Some(r) => Some(lower.lower_type_ref(r)?),
            None => None,
        };

        let return_ty = match &fun.return_ty {
            Some(ret) => lower.lower_type_ref(ret)?,
            None => builtins.unit,
        };

        map.insert(
            fqn,
            FunSigOwned {
                receiver,
                params,
                return_ty,
            },
        );
    }

    Ok(map)
}

fn check_fun_body_exprs(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 函数级的“局部值类型表”（binder decl span → TypeId）。
    //
    // 当前阶段规则（最小子集）：
    // - 参数：必须有类型注解（由 headers check 保证），因此可直接 lowering；
    // - 局部 `val/var`：
    //   - 若显式写了 `: Type`，则以该类型为准，并校验 initializer（若存在）类型匹配；
    //   - 否则若有 initializer，则以 initializer 类型推导；
    //   - 都没有则当前阶段无法推导（后续任务再补齐规则）。
    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    // 可用于 smart cast 的“稳定绑定”（当前阶段仅覆盖：参数 + `val`）。
    let mut stable_bindings: HashSet<Span> = HashSet::new();
    // 可赋值（mutable）的绑定：当前阶段仅覆盖局部 `var`。
    let mut mutable_bindings: HashSet<Span> = HashSet::new();

    for p in &fun.params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
        stable_bindings.insert(p.name.span);
    }

    // 该函数的期望返回类型（T0417）：用于 `return expr?` 的类型检查。
    let expected_return_ty = match &fun.return_ty {
        Some(ret) => lower.lower_type_ref(ret)?,
        None => builtins.unit,
    };

    match &fun.body {
        ast::FunBody::Block(b) => check_block_exprs(
            source,
            b,
            lower,
            builtins,
            &mut locals,
            &mut stable_bindings,
            &mut mutable_bindings,
            Some(expected_return_ty),
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
        ast::FunBody::Missing => {}
    }

    Ok(())
}

fn check_block_exprs(
    source: &SourceFile,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里用“进入时快照 + 退出时回滚”的方式实现最小作用域，不引入额外的数据结构。
    let saved_locals = locals.clone();
    let saved_stable = stable_bindings.clone();
    let saved_mutable = mutable_bindings.clone();

    for stmt in &block.stmts {
        check_stmt_exprs(
            source,
            stmt,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
    }

    *locals = saved_locals;
    *stable_bindings = saved_stable;
    *mutable_bindings = saved_mutable;

    Ok(())
}

fn check_stmt_exprs(
    source: &SourceFile,
    stmt: &ast::Stmt,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    match &stmt.kind {
        ast::StmtKind::Val(v) => check_local_val_decl_exprs(
            source,
            v,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
        ast::StmtKind::Expr(e) => check_expr_stmt(
            source,
            e,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
        ast::StmtKind::Return { return_span, value } => {
            let Some(expected) = expected_return_ty else {
                return Err(ExprTypeError::ReturnNotInFunctionBody {
                    span: (*return_span).into(),
                });
            };

            match value {
                Some(v) => {
                    let found = infer_expr_type(
                        source,
                        v,
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;
                    if !is_type_assignable(found, expected, lower, builtins) {
                        return Err(ExprTypeError::ReturnTypeMismatch {
                            expected: lower.fmt_type(expected),
                            found: lower.fmt_type(found),
                            span: v.span.into(),
                        });
                    }
                }
                None => {
                    // `return` 不带返回值：等价于返回 `Unit`。
                    if expected != builtins.unit {
                        return Err(ExprTypeError::ReturnValueRequired {
                            expected: lower.fmt_type(expected),
                            span: (*return_span).into(),
                        });
                    }
                }
            }
        }
        ast::StmtKind::While { body, .. } => {
            // 当前阶段仅递归进入 body，以支持其中局部绑定的类型推导。
            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
        }
        ast::StmtKind::ComptimeBlock { body, .. } => {
            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
        }
        ast::StmtKind::ComptimeIf(ci) => {
            check_block_exprs(
                source,
                &ci.then_branch,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            if let Some(else_branch) = &ci.else_branch {
                match &**else_branch {
                    ast::ComptimeIfElse::Block(b) => check_block_exprs(
                        source,
                        b,
                        lower,
                        builtins,
                        locals,
                        stable_bindings,
                        mutable_bindings,
                        expected_return_ty,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?,
                    ast::ComptimeIfElse::If(next) => {
                        // 递归跟进 else-if 链。
                        let mut cur: &ast::ComptimeIf = next;
                        loop {
                            check_block_exprs(
                                source,
                                &cur.then_branch,
                                lower,
                                builtins,
                                locals,
                                stable_bindings,
                                mutable_bindings,
                                expected_return_ty,
                                top_level_types,
                                top_level_funs,
                                struct_field_types,
                            )?;
                            match &cur.else_branch {
                                Some(e) => match &**e {
                                    ast::ComptimeIfElse::Block(b) => {
                                        check_block_exprs(
                                            source,
                                            b,
                                            lower,
                                            builtins,
                                            locals,
                                            stable_bindings,
                                            mutable_bindings,
                                            expected_return_ty,
                                            top_level_types,
                                            top_level_funs,
                                            struct_field_types,
                                        )?;
                                        break;
                                    }
                                    ast::ComptimeIfElse::If(next) => cur = next,
                                },
                                None => break,
                            }
                        }
                    }
                }
            }
        }
        ast::StmtKind::ComptimeFor(cf) => {
            check_block_exprs(
                source,
                &cf.body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
        }
        ast::StmtKind::Empty
        | ast::StmtKind::Break { .. }
        | ast::StmtKind::Continue { .. }
        | ast::StmtKind::Missing => {}
    }

    Ok(())
}

fn check_local_val_decl_exprs(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 先类型检查 initializer（语义：局部变量在其声明之后可见，因此 init 内不能引用自身）。
    let init_ty = match &v.init {
        Some(init) => Some(infer_expr_type(
            source,
            init,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?),
        None => None,
    };

    let declared_ty = match &v.ty {
        Some(ty_ref) => Some(lower.lower_type_ref(ty_ref)?),
        None => None,
    };

    if let (Some(expected), Some(found)) = (declared_ty, init_ty) {
        if expected != found {
            // 复用顶层 initializer 的错误码与文本（保持 fixtures 断言稳定）。
            let init = v.init.as_ref().unwrap();
            return Err(ExprTypeError::InitializerTypeMismatch {
                expected: lower.fmt_type(expected),
                found: lower.fmt_type(found),
                span: init.span.into(),
            });
        }
    }

    let inferred = declared_ty.or(init_ty);

    match &v.binding {
        ast::ValBinding::Name(name) => {
            let Some(ty) = inferred else {
                // 当前阶段不支持“无类型注解 + 无 initializer”的局部绑定推导。
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "局部 val/var（缺少类型与 initializer）",
                    span: name.span.into(),
                });
            };
            locals.insert(name.span, ty);
            match v.kind {
                ast::ValKind::Val => {
                    stable_bindings.insert(name.span);
                }
                ast::ValKind::Var => {
                    mutable_bindings.insert(name.span);
                }
            }
        }
        ast::ValBinding::Pattern(pat) => {
            return Err(ExprTypeError::UnsupportedPatternBinding {
                span: pat.span.into(),
            });
        }
    }

    Ok(())
}

fn check_expr_stmt(
    source: &SourceFile,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 当前阶段的表达式语句仅用于支持控制流结构内部的“局部 val/var 推导”回归：
    // - `if (...) { val ... } else { ... }`
    //
    // 其他表达式语句（例如单独的调用）暂不强制 typecheck，以避免在未实现更多 ExprKind
    // 的阶段引入大量不相关的回归失败。
    match &expr.kind {
        ast::ExprKind::Block(b) => check_block_exprs(
            source,
            b,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => check_if_expr_stmt(
            source,
            cond.as_ref(),
            then_branch.as_ref(),
            else_branch.as_deref(),
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式作为语句时：递归进入分支 body，以覆盖其中的局部绑定/控制流。
            check_expr_stmt(
                source,
                subject.as_ref(),
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            for arm in arms {
                check_expr_stmt(
                    source,
                    &arm.body,
                    lower,
                    builtins,
                    locals,
                    stable_bindings,
                    mutable_bindings,
                    expected_return_ty,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }
            Ok(())
        }
        ast::ExprKind::Lambda(lam) => {
            // spec §7.3：Scoop 不支持 non-local return，因此 lambda body 内出现 `return`
            // 需要报错。
            //
            // 说明：当前阶段 lambda 仍未完整 typecheck；这里仅复用现有的“语句层递归”
            // 逻辑来捕获非法 `return`，并保证它不会污染外层局部作用域。
            let mut lambda_locals = locals.clone();
            let mut lambda_stable = stable_bindings.clone();
            let mut lambda_mutable = mutable_bindings.clone();
            check_expr_stmt(
                source,
                lam.body.as_ref(),
                lower,
                builtins,
                &mut lambda_locals,
                &mut lambda_stable,
                &mut lambda_mutable,
                None,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )
        }
        ast::ExprKind::Assign { lhs, rhs, .. } => check_assign_expr_stmt(
            source,
            lhs,
            rhs,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            top_level_types,
            top_level_funs,
            struct_field_types,
        ),
        _ => Ok(()),
    }
}

fn check_if_expr_stmt(
    source: &SourceFile,
    cond: &ast::Expr,
    then_branch: &ast::Expr,
    else_branch: Option<&ast::Expr>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // smart cast（T0413）最小子集：仅识别 `if (x is T)` / `if (x !is T)` 形式，
    // 并且只对“稳定绑定”（参数 + `val`）在对应分支内做类型收窄。
    let smart_cast = detect_smart_cast_for_if_condition(cond, lower, locals, stable_bindings)?;

    // then 分支：在 `x is T` 时收窄；在 `x !is T` 时保持原类型。
    let mut then_locals = locals.clone();
    let mut then_stable = stable_bindings.clone();
    let mut then_mutable = mutable_bindings.clone();
    if let Some(smart_cast) = smart_cast {
        if smart_cast.narrow_in_then {
            then_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
        }
    }
    check_expr_stmt(
        source,
        then_branch,
        lower,
        builtins,
        &mut then_locals,
        &mut then_stable,
        &mut then_mutable,
        expected_return_ty,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // else 分支：在 `x !is T` 且存在 else 时收窄；否则保持原类型。
    if let Some(else_branch) = else_branch {
        let mut else_locals = locals.clone();
        let mut else_stable = stable_bindings.clone();
        let mut else_mutable = mutable_bindings.clone();
        if let Some(smart_cast) = smart_cast {
            if !smart_cast.narrow_in_then {
                else_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
            }
        }

        check_expr_stmt(
            source,
            else_branch,
            lower,
            builtins,
            &mut else_locals,
            &mut else_stable,
            &mut else_mutable,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
    }

    Ok(())
}

fn check_assign_expr_stmt(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // T0416：局部赋值规则最小子集：
    // - 仅允许对局部 `var` 进行赋值
    // - `val` 与参数均不可再次赋值
    // - 先不处理成员赋值（`a.b = ...`）与更复杂的 lhs（留给后续任务）
    match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            let Some(resolved) = id.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（unresolved ident）",
                    span: id.span.into(),
                });
            };

            match resolved {
                ast::ResolvedValueRef::Local { name, decl_span } => {
                    if stable_bindings.contains(decl_span) || !mutable_bindings.contains(decl_span)
                    {
                        return Err(ExprTypeError::AssignmentTargetNotMutable {
                            name: name.clone(),
                            span: id.span.into(),
                        });
                    }

                    // 防御性：var 绑定应当在 locals 类型表中可查询。
                    if !locals.contains_key(decl_span) {
                        return Err(ExprTypeError::UnknownLocalValueType {
                            name: name.clone(),
                            span: id.span.into(),
                        });
                    }
                }
                ast::ResolvedValueRef::TopLevel { .. } => {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "assignment lhs（top-level value）",
                        span: id.span.into(),
                    });
                }
            }
        }
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "assignment lhs（仅支持标识符）",
                span: lhs.span.into(),
            });
        }
    }

    // 递归 typecheck rhs：保证 `x = f()` 这类语句也会覆盖 rhs 中的表达式。
    let _ = infer_expr_type(
        source,
        rhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct SmartCastHint {
    decl_span: Span,
    target_ty: TypeId,
    narrow_in_then: bool,
}

fn detect_smart_cast_for_if_condition(
    cond: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
) -> Result<Option<SmartCastHint>, ExprTypeError> {
    let ast::ExprKind::TypeCheck { expr, op, ty, .. } = &cond.kind else {
        return Ok(None);
    };

    let ast::ExprKind::Ident(id) = &expr.kind else {
        return Ok(None);
    };

    let Some(ast::ResolvedValueRef::Local { decl_span, .. }) = id.resolved.as_ref() else {
        return Ok(None);
    };

    if !stable_bindings.contains(decl_span) {
        return Ok(None);
    }

    let Some(from_ty) = locals.get(decl_span).copied() else {
        return Ok(None);
    };

    let target_ty = lower.lower_type_ref(ty)?;

    // 与 `as/as?` 保持一致：当前阶段只对引用类型做运行期类型检查式收窄。
    if !is_cast_allowed(from_ty, target_ty, lower) {
        return Ok(None);
    }

    Ok(Some(SmartCastHint {
        decl_span: *decl_span,
        target_ty,
        narrow_in_then: matches!(op, ast::TypeCheckOp::Is),
    }))
}

fn expr_kind_name(kind: &ast::ExprKind) -> &'static str {
    match kind {
        ast::ExprKind::Missing => "missing",
        ast::ExprKind::Ident(_) => "ident",
        ast::ExprKind::IntLit => "int literal",
        ast::ExprKind::StringLit => "string literal",
        ast::ExprKind::UnitLit => "unit literal",
        ast::ExprKind::TupleLit { .. } => "tuple literal",
        ast::ExprKind::InterpolatedString { .. } => "interpolated string",
        ast::ExprKind::Block(_) => "block",
        ast::ExprKind::Lambda(_) => "lambda",
        ast::ExprKind::StructLit { .. } => "struct literal",
        ast::ExprKind::If { .. } => "if expression",
        ast::ExprKind::When { .. } => "when expression",
        ast::ExprKind::MemberAccess { .. } => "member access",
        ast::ExprKind::SpliceField { .. } => "splice field access",
        ast::ExprKind::SafeMemberAccess { .. } => "safe member access",
        ast::ExprKind::Call { .. } => "call",
        ast::ExprKind::NamedArg { .. } => "named argument",
        ast::ExprKind::NotNullAssert { .. } => "not-null assertion",
        ast::ExprKind::Unary { .. } => "unary expression",
        ast::ExprKind::Binary { .. } => "binary expression",
        ast::ExprKind::Assign { .. } => "assignment",
        ast::ExprKind::TypeCheck { .. } => "type check (`is`/`!is`)",
        ast::ExprKind::Cast { .. } => "cast (`as`/`as?`)",
        ast::ExprKind::WithUpdate { .. } => "with-update",
    }
}

fn infer_safe_member_access_expr_type(
    source: &SourceFile,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
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
        Some(ast::ResolvedMemberRef::Value { fqn }) => struct_field_types.get(fqn).copied().ok_or_else(|| {
            ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            }
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
                        },
                        span: member.span.into(),
                    });
                }
            }
        }
    };

    Ok(lower.ty_option(field_ty))
}

fn infer_elvis_expr_type(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
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

fn infer_member_access_expr_type(
    source: &SourceFile,
    receiver: &ast::Expr,
    member: &ast::MemberIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // 先递归类型检查 receiver：保证其中的表达式（如 `a().b` 的 `a()`）也会被覆盖。
    let _ = infer_expr_type(
        source,
        receiver,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // 当前阶段（T0408）仅支持 “struct 字段” 的成员访问：依赖 resolver 写回 `member.resolved`
    // 并以 FQN 在当前文件内查找字段类型。
    let Some(resolved) = member.resolved.as_ref() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "member access（未 resolve）",
            span: member.span.into(),
        });
    };

    match resolved {
        ast::ResolvedMemberRef::Value { fqn } => {
            struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })
        }
        ast::ResolvedMemberRef::Fun { fqn }
        | ast::ResolvedMemberRef::ExtensionValue { fqn }
        | ast::ResolvedMemberRef::ExtensionFun { fqn } => {
            Err(ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
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

/// 收集当前文件内所有 struct 字段的声明类型（member FQN → TypeId）。
///
/// 说明：
/// - 仅收集 `struct`（值类型）的字段，匹配 T0408 的最小目标；
/// - 字段来源：
///   - 主构造参数（`struct Point(val x: Int)`）：在语义上等价于字段
///   - type body 内的 `val/var` property（`struct Point { val x: Int }`）
/// - 当前阶段只在单文件内查找（typecheck fixtures 的编译单元即“sysroot + 单文件”）。
fn collect_struct_field_types(
    source: &SourceFile,
    file: &ast::File,
    lower: &mut TypeLowering<'_>,
) -> Result<HashMap<String, TypeId>, ExprTypeError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut map: HashMap<String, TypeId> = HashMap::new();

    for item in &file.items {
        let ast::Item::Type(ty) = item else {
            continue;
        };
        collect_struct_field_types_in_type_decl(source, ty, &pkg_prefix, lower, &mut map)?;
    }

    Ok(map)
}

fn collect_struct_field_types_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let field_name = source.slice(p.name.span);
                let field_fqn = format!("{type_fqn}.{field_name}");
                out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                if let ast::TypeMember::Property(p) = member {
                    let Some(ty_ref) = &p.ty else {
                        continue;
                    };
                    let field_name = source.slice(p.name.span);
                    let field_fqn = format!("{type_fqn}.{field_name}");
                    out.insert(field_fqn, lower.lower_type_ref(ty_ref)?);
                }
            }
        }
    }

    // 无论外层是否 struct，都递归收集 nested type（可能存在 nested struct）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            let ast::TypeMember::Type(nested) = member else {
                continue;
            };
            collect_struct_field_types_in_type_decl(source, nested, &type_fqn, lower, out)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;

    #[test]
    fn nothing_is_assignable_to_any_type() {
        // 该测试不依赖 sysroot 或 resolver 的完整能力；
        // 只验证 typecheck 的“赋值兼容”最小规则：`Nothing <: T`。
        let source = SourceFile::new_virtual("<mem>", "package a\nfun f(): Unit { return }");
        let file = parse_file(&source).unwrap();
        let index = Index::build(&[(&source, &file)]).unwrap();
        let imports = ImportTable::build(&source, &file, &index).unwrap();

        let env = TypeEnv::default();
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let lower = TypeLowering::new(&source, &file, &index, &imports, &env, &mut types, builtins);

        assert!(is_type_assignable(
            builtins.nothing,
            builtins.any,
            &lower,
            builtins
        ));
        assert!(is_type_assignable(
            builtins.nothing,
            builtins.unit,
            &lower,
            builtins
        ));
        assert!(is_type_assignable(
            builtins.nothing,
            builtins.bool_,
            &lower,
            builtins
        ));

        // 反例：普通值类型不应在 v0 阶段隐式互转。
        assert!(!is_type_assignable(
            builtins.unit,
            builtins.bool_,
            &lower,
            builtins
        ));
    }
}
