use super::*;

#[test]
fn nested_constructor_captures_read_the_enclosing_closure() {
    let source = scoop_parser::parse(
        "class Callback(seed: Int) { val nested: () -> () -> Int = { val inner: () -> Int = { seed }; inner } }\n\
         fun main() { Callback(42).nested()() }",
    )
    .unwrap();
    let output = lower_user_output(source).unwrap();
    let mut captures = output
        .export
        .lambdas
        .iter()
        .map(|(_, lambda)| {
            let [capture] = lambda.captures.as_slice() else {
                panic!("each closure captures the constructor parameter");
            };
            capture
        })
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 2);
    captures.sort_by_key(|capture| match capture.source.kind {
        hir::ExprKind::ConstructorParam(_) => 0,
        hir::ExprKind::Capture(_) => 1,
        _ => 2,
    });
    assert_eq!(captures[0].binding, captures[1].binding);
    assert!(matches!(
        captures[0].source.kind,
        hir::ExprKind::ConstructorParam(_)
    ));
    assert!(matches!(
        captures[1].source.kind,
        hir::ExprKind::Capture(binding) if binding == captures[0].binding
    ));
    scoop_mir_lower::lower(&output.local).unwrap();
}

#[test]
fn constructor_captures_compose_with_anonymous_local_and_generic_functions() {
    let source = concat!(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/regressions/constructor-captures/provider/src/callback.scoop"
        )),
        "\nfun main() { Callback<Int>(42).local() }"
    );
    let output = lower_user_output(scoop_parser::parse(source).unwrap()).unwrap();
    scoop_mir_lower::lower(&output.local).unwrap();
}
