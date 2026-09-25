//! Strict untrusted wire projection for C bridge production.

use std::fmt;
use std::marker::PhantomData;

use scoop_identity::{DecodedCapabilityId, DecodedPersistentId, GeneratedBridgeUnitId};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
    encode_canonical_temporary_with_meter,
};

use super::CBridgeProductionSetV1;
use crate::{
    CBridgeToolchainFingerprint, CanonicalCBridgeFlagFingerprint,
    GeneratedCSourceTemplateFingerprint,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedFingerprintV1<F> {
    bytes: [u8; 32],
    marker: PhantomData<fn() -> F>,
}

impl<F> WireEncode for DecodedFingerprintV1<F> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.bytes)
    }
}

impl<F> WireDecode for DecodedFingerprintV1<F> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let bytes = decoder.bytes()?;
        let bytes = <&[u8; 32]>::try_from(bytes).copied().map_err(|_| {
            wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 32,
                    actual: bytes.len() as u64,
                },
            )
        })?;
        Ok(Self {
            bytes,
            marker: PhantomData,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedCBridgeProductionBranchV1 {
    NotUsed,
    Used {
        profile_id: DecodedCapabilityId,
        profile_fingerprint: DecodedFingerprintV1<CBridgeToolchainFingerprint>,
        source_template_fingerprint: DecodedFingerprintV1<GeneratedCSourceTemplateFingerprint>,
        canonical_flag_fingerprint: DecodedFingerprintV1<CanonicalCBridgeFlagFingerprint>,
        units: Vec<DecodedPersistentId<GeneratedBridgeUnitId>>,
    },
}

/// Canonically decoded C bridge production without authority to promote any
/// carried profile, fingerprint, or generated-unit identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCBridgeProductionSetV1 {
    branch: DecodedCBridgeProductionBranchV1,
}

impl DecodedCBridgeProductionSetV1 {
    /// Promotes only the trusted projection after exact canonical equality.
    pub fn validate(
        &self,
        expected: CBridgeProductionSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<CBridgeProductionSetV1, CBridgeProductionValidationError> {
        let path = WirePath::root().field(10);
        let actual = encode_canonical_temporary_with_meter(self, meter, &path)?;
        let expected_bytes = encode_canonical_temporary_with_meter(&expected, meter, &path)?;
        meter.charge_work(
            (actual.len() as u64)
                .saturating_add(expected_bytes.len() as u64)
                .saturating_mul(3),
            &path,
        )?;
        if actual != expected_bytes {
            return Err(CBridgeProductionValidationError::ProjectionMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedCBridgeProductionSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.branch {
            DecodedCBridgeProductionBranchV1::NotUsed => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            DecodedCBridgeProductionBranchV1::Used {
                profile_id,
                profile_fingerprint,
                source_template_fingerprint,
                canonical_flag_fingerprint,
                units,
            } => {
                encoder.map(6)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                profile_id.encode(encoder)?;
                encoder.field(2)?;
                profile_fingerprint.encode(encoder)?;
                encoder.field(3)?;
                source_template_fingerprint.encode(encoder)?;
                encoder.field(4)?;
                canonical_flag_fingerprint.encode(encoder)?;
                encoder.field(5)?;
                encode_array(encoder, units)
            }
        }
    }
}

impl WireDecode for DecodedCBridgeProductionSetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let branch = match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                DecodedCBridgeProductionBranchV1::NotUsed
            }
            2 => {
                expect_sum_length(decoder, fields, 6)?;
                DecodedCBridgeProductionBranchV1::Used {
                    profile_id: decoder.field(1, DecodedCapabilityId::decode)?,
                    profile_fingerprint: decoder.field(2, DecodedFingerprintV1::decode)?,
                    source_template_fingerprint: decoder.field(3, DecodedFingerprintV1::decode)?,
                    canonical_flag_fingerprint: decoder.field(4, DecodedFingerprintV1::decode)?,
                    units: decoder.field(5, |decoder| {
                        decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
                    })?,
                }
            }
            tag => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        Ok(Self { branch })
    }
}

#[derive(Debug)]
pub enum CBridgeProductionValidationError {
    ProjectionMismatch,
    Resource(WireError),
}

impl From<WireError> for CBridgeProductionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for CBridgeProductionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid decoded C bridge production: {self:?}")
    }
}

impl std::error::Error for CBridgeProductionValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
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

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
