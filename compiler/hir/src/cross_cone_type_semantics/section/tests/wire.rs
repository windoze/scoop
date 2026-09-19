use super::*;

#[test]
fn empty_section_has_exactly_eight_required_fields() {
    let section = empty();
    let expected = [
        0xa8, 1, 0x80, 2, 0x80, 3, 0x80, 4, 0x80, 5, 0x80, 6, 0x80, 7, 0x80, 8, 0x80,
    ];
    assert_eq!(
        encode(&section.index_for_wire(&mut meter()).unwrap()).unwrap(),
        expected
    );
    let mut resolver = PendingIdentityValidation::new().finish().unwrap();
    assert_eq!(
        decoded(&section)
            .resolve(&mut resolver, &mut meter(), &WirePath::root())
            .unwrap(),
        section
    );
    for length in 0..expected.len() {
        assert!(
            decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(
                &expected[..length],
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    for prefix in [0xa7, 0xa9] {
        let mut malformed = expected.to_vec();
        malformed[0] = prefix;
        assert!(
            decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(
                &malformed,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    let mut duplicate_field = expected;
    duplicate_field[3] = 1;
    assert!(
        decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(
            &duplicate_field,
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn eight_nonempty_tables_round_trip_with_real_forward_default_indices() {
    let fixture = Fixture::new();
    let section = fixture.section();
    let wire = encode(&section.index_for_wire(&mut meter()).unwrap()).unwrap();
    let decoded: DecodedCrossConeTypeSemanticsSectionV1 =
        decode_canonical(&wire, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), wire);
    let restored = decoded
        .resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root())
        .unwrap();
    assert_eq!(restored, section);
    assert_eq!(restored.exact_facts().records().len(), 1);
    assert_eq!(restored.representation_support().records().len(), 1);
    assert_eq!(restored.inheritance().records().len(), 1);
    assert_eq!(restored.protected_declarations().records().len(), 1);
    assert_eq!(restored.protected_source_interfaces().records().len(), 1);
    assert_eq!(restored.protected_defaults().records().len(), 1);
    assert_eq!(restored.definition_sources().sources().len(), 1);
    assert_eq!(restored.selected().records().len(), 1);
    assert_eq!(
        restored.protected_source_interfaces().records()[0]
            .parameters()
            .parameters()[0]
            .calling()
            .template(),
        Some(fixture.key())
    );
    assert_eq!(
        encode(&restored.index_for_wire(&mut meter()).unwrap()).unwrap(),
        wire
    );
}

pub(super) fn fields(section: &CrossConeTypeSemanticsSectionV1) -> [Vec<u8>; 8] {
    [
        encode(section.exact_facts()).unwrap(),
        encode(section.representation_support()).unwrap(),
        encode(section.inheritance()).unwrap(),
        encode(section.protected_declarations()).unwrap(),
        encode(
            &section
                .protected_source_interfaces()
                .index_templates(section.protected_defaults().keys(), &mut meter())
                .unwrap(),
        )
        .unwrap(),
        encode(&section.protected_defaults().index_locals().unwrap()).unwrap(),
        encode(section.definition_sources()).unwrap(),
        encode(section.selected()).unwrap(),
    ]
}
pub(super) fn raw(fields: &[Vec<u8>; 8]) -> DecodedCrossConeTypeSemanticsSectionV1 {
    let mut bytes = vec![0xa8];
    for (index, field) in fields.iter().enumerate() {
        bytes.push(index as u8 + 1);
        bytes.extend(field);
    }
    decode_canonical(&bytes, DecodeLimits::default()).unwrap()
}

#[test]
fn reader_requires_both_directions_of_source_default_key_closure() {
    let fixture = Fixture::new();
    let section = fixture.section();
    for missing in [4, 5] {
        let mut fields = fields(&section);
        fields[missing] = vec![0x80];
        assert!(matches!(
            raw(&fields).resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root()),
            Err(TypeSemanticsSectionResolutionError::Sources(_))
        ));
    }
    let mut missing = section.clone();
    missing.protected_defaults = CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap();
    assert!(matches!(
        missing.index_for_wire(&mut meter()),
        Err(TypeSemanticsSectionIndexError::Source(_))
    ));
}

#[test]
fn reader_preserves_each_table_order_and_rejects_unresolved_typed_facts() {
    let fixture = Fixture::new();
    let section = fixture.section();
    let mut fields = fields(&section);
    let fact = encode(&section.exact_facts.records()[0]).unwrap();
    fields[0] = [vec![0x82], fact.clone(), fact].concat();
    assert!(matches!(
        raw(&fields).resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root()),
        Err(TypeSemanticsSectionResolutionError::Facts(
            MeteredExactTypeFactsResolutionError::Semantic(
                ExactTypeFactsTableResolutionError::Order(_)
            )
        ))
    ));
    let mut absent = PendingIdentityValidation::new().finish().unwrap();
    assert!(matches!(
        decoded(&section).resolve(&mut absent, &mut meter(), &WirePath::root()),
        Err(TypeSemanticsSectionResolutionError::Facts(
            MeteredExactTypeFactsResolutionError::Semantic(
                ExactTypeFactsTableResolutionError::Record {
                    index: 0,
                    error: ExactTypeFactsResolutionError::Exact(_)
                }
            )
        ))
    ));
}
