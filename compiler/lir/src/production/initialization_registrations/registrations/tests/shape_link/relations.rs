use super::*;

#[test]
fn shape_link_terminal_rebind_rejects_same_body_with_changed_gc_protocol() {
    let fixture = ProviderFixture::new(false);
    let target = fixture.callables.records()[0].target();
    let import = ExternalShapeLinkImportV1::replay(
        &fixture.provider(),
        Subject::Callable(target),
        ConeIdentity::CORE,
    )
    .unwrap();
    let result: ExactLayoutExportV1 = crate::exact_layout::tests::unit().into();
    let changed = ExactCallableAbiExportV1::replay(
        TARGET,
        target,
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            result.identity().exact(),
        ),
        ExactCallableProtocolV1::OrdinaryNoGc,
        CallableAbiLayoutInputsV1 {
            receiver: CallableAbiReceiverInputV1::NoReceiver,
            parameters: &[],
            result: &result,
        },
        &fixture.source.foundation,
    )
    .unwrap();
    let callables = CanonicalExactCallableAbiExportsV1::try_new(
        TARGET,
        &fixture.source.foundation,
        vec![changed],
    )
    .unwrap();
    assert!(matches!(
        import.validate_semantic_against(
            &fixture.layouts,
            &callables,
            &fixture.descriptors,
            &fixture.dispatch
        ),
        Err(ShapeLinkError::Contract)
    ));
}
