use super::*;

#[test]
fn generic_derived_equality_retains_the_bound_field_target() {
    let output = lower_user_output(
        scoop_parser::parse(include_str!(
            "../../../../../tests/fixtures/evaluation-order/derived-binding/program.scoop"
        ))
        .unwrap(),
    )
    .unwrap();
    let dump = hir::dump(&output.export);
    let generic = dump
        .split("fun genericSame")
        .nth(1)
        .expect("generic function");
    let generic = generic.split("\n  fun ").next().unwrap();
    assert!(generic.contains("via Wide -> Wide.equals"), "{generic}");
    assert!(!generic.contains("DerivedEquality"), "{generic}");
}
