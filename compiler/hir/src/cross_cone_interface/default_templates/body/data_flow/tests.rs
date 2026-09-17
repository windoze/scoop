use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, LocalValueSelector, PersistentFieldId, SignatureTypeKey,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    SyntheticLocalRole,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::Fixture;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultAppliedOptionV1, DefaultAssignTargetV1,
    DefaultBindingActionV1, DefaultBindingLeafV1, DefaultBindingPlanV1, DefaultBindingProjectionV1,
    DefaultBindingShapeV1, DefaultBindingStructFieldV1, DefaultBindingTemporaryV1,
    DefaultCaptureV1, DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1,
    DefaultExpressionKindV1, DefaultExpressionV1, DefaultForIterationPlanV1,
    DefaultIteratorConformanceV1, DefaultIteratorNextV1, DefaultLocalFunctionV1, DefaultPatternV1,
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

    assert_eq!(validate(&template, &fixture), Ok(()));
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
        validate(&reachable, &fixture),
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
    assert_eq!(validate(&unreachable, &fixture), Ok(()));
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
        validate(&one_branch, &fixture),
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
    assert_eq!(validate(&both_branches, &fixture), Ok(()));
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
        validate(&assignment, &fixture),
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
        validate(&capture, &fixture),
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
        validate(&template, &fixture),
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
        validate(&outside, &fixture),
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
    assert_eq!(validate(&inside, &fixture), Ok(()));
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

    assert_eq!(validate(&template, &fixture), Ok(()));
}

#[test]
fn validates_a_complete_for_binding_plan_and_private_temporary_ownership() {
    let fixture = Fixture::new();
    let temporaries = distinct_for_selectors(10);
    let element = temporary(temporaries[4].clone());
    let leaf_selector = local_selector(0);
    let leaf =
        DefaultBindingLeafV1::new(leaf_selector.clone(), binder(), CanonicalBooleanV1::False);
    let binding = DefaultBindingPlanV1::try_new(
        element.clone(),
        DefaultBindingShapeV1::binding(leaf.clone()),
        vec![DefaultBindingActionV1::bind(
            element,
            leaf,
            fixture.origin(),
        )],
    )
    .unwrap();
    let plan = for_plan(
        &fixture,
        &temporaries,
        binding,
        vec![expression_statement(
            &fixture,
            local_expression(&fixture, leaf_selector.clone(), binder()),
        )],
    );
    let mut records = synthetic_records(&temporaries);
    records.push(source_record(
        &fixture,
        leaf_selector,
        CanonicalBooleanV1::False,
    ));
    let template = template(
        &fixture,
        records,
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::For(Box::new(plan)),
        )],
        unit(&fixture),
    );

    assert_eq!(validate(&template, &fixture), Ok(()));
}

#[test]
fn rejects_aliased_for_temporaries_and_actions_outside_the_binding_shape() {
    let fixture = Fixture::new();
    let mut aliased = distinct_for_selectors(20);
    aliased[1] = aliased[0].clone();
    let binding = DefaultBindingPlanV1::try_new(
        temporary(aliased[4].clone()),
        DefaultBindingShapeV1::wildcard(),
        Vec::new(),
    )
    .unwrap();
    let alias_plan = for_plan(&fixture, &aliased, binding, Vec::new());
    let alias_records = vec![
        synthetic_record(aliased[0].clone()),
        synthetic_record(aliased[2].clone()),
        synthetic_record(aliased[3].clone()),
        synthetic_record(aliased[4].clone()),
    ];
    let alias_template = template(
        &fixture,
        alias_records,
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::For(Box::new(alias_plan)),
        )],
        unit(&fixture),
    );
    assert_eq!(
        validate(&alias_template, &fixture),
        Err(
            ExportDefaultLocalDataFlowValidationError::ForTemporaryAlias {
                first: DefaultForTemporaryRoleV1::Source,
                second: DefaultForTemporaryRoleV1::ConformanceSource,
                selector: Box::new(aliased[0].clone()),
            }
        )
    );

    let temporaries = distinct_for_selectors(30);
    let element = temporary(temporaries[4].clone());
    let projected_selector = synthetic_selector(40);
    let projected = temporary(projected_selector.clone());
    let leaf_selector = local_selector(1);
    let leaf =
        DefaultBindingLeafV1::new(leaf_selector.clone(), binder(), CanonicalBooleanV1::False);
    let binding = DefaultBindingPlanV1::try_new(
        element.clone(),
        DefaultBindingShapeV1::binding(leaf.clone()),
        vec![
            DefaultBindingActionV1::bind(element.clone(), leaf, fixture.origin()),
            DefaultBindingActionV1::project(
                element,
                projected,
                DefaultBindingProjectionV1::tuple_index(0),
                fixture.origin(),
            ),
        ],
    )
    .unwrap();
    let extra_plan = for_plan(&fixture, &temporaries, binding, Vec::new());
    let mut extra_records = synthetic_records(&temporaries);
    extra_records.push(synthetic_record(projected_selector));
    extra_records.push(source_record(
        &fixture,
        leaf_selector,
        CanonicalBooleanV1::False,
    ));
    let extra_template = template(
        &fixture,
        extra_records,
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::For(Box::new(extra_plan)),
        )],
        unit(&fixture),
    );
    assert_eq!(
        validate(&extra_template, &fixture),
        Err(ExportDefaultLocalDataFlowValidationError::BindingShape(
            Box::new(DefaultBindingShapeDataFlowValidationError::ExtraAction { index: 1 })
        ))
    );
}

