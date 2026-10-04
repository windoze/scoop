use super::*;
use hir::{CallableModalityV1, InheritanceInterfaceSemanticError as ContractError};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let mut rejected_ancestor = 0;
    let mut rejected_modality = false;
    let mut rejected_abstract = false;
    for nominal in checked.section().inheritance().records() {
        for slot in nominal.slots().records() {
            let target = slot.implementation().target();
            let source = callable(checked, slot.declaration());
            if target.declaration() != slot.declaration()
                && source.modality() != CallableModalityV1::Abstract
            {
                let ancestor = InheritanceSlotTargetV1::new(
                    slot.declaration(),
                    slot.signature().clone(),
                    source.modality(),
                    slot.declaration_access().clone(),
                );
                let ancestor = if source.modality() == CallableModalityV1::InterfaceDefault {
                    Implementation::InterfaceDefault(ancestor)
                } else {
                    Implementation::Concrete(ancestor)
                };
                assert!(matches!(
                    reject(checked, core, nominal.owner(), replace(slot, ancestor)),
                    Error::SlotContracts(error) if matches!(error.as_ref(), ContractError::SlotSelection)
                ));
                rejected_ancestor += 1;
            }
            if !rejected_modality && matches!(slot.implementation(), Implementation::Concrete(_)) {
                let modality = if target.modality() == CallableModalityV1::Final {
                    CallableModalityV1::Open
                } else {
                    CallableModalityV1::Final
                };
                let changed = InheritanceSlotTargetV1::new(
                    target.declaration(),
                    target.signature().clone(),
                    modality,
                    target.declaration_access().clone(),
                );
                assert!(matches!(
                    reject(checked, core, nominal.owner(), replace(slot, Implementation::Concrete(changed))),
                    Error::SlotContracts(error) if matches!(error.as_ref(), ContractError::SourceContract)
                ));
                rejected_modality = true;
            }
            if !rejected_abstract
                && nominal.edges().modality() == hir::NominalInheritanceModalityV1::Final
            {
                assert!(matches!(
                    reject(checked, core, nominal.owner(), replace(slot, Implementation::Abstract(InheritanceSlotTargetV1::new(
                        target.declaration(), target.signature().clone(),
                        CallableModalityV1::Abstract, target.declaration_access().clone(),
                    )))),
                    Error::SlotContracts(error) if matches!(error.as_ref(), ContractError::Slot(hir::InheritanceSlotContractSemanticError::AbstractObligation))
                ));
                rejected_abstract = true;
            }
        }
    }
    assert!(rejected_ancestor > 0 && rejected_modality && rejected_abstract);
    signature(checked, core);
}

fn signature(checked: CheckedSharedTypeFoundationV1<'_>, core: CheckedSharedTypeFoundationV1<'_>) {
    let (nominal, slot) = checked
        .section()
        .inheritance()
        .records()
        .iter()
        .flat_map(|nominal| {
            nominal
                .slots()
                .records()
                .iter()
                .map(move |slot| (nominal, slot))
        })
        .find(|(_, slot)| matches!(slot.implementation(), Implementation::Concrete(_)))
        .unwrap();
    let Implementation::Concrete(target) = slot.implementation() else {
        unreachable!()
    };
    let changed = |source: &hir::InheritanceCallableSignatureV1| {
        let e = source.effects();
        let effects = hir::CallableSourceEffectsV1::try_new(
            e.execution(),
            hir::CallableSafetyV1::Unsafe,
            e.gc_effect(),
            e.implementation(),
            e.operator_role(),
            e.infix(),
        )
        .unwrap();
        assert_ne!(effects, e);
        hir::InheritanceCallableSignatureV1::try_new(
            source.exact_signature().clone(),
            effects,
            Vec::new(),
        )
        .unwrap()
    };
    let target = InheritanceSlotTargetV1::new(
        target.declaration(),
        changed(target.signature()),
        target.modality(),
        target.declaration_access().clone(),
    );
    let replacement = InheritanceSlotContractV1::try_new(
        slot.role(),
        slot.slot(),
        slot.declaration(),
        changed(slot.signature()),
        Implementation::Concrete(target),
        slot.declaration_access().clone(),
    )
    .unwrap();
    assert!(matches!(
        reject(checked, core, nominal.owner(), replacement),
        Error::SlotContracts(error) if matches!(error.as_ref(), ContractError::SourceContract)
    ));
}
