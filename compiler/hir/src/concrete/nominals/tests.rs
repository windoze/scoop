use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, FieldIdentityKey,
    PackagePath, PersistentFieldId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::*;

fn nominal_identity(
    name: &str,
    kind: SourceNominalKind,
    type_parameter_count: u32,
) -> HirNominalIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .expect("the test declaration site is valid");
    HirNominalIdentity::from_source_declaration(SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).expect("the test declaration name is canonical"),
        kind,
        type_parameter_count,
    ))
    .expect("the test nominal identity is valid")
}

fn field_identity(owner_name: &str, field_name: &str) -> PersistentFieldId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let owner = SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(owner_name).unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    PersistentFieldId::from_key(
        &FieldIdentityKey::source_declared(&owner, CanonicalIdentifier::new(field_name).unwrap())
            .unwrap(),
    )
    .unwrap()
}

fn test_variant_id(name: &str) -> PersistentEnumVariantId {
    let owner = nominal_identity("TestEnum", SourceNominalKind::Enum, 0);
    let key = scoop_identity::EnumVariantIdentityKey::source(
        owner.source().unwrap().declaration(),
        scoop_identity::CanonicalIdentifier::new(name).unwrap(),
    )
    .unwrap();
    PersistentEnumVariantId::from_key(&key).unwrap()
}

fn test_variant_field_id(name: &str) -> PersistentEnumVariantFieldId {
    let key = scoop_identity::EnumVariantFieldKey::new(
        test_variant_id(name),
        scoop_identity::EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    );
    PersistentEnumVariantFieldId::from_key(&key).unwrap()
}

#[test]
fn concrete_variant_and_struct_field_refs_are_checked() {
    let ty = TypeId::from_raw(0.into());
    let mut enums = Arena::new();
    let enumeration = enums.alloc(EnumDef {
        origin: nominal_identity("Option", SourceNominalKind::Enum, 1),
        canonical_type: ty,
        name: "Option".to_string(),
        owner: None,
        type_arguments: vec![ty],
        gc_free: true,
        variants: vec![
            Variant {
                identity: test_variant_id("Some"),
                name: "Some".to_string(),
                gc_free: true,
                fields: vec![VariantField {
                    identity: test_variant_field_id("Some"),
                    name: "value".to_string(),
                    ty,
                }],
            },
            Variant {
                identity: test_variant_id("None"),
                name: "None".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        span: Span::new(0, 0),
    });
    let data =
        EnumVariantRef::checked(&enums, enumeration, VariantId::from_raw(0)).expect("Data exists");
    let empty =
        EnumVariantRef::checked(&enums, enumeration, VariantId::from_raw(1)).expect("Empty exists");
    assert!(EnumVariantRef::checked(&enums, enumeration, VariantId::from_raw(2)).is_none());
    assert!(
        EnumVariantRef::checked(&enums, EnumId::from_raw(99.into()), VariantId::from_raw(0))
            .is_none()
    );
    let field = EnumVariantFieldRef::checked(&enums, data, 0).expect("Data.value exists");
    assert_eq!(field.variant(), data);
    assert!(EnumVariantFieldRef::checked(&enums, data, 1).is_none());
    assert!(EnumVariantFieldRef::checked(&enums, empty, 0).is_none());
    let option = OptionCore::checked(&enums, field, empty).expect("valid Option shape");
    assert_eq!(option.enumeration(), enumeration);
    assert_eq!(option.some_payload(), field);
    assert_eq!(option.some(), data);
    assert_eq!(option.none(), empty);
    assert!(OptionCore::checked(&enums, field, data).is_none());
    enums[enumeration].type_arguments.clear();
    assert!(OptionCore::checked(&enums, field, empty).is_none());
    enums[enumeration].type_arguments.push(ty);
    enums[enumeration].type_arguments[0] = TypeId::from_raw(1.into());
    assert!(OptionCore::checked(&enums, field, empty).is_none());
    enums[enumeration].type_arguments[0] = ty;
    enums[enumeration].variants.push(Variant {
        identity: test_variant_id("Unexpected"),
        name: "Unexpected".to_string(),
        gc_free: true,
        fields: Vec::new(),
    });
    assert!(OptionCore::checked(&enums, field, empty).is_none());
    enums[enumeration].variants.pop();
    let other = enums.alloc(EnumDef {
        origin: nominal_identity("Other", SourceNominalKind::Enum, 0),
        canonical_type: ty,
        name: "Other".to_string(),
        owner: None,
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![Variant {
            identity: test_variant_id("Empty"),
            name: "Empty".to_string(),
            gc_free: true,
            fields: Vec::new(),
        }],
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        span: Span::new(0, 0),
    });
    let other_empty = EnumVariantRef::checked(&enums, other, VariantId::from_raw(0)).unwrap();
    assert!(OptionCore::checked(&enums, field, other_empty).is_none());

    let mut structs = Arena::new();
    let declared = structs.alloc(StructDef {
        origin: nominal_identity("Record", SourceNominalKind::Struct, 0),
        canonical_type: ty,
        name: "Record".to_string(),
        owner: None,
        type_arguments: Vec::new(),
        gc_free: true,
        representation: StructRepresentation::Declared {
            attributes: StructAttributes::default(),
            fields: vec![DeclaredStructField {
                identity: field_identity("Record", "value"),
                name: "value".to_string(),
                ty,
            }],
        },
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        span: Span::new(0, 0),
    });
    assert!(StructFieldRef::checked(&structs, declared, 0).is_some());
    assert!(StructFieldRef::checked(&structs, declared, 1).is_none());
    assert!(StructFieldRef::checked(&structs, StructId::from_raw(99.into()), 0).is_none());
}
