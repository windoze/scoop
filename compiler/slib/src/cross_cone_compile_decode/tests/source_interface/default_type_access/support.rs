use super::*;
use scoop_hir::*;
use scoop_identity::{
    CoreBuiltinNominal, DefinitionOwnerAtom, LocalValueSelector, NonEmptyVec,
    PersistentGenericTypeId, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

mod nominals;
pub(super) use nominals::hidden;

pub(super) fn front(bytes: &[u8]) -> HirProductionValidatedCrossConeHirFrontSections<'_> {
    let mut decoded = open_graph(bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities(std::iter::empty())
        .unwrap();
    decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap()
}

pub(super) fn validate(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
) -> Result<(), Error> {
    crate::cross_cone_hir_authority::CanonicalCrossConeHirSurfaceAuthority::new(
        front.graph.identity(),
        &front.identities,
        &front.foundations.hir,
        &front.hir_interface,
        vec![],
        front.graph.envelope.meter_mut(),
    )
    .validate_default_type_access(&front.hir_core_production)
}

pub(super) fn add_unused_local(fixture: &mut CallableSourceSurface, ty: SignatureTypeKey) {
    let interface = &fixture.interface;
    let template = &interface.default_templates().records()[0];
    let origin = template.definition_origin().clone();
    let mut locals = template.locals().records().to_vec();
    locals.push(
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::LocalDeclaration {
                path: StructuralDefinitionPath::from_first(
                    StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
                    [StructuralPathSegment::new(
                        StructuralDefinitionSiteRole::LocalDeclaration,
                        1,
                    )],
                ),
            },
            ty.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(origin.clone()),
        )
        .unwrap(),
    );
    let refs = template.references();
    let mut types = refs.types().to_vec();
    types.push(ExportDefaultReferenceV1::new(
        ty,
        origin,
        ExportDefaultAccessWitnessV1::new(fixture.owner, ExportDefaultCallDomainV1::DirectPublic),
    ));
    types.sort_unstable();
    let references = ExportDefaultReferenceSetV1::try_new(
        refs.callables().to_vec(),
        refs.constructors().to_vec(),
        types,
        refs.globals().to_vec(),
        refs.singleton_values().to_vec(),
        refs.fields().to_vec(),
    )
    .unwrap();
    let template = ExportDefaultTemplateV1::try_new(
        template.key(),
        template.definition_root(),
        template.definition_path().clone(),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        template.body().clone(),
        template.result().clone(),
        template.allows_suspend(),
        template.type_parameters().clone(),
        template.receiver().clone(),
        template.value_parameters().clone(),
        references,
        template.definition_origin().clone(),
    )
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
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

fn nominal_record(
    declaration: SourceNominalId,
    visibility: DeclaredVisibilityV1,
    children: Vec<SourceNominalId>,
) -> NominalInterfaceRecordV1 {
    let binders = if matches!(declaration, SourceNominalId::GenericTemplate(_)) {
        vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )]
    } else {
        vec![]
    };
    NominalInterfaceRecordV1::try_new(
        declaration,
        PublicNominalKindV1::Class,
        CanonicalBinderListV1::try_new(binders).unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::empty(),
        CanonicalPublicMemberRefsV1::default(),
        CanonicalPersistentIdsV1::empty(),
        NominalSourceShapeV1::Class(Default::default()),
        NominalDeclarationDetailsV1::new(
            NominalInheritanceModalityV1::Final,
            visibility,
            CanonicalPersistentIdsV1::empty(),
            CanonicalNestedMemberRefsV1::try_new(vec![]).unwrap(),
            CanonicalNestedNominalRefsV1::try_new(children).unwrap(),
        ),
    )
    .unwrap()
}
