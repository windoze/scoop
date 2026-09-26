use super::*;

#[test]
fn seven_field_wire_round_trips_only_after_full_closure_replay() {
    let provider = Fixture::new("wire-provider");
    let mut consumer = Fixture::new("wire-consumer");
    consumer.uses = vec![provider.shape_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer
        .section(&[terminal.dependency_view()], &graph)
        .unwrap();
    let bytes = encode(&section).unwrap();
    assert_eq!(bytes[0], 0xa7);
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let dependencies = [terminal.dependency_view()];
    let replayed =
        resolved_dependencies::read(&consumer, decoded, &dependencies, &mut graph).unwrap();
    replayed
        .replay_dependency_closure(&consumer.units, &dependencies, &consumer.uses, &graph)
        .unwrap();
    assert_eq!(
        replayed.selected_relations(),
        section.selected().relations().collect::<Vec<_>>()
    );
}

fn decoded_with_selected(
    section: &CrossConeMirTypeBridgeSectionV1<'_>,
    selected: &[MirTypeBridgeDependencyV1],
) -> DecodedCrossConeMirTypeBridgeSectionV1 {
    struct Records<'a, 'b>(
        &'a CrossConeMirTypeBridgeSectionV1<'b>,
        &'a [MirTypeBridgeDependencyV1],
    );
    impl WireEncode for Records<'_, '_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(7)?;
            encoder.field(1)?;
            self.0.types().encode(encoder)?;
            encoder.field(2)?;
            self.0.callables().encode(encoder)?;
            encoder.field(3)?;
            self.0.dispatch().encode(encoder)?;
            encoder.field(4)?;
            self.0.object_values().encode(encoder)?;
            encoder.field(5)?;
            self.0.shape_support().encode(encoder)?;
            encoder.field(6)?;
            self.0.initialization_uses().encode(encoder)?;
            encoder.field(7)?;
            sequence(encoder, self.1)
        }
    }
    decode_canonical(&encode(&Records(section, selected)).unwrap()).unwrap()
}

#[test]
fn reader_rejects_missing_extra_duplicate_and_local_selected_records() {
    let provider = Fixture::new("selected-provider");
    let mut consumer = Fixture::new("selected-consumer");
    consumer.uses = vec![provider.type_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer
        .section(&[terminal.dependency_view()], &graph)
        .unwrap();
    let dependencies = [terminal.dependency_view()];
    for records in [vec![], vec![provider.type_use(), provider.shape_use()]] {
        let decoded = decoded_with_selected(&section, &records);
        let read =
            resolved_dependencies::read(&consumer, decoded, &dependencies, &mut graph).unwrap();
        assert!(matches!(
            read.replay_dependency_closure(&consumer.units, &dependencies, &consumer.uses, &graph),
            Err(MirTypeBridgeSectionError::SelectedClosure)
        ));
    }
    let duplicate = decoded_with_selected(&section, &[provider.type_use(), provider.type_use()]);
    assert!(matches!(
        resolved_dependencies::read(&consumer, duplicate, &dependencies, &mut graph),
        Err(MirTypeBridgeSectionError::NonCanonicalSelected { .. })
    ));
    let local = decoded_with_selected(&section, &[consumer.type_use()]);
    assert!(matches!(
        resolved_dependencies::read(&consumer, local, &dependencies, &mut graph),
        Err(MirTypeBridgeSectionError::SelectedCurrentProvider)
    ));
}

#[test]
fn wire_requires_all_seven_fields_and_their_registered_tags() {
    let fixture = Fixture::new("wire-closed");
    let section = fixture.section(&[], &fixture.types.graph).unwrap();
    let mut bytes = encode(&section).unwrap();
    bytes[0] = 0xa6;
    assert!(decode_canonical::<DecodedCrossConeMirTypeBridgeSectionV1>(&bytes).is_err());
    let mut bytes = encode(&section).unwrap();
    let last = bytes.len() - 2;
    assert_eq!(bytes[last], 7);
    bytes[last] = 8;
    assert!(decode_canonical::<DecodedCrossConeMirTypeBridgeSectionV1>(&bytes).is_err());
}
