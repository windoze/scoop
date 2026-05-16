//! Main materialization loop. Pulls instance keys off the queue, builds the per-instance type/effect substitution, and drives body rewriting until the worklist is empty.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn run(
        mut self,
        initial_requests: Vec<InstanceKey>,
    ) -> MaterializeResult<MaterializedMir> {
        for request in initial_requests {
            self.enqueue(request);
        }

        while let Some(instance) = self.queue.pop_front() {
            self.queued.remove(&instance);
            if self.materialized.contains_key(&instance) {
                continue;
            }
            let family = self.materialize_instance(&instance)?;
            let mut discovered_requests = Vec::new();
            self.scan_materialized_family_reachable_calls(
                &instance,
                &family,
                &mut discovered_requests,
            )?;
            for request in discovered_requests {
                self.enqueue(request);
            }
            self.materialized.insert(instance, family);
        }

        let mut materialized_instance_keys = self.materialized.keys().cloned().collect::<Vec<_>>();
        materialized_instance_keys.sort_by_key(|a| {
            (
                self.instance_display_fqn(a),
                self.instance_exported_fun_symbol(a),
            )
        });
        let materialized_instance_set = materialized_instance_keys
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let decl_only_instances = self
            .declaration_only_instances
            .iter()
            .filter(|instance| !materialized_instance_set.contains(*instance))
            .cloned()
            .collect::<Vec<_>>();

        let mut instance_keys = materialized_instance_keys.clone();
        instance_keys.extend(decl_only_instances.iter().cloned());
        instance_keys.sort_by_key(|a| {
            (
                self.instance_display_fqn(a),
                self.instance_exported_fun_symbol(a),
            )
        });
        instance_keys.dedup();

        let mut pass_visible_non_generic_roots = self
            .pass_published_ordinary_callables
            .iter()
            .map(|published| {
                (
                    self.non_generic_pass_view_instance_key(
                        published.source_path.as_path(),
                        &published.fun,
                    ),
                    published.fun.clone(),
                )
            })
            .collect::<Vec<_>>();
        pass_visible_non_generic_roots
            .sort_by_key(|(instance, _)| self.instance_display_fqn(instance));
        pass_visible_non_generic_roots.dedup_by(|(left, _), (right, _)| left == right);
        for (_, fun) in &mut pass_visible_non_generic_roots {
            if let Some(body) = fun.body.as_mut() {
                self.repair_array_call_transport_types(body);
                self.repair_closure_capture_transport_targets(body);
                self.repair_handle_payload_metadata_types(body);
                self.repair_materialized_generic_transport_call_args(body);
                self.repair_transport_target_local_types(body);
                self.repair_perform_payload_metadata_types(body);
                self.repair_unused_unresolved_compiler_temporaries(body);
            }
        }

        let mut pass_instance_keys = instance_keys.clone();
        pass_instance_keys.extend(
            pass_visible_non_generic_roots
                .iter()
                .map(|(instance, _)| instance.clone()),
        );
        pass_instance_keys.sort_by_key(|a| {
            (
                self.instance_display_fqn(a),
                self.instance_exported_fun_symbol(a),
            )
        });
        pass_instance_keys.dedup();
        let stable_instance_keys = pass_instance_keys
            .iter()
            .cloned()
            .map(|instance| {
                let stable_key = self.stable_instance_key(&instance);
                (instance, stable_key)
            })
            .collect::<HashMap<_, _>>();

        let root_instances = materialized_instance_keys
            .iter()
            .cloned()
            .map(|instance| InstanceRootSummaryInput {
                root_fqn: self.instance_display_fqn(&instance),
                instance,
            })
            .collect::<Vec<_>>();
        let mut pass_root_instances = root_instances.clone();
        pass_root_instances.extend(
            pass_visible_non_generic_roots
                .iter()
                .map(|(instance, fun)| InstanceRootSummaryInput {
                    instance: instance.clone(),
                    root_fqn: fun.fqn.clone(),
                }),
        );
        let decl_only_inputs = decl_only_instances
            .iter()
            .filter_map(|instance| {
                let signature = self.template_signatures.get(&instance.template)?;
                let substitution =
                    self.build_instance_substitution_for_signature(signature, instance);
                Some(DeclOnlySummaryInput {
                    instance: instance.clone(),
                    root_fqn: self.instance_display_fqn(instance),
                    declared_fun_ty: substitute_type_and_effect_params(
                        &mut self.types,
                        signature.fun_ty,
                        &substitution,
                    ),
                    declared_return_ty: substitute_type_and_effect_params(
                        &mut self.types,
                        signature.return_ty,
                        &substitution,
                    ),
                    param_count: signature.params.len(),
                })
            })
            .collect::<Vec<_>>();
        let mut callable_family_inputs = materialized_instance_keys
            .iter()
            .cloned()
            .map(|instance| {
                let root_fqn = self.instance_display_fqn(&instance);
                let mut callable_fqns = self
                    .materialized
                    .get(&instance)
                    .cloned()
                    .expect("materialized instance should exist")
                    .into_iter()
                    .filter(|fun| fun.body.is_some())
                    .map(|fun| fun.fqn)
                    .collect::<Vec<_>>();
                callable_fqns.sort_by(|a, b| {
                    let a_root = a == &root_fqn;
                    let b_root = b == &root_fqn;
                    (!a_root).cmp(&!b_root).then_with(|| a.cmp(b))
                });
                callable_fqns.dedup();
                MaterializedCallableFamilyInput {
                    instance,
                    root_fqn,
                    callable_fqns,
                }
            })
            .collect::<Vec<_>>();
        let mut pass_callable_family_inputs = callable_family_inputs.clone();
        pass_callable_family_inputs.extend(pass_visible_non_generic_roots.iter().map(
            |(instance, fun)| MaterializedCallableFamilyInput {
                instance: instance.clone(),
                root_fqn: fun.fqn.clone(),
                callable_fqns: vec![fun.fqn.clone()],
            },
        ));
        let decl_only_callable_family_inputs = decl_only_instances
            .iter()
            .cloned()
            .map(|instance| MaterializedCallableFamilyInput {
                root_fqn: self.instance_display_fqn(&instance),
                instance,
                callable_fqns: Vec::new(),
            })
            .collect::<Vec<_>>();
        callable_family_inputs.extend(decl_only_callable_family_inputs.clone());
        pass_callable_family_inputs.extend(decl_only_callable_family_inputs);
        let callable_families = MaterializedCallableFamilies::from_inputs(callable_family_inputs);
        let pass_callable_families =
            MaterializedCallableFamilies::from_inputs(pass_callable_family_inputs);

        let mut items = Vec::new();
        for key in &materialized_instance_keys {
            let mut family = self
                .materialized
                .get(key)
                .cloned()
                .expect("materialized instance should exist");
            family.sort_by(|a, b| {
                let a_root = a.fqn == self.instance_display_fqn(key);
                let b_root = b.fqn == self.instance_display_fqn(key);
                (!a_root).cmp(&!b_root).then_with(|| a.fqn.cmp(&b.fqn))
            });
            items.extend(family.into_iter().map(Item::Fun));
        }
        let mut pass_items = items.clone();
        pass_items.extend(
            pass_visible_non_generic_roots
                .into_iter()
                .map(|(_, fun)| Item::Fun(fun)),
        );
        let file = File { items };
        let pass_file = File { items: pass_items };
        let summaries = build_materialized_summary_table(
            &file,
            &self.types,
            &root_instances,
            &decl_only_inputs,
        );
        let pass_summaries = build_materialized_summary_table(
            &pass_file,
            &self.types,
            &pass_root_instances,
            &decl_only_inputs,
        );
        let pass_artifacts = MaterializedMirPassArtifacts::from_initial_publication(
            &pass_file,
            &pass_summaries,
            &pass_callable_families,
            &pass_instance_keys,
        );
        let top_level_value_tys = self.collect_top_level_value_tys();

        let mut materialized = MaterializedMir {
            file,
            types: self.types,
            instance_keys,
            summaries,
            top_level_value_tys,
            stable_cone_key: self.stable_cone_key,
            stable_instance_keys,
            stable_template_keys: self.stable_template_keys,
            nongeneric_callable_signature_keys: self.nongeneric_callable_signature_keys,
            opt_level: self.opt_level,
            callable_families,
            pass_artifacts,
            caller_side_pass_candidates: self.caller_side_pass_candidates,
        };
        if self.enable_summary_driven_inlining {
            super::super::inline::run_summary_driven_inlining(&mut materialized);
        }
        if self.enable_mir_escape_analysis {
            super::super::escape::run_escape_analysis(&mut materialized);
            if super::super::closure_simplify::run_non_escaping_closure_simplification(
                &mut materialized,
            ) {
                super::super::escape::run_escape_analysis(&mut materialized);
            }
        }
        materialized.validate_materialized()?;
        Ok(materialized)
    }

    pub(super) fn enqueue(&mut self, key: InstanceKey) {
        if self.materialized.contains_key(&key)
            || self.declaration_only_instances.contains(&key)
            || self.queued.contains(&key)
        {
            return;
        }

        if self.roots.contains_key(&key.template) {
            self.queued.insert(key.clone());
            self.queue.push_back(key);
        } else if self.template_signatures.contains_key(&key.template) {
            self.declaration_only_instances.insert(key);
        }
    }

    pub(super) fn materialize_instance(
        &mut self,
        instance: &InstanceKey,
    ) -> MaterializeResult<Vec<FunDecl>> {
        let Some(root) = self.roots.get(&instance.template).cloned() else {
            return Err(materialize_err(
                MirMaterializeError::MissingGenericTemplate {
                    fqn: instance.template.fqn.clone(),
                    file: instance.template.source_path.display().to_string(),
                    span: instance.template.decl_span,
                    call_file: None,
                    call_site: None,
                },
            ));
        };

        if root.type_param_names.len() != instance.type_args.len() {
            return Err(materialize_err(MirMaterializeError::TypeArgArityMismatch {
                fqn: root.template.fqn.clone(),
                expected: root.type_param_names.len(),
                found: instance.type_args.len(),
                call_site: None,
                decl_span: root.template.decl_span.into(),
            }));
        }

        let substitution = self.build_instance_substitution(&root, instance)?;
        let instance_root_fqn = self.instance_display_fqn(instance);

        let mut out = Vec::with_capacity(root.family.len());
        for template_fun in &root.family {
            let mut fun = template_fun.clone();
            fun.fqn = rewrite_family_symbol_name(&fun.fqn, &root.template.fqn, &instance_root_fqn)
                .unwrap_or_else(|| fun.fqn.clone());
            fun.ty = substitute_type_and_effect_params(&mut self.types, fun.ty, &substitution);
            for param in &mut fun.params {
                param.ty =
                    substitute_type_and_effect_params(&mut self.types, param.ty, &substitution);
            }
            fun.return_ty =
                substitute_type_and_effect_params(&mut self.types, fun.return_ty, &substitution);
            if let Some(body) = &mut fun.body {
                self.rewrite_body(
                    body,
                    &substitution,
                    &root.template.source_path,
                    &root.template.fqn,
                    &instance_root_fqn,
                )?;
            }
            out.push(fun);
        }

        Ok(out)
    }

    pub(super) fn build_instance_substitution(
        &self,
        root: &TemplateRootInfo,
        instance: &InstanceKey,
    ) -> MaterializeResult<InstanceSubstitution> {
        let mut substitution = InstanceSubstitution {
            type_params: root
                .type_param_names
                .iter()
                .cloned()
                .zip(instance.type_args.iter().copied())
                .collect(),
            effect_params: HashMap::new(),
        };

        match (&root.eff_param_name, instance.eff_args.as_slice()) {
            (None, []) => {}
            (None, eff_args) => {
                return Err(materialize_err(
                    MirMaterializeError::EffectArgArityMismatch {
                        fqn: root.template.fqn.clone(),
                        expected: 0,
                        found: eff_args.len(),
                        call_site: None,
                        decl_span: root.template.decl_span.into(),
                    },
                ));
            }
            (Some(name), [row]) => {
                substitution.effect_params.insert(name.clone(), row.clone());
            }
            (Some(_), eff_args) => {
                return Err(materialize_err(
                    MirMaterializeError::EffectArgArityMismatch {
                        fqn: root.template.fqn.clone(),
                        expected: 1,
                        found: eff_args.len(),
                        call_site: None,
                        decl_span: root.template.decl_span.into(),
                    },
                ));
            }
        }

        Ok(substitution)
    }

    pub(super) fn build_instance_substitution_for_signature(
        &self,
        signature: &TemplateSignatureInfo,
        instance: &InstanceKey,
    ) -> InstanceSubstitution {
        let mut substitution = InstanceSubstitution {
            type_params: signature
                .type_param_names
                .iter()
                .cloned()
                .zip(instance.type_args.iter().copied())
                .collect(),
            effect_params: HashMap::new(),
        };
        if let (Some(name), [row]) = (&signature.eff_param_name, instance.eff_args.as_slice()) {
            substitution.effect_params.insert(name.clone(), row.clone());
        }
        substitution
    }

    pub(super) fn rewrite_body(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        self.rewrite_body_blocks(
            body,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
            None,
        )?;
        self.repair_direct_call_result_types(body);
        self.repair_array_call_transport_types(body);
        self.repair_closure_capture_transport_targets(body);
        self.repair_handle_payload_metadata_types(body);
        self.repair_materialized_generic_transport_call_args(body);
        self.repair_transport_target_local_types(body);
        self.repair_perform_payload_metadata_types(body);
        self.repair_unused_unresolved_compiler_temporaries(body);
        Ok(())
    }

    pub(super) fn rewrite_reachable_body(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        let reachable_blocks = reachable_body_block_indices(body);
        self.rewrite_body_blocks(
            body,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
            Some(reachable_blocks),
        )?;
        self.repair_direct_call_result_types(body);
        self.repair_array_call_transport_types(body);
        self.repair_closure_capture_transport_targets(body);
        self.repair_handle_payload_metadata_types(body);
        self.repair_materialized_generic_transport_call_args(body);
        self.repair_transport_target_local_types(body);
        self.repair_perform_payload_metadata_types(body);
        self.repair_unused_unresolved_compiler_temporaries(body);
        Ok(())
    }
}
