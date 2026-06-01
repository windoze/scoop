//! Public materialization entry points used by callers (dump-ir path, typechecked-input path).

use super::*;

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances。
pub fn materialize_for_dump(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<MaterializedMir> {
    materialize_for_dump_with_opt_level(session, source, OptLevel::O2)
}

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances，并显式指定 MIR pass 优化等级。
pub fn materialize_for_dump_with_opt_level(
    session: &Session,
    source: &SourceFile,
    opt_level: OptLevel,
) -> MaterializeResult<MaterializedMir> {
    let DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types,
        monomorph_requests,
    } = collect_dump_materialization_inputs(session, source)?;
    let compilation_unit = prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    super::super::materialize_compilation_unit_from_typechecked_inputs_with_opt_level(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &index,
        Some(&env),
        &typecheck_types,
        &monomorph_requests,
        opt_level,
    )
}

/// 基于既有 typechecked compilation-unit facts 执行 generic MIR template -> monomorphic
/// instance materialization。
///
/// 说明：
/// - 该入口直接复用调用方已经准备好的 `Index` / `TypeEnv` / `TypeStore` /
///   `MonomorphRequest` 与 AST side tables，不重新跑 parse/resolve/typecheck；
/// - dump/debug 路径目前通过它做包装，后续 build/frontend 主路径也将复用同一层。
pub(crate) fn materialize_compilation_unit_from_typechecked_inputs(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    type_env: Option<&TypeEnv>,
    typecheck_types: &TypeStore,
    monomorph_requests: &[MonomorphRequest],
    options: super::super::MaterializeCompilationUnitOptions<'_>,
) -> MaterializeResult<MaterializedMir> {
    let super::super::MaterializeCompilationUnitOptions {
        stable_cone_key,
        source_cones,
        request_source_paths,
        request_root_mode,
        opt_level,
    } = options;
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        stable_cone_key.clone(),
        index,
        compilation_unit,
        compilation_unit,
        type_env,
        typecheck_types,
    )?;
    lowered_hir.source_cones.extend(
        source_cones
            .iter()
            .map(|(path, info)| (path.clone(), info.clone())),
    );
    let request_root_fun_keys =
        collect_request_root_fun_keys(&lowered_hir, request_source_paths, index, request_root_mode);
    let request_sources = request_source_paths.iter().cloned().collect::<HashSet<_>>();
    let callable_signatures = collect_callable_signature_infos(&lowered_hir);
    let member_value_tys = collect_member_value_type_infos_from_hir_decls(&lowered_hir.file.decls);
    let lowered_top_level_fun_call_bindings =
        collect_lowered_top_level_fun_call_bindings(&lowered_hir);
    let ctor_call_sites = lowered_hir.ctor_call_sites.clone();
    let top_level_vars = lowered_hir.top_level_vars.clone();
    let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
    let object_inits = lowered_hir.object_inits.clone();
    let class_inits = lowered_hir.class_inits.clone();
    let known_receiver_subclasses =
        super::super::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
    let direct_subclasses =
        collect_direct_subclasses_from_supertypes(&lowered_hir.direct_supertypes);
    let class_vtables = lowered_hir.class_vtables.clone();
    let interfaces = lowered_hir.interfaces.clone();
    let class_itables = lowered_hir.class_itables.clone();
    let enum_layouts = lowered_hir.enum_layouts.clone();
    let extern_funs = lowered_hir.extern_funs.clone();
    let native_callable_funs = lowered_hir.native_callable_funs.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let default_contract_source_path = request_source_paths
        .first()
        .map(PathBuf::as_path)
        .or_else(|| compilation_unit.first().map(|(source, _)| source.path()))
        .ok_or_else(|| {
            materialize_err(MirMaterializeError::Frontend {
                message: "materialize compilation unit must contain at least one source file"
                    .to_string(),
            })
        })?;
    let hir_facts = scoopc_hir::stage::build_hir_facts_from_lowered_hir(
        &lowered_hir,
        default_contract_source_path,
    )
    .map_err(crate::hir::HirLowerError::from)?;
    let template_catalog = collect_generic_template_infos_from_hir_facts(&hir_facts)?;
    let callable_body_infos = collect_callable_body_infos_from_hir_facts(&hir_facts);
    let monomorph_requests = stabilize_monomorph_requests_from_hir_facts(
        typecheck_types,
        monomorph_requests,
        &hir_facts,
    )?;
    let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests_from_hir_facts(
        &lowered_hir,
        &hir_facts,
        &template_catalog,
    )?;
    let call_site_instance_facts = hir_facts.source_sites.call_site_instances.clone();
    let template_site_binding_facts = hir_facts.source_sites.template_site_bindings.clone();
    let facts = super::super::MirLoweringFacts::from_hir_facts(&lowered_hir, &hir_facts);
    let generic_file = super::super::lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = lowered_hir.types;

    materialize_generic_mir(
        generic_file,
        types,
        builtins,
        MaterializeRequestSet {
            monomorph_requests: &monomorph_requests,
            hir_direct_instance_keys_by_fun,
            construction_inputs: MaterializerConstructionInputs {
                stable_cone_key,
                typecheck_types,
                template_infos: template_catalog,
                callable_body_infos,
                callable_signatures,
                call_site_instance_facts,
                template_site_binding_facts,
                known_receiver_subclasses,
                direct_subclasses,
                class_vtables,
                interfaces,
                class_itables,
                enum_layouts,
                extern_funs,
                native_callable_funs,
                lowered_top_level_fun_call_bindings,
                ctor_call_sites,
                top_level_vars,
                top_level_immutable_values,
                object_inits,
                class_inits,
                member_value_tys,
                request_sources,
                request_root_mode,
                request_root_fun_keys,
            },
        },
        opt_level,
    )
}

