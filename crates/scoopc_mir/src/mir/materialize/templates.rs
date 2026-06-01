//! Generic template inventory: collects every generic fun / property-getter declaration available in the program and tags each with a stable signature key for later instance materialization.

use super::*;

#[derive(Clone)]
pub(super) struct GenericTemplateInfo {
    #[cfg(test)]
    pub(super) request_lookup_key: RequestTemplateKey,
    pub(super) template: TemplateKey,
    pub(super) stable_template_key: StableTemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) eff_param_names: Vec<String>,
    pub(super) signature_key: String,
    pub(super) has_body: bool,
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn generic_template_signature_key_with_owner_params(
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    owner_eff_param: Option<&ast::EffectRowParam>,
    fun: &ast::FunDecl,
) -> String {
    crate::hir::canonical_generic_fun_signature_key(
        stable_cone_key,
        source,
        file,
        index,
        owner_fqn,
        owner_type_params,
        owner_eff_param,
        fun,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn push_generic_template_info(
    out: &mut Vec<GenericTemplateInfo>,
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    owner_eff_param: Option<&ast::EffectRowParam>,
    fun: &ast::FunDecl,
) {
    if owner_type_params.is_empty()
        && owner_eff_param.is_none()
        && fun.type_params.is_empty()
        && fun.eff_param.is_none()
    {
        return;
    }

    let local_name = source.slice(fun.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    let signature_key = generic_template_signature_key_with_owner_params(
        stable_cone_key,
        source,
        file,
        index,
        owner_fqn,
        owner_type_params,
        owner_eff_param,
        fun,
    );
    out.push(GenericTemplateInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), fun.name.span),
        template: TemplateKey {
            fqn: fqn.clone(),
            source_path: source.path().to_path_buf(),
            decl_span: fun.span,
        },
        stable_template_key: stable_template_key_for_template(
            stable_cone_key,
            &fqn,
            StableDefNamespace::Fun,
            generic_fun_decl_kind(fun),
            &signature_key,
        ),
        type_param_names: owner_type_params
            .iter()
            .map(|param| param.name.text(source).to_string())
            .chain(
                fun.type_params
                    .iter()
                    .map(|param| param.name.text(source).to_string()),
            )
            .collect(),
        eff_param_names: owner_eff_param
            .into_iter()
            .chain(fun.eff_param.as_ref())
            .map(|param| param.name.text(source).to_string())
            .collect(),
        signature_key,
        has_body: matches!(fun.body, ast::FunBody::Block(_)),
    });
}

