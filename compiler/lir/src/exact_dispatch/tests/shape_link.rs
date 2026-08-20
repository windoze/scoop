use super::*;
use crate::{DecodedShapeLinkContractV1, ShapeLinkContractV1, ShapeLinkError};

#[test]
fn shape_link_dispatch_and_callable_contracts_keep_signature_gc_slots_and_adaptation() {
    let fixture = DirectFixture::reference();
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &[fixture.reference_input(Some(&fixture.owner))],
        &fixture.foundation,
        &mut fixture.local_resolver(),
    )
    .unwrap();
    let callable = ShapeLinkContractV1::CallableAbi {
        canonical_signature: fixture.abi.canonical_signature().clone(),
        calling_convention: fixture.abi.calling_convention(),
        protocol: fixture.abi.call_protocol(),
    };
    let dispatch = ShapeLinkContractV1::Dispatch {
        table_projection: record.clone(),
    };
    for contract in [&callable, &dispatch] {
        let bytes = encode(contract).unwrap();
        let raw: DecodedShapeLinkContractV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&raw).unwrap(), bytes);
        raw.validate_against(contract).unwrap();
    }
    let raw: DecodedShapeLinkContractV1 = decode_canonical(&encode(&callable).unwrap()).unwrap();
    let wrong = ShapeLinkContractV1::CallableAbi {
        canonical_signature: fixture.abi.canonical_signature().clone(),
        calling_convention: fixture.abi.calling_convention(),
        protocol: crate::ExactCallableProtocolV1::OrdinaryNoGc,
    };
    assert!(matches!(
        raw.validate_against(&wrong),
        Err(ShapeLinkError::Contract)
    ));
    let mut changed = encode(&dispatch).unwrap();
    let slot = fixture.slot.as_array();
    let offset = changed.windows(32).position(|part| part == slot).unwrap();
    changed[offset] ^= 1;
    let raw: DecodedShapeLinkContractV1 = decode_canonical(&changed).unwrap();
    assert!(raw.validate_against(&dispatch).is_err());
}
