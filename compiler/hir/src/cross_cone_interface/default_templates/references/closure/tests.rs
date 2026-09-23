use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOrigin, Effect, GcEffect, LocalValueSelector,
    NominalDeclarationOwner, NormalizedSourcePath, OptionalSignatureType, SignatureTypeKey,
    SourceContextKey, SourceIdentity, SourceSpan, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableModalityV1, CallableOperatorRoleV1,
    CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1, CanonicalBinderUseListV1,
    CanonicalBooleanV1, CanonicalSourceParameterShapesV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultCallableReferenceTargetV1,
    DefaultCallableReferenceV1, DefaultConstructorRefV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultFieldRefV1, ExportDefaultAccessWitnessV1, ExportDefaultBodyV1,
    ExportDefaultCallableReferenceV1, ExportDefaultConstructorReferenceV1,
    ExportDefaultFieldReferenceV1, ExportDefaultGlobalReferenceV1, ExportDefaultReferenceV1,
    ExportDefaultSingletonReferenceV1, ExportDefaultTemplateKeyV1, ExportDefaultTypeReferenceV1,
    OptionalTemplateReceiverV1, PersistentLexicalRootV1, PublicDeclarationOwnerV1,
    PublicLookupAccessV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};

#[test]
fn accepts_the_exact_deduplicated_six_domain_closure() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let nominal = SignatureTypeKey::Nominal(fixture.type_id);
    let constructor = DefaultConstructorRefV1::Struct {
        declaration: fixture.constructor,
        owner_type: nominal.clone(),
    };
    let field = DefaultFieldRefV1::Struct {
        declaration: fixture.field,
        owner_type: nominal.clone(),
    };
    let values = vec![
        expression(
            DefaultExpressionKindV1::Call {
                callee: fixture.callable(),
                arguments: Vec::new(),
            },
            nominal.clone(),
            origin.clone(),
        ),
        expression(
            DefaultExpressionKindV1::StructInit {
                constructor: constructor.clone(),
                arguments: Vec::new(),
            },
            nominal.clone(),
            origin.clone(),
        ),
        expression(
            DefaultExpressionKindV1::GlobalRead(fixture.property),
            nominal.clone(),
            origin.clone(),
        ),
        expression(
            DefaultExpressionKindV1::GlobalRead(fixture.property),
            nominal.clone(),
            origin.clone(),
        ),
        expression(
            DefaultExpressionKindV1::SingletonValue(fixture.object),
            nominal.clone(),
            origin.clone(),
        ),
        expression(
            DefaultExpressionKindV1::FieldAccess {
                receiver: Box::new(expression(
                    DefaultExpressionKindV1::UnitLiteral,
                    nominal.clone(),
                    origin.clone(),
                )),
                field: field.clone(),
            },
            nominal.clone(),
            origin.clone(),
        ),
    ];
    let global_body = body(
        DefaultExpressionKindV1::TupleLiteral(values),
        nominal.clone(),
        origin.clone(),
    );
    let references = reference_set(
        vec![reference(
            ExportDefaultCallableTargetV1::Callable(fixture.callable()),
            &origin,
            &fixture,
        )],
        vec![reference(constructor, &origin, &fixture)],
        vec![reference(nominal, &origin, &fixture)],
        vec![reference(fixture.property, &origin, &fixture)],
        vec![reference(fixture.object, &origin, &fixture)],
        vec![reference(field, &origin, &fixture)],
    );
    let template = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        global_body,
        references,
        origin,
    );
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(validate(&template, &fixture, &mut meter), Ok(()));
    assert!(meter.usage().decoded_nodes > 12);
    assert!(meter.usage().decoded_edges > 12);
}

