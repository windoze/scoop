use super::*;
use hir::{DefaultExpressionKindV1 as Expr, DefaultSourceTemplateV1 as Template};

pub(super) fn call(
    original: &Template,
    callee: &hir::DefaultCallableRefV1,
    target_domain: hir::DefaultSourceAccessDomainV1,
) -> Template {
    let original_reference = &original.references().callables()[0];
    let mut kind = original.body().value().kind().clone();
    match &mut kind {
        Expr::Call { callee: target, .. } => *target = callee.clone(),
        Expr::MethodCall { callee: target, .. } => {
            *target = hir::DefaultMethodCalleeV1::Callable(callee.clone())
        }
        _ => panic!("source default calls its provider target"),
    }
    let witness = original_reference.witness();
    let witness = hir::DefaultSourceAccessWitnessV1::try_new(
        witness.owner(),
        witness.direct_call_domain().clone(),
        witness.slot_call_domain().clone(),
        target_domain,
    )
    .unwrap();
    let references = original.references();
    let references = hir::DefaultSourceReferencesV1::try_new(
        vec![hir::DefaultSourceReferenceV1::new(
            Target::Callable(callee.clone()),
            original_reference.definition_origin().clone(),
            witness,
        )],
        references.constructors().to_vec(),
        references.types().to_vec(),
        references.globals().to_vec(),
        references.singleton_values().to_vec(),
        references.fields().to_vec(),
    )
    .unwrap();
    body(original, kind, references)
}

pub(super) fn body(
    original: &Template,
    kind: Expr,
    references: hir::DefaultSourceReferencesV1,
) -> Template {
    let body = hir::ExportDefaultBodyV1::try_new(
        original.body().statements().to_vec(),
        hir::DefaultExpressionV1::try_new(
            kind,
            original.result().clone(),
            original.body().value().definition_origin().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    Template::try_new(
        original.key(),
        original.definition_root(),
        original.definition_path().clone(),
        original.locals().clone(),
        body,
        original.result().clone(),
        original.allows_suspend(),
        original.type_parameters().clone(),
        original.receiver().clone(),
        original.value_parameters().clone(),
        references,
        original.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap()
}
