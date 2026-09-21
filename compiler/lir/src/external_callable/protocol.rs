use super::*;

impl ExternalCallable {
    pub(crate) fn initialization_cycle(
        target: StrongCallableDefinitionOwner,
        canonical_signature: CanonicalScoopAbiFunctionSignature,
        signature: ScoopAbiSignature,
        root_plan: ExternalCallableRootPlan,
    ) -> Result<Self, ExternalCallableBuildError> {
        validate_signature(&canonical_signature, &signature, root_plan)?;
        let (body, expected_symbol, required_definition) =
            crate::core_callable_link_contract(target)
                .map_err(ExternalCallableBuildError::ProtocolContract)?;
        Ok(Self {
            origin: ExternalCallableOrigin::InitializationCycle,
            provider: ConeIdentity::CORE,
            target,
            body,
            canonical_signature,
            calling_convention: signature.calling_convention(),
            signature,
            root_plan,
            expected_symbol,
            required_definition,
        })
    }
}
