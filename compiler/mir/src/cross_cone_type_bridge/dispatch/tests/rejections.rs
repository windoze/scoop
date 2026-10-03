use super::*;

#[test]
fn interface_providers_preserve_the_relative_order_of_retained_parent_slots() {
    let fixture = Fixture::new();
    for owner in [LEFT, DIAMOND] {
        let mut records = fixture.table().records;
        let record = records
            .iter_mut()
            .find(|record| record.owner() == fixture.exact(owner))
            .unwrap();
        let MirDispatchSlotsV1::InterfaceSlots(entries) = &mut record.slots else {
            panic!("an interface carries slot contracts");
        };
        entries.reverse();
        for (position, entry) in entries.iter_mut().enumerate() {
            entry.position = MirDispatchPositionV1::new(position as u32);
        }
        assert!(matches!(
            CanonicalMirDispatchSchemasV1::try_new(fixture.authority(), records),
            Err(MirDispatchSchemaError::InterfaceOrder { .. })
        ));
    }
}

#[test]
fn direct_receiver_tags_cannot_hide_wrong_or_unrelated_receiver() {
    let fixture = Fixture::new();
    let mut derived = fixture.record(DERIVED);
    let MirDispatchSlotsV1::ClassVtable(entries) = &mut derived.slots else {
        unreachable!()
    };
    entries[0].implementation = MirDispatchImplementationV1::DirectStrongTarget {
        target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target(1)),
        receiver: MirDispatchReceiverAdaptationV1::Identity,
    };
    assert!(matches!(
        fixture.check(&derived),
        Err(MirDispatchSchemaError::TargetSignature { .. })
    ));
    let MirDispatchSlotsV1::ClassVtable(entries) = &mut derived.slots else {
        unreachable!()
    };
    entries[0].implementation = MirDispatchImplementationV1::DirectStrongTarget {
        target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target(2)),
        receiver: MirDispatchReceiverAdaptationV1::ReferenceDispatch,
    };
    assert!(matches!(
        fixture.check(&derived),
        Err(MirDispatchSchemaError::MissingReceiverPath { .. })
    ));
    let mut base = fixture.record(BASE);
    let MirDispatchSlotsV1::ClassVtable(entries) = &mut base.slots else {
        unreachable!()
    };
    entries[0].implementation = MirDispatchImplementationV1::DirectStrongTarget {
        target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target(0)),
        receiver: MirDispatchReceiverAdaptationV1::ReferenceDispatch,
    };
    assert!(matches!(
        fixture.check(&base),
        Err(MirDispatchSchemaError::TargetSignature { .. })
    ));
}

#[test]
fn value_interface_default_cannot_bypass_boxing_adjust_or_inherit_wrong_interface() {
    let mut fixture = Fixture::new();
    let mut value = fixture.record(VALUE);
    let entry = value.itables[0]
        .entries
        .iter_mut()
        .find(|entry| entry.slot() == fixture.slots[2].id())
        .unwrap();
    entry.implementation = MirDispatchImplementationV1::InterfaceDefaultTarget {
        target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target(4)),
        receiver: MirDispatchReceiverAdaptationV1::Identity,
    };
    assert!(matches!(
        fixture.check(&value),
        Err(MirDispatchSchemaError::InvalidImplementation { .. })
    ));
    let value = fixture.record(VALUE);
    fixture.set_edges(VALUE, None, &[]);
    assert!(matches!(
        fixture.check(&value),
        Err(MirDispatchSchemaError::MissingReceiverPath { .. })
    ));
}

