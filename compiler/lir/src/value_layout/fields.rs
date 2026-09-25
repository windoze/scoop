//! Physical layout replay constituents. Identity relations are checked here;
//! source representation and cross-Cone definition ownership remain the
//! responsibility of the complete layout section validator.
//!
//! These builders accept already checked, in-memory declaration sequences.

use std::{collections::BTreeSet, sync::Arc};

use scoop_identity::{
    LayoutKey, PersistentExactTypeId, PersistentFieldId, PersistentLayoutId, RepresentationRole,
};

use super::*;

mod aggregate;
mod c_layout;
mod class;
mod placement;
mod scans;
mod tuple;
mod wire;

pub use aggregate::AggregateStorageLayoutV1;
pub use c_layout::CLayoutStorageReplayV1;
pub use class::{ClassBasePrefixV1, ClassBaseStorageV1, ClassStorageLayoutV1};
pub use tuple::{
    DecodedTupleElementStorageV1, TupleElementIndexV1, TupleElementStorageV1, TupleStorageLayoutV1,
    TupleStorageReplayError,
};
pub use wire::DecodedFieldStorageV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueLayoutConstituentV1 {
    layout: PersistentLayoutId,
    key: LayoutKey,
    target: LirTargetProfile,
    storage: Arc<ValueStorageLayoutV1>,
}

impl ValueLayoutConstituentV1 {
    pub fn new(
        target: LirTargetProfile,
        key: LayoutKey,
        storage: ValueStorageLayoutV1,
    ) -> Result<Self, StorageReplayError> {
        require_key(target, &key, false)?;
        storage
            .validate_target(target)
            .map_err(StorageReplayError::Shape)?;
        Ok(Self {
            layout: PersistentLayoutId::from_key(&key).map_err(StorageReplayError::Hash)?,
            key,
            target,
            storage: Arc::new(storage),
        })
    }

    pub const fn layout(&self) -> PersistentLayoutId {
        self.layout
    }
    pub const fn exact(&self) -> PersistentExactTypeId {
        self.key.exact_type()
    }
    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }
    pub fn storage(&self) -> &ValueStorageLayoutV1 {
        &self.storage
    }

    pub fn nonzero_ref(&self) -> Option<NonZeroValueLayoutRefV1> {
        match &self.storage.0 {
            StorageBody::ZeroSized(_) => None,
            StorageBody::Inline(storage) => Some(NonZeroValueLayoutRefV1 {
                layout: self.layout,
                exact: self.exact(),
                storage: Arc::clone(storage),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonZeroValueLayoutRefV1 {
    layout: PersistentLayoutId,
    exact: PersistentExactTypeId,
    storage: Arc<NonZeroValueStorageV1>,
}

impl NonZeroValueLayoutRefV1 {
    pub const fn layout(&self) -> PersistentLayoutId {
        self.layout
    }
    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
    pub fn storage(&self) -> &NonZeroValueStorageV1 {
        &self.storage
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteOffsetV1(u64);

impl ByteOffsetV1 {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldStorageV1(FieldBody);

#[derive(Clone, Debug, Eq, PartialEq)]
enum FieldBody {
    ElidedZst {
        exact: PersistentExactTypeId,
        alignment: NonZeroPow2,
    },
    Stored {
        offset: ByteOffsetV1,
        layout: NonZeroValueLayoutRefV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldStorageKindV1<'a> {
    ElidedZst {
        exact: PersistentExactTypeId,
        offset: ByteOffsetV1,
        alignment: NonZeroPow2,
    },
    Stored {
        exact: PersistentExactTypeId,
        offset: ByteOffsetV1,
        layout: &'a NonZeroValueLayoutRefV1,
    },
}

impl FieldStorageV1 {
    pub fn kind(&self) -> FieldStorageKindV1<'_> {
        match &self.0 {
            FieldBody::ElidedZst { exact, alignment } => FieldStorageKindV1::ElidedZst {
                exact: *exact,
                offset: ByteOffsetV1(0),
                alignment: *alignment,
            },
            FieldBody::Stored { offset, layout } => FieldStorageKindV1::Stored {
                exact: layout.exact(),
                offset: *offset,
                layout,
            },
        }
    }

    pub fn offset(&self) -> ByteOffsetV1 {
        match self.kind() {
            FieldStorageKindV1::ElidedZst { offset, .. }
            | FieldStorageKindV1::Stored { offset, .. } => offset,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeclaredFieldStorageV1<'a> {
    field: PersistentFieldId,
    layout: &'a ValueLayoutConstituentV1,
}

impl<'a> DeclaredFieldStorageV1<'a> {
    pub const fn new(field: PersistentFieldId, layout: &'a ValueLayoutConstituentV1) -> Self {
        Self { field, layout }
    }
    pub const fn field(self) -> PersistentFieldId {
        self.field
    }
    pub const fn layout(self) -> &'a ValueLayoutConstituentV1 {
        self.layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedFieldStorageV1 {
    field: PersistentFieldId,
    storage: FieldStorageV1,
    access_alignment: NonZeroPow2,
}

impl PlacedFieldStorageV1 {
    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }
    pub const fn storage(&self) -> &FieldStorageV1 {
        &self.storage
    }
    pub const fn access_alignment(&self) -> NonZeroPow2 {
        self.access_alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageReplayError {
    Shape(TypeInstanceShapeError),
    Scan(crate::RefScanValidationError),
    Hash(scoop_wire::HashError),
    TargetMismatch,
    RepresentationRoleMismatch,
    DuplicateField(PersistentFieldId),
    ClassCycle(PersistentExactTypeId),
    CLayoutMismatch,
    EmptyCLayout,
    InvalidCField(PersistentFieldId),
    MissingNestedCLayout(scoop_identity::CanonicalCAbiLayoutFingerprint),
    FieldWireMismatch,
    InvalidFieldPlacement,
    ClassProjectionMismatch,
}

impl std::fmt::Display for StorageReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "storage layout replay failed: {self:?}")
    }
}
impl std::error::Error for StorageReplayError {}

fn require_key(
    target: LirTargetProfile,
    key: &LayoutKey,
    instance: bool,
) -> Result<(), StorageReplayError> {
    if key.target_profile() != &target.wire_id() {
        return Err(StorageReplayError::TargetMismatch);
    }
    if (key.representation() == RepresentationRole::ManagedObject) != instance {
        return Err(StorageReplayError::RepresentationRoleMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
