use std::fmt;

use scoop_identity::{ConeIdentity, DigestKind, DigestNodeKey};
use scoop_lir::{RuntimeAbiFingerprint, TargetProfileFingerprint};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedConeImageV1;
use crate::link_object::{
    runtime_encode_callable_record_v1, runtime_encode_safepoint_record_v1,
    runtime_encode_strong_immortal_object_record_v1,
    runtime_encode_strong_initialization_record_v1, runtime_encode_strong_static_storage_record_v1,
    runtime_encode_type_record_v1,
};
use crate::{CompatibilityRecord, RuntimeImageFingerprint, VerifiedStrongRegistrationPatchSetV1};

const RUNTIME_IMAGE_DOMAIN: &str = "scoop-runtime-image-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeImageFingerprintV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = scoop_identity::PersistentInitializationUnitId,
> {
    image: VerifiedConeImageV1,
    registrations: VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    compatibility: CompatibilityRecord,
    fingerprint: RuntimeImageFingerprint,
}

pub type VerifiedRuntimeImageFingerprintV2 = VerifiedRuntimeImageFingerprintV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I>
    VerifiedRuntimeImageFingerprintV1<D, C, I>
{
    pub const fn producer(&self) -> ConeIdentity {
        self.image.producer()
    }

    pub const fn image(&self) -> &VerifiedConeImageV1 {
        &self.image
    }

    pub const fn registrations(&self) -> &VerifiedStrongRegistrationPatchSetV1<D, C, I> {
        &self.registrations
    }

    pub const fn compatibility(&self) -> &CompatibilityRecord {
        &self.compatibility
    }

    pub const fn fingerprint(&self) -> RuntimeImageFingerprint {
        self.fingerprint
    }
}

pub fn compute_runtime_image_fingerprint_v1(
    image: VerifiedConeImageV1,
    registrations: VerifiedStrongRegistrationPatchSetV1,
    compatibility: CompatibilityRecord,
) -> Result<VerifiedRuntimeImageFingerprintV1, RuntimeImageFingerprintError> {
    compute_runtime_image_fingerprint(image, registrations, compatibility)
}

pub fn compute_runtime_image_fingerprint_v2(
    image: VerifiedConeImageV1,
    registrations: crate::VerifiedStrongRegistrationPatchSetV2,
    compatibility: CompatibilityRecord,
) -> Result<VerifiedRuntimeImageFingerprintV2, RuntimeImageFingerprintError> {
    compute_runtime_image_fingerprint(image, registrations, compatibility)
}

fn compute_runtime_image_fingerprint<D, C, I>(
    image: VerifiedConeImageV1,
    registrations: VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    compatibility: CompatibilityRecord,
) -> Result<VerifiedRuntimeImageFingerprintV1<D, C, I>, RuntimeImageFingerprintError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    validate_proof_binding(&image, &registrations)?;
    validate_tables(&image, &registrations)?;
    validate_runtime_image_inputs(&image, &registrations)?;
    let fingerprint = domain_separated_runtime_hash(
        RUNTIME_IMAGE_DOMAIN,
        &RuntimeImageFingerprintInputV1 {
            image: &image,
            registrations: &registrations,
            runtime_abi: compatibility.runtime_abi(),
            target_profile: compatibility.target_fingerprint(),
        },
    )
    .map(|digest| RuntimeImageFingerprint::from_array(*digest.as_array()))
    .map_err(RuntimeImageFingerprintError::Hash)?;
    Ok(VerifiedRuntimeImageFingerprintV1 {
        image,
        registrations,
        compatibility,
        fingerprint,
    })
}

fn validate_proof_binding<D, C, I>(
    image: &VerifiedConeImageV1,
    registrations: &VerifiedStrongRegistrationPatchSetV1<D, C, I>,
) -> Result<(), RuntimeImageFingerprintError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    if image.producer() != registrations.producer() {
        return Err(RuntimeImageFingerprintError::ProducerMismatch {
            image: image.producer(),
            registrations: registrations.producer(),
        });
    }
    let registration_patch_sites = registrations.safepoints().registrations().patch_sites();
    if image.patch_sites() != registration_patch_sites {
        return Err(RuntimeImageFingerprintError::ObjectProofMismatch);
    }
    Ok(())
}

