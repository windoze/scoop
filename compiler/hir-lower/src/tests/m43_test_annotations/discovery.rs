use scoop_hir as hir;
use scoop_identity::SourceRole;

use super::{
    rejects,
    support::{lower, lower_files},
};

#[test]
fn catalogue_retains_direct_methods_hooks_ignored_cases_and_original_names() {
    let output = lower(
        r#"
        package example
        import scoop.test.*
        @Test open class Base {
            @Test open fun inherited() {}
            @BeforeTest fun prepareBase() {}
        }
        @Test class Cases private constructor(val seed: Int = 1): Base() {
            @AfterTest private fun cleanup() {}
            fun helper() {}
            @Test fun `雪 case`() {}
            @BeforeTest private fun prepare() {}
            @Ignore(reason = "later 雪") @Test fun ignored() {}
        }
        @Test class Empty
    "#,
        SourceRole::Test,
    )
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let module = &output.output().export;
    let containers = &module.test_containers;
    assert_eq!(
        containers
            .iter()
            .map(|c| c.qualified_name.as_str())
            .collect::<Vec<_>>(),
        ["example.Base", "example.Cases", "example.Empty"]
    );
    let cases = &containers[1];
    assert_eq!(
        module.class_constructors[cases.constructor].owner,
        cases.class
    );
    assert_eq!(
        module.class_constructors[cases.constructor].parameters[0].name,
        "seed"
    );
    assert_eq!(cases.before.as_ref().unwrap().name, "prepare");
    assert_eq!(cases.after.as_ref().unwrap().name, "cleanup");
    assert_eq!(
        cases
            .cases
            .iter()
            .map(|c| c.method.name.as_str())
            .collect::<Vec<_>>(),
        ["雪 case", "ignored"]
    );
    assert_eq!(
        cases.cases[0].display_name,
        "scoop:scoop.test:0.1.0::example.Cases::雪 case"
    );
    assert_eq!(cases.cases[0].ignore_reason, None);
    assert_eq!(cases.cases[1].ignore_reason.as_deref(), Some("later 雪"));
    for case in &cases.cases {
        assert!(
            module.classes[cases.class]
                .methods
                .contains(&case.method.function)
        );
        assert!(case.method.origin.span.start > cases.origin.span.start);
    }
    assert!(containers[2].cases.is_empty());
}

#[test]
fn private_containers_are_ordered_by_source_path_and_disambiguated() {
    let text = "import scoop.test.*\n@Test private class Cases { @Test fun check() {} }";
    let output = lower_files(&[
        ("tests/a.scoop", text, SourceRole::Test),
        ("tests/z.scoop", text, SourceRole::Test),
    ])
    .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let module = &output.output().export;
    let containers = &module.test_containers;
    assert_eq!(containers.len(), 2);
    for (container, path) in containers.iter().zip(["tests/a.scoop", "tests/z.scoop"]) {
        assert_eq!(
            module.source_files[container.origin.file as usize]
                .identity
                .logical_path()
                .as_str(),
            path
        );
        assert_eq!(
            container.cases[0].display_name,
            format!("scoop:scoop.test:0.1.0::Cases::check ({path})")
        );
    }
    assert_ne!(
        module.nominal_identities[containers[0].class].declaration_id(),
        module.nominal_identities[containers[1].class].declaration_id()
    );
}

#[test]
fn constructor_selection_uses_ordinary_defaults_varargs_and_specificity() {
    for (body, expected) in [
        ("constructor(first: Int = 1) {}", vec!["first"]),
        ("constructor(vararg values: Int) {}", vec!["values"]),
        ("constructor() {}\nconstructor(value: Int = 1) {}", vec![]),
        (
            "private constructor(value: Int = 1) {}\nconstructor(first: String = \"a\", second: String = \"b\") {}",
            vec!["value"],
        ),
        ("protected constructor() {}", vec![]),
    ] {
        let source = format!("import scoop.test.*\n@Test class Cases {{ {body} }}");
        let output = lower(&source, SourceRole::Test)
            .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"));
        let module = &output.output().export;
        let container = &module.test_containers[0];
        let constructor = &module.class_constructors[container.constructor];
        assert_eq!(constructor.owner, container.class);
        assert!(matches!(
            constructor.identity_kind,
            hir::ClassConstructorIdentityKind::Source
        ));
        assert_eq!(
            constructor
                .parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn invalid_construction_is_rejected_even_for_empty_or_ignored_containers() {
    for method in ["", "@Test @Ignore(\"later\") fun ignored() {}"] {
        for (constructors, message) in [
            (
                "constructor(required: Int) {}",
                "expects 1 argument(s), but 0 were supplied",
            ),
            (
                "constructor(first: Int = 1) {}\nconstructor(second: String = \"a\") {}",
                "ambiguous constructor call",
            ),
            ("@Unsafe constructor() {}", "unsafe constructor"),
        ] {
            rejects(
                &format!("@Test class Cases {{ {constructors}\n{method} }}"),
                message,
            );
        }
    }
}
