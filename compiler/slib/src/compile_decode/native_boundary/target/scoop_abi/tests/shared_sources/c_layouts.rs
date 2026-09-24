use super::*;
use scoop_identity::{
    CLayoutByteAlignment, CLayoutOverride, CanonicalCAbiLayout, CanonicalCAbiLayoutField,
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCStorageType, FieldIdentityKey, IntegerBitWidth,
    PersistentFieldId, Signedness,
};

mod support;
use support::*;

#[test]
fn materialized_c_layout_replays_shared_fields_without_a_native_witness_or_call() {
    let fixture = DeclaredLayout::new();
    let layout = fixture.contract(0);
    let usage = fixture.check(&[layout], true, true).unwrap();
    assert!(usage.validation_work_units > 0);
    fixture.check(&[], false, true).unwrap();
}

#[test]
fn representation_contracts_require_the_exact_materialized_set_and_canonical_offsets() {
    let fixture = DeclaredLayout::new();
    for (layouts, materialized) in [
        (vec![], true),
        (vec![fixture.contract(0)], false),
        (vec![fixture.contract(2)], true),
        (vec![fixture.contract(0), fixture.contract(2)], true),
    ] {
        assert!(matches!(
            fixture.check(&layouts, materialized, true),
            Err(NativeBoundaryCompileError::Target(
                NativeBoundaryTargetError::CAbiLayoutSetMismatch
            ))
        ));
    }
}

#[test]
fn declared_layouts_keep_dependency_and_resource_checks_in_the_common_replay() {
    let fixture = DeclaredLayout::new();
    assert!(matches!(
        fixture.check(&[fixture.contract(0)], true, false),
        Err(NativeBoundaryCompileError::ClosureRequired { .. })
            | Err(NativeBoundaryCompileError::Target(
                NativeBoundaryTargetError::MissingExactType { .. }
            ))
    ));
    let mut budget = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        dependencies::collect(
            fixture.current.borrow(),
            &[fixture.scalar.borrow()],
            &mut budget
        ),
        Err(NativeBoundaryCompileError::Identity(
            scoop_identity::IdentityValidationError::Resource(_)
        ))
    ));
}
