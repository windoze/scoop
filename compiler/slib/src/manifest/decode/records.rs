use std::fmt;

use scoop_identity::{
    ConeCoordinateError, ConeIdentity, DecodedCapabilityId, DecodedConeCoordinate,
    DecodedPersistentId, PersistentIdMismatch,
};
use scoop_wire::{
    BudgetMeter, Decoder, Digest256, Encoder, HashError, WireDecode, WireEncode, WireError,
    WireErrorKind, WirePath,
};

use super::super::{
    CodeFingerprint, ConeKind, ConeRecord, ConeRecordError, ConeSourceForm, DependencyRecord,
    FingerprintAvailability, HirFingerprint, LirFingerprint, ManifestSection, ManifestSectionError,
    MirFingerprint, RuntimeImageFingerprint, SemanticFingerprintRecord,
};
use crate::{ArtifactCapabilityProfile, FingerprintAvailabilityRequirement, MemberPurposeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
/// Untrusted wire projection of a Cone record. Call [`Self::validate`] before
/// treating its identity as semantic authority.
pub struct DecodedConeRecord {
    coordinate: DecodedConeCoordinate,
    identity: DecodedPersistentId<ConeIdentity>,
    kind: u32,
    source_form: u32,
}

impl DecodedConeRecord {
    pub fn validate(
        self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeRecord, ConeRecordValidationError> {
        let coordinate = self
            .coordinate
            .validate()
            .map_err(ConeRecordValidationError::Coordinate)?;
        let stream_length = coordinate
            .identity_hash_stream_length()
            .map_err(ConeRecordValidationError::Hash)?;
        meter
            .charge_sha256(stream_length, path)
            .map_err(ConeRecordValidationError::Resource)?;
        let expected = coordinate
            .identity()
            .map_err(ConeRecordValidationError::Hash)?;
        self.identity
            .verify(expected)
            .map_err(ConeRecordValidationError::Identity)?;
        let kind = match self.kind {
            1 => ConeKind::Library,
            2 => ConeKind::Executable,
            actual => return Err(ConeRecordValidationError::UnknownKind { actual }),
        };
        let source_form = match self.source_form {
            1 => ConeSourceForm::Manifest,
            2 => ConeSourceForm::SingleFile,
            actual => return Err(ConeRecordValidationError::UnknownSourceForm { actual }),
        };
        ConeRecord::from_validated(coordinate, expected, kind, source_form)
            .map_err(ConeRecordValidationError::Record)
    }
}

impl WireEncode for DecodedConeRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.coordinate.encode(encoder)?;
        encoder.field(2)?;
        self.identity.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.kind))?;
        encoder.field(4)?;
        encoder.unsigned(u64::from(self.source_form))
    }
}

impl WireDecode for DecodedConeRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            coordinate: decoder.field(1, DecodedConeCoordinate::decode)?,
            identity: decoder.field(2, DecodedPersistentId::decode)?,
            kind: decoder.field(3, Decoder::u32)?,
            source_form: decoder.field(4, Decoder::u32)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConeRecordValidationError {
    Coordinate(ConeCoordinateError),
    Hash(HashError),
    Identity(PersistentIdMismatch<ConeIdentity>),
    UnknownKind { actual: u32 },
    UnknownSourceForm { actual: u32 },
    Record(ConeRecordError),
    Resource(WireError),
}

impl fmt::Display for ConeRecordValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
            Self::UnknownKind { actual } => write!(formatter, "unknown Cone kind {actual}"),
            Self::UnknownSourceForm { actual } => {
                write!(formatter, "unknown Cone source form {actual}")
            }
            Self::Record(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConeRecordValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Untrusted wire projection of a dependency record. Call [`Self::validate`]
/// before comparing it with an artifact dependency proof.
pub struct DecodedDependencyRecord {
    coordinate: DecodedConeCoordinate,
    identity: DecodedPersistentId<ConeIdentity>,
    hir_fingerprint: Digest256,
    mir_fingerprint: Digest256,
    lir_fingerprint: Digest256,
}

impl DecodedDependencyRecord {
    pub fn validate(
        self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DependencyRecord, DependencyRecordValidationError> {
        let coordinate = self
            .coordinate
            .validate()
            .map_err(DependencyRecordValidationError::Coordinate)?;
        let stream_length = coordinate
            .identity_hash_stream_length()
            .map_err(DependencyRecordValidationError::Hash)?;
        meter
            .charge_sha256(stream_length, path)
            .map_err(DependencyRecordValidationError::Resource)?;
        let expected = coordinate
            .identity()
            .map_err(DependencyRecordValidationError::Hash)?;
        self.identity
            .verify(expected)
            .map_err(DependencyRecordValidationError::Identity)?;
        Ok(DependencyRecord::from_validated(
            coordinate,
            expected,
            HirFingerprint::from_array(*self.hir_fingerprint.as_array()),
            MirFingerprint::from_array(*self.mir_fingerprint.as_array()),
            LirFingerprint::from_array(*self.lir_fingerprint.as_array()),
        ))
    }
}

impl WireEncode for DecodedDependencyRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.coordinate.encode(encoder)?;
        encoder.field(2)?;
        self.identity.encode(encoder)?;
        encoder.field(3)?;
        self.hir_fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.mir_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.lir_fingerprint.encode(encoder)
    }
}

