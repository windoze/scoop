use super::*;

// --- when statements ----------------------------------------------------------

#[test]
fn when_with_bare_and_prefixed_unit_variants_and_else() {
    let when = when_with_arms(
        "        Red -> { print(\"r\") }\n        Color.Green -> { }\n        else -> { println(\"o\") }\n",
    );
    assert_eq!(when.arms.len(), 2);
    // A bare identifier is always a binding ("binding first"); HIR
    // resolves it to the unit variant.
    assert!(matches!(&when.arms[0].pattern, Pattern::Binding(name) if name.text == "Red"));
    // `Color.Green` is a unit variant pattern with a type prefix.
    let Pattern::Positional {
        path,
        elements,
        rest,
        ..
    } = &when.arms[1].pattern
    else {
        panic!("expected a positional pattern");
    };
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].text, "Color");
    assert_eq!(path[1].text, "Green");
    assert!(elements.is_empty());
    assert!(rest.is_none());
    assert!(when.else_body.is_some());
}

#[test]
fn when_statement_dump() {
    assert_eq!(
        stmt_dump(
            "when (c) {\n        Red -> { print(\"r\") }\n        else -> { println(\"o\") }\n    }"
        ),
        "when\n  Var c\n  arm Red\n    Call print\n      StringLiteral \"r\"\n  else\n    Call println\n      StringLiteral \"o\"\n"
    );
}

#[test]
fn when_arm_with_guard() {
    let when = when_with_arms("        Some(x) if (x > 0) -> { println(x) }\n");
    assert_eq!(when.arms.len(), 1);
    let arm = &when.arms[0];
    let Pattern::Positional { path, elements, .. } = &arm.pattern else {
        panic!("expected a positional pattern");
    };
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].text, "Some");
    assert!(matches!(&elements[0], Pattern::Binding(name) if name.text == "x"));
    let guard = arm.guard.as_ref().expect("guard present");
    assert!(matches!(guard, Expr::Binary { .. }));
}

#[test]
fn when_guard_dump() {
    assert_eq!(
        stmt_dump(
            "when (s) {\n        Some(x) if (x > 0) -> { println(x) }\n        None -> { }\n    }"
        ),
        "when\n  Var s\n  arm Some(x) if <guard>\n    Call println\n      Var x\n  arm None\n"
    );
}

#[test]
fn literal_pattern_dump_is_source_shaped() {
    assert_eq!(
        stmt_dump("when (value) {\n        Value((false,), 1, \"x\", ()) if (flag) -> { }\n    }"),
        "when\n  Var value\n  arm Value((false,), 1, \"x\", ()) if <guard>\n"
    );
}

#[test]
fn when_expression_dump_marks_guarded_arms() {
    let file = ok(
        "fun choose(value: Boolean, flag: Boolean): Int = when (value) {\n\
             true if (flag) -> 1\n\
             false -> 0\n\
         }\n",
    );
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun choose(value: Boolean, flag: Boolean): Int\n    =\n      WhenExpression\n        Var value\n        arm true if <guard>\n          IntLiteral 1\n        arm false\n          IntLiteral 0\n"
    );
}

#[test]
fn when_positional_pattern_with_literal_and_wildcard() {
    let when = when_with_arms("        Rect(0, _) -> { }\n");
    let Pattern::Positional { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(&elements[0], Pattern::Literal { .. }));
    assert!(matches!(&elements[1], Pattern::Wildcard { .. }));
    assert!(rest.is_none());
}

#[test]
fn when_prefixed_variant_with_payload() {
    let when = when_with_arms("        Option.Some(x) -> { }\n");
    let Pattern::Positional { path, elements, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    let names: Vec<&str> = path.iter().map(|ident| ident.text.as_str()).collect();
    assert_eq!(names, ["Option", "Some"]);
    assert_eq!(elements.len(), 1);
}

#[test]
fn when_named_field_pattern_with_rename_and_rest() {
    let when = when_with_arms("        Named { w, h: height, .. } -> { }\n");
    let Pattern::Named {
        path, fields, rest, ..
    } = &when.arms[0].pattern
    else {
        panic!("expected a named pattern");
    };
    assert_eq!(path.len(), 1);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].field.text, "w");
    assert!(matches!(
        &*fields[0].subpattern,
        Pattern::Binding(name) if name == &fields[0].field
    ));
    assert_eq!(fields[1].field.text, "h");
    assert!(matches!(
        &*fields[1].subpattern,
        Pattern::Binding(name) if name.text == "height"
    ));
    assert!(rest.is_some());
}

#[test]
fn when_named_fields_accept_recursive_subpatterns() {
    let when = when_with_arms(
        "        Named { literal: 0, wild: _, tuple: (x, _), nested: { value, .. }, variant: Some(y), .. } -> { }\n",
    );
    let Pattern::Named { fields, .. } = &when.arms[0].pattern else {
        panic!("expected a named pattern");
    };
    assert_eq!(fields.len(), 5);
    assert!(matches!(&*fields[0].subpattern, Pattern::Literal { .. }));
    assert!(matches!(&*fields[1].subpattern, Pattern::Wildcard { .. }));
    assert!(matches!(&*fields[2].subpattern, Pattern::Tuple { .. }));
    assert!(matches!(
        &*fields[3].subpattern,
        Pattern::Named { path, .. } if path.is_empty()
    ));
    assert!(matches!(
        &*fields[4].subpattern,
        Pattern::Positional { path, .. } if path.len() == 1 && path[0].text == "Some"
    ));
}

