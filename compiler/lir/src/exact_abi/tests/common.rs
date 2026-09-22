use super::*;

#[test]
fn common_abi_replay_matches_actual_exports_for_all_value_pass_modes() {
    let values: Vec<ExactLayoutExportV1> = vec![
        unit().into(),
        integer("Byte", IntegerKind::SIGNED_8).into(),
        managed().into(),
        fixtures::aggregate(false),
        fixtures::aggregate(true),
        fixtures::enumeration(&unit()),
        fixtures::enumeration(&managed()),
    ];
    let parameters = values.iter().collect::<Vec<_>>();
    for result in &values {
        let export = fixtures::function(result, &parameters);
        let signature = replay_canonical_scoop_abi_from_layouts(
            TARGET,
            export.canonical_signature().signature().clone(),
            export.call_protocol(),
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &parameters,
                result,
            },
            &mut meter(),
        )
        .unwrap();
        assert_eq!(&signature, export.canonical_signature());
        assert_eq!(
            encode(&signature).unwrap(),
            encode(export.canonical_signature()).unwrap()
        );
    }
}

#[test]
fn common_abi_replay_requires_exact_receiver_result_and_continuous_budget() {
    let value: ExactLayoutExportV1 = managed().into();
    let byte: ExactLayoutExportV1 = integer("Byte", IntegerKind::SIGNED_8).into();
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(value.identity().exact()),
        vec![],
        byte.identity().exact(),
    );
    let run = |receiver, result, meter: &mut BudgetMeter| {
        replay_canonical_scoop_abi_from_layouts(
            TARGET,
            signature.clone(),
            ExactCallableProtocolV1::OrdinaryManaged,
            CallableAbiLayoutInputsV1 {
                receiver,
                parameters: &[],
                result,
            },
            meter,
        )
    };
    assert!(matches!(
        run(CallableAbiReceiverInputV1::NoReceiver, &byte, &mut meter()),
        Err(ExactCallableAbiError::Receiver)
    ));
    assert!(matches!(
        run(
            CallableAbiReceiverInputV1::Receiver(&byte),
            &byte,
            &mut meter()
        ),
        Err(ExactCallableAbiError::ExactType)
    ));
    let receiver = CallableAbiReceiverInputV1::Receiver(&value);
    assert!(matches!(
        run(receiver, &value, &mut meter()),
        Err(ExactCallableAbiError::ExactType)
    ));
    let mut measured = meter();
    run(receiver, &byte, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    run(receiver, &byte, &mut shared).unwrap();
    assert!(matches!(
        run(receiver, &byte, &mut shared),
        Err(ExactCallableAbiError::Resource(_))
    ));
}
