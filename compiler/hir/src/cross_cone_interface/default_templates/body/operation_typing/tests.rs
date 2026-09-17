use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, CallingConvention, Effect, LocalValueSelector, SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalIntegerConstantV1,
    CanonicalTemplateLocalTableV1, CanonicalTemplateValueParametersV1, DefaultArrayAccessKindV1,
    DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1, DefaultExpressionKindV1,
    DefaultIntegerArgumentsV1, DefaultNoGcIntegerOperationV1, DefaultStatementV1,
    DefaultStringOwnerV1, ExportDefaultBodyV1, ExportDefaultReferenceSetV1,
    ExportDefaultTemplateKeyV1, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    TemplateLocalDefinitionV1, TemplateLocalRecordV1, TemplateValueParameterV1,
};

#[test]
fn validates_a_nested_option_and_boolean_expression_with_one_meter() {
    let fixture = Fixture::new();
    let string = core(DefaultOperationCoreTypeV1::String);
    let boolean = core(DefaultOperationCoreTypeV1::Boolean);
    let option = ty(100);
    let literal = expression(
        &fixture,
        DefaultExpressionKindV1::StringLiteral {
            value: "value".to_owned(),
            owner: DefaultStringOwnerV1::CurrentInstantiation,
        },
        string.clone(),
    );
    let some = expression(
        &fixture,
        DefaultExpressionKindV1::SomeWrap(Box::new(literal)),
        option.clone(),
    );
    let present = expression(
        &fixture,
        DefaultExpressionKindV1::IsSome(Box::new(some)),
        boolean.clone(),
    );
    let other = expression(
        &fixture,
        DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
        boolean.clone(),
    );
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::Binary {
            operator: crate::DefaultBinaryOperatorV1::And,
            lhs: Box::new(present),
            rhs: Box::new(other),
        },
        boolean,
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    authority
        .applications
        .push((option, DefaultCoreApplicationV1::Option { element: string }));
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        validate_with_meter(&template, &mut authority, &mut meter),
        Ok(())
    );
    assert_eq!(meter.usage().decoded_nodes, 5);
    assert_eq!(meter.usage().decoded_edges, 4);
    assert!(meter.usage().validation_work_units > meter.usage().decoded_nodes);
}

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
fn validates_integer_intrinsic_shape_and_dispatches_intrinsic_authority() {
    let fixture = Fixture::new();
    let integer = core(DefaultOperationCoreTypeV1::Integer(
        DefaultIntegerKindV1::Signed32,
    ));
    let operand = || {
        expression(
            &fixture,
            DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed32(1)),
            integer.clone(),
        )
    };
    let integer_operation = DefaultIntegerOperationV1::NoGc {
        kind: DefaultIntegerKindV1::Signed32,
        operation: DefaultNoGcIntegerOperationV1::Add,
        target: fixture.callable(),
    };
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::IntegerOperation {
            operation: integer_operation,
            arguments: DefaultIntegerArgumentsV1::binary(operand(), operand()),
        },
        integer.clone(),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    authority.callable = DefaultCallableOperationShapeV1::new(
        Effect::Ordinary,
        Some(integer.clone()),
        Vec::new(),
        vec![integer.clone()],
        integer,
    );

    assert_eq!(validate(&template, &mut authority), Ok(()));
    assert_eq!(authority.intrinsic_validations, 1);
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
fn enforces_expression_semantic_depth_before_scheduling_children() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let option = ty(130);
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::SomeWrap(Box::new(expression(
            &fixture,
            DefaultExpressionKindV1::UnitLiteral,
            unit.clone(),
        ))),
        option.clone(),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    authority
        .applications
        .push((option, DefaultCoreApplicationV1::Option { element: unit }));
    let path = WirePath::root().field(9);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let error = validate_with_meter(&template, &mut authority, &mut meter).unwrap_err();

    assert!(matches!(
        error,
        ExportDefaultOperationTypingValidationError::Resource(ref error)
            if error.kind()
                == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 1,
                    observed: 2,
                }
                && error.path() == &path
    ));
    assert_eq!(meter.usage().decoded_nodes, 1);
    assert_eq!(meter.usage().decoded_edges, 0);
}

fn validate(
    template: &ExportDefaultTemplateV1,
    authority: &mut Authority,
) -> Result<(), ExportDefaultOperationTypingValidationError<AuthorityError>> {
    validate_with_meter(
        template,
        authority,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
}

fn validate_with_meter(
    template: &ExportDefaultTemplateV1,
    authority: &mut Authority,
    meter: &mut BudgetMeter,
) -> Result<(), ExportDefaultOperationTypingValidationError<AuthorityError>> {
    template.body().value().validate_operation_typing_semantics(
        template,
        authority,
        meter,
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
    let result = value.result_type().clone();
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        definition_path(),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(Vec::<DefaultStatementV1>::new(), value).unwrap(),
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
                    self.field_type.clone(),
                    CanonicalBooleanV1::False,
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
