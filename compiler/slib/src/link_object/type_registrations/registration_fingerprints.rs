use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, PersistentExactTypeId};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedStrongTypeRegistrationSetV1;
use super::dependency_fingerprints::VerifiedStrongTypeDependencyFingerprintV1;
use crate::link_object::callable_registrations::object_definition::CanonicalDigestInputV1;
use crate::link_object::{
    LayoutFingerprintV1, ObjectDefinitionFingerprintV1, OdrShapeFingerprintV1,
    RegistrationFingerprintV1, ScoopLirObjectCandidateV1, StrongRegistrationFingerprintV1,
    VerifiedObjectDefinitionRequirementSetV1,
};

const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const TYPE_REGISTRATION_RECORD_KIND: u32 = 4;
const TYPE_DESCRIPTOR_ATOM_ROLE: u32 = 6;

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
    registration: RegistrationFingerprintV1,
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

    pub const fn registration(self) -> RegistrationFingerprintV1 {
        self.registration
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeFingerprintSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
> {
    registrations: VerifiedStrongTypeRegistrationSetV1<D, C>,
    shapes: Vec<OdrShapeFingerprintV1>,
    fingerprints: Vec<VerifiedStrongTypeFingerprintV1>,
}

pub type VerifiedStrongTypeFingerprintSetV2 = VerifiedStrongTypeFingerprintSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone> VerifiedStrongTypeFingerprintSetV1<D, C> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1<D, C> {
        &self.registrations
    }

    pub fn shapes(&self) -> &[OdrShapeFingerprintV1] {
        &self.shapes
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongTypeFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_type_fingerprints_v1(
    registrations: VerifiedStrongTypeRegistrationSetV1,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
    requirements: impl Into<VerifiedObjectDefinitionRequirementSetV1>,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeFingerprintSetV1, StrongTypeFingerprintError> {
    compute(registrations, canonical, requirements.into(), objects)
}

pub fn compute_strong_type_fingerprints_v2(
    registrations: super::VerifiedStrongTypeRegistrationSetV2,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
    requirements: impl Into<VerifiedObjectDefinitionRequirementSetV1>,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeFingerprintSetV2, StrongTypeFingerprintError> {
    compute(registrations, canonical, requirements.into(), objects)
}

fn compute<D, C>(
    registrations: VerifiedStrongTypeRegistrationSetV1<D, C>,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
    requirements: VerifiedObjectDefinitionRequirementSetV1,
    candidates: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeFingerprintSetV1<D, C>, StrongTypeFingerprintError>
where
    D: super::LinkDescriptorReference,
    C: super::LinkDispatchCallableReference + Clone,
{
    let builtins = registrations.patch_sites().builtins();
    let objects = super::physical::validate_objects(builtins, candidates)
        .map_err(StrongTypeFingerprintError::Objects)?;
    let closure = builtins.strong_relocations();
    if !requirements.matches_strong_closure(closure) {
        return Err(StrongTypeFingerprintError::Requirements);
    }
    let shapes = crate::link_object::shape_fingerprints::compute(
        canonical,
        closure,
        &requirements,
        &objects,
    )
    .map_err(StrongTypeFingerprintError::Shapes)?;
    let dependencies = super::dependency_fingerprints::compute(&registrations, &objects, &shapes)
        .map_err(StrongTypeFingerprintError::Dependencies)?;
    let mut fingerprints = Vec::with_capacity(registrations.registrations().len());
    for ((verified, plan), dependency) in registrations
        .registrations()
        .iter()
        .zip(registrations.plan().registrations())
        .zip(&dependencies)
    {
        let exact_type = plan.exact_type();
        let object = objects
            .get(&verified.member())
            .copied()
            .ok_or(StrongTypeFingerprintError::MissingObject(verified.member()))?;
        let registration_object = super::fingerprints::registration_object(
            plan,
            verified,
            dependency,
            object,
            closure,
            &requirements,
        )
        .map_err(StrongTypeFingerprintError::RegistrationObject)?;
        let registration = match plan.definition_owner() {
            scoop_lir::RegistrationDefinitionOwner::Strong => strong_type_registration_fingerprint(
                plan,
                plan.registration_object_node(),
                registration_object,
                dependency,
            )
            .map(RegistrationFingerprintV1::Strong),
            scoop_lir::RegistrationDefinitionOwner::Odr { group, member } => {
                crate::link_object::odr_member_fingerprints::type_registration(
                    group,
                    member,
                    plan,
                    registration_object,
                )
                .map(RegistrationFingerprintV1::Odr)
            }
        }
        .map_err(|source| StrongTypeFingerprintError::Hash { exact_type, source })?;
        fingerprints.push(VerifiedStrongTypeFingerprintV1 {
            exact_type,
            registration_object_node: plan.registration_object_node(),
            registration_object,
            descriptor_definition_node: dependency.descriptor_definition_node(),
            descriptor_definition: dependency.descriptor_definition(),
            layout_node: dependency.layout_node(),
            layout: dependency.layout(),
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }
    Ok(VerifiedStrongTypeFingerprintSetV1 {
        registrations,
        shapes,
        fingerprints,
    })
}

fn strong_type_registration_fingerprint<D: Copy, C>(
    plan: &scoop_lir::StrongTypeRegistrationPlan<D, C>,
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
        runtime_encode_strong_type_record_v1(
            encoder,
            self.exact_type,
            self.runtime_type,
            &[0; 32],
            self.descriptor_definition.as_array(),
            self.layout.as_array(),
        )?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

pub(in crate::link_object) fn runtime_encode_strong_type_record_v1(
    encoder: &mut RuntimeEncoder,
    exact_type: PersistentExactTypeId,
    runtime_type: u64,
    registration: &[u8; 32],
    descriptor_definition: &[u8; 32],
    layout: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(TYPE_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        exact_type.as_array(),
        scoop_lir::RegistrationDefinitionOwner::Strong,
        registration,
    )?;
    encoder.u64(runtime_type)?;
    encoder.u32(TYPE_DESCRIPTOR_ATOM_ROLE)?;
    encoder.fixed(exact_type.as_array())?;
    encoder.fixed(descriptor_definition)?;
    encoder.fixed(layout)
}

pub(in crate::link_object) fn runtime_encode_type_record_v1<D: Copy, C>(
    encoder: &mut RuntimeEncoder,
    plan: &scoop_lir::StrongTypeRegistrationPlan<D, C>,
    registration: &[u8; 32],
    descriptor_definition: &[u8; 32],
    layout: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(TYPE_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        plan.exact_type().as_array(),
        plan.definition_owner(),
        registration,
    )?;
    encoder.u64(plan.runtime_type().get())?;
    encoder.u32(TYPE_DESCRIPTOR_ATOM_ROLE)?;
    encoder.fixed(plan.exact_type().as_array())?;
    encoder.fixed(descriptor_definition)?;
    encoder.fixed(layout)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeFingerprintError {
    Objects(super::StrongTypeRegistrationValidationError),
    MissingObject(crate::SlibMemberId),
    Requirements,
    Shapes(crate::link_object::OdrShapeFingerprintError),
    Dependencies(super::StrongTypeDependencyFingerprintError),
    RegistrationObject(super::StrongTypeRegistrationObjectFingerprintError),
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
