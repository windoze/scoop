use super::*;
use hir::{
    CanonicalNominalDispatchSelectionsV1, InheritanceSourceSlotSelectionV1 as Selection,
    NominalDispatchSelectionV1, NominalInterfaceRecordV1,
};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let table = checked.metadata().public.nominal_interfaces();
    let nominal = table
        .all_records()
        .find(|record| {
            record
                .declaration_details()
                .dispatch_selections()
                .records()
                .iter()
                .any(|record| !matches!(record.selection(), Selection::Abstract(_)))
        })
        .unwrap();
    let records = nominal
        .declaration_details()
        .dispatch_selections()
        .records();
    let mut missing = records.to_vec();
    missing.pop();
    assert!(matches!(
        reject(checked, core, nominal, missing),
        Error::SlotSelectionInventory(_)
    ));
    let extra = table
        .all_records()
        .flat_map(|record| record.declaration_details().dispatch_selections().records())
        .find(|candidate| {
            records
                .iter()
                .all(|record| record.slot() != candidate.slot())
        })
        .unwrap();
    let mut extended = records.to_vec();
    extended.push(extra.clone());
    assert!(matches!(
        reject(checked, core, nominal, extended),
        Error::SlotSelectionInventory(_)
    ));
    let mut changed = records.to_vec();
    let target = changed
        .iter_mut()
        .find(|record| !matches!(record.selection(), Selection::Abstract(_)))
        .unwrap();
    *target = NominalDispatchSelectionV1::new(
        target.role().clone(),
        target.receiver().clone(),
        target.slot(),
        Selection::Abstract(target.selection().declaration()),
    );
    assert!(
        matches!(reject(checked, core, nominal, changed), Error::SlotContracts(error)
        if matches!(error.as_ref(), hir::InheritanceInterfaceSemanticError::SlotSelection))
    );
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    records: Vec<NominalDispatchSelectionV1>,
) -> Error {
    let details = nominal.declaration_details();
    dispatch::reject_nominal(
        checked,
        core,
        nominal,
        hir::NominalDeclarationDetailsV1::new(
            details.modality(),
            details.declared_visibility(),
            details.constructors().clone(),
            details.members().clone(),
            details.children().clone(),
            details.dispatch_order().clone(),
            CanonicalNominalDispatchSelectionsV1::try_new(records).unwrap(),
            details.primary_value_constructor(),
            details.instantiation_conditions().clone(),
            Default::default(),
            None,
            None,
        ),
    )
}
