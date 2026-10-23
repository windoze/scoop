use super::*;

const FAMILY: &str = r#"
    sealed interface Root<T> {}
    interface Mark {}
    struct Present() : Mark { public companion object : Mark {} }
    struct Absent()
    class Always<T> : Root<T>
    struct Conditional<T>(val value: T) {
        override interface Root<T> where T : Mark {}
    }
    class Companion<T> {
        override interface Root<T> where T.Companion : Mark {}
    }
"#;

#[test]
fn m57_sealed_families_preserve_unknown_conditions_and_exclude_false_ones() {
    let output = check(&format!(
        r#"{FAMILY}
        fun <T> all(value: Root<T>): Int = when (value) {{
            is Always<T> -> 1
            is Conditional<T> -> 2
            is Companion<T> -> 3
        }}
        fun absent(value: Root<Absent>): Int = when (value) {{
            is Always<Absent> -> 1
        }}
        fun main() {{
            val a = all(Conditional(Present()))
            val b = all(Companion<Present>())
            val c = absent(Always<Absent>())
        }}
        "#
    ))
    .unwrap();
    let root = output
        .export
        .interfaces
        .values()
        .find(|interface| interface.name == "Root")
        .unwrap();
    assert_eq!(root.inheritance_family.subtypes().unwrap().len(), 3);

    for body in [
        "fun <T> missing(value: Root<T>): Int = when (value) { is Always<T> -> 1 }",
        "fun missing(value: Root<Present>): Int = when (value) { is Always<Present> -> 1; is Conditional<Present> -> 2 }",
    ] {
        let errors = check(&format!("{FAMILY}\n{body}\nfun main() {{}}")).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("non-exhaustive when")),
            "{errors:#?}"
        );
    }
}

#[test]
fn m57_sealed_coverage_uses_the_conditions_of_each_path() {
    check(
        r#"
        interface Mark {}
        interface Tagged {}
        sealed interface Root<T> {}
        class Conditional<T> {
            override interface Root<T> where T : Mark {}
            override interface Tagged where T : Mark {}
        }
        sealed class Branch<T> {
            override interface Root<T> where T : Mark {}
        }
        class Leaf<T> : Branch<T>()
        fun <T> classify(value: Root<T>): Int = when (value) {
            is Tagged -> 1
            is Leaf<T> -> 2
        }
        fun main() {}
    "#,
    )
    .unwrap();
}

#[test]
fn m57_sealed_projection_and_extra_parameters_remain_whole_families() {
    for declaration in [
        "class Hidden<T, U> { override interface Root<T> where U : Mark {} }",
        "class Hidden<U> { override interface Root<U.Companion> where U.Companion : Mark {} }",
    ] {
        let errors = check(&format!(
            r#"
            interface Mark {{}}
            sealed interface Root<T> {{}}
            class Always<T> : Root<T>
            struct Seed() {{ public companion object : Mark {{}} }}
            {declaration}
            fun invalid(value: Root<Seed.Companion>): Int = when (value) {{
                is Always<Seed.Companion> -> 1
            }}
            fun main() {{}}
        "#
        ))
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("non-exhaustive when")
                    && error.message.contains("Hidden")),
            "{errors:#?}"
        );
    }
}

#[test]
fn m57_conditional_sealed_implementations_obey_nonlocal_scope_rules() {
    for body in [
        "class Local<T> { override interface Root where T : Mark {} }",
        "val value = object { override interface Root where T : Mark {} }",
    ] {
        let errors = check(&format!(
            "sealed interface Root {{}}\ninterface Mark {{}}\nfun <T> local() {{ {body} }}\nfun main() {{}}"
        ))
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("named non-local declaration")),
            "{errors:#?}"
        );
    }
}
