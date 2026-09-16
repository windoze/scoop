use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityIdError, DecodedCapabilityId};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_protocol::StructuredDiagnosticV1;
use scoop_wire::{
    BudgetMeter, DecodeLimits, DecodeUsage, Decoder, Digest256, Encoder, HashError, WireDecode,
    WireEncode, WireError, WirePath, decode_canonical_with_meter, domain_separated_cbor_hash,
    domain_separated_cbor_hash_stream_length,
};

use super::{
    CacheArtifactFingerprintClaimV1, CacheReceiptValidationError, CacheTargetSelectionV1,
    ConeCompileCacheKeyV1, canonical_warnings, validate_profile, validate_warning_order,
};
use crate::PairedCompilerFingerprintV1;

const CORE_SOURCE_KEY_DOMAIN: &str = "scoop-core-source-snapshot-v1";
const CORE_RECEIPT_FINGERPRINT_DOMAIN: &str = "scoop-trusted-core-slot-receipt-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CoreSourceSnapshotKeyV1([u8; 32]);

impl CoreSourceSnapshotKeyV1 {
    pub fn derive(compile_key: ConeCompileCacheKeyV1) -> Result<Self, HashError> {
        domain_separated_cbor_hash(CORE_SOURCE_KEY_DOMAIN, &compile_key)
            .map(|digest| Self(*digest.as_array()))
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for CoreSourceSnapshotKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for CoreSourceSnapshotKeyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustedCoreSlotReceiptFingerprintV1([u8; 32]);

impl TrustedCoreSlotReceiptFingerprintV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for TrustedCoreSlotReceiptFingerprintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for TrustedCoreSlotReceiptFingerprintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreSlotReceiptBodyV1 {
    source_snapshot_key: CoreSourceSnapshotKeyV1,
    artifact_fingerprint: CacheArtifactFingerprintClaimV1,
    target_selection: CacheTargetSelectionV1,
    compiler: PairedCompilerFingerprintV1,
    artifact_profile: ArtifactCapabilityProfileId,
    structured_warnings: Vec<StructuredDiagnosticV1>,
}

impl TrustedCoreSlotReceiptBodyV1 {
    pub fn new(
        source_snapshot_key: CoreSourceSnapshotKeyV1,
        artifact_fingerprint: scoop_slib::ArtifactFingerprint,
        target_selection: ValidatedLirTargetSelection,
        compiler: PairedCompilerFingerprintV1,
        artifact_profile: ArtifactCapabilityProfileId,
        structured_warnings: Vec<StructuredDiagnosticV1>,
    ) -> Result<Self, CacheReceiptValidationError> {
        validate_profile(&artifact_profile)?;
        let structured_warnings = canonical_warnings(structured_warnings)?;
        Ok(Self {
            source_snapshot_key,
            artifact_fingerprint: artifact_fingerprint.into(),
            target_selection: CacheTargetSelectionV1::new(target_selection),
            compiler,
            artifact_profile,
            structured_warnings,
        })
    }

    pub const fn source_snapshot_key(&self) -> CoreSourceSnapshotKeyV1 {
        self.source_snapshot_key
    }

    pub const fn artifact_fingerprint(&self) -> CacheArtifactFingerprintClaimV1 {
        self.artifact_fingerprint
    }

    pub const fn target_selection(&self) -> CacheTargetSelectionV1 {
        self.target_selection
    }

    pub const fn compiler(&self) -> PairedCompilerFingerprintV1 {
        self.compiler
    }

    pub const fn artifact_profile(&self) -> &ArtifactCapabilityProfileId {
        &self.artifact_profile
    }

    pub fn structured_warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.structured_warnings
    }

    fn fingerprint(&self) -> Result<TrustedCoreSlotReceiptFingerprintV1, HashError> {
        domain_separated_cbor_hash(CORE_RECEIPT_FINGERPRINT_DOMAIN, self)
            .map(|digest| TrustedCoreSlotReceiptFingerprintV1(*digest.as_array()))
    }
}

impl WireEncode for TrustedCoreSlotReceiptBodyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        encoder.unsigned(1)?;
        encoder.field(2)?;
        self.source_snapshot_key.encode(encoder)?;
        encoder.field(3)?;
        self.artifact_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.target_selection.encode(encoder)?;
        encoder.field(5)?;
        self.compiler.encode(encoder)?;
        encoder.field(6)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(7)?;
        encode_array(&self.structured_warnings, encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreSlotReceiptV1 {
    body: TrustedCoreSlotReceiptBodyV1,
    fingerprint: TrustedCoreSlotReceiptFingerprintV1,
}

impl TrustedCoreSlotReceiptV1 {
    pub fn new(body: TrustedCoreSlotReceiptBodyV1) -> Result<Self, HashError> {
        let fingerprint = body.fingerprint()?;
        Ok(Self { body, fingerprint })
    }

    pub const fn body(&self) -> &TrustedCoreSlotReceiptBodyV1 {
        &self.body
    }

    pub const fn fingerprint(&self) -> TrustedCoreSlotReceiptFingerprintV1 {
        self.fingerprint
    }
}

impl WireEncode for TrustedCoreSlotReceiptV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

pub fn decode_trusted_core_slot_receipt_v1(
    bytes: &[u8],
    limits: DecodeLimits,
) -> Result<(TrustedCoreSlotReceiptV1, DecodeUsage), TrustedCoreSlotReceiptDecodeError> {
    let mut meter = BudgetMeter::new(limits);
    let decoded: DecodedTrustedCoreSlotReceiptV1 =
        decode_canonical_with_meter(bytes, &mut meter)
            .map_err(TrustedCoreSlotReceiptDecodeError::Wire)?;
    let receipt = decoded.validate(&mut meter)?;
    Ok((receipt, meter.usage()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedTrustedCoreSlotReceiptV1 {
    body: DecodedTrustedCoreSlotReceiptBodyV1,
    fingerprint: Digest256,
}

impl DecodedTrustedCoreSlotReceiptV1 {
    fn validate(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<TrustedCoreSlotReceiptV1, TrustedCoreSlotReceiptDecodeError> {
        let body = self.body.validate(meter)?;
        let stream_length =
            domain_separated_cbor_hash_stream_length(CORE_RECEIPT_FINGERPRINT_DOMAIN, &body)
                .map_err(TrustedCoreSlotReceiptDecodeError::Hash)?;
        meter
            .charge_sha256(stream_length, &WirePath::root().field(1))
            .map_err(TrustedCoreSlotReceiptDecodeError::Wire)?;
        let receipt =
            TrustedCoreSlotReceiptV1::new(body).map_err(TrustedCoreSlotReceiptDecodeError::Hash)?;
        if receipt.fingerprint.as_array() != self.fingerprint.as_array() {
            return Err(TrustedCoreSlotReceiptDecodeError::FingerprintMismatch {
                expected: receipt.fingerprint,
                actual: TrustedCoreSlotReceiptFingerprintV1(*self.fingerprint.as_array()),
            });
        }
        Ok(receipt)
    }
}

impl WireEncode for DecodedTrustedCoreSlotReceiptV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.body.encode(encoder)?;
        encoder.field(2)?;
        self.fingerprint.encode(encoder)
    }
}

impl WireDecode for DecodedTrustedCoreSlotReceiptV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            body: decoder.field(1, DecodedTrustedCoreSlotReceiptBodyV1::decode)?,
            fingerprint: decoder.field(2, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedTrustedCoreSlotReceiptBodyV1 {
    schema: u32,
    source_snapshot_key: Digest256,
    artifact_fingerprint: Digest256,
    target: DecodedCoreTargetSelectionV1,
    compiler: DecodedCoreCompilerFingerprintV1,
    artifact_profile: DecodedCapabilityId,
    structured_warnings: Vec<StructuredDiagnosticV1>,
}

impl DecodedTrustedCoreSlotReceiptBodyV1 {
    fn validate(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<TrustedCoreSlotReceiptBodyV1, TrustedCoreSlotReceiptDecodeError> {
        if self.schema != 1 {
            return Err(TrustedCoreSlotReceiptDecodeError::UnsupportedSchema(
                self.schema,
            ));
        }
        validate_warning_order(&self.structured_warnings, Some(meter))
            .map_err(TrustedCoreSlotReceiptDecodeError::Validation)?;
        let capability = self
            .artifact_profile
            .validate()
            .map_err(TrustedCoreSlotReceiptDecodeError::Capability)?;
        let artifact_profile = ArtifactCapabilityProfileId::refine(capability)
            .map_err(|_| TrustedCoreSlotReceiptDecodeError::UnknownArtifactProfile)?;
        validate_profile(&artifact_profile)
            .map_err(TrustedCoreSlotReceiptDecodeError::Validation)?;
        Ok(TrustedCoreSlotReceiptBodyV1 {
            source_snapshot_key: CoreSourceSnapshotKeyV1(*self.source_snapshot_key.as_array()),
            artifact_fingerprint: CacheArtifactFingerprintClaimV1::from_array(
                *self.artifact_fingerprint.as_array(),
            ),
            target_selection: self.target.validate()?,
            compiler: self.compiler.validate(),
            artifact_profile,
            structured_warnings: self.structured_warnings,
        })
    }
}

impl WireEncode for DecodedTrustedCoreSlotReceiptBodyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.schema))?;
        encoder.field(2)?;
        self.source_snapshot_key.encode(encoder)?;
        encoder.field(3)?;
        self.artifact_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.target.encode(encoder)?;
        encoder.field(5)?;
        self.compiler.encode(encoder)?;
        encoder.field(6)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(7)?;
        encode_array(&self.structured_warnings, encoder)
    }
}

impl WireDecode for DecodedTrustedCoreSlotReceiptBodyV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            schema: decoder.field(1, Decoder::u32)?,
            source_snapshot_key: decoder.field(2, Digest256::decode)?,
            artifact_fingerprint: decoder.field(3, Digest256::decode)?,
            target: decoder.field(4, DecodedCoreTargetSelectionV1::decode)?,
            compiler: decoder.field(5, DecodedCoreCompilerFingerprintV1::decode)?,
            artifact_profile: decoder.field(6, DecodedCapabilityId::decode)?,
            structured_warnings: decoder.field(7, |decoder| {
                decoder.decode_array(|decoder, _| StructuredDiagnosticV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedCoreTargetSelectionV1 {
    target: u32,
    backend: u32,
}

impl DecodedCoreTargetSelectionV1 {
    fn validate(self) -> Result<CacheTargetSelectionV1, TrustedCoreSlotReceiptDecodeError> {
        if self.target != 1 || self.backend != 1 {
            return Err(
                TrustedCoreSlotReceiptDecodeError::UnsupportedTargetSelection {
                    target: self.target,
                    backend: self.backend,
                },
            );
        }
        Ok(CacheTargetSelectionV1::new(
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        ))
    }
}

impl WireEncode for DecodedCoreTargetSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.target))?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.backend))
    }
}

impl WireDecode for DecodedCoreTargetSelectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target: decoder.field(1, Decoder::u32)?,
            backend: decoder.field(2, Decoder::u32)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedCoreCompilerFingerprintV1 {
    executable: Digest256,
    distribution: Digest256,
    build: Digest256,
}

impl DecodedCoreCompilerFingerprintV1 {
    const fn validate(self) -> PairedCompilerFingerprintV1 {
        PairedCompilerFingerprintV1::from_parts(self.executable, self.distribution, self.build)
    }
}

impl WireEncode for DecodedCoreCompilerFingerprintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.executable.encode(encoder)?;
        encoder.field(2)?;
        self.distribution.encode(encoder)?;
        encoder.field(3)?;
        self.build.encode(encoder)
    }
}

impl WireDecode for DecodedCoreCompilerFingerprintV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            executable: decoder.field(1, Digest256::decode)?,
            distribution: decoder.field(2, Digest256::decode)?,
            build: decoder.field(3, Digest256::decode)?,
        })
    }
}

fn encode_array<T: WireEncode>(
    values: &[T],
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum TrustedCoreSlotReceiptDecodeError {
    Wire(WireError),
    Hash(HashError),
    UnsupportedSchema(u32),
    UnsupportedTargetSelection {
        target: u32,
        backend: u32,
    },
    Capability(CapabilityIdError),
    UnknownArtifactProfile,
    Validation(CacheReceiptValidationError),
    FingerprintMismatch {
        expected: TrustedCoreSlotReceiptFingerprintV1,
        actual: TrustedCoreSlotReceiptFingerprintV1,
    },
}

impl fmt::Display for TrustedCoreSlotReceiptDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(source) => source.fmt(formatter),
            Self::Hash(source) => source.fmt(formatter),
            Self::UnsupportedSchema(schema) => {
                write!(
                    formatter,
                    "unsupported trusted-core receipt schema {schema}"
                )
            }
            Self::UnsupportedTargetSelection { target, backend } => write!(
                formatter,
                "unsupported trusted-core receipt target/backend tags {target}/{backend}"
            ),
            Self::Capability(source) => source.fmt(formatter),
            Self::UnknownArtifactProfile => {
                formatter.write_str("trusted-core receipt names an unknown artifact profile")
            }
            Self::Validation(source) => source.fmt(formatter),
            Self::FingerprintMismatch { expected, actual } => write!(
                formatter,
                "trusted-core receipt fingerprint mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl std::error::Error for TrustedCoreSlotReceiptDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wire(source) => Some(source),
            Self::Hash(source) => Some(source),
            Self::Capability(source) => Some(source),
            Self::Validation(source) => Some(source),
            Self::UnsupportedSchema(_)
            | Self::UnsupportedTargetSelection { .. }
            | Self::UnknownArtifactProfile
            | Self::FingerprintMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
