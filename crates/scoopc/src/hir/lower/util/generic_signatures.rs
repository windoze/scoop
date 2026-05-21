//! Type-param substitution, generic-member-fun instantiations, signature stable keys, explicit member templates.

#![allow(dead_code)]

use super::*;

///
/// 说明：
/// - generic class instantiation 需要把字段/ctor 参数里的 `T` 穿透到嵌套类型内部；
/// - 仅替换顶层 `TypeKind::Param` 会让 `Option<T>`、`State<T>`、`() -> Step<T>` 等形状
///   把参数残留到 LLVM codegen。
pub(in crate::hir::lower) fn substitute_type_params(
    types: &mut TypeStore,
    ty: crate::ty::TypeId,
    param_map: &HashMap<String, crate::ty::TypeId>,
) -> crate::ty::TypeId {
    match types.kind(ty).clone() {
        TypeKind::Param(p) if p.decl_file.as_os_str() == EFFECT_ROW_PARAM_DECL_FILE => ty,
        TypeKind::Param(p) => param_map.get(&p.name).copied().unwrap_or(ty),
        TypeKind::StarProjection(star) => {
            let read_ty = substitute_type_params(types, star.read_ty, param_map);
            if read_ty == star.read_ty {
                ty
            } else {
                types.ty_star_projection(read_ty)
            }
        }
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
        | TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Char)
        | TypeKind::Value(ValueTypeKind::Float64)
        | TypeKind::Value(ValueTypeKind::Float32)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => ty,
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let new_inner = substitute_type_params(types, inner, param_map);
            if new_inner == inner {
                ty
            } else {
                types.ty_option(new_inner)
            }
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let mut changed = false;
            let new_elements: Vec<crate::ty::TypeId> = elements
                .into_iter()
                .map(|element| {
                    let new_element = substitute_type_params(types, element, param_map);
                    if new_element != element {
                        changed = true;
                    }
                    new_element
                })
                .collect();
            if changed {
                types.ty_tuple(new_elements)
            } else {
                ty
            }
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let args: Vec<crate::ty::TypeId> = nominal
                .args
                .into_iter()
                .map(|arg| {
                    let new_arg = substitute_type_params(types, arg, param_map);
                    if new_arg != arg {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            let eff = nominal.eff.map(|row| {
                let new_row = substitute_type_param_effect_row(types, &row, param_map);
                if new_row != row {
                    changed = true;
                }
                new_row
            });
            if changed {
                types.intern(TypeKind::Ref(RefTypeKind::Nominal(
                    crate::ty::NominalType {
                        fqn: nominal.fqn,
                        args,
                        eff,
                    },
                )))
            } else {
                ty
            }
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let args: Vec<crate::ty::TypeId> = nominal
                .args
                .into_iter()
                .map(|arg| {
                    let new_arg = substitute_type_params(types, arg, param_map);
                    if new_arg != arg {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            let eff = nominal.eff.map(|row| {
                let new_row = substitute_type_param_effect_row(types, &row, param_map);
                if new_row != row {
                    changed = true;
                }
                new_row
            });
            if changed {
                types.intern(TypeKind::Value(ValueTypeKind::Nominal(
                    crate::ty::NominalType {
                        fqn: nominal.fqn,
                        args,
                        eff,
                    },
                )))
            } else {
                ty
            }
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let mut changed = false;
            let receiver = fun.receiver.map(|receiver| {
                let new_receiver = substitute_type_params(types, receiver, param_map);
                if new_receiver != receiver {
                    changed = true;
                }
                new_receiver
            });
            let params: Vec<crate::ty::TypeId> = fun
                .params
                .into_iter()
                .map(|param| {
                    let new_param = substitute_type_params(types, param, param_map);
                    if new_param != param {
                        changed = true;
                    }
                    new_param
                })
                .collect();
            let return_ty = substitute_type_params(types, fun.return_ty, param_map);
            if return_ty != fun.return_ty {
                changed = true;
            }
            let effects = substitute_type_param_effect_row(types, &fun.effects, param_map);
            if effects != fun.effects {
                changed = true;
            }
            if changed {
                types.ty_function(receiver, params, return_ty, effects, fun.effects_closed)
            } else {
                ty
            }
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let mut changed = false;
            let variants: Vec<crate::ty::TypeId> = union
                .variants
                .into_iter()
                .map(|variant| {
                    let new_variant = substitute_type_params(types, variant, param_map);
                    if new_variant != variant {
                        changed = true;
                    }
                    new_variant
                })
                .collect();
            if changed {
                types.ty_union(variants)
            } else {
                ty
            }
        }
    }
}

pub(in crate::hir::lower) fn substitute_type_param_effect_row(
    types: &mut TypeStore,
    row: &EffectRow,
    param_map: &HashMap<String, crate::ty::TypeId>,
) -> EffectRow {
    let mut changed = false;
    let terms: Vec<crate::ty::TypeId> = row
        .terms
        .iter()
        .copied()
        .map(|term| {
            let new_term = substitute_type_params(types, term, param_map);
            if new_term != term {
                changed = true;
            }
            new_term
        })
        .collect();
    if changed {
        EffectRow::new(terms)
    } else {
        EffectRow { terms }
    }
}

/// 解析字段的类型 FQN：如果字段类型是 type param，替换为具体类型的 FQN。
pub(in crate::hir::lower) fn resolve_field_type_fqn_with_type_kinds(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: Option<&HashMap<String, ast::TypeKind>>,
    ty_ref: Option<&ast::TypeRef>,
    param_map: &HashMap<String, crate::ty::TypeId>,
    types: &mut TypeStore,
) -> Option<String> {
    let ty = resolve_field_type_id_with_type_kinds(
        source, file, index, type_kinds, ty_ref, param_map, types,
    )?;
    type_id_to_layout_fqn(types, ty)
}

pub(in crate::hir::lower) fn resolve_field_type_id_with_type_kinds(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: Option<&HashMap<String, ast::TypeKind>>,
    ty_ref: Option<&ast::TypeRef>,
    param_map: &HashMap<String, crate::ty::TypeId>,
    types: &mut TypeStore,
) -> Option<crate::ty::TypeId> {
    let ty_ref = ty_ref?;
    lower_layout_type_ref_with_bindings(source, file, index, ty_ref, type_kinds, param_map, types)
}

pub(in crate::hir::lower) fn resolve_field_type_id(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ty_ref: Option<&ast::TypeRef>,
    param_map: &HashMap<String, crate::ty::TypeId>,
    types: &mut TypeStore,
) -> Option<crate::ty::TypeId> {
    resolve_field_type_id_with_type_kinds(source, file, index, None, ty_ref, param_map, types)
}

/// T0126/T4010b1: 为所有具体的泛型 nominal 实例化生成单态化的成员 callable HIR。
///
/// 覆盖范围：
/// - class/struct/enum 的 member `fun`
/// - struct/enum 的 getter-only computed property
///
/// 生成的 FunDecl FQN 使用 monomorph 形式：`"pkg.Box.get::<Int>"`，
/// 以与原始的 `"pkg.Box.get"`（含 Param 类型）共存于 `fun_index` 中。
pub(in crate::hir::lower) fn collect_generic_member_fun_instantiations(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    typecheck_types: Option<&TypeStore>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Vec<super::super::FunDecl> {
    let stable_cone_key = virtual_stable_cone_key_for_compilation_unit(pairs);
    let source_cones = HashMap::<std::path::PathBuf, crate::cone::SourceConeInfo>::new();
    collect_generic_member_fun_instantiations_with_source_cones(
        pairs,
        index,
        type_kinds,
        typecheck_types,
        types,
        builtins,
        (&stable_cone_key, &source_cones),
    )
}

pub(in crate::hir::lower) fn collect_generic_member_fun_instantiations_with_source_cones(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    typecheck_types: Option<&TypeStore>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    source_cone_keys: (
        &StableConeKey,
        &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
    ),
) -> Vec<super::super::FunDecl> {
    let (stable_cone_key, source_cones) = source_cone_keys;
    // 1) 收集泛型 nominal 声明：base_fqn -> (source, file, decl)
    let mut generic_owners: HashMap<String, (&SourceFile, &ast::File, &ast::TypeDecl)> =
        HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        collect_generic_member_owner_decls_with_file(
            source,
            file,
            &pkg_prefix,
            &file.items,
            &mut generic_owners,
        );
    }

    if generic_owners.is_empty() {
        return Vec::new();
    }
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes_with_source_cones(
            stable_cone_key,
            index,
            pairs,
            source_cones,
        );
    let empty_known_receiver_subclasses = crate::devirtualize::KnownReceiverSubclassIndex::new();
    let empty_class_vtables = crate::vtable::ClassVtableIndex::new();
    let empty_interfaces = crate::itable::InterfaceIndex::new();
    let empty_class_itables = crate::itable::ClassItableIndex::new();

    // 2) 收集 TypeStore 中所有具体实例化，去重
    let mut instantiations: Vec<(String, Vec<crate::ty::TypeId>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 需要先收集所有 TypeId，因为后面会 &mut types
    let all_ids: Vec<crate::ty::TypeId> = types.iter_ids().collect();
    for ty_id in all_ids {
        let nominal = match types.kind(ty_id) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal,
            _ => continue,
        };
        if nominal.args.is_empty() {
            continue;
        }
        if !generic_owners.contains_key(&nominal.fqn) {
            continue;
        }
        // 跳过仍包含 Param 类型的实例化（例如 Box<T>）
        if nominal.args.iter().any(|&a| type_contains_param(types, a)) {
            continue;
        }

        let mangled = mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        if seen.contains(&mangled) {
            continue;
        }
        seen.insert(mangled.clone());
        instantiations.push((nominal.fqn.clone(), nominal.args.clone()));
    }

    // 3) 为每个实例化的每个成员方法生成单态化 FunDecl
    let mut out: Vec<super::super::FunDecl> = Vec::new();

    for (base_fqn, concrete_args) in &instantiations {
        let Some((source, file, decl)) = generic_owners.get(base_fqn) else {
            continue;
        };

        let type_params = &decl.type_params;
        if type_params.len() != concrete_args.len() {
            continue;
        }

        // 构建 type param name → concrete TypeId 映射
        let bindings: Vec<(String, crate::ty::TypeId)> = type_params
            .iter()
            .zip(concrete_args.iter())
            .map(|(p, &arg)| (p.name.text(source).to_string(), arg))
            .collect();

        // 遍历 nominal body 中的成员 callable。
        let Some(body) = &decl.body else {
            continue;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    let mut hir_fun = crate::hir::lower::lower_member_fun_with_type_bindings(
                        crate::hir::LoweringInputs {
                            source,
                            file,
                            index,
                            type_kinds,
                            known_receiver_subclasses: &empty_known_receiver_subclasses,
                            class_vtables: &empty_class_vtables,
                            interfaces: &empty_interfaces,
                            class_itables: &empty_class_itables,
                            typecheck_types,
                            compilation_unit: pairs,
                            types,
                            builtins,
                            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                            materialize_direct_call_targets: true,
                            devirtualize_dispatch_calls: false,
                        },
                        crate::hir::lower::BoundMemberFunLoweringTarget {
                            owner_fqn: base_fqn,
                            this_decl_span: decl.name.span,
                            this_concrete_args: concrete_args,
                            fun,
                        },
                        bindings.clone(),
                    );
                    let template = TemplateKey {
                        fqn: format!("{base_fqn}.{}", fun.name.text(source)),
                        source_path: source.path().to_path_buf(),
                        decl_span: fun.span,
                    };
                    hir_fun.fqn = stable_instance_fqn(
                        types,
                        &template,
                        concrete_args,
                        &[],
                        generic_template_symbol_suffixes
                            .get(&template)
                            .map(String::as_str)
                            .unwrap_or(""),
                    );
                    out.push(hir_fun);
                }
                ast::TypeMember::Property(property)
                    if matches!(decl.kind, ast::TypeKind::Struct | ast::TypeKind::Enum)
                        && property.getter.is_some() =>
                {
                    let mut hir_fun = super::super::lower_value_property_getter_with_type_bindings(
                        crate::hir::LoweringInputs {
                            source,
                            file,
                            index,
                            type_kinds,
                            known_receiver_subclasses: &empty_known_receiver_subclasses,
                            class_vtables: &empty_class_vtables,
                            interfaces: &empty_interfaces,
                            class_itables: &empty_class_itables,
                            typecheck_types,
                            compilation_unit: pairs,
                            types,
                            builtins,
                            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                            materialize_direct_call_targets: true,
                            devirtualize_dispatch_calls: false,
                        },
                        crate::hir::lower::BoundValuePropertyGetterLoweringTarget {
                            owner_fqn: base_fqn,
                            this_decl_span: decl.name.span,
                            this_concrete_args: concrete_args,
                            property,
                        },
                        bindings.clone(),
                    );
                    let template = TemplateKey {
                        fqn: format!("{base_fqn}.{}", property.name.text(source)),
                        source_path: source.path().to_path_buf(),
                        decl_span: property.span,
                    };
                    hir_fun.fqn = stable_instance_fqn(
                        types,
                        &template,
                        concrete_args,
                        &[],
                        generic_template_symbol_suffixes
                            .get(&template)
                            .map(String::as_str)
                            .unwrap_or(""),
                    );
                    out.push(hir_fun);
                }
                _ => {}
            }
        }
    }

    out
}

pub(in crate::hir::lower) enum ExplicitMemberTemplate<'a> {
    Fun {
        source: &'a SourceFile,
        file: &'a ast::File,
        owner_fqn: String,
        owner_type_params: &'a [ast::TypeParam],
        this_decl_span: Span,
        fun: &'a ast::FunDecl,
        signature_key: String,
        has_body: bool,
    },
    Getter {
        source: &'a SourceFile,
        file: &'a ast::File,
        owner_fqn: String,
        owner_type_params: &'a [ast::TypeParam],
        this_decl_span: Span,
        property: &'a ast::PropertyDecl,
        signature_key: String,
        has_body: bool,
    },
}

#[derive(Clone)]
pub(in crate::hir::lower) struct TemplateSymbolCandidate {
    pub(in crate::hir::lower) template: TemplateKey,
    pub(in crate::hir::lower) signature_key: String,
    pub(in crate::hir::lower) prefers_materialized_body: bool,
    pub(in crate::hir::lower) stable_template_key: StableTemplateKey,
}

pub(in crate::hir::lower) fn stable_signature_param_owner_key(
    stable_cone_key: &StableConeKey,
    namespace: StableDefNamespace,
    owner_fqn: &str,
    declaration_kind: &str,
) -> String {
    StableDefKey::new(
        stable_cone_key.clone(),
        namespace,
        owner_fqn,
        declaration_kind,
        None,
    )
    .canonical_text()
}

pub(in crate::hir::lower) fn bind_signature_type_params(
    source: &SourceFile,
    scope: &HashMap<String, TypeId>,
    params: &[ast::TypeParam],
    owner_key: &str,
    start_index: usize,
    types: &TypeStore,
    resolver: &mut HashMap<TypeParamType, StableTypeParamKey>,
) {
    for (offset, param) in params.iter().enumerate() {
        let name = param.name.text(source);
        let ty = scope
            .get(name)
            .copied()
            .unwrap_or_else(|| panic!("missing signature placeholder for type parameter `{name}`"));
        let TypeKind::Param(param_ty) = types.kind(ty) else {
            panic!(
                "signature placeholder for type parameter `{name}` should lower to TypeKind::Param"
            );
        };
        resolver.insert(
            param_ty.clone(),
            StableTypeParamKey::new(owner_key.to_string(), start_index + offset),
        );
    }
}

pub(in crate::hir::lower) fn bind_signature_effect_param(
    source: &SourceFile,
    scope: &HashMap<String, crate::hir::lower::EffectRowParamBinding>,
    eff_param: &ast::EffectRowParam,
    owner_key: &str,
    index: usize,
    types: &TypeStore,
    resolver: &mut HashMap<TypeParamType, StableTypeParamKey>,
) {
    let name = eff_param.name.text(source);
    let binding = scope
        .get(name)
        .unwrap_or_else(|| panic!("missing signature placeholder for effect parameter `{name}`"));
    let crate::hir::lower::EffectRowParamBinding::Placeholder(marker) = binding else {
        panic!("signature effect parameter `{name}` should stay on placeholder binding path");
    };
    let TypeKind::Param(param_ty) = types.kind(*marker) else {
        panic!(
            "signature placeholder for effect parameter `{name}` should lower to TypeKind::Param"
        );
    };
    resolver.insert(
        param_ty.clone(),
        StableTypeParamKey::new(owner_key.to_string(), index),
    );
}

pub(in crate::hir::lower) fn with_signature_lowering_ctx<T>(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    f: impl FnOnce(&mut HirLowering<'_>) -> T,
) -> T {
    let compilation_unit = [(source, file)];
    let type_kinds = HashMap::new();
    let delegated_properties: DelegatedPropertyIndex<'_> = HashMap::new();
    let default_arg_structs = HashMap::new();
    let computed_property_getters = HashSet::new();
    let computed_property_setters = HashSet::new();
    let generic_template_symbol_suffixes = HashMap::new();
    let known_receiver_subclasses = crate::devirtualize::KnownReceiverSubclassIndex::new();
    let class_vtables = crate::vtable::ClassVtableIndex::new();
    let interfaces = crate::itable::InterfaceIndex::new();
    let class_itables = crate::itable::ClassItableIndex::new();
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        &mut types,
        HirLoweringSetup {
            typecheck_types: None,
            type_kinds: &type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit: &compilation_unit,
            default_arg_structs,
            computed_property_getters: &computed_property_getters,
            computed_property_setters: &computed_property_setters,
            builtins,
            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            materialize_direct_call_targets: false,
            devirtualize_dispatch_calls: false,
        },
    );
    f(&mut ctx)
}

pub(crate) fn canonical_generic_fun_signature_key(
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    fun: &ast::FunDecl,
) -> String {
    let declaration_kind = generic_fun_decl_kind(fun);
    let owner_key = stable_signature_param_owner_key(
        stable_cone_key,
        StableDefNamespace::Fun,
        owner_fqn,
        declaration_kind,
    );
    with_signature_lowering_ctx(source, file, index, |ctx| {
        ctx.push_type_params(owner_type_params);
        let owner_scope_index = ctx.type_param_scopes.len() - 1;
        let mut resolver = HashMap::new();
        bind_signature_type_params(
            source,
            &ctx.type_param_scopes[owner_scope_index],
            owner_type_params,
            &owner_key,
            0,
            &*ctx.types,
            &mut resolver,
        );

        ctx.push_type_params(&fun.type_params);
        let fun_scope_index = ctx.type_param_scopes.len() - 1;
        bind_signature_type_params(
            source,
            &ctx.type_param_scopes[fun_scope_index],
            &fun.type_params,
            &owner_key,
            owner_type_params.len(),
            &*ctx.types,
            &mut resolver,
        );

        if let Some(eff_param) = &fun.eff_param {
            let name = eff_param.name.text(source).to_string();
            ctx.push_effect_row_param_placeholder(name, eff_param.name.span);
            let effect_scope_index = ctx.effect_row_param_scopes.len() - 1;
            bind_signature_effect_param(
                source,
                &ctx.effect_row_param_scopes[effect_scope_index],
                eff_param,
                &owner_key,
                owner_type_params.len() + fun.type_params.len(),
                &*ctx.types,
                &mut resolver,
            );
        }

        let receiver = fun
            .receiver
            .as_ref()
            .map(|receiver| ctx.lower_type_ref(receiver));
        let params = fun
            .params
            .iter()
            .map(|param| {
                param
                    .ty
                    .as_ref()
                    .map(|ty| ctx.lower_type_ref(ty))
                    .unwrap_or(ctx.builtins.any)
            })
            .collect::<Vec<_>>();
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|ret| ctx.lower_type_ref(ret))
            .unwrap_or(ctx.builtins.unit);
        let effects = ctx.lower_effect_row_expr(fun.effects.as_ref());
        let callable_ty = ctx.types.ty_function(
            receiver,
            params,
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|row| row.closed),
        );

        canonical_callable_signature_key(
            &*ctx.types,
            callable_ty,
            owner_type_params.len(),
            fun.type_params.len(),
            usize::from(fun.eff_param.is_some()),
            &resolver,
        )
        .expect("generic callable signature key should encode canonical type/effect text")
    })
}

