use std::fmt;

use scoop_identity::{DigestNodeId, PersistentExactTypeId};
use scoop_wire::HashError;

use super::physical::validate_objects;
use super::{
    StrongTypeRegistrationValidationError, VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
};
use crate::SlibMemberId;
use crate::link_object::{
    LayoutFingerprintV1, ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1,
};

mod encoding;
use encoding::{
    TypeDescriptorITableDirectoryFingerprintInputV1, descriptor_fingerprint, layout_fingerprint,
};

mod validation;
use validation::{TYPE_DESCRIPTOR_SIZE, exact_bytes, validate_descriptor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeDependencyFingerprintV1 {
    exact_type: PersistentExactTypeId,
    descriptor_definition_node: DigestNodeId,
    descriptor_definition: ObjectDefinitionFingerprintV1,
    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,
}

impl VerifiedStrongTypeDependencyFingerprintV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn descriptor_definition_node(self) -> DigestNodeId {
        self.descriptor_definition_node
    }

    pub const fn descriptor_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.descriptor_definition
    }

    pub const fn layout_node(self) -> DigestNodeId {
        self.layout_node
    }

    pub const fn layout(self) -> LayoutFingerprintV1 {
        self.layout
    }
}

/// Descriptor-definition and layout fingerprints rebuilt from the same
/// registration-object proof, exact object bytes, and complete LIR semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeDependencyFingerprintSetV1 {
    registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongTypeDependencyFingerprintV1>,
}

impl VerifiedStrongTypeDependencyFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registration_objects.producer()
    }

    pub const fn registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.registration_objects
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongTypeDependencyFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_type_dependency_fingerprints_v1(
    registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeDependencyFingerprintSetV1, StrongTypeDependencyFingerprintError> {
    let registrations = registration_objects.registrations();
    let objects = validate_objects(registrations.patch_sites().builtins(), scoop_objects)
        .map_err(StrongTypeDependencyFingerprintError::ObjectValidation)?;
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let object_fingerprints = registration_objects.fingerprints();
    if verified.len() != planned.len() || verified.len() != object_fingerprints.len() {
        return Err(StrongTypeDependencyFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for ((verified, plan), registration_object) in
        verified.iter().zip(planned).zip(object_fingerprints)
    {
        let exact_type = plan.exact_type();
        if verified.exact_type() != exact_type
            || registration_object.exact_type() != exact_type
            || registration_object.node() != plan.registration_object_node()
        {
            return Err(
                StrongTypeDependencyFingerprintError::RegistrationObjectMismatch { exact_type },
            );
        }
        let descriptor_proof = verified.descriptor();
        let object = objects.get(&descriptor_proof.member()).copied().ok_or(
            StrongTypeDependencyFingerprintError::MissingObject(descriptor_proof.member()),
        )?;
        let descriptor_bytes = exact_bytes(
            object,
            descriptor_proof.checked_offset(),
            TYPE_DESCRIPTOR_SIZE,
            exact_type,
            TypeDependencyArtifactV1::Descriptor,
        )?;
        validate_descriptor(
            descriptor_bytes,
            registrations.patch_sites().builtins(),
            verified,
            plan,
        )?;
        let diagnostic_bytes = exact_bytes(
            object,
            descriptor_proof.diagnostic_checked_offset(),
            usize::try_from(descriptor_proof.diagnostic_size()).map_err(|_| {
                StrongTypeDependencyFingerprintError::Range {
                    exact_type,
                    artifact: TypeDependencyArtifactV1::Diagnostic,
                }
            })?,
            exact_type,
            TypeDependencyArtifactV1::Diagnostic,
        )?;
        let itable_directory = match (plan.itable_directory(), descriptor_proof.itable_directory())
        {
            (
                scoop_lir::TypeDescriptorITableDirectoryV1::Null,
                super::VerifiedTypeDescriptorITableDirectoryV1::Null,
            ) => TypeDescriptorITableDirectoryFingerprintInputV1::Null,
            (
                scoop_lir::TypeDescriptorITableDirectoryV1::Defined(atom),
                super::VerifiedTypeDescriptorITableDirectoryV1::Defined {
                    checked_offset,
                    byte_size,
                    ..
                },
            ) => TypeDescriptorITableDirectoryFingerprintInputV1::Defined {
                atom,
                bytes: exact_bytes(
                    object,
                    *checked_offset,
                    usize::try_from(*byte_size).map_err(|_| {
                        StrongTypeDependencyFingerprintError::Range {
                            exact_type,
                            artifact: TypeDependencyArtifactV1::ITableDirectory,
                        }
                    })?,
                    exact_type,
                    TypeDependencyArtifactV1::ITableDirectory,
                )?,
            },
            _ => {
                return Err(
                    StrongTypeDependencyFingerprintError::ITableDirectoryProofMismatch {
                        exact_type,
                    },
                );
            }
        };
        let descriptor_definition =
            descriptor_fingerprint(descriptor_bytes, diagnostic_bytes, itable_directory, plan)
                .map_err(
                    |source| StrongTypeDependencyFingerprintError::DescriptorHash {
                        exact_type,
                        source,
                    },
                )?;
        let layout = layout_fingerprint(plan).map_err(|source| {
            StrongTypeDependencyFingerprintError::LayoutHash { exact_type, source }
        })?;
        fingerprints.push(VerifiedStrongTypeDependencyFingerprintV1 {
            exact_type,
            descriptor_definition_node: plan.descriptor_definition_node(),
            descriptor_definition,
            layout_node: plan.layout_fingerprint_node(),
            layout,
        });
    }

    Ok(VerifiedStrongTypeDependencyFingerprintSetV1 {
        registration_objects,
        fingerprints,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeDependencyArtifactV1 {
    Descriptor,
    Diagnostic,
    ITableDirectory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeDependencyFingerprintError {
    ObjectValidation(StrongTypeRegistrationValidationError),
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        exact_type: PersistentExactTypeId,
    },
    ITableDirectoryProofMismatch {
        exact_type: PersistentExactTypeId,
    },
    MissingObject(SlibMemberId),
    Range {
        exact_type: PersistentExactTypeId,
        artifact: TypeDependencyArtifactV1,
    },
    DescriptorByteMismatch {
        exact_type: PersistentExactTypeId,
        offset_within_descriptor: u8,
        expected: u8,
        actual: u8,
    },
    DescriptorRelocationSetMismatch {
        exact_type: PersistentExactTypeId,
        expected: Vec<u64>,
        actual: Vec<u64>,
    },
    DescriptorRelocationMismatch {
        exact_type: PersistentExactTypeId,
        offset_within_descriptor: u64,
    },
    DescriptorHash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
    LayoutHash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
}

impl fmt::Display for StrongTypeDependencyFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong type dependency fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongTypeDependencyFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::DescriptorHash { source, .. } | Self::LayoutHash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
