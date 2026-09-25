use std::{fmt, num::NonZeroU32};

use scoop_identity::{
    CallableTemplateOrigin, CallingConvention, Effect, LocalValueSelector, SignatureTypeKey,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    SyntheticLocalRole,
};
use scoop_wire::WirePath;

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalConstValueV1,
    CanonicalIntegerConstantV1, CanonicalTemplateLocalTableV1, CanonicalTemplateValueParametersV1,
    DefaultAppliedOptionV1, DefaultArrayAccessKindV1, DefaultAssignTargetV1,
    DefaultBindingActionV1, DefaultBindingClassComponentV1, DefaultBindingLeafV1,
    DefaultBindingPlanV1, DefaultBindingProjectionV1, DefaultBindingShapeV1,
    DefaultBindingTemporaryV1, DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1,
    DefaultCatchV1, DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1, DefaultExpressionKindV1,
    DefaultFieldRefV1, DefaultForIterationPlanV1, DefaultIntegerArgumentsV1,
    DefaultIteratorConformanceV1, DefaultIteratorNextV1, DefaultNoGcIntegerOperationV1,
    DefaultPatternFieldV1, DefaultPatternV1, DefaultStatementKindV1, DefaultStatementV1,
    DefaultStringOwnerV1, DefaultTryV1, DefaultWhenFallbackV1, DefaultWhenV1, ExportDefaultBodyV1,
    ExportDefaultReferenceSetV1, ExportDefaultTemplateKeyV1, OptionalDefaultExpressionV1,
    OptionalDefaultStatementListV1, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    TemplateLocalDefinitionV1, TemplateLocalRecordV1, TemplateValueParameterV1,
};

mod receivers;

#[test]
fn permits_authorized_reference_retype_but_requires_explicit_function_coercion() {
    let fixture = Fixture::new();
    let local = fixture.local();
    let source = ty(40);
    let target = ty(41);
    let template = template_with_parameter_local(
        &fixture,
        local.clone(),
        source.clone(),
        expression(
            &fixture,
            DefaultExpressionKindV1::Local(local.clone()),
            target.clone(),
        ),
    );
    let mut authority = Authority::new();
    authority
        .relations
        .push(DefaultOperationTypeRelationV1::ReferenceRetype);

    assert_eq!(validate(&template, &mut authority), Ok(()));
    assert_eq!(
        authority.relation_queries,
        vec![(
            DefaultOperationTypeRelationV1::ReferenceRetype,
            source,
            target,
        )]
    );

    let source = function(Effect::Ordinary, vec![], ty(50));
    let target = function(Effect::Ordinary, vec![], ty(51));
    let template = template_with_parameter_local(
        &fixture,
        local.clone(),
        source,
        expression(&fixture, DefaultExpressionKindV1::Local(local), target),
    );

    assert!(matches!(
        validate(&template, &mut authority),
        Err(ExportDefaultOperationTypingValidationError::Problem {
            site,
            problem: DefaultOperationTypingProblemV1::FunctionCoercionRequired,
        }) if site.operation() == DefaultExpressionOperationV1::Local
            && site.role() == DefaultOperationValueRoleV1::Result
    ));
}

#[test]
fn accepts_distinct_reference_types_for_identity_comparison() {
    let fixture = Fixture::new();
    let left = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let right = LocalValueSelector::Parameter {
        declaration_index: 1,
    };
    let left_type = ty(60);
    let right_type = ty(61);
    let boolean = core(DefaultOperationCoreTypeV1::Boolean);
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::Binary {
            operator: crate::DefaultBinaryOperatorV1::RefEq,
            lhs: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::Local(left.clone()),
                left_type.clone(),
            )),
            rhs: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::Local(right.clone()),
                right_type.clone(),
            )),
        },
        boolean,
    );
    let template = template(
        &fixture,
        value,
        vec![
            local_record(&fixture, left.clone(), left_type.clone()),
            local_record(&fixture, right.clone(), right_type.clone()),
        ],
        vec![
            TemplateValueParameterV1::try_new(0, left).unwrap(),
            TemplateValueParameterV1::try_new(1, right).unwrap(),
        ],
        false,
    );
    let mut authority = Authority::new();
    authority
        .relations
        .push(DefaultOperationTypeRelationV1::ReferenceIdentity);

    assert_eq!(validate(&template, &mut authority), Ok(()));
    assert_eq!(
        authority.relation_queries,
        vec![(
            DefaultOperationTypeRelationV1::ReferenceIdentity,
            left_type,
            right_type,
        )]
    );
}

