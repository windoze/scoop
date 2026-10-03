use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/protected-lexical-scopes.scoop"
));

#[test]
fn lexical_classes_supply_protected_access_and_allow_private_setters() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let module = output.export.module();
    let property = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "state")
        .unwrap()
        .1;
    assert_eq!(property.access.declared, hir::DeclaredVisibility::Protected);
    let setter = &module.property_setters[property.capability.setter().unwrap()];
    assert_eq!(setter.access.declared, hir::DeclaredVisibility::Private);
    assert!(setter.access.slot.is_none());
    let hir::PropertyOwner::Class(owner) = property.owner else {
        panic!("class-owned property");
    };
    assert_eq!(
        setter.access.lookup.0.constraints(),
        &[hir::AccessConstraint::LexicalOwner(
            module.nominal_identities[owner].declaration_id()
        )]
    );
}
