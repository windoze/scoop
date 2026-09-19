use super::*;

pub(super) fn charge_composition<'a>(
    fields: impl IntoIterator<Item = &'a FieldStorageV1>,
    meter: &mut scoop_wire::BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = scoop_wire::WirePath::root();
    meter.charge_nodes(1, &path)?;
    for field in fields {
        let FieldBody::Stored { layout, .. } = &field.0 else {
            continue;
        };
        let usage = layout.storage().scan().usage();
        meter.charge_work(usage.distinct_nodes, &path)?;
        meter.charge_work(usage.distinct_words, &path)?;
        meter.charge_nodes(usage.distinct_nodes, &path)?;
        meter.charge_collection_slots(usage.distinct_words, &path)?;
        meter.charge_owned_bytes(usage.canonical_bytes, &path)?;
    }
    Ok(())
}

pub(super) fn field_scan<'a>(
    fields: impl IntoIterator<Item = &'a FieldStorageV1>,
) -> Result<RefScan, StorageReplayError> {
    use crate::ScanBudgetResourceV1 as R;
    let mut nodes = 1_u64;
    let mut words = 2_u64;
    let mut bytes = 12_u64;
    let mut scans = Vec::new();
    for field in fields {
        let FieldBody::Stored { offset, layout } = &field.0 else {
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
