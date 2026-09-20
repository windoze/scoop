use super::support::parse_and_lower;

const NEGATIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/errors/protected-override-public.scoop"
));

#[test]
fn protected_overrides_cannot_inherit_a_public_coverage_domain() {
    let errors = parse_and_lower(NEGATIVE).unwrap_err();
    for (name, base, marker, prefix) in [
        (
            "execute",
            "Base.execute",
            "protected override fun execute",
            "protected override fun ",
        ),
        (
            "$get$value",
            "Base.$get$value",
            "protected override var value",
            "protected override var ",
        ),
        (
            "$set$value",
            "Base.$set$value",
            "protected override var value",
            "protected override var ",
        ),
        (
            "expanded",
            "Expanded.expanded",
            "protected override fun expanded",
            "protected override fun ",
        ),
    ] {
        let message = format!("visibility of `{name}` does not cover inherited slot `{base}`");
        let error = errors
            .iter()
            .find(|error| error.message == message)
            .unwrap_or_else(|| panic!("missing {message}: {errors:?}"));
        assert_eq!(
            error.span.unwrap().start as usize,
            NEGATIVE.find(marker).unwrap() + prefix.len()
        );
    }
}
