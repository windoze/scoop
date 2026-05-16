//! Lowering helpers: LoweringInputs, member-fun / value-property getter lowering bindings.

#![allow(dead_code)]

use super::*;

pub(crate) struct LoweringInputs<'a> {
    pub(crate) source: &'a SourceFile,
    pub(crate) file: &'a ast::File,
    pub(crate) index: &'a Index,
    pub(crate) type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub(crate) known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    pub(crate) class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub(crate) interfaces: &'a crate::itable::InterfaceIndex,
    pub(crate) class_itables: &'a crate::itable::ClassItableIndex,
    pub(crate) typecheck_types: Option<&'a TypeStore>,
    pub(crate) compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    pub(crate) types: &'a mut TypeStore,
    pub(crate) builtins: BuiltinTypes,
    pub(crate) generic_template_symbol_suffixes: &'a util::GenericTemplateSymbolSuffixIndex,
    pub(crate) materialize_direct_call_targets: bool,
    pub(crate) devirtualize_dispatch_calls: bool,
}

pub(in crate::hir) struct BoundMemberFunLoweringTarget<'a> {
    pub(in crate::hir::lower) owner_fqn: &'a str,
    pub(in crate::hir::lower) this_decl_span: Span,
    pub(in crate::hir::lower) this_concrete_args: &'a [TypeId],
    pub(in crate::hir::lower) fun: &'a ast::FunDecl,
}

pub(in crate::hir) struct BoundValuePropertyGetterLoweringTarget<'a> {
    pub(in crate::hir::lower) owner_fqn: &'a str,
    pub(in crate::hir::lower) this_decl_span: Span,
    pub(in crate::hir::lower) this_concrete_args: &'a [TypeId],
    pub(in crate::hir::lower) property: &'a ast::PropertyDecl,
}

pub(crate) struct LoweredFunWithMirFacts {
    pub(crate) fun: FunDecl,
    pub(crate) dispatch_call_sites: crate::hir::DispatchCallSiteIndex,
    pub(crate) effect_op_call_sites: crate::hir::EffectOpCallSiteIndex,
    pub(crate) when_pat_binding_tys: crate::hir::WhenPatBindingTypeIndex,
    pub(crate) top_level_fun_call_sites: crate::hir::TopLevelFunCallSiteIndex,
}

/// 将给定的 `ast::FunDecl` 在”已绑定 type params”的语境下降低为 HIR（用于单态化，T0712）。
///
/// 说明：
/// - 该函数假设调用方已在 AST 上运行 resolver（headers + bodies），以便 `ValueIdent.resolved` 等信息可用；
/// - `type_bindings` 用于把函数声明处的 `T` 等 type params 映射为具体 `TypeId`（调用点推断结果）。
pub(crate) fn lower_fun_with_type_bindings(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    lower_fun_with_bindings(inputs, fun, type_bindings, None)
}

pub(crate) fn lower_fun_with_bindings(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    effect_binding: Option<(String, EffectRow)>,
) -> FunDecl {
    lower_fun_with_bindings_and_mir_facts(inputs, fun, type_bindings, effect_binding).fun
}

