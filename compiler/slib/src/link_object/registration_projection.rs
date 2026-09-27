//! Manifest projection of the Strong subset of the six registration tables.

use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentCallableBodyId, PersistentExactTypeId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentSafepointSiteId, PersistentStaticStorageId,
};
use scoop_wire::{Encoder, WireEncode};

use super::{
    RegistrationFingerprintV1, StrongRegistrationFingerprintV1,
    VerifiedStrongRegistrationPatchSetV1,
};

mod wire;
pub use wire::{
    DecodedCanonicalStrongRegistrationFingerprintSetV1,
    StrongRegistrationFingerprintSetValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongRegistrationFingerprintEntryV1<I> {
    semantic_id: I,
    fingerprint: StrongRegistrationFingerprintV1,
}

impl<I: Copy> StrongRegistrationFingerprintEntryV1<I> {
    pub const fn semantic_id(self) -> I {
        self.semantic_id
    }

    pub const fn fingerprint(self) -> StrongRegistrationFingerprintV1 {
        self.fingerprint
    }
}

impl<I: WireEncode> WireEncode for StrongRegistrationFingerprintEntryV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStrongRegistrationFingerprintSetV1 {
    producer: ConeIdentity,
    static_storages: Vec<StrongRegistrationFingerprintEntryV1<PersistentStaticStorageId>>,
    immortal_objects: Vec<StrongRegistrationFingerprintEntryV1<PersistentImmortalObjectId>>,
    initialization_units: Vec<StrongRegistrationFingerprintEntryV1<PersistentInitializationUnitId>>,
    type_registrations: Vec<StrongRegistrationFingerprintEntryV1<PersistentExactTypeId>>,
    safepoints: Vec<StrongRegistrationFingerprintEntryV1<PersistentSafepointSiteId>>,
    callables: Vec<StrongRegistrationFingerprintEntryV1<PersistentCallableBodyId>>,
}

impl CanonicalStrongRegistrationFingerprintSetV1 {
    pub fn from_patch_set<D, C, I>(
        patch_set: &VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    ) -> Result<Self, StrongRegistrationFingerprintProjectionError>
    where
        D: scoop_lir::StrongDescriptorReference,
        C: Clone,
    {
        Ok(Self {
            producer: patch_set.producer(),
            static_storages: canonicalize_table(
                StrongRegistrationFingerprintTableV1::StaticStorage,
                patch_set
                    .static_storages()
                    .fingerprints()
                    .iter()
                    .map(|fingerprint| entry(fingerprint.storage(), fingerprint.registration()))
                    .collect(),
            )?,
            immortal_objects: canonicalize_table(
                StrongRegistrationFingerprintTableV1::ImmortalObject,
                patch_set
                    .immortal_objects()
                    .fingerprints()
                    .iter()
                    .map(|fingerprint| entry(fingerprint.object(), fingerprint.registration()))
                    .collect(),
            )?,
            initialization_units: canonicalize_table(
                StrongRegistrationFingerprintTableV1::InitializationUnit,
                patch_set
                    .initializations()
                    .fingerprints()
                    .iter()
                    .map(|fingerprint| entry(fingerprint.unit(), fingerprint.registration()))
                    .collect(),
            )?,
            type_registrations: canonicalize_table(
                StrongRegistrationFingerprintTableV1::Type,
                patch_set
                    .types()
                    .fingerprints()
                    .iter()
                    .map(|fingerprint| entry(fingerprint.exact_type(), fingerprint.registration()))
                    .collect(),
            )?,
            safepoints: canonicalize_table(
                StrongRegistrationFingerprintTableV1::Safepoint,
                patch_set
                    .safepoints()
                    .fingerprints()
                    .iter()
                    .filter_map(|fingerprint| {
                        strong_entry(fingerprint.site(), fingerprint.registration())
                    })
                    .collect(),
            )?,
            callables: canonicalize_table(
                StrongRegistrationFingerprintTableV1::Callable,
                patch_set
                    .callables()
                    .fingerprints()
                    .iter()
                    .filter_map(|fingerprint| {
                        strong_entry(fingerprint.body(), fingerprint.registration())
                    })
                    .collect(),
            )?,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn static_storages(
        &self,
    ) -> &[StrongRegistrationFingerprintEntryV1<PersistentStaticStorageId>] {
        &self.static_storages
    }

    pub fn immortal_objects(
        &self,
    ) -> &[StrongRegistrationFingerprintEntryV1<PersistentImmortalObjectId>] {
        &self.immortal_objects
    }

    pub fn initialization_units(
        &self,
    ) -> &[StrongRegistrationFingerprintEntryV1<PersistentInitializationUnitId>] {
        &self.initialization_units
    }

    pub fn type_registrations(
        &self,
    ) -> &[StrongRegistrationFingerprintEntryV1<PersistentExactTypeId>] {
        &self.type_registrations
    }

    pub fn safepoints(&self) -> &[StrongRegistrationFingerprintEntryV1<PersistentSafepointSiteId>] {
        &self.safepoints
    }

    pub fn callables(&self) -> &[StrongRegistrationFingerprintEntryV1<PersistentCallableBodyId>] {
        &self.callables
    }
}

impl WireEncode for CanonicalStrongRegistrationFingerprintSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        encode_table(encoder, &self.static_storages)?;
        encoder.field(2)?;
        encode_table(encoder, &self.immortal_objects)?;
        encoder.field(3)?;
        encode_table(encoder, &self.initialization_units)?;
        encoder.field(4)?;
        encode_table(encoder, &self.type_registrations)?;
        encoder.field(5)?;
        encode_table(encoder, &self.safepoints)?;
        encoder.field(6)?;
        encode_table(encoder, &self.callables)
    }
}

fn entry<I>(
    semantic_id: I,
    fingerprint: StrongRegistrationFingerprintV1,
) -> StrongRegistrationFingerprintEntryV1<I> {
    StrongRegistrationFingerprintEntryV1 {
        semantic_id,
        fingerprint,
    }
}

fn strong_entry<I>(
    semantic_id: I,
    fingerprint: RegistrationFingerprintV1,
) -> Option<StrongRegistrationFingerprintEntryV1<I>> {
    match fingerprint {
        RegistrationFingerprintV1::Strong(fingerprint) => Some(entry(semantic_id, fingerprint)),
        RegistrationFingerprintV1::Odr(_) => None,
    }
}

fn canonicalize_table<I: Copy + Ord>(
    table: StrongRegistrationFingerprintTableV1,
    mut entries: Vec<StrongRegistrationFingerprintEntryV1<I>>,
) -> Result<
    Vec<StrongRegistrationFingerprintEntryV1<I>>,
    StrongRegistrationFingerprintProjectionError,
> {
    entries.sort_unstable_by_key(|entry| entry.semantic_id);
    if entries
        .windows(2)
        .any(|pair| pair[0].semantic_id == pair[1].semantic_id)
    {
        return Err(StrongRegistrationFingerprintProjectionError::DuplicateSemanticId(table));
    }
    Ok(entries)
}

fn encode_table(
    encoder: &mut Encoder,
    entries: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(entries.len() as u64)?;
    for entry in entries {
        entry.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationFingerprintTableV1 {
    StaticStorage,
    ImmortalObject,
    InitializationUnit,
    Type,
    Safepoint,
    Callable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationFingerprintProjectionError {
    DuplicateSemanticId(StrongRegistrationFingerprintTableV1),
}

impl fmt::Display for StrongRegistrationFingerprintProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong registration fingerprint projection: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationFingerprintProjectionError {}

#[cfg(test)]
mod tests;
