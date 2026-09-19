use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregateStorageLayoutV1 {
    pub(super) storage: ValueStorageLayoutV1,
    pub(super) fields: Vec<PlacedFieldStorageV1>,
}

impl AggregateStorageLayoutV1 {
    /// Declaration order is retained. ZST fields have canonical offset zero,
    /// and their alignment still participates in the aggregate alignment.
    pub fn ordinary(
        target: LirTargetProfile,
        fields: &[DeclaredFieldStorageV1<'_>],
    ) -> Result<Self, StorageReplayError> {
        let cursor = StorageLayoutCursorV1::new(target, StoragePlacementPolicyV1::Ordinary)
            .map_err(StorageReplayError::Shape)?;
        let placed = place(target, fields, cursor, BTreeSet::new())?;
        finish(target, placed)
    }

    pub const fn storage(&self) -> &ValueStorageLayoutV1 {
        &self.storage
    }
    pub fn fields(&self) -> &[PlacedFieldStorageV1] {
        &self.fields
    }
}

pub(super) struct Placement {
    pub(super) fields: Vec<PlacedFieldStorageV1>,
    pub(super) geometry: StorageGeometryV1,
}

pub(super) fn place(
    target: LirTargetProfile,
    fields: &[DeclaredFieldStorageV1<'_>],
    mut cursor: StorageLayoutCursorV1,
    mut seen: BTreeSet<PersistentFieldId>,
) -> Result<Placement, StorageReplayError> {
    let mut placed = Vec::with_capacity(fields.len());
    for field in fields {
        if field.layout.target != target {
            return Err(StorageReplayError::TargetMismatch);
        }
        if !seen.insert(field.field) {
            return Err(StorageReplayError::DuplicateField(field.field));
        }
        let geometry = StorageGeometryV1::new(
            target,
            field.layout.storage.byte_size(),
            field.layout.storage.alignment().get(),
        )
        .map_err(StorageReplayError::Shape)?;
        let placement = cursor.push(geometry).map_err(StorageReplayError::Shape)?;
        let storage = match field.layout.nonzero_ref() {
            Some(layout) => FieldStorageV1(FieldBody::Stored {
                offset: ByteOffsetV1(placement.offset()),
                layout,
            }),
            None => FieldStorageV1(FieldBody::ElidedZst {
                exact: field.layout.exact(),
                alignment: geometry.alignment(),
            }),
        };
        placed.push(PlacedFieldStorageV1 {
            field: field.field,
            storage,
            access_alignment: placement.access_alignment(),
        });
    }
    Ok(Placement {
        fields: placed,
        geometry: cursor.finish().map_err(StorageReplayError::Shape)?,
    })
}

pub(super) fn finish(
    target: LirTargetProfile,
    placed: Placement,
) -> Result<AggregateStorageLayoutV1, StorageReplayError> {
    let size = placed.geometry.size();
    let alignment = placed.geometry.alignment().get();
    let storage = if size == 0 {
        ValueStorageLayoutV1::zero_sized(alignment)
    } else {
        let scan = field_scan(&placed.fields)?;
        ValueStorageLayoutV1::inline(size, alignment, scan)
    }
    .map_err(StorageReplayError::Shape)?;
    storage
        .validate_target(target)
        .map_err(StorageReplayError::Shape)?;
    Ok(AggregateStorageLayoutV1 {
        storage,
        fields: placed.fields,
    })
}

pub(super) fn field_scan(fields: &[PlacedFieldStorageV1]) -> Result<RefScan, StorageReplayError> {
    super::scans::field_scan(fields.iter().map(PlacedFieldStorageV1::storage))
}
