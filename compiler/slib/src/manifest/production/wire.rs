//! Strict untrusted wire projection for the production manifest.

use std::fmt;

use scoop_identity::{
    DecodedCborIdentityRecord, DecodedNativeLinkRequirementKey, DecodedPersistentId,
    NativeLinkRequirementId, PersistentCallableBodyId, PersistentStaticStorageId,
    SourceSignatureFingerprint,
};
use scoop_lir::{DecodedCBridgeProductionSetV1, DecodedStrongRegistrationIdentitySurfaceV1};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use super::SingleConeProductionManifestV1;
use crate::link_object::{
    DecodedCanonicalNativeExternalContractCodeSetV1,
    DecodedCanonicalStrongRegistrationFingerprintSetV1, DecodedFixedBytesV1,
    ObjectDefinitionFingerprintV1, VerifiedCodeFingerprintV1,
};
use crate::{CodeFingerprint, RuntimeImageFingerprint, SlibMemberId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedArtifactDistributionClassV1 {
    DistributableCone,
    LocalExecutableRoot,
}

impl WireEncode for DecodedArtifactDistributionClassV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encode_tag(
            encoder,
            match self {
                Self::DistributableCone => 1,
                Self::LocalExecutableRoot => 2,
            },
        )
    }
}

impl WireDecode for DecodedArtifactDistributionClassV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::DistributableCone),
            2 => Ok(Self::LocalExecutableRoot),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedExecutableRootProjectionV1 {
    main: DecodedPersistentId<PersistentCallableBodyId>,
    source_signature_fingerprint: DecodedPersistentId<SourceSignatureFingerprint>,
    gateway: DecodedPersistentId<PersistentCallableBodyId>,
    gateway_definition_fingerprint: DecodedFixedBytesV1<ObjectDefinitionFingerprintV1>,
    failure_root: DecodedPersistentId<PersistentStaticStorageId>,
    entry_owner_member: DecodedFixedBytesV1<SlibMemberId>,
}

impl WireEncode for DecodedExecutableRootProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.main.encode(encoder)?;
        encoder.field(2)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        self.gateway.encode(encoder)?;
        encoder.field(4)?;
        self.gateway_definition_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.failure_root.encode(encoder)?;
        encoder.field(6)?;
        self.entry_owner_member.encode(encoder)
    }
}

impl WireDecode for DecodedExecutableRootProjectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            main: decoder.field(1, DecodedPersistentId::decode)?,
            source_signature_fingerprint: decoder.field(2, DecodedPersistentId::decode)?,
            gateway: decoder.field(3, DecodedPersistentId::decode)?,
            gateway_definition_fingerprint: decoder.field(4, DecodedFixedBytesV1::decode)?,
            failure_root: decoder.field(5, DecodedPersistentId::decode)?,
            entry_owner_member: decoder.field(6, DecodedFixedBytesV1::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedSingleConeProductionOutputV1 {
    Library,
    Executable(DecodedExecutableRootProjectionV1),
}

impl WireEncode for DecodedSingleConeProductionOutputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Executable(root) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                root.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedSingleConeProductionOutputV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Library)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedExecutableRootProjectionV1::decode)
                    .map(Self::Executable)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

type DecodedNativeLibraryRequirementV1 =
    DecodedCborIdentityRecord<NativeLinkRequirementId, DecodedNativeLinkRequirementKey>;

/// Canonically decoded ten-field manifest without authority to promote any
/// carried identity, fingerprint, registration, contract, or C bridge value.
#[derive(Debug)]
pub struct DecodedSingleConeProductionManifestV1 {
    distribution: DecodedArtifactDistributionClassV1,
    output: DecodedSingleConeProductionOutputV1,
    image_owner_member: DecodedFixedBytesV1<SlibMemberId>,
    runtime_registration_projection: DecodedStrongRegistrationIdentitySurfaceV1,
    strong_registration_set: DecodedCanonicalStrongRegistrationFingerprintSetV1,
    runtime_image_fingerprint: DecodedFixedBytesV1<RuntimeImageFingerprint>,
    code_fingerprint: DecodedFixedBytesV1<CodeFingerprint>,
    native_contracts: DecodedCanonicalNativeExternalContractCodeSetV1,
    native_library_requirements: Vec<DecodedNativeLibraryRequirementV1>,
    c_bridge_production: DecodedCBridgeProductionSetV1,
}

impl DecodedSingleConeProductionManifestV1 {
    /// Rebuilds the complete manifest from the verified Code proof and only
    /// promotes that trusted projection after exact canonical equality.
    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<SingleConeProductionManifestV1, SingleConeProductionManifestValidationError> {
        let expected = SingleConeProductionManifestV1::from_verified_code(code.clone());
        let actual = encode(&self).map_err(SingleConeProductionManifestValidationError::Encode)?;
        let expected_bytes =
            encode(&expected).map_err(SingleConeProductionManifestValidationError::Encode)?;
        ensure_projection_equality(&actual, &expected_bytes)?;
        Ok(expected)
    }
}

impl WireEncode for DecodedSingleConeProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.distribution.encode(encoder)?;
        encoder.field(2)?;
        self.output.encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member.encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection.encode(encoder)?;
        encoder.field(5)?;
        self.strong_registration_set.encode(encoder)?;
        encoder.field(6)?;
        self.runtime_image_fingerprint.encode(encoder)?;
        encoder.field(7)?;
        self.code_fingerprint.encode(encoder)?;
        encoder.field(8)?;
        self.native_contracts.encode(encoder)?;
        encoder.field(9)?;
        encode_array(encoder, &self.native_library_requirements)?;
        encoder.field(10)?;
        self.c_bridge_production.encode(encoder)
    }
}

impl WireDecode for DecodedSingleConeProductionManifestV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            distribution: decoder.field(1, DecodedArtifactDistributionClassV1::decode)?,
            output: decoder.field(2, DecodedSingleConeProductionOutputV1::decode)?,
            image_owner_member: decoder.field(3, DecodedFixedBytesV1::decode)?,
            runtime_registration_projection: decoder
                .field(4, DecodedStrongRegistrationIdentitySurfaceV1::decode)?,
            strong_registration_set: decoder.field(
                5,
                DecodedCanonicalStrongRegistrationFingerprintSetV1::decode,
            )?,
            runtime_image_fingerprint: decoder.field(6, DecodedFixedBytesV1::decode)?,
            code_fingerprint: decoder.field(7, DecodedFixedBytesV1::decode)?,
            native_contracts: decoder
                .field(8, DecodedCanonicalNativeExternalContractCodeSetV1::decode)?,
            native_library_requirements: decoder.field(9, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCborIdentityRecord::decode(decoder))
            })?,
            c_bridge_production: decoder.field(10, DecodedCBridgeProductionSetV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub enum SingleConeProductionManifestValidationError {
    ProjectionMismatch,
    Encode(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for SingleConeProductionManifestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded single-Cone production manifest: {self:?}"
        )
    }
}

impl std::error::Error for SingleConeProductionManifestValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
}

fn ensure_projection_equality(
    actual: &[u8],
    expected: &[u8],
) -> Result<(), SingleConeProductionManifestValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(SingleConeProductionManifestValidationError::ProjectionMismatch)
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

#[cfg(test)]
mod tests;