#[test]
fn validates_callable_reference_target_shape_without_executing_suspend_code() {
    let fixture = Fixture::new();
    let receiver_type = core(DefaultOperationCoreTypeV1::Unit);
    let parameter_type = core(DefaultOperationCoreTypeV1::Boolean);
    let result_type = core(DefaultOperationCoreTypeV1::String);
    let function_type = function(
        Effect::Suspend,
        vec![parameter_type.clone()],
        result_type.clone(),
    );
    let reference = DefaultCallableReferenceV1::try_new(
        fixture.generated,
        definition_path(),
        DefaultCallableReferenceTargetV1::BoundExtension {
            receiver: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::UnitLiteral,
                receiver_type.clone(),
            )),
            callee: fixture.callable(),
        },
        function_type.clone(),
        Vec::new(),
        0,
    )
    .unwrap();
    let template = template(
        &fixture,
        expression(
            &fixture,
            DefaultExpressionKindV1::CallableReference(reference),
            function_type,
        ),
        Vec::new(),
        Vec::new(),
        false,
    );
    let mut authority = Authority::new();
    authority.callable = DefaultCallableOperationShapeV1::new(
        Effect::Suspend,
        Some(receiver_type),
        Vec::new(),
        vec![parameter_type],
        result_type,
    );

    assert_eq!(validate(&template, &mut authority), Ok(()));
}

#[test]
fn rejects_suspend_invocation_when_the_template_does_not_allow_it() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::Call {
            receiver: crate::SourceCallReceiver::NoReceiver,
            callee: fixture.callable(),
            arguments: Vec::new(),
        },
        unit.clone(),
    );
    let rejecting_template = template(&fixture, value.clone(), Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    authority.callable =
        DefaultCallableOperationShapeV1::new(Effect::Suspend, None, Vec::new(), Vec::new(), unit);

    assert!(matches!(
        validate(&rejecting_template, &mut authority),
        Err(ExportDefaultOperationTypingValidationError::Problem {
            site,
            problem: DefaultOperationTypingProblemV1::SuspendNotAllowed,
        }) if site.operation() == DefaultExpressionOperationV1::Call
            && site.role() == DefaultOperationValueRoleV1::Callable
    ));

    let accepting_template = template(&fixture, value, Vec::new(), Vec::new(), true);
    assert_eq!(validate(&accepting_template, &mut authority), Ok(()));
}

#[test]
fn validates_array_clone_and_rejects_an_access_kind_mismatch() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let long = core(DefaultOperationCoreTypeV1::Integer(
        DefaultIntegerKindV1::Signed64,
    ));
    let array = ty(110);
    let mutable_array = ty(111);
    let literal = expression(
        &fixture,
        DefaultExpressionKindV1::ArrayLiteral(vec![expression(
            &fixture,
            DefaultExpressionKindV1::UnitLiteral,
            unit.clone(),
        )]),
        array.clone(),
    );
    let clone = expression(
        &fixture,
        DefaultExpressionKindV1::ArrayClone(Box::new(literal)),
        mutable_array.clone(),
    );
    let valid_template = template(&fixture, clone, Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    authority.applications.extend([
        (
            array,
            DefaultCoreApplicationV1::Array {
                element: unit.clone(),
            },
        ),
        (
            mutable_array.clone(),
            DefaultCoreApplicationV1::MutableArray {
                element: unit.clone(),
            },
        ),
    ]);

    assert_eq!(validate(&valid_template, &mut authority), Ok(()));

    let invalid = expression(
        &fixture,
        DefaultExpressionKindV1::ArraySet {
            access: DefaultArrayAccessKindV1::ImmutableGet,
            receiver: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::ArrayLiteral(Vec::new()),
                mutable_array,
            )),
            index: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed64(0)),
                long,
            )),
            value: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::UnitLiteral,
                unit.clone(),
            )),
        },
        unit,
    );
    let invalid_template = template(&fixture, invalid, Vec::new(), Vec::new(), false);

    assert!(matches!(
        validate(&invalid_template, &mut authority),
        Err(ExportDefaultOperationTypingValidationError::Problem {
            site,
            problem: DefaultOperationTypingProblemV1::InvalidArrayAccess,
        }) if site.operation() == DefaultExpressionOperationV1::ArraySet
            && site.role() == DefaultOperationValueRoleV1::Receiver
    ));
}

