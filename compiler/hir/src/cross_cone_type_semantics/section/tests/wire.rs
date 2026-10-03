use super::*;

#[test]
fn empty_section_has_exactly_four_required_fields() {
    let section = empty();
    let expected = [0xa4, 1, 0x80, 2, 0x80, 3, 0x80, 8, 0x80];
    assert_eq!(encode(&section).unwrap(), expected);
    let mut resolver = PendingIdentityValidation::new().finish().unwrap();
    assert_eq!(
        decoded(&section)
            .resolve(&mut resolver, &WirePath::root())
            .unwrap(),
        section
    );
    for length in 0..expected.len() {
        assert!(
            decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(&expected[..length])
                .is_err()
        );
    }
    for prefix in [0xa3, 0xa5, 0xa8] {
        let mut malformed = expected.to_vec();
        malformed[0] = prefix;
        assert!(decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(&malformed).is_err());
    }
    for retired in [4, 5, 6, 7] {
        let mut malformed = expected;
        malformed[7] = retired;
        assert!(decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(&malformed).is_err());
    }
    let mut duplicate_field = expected;
    duplicate_field[3] = 1;
    assert!(decode_canonical::<DecodedCrossConeTypeSemanticsSectionV1>(&duplicate_field).is_err());
}

#[test]
fn nonempty_tables_preserve_complete_typed_references() {
    let fixture = Fixture::new();
    let section = fixture.section();
    let wire = encode(&section).unwrap();
    let decoded: DecodedCrossConeTypeSemanticsSectionV1 = decode_canonical(&wire).unwrap();
    assert_eq!(encode(&decoded).unwrap(), wire);
    let restored = decoded
        .resolve(&mut fixture.resolver(), &WirePath::root())
        .unwrap();
    assert_eq!(restored, section);
    assert_eq!(restored.exact_facts().records().len(), 1);
    assert_eq!(restored.representation_support().records().len(), 1);
    assert_eq!(restored.inheritance().records().len(), 1);
    assert_eq!(restored.selected().records().len(), 1);
    assert_eq!(encode(&restored).unwrap(), wire);
}

pub(super) fn fields(section: &CrossConeTypeSemanticsSectionV1) -> [Vec<u8>; 4] {
    [
        encode(section.exact_facts()).unwrap(),
        encode(section.representation_support()).unwrap(),
        encode(section.inheritance()).unwrap(),
        encode(section.selected()).unwrap(),
    ]
}
pub(super) fn raw(fields: &[Vec<u8>; 4]) -> DecodedCrossConeTypeSemanticsSectionV1 {
    let mut bytes = vec![0xa4];
    for (index, field) in fields.iter().enumerate() {
        bytes.push([1, 2, 3, 8][index]);
        bytes.extend(field);
    }
    decode_canonical(&bytes).unwrap()
}

#[test]
fn reader_preserves_each_table_order_and_rejects_unresolved_typed_facts() {
    let fixture = Fixture::new();
    let section = fixture.section();
    let mut fields = fields(&section);
    let fact = encode(&section.exact_facts.records()[0]).unwrap();
    fields[0] = [vec![0x82], fact.clone(), fact].concat();
    assert!(matches!(
        raw(&fields).resolve(&mut fixture.resolver(), &WirePath::root()),
        Err(TypeSemanticsSectionResolutionError::Facts(
            ExactTypeFactsTableResolutionError::Order(_)
        ))
    ));
    let mut absent = PendingIdentityValidation::new().finish().unwrap();
    assert!(matches!(
        decoded(&section).resolve(&mut absent, &WirePath::root()),
        Err(TypeSemanticsSectionResolutionError::Facts(
            ExactTypeFactsTableResolutionError::Record {
                index: 0,
                error: ExactTypeFactsResolutionError::Exact(_)
            }
        ))
    ));
}
