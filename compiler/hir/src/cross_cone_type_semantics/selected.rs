//! Persistent selected targets. Actual source roots and every semantic parent
//! are independently replayed; these transport records carry no access proof.
use super::wire;
use crate::InheritanceCallableDeclarationV1;
use scoop_identity::{
    ConeIdentity, PersistentConstructorId, PersistentDispatchSlotId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentObjectValueId,
};

mod decode;
mod encoding;
mod table;
pub use decode::*;
pub use table::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedTypeConstructionV1 {
    Constructor(PersistentConstructorId),
    EnumVariant(PersistentEnumVariantId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedDirectInheritanceEdgeV1 {
    ClassBase { exact: PersistentExactTypeId },
    Interface { exact: PersistentExactTypeId },
}
impl SelectedDirectInheritanceEdgeV1 {
    pub const fn exact(self) -> PersistentExactTypeId {
        match self {
            Self::ClassBase { exact } | Self::Interface { exact } => exact,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedTypeUseV1 {
    Signature {
        exact: PersistentExactTypeId,
    },
    Representation {
        exact: PersistentExactTypeId,
    },
    Construct {
        exact: PersistentExactTypeId,
        declaration: SelectedTypeConstructionV1,
    },
    MemberCall {
        receiver: PersistentExactTypeId,
        declaration: InheritanceCallableDeclarationV1,
    },
    SlotCall {
        receiver: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
    TypeTest {
        exact: PersistentExactTypeId,
    },
    SingletonValue {
        exact: PersistentExactTypeId,
        value: PersistentObjectValueId,
    },
    Inheritance {
        derived: PersistentExactTypeId,
        edge: SelectedDirectInheritanceEdgeV1,
    },
    ShapeSupport {
        exact: PersistentExactTypeId,
    },
}
impl SelectedTypeUseV1 {
    pub const fn exact(self) -> PersistentExactTypeId {
        match self {
            Self::Signature { exact }
            | Self::Representation { exact }
            | Self::Construct { exact, .. }
            | Self::TypeTest { exact }
            | Self::SingletonValue { exact, .. }
            | Self::ShapeSupport { exact } => exact,
            Self::MemberCall { receiver, .. } | Self::SlotCall { receiver, .. } => receiver,
            Self::Inheritance { edge, .. } => edge.exact(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedExternalTypeUseV1 {
    provider: ConeIdentity,
    usage: SelectedTypeUseV1,
}
impl SelectedExternalTypeUseV1 {
    pub const fn new(provider: ConeIdentity, usage: SelectedTypeUseV1) -> Self {
        Self { provider, usage }
    }
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn usage(self) -> SelectedTypeUseV1 {
        self.usage
    }
}

#[cfg(test)]
mod tests;
