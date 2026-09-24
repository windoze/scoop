use super::*;
use hir::{
    NestedSourceSupportV1, NestedSupportDeclarationV1, NominalInheritanceModalityV1 as Modality,
};

mod constants;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    for remove_child in [false, true] {
        let mut records = checked
            .section()
            .protected_declarations()
            .records()
            .to_vec();
        let nested = records
            .iter_mut()
            .find_map(|record| match record {
                ProtectedDeclarationInterfaceV1::NestedNominal(record)
                    if !record
                        .payload()
                        .source_interface()
                        .children()
                        .values()
                        .is_empty() =>
                {
                    Some(record)
                }
                _ => None,
            })
            .unwrap();
        let owner = nested.declaration();
        let source = nested.payload().source_interface();
        let mut children = source.children().clone();
        let mut support = source.source_support().clone();
        let mut modality = source.modality();
        if remove_child {
            let removed = children.values()[0];
            children = hir::CanonicalNestedNominalRefsV1::try_new(children.values()[1..].to_vec())
                .unwrap();
            support = hir::CanonicalNestedSourceSupportV1::try_new(
                support
                    .records()
                    .iter()
                    .filter(|record| {
                        record.declaration() != NestedSupportDeclarationV1::NestedNominal(removed)
                    })
                    .cloned()
                    .collect(),
            )
            .unwrap();
        } else {
            assert_eq!(modality, Modality::Final);
            modality = Modality::Open;
        }
        **nested = replace_nested(
            nested,
            nested_interface(source, modality, children, support),
        );
        assert!(
            matches!(reject(checked, core, records), Error::NestedContract(actual) if actual == owner)
        );
    }
    variant_parameters(checked, core);
    constants::check(checked, core);
}

fn variant_parameters(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let mut records = checked
        .section()
        .protected_declarations()
        .records()
        .to_vec();
    let nested = records
        .iter_mut()
        .find_map(|record| match record {
            ProtectedDeclarationInterfaceV1::NestedNominal(record)
                if record.payload().source_interface().kind() == hir::PublicNominalKindV1::Enum =>
            {
                Some(record)
            }
            _ => None,
        })
        .unwrap();
    let source = nested.payload().source_interface();
    let mut support = source.source_support().records().to_vec();
    let variant = support
        .iter_mut()
        .find_map(|record| match record {
            NestedSourceSupportV1::Callable(callable)
                if matches!(
                    callable.declaration(),
                    CallableTemplateOrigin::VariantConstructor(_)
                ) && !callable.payload().parameters().parameters().is_empty() =>
            {
                Some(callable)
            }
            _ => None,
        })
        .unwrap();
    let declaration = variant.declaration();
    let payload = variant.payload();
    **variant = hir::NominalSupportCallableInterfaceV1::try_new(
        declaration,
        variant.declaration_access().clone(),
        hir::NominalSourceCallablePayloadV1::try_new(
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
    **nested = replace_nested(
        nested,
        nested_interface(
            source,
            source.modality(),
            source.children().clone(),
            hir::CanonicalNestedSourceSupportV1::try_new(support).unwrap(),
        ),
    );
    assert!(
        matches!(reject(checked, core, records), Error::CallableContract(actual) if actual == declaration)
    );
}
