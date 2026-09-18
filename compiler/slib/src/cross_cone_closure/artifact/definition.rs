//! Terminal-provider Strong-definition resolution for Link imports.

use std::collections::BTreeMap;

use scoop_identity::{
    CallableBodyKey, ConeIdentity, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, PersistentSymbolRequest, StrongDefinitionEntity,
    StrongDefinitionRole,
};

use super::{
    CrossConeArtifactClosureValidationError, CrossConeDefinitionIdentityMismatch,
    CrossConeDefinitionResolutionError, CrossConeMissingStrongDefinition,
    CrossConeStrongDefinitionOwnerMismatch,
};
use crate::{LinkDefinitionOwnerV1, StrongDefinitionOwnerV1, ValidatedCrossConeStrongLinkArtifact};

pub(super) fn validate_terminal_definitions(
    links: &[ValidatedCrossConeStrongLinkArtifact<'_>],
    positions: &BTreeMap<ConeIdentity, usize>,
) -> Result<(), CrossConeArtifactClosureValidationError> {
    for consumer in links {
        for import in consumer
            .cross_cone_link_closure()
            .semantic_imports()
            .imports()
        {
            let provider = positions.get(&import.provider()).copied().ok_or(
                CrossConeDefinitionResolutionError::MissingProvider {
                    consumer: consumer.identity(),
                    provider: import.provider(),
                },
            )?;
            validate_terminal_definition(consumer.identity(), import, &links[provider])?;
        }
    }
    Ok(())
}

fn validate_terminal_definition(
    consumer: ConeIdentity,
    import: &crate::CrossConeLinkSemanticImportV1,
    provider: &ValidatedCrossConeStrongLinkArtifact<'_>,
) -> Result<(), CrossConeDefinitionResolutionError> {
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(import.target()))
        .map_err(CrossConeDefinitionResolutionError::Identity)?;
    let entity = StrongDefinitionEntity::callable_body(body);
    let definition_key = ObjectDefinitionPlanKey::strong(
        import.provider(),
        entity,
        StrongDefinitionRole::CallableBody,
    )
    .map_err(
        |_| CrossConeDefinitionResolutionError::InvalidDefinitionOwner {
            consumer,
            provider: import.provider(),
            definition: import.required_definition(),
        },
    )?;
    let expected_definition = ObjectDefinitionPlanId::from_key(&definition_key)
        .map_err(CrossConeDefinitionResolutionError::Identity)?;
    if import.required_definition() != expected_definition {
        return Err(
            CrossConeDefinitionResolutionError::DefinitionIdentityMismatch(Box::new(
                CrossConeDefinitionIdentityMismatch {
                    consumer,
                    provider: import.provider(),
                    expected: expected_definition,
                    actual: import.required_definition(),
                },
            )),
        );
    }
    let expected_owner = StrongDefinitionOwnerV1::new(entity, StrongDefinitionRole::CallableBody)
        .map(LinkDefinitionOwnerV1::StrongDefinition)
        .map_err(
            |_| CrossConeDefinitionResolutionError::InvalidDefinitionOwner {
                consumer,
                provider: import.provider(),
                definition: import.required_definition(),
            },
        )?;
    let expected_symbol = macho_name(import.expected_symbol());
    let actual = provider
        .defined_symbols()
        .owners()
        .binary_search_by(|candidate| candidate.symbol().cmp(expected_symbol.as_slice()))
        .ok()
        .map(|position| &provider.defined_symbols().owners()[position])
        .ok_or_else(|| {
            CrossConeDefinitionResolutionError::MissingStrongDefinition(Box::new(
                CrossConeMissingStrongDefinition {
                    consumer,
                    provider: import.provider(),
                    definition: import.required_definition(),
                    symbol: import.expected_symbol(),
                },
            ))
        })?;
    if actual.owner() != expected_owner {
        return Err(
            CrossConeDefinitionResolutionError::StrongDefinitionOwnerMismatch(Box::new(
                CrossConeStrongDefinitionOwnerMismatch {
                    consumer,
                    provider: import.provider(),
                    definition: import.required_definition(),
                    symbol: import.expected_symbol(),
                    actual: actual.owner(),
                },
            )),
        );
    }
    Ok(())
}

fn macho_name(request: PersistentSymbolRequest) -> Vec<u8> {
    let symbol = request.symbol();
    let mut name = Vec::with_capacity(symbol.as_str().len() + 1);
    name.push(b'_');
    name.extend_from_slice(symbol.as_str().as_bytes());
    name
}
