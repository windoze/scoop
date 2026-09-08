use super::*;

mod native;
mod resolution;

enum BoundReferenceLayer<'a> {
    Members(&'a [crate::CallableCandidate]),
    Extensions(&'a [hir::FunctionId]),
}

enum ReferenceResolutionOutcome {
    Resolved(ResolvedReference),
    NoApplicable,
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
        let candidate_layers = self.named_reference_candidate_layers(&name.text);
        if candidate_layers.is_empty() && first_failure.is_none() {
            if self.lexical_nested_nominal_target(&name.text).is_none()
                && self.source_type_alias_named(&name.text).is_some()
            {
                self.resolve_type_alias_reference(name, false)?;
                self.error(
                    span,
                    format!(
                        "constructor reference `::{}` is not supported; construct the value in a lambda",
                        name.text
                    ),
                );
                return None;
            }
            if self.is_declared_type_name(&name.text) {
                self.error(
                    span,
                    format!(
                        "constructor reference `::{}` is not supported; construct the value in a lambda",
                        name.text
                    ),
                );
                return None;
            }
            self.error(name.span, format!("unknown function `{}`", name.text));
            return None;
        }
        let expected_signature = self.expected_function_signature(expected);
        let display = format!("callable reference `::{}`", name.text);
        for layer in candidate_layers {
            let mut state = self.clone();
            match state.resolve_reference_candidates(
                &layer.candidates,
                &[],
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
                    first_failure.get_or_insert(Box::new(state));
                }
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
        let callee = resolved.callable;
        let ty = resolved.ty;
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let id = self.callable_references.alloc(hir::CallableReference {
            target: hir::CallableReferenceTarget::Named(callee),
            function_type,
            owner_type_param_count: self.type_params_in_scope.len(),
            captures: Vec::new(),
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(super) fn lower_bound_callable_reference(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let direct_alias = match self.resolve_direct_alias_qualifier(receiver) {
            Ok(alias) => alias,
            Err(()) => return None,
        };
        if let Some((alias, _)) = direct_alias {
            self.error(
                span,
                format!(
                    "unbound member reference `{}::{}` is not supported; bind an expression receiver first",
                    alias.name.text, name.text
                ),
            );
            return None;
        }
        if let ast::Expr::Var(type_name) = receiver
            && self.scopes.lookup(&type_name.text).is_none()
            && !self.host_has_property(&type_name.text)
            && (self.is_declared_type_name(&type_name.text)
                || self
                    .type_params_in_scope
                    .iter()
                    .any(|param| param.name == type_name.text))
        {
            self.error(
                span,
                format!(
                    "unbound member reference `{}::{}` is not supported; bind an expression receiver first",
                    type_name.text, name.text
                ),
            );
            return None;
        }
        // The source expression is retained on the reference entity and becomes
        // the first closure field initializer in MIR. It is therefore evaluated
        // once at reference creation, including when it reads a mutable local.
        let receiver = self.lower_expr(receiver, sink, None)?;
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
        for layer in self.named_extension_call_layers(&name.text) {
            if layer.candidates.is_empty() {
                continue;
            }
            let mut state = self.clone();
            match state.finish_bound_callable_reference(
                BoundReferenceLayer::Extensions(&layer.candidates),
                receiver.clone(),
                expected_signature.as_ref(),
                &name.text,
                &display,
                span,
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
                Ok(None) => {
                    first_failure.get_or_insert(Box::new(state));
                }
                Err(()) => {
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
    ) -> Result<Option<hir::Expr>, ()> {
        let is_extension = matches!(layer, BoundReferenceLayer::Extensions(_));
        let outcome = match layer {
            BoundReferenceLayer::Members(candidates) => self.resolve_member_reference_candidates(
                candidates,
                ReferenceResolutionContext {
                    expected,
                    name,
                    display,
                    span,
                    extension_mode: ReferenceExtensionMode::Exclude,
                },
            ),
            BoundReferenceLayer::Extensions(candidates) => self.resolve_reference_candidates(
                candidates,
                &[],
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
            ReferenceResolutionOutcome::NoApplicable => return Ok(None),
            ReferenceResolutionOutcome::Failed => return Err(()),
        };
        let callee = resolved.callable;
        let ty = resolved.ty;
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let target = if is_extension {
            hir::CallableReferenceTarget::BoundExtension {
                receiver: Box::new(receiver),
                callee,
            }
        } else {
            hir::CallableReferenceTarget::BoundMember {
                receiver: Box::new(receiver),
                callee: self.materialize_method_callee(
                    resolved.source,
                    callee,
                    &resolved.type_args,
                ),
            }
        };
        let id = self.callable_references.alloc(hir::CallableReference {
            target,
            function_type,
            owner_type_param_count: self.type_params_in_scope.len(),
            captures: Vec::new(),
            span,
        });
        Ok(Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
            origin: self.expression_origin(span),
        }))
    }

    pub(super) fn lower_local_callable_reference(
        &mut self,
        candidates: Vec<hir::LocalFunctionId>,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let expected_signature = self.expected_function_signature(expected);
        let owner_type_param_count =
            self.signatures[&self.local_functions[candidates[0]].function].owner_type_param_count;
        let owner_type_args = self.ambient_type_args(owner_type_param_count);
        let functions = candidates
            .iter()
            .map(|candidate| self.local_functions[*candidate].function)
            .collect::<Vec<_>>();
        let display = format!("local callable reference `::{}`", name.text);
        let resolved = match self.resolve_reference_candidates(
            &functions,
            &owner_type_args,
            ReferenceResolutionContext {
                expected: expected_signature.as_ref(),
                name: &name.text,
                display: &display,
                span,
                extension_mode: ReferenceExtensionMode::Exclude,
            },
        ) {
            ReferenceResolutionOutcome::Resolved(resolved) => resolved,
            ReferenceResolutionOutcome::NoApplicable => return Ok(None),
            ReferenceResolutionOutcome::Failed => return Err(()),
        };
        let function = self.callable_function_id(resolved.callable);
        let local_function = self.local_function_by_function[&function];
        let ty = resolved.ty;
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let Some(capture_sources) = self.local_call_capture_args(local_function, span) else {
            return Err(());
        };
        let capture_specs: Vec<_> = self.local_functions[local_function]
            .captures
            .iter()
            .map(|capture| {
                (
                    capture.binding,
                    capture.name.clone(),
                    capture.ty,
                    capture.first_use_span,
                )
            })
            .collect();
        let captures = capture_specs
            .into_iter()
            .zip(capture_sources)
            .map(
                |((binding, name, ty, first_use_span), source)| hir::Capture {
                    binding,
                    name,
                    ty: self.instantiate_ty(ty, &resolved.type_args),
                    first_use_span,
                    source,
                },
            )
            .collect();
        let id = self.callable_references.alloc(hir::CallableReference {
            target: hir::CallableReferenceTarget::Local {
                local_function,
                callee: resolved.callable,
            },
            function_type,
            owner_type_param_count: self.type_params_in_scope.len(),
            captures,
            span,
        });
        Ok(Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
            origin: self.expression_origin(span),
        }))
    }
}
