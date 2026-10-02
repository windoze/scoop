//! A generated equality call keeps the defining provider and exact value type.

use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentGeneratedCallableId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedDerivedEquality {
    pub(crate) provider: ConeIdentity,
    pub(crate) callable: PersistentGeneratedCallableId,
    pub(crate) owner: PersistentExactTypeId,
}

impl ImportedDerivedEquality {
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn callable(self) -> PersistentGeneratedCallableId {
        self.callable
    }
    pub const fn owner(self) -> PersistentExactTypeId {
        self.owner
    }
}