fn validate_tables<D, C, I>(
    image: &VerifiedConeImageV1,
    registrations: &VerifiedStrongRegistrationPatchSetV1<D, C, I>,
) -> Result<(), RuntimeImageFingerprintError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let tables = image.plan().tables();
    require_table(
        ConeImageTableKindV1::StaticStorages,
        tables.static_storages(),
        registrations
            .static_storages()
            .fingerprints()
            .iter()
            .map(|entry| entry.storage()),
    )?;
    require_table(
        ConeImageTableKindV1::ImmortalObjects,
        tables.immortal_objects(),
        registrations
            .immortal_objects()
            .fingerprints()
            .iter()
            .map(|entry| entry.object()),
    )?;
    require_table(
        ConeImageTableKindV1::InitializationUnits,
        tables.initialization_units(),
        registrations
            .initializations()
            .fingerprints()
            .iter()
            .map(|entry| entry.unit()),
    )?;
    require_table(
        ConeImageTableKindV1::TypeRegistrations,
        tables.type_registrations(),
        registrations
            .types()
            .fingerprints()
            .iter()
            .map(|entry| entry.exact_type()),
    )?;
    require_table(
        ConeImageTableKindV1::Safepoints,
        tables.safepoints(),
        registrations
            .safepoints()
            .fingerprints()
            .iter()
            .map(|entry| entry.site()),
    )?;
    require_table(
        ConeImageTableKindV1::Callables,
        tables.callables(),
        registrations
            .callables()
            .fingerprints()
            .iter()
            .map(|entry| entry.body()),
    )
}

fn require_table<T: Copy + Eq>(
    kind: ConeImageTableKindV1,
    expected: &[T],
    actual: impl Iterator<Item = T>,
) -> Result<(), RuntimeImageFingerprintError> {
    if actual.eq(expected.iter().copied()) {
        Ok(())
    } else {
        Err(RuntimeImageFingerprintError::TableMismatch(kind))
    }
}

fn validate_runtime_image_inputs<D, C, I>(
    image: &VerifiedConeImageV1,
    registrations: &VerifiedStrongRegistrationPatchSetV1<D, C, I>,
) -> Result<(), RuntimeImageFingerprintError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let expected_key = DigestNodeKey::runtime_image(image.producer());
    let node = image
        .patch_sites()
        .digest_plan()
        .nodes()
        .iter()
        .find(|node| node.key() == &expected_key)
        .ok_or(RuntimeImageFingerprintError::MissingDigestNode)?;
    let actual = node
        .direct_inputs()
        .iter()
        .map(|input| (input.kind(), input.node()))
        .collect::<Vec<_>>();
    let mut expected = registrations
        .static_storages()
        .fingerprints()
        .iter()
        .map(|entry| entry.registration_node())
        .chain(
            registrations
                .initializations()
                .fingerprints()
                .iter()
                .map(|entry| entry.registration_node()),
        )
        .map(|node| (DigestKind::StrongRegistration, node))
        .chain(
            registrations
                .immortal_objects()
                .fingerprints()
                .iter()
                .map(|entry| (entry.registration().kind(), entry.registration_node())),
        )
        .chain(
            registrations
                .types()
                .fingerprints()
                .iter()
                .map(|entry| (entry.registration().kind(), entry.registration_node())),
        )
        .chain(
            registrations
                .safepoints()
                .fingerprints()
                .iter()
                .map(|entry| (entry.registration().kind(), entry.registration_node())),
        )
        .chain(
            registrations
                .callables()
                .fingerprints()
                .iter()
                .map(|entry| (entry.registration().kind(), entry.registration_node())),
        )
        .collect::<Vec<_>>();
    expected.sort_unstable();
    if actual != expected {
        return Err(RuntimeImageFingerprintError::DigestInputMismatch);
    }
    Ok(())
}

struct RuntimeImageFingerprintInputV1<'proof, D, C, I> {
    image: &'proof VerifiedConeImageV1,
    registrations: &'proof VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    runtime_abi: RuntimeAbiFingerprint,
    target_profile: TargetProfileFingerprint,
}

impl<D, C, I> RuntimeEncode for RuntimeImageFingerprintInputV1<'_, D, C, I>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(self.runtime_abi.as_array())?;
        encoder.fixed(self.target_profile.as_array())?;
        self.image.plan().cone().runtime_encode(encoder)?;
        encoder.sequence_length(self.image.plan().dependencies().len())?;
        for dependency in self.image.plan().dependencies() {
            encoder.fixed(dependency.as_array())?;
        }
        self.encode_static_storages(encoder)?;
        self.encode_immortal_objects(encoder)?;
        self.encode_initializations(encoder)?;
        self.encode_types(encoder)?;
        self.encode_safepoints(encoder)?;
        self.encode_callables(encoder)
    }
}

