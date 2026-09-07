//! Typed member directory: stable keys, roles, purpose sets and records
//! (DESIGN section 4.1).

use core::fmt;

use scoop_identity::{
    CapabilityError, CapabilityId, CborReader, CborWriter, ConeIdentity, Digest256, DomainHasher,
    ObjectFormatId, TargetProfileWireId,
};

pub const SLIB_MEMBER_ID_DOMAIN: &[u8] = b"scoop-slib-member-v1";
pub const MEMBER_FINGERPRINT_DOMAIN: &[u8] = b"scoop-slib-member-content-v1";
pub const LINK_MEMBER_FINGERPRINT_DOMAIN: &[u8] = b"scoop-slib-link-member-v1";

/// Upper bound for one member's `logical_key` (DESIGN section 4.1).
pub const LOGICAL_KEY_MAX_BYTES: usize = 4096;

/// Member identity: `SHA-256("scoop-slib-member-v1" || ConeIdentity ||
/// canonical(MemberStableKey))`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlibMemberId(Digest256);

impl SlibMemberId {
    pub fn of(cone: ConeIdentity, stable_key: &MemberStableKey) -> Self {
        let digest = DomainHasher::new(SLIB_MEMBER_ID_DOMAIN)
            .field(cone.as_bytes())
            .field(&stable_key.canonical_cbor())
            .finish();
        SlibMemberId(digest)
    }

    pub fn from_validated(bytes: [u8; 32]) -> Self {
        SlibMemberId(Digest256::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for SlibMemberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SlibMemberId({})", self.0)
    }
}

impl fmt::Display for SlibMemberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Capability-defined canonical shard/translation-unit/bridge/owner key.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LogicalKey(Vec<u8>);

impl LogicalKey {
    pub fn new(bytes: Vec<u8>) -> Result<Self, MemberError> {
        if bytes.is_empty() || bytes.len() > LOGICAL_KEY_MAX_BYTES {
            return Err(MemberError::LogicalKeyLength(bytes.len()));
        }
        Ok(LogicalKey(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for LogicalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LogicalKey({} bytes)", self.0.len())
    }
}

/// The stable, content-independent identity of one member.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemberStableKey {
    HirMetadata,
    MirMetadata,
    LirMetadata,
    LinkObject {
        verifier_capability: CapabilityId,
        logical_key: LogicalKey,
    },
    DiagnosticAttachment {
        capability: CapabilityId,
        logical_key: LogicalKey,
    },
    ExtensionBlob {
        capability: CapabilityId,
        logical_key: LogicalKey,
    },
}

impl MemberStableKey {
    fn variant_tag(&self) -> u64 {
        match self {
            MemberStableKey::HirMetadata => 1,
            MemberStableKey::MirMetadata => 2,
            MemberStableKey::LirMetadata => 3,
            MemberStableKey::LinkObject { .. } => 4,
            MemberStableKey::DiagnosticAttachment { .. } => 5,
            MemberStableKey::ExtensionBlob { .. } => 6,
        }
    }

    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        self.write(&mut writer);
        writer.into_bytes()
    }

