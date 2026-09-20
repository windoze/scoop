use super::support::parse_and_lower;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/protected-nested-receivers.scoop"
));
const NEGATIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/errors/protected-nested-receivers.scoop"
));

#[test]
fn protected_nested_owner_domains_do_not_restrict_ordinary_member_receivers() {
    parse_and_lower(POSITIVE).unwrap();
}

#[test]
fn protected_nested_members_keep_their_own_receiver_and_type_visibility_restrictions() {
    let errors = parse_and_lower(NEGATIVE).unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:?}");
    for (occurrence, message) in [
        (
            "secret()",
            "protected method `secret` cannot be accessed through receiver of static type `Outer.Hidden`; receiver must be `Sub` or one of its subclasses",
        ),
        ("state = 2", "setter of property `state` is not accessible"),
    ] {
        let offset = NEGATIVE.rfind(occurrence).unwrap();
        let error = errors
            .iter()
            .find(|e| {
                e.span
                    .is_some_and(|span| span.start as usize <= offset && span.end as usize > offset)
            })
            .unwrap_or_else(|| panic!("missing diagnostic at {occurrence}: {errors:?}"));
        assert_eq!(error.message, message);
        assert_eq!(error.span.unwrap().start as usize, offset);
    }
}

#[test]
fn protected_nested_type_remains_hidden_outside_its_owner_scope() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m21-visibility/errors/protected-nested-type.scoop"
    ));
    let errors = parse_and_lower(source).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "type `Outer.Hidden` is not accessible from this source location"
    );
    assert_eq!(
        errors[0].span.unwrap().start as usize,
        source.rfind("Outer.Hidden").unwrap()
    );
}
