use std::fmt;

use scoop_identity::{
    ArtifactCapabilityProfileId, ArtifactCapabilityProfileRefinementError, BackendProfileWireId,
    CapabilityIdError, CapabilityRefinementError, DecodedCapabilityId, ManglingSchemaIdentity,
    TargetProfileWireId,
};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{Decoder, Digest256, Encoder, HashError, WireDecode, WireEncode, WireError};

use crate::{ArtifactCapabilityProfile, CompatibilityRecord};

const INITIAL_SCHEMA: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedCompatibilityRecord {
    language_abi: Digest256,
    runtime_abi: Digest256,
    identity_schema: u32,
    mangling_schema: String,
    target_profile: DecodedCapabilityId,
    target_fingerprint: Digest256,
    backend_profile: DecodedCapabilityId,
    backend_fingerprint: Digest256,
    hir_schema: u32,
    mir_schema: u32,
    lir_schema: u32,
    composite_identity_abi: Digest256,
    artifact_profile: DecodedCapabilityId,
    artifact_profile_fingerprint: Digest256,
}

impl DecodedCompatibilityRecord {
    pub(super) fn validate(
        self,
        selection: ValidatedLirTargetSelection,
    ) -> Result<CompatibilityRecord, CompatibilityValidationError> {
        require_schema(CompatibilitySchemaKind::Identity, self.identity_schema)?;
        require_schema(CompatibilitySchemaKind::Hir, self.hir_schema)?;
        require_schema(CompatibilitySchemaKind::Mir, self.mir_schema)?;
        require_schema(CompatibilitySchemaKind::Lir, self.lir_schema)?;

        let artifact_profile = ArtifactCapabilityProfileId::refine(
            self.artifact_profile
                .validate()
                .map_err(CompatibilityValidationError::Capability)?,
        )
        .map_err(CompatibilityValidationError::ArtifactProfile)?;
        let profile = ArtifactCapabilityProfile::from_id(&artifact_profile)
            .expect("every refined artifact profile is registered");

        let expected = CompatibilityRecord::new(selection, profile)
            .map_err(CompatibilityValidationError::Hash)?;
        if self.mangling_schema != ManglingSchemaIdentity.canonical_name() {
            return Err(CompatibilityValidationError::ManglingSchema {
                actual: self.mangling_schema,
            });
        }
        TargetProfileWireId::refine(
            self.target_profile
                .validate()
                .map_err(CompatibilityValidationError::Capability)?,
        )
        .map_err(CompatibilityValidationError::TargetProfile)?;
        BackendProfileWireId::refine(
            self.backend_profile
                .validate()
                .map_err(CompatibilityValidationError::Capability)?,
        )
        .map_err(CompatibilityValidationError::BackendProfile)?;
        require_fingerprint(
            CompatibilityFingerprintKind::LanguageAbi,
            expected.language_abi().as_array(),
            self.language_abi,
        )?;
        require_fingerprint(
            CompatibilityFingerprintKind::RuntimeAbi,
            expected.runtime_abi().as_array(),
            self.runtime_abi,
        )?;
        require_fingerprint(
            CompatibilityFingerprintKind::TargetProfile,
            expected.target_fingerprint().as_array(),
            self.target_fingerprint,
        )?;
        require_fingerprint(
            CompatibilityFingerprintKind::BackendProfile,
            expected.backend_fingerprint().as_array(),
            self.backend_fingerprint,
        )?;
        require_fingerprint(
            CompatibilityFingerprintKind::CompositeIdentityAbi,
            expected.composite_identity_abi().as_array(),
            self.composite_identity_abi,
        )?;
        require_fingerprint(
            CompatibilityFingerprintKind::ArtifactProfile,
            expected.artifact_profile_fingerprint().as_array(),
            self.artifact_profile_fingerprint,
        )?;
        Ok(expected)
    }

    #[cfg(test)]
    pub(super) fn set_identity_schema(&mut self, schema: u32) {
        self.identity_schema = schema;
    }

    #[cfg(test)]
    pub(super) fn corrupt_language_fingerprint(&mut self) {
        let mut bytes = *self.language_abi.as_array();
        bytes[0] ^= 1;
        self.language_abi = Digest256::from_array(bytes);
    }
}

impl WireEncode for DecodedCompatibilityRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(14)?;
        encoder.field(1)?;
        self.language_abi.encode(encoder)?;
        encoder.field(2)?;
        self.runtime_abi.encode(encoder)?;
        encode_u32_field(encoder, 3, self.identity_schema)?;
        encoder.field(4)?;
        encoder.text(&self.mangling_schema)?;
        encoder.field(5)?;
        self.target_profile.encode(encoder)?;
        encoder.field(6)?;
        self.target_fingerprint.encode(encoder)?;
        encoder.field(7)?;
        self.backend_profile.encode(encoder)?;
        encoder.field(8)?;
        self.backend_fingerprint.encode(encoder)?;
        encode_u32_field(encoder, 9, self.hir_schema)?;
        encode_u32_field(encoder, 10, self.mir_schema)?;
        encode_u32_field(encoder, 11, self.lir_schema)?;
        encoder.field(12)?;
        self.composite_identity_abi.encode(encoder)?;
        encoder.field(13)?;
        self.artifact_profile.encode(encoder)?;
        encoder.field(14)?;
        self.artifact_profile_fingerprint.encode(encoder)
    }
}