    /// Writes the stable key as an inline record value.
    pub fn write(&self, writer: &mut CborWriter) {
        match self {
            MemberStableKey::HirMetadata
            | MemberStableKey::MirMetadata
            | MemberStableKey::LirMetadata => {
                writer.map(1);
                writer.field(0).unsigned(self.variant_tag());
            }
            MemberStableKey::LinkObject {
                verifier_capability,
                logical_key,
            }
            | MemberStableKey::DiagnosticAttachment {
                capability: verifier_capability,
                logical_key,
            }
            | MemberStableKey::ExtensionBlob {
                capability: verifier_capability,
                logical_key,
            } => {
                writer.map(3);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1);
                write_capability(writer, verifier_capability);
                writer.field(2).bytes(logical_key.as_bytes());
            }
        }
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, MemberError> {
        let mut record = reader.map()?;
        let entries = record.remaining_entries();
        if entries != 1 && entries != 3 {
            return Err(MemberError::BadStableKeyShape);
        }
        expect_key(&mut record, 0, || MemberError::BadStableKeyShape)?;
        let tag = record.unsigned()?;
        match (tag, entries) {
            (1, 1) => Ok(MemberStableKey::HirMetadata),
            (2, 1) => Ok(MemberStableKey::MirMetadata),
            (3, 1) => Ok(MemberStableKey::LirMetadata),
            (4..=6, 3) => {
                expect_key(&mut record, 1, || MemberError::BadStableKeyShape)?;
                let capability = read_capability(&mut record)?;
                expect_key(&mut record, 2, || MemberError::BadStableKeyShape)?;
                let key = LogicalKey::new(record.bytes()?.to_vec())?;
                if record.next_key()?.is_some() {
                    return Err(MemberError::BadStableKeyShape);
                }
                match tag {
                    4 => Ok(MemberStableKey::LinkObject {
                        verifier_capability: capability,
                        logical_key: key,
                    }),
                    5 => Ok(MemberStableKey::DiagnosticAttachment {
                        capability,
                        logical_key: key,
                    }),
                    _ => Ok(MemberStableKey::ExtensionBlob {
                        capability,
                        logical_key: key,
                    }),
                }
            }
            _ => Err(MemberError::BadStableKeyShape),
        }
    }
}

/// Purpose bits (DESIGN section 4.1).
pub struct MemberPurposeSet(pub u32);

impl MemberPurposeSet {
    pub const GRAPH: u32 = 0x0000_0001;
    pub const COMPILE: u32 = 0x0000_0002;
    pub const LINK: u32 = 0x0000_0004;
    pub const DIAGNOSTICS: u32 = 0x0000_0008;
}

/// The role a member plays in the artifact, carrying its capability
/// contract on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SlibMemberRole {
    HirMetadata {
        wire_schema: u32,
    },
    MirMetadata {
        wire_schema: u32,
    },
    LirMetadata {
        wire_schema: u32,
    },
    LinkObject {
        target_profile: TargetProfileWireId,
        object_format: ObjectFormatId,
        verifier_capability: CapabilityId,
    },
    DiagnosticAttachment {
        capability: CapabilityId,
    },
    ExtensionBlob {
        capability: CapabilityId,
        required_for: u32,
    },
}

impl SlibMemberRole {
    /// The implicit purpose set of this role.
    pub fn purpose(&self) -> u32 {
        match self {
            SlibMemberRole::HirMetadata { .. } | SlibMemberRole::MirMetadata { .. } => {
                MemberPurposeSet::COMPILE
            }
            SlibMemberRole::LirMetadata { .. } => {
                MemberPurposeSet::COMPILE | MemberPurposeSet::LINK
            }
            SlibMemberRole::LinkObject { .. } => MemberPurposeSet::LINK,
            SlibMemberRole::DiagnosticAttachment { .. } => MemberPurposeSet::DIAGNOSTICS,
            SlibMemberRole::ExtensionBlob { required_for, .. } => *required_for,
        }
    }

    pub fn is_link_object(&self) -> bool {
        matches!(self, SlibMemberRole::LinkObject { .. })
    }

    /// Members whose bytes participate in the link fingerprint
    /// (`LinkObject` or a Link-required `ExtensionBlob`).
    pub fn participates_in_link(&self) -> bool {
        match self {
            SlibMemberRole::LinkObject { .. } => true,
            SlibMemberRole::ExtensionBlob { required_for, .. } => {
                *required_for & MemberPurposeSet::LINK != 0
            }
            _ => false,
        }
    }

