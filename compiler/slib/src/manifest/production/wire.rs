//! Strict untrusted wire projection for the production manifest.

use std::fmt;

use scoop_identity::{
    DecodedCborIdentityRecord, DecodedNativeLinkRequirementKey, DecodedPersistentId,
    NativeLinkRequirementId, PersistentCallableBodyId, PersistentStaticStorageId,
    SourceSignatureFingerprint,
};
use scoop_lir::{
    CBridgeProductionSetV1, CBridgeProductionValidationError, CBridgeToolchainProfileV1,
    DecodedCBridgeProductionSetV1, DecodedRegistrationIdentitySurfaceV1, GeneratedBridgePlanSetV1,
};
use scoop_wire::{
    Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath, encode,
    encode_canonical_temporary,
};

use super::SingleConeProductionManifestV1;
use crate::CrossConeLayoutProductionManifestV1;
use crate::link_object::{
    DecodedCanonicalNativeExternalContractCodeSetV1, DecodedCanonicalOdrMemberDirectoryV1,
    DecodedFixedBytesV1, ObjectDefinitionFingerprintV1, VerifiedCodeFingerprintV1,
    VerifiedCodeFingerprintV2,
};
use crate::{CodeFingerprint, RuntimeImageFingerprint, SlibMemberId};

mod runtime;
pub use runtime::RuntimeProductionProjectionError;
mod code;
pub use code::CodeProductionProjectionError;

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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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

/// Parsed twelve-field manifest, compared with the actual production result
/// before its IDs and fingerprints are consumed.
#[derive(Debug)]
pub struct DecodedSingleConeProductionManifestV1 {
    distribution: DecodedArtifactDistributionClassV1,
    output: DecodedSingleConeProductionOutputV1,
    image_owner_member: DecodedFixedBytesV1<SlibMemberId>,
    runtime_registration_projection: DecodedRegistrationIdentitySurfaceV1,

    runtime_image_fingerprint: DecodedFixedBytesV1<RuntimeImageFingerprint>,
    code_fingerprint: DecodedFixedBytesV1<CodeFingerprint>,
    native_contracts: DecodedCanonicalNativeExternalContractCodeSetV1,
    native_library_requirements: Vec<DecodedNativeLibraryRequirementV1>,
    native_cxx: bool,
    c_bridge_production: DecodedCBridgeProductionSetV1,
    odr_members: DecodedCanonicalOdrMemberDirectoryV1,
    optimization: scoop_lir::OptimizationMode,
}

/// A decoded production manifest whose generated-C production branch was
/// rebuilt from one typed bridge plan and toolchain profile. The other eleven
/// fields remain untrusted until the complete Code proof is available.
#[derive(Debug)]
pub struct CBridgeCheckedSingleConeProductionManifestV1 {
    decoded: DecodedSingleConeProductionManifestV1,
    c_bridge_production: CBridgeProductionSetV1,
}

impl CBridgeCheckedSingleConeProductionManifestV1 {
    pub const fn optimization(&self) -> scoop_lir::OptimizationMode {
        self.decoded.optimization()
    }

    pub const fn c_bridge_production(&self) -> &CBridgeProductionSetV1 {
        &self.c_bridge_production
    }

    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<SingleConeProductionManifestV1, SingleConeProductionManifestValidationError> {
        validate_manifest(self.decoded, code)
    }

    pub fn validate_layout(
        self,
        code: &VerifiedCodeFingerprintV2,
    ) -> Result<CrossConeLayoutProductionManifestV1, SingleConeProductionManifestValidationError>
    {
        validate_layout_manifest(self.decoded, code)
    }
}

impl DecodedSingleConeProductionManifestV1 {
    pub const fn optimization(&self) -> scoop_lir::OptimizationMode {
        self.optimization
    }

    pub fn validate_c_bridge_production(
        self,
        bridge_plan: &GeneratedBridgePlanSetV1,
        profile: &CBridgeToolchainProfileV1,
    ) -> Result<CBridgeCheckedSingleConeProductionManifestV1, CBridgeProductionValidationError>
    {
        let c_bridge_production = self.replay_c_bridge_production(bridge_plan, profile)?;
        Ok(CBridgeCheckedSingleConeProductionManifestV1 {
            decoded: self,
            c_bridge_production,
        })
    }

