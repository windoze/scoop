//! MirInstanceMaterializer construction and initialization. Loads pre-interned site bindings, enumerates monomorph request bindings, and prepares the per-instance state used by every later phase.

use super::*;

fn filter_materialized_metadata_root(root: &MetadataRoot) -> Option<MetadataRoot> {
    match root {
        MetadataRoot::TypeAlias(alias) => alias.type_params.is_empty().then(|| root.clone()),
        MetadataRoot::Nominal(nominal) => {
            if !nominal.type_params.is_empty() {
                return None;
            }
            let mut nominal = nominal.clone();
            nominal.members = filter_materialized_decl_members(&nominal.members);
            Some(MetadataRoot::Nominal(nominal))
        }
        MetadataRoot::Object(object) => {
            let mut object = object.clone();
            object.members = filter_materialized_decl_members(&object.members);
            Some(MetadataRoot::Object(object))
        }
        MetadataRoot::ExtensionProperty(prop) => prop.type_params.is_empty().then(|| root.clone()),
    }
}

fn filter_materialized_decl_members(members: &[DeclMemberMetadata]) -> Vec<DeclMemberMetadata> {
    members
        .iter()
        .filter_map(|member| match member {
            DeclMemberMetadata::Fun(fun) if !fun.type_params.is_empty() => None,
            DeclMemberMetadata::Nested(root) => filter_materialized_metadata_root(root)
                .map(|root| DeclMemberMetadata::Nested(Box::new(root))),
            _ => Some(member.clone()),
        })
        .collect()
}

fn materialized_callable_effect_template_from_hir_fact(
    fact: scoopc_hir::hir_facts::source_sites::CallableSourceEffectFacts,
) -> MaterializeResult<super::MaterializedCallableEffectTemplate> {
    Ok(super::MaterializedCallableEffectTemplate {
        fqn: fact.fqn,
        declared_surface_row: fact
            .declared_surface_row
            .map(stable_effect_row_template_from_hir_fact)
            .transpose()
            .map_err(|error| frontend_err(format!("invalid declared effect row fact: {error}")))?,
        actual_surface_row_template: stable_effect_row_template_from_hir_fact(
            fact.inferred_surface_row_template,
        )
        .map_err(|error| frontend_err(format!("invalid actual effect row fact: {error}")))?,
        published_surface_row_template: stable_effect_row_template_from_hir_fact(
            fact.published_surface_row_template,
        )
        .map_err(|error| frontend_err(format!("invalid published effect row fact: {error}")))?,
    })
}

