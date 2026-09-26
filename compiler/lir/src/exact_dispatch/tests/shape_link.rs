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

#[test]
fn shape_link_legacy_query_rejects_explicit_old_callable_but_allows_new_dispatch() {
    use crate::*;
    use scoop_identity::{
        ConeIdentity, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
    };
    let fixture = DirectFixture::new(1);
    let StrongCallableDefinitionOwner::Function(function) = fixture.target else {
        panic!("source function fixture");
    };
    let export = ParamFreeLirCallableExportV1::new(
        ConeIdentity::SINGLE_FILE,
        DependencyCallableDeclarationId::Function(function),
        fixture.target,
        fixture.abi.canonical_signature().clone(),
        fixture.abi.calling_convention(),
        ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap();
    let ordinary =
        CrossConeLirBridgeSectionV1::try_new(&fixture.foundation, vec![export], Vec::new())
            .unwrap();
    let callable = ExternalStrongShapeSubjectV1::Callable(fixture.target);
    assert!(
        matches!(ShapeLinkProviderV1::reject_legacy_subject(&ordinary, callable), Err(ShapeLinkError::LegacyPartition(actual)) if actual == callable)
    );
    ShapeLinkProviderV1::reject_legacy_subject(
        &ordinary,
        ExternalStrongShapeSubjectV1::DispatchTable(fixture.vtable.identity_record().id()),
    )
    .unwrap();
}
