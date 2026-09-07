//! `manifest.cbor` — the bootstrap member of every `.slib`
//! (DESIGN section 4.2).
//!
//! The manifest carries the artifact's identity, toolchain compatibility
//! fields, the dependency table, the typed member directory and the
//! whole-artifact fingerprint. Sections that depend on later milestone
//! features (ODR records, extern contracts, the image owner) are added
//! to this schema when their producers land; the fields present here are
//! all complete and validated.

use core::fmt;

use scoop_identity::{
    CapabilityId, CborReader, CborWriter, ConeCoordinate, ConeIdentity, Digest256, DomainHasher,
    TargetProfileWireId,
};
use scoop_manifest::ConeKind;

use crate::member::{SlibMemberId, SlibMemberRecord};

pub const MANIFEST_MEMBER_NAME: &str = "manifest.cbor";
pub const MANIFEST_MAGIC: &str = "SCOOPSLIB";
pub const ARTIFACT_FINGERPRINT_DOMAIN: &[u8] = b"scoop-artifact-v1";

/// The identity plus compatibility core of one artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestCore {
    pub container_version: u32,
    pub hir_wire_schema: u32,
    pub mir_wire_schema: u32,
    pub lir_wire_schema: u32,
    /// Producer compiler version; diagnostics only, never matched.
    pub producer_compiler_version: String,
    pub language_abi: u32,
    pub runtime_abi: Digest256,
    pub identity_schema_version: u32,
    pub coordinate: ConeCoordinate,
    pub cone_identity: ConeIdentity,
    pub kind: ConeKind,
    /// Direct dependencies as recorded at this Cone's compile time.
    pub dependencies: Vec<DependencyRecord>,
    pub target_profile: TargetProfileWireId,
    pub target_profile_fingerprint: Digest256,
    pub backend_profile_fingerprint: Digest256,
    /// Complete member directory (every non-manifest member), ordered by
    /// `SlibMemberId`.
    pub members: Vec<SlibMemberRecord>,
    /// Whole-artifact fingerprint; written last by the packager.
    pub artifact_fingerprint: ArtifactFingerprint,
}

/// One direct dependency record: coordinate, identity plus the three
/// semantic fingerprints the compile key consumes. The coordinate is
/// stored alongside the identity digest so build resolution can locate
/// transitive dependencies by canonical path; the reader verifies that
/// it hashes to the recorded identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRecord {
    pub coordinate: ConeCoordinate,
    pub cone_identity: ConeIdentity,
    pub hir_semantic_fingerprint: Digest256,
    pub mir_semantic_fingerprint: Digest256,
    pub lir_semantic_fingerprint: Digest256,
}

/// Whole-artifact fingerprint: computed over the canonical manifest
/// bytes with this field excluded, plus every member fingerprint in
/// directory order (DESIGN sections 1.1 and 4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArtifactFingerprint(Digest256);

impl ArtifactFingerprint {
    pub fn from_digest(digest: Digest256) -> Self {
        ArtifactFingerprint(digest)
    }

    pub fn as_digest(&self) -> &Digest256 {
        &self.0
    }
}

