use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::monomorph::{MonomorphKey, MonomorphRequest};
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, TypeId, TypeStore};

use super::call::{
    check_call_arg_named_rules, check_fn_value_to_any_erasure_gate,
    collect_call_arg_infos_allow_expected_type_placeholders, select_ctor_overload_for_owner,
};
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
use super::util::package_prefix;

use super::{ExprInferInputs, ExprTypeError, FunSigOwned, ProgramBoundaryKind};

use super::super::TypeEnv;
use super::super::assignable::is_type_assignable;
use super::super::builtin_annotations::BuiltinAnnotationFlags;
use super::super::builtin_annotations::collect_file_warning_suppressions;
use super::super::lower::{TypeInstantiationKey, TypeLowering, WhereBoundEntry};
use super::super::val_pat;
use crate::warnings;

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

#[derive(Clone)]
struct CheckFileExprsPassResult {
    inferred_expr_tys: HashMap<Span, TypeId>,
    inferred_binding_tys: HashMap<Span, TypeId>,
    inferred_fun_return_tys: HashMap<Span, TypeId>,
    inferred_performed_effect_tys: HashMap<Span, TypeId>,
    inferred_handle_arm_effect_tys: HashMap<Span, TypeId>,
    inferred_handle_arm_op_type_args: HashMap<Span, Vec<TypeId>>,
    safe_member_access_resolved: HashMap<Span, ast::ResolvedMemberRef>,
    typechecked_member_resolved: HashMap<Span, ast::ResolvedMemberRef>,
    splice_field_contracts: HashMap<Span, ast::SpliceFieldContract>,
    with_update_contracts: HashMap<Span, ast::WithUpdateContract>,
    assign_place_contracts: HashMap<Span, ast::AssignPlaceContract>,
    continuation_resume_call_sites: HashSet<Span>,
    non_pure_continuation_resume_call_sites: HashSet<Span>,
    zero_arg_unit_call_sugar_sites: HashSet<Span>,
    top_level_fun_value_refs: HashMap<Span, ast::TopLevelFunValueRef>,
    top_level_fun_call_bindings: HashMap<Span, ast::TopLevelFunCallBinding>,
    typechecked_call_arg_bindings: HashMap<Span, ast::CallArgBinding>,
    typechecked_effect_op_call_bindings: HashMap<Span, ast::EffectOpCallBinding>,
    typechecked_ctor_call_bindings: HashMap<Span, ast::CtorCallBinding>,
    monomorph_requests: Vec<MonomorphRequest>,
    type_instantiation_keys: Vec<TypeInstantiationKey>,
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
    let requests = check_file_exprs_with_monomorph_requests(
        source, file, index, imports, env, types, builtins,
    )?;
    Ok(monomorph_request_keys(requests))
}

