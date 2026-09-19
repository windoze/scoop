use super::identity_support::{add_units, set_uses};
use super::*;

#[test]
fn reader_unit_replays_roles_and_unit_type_without_claiming_an_emitted_body() {
    let core_fixture = core::fixture();
    let mut provider = Fixture::new("unit-provider");
    add_units(&mut provider, &core_fixture, &["setting"]);
    let mut consumer = Fixture::new("unit-consumer");
    let unit = provider.source.units[0];
    let relation = MirTypeBridgeDependencyV1::new(
        provider.source.provider,
        MirTypeBridgeTargetV1::InitializationUnit(unit),
    );
    consumer.source.uses = vec![relation];
    let mut graph = graph(&[&core_fixture, &provider, &consumer]);
    let core = core_fixture.section(&[], &graph).unwrap();
    let terminal = provider.section(&[&core], &graph).unwrap();
    let proof = &terminal.initialization_units()[0];
    assert_eq!(
        proof.proof_kind(),
        MirInitializationUnitProofKindV1::ReaderSemanticReplay
    );
    assert_ne!(proof.initializer(), proof.ensure());
    let section = consumer.section(&[&terminal], &graph).unwrap();
    let mut expected = vec![relation, core_fixture.type_use()];
    expected.sort_unstable();
    assert_eq!(section.selected().relations().collect::<Vec<_>>(), expected);
    let reference = section
        .selected()
        .reference(relation.provider(), relation.target())
        .unwrap();
    assert!(
        matches!(section.selected().resolve(reference), Some(MirTypeBridgeSemanticRecordV1::InitializationUnit(record)) if record.unit() == unit)
    );
    let bytes = encode(&terminal).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let replayed = decoded
        .validate(
            provider.authority(),
            &[&core],
            &provider.source,
            &mut graph,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(
        replayed.initialization_units()[0].proof_kind(),
        MirInitializationUnitProofKindV1::ReaderSemanticReplay
    );
}

#[test]
fn selected_unit_follows_its_committed_ensure_edges_but_not_other_units_edges() {
    let core_fixture = core::fixture();
    let mut leaf = Fixture::new("unit-leaf");
    add_units(&mut leaf, &core_fixture, &["one", "two"]);
    let mut middle = Fixture::new("unit-middle");
    add_units(&mut middle, &core_fixture, &["used", "unused"]);
    let mut consumer = Fixture::new("unit-client");
    let graph = graph(&[&core_fixture, &leaf, &middle, &consumer]);
    let uses = middle
        .source
        .units
        .iter()
        .zip(&leaf.source.units)
        .map(|(local, dependency)| {
            SelectedExternalInitializationUseV1::try_new(
                middle.source.provider,
                &graph,
                *local,
                leaf.source.provider,
                *dependency,
                MirExternalInitializationCauseV1::InitializationSupport(*dependency),
                &mut meter(),
            )
            .unwrap()
        })
        .collect();
    set_uses(&mut middle, uses);
    let relation = MirTypeBridgeDependencyV1::new(
        middle.source.provider,
        MirTypeBridgeTargetV1::InitializationUnit(middle.source.units[0]),
    );
    consumer.source.uses = vec![relation];
    let core = core_fixture.section(&[], &graph).unwrap();
    let leaf_section = leaf.section(&[&core], &graph).unwrap();
    let middle_section = middle.section(&[&leaf_section], &graph).unwrap();
    let section = consumer.section(&[&middle_section], &graph).unwrap();
    let mut expected = vec![
        relation,
        core_fixture.type_use(),
        MirTypeBridgeDependencyV1::new(
            leaf.source.provider,
            MirTypeBridgeTargetV1::InitializationUnit(leaf.source.units[0]),
        ),
    ];
    expected.sort_unstable();
    assert_eq!(section.selected().relations().collect::<Vec<_>>(), expected);
}

#[test]
fn unit_inventory_provider_and_full_logical_signature_are_checked() {
    let core_fixture = core::fixture();
    let mut provider = Fixture::new("bad-unit");
    add_units(&mut provider, &core_fixture, &["setting"]);
    let graph = graph(&[&core_fixture, &provider]);
    let core = core_fixture.section(&[], &graph).unwrap();
    let unit = provider.source.units[0];
    provider.source.units.push(unit);
    assert!(matches!(
        provider.section(&[&core], &graph),
        Err(MirTypeBridgeSectionError::NonCanonicalUnitInventory)
    ));
    provider.source.units.pop();
    let signature = &mut provider.source.signatures[0].1;
    *signature =
        MirBridgeCallableSignatureV1::new(signature.exact().clone(), crate::GcEffect::NoGc);
    assert!(matches!(
        provider.section(&[&core], &graph),
        Err(MirTypeBridgeSectionError::Unit {
            problem: MirTypeBridgeUnitProblemV1::Signature,
            ..
        })
    ));
    provider.source.signatures[0].1 = provider.source.signatures[0].2.clone();
    provider.source.signatures[0].2 = MirBridgeCallableSignatureV1::new(
        scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![],
            provider.types.payload.id(),
        ),
        crate::GcEffect::Managed,
    );
    assert!(matches!(
        provider.section(&[&core], &graph),
        Err(MirTypeBridgeSectionError::Unit {
            problem: MirTypeBridgeUnitProblemV1::Signature,
            ..
        })
    ));
    let mut foreign = Fixture::new("foreign-unit-owner");
    foreign.source.units = vec![unit];
    let foreign_graph = support::graph(&[&core_fixture, &provider, &foreign]);
    assert!(matches!(
        foreign.section(&[&core], &foreign_graph),
        Err(MirTypeBridgeSectionError::Unit {
            problem: MirTypeBridgeUnitProblemV1::WrongProvider,
            ..
        })
    ));
}

#[test]
fn initialization_use_cannot_supply_a_missing_local_unit_inventory() {
    let core_fixture = core::fixture();
    let mut provider = Fixture::new("use-target");
    add_units(&mut provider, &core_fixture, &["foreign"]);
    let mut consumer = Fixture::new("use-source");
    add_units(&mut consumer, &core_fixture, &["local"]);
    let graph = graph(&[&core_fixture, &provider, &consumer]);
    let use_record = SelectedExternalInitializationUseV1::try_new(
        consumer.source.provider,
        &graph,
        consumer.source.units[0],
        provider.source.provider,
        provider.source.units[0],
        MirExternalInitializationCauseV1::InitializationSupport(provider.source.units[0]),
        &mut meter(),
    )
    .unwrap();
    set_uses(&mut consumer, vec![use_record]);
    consumer.source.units.clear();
    let core = core_fixture.section(&[], &graph).unwrap();
    let target = provider.section(&[&core], &graph).unwrap();
    assert!(matches!(
        consumer.section(&[&target], &graph),
        Err(MirTypeBridgeSectionError::MissingDependency(
            MirTypeBridgeTargetV1::InitializationUnit(_)
        ))
    ));
}
