//! Persistent typed identity framework (DESIGN sections 3.1, 3.3 and
//! 3.5).
//!
//! Every cross-artifact entity kind gets its own id type: same-width
//! digests that cannot be confused at the type level and carry no
//! cast-based escape hatch. Ids are domain-separated hashes over
//! canonical definition keys; the canonical key is also kept for
//! verification, never as a lookup fallback.

pub mod callable;
pub mod exact_type;
pub mod odr;

#[cfg(test)]
mod tests;

pub use callable::{
    CallableBodyError, CallableBodyIdentityRecord, CallableBodyKey, CallableOdrMemberId,
    MainCallableBodyId, StrongCallableDefinitionOwner, decode_callable_body_key,
};
pub use exact_type::{
    AtomOwnerStep, DiagnosticNameContext, ExactTypeError, ExactTypeKey, ExactTypeTable,
    GeneratedNominalRole, ManagedFunctionEffect, NativeCallingConvention, NominalAtom,
    NominalKindTag, PersistentExactTypeId,
};
pub use odr::{
    CallableArguments, DelegatedPropertyOrigin, NoCallableArguments, NoOwnerApplication,
    OdrGroupId, OdrMemberId, OdrMemberRole, OwnerApplication, SpecializationKey,
};

use crate::{CborWriter, ConeIdentity, Digest256, DomainHasher};