#[test]
fn rejects_missing_wrong_origin_and_extra_records() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let other_origin = origin_at("src/Other.scoop", 30);
    let referenced_body = body(
        DefaultExpressionKindV1::GlobalRead(fixture.property),
        binder(),
        origin.clone(),
    );
    let wrong_origin = reference_set(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![reference(fixture.property, &other_origin, &fixture)],
        Vec::new(),
        Vec::new(),
    );
    let template_with_wrong_origin = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        referenced_body,
        wrong_origin,
        origin.clone(),
    );

    assert!(matches!(
        validate(
            &template_with_wrong_origin,
            &fixture,
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Err(ExportDefaultReferenceClosureValidationError::Missing {
            kind: ExportDefaultReferenceKindV1::Global,
            site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
            definition_origin,
            ..
        }) if definition_origin.as_ref() == &origin
    ));

    let empty_body = body(
        DefaultExpressionKindV1::UnitLiteral,
        binder(),
        origin.clone(),
    );
    let extra = reference_set(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![reference(fixture.property, &origin, &fixture)],
        Vec::new(),
        Vec::new(),
    );
    let template_with_extra = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        empty_body,
        extra,
        origin,
    );

    assert_eq!(
        validate(
            &template_with_extra,
            &fixture,
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Err(ExportDefaultReferenceClosureValidationError::Extra {
            kind: ExportDefaultReferenceKindV1::Global,
            index: 0,
        })
    );
}

#[test]
fn local_types_use_their_source_origin_and_binder_roots_are_not_references() {
    let fixture = Fixture::new();
    let template_origin = fixture.origin();
    let local_origin = origin_at("src/Local.scoop", 41);
    let nominal = SignatureTypeKey::Nominal(fixture.type_id);
    let locals = CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
            nominal.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(local_origin.clone()),
        )
        .unwrap(),
    ])
    .unwrap();
    let references = reference_set(
        Vec::new(),
        Vec::new(),
        vec![reference(nominal, &local_origin, &fixture)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let template = template(
        &fixture,
        locals,
        body(
            DefaultExpressionKindV1::UnitLiteral,
            binder(),
            template_origin.clone(),
        ),
        references,
        template_origin,
    );

    assert_eq!(
        validate(
            &template,
            &fixture,
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Ok(())
    );
}

#[test]
fn callable_reference_wrapper_does_not_leak_its_underlying_callable() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let nominal = SignatureTypeKey::Nominal(fixture.type_id);
    let underlying = crate::DefaultCallableRefV1::try_new(
        crate::DefaultCallableDeclarationV1::Function(fixture.function),
        OptionalSignatureType::Present(Box::new(nominal.clone())),
        vec![nominal.clone()],
    )
    .unwrap();
    let callable_reference = DefaultCallableReferenceV1::try_new(
        fixture.generated,
        definition_path(),
        DefaultCallableReferenceTargetV1::Named(underlying),
        nominal.clone(),
        Vec::new(),
        0,
    )
    .unwrap();
    let references = reference_set(
        vec![reference(
            ExportDefaultCallableTargetV1::CallableReference {
                invoke: fixture.generated,
            },
            &origin,
            &fixture,
        )],
        Vec::new(),
        vec![reference(nominal.clone(), &origin, &fixture)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let template = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        body(
            DefaultExpressionKindV1::CallableReference(callable_reference),
            nominal,
            origin.clone(),
        ),
        references,
        origin,
    );

    assert_eq!(
        validate(
            &template,
            &fixture,
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Ok(())
    );
}

#[test]
fn rejects_a_mismatched_owner_and_preserves_resource_failure() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let template = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        body(
            DefaultExpressionKindV1::UnitLiteral,
            binder(),
            origin.clone(),
        ),
        ExportDefaultReferenceSetV1::default(),
        origin,
    );
    let other = owner_interface(
        CallableTemplateOrigin::Constructor(fixture.constructor),
        &fixture,
    );
    assert_eq!(
        template.validate_reference_closure_semantics(
            &other,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        ),
        Err(
            ExportDefaultReferenceClosureValidationError::OwnerInterface {
                expected: CallableTemplateOrigin::Function(fixture.function),
                actual: CallableTemplateOrigin::Constructor(fixture.constructor),
            }
        )
    );

    let mut meter = BudgetMeter::new(DecodeLimits {
        validation_work_units: 1,
        ..DecodeLimits::default()
    });
    let error = validate(&template, &fixture, &mut meter).unwrap_err();
    assert!(matches!(
        error,
        ExportDefaultReferenceClosureValidationError::Resource(resource)
            if matches!(
                resource.kind(),
                WireErrorKind::LimitExceeded {
                    resource: ResourceKind::ValidationWorkUnits,
                    ..
                }
            )
    ));
    assert_eq!(meter.usage().validation_work_units, 1);
}

fn validate(
    template: &ExportDefaultTemplateV1,
    fixture: &Fixture,
    meter: &mut BudgetMeter,
) -> Result<(), ExportDefaultReferenceClosureValidationError> {
    template.validate_reference_closure_semantics(
        &owner_interface(CallableTemplateOrigin::Function(fixture.function), fixture),
        meter,
        &WirePath::root(),
    )
}

fn body(
    kind: DefaultExpressionKindV1,
    result: SignatureTypeKey,
    origin: ExportDefinitionSourceV1,
) -> ExportDefaultBodyV1 {
    ExportDefaultBodyV1::try_new(Vec::new(), expression(kind, result, origin)).unwrap()
}

fn expression(
    kind: DefaultExpressionKindV1,
    result: SignatureTypeKey,
    origin: ExportDefinitionSourceV1,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, result, origin).unwrap()
}

