//! Package-prefix, type-decl-kinds, nominal variances, supertypes, init collection (object & class).

#![allow(dead_code)]

use super::*;

pub(in crate::hir::lower) fn package_prefix(
    source: &SourceFile,
    package: Option<&ast::PackageDecl>,
) -> String {
    let Some(p) = package else {
        return String::new();
    };

    let mut out = String::new();
    for (idx, seg) in p.path.iter().enumerate() {
        if idx != 0 {
            out.push('.');
        }
        out.push_str(seg.text(source));
    }
    out
}

pub(in crate::hir::lower) fn collect_type_decl_kinds(
    pairs: &[(&SourceFile, &ast::File)],
) -> HashMap<String, ast::TypeKind> {
    let mut out: HashMap<String, ast::TypeKind> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            out.insert(fqn, ty.kind);
        }
    }
    out
}

pub(in crate::hir::lower) fn collect_nominal_variances(
    pairs: &[(&SourceFile, &ast::File)],
) -> HashMap<String, Vec<Option<ast::TypeParamVariance>>> {
    let mut out: HashMap<String, Vec<Option<ast::TypeParamVariance>>> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let fqn = join_prefix(&pkg_prefix, ty.name.text(source));
            out.insert(
                fqn,
                ty.type_params.iter().map(|param| param.variance).collect(),
            );
        }
    }
    out
}

pub(crate) fn collect_stable_type_param_keys(
    compilation_unit: &[(&SourceFile, &ast::File)],
    stable_cone_key: &StableConeKey,
) -> HashMap<TypeParamType, StableTypeParamKey> {
    let mut out = HashMap::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            collect_stable_type_param_keys_in_item(
                source,
                item,
                &pkg_prefix,
                stable_cone_key,
                &mut out,
            );
        }
    }
    out
}

pub(in crate::hir::lower) fn collect_stable_type_param_keys_in_item(
    source: &SourceFile,
    item: &ast::Item,
    owner_prefix: &str,
    stable_cone_key: &StableConeKey,
    out: &mut HashMap<TypeParamType, StableTypeParamKey>,
) {
    match item {
        ast::Item::TypeAlias(alias) => {
            let owner_fqn = join_prefix(owner_prefix, alias.name.text(source));
            let owner_key = stable_signature_param_owner_key(
                stable_cone_key,
                StableDefNamespace::Type,
                &owner_fqn,
                "type_alias",
            );
            register_owner_type_param_keys(&owner_key, source, &alias.type_params, None, out);
        }
        ast::Item::Fun(fun) => {
            let owner_fqn = join_prefix(owner_prefix, fun.name.text(source));
            let owner_key = stable_signature_param_owner_key(
                stable_cone_key,
                StableDefNamespace::Fun,
                &owner_fqn,
                generic_fun_decl_kind(fun),
            );
            register_owner_type_param_keys(
                &owner_key,
                source,
                &fun.type_params,
                fun.eff_param.as_ref(),
                out,
            );
        }
        ast::Item::ExtensionProperty(prop) => {
            let owner_fqn = join_prefix(owner_prefix, prop.name.text(source));
            let owner_key = stable_signature_param_owner_key(
                stable_cone_key,
                StableDefNamespace::PropertyGetter,
                &owner_fqn,
                "generic_extension_property_getter",
            );
            register_owner_type_param_keys(&owner_key, source, &prop.type_params, None, out);
        }
        ast::Item::Type(ty) => collect_stable_type_param_keys_in_type_decl(
            source,
            owner_prefix,
            ty,
            stable_cone_key,
            out,
        ),
        ast::Item::Object(obj) => collect_stable_type_param_keys_in_object_decl(
            source,
            owner_prefix,
            obj,
            stable_cone_key,
            out,
        ),
        ast::Item::Val(_) | ast::Item::ComptimeIf(_) => {}
    }
}

