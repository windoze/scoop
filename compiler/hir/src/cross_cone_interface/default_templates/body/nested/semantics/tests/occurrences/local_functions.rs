use super::*;
use crate::OptionalDefaultStatementListV1;

#[test]
fn repeated_local_identity_in_separate_scopes_keeps_each_signature() {
    let f = Fixture::new();
    let declaration = CallableTemplateOrigin::Function(source_function("expandedLocal"));
    let identity = DefaultNestedCallableIdentityV1::LocalFunction(declaration);
    let path = child_path(
        template(&f).definition_path(),
        StructuralDefinitionSiteRole::LocalDeclaration,
        0,
    );
    let types = [function(binder(0)), function(binder(1))];
    let local = |ty: &SignatureTypeKey| {
        statement(
            &f,
            DefaultStatementKindV1::LocalFunction(
                DefaultLocalFunctionV1::try_new(
                    declaration,
                    path.clone(),
                    ty.clone(),
                    Vec::new(),
                    1,
                )
                .unwrap(),
            ),
        )
    };
    let template = template_with_body(
        &f,
        vec![statement(
            &f,
            DefaultStatementKindV1::If {
                condition: Box::new(unit(&f)),
                then_body: vec![local(&types[0])],
                else_body: OptionalDefaultStatementListV1::try_present(vec![local(&types[1])])
                    .unwrap(),
            },
        )],
        unit(&f),
    );
    for collapse in [false, true] {
        let mut authority = Occurrences {
            identity,
            shape: identity_shape(
                DefaultNestedCallableProvenanceV1::DefaultDependency,
                path.clone(),
                1,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abis: types
                .iter()
                .map(|ty| abi_shape(ty.clone(), Vec::new()))
                .collect(),
            collapse,
            observed: Vec::new(),
        };
        let result = validate_body(&template, &mut authority);
        if collapse {
            assert!(matches!(result,
                Err(DefaultNestedCallableAbiValidationError::FunctionType {
                    kind: DefaultNestedCallableKindV1::LocalFunction, expected, actual,
                }) if *expected == types[0] && *actual == types[1]
            ));
        } else {
            result.unwrap();
        }
        assert_eq!(authority.observed.len(), 4);
        assert_eq!(authority.observed[0].0, Site::Body { ordinal: 0 });
        assert_eq!(authority.observed[2].0, Site::Body { ordinal: 1 });
    }
}