pub(crate) fn lower_fun_with_type_bindings_and_mir_facts(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> LoweredFunWithMirFacts {
    lower_fun_with_bindings_and_mir_facts(inputs, fun, type_bindings, None)
}

pub(crate) fn lower_fun_with_bindings_and_mir_facts(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    effect_binding: Option<(String, EffectRow)>,
) -> LoweredFunWithMirFacts {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        compilation_unit,
        types,
        builtins,
        generic_template_symbol_suffixes,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = inputs;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let computed_property_accessors = collect_computed_property_accessor_fqns(compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit,
            default_arg_structs,
            computed_property_getters: &computed_property_accessors.getters,
            computed_property_setters: &computed_property_accessors.setters,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );
    let type_bindings = type_bindings.into_iter().collect::<Vec<_>>();
    let type_bindings_pushed = !type_bindings.is_empty();
    if type_bindings_pushed {
        ctx.push_type_param_bindings(type_bindings);
    }
    let effect_binding_pushed = if let Some((name, row)) = effect_binding {
        ctx.push_effect_row_param_binding(name, row);
        true
    } else {
        false
    };
    let out = ctx.lower_fun_decl_with_bound_type_params(&pkg_prefix, fun);
    if effect_binding_pushed {
        ctx.pop_effect_row_param_binding();
    }
    if type_bindings_pushed {
        ctx.pop_type_params();
    }
    LoweredFunWithMirFacts {
        fun: out,
        dispatch_call_sites: std::mem::take(&mut ctx.dispatch_call_sites),
        effect_op_call_sites: std::mem::take(&mut ctx.effect_op_call_sites),
        when_pat_binding_tys: std::mem::take(&mut ctx.when_pat_binding_tys),
        top_level_fun_call_sites: collect_top_level_fun_call_sites(&[(source, file)]),
    }
}

/// T0126: 将泛型类的成员方法在”已绑定 owner type params”的语境下降低为 HIR。
///
/// 说明：
/// - 类似 `lower_fun_with_type_bindings`，但处理的是 class member method（带隐式 `this` 参数）；
/// - `owner_type_bindings` 映射 owner 的 type params 到具体 TypeId（例如 `T → Int` for `Box<Int>`）；
/// - `this_concrete_args` 是 owner 的具体类型实参（例如 `[Int]` for `Box<Int>`），用于构造 `this` 的精确类型；
/// - 生成的 FunDecl 中 `this` 参数类型为具体的 `owner_fqn<concrete_args>`，所有引用 owner
///   type params 的地方均被替换为具体类型。
pub(crate) fn lower_member_fun_with_type_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundMemberFunLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    lower_member_fun_with_bindings(
        inputs,
        target,
        owner_type_bindings,
        Vec::<(String, TypeId)>::new(),
        None,
    )
}

pub(crate) fn lower_member_fun_with_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundMemberFunLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    fun_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    effect_binding: Option<(String, EffectRow)>,
) -> FunDecl {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        compilation_unit,
        types,
        builtins,
        generic_template_symbol_suffixes,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = inputs;
    let BoundMemberFunLoweringTarget {
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        fun,
    } = target;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let computed_property_accessors = collect_computed_property_accessor_fqns(compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit,
            default_arg_structs,
            computed_property_getters: &computed_property_accessors.getters,
            computed_property_setters: &computed_property_accessors.setters,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );
    // 先绑定 owner type params（例如 class Box<T> 的 T → Int），
    // 再由 lower_member_fun_decl_with_bound_type_params 处理方法 body。
    let owner_type_bindings = owner_type_bindings.into_iter().collect::<Vec<_>>();
    let owner_type_bindings_pushed = !owner_type_bindings.is_empty();
    if owner_type_bindings_pushed {
        ctx.push_type_param_bindings(owner_type_bindings);
    }
    let fun_type_bindings = fun_type_bindings.into_iter().collect::<Vec<_>>();
    let fun_type_bindings_pushed = !fun_type_bindings.is_empty();
    if fun_type_bindings_pushed {
        ctx.push_type_param_bindings(fun_type_bindings);
    }
    let effect_binding_pushed = if let Some((name, row)) = effect_binding {
        ctx.push_effect_row_param_binding(name, row);
        true
    } else {
        false
    };
    let out = ctx.lower_member_fun_decl_with_bound_type_params(
        &pkg_prefix,
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        fun,
    );
    if effect_binding_pushed {
        ctx.pop_effect_row_param_binding();
    }
    if fun_type_bindings_pushed {
        ctx.pop_type_params();
    }
    if owner_type_bindings_pushed {
        ctx.pop_type_params();
    }
    out
}

/// 将值类型 computed property getter 在“已绑定 owner type params”的语境下降低为 HIR。
pub(crate) fn lower_value_property_getter_with_type_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundValuePropertyGetterLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        compilation_unit,
        types,
        builtins,
        generic_template_symbol_suffixes,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = inputs;
    let BoundValuePropertyGetterLoweringTarget {
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        property,
    } = target;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let computed_property_accessors = collect_computed_property_accessor_fqns(compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit,
            default_arg_structs,
            computed_property_getters: &computed_property_accessors.getters,
            computed_property_setters: &computed_property_accessors.setters,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );
    let owner_type_bindings = owner_type_bindings.into_iter().collect::<Vec<_>>();
    let owner_type_bindings_pushed = !owner_type_bindings.is_empty();
    if owner_type_bindings_pushed {
        ctx.push_type_param_bindings(owner_type_bindings);
    }
    let out = ctx.lower_value_property_getter_decl_with_bound_type_params(
        &pkg_prefix,
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        property,
    );
    if owner_type_bindings_pushed {
        ctx.pop_type_params();
    }
    out
}
