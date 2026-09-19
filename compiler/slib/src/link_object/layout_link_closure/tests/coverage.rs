use super::*;
use crate::link_object::layout_link_closure::tests::fixture::meter;
use crate::link_object::strong_relocation_closure::tests::verified_member_with_undefined;
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, verify_current_cone_strong_relocation_closure_v1,
};

#[test]
fn layout_link_coverage_rejects_changed_contents_under_the_same_member_id() {
    let object = fixture_for_producer(scoop_identity::ConeIdentity::SINGLE_FILE, "sameMember");
    let left =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object, b"_first",
        )])
        .unwrap();
    let right =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object, b"_other",
        )])
        .unwrap();
    assert_eq!(left.members()[0].member(), right.members()[0].member());
    same_relocation_proof(&left, &left, &mut meter()).unwrap();
    assert!(matches!(
        same_relocation_proof(&left, &right, &mut meter()),
        Err(LayoutLinkClosureError::ObjectProofMismatch)
    ));
}

#[test]
fn layout_link_coverage_uses_its_own_domain_and_binds_complete_use_fields() {
    let objects =
        crate::link_decode::tests::layout_link_support::verified_code_link_object_members();
    let object = fixture_for_producer(objects.producer(), "coverageUse");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object, b"_shape",
        )])
        .unwrap();
    let uses = [ExternalShapeUndefinedUseV1 {
        use_site: CanonicalUndefinedRelocationUseV1::from(&strong.bindings()[0]),
        import_index: 0,
    }];
    let first = digest(objects.projection(), &uses, &mut meter()).unwrap();
    let old_domain = domain_separated_cbor_hash(
        "scoop-cross-cone-object-coverage-v1",
        &Preimage {
            objects: objects.projection(),
            requirements: &uses,
        },
    )
    .unwrap();
    assert_ne!(first.as_array(), old_domain.as_array());
    let mut changed_index = uses.clone();
    changed_index[0].import_index = 1;
    assert_eq!(
        first,
        digest(objects.projection(), &changed_index, &mut meter()).unwrap()
    );
    assert_ne!(
        first,
        digest(objects.projection(), &[], &mut meter()).unwrap()
    );
    assert!(matches!(
        from_projection(objects.projection(), &uses, &mut meter()),
        Err(LayoutLinkClosureError::UseOutsideObjectSet { .. })
    ));
    let mut limited = BudgetMeter::new(scoop_wire::DecodeLimits {
        validation_work_units: 1,
        ..Default::default()
    });
    assert!(matches!(
        digest(objects.projection(), &uses, &mut limited),
        Err(LayoutLinkClosureError::Resource(_))
    ));
}
