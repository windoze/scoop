use std::fmt;

use scoop_identity::{
    CallableBodyKey, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExecutableSourceEntryIdentity, LinkageClass,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole, PersistentCallableBodyId,
    PersistentStaticStorageId, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    StaticStorageKey, StrongDefinitionEntity, StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::HashError;

use super::ExecutableEntryPlanV1;
use crate::{
    ConeLirFoundation, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
};

pub(super) fn build_executable(
    source: ExecutableSourceEntryIdentity,
    foundation: &ConeLirFoundation,
    registrations: &StrongRegistrationIdentitySurfaceV1,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<ExecutableEntryPlanV1, EntryProductionPlanBuildError> {
    let root_cone = source.root_cone();
    if root_cone != foundation.producer() {
        return Err(EntryProductionPlanBuildError::ProducerMismatch {
            source: root_cone,
            foundation: foundation.producer(),
        });
    }
    let declaration = source.declaration();
    let source_signature = source.source_signature().clone();
    let source_signature_fingerprint = source.source_signature_fingerprint();
    let main = source.main();
    let gateway =
        PersistentCallableBodyId::from_key(&CallableBodyKey::root_gateway(root_cone, main))
            .map_err(EntryProductionPlanBuildError::Hash)?;
    let failure_root = PersistentStaticStorageId::from_key(
        &StaticStorageKey::root_entry_failure_root(root_cone, main),
    )
    .map_err(EntryProductionPlanBuildError::Hash)?;
    validate_root_only_entities(foundation, gateway, failure_root)?;

    for body in [main.body(), gateway] {
        if !foundation.contains_callable_body(body) {
            return Err(EntryProductionPlanBuildError::MissingCallableBody(body));
        }
        require_callable_registration(registrations, body)?;
        require_symbol(foundation, PersistentSymbolKey::CallableBody(body))?;
        require_symbol(foundation, PersistentSymbolKey::CallableRegistration(body))?;
        require_definition(
            foundation,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )?;
        require_definition(
            foundation,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableRegistration,
        )?;
    }
    if !foundation.contains_static_storage(failure_root) {
        return Err(EntryProductionPlanBuildError::MissingFailureRoot(
            failure_root,
        ));
    }
    require_storage_registration(registrations, failure_root)?;
    require_symbol(foundation, PersistentSymbolKey::StaticStorage(failure_root))?;
    require_symbol(
        foundation,
        PersistentSymbolKey::RootRegistration(failure_root),
    )?;
    require_definition(
        foundation,
        StrongDefinitionEntity::static_storage(failure_root),
        StrongDefinitionRole::StaticStorage,
    )?;
    require_definition(
        foundation,
        StrongDefinitionEntity::static_storage(failure_root),
        StrongDefinitionRole::RootRegistration,
    )?;

    let root_descriptor_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::RootEntryDescriptor(root_cone),
    )?;
    let root_descriptor_definition = require_definition(
        foundation,
        StrongDefinitionEntity::root_entry(root_cone),
        StrongDefinitionRole::RootEntryDescriptor,
    )?;
    require_primary_atom(foundation, root_descriptor_definition)?;
    let gateway_definition = require_definition(
        foundation,
        StrongDefinitionEntity::callable_body(gateway),
        StrongDefinitionRole::CallableBody,
    )?;
    let gateway_atom = require_primary_atom(foundation, gateway_definition)?;

    let source_node = require_digest_node(digests, DigestNodeKey::source_signature(main.body()))?;
    let source_signature_patch = require_patch(
        source_node,
        root_descriptor_definition,
        DigestSemanticFieldRole::SourceSignature,
    )?;
    let gateway_node =
        require_digest_node(digests, DigestNodeKey::object_definition(gateway_atom))?;
    let gateway_definition_patch = require_patch(
        gateway_node,
        root_descriptor_definition,
        DigestSemanticFieldRole::GatewayDefinition,
    )?;

    Ok(ExecutableEntryPlanV1 {
        root_cone,
        declaration,
        source_signature,
        source_signature_fingerprint,
        main,
        failure_root,
        gateway,
        root_descriptor_symbol,
        root_descriptor_definition,
        source_signature_patch,
        gateway_definition_patch,
    })
}

pub(super) fn validate_library_foundation(
    foundation: &ConeLirFoundation,
) -> Result<(), EntryProductionPlanBuildError> {
    let gateway_count = foundation
        .root_gateway_bodies()
        .map_err(EntryProductionPlanBuildError::CallableBodyKey)?
        .len();
    let failure_root_count = foundation.root_entry_failure_roots().len();
    let descriptor_symbols = foundation
        .symbol_requests()
        .iter()
        .filter(|request| matches!(request.key(), PersistentSymbolKey::RootEntryDescriptor(_)))
        .count();
    let descriptor_definitions = root_descriptor_definitions(foundation).len();
    if gateway_count + failure_root_count + descriptor_symbols + descriptor_definitions != 0 {
        return Err(EntryProductionPlanBuildError::LibraryRootArtifacts {
            gateway_count,
            failure_root_count,
            descriptor_symbols,
            descriptor_definitions,
        });
    }
    Ok(())
}

fn validate_root_only_entities(
    foundation: &ConeLirFoundation,
    expected_gateway: PersistentCallableBodyId,
    expected_failure_root: PersistentStaticStorageId,
) -> Result<(), EntryProductionPlanBuildError> {
    let gateways = foundation
        .root_gateway_bodies()
        .map_err(EntryProductionPlanBuildError::CallableBodyKey)?;
    if gateways.as_slice() != [expected_gateway] {
        return Err(EntryProductionPlanBuildError::RootGatewaySet {
            expected: expected_gateway,
            actual: gateways,
        });
    }
    let failure_roots = foundation.root_entry_failure_roots();
    if failure_roots.as_slice() != [expected_failure_root] {
        return Err(EntryProductionPlanBuildError::FailureRootSet {
            expected: expected_failure_root,
            actual: failure_roots,
        });
    }
    let descriptor_symbols = foundation
        .symbol_requests()
        .iter()
        .filter(|request| matches!(request.key(), PersistentSymbolKey::RootEntryDescriptor(_)))
        .count();
    let descriptor_definitions = root_descriptor_definitions(foundation);
    if descriptor_symbols != 1 || descriptor_definitions.len() != 1 {
        return Err(EntryProductionPlanBuildError::RootDescriptorSet {
            symbols: descriptor_symbols,
            definitions: descriptor_definitions.len(),
        });
    }
    Ok(())
}

fn root_descriptor_definitions(foundation: &ConeLirFoundation) -> Vec<ObjectDefinitionPlanId> {
    foundation
        .definition_plans()
        .iter()
        .filter_map(|record| {
            matches!(
                (record.key().owner(), record.key().definition_role()),
                (
                    ObjectDefinitionPlanOwner::Strong {
                        entity,
                        ..
                    },
                    ObjectDefinitionPlanRole::Strong(StrongDefinitionRole::RootEntryDescriptor)
                ) if matches!(entity.kind(), StrongDefinitionEntityKind::RootEntry(_))
            )
            .then_some(record.id())
        })
        .collect()
}

fn require_symbol(
    foundation: &ConeLirFoundation,
    key: PersistentSymbolKey,
) -> Result<PersistentSymbolRequest, EntryProductionPlanBuildError> {
    let request = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .map_err(EntryProductionPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(request)
        .then_some(request)
        .ok_or(EntryProductionPlanBuildError::MissingSymbol(request))
}

fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<ObjectDefinitionPlanId, EntryProductionPlanBuildError> {
    let key = ObjectDefinitionPlanKey::strong(foundation.producer(), entity, role)
        .map_err(EntryProductionPlanBuildError::Definition)?;
    let id = ObjectDefinitionPlanId::from_key(&key).map_err(EntryProductionPlanBuildError::Hash)?;
    foundation
        .definition_plans()
        .iter()
        .any(|record| record.id() == id && record.key() == &key)
        .then_some(id)
        .ok_or(EntryProductionPlanBuildError::MissingDefinition(id))
}

fn require_primary_atom(
    foundation: &ConeLirFoundation,
    plan: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, EntryProductionPlanBuildError> {
    let atoms = foundation
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == plan && record.key().role() == DefinitionAtomRole::Primary
        })
        .map(|record| record.id())
        .collect::<Vec<_>>();
    match atoms.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(EntryProductionPlanBuildError::PrimaryAtomSet {
            plan,
            actual: atoms,
        }),
    }
}

