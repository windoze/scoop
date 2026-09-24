use super::*;

#[test]
fn shared_record_replays_the_same_complete_layout_and_physical_contract() {
    let unit: ExactLayoutExportV1 = unit().into();
    let byte: ExactLayoutExportV1 = integer("Byte", IntegerKind::SIGNED_8).into();
    let aggregate = fixtures::aggregate(false);
    let zst = fixtures::aggregate(true);
    let (target, foundation) = fixtures::foundation("shared", true);
    for result in [&unit, &byte, &aggregate, &zst] {
        for protocol in [
            ExactCallableProtocolV1::OrdinaryManaged,
            ExactCallableProtocolV1::OrdinaryNoGc,
        ] {
            let parameters = [&unit, &byte, &aggregate, &zst];
            let signature = ExactCallableSignature::new(
                Effect::Ordinary,
                Some(byte.identity().exact()),
                parameters
                    .iter()
                    .map(|value| value.identity().exact())
                    .collect(),
                result.identity().exact(),
            );
            let layouts = || CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::Receiver(&byte),
                parameters: &parameters,
                result,
            };
            let exact = ExactCallableAbiExportV1::replay(
                TARGET,
                target,
                signature.clone(),
                protocol,
                layouts(),
                &foundation,
                &mut meter(),
            )
            .unwrap();
            let shared = CallableAbiRecordV1::replay(
                TARGET,
                target,
                signature,
                protocol,
                layouts(),
                &foundation,
                &mut meter(),
            )
            .unwrap();
            assert_eq!(shared.abi_signature(), exact.canonical_signature());
            assert_eq!(shared.calling_convention(), exact.calling_convention());
            assert_eq!(
                shared.root_plan().canonical_gc_effect(),
                protocol.gc_effect()
            );
            let surface =
                StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
            shared.validate_against(&foundation, &surface).unwrap();
        }
    }
}

#[test]
fn shared_record_requires_each_physical_component_and_the_cumulative_budget() {
    let unit: ExactLayoutExportV1 = unit().into();
    let (target, foundation) = fixtures::foundation("physical", true);
    let replay = |foundation: &OdrFreeLirFoundation, meter: &mut BudgetMeter| {
        CallableAbiRecordV1::replay(
            TARGET,
            target,
            ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit.identity().exact()),
            ExactCallableProtocolV1::OrdinaryManaged,
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[],
                result: &unit,
            },
            foundation,
            meter,
        )
    };
    for missing in 0..4 {
        let mut canonical = foundation.as_canonical().clone();
        match missing {
            0 => canonical.set_callable_bodies(vec![]).unwrap(),
            1 => canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![]).unwrap()),
            2 => canonical.set_definition_plans(vec![]).unwrap(),
            3 => canonical.set_definition_atoms(vec![]).unwrap(),
            _ => unreachable!(),
        }
        let missing = OdrFreeLirFoundation::try_new(foundation.producer(), canonical).unwrap();
        assert!(matches!(
            replay(&missing, &mut meter()),
            Err(CallableAbiReplayError::Exact(_))
        ));
    }
    let mut measured = meter();
    replay(&foundation, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&foundation, &mut shared).unwrap();
    assert!(replay(&foundation, &mut shared).is_err());
    assert!(
        replay(
            &foundation,
            &mut BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            })
        )
        .is_err()
    );
}
