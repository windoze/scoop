use super::*;
use scoop_wire::decode_canonical;

fn decode(value: &impl WireEncode) -> DecodedCrossConeLirBridgeSectionV1 {
    decode_canonical(&scoop_wire::encode(value).unwrap()).unwrap()
}

#[test]
fn ordinary_bridge_preserves_both_provider_contracts() {
    for provider in [Fixture::new("ordinary").producer, ConeIdentity::CORE] {
        let fixture = Fixture::for_producer(provider, "exported");
        let expected = CrossConeLirBridgeSectionV1::try_new(
            &fixture.foundation,
            vec![fixture.export()],
            vec![],
        )
        .unwrap();
        assert_eq!(
            decode(&expected)
                .validate(&mut fixture.identities(&[]), &fixture.foundation)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn ordinary_bridge_requires_body_symbol_definition_and_primary_atom() {
    let fixture = Fixture::new("physical");
    for missing in 0..4 {
        let mut canonical = fixture.foundation.as_canonical().clone();
        match missing {
            0 => canonical.set_callable_bodies(vec![]).unwrap(),
            1 => canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![]).unwrap()),
            2 => canonical.set_definition_plans(vec![]).unwrap(),
            3 => canonical.set_definition_atoms(vec![]).unwrap(),
            _ => unreachable!(),
        }
        let foundation = ConeLirFoundation::try_new(fixture.producer, canonical).unwrap();
        let result =
            CrossConeLirBridgeSectionV1::try_new(&foundation, vec![fixture.export()], vec![]);
        assert!(result.is_err(), "missing physical component {missing}");
    }
}

#[test]
fn ordinary_bridge_rejects_duplicate_exports_and_self_selection() {
    let fixture = Fixture::new("repeated");
    assert!(matches!(
        CrossConeLirBridgeSectionV1::try_new(
            &fixture.foundation,
            vec![fixture.export(), fixture.export()],
            vec![],
        ),
        Err(CrossConeLirBridgeBuildError::DuplicateExport(_))
    ));
    let selected = SelectedDependencyLirCallableV1::new(
        fixture.producer,
        fixture.declaration,
        fixture.target,
        fixture.abi.clone(),
        CallingConvention::Cdecl,
        ExternalCallableRootPlan::NoGc,
    )
    .unwrap();
    assert!(matches!(
        CrossConeLirBridgeSectionV1::try_new(&fixture.foundation, vec![], vec![selected],),
        Err(CrossConeLirBridgeBuildError::Relation(
            CrossConeLirBridgeRelationError::SelectedCurrentProvider { .. }
        ))
    ));
}

#[test]
fn ordinary_wire_requires_canonical_selected_order() {
    let fixture = Fixture::new("consumer");
    let provider = Fixture::for_producer(ConeIdentity::CORE, "first");
    let other = Fixture::for_producer(ConeIdentity::SINGLE_FILE, "second");
    let selected = [&provider, &other]
        .into_iter()
        .map(|source| {
            SelectedDependencyLirCallableV1::new(
                source.producer,
                source.declaration,
                source.target,
                source.abi.clone(),
                CallingConvention::Cdecl,
                ExternalCallableRootPlan::NoGc,
            )
            .unwrap()
        })
        .collect();
    let expected =
        CrossConeLirBridgeSectionV1::try_new(&fixture.foundation, vec![fixture.export()], selected)
            .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(fixture.unit).unwrap();
    for source in [&fixture, &provider, &other] {
        pending.register_authority(source.producer).unwrap();
        register_declaration(&mut pending, source.declaration);
    }
    let mut identities = pending.finish().unwrap();
    let mut reversed = expected.clone();
    std::sync::Arc::make_mut(&mut reversed.selected).reverse();
    assert!(
        decode(&reversed)
            .validate(&mut identities, &fixture.foundation)
            .is_err()
    );
    let mut duplicate = expected.clone();
    std::sync::Arc::make_mut(&mut duplicate.selected)[1] = expected.selected[0].clone();
    assert!(
        decode(&duplicate)
            .validate(&mut identities, &fixture.foundation)
            .is_err()
    );
}
