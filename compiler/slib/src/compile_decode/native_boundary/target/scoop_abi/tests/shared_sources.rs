use super::*;
use scoop_hir::{
    CanonicalBinderListV1, CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1,
    CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1, NominalCLayoutPolicyV1,
    NominalSourceFieldsV1, NominalSourceShapeV1, SourceNominalId, StructSourceShapeV1,
};

#[test]
fn ordinary_shared_shapes_supply_abi_without_native_boundary_records() {
    let reference = shared_only(Fixture::nominal(ConeIdentity::CORE, true), true);
    let value = shared_only(Fixture::nominal(ConeIdentity::SINGLE_FILE, false), false);
    let current = Fixture::empty();
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(reference.exact()),
        vec![value.exact()],
        value.exact(),
    );
    let mut budget = meter();
    let actual = replay(
        &current,
        &[reference.borrow(), value.borrow()],
        &signature,
        &mut budget,
    )
    .unwrap();
    assert!(
        matches!(actual.arguments(), [ScoopAbiArgument::Direct(pointer), ScoopAbiArgument::ElidedZst(zst)]
        if pointer.byte_size() == 8 && zst.byte_size() == 0)
    );
    assert!(matches!(actual.result(), ScoopAbiReturn::ElidedZst(zst) if zst.byte_size() == 0));
    assert!(matches!(
        replay(&current, &[reference.borrow()], &signature, &mut meter()),
        Err(NativeBoundaryCompileError::Target(NativeBoundaryTargetError::MissingExactType { exact }))
            if exact == value.exact()
    ));
    let mut limit = BudgetMeter::new(DecodeLimits {
        validation_work_units: budget.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(
        &current,
        &[reference.borrow(), value.borrow()],
        &signature,
        &mut limit,
    )
    .unwrap();
    assert!(matches!(
        replay(
            &current,
            &[reference.borrow(), value.borrow()],
            &signature,
            &mut limit
        ),
        Err(NativeBoundaryCompileError::Resource(_))
    ));
}

#[test]
fn shared_shape_cannot_claim_another_provider_or_replace_a_native_representation() {
    let current = Fixture::empty();
    let value = shared_only(Fixture::nominal(ConeIdentity::CORE, false), false);
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], value.exact());
    let mut foreign = value.borrow();
    foreign.identity = ConeIdentity::SINGLE_FILE;
    assert!(matches!(
        replay(&current, &[foreign], &signature, &mut meter()),
        Err(NativeBoundaryCompileError::NominalProvider {
            declared: ConeIdentity::CORE,
            ..
        })
    ));
    let original = Fixture::nominal(ConeIdentity::SINGLE_FILE, false);
    let mut changed = original.with_c_layout();
    changed.nominals =
        shared_only(Fixture::nominal(ConeIdentity::SINGLE_FILE, false), false).nominals;
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], changed.exact());
    assert!(matches!(
        replay(&current, &[changed.borrow()], &signature, &mut meter()),
        Err(NativeBoundaryCompileError::ConflictingTypeWitness { .. })
    ));
}

fn shared_only(mut fixture: Fixture, reference: bool) -> Fixture {
    let ExactTypeKey::Nominal(owner) = *fixture
        .identities
        .canonical_key::<_, ExactTypeKey>(fixture.exact())
        .unwrap()
    else {
        panic!("the fixture has a nominal exact type")
    };
    let shape = if reference {
        NominalSourceShapeV1::Class(NominalSourceFieldsV1::try_new(Vec::new()).unwrap())
    } else {
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(Vec::new(), NominalCLayoutPolicyV1::Ordinary).unwrap(),
        )
    };
    fixture.nominals = CanonicalNominalInterfacesV1::try_new(vec![
        crate::nominal_interface_fixture::public_record(
            SourceNominalId::Concrete(owner),
            shape.kind(),
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
            shape,
        )
        .unwrap(),
    ])
    .unwrap();
    fixture.without_native_witness()
}
