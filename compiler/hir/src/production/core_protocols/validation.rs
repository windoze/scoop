use super::*;

impl DecodedCoreProtocolCallableV1 {
    pub(in crate::production) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreProtocolCallableV1, CoreProtocolCallableValidationError> {
        let definition = resolve_definition(foundation, self.definition)?;
        let mut resolver = ProtocolSignatureResolver { foundation };
        let signature = self
            .signature
            .resolve(&mut resolver)
            .map_err(CoreProtocolCallableValidationError::SignatureReference)?;
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
