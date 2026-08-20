use super::*;

pub(super) fn field_scan<'a>(
    fields: impl IntoIterator<Item = &'a FieldStorageV1>,
) -> Result<RefScan, StorageReplayError> {
    let mut scans = Vec::new();
    for field in fields {
        let FieldBody::Stored { offset, layout } = &field.0 else {
            continue;
        };
        let scan = layout.storage().scan();
        if matches!(scan.as_ref_scan(), RefScan::None) {
            continue;
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
