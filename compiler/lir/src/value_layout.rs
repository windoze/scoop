//! Closed storage constituents shared by value, array and descriptor layout.
//! They carry physical facts only; an exported layout still needs its exact
//! identity, representation replay and definition ownership to be validated.

use std::{num::NonZeroU64, sync::Arc};

use crate::{CheckedRefScanV1, LirTargetProfile, RefScan, TypeInstanceShapeError};

mod fields;
pub use fields::*;

mod wire;
pub use wire::{DecodedArrayElementStorageV1, DecodedValueStorageLayoutV1};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NonZeroPow2(NonZeroU64);

impl NonZeroPow2 {
    pub fn new(value: u64) -> Result<Self, TypeInstanceShapeError> {
        let value = NonZeroU64::new(value).ok_or(TypeInstanceShapeError::ZeroAlignment)?;
        if !value.get().is_power_of_two() {
            return Err(TypeInstanceShapeError::AlignmentNotPowerOfTwo(value.get()));
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn align_up(self, offset: u64) -> Result<u64, TypeInstanceShapeError> {
        offset
            .checked_add(self.get() - 1)
            .map(|sum| sum & !(self.get() - 1))
            .ok_or(TypeInstanceShapeError::SizeOverflow)
    }
}

/// Nonzero value storage cannot be obtained from raw size/alignment fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonZeroValueStorageV1 {
    size: NonZeroU64,
    alignment: NonZeroPow2,
    scan: CheckedRefScanV1,
}

impl NonZeroValueStorageV1 {
    fn new(size: u64, alignment: u64, scan: RefScan) -> Result<Self, TypeInstanceShapeError> {
        let size = NonZeroU64::new(size).ok_or(TypeInstanceShapeError::ZeroInlineSize)?;
        let alignment = NonZeroPow2::new(alignment)?;
        if size.get() % alignment.get() != 0 {
            return Err(TypeInstanceShapeError::UnalignedInlineSize);
        }
        let scan = CheckedRefScanV1::from_canonical(scan).map_err(TypeInstanceShapeError::Scan)?;
        scan.validate_extent(size.get(), alignment.get())
            .map_err(TypeInstanceShapeError::Scan)?;
        Ok(Self {
            size,
            alignment,
            scan,
        })
    }

    pub const fn size(&self) -> NonZeroU64 {
        self.size
    }
    pub const fn alignment(&self) -> NonZeroPow2 {
        self.alignment
    }
    pub const fn scan(&self) -> &CheckedRefScanV1 {
        &self.scan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StorageBody {
    ZeroSized(NonZeroPow2),
    Inline(Arc<NonZeroValueStorageV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueStorageLayoutV1(StorageBody);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueStorageKindV1<'a> {
    ZeroSized {
        alignment: NonZeroPow2,
    },
    Inline {
        size: NonZeroU64,
        alignment: NonZeroPow2,
        scan: &'a CheckedRefScanV1,
    },
}

impl ValueStorageLayoutV1 {
    pub fn zero_sized(alignment: u64) -> Result<Self, TypeInstanceShapeError> {
        Ok(Self(StorageBody::ZeroSized(NonZeroPow2::new(alignment)?)))
    }

    pub fn inline(
        size: u64,
        alignment: u64,
        scan: RefScan,
    ) -> Result<Self, TypeInstanceShapeError> {
        Ok(Self(StorageBody::Inline(Arc::new(
            NonZeroValueStorageV1::new(size, alignment, scan)?,
        ))))
    }

    pub fn validate_target(&self, target: LirTargetProfile) -> Result<(), TypeInstanceShapeError> {
        let maximum = target.contract().maximum_managed_alignment();
        if self.alignment().get() > maximum {
            return Err(TypeInstanceShapeError::ManagedAlignmentTooLarge {
                actual: self.alignment().get(),
                maximum,
            });
        }
        let maximum = target.contract().maximum_managed_object_size();
        if self.byte_size() > maximum {
            return Err(TypeInstanceShapeError::ManagedObjectTooLarge {
                actual: self.byte_size(),
                maximum,
            });
        }
        Ok(())
    }

    pub fn kind(&self) -> ValueStorageKindV1<'_> {
        match &self.0 {
            StorageBody::ZeroSized(alignment) => ValueStorageKindV1::ZeroSized {
                alignment: *alignment,
            },
            StorageBody::Inline(value) => ValueStorageKindV1::Inline {
                size: value.size,
                alignment: value.alignment,
                scan: &value.scan,
            },
        }
    }

    pub fn nonzero(&self) -> Option<&NonZeroValueStorageV1> {
        match &self.0 {
            StorageBody::ZeroSized(_) => None,
            StorageBody::Inline(value) => Some(value),
        }
    }

    pub fn byte_size(&self) -> u64 {
        match &self.0 {
            StorageBody::ZeroSized(_) => 0,
            StorageBody::Inline(value) => value.size.get(),
        }
    }

    pub fn alignment(&self) -> NonZeroPow2 {
        match &self.0 {
            StorageBody::ZeroSized(alignment) => *alignment,
            StorageBody::Inline(value) => value.alignment,
        }
    }
}

/// Array stride is the complete nonzero value extent, including tail padding.
/// There is no independent size field that could disagree with the stride.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayElementStorageV1(StorageBody);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayElementStorageKindV1<'a> {
    ZeroSized {
        alignment: NonZeroPow2,
    },
    Inline {
        stride: NonZeroU64,
        alignment: NonZeroPow2,
        scan: &'a CheckedRefScanV1,
    },
}

impl ArrayElementStorageV1 {
    pub fn zero_sized(alignment: u64) -> Result<Self, TypeInstanceShapeError> {
        ValueStorageLayoutV1::zero_sized(alignment).map(|value| Self(value.0))
    }

    pub fn inline(
        size: u64,
        alignment: u64,
        scan: RefScan,
    ) -> Result<Self, TypeInstanceShapeError> {
        ValueStorageLayoutV1::inline(size, alignment, scan).map(|value| Self(value.0))
    }

    pub fn from_value(value: &ValueStorageLayoutV1) -> Self {
        Self(value.0.clone())
    }

    pub fn kind(&self) -> ArrayElementStorageKindV1<'_> {
        match &self.0 {
            StorageBody::ZeroSized(alignment) => ArrayElementStorageKindV1::ZeroSized {
                alignment: *alignment,
            },
            StorageBody::Inline(value) => ArrayElementStorageKindV1::Inline {
                stride: value.size,
                alignment: value.alignment,
                scan: &value.scan,
            },
        }
    }
}

#[cfg(test)]
mod tests;
