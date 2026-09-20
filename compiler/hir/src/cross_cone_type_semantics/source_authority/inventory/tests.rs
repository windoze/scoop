use super::*;
use crate::*;
use scoop_identity::*;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod support;
use support::*;
mod rejection;

fn decoded<T: scoop_wire::WireDecode + scoop_wire::WireEncode>(
    value: &impl scoop_wire::WireEncode,
) -> T {
    let bytes = encode(value).unwrap();
    let restored = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    restored
}

#[test]
fn empty_inventories_have_fixed_wire_vectors() {
    assert_eq!(
        encode(&CanonicalSourceNominalIdsV1::default()).unwrap(),
        [0x80]
    );
    assert_eq!(
        encode(&CanonicalTypeSourceNominalsV1::default()).unwrap(),
        [0x80]
    );
    assert_eq!(
        encode(&CanonicalTypeSectionDependencyFactsV1::default()).unwrap(),
        [0x80]
    );
    assert_eq!(
        encode(&CanonicalNominalInheritanceEdgesV1::default()).unwrap(),
        [0x80]
    );
}

#[test]
fn concrete_and_generic_source_roots_follow_canonical_bytes() {
    let mut resolver = Resolver::new();
    let mut values = resolver.roots();
    values.reverse();
    let table = CanonicalSourceNominalIdsV1::try_new(values, &mut meter()).unwrap();
    let bytes = table
        .values()
        .iter()
        .map(|id| encode(id).unwrap())
        .collect::<Vec<_>>();
    assert!(bytes.windows(2).all(|pair| pair[0] < pair[1]));
    let restored = decoded::<DecodedCanonicalSourceNominalIdsV1>(&table)
        .resolve(&mut resolver, &mut meter())
        .unwrap();
    assert_eq!(table, restored);
}

#[test]
fn nominal_snapshots_retain_access_lexical_order_and_origin() {
    let mut resolver = Resolver::new();
    let record = resolver.snapshot();
    let expected = [
        vec![0xa2, 1],
        encode(&record.owner()).unwrap(),
        vec![2],
        encode(record.access()).unwrap(),
    ]
    .concat();
    assert_eq!(encode(&record).unwrap(), expected);
    let table = CanonicalTypeSourceNominalsV1::try_new(vec![record.clone()], &mut meter()).unwrap();
    let restored = decoded::<DecodedCanonicalTypeSourceNominalsV1>(&table)
        .resolve(&mut resolver, &mut meter())
        .unwrap();
    assert_eq!(restored, table);
    assert_eq!(restored.get(record.owner()), Some(&record));
    assert_eq!(
        restored.records()[0].access().lexical_owners(),
        &[resolver.roots()[0]]
    );
}

#[test]
fn dependency_inventory_orders_by_exact_and_retains_provider() {
    let mut resolver = Resolver::new();
    let records = resolver.dependencies();
    let record = &records[0];
    assert_eq!(
        encode(record).unwrap(),
        [
            vec![0xa2, 1],
            encode(&record.provider).unwrap(),
            vec![2],
            encode(&record.exact).unwrap()
        ]
        .concat()
    );
    let table = CanonicalTypeSectionDependencyFactsV1::try_new(records, &mut meter()).unwrap();
    let restored = decoded::<DecodedCanonicalTypeSectionDependencyFactsV1>(&table)
        .resolve(&mut resolver, &mut meter())
        .unwrap();
    assert_eq!(restored, table);
    assert!(
        restored
            .records()
            .windows(2)
            .all(|pair| pair[0].exact < pair[1].exact)
    );
}

#[test]
fn edge_inventory_reuses_existing_four_field_wire() {
    let mut resolver = Resolver::new();
    let edge = resolver.edge();
    let table =
        CanonicalNominalInheritanceEdgesV1::try_new(vec![edge.clone()], &mut meter()).unwrap();
    assert_eq!(
        encode(&table).unwrap(),
        [vec![0x81], encode(&edge).unwrap()].concat()
    );
    let restored = decoded::<DecodedCanonicalNominalInheritanceEdgesV1>(&table)
        .resolve(&mut resolver, &mut meter())
        .unwrap();
    assert_eq!(restored, table);
}