fn require_digest_node(
    digests: &StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&crate::DigestNodeV1, EntryProductionPlanBuildError> {
    let id = DigestNodeId::from_key(&key).map_err(EntryProductionPlanBuildError::Hash)?;
    digests
        .nodes()
        .iter()
        .find(|node| node.id() == id && node.key() == &key)
        .ok_or(EntryProductionPlanBuildError::MissingDigestNode(id))
}

fn require_patch(
    source: &crate::DigestNodeV1,
    target_definition: ObjectDefinitionPlanId,
    field: DigestSemanticFieldRole,
) -> Result<DigestPatchIntentId, EntryProductionPlanBuildError> {
    let key = DigestPatchIntentKey::new(
        source.id(),
        target_definition,
        DefinitionAtomRole::Primary,
        field,
    );
    let id = DigestPatchIntentId::from_key(&key).map_err(EntryProductionPlanBuildError::Hash)?;
    source
        .patch_intents()
        .iter()
        .any(|record| record.id() == id && record.key() == &key)
        .then_some(id)
        .ok_or(EntryProductionPlanBuildError::MissingPatch(
            source.id(),
            field,
        ))
}

fn require_callable_registration(
    registrations: &StrongRegistrationIdentitySurfaceV1,
    body: PersistentCallableBodyId,
) -> Result<(), EntryProductionPlanBuildError> {
    registrations
        .callables()
        .iter()
        .any(|entry| entry.semantic_id() == body)
        .then_some(())
        .ok_or(EntryProductionPlanBuildError::MissingCallableRegistration(
            body,
        ))
}

fn require_storage_registration(
    registrations: &StrongRegistrationIdentitySurfaceV1,
    storage: PersistentStaticStorageId,
) -> Result<(), EntryProductionPlanBuildError> {
    registrations
        .static_storages()
        .iter()
        .any(|entry| entry.semantic_id() == storage)
        .then_some(())
        .ok_or(EntryProductionPlanBuildError::MissingStorageRegistration(
            storage,
        ))
}

#[derive(Debug)]
pub enum EntryProductionPlanBuildError {
    Hash(HashError),
    Symbol(PersistentSymbolError),
    Definition(scoop_identity::ObjectDefinitionIdentityError),
    CallableBodyKey(scoop_wire::RuntimeDecodeError),
    ProducerMismatch {
        source: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    LibraryRootArtifacts {
        gateway_count: usize,
        failure_root_count: usize,
        descriptor_symbols: usize,
        descriptor_definitions: usize,
    },
    RootGatewaySet {
        expected: PersistentCallableBodyId,
        actual: Vec<PersistentCallableBodyId>,
    },
    FailureRootSet {
        expected: PersistentStaticStorageId,
        actual: Vec<PersistentStaticStorageId>,
    },
    RootDescriptorSet {
        symbols: usize,
        definitions: usize,
    },
    MissingCallableBody(PersistentCallableBodyId),
    MissingFailureRoot(PersistentStaticStorageId),
    MissingCallableRegistration(PersistentCallableBodyId),
    MissingStorageRegistration(PersistentStaticStorageId),
    MissingSymbol(PersistentSymbolRequest),
    MissingDefinition(ObjectDefinitionPlanId),
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingDigestNode(DigestNodeId),
    MissingPatch(DigestNodeId, DigestSemanticFieldRole),
}

impl fmt::Display for EntryProductionPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid entry production plan: {self:?}")
    }
}

impl std::error::Error for EntryProductionPlanBuildError {}

#[derive(Debug)]
pub enum EntryProductionPlanValidationError {
    Expected(EntryProductionPlanBuildError),
    Encode(scoop_wire::cbor::EncodeError),
    PlanMismatch,
}

impl fmt::Display for EntryProductionPlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid encoded entry production plan: {self:?}")
    }
}

impl std::error::Error for EntryProductionPlanValidationError {}
