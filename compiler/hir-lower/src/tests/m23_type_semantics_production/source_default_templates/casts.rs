use super::*;

#[test]
fn source_default_cast_wire_preserves_generic_checked_types() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-executable-type-sites/ordinary-cast-default.scoop"
    ));
    with_hir_source(source, |output, _| {
        let original = template(output, "withCast", 1);
        let restored = round_trip(output, &original);
        let hir::DefaultExpressionKindV1::Unbox(value) = restored.body().value().kind() else {
            panic!("a generic value cast explicitly extracts its payload")
        };
        let hir::DefaultExpressionKindV1::Cast {
            checked_type,
            optional,
            ..
        } = value.kind()
        else {
            panic!("the actual default body contains a cast")
        };
        assert_eq!(
            *checked_type,
            scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
        );
        assert_eq!(*optional, hir::CanonicalBooleanV1::False);
    });
}
