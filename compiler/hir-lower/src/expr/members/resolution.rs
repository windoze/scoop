use super::*;

impl Lowerer {
    pub(in crate::expr) fn finish_resolved_super_method_call(
        &mut self,
        resolved: crate::overload::ResolvedCallee,
        name: &str,
        span: Span,
    ) -> Option<hir::Expr> {
        let function = resolved.function();
        let method = self.functions[function]
            .method
            .expect("a direct-base member candidate is a method");
        if method.modifier == hir::MethodModifier::Abstract {
            self.error(
                span,
                format!("abstract base method `{name}` cannot be called with `super`"),
            );
            return None;
        }
        self.check_call_effects(hir::Callable::Function(function), span);
        let receiver = resolved
            .receiver
            .clone()
            .expect("a direct-base call materializes its receiver");
        let callee = self.materialize_resolved_callee(&resolved);
        let callee = self.materialize_method_callee(resolved.source, callee, &resolved.type_args);
        Some(hir::Expr {
            kind: ExprKind::DirectSuperMethodCall {
                receiver: Box::new(receiver),
                callee,
                args: resolved.args,
            },
            ty: resolved.return_ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(in crate::expr) fn finish_resolved_method_call(
        &mut self,
        resolved: crate::overload::ResolvedCallee,
        span: Span,
        sink: &[hir::Statement],
    ) -> Option<hir::Expr> {
        let ty = resolved.return_ty;
        let receiver = resolved
            .receiver
            .clone()
            .expect("an instance call returns its materialized receiver");
        let function = resolved.function();
        self.check_call_effects(hir::Callable::Function(function), span);
        if let hir::FunctionKind::Intrinsic(intrinsic) = self.functions[function].kind
            && let hir::IntrinsicFunctionKind::Atomic(kind) = intrinsic.kind
        {
            return self.normalize_atomic_method(kind, receiver, resolved.args, ty, span, sink);
        }
        if matches!(
            self.functions[function].kind,
            hir::FunctionKind::DerivedEquality
        ) {
            let callee = hir::MethodCallee::DerivedEquality(
                self.derived_equality_application_by_type[&receiver.ty],
            );
            return Some(hir::Expr {
                kind: ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee,
                    args: resolved.args,
                },
                ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        if let Some(expr) = self.normalize_primitive_method_call(
            function,
            receiver.clone(),
            &resolved.args,
            ty,
            span,
        ) {
            return Some(expr);
        }
        if let Some(expr) =
            self.normalize_array_method_call(function, receiver.clone(), &resolved.args, ty, span)
        {
            return Some(expr);
        }
        if let Some(expr) = self.normalize_pointer_method_call(
            function,
            receiver.clone(),
            resolved.args.clone(),
            ty,
            span,
        ) {
            return Some(expr);
        }
        let receiver = if resolved.source == crate::CallableCandidateSource::Direct
            && let Some(method) = self.functions[function].method
            && matches!(self.types[method.owner], Type::Interface(_))
        {
            let owner = self.instantiate_ty(method.owner, &resolved.type_args);
            self.adapt_to(receiver, owner)
        } else {
            receiver
        };
        let callee = self.materialize_resolved_callee(&resolved);
        let method_callee =
            self.materialize_method_callee(resolved.source, callee, &resolved.type_args);
        Some(hir::Expr {
            kind: ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: method_callee,
                args: resolved.args,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// Whether the current host type has a property named `name`
    /// (the quiet probe behind the enum-path fallback in
    /// `lower_method_call`: a bare receiver name that would resolve
    /// to `this.name` is a property access, not an enum path).
    pub(crate) fn host_has_property(&self, name: &str) -> bool {
        let Some(receiver_ty) = self.current_this_ty() else {
            return false;
        };
        let mut state = self.clone();
        if state
            .find_accessible_nominal_property(receiver_ty, name)
            .is_some()
        {
            return true;
        }
        state
            .imported_member_candidates(
                receiver_ty,
                hir::ImportedMemberLookup::PropertyGetter(name),
            )
            .is_ok_and(|candidates| !candidates.is_empty())
    }

    pub(crate) fn materialize_method_callee(
        &mut self,
        source: crate::CallableCandidateSource,
        callable: hir::Callable,
        type_args: &[TypeId],
    ) -> hir::MethodCallee {
        let (receiver_parameter, bound_source, function) = match source {
            crate::CallableCandidateSource::Direct => {
                return hir::MethodCallee::Callable(callable.into());
            }
            crate::CallableCandidateSource::ClassBound {
                receiver_parameter,
                bound,
                member,
            } => (
                receiver_parameter,
                hir::BoundCallableSource::Class {
                    bound,
                    callable: callable.into(),
                },
                member,
            ),
            crate::CallableCandidateSource::InterfaceBound {
                receiver_parameter,
                bound,
                member,
            } => (
                receiver_parameter,
                hir::BoundCallableSource::Interface {
                    bound,
                    member: hir::InterfaceMethodReference::Local(member),
                    declared: callable.into(),
                },
                self.interface_method_entities[member].function,
            ),
        };
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
            receiver_type: self.intern_type(Type::Param(receiver_parameter)),
            source: bound_source,
            instantiated_signature,
        };
        hir::MethodCallee::Bound(self.record_bound_callable(value))
    }

    pub(crate) fn record_bound_callable(
        &mut self,
        value: hir::BoundCallableRef,
    ) -> hir::BoundCallableRefId {
        let existing = self
            .bound_callable_refs
            .iter()
            .find_map(|(id, existing)| (existing == &value).then_some(id));
        existing.unwrap_or_else(|| self.bound_callable_refs.alloc(value))
    }
}
