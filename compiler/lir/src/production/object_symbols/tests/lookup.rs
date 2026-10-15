use std::collections::BTreeSet;

use super::*;

#[test]
fn removing_codegen_records_updates_boundary_queries_without_changing_wire() {
    let fixture = fixture(true);
    let mut surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    let primary = surface.atom_boundary(fixture.primary.id()).unwrap();
    let absent = atom(&fixture.plan, DefinitionAtomRole::Stackmap).id();
    assert_eq!(surface.atom_boundary(absent), None);

    surface.remove_codegen_records(&BTreeSet::new(), &BTreeSet::from([fixture.eh_frame.id()]));
    assert_eq!(surface.atom_boundary(fixture.eh_frame.id()), None);
    assert_eq!(surface.atom_boundary(fixture.primary.id()), Some(primary));

    let mut canonical = fixture.foundation.as_canonical().clone();
    canonical
        .set_definition_atoms(vec![fixture.primary.clone()])
        .unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    let expected = ObjectSymbolSurfaceV1::from_foundation(&foundation).unwrap();
    assert_eq!(encode(&surface).unwrap(), encode(&expected).unwrap());
    assert_eq!(surface, expected);
    let decoded: DecodedObjectSymbolSurfaceV1 =
        decode_canonical(&encode(&surface).unwrap()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), encode(&surface).unwrap());

    surface.remove_codegen_records(&BTreeSet::from([fixture.plan.id()]), &BTreeSet::new());
    assert!(surface.plans().is_empty());
    assert_eq!(surface.atom_boundary(fixture.primary.id()), None);
}
