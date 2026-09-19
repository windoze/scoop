use super::*;
use crate::SelectedDependencyMirCallableResolutionError;
use scoop_wire::{ResourceKind, WireErrorKind};

#[test]
fn shared_bridge_budget_has_inclusive_allocation_and_work_boundaries() {
    let fixture = fixture();
    let section = fixture.section();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let (mut identities, foundation) = fixture.validated_foundation();
    assert_eq!(
        decode(&section)
            .validate_with_meter(fixture.artifact, &mut identities, &foundation, &mut meter)
            .unwrap(),
        section
    );
    let required = meter.usage();
    for exact in [required.logical_heap_bytes, required.logical_heap_bytes + 1] {
        let (mut identities, foundation) = fixture.validated_foundation();
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: exact,
            validation_work_units: required.validation_work_units,
            ..DecodeLimits::default()
        });
        assert!(
            decode(&section)
                .validate_with_meter(fixture.artifact, &mut identities, &foundation, &mut meter)
                .is_ok()
        );
    }
    for limits in [
        DecodeLimits {
            logical_heap_bytes: required.logical_heap_bytes - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: required.validation_work_units - 1,
            ..DecodeLimits::default()
        },
    ] {
        let (mut identities, foundation) = fixture.validated_foundation();
        assert!(
            decode(&section)
                .validate_with_meter(
                    fixture.artifact,
                    &mut identities,
                    &foundation,
                    &mut BudgetMeter::new(limits)
                )
                .is_err()
        );
    }
    let before = meter.usage();
    decode(&section)
        .validate_with_meter(fixture.artifact, &mut identities, &foundation, &mut meter)
        .unwrap();
    assert_eq!(
        meter.usage().logical_heap_bytes,
        before.logical_heap_bytes * 2
    );
    assert_eq!(
        meter.usage().validation_work_units,
        before.validation_work_units * 2
    );
}

#[test]
fn bridge_charges_nested_signature_parameters_before_resolving_them() {
    let fixture = fixture();
    let selected = SelectedDependencyMirCallableV1::try_new(
        fixture.provider,
        DependencyCallableDeclarationId::Function(fixture.foreign_function.id()),
        StrongCallableDefinitionOwner::Function(fixture.foreign_function.id()),
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![fixture.signature.result(); 64],
            fixture.signature.result(),
        ),
    )
    .unwrap();
    let section = CrossConeMirBridgeSectionV1::try_new(
        fixture.artifact,
        &fixture.foundation,
        vec![],
        vec![selected],
    )
    .unwrap();
    let bytes = encode(&section).unwrap();
    let decoded = decode(&section);
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let (mut identities, foundation) = fixture.validated_foundation();
    let mut meter = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: 300,
        ..DecodeLimits::default()
    });
    let error = decoded
        .validate_with_meter(fixture.artifact, &mut identities, &foundation, &mut meter)
        .unwrap_err();
    let CrossConeMirBridgeValidationError::Selected {
        index: 0,
        source: SelectedDependencyMirCallableResolutionError::Resource(error),
    } = error
    else {
        panic!("parameter allocation must fail inside the metered signature");
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::LogicalHeapBytes,
            ..
        }
    ));
}

#[test]
fn bridge_rejects_collection_limit_before_reserving_the_table() {
    let fixture = fixture();
    let (mut identities, foundation) = fixture.validated_foundation();
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_table_entries: 0,
        ..DecodeLimits::default()
    });
    let error = decode(&fixture.section())
        .validate_with_meter(fixture.artifact, &mut identities, &foundation, &mut meter)
        .unwrap_err();
    let CrossConeMirBridgeValidationError::Resource(error) = error else {
        panic!("table budget must fail before record validation");
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticTableEntries,
            ..
        }
    ));
    assert_eq!(meter.usage(), scoop_wire::DecodeUsage::default());
}
