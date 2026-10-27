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