impl WireDecode for DecodedDependencyRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            coordinate: decoder.field(1, DecodedConeCoordinate::decode)?,
            identity: decoder.field(2, DecodedPersistentId::decode)?,
            hir_fingerprint: decoder.field(3, Digest256::decode)?,
            mir_fingerprint: decoder.field(4, Digest256::decode)?,
            lir_fingerprint: decoder.field(5, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyRecordValidationError {
    Coordinate(ConeCoordinateError),
    Hash(HashError),
    Identity(PersistentIdMismatch<ConeIdentity>),
    Resource(WireError),
}

impl fmt::Display for DependencyRecordValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DependencyRecordValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedFingerprintAvailability {
    Unavailable,
    Available(Digest256),
}

impl WireEncode for DecodedFingerprintAvailability {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unavailable => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Available(fingerprint) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                fingerprint.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedFingerprintAvailability {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Unavailable)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::Available(decoder.field(1, Digest256::decode)?))
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedSemanticFingerprintRecord {
    hir: Digest256,
    mir: Digest256,
    lir: Digest256,
    code: DecodedFingerprintAvailability,
    runtime_image: DecodedFingerprintAvailability,
}

impl DecodedSemanticFingerprintRecord {
    pub(super) fn validate(
        self,
        profile: ArtifactCapabilityProfile,
    ) -> Result<SemanticFingerprintRecord, SemanticFingerprintValidationError> {
        let descriptor = profile.descriptor();
        let code = validate_code_availability(self.code, descriptor.code_requirement())?;
        let runtime_image =
            validate_runtime_availability(self.runtime_image, descriptor.runtime_requirement())?;
        Ok(SemanticFingerprintRecord::from_validated_digests(
            HirFingerprint::from_array(*self.hir.as_array()),
            MirFingerprint::from_array(*self.mir.as_array()),
            LirFingerprint::from_array(*self.lir.as_array()),
            code,
            runtime_image,
        ))
    }
}

fn validate_code_availability(
    decoded: DecodedFingerprintAvailability,
    requirement: FingerprintAvailabilityRequirement,
) -> Result<FingerprintAvailability<CodeFingerprint>, SemanticFingerprintValidationError> {
    match (requirement, decoded) {
        (
            FingerprintAvailabilityRequirement::MustBeUnavailable,
            DecodedFingerprintAvailability::Unavailable,
        ) => Ok(FingerprintAvailability::Unavailable),
        (
            FingerprintAvailabilityRequirement::MustBeAvailable,
            DecodedFingerprintAvailability::Available(value),
        ) => Ok(FingerprintAvailability::Available(
            CodeFingerprint::from_array(*value.as_array()),
        )),
        (FingerprintAvailabilityRequirement::MustBeUnavailable, _) => {
            Err(SemanticFingerprintValidationError::CodeMustBeUnavailable)
        }
        (FingerprintAvailabilityRequirement::MustBeAvailable, _) => {
            Err(SemanticFingerprintValidationError::CodeMustBeAvailable)
        }
    }
}

fn validate_runtime_availability(
    decoded: DecodedFingerprintAvailability,
    requirement: FingerprintAvailabilityRequirement,
) -> Result<FingerprintAvailability<RuntimeImageFingerprint>, SemanticFingerprintValidationError> {
    match (requirement, decoded) {
        (
            FingerprintAvailabilityRequirement::MustBeUnavailable,
            DecodedFingerprintAvailability::Unavailable,
        ) => Ok(FingerprintAvailability::Unavailable),
        (
            FingerprintAvailabilityRequirement::MustBeAvailable,
            DecodedFingerprintAvailability::Available(value),
        ) => Ok(FingerprintAvailability::Available(
            RuntimeImageFingerprint::from_array(*value.as_array()),
        )),
        (FingerprintAvailabilityRequirement::MustBeUnavailable, _) => {
            Err(SemanticFingerprintValidationError::RuntimeImageMustBeUnavailable)
        }
        (FingerprintAvailabilityRequirement::MustBeAvailable, _) => {
            Err(SemanticFingerprintValidationError::RuntimeImageMustBeAvailable)
        }
    }
}

impl WireEncode for DecodedSemanticFingerprintRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.hir.encode(encoder)?;
        encoder.field(2)?;
        self.mir.encode(encoder)?;
        encoder.field(3)?;
        self.lir.encode(encoder)?;
        encoder.field(4)?;
        self.code.encode(encoder)?;
        encoder.field(5)?;
        self.runtime_image.encode(encoder)
    }
}

impl WireDecode for DecodedSemanticFingerprintRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            hir: decoder.field(1, Digest256::decode)?,
            mir: decoder.field(2, Digest256::decode)?,
            lir: decoder.field(3, Digest256::decode)?,
            code: decoder.field(4, DecodedFingerprintAvailability::decode)?,
            runtime_image: decoder.field(5, DecodedFingerprintAvailability::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticFingerprintValidationError {
    CodeMustBeAvailable,
    CodeMustBeUnavailable,
    RuntimeImageMustBeAvailable,
    RuntimeImageMustBeUnavailable,
}

impl fmt::Display for SemanticFingerprintValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CodeMustBeAvailable => "production artifact code fingerprint must be available",
            Self::CodeMustBeUnavailable => {
                "foundation artifact code fingerprint must be unavailable"
            }
            Self::RuntimeImageMustBeAvailable => {
                "production artifact runtime image fingerprint must be available"
            }
            Self::RuntimeImageMustBeUnavailable => {
                "foundation artifact runtime image fingerprint must be unavailable"
            }
        })
    }
}

impl std::error::Error for SemanticFingerprintValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedManifestSection {
    capability: DecodedCapabilityId,
    required_for: u32,
    payload: Vec<u8>,
}

impl DecodedManifestSection {
    pub(super) fn validate(self) -> Result<ManifestSection, ManifestSectionError> {
        let capability = self
            .capability
            .validate()
            .map_err(ManifestSectionError::Capability)?;
        ManifestSection::new(
            capability,
            MemberPurposeSet::from_bits(self.required_for),
            self.payload,
        )
    }
}

impl WireEncode for DecodedManifestSection {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.capability.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.required_for))?;
        encoder.field(3)?;
        encoder.bytes(&self.payload)
    }
}

impl WireDecode for DecodedManifestSection {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            capability: decoder.field(1, DecodedCapabilityId::decode)?,
            required_for: decoder.field(2, Decoder::u32)?,
            payload: decoder.field(3, Decoder::owned_carrier_bytes)?,
        })
    }
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
mod tests {
    use super::*;

    #[test]
    fn semantic_fingerprint_availability_follows_the_selected_profile() {
        let foundation = decoded(
            DecodedFingerprintAvailability::Unavailable,
            DecodedFingerprintAvailability::Unavailable,
        )
        .validate(ArtifactCapabilityProfile::IDENTITY_FOUNDATION)
        .unwrap();
        assert_eq!(foundation.code(), FingerprintAvailability::Unavailable);
        assert_eq!(
            foundation.runtime_image(),
            FingerprintAvailability::Unavailable
        );

        let production = decoded(
            DecodedFingerprintAvailability::Available(Digest256::from_array([4; 32])),
            DecodedFingerprintAvailability::Available(Digest256::from_array([5; 32])),
        )
        .validate(ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
        .unwrap();
        assert!(matches!(
            production.code(),
            FingerprintAvailability::Available(_)
        ));
        assert!(matches!(
            production.runtime_image(),
            FingerprintAvailability::Available(_)
        ));
    }

    #[test]
    fn semantic_fingerprint_availability_rejects_cross_profile_shapes() {
        assert_eq!(
            decoded(
                DecodedFingerprintAvailability::Unavailable,
                DecodedFingerprintAvailability::Unavailable,
            )
            .validate(ArtifactCapabilityProfile::SINGLE_CONE_STRONG),
            Err(SemanticFingerprintValidationError::CodeMustBeAvailable)
        );
        assert_eq!(
            decoded(
                DecodedFingerprintAvailability::Available(Digest256::from_array([4; 32])),
                DecodedFingerprintAvailability::Available(Digest256::from_array([5; 32])),
            )
            .validate(ArtifactCapabilityProfile::IDENTITY_FOUNDATION),
            Err(SemanticFingerprintValidationError::CodeMustBeUnavailable)
        );
    }

    fn decoded(
        code: DecodedFingerprintAvailability,
        runtime_image: DecodedFingerprintAvailability,
    ) -> DecodedSemanticFingerprintRecord {
        DecodedSemanticFingerprintRecord {
            hir: Digest256::from_array([1; 32]),
            mir: Digest256::from_array([2; 32]),
            lir: Digest256::from_array([3; 32]),
            code,
            runtime_image,
        }
    }
}
