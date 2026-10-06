use la_arena::Arena;
use scoop_ast::Span;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    EnumVariantFieldSelector, PackagePath, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

use super::*;
use crate::{
    ClassDecl, EnumApplicationId, EnumDecl, Field, HirNominalIdentities, HirNominalIdentity,
    InterfaceDecl, NominalAccess, ObjectDecl, StructDecl, Variant,
};

fn enum_key(name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Enum,
        0,
    )
}

fn enum_declaration() -> EnumDecl {
    EnumDecl {
        name: "Choice".to_string(),
        owner: None,
        access: NominalAccess::public(),
        methods: Vec::new(),
        properties: Vec::new(),
        derived_equality: None,
        span: Span::new(0, 0),

        definition: crate::EnumDefinition {
            element_encoding: None,
            self_application: EnumApplicationId::from_raw(0_u32.into()),
            type_params: Vec::new(),
            variants: vec![Variant {
                name: "Value".to_string(),
                style: crate::VariantStyle::Named,
                fields: vec![Field {
                    name: "payload".to_string(),
                    ty: crate::TypeId::from_raw(0_u32.into()),
                }],
            }],
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),

            gc_free_pointee_requirements: Vec::new(),
            no_gc: false,
        },
    }
}

fn nominal_identities(enums: &Arena<EnumDecl>) -> HirNominalIdentities {
    let structs = Arena::<StructDecl>::new();
    let classes = Arena::<ClassDecl>::new();
    let interfaces = Arena::<InterfaceDecl>::new();
    let objects = Arena::<ObjectDecl>::new();
    HirNominalIdentities::checked(
        &structs,
        Vec::new(),
        enums,
        vec![HirNominalIdentity::from_source_declaration(enum_key("Choice")).unwrap()],
        &classes,
        Vec::new(),
        &interfaces,
        Vec::new(),
        &objects,
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn relation_is_total_and_indexed_by_checked_enum_member_refs() {
    let mut enums = Arena::new();
    let enumeration = enums.alloc(enum_declaration());
    let nominals = nominal_identities(&enums);
    let identities = HirEnumMemberIdentities::from_declarations(&enums, &nominals).unwrap();
    let variant = EnumVariantRef::checked(&enums, enumeration, 0).unwrap();
    let field = EnumVariantFieldRef::checked(&enums, variant, 0).unwrap();

    assert_eq!(identities[field].key().variant(), identities[variant].id());
    assert!(matches!(
        identities[field].key().selector(),
        EnumVariantFieldSelector::Named(name) if name.as_str() == "payload"
    ));
}

#[test]
fn relation_rejects_a_field_on_a_unit_variant() {
    let mut enums = Arena::new();
    let enumeration = enums.alloc(enum_declaration());
    enums[enumeration].variants[0].style = crate::VariantStyle::Unit;
    let nominals = nominal_identities(&enums);

    assert!(matches!(
        HirEnumMemberIdentities::from_declarations(&enums, &nominals),
        Err(HirEnumMemberIdentityError::UnitVariantField {
            enumeration: 0,
            variant: 0,
            field: 0,
        })
    ));
}

#[test]
fn positional_selector_uses_the_declaration_index() {
    let mut enums = Arena::new();
    let enumeration = enums.alloc(enum_declaration());
    enums[enumeration].variants[0].style = crate::VariantStyle::Positional;
    let nominals = nominal_identities(&enums);
    let identities = HirEnumMemberIdentities::from_declarations(&enums, &nominals).unwrap();
    let variant = EnumVariantRef::checked(&enums, enumeration, 0).unwrap();
    let field = EnumVariantFieldRef::checked(&enums, variant, 0).unwrap();

    assert_eq!(
        identities[field].key().selector(),
        &EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        }
    );
}
