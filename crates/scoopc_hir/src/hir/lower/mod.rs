//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

mod decls;
mod types;
mod util;

#[cfg(test)]
mod placeholder_inventory;

mod patterns;
mod sugar;

mod block;
mod expr;
mod stmt;

pub use types::{HirLowerError, HirStageError, LoweredHir};
pub use util::GenericTemplateSymbolSuffixIndex;
pub use util::{
    canonical_generic_fun_signature_key, canonical_generic_property_getter_signature_key,
    collect_generic_template_symbol_suffixes, stable_instance_fqn,
};
pub use util::{mangle_nominal_fqn, mangle_nominal_fqn_with_eff};

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::parser::parse_file;
use crate::resolve::Index;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::stable_id::StableConeKey;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};

use super::EFFECT_ROW_PARAM_DECL_FILE;
use super::{
    AccessorContract, AssignPlaceSiteIndex, Block, CallArg, CallArgBindingSiteIndex, CallSite,
    ClassInitIndex, ContinuationResumeCallSiteIndex, CtorCallSiteIndex, CtorDecl, CtorParamDecl,
    Decl, DeclMember, DeclTypeParam, EnumVariantDecl, Expr, ExprKind, ExtensionPropertyDecl,
    FieldDecl, FieldOrigin, File, FunDecl, GenericClassDeclIndex, Item, MemberFunDecl, MemberRef,
    NominalDecl, NonPureContinuationResumeCallSiteIndex, ObjectDecl, ObjectInitIndex, Param,
    PropertyDecl, Stmt, StmtKind, SupertypeDecl, SymbolId, TopLevelVarStorage, TypeAliasDecl,
    ValDecl, ValueRef, WithUpdateSiteIndex,
};

use types::*;
use util::*;

fn collect_top_level_fun_call_sites(
    files: &[(&SourceFile, &ast::File)],
) -> crate::hir::TopLevelFunCallSiteIndex {
    let mut sites = HashMap::new();
    for (source, file) in files {
        for (span, binding) in file.top_level_fun_call_bindings() {
            sites.insert(CallSite::new(source.path().to_path_buf(), span), binding);
        }
    }
    sites
}

fn collect_synthetic_named_intrinsic_call_sites(
    index: &Index,
    types: &TypeStore,
    funs: &[FunDecl],
) -> crate::hir::TopLevelFunCallSiteIndex {
    let mut sites = HashMap::new();
    for fun in funs {
        if let Some(body) = &fun.body {
            collect_synthetic_named_intrinsic_call_sites_in_block(
                index,
                types,
                &fun.source_path,
                body,
                &mut sites,
            );
        }
    }
    sites
}

fn collect_synthetic_named_intrinsic_call_sites_for_file(
    index: &Index,
    types: &TypeStore,
    file: &File,
    member_funs: &[FunDecl],
) -> crate::hir::TopLevelFunCallSiteIndex {
    let mut funs = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    funs.extend(member_funs.iter().cloned());
    collect_synthetic_named_intrinsic_call_sites(index, types, &funs)
}

fn collect_synthetic_named_intrinsic_call_sites_in_block(
    index: &Index,
    types: &TypeStore,
    source_path: &std::path::Path,
    block: &Block,
    sites: &mut crate::hir::TopLevelFunCallSiteIndex,
) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Expr(expr) => collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                expr,
                sites,
            ),
            StmtKind::Val(val) => {
                if let Some(init) = &val.init {
                    collect_synthetic_named_intrinsic_call_sites_in_expr(
                        index,
                        types,
                        source_path,
                        init,
                        sites,
                    );
                }
            }
            StmtKind::Return { value } => {
                if let Some(value) = value {
                    collect_synthetic_named_intrinsic_call_sites_in_expr(
                        index,
                        types,
                        source_path,
                        value,
                        sites,
                    );
                }
            }
            StmtKind::Assign { lhs, rhs, .. } => {
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    lhs,
                    sites,
                );
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    rhs,
                    sites,
                );
            }
            StmtKind::While { cond, body } => {
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    cond,
                    sites,
                );
                collect_synthetic_named_intrinsic_call_sites_in_block(
                    index,
                    types,
                    source_path,
                    body,
                    sites,
                );
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }
}

