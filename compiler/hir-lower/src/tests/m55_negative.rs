use super::*;

fn check(source: &str) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
}

#[test]
fn m55_negative_overloads_and_forwarding() {
    check(
        r#"
        interface Debug {}
        fun <T> select(value: T): Int = 1
        fun <T> select(value: T): Int where T : !Debug = 2
        fun <T> general(value: T): Int = select(value)
        fun <T : !Debug> forward(value: T): Int = select(value)
        fun main() {
            val a = general(1)
            val b = forward(1)
            val c = select(1)
        }
    "#,
    )
    .unwrap();
}

#[test]
fn m55_negative_recursive_calls_keep_the_declared_argument() {
    check(
        r#"
        interface Debug {}
        fun <T : !Debug> recursive(value: T, depth: Int): String {
            fun <U : !Debug> local(item: U): String = "local"
            if (depth == 0) return local(value)
            return recursive(value, depth - 1)
        }
        fun main() { val result = recursive(1, 3) }
    "#,
    )
    .unwrap();
}

#[test]
fn m55_negative_unknown_and_partial_application_are_not_absence() {
    for subject in ["T", "Box<T>"] {
        let errors = check(&format!(
            r#"
            interface I<T> {{}}
            struct Box<T>(val value: T) : I<T>
            fun <T> absent(value: T): Int where T : !I<String> = 1
            fun <T> unknown(value: {subject}): Int = absent(value)
        "#
        ))
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("does not establish this negative fact")),
            "{errors:#?}"
        );
    }
}

#[test]
fn m55_condition_identity_ignores_aliases_and_parameter_spelling() {
    let errors = check(
        r#"
        interface Left {}
        interface Right {}
        typealias Alias = Left
        fun <T> select(value: T): Int where T : !Left, T : !Right = 1
        fun <U : !Right> select(value: U): Int where U : !Alias = 2
    "#,
    )
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("already declared with the same signature")),
        "{errors:#?}"
    );
}

#[test]
fn m55_negative_inheritance_facts_and_contradictions() {
    check(
        r#"
        interface Parent {}
        interface Child : Parent {}
        fun <T : !Child> absent(value: T): Int = 1
        fun <T : !Parent> forward(value: T): Int = absent(value)
        fun main() {}
    "#,
    )
    .unwrap();
    let errors = check(
        r#"
        interface Parent {}
        interface Child : Parent {}
        fun <T : Child> impossible(value: T): Int where T : !Parent = 1
    "#,
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("contradicts upper bound")),
        "{errors:#?}"
    );
}
