//! Closed writer-side production plans for strong static-storage registrations.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest, StrongDefinitionEntity,
    StrongDefinitionRole,
};

use super::{
    StrongStaticStorageInitialStatePlanV1, StrongStaticStorageSemanticPlanSetV1,
    StrongStaticStorageSemanticPlanV1,
};
use crate::{
    ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1,
};

/// Every typed identity and digest writer required to emit one static-storage
/// registration, owned storage, and actual layout/scan definitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongStaticStorageRegistrationPlanV1 {
    semantic: StrongStaticStorageSemanticPlanV1,
    definition_owner: crate::RegistrationDefinitionOwner,
    registration_symbol: PersistentSymbolRequest,
    registration_definition_plan: ObjectDefinitionPlanId,
    registration_primary_atom: ObjectDefinitionAtomId,
    storage_definition_plan: ObjectDefinitionPlanId,
    storage_primary_atom: ObjectDefinitionAtomId,
    initial_artifacts: StrongStaticStorageInitialArtifactPlanV1,
    immortal_registration_symbols: Vec<PersistentSymbolRequest>,
    layout_symbol: PersistentSymbolRequest,
    layout_definition_plan: ObjectDefinitionPlanId,
    layout_primary_atom: ObjectDefinitionAtomId,
    scan_symbol: PersistentSymbolRequest,
    scan_definition_plan: ObjectDefinitionPlanId,
    scan_primary_atom: ObjectDefinitionAtomId,

    layout_fingerprint_node: DigestNodeId,
    scan_fingerprint_node: DigestNodeId,
    layout_fingerprint_patch: DigestPatchIntentId,
    scan_fingerprint_patch: DigestPatchIntentId,
}

impl StrongStaticStorageRegistrationPlanV1 {
    pub const fn definition_owner(&self) -> crate::RegistrationDefinitionOwner {
        self.definition_owner
    }

    pub const fn semantic(&self) -> &StrongStaticStorageSemanticPlanV1 {
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

    pub const fn storage_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.storage_definition_plan
    }

    pub const fn storage_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.storage_primary_atom
    }

    pub const fn initial_artifacts(&self) -> StrongStaticStorageInitialArtifactPlanV1 {
        self.initial_artifacts
    }

    pub fn immortal_registration_symbols(&self) -> &[PersistentSymbolRequest] {
        &self.immortal_registration_symbols
    }

    pub const fn layout_symbol(&self) -> PersistentSymbolRequest {
        self.layout_symbol
    }

    pub const fn layout_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.layout_definition_plan
    }

    pub const fn layout_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.layout_primary_atom
    }

    pub const fn scan_symbol(&self) -> PersistentSymbolRequest {
        self.scan_symbol
    }

    pub const fn scan_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.scan_definition_plan
    }

    pub const fn scan_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.scan_primary_atom
    }

    pub const fn layout_fingerprint_node(&self) -> DigestNodeId {
        self.layout_fingerprint_node
    }

    pub const fn scan_fingerprint_node(&self) -> DigestNodeId {
        self.scan_fingerprint_node
    }

    pub const fn layout_fingerprint_patch(&self) -> DigestPatchIntentId {
        self.layout_fingerprint_patch
    }

    pub const fn scan_fingerprint_patch(&self) -> DigestPatchIntentId {
        self.scan_fingerprint_patch
    }
}

/// The complete final-LIR static-storage set has exactly one
/// strong registration plan per storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongStaticStorageRegistrationPlanSetV1 {
    producer: ConeIdentity,
    registrations: Vec<StrongStaticStorageRegistrationPlanV1>,
}