fn collect_synthetic_named_intrinsic_call_sites_in_expr(
    index: &Index,
    types: &TypeStore,
    source_path: &std::path::Path,
    expr: &Expr,
    sites: &mut crate::hir::TopLevelFunCallSiteIndex,
) {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let Some(binding) = named_intrinsic_binding_for_callee(index, callee).or_else(|| {
                synthetic_array_helper_binding_for_call(index, types, expr, callee, args)
            }) {
                sites
                    .entry(CallSite::new(source_path.to_path_buf(), expr.span))
                    .or_insert(binding);
            }
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                callee,
                sites,
            );
            for arg in args {
                match arg {
                    CallArg::Positional(value) | CallArg::Named { value, .. } => {
                        collect_synthetic_named_intrinsic_call_sites_in_expr(
                            index,
                            types,
                            source_path,
                            value,
                            sites,
                        );
                    }
                }
            }
        }
        ExprKind::MemberAccess { receiver, .. }
        | ExprKind::Unary { expr: receiver, .. }
        | ExprKind::TypeCheck { expr: receiver, .. }
        | ExprKind::Cast { expr: receiver, .. } => {
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                receiver,
                sites,
            );
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                lhs,
                sites,
            );
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                rhs,
                sites,
            );
        }
        ExprKind::Block(block) => collect_synthetic_named_intrinsic_call_sites_in_block(
            index,
            types,
            source_path,
            block,
            sites,
        ),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                cond,
                sites,
            );
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                then_branch,
                sites,
            );
            if let Some(else_branch) = else_branch {
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    else_branch,
                    sites,
                );
            }
        }
        ExprKind::When { subject, arms } => {
            collect_synthetic_named_intrinsic_call_sites_in_expr(
                index,
                types,
                source_path,
                subject,
                sites,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_synthetic_named_intrinsic_call_sites_in_expr(
                        index,
                        types,
                        source_path,
                        guard,
                        sites,
                    );
                }
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    &arm.body,
                    sites,
                );
            }
        }
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    &field.value,
                    sites,
                );
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    element,
                    sites,
                );
            }
        }
        ExprKind::Closure(closure) => collect_synthetic_named_intrinsic_call_sites_in_expr(
            index,
            types,
            source_path,
            &closure.body,
            sites,
        ),
        ExprKind::Handle(handle) => {
            collect_synthetic_named_intrinsic_call_sites_in_block(
                index,
                types,
                source_path,
                &handle.body,
                sites,
            );
            for arm in &handle.arms {
                collect_synthetic_named_intrinsic_call_sites_in_expr(
                    index,
                    types,
                    source_path,
                    &arm.body,
                    sites,
                );
            }
            if let Some(finally) = &handle.finally {
                collect_synthetic_named_intrinsic_call_sites_in_block(
                    index,
                    types,
                    source_path,
                    finally,
                    sites,
                );
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(value) | CallArg::Named { value, .. } => {
                        collect_synthetic_named_intrinsic_call_sites_in_expr(
                            index,
                            types,
                            source_path,
                            value,
                            sites,
                        );
                    }
                }
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let super::InterpolatedStringPart::Expr { expr } = part {
                    collect_synthetic_named_intrinsic_call_sites_in_expr(
                        index,
                        types,
                        source_path,
                        expr,
                        sites,
                    );
                }
            }
        }
        ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Missing
        | ExprKind::Todo(_) => {}
    }
}

fn named_intrinsic_binding_for_callee(
    index: &Index,
    callee: &Expr,
) -> Option<ast::TopLevelFunCallBinding> {
    let fqn = match &callee.kind {
        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => fqn,
        ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
            MemberRef::Fun { fqn, .. } | MemberRef::ExtensionFun { fqn, .. } => fqn,
            MemberRef::Value { .. } | MemberRef::ExtensionValue { .. } => return None,
        },
        _ => return None,
    };
    let overload = index.by_fqn.get(fqn)?.fun.iter().find(|overload| {
        overload.sig.builtin_flags.is_intrinsic
            && overload.sig.builtin_flags.intrinsic_entry_name.is_some()
    })?;
    Some(ast::TopLevelFunCallBinding {
        fqn: fqn.clone(),
        decl_file: overload.symbol.decl_file.clone(),
        decl_span: overload.symbol.span,
        is_intrinsic: true,
        intrinsic_entry_name: overload.sig.builtin_flags.intrinsic_entry_name.clone(),
        param_tys: Vec::new(),
        return_ty: None,
        type_args: Vec::new(),
        eff_args: Vec::new(),
        types_are_hir: true,
    })
}

