use scoop_ast::{
    self as ast, DeclarationFragment, RecoveredArgumentValue as Value,
    RecoveredCallTarget as Target,
};
use scoop_parser::{ParsedFile, parse_recovering};

#[path = "recovered_calls/callables.rs"]
mod callables;
#[path = "recovered_calls/trailing.rs"]
mod trailing;

fn body<'a>(parsed: &'a ParsedFile, name: &str) -> &'a ast::RecoveredFunctionBody {
    parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let DeclarationFragment::FunctionBody(body) = &item.value else {
                return None;
            };
            let declaration = &parsed.declarations()[item.declaration.0];
            (declaration.name.as_ref().unwrap().text == name).then_some(body.as_ref())
        })
        .unwrap_or_else(|| panic!("missing body of {name}: {:?}", parsed.diagnostics()))
}

fn source(text: &str, span: ast::Span) -> &str {
    &text[span.start as usize..span.end as usize]
}

#[test]
fn failed_arguments_keep_source_slots_names_spreads_and_real_separators() {
    let text = "fun editing() { call(true, name = , *items, false) }";
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.syntax().text().to_string(), text);
    let body = body(&parsed, "editing");
    assert_eq!(body.calls.len(), 1);
    let call = &body.calls[0];
    assert!(matches!(&call.target, Target::Named(name) if name.text == "call"));
    assert_eq!(call.block, Some(body.blocks[0].id));
    let arguments = &call.arguments.as_ref().unwrap();
    assert_eq!(source(text, arguments.opening), "(");
    assert!(arguments.closed);
    assert_eq!(arguments.commas.len(), 3);
    assert!(
        arguments
            .commas
            .iter()
            .all(|span| source(text, *span) == ",")
    );
    assert_eq!(arguments.items.len(), 4);
    assert!(
        matches!(&arguments.items[0].value, Value::Complete(expression) if matches!(**expression, ast::Expr::BoolLiteral { value: true, .. }))
    );
    assert!(
        matches!(&arguments.items[1].name, ast::CallArgumentName::Named(name) if name.text == "name")
    );
    assert!(matches!(arguments.items[1].value, Value::Unavailable(span) if span.start == span.end));
    assert!(
        matches!(arguments.items[2].spread, ast::SpreadSyntax::Spread(span) if source(text, span) == "*")
    );
    assert!(
        matches!(&arguments.items[2].value, Value::Complete(expression) if source(text, expression.span()) == "items")
    );
    assert!(
        matches!(&arguments.items[3].value, Value::Complete(expression) if matches!(**expression, ast::Expr::BoolLiteral { value: false, .. }))
    );
}

