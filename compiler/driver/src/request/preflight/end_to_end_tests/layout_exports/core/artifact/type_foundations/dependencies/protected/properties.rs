use super::*;
use hir::{
    ProtectedPropertyInterfaceV1, ProtectedPropertyMutabilityV1, ProtectedPropertyPayloadV1,
};
use scoop_identity::SignatureTypeKey;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    for change in [
        Change::Type,
        Change::Setter,
        Change::SetterAccess,
        Change::Slots,
    ] {
        let mut records = checked
            .section()
            .protected_declarations()
            .records()
            .to_vec();
        let record = records
            .iter_mut()
            .find_map(|record| match record {
                ProtectedDeclarationInterfaceV1::Property(property)
                    if matches!(
                        property.payload().owner(),
                        hir::SourceNominalId::Concrete(_)
                    ) && matches!(
                        property.payload().mutability(),
                        ProtectedPropertyMutabilityV1::ReadWrite { .. }
                    ) && match change {
                        Change::Slots => !property.payload().slot_relations().is_empty(),
                        _ => property.payload().slot_relations().is_empty(),
                    } =>
                {
                    Some(property)
                }
                _ => None,
            })
            .unwrap();
        let id = record.declaration();
        let payload = record.payload();
        let mut ty = payload.value_type().clone();
        let mut mutability = payload.mutability().clone();
        let mut slots = payload.slot_relations().clone();
        match change {
            Change::Type => {
                let hir::SourceNominalId::Concrete(owner) = payload.owner() else {
                    unreachable!()
                };
                ty = SignatureTypeKey::Nominal(owner);
            }
            Change::Setter => mutability = ProtectedPropertyMutabilityV1::ReadOnly,
            Change::SetterAccess => {
                let ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } =
                    &mut mutability
                else {
                    unreachable!()
                };
                *setter_access = changed_access(setter_access);
            }
            Change::Slots => {
                slots = hir::CanonicalProtectedSlotRefsV1::try_new(Vec::new()).unwrap()
            }
        }
        **record = ProtectedPropertyInterfaceV1::try_new(
            id,
            record.declaration_access().clone(),
            ProtectedPropertyPayloadV1::try_new(
                payload.owner(),
                ty,
                payload.getter(),
                mutability,
                payload.representation(),
                slots,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            matches!(reject(checked, core, records), Error::PropertyContract(actual) if actual == id)
        );
    }
}

enum Change {
    Type,
    Setter,
    SetterAccess,
    Slots,
}

fn changed_access(source: &hir::DeclarationAccessSourceV1) -> hir::DeclarationAccessSourceV1 {
    assert_eq!(
        source.declared_visibility(),
        hir::DeclaredVisibilityV1::Private
    );
    hir::DeclarationAccessSourceV1::try_new(
        hir::DeclaredVisibilityV1::Protected,
        source.lexical_owners().to_vec(),
        source.definition_origin().clone(),
    )
    .unwrap()
}