impl ManifestCore {
    /// Serializes the manifest, optionally excluding field 18
    /// (`artifact_fingerprint`) for self-reference-free hashing.
    pub fn canonical_cbor(&self, include_artifact_fingerprint: bool) -> Vec<u8> {
        let entries = if include_artifact_fingerprint { 20 } else { 19 };
        let mut writer = CborWriter::new();
        writer.map(entries);
        writer.field(1).text(MANIFEST_MAGIC);
        writer.field(2).unsigned(self.container_version as u64);
        writer.field(3).unsigned(self.hir_wire_schema as u64);
        writer.field(4).unsigned(self.mir_wire_schema as u64);
        writer.field(5).unsigned(self.lir_wire_schema as u64);
        writer.field(6).text(&self.producer_compiler_version);
        writer.field(7).unsigned(self.language_abi as u64);
        writer.field(8).bytes(self.runtime_abi.as_bytes());
        writer
            .field(9)
            .unsigned(self.identity_schema_version as u64);
        writer.field(10).text(self.coordinate.group());
        writer.field(11).text(self.coordinate.name());
        writer.field(12).text(self.coordinate.version().as_str());
        writer.field(13).bytes(self.cone_identity.as_bytes());
        writer.field(14).unsigned(match self.kind {
            ConeKind::Library => 1,
            ConeKind::Executable => 2,
        });
        writer.field(15).array(self.dependencies.len() as u64);
        for dependency in &self.dependencies {
            writer.map(7);
            writer.field(1).bytes(dependency.cone_identity.as_bytes());
            writer
                .field(2)
                .bytes(dependency.hir_semantic_fingerprint.as_bytes());
            writer
                .field(3)
                .bytes(dependency.mir_semantic_fingerprint.as_bytes());
            writer
                .field(4)
                .bytes(dependency.lir_semantic_fingerprint.as_bytes());
            writer.field(5).text(dependency.coordinate.group());
            writer.field(6).text(dependency.coordinate.name());
            writer
                .field(7)
                .text(dependency.coordinate.version().as_str());
        }
        write_capability_field(&mut writer, 16, self.target_profile.as_capability());
        writer
            .field(17)
            .bytes(self.target_profile_fingerprint.as_bytes());
        writer
            .field(18)
            .bytes(self.backend_profile_fingerprint.as_bytes());
        writer.field(19);
        writer.array(self.members.len() as u64);
        for member in &self.members {
            member.write(&mut writer);
        }
        if include_artifact_fingerprint {
            writer
                .field(20)
                .bytes(self.artifact_fingerprint.as_digest().as_bytes());
        }
        writer.into_bytes()
    }

    /// Computes the whole-artifact fingerprint over the fingerprint-free
    /// manifest bytes plus the per-member content fingerprints.
    pub fn compute_artifact_fingerprint(&self) -> ArtifactFingerprint {
        let mut hasher =
            DomainHasher::new(ARTIFACT_FINGERPRINT_DOMAIN).field(&self.canonical_cbor(false));
        for member in &self.members {
            hasher = hasher.field(member.member_fingerprint().as_bytes());
        }
        ArtifactFingerprint::from_digest(hasher.finish())
    }

    pub fn decode(data: &[u8]) -> Result<Self, ManifestError> {
        let mut reader = CborReader::new(data, crate::limits::CBOR_NESTING_LIMIT);
        let core = Self::decode_with(&mut reader)?;
        reader.finish().map_err(ManifestError::Cbor)?;
        Ok(core)
    }

