use std::fmt;

use super::VerifiedRuntimeImageFingerprintV1;
use super::record::{IMAGE_DESCRIPTOR_SIZE, expected_final_image_record};
use crate::SlibMemberId;
use crate::link_object::{
    ScoopLirObjectEnvelopeValidationError, ValidatedScoopLirObjectEnvelopeV1,
    VerifiedMaterializedPatchSiteV1, same_macho_shape,
    validate_scoop_lir_llvm_22_1_object_envelope_v1,
};

const DIGEST_WIDTH: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeImagePatchedScoopLirObjectV1 {
    member: SlibMemberId,
    bytes: Vec<u8>,
    envelope: ValidatedScoopLirObjectEnvelopeV1,
}

impl VerifiedRuntimeImagePatchedScoopLirObjectV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn envelope(&self) -> &ValidatedScoopLirObjectEnvelopeV1 {
        &self.envelope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeImagePatchSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = scoop_identity::PersistentInitializationUnitId,
> {
    fingerprint: VerifiedRuntimeImageFingerprintV1<D, C, I>,
    objects: Vec<VerifiedRuntimeImagePatchedScoopLirObjectV1>,
}

pub type VerifiedRuntimeImagePatchSetV2 = VerifiedRuntimeImagePatchSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I> VerifiedRuntimeImagePatchSetV1<D, C, I> {
    pub const fn fingerprint(&self) -> &VerifiedRuntimeImageFingerprintV1<D, C, I> {
        &self.fingerprint
    }

    pub fn objects(&self) -> &[VerifiedRuntimeImagePatchedScoopLirObjectV1] {
        &self.objects
    }
}

pub fn patch_runtime_image_fingerprint_v1(
    fingerprint: VerifiedRuntimeImageFingerprintV1,
) -> Result<VerifiedRuntimeImagePatchSetV1, RuntimeImagePatchError> {
    patch_runtime_image_fingerprint(fingerprint)
}

pub fn patch_runtime_image_fingerprint_v2(
    fingerprint: super::VerifiedRuntimeImageFingerprintV2,
) -> Result<VerifiedRuntimeImagePatchSetV2, RuntimeImagePatchError> {
    patch_runtime_image_fingerprint(fingerprint)
}

fn patch_runtime_image_fingerprint<D, C, I>(
    fingerprint: VerifiedRuntimeImageFingerprintV1<D, C, I>,
) -> Result<VerifiedRuntimeImagePatchSetV1<D, C, I>, RuntimeImagePatchError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let image = fingerprint.image();
    let source_objects = fingerprint.registrations().objects();
    let image_member = image.member();
    let image_patch = image.image_patch();
    if image_patch.member() != image_member {
        return Err(RuntimeImagePatchError::PatchMemberMismatch {
            image: image_member,
            patch: image_patch.member(),
        });
    }

    let mut objects = source_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();
    let image_object_count = objects
        .iter()
        .filter(|(member, _)| *member == image_member)
        .count();
    if image_object_count != 1 {
        return Err(RuntimeImagePatchError::ImageObjectCount {
            member: image_member,
            actual: image_object_count,
        });
    }
    let image_object = objects
        .iter_mut()
        .find(|(member, _)| *member == image_member)
        .expect("the exact image object count was validated");
    write_runtime_image_digest(
        &mut image_object.1,
        image_patch,
        fingerprint.fingerprint().as_array(),
    )?;
    validate_final_image_record(
        &image_object.1,
        image.primary().checked_offset(),
        &expected_final_image_record(image.plan(), fingerprint.fingerprint().as_array()),
    )?;

    let mut finalized = Vec::with_capacity(objects.len());
    for ((member, bytes), source) in objects.into_iter().zip(source_objects) {
        if member != source.member() {
            return Err(RuntimeImagePatchError::ObjectOrderMismatch);
        }
        verify_only_image_slot_changed(member, &bytes, source.bytes(), image_member, image_patch)?;
        let envelope = validate_scoop_lir_llvm_22_1_object_envelope_v1(&bytes)
            .map_err(|source| RuntimeImagePatchError::FinalEnvelope { member, source })?;
        if !same_macho_shape(source.envelope().sections(), envelope.sections()) {
            return Err(RuntimeImagePatchError::MachOShapeChanged(member));
        }
        finalized.push(VerifiedRuntimeImagePatchedScoopLirObjectV1 {
            member,
            bytes,
            envelope,
        });
    }

