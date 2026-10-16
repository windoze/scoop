use std::fmt;

mod encoding;
use encoding::RuntimeImageFingerprintInputV1;

use scoop_identity::ConeIdentity;
use scoop_lir::{RuntimeAbiFingerprint, TargetProfileFingerprint};
use scoop_wire::{HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder};

use super::VerifiedConeImageV1;
use crate::link_object::{
    runtime_encode_callable_record_v1, runtime_encode_safepoint_record_v1,
    runtime_encode_strong_immortal_object_record_v1,
    runtime_encode_strong_initialization_record_v1, runtime_encode_strong_static_storage_record_v1,
    runtime_encode_type_record_v1,
};
use crate::{CompatibilityRecord, RuntimeImageFingerprint, VerifiedStrongRegistrationPatchSetV1};

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
    records: crate::RuntimeImageRecords,
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

    pub const fn records(&self) -> &crate::RuntimeImageRecords {
        &self.records
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
    let records = RuntimeImageFingerprintInputV1 {
        image: &image,
        registrations: &registrations,
        runtime_abi: compatibility.runtime_abi(),
        target_profile: compatibility.target_fingerprint(),
    }
    .records()
    .map_err(|_| RuntimeImageFingerprintError::Hash(HashError::RuntimeEncoding))?;
    let fingerprint = records
        .fingerprint()
        .map_err(RuntimeImageFingerprintError::Hash)?;
    Ok(VerifiedRuntimeImageFingerprintV1 {
        image,
        registrations,
        compatibility,
        fingerprint,
        records,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeImageFingerprintError {
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
        }
    }
}
