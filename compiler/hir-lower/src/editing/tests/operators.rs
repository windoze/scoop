use super::imports::{analyze_files, at};
use super::*;

mod imported;
mod places;

fn references(analysis: &mut DeclarationAnalysis) -> Vec<hir::EditingCallableReference> {
    let function = function(analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("complete operator body: {:?}", analysis.diagnostics())
    };
    facts.callables.clone()
}

#[test]
fn unary_and_binary_operators_reference_the_selected_source_methods() {
    let methods = "class Value {\noperator fun plus(other: Value): Value = this\noperator fun minus(other: Value): Value = this\noperator fun times(other: Value): Value = this\noperator fun div(other: Value): Value = this\noperator fun rem(other: Value): Value = this\noperator fun rangeTo(other: Value): Value = this\noperator fun rangeUntil(other: Value): Value = this\noperator fun contains(other: Value): Boolean = true\noperator fun compareTo(other: Value): Long = 0L\noperator fun equals(other: Value): Boolean = true\noperator fun unaryPlus(): Value = this\noperator fun unaryMinus(): Value = this\noperator fun not(): Boolean = true\n}\n";
    let expressions = [
        ("a + b", "plus", "+"),
        ("a - b", "minus", "-"),
        ("a * b", "times", "*"),
        ("a / b", "div", "/"),
        ("a % b", "rem", "%"),
        ("a..b", "rangeTo", ".."),
        ("a..<b", "rangeUntil", "..<"),
        ("a in b", "contains", "in"),
        ("a !in b", "contains", "!in"),
        ("a < b", "compareTo", "<"),
        ("a <= b", "compareTo", "<="),
        ("a > b", "compareTo", ">"),
        ("a >= b", "compareTo", ">="),
        ("a == b", "equals", "=="),
        ("a != b", "equals", "!="),
        ("+a", "unaryPlus", "+"),
        ("-a", "unaryMinus", "-"),
        ("!a", "not", "!"),
    ];
    let mut text = format!("{methods}fun editing(a: Value, b: Value) {{\n");
    for (index, (expression, _, _)) in expressions.iter().enumerate() {
        text.push_str(&format!("val result{index} = {expression}\n"));
    }
    text.push('}');
    let mut analysis = analyze_files(&[("src/operators.scoop", &text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), expressions.len(), "{references:?}");
    for ((expression, name, operator), reference) in expressions.iter().zip(&references) {
        let start = text.find(expression).unwrap() + expression.find(operator).unwrap();
        assert_eq!(
            reference.span,
            ast::Span::new(start as u32, (start + operator.len()) as u32)
        );
        assert_eq!(
            reference.definition,
            Some(at(
                "src/operators.scoop",
                &text,
                &format!("{name}("),
                name.len()
            ))
        );
    }
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let equals = index.targets_at(&at("src/operators.scoop", &text, "equals(", 6))[0];
    assert_eq!(index.references_to(equals).count(), 3);
    let compare = index.targets_at(&at("src/operators.scoop", &text, "compareTo(", 9))[0];
    assert_eq!(index.references_to(compare).count(), 5);
}

#[test]
fn generic_operator_applications_keep_the_same_declared_target() {
    let text = "class Box<T>(val value: T) { operator fun plus(other: T): T = other }\nfun editing(first: Box<Boolean>, second: Box<String>) { val one = first + true; val two = second + \"text\" }";
    let mut analysis = analyze_files(&[("src/operators.scoop", text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].target, references[1].target);
    assert_eq!(references[0].definition, references[1].definition);
}

#[test]
fn abbreviated_membership_keeps_tokens_without_inventing_equality_tokens() {
    let text = "class Value { operator fun contains(other: Value): Boolean = true; operator fun equals(other: Value): Boolean = true }\nfun editing(value: Value, bag: Value): Boolean = when (value) { in bag -> true; !in bag -> false; bag -> true; else -> false }";
    let mut analysis = analyze_files(&[("src/operators.scoop", text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 2, "{references:?}");
    assert_eq!(references[0].target, references[1].target);
    for (reference, operator) in references.iter().zip(["in", "!in"]) {
        assert_eq!(
            &text[reference.span.start as usize..reference.span.end as usize],
            operator
        );
        assert_eq!(
            reference.definition,
            Some(at("src/operators.scoop", text, "contains(", 8))
        );
    }
}

#[test]
fn builtin_and_derived_operations_do_not_invent_named_source_calls() {
    let text = "struct Value(val value: Boolean)\nclass Ref\nfun editing(flag: Boolean, a: Ref, b: Ref, x: Value, y: Value) { val negative = -1; val signed = +1.0; val both = flag && flag; val same = a === b; val derived = x == y }";
    let mut analysis = analyze_files(&[("src/operators.scoop", text)]);
    assert!(references(&mut analysis).is_empty());
}

#[test]
fn failed_operator_selection_and_cancelled_queries_publish_no_body_references() {
    let text = "class Value { operator fun plus(other: Value): Value = this }\nfun editing(value: Value) { val good = value + value; val bad = value + true }";
    let mut analysis = analyze_files(&[("src/operators.scoop", text)]);
    let function = function(&analysis, "editing");
    assert!(analysis.function_facts(function, || true).is_err());
    assert!(matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at("src/operators.scoop", text, "plus(", 4))[0];
    assert_eq!(index.references_to(target).count(), 1);
    assert!(!index.unavailable.is_empty());
}
