use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::monomorph::MonomorphKey;
use crate::resolve::{ConstructorOverload, ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, TypeId, TypeStore};

use super::call::{check_fn_value_to_any_erasure_gate, is_ctor_visible_from};
use super::collect::{
    collect_member_mutabilities, collect_struct_field_types, collect_top_level_fun_signatures,
    collect_top_level_value_types,
};
use super::infer::ExpectedTypeFrom;
use super::ops::literal_absorbs_to_expected;
use super::stmt::{
    FunBodyCheckInputs, StmtExprFlow, StmtExprShared, StmtExprState, check_block_exprs,
    check_expr_stmt, check_fun_body_exprs, check_required_effects_for_fun_decl, check_stmt_exprs,
    expr_infer_inputs,
};
use super::util::{expr_kind_name, join_overload_signatures, package_prefix};

use super::{ASYNC_EFFECT_FQN, ExprInferInputs, ExprTypeError, FunSigOwned, ProgramBoundaryKind};

use super::super::TypeEnv;
use super::super::assignable::is_type_assignable;
use super::super::builtin_annotations::BuiltinAnnotationFlags;
use super::super::lower::{TypeInstantiationKey, TypeLowering, WhereBoundEntry};

#[derive(Clone, Copy)]
struct CheckFileExprsRequest<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    builtins: BuiltinTypes,
    collect_monomorph: bool,
    collect_type_insts: bool,
}

#[derive(Clone, Copy)]
struct FileExprShared<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    builtins: BuiltinTypes,
    top_level_types: &'a HashMap<String, TypeId>,
    top_level_funs: &'a HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &'a HashMap<String, bool>,
    struct_field_types: &'a HashMap<String, TypeId>,
}

impl<'a> FileExprShared<'a> {
    fn stmt_shared(self) -> StmtExprShared<'a> {
        StmtExprShared {
            source: self.source,
            builtins: self.builtins,
            top_level_types: self.top_level_types,
            top_level_funs: self.top_level_funs,
            member_mutabilities: self.member_mutabilities,
            struct_field_types: self.struct_field_types,
        }
    }
}

#[derive(Clone, Copy)]
struct ClassExprShared<'a> {
    file: FileExprShared<'a>,
    this_decl_span: Span,
    this_ty: TypeId,
    ctor_params: &'a [ast::Param],
}

type ClassInitLocals = (HashMap<Span, TypeId>, HashSet<Span>, HashSet<Span>);

impl<'a> ClassExprShared<'a> {
    fn stmt_shared(self) -> StmtExprShared<'a> {
        self.file.stmt_shared()
    }
}

struct CtorCallCheckRequest<'a> {
    callee_for_diag: String,
    ctor_owner_ty_fqn: &'a str,
    call_span: Span,
    args: &'a [ast::Expr],
    exclude_ctor_span: Option<Span>,
}

/// 对一个文件的表达式做最小类型检查。
///
/// 说明：
/// - 当前只覆盖能明确推导的字面量；
/// - 会进入函数体与 class 成员方法体，但对“普通表达式语句”的覆盖仍是增量推进：
///   只在需要时递归进入 block/if/when 等结构，以避免在语法/类型系统尚未齐全时引入大面积回归。
pub fn check_file_exprs(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let _ = check_file_exprs_impl(
        CheckFileExprsRequest {
            source,
            file,
            builtins,
            collect_monomorph: false,
            collect_type_insts: false,
        },
        index,
        imports,
        env,
        types,
    )?;
    Ok(())
}

