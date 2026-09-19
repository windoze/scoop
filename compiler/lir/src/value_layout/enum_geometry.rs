//! Target-qualified tagged-enum geometry shared by local construction and
//! cross-Cone replay. GC facts and niche eligibility remain caller-owned.

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{
    NonZeroPow2, StorageGeometryV1, StorageLayoutCursorV1, StoragePlacementPolicyV1,
    StoragePlacementV1,
};
use crate::{BackendScalarKind, LirTargetProfile, TypeInstanceShapeError};

#[derive(Clone, Copy, Debug)]
pub struct EnumVariantGeometryInputV1<'a> {
    pub fields: &'a [StorageGeometryV1],
    pub gc_free: bool,
}

/// A reserved region need not have a size divisible by its alignment: the
/// shared pure region combines independent maximum size and alignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnumStorageRegionV1 {
    offset: u64,
    byte_size: u64,
    alignment: NonZeroPow2,
}
impl EnumStorageRegionV1 {
    pub const fn offset(self) -> u64 {
        self.offset
    }
    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }
    pub const fn alignment(self) -> NonZeroPow2 {
        self.alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumVariantSlotV1 {
    /// The complete maximum-sized shared region, not this variant's size.
    SharedPure(EnumStorageRegionV1),
    Dedicated(EnumStorageRegionV1),
}
impl EnumVariantSlotV1 {
    pub const fn region(self) -> EnumStorageRegionV1 {
        match self {
            Self::SharedPure(region) | Self::Dedicated(region) => region,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantStorageGeometryV1 {
    storage: StorageGeometryV1,
    fields: Vec<StoragePlacementV1>,
    slot: EnumVariantSlotV1,
}
impl EnumVariantStorageGeometryV1 {
    /// Natural variant geometry, also used for the variant's wire slot size.
    pub const fn storage(&self) -> StorageGeometryV1 {
        self.storage
    }
    /// Stored offsets are enum-relative. Every zero-sized field has offset 0.
    pub fn fields(&self) -> &[StoragePlacementV1] {
        &self.fields
    }
    pub const fn slot(&self) -> EnumVariantSlotV1 {
        self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumStorageGeometryV1 {
    storage: StorageGeometryV1,
    tag_layout: EnumStorageRegionV1,
    pure_region: EnumStorageRegionV1,
    variants: Vec<EnumVariantStorageGeometryV1>,
}
impl EnumStorageGeometryV1 {
    pub fn tagged(
        target: LirTargetProfile,
        inputs: &[EnumVariantGeometryInputV1<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<Self, EnumStorageGeometryErrorV1> {
        let path = WirePath::root();
        let mut natural = reserve(inputs.len(), meter, &path)?;
        for (index, input) in inputs.iter().enumerate() {
            let path = path.clone().index(index as u64);
            meter.charge_nodes(1, &path)?;
            meter.charge_edges(input.fields.len() as u64, &path)?;
            meter.charge_work(8, &path)?;
            // Natural layout and absolute field projection each visit once.
            meter.charge_work(input.fields.len() as u64, &path)?;
            meter.charge_work(input.fields.len() as u64, &path)?;
            let mut cursor =
                StorageLayoutCursorV1::new(target, StoragePlacementPolicyV1::Ordinary)?;
            for field in input.fields {
                cursor.push(*field)?;
            }
            natural.push(cursor.finish()?);
        }
        let mut pure_size = 0;
        let mut pure_alignment = NonZeroPow2::new(1)?;
        for (input, geometry) in inputs.iter().zip(&natural) {
            if input.gc_free {
                pure_size = pure_size.max(geometry.size());
                pure_alignment = pure_alignment.max(geometry.alignment());
            }
        }
        let tag = target.scalar_layout(BackendScalarKind::I64);
        let tag = StorageGeometryV1::new(target, tag.size_bytes(), tag.alignment_bytes())?;
        let mut cursor = StorageLayoutCursorV1::new(target, StoragePlacementPolicyV1::Ordinary)?;
        let tag_layout = EnumStorageRegionV1 {
            offset: cursor.push(tag)?.offset(),
            byte_size: tag.size(),
            alignment: tag.alignment(),
        };
        let pure_region = EnumStorageRegionV1 {
            offset: cursor.reserve_region(pure_size, pure_alignment)?.offset(),
            byte_size: pure_size,
            alignment: pure_alignment,
        };
        let mut variants = reserve(inputs.len(), meter, &path)?;
        for (index, (input, storage)) in inputs.iter().zip(natural).enumerate() {
            let slot = if input.gc_free {
                EnumVariantSlotV1::SharedPure(pure_region)
            } else {
                EnumVariantSlotV1::Dedicated(EnumStorageRegionV1 {
                    offset: cursor
                        .reserve_region(storage.size(), storage.alignment())?
                        .offset(),
                    byte_size: storage.size(),
                    alignment: storage.alignment(),
                })
            };
            let mut fields = reserve(input.fields.len(), meter, &path.clone().index(index as u64))?;
            let prefix =
                StorageGeometryV1::new(target, slot.region().offset(), storage.alignment().get())?;
            let mut field_cursor = StorageLayoutCursorV1::with_prefix(prefix);
            for field in input.fields {
                fields.push(field_cursor.push(*field)?);
            }
            variants.push(EnumVariantStorageGeometryV1 {
                storage,
                fields,
                slot,
            });
        }
        Ok(Self {
            storage: cursor.finish()?,
            tag_layout,
            pure_region,
            variants,
        })
    }

    pub const fn storage(&self) -> StorageGeometryV1 {
        self.storage
    }
    pub const fn tag_layout(&self) -> EnumStorageRegionV1 {
        self.tag_layout
    }
    pub const fn pure_region(&self) -> EnumStorageRegionV1 {
        self.pure_region
    }
    pub fn variants(&self) -> &[EnumVariantStorageGeometryV1] {
        &self.variants
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumStorageGeometryErrorV1 {
    Shape(TypeInstanceShapeError),
    Resource(WireError),
}
impl From<TypeInstanceShapeError> for EnumStorageGeometryErrorV1 {
    fn from(error: TypeInstanceShapeError) -> Self {
        Self::Shape(error)
    }
}
impl From<WireError> for EnumStorageGeometryErrorV1 {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for EnumStorageGeometryErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "tagged enum storage geometry: {self:?}")
    }
}
impl std::error::Error for EnumStorageGeometryErrorV1 {}

fn reserve<T>(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<Vec<T>, WireError> {
    meter.check_table_entries(count as u64, path)?;
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, path)?;
    Ok(values)
}

#[cfg(test)]
mod tests;
