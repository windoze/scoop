use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn builtin_only_sections_round_trip_without_adding_source_or_selected_tags() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let mut requests = Vec::new();
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let nominal = builtin.identity_record();
        let exact = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(
            ExactTypeKey::Nominal(nominal.id()),
        )
        .unwrap();
        pending
            .register_external_canonical_authority(nominal)
            .unwrap();
        pending
            .register_external_canonical_authority(exact)
            .unwrap();
        requests.push(request(provider.builtin(builtin)));
    }
    let mut identities = pending.finish().unwrap();
    let public = public();
    let original = provider.section(vec![]);
    let provider_wire = round_trip(&original, &mut identities);
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let original = consumer.section(requests.clone());
    let consumer_wire = round_trip(&original, &mut identities);
    let checked = check(
        &consumer,
        &consumer_wire,
        &public,
        &[&terminal],
        &Uses::new(&requests),
    )
    .unwrap();
    assert_eq!(checked.selected().len(), 2);
    assert!(checked.selected().iter().all(|selected| matches!(
        selected.target().definition(),
        CheckedTypeSelectionDefinitionV1::LanguageBuiltin(_)
    )));
}

fn round_trip(
    section: &CrossConeTypeSemanticsSectionV1,
    identities: &mut ValidatedIdentityGraph,
) -> CrossConeTypeSemanticsSectionV1 {
    let bytes = encode(&section.index_for_wire(&mut meter()).unwrap()).unwrap();
    assert_eq!(bytes[0], 0xa8);
    let decoded: DecodedCrossConeTypeSemanticsSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let restored = decoded.resolve(identities, &mut meter(), &path()).unwrap();
    assert_eq!(&restored, section);
    restored
}
