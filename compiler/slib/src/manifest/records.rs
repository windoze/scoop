use std::fmt;

use scoop_identity::{CapabilityId, CapabilityIdError, ConeCoordinate, ConeIdentity};
use scoop_wire::{Encoder, HashError, WireEncode, byte_span, encode, sha256};

use super::{
    ArtifactFingerprint, CodeFingerprint, FingerprintAvailability, HirFingerprint, LirFingerprint,
    MirFingerprint, RuntimeImageFingerprint,
};
use crate::{CompatibilityRecord, MemberPurposeSet, SlibMember, SlibMemberId, SlibMemberRecord};

const MANIFEST_MAGIC: &[u8; 9] = b"SCOOPSLIB";
const INITIAL_SCHEMA: u64 = 1;
const ARTIFACT_FINGERPRINT_DOMAIN: &[u8] = b"scoop-artifact-v1";
const MAX_PRODUCER_BYTES: usize = 255;
const MAX_MEMBERS: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerRecord {
    compiler_version: String,
}

impl ProducerRecord {
    pub fn new(compiler_version: &str) -> Result<Self, ProducerRecordError> {
        if compiler_version.len() > MAX_PRODUCER_BYTES {
            return Err(ProducerRecordError::TooLong {
                actual: compiler_version.len(),
            });
        }
        Ok(Self {
            compiler_version: compiler_version.to_owned(),
        })
    }

    pub fn compiler_version(&self) -> &str {
        &self.compiler_version
    }
}

impl WireEncode for ProducerRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(1)?;
        encoder.text(&self.compiler_version)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducerRecordError {
    TooLong { actual: usize },
}

impl fmt::Display for ProducerRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { actual } => {
                write!(
                    formatter,
                    "producer text exceeds 255 UTF-8 bytes: found {actual}"
                )
            }
        }
    }
}

impl std::error::Error for ProducerRecordError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConeKind {
    Library,
    Executable,
}

impl WireEncode for ConeKind {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Library => 1,
            Self::Executable => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConeSourceForm {
    Manifest,
    SingleFile,
}

impl WireEncode for ConeSourceForm {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Manifest => 1,
            Self::SingleFile => 2,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeRecord {
    coordinate: ConeCoordinate,
    identity: ConeIdentity,
    kind: ConeKind,
    source_form: ConeSourceForm,
}

impl ConeRecord {
    pub fn new(
        coordinate: ConeCoordinate,
        kind: ConeKind,
        source_form: ConeSourceForm,
    ) -> Result<Self, ConeRecordError> {
        let is_reserved_single_file = coordinate == ConeCoordinate::reserved_single_file();
        if is_reserved_single_file != (source_form == ConeSourceForm::SingleFile) {
            return Err(ConeRecordError::SingleFileCoordinateMismatch);
        }
        let identity = coordinate.identity().map_err(ConeRecordError::Hash)?;
        Ok(Self {
            coordinate,
            identity,
            kind,
            source_form,
        })
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub const fn kind(&self) -> ConeKind {
        self.kind
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.source_form
    }
}

impl WireEncode for ConeRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.coordinate.encode(encoder)?;
        encoder.field(2)?;
        self.identity.encode(encoder)?;
        encoder.field(3)?;
        self.kind.encode(encoder)?;
        encoder.field(4)?;
        self.source_form.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeRecordError {
    SingleFileCoordinateMismatch,
    Hash(HashError),
}

impl fmt::Display for ConeRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingleFileCoordinateMismatch => formatter.write_str(
                "single-file source form and the reserved single-file coordinate must appear together",
            ),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConeRecordError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRecord {
    coordinate: ConeCoordinate,
    identity: ConeIdentity,
    hir_fingerprint: HirFingerprint,
    mir_fingerprint: MirFingerprint,
    lir_fingerprint: LirFingerprint,
}

impl DependencyRecord {
    pub fn new(
        coordinate: ConeCoordinate,
        hir_fingerprint: HirFingerprint,
        mir_fingerprint: MirFingerprint,
        lir_fingerprint: LirFingerprint,
    ) -> Result<Self, HashError> {
        let identity = coordinate.identity()?;
        Ok(Self {
            coordinate,
            identity,
            hir_fingerprint,
            mir_fingerprint,
            lir_fingerprint,
        })
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn hir_fingerprint(&self) -> HirFingerprint {
        self.hir_fingerprint
    }

    pub const fn mir_fingerprint(&self) -> MirFingerprint {
        self.mir_fingerprint
    }

    pub const fn lir_fingerprint(&self) -> LirFingerprint {
        self.lir_fingerprint
    }
}

impl WireEncode for DependencyRecord {
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticFingerprintRecord {
    hir: HirFingerprint,
    mir: MirFingerprint,
    lir: LirFingerprint,
    code: FingerprintAvailability<CodeFingerprint>,
    runtime_image: FingerprintAvailability<RuntimeImageFingerprint>,
}

impl SemanticFingerprintRecord {
    pub(crate) fn from_foundation_digests(
        hir: HirFingerprint,
        mir: MirFingerprint,
        lir: LirFingerprint,
    ) -> Self {
        Self {
            hir,
            mir,
            lir,
            code: FingerprintAvailability::Unavailable,
            runtime_image: FingerprintAvailability::Unavailable,
        }
    }

