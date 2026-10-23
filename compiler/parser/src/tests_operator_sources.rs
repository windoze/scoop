use scoop_ast::{Expr, StatementKind};

fn initializer(text: &str) -> Expr {
    let block = crate::parse_function_body(text).unwrap();
    let StatementKind::ValDecl(value) = &block.statements[0].kind else {
        panic!("the first statement is a value declaration")
    };
    value.init.clone()
}

#[test]
fn binary_and_unary_tokens_keep_their_exact_source_ranges() {
    for operator in [
        "+", "-", "*", "/", "%", "<", "<=", ">", ">=", "==", "!=", "===", "!==", "&&", "||", "..",
        "..<", "in", "!in",
    ] {
        let text = format!("{{ val result = left /* 🌟 */ {operator} right }}");
        let Expr::Binary {
            operator_span: Some(span),
            ..
        } = initializer(&text)
        else {
            panic!("a parsed binary expression has its actual operator token")
        };
        assert_eq!(&text[span.start as usize..span.end as usize], operator);
        assert_eq!(span.start as usize, text.find("*/ ").unwrap() + 3);
    }
    for operator in ["+", "-", "!"] {
        let text = format!("{{ val result = {operator} /* 🌟 */ value }}");
        let Expr::Unary {
            operator_span: Some(span),
            ..
        } = initializer(&text)
        else {
            panic!("a parsed unary expression has its actual operator token")
        };
        assert_eq!(&text[span.start as usize..span.end as usize], operator);
        assert_eq!(span.start as usize, text.find("= ").unwrap() + 2);
    }
}

#[test]
fn nested_operator_ranges_are_independent_of_parentheses_and_comments() {
    let text = "{ val result = - /* outer */ (left + /* inner */ right) }";
    let Expr::Unary {
        operator_span: Some(unary),
        operand,
        ..
    } = initializer(text)
    else {
        panic!("outer unary expression")
    };
    let Expr::Binary {
        operator_span: Some(binary),
        ..
    } = *operand
    else {
        panic!("inner binary expression")
    };
    assert_eq!(&text[unary.start as usize..unary.end as usize], "-");
    assert_eq!(&text[binary.start as usize..binary.end as usize], "+");
}

#[test]
fn abbreviated_membership_conditions_keep_their_actual_tokens() {
    let text = "{ when (value) { in /* 🌟 */ bag -> {}; !in bag -> {}; else -> {} } }";
    let block = crate::parse_function_body(text).unwrap();
    let StatementKind::When(when) = &block.statements[0].kind else {
        panic!("a when statement")
    };
    for (arm, expected) in when.arms.iter().zip(["in", "!in"]) {
        let scoop_ast::WhenArmCondition::Conditions(conditions) = &arm.condition else {
            panic!("a source membership condition")
        };
        let scoop_ast::WhenCondition::In { operator_span, .. } = conditions.first() else {
            panic!("the membership condition retains its token")
        };
        assert_eq!(
            &text[operator_span.start as usize..operator_span.end as usize],
            expected
        );
    }
}