#[test]
fn slot_positions_duplicates_and_concrete_abstract_obligations_are_rejected() {
    let fixture = Fixture::new();
    let mut root = fixture.record(ROOT);
    let MirDispatchSlotsV1::InterfaceSlots(slots) = &mut root.slots else {
        panic!("an interface carries slot contracts");
    };
    slots[1].position = MirDispatchPositionV1::new(0);
    assert!(matches!(
        fixture.check(&root),
        Err(MirDispatchSchemaError::Position { index: 1 })
    ));
    let MirDispatchSlotsV1::InterfaceSlots(slots) = &mut root.slots else {
        panic!("an interface carries slot contracts");
    };
    slots[1] = slots[0].clone();
    slots[1].position = MirDispatchPositionV1::new(1);
    assert!(matches!(
        fixture.check(&root),
        Err(MirDispatchSchemaError::DuplicateSlot { .. })
    ));
    let mut derived = fixture.record(DERIVED);
    let interface = derived.itables[0].interface();
    let entry = derived.itables[0]
        .entries
        .iter_mut()
        .find(|entry| entry.slot() == fixture.slots[1].id())
        .unwrap();
    entry.implementation = MirDispatchImplementationV1::AbstractObligation {
        declaration: DispatchDeclarationOwner::Function(fixture.methods[3].id()),
        trap_target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target(3)),
        receiver: if interface == fixture.exact(ROOT) {
            MirDispatchReceiverAdaptationV1::Identity
        } else {
            MirDispatchReceiverAdaptationV1::ReferenceDispatch
        },
    };
    assert!(matches!(
        fixture.check(&derived),
        Err(MirDispatchSchemaError::ConcreteObligation { .. })
    ));
}

#[test]
fn cross_record_prefix_provider_order_and_closure_are_checked() {
    let fixture = Fixture::new();
    for change in 0..4 {
        let mut records = fixture.table().records;
        let derived = records
            .iter_mut()
            .find(|record| record.owner() == fixture.exact(DERIVED))
            .unwrap();
        match change {
            0 => derived.slots = MirDispatchSlotsV1::ClassVtable(vec![]),
            1 => {
                derived.itables[0].entries.reverse();
                for (position, entry) in derived.itables[0].entries.iter_mut().enumerate() {
                    entry.contract.position = MirDispatchPositionV1::new(position as u32);
                }
            }
            2 => {
                derived.itables.pop();
            }
            _ => {
                records.retain(|record| record.owner() != fixture.exact(ROOT));
            }
        }
        let error =
            CanonicalMirDispatchSchemasV1::try_new(fixture.authority(), records).unwrap_err();
        assert!(matches!(
            (change, error),
            (0, MirDispatchSchemaError::BasePrefix { .. })
                | (1, MirDispatchSchemaError::InterfaceOrder { .. })
                | (2, MirDispatchSchemaError::InterfaceClosure { .. })
                | (3, MirDispatchSchemaError::MissingSchema { .. })
        ));
    }
}

#[test]
fn canonical_tables_reject_inheritance_cycles_and_duplicate_owners() {
    let mut fixture = Fixture::new();
    let mut records = fixture.table().records;
    records.push(records[0].clone());
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new(fixture.authority(), records),
        Err(MirDispatchSchemaError::DuplicateOwner { .. })
    ));
    let records = fixture.table().records;
    fixture.set_edges(ROOT, None, &[DIAMOND]);
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new(fixture.authority(), records),
        Err(MirDispatchSchemaError::InheritanceCycle { .. })
    ));
}

