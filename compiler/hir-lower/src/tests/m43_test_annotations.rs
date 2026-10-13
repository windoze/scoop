use scoop_ast as ast;
use scoop_identity::SourceRole;

mod discovery;
mod entry;
mod imports;
mod support;
use support::lower;

fn rejects(source: &str, message: &str) -> Vec<ast::Diagnostic> {
    let errors = match lower(&format!("import scoop.test.*\n{source}"), SourceRole::Test) {
        Ok(_) => panic!("accepted invalid test source: {source}"),
        Err(errors) => errors,
    };
    assert!(
        errors.iter().any(|error| error.message.contains(message)
            && error
                .source
                .as_deref()
                .is_some_and(|source| source.logical_path().as_str() == "tests/check.scoop")
            && error.span.is_some()),
        "expected {message}: {errors:#?}"
    );
    errors
}

#[test]
fn official_markers_allow_private_containers_hooks_aliases_and_ordinary_helpers() {
    lower(
        r#"
        import scoop.test.Test as Check
        import scoop.test.BeforeTest
        import scoop.test.AfterTest
        import scoop.test.Ignore
        open class Base { fun inherited(value: Int): Int = value }
        @Check private class Cases private constructor(val initial: Int = 1): Base() {
            @BeforeTest private fun prepare() {}
            @AfterTest private fun cleanup() {}
            @Check private fun `works 雪`() { helper(1) }
            @Check @Ignore("later") fun ignored() {}
            fun <T> helper(value: T): T = value
        }
        @Check class Empty
    "#,
        SourceRole::Test,
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}

#[test]
fn official_markers_are_rejected_in_production_sources() {
    for declaration in [
        "@Test class Cases",
        "@Test fun check() {}",
        "@BeforeTest fun before() {}",
        "@AfterTest fun after() {}",
        "@Ignore(\"later\") fun ignored() {}",
    ] {
        let errors = match lower(
            &format!("import scoop.test.*\n{declaration}"),
            SourceRole::Production,
        ) {
            Ok(_) => panic!("accepted production test marker"),
            Err(errors) => errors,
        };
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("only allowed in test sources")),
            "{errors:#?}"
        );
    }
}

#[test]
fn test_containers_reject_non_class_generic_abstract_sealed_local_and_nested_shapes() {
    for source in [
        "@Test struct Cases()",
        "@Test enum Cases { A }",
        "@Test interface Cases {}",
        "@Test object Cases {}",
        "@Test class Cases<T>",
        "@Test abstract class Cases",
        "@Test sealed class Cases",
        "class Outer { @Test class Cases }",
        "fun local() { @Test class Cases }",
    ] {
        rejects(source, "a test container must be");
    }
}

#[test]
fn test_methods_require_a_direct_marked_container_and_a_safe_unit_signature() {
    for source in [
        "@Test fun check() {}",
        "class Cases { @Test fun check() {} }",
        "fun outer() { @Test fun local() {} }",
    ] {
        rejects(source, "must be declared directly");
    }
    for (method, message) in [
        ("@Test fun check(value: Int) {}", "value parameters"),
        ("@Test fun check(value: Int = 1) {}", "value parameters"),
        ("@Test fun <T> check() {}", "type parameters"),
        ("@Test suspend fun check() {}", "suspend"),
        ("@Test fun check(): Int = 1", "return type must be Unit"),
        ("@Test @Unsafe fun check() {}", "unsafe calls"),
        (
            "context(value: String) @Test fun check() {}",
            "context parameters",
        ),
    ] {
        rejects(&format!("@Test class Cases {{ {method} }}"), message);
    }
}

#[test]
fn marker_targets_arguments_repetition_and_roles_use_normal_annotation_rules() {
    for (source, message) in [
        ("@Test @Test class Cases", "must not be repeated"),
        ("@Test(1) class Cases", "too many annotation arguments"),
        (
            "@Test class Cases { @Test @Ignore fun check() {} }",
            "missing annotation argument `reason`",
        ),
        ("@Ignore(\"later\") class Cases", "not allowed on CLASS"),
        (
            "@Test annotation class Cases",
            "not allowed on ANNOTATION_CLASS",
        ),
        (
            "@Test class Cases { @Test @BeforeTest fun check() {} }",
            "only one of",
        ),
        (
            "@Test class Cases { @Ignore(\"later\") fun helper() {} }",
            "requires `@Test`",
        ),
        (
            "@Test class Cases { @BeforeTest @Ignore(\"later\") fun before() {} }",
            "requires `@Test`",
        ),
    ] {
        rejects(source, message);
    }
}

#[test]
fn duplicate_hooks_report_both_declarations() {
    for marker in ["BeforeTest", "AfterTest"] {
        let source = format!(
            "@Test class Cases {{ @{marker} fun first() {{}}; @{marker} fun second() {{}} }}"
        );
        let errors = rejects(&source, "at most one");
        let diagnostic = errors
            .iter()
            .find(|e| e.message.contains("at most one"))
            .unwrap();
        assert_eq!(diagnostic.notes.len(), 1);
        assert_ne!(diagnostic.span, Some(diagnostic.notes[0].span));
    }
}

#[test]
fn same_named_user_annotations_have_no_test_semantics() {
    lower(
        "annotation class Test\n@Test struct Ordinary(val value: Int)",
        SourceRole::Production,
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
}
