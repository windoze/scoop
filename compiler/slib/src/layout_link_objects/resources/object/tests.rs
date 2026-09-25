use super::*;
use scoop_wire::{DecodeLimits, ResourceKind};

#[test]
fn repeated_symbol_name_scans_use_one_cumulative_budget() {
    let mut bytes = vec![b'a'; 511];
    bytes.push(0);
    let mut meter = BudgetMeter::new(DecodeLimits {
        validation_work_units: 512,
        ..DecodeLimits::default()
    });
    assert_eq!(name_length(&bytes, &mut meter).unwrap(), Some(511));
    assert_resource(
        name_length(&bytes, &mut meter),
        ResourceKind::ValidationWorkUnits,
    );
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: 511,
        ..DecodeLimits::default()
    });
    assert_resource(
        name_length(&bytes, &mut short),
        ResourceKind::ValidationWorkUnits,
    );
    assert_eq!(
        name_length(
            b"unterminated",
            &mut BudgetMeter::new(DecodeLimits::default())
        )
        .unwrap(),
        None
    );
}

#[test]
fn object_inspection_charges_work_before_parsing_or_allocating() {
    let member = crate::SlibMemberId::from_stable_key(
        scoop_identity::ConeIdentity::CORE,
        &crate::MemberStableKey::LirMetadata,
    )
    .unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(inspect(&[0; 32], member, &mut meter),
        Err(LayoutLinkObjectContentsError::Resource(error))
        if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. })));
    assert_eq!(meter.usage().owned_bytes, 0);
    assert!(
        matches!(inspect(&[0; 32], member, &mut BudgetMeter::new(DecodeLimits::default())),
        Err(LayoutLinkObjectContentsError::ObjectEnvelope { member: actual, .. }) if actual == member)
    );
}

#[test]
fn object_proof_copy_is_bounded_by_owned_and_logical_heap_limits() {
    let cost = ObjectCosts {
        bytes: 1024,
        sections: 1,
        symbols: 4,
        relocations: 2,
        names: 64,
        longest_name: 32,
        stackmap_bytes: 0,
    };
    for (limits, resource) in [
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
        ),
    ] {
        assert_resource(cost.copy_proof(&mut BudgetMeter::new(limits)), resource);
    }
}

fn assert_resource<T>(result: Result<T, WireError>, resource: ResourceKind) {
    assert!(matches!(result, Err(error)
        if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource)));
}
