use super::*;
use crate::DefinitionAtomResolutionError;

#[test]
fn definition_atom_lookup_retains_missing_and_ambiguous_results() {
    let records = records();
    let plan = records.plan.id();
    let primary = primary_atom(&records);
    let additional = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::CallableBody(records.body.id()),
    ))
    .unwrap();
    for (atoms, expected) in [
        (
            vec![associated_atom(&records)],
            Err(DefinitionAtomResolutionError::Missing),
        ),
        (vec![primary.clone()], Ok((plan, primary.id()))),
        (
            vec![primary, additional],
            Err(DefinitionAtomResolutionError::Ambiguous),
        ),
    ] {
        let canonical = foundation(&records, atoms);
        let expected_bytes = encode(&canonical).unwrap();
        let indexed = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
        assert_eq!(
            indexed.resolve_definition_atom(plan, DefinitionAtomRole::Primary),
            expected
        );
        assert_eq!(
            indexed
                .clone()
                .resolve_definition_atom(plan, DefinitionAtomRole::Primary),
            expected
        );
        assert_eq!(encode(&indexed).unwrap(), expected_bytes);
        assert_eq!(
            indexed.resolve_definition_atom(plan, DefinitionAtomRole::Stackmap),
            Err(DefinitionAtomResolutionError::Missing)
        );
    }
}

#[test]
fn decoded_definition_atom_lookup_preserves_plan_membership_and_order() {
    let (_, _, indexed) = fixture();
    let records = records();
    let plan = records.plan.id();
    let primary = primary_atom(&records);
    let mut expected = vec![primary.id(), associated_atom(&records).id()];
    expected.sort_unstable();
    assert_eq!(
        indexed.resolve_definition_atom(plan, DefinitionAtomRole::Primary),
        Ok((plan, primary.id()))
    );
    assert_eq!(
        indexed
            .definition_atoms_for_plan(plan)
            .map(|atom| atom.id())
            .collect::<Vec<_>>(),
        expected
    );

    let other = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(records.body.id()),
            StrongDefinitionRole::CallableRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let other_plan = other.id();
    assert!(
        indexed
            .definition_atoms_for_plan(other_plan)
            .next()
            .is_none()
    );
    assert_eq!(
        indexed.resolve_definition_atom(other_plan, DefinitionAtomRole::Primary),
        Err(DefinitionAtomResolutionError::Missing)
    );

    let other_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        other_plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let other_target = other_atom.id();
    let mut atoms = indexed.definition_atoms().to_vec();
    atoms.push(other_atom);
    let mut canonical = indexed.clone().into_canonical();
    canonical
        .set_definition_plans(vec![records.plan, other])
        .unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    let extended = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    assert_eq!(
        extended.resolve_definition_atom(plan, DefinitionAtomRole::Primary),
        Ok((plan, primary.id()))
    );
    assert_eq!(
        extended.resolve_definition_atom(other_plan, DefinitionAtomRole::Primary),
        Ok((other_plan, other_target))
    );
    assert!(
        indexed
            .definition_atoms_for_plan(other_plan)
            .next()
            .is_none()
    );
}
