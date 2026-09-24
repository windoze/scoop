use super::*;
use hir::{DeclaredVisibilityV1, NestedSourceSupportV1, SourceNominalId};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let records = checked.section().protected_declarations().records();
    let reference = records
        .iter()
        .find(|record| {
            matches!(record, ProtectedDeclarationInterfaceV1::Callable(callable)
            if matches!(callable.payload().owner(), SourceNominalId::GenericTemplate(_)))
        })
        .unwrap()
        .reference();
    let missing = records
        .iter()
        .filter(|record| record.reference() != reference)
        .cloned()
        .collect();
    assert!(
        matches!(reject(checked, core, missing), Error::ProtectedMember(actual) if actual == reference)
    );

    let source = records
        .iter()
        .find_map(|record| {
            let ProtectedDeclarationInterfaceV1::NestedNominal(nested) = record else {
                return None;
            };
            nested
                .payload()
                .source_interface()
                .source_support()
                .records()
                .iter()
                .find_map(|record| match record {
                    NestedSourceSupportV1::Callable(callable)
                        if matches!(
                            callable.declaration(),
                            CallableTemplateOrigin::Function(_)
                        ) && callable.declaration_access().declared_visibility()
                            == DeclaredVisibilityV1::Public =>
                    {
                        Some(callable)
                    }
                    _ => None,
                })
        })
        .unwrap();
    let payload = source.payload();
    let access = source.declaration_access();
    let extra = hir::ProtectedCallableInterfaceV1::try_new(
        source.declaration(),
        hir::DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Protected,
            access.lexical_owners().to_vec(),
            access.definition_origin().clone(),
        )
        .unwrap(),
        hir::ProtectedCallablePayloadV1::try_new(
            source.declaration(),
            payload.owner(),
            payload.type_parameters().clone(),
            payload.parameters().clone(),
            payload.result().clone(),
            payload.effects(),
            payload.modality(),
            payload.slot_relations().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut candidates = records.to_vec();
    candidates.push(ProtectedDeclarationInterfaceV1::Callable(Box::new(extra)));
    assert!(
        matches!(reject(checked, core, candidates), Error::ProtectedInventory(provider) if provider == checked.provider())
    );
}