#[cfg(test)]
pub(super) fn generic_value_property_getter_signature_key(
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    property: &ast::PropertyDecl,
) -> String {
    crate::hir::canonical_generic_property_getter_signature_key(
        stable_cone_key,
        source,
        file,
        index,
        owner_fqn,
        owner_type_params,
        property,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn push_generic_value_property_getter_template_info(
    out: &mut Vec<GenericTemplateInfo>,
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    property: &ast::PropertyDecl,
) {
    if owner_type_params.is_empty() || property.getter.is_none() {
        return;
    }

    let local_name = source.slice(property.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    let signature_key = generic_value_property_getter_signature_key(
        stable_cone_key,
        source,
        file,
        index,
        owner_fqn,
        owner_type_params,
        property,
    );
    out.push(GenericTemplateInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), property.name.span),
        template: TemplateKey {
            fqn: fqn.clone(),
            source_path: source.path().to_path_buf(),
            decl_span: property.span,
        },
        stable_template_key: stable_template_key_for_template(
            stable_cone_key,
            &fqn,
            StableDefNamespace::PropertyGetter,
            generic_property_getter_decl_kind(property),
            &signature_key,
        ),
        type_param_names: owner_type_params
            .iter()
            .map(|param| param.name.text(source).to_string())
            .collect(),
        eff_param_names: Vec::new(),
        signature_key,
        has_body: property
            .getter
            .as_ref()
            .is_some_and(|getter| !matches!(getter.body, ast::AccessorBody::Missing)),
    });
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_generic_templates_from_type_body(
    out: &mut Vec<GenericTemplateInfo>,
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    owner_eff_param: Option<&ast::EffectRowParam>,
    owner_kind: Option<ast::TypeKind>,
    body: Option<&ast::TypeBody>,
) {
    let Some(body) = body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => push_generic_template_info(
                out,
                stable_cone_key,
                source,
                file,
                index,
                owner_fqn,
                owner_type_params,
                owner_eff_param,
                fun,
            ),
            ast::TypeMember::Property(property)
                if matches!(
                    owner_kind,
                    Some(ast::TypeKind::Struct | ast::TypeKind::Enum)
                ) =>
            {
                push_generic_value_property_getter_template_info(
                    out,
                    stable_cone_key,
                    source,
                    file,
                    index,
                    owner_fqn,
                    owner_type_params,
                    property,
                );
            }
            ast::TypeMember::Type(ty) => {
                let nested_owner = format!("{owner_fqn}.{}", ty.name.text(source));
                collect_generic_templates_from_type_body(
                    out,
                    stable_cone_key,
                    source,
                    file,
                    index,
                    &nested_owner,
                    &ty.type_params,
                    ty.eff_param.as_ref(),
                    Some(ty.kind),
                    ty.body.as_ref(),
                );
            }
            ast::TypeMember::Object(obj) => {
                let object_name = obj
                    .name
                    .as_ref()
                    .map(|name| name.text(source).to_string())
                    .or_else(|| {
                        matches!(obj.kind, ast::ObjectKind::Companion)
                            .then(|| "Companion".to_string())
                    });
                let Some(object_name) = object_name else {
                    continue;
                };
                let nested_owner = format!("{owner_fqn}.{object_name}");
                collect_generic_templates_from_type_body(
                    out,
                    stable_cone_key,
                    source,
                    file,
                    index,
                    &nested_owner,
                    &[],
                    None,
                    None,
                    obj.body.as_ref(),
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Property(_) => {}
        }
    }
}

#[cfg(test)]
pub(super) fn collect_generic_template_infos(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Vec<GenericTemplateInfo> {
    collect_generic_template_infos_with_source_cones(
        stable_cone_key,
        &HashMap::new(),
        index,
        compilation_unit,
    )
}

#[cfg(test)]
pub(super) fn collect_generic_template_infos_with_source_cones(
    stable_cone_key: &StableConeKey,
    source_cones: &HashMap<PathBuf, crate::cone::SourceConeInfo>,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Vec<GenericTemplateInfo> {
    let mut out = Vec::new();
    for (source, file) in compilation_unit {
        let source_stable_cone_key = source_cones
            .get(source.path())
            .map(|info| &info.stable_key)
            .unwrap_or(stable_cone_key);
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    push_generic_template_info(
                        &mut out,
                        source_stable_cone_key,
                        source,
                        file,
                        index,
                        &pkg_prefix,
                        &[],
                        None,
                        fun,
                    );
                }
                ast::Item::Type(ty) => {
                    let owner_fqn = if pkg_prefix.is_empty() {
                        ty.name.text(source).to_string()
                    } else {
                        format!("{pkg_prefix}.{}", ty.name.text(source))
                    };
                    collect_generic_templates_from_type_body(
                        &mut out,
                        source_stable_cone_key,
                        source,
                        file,
                        index,
                        &owner_fqn,
                        &ty.type_params,
                        ty.eff_param.as_ref(),
                        Some(ty.kind),
                        ty.body.as_ref(),
                    );
                }
                ast::Item::Object(obj) => {
                    let object_name = obj
                        .name
                        .as_ref()
                        .map(|name| name.text(source).to_string())
                        .or_else(|| {
                            matches!(obj.kind, ast::ObjectKind::Companion)
                                .then(|| "Companion".to_string())
                        });
                    let Some(object_name) = object_name else {
                        continue;
                    };
                    let owner_fqn = if pkg_prefix.is_empty() {
                        object_name
                    } else {
                        format!("{pkg_prefix}.{object_name}")
                    };
                    collect_generic_templates_from_type_body(
                        &mut out,
                        source_stable_cone_key,
                        source,
                        file,
                        index,
                        &owner_fqn,
                        &[],
                        None,
                        None,
                        obj.body.as_ref(),
                    );
                }
                ast::Item::TypeAlias(_) | ast::Item::ExtensionProperty(_) | ast::Item::Val(_) => {}
            }
        }
    }
    out
}

#[cfg(test)]
pub(super) fn collect_generic_template_infos_from_lowered_hir(
    lowered_hir: &crate::hir::LoweredHir,
) -> Vec<GenericTemplateInfo> {
    lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered_hir.member_funs.iter())
        .filter_map(|fun| generic_template_info_from_hir_fun(lowered_hir, fun))
        .collect()
}

