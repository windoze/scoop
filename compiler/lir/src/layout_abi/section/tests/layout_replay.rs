use super::*;

#[test]
fn staged_layout_replay_preserves_the_remaining_wire_and_requires_the_same_layouts() {
    let expected = empty_exports(cone("layout-stage"));
    let source = Source::default();
    let section = section(expected.clone(), &[], &source).unwrap();
    let bytes = encode(&section).unwrap();
    let decode = || {
        decode_canonical::<DecodedCrossConeLayoutAbiSectionV1>(&bytes, DecodeLimits::default())
            .unwrap()
    };
    let checked = decode()
        .validate_layouts(expected.layouts(), &mut meter())
        .unwrap();
    assert_eq!(checked.layouts(), expected.layouts());
    assert_eq!(encode(&checked).unwrap(), bytes);
    let mut identities = PendingIdentityValidation::new().finish().unwrap();
    let complete = checked
        .validate(
            &expected,
            &[],
            vec![],
            &source,
            &mut identities,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&complete).unwrap(), bytes);

    let changed = empty_exports(cone("different-layout-stage"));
    let checked = decode()
        .validate_layouts(expected.layouts(), &mut meter())
        .unwrap();
    assert!(matches!(
        checked.validate(
            &changed,
            &[],
            vec![],
            &source,
            &mut identities,
            &mut meter()
        ),
        Err(LayoutAbiSectionError::LayoutReplayChanged)
    ));
}
