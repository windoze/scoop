//! Validation of initialization registration definition relationships.

use super::*;

pub(super) fn require_static_storage(
    foundation: &ConeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    storage: PersistentStaticStorageId,
    digests: &StrongDigestFinalizationPlanV1,
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
    let symbol = require_symbol(foundation, PersistentSymbolKey::RootRegistration(storage))?;
    let fingerprint =
        require_digest_node(digests, DigestNodeKey::strong_registration(definition.id()))?;
    if fingerprint.id() != identity.fingerprint_node() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::ReferencedRegistrationDigestMismatch {
                registration: InitializationReferencedRegistrationV1::StaticStorage(storage),
                expected: fingerprint.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }
    Ok(StrongInitializationStaticStorageRefPlanV1 {
        storage,
        storage_symbol: require_symbol(foundation, PersistentSymbolKey::StaticStorage(storage))?,
        registration_symbol: symbol,
        registration_definition_plan: definition.id(),
        registration_primary_atom: primary,
        registration_fingerprint_node: fingerprint.id(),
    })
}

pub(super) fn require_callable(
    foundation: &ConeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    body: PersistentCallableBodyId,
    digests: &StrongDigestFinalizationPlanV1,
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
    let registration_fingerprint = require_digest_node(
        digests,
        DigestNodeKey::strong_registration(registration_definition.id()),
    )?;
    if registration_fingerprint.id() != identity.fingerprint_node() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::ReferencedRegistrationDigestMismatch {
                registration: InitializationReferencedRegistrationV1::Callable(body),
                expected: registration_fingerprint.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }
    Ok(StrongInitializationCallableRefPlanV1 {
        body,
        entry_symbol: require_symbol(foundation, PersistentSymbolKey::CallableBody(body))?,
        registration_symbol: require_symbol(
            foundation,
            PersistentSymbolKey::CallableRegistration(body),
        )?,
        body_definition_plan: body_definition.id(),
        body_primary_atom: body_primary,
        body_definition_node: body_node.id(),
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom: registration_primary,
        registration_fingerprint_node: registration_fingerprint.id(),
    })
}

pub(super) fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<&crate::DefinitionPlanRecord, StrongInitializationUnitRegistrationPlanBuildError> {
    let key = ObjectDefinitionPlanKey::strong(foundation.producer(), entity, role)
        .map_err(StrongInitializationUnitRegistrationPlanBuildError::DefinitionIdentity)?;
    foundation
        .definition_plans()
        .iter()
        .find(|record| record.key() == &key)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingDefinition(Box::new(key)))
}

pub(super) fn require_primary_atom(
    foundation: &ConeLirFoundation,
    definition: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongInitializationUnitRegistrationPlanBuildError> {
    let atoms = foundation
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == definition && record.key().role() == DefinitionAtomRole::Primary
        })
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
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == definition && record.key().role() != DefinitionAtomRole::Primary
        })
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
) -> Result<PersistentSymbolRequest, StrongInitializationUnitRegistrationPlanBuildError> {
    let symbol = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .map_err(StrongInitializationUnitRegistrationPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(symbol)
        .then_some(symbol)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingSymbol(symbol))
}

pub(super) fn require_digest_node(
    digests: &StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongInitializationUnitRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingDigestNode(key))
}

pub(super) fn require_leaf_object_node(
    digests: &StrongDigestFinalizationPlanV1,
    atom: ObjectDefinitionAtomId,
    leaf: InitializationObjectLeafV1,
) -> Result<&DigestNodeV1, StrongInitializationUnitRegistrationPlanBuildError> {
    let node = require_digest_node(digests, DigestNodeKey::object_definition(atom))?;
    if !node.direct_inputs().is_empty() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::ObjectLeafInputs {
                leaf,
                node: node.id(),
                actual: node.direct_inputs().to_vec(),
            },
        );
    }
    if !node.patch_intents().is_empty() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::ObjectLeafPatches {
                leaf,
                node: node.id(),
                actual: node
                    .patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            },
        );
    }
    Ok(node)
}

pub(super) fn require_only_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongInitializationUnitRegistrationPlanBuildError> {
    match node.patch_intents() {
        [patch] if patch.key() == &expected => Ok(patch.id()),
        actual => Err(
            StrongInitializationUnitRegistrationPlanBuildError::PatchSet {
                node: node.id(),
                expected: Box::new(expected),
                actual: actual.iter().map(|patch| *patch.key()).collect(),
            },
        ),
    }
}

fn gateway_patches(
    digests: &StrongDigestFinalizationPlanV1,
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
    digests: &StrongDigestFinalizationPlanV1,
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
    digests: &StrongDigestFinalizationPlanV1,
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