impl<D, C, I> RuntimeImageFingerprintInputV1<'_, D, C, I>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    fn encode_static_storages(
        &self,
        encoder: &mut RuntimeEncoder,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.static_storages();
        let plans = fingerprints
            .shapes()
            .storage_definitions()
            .registration_objects()
            .registrations()
            .plan()
            .registrations();
        encoder.sequence_length(plans.len())?;
        for (plan, fingerprint) in plans.iter().zip(fingerprints.fingerprints()) {
            runtime_encode_strong_static_storage_record_v1(
                encoder,
                plan,
                fingerprint.registration().as_array(),
                fingerprint.layout(),
                fingerprint.scan(),
            )?;
        }
        Ok(())
    }

    fn encode_immortal_objects(
        &self,
        encoder: &mut RuntimeEncoder,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.immortal_objects();
        let plans = fingerprints
            .object_definitions()
            .registration_objects()
            .registrations()
            .plan()
            .registrations();
        encoder.sequence_length(plans.len())?;
        for (plan, fingerprint) in plans.iter().zip(fingerprints.fingerprints()) {
            runtime_encode_strong_immortal_object_record_v1(
                encoder,
                plan.object(),
                plan.object_size(),
                plan.required_alignment(),
                plan.type_registration(),
                plan.definition_owner(),
                fingerprint.registration().as_array(),
            )?;
        }
        Ok(())
    }

    fn encode_initializations(
        &self,
        encoder: &mut RuntimeEncoder,
    ) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.initializations();
        let plans = fingerprints
            .definitions()
            .registration_objects()
            .registrations()
            .plan()
            .registrations();
        encoder.sequence_length(plans.len())?;
        for (plan, fingerprint) in plans.iter().zip(fingerprints.fingerprints()) {
            let gateway = fingerprint
                .gateway_body()
                .zip(fingerprint.gateway_definition());
            runtime_encode_strong_initialization_record_v1(
                encoder,
                plan,
                fingerprint.registration().as_array(),
                gateway,
            )?;
        }
        Ok(())
    }

    fn encode_types(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.types();
        let plans = fingerprints.registrations().plan().registrations();
        encoder.sequence_length(plans.len())?;
        for (plan, fingerprint) in plans.iter().zip(fingerprints.fingerprints()) {
            runtime_encode_type_record_v1(
                encoder,
                plan,
                fingerprint.registration().as_array(),
                fingerprint.descriptor_definition().as_array(),
                fingerprint.layout().as_array(),
            )?;
        }
        Ok(())
    }

    fn encode_safepoints(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.safepoints();
        let plans = fingerprints.registrations().plan().registrations();
        encoder.sequence_length(plans.len())?;
        for (plan, fingerprint) in plans.iter().zip(fingerprints.fingerprints()) {
            runtime_encode_safepoint_record_v1(
                encoder,
                *plan,
                fingerprint.registration().as_array(),
                fingerprint.stackmap().as_array(),
            )?;
        }
        Ok(())
    }

    fn encode_callables(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let fingerprints = self.registrations.callables();
        let plans = fingerprints
            .body_objects()
            .registration_objects()
            .registrations()
            .plan()
            .registrations();
        encoder.sequence_length(plans.len())?;
        for (plan, fingerprint) in plans.iter().zip(fingerprints.fingerprints()) {
            runtime_encode_callable_record_v1(
                encoder,
                *plan,
                fingerprint.registration().as_array(),
                fingerprint.body_definition().as_array(),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeImageTableKindV1 {
    StaticStorages,
    ImmortalObjects,
    InitializationUnits,
    TypeRegistrations,
    Safepoints,
    Callables,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeImageFingerprintError {
    ProducerMismatch {
        image: ConeIdentity,
        registrations: ConeIdentity,
    },
    ObjectProofMismatch,
    TableMismatch(ConeImageTableKindV1),
    MissingDigestNode,
    DigestInputMismatch,
    Hash(HashError),
}

impl fmt::Display for RuntimeImageFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute runtime image fingerprint: {self:?}"
        )
    }
}

impl std::error::Error for RuntimeImageFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}
