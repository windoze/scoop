//! MirInstanceMaterializer construction and initialization. Loads pre-interned site bindings, enumerates monomorph request bindings, and prepares the per-instance state used by every later phase.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn new(
        generic_file: File,
        types: TypeStore,
        builtins: BuiltinTypes,
        construction_inputs: MaterializerConstructionInputs<'_>,
        opt_level: OptLevel,
        enable_summary_driven_inlining: bool,
        enable_mir_escape_analysis: bool,
    ) -> MaterializeResult<Self> {
        let MaterializerConstructionInputs {
            stable_cone_key,
            typecheck_types,
            template_infos,
            callable_body_infos,
            callable_signatures,
            known_receiver_subclasses,
            direct_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            top_level_fun_value_refs,
            top_level_fun_call_bindings,
            lowered_top_level_fun_call_bindings,
            top_level_vars,
            top_level_consts,
            top_level_immutable_values,
            object_inits,
            class_inits,
            member_value_tys: hir_member_value_tys,
            request_sources,
            request_root_mode,
            request_root_fun_keys,
        } = construction_inputs;
        let mut generic_funs = Vec::new();
        for item in &generic_file.items {
            if let Item::Fun(fun) = item {
                generic_funs.push(fun.clone());
            }
        }
        let mut member_value_tys = collect_member_value_type_infos(&generic_file);
        member_value_tys.extend(hir_member_value_tys);
        let nongeneric_callable_signature_keys = callable_signatures
            .iter()
            .filter(|signature| !signature.has_generic_params_or_effect_param)
            .filter_map(|signature| {
                canonical_callable_signature_key(
                    &types,
                    signature.fun_ty,
                    0,
                    0,
                    0,
                    &NoTypeParamResolver,
                )
                .ok()
                .map(|signature_key| (signature.template.clone(), signature_key))
            })
            .collect::<HashMap<_, _>>();
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
                        request_lookup_key: info.request_lookup_key,
                        template,
                        type_param_names: info.type_param_names,
                        eff_param_name: info.eff_param_name,
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
                request_lookup_key: info.request_lookup_key,
                template,
                type_param_names: info.type_param_names,
                eff_param_name: info.eff_param_name,
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

        let mut request_templates = HashMap::new();
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
            request_templates.insert(candidate.request_lookup_key, canonical.clone());

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
                    template: canonical.clone(),
                    type_param_names: candidate.type_param_names.clone(),
                    eff_param_name: candidate.eff_param_name.clone(),
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
                    eff_param_name: candidate.eff_param_name,
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
            request_templates.insert(candidate.request_lookup_key, canonical.clone());

            if candidate.template != canonical || template_signatures.contains_key(&canonical) {
                continue;
            }

            template_signatures.insert(
                canonical.clone(),
                TemplateSignatureInfo {
                    template: canonical,
                    type_param_names: candidate.type_param_names,
                    eff_param_name: candidate.eff_param_name,
                    fun_ty: candidate.signature.fun_ty,
                    return_ty: candidate.signature.return_ty,
                    params: candidate.signature.params,
                },
            );
        }

        let template_symbol_suffixes = build_template_symbol_suffixes(&canonical_stable_keys);
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

        let mut direct_call_bindings = top_level_fun_call_bindings.clone();
        direct_call_bindings.extend(lowered_top_level_fun_call_bindings.clone());

        let mut materializer = Self {
            stable_cone_key,
            types,
            builtins,
            opt_level,
            known_receiver_subclasses,
            direct_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            request_root_funs,
            hir_direct_instance_keys_by_fun: HashMap::new(),
            generic_family_fqns,
            request_templates,
            roots,
            template_signatures,
            stable_template_keys,
            nongeneric_callable_signature_keys,
            template_symbol_suffixes,
            roots_by_fqn,
            explicit_dispatch_candidate_instances: HashMap::new(),
            direct_call_bindings,
            top_level_vars,
            top_level_consts,
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
            scanned_top_level_consts: HashSet::new(),
            scanned_top_level_immutable_values: HashSet::new(),
            scanned_object_inits: HashSet::new(),
            scanned_class_inits: HashSet::new(),
            scanned_non_generic_funs: HashSet::new(),
            caller_side_pass_candidates: Vec::new(),
            pass_published_ordinary_callables: Vec::new(),
            materialized_direct_call_result_tys: HashMap::new(),
            enable_summary_driven_inlining,
            enable_mir_escape_analysis,
            queued: HashSet::new(),
            queue: VecDeque::new(),
            materialized: HashMap::new(),
            declaration_only_instances: HashSet::new(),
        };
        materializer.explicit_dispatch_candidate_instances =
            materializer.collect_explicit_dispatch_candidate_instances(typecheck_types);
        materializer.load_site_instance_bindings(
            typecheck_types,
            top_level_fun_value_refs,
            top_level_fun_call_bindings,
        )?;
        materializer
            .load_preinterned_call_site_instance_bindings(lowered_top_level_fun_call_bindings)?;
        Ok(materializer)
    }

    pub(super) fn collect_top_level_value_tys(&self) -> HashMap<String, TypeId> {
        let mut tys = self
            .top_level_consts
            .iter()
            .map(|(fqn, value)| (fqn.clone(), value.ty))
            .collect::<HashMap<_, _>>();
        tys.extend(
            self.top_level_immutable_values
                .iter()
                .map(|(fqn, value)| (fqn.clone(), value.ty)),
        );
        tys.extend(
            self.top_level_vars
                .iter()
                .map(|(fqn, value)| (fqn.clone(), value.ty)),
        );
        tys
    }

    pub(super) fn load_site_instance_bindings(
        &mut self,
        typecheck_types: &TypeStore,
        top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
        top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    ) -> MaterializeResult<()> {
        for (site, binding) in top_level_fun_call_bindings {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                        call_file: Some(site.0.display().to_string()),
                        call_site: Some(site.1),
                    },
                ));
            };
            let type_args = binding
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = binding
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            self.call_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }

        for (site, binding) in top_level_fun_value_refs {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                        call_file: Some(site.0.display().to_string()),
                        call_site: Some(site.1),
                    },
                ));
            };
            let type_args = binding
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = binding
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            self.value_ref_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }

        Ok(())
    }

    pub(super) fn load_preinterned_call_site_instance_bindings(
        &mut self,
        top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    ) -> MaterializeResult<()> {
        for (site, binding) in top_level_fun_call_bindings {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                        call_file: Some(site.0.display().to_string()),
                        call_site: Some(site.1),
                    },
                ));
            };
            self.call_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args: binding.type_args,
                    eff_args: binding.eff_args,
                },
            );
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
            let Some(template) = self.resolve_request_template(
                &key.symbol.fqn,
                &key.symbol.decl_file,
                key.symbol.decl_span,
            ) else {
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
            let type_args = key
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = key
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
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