fn collect_hir_direct_call_instance_requests_from_hir_facts(
    lowered_hir: &crate::hir::LoweredHir,
    hir_facts: &scoopc_hir::hir_facts::HirFacts,
    template_catalog: &[GenericTemplateInfo],
) -> MaterializeResult<HashMap<(PathBuf, Span), Vec<InstanceKey>>> {
    let templates_by_stable_key = template_catalog
        .iter()
        .map(|info| (info.stable_template_key.clone(), info.template.clone()))
        .collect::<HashMap<_, _>>();
    let mut out = HashMap::<(PathBuf, Span), Vec<InstanceKey>>::new();
    for fact in &hir_facts.source_sites.call_site_instances {
        let instance =
            hir_call_site_instance_fact_key(&lowered_hir.types, &templates_by_stable_key, fact)?;
        let Some(owner) =
            containing_hir_fun_site(lowered_hir, &fact.identity.source_path, fact.identity.span)
        else {
            continue;
        };
        out.entry(owner).or_default().push(instance);
    }
    for instances in out.values_mut() {
        instances.sort_by_key(|instance| {
            (
                instance.template.fqn.clone(),
                instance.template.source_path.clone(),
                instance.template.decl_span.start,
                instance.template.decl_span.end,
                instance
                    .type_args
                    .iter()
                    .map(|ty| ty.as_u32())
                    .collect::<Vec<_>>(),
            )
        });
        instances.dedup();
    }
    Ok(out)
}

fn hir_call_site_instance_fact_key(
    types: &TypeStore,
    templates_by_stable_key: &HashMap<StableTemplateKey, TemplateKey>,
    fact: &scoopc_hir::hir_facts::source_sites::CallSiteInstanceFact,
) -> MaterializeResult<InstanceKey> {
    let stable_instance_key =
        StableInstanceKey::from_canonical_text(fact.stable_instance_key.as_str())
            .map_err(|err| frontend_err(format!("invalid HIR stable instance key: {err}")))?;
    let Some(template) = templates_by_stable_key
        .get(stable_instance_key.template())
        .cloned()
    else {
        return Err(frontend_err(format!(
            "HIR call-site instance references unpublished stable template `{}`",
            stable_instance_key.template().canonical_text()
        )));
    };
    let type_args = stable_instance_key
        .canonical_type_args()
        .iter()
        .map(|canonical| {
            find_canonical_type_in_store(types, canonical).ok_or_else(|| {
                frontend_err(format!(
                    "unable to localize HIR stable type argument `{canonical}`"
                ))
            })
        })
        .collect::<MaterializeResult<Vec<_>>>()?;
    let eff_args = stable_instance_key
        .effect_arg_templates()
        .iter()
        .map(|template| {
            template
                .to_effect_row_with(|type_key| {
                    find_canonical_type_in_store(types, type_key.as_str())
                })
                .map_err(|err| {
                    frontend_err(format!(
                        "unable to localize HIR stable effect row `{template}`: {err}"
                    ))
                })
        })
        .collect::<MaterializeResult<Vec<_>>>()?;
    Ok(InstanceKey {
        template,
        type_args,
        eff_args,
    })
}

