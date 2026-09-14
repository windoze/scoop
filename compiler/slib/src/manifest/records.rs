use std::fmt;

use scoop_identity::{CapabilityId, CapabilityIdError, ConeCoordinate, ConeIdentity};
use scoop_wire::{
    BudgetMeter, CanonicalHashStream, Encoder, HashError, WireEncode, WireError, encoded_length,
};

use super::{
    ArtifactFingerprint, CodeFingerprint, FingerprintAvailability, HirFingerprint, LirFingerprint,
    MirFingerprint, RuntimeImageFingerprint,
};
use crate::{CompatibilityRecord, MemberPurposeSet, SlibMember, SlibMemberId, SlibMemberRecord};

const MANIFEST_MAGIC: &[u8; 9] = b"SCOOPSLIB";
const INITIAL_SCHEMA: u64 = 1;
const ARTIFACT_FINGERPRINT_DOMAIN: &[u8] = b"scoop-artifact-v1";
const MAX_PRODUCER_BYTES: usize = 255;
pub(super) const MAX_MEMBERS: usize = 65_536;

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

    pub(super) fn from_owned(compiler_version: String) -> Result<Self, ProducerRecordError> {
        if compiler_version.len() > MAX_PRODUCER_BYTES {
            return Err(ProducerRecordError::TooLong {
                actual: compiler_version.len(),
            });
        }
        Ok(Self { compiler_version })
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
        let identity = coordinate.identity().map_err(ConeRecordError::Hash)?;
        Self::from_validated(coordinate, identity, kind, source_form)
    }

    pub(super) fn from_validated(
        coordinate: ConeCoordinate,
        identity: ConeIdentity,
        kind: ConeKind,
        source_form: ConeSourceForm,
    ) -> Result<Self, ConeRecordError> {
        let is_reserved_single_file = coordinate == ConeCoordinate::reserved_single_file();
        if is_reserved_single_file != (source_form == ConeSourceForm::SingleFile) {
            return Err(ConeRecordError::SingleFileCoordinateMismatch);
        }
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
        Ok(Self::from_validated(
            coordinate,
            identity,
            hir_fingerprint,
            mir_fingerprint,
            lir_fingerprint,
        ))
    }

    pub(super) const fn from_validated(
        coordinate: ConeCoordinate,
        identity: ConeIdentity,
        hir_fingerprint: HirFingerprint,
        mir_fingerprint: MirFingerprint,
        lir_fingerprint: LirFingerprint,
    ) -> Self {
        Self {
            coordinate,
            identity,
            hir_fingerprint,
            mir_fingerprint,
            lir_fingerprint,
        }
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

    pub fn from_production_manifest(
        hir: HirFingerprint,
        mir: MirFingerprint,
        lir: LirFingerprint,
        production: &super::SingleConeProductionManifestV1,
    ) -> Self {
        Self {
            hir,
            mir,
            lir,
            code: FingerprintAvailability::Available(production.code_fingerprint()),
            runtime_image: FingerprintAvailability::Available(
                production.runtime_image_fingerprint(),
            ),
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

    pub const fn code(self) -> FingerprintAvailability<CodeFingerprint> {
        self.code
    }

    pub const fn runtime_image(self) -> FingerprintAvailability<RuntimeImageFingerprint> {
        self.runtime_image
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
        if !matches!(required_for.bits(), 0 | 2 | 4 | 6) {
            return Err(ManifestSectionError::InvalidPurpose {
                bits: required_for.bits(),
            });
        }
        if let Some(contract) = crate::CapabilityContractRegistry::contract(&capability) {
            if contract.location() != crate::SectionLocation::Manifest {
                return Err(ManifestSectionError::KnownCapabilityWrongLocation {
                    capability,
                    expected: contract.location(),
                });
            }
            if contract.required_for() != required_for {
                return Err(ManifestSectionError::KnownCapabilityWrongPurpose {
                    capability,
                    expected: contract.required_for(),
                    bits: required_for.bits(),
                });
            }
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
    InvalidPurpose {
        bits: u32,
    },
    Capability(CapabilityIdError),
    KnownCapabilityWrongLocation {
        capability: CapabilityId,
        expected: crate::SectionLocation,
    },
    KnownCapabilityWrongPurpose {
        capability: CapabilityId,
        expected: MemberPurposeSet,
        bits: u32,
    },
}

impl fmt::Display for ManifestSectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPurpose { bits } => write!(
                formatter,
                "manifest section required_for must be 0, Compile, Link, or Compile|Link; found {bits:#x}"
            ),
            Self::Capability(error) => error.fmt(formatter),
            Self::KnownCapabilityWrongLocation {
                capability,
                expected,
            } => write!(
                formatter,
                "capability {}/{}/{} belongs in {expected}, not Manifest",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::KnownCapabilityWrongPurpose {
                capability,
                expected,
                bits,
            } => write!(
                formatter,
                "capability {}/{}/{} requires purpose bits {:#x}, found {bits:#x}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
                expected.bits(),
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

pub(super) struct CanonicalManifestRecords {
    pub(super) producer: ProducerRecord,
    pub(super) compatibility: CompatibilityRecord,
    pub(super) cone: ConeRecord,
    pub(super) direct_dependencies: Vec<DependencyRecord>,
    pub(super) members: Vec<SlibMemberRecord>,
    pub(super) semantic_fingerprints: SemanticFingerprintRecord,
    pub(super) sections: Vec<ManifestSection>,
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
        if member_records.len() > MAX_MEMBERS {
            return Err(BootstrapManifestError::TooManyMembers {
                actual: member_records.len(),
            });
        }
        reject_duplicate_dependencies(&direct_dependencies)?;
        reject_duplicate_members(&member_records)?;
        require_foundation_metadata(&member_records)?;
        reject_duplicate_sections(&sections)?;
        Self::build(
            CanonicalManifestRecords {
                producer,
                compatibility,
                cone,
                direct_dependencies,
                members: member_records,
                semantic_fingerprints,
                sections,
            },
            None,
        )
    }

    pub(super) fn from_canonical_records(
        records: CanonicalManifestRecords,
        meter: &mut BudgetMeter,
    ) -> Result<Self, BootstrapManifestError> {
        if records.members.len() > MAX_MEMBERS {
            return Err(BootstrapManifestError::TooManyMembers {
                actual: records.members.len(),
            });
        }
        require_foundation_metadata(&records.members)?;
        Self::build(records, Some(meter))
    }

    fn build(
        records: CanonicalManifestRecords,
        meter: Option<&mut BudgetMeter>,
    ) -> Result<Self, BootstrapManifestError> {
        let input = ArtifactManifestInput {
            producer: &records.producer,
            compatibility: &records.compatibility,
            cone: &records.cone,
            direct_dependencies: &records.direct_dependencies,
            members: &records.members,
            semantic_fingerprints: &records.semantic_fingerprints,
            sections: &records.sections,
        };
        let artifact_fingerprint = calculate_artifact_fingerprint(&input, &records.members, meter)?;
        Ok(Self {
            producer: records.producer,
            compatibility: records.compatibility,
            cone: records.cone,
            direct_dependencies: records.direct_dependencies,
            members: records.members,
            semantic_fingerprints: records.semantic_fingerprints,
            sections: records.sections,
            artifact_fingerprint,
        })
    }

    pub const fn cone(&self) -> &ConeRecord {
        &self.cone
    }

    pub const fn compatibility(&self) -> &CompatibilityRecord {
        &self.compatibility
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
    Resource(WireError),
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
            Self::Resource(error) => error.fmt(formatter),
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
    meter: Option<&mut BudgetMeter>,
) -> Result<ArtifactFingerprint, BootstrapManifestError> {
    if let Some(meter) = meter {
        meter
            .charge_sha256(
                artifact_fingerprint_hash_stream_length(input, members.len())
                    .map_err(BootstrapManifestError::Hash)?,
                &Default::default(),
            )
            .map_err(BootstrapManifestError::Resource)?;
        for member in members {
            meter
                .charge_sha256(
                    member
                        .fingerprint_hash_stream_length()
                        .map_err(BootstrapManifestError::Hash)?,
                    &Default::default(),
                )
                .map_err(BootstrapManifestError::Resource)?;
        }
    }

    let mut stream = CanonicalHashStream::new();
    stream
        .update_byte_span(ARTIFACT_FINGERPRINT_DOMAIN)
        .map_err(BootstrapManifestError::Hash)?;
    stream
        .update_canonical_cbor_span(input)
        .map_err(BootstrapManifestError::Hash)?;
    for member in members {
        let fingerprint = member.fingerprint().map_err(BootstrapManifestError::Hash)?;
        stream
            .update_byte_span(fingerprint.as_array())
            .map_err(BootstrapManifestError::Hash)?;
    }
    Ok(ArtifactFingerprint::from_array(
        *stream.finalize().as_array(),
    ))
}

fn artifact_fingerprint_hash_stream_length(
    input: &ArtifactManifestInput<'_>,
    member_count: usize,
) -> Result<u64, HashError> {
    let domain_length =
        u64::try_from(ARTIFACT_FINGERPRINT_DOMAIN.len()).map_err(|_| HashError::LengthOverflow)?;
    let input_length = encoded_length(input).map_err(|_| HashError::CborEncoding)?;
    let member_count = u64::try_from(member_count).map_err(|_| HashError::LengthOverflow)?;
    let member_spans = member_count
        .checked_mul(40)
        .ok_or(HashError::LengthOverflow)?;
    8_u64
        .checked_add(domain_length)
        .and_then(|length| length.checked_add(8))
        .and_then(|length| length.checked_add(input_length))
        .and_then(|length| length.checked_add(member_spans))
        .ok_or(HashError::LengthOverflow)
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
