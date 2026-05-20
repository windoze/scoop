//! Default-arg / computed-property / object & class init collection helpers.

#![allow(dead_code)]

use super::*;

pub(crate) fn collect_default_arg_structs(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> HashMap<String, DefaultArgStructInfo> {
    let mut out: HashMap<String, DefaultArgStructInfo> = HashMap::new();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            collect_default_arg_structs_in_type_decl(source, ty, &pkg_prefix, &mut out);
        }
    }

    out
}

#[derive(Default)]
pub(crate) struct ComputedPropertyAccessorFqns {
    pub(crate) getters: HashSet<String>,
    pub(crate) setters: HashSet<String>,
}

pub(crate) fn computed_property_has_backing_field(prop: &ast::PropertyDecl) -> bool {
    prop.delegate.is_none()
        && (prop.init.is_some()
            || prop.getter.is_none()
            || (matches!(prop.kind, ast::ValKind::Var) && prop.setter.is_none()))
}

pub(crate) fn should_lower_computed_property_getter(prop: &ast::PropertyDecl) -> bool {
    prop.getter.is_some() && !computed_property_has_backing_field(prop)
}

pub(crate) fn should_lower_computed_property_setter(prop: &ast::PropertyDecl) -> bool {
    prop.setter.is_some() && !computed_property_has_backing_field(prop)
}

pub(crate) fn computed_property_setter_fqn(property_fqn: &str) -> String {
    format!("{property_fqn}$set")
}

