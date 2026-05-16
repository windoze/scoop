//! Generic template inventory: collects every generic fun / property-getter declaration available in the program and tags each with a stable signature key for later instance materialization.

use super::*;

#[derive(Clone)]
pub(super) struct GenericTemplateInfo {
    pub(super) request_lookup_key: RequestTemplateKey,
    pub(super) template: TemplateKey,
    pub(super) stable_template_key: StableTemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) eff_param_name: Option<String>,
    pub(super) signature_key: String,
    pub(super) has_body: bool,
}

pub(super) fn generic_template_signature_key_with_owner_params(
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    fun: &ast::FunDecl,
) -> String {
    crate::hir::canonical_generic_fun_signature_key(
        stable_cone_key,
        source,
        file,
        index,
        owner_fqn,
        owner_type_params,
        fun,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_generic_template_info(
    out: &mut Vec<GenericTemplateInfo>,
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
    fun: &ast::FunDecl,
) {
    if owner_type_params.is_empty() && fun.type_params.is_empty() && fun.eff_param.is_none() {
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
        eff_param_name: fun
            .eff_param
            .as_ref()
            .map(|param| param.name.text(source).to_string()),
        signature_key,
        has_body: matches!(fun.body, ast::FunBody::Block(_)),
    });
}

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
        eff_param_name: None,
        signature_key,
        has_body: property
            .getter
            .as_ref()
            .is_some_and(|getter| !matches!(getter.body, ast::AccessorBody::Missing)),
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_generic_templates_from_type_body(
    out: &mut Vec<GenericTemplateInfo>,
    stable_cone_key: &StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    owner_fqn: &str,
    owner_type_params: &[ast::TypeParam],
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

pub(super) fn collect_generic_template_infos(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Vec<GenericTemplateInfo> {
    let mut out = Vec::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    push_generic_template_info(
                        &mut out,
                        stable_cone_key,
                        source,
                        file,
                        index,
                        &pkg_prefix,
                        &[],
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
                        stable_cone_key,
                        source,
                        file,
                        index,
                        &owner_fqn,
                        &ty.type_params,
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
                        stable_cone_key,
                        source,
                        file,
                        index,
                        &owner_fqn,
                        &[],
                        None,
                        obj.body.as_ref(),
                    );
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Val(_) => {}
            }
        }
    }
    out
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

pub(super) fn generic_fun_decl_kind(fun: &ast::FunDecl) -> &'static str {
    match fun.kind {
        ast::FunDeclKind::Regular => "generic_fun",
        ast::FunDeclKind::EffectOp => "generic_effect_op",
    }
}

pub(super) fn generic_property_getter_decl_kind(_: &ast::PropertyDecl) -> &'static str {
    "generic_value_getter"
}

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
                ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Val(_) => {}
            }
        }
    }
    out
}

pub(super) fn load_dump_support_sources(session: &Session) -> MaterializeResult<Vec<SourceFile>> {
    crate::frontend::load_default_support_sources(session.options())
        .map_err(|error| frontend_err(format!("dump-ir 无法加载默认 support sources：{error}")))
}

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