    /// Replays only this field while retaining the original manifest for the
    /// later registration and Code fingerprint comparison.
    pub fn replay_c_bridge_production(
        &self,
        bridge_plan: &GeneratedBridgePlanSetV1,
        profile: &CBridgeToolchainProfileV1,
    ) -> Result<CBridgeProductionSetV1, CBridgeProductionValidationError> {
        let expected = CBridgeProductionSetV1::from_generated_bridge_plan(bridge_plan, profile);
        self.c_bridge_production.validate(expected)
    }

    /// Rebuilds the complete manifest from the verified Code proof and only
    /// promotes that trusted projection after exact canonical equality.
    pub fn validate(
        self,
        code: &VerifiedCodeFingerprintV1,
    ) -> Result<SingleConeProductionManifestV1, SingleConeProductionManifestValidationError> {
        validate_manifest(self, code)
    }

    pub fn validate_layout(
        self,
        code: &VerifiedCodeFingerprintV2,
    ) -> Result<CrossConeLayoutProductionManifestV1, SingleConeProductionManifestValidationError>
    {
        validate_layout_manifest(self, code)
    }
}

impl WireEncode for DecodedSingleConeProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        self.distribution.encode(encoder)?;
        encoder.field(2)?;
        self.output.encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member.encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection.encode(encoder)?;
        encoder.field(6)?;
        self.runtime_image_fingerprint.encode(encoder)?;
        encoder.field(7)?;
        self.code_fingerprint.encode(encoder)?;
        encoder.field(8)?;
        self.native_contracts.encode(encoder)?;
        encoder.field(9)?;
        encode_array(encoder, &self.native_library_requirements)?;
        encoder.field(10)?;
        self.c_bridge_production.encode(encoder)?;
        encoder.field(11)?;
        self.odr_members.encode(encoder)?;
        encoder.field(12)?;
        self.optimization.encode(encoder)?;
        encoder.field(13)?;
        encoder.unsigned(u64::from(self.native_cxx))
    }
}

impl WireDecode for DecodedSingleConeProductionManifestV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            distribution: decoder.field(1, DecodedArtifactDistributionClassV1::decode)?,
            output: decoder.field(2, DecodedSingleConeProductionOutputV1::decode)?,
            image_owner_member: decoder.field(3, DecodedFixedBytesV1::decode)?,
            runtime_registration_projection: decoder
                .field(4, DecodedRegistrationIdentitySurfaceV1::decode)?,
            runtime_image_fingerprint: decoder.field(6, DecodedFixedBytesV1::decode)?,
            code_fingerprint: decoder.field(7, DecodedFixedBytesV1::decode)?,
            native_contracts: decoder
                .field(8, DecodedCanonicalNativeExternalContractCodeSetV1::decode)?,
            native_library_requirements: decoder.field(9, |decoder| {
                decoder.decode_array(|decoder, _| DecodedCborIdentityRecord::decode(decoder))
            })?,
            c_bridge_production: decoder.field(10, DecodedCBridgeProductionSetV1::decode)?,
            odr_members: decoder.field(11, DecodedCanonicalOdrMemberDirectoryV1::decode)?,
            optimization: decoder.field(12, scoop_lir::OptimizationMode::decode)?,
            native_cxx: decoder.field(13, |decoder| match decoder.unsigned()? {
                0 => Ok(false),
                1 => Ok(true),
                tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
            })?,
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

fn validate_manifest(
    decoded: DecodedSingleConeProductionManifestV1,
    code: &VerifiedCodeFingerprintV1,
) -> Result<SingleConeProductionManifestV1, SingleConeProductionManifestValidationError> {
    let expected = SingleConeProductionManifestV1::from_verified_code(code.clone());
    let actual = encode(&decoded).map_err(SingleConeProductionManifestValidationError::Encode)?;
    let expected_bytes =
        encode(&expected).map_err(SingleConeProductionManifestValidationError::Encode)?;
    ensure_projection_equality(&actual, &expected_bytes)?;
    Ok(expected)
}

fn validate_layout_manifest(
    decoded: DecodedSingleConeProductionManifestV1,
    code: &VerifiedCodeFingerprintV2,
) -> Result<CrossConeLayoutProductionManifestV1, SingleConeProductionManifestValidationError> {
    let expected = CrossConeLayoutProductionManifestV1::from_verified_code(code.clone());
    let actual = encode(&decoded).map_err(SingleConeProductionManifestValidationError::Encode)?;
    let expected_bytes =
        encode(&expected).map_err(SingleConeProductionManifestValidationError::Encode)?;
    ensure_projection_equality(&actual, &expected_bytes)?;
    Ok(expected)
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

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
pub(crate) mod tests;
