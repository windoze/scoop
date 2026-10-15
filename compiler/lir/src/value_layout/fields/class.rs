use crate::TypeInstanceShapeV1;

use super::*;

#[derive(Clone, Copy, Debug)]
pub enum ClassBaseStorageV1<'a> {
    NoBase,
    Base(&'a ClassStorageLayoutV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassBasePrefixV1 {
    NoBase,
    BasePrefix {
        exact: PersistentExactTypeId,
        layout: PersistentLayoutId,
        byte_size: u64,
        alignment: NonZeroPow2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassStorageLayoutV1 {
    pub(crate) exact: PersistentExactTypeId,
    pub(crate) layout: PersistentLayoutId,
    pub(crate) target: LirTargetProfile,
    pub(crate) base_prefix: ClassBasePrefixV1,
    pub(crate) declared_fields: Vec<PlacedFieldStorageV1>,
    pub(crate) complete_fields: Vec<PlacedFieldStorageV1>,
    pub(crate) shape: TypeInstanceShapeV1,
}

impl ClassStorageLayoutV1 {
    pub fn replay(
        target: LirTargetProfile,
        key: LayoutKey,
        base: ClassBaseStorageV1<'_>,
        declared_fields: &[DeclaredFieldStorageV1<'_>],
    ) -> Result<Self, StorageReplayError> {
        require_key(target, &key, true)?;
        let exact = key.exact_type();
        let (base_prefix, cursor, alignment, mut complete_fields) = match base {
            ClassBaseStorageV1::NoBase => (
                ClassBasePrefixV1::NoBase,
                16,
                NonZeroPow2::new(8).map_err(StorageReplayError::Shape)?,
                Vec::new(),
            ),
            ClassBaseStorageV1::Base(base) => {
                if base.target != target {
                    return Err(StorageReplayError::TargetMismatch);
                }
                let alignment = NonZeroPow2::new(base.shape.instance_alignment())
                    .map_err(StorageReplayError::Shape)?;
                (
                    ClassBasePrefixV1::BasePrefix {
                        exact: base.exact,
                        layout: base.layout,
                        byte_size: base.shape.minimum_size(),
                        alignment,
                    },
                    base.shape.minimum_size(),
                    alignment,
                    base.complete_fields.clone(),
                )
            }
        };
        let seen = complete_fields
            .iter()
            .map(PlacedFieldStorageV1::field)
            .collect();
        let prefix = StorageGeometryV1::new(target, cursor, alignment.get())
            .map_err(StorageReplayError::Shape)?;
        let placed = aggregate::place(
            target,
            declared_fields,
            StorageLayoutCursorV1::with_prefix(prefix),
            seen,
        )?;
        let size = placed.geometry.size();
        complete_fields.extend(placed.fields.iter().cloned());
        let scan = aggregate::field_scan(&complete_fields)?;
        let shape = TypeInstanceShapeV1::fixed_object(
            target,
            size,
            placed.geometry.alignment().get(),
            scan,
        )
        .map_err(StorageReplayError::Shape)?;
        Ok(Self {
            exact,
            layout: PersistentLayoutId::from_key(&key).map_err(StorageReplayError::Hash)?,
            target,
            base_prefix,
            declared_fields: placed.fields,
            complete_fields,
            shape,
        })
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
    pub const fn layout(&self) -> PersistentLayoutId {
        self.layout
    }
    pub const fn base_prefix(&self) -> ClassBasePrefixV1 {
        self.base_prefix
    }
    pub const fn shape(&self) -> &TypeInstanceShapeV1 {
        &self.shape
    }
    pub fn declared_fields(&self) -> &[PlacedFieldStorageV1] {
        &self.declared_fields
    }
    pub fn complete_fields(&self) -> &[PlacedFieldStorageV1] {
        &self.complete_fields
    }
}
