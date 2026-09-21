use super::*;
use crate::OptionalDefaultStatementListV1;

mod control_flow;
mod references;

struct LocalFixture {
    fixture: Fixture,
    descriptor: DefaultLocalFunctionV1,
}

impl LocalFixture {
    fn new() -> Self {
        let fixture = Fixture::new();
        let descriptor = DefaultLocalFunctionV1::try_new(
            CallableTemplateOrigin::Function(source_function("scopedLocal")),
            child_path(
                template(&fixture).definition_path(),
                StructuralDefinitionSiteRole::LocalDeclaration,
                0,
            ),
            function(binder(0)),
            Vec::new(),
            0,
        )
        .unwrap();
        Self {
            fixture,
            descriptor,
        }
    }

    fn declaration(&self) -> DefaultStatementV1 {
        statement(
            &self.fixture,
            DefaultStatementKindV1::LocalFunction(self.descriptor.clone()),
        )
    }

    fn call(&self) -> DefaultExpressionV1 {
        expression(
            &self.fixture,
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration: self.descriptor.declaration(),
                callee: callable(source_function("scopedLocal")),
                captures: Vec::new(),
                arguments: Vec::new(),
            },
            binder(0),
        )
    }

    fn call_statement(&self) -> DefaultStatementV1 {
        statement(
            &self.fixture,
            DefaultStatementKindV1::Expr(Box::new(self.call())),
        )
    }

    fn branch(
        &self,
        then_body: Vec<DefaultStatementV1>,
        else_body: Vec<DefaultStatementV1>,
    ) -> DefaultStatementV1 {
        statement(
            &self.fixture,
            DefaultStatementKindV1::If {
                condition: Box::new(expression(
                    &self.fixture,
                    DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
                    binder(0),
                )),
                then_body,
                else_body: OptionalDefaultStatementListV1::try_present(else_body).unwrap(),
            },
        )
    }

    fn authority(&self) -> AuthoritySet {
        AuthoritySet::new(vec![(
            DefaultNestedCallableIdentityV1::LocalFunction(self.descriptor.declaration()),
            identity_shape(
                DefaultNestedCallableProvenanceV1::DefaultDependency,
                self.descriptor.definition_path().clone(),
                0,
                DefaultNestedCallableBodyShapeV1::Absent,
            ),
            abi_shape(self.descriptor.function_type().clone(), Vec::new()),
        )])
    }
}

#[test]
fn repeated_default_declarations_in_distinct_blocks_keep_separate_occurrences() {
    let f = LocalFixture::new();
    let template = template_with_body(
        &f.fixture,
        vec![f.branch(
            vec![f.declaration(), f.call_statement()],
            vec![f.declaration(), f.call_statement()],
        )],
        unit(&f.fixture),
    );
    let mut authority = f.authority();
    validate_body(&template, &mut authority).unwrap();
    occurrences::assert_source_index_order(&template, &authority);
    assert_eq!(authority.sites.len(), 4);
    assert_eq!(
        authority.sites[0],
        DefaultNestedCallableSiteV1::Body { ordinal: 0 }
    );
    assert_eq!(
        authority.sites[2],
        DefaultNestedCallableSiteV1::Body { ordinal: 1 }
    );
}

#[test]
fn local_uses_reject_forward_calls_and_declarations_from_other_blocks() {
    let f = LocalFixture::new();
    let bodies = [
        vec![f.call_statement(), f.declaration()],
        vec![f.branch(vec![f.declaration()], vec![f.call_statement()])],
        vec![
            f.branch(vec![f.declaration()], Vec::new()),
            f.call_statement(),
        ],
    ];
    for statements in bodies {
        let template = template_with_body(&f.fixture, statements, unit(&f.fixture));
        assert_eq!(
            validate_body(&template, &mut f.authority()).unwrap_err(),
            DefaultNestedCallableAbiValidationError::MissingLocalFunction {
                declaration: f.descriptor.declaration(),
                site: DefaultNestedCallableLocalUseV1::DirectCall,
            }
        );
    }
}

#[test]
fn child_blocks_and_the_root_result_can_use_an_existing_outer_declaration() {
    let f = LocalFixture::new();
    let template = template_with_body(
        &f.fixture,
        vec![
            f.declaration(),
            f.branch(vec![f.call_statement()], vec![f.call_statement()]),
        ],
        f.call(),
    );
    validate_body(&template, &mut f.authority()).unwrap();
}