pub(crate) fn collect_computed_property_accessor_fqns(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> ComputedPropertyAccessorFqns {
    let mut out = ComputedPropertyAccessorFqns::default();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_computed_property_accessor_fqns_in_type_decl(
                        source,
                        ty,
                        &pkg_prefix,
                        &mut out,
                    );
                }
                ast::Item::Object(obj) => {
                    collect_computed_property_accessor_fqns_in_object_decl(
                        source,
                        obj,
                        &pkg_prefix,
                        &mut out,
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

pub(crate) fn collect_computed_property_accessor_fqns_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut ComputedPropertyAccessorFqns,
) {
    let local_name = decl.name.text(source).to_string();
    let type_fqn = join_prefix(prefix, &local_name);

    if let Some(body) = &decl.body {
        for member in &body.members {
            let ast::TypeMember::Property(prop) = member else {
                continue;
            };
            let property_fqn = format!("{}.{}", type_fqn, prop.name.text(source));
            if should_lower_computed_property_getter(prop) {
                out.getters.insert(property_fqn.clone());
            }
            if should_lower_computed_property_setter(prop) {
                out.setters.insert(property_fqn);
            }
        }
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_computed_property_accessor_fqns_in_type_decl(
                    source, nested, &type_fqn, out,
                );
            }
            ast::TypeMember::Object(obj) => {
                collect_computed_property_accessor_fqns_in_object_decl(source, obj, &type_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(crate) fn collect_computed_property_accessor_fqns_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut ComputedPropertyAccessorFqns,
) {
    let Some(obj_name) = object_decl_name(source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(prefix, &obj_name);

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_computed_property_accessor_fqns_in_type_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_computed_property_accessor_fqns_in_object_decl(
                    source, nested, &obj_fqn, out,
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

pub(crate) fn collect_default_arg_structs_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut HashMap<String, DefaultArgStructInfo>,
) {
    let local_name = decl.name.text(source).to_string();
    let type_fqn = if prefix.is_empty() {
        local_name
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        let mut params: Vec<DefaultArgParamInfo> = Vec::new();

        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name: p.name.text(source).to_string(),
                    is_vararg: p.is_vararg,
                    ty_ref: p.ty.clone(),
                    default_value: p.default_value.clone(),
                });
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                if !p.is_direct_field() {
                    continue;
                }
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name: p.name.text(source).to_string(),
                    is_vararg: false,
                    ty_ref: p.ty.clone(),
                    default_value: p.init.clone(),
                });
            }
        }

        if params.iter().any(|p| p.default_value.is_some()) {
            out.insert(
                type_fqn.clone(),
                DefaultArgStructInfo {
                    decl_file: source.path().to_path_buf(),
                    type_params: decl
                        .type_params
                        .iter()
                        .map(|p| p.name.text(source).to_string())
                        .collect(),
                    params,
                },
            );
        }
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_default_arg_structs_in_type_decl(source, nested, &type_fqn, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_default_arg_structs_in_object_decl(source, obj, &type_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

pub(crate) fn collect_default_arg_structs_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut HashMap<String, DefaultArgStructInfo>,
) {
    let Some(obj_name) = obj
        .name
        .as_ref()
        .map(|id| id.text(source).to_string())
        .or_else(|| match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        })
    else {
        return;
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_default_arg_structs_in_type_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_default_arg_structs_in_object_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CompilationUnitInitCollectionInputs<'a> {
    pub(crate) index: &'a Index,
    pub(crate) type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub(crate) known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    pub(crate) class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub(crate) interfaces: &'a crate::itable::InterfaceIndex,
    pub(crate) class_itables: &'a crate::itable::ClassItableIndex,
    pub(crate) typecheck_types: Option<&'a TypeStore>,
    pub(crate) materialize_direct_call_targets: bool,
    pub(crate) devirtualize_dispatch_calls: bool,
    pub(crate) builtins: BuiltinTypes,
}

pub(crate) fn collect_compilation_unit_object_and_class_inits(
    compilation_unit: &[(&SourceFile, &ast::File)],
    inputs: CompilationUnitInitCollectionInputs<'_>,
    types: &mut TypeStore,
) -> Result<
    (
        ObjectInitIndex,
        ClassInitIndex,
        CtorCallSiteIndex,
        crate::hir::DispatchCallSiteIndex,
        WithUpdateSiteIndex,
        AssignPlaceSiteIndex,
    ),
    HirLowerError,
> {
    let CompilationUnitInitCollectionInputs {
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
        builtins,
    } = inputs;
    let mut object_inits = ObjectInitIndex::new();
    let mut class_inits = ClassInitIndex::new();
    let mut ctor_call_sites = CtorCallSiteIndex::new();
    let mut dispatch_call_sites = crate::hir::DispatchCallSiteIndex::new();
    let mut with_update_contracts = WithUpdateSiteIndex::new();
    let mut assign_place_contracts = AssignPlaceSiteIndex::new();

    for (source, file) in compilation_unit {
        let init_collection_cx = InitCollectionCx {
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
        };
        let (
            file_object_inits,
            file_object_ctor_call_sites,
            file_object_dispatch_call_sites,
            file_object_with_update_contracts,
            file_object_assign_place_contracts,
        ) = collect_object_inits(init_collection_cx, types)?;
        object_inits.extend(file_object_inits);
        ctor_call_sites.extend(file_object_ctor_call_sites);
        dispatch_call_sites.extend(file_object_dispatch_call_sites);
        with_update_contracts.extend(file_object_with_update_contracts);
        assign_place_contracts.extend(file_object_assign_place_contracts);

        let (
            file_class_inits,
            file_class_ctor_call_sites,
            file_class_dispatch_call_sites,
            file_class_with_update_contracts,
            file_class_assign_place_contracts,
        ) = collect_class_inits(init_collection_cx, types)?;
        class_inits.extend(file_class_inits);
        ctor_call_sites.extend(file_class_ctor_call_sites);
        dispatch_call_sites.extend(file_class_dispatch_call_sites);
        with_update_contracts.extend(file_class_with_update_contracts);
        assign_place_contracts.extend(file_class_assign_place_contracts);
    }

    Ok((
        object_inits,
        class_inits,
        ctor_call_sites,
        dispatch_call_sites,
        with_update_contracts,
        assign_place_contracts,
    ))
}