/// Declares one persistent id newtype with its own hash domain.
macro_rules! persistent_id {
    ($(#[$meta:meta])* $name:ident, $domain:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Digest256);

        impl $name {
            /// Hashes the canonical definition-key bytes under this
            /// kind's domain.
            pub fn from_definition_key(cone: ConeIdentity, key: &[u8]) -> Self {
                let digest = DomainHasher::new($domain.as_bytes())
                    .field(cone.as_bytes())
                    .field(key)
                    .finish();
                $name(digest)
            }

            /// Wraps a digest that was verified against its key.
            pub fn from_validated(bytes: [u8; 32]) -> Self {
                $name(Digest256::from_bytes(bytes))
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                self.0.as_bytes()
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, stringify!($name))?;
                write!(f, "({})", self.0)
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

persistent_id!(
    /// A declared nominal type (class/struct/enum/interface/object).
    PersistentTypeId,
    "scoop-persistent-type-v1"
);
persistent_id!(
    /// A declared generic nominal type template.
    PersistentGenericTypeId,
    "scoop-persistent-generic-type-v1"
);
persistent_id!(
    /// A non-generic source function.
    PersistentFunctionId,
    "scoop-persistent-function-v1"
);
persistent_id!(
    /// A generic function template.
    PersistentGenericFunctionId,
    "scoop-persistent-generic-function-v1"
);
persistent_id!(
    /// A generic callable template (function or method) as used by
    /// specialization keys.
    PersistentGenericCallableId,
    "scoop-persistent-generic-callable-v1"
);
persistent_id!(
    /// A declared property.
    PersistentPropertyId,
    "scoop-persistent-property-v1"
);
persistent_id!(
    /// A delegated extension property template.
    PersistentExtensionPropertyId,
    "scoop-persistent-extension-property-v1"
);
persistent_id!(
    /// A non-generic type alias.
    PersistentTypeAliasId,
    "scoop-persistent-type-alias-v1"
);
persistent_id!(
    /// A dispatch slot (virtual/interface method slot).
    PersistentDispatchSlotId,
    "scoop-persistent-dispatch-slot-v1"
);
persistent_id!(
    /// A binary initialization unit.
    PersistentInitializationUnitId,
    "scoop-persistent-initialization-unit-v1"
);
persistent_id!(
    /// A target-specific layout record. Not a language type identity.
    PersistentLayoutId,
    "scoop-persistent-layout-v1"
);
persistent_id!(
    /// A compiler-owned static storage atom.
    PersistentStaticStorageId,
    "scoop-persistent-static-storage-v1"
);
persistent_id!(
    /// An immortal (read-only, pre-initialized) object.
    PersistentImmortalObjectId,
    "scoop-persistent-immortal-object-v1"
);
persistent_id!(
    /// One safepoint site inside a concrete callable body.
    PersistentSafepointSiteId,
    "scoop-persistent-safepoint-site-v1"
);
persistent_id!(
    /// The owner identity of one LIR-defined callable body.
    PersistentCallableBodyId,
    "scoop-persistent-callable-body-v1"
);

/// Canonical definition keys for source-declared and compiler-generated
/// entities (DESIGN 3.1). `signature` is the language-side normalized
/// signature key; the identity crate treats it as opaque canonical
/// bytes. Owner chains are typed `(kind, id)` steps — no raw FQN ever
/// participates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionKey {
    Source {
        package: String,
        owner: Vec<OwnerStep>,
        name: String,
        signature: Vec<u8>,
    },
    /// Compiler-generated entity anchored at an owning persistent
    /// callable plus a structural definition path.
    GeneratedByOwner {
        owner: OwnerStep,
        structural_path: Vec<u8>,
    },
    /// Compiler-generated entity anchored at a generation role with
    /// typed operands (exact types / callables).
    GeneratedByRole {
        role: GeneratedRole,
        operands: Vec<[u8; 32]>,
        discriminator: Vec<u8>,
    },
}

/// One typed step of an owner chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerStep {
    pub kind: OwnerKind,
    pub id: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    Type,
    GenericType,
    Function,
    GenericFunction,
    Property,
    ExtensionProperty,
    TypeAlias,
    Object,
}

impl OwnerKind {
    fn tag(self) -> u64 {
        match self {
            OwnerKind::Type => 1,
            OwnerKind::GenericType => 2,
            OwnerKind::Function => 3,
            OwnerKind::GenericFunction => 4,
            OwnerKind::Property => 5,
            OwnerKind::ExtensionProperty => 6,
            OwnerKind::TypeAlias => 7,
            OwnerKind::Object => 8,
        }
    }
}

/// Closed set of compiler generation roles for `GeneratedByRole` keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedRole {
    ClosureEnvironment,
    CallableAdapter,
    CoroutineFrame,
    CallbackTrampoline,
    BoxAdapter,
    AdjustThunk,
    InitStorage,
    FailureStorage,
    StringConstant,
    ScanProgram,
}

impl GeneratedRole {
    fn tag(self) -> u64 {
        match self {
            GeneratedRole::ClosureEnvironment => 1,
            GeneratedRole::CallableAdapter => 2,
            GeneratedRole::CoroutineFrame => 3,
            GeneratedRole::CallbackTrampoline => 4,
            GeneratedRole::BoxAdapter => 5,
            GeneratedRole::AdjustThunk => 6,
            GeneratedRole::InitStorage => 7,
            GeneratedRole::FailureStorage => 8,
            GeneratedRole::StringConstant => 9,
            GeneratedRole::ScanProgram => 10,
        }
    }
}

impl DefinitionKey {
    /// Canonical CBOR product encoding: variant tag at key 0.
    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        match self {
            DefinitionKey::Source {
                package,
                owner,
                name,
                signature,
            } => {
                writer.map(5);
                writer.field(0).unsigned(1);
                writer.field(1).text(package);
                writer.field(2).array(owner.len() as u64);
                for step in owner {
                    writer.array(2);
                    writer.unsigned(step.kind.tag());
                    writer.bytes(&step.id);
                }
                writer.field(3).text(name);
                writer.field(4).bytes(signature);
            }
            DefinitionKey::GeneratedByOwner {
                owner,
                structural_path,
            } => {
                writer.map(3);
                writer.field(0).unsigned(2);
                writer.field(1).array(2);
                writer.unsigned(owner.kind.tag());
                writer.bytes(&owner.id);
                writer.field(2).bytes(structural_path);
            }
            DefinitionKey::GeneratedByRole {
                role,
                operands,
                discriminator,
            } => {
                writer.map(4);
                writer.field(0).unsigned(3);
                writer.field(1).unsigned(role.tag());
                writer.field(2).array(operands.len() as u64);
                for operand in operands {
                    writer.bytes(operand);
                }
                writer.field(3).bytes(discriminator);
            }
        }
        writer.into_bytes()
    }
}

