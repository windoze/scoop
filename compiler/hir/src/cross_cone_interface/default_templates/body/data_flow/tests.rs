use scoop_identity::{
    CallableTemplateOrigin, LocalValueSelector, SignatureTypeKey, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::WirePath;

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::Fixture;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultAssignTargetV1, DefaultCaptureV1,
    DefaultExpressionKindV1, DefaultExpressionV1, DefaultLocalFunctionV1, DefaultPatternV1,
    DefaultStatementKindV1, DefaultStatementV1, DefaultTryV1, ExportDefaultBodyV1,
    ExportDefaultReferenceSetV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    OptionalDefaultExpressionV1, OptionalDefaultStatementListV1, OptionalTemplateReceiverV1,
    PersistentLexicalRootV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
    TemplateValueParameterV1,
};

#[test]
fn accepts_parameter_declaration_capture_and_final_read_in_definition_order() {
    let fixture = Fixture::new();
    let parameter = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let declared = local_selector(0);
    let local_function = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        path(StructuralDefinitionSiteRole::LocalDeclaration, 9),
        binder(),
        vec![DefaultCaptureV1::new(
            declared.clone(),
            binder(),
            fixture.origin(),
        )],
        0,
    )
    .unwrap();
    let template = template(
        &fixture,
        vec![
            source_record(&fixture, parameter.clone(), CanonicalBooleanV1::False),
            source_record(&fixture, declared.clone(), CanonicalBooleanV1::False),
        ],
        vec![TemplateValueParameterV1::try_new(0, parameter.clone()).unwrap()],
        vec![
            statement(
                &fixture,
                DefaultStatementKindV1::ValDecl {
                    pattern: DefaultPatternV1::binding(declared.clone()),
                    init: Box::new(local_expression(&fixture, parameter, binder())),
                },
            ),
            statement(
                &fixture,
                DefaultStatementKindV1::LocalFunction(local_function),
            ),
        ],
        local_expression(&fixture, declared, binder()),
    );

    assert_eq!(validate(&template), Ok(()));
}

#[test]
fn leaves_local_reference_retyping_to_operation_typing() {
    let fixture = Fixture::new();
    let parameter = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let retyped = SignatureTypeKey::Binder { depth: 0, index: 1 };
    let template = template(
        &fixture,
        vec![source_record(
            &fixture,
            parameter.clone(),
            CanonicalBooleanV1::False,
        )],
        vec![TemplateValueParameterV1::try_new(0, parameter.clone()).unwrap()],
        Vec::new(),
        local_expression(&fixture, parameter, retyped),
    );

    assert_eq!(validate(&template), Ok(()));
}

#[test]
fn rejects_reachable_future_read_but_ignores_it_after_abrupt_completion() {
    let fixture = Fixture::new();
    let future = local_selector(0);
    let records = vec![source_record(
        &fixture,
        future.clone(),
        CanonicalBooleanV1::False,
    )];
    let declaration = || {
        statement(
            &fixture,
            DefaultStatementKindV1::ValDecl {
                pattern: DefaultPatternV1::binding(future.clone()),
                init: Box::new(unit(&fixture)),
            },
        )
    };
    let read = || {
        expression_statement(
            &fixture,
            local_expression(&fixture, future.clone(), binder()),
        )
    };
    let reachable = template(
        &fixture,
        records.clone(),
        Vec::new(),
        vec![read(), declaration()],
        unit(&fixture),
    );

    assert_eq!(
        validate(&reachable),
        Err(ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Expression,
            selector: Box::new(future.clone()),
            error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
        })
    );

    let unreachable = template(
        &fixture,
        records,
        Vec::new(),
        vec![
            statement(
                &fixture,
                DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::absent()),
            ),
            read(),
            declaration(),
        ],
        unit(&fixture),
    );
    assert_eq!(validate(&unreachable), Ok(()));
}

