use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{DeclarationAccess, MethodModifier, PropertyCapability, PropertyRepresentation};
use scoop_ast::Span;

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identity(extension: bool) -> HirPropertyIdentity {
    let name = CanonicalIdentifier::new(if extension { "extended" } else { "plain" }).unwrap();
    if extension {
        HirPropertyIdentity::from_extension_declaration(SourceDeclarationKey::extension_property(
            site(),
            name,
            0,
            SignatureTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Any
                    .identity_record()
                    .id(),
            ),
        ))
        .unwrap()
    } else {
        HirPropertyIdentity::from_ordinary_declaration(SourceDeclarationKey::property(site(), name))
            .unwrap()
    }
}

fn access() -> DeclarationAccess {
    DeclarationAccess::public()
}

fn property(owner: PropertyOwner, getter: crate::PropertyGetterId) -> Property {
    Property {
        owner,
        name: "value".to_string(),
        access: access(),
        modifier: MethodModifier::Final,
        is_override: false,
        overrides: Vec::new(),
        override_access: Vec::new(),
        ty: crate::TypeId::from_raw(0_u32.into()),
        capability: PropertyCapability::ReadOnly { getter },
        representation: PropertyRepresentation::AccessorOnly,
        span: Span { start: 0, end: 0 },
    }
}

#[test]
fn checked_table_keeps_ordinary_and_extension_ids_distinct() {
    let getter = crate::PropertyGetterId::from_raw(0_u32.into());
    let mut properties = Arena::new();
    let ordinary = properties.alloc(property(PropertyOwner::TopLevel, getter));
    let expected_extension = crate::ExtensionPropertyId::from_raw(0_u32.into());
    let extension_property = properties.alloc(property(
        PropertyOwner::Extension(expected_extension),
        getter,
    ));
    let mut extensions = Arena::new();
    let extension = extensions.alloc(ExtensionProperty {
        property: extension_property,
        receiver_ty: crate::TypeId::from_raw(0_u32.into()),
        type_params: Vec::new(),
    });
    assert_eq!(extension, expected_extension);

    let identities = HirPropertyIdentities::checked(
        &properties,
        vec![identity(false), identity(true)],
        &extensions,
    )
    .unwrap();
    assert!(identities[ordinary].ordinary_id().is_some());
    assert!(identities[ordinary].extension_id().is_none());
    assert!(identities[extension_property].ordinary_id().is_none());
    assert!(identities[extension_property].extension_id().is_some());
}

#[test]
fn checked_table_rejects_kind_and_back_reference_mismatches() {
    let mut properties = Arena::new();
    let getter = crate::PropertyGetterId::from_raw(0_u32.into());
    properties.alloc(property(PropertyOwner::TopLevel, getter));
    assert!(matches!(
        HirPropertyIdentities::checked(&properties, vec![identity(true)], &Arena::new()),
        Err(HirPropertyIdentityTableError::Kind { property: 0 })
    ));

    let mut extension_properties = Arena::new();
    extension_properties.alloc(ExtensionProperty {
        property: PropertyId::from_raw(0_u32.into()),
        receiver_ty: crate::TypeId::from_raw(0_u32.into()),
        type_params: Vec::new(),
    });
    assert!(matches!(
        HirPropertyIdentities::checked(&properties, vec![identity(false)], &extension_properties),
        Err(HirPropertyIdentityTableError::PropertyBackReference {
            extension: 0,
            property: 0
        })
    ));
}
