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
        &mut meter(),
    )
    .unwrap();
    let callable = ShapeLinkContractV1::CallableAbi {
        canonical_signature: fixture.abi.canonical_signature(),
        calling_convention: fixture.abi.calling_convention(),
        protocol: fixture.abi.call_protocol(),
    };
    let dispatch = ShapeLinkContractV1::Dispatch {
        table_projection: &record,
    };
    for contract in [callable, dispatch] {
        let bytes = encode(&contract).unwrap();
        let raw: DecodedShapeLinkContractV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&raw).unwrap(), bytes);
        raw.validate_against(&contract, &mut meter()).unwrap();
    }
    let raw: DecodedShapeLinkContractV1 =
        decode_canonical(&encode(&callable).unwrap(), DecodeLimits::default()).unwrap();
    let wrong = ShapeLinkContractV1::CallableAbi {
        canonical_signature: fixture.abi.canonical_signature(),
        calling_convention: fixture.abi.calling_convention(),
        protocol: crate::ExactCallableProtocolV1::OrdinaryNoGc,
    };
    assert!(matches!(
        raw.validate_against(&wrong, &mut meter()),
        Err(ShapeLinkError::Contract)
    ));
    let mut changed = encode(&dispatch).unwrap();
    let slot = fixture.slot.as_array();
    let offset = changed.windows(32).position(|part| part == slot).unwrap();
    changed[offset] ^= 1;
    let raw: DecodedShapeLinkContractV1 =
        decode_canonical(&changed, DecodeLimits::default()).unwrap();
    assert!(raw.validate_against(&dispatch, &mut meter()).is_err());
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
        matches!(ShapeLinkProviderV1::reject_legacy_subject(&ordinary, &CoreLirBridgeBranchV1::NotCore, callable, &mut meter()), Err(ShapeLinkError::LegacyPartition(actual)) if actual == callable)
    );
    ShapeLinkProviderV1::reject_legacy_subject(
        &ordinary,
        &CoreLirBridgeBranchV1::NotCore,
        ExternalStrongShapeSubjectV1::DispatchTable(fixture.vtable.identity_record().id()),
        &mut meter(),
    )
    .unwrap();
    let empty =
        CrossConeLirBridgeSectionV1::try_new(&fixture.foundation, Vec::new(), Vec::new()).unwrap();
    let thrower = CallableAbiRecordV1::new(
        scoop_identity::ConeIdentity::CORE,
        fixture.target,
        fixture.abi.canonical_signature().clone(),
        fixture.abi.calling_convention(),
        ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap();
    let core = CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(thrower));
    assert!(matches!(
        ShapeLinkProviderV1::reject_legacy_subject(&empty, &core, callable, &mut meter()),
        Err(ShapeLinkError::LegacyPartition(_))
    ));
}
