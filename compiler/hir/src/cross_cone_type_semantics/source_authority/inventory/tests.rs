use super::*;
use crate::*;
use scoop_identity::*;
use scoop_wire::{decode_canonical, encode};

mod support;
use support::*;
mod rejection;

fn decoded<T: scoop_wire::WireDecode + scoop_wire::WireEncode>(
    value: &impl scoop_wire::WireEncode,
) -> T {
    let bytes = encode(value).unwrap();
    let restored = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    restored
}

#[test]
fn empty_inventories_have_fixed_wire_vectors() {
    assert_eq!(
        encode(&CanonicalSourceNominalIdsV1::default()).unwrap(),
        [0x80]
    );
}

#[test]
fn concrete_and_generic_source_roots_follow_canonical_bytes() {
    let mut resolver = Resolver::new();
    let mut values = resolver.roots();
    values.reverse();
    let table = CanonicalSourceNominalIdsV1::try_new(values).unwrap();
    let bytes = table
        .values()
        .iter()
        .map(|id| encode(id).unwrap())
        .collect::<Vec<_>>();
    assert!(bytes.windows(2).all(|pair| pair[0] < pair[1]));
    let restored = decoded::<DecodedCanonicalSourceNominalIdsV1>(&table)
        .resolve(&mut resolver)
        .unwrap();
    assert_eq!(table, restored);
}
