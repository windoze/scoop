use super::*;

#[test]
fn safe_method_extension_and_callable_property_results_can_be_discarded() {
    let source = scoop_parser::parse(
        r#"
        class Resource(val callback: () -> Int) {
            fun close(): Int = callback()
        }
        class Holder(val resource: Resource)
        fun Resource.extra() { callback() }
        fun invoke(resource: Resource?, holder: Holder?) {
            resource?.close()
            resource?.extra()
            resource?.callback()
            holder?.resource?.close()
            try { resource?.close() } finally { resource?.extra() }
        }
        fun main() {}
        "#,
    )
    .unwrap();
    lower_user(source).expect("safe calls are calls even when their result is a hidden local");
}

#[test]
fn safe_field_reads_are_still_not_call_statements() {
    let source = scoop_parser::parse(
        "class Resource(val value: Int)\nfun read(resource: Resource?) { resource?.value }\nfun main() {}",
    )
    .unwrap();
    let errors = lower_user(source).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("statement must be a function call")),
        "{errors:#?}"
    );
}
