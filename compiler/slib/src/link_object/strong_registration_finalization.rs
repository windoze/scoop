use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    PersistentCallableBodyId, PersistentExactTypeId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentSafepointSiteId, PersistentStaticStorageId,
};
use scoop_wire::sha256;

use super::initialization_registrations::record::expected_final_record as expected_final_initialization_record;
use super::safepoint_registrations::physical::{validate_objects, verified_member};
use super::safepoint_registrations::record::expected_final_record as expected_final_safepoint_record;
use super::static_storage_registrations::record::expected_final_record as expected_final_static_storage_record;
use super::{StrongSafepointRegistrationValidationError, VerifiedStrongSafepointFingerprintSetV1};
use crate::SlibMemberId;
use crate::link_object::{
    ScoopLirObjectCandidateV1, ScoopLirObjectEnvelopeValidationError,
    ValidatedBuiltinObjectSectionInventoryV1, ValidatedScoopLirObjectEnvelopeV1,
    VerifiedMaterializedPatchSiteV1, VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongImmortalObjectFingerprintSetV1, VerifiedStrongInitializationFingerprintSetV1,
    VerifiedStrongStaticStorageFingerprintSetV1, VerifiedStrongTypeFingerprintSetV1,
    callable_registrations::record::expected_final_record as expected_final_callable_record,
    immortal_registrations::record::expected_final_record as expected_final_immortal_object_record,
    type_registrations::record::expected_final_record as expected_final_type_record,
    validate_scoop_lir_llvm_22_1_object_envelope_v1,
};

mod coverage;
mod normalization;
mod patching;
mod records;

use coverage::validate_proof_coverage;
pub(in crate::link_object) use normalization::same_object_shape;
use normalization::verify_normalized_content;
#[cfg(test)]
pub(in crate::link_object) use patching::patch_initializations_for_test;
use patching::*;
use records::*;

const DIGEST_WIDTH: usize = 32;

/// One copied Scoop LIR object whose verified strong-registration digest slots
/// have been patched and whose final Mach-O envelope has been revalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongRegistrationPatchedScoopLirObjectV1 {
    member: SlibMemberId,
    bytes: Vec<u8>,
    envelope: ValidatedScoopLirObjectEnvelopeV1,
}

