use super::*;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::expr) fn finish_super_method_call(
        &mut self,
        candidates: Vec<crate::CallableCandidate>,
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        let resolved = self.resolve_member_overload(
            name,
            &candidates,
            receiver,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
                expected_result: expected,
                argument_protocol: crate::overload::CallArgumentProtocol::Ordinary,
            },
            sink,
        )?;
        let function = resolved.function();
        let method = self.functions[function]
            .method
            .expect("a direct-base member candidate is a method");
        if method.modifier == hir::MethodModifier::Abstract {
            self.error(
                call.span,
                format!("abstract base method `{name}` cannot be called with `super`"),
            );
            return None;
        }
        self.check_call_effects(hir::Callable::Function(function), call.span);
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
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }

    /// The unified path of a method call (explicit receiver or bare
    /// `m(...)`): `resolve_overload` picks the winner among the
    /// receiver type's methods and the call becomes a resolved
    /// `MethodCall`.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::expr) fn finish_overloaded_method_call(
        &mut self,
        candidates: Vec<crate::CallableCandidate>,
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        operator_set: bool,
    ) -> Option<hir::Expr> {
        let explicit_type_args = self.resolve_call_type_args(call.type_args)?;
        let resolved = self.resolve_member_overload(
            name,
            &candidates,
            receiver,
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
                expected_result: expected,
                argument_protocol: if operator_set {
                    crate::overload::CallArgumentProtocol::OperatorSet
                } else {
                    crate::overload::CallArgumentProtocol::Ordinary
                },
            },
            sink,
        )?;
        let ty = resolved.return_ty;
        let receiver = resolved
            .receiver
            .clone()
            .expect("an instance call returns its materialized receiver");
        let function = resolved.function();
        self.check_call_effects(hir::Callable::Function(function), call.span);
        if let Some(expr) = self.normalize_primitive_method_call(
            function,
            receiver.clone(),
            &resolved.args,
            ty,
            call.span,
        ) {
            return Some(expr);
        }
        if let Some(expr) = self.normalize_array_method_call(
            function,
            receiver.clone(),
            &resolved.args,
            ty,
            call.span,
        ) {
            return Some(expr);
        }
        if let Some(expr) = self.normalize_pointer_method_call(
            function,
            receiver.clone(),
            resolved.args.clone(),
            ty,
            call.span,
        ) {
            return Some(expr);
        }
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
            span: call.span,
            origin: self.expression_origin(call.span),
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

    pub(crate) fn materialize_method_callee(
        &mut self,
        source: crate::CallableCandidateSource,
        callable: hir::Callable,
        type_args: &[TypeId],
    ) -> hir::MethodCallee {
        let (receiver_parameter, bound_source, function) = match source {
            crate::CallableCandidateSource::Direct => {
                return hir::MethodCallee::Callable(callable);
            }
            crate::CallableCandidateSource::ClassBound {
                receiver_parameter,
                bound,
                member,
            } => (
                receiver_parameter,
                hir::BoundCallableSource::Class { bound, callable },
                member,
            ),
            crate::CallableCandidateSource::InterfaceBound {
                receiver_parameter,
                bound,
                member,
            } => (
                receiver_parameter,
                hir::BoundCallableSource::Interface { bound, member },
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
            receiver_parameter,
            source: bound_source,
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
