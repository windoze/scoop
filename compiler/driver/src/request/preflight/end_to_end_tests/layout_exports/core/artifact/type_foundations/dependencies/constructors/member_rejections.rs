use super::*;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let records = checked.section().inheritance().records();
    let index = records
        .iter()
        .position(|record| !record.protected_members().values().is_empty())
        .unwrap();
    let record = &records[index];
    let reference = record.protected_members().values()[0];
    let mut missing = record.protected_members().values().to_vec();
    missing.remove(0);
    let mut candidates = records.to_vec();
    candidates[index] = replace(
        record,
        record.constructors().clone(),
        CanonicalProtectedDeclarationRefsV1::try_new(missing).unwrap(),
    );
    assert!(
        matches!(reject(checked, core, candidates, checked.section().protected_declarations().clone()), Error::ProtectedMemberInventory(owner) if owner == record.owner())
    );

    let missing = checked
        .section()
        .protected_declarations()
        .records()
        .iter()
        .filter(|declaration| declaration.reference() != reference)
        .cloned()
        .collect();
    assert!(
        matches!(reject(checked, core, records.to_vec(), CanonicalProtectedDeclarationInterfacesV1::try_new(missing).unwrap()), Error::ProtectedMember(actual) if actual == reference)
    );

    let public = checked
        .metadata()
        .public
        .callable_interfaces()
        .all_declarations()
        .find(|callable| {
            callable.declared_visibility() == hir::DeclaredVisibilityV1::Public
                && callable.owner().nominal_owner().is_some()
                && matches!(callable.declaration(), CallableTemplateOrigin::Function(_))
        })
        .unwrap();
    let public = ProtectedDeclarationRefV1::Callable(
        hir::ProtectedCallableDeclarationRefV1::try_new(public.declaration()).unwrap(),
    );
    let mut extra = record.protected_members().values().to_vec();
    extra.push(public);
    let mut candidates = records.to_vec();
    candidates[index] = replace(
        record,
        record.constructors().clone(),
        CanonicalProtectedDeclarationRefsV1::try_new(extra).unwrap(),
    );
    assert!(
        matches!(reject(checked, core, candidates, checked.section().protected_declarations().clone()), Error::ProtectedMemberInventory(owner) if owner == record.owner())
    );

    callable_contract(checked, core);
}

fn callable_contract(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let mut records = checked
        .section()
        .protected_declarations()
        .records()
        .to_vec();
    let record = records
        .iter_mut()
        .find(|record| {
            matches!(record, ProtectedDeclarationInterfaceV1::Callable(callable)
            if matches!(callable.declaration(), CallableTemplateOrigin::Function(_))
                && !callable.payload().parameters().parameters().is_empty())
        })
        .unwrap();
    let ProtectedDeclarationInterfaceV1::Callable(callable) = record else {
        unreachable!("selected a source method above")
    };
    let declaration = callable.declaration();
    let payload = callable.payload();
    **callable = hir::ProtectedCallableInterfaceV1::try_new(
        declaration,
        callable.declaration_access().clone(),
        hir::ProtectedCallablePayloadV1::try_new(
            declaration,
            payload.owner(),
            payload.type_parameters().clone(),
            hir::CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
            payload.result().clone(),
            payload.effects(),
            payload.modality(),
            payload.slot_relations().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(reject(
        checked,
        core,
        checked.section().inheritance().records().to_vec(),
        CanonicalProtectedDeclarationInterfacesV1::try_new(records).unwrap(),
    ), Error::CallableContract(actual) if actual == declaration));
}