pub(crate) fn canonical_generic_property_getter_signature_key(
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    property: &ast::PropertyDecl,
) -> String {
    let owner_key = stable_signature_param_owner_key(
        stable_cone_key,
        StableDefNamespace::PropertyGetter,
        owner_fqn,
        generic_property_getter_decl_kind(property),
    );
    with_signature_lowering_ctx(source, file, index, |ctx| {
        ctx.push_type_params(owner_type_params);
        let owner_scope_index = ctx.type_param_scopes.len() - 1;
        let mut resolver = HashMap::new();
        bind_signature_type_params(
            source,
            &ctx.type_param_scopes[owner_scope_index],
            owner_type_params,
            &owner_key,
            0,
            &*ctx.types,
            &mut resolver,
        );

        let return_ty = property
            .ty
            .as_ref()
            .map(|ret| ctx.lower_type_ref(ret))
            .unwrap_or(ctx.builtins.any);
        canonical_property_getter_signature_key(
            &*ctx.types,
            return_ty,
            owner_type_params.len(),
            &resolver,
        )
        .expect("generic value getter signature key should encode canonical return type")
    })
}

pub(in crate::hir::lower) fn build_template_symbol_suffixes(
    stable_template_keys: &HashMap<TemplateKey, StableTemplateKey>,
) -> GenericTemplateSymbolSuffixIndex {
    let mut templates_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
    for template in stable_template_keys.keys() {
        templates_by_fqn
            .entry(template.fqn.clone())
            .or_default()
            .push(template.clone());
    }

    let mut out = HashMap::new();
    for (_, mut templates) in templates_by_fqn {
        templates.sort_by(template_key_sort);
        let overloaded = templates.len() > 1;
        for template in templates {
            let symbol_suffix = if overloaded {
                let stable_template_key = stable_template_keys
                    .get(&template)
                    .expect("every overloaded generic template should have a stable template key");
                format!(
                    "$overload${}",
                    stable_template_symbol_suffix(stable_template_key)
                )
            } else {
                String::new()
            };
            out.insert(template, symbol_suffix);
        }
    }
    out
}

