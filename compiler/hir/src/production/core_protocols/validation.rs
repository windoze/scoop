use super::*;

impl DecodedCoreProtocolCallableV1 {
    pub(in crate::production) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreProtocolCallableV1, CoreProtocolCallableValidationError> {
        let definition = resolve_definition(foundation, self.definition)?;
        if foundation
            .definition_origin(definition.origin_subject())
            .is_none()
        {
            return Err(CoreProtocolCallableValidationError::MissingDefinitionOrigin(definition));
        }
        let mut resolver = ProtocolSignatureResolver { foundation };
        let signature = self
            .signature
            .resolve(&mut resolver)
            .map_err(CoreProtocolCallableValidationError::SignatureReference)?;
        validate_definition_signature(foundation, definition, &signature)?;
        Ok(CoreProtocolCallableV1 {
            definition,
            signature,
        })
    }
}

fn resolve_definition(
    foundation: &CanonicalHirFoundation,
    decoded: DecodedCoreProtocolCallableDefinitionV1,
) -> Result<CoreProtocolCallableDefinitionV1, CoreProtocolCallableValidationError> {
    match decoded {
        DecodedCoreProtocolCallableDefinitionV1::Function(id) => foundation
            .function_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::Function(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownFunction(
                *id.as_array(),
            )),
        DecodedCoreProtocolCallableDefinitionV1::GenericFunction(id) => foundation
            .generic_function_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::GenericFunction(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownGenericFunction(
                *id.as_array(),
            )),
        DecodedCoreProtocolCallableDefinitionV1::Constructor(id) => foundation
            .constructor_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::Constructor(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownConstructor(
                *id.as_array(),
            )),
        DecodedCoreProtocolCallableDefinitionV1::GeneratedCallable(id) => foundation
            .generated_callable_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::GeneratedCallable(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownGeneratedCallable(*id.as_array())),
    }
}

fn validate_definition_signature(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    match definition {
        CoreProtocolCallableDefinitionV1::Function(id) => {
            let (_, source) = foundation
                .function_by_bytes(id.as_array())
                .expect("resolved function remains in the foundation");
            validate_source_function(foundation, definition, source, signature)
        }
        CoreProtocolCallableDefinitionV1::GenericFunction(id) => {
            let (_, source) = foundation
                .generic_function_by_bytes(id.as_array())
                .expect("resolved generic function remains in the foundation");
            validate_source_function(foundation, definition, source, signature)
        }
        CoreProtocolCallableDefinitionV1::Constructor(id) => {
            let (_, source) = foundation
                .constructor_by_bytes(id.as_array())
                .expect("resolved constructor remains in the foundation");
            validate_source_constructor(foundation, definition, source, signature)
        }
        CoreProtocolCallableDefinitionV1::GeneratedCallable(id) => {
            let (_, key) = foundation
                .generated_callable_by_bytes(id.as_array())
                .expect("resolved generated callable remains in the foundation");
            let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } = key else {
                return Err(
                    CoreProtocolCallableValidationError::UnsupportedGeneratedRole(definition),
                );
            };
            let (_, source) = foundation
                .constructor_by_bytes(constructor.as_array())
                .ok_or(CoreProtocolCallableValidationError::MissingAdapterSource(
                    definition,
                ))?;
            if !signature.parameters().is_empty() {
                return Err(CoreProtocolCallableValidationError::AdapterHasParameters(
                    definition,
                ));
            }
            validate_constructor_common(foundation, definition, source, signature)
        }
    }
}

fn validate_source_function(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    let DuplicateSignatureKey::Function {
        type_parameter_count,
        receiver,
        parameters,
    } = source.duplicate_signature()
    else {
        return Err(CoreProtocolCallableValidationError::WrongSourceKind(
            definition,
        ));
    };
    let receiver_matches = match (signature.receiver(), receiver) {
        (OptionalSignatureType::Absent, OptionalSignatureType::Absent) => true,
        (OptionalSignatureType::Present(actual), OptionalSignatureType::Present(expected)) => {
            actual == expected
        }
        _ => false,
    };
    if !receiver_matches || signature.parameters() != parameters {
        return Err(CoreProtocolCallableValidationError::SourceSignatureMismatch(definition));
    }
    validate_protocol_binders(
        foundation,
        source,
        signature,
        *type_parameter_count,
        definition,
    )
}

fn validate_source_constructor(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    let DuplicateSignatureKey::Constructor { parameters } = source.duplicate_signature() else {
        return Err(CoreProtocolCallableValidationError::WrongSourceKind(
            definition,
        ));
    };
    if signature.parameters() != parameters {
        return Err(CoreProtocolCallableValidationError::SourceSignatureMismatch(definition));
    }
    validate_constructor_common(foundation, definition, source, signature)
}