impl VerifiedStrongRegistrationPatchedScoopLirObjectV1 {
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

/// Typed result of applying every verified strong-registration digest to
/// copied object bytes. This is not yet the full digest-graph finalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongRegistrationPatchSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = PersistentInitializationUnitId,
> {
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1<D, C>,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1<I>,
    objects: Vec<VerifiedStrongRegistrationPatchedScoopLirObjectV1>,
}

pub type VerifiedStrongRegistrationPatchSetV2 = VerifiedStrongRegistrationPatchSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I>
    VerifiedStrongRegistrationPatchSetV1<D, C, I>
{
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.safepoints.producer()
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableFingerprintSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &VerifiedStrongTypeFingerprintSetV1<D, C> {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &VerifiedStrongImmortalObjectFingerprintSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &VerifiedStrongStaticStorageFingerprintSetV1 {
        &self.static_storages
    }

    pub const fn initializations(&self) -> &VerifiedStrongInitializationFingerprintSetV1<I> {
        &self.initializations
    }

    pub fn objects(&self) -> &[VerifiedStrongRegistrationPatchedScoopLirObjectV1] {
        &self.objects
    }
}

/// Copy the exact verified provisional objects, write every declared strong
/// registration slot, and revalidate normalized content and shape.
pub fn patch_strong_registration_fingerprints_v1(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongRegistrationPatchSetV1, StrongRegistrationPatchError> {
    patch_strong_registration_fingerprints(
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        scoop_objects,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn patch_strong_registration_fingerprints_v2(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: crate::VerifiedStrongTypeFingerprintSetV2,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: crate::VerifiedStrongInitializationFingerprintSetV2,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongRegistrationPatchSetV2, StrongRegistrationPatchError> {
    patch_strong_registration_fingerprints(
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        scoop_objects,
    )
}

#[allow(clippy::too_many_arguments)]
fn patch_strong_registration_fingerprints<D, C, I>(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1<D, C>,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1<I>,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongRegistrationPatchSetV1<D, C, I>, StrongRegistrationPatchError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let safepoint_registrations = safepoints.registrations();
    let callable_body_objects = callables.body_objects();
    let callable_registrations = callable_body_objects.registrations();
    let type_registrations = types.registrations();
    let immortal_registrations = immortal_objects.registrations();
    let static_storage_registrations = static_storages.shapes().registrations();
    let initialization_registrations = initializations.registrations();
    if safepoint_registrations.patch_sites() != callable_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != type_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != immortal_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != static_storage_registrations.patch_sites()
        || safepoint_registrations.patch_sites() != initialization_registrations.patch_sites()
        || safepoint_registrations.stackmaps() != callable_body_objects.stackmaps()
    {
        return Err(StrongRegistrationPatchError::ProofMismatch);
    }
    let builtins = safepoint_registrations.stackmaps().builtins();
    validate_objects(builtins, scoop_objects)
        .map_err(StrongRegistrationPatchError::ObjectValidation)?;
    validate_proof_coverage(
        &safepoints,
        &callables,
        &types,
        &immortal_objects,
        &static_storages,
        &initializations,
    )?;

    let mut objects = scoop_objects
        .iter()
        .map(|object| (object.member(), object.bytes().to_vec()))
        .collect::<Vec<_>>();
    let object_indexes = objects
        .iter()
        .enumerate()
        .map(|(index, (member, _))| (*member, index))
        .collect::<BTreeMap<_, _>>();

    patch_safepoints(&safepoints, &object_indexes, &mut objects)?;
    patch_callables(&callables, &object_indexes, &mut objects)?;
    patch_types(&types, &object_indexes, &mut objects)?;
    patch_immortal_objects(&immortal_objects, &object_indexes, &mut objects)?;
    patch_static_storages(&static_storages, &object_indexes, &mut objects)?;
    patch_initializations(&initializations, &object_indexes, &mut objects)?;

    let mut finalized = Vec::with_capacity(objects.len());
    for (member, bytes) in objects {
        verify_normalized_content(
            member,
            &bytes,
            &safepoints,
            &callables,
            &types,
            &static_storages,
            &initializations,
        )?;
        let envelope = validate_scoop_lir_llvm_22_1_object_envelope_v1(
            builtins.member_plan().target(),
            &bytes,
        )
        .map_err(|source| StrongRegistrationPatchError::FinalEnvelope { member, source })?;
        let original = verified_member(builtins, member)
            .map_err(StrongRegistrationPatchError::ObjectValidation)?
            .definitions()
            .sections();
        if !same_object_shape(original, envelope.sections()) {
            return Err(StrongRegistrationPatchError::MachOShapeChanged(member));
        }
        finalized.push(VerifiedStrongRegistrationPatchedScoopLirObjectV1 {
            member,
            bytes,
            envelope,
        });
    }

    Ok(VerifiedStrongRegistrationPatchSetV1 {
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        objects: finalized,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationPatchOwnerV1 {
    Safepoint(PersistentSafepointSiteId),
    Callable(PersistentCallableBodyId),
    Type(PersistentExactTypeId),
    ImmortalObject(PersistentImmortalObjectId),
    StaticStorage(PersistentStaticStorageId),
    InitializationUnit(PersistentInitializationUnitId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationPatchFieldV1 {
    NormalizedStackmap,
    CallableBodyDefinition,
    DescriptorDefinition,
    Layout,
    Scan,
    GatewayDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongRegistrationPatchError {
    ProofMismatch,
    ObjectValidation(StrongSafepointRegistrationValidationError),
    ProofCoverageMismatch,
    ProofOwnerMismatch {
        owner: StrongRegistrationPatchOwnerV1,
    },
    MissingObject(SlibMemberId),
    PatchRange {
        owner: StrongRegistrationPatchOwnerV1,
        field: StrongRegistrationPatchFieldV1,
    },
    NonZeroPatchSlot {
        owner: StrongRegistrationPatchOwnerV1,
        field: StrongRegistrationPatchFieldV1,
        offset: u8,
    },
    RecordRange(StrongRegistrationPatchOwnerV1),
    FinalRecordMismatch {
        owner: StrongRegistrationPatchOwnerV1,
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

impl fmt::Display for StrongRegistrationPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to patch strong registration digests: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationPatchError {
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