#[test]
fn checks_function_address_and_callback_result_shapes() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let invalid = template(
        &fixture,
        expression(
            &fixture,
            DefaultExpressionKindV1::FunctionAddress(DefaultCallableDeclarationV1::Function(
                fixture.function,
            )),
            unit.clone(),
        ),
        Vec::new(),
        Vec::new(),
        false,
    );
    let mut authority = Authority::new();
    authority.function_address = unit.clone();

    assert!(matches!(
        validate(&invalid, &mut authority),
        Err(ExportDefaultOperationTypingValidationError::TypeShape {
            site,
            expected: DefaultOperationExpectedTypeShapeV1::NativeFunctionPointer,
            ..
        }) if site.operation() == DefaultExpressionOperationV1::FunctionAddress
            && site.role() == DefaultOperationValueRoleV1::Target
    ));

    let closure_type = function(Effect::Ordinary, Vec::new(), unit.clone());
    let callback_type = ty(120);
    authority.global =
        DefaultValueOperationShapeV1::new(closure_type.clone(), CanonicalBooleanV1::False);
    authority.callback =
        DefaultCallbackOperationShapeV1::new(closure_type.clone(), callback_type.clone());
    authority.applications.push((
        callback_type.clone(),
        DefaultCoreApplicationV1::ForeignCallback {
            function_type: closure_type,
        },
    ));
    let callback = template(
        &fixture,
        expression(
            &fixture,
            DefaultExpressionKindV1::ForeignCallbackRegister {
                registration: fixture.callback,
                closure: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::GlobalRead(fixture.property),
                    authority.global.value_type().clone(),
                )),
            },
            callback_type,
        ),
        Vec::new(),
        Vec::new(),
        false,
    );

    assert_eq!(validate(&callback, &mut authority), Ok(()));
}

#[test]
fn rejects_field_authority_kind_mismatch() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let field = crate::DefaultFieldRefV1::Struct {
        declaration: fixture.field,
        owner_type: unit.clone(),
    };
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::FieldAccess {
            receiver: Box::new(expression(
                &fixture,
                DefaultExpressionKindV1::UnitLiteral,
                unit.clone(),
            )),
            field,
        },
        unit.clone(),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    authority.field_kind = DefaultFieldOperationKindV1::Class;

    assert!(matches!(
        validate(&template, &mut authority),
        Err(ExportDefaultOperationTypingValidationError::Problem {
            site,
            problem: DefaultOperationTypingProblemV1::FieldKindMismatch {
                expected: DefaultFieldOperationKindV1::Struct,
                actual: DefaultFieldOperationKindV1::Class,
            },
        }) if site.operation() == DefaultExpressionOperationV1::FieldAccess
            && site.role() == DefaultOperationValueRoleV1::Target
    ));
}

#[test]
fn validates_statement_conditions_and_tuple_pattern_bindings() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let boolean = core(DefaultOperationCoreTypeV1::Boolean);
    let tuple = SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
        unit.clone(),
        [boolean.clone()],
    ));
    let unit_local = local_selector(0);
    let boolean_local = local_selector(1);
    let pattern = DefaultPatternV1::try_tuple(vec![
        DefaultPatternV1::binding(unit_local.clone()),
        DefaultPatternV1::binding(boolean_local.clone()),
    ])
    .unwrap();
    let init = expression(
        &fixture,
        DefaultExpressionKindV1::TupleLiteral(vec![
            expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
            expression(
                &fixture,
                DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
                boolean.clone(),
            ),
        ]),
        tuple,
    );
    let statements = vec![
        statement(
            &fixture,
            DefaultStatementKindV1::ValDecl {
                pattern,
                init: Box::new(init),
            },
        ),
        statement(
            &fixture,
            DefaultStatementKindV1::If {
                condition: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
                    boolean,
                )),
                then_body: Vec::new(),
                else_body: OptionalDefaultStatementListV1::absent(),
            },
        ),
    ];
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        vec![
            local_record(&fixture, unit_local, unit),
            local_record(
                &fixture,
                boolean_local,
                core(DefaultOperationCoreTypeV1::Boolean),
            ),
        ],
        Vec::new(),
        statements,
        false,
    );

    assert_eq!(validate_body(&template, &mut Authority::new()), Ok(()));
}

#[test]
fn rejects_missing_return_value_for_non_unit_provider() {
    let fixture = Fixture::new();
    let string = core(DefaultOperationCoreTypeV1::String);
    let template = template_with_statements(
        &fixture,
        expression(
            &fixture,
            DefaultExpressionKindV1::StringLiteral {
                value: "result".to_owned(),
                owner: DefaultStringOwnerV1::CurrentInstantiation,
            },
            string,
        ),
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::absent()),
        )],
        false,
    );

    assert!(matches!(
        validate_body(&template, &mut Authority::new()),
        Err(ExportDefaultBodyOperationTypingValidationError::Problem {
            site,
            problem: DefaultBodyOperationTypingProblemV1::MissingReturnValue,
        }) if site.operation()
            == DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::Return)
            && site.role() == DefaultOperationValueRoleV1::ReturnValue
    ));
}

#[test]
fn rejects_non_boolean_statement_condition() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::While {
                condition_setup: Vec::new(),
                condition: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::UnitLiteral,
                    unit,
                )),
                body: Vec::new(),
            },
        )],
        false,
    );

    assert!(matches!(
        validate_body(&template, &mut Authority::new()),
        Err(ExportDefaultBodyOperationTypingValidationError::Type { site, .. })
            if site.operation()
                == DefaultBodyOperationV1::Statement(
                    DefaultStatementOperationV1::WhileCondition,
                )
                && site.role() == DefaultOperationValueRoleV1::Condition
    ));
}