fn containing_hir_fun_site(
    lowered_hir: &crate::hir::LoweredHir,
    source_path: &Path,
    span: Span,
) -> Option<(PathBuf, Span)> {
    fn span_contains(outer: Span, inner: Span) -> bool {
        outer.start <= inner.start && inner.end <= outer.end
    }

    lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered_hir.member_funs.iter())
        .filter(|fun| fun.source_path == source_path && span_contains(fun.span, span))
        .min_by_key(|fun| fun.span.end.saturating_sub(fun.span.start))
        .map(|fun| (fun.source_path.clone(), fun.span))
}

#[cfg(test)]
pub(super) fn stabilize_monomorph_requests(
    typecheck_types: &TypeStore,
    monomorph_requests: &[MonomorphRequest],
    template_catalog: &[GenericTemplateInfo],
) -> MaterializeResult<Vec<MonomorphRequest>> {
    let templates_by_request = template_catalog
        .iter()
        .map(|info| {
            (
                info.request_lookup_key.clone(),
                info.stable_template_key.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    monomorph_requests
        .iter()
        .map(|request| {
            let key = &request.key;
            if key.type_args.is_empty() && key.eff_args.is_empty() {
                return Ok(request.clone());
            }
            if !instance_request_is_concrete(typecheck_types, &key.type_args, &key.eff_args) {
                return Ok(request.clone());
            }
            let lookup_key = (
                key.symbol.fqn.clone(),
                key.symbol.decl_file.clone(),
                key.symbol.decl_span,
            );
            let stable_template_key = templates_by_request
                .get(&lookup_key)
                .cloned()
                .or_else(|| stable_template_key_by_containing_span(template_catalog, key));
            let Some(stable_template_key) = stable_template_key else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: key.symbol.fqn.clone(),
                        file: key.symbol.decl_file.display().to_string(),
                        span: key.symbol.decl_span,
                        call_file: Some(request.request_source_path.display().to_string()),
                        call_site: Some(request.call_span),
                    },
                ));
            };
            let stable_instance_key = StableInstanceKey::from_type_arguments(
                stable_template_key.clone(),
                typecheck_types,
                &key.type_args,
                &key.eff_args,
                &NoTypeParamResolver,
            )
            .map_err(|err| {
                frontend_err(format!(
                    "无法为 monomorph request `{}` 构造 stable instance key: {err}",
                    key.symbol.fqn
                ))
            })?;
            let mut request = request.clone();
            request.key = request
                .key
                .with_stable_identity(stable_template_key, stable_instance_key);
            Ok(request)
        })
        .collect()
}

#[cfg(test)]
fn stable_template_key_by_containing_span(
    template_catalog: &[GenericTemplateInfo],
    key: &crate::monomorph::MonomorphKey,
) -> Option<StableTemplateKey> {
    fn span_contains(outer: Span, inner: Span) -> bool {
        outer.start <= inner.start && inner.end <= outer.end
    }

    template_catalog
        .iter()
        .find(|info| {
            info.request_lookup_key.0 == key.symbol.fqn
                && info.request_lookup_key.1 == key.symbol.decl_file
                && (span_contains(info.template.decl_span, key.symbol.decl_span)
                    || span_contains(key.symbol.decl_span, info.request_lookup_key.2))
        })
        .map(|info| info.stable_template_key.clone())
}