/// 对一个文件的表达式做最小类型检查，并在成功时返回带 call-site 来源的单态化请求集合。
pub fn check_file_exprs_with_monomorph_requests(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<Vec<MonomorphRequest>, ExprTypeError> {
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
    let (monomorph_requests, type_insts) = check_file_exprs_impl(
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
    )?;
    Ok((monomorph_request_keys(monomorph_requests), type_insts))
}

fn check_file_exprs_impl(
    request: CheckFileExprsRequest<'_>,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
) -> Result<(Vec<MonomorphRequest>, Vec<TypeInstantiationKey>), ExprTypeError> {
    let file = request.file;
    reset_file_expr_side_tables(file);

    let first_pass = check_file_exprs_pass(request, index, imports, env, types, HashMap::new())?;
    apply_check_file_exprs_pass_result(file, &first_pass);
    Ok((
        first_pass.monomorph_requests,
        first_pass.type_instantiation_keys,
    ))
}

fn reset_file_expr_side_tables(file: &ast::File) {
    file.replace_inferred_expr_tys(HashMap::new());
    file.replace_inferred_binding_tys(HashMap::new());
    file.replace_inferred_fun_return_tys(HashMap::new());
    file.replace_inferred_performed_effect_tys(HashMap::new());
    file.replace_inferred_handle_arm_effect_tys(HashMap::new());
    file.replace_inferred_handle_arm_op_type_args(HashMap::new());
    file.replace_safe_member_access_resolved(HashMap::new());
    file.replace_typechecked_member_resolved(HashMap::new());
    file.replace_splice_field_contracts(HashMap::new());
    file.replace_with_update_contracts(HashMap::new());
    file.replace_assign_place_contracts(HashMap::new());
    file.replace_continuation_resume_call_sites(HashSet::new());
    file.replace_non_pure_continuation_resume_call_sites(HashSet::new());
    file.replace_zero_arg_unit_call_sugar_sites(HashSet::new());
    file.replace_top_level_fun_value_refs(HashMap::new());
    file.replace_top_level_fun_call_bindings(HashMap::new());
    file.replace_typechecked_call_arg_bindings(HashMap::new());
    file.replace_typechecked_effect_op_call_bindings(HashMap::new());
    file.replace_typechecked_ctor_call_bindings(HashMap::new());
}

fn apply_check_file_exprs_pass_result(file: &ast::File, result: &CheckFileExprsPassResult) {
    file.replace_inferred_expr_tys(result.inferred_expr_tys.clone());
    file.replace_inferred_binding_tys(result.inferred_binding_tys.clone());
    file.replace_inferred_fun_return_tys(result.inferred_fun_return_tys.clone());
    file.replace_inferred_performed_effect_tys(result.inferred_performed_effect_tys.clone());
    file.replace_inferred_handle_arm_effect_tys(result.inferred_handle_arm_effect_tys.clone());
    file.replace_inferred_handle_arm_op_type_args(result.inferred_handle_arm_op_type_args.clone());
    file.replace_safe_member_access_resolved(result.safe_member_access_resolved.clone());
    file.replace_typechecked_member_resolved(result.typechecked_member_resolved.clone());
    file.replace_splice_field_contracts(result.splice_field_contracts.clone());
    file.replace_with_update_contracts(result.with_update_contracts.clone());
    file.replace_assign_place_contracts(result.assign_place_contracts.clone());
    file.replace_continuation_resume_call_sites(result.continuation_resume_call_sites.clone());
    file.replace_non_pure_continuation_resume_call_sites(
        result.non_pure_continuation_resume_call_sites.clone(),
    );
    file.replace_zero_arg_unit_call_sugar_sites(result.zero_arg_unit_call_sugar_sites.clone());
    file.replace_top_level_fun_value_refs(result.top_level_fun_value_refs.clone());
    file.replace_top_level_fun_call_bindings(result.top_level_fun_call_bindings.clone());
    file.replace_typechecked_call_arg_bindings(result.typechecked_call_arg_bindings.clone());
    file.replace_typechecked_effect_op_call_bindings(
        result.typechecked_effect_op_call_bindings.clone(),
    );
    file.replace_typechecked_ctor_call_bindings(result.typechecked_ctor_call_bindings.clone());
}

fn check_file_exprs_pass(
    request: CheckFileExprsRequest<'_>,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    _unused_escape_continuation_effect_rows: HashMap<Span, EffectRow>,
) -> Result<CheckFileExprsPassResult, ExprTypeError> {
    let source = request.source;
    let file = request.file;
    let builtins = request.builtins;
    let _warning_suppressions =
        warnings::install_suppressions(collect_file_warning_suppressions(source, file));

    // 这里单独拷贝一份 package 前缀，避免在借用 `lower` 的同时再借用其字段导致借用冲突。
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    // T0629a：program boundary 的 entry point 需要对 cone 边界敏感。
    // 在多 cone 编译单元里，仅把 consumer cone 的 `main` 视为 entry point，
    // 避免把依赖 cone（库）里的同名 `main` 误判为 entry point。
    let file_cone = index.cone_of_source(source);
    let consumer_cone = index.consumer_cone();

    // 顶层函数签名与顶层值类型表：
    // - 函数签名保持“当前文件内声明 + 跨文件按 Index fallback”；
    // - 顶层值类型表现在会补齐无整体注解的顶层 pattern binder 与跨文件静态引用。
    let mut top_level_funs = {
        let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
        collect_top_level_fun_signatures(source, file, &mut lower, builtins)?
    };
    let top_level_types =
        collect_top_level_value_types(source, file, index, imports, env, types, builtins)?;
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    if request.collect_monomorph {
        lower.enable_monomorph_collection();
    }
    if request.collect_type_insts {
        lower.enable_type_instantiation_collection();
    }
    let struct_field_types = collect_struct_field_types(source, file, &mut lower)?;
    let member_mutabilities = collect_member_mutabilities(source, file, env);

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
            ast::Item::Object(obj) => check_object_decl_init_exprs(
                FileExprShared {
                    source,
                    file,
                    builtins,
                    top_level_types: &top_level_types,
                    top_level_funs: &top_level_funs,
                    member_mutabilities: &member_mutabilities,
                    struct_field_types: &struct_field_types,
                },
                obj,
                &pkg_prefix,
                &mut lower,
            )?,
            ast::Item::ExtensionProperty(_) | ast::Item::TypeAlias(_) => {}
        }
    }

    let monomorph_requests = lower.take_monomorph_requests();
    let type_instantiation_keys = lower.take_type_instantiation_keys();
    Ok(CheckFileExprsPassResult {
        inferred_expr_tys: lower.take_inferred_expr_tys(),
        inferred_binding_tys: lower.take_inferred_binding_tys(),
        inferred_fun_return_tys: lower.take_inferred_fun_return_tys(),
        inferred_performed_effect_tys: lower.take_inferred_performed_effect_tys(),
        inferred_handle_arm_effect_tys: lower.take_inferred_handle_arm_effect_tys(),
        inferred_handle_arm_op_type_args: lower.take_inferred_handle_arm_op_type_args(),
        safe_member_access_resolved: lower.take_safe_member_access_resolutions(),
        typechecked_member_resolved: lower.take_typechecked_member_resolutions(),
        splice_field_contracts: lower.take_splice_field_contracts(),
        with_update_contracts: lower.take_with_update_contracts(),
        assign_place_contracts: lower.take_assign_place_contracts(),
        continuation_resume_call_sites: lower.take_continuation_resume_call_sites(),
        non_pure_continuation_resume_call_sites: lower
            .take_non_pure_continuation_resume_call_sites(),
        zero_arg_unit_call_sugar_sites: lower.take_zero_arg_unit_call_sugar_sites(),
        top_level_fun_value_refs: lower.take_top_level_fun_value_refs(),
        top_level_fun_call_bindings: lower.take_top_level_fun_call_bindings(),
        typechecked_call_arg_bindings: lower.take_typechecked_call_arg_bindings(),
        typechecked_effect_op_call_bindings: lower.take_typechecked_effect_op_call_bindings(),
        typechecked_ctor_call_bindings: lower.take_typechecked_ctor_call_bindings(),
        monomorph_requests,
        type_instantiation_keys,
    })
}

