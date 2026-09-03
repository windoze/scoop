use super::*;

mod resolution;

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
        if !local_candidates.is_empty() {
            return self.lower_local_callable_reference(local_candidates, name, span, expected);
        }
        let candidates = self.named_reference_candidate_layer(&name.text);
        if candidates.is_empty() {
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
        let resolved = self.resolve_reference_candidates(
            &candidates,
            &[],
            expected_signature.as_ref(),
            &display,
            span,
            ReferenceExtensionMode::IncludeUnbound,
        )?;
        let callee = resolved.callable;
        let ty = resolved.ty;
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
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
        let candidates = self.named_reference_candidate_layer(&name.text);
        let mut matching = Vec::new();
        for function in candidates {
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
            let params_match = sig
                .params
                .iter()
                .zip(&expected_signature.parameter_types)
                .all(|(parameter, expected)| self.types_equal(parameter.ty, *expected));
            if params_match && self.types_equal(sig.return_ty, expected_signature.return_type) {
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
        let is_extension = member_candidates.is_empty();
        let extension_candidates = if is_extension {
            let candidates = self.extension_candidate_layer(&name.text);
            if candidates.is_empty() {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!("type `{found}` has no method `{}`", name.text),
                );
                return None;
            }
            candidates
        } else {
            Vec::new()
        };
        if !is_extension && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = member_candidates.len();
            member_candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if member_candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be referenced through interface type `{found}`",
                        name.text
                    ),
                );
                return None;
            }
        }
        let expected_signature = self.expected_function_signature(expected);
        let display = format!("bound callable reference `receiver::{}`", name.text);
        let resolved = if is_extension {
            self.resolve_reference_candidates(
                &extension_candidates,
                &[],
                expected_signature.as_ref(),
                &display,
                span,
                ReferenceExtensionMode::Bound(receiver.ty),
            )?
        } else {
            self.resolve_member_reference_candidates(
                &member_candidates,
                expected_signature.as_ref(),
                &display,
                span,
            )?
        };
        let callee = resolved.callable;
        let ty = resolved.ty;
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
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
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        });
        let mut applicable = Vec::new();
        for local_function in candidates {
            let function = self.local_functions[local_function].function;
            let sig = self.signatures[&function].clone();
            let mut bindings = vec![None; sig.type_params.len()];
            for (binding, ty) in bindings
                .iter_mut()
                .zip(self.ambient_type_args(sig.owner_type_param_count))
            {
                *binding = Some(ty);
            }
            match &expected_signature {
                Some((_, expected)) => {
                    if sig.is_suspend != expected.is_suspend
                        || sig.params.len() != expected.parameter_types.len()
                    {
                        continue;
                    }
                    let mut matches = true;
                    for (parameter, expected) in sig.params.iter().zip(&expected.parameter_types) {
                        matches &= self.try_bind(parameter.ty, *expected, &mut bindings);
                    }
                    matches &= self.try_bind(sig.return_ty, expected.return_type, &mut bindings);
                    if !matches || bindings.iter().any(Option::is_none) {
                        continue;
                    }
                    let type_args: Vec<_> = bindings.into_iter().flatten().collect();
                    if !self.type_arguments_satisfy_kinds(&sig.type_params, &type_args) {
                        continue;
                    }
                    let instantiated_params: Vec<_> = sig
                        .params
                        .iter()
                        .map(|parameter| self.instantiate_ty(parameter.ty, &type_args))
                        .collect();
                    let instantiated_return = self.instantiate_ty(sig.return_ty, &type_args);
                    let exact = instantiated_params
                        .iter()
                        .zip(&expected.parameter_types)
                        .all(|(parameter, expected)| self.types_equal(*parameter, *expected))
                        && self.types_equal(instantiated_return, expected.return_type);
                    if exact {
                        let own_type_param_count =
                            sig.type_params.len() - sig.owner_type_param_count;
                        applicable.push((local_function, type_args, own_type_param_count));
                    }
                }
                None => {
                    if sig.type_params.len() != sig.owner_type_param_count {
                        continue;
                    }
                    applicable.push((local_function, bindings.into_iter().flatten().collect(), 0));
                }
            }
        }
        let selected = match applicable.len() {
            0 => {
                self.error(
                    span,
                    format!(
                        "no local overload of `::{}` matches the expected function type",
                        name.text
                    ),
                );
                return None;
            }
            1 => 0,
            _ if expected_signature.is_some() => {
                let concrete: Vec<_> = applicable
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| (candidate.2 == 0).then_some(index))
                    .collect();
                if let [index] = concrete.as_slice() {
                    *index
                } else {
                    self.error(
                        span,
                        format!(
                            "local callable reference `::{}` is ambiguous for the expected function type",
                            name.text
                        ),
                    );
                    return None;
                }
            }
            _ => {
                self.error(
                    span,
                    format!(
                        "local callable reference `::{}` is ambiguous; provide an expected function type",
                        name.text
                    ),
                );
                return None;
            }
        };
        let (local_function, type_args, _) = applicable.swap_remove(selected);
        let local = &self.local_functions[local_function];
        let function = local.function;
        let ty = expected_signature.map_or_else(
            || {
                let signature = self.signatures[&function].clone();
                let parameters = signature
                    .params
                    .iter()
                    .map(|parameter| self.instantiate_ty(parameter.ty, &type_args))
                    .collect();
                let return_ty = self.instantiate_ty(signature.return_ty, &type_args);
                self.intern_function_type(signature.is_suspend, parameters, return_ty)
            },
            |(ty, _)| ty,
        );
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
                    ty: self.instantiate_ty(ty, &type_args),
                    first_use_span,
                    source,
                },
            )
            .collect();
        let callee = if type_args.is_empty() {
            hir::Callable::Function(function)
        } else {
            hir::Callable::Generic(self.record_instantiation(function, type_args))
        };
        let id = self.callable_references.alloc(hir::CallableReference {
            target: hir::CallableReferenceTarget::Local {
                local_function,
                callee,
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
