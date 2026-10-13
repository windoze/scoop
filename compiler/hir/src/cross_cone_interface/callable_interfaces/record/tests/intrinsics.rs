use super::*;

#[test]
fn intrinsic_members_can_publish_final_interface_implementations() {
    let fixture = Fixture::new();
    let declaration = CallableTemplateOrigin::Function(fixture.function.id());
    let owner =
        PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(fixture.nominal.id()));
    let effects = CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::NoGc,
        CallableImplementationV1::Intrinsic(crate::IntrinsicFunctionKind::Integer(
            crate::IntegerIntrinsicKind::NoGcOperation {
                kind: crate::IntegerKind::SIGNED_32,
                operation: crate::NoGcIntegerOperation::Add,
            },
        )),
        CallableOperatorRoleV1::Language(crate::CallableOperatorV1::Plus),
        CallableInfixV1::Ordinary,
    )
    .unwrap();
    for access in [
        PublicLookupAccessV1::DirectOnly,
        PublicLookupAccessV1::PublicSlot,
    ] {
        let record = build_record(
            declaration,
            owner,
            empty_binders(),
            None,
            effects.clone(),
            CallableModalityV1::Final,
            access,
        )
        .unwrap();
        assert_eq!(record.access(), access);
        assert_eq!(record.effects(), effects);
        let decoded = decode_record(&record);
        assert_eq!(encode(&decoded).unwrap(), encode(&record).unwrap());
    }
    assert_eq!(
        build_record(
            declaration,
            owner,
            empty_binders(),
            None,
            effects,
            CallableModalityV1::Open,
            PublicLookupAccessV1::PublicSlot,
        ),
        Err(CallableInterfaceRecordBuildError::IntrinsicMustBeFinal {
            declaration,
            modality: CallableModalityV1::Open,
        })
    );
}
