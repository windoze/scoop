use super::*;

const RESOURCE: &str = r#"
interface Carrier { fun read(): Int }
class Resource : Carrier { public override fun read(): Int = 11 }
fun <T : Carrier, R> T.scope(block: (T) -> R): R = block(this)
suspend fun <T : Carrier, R> T.scope(block: suspend (T) -> R): R = block(this)
fun <T> keep(value: T): T = value
"#;

fn check(source: &str) -> hir::ExportHirOutput {
    lower_user(scoop_parser::parse(&format!("{RESOURCE}\n{source}")).unwrap()).unwrap()
}

#[test]
fn receiver_types_lambda_parameters_before_its_result_is_known() {
    let module = check(
        r#"fun main() {
            val direct = Resource().scope { resource -> resource.read() }
            val resource = Resource()
            val bound = resource.scope { value -> value.read() }
            val implicit = resource.scope { it.read() }
            val explicit = resource.scope { value: Resource -> value.read() }
            val nested = keep(resource.scope { value -> value.scope { it.read() } })
        }"#,
    );
    let hir::FunctionKind::User(body) = &module.functions[module.entry()].kind else {
        panic!("main has a source body")
    };
    for name in ["direct", "bound", "implicit", "explicit", "nested"] {
        assert_eq!(local_init(body, name).ty, int_type(&module), "{name}");
    }
}

#[test]
fn suspend_and_receiver_lambdas_infer_their_results_from_the_body() {
    check(
        r#"
        fun <T : Carrier, R> T.inspect(block: T.() -> R): R = this.block()
        suspend fun read(): Int {
            val resource = Resource()
            val direct = Resource().scope(suspend { value -> value.read() })
            val bound = resource.scope(suspend { it.read() })
            return resource.inspect { read() }
        }
        fun main() {}
        "#,
    );
}

#[test]
fn an_expected_result_can_complete_a_context_dependent_lambda_body() {
    check(
        r#"fun main() {
            val implicit: Int? = Resource().scope { None }
            val named: Int? = Resource().scope { value -> None }
            val explicit: Int? = Resource().scope { value: Resource -> None }
        }"#,
    );
}

#[test]
fn a_later_lambda_can_supply_the_missing_result_constraint() {
    check(
        r#"
        fun <T, R> collect(value: T, first: (T) -> R?, second: (T) -> R): R = second(value)
        fun main() {
            val result = collect(Resource(), { value -> None }, { value -> value.read() })
        }
        "#,
    );
}

#[test]
fn partial_lambda_hints_do_not_relax_effects_arity_or_missing_constraints() {
    for (source, message) in [
        (
            "fun main() { Resource().scope(suspend { it.read() }) }",
            "suspend",
        ),
        (
            "suspend fun read(): Int = 1\nfun main() { Resource().scope { read() } }",
            "suspend",
        ),
        (
            "fun main() { Resource().scope { first, second -> 1 } }",
            "lambda has 2 parameter(s)",
        ),
        (
            "fun <T, R> missing(block: (T) -> R): R = throw Throwable()\nfun main() { missing { value -> 1 } }",
            "lambda parameter requires a type",
        ),
    ] {
        let source = scoop_parser::parse(&format!("{RESOURCE}\n{source}")).unwrap();
        let errors = lower_user(source).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{errors:#?}"
        );
    }
}
