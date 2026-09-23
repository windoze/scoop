use scoop_identity::{ConeIdentity, SignatureTypeKey};
use scoop_wire::{DecodeLimits, ResourceKind, WireErrorKind};

use super::*;

mod support;
use support::{nominal, ordinary_provider, section};

#[test]
fn indexes_declared_field_order_for_core_and_ordinary_providers() {
    let core = nominal(ConeIdentity::CORE, 0);
    let ordinary = nominal(ordinary_provider(), 0);
    let core_section = section(vec![core.record.clone()]);
    let ordinary_section = section(vec![ordinary.record.clone()]);
    let index = DefaultStructFields::from_interfaces(
        [&ordinary_section, &core_section],
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
    .unwrap();
    for nominal in [&core, &ordinary] {
        for (position, field) in nominal.fields.iter().enumerate() {
            assert_eq!(
                index.field_index(*field, &nominal.applied_type(0)),
                Ok(position as u32)
            );
        }
    }
    assert!(matches!(
        index.field_index(core.fields[0], &ordinary.applied_type(0)),
        Err(CrossConeHirDefaultFieldError::Owner { .. })
    ));
}

#[test]
fn generic_binding_preserves_owner_identity_and_complete_arity() {
    let generic = nominal(ordinary_provider(), 2);
    let provider = section(vec![generic.record.clone()]);
    let index = DefaultStructFields::from_interfaces(
        [&provider],
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
    .unwrap();
    assert_eq!(
        index.field_index(generic.fields[1], &generic.applied_type(2)),
        Ok(1)
    );
    assert!(matches!(
        index.field_index(generic.fields[1], &generic.applied_type(1)),
        Err(CrossConeHirDefaultFieldError::Arity {
            expected: 2,
            actual: 1,
            ..
        })
    ));
    assert!(matches!(
        index.field_index(
            generic.fields[0],
            &SignatureTypeKey::Binder { depth: 0, index: 0 }
        ),
        Err(CrossConeHirDefaultFieldError::NonNominalOwner(_))
    ));
}

#[test]
fn a_field_is_unavailable_until_its_provider_is_in_the_explicit_closure() {
    let external = nominal(ordinary_provider(), 0);
    let current = section(vec![]);
    let index = DefaultStructFields::from_interfaces(
        [&current],
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
    .unwrap();
    assert_eq!(
        index.field_index(external.fields[0], &external.applied_type(0)),
        Err(CrossConeHirDefaultFieldError::MissingField(
            external.fields[0]
        ))
    );
    let provider = section(vec![external.record.clone()]);
    assert!(matches!(
        DefaultStructFields::from_interfaces(
            [&provider, &provider],
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        ),
        Err(CrossConeHirDefaultDataFlowError::DuplicateField(_))
    ));
}

#[test]
fn index_allocation_and_sorting_use_the_existing_artifact_budget() {
    let nominal = nominal(ordinary_provider(), 0);
    let provider = section(vec![nominal.record]);
    let path = WirePath::root().field(7);
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    DefaultStructFields::from_interfaces([&provider], &mut measured, &path).unwrap();
    let usage = measured.usage();
    assert!(usage.logical_heap_bytes > 0);
    assert!(usage.validation_work_units > 0);
    for resource in [
        ResourceKind::LogicalHeapBytes,
        ResourceKind::ValidationWorkUnits,
    ] {
        let mut limits = DecodeLimits::default();
        match resource {
            ResourceKind::LogicalHeapBytes => limits.logical_heap_bytes = usage.logical_heap_bytes,
            ResourceKind::ValidationWorkUnits => {
                limits.validation_work_units = usage.validation_work_units;
            }
            _ => unreachable!(),
        }
        let mut meter = BudgetMeter::new(limits);
        match resource {
            ResourceKind::LogicalHeapBytes => meter.charge_collection_slots(1, &path).unwrap(),
            ResourceKind::ValidationWorkUnits => meter.charge_work(1, &path).unwrap(),
            _ => unreachable!(),
        }
        let Err(CrossConeHirDefaultDataFlowError::Resource(error)) =
            DefaultStructFields::from_interfaces([&provider], &mut meter, &path)
        else {
            panic!("field index must not reset the artifact budget");
        };
        assert!(
            matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource)
        );
        assert_eq!(error.path(), &path);
    }
}
