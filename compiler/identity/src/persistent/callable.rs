//! Callable body identity (DESIGN section 3.1).
//!
//! Every LIR-defined Scoop callable body — ordinary body, generated
//! adapter, root gateway or init startup gateway — carries one
//! `PersistentCallableBodyId` derived from a closed key. The key uses a
//! frozen `u32` little-endian variant tag and declaration-order product
//! encoding; unknown or reserved tags are rejected on decode.

use core::fmt;

use crate::persistent::{PersistentCallableBodyId, PersistentInitializationUnitId};
use crate::{CborWriter, ConeIdentity, DomainHasher};

pub const CALLABLE_BODY_DOMAIN: &[u8] = "scoop-callable-body-v1".as_bytes();

/// Typed refinement: an ODR member id that must reference a callable
/// atom. Construction from non-callable member ids is a type error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CallableOdrMemberId([u8; 32]);

impl CallableOdrMemberId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        CallableOdrMemberId(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The strong definition owner of one concrete body: a typed refinement
/// over the id of the owning callable entity. Storage/type-descriptor
/// ids do not fit this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StrongCallableDefinitionOwner([u8; 32]);

impl StrongCallableDefinitionOwner {
    pub fn from_function(id: crate::persistent::PersistentFunctionId) -> Self {
        StrongCallableDefinitionOwner(*id.as_bytes())
    }

    pub fn from_property(id: crate::persistent::PersistentPropertyId) -> Self {
        StrongCallableDefinitionOwner(*id.as_bytes())
    }

    pub fn from_dispatch_slot(id: crate::persistent::PersistentDispatchSlotId) -> Self {
        StrongCallableDefinitionOwner(*id.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The id of a root Cone's `main` strong body; only convertible from a
/// body computed for that specific shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MainCallableBodyId([u8; 32]);

impl MainCallableBodyId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        MainCallableBodyId(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The closed callable-body key. v1 variants are frozen: `Strong=1`,
/// `Odr=2`, `RootGateway=3`, `InitializationStartupGateway=4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableBodyKey {
    Strong {
        owner: StrongCallableDefinitionOwner,
    },
    Odr {
        member: CallableOdrMemberId,
    },
    RootGateway {
        root_cone: ConeIdentity,
        main: MainCallableBodyId,
    },
    InitializationStartupGateway {
        unit: PersistentInitializationUnitId,
    },
}

impl CallableBodyKey {
    /// Fixed encoding: `u32` little-endian variant tag, then payload
    /// fields in declaration order.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            CallableBodyKey::Strong { owner } => {
                out.extend_from_slice(&1u32.to_le_bytes());
                out.extend_from_slice(owner.as_bytes());
            }
            CallableBodyKey::Odr { member } => {
                out.extend_from_slice(&2u32.to_le_bytes());
                out.extend_from_slice(member.as_bytes());
            }
            CallableBodyKey::RootGateway { root_cone, main } => {
                out.extend_from_slice(&3u32.to_le_bytes());
                out.extend_from_slice(root_cone.as_bytes());
                out.extend_from_slice(main.as_bytes());
            }
            CallableBodyKey::InitializationStartupGateway { unit } => {
                out.extend_from_slice(&4u32.to_le_bytes());
                out.extend_from_slice(unit.as_bytes());
            }
        }
        out
    }
}

impl PersistentCallableBodyId {
    /// `SHA-256("scoop-callable-body-v1" || canonical key)`.
    pub fn of(key: &CallableBodyKey) -> Self {
        let digest = DomainHasher::new(CALLABLE_BODY_DOMAIN)
            .field(&key.canonical_bytes())
            .finish();
        PersistentCallableBodyId::from_validated(*digest.as_bytes())
    }
}

/// Encodes one wire record pair for callable-body identity tables.
pub struct CallableBodyIdentityRecord {
    pub id: PersistentCallableBodyId,
    pub key: CallableBodyKey,
}

impl CallableBodyIdentityRecord {
    /// Canonical CBOR `{1: id, 2: key bytes}` for manifest sections.
    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        writer.map(2);
        writer.field(1).bytes(self.id.as_bytes());
        writer.field(2).bytes(&self.key.canonical_bytes());
        writer.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableBodyError {
    UnknownTag(u32),
}

impl fmt::Display for CallableBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallableBodyError::UnknownTag(tag) => {
                write!(f, "unknown callable body key tag {tag}")
            }
        }
    }
}

impl std::error::Error for CallableBodyError {}

/// Decodes canonical key bytes; unknown/reserved tags and malformed
/// lengths are rejected without panicking.
pub fn decode_callable_body_key(bytes: &[u8]) -> Result<CallableBodyKey, CallableBodyError> {
    let digest_at = |offset: usize| -> Result<[u8; 32], CallableBodyError> {
        bytes
            .get(offset..offset + 32)
            .and_then(|slice| <[u8; 32]>::try_from(slice).ok())
            .ok_or(CallableBodyError::UnknownTag(0))
    };
    let tag_bytes: [u8; 4] = bytes
        .get(0..4)
        .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
        .ok_or(CallableBodyError::UnknownTag(0))?;
    let tag = u32::from_le_bytes(tag_bytes);
    match tag {
        1 => Ok(CallableBodyKey::Strong {
            owner: StrongCallableDefinitionOwner(digest_at(4)?),
        }),
        2 => Ok(CallableBodyKey::Odr {
            member: CallableOdrMemberId::from_bytes(digest_at(4)?),
        }),
        3 => Ok(CallableBodyKey::RootGateway {
            root_cone: ConeIdentity::from_bytes(&digest_at(4)?),
            main: MainCallableBodyId::from_bytes(digest_at(36)?),
        }),
        4 => Ok(CallableBodyKey::InitializationStartupGateway {
            unit: PersistentInitializationUnitId::from_validated(digest_at(4)?),
        }),
        other => Err(CallableBodyError::UnknownTag(other)),
    }
}
