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
            )
            .unwrap();
            let shared = CallableAbiRecordV1::replay(
                TARGET,
                target,
                signature,
                protocol,
                layouts(),
                &foundation,
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