fn validate_constructor_common(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    if signature.effect() != Effect::Ordinary
        || !matches!(signature.receiver(), OptionalSignatureType::Absent)
    {
        return Err(CoreProtocolCallableValidationError::InvalidConstructorShape(definition));
    }
    let owner = source
        .owners()
        .owners()
        .last()
        .ok_or(CoreProtocolCallableValidationError::MissingConstructorOwner(definition))?;
    let valid_result = match (owner, signature.result()) {
        (DefinitionOwnerAtom::Type(owner), SignatureTypeKey::Nominal(result)) => owner == result,
        (
            DefinitionOwnerAtom::GenericType(owner),
            SignatureTypeKey::NominalApplication { origin, .. },
        ) => owner == origin,
        _ => false,
    };
    if !valid_result {
        return Err(CoreProtocolCallableValidationError::ConstructorResultMismatch(definition));
    }
    validate_protocol_binders(foundation, source, signature, 0, definition)
}

fn validate_protocol_binders(
    foundation: &CanonicalHirFoundation,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
    own_type_parameter_count: u32,
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<(), CoreProtocolCallableValidationError> {
    let mut binder_parameter_counts = Vec::new();
    if own_type_parameter_count > 0 {
        binder_parameter_counts.push(own_type_parameter_count);
    }
    for owner in source.owners().owners().iter().rev() {
        let count = match owner {
            DefinitionOwnerAtom::GenericType(id) => foundation
                .generic_type_by_bytes(id.as_array())
                .map(|(_, source)| source.duplicate_signature().type_parameter_count())
                .ok_or_else(|| {
                    CoreProtocolCallableValidationError::UnknownBinderOwner(owner.clone())
                })?,
            DefinitionOwnerAtom::GenericFunction(id) => foundation
                .generic_function_by_bytes(id.as_array())
                .map(|(_, source)| source.duplicate_signature().type_parameter_count())
                .ok_or_else(|| {
                    CoreProtocolCallableValidationError::UnknownBinderOwner(owner.clone())
                })?,
            DefinitionOwnerAtom::ExtensionProperty(id) => foundation
                .extension_property_by_bytes(id.as_array())
                .map(|(_, source)| source.duplicate_signature().type_parameter_count())
                .ok_or_else(|| {
                    CoreProtocolCallableValidationError::UnknownBinderOwner(owner.clone())
                })?,
            DefinitionOwnerAtom::Type(_)
            | DefinitionOwnerAtom::Function(_)
            | DefinitionOwnerAtom::Constructor(_)
            | DefinitionOwnerAtom::Property(_)
            | DefinitionOwnerAtom::GeneratedCallable(_)
            | DefinitionOwnerAtom::PropertyAccessor(_)
            | DefinitionOwnerAtom::EnumVariant(_) => 0,
        };
        if count > 0 {
            binder_parameter_counts.push(count);
        }
    }
    let mut types = signature.parameters().iter().collect::<Vec<_>>();
    types.push(signature.result());
    if let OptionalSignatureType::Present(receiver) = signature.receiver() {
        types.push(receiver);
    }
    for ty in types {
        validate_protocol_signature_binders(ty, &binder_parameter_counts).map_err(|reason| {
            CoreProtocolCallableValidationError::InvalidBinder { definition, reason }
        })?;
    }
    Ok(())
}

fn validate_protocol_signature_binders(
    signature: &SignatureTypeKey,
    binder_parameter_counts: &[u32],
) -> Result<(), CoreProtocolBinderError> {
    match signature {
        SignatureTypeKey::Nominal(_) => Ok(()),
        SignatureTypeKey::NominalApplication { arguments, .. }
        | SignatureTypeKey::Tuple(arguments) => {
            for argument in arguments.as_slice() {
                validate_protocol_signature_binders(argument, binder_parameter_counts)?;
            }
            Ok(())
        }
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for parameter in parameters {
                validate_protocol_signature_binders(parameter, binder_parameter_counts)?;
            }
            validate_protocol_signature_binders(result, binder_parameter_counts)
        }
        SignatureTypeKey::RawPointer(pointee) => {
            validate_protocol_signature_binders(pointee, binder_parameter_counts)
        }
        SignatureTypeKey::Binder { depth, index } => {
            let depth_index = usize::try_from(*depth)
                .map_err(|_| CoreProtocolBinderError::DepthOutOfRange(*depth))?;
            let Some(&count) = binder_parameter_counts.get(depth_index) else {
                return Err(CoreProtocolBinderError::DepthOutOfRange(*depth));
            };
            if *index >= count {
                return Err(CoreProtocolBinderError::IndexOutOfRange {
                    depth: *depth,
                    index: *index,
                    type_parameter_count: count,
                });
            }
            Ok(())
        }
    }
}

struct ProtocolSignatureResolver<'a> {
    foundation: &'a CanonicalHirFoundation,
}

impl PersistentIdResolver<PersistentTypeId> for ProtocolSignatureResolver<'_> {
    type Error = CoreProtocolSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.foundation
            .source_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreProtocolSignatureReferenceError::UnknownSourceType(
                *id.as_array(),
            ))
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for ProtocolSignatureResolver<'_> {
    type Error = CoreProtocolSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        self.foundation
            .generic_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreProtocolSignatureReferenceError::UnknownGenericType(
                *id.as_array(),
            ))
    }
}