#[test]
fn intersects_first_assignments_across_normal_branch_exits() {
    let fixture = Fixture::new();
    let local = local_selector(0);
    let records = vec![source_record(
        &fixture,
        local.clone(),
        CanonicalBooleanV1::True,
    )];
    let assignment = || assignment_statement(&fixture, local.clone());
    let one_branch = template(
        &fixture,
        records.clone(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::If {
                condition: Box::new(unit(&fixture)),
                then_body: vec![assignment()],
                else_body: OptionalDefaultStatementListV1::absent(),
            },
        )],
        local_expression(&fixture, local.clone(), binder()),
    );

    assert_eq!(
        validate(&one_branch),
        Err(ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Expression,
            selector: Box::new(local.clone()),
            error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
        })
    );

    let both_branches = template(
        &fixture,
        records,
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::If {
                condition: Box::new(unit(&fixture)),
                then_body: vec![assignment()],
                else_body: OptionalDefaultStatementListV1::try_present(vec![assignment()]).unwrap(),
            },
        )],
        local_expression(&fixture, local, binder()),
    );
    assert_eq!(validate(&both_branches), Ok(()));
}

#[test]
fn rejects_immutable_assignment_and_mismatched_capture_type() {
    let fixture = Fixture::new();
    let immutable = local_selector(0);
    let assignment = template(
        &fixture,
        vec![source_record(
            &fixture,
            immutable.clone(),
            CanonicalBooleanV1::False,
        )],
        Vec::new(),
        vec![assignment_statement(&fixture, immutable.clone())],
        unit(&fixture),
    );
    assert_eq!(
        validate(&assignment),
        Err(ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Assignment,
            selector: Box::new(immutable),
            error: DefaultLocalDataFlowLocalError::Mutability {
                expected: CanonicalBooleanV1::False,
                actual: CanonicalBooleanV1::True,
            },
        })
    );

    let parameter = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let mismatched = SignatureTypeKey::Binder { depth: 0, index: 1 };
    let local_function = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        path(StructuralDefinitionSiteRole::LocalDeclaration, 1),
        binder(),
        vec![DefaultCaptureV1::new(
            parameter.clone(),
            mismatched.clone(),
            fixture.origin(),
        )],
        0,
    )
    .unwrap();
    let capture = template(
        &fixture,
        vec![source_record(
            &fixture,
            parameter.clone(),
            CanonicalBooleanV1::False,
        )],
        vec![TemplateValueParameterV1::try_new(0, parameter.clone()).unwrap()],
        vec![statement(
            &fixture,
            DefaultStatementKindV1::LocalFunction(local_function),
        )],
        unit(&fixture),
    );
    assert_eq!(
        validate(&capture),
        Err(ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Capture { index: 0 },
            selector: Box::new(parameter),
            error: DefaultLocalDataFlowLocalError::Type {
                expected: Box::new(binder()),
                actual: Box::new(mismatched),
            },
        })
    );
}

#[test]
fn rejects_mutable_capture_source() {
    let fixture = Fixture::new();
    let local = local_selector(0);
    let local_function = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        path(StructuralDefinitionSiteRole::LocalDeclaration, 2),
        binder(),
        vec![DefaultCaptureV1::new(
            local.clone(),
            binder(),
            fixture.origin(),
        )],
        0,
    )
    .unwrap();
    let template = template(
        &fixture,
        vec![source_record(
            &fixture,
            local.clone(),
            CanonicalBooleanV1::True,
        )],
        Vec::new(),
        vec![
            assignment_statement(&fixture, local.clone()),
            statement(
                &fixture,
                DefaultStatementKindV1::LocalFunction(local_function),
            ),
        ],
        unit(&fixture),
    );

    assert_eq!(
        validate(&template),
        Err(ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Capture { index: 0 },
            selector: Box::new(local),
            error: DefaultLocalDataFlowLocalError::Mutability {
                expected: CanonicalBooleanV1::True,
                actual: CanonicalBooleanV1::False,
            },
        })
    );
}

#[test]
fn enforces_loop_control_nesting() {
    let fixture = Fixture::new();
    let outside = template(
        &fixture,
        Vec::new(),
        Vec::new(),
        vec![statement(&fixture, DefaultStatementKindV1::Break)],
        unit(&fixture),
    );
    assert_eq!(
        validate(&outside),
        Err(
            ExportDefaultLocalDataFlowValidationError::LoopControlOutsideLoop {
                control: DefaultLoopControlV1::Break,
            }
        )
    );

    let inside = template(
        &fixture,
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::While {
                condition_setup: Vec::new(),
                condition: Box::new(unit(&fixture)),
                body: vec![statement(&fixture, DefaultStatementKindV1::Break)],
            },
        )],
        unit(&fixture),
    );
    assert_eq!(validate(&inside), Ok(()));
}

