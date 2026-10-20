use std::fmt;

use scoop_identity::PersistentInitializationUnitId;

use super::VerifiedStrongSafepointFingerprintSetV1;
use crate::SlibMemberId;
use crate::link_object::{
    ImmortalObjectFingerprintSetV1, InitializationFingerprintSetV1, ScoopLirObjectCandidateV1,
    StaticStorageFingerprintSetV1, TypeFingerprintSetV1, ValidatedScoopLirObjectEnvelopeV1,
    VerifiedStrongCallableFingerprintSetV1,
};

/// Original Scoop object bytes and the parsed structure used by archive assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalScoopLirObjectV1 {
    member: SlibMemberId,
    bytes: Vec<u8>,
    envelope: ValidatedScoopLirObjectEnvelopeV1,
}

impl FinalScoopLirObjectV1 {
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

/// Registration content values and their original objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationObjectSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = PersistentInitializationUnitId,
> {
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: TypeFingerprintSetV1<D, C>,
    immortal_objects: ImmortalObjectFingerprintSetV1,
    static_storages: StaticStorageFingerprintSetV1,
    initializations: InitializationFingerprintSetV1<I>,
    objects: Vec<FinalScoopLirObjectV1>,
}

pub type RegistrationObjectSetV2 = RegistrationObjectSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I> RegistrationObjectSetV1<D, C, I> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.safepoints.producer()
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableFingerprintSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &TypeFingerprintSetV1<D, C> {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &ImmortalObjectFingerprintSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &StaticStorageFingerprintSetV1 {
        &self.static_storages
    }

    pub const fn initializations(&self) -> &InitializationFingerprintSetV1<I> {
        &self.initializations
    }

    pub fn objects(&self) -> &[FinalScoopLirObjectV1] {
        &self.objects
    }
}

/// Collect registration content values without modifying emitted objects.
pub fn collect_registration_objects_v1(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: TypeFingerprintSetV1,
    immortal_objects: ImmortalObjectFingerprintSetV1,
    static_storages: StaticStorageFingerprintSetV1,
    initializations: InitializationFingerprintSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<RegistrationObjectSetV1, RegistrationObjectError> {
    collect_registration_objects(
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
pub fn collect_registration_objects_v2(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: crate::TypeFingerprintSetV2,
    immortal_objects: ImmortalObjectFingerprintSetV1,
    static_storages: StaticStorageFingerprintSetV1,
    initializations: crate::InitializationFingerprintSetV2,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<RegistrationObjectSetV2, RegistrationObjectError> {
    collect_registration_objects(
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
fn collect_registration_objects<D, C, I>(
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: TypeFingerprintSetV1<D, C>,
    immortal_objects: ImmortalObjectFingerprintSetV1,
    static_storages: StaticStorageFingerprintSetV1,
    initializations: InitializationFingerprintSetV1<I>,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<RegistrationObjectSetV1<D, C, I>, RegistrationObjectError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let builtins = callables.body_objects().registrations().builtins();
    let mut finalized = Vec::with_capacity(scoop_objects.len());
    for object in scoop_objects {
        let member = object.member();
        let sections = builtins
            .member(member)
            .ok_or(RegistrationObjectError::MissingObject(member))?
            .definitions()
            .sections()
            .clone();
        finalized.push(FinalScoopLirObjectV1 {
            member,
            bytes: object.bytes().to_vec(),
            envelope: ValidatedScoopLirObjectEnvelopeV1::from_sections(sections),
        });
    }

    Ok(RegistrationObjectSetV1 {
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        objects: finalized,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationObjectError {
    MissingObject(SlibMemberId),
}

impl fmt::Display for RegistrationObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot collect registration objects: {self:?}")
    }
}

impl std::error::Error for RegistrationObjectError {}

#[cfg(test)]
mod tests;
