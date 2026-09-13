use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::PersistentSafepointSiteId;
use scoop_wire::sha256;

use super::physical::{validate_objects, verified_member};
use super::record::{DESCRIPTOR_SIZE, expected_final_record};
use super::{StrongSafepointRegistrationValidationError, VerifiedStrongSafepointFingerprintSetV1};
use crate::SlibMemberId;
use crate::link_object::{
    ScoopLirObjectCandidateV1, ScoopLirObjectEnvelopeValidationError,
    ValidatedBuiltinObjectSectionInventoryV1, ValidatedScoopLirObjectEnvelopeV1,
    VerifiedMaterializedPatchSiteV1, validate_scoop_lir_llvm_22_1_object_envelope_v1,
};

const DIGEST_WIDTH: usize = 32;

/// One copied Scoop LIR object whose strong safepoint digest slots have been
/// patched and whose final Mach-O envelope has been revalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSafepointPatchedScoopLirObjectV1 {
    member: SlibMemberId,
    bytes: Vec<u8>,
    envelope: ValidatedScoopLirObjectEnvelopeV1,
}

impl VerifiedSafepointPatchedScoopLirObjectV1 {
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

/// Typed result of applying every verified safepoint digest to copied object
/// bytes. This is not yet the full digest-graph finalization proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointPatchSetV1 {
    fingerprints: VerifiedStrongSafepointFingerprintSetV1,
    objects: Vec<VerifiedSafepointPatchedScoopLirObjectV1>,
}

impl VerifiedStrongSafepointPatchSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.fingerprints.producer()
    }

    pub const fn fingerprints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.fingerprints
    }

    pub fn objects(&self) -> &[VerifiedSafepointPatchedScoopLirObjectV1] {
        &self.objects
    }
}