#[test]
fn unprefixed_named_pattern_and_recursive_dump() {
    let when = when_with_arms("        { child: { value, .. }, flag: false, .. } -> { }\n");
    let pattern = &when.arms[0].pattern;
    assert!(matches!(pattern, Pattern::Named { path, .. } if path.is_empty()));
    assert_eq!(
        scoop_ast::dump_pattern(pattern),
        "{child: {value, ..}, flag: false, ..}"
    );
}

#[test]
fn field_pattern_span_includes_recursive_rhs() {
    let when = when_with_arms("        S { child: (x, _) } -> { }\n");
    let Pattern::Named { fields, .. } = &when.arms[0].pattern else {
        panic!("expected a named pattern");
    };
    let field = &fields[0];
    assert_eq!(field.span.start, field.field.span.start);
    assert_eq!(
        field.span.end,
        crate::pattern::pattern_span(&field.subpattern).end
    );
}

#[test]
fn when_named_field_pattern_without_rest() {
    let when = when_with_arms("        Point { x, y } -> { }\n");
    let Pattern::Named { fields, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a named pattern");
    };
    assert_eq!(fields.len(), 2);
    assert!(rest.is_none());
}

#[test]
fn when_tuple_pattern() {
    let when = when_with_arms("        (x, 0, _) -> { }\n");
    let Pattern::Tuple { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(elements.len(), 3);
    assert!(matches!(&elements[0], Pattern::Binding(name) if name.text == "x"));
    assert!(matches!(&elements[1], Pattern::Literal { .. }));
    assert!(matches!(&elements[2], Pattern::Wildcard { .. }));
    assert!(rest.is_none());
}

#[test]
fn when_tuple_pattern_with_rest_in_the_middle() {
    let when = when_with_arms("        (x, .., y) -> { }\n");
    let Pattern::Tuple { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a tuple pattern");
    };
    assert_eq!(elements.len(), 2);
    assert!(rest.is_some());
}

#[test]
fn when_tuple_pattern_with_only_rest() {
    let when = when_with_arms("        (..) -> { }\n");
    let Pattern::Tuple { elements, rest, .. } = &when.arms[0].pattern else {
        panic!("expected a tuple pattern");
    };
    assert!(elements.is_empty());
    assert!(rest.is_some());
}

#[test]
fn when_parenthesized_pattern_is_not_a_tuple() {
    // `(x)` mirrors the expression disambiguation: just `x`.
    let when = when_with_arms("        (x) -> { }\n");
    assert!(matches!(&when.arms[0].pattern, Pattern::Binding(name) if name.text == "x"));
}

#[test]
fn when_unit_literal_patterns() {
    let when = when_with_arms("        () -> { }\n");
    let Pattern::Literal { expr, .. } = &when.arms[0].pattern else {
        panic!("expected a literal pattern");
    };
    assert!(matches!(*expr.clone(), Expr::UnitLiteral { .. }));
    let when = when_with_arms("        Unit -> { }\n");
    let Pattern::Literal { expr, .. } = &when.arms[0].pattern else {
        panic!("expected a literal pattern");
    };
    assert!(matches!(*expr.clone(), Expr::UnitLiteral { .. }));
}

#[test]
fn when_literal_patterns() {
    for (source, expect) in [
        ("0", "int"),
        ("\"x\"", "string"),
        ("true", "bool"),
        ("false", "bool"),
    ] {
        let when = when_with_arms(&format!("        {source} -> {{ }}\n"));
        let Pattern::Literal { expr, .. } = &when.arms[0].pattern else {
            panic!("expected a literal pattern");
        };
        match expect {
            "int" => assert!(matches!(*expr.clone(), Expr::IntLiteral(_))),
            "string" => assert!(matches!(*expr.clone(), Expr::StringLiteral { .. })),
            _ => assert!(matches!(*expr.clone(), Expr::BoolLiteral { .. })),
        }
    }
}

#[test]
fn when_nested_patterns() {
    let when = when_with_arms("        Some((a, b)) -> { }\n");
    let Pattern::Positional { elements, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    let Pattern::Tuple {
        elements: inner, ..
    } = &elements[0]
    else {
        panic!("expected a nested tuple pattern");
    };
    assert_eq!(inner.len(), 2);

    let when = when_with_arms("        Some(Some(x)) -> { }\n");
    let Pattern::Positional { elements, .. } = &when.arms[0].pattern else {
        panic!("expected a positional pattern");
    };
    let Pattern::Positional { path, .. } = &elements[0] else {
        panic!("expected a nested positional pattern");
    };
    assert_eq!(path[0].text, "Some");
}

#[test]
fn when_statement_spans() {
    let file = ok("fun main() {\n    when (s) {\n        Red -> { }\n    }\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 52));
    let StatementKind::When(when) = &stmt.kind else {
        panic!("expected a when statement");
    };
    assert_eq!(when.arms.len(), 1);
    assert_eq!(when.arms[0].span, Span::new(36, 46));
    assert!(when.else_body.is_none());
}