    fn decode_with(reader: &mut CborReader<'_>) -> Result<Self, ManifestError> {
        let mut record = reader.map()?;
        let entries = record.remaining_entries();
        let has_fingerprint = entries == 20;
        if entries != 19 && entries != 20 {
            return Err(ManifestError::BadShape);
        }
        expect_key(&mut record, 1)?;
        if record.text()? != MANIFEST_MAGIC {
            return Err(ManifestError::BadMagic);
        }
        expect_key(&mut record, 2)?;
        let container_version = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 3)?;
        let hir_wire_schema = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 4)?;
        let mir_wire_schema = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 5)?;
        let lir_wire_schema = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 6)?;
        let producer_compiler_version = record.text()?.to_owned();
        expect_key(&mut record, 7)?;
        let language_abi = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 8)?;
        let runtime_abi = Digest256::from_bytes(read_digest(&mut record)?);
        expect_key(&mut record, 9)?;
        let identity_schema_version = read_u32(record.unsigned()?)?;
        expect_key(&mut record, 10)?;
        let group = record.text()?.to_owned();
        expect_key(&mut record, 11)?;
        let name = record.text()?.to_owned();
        expect_key(&mut record, 12)?;
        let version = record.text()?.to_owned();
        expect_key(&mut record, 13)?;
        let cone_identity = ConeIdentity::from_bytes(&read_digest(&mut record)?);
        expect_key(&mut record, 14)?;
        let kind = match record.unsigned()? {
            1 => ConeKind::Library,
            2 => ConeKind::Executable,
            _ => return Err(ManifestError::BadShape),
        };
        let coordinate =
            ConeCoordinate::new(&group, &name, &version).map_err(ManifestError::Coordinate)?;
        if cone_identity != ConeIdentity::of(&coordinate) {
            return Err(ManifestError::IdentityMismatch);
        }
        expect_key(&mut record, 15)?;
        let mut items = record.array()?;
        let mut dependencies = Vec::with_capacity(items.count() as usize);
        for _ in 0..items.count() {
            let mut entry = items.map()?;
            let entry_entries = entry.remaining_entries();
            if entry_entries != 7 {
                return Err(ManifestError::BadShape);
            }
            expect_key(&mut entry, 1)?;
            let cone_identity = ConeIdentity::from_bytes(&read_digest(&mut entry)?);
            expect_key(&mut entry, 2)?;
            let hir = Digest256::from_bytes(read_digest(&mut entry)?);
            expect_key(&mut entry, 3)?;
            let mir = Digest256::from_bytes(read_digest(&mut entry)?);
            expect_key(&mut entry, 4)?;
            let lir = Digest256::from_bytes(read_digest(&mut entry)?);
            expect_key(&mut entry, 5)?;
            let group = entry.text()?.to_owned();
            expect_key(&mut entry, 6)?;
            let name = entry.text()?.to_owned();
            expect_key(&mut entry, 7)?;
            let version = entry.text()?.to_owned();
            if entry.next_key()?.is_some() {
                return Err(ManifestError::BadShape);
            }
            let coordinate =
                ConeCoordinate::new(&group, &name, &version).map_err(ManifestError::Coordinate)?;
            if cone_identity != ConeIdentity::of(&coordinate) {
                return Err(ManifestError::DependencyIdentityMismatch);
            }
            dependencies.push(DependencyRecord {
                coordinate,
                cone_identity,
                hir_semantic_fingerprint: hir,
                mir_semantic_fingerprint: mir,
                lir_semantic_fingerprint: lir,
            });
        }
        drop(items);
        expect_key(&mut record, 16)?;
        let target_profile = TargetProfileWireId::new(read_capability(&mut record)?);
        expect_key(&mut record, 17)?;
        let target_profile_fingerprint = Digest256::from_bytes(read_digest(&mut record)?);
        expect_key(&mut record, 18)?;
        let backend_profile_fingerprint = Digest256::from_bytes(read_digest(&mut record)?);
        expect_key(&mut record, 19)?;
        let mut member_items = record.array()?;
        let mut members = Vec::with_capacity(member_items.count() as usize);
        for _ in 0..member_items.count() {
            members.push(SlibMemberRecord::decode(&mut member_items)?);
        }
        drop(member_items);
        let artifact_fingerprint = if has_fingerprint {
            expect_key(&mut record, 20)?;
            ArtifactFingerprint::from_digest(Digest256::from_bytes(read_digest(&mut record)?))
        } else {
            return Err(ManifestError::MissingArtifactFingerprint);
        };
        if record.next_key()?.is_some() {
            return Err(ManifestError::BadShape);
        }
        let core = ManifestCore {
            container_version,
            hir_wire_schema,
            mir_wire_schema,
            lir_wire_schema,
            producer_compiler_version,
            language_abi,
            runtime_abi,
            identity_schema_version,
            coordinate,
            cone_identity,
            kind,
            dependencies,
            target_profile,
            target_profile_fingerprint,
            backend_profile_fingerprint,
            members,
            artifact_fingerprint,
        };
        core.validate_directory()?;
        Ok(core)
    }

    /// Directory-level invariants: strictly increasing ids, ids match
    /// their stable keys, and exactly one of each metadata role.
    pub fn validate_directory(&self) -> Result<(), ManifestError> {
        let mut previous: Option<&SlibMemberId> = None;
        let mut metadata_counts = [0usize; 3];
        for member in &self.members {
            if let Some(previous) = previous {
                if member.id <= *previous {
                    return Err(ManifestError::MemberIdOrder);
                }
            }
            previous = Some(&member.id);
            let expected = SlibMemberId::of(self.cone_identity, &member.stable_key);
            if member.id != expected {
                return Err(ManifestError::MemberIdMismatch);
            }
            match member.stable_key {
                crate::member::MemberStableKey::HirMetadata => metadata_counts[0] += 1,
                crate::member::MemberStableKey::MirMetadata => metadata_counts[1] += 1,
                crate::member::MemberStableKey::LirMetadata => metadata_counts[2] += 1,
                _ => {}
            }
        }
        if metadata_counts != [1, 1, 1] {
            return Err(ManifestError::MetadataRoleCount);
        }
        Ok(())
    }
}

