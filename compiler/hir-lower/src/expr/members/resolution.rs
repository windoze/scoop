use super::*;

impl Lowerer {
    /// The multi-candidate path of a method call (explicit receiver or
    /// bare `m(...)`): `resolve_overload` picks the winner among the
    /// receiver type's methods and the call becomes a resolved
    /// `MethodCall`.
    pub(in crate::expr) fn finish_overloaded_method_call(
        &mut self,
        candidates: Vec<crate::CallableCandidate>,
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        let resolved = self.resolve_member_overload(
            name,
            &candidates,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
            },
            sink,
        )?;
        let ty = resolved.return_ty;
        self.check_call_effects(resolved.callee, call.span);
        if let Some(expr) = self.normalize_pointer_method_call(
            resolved.callee,
            receiver.clone(),
            resolved.args.clone(),
            ty,
            call.span,
        ) {
            return Some(expr);
        }
        let method_callee =
            self.materialize_method_callee(resolved.source, resolved.callee, &resolved.type_args);
        Some(hir::Expr {
            kind: ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: method_callee,
                args: resolved.args,
            },
            ty,
            span: call.span,
        })
    }

    /// Whether the current host type has a property named `name`
    /// (the quiet probe behind the enum-path fallback in
    /// `lower_method_call`: a bare receiver name that would resolve
    /// to `this.name` is a property access, not an enum path).
    pub(in crate::expr) fn host_has_property(&self, name: &str) -> bool {
        match self.current_this_ty().map(|ty| self.types[ty].clone()) {
            Some(Type::Class(application)) => self
                .find_class_field(self.class_applications[application].template, name)
                .is_some(),
            Some(Type::Struct(application)) => self.structs
                [self.struct_applications[application].template]
                .semantic_fields()
                .iter()
                .any(|field| field.name == name),
            _ => false,
        }
    }

    /// Check and build a resolved method call: arity and argument
    /// types against the method's declared parameters. The receiver binds
    /// the owner prefix and the complete argument group infers the method
    /// suffix; subtype adaptation (boxing) happens afterwards.
    pub(in crate::expr) fn finish_method_call(
        &mut self,
        candidate: crate::CallableCandidate,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = candidate.function;
        let name = self.functions[function].name.clone();
        let owner_type_args = self.callable_candidate_owner_arguments(&candidate);
        let sig = self.signatures[&function].clone();
        if sig.params.len() != call.args.len() {
            let expected = sig.params.len();
            let supplied = call.args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                call.span,
                format!(
                    "method `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }
        debug_assert_eq!(sig.owner_type_param_count, owner_type_args.len());
        let mut bindings = vec![None; sig.type_params.len()];
        for (binding, &ty) in bindings.iter_mut().zip(&owner_type_args) {
            *binding = Some(ty);
        }
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        if !self.bind_explicit_type_args(
            &mut bindings,
            sig.owner_type_param_count,
            &explicit_type_args,
            call.span,
            &format!("method `{name}`"),
        ) {
            return None;
        }
        let param_tys: Vec<_> = sig.params.iter().map(|param| param.ty).collect();
        let inferred =
            self.lower_inference_args(call.args, &param_tys, bindings, &sig.type_params)?;
        let mut type_args = Vec::with_capacity(sig.type_params.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&sig.type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        call.span,
                        format!("cannot infer type argument `{}` for `{name}`", param.name),
                    );
                    return None;
                }
            }
        }
        if !self.check_type_argument_kinds(
            &sig.type_params,
            &type_args,
            call.span,
            &format!("function `{}`", self.functions[function].name),
        ) {
            return None;
        }
        let lowered = inferred.finish(sink);
        let mut adapted = Vec::with_capacity(lowered.len());
        for (param, arg) in sig.params.iter().zip(lowered) {
            let param_ty = self.instantiate_ty(param.ty, &type_args);
            if !self.is_subtype(arg.ty, param_ty) {
                let param_name = param.name.text.clone();
                let expected = self.type_name(param_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for parameter `{param_name}` of `{name}` must be of type {expected}, found {found}"
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, param_ty));
        }
        let callee = self.materialize_candidate_callable(&candidate, &type_args);
        let ty = self.instantiate_ty(sig.return_ty, &type_args);
        self.check_call_effects(callee, call.span);
        if let Some(expr) = self.normalize_pointer_method_call(
            callee,
            receiver.clone(),
            adapted.clone(),
            ty,
            call.span,
        ) {
            return Some(expr);
        }
        let method_callee = self.materialize_method_callee(candidate.source, callee, &type_args);
        Some(hir::Expr {
            kind: ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: method_callee,
                args: adapted,
            },
            ty,
            span: call.span,
        })
    }

    pub(crate) fn materialize_method_callee(
        &mut self,
        source: crate::CallableCandidateSource,
        callable: hir::Callable,
        type_args: &[TypeId],
    ) -> hir::MethodCallee {
        let crate::CallableCandidateSource::Bound {
            receiver_parameter,
            bound,
            member,
        } = source
        else {
            return hir::MethodCallee::Callable(callable);
        };
        let function = self.interface_method_entities[member].function;
        let signature = self.signatures[&function].clone();
        let parameter_types = signature
            .params
            .iter()
            .map(|parameter| self.instantiate_ty(parameter.ty, type_args))
            .collect();
        let return_type = self.instantiate_ty(signature.return_ty, type_args);
        let signature_type =
            self.intern_function_type(signature.is_suspend, parameter_types, return_type);
        let Type::Function(instantiated_signature) = self.types[signature_type] else {
            unreachable!("interned function signatures have function type identity")
        };
        let value = hir::BoundCallableRef {
            receiver_parameter,
            bound,
            member,
            instantiated_signature,
        };
        let existing = self
            .bound_callable_refs
            .iter()
            .find_map(|(id, existing)| (existing == &value).then_some(id));
        let id = match existing {
            Some(id) => id,
            None => self.bound_callable_refs.alloc(value),
        };
        hir::MethodCallee::Bound(id)
    }
}