#[test]
fn slot_signature_checks_every_axis_and_adjust_checks_actual_target_gc() {
    let mut fixture = Fixture::new();
    for axis in 0..4 {
        let mut derived = fixture.record(DERIVED);
        let MirDispatchSlotsV1::ClassVtable(entries) = &mut derived.slots else {
            unreachable!()
        };
        let original = entries[0].signature().exact();
        entries[0].contract.signature = MirBridgeCallableSignatureV1::new(
            ExactCallableSignature::new(
                if axis == 0 {
                    Effect::Suspend
                } else {
                    original.effect()
                },
                original.receiver().into_option(),
                if axis == 1 {
                    vec![fixture.exact(UNIT)]
                } else {
                    vec![]
                },
                if axis == 2 {
                    fixture.exact(VALUE)
                } else {
                    original.result()
                },
            ),
            if axis == 3 {
                crate::GcEffect::NoGc
            } else {
                crate::GcEffect::Managed
            },
        );
        let error = fixture.check(&derived).unwrap_err();
        assert!(match axis {
            3 => matches!(error, MirDispatchSchemaError::SlotSignature { .. }),
            _ => matches!(error, MirDispatchSchemaError::TargetSignature { .. }),
        });
    }
    let value = fixture.record(VALUE);
    let binding = fixture.callables.get(fixture.target(6)).unwrap();
    let signature = MirBridgeCallableSignatureV1::new(
        binding.lowered_signature().exact().clone(),
        crate::GcEffect::Managed,
    );
    let replacement = ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority {
            identities: &fixture.graph,
            foundation: &fixture.foundation,
            types: &fixture.types,
        },
        binding.origin().clone(),
        binding.implementation(),
        signature.clone(),
        signature,
        *binding.lowering_role(),
    )
    .unwrap();
    fixture.callables = CanonicalMirCallableBindingsV1::try_new(
        fixture
            .callables
            .entries()
            .iter()
            .map(|binding| {
                if binding.implementation() == replacement.implementation() {
                    replacement.clone()
                } else {
                    binding.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        fixture.check(&value),
        Err(MirDispatchSchemaError::InvalidImplementation { .. })
    ));
}

#[test]
fn adjust_cannot_move_to_another_interface_with_the_same_slot_shape() {
    let fixture = Fixture::new();
    let mut value = fixture.record(VALUE);
    let table = value
        .itables
        .iter_mut()
        .find(|table| table.interface() == fixture.exact(LEFT))
        .unwrap();
    for entry in &mut table.entries {
        let index = usize::from(entry.slot() == fixture.slots[2].id());
        entry.implementation = MirDispatchImplementationV1::AdjustThunkTarget(
            scoop_identity::CallableDefinitionOwner::Strong(
                StrongCallableDefinitionOwner::GeneratedCallable(fixture.boxing[index].id()),
            ),
        );
    }
    assert!(matches!(
        fixture.check(&value),
        Err(MirDispatchSchemaError::InvalidImplementation { .. })
    ));
}

#[test]
fn abstract_class_keeps_original_typed_trap_and_derived_replaces_only_target() {
    let mut fixture = Fixture::new();
    let mut records = fixture.table().records;
    let base = fixture.types.get(fixture.exact(BASE)).unwrap();
    let abstract_base = ParamFreeMirTypeExportV1::try_new(
        MirTypeBridgeAuthority {
            identities: &fixture.graph,
            foundation: &fixture.foundation,
        },
        base.exact(),
        base.origin().clone(),
        base.facts(),
        MirTypeRepresentationV1::Class {
            release_policy: Default::default(),
            kind: MirClassKindV1::Abstract,
            declared_fields: vec![],
        },
        base.base_and_interfaces().clone(),
    )
    .unwrap();
    fixture.types = CanonicalParamFreeMirTypeExportsV1::try_new(
        fixture
            .types
            .records()
            .iter()
            .map(|record| {
                if record.exact() == abstract_base.exact() {
                    abstract_base.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    let declaration = fixture.callables.get(fixture.target(0)).unwrap();
    let trap = ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority {
            identities: &fixture.graph,
            foundation: &fixture.foundation,
            types: &fixture.types,
        },
        declaration.origin().clone(),
        declaration.implementation(),
        declaration.semantic_signature().clone(),
        declaration.lowered_signature().clone(),
        MirCallableLoweringRoleV1::PureVirtualTrap {
            slot: fixture.slots[0].id(),
        },
    )
    .unwrap();
    fixture.callables = CanonicalMirCallableBindingsV1::try_new(
        fixture
            .callables
            .entries()
            .iter()
            .map(|binding| {
                if binding.implementation() == trap.implementation() {
                    trap.clone()
                } else {
                    binding.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    let record = records
        .iter_mut()
        .find(|record| record.owner() == fixture.exact(BASE))
        .unwrap();
    let MirDispatchSlotsV1::ClassVtable(entries) = &mut record.slots else {
        unreachable!()
    };
    entries[0].implementation = MirDispatchImplementationV1::AbstractObligation {
        declaration: DispatchDeclarationOwner::Function(fixture.methods[0].id()),
        trap_target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target(0)),
        receiver: MirDispatchReceiverAdaptationV1::Identity,
    };
    let table = CanonicalMirDispatchSchemasV1::try_new(fixture.authority(), records).unwrap();
    assert!(matches!(
        table.get(fixture.exact(BASE)).unwrap().vtable()[0].implementation(),
        MirDispatchImplementationV1::AbstractObligation { .. }
    ));
    assert!(matches!(
        table.get(fixture.exact(DERIVED)).unwrap().vtable()[0].implementation(),
        MirDispatchImplementationV1::DirectStrongTarget { .. }
    ));
}
