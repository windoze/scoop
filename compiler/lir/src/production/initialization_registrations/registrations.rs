//! Closed writer-side production plans for strong initialization registrations.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionIdentityError, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, PersistentInitializationUnitId, PersistentStaticStorageId,
    PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest, StrongDefinitionEntity,
    StrongDefinitionRole,
};

use super::{
    StrongInitializationSchedulePlanV1, StrongInitializationUnitSemanticPlanSetV1,
    StrongInitializationUnitSemanticPlanV1,
};
use crate::{
    DigestInputRefV1, DigestNodeV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongRegistrationIdentitySurfaceV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationStaticStorageRefPlanV1 {
    storage: PersistentStaticStorageId,
    registration_symbol: PersistentSymbolRequest,
    registration_definition_plan: ObjectDefinitionPlanId,
    registration_primary_atom: ObjectDefinitionAtomId,
    registration_fingerprint_node: DigestNodeId,
}

impl StrongInitializationStaticStorageRefPlanV1 {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn registration_symbol(self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn registration_definition_plan(self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationCallableRefPlanV1 {
    body: PersistentCallableBodyId,
    entry_symbol: PersistentSymbolRequest,
    registration_symbol: PersistentSymbolRequest,
    body_definition_plan: ObjectDefinitionPlanId,
    body_primary_atom: ObjectDefinitionAtomId,
    body_definition_node: DigestNodeId,
    registration_definition_plan: ObjectDefinitionPlanId,
    registration_primary_atom: ObjectDefinitionAtomId,
    registration_fingerprint_node: DigestNodeId,
}

impl StrongInitializationCallableRefPlanV1 {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn entry_symbol(self) -> PersistentSymbolRequest {
        self.entry_symbol
    }

    pub const fn registration_symbol(self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn body_definition_plan(self) -> ObjectDefinitionPlanId {
        self.body_definition_plan
    }

    pub const fn body_primary_atom(self) -> ObjectDefinitionAtomId {
        self.body_primary_atom
    }

    pub const fn body_definition_node(self) -> DigestNodeId {
        self.body_definition_node
    }

    pub const fn registration_definition_plan(self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationRegistrationSchedulePlanV1 {
    EagerStartup {
        gateway: Box<StrongInitializationCallableRefPlanV1>,
        gateway_definition_patch: DigestPatchIntentId,
    },
    LazyAccess,
}

impl StrongInitializationRegistrationSchedulePlanV1 {
    pub fn gateway(&self) -> Option<&StrongInitializationCallableRefPlanV1> {
        match self {
            Self::EagerStartup { gateway, .. } => Some(gateway),
            Self::LazyAccess => None,
        }
    }

    pub const fn gateway_definition_patch(&self) -> Option<DigestPatchIntentId> {
        match self {
            Self::EagerStartup {
                gateway_definition_patch,
                ..
            } => Some(*gateway_definition_patch),
            Self::LazyAccess => None,
        }
    }
}

/// Every typed definition, referenced registration, and digest writer needed
/// to emit one strong initialization-unit registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitRegistrationPlanV1 {
    semantic: StrongInitializationUnitSemanticPlanV1,
    registration_symbol: PersistentSymbolRequest,
    registration_definition_plan: ObjectDefinitionPlanId,
    registration_primary_atom: ObjectDefinitionAtomId,
    cell_symbol: PersistentSymbolRequest,
    cell_definition_plan: ObjectDefinitionPlanId,
    cell_primary_atom: ObjectDefinitionAtomId,
    descriptor_symbol: PersistentSymbolRequest,
    descriptor_definition_plan: ObjectDefinitionPlanId,
    descriptor_primary_atom: ObjectDefinitionAtomId,
    storage: StrongInitializationStaticStorageRefPlanV1,
    failure_root: StrongInitializationStaticStorageRefPlanV1,
    initializer: StrongInitializationCallableRefPlanV1,
    ensure: StrongInitializationCallableRefPlanV1,
    schedule: StrongInitializationRegistrationSchedulePlanV1,
    registration_object_node: DigestNodeId,
    cell_definition_node: DigestNodeId,
    descriptor_definition_node: DigestNodeId,
    registration_fingerprint_node: DigestNodeId,
    registration_definition_patch: DigestPatchIntentId,
}

impl StrongInitializationUnitRegistrationPlanV1 {
    pub const fn semantic(&self) -> &StrongInitializationUnitSemanticPlanV1 {
        &self.semantic
    }

    pub const fn registration_symbol(&self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn registration_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn cell_symbol(&self) -> PersistentSymbolRequest {
        self.cell_symbol
    }

    pub const fn cell_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.cell_definition_plan
    }

    pub const fn cell_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.cell_primary_atom
    }

    pub const fn descriptor_symbol(&self) -> PersistentSymbolRequest {
        self.descriptor_symbol
    }

    pub const fn descriptor_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.descriptor_definition_plan
    }

    pub const fn descriptor_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.descriptor_primary_atom
    }

    pub const fn storage(&self) -> StrongInitializationStaticStorageRefPlanV1 {
        self.storage
    }

    pub const fn failure_root(&self) -> StrongInitializationStaticStorageRefPlanV1 {
        self.failure_root
    }

    pub const fn initializer(&self) -> StrongInitializationCallableRefPlanV1 {
        self.initializer
    }

    pub const fn ensure(&self) -> StrongInitializationCallableRefPlanV1 {
        self.ensure
    }

    pub const fn schedule(&self) -> &StrongInitializationRegistrationSchedulePlanV1 {
        &self.schedule
    }

    pub const fn registration_object_node(&self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn cell_definition_node(&self) -> DigestNodeId {
        self.cell_definition_node
    }

    pub const fn descriptor_definition_node(&self) -> DigestNodeId {
        self.descriptor_definition_node
    }

    pub const fn registration_fingerprint_node(&self) -> DigestNodeId {
        self.registration_fingerprint_node
    }

    pub const fn registration_definition_patch(&self) -> DigestPatchIntentId {
        self.registration_definition_patch
    }
}

/// Proof that the complete final-LIR initialization-unit set has exactly one
/// strong registration production plan per unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongInitializationUnitRegistrationPlanV1>,
}

impl StrongInitializationUnitRegistrationPlanSetV1 {
    pub fn new(
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        semantics: &StrongInitializationUnitSemanticPlanSetV1,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongInitializationUnitRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(
                StrongInitializationUnitRegistrationPlanBuildError::ProducerMismatch {
                    foundation: foundation.producer(),
                    semantics: semantics.producer(),
                },
            );
        }
        let expected = semantics
            .units()
            .iter()
            .map(StrongInitializationUnitSemanticPlanV1::unit)
            .collect::<Vec<_>>();
        let actual = identities
            .initialization_units()
            .iter()
            .map(|identity| identity.semantic_id())
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(
                StrongInitializationUnitRegistrationPlanBuildError::UnitSet { expected, actual },
            );
        }

        let registrations = semantics
            .units()
            .iter()
            .zip(identities.initialization_units())
            .map(|(semantic, identity)| {
                build_registration(foundation, identities, semantic, identity, digests)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongInitializationUnitRegistrationPlanV1] {
        &self.registrations
    }
}

fn build_registration(
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    semantic: &StrongInitializationUnitSemanticPlanV1,
    identity: &crate::StrongRegistrationIdentityV1<PersistentInitializationUnitId>,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<
    StrongInitializationUnitRegistrationPlanV1,
    StrongInitializationUnitRegistrationPlanBuildError,
> {
    let unit = semantic.unit();
    let entity = StrongDefinitionEntity::initialization_unit(unit);
    let registration_definition = require_definition(
        foundation,
        entity,
        StrongDefinitionRole::InitializationRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                unit,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let registration_primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let registration_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::InitializationRegistration(unit),
    )?;

    let cell_definition =
        require_definition(foundation, entity, StrongDefinitionRole::InitializationCell)?;
    let cell_primary_atom = require_primary_atom(foundation, cell_definition.id())?;
    let cell_symbol = require_symbol(foundation, PersistentSymbolKey::InitializationCell(unit))?;

    let descriptor_definition = require_definition(
        foundation,
        entity,
        StrongDefinitionRole::InitializationDescriptor,
    )?;
    let descriptor_primary_atom = require_primary_atom(foundation, descriptor_definition.id())?;
    let descriptor_symbol = require_symbol(
        foundation,
        PersistentSymbolKey::InitializationDescriptor(unit),
    )?;

    let storage = require_static_storage(foundation, identities, semantic.storage(), digests)?;
    let failure_root =
        require_static_storage(foundation, identities, semantic.failure_root(), digests)?;
    let initializer = require_callable(foundation, identities, semantic.initializer(), digests)?;
    let ensure = require_callable(foundation, identities, semantic.ensure(), digests)?;

    let registration_object = require_leaf_object_node(
        digests,
        registration_primary_atom,
        InitializationObjectLeafV1::Registration,
    )?;
    let cell_object =
        require_leaf_object_node(digests, cell_primary_atom, InitializationObjectLeafV1::Cell)?;
    let descriptor_object = require_leaf_object_node(
        digests,
        descriptor_primary_atom,
        InitializationObjectLeafV1::Descriptor,
    )?;
    let registration_fingerprint = require_digest_node(
        digests,
        DigestNodeKey::strong_registration(registration_definition.id()),
    )?;
    if registration_fingerprint.id() != identity.fingerprint_node() {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::RegistrationDigestMismatch {
                unit,
                expected: registration_fingerprint.id(),
                actual: identity.fingerprint_node(),
            },
        );
    }

    let mut expected_inputs = vec![
        DigestInputRefV1::from_node(registration_object),
        DigestInputRefV1::from_node(cell_object),
        DigestInputRefV1::from_node(descriptor_object),
    ];
    let schedule = match semantic.schedule() {
        StrongInitializationSchedulePlanV1::EagerStartup { gateway } => {
            let gateway = require_callable(foundation, identities, gateway, digests)?;
            let gateway_node = require_digest_node(
                digests,
                DigestNodeKey::object_definition(gateway.body_primary_atom()),
            )?;
            expected_inputs.push(DigestInputRefV1::from_node(gateway_node));
            let expected_patch = DigestPatchIntentKey::new(
                gateway_node.id(),
                registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::GatewayDefinition,
            );
            let gateway_definition_patch = require_exact_gateway_patch_set(
                digests,
                registration_definition.id(),
                Some(expected_patch),
            )?;
            StrongInitializationRegistrationSchedulePlanV1::EagerStartup {
                gateway: Box::new(gateway),
                gateway_definition_patch: gateway_definition_patch
                    .expect("the eager gateway patch was required"),
            }
        }
        StrongInitializationSchedulePlanV1::LazyAccess => {
            require_exact_gateway_patch_set(digests, registration_definition.id(), None)?;
            StrongInitializationRegistrationSchedulePlanV1::LazyAccess
        }
    };
    expected_inputs.sort_unstable();
    if registration_fingerprint.direct_inputs() != expected_inputs {
        return Err(
            StrongInitializationUnitRegistrationPlanBuildError::DirectInputs {
                node: registration_fingerprint.id(),
                expected: expected_inputs,
                actual: registration_fingerprint.direct_inputs().to_vec(),
            },
        );
    }
    let registration_definition_patch = require_only_patch(
        registration_fingerprint,
        DigestPatchIntentKey::new(
            registration_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        ),
    )?;

    Ok(StrongInitializationUnitRegistrationPlanV1 {
        semantic: semantic.clone(),
        registration_symbol,
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom,
        cell_symbol,
        cell_definition_plan: cell_definition.id(),
        cell_primary_atom,
        descriptor_symbol,
        descriptor_definition_plan: descriptor_definition.id(),
        descriptor_primary_atom,
        storage,
        failure_root,
        initializer,
        ensure,
        schedule,
        registration_object_node: registration_object.id(),
        cell_definition_node: cell_object.id(),
        descriptor_definition_node: descriptor_object.id(),
        registration_fingerprint_node: registration_fingerprint.id(),
        registration_definition_patch,
    })
}

fn require_static_storage(
    foundation: &OdrFreeLirFoundation,
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
        registration_symbol: symbol,
        registration_definition_plan: definition.id(),
        registration_primary_atom: primary,
        registration_fingerprint_node: fingerprint.id(),
    })
}

fn require_callable(
    foundation: &OdrFreeLirFoundation,
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

fn require_definition(
    foundation: &OdrFreeLirFoundation,
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

fn require_primary_atom(
    foundation: &OdrFreeLirFoundation,
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

fn require_symbol(
    foundation: &OdrFreeLirFoundation,
    key: PersistentSymbolKey,
) -> Result<PersistentSymbolRequest, StrongInitializationUnitRegistrationPlanBuildError> {
    let symbol = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .map_err(StrongInitializationUnitRegistrationPlanBuildError::Symbol)?;
    foundation
        .contains_symbol_request(symbol)
        .then_some(symbol)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingSymbol(symbol))
}

fn require_digest_node(
    digests: &StrongDigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongInitializationUnitRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongInitializationUnitRegistrationPlanBuildError::MissingDigestNode(key))
}

fn require_leaf_object_node(
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

fn require_only_patch(
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

fn require_exact_gateway_patch_set(
    digests: &StrongDigestFinalizationPlanV1,
    registration: ObjectDefinitionPlanId,
    expected: Option<DigestPatchIntentKey>,
) -> Result<Option<DigestPatchIntentId>, StrongInitializationUnitRegistrationPlanBuildError> {
    let actual = digests
        .nodes()
        .iter()
        .flat_map(|node| node.patch_intents())
        .filter(|patch| {
            patch.key().target_definition() == registration
                && patch.key().atom_role() == DefinitionAtomRole::Primary
                && patch.key().semantic_field_role() == DigestSemanticFieldRole::GatewayDefinition
        })
        .collect::<Vec<_>>();
    match (expected, actual.as_slice()) {
        (None, []) => Ok(None),
        (Some(expected), [actual]) if actual.key() == &expected => Ok(Some(actual.id())),
        (expected, actual) => Err(
            StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet {
                registration,
                expected: expected.map(Box::new),
                actual: actual.iter().map(|patch| *patch.key()).collect(),
            },
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationObjectLeafV1 {
    Registration,
    Cell,
    Descriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationReferencedRegistrationV1 {
    StaticStorage(PersistentStaticStorageId),
    Callable(PersistentCallableBodyId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationUnitRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    UnitSet {
        expected: Vec<PersistentInitializationUnitId>,
        actual: Vec<PersistentInitializationUnitId>,
    },
    MissingStaticStorage(PersistentStaticStorageId),
    MissingStaticStorageRegistration(PersistentStaticStorageId),
    MissingCallableBody(PersistentCallableBodyId),
    MissingCallableRegistration(PersistentCallableBodyId),
    MissingDefinition(Box<ObjectDefinitionPlanKey>),
    RegistrationDefinitionMismatch {
        unit: PersistentInitializationUnitId,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    ReferencedRegistrationDefinitionMismatch {
        registration: InitializationReferencedRegistrationV1,
        expected: ObjectDefinitionPlanId,
        actual: ObjectDefinitionPlanId,
    },
    PrimaryAtomSet {
        plan: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingDigestNode(DigestNodeKey),
    RegistrationDigestMismatch {
        unit: PersistentInitializationUnitId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    ReferencedRegistrationDigestMismatch {
        registration: InitializationReferencedRegistrationV1,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectLeafInputs {
        leaf: InitializationObjectLeafV1,
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectLeafPatches {
        leaf: InitializationObjectLeafV1,
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
    GatewayPatchSet {
        registration: ObjectDefinitionPlanId,
        expected: Option<Box<DigestPatchIntentKey>>,
        actual: Vec<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongInitializationUnitRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong initialization-unit registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationUnitRegistrationPlanBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinitionIdentity(source) => Some(source),
            Self::Symbol(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
