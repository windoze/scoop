use super::*;

#[test]
fn source_override_replay_does_not_reintroduce_a_diamond_ancestor() {
    let mut fixture = Fixture::default();
    let root = fixture.add("Root", SourceNominalKind::Interface);
    let left = fixture.add("Left", SourceNominalKind::Interface);
    let right = fixture.add("Right", SourceNominalKind::Interface);
    let join = fixture.add("Join", SourceNominalKind::Interface);
    let old = fixture.function(root, "run");
    let keep = fixture.function(root, "keep");
    let replacement = fixture.function(left, "run");
    let extra = fixture.function(right, "extra");
    fixture.inheritance.edges(left, None, &[root]);
    fixture.inheritance.edges(right, None, &[root]);
    fixture.inheritance.edges(join, None, &[left, right]);
    fixture.interface_source(left, &[root], &[(replacement, &[old])]);
    fixture.interface_source(right, &[root], &[(extra, &[])]);
    fixture.interface_source(join, &[right, left], &[]);
    fixture.set(root, vec![interface(root, &[old, keep])]);
    fixture.set(left, vec![interface(left, &[keep, replacement])]);
    fixture.set(right, vec![interface(right, &[old, keep, extra])]);
    fixture.set(join, vec![interface(join, &[keep, extra, replacement])]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
    )
    .unwrap();
    graph.validate_slot_schemas(join.exact, &fixture).unwrap();
    for slots in [
        vec![old, keep, extra, replacement],
        vec![keep, replacement, extra],
        vec![extra, replacement],
    ] {
        fixture.schemas.insert(
            join.exact,
            CanonicalInheritanceSlotSchemasV1::try_new(vec![interface(join, &slots)]).unwrap(),
        );
        assert!(
            matches!(graph.validate_slot_schemas(join.exact, &fixture), Err(InheritanceSlotSchemaSemanticError::InheritedSlots(owner)) if owner == join.exact)
        );
    }
}

#[test]
fn getter_override_leaves_the_independent_setter_slot() {
    let mut fixture = Fixture::default();
    let root = fixture.add("Root", SourceNominalKind::Interface);
    let child = fixture.add("Child", SourceNominalKind::Interface);
    let getter = fixture.accessor(root, "value", AccessorRole::Getter);
    let setter = fixture.accessor(root, "value", AccessorRole::Setter);
    let new_getter = fixture.accessor(child, "value", AccessorRole::Getter);
    fixture.inheritance.edges(child, None, &[root]);
    fixture.interface_source(child, &[root], &[(new_getter, &[getter])]);
    fixture.set(root, vec![interface(root, &[getter, setter])]);
    fixture.set(child, vec![interface(child, &[setter, new_getter])]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
    )
    .unwrap();
    graph.validate_slot_schemas(child.exact, &fixture).unwrap();
    let wrong = crate::InterfaceSourceDispatchV1::try_new(
        child.exact,
        vec![root.exact],
        vec![crate::InterfaceSourceMemberV1::new(
            new_getter,
            crate::CanonicalPersistentIdsV1::try_new(vec![setter]).unwrap(),
        )],
    )
    .unwrap();
    fixture.interface_sources.insert(child.exact, wrong);
    assert!(
        matches!(graph.validate_slot_schemas(child.exact, &fixture), Err(InheritanceSlotSchemaSemanticError::InvalidOverride { slot, overridden, .. }) if slot == new_getter && overridden == setter)
    );
}

#[test]
fn source_parent_owner_and_override_ancestry_are_required() {
    let mut fixture = Fixture::default();
    let root = fixture.add("Root", SourceNominalKind::Interface);
    let child = fixture.add("Child", SourceNominalKind::Interface);
    let unrelated = fixture.add("Unrelated", SourceNominalKind::Interface);
    let inherited = fixture.function(root, "run");
    let replacement = fixture.function(child, "run");
    let foreign = fixture.function(unrelated, "run");
    fixture.inheritance.edges(child, None, &[root]);
    fixture.set(root, vec![interface(root, &[inherited])]);
    fixture.set(unrelated, vec![interface(unrelated, &[foreign])]);
    fixture.set(child, vec![interface(child, &[replacement])]);
    fixture.interface_source(child, &[root], &[(replacement, &[foreign])]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
    )
    .unwrap();
    assert!(matches!(
        graph.validate_slot_schemas(child.exact, &fixture),
        Err(InheritanceSlotSchemaSemanticError::InvalidOverride { .. })
    ));
    fixture.interface_sources.insert(
        child.exact,
        crate::InterfaceSourceDispatchV1::try_new(child.exact, vec![], vec![]).unwrap(),
    );
    assert!(matches!(
        graph.validate_slot_schemas(child.exact, &fixture),
        Err(InheritanceSlotSchemaSemanticError::InterfaceSource(_))
    ));
    fixture
        .interface_sources
        .insert(child.exact, fixture.interface_sources[&root.exact].clone());
    assert!(matches!(
        graph.validate_slot_schemas(child.exact, &fixture),
        Err(InheritanceSlotSchemaSemanticError::InterfaceSource(_))
    ));
}

#[test]
fn one_member_can_override_two_distinct_parent_declarations() {
    let mut fixture = Fixture::default();
    let left = fixture.add("Left", SourceNominalKind::Interface);
    let right = fixture.add("Right", SourceNominalKind::Interface);
    let child = fixture.add("Child", SourceNominalKind::Interface);
    let a = fixture.function(left, "run");
    let b = fixture.function(right, "run");
    let replacement = fixture.function(child, "run");
    fixture.inheritance.edges(child, None, &[left, right]);
    fixture.interface_source(child, &[right, left], &[(replacement, &[a, b])]);
    fixture.set(left, vec![interface(left, &[a])]);
    fixture.set(right, vec![interface(right, &[b])]);
    fixture.set(child, vec![interface(child, &[replacement])]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture.inheritance,
    )
    .unwrap();
    graph.validate_slot_schemas(child.exact, &fixture).unwrap();
}
