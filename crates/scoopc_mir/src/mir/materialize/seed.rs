//! Seeds the materialization queue from monomorph requests: turns each declared instance request into an InstanceKey ready for the main loop to process.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn resolve_request_template(
        &self,
        fqn: &str,
        decl_file: &Path,
        decl_span: Span,
    ) -> Option<TemplateKey> {
        self.request_templates
            .get(&(fqn.to_string(), decl_file.to_path_buf(), decl_span))
            .cloned()
    }

    pub(super) fn resolve_request_template_by_decl_site(
        &self,
        fqn: &str,
        decl_file: &Path,
        decl_span: Span,
    ) -> Option<TemplateKey> {
        fn span_contains(outer: Span, inner: Span) -> bool {
            outer.start <= inner.start && inner.end <= outer.end
        }

        self.resolve_request_template(fqn, decl_file, decl_span)
            .or_else(|| {
                self.request_templates
                    .iter()
                    .find(|((request_fqn, request_file, request_span), template)| {
                        request_fqn == fqn
                            && request_file.as_path() == decl_file
                            && (*request_span == decl_span
                                || span_contains(template.decl_span, decl_span)
                                || span_contains(decl_span, *request_span))
                    })
                    .map(|(_, template)| template.clone())
            })
    }

    pub(super) fn resolve_stable_request_template(
        &self,
        stable_template_key: &StableTemplateKey,
    ) -> Option<TemplateKey> {
        self.templates_by_stable_key
            .get(stable_template_key)
            .cloned()
    }

    pub(super) fn instance_key_from_stable_instance_key(
        &self,
        stable_instance_key: &StableInstanceKey,
    ) -> MaterializeResult<Option<InstanceKey>> {
        let Some(template) = self.resolve_stable_request_template(stable_instance_key.template())
        else {
            return Ok(None);
        };
        let type_args = stable_instance_key
            .canonical_type_args()
            .iter()
            .map(|canonical| {
                find_canonical_type_in_store(&self.types, canonical).ok_or_else(|| {
                    frontend_err(format!(
                        "无法在 materializer type store 中定位 stable type argument `{canonical}`"
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
                        find_canonical_type_in_store(&self.types, type_key.as_str())
                    })
                    .map_err(|err| {
                        frontend_err(format!(
                            "无法在 materializer type store 中定位 stable effect row `{template}`: {err}"
                        ))
                    })
            })
            .collect::<MaterializeResult<Vec<_>>>()?;
        Ok(Some(InstanceKey {
            template,
            type_args,
            eff_args,
        }))
    }

    pub(super) fn localize_stable_request_args(
        &mut self,
        typecheck_types: &TypeStore,
        key: &crate::monomorph::MonomorphKey,
        stable_instance_key: &StableInstanceKey,
    ) -> MaterializeResult<(Vec<TypeId>, Vec<EffectRow>)> {
        if key.type_args.len() != stable_instance_key.canonical_type_args().len()
            || key.eff_args.len() != stable_instance_key.effect_arg_templates().len()
        {
            return Err(frontend_err(format!(
                "monomorph request `{}` 的 stable argument arity 与 source payload 不一致",
                key.symbol.fqn
            )));
        }

        let type_args = key
            .type_args
            .iter()
            .copied()
            .zip(stable_instance_key.canonical_type_args())
            .map(|(source_ty, canonical)| {
                self.localize_stable_type_arg(typecheck_types, source_ty, canonical)
            })
            .collect::<MaterializeResult<Vec<_>>>()?;
        let eff_args = key
            .eff_args
            .iter()
            .zip(stable_instance_key.effect_arg_templates())
            .map(|(source_row, template)| {
                self.localize_stable_effect_arg(typecheck_types, source_row, template)
            })
            .collect::<MaterializeResult<Vec<_>>>()?;
        Ok((type_args, eff_args))
    }

    fn localize_stable_type_arg(
        &mut self,
        typecheck_types: &TypeStore,
        source_ty: TypeId,
        canonical: &str,
    ) -> MaterializeResult<TypeId> {
        if let Some(ty) = find_canonical_type_in_store(&self.types, canonical) {
            return Ok(ty);
        }
        let ty = self.types.re_intern_from(typecheck_types, source_ty);
        let actual = canonical_type_text(&self.types, ty, &NoTypeParamResolver).map_err(|err| {
            frontend_err(format!(
                "无法验证 stable type argument `{canonical}` 的本地类型: {err}"
            ))
        })?;
        if actual != canonical {
            return Err(frontend_err(format!(
                "stable type argument mismatch: expected `{canonical}`, got `{actual}`"
            )));
        }
        Ok(ty)
    }

    fn localize_stable_effect_arg(
        &mut self,
        typecheck_types: &TypeStore,
        source_row: &EffectRow,
        template: &EffectRowTemplate,
    ) -> MaterializeResult<EffectRow> {
        template
            .to_effect_row_with(|type_key| {
                self.localize_stable_effect_term(typecheck_types, source_row, type_key)
            })
            .map_err(|err| {
                frontend_err(format!("无法本地化 stable effect row `{template}`: {err}"))
            })
    }

    fn localize_stable_effect_term(
        &mut self,
        typecheck_types: &TypeStore,
        source_row: &EffectRow,
        type_key: &CanonicalTextKey,
    ) -> Option<TypeId> {
        if let Some(ty) = find_canonical_type_in_store(&self.types, type_key.as_str()) {
            return Some(ty);
        }
        let source_ty = source_row.terms.iter().copied().find(|&ty| {
            canonical_type_text(typecheck_types, ty, &NoTypeParamResolver)
                .is_ok_and(|text| text == type_key.as_str())
        })?;
        let ty = self.types.re_intern_from(typecheck_types, source_ty);
        let actual = canonical_type_text(&self.types, ty, &NoTypeParamResolver).ok()?;
        (actual == type_key.as_str()).then_some(ty)
    }

    pub(super) fn seed_requests(
        &mut self,
        typecheck_types: &TypeStore,
        monomorph_requests: &[MonomorphRequest],
    ) -> MaterializeResult<Vec<InstanceKey>> {
        let request_root_instances = self.seed_request_root_direct_call_instances()?;
        let mut initial = Vec::new();
        for request in monomorph_requests {
            if !self.request_sources.contains(&request.request_source_path) {
                continue;
            }
            if !self.monomorph_request_is_reachable_initial_seed(request) {
                continue;
            }
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
            let Some(template) = self
                .resolve_stable_request_template(stable_template_key)
                .or_else(|| {
                    self.resolve_request_template_by_decl_site(
                        &key.symbol.fqn,
                        &key.symbol.decl_file,
                        key.symbol.decl_span,
                    )
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

            if self
                .roots
                .get(&template)
                .is_some_and(|root| root.eff_param_name.is_some())
                && key.eff_args.is_empty()
            {
                continue;
            }
            let Some(expected_stable_instance_key) = key.stable_instance_key.as_ref() else {
                return Err(frontend_err(format!(
                    "monomorph request `{}` 缺少 stable instance key",
                    key.symbol.fqn
                )));
            };
            let (type_args, eff_args) = self.localize_stable_request_args(
                typecheck_types,
                key,
                expected_stable_instance_key,
            )?;
            if !instance_request_is_concrete(&self.types, &type_args, &eff_args) {
                continue;
            }
            let actual_stable_instance_key = StableInstanceKey::from_type_arguments(
                stable_template_key.clone(),
                &self.types,
                &type_args,
                &eff_args,
                &NoTypeParamResolver,
            )
            .map_err(|err| {
                frontend_err(format!(
                    "无法验证 monomorph request `{}` 的 stable instance key: {err}",
                    key.symbol.fqn
                ))
            })?;
            if &actual_stable_instance_key != expected_stable_instance_key {
                return Err(frontend_err(format!(
                    "monomorph request `{}` 的 stable instance key 与本地化参数不一致",
                    key.symbol.fqn
                )));
            }
            initial.push(InstanceKey {
                template,
                type_args,
                eff_args,
            });
        }
        initial.extend(request_root_instances);
        initial.sort_by_key(|a| {
            (
                self.instance_display_fqn(a),
                self.instance_exported_fun_symbol(a),
            )
        });
        initial.dedup();
        Ok(initial)
    }

    pub(super) fn monomorph_request_is_reachable_initial_seed(
        &self,
        request: &MonomorphRequest,
    ) -> bool {
        if !self.filter_initial_requests_to_reachable_call_sites {
            return true;
        }
        let site = (request.request_source_path.clone(), request.call_span);
        self.reachable_request_call_sites.contains(&site)
            || self
                .reachable_request_stmt_spans
                .iter()
                .any(|(source_path, stmt_span)| {
                    source_path == &request.request_source_path
                        && request.call_span.start >= stmt_span.start
                        && request.call_span.end <= stmt_span.end
                })
    }

    pub(super) fn seed_request_root_direct_call_instances(
        &mut self,
    ) -> MaterializeResult<Vec<InstanceKey>> {
        if self.request_root_funs.is_empty() {
            return Ok(Vec::new());
        }
        let request_root_funs = self.request_root_funs.clone();
        let mut out = Vec::new();

        for request_root in request_root_funs {
            self.scan_reachable_non_generic_fun(&request_root, &mut out)?;
        }

        Ok(out)
    }
}

pub(super) fn find_canonical_type_in_store(types: &TypeStore, canonical: &str) -> Option<TypeId> {
    types.iter_ids().find(|&ty| {
        !type_contains_param(types, ty)
            && canonical_type_text(types, ty, &NoTypeParamResolver)
                .is_ok_and(|text| text == canonical)
    })
}