    Ok(VerifiedRuntimeImagePatchSetV1 {
        fingerprint,
        objects: finalized,
    })
}

fn write_runtime_image_digest(
    object: &mut [u8],
    patch: VerifiedMaterializedPatchSiteV1,
    fingerprint: &[u8; DIGEST_WIDTH],
) -> Result<(), RuntimeImagePatchError> {
    let slot = digest_slot_mut(object, patch.checked_offset())?;
    if let Some(offset) = slot.iter().position(|byte| *byte != 0) {
        return Err(RuntimeImagePatchError::NonZeroPatchSlot {
            offset: u8::try_from(offset).expect("digest width fits u8"),
        });
    }
    slot.copy_from_slice(fingerprint);
    Ok(())
}

fn digest_slot_mut(
    object: &mut [u8],
    checked_offset: u64,
) -> Result<&mut [u8], RuntimeImagePatchError> {
    let start = usize::try_from(checked_offset).map_err(|_| RuntimeImagePatchError::PatchRange)?;
    let end = start
        .checked_add(DIGEST_WIDTH)
        .ok_or(RuntimeImagePatchError::PatchRange)?;
    object
        .get_mut(start..end)
        .ok_or(RuntimeImagePatchError::PatchRange)
}

fn validate_final_image_record(
    object: &[u8],
    checked_offset: u64,
    expected: &[u8; IMAGE_DESCRIPTOR_SIZE],
) -> Result<(), RuntimeImagePatchError> {
    let start =
        usize::try_from(checked_offset).map_err(|_| RuntimeImagePatchError::FinalRecordRange)?;
    let end = start
        .checked_add(IMAGE_DESCRIPTOR_SIZE)
        .ok_or(RuntimeImagePatchError::FinalRecordRange)?;
    let actual = object
        .get(start..end)
        .ok_or(RuntimeImagePatchError::FinalRecordRange)?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(RuntimeImagePatchError::FinalRecordMismatch {
            offset: u16::try_from(offset).expect("image descriptor size fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

fn verify_only_image_slot_changed(
    member: SlibMemberId,
    final_bytes: &[u8],
    source_bytes: &[u8],
    image_member: SlibMemberId,
    image_patch: VerifiedMaterializedPatchSiteV1,
) -> Result<(), RuntimeImagePatchError> {
    if member != image_member {
        if final_bytes != source_bytes {
            return Err(RuntimeImagePatchError::NormalizedObjectMismatch(member));
        }
        return Ok(());
    }
    let mut normalized = final_bytes.to_vec();
    digest_slot_mut(&mut normalized, image_patch.checked_offset())?.fill(0);
    if normalized != source_bytes {
        return Err(RuntimeImagePatchError::NormalizedObjectMismatch(member));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeImagePatchError {
    PatchMemberMismatch {
        image: SlibMemberId,
        patch: SlibMemberId,
    },
    ImageObjectCount {
        member: SlibMemberId,
        actual: usize,
    },
    ObjectOrderMismatch,
    PatchRange,
    NonZeroPatchSlot {
        offset: u8,
    },
    FinalRecordRange,
    FinalRecordMismatch {
        offset: u16,
        expected: u8,
        actual: u8,
    },
    NormalizedObjectMismatch(SlibMemberId),
    FinalEnvelope {
        member: SlibMemberId,
        source: ScoopLirObjectEnvelopeValidationError,
    },
    MachOShapeChanged(SlibMemberId),
}

impl fmt::Display for RuntimeImagePatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to patch runtime image fingerprint: {self:?}"
        )
    }
}

impl std::error::Error for RuntimeImagePatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FinalEnvelope { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(in crate::link_object::cone_image) fn patch_image_bytes_for_test(
    object: &mut [u8],
    image_offset: u64,
    patch: VerifiedMaterializedPatchSiteV1,
    fingerprint: &[u8; 32],
    expected: &[u8; IMAGE_DESCRIPTOR_SIZE],
) -> Result<(), RuntimeImagePatchError> {
    write_runtime_image_digest(object, patch, fingerprint)?;
    validate_final_image_record(object, image_offset, expected)
}