pub(super) fn collect_generic_template_infos_from_hir_facts(
    hir_facts: &scoopc_hir::hir_facts::HirFacts,
) -> MaterializeResult<Vec<GenericTemplateInfo>> {
    hir_facts
        .declarations
        .generic_templates
        .iter()
        .map(|fact| {
            let stable_template_key = StableTemplateKey::from_canonical_text(
                fact.stable_template_key.as_str(),
            )
            .map_err(|err| {
                frontend_err(format!(
                    "HIR generic template fact `{}` has invalid stable template key: {err}",
                    fact.template_fqn
                ))
            })?;
            let mut type_param_names = fact.owner_type_param_names.clone();
            type_param_names.extend(fact.function_type_param_names.clone());
            let eff_param_names = fact
                .owner_eff_param_name
                .iter()
                .chain(fact.function_eff_param_name.iter())
                .cloned()
                .collect();
            Ok(GenericTemplateInfo {
                #[cfg(test)]
                request_lookup_key: (
                    fact.request_fqn.clone(),
                    fact.request_source_path.clone(),
                    fact.request_span,
                ),
                template: TemplateKey {
                    fqn: fact.template_fqn.clone(),
                    source_path: fact.template_source_path.clone(),
                    decl_span: fact.template_decl_span,
                },
                stable_template_key,
                type_param_names,
                eff_param_names,
                signature_key: fact.signature_key.as_str().to_string(),
                has_body: fact.has_body,
            })
        })
        .collect()
}

pub(super) fn collect_callable_body_infos_from_hir_facts(
    hir_facts: &scoopc_hir::hir_facts::HirFacts,
) -> Vec<CallableBodyInfo> {
    hir_facts
        .declarations
        .callable_bodies
        .iter()
        .map(|fact| CallableBodyInfo {
            request_lookup_key: (
                fact.request_fqn.clone(),
                fact.request_source_path.clone(),
                fact.request_span,
            ),
            source_path: fact.source_path.clone(),
            fqn: fact.fqn.clone(),
            body_span: fact.body_span,
        })
        .collect()
}

#[cfg(test)]
fn generic_template_info_from_hir_fun(
    lowered_hir: &crate::hir::LoweredHir,
    fun: &crate::hir::FunDecl,
) -> Option<GenericTemplateInfo> {
    let template = TemplateKey {
        fqn: fun.fqn.clone(),
        source_path: fun.source_path.clone(),
        decl_span: fun.span,
    };
    let stable_template_key = lowered_hir
        .generic_stable_template_keys
        .get(&template)
        .cloned()?;
    let mut type_param_names = Vec::new();
    for param in &fun.params {
        collect_type_param_names_in_type(&lowered_hir.types, param.ty, &mut type_param_names);
    }
    collect_type_param_names_in_type(&lowered_hir.types, fun.return_ty, &mut type_param_names);
    let eff_param_names = hir_fun_effect_param_names(&lowered_hir.types, fun.ty);
    Some(GenericTemplateInfo {
        request_lookup_key: (fun.fqn.clone(), fun.source_path.clone(), fun.span),
        template,
        signature_key: stable_template_key.canonical_text(),
        stable_template_key,
        type_param_names,
        eff_param_names,
        has_body: fun.body.is_some(),
    })
}

#[cfg(test)]
fn hir_fun_effect_param_names(types: &TypeStore, fun_ty: TypeId) -> Vec<String> {
    let mut names = HashSet::new();
    collect_effect_row_param_names_in_type(types, fun_ty, &mut names);
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();
    names
}

pub(super) fn stable_template_key_for_template(
    stable_cone_key: &StableConeKey,
    template_fqn: &str,
    namespace: StableDefNamespace,
    declaration_kind: &str,
    signature_key: &str,
) -> StableTemplateKey {
    StableTemplateKey::new(StableDefKey::new(
        stable_cone_key.clone(),
        namespace,
        template_fqn,
        declaration_kind,
        Some(signature_key.to_string()),
    ))
}

#[cfg(test)]
pub(super) fn generic_fun_decl_kind(fun: &ast::FunDecl) -> &'static str {
    match fun.kind {
        ast::FunDeclKind::Regular => "generic_fun",
        ast::FunDeclKind::EffectOp => "generic_effect_op",
    }
}

#[cfg(test)]
pub(super) fn generic_property_getter_decl_kind(_: &ast::PropertyDecl) -> &'static str {
    "generic_value_getter"
}

#[cfg(test)]
pub(super) fn push_callable_fun_body_info(
    out: &mut Vec<CallableBodyInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    fun: &ast::FunDecl,
) {
    if !matches!(fun.body, ast::FunBody::Block(_)) {
        return;
    }

    let local_name = source.slice(fun.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(CallableBodyInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), fun.name.span),
        source_path: source.path().to_path_buf(),
        fqn,
        body_span: fun.span,
    });
}

#[cfg(test)]
pub(super) fn push_callable_property_getter_body_info(
    out: &mut Vec<CallableBodyInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    property: &ast::PropertyDecl,
) {
    let Some(getter) = property.getter.as_ref() else {
        return;
    };
    if matches!(getter.body, ast::AccessorBody::Missing) {
        return;
    }

    let local_name = source.slice(property.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(CallableBodyInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), property.name.span),
        source_path: source.path().to_path_buf(),
        fqn,
        body_span: property.span,
    });
}