pub(super) fn stabilize_monomorph_requests_from_hir_facts(
    typecheck_types: &TypeStore,
    monomorph_requests: &[MonomorphRequest],
    hir_facts: &scoopc_hir::hir_facts::HirFacts,
) -> MaterializeResult<Vec<MonomorphRequest>> {
    monomorph_requests
        .iter()
        .map(|request| {
            let key = &request.key;
            if key.type_args.is_empty() && key.eff_args.is_empty() {
                return Ok(request.clone());
            }
            if !instance_request_is_concrete(typecheck_types, &key.type_args, &key.eff_args) {
                return Ok(request.clone());
            }
            if key.stable_template_key.is_some() && key.stable_instance_key.is_some() {
                return Ok(request.clone());
            }
            let (stable_template_key, stable_instance_key) =
                stable_request_identity_from_hir_facts(typecheck_types, hir_facts, request)?;
            let mut request = request.clone();
            request.key = request
                .key
                .with_stable_identity(stable_template_key, stable_instance_key);
            Ok(request)
        })
        .collect()
}

fn stable_request_identity_from_hir_facts(
    typecheck_types: &TypeStore,
    hir_facts: &scoopc_hir::hir_facts::HirFacts,
    request: &MonomorphRequest,
) -> MaterializeResult<(StableTemplateKey, StableInstanceKey)> {
    let key = &request.key;
    if let Some(fact) = stable_call_site_instance_fact_for_request(hir_facts, request) {
        let stable_template_key =
            StableTemplateKey::from_canonical_text(fact.template_key.as_str()).map_err(|err| {
                frontend_err(format!(
                    "HIR call-site fact for `{}` has invalid stable template key: {err}",
                    key.symbol.fqn
                ))
            })?;
        let stable_instance_key = StableInstanceKey::from_canonical_text(
            fact.stable_instance_key.as_str(),
        )
        .map_err(|err| {
            frontend_err(format!(
                "HIR call-site fact for `{}` has invalid stable instance key: {err}",
                key.symbol.fqn
            ))
        })?;
        if stable_instance_key.template() != &stable_template_key {
            return Err(frontend_err(format!(
                "HIR call-site fact for `{}` has mismatched stable template/instance keys",
                key.symbol.fqn
            )));
        }
        return Ok((stable_template_key, stable_instance_key));
    }

    let Some(template_fact) = hir_facts
        .declarations
        .generic_templates
        .iter()
        .find(|fact| {
            fact.request_fqn == key.symbol.fqn
                && fact.request_source_path == key.symbol.decl_file
                && fact.request_span == key.symbol.decl_span
        })
    else {
        return Err(materialize_err(
            MirMaterializeError::MissingGenericTemplate {
                fqn: key.symbol.fqn.clone(),
                file: key.symbol.decl_file.display().to_string(),
                span: key.symbol.decl_span,
                call_file: Some(request.request_source_path.display().to_string()),
                call_site: Some(request.call_span),
            },
        ));
    };
    let stable_template_key =
        StableTemplateKey::from_canonical_text(template_fact.stable_template_key.as_str())
            .map_err(|err| {
                frontend_err(format!(
                    "HIR generic template fact for `{}` has invalid stable template key: {err}",
                    key.symbol.fqn
                ))
            })?;
    let stable_instance_key = StableInstanceKey::from_type_arguments(
        stable_template_key.clone(),
        typecheck_types,
        &key.type_args,
        &key.eff_args,
        &NoTypeParamResolver,
    )
    .map_err(|err| {
        frontend_err(format!(
            "无法为 monomorph request `{}` 构造 stable instance key: {err}",
            key.symbol.fqn
        ))
    })?;
    Ok((stable_template_key, stable_instance_key))
}

fn stable_call_site_instance_fact_for_request<'a>(
    hir_facts: &'a scoopc_hir::hir_facts::HirFacts,
    request: &MonomorphRequest,
) -> Option<&'a scoopc_hir::hir_facts::source_sites::CallSiteInstanceFact> {
    hir_facts
        .source_sites
        .call_site_instances
        .iter()
        .filter(|fact| fact.identity.source_path == request.request_source_path)
        .find(|fact| fact.identity.span == request.call_span)
}
