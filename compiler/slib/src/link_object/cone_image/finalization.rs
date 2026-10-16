use std::fmt;

use super::VerifiedRuntimeImageFingerprintV1;
use crate::SlibMemberId;
use crate::link_object::{DigestPatchSiteV1, ValidatedScoopLirObjectEnvelopeV1};

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
    let mut objects = source_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();
    let image_object = objects
        .iter_mut()
        .find(|(member, _)| *member == image_member)
        .ok_or(RuntimeImagePatchError::MissingImageObject(image_member))?;
    write_runtime_image_digest(
        &mut image_object.1,
        image_patch,
        fingerprint.fingerprint().as_array(),
    )?;

    let mut finalized = Vec::with_capacity(objects.len());
    for ((member, bytes), source) in objects.into_iter().zip(source_objects) {
        let envelope = if member == image_member {
            ValidatedScoopLirObjectEnvelopeV1::after_digest_patch(
                source.envelope().sections().clone(),
                &bytes,
            )
        } else {
            source.envelope().clone()
        };
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

pub(super) fn write_runtime_image_digest(
    object: &mut [u8],
    patch: DigestPatchSiteV1,
    fingerprint: &[u8; DIGEST_WIDTH],
) -> Result<(), RuntimeImagePatchError> {
    let slot = digest_slot_mut(object, patch.checked_offset())?;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeImagePatchError {
    MissingImageObject(SlibMemberId),
    PatchRange,
}

impl fmt::Display for RuntimeImagePatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to patch runtime image fingerprint: {self:?}"
        )
    }
}

impl std::error::Error for RuntimeImagePatchError {}
