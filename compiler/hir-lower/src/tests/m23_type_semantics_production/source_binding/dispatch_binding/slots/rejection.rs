use super::*;

#[test]
fn another_applicable_base_implementation_cannot_replace_the_sealed_override_choice() {
    with_hir_source(VIRTUAL, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let dispatch = sources.bind(&foundation).unwrap();

        let slots = dispatch.bind_slot_sources().unwrap();
        let mut checked = 0;
        for selection in sources.selections.records() {
            let record = candidate(&slots, *selection);
            let Implementation::Concrete(selected) = record.implementation() else {
                continue;
            };
            if selected.declaration() == record.declaration() {
                continue;
            }
            let base = target(&slots, record.declaration());
            let forged = replace(
                &record,
                record.signature().clone(),
                Implementation::Concrete(base),
                record.declaration_access().clone(),
            );
            slots
                .graph()
                .validate_slot_contract(selection.owner(), &forged, &slots)
                .unwrap();
            assert!(matches!(
                slots.validate_contract(selection.owner(), &forged),
                Err(hir::InheritanceInterfaceSemanticError::SlotSelection)
            ));
            checked += 1;
        }
        assert!(checked > 0);
    });
}

#[test]
fn matching_candidate_root_and_target_effects_cannot_override_source_contracts() {
    with_hir_source(CALLABLES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let dispatch = sources.bind(&foundation).unwrap();

        let slots = dispatch.bind_slot_sources().unwrap();
        for selection in sources.selections.records() {
            let record = candidate(&slots, *selection);
            let rewrite = |signature: &hir::InheritanceCallableSignatureV1| {
                let effects = signature.effects();
                let safety = if effects.safety() == hir::CallableSafetyV1::Safe {
                    hir::CallableSafetyV1::Unsafe
                } else {
                    hir::CallableSafetyV1::Safe
                };
                hir::InheritanceCallableSignatureV1::try_new(
                    signature.exact_signature().clone(),
                    hir::CallableSourceEffectsV1::try_new(
                        effects.execution(),
                        safety,
                        effects.gc_effect(),
                        effects.implementation(),
                        effects.operator_role(),
                        effects.infix(),
                    )
                    .unwrap(),
                )
                .unwrap()
            };
            let implementation = match record.implementation() {
                Implementation::Abstract => Implementation::Abstract,
                Implementation::Concrete(target) | Implementation::InterfaceDefault(target) => {
                    let changed = hir::InheritanceSlotTargetV1::try_new(
                        target.declaration(),
                        target.owner(),
                        rewrite(target.signature()),
                        target.modality(),
                        target.declaration_access().clone(),
                    )
                    .unwrap();
                    if matches!(record.implementation(), Implementation::Concrete(_)) {
                        Implementation::Concrete(changed)
                    } else {
                        Implementation::InterfaceDefault(changed)
                    }
                }
            };
            let forged = replace(
                &record,
                rewrite(record.signature()),
                implementation,
                record.declaration_access().clone(),
            );
            slots
                .graph()
                .validate_slot_contract(selection.owner(), &forged, &slots)
                .unwrap();
            assert!(matches!(
                slots.validate_contract(selection.owner(), &forged),
                Err(hir::InheritanceInterfaceSemanticError::SourceContract)
            ));
        }
    });
}
