use std::fmt;

use super::record::{ROOT_ENTRY_DESCRIPTOR_SIZE, expected_final_root_entry_record};
use super::{VerifiedEntryProductionBranchV1, VerifiedEntryProductionV1};
use crate::SlibMemberId;
use crate::link_object::{
    ScoopLirObjectEnvelopeValidationError, ValidatedScoopLirObjectEnvelopeV1,
    VerifiedMaterializedPatchSiteV1, VerifiedRuntimeImagePatchSetV1, same_object_shape,
    validate_scoop_lir_llvm_22_1_object_envelope_v1,
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
    if runtime_images.fingerprint().image().patch_sites() != entry.patch_sites() {
        return Err(EntryPatchError::ObjectProofMismatch);
    }
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
        validate_final_record(
            &object.1,
            verified.checked_offset(),
            &expected_final_root_entry_record(plan, gateway.body_definition().as_array()),
        )?;
    } else if !matches!(entry.plan(), scoop_lir::EntryProductionPlanV1::Library) {
        return Err(EntryPatchError::BranchMismatch);
    }

    let mut finalized = Vec::with_capacity(objects.len());
    for ((member, bytes), source) in objects.into_iter().zip(source_objects) {
        if member != source.member() {
            return Err(EntryPatchError::ObjectOrderMismatch);
        }
        verify_only_entry_slots_changed(member, &bytes, source.bytes(), entry.branch())?;
        let envelope = validate_scoop_lir_llvm_22_1_object_envelope_v1(
            scoop_lir::LirTargetProfile::from_id(source.envelope().sections().envelope().target()),
            &bytes,
        )
        .map_err(|source| EntryPatchError::FinalEnvelope { member, source })?;
        if !same_object_shape(source.envelope().sections(), envelope.sections()) {
            return Err(EntryPatchError::MachOShapeChanged(member));
        }
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
    let count = objects
        .iter()
        .filter(|(candidate, _)| *candidate == member)
        .count();
    if count != 1 {
        return Err(EntryPatchError::EntryObjectCount {
            member,
            actual: count,
        });
    }
    Ok(objects
        .iter_mut()
        .find(|(candidate, _)| *candidate == member)
        .expect("the exact entry object count was validated"))
}

fn write_digest(
    bytes: &mut [u8],
    patch: VerifiedMaterializedPatchSiteV1,
    digest: &[u8; DIGEST_WIDTH],
    field: EntryPatchFieldV1,
) -> Result<(), EntryPatchError> {
    let slot = digest_slot_mut(bytes, patch.checked_offset(), field)?;
    if let Some(offset) = slot.iter().position(|byte| *byte != 0) {
        return Err(EntryPatchError::NonZeroPatchSlot {
            field,
            offset: u8::try_from(offset).expect("digest width fits u8"),
        });
    }
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

fn validate_final_record(
    object: &[u8],
    checked_offset: u64,
    expected: &[u8; ROOT_ENTRY_DESCRIPTOR_SIZE],
) -> Result<(), EntryPatchError> {
    let start = usize::try_from(checked_offset).map_err(|_| EntryPatchError::FinalRecordRange)?;
    let end = start
        .checked_add(ROOT_ENTRY_DESCRIPTOR_SIZE)
        .ok_or(EntryPatchError::FinalRecordRange)?;
    let actual = object
        .get(start..end)
        .ok_or(EntryPatchError::FinalRecordRange)?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(EntryPatchError::FinalRecordMismatch {
            offset: u16::try_from(offset).expect("root descriptor size fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

fn verify_only_entry_slots_changed(
    member: SlibMemberId,
    final_bytes: &[u8],
    source_bytes: &[u8],
    branch: &VerifiedEntryProductionBranchV1,
) -> Result<(), EntryPatchError> {
    let mut normalized = final_bytes.to_vec();
    if let VerifiedEntryProductionBranchV1::Executable(entry) = branch
        && entry.member() == member
    {
        digest_slot_mut(
            &mut normalized,
            entry.source_signature_patch().checked_offset(),
            EntryPatchFieldV1::SourceSignature,
        )?
        .fill(0);
        digest_slot_mut(
            &mut normalized,
            entry.gateway_definition_patch().checked_offset(),
            EntryPatchFieldV1::GatewayDefinition,
        )?
        .fill(0);
    }
    if normalized != source_bytes {
        return Err(EntryPatchError::NormalizedObjectMismatch(member));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryPatchFieldV1 {
    SourceSignature,
    GatewayDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryPatchError {
    ObjectProofMismatch,
    BranchMismatch,
    MissingGatewayFingerprint(scoop_identity::PersistentCallableBodyId),
    EntryObjectCount {
        member: SlibMemberId,
        actual: usize,
    },
    ObjectOrderMismatch,
    PatchRange(EntryPatchFieldV1),
    NonZeroPatchSlot {
        field: EntryPatchFieldV1,
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

impl fmt::Display for EntryPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "failed to patch executable entry: {self:?}")
    }
}

impl std::error::Error for EntryPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FinalEnvelope { source, .. } => Some(source),
            _ => None,
        }
    }
}
