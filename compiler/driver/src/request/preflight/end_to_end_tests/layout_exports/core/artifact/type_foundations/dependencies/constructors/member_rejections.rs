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
    let mut missing = record.protected_members().values().to_vec();
    missing.remove(0);
    let mut candidates = records.to_vec();
    candidates[index] = replace(
        record,
        CanonicalProtectedDeclarationRefsV1::try_new(missing).unwrap(),
    );
    assert!(
        matches!(reject(checked, core, candidates), Error::ProtectedMemberInventory(owner) if owner == record.owner())
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
        CanonicalProtectedDeclarationRefsV1::try_new(extra).unwrap(),
    );
    assert!(
        matches!(reject(checked, core, candidates), Error::ProtectedMemberInventory(owner) if owner == record.owner())
    );
}
