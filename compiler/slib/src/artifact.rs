//! `.slib` envelope: deterministic builder, raw decode, and the
//! purpose-typed validation views (DESIGN sections 4.1-4.2).

use core::fmt;
use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity, Digest256, TargetProfileWireId};
use scoop_manifest::ConeKind;
use sha2::{Digest as _, Sha256};

use crate::archive::{ArchiveError, read_archive, write_archive};
use crate::limits::SlibDecodeLimits;
use crate::manifest::{
    ArtifactFingerprint, DependencyRecord, MANIFEST_MEMBER_NAME, ManifestCore, ManifestError,
};
use crate::member::{
    LogicalKey, MemberError, MemberStableKey, SlibMemberId, SlibMemberRecord, SlibMemberRole,
};

/// Envelope-level failure. Errors never leak host paths; the artifact
/// coordinate is carried for attribution by callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlibError {
    Archive(ArchiveError),
    Member(MemberError),
    Manifest(ManifestError),
    Limits(&'static str),
    /// The bootstrap member is missing or duplicated.
    BootstrapMember(&'static str),
    /// A directory member has no physical payload or vice versa.
    UndeclaredMember {
        name: String,
    },
    MissingMember {
        id: SlibMemberId,
    },
    PayloadLengthMismatch {
        id: SlibMemberId,
        declared: u64,
        actual: u64,
    },
    PayloadHashMismatch {
        id: SlibMemberId,
    },
    FingerprintMismatch,
    MemberNameOrdinal {
        expected: String,
        actual: String,
    },
}

impl fmt::Display for SlibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlibError::Archive(error) => write!(f, "archive error: {error}"),
            SlibError::Member(error) => write!(f, "member error: {error}"),
            SlibError::Manifest(error) => write!(f, "manifest error: {error}"),
            SlibError::Limits(field) => write!(f, "decode budget exceeded: {field}"),
            SlibError::BootstrapMember(detail) => {
                write!(f, "bootstrap member manifest.cbor: {detail}")
            }
            SlibError::UndeclaredMember { name } => {
                write!(
                    f,
                    "physical member {name:?} is not declared in the directory"
                )
            }
            SlibError::MissingMember { id } => {
                write!(f, "declared member {id} has no physical payload")
            }
            SlibError::PayloadLengthMismatch {
                id,
                declared,
                actual,
            } => write!(
                f,
                "member {id} declares {declared} bytes but the payload has {actual}"
            ),
            SlibError::PayloadHashMismatch { id } => {
                write!(f, "member {id} payload does not match its declared digest")
            }
            SlibError::FingerprintMismatch => {
                write!(
                    f,
                    "artifact fingerprint does not match the recomputed value"
                )
            }
            SlibError::MemberNameOrdinal { expected, actual } => write!(
                f,
                "physical member name {actual:?} does not match the directory ordinal ({expected:?})"
            ),
        }
    }
}

impl std::error::Error for SlibError {}

impl From<ArchiveError> for SlibError {
    fn from(error: ArchiveError) -> Self {
        SlibError::Archive(error)
    }
}

impl From<MemberError> for SlibError {
    fn from(error: MemberError) -> Self {
        SlibError::Member(error)
    }
}

impl From<ManifestError> for SlibError {
    fn from(error: ManifestError) -> Self {
        SlibError::Manifest(error)
    }
}

/// Deterministic `.slib` writer. Members are emitted sorted by
/// `SlibMemberId`; identical inputs produce byte-identical archives.
pub struct SlibBuilder {
    core: ManifestCoreTemplate,
    members: BTreeMap<SlibMemberId, (SlibMemberRecord, Vec<u8>)>,
}

/// The manifest fields supplied by the packager; the member directory
/// and artifact fingerprint are derived by the builder.
#[derive(Debug, Clone)]
pub struct ManifestCoreTemplate {
    pub container_version: u32,
    pub hir_wire_schema: u32,
    pub mir_wire_schema: u32,
    pub lir_wire_schema: u32,
    pub producer_compiler_version: String,
    pub language_abi: u32,
    pub runtime_abi: Digest256,
    pub identity_schema_version: u32,
    pub coordinate: ConeCoordinate,
    pub kind: ConeKind,
    pub dependencies: Vec<DependencyRecord>,
    pub target_profile: TargetProfileWireId,
    pub target_profile_fingerprint: Digest256,
    pub backend_profile_fingerprint: Digest256,
}

impl SlibBuilder {
    pub fn new(core: ManifestCoreTemplate) -> Self {
        SlibBuilder {
            core,
            members: BTreeMap::new(),
        }
    }

