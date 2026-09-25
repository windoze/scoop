use super::*;
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

#[test]
fn native_requirement_replay_keeps_typed_libraries_and_cumulative_limits() {
    let requirement = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        CanonicalNativeLibraryName::new("budgeted-native").unwrap(),
    ))
    .unwrap();
    let contract = NativeExternalContract::c_function(
        NativeLibraryBinding::Requirement(requirement.id()),
        signature(),
    );
    let input = foundation(
        vec![
            contract_record(1, "native", contract.clone()),
            contract_record(2, "native", contract),
        ],
        vec![requirement],
    );
    let expected = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        LirTargetProfile::DARWIN_AARCH64,
        &input,
    )
    .unwrap();
    let replay = |meter: &mut BudgetMeter| {
        CanonicalNativeExternalRequirementSurfaceV1::from_foundation_with_meter(
            LirTargetProfile::DARWIN_AARCH64,
            &input,
            meter,
        )
    };
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let actual = replay(&mut meter).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.contracts()[0].sources().len(), 2);
    let usage = meter.usage();
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    assert_eq!(replay(&mut exact).unwrap(), expected);
    for (limits, resource, prefix) in [
        (
            DecodeLimits {
                validation_work_units: usage.validation_work_units - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            false,
        ),
        (
            DecodeLimits {
                validation_work_units: usage.validation_work_units,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
            true,
        ),
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
            false,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
            false,
        ),
    ] {
        let mut meter = BudgetMeter::new(limits);
        if prefix {
            meter.charge_work(1, &WirePath::root()).unwrap();
        }
        assert!(
            matches!(replay(&mut meter), Err(CanonicalNativeExternalRequirementBuildError::Resource(error))
            if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource))
        );
    }
}
