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

pub(in crate::hir::lower) fn collect_interior_mutable_nominals(
    pairs: &[(&SourceFile, &ast::File)],
    type_env: Option<&crate::typecheck::TypeEnv>,
) -> HashSet<String> {
    if let Some(type_env) = type_env {
        return type_env
            .interior_mutable_nominal_fqns()
            .map(|fqn| fqn.to_string())
            .collect();
    }

    let mut out: HashSet<String> = HashSet::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Struct | ast::TypeKind::Class) {
                continue;
            }
            if !ty.annotations.iter().any(|ann| {
                crate::typecheck::builtin_annotation_kind(source, ann)
                    == Some(crate::typecheck::BuiltinAnnotationKind::InteriorMutable)
            }) {
                continue;
            }
            out.insert(join_prefix(&pkg_prefix, ty.name.text(source)));
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
    let source_cones = HashMap::<std::path::PathBuf, crate::cone::SourceConeInfo>::new();
    collect_stable_type_param_keys_with_source_cones(
        compilation_unit,
        stable_cone_key,
        &source_cones,
    )
}

pub(crate) fn collect_stable_type_param_keys_with_source_cones(
    compilation_unit: &[(&SourceFile, &ast::File)],
    stable_cone_key: &StableConeKey,
    source_cones: &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> HashMap<TypeParamType, StableTypeParamKey> {
    let mut out = HashMap::new();
    for (source, file) in compilation_unit {
        let source_stable_cone_key =
            stable_cone_key_for_source(source, stable_cone_key, source_cones);
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            collect_stable_type_param_keys_in_item(
                source,
                item,
                &pkg_prefix,
                source_stable_cone_key,
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
        ast::Item::Val(_) => {}
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
                | ast::Item::TypeAlias(_) => {}
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
    pub compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    pub index: &'a Index,
    pub type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub typecheck_types: Option<&'a TypeStore>,
    pub builtins: BuiltinTypes,
    pub materialize_direct_call_targets: bool,
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
        compilation_unit,
        index,
        type_kinds,
        typecheck_types,
        builtins,
        materialize_direct_call_targets,
    } = cx;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties =
        collect_delegated_properties(compilation_unit, index, typecheck_types);
    let default_arg_structs = super::super::collect_default_arg_structs(compilation_unit);
    let computed_property_accessors =
        super::super::collect_computed_property_accessor_fqns(compilation_unit);
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes(index, compilation_unit);
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
            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
            materialize_direct_call_targets,
        },
    );
    ctx.next_synthetic_call_site = 50_000;

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
            | ast::Item::TypeAlias(_) => {}
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
        GenericClassDeclIndex,
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
        compilation_unit,
        index,
        type_kinds,
        typecheck_types,
        builtins,
        materialize_direct_call_targets,
    } = cx;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties =
        collect_delegated_properties(compilation_unit, index, typecheck_types);
    let default_arg_structs = super::super::collect_default_arg_structs(compilation_unit);
    let computed_property_accessors =
        super::super::collect_computed_property_accessor_fqns(compilation_unit);
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes(index, compilation_unit);
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
            generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
            materialize_direct_call_targets,
        },
    );
    ctx.next_synthetic_call_site = 100_000;

    let mut out: GenericClassDeclIndex = HashMap::new();
    let mut mono_out: ClassInitIndex = HashMap::new();
    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                collect_classes_in_type_decl(
                    &mut ctx,
                    &pkg_prefix,
                    &pkg_prefix,
                    ty,
                    &mut out,
                    &mut mono_out,
                )?;
            }
            ast::Item::Object(obj) => {
                collect_classes_in_object_decl(
                    &mut ctx,
                    &pkg_prefix,
                    &pkg_prefix,
                    obj,
                    &mut out,
                    &mut mono_out,
                )?;
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
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
        mono_out,
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
    out: &mut GenericClassDeclIndex,
    mono_out: &mut ClassInitIndex,
) -> Result<(), HirLowerError> {
    let name = decl.name.text(ctx.source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Class) {
        collect_class_decl_init(ctx, pkg_prefix, &type_fqn, decl, out, mono_out)?;
    }

    let Some(body) = &decl.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(ctx, pkg_prefix, &type_fqn, nested, out, mono_out)?;
            }
            ast::TypeMember::Object(obj) => {
                collect_classes_in_object_decl(ctx, pkg_prefix, &type_fqn, obj, out, mono_out)?;
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
    Ok(())
}

