use std::fmt;

use scoop_identity::{DefinitionAtomRole, DigestNodeId, PersistentInitializationUnitId};
use scoop_lir::StrongInitializationUnitRegistrationPlan;
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::physical::validate_objects;
use super::record::DESCRIPTOR_SIZE;
use super::{
    StrongInitializationRegistrationValidationError, VerifiedStrongInitializationRegistrationSetV1,
};
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalObjectRelocationV1, ObjectDefinitionFingerprintInputV1,
    ObjectDefinitionRelocationFailureV1,
};
use crate::link_object::{ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationRegistrationObjectFingerprintV1 {
    unit: PersistentInitializationUnitId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongInitializationRegistrationObjectFingerprintV1 {
    pub const fn unit(self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub const fn node(self) -> DigestNodeId {
        self.node
    }

    pub const fn fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.fingerprint
    }
}

/// Canonical initialization-registration object leaves derived from exact
/// provisional records and their complete verified relocation surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<
    D = PersistentInitializationUnitId,
> {
    registrations: VerifiedStrongInitializationRegistrationSetV1<D>,
    fingerprints: Vec<VerifiedStrongInitializationRegistrationObjectFingerprintV1>,
}

pub type VerifiedStrongInitializationRegistrationObjectFingerprintSetV2 =
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<
        scoop_lir::StrongInitializationDependencyRefV2,
    >;

impl<D> VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<D> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongInitializationRegistrationSetV1<D> {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongInitializationRegistrationObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_initialization_registration_object_fingerprints_v1(
    registrations: VerifiedStrongInitializationRegistrationSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    StrongInitializationRegistrationObjectFingerprintError,
> {
    compute_strong_initialization_registration_object_fingerprints(registrations, scoop_objects)
}

pub fn compute_strong_initialization_registration_object_fingerprints_v2(
    registrations: super::VerifiedStrongInitializationRegistrationSetV2,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV2,
    StrongInitializationRegistrationObjectFingerprintError,
> {
    compute_strong_initialization_registration_object_fingerprints(registrations, scoop_objects)
}

fn compute_strong_initialization_registration_object_fingerprints<D>(
    registrations: VerifiedStrongInitializationRegistrationSetV1<D>,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<D>,
    StrongInitializationRegistrationObjectFingerprintError,
> {
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongInitializationRegistrationObjectFingerprintError::ObjectValidation)?;
    if registrations.registrations().len() != registrations.plan().registrations().len() {
        return Err(StrongInitializationRegistrationObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for (verified, plan) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
    {
        let unit = plan.semantic().unit();
        if verified.unit() != unit {
            return Err(
                StrongInitializationRegistrationObjectFingerprintError::ProofCoverageMismatch,
            );
        }
        let object = objects.get(&verified.member()).copied().ok_or(
            StrongInitializationRegistrationObjectFingerprintError::MissingObject(
                verified.member(),
            ),
        )?;
        let mut bytes = registration_bytes(object, verified.checked_offset(), unit)?;
        let relocations = canonical_relocations(plan, verified);
        for relocation in &relocations {
            relocation.normalize_bytes(&mut bytes).map_err(|kind| {
                StrongInitializationRegistrationObjectFingerprintError::Relocation { unit, kind }
            })?;
        }
        let fingerprint = domain_separated_runtime_hash(
            OBJECT_DEFINITION_DOMAIN,
            &ObjectDefinitionFingerprintInputV1 {
                bytes: &bytes,
                relocations: &relocations,
                direct_inputs: &[],
            },
        )
        .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
        .map_err(|source| {
            StrongInitializationRegistrationObjectFingerprintError::Hash { unit, source }
        })?;
        fingerprints.push(
            VerifiedStrongInitializationRegistrationObjectFingerprintV1 {
                unit,
                node: plan.registration_object_node(),
                fingerprint,
            },
        );
    }

    Ok(
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
            registrations,
            fingerprints,
        },
    )
}

fn registration_bytes(
    object: &[u8],
    checked_offset: u64,
    unit: PersistentInitializationUnitId,
) -> Result<Vec<u8>, StrongInitializationRegistrationObjectFingerprintError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongInitializationRegistrationObjectFingerprintError::RecordRange(unit))?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongInitializationRegistrationObjectFingerprintError::RecordRange(unit))?;
    object
        .get(start..end)
        .map(<[u8]>::to_vec)
        .ok_or(StrongInitializationRegistrationObjectFingerprintError::RecordRange(unit))
}

fn canonical_relocations<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    verified: &super::VerifiedStrongInitializationRegistrationV1,
) -> Vec<CanonicalObjectRelocationV1> {
    use super::super::registration_identity::canonical_local_relocation;
    let mut relocations = vec![
        CanonicalObjectRelocationV1::owning_associated_atom_offset(
            160,
            plan.diagnostic_atom(),
            DefinitionAtomRole::AddressTakenConstant,
            0,
        ),
        canonical_local_relocation(verified.registration_cell_relocation()),
        canonical_local_relocation(verified.registration_storage_relocation()),
        canonical_local_relocation(verified.registration_failure_relocation()),
        canonical_local_relocation(verified.registration_initializer_relocation()),
        canonical_local_relocation(verified.registration_ensure_relocation()),
    ];
    if let Some(gateway) = verified.registration_gateway_relocation() {
        relocations.push(canonical_local_relocation(gateway));
    }
    relocations
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationRegistrationObjectFingerprintError {
    ObjectValidation(StrongInitializationRegistrationValidationError),
    ProofCoverageMismatch,
    MissingObject(SlibMemberId),
    RecordRange(PersistentInitializationUnitId),
    Relocation {
        unit: PersistentInitializationUnitId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash {
        unit: PersistentInitializationUnitId,
        source: HashError,
    },
}

impl fmt::Display for StrongInitializationRegistrationObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong initialization registration object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationRegistrationObjectFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}
