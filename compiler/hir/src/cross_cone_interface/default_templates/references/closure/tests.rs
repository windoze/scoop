use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOrigin, LocalValueSelector,
    NormalizedSourcePath, OptionalSignatureType, SignatureTypeKey, SourceContextKey,
    SourceIdentity, SourceSpan, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::WirePath;

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultCallableReferenceTargetV1,
    DefaultCallableReferenceV1, DefaultConstructorRefV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultFieldRefV1, ExportDefaultBodyV1, ExportDefaultCallableReferenceV1,
    ExportDefaultConstructorReferenceV1, ExportDefaultFieldReferenceV1,
    ExportDefaultGlobalReferenceV1, ExportDefaultReferenceV1, ExportDefaultSingletonReferenceV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTypeReferenceV1, OptionalTemplateReceiverV1,
    PersistentLexicalRootV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
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
                receiver: crate::SourceCallReceiver::NoReceiver,
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
        )],
        vec![reference(constructor, &origin)],
        vec![reference(nominal, &origin)],
        vec![reference(fixture.property, &origin)],
        vec![reference(fixture.object, &origin)],
        vec![reference(field, &origin)],
    );
    let template = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        global_body,
        references,
        origin,
    );

    assert_eq!(validate(&template), Ok(()));
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
        vec![reference(fixture.property, &other_origin)],
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
            &template_with_wrong_origin
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
        vec![reference(fixture.property, &origin)],
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
        validate(&template_with_extra),
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
        vec![reference(nominal, &local_origin)],
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

    assert_eq!(validate(&template), Ok(()));
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
        )],
        Vec::new(),
        vec![reference(nominal.clone(), &origin)],
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

    assert_eq!(validate(&template), Ok(()));
}

fn validate(
    template: &ExportDefaultTemplateV1,
) -> Result<(), ExportDefaultReferenceClosureValidationError> {
    template.validate_reference_closure(&WirePath::root())
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

fn reference<T>(target: T, origin: &ExportDefinitionSourceV1) -> ExportDefaultReferenceV1<T> {
    ExportDefaultReferenceV1::new(target, origin.clone())
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

mod visitor;
