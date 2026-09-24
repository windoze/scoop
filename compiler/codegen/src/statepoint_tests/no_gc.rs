use super::*;

#[test]
fn no_gc_functions_require_the_backend_frame_policy_without_a_gc_strategy() {
    let context = Context::create();
    let expected = ExpectedSafepoints {
        sites: BTreeMap::new(),
        functions: BTreeMap::from([("f".to_string(), GcEffect::NoGc)]),
    };
    let valid = "define void @f() #0 { ret void }\nattributes #0 = { \"frame-pointer\"=\"all\" }";
    verify_rewritten(&parse(&context, valid), &expected).unwrap();
    for invalid in [
        "define void @f() { ret void }",
        "define void @f() #0 { ret void }\nattributes #0 = { \"frame-pointer\"=\"none\" }",
    ] {
        let error = verify_rewritten(&parse(&context, invalid), &expected).unwrap_err();
        assert!(error.0.contains("frame-pointer"), "{error}");
    }
    let unexpected_gc = valid.replace("#0 {", "#0 gc \"statepoint-example\" {");
    let error = verify_rewritten(&parse(&context, &unexpected_gc), &expected).unwrap_err();
    assert!(
        error
            .0
            .contains("NoGc function `f` unexpectedly has GC strategy"),
        "{error}"
    );
}
