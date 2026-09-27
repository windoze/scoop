mod compatibility;
use compatibility::DecodedCompatibilityRecord;
pub use compatibility::{
    CompatibilityFingerprintKind, CompatibilitySchemaKind, CompatibilityValidationError,
};

mod records;
pub use records::{
    ConeRecordValidationError, DecodedConeRecord, DecodedDependencyRecord,
    DependencyRecordValidationError, SemanticFingerprintValidationError,
};
use records::{DecodedManifestSection, DecodedSemanticFingerprintRecord};

use std::fmt;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{Decoder, Digest256, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::{
    ArtifactFingerprint, BootstrapManifest, BootstrapManifestError, CanonicalManifestRecords,
    ProducerRecord, ProducerRecordError,
};
use crate::{DecodedSlibMemberRecord, SlibMemberRecordValidationError};

const MANIFEST_MAGIC: &[u8; 9] = b"SCOOPSLIB";
const INITIAL_SCHEMA: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedBootstrapManifest {
    magic: Vec<u8>,
    manifest_schema: u32,
    container_schema: u32,
    producer: DecodedProducerRecord,
    compatibility: DecodedCompatibilityRecord,
    cone: DecodedConeRecord,
    direct_dependencies: Vec<DecodedDependencyRecord>,
    members: Vec<DecodedSlibMemberRecord>,
    semantic_fingerprints: DecodedSemanticFingerprintRecord,
    sections: Vec<DecodedManifestSection>,
    artifact_fingerprint: Digest256,
}

impl DecodedBootstrapManifest {
    pub fn validate(
        self,
        selection: ValidatedLirTargetSelection,
    ) -> Result<BootstrapManifest, BootstrapManifestValidationError> {
        if self.magic != MANIFEST_MAGIC {
            return Err(BootstrapManifestValidationError::BadMagic);
        }
        require_initial_schema(SchemaKind::Manifest, self.manifest_schema)?;
        require_initial_schema(SchemaKind::Container, self.container_schema)?;

        let producer = self
            .producer
            .validate()
            .map_err(BootstrapManifestValidationError::Producer)?;
        let compatibility = self
            .compatibility
            .validate(selection)
            .map_err(BootstrapManifestValidationError::Compatibility)?;
        let cone = self
            .cone
            .validate()
            .map_err(BootstrapManifestValidationError::Cone)?;
        let cone_identity = cone.identity();

        let mut dependencies = Vec::new();
        let dependency_path = WirePath::root().field(7);
        let dependency_count = u64::try_from(self.direct_dependencies.len())
            .map_err(|_| BootstrapManifestValidationError::Wire(integer_range(&dependency_path)))?;
        scoop_wire::allocation::try_reserve_count(
            &mut dependencies,
            dependency_count,
            &dependency_path,
        )
        .map_err(BootstrapManifestValidationError::Wire)?;
        for (index, dependency) in self.direct_dependencies.into_iter().enumerate() {
            dependencies.push(
                dependency.validate().map_err(|error| {
                    BootstrapManifestValidationError::Dependency { index, error }
                })?,
            );
        }

        require_strictly_increasing_dependencies(&dependencies)?;

        let mut members = Vec::new();
        let member_path = WirePath::root().field(8);
        let member_count = u64::try_from(self.members.len())
            .map_err(|_| BootstrapManifestValidationError::Wire(integer_range(&member_path)))?;
        scoop_wire::allocation::try_reserve_count(&mut members, member_count, &member_path)
            .map_err(BootstrapManifestValidationError::Wire)?;
        for (index, member) in self.members.into_iter().enumerate() {
            members.push(member.validate(cone_identity).map_err(|error| {
                BootstrapManifestValidationError::Member {
                    index,
                    error: Box::new(error),
                }
            })?);
        }

        require_strictly_increasing_members(&members)?;

        let semantic_fingerprints = self
            .semantic_fingerprints
            .validate()
            .map_err(BootstrapManifestValidationError::SemanticFingerprints)?;

        let mut sections = Vec::new();
        let section_path = WirePath::root().field(10);
        let section_count = u64::try_from(self.sections.len())
            .map_err(|_| BootstrapManifestValidationError::Wire(integer_range(&section_path)))?;
        scoop_wire::allocation::try_reserve_count(&mut sections, section_count, &section_path)
            .map_err(BootstrapManifestValidationError::Wire)?;
        for (index, section) in self.sections.into_iter().enumerate() {
            sections.push(
                section
                    .validate()
                    .map_err(|error| BootstrapManifestValidationError::Section { index, error })?,
            );
        }

        require_strictly_increasing_sections(&sections)?;

        let manifest = BootstrapManifest::from_canonical_records(CanonicalManifestRecords {
            producer,
            compatibility,
            cone,
            direct_dependencies: dependencies,
            members,
            semantic_fingerprints,
            sections,
        })
        .map_err(BootstrapManifestValidationError::Manifest)?;
        if self.artifact_fingerprint.as_array() != manifest.artifact_fingerprint().as_array() {
            return Err(
                BootstrapManifestValidationError::ArtifactFingerprintMismatch {
                    expected: manifest.artifact_fingerprint(),
                    actual: *self.artifact_fingerprint.as_array(),
                },
            );
        }
        Ok(manifest)
    }
}

impl WireEncode for DecodedBootstrapManifest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encoder.field(1)?;
        encoder.bytes(&self.magic)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.manifest_schema))?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.container_schema))?;
        encoder.field(4)?;
        self.producer.encode(encoder)?;
        encoder.field(5)?;
        self.compatibility.encode(encoder)?;
        encoder.field(6)?;
        self.cone.encode(encoder)?;
        encode_array_field(encoder, 7, &self.direct_dependencies)?;
        encode_array_field(encoder, 8, &self.members)?;
        encoder.field(9)?;
        self.semantic_fingerprints.encode(encoder)?;
        encode_array_field(encoder, 10, &self.sections)?;
        encoder.field(11)?;
        self.artifact_fingerprint.encode(encoder)
    }
}