#[test]
fn rejects_assignment_to_an_immutable_global() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::Assign {
                target: Box::new(DefaultAssignTargetV1::Global {
                    property: fixture.property,
                }),
                value: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::UnitLiteral,
                    unit,
                )),
            },
        )],
        false,
    );

    assert!(matches!(
        validate_body(&template, &mut Authority::new()),
        Err(ExportDefaultBodyOperationTypingValidationError::Problem {
            site,
            problem: DefaultBodyOperationTypingProblemV1::ImmutableAssignmentTarget,
        }) if site.operation()
            == DefaultBodyOperationV1::Assignment(DefaultAssignmentOperationV1::Global)
    ));
}

#[test]
fn validates_throw_and_catch_against_throwable() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let exception = ty(301);
    let catch_local = local_selector(2);
    let try_statement = statement(
        &fixture,
        DefaultStatementKindV1::Try(Box::new(
            DefaultTryV1::try_new(
                vec![statement(
                    &fixture,
                    DefaultStatementKindV1::Throw(Box::new(expression(
                        &fixture,
                        DefaultExpressionKindV1::GlobalRead(fixture.property),
                        exception.clone(),
                    ))),
                )],
                vec![
                    DefaultCatchV1::try_new(
                        catch_local.clone(),
                        exception.clone(),
                        Vec::new(),
                        fixture.origin(),
                    )
                    .unwrap(),
                ],
                OptionalDefaultStatementListV1::absent(),
            )
            .unwrap(),
        )),
    );
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit),
        vec![local_record(&fixture, catch_local, exception.clone())],
        Vec::new(),
        vec![try_statement],
        false,
    );
    let mut authority = Authority::new();
    authority.global = DefaultValueOperationShapeV1::new(exception, CanonicalBooleanV1::False);
    authority
        .relations
        .push(DefaultOperationTypeRelationV1::ReferenceRetype);

    assert_eq!(validate_body(&template, &mut authority), Ok(()));
    assert_eq!(
        authority
            .relation_queries
            .iter()
            .filter(|(relation, _, _)| {
                *relation == DefaultOperationTypeRelationV1::ReferenceRetype
            })
            .count(),
        2
    );
}

#[test]
fn validates_variant_and_literal_pattern_roles() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let string = core(DefaultOperationCoreTypeV1::String);
    let enum_type = ty(304);
    let literal = DefaultPatternV1::literal(
        CanonicalConstValueV1::String("case".to_owned()),
        DefaultLiteralEqualityV1::Ordinary {
            target: fixture.callable(),
        },
        string.clone(),
    );
    let pattern = DefaultPatternV1::try_variant(
        DefaultEnumVariantRefV1::new(fixture.variant, enum_type.clone()),
        vec![DefaultPatternFieldV1::new(0, literal)],
    )
    .unwrap();
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit),
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::ValDecl {
                pattern,
                init: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::GlobalRead(fixture.property),
                    enum_type.clone(),
                )),
            },
        )],
        false,
    );
    let mut authority = Authority::new();
    authority.global = DefaultValueOperationShapeV1::new(enum_type, CanonicalBooleanV1::False);
    authority.aggregate_fields = vec![string];

    assert_eq!(validate_body(&template, &mut authority), Ok(()));
    assert_eq!(authority.intrinsic_validations, 1);
}

#[test]
fn rejects_out_of_bounds_struct_pattern_field() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let owner = ty(305);
    let pattern = DefaultPatternV1::try_struct(
        owner.clone(),
        vec![DefaultPatternFieldV1::new(1, DefaultPatternV1::wildcard())],
    )
    .unwrap();
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::ValDecl {
                pattern,
                init: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::GlobalRead(fixture.property),
                    owner.clone(),
                )),
            },
        )],
        false,
    );
    let mut authority = Authority::new();
    authority.global = DefaultValueOperationShapeV1::new(owner, CanonicalBooleanV1::False);
    authority.aggregate_fields = vec![unit];

    assert!(matches!(
        validate_body(&template, &mut authority),
        Err(ExportDefaultBodyOperationTypingValidationError::Problem {
            site,
            problem: DefaultBodyOperationTypingProblemV1::IndexOutOfBounds {
                index: 1,
                length: 1,
            },
        }) if site.operation()
            == DefaultBodyOperationV1::Pattern(DefaultPatternOperationV1::Struct)
    ));
}

