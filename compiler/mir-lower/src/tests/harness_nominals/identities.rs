use la_arena::Arena;
use scoop_hir as hir;

pub(in crate::tests) fn test_source_nominal_identity(
    name: &str,
    kind: scoop_identity::SourceNominalKind,
    type_parameter_count: usize,
) -> hir::HirNominalIdentity {
    let source = scoop_identity::SourceIdentity::single_file();
    let site = scoop_identity::SourceDeclarationSite::new(
        source.cone(),
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::SourceScoped(source),
    )
    .expect("the single-file test declaration site is valid");
    let name = scoop_identity::CanonicalIdentifier::new(name)
        .expect("synthetic test declaration names follow the source identifier grammar");
    hir::HirNominalIdentity::from_source_declaration(scoop_identity::SourceDeclarationKey::nominal(
        site,
        name,
        kind,
        u32::try_from(type_parameter_count)
            .expect("test declaration type parameter count fits u32"),
    ))
    .expect("the test source nominal identity is valid")
}

impl super::super::Harness {
    pub(in crate::tests) fn struct_id(&self, declaration: hir::SourceNominalId) -> hir::StructId {
        self.nominal_identities()
            .struct_id(declaration)
            .expect("a test application retains its declaration")
    }

    pub(in crate::tests) fn enum_id(&self, declaration: hir::SourceNominalId) -> hir::EnumId {
        self.nominal_identities()
            .enum_id(declaration)
            .expect("a test application retains its declaration")
    }

    pub(in crate::tests) fn class_id(&self, declaration: hir::SourceNominalId) -> hir::ClassId {
        self.nominal_identities()
            .class_id(declaration)
            .expect("a test application retains its declaration")
    }

    pub(in crate::tests) fn interface_id(
        &self,
        declaration: hir::SourceNominalId,
    ) -> hir::InterfaceId {
        self.nominal_identities()
            .interface_id(declaration)
            .expect("a test application retains its declaration")
    }

    pub(in crate::tests) fn nominal_identities(&self) -> hir::HirNominalIdentities {
        test_nominal_identities_without_objects(
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
        )
    }
}

/// The MIR unit harness assembles nominal arenas directly rather than parsing
/// source declarations. Give each fixture entity a valid, arena-aligned typed
/// identity matching its complete source declaration contract.
pub(in crate::tests) fn test_nominal_identities_without_objects(
    structs: &Arena<hir::StructDecl>,
    enums: &Arena<hir::EnumDecl>,
    classes: &Arena<hir::ClassDecl>,
    interfaces: &Arena<hir::InterfaceDecl>,
) -> hir::HirNominalIdentities {
    test_nominal_identities(structs, enums, classes, interfaces, &Arena::new())
}

pub(in crate::tests) fn test_nominal_identities(
    structs: &Arena<hir::StructDecl>,
    enums: &Arena<hir::EnumDecl>,
    classes: &Arena<hir::ClassDecl>,
    interfaces: &Arena<hir::InterfaceDecl>,
    objects: &Arena<hir::ObjectDecl>,
) -> hir::HirNominalIdentities {
    let struct_identities = structs
        .iter()
        .map(|(_, declaration)| {
            test_source_nominal_identity(
                &declaration.name,
                scoop_identity::SourceNominalKind::Struct,
                declaration.type_params.len(),
            )
        })
        .collect();
    let enum_identities = enums
        .iter()
        .map(|(_, declaration)| {
            test_source_nominal_identity(
                &declaration.name,
                scoop_identity::SourceNominalKind::Enum,
                declaration.type_params.len(),
            )
        })
        .collect();
    let class_identities = classes
        .iter()
        .map(|(_, declaration)| {
            test_source_nominal_identity(
                &declaration.name,
                scoop_identity::SourceNominalKind::Class,
                declaration.type_params.len(),
            )
        })
        .collect();
    let interface_identities = interfaces
        .iter()
        .map(|(_, declaration)| {
            test_source_nominal_identity(
                &declaration.name,
                scoop_identity::SourceNominalKind::Interface,
                declaration.type_params.len(),
            )
        })
        .collect();
    let object_identities = objects
        .iter()
        .map(|(_, declaration)| {
            test_source_nominal_identity(
                &declaration.name,
                scoop_identity::SourceNominalKind::Object,
                0,
            )
        })
        .collect();
    hir::HirNominalIdentities::checked(
        structs,
        struct_identities,
        enums,
        enum_identities,
        classes,
        class_identities,
        interfaces,
        interface_identities,
        objects,
        object_identities,
    )
    .expect("the MIR test fixture provides one identity per nominal declaration")
}