    pub fn write(&self, writer: &mut CborWriter) {
        match self {
            SlibMemberRole::HirMetadata { wire_schema } => {
                writer.map(2);
                writer.field(0).unsigned(1);
                writer.field(1).unsigned(*wire_schema as u64);
            }
            SlibMemberRole::MirMetadata { wire_schema } => {
                writer.map(2);
                writer.field(0).unsigned(2);
                writer.field(1).unsigned(*wire_schema as u64);
            }
            SlibMemberRole::LirMetadata { wire_schema } => {
                writer.map(2);
                writer.field(0).unsigned(3);
                writer.field(1).unsigned(*wire_schema as u64);
            }
            SlibMemberRole::LinkObject {
                target_profile,
                object_format,
                verifier_capability,
            } => {
                writer.map(4);
                writer.field(0).unsigned(4);
                writer.field(1);
                write_capability(writer, target_profile.as_capability());
                writer.field(2);
                write_capability(writer, object_format.as_capability());
                writer.field(3);
                write_capability(writer, verifier_capability);
            }
            SlibMemberRole::DiagnosticAttachment { capability } => {
                writer.map(2);
                writer.field(0).unsigned(5);
                writer.field(1);
                write_capability(writer, capability);
            }
            SlibMemberRole::ExtensionBlob {
                capability,
                required_for,
            } => {
                writer.map(3);
                writer.field(0).unsigned(6);
                writer.field(1);
                write_capability(writer, capability);
                writer.field(2).unsigned(*required_for as u64);
            }
        }
    }

    fn decode(reader: &mut CborReader<'_>) -> Result<Self, MemberError> {
        let mut record = reader.map()?;
        let entries = record.remaining_entries();
        expect_key(&mut record, 0, || MemberError::BadRoleShape)?;
        let tag = record.unsigned()?;
        let role = match tag {
            1..=3 => {
                if entries != 2 {
                    return Err(MemberError::BadRoleShape);
                }
                expect_key(&mut record, 1, || MemberError::BadRoleShape)?;
                let wire_schema = read_u32(record.unsigned()?)?;
                match tag {
                    1 => SlibMemberRole::HirMetadata { wire_schema },
                    2 => SlibMemberRole::MirMetadata { wire_schema },
                    _ => SlibMemberRole::LirMetadata { wire_schema },
                }
            }
            4 => {
                if entries != 4 {
                    return Err(MemberError::BadRoleShape);
                }
                expect_key(&mut record, 1, || MemberError::BadRoleShape)?;
                let target_profile = TargetProfileWireId::new(read_capability(&mut record)?);
                expect_key(&mut record, 2, || MemberError::BadRoleShape)?;
                let object_format = ObjectFormatId::new(read_capability(&mut record)?);
                expect_key(&mut record, 3, || MemberError::BadRoleShape)?;
                let verifier_capability = read_capability(&mut record)?;
                SlibMemberRole::LinkObject {
                    target_profile,
                    object_format,
                    verifier_capability,
                }
            }
            5 => {
                if entries != 2 {
                    return Err(MemberError::BadRoleShape);
                }
                expect_key(&mut record, 1, || MemberError::BadRoleShape)?;
                SlibMemberRole::DiagnosticAttachment {
                    capability: read_capability(&mut record)?,
                }
            }
            6 => {
                if entries != 3 {
                    return Err(MemberError::BadRoleShape);
                }
                expect_key(&mut record, 1, || MemberError::BadRoleShape)?;
                let capability = read_capability(&mut record)?;
                expect_key(&mut record, 2, || MemberError::BadRoleShape)?;
                let required_for = read_u32(record.unsigned()?)?;
                SlibMemberRole::ExtensionBlob {
                    capability,
                    required_for,
                }
            }
            _ => return Err(MemberError::BadRoleShape),
        };
        if record.next_key()?.is_some() {
            return Err(MemberError::BadRoleShape);
        }
        Ok(role)
    }
}

fn read_u32(value: u64) -> Result<u32, MemberError> {
    u32::try_from(value).map_err(|_| MemberError::FieldOverflow(value))
}

/// One directory record; CBOR field keys are fixed to `1..=5`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlibMemberRecord {
    pub id: SlibMemberId,
    pub stable_key: MemberStableKey,
    pub role: SlibMemberRole,
    pub byte_length: u64,
    pub sha256: Digest256,
}