impl WireDecode for DecodedBootstrapManifest {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(11)?;
        Ok(Self {
            magic: decoder.field(1, Decoder::owned_bytes)?,
            manifest_schema: decoder.field(2, Decoder::u32)?,
            container_schema: decoder.field(3, Decoder::u32)?,
            producer: decoder.field(4, DecodedProducerRecord::decode)?,
            compatibility: decoder.field(5, DecodedCompatibilityRecord::decode)?,
            cone: decoder.field(6, DecodedConeRecord::decode)?,
            direct_dependencies: decoder.field(7, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDependencyRecord::decode(decoder))
            })?,
            members: decoder.field(8, |decoder| {
                decoder.decode_array(|decoder, _| DecodedSlibMemberRecord::decode(decoder))
            })?,
            semantic_fingerprints: decoder.field(9, DecodedSemanticFingerprintRecord::decode)?,
            sections: decoder.field(10, |decoder| {
                decoder.decode_array(|decoder, _| DecodedManifestSection::decode(decoder))
            })?,
            artifact_fingerprint: decoder.field(11, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedProducerRecord {
    compiler_version: String,
}

impl DecodedProducerRecord {
    fn validate(self) -> Result<ProducerRecord, ProducerRecordError> {
        ProducerRecord::from_owned(self.compiler_version)
    }
}

impl WireEncode for DecodedProducerRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.text(&self.compiler_version)
    }
}

