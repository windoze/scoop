//! Specialization keys and ODR groups (DESIGN section 3.4).
//!
//! A specialization identifies one program-wide semantic entity
//! instantiation; every generated member of that specialization shares
//! its group id and differs by generated role and typed owner path.

use crate::persistent::{
    PersistentExactTypeId, PersistentExtensionPropertyId, PersistentGenericCallableId,
    PersistentGenericTypeId,
};
use crate::{CborWriter, DomainHasher};

pub const ODR_GROUP_DOMAIN: &[u8] = "scoop-odr-v1".as_bytes();
pub const ODR_MEMBER_DOMAIN: &[u8] = "scoop-odr-member-v1".as_bytes();

/// Typed marker: a callable specialization without an owner application
/// (top-level generic function). Distinct from "has an owner" so the
/// variant cannot be confused with an empty vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoOwnerApplication;

impl NoOwnerApplication {
    pub const OWNER_NONE: Self = Self;
}

/// Typed marker: a callable specialization without callable type
/// arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoCallableArguments;

impl NoCallableArguments {
    pub const ARGUMENTS_NONE: Self = Self;
}

/// The origin of a delegated property specialization.
pub type DelegatedPropertyOrigin = PersistentExtensionPropertyId;

/// Owner application of a callable specialization: either a typed
/// "no owner" marker or the exact receiver application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerApplication {
    None(NoOwnerApplication),
    Exact(PersistentExactTypeId),
}

/// Callable arguments of a callable specialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableArguments {
    None(NoCallableArguments),
    Exact(Vec<PersistentExactTypeId>),
}

/// The closed specialization key (DESIGN 3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializationKey {
    Nominal {
        origin: PersistentGenericTypeId,
        arguments: Vec<PersistentExactTypeId>,
    },
    Callable {
        origin: PersistentGenericCallableId,
        owner_application: OwnerApplication,
        callable_arguments: CallableArguments,
    },
    DelegatedProperty {
        origin: DelegatedPropertyOrigin,
        receiver_arguments: Vec<PersistentExactTypeId>,
    },
    StructuralType {
        exact_type: PersistentExactTypeId,
    },
}

impl SpecializationKey {
    pub fn callable(
        origin: PersistentGenericCallableId,
        owner_application: OwnerApplication,
        callable_arguments: CallableArguments,
    ) -> Self {
        SpecializationKey::Callable {
            origin,
            owner_application,
            callable_arguments,
        }
    }

    /// Canonical CBOR: variant tag at key 0; nested choices carry their
    /// own tags so "no owner"/"no arguments" never collide with empties.
    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        match self {
            SpecializationKey::Nominal { origin, arguments } => {
                writer.map(3);
                writer.field(0).unsigned(1);
                writer.field(1).bytes(origin.as_bytes());
                writer.field(2).array(arguments.len() as u64);
                for argument in arguments {
                    writer.bytes(argument.as_bytes());
                }
            }
            SpecializationKey::Callable {
                origin,
                owner_application,
                callable_arguments,
            } => {
                writer.map(4);
                writer.field(0).unsigned(2);
                writer.field(1).bytes(origin.as_bytes());
                writer.field(2);
                match owner_application {
                    OwnerApplication::None(_) => {
                        writer.array(1);
                        writer.unsigned(0);
                    }
                    OwnerApplication::Exact(id) => {
                        writer.array(2);
                        writer.unsigned(1);
                        writer.bytes(id.as_bytes());
                    }
                }
                writer.field(3);
                match callable_arguments {
                    CallableArguments::None(_) => {
                        writer.array(1);
                        writer.unsigned(0);
                    }
                    CallableArguments::Exact(arguments) => {
                        writer.array(arguments.len() as u64 + 1);
                        writer.unsigned(1);
                        for argument in arguments {
                            writer.bytes(argument.as_bytes());
                        }
                    }
                }
            }
            SpecializationKey::DelegatedProperty {
                origin,
                receiver_arguments,
            } => {
                writer.map(3);
                writer.field(0).unsigned(3);
                writer.field(1).bytes(origin.as_bytes());
                writer.field(2).array(receiver_arguments.len() as u64);
                for argument in receiver_arguments {
                    writer.bytes(argument.as_bytes());
                }
            }
            SpecializationKey::StructuralType { exact_type } => {
                writer.map(2);
                writer.field(0).unsigned(4);
                writer.field(1).bytes(exact_type.as_bytes());
            }
        }
        writer.into_bytes()
    }
}

/// `SHA-256("scoop-odr-v1" || canonical key)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OdrGroupId([u8; 32]);

impl OdrGroupId {
    pub fn of(key: &SpecializationKey) -> Self {
        let mut raw = [0u8; 32];
        raw.copy_from_slice(
            DomainHasher::new(ODR_GROUP_DOMAIN)
                .field(&key.canonical_cbor())
                .finish()
                .as_bytes(),
        );
        OdrGroupId(raw)
    }

    pub fn from_validated(bytes: [u8; 32]) -> Self {
        OdrGroupId(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Display for OdrGroupId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Generated roles inside an ODR group; the role identifies the member
/// kind without entering the group key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OdrMemberRole {
    Body,
    TypeDescriptor,
    Layout,
    ScanProgram,
    Vtable,
    Itable,
    BoxAdapter,
    AdjustThunk,
    Storage,
    FailureStorage,
    InitializationCell,
    InitializationUnitDescriptor,
    DiagnosticAtom,
}

impl OdrMemberRole {
    pub fn tag(self) -> u64 {
        match self {
            OdrMemberRole::Body => 1,
            OdrMemberRole::TypeDescriptor => 2,
            OdrMemberRole::Layout => 3,
            OdrMemberRole::ScanProgram => 4,
            OdrMemberRole::Vtable => 5,
            OdrMemberRole::Itable => 6,
            OdrMemberRole::BoxAdapter => 7,
            OdrMemberRole::AdjustThunk => 8,
            OdrMemberRole::Storage => 9,
            OdrMemberRole::FailureStorage => 10,
            OdrMemberRole::InitializationCell => 11,
            OdrMemberRole::InitializationUnitDescriptor => 12,
            OdrMemberRole::DiagnosticAtom => 13,
        }
    }
}

/// `SHA-256("scoop-odr-member-v1" || group || role || typed owner path)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OdrMemberId([u8; 32]);

impl OdrMemberId {
    pub fn of(group: &OdrGroupId, role: OdrMemberRole, owner_path: &[u8]) -> Self {
        let mut writer = CborWriter::new();
        writer.array(3);
        writer.bytes(group.as_bytes());
        writer.unsigned(role.tag());
        writer.bytes(owner_path);
        let mut raw = [0u8; 32];
        raw.copy_from_slice(
            DomainHasher::new(ODR_MEMBER_DOMAIN)
                .field(&writer.into_bytes())
                .finish()
                .as_bytes(),
        );
        OdrMemberId(raw)
    }

    pub fn from_validated(bytes: [u8; 32]) -> Self {
        OdrMemberId(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Display for OdrMemberId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
