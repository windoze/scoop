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
            .or_else(|| {
                let matches = self
                    .request_templates
                    .iter()
                    .filter(|((candidate_fqn, candidate_file, _), _)| {
                        candidate_fqn == fqn && candidate_file == decl_file
                    })
                    .map(|(_, template)| template.clone())
                    .collect::<HashSet<_>>();
                (matches.len() == 1).then(|| matches.into_iter().next().unwrap())
            })
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

            if key.type_args.is_empty() && key.eff_args.is_empty() {
                continue;
            }
            let type_args = key
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect::<Vec<_>>();
            let eff_args = key
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect::<Vec<_>>();
            if !instance_request_is_concrete(&self.types, &type_args, &eff_args) {
                continue;
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