#[test]
fn finally_observes_the_intersection_of_all_incoming_completions() {
    let fixture = Fixture::new();
    let local = local_selector(0);
    let conditional_return = statement(
        &fixture,
        DefaultStatementKindV1::If {
            condition: Box::new(unit(&fixture)),
            then_body: vec![statement(
                &fixture,
                DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::absent()),
            )],
            else_body: OptionalDefaultStatementListV1::absent(),
        },
    );
    let try_value = DefaultTryV1::try_new(
        vec![
            assignment_statement(&fixture, local.clone()),
            conditional_return,
        ],
        Vec::new(),
        OptionalDefaultStatementListV1::try_present(vec![expression_statement(
            &fixture,
            local_expression(&fixture, local.clone(), binder()),
        )])
        .unwrap(),
    )
    .unwrap();
    let template = template(
        &fixture,
        vec![source_record(&fixture, local, CanonicalBooleanV1::True)],
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::Try(Box::new(try_value)),
        )],
        unit(&fixture),
    );

    assert_eq!(validate(&template), Ok(()));
}

fn validate(
    template: &ExportDefaultTemplateV1,
) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
    template.validate_local_data_flow_semantics(&WirePath::root().field(8))
}

fn template(
    fixture: &Fixture,
    records: Vec<TemplateLocalRecordV1>,
    parameters: Vec<TemplateValueParameterV1>,
    statements: Vec<DefaultStatementV1>,
    value: DefaultExpressionV1,
) -> ExportDefaultTemplateV1 {
    let result = value.result_type().clone();
    let body = ExportDefaultBodyV1::try_new(statements, value).unwrap();
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        path(StructuralDefinitionSiteRole::DefaultValue, 0),
        CanonicalTemplateLocalTableV1::try_new(records).unwrap(),
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(parameters).unwrap(),
        ExportDefaultReferenceSetV1::default(),
        fixture.origin(),
    )
    .unwrap()
}

fn unit(fixture: &Fixture) -> DefaultExpressionV1 {
    expression(fixture, DefaultExpressionKindV1::UnitLiteral, binder())
}

fn local_expression(
    fixture: &Fixture,
    local: LocalValueSelector,
    value_type: SignatureTypeKey,
) -> DefaultExpressionV1 {
    expression(fixture, DefaultExpressionKindV1::Local(local), value_type)
}

fn expression(
    fixture: &Fixture,
    kind: DefaultExpressionKindV1,
    value_type: SignatureTypeKey,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(
        kind,
        value_type,
        fixture.origin(),
        scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
    )
    .unwrap()
}

fn statement(fixture: &Fixture, kind: DefaultStatementKindV1) -> DefaultStatementV1 {
    DefaultStatementV1::try_new(kind, fixture.origin()).unwrap()
}

fn expression_statement(fixture: &Fixture, expression: DefaultExpressionV1) -> DefaultStatementV1 {
    statement(fixture, DefaultStatementKindV1::Expr(Box::new(expression)))
}

fn assignment_statement(fixture: &Fixture, local: LocalValueSelector) -> DefaultStatementV1 {
    statement(
        fixture,
        DefaultStatementKindV1::Assign {
            target: Box::new(DefaultAssignTargetV1::Local { local }),
            value: Box::new(unit(fixture)),
        },
    )
}

fn source_record(
    fixture: &Fixture,
    selector: LocalValueSelector,
    mutable: CanonicalBooleanV1,
) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        binder(),
        mutable,
        TemplateLocalDefinitionV1::Source(fixture.origin()),
    )
    .unwrap()
}

fn local_selector(ordinal: u32) -> LocalValueSelector {
    LocalValueSelector::LocalDeclaration {
        path: path(StructuralDefinitionSiteRole::LocalDeclaration, ordinal),
    }
}

fn path(role: StructuralDefinitionSiteRole, ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(StructuralPathSegment::new(role, ordinal), [])
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}
