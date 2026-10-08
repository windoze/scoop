use super::*;

#[test]
fn generated_equality_retains_its_nominal_identity_without_a_source_function() {
    let fixture = Fixture::new();
    for (owner, family) in [
        (
            crate::SourceNominalId::Concrete(fixture.enumeration_id),
            Family::Type,
        ),
        (
            crate::SourceNominalId::GenericTemplate(fixture.generic),
            Family::GenericType,
        ),
    ] {
        let value = InheritanceCallableDeclarationV1::DerivedEquality(owner);
        let mut expected = vec![0xa2, 0, 4, 1];
        expected.extend(encode(&owner).unwrap());
        assert_eq!(encode(&value).unwrap(), expected);
        assert_eq!(value.origin(), None);
        let decoded: crate::DecodedInheritanceCallableDeclarationV1 = parsed(&value);
        assert_eq!(encode(&decoded).unwrap(), expected);
        let mut resolver = fixture.resolver();
        assert_eq!(decoded.resolve(&mut resolver).unwrap(), value);
        assert_eq!(resolver.calls, [family]);
    }
}

#[test]
fn all_selected_use_leaves_preserve_the_frozen_wire_and_typed_resolution() {
    let f = Fixture::new();
    let cases = f.cases();
    assert_eq!(cases.len(), 13);
    let mut resolver = f.resolver();

    for (usage, expected_usage) in cases {
        assert_eq!(encode(&usage).unwrap(), expected_usage);
        let decoded_usage: DecodedSelectedTypeUseV1 = parsed(&usage);
        assert_eq!(encode(&decoded_usage).unwrap(), expected_usage);
        assert_eq!(decoded_usage.resolve(&mut resolver).unwrap(), usage);
        let record = f.record(usage);
        let expected = record_wire(f.provider, &expected_usage);
        assert_eq!(encode(&record).unwrap(), expected);
        let decoded: DecodedSelectedExternalTypeUseV1 = parsed(&record);
        assert_eq!(encode(&decoded).unwrap(), expected);
        let resolved = decoded.resolve(&mut resolver).unwrap();
        assert_eq!(resolved, record);
        assert_eq!(resolved.provider(), f.provider);
    }
    for family in [
        Family::Cone,
        Family::Exact,
        Family::Constructor,
        Family::Variant,
        Family::Function,
        Family::Accessor,
        Family::Slot,
        Family::Object,
    ] {
        assert!(resolver.calls.contains(&family), "{family:?}");
    }
}

#[test]
fn construction_and_direct_edge_leaves_keep_their_distinct_tags() {
    let f = Fixture::new();
    for (value, expected) in [
        (
            SelectedTypeConstructionV1::Constructor(f.constructor),
            sum(1, f.constructor.as_array(), None),
        ),
        (
            SelectedTypeConstructionV1::EnumVariant(f.variant),
            sum(2, f.variant.as_array(), None),
        ),
    ] {
        assert_eq!(encode(&value).unwrap(), expected);
        let decoded: DecodedSelectedTypeConstructionV1 = parsed(&value);
        assert_eq!(encode(&decoded).unwrap(), expected);
    }
    for (edge, exact, tag) in [
        (
            SelectedDirectInheritanceEdgeV1::ClassBase { exact: f.owner },
            f.owner,
            1,
        ),
        (
            SelectedDirectInheritanceEdgeV1::Interface { exact: f.interface },
            f.interface,
            2,
        ),
    ] {
        assert_eq!(edge.exact(), exact);
        assert_eq!(encode(&edge).unwrap(), sum(tag, exact.as_array(), None));
        let decoded: DecodedSelectedDirectInheritanceEdgeV1 = parsed(&edge);
        assert_eq!(encode(&decoded).unwrap(), encode(&edge).unwrap());
        let usage = SelectedTypeUseV1::Inheritance {
            derived: f.derived,
            edge,
        };
        assert_eq!(usage.exact(), exact);
        assert_ne!(usage.exact(), f.derived);
    }
}

#[test]
fn empty_selected_table_is_an_empty_sequence_without_resolution() {
    let f = Fixture::new();
    let empty = CanonicalSelectedExternalTypeUsesV1::try_new(vec![]).unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    let mut resolver = f.resolver();
    let decoded: DecodedCanonicalSelectedExternalTypeUsesV1 = parsed(&empty);
    assert_eq!(decoded.resolve(&mut resolver, &path()).unwrap(), empty);
    assert!(resolver.calls.is_empty());
}
