use la_arena::Arena;
use scoop_ast::Span;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, FieldIdentityKey,
    PackagePath, PersistentFieldId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::*;
use crate::{
    ClassApplicationId, ClassModifier, ClassRepresentation, DeclarationAccess, EnumDecl,
    HirNominalIdentity, HirPropertyIdentity, InterfaceDecl, MethodModifier, NominalAccess,
    NominalLinkStem, PropertyCapability, PropertyGetterId, StoredProperty, StructApplicationId,
    StructAttributes, StructRepresentation, TypeId,
};

fn source_key(name: &str, kind: SourceNominalKind) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    )
}

fn structure(name: &str, fields: &[&str]) -> StructDecl {
    StructDecl {
        link_stem: NominalLinkStem::from_session_local_encoding(name.to_string()),
        name: name.to_string(),
        owner: None,
        access: NominalAccess::public(),
        self_application: StructApplicationId::from_raw(0_u32.into()),
        type_params: Vec::new(),
        gc_free_pointee_requirements: Vec::new(),
        attributes: StructAttributes::default(),
        representation: StructRepresentation::Declared(
            fields
                .iter()
                .map(|name| crate::Field {
                    name: (*name).to_string(),
                    ty: TypeId::from_raw(0_u32.into()),
                })
                .collect(),
        ),
        constructors: Vec::new(),
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        properties: Vec::new(),
        derived_equality: None,
        span: Span::new(0, 0),
    }
}

fn class(name: &str, fields: Vec<ClassFieldId>, properties: Vec<PropertyId>) -> ClassDecl {
    ClassDecl {
        modifier: ClassModifier::Final,
        link_stem: NominalLinkStem::from_session_local_encoding(name.to_string()),
        name: name.to_string(),
        owner: None,
        access: NominalAccess::public(),
        self_application: ClassApplicationId::from_raw(0_u32.into()),
        type_params: Vec::new(),
        gc_free_pointee_requirements: Vec::new(),
        representation: ClassRepresentation::Declared,
        fields,
        properties,
        constructors: Vec::new(),
        base_class: None,
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        span: Span::new(0, 0),
    }
}

fn stored_property(owner: ClassId, field: ClassFieldId, name: &str) -> Property {
    Property {
        owner: PropertyOwner::Class(owner),
        name: name.to_string(),
        access: DeclarationAccess::public(),
        modifier: MethodModifier::Final,
        is_override: false,
        overrides: Vec::new(),
        override_access: Vec::new(),
        ty: TypeId::from_raw(0_u32.into()),
        capability: PropertyCapability::ReadOnly {
            getter: PropertyGetterId::from_raw(0_u32.into()),
        },
        representation: PropertyRepresentation::Stored(StoredProperty {
            backing: PropertyBacking::ClassField {
                field,
                initializer: crate::ClassPropertyInitializer::Expression,
            },
        }),
        span: Span::new(0, 0),
    }
}

fn nominal_identities(
    structs: &Arena<StructDecl>,
    struct_keys: Vec<SourceDeclarationKey>,
    classes: &Arena<ClassDecl>,
    class_keys: Vec<SourceDeclarationKey>,
) -> HirNominalIdentities {
    HirNominalIdentities::checked(
        structs,
        struct_keys
            .into_iter()
            .map(|key| HirNominalIdentity::from_source_declaration(key).unwrap())
            .collect(),
        &Arena::<EnumDecl>::new(),
        Vec::new(),
        classes,
        class_keys
            .into_iter()
            .map(|key| HirNominalIdentity::from_source_declaration(key).unwrap())
            .collect(),
        &Arena::<InterfaceDecl>::new(),
        Vec::new(),
        &Arena::new(),
        Vec::new(),
    )
    .unwrap()
}

