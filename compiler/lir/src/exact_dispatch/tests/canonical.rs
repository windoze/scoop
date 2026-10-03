use super::*;

#[test]
fn canonical_replay_matches_physical_join_but_does_not_replace_it() {
    let fixture = DirectFixture::reference();
    let canonical = ExactDispatchExportV1::replay_from_schema(
        TARGET,
        fixture.vtable.identity_record(),
        &[fixture.reference_input(Some(&fixture.owner))],
        &fixture.foundation,
    )
    .unwrap();
    let physical = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &[fixture.reference_input(Some(&fixture.owner))],
        &fixture.foundation,
        &mut fixture.local_resolver(),
    )
    .unwrap();
    assert_eq!(canonical, physical);
    let mut missing = |_| Ok(None);
    assert!(matches!(
        ExactDispatchExportV1::replay(
            TARGET,
            (&fixture.vtable).into(),
            &[fixture.reference_input(Some(&fixture.owner))],
            &fixture.foundation,
            &mut missing,
        ),
        Err(ExactDispatchError::MissingPhysicalCallable(0))
    ));
    assert!(matches!(
        ExactDispatchExportV1::replay_from_schema(
            TARGET,
            fixture.vtable.identity_record(),
            &[fixture.reference_input(None)],
            &fixture.foundation,
        ),
        Err(ExactDispatchError::ReceiverLayout(_))
    ));
}

#[test]
fn canonical_replay_preserves_signature_effect_position_and_slot_requirements() {
    let fixture = DirectFixture::new(1);
    let replay = |inputs: &[ExactDispatchEntryInputV1<'_>]| {
        ExactDispatchExportV1::replay_from_schema(
            TARGET,
            fixture.vtable.identity_record(),
            inputs,
            &fixture.foundation,
        )
    };
    let mut wrong = fixture.identity_input();
    wrong.position = ExactDispatchPositionV1::from_u32(1);
    assert!(matches!(
        replay(&[wrong]),
        Err(ExactDispatchError::Position { .. })
    ));
    let mut wrong = fixture.identity_input();
    wrong.slot_signature =
        ExactDispatchSlotSignatureV1::new(wrong.slot_signature.exact().clone(), GcEffect::NoGc);
    assert!(matches!(
        replay(&[wrong]),
        Err(ExactDispatchError::AbiSignature(_))
    ));
    let mut second = fixture.identity_input();
    second.position = ExactDispatchPositionV1::from_u32(1);
    assert!(matches!(
        replay(&[fixture.identity_input(), second]),
        Err(ExactDispatchError::DuplicateSlot(_))
    ));
}
