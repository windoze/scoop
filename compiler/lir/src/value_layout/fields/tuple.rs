use scoop_identity::{CborIdentityRecord, ExactTypeKey};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::*;

mod wire;
pub use wire::DecodedTupleElementStorageV1;

/// An ordinal scoped to one exact tuple, never a nominal declaration field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TupleElementIndexV1 {
    tuple: PersistentExactTypeId,
    ordinal: u64,
}

impl TupleElementIndexV1 {
    pub const fn tuple(self) -> PersistentExactTypeId {
        self.tuple
    }
    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TupleElementStorageV1 {
    index: TupleElementIndexV1,
    storage: FieldStorageV1,
    access_alignment: NonZeroPow2,
}

impl TupleElementStorageV1 {
    pub const fn index(&self) -> TupleElementIndexV1 {
        self.index
    }
    pub const fn storage(&self) -> &FieldStorageV1 {
        &self.storage
    }
    pub const fn access_alignment(&self) -> NonZeroPow2 {
        self.access_alignment
    }
}

/// Physical tuple replay against a canonical exact key and checked values.
/// Export availability and source production gates are not established here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TupleStorageLayoutV1 {
    exact: PersistentExactTypeId,
    storage: ValueStorageLayoutV1,
    elements: Vec<TupleElementStorageV1>,
}

impl TupleStorageLayoutV1 {
    pub fn replay(
        target: LirTargetProfile,
        exact: &CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
        values: &[&ValueLayoutConstituentV1],
    ) -> Result<Self, TupleStorageReplayError> {
        let path = WirePath::root();

        let ExactTypeKey::Tuple(expected) = exact.key() else {
            return Err(TupleStorageReplayError::ExpectedTuple);
        };
        if expected.as_slice().len() != values.len() {
            return Err(TupleStorageReplayError::ArityMismatch);
        }

        let mut cursor = cursor(target)?;
        for (expected, value) in expected.as_slice().iter().zip(values) {
            if *expected != value.exact() {
                return Err(TupleStorageReplayError::ElementTypeMismatch);
            }
            if value.target() != target {
                return Err(StorageReplayError::TargetMismatch.into());
            }
            cursor
                .push(geometry(value)?)
                .map_err(StorageReplayError::Shape)?;
        }
        let whole = cursor.finish().map_err(StorageReplayError::Shape)?;

        let mut elements = Vec::new();
        elements.try_reserve_exact(values.len()).map_err(|_| {
            TupleStorageReplayError::Resource(WireError::new(
                WireErrorKind::Allocation,
                path.clone(),
                None,
            ))
        })?;
        let mut cursor = self::cursor(target)?;
        for (ordinal, value) in values.iter().enumerate() {
            let placement = cursor
                .push(geometry(value)?)
                .map_err(StorageReplayError::Shape)?;
            elements.push(TupleElementStorageV1 {
                index: TupleElementIndexV1 {
                    tuple: exact.id(),
                    ordinal: ordinal as u64,
                },
                storage: FieldStorageV1::within(value, placement.offset(), whole)?,
                access_alignment: placement.access_alignment(),
            });
        }
        let scan = super::scans::field_scan(elements.iter().map(TupleElementStorageV1::storage))?;
        let storage = if whole.size() == 0 {
            ValueStorageLayoutV1::zero_sized(whole.alignment().get())
        } else {
            ValueStorageLayoutV1::inline(whole.size(), whole.alignment().get(), scan)
        }
        .map_err(StorageReplayError::Shape)?;
        Ok(Self {
            exact: exact.id(),
            storage,
            elements,
        })
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
    pub const fn storage(&self) -> &ValueStorageLayoutV1 {
        &self.storage
    }
    pub fn elements(&self) -> &[TupleElementStorageV1] {
        &self.elements
    }
}

fn cursor(target: LirTargetProfile) -> Result<StorageLayoutCursorV1, StorageReplayError> {
    StorageLayoutCursorV1::new(target, StoragePlacementPolicyV1::Ordinary)
        .map_err(StorageReplayError::Shape)
}

fn geometry(value: &ValueLayoutConstituentV1) -> Result<StorageGeometryV1, StorageReplayError> {
    StorageGeometryV1::new(
        value.target(),
        value.storage().byte_size(),
        value.storage().alignment().get(),
    )
    .map_err(StorageReplayError::Shape)
}

#[derive(Debug)]
pub enum TupleStorageReplayError {
    ExpectedTuple,
    ArityMismatch,
    ElementTypeMismatch,
    ElementWireMismatch,
    Storage(StorageReplayError),
    Resource(WireError),
}

impl From<StorageReplayError> for TupleStorageReplayError {
    fn from(error: StorageReplayError) -> Self {
        Self::Storage(error)
    }
}
impl From<WireError> for TupleStorageReplayError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for TupleStorageReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid tuple storage: {self:?}")
    }
}
impl std::error::Error for TupleStorageReplayError {}

#[cfg(test)]
mod tests;
