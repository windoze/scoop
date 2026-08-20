use super::*;

#[test]
fn imported_derived_equality_member_keeps_its_generated_identity() {
    with_provider_consumer(
        r#"
        package values
        public struct Point(val value: Boolean) {
            public fun equals(other: Any): Boolean = false
        }
        "#,
        r#"
        import values.Point
        public fun same(left: Point, right: Point): Boolean = left.equals(other = right)
        "#,
        |output, _, _, _, _| {
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let local = output.output().local.module();
            assert_eq!(local.imported_derived_equalities.len(), 1);
            assert!(local.generated_callable_identities.is_empty());
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn imported_derived_equality_member_participates_in_normal_overload_selection() {
    let errors = with_provider_consumer(
        r#"
        package values
        public struct Point(val value: Boolean) {
            public fun equals(other: String): Boolean = false
        }
        "#,
        r#"
        import values.Point
        public fun same(left: Point, right: Nothing): Boolean = left.equals(right)
        "#,
        |_, _, _, _, _| (),
    )
    .expect_err("unrelated Point and String parameters must remain ambiguous");
    assert!(
        errors.iter().any(|error| {
            error.message.contains("call to `equals` is ambiguous")
                && error.message.contains("Point")
                && error.message.contains("String")
        }),
        "{errors:?}"
    );
}