fn property_identities(properties: &Arena<Property>) -> HirPropertyIdentities {
    let identities = properties
        .iter()
        .map(|(id, _)| {
            let key = SourceDeclarationKey::property(
                SourceDeclarationSite::new(
                    ConeIdentity::CORE,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new(&format!("field{}", id.into_raw().into_u32())).unwrap(),
            );
            HirPropertyIdentity::from_ordinary_declaration(key).unwrap()
        })
        .collect();
    HirPropertyIdentities::checked(properties, identities, &Arena::new()).unwrap()
}

#[test]
fn relation_is_total_for_struct_and_source_class_fields() {
    let struct_key = source_key("Point", SourceNominalKind::Struct);
    let class_key = source_key("Box", SourceNominalKind::Class);
    let mut structs = Arena::new();
    let structure = structs.alloc(structure("Point", &["x", "y"]));
    let mut classes = Arena::new();
    let class_id = ClassId::from_raw(0_u32.into());
    let field_id = ClassFieldId::from_raw(0_u32.into());
    let property_id = PropertyId::from_raw(0_u32.into());
    classes.alloc(class("Box", vec![field_id], vec![property_id]));
    let mut class_fields = Arena::new();
    class_fields.alloc(ClassField {
        owner: class_id,
        property: property_id,
        ty: TypeId::from_raw(0_u32.into()),
        source: crate::ClassFieldSource::Body,
        span: Span::new(0, 0),
    });
    let mut properties = Arena::new();
    properties.alloc(stored_property(class_id, field_id, "value"));
    let nominals = nominal_identities(
        &structs,
        vec![struct_key.clone()],
        &classes,
        vec![class_key.clone()],
    );
    let property_identities = property_identities(&properties);

    let identities = HirFieldIdentities::from_declarations(
        &structs,
        &classes,
        &Arena::new(),
        &class_fields,
        &properties,
        &Arena::new(),
        &nominals,
        &property_identities,
    )
    .unwrap();
    let x = StructFieldRef::checked(&structs, structure, 0).unwrap();
    let expected_x =
        FieldIdentityKey::source_declared(&struct_key, CanonicalIdentifier::new("x").unwrap())
            .unwrap();
    let property = property_identities[property_id].ordinary_id().unwrap();
    let expected_class = FieldIdentityKey::source_property_backing(&class_key, property).unwrap();

    assert_eq!(
        identities[x].id(),
        PersistentFieldId::from_key(&expected_x).unwrap()
    );
    assert_eq!(
        identities[field_id].id(),
        PersistentFieldId::from_key(&expected_class).unwrap()
    );

    let stored_id = identities[field_id].id();
    let mut delegate_storages = Arena::new();
    let storage = delegate_storages.alloc(DelegateStorage {
        property: property_id,
        ty: TypeId::from_raw(0_u32.into()),
        location: DelegateStorageLocation::ClassField(field_id),
    });
    properties[property_id].representation = PropertyRepresentation::Delegated { storage };
    let delegated = HirFieldIdentities::from_declarations(
        &structs,
        &classes,
        &Arena::new(),
        &class_fields,
        &properties,
        &delegate_storages,
        &nominals,
        &property_identities,
    )
    .unwrap();
    let expected_delegate =
        FieldIdentityKey::source_property_delegate(&class_key, property).unwrap();
    assert_eq!(
        delegated[field_id].id(),
        PersistentFieldId::from_key(&expected_delegate).unwrap()
    );
    assert_ne!(stored_id, delegated[field_id].id());
}

#[test]
fn relation_rejects_a_class_field_without_physical_property_storage() {
    let mut classes = Arena::new();
    let class_id = ClassId::from_raw(0_u32.into());
    let field_id = ClassFieldId::from_raw(0_u32.into());
    let property_id = PropertyId::from_raw(0_u32.into());
    classes.alloc(class("Box", vec![field_id], vec![property_id]));
    let mut class_fields = Arena::new();
    class_fields.alloc(ClassField {
        owner: class_id,
        property: property_id,
        ty: TypeId::from_raw(0_u32.into()),
        source: crate::ClassFieldSource::Body,
        span: Span::new(0, 0),
    });
    let mut properties = Arena::new();
    let mut property = stored_property(class_id, field_id, "value");
    property.representation = PropertyRepresentation::AccessorOnly;
    properties.alloc(property);
    let structs = Arena::new();
    let nominals = nominal_identities(
        &structs,
        Vec::new(),
        &classes,
        vec![source_key("Box", SourceNominalKind::Class)],
    );
    let property_identities = property_identities(&properties);

    assert!(matches!(
        HirFieldIdentities::from_declarations(
            &structs,
            &classes,
            &Arena::new(),
            &class_fields,
            &properties,
            &Arena::new(),
            &nominals,
            &property_identities,
        ),
        Err(HirFieldIdentityError::PropertyStorageMismatch {
            field: 0,
            property: 0,
        })
    ));
}
