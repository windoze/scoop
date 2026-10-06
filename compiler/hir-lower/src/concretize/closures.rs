use super::*;

mod references;

#[derive(Debug)]
pub(super) struct PendingCallableReference {
    pub(super) source: export::CallableReferenceId,
    pub(super) owner_arguments: Vec<concrete::TypeId>,
    target: concrete::CallableReferenceTarget,
    function_type: concrete::FunctionTypeId,
    pub(super) captures: Vec<concrete::Capture>,
    origin: export::DefinitionOrigin,
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
            origin: pending.origin,
            span: pending.span,
        });
        assert_eq!(id.into_raw().into_u32() as usize, references.len() - 1);
    }
    references
}

impl Concretizer<'_> {
    pub(super) fn lower_lambda(
        &mut self,
        source_id: export::LambdaId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::LambdaId {
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
            function: self.request_lexical_function(source.definition, body_arguments),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        self.lambdas.alloc(value)
    }

    pub(super) fn lower_anonymous(
        &mut self,
        source_id: export::AnonymousFunctionId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AnonymousFunctionId {
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
            function: self.request_lexical_function(source.definition, body_arguments),
            function_type: self.lower_function_type(source.function_type, substitution),
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            span: source.span,
        };
        self.anonymous_functions.alloc(value)
    }

    fn intern_local_function(
        &mut self,
        function: concrete::FunctionId,
        definition_path: scoop_identity::StructuralDefinitionPath,
        function_type: concrete::FunctionTypeId,
    ) -> concrete::LocalFunctionId {
        if let Some(&id) = self.local_by_function.get(&function) {
            return id;
        }
        let key = &self.function_keys[function.into_raw().into_u32() as usize];
        let span = self.resolved_function_definition(key).signature().span;
        let id = self.local_functions.alloc(concrete::LocalFunction {
            definition_path,
            function,
            function_type,
            span,
        });
        self.local_by_function.insert(function, id);
        id
    }

    pub(super) fn lower_reference(
        &mut self,
        source_id: export::CallableReferenceId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::CallableReferenceId {
        let source = self.source.callable_references[source_id].clone();
        let function_type = self.lower_function_type(source.function_type, substitution);
        let target =
            self.lower_reference_target(&source.target, function_type, substitution, locals);
        let value = PendingCallableReference {
            source: source_id,
            owner_arguments: source
                .owner_type_arguments
                .iter()
                .map(|&argument| self.lower_type(argument, substitution))
                .collect(),
            target,
            function_type,
            captures: source
                .captures
                .iter()
                .map(|capture| self.lower_capture(capture, substitution, locals))
                .collect(),
            origin: source.origin,
            span: source.span,
        };
        let id = concrete::CallableReferenceId::from_raw(
            (self.callable_reference_slots.len() as u32).into(),
        );
        self.callable_reference_slots.push(value);
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
        self.ensure_coercion_box_sources(
            self.function_types[value.source].canonical_type,
            self.function_types[value.target].canonical_type,
        );
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
        let source = self.source.foreign_callback_registrations[source_id].clone();
        let arguments = match &source.definition {
            export::ForeignCallbackDefinition::Source { .. } => substitution.to_vec(),
            export::ForeignCallbackDefinition::Imported { arguments, .. } => arguments
                .iter()
                .map(|argument| self.lower_type(*argument, substitution))
                .collect(),
        };
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.foreign_callback_by_key.get(&key) {
            assert_eq!(
                self.foreign_callback_slots[id.into_raw().into_u32() as usize].callback,
                callback
            );
            return id;
        }
        let native_function_type =
            self.lower_function_type(source.native_function_type, substitution);
        let managed_function_type =
            self.lower_function_type(source.managed_function_type, substitution);
        self.lower_imported_callback_support();
        let mode = source.mode;
        let id = concrete::ForeignCallbackRegistrationId::from_raw(
            (self.foreign_callback_slots.len() as u32).into(),
        );
        self.foreign_callback_slots
            .push(PendingForeignCallbackRegistration {
                source: source_id,
                arguments,
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
