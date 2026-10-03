use std::num::NonZeroU64;

use scoop_wire::{
    Decoder, Encoder, HashError, WireDecode, WireEncode, WireError, WireErrorKind,
    domain_separated_cbor_hash,
};

use super::{CanonicalCAbiResolutionError, DecodedCanonicalCStorageType};
use crate::{
    CLayoutByteAlignment, CLayoutOverride, CanonicalCAbiLayout, CanonicalCAbiLayoutField,
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiLayoutFingerprintRecord, DecodedPersistentId,
    PersistentExactTypeId, PersistentFieldId, PersistentIdResolver, PersistentKeyResolver,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCLayoutOverride {
    Natural,
    Bytes(CLayoutByteAlignment),
}

impl From<DecodedCLayoutOverride> for CLayoutOverride {
    fn from(value: DecodedCLayoutOverride) -> Self {
        match value {
            DecodedCLayoutOverride::Natural => Self::Natural,
            DecodedCLayoutOverride::Bytes(bytes) => Self::Bytes(bytes),
        }
    }
}

impl WireEncode for DecodedCLayoutOverride {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Natural => super::encode_empty_sum(encoder, 1),
            Self::Bytes(bytes) => super::encode_value_sum(encoder, 2, bytes),
        }
    }
}

impl WireDecode for DecodedCLayoutOverride {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = super::decode_sum_header(decoder)?;
        match tag {
            1 => {
                super::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Natural)
            }
            2 => {
                super::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, CLayoutByteAlignment::decode)
                    .map(Self::Bytes)
            }
            tag => Err(super::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedCanonicalCAbiLayoutField {
    field: DecodedPersistentId<PersistentFieldId>,
    offset: u64,
    storage: DecodedCanonicalCStorageType,
}

impl DecodedCanonicalCAbiLayoutField {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiLayoutField, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFieldId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let field = resolver
            .resolve(self.field)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        let storage = self
            .storage
            .resolve(resolver)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        Ok(CanonicalCAbiLayoutField::new(field, self.offset, storage))
    }
}

impl WireEncode for DecodedCanonicalCAbiLayoutField {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.offset)?;
        encoder.field(3)?;
        self.storage.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalCAbiLayoutField {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            offset: decoder.field(2, Decoder::unsigned)?,
            storage: decoder.field(3, DecodedCanonicalCStorageType::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCAbiLayout {
    exact_type: DecodedPersistentId<PersistentExactTypeId>,
    byte_size: u64,
    alignment: NonZeroU64,
    aligned: DecodedCLayoutOverride,
    packed: DecodedCLayoutOverride,
    fields: Vec<DecodedCanonicalCAbiLayoutField>,
}

impl DecodedCanonicalCAbiLayout {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiLayout, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFieldId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let exact_type = resolver
            .resolve(self.exact_type)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        let fields = self
            .fields
            .into_iter()
            .map(|field| field.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CanonicalCAbiLayout::new(
            exact_type,
            self.byte_size,
            self.alignment,
            self.aligned.into(),
            self.packed.into(),
            fields,
        ))
    }
}

impl WireEncode for DecodedCanonicalCAbiLayout {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.byte_size)?;
        encoder.field(3)?;
        encoder.unsigned(self.alignment.get())?;
        encoder.field(4)?;
        self.aligned.encode(encoder)?;
        encoder.field(5)?;
        self.packed.encode(encoder)?;
        encoder.field(6)?;
        encoder.array(self.fields.len() as u64)?;
        for field in &self.fields {
            field.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalCAbiLayout {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            byte_size: decoder.field(2, Decoder::unsigned)?,
            alignment: decoder.field(3, decode_non_zero_u64)?,
            aligned: decoder.field(4, DecodedCLayoutOverride::decode)?,
            packed: decoder.field(5, DecodedCLayoutOverride::decode)?,
            fields: decoder.field(6, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCanonicalCAbiLayoutField::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCAbiLayoutFingerprintRecord {
    fingerprint: DecodedPersistentId<CanonicalCAbiLayoutFingerprint>,
    layout: DecodedCanonicalCAbiLayout,
}

impl DecodedCanonicalCAbiLayoutFingerprintRecord {
    pub const fn decoded_fingerprint(&self) -> DecodedPersistentId<CanonicalCAbiLayoutFingerprint> {
        self.fingerprint
    }

    /// Recomputes the typed fingerprint directly from the canonical decoded
    /// preimage. Persistent references remain untrusted until `resolve`.
    pub fn candidate_fingerprint(&self) -> Result<CanonicalCAbiLayoutFingerprint, HashError> {
        domain_separated_cbor_hash("scoop-c-abi-layout-v1", &self.layout)
            .map(|digest| CanonicalCAbiLayoutFingerprint(*digest.as_array()))
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFieldId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>,
    {
        let layout = self.layout.resolve(resolver)?;
        let record = CanonicalCAbiLayoutFingerprintRecord::new(layout)
            .map_err(CanonicalCAbiResolutionError::Hash)?;
        self.fingerprint
            .verify(record.fingerprint)
            .map_err(CanonicalCAbiResolutionError::LayoutFingerprint)?;
        Ok(record)
    }

    pub fn resolve_verified<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiResolutionError<E>>
    where
        R: PersistentIdResolver<CanonicalCAbiLayoutFingerprint, Error = E>
            + PersistentKeyResolver<CanonicalCAbiLayoutFingerprint, CanonicalCAbiLayout, Error = E>,
    {
        let fingerprint = resolver
            .resolve(self.fingerprint)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        let layout = resolver
            .resolve_key(self.fingerprint)
            .map_err(CanonicalCAbiResolutionError::Reference)?;
        Ok(CanonicalCAbiLayoutFingerprintRecord::from_verified(
            fingerprint,
            layout,
        ))
    }
}

impl WireEncode for DecodedCanonicalCAbiLayoutFingerprintRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(2)?;
        self.layout.encode(encoder)
    }
}

impl WireDecode for DecodedCanonicalCAbiLayoutFingerprintRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            fingerprint: decoder.field(1, DecodedPersistentId::decode)?,
            layout: decoder.field(2, DecodedCanonicalCAbiLayout::decode)?,
        })
    }
}

impl WireDecode for CLayoutByteAlignment {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Bytes1),
            2 => Ok(Self::Bytes2),
            4 => Ok(Self::Bytes4),
            8 => Ok(Self::Bytes8),
            16 => Ok(Self::Bytes16),
            tag => Err(super::unknown_tag(decoder, tag)),
        }
    }
}

fn decode_non_zero_u64(decoder: &mut Decoder<'_>) -> Result<NonZeroU64, WireError> {
    NonZeroU64::new(decoder.unsigned()?).ok_or_else(|| {
        WireError::new(
            WireErrorKind::IntegerOutOfRange,
            decoder.path().clone(),
            Some(decoder.position()),
        )
    })
}

#[cfg(test)]
mod tests;
