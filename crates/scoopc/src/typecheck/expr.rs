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
use crate::ty::{BuiltinTypes, TypeId, TypeStore};

use super::lower::{TypeLowerError, TypeLowering};
use super::TypeEnv;

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
}

#[derive(Debug, Clone)]
struct FunSigOwned {
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
        ast::ExprKind::TypeCheck { expr: inner, ty, .. } => {
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
            // - 否则 fallback 为 `Any`（真正的 LUB 规则留到后续任务实现）。
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

                match result {
                    None => result = Some(arm_ty),
                    Some(prev) if prev == arm_ty => {}
                    Some(_) => return Ok(builtins.any),
                }
            }

            Ok(result.unwrap_or(builtins.any))
        }
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

fn is_cast_allowed(from: TypeId, to: TypeId, lower: &TypeLowering<'_>) -> bool {
    if from == to {
        return true;
    }

    // spec §4.4：`as`/`as?` 不做值类型转换；当前阶段也不实现 boxing/unboxing，
    // 因此只允许在引用类型之间做运行期检查式转换。
    lower.is_ref(from) && lower.is_ref(to)
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
        ast::ResolvedValueRef::TopLevel { fqn } => top_level_types
            .get(fqn)
            .copied()
            .ok_or_else(|| ExprTypeError::UnsupportedTopLevelValueType {
                fqn: fqn.clone(),
                span: id.span.into(),
            }),
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
    let (callee_fqn, callee_span) = match &callee.kind {
        ast::ExprKind::Ident(id) => {
            let callee_name = source.slice(id.span);
            let Some(resolved) = &id.resolved else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_name.to_string(),
                    span: id.span.into(),
                });
            };

            match resolved {
                ast::ResolvedValueRef::TopLevel { fqn } => (fqn.clone(), id.span),
                ast::ResolvedValueRef::Local { .. } => {
                    // 当前阶段（T0407）只支持直接调用“顶层 fun symbol”，
                    // 不支持通过值调用（函数值/闭包等）。
                    return Err(ExprTypeError::CalleeNotCallable {
                        callee: callee_name.to_string(),
                        span: id.span.into(),
                    });
                }
            }
        }
        other => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: expr_kind_name(other),
                span: callee.span.into(),
            });
        }
    };

    // 当前阶段（T0407）仅支持“当前文件内”的顶层函数调用类型检查（无重载、无默认参数）。
    let Some(sig) = top_level_funs.get(&callee_fqn) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: callee_span.into(),
        });
    };

    if args.len() != sig.params.len() {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_fqn,
            expected: sig.params.len(),
            found: args.len(),
            span: call_expr.span.into(),
        });
    }

    for (idx, (arg, expected_ty)) in args.iter().zip(sig.params.iter().copied()).enumerate() {
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

        if found_ty != expected_ty {
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
/// 当前阶段（T0407）限制：
/// - 仅收集“非 receiver”的普通顶层函数（排除扩展函数）；
/// - 不处理 type param / overload / default param；
/// - 未显式标注 return type 的函数，暂视为 `Unit`。
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

        if fun.receiver.is_some() {
            continue;
        }

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

        let return_ty = match &fun.return_ty {
            Some(ret) => lower.lower_type_ref(ret)?,
            None => builtins.unit,
        };

        map.insert(
            fqn,
            FunSigOwned {
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

    for p in &fun.params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
        stable_bindings.insert(p.name.span);
    }

    match &fun.body {
        ast::FunBody::Block(b) => check_block_exprs(
            source,
            b,
            lower,
            builtins,
            &mut locals,
            &mut stable_bindings,
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
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, FunSigOwned>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    for stmt in &block.stmts {
        check_stmt_exprs(
            source,
            stmt,
            lower,
            builtins,
            locals,
            stable_bindings,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
    }
    Ok(())
}

fn check_stmt_exprs(
    source: &SourceFile,
    stmt: &ast::Stmt,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
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
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
        ast::StmtKind::While { body, .. } => {
            // 当前阶段仅递归进入 body，以支持其中局部绑定的类型推导。
            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
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
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            if let Some(else_branch) = &ci.else_branch {
                match &**else_branch {
                    ast::ComptimeIfElse::Block(b) => {
                        check_block_exprs(
                            source,
                            b,
                            lower,
                            builtins,
                            locals,
                            stable_bindings,
                            top_level_types,
                            top_level_funs,
                            struct_field_types,
                        )?
                    }
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
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
        }
        ast::StmtKind::Empty
        | ast::StmtKind::Return { .. }
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
            if matches!(v.kind, ast::ValKind::Val) {
                stable_bindings.insert(name.span);
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
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // else 分支：在 `x !is T` 且存在 else 时收窄；否则保持原类型。
    if let Some(else_branch) = else_branch {
        let mut else_locals = locals.clone();
        let mut else_stable = stable_bindings.clone();
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
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
    }

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
        ast::ResolvedMemberRef::Value { fqn } => struct_field_types
            .get(fqn)
            .copied()
            .ok_or_else(|| ExprTypeError::UnsupportedMemberAccess {
                fqn: fqn.clone(),
                span: member.span.into(),
            }),
        ast::ResolvedMemberRef::Fun { fqn }
        | ast::ResolvedMemberRef::ExtensionValue { fqn }
        | ast::ResolvedMemberRef::ExtensionFun { fqn } => Err(ExprTypeError::UnsupportedMemberAccess {
            fqn: fqn.clone(),
            span: member.span.into(),
        }),
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
