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
    fixture.check(&[layout], true, true).unwrap();

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