impl WireDecode for DecodedCompatibilityRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(14)?;
        Ok(Self {
            language_abi: decoder.field(1, Digest256::decode)?,
            runtime_abi: decoder.field(2, Digest256::decode)?,
            identity_schema: decoder.field(3, Decoder::u32)?,
            mangling_schema: decoder.field(4, Decoder::owned_text)?,
            target_profile: decoder.field(5, DecodedCapabilityId::decode)?,
            target_fingerprint: decoder.field(6, Digest256::decode)?,
            backend_profile: decoder.field(7, DecodedCapabilityId::decode)?,
            backend_fingerprint: decoder.field(8, Digest256::decode)?,
            hir_schema: decoder.field(9, Decoder::u32)?,
            mir_schema: decoder.field(10, Decoder::u32)?,
            lir_schema: decoder.field(11, Decoder::u32)?,
            composite_identity_abi: decoder.field(12, Digest256::decode)?,
            artifact_profile: decoder.field(13, DecodedCapabilityId::decode)?,
            artifact_profile_fingerprint: decoder.field(14, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilitySchemaKind {
    Identity,
    Hir,
    Mir,
    Lir,
}

impl CompatibilitySchemaKind {
    const fn current_schema(self) -> u32 {
        match self {
            Self::Identity => INITIAL_SCHEMA,
            Self::Hir | Self::Mir | Self::Lir => crate::metadata::METADATA_SCHEMA,
        }
    }
}

impl fmt::Display for CompatibilitySchemaKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Identity => "identity",
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityFingerprintKind {
    LanguageAbi,
    RuntimeAbi,
    TargetProfile,
    BackendProfile,
    CompositeIdentityAbi,
    ArtifactProfile,
}

impl fmt::Display for CompatibilityFingerprintKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LanguageAbi => "language ABI",
            Self::RuntimeAbi => "runtime ABI",
            Self::TargetProfile => "target profile",
            Self::BackendProfile => "backend profile",
            Self::CompositeIdentityAbi => "composite identity ABI",
            Self::ArtifactProfile => "artifact profile",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityValidationError {
    UnsupportedSchema {
        kind: CompatibilitySchemaKind,
        actual: u32,
    },
    ManglingSchema {
        actual: String,
    },
    Capability(CapabilityIdError),
    TargetProfile(CapabilityRefinementError),
    BackendProfile(CapabilityRefinementError),
    ArtifactProfile(ArtifactCapabilityProfileRefinementError),
    FingerprintMismatch {
        kind: CompatibilityFingerprintKind,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    Hash(HashError),
    Resource(WireError),
}

impl fmt::Display for CompatibilityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema { kind, actual } => {
                write!(
                    formatter,
                    "{kind} schema must be {}, found {actual}",
                    kind.current_schema()
                )
            }
            Self::ManglingSchema { actual } => {
                write!(
                    formatter,
                    "mangling schema must be persistent-v1, found {actual:?}"
                )
            }
            Self::Capability(error) => error.fmt(formatter),
            Self::TargetProfile(error) => error.fmt(formatter),
            Self::BackendProfile(error) => error.fmt(formatter),
            Self::ArtifactProfile(error) => error.fmt(formatter),
            Self::FingerprintMismatch {
                kind,
                expected,
                actual,
            } => {
                write!(formatter, "{kind} fingerprint mismatch: expected ")?;
                write_hex(expected, formatter)?;
                formatter.write_str(", found ")?;
                write_hex(actual, formatter)
            }
            Self::Hash(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompatibilityValidationError {}

fn require_schema(
    kind: CompatibilitySchemaKind,
    actual: u32,
) -> Result<(), CompatibilityValidationError> {
    if actual == kind.current_schema() {
        Ok(())
    } else {
        Err(CompatibilityValidationError::UnsupportedSchema { kind, actual })
    }
}

fn require_fingerprint(
    kind: CompatibilityFingerprintKind,
    expected: &[u8; 32],
    actual: Digest256,
) -> Result<(), CompatibilityValidationError> {
    if actual.as_array() == expected {
        Ok(())
    } else {
        Err(CompatibilityValidationError::FingerprintMismatch {
            kind,
            expected: *expected,
            actual: *actual.as_array(),
        })
    }
}

fn encode_u32_field(
    encoder: &mut Encoder,
    field: u32,
    value: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(u64::from(value))
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::{decode_canonical, encode};

    use super::*;

    #[test]
    fn compatibility_reader_selects_the_exact_registered_artifact_profile() {
        let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        for profile in [
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ] {
            let expected = CompatibilityRecord::new(selection, profile).unwrap();
            let decoded =
                decode_canonical::<DecodedCompatibilityRecord>(&encode(&expected).unwrap())
                    .unwrap();
            let actual = decoded.validate(selection).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.artifact_profile(), &profile.id());
        }
    }

    #[test]
    fn compatibility_reader_rejects_each_old_outer_schema_and_mixed_generations() {
        let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let current =
            CompatibilityRecord::new(selection, ArtifactCapabilityProfile::CROSS_CONE_GENERIC)
                .unwrap();
        for (kind, schemas) in [
            (CompatibilitySchemaKind::Hir, (1, 2, 2)),
            (CompatibilitySchemaKind::Mir, (2, 1, 2)),
            (CompatibilitySchemaKind::Lir, (2, 2, 1)),
            (CompatibilitySchemaKind::Hir, (1, 1, 1)),
        ] {
            let mut decoded =
                decode_canonical::<DecodedCompatibilityRecord>(&encode(&current).unwrap()).unwrap();
            (decoded.hir_schema, decoded.mir_schema, decoded.lir_schema) = schemas;
            assert!(
                matches!(decoded.validate(selection), Err(CompatibilityValidationError::UnsupportedSchema { kind: actual, actual: 1 }) if actual == kind)
            );
        }
    }
}
