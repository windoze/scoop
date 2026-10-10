//! M35 syntax boundaries: patterns are explicitly introduced by `case`.

use scoop_ast::{Pattern, StatementKind};

use crate::tests::{block_body, err, ok, only_function};

#[test]
fn case_is_contextual_and_introduces_the_entire_recursive_pattern() {
    let file = ok(
        "fun case(case: Int) {\n    when (case) {\n        case Some((value, _)) -> {}\n        case None -> {}\n    }\n}",
    );
    let function = only_function(&file);
    assert_eq!(function.name.text, "case");
    let StatementKind::When(when) = &block_body(function).statements[0].kind else {
        panic!("expected when");
    };
    let Pattern::Positional { elements, .. } = &when.arms[0].pattern else {
        panic!("expected variant pattern");
    };
    assert!(matches!(&elements[0], Pattern::Tuple { .. }));
    assert!(matches!(&when.arms[1].pattern, Pattern::Binding(name) if name.text == "None"));
}

#[test]
fn missing_case_reports_the_pattern_start() {
    let source = "fun f() { when (value) { _ -> {} } }";
    let (span, message) = err(source);
    assert_eq!(&source[span.start as usize..span.end as usize], "_");
    assert_eq!(message, "match patterns require `case` before the pattern");
}

#[test]
fn binding_positions_do_not_require_case() {
    ok("fun f() { val (first, _) = pair; for ((value, _) in values) {} }");
}
