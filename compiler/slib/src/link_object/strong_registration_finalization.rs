use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    PersistentCallableBodyId, PersistentExactTypeId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentSafepointSiteId, PersistentStaticStorageId,
};

use super::safepoint_registrations::physical::verified_member;
use super::{StrongSafepointRegistrationValidationError, VerifiedStrongSafepointFingerprintSetV1};
use crate::SlibMemberId;
use crate::link_object::{
    ScoopLirObjectCandidateV1, ValidatedScoopLirObjectEnvelopeV1, VerifiedMaterializedPatchSiteV1,
    VerifiedStrongCallableFingerprintSetV1, VerifiedStrongImmortalObjectFingerprintSetV1,
    VerifiedStrongInitializationFingerprintSetV1, VerifiedStrongStaticStorageFingerprintSetV1,
    VerifiedStrongTypeFingerprintSetV1,
};

mod patching;

#[cfg(test)]
pub(in crate::link_object) use patching::patch_initializations_for_test;
use patching::*;

const DIGEST_WIDTH: usize = 32;

/// A Scoop LIR object with final registration digests and its reused parsed structure.
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

/// Write the declared registration digest slots and update the content fingerprint.
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
    let builtins = safepoints.registrations().patch_sites().builtins();

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
    patch_static_storages(&static_storages, &object_indexes, &mut objects)?;
    patch_initializations(&initializations, &object_indexes, &mut objects)?;

    let mut finalized = Vec::with_capacity(objects.len());
    for (member, bytes) in objects {
        let original = verified_member(builtins, member)
            .map_err(StrongRegistrationPatchError::ObjectValidation)?
            .definitions()
            .sections();
        let envelope =
            ValidatedScoopLirObjectEnvelopeV1::after_digest_patch(original.clone(), &bytes);
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
    ObjectValidation(StrongSafepointRegistrationValidationError),
    MissingObject(SlibMemberId),
    MissingGatewayDigest(PersistentInitializationUnitId),
    PatchRange {
        owner: StrongRegistrationPatchOwnerV1,
        field: StrongRegistrationPatchFieldV1,
    },
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
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