impl SlibMemberRecord {
    pub fn new(
        id: SlibMemberId,
        stable_key: MemberStableKey,
        role: SlibMemberRole,
        byte_length: u64,
        sha256: Digest256,
    ) -> Result<Self, MemberError> {
        let record = SlibMemberRecord {
            id,
            stable_key,
            role,
            byte_length,
            sha256,
        };
        record.validate()?;
        Ok(record)
    }

    /// Role/stable-key pairing rules from DESIGN 4.1.
    pub fn validate(&self) -> Result<(), MemberError> {
        match (&self.stable_key, &self.role) {
            (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata { .. })
            | (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata { .. })
            | (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata { .. }) => {}
            (
                MemberStableKey::LinkObject {
                    verifier_capability,
                    ..
                },
                SlibMemberRole::LinkObject {
                    verifier_capability: role_capability,
                    ..
                },
            ) => {
                if verifier_capability != role_capability {
                    return Err(MemberError::CapabilityMismatch);
                }
            }
            (
                MemberStableKey::DiagnosticAttachment {
                    capability: key_capability,
                    ..
                },
                SlibMemberRole::DiagnosticAttachment {
                    capability: role_capability,
                },
            ) => {
                if key_capability != role_capability {
                    return Err(MemberError::CapabilityMismatch);
                }
            }
            (
                MemberStableKey::ExtensionBlob {
                    capability: key_capability,
                    ..
                },
                SlibMemberRole::ExtensionBlob {
                    capability: role_capability,
                    required_for,
                },
            ) => {
                if key_capability != role_capability {
                    return Err(MemberError::CapabilityMismatch);
                }
                // v1: an extension blob is either optional for every
                // purpose or required exactly for Link.
                if *required_for != 0 && *required_for != MemberPurposeSet::LINK {
                    return Err(MemberError::IllegalPurposeCombination(*required_for));
                }
            }
            _ => return Err(MemberError::KeyRoleMismatch),
        }
        Ok(())
    }

    pub fn write(&self, writer: &mut CborWriter) {
        writer.map(5);
        writer.field(1).bytes(self.id.as_bytes());
        writer.field(2);
        self.stable_key.write(writer);
        writer.field(3);
        self.role.write(writer);
        writer.field(4).unsigned(self.byte_length);
        writer.field(5).bytes(self.sha256.as_bytes());
    }

    pub(crate) fn decode(reader: &mut CborReader<'_>) -> Result<Self, MemberError> {
        let mut record = reader.map()?;
        let entries = record.remaining_entries();
        if entries != 5 {
            return Err(MemberError::BadRecordShape);
        }
        expect_key(&mut record, 1, || MemberError::BadRecordShape)?;
        let id = read_digest(&mut record)?;
        expect_key(&mut record, 2, || MemberError::BadRecordShape)?;
        let stable_key = MemberStableKey::decode(&mut record)?;
        expect_key(&mut record, 3, || MemberError::BadRecordShape)?;
        let role = SlibMemberRole::decode(&mut record)?;
        expect_key(&mut record, 4, || MemberError::BadRecordShape)?;
        let byte_length = record.unsigned()?;
        expect_key(&mut record, 5, || MemberError::BadRecordShape)?;
        let sha256 = Digest256::from_bytes(read_digest(&mut record)?);
        if record.next_key()?.is_some() {
            return Err(MemberError::BadRecordShape);
        }
        let id = SlibMemberId::from_validated(id);
        SlibMemberRecord::new(id, stable_key, role, byte_length, sha256)
    }

    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        self.write(&mut writer);
        writer.into_bytes()
    }

    /// `SHA-256("scoop-slib-member-content-v1" || canonical record)`.
    pub fn member_fingerprint(&self) -> Digest256 {
        DomainHasher::new(MEMBER_FINGERPRINT_DOMAIN)
            .field(&self.canonical_cbor())
            .finish()
    }

    /// The link fingerprint; valid only for link-participating members.
    pub fn link_member_fingerprint(&self) -> Result<Digest256, MemberError> {
        if !self.role.participates_in_link() {
            return Err(MemberError::LinkFingerprintOfNonLinkMember);
        }
        Ok(DomainHasher::new(LINK_MEMBER_FINGERPRINT_DOMAIN)
            .field(&self.canonical_cbor())
            .finish())
    }
}