#[test]
fn validates_mutable_array_and_class_field_assignments() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let long = core(DefaultOperationCoreTypeV1::Integer(
        DefaultIntegerKindV1::Signed64,
    ));
    let mutable_array = ty(306);
    let array_local = LocalValueSelector::Parameter {
        declaration_index: 0,
    };
    let class_type = ty(307);
    let class_local = LocalValueSelector::Parameter {
        declaration_index: 1,
    };
    let statements = vec![
        statement(
            &fixture,
            DefaultStatementKindV1::Assign {
                target: Box::new(DefaultAssignTargetV1::Index {
                    array: Box::new(expression(
                        &fixture,
                        DefaultExpressionKindV1::Local(array_local.clone()),
                        mutable_array.clone(),
                    )),
                    index: Box::new(expression(
                        &fixture,
                        DefaultExpressionKindV1::IntegerLiteral(
                            CanonicalIntegerConstantV1::Signed64(0),
                        ),
                        long,
                    )),
                }),
                value: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::UnitLiteral,
                    unit.clone(),
                )),
            },
        ),
        statement(
            &fixture,
            DefaultStatementKindV1::Assign {
                target: Box::new(DefaultAssignTargetV1::Field {
                    receiver: Box::new(expression(
                        &fixture,
                        DefaultExpressionKindV1::Local(class_local.clone()),
                        class_type.clone(),
                    )),
                    field: DefaultFieldRefV1::Class {
                        declaration: fixture.field,
                        owner_type: class_type.clone(),
                    },
                }),
                value: Box::new(expression(
                    &fixture,
                    DefaultExpressionKindV1::UnitLiteral,
                    unit.clone(),
                )),
            },
        ),
    ];
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        vec![
            local_record(&fixture, array_local.clone(), mutable_array.clone()),
            local_record(&fixture, class_local.clone(), class_type.clone()),
        ],
        vec![
            TemplateValueParameterV1::try_new(0, array_local).unwrap(),
            TemplateValueParameterV1::try_new(1, class_local).unwrap(),
        ],
        statements,
        false,
    );
    let mut authority = Authority::new();
    authority.applications.push((
        mutable_array,
        DefaultCoreApplicationV1::MutableArray {
            element: unit.clone(),
        },
    ));
    authority.field_kind = DefaultFieldOperationKindV1::Class;
    authority.field_type = unit;
    authority.field_mutable = CanonicalBooleanV1::True;
    authority
        .relations
        .push(DefaultOperationTypeRelationV1::MemberReceiver);

    assert_eq!(validate_body(&template, &mut authority), Ok(()));
}

#[test]
fn rejects_when_enum_fallback_for_a_different_owner_application() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let subject_type = ty(308);
    let other_owner = ty(309);
    let when = DefaultWhenV1::try_new(
        expression(
            &fixture,
            DefaultExpressionKindV1::GlobalRead(fixture.property),
            subject_type.clone(),
        ),
        Vec::new(),
        DefaultWhenFallbackV1::enum_pattern_matrix(subject_type.clone(), other_owner),
    )
    .unwrap();
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit),
        Vec::new(),
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::When(Box::new(when)),
        )],
        false,
    );
    let mut authority = Authority::new();
    authority.global = DefaultValueOperationShapeV1::new(subject_type, CanonicalBooleanV1::False);

    assert!(matches!(
        validate_body(&template, &mut authority),
        Err(ExportDefaultBodyOperationTypingValidationError::Type { site, .. })
            if site.operation()
                == DefaultBodyOperationV1::Statement(
                    DefaultStatementOperationV1::WhenFallback,
                )
                && site.role() == DefaultOperationValueRoleV1::Subject
    ));
}