#[test]
fn maps_struct_binding_fields_through_authority_and_checks_the_applied_owner() {
    let fixture = Fixture::new();
    let owner = SignatureTypeKey::Nominal(fixture.type_id);
    let valid = struct_binding_template(&fixture, owner.clone());
    let mut authority = Authority::with_index(fixture.field, 7);
    let path = WirePath::root().field(8).index(0);
    assert_eq!(
        valid.validate_local_data_flow_semantics(
            &mut authority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &path,
        ),
        Ok(())
    );
    assert_eq!(authority.owners, vec![owner]);

    let wrong_owner = binder();
    let invalid = struct_binding_template(&fixture, wrong_owner.clone());
    let mut authority = Authority::with_index(fixture.field, 7);
    assert_eq!(
        invalid.validate_local_data_flow_semantics(
            &mut authority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &path,
        ),
        Err(ExportDefaultLocalDataFlowValidationError::BindingShape(
            Box::new(DefaultBindingShapeDataFlowValidationError::MissingAction {
                kind: DefaultBindingShapeActionKindV1::StructProjection,
            })
        ))
    );
}

#[test]
fn charges_the_callers_budget_and_preserves_the_callers_path() {
    let fixture = Fixture::new();
    let template = template(&fixture, Vec::new(), Vec::new(), Vec::new(), unit(&fixture));
    let path = WirePath::root().field(8).index(3).field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    let error = template
        .validate_local_data_flow_semantics(&mut Authority::new(fixture.field), &mut meter, &path)
        .unwrap_err();

    assert!(matches!(
        error,
        ExportDefaultLocalDataFlowValidationError::Resource(ref error)
            if error.kind()
                == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::ValidationWorkUnits,
                    limit: 0,
                    observed: 1,
                }
                && error.path() == &path
    ));
    assert_eq!(meter.usage().validation_work_units, 0);
}

fn validate(
    template: &ExportDefaultTemplateV1,
    fixture: &Fixture,
) -> Result<(), ExportDefaultLocalDataFlowValidationError<FieldIndexError>> {
    template.validate_local_data_flow_semantics(
        &mut Authority::new(fixture.field),
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root().field(8),
    )
}

fn struct_binding_template(
    fixture: &Fixture,
    projection_owner: SignatureTypeKey,
) -> ExportDefaultTemplateV1 {
    let temporaries = distinct_for_selectors(50);
    let element = temporary(temporaries[4].clone());
    let projected_selector = synthetic_selector(60);
    let projected = temporary(projected_selector.clone());
    let leaf_selector = local_selector(2);
    let leaf =
        DefaultBindingLeafV1::new(leaf_selector.clone(), binder(), CanonicalBooleanV1::False);
    let shape_owner = SignatureTypeKey::Nominal(fixture.type_id);
    let shape = DefaultBindingShapeV1::try_struct(
        shape_owner,
        vec![DefaultBindingStructFieldV1::new(
            7,
            DefaultBindingShapeV1::binding(leaf.clone()),
        )],
    )
    .unwrap();
    let binding = DefaultBindingPlanV1::try_new(
        element.clone(),
        shape,
        vec![
            DefaultBindingActionV1::project(
                element,
                projected.clone(),
                DefaultBindingProjectionV1::struct_field(fixture.field, projection_owner),
                fixture.origin(),
            ),
            DefaultBindingActionV1::bind(projected, leaf, fixture.origin()),
        ],
    )
    .unwrap();
    let plan = for_plan(fixture, &temporaries, binding, Vec::new());
    let mut records = synthetic_records(&temporaries);
    records.push(synthetic_record(projected_selector));
    records.push(source_record(
        fixture,
        leaf_selector,
        CanonicalBooleanV1::False,
    ));
    template(
        fixture,
        records,
        Vec::new(),
        vec![statement(
            fixture,
            DefaultStatementKindV1::For(Box::new(plan)),
        )],
        unit(fixture),
    )
}