fn monomorph_request_keys(requests: Vec<MonomorphRequest>) -> Vec<MonomorphKey> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for request in requests {
        if seen.insert(request.key.clone()) {
            out.push(request.key);
        }
    }
    out
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
                        lambda_this_decl_span: None,
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
                        lambda_this_decl_span: None,
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

    let ctor_params: &[ast::Param] = decl
        .primary_ctor
        .as_ref()
        .map(|c| c.params.as_slice())
        .unwrap_or(&[]);
    let is_annotation_class =
        decl.kind == ast::TypeKind::Class && decl.modifiers.contains(&ast::Modifier::Annotation);

    if matches!(decl.kind, ast::TypeKind::Class | ast::TypeKind::Struct) {
        lower.push_type_params(&decl.type_params);

        let type_where_bounds_pushed = if let Some(wc) = &decl.where_clause {
            let bounds = build_type_where_bound_entries(source, &decl.type_params, wc);
            lower.push_where_bounds(bounds);
            true
        } else {
            false
        };

        let result: Result<(), ExprTypeError> = (|| {
            check_primary_ctor_default_exprs(shared, decl, lower)?;

            if matches!(decl.kind, ast::TypeKind::Struct) {
                check_struct_direct_field_initializer_exprs(shared, decl, ctor_params, lower)?;
            }

            if !is_annotation_class {
                let this_ty_args = decl
                    .type_params
                    .iter()
                    .map(|p| lower.ty_param_from_decl(p))
                    .collect::<Vec<_>>();
                let this_ty = lower.with_warning_emission_suspended(|lower| {
                    lower.lower_type_fqn_with_args(type_fqn.clone(), this_ty_args, decl.name.span)
                })?;

                let member_shared = ClassExprShared {
                    file: shared,
                    this_decl_span: decl.name.span,
                    this_ty,
                    ctor_params,
                };

                let superclass_fqn = matches!(decl.kind, ast::TypeKind::Class)
                    .then(|| {
                        decl.supertypes
                            .iter()
                            .find(|st| st.ctor_args_span.is_some())
                            .and_then(|st| {
                                lower.index().type_ref_to_fqn_in_file(source, file, &st.ty)
                            })
                    })
                    .flatten();

                if matches!(decl.kind, ast::TypeKind::Class) {
                    check_class_super_ctor_call_exprs(shared, &type_fqn, decl, ctor_params, lower)?;
                }

                if let Some(body) = &decl.body {
                    for member in &body.members {
                        match member {
                            ast::TypeMember::Fun(fun) => {
                                check_class_member_fun_body_exprs(member_shared, fun, lower)?;
                            }
                            ast::TypeMember::Property(p)
                                if matches!(decl.kind, ast::TypeKind::Class) =>
                            {
                                check_class_property_initializer_exprs(member_shared, p, lower)?;
                            }
                            ast::TypeMember::InitBlock(b)
                                if matches!(decl.kind, ast::TypeKind::Class) =>
                            {
                                check_class_init_block_exprs(member_shared, b, lower)?;
                            }
                            ast::TypeMember::SecondaryCtor(ctor)
                                if matches!(decl.kind, ast::TypeKind::Class) =>
                            {
                                check_class_secondary_ctor_exprs(
                                    member_shared,
                                    &type_fqn,
                                    decl.primary_ctor.is_some(),
                                    superclass_fqn.as_deref(),
                                    ctor,
                                    lower,
                                )?;
                            }
                            ast::TypeMember::EnumVariant(_)
                            | ast::TypeMember::Type(_)
                            | ast::TypeMember::Object(_)
                            | ast::TypeMember::Property(_)
                            | ast::TypeMember::InitBlock(_)
                            | ast::TypeMember::SecondaryCtor(_) => {}
                        }
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
    if !is_annotation_class && let Some(body) = &decl.body {
        for member in &body.members {
            let ast::TypeMember::Type(nested) = member else {
                continue;
            };
            check_class_member_fun_bodies_in_type_decl(shared, nested, &type_fqn, lower)?;
        }
    }

    Ok(())
}

fn check_primary_ctor_default_exprs(
    shared: FileExprShared<'_>,
    decl: &ast::TypeDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.source;
    let builtins = shared.builtins;
    let Some(primary_ctor) = &decl.primary_ctor else {
        return Ok(());
    };
    let annotation_payload_context =
        decl.kind == ast::TypeKind::Class && decl.modifiers.contains(&ast::Modifier::Annotation);

    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    for p in &primary_ctor.params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = if annotation_payload_context {
            lower.with_annotation_types_allowed(|lower| lower.lower_type_ref(ty_ref))?
        } else {
            lower.lower_type_ref(ty_ref)?
        };
        locals.insert(p.name.span, ty);

        let Some(default_value) = &p.default_value else {
            continue;
        };

        let type_name = source.slice(decl.name.span).to_string();
        let param_name = source.slice(p.name.span).to_string();
        let found_ty = expr_infer_inputs(shared.stmt_shared(), &locals).infer_in_expected(
            lower,
            default_value,
            ty,
            ExpectedTypeFrom::new(format!(
                "`{}` 主构造参数 `{}` 的默认值",
                type_name, param_name
            )),
        )?;

        if is_type_assignable(found_ty, ty, lower, builtins)
            || literal_absorbs_to_expected(default_value, ty, source, lower, builtins)
        {
            continue;
        }

        return Err(ExprTypeError::DefaultParamValueTypeMismatch {
            fun: format!("{}::<ctor>", type_name),
            param: param_name,
            expected: lower.fmt_type(ty),
            found: lower.fmt_type(found_ty),
            span: default_value.span.into(),
        });
    }

    Ok(())
}

fn check_struct_direct_field_initializer_exprs(
    shared: FileExprShared<'_>,
    decl: &ast::TypeDecl,
    ctor_params: &[ast::Param],
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.source;
    let builtins = shared.builtins;
    let Some(body) = &decl.body else {
        return Ok(());
    };

    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    for p in ctor_params {
        let Some(ty_ref) = &p.ty else {
            continue;
        };
        let ty = lower.lower_type_ref(ty_ref)?;
        locals.insert(p.name.span, ty);
    }

    for member in &body.members {
        let ast::TypeMember::Property(p) = member else {
            continue;
        };
        if !p.is_direct_field() {
            continue;
        }
        let Some(init) = &p.init else {
            continue;
        };
        let Some(ty_ref) = &p.ty else {
            continue;
        };

        let expected = lower.lower_type_ref(ty_ref)?;
        let found = expr_infer_inputs(shared.stmt_shared(), &locals).infer_in_expected(
            lower,
            init,
            expected,
            ExpectedTypeFrom::new(format!(
                "struct `{}` 字段 `{}` 的默认值",
                source.slice(decl.name.span),
                source.slice(p.name.span)
            )),
        )?;

        if is_type_assignable(found, expected, lower, builtins)
            || literal_absorbs_to_expected(init, expected, source, lower, builtins)
        {
            continue;
        }

        return Err(ExprTypeError::InitializerTypeMismatch {
            expected: lower.fmt_type(expected),
            found: lower.fmt_type(found),
            span: init.span.into(),
        });
    }

    Ok(())
}

fn check_object_decl_init_exprs(
    shared: FileExprShared<'_>,
    obj: &ast::ObjectDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let source = shared.source;
    let Some(obj_name) = obj
        .name
        .as_ref()
        .map(|id| source.slice(id.span).to_string())
        .or_else(|| match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        })
    else {
        return Ok(());
    };
    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };
    let this_decl_span = obj.name.as_ref().map(|name| name.span).unwrap_or(obj.span);
    let this_ty = lower.with_warning_emission_suspended(|lower| {
        lower.lower_type_fqn_with_args(obj_fqn.clone(), Vec::new(), this_decl_span)
    })?;
    let object_shared = ClassExprShared {
        file: shared,
        this_decl_span,
        this_ty,
        ctor_params: &[],
    };

    if let Some(body) = &obj.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    check_class_member_fun_body_exprs(object_shared, fun, lower)?;
                }
                ast::TypeMember::Property(p) => {
                    check_object_property_initializer_exprs(shared, &obj_fqn, p, lower)?;
                }
                ast::TypeMember::InitBlock(b) => {
                    check_object_init_block_exprs(shared, &obj_fqn, b, lower)?;
                }
                ast::TypeMember::Type(nested) => {
                    check_class_member_fun_bodies_in_type_decl(shared, nested, &obj_fqn, lower)?;
                }
                ast::TypeMember::Object(nested) => {
                    check_object_decl_init_exprs(shared, nested, &obj_fqn, lower)?;
                }
                ast::TypeMember::EnumVariant(_) | ast::TypeMember::SecondaryCtor(_) => {}
            }
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
        if let Some(expr) = eff_param.default.as_ref()
            && let Err(e) = lower.lower_effect_row_expr(Some(expr))
        {
            lower.pop_type_params(&fun.type_params);
            return Err(e.into());
        }
        lower.push_effect_row_param_marker_binding(name, eff_param.name.span);
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
                        let inferred = inferred.unwrap_or(builtins.unit);
                        lower.record_inferred_fun_return_ty(fun.name.span, inferred);
                        inferred
                    }
                    ast::FunBody::Missing => {
                        lower.record_inferred_fun_return_ty(fun.name.span, builtins.unit);
                        builtins.unit
                    }
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
                            lambda_this_decl_span: None,
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
            check_required_effects_for_fun_decl(
                fun,
                &performed_effects,
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
    if p.delegate.is_some() {
        let (locals, _, _) = class_init_locals(shared, lower)?;
        return check_standard_delegated_property_inline_exprs(
            shared.stmt_shared(),
            source,
            p,
            &locals,
            lower,
        );
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

fn check_object_property_initializer_exprs(
    shared: FileExprShared<'_>,
    owner: &str,
    p: &ast::PropertyDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    lower.begin_effect_collection();
    let source = shared.source;
    let builtins = shared.builtins;
    let result = (|| {
        if p.delegate.is_some() {
            let empty_locals = HashMap::new();
            return check_standard_delegated_property_inline_exprs(
                shared.stmt_shared(),
                source,
                p,
                &empty_locals,
                lower,
            );
        }

        let Some(init) = &p.init else {
            return Ok(());
        };
        let Some(ty_ref) = &p.ty else {
            return Ok(());
        };

        let expected = lower.lower_type_ref(ty_ref)?;
        let empty_locals = HashMap::new();
        let found = expr_infer_inputs(shared.stmt_shared(), &empty_locals).infer_in_expected(
            lower,
            init,
            expected,
            ExpectedTypeFrom::new(format!(
                "object property `{}` 的类型注解",
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
    })();
    let performed_effects = lower.finish_effect_collection();
    result?;
    reject_static_initializer_effects(
        format!("object `{owner}` 属性 `{}`", source.slice(p.name.span)),
        &performed_effects,
        lower,
    )?;
    Ok(())
}

fn check_standard_delegated_property_inline_exprs(
    shared: StmtExprShared<'_>,
    source: &SourceFile,
    property: &ast::PropertyDecl,
    outer_locals: &HashMap<Span, TypeId>,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let Some(delegate) = &property.delegate else {
        return Ok(());
    };
    let Some(property_ty_ref) = &property.ty else {
        return Ok(());
    };

    let ast::ExprKind::Call { callee, args } = &delegate.kind else {
        return Ok(());
    };
    let Some(delegate_fqn) = unique_top_level_fun_fqn_from_callee(callee) else {
        return Ok(());
    };

    let property_ty = lower.lower_type_ref(property_ty_ref)?;
    let property_name = source.slice(property.name.span);

    match delegate_fqn.as_str() {
        "scoop.delegates.lazy" => {
            let Some(last_arg) = args.last() else {
                return Ok(());
            };
            let ast::ExprKind::Lambda(lambda) = &last_arg.kind else {
                return Ok(());
            };
            if !lambda.params.is_empty() {
                return Ok(());
            }

            lower.with_safe_lambda_context(lambda, |lower| {
                let _ = expr_infer_inputs(shared, outer_locals).infer_in_expected(
                    lower,
                    lambda.body.as_ref(),
                    property_ty,
                    ExpectedTypeFrom::new(format!(
                        "lazy 委托属性 `{property_name}` 的 initializer 返回类型"
                    )),
                )?;
                Ok::<(), ExprTypeError>(())
            })?;
        }
        "scoop.delegates.observable" | "scoop.delegates.vetoable" => {
            let Some(initial) = args.first() else {
                return Ok(());
            };
            let _ = expr_infer_inputs(shared, outer_locals).infer_in_expected(
                lower,
                initial,
                property_ty,
                ExpectedTypeFrom::new(format!("委托属性 `{property_name}` 的初始值")),
            )?;

            let Some(last_arg) = args.last() else {
                return Ok(());
            };
            let ast::ExprKind::Lambda(lambda) = &last_arg.kind else {
                return Ok(());
            };
            if lambda.params.len() != 2 {
                return Ok(());
            }

            let mut callback_locals = outer_locals.clone();
            callback_locals.insert(lambda.params[0].name.span, property_ty);
            callback_locals.insert(lambda.params[1].name.span, property_ty);
            let callback_return_ty = if delegate_fqn == "scoop.delegates.observable" {
                shared.builtins.unit
            } else {
                shared.builtins.bool_
            };
            let callback_kind = if delegate_fqn == "scoop.delegates.observable" {
                "observable"
            } else {
                "vetoable"
            };
            lower.with_safe_lambda_context(lambda, |lower| {
                let _ = expr_infer_inputs(shared, &callback_locals).infer_in_expected(
                    lower,
                    lambda.body.as_ref(),
                    callback_return_ty,
                    ExpectedTypeFrom::new(format!(
                        "{callback_kind} 委托属性 `{property_name}` 的回调返回类型"
                    )),
                )?;
                Ok::<(), ExprTypeError>(())
            })?;
        }
        _ => {}
    }

    Ok(())
}

fn unique_top_level_fun_fqn_from_callee(callee: &ast::Expr) -> Option<String> {
    let ast::ExprKind::Ident(id) = &callee.kind else {
        return None;
    };

    if let Some(call) = id.call.as_ref() {
        let mut funs: Vec<String> = call
            .candidates
            .iter()
            .filter_map(|candidate| match candidate {
                ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                ast::CallCandidate::Constructor { .. } => None,
            })
            .collect();
        funs.sort();
        funs.dedup();
        if funs.len() == 1 {
            return Some(funs[0].clone());
        }
    }

    match id.resolved.as_ref() {
        Some(ast::ResolvedValueRef::TopLevel { fqn }) => Some(fqn.clone()),
        _ => None,
    }
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

/// 在“已知 ctor 所属类型”的前提下，对 ctor call 的 args 做完整参数绑定 typecheck。
///
/// 说明：
/// - 与普通 `Class(...)` 构造调用共用同一套 ctor 绑定逻辑；
/// - 这里不仅验证 named/default 参数规则，还会把“最终选中的 ctor 目标 + 绑定映射”
///   写入 typecheck side table，供 HIR/codegen 复用。
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
    let call_args = collect_call_arg_infos_allow_expected_type_placeholders(inputs, args, lower)?;
    check_call_arg_named_rules(&callee_for_diag, &call_args)?;
    let chosen = select_ctor_overload_for_owner(
        inputs,
        ctor_owner_ty_fqn,
        call_span,
        &callee_for_diag,
        &call_args,
        exclude_ctor_span,
        lower,
    )?;

    lower.record_typechecked_ctor_call_binding(
        call_span,
        chosen.owner_fqn,
        chosen.ctor_span,
        chosen.arg_mapping,
    );

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
            lambda_this_decl_span: None,
        },
    )?;

    Ok(())
}

fn check_object_init_block_exprs(
    shared: FileExprShared<'_>,
    owner: &str,
    b: &ast::InitBlockDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    lower.begin_effect_collection();
    let mut locals: HashMap<Span, TypeId> = HashMap::new();
    let mut stable_bindings: HashSet<Span> = HashSet::new();
    let mut mutable_bindings: HashSet<Span> = HashSet::new();

    let mut state = StmtExprState {
        locals: &mut locals,
        stable_bindings: &mut stable_bindings,
        mutable_bindings: &mut mutable_bindings,
    };
    let result = check_block_exprs(
        shared.stmt_shared(),
        &b.body,
        lower,
        &mut state,
        StmtExprFlow {
            loop_depth: 0,
            expected_return_ty: None,
            lambda_this_decl_span: None,
        },
    );
    let performed_effects = lower.finish_effect_collection();
    result?;
    reject_static_initializer_effects(
        format!("object `{owner}` init block"),
        &performed_effects,
        lower,
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
            lambda_this_decl_span: None,
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
    lower.begin_effect_collection();
    let found_result = (|| {
        let empty_locals = HashMap::new();
        let declared_ty = match &v.ty {
            Some(ty_ref) => Some(lower.lower_type_ref(ty_ref)?),
            None => None,
        };
        let expected_from = match &v.binding {
            ast::ValBinding::Name(name) => {
                ExpectedTypeFrom::new(format!("顶层绑定 `{}` 的类型注解", source.slice(name.span)))
            }
            ast::ValBinding::Pattern(_) => ExpectedTypeFrom::new("顶层解构绑定的类型注解"),
        };
        let found = match declared_ty {
            Some(expected) => ExprInferInputs {
                source,
                builtins,
                locals: &empty_locals,
                mutable_bindings: None,
                lambda_this_decl_span: None,
                top_level_types,
                top_level_funs,
                member_mutabilities: None,
                struct_field_types,
                loop_depth: 0,
                expected_return_ty: None,
            }
            .infer_in_expected(lower, init, expected, expected_from),
            None => ExprInferInputs {
                source,
                builtins,
                locals: &empty_locals,
                mutable_bindings: None,
                lambda_this_decl_span: None,
                top_level_types,
                top_level_funs,
                member_mutabilities: None,
                struct_field_types,
                loop_depth: 0,
                expected_return_ty: None,
            }
            .infer(lower, init),
        }?;
        Ok::<_, ExprTypeError>((declared_ty, found))
    })();
    let performed_effects = lower.finish_effect_collection();
    let (declared_ty, found) = found_result?;

    if let Some(expected) = declared_ty
        && !is_type_assignable(found, expected, lower, builtins)
        && !literal_absorbs_to_expected(init, expected, source, lower, builtins)
    {
        return Err(ExprTypeError::InitializerTypeMismatch {
            expected: lower.fmt_type(expected),
            found: lower.fmt_type(found),
            span: init.span.into(),
        });
    }

    if let ast::ValBinding::Pattern(pat) = &v.binding {
        let bindings = val_pat::infer_val_pat_bindings(
            source,
            pat,
            found,
            lower,
            builtins,
            struct_field_types,
        )?;
        for (decl_span, ty) in bindings {
            lower.record_inferred_binding_ty(decl_span, ty);
        }
    }

    let owner = match &v.binding {
        ast::ValBinding::Name(name) => {
            format!("顶层绑定 `{}`", source.slice(name.span))
        }
        ast::ValBinding::Pattern(_) => "顶层解构绑定".to_string(),
    };
    reject_static_initializer_effects(owner, &performed_effects, lower)?;

    Ok(())
}

fn reject_static_initializer_effects(
    owner: String,
    performed_effects: &[(TypeId, Span)],
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let Some((effect, span)) = performed_effects.first().copied() else {
        return Ok(());
    };
    Err(ExprTypeError::StaticInitializerMustBePure {
        owner,
        required: lower.fmt_type(effect),
        span: span.into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parser::parse_file;
    use crate::resolve;
    use crate::session::Session;
    use crate::typecheck;

    fn setup_typed_file(
        source_text: &str,
    ) -> (
        SourceFile,
        ast::File,
        Index,
        ImportTable,
        TypeEnv,
        TypeStore,
        BuiltinTypes,
    ) {
        let session = Session::new().expect("session");
        let source = SourceFile::new_virtual("<t4008b2-entry>", source_text);
        let mut ast = parse_file(&source).expect("parse");

        typecheck::check_file_headers(&source, &ast).expect("headers");
        typecheck::check_file_struct_decls(&source, &ast).expect("struct decls");

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in session.sysroot().index_files() {
                unit.push((&file.source, &file.ast));
            }
            unit.push((&source, &ast));
            Index::build(&unit).expect("index")
        };
        let headers = resolve::check_file_headers(&source, &ast, &index).expect("resolve headers");
        resolve::check_file_bodies(&source, &mut ast, &index, &headers).expect("resolve bodies");
        let imports = headers.imports.clone();

        let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).expect("type env");
        env.extend_from_file(&source, &ast, &index)
            .expect("extend type env");

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        typecheck::check_file_annotations(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect("annotations");
        typecheck::check_file_properties(&source, &ast, &index, &env).expect("properties");
        typecheck::check_file_inheritance(&source, &ast, &index).expect("inheritance");
        typecheck::check_file_interfaces(&source, &ast, &index, &env).expect("interfaces");
        typecheck::check_file_override_effects(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect("override effects");
        typecheck::check_file_type_refs(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect("type refs");
        typecheck::check_file_where_clauses(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .expect("where clauses");
        typecheck::check_file_overload_conflicts(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect("overload conflicts");

        (source, ast, index, imports, env, types, builtins)
    }

    fn escape_arm_spans(file: &ast::File) -> (Span, Span) {
        for item in &file.items {
            if let ast::Item::Fun(fun) = item
                && let ast::FunBody::Block(body) = &fun.body
            {
                for stmt in &body.stmts {
                    if let ast::StmtKind::Return {
                        value: Some(expr), ..
                    } = &stmt.kind
                        && let ast::ExprKind::Handle { arms, .. } = &expr.kind
                    {
                        for arm in arms {
                            if let ast::HandleArmKind::EscapeContinuation { k_span } = arm.kind {
                                return (arm.span, k_span);
                            }
                        }
                    }
                }
            }
        }
        panic!("expected an escape continuation arm");
    }

    const ESCAPE_BINDER_SOURCE: &str = r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun next(): Int
}

private fun demo(): Int {
    return handle {
        val seed: Int = Ask.current()
        val extra: Int = Boom.next()
        seed + extra
    } on {
        Ask.current(), k -> {
            0
        }
    }
}
"#;

    const ESCAPE_BINDER_REQUIRE_PURE_SOURCE: &str = r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun next(): Int
}

fun requirePure(k: Continuation<Int, Int, eff Pure>): Unit {}

fun requireBoom(k: Continuation<Int, Int, eff Boom>): Unit {}

private fun bad(): Int {
    return handle {
        val seed: Int = Ask.current()
        val extra: Int = Boom.next()
        seed + extra
    } on {
        Ask.current(), k -> {
            requirePure(k)
            requireBoom(k)
            0
        }
    }
}
"#;

    const ESCAPE_BINDER_REQUIRE_WRONG_ANSWER_SOURCE: &str = r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

fun requireUnit(k: Continuation<Int, Unit, eff Pure>): Unit {}

private fun bad(): Int {
    return handle {
        val seed: Int = Ask.current()
        seed + 1
    } on {
        Ask.current(), k -> {
            requireUnit(k)
            0
        }
    }
}
"#;

    const UNIT_SINGLE_PARAM_ZERO_ARG_OVERLOAD_SOURCE: &str = r#"
package fixtures.typecheck

fun pick(): Int {
    return 1
}

fun pick(value: Unit): String {
    return "unit"
}

fun run(): Int {
    val exact: Int = pick()
    val via_unit: String = pick(())
    exact
}
"#;

    const UNIT_SINGLE_PARAM_ZERO_ARG_EXTENSION_SOURCE: &str = r#"
package fixtures.typecheck

fun String.needUnit(value: Unit): Int {
    return 1
}

fun run(): Int {
    "hi".needUnit()
}
"#;

    const CONTINUATION_RUNTIME_CTOR_SOURCE: &str = r#"
package fixtures.typecheck

import scoop.core.*

fun bad(): Unit {
    val _k = Continuation<Int, Unit, eff Pure>()
}
"#;

    const FSTRING_DESUGAR_NON_TO_STRING_SOURCE: &str = r#"
package fixtures.typecheck

class NotText(val x: Int)

fun bad(value: NotText): String {
    return f"${value}"
}
"#;

    #[test]
    fn check_file_exprs_retypes_escape_continuation_binder_with_precise_effect_row() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(ESCAPE_BINDER_SOURCE);
        typecheck::check_file_exprs(&source, &ast, &index, &imports, &env, &mut types, builtins)
            .expect("typecheck should succeed");

        let (_, k_span) = escape_arm_spans(&ast);
        let k_ty = ast
            .inferred_binding_ty(k_span)
            .expect("escape continuation binder type");
        assert_eq!(
            types.display(k_ty).to_string(),
            "scoop.core.Continuation<Int, Int, eff a.Boom>"
        );
    }

    #[test]
    fn precise_escape_continuation_type_rejects_require_pure_helper() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(ESCAPE_BINDER_REQUIRE_PURE_SOURCE);
        let err = typecheck::check_file_exprs(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect_err("escape continuation binder should not pass as eff Pure");

        assert!(matches!(err, ExprTypeError::CallArgTypeMismatch { .. }));
    }

    #[test]
    fn precise_escape_continuation_type_rejects_wrong_answer_helper() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(ESCAPE_BINDER_REQUIRE_WRONG_ANSWER_SOURCE);
        let err = typecheck::check_file_exprs(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect_err("escape continuation binder should not pass as wrong answer type");

        assert!(matches!(err, ExprTypeError::CallArgTypeMismatch { .. }));
    }

    #[test]
    fn unit_single_param_zero_arg_prefers_exact_zero_arg_overload() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(UNIT_SINGLE_PARAM_ZERO_ARG_OVERLOAD_SOURCE);
        typecheck::check_file_exprs(&source, &ast, &index, &imports, &env, &mut types, builtins)
            .expect("exact zero-arg overload should win before Unit sugar fallback");

        assert!(
            ast.zero_arg_unit_call_sugar_sites().is_empty(),
            "exact zero-arg overload 命中时不应把调用点记成 Unit sugar"
        );
    }

    #[test]
    fn unit_single_param_zero_arg_extension_call_records_typed_sugar_site() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(UNIT_SINGLE_PARAM_ZERO_ARG_EXTENSION_SOURCE);
        typecheck::check_file_exprs(&source, &ast, &index, &imports, &env, &mut types, builtins)
            .expect("extension call should accept Unit zero-arg sugar in typed stage");

        assert_eq!(
            ast.zero_arg_unit_call_sugar_sites().len(),
            1,
            "extension/member-call 语法的 Unit sugar 应写回 typed side table"
        );
    }

    #[test]
    fn continuation_typecheck_rejects_runtime_construction_of_compiler_owned_continuation() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(CONTINUATION_RUNTIME_CTOR_SOURCE);
        let err = typecheck::check_file_exprs(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect_err("用户代码不应允许直接构造 Continuation");

        assert!(matches!(
            err,
            ExprTypeError::ContinuationNotConstructible { .. }
        ));
    }

    #[test]
    fn fstring_desugar_rejects_non_to_string_expr_part() {
        let (source, ast, index, imports, env, mut types, builtins) =
            setup_typed_file(FSTRING_DESUGAR_NON_TO_STRING_SOURCE);
        let err = typecheck::check_file_exprs(
            &source, &ast, &index, &imports, &env, &mut types, builtins,
        )
        .expect_err("f-string expr part must implement ToString");

        assert!(matches!(
            err,
            ExprTypeError::InterpolationExprNotToString { .. }
        ));
    }
}
