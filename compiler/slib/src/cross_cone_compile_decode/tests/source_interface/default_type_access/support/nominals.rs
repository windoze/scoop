use super::*;

pub(in super::super) fn hidden(
    visibility: DeclaredVisibilityV1,
    generic: bool,
    child: bool,
) -> (CallableSourceSurface, SignatureTypeKey) {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let original = OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap();
    let template = &fixture.interface.default_templates().records()[0];
    let SignatureTypeKey::Nominal(parent) = *template.result() else {
        panic!("fixture type")
    };
    let site = SourceDeclarationSite::new(
        fixture.cone.identity(),
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let parent_record =
        CborIdentityRecord::<PersistentTypeId, _>::from_key(SourceDeclarationKey::nominal(
            site.clone(),
            CanonicalIdentifier::new("Value").unwrap(),
            SourceNominalKind::Struct,
            0,
        ))
        .unwrap();
    assert_eq!(parent_record.id(), parent);
    let array =
        CborIdentityRecord::<PersistentGenericTypeId, _>::from_key(SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new("ValueArray").unwrap(),
            SourceNominalKind::Class,
            1,
        ))
        .unwrap();
    let mut types = vec![
        CoreBuiltinNominal::Unit.identity_record(),
        CoreBuiltinNominal::Any.identity_record(),
        parent_record,
    ];
    let mut generics = vec![array.clone()];
    let mut owners = vec![DefinitionOwnerAtom::Type(parent)];
    let key = source_key(&fixture, "Hidden", owners.clone(), u32::from(generic));
    let (hidden, subject, hidden_type, atom) = if generic {
        let record = CborIdentityRecord::<PersistentGenericTypeId, _>::from_key(key).unwrap();
        let id = record.id();
        generics.push(record);
        (
            SourceNominalId::GenericTemplate(id),
            DefinitionOriginSubject::GenericType(id),
            SignatureTypeKey::NominalApplication {
                origin: id,
                arguments: NonEmptyVec::from_first(SignatureTypeKey::Nominal(parent), []),
            },
            DefinitionOwnerAtom::GenericType(id),
        )
    } else {
        let record = CborIdentityRecord::<PersistentTypeId, _>::from_key(key).unwrap();
        let id = record.id();
        types.push(record);
        (
            SourceNominalId::Concrete(id),
            DefinitionOriginSubject::Type(id),
            SignatureTypeKey::Nominal(id),
            DefinitionOwnerAtom::Type(id),
        )
    };
    let origin = original
        .definition_origin(DefinitionOriginSubject::Type(parent))
        .unwrap()
        .origin();
    let CallableTemplateOrigin::Function(function) = fixture.owner else {
        panic!("fixture callable")
    };
    let mut origins = [
        DefinitionOriginSubject::Type(parent),
        DefinitionOriginSubject::GenericType(array.id()),
        DefinitionOriginSubject::Function(function),
    ]
    .map(|subject| original.definition_origin(subject).unwrap().clone())
    .to_vec();
    origins.push(DefinitionOriginRecord::new(subject, origin.clone()));
    let mut support = Vec::new();
    let (children, result) = if child {
        owners.push(atom);
        let record = CborIdentityRecord::<PersistentTypeId, _>::from_key(source_key(
            &fixture, "Exposed", owners, 0,
        ))
        .unwrap();
        let id = record.id();
        types.push(record);
        origins.push(DefinitionOriginRecord::new(
            DefinitionOriginSubject::Type(id),
            origin.clone(),
        ));
        support.push(nominal_record(
            SourceNominalId::Concrete(id),
            DeclaredVisibilityV1::Public,
            vec![],
        ));
        (
            vec![SourceNominalId::Concrete(id)],
            SignatureTypeKey::Nominal(id),
        )
    } else {
        (vec![], hidden_type)
    };
    support.push(nominal_record(hidden, visibility, children));
    fixture.foundation.set_types(types).unwrap();
    fixture.foundation.set_generic_types(generics).unwrap();
    fixture.foundation.set_definition_origins(origins).unwrap();
    let interface = &fixture.interface;
    let public = interface
        .nominal_interfaces()
        .records()
        .iter()
        .map(|record| {
            if record.declaration() != SourceNominalId::Concrete(parent) {
                return record.clone();
            }
            let details = record.declaration_details();
            NominalInterfaceRecordV1::try_new(
                record.declaration(),
                record.kind(),
                record.type_parameters().clone(),
                record.exact_supertypes().clone(),
                record.constructors().clone(),
                record.members().clone(),
                record.nested_bindings().clone(),
                record.source_shape().clone(),
                NominalDeclarationDetailsV1::new(
                    details.modality(),
                    details.declared_visibility(),
                    details.constructors().clone(),
                    details.members().clone(),
                    CanonicalNestedNominalRefsV1::try_new(vec![hidden]).unwrap(),
                    details.dispatch_order().clone(),
                    details.dispatch_selections().clone(),
                ),
            )
            .unwrap()
        })
        .collect();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        CanonicalNominalInterfacesV1::with_support(public, support).unwrap(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        interface.default_templates().clone(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
    );
    (fixture, result)
}

fn source_key(
    fixture: &CallableSourceSurface,
    name: &str,
    owners: Vec<DefinitionOwnerAtom>,
    arity: u32,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            fixture.cone.identity(),
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(owners),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        arity,
    )
}
