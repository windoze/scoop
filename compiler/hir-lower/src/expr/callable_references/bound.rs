use super::*;

impl Lowerer {
    pub(super) fn lower_bound_callable_reference(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // The source expression is retained on the reference entity and becomes
        // the first closure field initializer in MIR. It is therefore evaluated
        // once at reference creation, including when it reads a mutable local.
        let receiver = if let Some(ty) = self.reference_type_qualifier(receiver, expected).ok()? {
            if let Some(crate::NominalTarget::Object(object)) = self.nominal_target_for_type(ty) {
                self.lower_qualified_singleton(object, Some(ty), receiver.span())?
            } else if self
                .imported_nominal_owner(ty)
                .and_then(|owner| self.dependencies.as_ref()?.nominal_declaration(owner))
                .is_some_and(|declaration| {
                    matches!(
                        declaration.interface.source_shape(),
                        hir::NominalSourceShapeV1::Object(_)
                    )
                })
            {
                self.lower_imported_singleton_type(ty, receiver.span())?
            } else {
                return self.lower_unbound_callable_reference(ty, name, span, expected);
            }
        } else {
            self.lower_expr(receiver, sink, None)?
        };
        let mut member_candidates = self.methods_by_name(receiver.ty, &name.text);
        let mut first_failure = None;
        if !member_candidates.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = member_candidates.len();
            member_candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if member_candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be referenced through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        let imported_candidates = match self.imported_member_call_candidates(
            receiver.ty,
            name,
            RequiredCallableModifiers::default(),
            MemberCallKind::Ordinary,
        ) {
            Ok(candidates) => candidates,
            Err(failure) => {
                self.commit_layer_diagnostics(*failure);
                return None;
            }
        };
        let views = self.smart_cast_receiver_views(&receiver);
        let mut member_candidates = member_candidates
            .into_iter()
            .map(|candidate| (candidate, receiver.clone()))
            .collect::<Vec<_>>();
        let mut imported_candidates = imported_candidates
            .into_iter()
            .map(|candidate| (candidate, receiver.clone()))
            .collect::<Vec<_>>();
        if let Err(failure) = self.add_smart_cast_member_views(
            &mut member_candidates,
            &mut imported_candidates,
            &views[1..],
            name,
            RequiredCallableModifiers::default(),
        ) {
            self.commit_layer_diagnostics(*failure);
            return None;
        }
        let mut member_candidates =
            member_candidates
                .into_iter()
                .map(|(candidate, view)| ReferenceCandidate::Local(candidate, Some(view)))
                .chain(imported_candidates.into_iter().map(|(declaration, view)| {
                    ReferenceCandidate::Member(Box::new(declaration), view)
                }))
                .collect::<Vec<_>>();
        member_candidates.extend(
            self.bound_inner_reference_candidates(receiver.clone(), name, span, expected)
                .ok()?,
        );
        let expected_signature = self.expected_function_signature(expected);
        let display = format!("bound callable reference `receiver::{}`", name.text);
        if !member_candidates.is_empty() {
            let mut state = self.clone();
            match state.finish_bound_callable_reference(
                BoundReferenceLayer::Members(&member_candidates),
                receiver.clone(),
                expected_signature.as_ref(),
                &name.text,
                &display,
                span,
            ) {
                BoundReferenceOutcome::Resolved(expression) => {
                    return Some(self.commit_expr_layer(
                        SuccessfulExprLayer {
                            state: Box::new(state),
                            expression,
                            sink: Vec::new(),
                        },
                        sink,
                    ));
                }
                BoundReferenceOutcome::NoApplicable => first_failure = Some(Box::new(state)),
                BoundReferenceOutcome::Blocked => return None,
                BoundReferenceOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        for layer in self.named_executable_extension_call_layers(&name.text) {
            if layer.candidates.is_empty() {
                if !layer.suppressed_callables.is_empty() {
                    if let Some(failure) = first_failure {
                        self.commit_layer_diagnostics(*failure);
                    }
                    return None;
                }
                continue;
            }
            let mut state = self.clone();
            let candidates = layer
                .candidates
                .into_iter()
                .map(ReferenceCandidate::extension)
                .collect::<Vec<_>>();
            match state.finish_bound_callable_reference(
                BoundReferenceLayer::Extensions(&candidates),
                receiver.clone(),
                expected_signature.as_ref(),
                &name.text,
                &display,
                span,
            ) {
                BoundReferenceOutcome::Resolved(expression) => {
                    return Some(self.commit_expr_layer(
                        SuccessfulExprLayer {
                            state: Box::new(state),
                            expression,
                            sink: Vec::new(),
                        },
                        sink,
                    ));
                }
                BoundReferenceOutcome::NoApplicable => {
                    if !layer.suppressed_callables.is_empty() {
                        self.commit_layer_diagnostics(state);
                        return None;
                    }
                    first_failure.get_or_insert(Box::new(state));
                }
                BoundReferenceOutcome::Blocked => return None,
                BoundReferenceOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else {
            let found = self.type_name(receiver.ty);
            self.error(
                name.span,
                format!("type `{found}` has no method `{}`", name.text),
            );
        }
        None
    }

    fn finish_bound_callable_reference(
        &mut self,
        layer: BoundReferenceLayer<'_>,
        receiver: hir::Expr,
        expected: Option<&(TypeId, hir::FunctionType)>,
        name: &str,
        display: &str,
        span: Span,
    ) -> BoundReferenceOutcome {
        let outcome = match layer {
            BoundReferenceLayer::Members(candidates) => self.resolve_reference_candidate_set(
                candidates,
                Some(&receiver),
                ReferenceResolutionContext {
                    expected,
                    name,
                    display,
                    span,
                    extension_mode: ReferenceExtensionMode::Exclude,
                },
            ),
            BoundReferenceLayer::Extensions(candidates) => self.resolve_reference_candidate_set(
                candidates,
                Some(&receiver),
                ReferenceResolutionContext {
                    expected,
                    name,
                    display,
                    span,
                    extension_mode: ReferenceExtensionMode::Bound(receiver.ty),
                },
            ),
        };
        let resolved = match outcome {
            ReferenceResolutionOutcome::Resolved(resolved) => resolved,
            ReferenceResolutionOutcome::NoApplicable => {
                return BoundReferenceOutcome::NoApplicable;
            }
            ReferenceResolutionOutcome::Blocked => return BoundReferenceOutcome::Blocked,
            ReferenceResolutionOutcome::Failed => return BoundReferenceOutcome::Failed,
        };
        let target = match resolved.target {
            ReferenceTarget::Callable(target) => target,
            ReferenceTarget::Value(value) => return BoundReferenceOutcome::Resolved(value),
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
        BoundReferenceOutcome::Resolved(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
