use super::*;
use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

pub(super) fn surface(cone: ConeRecord) -> Vec<u8> {
    let (foundation, interface, ..) = nominal_surface(cone.identity(), true, true, true);
    cross_cone_artifact_for_with_hir_foundation(cone, vec![], &foundation, interface)
}

pub(super) fn context(
    front: &HirProductionValidatedCrossConeHirFrontSections<'_>,
) -> (
    CallableTemplateOrigin,
    ExportDefinitionSourceV1,
    SignatureTypeKey,
) {
    let record = front
        .hir_interface
        .callable_interfaces()
        .records()
        .iter()
        .find(|r| matches!(r.declaration(), CallableTemplateOrigin::Function(_)))
        .unwrap();
    let CallableTemplateOrigin::Function(id) = record.declaration() else {
        panic!("function")
    };
    let origin = ExportDefinitionSourceV1::new(
        front
            .foundations
            .hir
            .definition_origin(DefinitionOriginSubject::Function(id))
            .unwrap()
            .origin()
            .clone(),
    );
    (record.declaration(), origin, record.result().clone())
}

pub(super) fn value(
    ty: &SignatureTypeKey,
    origin: &ExportDefinitionSourceV1,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::StructConstruct {
            owner_type: ty.clone(),
            fields: vec![],
        },
        ty.clone(),
        origin.clone(),
    )
    .unwrap()
}

pub(super) fn install_call(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
    declaration: CallableTemplateOrigin,
) {
    let (owner, origin, ty) = context(front);
    let source = front
        .hir_interface
        .callable_interfaces()
        .declaration(declaration)
        .unwrap();
    let id = match declaration {
        CallableTemplateOrigin::Function(id) => DefaultCallableDeclarationV1::Function(id),
        CallableTemplateOrigin::GenericFunction(id) => {
            DefaultCallableDeclarationV1::GenericFunction(id)
        }
        CallableTemplateOrigin::Accessor(id) => DefaultCallableDeclarationV1::PropertyAccessor(id),
        _ => panic!("named callable fixture"),
    };
    let receiver = match source.owner() {
        PublicDeclarationOwnerV1::Nominal(_) => {
            OptionalSignatureType::Present(Box::new(ty.clone()))
        }
        _ => OptionalSignatureType::Absent,
    };
    let arguments = source
        .parameters()
        .parameters()
        .iter()
        .map(|_| value(&ty, &origin))
        .collect();
    let type_arguments = if matches!(declaration, CallableTemplateOrigin::GenericFunction(_)) {
        vec![ty.clone()]
    } else {
        vec![]
    };
    let callee = DefaultCallableRefV1::try_new(id, receiver, type_arguments).unwrap();
    let expression = if matches!(
        source.owner(),
        PublicDeclarationOwnerV1::Nominal(_) | PublicDeclarationOwnerV1::Extension
    ) {
        DefaultExpressionKindV1::MethodCall {
            receiver: Box::new(value(&ty, &origin)),
            callee: DefaultMethodCalleeV1::Callable(callee.clone()),
            arguments,
        }
    } else {
        DefaultExpressionKindV1::Call {
            receiver: scoop_hir::SourceCallReceiver::NoReceiver,
            callee: callee.clone(),
            arguments,
        }
    };
    install(
        front,
        owner,
        origin,
        ty,
        expression,
        ExportDefaultCallableTargetV1::Callable(callee),
    );
}

pub(super) fn install(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
    owner: CallableTemplateOrigin,
    origin: ExportDefinitionSourceV1,
    result: SignatureTypeKey,
    kind: DefaultExpressionKindV1,
    target: ExportDefaultCallableTargetV1,
) {
    let expression = DefaultExpressionV1::try_new(kind, result.clone(), origin.clone()).unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Expr(Box::new(expression)),
                origin.clone(),
            )
            .unwrap(),
        ],
        value(&result, &origin),
    )
    .unwrap();
    let references = ExportDefaultReferenceSetV1::try_new(
        vec![ExportDefaultReferenceV1::new(
            target,
            origin.clone(),
            ExportDefaultAccessWitnessV1::new(owner, ExportDefaultCallDomainV1::DirectPublic),
        )],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let template = ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(owner, 0),
        PersistentLexicalRootV1::try_from(owner).unwrap(),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap(),
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap(),
        references,
        origin,
    )
    .unwrap();
    replace_template(front, template);
}

fn replace_template(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
    template: ExportDefaultTemplateV1,
) {
    let i = &front.hir_interface;
    front.hir_interface = CrossConeHirInterfaceSectionV1::new(
        i.public_bindings().clone(),
        i.nominal_interfaces().clone(),
        i.callable_interfaces().clone(),
        i.property_interfaces().clone(),
        i.type_aliases().clone(),
        i.source_interfaces().clone(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap(),
        i.constants().clone(),
        i.definition_sources().clone(),
        i.external_references().clone(),
    );
}

pub(super) fn validate(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
) -> Result<(), Error> {
    CanonicalCrossConeHirSurfaceAuthority::new(
        front.graph.identity(),
        &front.identities,
        &front.foundations.hir,
        &front.hir_interface,
        vec![],
    )
    .validate_default_callable_access(&front.hir_core_production)
}

pub(super) fn failure(front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>) -> Error {
    let expected = front.hir_interface.default_templates().records()[0].key();
    let Err(Error::Template { key, source }) = validate(front) else {
        panic!("callable access must fail")
    };
    assert_eq!(key, expected);
    let Error::Occurrence {
        index: 0,
        site: ExportDefaultReferenceOccurrenceSiteV1::Expression,
        expression_index: Some(0),
        source,
    } = *source
    else {
        panic!("first expression callable reference")
    };
    *source
}