fn synthetic_array_helper_binding_for_call(
    index: &Index,
    types: &TypeStore,
    expr: &Expr,
    callee: &Expr,
    args: &[CallArg],
) -> Option<ast::TopLevelFunCallBinding> {
    let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
        return None;
    };
    let type_args = synthetic_array_helper_type_args(types, fqn, expr.ty, args)?;
    if type_args.is_empty() {
        return None;
    }
    let overload = index
        .by_fqn
        .get(fqn)?
        .fun
        .iter()
        .find(|overload| !overload.sig.type_params.is_empty())?;

    Some(ast::TopLevelFunCallBinding {
        fqn: fqn.clone(),
        decl_file: overload.symbol.decl_file.clone(),
        decl_span: overload.symbol.span,
        is_intrinsic: false,
        intrinsic_entry_name: None,
        param_tys: Vec::new(),
        return_ty: None,
        type_args,
        eff_args: Vec::new(),
        types_are_hir: true,
    })
}

fn synthetic_array_helper_type_args(
    types: &TypeStore,
    fqn: &str,
    result_ty: TypeId,
    args: &[CallArg],
) -> Option<Vec<TypeId>> {
    let ty = match fqn {
        "scoop.core.mutableArrayNew" => {
            nominal_type_arg(types, result_ty, "scoop.core.MutableArray")
        }
        "scoop.core.freeze" => args
            .first()
            .and_then(|arg| nominal_type_arg(types, call_arg_ty(arg), "scoop.core.MutableArray"))
            .or_else(|| nominal_type_arg(types, result_ty, "scoop.core.Array")),
        "scoop.core.push" => args
            .first()
            .and_then(|arg| nominal_type_arg(types, call_arg_ty(arg), "scoop.core.MutableArray"))
            .or_else(|| args.get(1).map(call_arg_ty)),
        _ => None,
    }?;
    Some(vec![ty])
}

fn call_arg_ty(arg: &CallArg) -> TypeId {
    match arg {
        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => expr.ty,
    }
}

fn nominal_type_arg(types: &TypeStore, ty: TypeId, expected_fqn: &str) -> Option<TypeId> {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if nominal.fqn == expected_fqn && nominal.args.len() == 1 =>
        {
            nominal.args.first().copied()
        }
        _ => None,
    }
}

fn collect_top_level_fun_call_sites_with_type_remap(
    files: &[(&SourceFile, &ast::File)],
    typecheck_types: Option<&TypeStore>,
    types: &mut TypeStore,
) -> crate::hir::TopLevelFunCallSiteIndex {
    let mut sites = collect_top_level_fun_call_sites(files);
    let Some(typecheck_types) = typecheck_types else {
        return sites;
    };
    for binding in sites.values_mut() {
        if binding.types_are_hir {
            continue;
        }
        binding.param_tys = binding
            .param_tys
            .iter()
            .map(|&ty| types.re_intern_from(typecheck_types, ty))
            .collect();
        binding.return_ty = binding
            .return_ty
            .map(|ty| types.re_intern_from(typecheck_types, ty));
        binding.type_args = binding
            .type_args
            .iter()
            .map(|&ty| types.re_intern_from(typecheck_types, ty))
            .collect();
        binding.eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                crate::ty::EffectRow::new(
                    row.terms
                        .iter()
                        .map(|&ty| types.re_intern_from(typecheck_types, ty))
                        .collect(),
                )
            })
            .collect();
        binding.types_are_hir = true;
    }
    sites
}