impl WireDecode for DecodedProducerRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        Ok(Self {
            compiler_version: decoder.field(1, Decoder::owned_text)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaKind {
    Manifest,
    Container,
}

impl fmt::Display for SchemaKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Manifest => "manifest",
            Self::Container => "container",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapManifestValidationError {
    BadMagic,
    UnsupportedSchema {
        kind: SchemaKind,
        actual: u32,
    },
    Producer(ProducerRecordError),
    Compatibility(CompatibilityValidationError),
    Cone(ConeRecordValidationError),
    Dependency {
        index: usize,
        error: DependencyRecordValidationError,
    },
    NonIncreasingDependency {
        first_index: usize,
        second_index: usize,
    },
    Member {
        index: usize,
        error: Box<SlibMemberRecordValidationError>,
    },
    NonIncreasingMember {
        first_index: usize,
        second_index: usize,
    },
    SemanticFingerprints(SemanticFingerprintValidationError),
    Section {
        index: usize,
        error: super::ManifestSectionError,
    },
    NonIncreasingSection {
        first_index: usize,
        second_index: usize,
    },
    Manifest(BootstrapManifestError),
    ArtifactFingerprintMismatch {
        expected: ArtifactFingerprint,
        actual: [u8; 32],
    },
    Wire(WireError),
}

impl fmt::Display for BootstrapManifestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("manifest magic must be SCOOPSLIB"),
            Self::UnsupportedSchema { kind, actual } => {
                write!(formatter, "{kind} schema must be 1, found {actual}")
            }
            Self::Producer(error) => error.fmt(formatter),
            Self::Compatibility(error) => error.fmt(formatter),
            Self::Cone(error) => error.fmt(formatter),
            Self::Dependency { index, error } => {
                write!(formatter, "invalid dependency {index}: {error}")
            }
            Self::NonIncreasingDependency {
                first_index,
                second_index,
            } => write!(
                formatter,
                "dependency identities at indexes {first_index} and {second_index} are not strictly increasing"
            ),
            Self::Member { index, error } => write!(formatter, "invalid member {index}: {error}"),
            Self::NonIncreasingMember {
                first_index,
                second_index,
            } => write!(
                formatter,
                "member ids at indexes {first_index} and {second_index} are not strictly increasing"
            ),
            Self::SemanticFingerprints(error) => error.fmt(formatter),
            Self::Section { index, error } => {
                write!(formatter, "invalid manifest section {index}: {error}")
            }
            Self::NonIncreasingSection {
                first_index,
                second_index,
            } => write!(
                formatter,
                "section capabilities at indexes {first_index} and {second_index} are not strictly increasing"
            ),
            Self::Manifest(error) => error.fmt(formatter),
            Self::ArtifactFingerprintMismatch { expected, actual } => {
                write!(
                    formatter,
                    "artifact fingerprint mismatch: expected {expected}, found "
                )?;
                write_hex(actual, formatter)
            }
            Self::Wire(error) => error.fmt(formatter),
        }
    }
}

fn integer_range(path: &WirePath) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::IntegerOutOfRange,
        path.clone(),
        None,
    )
}

impl std::error::Error for BootstrapManifestValidationError {}

fn require_initial_schema(
    kind: SchemaKind,
    actual: u32,
) -> Result<(), BootstrapManifestValidationError> {
    if actual == INITIAL_SCHEMA {
        Ok(())
    } else {
        Err(BootstrapManifestValidationError::UnsupportedSchema { kind, actual })
    }
}

fn require_strictly_increasing_dependencies(
    values: &[super::DependencyRecord],
) -> Result<(), BootstrapManifestValidationError> {
    if let Some((index, _)) = values
        .windows(2)
        .enumerate()
        .find(|(_, pair)| pair[0].identity() >= pair[1].identity())
    {
        Err(BootstrapManifestValidationError::NonIncreasingDependency {
            first_index: index,
            second_index: index + 1,
        })
    } else {
        Ok(())
    }
}

fn require_strictly_increasing_members(
    values: &[crate::SlibMemberRecord],
) -> Result<(), BootstrapManifestValidationError> {
    if let Some((index, _)) = values
        .windows(2)
        .enumerate()
        .find(|(_, pair)| pair[0].id() >= pair[1].id())
    {
        Err(BootstrapManifestValidationError::NonIncreasingMember {
            first_index: index,
            second_index: index + 1,
        })
    } else {
        Ok(())
    }
}

fn require_strictly_increasing_sections(
    values: &[super::ManifestSection],
) -> Result<(), BootstrapManifestValidationError> {
    if let Some((index, _)) = values
        .windows(2)
        .enumerate()
        .find(|(_, pair)| pair[0].capability() >= pair[1].capability())
    {
        Err(BootstrapManifestValidationError::NonIncreasingSection {
            first_index: index,
            second_index: index + 1,
        })
    } else {
        Ok(())
    }
}

fn encode_array_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
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

#[cfg(test)]
mod tests;
