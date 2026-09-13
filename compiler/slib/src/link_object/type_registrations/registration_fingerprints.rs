use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, PersistentExactTypeId};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedStrongTypeRegistrationObjectFingerprintSetV1;
use crate::link_object::callable_registrations::object_definition::CanonicalDigestInputV1;
use crate::link_object::{
    LayoutFingerprintV1, ObjectDefinitionFingerprintV1, StrongRegistrationFingerprintV1,
};

const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const TYPE_REGISTRATION_RECORD_KIND: u32 = 4;
const STRONG_LINKAGE: u32 = 1;
const TYPE_DESCRIPTOR_ATOM_ROLE: u32 = 6;

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

/// Descriptor-definition and layout fingerprints bound to the same exact
/// type registration proof. Only their dedicated verified calculators can
/// construct this set.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeFingerprintV1 {
    exact_type: PersistentExactTypeId,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    descriptor_definition_node: DigestNodeId,
    descriptor_definition: ObjectDefinitionFingerprintV1,
    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,
    registration_node: DigestNodeId,
    registration: StrongRegistrationFingerprintV1,
}

impl VerifiedStrongTypeFingerprintV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn registration_object(self) -> ObjectDefinitionFingerprintV1 {
        self.registration_object
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

    pub const fn registration_node(self) -> DigestNodeId {
        self.registration_node
    }

    pub const fn registration(self) -> StrongRegistrationFingerprintV1 {
        self.registration
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeFingerprintSetV1 {
    dependencies: VerifiedStrongTypeDependencyFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongTypeFingerprintV1>,
}

impl VerifiedStrongTypeFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.dependencies.producer()
    }

    pub const fn dependencies(&self) -> &VerifiedStrongTypeDependencyFingerprintSetV1 {
        &self.dependencies
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongTypeFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_type_fingerprints_v1(
    dependencies: VerifiedStrongTypeDependencyFingerprintSetV1,
) -> Result<VerifiedStrongTypeFingerprintSetV1, StrongTypeFingerprintError> {
    let registration_objects = dependencies.registration_objects();
    let registrations = registration_objects.registrations();
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let object_fingerprints = registration_objects.fingerprints();
    if verified.len() != planned.len()
        || verified.len() != object_fingerprints.len()
        || verified.len() != dependencies.fingerprints().len()
    {
        return Err(StrongTypeFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for (((verified, plan), registration_object), dependency) in verified
        .iter()
        .zip(planned)
        .zip(object_fingerprints)
        .zip(dependencies.fingerprints())
    {
        let exact_type = plan.exact_type();
        if verified.exact_type() != exact_type
            || registration_object.exact_type() != exact_type
            || registration_object.node() != plan.registration_object_node()
        {
            return Err(StrongTypeFingerprintError::RegistrationObjectMismatch { exact_type });
        }
        if dependency.exact_type() != exact_type
            || dependency.descriptor_definition_node() != plan.descriptor_definition_node()
        {
            return Err(StrongTypeFingerprintError::DescriptorDefinitionMismatch { exact_type });
        }
        if dependency.layout_node() != plan.layout_fingerprint_node() {
            return Err(StrongTypeFingerprintError::LayoutMismatch { exact_type });
        }
        let registration = strong_type_registration_fingerprint(
            *plan,
            registration_object.node(),
            registration_object.fingerprint(),
            dependency,
        )
        .map_err(|source| StrongTypeFingerprintError::Hash { exact_type, source })?;
        fingerprints.push(VerifiedStrongTypeFingerprintV1 {
            exact_type,
            registration_object_node: registration_object.node(),
            registration_object: registration_object.fingerprint(),
            descriptor_definition_node: dependency.descriptor_definition_node(),
            descriptor_definition: dependency.descriptor_definition(),
            layout_node: dependency.layout_node(),
            layout: dependency.layout(),
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }

    Ok(VerifiedStrongTypeFingerprintSetV1 {
        dependencies,
        fingerprints,
    })
}

fn strong_type_registration_fingerprint(
    plan: scoop_lir::StrongTypeRegistrationPlanV1,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    dependency: &VerifiedStrongTypeDependencyFingerprintV1,
) -> Result<StrongRegistrationFingerprintV1, HashError> {
    let mut direct_inputs = [
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: registration_object_node,
            digest: *registration_object.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: dependency.descriptor_definition_node(),
            digest: *dependency.descriptor_definition().as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::Layout,
            node: dependency.layout_node(),
            digest: *dependency.layout().as_array(),
        },
    ];
    direct_inputs.sort_unstable_by_key(|input| (input.kind.tag(), input.node));
    domain_separated_runtime_hash(
        STRONG_REGISTRATION_DOMAIN,
        &StrongTypeRegistrationFingerprintInputV1 {
            exact_type: plan.exact_type(),
            runtime_type: plan.runtime_type().get(),
            descriptor_definition: dependency.descriptor_definition(),
            layout: dependency.layout(),
            direct_inputs,
        },
    )
    .map(|digest| StrongRegistrationFingerprintV1::from_array(*digest.as_array()))
}

struct StrongTypeRegistrationFingerprintInputV1 {
    exact_type: PersistentExactTypeId,
    runtime_type: u64,
    descriptor_definition: ObjectDefinitionFingerprintV1,
    layout: LayoutFingerprintV1,
    direct_inputs: [CanonicalDigestInputV1; 3],
}

impl RuntimeEncode for StrongTypeRegistrationFingerprintInputV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(TYPE_REGISTRATION_RECORD_KIND)?;
        encoder.u32(STRONG_LINKAGE)?;
        encoder.fixed(self.exact_type.as_array())?;
        encoder.fixed(&[0; 32])?;
        encoder.fixed(&[0; 32])?;
        encoder.fixed(&[0; 32])?;
        encoder.u64(self.runtime_type)?;
        encoder.u32(TYPE_DESCRIPTOR_ATOM_ROLE)?;
        encoder.fixed(self.exact_type.as_array())?;
        encoder.fixed(self.descriptor_definition.as_array())?;
        encoder.fixed(self.layout.as_array())?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeFingerprintError {
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        exact_type: PersistentExactTypeId,
    },
    DescriptorDefinitionMismatch {
        exact_type: PersistentExactTypeId,
    },
    LayoutMismatch {
        exact_type: PersistentExactTypeId,
    },
    Hash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
}

impl fmt::Display for StrongTypeFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong type registration fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongTypeFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
