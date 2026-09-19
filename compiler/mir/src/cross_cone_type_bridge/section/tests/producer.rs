use super::identity_support::add_units;
use super::*;

#[test]
fn producer_unit_contract_retains_the_actual_sealed_body_roots() {
    let input = crate::strong_input::initialization_test_input();
    let core = core::fixture();
    let mut fixture = Fixture::in_provider(ConeIdentity::SINGLE_FILE);
    add_units(&mut fixture, &core, &["setting"]);
    let graph = graph(&[&core, &fixture]);
    let ordinary = crate::CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        input.foundation(),
        vec![],
        vec![],
    )
    .unwrap();
    let authority = MirTypeBridgeLocalAuthorityV1::Producer {
        provider: ConeIdentity::SINGLE_FILE,
        input: &input,
        ordinary: &ordinary,
    };
    authority.validate::<&'static str>(&mut meter()).unwrap();
    let units = super::super::units::build(
        authority,
        &fixture.source,
        &graph,
        core.source.expected.types(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(units.len(), 1);
    let root = input.materialization().initialization_roots()[0];
    assert_eq!(units[0].unit(), root.identity());
    assert_eq!(
        units[0].proof_kind(),
        MirInitializationUnitProofKindV1::ProducerEmitted(root)
    );
    fixture.source.units.clear();
    assert!(matches!(
        super::super::units::build(
            authority,
            &fixture.source,
            &graph,
            core.source.expected.types(),
            &mut meter()
        ),
        Err(MirTypeBridgeSectionError::ProviderContext)
    ));
}

#[test]
fn producer_authority_cannot_relabel_another_cones_sealed_module() {
    let input = crate::strong_input::initialization_test_input();
    let fixture = Fixture::new("wrong-producer");
    let authority = MirTypeBridgeLocalAuthorityV1::Producer {
        provider: fixture.source.provider,
        input: &input,
        ordinary: &fixture.ordinary,
    };
    assert!(matches!(
        authority.validate::<&'static str>(&mut meter()),
        Err(MirTypeBridgeSectionError::ProviderContext)
    ));
}