    pub const fn hir(self) -> HirFingerprint {
        self.hir
    }

    pub const fn mir(self) -> MirFingerprint {
        self.mir
    }

    pub const fn lir(self) -> LirFingerprint {
        self.lir
    }
}

impl WireEncode for SemanticFingerprintRecord {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestSection {
    capability: CapabilityId,
    required_for: MemberPurposeSet,
    payload: Vec<u8>,
}

impl ManifestSection {
    pub fn new(
        capability: CapabilityId,
        required_for: MemberPurposeSet,
        payload: Vec<u8>,
    ) -> Result<Self, ManifestSectionError> {
        if capability == crate::hir_identity_foundation_capability()
            || capability == crate::mir_identity_foundation_capability()
            || capability == crate::lir_identity_foundation_capability()
        {
            return Err(ManifestSectionError::KnownCapabilityWrongLocation { capability });
        }
        if !matches!(required_for.bits(), 0 | 2 | 4 | 6) {
            return Err(ManifestSectionError::InvalidPurpose {
                bits: required_for.bits(),
            });
        }
        Ok(Self {
            capability,
            required_for,
            payload,
        })
    }

    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub const fn required_for(&self) -> MemberPurposeSet {
        self.required_for
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl WireEncode for ManifestSection {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.capability.encode(encoder)?;
        encoder.field(2)?;
        self.required_for.encode(encoder)?;
        encoder.field(3)?;
        encoder.bytes(&self.payload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestSectionError {
    InvalidPurpose { bits: u32 },
    Capability(CapabilityIdError),
    KnownCapabilityWrongLocation { capability: CapabilityId },
}

impl fmt::Display for ManifestSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPurpose { bits } => write!(
                formatter,
                "manifest section required_for must be 0, Compile, Link, or Compile|Link; found {bits:#x}"
            ),
            Self::Capability(error) => error.fmt(formatter),
            Self::KnownCapabilityWrongLocation { capability } => write!(
                formatter,
                "capability {}/{}/{} belongs in its metadata envelope, not the manifest",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
        }
    }
}

impl std::error::Error for ManifestSectionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapManifest {
    producer: ProducerRecord,
    compatibility: CompatibilityRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    members: Vec<SlibMemberRecord>,
    semantic_fingerprints: SemanticFingerprintRecord,
    sections: Vec<ManifestSection>,
    artifact_fingerprint: ArtifactFingerprint,
}

impl BootstrapManifest {
    pub fn new(
        producer: ProducerRecord,
        compatibility: CompatibilityRecord,
        cone: ConeRecord,
        mut direct_dependencies: Vec<DependencyRecord>,
        members: &[SlibMember],
        semantic_fingerprints: SemanticFingerprintRecord,
        mut sections: Vec<ManifestSection>,
    ) -> Result<Self, BootstrapManifestError> {
        direct_dependencies.sort_unstable_by_key(DependencyRecord::identity);
        let mut member_records = members
            .iter()
            .map(|member| member.record().clone())
            .collect::<Vec<_>>();
        member_records.sort_unstable_by_key(SlibMemberRecord::id);
        sections.sort_unstable_by(|left, right| left.capability.cmp(&right.capability));
        Self::from_validated_records(
            producer,
            compatibility,
            cone,
            direct_dependencies,
            member_records,
            semantic_fingerprints,
            sections,
        )
    }

    pub(super) fn from_validated_records(
        producer: ProducerRecord,
        compatibility: CompatibilityRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        members: Vec<SlibMemberRecord>,
        semantic_fingerprints: SemanticFingerprintRecord,
        sections: Vec<ManifestSection>,
    ) -> Result<Self, BootstrapManifestError> {
        if members.len() > MAX_MEMBERS {
            return Err(BootstrapManifestError::TooManyMembers {
                actual: members.len(),
            });
        }
        reject_duplicate_dependencies(&direct_dependencies)?;
        reject_duplicate_members(&members)?;
        require_foundation_metadata(&members)?;
        reject_duplicate_sections(&sections)?;
        let input = ArtifactManifestInput {
            producer: &producer,
            compatibility: &compatibility,
            cone: &cone,
            direct_dependencies: &direct_dependencies,
            members: &members,
            semantic_fingerprints: &semantic_fingerprints,
            sections: &sections,
        };
        let artifact_fingerprint = calculate_artifact_fingerprint(&input, &members)
            .map_err(BootstrapManifestError::Hash)?;
        Ok(Self {
            producer,
            compatibility,
            cone,
            direct_dependencies,
            members,
            semantic_fingerprints,
            sections,
            artifact_fingerprint,
        })
    }

    pub const fn cone(&self) -> &ConeRecord {
        &self.cone
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        &self.direct_dependencies
    }

    pub fn members(&self) -> &[SlibMemberRecord] {
        &self.members
    }

    pub fn sections(&self) -> &[ManifestSection] {
        &self.sections
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }
}

impl WireEncode for BootstrapManifest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        ArtifactManifestInput {
            producer: &self.producer,
            compatibility: &self.compatibility,
            cone: &self.cone,
            direct_dependencies: &self.direct_dependencies,
            members: &self.members,
            semantic_fingerprints: &self.semantic_fingerprints,
            sections: &self.sections,
        }
        .encode_fields(encoder)?;
        encoder.field(11)?;
        self.artifact_fingerprint.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapManifestError {
    DuplicateDependency { identity: ConeIdentity },
    DuplicateMember { id: SlibMemberId },
    TooManyMembers { actual: usize },
    MissingMetadata { kind: MetadataKind },
    DuplicateSection { capability: CapabilityId },
    Hash(HashError),
}

impl fmt::Display for BootstrapManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDependency { identity } => {
                write!(formatter, "duplicate direct dependency {identity}")
            }
            Self::DuplicateMember { id } => write!(formatter, "duplicate member {id}"),
            Self::TooManyMembers { actual } => {
                write!(formatter, "manifest exceeds 65536 members: found {actual}")
            }
            Self::MissingMetadata { kind } => write!(formatter, "missing {kind} metadata member"),
            Self::DuplicateSection { capability } => write!(
                formatter,
                "duplicate manifest section capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BootstrapManifestError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataKind {
    Hir,
    Mir,
    Lir,
}

impl fmt::Display for MetadataKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

struct ArtifactManifestInput<'manifest> {
    producer: &'manifest ProducerRecord,
    compatibility: &'manifest CompatibilityRecord,
    cone: &'manifest ConeRecord,
    direct_dependencies: &'manifest [DependencyRecord],
    members: &'manifest [SlibMemberRecord],
    semantic_fingerprints: &'manifest SemanticFingerprintRecord,
    sections: &'manifest [ManifestSection],
}

impl WireEncode for ArtifactManifestInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        self.encode_fields(encoder)
    }
}

impl ArtifactManifestInput<'_> {
    fn encode_fields(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        encoder.bytes(MANIFEST_MAGIC)?;
        encoder.field(2)?;
        encoder.unsigned(INITIAL_SCHEMA)?;
        encoder.field(3)?;
        encoder.unsigned(INITIAL_SCHEMA)?;
        encoder.field(4)?;
        self.producer.encode(encoder)?;
        encoder.field(5)?;
        self.compatibility.encode(encoder)?;
        encoder.field(6)?;
        self.cone.encode(encoder)?;
        encode_array_field(encoder, 7, self.direct_dependencies)?;
        encode_array_field(encoder, 8, self.members)?;
        encoder.field(9)?;
        self.semantic_fingerprints.encode(encoder)?;
        encode_array_field(encoder, 10, self.sections)
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

fn calculate_artifact_fingerprint(
    input: &ArtifactManifestInput<'_>,
    members: &[SlibMemberRecord],
) -> Result<ArtifactFingerprint, HashError> {
    let encoded_input = encode(input).map_err(|_| HashError::CborEncoding)?;
    let mut bytes = byte_span(ARTIFACT_FINGERPRINT_DOMAIN)?;
    append_span(&mut bytes, &encoded_input)?;
    for member in members {
        append_span(&mut bytes, member.fingerprint()?.as_array())?;
    }
    Ok(ArtifactFingerprint::from_array(*sha256(&bytes).as_array()))
}

fn append_span(output: &mut Vec<u8>, value: &[u8]) -> Result<(), HashError> {
    let span = byte_span(value)?;
    output
        .try_reserve_exact(span.len())
        .map_err(|_| HashError::LengthOverflow)?;
    output.extend_from_slice(&span);
    Ok(())
}

fn reject_duplicate_dependencies(
    dependencies: &[DependencyRecord],
) -> Result<(), BootstrapManifestError> {
    if let Some(pair) = dependencies
        .windows(2)
        .find(|pair| pair[0].identity == pair[1].identity)
    {
        Err(BootstrapManifestError::DuplicateDependency {
            identity: pair[0].identity,
        })
    } else {
        Ok(())
    }
}

fn reject_duplicate_members(members: &[SlibMemberRecord]) -> Result<(), BootstrapManifestError> {
    if let Some(pair) = members.windows(2).find(|pair| pair[0].id() == pair[1].id()) {
        Err(BootstrapManifestError::DuplicateMember { id: pair[0].id() })
    } else {
        Ok(())
    }
}

fn require_foundation_metadata(members: &[SlibMemberRecord]) -> Result<(), BootstrapManifestError> {
    for (kind, present) in [
        (
            MetadataKind::Hir,
            members
                .iter()
                .any(|member| matches!(member.stable_key(), crate::MemberStableKey::HirMetadata)),
        ),
        (
            MetadataKind::Mir,
            members
                .iter()
                .any(|member| matches!(member.stable_key(), crate::MemberStableKey::MirMetadata)),
        ),
        (
            MetadataKind::Lir,
            members
                .iter()
                .any(|member| matches!(member.stable_key(), crate::MemberStableKey::LirMetadata)),
        ),
    ] {
        if !present {
            return Err(BootstrapManifestError::MissingMetadata { kind });
        }
    }
    Ok(())
}

fn reject_duplicate_sections(sections: &[ManifestSection]) -> Result<(), BootstrapManifestError> {
    if let Some(pair) = sections
        .windows(2)
        .find(|pair| pair[0].capability == pair[1].capability)
    {
        Err(BootstrapManifestError::DuplicateSection {
            capability: pair[0].capability.clone(),
        })
    } else {
        Ok(())
    }
}
