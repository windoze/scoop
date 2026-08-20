use super::*;

impl Fixture {
    pub fn virtual_entry(&self, derived: bool) -> MirDispatchEntryV1 {
        MirDispatchEntryV1::new(
            self.slots[0].id(),
            MirDispatchPositionV1::new(0),
            self.callables
                .get(self.target(0))
                .unwrap()
                .lowered_signature()
                .clone(),
            MirDispatchImplementationV1::DirectStrongTarget {
                target: scoop_identity::CallableDefinitionOwner::Strong(
                    self.target(usize::from(derived)),
                ),
                receiver: if derived {
                    MirDispatchReceiverAdaptationV1::ReferenceDispatch
                } else {
                    MirDispatchReceiverAdaptationV1::Identity
                },
            },
        )
    }
    pub fn interface_entries(&self, owner: usize, interface: usize) -> Vec<MirDispatchEntryV1> {
        let mut slots = [1, 2];
        slots.sort_by_key(|index| std::cmp::Reverse(self.slots[*index].id()));
        slots
            .into_iter()
            .enumerate()
            .map(|(position, slot)| {
                let implementation = if owner == VALUE {
                    MirDispatchImplementationV1::AdjustThunkTarget(
                        scoop_identity::CallableDefinitionOwner::Strong(
                            StrongCallableDefinitionOwner::GeneratedCallable(
                                self.boxing[usize::from(interface == LEFT) * 2 + slot - 1].id(),
                            ),
                        ),
                    )
                } else if slot == 2 {
                    MirDispatchImplementationV1::InterfaceDefaultTarget {
                        target: scoop_identity::CallableDefinitionOwner::Strong(self.target(4)),
                        receiver: if interface == ROOT {
                            MirDispatchReceiverAdaptationV1::Identity
                        } else {
                            MirDispatchReceiverAdaptationV1::ReferenceDispatch
                        },
                    }
                } else if owner == DERIVED {
                    MirDispatchImplementationV1::DirectStrongTarget {
                        target: scoop_identity::CallableDefinitionOwner::Strong(self.target(5)),
                        receiver: MirDispatchReceiverAdaptationV1::ReferenceDispatch,
                    }
                } else {
                    MirDispatchImplementationV1::AbstractObligation {
                        declaration: DispatchDeclarationOwner::Function(self.methods[3].id()),
                        trap_target: scoop_identity::CallableDefinitionOwner::Strong(
                            self.target(3),
                        ),
                        receiver: if interface == ROOT {
                            MirDispatchReceiverAdaptationV1::Identity
                        } else {
                            MirDispatchReceiverAdaptationV1::ReferenceDispatch
                        },
                    }
                };
                MirDispatchEntryV1::new(
                    self.slots[slot].id(),
                    MirDispatchPositionV1::new(position as u32),
                    MirBridgeCallableSignatureV1::new(
                        ExactCallableSignature::new(
                            Effect::Ordinary,
                            Some(self.exact(interface)),
                            vec![],
                            self.exact(UNIT),
                        ),
                        crate::GcEffect::Managed,
                    ),
                    implementation,
                )
            })
            .collect()
    }
    pub fn record(&self, owner: usize) -> ParamFreeMirDispatchSchemaV1 {
        let vtable = match owner {
            BASE | DERIVED => {
                MirDispatchSlotsV1::ClassVtable(vec![self.virtual_entry(owner == DERIVED)])
            }
            OTHER => MirDispatchSlotsV1::ClassVtable(vec![]),
            ROOT..=DIAMOND => MirDispatchSlotsV1::InterfaceSlots(
                self.interface_entries(owner, owner)
                    .into_iter()
                    .map(|entry| entry.contract)
                    .collect(),
            ),
            _ => MirDispatchSlotsV1::NoClassVtable,
        };
        let interfaces = match owner {
            DERIVED => vec![ROOT, LEFT, RIGHT, DIAMOND],
            VALUE => vec![ROOT, LEFT],
            _ => vec![],
        };
        let mut itables: Vec<_> = interfaces
            .into_iter()
            .map(|interface| {
                MirInterfaceDispatchTableV1::new(
                    self.exact(interface),
                    self.interface_entries(owner, interface),
                )
            })
            .collect();
        itables.sort_by_key(MirInterfaceDispatchTableV1::interface);
        ParamFreeMirDispatchSchemaV1::try_new(self.authority(), self.exact(owner), vtable, itables)
            .unwrap()
    }
    pub fn table(&self) -> CanonicalMirDispatchSchemasV1 {
        CanonicalMirDispatchSchemasV1::try_new(
            self.authority(),
            (BASE..=VALUE).map(|owner| self.record(owner)).collect(),
        )
        .unwrap()
    }
    pub fn check(
        &self,
        record: &ParamFreeMirDispatchSchemaV1,
    ) -> Result<(), MirDispatchSchemaError> {
        self.authority().validate_record(record)
    }
}

