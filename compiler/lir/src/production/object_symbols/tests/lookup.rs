use std::collections::BTreeSet;

use super::*;

#[test]
fn pruning_foundation_atoms_updates_queries_and_preserves_the_original() {
    let fixture = fixture(true);
    let filtered = fixture.foundation.without_codegen_records(
        &BTreeSet::new(),
        &BTreeSet::new(),
        &BTreeSet::from([fixture.eh_frame.id()]),
    );
    assert_eq!(
        filtered.resolve_definition_atom(fixture.plan.id(), DefinitionAtomRole::EhFrame),
        Err(crate::DefinitionAtomResolutionError::Missing)
    );
    assert_eq!(
        filtered.resolve_definition_atom(fixture.plan.id(), DefinitionAtomRole::Primary),
        Ok((fixture.plan.id(), fixture.primary.id()))
    );
    assert_eq!(
        filtered
            .definition_atoms_for_plan(fixture.plan.id())
            .map(|atom| atom.id())
            .collect::<Vec<_>>(),
        vec![fixture.primary.id()]
    );
    assert_eq!(
        fixture
            .foundation
            .resolve_definition_atom(fixture.plan.id(), DefinitionAtomRole::EhFrame),
        Ok((fixture.plan.id(), fixture.eh_frame.id()))
    );
}

#[test]
fn removing_codegen_records_updates_boundary_queries_without_changing_wire() {
    let fixture = fixture(true);
    let mut surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    let owner = StrongDefinitionEntity::callable_body(fixture.body.id());
    assert_eq!(
        surface.definition(owner, StrongDefinitionRole::CallableBody),
        surface.plan(fixture.plan.id())
    );
    assert_eq!(
        surface.definition(owner, StrongDefinitionRole::CallableRegistration),
        None
    );
    let primary = surface.atom_boundary(fixture.primary.id()).unwrap();
    let absent = atom(&fixture.plan, DefinitionAtomRole::Stackmap).id();
    assert_eq!(surface.atom_boundary(absent), None);

    surface.remove_codegen_records(&BTreeSet::new(), &BTreeSet::from([fixture.eh_frame.id()]));
    assert_eq!(surface.atom_boundary(fixture.eh_frame.id()), None);
    assert_eq!(surface.atom_boundary(fixture.primary.id()), Some(primary));
    assert_eq!(
        surface.definition(owner, StrongDefinitionRole::CallableBody),
        surface.plan(fixture.plan.id())
    );

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
    assert_eq!(
        surface.definition(owner, StrongDefinitionRole::CallableBody),
        None
    );
}
