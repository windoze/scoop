use std::fmt;

use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, PersistentInitializationUnitId, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::StrongInitializationUnitRegistrationPlan;
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::StrongInitializationRegistrationValidationError;
use super::fingerprints::VerifiedStrongInitializationRegistrationObjectFingerprintSetV1;
use super::physical::validate_objects;
use super::record::{CELL_SIZE, COORDINATOR_SIZE};
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalAssociatedObjectAtomV1, CanonicalObjectRelocationV1,
    ObjectDefinitionFingerprintInputV1, ObjectDefinitionLeafWithAssociatedAtomsInputV1,
    ObjectDefinitionRelocationFailureV1,
};
use crate::link_object::{
    FinalUndefinedSymbolRequirementV1, ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1,
    StrongDefinitionOwnerV1,
};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationDefinitionFingerprintV1 {
    unit: PersistentInitializationUnitId,
    cell_node: DigestNodeId,
    cell: ObjectDefinitionFingerprintV1,
    descriptor_node: DigestNodeId,
    descriptor: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongInitializationDefinitionFingerprintV1 {
    pub const fn unit(self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub const fn cell_node(self) -> DigestNodeId {
        self.cell_node
    }

    pub const fn cell(self) -> ObjectDefinitionFingerprintV1 {
        self.cell
    }

    pub const fn descriptor_node(self) -> DigestNodeId {
        self.descriptor_node
    }

    pub const fn descriptor(self) -> ObjectDefinitionFingerprintV1 {
        self.descriptor
    }
}

/// Canonical initialization cell and coordinator descriptor leaves, including
/// the descriptor's owned diagnostic associated atom.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationDefinitionFingerprintSetV1<
    D = scoop_identity::PersistentInitializationUnitId,
> {
    registration_objects: VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<D>,
    fingerprints: Vec<VerifiedStrongInitializationDefinitionFingerprintV1>,
}

pub type VerifiedStrongInitializationDefinitionFingerprintSetV2 =
    VerifiedStrongInitializationDefinitionFingerprintSetV1<
        scoop_lir::StrongInitializationDependencyRefV2,
    >;

impl<D> VerifiedStrongInitializationDefinitionFingerprintSetV1<D> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registration_objects.producer()
    }

    pub const fn registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<D> {
        &self.registration_objects
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongInitializationDefinitionFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_initialization_definition_fingerprints_v1(
    registration_objects: VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationDefinitionFingerprintSetV1,
    StrongInitializationDefinitionFingerprintError,
> {
    compute_strong_initialization_definition_fingerprints(registration_objects, scoop_objects)
}

pub fn compute_strong_initialization_definition_fingerprints_v2(
    registration_objects: super::VerifiedStrongInitializationRegistrationObjectFingerprintSetV2,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationDefinitionFingerprintSetV2,
    StrongInitializationDefinitionFingerprintError,
> {
    compute_strong_initialization_definition_fingerprints(registration_objects, scoop_objects)
}

fn compute_strong_initialization_definition_fingerprints<D>(
    registration_objects: VerifiedStrongInitializationRegistrationObjectFingerprintSetV1<D>,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationDefinitionFingerprintSetV1<D>,
    StrongInitializationDefinitionFingerprintError,
> {
    let registrations = registration_objects.registrations();
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongInitializationDefinitionFingerprintError::ObjectValidation)?;
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let registration_fingerprints = registration_objects.fingerprints();
    if verified.len() != planned.len() || verified.len() != registration_fingerprints.len() {
        return Err(StrongInitializationDefinitionFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for ((verified, plan), registration_fingerprint) in
        verified.iter().zip(planned).zip(registration_fingerprints)
    {
        let unit = plan.semantic().unit();
        if verified.unit() != unit
            || registration_fingerprint.unit() != unit
            || registration_fingerprint.node() != plan.registration_object_node()
        {
            return Err(
                StrongInitializationDefinitionFingerprintError::RegistrationObjectMismatch { unit },
            );
        }
        let cell_object = objects.get(&verified.cell_member()).copied().ok_or(
            StrongInitializationDefinitionFingerprintError::MissingObject(verified.cell_member()),
        )?;
        let cell_bytes = exact_bytes(
            cell_object,
            verified.cell_checked_offset(),
            CELL_SIZE,
            unit,
            InitializationDefinitionArtifactV1::Cell,
        )?;
        let cell = domain_separated_runtime_hash(
            OBJECT_DEFINITION_DOMAIN,
            &ObjectDefinitionFingerprintInputV1 {
                bytes: cell_bytes,
                relocations: &[],
                direct_inputs: &[],
            },
        )
        .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
        .map_err(|source| StrongInitializationDefinitionFingerprintError::Hash { unit, source })?;

        let descriptor_object = objects.get(&verified.descriptor_member()).copied().ok_or(
            StrongInitializationDefinitionFingerprintError::MissingObject(
                verified.descriptor_member(),
            ),
        )?;
        let mut descriptor_bytes = exact_bytes(
            descriptor_object,
            verified.descriptor_checked_offset(),
            COORDINATOR_SIZE,
            unit,
            InitializationDefinitionArtifactV1::Descriptor,
        )?
        .to_vec();
        let relocations = descriptor_relocations(plan);
        for relocation in &relocations {
            relocation
                .normalize_bytes(&mut descriptor_bytes)
                .map_err(
                    |kind| StrongInitializationDefinitionFingerprintError::Relocation {
                        unit,
                        kind,
                    },
                )?;
        }
        let diagnostic_len = plan.semantic().diagnostic_path().len() + 1;
        let diagnostic_bytes = exact_bytes(
            descriptor_object,
            verified.diagnostic_checked_offset(),
            diagnostic_len,
            unit,
            InitializationDefinitionArtifactV1::Diagnostic,
        )?;
        let associated_atoms = [CanonicalAssociatedObjectAtomV1 {
            atom: plan.diagnostic_atom(),
            role: DefinitionAtomRole::AddressTakenConstant,
            bytes: diagnostic_bytes,
            relocations: &[],
        }];
        let descriptor = domain_separated_runtime_hash(
            OBJECT_DEFINITION_DOMAIN,
            &ObjectDefinitionLeafWithAssociatedAtomsInputV1 {
                primary: ObjectDefinitionFingerprintInputV1 {
                    bytes: &descriptor_bytes,
                    relocations: &relocations,
                    direct_inputs: &[],
                },
                associated_atoms: &associated_atoms,
            },
        )
        .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
        .map_err(|source| StrongInitializationDefinitionFingerprintError::Hash { unit, source })?;

        fingerprints.push(VerifiedStrongInitializationDefinitionFingerprintV1 {
            unit,
            cell_node: plan.cell_definition_node(),
            cell,
            descriptor_node: plan.descriptor_definition_node(),
            descriptor,
        });
    }

    Ok(VerifiedStrongInitializationDefinitionFingerprintSetV1 {
        registration_objects,
        fingerprints,
    })
}

fn exact_bytes(
    object: &[u8],
    checked_offset: u64,
    size: usize,
    unit: PersistentInitializationUnitId,
    artifact: InitializationDefinitionArtifactV1,
) -> Result<&[u8], StrongInitializationDefinitionFingerprintError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongInitializationDefinitionFingerprintError::Range { unit, artifact })?;
    let end = start
        .checked_add(size)
        .ok_or(StrongInitializationDefinitionFingerprintError::Range { unit, artifact })?;
    object
        .get(start..end)
        .ok_or(StrongInitializationDefinitionFingerprintError::Range { unit, artifact })
}

fn descriptor_relocations<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> [CanonicalObjectRelocationV1; 6] {
    [
        CanonicalObjectRelocationV1::owning_associated_atom_offset(
            40,
            plan.diagnostic_atom(),
            DefinitionAtomRole::AddressTakenConstant,
            0,
        ),
        strong_relocation(
            48,
            StrongDefinitionEntity::initialization_unit(plan.semantic().unit()),
            StrongDefinitionRole::InitializationCell,
        ),
        strong_relocation(
            56,
            StrongDefinitionEntity::static_storage(plan.storage().storage()),
            StrongDefinitionRole::StaticStorage,
        ),
        strong_relocation(
            64,
            StrongDefinitionEntity::static_storage(plan.failure_root().storage()),
            StrongDefinitionRole::StaticStorage,
        ),
        strong_relocation(
            72,
            StrongDefinitionEntity::callable_body(plan.initializer().body()),
            StrongDefinitionRole::CallableBody,
        ),
        strong_relocation(
            80,
            StrongDefinitionEntity::callable_body(plan.ensure().body()),
            StrongDefinitionRole::CallableBody,
        ),
    ]
}

fn strong_relocation(
    offset: u64,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CanonicalObjectRelocationV1 {
    let owner = StrongDefinitionOwnerV1::new(entity, role)
        .expect("initialization descriptor relocation has a valid strong owner");
    CanonicalObjectRelocationV1::unsigned64(
        offset,
        FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationDefinitionArtifactV1 {
    Cell,
    Descriptor,
    Diagnostic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationDefinitionFingerprintError {
    ObjectValidation(StrongInitializationRegistrationValidationError),
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        unit: PersistentInitializationUnitId,
    },
    MissingObject(SlibMemberId),
    Range {
        unit: PersistentInitializationUnitId,
        artifact: InitializationDefinitionArtifactV1,
    },
    Relocation {
        unit: PersistentInitializationUnitId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash {
        unit: PersistentInitializationUnitId,
        source: HashError,
    },
}

impl fmt::Display for StrongInitializationDefinitionFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong initialization definition fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationDefinitionFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}
