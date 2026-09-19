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
        let placed = place(
            target,
            fields,
            0,
            NonZeroPow2::new(1).map_err(StorageReplayError::Shape)?,
            None,
            BTreeSet::new(),
        )?;
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
    pub(super) cursor: u64,
    pub(super) alignment: NonZeroPow2,
}

pub(super) fn place(
    target: LirTargetProfile,
    fields: &[DeclaredFieldStorageV1<'_>],
    mut cursor: u64,
    mut alignment: NonZeroPow2,
    packing: Option<NonZeroPow2>,
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
        let natural = field.layout.storage.alignment();
        let access = packing.map_or(natural, |packing| packing.min(natural));
        alignment = alignment.max(access);
        let storage = if let Some(layout) = field.layout.nonzero_ref() {
            let offset = access.align_up(cursor).map_err(StorageReplayError::Shape)?;
            cursor = offset.checked_add(layout.storage().size().get()).ok_or(
                StorageReplayError::Shape(TypeInstanceShapeError::SizeOverflow),
            )?;
            let maximum = target.contract().maximum_managed_object_size();
            if cursor > maximum {
                return Err(StorageReplayError::Shape(
                    TypeInstanceShapeError::ManagedObjectTooLarge {
                        actual: cursor,
                        maximum,
                    },
                ));
            }
            FieldStorageV1(FieldBody::Stored {
                offset: ByteOffsetV1(offset),
                layout,
            })
        } else {
            FieldStorageV1(FieldBody::ElidedZst {
                exact: field.layout.exact(),
                alignment: natural,
            })
        };
        placed.push(PlacedFieldStorageV1 {
            field: field.field,
            storage,
            access_alignment: access,
        });
    }
    Ok(Placement {
        fields: placed,
        cursor,
        alignment,
    })
}

pub(super) fn finish(
    target: LirTargetProfile,
    placed: Placement,
) -> Result<AggregateStorageLayoutV1, StorageReplayError> {
    let size = placed
        .alignment
        .align_up(placed.cursor)
        .map_err(StorageReplayError::Shape)?;
    let storage = if size == 0 {
        ValueStorageLayoutV1::zero_sized(placed.alignment.get())
    } else {
        let scan = field_scan(&placed.fields)?;
        ValueStorageLayoutV1::inline(size, placed.alignment.get(), scan)
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
    use crate::ScanBudgetResourceV1 as R;
    let mut nodes = 1_u64;
    let mut words = 2_u64;
    let mut bytes = 12_u64;
    let mut scans = Vec::new();
    for field in fields {
        let FieldBody::Stored { offset, layout } = &field.storage.0 else {
            continue;
        };
        let scan = layout.storage().scan();
        if matches!(scan.as_ref_scan(), RefScan::None) {
            continue;
        }
        let usage = scan.usage();
        for (used, cost, resource) in [
            (&mut nodes, usage.distinct_nodes, R::DistinctNodes),
            (&mut words, usage.distinct_words + 1, R::DistinctWords),
            (&mut bytes, usage.canonical_bytes, R::CanonicalBytes),
        ] {
            let actual = used.checked_add(cost).unwrap_or(u64::MAX);
            if actual > resource.maximum() {
                return Err(StorageReplayError::Scan(
                    crate::RefScanValidationError::BudgetExceeded {
                        resource,
                        maximum: resource.maximum(),
                        actual,
                    },
                ));
            }
            *used = actual;
        }
        scans.push(
            scan.translated(offset.get())
                .map_err(StorageReplayError::Scan)?
                .into_ref_scan(),
        );
    }
    CheckedRefScanV1::normalize(RefScan::Sequence(scans))
        .map(CheckedRefScanV1::into_ref_scan)
        .map_err(StorageReplayError::Scan)
}
