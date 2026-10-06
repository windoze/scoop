//! Remove actual encoded registrations without embedding artifact fingerprints.

use super::*;
use crate::RegistrationProductionTableV1 as Table;

pub(super) fn reject_incomplete_surface(
    fixture: &Fixture,
    provider: &Fixture,
    surface: &crate::StrongRegistrationProductionSurfaceV2,
) {
    let bytes = encode(surface).unwrap();
    let storage = surface.static_storages().registrations();
    let callables = surface.callables().registrations();
    let units = surface.initialization_units().registrations();
    for missing in [fixture.storage, fixture.failure_root] {
        let index = storage
            .iter()
            .position(|plan| plan.semantic().storage() == missing)
            .unwrap();
        let error = replay(fixture, provider, &remove(&bytes, 6, storage, index)).unwrap_err();
        assert!(matches!(error, Error::TableLength {
            table: Table::StaticStorage, expected, actual
        } if expected == storage.len() && actual + 1 == expected));
    }
    let index = callables
        .iter()
        .position(|plan| plan.body() == fixture.initializer)
        .unwrap();
    assert!(matches!(
        replay(fixture, provider, &remove(&bytes, 3, callables, index)),
        Err(Error::SurfaceMismatch)
    ));
    assert!(matches!(
        replay(fixture, provider, &remove(&bytes, 7, units, 0)),
        Err(Error::TableLength {
            table: Table::InitializationUnit,
            expected: 1,
            actual: 0
        })
    ));
}

fn replay(
    fixture: &Fixture,
    provider: &Fixture,
    bytes: &[u8],
) -> Result<crate::StrongRegistrationProductionSurfaceV2, Error> {
    let producer = fixture.foundation.producer();
    let decoded: crate::DecodedStrongRegistrationProductionSurfaceV2 =
        decode_canonical(bytes).unwrap();
    decoded.replay(
        LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.digests,
        &crate::StrongTypeReferenceDefinitionsV2::new(producer, &[], &[]).unwrap(),
        &Catalog::new(producer, &[definition(provider)]).unwrap(),
    )
}

fn remove<T: scoop_wire::WireEncode>(
    bytes: &[u8],
    field: u8,
    records: &[T],
    index: usize,
) -> Vec<u8> {
    // These fixture tables use the one-byte canonical array length encoding.
    assert!(index < records.len() && records.len() < 24);
    let mut old = vec![field, 0x80 + records.len() as u8];
    let mut new = vec![field, 0x80 + records.len() as u8 - 1];
    for (position, record) in records.iter().enumerate() {
        let encoded = encode(record).unwrap();
        old.extend_from_slice(&encoded);
        if position != index {
            new.extend_from_slice(&encoded);
        }
    }
    let positions: Vec<_> = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == old).then_some(offset))
        .collect();
    let [offset] = positions.as_slice() else {
        panic!("the encoded registration table must occur exactly once");
    };
    let mut corrupted = bytes.to_vec();
    corrupted.splice(*offset..offset + old.len(), new);
    corrupted
}