/// Copy the exact verified provisional objects, write only the two declared
/// safepoint slots, and revalidate both normalized content and Mach-O shape.
pub fn patch_strong_safepoint_fingerprints_v1(
    fingerprints: VerifiedStrongSafepointFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongSafepointPatchSetV1, StrongSafepointPatchError> {
    let builtins = fingerprints.registrations().stackmaps().builtins();
    validate_objects(builtins, scoop_objects)
        .map_err(StrongSafepointPatchError::ObjectValidation)?;
    let verified_registrations = fingerprints.registrations().registrations();
    let planned_registrations = fingerprints.registrations().plan().registrations();
    let computed_fingerprints = fingerprints.fingerprints();
    if verified_registrations.len() != planned_registrations.len()
        || verified_registrations.len() != computed_fingerprints.len()
    {
        return Err(StrongSafepointPatchError::ProofCoverageMismatch);
    }

    let mut objects = scoop_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();
    let object_indexes = objects
        .iter()
        .enumerate()
        .map(|(index, (member, _))| (*member, index))
        .collect::<BTreeMap<_, _>>();

    for ((verified, plan), computed) in verified_registrations
        .iter()
        .zip(planned_registrations)
        .zip(computed_fingerprints)
    {
        if verified.site() != plan.site() || verified.site() != computed.site() {
            return Err(StrongSafepointPatchError::ProofSiteMismatch {
                verified: verified.site(),
                planned: plan.site(),
                computed: computed.site(),
            });
        }
        let index = object_indexes
            .get(&verified.member())
            .copied()
            .ok_or(StrongSafepointPatchError::MissingObject(verified.member()))?;
        let bytes = &mut objects[index].1;
        write_digest(
            bytes,
            verified.normalized_stackmap_patch(),
            computed.stackmap().as_array(),
            plan.site(),
            SafepointPatchFieldV1::NormalizedStackmap,
        )?;
        write_digest(
            bytes,
            verified.registration_definition_patch(),
            computed.registration().as_array(),
            plan.site(),
            SafepointPatchFieldV1::RegistrationDefinition,
        )?;
        validate_final_record(bytes, verified.checked_offset(), *plan, computed)?;
    }

    let mut finalized = Vec::with_capacity(objects.len());
    for (member, bytes) in objects {
        verify_normalized_content(member, &bytes, &fingerprints)?;
        let envelope = validate_scoop_lir_llvm_22_1_object_envelope_v1(&bytes)
            .map_err(|source| StrongSafepointPatchError::FinalEnvelope { member, source })?;
        let original = verified_member(builtins, member)
            .map_err(StrongSafepointPatchError::ObjectValidation)?
            .definitions()
            .sections();
        if !same_macho_shape(original, envelope.sections()) {
            return Err(StrongSafepointPatchError::MachOShapeChanged(member));
        }
        finalized.push(VerifiedSafepointPatchedScoopLirObjectV1 {
            member,
            bytes,
            envelope,
        });
    }

    Ok(VerifiedStrongSafepointPatchSetV1 {
        fingerprints,
        objects: finalized,
    })
}

fn write_digest(
    object: &mut [u8],
    patch: VerifiedMaterializedPatchSiteV1,
    digest: &[u8; DIGEST_WIDTH],
    site: PersistentSafepointSiteId,
    field: SafepointPatchFieldV1,
) -> Result<(), StrongSafepointPatchError> {
    let start = usize::try_from(patch.checked_offset())
        .map_err(|_| StrongSafepointPatchError::PatchRange { site, field })?;
    let end = start
        .checked_add(DIGEST_WIDTH)
        .ok_or(StrongSafepointPatchError::PatchRange { site, field })?;
    let slot = object
        .get_mut(start..end)
        .ok_or(StrongSafepointPatchError::PatchRange { site, field })?;
    if let Some(offset) = slot.iter().position(|byte| *byte != 0) {
        return Err(StrongSafepointPatchError::NonZeroPatchSlot {
            site,
            field,
            offset: u8::try_from(offset).expect("digest width fits u8"),
        });
    }
    slot.copy_from_slice(digest);
    Ok(())
}

fn validate_final_record(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    computed: &super::VerifiedStrongSafepointFingerprintV1,
) -> Result<(), StrongSafepointPatchError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongSafepointPatchError::RecordRange(plan.site()))?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongSafepointPatchError::RecordRange(plan.site()))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongSafepointPatchError::RecordRange(plan.site()))?;
    let expected = expected_final_record(
        plan,
        computed.registration().as_array(),
        computed.stackmap().as_array(),
    );
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(StrongSafepointPatchError::FinalRecordMismatch {
            site: plan.site(),
            offset: u16::try_from(offset).expect("descriptor offset fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

fn verify_normalized_content(
    member: SlibMemberId,
    final_bytes: &[u8],
    fingerprints: &VerifiedStrongSafepointFingerprintSetV1,
) -> Result<(), StrongSafepointPatchError> {
    let mut normalized = final_bytes.to_vec();
    for registration in fingerprints
        .registrations()
        .registrations()
        .iter()
        .filter(|registration| registration.member() == member)
    {
        for patch in [
            registration.registration_definition_patch(),
            registration.normalized_stackmap_patch(),
        ] {
            let start = usize::try_from(patch.checked_offset())
                .map_err(|_| StrongSafepointPatchError::NormalizedObjectRange(member))?;
            let end = start
                .checked_add(DIGEST_WIDTH)
                .ok_or(StrongSafepointPatchError::NormalizedObjectRange(member))?;
            normalized
                .get_mut(start..end)
                .ok_or(StrongSafepointPatchError::NormalizedObjectRange(member))?
                .fill(0);
        }
    }
    let original = verified_member(fingerprints.registrations().stackmaps().builtins(), member)
        .map_err(StrongSafepointPatchError::ObjectValidation)?
        .definitions()
        .sections()
        .envelope();
    if u64::try_from(normalized.len()).ok() != Some(original.byte_length())
        || sha256(&normalized) != original.content_digest()
    {
        return Err(StrongSafepointPatchError::NormalizedObjectMismatch(member));
    }
    Ok(())
}

fn same_macho_shape(
    original: &ValidatedBuiltinObjectSectionInventoryV1,
    final_inventory: &ValidatedBuiltinObjectSectionInventoryV1,
) -> bool {
    let original_envelope = original.envelope();
    let final_envelope = final_inventory.envelope();
    original.profile() == final_inventory.profile()
        && original.roles() == final_inventory.roles()
        && original_envelope.byte_length() == final_envelope.byte_length()
        && original_envelope.load_command_count() == final_envelope.load_command_count()
        && original_envelope.section_count() == final_envelope.section_count()
        && original_envelope.symbol_count() == final_envelope.symbol_count()
        && original_envelope.relocation_count() == final_envelope.relocation_count()
        && original_envelope.deployment() == final_envelope.deployment()
        && original_envelope.sections() == final_envelope.sections()
        && original_envelope.symbols() == final_envelope.symbols()
        && original_envelope.relocations() == final_envelope.relocations()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointPatchFieldV1 {
    RegistrationDefinition,
    NormalizedStackmap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongSafepointPatchError {
    ObjectValidation(StrongSafepointRegistrationValidationError),
    ProofCoverageMismatch,
    ProofSiteMismatch {
        verified: PersistentSafepointSiteId,
        planned: PersistentSafepointSiteId,
        computed: PersistentSafepointSiteId,
    },
    MissingObject(SlibMemberId),
    PatchRange {
        site: PersistentSafepointSiteId,
        field: SafepointPatchFieldV1,
    },
    NonZeroPatchSlot {
        site: PersistentSafepointSiteId,
        field: SafepointPatchFieldV1,
        offset: u8,
    },
    RecordRange(PersistentSafepointSiteId),
    FinalRecordMismatch {
        site: PersistentSafepointSiteId,
        offset: u16,
        expected: u8,
        actual: u8,
    },
    NormalizedObjectRange(SlibMemberId),
    NormalizedObjectMismatch(SlibMemberId),
    FinalEnvelope {
        member: SlibMemberId,
        source: ScoopLirObjectEnvelopeValidationError,
    },
    MachOShapeChanged(SlibMemberId),
}

impl fmt::Display for StrongSafepointPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to patch strong safepoint digests: {self:?}"
        )
    }
}

impl std::error::Error for StrongSafepointPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::FinalEnvelope { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
