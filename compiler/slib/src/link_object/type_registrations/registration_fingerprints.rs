use std::fmt;

use scoop_identity::{DigestNodeId, PersistentExactTypeId};
use scoop_wire::{HashError, RuntimeEncodeError, RuntimeEncoder};

use super::TypeRegistrationDataV1;
use crate::link_object::{
    LayoutFingerprintV1, ObjectDefinitionFingerprintV1, OdrMemberAbiV1, RegistrationAbiV1,
    ScoopLirObjectCandidateV1,
};

const TYPE_REGISTRATION_RECORD_KIND: u32 = 4;
const TYPE_DESCRIPTOR_ATOM_ROLE: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeFingerprintV1 {
    exact_type: PersistentExactTypeId,

    descriptor_definition_node: DigestNodeId,
    descriptor_definition: ObjectDefinitionFingerprintV1,
    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,

    registration: RegistrationAbiV1,
}

impl TypeFingerprintV1 {
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

    pub const fn registration(self) -> RegistrationAbiV1 {
        self.registration
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFingerprintSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
> {
    registrations: TypeRegistrationDataV1<D, C>,
    shapes: Vec<OdrMemberAbiV1>,
    fingerprints: Vec<TypeFingerprintV1>,
}

pub type TypeFingerprintSetV2 = TypeFingerprintSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone> TypeFingerprintSetV1<D, C> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &TypeRegistrationDataV1<D, C> {
        &self.registrations
    }

    pub fn shapes(&self) -> &[OdrMemberAbiV1] {
        &self.shapes
    }

    pub fn fingerprints(&self) -> &[TypeFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_type_fingerprints_v1(
    registrations: TypeRegistrationDataV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<TypeFingerprintSetV1, StrongTypeFingerprintError> {
    compute(registrations, canonical, objects)
}

pub fn compute_strong_type_fingerprints_v2(
    registrations: super::TypeRegistrationDataV1<
        scoop_lir::StrongTypeDescriptorRefV2,
        scoop_lir::StrongTypeDispatchCallableRefV2,
    >,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<TypeFingerprintSetV2, StrongTypeFingerprintError> {
    compute(registrations, canonical, objects)
}

fn compute<D, C>(
    registrations: TypeRegistrationDataV1<D, C>,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
    candidates: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<TypeFingerprintSetV1<D, C>, StrongTypeFingerprintError>
where
    D: super::LinkDescriptorReference,
    C: super::LinkDispatchCallableReference + Clone,
{
    let objects = candidates
        .iter()
        .map(|object| (object.member(), object.bytes()))
        .collect();
    let shapes = canonical
        .definitions()
        .iter()
        .filter(|shape| {
            !matches!(
                shape.role(),
                scoop_identity::OdrMemberRole::ImmortalObject
                    | scoop_identity::OdrMemberRole::StaticStorage
                    | scoop_identity::OdrMemberRole::InitializationCell
            )
        })
        .map(|shape| crate::link_object::odr_member_fingerprints::shape_definition(*shape))
        .collect();
    let dependencies = super::dependency_fingerprints::compute(&registrations, &objects)
        .map_err(StrongTypeFingerprintError::Dependencies)?;
    let mut fingerprints = Vec::with_capacity(registrations.plan().registrations().len());
    for (plan, dependency) in registrations
        .plan()
        .registrations()
        .iter()
        .zip(&dependencies)
    {
        let exact_type = plan.exact_type();
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::Type,
        )
        .map_err(|source| StrongTypeFingerprintError::Hash { exact_type, source })?;
        fingerprints.push(TypeFingerprintV1 {
            exact_type,

            descriptor_definition_node: dependency.descriptor_definition_node(),
            descriptor_definition: dependency.descriptor_definition(),
            layout_node: dependency.layout_node(),
            layout: dependency.layout(),

            registration,
        });
    }
    Ok(TypeFingerprintSetV1 {
        registrations,
        shapes,
        fingerprints,
    })
}

pub(in crate::link_object) fn runtime_encode_type_record_v1<D: Copy, C>(
    encoder: &mut RuntimeEncoder,
    plan: &scoop_lir::StrongTypeRegistrationPlan<D, C>,
    descriptor_definition: &[u8; 32],
    layout: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(TYPE_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        plan.exact_type().as_array(),
        plan.definition_owner(),
    )?;
    encoder.u64(plan.runtime_type().get())?;
    encoder.u32(TYPE_DESCRIPTOR_ATOM_ROLE)?;
    encoder.fixed(plan.exact_type().as_array())?;
    encoder.fixed(descriptor_definition)?;
    encoder.fixed(layout)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeFingerprintError {
    Dependencies(super::StrongTypeDependencyFingerprintError),
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