/// 对一个文件的表达式做最小类型检查，并在成功时返回单态化（monomorphization）请求集合（T0712）。
///
/// 说明：
/// - 该入口会执行与 `check_file_exprs` 相同的类型检查；
/// - 额外收集“泛型函数调用”的实例化信息，供后续 monomorph pass 生成专用实例并做去重缓存。
pub fn check_file_exprs_with_monomorph_keys(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<Vec<MonomorphKey>, ExprTypeError> {
    let (monomorph, _) = check_file_exprs_impl(
        CheckFileExprsRequest {
            source,
            file,
            builtins,
            collect_monomorph: true,
            collect_type_insts: false,
        },
        index,
        imports,
        env,
        types,
    )?;
    Ok(monomorph)
}

/// 对一个文件的表达式做最小类型检查，并在成功时返回“泛型类型实例化”的集合（T1109）。
pub fn check_file_exprs_with_type_instantiation_keys(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<Vec<TypeInstantiationKey>, ExprTypeError> {
    let (_, type_insts) = check_file_exprs_impl(
        CheckFileExprsRequest {
            source,
            file,
            builtins,
            collect_monomorph: false,
            collect_type_insts: true,
        },
        index,
        imports,
        env,
        types,
    )?;
    Ok(type_insts)
}

/// 对一个文件的表达式做最小类型检查，并在成功时同时返回：
/// - monomorph keys（泛型函数调用实例化）
/// - type instantiation keys（泛型类型实例化）
pub fn check_file_exprs_with_monomorph_and_type_instantiation_keys(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(Vec<MonomorphKey>, Vec<TypeInstantiationKey>), ExprTypeError> {
    check_file_exprs_impl(
        CheckFileExprsRequest {
            source,
            file,
            builtins,
            collect_monomorph: true,
            collect_type_insts: true,
        },
        index,
        imports,
        env,
        types,
    )
}

fn check_file_exprs_impl(
    request: CheckFileExprsRequest<'_>,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
) -> Result<(Vec<MonomorphKey>, Vec<TypeInstantiationKey>), ExprTypeError> {
    let source = request.source;
    let file = request.file;
    let builtins = request.builtins;
    file.replace_inferred_expr_tys(HashMap::new());
    file.replace_inferred_binding_tys(HashMap::new());
    file.replace_safe_member_access_resolved(HashMap::new());
    file.replace_continuation_resume_call_sites(HashSet::new());
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    if request.collect_monomorph {
        lower.enable_monomorph_collection();
    }
    if request.collect_type_insts {
        lower.enable_type_instantiation_collection();
    }

    // 这里单独拷贝一份 package 前缀，避免在借用 `lower` 的同时再借用其字段导致借用冲突。
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    // T0629a：program boundary 的 entry point 需要对 cone 边界敏感。
    // 在多 cone 编译单元里，仅把 consumer cone 的 `main` 视为 entry point，
    // 避免把依赖 cone（库）里的同名 `main` 误判为 entry point。
    let file_cone = index.cone_of_source(source);
    let consumer_cone = index.consumer_cone();

    // 顶层 `val/var` 的类型表：用于在表达式里引用顶层变量时查询其声明类型。
    //
    // 当前阶段约束：
    // - 只支持“当前文件内”的顶层变量（因为 typecheck phase 目前只解析单文件 AST）；
    // - 顶层变量必须有显式类型注解（由 `typecheck::check_file_headers` 保证）。
    let top_level_types = collect_top_level_value_types(source, file, &mut lower)?;
    let mut top_level_funs = collect_top_level_fun_signatures(source, file, &mut lower, builtins)?;
    let struct_field_types = collect_struct_field_types(source, file, &mut lower)?;
    let member_mutabilities = collect_member_mutabilities(source, file);

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
            ast::Item::Fun(fun) => {
                let local_name = source.slice(fun.name.span);
                let fun_fqn = if pkg_prefix.is_empty() {
                    local_name.to_string()
                } else {
                    format!("{pkg_prefix}.{local_name}")
                };
                let program_boundary = if file_cone == consumer_cone
                    && fun.kind == ast::FunDeclKind::Regular
                    && fun.receiver.is_none()
                {
                    let is_selected_main = if let Some(entry) = index.runtime_entry_point() {
                        fun_fqn == entry
                    } else {
                        local_name == "main"
                    };

                    if is_selected_main {
                        ProgramBoundaryKind::Main
                    } else if index.is_export_entry_point(&fun_fqn) {
                        ProgramBoundaryKind::Export
                    } else {
                        ProgramBoundaryKind::None
                    }
                } else {
                    ProgramBoundaryKind::None
                };

                check_fun_body_exprs(
                    &fun_fqn,
                    fun,
                    program_boundary,
                    &mut lower,
                    FunBodyCheckInputs {
                        source,
                        builtins,
                        top_level_types: &top_level_types,
                        top_level_funs: &mut top_level_funs,
                        member_mutabilities: &member_mutabilities,
                        struct_field_types: &struct_field_types,
                    },
                )?;
            }
            ast::Item::Type(ty) => check_class_member_fun_bodies_in_type_decl(
                FileExprShared {
                    source,
                    file,
                    builtins,
                    top_level_types: &top_level_types,
                    top_level_funs: &top_level_funs,
                    member_mutabilities: &member_mutabilities,
                    struct_field_types: &struct_field_types,
                },
                ty,
                &pkg_prefix,
                &mut lower,
            )?,
            ast::Item::ExtensionProperty(_)
            | ast::Item::Object(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    request
        .file
        .replace_inferred_expr_tys(lower.take_inferred_expr_tys());
    request
        .file
        .replace_inferred_binding_tys(lower.take_inferred_binding_tys());
    request
        .file
        .replace_safe_member_access_resolved(lower.take_safe_member_access_resolutions());
    request
        .file
        .replace_continuation_resume_call_sites(lower.take_continuation_resume_call_sites());
    let monomorph = lower.take_monomorph_keys();
    let type_insts = lower.take_type_instantiation_keys();
    Ok((monomorph, type_insts))
}

pub(super) fn try_infer_fun_return_ty_from_block(
    shared: StmtExprShared<'_>,
    body: &ast::Block,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    loop_depth: usize,
) -> Result<Option<TypeId>, ExprTypeError> {
    // T0507：返回类型推断（最小实现）。
    //
    // 当前阶段只支持：
    // - “无显式 return”且 block 以表达式语句结尾：以最后表达式类型作为返回类型；
    // - “唯一的 return”且它是函数体最后一条语句：以该 return 的值类型作为返回类型；
    //
    // 其它情况（多 return、return 不在末尾、或最后表达式暂不可推导）先不推断，保持兼容旧行为：
    // 返回类型仍视为 `Unit`，并由现有的 `return_type_mismatch` 等错误兜底。

    // 注意：返回类型推断依赖“最后表达式 / return value”的类型推导，
    // 而这些表达式往往会引用在函数体中声明的局部变量：
    //
    // ```
    // fun f() {
    //   val x: Any = ...
    //   if (x is String) { ... }
    // }
    // ```
    //
    // 因此这里必须按语句顺序“先走一遍最小语句 typecheck”，把局部绑定写进 `locals`，
    // 再去推导最后表达式/return 的类型；否则会出现 `unknown_local_value_type` 的假错误。

    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里与 `check_block_exprs` 一样用“进入时快照 + 退出时回滚”实现。
    let saved_locals = state.locals.clone();
    let saved_stable = state.stable_bindings.clone();
    let saved_mutable = state.mutable_bindings.clone();

    let mut top_level_return_count = 0usize;
    let mut last_return_ty: Option<TypeId> = None;
    let mut tail_expr_ty: Option<TypeId> = None;

    for (idx, stmt) in body.stmts.iter().enumerate() {
        let is_last = idx + 1 == body.stmts.len();

        match &stmt.kind {
            ast::StmtKind::Return { value, .. } => {
                top_level_return_count += 1;
                if is_last {
                    last_return_ty = Some(match value {
                        Some(v) => expr_infer_inputs(shared, &*state.locals).infer(lower, v)?,
                        None => shared.builtins.unit,
                    });
                }
                // 说明：这里刻意不做 `return` 的“类型匹配检查”，因为 expected return type 尚未确定。
                // 真正的 `return` 校验由下方第二遍 `check_block_exprs` 完成。
            }
            ast::StmtKind::Expr(e) => {
                // 先执行现有的”语句层递归”检查（smart cast / lambda return 门禁等）。
                check_expr_stmt(
                    shared,
                    e,
                    lower,
                    state,
                    StmtExprFlow {
                        loop_depth,
                        expected_return_ty: Some(shared.builtins.unit),
                    },
                )?;

                // T3102：若最后一条表达式语句以 `;` 结尾，不用它推断返回类型。
                if is_last && !stmt.has_trailing_semi {
                    match expr_infer_inputs(shared, &*state.locals).infer(lower, e) {
                        Ok(ty) => tail_expr_ty = Some(ty),
                        Err(ExprTypeError::UnsupportedExpr { .. }) => {
                            // 兼容：statement position 的表达式当前并不总是完整 typecheck；
                            // 若仅因为”未实现某个 ExprKind”而失败，则不启用返回类型推断。
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            _ => {
                // 其它语句：复用现有逻辑以便正确更新 locals/stable/mutable，并递归覆盖子结构。
                check_stmt_exprs(
                    shared,
                    stmt,
                    lower,
                    state,
                    StmtExprFlow {
                        loop_depth,
                        expected_return_ty: Some(shared.builtins.unit),
                    },
                )?;
            }
        }
    }

    *state.locals = saved_locals;
    *state.stable_bindings = saved_stable;
    *state.mutable_bindings = saved_mutable;

    // 推断规则（最小子集）：
    // - 唯一的 top-level return 且它是最后一条语句：返回该 return 的值类型
    // - 没有 top-level return：返回最后表达式语句的类型
    // - 其它情况暂不推断
    if top_level_return_count == 1 {
        Ok(last_return_ty)
    } else if top_level_return_count == 0 {
        Ok(tail_expr_ty)
    } else {
        Ok(None)
    }
}

fn check_class_member_fun_bodies_in_type_decl(
    shared: FileExprShared<'_>,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.source;
    let file = shared.file;
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    // 仅在 class 内启用 member fun body typecheck（T0438）。
    if matches!(decl.kind, ast::TypeKind::Class) {
        let ctor_params: &[ast::Param] = decl
            .primary_ctor
            .as_ref()
            .map(|c| c.params.as_slice())
            .unwrap_or(&[]);

        // `this` 在 class 成员体中可见：resolver 会把 `this` 解析到 `decl.name.span`（T0313）。
        //
        // class 的 type params 在成员体内可见：
        // - 让 `this` 的类型可表示为 `C<T, ...>`（而不是 `C<Any, ...>` 占位）；
        // - 让成员体内出现的 `as T` / `is T` 等 type position 能通过 lowering。
        //
        // 这同样避免了 `where` 约束满足性检查（T0458）对 “未知实参” 的误报：
        // `this: C<T>` 中的 `T` 是 `TypeKind::Param`，约束在该层被视作假设而非此刻验证的条件。
        lower.push_type_params(&decl.type_params);

        // T0130：推入类型声明处的 where 约束，以便成员方法体内可通过 bound 驱动方法分发。
        let type_where_bounds_pushed = if let Some(wc) = &decl.where_clause {
            let bounds = build_type_where_bound_entries(source, &decl.type_params, wc);
            lower.push_where_bounds(bounds);
            true
        } else {
            false
        };

        let result: Result<(), ExprTypeError> = (|| {
            let this_ty_args = decl
                .type_params
                .iter()
                .map(|p| lower.ty_param_from_decl(p))
                .collect::<Vec<_>>();
            let this_ty =
                lower.lower_type_fqn_with_args(type_fqn.clone(), this_ty_args, decl.name.span)?;

            let superclass_fqn = decl
                .supertypes
                .iter()
                .find(|st| st.ctor_args_span.is_some())
                .and_then(|st| lower.index().type_ref_to_fqn_in_file(source, file, &st.ty));

            check_class_super_ctor_call_exprs(shared, &type_fqn, decl, ctor_params, lower)?;

            let class_shared = ClassExprShared {
                file: shared,
                this_decl_span: decl.name.span,
                this_ty,
                ctor_params,
            };

            if let Some(body) = &decl.body {
                for member in &body.members {
                    match member {
                        ast::TypeMember::Fun(fun) => {
                            check_class_member_fun_body_exprs(class_shared, fun, lower)?;
                        }
                        ast::TypeMember::Property(p) => {
                            check_class_property_initializer_exprs(class_shared, p, lower)?;
                        }
                        ast::TypeMember::InitBlock(b) => {
                            check_class_init_block_exprs(class_shared, b, lower)?;
                        }
                        ast::TypeMember::SecondaryCtor(ctor) => {
                            check_class_secondary_ctor_exprs(
                                class_shared,
                                &type_fqn,
                                decl.primary_ctor.is_some(),
                                superclass_fqn.as_deref(),
                                ctor,
                                lower,
                            )?;
                        }
                        ast::TypeMember::EnumVariant(_)
                        | ast::TypeMember::Type(_)
                        | ast::TypeMember::Object(_) => {}
                    }
                }
            }

            Ok(())
        })();

        if type_where_bounds_pushed {
            lower.pop_where_bounds();
        }
        lower.pop_type_params(&decl.type_params);
        result?;
    }

    // 递归处理 nested types（可能存在 nested class）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            let ast::TypeMember::Type(nested) = member else {
                continue;
            };
            check_class_member_fun_bodies_in_type_decl(shared, nested, &type_fqn, lower)?;
        }
    }

    Ok(())
}

fn check_class_member_fun_body_exprs(
    shared: ClassExprShared<'_>,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.file.source;
    let builtins = shared.file.builtins;
    lower.push_type_params(&fun.type_params);
    let eff_binding_pushed = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => match lower.lower_effect_row_expr(Some(expr)) {
                Ok(row) => row,
                Err(e) => {
                    lower.pop_type_params(&fun.type_params);
                    return Err(e.into());
                }
            },
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let builtin_flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);
    let unsafe_ctx_pushed = builtin_flags.is_unsafe;
    let nogc_ctx_pushed = builtin_flags.is_nogc;
    if unsafe_ctx_pushed {
        lower.push_unsafe_context();
    }
    if nogc_ctx_pushed {
        lower.push_nogc_context();
    }
    let const_ctx_pushed = fun.modifiers.contains(&ast::Modifier::Const);
    if const_ctx_pushed {
        lower.push_const_context();
    }

    lower.begin_effect_collection();
    let body_result: Result<(), ExprTypeError> = {
        let check_body = |lower: &mut TypeLowering<'_>| -> Result<(), ExprTypeError> {
            let mut locals: HashMap<Span, TypeId> = HashMap::new();
            let mut stable_bindings: HashSet<Span> = HashSet::new();
            let mut mutable_bindings: HashSet<Span> = HashSet::new();

            // `this`：resolver 使用 `decl.name.span` 作为 decl_span。
            locals.insert(shared.this_decl_span, shared.this_ty);
            stable_bindings.insert(shared.this_decl_span);

            // 若该 member fun 本身是扩展函数（member extension），resolver 会把 `this` 解析到 receiver 的 span；
            // 这里沿用顶层扩展函数的处理方式：receiver 作为一个隐式稳定绑定。
            if let Some(receiver) = &fun.receiver {
                let receiver_ty = lower.lower_type_ref(receiver)?;
                locals.insert(receiver.span(), receiver_ty);
                stable_bindings.insert(receiver.span());
            }

            // 主构造参数：resolver 在 member fun 内把 ctor params 当作外层局部绑定（T0313）。
            for p in shared.ctor_params {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let ty = lower.lower_type_ref(ty_ref)?;
                locals.insert(p.name.span, ty);
                stable_bindings.insert(p.name.span);
            }

            // member fun 自身的参数（与顶层 fun 保持一致）。
            for p in &fun.params {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let ty = lower.lower_type_ref(ty_ref)?;
                locals.insert(p.name.span, ty);
                stable_bindings.insert(p.name.span);

                // T1305：默认参数的默认值表达式需要在声明处通过类型检查。
                //
                // 说明：
                // - 默认值会在调用点求值（Kotlin-like），但其语义与可见性依赖于函数声明本身；
                // - 因此这里在“函数体 typecheck”的入口处把默认值纳入最小 expr typecheck 覆盖，
                //   避免后端/fixture 通过后才在 codegen 阶段暴露不一致行为。
                if let Some(default_value) = &p.default_value {
                    let fun_name = fun.name.text(source).to_string();
                    let param_name = p.name.text(source).to_string();
                    let found_ty = expr_infer_inputs(shared.stmt_shared(), &locals)
                        .infer_in_expected(
                            lower,
                            default_value,
                            ty,
                            ExpectedTypeFrom::new(format!(
                                "`{}` 的形参 `{}` 的默认值",
                                fun_name, param_name
                            )),
                        )?;

                    if is_type_assignable(found_ty, ty, lower, builtins)
                        || literal_absorbs_to_expected(default_value, ty, source, lower, builtins)
                    {
                        continue;
                    }

                    return Err(ExprTypeError::DefaultParamValueTypeMismatch {
                        fun: fun_name,
                        param: param_name,
                        expected: lower.fmt_type(ty),
                        found: lower.fmt_type(found_ty),
                        span: default_value.span.into(),
                    });
                }
            }

            // 函数的期望返回类型：用于 `return expr?` 的检查。
            let expected_return_ty = match &fun.return_ty {
                Some(ret) => lower.lower_type_ref(ret)?,
                None => match &fun.body {
                    ast::FunBody::Block(b) => {
                        let inferred = {
                            let mut state = StmtExprState {
                                locals: &mut locals,
                                stable_bindings: &mut stable_bindings,
                                mutable_bindings: &mut mutable_bindings,
                            };
                            try_infer_fun_return_ty_from_block(
                                shared.stmt_shared(),
                                b,
                                lower,
                                &mut state,
                                0,
                            )?
                        };
                        inferred.unwrap_or(builtins.unit)
                    }
                    ast::FunBody::Missing => builtins.unit,
                },
            };

            match &fun.body {
                ast::FunBody::Block(b) => {
                    let mut state = StmtExprState {
                        locals: &mut locals,
                        stable_bindings: &mut stable_bindings,
                        mutable_bindings: &mut mutable_bindings,
                    };
                    check_block_exprs(
                        shared.stmt_shared(),
                        b,
                        lower,
                        &mut state,
                        StmtExprFlow {
                            loop_depth: 0,
                            expected_return_ty: Some(expected_return_ty),
                        },
                    )?
                }
                ast::FunBody::Missing => {}
            }

            Ok(())
        };

        if builtin_flags.is_safe {
            lower.with_unsafe_context_suspended(|lower| check_body(lower))
        } else {
            check_body(lower)
        }
    };
    let performed_effects = lower.finish_effect_collection();

    let result = match body_result {
        Ok(()) => {
            // T0623：member `async fun` 同样需要把 `Async` 留在 Task 的计算语境内。
            let performed_for_decl = if fun.modifiers.contains(&ast::Modifier::Async) {
                let async_effect = lower.lower_type_fqn_with_args(
                    ASYNC_EFFECT_FQN.to_string(),
                    Vec::new(),
                    fun.name.span,
                )?;
                performed_effects
                    .iter()
                    .copied()
                    .filter(|(effect, _)| *effect != async_effect)
                    .collect::<Vec<_>>()
            } else {
                performed_effects.clone()
            };

            check_required_effects_for_fun_decl(
                fun,
                &performed_for_decl,
                ProgramBoundaryKind::None,
                None,
                lower,
            )?;
            Ok(())
        }
        Err(e) => Err(e),
    };
    if eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    if const_ctx_pushed {
        lower.pop_const_context();
    }
    if nogc_ctx_pushed {
        lower.pop_nogc_context();
    }
    if unsafe_ctx_pushed {
        lower.pop_unsafe_context();
    }
    lower.pop_type_params(&fun.type_params);
    result
}

/// 检查 class 属性 initializer 的最小表达式类型（T0448）。
///
/// 说明：
/// - 仅覆盖 `= expr` initializer（delegate `by expr` 的表达式类型检查留给 delegated property lowering 任务）。
/// - initializer 处于 class 初始化语境：可见 `this` 与主构造参数（resolver 已写回 Local decl_span）。
fn check_class_property_initializer_exprs(
    shared: ClassExprShared<'_>,
    p: &ast::PropertyDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.file.source;
    let builtins = shared.file.builtins;
    // delegated property 的语义由 `typecheck::properties` 覆盖；这里避免引入不完整的 delegate expr typecheck。
    if p.delegate.is_some() {
        return Ok(());
    }

    let Some(init) = &p.init else {
        return Ok(());
    };
    let Some(ty_ref) = &p.ty else {
        // `check_file_headers` 已保证类型注解存在；这里仅做健壮性兜底。
        return Ok(());
    };

    let expected = lower.lower_type_ref(ty_ref)?;

    let (locals, _, _) = class_init_locals(shared, lower)?;

    let found = expr_infer_inputs(shared.stmt_shared(), &locals).infer_in_expected(
        lower,
        init,
        expected,
        ExpectedTypeFrom::new(format!(
            "property `{}` 的类型注解",
            source.slice(p.name.span)
        )),
    )?;

    if is_type_assignable(found, expected, lower, builtins) {
        check_fn_value_to_any_erasure_gate(found, expected, init.span, lower, builtins)?;
        return Ok(());
    }

    if literal_absorbs_to_expected(init, expected, source, lower, builtins) {
        return Ok(());
    }

    Err(ExprTypeError::InitializerTypeMismatch {
        expected: lower.fmt_type(expected),
        found: lower.fmt_type(found),
        span: init.span.into(),
    })
}

/// 检查 class header 的 `: Base(args...)` super ctor args（T1327c）。
///
/// 说明：
/// - 该位置并非普通表达式调用点，因此不依赖 resolver 的 `call candidates`；
/// - 为与当前 LLVM codegen 行为保持一致：ctor overload 选择仅按“参数个数（arity）”做最小匹配；
/// - 更完整的 most-specific 重载规则由普通调用点（`C(...)`）的 typecheck 覆盖（T0454）。
fn check_class_super_ctor_call_exprs(
    shared: FileExprShared<'_>,
    class_fqn: &str,
    decl: &ast::TypeDecl,
    ctor_params: &[ast::Param],
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.source;
    let file = shared.file;
    let Some(superclass) = decl
        .supertypes
        .iter()
        .find(|st| st.ctor_args_span.is_some())
    else {
        return Ok(());
    };

    let Some(base_fqn) = lower
        .index()
        .type_ref_to_fqn_in_file(source, file, &superclass.ty)
    else {
        return Ok(());
    };

    // super ctor args 的可见 locals：仅主构造参数（不引入 `this`）。
    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    for p in ctor_params {
        let Some(ty_ref) = p.ty.as_ref() else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
    }

    check_ctor_call_args_by_arity(
        expr_infer_inputs(shared.stmt_shared(), &locals),
        CtorCallCheckRequest {
            callee_for_diag: format!("{class_fqn} -> {base_fqn}"),
            ctor_owner_ty_fqn: &base_fqn,
            call_span: superclass.span,
            args: &superclass.ctor_args,
            exclude_ctor_span: None,
        },
        lower,
    )?;

    Ok(())
}

/// 在“已知 ctor 所属类型”的前提下，对 ctor call 的 args 做最小 typecheck。
///
/// 当前阶段约束（与 LLVM codegen 对齐）：
/// - 仅按 arity 匹配 ctor overload；
/// - 逐个实参按形参类型做 assignable 检查（允许 int literal 吸收）；
/// - defaults/named/spread/vararg 的完整调用规则留给后续任务补齐。
fn check_ctor_call_args_by_arity(
    inputs: ExprInferInputs<'_>,
    request: CtorCallCheckRequest<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let CtorCallCheckRequest {
        callee_for_diag,
        ctor_owner_ty_fqn,
        call_span,
        args,
        exclude_ctor_span,
    } = request;
    let builtins = inputs.builtins;

    // 当前阶段（T1327c）约束：super ctor args / ctor delegation args 仅支持位置参数。
    //
    // 备注：这些位置并非普通调用点（`callee(args...)`），HIR lowering 也不会把它们转成 `CallArg`，
    // 因此这里必须显式拒绝 `name = value` / `*spread` 语法，以避免后续 lowering/codegen 落到 `todo` 分支。
    for arg in args {
        match &arg.kind {
            ast::ExprKind::NamedArg { .. } | ast::ExprKind::SpreadArg { .. } => {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: expr_kind_name(&arg.kind),
                    span: arg.span.into(),
                });
            }
            _ => {}
        }
    }

    let use_cone = lower.index().cone_of_source(inputs.source);
    let Some(ctors) = lower.index().constructors.get(ctor_owner_ty_fqn).cloned() else {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_for_diag.clone(),
            span: call_span.into(),
        });
    };

    let mut visible: Vec<&ConstructorOverload> = ctors
        .iter()
        .filter(|c| is_ctor_visible_from(use_cone, inputs.source, c))
        .collect();
    if let Some(exclude) = exclude_ctor_span {
        visible.retain(|c| c.span != exclude);
    }

    let mut matching: Vec<&ConstructorOverload> = visible
        .iter()
        .copied()
        .filter(|c| c.params.len() == args.len())
        .collect();

    if matching.is_empty() {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_for_diag.clone(),
            span: call_span.into(),
        });
    }
    if matching.len() != 1 {
        let mut sigs: Vec<String> = Vec::with_capacity(matching.len());
        for ctor in &matching {
            let mut param_ty_strs: Vec<String> = Vec::with_capacity(ctor.params.len());
            for p in &ctor.params {
                let Some(ty_ref) = p.ty.as_ref() else {
                    param_ty_strs.push(lower.fmt_type(builtins.any));
                    continue;
                };
                let ty = lower.lower_type_ref_in_decl_file(&ctor.decl_file, ty_ref)?;
                param_ty_strs.push(lower.fmt_type(ty));
            }
            sigs.push(format!("{ctor_owner_ty_fqn}({})", param_ty_strs.join(", ")));
        }
        return Err(ExprTypeError::AmbiguousOverload {
            callee: callee_for_diag,
            candidates: join_overload_signatures(sigs),
            span: call_span.into(),
        });
    }

    let ctor = matching.pop().expect("len == 1");
    for (idx, (arg, param)) in args.iter().zip(ctor.params.iter()).enumerate() {
        let expected = match param.ty.as_ref() {
            Some(ty_ref) => lower.lower_type_ref_in_decl_file(&ctor.decl_file, ty_ref)?,
            None => builtins.any,
        };

        let found = inputs.infer_in_expected(
            lower,
            arg,
            expected,
            ExpectedTypeFrom::new(format!(
                "constructor `{}` 的第 {} 个参数",
                ctor_owner_ty_fqn,
                idx + 1
            )),
        )?;

        if is_type_assignable(found, expected, lower, builtins) {
            check_fn_value_to_any_erasure_gate(found, expected, arg.span, lower, builtins)?;
            continue;
        }
        if literal_absorbs_to_expected(arg, expected, inputs.source, lower, builtins) {
            continue;
        }

        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: callee_for_diag,
            index: idx + 1,
            expected: lower.fmt_type(expected),
            found: lower.fmt_type(found),
            span: arg.span.into(),
        });
    }

    Ok(())
}