#[test]
fn empty_slots_and_eof_remain_distinct_from_omitted_arguments() {
    for (text, count, closed) in [
        ("fun editing() = call(", 0, false),
        ("fun editing() = call(, , value, )", 4, true),
        ("fun editing() = call(value, named = *", 2, false),
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        let call = &body.calls[0];
        assert_eq!(call.block, None);
        assert_eq!(call.arguments.as_ref().unwrap().closed, closed);
        assert_eq!(call.arguments.as_ref().unwrap().items.len(), count);
        assert_eq!(call.arguments.as_ref().unwrap().end as usize, text.len());
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn a_missing_comma_does_not_invent_a_separator_or_remove_complete_inputs() {
    let text = "fun editing() = call(true\nfalse)";
    let parsed = parse_recovering(text).unwrap();
    let arguments = &body(&parsed, "editing").calls[0]
        .arguments
        .as_ref()
        .unwrap();
    assert!(arguments.commas.is_empty());
    assert_eq!(arguments.items.len(), 2);
    assert!(
        arguments
            .items
            .iter()
            .all(|argument| matches!(argument.value, Value::Complete(_)))
    );
    assert!(parsed.ast().is_none());
}

#[test]
fn nested_errors_never_become_complete_outer_arguments_or_receivers() {
    let text = "fun editing() { outer(inner(,), true)\nmake(,).method(,) }";
    let parsed = parse_recovering(text).unwrap();
    let calls = &body(&parsed, "editing").calls;
    assert_eq!(calls.len(), 4);
    let outer = calls
        .iter()
        .find(|call| matches!(&call.target, Target::Named(name) if name.text == "outer"))
        .unwrap();
    assert!(matches!(
        outer.arguments.as_ref().unwrap().items[0].value,
        Value::Unavailable(_)
    ));
    assert!(matches!(
        outer.arguments.as_ref().unwrap().items[1].value,
        Value::Complete(_)
    ));
    assert!(matches!(
        calls.last().unwrap().target,
        Target::Unavailable(_)
    ));
}

#[test]
fn call_forms_and_unicode_positions_follow_the_original_grammar() {
    for (text, kind) in [
        ("fun 中文() = 值?.方法<Boolean>(参数 = )", "member"),
        ("fun editing() = (value)(,)", "invoke"),
        ("fun editing() = super.method(,)", "super"),
        (
            "fun editing() = super<Base<Boolean>>.method(,)",
            "qualified",
        ),
    ] {
        let parsed = parse_recovering(text).unwrap();
        let name = if kind == "member" {
            "中文"
        } else {
            "editing"
        };
        let call = &body(&parsed, name).calls[0];
        match (&call.target, kind) {
            (
                Target::Member {
                    receiver,
                    name,
                    navigation,
                },
                "member",
            ) => {
                assert_eq!(source(text, receiver.span()), "值");
                assert_eq!(source(text, name.span), "方法");
                assert_eq!(*navigation, ast::Navigation::Safe);
                assert_eq!(call.type_args.len(), 1);
            }
            (Target::Invoke(callee), "invoke") => assert_eq!(source(text, callee.span()), "value"),
            (Target::Super { super_span, name }, "super") => {
                assert_eq!(source(text, *super_span), "super");
                assert_eq!(name.text, "method");
            }
            (Target::QualifiedSuper { qualifier, .. }, "qualified") => {
                assert_eq!(source(text, qualifier.span), "Base<Boolean>");
            }
            _ => panic!("wrong call form: {:?}", call.target),
        }
        assert_eq!(source(text, call.arguments.as_ref().unwrap().opening), "(");
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn lexical_errors_do_not_expose_a_complete_argument() {
    let text = r#"fun editing() = call("\q", true)"#;
    let parsed = parse_recovering(text).unwrap();
    let call = &body(&parsed, "editing").calls[0];
    assert!(matches!(
        call.arguments.as_ref().unwrap().items[0].value,
        Value::Unavailable(_)
    ));
    assert!(matches!(
        call.arguments.as_ref().unwrap().items[1].value,
        Value::Complete(_)
    ));
}

#[test]
fn complete_calls_are_not_copied_and_nested_functions_keep_their_own_calls() {
    let text = "fun outer() { healthy(true)\nfun inner() { nested(,) }\nouterCall(,) }";
    let parsed = parse_recovering(text).unwrap();
    let outer = body(&parsed, "outer");
    let inner = body(&parsed, "inner");
    assert_eq!(outer.calls.len(), 1);
    assert_eq!(inner.calls.len(), 1);
    assert!(matches!(&outer.calls[0].target, Target::Named(name) if name.text == "outerCall"));
    assert!(matches!(&inner.calls[0].target, Target::Named(name) if name.text == "nested"));
    assert_eq!(inner.calls[0].block, Some(inner.blocks[0].id));
    assert!(inner.blocks[0].parent.is_none());
}

#[test]
fn recovered_calls_keep_complete_and_damaged_trailing_lambdas() {
    for (text, complete) in [
        ("fun editing() = call(,) { value -> value }", true),
        ("fun editing() = call(,) { value -> value. }", false),
    ] {
        let parsed = parse_recovering(text).unwrap();
        let call = &body(&parsed, "editing").calls[0];
        let trailing = call.trailing.as_ref().unwrap();
        assert!(matches!(
            trailing.name,
            ast::CallArgumentName::TrailingLambda
        ));
        assert_eq!(matches!(trailing.value, Value::Complete(_)), complete);
        if let Value::Complete(expression) = &trailing.value {
            assert!(
                matches!(expression.as_ref(), ast::Expr::Lambda { label: Some(label), .. } if label.text == "call")
            );
        }
        assert!(trailing.span.start > call.arguments.as_ref().unwrap().end);
        assert_eq!(call.span.end, trailing.span.end);
        assert_eq!(call.span.end as usize, text.len());
        assert_eq!(
            source(text, trailing.span),
            if complete {
                "{ value -> value }"
            } else {
                "{ value -> value. }"
            }
        );
    }
}

#[test]
fn complete_lambda_arguments_keep_the_ordinary_implicit_call_label() {
    let text = "fun editing() = call({ return@call true }, )";
    let parsed = parse_recovering(text).unwrap();
    let Value::Complete(expression) = &body(&parsed, "editing").calls[0]
        .arguments
        .as_ref()
        .unwrap()
        .items[0]
        .value
    else {
        panic!("complete lambda input")
    };
    assert!(
        matches!(expression.as_ref(), ast::Expr::Lambda { label: Some(label), .. } if label.text == "call")
    );
}
