use super::*;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let table = checked.metadata().public.nominal_interfaces();
    let class = table
        .all_records()
        .find(|record| {
            matches!(record.declaration_details().dispatch_order(),
        NominalDispatchOrderV1::Class { slots } if !slots.is_empty())
        })
        .unwrap();
    let mut missing = class.declaration_details().dispatch_order().clone();
    let NominalDispatchOrderV1::Class { slots } = &mut missing else {
        unreachable!("selected a class")
    };
    slots.pop();
    assert!(matches!(
        reject(checked, core, class, missing),
        Error::DispatchDeclarations(_)
    ));

    let interface = table
        .all_records()
        .find(|record| {
            matches!(record.declaration_details().dispatch_order(),
        NominalDispatchOrderV1::Interface { parents, .. } if parents.len() > 1)
        })
        .unwrap();
    let mut reversed = interface.declaration_details().dispatch_order().clone();
    let NominalDispatchOrderV1::Interface { parents, .. } = &mut reversed else {
        unreachable!("selected an interface")
    };
    parents.reverse();
    assert!(matches!(
        reject(checked, core, interface, reversed),
        Error::SlotSchemas(_)
    ));

    let mut unsuppressed = interface.declaration_details().dispatch_order().clone();
    let NominalDispatchOrderV1::Interface { members, .. } = &mut unsuppressed else {
        unreachable!("selected an interface")
    };
    let member = members
        .iter_mut()
        .find(|member| !member.overrides().is_empty())
        .unwrap();
    *member =
        hir::InterfaceSourceMemberV1::new(member.slot(), hir::CanonicalPersistentIdsV1::empty());
    assert!(matches!(
        reject(checked, core, interface, unsuppressed),
        Error::SlotSchemas(_)
    ));
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    order: NominalDispatchOrderV1,
) -> Error {
    let details = nominal.declaration_details();
    super::reject_nominal(
        checked,
        core,
        nominal,
        hir::NominalDeclarationDetailsV1::new(
            details.modality(),
            details.declared_visibility(),
            details.constructors().clone(),
            details.members().clone(),
            details.children().clone(),
            order,
            details.dispatch_selections().clone(),
        ),
    )
}
