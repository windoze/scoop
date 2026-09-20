use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/protected-lexical-scopes.scoop"
));
const NEGATIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/errors/protected-lexical-scopes.scoop"
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
            hir::VisibilityOwner::Class(owner)
        )]
    );
}

#[test]
fn nested_scopes_preserve_receiver_and_private_setter_restrictions() {
    let errors = parse_and_lower(NEGATIVE).unwrap_err();
    let expected = [
        (
            "target.secret()",
            "protected method `secret` cannot be accessed through receiver of static type `Base`; receiver must be `Derived` or one of its subclasses",
        ),
        (
            "peer.secret()",
            "protected method `secret` cannot be accessed through receiver of static type `Sibling`; receiver must be `Derived` or one of its subclasses",
        ),
        (
            "target.state = 2",
            "setter of property `state` is not accessible",
        ),
        (
            "target.secret()\n}",
            "method `secret` is not accessible here",
        ),
    ];
    for (occurrence, message) in expected {
        let error = errors
            .iter()
            .find(|error| error.message == message)
            .unwrap_or_else(|| panic!("missing {message}: {errors:?}"));
        assert_eq!(
            error.span.unwrap().start as usize,
            NEGATIVE.find(occurrence).unwrap() + occurrence.find('.').unwrap() + 1
        );
    }
}