#[cfg(test)]
pub(super) fn collect_callable_body_infos_from_type_body(
    out: &mut Vec<CallableBodyInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    body: Option<&ast::TypeBody>,
) {
    let Some(body) = body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => push_callable_fun_body_info(out, source, owner_fqn, fun),
            ast::TypeMember::Property(property) => {
                push_callable_property_getter_body_info(out, source, owner_fqn, property);
            }
            ast::TypeMember::Type(ty) => {
                let nested_owner = format!("{owner_fqn}.{}", ty.name.text(source));
                collect_callable_body_infos_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    ty.body.as_ref(),
                );
            }
            ast::TypeMember::Object(obj) => {
                let object_name = obj
                    .name
                    .as_ref()
                    .map(|name| name.text(source).to_string())
                    .or_else(|| {
                        matches!(obj.kind, ast::ObjectKind::Companion)
                            .then(|| "Companion".to_string())
                    });
                let Some(object_name) = object_name else {
                    continue;
                };
                let nested_owner = format!("{owner_fqn}.{object_name}");
                collect_callable_body_infos_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    obj.body.as_ref(),
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

#[cfg(test)]
pub(super) fn collect_callable_body_infos(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Vec<CallableBodyInfo> {
    let mut out = Vec::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    push_callable_fun_body_info(&mut out, source, &pkg_prefix, fun);
                }
                ast::Item::Type(ty) => {
                    let owner_fqn = if pkg_prefix.is_empty() {
                        ty.name.text(source).to_string()
                    } else {
                        format!("{pkg_prefix}.{}", ty.name.text(source))
                    };
                    collect_callable_body_infos_from_type_body(
                        &mut out,
                        source,
                        &owner_fqn,
                        ty.body.as_ref(),
                    );
                }
                ast::Item::Object(obj) => {
                    let object_name = obj
                        .name
                        .as_ref()
                        .map(|name| name.text(source).to_string())
                        .or_else(|| {
                            matches!(obj.kind, ast::ObjectKind::Companion)
                                .then(|| "Companion".to_string())
                        });
                    let Some(object_name) = object_name else {
                        continue;
                    };
                    let owner_fqn = if pkg_prefix.is_empty() {
                        object_name
                    } else {
                        format!("{pkg_prefix}.{object_name}")
                    };
                    collect_callable_body_infos_from_type_body(
                        &mut out,
                        source,
                        &owner_fqn,
                        obj.body.as_ref(),
                    );
                }
                ast::Item::TypeAlias(_) | ast::Item::ExtensionProperty(_) | ast::Item::Val(_) => {}
            }
        }
    }
    out
}

#[cfg(test)]
pub(super) fn collect_callable_body_infos_from_lowered_hir(
    lowered_hir: &crate::hir::LoweredHir,
) -> Vec<CallableBodyInfo> {
    lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered_hir.member_funs.iter())
        .filter_map(|fun| {
            fun.body.as_ref().map(|_| CallableBodyInfo {
                request_lookup_key: (fun.fqn.clone(), fun.source_path.clone(), fun.span),
                source_path: fun.source_path.clone(),
                fqn: fun.fqn.clone(),
                body_span: fun.span,
            })
        })
        .collect()
}

pub(super) fn load_dump_support_sources(session: &Session) -> MaterializeResult<Vec<SourceFile>> {
    use miette::{Context as _, IntoDiagnostic as _};

    let mut support_paths: Vec<(PathBuf, bool)> = Vec::new();
    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（T0143）")
        .map_err(|error| frontend_err(format!("dump-ir 无法加载默认 support sources：{error}")))?;
    let sysroot_entries = crate::sysroot::collect_auto_sysroot_source_entries(
        &sysroot_root,
        session.options().sysroot_overlay(),
        session.options().extra_sysroot_dependencies(),
    )
    .map_err(|error| frontend_err(format!("dump-ir 无法加载默认 support sources：{error}")))?;

    support_paths.extend(
        sysroot_entries
            .into_iter()
            .map(|entry| (entry.path, entry.trusted_syslib)),
    );
    support_paths.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

    let mut out = Vec::with_capacity(support_paths.len());
    for (path, trusted_syslib) in support_paths {
        let source = if trusted_syslib {
            SourceFile::load_trusted_syslib(&path)
        } else {
            SourceFile::load_sysroot(&path)
        }
        .map_err(|error| frontend_err(format!("dump-ir 无法加载默认 support sources：{error}")))?;
        out.push(source);
    }
    Ok(out)
}

#[cfg(test)]
pub(super) fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(package) = package else {
        return String::new();
    };
    package
        .path
        .iter()
        .map(|seg| seg.text(source))
        .collect::<Vec<_>>()
        .join(".")
}