#[test]
fn validates_for_protocol_and_binding_component_projection_chain() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let boolean = core(DefaultOperationCoreTypeV1::Boolean);
    let raw_iterator = ty(310);
    let iterator = ty(311);
    let component_result = SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
        unit.clone(),
        [boolean.clone()],
    ));
    let element = ty(313);
    let option = ty(312);
    let selectors: [LocalValueSelector; 5] =
        std::array::from_fn(|offset| synthetic_selector(20 + offset as u32));
    let source = DefaultBindingTemporaryV1::new(selectors[0].clone(), unit.clone());
    let conformance_source =
        DefaultBindingTemporaryV1::new(selectors[1].clone(), raw_iterator.clone());
    let iterator_temporary = DefaultBindingTemporaryV1::new(selectors[2].clone(), iterator.clone());
    let next_result = DefaultBindingTemporaryV1::new(selectors[3].clone(), option.clone());
    let next_element = DefaultBindingTemporaryV1::new(selectors[4].clone(), element.clone());
    let projection_selector = synthetic_selector(30);
    let projection = DefaultBindingTemporaryV1::new(projection_selector.clone(), unit.clone());
    let component_selector = synthetic_selector(31);
    let component =
        DefaultBindingTemporaryV1::new(component_selector.clone(), component_result.clone());
    let leaf_selector = local_selector(3);
    let leaf = DefaultBindingLeafV1::new(
        leaf_selector.clone(),
        unit.clone(),
        CanonicalBooleanV1::False,
    );
    let binding = DefaultBindingPlanV1::try_new(
        next_element.clone(),
        DefaultBindingShapeV1::try_class(
            element.clone(),
            vec![DefaultBindingClassComponentV1::new(
                NonZeroU32::new(1).unwrap(),
                DefaultBindingShapeV1::try_tuple(vec![
                    DefaultBindingShapeV1::binding(leaf.clone()),
                    DefaultBindingShapeV1::wildcard(),
                ])
                .unwrap(),
            )],
        )
        .unwrap(),
        vec![
            DefaultBindingActionV1::try_component(
                next_element.clone(),
                NonZeroU32::new(1).unwrap(),
                component.clone(),
                Vec::new(),
                expression(
                    &fixture,
                    DefaultExpressionKindV1::TupleLiteral(vec![
                        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
                        expression(
                            &fixture,
                            DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
                            boolean,
                        ),
                    ]),
                    component_result.clone(),
                ),
                fixture.origin(),
            )
            .unwrap(),
            DefaultBindingActionV1::project(
                component,
                projection.clone(),
                DefaultBindingProjectionV1::tuple_index(0),
                fixture.origin(),
            ),
            DefaultBindingActionV1::bind(projection, leaf, fixture.origin()),
        ],
    )
    .unwrap();
    let conformance = DefaultIteratorConformanceV1::new(
        conformance_source,
        iterator_temporary,
        iterator.clone(),
        fixture.origin(),
    );
    let next = DefaultIteratorNextV1::new(
        fixture.callable(),
        next_result,
        DefaultAppliedOptionV1::new(
            DefaultEnumVariantFieldRefV1::new(fixture.variant_field, option.clone()),
            DefaultEnumVariantRefV1::new(fixture.variant, option.clone()),
        ),
        next_element,
        fixture.origin(),
    );
    let plan = DefaultForIterationPlanV1::try_new(
        Vec::new(),
        source,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        Vec::new(),
        expression(
            &fixture,
            DefaultExpressionKindV1::GlobalRead(fixture.property),
            raw_iterator.clone(),
        ),
        conformance,
        next,
        binding,
        Vec::new(),
    )
    .unwrap();
    let mut records = vec![
        synthetic_record(selectors[0].clone(), unit.clone()),
        synthetic_record(selectors[1].clone(), raw_iterator.clone()),
        synthetic_record(selectors[2].clone(), iterator.clone()),
        synthetic_record(selectors[3].clone(), option.clone()),
        synthetic_record(selectors[4].clone(), element.clone()),
        synthetic_record(component_selector, component_result),
        synthetic_record(projection_selector, unit.clone()),
        local_record(&fixture, leaf_selector, unit.clone()),
    ];
    records.shrink_to_fit();
    let template = template_with_statements(
        &fixture,
        expression(&fixture, DefaultExpressionKindV1::UnitLiteral, unit.clone()),
        records,
        Vec::new(),
        vec![statement(
            &fixture,
            DefaultStatementKindV1::For(Box::new(plan)),
        )],
        false,
    );
    let mut authority = Authority::new();
    authority.global = DefaultValueOperationShapeV1::new(raw_iterator, CanonicalBooleanV1::False);
    authority.callable = DefaultCallableOperationShapeV1::new(
        Effect::Ordinary,
        Some(iterator),
        Vec::new(),
        Vec::new(),
        option.clone(),
    );
    authority.variant_field_type = element.clone();
    authority
        .applications
        .push((option, DefaultCoreApplicationV1::Option { element }));
    authority
        .relations
        .push(DefaultOperationTypeRelationV1::IteratorConformance);

    assert_eq!(validate_body(&template, &mut authority), Ok(()));
    assert_eq!(authority.intrinsic_validations, 2);
}

fn validate_body(
    template: &ExportDefaultTemplateV1,
    authority: &mut Authority,
) -> Result<(), ExportDefaultBodyOperationTypingValidationError<AuthorityError>> {
    template.body().validate_operation_typing_semantics(
        template,
        authority,
        &WirePath::root().field(10),
    )
}

fn validate(
    template: &ExportDefaultTemplateV1,
    authority: &mut Authority,
) -> Result<(), ExportDefaultOperationTypingValidationError<AuthorityError>> {
    template.body().value().validate_operation_typing_semantics(
        template,
        authority,
        &WirePath::root().field(9),
    )
}

fn template_with_parameter_local(
    fixture: &Fixture,
    local: LocalValueSelector,
    local_type: SignatureTypeKey,
    value: DefaultExpressionV1,
) -> ExportDefaultTemplateV1 {
    template(
        fixture,
        value,
        vec![local_record(fixture, local.clone(), local_type)],
        vec![TemplateValueParameterV1::try_new(0, local).unwrap()],
        false,
    )
}

fn template(
    fixture: &Fixture,
    value: DefaultExpressionV1,
    locals: Vec<TemplateLocalRecordV1>,
    parameters: Vec<TemplateValueParameterV1>,
    allows_suspend: bool,
) -> ExportDefaultTemplateV1 {
    template_with_statements(
        fixture,
        value,
        locals,
        parameters,
        Vec::new(),
        allows_suspend,
    )
}

