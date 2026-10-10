//! Validation of initialization registration definition relationships.

use super::*;

pub(super) fn require_static_storage(
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,
    storage: PersistentStaticStorageId,
) -> Result<
    StrongInitializationStaticStorageRefPlanV1,
    StrongInitializationUnitRegistrationPlanBuildError,
> {
    if !foundation.contains_static_storage(storage) {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::MissingStaticStorage(storage),
        );
    }
    let Some(identity) = identities
        .static_storages()
        .iter()
        .find(|identity| identity.semantic_id() == storage)
    else {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::MissingStaticStorageRegistration(
                storage,
            ),
        );
    };
    let definition = require_definition(
        foundation,
        StrongDefinitionEntity::static_storage(storage),
        StrongDefinitionRole::RootRegistration,
    )?;
    if definition.id() != identity.definition_plan() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::ReferencedRegistrationDefinitionMismatch {
                registration: InitializationReferencedRegistrationV1::StaticStorage(storage),
                expected: definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let primary = require_primary_atom(foundation, definition.id())?;
    let symbol = require_symbol(
        foundation,
        PersistentSymbolKey::RootRegistration(storage),
        identity.owner().linkage(),
    )?;
    Ok(StrongInitializationStaticStorageRefPlanV1 {
        storage,
        storage_symbol: require_symbol(
            foundation,
            PersistentSymbolKey::StaticStorage(storage),
            identity.owner().linkage(),
        )?,
        registration_symbol: symbol,
        registration_definition_plan: definition.id(),
        registration_primary_atom: primary,
    })
}

pub(super) fn require_callable(
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,
    body: PersistentCallableBodyId,
    digests: &DigestFinalizationPlanV1,
) -> Result<StrongInitializationCallableRefPlanV1, StrongInitializationUnitRegistrationPlanBuildError>
{
    if !foundation.contains_callable_body(body) {
        return Err(StrongInitializationUnitRegistrationPlanBuildError::MissingCallableBody(body));
    }
    let Some(identity) = identities
        .callables()
        .iter()
        .find(|identity| identity.semantic_id() == body)
    else {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::MissingCallableRegistration(body),
        );
    };
    let body_definition = require_definition(
        foundation,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableBody,
    )?;
    let body_primary = require_primary_atom(foundation, body_definition.id())?;
    let body_node = require_digest_node(digests, DigestNodeKey::object_definition(body_primary))?;
    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::ReferencedRegistrationDefinitionMismatch {
                registration: InitializationReferencedRegistrationV1::Callable(body),
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let registration_primary = require_primary_atom(foundation, registration_definition.id())?;
    Ok(StrongInitializationCallableRefPlanV1 {
        body,
        entry_symbol: require_symbol(
            foundation,
            PersistentSymbolKey::CallableBody(body),
            identity.owner().linkage(),
        )?,
        registration_symbol: require_symbol(
            foundation,
            PersistentSymbolKey::CallableRegistration(body),
            identity.owner().linkage(),
        )?,
        body_definition_plan: body_definition.id(),
        body_primary_atom: body_primary,
        body_definition_node: body_node.id(),
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom: registration_primary,
    })
}

pub(super) fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<&crate::DefinitionPlanRecord, StrongInitializationUnitRegistrationPlanBuildError> {
    foundation.definition_for(entity, role).ok_or(
        StrongInitializationUnitRegistrationPlanBuildError::MissingDefinition { entity, role },
    )
}

pub(super) fn require_primary_atom(
    foundation: &ConeLirFoundation,
    definition: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongInitializationUnitRegistrationPlanBuildError> {
    let atoms = foundation
        .definition_atoms_for_plan(definition)
        .filter(|record| record.key().role() == DefinitionAtomRole::Primary)
        .map(|record| record.id())
        .collect::<Vec<_>>();
    match atoms.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(
            StrongInitializationUnitRegistrationPlanBuildError::PrimaryAtomSet {
                plan: definition,
                actual: atoms,
            },
        ),
    }
}

pub(super) fn require_associated_atoms<const N: usize>(
    foundation: &ConeLirFoundation,
    unit: PersistentInitializationUnitId,
    definition: ObjectDefinitionPlanId,
    mut expected: [ObjectDefinitionAtomKey; N],
) -> Result<[ObjectDefinitionAtomId; N], StrongInitializationUnitRegistrationPlanBuildError> {
    expected.sort_unstable();
    let mut records = foundation
        .definition_atoms_for_plan(definition)
        .filter(|record| record.key().role() != DefinitionAtomRole::Primary)
        .collect::<Vec<_>>();
    records.sort_unstable_by(|a, b| a.key().cmp(b.key()));
    let actual = records
        .iter()
        .map(|record| record.key().clone())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::AssociatedAtomSet {
                unit,
                definition,
                expected: expected.to_vec(),
                actual,
            },
        );
    }
    let records: [_; N] = records.try_into().map_err(|_| {
        StrongInitializationUnitRegistrationPlanBuildError::AssociatedAtomSet {
            unit,
            definition,
            expected: expected.to_vec(),
            actual,
        }
    })?;
    Ok(records.map(|record| record.id()))
}

pub(super) fn require_symbol(
    foundation: &ConeLirFoundation,
    key: PersistentSymbolKey,
    linkage: LinkageClass,
) -> Result<PersistentSymbolRequest, StrongInitializationUnitRegistrationPlanBuildError> {
    let symbol = PersistentSymbolRequest::new(key, linkage)
        .map_err(StrongInitializationUnitRegistrationPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(symbol)
        .then_some(symbol)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingSymbol(symbol))
}

pub(super) fn require_digest_node(
    digests: &DigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongInitializationUnitRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingDigestNode(key))
}

fn gateway_patches(
    digests: &DigestFinalizationPlanV1,
    registration: ObjectDefinitionPlanId,
) -> Vec<(DigestPatchIntentKey, DigestPatchIntentId)> {
    digests
        .nodes()
        .iter()
        .flat_map(|node| node.patch_intents())
        .filter(|patch| {
            patch.key().target_definition() == registration
                && patch.key().atom_role() == DefinitionAtomRole::Primary
                && patch.key().semantic_field_role() == DigestSemanticFieldRole::GatewayDefinition
        })
        .map(|patch| (*patch.key(), patch.id()))
        .collect()
}

pub(super) fn require_gateway_patch(
    digests: &DigestFinalizationPlanV1,
    registration: ObjectDefinitionPlanId,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongInitializationUnitRegistrationPlanBuildError> {
    let actual = gateway_patches(digests, registration);
    match actual.as_slice() {
        [(key, id)] if *key == expected => Ok(*id),
        _ => Err(
            StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet {
                registration,
                expected: Some(Box::new(expected)),
                actual: actual.into_iter().map(|(key, _)| key).collect(),
            },
        ),
    }
}

pub(super) fn require_no_gateway_patches(
    digests: &DigestFinalizationPlanV1,
    registration: ObjectDefinitionPlanId,
) -> Result<(), StrongInitializationUnitRegistrationPlanBuildError> {
    let actual = gateway_patches(digests, registration);
    if actual.is_empty() {
        return Ok(());
    }
    Err(
        StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet {
            registration,
            expected: None,
            actual: actual.into_iter().map(|(key, _)| key).collect(),
        },
    )
}