fn write_capability_field(writer: &mut CborWriter, key: u64, capability: &CapabilityId) {
    writer.field(key);
    writer.map(3);
    writer.field(1).text(capability.namespace());
    writer.field(2).text(capability.name());
    writer.field(3).unsigned(capability.major_version() as u64);
}

fn read_capability(reader: &mut CborReader<'_>) -> Result<CapabilityId, ManifestError> {
    let mut record = reader.map()?;
    let entries = record.remaining_entries();
    if entries != 3 {
        return Err(ManifestError::BadShape);
    }
    expect_key(&mut record, 1)?;
    let namespace = record.text()?.to_owned();
    expect_key(&mut record, 2)?;
    let name = record.text()?.to_owned();
    expect_key(&mut record, 3)?;
    let major = u32::try_from(record.unsigned()?).map_err(|_| ManifestError::BadShape)?;
    if record.next_key()?.is_some() {
        return Err(ManifestError::BadShape);
    }
    CapabilityId::new(&namespace, &name, major).map_err(ManifestError::Capability)
}

fn read_digest(reader: &mut CborReader<'_>) -> Result<[u8; 32], ManifestError> {
    let bytes = reader.bytes()?;
    if bytes.len() != 32 {
        return Err(ManifestError::DigestLength(bytes.len()));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(bytes);
    Ok(raw)
}

fn read_u32(value: u64) -> Result<u32, ManifestError> {
    u32::try_from(value).map_err(|_| ManifestError::BadShape)
}

fn expect_key(
    record: &mut scoop_identity::MapGuard<'_, '_>,
    expected: u64,
) -> Result<(), ManifestError> {
    match record.next_key()? {
        Some(key) if key == expected => Ok(()),
        _ => Err(ManifestError::BadShape),
    }
}

/// Manifest-level failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    BadMagic,
    BadShape,
    MissingArtifactFingerprint,
    IdentityMismatch,
    MemberIdOrder,
    MemberIdMismatch,
    MetadataRoleCount,
    DependencyIdentityMismatch,
    Coordinate(scoop_identity::CoordinateError),
    Capability(scoop_identity::CapabilityError),
    Member(crate::member::MemberError),
    DigestLength(usize),
    Cbor(scoop_identity::CborError),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::BadMagic => write!(f, "manifest magic is not SCOOPSLIB"),
            ManifestError::BadShape => write!(f, "manifest record shape is invalid"),
            ManifestError::MissingArtifactFingerprint => {
                write!(f, "manifest lacks the artifact fingerprint")
            }
            ManifestError::IdentityMismatch => {
                write!(f, "cone identity does not match the coordinate")
            }
            ManifestError::MemberIdOrder => {
                write!(f, "member directory ids are not strictly increasing")
            }
            ManifestError::MemberIdMismatch => {
                write!(f, "member id does not match its stable key")
            }
            ManifestError::MetadataRoleCount => write!(
                f,
                "member directory must contain exactly one HIR, MIR and LIR metadata member"
            ),
            ManifestError::DependencyIdentityMismatch => write!(
                f,
                "dependency record coordinate does not hash to its recorded identity"
            ),
            ManifestError::Coordinate(error) => write!(f, "invalid coordinate: {error}"),
            ManifestError::Capability(error) => write!(f, "invalid capability: {error}"),
            ManifestError::Member(error) => write!(f, "member error: {error}"),
            ManifestError::DigestLength(len) => {
                write!(f, "digest field of {len} bytes, expected 32")
            }
            ManifestError::Cbor(error) => write!(f, "canonical CBOR error: {error}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<crate::member::MemberError> for ManifestError {
    fn from(error: crate::member::MemberError) -> Self {
        ManifestError::Member(error)
    }
}

impl From<scoop_identity::CborError> for ManifestError {
    fn from(error: scoop_identity::CborError) -> Self {
        ManifestError::Cbor(error)
    }
}