fn template_with_statements(
    fixture: &Fixture,
    value: DefaultExpressionV1,
    locals: Vec<TemplateLocalRecordV1>,
    parameters: Vec<TemplateValueParameterV1>,
    statements: Vec<DefaultStatementV1>,
    allows_suspend: bool,
) -> ExportDefaultTemplateV1 {
    let result = value.result_type().clone();
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        definition_path(),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(statements, value).unwrap(),
        result,
        CanonicalBooleanV1::from(allows_suspend),
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(parameters).unwrap(),
        ExportDefaultReferenceSetV1::default(),
        fixture.origin(),
    )
    .unwrap()
}

fn statement(fixture: &Fixture, kind: DefaultStatementKindV1) -> DefaultStatementV1 {
    DefaultStatementV1::try_new(kind, fixture.origin()).unwrap()
}

fn local_selector(ordinal: u32) -> LocalValueSelector {
    LocalValueSelector::LocalDeclaration {
        path: local_path(StructuralDefinitionSiteRole::LocalDeclaration, ordinal),
    }
}

fn synthetic_selector(ordinal: u32) -> LocalValueSelector {
    LocalValueSelector::Synthetic {
        path: local_path(StructuralDefinitionSiteRole::SyntheticValue, ordinal),
        role: SyntheticLocalRole::Temporary,
    }
}

fn local_path(role: StructuralDefinitionSiteRole, ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(StructuralPathSegment::new(role, ordinal), [])
}

fn synthetic_record(
    selector: LocalValueSelector,
    value_type: SignatureTypeKey,
) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        value_type,
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Synthetic,
    )
    .unwrap()
}

fn local_record(
    fixture: &Fixture,
    selector: LocalValueSelector,
    value_type: SignatureTypeKey,
) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        value_type,
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Source(fixture.origin()),
    )
    .unwrap()
}

fn expression(
    fixture: &Fixture,
    kind: DefaultExpressionKindV1,
    result_type: SignatureTypeKey,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, result_type, fixture.origin()).unwrap()
}

fn function(
    effect: Effect,
    parameters: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::Function {
        effect,
        parameters,
        result: Box::new(result),
    }
}

const fn ty(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

const fn core(role: DefaultOperationCoreTypeV1) -> SignatureTypeKey {
    let index = match role {
        DefaultOperationCoreTypeV1::Unit => 0,
        DefaultOperationCoreTypeV1::Boolean => 1,
        DefaultOperationCoreTypeV1::String => 2,
        DefaultOperationCoreTypeV1::Integer(kind) => match kind {
            DefaultIntegerKindV1::Signed8 => 10,
            DefaultIntegerKindV1::Signed16 => 11,
            DefaultIntegerKindV1::Signed32 => 12,
            DefaultIntegerKindV1::Signed64 => 13,
            DefaultIntegerKindV1::Unsigned8 => 14,
            DefaultIntegerKindV1::Unsigned16 => 15,
            DefaultIntegerKindV1::Unsigned32 => 16,
            DefaultIntegerKindV1::Unsigned64 => 17,
        },
        DefaultOperationCoreTypeV1::Throwable => 20,
        DefaultOperationCoreTypeV1::ForeignCallbackState => 21,
    };
    ty(index)
}

struct Authority {
    callable: DefaultCallableOperationShapeV1,
    method: DefaultCallableOperationShapeV1,
    constructor_parameters: Vec<SignatureTypeKey>,
    aggregate_fields: Vec<SignatureTypeKey>,
    variant_field_type: SignatureTypeKey,
    global: DefaultValueOperationShapeV1,
    singleton: SignatureTypeKey,
    field_kind: DefaultFieldOperationKindV1,
    field_type: SignatureTypeKey,
    field_mutable: CanonicalBooleanV1,
    function_address: SignatureTypeKey,
    callback: DefaultCallbackOperationShapeV1,
    applications: Vec<(SignatureTypeKey, DefaultCoreApplicationV1)>,
    relations: Vec<DefaultOperationTypeRelationV1>,
    relation_queries: Vec<(
        DefaultOperationTypeRelationV1,
        SignatureTypeKey,
        SignatureTypeKey,
    )>,
    intrinsic_validations: usize,
}

impl Authority {
    fn new() -> Self {
        let unit = core(DefaultOperationCoreTypeV1::Unit);
        let closure = function(Effect::Ordinary, Vec::new(), unit.clone());
        let callback = ty(200);
        Self {
            callable: DefaultCallableOperationShapeV1::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                Vec::new(),
                unit.clone(),
            ),
            method: DefaultCallableOperationShapeV1::new(
                Effect::Ordinary,
                Some(unit.clone()),
                Vec::new(),
                Vec::new(),
                unit.clone(),
            ),
            constructor_parameters: Vec::new(),
            aggregate_fields: Vec::new(),
            variant_field_type: unit.clone(),
            global: DefaultValueOperationShapeV1::new(unit.clone(), CanonicalBooleanV1::False),
            singleton: unit.clone(),
            field_kind: DefaultFieldOperationKindV1::Struct,
            field_type: unit.clone(),
            field_mutable: CanonicalBooleanV1::False,
            function_address: SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: Vec::new(),
                result: Box::new(unit),
            },
            callback: DefaultCallbackOperationShapeV1::new(closure.clone(), callback.clone()),
            applications: vec![(
                callback,
                DefaultCoreApplicationV1::ForeignCallback {
                    function_type: closure,
                },
            )],
            relations: Vec::new(),
            relation_queries: Vec::new(),
            intrinsic_validations: 0,
        }
    }
}