pub(in crate::hir::lower) fn template_key_sort(
    lhs: &TemplateKey,
    rhs: &TemplateKey,
) -> std::cmp::Ordering {
    lhs.source_path
        .cmp(&rhs.source_path)
        .then_with(|| lhs.decl_span.start.cmp(&rhs.decl_span.start))
        .then_with(|| lhs.decl_span.end.cmp(&rhs.decl_span.end))
}

pub(in crate::hir::lower) struct ExplicitGenericMemberInstantiationInputs<'a> {
    pub compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    pub instance_keys: &'a [InstanceKey],
    pub instance_types: &'a TypeStore,
    pub index: &'a Index,
    pub type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub typecheck_types: Option<&'a TypeStore>,
    pub types: &'a mut TypeStore,
    pub builtins: BuiltinTypes,
    pub stable_cone_key: &'a StableConeKey,
    pub source_cones: &'a HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
}

pub(in crate::hir::lower) fn collect_generic_member_fun_instantiations_from_instance_keys(
    inputs: ExplicitGenericMemberInstantiationInputs<'_>,
) -> Result<Vec<super::super::FunDecl>, crate::hir::HirLowerError> {
    let ExplicitGenericMemberInstantiationInputs {
        compilation_unit,
        instance_keys,
        instance_types,
        index,
        type_kinds,
        typecheck_types,
        types,
        builtins,
        stable_cone_key,
        source_cones,
    } = inputs;

    if instance_keys.is_empty() {
        return Ok(Vec::new());
    }

    let templates = collect_explicit_member_templates_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        source_cones,
    );
    if templates.is_empty() {
        return Ok(Vec::new());
    }
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes_with_source_cones(
            stable_cone_key,
            index,
            compilation_unit,
            source_cones,
        );
    let direct_supertypes = super::collect_direct_supertypes(compilation_unit, index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = match typecheck_types {
        Some(typecheck_types) => crate::itable::collect_runtime_interfaces_and_class_itables(
            compilation_unit,
            index,
            &class_vtables,
            typecheck_types,
        )?,
        None => crate::itable::collect_interfaces_and_class_itables(
            compilation_unit,
            index,
            &class_vtables,
        )?,
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for instance in instance_keys {
        let Some(template) = templates.get(&instance.template) else {
            continue;
        };

        match template {
            ExplicitMemberTemplate::Fun {
                source,
                file,
                owner_fqn,
                owner_type_params,
                this_decl_span,
                fun,
                ..
            } => {
                let owner_param_count = owner_type_params.len();
                let fun_param_count = fun.type_params.len();
                if instance.type_args.len() != owner_param_count + fun_param_count {
                    return Err(explicit_instance_lowering_error(format!(
                        "member fun `{}` 的 type args 数量不匹配：期望 {}，得到 {}",
                        instance.template.fqn,
                        owner_param_count + fun_param_count,
                        instance.type_args.len()
                    )));
                }
                let owner_args = instance.type_args[..owner_param_count]
                    .iter()
                    .map(|&arg| types.re_intern_from(instance_types, arg))
                    .collect::<Vec<_>>();
                let fun_args = instance.type_args[owner_param_count..]
                    .iter()
                    .map(|&arg| types.re_intern_from(instance_types, arg))
                    .collect::<Vec<_>>();
                let eff_args = instance
                    .eff_args
                    .iter()
                    .map(|row| re_intern_effect_row_from(types, instance_types, row))
                    .collect::<Vec<_>>();
                let effect_binding = build_effect_binding(
                    source,
                    &instance.template.fqn,
                    &fun.eff_param,
                    &eff_args,
                )?;
                let instance_fqn = stable_instance_fqn(
                    types,
                    &instance.template,
                    &[owner_args.as_slice(), fun_args.as_slice()].concat(),
                    &eff_args,
                    generic_template_symbol_suffixes
                        .get(&instance.template)
                        .map(String::as_str)
                        .unwrap_or(""),
                );
                if !seen.insert(instance_fqn.clone()) {
                    continue;
                }
                let owner_bindings = owner_type_params
                    .iter()
                    .zip(owner_args.iter())
                    .map(|(param, &arg)| (param.name.text(source).to_string(), arg))
                    .collect::<Vec<_>>();
                let fun_bindings = fun
                    .type_params
                    .iter()
                    .zip(fun_args.iter())
                    .map(|(param, &arg)| (param.name.text(source).to_string(), arg))
                    .collect::<Vec<_>>();
                let mut hir_fun = super::super::lower_member_fun_with_bindings(
                    crate::hir::LoweringInputs {
                        source,
                        file,
                        index,
                        type_kinds,
                        known_receiver_subclasses: &known_receiver_subclasses,
                        class_vtables: &class_vtables,
                        interfaces: &interfaces,
                        class_itables: &class_itables,
                        typecheck_types,
                        compilation_unit,
                        types,
                        builtins,
                        generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                        materialize_direct_call_targets: true,
                        devirtualize_dispatch_calls: true,
                    },
                    crate::hir::lower::BoundMemberFunLoweringTarget {
                        owner_fqn,
                        this_decl_span: *this_decl_span,
                        this_concrete_args: &owner_args,
                        fun,
                    },
                    owner_bindings,
                    fun_bindings,
                    effect_binding,
                );
                hir_fun.fqn = instance_fqn;
                out.push(hir_fun);
            }
            ExplicitMemberTemplate::Getter {
                source,
                file,
                owner_fqn,
                owner_type_params,
                this_decl_span,
                property,
                ..
            } => {
                if !instance.eff_args.is_empty() {
                    return Err(explicit_instance_lowering_error(format!(
                        "value getter `{}` 不应携带 effect args，但实例请求提供了 {} 个",
                        instance.template.fqn,
                        instance.eff_args.len()
                    )));
                }
                if instance.type_args.len() != owner_type_params.len() {
                    return Err(explicit_instance_lowering_error(format!(
                        "value getter `{}` 的 owner type args 数量不匹配：期望 {}，得到 {}",
                        instance.template.fqn,
                        owner_type_params.len(),
                        instance.type_args.len()
                    )));
                }
                let owner_args = instance
                    .type_args
                    .iter()
                    .map(|&arg| types.re_intern_from(instance_types, arg))
                    .collect::<Vec<_>>();
                let instance_fqn = stable_instance_fqn(
                    types,
                    &instance.template,
                    &owner_args,
                    &[],
                    generic_template_symbol_suffixes
                        .get(&instance.template)
                        .map(String::as_str)
                        .unwrap_or(""),
                );
                if !seen.insert(instance_fqn.clone()) {
                    continue;
                }
                let owner_bindings = owner_type_params
                    .iter()
                    .zip(owner_args.iter())
                    .map(|(param, &arg)| (param.name.text(source).to_string(), arg))
                    .collect::<Vec<_>>();
                let mut hir_fun = super::super::lower_value_property_getter_with_type_bindings(
                    crate::hir::LoweringInputs {
                        source,
                        file,
                        index,
                        type_kinds,
                        known_receiver_subclasses: &known_receiver_subclasses,
                        class_vtables: &class_vtables,
                        interfaces: &interfaces,
                        class_itables: &class_itables,
                        typecheck_types,
                        compilation_unit,
                        types,
                        builtins,
                        generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                        materialize_direct_call_targets: true,
                        devirtualize_dispatch_calls: true,
                    },
                    crate::hir::lower::BoundValuePropertyGetterLoweringTarget {
                        owner_fqn,
                        this_decl_span: *this_decl_span,
                        this_concrete_args: &owner_args,
                        property,
                    },
                    owner_bindings,
                );
                hir_fun.fqn = instance_fqn;
                out.push(hir_fun);
            }
        }
    }

    Ok(out)
}

pub(in crate::hir::lower) fn collect_explicit_member_templates<'a>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
) -> HashMap<TemplateKey, ExplicitMemberTemplate<'a>> {
    let source_cones = HashMap::<std::path::PathBuf, crate::cone::SourceConeInfo>::new();
    collect_explicit_member_templates_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        &source_cones,
    )
}