/// 检查 class `init { ... }` 初始化块的最小表达式类型（T0448）。
fn check_class_init_block_exprs(
    shared: ClassExprShared<'_>,
    b: &ast::InitBlockDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let (mut locals, mut stable_bindings, mut mutable_bindings) = class_init_locals(shared, lower)?;

    // init block 不是函数体：`return` 在此处无意义，因此 expected_return_ty = None。
    let mut state = StmtExprState {
        locals: &mut locals,
        stable_bindings: &mut stable_bindings,
        mutable_bindings: &mut mutable_bindings,
    };
    check_block_exprs(
        shared.stmt_shared(),
        &b.body,
        lower,
        &mut state,
        StmtExprFlow {
            loop_depth: 0,
            expected_return_ty: None,
        },
    )?;

    Ok(())
}

/// 检查 class 次构造器 body 的最小表达式类型（T0448）。
fn check_class_secondary_ctor_exprs(
    shared: ClassExprShared<'_>,
    class_fqn: &str,
    has_primary_ctor: bool,
    superclass_fqn: Option<&str>,
    ctor: &ast::SecondaryCtorDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    // Kotlin-like 语义：当 class 有主构造器时，secondary constructor 必须显式委托到 `this(...)`。
    if has_primary_ctor {
        match ctor.delegation_call.as_ref() {
            None => {
                return Err(ExprTypeError::SecondaryCtorDelegationRequired {
                    class_fqn: class_fqn.to_string(),
                    span: ctor.span.into(),
                });
            }
            Some(call) if call.kind != ast::CtorDelegationKind::This => {
                return Err(ExprTypeError::SecondaryCtorDelegationMustBeThis {
                    class_fqn: class_fqn.to_string(),
                    span: call.target_span.into(),
                });
            }
            Some(_) => {}
        }
    }

    // delegation call 的 args 类型检查（T1327c）。
    if let Some(call) = ctor.delegation_call.as_ref() {
        // delegation args 的可见 locals：仅 secondary ctor params（不引入 `this`）。
        let mut delegation_locals: HashMap<Span, TypeId> = HashMap::new();
        for p in &ctor.params {
            let Some(ty_ref) = p.ty.as_ref() else {
                continue;
            };
            let ty = lower.lower_type_ref(ty_ref)?;
            delegation_locals.insert(p.name.span, ty);
        }

        match call.kind {
            ast::CtorDelegationKind::This => {
                check_ctor_call_args_by_arity(
                    expr_infer_inputs(shared.stmt_shared(), &delegation_locals),
                    CtorCallCheckRequest {
                        callee_for_diag: "this".to_string(),
                        ctor_owner_ty_fqn: class_fqn,
                        call_span: call.span,
                        args: &call.args,
                        exclude_ctor_span: Some(ctor.span),
                    },
                    lower,
                )?;
            }
            ast::CtorDelegationKind::Super => {
                let Some(base_fqn) = superclass_fqn else {
                    return Err(ExprTypeError::NoMatchingOverload {
                        callee: "super".to_string(),
                        span: call.span.into(),
                    });
                };
                check_ctor_call_args_by_arity(
                    expr_infer_inputs(shared.stmt_shared(), &delegation_locals),
                    CtorCallCheckRequest {
                        callee_for_diag: format!("super({base_fqn})"),
                        ctor_owner_ty_fqn: base_fqn,
                        call_span: call.span,
                        args: &call.args,
                        exclude_ctor_span: None,
                    },
                    lower,
                )?;
            }
        }
    }

    let (mut locals, mut stable_bindings, mut mutable_bindings) = class_init_locals(shared, lower)?;

    // 次构造器参数：作为函数参数语义处理（稳定绑定；不可赋值）。
    for p in &ctor.params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
        stable_bindings.insert(p.name.span);
    }

    // secondary ctor body 不是函数体：不允许 `return`。
    let mut state = StmtExprState {
        locals: &mut locals,
        stable_bindings: &mut stable_bindings,
        mutable_bindings: &mut mutable_bindings,
    };
    check_block_exprs(
        shared.stmt_shared(),
        &ctor.body,
        lower,
        &mut state,
        StmtExprFlow {
            loop_depth: 0,
            expected_return_ty: None,
        },
    )?;

    Ok(())
}

