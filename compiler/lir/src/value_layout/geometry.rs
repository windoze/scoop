//! Checked placement geometry for representation construction. This carries
//! no scan, exact-type, C-FFI predicate, or exported-layout authority.

use crate::{LirCLayoutContract, LirTargetProfile, TypeInstanceShapeError};

use super::NonZeroPow2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageGeometryV1 {
    target: LirTargetProfile,
    size: u64,
    alignment: NonZeroPow2,
}

impl StorageGeometryV1 {
    pub fn new(
        target: LirTargetProfile,
        size: u64,
        alignment: u64,
    ) -> Result<Self, TypeInstanceShapeError> {
        let alignment = NonZeroPow2::new(alignment)?;
        require_target(target, size, alignment)?;
        if size % alignment.get() != 0 {
            return Err(TypeInstanceShapeError::UnalignedInlineSize);
        }
        Ok(Self {
            target,
            size,
            alignment,
        })
    }

    pub const fn size(self) -> u64 {
        self.size
    }
    pub const fn alignment(self) -> NonZeroPow2 {
        self.alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoragePlacementPolicyV1 {
    Ordinary,
    CLayout(LirCLayoutContract),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoragePlacementV1 {
    offset: u64,
    access_alignment: NonZeroPow2,
}

impl StoragePlacementV1 {
    pub const fn offset(self) -> u64 {
        self.offset
    }
    pub const fn access_alignment(self) -> NonZeroPow2 {
        self.access_alignment
    }
}

/// The single checked cursor used for ordinary and packed aggregate fields.
/// A class caller supplies its complete base extent through `with_prefix`.
pub struct StorageLayoutCursorV1 {
    target: LirTargetProfile,
    cursor: u64,
    alignment: NonZeroPow2,
    policy: StoragePlacementPolicyV1,
}

impl StorageLayoutCursorV1 {
    pub fn new(
        target: LirTargetProfile,
        policy: StoragePlacementPolicyV1,
    ) -> Result<Self, TypeInstanceShapeError> {
        let aligned = match policy {
            StoragePlacementPolicyV1::Ordinary => 1,
            StoragePlacementPolicyV1::CLayout(contract) => contract.aligned.bytes().unwrap_or(1),
        };
        let alignment = NonZeroPow2::new(aligned)?;
        require_target(target, 0, alignment)?;
        Ok(Self {
            target,
            cursor: 0,
            alignment,
            policy,
        })
    }

    pub fn with_prefix(prefix: StorageGeometryV1) -> Self {
        Self {
            target: prefix.target,
            cursor: prefix.size,
            alignment: prefix.alignment,
            policy: StoragePlacementPolicyV1::Ordinary,
        }
    }

    pub fn push(
        &mut self,
        field: StorageGeometryV1,
    ) -> Result<StoragePlacementV1, TypeInstanceShapeError> {
        if field.target != self.target {
            return Err(TypeInstanceShapeError::StorageTargetMismatch);
        }
        let access_alignment = match self.policy {
            StoragePlacementPolicyV1::Ordinary => field.alignment,
            StoragePlacementPolicyV1::CLayout(contract) => {
                if field.size == 0 {
                    return Err(TypeInstanceShapeError::ZeroInlineSize);
                }
                match contract.packed.bytes() {
                    None => field.alignment,
                    Some(packed) => field.alignment.min(NonZeroPow2::new(packed)?),
                }
            }
        };
        let alignment = self.alignment.max(access_alignment);
        if field.size == 0 {
            self.alignment = alignment;
            return Ok(StoragePlacementV1 {
                offset: 0,
                access_alignment,
            });
        }
        self.reserve_region(field.size, access_alignment)
    }

    /// Reserve an enum region whose maximum payload size is not independently
    /// tail-padded. Unlike a logical ZST field, an empty region retains its
    /// aligned position after the tag or preceding regions.
    pub fn reserve_region(
        &mut self,
        size: u64,
        access_alignment: NonZeroPow2,
    ) -> Result<StoragePlacementV1, TypeInstanceShapeError> {
        let alignment = self.alignment.max(access_alignment);
        let offset = access_alignment.align_up(self.cursor)?;
        let cursor = offset
            .checked_add(size)
            .ok_or(TypeInstanceShapeError::SizeOverflow)?;
        require_target(self.target, cursor, alignment)?;
        self.cursor = cursor;
        self.alignment = alignment;
        Ok(StoragePlacementV1 {
            offset,
            access_alignment,
        })
    }

    pub fn finish(self) -> Result<StorageGeometryV1, TypeInstanceShapeError> {
        StorageGeometryV1::new(
            self.target,
            self.alignment.align_up(self.cursor)?,
            self.alignment.get(),
        )
    }
}

fn require_target(
    target: LirTargetProfile,
    size: u64,
    alignment: NonZeroPow2,
) -> Result<(), TypeInstanceShapeError> {
    let maximum = target.contract().maximum_managed_alignment();
    if alignment.get() > maximum {
        return Err(TypeInstanceShapeError::ManagedAlignmentTooLarge {
            actual: alignment.get(),
            maximum,
        });
    }
    let maximum = target.contract().maximum_managed_object_size();
    if size > maximum {
        return Err(TypeInstanceShapeError::ManagedObjectTooLarge {
            actual: size,
            maximum,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
