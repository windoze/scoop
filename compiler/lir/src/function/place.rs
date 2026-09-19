use super::{AbiValue, AbiZst, LirType};

/// A logical value whose exact identity survives payload elision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalZstValue {
    exact: scoop_identity::PersistentExactTypeId,
    representation: AbiZst,
}

impl LogicalZstValue {
    pub const fn new(exact: scoop_identity::PersistentExactTypeId, representation: AbiZst) -> Self {
        Self {
            exact,
            representation,
        }
    }

    pub const fn exact(&self) -> scoop_identity::PersistentExactTypeId {
        self.exact
    }

    pub const fn representation(&self) -> &AbiZst {
        &self.representation
    }
}

/// Address identity is live for the complete function activation. Keeping the
/// conservative lifetime explicit permits later liveness-based token reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalPlaceLifetime {
    FunctionActivation,
}

/// An address-observed semantic place. Its enclosing `LocalId` identifies the
/// place; copying this value into another local creates another token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressableZstPlace {
    value: LogicalZstValue,
    lifetime: LocalPlaceLifetime,
}

impl AddressableZstPlace {
    pub const fn new(value: LogicalZstValue, lifetime: LocalPlaceLifetime) -> Self {
        Self { value, lifetime }
    }

    pub const fn value(&self) -> &LogicalZstValue {
        &self.value
    }

    pub const fn lifetime(&self) -> LocalPlaceLifetime {
        self.lifetime
    }
}

/// LIR owns the decision to allocate a value slot, an identity token, or no
/// storage. Codegen must never infer that decision from an LLVM empty struct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalStorage {
    LogicalZst(LogicalZstValue),
    AddressableZst(AddressableZstPlace),
    NonZero(AbiValue),
}

impl LocalStorage {
    pub const fn ty(&self) -> &LirType {
        match self {
            Self::LogicalZst(value) => value.representation().storage_type(),
            Self::AddressableZst(place) => place.value().representation().storage_type(),
            Self::NonZero(value) => value.storage_type(),
        }
    }

    pub const fn is_zst(&self) -> bool {
        matches!(self, Self::LogicalZst(_) | Self::AddressableZst(_))
    }
}

/// One semantic local, including locals that have no physical payload.
#[derive(Debug)]
pub struct Local {
    pub name: String,
    storage: LocalStorage,
}

impl Local {
    pub fn new(name: impl Into<String>, storage: LocalStorage) -> Self {
        Self {
            name: name.into(),
            storage,
        }
    }

    pub const fn ty(&self) -> &LirType {
        self.storage.ty()
    }

    pub const fn storage(&self) -> &LocalStorage {
        &self.storage
    }
}