fn template(
    fixture: &Fixture,
    locals: CanonicalTemplateLocalTableV1,
    body: ExportDefaultBodyV1,
    references: ExportDefaultReferenceSetV1,
    origin: ExportDefinitionSourceV1,
) -> ExportDefaultTemplateV1 {
    let result = body.value().result_type().clone();
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        locals,
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
        references,
        origin,
    )
    .unwrap()
}

fn owner_interface(
    declaration: CallableTemplateOrigin,
    fixture: &Fixture,
) -> CallableInterfaceRecordV1 {
    let owner = match declaration {
        CallableTemplateOrigin::Constructor(_) => {
            PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(fixture.type_id))
        }
        _ => PublicDeclarationOwnerV1::TopLevel,
    };
    CallableInterfaceRecordV1::try_new(
        declaration,
        owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        binder(),
        effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap()
}

fn effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn reference_set(
    callables: Vec<ExportDefaultCallableReferenceV1>,
    constructors: Vec<ExportDefaultConstructorReferenceV1>,
    types: Vec<ExportDefaultTypeReferenceV1>,
    globals: Vec<ExportDefaultGlobalReferenceV1>,
    singleton_values: Vec<ExportDefaultSingletonReferenceV1>,
    fields: Vec<ExportDefaultFieldReferenceV1>,
) -> ExportDefaultReferenceSetV1 {
    ExportDefaultReferenceSetV1::try_new(
        callables,
        constructors,
        types,
        globals,
        singleton_values,
        fields,
    )
    .unwrap()
}

fn reference<T>(
    target: T,
    origin: &ExportDefinitionSourceV1,
    fixture: &Fixture,
) -> ExportDefaultReferenceV1<T> {
    ExportDefaultReferenceV1::new(
        target,
        origin.clone(),
        ExportDefaultAccessWitnessV1::new(
            CallableTemplateOrigin::Function(fixture.function),
            ExportDefaultCallDomainV1::DirectPublic,
        ),
    )
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}

fn origin_at(path: &str, point: u64) -> ExportDefinitionSourceV1 {
    let source =
        SourceIdentity::new(ConeIdentity::CORE, NormalizedSourcePath::new(path).unwrap()).unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(point, point + 1).unwrap(), &context)
            .unwrap(),
    )
}

mod source;
mod visitor;