    /// Adds one member; the payload's length and digest are fixed into
    /// the directory record.
    pub fn add_member(
        &mut self,
        stable_key: MemberStableKey,
        role: SlibMemberRole,
        payload: Vec<u8>,
    ) -> Result<SlibMemberId, SlibError> {
        let cone_identity = ConeIdentity::of(&self.core.coordinate);
        let id = SlibMemberId::of(cone_identity, &stable_key);
        let sha256 = {
            let mut raw = [0u8; 32];
            raw.copy_from_slice(&Sha256::digest(&payload));
            Digest256::from_bytes(raw)
        };
        let record = SlibMemberRecord::new(id, stable_key, role, payload.len() as u64, sha256)?;
        if self.members.insert(id, (record, payload)).is_some() {
            return Err(SlibError::Limits("duplicate member id"));
        }
        Ok(id)
    }

    /// Serializes the canonical archive. The manifest (including its
    /// computed artifact fingerprint) is the bootstrap member.
    pub fn finish(self) -> Result<Vec<u8>, SlibError> {
        let cone_identity = ConeIdentity::of(&self.core.coordinate);
        let members: Vec<SlibMemberRecord> = self
            .members
            .values()
            .map(|(record, _)| record.clone())
            .collect();
        let mut core = ManifestCore {
            container_version: self.core.container_version,
            hir_wire_schema: self.core.hir_wire_schema,
            mir_wire_schema: self.core.mir_wire_schema,
            lir_wire_schema: self.core.lir_wire_schema,
            producer_compiler_version: self.core.producer_compiler_version,
            language_abi: self.core.language_abi,
            runtime_abi: self.core.runtime_abi,
            identity_schema_version: self.core.identity_schema_version,
            coordinate: self.core.coordinate,
            cone_identity,
            kind: self.core.kind,
            dependencies: self.core.dependencies,
            target_profile: self.core.target_profile,
            target_profile_fingerprint: self.core.target_profile_fingerprint,
            backend_profile_fingerprint: self.core.backend_profile_fingerprint,
            members,
            artifact_fingerprint: ArtifactFingerprint::from_digest(Digest256::ZERO),
        };
        core.artifact_fingerprint = core.compute_artifact_fingerprint();
        core.validate_directory()?;

        let manifest_bytes = core.canonical_cbor(true);
        let mut physical: Vec<(String, Vec<u8>)> = Vec::with_capacity(self.members.len() + 1);
        physical.push((MANIFEST_MEMBER_NAME.to_owned(), manifest_bytes));
        for (ordinal, (_record, payload)) in self.members.values().enumerate() {
            physical.push((ordinal_name(ordinal), payload.clone()));
        }
        let referenced: Vec<(&str, &[u8])> = physical
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        Ok(write_archive(&referenced))
    }
}

/// Directory ordinal → physical name: `m` plus eight zero-padded digits.
pub fn ordinal_name(ordinal: usize) -> String {
    format!("m{ordinal:08}")
}

/// A raw-decoded envelope: canonical container plus hash-verified
/// payloads. Grants no semantic API; stronger views are constructed on
/// top of it.
pub struct DecodedSlibEnvelope {
    manifest: ManifestCore,
    payloads: BTreeMap<SlibMemberId, Vec<u8>>,
}

