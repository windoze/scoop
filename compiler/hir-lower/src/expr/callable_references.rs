use super::*;

mod bound;
mod constructors;
mod local;
mod native;
mod resolution;
mod unbound;
use resolution::ReferenceCandidate;

pub(super) enum ReferenceTarget {
    Callable(hir::CallableReferenceTarget),
    Value(hir::Expr),
}

enum BoundReferenceLayer<'a> {
    Members(&'a [ReferenceCandidate]),
    Extensions(&'a [ReferenceCandidate]),
}

enum ReferenceResolutionOutcome {
    Resolved(ResolvedReference),
    NoApplicable,
    Blocked,
    Failed,
}

enum BoundReferenceOutcome {
    Resolved(hir::Expr),
    NoApplicable,
    Blocked,
    Failed,
}

impl Lowerer {
    pub(super) fn lower_callable_reference(
        &mut self,
        receiver: Option<&ast::Expr>,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if let Some(expected) = expected
            && let Type::FunPtr(_) = self.types[expected]
        {
            if receiver.is_some() {
                self.error(
                    span,
                    "a native `FunPtr` address must reference an unbound top-level function"
                        .to_string(),
                );
                return None;
            }
            return self.lower_native_function_reference(name, span, expected);
        }
        if let Some(receiver) = receiver {
            return self.lower_bound_callable_reference(receiver, name, span, expected, sink);
        }
        if let Some(local) = self.scopes.lookup(&name.text)
            && matches!(self.types[self.locals[local].ty], Type::Function(_))
        {
            self.error(
                span,
                format!(
                    "`::{}` cannot reference an existing function value; use `{}` directly",
                    name.text, name.text
                ),
            );
            return None;
        }
        let local_candidates = self.local_function_scopes.lookup(&name.text);
        let mut first_failure = None;
        if !local_candidates.is_empty() {
            let mut state = self.clone();
            match state.lower_local_callable_reference(
                local_candidates.clone(),
                name,
                span,
                expected,
            ) {
                Ok(Some(expression)) => {
                    return Some(self.commit_expr_layer(
                        SuccessfulExprLayer {
                            state: Box::new(state),
                            expression,
                            sink: Vec::new(),
                        },
                        sink,
                    ));
                }
                Ok(None) => first_failure = Some(Box::new(state)),
                Err(()) => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        if let Some(nominal) = self.lexical_nested_nominal_target(&name.text) {
            let mut state = self.clone();
            match state.lower_lexical_constructor_reference(nominal, name, span, expected) {
                Ok(Some(expression)) => {
                    *self = state;
                    return Some(expression);
                }
                Ok(None) => {
                    first_failure.get_or_insert(Box::new(state));
                }
                Err(()) => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        let candidate_layers = self
            .named_callable_reference_layers(&name.text)
            .into_iter()
            .filter(|layer| !layer.candidates.is_empty() || !layer.suppressed_callables.is_empty())
            .collect::<Vec<_>>();
        if candidate_layers.is_empty() && first_failure.is_none() {
            self.error(name.span, format!("unknown function `{}`", name.text));
            return None;
        }
        let expected_signature = self.expected_function_signature(expected);
        let display = format!("callable reference `::{}`", name.text);
        for layer in candidate_layers {
            if layer.candidates.is_empty() && !layer.suppressed_callables.is_empty() {
                if let Some(failure) = first_failure {
                    self.commit_layer_diagnostics(*failure);
                }
                return None;
            }
            let mut state = self.clone();
            let candidates = match state.named_reference_candidates(layer.candidates, name, span) {
                Ok(candidates) => candidates,
                Err(()) => {
                    first_failure.get_or_insert(Box::new(state));
                    continue;
                }
            };
            match state.resolve_reference_candidate_set(
                &candidates,
                None,
                ReferenceResolutionContext {
                    expected: expected_signature.as_ref(),
                    name: &name.text,
                    display: &display,
                    span,
                    extension_mode: ReferenceExtensionMode::IncludeUnbound,
                },
            ) {
                ReferenceResolutionOutcome::Resolved(resolved) => {
                    let expression = state.finish_named_callable_reference(resolved, span)?;
                    return Some(self.commit_expr_layer(
                        SuccessfulExprLayer {
                            state: Box::new(state),
                            expression,
                            sink: Vec::new(),
                        },
                        sink,
                    ));
                }
                ReferenceResolutionOutcome::NoApplicable => {
                    if !layer.suppressed_callables.is_empty() {
                        self.commit_layer_diagnostics(state);
                        return None;
                    }
                    first_failure.get_or_insert(Box::new(state));
                }
                ReferenceResolutionOutcome::Blocked => return None,
                ReferenceResolutionOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        self.commit_layer_diagnostics(*first_failure.expect("at least one reference layer failed"));
        None
    }

    fn finish_named_callable_reference(
        &mut self,
        resolved: ResolvedReference,
        span: Span,
    ) -> Option<hir::Expr> {
        let target = match resolved.target {
            ReferenceTarget::Callable(target) => target,
            ReferenceTarget::Value(value) => return Some(value),
        };
        let ty = resolved.ty;
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let definition_path = self
            .definition_paths
            .next(scoop_identity::StructuralDefinitionSiteRole::CallableConversion);
        let owner_type_arguments = self.ambient_type_args(self.type_params_in_scope.len());
        let id = self.callable_references.alloc(hir::CallableReference {
            definition_root: hir::CallableReferenceRoot::Source(self.current_definition_root()),
            definition_path,
            target,
            function_type,
            owner_type_arguments,
            captures: Vec::new(),
            origin: self.definition_origin(span),
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
