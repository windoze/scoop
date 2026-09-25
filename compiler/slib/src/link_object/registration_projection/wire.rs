//! Strict untrusted wire projection for strong registration fingerprints.

use std::fmt;

use scoop_identity::{
    DecodedPersistentId, PersistentCallableBodyId, PersistentExactTypeId, PersistentId,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentSafepointSiteId,
    PersistentStaticStorageId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use super::{CanonicalStrongRegistrationFingerprintSetV1, StrongRegistrationFingerprintV1};
use crate::link_object::DecodedFixedBytesV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedStrongRegistrationFingerprintEntryV1<I: PersistentId> {
    semantic_id: DecodedPersistentId<I>,
    fingerprint: DecodedFixedBytesV1<StrongRegistrationFingerprintV1>,
}

impl<I: PersistentId> WireEncode for DecodedStrongRegistrationFingerprintEntryV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

impl<I: PersistentId> WireDecode for DecodedStrongRegistrationFingerprintEntryV1<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            semantic_id: decoder.field(1, DecodedPersistentId::decode)?,
            fingerprint: decoder.field(2, DecodedFixedBytesV1::decode)?,
        })
    }
}

/// Canonically decoded six-table projection without authority to promote any
/// semantic identity or registration fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalStrongRegistrationFingerprintSetV1 {
    static_storages: Vec<DecodedStrongRegistrationFingerprintEntryV1<PersistentStaticStorageId>>,
    immortal_objects: Vec<DecodedStrongRegistrationFingerprintEntryV1<PersistentImmortalObjectId>>,
    initialization_units:
        Vec<DecodedStrongRegistrationFingerprintEntryV1<PersistentInitializationUnitId>>,
    type_registrations: Vec<DecodedStrongRegistrationFingerprintEntryV1<PersistentExactTypeId>>,
    safepoints: Vec<DecodedStrongRegistrationFingerprintEntryV1<PersistentSafepointSiteId>>,
    callables: Vec<DecodedStrongRegistrationFingerprintEntryV1<PersistentCallableBodyId>>,
}

impl DecodedCanonicalStrongRegistrationFingerprintSetV1 {
    /// Promotes only the projection rebuilt from the verified registration
    /// patch set after exact canonical equality.
    pub fn validate(
        self,
        expected: &CanonicalStrongRegistrationFingerprintSetV1,
    ) -> Result<
        CanonicalStrongRegistrationFingerprintSetV1,
        StrongRegistrationFingerprintSetValidationError,
    > {
        let actual =
            encode(&self).map_err(StrongRegistrationFingerprintSetValidationError::Encode)?;
        let expected_bytes =
            encode(expected).map_err(StrongRegistrationFingerprintSetValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(StrongRegistrationFingerprintSetValidationError::ProjectionMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedCanonicalStrongRegistrationFingerprintSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        encode_array(encoder, &self.static_storages)?;
        encoder.field(2)?;
        encode_array(encoder, &self.immortal_objects)?;
        encoder.field(3)?;
        encode_array(encoder, &self.initialization_units)?;
        encoder.field(4)?;
        encode_array(encoder, &self.type_registrations)?;
        encoder.field(5)?;
        encode_array(encoder, &self.safepoints)?;
        encoder.field(6)?;
        encode_array(encoder, &self.callables)
    }
}

impl WireDecode for DecodedCanonicalStrongRegistrationFingerprintSetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            static_storages: decode_array_field(decoder, 1)?,
            immortal_objects: decode_array_field(decoder, 2)?,
            initialization_units: decode_array_field(decoder, 3)?,
            type_registrations: decode_array_field(decoder, 4)?,
            safepoints: decode_array_field(decoder, 5)?,
            callables: decode_array_field(decoder, 6)?,
        })
    }
}

#[derive(Debug)]
pub enum StrongRegistrationFingerprintSetValidationError {
    ProjectionMismatch,
    Encode(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for StrongRegistrationFingerprintSetValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong registration fingerprint set: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationFingerprintSetValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
}

fn decode_array_field<I: PersistentId>(
    decoder: &mut Decoder<'_>,
    field: u32,
) -> Result<Vec<DecodedStrongRegistrationFingerprintEntryV1<I>>, WireError> {
    decoder.field(field, |decoder| {
        decoder
            .decode_array(|decoder, _| DecodedStrongRegistrationFingerprintEntryV1::decode(decoder))
    })
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}
