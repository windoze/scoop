use la_arena::Arena;
use scoop_ast::Span;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{
    DeclarationAccess, HirPropertyIdentity, MethodModifier, PropertyCapability,
    PropertyOwner as HirPropertyOwner, PropertyRepresentation,
};

fn property_identity(name: &str) -> HirPropertyIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    HirPropertyIdentity::from_ordinary_declaration(SourceDeclarationKey::property(
        site,
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn property(capability: PropertyCapability) -> Property {
    Property {
        owner: HirPropertyOwner::TopLevel,
        name: "value".to_string(),
        access: DeclarationAccess::public(),
        modifier: MethodModifier::Final,
        is_override: false,
        overrides: Vec::new(),
        override_access: Vec::new(),
        ty: crate::TypeId::from_raw(0_u32.into()),
        capability,
        representation: PropertyRepresentation::AccessorOnly,
        span: Span { start: 0, end: 0 },
    }
}

fn getter() -> PropertyGetter {
    PropertyGetter {
        access: DeclarationAccess::public(),
        implementation: crate::PropertyAccessorImplementation::Constant,
        attributes: crate::FunctionAttributes::default(),
        span: Span { start: 0, end: 0 },
    }
}

fn setter() -> PropertySetter {
    PropertySetter {
        access: DeclarationAccess::public(),
        implementation: crate::PropertyAccessorImplementation::Storage,
        attributes: crate::FunctionAttributes::default(),
        parameter_name: "value".to_string(),
        span: Span { start: 0, end: 0 },
    }
}

struct Fixture {
    properties: Arena<Property>,
    property_identities: HirPropertyIdentities,
    getters: Arena<PropertyGetter>,
    getter_identities: Vec<HirPropertyAccessorIdentity>,
    setters: Arena<PropertySetter>,
    setter_identities: Vec<HirPropertyAccessorIdentity>,
    first_property: PropertyId,
    second_property: PropertyId,
    first_getter: PropertyGetterId,
    second_getter: PropertyGetterId,
    first_setter: PropertySetterId,
}

fn fixture() -> Fixture {
    let mut getters = Arena::new();
    let first_getter = getters.alloc(getter());
    let second_getter = getters.alloc(getter());
    let mut setters = Arena::new();
    let first_setter = setters.alloc(setter());
    let mut properties = Arena::new();
    let first_property = properties.alloc(property(PropertyCapability::ReadWrite {
        getter: first_getter,
        setter: first_setter,
    }));
    let second_property = properties.alloc(property(PropertyCapability::ReadOnly {
        getter: second_getter,
    }));
    let property_identities = HirPropertyIdentities::checked(
        &properties,
        vec![property_identity("first"), property_identity("second")],
        &Arena::new(),
    )
    .unwrap();
    let first_owner = property_identities[first_property].property_owner();
    let second_owner = property_identities[second_property].property_owner();
    let getter_identities = vec![
        HirPropertyAccessorIdentity::getter(first_property, first_owner).unwrap(),
        HirPropertyAccessorIdentity::getter(second_property, second_owner).unwrap(),
    ];
    let setter_identities =
        vec![HirPropertyAccessorIdentity::setter(first_property, first_owner).unwrap()];
    Fixture {
        properties,
        property_identities,
        getters,
        getter_identities,
        setters,
        setter_identities,
        first_property,
        second_property,
        first_getter,
        second_getter,
        first_setter,
    }
}

fn checked(
    fixture: Fixture,
) -> Result<HirPropertyAccessorIdentities, HirPropertyAccessorIdentityTableError> {
    HirPropertyAccessorIdentities::checked(
        &fixture.properties,
        &fixture.property_identities,
        &fixture.getters,
        fixture.getter_identities,
        &fixture.setters,
        fixture.setter_identities,
    )
}

#[test]
fn checked_table_preserves_typed_accessor_relations() {
    let fixture = fixture();
    let first_property = fixture.first_property;
    let second_property = fixture.second_property;
    let first_getter = fixture.first_getter;
    let second_getter = fixture.second_getter;
    let first_setter = fixture.first_setter;
    let identities = checked(fixture).unwrap();

    assert_eq!(identities[first_getter].property(), first_property);
    assert_eq!(identities[second_getter].property(), second_property);
    assert_eq!(identities[first_setter].property(), first_property);
    assert_ne!(identities[first_getter].id(), identities[first_setter].id());
}

#[test]
fn checked_table_rejects_role_owner_and_back_reference_mismatches() {
    let mut wrong_role = fixture();
    let owner = wrong_role.property_identities[wrong_role.first_property].property_owner();
    wrong_role.getter_identities[0] =
        HirPropertyAccessorIdentity::setter(wrong_role.first_property, owner).unwrap();
    assert!(matches!(
        checked(wrong_role),
        Err(HirPropertyAccessorIdentityTableError::Role {
            table: AccessorTable::Getter,
            accessor: 0
        })
    ));

    let mut wrong_owner = fixture();
    let second_owner =
        wrong_owner.property_identities[wrong_owner.second_property].property_owner();
    wrong_owner.getter_identities[0] =
        HirPropertyAccessorIdentity::getter(wrong_owner.first_property, second_owner).unwrap();
    assert!(matches!(
        checked(wrong_owner),
        Err(HirPropertyAccessorIdentityTableError::Owner {
            table: AccessorTable::Getter,
            accessor: 0
        })
    ));

    let mut wrong_property = fixture();
    let owner = wrong_property.property_identities[wrong_property.first_property].property_owner();
    wrong_property.getter_identities[0] =
        HirPropertyAccessorIdentity::getter(wrong_property.second_property, owner).unwrap();
    assert!(matches!(
        checked(wrong_property),
        Err(
            HirPropertyAccessorIdentityTableError::PropertyBackReference {
                table: AccessorTable::Getter,
                property: 0,
                accessor: 0,
                actual_property: 1
            }
        )
    ));
}

#[test]
fn checked_table_rejects_duplicate_and_unowned_accessors() {
    let mut duplicate = fixture();
    duplicate.properties[duplicate.second_property].capability = PropertyCapability::ReadOnly {
        getter: duplicate.first_getter,
    };
    assert!(matches!(
        checked(duplicate),
        Err(HirPropertyAccessorIdentityTableError::DuplicateAccessor {
            table: AccessorTable::Getter,
            accessor: 0
        })
    ));

    let mut unowned = fixture();
    let extra = unowned.getters.alloc(getter());
    assert_eq!(raw_index(extra), 2);
    let owner = unowned.property_identities[unowned.first_property].property_owner();
    unowned
        .getter_identities
        .push(HirPropertyAccessorIdentity::getter(unowned.first_property, owner).unwrap());
    assert!(matches!(
        checked(unowned),
        Err(HirPropertyAccessorIdentityTableError::UnownedAccessor {
            table: AccessorTable::Getter,
            accessor: 2
        })
    ));
}