fn write_capability(writer: &mut CborWriter, capability: &CapabilityId) {
    writer.map(3);
    writer.field(1).text(capability.namespace());
    writer.field(2).text(capability.name());
    writer.field(3).unsigned(capability.major_version() as u64);
}

fn read_capability(reader: &mut CborReader<'_>) -> Result<CapabilityId, MemberError> {
    let mut record = reader.map()?;
    let entries = record.remaining_entries();
    if entries != 3 {
        return Err(MemberError::BadCapabilityShape);
    }
    expect_key(&mut record, 1, || MemberError::BadCapabilityShape)?;
    let namespace = record.text()?.to_owned();
    expect_key(&mut record, 2, || MemberError::BadCapabilityShape)?;
    let name = record.text()?.to_owned();
    expect_key(&mut record, 3, || MemberError::BadCapabilityShape)?;
    let major = read_u32(record.unsigned()?)?;
    if record.next_key()?.is_some() {
        return Err(MemberError::BadCapabilityShape);
    }
    CapabilityId::new(&namespace, &name, major).map_err(MemberError::Capability)
}

/// Reads the next field key and requires it to equal `expected`.
fn expect_key(
    record: &mut scoop_identity::MapGuard<'_, '_>,
    expected: u64,
    shape: impl FnOnce() -> MemberError,
) -> Result<(), MemberError> {
    match record.next_key()? {
        Some(key) if key == expected => Ok(()),
        _ => Err(shape()),
    }
}

fn read_digest(reader: &mut CborReader<'_>) -> Result<[u8; 32], MemberError> {
    let bytes = reader.bytes()?;
    if bytes.len() != 32 {
        return Err(MemberError::DigestLength(bytes.len()));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(bytes);
    Ok(raw)
}

/// Member-directory level failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberError {
    LogicalKeyLength(usize),
    BadStableKeyShape,
    BadRoleShape,
    BadRecordShape,
    BadCapabilityShape,
    KeyRoleMismatch,
    CapabilityMismatch,
    Capability(CapabilityError),
    IllegalPurposeCombination(u32),
    DigestLength(usize),
    FieldOverflow(u64),
    Cbor(scoop_identity::CborError),
    LinkFingerprintOfNonLinkMember,
}

impl fmt::Display for MemberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemberError::LogicalKeyLength(len) => {
                write!(f, "logical key of {len} bytes is outside 1..=4096")
            }
            MemberError::BadStableKeyShape => write!(f, "malformed member stable key"),
            MemberError::BadRoleShape => write!(f, "malformed member role"),
            MemberError::BadRecordShape => write!(f, "malformed member record"),
            MemberError::BadCapabilityShape => write!(f, "malformed capability id"),
            MemberError::KeyRoleMismatch => {
                write!(f, "member stable key does not pair with this role")
            }
            MemberError::CapabilityMismatch => {
                write!(f, "stable key capability differs from the role capability")
            }
            MemberError::Capability(error) => write!(f, "invalid capability: {error}"),
            MemberError::IllegalPurposeCombination(bits) => write!(
                f,
                "extension blob required_for=0x{bits:08x} is not a legal v1 purpose set"
            ),
            MemberError::DigestLength(len) => write!(f, "digest field of {len} bytes, expected 32"),
            MemberError::FieldOverflow(value) => write!(f, "field value {value} does not fit u32"),
            MemberError::Cbor(error) => write!(f, "canonical CBOR error: {error}"),
            MemberError::LinkFingerprintOfNonLinkMember => {
                write!(f, "link member fingerprint requested for a non-link member")
            }
        }
    }
}

impl std::error::Error for MemberError {}

impl From<scoop_identity::CborError> for MemberError {
    fn from(error: scoop_identity::CborError) -> Self {
        MemberError::Cbor(error)
    }
}

impl From<CapabilityError> for MemberError {
    fn from(error: CapabilityError) -> Self {
        MemberError::Capability(error)
    }
}
