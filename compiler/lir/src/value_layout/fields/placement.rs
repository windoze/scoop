use super::*;

impl FieldStorageV1 {
    pub(crate) fn combined_scan<'a>(
        fields: impl IntoIterator<Item = &'a Self>,
    ) -> Result<RefScan, StorageReplayError> {
        super::scans::field_scan(fields)
    }

    /// The enclosing replay supplies its final geometry. The caller must
    /// still prove declaration order and non-overlap across the complete list.
    pub(crate) fn within(
        value: &ValueLayoutConstituentV1,
        offset: u64,
        container: StorageGeometryV1,
    ) -> Result<Self, StorageReplayError> {
        if value.target() != container.target() {
            return Err(StorageReplayError::TargetMismatch);
        }
        let alignment = value.storage().alignment();
        if alignment > container.alignment() || offset % alignment.get() != 0 {
            return Err(StorageReplayError::InvalidFieldPlacement);
        }
        match value.nonzero_ref() {
            Some(layout) => {
                let end = offset
                    .checked_add(layout.storage().size().get())
                    .ok_or(StorageReplayError::InvalidFieldPlacement)?;
                if end > container.size() {
                    return Err(StorageReplayError::InvalidFieldPlacement);
                }
                Ok(Self(FieldBody::Stored {
                    offset: ByteOffsetV1(offset),
                    layout,
                }))
            }
            None if offset == 0 => Ok(Self(FieldBody::ElidedZst {
                exact: value.exact(),
                alignment,
            })),
            None => Err(StorageReplayError::InvalidFieldPlacement),
        }
    }
}