impl StrongStaticStorageRegistrationPlanSetV1 {
    pub fn new(
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
        semantics: &StrongStaticStorageSemanticPlanSetV1,
        digests: &DigestFinalizationPlanV1,
    ) -> Result<Self, StrongStaticStorageRegistrationPlanBuildError> {
        if semantics.producer() != foundation.producer() {
            return Err(
                StrongStaticStorageRegistrationPlanBuildError::ProducerMismatch {
                    foundation: foundation.producer(),
                    semantics: semantics.producer(),
                },
            );
        }
        let expected = semantics
            .storages()
            .iter()
            .map(StrongStaticStorageSemanticPlanV1::storage)
            .collect::<Vec<_>>();
        let actual = identities
            .static_storages()
            .iter()
            .map(|identity| identity.semantic_id())
            .collect::<Vec<_>>();
        if expected != actual {
            return Err(StrongStaticStorageRegistrationPlanBuildError::StorageSet {
                expected,
                actual,
            });
        }
        for semantic in semantics.storages() {
            for relocation in semantic.initial_state().immortal_relocations() {
                let target = relocation.target();
                let actual = identities
                    .immortal_objects()
                    .iter()
                    .filter(|identity| identity.semantic_id() == target)
                    .count();
                if actual != 1 {
                    return Err(
                        StrongStaticStorageRegistrationPlanBuildError::ImmortalTargetRegistrationSet {
                            storage: semantic.storage(),
                            target,
                            actual,
                        },
                    );
                }
            }
        }

        let registrations = semantics
            .storages()
            .iter()
            .zip(identities.static_storages())
            .map(|(semantic, identity)| build_registration(foundation, semantic, identity, digests))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            producer: foundation.producer(),
            registrations,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[StrongStaticStorageRegistrationPlanV1] {
        &self.registrations
    }
}

fn build_registration(
    foundation: &ConeLirFoundation,
    semantic: &StrongStaticStorageSemanticPlanV1,
    identity: &crate::RegistrationIdentityV1<scoop_identity::PersistentStaticStorageId>,
    digests: &DigestFinalizationPlanV1,
) -> Result<StrongStaticStorageRegistrationPlanV1, StrongStaticStorageRegistrationPlanBuildError> {
    let storage = semantic.storage();
    if !foundation.contains_static_storage(storage) {
        return Err(StrongStaticStorageRegistrationPlanBuildError::MissingStorage(storage));
    }
    if semantic.value_layout().local().is_some() && !foundation.contains_layout(semantic.layout()) {
        return Err(
            StrongStaticStorageRegistrationPlanBuildError::MissingLayout {
                storage,
                layout: semantic.layout(),
            },
        );
    }
    if semantic.value_layout().local().is_some() && !foundation.contains_scan(semantic.scan()) {
        return Err(StrongStaticStorageRegistrationPlanBuildError::MissingScan {
            storage,
            scan: semantic.scan(),
        });
    }

    let storage_definition = require_definition(
        foundation,
        StrongDefinitionEntity::static_storage(storage),
        StrongDefinitionRole::StaticStorage,
    )?;
    let storage_primary_atom = require_primary_atom(foundation, storage_definition.id())?;
    let initial_artifacts =
        require_initial_artifacts(foundation, storage_definition.id(), semantic)?;
    require_symbol(foundation, semantic.symbol())?;
    let immortal_registration_symbols = semantic
        .initial_state()
        .immortal_relocations()
        .iter()
        .map(|relocation| {
            let definition = require_definition(
                foundation,
                StrongDefinitionEntity::immortal_object(relocation.target()),
                StrongDefinitionRole::ImmortalRegistration,
            )?;
            require_symbol_key(
                foundation,
                PersistentSymbolKey::ImmortalRegistration(relocation.target()),
                definition_linkage(definition),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let registration_definition = require_definition(
        foundation,
        StrongDefinitionEntity::static_storage(storage),
        StrongDefinitionRole::RootRegistration,
    )?;
    if registration_definition.id() != identity.definition_plan() {
        return Err(
            StrongStaticStorageRegistrationPlanBuildError::RegistrationDefinitionMismatch {
                storage,
                expected: registration_definition.id(),
                actual: identity.definition_plan(),
            },
        );
    }
    let registration_primary_atom = require_primary_atom(foundation, registration_definition.id())?;
    let registration_symbol = require_symbol_key(
        foundation,
        PersistentSymbolKey::RootRegistration(storage),
        identity.owner().linkage(),
    )?;

    let (
        layout_definition_plan,
        layout_primary_atom,
        layout_symbol,
        scan_definition_plan,
        scan_primary_atom,
        scan_symbol,
    ) = match semantic.value_layout() {
        crate::StaticStorageLayout::External(value) => {
            let layout = value.identity().physical_definition();
            let scan = value.scan_definition();
            (
                layout.definition(),
                layout.primary(),
                layout.symbol(),
                scan.definition(),
                scan.primary(),
                scan.symbol(),
            )
        }
        crate::StaticStorageLayout::Local(_) => {
            let layout_definition = require_definition(
                foundation,
                StrongDefinitionEntity::layout(semantic.layout()),
                StrongDefinitionRole::Layout,
            )?;
            let layout_primary_atom = require_primary_atom(foundation, layout_definition.id())?;
            let layout_symbol = require_symbol_key(
                foundation,
                PersistentSymbolKey::Layout(semantic.layout()),
                definition_linkage(layout_definition),
            )?;

            let scan_definition = require_definition(
                foundation,
                StrongDefinitionEntity::scan(semantic.scan()),
                StrongDefinitionRole::ScanProgram,
            )?;
            let scan_primary_atom = require_primary_atom(foundation, scan_definition.id())?;
            let scan_symbol = require_symbol_key(
                foundation,
                PersistentSymbolKey::ScanProgram(semantic.scan()),
                definition_linkage(scan_definition),
            )?;

            (
                layout_definition.id(),
                layout_primary_atom,
                layout_symbol,
                scan_definition.id(),
                scan_primary_atom,
                scan_symbol,
            )
        }
    };

    let layout_fingerprint =
        require_digest_node(digests, DigestNodeKey::layout(semantic.layout()))?;
    let scan_fingerprint = require_digest_node(digests, DigestNodeKey::scan(semantic.scan()))?;
    let layout_fingerprint_patch = require_patch(
        layout_fingerprint,
        DigestPatchIntentKey::new(
            layout_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        ),
    )?;
    let scan_fingerprint_patch = require_patch(
        scan_fingerprint,
        DigestPatchIntentKey::new(
            scan_fingerprint.id(),
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Scan,
        ),
    )?;

    Ok(StrongStaticStorageRegistrationPlanV1 {
        semantic: semantic.clone(),
        definition_owner: identity.owner(),
        registration_symbol,
        registration_definition_plan: registration_definition.id(),
        registration_primary_atom,
        storage_definition_plan: storage_definition.id(),
        storage_primary_atom,
        initial_artifacts,
        immortal_registration_symbols,
        layout_symbol,
        layout_definition_plan,
        layout_primary_atom,
        scan_symbol,
        scan_definition_plan,
        scan_primary_atom,

        layout_fingerprint_node: layout_fingerprint.id(),
        scan_fingerprint_node: scan_fingerprint.id(),
        layout_fingerprint_patch,
        scan_fingerprint_patch,
    })
}

fn require_initial_artifacts(
    foundation: &ConeLirFoundation,
    storage_definition: ObjectDefinitionPlanId,
    semantic: &StrongStaticStorageSemanticPlanV1,
) -> Result<StrongStaticStorageInitialArtifactPlanV1, StrongStaticStorageRegistrationPlanBuildError>
{
    let storage = semantic.storage();
    let template_key = ObjectDefinitionAtomKey::new(
        storage_definition,
        DefinitionAtomRole::AddressTakenConstant,
        scoop_identity::DefinitionAtomSubkey::StaticStorage(storage),
    );
    let relocation_key = ObjectDefinitionAtomKey::new(
        storage_definition,
        DefinitionAtomRole::RuntimeRecord,
        scoop_identity::DefinitionAtomSubkey::StaticStorage(storage),
    );
    let (mut expected, initial_artifacts) = match semantic.initial_state() {
        StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit => (
            Vec::new(),
            StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit,
        ),
        StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            immortal_relocations,
            ..
        } => {
            let template_atom = require_atom(foundation, &template_key)?;
            let relocation_table = if immortal_relocations.is_empty() {
                StaticStorageRelocationTableArtifactV1::SharedEmptySentinel
            } else {
                StaticStorageRelocationTableArtifactV1::Defined {
                    atom: require_atom(foundation, &relocation_key)?,
                }
            };
            let mut expected = vec![template_key];
            if !immortal_relocations.is_empty() {
                expected.push(relocation_key);
            }
            (
                expected,
                StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
                    template_atom,
                    relocation_table,
                },
            )
        }
    };
    expected.sort_unstable();
    let mut actual = foundation
        .definition_atoms_for_plan(storage_definition)
        .filter(|record| record.key().role() != DefinitionAtomRole::Primary)
        .map(|record| record.key().clone())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != expected {
        return Err(
            StrongStaticStorageRegistrationPlanBuildError::InitialArtifactSet {
                storage,
                expected,
                actual,
            },
        );
    }
    Ok(initial_artifacts)
}

fn require_atom(
    foundation: &ConeLirFoundation,
    key: &ObjectDefinitionAtomKey,
) -> Result<ObjectDefinitionAtomId, StrongStaticStorageRegistrationPlanBuildError> {
    foundation
        .definition_atoms_for_plan(key.plan())
        .find(|record| record.key() == key)
        .map(|record| record.id())
        .ok_or_else(|| {
            StrongStaticStorageRegistrationPlanBuildError::MissingAtom(Box::new(key.clone()))
        })
}

fn require_definition(
    foundation: &ConeLirFoundation,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<&crate::DefinitionPlanRecord, StrongStaticStorageRegistrationPlanBuildError> {
    foundation
        .definition_for(entity, role)
        .ok_or(StrongStaticStorageRegistrationPlanBuildError::MissingDefinition { entity, role })
}

fn require_primary_atom(
    foundation: &ConeLirFoundation,
    definition: ObjectDefinitionPlanId,
) -> Result<ObjectDefinitionAtomId, StrongStaticStorageRegistrationPlanBuildError> {
    let atoms = foundation
        .definition_atoms_for_plan(definition)
        .filter(|record| record.key().role() == DefinitionAtomRole::Primary)
        .map(|record| record.id())
        .collect::<Vec<_>>();
    match atoms.as_slice() {
        [atom] => Ok(*atom),
        _ => Err(
            StrongStaticStorageRegistrationPlanBuildError::PrimaryAtomSet {
                plan: definition,
                actual: atoms,
            },
        ),
    }
}

fn require_symbol_key(
    foundation: &ConeLirFoundation,
    key: PersistentSymbolKey,
    linkage: LinkageClass,
) -> Result<PersistentSymbolRequest, StrongStaticStorageRegistrationPlanBuildError> {
    let symbol = PersistentSymbolRequest::new(key, linkage)
        .map_err(StrongStaticStorageRegistrationPlanBuildError::Symbol)?;
    require_symbol(foundation, symbol)?;
    Ok(symbol)
}

fn definition_linkage(record: &crate::DefinitionPlanRecord) -> LinkageClass {
    match record.key().owner() {
        scoop_identity::ObjectDefinitionPlanOwner::Strong { .. } => LinkageClass::ConeStrong,
        scoop_identity::ObjectDefinitionPlanOwner::Odr { .. } => LinkageClass::OdrWeak,
    }
}

fn require_symbol(
    foundation: &ConeLirFoundation,
    symbol: PersistentSymbolRequest,
) -> Result<(), StrongStaticStorageRegistrationPlanBuildError> {
    foundation
        .contains_symbol_request(symbol)
        .then_some(())
        .ok_or(StrongStaticStorageRegistrationPlanBuildError::MissingSymbol(symbol))
}

fn require_digest_node(
    digests: &DigestFinalizationPlanV1,
    key: DigestNodeKey,
) -> Result<&DigestNodeV1, StrongStaticStorageRegistrationPlanBuildError> {
    digests
        .nodes()
        .iter()
        .find(|node| node.key() == &key)
        .ok_or(StrongStaticStorageRegistrationPlanBuildError::MissingDigestNode(key))
}

fn require_patch(
    node: &DigestNodeV1,
    expected: DigestPatchIntentKey,
) -> Result<DigestPatchIntentId, StrongStaticStorageRegistrationPlanBuildError> {
    node.patch_intents()
        .iter()
        .find(|patch| patch.key() == &expected)
        .map(|patch| patch.id())
        .ok_or(
            StrongStaticStorageRegistrationPlanBuildError::MissingPatch {
                node: node.id(),
                expected: Box::new(expected),
            },
        )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageObjectLeafV1 {
    Registration,
    Storage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageRelocationTableArtifactV1 {
    SharedEmptySentinel,
    Defined { atom: ObjectDefinitionAtomId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageInitialArtifactPlanV1 {
    ZeroedForRuntimeUnit,
    EncodedStaticValue {
        template_atom: ObjectDefinitionAtomId,
        relocation_table: StaticStorageRelocationTableArtifactV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageRegistrationPlanBuildError {
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    ProducerMismatch {
        foundation: ConeIdentity,
        semantics: ConeIdentity,
    },
    StorageSet {
        expected: Vec<scoop_identity::PersistentStaticStorageId>,
        actual: Vec<scoop_identity::PersistentStaticStorageId>,
    },
    ImmortalTargetRegistrationSet {
        storage: scoop_identity::PersistentStaticStorageId,
        target: scoop_identity::PersistentImmortalObjectId,
        actual: usize,
    },
    MissingStorage(scoop_identity::PersistentStaticStorageId),
    MissingLayout {
        storage: scoop_identity::PersistentStaticStorageId,
        layout: scoop_identity::PersistentLayoutId,
    },
    MissingScan {
        storage: scoop_identity::PersistentStaticStorageId,
        scan: scoop_identity::PersistentScanId,
    },
    MissingDefinition {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    MissingAtom(Box<ObjectDefinitionAtomKey>),
    InitialArtifactSet {
        storage: scoop_identity::PersistentStaticStorageId,
        expected: Vec<ObjectDefinitionAtomKey>,
        actual: Vec<ObjectDefinitionAtomKey>,
    },
    RegistrationDefinitionMismatch {
        storage: scoop_identity::PersistentStaticStorageId,
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
        storage: scoop_identity::PersistentStaticStorageId,
        expected: DigestNodeId,
        actual: DigestNodeId,
    },
    DirectInputs {
        node: DigestNodeId,
        expected: Vec<DigestInputRefV1>,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectLeafInputs {
        leaf: StaticStorageObjectLeafV1,
        node: DigestNodeId,
        actual: Vec<DigestInputRefV1>,
    },
    ObjectLeafPatches {
        leaf: StaticStorageObjectLeafV1,
        node: DigestNodeId,
        actual: Vec<DigestPatchIntentKey>,
    },
    PatchSet {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
        actual: Vec<DigestPatchIntentKey>,
    },
    MissingPatch {
        node: DigestNodeId,
        expected: Box<DigestPatchIntentKey>,
    },
}

impl fmt::Display for StrongStaticStorageRegistrationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong static-storage registration plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageRegistrationPlanBuildError {}

#[cfg(test)]
mod tests;