#[test]
fn direct_class_override_and_interface_default_keep_source_slot_order() {
    let fixture = Fixture::new();
    let table = fixture.table();
    let derived = table.get(fixture.exact(DERIVED)).unwrap();
    assert_eq!(
        derived.vtable()[0].signature(),
        table.get(fixture.exact(BASE)).unwrap().vtable()[0].signature()
    );
    assert!(matches!(
        derived.vtable()[0].implementation(),
        MirDispatchImplementationV1::DirectStrongTarget {
            receiver: MirDispatchReceiverAdaptationV1::ReferenceDispatch,
            ..
        }
    ));
    for interface in derived.itables() {
        assert!(interface.entries()[0].slot() > interface.entries()[1].slot());
        assert_eq!(
            interface
                .entries()
                .iter()
                .map(|entry| entry.position().get())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            interface
                .entries()
                .iter()
                .map(MirDispatchEntryV1::slot)
                .collect::<Vec<_>>(),
            fixture
                .record(ROOT)
                .interface_slots()
                .unwrap()
                .iter()
                .map(MirDispatchSlotV1::slot)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn boxed_value_method_and_default_have_distinct_semantic_receivers() {
    let fixture = Fixture::new();
    let table = fixture.table();
    let value = table.get(fixture.exact(VALUE)).unwrap();
    for entry in value.itables()[0].entries() {
        let MirDispatchImplementationV1::AdjustThunkTarget(target) = entry.implementation() else {
            panic!("boxed entries require a real adjust")
        };
        let binding = fixture.callables.get(target).unwrap();
        let is_method = entry.slot() == fixture.slots[1].id();
        assert_eq!(
            binding
                .semantic_signature()
                .exact()
                .receiver()
                .into_option(),
            Some(fixture.exact(if is_method { VALUE } else { ROOT }))
        );
        assert_eq!(
            binding.semantic_signature().gc_effect(),
            if is_method {
                crate::GcEffect::NoGc
            } else {
                crate::GcEffect::Managed
            }
        );
        assert_eq!(
            binding.lowered_signature().gc_effect(),
            crate::GcEffect::Managed
        );
    }
}

#[test]
fn diamond_receiver_paths_choose_shortest_then_canonical_exact_sequence() {
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture
            .authority()
            .canonical_receiver_path(fixture.exact(DERIVED), fixture.exact(ROOT))
            .unwrap(),
        vec![
            fixture.exact(DERIVED),
            fixture.exact(DIAMOND),
            fixture.exact(LEFT).min(fixture.exact(RIGHT)),
            fixture.exact(ROOT)
        ]
    );
    fixture.set_edges(DERIVED, Some(BASE), &[DIAMOND, ROOT]);
    assert_eq!(
        fixture
            .authority()
            .canonical_receiver_path(fixture.exact(DERIVED), fixture.exact(ROOT))
            .unwrap(),
        vec![fixture.exact(DERIVED), fixture.exact(ROOT)]
    );
}

#[test]
fn inherited_interface_contracts_and_boxed_entries_use_current_table_receiver() {
    let fixture = Fixture::new();
    let table = fixture.table();
    let left = table.get(fixture.exact(LEFT)).unwrap();
    assert!(left.itables().is_empty());
    for entry in left.interface_slots().unwrap() {
        assert_eq!(
            entry.signature().exact().receiver().into_option(),
            Some(fixture.exact(LEFT))
        );
    }
    let boxed = table
        .get(fixture.exact(VALUE))
        .unwrap()
        .interface_table(fixture.exact(LEFT))
        .unwrap();
    for (entry, contract) in boxed.entries().iter().zip(left.interface_slots().unwrap()) {
        assert_eq!(entry.signature(), contract.signature());
        let target = fixture
            .callables
            .get(entry.implementation().target())
            .unwrap();
        assert_eq!(
            target.lowered_signature().exact().receiver().into_option(),
            Some(crate::InterfaceAdjustIdentity::boxed_receiver(fixture.exact(VALUE)).unwrap())
        );
        assert_eq!(
            target.lowered_signature().exact().parameters(),
            contract.signature().exact().parameters()
        );
        assert_eq!(
            target.lowered_signature().exact().result(),
            contract.signature().exact().result()
        );
        assert_eq!(
            target.semantic_signature().exact().receiver().into_option(),
            Some(fixture.exact(if entry.slot() == fixture.slots[1].id() {
                VALUE
            } else {
                ROOT
            }))
        );
    }
}