fn collect_call_arg_bindings(files: &[(&SourceFile, &ast::File)]) -> CallArgBindingSiteIndex {
    let mut sites = HashMap::new();
    for (source, file) in files {
        for (span, binding) in file.typechecked_call_arg_bindings() {
            sites.insert(CallSite::new(source.path().to_path_buf(), span), binding);
        }
    }
    sites
}

/// HIR lowering 的上下文（按单文件构建，用于 `dump-hir` 与 HIR fixtures）。
struct HirLowering<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    /// typecheck 阶段的 TypeStore（若存在）。
    ///
    /// 用途：把 `ast::File` side table 中记录的 typecheck `TypeId` 重新 intern 到当前 HIR 的
    /// `TypeStore`，从而在 lowering 阶段恢复“表达式最终类型”。
    typecheck_types: Option<&'a TypeStore>,
    /// `type fqn -> ast::TypeKind` 的最小索引，用于决定 nominal type 是 ref 还是 value。
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    /// delegated property（spec §10.4）索引：`Owner.prop` → lowering 所需的合成符号信息。
    ///
    /// 用途：
    /// - `receiver.prop` 读取：降糖为 `receiver.prop$delegate.getValue(receiver, PropertyMeta)`；
    /// - `receiver.prop = value` 写入：降糖为 `receiver.prop$delegate.setValue(receiver, PropertyMeta, value)`。
    ///
    /// `$delegate` 字段由 class init side table 初始化，`PropertyMeta` 在调用点按值合成。
    delegated_properties: &'a DelegatedPropertyIndex,
    /// 当前 lowering 可见的完整编译单元 AST（含 sysroot/同编译单元其它文件）。
    ///
    /// 用途：
    /// - 跨文件 struct 默认字段 lowering 需要回到“声明处 AST”读取默认值表达式；
    /// - 这里保留 `(SourceFile, ast::File)` 对，按 `decl_file` 做查找即可。
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    /// 顶层函数默认参数信息索引：`fqn -> params(default)`（用于 call-site 默认参数补齐，T1305）。
    default_arg_funs: HashMap<String, DefaultArgFunInfo>,
    /// struct 直接字段默认值信息索引：`struct fqn -> direct field params(default)`。
    default_arg_structs: HashMap<String, DefaultArgStructInfo>,
    /// 无 backing field 的 computed property getter 索引：`Owner.prop`。
    ///
    /// 用途：
    /// - computed property 读取需要在 HIR 阶段降糖为 getter 调用；
    /// - 避免 LLVM/codegen 再把它误当作 direct field 去查 layout。
    computed_property_getters: &'a HashSet<String>,
    /// 无 backing field 的 computed property setter 索引：`Owner.prop`。
    ///
    /// 用途：`receiver.prop = value` 在 HIR 阶段降糖为合成 setter 调用。
    computed_property_setters: &'a HashSet<String>,
    /// ctor 调用点候选集合：callee span → candidate type fqns。
    ///
    /// 说明：HIR v0 仍把 ctor 调用的 callee 降为 `UnresolvedIdent`，因此需要 side table
    /// 把 resolver 的 call candidates 保留下来，供 LLVM codegen 决定“这是 ctor call”。
    ctor_call_sites: CtorCallSiteIndex,
    /// 动态 dispatch 调用点 side table：`source_path + call span + receiver_ty` → dispatch kind。
    dispatch_call_sites: super::DispatchCallSiteIndex,
    /// effect-op 调用点绑定信息：`source_path + call span` → arg_mapping / payload tuple。
    effect_op_call_sites: super::EffectOpCallSiteIndex,
    /// handler arm 多 binder payload tuple 索引：`source_path + op head span` → tuple `TypeId`。
    handle_payload_tuple_tys: super::HandlePayloadTupleSiteIndex,
    /// `with` copy-update 的 typechecked aggregate/update contract。
    with_update_contracts: WithUpdateSiteIndex,
    /// assignment statement LHS 的 typed HIR place contract。
    assign_place_contracts: AssignPlaceSiteIndex,
    /// 顶层可变全局变量（`@ThreadLocal/@Global`）索引（TODO T1023）。
    top_level_vars: super::TopLevelVarIndex,
    /// `@Extern` 顶层变量索引。
    extern_globals: super::ExternGlobalIndex,
    /// 普通顶层 immutable value 索引：供后端生成 eager init + 稳定读取主线。
    top_level_immutable_values: super::TopLevelImmutableValueIndex,
    /// `when` pattern binder 的精确类型索引：供后端恢复 binder 的原始 `TypeId`。
    when_pat_binding_tys: super::WhenPatBindingTypeIndex,
    symbols: SymbolInterner,
    /// 本文件内的“局部 symbol → 是否可变（var）”信息。
    ///
    /// 用途：closure capture set 需要知道捕获目标是否为 `var`，以便 closure body 从 env load
    /// 后创建的 per-call local 能按外层 binding mutability 重新绑定。
    local_mutability: HashMap<SymbolId, bool>,
    /// HIR lowering 合成出来、因此没有 typecheck side table 记录的局部声明类型。
    local_decl_tys: HashMap<Span, TypeId>,
    next_closure: u32,
    /// 当前 receiver lambda 中隐式 `this` 的合成声明 span。
    ///
    /// 说明：
    /// - 普通函数 / 普通 lambda 为 `None`；
    /// - receiver lambda lowering body 时会覆盖为当前 lambda 的合成 `this` 绑定；
    /// - 嵌套普通 lambda 会继承外层 receiver lambda 的 `this`，嵌套 receiver lambda 会再次覆盖。
    lambda_this_decl_span: Option<Span>,
    /// 合成局部绑定计数器：用于给 lowering 生成的临时局部变量分配唯一 decl span / 名字。
    next_synthetic_local: usize,
    /// 合成 helper call-site 计数器：用于给 synthetic helper call 分配稳定且不与用户源码冲突的 span。
    next_synthetic_call_site: usize,
    /// 类型表（HIR 内所有 `TypeId` 必须来自同一个 store）。
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
    /// effect row parameter 作用域栈：用于 lowering `/ E` 或 `Type<eff E>` 这类 row 变量引用。
    effect_row_param_scopes: Vec<HashMap<String, EffectRowParamBinding>>,
    /// generic template 的 overload-aware 符号后缀索引。
    ///
    /// 说明：
    /// - HIR 兼容 lowering 仍需要为 concrete generic direct-call / function-value target 生成实例 FQN；
    /// - 当同一 `template.fqn` 存在多个 generic overload 时，必须与 MIR materialization 使用同一套
    ///   stable overload suffix 规则，避免生产路径上的实例声明名与调用目标继续碰撞。
    generic_template_symbol_suffixes: &'a util::GenericTemplateSymbolSuffixIndex,
    /// 是否把已 concrete 的非 intrinsic direct-call target 物化为最终实例 FQN。
    ///
    /// compilation-unit / LLVM frontend lowering 需要开启它，以便 backend 直接消费实例身份；
    /// dump / generic-template lowering 必须关闭它，保持 generic MIR template 不提前单态化。
    materialize_direct_call_targets: bool,
    /// 当前展开体中需要重映射的局部声明 span，避免同一源码 body 多次 unroll 后局部 SymbolId 冲突。
    local_decl_span_overrides: Vec<HashMap<Span, Span>>,
    /// lowering 过程中发现的第一个 typed HIR contract 错误。
    stage_error: Option<HirStageError>,
}

/// 构造 `HirLowering` 时用到的非必需上下文集合。
///
/// 说明：
/// - 这些字段在不同 lowering 入口之间经常成组出现；
/// - 单独打包后可以避免初始化函数参数过多，同时保持调用点语义明确。
struct HirLoweringSetup<'a> {
    typecheck_types: Option<&'a TypeStore>,
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    delegated_properties: &'a DelegatedPropertyIndex,
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    default_arg_structs: HashMap<String, DefaultArgStructInfo>,
    computed_property_getters: &'a HashSet<String>,
    computed_property_setters: &'a HashSet<String>,
    builtins: BuiltinTypes,
    generic_template_symbol_suffixes: &'a util::GenericTemplateSymbolSuffixIndex,
    materialize_direct_call_targets: bool,
}

#[derive(Clone)]
enum EffectRowParamBinding {
    Placeholder(TypeId),
    Concrete(EffectRow),
}

mod main;
pub use main::*;
