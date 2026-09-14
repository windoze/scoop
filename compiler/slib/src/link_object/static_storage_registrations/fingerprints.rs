use std::fmt;

use scoop_identity::{
    DigestNodeId, PersistentStaticStorageId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    StaticStorageRelocationTableArtifactV1, StrongStaticStorageInitialArtifactPlanV1,
    StrongStaticStorageRegistrationPlanV1,
};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::physical::validate_objects;
use super::record::DESCRIPTOR_SIZE;
use super::{
    StrongStaticStorageRegistrationValidationError, VerifiedStrongStaticStorageRegistrationSetV1,
};
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalObjectRelocationV1, CanonicalStaticStorageTargetV1, ObjectDefinitionFingerprintInputV1,
};
use crate::link_object::{
    FinalUndefinedSymbolRequirementV1, ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1,
    StrongDefinitionOwnerV1,
};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";
const STORAGE_POINTER_OFFSET: usize = 160;
const SCAN_POINTER_OFFSET: usize = 192;
const TEMPLATE_POINTER_OFFSET: usize = 264;
const RELOCATION_TABLE_POINTER_OFFSET: usize = 280;
const POINTER_WIDTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageRegistrationObjectFingerprintV1 {
    storage: PersistentStaticStorageId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongStaticStorageRegistrationObjectFingerprintV1 {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn node(self) -> DigestNodeId {
        self.node
    }

    pub const fn fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.fingerprint
    }
}

/// Canonical root-registration object leaves derived from exact provisional
/// descriptor bytes and the verified typed relocation surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
    registrations: VerifiedStrongStaticStorageRegistrationSetV1,
    fingerprints: Vec<VerifiedStrongStaticStorageRegistrationObjectFingerprintV1>,
}

impl VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongStaticStorageRegistrationSetV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongStaticStorageRegistrationObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_static_storage_registration_object_fingerprints_v1(
    registrations: VerifiedStrongStaticStorageRegistrationSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    StrongStaticStorageRegistrationObjectFingerprintError,
> {
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongStaticStorageRegistrationObjectFingerprintError::ObjectValidation)?;
    if registrations.registrations().len() != registrations.plan().registrations().len() {
        return Err(StrongStaticStorageRegistrationObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for (verified, plan) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
    {
        let storage = plan.semantic().storage();
        if verified.storage() != storage {
            return Err(
                StrongStaticStorageRegistrationObjectFingerprintError::ProofCoverageMismatch,
            );
        }
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongStaticStorageRegistrationObjectFingerprintError::MissingObject(verified.member()),
        )?;
        let mut bytes = registration_record_bytes(object, verified.checked_offset(), storage)?;
        normalize_pointer(
            &mut bytes,
            STORAGE_POINTER_OFFSET,
            verified.storage_relocation().encoded_value(),
            storage,
        )?;
        normalize_pointer(
            &mut bytes,
            SCAN_POINTER_OFFSET,
            verified.scan_relocation().encoded_value(),
            storage,
        )?;
        normalize_pointer(
            &mut bytes,
            TEMPLATE_POINTER_OFFSET,
            verified.template_relocation().encoded_value(),
            storage,
        )?;
        normalize_pointer(
            &mut bytes,
            RELOCATION_TABLE_POINTER_OFFSET,
            verified.relocation_table_relocation().encoded_value(),
            storage,
        )?;
        let relocations = canonical_relocations(plan);
        let fingerprint =
            domain_separated_runtime_hash(
                OBJECT_DEFINITION_DOMAIN,
                &ObjectDefinitionFingerprintInputV1 {
                    bytes: &bytes,
                    relocations: &relocations,
                    direct_inputs: &[],
                },
            )
            .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
            .map_err(|source| {
                StrongStaticStorageRegistrationObjectFingerprintError::Hash { storage, source }
            })?;
        fingerprints.push(VerifiedStrongStaticStorageRegistrationObjectFingerprintV1 {
            storage,
            node: plan.registration_object_node(),
            fingerprint,
        });
    }

    Ok(
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
            registrations,
            fingerprints,
        },
    )
}

fn registration_record_bytes(
    object: &[u8],
    checked_offset: u64,
    storage: PersistentStaticStorageId,
) -> Result<Vec<u8>, StrongStaticStorageRegistrationObjectFingerprintError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongStaticStorageRegistrationObjectFingerprintError::RecordRange(storage))?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongStaticStorageRegistrationObjectFingerprintError::RecordRange(storage))?;
    object
        .get(start..end)
        .map(<[u8]>::to_vec)
        .ok_or(StrongStaticStorageRegistrationObjectFingerprintError::RecordRange(storage))
}

fn normalize_pointer(
    bytes: &mut [u8],
    offset: usize,
    expected: u64,
    storage: PersistentStaticStorageId,
) -> Result<(), StrongStaticStorageRegistrationObjectFingerprintError> {
    let slot = bytes
        .get_mut(offset..offset + POINTER_WIDTH)
        .ok_or(StrongStaticStorageRegistrationObjectFingerprintError::RecordRange(storage))?;
    let actual = u64::from_le_bytes(slot.try_into().expect("pointer slot is eight bytes"));
    if actual != expected {
        return Err(
            StrongStaticStorageRegistrationObjectFingerprintError::RelocationValueMismatch {
                storage,
                offset_within_atom: u16::try_from(offset)
                    .expect("static descriptor offset fits u16"),
                expected,
                actual,
            },
        );
    }
    slot.fill(0);
    Ok(())
}

fn canonical_relocations(
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> [CanonicalObjectRelocationV1; 4] {
    let storage = plan.semantic().storage();
    let storage_owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::static_storage(storage),
        StrongDefinitionRole::StaticStorage,
    )
    .expect("static storages are valid strong definition owners");
    let scan_owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::scan(plan.semantic().scan()),
        StrongDefinitionRole::ScanProgram,
    )
    .expect("scan programs are valid strong definition owners");
    let template = match plan.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            CanonicalStaticStorageTargetV1::EmptyTemplateSentinel
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue { .. } => {
            CanonicalStaticStorageTargetV1::InitialTemplate(storage)
        }
    };
    let relocation_table = match plan.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: StaticStorageRelocationTableArtifactV1::Defined { .. },
            ..
        } => CanonicalStaticStorageTargetV1::InitialRelocationTable(storage),
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
            ..
        } => CanonicalStaticStorageTargetV1::EmptyRelocationTableSentinel,
    };
    [
        CanonicalObjectRelocationV1::unsigned64(
            STORAGE_POINTER_OFFSET as u64,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong {
                owner: storage_owner,
            },
        ),
        CanonicalObjectRelocationV1::unsigned64(
            SCAN_POINTER_OFFSET as u64,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner: scan_owner },
        ),
        CanonicalObjectRelocationV1::static_storage_target(
            TEMPLATE_POINTER_OFFSET as u64,
            template,
        ),
        CanonicalObjectRelocationV1::static_storage_target(
            RELOCATION_TABLE_POINTER_OFFSET as u64,
            relocation_table,
        ),
    ]
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageRegistrationObjectFingerprintError {
    ObjectValidation(StrongStaticStorageRegistrationValidationError),
    ProofCoverageMismatch,
    MissingObject(SlibMemberId),
    RecordRange(PersistentStaticStorageId),
    RelocationValueMismatch {
        storage: PersistentStaticStorageId,
        offset_within_atom: u16,
        expected: u64,
        actual: u64,
    },
    Hash {
        storage: PersistentStaticStorageId,
        source: HashError,
    },
}

impl fmt::Display for StrongStaticStorageRegistrationObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong static-storage registration object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageRegistrationObjectFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