impl DefaultOperationTypingSemanticAuthority<AuthorityError> for Authority {
    fn canonical_default_operation_type(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        role: DefaultOperationCoreTypeV1,
    ) -> Result<SignatureTypeKey, AuthorityError> {
        Ok(core(role))
    }

    fn classify_default_core_application(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        value: &SignatureTypeKey,
    ) -> Result<Option<DefaultCoreApplicationV1>, AuthorityError> {
        Ok(self
            .applications
            .iter()
            .find(|(candidate, _)| candidate == value)
            .map(|(_, application)| application.clone()))
    }

    fn default_operation_entity_shape(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        entity: DefaultOperationEntityV1<'_>,
    ) -> Result<DefaultOperationEntityShapeV1, AuthorityError> {
        Ok(match entity {
            DefaultOperationEntityV1::Callable(_) => {
                DefaultOperationEntityShapeV1::Callable(self.callable.clone())
            }
            DefaultOperationEntityV1::MethodCallee(_) => {
                DefaultOperationEntityShapeV1::Callable(self.method.clone())
            }
            DefaultOperationEntityV1::Constructor(constructor) => {
                DefaultOperationEntityShapeV1::Constructor {
                    owner_type: constructor.owner_type().clone(),
                    parameters: self.constructor_parameters.clone(),
                }
            }
            DefaultOperationEntityV1::Struct(owner) => DefaultOperationEntityShapeV1::Aggregate(
                DefaultAggregateOperationShapeV1::new(owner.clone(), self.aggregate_fields.clone()),
            ),
            DefaultOperationEntityV1::Class(owner) | DefaultOperationEntityV1::Enum(owner) => {
                DefaultOperationEntityShapeV1::Type(owner.clone())
            }
            DefaultOperationEntityV1::Variant(variant) => {
                DefaultOperationEntityShapeV1::Aggregate(DefaultAggregateOperationShapeV1::new(
                    variant.owner_type().clone(),
                    self.aggregate_fields.clone(),
                ))
            }
            DefaultOperationEntityV1::VariantField(field) => {
                DefaultOperationEntityShapeV1::VariantField(
                    DefaultVariantFieldOperationShapeV1::new(
                        field.owner_type().clone(),
                        0,
                        self.variant_field_type.clone(),
                    ),
                )
            }
            DefaultOperationEntityV1::Global(_) => {
                DefaultOperationEntityShapeV1::Value(self.global.clone())
            }
            DefaultOperationEntityV1::Singleton(_) => {
                DefaultOperationEntityShapeV1::Type(self.singleton.clone())
            }
            DefaultOperationEntityV1::Field(field) => {
                let owner_type = match field {
                    crate::DefaultFieldRefV1::Struct { owner_type, .. }
                    | crate::DefaultFieldRefV1::Class { owner_type, .. } => owner_type.clone(),
                    crate::DefaultFieldRefV1::Tuple { .. } => return Err(AuthorityError),
                };
                DefaultOperationEntityShapeV1::Field(DefaultFieldOperationShapeV1::new(
                    self.field_kind,
                    owner_type,
                    0,
                    self.field_type.clone(),
                    self.field_mutable,
                ))
            }
            DefaultOperationEntityV1::FunctionAddress(_) => {
                DefaultOperationEntityShapeV1::Type(self.function_address.clone())
            }
            DefaultOperationEntityV1::CallbackRegistration(_) => {
                DefaultOperationEntityShapeV1::CallbackRegistration(self.callback.clone())
            }
        })
    }

    fn default_operation_type_relation(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
    ) -> Result<bool, AuthorityError> {
        self.relation_queries
            .push((relation, source.clone(), target.clone()));
        Ok(self.relations.contains(&relation))
    }

    fn validate_default_operation_intrinsic(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _intrinsic: DefaultOperationIntrinsicV1<'_>,
    ) -> Result<(), AuthorityError> {
        self.intrinsic_validations += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthorityError;

impl fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("operation authority rejected the query")
    }
}

impl std::error::Error for AuthorityError {}

#[path = "integer_tests.rs"]
mod integers;

mod casts;
