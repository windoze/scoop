use super::*;

#[test]
fn replay_joins_schema_physical_slot_callable_abi_and_definition() {
    let fixture = DirectFixture::new(1);
    let input = fixture.identity_input();
    let mut resolver = fixture.local_resolver();
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &[input],
        &fixture.foundation,
        &mut resolver,
        &mut meter(),
    )
    .unwrap();

    assert_eq!(record.table(), fixture.vtable.identity_record().id());
    assert_eq!(record.owner_exact(), fixture.owner.identity().exact());
    assert_eq!(record.role(), ExactDispatchRoleV1::Vtable);
    assert_eq!(record.entries()[0].position().into_u32(), 0);
    assert_eq!(record.entries()[0].slot(), fixture.slot);
    assert_eq!(record.entries()[0].callable_abi(), &fixture.abi);
    assert_eq!(
        record.entries()[0].abi(),
        StrongTypeDispatchCallableRefV2::Local(fixture.abi.definition().semantic_id())
    );
    assert_eq!(
        record.definition().semantic_id(),
        fixture.vtable.identity_record().id()
    );

    let bytes = encode(&record).unwrap();
    assert_eq!(bytes[0], 0xa5);
    let decoded =
        decode_canonical::<DecodedExactDispatchExportV1>(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.validate_against(&record, &mut meter()).unwrap(),
        record
    );
}

#[test]
fn reference_dispatch_requires_distinct_managed_receivers_and_preserves_call_signature() {
    let fixture = DirectFixture::reference();
    let input = fixture.reference_input(Some(&fixture.owner));
    let mut resolver = fixture.local_resolver();
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &[input],
        &fixture.foundation,
        &mut resolver,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(
        record.entries()[0]
            .slot_receiver_layout()
            .unwrap()
            .identity()
            .exact(),
        fixture.owner.identity().exact()
    );

    let mut resolver = fixture.local_resolver();
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[fixture.reference_input(None)],
            &fixture.foundation,
            &mut resolver,
            &mut meter(),
        ),
        Err(ExactDispatchError::ReceiverLayout(_))
    ));

    let non_managed: crate::ExactLayoutExportV1 = crate::exact_layout::tests::unit().into();
    let mut non_managed_input = fixture.reference_input(Some(&non_managed));
    non_managed_input.slot_signature = ExactDispatchSlotSignatureV1::new(
        ExactCallableSignature::new(
            Effect::Ordinary,
            Some(non_managed.identity().exact()),
            Vec::new(),
            fixture.abi.canonical_signature().signature().result(),
        ),
        GcEffect::Managed,
    );
    let mut resolver = fixture.local_resolver();
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[non_managed_input],
            &fixture.foundation,
            &mut resolver,
            &mut meter(),
        ),
        Err(ExactDispatchError::ReceiverLayout(_))
    ));

    let same_receiver = ExactDispatchSlotSignatureV1::new(
        fixture.abi.canonical_signature().signature().clone(),
        GcEffect::Managed,
    );
    let mut same = fixture.reference_input(Some(&fixture.target_receiver));
    same.slot_signature = same_receiver;
    let mut resolver = fixture.local_resolver();
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[same],
            &fixture.foundation,
            &mut resolver,
            &mut meter(),
        ),
        Err(ExactDispatchError::ReceiverAdaptation(_))
    ));
}

#[test]
fn replay_rejects_position_duplicate_slot_physical_target_and_budget() {
    let fixture = DirectFixture::new(2);
    let mut first = fixture.identity_input();
    first.position = ExactDispatchPositionV1::from_u32(1);
    let mut resolver = fixture.local_resolver();
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[first, fixture.identity_input()],
            &fixture.foundation,
            &mut resolver,
            &mut meter(),
        ),
        Err(ExactDispatchError::Position { .. })
    ));

    let mut second = fixture.identity_input();
    second.position = ExactDispatchPositionV1::from_u32(1);
    let mut resolver = fixture.local_resolver();
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[fixture.identity_input(), second],
            &fixture.foundation,
            &mut resolver,
            &mut meter(),
        ),
        Err(ExactDispatchError::DuplicateSlot(_))
    ));

    let mut wrong = |_, _: &mut BudgetMeter| {
        Ok(Some(StrongTypeDispatchCallableRefV2::CoreExternal(
            fixture.abi.definition().semantic_id(),
        )))
    };
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[fixture.identity_input(), {
                let mut value = fixture.identity_input();
                value.position = ExactDispatchPositionV1::from_u32(1);
                value.slot = fixture.other_slot;
                value
            }],
            &fixture.foundation,
            &mut wrong,
            &mut meter(),
        ),
        Err(ExactDispatchError::PhysicalCallable(0))
    ));

    let mut resolver = fixture.local_resolver();
    let mut second = fixture.identity_input();
    second.position = ExactDispatchPositionV1::from_u32(1);
    second.slot = fixture.other_slot;
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[fixture.identity_input(), second],
            &fixture.foundation,
            &mut resolver,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            }),
        ),
        Err(ExactDispatchError::Resource(_))
    ));
}
