use super::*;
use scoop_identity::{CallableTemplateOrigin, Effect};

mod fixture;

#[test]
fn callable_access_replays_every_body_descriptor_with_the_same_invoke_identity() {
    let fixture = fixture::references(false);
    let bytes = fixture.artifact();
    validate_until_type_alias(&bytes)
        .validate_source_interfaces(vec![])
        .unwrap();
    let mut front = declaration_front(&bytes);
    let CallableTemplateOrigin::Function(owner) = fixture.owner else {
        panic!("source owner")
    };
    restrict(
        &mut front,
        DefinitionOriginSubject::Function(owner),
        DeclaredVisibilityV1::Private,
    );
    let Err(Error::Template { key, source }) = support::validate(&mut front) else {
        panic!("second invoke must be checked")
    };
    assert_eq!(key.owner(), fixture.owner);
    let Error::Occurrence {
        index: 1,
        expression_index: Some(2),
        source,
        ..
    } = *source
    else {
        panic!("the second descriptor must retain its own expression index")
    };
    assert!(matches!(*source, Error::WitnessDomain));
}

#[test]
fn callable_access_rejects_generated_named_targets_and_false_local_reference_roles() {
    let fixture = fixture::references(true);
    let bytes = fixture.artifact();
    let mut front = declaration_front(&bytes);
    assert!(matches!(nested_failure(&mut front), Error::Target(error)
        if matches!(*error, DefaultSourceTargetSubjectError::CallableRole(_))));

    let mut fixture = fixture::references(false);
    let template = &fixture.interface.default_templates().records()[0];
    let reference = fixture::first_reference(template);
    let DefaultCallableReferenceTargetV1::Local { callee, .. } = reference.target() else {
        panic!("local target")
    };
    let changed = DefaultCallableReferenceV1::try_new(
        reference.invoke(),
        reference.definition_path().clone(),
        DefaultCallableReferenceTargetV1::Local {
            declaration: fixture.owner,
            callee: callee.clone(),
        },
        reference.function_type().clone(),
        vec![],
        0,
    )
    .unwrap();
    fixture::replace_first_reference(&mut fixture, changed);
    let bytes = fixture.artifact();
    let mut front = declaration_front(&bytes);
    assert!(matches!(nested_failure(&mut front), Error::Target(error)
        if matches!(*error, DefaultSourceTargetSubjectError::LocalReference(id) if id == fixture.owner)));
}

#[test]
fn callable_access_requires_local_and_generated_targets_at_actual_body_attachments() {
    let fixture = fixture::references(false);
    let bytes = fixture.artifact();
    let mut front = declaration_front(&bytes);
    let template = &fixture.interface.default_templates().records()[0];
    let reference = fixture::first_reference(template);
    let DefaultCallableReferenceTargetV1::Local { callee, .. } = reference.target() else {
        panic!("local target")
    };
    let (owner, origin, ty) = support::context(&front);
    let value = support::value(&ty, &origin);
    support::install(
        &mut front,
        owner,
        origin,
        ty,
        DefaultExpressionKindV1::Call {
            callee: callee.clone(),
            arguments: vec![value],
        },
        ExportDefaultCallableTargetV1::Callable(callee.clone()),
    );
    assert!(matches!(support::failure(&mut front), Error::Nested(error)
        if matches!(*error, DefaultSourceNestedCallableQueryError::MissingIdentity(_))));

    let mut front = declaration_front(&bytes);
    let (owner, origin, ty) = support::context(&front);
    support::install(
        &mut front,
        owner,
        origin,
        ty,
        DefaultExpressionKindV1::FunctionAddress(DefaultCallableDeclarationV1::Generated(
            reference.invoke(),
        )),
        ExportDefaultCallableTargetV1::FunctionAddress {
            declaration: DefaultCallableDeclarationV1::Generated(reference.invoke()),
        },
    );
    assert!(matches!(support::failure(&mut front), Error::Nested(error)
        if matches!(*error, DefaultSourceNestedCallableQueryError::Attachment)));
}

fn nested_failure(front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>) -> Error {
    let Err(Error::Template { source, .. }) = support::validate(front) else {
        panic!("nested access")
    };
    let Error::Occurrence { source, .. } = *source else {
        panic!("nested occurrence")
    };
    *source
}
