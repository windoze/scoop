use super::*;
use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

pub(super) struct Targets {
    pub constructor: DefaultConstructorRefV1,
    pub field: DefaultFieldRefV1,
    pub global: scoop_identity::PersistentPropertyId,
    pub nominal: PersistentTypeId,
}

pub(super) fn surface(cone: ConeRecord) -> (Vec<u8>, Targets) {
    let (foundation, interface, nominal, _, constructor, global, _, _) =
        super::super::super::surface_fixture::nominal_surface(cone.identity(), true, true, true);
    let bytes = cross_cone_artifact_for_with_hir_foundation(cone, vec![], &foundation, interface);
    let front = declaration_front(&bytes);
    let field = front.hir_interface.nominal_interfaces().records()[0]
        .source_shape()
        .declared_fields()[0]
        .field();
    let owner_type = SignatureTypeKey::Nominal(nominal);
    (
        bytes,
        Targets {
            constructor: DefaultConstructorRefV1::Struct {
                declaration: constructor,
                owner_type: owner_type.clone(),
            },
            field: DefaultFieldRefV1::Struct {
                declaration: field,
                owner_type,
            },
            global,
            nominal,
        },
    )
}

pub(super) fn insert_reference(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
    target: Target<'_>,
) {
    let interface = &front.hir_interface;
    let (owner, function) = interface
        .callable_interfaces()
        .records()
        .iter()
        .find_map(|record| match record.declaration() {
            CallableTemplateOrigin::Function(id) => Some((record.declaration(), id)),
            _ => None,
        })
        .unwrap();
    let origin = ExportDefinitionSourceV1::new(
        front
            .foundations
            .hir
            .definition_origin(DefinitionOriginSubject::Function(function))
            .unwrap()
            .origin()
            .clone(),
    );
    let witness = ExportDefaultAccessWitnessV1::new(owner, ExportDefaultCallDomainV1::DirectPublic);
    let mut constructors = vec![];
    let mut globals = vec![];
    let mut singletons = vec![];
    let mut fields = vec![];
    match target {
        Target::Constructor(target) => constructors.push(ExportDefaultReferenceV1::new(
            target.clone(),
            origin.clone(),
            witness,
        )),
        Target::Global(target) => globals.push(ExportDefaultReferenceV1::new(
            target,
            origin.clone(),
            witness,
        )),
        Target::Singleton(target) => singletons.push(ExportDefaultReferenceV1::new(
            target,
            origin.clone(),
            witness,
        )),
        Target::Field(target) => fields.push(ExportDefaultReferenceV1::new(
            target.clone(),
            origin.clone(),
            witness,
        )),
    }
    let result = interface
        .callable_interfaces()
        .declaration(owner)
        .unwrap()
        .result()
        .clone();
    let value = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::StructConstruct {
            owner_type: result.clone(),
            fields: vec![],
        },
        result.clone(),
        origin.clone(),
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
        ExportDefaultBodyV1::try_new(vec![], value).unwrap(),
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap(),
        ExportDefaultReferenceSetV1::try_new(
            vec![],
            constructors,
            vec![],
            globals,
            singletons,
            fields,
        )
        .unwrap(),
        origin,
    )
    .unwrap();
    front.hir_interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
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
        front.graph.envelope.meter_mut(),
    )
    .validate_default_value_access()
}

pub(super) fn failure(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
    expected: ExportDefaultReferenceKindV1,
) -> Error {
    let Err(Error::Reference {
        kind,
        index: 0,
        source,
        ..
    }) = validate(front)
    else {
        panic!("an invalid value target must fail with its reference location")
    };
    assert_eq!(kind, expected);
    *source
}