fn for_plan(
    fixture: &Fixture,
    selectors: &[LocalValueSelector; 5],
    binding: DefaultBindingPlanV1,
    body: Vec<DefaultStatementV1>,
) -> DefaultForIterationPlanV1 {
    let source = temporary(selectors[0].clone());
    let conformance_source = temporary(selectors[1].clone());
    let iterator = temporary(selectors[2].clone());
    let next_result = temporary(selectors[3].clone());
    let next_element = temporary(selectors[4].clone());
    let conformance =
        DefaultIteratorConformanceV1::new(conformance_source, iterator, binder(), fixture.origin());
    let option = DefaultAppliedOptionV1::new(
        DefaultEnumVariantFieldRefV1::new(fixture.variant_field, binder()),
        DefaultEnumVariantRefV1::new(fixture.variant, binder()),
    );
    let next = DefaultIteratorNextV1::new(
        fixture.callable(),
        next_result,
        option,
        next_element,
        fixture.origin(),
    );
    DefaultForIterationPlanV1::try_new(
        Vec::new(),
        source,
        unit(fixture),
        Vec::new(),
        unit(fixture),
        conformance,
        next,
        binding,
        body,
    )
    .unwrap()
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
    DefaultExpressionV1::try_new(kind, value_type, fixture.origin()).unwrap()
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

fn synthetic_record(selector: LocalValueSelector) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        binder(),
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Synthetic,
    )
    .unwrap()
}

fn synthetic_records(selectors: &[LocalValueSelector; 5]) -> Vec<TemplateLocalRecordV1> {
    selectors.iter().cloned().map(synthetic_record).collect()
}

fn temporary(selector: LocalValueSelector) -> DefaultBindingTemporaryV1 {
    DefaultBindingTemporaryV1::new(selector, binder())
}

fn distinct_for_selectors(start: u32) -> [LocalValueSelector; 5] {
    std::array::from_fn(|offset| synthetic_selector(start + offset as u32))
}

fn local_selector(ordinal: u32) -> LocalValueSelector {
    LocalValueSelector::LocalDeclaration {
        path: path(StructuralDefinitionSiteRole::LocalDeclaration, ordinal),
    }
}

fn synthetic_selector(ordinal: u32) -> LocalValueSelector {
    LocalValueSelector::Synthetic {
        path: path(StructuralDefinitionSiteRole::SyntheticValue, ordinal),
        role: SyntheticLocalRole::Temporary,
    }
}

fn path(role: StructuralDefinitionSiteRole, ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(StructuralPathSegment::new(role, ordinal), [])
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}

struct Authority {
    field: PersistentFieldId,
    field_index: u32,
    owners: Vec<SignatureTypeKey>,
}

impl Authority {
    const fn new(field: PersistentFieldId) -> Self {
        Self {
            field,
            field_index: 0,
            owners: Vec::new(),
        }
    }

    const fn with_index(field: PersistentFieldId, field_index: u32) -> Self {
        Self {
            field,
            field_index,
            owners: Vec::new(),
        }
    }
}

impl DefaultLocalDataFlowSemanticAuthority<FieldIndexError> for Authority {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
    ) -> Result<u32, FieldIndexError> {
        self.owners.push(owner_type.clone());
        if declaration == self.field {
            Ok(self.field_index)
        } else {
            Err(FieldIndexError)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FieldIndexError;

impl fmt::Display for FieldIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("field is absent")
    }
}

impl std::error::Error for FieldIndexError {}