pub(in crate::hir::lower) fn collect_stable_type_param_keys_in_type_decl(
    source: &SourceFile,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    stable_cone_key: &StableConeKey,
    out: &mut HashMap<TypeParamType, StableTypeParamKey>,
) {
    let owner_fqn = join_prefix(owner_prefix, decl.name.text(source));
    let owner_key = stable_signature_param_owner_key(
        stable_cone_key,
        StableDefNamespace::Type,
        &owner_fqn,
        stable_type_decl_kind(decl.kind),
    );
    register_owner_type_param_keys(
        &owner_key,
        source,
        &decl.type_params,
        decl.eff_param.as_ref(),
        out,
    );

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => {
                let owner_fqn = format!("{owner_fqn}.{}", fun.name.text(source));
                let owner_key = stable_signature_param_owner_key(
                    stable_cone_key,
                    StableDefNamespace::Fun,
                    &owner_fqn,
                    generic_fun_decl_kind(fun),
                );
                register_owner_type_param_keys(
                    &owner_key,
                    source,
                    &fun.type_params,
                    fun.eff_param.as_ref(),
                    out,
                );
            }
            ast::TypeMember::Type(nested) => collect_stable_type_param_keys_in_type_decl(
                source,
                &owner_fqn,
                nested,
                stable_cone_key,
                out,
            ),
            ast::TypeMember::Object(obj) => collect_stable_type_param_keys_in_object_decl(
                source,
                &owner_fqn,
                obj,
                stable_cone_key,
                out,
            ),
            ast::TypeMember::Property(_)
            | ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_stable_type_param_keys_in_object_decl(
    source: &SourceFile,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    stable_cone_key: &StableConeKey,
    out: &mut HashMap<TypeParamType, StableTypeParamKey>,
) {
    let Some(name) = object_decl_name(source, obj) else {
        return;
    };
    let owner_fqn = join_prefix(owner_prefix, &name);
    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => {
                let member_fqn = format!("{owner_fqn}.{}", fun.name.text(source));
                let owner_key = stable_signature_param_owner_key(
                    stable_cone_key,
                    StableDefNamespace::Fun,
                    &member_fqn,
                    generic_fun_decl_kind(fun),
                );
                register_owner_type_param_keys(
                    &owner_key,
                    source,
                    &fun.type_params,
                    fun.eff_param.as_ref(),
                    out,
                );
            }
            ast::TypeMember::Type(nested) => collect_stable_type_param_keys_in_type_decl(
                source,
                &owner_fqn,
                nested,
                stable_cone_key,
                out,
            ),
            ast::TypeMember::Object(nested) => collect_stable_type_param_keys_in_object_decl(
                source,
                &owner_fqn,
                nested,
                stable_cone_key,
                out,
            ),
            ast::TypeMember::Property(_)
            | ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn register_owner_type_param_keys(
    owner_key: &str,
    source: &SourceFile,
    type_params: &[ast::TypeParam],
    eff_param: Option<&ast::EffectRowParam>,
    out: &mut HashMap<TypeParamType, StableTypeParamKey>,
) {
    for (index, param) in type_params.iter().enumerate() {
        insert_stable_type_param_key(
            out,
            TypeParamType {
                name: param.name.text(source).to_string(),
                decl_file: source.path().to_path_buf(),
                decl_span: param.name.span,
            },
            StableTypeParamKey::new(owner_key.to_string(), index),
        );
    }
    if let Some(eff_param) = eff_param {
        insert_stable_type_param_key(
            out,
            TypeParamType {
                name: eff_param.name.text(source).to_string(),
                decl_file: std::path::PathBuf::from(EFFECT_ROW_PARAM_DECL_FILE),
                decl_span: eff_param.name.span,
            },
            StableTypeParamKey::new(owner_key.to_string(), type_params.len()),
        );
    }
}

pub(in crate::hir::lower) fn insert_stable_type_param_key(
    out: &mut HashMap<TypeParamType, StableTypeParamKey>,
    param: TypeParamType,
    key: StableTypeParamKey,
) {
    match out.get(&param) {
        Some(existing) => debug_assert_eq!(
            existing, &key,
            "stable type parameter key collision for `{}` at {:?}:{:?}",
            param.name, param.decl_file, param.decl_span,
        ),
        None => {
            out.insert(param, key);
        }
    }
}

pub(in crate::hir::lower) fn stable_type_decl_kind(kind: ast::TypeKind) -> &'static str {
    match kind {
        ast::TypeKind::Class => "class",
        ast::TypeKind::Interface => "interface",
        ast::TypeKind::Struct => "struct",
        ast::TypeKind::Enum => "enum",
        ast::TypeKind::Effect => "effect",
    }
}

pub(in crate::hir::lower) fn collect_direct_supertypes(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    let fqn = join_prefix(&pkg_prefix, ty.name.text(source));
                    out.insert(fqn, resolve_supertypes(source, file, &ty.supertypes, index));
                }
                ast::Item::Object(obj) => {
                    let Some(name) = obj.name.as_ref() else {
                        continue;
                    };
                    let fqn = join_prefix(&pkg_prefix, name.text(source));
                    out.insert(
                        fqn,
                        resolve_supertypes(source, file, &obj.supertypes, index),
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    out
}

pub(in crate::hir::lower) fn resolve_supertypes(
    source: &SourceFile,
    file: &ast::File,
    supertypes: &[ast::SuperType],
    index: &Index,
) -> Vec<String> {
    let mut resolved = supertypes
        .iter()
        .filter_map(|super_ty| index.type_ref_to_fqn_in_file(source, file, &super_ty.ty))
        .collect::<Vec<_>>();
    resolved.sort();
    resolved.dedup();
    resolved
}

#[derive(Clone, Copy)]
pub(in crate::hir::lower) struct InitCollectionCx<'a> {
    pub source: &'a SourceFile,
    pub file: &'a ast::File,
    pub index: &'a Index,
    pub type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    pub class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub interfaces: &'a crate::itable::InterfaceIndex,
    pub class_itables: &'a crate::itable::ClassItableIndex,
    pub typecheck_types: Option<&'a TypeStore>,
    pub builtins: BuiltinTypes,
    pub materialize_direct_call_targets: bool,
    pub devirtualize_dispatch_calls: bool,
}

pub(in crate::hir::lower) fn collect_object_inits(
    cx: InitCollectionCx<'_>,
    types: &mut TypeStore,
) -> Result<
    (
        ObjectInitIndex,
        CtorCallSiteIndex,
        crate::hir::DispatchCallSiteIndex,
        crate::hir::WithUpdateSiteIndex,
        crate::hir::AssignPlaceSiteIndex,
    ),
    HirLowerError,
> {
    let InitCollectionCx {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        builtins,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = cx;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let compilation_unit = [(source, file)];
    let delegated_properties: DelegatedPropertyIndex<'_> = HashMap::new();
    let default_arg_structs = super::super::collect_default_arg_structs(&compilation_unit);
    let computed_property_accessors =
        super::super::collect_computed_property_accessor_fqns(&compilation_unit);
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes(index, &compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit: &compilation_unit,
            default_arg_structs,
            computed_property_getters: &computed_property_accessors.getters,
            computed_property_setters: &computed_property_accessors.setters,
            builtins,
            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );

    let mut out: ObjectInitIndex = HashMap::new();
    for item in &file.items {
        match item {
            ast::Item::Object(obj) => {
                collect_object_decl_inits(&mut ctx, &pkg_prefix, &pkg_prefix, obj, &mut out);
            }
            ast::Item::Type(ty) => {
                collect_objects_in_type_decl(&mut ctx, &pkg_prefix, &pkg_prefix, ty, &mut out);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    ctx.record_missing_assign_place_contracts_in_object_inits(&out);
    if let Some(err) = ctx.take_stage_error() {
        return Err(err.into());
    }

    let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
    let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
    let with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
    let assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
    Ok((
        out,
        ctor_call_sites,
        dispatch_call_sites,
        with_update_contracts,
        assign_place_contracts,
    ))
}

pub(in crate::hir::lower) fn collect_objects_in_type_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    out: &mut ObjectInitIndex,
) {
    let name = decl.name.text(ctx.source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);
    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Object(obj) => {
                collect_object_decl_inits(ctx, pkg_prefix, &type_fqn, obj, out);
            }
            ast::TypeMember::Type(nested) => {
                collect_objects_in_type_decl(ctx, pkg_prefix, &type_fqn, nested, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_object_decl_inits(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    out: &mut ObjectInitIndex,
) {
    let Some(name) = object_decl_name(ctx.source, obj) else {
        return;
    };

    let fqn = join_prefix(owner_prefix, &name);
    let mut init = ObjectInit {
        fqn: fqn.clone(),
        span: obj.span,
        source_path: ctx.source.path().to_path_buf(),
        properties: HashMap::new(),
        steps: Vec::new(),
    };

    if let Some(body) = &obj.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    let name = p.name.text(ctx.source).to_string();
                    let mutable = matches!(p.kind, ast::ValKind::Var);
                    let ty =
                        p.ty.as_ref()
                            .map(|t| ctx.lower_type_ref(t))
                            .unwrap_or(ctx.builtins.any);
                    let has_init = p.init.is_some();
                    init.properties.insert(
                        name.clone(),
                        ObjectProperty {
                            name: name.clone(),
                            mutable,
                            ty,
                            has_init,
                        },
                    );

                    if let Some(expr) = p.init.as_ref() {
                        let lowered = ctx.lower_expr(pkg_prefix, expr);
                        init.steps.push(ObjectInitStep::PropertyInit {
                            name,
                            init: lowered,
                        });
                    }
                }
                ast::TypeMember::InitBlock(b) => {
                    let block = ctx.lower_block(pkg_prefix, &b.body);
                    init.steps.push(ObjectInitStep::InitBlock { block });
                }
                ast::TypeMember::Object(nested) => {
                    collect_object_decl_inits(ctx, pkg_prefix, &fqn, nested, out);
                }
                ast::TypeMember::Type(_)
                | ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    out.entry(fqn).or_insert(init);
}

pub(in crate::hir::lower) fn collect_class_inits(
    cx: InitCollectionCx<'_>,
    types: &mut TypeStore,
) -> Result<
    (
        ClassInitIndex,
        CtorCallSiteIndex,
        crate::hir::DispatchCallSiteIndex,
        crate::hir::WithUpdateSiteIndex,
        crate::hir::AssignPlaceSiteIndex,
    ),
    HirLowerError,
> {
    let InitCollectionCx {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        builtins,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = cx;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let compilation_unit = [(source, file)];
    let delegated_properties: DelegatedPropertyIndex<'_> = HashMap::new();
    let default_arg_structs = super::super::collect_default_arg_structs(&compilation_unit);
    let computed_property_accessors =
        super::super::collect_computed_property_accessor_fqns(&compilation_unit);
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes(index, &compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit: &compilation_unit,
            default_arg_structs,
            computed_property_getters: &computed_property_accessors.getters,
            computed_property_setters: &computed_property_accessors.setters,
            builtins,
            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );

    let mut out: ClassInitIndex = HashMap::new();
    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_classes_in_type_decl(&mut ctx, &pkg_prefix, &pkg_prefix, ty, &mut out);
            }
            ast::Item::Object(obj) => {
                collect_classes_in_object_decl(&mut ctx, &pkg_prefix, &pkg_prefix, obj, &mut out);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }
    ctx.record_missing_assign_place_contracts_in_class_inits(&out);
    if let Some(err) = ctx.take_stage_error() {
        return Err(err.into());
    }
    let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
    let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
    let with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
    let assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
    Ok((
        out,
        ctor_call_sites,
        dispatch_call_sites,
        with_update_contracts,
        assign_place_contracts,
    ))
}

pub(in crate::hir::lower) fn collect_classes_in_type_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    out: &mut ClassInitIndex,
) {
    let name = decl.name.text(ctx.source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Class) {
        collect_class_decl_init(ctx, pkg_prefix, &type_fqn, decl, out);
    }

    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(ctx, pkg_prefix, &type_fqn, nested, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_classes_in_object_decl(ctx, pkg_prefix, &type_fqn, obj, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_classes_in_object_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    out: &mut ClassInitIndex,
) {
    let Some(name) = object_decl_name(ctx.source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(owner_prefix, &name);

    let Some(body) = &obj.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(ctx, pkg_prefix, &obj_fqn, nested, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_classes_in_object_decl(ctx, pkg_prefix, &obj_fqn, nested, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(in crate::hir::lower) fn collect_class_decl_init(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    class_fqn: &str,
    decl: &ast::TypeDecl,
    out: &mut ClassInitIndex,
) {
    // resolver 使用 class name 的 span 作为 `this` 的 decl_span（T0313），因此这里提前 intern，
    // 以便后续 lowering 的 init blocks/ctor bodies 与 codegen 使用同一个 `SymbolId`。
    let this_id = ctx.intern_local_symbol(decl.name.span, false);

    // 仅记录“直接 superclass”的 FQN（class 单继承；interface 实现列表不计入）。
    // - typecheck（T0439）已保证“最多一个 class supertype”；
    // - 当前阶段若无法解析（例如缺失 import 或语法未覆盖），保持为 None 以便后端走最小行为。
    let super_class_fqn = decl
        .supertypes
        .iter()
        .filter_map(|s| {
            ctx.index
                .type_ref_to_fqn_in_file(ctx.source, ctx.file, &s.ty)
        })
        .find(|fqn| matches!(ctx.type_kinds.get(fqn), Some(ast::TypeKind::Class)));

    // class header 的 `: Base(args...)`：记录 super ctor args 与 typecheck 已选定的绑定（若存在）。
    let (super_ctor_args_span, super_ctor_call, super_ctor_args) = decl
        .supertypes
        .iter()
        .find(|st| st.ctor_args_span.is_some())
        .map(|st| {
            let span = st.ctor_args_span;
            let call = ctx
                .file
                .typechecked_ctor_call_binding(st.span)
                .map(|binding| CtorCallInfo {
                    class_fqn: binding.owner_fqn,
                    ctor_span: binding.ctor_span,
                    arg_mapping: binding.arg_mapping,
                });
            let args = st
                .ctor_args
                .iter()
                .map(|arg| ctx.lower_call_arg(pkg_prefix, arg))
                .collect::<Vec<_>>();
            (span, call, args)
        })
        .unwrap_or((None, None, Vec::new()));

    let mut init = ClassInit {
        fqn: class_fqn.to_string(),
        source_path: ctx.source.path().to_path_buf(),
        super_class_fqn,
        super_ctor_args_span,
        super_ctor_call,
        super_ctor_args,
        this_id,
        fields: Vec::new(),
        field_indices: HashMap::new(),
        steps: Vec::new(),
        ctors: Vec::new(),
    };

    let insert_field = |init: &mut ClassInit, field: ClassField| {
        if init.field_indices.contains_key(&field.fqn) {
            return;
        }
        let idx = init.fields.len() as u32;
        init.field_indices.insert(field.fqn.clone(), idx);
        init.fields.push(field);
    };

    // T0125：泛型 class 的 ctor 参数类型可能引用 type params（如 `T`），
    // 需要在 lowering 之前推入 type param 作用域，使 `lower_type_ref` 能够解析为 `TypeKind::Param`。
    ctx.push_type_params(&decl.type_params);

    // primary ctor（若存在）。注意：resolver 当前只会把”显式 primary ctor”加入 constructors overload set，
    // 因此这里也只收集显式 primary ctor。
    if let Some(primary) = &decl.primary_ctor {
        let mut params: Vec<ClassCtorParam> = Vec::with_capacity(primary.params.len());
        for p in &primary.params {
            let name = p.name.text(ctx.source).to_string();
            let id = ctx.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| ctx.lower_type_ref(t))
                    .unwrap_or(ctx.builtins.any);
            let is_property = p.kind.is_some();
            let property_field_fqn = is_property.then(|| format!("{class_fqn}.{name}"));

            params.push(ClassCtorParam {
                id,
                name: name.clone(),
                decl_span: p.name.span,
                ty,
                has_default: p.default_value.is_some(),
                default_value: p
                    .default_value
                    .as_ref()
                    .map(|expr| ctx.lower_expr(pkg_prefix, expr)),
                is_property,
                property_field_fqn: property_field_fqn.clone(),
            });

            // `class C(val x: T)`：`x` 同时声明字段/属性，因此需要参与实例 layout，
            // 并在 ctor 执行时先从实参写入该字段（顺序由 codegen 决定）。
            if let Some(field_fqn) = property_field_fqn {
                let mutable = matches!(p.kind, Some(ast::ValKind::Var));
                insert_field(
                    &mut init,
                    ClassField {
                        fqn: field_fqn.clone(),
                        name,
                        mutable,
                        ty,
                    },
                );
            }
        }

        init.ctors.push(ClassCtor {
            kind: ClassCtorKind::Primary,
            span: primary.params_span,
            params,
            delegation: None,
            body: None,
        });
    }

    // type body：property initializer / init blocks / secondary ctors
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    // v0：仍跳过显式 getter/setter（computed/accessor codegen 需要 function-level CFG）。
                    if p.getter.is_some() || p.setter.is_some() {
                        continue;
                    }

                    let name = p.name.text(ctx.source).to_string();
                    let ty =
                        p.ty.as_ref()
                            .map(|t| ctx.lower_type_ref(t))
                            .unwrap_or(ctx.builtins.any);

                    // delegated property（spec §10.4）：标准 delegates（lazy/observable/vetoable）与 map-backed。
                    if let Some(delegate_expr) = p.delegate.as_ref() {
                        match parse_std_delegate_expr(ctx.source, delegate_expr) {
                            Some(ParsedStdDelegateExpr::Lazy { mode, .. }) => {
                                // lazy：为属性生成两个隐藏字段：
                                // - `<name>$lazy_inited: Bool`
                                // - `<name>$lazy_value: T`
                                // - （可选）`<name>$lazy_mutex: Mutex`（当 mode 需要互斥锁时）
                                //
                                // getter 会在首次访问时写入 `<name>$lazy_value` 并把 `<name>$lazy_inited` 置 true。
                                let inited_fqn = format!("{class_fqn}.{name}$lazy_inited");
                                let value_fqn = format!("{class_fqn}.{name}$lazy_value");
                                let mutex_fqn = format!("{class_fqn}.{name}$lazy_mutex");

                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: inited_fqn.clone(),
                                        name: format!("{name}$lazy_inited"),
                                        mutable: true,
                                        ty: ctx.builtins.bool_,
                                    },
                                );
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: value_fqn,
                                        name: format!("{name}$lazy_value"),
                                        mutable: true,
                                        ty,
                                    },
                                );

                                if mode.requires_mutex() {
                                    let mutex_ty = ctx.intern_nominal(
                                        HirLowering::SYNC_MUTEX_TYPE_FQN.to_string(),
                                        Vec::new(),
                                        None,
                                    );
                                    insert_field(
                                        &mut init,
                                        ClassField {
                                            fqn: mutex_fqn.clone(),
                                            name: format!("{name}$lazy_mutex"),
                                            mutable: false,
                                            ty: mutex_ty,
                                        },
                                    );
                                    init.steps.push(ClassInitStep::PropertyInit {
                                        field_fqn: mutex_fqn,
                                        init: ctx.call_top_level_fun(
                                            p.name.span,
                                            HirLowering::SYNC_MUTEX_CREATE_FQN,
                                            Vec::new(),
                                            mutex_ty,
                                        ),
                                    });
                                }

                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn: inited_fqn,
                                    init: super::super::Expr {
                                        span: p.name.span,
                                        ty: ctx.builtins.bool_,
                                        kind: super::super::ExprKind::Literal(LiteralKind::Bool(
                                            false,
                                        )),
                                    },
                                });
                            }
                            Some(ParsedStdDelegateExpr::Observable { initial, .. })
                            | Some(ParsedStdDelegateExpr::Vetoable { initial, .. }) => {
                                // observable/vetoable：在 early stage 采用“编译器内建 delegate”策略：
                                // - 把当前值落到真实字段 `<name>`；
                                // - 注入一个内部互斥锁字段 `<name>$delegate_mutex: Mutex`；
                                // - 在 getter/setter lowering 时通过该 mutex 保障并发可见性（T1326b）。
                                let mutex_fqn = format!("{class_fqn}.{name}$delegate_mutex");
                                let mutex_ty = ctx.intern_nominal(
                                    HirLowering::SYNC_MUTEX_TYPE_FQN.to_string(),
                                    Vec::new(),
                                    None,
                                );
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: mutex_fqn.clone(),
                                        name: format!("{name}$delegate_mutex"),
                                        mutable: false,
                                        ty: mutex_ty,
                                    },
                                );
                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn: mutex_fqn,
                                    init: ctx.call_top_level_fun(
                                        p.name.span,
                                        HirLowering::SYNC_MUTEX_CREATE_FQN,
                                        Vec::new(),
                                        mutex_ty,
                                    ),
                                });

                                // 把当前值落到真实字段 `<name>`，并在初始化时写入 `initial`。
                                let field_fqn = format!("{class_fqn}.{name}");
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: field_fqn.clone(),
                                        name,
                                        mutable: true,
                                        ty,
                                    },
                                );
                                let lowered = ctx.lower_expr(pkg_prefix, &initial);
                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn,
                                    init: lowered,
                                });
                            }
                            Some(ParsedStdDelegateExpr::MapBacked { delegate }) => {
                                // map-backed：早期阶段在初始化时把 `by data` 的值写入真实字段 `<name>`。
                                //
                                // 约束：目前只支持 delegate 为 `this.data` 这类“class 字段访问”，
                                // 并要求 delegate 类型存在同名字段（`data.<name>`）。
                                let field_fqn = format!("{class_fqn}.{name}");
                                insert_field(
                                    &mut init,
                                    ClassField {
                                        fqn: field_fqn.clone(),
                                        name: name.clone(),
                                        mutable: false,
                                        ty,
                                    },
                                );

                                let delegate_field_fqn = match &delegate.kind {
                                    ast::ExprKind::MemberAccess { member, .. } => {
                                        let Some(ast::ResolvedMemberRef::Value { fqn }) =
                                            member.resolved.as_ref()
                                        else {
                                            continue;
                                        };
                                        fqn.clone()
                                    }
                                    _ => continue,
                                };

                                let Some(idx) =
                                    init.field_indices.get(&delegate_field_fqn).copied()
                                else {
                                    continue;
                                };
                                let Some(delegate_field) = init.fields.get(idx as usize) else {
                                    continue;
                                };

                                let delegate_ty_fqn = match ctx.types.kind(delegate_field.ty) {
                                    TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                                        nominal.fqn.clone()
                                    }
                                    TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                                        nominal.fqn.clone()
                                    }
                                    _ => continue,
                                };

                                let delegate_member_fqn = format!("{delegate_ty_fqn}.{name}");
                                let delegate_recv = ctx.lower_expr(pkg_prefix, &delegate);
                                let init_expr = super::super::Expr {
                                    span: p.name.span,
                                    ty: ctx.builtins.any,
                                    kind: super::super::ExprKind::MemberAccess {
                                        receiver: Box::new(delegate_recv),
                                        member: MemberAccess {
                                            span: p.name.span,
                                            name: name.clone(),
                                            resolved: Some(MemberRef::Value {
                                                id: ctx
                                                    .symbols
                                                    .intern_top_level(delegate_member_fqn.clone()),
                                                fqn: delegate_member_fqn,
                                            }),
                                        },
                                    },
                                };
                                init.steps.push(ClassInitStep::PropertyInit {
                                    field_fqn,
                                    init: init_expr,
                                });
                            }
                            None => {
                                // 非标准 delegated property：当前阶段不纳入 class init side table。
                            }
                        }
                        continue;
                    }

                    // v0：只收集“具备 backing field” 的属性；delegate/getter/setter 的完整语义留到后续任务。
                    let field_fqn = format!("{class_fqn}.{name}");
                    let mutable = matches!(p.kind, ast::ValKind::Var);

                    insert_field(
                        &mut init,
                        ClassField {
                            fqn: field_fqn.clone(),
                            name,
                            mutable,
                            ty,
                        },
                    );

                    if let Some(expr) = p.init.as_ref() {
                        let lowered = ctx.lower_expr(pkg_prefix, expr);
                        init.steps.push(ClassInitStep::PropertyInit {
                            field_fqn,
                            init: lowered,
                        });
                    }
                }
                ast::TypeMember::InitBlock(b) => {
                    let block = ctx.lower_block(pkg_prefix, &b.body);
                    init.steps.push(ClassInitStep::InitBlock { block });
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    let mut params: Vec<ClassCtorParam> = Vec::with_capacity(ctor.params.len());
                    for p in &ctor.params {
                        let name = p.name.text(ctx.source).to_string();
                        let id = ctx.intern_local_symbol(p.name.span, false);
                        let ty =
                            p.ty.as_ref()
                                .map(|t| ctx.lower_type_ref(t))
                                .unwrap_or(ctx.builtins.any);
                        params.push(ClassCtorParam {
                            id,
                            name,
                            decl_span: p.name.span,
                            ty,
                            has_default: p.default_value.is_some(),
                            default_value: p
                                .default_value
                                .as_ref()
                                .map(|expr| ctx.lower_expr(pkg_prefix, expr)),
                            is_property: false,
                            property_field_fqn: None,
                        });
                    }

                    let delegation = ctor.delegation_call.as_ref().map(|d| ClassCtorDelegation {
                        kind: d.kind,
                        span: d.span,
                        call: ctx
                            .file
                            .typechecked_ctor_call_binding(d.span)
                            .map(|binding| CtorCallInfo {
                                class_fqn: binding.owner_fqn,
                                ctor_span: binding.ctor_span,
                                arg_mapping: binding.arg_mapping,
                            }),
                        args: d
                            .args
                            .iter()
                            .map(|arg| ctx.lower_call_arg(pkg_prefix, arg))
                            .collect::<Vec<_>>(),
                    });
                    let body = ctx.lower_block(pkg_prefix, &ctor.body);
                    init.ctors.push(ClassCtor {
                        kind: ClassCtorKind::Secondary,
                        span: ctor.span,
                        params,
                        delegation,
                        body: Some(body),
                    });
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Fun(_)
                | ast::TypeMember::Type(_)
                | ast::TypeMember::Object(_) => {}
            }
        }
    }

    ctx.pop_type_params();
    out.entry(class_fqn.to_string()).or_insert(init);
}
