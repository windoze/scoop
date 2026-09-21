use super::*;

#[test]
fn local_callable_references_require_an_existing_visible_declaration() {
    let f = LocalFixture::new();
    let path = child_path(
        template(&f.fixture).definition_path(),
        StructuralDefinitionSiteRole::CallableConversion,
        0,
    );
    let invoke = callable_reference_invoke(f.fixture.function, path.clone());
    let ty = f.descriptor.function_type().clone();
    let reference = DefaultCallableReferenceV1::try_new(
        invoke,
        path.clone(),
        DefaultCallableReferenceTargetV1::Local {
            declaration: f.descriptor.declaration(),
            callee: callable(source_function("scopedLocal")),
        },
        ty.clone(),
        Vec::new(),
        0,
    )
    .unwrap();
    let expression = || {
        expression(
            &f.fixture,
            DefaultExpressionKindV1::CallableReference(reference.clone()),
            ty.clone(),
        )
    };
    let ref_statement = || {
        statement(
            &f.fixture,
            DefaultStatementKindV1::Expr(Box::new(expression())),
        )
    };
    let authority = || {
        let mut authority = f.authority();
        authority.entries.push((
            DefaultNestedCallableIdentityV1::CallableReference(invoke),
            identity_shape(
                DefaultNestedCallableProvenanceV1::TemplateLexical,
                path.clone(),
                0,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(ty.clone(), Vec::new()),
        ));
        authority
    };
    let valid = template_with_body(
        &f.fixture,
        vec![f.declaration(), f.branch(vec![ref_statement()], Vec::new())],
        expression(),
    );
    validate_body(&valid, &mut authority()).unwrap();
    for statements in [
        vec![ref_statement(), f.declaration()],
        vec![f.branch(vec![f.declaration()], vec![ref_statement()])],
        vec![f.branch(vec![f.declaration()], Vec::new()), ref_statement()],
    ] {
        let invalid = template_with_body(&f.fixture, statements, unit(&f.fixture));
        assert_eq!(
            validate_body(&invalid, &mut authority()).unwrap_err(),
            DefaultNestedCallableAbiValidationError::MissingLocalFunction {
                declaration: f.descriptor.declaration(),
                site: DefaultNestedCallableLocalUseV1::CallableReference,
            }
        );
    }
}