pub(in crate::hir::lower) fn collect_classes_in_object_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    out: &mut GenericClassDeclIndex,
    mono_out: &mut ClassInitIndex,
) -> Result<(), HirLowerError> {
    let Some(name) = object_decl_name(ctx.source, obj) else {
        return Ok(());
    };
    let obj_fqn = join_prefix(owner_prefix, &name);

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(ctx, pkg_prefix, &obj_fqn, nested, out, mono_out)?;
            }
            ast::TypeMember::Object(nested) => {
                collect_classes_in_object_decl(ctx, pkg_prefix, &obj_fqn, nested, out, mono_out)?;
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
    Ok(())
}

pub(in crate::hir::lower) fn collect_class_decl_init(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    class_fqn: &str,
    decl: &ast::TypeDecl,
    out: &mut GenericClassDeclIndex,
    mono_out: &mut ClassInitIndex,
) -> Result<(), HirLowerError> {
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

    let mut init = GenericClassDecl {
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

    let insert_field = |init: &mut GenericClassDecl, field: ClassField<TypeId>| {
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
        let mut params: Vec<ClassCtorParam<TypeId>> = Vec::with_capacity(primary.params.len());
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

                    // delegated property（spec §10.4）：store the delegate object in a normal
                    // `$delegate` field; reads/writes lower to `getValue` / `setValue` calls.
                    if let Some(delegate_expr) = p.delegate.as_ref() {
                        let field_fqn = format!("{class_fqn}.{name}$delegate");
                        let property_fqn = format!("{class_fqn}.{name}");
                        let delegate_ty = ctx
                            .delegated_properties
                            .get(&property_fqn)
                            .cloned()
                            .map(|info| {
                                if let Some(class_fqn) = info.delegate_class_fqn.as_ref()
                                    && ctx.type_param_count_for_nominal_fqn(class_fqn) == Some(1)
                                {
                                    ctx.intern_nominal(class_fqn.clone(), vec![ty], None)
                                } else {
                                    ctx.specialized_delegated_property_delegate_ty(&info, ty)
                                }
                            })
                            .unwrap_or(ctx.builtins.any);
                        let expected_fun_binding = ctx
                            .typechecked_top_level_fun_call_binding(delegate_expr.span)
                            .map(|binding| {
                                ctx.fun_call_binding_with_expected_return(
                                    binding,
                                    Some(delegate_ty),
                                )
                            });
                        if let Some(binding) = expected_fun_binding.as_ref() {
                            let mut bindings = ctx.file.top_level_fun_call_bindings();
                            bindings.insert(delegate_expr.span, binding.clone());
                            ctx.file.replace_top_level_fun_call_bindings(bindings);
                        }
                        let mut init_expr = ctx.lower_expr_with_expected(
                            pkg_prefix,
                            delegate_expr,
                            ExpectedExpr {
                                value_ty: Some(delegate_ty),
                                ..ExpectedExpr::default()
                            },
                        );
                        let materialized_target = expected_fun_binding
                            .as_ref()
                            .and_then(|binding| {
                                ctx.materialized_direct_call_target_fqn_for_binding(binding)
                            })
                            .or_else(|| {
                                materialized_delegate_factory_target_fqn(
                                    ctx,
                                    delegate_expr,
                                    delegate_ty,
                                )
                            });
                        if let Some(target_fqn) = materialized_target
                            && let super::super::ExprKind::Call { callee, .. } = &mut init_expr.kind
                        {
                            **callee =
                                ctx.top_level_callee_expr_with_fqn(delegate_expr.span, target_fqn);
                        }
                        insert_field(
                            &mut init,
                            ClassField {
                                fqn: field_fqn.clone(),
                                name: format!("{name}$delegate"),
                                mutable: false,
                                ty: delegate_ty,
                            },
                        );
                        init.steps.push(ClassInitStep::PropertyInit {
                            field_fqn,
                            init: init_expr,
                        });
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
                    let mut params: Vec<ClassCtorParam<TypeId>> =
                        Vec::with_capacity(ctor.params.len());
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

    // 非泛型 class 在此处直接构造 MonoClassInit；泛型 class 仅入 generic_class_decls，
    // 等监 monomorph driver 在 substitute 后产出 MonoClassInit。
    let is_generic = !decl.type_params.is_empty();
    let entry_fqn = class_fqn.to_string();
    let inserted = out.entry(entry_fqn.clone()).or_insert(init);
    let entry_key = crate::hir::ClassInstanceKey::for_unparameterized(&entry_fqn);
    if !is_generic && !mono_out.contains_key(&entry_key) {
        match crate::hir::MonoClassInit::from_generic_decl(inserted, ctx.types) {
            Ok(mono) => {
                mono_out.insert(entry_key, mono);
            }
            Err(diag) => {
                return Err(HirStageError::new(
                    ctx.source.path(),
                    decl.name.span,
                    format!(
                        "non-generic class `{}` failed to monomorphize: {}",
                        diag.class_fqn, diag
                    ),
                    diag.class_fqn.clone(),
                )
                .into());
            }
        }
    }
    Ok(())
}

fn materialized_delegate_factory_target_fqn(
    ctx: &HirLowering<'_>,
    delegate_expr: &ast::Expr,
    delegate_ty: TypeId,
) -> Option<String> {
    let ast::ExprKind::Call { callee, args } = &delegate_expr.kind else {
        return None;
    };
    let ast::ExprKind::Ident(id) = &callee.kind else {
        return None;
    };
    let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
        return None;
    };
    let type_args = match ctx.types.kind(delegate_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.args.clone(),
        _ => return None,
    };
    if type_args.is_empty() {
        return None;
    }
    let overload = ctx.index.by_fqn.get(fqn).and_then(|symbols| {
        symbols
            .fun
            .iter()
            .find(|overload| overload.sig.params.len() == args.len())
            .or_else(|| symbols.fun.first())
    })?;
    if overload.sig.type_params.len() != type_args.len() {
        return None;
    }
    Some(ctx.materialized_instance_fqn_for_decl(
        fqn,
        overload.symbol.decl_file.as_path(),
        overload.symbol.span,
        &type_args,
        &[],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cone::{ConeId, ConeKind, SourceConeInfo, SourceConeTrust};
    use crate::parser::parse_file;

    fn source_cone_info(id: u32, stable_key: StableConeKey) -> SourceConeInfo {
        SourceConeInfo {
            id: ConeId::new(id),
            kind: ConeKind::Lib,
            stable_key,
            trust: SourceConeTrust::Untrusted,
        }
    }

    #[test]
    fn stable_type_param_keys_use_owning_source_cone_key() {
        let dep_source = SourceFile::new_virtual(
            "/tmp/scoop-stable-keys/dep/src/lib.scoop",
            "package shared\nfun depId<T>(value: T): T = value\n",
        );
        let app_source = SourceFile::new_virtual(
            "/tmp/scoop-stable-keys/app/src/main.scoop",
            "package shared\nfun appId<T>(value: T): T = value\n",
        );
        let dep_ast = parse_file(&dep_source).unwrap();
        let app_ast = parse_file(&app_source).unwrap();
        let dep_key = StableConeKey::new("dep.cone", "0.1.0");
        let app_key = StableConeKey::new("app.cone", "0.1.0");
        let fallback_key = StableConeKey::new("fallback.cone", "0.1.0");
        let source_cones = HashMap::from([
            (
                dep_source.path().to_path_buf(),
                source_cone_info(2, dep_key),
            ),
            (
                app_source.path().to_path_buf(),
                source_cone_info(1, app_key),
            ),
        ]);

        let keys = collect_stable_type_param_keys_with_source_cones(
            &[(&dep_source, &dep_ast), (&app_source, &app_ast)],
            &fallback_key,
            &source_cones,
        );
        let owner_keys = keys
            .values()
            .map(StableTypeParamKey::owner_def_key)
            .collect::<Vec<_>>();

        assert!(owner_keys.iter().any(|key| key.contains("dep.cone")));
        assert!(owner_keys.iter().any(|key| key.contains("app.cone")));
        assert!(!owner_keys.iter().any(|key| key.contains("fallback.cone")));
    }
}