impl DecodedSlibEnvelope {
    /// Decodes and validates the container/manifest/payload envelope of
    /// a whole `.slib`.
    pub fn decode(data: &[u8], limits: &SlibDecodeLimits) -> Result<Self, SlibError> {
        if data.len() as u64 > limits.archive_max_bytes {
            return Err(SlibError::Limits("archive_max_bytes"));
        }
        let members = read_archive(data)?;
        let mut manifest_bytes: Option<&[u8]> = None;
        let mut physicals: Vec<(String, &[u8])> = Vec::with_capacity(members.len());
        for member in &members {
            if member.name == MANIFEST_MEMBER_NAME {
                if manifest_bytes.is_some() {
                    return Err(SlibError::BootstrapMember("duplicated"));
                }
                manifest_bytes = Some(member.payload);
            } else {
                physicals.push((member.name.clone(), member.payload));
            }
        }
        let manifest_bytes = manifest_bytes.ok_or(SlibError::BootstrapMember("missing"))?;
        if manifest_bytes.len() as u64 > limits.manifest_max_bytes {
            return Err(SlibError::Limits("manifest_max_bytes"));
        }
        let manifest = ManifestCore::decode(manifest_bytes)?;
        if manifest.members.len() as u64 > limits.members_max_count {
            return Err(SlibError::Limits("members_max_count"));
        }
        if physicals.len() != manifest.members.len() {
            return Err(SlibError::Limits(
                "physical member count differs from the directory",
            ));
        }
        // Physical order must follow directory order with derived names.
        let mut payloads = BTreeMap::new();
        for (index, record) in manifest.members.iter().enumerate() {
            let (name, payload) = &physicals[index];
            let expected = ordinal_name(index);
            if *name != expected {
                return Err(SlibError::MemberNameOrdinal {
                    expected,
                    actual: name.clone(),
                });
            }
            let per_member_limit = match record.role {
                SlibMemberRole::HirMetadata { .. }
                | SlibMemberRole::MirMetadata { .. }
                | SlibMemberRole::LirMetadata { .. } => limits.metadata_section_max_bytes,
                SlibMemberRole::LinkObject { .. } => limits.link_member_max_bytes,
                SlibMemberRole::DiagnosticAttachment { .. } => limits.diagnostic_member_max_bytes,
                SlibMemberRole::ExtensionBlob { .. } => limits.link_member_max_bytes,
            };
            if record.byte_length > per_member_limit {
                return Err(SlibError::Limits(
                    "member byte_length exceeds its role budget",
                ));
            }
            if payload.len() as u64 != record.byte_length {
                return Err(SlibError::PayloadLengthMismatch {
                    id: record.id,
                    declared: record.byte_length,
                    actual: payload.len() as u64,
                });
            }
            let mut raw = [0u8; 32];
            raw.copy_from_slice(&Sha256::digest(payload));
            if Digest256::from_bytes(raw) != record.sha256 {
                return Err(SlibError::PayloadHashMismatch { id: record.id });
            }
            payloads.insert(record.id, payload.to_vec());
        }
        let envelope = DecodedSlibEnvelope { manifest, payloads };
        envelope.verify_artifact_fingerprint()?;
        Ok(envelope)
    }

    fn verify_artifact_fingerprint(&self) -> Result<(), SlibError> {
        if self.manifest.compute_artifact_fingerprint() != self.manifest.artifact_fingerprint {
            return Err(SlibError::FingerprintMismatch);
        }
        Ok(())
    }

    pub fn manifest(&self) -> &ManifestCore {
        &self.manifest
    }

    /// Raw payload bytes of one member; access is envelope-level only.
    pub fn member_payload(&self, id: &SlibMemberId) -> Option<&[u8]> {
        self.payloads.get(id).map(Vec::as_slice)
    }
}

/// Purpose markers selecting the validation tier (DESIGN section 4.1).
pub mod purpose {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Graph;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Compile;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Link;
}

/// The Graph view: identity, dependency/kind/target compatibility and
/// the complete member envelope. It grants no metadata or link APIs.
pub struct ValidatedGraphArtifact {
    envelope: DecodedSlibEnvelope,
}

impl fmt::Debug for ValidatedGraphArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedGraphArtifact")
            .field("coordinate", &self.envelope.manifest().coordinate)
            .field("members", &self.envelope.manifest().members.len())
            .finish()
    }
}

impl ValidatedGraphArtifact {
    /// Constructs the Graph view over a fully decoded envelope.
    pub fn new(envelope: DecodedSlibEnvelope) -> Self {
        ValidatedGraphArtifact { envelope }
    }

    pub fn coordinate(&self) -> &ConeCoordinate {
        &self.envelope.manifest().coordinate
    }

    pub fn cone_identity(&self) -> ConeIdentity {
        self.envelope.manifest().cone_identity
    }

    pub fn kind(&self) -> ConeKind {
        self.envelope.manifest().kind
    }

    pub fn dependencies(&self) -> &[DependencyRecord] {
        &self.envelope.manifest().dependencies
    }

    pub fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.envelope.manifest().artifact_fingerprint
    }

    pub fn members(&self) -> &[SlibMemberRecord] {
        &self.envelope.manifest().members
    }

    pub fn manifest(&self) -> &ManifestCore {
        self.envelope.manifest()
    }

    /// Decodes one artifact and constructs its Graph view.
    pub fn read(data: &[u8], limits: &SlibDecodeLimits) -> Result<Self, SlibError> {
        let envelope = DecodedSlibEnvelope::decode(data, limits)?;
        Ok(ValidatedGraphArtifact { envelope })
    }
}

/// Convenience constructor for link-object logical keys in tests and
/// producers until the unit-set key scheme lands with codegen
/// collection work.
pub fn plain_logical_key(text: &str) -> Result<LogicalKey, MemberError> {
    LogicalKey::new(text.as_bytes().to_vec())
}
