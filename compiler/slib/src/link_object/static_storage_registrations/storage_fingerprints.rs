use std::fmt;

use scoop_identity::{
    DigestNodeId, PersistentStaticStorageId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::StrongStaticStorageRegistrationValidationError;
use super::fingerprints::VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1;
use super::physical::validate_objects;
use super::verification::VerifiedStaticStorageMaterializationV1;
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalObjectRelocationV1, ObjectDefinitionFingerprintInputV1,
    ObjectDefinitionRelocationFailureV1,
};
use crate::link_object::{
    FinalUndefinedSymbolRequirementV1, ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1,
    StrongDefinitionOwnerV1,
};

const OBJECT_DEFINITION_DOMAIN: &str = "scoop-object-definition-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageDefinitionFingerprintV1 {
    storage: PersistentStaticStorageId,
    node: DigestNodeId,
    fingerprint: ObjectDefinitionFingerprintV1,
}

impl VerifiedStrongStaticStorageDefinitionFingerprintV1 {
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

/// Canonical writable-storage leaves bound to the complete registration and
/// initial-state artifact proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageDefinitionFingerprintSetV1 {
    registration_objects: VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongStaticStorageDefinitionFingerprintV1>,
}

impl VerifiedStrongStaticStorageDefinitionFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registration_objects.producer()
    }

    pub const fn registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.registration_objects
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongStaticStorageDefinitionFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_static_storage_definition_fingerprints_v1(
    registration_objects: VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongStaticStorageDefinitionFingerprintSetV1,
    StrongStaticStorageDefinitionFingerprintError,
> {
    let registrations = registration_objects.registrations();
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongStaticStorageDefinitionFingerprintError::ObjectValidation)?;
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let registration_fingerprints = registration_objects.fingerprints();
    if verified.len() != planned.len() || verified.len() != registration_fingerprints.len() {
        return Err(StrongStaticStorageDefinitionFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for ((verified, plan), registration_fingerprint) in
        verified.iter().zip(planned).zip(registration_fingerprints)
    {
        let storage = plan.semantic().storage();
        if verified.storage() != storage
            || registration_fingerprint.storage() != storage
            || registration_fingerprint.node() != plan.registration_object_node()
        {
            return Err(
                StrongStaticStorageDefinitionFingerprintError::RegistrationObjectMismatch {
                    storage,
                },
            );
        }
        let object = objects.get(&verified.storage_member()).copied().ok_or(
            StrongStaticStorageDefinitionFingerprintError::MissingObject(verified.storage_member()),
        )?;
        let mut bytes = storage_bytes(
            object,
            verified.storage_materialization(),
            plan.semantic().allocation_extent(),
            storage,
        )?;
        let relocations = plan
            .semantic()
            .initial_state()
            .immortal_relocations()
            .iter()
            .map(|relocation| {
                let owner = StrongDefinitionOwnerV1::new(
                    StrongDefinitionEntity::immortal_object(relocation.target()),
                    StrongDefinitionRole::ImmortalObject,
                )
                .expect("immortal objects are valid strong definition owners");
                CanonicalObjectRelocationV1::unsigned64(
                    relocation.pointer_offset(),
                    FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
                )
            })
            .collect::<Vec<_>>();
        for relocation in &relocations {
            relocation.normalize_bytes(&mut bytes).map_err(|kind| {
                StrongStaticStorageDefinitionFingerprintError::Relocation { storage, kind }
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
        .map_err(
            |source| StrongStaticStorageDefinitionFingerprintError::Hash { storage, source },
        )?;
        fingerprints.push(VerifiedStrongStaticStorageDefinitionFingerprintV1 {
            storage,
            node: plan.storage_definition_node(),
            fingerprint,
        });
    }

    Ok(VerifiedStrongStaticStorageDefinitionFingerprintSetV1 {
        registration_objects,
        fingerprints,
    })
}

fn storage_bytes(
    object: &[u8],
    materialization: VerifiedStaticStorageMaterializationV1,
    extent: u64,
    storage: PersistentStaticStorageId,
) -> Result<Vec<u8>, StrongStaticStorageDefinitionFingerprintError> {
    let extent = usize::try_from(extent)
        .map_err(|_| StrongStaticStorageDefinitionFingerprintError::StorageRange(storage))?;
    match materialization {
        VerifiedStaticStorageMaterializationV1::FileBacked { checked_offset } => {
            let start = usize::try_from(checked_offset).map_err(|_| {
                StrongStaticStorageDefinitionFingerprintError::StorageRange(storage)
            })?;
            let end = start.checked_add(extent).ok_or(
                StrongStaticStorageDefinitionFingerprintError::StorageRange(storage),
            )?;
            object.get(start..end).map(<[u8]>::to_vec).ok_or(
                StrongStaticStorageDefinitionFingerprintError::StorageRange(storage),
            )
        }
        VerifiedStaticStorageMaterializationV1::ZeroFill => Ok(vec![0; extent]),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageDefinitionFingerprintError {
    ObjectValidation(StrongStaticStorageRegistrationValidationError),
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        storage: PersistentStaticStorageId,
    },
    MissingObject(SlibMemberId),
    StorageRange(PersistentStaticStorageId),
    Relocation {
        storage: PersistentStaticStorageId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash {
        storage: PersistentStaticStorageId,
        source: HashError,
    },
}

impl fmt::Display for StrongStaticStorageDefinitionFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong static-storage definition fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageDefinitionFingerprintError {
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