fn stable_effect_row_template_from_hir_fact(
    row: scoopc_hir::hir_facts::source_sites::EffectRowTemplate,
) -> Result<EffectRowTemplate, crate::stable_id::CanonicalEncodingError> {
    let terms = row
        .terms
        .into_iter()
        .map(|term| match term {
            scoopc_hir::hir_facts::source_sites::EffectRowTerm::Concrete { type_key } => {
                Ok(crate::stable_id::EffectTerm::Concrete { type_key })
            }
            scoopc_hir::hir_facts::source_sites::EffectRowTerm::Param {
                owner,
                ordinal,
                name,
            } => Ok(crate::stable_id::EffectTerm::Param {
                owner: crate::stable_id::StableDefKey::from_canonical_text(owner.as_str())?,
                ordinal,
                name,
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EffectRowTemplate::new(terms, row.closed))
}

impl MirInstanceMaterializer {
    pub(super) fn new(
        generic_file: File,
        types: TypeStore,
        builtins: BuiltinTypes,
        construction_inputs: MaterializerConstructionInputs<'_>,
        opt_level: OptLevel,
    ) -> MaterializeResult<Self> {
        let MaterializerConstructionInputs {
            stable_cone_key,
            typecheck_types: _typecheck_types,
            template_infos,
            callable_body_infos,
            callable_signatures,
            callable_effects,
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
            member_value_tys: hir_member_value_tys,
            request_sources,
            request_root_mode,
            request_root_fun_keys,
        } = construction_inputs;
        let mut generic_funs = Vec::new();
        let mut non_fun_items = Vec::new();
        for item in &generic_file.items {
            match item {
                Item::Fun(fun) => generic_funs.push(fun.clone()),
                Item::Metadata(root) => {
                    if let Some(root) = filter_materialized_metadata_root(root) {
                        non_fun_items.push(Item::Metadata(root));
                    }
                }
                Item::InitializerRoot(_) | Item::ExternGlobal(_) | Item::Todo { .. } => {
                    non_fun_items.push(item.clone());
                }
            }
        }
        let mut member_value_tys = collect_member_value_type_infos(&generic_file);
        member_value_tys.extend(hir_member_value_tys);
        let nongeneric_callable_stable_template_keys = callable_signatures
            .iter()
            .filter(|signature| !signature.has_generic_params_or_effect_param)
            .filter_map(|signature| {
                signature
                    .stable_template_key
                    .clone()
                    .map(|stable_template_key| (signature.template.clone(), stable_template_key))
            })
            .collect::<HashMap<_, _>>();
        let source_callable_signatures = callable_signatures
            .iter()
            .map(|signature| super::MaterializedCallableSignature {
                fqn: signature.template.fqn.clone(),
                param_names: signature
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect(),
                param_tys: signature.params.iter().map(|param| param.ty).collect(),
                return_ty: signature.return_ty,
            })
            .collect();
        let source_callable_effects = callable_effects
            .into_iter()
            .map(materialized_callable_effect_template_from_hir_fact)
            .collect::<MaterializeResult<Vec<_>>>()?;
        let callable_signatures = callable_signatures
            .into_iter()
            .map(|signature| (signature.template.clone(), signature))
            .collect::<HashMap<_, _>>();

        let mut root_candidates = Vec::new();
        let mut decl_only_candidates = Vec::new();
        let mut canonical_candidates = Vec::new();
        for info in template_infos {
            let template = info.template.clone();
            let root_fun = generic_funs
                .iter()
                .find(|fun| fun.fqn == template.fqn && fun.span == template.decl_span)
                .cloned();
            let Some(root_fun) = root_fun else {
                if !info.has_body {
                    let Some(signature) = callable_signatures.get(&template).cloned() else {
                        return Err(frontend_err(format!(
                            "materialize 无法定位 declaration-only generic template 的 HIR 签名：{}@{}:{:?}",
                            template.fqn,
                            template.source_path.display(),
                            template.decl_span
                        )));
                    };
                    canonical_candidates.push(TemplateCatalogCandidate {
                        template: template.clone(),
                        stable_template_key: info.stable_template_key.clone(),
                        signature_key: info.signature_key.clone(),
                        prefers_materialized_body: false,
                    });
                    decl_only_candidates.push(DeclOnlyTemplateCandidate {
                        template,
                        type_param_names: info.type_param_names,
                        eff_param_names: info.eff_param_names,
                        signature_key: info.signature_key,
                        signature,
                    });
                    continue;
                }
                return Err(materialize_err(
                    MirMaterializeError::MissingMirRootForTemplate {
                        fqn: template.fqn.clone(),
                        file: template.source_path.display().to_string(),
                        span: template.decl_span,
                        call_file: None,
                        call_site: None,
                    },
                ));
            };

            canonical_candidates.push(TemplateCatalogCandidate {
                template: template.clone(),
                stable_template_key: info.stable_template_key.clone(),
                signature_key: info.signature_key.clone(),
                prefers_materialized_body: root_fun.body.is_some(),
            });
            root_candidates.push(TemplateRootCandidate {
                template,
                type_param_names: info.type_param_names,
                eff_param_names: info.eff_param_names,
                signature_key: info.signature_key,
                root_fun,
            });
        }

        let canonical_templates = canonical_template_map(&canonical_candidates);
        let mut canonical_stable_keys = HashMap::new();
        for candidate in &canonical_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every template candidate");
            canonical_stable_keys
                .entry(canonical)
                .or_insert_with(|| candidate.stable_template_key.clone());
        }

        let mut stable_template_keys = HashMap::new();
        for candidate in &canonical_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every stable template alias");
            let stable_template_key = canonical_stable_keys
                .get(&canonical)
                .cloned()
                .expect("every canonical template should retain a stable template key");
            stable_template_keys.insert(candidate.template.clone(), stable_template_key);
        }

        let mut roots = HashMap::new();
        let mut template_signatures = HashMap::new();
        for candidate in root_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every root candidate");
            if candidate.template != canonical || roots.contains_key(&canonical) {
                continue;
            }

            let family = generic_funs
                .iter()
                .filter(|fun| belongs_to_template_family(fun, &candidate.root_fun))
                .cloned()
                .collect::<Vec<_>>();
            template_signatures.insert(
                canonical.clone(),
                TemplateSignatureInfo {
                    type_param_names: candidate.type_param_names.clone(),
                    eff_param_names: candidate.eff_param_names.clone(),
                    fun_ty: candidate.root_fun.ty,
                    return_ty: candidate.root_fun.return_ty,
                    params: candidate
                        .root_fun
                        .params
                        .iter()
                        .map(|param| CallableSignatureParam {
                            name: param.name.clone(),
                            ty: param.ty,
                        })
                        .collect(),
                },
            );
            roots.insert(
                canonical.clone(),
                TemplateRootInfo {
                    template: canonical,
                    type_param_names: candidate.type_param_names,
                    eff_param_names: candidate.eff_param_names,
                    family,
                },
            );
        }

        for candidate in decl_only_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every decl-only candidate");
            if candidate.template != canonical || template_signatures.contains_key(&canonical) {
                continue;
            }

            template_signatures.insert(
                canonical.clone(),
                TemplateSignatureInfo {
                    type_param_names: candidate.type_param_names,
                    eff_param_names: candidate.eff_param_names,
                    fun_ty: candidate.signature.fun_ty,
                    return_ty: candidate.signature.return_ty,
                    params: candidate.signature.params,
                },
            );
        }

        let template_symbol_suffixes = build_template_symbol_suffixes(&canonical_stable_keys);
        let mut templates_by_stable_key = stable_template_keys
            .iter()
            .map(|(template, stable_key)| (stable_key.clone(), template.clone()))
            .collect::<HashMap<_, _>>();
        templates_by_stable_key.extend(
            nongeneric_callable_stable_template_keys
                .iter()
                .map(|(template, stable_key)| (stable_key.clone(), template.clone())),
        );
        let mut roots_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
        for template in template_signatures.keys() {
            roots_by_fqn
                .entry(template.fqn.clone())
                .or_default()
                .push(template.clone());
        }

        let request_root_funs = request_root_fun_keys
            .into_iter()
            .filter_map(|key| {
                generic_funs
                    .iter()
                    .find(|fun| fun.fqn == key.fqn && fun.span == key.span)
                    .cloned()
                    .map(|fun| ReachableMirFun {
                        source_path: key.source_path,
                        fun,
                    })
            })
            .collect::<Vec<_>>();

        let generic_family_fqns = roots
            .values()
            .flat_map(|root| root.family.iter().map(|fun| fun.fqn.clone()))
            .collect::<HashSet<_>>();

        let mut reachable_fun_bodies_by_request = HashMap::new();
        let mut reachable_fun_bodies_by_fqn: HashMap<String, Vec<ReachableMirFun>> = HashMap::new();
        let mut all_fun_bodies_by_fqn: HashMap<String, Vec<FunDecl>> = HashMap::new();
        for info in callable_body_infos {
            let Some(fun) = generic_funs
                .iter()
                .find(|fun| fun.fqn == info.fqn && fun.span == info.body_span)
                .cloned()
            else {
                continue;
            };
            let reachable = ReachableMirFun {
                source_path: info.source_path.clone(),
                fun,
            };
            reachable_fun_bodies_by_request.insert(info.request_lookup_key, reachable.clone());
            reachable_fun_bodies_by_fqn
                .entry(reachable.fun.fqn.clone())
                .or_default()
                .push(reachable);
        }
        for fun in &generic_funs {
            let Some(_) = &fun.body else {
                continue;
            };
            let entry = all_fun_bodies_by_fqn.entry(fun.fqn.clone()).or_default();
            if entry.iter().any(|existing| existing.span == fun.span) {
                continue;
            }
            entry.push(fun.clone());
        }

        let direct_call_bindings = lowered_top_level_fun_call_bindings.clone();

        let mut materializer = Self {
            stable_cone_key,
            types,
            builtins,
            opt_level,
            known_receiver_subclasses,
            direct_subclasses,
            non_fun_items,
            class_vtables,
            interfaces,
            class_itables,
            enum_layouts,
            extern_funs,
            native_callable_funs,
            request_root_funs,
            hir_direct_instance_keys_by_fun: HashMap::new(),
            generic_family_fqns,
            templates_by_stable_key,
            roots,
            source_callable_signatures,
            source_callable_effects,
            template_signatures,
            stable_template_keys,
            nongeneric_callable_stable_template_keys,
            template_symbol_suffixes,
            roots_by_fqn,
            direct_call_bindings,
            ctor_call_sites,
            top_level_vars,
            top_level_immutable_values,
            object_inits,
            class_inits,
            member_value_tys,
            request_sources,
            filter_initial_requests_to_reachable_call_sites: matches!(
                request_root_mode,
                super::super::MaterializeRequestRootMode::EntryMain { .. }
            ),
            reachable_fun_bodies_by_request,
            reachable_fun_bodies_by_fqn,
            all_fun_bodies_by_fqn,
            call_bindings: HashMap::new(),
            value_ref_bindings: HashMap::new(),
            reachable_request_call_sites: HashSet::new(),
            reachable_request_stmt_spans: Vec::new(),
            scanned_top_level_vars: HashSet::new(),
            scanned_top_level_immutable_values: HashSet::new(),
            scanned_object_inits: HashSet::new(),
            scanned_class_inits: HashSet::new(),
            scanned_non_generic_funs: HashSet::new(),
            caller_side_pass_candidates: Vec::new(),
            pass_published_ordinary_callables: Vec::new(),
            materialized_direct_call_result_tys: HashMap::new(),
            dispatch_devirtualization_targets: HashMap::new(),
            queued: HashSet::new(),
            queue: VecDeque::new(),
            materialized: HashMap::new(),
            declaration_only_instances: HashSet::new(),
        };
        materializer.load_hir_call_site_instance_bindings(call_site_instance_facts)?;
        materializer.load_hir_template_site_bindings(template_site_binding_facts)?;
        Ok(materializer)
    }

    pub(super) fn collect_top_level_value_tys(&self) -> HashMap<String, TypeId> {
        let mut tys = self
            .top_level_immutable_values
            .iter()
            .map(|(fqn, value)| (fqn.clone(), value.ty))
            .collect::<HashMap<_, _>>();
        tys.extend(
            self.top_level_vars
                .iter()
                .map(|(fqn, value)| (fqn.clone(), value.ty)),
        );
        tys
    }

    pub(super) fn load_hir_call_site_instance_bindings(
        &mut self,
        facts: Vec<scoopc_hir::hir_facts::source_sites::CallSiteInstanceFact>,
    ) -> MaterializeResult<()> {
        for fact in facts {
            let stable_template_key = StableTemplateKey::from_canonical_text(
                fact.template_key.as_str(),
            )
            .map_err(|err| {
                frontend_err(format!(
                    "HIR call-site instance fact has invalid stable template key: {err}"
                ))
            })?;
            let stable_instance_key = StableInstanceKey::from_canonical_text(
                fact.stable_instance_key.as_str(),
            )
            .map_err(|err| {
                frontend_err(format!(
                    "HIR call-site instance fact has invalid stable instance key: {err}"
                ))
            })?;
            if stable_instance_key.template() != &stable_template_key {
                return Err(frontend_err(
                    "HIR call-site instance fact stable template/instance mismatch",
                ));
            }
            let Some(binding) = self.instance_key_from_stable_instance_key(&stable_instance_key)?
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: stable_template_key.def().owner_path().to_string(),
                        file: fact.identity.source_path.display().to_string(),
                        span: fact.identity.span,
                        call_file: Some(fact.identity.source_path.display().to_string()),
                        call_site: Some(fact.identity.span),
                    },
                ));
            };
            self.call_bindings.insert(
                (fact.identity.source_path, fact.identity.span),
                SiteInstanceBinding {
                    template: binding.template,
                    type_args: binding.type_args,
                    eff_args: binding.eff_args,
                },
            );
        }
        Ok(())
    }

    pub(super) fn load_hir_template_site_bindings(
        &mut self,
        facts: Vec<scoopc_hir::hir_facts::source_sites::TemplateSiteBindingFact>,
    ) -> MaterializeResult<()> {
        for fact in facts {
            if fact.type_args.is_empty() && fact.eff_args.is_empty() {
                continue;
            }
            let stable_template_key = StableTemplateKey::from_canonical_text(
                fact.template_key.as_str(),
            )
            .map_err(|err| {
                frontend_err(format!(
                    "HIR template-site binding fact has invalid stable template key: {err}"
                ))
            })?;
            let Some(template) = self.resolve_stable_request_template(&stable_template_key) else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: stable_template_key.def().owner_path().to_string(),
                        file: fact.identity.source_path.display().to_string(),
                        span: fact.identity.span,
                        call_file: Some(fact.identity.source_path.display().to_string()),
                        call_site: Some(fact.identity.span),
                    },
                ));
            };
            let binding = SiteInstanceBinding {
                template,
                type_args: fact.type_args,
                eff_args: fact.eff_args,
            };
            match fact.kind {
                scoopc_hir::hir_facts::source_sites::TemplateSiteBindingKind::DirectCall => {
                    self.call_bindings
                        .insert((fact.identity.source_path, fact.identity.span), binding);
                }
                scoopc_hir::hir_facts::source_sites::TemplateSiteBindingKind::FunValue => {
                    self.value_ref_bindings
                        .insert((fact.identity.source_path, fact.identity.span), binding);
                }
            }
        }
        Ok(())
    }

    pub(super) fn load_monomorph_request_site_bindings(
        &mut self,
        typecheck_types: &TypeStore,
        monomorph_requests: &[MonomorphRequest],
    ) -> MaterializeResult<()> {
        for request in monomorph_requests {
            let key = &request.key;
            if key.type_args.is_empty() && key.eff_args.is_empty() {
                continue;
            }
            if !instance_request_is_concrete(typecheck_types, &key.type_args, &key.eff_args) {
                continue;
            }
            let Some(stable_template_key) = key.stable_template_key.as_ref() else {
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
            let Some(template) = self.resolve_stable_request_template(stable_template_key) else {
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
            let Some(stable_instance_key) = key.stable_instance_key.as_ref() else {
                return Err(frontend_err(format!(
                    "monomorph request `{}` 缺少 stable instance key",
                    key.symbol.fqn
                )));
            };
            let (type_args, eff_args) =
                self.localize_stable_request_args(typecheck_types, key, stable_instance_key)?;
            self.call_bindings.insert(
                (request.request_source_path.clone(), request.call_span),
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }
        Ok(())
    }
}