/// Linker-visible symbol kinds (DESIGN 3.3). One closed set consumed by
/// the mangler; `@Extern` native symbols never pass through here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Constructor,
    Accessor,
    ClosureAdapter,
    CoroutineFrame,
    CallbackAdapter,
    Global,
    PropertyStorage,
    DelegateStorage,
    SingletonRoot,
    FailureStorage,
    InitCell,
    InitEntry,
    InitDescriptor,
    TypeDescriptor,
    StringConstant,
    Constant,
    ScanProgram,
    LayoutHelper,
    Vtable,
    Itable,
    AdjustThunk,
    CBridgeWrapper,
    ImageDescriptor,
    ProgramDescriptor,
    RootEntryGateway,
    InitStartupGateway,
    CallableRegistration,
    SafepointRegistration,
    StaticStorageDescriptor,
    ImmortalObjectDescriptor,
    InitializationUnitDescriptor,
}

impl SymbolKind {
    /// Fixed ASCII tag embedded in the mangled symbol.
    pub fn tag(self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Method => "m",
            SymbolKind::Constructor => "ctor",
            SymbolKind::Accessor => "acc",
            SymbolKind::ClosureAdapter => "cladapter",
            SymbolKind::CoroutineFrame => "coro",
            SymbolKind::CallbackAdapter => "cbadapter",
            SymbolKind::Global => "glob",
            SymbolKind::PropertyStorage => "prop",
            SymbolKind::DelegateStorage => "deleg",
            SymbolKind::SingletonRoot => "single",
            SymbolKind::FailureStorage => "failroot",
            SymbolKind::InitCell => "cell",
            SymbolKind::InitEntry => "init",
            SymbolKind::InitDescriptor => "initd",
            SymbolKind::TypeDescriptor => "td",
            SymbolKind::StringConstant => "str",
            SymbolKind::Constant => "const",
            SymbolKind::ScanProgram => "scan",
            SymbolKind::LayoutHelper => "layout",
            SymbolKind::Vtable => "vt",
            SymbolKind::Itable => "it",
            SymbolKind::AdjustThunk => "adj",
            SymbolKind::CBridgeWrapper => "cbridge",
            SymbolKind::ImageDescriptor => "img",
            SymbolKind::ProgramDescriptor => "prog",
            SymbolKind::RootEntryGateway => "rgw",
            SymbolKind::InitStartupGateway => "igw",
            SymbolKind::CallableRegistration => "creg",
            SymbolKind::SafepointRegistration => "sreg",
            SymbolKind::StaticStorageDescriptor => "ssto",
            SymbolKind::ImmortalObjectDescriptor => "iobj",
            SymbolKind::InitializationUnitDescriptor => "iud",
        }
    }
}

/// The single Scoop-owned mangler: `scoop$1$<kind>$<lowercase hex>`.
/// Human names live in debug metadata only.
pub fn mangle(kind: SymbolKind, id: &[u8; 32]) -> String {
    let mut symbol = format!("scoop$1${}$", kind.tag());
    for byte in id {
        symbol.push(char::from_digit((byte >> 4) as u32, 16).expect("hex digit"));
        symbol.push(char::from_digit((byte & 0xF) as u32, 16).expect("hex digit"));
    }
    symbol
}

/// Truncates a 256-bit persistent key to the nonzero 64-bit runtime id
/// (DESIGN 3.5). Two different full keys mapping to the same 64-bit
/// value are a build failure at the table level; this function only
/// derives the value.
pub fn truncated_runtime_id(full_key: &[u8; 32]) -> u64 {
    let mut value = u64::from_le_bytes(full_key[..8].try_into().expect("32 bytes"));
    if value == 0 {
        value = u64::from_le_bytes(full_key[8..16].try_into().expect("32 bytes")) | 1;
    }
    value
}
