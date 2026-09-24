use super::*;
use hir::{
    CanonicalProtectedDefaultExpressionUsesV1, ProtectedDefaultBodyClosureError,
    ProtectedDefaultCallableReferenceV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultReceiverUseV1,
};

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let templates = checked.section().protected_defaults().records();
    let template = templates
        .iter()
        .find(|template| !template.references().callables().is_empty())
        .unwrap();
    let reference = &template.references().callables()[0];
    let mut removed = template.references().callables().to_vec();
    removed.remove(0);
    assert!(
        matches!(reject(checked, core, with_callables(template, removed)), Error::DefaultReferences(key) if key == template.key())
    );

    for uses in [
        Vec::new(),
        vec![ProtectedDefaultExpressionUseV1::new(
            u32::MAX,
            ProtectedDefaultReceiverUseV1::None,
        )],
        reference
            .uses()
            .values()
            .iter()
            .map(|occurrence| {
                ProtectedDefaultExpressionUseV1::new(
                    occurrence.expression_index(),
                    ProtectedDefaultReceiverUseV1::ImplicitThis,
                )
            })
            .collect(),
    ] {
        let changed = ProtectedDefaultCallableReferenceV1::new(
            reference.target().clone(),
            reference.definition_origin().clone(),
            reference.witness().clone(),
            CanonicalProtectedDefaultExpressionUsesV1::try_new(uses).unwrap(),
        );
        assert!(matches!(
            reject_reference(checked, core, template, changed),
            Error::DefaultBody(error)
                if matches!(error.as_ref(), ProtectedDefaultBodyClosureError::MissingUse { .. })
        ));
    }
    let wrong_origin = checked
        .section()
        .definition_sources()
        .sources()
        .iter()
        .find(|origin| *origin != reference.definition_origin())
        .unwrap();
    let changed = ProtectedDefaultCallableReferenceV1::new(
        reference.target().clone(),
        wrong_origin.clone(),
        reference.witness().clone(),
        reference.uses().clone(),
    );
    assert!(matches!(
        reject_reference(checked, core, template, changed),
        Error::DefaultReferences(key) if key == template.key()
    ));
}

pub(super) fn reject_reference(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    template: &ProtectedDefaultTemplateV1,
    replacement: ProtectedDefaultCallableReferenceV1,
) -> Error {
    let mut records = template.references().callables().to_vec();
    records[0] = replacement;
    reject(checked, core, with_callables(template, records))
}

fn with_callables(
    template: &ProtectedDefaultTemplateV1,
    callables: Vec<ProtectedDefaultCallableReferenceV1>,
) -> ProtectedDefaultTemplateV1 {
    let r = template.references();
    let references = ProtectedDefaultReferenceSetV1::try_new(
        callables,
        r.constructors().to_vec(),
        r.types().to_vec(),
        r.globals().to_vec(),
        r.singleton_values().to_vec(),
        r.fields().to_vec(),
    )
    .unwrap();
    replace(
        template,
        template.body().clone(),
        references,
        template.allows_suspend(),
    )
}
