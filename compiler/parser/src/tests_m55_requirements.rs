use scoop_ast::{CallTypeArgument, Decl, RequiredCall, RequirementArgumentValue, SpreadSyntax};

use crate::tests::{err, ok};

#[test]
fn member_requirements_retain_call_shapes_and_effects() {
    let file = ok(r#"
        fun <T> inspect(value: T): String
            where T : !Debug,
                  requires(x: T, text: String) {
                      @NoGC x.format<String, _>(prefix = text, *rest: Array<String>): String;
                      suspend T.Companion.read(): String
                  } = "ok"
    "#);
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("function")
    };
    let clause = function.where_clause.as_ref().unwrap();
    assert_eq!(clause.constraints.len(), 1);
    let block = &clause.requirements[0];
    assert_eq!(block.parameters.len(), 2);
    let first = &block.calls[0];
    assert_eq!(first.annotations[0].name.to_string(), "NoGC");
    let RequiredCall::Method {
        type_arguments,
        companion,
        name,
        ..
    } = &first.call
    else {
        panic!("method")
    };
    assert!(!companion);
    assert_eq!(name.text, "format");
    assert!(matches!(type_arguments[1], CallTypeArgument::Infer { .. }));
    assert_eq!(first.arguments[0].name.as_ref().unwrap().text, "prefix");
    assert!(matches!(
        first.arguments[0].value,
        RequirementArgumentValue::Parameter(_)
    ));
    assert!(matches!(first.arguments[1].spread, SpreadSyntax::Spread(_)));
    assert!(matches!(
        first.arguments[1].value,
        RequirementArgumentValue::Inline { .. }
    ));
    assert!(block.calls[1].suspend_span.is_some());
    assert!(matches!(
        block.calls[1].call,
        RequiredCall::Method {
            companion: true,
            ..
        }
    ));
    let dump = scoop_ast::dump(&file);
    assert!(dump.contains("requires(x: T, text: String)"));
    assert!(dump.contains("T.Companion.read(): String"));
}

#[test]
fn requirements_allow_omitted_virtual_parameters_and_constructor_result() {
    for source in [
        "fun <T> make(): T where requires { T(input: String) } = T(\"value\")",
        "fun <T> make(): T where requires() { T(input: String): T } = T(\"value\")",
    ] {
        let file = ok(source);
        let Decl::Function(function) = &file.declarations[0] else {
            panic!("function")
        };
        let block = &function.where_clause.as_ref().unwrap().requirements[0];
        assert!(block.parameters.is_empty());
        assert!(matches!(
            block.calls[0].call,
            RequiredCall::Constructor { .. }
        ));
    }
}

#[test]
fn requires_remains_an_ordinary_identifier_outside_the_block_prefix() {
    ok("fun requires(value: Int): Int = value
fun <requires> use(value: requires): requires where requires : ToString = value");
    ok("fun <requires> use(value: requires): Int where requires.Companion : ref = 1");
}

#[test]
fn requirement_grammar_rejects_untyped_results_and_executable_expressions() {
    for source in [
        "fun <T> f(value: T) where requires {} {}",
        "fun <T> f(value: T) where requires(x: T) { x.read() } {}",
        "fun <T> f(value: T) where requires(x: T) { val y = x } {}",
        "fun <T> f(value: T) where requires(x: T) { x.read(1): Int } {}",
        "fun <T> f(value: T) where requires(x: T) { x.read(x.other()): Int } {}",
        "fun <T> f(value: T) where requires(x: T) { x.read(): Int x.other(): Int } {}",
        "fun <T> f(value: T) where requires(x: T) { suspend suspend x.read(): Int } {}",
    ] {
        err(source);
    }
}