pub(in crate::tests) fn test_property_identities(
    properties: &Arena<hir::Property>,
    extensions: &Arena<hir::ExtensionProperty>,
    nominals: &hir::HirNominalIdentities,
) -> hir::HirPropertyIdentities {
    let source = scoop_identity::SourceIdentity::single_file();
    let identities = properties
        .iter()
        .map(|(_, property)| {
            let owner = match property.owner {
                hir::PropertyOwner::Class(id) => Some(&nominals[id]),
                hir::PropertyOwner::Struct(id) => Some(&nominals[id]),
                hir::PropertyOwner::Enum(id) => Some(&nominals[id]),
                hir::PropertyOwner::Interface(id) => Some(&nominals[id]),
                hir::PropertyOwner::Object(id) => Some(&nominals[id]),
                hir::PropertyOwner::TopLevel => None,
                hir::PropertyOwner::Extension(_) => {
                    panic!("the MIR harness has no extension properties")
                }
            };
            let owners = owner
                .map(|owner| owner.source().unwrap().definition_owner())
                .into_iter()
                .collect();
            let site = scoop_identity::SourceDeclarationSite::new(
                source.cone(),
                scoop_identity::PackagePath::root(),
                scoop_identity::DefinitionOwnerChain::from_outer_to_inner(owners),
                scoop_identity::DeclarationScope::SourceScoped(source.clone()),
            )
            .expect("the single-file test property site is valid");
            let name = scoop_identity::CanonicalIdentifier::new(&property.name)
                .expect("synthetic property names are canonical identifiers");
            hir::HirPropertyIdentity::from_ordinary_declaration(
                scoop_identity::SourceDeclarationKey::property(site, name),
            )
            .expect("the synthetic ordinary property identity is valid")
        })
        .collect();
    hir::HirPropertyIdentities::checked(properties, identities, extensions)
        .expect("the MIR test fixture provides one identity per property declaration")
}

pub(in crate::tests) fn test_property_accessor_identities(
    properties: &Arena<hir::Property>,
    property_identities: &hir::HirPropertyIdentities,
    getters: &Arena<hir::PropertyGetter>,
    setters: &Arena<hir::PropertySetter>,
) -> hir::HirPropertyAccessorIdentities {
    let mut getter_properties = vec![None; getters.len()];
    let mut setter_properties = vec![None; setters.len()];
    for (property_id, property) in properties.iter() {
        let getter = property.capability.getter().into_raw().into_u32() as usize;
        assert!(getter_properties[getter].replace(property_id).is_none());
        if let Some(setter) = property.capability.setter() {
            let setter = setter.into_raw().into_u32() as usize;
            assert!(setter_properties[setter].replace(property_id).is_none());
        }
    }
    let getter_identities = getter_properties
        .into_iter()
        .map(|property| {
            let property = property.expect("every test getter has a logical property");
            hir::HirPropertyAccessorIdentity::getter(
                property,
                property_identities[property].property_owner(),
            )
            .expect("the synthetic getter identity is valid")
        })
        .collect();
    let setter_identities = setter_properties
        .into_iter()
        .map(|property| {
            let property = property.expect("every test setter has a logical property");
            hir::HirPropertyAccessorIdentity::setter(
                property,
                property_identities[property].property_owner(),
            )
            .expect("the synthetic setter identity is valid")
        })
        .collect();
    hir::HirPropertyAccessorIdentities::checked(
        properties,
        property_identities,
        getters,
        getter_identities,
        setters,
        setter_identities,
    )
    .expect("the MIR test fixture provides one identity per property accessor")
}

pub(in crate::tests) fn test_constructor_identities(
    type_inputs: hir::HirTypeIdentityInputs<'_>,
    struct_constructors: &Arena<hir::StructConstructor>,
    class_constructors: &Arena<hir::ClassConstructor>,
    class_constructor_applications: &Arena<hir::ClassConstructorApplication>,
) -> hir::HirConstructorIdentities {
    let mapper = hir::HirSignatureTypeMapper::new(type_inputs);
    let source_record = |owner: &hir::HirSourceNominalIdentity,
                         type_params: &[hir::TypeParamDecl],
                         parameters: &[hir::ConstructorParameter]|
     -> hir::HirSourceConstructorIdentity {
        let binders = type_params
            .iter()
            .enumerate()
            .map(|(index, parameter)| hir::HirSignatureBinder {
                parameter: parameter.id,
                depth: 0,
                index: u32::try_from(index).expect("test binder index fits u32"),
            })
            .collect::<Vec<_>>();
        let parameters = parameters
            .iter()
            .map(|parameter| mapper.map(parameter.ty, &binders).unwrap())
            .collect();
        let mut owners = owner.declaration().owners().owners().to_vec();
        owners.push(owner.definition_owner());
        let site = scoop_identity::SourceDeclarationSite::new(
            owner.declaration().origin(),
            owner.declaration().package().clone(),
            scoop_identity::DefinitionOwnerChain::from_outer_to_inner(owners),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap();
        scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::SourceDeclarationKey::constructor(site, parameters),
        )
        .unwrap()
    };
    let structs = struct_constructors
        .iter()
        .map(|(_, constructor)| {
            let owner = type_inputs.nominal_identities[constructor.owner]
                .source()
                .unwrap();
            source_record(
                owner,
                &type_inputs.structs[constructor.owner].type_params,
                &constructor.parameters,
            )
        })
        .collect();
    let classes = class_constructors
        .iter()
        .map(|(_, constructor)| {
            assert_eq!(
                constructor.identity_kind,
                hir::ClassConstructorIdentityKind::Source
            );
            let owner = type_inputs.nominal_identities[constructor.owner]
                .source()
                .unwrap();
            hir::HirClassConstructorIdentity::Source(source_record(
                owner,
                &type_inputs.classes[constructor.owner].type_params,
                &constructor.parameters,
            ))
        })
        .collect();
    hir::HirConstructorIdentities::checked(
        hir::HirConstructorIdentityInputs {
            type_inputs,
            struct_constructors,
            class_constructors,
            class_constructor_applications,
        },
        structs,
        classes,
    )
    .expect("the MIR test fixture constructors have persistent identities")
}
