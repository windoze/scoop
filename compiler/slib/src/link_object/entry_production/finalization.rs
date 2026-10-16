use std::fmt;

use super::{VerifiedEntryProductionBranchV1, VerifiedEntryProductionV1};
use crate::SlibMemberId;
use crate::link_object::{
    ValidatedScoopLirObjectEnvelopeV1, VerifiedMaterializedPatchSiteV1,
    VerifiedRuntimeImagePatchSetV1,
};

const DIGEST_WIDTH: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEntryPatchedScoopLirObjectV1 {
    member: SlibMemberId,
    bytes: Vec<u8>,
    envelope: ValidatedScoopLirObjectEnvelopeV1,
}

impl VerifiedEntryPatchedScoopLirObjectV1 {
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
pub struct VerifiedEntryPatchSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = scoop_identity::PersistentInitializationUnitId,
> {
    runtime_images: VerifiedRuntimeImagePatchSetV1<D, C, I>,
    entry: VerifiedEntryProductionV1,
    objects: Vec<VerifiedEntryPatchedScoopLirObjectV1>,
}

pub type VerifiedEntryPatchSetV2 = VerifiedEntryPatchSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I> VerifiedEntryPatchSetV1<D, C, I> {
    pub const fn runtime_images(&self) -> &VerifiedRuntimeImagePatchSetV1<D, C, I> {
        &self.runtime_images
    }

    pub const fn entry(&self) -> &VerifiedEntryProductionV1 {
        &self.entry
    }

    pub fn objects(&self) -> &[VerifiedEntryPatchedScoopLirObjectV1] {
        &self.objects
    }
}

pub fn patch_entry_production_v1(
    runtime_images: VerifiedRuntimeImagePatchSetV1,
    entry: VerifiedEntryProductionV1,
) -> Result<VerifiedEntryPatchSetV1, EntryPatchError> {
    patch_entry_production(runtime_images, entry)
}

pub fn patch_entry_production_v2(
    runtime_images: crate::VerifiedRuntimeImagePatchSetV2,
    entry: VerifiedEntryProductionV1,
) -> Result<VerifiedEntryPatchSetV2, EntryPatchError> {
    patch_entry_production(runtime_images, entry)
}

fn patch_entry_production<D, C, I>(
    runtime_images: VerifiedRuntimeImagePatchSetV1<D, C, I>,
    entry: VerifiedEntryProductionV1,
) -> Result<VerifiedEntryPatchSetV1<D, C, I>, EntryPatchError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let source_objects = runtime_images.objects();
    let mut objects = source_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();

    if let VerifiedEntryProductionBranchV1::Executable(verified) = entry.branch() {
        let plan = match entry.plan() {
            scoop_lir::EntryProductionPlanV1::Executable(plan) => plan.as_ref(),
            scoop_lir::EntryProductionPlanV1::Library => {
                return Err(EntryPatchError::BranchMismatch);
            }
        };
        let gateway = runtime_images
            .fingerprint()
            .registrations()
            .callables()
            .fingerprints()
            .iter()
            .find(|fingerprint| fingerprint.body() == plan.gateway())
            .copied()
            .ok_or(EntryPatchError::MissingGatewayFingerprint(plan.gateway()))?;
        let object = unique_object_mut(&mut objects, verified.member())?;
        write_digest(
            &mut object.1,
            verified.source_signature_patch(),
            plan.source_signature_fingerprint().as_array(),
            EntryPatchFieldV1::SourceSignature,
        )?;
        write_digest(
            &mut object.1,
            verified.gateway_definition_patch(),
            gateway.body_definition().as_array(),
            EntryPatchFieldV1::GatewayDefinition,
        )?;
    } else if !matches!(entry.plan(), scoop_lir::EntryProductionPlanV1::Library) {
        return Err(EntryPatchError::BranchMismatch);
    }

    let mut finalized = Vec::with_capacity(objects.len());
    for ((member, bytes), source) in objects.into_iter().zip(source_objects) {
        let envelope = if matches!(entry.branch(), VerifiedEntryProductionBranchV1::Executable(entry) if entry.member() == member)
        {
            ValidatedScoopLirObjectEnvelopeV1::after_digest_patch(
                source.envelope().sections().clone(),
                &bytes,
            )
        } else {
            source.envelope().clone()
        };
        finalized.push(VerifiedEntryPatchedScoopLirObjectV1 {
            member,
            bytes,
            envelope,
        });
    }
    Ok(VerifiedEntryPatchSetV1 {
        runtime_images,
        entry,
        objects: finalized,
    })
}

fn unique_object_mut(
    objects: &mut [(SlibMemberId, Vec<u8>)],
    member: SlibMemberId,
) -> Result<&mut (SlibMemberId, Vec<u8>), EntryPatchError> {
    objects
        .iter_mut()
        .find(|(candidate, _)| *candidate == member)
        .ok_or(EntryPatchError::MissingEntryObject(member))
}

fn write_digest(
    bytes: &mut [u8],
    patch: VerifiedMaterializedPatchSiteV1,
    digest: &[u8; DIGEST_WIDTH],
    field: EntryPatchFieldV1,
) -> Result<(), EntryPatchError> {
    let slot = digest_slot_mut(bytes, patch.checked_offset(), field)?;
    slot.copy_from_slice(digest);
    Ok(())
}

fn digest_slot_mut(
    bytes: &mut [u8],
    checked_offset: u64,
    field: EntryPatchFieldV1,
) -> Result<&mut [u8], EntryPatchError> {
    let start = usize::try_from(checked_offset).map_err(|_| EntryPatchError::PatchRange(field))?;
    let end = start
        .checked_add(DIGEST_WIDTH)
        .ok_or(EntryPatchError::PatchRange(field))?;
    bytes
        .get_mut(start..end)
        .ok_or(EntryPatchError::PatchRange(field))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryPatchFieldV1 {
    SourceSignature,
    GatewayDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryPatchError {
    BranchMismatch,
    MissingGatewayFingerprint(scoop_identity::PersistentCallableBodyId),
    MissingEntryObject(SlibMemberId),
    PatchRange(EntryPatchFieldV1),
}

impl fmt::Display for EntryPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "failed to patch executable entry: {self:?}")
    }
}

impl std::error::Error for EntryPatchError {}
