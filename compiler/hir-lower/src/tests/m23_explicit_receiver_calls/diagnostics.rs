use super::*;

fn check_local_and_imported(
    provider: &str,
    consumer: &str,
) -> [Result<(), Vec<ast::Diagnostic>>; 2] {
    [
        lower(&[
            complete_core_file(),
            scoop_parser::parse(&format!("{provider}\n{consumer}")).unwrap(),
        ])
        .map(|_| ()),
        crate::tests::m23_generic_body_consumption::with_provider_consumer(
            &format!("package extensiondiagnostics\n{provider}"),
            &format!("import extensiondiagnostics.*\n{consumer}"),
            |_, _, _, _, _| (),
        ),
    ]
}

#[test]
fn extension_failures_retain_lambda_errors_in_both_declaration_orders() {
    let declarations = [
        "public fun <R> A.run1(block: () -> R): R = block()",
        "public fun <R> B.run1(block: () -> R): R = block()",
    ];
    for order in [[0, 1], [1, 0]] {
        let provider = format!(
            "public class A public constructor()\npublic class B public constructor()\n{}\n{}",
            declarations[order[0]], declarations[order[1]],
        );
        let consumer = "@Unsafe fun danger(): Long = 1L\n\
            fun main() { @Unsafe { B().run1 { danger() } } }";
        for result in check_local_and_imported(&provider, consumer) {
            let errors = result.unwrap_err();
            assert!(
                errors.iter().any(|error| error.message.contains(
                    "unsafe function `danger` may only be called from an unsafe context"
                )),
                "{errors:#?}"
            );
        }

        let safe_consumer = consumer.replace("{ danger() }", "{ @Unsafe { danger() } }");
        for result in check_local_and_imported(&provider, &safe_consumer) {
            result.expect("failed receiver candidates must not affect a valid lambda");
        }
    }
}

#[test]
fn extension_failures_deduplicate_context_independent_argument_errors() {
    let provider = "public class Owner public constructor()\n\
        public fun Owner.pick(value: Long): Long = value\n\
        public fun Owner.pick(value: String): String = value";
    for result in check_local_and_imported(provider, "fun main() { Owner().pick(missing) }") {
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert!(
            errors[0].message.contains("unknown variable `missing`"),
            "{errors:#?}"
        );
    }
}

#[test]
fn non_callable_properties_report_the_property_type() {
    let providers = [
        "public class Owner public constructor(public val count: Long = 1L)",
        "public class Owner public constructor()\n\
         public val Owner.count: Long get() = 1L",
    ];
    for provider in providers {
        for result in check_local_and_imported(provider, "fun main() { Owner().count() }") {
            let errors = result.unwrap_err();
            assert_eq!(errors.len(), 1, "{errors:#?}");
            assert_eq!(errors[0].message, "value of type Long is not callable");
            let span = errors[0]
                .span
                .expect("the property error has a source span");
            assert_eq!(span.end - span.start, "count".len() as u32);
        }
    }
}

#[test]
fn non_callable_properties_do_not_block_applicable_functions() {
    let providers = [
        "public class Owner public constructor(public val count: Long = 1L)\n\
         public fun Owner.count(): Long = 2L",
        "public class Owner public constructor()\n\
         public val Owner.count: Long get() = 1L\n\
         public fun Owner.count(): Long = 2L",
    ];
    for provider in providers {
        for result in check_local_and_imported(provider, "fun main() { Owner().count() }") {
            result.expect("a non-callable property must not block an applicable function");
        }
    }
}

#[test]
fn callable_properties_retain_argument_errors() {
    let provider = "public class Owner public constructor(public val action: (Long) -> Long)";
    let consumer = "fun main() { Owner({ x: Long -> x }).action(\"bad\") }";
    for result in check_local_and_imported(provider, consumer) {
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert!(errors[0].message.contains("Long"), "{errors:#?}");
        assert!(errors[0].message.contains("String"), "{errors:#?}");
        assert!(!errors[0].message.contains("not callable"), "{errors:#?}");
    }
}

#[test]
fn failed_property_initializers_do_not_cascade_into_overload_errors() {
    let provider = "public val String.charLength: Long get() = 13L\n\
        public fun assertEquals(expected: Long, actual: Long) {}\n\
        public fun assertEquals(expected: String, actual: String) {}";
    let consumer = "fun main() {\n\
        val x = \"Hello, Scoop!\".charLength()\n\
        assertEquals(13L, x)\n\
    }";
    for result in check_local_and_imported(provider, consumer) {
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].message, "value of type Long is not callable");
    }
}

#[test]
fn named_values_report_non_callable_types() {
    for result in
        check_local_and_imported("public val count: Long = 1L", "fun main() { count(100) }")
    {
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert_eq!(errors[0].message, "value of type Long is not callable");
    }
}

#[test]
fn ordinary_expressions_and_local_values_report_non_callable_types() {
    for (source, expected) in [
        ("fun main() { val x = 1; x(100) }", "Int"),
        ("fun main() { val x = true; x(100) }", "Boolean"),
        ("fun main() { (1)(100) }", "Int"),
        ("fun main() { \"text\"(100) }", "String"),
        ("fun use(x: Long) { x(100) }\nfun main() {}", "Long"),
        (
            "fun main() { val x = 1; val run: () -> Unit = { x(100) } }",
            "Int",
        ),
    ] {
        let errors =
            lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap_err();
        assert_eq!(errors.len(), 1, "{source}\n{errors:#?}");
        assert_eq!(
            errors[0].message,
            format!("value of type {expected} is not callable"),
            "{source}"
        );
    }
}

#[test]
fn non_callable_local_values_do_not_block_applicable_named_functions() {
    let source = "fun x(value: Int): Int = value\n\
        fun main() { val x = 1; x(100) }";
    lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()])
        .expect("an ordinary non-callable local does not enter the callable layer");
}