/// 构造 class 初始化语境（property initializer / `init {}` / ctor body）所需的 locals 集合。
///
/// 说明：
/// - `this` 与主构造参数在 resolver 阶段会被写回为 `ResolvedValueRef::Local { decl_span }`；
/// - 这里把这些 decl_span 映射到 TypeId，供后续 type inference 查询。
fn class_init_locals(
    shared: ClassExprShared<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<ClassInitLocals, ExprTypeError> {
    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    let mut stable_bindings: HashSet<Span> = HashSet::new();
    let mutable_bindings: HashSet<Span> = HashSet::new();

    // `this`：resolver 使用 class name 的 span 作为 decl_span。
    locals.insert(shared.this_decl_span, shared.this_ty);
    stable_bindings.insert(shared.this_decl_span);

    // 主构造参数：在初始化语境内可见（T0313）。
    for p in shared.ctor_params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
        stable_bindings.insert(p.name.span);
    }

    Ok((locals, stable_bindings, mutable_bindings))
}

fn check_top_level_val_initializer(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
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
    let expected_from = match &v.binding {
        ast::ValBinding::Name(name) => {
            ExpectedTypeFrom::new(format!("顶层绑定 `{}` 的类型注解", source.slice(name.span)))
        }
        ast::ValBinding::Pattern(_) => ExpectedTypeFrom::new("顶层解构绑定的类型注解"),
    };
    let empty_locals = HashMap::new();
    let found = ExprInferInputs {
        source,
        builtins,
        locals: &empty_locals,
        top_level_types,
        top_level_funs,
        member_mutabilities: None,
        struct_field_types,
        loop_depth: 0,
        expected_return_ty: None,
    }
    .infer_in_expected(lower, init, expected, expected_from)?;

    if is_type_assignable(found, expected, lower, builtins) {
        return Ok(());
    }

    if literal_absorbs_to_expected(init, expected, source, lower, builtins) {
        return Ok(());
    }

    Err(ExprTypeError::InitializerTypeMismatch {
        expected: lower.fmt_type(expected),
        found: lower.fmt_type(found),
        span: init.span.into(),
    })
}

/// 从类型声明的 `where_clause` 和 `type_params` 构建 `WhereBoundEntry` 列表（T0130）。
fn build_type_where_bound_entries(
    source: &SourceFile,
    type_params: &[ast::TypeParam],
    where_clause: &ast::WhereClause,
) -> Vec<WhereBoundEntry> {
    let param_names: Vec<String> = type_params
        .iter()
        .map(|p| source.slice(p.name.span).to_string())
        .collect();

    let mut out = Vec::new();
    for c in &where_clause.constraints {
        let target_name = source.slice(c.ty_param.span).to_string();
        if !param_names.contains(&target_name) {
            continue;
        }
        out.push(WhereBoundEntry {
            param_name: target_name,
            bound: c.bound.clone(),
            decl_file: source.path().to_path_buf(),
        });
    }
    out
}
