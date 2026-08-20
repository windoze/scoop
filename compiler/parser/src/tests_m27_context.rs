use crate::tests::{ok, only_function};
use scoop_ast::{ContextParameterLabel, Expr, FunctionBody, StatementKind};

#[test]
fn ordered_context_parameters_and_scope_keep_source_spans() {
    let source = "context(request: Request, _: Logger)\nfun run() = context(request) { request }";
    let file = ok(source);
    let function = only_function(&file);
    assert_eq!(function.context_parameters.len(), 2);
    assert!(
        matches!(&function.context_parameters[0].label, ContextParameterLabel::Named(name) if name.text == "request")
    );
    assert!(matches!(
        &function.context_parameters[1].label,
        ContextParameterLabel::Unnamed(_)
    ));
    let FunctionBody::Expr(body) = &function.body else {
        panic!("expression body");
    };
    let Expr::ContextScope { value, body, span } = body.as_ref() else {
        panic!("context scope");
    };
    assert!(matches!(value.as_ref(), Expr::Var(name) if name.text == "request"));
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        "context(request) { request }"
    );
    assert_eq!(body.statements.len(), 1);
    let dump = scoop_ast::dump(&file);
    assert!(dump.contains("request: Request\n"));
    assert!(dump.contains("ContextScope\n"));
    assert!(!dump.contains("Lambda"));
}

#[test]
fn local_context_function_and_control_flow_are_lexical() {
    let file = ok(
        "fun outer() {\ncontext(_: String) fun local() {}\ncontext(\"x\")\n{ while (true) { break }; return }\n}",
    );
    let FunctionBody::Block(body) = &only_function(&file).body else {
        panic!("block");
    };
    assert!(
        matches!(&body.statements[0].kind, StatementKind::LocalFunction(function) if function.context_parameters.len() == 1)
    );
    assert!(matches!(
        &body.statements[1].kind,
        StatementKind::Expr(Expr::ContextScope { .. })
    ));
}

#[test]
fn qualified_context_and_calls_without_blocks_remain_ordinary() {
    let file = ok("fun run() { service.context(1) { 2 }; context(3) }");
    let dump = scoop_ast::dump(&file);
    assert!(!dump.contains("ContextScope"));
    assert!(dump.contains("Lambda"));
}

#[test]
fn enum_context_members_are_distinct_from_a_variant_named_context() {
    let file = ok(
        "enum Choice { context(val value: String), Ready\ncontext(value: String) fun read(): String = value\n}",
    );
    let scoop_ast::Decl::Enum(declaration) = &file.declarations[0] else {
        panic!("enum declaration");
    };
    assert_eq!(declaration.variants.len(), 2);
    assert_eq!(declaration.variants[0].name.text, "context");
    assert_eq!(declaration.methods[0].context_parameters.len(), 1);
}

#[test]
fn property_and_member_targets_retain_the_list() {
    let file = ok(
        "interface View {\ncontext(_: String) fun read()\ncontext(_: String) val value: Long\n}\ncontext(_: String) val computed: Long get() = 3",
    );
    let scoop_ast::Decl::Interface(interface) = &file.declarations[0] else {
        panic!("interface");
    };
    assert_eq!(interface.methods[0].context_parameters.len(), 1);
    assert_eq!(interface.properties[0].context_parameters.len(), 1);
    let scoop_ast::Decl::Global(property) = &file.declarations[1] else {
        panic!("property");
    };
    assert_eq!(property.context_parameters.len(), 1);
}

#[test]
fn malformed_prefixes_and_scope_values_are_diagnosed_at_the_boundary() {
    for (source, expected) in [
        ("context() fun bad() {}", "cannot be empty"),
        (
            "context(a: String = \"x\") fun bad() {}",
            "cannot have defaults",
        ),
        (
            "context(a: String) context(b: String) fun bad() {}",
            "one `context` prefix",
        ),
        (
            "@NoGC context(a: String) fun bad() {}",
            "before annotations",
        ),
        ("context(a: String); fun bad() {}", "may only prefix"),
        ("context(a: String) class Bad", "may only prefix"),
        (
            "context(vararg a: String) fun bad() {}",
            "context parameter name",
        ),
        ("context(a) fun bad() {}", "after context parameter name"),
        ("fun bad() = context() {}", "exactly one binding value"),
        ("fun bad() = context(1, 2) {}", "exactly one binding value"),
    ] {
        let diagnostics = crate::parse(source).expect_err(source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{source}: {diagnostics:?}"
        );
    }
}
