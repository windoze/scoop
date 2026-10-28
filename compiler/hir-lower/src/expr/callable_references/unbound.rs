use super::*;

mod adapter;
mod qualifier;

impl Lowerer {
    pub(super) fn lower_unbound_callable_reference(
        &mut self,
        receiver: TypeId,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let members = self.methods_by_name(receiver, &name.text);
        let imported = match self.imported_member_call_candidates(
            receiver,
            name,
            RequiredCallableModifiers::default(),
            MemberCallKind::Ordinary,
        ) {
            Ok(candidates) => candidates,
            Err(failure) => {
                self.commit_layer_diagnostics(&failure);
                return None;
            }
        };
        let mut layers = Vec::new();
        let members =
            members
                .into_iter()
                .map(|candidate| ReferenceCandidate::Local(candidate, None))
                .chain(imported.into_iter().map(|candidate| {
                    ReferenceCandidate::UnboundMember(Box::new(candidate), receiver)
                }))
                .collect::<Vec<_>>();
        if !members.is_empty() {
            layers.push((
                members,
                ReferenceExtensionMode::UnboundMember(receiver),
                false,
                Vec::new(),
                None,
            ));
        }
        for layer in self.named_executable_extension_call_layers(&name.text) {
            if !layer.candidates.is_empty()
                || !layer.suppressed_callables.is_empty()
                || !layer.unavailable.is_empty()
            {
                layers.push((
                    layer
                        .candidates
                        .into_iter()
                        .map(ReferenceCandidate::extension)
                        .collect(),
                    ReferenceExtensionMode::UnboundExtension(receiver),
                    !layer.suppressed_callables.is_empty(),
                    layer.unavailable,
                    Some(layer.kind),
                ));
            }
        }
        let expected_signature = self.expected_function_signature(expected);
        let display = format!(
            "unbound callable reference `{}::{}`",
            self.type_name(receiver),
            name.text
        );
        let mut first_failure = None;
        for (candidates, mode, suppressed, unavailable, import_layer) in layers {
            if self.report_unavailable_lookup(name, &unavailable) {
                return None;
            }
            if candidates.is_empty() && suppressed {
                return None;
            }
            let mut state = self.clone();
            match state.resolve_reference_candidate_set(
                &candidates,
                None,
                ReferenceResolutionContext {
                    expected: expected_signature.as_ref(),
                    name: &name.text,
                    display: &display,
                    span,
                    extension_mode: mode,
                },
            ) {
                ReferenceResolutionOutcome::Resolved(resolved) => {
                    let expression = state.finish_named_callable_reference(resolved, span)?;
                    if let Some(layer) = import_layer {
                        state.observe_import_lookup(name, layer);
                    }
                    *self = state;
                    return Some(expression);
                }
                ReferenceResolutionOutcome::NoApplicable => {
                    if suppressed {
                        self.commit_layer_diagnostics(&state);
                        return None;
                    }
                    first_failure.get_or_insert(state);
                }
                ReferenceResolutionOutcome::Blocked => return None,
                ReferenceResolutionOutcome::Failed => {
                    self.commit_layer_diagnostics(&state);
                    return None;
                }
            }
        }
        if self.has_required_member(receiver, &name.text) {
            self.reject_requirement_reference(span);
            return None;
        }
        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(&failure);
        } else {
            self.error(
                name.span,
                format!(
                    "type `{}` has no method `{}`",
                    self.type_name(receiver),
                    name.text
                ),
            );
        }
        None
    }
}