pub(in crate::hir::lower) fn collect_explicit_member_templates_with_source_cones<'a>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    source_cones: &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> HashMap<TemplateKey, ExplicitMemberTemplate<'a>> {
    let mut out = HashMap::new();
    for (source, file) in compilation_unit {
        let source_stable_cone_key =
            stable_cone_key_for_source(source, stable_cone_key, source_cones);
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        collect_explicit_member_templates_in_items(
            source_stable_cone_key,
            index,
            source,
            file,
            &pkg_prefix,
            &file.items,
            &mut out,
        );
    }
    out
}

pub(in crate::hir::lower) fn collect_explicit_member_templates_in_items<'a>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    source: &'a SourceFile,
    file: &'a ast::File,
    owner_prefix: &str,
    items: &'a [ast::Item],
    out: &mut HashMap<TemplateKey, ExplicitMemberTemplate<'a>>,
) {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                collect_explicit_member_templates_in_type_decl(
                    stable_cone_key,
                    index,
                    source,
                    file,
                    ty,
                    owner_prefix,
                    out,
                );
            }
            ast::Item::Object(obj) => {
                collect_explicit_member_templates_in_object_decl(
                    stable_cone_key,
                    index,
                    source,
                    file,
                    obj,
                    owner_prefix,
                    out,
                );
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_explicit_member_templates_in_type_decl<'a>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    source: &'a SourceFile,
    file: &'a ast::File,
    decl: &'a ast::TypeDecl,
    owner_prefix: &str,
    out: &mut HashMap<TemplateKey, ExplicitMemberTemplate<'a>>,
) {
    let local_name = decl.name.text(source);
    let owner_fqn = join_prefix(owner_prefix, local_name);
    let owner_is_generic = !decl.type_params.is_empty();
    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun)
                if owner_is_generic || !fun.type_params.is_empty() || fun.eff_param.is_some() =>
            {
                let fqn = format!("{owner_fqn}.{}", fun.name.text(source));
                out.insert(
                    TemplateKey {
                        fqn,
                        source_path: source.path().to_path_buf(),
                        decl_span: fun.span,
                    },
                    ExplicitMemberTemplate::Fun {
                        source,
                        file,
                        owner_fqn: owner_fqn.clone(),
                        owner_type_params: &decl.type_params,
                        this_decl_span: decl.name.span,
                        fun,
                        signature_key: canonical_generic_fun_signature_key(
                            stable_cone_key,
                            source,
                            file,
                            index,
                            &owner_fqn,
                            &decl.type_params,
                            fun,
                        ),
                        has_body: !matches!(fun.body, ast::FunBody::Missing),
                    },
                );
            }
            ast::TypeMember::Property(property)
                if owner_is_generic
                    && matches!(decl.kind, ast::TypeKind::Struct | ast::TypeKind::Enum)
                    && property.getter.is_some() =>
            {
                let fqn = format!("{owner_fqn}.{}", property.name.text(source));
                out.insert(
                    TemplateKey {
                        fqn,
                        source_path: source.path().to_path_buf(),
                        decl_span: property.span,
                    },
                    ExplicitMemberTemplate::Getter {
                        source,
                        file,
                        owner_fqn: owner_fqn.clone(),
                        owner_type_params: &decl.type_params,
                        this_decl_span: decl.name.span,
                        property,
                        signature_key: canonical_generic_property_getter_signature_key(
                            stable_cone_key,
                            source,
                            file,
                            index,
                            &owner_fqn,
                            &decl.type_params,
                            property,
                        ),
                        has_body: property.getter.as_ref().is_some_and(|getter| {
                            !matches!(getter.body, ast::AccessorBody::Missing)
                        }),
                    },
                );
            }
            ast::TypeMember::Type(nested) => {
                collect_explicit_member_templates_in_type_decl(
                    stable_cone_key,
                    index,
                    source,
                    file,
                    nested,
                    &owner_fqn,
                    out,
                );
            }
            ast::TypeMember::Object(obj) => {
                collect_explicit_member_templates_in_object_decl(
                    stable_cone_key,
                    index,
                    source,
                    file,
                    obj,
                    &owner_fqn,
                    out,
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_explicit_member_templates_in_object_decl<'a>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    source: &'a SourceFile,
    file: &'a ast::File,
    obj: &'a ast::ObjectDecl,
    owner_prefix: &str,
    out: &mut HashMap<TemplateKey, ExplicitMemberTemplate<'a>>,
) {
    let Some(name) = object_decl_name(source, obj) else {
        return;
    };
    let owner_fqn = join_prefix(owner_prefix, &name);
    let this_decl_span = obj.name.as_ref().map(|n| n.span).unwrap_or(obj.span);
    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) if !fun.type_params.is_empty() || fun.eff_param.is_some() => {
                let fqn = format!("{owner_fqn}.{}", fun.name.text(source));
                out.insert(
                    TemplateKey {
                        fqn,
                        source_path: source.path().to_path_buf(),
                        decl_span: fun.span,
                    },
                    ExplicitMemberTemplate::Fun {
                        source,
                        file,
                        owner_fqn: owner_fqn.clone(),
                        owner_type_params: &[],
                        this_decl_span,
                        fun,
                        signature_key: canonical_generic_fun_signature_key(
                            stable_cone_key,
                            source,
                            file,
                            index,
                            &owner_fqn,
                            &[],
                            fun,
                        ),
                        has_body: !matches!(fun.body, ast::FunBody::Missing),
                    },
                );
            }
            ast::TypeMember::Type(nested) => {
                collect_explicit_member_templates_in_type_decl(
                    stable_cone_key,
                    index,
                    source,
                    file,
                    nested,
                    &owner_fqn,
                    out,
                );
            }
            ast::TypeMember::Object(nested) => {
                collect_explicit_member_templates_in_object_decl(
                    stable_cone_key,
                    index,
                    source,
                    file,
                    nested,
                    &owner_fqn,
                    out,
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

/// 类似 `collect_generic_class_decls_in_items`，但同时记录 file 引用（用于单态化 lowering）。
pub(in crate::hir::lower) fn collect_generic_member_owner_decls_with_file<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    owner_prefix: &str,
    items: &'a [ast::Item],
    out: &mut HashMap<String, (&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)>,
) {
    for item in items {
        match item {
            ast::Item::Type(ty) => {
                let name = ty.name.text(source).to_string();
                let fqn = join_prefix(owner_prefix, &name);

                if matches!(
                    ty.kind,
                    ast::TypeKind::Class | ast::TypeKind::Struct | ast::TypeKind::Enum
                ) && !ty.type_params.is_empty()
                {
                    out.insert(fqn.clone(), (source, file, ty));
                }

                // 嵌套声明
                if let Some(body) = &ty.body {
                    for member in &body.members {
                        match member {
                            ast::TypeMember::Type(nested) => {
                                collect_generic_member_owner_decls_in_type_decl(
                                    source, file, nested, &fqn, out,
                                );
                            }
                            ast::TypeMember::Object(obj) => {
                                collect_generic_member_owner_decls_in_object_decl(
                                    source, file, obj, &fqn, out,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            ast::Item::Object(obj) => {
                let obj_name = obj.name.as_ref().map(|n| n.text(source).to_string());
                if let Some(obj_name) = obj_name {
                    let obj_fqn = join_prefix(owner_prefix, &obj_name);
                    if let Some(body) = &obj.body {
                        for member in &body.members {
                            match member {
                                ast::TypeMember::Type(nested) => {
                                    collect_generic_member_owner_decls_in_type_decl(
                                        source, file, nested, &obj_fqn, out,
                                    );
                                }
                                ast::TypeMember::Object(nested) => {
                                    collect_generic_member_owner_decls_in_object_decl(
                                        source, file, nested, &obj_fqn, out,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_generic_member_owner_decls_in_type_decl<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    decl: &'a ast::TypeDecl,
    prefix: &str,
    out: &mut HashMap<String, (&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)>,
) {
    let name = decl.name.text(source).to_string();
    let fqn = join_prefix(prefix, &name);
    if matches!(
        decl.kind,
        ast::TypeKind::Class | ast::TypeKind::Struct | ast::TypeKind::Enum
    ) && !decl.type_params.is_empty()
    {
        out.insert(fqn.clone(), (source, file, decl));
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_generic_member_owner_decls_in_type_decl(source, file, nested, &fqn, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_generic_member_owner_decls_in_object_decl(source, file, obj, &fqn, out);
            }
            _ => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_generic_member_owner_decls_in_object_decl<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
    obj: &'a ast::ObjectDecl,
    prefix: &str,
    out: &mut HashMap<String, (&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)>,
) {
    let Some(name) = obj.name.as_ref().map(|n| n.text(source).to_string()) else {
        return;
    };
    let owner_fqn = join_prefix(prefix, &name);
    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_generic_member_owner_decls_in_type_decl(
                    source, file, nested, &owner_fqn, out,
                );
            }
            ast::TypeMember::Object(nested) => {
                collect_generic_member_owner_decls_in_object_decl(
                    source, file, nested, &owner_fqn, out,
                );
            }
            _ => {}
        }
    }
}

pub(in crate::hir::lower) fn map_hir_call_args_to_params_by_name(
    param_names: &[String],
    args: &[CallArg],
) -> Option<Vec<usize>> {
    if args.len() != param_names.len() {
        return None;
    }

    let mut seen_named = false;
    let mut positional_count = 0usize;
    for arg in args {
        match arg {
            CallArg::Positional(_) => {
                if seen_named {
                    return None;
                }
                positional_count = positional_count.saturating_add(1);
            }
            CallArg::Named { .. } => {
                seen_named = true;
            }
        }
    }

    if positional_count > param_names.len() {
        return None;
    }

    let mut param_to_arg: Vec<Option<usize>> = vec![None; param_names.len()];
    for (slot_idx, arg_idx) in (0..positional_count).enumerate() {
        *param_to_arg.get_mut(slot_idx)? = Some(arg_idx);
    }

    for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
        let CallArg::Named { name, .. } = arg else {
            return None;
        };
        let slot_idx = param_names.iter().position(|param| param == name)?;
        let slot = param_to_arg.get_mut(slot_idx)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(arg_idx);
    }

    let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
    for (param_idx, arg_idx) in param_to_arg.into_iter().enumerate() {
        let arg_idx = arg_idx?;
        let slot = arg_to_param.get_mut(arg_idx)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(param_idx);
    }

    arg_to_param.into_iter().collect()
}

pub(in crate::hir::lower) fn collect_hir_type_param_names(
    types: &TypeStore,
    ty: crate::ty::TypeId,
    out: &mut Vec<String>,
) {
    let mut stack = vec![ty];
    while let Some(id) = stack.pop() {
        match types.kind(id) {
            TypeKind::Param(tp) => {
                if !out.contains(&tp.name) {
                    out.push(tp.name.clone());
                }
            }
            TypeKind::StarProjection(star) => stack.push(star.read_ty),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                stack.extend(nominal.args.iter().copied());
                if let Some(eff) = &nominal.eff {
                    stack.extend(eff.terms.iter().copied());
                }
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                stack.extend(elements.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if let Some(receiver) = fun.receiver {
                    stack.push(receiver);
                }
                stack.extend(fun.params.iter().copied());
                stack.push(fun.return_ty);
                stack.extend(fun.effects.terms.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => {
                stack.extend(union.variants.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
            | TypeKind::Value(ValueTypeKind::Unit)
            | TypeKind::Value(ValueTypeKind::Nothing)
            | TypeKind::Value(ValueTypeKind::Bool)
            | TypeKind::Value(ValueTypeKind::Char)
            | TypeKind::Value(ValueTypeKind::Float64)
            | TypeKind::Value(ValueTypeKind::Float32)
            | TypeKind::Value(ValueTypeKind::Int)
            | TypeKind::Value(ValueTypeKind::UInt)
            | TypeKind::Value(ValueTypeKind::IntN(_))
            | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_hir_type_param_bindings(
    types: &TypeStore,
    declared_ty: crate::ty::TypeId,
    concrete_ty: crate::ty::TypeId,
    bindings: &mut HashMap<String, crate::ty::TypeId>,
) {
    match (types.kind(declared_ty), types.kind(concrete_ty)) {
        (TypeKind::Param(tp), _) => match bindings.get(&tp.name).copied() {
            Some(existing) if existing == concrete_ty => {}
            Some(_) => {}
            None => {
                bindings.insert(tp.name.clone(), concrete_ty);
            }
        },
        (
            TypeKind::Ref(RefTypeKind::Nominal(declared)),
            TypeKind::Ref(RefTypeKind::Nominal(concrete)),
        )
        | (
            TypeKind::Value(ValueTypeKind::Nominal(declared)),
            TypeKind::Value(ValueTypeKind::Nominal(concrete)),
        ) => {
            if declared.fqn != concrete.fqn || declared.args.len() != concrete.args.len() {
                return;
            }
            for (decl_arg, concrete_arg) in declared.args.iter().zip(concrete.args.iter()) {
                collect_hir_type_param_bindings(types, *decl_arg, *concrete_arg, bindings);
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Option(declared_inner)),
            TypeKind::Value(ValueTypeKind::Option(concrete_inner)),
        ) => {
            collect_hir_type_param_bindings(types, *declared_inner, *concrete_inner, bindings);
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(declared_elements)),
            TypeKind::Value(ValueTypeKind::Tuple(concrete_elements)),
        ) => {
            if declared_elements.len() != concrete_elements.len() {
                return;
            }
            for (decl_elem, concrete_elem) in declared_elements.iter().zip(concrete_elements.iter())
            {
                collect_hir_type_param_bindings(types, *decl_elem, *concrete_elem, bindings);
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(declared_fun)),
            TypeKind::Ref(RefTypeKind::Function(concrete_fun)),
        ) => {
            match (declared_fun.receiver, concrete_fun.receiver) {
                (Some(declared_receiver), Some(concrete_receiver)) => {
                    collect_hir_type_param_bindings(
                        types,
                        declared_receiver,
                        concrete_receiver,
                        bindings,
                    );
                }
                (None, None) => {}
                _ => return,
            }
            if declared_fun.params.len() != concrete_fun.params.len() {
                return;
            }
            for (decl_param, concrete_param) in
                declared_fun.params.iter().zip(concrete_fun.params.iter())
            {
                collect_hir_type_param_bindings(types, *decl_param, *concrete_param, bindings);
            }
            collect_hir_type_param_bindings(
                types,
                declared_fun.return_ty,
                concrete_fun.return_ty,
                bindings,
            );
        }
        (
            TypeKind::Ref(RefTypeKind::Union(declared_union)),
            TypeKind::Ref(RefTypeKind::Union(concrete_union)),
        ) => {
            if declared_union.variants.len() != concrete_union.variants.len() {
                return;
            }
            for (decl_variant, concrete_variant) in declared_union
                .variants
                .iter()
                .zip(concrete_union.variants.iter())
            {
                collect_hir_type_param_bindings(types, *decl_variant, *concrete_variant, bindings);
            }
        }
        _ => {}
    }
}

pub(in crate::hir::lower) fn extract_concrete_hir_expr_ty(
    types: &TypeStore,
    expr: &super::super::Expr,
) -> Option<crate::ty::TypeId> {
    let ty = expr.ty;
    (!type_contains_param(types, ty) && !matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Any)))
        .then_some(ty)
}
