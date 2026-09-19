use super::*;

const USE: usize = 37;
const NESTED: usize = USE + 39;

#[test]
fn unknown_use_construction_member_and_edge_tags_are_rejected() {
    let f = Fixture::new();
    for (case, offset, tag, at) in [
        (0, USE + 2, 10, WirePath::root().field(2)),
        (2, NESTED + 2, 3, WirePath::root().field(2).field(2)),
        (4, NESTED + 2, 4, WirePath::root().field(2).field(2)),
        (10, NESTED + 2, 0, WirePath::root().field(2).field(2)),
    ] {
        let mut bytes = record_wire(f.provider, &f.cases()[case].1);
        bytes[offset] = tag;
        let error =
            decode_canonical::<DecodedSelectedExternalTypeUseV1>(&bytes, DecodeLimits::default())
                .unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::UnknownTag {
                tag: u64::from(tag)
            }
        );
        assert_eq!(error.path(), &at);
        assert!(error.byte_offset().is_some());
    }
}

#[test]
fn malformed_products_fields_and_id_lengths_are_rejected() {
    let f = Fixture::new();
    let valid = record_wire(f.provider, &f.cases()[2].1);
    assert_eq!(&valid[USE..USE + 4], &[0xa3, 0, 3, 1]);
    assert_eq!(&valid[NESTED..NESTED + 4], &[0xa2, 0, 1, 1]);
    for (offset, replacement) in [
        (0, 0xa1),
        (0, 0xa3),
        (1, 2),
        (36, 1),
        (3, 31),
        (USE, 0xa2),
        (USE + 1, 1),
        (USE + 3, 2),
        (NESTED, 0xa1),
        (NESTED + 1, 1),
        (NESTED + 5, 31),
    ] {
        let mut bytes = valid.clone();
        bytes[offset] = replacement;
        assert!(
            decode_canonical::<DecodedSelectedExternalTypeUseV1>(&bytes, DecodeLimits::default())
                .is_err(),
            "offset {offset}"
        );
    }
    for length in [0, 1, USE, valid.len() - 1] {
        assert!(
            decode_canonical::<DecodedSelectedExternalTypeUseV1>(
                &valid[..length],
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(matches!(
        decode_canonical::<DecodedSelectedExternalTypeUseV1>(&trailing, DecodeLimits::default())
            .unwrap_err()
            .kind(),
        WireErrorKind::TrailingData
    ));
    let mut nonminimal = valid;
    nonminimal.splice(USE + 2..USE + 3, [0x18, 3]);
    assert!(matches!(
        decode_canonical::<DecodedSelectedExternalTypeUseV1>(&nonminimal, DecodeLimits::default())
            .unwrap_err()
            .kind(),
        WireErrorKind::NonCanonicalCbor
    ));
}

#[test]
fn wire_ids_must_resolve_in_the_correct_persistent_family() {
    let f = Fixture::new();
    for (case, start, wrong, family) in [
        (0, 4, f.owner.as_array(), Family::Cone),
        (0, USE + 6, f.provider.as_array(), Family::Exact),
        (2, NESTED + 6, f.function.as_array(), Family::Constructor),
        (3, NESTED + 6, f.constructor.as_array(), Family::Variant),
        (4, NESTED + 6, f.constructor.as_array(), Family::Function),
        (5, NESTED + 6, f.function.as_array(), Family::Accessor),
        (6, NESTED + 6, f.function.as_array(), Family::Accessor),
        (7, USE + 41, f.function.as_array(), Family::Slot),
        (9, USE + 41, f.slot.as_array(), Family::Object),
        (10, NESTED + 6, f.constructor.as_array(), Family::Exact),
        (11, NESTED + 6, f.constructor.as_array(), Family::Exact),
    ] {
        let mut bytes = record_wire(f.provider, &f.cases()[case].1);
        bytes[start..start + 32].copy_from_slice(wrong);
        let decoded: DecodedSelectedExternalTypeUseV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let mut resolver = f.resolver();
        assert!(
            matches!(decoded.resolve(&mut resolver, &mut meter(), &path()), Err(SelectedTypeUseResolutionError::Reference(actual)) if actual == family)
        );
        assert_eq!(resolver.calls.last(), Some(&family));
    }
}
