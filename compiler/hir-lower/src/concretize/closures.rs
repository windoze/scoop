use super::*;

#[derive(Debug)]
pub(super) struct PendingCallableReference {
    pub(super) source: export::CallableReferenceId,
    pub(super) owner_arguments: Vec<concrete::TypeId>,
    target: concrete::CallableReferenceTarget,
    function_type: concrete::FunctionTypeId,
    captures: Vec<concrete::Capture>,
    span: scoop_ast::Span,
}

pub(super) fn finish_callable_references(
    pending: Vec<PendingCallableReference>,
    identities: Vec<concrete::CallableReferenceIdentity>,
) -> Arena<concrete::CallableReference> {
    assert_eq!(pending.len(), identities.len());
    let mut references = Arena::new();
    for (pending, identity) in pending.into_iter().zip(identities) {
        let id = references.alloc(concrete::CallableReference {
            identity,
            target: pending.target,
            function_type: pending.function_type,
            captures: pending.captures,
            span: pending.span,
        });
        assert_eq!(id.into_raw().into_u32() as usize, references.len() - 1);
    }
    references
}

impl Concretizer<'_> {
    pub(super) fn ensure_lambda(
        &mut self,
        source_id: export::LambdaId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::LambdaId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.lambda_by_key.get(&key) {
            return id;
        }
        let source = self.source.lambdas[source_id].clone();
        let body_arguments = match &source.body_type_arguments {
            export::CallableBodyTypeArguments::Lexical => substitution.to_vec(),
            export::CallableBodyTypeArguments::Explicit(arguments) => arguments
                .iter()
                .map(|&argument| self.lower_type(argument, substitution))
                .collect(),
        };
        let value = concrete::Lambda {
            definition_path: source.definition_path,
            function: self.request_function(source.function, body_arguments),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.lambdas.alloc(value);
        self.lambda_by_key.insert(key, id);
        id
    }

    pub(super) fn ensure_anonymous(
        &mut self,
        source_id: export::AnonymousFunctionId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AnonymousFunctionId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.anonymous_by_key.get(&key) {
            return id;
        }
        let source = self.source.anonymous_functions[source_id].clone();
        let body_arguments = match &source.body_type_arguments {
            export::CallableBodyTypeArguments::Lexical => substitution.to_vec(),
            export::CallableBodyTypeArguments::Explicit(arguments) => arguments
                .iter()
                .map(|&argument| self.lower_type(argument, substitution))
                .collect(),
        };
        let value = concrete::AnonymousFunction {
            definition_path: source.definition_path,
            function: self.request_function(source.function, body_arguments),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.anonymous_functions.alloc(value);
        self.anonymous_by_key.insert(key, id);
        id
    }

    pub(super) fn ensure_local_function(
        &mut self,
        source_id: export::LocalFunctionId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::LocalFunctionId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.local_by_key.get(&key) {
            return id;
        }
        let source = self.source.local_functions[source_id].clone();
        let value = concrete::LocalFunction {
            definition_path: source.definition_path,
            function: self.request_function(source.function, substitution.to_vec()),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = self.local_functions.alloc(value);
        self.local_by_key.insert(key, id);
        id
    }

    pub(super) fn ensure_reference(
        &mut self,
        source_id: export::CallableReferenceId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::CallableReferenceId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.reference_by_key.get(&key) {
            return id;
        }
        let source = self.source.callable_references[source_id].clone();
        let target = match source.target {
            export::CallableReferenceTarget::Named(callee) => {
                concrete::CallableReferenceTarget::Named(self.lower_callable(callee, substitution))
            }
            export::CallableReferenceTarget::Local {
                local_function,
                callee,
            } => {
                let (callee, arguments) = self.lower_callable_with_arguments(callee, substitution);
                concrete::CallableReferenceTarget::Local {
                    local_function: self.ensure_local_function(local_function, &arguments, locals),
                    callee,
                }
            }
            export::CallableReferenceTarget::BoundMember { receiver, callee } => {
                let mut receiver = self.lower_expr(&receiver, substitution, locals);
                let callee = match callee {
                    export::MethodCallee::Callable(callable) => {
                        self.lower_callable(callable, substitution)
                    }
                    export::MethodCallee::Bound(bound) => {
                        let (callee, interface) =
                            self.resolve_bound_callee(bound, receiver.ty, substitution);
                        if let Some(interface) = interface {
                            receiver = self.adapt_receiver_to_interface(receiver, interface);
                        }
                        callee
                    }
                    export::MethodCallee::DerivedEquality(application) => {
                        self.lower_derived_equality_application(application, substitution)
                    }
                };
                concrete::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(receiver),
                    callee,
                }
            }
            export::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                concrete::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.lower_expr(&receiver, substitution, locals)),
                    callee: self.lower_callable(callee, substitution),
                }
            }
        };
        assert!(source.owner_type_param_count <= substitution.len());
        let value = PendingCallableReference {
            source: source_id,
            owner_arguments: substitution[..source.owner_type_param_count].to_vec(),
            target,
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        let id = concrete::CallableReferenceId::from_raw(
            (self.callable_reference_slots.len() as u32).into(),
        );
        self.callable_reference_slots.push(value);
        self.reference_by_key.insert(key, id);
        id
    }

    pub(super) fn ensure_coercion(
        &mut self,
        source_id: export::FunctionCoercionId,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionCoercionId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.coercion_by_key.get(&key) {
            return id;
        }
        let source = self.source.function_coercions[source_id].clone();
        let value = concrete::FunctionCoercion {
            source: self.lower_function_type(source.source, substitution),
            target: self.lower_function_type(source.target, substitution),
        };
        let id = self.function_coercions.alloc(value);
        self.coercion_by_key.insert(key, id);
        id
    }

    pub(super) fn ensure_foreign_callback(
        &mut self,
        source_id: export::ForeignCallbackRegistrationId,
        substitution: &[concrete::TypeId],
        callback: concrete::StructId,
    ) -> concrete::ForeignCallbackRegistrationId {
        let key = (source_id, substitution.to_vec());
        if let Some(&id) = self.foreign_callback_by_key.get(&key) {
            assert_eq!(
                self.foreign_callback_slots[id.into_raw().into_u32() as usize].callback,
                callback
            );
            return id;
        }
        let source = self.source.foreign_callback_registrations[source_id].clone();
        let native_function_type =
            self.lower_function_type(source.native_function_type, substitution);
        let managed_function_type =
            self.lower_function_type(source.managed_function_type, substitution);
        assert!(
            self.source
                .foreign_callback_core
                .modes
                .contains(source.mode)
        );
        let mode = self.lower_applied_enum_variant_ref(source.mode, substitution);
        let id = concrete::ForeignCallbackRegistrationId::from_raw(
            (self.foreign_callback_slots.len() as u32).into(),
        );
        self.foreign_callback_slots
            .push(PendingForeignCallbackRegistration {
                source: source_id,
                arguments: substitution.to_vec(),
                callback,
                native_function_type,
                managed_function_type,
                context_index: source.context_index,
                mode,
            });
        self.foreign_callback_by_key.insert(key, id);
        id
    }
}
