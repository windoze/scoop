use super::*;

use crate::call_resolution::constraints::CallableCategory;

mod resolution;

enum BoundReferenceLayer<'a> {
    Members(&'a [crate::CallableCandidate]),
    Extensions(&'a [hir::FunctionId]),
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
            && let Type::FunPtr(signature) = self.types[expected]
        {
            if receiver.is_some() {
                self.error(
                    span,
                    "a native `FunPtr` address must reference an unbound top-level function"
                        .to_string(),
                );
                return None;
            }
            return self.lower_native_function_reference(name, span, expected, signature);
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
            match self.probe_expr_layer(|state, _| {
                state.lower_local_callable_reference(local_candidates.clone(), name, span, expected)
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => first_failure = Some(failure),
            }
        }
        let candidate_layers = self.named_reference_candidate_layers(&name.text);
        if candidate_layers.is_empty() && first_failure.is_none() {
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
        for candidates in candidate_layers {
            match self.probe_expr_layer(|state, _| {
                let resolved = state.resolve_reference_candidates(
                    &candidates,
                    &[],
                    expected_signature.as_ref(),
                    &display,
                    span,
                    ReferenceExtensionMode::IncludeUnbound,
                )?;
                state.finish_named_callable_reference(resolved, span)
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
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
        })
    }

    pub(super) fn lower_native_function_reference(
        &mut self,
        name: &ast::Ident,
        span: Span,
        expected: TypeId,
        signature: hir::FunctionTypeId,
    ) -> Option<hir::Expr> {
        if self.scopes.lookup(&name.text).is_some()
            || !self.local_function_scopes.lookup(&name.text).is_empty()
        {
            self.error(
                span,
                "a native `FunPtr` address cannot target a local function or function value"
                    .to_string(),
            );
            return None;
        }
        let expected_signature = self.function_types[signature].clone();
        let candidate_layers = self.named_reference_candidate_layers(&name.text);
        if candidate_layers.is_empty() {
            return self.lower_native_function_reference_layer(
                name,
                span,
                expected,
                &expected_signature,
                &[],
            );
        }
        let mut first_failure = None;
        for candidates in candidate_layers {
            match self.probe_expr_layer(|state, _| {
                state.lower_native_function_reference_layer(
                    name,
                    span,
                    expected,
                    &expected_signature,
                    &candidates,
                )
            }) {
                Ok(layer) => {
                    let mut sink = Vec::new();
                    return Some(self.commit_expr_layer(layer, &mut sink));
                }
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }
        self.commit_layer_diagnostics(*first_failure.expect("at least one native layer failed"));
        None
    }

    fn lower_native_function_reference_layer(
        &mut self,
        name: &ast::Ident,
        span: Span,
        expected: TypeId,
        expected_signature: &hir::FunctionType,
        candidates: &[hir::FunctionId],
    ) -> Option<hir::Expr> {
        let mut matching = Vec::new();
        for &function in candidates {
            let declaration = &self.functions[function];
            let sig = &self.signatures[&function];
            if declaration.method.is_some()
                || self.extension_receivers.contains_key(&function)
                || !sig.type_params.is_empty()
                || sig.is_suspend
                || declaration.attributes.gc_effect != hir::GcEffect::NoGc
                || !matches!(declaration.kind, hir::FunctionKind::User(_))
                || sig.params.len() != expected_signature.parameter_types.len()
            {
                continue;
            }
            let parameters = sig
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .collect::<Vec<_>>();
            if self.concrete_callable_signature_matches(
                CallableCategory::Native,
                sig.is_suspend,
                &parameters,
                sig.return_ty,
                expected,
            ) {
                matching.push(function);
            }
        }
        let function = match matching.as_slice() {
            [function] => *function,
            [] => {
                self.error(
                    span,
                    format!(
                        "no eligible `@NoGC` top-level function `::{}` exactly matches the expected FunPtr signature",
                        name.text
                    ),
                );
                return None;
            }
            _ => {
                self.error(
                    span,
                    format!(
                        "native function reference `::{}` is ambiguous for the expected FunPtr signature",
                        name.text
                    ),
                );
                return None;
            }
        };
        if self.functions[function].attributes.safety == hir::Safety::Unsafe {
            self.require_unsafe_operation(span, "taking the address of an unsafe callback");
        }
        Some(hir::Expr {
            kind: ExprKind::FunctionAddress(function),
            ty: expected,
            span,
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
            match self.probe_expr_layer(|state, _| {
                state.finish_bound_callable_reference(
                    BoundReferenceLayer::Members(&member_candidates),
                    receiver.clone(),
                    expected_signature.as_ref(),
                    &display,
                    span,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => first_failure = Some(failure),
            }
        }
        for extensions in self.extension_candidate_layers(&name.text) {
            match self.probe_expr_layer(|state, _| {
                state.finish_bound_callable_reference(
                    BoundReferenceLayer::Extensions(&extensions),
                    receiver.clone(),
                    expected_signature.as_ref(),
                    &display,
                    span,
                )
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
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
        display: &str,
        span: Span,
    ) -> Option<hir::Expr> {
        let is_extension = matches!(layer, BoundReferenceLayer::Extensions(_));
        let resolved = match layer {
            BoundReferenceLayer::Members(candidates) => {
                self.resolve_member_reference_candidates(candidates, expected, display, span)?
            }
            BoundReferenceLayer::Extensions(candidates) => self.resolve_reference_candidates(
                candidates,
                &[],
                expected,
                display,
                span,
                ReferenceExtensionMode::Bound(receiver.ty),
            )?,
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
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
        })
    }

    pub(super) fn lower_local_callable_reference(
        &mut self,
        candidates: Vec<hir::LocalFunctionId>,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let expected_signature = self.expected_function_signature(expected);
        let owner_type_param_count =
            self.signatures[&self.local_functions[candidates[0]].function].owner_type_param_count;
        let owner_type_args = self.ambient_type_args(owner_type_param_count);
        let functions = candidates
            .iter()
            .map(|candidate| self.local_functions[*candidate].function)
            .collect::<Vec<_>>();
        let display = format!("local callable reference `::{}`", name.text);
        let resolved = self.resolve_reference_candidates(
            &functions,
            &owner_type_args,
            expected_signature.as_ref(),
            &display,
            span,
            ReferenceExtensionMode::Exclude,
        )?;
        let function = self.callable_function_id(resolved.callable);
        let local_function = self.local_function_by_function[&function];
        let ty = resolved.ty;
        let Type::Function(function_type) = self.types[ty] else {
            unreachable!("a callable reference has a function type")
        };
        let capture_sources = self.local_call_capture_args(local_function, span)?;
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
        Some(hir::Expr {
            kind: ExprKind::CallableReference(id),
            ty,
            span,
        })
    }
}
