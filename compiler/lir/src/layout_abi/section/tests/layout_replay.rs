use super::*;

#[test]
fn staged_layout_replay_preserves_the_remaining_wire() {
    let expected = empty_exports(cone("layout-stage"));
    let source = [];
    let section = section(expected.clone(), &[], &source).unwrap();
    let bytes = encode(&section).unwrap();
    let decode = || decode_canonical::<DecodedCrossConeLayoutAbiSectionV1>(&bytes).unwrap();
    let checked = decode().validate_layouts(expected.layouts()).unwrap();
    assert_eq!(checked.layouts(), expected.layouts());
    assert_eq!(encode(&checked).unwrap(), bytes);
}

#[test]
fn staged_callable_replay_preserves_wire_and_rejects_a_different_layout_provider() {
    let expected = empty_exports(cone("abi-stage"));
    let source = [];
    let section = section(expected.clone(), &[], &source).unwrap();
    let bytes = encode(&section).unwrap();
    let decode = || {
        decode_canonical::<DecodedCrossConeLayoutAbiSectionV1>(&bytes)
            .unwrap()
            .validate_layouts(expected.layouts())
            .unwrap()
    };
    let changed = empty_exports(cone("different-abi-stage"));
    assert!(matches!(
        decode().validate_callables(changed.callables()),
        Err(crate::ExactCallableAbiTableError::LayoutProvider)
    ));
    let checked = decode().validate_callables(expected.callables()).unwrap();
    assert_eq!(checked.layouts(), expected.layouts());
    assert_eq!(checked.callables(), expected.callables());
    assert_eq!(encode(&checked).unwrap(), bytes);
}

#[test]
fn staged_dispatch_replay_keeps_wire_and_rejects_a_different_provider() {
    let expected = empty_exports(cone("dispatch-stage"));
    let source = [];
    let section = section(expected.clone(), &[], &source).unwrap();
    let bytes = encode(&section).unwrap();
    let decode = || {
        decode_canonical::<DecodedCrossConeLayoutAbiSectionV1>(&bytes)
            .unwrap()
            .validate_layouts(expected.layouts())
            .unwrap()
            .validate_callables(expected.callables())
            .unwrap()
    };
    let changed = empty_exports(cone("different-dispatch-stage"));
    assert!(matches!(
        decode().validate_dispatch(changed.dispatch()),
        Err(crate::ExactDispatchTableError::LayoutProvider)
    ));
    let checked = decode().validate_dispatch(expected.dispatch()).unwrap();
    assert_eq!(checked.dispatch(), expected.dispatch());
    assert_eq!(encode(&checked).unwrap(), bytes);
}

#[test]
fn staged_descriptor_replay_keeps_wire_and_requires_the_layout_provider() {
    let expected = empty_exports(cone("descriptor-stage"));
    let source = [];
    let section = section(expected.clone(), &[], &source).unwrap();
    let bytes = encode(&section).unwrap();
    let decode = || {
        decode_canonical::<DecodedCrossConeLayoutAbiSectionV1>(&bytes)
            .unwrap()
            .validate_layouts(expected.layouts())
            .unwrap()
            .validate_callables(expected.callables())
            .unwrap()
            .validate_dispatch(expected.dispatch())
            .unwrap()
    };
    let changed = empty_exports(cone("different-descriptor-stage"));
    assert!(matches!(
        decode().validate_descriptors(changed.descriptors()),
        Err(crate::ExactDescriptorTableError::LayoutProvider)
    ));
    let checked = decode()
        .validate_descriptors(expected.descriptors())
        .unwrap();
    assert_eq!(checked.descriptors(), expected.descriptors());
    assert_eq!(encode(&checked).unwrap(), bytes);
}

#[test]
fn staged_shape_replay_retains_all_exports_and_checks_provider_before_selection() {
    let expected = empty_exports(cone("shapes-stage"));
    let source = [];
    let section = section(expected.clone(), &[], &source).unwrap();
    let bytes = encode(&section).unwrap();
    let decode = || {
        decode_canonical::<DecodedCrossConeLayoutAbiSectionV1>(&bytes)
            .unwrap()
            .validate_layouts(expected.layouts())
            .unwrap()
            .validate_callables(expected.callables())
            .unwrap()
            .validate_dispatch(expected.dispatch())
            .unwrap()
            .validate_descriptors(expected.descriptors())
            .unwrap()
    };
    let changed = empty_exports(cone("different-shapes-stage"));
    assert!(matches!(
        decode().validate_shape_support(changed.shape_support(), changed.direct_callables()),
        Err(LayoutAbiSectionError::Exports(
            LayoutAbiExportConstituentsError::Provider
        ))
    ));
    let checked = decode()
        .validate_shape_support(expected.shape_support(), expected.direct_callables())
        .unwrap();
    assert_eq!(checked.exports(), &expected);
    assert_eq!(encode(&checked).unwrap(), bytes);
}
